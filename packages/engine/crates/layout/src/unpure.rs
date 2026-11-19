//! The pure calls on a grob (MS3 step 14): `unpure-pure-container.cc`'s
//! calls over the music crate's container value (`unpure.rs` there, which
//! music functions make), and what `grob-property.cc` and `grob.cc` make
//! of them for the pure calculations of line breaking.
//!
//! - `ly:pure-call` and `ly:unpure-call` stay the music crate's prelude's,
//!   which are LilyPond's for any grob; layout's grobs are given to them
//!   as they are.
//! - `call_pure_function` ([`call_pure_function`], `grob-property.cc:334`):
//!   a container's pure part called with the grob, the start and the end,
//!   its unpure part without them if it has no pure part of its own; a
//!   value as it is; a procedure that is no container gives `#f`.
//! - `internal_get_pure_property` ([`get_pure_property`],
//!   `grob-property.cc:226`): the property's data, nothing kept, but for a
//!   container without a pure part, which the unpure lookup calls and
//!   keeps; with `ly:grob-pure-property`.
//! - `pure_relative_y_coordinate` ([`pure_relative_y_coordinate`],
//!   `grob.cc:408`): the pure `Y-offset`s up to `refp`, an offset already
//!   kept in their place, `pure-Y-offset-in-progress` set while one is
//!   called and its programming error for a cycle, and
//!   `align-interface.cc`'s pure translation of an alignment's child
//!   (`align.rs`); with `ly:grob-pure-relative-coordinate`.
//! - `pure_y_extent` ([`pure_y_extent`], `grob.cc:546`, and `Item`'s,
//!   `item.cc:242`): the pure `Y-extent` with the `minimum-Y-extent`,
//!   translated by the pure coordinate; an item keeps its height relative
//!   to itself the first time and then gives it whatever the columns
//!   ([`crate::grob::Grob::pure_height`]); with `ly:grob-pure-height`.
//! - `internal_get_maybe_pure_property` ([`get_maybe_pure_property`],
//!   `grob-property.cc:244`, MS3 step 28): the pure property or the
//!   property.
//!
//! A primitive cannot call a procedure, so [`PRELUDE`] does the calls and
//! reads the properties, and the primitives check the arguments and keep
//! the caches. Since MS4 step 1 (OPEN D3, 05 §5.1) the pure lookups are
//! Rust over a [`Host`], as `grob.rs`' offsets and extents: the pure
//! coordinate and height, the pure property and `call_pure_function`,
//! which runs a pure callback the callback table has in Rust without the
//! VM (`callbacks.rs`). [`PRELUDE`]'s `%grob-pure-relative-y-coordinate`
//! and `%grob-pure-y-extent` reach them over `%host-call` (D127,
//! [`HOSTS`]), their Scheme kept for a run cancelled for the host. A
//! spanner's pure property cache (`spanner.cc:594–619`) is `spanner.rs`'.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use lily_engine_music::{Arena, Host};
use lily_engine_scheme::{Cx, Exception, Heap, HostHandler, Tag, Thrown, Value, Vm};

use crate::R;
use crate::callbacks::{self, Pure};
use crate::geometry::{Interval, Y};
use crate::grob::{self, grob_arg, grobs, has_interface, reported, with_dim};
use crate::item;

/// `LY_ASSERT_TYPE (scm_is_integer, v, position)` of `subr`.
fn integer_arg(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<(), Thrown> {
    if cx.heap().is_integer(v) {
        Ok(())
    } else {
        Err(cx.wrong_type(subr, position, v, "integer"))
    }
}

/// `from_scm<vsize>` (`scm_to_ulong`): an exact integer from 0, else
/// Guile's error, from no procedure. One of 2⁶³ and more is out of range
/// here, where LilyPond takes it up to 2⁶⁴ − 1.
fn vsize(cx: &mut Cx, v: Value) -> Result<(), Thrown> {
    match cx.heap().to_i64(v) {
        Some(0..) => Ok(()),
        Some(_) => Err(cx.out_of_bounds(0, u64::MAX, v)),
        None if cx.heap().is_exact_integer(v) => Err(cx.out_of_bounds(0, u64::MAX, v)),
        None => Err(cx.wrong_type_vm(None, v, "exact integer")),
    }
}

/// The columns `start` and `end`, arguments 3 and 4 of `subr`, checked
/// and then converted, in LilyPond's order.
fn columns(cx: &mut Cx, subr: &str, start: Value, end: Value) -> Result<(), Thrown> {
    integer_arg(cx, subr, 3, start)?;
    integer_arg(cx, subr, 4, end)?;
    vsize(cx, start)?;
    vsize(cx, end)
}

/// `(ly:grob-pure-property grob sym beg end [val])`
/// (`grob-scheme.cc:104–124`): [`PRELUDE`]'s `%grob-pure-property`, `val`
/// or `'()` for `'()`.
pub(crate) fn ly_grob_pure_property(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-pure-property";
    grob_arg(cx, SUBR, 1, args[0])?;
    if args[1].tag() != Tag::Symbol {
        return Err(cx.wrong_type(SUBR, 2, args[1], "symbol"));
    }
    columns(cx, SUBR, args[2], args[3])?;
    let default = args.get(4).copied().unwrap_or(Value::NULL);
    match cx.global("lily", "%grob-pure-property-or") {
        Some(f) => Ok(cx.tail_call(f, &[args[0], args[1], args[2], args[3], default])),
        None => Ok(default),
    }
}

/// `(ly:grob-pure-height grob refp beg end [val])`
/// (`grob-scheme.cc:126–143`): [`PRELUDE`]'s `%grob-pure-y-extent`; `val`
/// is not used, as in LilyPond.
pub(crate) fn ly_grob_pure_height(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-pure-height";
    grob_arg(cx, SUBR, 1, args[0])?;
    grob_arg(cx, SUBR, 2, args[1])?;
    columns(cx, SUBR, args[2], args[3])?;
    match cx.global("lily", "%grob-pure-y-extent") {
        Some(f) => Ok(cx.tail_call(f, &[args[0], args[1], args[2], args[3]])),
        None => Ok(Value::UNSPECIFIED),
    }
}

/// `LY_ASSERT_TYPE (is_scm<vsize>, v, position)` of `subr`: an exact
/// integer from 0; one of 2⁶³ and more is refused here.
fn vsize_arg(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<(), Thrown> {
    let heap = cx.heap();
    if heap.is_exact_integer(v) && heap.to_i64(v).is_some_and(|n| n >= 0) {
        Ok(())
    } else {
        Err(cx.wrong_type(subr, position, v, "integer in [0, 2^64)"))
    }
}

/// `(ly:grob-pure-relative-coordinate grob refp start end)`
/// (`grob-scheme.cc:287–302`): [`PRELUDE`]'s
/// `%grob-pure-relative-y-coordinate`.
pub(crate) fn ly_grob_pure_relative_coordinate(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-pure-relative-coordinate";
    grob_arg(cx, SUBR, 1, args[0])?;
    grob_arg(cx, SUBR, 2, args[1])?;
    vsize_arg(cx, SUBR, 3, args[2])?;
    vsize_arg(cx, SUBR, 4, args[3])?;
    match cx.global("lily", "%grob-pure-relative-y-coordinate") {
        Some(f) => Ok(cx.tail_call(f, &args[..4])),
        None => Ok(Value::UNSPECIFIED),
    }
}

/// `(%grob-pure-offset-begin grob)`: the offset on Y kept, or `#f` and
/// the offset made 0 while the pure one is computed.
pub(crate) fn pure_offset_begin(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-pure-offset-begin", 1, args[0])?;
    let kept = with_dim(&arena, id, Y, |d| {
        let kept = d.offset;
        d.offset.get_or_insert(0.0);
        kept
    });
    Ok(match kept {
        Some(offset) => cx.heap_mut().flonum(offset),
        None => Value::FALSE,
    })
}

/// `(%grob-pure-offset-end grob)`: the offset on Y forgotten again.
pub(crate) fn pure_offset_end(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-pure-offset-end", 1, args[0])?;
    with_dim(&arena, id, Y, |d| d.offset = None);
    Ok(Value::UNSPECIFIED)
}

/// `(%grob-del-property! grob sym)`: `del_property`.
pub(crate) fn del_property(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-del-property!", 1, args[0])?;
    grob::del_property(cx.heap_mut(), &arena, id, args[1]);
    Ok(Value::UNSPECIFIED)
}

/// `(%grob-align-child? grob)`: whether the grob's parent on Y is an
/// alignment (`has_interface<Align_interface>`).
pub(crate) fn is_align_child(cx: &mut Cx, args: &[Value]) -> R {
    let heap = cx.heap_mut();
    let parent = grob::get_parent(heap, args[0], Y);
    Ok(Value::bool(parent.is_some_and(|p| {
        has_interface(heap, p, "align-interface")
    })))
}

/// `(%grob-programming-error grob text)`: `Grob::programming_error`, at
/// the grob's origin.
pub(crate) fn grob_programming_error(cx: &mut Cx, args: &[Value]) -> R {
    let mut text = String::new();
    cx.heap().display(args[1], &mut text);
    reported(
        cx,
        args[0],
        "programming-error",
        Value::UNSPECIFIED,
        &[text],
    )
}

/// The pair of reals `v` as an interval (`from_scm<Interval>`), else
/// `fallback`.
fn interval_or(heap: &Heap, v: Value, fallback: Interval) -> Interval {
    match (heap.car(v), heap.cdr(v)) {
        (Some(l), Some(r)) if heap.is_real(l) && heap.is_real(r) => Interval {
            left: heap.to_f64(l).unwrap_or(0.0),
            right: heap.to_f64(r).unwrap_or(0.0),
        },
        _ => fallback,
    }
}

fn interval_value(heap: &mut Heap, iv: Interval) -> Value {
    let (l, r) = (heap.flonum(iv.left), heap.flonum(iv.right));
    heap.cons(l, r)
}

/// `(%pure-y-extent extent offset minimum)`: the rest of
/// `Grob::pure_y_extent`, with the pure `Y-extent`, the pure coordinate
/// and the `minimum-Y-extent` read: the minimum added and the offset
/// applied only to an extent that is not empty.
pub(crate) fn pure_extent(cx: &mut Cx, args: &[Value]) -> R {
    let heap = cx.heap_mut();
    let mut iv = interval_or(heap, args[0], Interval::EMPTY);
    if !iv.is_empty() {
        iv.unite(interval_or(heap, args[2], Interval::EMPTY));
        iv.translate(heap.to_f64(args[1]).unwrap_or(0.0));
    }
    Ok(interval_value(heap, iv))
}

/// `(%item-pure-height grob)`: the item's height kept, or `#f`.
pub(crate) fn item_pure_height(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%item-pure-height", 1, args[0])?;
    let kept = grobs(&mut arena.borrow_mut())
        .get(id)
        .and_then(|g| g.pure_height);
    Ok(match kept {
        Some(iv) => interval_value(cx.heap_mut(), iv),
        None => Value::FALSE,
    })
}

/// `(%item-cache-pure-height! grob height)`: `cache_pure_height`; the
/// height kept.
pub(crate) fn item_cache_pure_height(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%item-cache-pure-height!", 1, args[0])?;
    let iv = interval_or(cx.heap(), args[1], Interval::EMPTY);
    if let Some(g) = grobs(&mut arena.borrow_mut()).get_mut(id) {
        g.pure_height = Some(iv);
    }
    Ok(interval_value(cx.heap_mut(), iv))
}

/// `start` and `end` as Scheme's integers.
fn columns_of(host: &mut dyn Host, start: usize, end: usize) -> (Value, Value) {
    let heap = host.heap();
    (heap.integer(start as i64), heap.integer(end as i64))
}

/// `call_pure_function` (MS4 step 1, [`PRELUDE`]'s `%call-pure-function`
/// in Rust): `value` called in a pure context between the columns `start`
/// and `end`, with `args`, the grob first. What is no container is given
/// as it is, but `#f` for a procedure; a container without a pure part of
/// its own calls its unpure part with `args`, another its pure part with
/// the grob, the columns and the rest of `args`; a part that is no
/// procedure is given as it is. A part that the callback table has in Rust
/// (`callbacks.rs`) runs without the VM when `args` is the grob alone.
pub fn call_pure_function(
    host: &mut dyn Host,
    value: Value,
    args: &[Value],
    start: usize,
    end: usize,
) -> Result<Value, Exception> {
    let heap = host.heap();
    let Some((unpure, pure)) = lily_engine_music::unpure_pure_parts(heap, value) else {
        return Ok(if heap.is_procedure(value) {
            Value::FALSE
        } else {
            value
        });
    };
    let Some(pure) = pure else {
        if !host.heap().is_procedure(unpure) {
            return Ok(unpure);
        }
        if let &[g] = args
            && let Some((arena, _)) = grob::host_grob(host, g)
            && let Some(f) = callbacks::find(host, &arena, unpure)
        {
            return f(host, g);
        }
        return host.call(unpure, args);
    };
    if !host.heap().is_procedure(pure) {
        return Ok(pure);
    }
    if let &[g] = args
        && let Some((arena, _)) = grob::host_grob(host, g)
    {
        match callbacks::find_pure(host, &arena, pure) {
            Some(Pure::Grob(f)) => return f(host, g),
            Some(Pure::Columns(f)) => return f(host, g, start, end),
            None => {}
        }
    }
    let (s, e) = columns_of(host, start, end);
    let mut all = Vec::with_capacity(args.len() + 2);
    all.extend([args[0], s, e]);
    all.extend_from_slice(&args[1..]);
    host.call(pure, &all)
}

/// `internal_get_pure_property` ([`PRELUDE`]'s `%grob-pure-property` in
/// Rust): the pure value of the grob's property `sym`. Its data called by
/// [`call_pure_function`] if that is a procedure or a container, but read
/// through its callbacks for a container without a pure part of its own.
pub fn get_pure_property(
    host: &mut dyn Host,
    grob: Value,
    sym: Value,
    start: usize,
    end: usize,
) -> Result<Value, Exception> {
    let Some((arena, id)) = grob::host_grob(host, grob) else {
        let f = crate::lily_procedure(host, "%grob-pure-property")?;
        let (start, end) = columns_of(host, start, end);
        return host.call(f, &[grob, sym, start, end]);
    };
    let value = grob::property_data(host.heap(), &arena, id, grob, sym);
    let heap = host.heap();
    if heap.is_procedure(value) {
        return Ok(Value::FALSE);
    }
    match lily_engine_music::unpure_pure_parts(heap, value) {
        None => Ok(value),
        Some((_, None)) => grob::get_property(host, grob, sym),
        Some(_) => call_pure_function(host, value, &[grob], start, end),
    }
}

/// `internal_get_maybe_pure_property` (`grob-property.cc:244–249`): the
/// grob's property `name`, its pure value between `start` and `end` with
/// `pure` ([`get_pure_property`]), else through its callbacks.
pub fn get_maybe_pure_property(
    host: &mut dyn Host,
    grob: Value,
    name: &str,
    pure: bool,
    start: usize,
    end: usize,
) -> Result<Value, Exception> {
    if pure {
        let sym = host.heap().intern(name);
        get_pure_property(host, grob, sym, start, end)
    } else {
        grob::property(host, grob, name)
    }
}

/// `pure_relative_y_coordinate` (`grob.cc:408–448`, MS4 step 1:
/// [`PRELUDE`]'s `%grob-pure-relative-y-coordinate` in Rust): the grob's
/// pure coordinate on Y relative to `refp`, the pure `Y-offset`s up to it.
/// An offset already kept is taken, with the programming error for a
/// cycle if `pure-Y-offset-in-progress` is set; else the offset is 0 while
/// its data is called (`pure-Y-offset-in-progress` set meanwhile), and
/// forgotten after. The child of an alignment without a kept offset adds
/// its pure translation there (`align.rs`).
pub fn pure_relative_y_coordinate(
    host: &mut dyn Host,
    grob: Value,
    refp: Value,
    start: usize,
    end: usize,
) -> Result<f64, Exception> {
    if grob == refp {
        return Ok(0.0);
    }
    let (arena, id) = grob::host_grob_arg(host, "%grob-pure-offset-begin", grob)?;
    let kept = with_dim(&arena, id, Y, |d| {
        let kept = d.offset;
        d.offset.get_or_insert(0.0);
        kept
    });
    let cached = match kept {
        None => cached_offset(&arena, id, start, end),
        Some(_) => None,
    };
    let offset = match (kept, cached) {
        (None, Some((offset, moves))) => {
            with_dim(&arena, id, Y, |d| d.offset = None);
            if moves {
                read_translation();
            }
            offset
        }
        (Some(offset), _) => {
            if grob::property(host, grob, "pure-Y-offset-in-progress")? == Value::TRUE {
                // As the Scheme reports it, at no origin too.
                let text = host
                    .heap()
                    .string("cyclic chain in pure-Y-offset callbacks");
                crate::call_lily(host, "%grob-programming-error", &[grob, text])?;
            }
            offset
        }
        (None, None) => {
            let heap = host.heap();
            let (sym, flag) = (
                heap.intern("Y-offset"),
                heap.intern("pure-Y-offset-in-progress"),
            );
            let proc = grob::property_data(heap, &arena, id, grob, sym);
            grob::set_property_data(heap, &arena, id, flag, Value::TRUE);
            let said = said_mark(host);
            let reads = translations_read();
            let value = call_pure_function(host, proc, &[grob], start, end)?;
            crate::queue::check(crate::queue::Step::Box);
            grob::del_property(host.heap(), &arena, id, flag);
            with_dim(&arena, id, Y, |d| d.offset = None);
            let heap = host.heap();
            let offset = if heap.is_real(value) {
                heap.to_f64(value).unwrap_or(0.0)
            } else {
                0.0
            };
            // Said again where it is computed again, as LilyPond says it.
            if said_mark(host) == said {
                let moves = translations_read() != reads;
                cache_offset(&arena, id, start, end, offset, moves);
            }
            offset
        }
    };
    let Some(parent) = grob::get_parent(host.heap(), grob, Y) else {
        return Ok(offset);
    };
    let translation = if kept.is_none() && has_interface(host.heap(), parent, "align-interface") {
        crate::align::get_pure_child_y_translation(host, parent, grob, start, end)?
    } else {
        0.0
    };
    Ok(offset + translation + pure_relative_y_coordinate(host, parent, refp, start, end)?)
}

/// Whether the grobs' pure offsets are kept (D252): from the start of
/// `read_before_breaking` (`system.rs`) to the break substitution, an epoch
/// each time, which the offsets kept are of.
static EPOCH: AtomicU32 = AtomicU32::new(0);
static KEEPING: AtomicBool = AtomicBool::new(false);
/// How many times a pure translation of an alignment's child was read
/// ([`translations_read`]).
static TRANSLATIONS: AtomicU32 = AtomicU32::new(0);

/// A count that moves when a pure translation of an alignment's child is
/// read: a value made with the count still is no line's place, and stays
/// what it is when the lines move (OPEN B21). Another thread's compile can
/// only move it, which costs that reuse and nothing else.
pub(crate) fn translations_read() -> u32 {
    TRANSLATIONS.load(Ordering::Relaxed)
}

/// A pure translation of an alignment's child is read.
pub(crate) fn read_translation() {
    TRANSLATIONS.fetch_add(1, Ordering::Relaxed);
}

/// The pure offsets are kept from here, in a new epoch: nothing laid out
/// before the line breaks moves a grob purely, so a pure offset asked for
/// again between the same columns is what it was.
pub fn keep_pure_offsets() {
    if !KEEPING.swap(true, Ordering::Relaxed) {
        EPOCH.fetch_add(1, Ordering::Relaxed);
    }
}

/// The lines are broken: what comes after them moves grobs. Those of the
/// grobs of `arena` go; another score's, not yet broken, are of this epoch.
pub fn forget_pure_offsets(arena: &RefCell<Arena>) {
    KEEPING.store(false, Ordering::Relaxed);
    let mut a = arena.borrow_mut();
    let slot = grobs(&mut a);
    for id in 0..slot.len() as u32 {
        if let Some(g) = slot.get_mut(id) {
            g.pure_offsets = Vec::new();
        }
    }
}

/// A compile was stopped between its steps (H3, D254), its grobs dropped
/// with their offsets: nothing is being kept.
pub fn stop_keeping() {
    KEEPING.store(false, Ordering::Relaxed);
}

/// What has been said: the error port's length and the last diagnostic.
fn said_mark(host: &mut dyn Host) -> (usize, u64) {
    let cell = host.lily("%diagnostics").unwrap_or(Value::NULL);
    let last = host.heap().car(cell).map_or(0, Value::to_bits);
    (host.error_len(), last)
}

/// The offset kept of the grob `id` between the columns, and whether making
/// it read an alignment's translation.
fn cached_offset(arena: &RefCell<Arena>, id: u32, start: usize, end: usize) -> Option<(f64, bool)> {
    if !KEEPING.load(Ordering::Relaxed) {
        return None;
    }
    let epoch = EPOCH.load(Ordering::Relaxed);
    let mut a = arena.borrow_mut();
    let g = grobs(&mut a).get(id)?;
    g.pure_offsets
        .iter()
        .find(|&&(e, s, t, _, _)| e == epoch && s == start && t == end)
        .map(|o| (o.3, o.4))
}

/// The offsets from `grob` up to `stop` (not its own), as
/// [`pure_relative_y_coordinate`] has them over the whole line, kept; none
/// if one was not kept, read a translation, or lies past an alignment: what
/// stays of the pure coordinate when `stop` moves. The pure coordinate is
/// what follows `offset + (…)`, so this and a coordinate of `stop` make it
/// bit for bit ([`coordinate_from`]).
pub(crate) fn offsets_to(host: &mut dyn Host, grob: Value, stop: Value) -> Option<Vec<f64>> {
    let mut offsets = Vec::new();
    let mut at = grob;
    while at != stop {
        let (arena, id) = lily_engine_music::grob_of(host.heap(), at)?;
        let kept = with_dim(&arena, id, Y, |d| d.offset);
        let offset = match kept {
            Some(offset) => offset,
            None => match cached_offset(&arena, id, 0, 2_147_483_647)? {
                (offset, false) => offset,
                (_, true) => return None,
            },
        };
        offsets.push(offset);
        let parent = grob::get_parent(host.heap(), at, Y)?;
        if kept.is_none() && has_interface(host.heap(), parent, "align-interface") {
            return None;
        }
        at = parent;
    }
    Some(offsets)
}

/// The pure coordinate of what [`offsets_to`] gave `offsets` for, its stop
/// at `coordinate`, as [`pure_relative_y_coordinate`] adds them.
pub(crate) fn coordinate_from(offsets: &[f64], coordinate: f64) -> f64 {
    offsets
        .iter()
        .rev()
        .fold(coordinate, |above, &offset| offset + 0.0 + above)
}

/// The height relative to itself the item `id` keeps, if making it read no
/// translation.
pub(crate) fn steady_height(arena: &RefCell<Arena>, id: u32) -> Option<Interval> {
    let mut a = arena.borrow_mut();
    let g = grobs(&mut a).get(id)?;
    if g.pure_height_moves {
        None
    } else {
        g.pure_height
    }
}

fn cache_offset(
    arena: &RefCell<Arena>,
    id: u32,
    start: usize,
    end: usize,
    offset: f64,
    moves: bool,
) {
    if !KEEPING.load(Ordering::Relaxed) {
        return;
    }
    let epoch = EPOCH.load(Ordering::Relaxed);
    let mut a = arena.borrow_mut();
    if let Some(g) = grobs(&mut a).get_mut(id) {
        g.pure_offsets.retain(|o| o.0 == epoch);
        g.pure_offsets.push((epoch, start, end, offset, moves));
    }
}

/// `pure_y_extent`, an item's or a grob's (MS4 step 1: [`PRELUDE`]'s
/// `%grob-pure-y-extent` in Rust): the grob's pure height relative to
/// `refp`. An item's height relative to itself is kept the first time
/// ([`crate::grob::Grob::pure_height`]) and then given whatever the
/// columns, translated by its pure coordinate.
pub fn pure_y_extent(
    host: &mut dyn Host,
    grob: Value,
    refp: Value,
    start: usize,
    end: usize,
) -> Result<Interval, Exception> {
    if !item::is_item(host.heap(), grob) {
        return own_pure_y_extent(host, grob, refp, start, end);
    }
    let (arena, id) = grob::host_grob_arg(host, "%item-pure-height", grob)?;
    let kept = grobs(&mut arena.borrow_mut())
        .get(id)
        .and_then(|g| g.pure_height.map(|height| (height, g.pure_height_moves)));
    let height = match kept {
        Some((height, moves)) => {
            if moves {
                read_translation();
            }
            height
        }
        None => {
            let reads = translations_read();
            let height = own_pure_y_extent(host, grob, grob, start, end)?;
            if let Some(g) = grobs(&mut arena.borrow_mut()).get_mut(id) {
                g.pure_height = Some(height);
                g.pure_height_moves = translations_read() != reads;
            }
            height
        }
    };
    let offset = pure_relative_y_coordinate(host, grob, refp, start, end)?;
    Ok(Interval {
        left: height.left + offset,
        right: height.right + offset,
    })
}

/// `Grob::pure_y_extent`: the pure `Y-extent`, then the pure coordinate,
/// then the `minimum-Y-extent` read; the minimum added and the offset
/// applied only to an extent that is not empty.
fn own_pure_y_extent(
    host: &mut dyn Host,
    grob: Value,
    refp: Value,
    start: usize,
    end: usize,
) -> Result<Interval, Exception> {
    let sym = host.heap().intern("Y-extent");
    // Read as an interval at once: a collection may come with the calls
    // after it.
    let extent = get_pure_property(host, grob, sym, start, end)?;
    let mut iv = interval_or(host.heap(), extent, Interval::EMPTY);
    let offset = pure_relative_y_coordinate(host, grob, refp, start, end)?;
    let minimum = grob::property(host, grob, "minimum-Y-extent")?;
    if !iv.is_empty() {
        iv.unite(interval_or(host.heap(), minimum, Interval::EMPTY));
        iv.translate(offset);
    }
    Ok(iv)
}

/// What `%host-call` runs for [`PRELUDE`]'s `(%host-call 'unpure what grob
/// refp start end)` (D127): [`pure_y_extent`] and
/// [`pure_relative_y_coordinate`] with the VM as their host; through
/// `(lily)`'s Scheme for columns that are no `usize`.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "unpure",
    f: host_unpure,
}];

fn host_unpure(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let heap = vm.heap();
    let column = |v: Value| {
        let n = heap.is_exact_integer(v).then(|| heap.to_i64(v)).flatten();
        n.and_then(|n| usize::try_from(n).ok())
    };
    let extent = heap.name(args[1]) == Some("pure-y-extent");
    let (grob, refp) = (args[2], args[3]);
    let (Some(start), Some(end)) = (column(args[4]), column(args[5])) else {
        let name = if extent {
            "%grob-scheme-pure-y-extent"
        } else {
            "%grob-scheme-pure-relative-y-coordinate"
        };
        return crate::call_lily(vm, name, &args[2..6]);
    };
    if extent {
        let iv = pure_y_extent(vm, grob, refp, start, end)?;
        Ok(interval_value(vm.heap_mut(), iv))
    } else {
        let y = pure_relative_y_coordinate(vm, grob, refp, start, end)?;
        Ok(vm.heap_mut().flonum(y))
    }
}

/// What calls a procedure of the pure lookups, in `grob-property.cc`'s
/// and `grob.cc`'s order, around the primitives that keep the caches.
pub(crate) const PRELUDE: &str = r#"
(define (%call-pure-function value args start end)
  (cond ((not (ly:unpure-pure-container? value))
         (if (procedure? value) #f value))
        ((%unpure-pure-unchanging? value)
         (let ((unpure (ly:unpure-pure-container-unpure-part value)))
           (if (procedure? unpure) (apply unpure args) unpure)))
        (else
         (let ((pure (ly:unpure-pure-container-pure-part value)))
           (if (procedure? pure) (apply pure (car args) start end (cdr args)) pure)))))
(define (%grob-pure-property grob sym start end)
  (let ((value (%grob-property-data grob sym)))
    (cond ((procedure? value) (%call-pure-function value (list grob) start end))
          ((not (ly:unpure-pure-container? value)) value)
          ((%unpure-pure-unchanging? value) (ly:grob-property grob sym))
          (else (%call-pure-function value (list grob) start end)))))
(define (%grob-pure-property-or grob sym start end default)
  (let ((value (%grob-pure-property grob sym start end)))
    (if (null? value) default value)))
(define (%grob-pure-relative-y-coordinate grob refp start end)
  (if (%host-waiting?)
      (%grob-scheme-pure-relative-y-coordinate grob refp start end)
      (%host-call 'unpure 'pure-relative-y-coordinate grob refp start end)))
(define (%grob-pure-y-extent grob refp start end)
  (if (%host-waiting?)
      (%grob-scheme-pure-y-extent grob refp start end)
      (%host-call 'unpure 'pure-y-extent grob refp start end)))
(define (%grob-scheme-pure-relative-y-coordinate grob refp start end)
  (if (eq? grob refp)
      0.0
      (let* ((kept (%grob-pure-offset-begin grob))
             (offset
              (if kept
                  (begin
                    (if (eq? (ly:grob-property grob 'pure-Y-offset-in-progress) #t)
                        (%grob-programming-error grob "cyclic chain in pure-Y-offset callbacks"))
                    kept)
                  (let ((proc (%grob-property-data grob 'Y-offset)))
                    (%grob-set-property-data! grob 'pure-Y-offset-in-progress #t)
                    (let ((value (%call-pure-function proc (list grob) start end)))
                      (%grob-del-property! grob 'pure-Y-offset-in-progress)
                      (%grob-pure-offset-end grob)
                      (if (real? value) (exact->inexact value) 0.0)))))
             (parent (ly:grob-parent grob 1)))
        (if (ly:grob? parent)
            (let ((translation (if (and (not kept) (%grob-align-child? grob))
                                   (%align-pure-child-y-translation parent grob start end)
                                   0.0)))
              (+ offset translation (%grob-scheme-pure-relative-y-coordinate parent refp start end)))
            offset))))
(define (%grob-scheme-pure-y-extent grob refp start end)
  (if (ly:item? grob)
      (let* ((height (or (%item-pure-height grob)
                         (%item-cache-pure-height! grob (%grob-own-pure-y-extent grob grob start end))))
             (offset (%grob-scheme-pure-relative-y-coordinate grob refp start end)))
        (cons (+ (car height) offset) (+ (cdr height) offset)))
      (%grob-own-pure-y-extent grob refp start end)))
(define (%grob-own-pure-y-extent grob refp start end)
  (let* ((extent (%grob-pure-property grob 'Y-extent start end))
         (offset (%grob-scheme-pure-relative-y-coordinate grob refp start end)))
    (%pure-y-extent extent offset (ly:grob-property grob 'minimum-Y-extent))))
"#;
