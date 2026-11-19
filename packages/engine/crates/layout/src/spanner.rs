//! `spanner.cc` and `spanner-scheme.cc` (MS3 step 15): a spanner's bounds
//! and the slots of its broken pieces, `spanner-scheme.cc`'s five and
//! three of `spanner.cc`'s callbacks.
//!
//! - A spanner is a grob of the class `Spanner` or `System` (`item.rs`).
//!   Its bounds (`spanned_drul_`) are kept with its grob
//!   (`Grob::bounds`): `get_bound` ([`get_bound`]), a sticky spanner's
//!   falling back on its `sticky-host`'s; `set_bound` ([`set_bound`]), an
//!   item for a spanner, a column for a spanner or a system, which takes
//!   the spanner in its `bounded-by-me`, the left bound the spanner's
//!   parent on X unless that is a spanner, LilyPond's programming error for
//!   another grob; `add_bound_item`, `get_system` (its bounds' system) and
//!   `spanned_column_rank_interval` (its bounds' columns' ranks).
//! - Its broken pieces (`broken_intos_`, `Grob::broken_intos`) and a
//!   piece's `break_index_`, which breaking fills
//!   ([`do_break_processing`], MS3 step 33, with the copy constructor's
//!   `clone`, [`clone`]): `is_broken`, `find_broken_piece` and
//!   `broken_neighbor`.
//! - `ly:spanner-bound`, `ly:spanner-set-bound!`, `ly:spanner-broken-into`,
//!   `ly:spanner-broken-neighbor` and `ly:spanner?`.
//! - `ly:spanner::calc-normalized-endpoints`, `::kill-zero-spanned-time`
//!   and `::set-spacing-rods`, which were the music crate's placeholders
//!   (D67), in [`PRELUDE`], since they read properties through their
//!   callbacks, with `spanner_length` and `spanned_time` (`item.cc`'s
//!   `spanned_time_interval`). `set-spacing-rods` makes its rods in Rust
//!   ([`set_spacing_rods`], MS3 step 33), through `%host-call` (D127).
//!   LilyPond does not check the grob of the first two; the engine's
//!   calls throw `wrong-type-arg` for what is no spanner where LilyPond's
//!   crash.
//!
//! - The pure property cache (`spanner.cc:594–619`, part 2's, ported first
//!   by MS3 step 28): [`get_cached_pure_property`] and
//!   [`cache_pure_property`], kept with its grob (`Grob::pure_cache`).
//!
//! - What breaking reads of a spanner (MS3 step 33): `set_my_columns`
//!   ([`set_my_columns`]), its bounds moved to their pieces, and
//!   `spanned_system_rank_interval` ([`spanned_system_rank_interval`]),
//!   the ranks of its system or its pieces' (`system.rs`' `get_rank`).
//!
//! Not here: `ly:spanner::bounds-width`, which no init file names. A
//! sticky spanner's programming error is given by [`get_bound`] and
//! `ly:spanner-bound`; `get_system` and `spanned_column_rank_interval`,
//! which `item.rs` reads without a host, do not give it.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;

use lily_engine_music::{Host, Moment, Rational, clone_grob, grob_of};
use lily_engine_scheme::{Cx, Exception, Heap, HostHandler, Thrown, Value, Vm};

use crate::R;
use crate::geometry::{Direction, X, Y};
use crate::grob::{self, with_grob, with_grob_mut};
use crate::grob_array;
use crate::item::{self, Class, VPOS};
use crate::paper_column;
use crate::rod::Rod;
use crate::spacing_options::double_or;
use crate::system;

/// The programming error of a sticky spanner without a spanner for a host.
const STICKY: &str = "sticky spanner's host is not a spanner";

/// `spanned_drul_`'s index of `d`: `LEFT` (and `CENTER`) 0, `RIGHT` 1.
fn side(d: Direction) -> usize {
    usize::from(d > 0)
}

/// Whether `grob` is a `Spanner` (`unsmob<Spanner>`): a spanner or a
/// system.
pub fn is_spanner(heap: &mut Heap, grob: Value) -> bool {
    item::class(heap, grob).is_some_and(Class::is_spanner)
}

/// `get_bound` without its programming error: the bound at `d`, or a
/// sticky spanner's host's; `Err` with the spanner at which LilyPond gives
/// [`STICKY`].
fn bound(heap: &mut Heap, sp: Value, d: Direction) -> Result<Option<Value>, Value> {
    if let Some(b) = with_grob(heap, sp, |g| g.bounds[side(d)]).flatten() {
        return Ok(Some(b));
    }
    if !grob::has_interface(heap, sp, "sticky-grob-interface") {
        return Ok(None);
    }
    let sym = heap.intern("sticky-host");
    let host = grob::object_data(heap, sp, sym);
    if is_spanner(heap, host) {
        bound(heap, host, d)
    } else {
        Err(sp)
    }
}

/// `get_bounds`, read without a host: what has no bound at a side, or
/// whose sticky host is no spanner, none there.
pub(crate) fn bounds(heap: &mut Heap, sp: Value) -> [Option<Value>; 2] {
    [-1, 1].map(|d| bound(heap, sp, d).unwrap_or(None))
}

/// `get_bound`: the spanner's bound at `d`, an item, if it has one; a
/// sticky spanner without one has its `sticky-host`'s, or LilyPond's
/// programming error if the host is no spanner.
pub fn get_bound(host: &mut dyn Host, sp: Value, d: Direction) -> Result<Option<Value>, Exception> {
    match bound(host.heap(), sp, d) {
        Ok(b) => Ok(b),
        Err(at) => grob::programming_error(host, at, STICKY).map(|()| None),
    }
}

/// `internal_set_as_bound_of_spanner`: whether `g` is taken as a bound of
/// `sp`, and whether it is a column. An item by a spanner but a system
/// (`accepts_as_bound_item`), a column by both
/// (`accepts_as_bound_paper_column`), nothing else.
fn accepts(heap: &mut Heap, sp: Value, g: Value) -> Option<bool> {
    match item::class(heap, g)? {
        Class::Item => (item::class(heap, sp) != Some(Class::System)).then_some(false),
        Class::PaperColumn => Some(true),
        Class::Spanner | Class::System => None,
    }
}

/// `set_bound`'s programming error at `sp` for a bound it does not take.
fn refused(heap: &mut Heap, sp: Value, g: Value) -> String {
    let (g, sp) = (grob::grob_name(heap, g), grob::grob_name(heap, sp));
    format!("cannot set {g} as bound of {sp}")
}

/// What `set_bound` sets once `g` is taken: the bound, and, at the left of
/// a spanner that is no system, `g` its parent on X unless that is a
/// spanner, which is broken at line breaks too.
fn put(heap: &mut Heap, sp: Value, d: Direction, g: Value) {
    if let Some((arena, id)) = grob_of(heap, sp) {
        let op = lily_engine_music::Op::Bound(side(d) as u8, g);
        lily_engine_music::grob_wrote(&arena, Some(id), op, None);
    }
    with_grob_mut(heap, sp, |s| s.bounds[side(d)] = Some(g));
    if d == -1 && item::class(heap, sp) != Some(Class::System) {
        let parent = grob::get_parent(heap, sp, X);
        if !parent.is_some_and(|p| is_spanner(heap, p)) {
            grob::set_parent(heap, sp, g, X);
        }
    }
}

/// A bound the old run's translator set, set again (N8's replay, MS6 step
/// 7c): its parent and the column's `bounded-by-me` are writes of their own.
pub(crate) fn set_bound_replayed(heap: &mut Heap, sp: Value, side: usize, g: Value) {
    if let Some((arena, id)) = grob_of(heap, sp) {
        let op = lily_engine_music::Op::Bound(side as u8, g);
        lily_engine_music::grob_wrote(&arena, Some(id), op, None);
    }
    with_grob_mut(heap, sp, |s| s.bounds[side] = Some(g));
}

/// `set_bound`: `g` the spanner's bound at `d`, if it takes it, a column
/// taking the spanner in its `bounded-by-me`; else LilyPond's programming
/// error at the spanner.
pub fn set_bound(host: &mut dyn Host, sp: Value, d: Direction, g: Value) -> Result<(), Exception> {
    let Some(column) = accepts(host.heap(), sp, g) else {
        let text = refused(host.heap(), sp, g);
        return grob::programming_error(host, sp, &text);
    };
    if column {
        let sym = host.heap().intern("bounded-by-me");
        grob_array::add_grob(host, g, sym, sp)?;
    }
    put(host.heap(), sp, d, g);
    Ok(())
}

/// `add_bound_item`: `it` the spanner's left bound if it has none, else
/// its right.
pub fn add_bound_item(host: &mut dyn Host, sp: Value, it: Value) -> Result<(), Exception> {
    let d = if get_bound(host, sp, -1)?.is_none() {
        -1
    } else {
        1
    };
    set_bound(host, sp, d, it)
}

/// `get_system`: the system of both its bounds, if they have the same.
pub fn get_system(heap: &mut Heap, sp: Value) -> Option<Value> {
    let [left, right] = bounds(heap, sp);
    let system = item::get_system(heap, left?)?;
    (item::get_system(heap, right?) == Some(system)).then_some(system)
}

/// `get_cached_pure_property`: what [`cache_pure_property`] kept of the
/// spanner's `sym` between the columns `start` and `end`, if anything.
pub fn get_cached_pure_property(
    heap: &Heap,
    sp: Value,
    sym: Value,
    start: usize,
    end: usize,
) -> Option<Value> {
    with_grob(heap, sp, |g| {
        let key = (sym.to_bits(), start, end);
        g.pure_cache.get(&key).map(|&(_, v)| v)
    })
    .flatten()
}

/// `cache_pure_property`: `val` kept as the spanner's `sym` between the
/// columns `start` and `end`, in place of what was.
pub fn cache_pure_property(
    heap: &Heap,
    sp: Value,
    sym: Value,
    start: usize,
    end: usize,
    val: Value,
) {
    with_grob_mut(heap, sp, |g| {
        g.pure_cache.insert((sym.to_bits(), start, end), (sym, val));
    });
}

/// `spanned_column_rank_interval`: the ranks of its bounds' columns, 0 at
/// a side without one.
pub fn spanned_column_rank_interval(heap: &mut Heap, sp: Value) -> (u64, u64) {
    let mut iv = [0; 2];
    for (rank, b) in iv.iter_mut().zip(bounds(heap, sp)) {
        if let Some(col) = b.and_then(|b| item::get_column(heap, b)) {
            *rank = paper_column::rank(heap, col);
        }
    }
    (iv[0], iv[1])
}

/// The broken pieces of `sp`.
pub(crate) fn broken_intos(heap: &Heap, sp: Value) -> Vec<Value> {
    with_grob(heap, sp, |g| g.broken_intos.clone()).unwrap_or_default()
}

/// `is_broken`: whether the spanner has broken pieces.
pub fn is_broken(heap: &Heap, sp: Value) -> bool {
    with_grob(heap, sp, |g| !g.broken_intos.is_empty()).unwrap_or(false)
}

/// `find_broken_piece`: the piece of the spanner on the system `l`, by
/// `l`'s rank from the first piece's system's.
pub fn find_broken_piece(heap: &mut Heap, sp: Value, l: Value) -> Option<Value> {
    let pieces = broken_intos(heap, sp);
    let first = get_system(heap, *pieces.first()?)?;
    let delta = system::get_rank(heap, l).checked_sub(system::get_rank(heap, first))?;
    pieces.get(usize::try_from(delta).ok()?).copied()
}

/// `set_my_columns`: each bound on no system replaced by its prebroken
/// piece on the other side, the one that faces into the spanner. A side
/// without a bound, or a bound without that piece, which LilyPond reads
/// through, is left as it is.
pub fn set_my_columns(host: &mut dyn Host, sp: Value) -> Result<(), Exception> {
    for d in [-1, 1] {
        let Some(b) = get_bound(host, sp, d)? else {
            continue;
        };
        if item::get_system(host.heap(), b).is_none()
            && let Some(piece) = item::find_prebroken_piece(host.heap(), b, -d)
        {
            set_bound(host, sp, d, piece)?;
        }
    }
    Ok(())
}

/// `spanned_system_rank_interval`: the rank of the spanner's system
/// twice; else, if it is broken, its first and its last piece's systems'
/// ranks; else the empty `System_rank_interval`, `(VPOS . 0)`, which a
/// piece on no system, which LilyPond reads through, gives too.
pub fn spanned_system_rank_interval(heap: &mut Heap, sp: Value) -> (u64, u64) {
    if let Some(st) = get_system(heap, sp) {
        let rank = system::get_rank(heap, st);
        return (rank, rank);
    }
    let pieces = broken_intos(heap, sp);
    let (Some(&front), Some(&back)) = (pieces.first(), pieces.last()) else {
        return (VPOS, 0);
    };
    match (get_system(heap, front), get_system(heap, back)) {
        (Some(first), Some(last)) => (system::get_rank(heap, first), system::get_rank(heap, last)),
        _ => (VPOS, 0),
    }
}

/// `clone` of the spanner `sp`: the copy `Spanner`'s copy constructor
/// makes over `Grob`'s ([`grob::copied`]), with no bounds, pieces or pure
/// cache of its own, as a new grob has. The spanner is made first
/// ([`grob::make`]), as an item is when it is copied (`item::clone`).
/// `None` for what is not a grob.
pub(crate) fn clone(host: &mut dyn Host, sp: Value) -> Option<Value> {
    grob::make(host, sp);
    let heap = host.heap();
    let copy = clone_grob(heap, sp)?;
    grob::copied(heap, copy, sp);
    Some(copy)
}

/// `do_break_processing` (`spanner.cc:35–145`): a live spanner with both
/// bounds, on no system and not broken yet, broken into a piece a
/// system. Spanning one column, a piece bounded twice by each of the
/// column's prebroken pieces that is on a system (LilyPond's programming
/// error, at the spanner, for a column without one), since it might be
/// another grob's parent; else a piece between each two of its bounds and
/// the root system's breakable columns between them (`broken_col_range`,
/// `system.rs`), each bound on no system replaced by its piece facing
/// into the piece, and the piece left out, with LilyPond's programming
/// errors, if a bound has no such piece or is outside the columns its
/// spanners parents span, and killed if its bounds are on no system or
/// on two. Each piece typeset in its system and its pieces numbered by
/// their systems' ranks. A spanner without a root system, which LilyPond
/// reads through, has no breakable column between its bounds; a bound
/// without a column, which LilyPond reads through, is outside.
pub fn do_break_processing(host: &mut dyn Host, me: Value) -> Result<(), Exception> {
    let Some(left) = get_bound(host, me, -1)? else {
        return Ok(());
    };
    let Some(right) = get_bound(host, me, 1)? else {
        return Ok(());
    };
    let heap = host.heap();
    if !grob::is_live_grob(heap, me) || get_system(heap, me).is_some() || is_broken(heap, me) {
        return Ok(());
    }
    // The pieces are rooted until they are the spanner's.
    let depth = host.heap().root_depth();
    let made = break_pieces(host, me, left, right);
    host.heap().unroot_to(depth);
    let mut pieces = made?;
    let heap = host.heap();
    // `Spanner::less`; the pieces' systems differ, so the sort's
    // stability does not matter.
    pieces.sort_by_key(|&p| get_system(heap, p).map_or(0, |s| system::get_rank(heap, s)));
    for (i, &p) in pieces.iter().enumerate() {
        with_grob_mut(heap, p, |g| g.break_index = i);
    }
    with_grob_mut(heap, me, |g| g.broken_intos = pieces);
    Ok(())
}

/// The pieces [`do_break_processing`] makes of `me`, bounded by `left`
/// and `right`, unsorted, each rooted.
fn break_pieces(
    host: &mut dyn Host,
    me: Value,
    left: Value,
    right: Value,
) -> Result<Vec<Value>, Exception> {
    let mut pieces = Vec::new();
    if left == right {
        // If we have a spanner spanning one column, we must break it
        // anyway because it might provide a parent for another item.
        for d in [-1, 1] {
            let Some(bound) = item::find_prebroken_piece(host.heap(), left, d) else {
                grob::programming_error(host, me, "no broken bound")?;
                continue;
            };
            if let Some(sys) = item::get_system(host.heap(), bound)
                && let Some(span) = piece(host, me, [bound, bound])?
            {
                system::typeset_grob(host, sys, span)?;
                pieces.push(span);
            }
        }
        return Ok(pieces);
    }

    let mut break_points = alloc::vec![left];
    if let Some(root) = system::get_root_system(host.heap(), me) {
        break_points.extend(system::broken_col_range(host, root, left, right)?);
    }
    break_points.push(right);

    // Check if our parent in X-direction spans equally wide or wider than
    // we do.
    let mut parent_rank_slice = (0, VPOS);
    for a in [X, Y] {
        let heap = host.heap();
        if let Some(parent) = grob::get_parent(heap, me, a).filter(|&p| is_spanner(heap, p)) {
            let (l, r) = spanned_column_rank_interval(heap, parent);
            parent_rank_slice = (parent_rank_slice.0.max(l), parent_rank_slice.1.min(r));
        }
    }

    for w in break_points.windows(2) {
        let mut bounds = [Some(w[0]), Some(w[1])];
        for (b, d) in bounds.iter_mut().zip([-1, 1]) {
            if let Some(it) = *b
                && item::get_system(host.heap(), it).is_none()
            {
                *b = item::find_prebroken_piece(host.heap(), it, -d);
            }
        }
        let [Some(l), Some(r)] = bounds else {
            grob::programming_error(host, me, "bounds of this piece aren't breakable.")?;
            continue;
        };

        let heap = host.heap();
        let ok = [l, r].into_iter().all(|b| {
            item::get_column(heap, b).is_some_and(|c| {
                let rank = paper_column::rank(heap, c);
                parent_rank_slice.0 <= rank && rank <= parent_rank_slice.1
            })
        });
        if !ok {
            let name = grob::grob_name(heap, me);
            let text = format!(
                "Spanner `{name}' is not fully contained in parent spanner.  Ignoring orphaned part"
            );
            grob::programming_error(host, me, &text)?;
            continue;
        }

        let Some(span) = piece(host, me, [l, r])? else {
            continue;
        };
        let heap = host.heap();
        match (item::get_system(heap, l), item::get_system(heap, r)) {
            (Some(ls), Some(rs)) if ls == rs => {
                system::typeset_grob(host, ls, span)?;
                pieces.push(span);
            }
            _ => {
                grob::programming_error(host, me, "bounds of spanner are invalid")?;
                grob::suicide(host.heap(), span);
            }
        }
    }
    Ok(pieces)
}

/// A copy of `me` ([`clone`]), rooted, bounded by `bounds`.
fn piece(host: &mut dyn Host, me: Value, bounds: [Value; 2]) -> Result<Option<Value>, Exception> {
    let Some(span) = clone(host, me) else {
        return Ok(None);
    };
    host.heap().root(span);
    set_bound(host, span, -1, bounds[0])?;
    set_bound(host, span, 1, bounds[1])?;
    Ok(Some(span))
}

/// `broken_neighbor`: the piece of the spanner's original before (`d` -1)
/// or after (1) it, if it is a piece and has one.
pub fn broken_neighbor(heap: &mut Heap, sp: Value, d: Direction) -> Option<Value> {
    let (original, k) = with_grob(heap, sp, |g| (g.original, g.break_index))?;
    let j = usize::try_from(k as i64 + i64::from(d)).ok()?;
    broken_intos(heap, original?).get(j).copied()
}

// --- the Scheme face (spanner-scheme.cc) -----------------------------------

/// `LY_ASSERT_SMOB (Spanner, v, position)` of `subr`.
pub(crate) fn spanner_arg(
    cx: &mut Cx,
    subr: &str,
    position: usize,
    v: Value,
) -> Result<(), Thrown> {
    if grob_of(cx.heap(), v).is_some() {
        grob::grob_arg(cx, subr, position, v)?;
        if is_spanner(cx.heap_mut(), v) {
            return Ok(());
        }
    }
    Err(cx.wrong_type(subr, position, v, "Spanner"))
}

/// `(ly:spanner-bound spanner dir [def])`: its bound at `dir`, else `def`
/// or `'()`.
pub(crate) fn ly_spanner_bound(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:spanner-bound";
    spanner_arg(cx, SUBR, 1, args[0])?;
    let d = crate::direction_arg(cx, SUBR, 2, args[1])?;
    let def = args.get(2).copied().unwrap_or(Value::NULL);
    match bound(cx.heap_mut(), args[0], d) {
        Ok(b) => Ok(b.unwrap_or(def)),
        Err(at) => grob::reported(cx, at, "programming-error", def, &[STICKY.into()]),
    }
}

/// `(%spanner-set-bound! spanner dir item)`: `ly:spanner-set-bound!`'s
/// checks and [`set_bound`] but the column's `bounded-by-me`, which
/// [`PRELUDE`] adds where this gives `#t`.
pub(crate) fn set_bound_prim(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:spanner-set-bound!";
    spanner_arg(cx, SUBR, 1, args[0])?;
    let d = crate::direction_arg(cx, SUBR, 2, args[1])?;
    item::item_arg(cx, SUBR, 3, args[2])?;
    let heap = cx.heap_mut();
    let Some(column) = accepts(heap, args[0], args[2]) else {
        let text = refused(heap, args[0], args[2]);
        return grob::reported(cx, args[0], "programming-error", Value::FALSE, &[text]);
    };
    put(heap, args[0], d, args[2]);
    Ok(Value::bool(column))
}

/// `(ly:spanner-broken-into spanner)`: its pieces, in order.
pub(crate) fn ly_spanner_broken_into(cx: &mut Cx, args: &[Value]) -> R {
    spanner_arg(cx, "ly:spanner-broken-into", 1, args[0])?;
    let pieces = broken_intos(cx.heap(), args[0]);
    Ok(cx.heap_mut().list(&pieces))
}

/// `(ly:spanner-broken-neighbor spanner dir)`: [`broken_neighbor`], or
/// `#f`.
pub(crate) fn ly_spanner_broken_neighbor(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:spanner-broken-neighbor";
    spanner_arg(cx, SUBR, 1, args[0])?;
    let d = crate::direction_arg(cx, SUBR, 2, args[1])?;
    Ok(broken_neighbor(cx.heap_mut(), args[0], d).unwrap_or(Value::FALSE))
}

/// `(ly:spanner? g)`.
pub(crate) fn ly_spanner_p(cx: &mut Cx, args: &[Value]) -> R {
    if grob_of(cx.heap(), args[0]).is_none() {
        return Ok(Value::FALSE);
    }
    grob::grob_arg(cx, "ly:spanner?", 1, args[0])?;
    Ok(Value::bool(is_spanner(cx.heap_mut(), args[0])))
}

/// A column of `(%spanner-cached-pure-property …)`'s, an exact integer
/// from 0 ([`PRELUDE`]s give no other); one past `i64` is `usize::MAX`.
fn column_of(heap: &Heap, v: Value) -> usize {
    heap.to_i64(v).map_or(usize::MAX, |n| n as usize)
}

/// `(%spanner-cached-pure-property spanner sym start end)`:
/// [`get_cached_pure_property`], or `'()`.
pub(crate) fn cached_pure_property(cx: &mut Cx, args: &[Value]) -> R {
    spanner_arg(cx, "%spanner-cached-pure-property", 1, args[0])?;
    let heap = cx.heap();
    let (start, end) = (column_of(heap, args[2]), column_of(heap, args[3]));
    Ok(get_cached_pure_property(heap, args[0], args[1], start, end).unwrap_or(Value::NULL))
}

/// `(%spanner-cache-pure-property! spanner sym start end val)`:
/// [`cache_pure_property`].
pub(crate) fn cache_pure_property_prim(cx: &mut Cx, args: &[Value]) -> R {
    spanner_arg(cx, "%spanner-cache-pure-property!", 1, args[0])?;
    let heap = cx.heap();
    let (start, end) = (column_of(heap, args[2]), column_of(heap, args[3]));
    cache_pure_property(heap, args[0], args[1], start, end, args[4]);
    Ok(Value::UNSPECIFIED)
}

/// `(%spanner-break-index spanner)`: `get_break_index`.
pub(crate) fn break_index(cx: &mut Cx, args: &[Value]) -> R {
    let k = with_grob(cx.heap(), args[0], |g| g.break_index).unwrap_or(0);
    Ok(cx.heap_mut().integer(k as i64))
}

/// An end of `spanned_time_interval`'s interval: a moment, or the empty
/// interval's infinity on its side.
#[derive(Clone, Copy)]
enum End {
    At(Moment),
    Infinite(Direction),
}

impl End {
    /// `<=`, as `Interval_t<Moment>::length` compares its ends.
    fn le(self, other: End) -> bool {
        match (self, other) {
            (End::Infinite(a), End::Infinite(b)) => a <= b,
            (End::Infinite(a), End::At(_)) => a < 0,
            (End::At(_), End::Infinite(b)) => b > 0,
            (End::At(a), End::At(b)) => a.compare(b) != Ordering::Greater,
        }
    }
}

/// `(%zero-spanned-time? left right)`: whether `kill_zero_spanned_time`
/// finds the spanned time of no length. Each of `left` and `right` is
/// `'()` for a bound without a column, else the list of its column's
/// `when`. `spanned_time_interval`: an end the `when` that is a moment,
/// else the empty interval's; at a side without a column, the other
/// side's. The left end's grace part then 0.
pub(crate) fn zero_spanned_time(cx: &mut Cx, args: &[Value]) -> R {
    let heap = cx.heap();
    let mut iv = [End::Infinite(1), End::Infinite(-1)];
    let mut column = [false; 2];
    for (i, &v) in args.iter().enumerate() {
        if let Some(when) = heap.car(v) {
            column[i] = true;
            if let Some(&m) = heap.foreign::<Moment>(when) {
                iv[i] = End::At(m);
            }
        }
    }
    for i in 0..2 {
        if !column[i] {
            iv[i] = iv[1 - i];
        }
    }
    if let End::At(m) = &mut iv[0] {
        m.grace = Rational::ZERO;
    }
    Ok(Value::bool(iv[1].le(iv[0])))
}

/// `spanner_length` (`spanner.cc`, MS4 step 4, for the beam's quanting):
/// the length of `X-positions`, else of the bounds' `X` in their bound
/// infos, else of the bounds' coordinates; LilyPond's programming error
/// for a negative one.
pub fn spanner_length(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    let v = grob::property(host, me, "X-positions")?;
    let mut lr =
        grob::number_pair(host.heap(), v).unwrap_or(crate::geometry::Interval::new(1.0, -1.0));

    if lr.is_empty() {
        let bounds = [
            grob::property(host, me, "left-bound-info")?,
            grob::property(host, me, "right-bound-info")?,
        ];

        for (i, d) in [-1, 1].into_iter().enumerate() {
            let heap = host.heap();
            let sym = heap.intern("X");
            let x = grob::assq_ref(heap, sym, bounds[i]).unwrap_or(Value::FALSE);
            *lr.at_mut(d) = double_or(host, x, f64::from(-d));
        }
    }

    if lr.is_empty() {
        for d in [-1, 1] {
            *lr.at_mut(d) = match get_bound(host, me, d)? {
                Some(b) => grob::relative_coordinate(host, b, Value::NULL, X)?,
                None => 0.0,
            };
        }
    }

    if lr.is_empty() {
        grob::programming_error(host, me, "spanner with negative length")?;
    }

    Ok(lr.length())
}

/// `set_spacing_rods` (`spanner.cc:426–493`), once the grob is checked
/// ([`PRELUDE`]): with a `minimum-length` or a
/// `minimum-length-after-break` and both bounds, the rods of the
/// spanner's length. If the root system has breakable columns between
/// the bounds' columns on no system yet (`System::broken_col_range`), a
/// rod from the left bound to the first one's piece at the end of a line
/// and one from the last one's piece at the start of a line to the right
/// bound, `minimum-length-after-break` long; then a rod between the
/// bounds, and the same rod again to the right bound's piece at the end
/// of a line, if it has one. A length that is no real number is 0
/// (`from_scm<double>`). A spanner with no root system, where LilyPond
/// reads through a null, has no broken columns.
pub fn set_spacing_rods(host: &mut dyn Host, me: Value) -> Result<(), Exception> {
    let num_length = grob::property(host, me, "minimum-length")?;
    let broken_length = grob::property(host, me, "minimum-length-after-break")?;
    let heap = host.heap();
    if !heap.is_number(num_length) && !heap.is_number(broken_length) {
        return Ok(());
    }
    let root = system::get_root_system(heap, me);
    let left = get_bound(host, me, -1)?;
    let right = get_bound(host, me, 1)?;
    let (Some(left), Some(right)) = (left, right) else {
        return Ok(());
    };
    let num = double_or(host, num_length, 0.0);

    let cols = match root {
        Some(root) => system::broken_col_range(host, root, left, right)?,
        None => Vec::new(),
    };
    if let (Some(&first), Some(&last)) = (cols.first(), cols.last()) {
        let mut r = Rod {
            item_drul: [
                Some(left),
                item::find_prebroken_piece(host.heap(), first, -1),
            ],
            distance: num,
        };
        r.add_to_cols(host)?;

        r.item_drul = [
            item::find_prebroken_piece(host.heap(), last, 1),
            Some(right),
        ];
        if host.heap().is_number(broken_length) {
            // add_to_cols may have changed the distance: the difference is
            // added, so that it is treated as minimum-length is.
            r.distance += double_or(host, broken_length, 0.0) - num;
        }
        r.add_to_cols(host)?;
    }

    let mut r = Rod {
        item_drul: [Some(left), Some(right)],
        distance: num,
    };
    r.add_to_cols(host)?;
    // Whether the right bound will be broken is not known yet: the rod
    // goes to its piece at the end of a line too, and spacing reads only
    // one of the two.
    if let Some(left_pbp) = item::find_prebroken_piece(host.heap(), right, -1) {
        r.item_drul[1] = Some(left_pbp);
        r.add_to_cols(host)?;
    }
    Ok(())
}

/// What `%host-call` runs for [`PRELUDE`] (D127): [`set_spacing_rods`] of
/// the spanner given, with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "spanner-set-spacing-rods",
    f: host_set_spacing_rods,
}];

fn host_set_spacing_rods(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    set_spacing_rods(vm, args[1])?;
    Ok(Value::UNSPECIFIED)
}

/// `ly:spanner-set-bound!`, with the column's `bounded-by-me`; and
/// `spanner.cc`'s callbacks, which read properties through their
/// callbacks: `spanner_length`, `kill_zero_spanned_time` with
/// `spanned_time` and `calc_normalized_endpoints`; `set_spacing_rods`
/// checks its grob and calls [`set_spacing_rods`] ([`HOSTS`]).
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:spanner-set-bound! spanner dir item)
  (if (%spanner-set-bound! spanner dir item)
      (ly:pointer-group-interface::add-grob item 'bounded-by-me spanner))
  *unspecified*)
(define (%spanner-length grob)
  (define (empty? lr) (> (car lr) (cdr lr)))
  (define (bound-x info d)
    (let ((entry (and (pair? info) (assoc 'X info))))
      (exact->inexact (if (and (pair? entry) (real? (cdr entry))) (cdr entry) (- d)))))
  (let* ((xp (ly:grob-property grob 'X-positions))
         (lr (if (and (pair? xp) (real? (car xp)) (real? (cdr xp)))
                 (cons (exact->inexact (car xp)) (exact->inexact (cdr xp)))
                 '(1.0 . -1.0)))
         (lr (if (empty? lr)
                 (let* ((left (ly:grob-property grob 'left-bound-info))
                        (right (ly:grob-property grob 'right-bound-info)))
                   (cons (bound-x left -1) (bound-x right 1)))
                 lr))
         (lr (if (empty? lr)
                 (let* ((left (%grob-relative-coordinate (ly:spanner-bound grob -1) '() 0))
                        (right (%grob-relative-coordinate (ly:spanner-bound grob 1) '() 0)))
                   (cons left right))
                 lr)))
    (if (empty? lr) (%grob-programming-error grob "spanner with negative length"))
    (if (empty? lr) 0.0 (- (cdr lr) (car lr)))))
(define-public (ly:spanner::calc-normalized-endpoints grob)
  (let* ((orig (ly:grob-original grob))
         (pieces (ly:spanner-broken-into orig)))
    (if (null? pieces)
        (let ((result (cons 0.0 1.0)))
          (%grob-set-property-data! orig 'normalized-endpoints result)
          result)
        (let* ((lengths (let loop ((ps pieces) (ls '()))
                          (if (null? ps) (reverse ls)
                              (loop (cdr ps) (cons (%spanner-length (car ps)) ls)))))
               (total (let sum ((ls lengths) (t 0.0))
                        (if (null? ls) t (sum (cdr ls) (+ t (car ls))))))
               (index (%spanner-break-index grob)))
          (let loop ((ps pieces) (ls lengths) (i 0) (start 0.0) (result '()))
            (if (null? ps)
                result
                (let* ((end (+ start (car ls)))
                       (t (cons (* (/ 1.0 total) start) (* (/ 1.0 total) end))))
                  (%grob-set-property-data! (car ps) 'normalized-endpoints t)
                  (loop (cdr ps) (cdr ls) (+ i 1) end (if (= i index) t result)))))))))
(define (%spanner-bound-when grob d)
  (let* ((bound (ly:spanner-bound grob d))
         (column (if (null? bound) bound (ly:item-get-column bound))))
    (if (null? column) '() (list (ly:grob-property column 'when)))))
(define-public (ly:spanner::kill-zero-spanned-time grob)
  (if (not (eqv? 0 (ly:item-break-dir (ly:spanner-bound grob -1))))
      (let* ((left (%spanner-bound-when grob -1))
             (right (%spanner-bound-when grob 1)))
        (if (%zero-spanned-time? left right) (ly:grob-suicide! grob))))
  *unspecified*)
(define-public (ly:spanner::set-spacing-rods grob)
  (%check-type "ly:spanner::set-spacing-rods" 1 ly:spanner? "Spanner" grob)
  (%host-call 'spanner-set-spacing-rods grob))
"#;
