//! `grob-array.cc`, `grob-array-scheme.cc`, `pointer-group-interface.cc`
//! and `pointer-group-interface-scheme.cc` (MS3 step 14): a grob's grobs in
//! one of its object slots.
//!
//! - [`GrobArray`], `Grob_array`: the handles of its grobs, in order, and
//!   whether their order matters; `ly:grob-array?` (which replaces the
//!   scheme crate's placeholder), `ly:grob-array-length`,
//!   `ly:grob-array-ref`, `ly:grob-array-filter`, `ly:grob-array->list` and
//!   `ly:grob-list->grob-array`. No image holds one: grobs are made after
//!   the image is.
//! - The pointer groups (`Pointer_group_interface`): the grob array in an
//!   object slot of a grob, made there the first time it is asked for
//!   ([`get_grob_array`]), its grobs counted, added to ([`add_grob`]) and
//!   searched, with `ly:pointer-group-interface::add-grob`; and
//!   `extract_grob_set`, `extract_item_set` and `extract_spanner_set`
//!   ([`extract_grob_array`], [`extract_item_array`],
//!   [`extract_spanner_array`]). The slot is read as `internal_get_object`
//!   reads it, its callback called, so Rust reaches it through a [`Host`]
//!   and [`PRELUDE`]'s `%grob-array`.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, Foreign, Heap, Tracer, Value};

use crate::R;
use crate::grob;
use crate::item::{self, Class};

/// `Grob_array`: the handles of a grob's grobs, and whether their order
/// matters (`ordered_`, true when made).
pub struct GrobArray {
    pub grobs: Vec<Value>,
    pub ordered: bool,
}

impl Foreign for GrobArray {
    fn type_name(&self) -> &'static str {
        "Grob_array"
    }

    /// `#<Grob_array#<Grob NoteHead > #<Grob Stem > >`, each grob displayed
    /// and followed by a space.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<Grob_array");
        for &g in &self.grobs {
            heap.display(g, out);
            out.push(' ');
        }
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        for &g in &self.grobs {
            tracer.trace(g);
        }
    }

    /// A copy for S5's undo log: N8's snapshots give a grob's grobs back
    /// as a checkpoint had them (D241).
    fn snapshot(&self) -> Option<alloc::boxed::Box<dyn Foreign>> {
        Some(alloc::boxed::Box::new(GrobArray {
            grobs: self.grobs.clone(),
            ordered: self.ordered,
        }))
    }

    fn same(&self, other: &dyn Foreign) -> bool {
        let any: &dyn core::any::Any = other;
        any.downcast_ref::<GrobArray>()
            .is_some_and(|o| o.grobs == self.grobs && o.ordered == self.ordered)
    }
}

impl GrobArray {
    /// `make_array`: an empty, ordered array.
    pub fn new() -> GrobArray {
        GrobArray {
            grobs: Vec::new(),
            ordered: true,
        }
    }

    /// `remove_duplicates` of an unordered array (`uniquify`,
    /// `grob.cc:1033–1046`): each grob kept once, where it first is, in
    /// the array's order (MS3 step 33: LilyPond 2.26.0's does not sort by
    /// address).
    pub fn remove_duplicates(&mut self, heap: &Heap) {
        debug_assert!(!self.ordered);
        let key = |g: Value| grob_of(heap, g).map(|(arena, id)| (arena.as_ptr() as usize, id));
        let mut seen = BTreeSet::new();
        self.grobs
            .retain(|&g| key(g).is_none_or(|k| seen.insert(k)));
    }
}

impl Default for GrobArray {
    fn default() -> GrobArray {
        GrobArray::new()
    }
}

/// `ly_scm2link_array`: the grobs of `v`, if it is a grob array, else none.
pub fn grobs_of(heap: &Heap, v: Value) -> Vec<Value> {
    heap.foreign::<GrobArray>(v)
        .map(|a| a.grobs.clone())
        .unwrap_or_default()
}

/// `LY_ASSERT_SMOB (Grob_array, v, position)` of `subr`.
fn array_arg<'a>(
    cx: &'a mut Cx,
    subr: &str,
    position: usize,
    v: Value,
) -> Result<&'a GrobArray, lily_engine_scheme::Thrown> {
    if cx.heap().foreign::<GrobArray>(v).is_none() {
        return Err(cx.wrong_type(subr, position, v, "Grob_array"));
    }
    Ok(cx.heap().foreign::<GrobArray>(v).expect("checked"))
}

/// `(ly:grob-array? x)`.
pub(crate) fn is_grob_array(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        cx.heap().foreign::<GrobArray>(args[0]).is_some(),
    ))
}

/// `(ly:grob-array-length grob-arr)`.
pub(crate) fn length(cx: &mut Cx, args: &[Value]) -> R {
    let n = array_arg(cx, "ly:grob-array-length", 1, args[0])?
        .grobs
        .len();
    Ok(cx.heap_mut().integer(n as i64))
}

/// `(ly:grob-array-ref grob-arr index)`: an integer, converted as
/// `scm_to_uint` converts it; Guile's `Value out of range` past the end,
/// which [`PRELUDE`]'s `%grob-array-out-of-range` throws with its data.
pub(crate) fn grob_array_ref(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-array-ref";
    array_arg(cx, SUBR, 1, args[0])?;
    let index = args[1];
    if !cx.heap().is_integer(index) {
        return Err(cx.wrong_type(SUBR, 2, index, "integer"));
    }
    let i = match cx.heap().to_i64(index) {
        Some(i @ 0..=0xffff_ffff) => i as usize,
        Some(_) => return Err(cx.out_of_bounds(0, u64::from(u32::MAX), index)),
        None if cx.heap().is_exact_integer(index) => {
            return Err(cx.out_of_bounds(0, u64::from(u32::MAX), index));
        }
        None => return Err(cx.wrong_type_vm(None, index, "exact integer")),
    };
    if let Some(&g) = array_arg(cx, SUBR, 1, args[0])?.grobs.get(i) {
        return Ok(g);
    }
    match cx.global("lily", "%grob-array-out-of-range") {
        Some(proc) => Ok(cx.tail_call(proc, &[index])),
        None => Ok(Value::UNSPECIFIED),
    }
}

/// `(ly:grob-array->list grob-arr)`: `grob_array_to_list`.
pub(crate) fn to_list(cx: &mut Cx, args: &[Value]) -> R {
    let grobs = array_arg(cx, "ly:grob-array->list", 1, args[0])?
        .grobs
        .clone();
    Ok(cx.heap_mut().list(&grobs))
}

/// `(ly:grob-list->grob-array grob-list)`: `grob_list_to_grob_array`, the
/// grobs of the list, with LilyPond's warning for what is no grob and for
/// a list that is not proper.
pub(crate) fn from_list(cx: &mut Cx, args: &[Value]) -> R {
    let (array, warnings) = grob_list_to_grob_array(cx.heap_mut(), args[0]);
    crate::warned(cx, array, &warnings)
}

/// `grob_list_to_grob_array` (`grob-array.cc:67–85`): an ordered array of
/// the grobs of `lst`, with the warnings LilyPond gives for what is no grob
/// and for a list that is not proper.
pub(crate) fn grob_list_to_grob_array(heap: &mut Heap, lst: Value) -> (Value, Vec<String>) {
    let (mut grobs, mut warnings) = (Vec::new(), Vec::new());
    let mut s = lst;
    while let (Some(g), Some(rest)) = (heap.car(s), heap.cdr(s)) {
        if grob_of(heap, g).is_some() {
            grobs.push(g);
        } else {
            warnings.push(String::from(
                "ly:grob-list->grob-array encountered a non-grob object",
            ));
        }
        s = rest;
    }
    if s != Value::NULL {
        warnings.push(String::from("ly:grob-list->grob-array expected a list"));
    }
    let array = heap.wrap(GrobArray {
        grobs,
        ordered: true,
    });
    (array, warnings)
}

/// `(%grob-array-filtered grob-arr grobs)`: a new array of `grobs`, which
/// `ly:grob-array-filter` kept, ordered as `grob-arr` is.
pub(crate) fn filtered(cx: &mut Cx, args: &[Value]) -> R {
    let ordered = array_arg(cx, "ly:grob-array-filter", 2, args[0])?.ordered;
    let heap = cx.heap();
    let mut grobs = Vec::new();
    let mut p = args[1];
    while let (Some(g), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        grobs.push(g);
        p = rest;
    }
    Ok(cx.heap_mut().wrap(GrobArray { grobs, ordered }))
}

/// `(%grob-array-add! grob-arr grob)`: `Grob_array::add`.
pub(crate) fn add(cx: &mut Cx, args: &[Value]) -> R {
    array_arg(cx, "%grob-array-add!", 1, args[0])?;
    written_unknown(cx.heap(), &args[1..2]);
    if let Some(a) = cx.heap_mut().foreign_mut::<GrobArray>(args[0]) {
        a.grobs.push(args[1]);
    }
    Ok(Value::UNSPECIFIED)
}

/// `(%grob-array-set! grob-arr grobs)`: `Grob_array::set_array`, its grobs
/// those of the list `grobs`, whether their order matters kept.
pub(crate) fn set(cx: &mut Cx, args: &[Value]) -> R {
    array_arg(cx, "%grob-array-set!", 1, args[0])?;
    let heap = cx.heap();
    let mut grobs = Vec::new();
    let mut p = args[1];
    while let (Some(g), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        grobs.push(g);
        p = rest;
    }
    let mut all = grobs.clone();
    if let Some(a) = cx.heap().foreign::<GrobArray>(args[0]) {
        all.extend_from_slice(&a.grobs);
    }
    written_unknown(cx.heap(), &all);
    if let Some(a) = cx.heap_mut().foreign_mut::<GrobArray>(args[0]) {
        a.grobs = grobs;
    }
    Ok(Value::UNSPECIFIED)
}

// --- pointer groups (pointer-group-interface.cc) ---------------------------

/// `get_grob_array` (`pointer-group-interface.cc:52–64`): the grob array
/// in the object slot `sym` of the grob `me`, its callback called; a new
/// one set there if it holds none, on a live grob (`set_object`), as
/// [`PRELUDE`]'s `%grob-array` does.
pub fn get_grob_array(host: &mut dyn Host, me: Value, sym: Value) -> Result<Value, Exception> {
    let arr = object(host, me, sym)?;
    let heap = host.heap();
    if heap.foreign::<GrobArray>(arr).is_some() {
        return Ok(arr);
    }
    let (arr, _) = grob_list_to_grob_array(heap, Value::NULL);
    grob::set_object(heap, me, sym, arr);
    Ok(arr)
}

/// What `f` does to the grob array `arr`.
fn with_array<T>(
    host: &mut dyn Host,
    arr: Value,
    f: impl FnOnce(&mut GrobArray) -> T,
) -> Option<T> {
    host.heap().foreign_mut::<GrobArray>(arr).map(f)
}

/// `v` copied if it is a grob array, its grobs and order as they are now
/// (N8's log of an object set, MS6 step 7b), else `v`.
pub(crate) fn copied(heap: &mut Heap, v: Value) -> Value {
    match heap.foreign::<GrobArray>(v) {
        Some(a) => {
            let copy = GrobArray {
                grobs: a.grobs.clone(),
                ordered: a.ordered,
            };
            heap.wrap(copy)
        }
        None => v,
    }
}

/// A grob array of the old run's log made new for this run (N8's replay,
/// MS6 step 7b): its grobs this run's; `None` for a grob with none here.
/// Another value, which the replay has resolved, as it is.
pub(crate) fn replayed(heap: &mut Heap, v: Value) -> Option<Value> {
    let Some(a) = heap.foreign::<GrobArray>(v) else {
        return Some(v);
    };
    let (old, ordered) = (a.grobs.clone(), a.ordered);
    let grobs = old
        .into_iter()
        .map(|g| lily_engine_music::resolve_replayed(heap, g))
        .collect::<Option<Vec<Value>>>()?;
    Some(heap.wrap(GrobArray { grobs, ordered }))
}

/// The grob `me`, whose grob array is changed by `op`, written, `p` added
/// to it if it was (N8, MS6 steps 7a and 7b).
fn written(host: &mut dyn Host, me: Value, op: lily_engine_music::Op, p: Option<Value>) {
    let heap = host.heap();
    if let Some((arena, id)) = lily_engine_music::grob_of(heap, me) {
        let added = p
            .and_then(|p| lily_engine_music::grob_of(heap, p))
            .filter(|(of, _)| alloc::rc::Rc::ptr_eq(of, &arena))
            .map(|(_, i)| i);
        lily_engine_music::grob_wrote(&arena, Some(id), op, added);
    }
}

/// [`get_grob_array`] as an add calls it: the array it may set on `me` is
/// the add's write, not one of its own.
fn array_to_add(host: &mut dyn Host, me: Value, sym: Value) -> Result<Value, Exception> {
    let arena = lily_engine_music::grob_of(host.heap(), me).map(|(a, _)| a);
    let quiet = arena.as_ref().map(|a| lily_engine_music::quiet(a));
    let arr = get_grob_array(host, me, sym);
    if let (Some(a), Some(was)) = (arena, quiet) {
        lily_engine_music::unquiet(&a, was);
    }
    arr
}

/// A grob array changed from Scheme, its grob not known: written for each
/// translation one of `grobs` is of (N8, MS6 step 7a).
fn written_unknown(heap: &Heap, grobs: &[Value]) {
    if let Some((arena, _)) = grobs
        .iter()
        .find_map(|&g| lily_engine_music::grob_of(heap, g))
    {
        lily_engine_music::grob_wrote(&arena, None, lily_engine_music::Op::Unknown, None);
    }
}

/// `count`: how many grobs the object `sym` of `me` holds, if it is a grob
/// array, else 0.
pub fn count(host: &mut dyn Host, me: Value, sym: Value) -> Result<usize, Exception> {
    let v = object(host, me, sym)?;
    Ok(host
        .heap()
        .foreign::<GrobArray>(v)
        .map_or(0, |a| a.grobs.len()))
}

/// `add_grob`: `p` added to the grob array of `me`'s `sym`.
pub fn add_grob(host: &mut dyn Host, me: Value, sym: Value, p: Value) -> Result<(), Exception> {
    written(host, me, lily_engine_music::Op::Add(sym, p, true), Some(p));
    let arr = array_to_add(host, me, sym)?;
    with_array(host, arr, |a| a.grobs.push(p));
    Ok(())
}

/// `add_unordered_grob`: [`add_grob`], the array then unordered.
pub fn add_unordered_grob(
    host: &mut dyn Host,
    me: Value,
    sym: Value,
    p: Value,
) -> Result<(), Exception> {
    written(host, me, lily_engine_music::Op::Add(sym, p, false), Some(p));
    let arr = array_to_add(host, me, sym)?;
    with_array(host, arr, |a| {
        a.grobs.push(p);
        a.ordered = false;
    });
    Ok(())
}

/// `clear` and `add` of `me`'s `sym`: the grob array holds `p` alone, or
/// nothing.
pub fn reset(
    host: &mut dyn Host,
    me: Value,
    sym: Value,
    p: Option<Value>,
) -> Result<(), Exception> {
    let op = lily_engine_music::Op::Reset(sym, p.unwrap_or(Value::FALSE));
    written(host, me, op, None);
    let arr = get_grob_array(host, me, sym)?;
    with_array(host, arr, |a| {
        a.grobs.clear();
        a.grobs.extend(p);
    });
    Ok(())
}

/// `set_ordered`: whether the order of the grob array of `me`'s `sym`
/// matters.
pub fn set_ordered(
    host: &mut dyn Host,
    me: Value,
    sym: Value,
    ordered: bool,
) -> Result<(), Exception> {
    written(host, me, lily_engine_music::Op::Order(sym, ordered), None);
    let arr = get_grob_array(host, me, sym)?;
    with_array(host, arr, |a| a.ordered = ordered);
    Ok(())
}

/// `find_grob`: the first grob of the grob array of `me`'s `sym` that
/// `pred` holds of.
pub fn find_grob(
    host: &mut dyn Host,
    me: Value,
    sym: Value,
    mut pred: impl FnMut(&mut Heap, Value) -> bool,
) -> Result<Option<Value>, Exception> {
    let arr = get_grob_array(host, me, sym)?;
    let heap = host.heap();
    Ok(grobs_of(heap, arr).into_iter().find(|&g| pred(heap, g)))
}

/// `internal_get_object`: [`grob::object`], its callback called.
pub(crate) fn object(host: &mut dyn Host, me: Value, sym: Value) -> Result<Value, Exception> {
    grob::object(host, me, sym)
}

/// `extract_grob_set` (`internal_extract_grob_array`): the grobs of the
/// object `name` of `elt`, if it is a grob array, else none.
pub fn extract_grob_array(
    host: &mut dyn Host,
    elt: Value,
    name: &str,
) -> Result<Vec<Value>, Exception> {
    let sym = host.heap().intern(name);
    let v = object(host, elt, sym)?;
    Ok(grobs_of(host.heap(), v))
}

/// `extract_item_set`: the items of the object `name` of `elt`, with
/// LilyPond's programming error at each grob that is not one.
pub fn extract_item_array(
    host: &mut dyn Host,
    elt: Value,
    name: &str,
) -> Result<Vec<Value>, Exception> {
    extract_subtype_array(host, elt, name, Class::is_item)
}

/// `extract_spanner_set`: the spanners of the object `name` of `elt`, as
/// [`extract_item_array`] gives items.
pub fn extract_spanner_array(
    host: &mut dyn Host,
    elt: Value,
    name: &str,
) -> Result<Vec<Value>, Exception> {
    extract_subtype_array(host, elt, name, |c| !c.is_item())
}

/// `internal_extract_grob_subtype_array`: the grobs of the object `name`
/// of `elt` whose class `is` holds of.
fn extract_subtype_array(
    host: &mut dyn Host,
    elt: Value,
    name: &str,
    is: fn(Class) -> bool,
) -> Result<Vec<Value>, Exception> {
    let sym = host.heap().intern(name);
    let v = object(host, elt, sym)?;
    let depth = host.heap().root_depth();
    host.heap().root(v);
    let mut result = Vec::new();
    for g in grobs_of(host.heap(), v) {
        if item::class(host.heap(), g).is_some_and(is) {
            result.push(g);
        } else if let Err(e) =
            crate::grob::programming_error(host, g, "unexpected grob subtype in grob array")
        {
            host.heap().unroot_to(depth);
            return Err(e);
        }
    }
    host.heap().unroot_to(depth);
    Ok(result)
}

/// `ly:grob-array-filter`, which calls its predicate;
/// `ly:pointer-group-interface::add-grob` and `%grob-array`
/// (`get_grob_array`), which read an object slot as `internal_get_object`
/// does, its callback called; and Guile's `Value out of range` of an
/// index, with its data.
pub(crate) const PRELUDE: &str = r#"
(define (%grob-array-out-of-range index)
  (scm-error 'out-of-range #f "Value out of range: ~S" (list index) (list index)))
(define-public (ly:grob-array-filter predicate grob-arr)
  (%check-type "ly:grob-array-filter" 1 procedure? "procedure" predicate)
  (%check-type "ly:grob-array-filter" 2 ly:grob-array? "Grob_array" grob-arr)
  (%grob-array-filtered grob-arr (filter predicate (ly:grob-array->list grob-arr))))
(define (%grob-array grob sym)
  (let ((arr (ly:grob-object grob sym)))
    (if (ly:grob-array? arr)
        arr
        (let ((arr (ly:grob-list->grob-array '())))
          (ly:grob-set-object! grob sym arr)
          arr))))
(define-public (ly:pointer-group-interface::add-grob grob sym grob-element)
  (%check-type "ly:pointer-group-interface::add-grob" 1 ly:grob? "Grob" grob)
  (%check-type "ly:pointer-group-interface::add-grob" 2 symbol? "symbol" sym)
  (%check-type "ly:pointer-group-interface::add-grob" 3 ly:grob? "Grob" grob-element)
  (%grob-array-add! (%grob-array grob sym) grob-element))
"#;
