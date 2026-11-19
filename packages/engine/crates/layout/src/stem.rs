//! `stem.cc` (part 3's, MS3): so far what `Stem_engraver` calls of it,
//! ported first (MS3 step 16): `set_beaming` (`stem.cc:65–84`), `add_head`
//! (`:348–357`), `is_normal_stem` (`:370–377`) and `duration_log`
//! (`:599–604`), 44 lines, and `head_count` (`:206–210`), which
//! `is_normal_stem` calls. Then what the note spacing reads of a stem
//! (MS3 step 31): `head_positions` (`:102–112`, 11), `support_head`
//! (`:181–204`, 24), `first_head` (`:214–221`, 8), `extremal_heads_if`
//! and `extremal_heads` (`:233–281`, 49), `is_invisible` (`:359–368`, 10),
//! `thickness` (`:908–913`, 6), `internal_calc_stem_offset_from_head`
//! (`:1050–1088`, 39) and `get_beam` (`:1116–1121`, 6), with what they
//! read of a note head, `Note_head::stem_attachment_coordinate`
//! (`note-head.cc:150–156`, 7, in `note_head.rs` since MS3 step 37).
//!
//! Then the stem's direction, length and begin position (MS3 step 38,
//! the music crate's placeholders until its callbacks move here):
//! `ly:stem::calc-direction` (`:767–788`, 22) and
//! `::calc-default-direction` (`:790–809`, 20), from its heads and the
//! staff's middle; `::calc-length` (`:976–991`, 16) and
//! `::pure-calc-length` (`:965–974`, 10) over
//! `internal_calc_stem_end_position` (`:480–596`, 117);
//! `::calc-stem-begin-position` (`:915–922`, 8) and
//! `::pure-calc-stem-begin-position` (`:924–932`, 9) over
//! `internal_calc_stem_begin_position` (`:934–963`, 30); with `last_head`
//! (`:224–231`, 8) and `get_reference_head` (`:821–827`, 7). The details'
//! lists are read with `ly:assoc-get` (MS1's) and `%robust-list-ref`
//! (`robust_list_ref`, `spacing_spec.rs`). A tremolo flag's length needs
//! MS4's `Stem_tremolo::vertical_length` (and `::get_beam_translation`)
//! and throws `misc-error` by name at the first; a beam is read only by
//! its properties.
//!
//! Then the rest of the stem's callbacks (MS3 step 38):
//! `::calc-positioning-done` (`:606–765`, 160), the heads moved onto the
//! support head's stem and the seconds' heads across it;
//! `::calc-stem-info` (`:1135–1266`, 132); `::offset-callback`
//! (`:1093–1114`, 22); `::width` (`:889–906`, 18); `::height`
//! (`:813–819`, 7) and `::pure-height` (`:379–385`, 7) over
//! `internal_height` (`:846–887`, 42) and `internal_pure_height`
//! (`:387–447`, 61) with `cache_pure_height` (`:449–459`, 11);
//! `::calc-cross-staff` with `is_cross_staff` (`:1278–1290`, 13);
//! `::print` (`:1012–1048`, 37) with `is_valid_stem` (`:993–1010`, 18)
//! and `beam_end_corrective` (`:829–844`, 16). All fourteen are Rust,
//! the music crate's placeholders until then (D67), under primitives
//! that print as LilyPond's subrs and call it over `%host-call` (D127,
//! [`HOSTS`]). The
//! beam's four functions they call (`Beam::get_beam_translation`,
//! `::get_beam_thickness`, `::get_direction_beam_count`,
//! `::is_cross_staff`) are beam.rs' since MS4 step 3, and the tremolo's
//! two (`Stem_tremolo::vertical_length`, `::get_beam_translation`)
//! stem_tremolo.rs' since MS4 step 5. The flag and
//! `get_stem_info` go with their callers (the flag, MS3 step 41; the
//! beam, MS4), `beam_multiplicity` came with the beam engravers (MS4 step
//! 2), `get_beaming` and `chord_start_y` with the beam's callbacks (MS4
//! step 3), `set_stem_positions` with the beam's stem lengths (MS4 step
//! 4); `note_head_positions` (`:300–346`, 47) came with the note
//! collision's (MS3 step 41).
//!
//! LilyPond 2.26.0's clang fuses the stem offset's two sums of products
//! (otool): the linear combination, as `Interval::linear_combination`
//! does, and the thickness correction, `r + (rule_thick × -d) × 0.5`
//! rounded once. Where LilyPond reads through a null or asserts, a
//! direction of 0 takes a head's right edge as the one inside the stem
//! (`drul-array.hh`'s `assert`), and a stem whose lowest or highest head
//! cannot be found (a position rounding to `INT_MAX`, or below `-INT_MAX`)
//! has no head positions. `calc_stem_info` reads the beam's functions
//! even without a beam, where LilyPond reads through a null: each throws
//! an error naming the function there.

use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, Heap, HostHandler, Tag, Value, Vm};

use crate::beam::{Slice, int_list_to_slice};
use crate::callbacks::Callback;
use crate::direction;
use crate::geometry::{Box, Direction, Interval, X, Y, fma, max, min};
use crate::grob;
use crate::grob_array;
use crate::lookup;
use crate::note_head;
use crate::spacing_options::double_or;
use crate::spanner;
use crate::staff_symbol_referencer::staff_space;
use crate::stencil::Stencil;
use crate::unpure;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const DOWN: Direction = -1;
const UP: Direction = 1;

/// `set_beaming`: on the side `d` of `me`'s `beaming`, a pair made and set
/// if it is none, `beam_count` beams (`(n-1 … 1 0)` before what is there)
/// or, for none, `#f`.
pub fn set_beaming(
    host: &mut dyn Host,
    me: Value,
    beam_count: i32,
    d: Direction,
) -> Result<(), Exception> {
    let mut pair = grob::property(host, me, "beaming")?;
    let heap = host.heap();
    if heap.car(pair).is_none() {
        pair = heap.cons(Value::NULL, Value::NULL);
        grob::set_property(heap, me, "beaming", pair);
    }
    let mut lst = if d == LEFT {
        heap.car(pair)
    } else {
        heap.cdr(pair)
    }
    .unwrap_or(Value::NULL);
    if beam_count != 0 {
        for i in 0..beam_count {
            let i = heap.integer(i64::from(i));
            lst = heap.cons(i, lst);
        }
    } else {
        lst = Value::FALSE;
    }
    if d == LEFT {
        heap.set_car(pair, lst);
    } else if d == RIGHT {
        heap.set_cdr(pair, lst);
    }
    Ok(())
}

/// `beam_multiplicity` (`stem.cc:1268–1276`, MS4 step 2): the beams of
/// `me`'s `beaming` on both sides, as a slice; Guile's error for a
/// `beaming` that is no pair.
pub fn beam_multiplicity(host: &mut dyn Host, me: Value) -> Result<Slice, Exception> {
    let beaming = grob::property(host, me, "beaming")?;
    let heap = host.heap();
    let (Some(l), Some(r)) = (heap.car(beaming), heap.cdr(beaming)) else {
        return Err(crate::wrong_type(heap, Some("car"), 1, beaming, "pair"));
    };
    let mut le = int_list_to_slice(host, l)?;
    let ri = int_list_to_slice(host, r)?;
    le.unite(ri);
    Ok(le)
}

/// `get_beaming` (`stem.cc:86–99`, MS4 step 3): how many beams `me`'s
/// `beaming` has on the side `d`, 0 for none or `#f`; Guile's error for a
/// list that is not proper.
pub fn get_beaming(host: &mut dyn Host, me: Value, d: Direction) -> Result<i32, Exception> {
    let pair = grob::property(host, me, "beaming")?;
    let heap = host.heap();
    let (Some(car), Some(cdr)) = (heap.car(pair), heap.cdr(pair)) else {
        return Ok(0);
    };

    let lst = if d == LEFT { car } else { cdr };
    if lst == Value::FALSE {
        return Ok(0);
    }

    // This list represents the vertical positions at which beams start/end
    // at this stem, so the O(n) cost of scm_length is fine.
    let mut n = 0;
    let mut l = lst;
    while let Some(rest) = heap.cdr(l) {
        n += 1;
        l = rest;
    }
    if l != Value::NULL {
        return Err(crate::wrong_type(heap, Some("length"), 1, lst, "list"));
    }
    Ok(n)
}

/// `chord_start_y` (`stem.cc:114–121`, MS4 step 3): the position of the
/// last head, in staff spaces from the middle line, 0 without heads.
pub fn chord_start_y(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    if head_count(host, me)? != 0 {
        let position = match last_head(host, me)? {
            Some(h) => crate::staff_symbol_referencer::get_position(host, h, false)?,
            None => 0.0,
        };
        return Ok(position * staff_space(host, me)? * 0.5);
    }

    Ok(0.0)
}

/// `set_stem_positions` (`stem.cc:124–175`, MS4 step 4): the stem's
/// `stem-begin-position` and `length` set to end at `se` (in half staff
/// spaces, from the beam's quanting), with LilyPond's warning for an end
/// short of the heads; a stem of no heads a stemlet's length from its end
/// under a beam, or none; and `fc`, French beaming's shortening, as
/// `french-beaming-stem-adjustment` unless 0.
pub fn set_stem_positions(
    host: &mut dyn Host,
    me: Value,
    se: f64,
    fc: f64,
) -> Result<(), Exception> {
    // todo: margins
    let d = direction::get_grob_direction(host, me)?;

    let beam = get_beam(host, me)?;
    if d != 0 && is_normal_stem(host, me)? {
        let dd = direction::get_grob_direction(host, me)?;
        if f64::from(d) * head_positions(host, me)?.at(dd) >= se * f64::from(d) {
            grob::warning(host, me, "weird stem size, check for narrow beams")?;
        }
    }

    // trigger note collision mechanisms
    let stem_beg = internal_calc_stem_begin_position(host, me, false)?;
    let staff_space = staff_space(host, me)?;
    let half_space = staff_space * 0.5;

    let mut height = Interval::EMPTY;
    *height.at_mut(-d) = stem_beg * half_space;
    *height.at_mut(d) = fma(se, half_space, beam_end_corrective(host, me)?);

    let v = grob::property(host, me, "stemlet-length")?;
    let stemlet_length = double_or(host, v, 0.0);
    let stemlet = stemlet_length > 0.0;

    let lh = get_reference_head(host, me)?;

    if lh.is_none() {
        match beam {
            Some(beam) if stemlet => {
                let beam_translation = crate::beam::get_beam_translation(host, beam)?;
                let beam_thickness = crate::beam::get_beam_thickness(host, beam)?;
                let beam_count = beam_multiplicity(host, me)?.length().wrapping_add(1);

                let h = height.at(d);
                *height.at_mut(-d) = fma(
                    -f64::from(d),
                    fma(
                        0.5,
                        beam_thickness,
                        beam_translation * f64::from(0.max(beam_count.wrapping_sub(1))),
                    ) + stemlet_length,
                    h,
                );
            }
            Some(_) => *height.at_mut(-d) = height.at(d),
            None if stemlet => {
                grob::programming_error(host, me, "Can't have a stemlet without a beam.")?;
            }
            None => {}
        }
    }

    let heap = host.heap();
    let v = heap.flonum(height.at(-d) * 2.0 / staff_space);
    grob::set_property(heap, me, "stem-begin-position", v);
    let v = heap.flonum(height.length() * 2.0 / staff_space);
    grob::set_property(heap, me, "length", v);

    if fc != 0.0 {
        let v = heap.flonum(fc);
        grob::set_property(heap, me, "french-beaming-stem-adjustment", v);
    }
    Ok(())
}

/// `head_count`: how many grobs `me`'s `note-heads` holds.
pub fn head_count(host: &mut dyn Host, me: Value) -> Result<usize, Exception> {
    let sym = host.heap().intern("note-heads");
    grob_array::count(host, me, sym)
}

/// `add_head`: the stem `me` set as `n`'s `stem` object, and `n` added to
/// its `note-heads` if it is a note head, to its `rests` if it is a rest.
pub fn add_head(host: &mut dyn Host, me: Value, n: Value) -> Result<(), Exception> {
    let heap = host.heap();
    let sym = heap.intern("stem");
    grob::set_object(heap, n, sym, me);
    let group = if grob::has_interface(heap, n, "note-head-interface") {
        "note-heads"
    } else if grob::has_interface(heap, n, "rest-interface") {
        "rests"
    } else {
        return Ok(());
    };
    let sym = heap.intern(group);
    grob_array::add_grob(host, me, sym, n)
}

/// `is_normal_stem`: whether `me` has note heads and a `duration-log` of
/// 1 or more, converted as `scm_to_int` converts it.
pub fn is_normal_stem(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    if head_count(host, me)? == 0 {
        return Ok(false);
    }
    let log = grob::property(host, me, "duration-log")?;
    Ok(to_int(host.heap(), log)? >= 1)
}

/// `duration_log`: `me`'s `duration-log`, converted as `scm_to_int`
/// converts it, if it is a number, else 2.
pub fn duration_log(host: &mut dyn Host, me: Value) -> Result<i32, Exception> {
    let s = grob::property(host, me, "duration-log")?;
    let heap = host.heap();
    if heap.is_number(s) {
        to_int(heap, s)
    } else {
        Ok(2)
    }
}

/// `head_positions`: the staff positions of `me`'s lowest and highest
/// heads ([`extremal_heads`]), empty without heads.
pub fn head_positions(host: &mut dyn Host, me: Value) -> Result<Interval, Exception> {
    if head_count(host, me)? == 0 {
        return Ok(Interval::EMPTY);
    }
    let [Some(down), Some(up)] = extremal_heads(host, me)? else {
        return Ok(Interval::EMPTY);
    };
    let down = staff_position(host, down)?;
    let up = staff_position(host, up)?;
    Ok(Interval::new(down, up))
}

/// `support_head`: `me`'s one head; of more, the first with the widest
/// part inside the stem (left of an up-stem, right of a down-stem), from
/// the edge to the stem's attachment; of none, [`first_head`].
pub fn support_head(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let heads = grob_array::extract_grob_array(host, me, "note-heads")?;
    if heads.len() == 1 {
        return Ok(Some(heads[0]));
    }
    let d = direction::get_grob_direction(host, me)?;
    let mut widest: Option<(Value, f64)> = None;
    for &head in &heads {
        let x_ext = grob::extent(host, head, head, X)?;
        let attach = internal_calc_stem_offset_from_head(host, me, head, false)?;
        let inside = (x_ext.at(-d) - attach).abs();
        // `std::max_element`: a later head only if wider.
        if widest.is_none_or(|(_, w)| w < inside) {
            widest = Some((head, inside));
        }
    }
    match widest {
        Some((head, _)) => Ok(Some(head)),
        None => first_head(host, me),
    }
}

/// `first_head`: the head at the stem's end, the lowest of an up-stem
/// and the highest of a down-stem; none for a direction of 0.
pub fn first_head(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let d = direction::get_grob_direction(host, me)?;
    if d == 0 {
        return Ok(None);
    }
    let [down, up] = extremal_heads(host, me)?;
    Ok(if d > 0 { down } else { up })
}

/// `last_head`: the head opposite the first, the highest of an up-stem
/// and the lowest of a down-stem; none for a direction of 0.
pub fn last_head(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let d = direction::get_grob_direction(host, me)?;
    if d == 0 {
        return Ok(None);
    }
    let [down, up] = extremal_heads(host, me)?;
    Ok(if d > 0 { up } else { down })
}

/// `extremal_heads`: of `me`'s `note-heads` by rounded staff position,
/// the lowest (the first of a unison) and the highest (the last of one);
/// none without heads.
pub fn extremal_heads(host: &mut dyn Host, me: Value) -> Result<[Option<Value>; 2], Exception> {
    let mut extpos = [i32::MAX, -i32::MAX];
    let mut exthead = [None, None];
    for n in grob_array::extract_grob_array(host, me, "note-heads")? {
        // `int (rint (p))`: frintx then fcvtzs, saturating, NaN to 0.
        let p = libm::rint(staff_position(host, n)?) as i32;
        if p < extpos[0] {
            exthead[0] = Some(n);
            extpos[0] = p;
        }
        if p >= extpos[1] {
            exthead[1] = Some(n);
            extpos[1] = p;
        }
    }
    Ok(exthead)
}

/// `note_head_positions`: the rounded staff positions of `me`'s heads,
/// ascending; with `filter`, of its main column only, the heads at X 0
/// from the common refpoint of the heads and the stem, an invisible
/// stem's harmonic heads up to 0.375 right of it.
pub fn note_head_positions(
    host: &mut dyn Host,
    me: Value,
    filter: bool,
) -> Result<Vec<i32>, Exception> {
    let heads = grob_array::extract_grob_array(host, me, "note-heads")?;
    let xref = grob::common_refpoint_of_array(host.heap(), &heads, Some(me), X);
    let filter_stemless = filter && is_invisible(host, me)?;
    let mut ps = Vec::with_capacity(heads.len());
    for &n in heads.iter().rev() {
        if filter {
            let filter_centered = filter_stemless && {
                let harmonic = host.heap().intern("harmonic");
                grob::property(host, n, "style")? == harmonic
            };
            let tolerated = if filter_centered {
                Interval::new(0.0, 0.375)
            } else {
                Interval::new(0.0, 0.0)
            };
            let x = grob::relative_coordinate(host, n, xref.unwrap_or(Value::NULL), X)?;
            if !(tolerated.left <= x && x <= tolerated.right) {
                continue;
            }
        }
        ps.push(rounded_position(host, n)?);
    }
    ps.sort_unstable();
    Ok(ps)
}

/// `Staff_symbol_referencer::get_rounded_position`: `int (rint (p))` of
/// the position, not pure.
pub(crate) fn rounded_position(host: &mut dyn Host, g: Value) -> Result<i32, Exception> {
    Ok(libm::rint(staff_position(host, g)?) as i32)
}

/// `Staff_symbol_referencer::get_position`: `%staff-position`, not pure.
fn staff_position(host: &mut dyn Host, g: Value) -> Result<f64, Exception> {
    let p = crate::call_lily(host, "%staff-position", &[g, Value::FALSE])?;
    Ok(host.heap().to_f64(p).unwrap_or(0.0))
}

/// `is_invisible`: a normal stem is visible, one with heads is not, and
/// one without is unless its `stemlet-length` (0 if no number) is not 0.
pub fn is_invisible(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    if is_normal_stem(host, me)? {
        return Ok(false);
    }
    if head_count(host, me)? != 0 {
        return Ok(true);
    }
    let v = grob::property(host, me, "stemlet-length")?;
    Ok(double_or(host, v, 0.0) == 0.0)
}

/// `thickness`: `me`'s `thickness` (Guile's type error for no real) in
/// staff line thicknesses.
pub fn thickness(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    let v = grob::property(host, me, "thickness")?;
    let t = grob::to_double(host.heap(), v)?;
    Ok(t * crate::staff_symbol_referencer::line_thickness(host, me)?)
}

/// `internal_calc_stem_offset_from_head`: where on X, relative to the
/// head, the stem attaches to it: at its `stem-attachment` (0 for an
/// invisible stem if `center_invisible`), less half the stem's thickness
/// towards the head unless the attachment is within 10⁻³ of the middle or
/// the head of a style that does not (`neomensural` and the `petrucci`
/// ones); a `mensural` head's is a quarter of it the other way. 0 where
/// the head's extent makes it NaN.
pub fn internal_calc_stem_offset_from_head(
    host: &mut dyn Host,
    me: Value,
    head: Value,
    center_invisible: bool,
) -> Result<f64, Exception> {
    let head_wid = grob::extent(host, head, head, X)?;
    let attach = if center_invisible && is_invisible(host, me)? {
        0.0
    } else {
        note_head::stem_attachment_coordinate(host, head, X)?
    };
    let real_attach = head_wid.linear_combination(attach);
    let mut r = if real_attach.is_nan() {
        0.0
    } else {
        real_attach
    };

    const EPSILON: f64 = 1e-3;
    let style = grob::property(host, head, "style")?;
    let heap = host.heap();
    let style = match style.tag() {
        Tag::Symbol => heap.name(style).unwrap_or("default"),
        _ => "default",
    };
    let mensural = style == "mensural";
    if attach.abs() > EPSILON
        && !matches!(
            style,
            "neomensural" | "petrucci" | "blackpetrucci" | "semipetrucci"
        )
    {
        let d = direction::get_grob_direction(host, me)?;
        let mut rule_thick = thickness(host, me)?;
        if mensural {
            rule_thick /= -2.0;
        }
        r = fma(rule_thick * f64::from(-d), 0.5, r);
    }
    Ok(r)
}

/// `get_beam`: `me`'s `beam`, if it is a spanner.
pub fn get_beam(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern("beam");
    let b = grob_array::object(host, me, sym)?;
    Ok(spanner::is_spanner(host.heap(), b).then_some(b))
}

/// `calc_direction`: with a beam, `me`'s direction once the beam's is
/// read (which sets its stems'); else its `default-direction`, or for 0
/// its `neutral-direction`, as it is.
pub fn calc_direction(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let sym = host.heap().intern("beam");
    let beam = grob_array::object(host, me, sym)?;
    let dir = if grob_of(host.heap(), beam).is_some() {
        grob::property(host, beam, "direction")?;
        direction::get_grob_direction(host, me)?
    } else {
        let dd = grob::property(host, me, "default-direction")?;
        let dir = crate::dynamic_align_engraver::direction(dd);
        if dir == 0 {
            return grob::property(host, me, "neutral-direction");
        }
        dir
    };
    Ok(host.heap().integer(i64::from(dir)))
}

/// `calc_default_direction`: away from the head further from the staff's
/// middle (position 0), each distance truncated to an `int` (saturating,
/// as LilyPond 2.26.0's `fcvtzs`); 0 for heads as far, or none.
pub fn calc_default_direction(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let mut dir = 0;
    if head_count(host, me)? != 0 {
        let staff_center = 0;
        let hp = head_positions(host, me)?;
        let udistance = (f64::from(UP) * hp.at(UP) - f64::from(staff_center)) as i32;
        let ddistance = (f64::from(DOWN) * hp.at(DOWN) - f64::from(staff_center)) as i32;
        dir = ddistance.wrapping_sub(udistance).signum();
    }
    Ok(host.heap().integer(i64::from(dir)))
}

/// `get_reference_head`: with `avoid-note-head` `#t`, the [`last_head`];
/// else the [`first_head`].
pub fn get_reference_head(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    if grob::property(host, me, "avoid-note-head")? == Value::TRUE {
        last_head(host, me)
    } else {
        first_head(host, me)
    }
}

/// `calc_stem_begin_position`: [`internal_calc_stem_begin_position`]
/// with its beam.
pub fn calc_stem_begin_position(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let pos = internal_calc_stem_begin_position(host, me, true)?;
    Ok(host.heap().flonum(pos))
}

/// `pure_calc_stem_begin_position`: [`internal_calc_stem_begin_position`]
/// as if it had no beam, of any columns.
pub fn pure_calc_stem_begin_position(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let pos = internal_calc_stem_begin_position(host, me, false)?;
    Ok(host.heap().flonum(pos))
}

/// `internal_calc_stem_begin_position`: where the stem begins, in half
/// staff spaces: with a beam (`calc_beam`), its `stem-begin-position`
/// once the beam's `quantized-positions` are read; else the staff
/// position of its [`get_reference_head`] (0 for none), moved to where
/// the [`first_head`]'s `stem-attachment` is on its Y extent, unless
/// that is not finite (an empty head).
pub fn internal_calc_stem_begin_position(
    host: &mut dyn Host,
    me: Value,
    calc_beam: bool,
) -> Result<f64, Exception> {
    let beam = get_beam(host, me)?;
    let ss = staff_space(host, me)?;
    if let Some(beam) = beam
        && calc_beam
    {
        grob::property(host, beam, "quantized-positions")?;
        let v = grob::property(host, me, "stem-begin-position")?;
        return Ok(double_or(host, v, 0.0));
    }
    let Some(lh) = get_reference_head(host, me)? else {
        return Ok(0.0);
    };
    let mut pos = staff_position(host, lh)?;
    if let Some(head) = first_head(host, me)? {
        let head_height = grob::extent(host, head, head, Y)?;
        let y_attach = note_head::stem_attachment_coordinate(host, head, Y)?;
        let y_attach = head_height.linear_combination(y_attach);
        if y_attach.is_finite() {
            pos += y_attach * 2.0 / ss;
        }
    }
    Ok(pos)
}

/// `pure_calc_length`: the distance from the pure `stem-begin-position`
/// (0 if no number) to [`internal_calc_stem_end_position`] as if the stem
/// had no beam, of any columns.
pub fn pure_calc_length(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let sym = host.heap().intern("stem-begin-position");
    let v = unpure::get_pure_property(host, me, sym, 0, i32::MAX as usize)?;
    let beg = double_or(host, v, 0.0);
    let res = (internal_calc_stem_end_position(host, me, false)? - beg).abs();
    Ok(host.heap().flonum(res))
}

/// `calc_length`: for a beamed stem, LilyPond's programming error and 0;
/// else the distance from its `stem-begin-position` (0 if no number) to
/// [`internal_calc_stem_end_position`].
pub fn calc_length(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let sym = host.heap().intern("beam");
    let beam = grob_array::object(host, me, sym)?;
    if grob_of(host.heap(), beam).is_some() {
        grob::programming_error(
            host,
            me,
            "ly:stem::calc-length called but will not be used for beamed stem.",
        )?;
        return Ok(host.heap().flonum(0.0));
    }
    let v = grob::property(host, me, "stem-begin-position")?;
    let beg = double_or(host, v, 0.0);
    let res = (internal_calc_stem_end_position(host, me, true)? - beg).abs();
    Ok(host.heap().flonum(res))
}

/// `internal_calc_stem_end_position`: where the stem ends, in half staff
/// spaces; 0 without heads. With a beam (`calc_beam`), its `length` past
/// its `stem-begin-position` (each 0 if no number) once the beam's
/// `quantized-positions` are read. Else its end head's position and its
/// length that way: twice the `details`' `lengths` of its duration log
/// (`(up . down)` for a pair; 7 for none), less a shortening for a stem
/// pointing away from the staff's middle by the `stem-shorten` of its
/// duration log, times its `length-fraction` (1 if no number); not short
/// of the staff's middle unless `no-stem-extend` is `#t`. The `lengths`
/// and the end head's length are `from_scm<double>`: Guile's type error
/// for what is no real, and `robust_list_ref`'s `car` error for a stem
/// shortened without `lengths`. A tremolo flag (unless the stem's beam
/// is taken) needs MS4's `Stem_tremolo::vertical_length` and throws by
/// name ([`stem_tremolo_vertical_length`]).
pub fn internal_calc_stem_end_position(
    host: &mut dyn Host,
    me: Value,
    calc_beam: bool,
) -> Result<f64, Exception> {
    if head_count(host, me)? == 0 {
        return Ok(0.0);
    }
    let beam = get_beam(host, me)?;
    let ss = staff_space(host, me)?;
    let dir = direction::get_grob_direction(host, me)?;
    if let Some(beam) = beam
        && calc_beam
    {
        grob::property(host, beam, "quantized-positions")?;
        let v = grob::property(host, me, "length")?;
        let length = double_or(host, v, 0.0);
        let v = grob::property(host, me, "stem-begin-position")?;
        let begin = double_or(host, v, 0.0);
        return Ok(length + f64::from(dir) * begin);
    }

    // In half spaces.
    let details = grob::property(host, me, "details")?;
    let durlog = duration_log(host, me)?;
    let staff_rad = crate::staff_symbol_referencer::staff_radius(host, me)?;
    let mut length = 7.0;
    let s = assoc_get(host, "lengths", details)?;
    if s.tag() == Tag::Pair {
        let elem = robust_list_ref(host, durlog - 2, s)?;
        let heap = host.heap();
        let len = match (heap.car(elem), heap.cdr(elem)) {
            (Some(up), Some(down)) => {
                if dir == DOWN {
                    down
                } else {
                    up
                }
            }
            _ => elem,
        };
        length = 2.0 * grob::to_double(heap, len)?;
    }

    // Stems in the unnatural (forced) direction are shortened, after
    // [Roush & Gourlay].
    let hp = head_positions(host, me)?;
    if dir != 0 && f64::from(dir) * hp.at(dir) >= 0.0 {
        let sshorten = assoc_get(host, "stem-shorten", details)?;
        let scm_shorten = if sshorten.tag() == Tag::Pair {
            robust_list_ref(host, (durlog - 2).max(0), sshorten)?
        } else {
            Value::NULL
        };
        let shorten_property = 2.0 * double_or(host, scm_shorten, 0.0);
        let first = robust_list_ref(host, 0, s)?;
        let quarter_stem_length = 2.0 * grob::to_double(host.heap(), first)?;
        // The shortening from one stem length to the next in the
        // transition, from a quarter to half a space.
        let shortening_step = min(max(0.25, shorten_property / 6.0), 0.5);
        // From the first stem out of the staff by more than a space, a
        // flagged one not before the middle line.
        let which_step = min(1.0, quarter_stem_length - (2.0 * staff_rad) - 2.0) + hp.at(dir).abs();
        let shorten = min(max(0.0, shortening_step * which_step), shorten_property);
        length -= shorten;
    }

    let v = grob::property(host, me, "length-fraction")?;
    length *= double_or(host, v, 1.0);

    let sym = host.heap().intern("tremolo-flag");
    let t_flag = grob_array::object(host, me, sym)?;
    if grob_of(host.heap(), t_flag).is_some() {
        let sym = host.heap().intern("beam");
        let b = grob_array::object(host, me, sym)?;
        if grob_of(host.heap(), b).is_none() || !calc_beam {
            // Crude hack: add extra space if tremolo flag is there.
            let mut minlen = 1.0 + 2.0 * stem_tremolo_vertical_length(host, t_flag)? / ss;
            if durlog >= 3 {
                let beam_trans = stem_tremolo_get_beam_translation(host, t_flag)?;
                minlen += 2.0 * (f64::from(durlog) - 1.5) * beam_trans;
                if dir == UP {
                    minlen += beam_trans;
                }
            }
            length = max(length, minlen + 1.0);
        }
    }

    let mut stem_end = if dir != 0 {
        hp.at(dir) + f64::from(dir) * length
    } else {
        0.0
    };
    let no_extend = grob::property(host, me, "no-stem-extend")? == Value::TRUE;
    if !no_extend && f64::from(dir) * stem_end < 0.0 {
        stem_end = 0.0;
    }
    Ok(stem_end)
}

/// `calc_positioning_done`: `#t`, and for a stem with heads its
/// `positioning-done` set `#t` first. Its heads, sorted by staff position
/// from the stem's end, move on X onto the stem as the support head's
/// does (a harmonic head of an invisible stem onto the support head's
/// centre instead), and of heads closer than `note-collision-threshold`
/// (1 for no `int`) plus 0.1, every other one moves across the stem by its
/// width less part of the stem's thickness: half; 1.1 for a unison
/// first; for an invisible stem 2 (a whole note) or 0 (a breve or
/// longer). LilyPond 2.26.0's clang fuses `ell - thick × overlap`.
pub fn calc_positioning_done(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    if head_count(host, me)? == 0 {
        return Ok(Value::TRUE);
    }
    grob::set_property(host.heap(), me, "positioning-done", Value::TRUE);

    // `position_less`, each position read once (they are cached), in
    // libc++'s `std::sort`.
    let mut keyed = Vec::new();
    for h in grob_array::extract_grob_array(host, me, "note-heads")? {
        keyed.push((staff_position(host, h)?, h));
    }
    lily_engine_skyline::sort(&mut keyed, &|a: &(f64, Value), b: &(f64, Value)| a.0 < b.0);
    let mut heads: Vec<Value> = keyed.into_iter().map(|(_, h)| h).collect();
    let dir = direction::get_strict_grob_direction(host, me)?;
    if dir < 0 {
        heads.reverse();
    }

    let thick = thickness(host, me)?;

    // Align other heads relative to the "support head."
    if heads.len() > 1 {
        let i_am_invisible = is_invisible(host, me)?;
        // Of more than one head there is one.
        let Some(sup) = support_head(host, me)? else {
            return Ok(Value::TRUE);
        };
        let stem_offset = internal_calc_stem_offset_from_head(host, me, sup, false)?;
        let harmonic = host.heap().intern("harmonic");
        for &h in &heads {
            if h == sup {
                continue;
            }
            // In a whole-note chord, center harmonic heads on the normal
            // heads. Otherwise, attach all heads to the stem as defined by
            // the font.
            let amount = if i_am_invisible && grob::property(host, h, "style")? == harmonic {
                let sup_extent = grob::extent(host, sup, sup, X)?;
                let h_extent = grob::extent(host, h, h, X)?;
                sup_extent.center() - h_extent.center()
            } else {
                stem_offset - internal_calc_stem_offset_from_head(host, me, h, false)?
            };
            // Empty heads can produce NaN.
            if !amount.is_nan() {
                translate_x(host, h, amount)?;
            }
        }
    }

    let mut parity = true;
    let mut lastpos = staff_position(host, heads[0])?;
    let v = grob::property(host, me, "note-collision-threshold")?;
    let threshold = int_or(host.heap(), v, 1);
    for (i, &head) in heads.iter().enumerate().skip(1) {
        let p = staff_position(host, head)?;
        let dy = (lastpos - p).abs();
        // dy should always be 0.5, 0.0, 1.0, but provide safety margin
        // for rounding errors.
        if dy < 0.1 + f64::from(threshold) {
            if parity {
                // Don't include the glyph's 'breapth' value.
                let ell = grob::extent(host, head, head, X)?.right;
                let d = direction::get_grob_direction(host, me)?;
                // Reversed heads overlap the stem by half its thickness;
                // the first less if it is a unison (issue 346).
                let mut reverse_overlap = if i == 1 && dy < 0.1 { 1.1 } else { 0.5 };
                if is_invisible(host, me)? {
                    // Semibreves nearer; breves and longer 'exactly', their
                    // vertical lines aligned by the glyphs' 'breapth'.
                    reverse_overlap = if duration_log(host, me)? >= 0 {
                        2.0
                    } else {
                        0.0
                    };
                }
                let shift = fma(-thick, reverse_overlap, ell) * f64::from(d);
                translate_x(host, head, shift)?;
            }
            parity = !parity;
        } else {
            parity = true;
        }
        lastpos = p;
    }
    Ok(Value::TRUE)
}

/// `translate_axis` on X, LilyPond's programming error where it refuses.
fn translate_x(host: &mut dyn Host, g: Value, amount: f64) -> Result<(), Exception> {
    if let Err(text) = grob::translate_axis(host.heap(), g, amount, X) {
        grob::programming_error(host, g, text)?;
    }
    Ok(())
}

/// `from_scm<int> (v, fallback)`: an exact integer of an `int`'s range,
/// else the fallback.
pub(crate) fn int_or(heap: &mut Heap, v: Value, fallback: i32) -> i32 {
    if !heap.is_exact_integer(v) {
        return fallback;
    }
    heap.to_i64(v)
        .and_then(|n| i32::try_from(n).ok())
        .unwrap_or(fallback)
}

/// `calc_stem_info`: the ideal and the shortest Y of the stem's end
/// under its beam, `(ideal shortest)`, from the `details`' beamed lengths
/// of its beam count. It reads the beam's translation, thickness and
/// count, MS4's, which throw by name (with no beam too, where LilyPond
/// reads through a null). A stem of direction 0 takes up, after
/// LilyPond's programming error. The sums of products are fused as clang
/// fuses them (the left product first).
pub fn calc_stem_info(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let mut my_dir = direction::get_grob_direction(host, me)?;
    if my_dir == 0 {
        grob::programming_error(host, me, "no stem dir set")?;
        my_dir = UP;
    }
    let staff_space = staff_space(host, me)?;
    let beam = get_beam(host, me)?;
    if let Some(beam) = beam {
        grob::property(host, beam, "beaming")?;
    }
    let beam_translation = beam_get_beam_translation(host, beam)?;
    let beam_thickness = beam_get_beam_thickness(host, beam)?;
    let beam_count = beam_get_direction_beam_count(host, beam, my_dir)?;
    let v = grob::property(host, me, "length-fraction")?;
    let length_fraction = double_or(host, v, 1.0);

    // Simple standard stem length.
    let details = grob::property(host, me, "details")?;
    let beamed = |host: &mut dyn Host, key: &str| -> Result<Option<f64>, Exception> {
        let lengths = assoc_get(host, key, details)?;
        if lengths.tag() != Tag::Pair {
            return Ok(None);
        }
        let v = robust_list_ref(host, beam_count - 1, lengths)?;
        Ok(Some(grob::to_double(host.heap(), v)? * staff_space))
    };
    // Stem only extends to center of beam.
    let ideal_length = match beamed(host, "beamed-lengths")? {
        Some(l) => fma(l, length_fraction, -(0.5 * beam_thickness)),
        None => 0.0,
    };
    // Condition: sane minimum free stem length (chord to beams).
    let ideal_minimum_free =
        beamed(host, "beamed-minimum-free-lengths")?.map_or(0.0, |l| l * length_fraction);

    let mut height_of_my_trem = 0.0;
    let sym = host.heap().intern("tremolo-flag");
    let trem = grob_array::object(host, me, sym)?;
    if grob_of(host.heap(), trem).is_some() {
        // Hack a bit of space around the trem.
        height_of_my_trem = stem_tremolo_vertical_length(host, trem)? + beam_translation;
    }

    // Also for the ideal minimum length the maximum beam count (for this
    // direction).
    let height_of_my_beams = fma(f64::from(beam_count - 1), beam_translation, beam_thickness);
    let ideal_minimum_length = fma(
        -0.5,
        beam_thickness,
        ideal_minimum_free + height_of_my_beams + height_of_my_trem,
    );
    let ideal_length = max(ideal_length, ideal_minimum_length);

    // Convert to Y position, calculate for dir == UP.
    let note_start = head_positions(host, me)?.at(my_dir) * 0.5 * f64::from(my_dir) * staff_space;
    let mut ideal_y = note_start + ideal_length;

    // Not lower than the middle line nor the lowest beam than the second
    // line; not for grace beams nor knees.
    let no_extend = grob::property(host, me, "no-stem-extend")? == Value::TRUE;
    let is_knee = match beam {
        Some(beam) => grob::property(host, beam, "knee")? == Value::TRUE,
        None => false,
    };
    if !no_extend && !is_knee {
        ideal_y = max(ideal_y, 0.0);
        ideal_y = max(ideal_y, -staff_space - beam_thickness + height_of_my_beams);
    }
    if let Some(beam) = beam {
        let v = grob::property(host, beam, "shorten")?;
        ideal_y -= double_or(host, v, 0.0);
    }

    let minimum_free =
        beamed(host, "beamed-extreme-minimum-free-lengths")?.map_or(0.0, |l| l * length_fraction);
    let minimum_length = fma(
        -0.5,
        beam_thickness,
        max(minimum_free, height_of_my_trem) + height_of_my_beams,
    );

    ideal_y *= f64::from(my_dir);
    let minimum_y = note_start + minimum_length;
    let shortest_y = minimum_y * f64::from(my_dir);
    let heap = host.heap();
    let (ideal, shortest) = (heap.flonum(ideal_y), heap.flonum(shortest_y));
    Ok(heap.list(&[ideal, shortest]))
}

/// `offset_callback`: with rests, the centre of the last one's X extent
/// (its coordinate for an empty one); else where the stem attaches to its
/// support head, an invisible stem at its centre; else 0, after
/// LilyPond's programming error.
pub fn offset_callback(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let rests = grob_array::extract_grob_array(host, me, "rests")?;
    if let Some(&rest) = rests.last() {
        let r = grob::robust_relative_extent(host, rest, rest, X)?.center();
        return Ok(host.heap().flonum(r));
    }
    if let Some(head) = support_head(host, me)? {
        let r = internal_calc_stem_offset_from_head(host, me, head, true)?;
        return Ok(host.heap().flonum(r));
    }
    grob::programming_error(host, me, "Weird stem.")?;
    Ok(host.heap().flonum(0.0))
}

/// `width`: empty for an invisible stem, else its thickness about 0.
pub fn width(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let r = if is_invisible(host, me)? {
        Interval::EMPTY
    } else {
        let mut r = Interval::new(-1.0, 1.0);
        r.scale(thickness(host, me)? / 2.0);
        r
    };
    Ok(crate::offset(host.heap(), (r.left, r.right)))
}

/// `height`: [`internal_height`] with its beam.
pub fn height(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let iv = internal_height(host, me, true)?;
    Ok(crate::offset(host.heap(), (iv.left, iv.right)))
}

/// `pure_height`: [`internal_pure_height`] with its beam, of any columns.
pub fn pure_height(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let iv = internal_pure_height(host, me, true)?;
    Ok(crate::offset(host.heap(), (iv.left, iv.right)))
}

/// `internal_pure_height`: `(0 . 0)` for a stem that is not normal; else
/// its pure [`internal_height`], and with a beam (`calc_beam`) united
/// with the pure heights of the beam's `normal-stems` its way, each
/// moved to the end of the beam by its pure coordinate among theirs, and
/// cut to its own on the side away from the beam; each of those stems'
/// cut kept as its pure height ([`cache_pure_height`]), which moves when
/// the lines do if making it read where an alignment puts one (a beam
/// across staves, OPEN B27). A stem with no common refpoint among them is
/// at 0.
pub fn internal_pure_height(
    host: &mut dyn Host,
    me: Value,
    calc_beam: bool,
) -> Result<Interval, Exception> {
    if !is_normal_stem(host, me)? {
        return Ok(Interval::new(0.0, 0.0));
    }
    let reads = unpure::translations_read();
    let sym = host.heap().intern("beam");
    let beam = grob_array::object(host, me, sym)?;
    let mut iv = internal_height(host, me, false)?;
    if grob_of(host.heap(), beam).is_none() || !calc_beam {
        return Ok(iv);
    }
    let dir = direction::get_grob_direction(host, me)?;
    let mut overshoot = Interval::EMPTY;
    for d in [DOWN, UP] {
        *overshoot.at_mut(d) = if d == dir {
            f64::from(dir) * f64::INFINITY
        } else {
            iv.at(d)
        };
    }
    let mut heights = Vec::new();
    let mut my_stems = Vec::new();
    for stem in grob_array::extract_grob_array(host, beam, "normal-stems")? {
        if direction::get_grob_direction(host, stem)? == dir {
            heights.push(if stem != me {
                internal_pure_height(host, stem, false)?
            } else {
                iv
            });
            my_stems.push(stem);
        }
    }
    // Look for cross staff effects.
    let common = grob::common_refpoint_of_array(host.heap(), &my_stems, Some(me), Y);
    let mut coords = Vec::with_capacity(my_stems.len());
    let (mut min_pos, mut max_pos) = (f64::INFINITY, f64::NEG_INFINITY);
    for &stem in &my_stems {
        let c = match common {
            Some(common) => {
                unpure::pure_relative_y_coordinate(host, stem, common, 0, i32::MAX as usize)?
            }
            None => 0.0,
        };
        min_pos = min(min_pos, c);
        max_pos = max(max_pos, c);
        coords.push(c);
    }
    for (h, &c) in heights.iter_mut().zip(&coords) {
        *h.at_mut(dir) += if dir == DOWN {
            c - max_pos
        } else {
            c - min_pos
        };
    }
    for &h in &heights {
        iv.unite(h);
    }
    let moves = unpure::translations_read() != reads;
    for (&stem, &h) in my_stems.iter().zip(&heights) {
        cache_pure_height(host, stem, iv, h, moves)?;
    }
    iv.intersect(overshoot);
    Ok(iv)
}

/// `cache_pure_height`: `iv` cut to `my_iv` on the side away from the
/// stem's direction, kept as the item `me`'s pure height, which `moves`
/// with the lines if making it read an alignment's translation
/// ([`unpure::steady_height`]).
fn cache_pure_height(
    host: &mut dyn Host,
    me: Value,
    mut iv: Interval,
    my_iv: Interval,
    moves: bool,
) -> Result<(), Exception> {
    let dir = direction::get_grob_direction(host, me)?;
    let mut overshoot = Interval::EMPTY;
    for d in [DOWN, UP] {
        *overshoot.at_mut(d) = if d == dir {
            f64::from(dir) * f64::INFINITY
        } else {
            my_iv.at(d)
        };
    }
    iv.intersect(overshoot);
    grob::with_grob_mut(host.heap(), me, |g| {
        g.pure_height = Some(iv);
        g.pure_height_moves = moves;
    });
    Ok(())
}

/// `internal_height`: empty for a stem that is not valid
/// ([`is_valid_stem`]) and has no beam; with a beam (`calc_beam`), once
/// its `quantized-positions` are read; without, empty if the stem has no
/// stencil. Else from its `stem-begin-position` to its `length` past it
/// that way (each 0 if no number; their pure values for no `calc_beam`),
/// in staff spaces.
pub fn internal_height(
    host: &mut dyn Host,
    me: Value,
    calc_beam: bool,
) -> Result<Interval, Exception> {
    let beam = get_beam(host, me)?;
    if !is_valid_stem(host, me)? && beam.is_none() {
        return Ok(Interval::EMPTY);
    }
    let dir = direction::get_grob_direction(host, me)?;
    if let Some(beam) = beam
        && calc_beam
    {
        // Trigger set-stem-lengths.
        grob::property(host, beam, "quantized-positions")?;
    }
    // If there is a beam but no stem, slope calculations depend on this
    // routine to return where the stem end /would/ be.
    if calc_beam && beam.is_none() {
        let stencil = grob::property(host, me, "stencil")?;
        if host.heap().foreign::<Stencil>(stencil).is_none() {
            return Ok(Interval::EMPTY);
        }
    }
    let begin = unpure::get_maybe_pure_property(
        host,
        me,
        "stem-begin-position",
        !calc_beam,
        0,
        i32::MAX as usize,
    )?;
    let y1 = double_or(host, begin, 0.0);
    let length =
        unpure::get_maybe_pure_property(host, me, "length", !calc_beam, 0, i32::MAX as usize)?;
    let y2 = f64::from(dir) * double_or(host, length, 0.0) + y1;
    let half_space = staff_space(host, me)? * 0.5;
    let mut stem_y = Interval::new(min(y1, y2), max(y2, y1));
    stem_y.scale(half_space);
    Ok(stem_y)
}

/// `is_valid_stem`: a stem with a reference head or a beam that is
/// visible.
pub fn is_valid_stem(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    let lh = get_reference_head(host, me)?;
    let sym = host.heap().intern("beam");
    let beam = grob_array::object(host, me, sym)?;
    if lh.is_none() && grob_of(host.heap(), beam).is_none() {
        return Ok(false);
    }
    Ok(!is_invisible(host, me)?)
}

/// `beam_end_corrective`: with a beam, half its thickness its way (up
/// for a direction of 0, after LilyPond's programming error), which
/// needs MS4's `Beam::get_beam_thickness`; else 0.
pub(crate) fn beam_end_corrective(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    let sym = host.heap().intern("beam");
    let beam = grob_array::object(host, me, sym)?;
    let mut dir = direction::get_grob_direction(host, me)?;
    if grob_of(host.heap(), beam).is_none() {
        return Ok(0.0);
    }
    if dir == 0 {
        grob::programming_error(host, me, "no stem direction")?;
        dir = UP;
    }
    Ok(f64::from(dir) * beam_get_beam_thickness(host, Some(beam))? * 0.5)
}

/// `print`: `'()` for a stem that is not valid ([`is_valid_stem`]); else
/// a box of its thickness from its `stem-begin-position` to its `length`
/// (less its `french-beaming-stem-adjustment`) past it, each 0 if no
/// number, in staff spaces, less the beam's corrective at its end
/// ([`beam_end_corrective`]), its corners rounded by the layout's
/// `blot-diameter`.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    if !is_valid_stem(host, me)? {
        return Ok(Value::NULL);
    }
    let dir = direction::get_grob_direction(host, me)?;
    let v = grob::property(host, me, "stem-begin-position")?;
    let y1 = double_or(host, v, 0.0);
    let v = grob::property(host, me, "length")?;
    let mut stem_length = double_or(host, v, 0.0);
    let v = grob::property(host, me, "french-beaming-stem-adjustment")?;
    let fb_stem_adjustment = double_or(host, v, 0.0);
    let half_space = staff_space(host, me)? * 0.5;

    // Shorten inner French Beams (for printing).
    stem_length -= fb_stem_adjustment;
    let y2 = f64::from(dir) * stem_length + y1;
    let mut stem_y = Interval::new(min(y1, y2), max(y2, y1));
    stem_y.scale(half_space);
    *stem_y.at_mut(dir) -= beam_end_corrective(host, me)?;

    let stem_width = thickness(host, me)?;
    let blot = grob::layout_dimension(host, me, "blot-diameter")?;
    let b = Box::new(Interval::new(-stem_width / 2.0, stem_width / 2.0), stem_y);
    let mut warnings = Vec::new();
    let heap = host.heap();
    let ss = lookup::round_filled_box(heap, b, blot, &mut warnings);
    let mut mol = Stencil::default();
    mol.add_stencil(heap, &ss);
    let mol = heap.wrap(mol);
    // What a warning could free: the stencil.
    let depth = heap.root_depth();
    heap.root(mol);
    let warned = warnings
        .iter()
        .try_for_each(|text| grob::unlocated_warning(host, text));
    host.heap().unroot_to(depth);
    warned.map(|()| mol)
}

/// `calc_cross_staff`: whether the stem's beam is cross-staff
/// (`is_cross_staff`), which needs MS4's `Beam::is_cross_staff`; `#f`
/// without a beam.
pub fn calc_cross_staff(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let sym = host.heap().intern("beam");
    let beam = grob_array::object(host, me, sym)?;
    let cross = grob_of(host.heap(), beam).is_some() && beam_is_cross_staff(host, beam)?;
    Ok(if cross { Value::TRUE } else { Value::FALSE })
}

// --- the beam's and the tremolo's functions stem.cc calls -------------------

/// The beam a function of beam.cc is called on, which LilyPond reads
/// through a null without one: an error naming it.
fn the_beam(host: &mut dyn Host, beam: Option<Value>, name: &str) -> Result<Value, Exception> {
    beam.ok_or_else(|| crate::misc_error(host.heap(), name, "a stem with no beam"))
}

/// `Beam::get_beam_translation` (`beam.rs`).
fn beam_get_beam_translation(host: &mut dyn Host, beam: Option<Value>) -> Result<f64, Exception> {
    let beam = the_beam(host, beam, "Beam::get_beam_translation")?;
    crate::beam::get_beam_translation(host, beam)
}

/// `Beam::get_beam_thickness` (`beam.rs`).
fn beam_get_beam_thickness(host: &mut dyn Host, beam: Option<Value>) -> Result<f64, Exception> {
    let beam = the_beam(host, beam, "Beam::get_beam_thickness")?;
    crate::beam::get_beam_thickness(host, beam)
}

/// `Beam::get_direction_beam_count` (`beam.rs`).
fn beam_get_direction_beam_count(
    host: &mut dyn Host,
    beam: Option<Value>,
    d: Direction,
) -> Result<i32, Exception> {
    let beam = the_beam(host, beam, "Beam::get_direction_beam_count")?;
    crate::beam::get_direction_beam_count(host, beam, d)
}

/// `Beam::is_cross_staff` (`beam.rs`).
fn beam_is_cross_staff(host: &mut dyn Host, beam: Value) -> Result<bool, Exception> {
    crate::beam::is_cross_staff(host, beam)
}

/// `Stem_tremolo::vertical_length` (`stem_tremolo.rs`).
fn stem_tremolo_vertical_length(host: &mut dyn Host, trem: Value) -> Result<f64, Exception> {
    crate::stem_tremolo::vertical_length(host, trem)
}

/// `Stem_tremolo::get_beam_translation` (`stem_tremolo.rs`).
fn stem_tremolo_get_beam_translation(host: &mut dyn Host, trem: Value) -> Result<f64, Exception> {
    crate::stem_tremolo::get_beam_translation(host, trem)
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:stem::` primitives (D127): the stem
/// callback its second argument names, of the grob given, with the VM as
/// its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "stem",
    f: host_stem,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 14] = [
    ("calc-cross-staff", calc_cross_staff),
    ("calc-default-direction", calc_default_direction),
    ("calc-direction", calc_direction),
    ("calc-length", calc_length),
    ("calc-positioning-done", calc_positioning_done),
    ("calc-stem-begin-position", calc_stem_begin_position),
    ("calc-stem-info", calc_stem_info),
    ("height", height),
    ("offset-callback", offset_callback),
    ("print", print),
    ("pure-calc-length", pure_calc_length),
    (
        "pure-calc-stem-begin-position",
        pure_calc_stem_begin_position,
    ),
    ("pure-height", pure_height),
    ("width", width),
];

fn host_stem(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such stem callback")
}

/// `(ly:stem::… grob)`, the pure ones `(… grob start end)`: LilyPond's
/// subrs, primitives here so that they print as LilyPond's
/// (`#<procedure ly:stem::calc-direction (_)>`, which a trace writes of a
/// stem whose direction is not yet read). Each checks its grob and
/// tail-calls `%host-call` with its name ([`HOSTS`]).
fn callback(cx: &mut Cx, subr: &str, what: &str, grob: Value) -> R {
    grob::grob_arg(cx, subr, 1, grob)?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let stem = cx.heap_mut().intern("stem");
    let what = cx.heap_mut().intern(what);
    Ok(cx.tail_call(f, &[stem, what, grob]))
}

type R = Result<Value, lily_engine_scheme::Thrown>;

macro_rules! callbacks {
    ($($f:ident $what:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            callback(cx, concat!("ly:stem::", $what), $what, args[0])
        })*
    };
}

callbacks! {
    ly_calc_cross_staff "calc-cross-staff",
    ly_calc_default_direction "calc-default-direction",
    ly_calc_direction "calc-direction",
    ly_calc_length "calc-length",
    ly_calc_positioning_done "calc-positioning-done",
    ly_calc_stem_begin_position "calc-stem-begin-position",
    ly_calc_stem_info "calc-stem-info",
    ly_height "height",
    ly_offset_callback "offset-callback",
    ly_print "print",
    ly_pure_calc_length "pure-calc-length",
    ly_pure_calc_stem_begin_position "pure-calc-stem-begin-position",
    ly_pure_height "pure-height",
    ly_width "width",
}

/// `ly_assoc_get (key, alist, SCM_EOL)`: `ly:assoc-get`'s.
fn assoc_get(host: &mut dyn Host, key: &str, alist: Value) -> Result<Value, Exception> {
    let key = host.heap().intern(key);
    crate::call_lily(host, "ly:assoc-get", &[key, alist, Value::NULL])
}

/// `robust_list_ref`: `%robust-list-ref`'s.
fn robust_list_ref(host: &mut dyn Host, i: i32, l: Value) -> Result<Value, Exception> {
    let i = host.heap().integer(i64::from(i));
    crate::call_lily(host, "%robust-list-ref", &[i, l])
}

/// `from_scm<int>` (`scm_to_int`): an exact integer of an `int`'s range,
/// else Guile's error, from no procedure.
pub(crate) fn to_int(heap: &mut Heap, v: Value) -> Result<i32, Exception> {
    if !heap.is_exact_integer(v) {
        return Err(crate::wrong_type(heap, None, 0, v, "exact integer"));
    }
    match heap.to_i64(v).and_then(|n| i32::try_from(n).ok()) {
        Some(n) => Ok(n),
        None => {
            let key = heap.intern("out-of-range");
            let message = heap.string("Value out of range ~S to< ~S: ~S");
            let (min, max) = (
                heap.integer(i64::from(i32::MIN)),
                heap.integer(i64::from(i32::MAX)),
            );
            let items = heap.list(&[min, max, v]);
            let rest = heap.list(&[v]);
            let args = heap.list(&[Value::FALSE, message, items, rest]);
            Err(Exception { key, args })
        }
    }
}
