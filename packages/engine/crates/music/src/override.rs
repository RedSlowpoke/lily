//! Override stacks (04 §7): LilyPond's `Grob_properties` and
//! `Grob_property_info` (`context-property.cc`) with `nested-property.cc`'s
//! lists, N3's instances.
//!
//! A context's property named by a grob holds its grob properties: the
//! context's overrides pushed onto what its parent's were when it last
//! looked (`based_on`), as a Scheme list, so a push is a cons and a
//! snapshot a pointer. Global holds each grob's description. `\once` is a
//! push with a matched pop at the end of the timestep, and a cell whose key
//! is `#t` or `#f` a temporary override or revert. [`updated`] is the
//! effective list, the parents' changes taken in and nested overrides
//! (`(path . value)`, path a list) folded into their properties.

use alloc::vec::Vec;

use lily_engine_scheme::{Cx, Foreign, Heap, Tag, Tracer, Value};

use crate::R;
use crate::context::{Arena, CtxId, GLOBAL, Host, Translation};

/// A grob's properties in a context (`ly:grob-properties?`).
pub struct GrobProperties {
    alist: Value,
    based_on: Value,
    cooked: Value,
    cooked_from: Value,
    /// How many entries of `alist` are not plain properties.
    nested: i32,
}

impl Foreign for GrobProperties {
    fn type_name(&self) -> &'static str {
        "Grob_properties"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        for v in [self.alist, self.based_on, self.cooked, self.cooked_from] {
            tracer.trace(v);
        }
    }

    /// A copy for S5's undo log: N8's snapshots give a context's grob
    /// properties back as a checkpoint had them (D241).
    fn snapshot(&self) -> Option<alloc::boxed::Box<dyn Foreign>> {
        Some(alloc::boxed::Box::new(GrobProperties { ..*self }))
    }

    fn same(&self, other: &dyn Foreign) -> bool {
        let any: &dyn core::any::Any = other;
        any.downcast_ref::<GrobProperties>().is_some_and(|o| {
            (o.alist, o.based_on, o.cooked, o.cooked_from, o.nested)
                == (
                    self.alist,
                    self.based_on,
                    self.cooked,
                    self.cooked_from,
                    self.nested,
                )
        })
    }
}

impl GrobProperties {
    /// The properties pushed onto its description, the newest first.
    pub(crate) fn alist(&self) -> Value {
        self.alist
    }

    /// `ly:make-grob-properties` of `alist`.
    pub fn make(heap: &mut Heap, alist: Value) -> Value {
        heap.wrap(GrobProperties {
            alist,
            based_on: Value::NULL,
            cooked: alist,
            cooked_from: alist,
            nested: 0,
        })
    }
}

fn gp(heap: &mut Heap, v: Value) -> &mut GrobProperties {
    heap.foreign_mut::<GrobProperties>(v)
        .expect("grob properties")
}

/// `check`: the grob properties `c` itself holds for `sym`.
fn check(heap: &Heap, a: &Arena, c: CtxId, sym: Value) -> Option<Value> {
    a.here(c, sym)
        .filter(|&v| heap.foreign::<GrobProperties>(v).is_some())
}

/// `create`: `c`'s grob properties for `sym`, copied from Global's
/// description if it has none; none in Global itself, in a context no
/// longer below Global, or for a grob Global does not describe.
fn create(heap: &mut Heap, a: &mut Arena, c: CtxId, sym: Value) -> Option<Value> {
    if let Some(found) = check(heap, a, c, sym) {
        return Some(found);
    }
    let mut top = c;
    while let Some(p) = a.parent(top) {
        top = p;
    }
    if top != GLOBAL || c == GLOBAL {
        return None;
    }
    let def = check(heap, a, GLOBAL, sym)?;
    let alist = heap.foreign::<GrobProperties>(def)?.alist;
    let props = heap.wrap(GrobProperties {
        alist,
        based_on: alist,
        cooked: alist,
        cooked_from: alist,
        nested: 0,
    });
    a.set(c, sym, props);
    Some(props)
}

/// `push` of `path` (a list of symbols) and `value` onto `c`'s `sym`, the
/// value's type checked: the cell pushed, or `'()`.
pub fn push(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    sym: Value,
    path: Value,
    value: Value,
) -> Result<Value, lily_engine_scheme::Exception> {
    let heap = host.heap();
    if create(heap, &mut t.arena.borrow_mut(), c, sym).is_none() {
        return Ok(Value::NULL);
    }
    let (Some(symbol), Some(rest)) = (heap.car(path), heap.cdr(path)) else {
        return Ok(Value::NULL);
    };
    let nested = rest.tag() == Tag::Pair;
    let checked_value = if nested {
        nested_create_alist(heap, rest, value)
    } else {
        value
    };
    let Some(check) = host.lily("%type-check") else {
        let heap = host.heap();
        return Ok(push_unchecked(
            heap,
            &mut t.arena.borrow_mut(),
            c,
            sym,
            path,
            value,
        ));
    };
    let ty = host.heap().intern("backend-type?");
    let checked = host.call(check, &[symbol, checked_value, ty, Value::FALSE])?;
    let heap = host.heap();
    let (Some(s), Some(v)) = (heap.car(checked), heap.cdr(checked)) else {
        return Ok(Value::NULL);
    };
    if nested {
        if s != symbol {
            crate::context::diagnose(
                host,
                "programming-error",
                "deprecation not supported in this case",
            )?;
            return Ok(Value::NULL);
        }
        return Ok(push_unchecked(
            host.heap(),
            &mut t.arena.borrow_mut(),
            c,
            sym,
            path,
            value,
        ));
    }
    let path = heap.list(&[s]);
    Ok(push_unchecked(
        heap,
        &mut t.arena.borrow_mut(),
        c,
        sym,
        path,
        v,
    ))
}

/// `push` without the type check: a cell `(symbol . value)`, or for a
/// nested path `(path . value)`.
pub fn push_unchecked(
    heap: &mut Heap,
    a: &mut Arena,
    c: CtxId,
    sym: Value,
    path: Value,
    value: Value,
) -> Value {
    let Some(props) = create(heap, a, c, sym) else {
        return Value::NULL;
    };
    let (Some(symbol), Some(rest)) = (heap.car(path), heap.cdr(path)) else {
        return Value::NULL;
    };
    let nested = rest.tag() == Tag::Pair;
    let cell = heap.cons(if nested { path } else { symbol }, value);
    let alist = gp(heap, props).alist;
    let alist = heap.cons(cell, alist);
    let p = gp(heap, props);
    p.alist = alist;
    p.nested += i32::from(nested);
    a.touch(c, sym, alist);
    cell
}

/// `temporary_override`, `\once \override`'s: the push, marked `#t`, as
/// the token for [`matched_pop`].
pub fn temporary_override(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    sym: Value,
    path: Value,
    value: Value,
) -> Result<Value, lily_engine_scheme::Exception> {
    let cell = push(host, t, c, sym, path, value)?;
    let heap = host.heap();
    let Some(key) = heap.car(cell) else {
        return Ok(cell);
    };
    let Some(props) = check(heap, &t.arena.borrow(), c, sym) else {
        return Ok(cell);
    };
    let marked = heap.cons(Value::TRUE, cell);
    let alist = gp(heap, props).alist;
    let rest = heap.cdr(alist).unwrap_or(Value::NULL);
    let alist = heap.cons(marked, rest);
    let p = gp(heap, props);
    p.alist = alist;
    p.nested += i32::from(key.tag() == Tag::Symbol);
    t.arena.borrow().touch(c, sym, alist);
    Ok(marked)
}

/// Whether `path` is a list that starts with a symbol, or LilyPond's
/// programming error.
fn valid_path(heap: &Heap, path: Value) -> bool {
    heap.car(path).is_some_and(|s| s.tag() == Tag::Symbol)
}

/// `temporary_revert`, `\once \revert`'s: the entry of `path` above the
/// parent's hidden behind a `#f` cell, the token for [`matched_pop`].
pub fn temporary_revert(
    heap: &mut Heap,
    a: &mut Arena,
    c: CtxId,
    sym: Value,
    path: Value,
) -> Value {
    let Some(props) = check(heap, a, c, sym) else {
        return Value::NULL;
    };
    if !valid_path(heap, path) {
        return Value::NULL;
    }
    let (current, daddy) = {
        let p = gp(heap, props);
        (p.alist, p.based_on)
    };
    let nested = heap.cdr(path).is_some_and(|r| r.tag() == Tag::Pair);
    let tail = if nested {
        assoc_tail(heap, path, current, daddy)
    } else {
        heap.car(path)
            .and_then(|k| assq_tail(heap, k, current, daddy))
    };
    let Some(tail) = tail else {
        return Value::NULL;
    };
    let entry = heap.car(tail).unwrap_or(Value::NULL);
    let cell = heap.cons(Value::FALSE, entry);
    let after = heap.cdr(tail).unwrap_or(Value::NULL);
    let newtail = heap.cons(cell, after);
    let alist = partial_list_copy(heap, current, tail, newtail);
    let p = gp(heap, props);
    p.alist = alist;
    p.nested += i32::from(!nested);
    a.touch(c, sym, alist);
    cell
}

/// `matched_pop`: the push or temporary revert whose token is `cell`
/// undone.
pub fn matched_pop(heap: &mut Heap, a: &Arena, c: CtxId, sym: Value, cell: Value) {
    if cell.tag() != Tag::Pair {
        return;
    }
    let Some(props) = check(heap, a, c, sym) else {
        return;
    };
    let (current, daddy) = {
        let p = gp(heap, props);
        (p.alist, p.based_on)
    };
    let mut p = current;
    while p != daddy && p.tag() == Tag::Pair {
        if heap.car(p) == Some(cell) {
            let key = heap.car(cell).unwrap_or(Value::NULL);
            let after = heap.cdr(p).unwrap_or(Value::NULL);
            if key == Value::FALSE {
                // A temporary revert: the entry it hid is back.
                let entry = heap.cdr(cell).unwrap_or(Value::NULL);
                let symbol = heap.car(entry).is_some_and(|k| k.tag() == Tag::Symbol);
                let newtail = heap.cons(entry, after);
                let alist = partial_list_copy(heap, current, p, newtail);
                let g = gp(heap, props);
                g.alist = alist;
                g.nested -= i32::from(symbol);
                a.touch(c, sym, alist);
                return;
            }
            let alist = partial_list_copy(heap, current, p, after);
            let g = gp(heap, props);
            g.alist = alist;
            g.nested -= i32::from(key.tag() != Tag::Symbol);
            a.touch(c, sym, alist);
            return;
        }
        p = heap.cdr(p).unwrap_or(Value::NULL);
    }
}

/// `pop`, `\revert`'s: the newest entry of `path` above the parent's
/// removed; none left, and `c` no longer holds `sym`.
pub fn pop(heap: &mut Heap, a: &mut Arena, c: CtxId, sym: Value, path: Value) {
    let Some(props) = check(heap, a, c, sym) else {
        return;
    };
    if !valid_path(heap, path) {
        return;
    }
    let (mut current, daddy) = {
        let p = gp(heap, props);
        (p.alist, p.based_on)
    };
    if heap.cdr(path).is_some_and(|r| r.tag() == Tag::Pair) {
        let old = current;
        current = evict_from_alist(heap, path, current, daddy);
        if current == old {
            return;
        }
        gp(heap, props).nested -= 1;
    } else {
        let key = heap.car(path).unwrap_or(Value::NULL);
        current = evict_from_alist(heap, key, current, daddy);
    }
    if current == daddy {
        a.unset(c, sym);
        return;
    }
    gp(heap, props).alist = current;
    a.touch(c, sym, current);
}

/// `updated`: the effective properties of `sym` seen from `c`, its
/// parents' changes taken in.
pub fn updated(heap: &mut Heap, a: &Arena, c: CtxId, sym: Value) -> Value {
    let Some((at, props)) = a.where_defined(c, sym) else {
        return Value::NULL;
    };
    if heap.foreign::<GrobProperties>(props).is_none() {
        return Value::NULL;
    }
    let daddy = match a.parent(at) {
        Some(p) => updated(heap, a, p, sym),
        None => Value::NULL,
    };
    let (based_on, mut alist) = {
        let p = gp(heap, props);
        (p.based_on, p.alist)
    };
    if based_on != daddy {
        alist = partial_list_copy(heap, alist, based_on, daddy);
        let p = gp(heap, props);
        p.based_on = daddy;
        p.alist = alist;
    }
    let p = gp(heap, props);
    if p.cooked_from == alist {
        return p.cooked;
    }
    let nested = p.nested;
    let cooked = nalist_to_alist(heap, alist, nested);
    let p = gp(heap, props);
    p.cooked_from = alist;
    p.cooked = cooked;
    cooked
}

// --- nested-property.cc ----------------------------------------------------

/// `partial_list_copy`: the items of `list` before `tail`, then `newtail`.
fn partial_list_copy(heap: &mut Heap, list: Value, tail: Value, newtail: Value) -> Value {
    let mut items = Vec::new();
    let mut p = list;
    while p != tail {
        let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) else {
            break;
        };
        items.push(item);
        p = rest;
    }
    let mut out = newtail;
    for &item in items.iter().rev() {
        out = heap.cons(item, out);
    }
    out
}

/// `assq_tail`: the pair of `alist` before `end` whose entry's key is `key`.
fn assq_tail(heap: &Heap, key: Value, alist: Value, end: Value) -> Option<Value> {
    let mut p = alist;
    while p != end {
        let (entry, rest) = (heap.car(p)?, heap.cdr(p)?);
        if heap.car(entry) == Some(key) {
            return Some(p);
        }
        p = rest;
    }
    None
}

/// `assoc_tail`: as [`assq_tail`], keys compared by `equal?` (`eqv?` for
/// numbers and characters).
fn assoc_tail(heap: &Heap, key: Value, alist: Value, end: Value) -> Option<Value> {
    let mut p = alist;
    while p != end {
        let (entry, rest) = (heap.car(p)?, heap.cdr(p)?);
        if heap.car(entry).is_some_and(|k| heap.equal(k, key)) {
            return Some(p);
        }
        p = rest;
    }
    None
}

/// `evict_from_alist`: `alist` without its first entry of `key` before `end`.
fn evict_from_alist(heap: &mut Heap, key: Value, alist: Value, end: Value) -> Value {
    match assoc_tail(heap, key, alist, end) {
        Some(p) => {
            let rest = heap.cdr(p).unwrap_or(Value::NULL);
            partial_list_copy(heap, alist, p, rest)
        }
        None => alist,
    }
}

/// `nested_create_alist`: `((a (b . value)))` for the path `(a b)`.
fn nested_create_alist(heap: &mut Heap, path: Value, value: Value) -> Value {
    let keys = items(heap, path);
    let mut out = value;
    for &k in keys.iter().rev() {
        let entry = heap.cons(k, out);
        out = heap.list(&[entry]);
    }
    out
}

/// `nested_property_alist`: `alist` with `value` set at `path`.
fn nested_property_alist(heap: &mut Heap, alist: Value, path: Value, value: Value) -> Value {
    let (Some(key), Some(rest)) = (heap.car(path), heap.cdr(path)) else {
        return alist;
    };
    if rest.tag() == Tag::Pair {
        let inner = match assoc_tail(heap, key, alist, Value::NULL) {
            None => {
                let inner = nested_create_alist(heap, rest, value);
                let entry = heap.cons(key, inner);
                return heap.cons(entry, alist);
            }
            Some(at) => at,
        };
        let old = heap
            .car(inner)
            .and_then(|e| heap.cdr(e))
            .unwrap_or(Value::NULL);
        let new = nested_property_alist(heap, old, rest, value);
        let after = heap.cdr(inner).unwrap_or(Value::NULL);
        let others = partial_list_copy(heap, alist, inner, after);
        let entry = heap.cons(key, new);
        return heap.cons(entry, others);
    }
    let entry = heap.cons(key, value);
    heap.cons(entry, alist)
}

fn items(heap: &Heap, list: Value) -> Vec<Value> {
    let mut out = Vec::new();
    let mut p = list;
    while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        out.push(item);
        p = rest;
    }
    out
}

/// `nalist_to_alist`: the first entries of `nalist` that are not plain
/// properties (`nested` of them) folded: temporary cells opened or dropped,
/// and nested overrides applied to their properties.
fn nalist_to_alist(heap: &mut Heap, mut nalist: Value, mut nested: i32) -> Value {
    if nested == 0 {
        return nalist;
    }
    let mut copied: Vec<Value> = Vec::new();
    // Per property, its nested entries, the oldest first, as they apply.
    let mut partials: Vec<(Value, Vec<Value>)> = Vec::new();
    let apply = |heap: &mut Heap, mut value: Value, elts: &[Value]| {
        for &elt in elts {
            let path = heap
                .car(elt)
                .and_then(|p| heap.cdr(p))
                .unwrap_or(Value::NULL);
            let v = heap.cdr(elt).unwrap_or(Value::NULL);
            value = nested_property_alist(heap, value, path, v);
        }
        value
    };
    while nested > 0 {
        let (Some(mut elt), Some(rest)) = (heap.car(nalist), heap.cdr(nalist)) else {
            break;
        };
        nalist = rest;
        let mut key = heap.car(elt).unwrap_or(Value::NULL);
        if key.tag() != Tag::Symbol {
            nested -= 1;
        }
        if key == Value::FALSE {
            continue;
        }
        if key == Value::TRUE {
            elt = heap.cdr(elt).unwrap_or(Value::NULL);
            key = heap.car(elt).unwrap_or(Value::NULL);
        }
        if key.tag() == Tag::Pair {
            let head = heap.car(key).unwrap_or(Value::NULL);
            match partials.iter_mut().find(|(k, _)| *k == head) {
                Some((_, elts)) => elts.insert(0, elt),
                None => partials.insert(0, (head, alloc::vec![elt])),
            }
            continue;
        }
        match partials.iter().position(|(k, _)| *k == key) {
            Some(at) => {
                let (_, elts) = partials.remove(at);
                let value = heap.cdr(elt).unwrap_or(Value::NULL);
                let value = apply(heap, value, &elts);
                copied.push(heap.cons(key, value));
            }
            None => copied.push(elt),
        }
    }
    for (key, elts) in partials {
        let base = assq_tail(heap, key, nalist, Value::NULL)
            .and_then(|p| heap.car(p))
            .and_then(|e| heap.cdr(e))
            .unwrap_or(Value::NULL);
        let value = apply(heap, base, &elts);
        copied.push(heap.cons(key, value));
    }
    let mut out = nalist;
    for &entry in copied.iter().rev() {
        out = heap.cons(entry, out);
    }
    out
}

// --- the Scheme face --------------------------------------------------------

/// `(ly:grob-properties? x)`.
pub(crate) fn is_grob_properties(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<GrobProperties>(cx, args[0])
}

/// `(ly:make-grob-properties alist)`.
pub(crate) fn ly_make_grob_properties(cx: &mut Cx, args: &[Value]) -> R {
    if !matches!(args[0].tag(), Tag::Pair | Tag::Null) {
        return Err(cx.wrong_type("ly:make-grob-properties", 1, args[0], "list"));
    }
    Ok(GrobProperties::make(cx.heap_mut(), args[0]))
}

/// The arena and number of the context `v` and the symbol `grob`, checked.
fn context_and_grob(
    cx: &mut Cx,
    subr: &str,
    args: &[Value],
) -> Result<(alloc::rc::Rc<core::cell::RefCell<Arena>>, CtxId), lily_engine_scheme::Thrown> {
    let found = crate::context::handle_of(cx.heap(), args[0]);
    let Some(found) = found else {
        return Err(cx.wrong_type(subr, 1, args[0], "Context"));
    };
    if args[1].tag() != Tag::Symbol {
        return Err(cx.wrong_type(subr, 2, args[1], "symbol"));
    }
    Ok(found)
}

/// `(ly:context-grob-definition context grob)`: [`updated`].
pub(crate) fn ly_context_grob_definition(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context_and_grob(cx, "ly:context-grob-definition", args)?;
    let a = a.borrow();
    Ok(updated(cx.heap_mut(), &a, c, args[1]))
}

/// `(ly:context-matched-pop-property context grob cell)`.
pub(crate) fn ly_context_matched_pop_property(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context_and_grob(cx, "ly:context-matched-pop-property", args)?;
    matched_pop(cx.heap_mut(), &a.borrow(), c, args[1], args[2]);
    Ok(Value::UNSPECIFIED)
}

/// `(%context-grob-create! context grob)`: whether `context` has, or now
/// has, grob properties of `grob`.
pub(crate) fn context_grob_create(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context_and_grob(cx, "ly:context-pushpop-property", args)?;
    let made = create(cx.heap_mut(), &mut a.borrow_mut(), c, args[1]);
    Ok(Value::bool(made.is_some()))
}

/// `(%context-push-property! context grob path value)`, after the type
/// check: the cell.
pub(crate) fn context_push_property(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context_and_grob(cx, "ly:context-pushpop-property", args)?;
    Ok(push_unchecked(
        cx.heap_mut(),
        &mut a.borrow_mut(),
        c,
        args[1],
        args[2],
        args[3],
    ))
}

/// `(%context-pop-property! context grob path)`.
pub(crate) fn context_pop_property(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context_and_grob(cx, "ly:context-pushpop-property", args)?;
    pop(cx.heap_mut(), &mut a.borrow_mut(), c, args[1], args[2]);
    Ok(Value::UNSPECIFIED)
}
