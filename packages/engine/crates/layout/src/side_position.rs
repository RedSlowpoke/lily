//! `side-position-interface.cc` (part 4's, MS3): `add_support`
//! (`side-position-interface.cc:49–54`, 6 lines), which
//! `Dot_column::add_head` calls, ported first (MS3 step 16); six of part
//! 2's engravers call it too. And `is_on_x_axis` (`:498–502`, 5 lines)
//! with the static `is_on_axis` it calls (`:485–496`, 12), which
//! `Script_row_engraver` calls, ported first (MS3 step 19). And
//! `is_on_y_axis` (`:504–508`, 5 lines), which
//! `Non_musical_script_column_engraver` calls, ported first (MS3 step 22).
//!
//! The side positions themselves (MS3 step 40), ported first for
//! `SystemStartBar`'s `X-offset` and `BarNumber`'s offsets, which every
//! score reaches: `get_support_set` (`:56–82`, 27 lines),
//! `axis_aligned_side_helper` (`:87–106`, 20), the callbacks
//! `ly:side-position-interface::x-aligned-side`, `::y-aligned-side` and
//! `::pure-y-aligned-side` (`:108–143`, 36) over `aligned_side`
//! (`:188–456`, 269), and `::move-to-extremal-staff` (`:510–564`, 55),
//! the music crate's placeholders until then (D67). They are Rust under
//! primitives that print as LilyPond's subrs and call it over
//! `%host-call` (D127, [`HOSTS`]), as the stem's are. And the rest of the
//! file (MS3 step 48), for Y2's outside-staff grobs: `::calc-cross-staff`
//! (`:145–184`, 40 lines), `DynamicLineSpanner`'s and, through
//! `script-or-side-position-cross-staff`, the scripts', the music crate's
//! placeholder until then (D67); and `set_axis` with `::set-axis!`
//! (`:458–483`, 26), which `Trill_spanner_engraver`'s Scheme calls.
//!
//! LilyPond's support set is a `std::unordered_set` of grob pointers,
//! which it walks in an order of addresses; the engine's is the
//! `side-support-elements`' order, each grob once. The skylines merged and
//! the common refpoints found are the same in either order. LilyPond
//! 2.26.0's clang fuses `aligned_side`'s sums of products (otool: the
//! padding's `total_off +=`, the staff padding's `diff` and its
//! `total_off +=`, and both of the quantized position's `total_off +=`).
//! Where LilyPond reads through a null, a grob without the Y parent it
//! reads is Guile's type error, and an entry of an accidental placement
//! that is no grob is left out of the support.

use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};
use lily_engine_skyline::{Axis as Horizon, Direction as Sky, Skyline, SkylinePair};

use crate::axis_group;
use crate::direction::get_grob_direction;
use crate::geometry::{Axis, Direction, Interval, X, Y, fma, max, other_axis};
use crate::grob;
use crate::grob_array::{self, GrobArray};
use crate::item;
use crate::skylines;
use crate::spacing_options::double_or;
use crate::spanner;
use crate::staff_grouper;
use crate::staff_symbol;
use crate::unpure::{self, get_maybe_pure_property};

const DOWN: Direction = -1;
const UP: Direction = 1;

/// `add_support`: `e` added to `me`'s `side-support-elements`, an
/// unordered grob array.
pub fn add_support(host: &mut dyn Host, me: Value, e: Value) -> Result<(), Exception> {
    let sym = host.heap().intern("side-support-elements");
    grob_array::add_unordered_grob(host, me, sym, e)
}

/// `is_on_axis`: whether `me`'s `side-axis` is `a`. A grob whose
/// `side-axis` is no axis (`is_scm<Axis>`, an exact 0 or 1) is on
/// neither, LilyPond's programming error if it has a stencil.
fn is_on_axis(host: &mut dyn Host, me: Value, a: Axis) -> Result<bool, Exception> {
    let axis_scm = grob::property(host, me, "side-axis")?;
    if let Some(axis @ 0..=1) = axis_scm.as_fixnum() {
        return Ok(axis as Axis == a);
    }

    if grob::property(host, me, "stencil")? != Value::FALSE {
        let text = alloc::format!(
            "no side-axis setting found for grob {}.",
            grob::grob_name(host.heap(), me)
        );
        grob::programming_error(host, me, &text)?;
    }
    Ok(false)
}

/// `is_on_x_axis`.
pub fn is_on_x_axis(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    is_on_axis(host, me, X)
}

/// `is_on_y_axis`.
pub fn is_on_y_axis(host: &mut dyn Host, me: Value) -> Result<bool, Exception> {
    is_on_axis(host, me, Y)
}

// --- the side positions ------------------------------------------------------

/// `get_support_set`: `me`'s `side-support-elements`, an accidental
/// placement among them replaced by the accidentals of its
/// `accidental-grobs`' entries; each grob once, where it first is.
pub fn get_support_set(host: &mut dyn Host, me: Value) -> Result<Vec<Value>, Exception> {
    // Only slightly kludgy heuristic...
    // We want to make sure that all AccidentalPlacements'
    // accidentals make it into the side support
    let proto_support = grob_array::extract_grob_array(host, me, "side-support-elements")?;
    let mut support = Vec::new();
    let insert = |support: &mut Vec<Value>, g: Value| {
        if !support.contains(&g) {
            support.push(g);
        }
    };
    for p in proto_support {
        if grob::has_interface(host.heap(), p, "accidental-placement-interface") {
            let sym = host.heap().intern("accidental-grobs");
            let mut acs = grob::object(host, p, sym)?;
            let heap = host.heap();
            while let (Some(entry), Some(rest)) = (heap.car(acs), heap.cdr(acs)) {
                let mut s = heap.cdr(entry).unwrap_or(Value::NULL);
                while let (Some(a), Some(next)) = (heap.car(s), heap.cdr(s)) {
                    if grob_of(heap, a).is_some() {
                        insert(&mut support, a);
                    }
                    s = next;
                }
                acs = rest;
            }
        } else {
            insert(&mut support, p);
        }
    }
    Ok(support)
}

/// The skyline side of `d`.
fn sky(d: Direction) -> Sky {
    if d < 0 { Sky::Down } else { Sky::Up }
}

/// The skylines' horizon axis `a`.
fn horizon(a: Axis) -> Horizon {
    if a == X { Horizon::X } else { Horizon::Y }
}

/// A grob's handle as a refpoint, `'()` (the root) for none.
fn refp(g: Option<Value>) -> Value {
    g.unwrap_or(Value::NULL)
}

/// `g->common_refpoint (other, a)`: none for no `other`.
fn common_with(host: &mut dyn Host, g: Value, other: Option<Value>, a: Axis) -> Option<Value> {
    other.and_then(|o| grob::common_refpoint(host.heap(), g, o, a))
}

/// `parent_relative`: the coordinate of the grob's parent on `a` relative
/// to `common`, 0 without one.
fn parent_relative(
    host: &mut dyn Host,
    g: Value,
    common: Option<Value>,
    a: Axis,
) -> Result<f64, Exception> {
    match grob::get_parent(host.heap(), g, a) {
        Some(p) => grob::relative_coordinate(host, p, refp(common), a),
        None => Ok(0.0),
    }
}

/// `get_y_parent`, read through: Guile's type error for none, where
/// LilyPond 2.26.0 dereferences no grob.
fn y_parent(host: &mut dyn Host, g: Value) -> Result<Value, Exception> {
    grob::get_parent(host.heap(), g, Y)
        .ok_or_else(|| crate::wrong_type(host.heap(), None, 0, Value::NULL, "Grob"))
}

/// `maybe_pure_coordinate` on Y: pure between `start` and `end`, or not.
fn maybe_pure_y_coordinate(
    host: &mut dyn Host,
    g: Value,
    common: Option<Value>,
    pure: bool,
    start: usize,
    end: usize,
) -> Result<f64, Exception> {
    if pure {
        unpure::pure_relative_y_coordinate(host, g, refp(common), start, end)
    } else {
        grob::relative_coordinate(host, g, refp(common), Y)
    }
}

/// `maybe_pure_extent` on Y: pure between `start` and `end`, or not.
fn maybe_pure_y_extent(
    host: &mut dyn Host,
    g: Value,
    common: Option<Value>,
    pure: bool,
    start: usize,
    end: usize,
) -> Result<Interval, Exception> {
    if pure {
        unpure::pure_y_extent(host, g, refp(common), start, end)
    } else {
        grob::extent(host, g, refp(common), Y)
    }
}

/// `is_scm<Skyline_pair>` of `v`, and its skylines as
/// `from_scm<Skyline_pair>` gives them, read at once: none for what is no
/// pair of skylines; LilyPond's error, thrown where LilyPond converts it,
/// for two the wrong way round.
fn skylines_of(host: &mut dyn Host, v: Value) -> Option<Result<SkylinePair, Exception>> {
    match skylines::skyline_pair_in(host.heap(), v) {
        Ok(None) => None,
        Ok(Some(skyp)) => Some(Ok(skyp)),
        Err(message) => Some(Err(crate::misc_error(host.heap(), "from_scm", message))),
    }
}

/// `scm_is_number`.
fn is_number(host: &mut dyn Host, v: Value) -> bool {
    host.heap().is_number(v)
}

/// `Staff_symbol_referencer::staff_space`: `%staff-space`.
fn staff_space(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    crate::staff_symbol_referencer::staff_space(host, me)
}

/// `Staff_symbol_referencer::get_staff_symbol`: `%staff-symbol`.
fn get_staff_symbol(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    crate::staff_symbol_referencer::staff_symbol(host, me)
}

/// `directed_round`: down by `floor`, else up by `ceil`.
fn directed_round(f: f64, d: Direction) -> f64 {
    if d < 0 { libm::floor(f) } else { libm::ceil(f) }
}

/// `is_scm<Direction> (get_property_data (e, "direction"))`: an exact -1,
/// 0 or 1, no callback called.
fn has_direction(host: &mut dyn Host, e: Value) -> bool {
    let heap = host.heap();
    let sym = heap.intern("direction");
    let v = match grob_of(heap, e) {
        Some((arena, id)) => grob::property_data(heap, &arena, id, e, sym),
        None => Value::NULL,
    };
    matches!(v.as_fixnum(), Some(-1..=1))
}

/// `aligned_side`: `me`'s offset on `a` that puts it on the side its
/// `direction` gives of its support's skylines, `padding` (or
/// `X-padding`) staff spaces past them, at least `minimum-space` (or
/// `minimum-X-space`) staff spaces out, past `current_off` if given, and
/// on Y `staff-padding` staff spaces from the staff or quantized between
/// its lines; pure between the columns `start` and `end` with `pure`.
/// Without a direction, `current_off`, else 0. An offset over 1000 is
/// LilyPond's programming error, and with the option
/// `strict-infinity-checking` its error.
pub fn aligned_side(
    host: &mut dyn Host,
    me: Value,
    a: Axis,
    pure: bool,
    start: usize,
    end: usize,
    current_off: Option<f64>,
) -> Result<f64, Exception> {
    let dir = get_grob_direction(host, me)?;

    if dir == 0 {
        // This is occasionally useful, for example to place
        // scripts in the middle of two piano staves using a
        // Dynamics context.
        return Ok(current_off.unwrap_or(0.0));
    }

    let support = get_support_set(host, me)?;

    let mut common = [None; 2];
    for ax in [X, Y] {
        let from = if ax == a {
            grob::get_parent(host.heap(), me, ax)
        } else {
            Some(me)
        };
        common[ax] = grob::common_refpoint_of_array(host.heap(), &support, from, ax);
    }

    let staff_symbol = get_staff_symbol(host, me)?;
    let quantize_position =
        get_maybe_pure_property(host, me, "quantize-position", pure, start, end)? == Value::TRUE;
    let me_cross_staff = grob::property(host, me, "cross-staff")? == Value::TRUE;

    let include_staff = staff_symbol.is_some()
        && a == Y
        && {
            let v = get_maybe_pure_property(host, me, "staff-padding", pure, start, end)?;
            is_number(host, v)
        }
        && !quantize_position;

    if include_staff && let Some(st) = staff_symbol {
        common[Y] = common_with(host, st, common[Y], Y);
    }

    let skylines_name = if a == X {
        "horizontal-skylines"
    } else {
        "vertical-skylines"
    };
    let mut my_dim = Skyline::empty(sky(-dir));
    let skyp_scm = get_maybe_pure_property(host, me, skylines_name, pure, start, end)?;
    if let Some(skyp) = skylines_of(host, skyp_scm) {
        // for spanner pure heights, we don't know horizontal spacing,
        // so a spanner can never have a meaningful x coordiante
        // we just give it the parents' coordinate because its
        // skyline will likely be of infinite width anyway
        // and we don't want to prematurely trigger H spacing
        let xc = if a == X {
            parent_relative(host, me, common[X], X)?
        } else if !pure {
            grob::relative_coordinate(host, me, refp(common[X]), X)?
        } else {
            // Not safe to call X-offset callbacks here as that may
            // trigger stem and beam direction, so just set to 0
            0.0
        };

        // same here, for X_AXIS spacing, if it's happening, it should only be
        // before line breaking.  because there is no thing as "pure" x spacing,
        // we assume that it is all pure
        let yc = if a == X {
            unpure::pure_relative_y_coordinate(host, me, refp(common[Y]), start, end)?
        } else {
            let parent = y_parent(host, me)?;
            maybe_pure_y_coordinate(host, parent, common[Y], pure, start, end)?
        };
        let mut skyp = skyp?;
        skyp.shift(if a == X { yc } else { xc });
        skyp.raise(if a == X { xc } else { yc });
        my_dim = skyp[sky(-dir)].clone();
    }

    let mut skyps = Vec::new();

    for e in support {
        let cross_staff = grob::property(host, e, "cross-staff")? == Value::TRUE;
        // 'me' promised not to adapt to staff-spacing, but 'e' might move
        // based on staff-pacing, so 'me' may not move in response to 'e'
        if a == Y && !me_cross_staff && cross_staff {
            continue;
        }

        let is_stem = grob::has_interface(host.heap(), e, "stem-interface");
        if a == Y && is_stem {
            // If called as 'pure' we may not force a stem to set its direction,
            if pure && !has_direction(host, e) {
                continue;
            }
            // There is no need to consider stems pointing away.
            if dir == -get_grob_direction(host, e)? {
                continue;
            }
        }

        let skyp_scm = get_maybe_pure_property(host, e, skylines_name, pure, start, end)?;
        // else: no warning
        if let Some(skyp) = skylines_of(host, skyp_scm) {
            let xc = if pure && spanner::is_spanner(host.heap(), e) {
                parent_relative(host, e, common[X], X)?
            } else {
                grob::relative_coordinate(host, e, refp(common[X]), X)?
            };
            // same logic as above
            // we assume horizontal spacing is always pure
            let yc = if a == X {
                unpure::pure_relative_y_coordinate(host, e, refp(common[Y]), start, end)?
            } else {
                maybe_pure_y_coordinate(host, e, common[Y], pure, start, end)?
            };
            let mut skyp = skyp?;
            if a == Y
                && is_stem
                && get_maybe_pure_property(host, me, "add-stem-support", pure, start, end)?
                    == Value::TRUE
            {
                let h = skyp[sky(dir)].max_height();
                skyp[sky(dir)].set_minimum_height(h);
            }
            skyp.shift(if a == X { yc } else { xc });
            skyp.raise(if a == X { xc } else { yc });
            skyps.push(skyp);
        }
    }

    let mut dim = Skyline::from_boxes(&[], horizon(other_axis(a)), sky(dir));
    if !skyps.is_empty() {
        let merged = SkylinePair::from_pairs(&skyps);
        dim.merge(&merged[sky(dir)]);
    }

    if include_staff && let Some(st) = staff_symbol {
        common[Y] = common_with(host, st, common[Y], Y);
        let staff_extents = maybe_pure_y_extent(host, st, common[Y], pure, start, end)?;
        dim.set_minimum_height(staff_extents.at(dir));
    }

    // Sometimes, we want to side position for grobs but they
    // don't position against anything.  Some cases where this is true:
    //   - StanzaNumber if the supporting lyrics are hara-kiri'd
    //     SystemStartBracket
    //     InstrumentName
    // In all these cases, we set the height of the support to 0.
    // This becomes then like the self-alignment-interface with the
    // caveat that there is padding added.
    if dim.is_empty() {
        dim = Skyline::empty(dim.sky());
        dim.set_minimum_height(0.0);
    }

    let ss = staff_space(host, me)?;
    let v = get_maybe_pure_property(host, me, "horizon-padding", pure, start, end)?;
    let horizon_padding = double_or(host, v, 0.0);
    let dist = dim.distance(&my_dim, horizon_padding);
    let d = f64::from(dir);
    let mut total_off = if !dist.is_infinite() { d * dist } else { 0.0 };

    let v = get_maybe_pure_property(host, me, "padding", pure, start, end)?;
    let mut padding = double_or(host, v, 0.0);
    if a == X {
        let x_padding = get_maybe_pure_property(host, me, "X-padding", pure, start, end)?;
        if is_number(host, x_padding) {
            padding = double_or(host, x_padding, 0.0);
        }
    }

    total_off = fma(d * ss, padding, total_off);

    let v = get_maybe_pure_property(host, me, "minimum-space", pure, start, end)?;
    let mut minimum_space = ss * double_or(host, v, -1.0);
    if a == X {
        let minimum_x_space =
            get_maybe_pure_property(host, me, "minimum-X-space", pure, start, end)?;
        if is_number(host, minimum_x_space) {
            minimum_space = ss * double_or(host, minimum_x_space, 0.0);
        }
    }

    if minimum_space >= 0.0 && total_off * d < minimum_space {
        total_off = minimum_space * d;
    }

    if let Some(current_off) = current_off {
        total_off = d * max(d * total_off, d * current_off);
    }

    // FIXME: 1000 should relate to paper size.
    if libm::fabs(total_off) > 1000.0 {
        let msg = alloc::format!(
            "Improbable offset for grob {}: {total_off:.6}",
            grob::grob_name(host.heap(), me)
        );

        grob::unlocated_programming_error(host, &msg)?;
        let option = host.heap().intern("strict-infinity-checking");
        if crate::call_lily(host, "ly:get-option", &[option])? != Value::FALSE {
            return Err(crate::misc_error(
                host.heap(),
                "aligned_side",
                "Improbable offset.",
            ));
        }
    }

    // Ensure 'staff-padding' from my refpoint to the staff.  This is similar to
    // side-position with padding, but it will put adjoining objects on a row if
    // stuff sticks out of the staff a little.
    let staff = get_staff_symbol(host, me)?;
    if let Some(staff) = staff
        && a == Y
    {
        if quantize_position {
            let common = grob::common_refpoint(host.heap(), me, staff, Y);
            let parent = y_parent(host, me)?;
            let my_off = maybe_pure_y_coordinate(host, parent, common, pure, start, end)?;
            let staff_off = maybe_pure_y_coordinate(host, staff, common, pure, start, end)?;
            let v = crate::call_lily(host, "%staff-symbol-staff-space", &[staff])?;
            let ss = host.heap().to_f64(v).unwrap_or(1.0);
            let position = 2.0 * (my_off + total_off - staff_off) / ss;
            let rounded = directed_round(position, dir);
            let head = grob::get_parent(host.heap(), me, X);

            let v = crate::call_lily(host, "%staff-symbol-line-span", &[staff])?;
            let heap = host.heap();
            let end_of = |e: Option<Value>| e.and_then(|e| heap.to_f64(e)).unwrap_or(0.0);
            let mut staff_span = Interval::new(end_of(heap.car(v)), end_of(heap.cdr(v)));
            staff_span.widen(1.0);
            let on_head = head.is_some_and(|h| grob::has_interface(heap, h, "note-head-interface"));
            if (staff_span.left <= position && position <= staff_span.right)
                // If we are between notehead and staff, quantize for ledger lines.
                || (on_head && d * position < 0.0)
            {
                total_off = fma((rounded - position) * 0.5, ss, total_off);
                if staff_symbol::on_line(host, staff, rounded as i32, true)? {
                    total_off = fma(d * 0.5, ss, total_off);
                }
            }
        } else {
            let v = get_maybe_pure_property(host, me, "staff-padding", pure, start, end)?;
            if is_number(host, v) {
                let ss = staff_space(host, me)?;
                let v = get_maybe_pure_property(host, me, "staff-padding", pure, start, end)?;
                let staff_padding = ss * grob::to_double(host.heap(), v)?;

                let parent = y_parent(host, me)?;
                let common = grob::common_refpoint(host.heap(), me, staff, Y);
                let parent_position =
                    maybe_pure_y_coordinate(host, parent, common, pure, start, end)?;
                let staff_position =
                    maybe_pure_y_coordinate(host, staff, common, pure, start, end)?;
                let staff_extent = maybe_pure_y_extent(host, staff, Some(staff), pure, start, end)?;
                let diff = fma(
                    d,
                    staff_position - parent_position,
                    fma(-d, total_off, fma(d, staff_extent.at(dir), staff_padding)),
                );
                total_off = fma(d, max(diff, 0.0), total_off);
            }
        }
    }
    Ok(total_off)
}

/// `axis_aligned_side_helper`: [`aligned_side`] on `a`, after the current
/// offset `current_off_scm` if it is a number; never pure on X for a
/// spanner, whose widths are only wanted after line breaking.
fn axis_aligned_side_helper(
    host: &mut dyn Host,
    me: Value,
    a: Axis,
    mut pure: bool,
    start: usize,
    end: usize,
    current_off_scm: Option<Value>,
) -> Result<Value, Exception> {
    let current_off = match current_off_scm {
        Some(v) if is_number(host, v) => Some(grob::to_double(host.heap(), v)?),
        _ => None,
    };

    // We will only ever want widths of spanners after line breaking
    // so we can set pure to false
    if a == X && spanner::is_spanner(host.heap(), me) {
        pure = false;
    }

    let off = aligned_side(host, me, a, pure, start, end, current_off)?;
    Ok(host.heap().flonum(off))
}

/// `move_to_extremal_staff`: `me`, on a staff grouper or on its system,
/// moved onto the extremal live staff of the grouper or of the system's
/// `vertical-alignment` (`Staff_grouper_interface::get_extremal_staff`,
/// in `me`'s `direction`, up unless down, over its X extent widened by
/// 1): its Y parent, one of its elements, its `side-support-elements`
/// only those with it as their common refpoint on Y; `#t`. `#f`, not
/// moved, for a grob on another parent or without such a staff.
fn move_to_extremal_staff(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let mut dir = get_grob_direction(host, me)?;
    if dir != DOWN {
        dir = UP;
    }

    let sys = item::get_system(host.heap(), me);
    let mut iv = grob::extent(host, me, refp(sys), X)?;
    iv.widen(1.0);

    let grouper = grob::get_parent(host.heap(), me, Y);
    let grouper = match grouper {
        // find the extremal staff of this group
        Some(g) if grob::has_interface(host.heap(), g, "staff-grouper-interface") => g,
        // find the extremal staff of the whole system
        _ if grouper == sys => {
            let Some(sys) = sys else {
                return Ok(Value::FALSE);
            };
            let sym = host.heap().intern("vertical-alignment");
            let align = grob::object(host, sys, sym)?;
            if grob_of(host.heap(), align).is_none() {
                return Ok(Value::FALSE);
            }
            align
        }
        // do not move marks from other staves to the top staff
        _ => return Ok(Value::FALSE),
    };

    // N.B. It's ugly to pass a VerticalAlignment to this staff-grouper function.
    // Read the comments in the function for more detail.
    let Some(top_staff) = staff_grouper::get_extremal_staff(host, grouper, refp(sys), dir, iv)?
    else {
        return Ok(Value::FALSE);
    };

    let order = grob_array::extract_grob_array(host, grouper, "elements")?;
    let first = if dir == UP {
        order.first()
    } else {
        order.last()
    };
    crate::cell::carried(host.heap(), me, top_staff, first == Some(&top_staff));

    grob::set_parent(host.heap(), me, top_staff, Y);
    grob::flush_extent_cache(host.heap(), me, Y);
    axis_group::add_element(host, top_staff, me)?;

    // Remove any cross-staff side-support dependencies
    let sym = host.heap().intern("side-support-elements");
    let ga = grob::object(host, me, sym)?;
    let heap = host.heap();
    if heap.foreign::<GrobArray>(ga).is_some() {
        let new_elts: Vec<Value> = grob_array::grobs_of(heap, ga)
            .into_iter()
            .filter(|&e| grob::common_refpoint(heap, me, e, Y) == Some(top_staff))
            .collect();
        if let Some(arr) = heap.foreign_mut::<GrobArray>(ga) {
            arr.grobs = new_elts;
        }
    }
    Ok(Value::TRUE)
}

/// `calc_cross_staff`: whether `me` is placed by something across the
/// staves: an element of its `side-support-elements` that is `cross-staff`
/// and has no `direction` of its own, or points the way `me` does, or one
/// of another vertical axis group than `me`'s.
fn calc_cross_staff(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let elts = grob_array::extract_grob_array(host, me, "side-support-elements")?;
    let my_dir = get_grob_direction(host, me)?;
    for &e in &elts {
        if grob::property(host, e, "cross-staff")? != Value::TRUE {
            continue;
        }
        // If 'me' is placed relative to any cross-staff element with a
        // 'direction callback defined, the placement of 'me' is likely to
        // depend on staff-spacing, thus 'me' should be considered
        // cross-staff.
        if !has_direction(host, e) {
            return Ok(Value::TRUE);
        }
        // If elts[i] is cross-staff and is pointing in the same direction
        // as 'me', we assume that the alignment of 'me' is influenced the
        // cross-staffitude of elts[i] and thus we mark 'me' as cross-staff.
        if my_dir == get_grob_direction(host, e)? {
            return Ok(Value::TRUE);
        }
    }

    let heap = host.heap();
    let myvag = grob::vertical_axis_group(heap, me);
    let other = elts
        .iter()
        .any(|&e| grob::vertical_axis_group(heap, e) != myvag);
    Ok(Value::bool(other))
}

/// `set_axis`: unless `me` has a `side-axis` already, `a` as its
/// `side-axis` and its offset on `a` chained with the side position on it,
/// on Y with its pure form.
pub fn set_axis(host: &mut dyn Host, me: Value, a: Axis) -> Result<(), Exception> {
    let side_axis = grob::property(host, me, "side-axis")?;
    if host.heap().is_number(side_axis) {
        return Ok(());
    }
    let axis = host.heap().integer(a as i64);
    grob::set_property(host.heap(), me, "side-axis", axis);
    let proc = if a == X {
        crate::lily_procedure(host, "ly:side-position-interface::x-aligned-side")?
    } else {
        let unpure = crate::lily_procedure(host, "ly:side-position-interface::y-aligned-side")?;
        let pure = crate::lily_procedure(host, "ly:side-position-interface::pure-y-aligned-side")?;
        crate::call_lily(host, "ly:make-unpure-pure-container", &[unpure, pure])?
    };
    grob::chain_offset_callback(host, me, proc, a)
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:side-position-interface::`
/// primitives (D127): the callback its second argument names, of the grob
/// given, the pure one's columns and the current offset if given, with the
/// VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "side-position",
    f: host_side_position,
}];

fn host_side_position(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let me = args[2];
    let column = |vm: &mut Vm, v: Value| {
        let n = vm.heap().to_i64(v).unwrap_or(0);
        usize::try_from(n).unwrap_or(0)
    };
    match vm.heap().name(args[1]) {
        Some("x-aligned-side") => {
            axis_aligned_side_helper(vm, me, X, true, 0, 0, args.get(3).copied())
        }
        Some("y-aligned-side") => {
            axis_aligned_side_helper(vm, me, Y, false, 0, 0, args.get(3).copied())
        }
        Some("pure-y-aligned-side") => {
            let (start, end) = (column(vm, args[3]), column(vm, args[4]));
            axis_aligned_side_helper(vm, me, Y, true, start, end, args.get(5).copied())
        }
        Some("move-to-extremal-staff") => move_to_extremal_staff(vm, me),
        Some("calc-cross-staff") => calc_cross_staff(vm, me),
        Some("set-axis!") => {
            let a = vm.heap().to_i64(args[3]).unwrap_or(0) as Axis;
            set_axis(vm, me, a)?;
            Ok(Value::UNSPECIFIED)
        }
        _ => Err(crate::misc_error(
            vm.heap(),
            "%host-call",
            "no such side position callback",
        )),
    }
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:side-position-interface::… grob …)`: LilyPond's subrs, primitives
/// here so that they print as LilyPond's
/// (`#<procedure ly:side-position-interface::y-aligned-side (_ #:optional _)>`).
/// Each checks its grob, under the method's name as LilyPond's
/// `LY_ASSERT_SMOB` gives it (`ly:y-aligned-side`); the pure one then
/// converts its columns (`from_scm<vsize>`), `::set-axis!` checks its axis;
/// and it tail-calls `%host-call` with its name and its arguments
/// ([`HOSTS`]).
fn callback(cx: &mut Cx, subr: &str, what: &str, args: &[Value]) -> R {
    grob::grob_arg(cx, subr, 1, args[0])?;
    if what == "pure-y-aligned-side" {
        crate::simple_spacer::vsize(cx, args[1])?;
        crate::simple_spacer::vsize(cx, args[2])?;
    }
    if what == "set-axis!" {
        crate::axis_arg(cx, subr, 2, args[1])?;
    }
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let mut call = [Value::NULL; 6];
    call[0] = cx.heap_mut().intern("side-position");
    call[1] = cx.heap_mut().intern(what);
    call[2..2 + args.len()].copy_from_slice(args);
    Ok(cx.tail_call(f, &call[..2 + args.len()]))
}

macro_rules! callbacks {
    ($($f:ident $what:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            callback(cx, concat!("ly:", $what), $what, args)
        })*
    };
}

callbacks! {
    ly_calc_cross_staff "calc-cross-staff",
    ly_move_to_extremal_staff "move-to-extremal-staff",
    ly_set_axis "set-axis!",
    ly_pure_y_aligned_side "pure-y-aligned-side",
    ly_x_aligned_side "x-aligned-side",
    ly_y_aligned_side "y-aligned-side",
}
