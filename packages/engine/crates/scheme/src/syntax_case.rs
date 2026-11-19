//! `syntax-case` (chapter 03 §5.5 and §6, README §4.2, D273): the part on
//! demand in place of D60's refusal. `(lily)`'s four native macros stay.
//!
//! A syntax object is the datum itself, as the expander's renaming has it
//! (`syntax_rules.rs`): an identifier a macro introduced is an alias of the
//! symbol it stands for in the macro's module. A procedure transformer
//! (`define-syntax` of a `lambda`, evaluated where it is defined) is called
//! with the form, and every `syntax` template it instantiates during that
//! call renames its symbols as one expansion of `syntax-rules` does: each
//! once, in the macro's module ([`super::Transcription`]).
//!
//! Expanded here: `(syntax-case e (literal …) (pattern [fender] expr) …)` as
//! a match of each clause in turn at run time, its pattern variables bound
//! to fresh variables that only `syntax` reads; `(syntax template)` as the
//! template instantiated with them; `quasisyntax` as `with-syntax`, and
//! `with-syntax` as `syntax-case`, as psyntax has them. The patterns and the
//! templates are `syntax-rules`' own (`Rules`' matcher and transcriber).
//! At run time: `datum->syntax`, `syntax->datum`, `identifier?`,
//! `bound-identifier=?`, `free-identifier=?`, `generate-temporaries` and
//! `syntax-violation`.
//!
//! The expander reaches this module through [`HOOKS`]' pointers and Scheme
//! through its primitives, both in the function table, so that wasm-split
//! gives it to the `syntax-case` module (`scripts/split.mjs`), which the
//! host loads when a placeholder is first called.

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;

use super::{Env, Fail, Kw, Meaning, R};
use crate::compile::{FAILED, SyntaxError, syntax};
use crate::heap::Heap;
use crate::syntax_rules::{Binding, Renamer, Rules, base};
use crate::value::{Tag, Value};
use crate::vm::{Cx, Exception, Thrown, Vm};

type Form = fn(&mut Vm, Value, &mut Env) -> R<Value>;

/// The expander's forms of this part.
pub(crate) struct Hooks {
    pub syntax_case: Form,
    pub syntax: Form,
    pub quasisyntax: Form,
    pub with_syntax: Form,
}

pub(crate) static HOOKS: Hooks = Hooks {
    syntax_case,
    syntax: syntax_form,
    quasisyntax,
    with_syntax,
};

// --- the expander's forms

/// `(syntax-case e (literal …) clause …)`, each clause `(pattern expr)` or
/// `(pattern fender expr)`, as
///
/// ```scheme
/// ((lambda (tmp)
///    ((lambda (next)
///       ((lambda (m) (if m ((lambda (v …) (if fender expr (next))) (vector-ref m 0) …) (next)))
///        (%syntax-case-match tmp 'pattern '(literal …))))
///     (lambda () <the next clause>)))
///  e)
/// ```
///
/// the last clause's next `(%syntax-case-fail tmp)`.
fn syntax_case(vm: &mut Vm, x: Value, env: &mut Env) -> R<Value> {
    let bad = || -> Fail { syntax(FAILED, x, Some(x)).into() };
    let items = vm.items(x).ok_or_else(bad)?;
    let [_, e, literals, ref clauses @ ..] = items[..] else {
        return Err(bad());
    };
    let literals = vm
        .items(literals)
        .filter(|l| l.iter().all(|v| v.tag() == Tag::Symbol))
        .ok_or_else(bad)?;
    let mut parsed = Vec::new();
    for &clause in clauses {
        match vm.items(clause).as_deref() {
            Some(&[pattern, expr]) => parsed.push((pattern, None, expr)),
            Some(&[pattern, fender, expr]) => parsed.push((pattern, Some(fender), expr)),
            _ => return Err(bad()),
        }
    }
    let e = vm.expand(e, env)?;
    let (lambda, if_, quote) = (vm.kw(Kw::Lambda), vm.kw(Kw::If), vm.kw(Kw::Quote));
    let tmp = vm.fresh("tmp");
    let fail = vm.guile("%syntax-case-fail");
    let mut chain = vm.list(&[fail, tmp]);
    let rules = Rules::matcher(&mut vm.cx.heap, literals.clone());
    let literals = vm.list(&literals);
    for &(pattern, fender, expr) in parsed.iter().rev() {
        let mut vars = Vec::new();
        rules.variables(&vm.cx.heap, &vm.cx.globals.aliases, pattern, 0, &mut vars);
        let fresh: Vec<Value> = vars
            .iter()
            .map(|&(v, _)| {
                let name = String::from(
                    vm.cx
                        .heap
                        .name(base(&vm.cx.globals.aliases, v))
                        .unwrap_or("v"),
                );
                vm.fresh(&name)
            })
            .collect();
        let mark = env.scope.len();
        for (&(v, depth), &f) in vars.iter().zip(&fresh) {
            let marker = vm
                .cx
                .heap
                .cons(f, Value::fixnum(depth as i64).unwrap_or(Value::FALSE));
            env.scope.push((v, marker));
        }
        let expanded = (|| -> R<(Option<Value>, Value)> {
            let fender = match fender {
                Some(f) => Some(vm.expand(f, env)?),
                None => None,
            };
            Ok((fender, vm.expand(expr, env)?))
        })();
        env.scope.truncate(mark);
        let (fender, expr) = expanded?;
        let (next, m) = (vm.fresh("next"), vm.fresh("m"));
        let call_next = vm.list(&[next]);
        let body = match fender {
            Some(f) => vm.form(if_, &[f, expr, call_next]),
            None => expr,
        };
        let formals = vm.list(&fresh);
        let inner = vm.form(lambda, &[formals, body]);
        let vector_ref = vm.guile("vector-ref");
        let mut call = alloc::vec![inner];
        for i in 0..fresh.len() {
            let index = Value::fixnum(i as i64).unwrap_or(Value::FALSE);
            call.push(vm.list(&[vector_ref, m, index]));
        }
        let call = vm.list(&call);
        let test = vm.form(if_, &[m, call, call_next]);
        let m_formals = vm.list(&[m]);
        let on_m = vm.form(lambda, &[m_formals, test]);
        let matcher = vm.guile("%syntax-case-match");
        let quoted_pattern = vm.list(&[quote, pattern]);
        let quoted_literals = vm.list(&[quote, literals]);
        let matched = vm.list(&[matcher, tmp, quoted_pattern, quoted_literals]);
        let try_ = vm.list(&[on_m, matched]);
        let next_formals = vm.list(&[next]);
        let on_next = vm.form(lambda, &[next_formals, try_]);
        let thunk = vm.form(lambda, &[Value::NULL, chain]);
        chain = vm.list(&[on_next, thunk]);
    }
    let tmp_formals = vm.list(&[tmp]);
    let outer = vm.form(lambda, &[tmp_formals, chain]);
    Ok(vm.list(&[outer, e]))
}

/// `(syntax template)`: `(quote template)` where it holds no pattern
/// variable, else `(%syntax 'template '((variable . depth) …) binding …)`,
/// after psyntax's checks of its ellipses.
fn syntax_form(vm: &mut Vm, x: Value, env: &mut Env) -> R<Value> {
    let Some(&[_, template]) = vm.items(x).as_deref() else {
        return Err(syntax(FAILED, x, Some(x)).into());
    };
    let mut used: Vec<(Value, Value, usize)> = Vec::new();
    pattern_variables(vm, template, env, &mut used);
    let quote = vm.kw(Kw::Quote);
    let quoted = vm.list(&[quote, template]);
    if used.is_empty() && !has_symbol(&vm.cx.heap, template) {
        return Ok(quoted);
    }
    let rules = Rules::matcher(&mut vm.cx.heap, Vec::new());
    let vars: Vec<(Value, usize)> = used.iter().map(|&(v, _, d)| (v, d)).collect();
    if let Err(message) = rules.check(
        &vm.cx.heap,
        &vm.cx.globals.aliases,
        template,
        0,
        &vars,
        false,
    ) {
        return Err(SyntaxError {
            who: Some("syntax"),
            message,
            form: x,
            subform: Value::FALSE,
            at: Some(x),
        }
        .into());
    }
    let mut pairs = Vec::new();
    for &(v, _, d) in &used {
        let depth = Value::fixnum(d as i64).unwrap_or(Value::FALSE);
        pairs.push(vm.cx.heap.cons(v, depth));
    }
    let pairs = vm.list(&pairs);
    let quoted_pairs = vm.list(&[quote, pairs]);
    let mut call = alloc::vec![vm.guile("%syntax"), quoted, quoted_pairs];
    call.extend(used.iter().map(|&(_, f, _)| f));
    Ok(vm.list(&call))
}

/// The pattern variables `t` names, each once: its symbol in `t`, the
/// variable its binding is in, and its depth.
fn pattern_variables(vm: &Vm, t: Value, env: &Env, out: &mut Vec<(Value, Value, usize)>) {
    match t.tag() {
        Tag::Symbol => {
            if out.iter().any(|&(v, _, _)| v == t) {
                return;
            }
            if let Meaning::Pattern(marker) = vm.meaning(t, env) {
                let heap = &vm.cx.heap;
                let variable = heap.car(marker).unwrap_or(Value::FALSE);
                let depth = heap.cdr(marker).and_then(|d| d.as_fixnum()).unwrap_or(0);
                out.push((t, variable, depth as usize));
            }
        }
        Tag::Pair | Tag::Vector => {
            for item in parts(&vm.cx.heap, t) {
                pattern_variables(vm, item, env, out);
            }
        }
        _ => {}
    }
}

/// A pair's or a vector's items, a list's tail among them.
fn parts(heap: &Heap, t: Value) -> Vec<Value> {
    if t.tag() == Tag::Vector {
        return heap.vector_items(t).unwrap_or_default().to_vec();
    }
    let mut out = Vec::new();
    let mut at = t;
    while let (Some(car), Some(cdr)) = (heap.car(at), heap.cdr(at)) {
        out.push(car);
        at = cdr;
    }
    out.push(at);
    out
}

fn has_symbol(heap: &Heap, t: Value) -> bool {
    match t.tag() {
        Tag::Symbol => true,
        Tag::Pair | Tag::Vector => parts(heap, t).into_iter().any(|i| has_symbol(heap, i)),
        _ => false,
    }
}

/// `(quasisyntax template)` as psyntax has it: each `(unsyntax e)` at its
/// level a fresh pattern variable bound to `e`, each `(unsyntax-splicing
/// e)` in a list one followed by `...`, all bound by `with-syntax`.
fn quasisyntax(vm: &mut Vm, x: Value, env: &mut Env) -> R<Value> {
    let Some(&[_, template]) = vm.items(x).as_deref() else {
        return Err(syntax(FAILED, x, Some(x)).into());
    };
    let mut bindings = Vec::new();
    let template = quasi(vm, template, 0, env, &mut bindings);
    let syntax_kw = vm.kw(Kw::Syntax);
    let body = vm.list(&[syntax_kw, template]);
    if bindings.is_empty() {
        return vm.expand(body, env);
    }
    let bindings = vm.list(&bindings);
    let with = vm.kw(Kw::WithSyntax);
    let form = vm.list(&[with, bindings, body]);
    vm.expand(form, env)
}

fn quasi(vm: &mut Vm, t: Value, level: usize, env: &Env, bindings: &mut Vec<Value>) -> Value {
    match t.tag() {
        Tag::Pair => {
            let keyword = |vm: &Vm, v: Value| match vm.cx.heap.car(v) {
                Some(head) if head.tag() == Tag::Symbol => match vm.meaning(head, env) {
                    Meaning::Keyword(kw) => Some(kw),
                    _ => None,
                },
                _ => None,
            };
            let operand = |vm: &Vm, v: Value| match vm.items(v).as_deref() {
                Some(&[_, e]) => Some(e),
                _ => None,
            };
            if let (Some(kw @ (Kw::Unsyntax | Kw::Quasisyntax)), Some(e)) =
                (keyword(vm, t), operand(vm, t))
            {
                let head = vm.car(t);
                if kw == Kw::Quasisyntax {
                    let inner = quasi(vm, e, level + 1, env, bindings);
                    return vm.list(&[head, inner]);
                }
                if level > 0 {
                    let inner = quasi(vm, e, level - 1, env, bindings);
                    return vm.list(&[head, inner]);
                }
                let v = vm.fresh("t");
                bindings.push(vm.list(&[v, e]));
                return v;
            }
            let (items, tail) = vm.split(t);
            let mut out = Vec::new();
            for item in items {
                if level == 0
                    && keyword(vm, item) == Some(Kw::UnsyntaxSplicing)
                    && let Some(e) = operand(vm, item)
                {
                    let v = vm.fresh("t");
                    let ellipsis = vm.cx.heap.intern("...");
                    let pattern = vm.list(&[v, ellipsis]);
                    bindings.push(vm.list(&[pattern, e]));
                    out.extend([v, ellipsis]);
                } else {
                    out.push(quasi(vm, item, level, env, bindings));
                }
            }
            let tail = quasi(vm, tail, level, env, bindings);
            vm.form_star(out[0], &out[1..], tail)
        }
        Tag::Vector => {
            let list = {
                let items = vm.cx.heap.vector_items(t).unwrap_or_default().to_vec();
                vm.list(&items)
            };
            let new = quasi(vm, list, level, env, bindings);
            let (items, _) = vm.split(new);
            vm.cx.heap.vector(items)
        }
        _ => t,
    }
}

/// `(with-syntax ((pattern e) …) body …)` as psyntax has it: `(let ()
/// body …)` for none, `(syntax-case e () (pattern (let () body …)))` for
/// one, and the list of all for more.
fn with_syntax(vm: &mut Vm, x: Value, env: &mut Env) -> R<Value> {
    let bad = || -> Fail { syntax(FAILED, x, Some(x)).into() };
    let items = vm.items(x).ok_or_else(bad)?;
    let [_, bindings, ref body @ ..] = items[..] else {
        return Err(bad());
    };
    if body.is_empty() {
        return Err(bad());
    }
    let mut pairs = Vec::new();
    for binding in vm.items(bindings).ok_or_else(bad)? {
        match vm.items(binding).as_deref() {
            Some(&[pattern, e]) => pairs.push((pattern, e)),
            _ => return Err(bad()),
        }
    }
    let let_ = vm.kw(Kw::Let);
    let body = vm.cx.heap.list(body);
    let body = vm.form_star(let_, &[Value::NULL], body);
    let form = match pairs[..] {
        [] => body,
        [(pattern, e)] => {
            let case = vm.kw(Kw::SyntaxCase);
            let clause = vm.list(&[pattern, body]);
            vm.list(&[case, e, Value::NULL, clause])
        }
        _ => {
            let case = vm.kw(Kw::SyntaxCase);
            let patterns: Vec<Value> = pairs.iter().map(|p| p.0).collect();
            let exprs: Vec<Value> = pairs.iter().map(|p| p.1).collect();
            let list = vm.guile("list");
            let e = vm.form(list, &exprs);
            let patterns = vm.list(&patterns);
            let clause = vm.list(&[patterns, body]);
            vm.list(&[case, e, Value::NULL, clause])
        }
    };
    vm.expand(form, env)
}

// --- at run time

type P = Result<Value, Thrown>;

/// A syntax error as psyntax throws it, `(who what #f form subform)`; the
/// expander places it by its form ([`Vm::failed`]).
fn violation(cx: &mut Cx, who: Value, message: &str, form: Value, subform: Value) -> Thrown {
    let key = cx.heap.intern("syntax-error");
    let what = cx.heap.string(message);
    let args = cx.heap.list(&[who, what, Value::FALSE, form, subform]);
    cx.error = Some(Exception { key, args });
    Thrown
}

/// `(%syntax-case-match x pattern literals)`: a vector of the pattern
/// variables' bindings, in `Rules::variables`' order, or `#f`. A binding
/// under ellipses is a list of each repetition's.
pub(crate) fn case_match(cx: &mut Cx, args: &[Value]) -> P {
    let (x, pattern) = (args[0], args[1]);
    let mut literals = Vec::new();
    let mut at = args[2];
    while let (Some(car), Some(cdr)) = (cx.heap.car(at), cx.heap.cdr(at)) {
        literals.push(car);
        at = cdr;
    }
    let rules = Rules::matcher(&mut cx.heap, literals);
    let mut out = Vec::new();
    if !rules.match_pattern(&cx.heap, &cx.globals.aliases, pattern, x, &mut out) {
        return Ok(Value::FALSE);
    }
    let mut vars = Vec::new();
    rules.variables(&cx.heap, &cx.globals.aliases, pattern, 0, &mut vars);
    let mut values = Vec::new();
    for (v, _) in vars {
        let value = match out.iter().find(|(o, _)| *o == v) {
            Some((_, b)) => binding_value(&mut cx.heap, b),
            None => Value::NULL,
        };
        values.push(value);
    }
    Ok(cx.heap.vector(values))
}

fn binding_value(heap: &mut Heap, b: &Binding) -> Value {
    match b {
        Binding::One(v) => *v,
        Binding::Many(seq) => {
            let items: Vec<Value> = seq.iter().map(|b| binding_value(heap, b)).collect();
            heap.list(&items)
        }
    }
}

fn binding_of(heap: &Heap, v: Value, depth: i64) -> Binding {
    if depth <= 0 {
        return Binding::One(v);
    }
    let mut seq = Vec::new();
    let mut at = v;
    while let (Some(car), Some(cdr)) = (heap.car(at), heap.cdr(at)) {
        seq.push(binding_of(heap, car, depth - 1));
        at = cdr;
    }
    Binding::Many(Rc::new(seq))
}

/// `(%syntax-case-fail x)`: no clause matched.
pub(crate) fn case_fail(cx: &mut Cx, args: &[Value]) -> P {
    Err(violation(cx, Value::FALSE, FAILED, args[0], Value::FALSE))
}

/// `(%syntax template '((variable . depth) …) binding …)`: the template
/// with the bindings put in and its other symbols renamed as the running
/// transformer's expansion renames them; outside one, as one expansion in
/// the current module.
pub(crate) fn instantiate(cx: &mut Cx, args: &[Value]) -> P {
    let template = args[0];
    let mut bindings = Vec::new();
    let mut at = args[1];
    let mut values = args[2..].iter();
    while let (Some(pair), Some(cdr)) = (cx.heap.car(at), cx.heap.cdr(at)) {
        let v = cx.heap.car(pair).unwrap_or(Value::FALSE);
        let depth = cx.heap.cdr(pair).and_then(|d| d.as_fixnum()).unwrap_or(0);
        let value = values.next().copied().unwrap_or(Value::NULL);
        bindings.push((v, binding_of(&cx.heap, value, depth)));
        at = cdr;
    }
    let rules = Rules::matcher(&mut cx.heap, Vec::new());
    let (module, depth, done) = match cx.transcriptions.last_mut() {
        Some(t) => (t.module, t.depth, core::mem::take(&mut t.done)),
        None => (cx.globals.current_module(&cx.heap), 0, Vec::new()),
    };
    let mut renamer = Renamer {
        aliases: &mut cx.globals.aliases,
        module,
        depth,
        done,
    };
    let r = rules.instantiate(&mut cx.heap, &mut renamer, template, &bindings);
    let done = core::mem::take(&mut renamer.done);
    if let Some(t) = cx.transcriptions.last_mut() {
        t.done = done;
    }
    r.map_err(|e| {
        let who = match e.who {
            Some(who) => cx.heap.intern(who),
            None => Value::FALSE,
        };
        violation(cx, who, e.message, e.form, e.subform)
    })
}

/// `syntax->datum`.
pub(crate) fn to_datum(cx: &mut Cx, args: &[Value]) -> P {
    Ok(strip(&mut cx.heap, &cx.globals.aliases, args[0]))
}

fn strip(
    heap: &mut Heap,
    aliases: &alloc::collections::BTreeMap<u32, crate::module::Alias>,
    v: Value,
) -> Value {
    match v.tag() {
        Tag::Symbol => base(aliases, v),
        Tag::Pair => {
            let (car, cdr) = (heap.car(v).unwrap_or(v), heap.cdr(v).unwrap_or(v));
            let (a, d) = (strip(heap, aliases, car), strip(heap, aliases, cdr));
            if a == car && d == cdr {
                v
            } else {
                heap.cons(a, d)
            }
        }
        Tag::Vector => {
            let items = heap.vector_items(v).unwrap_or_default().to_vec();
            let new: Vec<Value> = items.iter().map(|&i| strip(heap, aliases, i)).collect();
            if new == items { v } else { heap.vector(new) }
        }
        _ => v,
    }
}

/// `(datum->syntax context datum)`: the datum's symbols as the context's
/// identifier is, its first in a pair or vector (a macro's form: its
/// keyword). The user's, a symbol as it is; an expansion's, an alias in
/// that expansion's module, each symbol once.
pub(crate) fn from_datum(cx: &mut Cx, args: &[Value]) -> P {
    let datum = strip(&mut cx.heap, &cx.globals.aliases, args[1]);
    let Some(id) = first_identifier(&cx.heap, args[0]) else {
        return Ok(datum);
    };
    let Some((module, depth)) = cx
        .globals
        .aliases
        .get(&id.payload())
        .map(|a| (a.module, a.depth))
    else {
        return Ok(datum);
    };
    let mut renamer = Renamer {
        aliases: &mut cx.globals.aliases,
        module,
        depth,
        done: Vec::new(),
    };
    Ok(rename_all(&mut cx.heap, &mut renamer, datum))
}

fn first_identifier(heap: &Heap, v: Value) -> Option<Value> {
    match v.tag() {
        Tag::Symbol => Some(v),
        Tag::Pair | Tag::Vector => parts(heap, v)
            .into_iter()
            .find_map(|i| first_identifier(heap, i)),
        _ => None,
    }
}

fn rename_all(heap: &mut Heap, renamer: &mut Renamer<'_>, v: Value) -> Value {
    match v.tag() {
        Tag::Symbol => renamer.rename(heap, v),
        Tag::Pair => {
            let (car, cdr) = (heap.car(v).unwrap_or(v), heap.cdr(v).unwrap_or(v));
            let a = rename_all(heap, renamer, car);
            let d = rename_all(heap, renamer, cdr);
            heap.cons(a, d)
        }
        Tag::Vector => {
            let items = heap.vector_items(v).unwrap_or_default().to_vec();
            let new = items
                .into_iter()
                .map(|i| rename_all(heap, renamer, i))
                .collect();
            heap.vector(new)
        }
        _ => v,
    }
}

/// `identifier?`.
pub(crate) fn is_identifier(_: &mut Cx, args: &[Value]) -> P {
    Ok(Value::bool(args[0].tag() == Tag::Symbol))
}

fn identifiers(cx: &mut Cx, subr: &str, args: &[Value]) -> Result<(), Thrown> {
    for (i, &v) in args.iter().enumerate() {
        if v.tag() != Tag::Symbol {
            return Err(cx.wrong_type(subr, i + 1, v, "identifier"));
        }
    }
    Ok(())
}

/// `bound-identifier=?`: the same identifier, introduced by the same
/// expansion.
pub(crate) fn bound_identifier_eq(cx: &mut Cx, args: &[Value]) -> P {
    identifiers(cx, "bound-identifier=?", args)?;
    Ok(Value::bool(args[0] == args[1]))
}

/// `free-identifier=?`: the same binding where each is free, or the same
/// name where neither is bound.
pub(crate) fn free_identifier_eq(cx: &mut Cx, args: &[Value]) -> P {
    identifiers(cx, "free-identifier=?", args)?;
    let current = cx.globals.current_module(&cx.heap);
    let resolve = |cx: &Cx, v: Value| match cx.globals.aliases.get(&v.payload()) {
        Some(_) => {
            let module = {
                let mut at = v;
                let mut module = current;
                while let Some(a) = cx.globals.aliases.get(&at.payload()) {
                    module = a.module;
                    at = a.name;
                }
                module
            };
            (base(&cx.globals.aliases, v), module)
        }
        None => (v, current),
    };
    let (a, ma) = resolve(cx, args[0]);
    let (b, mb) = resolve(cx, args[1]);
    let same = match (cx.globals.lookup(ma, a), cx.globals.lookup(mb, b)) {
        (Some(x), Some(y)) => x == y,
        (None, None) => a == b,
        _ => false,
    };
    Ok(Value::bool(same))
}

/// `(generate-temporaries list)`: a fresh identifier for each item.
pub(crate) fn generate_temporaries(cx: &mut Cx, args: &[Value]) -> P {
    let mut n = 0;
    let mut at = args[0];
    while let Some(cdr) = cx.heap.cdr(at) {
        n += 1;
        at = cdr;
    }
    if at != Value::NULL {
        return Err(cx.wrong_type("generate-temporaries", 1, args[0], "list"));
    }
    let temporaries: Vec<Value> = (0..n).map(|_| cx.heap.uninterned("t")).collect();
    Ok(cx.heap.list(&temporaries))
}

/// `(syntax-violation who message form [subform])`.
pub(crate) fn syntax_violation(cx: &mut Cx, args: &[Value]) -> P {
    let Some(message) = cx.heap.str(args[1]).map(|s| String::from(s.as_str())) else {
        return Err(cx.wrong_type("syntax-violation", 2, args[1], "string"));
    };
    let subform = args.get(3).copied().unwrap_or(Value::FALSE);
    Err(violation(cx, args[0], &message, args[2], subform))
}
