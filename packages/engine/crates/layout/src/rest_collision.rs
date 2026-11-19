//! `rest-collision.cc` (part 3's, MS3 step 41): the rest collision's
//! positioning, `ly:rest-collision::calc-positioning-done`
//! (`rest-collision.cc:99–292`, 194 lines), the music crate's placeholder
//! until then (D67), and `ly:rest-collision::force-shift-callback-rest`
//! (`:42–67`, 26), the offset callback chained on each rest that
//! triggers it: Rust under primitives that print as LilyPond's subrs and
//! call it over `%host-call` (D127, [`HOSTS`]), as the note collision's
//! do. And `add_column` (`:69–85`, 17), which `Rest_collision_engraver`
//! calls (`rest_collision_engraver.rs`), with `rest.cc`'s
//! `Rest::translate` (`:263–273`, 11).
//!
//! LilyPond sorts each direction's rests with `std::sort`, which is not
//! stable: the port sorts with libc++'s ([`lily_engine_skyline::sort`]),
//! each column's `horizontal-shift` read once before, where LilyPond
//! reads it at each comparison (the same error, if one throws).

use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::direction;
use crate::geometry::{Direction, Interval, X, Y};
use crate::grob;
use crate::grob_array;
use crate::item;
use crate::note_column;

const DOWN: Direction = -1;
const UP: Direction = 1;

/// A `Drul_array`'s index of `d`: 0 down, 1 up.
fn ix(d: Direction) -> usize {
    usize::from(d > 0)
}

/// `get_object (me, name)` if it is a grob.
fn grob_object(host: &mut dyn Host, me: Value, name: &str) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern(name);
    let g = grob_array::object(host, me, sym)?;
    Ok(grob_of(host.heap(), g).is_some().then_some(g))
}

/// `Staff_symbol_referencer::staff_space`.
fn staff_space(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    crate::staff_symbol_referencer::staff_space(host, me)
}

/// `Grob::translate_axis`, LilyPond's programming error for an infinite
/// distance.
fn translate_axis(host: &mut dyn Host, me: Value, y: f64) -> Result<(), Exception> {
    match grob::translate_axis(host.heap(), me, y, Y) {
        Ok(()) => Ok(()),
        Err(text) => grob::programming_error(host, me, text),
    }
}

/// `Rest::translate`: the rest `me` moved `dy` half staff spaces, and its
/// Y parent's extent flushed, unless its `staff-position` is a number.
fn translate(host: &mut dyn Host, me: Value, dy: i32) -> Result<(), Exception> {
    let pos = grob::property(host, me, "staff-position")?;
    if host.heap().is_number(pos) {
        return Ok(());
    }
    let ss = staff_space(host, me)?;
    translate_axis(host, me, f64::from(dy) * ss / 2.0)?;
    if let Some(p) = grob::get_parent(host.heap(), me, Y) {
        grob::flush_extent_cache(host.heap(), p, Y);
    }
    Ok(())
}

/// `force_shift_callback_rest`: the rest moved `offset` on Y if it is a
/// number (the offset its callback chain had so far), and the rest
/// collision of its note column positioned; 0.
pub fn force_shift_callback_rest(
    host: &mut dyn Host,
    rest: Value,
    offset: Value,
) -> Result<Value, Exception> {
    let parent = grob::get_parent(host.heap(), rest, X);
    if host.heap().is_number(offset) {
        let y = host.heap().to_f64(offset).unwrap_or(0.0);
        translate_axis(host, rest, y)?;
    }
    if let Some(parent) = parent
        && grob::has_interface(host.heap(), parent, "note-column-interface")
        && note_column::has_rests(host, parent)?
        && let Some(collision) = grob_object(host, parent, "rest-collision")?
    {
        grob::property(host, collision, "positioning-done")?;
    }
    Ok(host.heap().flonum(0.0))
}

/// `add_column`: the note column `p` made an element of the rest
/// collision `me` and given it as its `rest-collision`; its rest's
/// `Y-offset` chained with `force-shift-callback-rest`, its pure part
/// `pure-chain-offset-callback`.
pub fn add_column(host: &mut dyn Host, me: Value, p: Value) -> Result<(), Exception> {
    let sym = host.heap().intern("elements");
    grob_array::add_grob(host, me, sym, p)?;
    let heap = host.heap();
    let sym = heap.intern("rest-collision");
    grob::set_object(heap, p, sym, me);
    if let Some(rest) = grob_object(host, p, "rest")? {
        let unpure = crate::lily_procedure(host, "ly:rest-collision::force-shift-callback-rest")?;
        let pure = crate::lily_procedure(host, "pure-chain-offset-callback")?;
        let upc = crate::call_lily(host, "ly:make-unpure-pure-container", &[unpure, pure])?;
        grob::chain_offset_callback(host, rest, upc, Y)?;
    }
    Ok(())
}

/// `calc_positioning_done`: `me`'s rests moved clear of each other, or,
/// with notes among its columns, of the other voices' notes; `#t`.
pub fn calc_positioning_done(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    grob::set_property(host.heap(), me, "positioning-done", Value::TRUE);
    let mut rests = Vec::new();
    let mut notes = Vec::new();
    for e in grob_array::extract_grob_array(host, me, "elements")? {
        if grob::has_interface(host.heap(), e, "note-column-interface") {
            if grob_object(host, e, "rest")?.is_some() {
                rests.push(e);
            } else {
                notes.push(e);
            }
        }
    }

    // No partners to collide with.
    if rests.len() + notes.len() < 2 {
        return Ok(Value::TRUE);
    }
    let staff_space = staff_space(host, me)?;
    if notes.is_empty() {
        rests_only(host, me, &rests, staff_space)?;
    } else {
        rests_and_notes(host, me, &rests, &notes)?;
    }
    Ok(Value::TRUE)
}

/// The rests of columns without notes: the last of each direction (by
/// `horizontal-shift`) moved apart, the down one first, by whole staff
/// spaces; each before it moved clear of the one after it. A rest without
/// a direction is left out after LilyPond's warning at the first column;
/// with rests of one direction only, nothing moves (LilyPond's warning if
/// there are several).
fn rests_only(
    host: &mut dyn Host,
    me: Value,
    rests: &[Value],
    staff_space: f64,
) -> Result<(), Exception> {
    let mut ordered: [Vec<Value>; 2] = [Vec::new(), Vec::new()];
    for &col in rests {
        let Some(r) = grob_object(host, col, "rest")? else {
            continue;
        };
        let d = direction::get_grob_direction(host, r)?;
        if d != 0 {
            ordered[ix(d)].push(r);
        } else {
            grob::warning(
                host,
                rests[0],
                "cannot resolve rest collision: rest direction not set",
            )?;
        }
    }
    for side in &mut ordered {
        if side.len() < 2 {
            continue;
        }
        let mut keyed = Vec::with_capacity(side.len());
        for &r in side.iter() {
            let shift = match grob::get_parent(host.heap(), r, X) {
                Some(col) => note_column::horizontal_shift(host, col)?,
                None => 0,
            };
            keyed.push((shift, r));
        }
        lily_engine_skyline::sort(&mut keyed, &|a: &(i32, Value), b: &(i32, Value)| a.0 < b.0);
        *side = keyed.into_iter().map(|(_, r)| r).collect();
    }
    for d in [DOWN, UP] {
        if ordered[ix(d)].is_empty() {
            if ordered[ix(-d)].len() > 1 {
                grob::warning(host, ordered[ix(-d)][0], "too many colliding rests")?;
            }
            return Ok(());
        }
    }

    let heap = host.heap();
    let common = grob::common_refpoint_of_array(heap, &ordered[ix(DOWN)], Some(me), Y);
    let common = grob::common_refpoint_of_array(heap, &ordered[ix(UP)], common, Y).unwrap_or(me);
    let (down, up) = (
        ordered[ix(DOWN)][ordered[ix(DOWN)].len() - 1],
        ordered[ix(UP)][ordered[ix(UP)].len() - 1],
    );
    let top = grob::extent(host, down, common, Y)?.at(UP);
    let bottom = grob::extent(host, up, common, Y)?.at(DOWN);
    let mut diff = (top - bottom) / staff_space;
    if diff > 0.0 {
        let amount_down = libm::ceil(diff / 2.0) as i32;
        diff -= f64::from(amount_down);
        translate(host, down, -2 * amount_down)?;
        if diff > 0.0 {
            translate(host, up, 2 * libm::ceil(diff) as i32)?;
        }
    }

    for d in [DOWN, UP] {
        let side = &ordered[ix(d)];
        for i in (0..side.len() - 1).rev() {
            let last_y = grob::extent(host, side[i + 1], common, Y)?.at(d);
            let y = grob::extent(host, side[i], common, Y)?.at(-d);
            let diff = f64::from(d) * ((last_y - y) / staff_space);
            if diff > 0.0 {
                let amount = libm::ceil(diff) as i32 * 2;
                translate(host, side[i], d * amount)?;
            }
        }
    }
    Ok(())
}

/// The rests of `rests`' columns moved, each its direction's way (its
/// own, else its column's), clear of the notes by `minimum-distance`
/// staff spaces: by half spaces outside the staff, whole ones inside it.
/// Of the notes, the stems of those on the rest's side that sound with
/// it count, of the others only their first heads. Rests with a
/// `staff-position` stay; LilyPond's warning at each further rest moved
/// one way.
fn rests_and_notes(
    host: &mut dyn Host,
    me: Value,
    rests: &[Value],
    notes: &[Value],
) -> Result<(), Exception> {
    // How many rests move each way.
    let mut rcount = [0; 2];
    for &rcol in rests {
        let Some(rest) = grob_object(host, rcol, "rest")? else {
            continue;
        };
        let mut dir = direction::get_grob_direction(host, rest)?;
        if dir == 0 {
            dir = note_column::dir(host, rcol)?;
        }

        // Pre-positioned rests are neither moved nor counted.
        let pos = grob::property(host, rest, "staff-position")?;
        if host.heap().is_number(pos) {
            continue;
        }

        let common =
            grob::common_refpoint_of_array(host.heap(), notes, Some(rcol), Y).unwrap_or(rcol);
        let restdim = grob::extent(host, rest, common, Y)?;
        if restdim.is_empty() {
            continue;
        }

        let staff_space = staff_space(host, rcol)?;
        let v = grob::property(host, me, "minimum-distance")?;
        let heap = host.heap();
        let minimum_dist = if heap.is_number(v) {
            heap.to_f64(v).unwrap_or(1.0)
        } else {
            1.0
        } * staff_space;

        let mut notedim = Interval::EMPTY;
        for &n in notes {
            let heap = host.heap();
            let other_column = item::get_column(heap, n) != item::get_column(heap, rest);
            if note_column::dir(host, n)? == -dir || other_column {
                // Not the stem, which may trigger a beam's positioning
                // too early.
                match note_column::first_head(host, n)? {
                    Some(head) => notedim.unite(grob::extent(host, head, common, Y)?),
                    None => {
                        grob::unlocated_programming_error(host, "Note_column without first_head()")?
                    }
                }
            } else {
                notedim.unite(grob::extent(host, n, common, Y)?);
            }
        }

        let d = f64::from(dir);
        let y = d * 0.0f64.max(-d * restdim.at(-dir) + d * notedim.at(dir) + minimum_dist);

        // Moved discretely by half spaces.
        let mut discrete_y = dir * libm::ceil(y / (0.5 * d * staff_space)) as i32;

        let mut staff_span = crate::staff_symbol_referencer::staff_span(host, rest)?;
        staff_span.widen(1.0);
        let p = crate::staff_symbol_referencer::get_position(host, rest, false)?
            + f64::from(discrete_y);
        // Moved by whole spaces inside the staff.
        if staff_span.left <= p && p <= staff_span.right {
            discrete_y = dir * (libm::ceil(d * f64::from(discrete_y) / 2.0) * 2.0) as i32;
        }

        translate(host, rest, discrete_y)?;
        let counted = rcount[ix(dir)];
        rcount[ix(dir)] += 1;
        if counted > 0 {
            grob::warning(host, rest, "too many colliding rests")?;
        }
    }
    Ok(())
}

/// What `%host-call` runs for `ly:rest-collision::calc-positioning-done`
/// and `ly:rest-collision::force-shift-callback-rest` (D127):
/// [`calc_positioning_done`] and [`force_shift_callback_rest`] of the
/// grob given, with the VM as their host.
pub(crate) const HOSTS: [HostHandler; 2] = [
    HostHandler {
        what: "rest-collision",
        f: host_collision,
    },
    HostHandler {
        what: "rest-collision-force-shift",
        f: host_force_shift,
    },
];

fn host_collision(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    calc_positioning_done(vm, args[1])
}

fn host_force_shift(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    force_shift_callback_rest(vm, args[1], args[2])
}

/// `(ly:rest-collision::calc-positioning-done grob)`: LilyPond's subr, a
/// primitive here so that it prints as LilyPond's; it checks its grob and
/// tail-calls `%host-call` ([`HOSTS`]).
pub(crate) fn ly_calc_positioning_done(
    cx: &mut Cx,
    args: &[Value],
) -> Result<Value, lily_engine_scheme::Thrown> {
    const SUBR: &str = "ly:rest-collision::calc-positioning-done";
    grob::grob_arg(cx, SUBR, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("rest-collision");
    Ok(cx.tail_call(f, &[what, args[0]]))
}

/// `(ly:rest-collision::force-shift-callback-rest rest [offset])`:
/// LilyPond's subr, a primitive here so that it prints as LilyPond's; it
/// checks its grob and tail-calls `%host-call` ([`HOSTS`]), an `offset`
/// not given being no number.
pub(crate) fn ly_force_shift_callback_rest(
    cx: &mut Cx,
    args: &[Value],
) -> Result<Value, lily_engine_scheme::Thrown> {
    const SUBR: &str = "ly:rest-collision::force-shift-callback-rest";
    grob::grob_arg(cx, SUBR, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("rest-collision-force-shift");
    let offset = args.get(1).copied().unwrap_or(Value::FALSE);
    Ok(cx.tail_call(f, &[what, args[0], offset]))
}
