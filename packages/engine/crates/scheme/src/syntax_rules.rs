//! `syntax-rules` (chapter 03 §5.5, R7RS 4.3.2): patterns with literals,
//! `_`, ellipses at any depth and in the middle of a list, vectors and
//! dotted tails, a custom ellipsis (`(syntax-rules ::: (literal …) …)`) and
//! `(... ...)`; templates whose symbols are renamed.
//!
//! Renaming is how the expansion stays hygienic (`expand.rs`): each symbol a
//! template introduces becomes an alias, a fresh uninterned symbol that
//! stands for the template's symbol in the macro's module. Bound by the
//! expansion, it captures none of the user's variables; free, it is the
//! macro's variable, not one the user bound where the macro is used. A
//! literal matches an identifier of the same name that is not bound where
//! the macro is used, which is `free-identifier=?` for macros defined at
//! toplevel.

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::compile::{FAILED, SyntaxError, syntax};
use crate::heap::{Heap, Tracer};
use crate::image::{ImageReader, ImageWriter};
use crate::module::Alias;
use crate::value::{Tag, Value};

/// A macro's rules, parsed once from its `syntax-rules` form.
pub(crate) struct Rules {
    ellipsis: Value,
    underscore: Value,
    literals: Vec<Value>,
    /// Each rule's pattern without the macro's keyword, and its template.
    rules: Vec<(Value, Value)>,
}

/// What a pattern variable matched: one form, or one binding per repetition
/// of the ellipsis it is under.
#[derive(Clone)]
pub(crate) enum Binding {
    One(Value),
    Many(Rc<Vec<Binding>>),
}

/// The symbol an alias stands for, through aliases of aliases.
pub(crate) fn base(aliases: &BTreeMap<u32, Alias>, mut v: Value) -> Value {
    while v.tag() == Tag::Symbol
        && let Some(alias) = aliases.get(&v.payload())
    {
        v = alias.name;
    }
    v
}

/// A list's items and its final cdr.
fn split(heap: &Heap, mut list: Value) -> (Vec<Value>, Value) {
    let mut items = Vec::new();
    while let (Some(car), Some(cdr)) = (heap.car(list), heap.cdr(list)) {
        items.push(car);
        list = cdr;
    }
    (items, list)
}

/// The items of a pattern or template: a list's, or a vector's with `()` as
/// its tail.
fn parts(heap: &Heap, x: Value) -> (Vec<Value>, Value) {
    match x.tag() {
        Tag::Vector => (
            heap.vector_items(x).unwrap_or_default().to_vec(),
            Value::NULL,
        ),
        _ => split(heap, x),
    }
}

impl Rules {
    /// `(syntax-rules [ellipsis] (literal …) (pattern template) …)`: the
    /// rules, or psyntax's errors for a template whose ellipses do not
    /// follow its pattern's.
    pub(crate) fn parse(
        heap: &mut Heap,
        aliases: &BTreeMap<u32, Alias>,
        form: Value,
    ) -> Result<Rules, SyntaxError> {
        let (items, tail) = split(heap, form);
        let bad = || syntax(FAILED, form, Some(form));
        if tail != Value::NULL || items.len() < 2 {
            return Err(bad());
        }
        let (ellipsis, rest) = match items[1].tag() {
            Tag::Symbol => (items[1], &items[2..]),
            _ => (heap.intern("..."), &items[1..]),
        };
        let Some((&literals, rules)) = rest.split_first() else {
            return Err(bad());
        };
        let (literals, tail) = split(heap, literals);
        if tail != Value::NULL || literals.iter().any(|l| l.tag() != Tag::Symbol) {
            return Err(bad());
        }
        let mut parsed = Rules {
            ellipsis,
            underscore: heap.intern("_"),
            literals,
            rules: Vec::new(),
        };
        for &rule in rules {
            let (pair, tail) = split(heap, rule);
            let [pattern, template] = pair[..] else {
                return Err(bad());
            };
            if tail != Value::NULL || pattern.tag() != Tag::Pair {
                return Err(bad());
            }
            let pattern = heap.cdr(pattern).unwrap_or(Value::NULL);
            let mut vars = Vec::new();
            parsed.variables(heap, aliases, pattern, 0, &mut vars);
            if let Err(message) = parsed.check(heap, aliases, template, 0, &vars, false) {
                let keyword = heap.intern("syntax");
                let shown = heap.list(&[keyword, template]);
                return Err(SyntaxError {
                    who: Some("syntax"),
                    message,
                    form: shown,
                    subform: Value::FALSE,
                    at: Some(form),
                });
            }
            parsed.rules.push((pattern, template));
        }
        Ok(parsed)
    }

    pub(crate) fn save(&self, w: &mut ImageWriter) {
        w.value(self.ellipsis);
        w.value(self.underscore);
        w.values(&self.literals);
        w.count(self.rules.len());
        for &(pattern, template) in &self.rules {
            w.value(pattern);
            w.value(template);
        }
    }

    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<Rules> {
        Some(Rules {
            ellipsis: r.value()?,
            underscore: r.value()?,
            literals: r.values()?,
            rules: (0..r.count()?)
                .map(|_| Some((r.value()?, r.value()?)))
                .collect::<Option<_>>()?,
        })
    }

    /// No rules, `...` the ellipsis: what `syntax-case`'s clauses match
    /// with and its templates are made with (`syntax_case.rs`).
    pub(crate) fn matcher(heap: &mut Heap, literals: Vec<Value>) -> Rules {
        Rules {
            ellipsis: heap.intern("..."),
            underscore: heap.intern("_"),
            literals,
            rules: Vec::new(),
        }
    }

    /// Whether the whole of `form` matches `pattern`, its bindings put in
    /// `out`; a literal matches an identifier of its name, as no binding is
    /// known where it runs.
    pub(crate) fn match_pattern(
        &self,
        heap: &Heap,
        aliases: &BTreeMap<u32, Alias>,
        pattern: Value,
        form: Value,
        out: &mut Vec<(Value, Binding)>,
    ) -> bool {
        let m = Matcher {
            heap,
            aliases,
            bound: &|_| false,
            rules: self,
        };
        m.pattern(pattern, form, out)
    }

    /// Hands the rules' data to the collector.
    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        for &(pattern, template) in &self.rules {
            tracer.trace(pattern);
            tracer.trace(template);
        }
    }

    fn is_ellipsis(&self, aliases: &BTreeMap<u32, Alias>, v: Value) -> bool {
        v.tag() == Tag::Symbol
            && (v == self.ellipsis || base(aliases, v) == base(aliases, self.ellipsis))
    }

    /// Whether the pattern symbol `p` is a pattern variable.
    fn is_variable(&self, aliases: &BTreeMap<u32, Alias>, p: Value) -> bool {
        p.tag() == Tag::Symbol
            && !self.literals.contains(&p)
            && base(aliases, p) != self.underscore
            && !self.is_ellipsis(aliases, p)
    }

    /// The pattern variables of `p` and how many ellipses each is under.
    pub(crate) fn variables(
        &self,
        heap: &Heap,
        aliases: &BTreeMap<u32, Alias>,
        p: Value,
        depth: usize,
        out: &mut Vec<(Value, usize)>,
    ) {
        match p.tag() {
            Tag::Symbol if self.is_variable(aliases, p) => out.push((p, depth)),
            Tag::Pair | Tag::Vector => {
                let (items, tail) = parts(heap, p);
                for (i, &item) in items.iter().enumerate() {
                    let repeated = items
                        .get(i + 1)
                        .is_some_and(|&next| self.is_ellipsis(aliases, next));
                    if !self.is_ellipsis(aliases, item) {
                        self.variables(heap, aliases, item, depth + usize::from(repeated), out);
                    }
                }
                self.variables(heap, aliases, tail, depth, out);
            }
            _ => {}
        }
    }

    /// Psyntax's checks of a template against its pattern's variables:
    /// each variable under at least as many ellipses as in the pattern, and
    /// each ellipsis after something with a variable under it. `escaped`
    /// inside `(... template)`, where an ellipsis is a symbol.
    pub(crate) fn check(
        &self,
        heap: &Heap,
        aliases: &BTreeMap<u32, Alias>,
        t: Value,
        depth: usize,
        vars: &[(Value, usize)],
        escaped: bool,
    ) -> Result<(), &'static str> {
        match t.tag() {
            Tag::Symbol => match vars.iter().find(|(v, _)| *v == t) {
                Some(&(_, d)) if d > depth => Err("missing ellipsis"),
                _ => Ok(()),
            },
            Tag::Pair | Tag::Vector => {
                let (items, tail) = parts(heap, t);
                if !escaped
                    && t.tag() == Tag::Pair
                    && items.len() == 2
                    && tail == Value::NULL
                    && self.is_ellipsis(aliases, items[0])
                {
                    return self.check(heap, aliases, items[1], depth, vars, true);
                }
                let mut i = 0;
                while i < items.len() {
                    let item = items[i];
                    let mut k = 0;
                    while !escaped
                        && items
                            .get(i + 1 + k)
                            .is_some_and(|&n| self.is_ellipsis(aliases, n))
                    {
                        k += 1;
                    }
                    if k > 0 {
                        let mut inner = Vec::new();
                        self.used(heap, vars, item, &mut inner);
                        if !inner.iter().any(|&d| d >= depth + k) {
                            return Err("extra ellipsis");
                        }
                    }
                    self.check(heap, aliases, item, depth + k, vars, escaped)?;
                    i += 1 + k;
                }
                self.check(heap, aliases, tail, depth, vars, escaped)
            }
            _ => Ok(()),
        }
    }

    /// The depths of the pattern variables that `t` uses.
    fn used(&self, heap: &Heap, vars: &[(Value, usize)], t: Value, out: &mut Vec<usize>) {
        match t.tag() {
            Tag::Symbol => out.extend(vars.iter().filter(|(v, _)| *v == t).map(|&(_, d)| d)),
            Tag::Pair | Tag::Vector => {
                let (items, tail) = parts(heap, t);
                for item in items.into_iter().chain([tail]) {
                    self.used(heap, vars, item, out);
                }
            }
            _ => {}
        }
    }

    /// The first rule that matches `form` (the macro's use, keyword
    /// included): its bindings and its template. `bound` says whether an
    /// identifier is bound where the macro is used, for literals.
    pub(crate) fn find(
        &self,
        heap: &Heap,
        aliases: &BTreeMap<u32, Alias>,
        bound: &dyn Fn(Value) -> bool,
        form: Value,
    ) -> Option<(Vec<(Value, Binding)>, Value)> {
        let input = heap.cdr(form)?;
        let m = Matcher {
            heap,
            aliases,
            bound,
            rules: self,
        };
        self.rules.iter().find_map(|&(pattern, template)| {
            let mut out = Vec::new();
            m.pattern(pattern, input, &mut out)
                .then_some((out, template))
        })
    }

    /// The template with the bindings put in and its own symbols renamed.
    pub(crate) fn instantiate(
        &self,
        heap: &mut Heap,
        renamer: &mut Renamer<'_>,
        template: Value,
        bindings: &[(Value, Binding)],
    ) -> Result<Value, SyntaxError> {
        let mut t = Transcriber {
            rules: self,
            renamer,
        };
        t.template(heap, template, bindings, false)
    }
}

struct Matcher<'a> {
    heap: &'a Heap,
    aliases: &'a BTreeMap<u32, Alias>,
    bound: &'a dyn Fn(Value) -> bool,
    rules: &'a Rules,
}

impl Matcher<'_> {
    fn pattern(&self, p: Value, f: Value, out: &mut Vec<(Value, Binding)>) -> bool {
        let r = self.rules;
        match p.tag() {
            Tag::Symbol if r.literals.contains(&p) => {
                f.tag() == Tag::Symbol
                    && base(self.aliases, f) == base(self.aliases, p)
                    && !(self.bound)(f)
            }
            Tag::Symbol if base(self.aliases, p) == r.underscore => true,
            Tag::Symbol => {
                out.push((p, Binding::One(f)));
                true
            }
            Tag::Pair => {
                let (pats, ptail) = split(self.heap, p);
                if !pats.iter().any(|&q| r.is_ellipsis(self.aliases, q)) {
                    // Pairwise, the pattern's tail matching the rest.
                    let mut f = f;
                    for q in pats {
                        let (Some(car), Some(cdr)) = (self.heap.car(f), self.heap.cdr(f)) else {
                            return false;
                        };
                        if !self.pattern(q, car, out) {
                            return false;
                        }
                        f = cdr;
                    }
                    return self.pattern(ptail, f, out);
                }
                let (forms, ftail) = split(self.heap, f);
                self.sequence(&pats, ptail, &forms, ftail, out)
            }
            Tag::Vector => {
                if f.tag() != Tag::Vector {
                    return false;
                }
                let (pats, _) = parts(self.heap, p);
                let (forms, _) = parts(self.heap, f);
                self.sequence(&pats, Value::NULL, &forms, Value::NULL, out)
            }
            _ => self.heap.equal(p, f),
        }
    }

    /// Items with an ellipsis among them: those before it, the repeated
    /// one as many times as the forms leave, those after it, then the
    /// tails. Without an ellipsis (a vector's), item by item.
    fn sequence(
        &self,
        pats: &[Value],
        ptail: Value,
        forms: &[Value],
        ftail: Value,
        out: &mut Vec<(Value, Binding)>,
    ) -> bool {
        let r = self.rules;
        let Some(at) = pats.iter().position(|&q| r.is_ellipsis(self.aliases, q)) else {
            return pats.len() == forms.len()
                && pats
                    .iter()
                    .zip(forms)
                    .all(|(&q, &g)| self.pattern(q, g, out))
                && self.pattern(ptail, ftail, out);
        };
        let Some(before) = at.checked_sub(1) else {
            return false;
        };
        let repeated = pats[before];
        let after = &pats[at + 1..];
        let Some(n) = forms.len().checked_sub(before + after.len()) else {
            return false;
        };
        if !pats[..before]
            .iter()
            .zip(forms)
            .all(|(&q, &g)| self.pattern(q, g, out))
        {
            return false;
        }
        let mut vars = Vec::new();
        r.variables(self.heap, self.aliases, repeated, 0, &mut vars);
        let mut seqs: Vec<Vec<Binding>> = vec![Vec::new(); vars.len()];
        for &g in &forms[before..before + n] {
            let mut one = Vec::new();
            if !self.pattern(repeated, g, &mut one) {
                return false;
            }
            for ((var, _), seq) in vars.iter().zip(&mut seqs) {
                if let Some((_, b)) = one.iter().find(|(v, _)| v == var) {
                    seq.push(b.clone());
                }
            }
        }
        for ((var, _), seq) in vars.into_iter().zip(seqs) {
            out.push((var, Binding::Many(Rc::new(seq))));
        }
        after
            .iter()
            .zip(&forms[before + n..])
            .all(|(&q, &g)| self.pattern(q, g, out))
            && self.pattern(ptail, ftail, out)
    }
}

/// Makes the aliases of one expansion: each symbol of the template once.
pub(crate) struct Renamer<'a> {
    pub aliases: &'a mut BTreeMap<u32, Alias>,
    pub module: u32,
    pub depth: u32,
    pub done: Vec<(Value, Value)>,
}

impl Renamer<'_> {
    pub(crate) fn rename(&mut self, heap: &mut Heap, symbol: Value) -> Value {
        if let Some(&(_, alias)) = self.done.iter().find(|(s, _)| *s == symbol) {
            return alias;
        }
        let name = String::from(heap.name(symbol).unwrap_or(""));
        let alias = heap.uninterned(&name);
        self.aliases.insert(
            alias.payload(),
            Alias {
                name: symbol,
                module: self.module,
                depth: self.depth,
            },
        );
        self.done.push((symbol, alias));
        alias
    }
}

struct Transcriber<'a, 'b> {
    rules: &'a Rules,
    renamer: &'a mut Renamer<'b>,
}

impl Transcriber<'_, '_> {
    fn template(
        &mut self,
        heap: &mut Heap,
        t: Value,
        bindings: &[(Value, Binding)],
        escaped: bool,
    ) -> Result<Value, SyntaxError> {
        match t.tag() {
            Tag::Symbol => match bindings.iter().find(|(v, _)| *v == t) {
                Some((_, Binding::One(value))) => Ok(*value),
                Some((_, Binding::Many(_))) => Err(syntax("missing ellipsis", t, None)),
                None => Ok(self.renamer.rename(heap, t)),
            },
            Tag::Pair | Tag::Vector => {
                let (items, tail) = parts(heap, t);
                let aliases = &*self.renamer.aliases;
                if !escaped
                    && t.tag() == Tag::Pair
                    && items.len() == 2
                    && tail == Value::NULL
                    && self.rules.is_ellipsis(aliases, items[0])
                {
                    return self.template(heap, items[1], bindings, true);
                }
                let mut out = Vec::new();
                let mut i = 0;
                while i < items.len() {
                    let mut k = 0;
                    while !escaped
                        && items
                            .get(i + 1 + k)
                            .is_some_and(|&n| self.rules.is_ellipsis(&*self.renamer.aliases, n))
                    {
                        k += 1;
                    }
                    if k == 0 {
                        out.push(self.template(heap, items[i], bindings, escaped)?);
                    } else {
                        self.repeat(heap, items[i], k, bindings, &mut out)?;
                    }
                    i += 1 + k;
                }
                if t.tag() == Tag::Vector {
                    return Ok(heap.vector(out));
                }
                let tail = self.template(heap, tail, bindings, escaped)?;
                Ok(out
                    .into_iter()
                    .rev()
                    .fold(tail, |rest, item| heap.cons(item, rest)))
            }
            _ => Ok(t),
        }
    }

    /// `item` followed by `k` ellipses: once for each repetition of the
    /// variables under it that matched more than once.
    fn repeat(
        &mut self,
        heap: &mut Heap,
        item: Value,
        k: usize,
        bindings: &[(Value, Binding)],
        out: &mut Vec<Value>,
    ) -> Result<(), SyntaxError> {
        let mut used = Vec::new();
        symbols(heap, item, &mut used);
        let controls: Vec<(Value, Rc<Vec<Binding>>)> = bindings
            .iter()
            .filter_map(|(v, b)| match b {
                Binding::Many(seq) if used.contains(v) => Some((*v, seq.clone())),
                _ => None,
            })
            .collect();
        let Some(n) = controls.first().map(|(_, seq)| seq.len()) else {
            return Err(syntax("extra ellipsis", item, None));
        };
        if controls.iter().any(|(_, seq)| seq.len() != n) {
            return Err(syntax(FAILED, item, None));
        }
        for i in 0..n {
            let mut inner: Vec<(Value, Binding)> = bindings.to_vec();
            for (var, seq) in &controls {
                if let Some(slot) = inner.iter_mut().find(|(v, _)| v == var) {
                    slot.1 = seq[i].clone();
                }
            }
            if k > 1 {
                self.repeat(heap, item, k - 1, &inner, out)?;
            } else {
                out.push(self.template(heap, item, &inner, false)?);
            }
        }
        Ok(())
    }
}

/// The symbols in `t`.
fn symbols(heap: &Heap, t: Value, out: &mut Vec<Value>) {
    match t.tag() {
        Tag::Symbol => out.push(t),
        Tag::Pair | Tag::Vector => {
            let (items, tail) = parts(heap, t);
            for item in items.into_iter().chain([tail]) {
                symbols(heap, item, out);
            }
        }
        _ => {}
    }
}
