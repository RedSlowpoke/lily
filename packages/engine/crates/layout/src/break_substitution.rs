//! `break-substitution.cc` (part 5's): a grob's links to other grobs
//! made links to grobs on its own side of a break or its own system.
//!
//! - The first pass, pulled into MS3 step 32 with the pieces of the
//!   breakable items (the user's answer at step 32's session 6): before
//!   line breaking, a piece of a breakable item takes its original's
//!   objects with each item in them replaced by that item's piece on the
//!   same side of a break ([`Criterion::Direction`]), which
//!   [`crate::item::handle_prebroken_dependencies`] asks for.
//! - The second pass, by system after line breaking (MS3 step 33):
//!   each grob on a broken system takes its objects with each grob in them
//!   replaced by its piece on that system ([`Criterion::System`],
//!   [`substitute_object_links`]), and each piece of a broken spanner its
//!   original's ([`substitute_one_mutable_property`], with
//!   `fast_substitute_grob_array` for long unordered grob arrays), which
//!   `Grob::handle_broken_dependencies` (part 4 of step 33) asks for.
//!
//! `substitute_grob` (`:52–81`), `do_break_substitution` (`:98–153`),
//! `fast_substitute_grob_array` (`:220–295`), `substitute_object_alist`
//! (`:311–332`), `substitute_one_mutable_property` (`:334–348`) and
//! `Grob::substitute_object_links` (`:350–360`).

use alloc::vec::Vec;

use lily_engine_music::grob_of;
use lily_engine_scheme::{Heap, Value};

use crate::geometry::{Direction, X, Y};
use crate::grob::{self, with_grob_mut};
use crate::grob_array::GrobArray;
use crate::item;
use crate::spanner;

/// What a grob is substituted by: its piece on a side of a break, before
/// line breaking, or its piece on a broken system, after it.
#[derive(Clone, Copy)]
pub enum Criterion {
    /// A break direction, -1 or 1 (0 for an item that is no piece).
    Direction(Direction),
    /// A broken system.
    System(Value),
}

/// `substitute_grob`: by the direction `d`, an item broken on another
/// side than `d` replaced by its piece on `d`, none if it has none, and
/// any other grob itself; by the system `line`, the grob if it is on
/// `line`, else its piece on `line`, kept only if it has `line` among its
/// ancestors on both axes. A grob of no class, which LilyPond does not
/// have, has no piece.
fn substitute_grob(heap: &mut Heap, crit: Criterion, sc: Value) -> Option<Value> {
    match crit {
        Criterion::Direction(d) => {
            if item::is_item(heap, sc) && item::break_status_dir(heap, sc) != d {
                return item::find_prebroken_piece(heap, sc, d);
            }
            Some(sc)
        }
        Criterion::System(line) => {
            let sc = if item::get_system(heap, sc) == Some(line) {
                sc
            } else {
                grob::find_broken_piece(heap, sc, line)?
            };
            (grob::has_in_ancestry(heap, sc, line, X) && grob::has_in_ancestry(heap, sc, line, Y))
                .then_some(sc)
        }
    }
}

/// `do_break_substitution` by `crit`: `src` with each grob in it
/// substituted ([`substitute_grob`]): a grob, or `*unspecified*` if it has
/// no substitute; a grob array, a new one as ordered as `src`, without the
/// grobs that have none; a vector, a pair and the cdrs of a list, new ones
/// of their items' substitutes; anything else itself.
fn do_break_substitution(heap: &mut Heap, crit: Criterion, src: Value) -> Value {
    if grob_of(heap, src).is_some() {
        return substitute_grob(heap, crit, src).unwrap_or(Value::UNSPECIFIED);
    }
    if let Some(ga) = heap.foreign::<GrobArray>(src) {
        let (old, ordered) = (ga.grobs.clone(), ga.ordered);
        let grobs = old
            .into_iter()
            .filter_map(|g| substitute_grob(heap, crit, g))
            .collect();
        return heap.wrap(GrobArray { grobs, ordered });
    }
    if let Some(items) = heap.vector_items(src) {
        let items = items.to_vec();
        let items = items
            .into_iter()
            .map(|v| do_break_substitution(heap, crit, v))
            .collect();
        return heap.vector(items);
    }
    // A pair: the cars recursed into, the cdrs walked as a list's.
    let mut cars = Vec::new();
    let mut p = src;
    while let (Some(car), Some(cdr)) = (heap.car(p), heap.cdr(p)) {
        cars.push(do_break_substitution(heap, crit, car));
        p = cdr;
    }
    if cars.is_empty() {
        return src;
    }
    let tail = do_break_substitution(heap, crit, p);
    cars.into_iter()
        .rev()
        .fold(tail, |tail, car| heap.cons(car, tail))
}

/// `fast_substitute_grob_array` of the broken spanner `sp`: for an
/// unordered grob array `ga` of 15 grobs or more, the object `sym` of
/// each of `sp`'s pieces given the substitutes on the piece's system of
/// the items that may be on it and of all the spanners, in a grob array
/// the piece has there or a new one. The items are taken in the order of
/// the first of `sp`'s systems they are on, each piece's from the first
/// to the last that span its system, those between too; an item on none
/// of `sp`'s systems is left out. Where LilyPond indexes past its table
/// of the items each system has, for an item on none of them before the
/// first when that system's rank is not 0, which is undefined behaviour,
/// the item is left out too; and a piece past the table, which its
/// assertion forbids, gets the spanners only. False, with nothing set,
/// for a smaller or ordered array.
///
/// The piece's slot is read as it is set (no callback is called): until
/// this pass a piece has no objects but a System's `all-elements`.
fn fast_substitute_grob_array(heap: &mut Heap, sp: Value, sym: Value, ga: Value) -> bool {
    let Some((grobs, ordered)) = heap
        .foreign::<GrobArray>(ga)
        .map(|ga| (ga.grobs.clone(), ga.ordered))
    else {
        return false;
    };
    if ordered || grobs.len() < 15 {
        return false;
    }
    let (first, last) = spanner::spanned_system_rank_interval(heap, sp);
    let length = last.saturating_sub(first);

    // `Substitution_entry`s: each item with the ranks, from `first`, of
    // `sp`'s systems it is on.
    let mut items = Vec::new();
    let mut spanners = Vec::new();
    for g in grobs {
        if item::is_item(heap, g) {
            let (l, r) = item::spanned_system_rank_interval(heap, g);
            let (l, r) = (l.max(first), r.min(last));
            if l <= r {
                items.push((g, l - first, r - first));
            }
        } else {
            spanners.push(g);
        }
    }
    items.sort_by_key(|e| e.1);

    // Each of `sp`'s systems, the first and the last item on it.
    let mut item_indices: Vec<Option<(usize, usize)>> = alloc::vec![None; length as usize + 1];
    for (i, &(_, l, r)) in items.iter().enumerate() {
        for j in l..=r {
            let iv = &mut item_indices[j as usize];
            *iv = Some(iv.map_or((i, i), |(lo, hi)| (lo.min(i), hi.max(i))));
        }
    }

    let pieces = spanner::broken_intos(heap, sp);
    for (i, sc) in pieces.into_iter().enumerate() {
        let Some(system) = item::get_system(heap, sc) else {
            continue;
        };
        let mut newval = grob::object_data(heap, sc, sym);
        if heap.foreign::<GrobArray>(newval).is_none() {
            newval = heap.wrap(GrobArray::new());
            grob::set_object(heap, sc, sym, newval);
        }
        let range = item_indices.get(i).copied().flatten();
        let candidates = range
            .map_or(&[][..], |(lo, hi)| &items[lo..=hi])
            .iter()
            .map(|e| e.0)
            .chain(spanners.iter().copied())
            .collect::<Vec<_>>();
        let found: Vec<Value> = candidates
            .into_iter()
            .filter_map(|og| substitute_grob(heap, Criterion::System(system), og))
            .collect();
        if let Some(new_array) = heap.foreign_mut::<GrobArray>(newval) {
            new_array.grobs.extend(found);
        }
    }
    true
}

/// `substitute_object_alist` by `crit`: each entry of `alist` with its
/// value substituted ([`do_break_substitution`]), in order, an entry
/// whose value has no substitute left out.
fn substitute_object_alist(heap: &mut Heap, crit: Criterion, alist: Value) -> Value {
    let mut entries = Vec::new();
    let mut s = alist;
    while let (Some(entry), Some(rest)) = (heap.car(s), heap.cdr(s)) {
        if let (Some(sym), Some(val)) = (heap.car(entry), heap.cdr(entry)) {
            let val = do_break_substitution(heap, crit, val);
            if val != Value::UNSPECIFIED {
                entries.push((sym, val));
            }
        }
        s = rest;
    }
    entries
        .into_iter()
        .rev()
        .fold(Value::NULL, |tail, (sym, val)| {
            let entry = heap.cons(sym, val);
            heap.cons(entry, tail)
        })
}

/// `substitute_one_mutable_property` of the broken spanner `sp`, its
/// object `sym` whose value is `val`: each of its pieces given `val` with
/// each grob in it substituted by its piece on the piece's system
/// ([`do_break_substitution`]), or, for a long unordered grob array,
/// [`fast_substitute_grob_array`]'s. A piece on no system, for which
/// LilyPond's `substitute_grob` asserts, is left as it is, here and
/// there.
pub fn substitute_one_mutable_property(heap: &mut Heap, sp: Value, sym: Value, val: Value) {
    if heap.foreign::<GrobArray>(val).is_some() && fast_substitute_grob_array(heap, sp, sym, val) {
        return;
    }
    for sc in spanner::broken_intos(heap, sp) {
        if let Some(system) = item::get_system(heap, sc) {
            let newval = do_break_substitution(heap, Criterion::System(system), val);
            grob::set_object(heap, sc, sym, newval);
        }
    }
}

/// `Grob::substitute_object_links`: the objects of the grob `me` made
/// `orig`, an object alist, substituted by `crit`
/// ([`substitute_object_alist`]).
pub fn substitute_object_links(heap: &mut Heap, me: Value, crit: Criterion, orig: Value) {
    let objects = substitute_object_alist(heap, crit, orig);
    with_grob_mut(heap, me, |g| g.objects = objects);
}
