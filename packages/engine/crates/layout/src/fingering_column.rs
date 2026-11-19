//! `fingering-column.cc` (part 4's, MS3 step 53, 183 lines): the fingerings
//! set beside a chord kept apart and flush. `add_fingering` (`:162–168`),
//! which `Fingering_column_engraver` calls, and
//! `ly:fingering-column::calc-positioning-done` (`:50–63`), the music
//! crate's placeholder until then (D67), over `do_y_positioning`
//! (`:65–119`) and `do_x_positioning` (`:121–160`): Rust under a primitive
//! that prints as LilyPond's subr and calls it over `%host-call` (D127,
//! [`HOSTS`]), in 05 §5.1's callback table (`callbacks.rs`) too.
//!
//! LilyPond sorts the fingerings with `std::sort`, which is not stable;
//! they are sorted stably here. Only fingerings at the same staff position,
//! or at the same X offset, could be put in another order.

use alloc::vec;
use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::direction::get_grob_direction;
use crate::geometry::{Axis, Direction, Interval, X, Y};
use crate::spacing_options::double_or;
use crate::staff_symbol_referencer as ssr;
use crate::{grob, grob_array};

/// `add_fingering`: `f` one of the column `fc`'s `fingerings`, the column
/// its X parent and its `Y-offset` `ly:grob::x-parent-positioning`.
pub fn add_fingering(host: &mut dyn Host, fc: Value, f: Value) -> Result<(), Exception> {
    let sym = host.heap().intern("fingerings");
    grob_array::add_grob(host, fc, sym, f)?;
    grob::set_parent(host.heap(), f, fc, X);
    let proc = crate::lily_procedure(host, "ly:grob::x-parent-positioning")?;
    grob::set_property(host.heap(), f, "Y-offset", proc);
    Ok(())
}

/// `calc_positioning_done`: the live column's fingerings stacked on Y,
/// then snapped on X.
pub fn calc_positioning_done(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    if !grob::is_live_grob(host.heap(), me) {
        return Ok(Value::TRUE);
    }

    grob::set_property(host.heap(), me, "positioning-done", Value::TRUE);

    do_y_positioning(host, me)?;
    do_x_positioning(host, me)?;

    Ok(Value::TRUE)
}

/// `minmax`: whichever of `a` and `b` is further along `d`, `a` at a tie
/// (`std::max`, `std::min`).
fn minmax(d: Direction, a: f64, b: f64) -> f64 {
    if d > 0 {
        if a < b { b } else { a }
    } else if b < a {
        b
    } else {
        a
    }
}

/// `Grob::parent_relative`: the coordinate of the grob's parent on `a`
/// relative to `refp`, 0 for none.
fn parent_relative(host: &mut dyn Host, g: Value, refp: Value, a: Axis) -> Result<f64, Exception> {
    match grob::get_parent(host.heap(), g, a) {
        Some(p) => grob::relative_coordinate(host, p, refp, a),
        None => Ok(0.0),
    }
}

/// `translate_axis`, LilyPond's programming error for an infinite one.
fn translate(host: &mut dyn Host, g: Value, y: f64, a: Axis) -> Result<(), Exception> {
    if let Err(text) = grob::translate_axis(host.heap(), g, y, a) {
        grob::programming_error(host, g, text)?;
    }
    Ok(())
}

/// `do_y_positioning`: the fingerings, from bottom to top by their pure
/// staff positions, stacked from the bottom up and from the top down, a
/// fingering moved by its `padding` (0.2 if it is no number) and its
/// height off the one before it where they meet on X; each put at the
/// average of the two. LilyPond's programming error for a column of fewer
/// than two.
fn do_y_positioning(host: &mut dyn Host, me: Value) -> Result<(), Exception> {
    let const_fingerings = grob_array::extract_grob_array(host, me, "fingerings")?;

    if const_fingerings.len() < 2 {
        return grob::programming_error(
            host,
            me,
            "This FingeringColumn should have never been created.",
        );
    }

    let heap = host.heap();
    let common_x = grob::common_refpoint_of_array(heap, &const_fingerings, Some(me), X);
    let common_y = grob::common_refpoint_of_array(heap, &const_fingerings, Some(me), Y);
    let (common_x, common_y) = (
        common_x.unwrap_or(Value::NULL),
        common_y.unwrap_or(Value::NULL),
    );

    let padding = grob::property(host, me, "padding")?;
    let padding = double_or(host, padding, 0.2);

    // order the fingerings from bottom to top
    let mut keyed = Vec::with_capacity(const_fingerings.len());
    for &f in &const_fingerings {
        keyed.push((ssr::get_position(host, f, true)?, f));
    }
    keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(core::cmp::Ordering::Equal));
    let fingerings: Vec<Value> = keyed.into_iter().map(|(_, f)| f).collect();

    let n = fingerings.len();
    let mut shift = vec![0.0; n];

    // Try stacking the fingerings top-to-bottom, and then bottom-to-top.
    // Use the average of the resulting stacked locations as the final
    // positions.
    for d in [1, -1] {
        let mut stack_end = -f64::from(d) * f64::INFINITY;
        let mut prev_x_ext = Interval::EMPTY;
        let order: Vec<usize> = if d > 0 {
            (0..n).collect()
        } else {
            (0..n).rev().collect()
        };
        for i in order {
            let f = fingerings[i];
            let x_ext = grob::robust_relative_extent(host, f, common_x, X)?;
            let y_ext = grob::robust_relative_extent(host, f, f, Y)?;
            let parent_y = parent_relative(host, f, common_y, Y)?;

            // Checking only between sequential neighbors, seems good enough
            let mut meet = x_ext;
            meet.intersect(prev_x_ext);
            if !meet.is_empty() {
                stack_end += f64::from(d) * (y_ext.length() + padding);
            }
            // minmax() returns whichever is further along in direction d
            stack_end = minmax(d, stack_end, parent_y);

            shift[i] += 0.5 * (stack_end - y_ext.at(d) - parent_y);

            prev_x_ext = x_ext;
        }
    }

    for (i, &f) in fingerings.iter().enumerate() {
        translate(host, f, shift[i], Y)?;
    }
    Ok(())
}

/// `do_x_positioning`: the fingerings by their X offsets, from the right
/// for a column on the right; each within the column's `snap-radius` (0.3
/// if it is no number) of the one before it, but not at it, moved there.
fn do_x_positioning(host: &mut dyn Host, me: Value) -> Result<(), Exception> {
    let fingerings = grob_array::extract_grob_array(host, me, "fingerings")?;
    let Some(&first) = fingerings.first() else {
        return Ok(());
    };

    let common_x = grob::common_refpoint_of_array(host.heap(), &fingerings, Some(me), X)
        .unwrap_or(Value::NULL);

    let snap = grob::property(host, me, "snap-radius")?;
    let snap = double_or(host, snap, 0.3);
    let mut fos = Vec::with_capacity(fingerings.len());
    for &f in &fingerings {
        fos.push((f, grob::relative_coordinate(host, f, common_x, X)?));
    }

    fos.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal));
    let dir = get_grob_direction(host, first)?;
    if dir == 1 {
        fos.reverse();
    }

    let mut prev = f64::INFINITY * f64::from(dir);
    const EPS: f64 = 1.0e-5;
    for fo in &mut fos {
        let d = (fo.1 - prev).abs();
        if d < snap && d > EPS {
            fo.1 = prev;
        }
        prev = fo.1;
    }

    for (f, offset) in fos {
        let c = grob::relative_coordinate(host, f, common_x, X)?;
        translate(host, f, offset - c, X)?;
    }
    Ok(())
}

// --- the callback -----------------------------------------------------------

/// What `%host-call` runs for the `ly:fingering-column::` primitive
/// (D127): the callback its second argument names, of the grob given,
/// with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "fingering-column",
    f: host_fingering_column,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 1] =
    [("calc-positioning-done", calc_positioning_done)];

fn host_fingering_column(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such fingering column callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:fingering-column::calc-positioning-done grob)`: LilyPond's subr, a
/// primitive here so that it prints as LilyPond's. It checks its grob under
/// the method's name, as LilyPond's `LY_ASSERT_SMOB` gives it, and
/// tail-calls `%host-call` ([`HOSTS`]).
pub(crate) fn ly_calc_positioning_done(cx: &mut Cx, args: &[Value]) -> R {
    grob::grob_arg(cx, "ly:calc-positioning-done", 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("fingering-column");
    let name = cx.heap_mut().intern("calc-positioning-done");
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}
