//! LilyPond's `Context_mod` (`context-mod.cc`, `context-mod-scheme.cc`): the
//! modifications of a context as `\with { … }` gives them (P6, 02 §6), a
//! list such as `((consists "Foo") (assign instrumentName "x"))` that a
//! context definition (N3, `context_def.rs`) or a context made by `\new`
//! applies.
//!
//! It holds its list last first, as LilyPond's does, and gives it in order,
//! a fresh list each time ([`context_mods`]); a copy shares the list and
//! adds to its own ([`copy_context_mod`], `smobbed_copy`).
//! `ly:context-mod-apply!` applies its property operations to a context,
//! N3's instances of part 3: until then no value is a context.

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Cx, Foreign, Heap, ImageReader, ImageWriter, Tracer, Value};

use crate::R;

/// A context modification: its mods, last first.
pub struct ContextMod {
    mods: Value,
}

impl ContextMod {
    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<ContextMod> {
        Some(ContextMod { mods: r.value()? })
    }
}

/// The elements of the list `list`, as far as it is one.
fn items(heap: &Heap, list: Value) -> Vec<Value> {
    let mut items = Vec::new();
    let mut p = list;
    while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        items.push(item);
        p = rest;
    }
    items
}

/// `scm_reverse` of what is a list.
fn reverse(heap: &mut Heap, list: Value) -> Value {
    items(heap, list)
        .into_iter()
        .fold(Value::NULL, |rest, item| heap.cons(item, rest))
}

impl Foreign for ContextMod {
    /// A copy for S5's undo log (MS6 step 3, D235): a parse's segment made
    /// it, and what changes it after (a translation) is undone with it.
    fn snapshot(&self) -> Option<alloc::boxed::Box<dyn Foreign>> {
        Some(alloc::boxed::Box::new(ContextMod { mods: self.mods }))
    }

    fn type_name(&self) -> &'static str {
        "Context_mod"
    }

    /// `#<Context_mod ` and its mods in order, displayed, then `>`.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<Context_mod (");
        for (i, item) in items(heap, self.mods).into_iter().rev().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            heap.display(item, out);
        }
        out.push_str(")>");
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.mods);
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.value(self.mods);
        true
    }
}

/// `Context_mod (mod_list)`: a context modification of the mods of `list`,
/// in order.
pub fn make_context_mod(heap: &mut Heap, list: Value) -> Value {
    let mods = reverse(heap, list);
    heap.wrap(ContextMod { mods })
}

/// `smobbed_copy`: a context modification with `value`'s mods, to which
/// what is added is its own; None for what is none.
pub fn copy_context_mod(heap: &mut Heap, value: Value) -> Option<Value> {
    let mods = heap.foreign::<ContextMod>(value)?.mods;
    Some(heap.wrap(ContextMod { mods }))
}

/// `add_context_mod`: `m` added after the mods of `value`, a context
/// modification.
pub fn add_context_mod(heap: &mut Heap, value: Value, m: Value) {
    let Some(mods) = heap.foreign::<ContextMod>(value).map(|c| c.mods) else {
        return;
    };
    let mods = heap.cons(m, mods);
    if let Some(c) = heap.foreign_mut::<ContextMod>(value) {
        c.mods = mods;
    }
}

/// `get_mods`: the mods of `value`, a context modification, in order, as a
/// fresh list; None for what is none.
pub fn context_mods(heap: &mut Heap, value: Value) -> Option<Value> {
    let mods = heap.foreign::<ContextMod>(value)?.mods;
    Some(reverse(heap, mods))
}

/// `(ly:make-context-mod [mod-list])`.
pub(crate) fn ly_make_context_mod(cx: &mut Cx, args: &[Value]) -> R {
    let list = match args.first() {
        Some(&list) => {
            // `ly_cheap_is_list`.
            let ok = list == Value::NULL || cx.heap().car(list).is_some();
            if !ok {
                return Err(cx.wrong_type("ly:make-context-mod", 1, list, "list"));
            }
            list
        }
        None => Value::NULL,
    };
    Ok(make_context_mod(cx.heap_mut(), list))
}

/// `(ly:context-mod? x)`.
pub(crate) fn is_context_mod(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<ContextMod>(cx, args[0])
}

/// `LY_ASSERT_SMOB (Context_mod, …)`.
fn check(cx: &mut Cx, subr: &str, v: Value) -> Result<(), lily_engine_scheme::Thrown> {
    match cx.heap().foreign::<ContextMod>(v) {
        Some(_) => Ok(()),
        None => Err(cx.wrong_type(subr, 1, v, "Context_mod")),
    }
}

/// `(ly:add-context-mod contextmods modification)`.
pub(crate) fn ly_add_context_mod(cx: &mut Cx, args: &[Value]) -> R {
    check(cx, "ly:add-context-mod", args[0])?;
    add_context_mod(cx.heap_mut(), args[0], args[1]);
    Ok(Value::UNSPECIFIED)
}

/// `(ly:get-context-mods contextmod)`.
pub(crate) fn ly_get_context_mods(cx: &mut Cx, args: &[Value]) -> R {
    check(cx, "ly:get-context-mods", args[0])?;
    Ok(context_mods(cx.heap_mut(), args[0]).unwrap_or(Value::NULL))
}

/// `(ly:context-mod-apply! context mod)`: `LY_ASSERT_SMOB (Context, …)`
/// first, which no value passes until part 3 makes contexts (D73), then
/// `apply_property_operations` of `mod`'s mods to the context.
pub(crate) fn ly_context_mod_apply(cx: &mut Cx, args: &[Value]) -> R {
    Err(cx.wrong_type("ly:context-mod-apply!", 1, args[0], "Context"))
}
