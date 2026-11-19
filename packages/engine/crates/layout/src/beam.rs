//! `beam.cc` (MS4): what the beam engravers call of it (MS4 step 2):
//! `Beam::add_stem` (`beam.cc:106–118`), `Beam::set_beaming`
//! (`:1251–1279`), and the offsets chained on a rest under a beam,
//! `ly:beam::rest-collision-callback` (`:1327–1417`) and
//! `ly:beam::pure-rest-collision-callback` (`:1419–1490`), with what they
//! read of the beam: `get_beam_thickness` (`:120–125`),
//! `get_beam_translation` (`:128–147`), `get_beam_count` (`:150–161`),
//! `normal_stem_count` (`:1296–1301`), and lily-guile.cc's
//! `int_list_to_slice` ([`Slice`]).
//!
//! Then the Beam's callbacks but its stem lengths (MS4 step 3), the music
//! crate's placeholders until then (D67): `ly:beam::calc-normal-stems`
//! (`:163–179`), `::calc-direction` (`:181–239`) with
//! `get_default_dir` (`:874–945`), `set_stem_directions` (`:947–962`) and
//! `consider_auto_knees` (`:964–1047`), `::calc-beaming` (`:241–355`)
//! with `position_with_maximal_common_beams` (`:255–294`), the gaps and
//! accidentals of a tremolo (`get_accidentals`, `get_gaps`, `:362–426`)
//! and `::tremolo-springs-and-rods` (`:428–448`), `::calc-beam-segments`
//! (`:453–681`), `::calc-x-positions` (`:683–707`) with
//! `get_beam_segments`, `::print` (`:726–872`), `::calc-stem-shorten`
//! (`:1049–1081`) with `forced_stem_count` (`:1281–1294`),
//! `::calc-cross-staff` with `is_cross_staff` (`:1492–1509`), and
//! `get_direction_beam_count` (`:1511–1527`), which stem.rs calls; stem.cc's
//! `get_beaming` and `chord_start_y`. Rust under primitives that print as
//! LilyPond's subrs and call it over `%host-call` (D127, [`HOSTS`]), in
//! 05 §5.1's callback table.
//!
//! Then the stems' lengths (MS4 step 4): `ly:beam::set-stem-lengths`
//! (`:1164–1249`) over `calc_stem_y` (`:1097–1157`) and stem.cc's
//! `set_stem_positions`, and `ly:beam::quanting` (`:1083–1095`), a new
//! primitive over `%host-call` that runs beam-quanting.cc's scoring problem
//! (`beam_quanting.rs`) for output-lib.scm's `positions` callbacks.
//!
//! LilyPond 2.26.0's clang fuses the sums of products here (otool): the
//! translation's `2 * staff_space * fract + line * fract`, the rest
//! offsets' stem end, beams' height and each step past the beam, the
//! knees' beams' height, the segments' ends past a break, a beamlet's
//! length and a gap, the stencil's translations and their weighted
//! average, and a stem's end past its beams and a tremolo beam's
//! thickness. Where LilyPond reads past the beam's segments for a
//! feathered beam's lowest one, the translation weighs nothing; where it
//! loops forever on a beaming of no direction, one position is tried.

use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, Heap, HostHandler, Value, Vm};

use crate::beaming_pattern::BeamingPattern;
use crate::callbacks::Callback;
use crate::direction;
use crate::geometry::{Direction, Interval, X, Y, fma, max, min};
use crate::grob;
use crate::grob_array::{self, GrobArray};
use crate::item;
use crate::lookup;
use crate::spanner;
use crate::staff_symbol_referencer::{line_thickness, staff_space, staff_span};
use crate::stem;
use crate::stencil::Stencil;
use crate::unpure;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const DOWN: Direction = -1;
const UP: Direction = 1;

/// `Slice`: an interval of `int`s, empty when its left end is past its
/// right; LilyPond's empty one is `(INT_MAX, -INT_MAX)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slice {
    pub left: i32,
    pub right: i32,
}

impl Slice {
    pub const EMPTY: Slice = Slice {
        left: i32::MAX,
        right: -i32::MAX,
    };

    pub fn add_point(&mut self, p: i32) {
        self.left = self.left.min(p);
        self.right = self.right.max(p);
    }

    pub fn unite(&mut self, h: Slice) {
        self.left = h.left.min(self.left);
        self.right = h.right.max(self.right);
    }

    pub fn intersect(&mut self, h: Slice) {
        self.left = h.left.max(self.left);
        self.right = h.right.min(self.right);
    }

    pub fn is_empty(self) -> bool {
        self.left > self.right
    }

    /// `length`: 0 if empty, wrapping as LilyPond's `int`s do.
    pub fn length(self) -> i32 {
        if self.is_empty() {
            0
        } else {
            self.right.wrapping_sub(self.left)
        }
    }
}

/// `int_list_to_slice`: the slice of the list's numbers.
pub(crate) fn int_list_to_slice(host: &mut dyn Host, mut l: Value) -> Result<Slice, Exception> {
    let mut s = Slice::EMPTY;
    while let (Some(n), Some(rest)) = (host.heap().car(l), host.heap().cdr(l)) {
        if host.heap().is_number(n) {
            s.add_point(stem::to_int(host.heap(), n)?);
        }
        l = rest;
    }
    Ok(s)
}

/// `get_object (me, name)` if it is a grob.
fn grob_object(host: &mut dyn Host, me: Value, name: &str) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern(name);
    let g = grob_array::object(host, me, sym)?;
    Ok(grob_of(host.heap(), g).is_some().then_some(g))
}

/// `from_scm<double> (get_property (me, name), fallback)`.
fn double(host: &mut dyn Host, me: Value, name: &str, fallback: f64) -> Result<f64, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(crate::spacing_options::double_or(host, v, fallback))
}

/// `from_scm (get_property (me, name), Drul_array (a, b))`: a pair of
/// reals, else the fallback.
fn drul(
    host: &mut dyn Host,
    me: Value,
    name: &str,
    fallback: Interval,
) -> Result<Interval, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(grob::number_pair(host.heap(), v).unwrap_or(fallback))
}

/// `add_stem`: the stem `s` added to `me`'s `stems`, given `me` as its
/// `beam`, and a bound of `me`; LilyPond's programming error for a stem
/// that has a beam already.
pub fn add_stem(host: &mut dyn Host, me: Value, s: Value) -> Result<(), Exception> {
    if stem::get_beam(host, s)?.is_some() {
        return grob::unlocated_programming_error(host, "Stem already has beam");
    }

    let sym = host.heap().intern("stems");
    grob_array::add_grob(host, me, sym, s)?;
    let heap = host.heap();
    let sym = heap.intern("beam");
    grob::set_object(heap, s, sym, me);
    spanner::add_bound_item(host, me, s)
}

/// `get_beam_thickness`: `beam-thickness` in staff spaces.
pub fn get_beam_thickness(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    Ok(double(host, me, "beam-thickness", 0.0)? * staff_space(host, me)?)
}

/// `get_beam_translation`: the translation between two adjoining beams.
pub fn get_beam_translation(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    let beam_count = get_beam_count(host, me)?;
    let staff_space = staff_space(host, me)?;
    let line = line_thickness(host, me)?;
    let beam_thickness = get_beam_thickness(host, me)?;
    let fract = double(host, me, "length-fraction", 1.0)?;

    // if fract != 1.0, as is the case for grace notes, we want the gap to
    // decrease too. To achieve this, we divide the thickness by fract
    Ok(if beam_count < 4 {
        (fma(2.0 * staff_space, fract, line * fract) - beam_thickness) / 2.0
    } else {
        (fma(3.0 * staff_space, fract, line * fract) - beam_thickness) / 3.0
    })
}

/// `get_beam_count`: the most beams of its stems.
pub fn get_beam_count(host: &mut dyn Host, me: Value) -> Result<i32, Exception> {
    let mut m = 0;
    for stem in grob_array::extract_grob_array(host, me, "stems")? {
        m = m.max(
            stem::beam_multiplicity(host, stem)?
                .length()
                .wrapping_add(1),
        );
    }
    Ok(m)
}

/// `normal_stem_count`: how many `normal-stems` it has.
pub fn normal_stem_count(host: &mut dyn Host, me: Value) -> Result<usize, Exception> {
    Ok(grob_array::extract_grob_array(host, me, "normal-stems")?.len())
}

/// `set_beaming`: each stem's `beaming` on each side it has none of, from
/// the pattern; an invisible stem inside the beam takes the fewer of its
/// two sides, and with `clip-edges` the beam's two ends none.
pub fn set_beaming(
    host: &mut dyn Host,
    me: Value,
    beaming: &BeamingPattern,
) -> Result<(), Exception> {
    let stems = grob_array::extract_grob_array(host, me, "stems")?;

    for (i, &stem) in stems.iter().enumerate() {
        // Don't overwrite user settings.
        for d in [LEFT, RIGHT] {
            let beaming_prop = grob::property(host, stem, "beaming")?;
            let heap = host.heap();
            let side = if d == LEFT {
                heap.car(beaming_prop)
            } else {
                heap.cdr(beaming_prop)
            };
            if beaming_prop == Value::NULL || side == Some(Value::NULL) {
                let mut count = beaming.beamlet_count(i, d);
                if i > 0 && i + 1 < stems.len() && stem::is_invisible(host, stem)? {
                    count = count.min(beaming.beamlet_count(i, -d));
                }

                if ((i == 0 && d == LEFT) || (i == stems.len() - 1 && d == RIGHT))
                    && stems.len() > 1
                    && grob::property(host, me, "clip-edges")? == Value::TRUE
                {
                    count = 0;
                }

                stem::set_beaming(host, stem, count as i32, d)?;
            }
        }
    }
    Ok(())
}

/// `rest_collision_callback`: the offset `prev_offset` (0 if no number) of
/// a rest under a beam, moved clear of the beam its stem has, by half
/// staff spaces, and by whole ones inside the staff; unchanged for a rest
/// with a `staff-position`, or without a beam of normal stems.
pub fn rest_collision_callback(
    host: &mut dyn Host,
    rest: Value,
    prev_offset: Value,
) -> Result<Value, Exception> {
    let prev_offset = if host.heap().is_number(prev_offset) {
        prev_offset
    } else {
        host.heap().integer(0)
    };

    let position = grob::property(host, rest, "staff-position")?;
    if host.heap().is_number(position) {
        return Ok(prev_offset);
    }

    let Some(stem) = grob_object(host, rest, "stem")? else {
        return Ok(prev_offset);
    };

    let Some(beam) = grob_object(host, stem, "beam")? else {
        return Ok(prev_offset);
    };
    if !grob::has_interface(host.heap(), beam, "beam-interface")
        || normal_stem_count(host, beam)? == 0
    {
        return Ok(prev_offset);
    }

    let common_y = grob::common_refpoint(host.heap(), rest, beam, Y).unwrap_or(Value::NULL);

    let mut pos = drul(host, beam, "positions", Interval::new(0.0, 0.0))?;

    let beam_y = grob::relative_coordinate(host, beam, common_y, Y)?;
    pos.left += beam_y;
    let beam_y = grob::relative_coordinate(host, beam, common_y, Y)?;
    pos.right += beam_y;

    let staff_space = staff_space(host, rest)?;

    pos.left *= staff_space;
    pos.right *= staff_space;

    let dy = pos.right - pos.left;

    let stems = grob_array::extract_grob_array(host, beam, "stems")?;
    let common =
        grob::common_refpoint_of_array(host.heap(), &stems, Some(beam), X).unwrap_or(Value::NULL);

    let x_span = drul(host, beam, "X-positions", Interval::new(0.0, 0.0))?;
    let x0 = x_span.left;
    let dx = x_span.length();
    let slope = if dy != 0.0 && dx != 0.0 { dy / dx } else { 0.0 };

    let d = direction::get_grob_direction(host, stem)?;
    let stem_x = grob::relative_coordinate(host, stem, common, X)?;
    let stem_y = fma(stem_x - x0, slope, pos.left);

    let beam_translation = get_beam_translation(host, beam)?;
    let beam_thickness = get_beam_thickness(host, beam)?;

    // TODO: this is not strictly correct for 16th knee beams.
    let beam_count = stem::beam_multiplicity(host, stem)?
        .length()
        .wrapping_add(1);

    let height_of_my_beams = fma(
        f64::from(beam_count.wrapping_sub(1)),
        beam_translation,
        beam_thickness / 2.0,
    );
    let beam_y = fma(-f64::from(d), height_of_my_beams, stem_y);

    let offset = host.heap().to_f64(prev_offset).unwrap_or(0.0);
    let mut rest_extent = grob::extent(host, rest, rest, Y)?;
    let parent_relative = match grob::get_parent(host.heap(), rest, Y) {
        Some(p) => grob::relative_coordinate(host, p, common_y, Y)?,
        None => 0.0,
    };
    rest_extent.translate(offset + parent_relative);

    let rest_dim = rest_extent.at(d);
    let minimum_distance = staff_space
        * (double(host, stem, "stemlet-length", 0.0)?
            + double(host, rest, "minimum-distance", 0.0)?);

    let d = f64::from(d);
    let mut shift = d * min(d * (fma(-d, minimum_distance, beam_y) - rest_dim), 0.0);

    shift /= staff_space;

    // Always move discretely by half spaces
    shift = libm::ceil(libm::fabs(shift * 2.0)) / 2.0 * sign(shift);

    let mut staff_span = staff_span(host, rest)?;
    staff_span.scale(staff_space / 2.0);

    // Inside staff, move by whole spaces
    let contains = |x: f64| staff_span.left <= x && x <= staff_span.right;
    if contains(fma(staff_space, shift, rest_extent.at(d as Direction)))
        || contains(fma(staff_space, shift, rest_extent.at(-d as Direction)))
    {
        shift = libm::ceil(libm::fabs(shift)) * sign(shift);
    }

    Ok(host.heap().flonum(fma(staff_space, shift, offset)))
}

/// `sign`: 0 for 0, else 1 if positive and -1 if not (NaN among them).
fn sign(x: f64) -> f64 {
    if x == 0.0 {
        0.0
    } else if x > 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// `pure_rest_collision_callback`: the offset `prev_offset` (0 if no
/// number) of a rest under a beam estimated from its neighbouring heads:
/// moved by whole staff spaces away from the beam, which is taken four
/// positions past their average and never more than two past the staff's
/// center; unchanged without a beam of normal stems and a direction.
pub fn pure_rest_collision_callback(
    host: &mut dyn Host,
    me: Value,
    prev_offset: Value,
) -> Result<Value, Exception> {
    let prev_offset = if host.heap().is_number(prev_offset) {
        prev_offset
    } else {
        host.heap().integer(0)
    };

    let Some(stem) = grob_object(host, me, "stem")? else {
        return Ok(prev_offset);
    };
    let Some(beam) = grob_object(host, stem, "beam")? else {
        return Ok(prev_offset);
    };
    if normal_stem_count(host, beam)? == 0 {
        return Ok(prev_offset);
    }
    let dir = {
        let heap = host.heap();
        let sym = heap.intern("direction");
        match grob_of(heap, beam) {
            Some((arena, id)) => grob::property_data(heap, &arena, id, beam, sym),
            None => Value::NULL,
        }
    };
    if !matches!(dir.as_fixnum(), Some(-1..=1)) {
        return Ok(prev_offset);
    }

    let ss = staff_space(host, me)?;

    let stems = grob_array::extract_grob_array(host, beam, "stems")?;
    let mut my_stems = alloc::vec::Vec::new();
    let mut idx = None;

    for &s in &stems {
        if stem::head_count(host, s)? != 0 || s == stem {
            my_stems.push(s);
        }

        if s == stem {
            idx = Some(my_stems.len() - 1);
        }
    }

    let (left, right) = match idx {
        None => return Ok(prev_offset),
        Some(_) if my_stems.len() == 1 => return Ok(prev_offset),
        Some(0) => (my_stems[1], my_stems[1]),
        Some(i) if i == my_stems.len() - 1 => (my_stems[i - 1], my_stems[i - 1]),
        Some(i) => (my_stems[i - 1], my_stems[i + 1]),
    };

    // Estimate the closest beam to be four positions away from the heads,
    let beamdir = direction::get_grob_direction(host, beam)?;
    let l = stem::head_positions(host, left)?.at(beamdir);
    let r = stem::head_positions(host, right)?.at(beamdir);
    let beamdir = f64::from(beamdir);
    // four staff-positions
    let mut beam_pos = fma(beamdir, 4.0, (l + r) / 2.0);
    // and that the closest beam never crosses staff center by more than two
    // positions
    beam_pos = (beam_pos * beamdir).max(-2.0) * beamdir;

    let minimum_distance = ss
        * (double(host, stem, "stemlet-length", 0.0)? + double(host, me, "minimum-distance", 0.0)?);
    let extent = grob::extent(host, me, me, Y)?.at(beamdir as Direction);
    let offset = fma(-minimum_distance, beamdir, beam_pos * ss / 2.0) - extent;
    let previous = host.heap().to_f64(prev_offset).unwrap_or(0.0);

    // Always move by a whole number of staff spaces, always away from the
    // beam
    let offset = fma(
        ss * libm::floor(((offset - previous) / ss * beamdir).min(0.0)),
        beamdir,
        previous,
    );

    Ok(host.heap().flonum(offset))
}

// --- the beam's callbacks (MS4 step 3) ---------------------------------------

/// `Beam_segment`: a beam's line at its vertical count, from one end to
/// the other.
#[derive(Clone, Copy, Debug)]
pub struct BeamSegment {
    pub vertical_count: i32,
    pub horizontal: Interval,
}

impl Default for BeamSegment {
    fn default() -> BeamSegment {
        BeamSegment {
            vertical_count: 0,
            horizontal: Interval::EMPTY,
        }
    }
}

/// `Beam_stem_segment`: a beamlet of one stem on one side.
#[derive(Clone, Copy)]
struct BeamStemSegment {
    max_connect: i32,
    stem: Value,
    width: f64,
    stem_x: f64,
    /// `2 i` for stem `i`'s left side, `2 i + 1` for its right.
    rank: usize,
    stem_index: usize,
    gapped: bool,
    dir: Direction,
}

/// `Direction (x)` of a real: its sign, 0 for 0; a NaN by its sign bit.
fn dir_of(x: f64) -> Direction {
    if x != 0.0 {
        if x.is_sign_negative() { -1 } else { 1 }
    } else {
        0
    }
}

/// `get_property_data (me, name)`: what is set for `name`, no callback
/// called.
fn data(host: &mut dyn Host, me: Value, name: &str) -> Value {
    let heap = host.heap();
    let sym = heap.intern(name);
    match grob_of(heap, me) {
        Some((arena, id)) => grob::property_data(heap, &arena, id, me, sym),
        None => Value::NULL,
    }
}

/// `is_scm<Direction>`: an exact -1, 0 or 1.
fn is_direction(v: Value) -> bool {
    matches!(v.as_fixnum(), Some(-1..=1))
}

/// `from_scm<Direction> (get_property (me, name))`.
fn direction_property(host: &mut dyn Host, me: Value, name: &str) -> Result<Direction, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(crate::dynamic_align_engraver::direction(v))
}

/// `to_scm (Interval)`: a pair of reals.
fn interval_value(heap: &mut Heap, iv: Interval) -> Value {
    let (l, r) = (heap.flonum(iv.left), heap.flonum(iv.right));
    heap.cons(l, r)
}

/// `calc_normal_stems`: the beam's stems that are normal
/// (`Stem::is_normal_stem`), a grob array.
pub fn calc_normal_stems(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let mut ga = GrobArray::new();
    for stem in grob_array::extract_grob_array(host, me, "stems")? {
        if stem::is_normal_stem(host, stem)? {
            ga.grobs.push(stem);
        }
    }
    Ok(host.heap().wrap(ga))
}

/// `first_normal_stem`.
fn first_normal_stem(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    Ok(grob_array::extract_grob_array(host, me, "normal-stems")?
        .first()
        .copied())
}

/// `calc_direction`: the beam's direction, its stems' directions set to it
/// where they have none, after its knees are considered. A beam with
/// fewer than two normal stems takes its first one's (a chord tremolo's
/// first stem's), down for a Kievan head of none; a beam with no stems at
/// all is removed with LilyPond's warning.
pub fn calc_direction(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    // Beams with less than 2 two stems don't make much sense, but could
    // happen when you do
    //
    //   r8[ c8 r8]
    let mut dir = 0;

    let count = normal_stem_count(host, me)?;
    if count < 2 {
        let stems = grob_array::extract_grob_array(host, me, "stems")?;
        if stems.is_empty() {
            grob::warning(host, me, "removing beam with no stems")?;
            grob::suicide(host.heap(), me);

            return Ok(Value::UNSPECIFIED);
        }
        // This happens for chord tremolos.
        let stem = first_normal_stem(host, me)?.unwrap_or(stems[0]);

        let d = data(host, stem, "direction");
        dir = if is_direction(d) {
            crate::dynamic_align_engraver::direction(d)
        } else {
            direction_property(host, stem, "default-direction")?
        };

        let heads = grob_array::extract_grob_array(host, stem, "note-heads")?;
        // default position of Kievan heads with beams is down placing this
        // here avoids warnings downstream
        if let Some(&head) = heads.first() {
            let kievan = host.heap().intern("kievan");
            if grob::property(host, head, "style")? == kievan && dir == 0 {
                dir = DOWN;
            }
        }
    }

    if dir == 0 {
        dir = get_default_dir(host, me)?;
    }

    if count >= 1 {
        consider_auto_knees(host, me)?;
    }

    set_stem_directions(host, me, dir)?;

    Ok(host.heap().integer(i64::from(dir)))
}

/// `position_with_maximal_common_beams`: we want a maximal number of
/// shared beams, but if there is choice, we take the one that is closest
/// to the end of the stem. A `left_dir` of 0, where LilyPond loops
/// forever, tries one position.
fn position_with_maximal_common_beams(
    host: &mut dyn Host,
    left_beaming: Value,
    right_beaming: Value,
    left_dir: Direction,
    right_dir: Direction,
    special_shift: bool,
) -> Result<i32, Exception> {
    let left_cdr = host.heap().cdr(left_beaming).unwrap_or(Value::NULL);
    let lslice = int_list_to_slice(host, left_cdr)?;
    let right_car = host.heap().car(right_beaming).unwrap_or(Value::NULL);
    let mut right = Vec::new();
    let mut p = right_car;
    while let (Some(n), Some(rest)) = (host.heap().car(p), host.heap().cdr(p)) {
        right.push(stem::to_int(host.heap(), n)?);
        p = rest;
    }

    let mut best_count = 0;
    let mut best_start = 0;
    let mut i = slice_at(lslice, -left_dir);
    while i
        .wrapping_sub(slice_at(lslice, left_dir))
        .wrapping_mul(left_dir)
        <= 0
    {
        let mut count = 0;
        for &beam_no in &right {
            let k = (-right_dir).wrapping_mul(beam_no).wrapping_add(i);
            let heap = host.heap();
            let k = heap.integer(i64::from(k));
            let mut l = left_cdr;
            while let (Some(x), Some(rest)) = (heap.car(l), heap.cdr(l)) {
                if heap.eqv(k, x) {
                    count += 1;
                    break;
                }
                l = rest;
            }
        }

        // TODO: consider flipping left_dir based on value of special_shift
        // instead of the below conjunction and calculation of
        // `new_beam_pos` in Beam::calc_beaming
        if count > best_count || (count == best_count && !special_shift) {
            best_count = count;
            best_start = i;
        }
        if left_dir == 0 {
            break;
        }
        i = i.wrapping_add(left_dir);
    }

    Ok(best_start)
}

/// `Slice`'s end `d`: the left one for `LEFT`, the right one otherwise.
fn slice_at(s: Slice, d: Direction) -> i32 {
    if d < 0 { s.left } else { s.right }
}

/// `calc_beaming`: each stem's `beaming` moved to the beam positions of
/// its beam, where the most beams continue from the last stem with any on
/// its right, or past the previous ones of its direction for a stem that
/// turns where the beam is subdivided; `'()`.
pub fn calc_beaming(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let done = calc_beaming_rooted(host, me);
    host.heap().unroot_to(depth);
    done
}

fn calc_beaming_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let stems = grob_array::extract_grob_array(host, me, "stems")?;

    // When a kneed beam alternates stem directions, the previous Slice of
    // the same direction of the current direction (index 0) should be
    // remembered should the current stem be specially treated as explained
    // in the comment below. The previous Slice of the opposite direction
    // (index 1) would shift to index 0 to prepare for a future possible
    // direction flip
    let mut first_slice_of_prev_dirs = [Slice { left: 0, right: 0 }; 2];

    let heap = host.heap();
    let zero = heap.integer(0);
    let zero = heap.list(&[zero]);
    let mut last_beaming = heap.cons(Value::NULL, zero);
    host.heap().root(last_beaming);
    let mut last_dir = 0;
    let mut last_right_beam_count = 0;
    for this_stem in stems {
        let this_beaming = grob::property(host, this_stem, "beaming")?;
        if host.heap().car(this_beaming).is_some() {
            let this_dir = direction::get_grob_direction(host, this_stem)?;

            // Gould's pg 316-317 explicitly allows beam corners when both
            // cases of "outer notes of a subdivision [having] opposite stem
            // direction" and "subdivided group [occuring] at the end of
            // main beam." If special_shift is always false, then old
            // behavior of "funky" beams that incrementally shift the main
            // beam persists
            let right_beaming = stem::get_beaming(host, this_stem, RIGHT)?;
            let mut special_shift = this_dir * last_dir < 0
                // treat specially if left side of current stem is subdivided
                && last_right_beam_count >= stem::get_beaming(host, this_stem, LEFT)?
                // do not treat specially if previous stem had fractional beam
                && last_right_beam_count < right_beaming;
            let start_point = position_with_maximal_common_beams(
                host,
                last_beaming,
                this_beaming,
                if last_dir != 0 { last_dir } else { this_dir },
                this_dir,
                special_shift,
            )?;
            if special_shift {
                special_shift = this_dir
                    * slice_at(first_slice_of_prev_dirs[0], this_dir).wrapping_sub(start_point)
                    > 0;
            }

            let mut new_slice = Slice::EMPTY;
            for d in [LEFT, RIGHT] {
                new_slice = Slice::EMPTY;
                let heap = host.heap();
                let mut s = if d == LEFT {
                    heap.car(this_beaming)
                } else {
                    heap.cdr(this_beaming)
                }
                .unwrap_or(Value::NULL);
                while let (Some(n), Some(rest)) = (host.heap().car(s), host.heap().cdr(s)) {
                    let n = stem::to_int(host.heap(), n)?;
                    let new_beam_pos = if special_shift {
                        slice_at(first_slice_of_prev_dirs[1], -this_dir)
                            .wrapping_add(this_dir.wrapping_mul(n))
                    } else {
                        start_point.wrapping_sub(this_dir.wrapping_mul(n))
                    };
                    new_slice.add_point(new_beam_pos);
                    let heap = host.heap();
                    let v = heap.integer(i64::from(new_beam_pos));
                    heap.set_car(s, v);
                    s = rest;
                }
            }

            if !new_slice.is_empty() && this_dir != 0 && this_dir != last_dir {
                first_slice_of_prev_dirs = [first_slice_of_prev_dirs[1], new_slice];
            }

            let heap = host.heap();
            let right = heap.cdr(this_beaming).unwrap_or(Value::NULL);
            if heap.car(right).is_some() {
                last_beaming = this_beaming;
                host.heap().root(last_beaming);
                last_dir = this_dir;
                last_right_beam_count = right_beaming;
            }
        }
    }

    Ok(Value::NULL)
}

/// `get_accidentals`: the accidentals of the last stem's heads.
fn get_accidentals(host: &mut dyn Host, me: Value) -> Result<Vec<Value>, Exception> {
    let stems = grob_array::extract_grob_array(host, me, "stems")?;
    let mut accs = Vec::new();
    if let Some(&last_stem) = stems.last() {
        for note_head in grob_array::extract_grob_array(host, last_stem, "note-heads")? {
            if let Some(acc) = grob_object(host, note_head, "accidental-grob")? {
                accs.push(acc);
            }
        }
    }
    Ok(accs)
}

/// `get_gaps`: the beam's `gap` at each end, the right one past the last
/// stem's accidentals (and `accidental-padding`) for a beam of whole notes
/// or longer (a tremolo's).
fn get_gaps(host: &mut dyn Host, me: Value, commonx: Value) -> Result<Interval, Exception> {
    let stems = grob_array::extract_grob_array(host, me, "stems")?;
    let gap_length = double(host, me, "gap", 0.0)?;
    let mut gap_lengths = Interval::new(gap_length, gap_length);

    if let Some(&first) = stems.first()
        && stem::duration_log(host, first)? <= 0
    {
        let accs = get_accidentals(host, me)?;

        if !accs.is_empty() {
            let heap = host.heap();
            let list = heap.list(&accs);
            let axis = heap.integer(0);
            let ext = crate::call_lily(host, "ly:relative-group-extent", &[list, commonx, axis])?;
            let accs_ext = grob::number_pair(host.heap(), ext).unwrap_or(Interval::EMPTY);
            if !accs_ext.is_empty() {
                let accs_length = accs_ext.length();
                let acc_padding = double(host, me, "accidental-padding", 1.0)?;
                gap_lengths.right += accs_length + acc_padding;
            }
        }
    }
    Ok(gap_lengths)
}

/// `tremolo_springs_and_rods`: the spanner's spacing rods for a tremolo
/// beam of whole notes or longer whose last stem has accidentals.
pub fn tremolo_springs_and_rods(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let stems = grob_array::extract_grob_array(host, me, "stems")?;

    if let Some(&first) = stems.first() {
        let accs = get_accidentals(host, me)?;

        if !accs.is_empty() && stem::duration_log(host, first)? <= 0 {
            spanner::set_spacing_rods(host, me)?;
        }
    }

    Ok(Value::UNSPECIFIED)
}

/// `commonx` of `print` and `calc_beam_segments`: the stems' common
/// reference point on X with the beam's, then with its bounds'.
fn common_x(host: &mut dyn Host, me: Value, stems: &[Value]) -> Result<Value, Exception> {
    let mut commonx = grob::common_refpoint_of_array(host.heap(), stems, Some(me), X);
    for d in [LEFT, RIGHT] {
        if let (Some(b), Some(c)) = (spanner::get_bound(host, me, d)?, commonx) {
            commonx = grob::common_refpoint(host.heap(), b, c, X);
        }
    }
    Ok(commonx.unwrap_or(Value::NULL))
}

/// `calc_beam_segments`: the beam's segments, from the stems' `beaming`:
/// at each vertical count the beamlets of adjacent stem sides joined, each
/// ending at its stem's middle (less the gap of a tremolo's gapped
/// beamlet) or, away from it, a beamlet's length (shorter between close
/// stems) or past the line's end at a break; a list of alists with
/// `vertical-count` and `horizontal`.
pub fn calc_beam_segments(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    // ugh, this has a side-effect that we need to ensure that Stem.beaming
    // is correct
    grob::property(host, me, "beaming")?;

    let stems = grob_array::extract_grob_array(host, me, "stems")?;

    let commonx = common_x(host, me, &stems)?;

    let gap_count = {
        let v = grob::property(host, me, "gap-count")?;
        crate::stem_engraver::int_or(host.heap(), v, 0)
    };
    let gap_lengths = get_gaps(host, me, commonx)?;

    let mut stem_segments: alloc::collections::BTreeMap<i32, Vec<BeamStemSegment>> =
        alloc::collections::BTreeMap::new();
    let lt = line_thickness(host, me)?;

    // There are two concepts of "rank" that are used in the following
    // code. The beam_rank is the vertical position of the beam (larger
    // numbers are closer to the noteheads). Beam_stem_segment.rank_, on
    // the other hand, is the horizontal position of the segment (this is
    // incremented by two for each stem; the beam segment on the right side
    // of the stem has a higher rank (by one) than its neighbour to the
    // left).
    let mut ranks = Slice::EMPTY;
    for (i, &stem) in stems.iter().enumerate() {
        let stem_width = double(host, stem, "thickness", 1.0)? * lt;
        let stem_x = grob::relative_coordinate(host, stem, commonx, X)?;
        let beaming = grob::property(host, stem, "beaming")?;

        for d in [LEFT, RIGHT] {
            let heap = host.heap();
            let side = if d == LEFT {
                heap.car(beaming)
            } else {
                heap.cdr(beaming)
            };
            let Some(side) = side else {
                return Err(crate::wrong_type(
                    heap,
                    Some(if d == LEFT { "car" } else { "cdr" }),
                    1,
                    beaming,
                    "pair",
                ));
            };
            let mut beam_ranks = Vec::new();
            let mut s = side;
            while let (Some(n), Some(rest)) = (heap.car(s), heap.cdr(s)) {
                if heap.is_exact_integer(n) {
                    beam_ranks.push(n);
                }
                s = rest;
            }
            // Find the maximum and minimum beam ranks. Given that RANKS is
            // never reset to empty, the interval will always be smallest
            // for the left beamlet of the first stem, and then it might
            // grow. Do we really want this? (It only affects the tremolo
            // gaps) --jneem
            let mut beam_rank_ints = Vec::new();
            for &n in &beam_ranks {
                let beam_rank = stem::to_int(host.heap(), n)?;
                ranks.add_point(beam_rank);
                beam_rank_ints.push(beam_rank);
            }

            for beam_rank in beam_rank_ints {
                let max_connect = {
                    let v = grob::property(host, stem, "max-beam-connect")?;
                    crate::stem_engraver::int_or(host.heap(), v, 1000)
                };
                let stem_dir = direction::get_grob_direction(host, stem)?;
                let seg = BeamStemSegment {
                    stem,
                    stem_x,
                    rank: 2 * i + usize::from(d == RIGHT),
                    width: stem_width,
                    stem_index: i,
                    dir: d,
                    max_connect,
                    gapped: stem_dir.wrapping_mul(beam_rank)
                        < stem_dir
                            .wrapping_mul(slice_at(ranks, -stem_dir))
                            .wrapping_add(gap_count),
                };
                stem_segments.entry(beam_rank).or_default().push(seg);
            }
        }
    }

    let break_overshoot = drul(host, me, "break-overshoot", Interval::new(-0.5, 0.0))?;

    let mut segments: Vec<BeamSegment> = Vec::new();
    for (&vertical_count, unsorted_segs) in &stem_segments {
        let mut segs = unsorted_segs.clone();
        lily_engine_skyline::sort(&mut segs, &|a: &BeamStemSegment, b: &BeamStemSegment| {
            a.rank < b.rank
        });

        let mut current = BeamSegment::default();

        // Iterate over all of the segments of the current beam rank,
        // merging the adjacent Beam_stem_segments into one Beam_segment
        // when appropriate.
        for j in 0..segs.len() {
            // Keeping track of the different directions here is a little
            // tricky. segs[j].dir_ is the direction of the beam segment
            // relative to the stem (ie. segs[j].dir_ == LEFT if the beam
            // segment sticks out to the left of its stem) whereas event_dir
            // refers to the edge of the beam segment that we are currently
            // looking at (ie. if segs[j].dir_ == event_dir then we are
            // looking at that edge of the beam segment that is furthest
            // from its stem).
            let seg = segs[j];
            for event_dir in [LEFT, RIGHT] {
                // on_line_bound: whether the current segment is on the
                // boundary of the WHOLE beam; on_beam_bound: whether the
                // current segment is on the boundary of just that part of
                // the beam with the current beam_rank
                let on_line_bound = if seg.dir == LEFT {
                    seg.stem_index == 0
                } else {
                    seg.stem_index == stems.len() - 1
                };
                let on_beam_bound = if event_dir == LEFT {
                    j == 0
                } else {
                    j == segs.len() - 1
                };
                let inside_stem = if event_dir == LEFT {
                    seg.stem_index > 0
                } else {
                    seg.stem_index + 1 < stems.len()
                };

                let event = on_beam_bound || {
                    let next = segs[if event_dir == LEFT { j - 1 } else { j + 1 }];
                    seg.rank.abs_diff(next.rank) > 1
                        || (vertical_count.wrapping_abs() >= seg.max_connect
                            || vertical_count.wrapping_abs() >= next.max_connect)
                };

                if !event {
                    // Then this edge of the current segment is irrelevant
                    // because it will be connected with the next segment in
                    // the event_dir direction. If we skip the left edge
                    // here, the right edge of the previous segment has
                    // already been skipped since the conditions are
                    // symmetric
                    continue;
                }

                current.vertical_count = vertical_count;
                *current.horizontal.at_mut(event_dir) = seg.stem_x;
                let ed = f64::from(event_dir);
                if seg.dir == event_dir {
                    // then we are examining the edge of a beam segment that
                    // is furthest from its stem.
                    let bound = spanner::get_bound(host, me, event_dir)?;
                    match bound {
                        Some(bound)
                            if on_line_bound && item::break_status_dir(host.heap(), bound) != 0 =>
                        {
                            let axis = host.heap().integer(0);
                            let e = crate::call_lily(
                                host,
                                "%generic-bound-extent",
                                &[bound, commonx, axis],
                            )?;
                            let ext = grob::number_pair(host.heap(), e).unwrap_or(Interval::EMPTY);
                            *current.horizontal.at_mut(event_dir) =
                                fma(ed, break_overshoot.at(event_dir), ext.right);
                        }
                        _ => {
                            let stem = stems[seg.stem_index];
                            let beamlet_length = drul(
                                host,
                                stem,
                                "beamlet-default-length",
                                Interval::new(1.1, 1.1),
                            )?;
                            let max_proportion = drul(
                                host,
                                stem,
                                "beamlet-max-length-proportion",
                                Interval::new(0.75, 0.75),
                            )?;
                            let mut length = beamlet_length.at(seg.dir);

                            if inside_stem {
                                let neighbor_stem = stems[if event_dir == LEFT {
                                    seg.stem_index - 1
                                } else {
                                    seg.stem_index + 1
                                }];
                                let neighbor_stem_x =
                                    grob::relative_coordinate(host, neighbor_stem, commonx, X)?;

                                length = min(
                                    length,
                                    libm::fabs(neighbor_stem_x - seg.stem_x)
                                        * max_proportion.at(seg.dir),
                                );
                            }
                            let h = current.horizontal.at(event_dir);
                            *current.horizontal.at_mut(event_dir) = fma(ed, length, h);
                        }
                    }
                } else {
                    // we are examining the edge of a beam segment that is
                    // closest (ie. touching, unless there is a gap) its
                    // stem.
                    *current.horizontal.at_mut(event_dir) += ed * seg.width / 2.0;
                    if seg.gapped {
                        let h = current.horizontal.at(event_dir);
                        *current.horizontal.at_mut(event_dir) =
                            fma(-ed, gap_lengths.at(event_dir), h);

                        if stem::is_invisible(host, seg.stem)? {
                            // Need to do this in case of whole notes. We
                            // don't want the heads to collide with the
                            // beams.
                            for head in
                                grob_array::extract_grob_array(host, seg.stem, "note-heads")?
                            {
                                let ext = grob::extent(host, head, commonx, X)?;
                                let h = current.horizontal.at(event_dir);
                                *current.horizontal.at_mut(event_dir) = ed
                                    * min(
                                        ed * h,
                                        fma(
                                            ed,
                                            ext.at(-event_dir),
                                            -gap_lengths.at(event_dir) / 2.0,
                                        ),
                                    );
                            }
                        }
                    }
                }

                if event_dir == RIGHT {
                    segments.push(current);
                    current = BeamSegment::default();
                }
            }
        }
    }

    let heap = host.heap();
    let (vertical_count, horizontal) = (heap.intern("vertical-count"), heap.intern("horizontal"));
    let mut segments_scm = Value::NULL;
    let depth = heap.root_depth();
    for s in segments.iter().rev() {
        let n = heap.integer(i64::from(s.vertical_count));
        let a = heap.cons(vertical_count, n);
        heap.root(a);
        let iv = interval_value(heap, s.horizontal);
        let b = heap.cons(horizontal, iv);
        heap.root(b);
        let segment = heap.list(&[a, b]);
        segments_scm = heap.cons(segment, segments_scm);
        heap.root(segments_scm);
    }
    heap.unroot_to(depth);

    Ok(segments_scm)
}

/// `calc_x_positions`: the span of the beam's segments, or, for a beam of
/// none (two skips beamed), its own X coordinate at both ends.
pub fn calc_x_positions(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let segments = grob::property(host, me, "beam-segments")?;
    let mut x_positions = Interval::EMPTY;
    let heap = host.heap();
    let horizontal = heap.intern("horizontal");
    let mut s = segments;
    while let (Some(segment), Some(rest)) = (heap.car(s), heap.cdr(s)) {
        let h = grob::assq_ref(heap, horizontal, segment).unwrap_or(Value::NULL);
        x_positions.unite(grob::number_pair(heap, h).unwrap_or(Interval::new(0.0, 0.0)));
        s = rest;
    }

    // Case for beams without segments (i.e. uniting two skips with a beam)
    // TODO: should issue a warning?  warning likely issued downstream, but
    // couldn't hurt...
    if x_positions.is_empty() {
        let stems = grob_array::extract_grob_array(host, me, "stems")?;
        let common_x =
            grob::common_refpoint_of_array(host.heap(), &stems, Some(me), X).unwrap_or(Value::NULL);
        for d in [LEFT, RIGHT] {
            *x_positions.at_mut(d) = grob::relative_coordinate(host, me, common_x, X)?;
        }
    }
    Ok(interval_value(host.heap(), x_positions))
}

/// `get_beam_segments`: its `beam-segments` read back.
pub fn get_beam_segments(host: &mut dyn Host, me: Value) -> Result<Vec<BeamSegment>, Exception> {
    let segments_scm = grob::property(host, me, "beam-segments")?;
    let heap = host.heap();
    let (vertical_count, horizontal) = (heap.intern("vertical-count"), heap.intern("horizontal"));
    let mut segments = Vec::new();
    let mut s = segments_scm;
    while let (Some(segment), Some(rest)) = (heap.car(s), heap.cdr(s)) {
        let vc = grob::assq_ref(heap, vertical_count, segment).unwrap_or(Value::NULL);
        let h = grob::assq_ref(heap, horizontal, segment).unwrap_or(Value::NULL);
        segments.push(BeamSegment {
            vertical_count: crate::stem_engraver::int_or(heap, vc, 0),
            horizontal: grob::number_pair(heap, h).unwrap_or(Interval::new(0.0, 0.0)),
        });
        s = rest;
    }
    Ok(segments)
}

/// `print`: each segment a beam of the slope `quantized-positions` gives
/// over `X-positions`, at its vertical count's translation, a feathered
/// beam's (`grow-direction`) sloped by its count; with `annotation` its
/// demerits beside it.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    // TODO - mild code dup for all the commonx calls. Some use just
    // common_refpoint_of_array, some (in print and calc_beam_segments) use
    // this plus calls to get_bound.
    //
    // Figure out if there is any particular reason for this and
    // consolidate in one Beam::get_common function.
    let stems = grob_array::extract_grob_array(host, me, "stems")?;
    let commonx = common_x(host, me, &stems)?;

    let segments = get_beam_segments(host, me)?;

    if segments.is_empty() {
        return Ok(Value::NULL);
    }

    let blot = grob::layout_dimension(host, me, "blot-diameter")?;

    let posns = grob::property(host, me, "quantized-positions")?;
    let span = drul(host, me, "X-positions", Interval::new(0.0, 0.0))?;
    let mut pos = match grob::number_pair(host.heap(), posns) {
        Some(pos) => pos,
        None => {
            grob::programming_error(host, me, "no beam positions?")?;
            Interval::new(0.0, 0.0)
        }
    };

    let ss = staff_space(host, me)?;
    pos.left *= ss;
    pos.right *= ss;

    let dy = pos.right - pos.left;
    let slope = if dy != 0.0 && span.length() != 0.0 {
        dy / span.length()
    } else {
        0.0
    };

    let beam_thickness = get_beam_thickness(host, me)?;
    let beam_dy = get_beam_translation(host, me)?;

    let feather_dir = direction_property(host, me, "grow-direction")?;

    let placements = drul(host, me, "normalized-endpoints", Interval::new(0.0, 0.0))?;

    let mut the_beam = Stencil::default();
    let extreme = if segments[0].vertical_count == 0 {
        0
    } else {
        segments[segments.len() - 1].vertical_count as isize as usize
    };

    for i in 0..segments.len() {
        let mut local_slope = slope;
        // Makes local slope proportional to the ratio of the length of this
        // beam to the total length.
        if feather_dir != 0 {
            local_slope += f64::from(feather_dir.wrapping_mul(segments[i].vertical_count))
                * beam_dy
                * placements.length()
                / span.length();
        }

        let heap = host.heap();
        let mut b = lookup::beam(
            heap,
            local_slope,
            segments[i].horizontal.length(),
            beam_thickness,
            blot,
        );
        heap.root(b.expr);

        b.translate_axis(heap, segments[i].horizontal.left, X);
        let multiplier = if feather_dir != 0 {
            placements.left
        } else {
            1.0
        };

        let mut weights = Interval::new(1.0 - multiplier, multiplier);

        if feather_dir != LEFT {
            weights = Interval::new(weights.right, weights.left);
        }

        // we need two translations: the normal one and the one of the
        // lowest segment
        let mut translations = [0.0; 2];
        for (j, idx) in [i, extreme].into_iter().enumerate() {
            // An extreme past the segments, where LilyPond reads past them,
            // weighs nothing.
            if let Some(s) = segments.get(idx) {
                translations[j] = fma(
                    beam_dy,
                    f64::from(s.vertical_count),
                    fma(
                        slope,
                        s.horizontal.left - span.center(),
                        (pos.left + pos.right) / 2.0,
                    ),
                );
            }
        }

        let mut weighted_average = fma(
            translations[0],
            weights.left,
            translations[1] * weights.right,
        );

        // Tricky.  The manipulation of the variable `weighted_average'
        // below ensures that beams with a RIGHT grow direction will start
        // from the position of the lowest segment at 0, and this error will
        // decrease and decrease over the course of the beam.  Something
        // with a LEFT grow direction, on the other hand, will always start
        // in the correct place but progressively accrue error at broken
        // places.  This code shifts beams up given where they are in the
        // total span length (controlled by the variable `multiplier').  To
        // better understand what it does, try commenting it out: you'll see
        // that all of the RIGHT growing beams immediately start too low and
        // get better over line breaks, whereas all of the LEFT growing
        // beams start just right and get worse over line breaks.
        let factor =
            Interval::new(multiplier, 1.0 - multiplier).linear_combination(f64::from(feather_dir));

        if segments[0].vertical_count < 0 && feather_dir != 0 {
            let n = (segments.len() - 1) as f64;
            weighted_average = fma(beam_dy * n, factor, weighted_average);
        }

        b.translate_axis(heap, weighted_average, Y);

        the_beam.add_stencil(heap, &b);
        heap.root(the_beam.expr);
    }

    let annotation = grob::property(host, me, "annotation")?;
    if host.heap().str(annotation).is_some() {
        let stems = grob_array::extract_grob_array(host, me, "stems")?;

        // This code prints the demerits for each beam. Perhaps this should
        // be switchable for those who want to twiddle with the parameters.
        let stem_dir = match stems.first() {
            Some(&s) => direction_property(host, s, "direction")?,
            None => UP,
        };

        let score = crate::call_lily(host, "%grob-interpret-markup", &[me, annotation])?;
        let score = host.heap().foreign::<Stencil>(score).copied();

        if let Some(mut score) = score
            && !score.is_empty()
        {
            host.heap().root(score.expr);
            let x = grob::relative_coordinate(host, me, commonx, X)?;
            let heap = host.heap();
            score.translate_axis(heap, x, X);
            the_beam.add_at_edge(heap, Y, stem_dir, &score, 1.0);
            heap.root(the_beam.expr);
        }
    }

    let x = grob::relative_coordinate(host, me, commonx, X)?;
    let heap = host.heap();
    the_beam.translate_axis(heap, -x, X);
    Ok(heap.wrap(the_beam))
}

/// `get_default_dir`: away from the extremes' longer side, else by the
/// stems' count, then their mean and total distance from the middle
/// line, each of its forced or default direction (else its neutral one);
/// else `neutral-direction`.
fn get_default_dir(host: &mut dyn Host, me: Value) -> Result<Direction, Exception> {
    let stems = grob_array::extract_grob_array(host, me, "stems")?;

    let mut extremes = [0.0f64; 2];
    for &s in &stems {
        let positions = stem::head_positions(host, s)?;
        for d in [DOWN, UP] {
            if dir_of(positions.at(d)) == d {
                let df = f64::from(d);
                extremes[ix(d)] = df * max(df * positions.at(d), df * extremes[ix(d)]);
            }
        }
    }

    let mut total = [0i32; 2];
    let mut count = [0i32; 2];

    let mut force_dir = false;
    for &s in &stems {
        let stem_dir_scm = data(host, s, "direction");
        let mut stem_dir = if is_direction(stem_dir_scm) {
            force_dir = true;
            crate::dynamic_align_engraver::direction(stem_dir_scm)
        } else {
            direction_property(host, s, "default-direction")?
        };

        if stem_dir == 0 {
            stem_dir = direction_property(host, s, "neutral-direction")?;
        }

        if stem_dir != 0 {
            count[ix(stem_dir)] += 1;
            let p = stem::head_positions(host, s)?.at(-stem_dir);
            total[ix(stem_dir)] += ((-f64::from(stem_dir) * p) as i32).max(0);
        }
    }

    if !force_dir {
        if libm::fabs(extremes[ix(UP)]) > -extremes[ix(DOWN)] {
            return Ok(DOWN);
        } else if extremes[ix(UP)] < -extremes[ix(DOWN)] {
            return Ok(UP);
        }
    }

    let (up, down) = (ix(UP), ix(DOWN));
    let d = (count[up] - count[down]).signum();
    if d != 0 {
        return Ok(d);
    }
    if count[up] != 0 && count[down] != 0 {
        let d = (total[up] / count[up] - total[down] / count[down]).signum();
        if d != 0 {
            return Ok(d);
        }
    }
    let d = (total[up] - total[down]).signum();
    if d != 0 {
        return Ok(d);
    }
    direction_property(host, me, "neutral-direction")
}

/// A `Drul_array`'s index of `d`: 0 down, 1 up.
fn ix(d: Direction) -> usize {
    usize::from(d > 0)
}

/// `set_stem_directions`: each stem with no direction of its own given
/// `d`.
fn set_stem_directions(host: &mut dyn Host, me: Value, d: Direction) -> Result<(), Exception> {
    for s in grob_array::extract_grob_array(host, me, "stems")? {
        let forcedir = data(host, s, "direction");
        if crate::dynamic_align_engraver::direction(forcedir) == 0 {
            direction::set_grob_direction(host, s, d);
        }
    }
    Ok(())
}

/// `consider_auto_knees`: with `auto-knee-gap`, the normal stems turned
/// apart at the widest gap between their heads (the pure extents, a forced
/// stem's open to its stem) if it is wider than the gap and the beams'
/// height.
///
/// Only try horizontal beams for knees.  No reliable detection of anything
/// else is possible here, since we don't know funky-beaming settings, or
/// X-distances (slopes!)  People that want sloped knee-beams, should set
/// the directions manually.
fn consider_auto_knees(host: &mut dyn Host, me: Value) -> Result<(), Exception> {
    let scm = grob::property(host, me, "auto-knee-gap")?;
    if !host.heap().is_number(scm) {
        return Ok(());
    }

    let stems = grob_array::extract_grob_array(host, me, "normal-stems")?;

    let common =
        grob::common_refpoint_of_array(host.heap(), &stems, Some(me), Y).unwrap_or(Value::NULL);
    let staff_space = staff_space(host, me)?;

    let mut head_extents_array = Vec::new();
    for &stem in &stems {
        let mut head_extents = Interval::EMPTY;
        if stem::head_count(host, stem)? != 0 {
            head_extents = stem::head_positions(host, stem)?;
            head_extents.widen(1.0);
            head_extents.scale(staff_space * 0.5);

            // We could subtract beam Y position, but this routine only sets
            // stem directions, a constant shift does not have an influence.
            head_extents.translate(unpure::pure_relative_y_coordinate(
                host,
                stem,
                common,
                0,
                i32::MAX as usize,
            )?);

            if crate::dynamic_align_engraver::direction(data(host, stem, "direction")) != 0 {
                let stemdir = direction_property(host, stem, "direction")?;
                *head_extents.at_mut(-stemdir) = -f64::from(stemdir) * f64::INFINITY;
            }
        }
        head_extents_array.push(head_extents);
    }

    let mut max_gap = Interval::EMPTY;
    let mut max_gap_len = 0.0;

    let union = crate::axis_group::interval_union(
        head_extents_array
            .iter()
            .map(|i| (i.left, i.right))
            .collect(),
    );
    let allowed_regions = crate::axis_group::complement(&union);
    for &(l, r) in allowed_regions.iter().rev() {
        let gap = Interval::new(l, r);

        // the outer gaps are not knees.
        if gap.left.is_infinite() || gap.right.is_infinite() {
            continue;
        }

        if gap.length() >= max_gap_len {
            max_gap_len = gap.length();
            max_gap = gap;
        }
    }

    let beam_translation = get_beam_translation(host, me)?;
    let beam_thickness = get_beam_thickness(host, me)?;
    let beam_count = get_beam_count(host, me)?;
    let height_of_beams = fma(
        f64::from(beam_count.wrapping_sub(1)),
        beam_translation,
        beam_thickness / 2.0,
    );
    let threshold = grob::to_double(host.heap(), scm)? + height_of_beams;

    if max_gap_len > threshold {
        for (j, &stem) in stems.iter().enumerate() {
            let head_extents = head_extents_array[j];

            let d = if head_extents.center() < max_gap.center() {
                UP
            } else {
                DOWN
            };

            let heap = host.heap();
            let d = heap.integer(i64::from(d));
            grob::set_property(heap, stem, "direction", d);
        }
    }
    Ok(())
}

/// `calc_stem_shorten`: the `beamed-stem-shorten` of its beam count in
/// staff spaces, by the fraction of its normal stems forced against their
/// default direction; 0 for a knee (shortening looks silly for x staff
/// beams) or no list.
pub fn calc_stem_shorten(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    if grob::property(host, me, "knee")? == Value::TRUE {
        return Ok(host.heap().integer(0));
    }

    let forced_fraction = forced_stem_count(host, me)? as f64 / normal_stem_count(host, me)? as f64;

    let beam_count = get_beam_count(host, me)?;

    let shorten_list = grob::property(host, me, "beamed-stem-shorten")?;
    if shorten_list == Value::NULL {
        return Ok(host.heap().integer(0));
    }

    let staff_space = staff_space(host, me)?;

    let heap = host.heap();
    let mut l = shorten_list;
    let mut i = beam_count.wrapping_sub(1);
    while i > 0 {
        i -= 1;
        match heap.cdr(l) {
            Some(rest) if heap.car(rest).is_some() => l = rest,
            _ => break,
        }
    }
    let Some(shorten_elt) = heap.car(l) else {
        return Err(crate::wrong_type(heap, Some("car"), 1, l, "pair"));
    };
    let mut shorten = grob::to_double(heap, shorten_elt)? * staff_space;

    shorten *= forced_fraction;

    if shorten != 0.0 {
        return Ok(host.heap().flonum(shorten));
    }

    Ok(host.heap().flonum(0.0))
}

/// `forced_stem_count`: the normal stems off their chord's middle forced
/// against their default direction.
fn forced_stem_count(host: &mut dyn Host, me: Value) -> Result<usize, Exception> {
    let mut f = 0;
    for s in grob_array::extract_grob_array(host, me, "normal-stems")? {
        // I can imagine counting those boundaries as a half forced stem,
        // but let's count them full for now.
        let defdir = direction_property(host, s, "default-direction")?;

        if libm::fabs(stem::chord_start_y(host, s)?) > 0.1
            && defdir != 0
            && direction::get_grob_direction(host, s)? != defdir
        {
            f += 1;
        }
    }
    Ok(f)
}

/// `is_cross_staff`: whether a stem is on another staff than the beam.
pub fn is_cross_staff(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    let stems = grob_array::extract_grob_array(host, me, "stems")?;
    let staff_symbol = crate::staff_symbol_referencer::staff_symbol(host, me)?;
    for s in stems {
        if crate::staff_symbol_referencer::staff_symbol(host, s)? != staff_symbol {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `calc_cross_staff`.
pub fn calc_cross_staff(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    Ok(Value::bool(is_cross_staff(host, me)?))
}

/// `get_direction_beam_count`: the most beams of its stems of the
/// direction `d`.
pub fn get_direction_beam_count(
    host: &mut dyn Host,
    me: Value,
    d: Direction,
) -> Result<i32, Exception> {
    let stems = grob_array::extract_grob_array(host, me, "stems")?;
    let mut bc = 0;

    for &s in stems.iter().rev() {
        // Should we take invisible stems into account?
        if direction::get_grob_direction(host, s)? == d {
            bc = bc.max(stem::beam_multiplicity(host, s)?.length().wrapping_add(1));
        }
    }

    Ok(bc)
}

// --- the stems' lengths (MS4 step 4) -----------------------------------------

/// `Beam_stem_end`: a stem's end at the beam, relative to the stem, and its
/// French beaming's shortening.
#[derive(Clone, Copy, Debug, Default)]
pub struct StemEnd {
    pub stem_y: f64,
    pub french_beaming_stem_adjustment: f64,
}

/// `calc_stem_y`: the stem `stem`'s end on the beam from `xl` to `xr` at
/// the positions `pos`, past its beams (a feathered beam's by its place
/// along it), relative to the stem; and for French beaming, `french_count`
/// beams' shortening.
#[allow(clippy::too_many_arguments)]
pub fn calc_stem_y(
    host: &mut dyn Host,
    me: Value,
    stem: Value,
    common: [Value; 2],
    xl: f64,
    xr: f64,
    feather_dir: Direction,
    pos: Interval,
    french_count: i32,
) -> Result<StemEnd, Exception> {
    let beam_translation = get_beam_translation(host, me)?;
    let stem_dir = direction::get_grob_direction(host, stem)?;

    let dx = xr - xl;
    let relx = if dx != 0.0 {
        (grob::relative_coordinate(host, stem, common[X], X)? - xl) / dx
    } else {
        0.0
    };
    let xdir = fma(relx, 2.0, -1.0);

    let mut stem_y = pos.linear_combination(xdir);

    let mut beam_slice = stem::beam_multiplicity(host, stem)?;
    if beam_slice.is_empty() {
        beam_slice = Slice { left: 0, right: 0 };
    }
    let beam_multiplicity = Interval::new(f64::from(beam_slice.left), f64::from(beam_slice.right));

    // feather dir = 1 , relx 0->1 : factor 0 -> 1
    // feather dir = 0 , relx 0->1 : factor 1 -> 1
    // feather dir = -1, relx 0->1 : factor 1 -> 0
    let feather_factor = if feather_dir > 0 {
        relx
    } else if feather_dir < 0 {
        1.0 - relx
    } else {
        1.0
    };

    stem_y = fma(
        feather_factor * beam_translation,
        beam_multiplicity.at(stem_dir),
        stem_y,
    );
    let id = grob::relative_coordinate(host, me, common[Y], Y)?
        - grob::relative_coordinate(host, stem, common[Y], Y)?;

    Ok(StemEnd {
        stem_y: stem_y + id,
        // Set French Beaming stem end shortening value for stems to be
        // shortened
        french_beaming_stem_adjustment: f64::from(french_count) * beam_translation * feather_factor,
    })
}

/// `set_stem_lengths`: each stem's length and begin set to reach the
/// beam's quanted `positions` (a tremolo beam's to its end), French
/// beaming's inner stems shortened; the positions. Its direction and
/// beaming are read first.
///
/// Hmm.  At this time, beam position and slope are determined.  Maybe,
/// stem directions and length should set to relative to the chord's
/// position of the beam.
pub fn set_stem_lengths(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    // trigger callbacks.
    grob::property(host, me, "direction")?;
    grob::property(host, me, "beaming")?;

    let posns = grob::property(host, me, "positions")?;

    let stems = grob_array::extract_grob_array(host, me, "stems")?;
    if stems.is_empty() {
        return Ok(posns);
    }

    let mut common = [Value::NULL; 2];
    for a in [X, Y] {
        common[a] =
            grob::common_refpoint_of_array(host.heap(), &stems, Some(me), a).unwrap_or(Value::NULL);
    }

    let staff_space = staff_space(host, me)?;
    let mut pos = grob::number_pair(host.heap(), posns).unwrap_or(Interval::new(0.0, 0.0));
    pos.left *= staff_space;
    pos.right *= staff_space;

    let mut gap = false;
    let mut thick = 0.0;
    let v = grob::property(host, me, "gap-count")?;
    if crate::stem_engraver::int_or(host.heap(), v, 0) != 0 {
        gap = true;
        thick = get_beam_thickness(host, me)?;
    }

    let x_span = drul(host, me, "X-positions", Interval::new(0.0, 0.0))?;
    let feather_dir = direction_property(host, me, "grow-direction")?;

    for s in stems {
        let french = grob::property(host, s, "french-beaming")? == Value::TRUE;
        let mut french_count = 0;
        if french {
            // french_count is the number of beams a particular stem length
            // must be shortened in French Beaming.  Determined by
            // intersecting left/right beaming information Slices.
            let beaming = grob::property(host, s, "beaming")?;
            let heap = host.heap();
            let (Some(l), Some(r)) = (heap.car(beaming), heap.cdr(beaming)) else {
                return Err(crate::wrong_type(heap, Some("car"), 1, beaming, "pair"));
            };
            let mut le = int_list_to_slice(host, l)?;
            let ri = int_list_to_slice(host, r)?;
            le.intersect(ri);
            french_count = le.length();
        }
        let stem_end = calc_stem_y(
            host,
            me,
            s,
            common,
            x_span.left,
            x_span.right,
            feather_dir,
            pos,
            french_count,
        )?;
        let mut stem_y = stem_end.stem_y;
        let fb_stem_adjustment = stem_end.french_beaming_stem_adjustment;
        // Make the stems go up to the end of the beam. This doesn't matter
        // for normal beams, but for tremolo beams it looks silly otherwise.
        if gap && !stem::is_invisible(host, s)? {
            let d = direction::get_grob_direction(host, s)?;
            stem_y = fma(thick * 0.5, f64::from(d), stem_y);
        }

        // Do set_stem_positions for invisible stems too, so tuplet brackets
        // have a reference point for sloping
        stem::set_stem_positions(
            host,
            s,
            2.0 * stem_y / staff_space,
            2.0 * fb_stem_adjustment / staff_space,
        )?;
    }

    Ok(posns)
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:beam::` primitives (D127): the
/// callback its second argument names, of the grob given, and the rest
/// offsets, of the grob and the offset given, with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 4] = [
    HostHandler {
        what: "beam",
        f: host_beam,
    },
    HostHandler {
        what: "beam-quanting",
        f: host_quanting,
    },
    HostHandler {
        what: "beam-rest-collision",
        f: host_rest_collision,
    },
    HostHandler {
        what: "beam-pure-rest-collision",
        f: host_pure_rest_collision,
    },
];

/// The `ly:beam::` callbacks [`HOSTS`] runs, by name, which the callback
/// table (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 10] = [
    ("calc-beam-segments", calc_beam_segments),
    ("calc-beaming", calc_beaming),
    ("calc-cross-staff", calc_cross_staff),
    ("calc-direction", calc_direction),
    ("calc-normal-stems", calc_normal_stems),
    ("calc-stem-shorten", calc_stem_shorten),
    ("calc-x-positions", calc_x_positions),
    ("print", print),
    ("set-stem-lengths", set_stem_lengths),
    ("tremolo-springs-and-rods", tremolo_springs_and_rods),
];

fn host_beam(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such beam callback")
}

fn host_quanting(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::beam_quanting::quanting(vm, args[1], args[2], args[3])
}

fn host_rest_collision(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    rest_collision_callback(vm, args[1], args[2])
}

fn host_pure_rest_collision(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    pure_rest_collision_callback(vm, args[1], args[2])
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// A `ly:beam::` subr: its grob checked under the callback's name, as
/// LilyPond's `LY_ASSERT_SMOB` gives it, and `%host-call` tail-called with
/// the callback's name ([`HOSTS`]).
fn host_call(cx: &mut Cx, name: &str, args: &[Value]) -> R {
    let method = alloc::format!("ly:beam::{name}");
    grob::grob_arg(cx, &method, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("beam");
    let name = cx.heap_mut().intern(name);
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}

macro_rules! callbacks {
    ($($f:ident $name:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            host_call(cx, $name, args)
        })*
    };
}

callbacks! {
    ly_calc_beam_segments "calc-beam-segments",
    ly_calc_beaming "calc-beaming",
    ly_calc_cross_staff "calc-cross-staff",
    ly_calc_direction "calc-direction",
    ly_calc_normal_stems "calc-normal-stems",
    ly_calc_stem_shorten "calc-stem-shorten",
    ly_calc_x_positions "calc-x-positions",
    ly_print "print",
    ly_set_stem_lengths "set-stem-lengths",
    ly_tremolo_springs_and_rods "tremolo-springs-and-rods",
}

/// `(ly:beam::quanting beam ys align-broken-intos)` (`beam.cc:1083–1095`):
/// LilyPond's subr, a primitive here so that it prints as LilyPond's; it
/// checks its grob and tail-calls `%host-call` ([`HOSTS`]), which runs
/// `beam_quanting.rs`' scoring problem.
pub(crate) fn ly_quanting(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:beam::quanting";
    grob::grob_arg(cx, SUBR, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("beam-quanting");
    Ok(cx.tail_call(f, &[what, args[0], args[1], args[2]]))
}

/// `(ly:beam::rest-collision-callback grob [offset])`: LilyPond's subr, a
/// primitive here so that it prints as LilyPond's; it checks its grob and
/// tail-calls `%host-call` ([`HOSTS`]), an `offset` not given being no
/// number.
pub(crate) fn ly_rest_collision_callback(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:beam::rest-collision-callback";
    grob::grob_arg(cx, SUBR, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("beam-rest-collision");
    let offset = args.get(1).copied().unwrap_or(Value::FALSE);
    Ok(cx.tail_call(f, &[what, args[0], offset]))
}

/// `(ly:beam::pure-rest-collision-callback grob start end [offset])`: as
/// [`ly_rest_collision_callback`]; the start and end are LilyPond's to
/// ignore.
pub(crate) fn ly_pure_rest_collision_callback(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:beam::pure-rest-collision-callback";
    grob::grob_arg(cx, SUBR, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("beam-pure-rest-collision");
    let offset = args.get(3).copied().unwrap_or(Value::FALSE);
    Ok(cx.tail_call(f, &[what, args[0], offset]))
}

/// The two rest offsets as an unpure-pure container, which
/// `Beam_engraver` chains on each rest under its beam
/// (`beam-engraver.cc:212–221`).
pub fn rest_collision_container(host: &mut dyn Host) -> Result<Value, Exception> {
    let unpure = crate::lily_procedure(host, "ly:beam::rest-collision-callback")?;
    let pure = crate::lily_procedure(host, "ly:beam::pure-rest-collision-callback")?;
    crate::call_lily(host, "ly:make-unpure-pure-container", &[unpure, pure])
}
