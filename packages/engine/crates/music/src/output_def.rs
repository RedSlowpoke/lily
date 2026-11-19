//! LilyPond's `Output_def` (`output-def.cc`, `output-def-scheme.cc`), as far
//! as context definitions need it (N3, D73): a scope, a module in which
//! `output-def-kind` names its kind and each context definition is bound
//! to its name (`assign_context_def`). The parser makes them (`\paper`,
//! `\layout`, `\midi`); here are their Scheme face and the lookups of the
//! definitions in them. A score's `\layout` copies its default's scope
//! (`lily_engine_parser`'s `output_def.rs`). An output definition has a
//! parent only as a translation's scaled layout and its book's paper have
//! one (`score.cc:135–142`, [`scale_output_def`], `book.rs`'
//! [`crate::book_scores`]); its variables are looked up through it
//! ([`lookup_variable`]), and no image holds one with a parent.

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{
    Cx, Exception, Foreign, Heap, ImageReader, ImageWriter, Tag, Tracer, Value, Vm,
};

use crate::R;
use crate::context::Host;
use crate::context_def::ContextDef;

/// An output definition: its scope, a module, and its parent (`parent_`),
/// another output definition, if it has one.
pub struct OutputDef {
    pub scope: Value,
    pub parent: Option<Value>,
}

impl OutputDef {
    /// An output definition of the scope `scope`, with no parent.
    pub fn new(scope: Value) -> OutputDef {
        OutputDef {
            scope,
            parent: None,
        }
    }

    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<OutputDef> {
        Some(OutputDef::new(r.value()?))
    }
}

impl Foreign for OutputDef {
    /// A copy for S5's undo log (MS6 step 3, D235): a parse's segment made
    /// it, and what changes it after (a translation) is undone with it.
    fn snapshot(&self) -> Option<alloc::boxed::Box<dyn Foreign>> {
        Some(alloc::boxed::Box::new(OutputDef {
            scope: self.scope,
            parent: self.parent,
        }))
    }

    fn type_name(&self) -> &'static str {
        "Output_def"
    }

    /// `#< Output_def>`, as LilyPond prints its class's name.
    fn print(&self, _: &Heap, out: &mut String) {
        out.push_str("#< Output_def>");
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.scope);
        if let Some(parent) = self.parent {
            tracer.trace(parent);
        }
    }

    /// Its scope; one with a parent is a translation's, which no image
    /// holds, and fails the write.
    fn save(&self, out: &mut ImageWriter) -> bool {
        if self.parent.is_some() {
            return false;
        }
        out.value(self.scope);
        true
    }
}

/// `Output_def::lookup_variable` (`output-def.cc:109–120`): `sym` in the
/// scope of `def`, its own or imported, as `module-variable` finds it,
/// else in its parent's, up to the root; None if none binds it.
pub fn lookup_variable(cx: &Cx, def: Value, sym: Value) -> Option<Value> {
    let mut at = Some(def);
    while let Some(def) = at.and_then(|d| cx.heap().foreign::<OutputDef>(d)) {
        if let Some(v) = cx.module_ref(def.scope, sym) {
            return Some(v);
        }
        at = def.parent;
    }
    None
}

/// [`lookup_variable`] through a [`Host`].
pub fn host_lookup_variable(host: &mut dyn Host, def: Value, sym: Value) -> Option<Value> {
    let mut at = Some(def);
    while let Some((scope, parent)) = at.and_then(|d| {
        host.heap()
            .foreign::<OutputDef>(d)
            .map(|d| (d.scope, d.parent))
    }) {
        if let Some(v) = host.module_ref(scope, sym) {
            return Some(v);
        }
        at = parent;
    }
    None
}

/// The root of `def`'s parents, as `find_scaled_font` and
/// `find_pango_font` recurse to it (`paper-def.cc:56–57`, `:80–81`):
/// `def` itself if it has none.
pub fn root(heap: &Heap, mut def: Value) -> Value {
    while let Some(parent) = heap.foreign::<OutputDef>(def).and_then(|d| d.parent) {
        def = parent;
    }
    def
}

/// Sets the parent of the output definition `def` (`def->parent_ = …`).
pub fn set_parent(heap: &mut Heap, def: Value, parent: Value) {
    if let Some(def) = heap.foreign_mut::<OutputDef>(def) {
        def.parent = Some(parent);
    }
}

/// `scale_output_def` (`paper-def.cc:99–107`) with the `scale-layout` of
/// `paper.scm` (`:417–436`) it calls, in Rust (D110): a clone of `def`,
/// with no parent as `Output_def`'s copy constructor makes it, in which
/// each of `dimension-variables` (looked up through `def`'s parents)
/// that `def`'s scope binds to a number, as `module-variable` finds it,
/// is that number divided by `scale` (`/`); and `cloned` set to `#t`.
pub fn scale_output_def(vm: &mut Vm, def: Value, scale: f64) -> Value {
    let old = vm
        .heap()
        .foreign::<OutputDef>(def)
        .map_or(Value::FALSE, |d| d.scope);
    let scope = vm.make_module(&["lily"]).unwrap_or(Value::FALSE);
    vm.module_copy(scope, old);
    let new = vm.heap_mut().wrap(OutputDef::new(scope));
    let key = vm.heap_mut().intern("dimension-variables");
    let mut vars = host_lookup_variable(vm, def, key).unwrap_or(Value::NULL);
    let scale = vm.heap_mut().flonum(scale);
    while let (Some(var), Some(rest)) = (vm.heap().car(vars), vm.heap().cdr(vars)) {
        vars = rest;
        let Some(name) = vm.heap().name(var).map(String::from) else {
            continue;
        };
        let Some(val) = vm.module_value(old, &name) else {
            continue;
        };
        if vm.heap().is_number(val)
            && let Ok(scaled) = vm.heap_mut().div(val, scale)
        {
            vm.module_define(scope, &name, scaled);
        }
    }
    vm.module_define(scope, "cloned", Value::TRUE);
    new
}

/// `Output_def::normalize` (`output-def.cc:134–243`), borrowed from MS3
/// step 35 by step 32 (D129), whose line reads `line-width`: `def`'s
/// `left-margin`, `right-margin` and `line-width` set from its
/// `paper-width` and its margins (`outer-` and `inner-` for `two-sided`),
/// the default margins where none is given, the margins centring a given
/// `line-width` alone, and, under `check-consistency`, the defaults with
/// LilyPond's warning when they do not add up or a margin is negative.
/// LilyPond's programming error and nothing set when `paper-width` or a
/// default margin is unbound; `scm_to_double`'s type error for a value
/// that is no real.
pub fn normalize(vm: &mut Vm, def: Value) -> Result<(), Exception> {
    let mut var = |name: &str| {
        let key = vm.heap_mut().intern(name);
        host_lookup_variable(vm, def, key).unwrap_or(Value::UNDEFINED)
    };
    let scm_paper_width = var("paper-width");
    let twosided = var("two-sided") == Value::TRUE;
    // We don't distinguish between outer-margin / left-margin and so on
    // until page-stencil positioning in page.scm
    let (left, right) = if twosided {
        ("outer-margin", "inner-margin")
    } else {
        ("left-margin", "right-margin")
    };
    let scm_left_margin_default = var(&alloc::format!("{left}-default-scaled"));
    let scm_left_margin = var(left);
    let scm_right_margin_default = var(&alloc::format!("{right}-default-scaled"));
    let scm_right_margin = var(right);
    let scm_line_width = var("line-width");
    let scm_binding_offset = var("binding-offset");
    let check_consistency = var("check-consistency") == Value::TRUE;

    if [
        scm_paper_width,
        scm_left_margin_default,
        scm_right_margin_default,
    ]
    .contains(&Value::UNDEFINED)
    {
        return crate::context::diagnose(
            vm,
            "programming-error",
            "called normalize () on paper with missing settings",
        );
    }
    let paper_width = real(vm, scm_paper_width)?;
    let left_margin_default = real(vm, scm_left_margin_default)?;
    let right_margin_default = real(vm, scm_right_margin_default)?;

    let line_width_default = paper_width - left_margin_default - right_margin_default;
    let binding_offset = match twosided {
        true if vm.heap().is_real(scm_binding_offset) => real(vm, scm_binding_offset)?,
        _ => 0.0,
    };

    let (mut line_width, mut left_margin, mut right_margin);
    if scm_line_width == Value::UNDEFINED {
        left_margin = match scm_left_margin {
            Value::UNDEFINED => left_margin_default,
            v => real(vm, v)?,
        };
        right_margin = match scm_right_margin {
            Value::UNDEFINED => right_margin_default,
            v => real(vm, v)?,
        } + binding_offset;
        line_width = paper_width - left_margin - right_margin;
    } else {
        line_width = real(vm, scm_line_width)?;
        if scm_left_margin == Value::UNDEFINED {
            // Vertically center systems if only line-width is given
            if scm_right_margin == Value::UNDEFINED {
                left_margin = (paper_width - line_width) / 2.0;
                right_margin = left_margin;
            } else {
                right_margin = real(vm, scm_right_margin)? + binding_offset;
                left_margin = paper_width - line_width - right_margin;
            }
        } else {
            left_margin = real(vm, scm_left_margin)?;
            right_margin = match scm_right_margin {
                Value::UNDEFINED => paper_width - line_width - left_margin,
                v => real(vm, v)?,
            } + binding_offset;
        }
    }

    if check_consistency {
        // Consistency checks. If values don't match, set defaults.
        let warning = if (paper_width - line_width - left_margin - right_margin).abs() > 1e-6 {
            Some("margins do not fit with line-width, setting default values")
        } else if left_margin < 0.0 || right_margin < 0.0 {
            Some("systems run off the page due to improper paper settings, setting default values")
        } else {
            None
        };
        if let Some(text) = warning {
            line_width = line_width_default;
            left_margin = left_margin_default;
            right_margin = right_margin_default;
            crate::context::diagnose(vm, "warning", text)?;
        }
    }

    let scope = vm
        .heap()
        .foreign::<OutputDef>(def)
        .map_or(Value::FALSE, |d| d.scope);
    for (name, x) in [
        ("left-margin", left_margin),
        ("right-margin", right_margin),
        ("line-width", line_width),
    ] {
        let x = vm.heap_mut().flonum(x);
        vm.module_define(scope, name, x);
    }
    Ok(())
}

/// `from_scm<double>` of `name` in `def`, looked up through its parents:
/// `scm_to_double`'s type error for what is no real.
pub(crate) fn real_variable(vm: &mut Vm, def: Value, name: &str) -> Result<f64, Exception> {
    let key = vm.heap_mut().intern(name);
    let v = host_lookup_variable(vm, def, key).unwrap_or(Value::UNDEFINED);
    real(vm, v)
}

/// `from_scm<double>` of `v`: `scm_to_double`'s type error for what is no
/// real.
fn real(vm: &mut Vm, v: Value) -> Result<f64, Exception> {
    let heap = vm.heap_mut();
    match heap.to_f64(v) {
        Some(x) if heap.is_real(v) => Ok(x),
        _ => {
            let key = heap.intern("wrong-type-arg");
            let expecting = heap.string("real number");
            let message = heap.string("Wrong type (expecting ~A): ~S");
            let items = heap.list(&[expecting, v]);
            let rest = heap.list(&[v]);
            let args = heap.list(&[Value::FALSE, message, items, rest]);
            Err(Exception { key, args })
        }
    }
}

/// `LY_ASSERT_SMOB (Output_def, …)`: the scope of `v`.
fn scope(cx: &mut Cx, subr: &str, v: Value) -> Result<Value, lily_engine_scheme::Thrown> {
    match cx.heap().foreign::<OutputDef>(v) {
        Some(def) => Ok(def.scope),
        None => Err(cx.wrong_type(subr, 1, v, "Output_def")),
    }
}

/// `LY_ASSERT_TYPE (ly_is_symbol, …)`.
fn symbol(cx: &mut Cx, subr: &str, at: usize, v: Value) -> Result<(), lily_engine_scheme::Thrown> {
    match v.tag() {
        Tag::Symbol => Ok(()),
        _ => Err(cx.wrong_type(subr, at, v, "symbol")),
    }
}

/// The context definitions of the scope `scope` bound to their own names,
/// as `ly_module_2_alist` and the loops over it find them: each name and
/// definition.
fn context_defs(cx: &Cx, scope: Value) -> Vec<(Value, Value)> {
    let heap = cx.heap();
    let mut defs = cx.module_locals(scope).unwrap_or_default();
    defs.retain(|&(name, def)| {
        heap.foreign::<ContextDef>(def)
            .is_some_and(|d| d.name() == name)
    });
    defs
}

/// `find_context_def`: the context definition `name` names in `def`, an
/// output definition, or its parents, or None.
pub fn find_context_def(cx: &Cx, def: Value, name: Value) -> Option<Value> {
    lookup_variable(cx, def, name).filter(|&v| cx.heap().foreign::<ContextDef>(v).is_some())
}

/// `(ly:output-def? x)`.
pub(crate) fn is_output_def(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<OutputDef>(cx, args[0])
}

/// `(ly:output-def-lookup def sym [val])`: `sym`'s value in `def`'s scope
/// or its parents', else `val`, else `'()`.
pub(crate) fn ly_output_def_lookup(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:output-def-lookup";
    scope(cx, SUBR, args[0])?;
    symbol(cx, SUBR, 2, args[1])?;
    Ok(lookup_variable(cx, args[0], args[1])
        .or(args.get(2).copied())
        .unwrap_or(Value::NULL))
}

/// `(ly:paper-get-number def sym)` (`output-def-scheme.cc:179–188`): `sym`
/// in `def` or its parents as a double (`Output_def::get_dimension`),
/// `scm_to_double`'s error if it is no real number, unbound included.
pub(crate) fn ly_paper_get_number(cx: &mut Cx, args: &[Value]) -> R {
    scope(cx, "ly:paper-get-number", args[0])?;
    let v = lookup_variable(cx, args[0], args[1]).unwrap_or(Value::UNDEFINED);
    match cx.heap().to_f64(v) {
        Some(x) if cx.heap().is_real(v) => Ok(cx.heap_mut().flonum(x)),
        _ => Err(cx.wrong_type_vm(None, v, "real number")),
    }
}

/// `(ly:output-def-clone def)`: a new output definition whose scope is a
/// copy of `def`'s, with no parent (`Output_def`'s copy constructor).
pub(crate) fn ly_output_def_clone(cx: &mut Cx, args: &[Value]) -> R {
    let old = scope(cx, "ly:output-def-clone", args[0])?;
    let scope = cx.make_module(&["lily"]).unwrap_or(Value::FALSE);
    for (name, value) in cx.module_locals(old).unwrap_or_default() {
        cx.module_define(scope, name, value);
    }
    Ok(cx.heap_mut().wrap(OutputDef::new(scope)))
}

/// `(ly:make-output-def)`: a new output definition, its scope a module
/// that uses `(lily)` (`Output_def`'s constructor).
pub(crate) fn ly_make_output_def(cx: &mut Cx, _: &[Value]) -> R {
    let scope = cx.make_module(&["lily"]).unwrap_or(Value::FALSE);
    Ok(cx.heap_mut().wrap(OutputDef::new(scope)))
}

/// `(ly:output-def-scope def)`.
pub(crate) fn ly_output_def_scope(cx: &mut Cx, args: &[Value]) -> R {
    scope(cx, "ly:output-def-scope", args[0])
}

/// `(ly:output-def-parent def [default])`: its parent, else `default`, or
/// `()`.
pub(crate) fn ly_output_def_parent(cx: &mut Cx, args: &[Value]) -> R {
    scope(cx, "ly:output-def-parent", args[0])?;
    let parent = cx
        .heap()
        .foreign::<OutputDef>(args[0])
        .and_then(|d| d.parent);
    Ok(parent.or(args.get(1).copied()).unwrap_or(Value::NULL))
}

/// `(ly:output-def-set-variable! def sym val)`.
pub(crate) fn ly_output_def_set_variable(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:output-def-set-variable!";
    let scope = scope(cx, SUBR, args[0])?;
    symbol(cx, SUBR, 2, args[1])?;
    cx.module_define(scope, args[1], args[2]);
    Ok(Value::UNSPECIFIED)
}

/// `(ly:output-find-context-def output-def [context-name])`: the alist of
/// its context definitions, of those `context-name` names if it is given.
pub(crate) fn ly_output_find_context_def(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:output-find-context-def";
    let scope = scope(cx, SUBR, args[0])?;
    if let Some(&name) = args.get(1) {
        symbol(cx, SUBR, 2, name)?;
    }
    let defs = context_defs(cx, scope);
    let heap = cx.heap_mut();
    let mut list = Value::NULL;
    for (name, def) in defs {
        let named = args.get(1).is_none_or(|&wanted| {
            heap.foreign::<ContextDef>(def)
                .is_some_and(|d| d.is_alias(heap, wanted))
        });
        if named {
            let entry = heap.cons(name, def);
            list = heap.cons(entry, list);
        }
    }
    Ok(list)
}

/// `(ly:output-description output-def)`: each context definition's name
/// and its `to_alist`.
pub(crate) fn ly_output_description(cx: &mut Cx, args: &[Value]) -> R {
    let scope = scope(cx, "ly:output-description", args[0])?;
    let defs = context_defs(cx, scope);
    let heap = cx.heap_mut();
    let mut list = Value::NULL;
    for (name, def) in defs {
        let Some(alist) = crate::context_def::alist(heap, def) else {
            continue;
        };
        let entry = heap.cons(name, alist);
        list = heap.cons(entry, list);
    }
    Ok(list)
}
