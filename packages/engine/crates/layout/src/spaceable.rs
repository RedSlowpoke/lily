//! `spaceable-grob.cc` (part 3's, MS3 step 29): what a paper column keeps
//! of the spacing problem. Its rods are its `minimum-distances`, an alist
//! of each column to its right and the least distance to it; its springs
//! are its `ideal-distances`, a list of `(spring . column)`, the spring a
//! [`SpringObject`] (a copy, `smobbed_copy`). Its interface, with its
//! properties' list, is the image's (`define-grob-interfaces.scm`).
//!
//! The objects are read by `internal_get_object`, through the image
//! ([`grob_array::object`]), and set by `internal_set_object`
//! ([`grob::set_object`]). Two rods to one column keep the larger
//! distance, as Guile's `max` gives it. `get_ideal_distances` is declared
//! and never defined in LilyPond, so it is not here.

use alloc::format;

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::grob_array;
use crate::paper_column;
use crate::spring::{Spring, SpringObject};

/// `get_minimum_distances`: the object `minimum-distances` of `me`.
pub fn get_minimum_distances(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let sym = host.heap().intern("minimum-distances");
    grob_array::object(host, me, sym)
}

/// `add_rod`: the distance `d` to the column `p` kept on the column `me`,
/// the larger if it has one to `p`; none for a negative `d`, LilyPond's
/// programming error for an infinite one, and for a `p` ranked before `me`.
pub fn add_rod(host: &mut dyn Host, me: Value, p: Value, d: f64) -> Result<(), Exception> {
    if d < 0.0 {
        return Ok(());
    }
    if d.is_infinite() {
        grob::unlocated_programming_error(host, "infinite rod")?;
    }
    let mins = get_minimum_distances(host, me)?;
    let mut s = mins;
    while let (Some(dist), Some(rest)) = (host.heap().car(s), host.heap().cdr(s)) {
        if host.heap().car(dist) == Some(p) {
            let max = crate::lily_procedure(host, "max")?;
            let old = host.heap().cdr(dist).unwrap_or(Value::NULL);
            let newdist = host.heap().flonum(d);
            let larger = host.call(max, &[old, newdist])?;
            host.heap().set_cdr(dist, larger);
            return Ok(());
        }
        s = rest;
    }
    let heap = host.heap();
    if paper_column::rank(heap, p) < paper_column::rank(heap, me) {
        grob::unlocated_programming_error(host, "Adding reverse rod")?;
    }
    let heap = host.heap();
    let newdist = heap.flonum(d);
    let entry = heap.cons(p, newdist);
    let mins = heap.cons(entry, mins);
    let sym = heap.intern("minimum-distances");
    grob::set_object(heap, me, sym, mins);
    Ok(())
}

/// `add_spring`: a copy of `sp` to the column `other` kept on `me`.
pub fn add_spring(
    host: &mut dyn Host,
    me: Value,
    other: Value,
    sp: &Spring,
) -> Result<(), Exception> {
    let sym = host.heap().intern("ideal-distances");
    let ideal = grob_array::object(host, me, sym)?;
    let heap = host.heap();
    let spring = heap.wrap(SpringObject(*sp));
    let entry = heap.cons(spring, other);
    let ideal = heap.cons(entry, ideal);
    grob::set_object(heap, me, sym, ideal);
    Ok(())
}

/// `get_spring`: the first spring kept on `this_col` to `next_col`, else
/// LilyPond's programming error and `Spring ()`.
pub fn get_spring(
    host: &mut dyn Host,
    this_col: Value,
    next_col: Value,
) -> Result<Spring, Exception> {
    let sym = host.heap().intern("ideal-distances");
    let mut s = grob_array::object(host, this_col, sym)?;
    let heap = host.heap();
    while let (Some(entry), Some(rest)) = (heap.car(s), heap.cdr(s)) {
        if let (Some(spring), Some(col)) = (heap.car(entry), heap.cdr(entry))
            && col == next_col
            && let Some(o) = heap.foreign::<SpringObject>(spring)
        {
            return Ok(o.0);
        }
        s = rest;
    }
    let rank = paper_column::rank(heap, this_col);
    let text = format!("No spring between column {rank} and next one");
    grob::unlocated_programming_error(host, &text)?;
    Ok(Spring::default())
}
