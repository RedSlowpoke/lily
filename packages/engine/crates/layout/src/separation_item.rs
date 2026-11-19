//! `separation-item.cc` (part 3's, MS3): so far `add_item`
//! (`separation-item.cc:33–38`) and `add_conditional_item` (`:40–45`), 12
//! lines, which `Paper_column_engraver` calls, ported first (MS3 step
//! 15); and the separation items' skylines (MS3 step 30),
//! `ly:separation-item::calc-skylines` (`:89–109`, 21) and `boxes`
//! (`:119–184`, 66) of the unconditional elements, the image's Scheme over
//! `crates/skyline`'s boxes (`%boxes-skylines`, `skylines.rs`) until MS3
//! step 59 made them Rust, a callback of the table (`callbacks.rs`) under
//! a primitive over `%host-call` (D127, [`HOSTS`]); and
//! `conditional_skyline` (`:78–88`, 11) with `boxes`' conditional
//! elements (`:132–145`, 14), which
//! `Accidental_placement::get_relevant_accidentals` filters, for
//! `spacing_interface.rs` (MS3 step 30); and, for the spacing spanner's
//! `set_column_rods` (`spacing_spanner.rs`), `set_distance` (`:47–68`, 22)
//! and `is_empty` (`:70–76`, 7), Rust with no Scheme face (MS3 step 31).
//! As `paper_column.rs`' `minimum_distance` reads them, a
//! `horizontal-skylines` that is no skyline pair is an empty pair, and a
//! pair the wrong way round `from_scm`'s error. LilyPond's
//! `lines[LEFT][RIGHT].distance (right)` adds no product: nothing to fuse.

use alloc::vec::Vec;

use lily_engine_fonts::Box;
use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};
use lily_engine_skyline::{Axis as Horizon, Direction, Skyline, SkylinePair};

use crate::callbacks::Callback;
use crate::geometry::{Interval, X, Y, max};
use crate::rod::Rod;
use crate::skylines::{self, skyline_pair_in};
use crate::{R, accidental_placement, grob, grob_array, item, unpure};

/// `add_item`: the item `i` added to `s`'s `elements`.
pub fn add_item(host: &mut dyn Host, s: Value, i: Value) -> Result<(), Exception> {
    let sym = host.heap().intern("elements");
    grob_array::add_grob(host, s, sym, i)
}

/// `add_conditional_item`: `e` added to `me`'s `conditional-elements`.
pub fn add_conditional_item(host: &mut dyn Host, me: Value, e: Value) -> Result<(), Exception> {
    let sym = host.heap().intern("conditional-elements");
    grob_array::add_grob(host, me, sym, e)
}

/// `me`'s `horizontal-skylines` as `from_scm<Skyline_pair>` reads them,
/// none for what is no skyline pair.
pub(crate) fn horizontal_skylines(
    host: &mut dyn Host,
    me: Value,
) -> Result<Option<SkylinePair>, Exception> {
    let v = grob::property(host, me, "horizontal-skylines")?;
    let heap = host.heap();
    skyline_pair_in(heap, v).map_err(|message| crate::misc_error(heap, "from_scm", message))
}

/// `set_distance`: how far `r` must be right of `l` for `l`'s skyline on
/// the right to stay `padding` from `r`'s on the left, merged with `r`'s
/// [`conditional_skyline`] given `l`; above 0, a rod of that distance
/// added between them (`add_to_cols`). At least 0.
pub fn set_distance(
    host: &mut dyn Host,
    l: Value,
    r: Value,
    padding: f64,
) -> Result<f64, Exception> {
    let lines = [horizontal_skylines(host, l)?, horizontal_skylines(host, r)?];
    let mut right = conditional_skyline(host, r, l)?;
    if let Some(r_lines) = &lines[1] {
        right.merge(&r_lines[Direction::Down]);
    }

    let left = lines[0].as_ref().map_or_else(
        || Skyline::empty(Direction::Up),
        |l_lines| l_lines[Direction::Up].clone(),
    );
    let dist = padding + left.distance(&right, 0.0);
    if dist > 0.0 {
        let mut rod = Rod {
            item_drul: [Some(l), Some(r)],
            distance: dist,
        };
        rod.add_to_cols(host)?;
    }

    Ok(max(dist, 0.0))
}

/// `is_empty`: whether both of `me`'s `horizontal-skylines` are empty.
pub fn is_empty(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    Ok(horizontal_skylines(host, me)?.is_none_or(|sky| sky.is_empty()))
}

/// `conditional_skyline`: the skyline bounding on the left the boxes of
/// `me`'s `conditional-elements`, given the item `left` before it: the
/// accidentals `get_relevant_accidentals` keeps of its accidental
/// placements, then the other elements (for now only arpeggios), as
/// [`boxes`] boxes them, along Y.
pub fn conditional_skyline(
    host: &mut dyn Host,
    me: Value,
    left: Value,
) -> Result<Skyline, Exception> {
    let read_only = grob_array::extract_grob_array(host, me, "conditional-elements")?;
    let (mut accidentals, mut others) = (Vec::new(), Vec::new());
    for e in read_only {
        if grob::has_interface(host.heap(), e, "accidental-placement-interface") {
            accidentals.push(e);
        } else {
            others.push(e);
        }
    }
    let mut elts = accidental_placement::get_relevant_accidentals(host, &accidentals, left)?;
    elts.append(&mut others);
    let boxes = boxes(host, me, &elts, &mut None)?;
    Ok(Skyline::from_boxes(&boxes, Horizon::Y, Direction::Down))
}

/// What is kept of a column's boxes (OPEN B21): all of each but where its
/// line is put, so that a column whose grobs read as they did can be made
/// again from it when the lines move, as [`boxes`] makes it. One block a
/// column, as a score has thousands of boxes: for each, in `numbers`, its
/// width, widened, its height relative to itself, what widens that, then
/// the offsets up to its line's group; in `lines`, the line and how many
/// offsets.
#[derive(Default)]
pub(crate) struct KeptBoxes {
    numbers: Vec<f64>,
    lines: Vec<(u64, u32)>,
}

/// A box of [`KeptBoxes`] in no line.
const NO_LINE: u64 = u64::MAX;

impl KeptBoxes {
    /// How many boxes it holds.
    pub(crate) fn len(&self) -> usize {
        self.lines.len()
    }

    /// As little as it can hold them in.
    pub(crate) fn shrink(&mut self) {
        self.numbers.shrink_to_fit();
        self.lines.shrink_to_fit();
    }
}

/// The box of `height`, at the pure coordinate `y`, as [`boxes`] widens it,
/// if it is not empty.
fn placed(x: lily_engine_fonts::Interval, height: Interval, y: f64, h: Interval) -> Option<Box> {
    let y = Interval {
        left: height.left + y,
        right: height.right + y,
    };
    let y = widened(y, h);
    (!(x.left > x.right || y.left > y.right)).then_some(Box { x, y })
}

/// The interval `e` widened by `extra`, on the sides it is not infinite.
fn widened(e: Interval, extra: Interval) -> lily_engine_fonts::Interval {
    lily_engine_fonts::Interval {
        left: if e.left.is_infinite() {
            e.left
        } else {
            e.left + extra.left
        },
        right: if e.right.is_infinite() {
            e.right
        } else {
            e.right + extra.right
        },
    }
}

/// `boxes`: the boxes of the items `elts` in `me`'s column that are no
/// axis groups, the X extent relative to the column, the pure Y extent of
/// the whole line relative to the elements' and `me`'s common Y refpoint,
/// widened by `extra-spacing-width` (`(-0.1 . 0.1)` if no number pair) and
/// `extra-spacing-height` (`(0.0 . 0.0)`) where not infinite, and those
/// empty on neither axis, in their order. What is no item, where
/// LilyPond's `dynamic_cast` gives a null it reads through, throws
/// `ly:item-get-column`'s `wrong-type-arg`. With `kept`, what stays of each
/// when the lines move goes there, and it is none if one would not stay.
fn boxes(
    host: &mut dyn Host,
    me: Value,
    elts: &[Value],
    kept: &mut Option<KeptBoxes>,
) -> Result<Vec<Box>, Exception> {
    let column = |host: &mut dyn Host, e: Value| {
        let heap = host.heap();
        if !item::is_item(heap, e) {
            return Err(crate::wrong_type(
                heap,
                Some("ly:item-get-column"),
                1,
                e,
                "Item",
            ));
        }
        Ok(item::get_column(heap, e).unwrap_or(Value::NULL))
    };
    let pc = column(host, me)?;
    let ycommon =
        grob::common_refpoint_of_array(host.heap(), elts, Some(me), Y).unwrap_or(Value::FALSE);
    let mut out = Vec::new();
    for &il in elts {
        if pc != column(host, il)? || grob::has_interface(host.heap(), il, "axis-group-interface") {
            continue;
        }
        crate::queue::check(crate::queue::Step::Box);
        let mut y = unpure::pure_y_extent(host, il, ycommon, 0, 2_147_483_647)?;
        // A grob of a line given back that moved after a column read it in
        // the compile it was given back from: where it was read then, as a
        // compile from scratch reads it (OPEN B27, D268).
        let given = crate::cell::given_place(host.heap(), il);
        if let Some(first) = &given {
            y = first.extent(host, il, ycommon)?;
        }
        let reads = unpure::translations_read();
        // Else where the box is on Y as read here, kept with the column's
        // boxes and noted with its line ([`crate::cell::note_place`]):
        // reading its X may move it (a dot's column puts its dots), and a
        // line given back holds where it ended.
        let notes = given.is_none() && crate::cell::notes_places(host.heap(), il);
        let place = match &given {
            Some(first) => Some(Place::clone(first)),
            None if kept.is_some() || notes => kept_place(host, il, ycommon),
            None => None,
        };
        if notes {
            crate::cell::note_place(host.heap(), il, place.as_ref());
        }
        let x = grob::extent(host, il, pc, X)?;
        let v = grob::property(host, il, "extra-spacing-width")?;
        let w = grob::number_pair(host.heap(), v).unwrap_or(Interval {
            left: -0.1,
            right: 0.1,
        });
        let v = grob::property(host, il, "extra-spacing-height")?;
        let h = grob::number_pair(host.heap(), v).unwrap_or(Interval {
            left: 0.0,
            right: 0.0,
        });
        let (x, y) = (widened(x, w), widened(y, h));
        if !(x.left > x.right || y.left > y.right) {
            out.push(Box { x, y });
        }
        if let Some(boxes) = kept.as_mut() {
            let steady = unpure::translations_read() == reads;
            match place {
                Some(place) if steady => keep_box(boxes, place, x, h),
                _ => *kept = None,
            }
        }
    }
    Ok(out)
}

/// Where the item `il` is on Y against `common`, as what stays of it when
/// the lines move: its height, the offsets up to its line's group and the
/// line; none if it moves any other way than with its line.
#[derive(Clone)]
pub(crate) struct Place {
    height: Interval,
    offsets: Vec<f64>,
    line: u64,
}

impl Place {
    /// Whether it moves with a line, not with what all lines are put in.
    pub(crate) fn in_line(&self) -> bool {
        self.line != NO_LINE
    }

    /// The pure Y extent against `common` of the item `il` it is the place
    /// of, as [`unpure::pure_y_extent`] adds it: the offsets up to `common`
    /// if it is on the way to the line's group, else up to the group and the
    /// group's pure coordinate, as [`skylines_again`] places a kept box.
    fn extent(&self, host: &mut dyn Host, il: Value, common: Value) -> Result<Interval, Exception> {
        let mut at = il;
        let mut below = None;
        for k in 0..=self.offsets.len() {
            if at == common {
                below = Some(k);
                break;
            }
            match grob::get_parent(host.heap(), at, Y) {
                Some(parent) => at = parent,
                None => break,
            }
        }
        let y = match below {
            Some(k) => unpure::coordinate_from(&self.offsets[..k], 0.0),
            None => {
                let top = match self.line {
                    NO_LINE => None,
                    _ => grob::vertical_axis_group(host.heap(), il).filter(|&g| g != il),
                };
                let at = match top {
                    Some(group) => {
                        unpure::pure_relative_y_coordinate(host, group, common, 0, 2_147_483_647)?
                    }
                    None => 0.0,
                };
                unpure::coordinate_from(&self.offsets, at)
            }
        };
        Ok(Interval {
            left: self.height.left + y,
            right: self.height.right + y,
        })
    }

    /// Whether `other` is the same place, bit for bit.
    pub(crate) fn same(&self, other: &Place) -> bool {
        let bits = |p: &Place| ([p.height.left, p.height.right].map(f64::to_bits), p.line);
        bits(self) == bits(other)
            && self.offsets.len() == other.offsets.len()
            && self
                .offsets
                .iter()
                .zip(&other.offsets)
                .all(|(a, b)| a.to_bits() == b.to_bits())
    }
}

/// The item `il`'s [`Place`] against `common` as it is now.
pub(crate) fn kept_place(host: &mut dyn Host, il: Value, common: Value) -> Option<Place> {
    let (arena, id) = lily_engine_music::grob_of(host.heap(), il)?;
    let height = unpure::steady_height(&arena, id)?;
    let group = grob::vertical_axis_group(host.heap(), il).filter(|&g| g != il);
    let offsets = unpure::offsets_to(host, il, group.unwrap_or(common))?;
    let line = match group {
        Some(group) => crate::cell::line_of(host.heap(), group)?,
        None => NO_LINE,
    };
    Some(Place {
        height,
        offsets,
        line,
    })
}

/// What stays of an item, its [`Place`], its width `x` and what widens its
/// height `extra`, put in `boxes`.
fn keep_box(boxes: &mut KeptBoxes, place: Place, x: lily_engine_fonts::Interval, extra: Interval) {
    let Place {
        height,
        offsets,
        line,
    } = place;
    boxes.numbers.extend([
        x.left,
        x.right,
        height.left,
        height.right,
        extra.left,
        extra.right,
    ]);
    boxes.numbers.extend_from_slice(&offsets);
    boxes.lines.push((line, offsets.len() as u32));
}

/// The skylines of `boxes`, padded by `vp`, as [`calc_skylines`] makes them.
fn skylines_of(host: &mut dyn Host, boxes: &[Box], vp: f64) -> Result<(Value, bool), Exception> {
    let mut pair = SkylinePair::from_boxes(boxes, Horizon::Y);
    let mut warnings = Vec::new();
    skylines::pad(&mut pair, vp, &mut warnings);
    let warned = !warnings.is_empty();
    for w in warnings {
        let f = crate::lily_procedure(host, "ly:warning")?;
        let text = host.heap().string(w);
        host.call(f, &[text])?;
    }
    Ok((skylines::skyline_pair_value(host.heap(), pair), warned))
}

/// `me`'s skylines from what stayed of its boxes, each at its line's place
/// now, padded by `vp`; `groups` finds a line's group.
pub(crate) fn skylines_again(
    host: &mut dyn Host,
    me: Value,
    kept: &KeptBoxes,
    vp: f64,
    group: &dyn Fn(u64) -> Option<Value>,
) -> Result<Option<Value>, Exception> {
    let elts = grob_array::extract_grob_array(host, me, "elements")?;
    let common =
        grob::common_refpoint_of_array(host.heap(), &elts, Some(me), Y).unwrap_or(Value::FALSE);
    let mut out = Vec::with_capacity(kept.len());
    let mut numbers = kept.numbers.as_slice();
    for &(line, count) in &kept.lines {
        crate::queue::check(crate::queue::Step::Box);
        let y = if line == NO_LINE {
            0.0
        } else {
            let Some(g) = group(line) else {
                return Ok(None);
            };
            unpure::pure_relative_y_coordinate(host, g, common, 0, 2_147_483_647)?
        };
        let (b, rest) = numbers.split_at(6 + count as usize);
        numbers = rest;
        let x = lily_engine_fonts::Interval {
            left: b[0],
            right: b[1],
        };
        let height = Interval {
            left: b[2],
            right: b[3],
        };
        let extra = Interval {
            left: b[4],
            right: b[5],
        };
        let y = unpure::coordinate_from(&b[6..], y);
        out.extend(placed(x, height, y, extra));
    }
    Ok(Some(skylines_of(host, &out, vp)?.0))
}

/// `ly:separation-item::calc-skylines`: the skylines left and right of
/// the [`boxes`] of the item's `elements` (`Skyline_pair (boxes, Y_AXIS)`),
/// each padded by `skyline-vertical-padding` (0.0 if no real), with
/// `Skyline::padded`'s warning for a padding below 0, once a side. Rust
/// since MS3 step 59 (05 §5.1), Scheme over `%boxes-skylines` before,
/// where it was most of the eight's layout.
pub fn calc_skylines(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    if !item::is_item(host.heap(), me) {
        let heap = host.heap();
        return Err(crate::wrong_type(
            heap,
            Some("ly:separation-item::calc-skylines"),
            1,
            me,
            "Item",
        ));
    }
    if let Some(kept) = crate::cell::kept_column_skylines(host, me)? {
        return Ok(kept);
    }
    let elts = grob_array::extract_grob_array(host, me, "elements")?;
    let mut kept = crate::cell::keeps_column_skylines(host.heap(), me).then(KeptBoxes::default);
    let crossed = crate::cell::crossed_mark(host.heap(), me);
    let boxes = boxes(host, me, &elts, &mut kept)?;
    let v = grob::property(host, me, "skyline-vertical-padding")?;
    let heap = host.heap();
    let vp = if heap.is_real(v) {
        heap.to_f64(v).unwrap_or(0.0)
    } else {
        0.0
    };
    let (value, warned) = skylines_of(host, &boxes, vp)?;
    let kept = kept.filter(|_| !warned).map(|boxes| (boxes, vp));
    crate::cell::keep_column_skylines(host, me, value, kept, crossed)?;
    Ok(value)
}

// --- the callback -----------------------------------------------------------

/// What `%host-call` runs for the `ly:separation-item::` primitive (D127):
/// the callback its second argument names, of the grob given, with the VM
/// as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "separation-item",
    f: host_separation_item,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 1] = [("calc-skylines", calc_skylines)];

fn host_separation_item(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such separation item callback")
}

/// `(ly:separation-item::calc-skylines grob)`: a primitive, as LilyPond's
/// subr, that checks its item (LilyPond does not, and crashes on what is
/// none) and tail-calls `%host-call` ([`HOSTS`]).
pub(crate) fn ly_calc_skylines(cx: &mut Cx, args: &[Value]) -> R {
    item::item_arg(cx, "ly:separation-item::calc-skylines", 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("separation-item");
    let name = cx.heap_mut().intern("calc-skylines");
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}
