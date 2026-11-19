//! The expander (chapter 03 §5.5): turns a form into the compiler's core
//! forms before it is compiled, running macros as it goes.
//!
//! Native, in Rust: `quasiquote` (psyntax's algorithm, with splicing and
//! vectors), `lambda*` and `define*` with `#:optional`, `#:key`,
//! `#:allow-other-keys` and `#:rest`, `define*-public`, `case-lambda`, `do`,
//! `while` (with `break` and `continue`), `eval-when`, the generalized
//! `set!` of a procedure with a setter, and `let-syntax` and
//! `letrec-syntax`. Macros are values in slots, so modules export and import
//! them as they do variables (03 §5.4): `define-syntax` and
//! `define-syntax-public` of `syntax-rules` (`syntax_rules.rs`),
//! `define-syntax-rule` and `define-syntax-rule-public`, `defmacro`,
//! `defmacro-public`, `defmacro*`, `defmacro*-public` and `define-macro`. A
//! macro is defined when the expander meets its definition, as Guile's
//! `eval-when (expand load eval)` does; one defined inside a body is local
//! to it.
//!
//! `(lily)` has two native macros in place of the `syntax-case` ones of
//! `lily-library.scm` and `music-functions.scm`, which the engine does not
//! expand (D60): `assert`, and `define-syntax-function`, whose procedure
//! carries the music function's signature in its header
//! ([`crate::Vm::procedure_header`]). Loading either file keeps the native
//! macro where it defines its own with a procedure. `(lily
//! curried-definitions)`, which `(lily)` uses, has `define`, `define*`,
//! `define-public` and `define*-public` as native macros that take a
//! curried head, as `curried-definitions.scm`'s `syntax-case` ones do.
//! `define-markup-command` and `markup` are `markup-macros.scm`'s
//! `defmacro`s.
//!
//! The expander keeps the forms it does not change, so the compiler still
//! finds a source form's place for its errors, and leaves a core form it
//! cannot read to the compiler, which words the error as psyntax does. The
//! collector is held while it runs, since a transformer may run Scheme
//! while the expander holds values in Rust.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::compile::{self, FAILED, MAX_NESTING, SyntaxError, Template, syntax};
use crate::heap::{Foreign, Heap, Tracer};
use crate::image::{ImageReader, ImageWriter};
use crate::module::{Alias, GUILE, LILY};
use crate::syntax_rules::{Renamer, Rules, base};
use crate::value::{Tag, Value};
use crate::vm::{Exception, Vm};

#[path = "syntax_case.rs"]
pub(crate) mod syntax_case;

/// A procedure transformer's expansion running: what its templates'
/// symbols are renamed to stand for, and the aliases made so far, each
/// symbol's once (`syntax_case.rs`).
pub(crate) struct Transcription {
    pub module: u32,
    pub depth: u32,
    pub done: Vec<(Value, Value)>,
}

/// The forms the expander knows, in the order of [`KEYWORDS`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kw {
    Quote,
    Quasiquote,
    Unquote,
    UnquoteSplicing,
    Lambda,
    LambdaStar,
    Define,
    DefinePublic,
    DefineStar,
    DefineStarPublic,
    Set,
    If,
    Begin,
    Let,
    LetStar,
    Letrec,
    LetrecStar,
    Cond,
    Case,
    And,
    Or,
    When,
    Unless,
    Do,
    While,
    CaseLambda,
    Parameterize,
    WithFluids,
    DefineModule,
    UseModules,
    Export,
    ReExport,
    At,
    AtAt,
    EvalWhen,
    DefineSyntax,
    DefineSyntaxPublic,
    DefineSyntaxRule,
    DefineSyntaxRulePublic,
    LetSyntax,
    LetrecSyntax,
    SyntaxRules,
    Defmacro,
    DefmacroPublic,
    DefmacroStar,
    DefmacroStarPublic,
    DefineMacro,
    Dyn,
    CallValues,
    /// `syntax-case` and its forms, the `syntax-case` part's (D273).
    SyntaxCase,
    Syntax,
    Quasisyntax,
    Unsyntax,
    UnsyntaxSplicing,
    WithSyntax,
    /// The compiler's `(%procedure star header)`.
    Procedure,
}

const KEYWORDS: &[(&str, Kw)] = &[
    ("quote", Kw::Quote),
    ("quasiquote", Kw::Quasiquote),
    ("unquote", Kw::Unquote),
    ("unquote-splicing", Kw::UnquoteSplicing),
    ("lambda", Kw::Lambda),
    ("lambda*", Kw::LambdaStar),
    ("define", Kw::Define),
    ("define-public", Kw::DefinePublic),
    ("define*", Kw::DefineStar),
    ("define*-public", Kw::DefineStarPublic),
    ("set!", Kw::Set),
    ("if", Kw::If),
    ("begin", Kw::Begin),
    ("let", Kw::Let),
    ("let*", Kw::LetStar),
    ("letrec", Kw::Letrec),
    ("letrec*", Kw::LetrecStar),
    ("cond", Kw::Cond),
    ("case", Kw::Case),
    ("and", Kw::And),
    ("or", Kw::Or),
    ("when", Kw::When),
    ("unless", Kw::Unless),
    ("do", Kw::Do),
    ("while", Kw::While),
    ("case-lambda", Kw::CaseLambda),
    ("parameterize", Kw::Parameterize),
    ("with-fluids", Kw::WithFluids),
    ("define-module", Kw::DefineModule),
    ("use-modules", Kw::UseModules),
    ("export", Kw::Export),
    ("re-export", Kw::ReExport),
    ("@", Kw::At),
    ("@@", Kw::AtAt),
    ("eval-when", Kw::EvalWhen),
    ("define-syntax", Kw::DefineSyntax),
    ("define-syntax-public", Kw::DefineSyntaxPublic),
    ("define-syntax-rule", Kw::DefineSyntaxRule),
    ("define-syntax-rule-public", Kw::DefineSyntaxRulePublic),
    ("let-syntax", Kw::LetSyntax),
    ("letrec-syntax", Kw::LetrecSyntax),
    ("syntax-rules", Kw::SyntaxRules),
    ("defmacro", Kw::Defmacro),
    ("defmacro-public", Kw::DefmacroPublic),
    ("defmacro*", Kw::DefmacroStar),
    ("defmacro*-public", Kw::DefmacroStarPublic),
    ("define-macro", Kw::DefineMacro),
    ("%dyn", Kw::Dyn),
    ("%call-values", Kw::CallValues),
    ("syntax-case", Kw::SyntaxCase),
    ("syntax", Kw::Syntax),
    ("quasisyntax", Kw::Quasisyntax),
    ("unsyntax", Kw::Unsyntax),
    ("unsyntax-splicing", Kw::UnsyntaxSplicing),
    ("with-syntax", Kw::WithSyntax),
];

impl Kw {
    /// Whether it defines a macro.
    fn defines_syntax(self) -> bool {
        matches!(
            self,
            Kw::DefineSyntax
                | Kw::DefineSyntaxPublic
                | Kw::DefineSyntaxRule
                | Kw::DefineSyntaxRulePublic
                | Kw::Defmacro
                | Kw::DefmacroPublic
                | Kw::DefmacroStar
                | Kw::DefmacroStarPublic
                | Kw::DefineMacro
        )
    }

    fn public(self) -> bool {
        matches!(
            self,
            Kw::DefinePublic
                | Kw::DefineStarPublic
                | Kw::DefineSyntaxPublic
                | Kw::DefineSyntaxRulePublic
                | Kw::DefmacroPublic
                | Kw::DefmacroStarPublic
        )
    }
}

// --- macros ---------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) enum MacroKind {
    Rules(Rc<Rules>),
    /// `defmacro` and `define-macro`: called with the form's data.
    Defmacro(Value),
    /// A transformer other than `syntax-rules`, a procedure (`syntax-case`'s
    /// as a rule): called with the form, its `syntax` templates renamed as
    /// one expansion's (D273).
    Procedure(Value),
    /// The native `assert` and `define-syntax-function` of `(lily)`.
    Assert,
    SyntaxFunction,
    /// `(lily curried-definitions)`' form of the definition keyword.
    Curried(Kw),
    /// Guile's `include` of `(guile)`, which reads no file in the engine.
    Include,
}

/// A macro, the value of its keyword's slot: Guile's syntax transformer.
pub(crate) struct Macro {
    name: Value,
    /// Where it was defined, which its introduced identifiers refer to.
    module: u32,
    /// For a local macro, how many bindings of the expander's scope its
    /// definition sees; 0 at toplevel.
    depth: u32,
    kind: MacroKind,
}

impl Foreign for Macro {
    fn type_name(&self) -> &'static str {
        "syntax-transformer"
    }

    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<syntax-transformer ");
        heap.write(self.name, out);
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.name);
        tracer.module(self.module);
        match &self.kind {
            MacroKind::Rules(rules) => rules.trace(tracer),
            MacroKind::Defmacro(p) | MacroKind::Procedure(p) => tracer.trace(*p),
            MacroKind::Assert
            | MacroKind::SyntaxFunction
            | MacroKind::Curried(_)
            | MacroKind::Include => {}
        }
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.value(self.name);
        out.u32(self.module);
        out.u32(self.depth);
        match &self.kind {
            MacroKind::Rules(rules) => {
                out.u8(0);
                rules.save(out);
            }
            MacroKind::Defmacro(p) => {
                out.u8(1);
                out.value(*p);
            }
            MacroKind::Procedure(p) => {
                out.u8(2);
                out.value(*p);
            }
            MacroKind::Assert => out.u8(3),
            MacroKind::SyntaxFunction => out.u8(4),
            MacroKind::Curried(kw) => {
                out.u8(5);
                out.u32(*kw as u32);
            }
            MacroKind::Include => out.u8(6),
        }
        true
    }
}

pub(crate) fn load_macro(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    let (name, module, depth) = (r.value()?, r.u32()?, r.u32()?);
    let kind = match r.u8()? {
        0 => MacroKind::Rules(Rc::new(Rules::load(r)?)),
        1 => MacroKind::Defmacro(r.value()?),
        2 => MacroKind::Procedure(r.value()?),
        3 => MacroKind::Assert,
        4 => MacroKind::SyntaxFunction,
        5 => MacroKind::Curried(KEYWORDS.get(r.u32()? as usize)?.1),
        6 => MacroKind::Include,
        _ => return None,
    };
    Some(Box::new(Macro {
        name,
        module,
        depth,
        kind,
    }))
}

fn is_macro(heap: &Heap, v: Value) -> bool {
    v.tag() == Tag::Foreign && heap.foreign::<Macro>(v).is_some()
}

/// The native macros, defined when the VM is made, with their modules:
/// `(lily)`'s, and those of `curried`, the module `(lily
/// curried-definitions)`, which `lily.scm` imports into `(lily)` and whose
/// `syntax-case` definitions they stand in for (D60); and `(guile)`'s
/// `include`, which LilyPond's lexer finds when `\include` is read again
/// in its `incl` state (MS6 step 3, OPEN F).
pub(crate) fn natives(heap: &mut Heap, curried: u32) -> [(u32, &'static str, Value); 7] {
    [
        (GUILE, "include", MacroKind::Include),
        (LILY, "assert", MacroKind::Assert),
        (LILY, "define-syntax-function", MacroKind::SyntaxFunction),
        (curried, "define", MacroKind::Curried(Kw::Define)),
        (curried, "define*", MacroKind::Curried(Kw::DefineStar)),
        (
            curried,
            "define-public",
            MacroKind::Curried(Kw::DefinePublic),
        ),
        (
            curried,
            "define*-public",
            MacroKind::Curried(Kw::DefineStarPublic),
        ),
    ]
    .map(|(module, name, kind)| {
        let symbol = heap.intern(name);
        let value = heap.wrap(Macro {
            name: symbol,
            module,
            depth: 0,
            kind,
        });
        (module, name, value)
    })
}

// --- the expander -------------------------------------------------------------------

/// What makes an expansion fail: a syntax error, or a throw from a
/// transformer or from code that `define-syntax` ran.
pub(crate) enum Fail {
    Syntax(SyntaxError),
    Thrown(Exception),
}

impl From<SyntaxError> for Fail {
    fn from(e: SyntaxError) -> Fail {
        Fail::Syntax(e)
    }
}

type R<T> = Result<T, Fail>;

/// The lexical environment of the form being expanded.
pub(crate) struct Env {
    /// Bound identifiers, the innermost last: `UNDEFINED` for a variable,
    /// else a local macro.
    scope: Vec<(Value, Value)>,
    module: u32,
    internal: bool,
    depth: usize,
}

impl Env {
    fn new(module: u32, internal: bool) -> Env {
        Env {
            scope: Vec::new(),
            module,
            internal,
            depth: 0,
        }
    }
}

enum Meaning {
    /// A variable bound here, by this symbol.
    Lexical(Value),
    Macro(Value),
    /// A `syntax-case` pattern variable: `(variable . depth)`, the symbol
    /// its binding is in and how many ellipses it is under.
    Pattern(Value),
    Keyword(Kw),
    Global,
}

/// The expander's symbols.
pub(crate) struct Expander {
    keywords: BTreeMap<u32, Kw>,
    /// Each keyword's symbol, by [`Kw`].
    canon: Vec<Value>,
    else_: Value,
    arrow: Value,
    parser: Value,
    underscore_i: Value,
    /// Aliases of the variables expansions refer to, by module and symbol.
    refs: BTreeMap<(u32, u32), Value>,
}

impl Expander {
    pub(crate) fn new(heap: &mut Heap, procedure: Value) -> Expander {
        let mut keywords = BTreeMap::new();
        let mut canon = Vec::new();
        for (i, &(name, kw)) in KEYWORDS.iter().enumerate() {
            debug_assert_eq!(kw as usize, i, "KEYWORDS is in Kw's order");
            let symbol = heap.intern(name);
            keywords.insert(symbol.payload(), kw);
            canon.push(symbol);
        }
        keywords.insert(procedure.payload(), Kw::Procedure);
        canon.push(procedure);
        Expander {
            keywords,
            canon,
            else_: heap.intern("else"),
            arrow: heap.intern("=>"),
            parser: heap.intern("parser"),
            underscore_i: heap.intern("_i"),
            refs: BTreeMap::new(),
        }
    }

    fn sym(&self, kw: Kw) -> Value {
        self.canon[kw as usize]
    }

    /// Forgets the aliases into `modules`, which a collection freed.
    pub(crate) fn forget(&mut self, modules: &[u32]) {
        self.refs
            .retain(|&(module, _), _| !modules.contains(&module));
    }

    /// Writes the aliases of the variables expansions refer to, the rest
    /// being what [`Expander::new`] makes.
    pub(crate) fn save(&self, w: &mut ImageWriter) {
        w.count(self.refs.len());
        for (&(module, symbol), &alias) in &self.refs {
            w.u32(module);
            w.u32(symbol);
            w.value(alias);
        }
    }

    pub(crate) fn load(&mut self, r: &mut ImageReader<'_>) -> Option<()> {
        for _ in 0..r.count()? {
            let key = (r.u32()?, r.u32()?);
            self.refs.insert(key, r.value()?);
        }
        Some(())
    }
}

/// psyntax's quasiquote, before it is emitted as code.
#[derive(Clone)]
enum Q {
    Quote(Value),
    Value(Value),
    List(Vec<Q>),
    /// `cons*`.
    ListStar(Vec<Q>, Box<Q>),
    Append(Vec<Q>),
    Vector(Vec<Q>),
    ListToVector(Box<Q>),
}

/// `lambda*`'s formals.
struct Star {
    required: Vec<Value>,
    optional: Vec<(Value, Value)>,
    /// Each key's keyword, variable and init, after `#:key`.
    keys: Option<Vec<(Value, Value, Value)>>,
    allow_other_keys: bool,
    rest: Option<Value>,
}

impl Vm {
    // --- entry points

    /// Expands `form` at toplevel in `module` and compiles it.
    pub(crate) fn compile_form(
        &mut self,
        form: Value,
        module: u32,
        internal: bool,
    ) -> Result<Rc<Template>, Fail> {
        self.cx.heap.hold();
        let mut env = Env::new(module, internal);
        let expanded = self.expand_top(form, &mut env);
        self.cx.heap.release();
        compile::compile(
            &mut self.cx.heap,
            &mut self.cx.globals,
            &self.syntax,
            expanded?,
            module,
            internal,
        )
        .map_err(Fail::Syntax)
    }

    /// A toplevel form with its macro use expanded until it is not one, and
    /// an `eval-when` as the `begin` or nothing it is, and whether it is a
    /// `begin`: what [`Vm::eval_text`] splits into forms that it compiles
    /// and runs one by one.
    pub(crate) fn expand_toplevel_head(&mut self, form: Value, module: u32) -> R<(Value, bool)> {
        self.cx.heap.hold();
        let mut env = Env::new(module, false);
        let x = self
            .expand_head(form, &mut env)
            .and_then(|x| match self.head_keyword(x, &env) {
                Some(Kw::EvalWhen) => self.eval_when(x, true),
                _ => Ok(x),
            });
        let x = x.map(|x| (x, self.head_keyword(x, &env) == Some(Kw::Begin)));
        self.cx.heap.release();
        x
    }

    /// The header of a procedure's code: a music function's signature, as
    /// `define-syntax-function` recorded it; `None` for any other value.
    pub fn procedure_header(&self, procedure: Value) -> Option<Value> {
        if procedure.tag() != Tag::Closure {
            return None;
        }
        let code = self.cx.heap.closure_code(procedure)?;
        (code.header != Value::FALSE).then_some(code.header)
    }

    // --- identifiers

    fn meaning(&self, id: Value, env: &Env) -> Meaning {
        let (mut id, mut limit, mut module) = (id, env.scope.len(), env.module);
        loop {
            if let Some(&(name, bound)) = env.scope[..limit].iter().rev().find(|(n, _)| *n == id) {
                return if bound == Value::UNDEFINED {
                    Meaning::Lexical(name)
                } else if bound.tag() == Tag::Pair {
                    Meaning::Pattern(bound)
                } else {
                    Meaning::Macro(bound)
                };
            }
            let globals = &self.cx.globals;
            match globals.aliases.get(&id.payload()) {
                Some(alias) if globals.local_slot(module, id).is_none() => {
                    (id, module) = (alias.name, alias.module);
                    limit = limit.min(alias.depth as usize);
                }
                _ => break,
            }
        }
        if let Some(slot) = self.cx.globals.lookup(module, id) {
            let value = self.cx.globals.values[slot as usize];
            if is_macro(&self.cx.heap, value) {
                return Meaning::Macro(value);
            }
        }
        match self.expander.keywords.get(&id.payload()) {
            Some(&kw) if env.internal || !matches!(kw, Kw::Dyn | Kw::CallValues) => {
                Meaning::Keyword(kw)
            }
            _ => Meaning::Global,
        }
    }

    fn is_lexical(&self, id: Value, env: &Env) -> bool {
        matches!(self.meaning(id, env), Meaning::Lexical(_))
    }

    /// Whether `id` is the free identifier `symbol`, as a literal matches.
    fn is_free(&self, id: Value, symbol: Value, env: &Env) -> bool {
        id.tag() == Tag::Symbol
            && base(&self.cx.globals.aliases, id) == symbol
            && !self.is_lexical(id, env)
    }

    fn head_keyword(&self, x: Value, env: &Env) -> Option<Kw> {
        let head = self.cx.heap.car(x)?;
        if head.tag() != Tag::Symbol {
            return None;
        }
        match self.meaning(head, env) {
            Meaning::Keyword(kw) => Some(kw),
            _ => None,
        }
    }

    /// An alias for the variable `name` of `module`, which an expansion
    /// refers to whatever the code around it binds.
    fn module_ref(&mut self, name: &str, module: u32) -> Value {
        let symbol = self.cx.heap.intern(name);
        if let Some(&alias) = self.expander.refs.get(&(module, symbol.payload())) {
            return alias;
        }
        let alias = self.cx.heap.uninterned(name);
        self.cx.globals.aliases.insert(
            alias.payload(),
            Alias {
                name: symbol,
                module,
                depth: 0,
            },
        );
        self.expander.refs.insert((module, symbol.payload()), alias);
        alias
    }

    fn guile(&mut self, name: &str) -> Value {
        self.module_ref(name, GUILE)
    }

    /// A symbol that no other is, for a variable an expansion binds.
    fn fresh(&mut self, name: &str) -> Value {
        self.cx.heap.uninterned(name)
    }

    /// `syntax->datum`: the data with each alias as the symbol it stands
    /// for.
    pub(crate) fn strip(&mut self, v: Value) -> Value {
        if self.cx.globals.aliases.is_empty() {
            return v;
        }
        match v.tag() {
            Tag::Symbol => base(&self.cx.globals.aliases, v),
            Tag::Pair => {
                let (items, tail) = self.split(v);
                let stripped: Vec<Value> = items.iter().map(|&i| self.strip(i)).collect();
                let new_tail = self.strip(tail);
                if new_tail == tail && stripped == items {
                    return v;
                }
                stripped
                    .into_iter()
                    .rev()
                    .fold(new_tail, |rest, item| self.cx.heap.cons(item, rest))
            }
            Tag::Vector => {
                let items = self.cx.heap.vector_items(v).unwrap_or_default().to_vec();
                let stripped: Vec<Value> = items.iter().map(|&i| self.strip(i)).collect();
                if stripped == items {
                    v
                } else {
                    self.cx.heap.vector(stripped)
                }
            }
            _ => v,
        }
    }

    // --- data

    fn car(&self, x: Value) -> Value {
        self.cx.heap.car(x).unwrap_or(Value::UNSPECIFIED)
    }

    fn cdr(&self, x: Value) -> Value {
        self.cx.heap.cdr(x).unwrap_or(Value::UNSPECIFIED)
    }

    fn split(&self, mut x: Value) -> (Vec<Value>, Value) {
        let mut items = Vec::new();
        while let (Some(car), Some(cdr)) = (self.cx.heap.car(x), self.cx.heap.cdr(x)) {
            items.push(car);
            x = cdr;
        }
        (items, x)
    }

    /// The items of a proper list.
    fn items(&self, x: Value) -> Option<Vec<Value>> {
        let (items, tail) = self.split(x);
        (tail == Value::NULL).then_some(items)
    }

    fn list(&mut self, items: &[Value]) -> Value {
        self.cx.heap.list(items)
    }

    /// `(head item …)`.
    fn form(&mut self, head: Value, items: &[Value]) -> Value {
        let list = self.cx.heap.list(items);
        self.cx.heap.cons(head, list)
    }

    /// `(head item … . tail)`.
    fn form_star(&mut self, head: Value, items: &[Value], tail: Value) -> Value {
        let list = items
            .iter()
            .rev()
            .fold(tail, |rest, &item| self.cx.heap.cons(item, rest));
        self.cx.heap.cons(head, list)
    }

    /// The keyword as an alias, which the code an expansion makes uses so
    /// that no binding around it changes what it is.
    fn kw(&mut self, kw: Kw) -> Value {
        match kw {
            Kw::Procedure => self.syntax.procedure(),
            kw => self.module_ref(KEYWORDS[kw as usize].0, GUILE),
        }
    }

    /// `x` with the operands `new`, or `x` itself if they are its own,
    /// `old`. Its head stays as it was written: the compiler reads an alias
    /// of a keyword as the keyword.
    fn rebuild(&mut self, x: Value, old: &[Value], new: &[Value]) -> Value {
        if old == new {
            return x;
        }
        let head = self.car(x);
        self.form(head, new)
    }

    // --- forms

    fn expand_top(&mut self, x: Value, env: &mut Env) -> R<Value> {
        let x = self.expand_head(x, env)?;
        match self.head_keyword(x, env) {
            Some(Kw::Begin) => {
                let Some(items) = self.items(self.cdr(x)) else {
                    return Ok(x);
                };
                let mut new = Vec::new();
                for &item in &items {
                    new.push(self.expand_top(item, env)?);
                }
                Ok(self.rebuild(x, &items, &new))
            }
            Some(Kw::EvalWhen) => match self.eval_when(x, true)? {
                Value::UNSPECIFIED => Ok(Value::UNSPECIFIED),
                y => self.expand_top(y, env),
            },
            Some(kw @ (Kw::Define | Kw::DefinePublic | Kw::DefineStar | Kw::DefineStarPublic)) => {
                self.definition(x, kw, env)
            }
            Some(kw) if kw.defines_syntax() => {
                self.define_syntax(x, kw, env, true)?;
                Ok(Value::UNSPECIFIED)
            }
            _ => self.expand(x, env),
        }
    }

    fn expand(&mut self, x: Value, env: &mut Env) -> R<Value> {
        match x.tag() {
            Tag::Symbol => match self.meaning(x, env) {
                Meaning::Lexical(found) => Ok(found),
                Meaning::Macro(_) => Err(syntax(FAILED, x, None).into()),
                Meaning::Pattern(_) => {
                    Err(syntax("reference to pattern variable outside syntax form", x, None).into())
                }
                _ => Ok(x),
            },
            Tag::Pair => {
                if env.depth >= MAX_NESTING {
                    return Err(syntax("expression nested too deeply", x, None).into());
                }
                env.depth += 1;
                let r = self.combination(x, env);
                env.depth -= 1;
                r
            }
            Tag::Vector => Ok(self.strip(x)),
            _ => Ok(x),
        }
    }

    /// Expands the macro use `x` is until it is not one.
    fn expand_head(&mut self, mut x: Value, env: &mut Env) -> R<Value> {
        loop {
            let head = self.car(x);
            if x.tag() != Tag::Pair || head.tag() != Tag::Symbol {
                return Ok(x);
            }
            let Meaning::Macro(m) = self.meaning(head, env) else {
                return Ok(x);
            };
            x = self.transcribe(m, x, env)?;
        }
    }

    fn combination(&mut self, x: Value, env: &mut Env) -> R<Value> {
        let x = self.expand_head(x, env)?;
        if x.tag() != Tag::Pair {
            return self.expand(x, env);
        }
        if let Some(kw) = self.head_keyword(x, env) {
            return self.keyword(kw, x, env);
        }
        self.operands(x, false, env)
    }

    /// A call, or a keyword's form whose operands are expressions: each
    /// expanded.
    fn operands(&mut self, x: Value, keyword: bool, env: &mut Env) -> R<Value> {
        let Some(items) = self.items(x) else {
            return Ok(x);
        };
        let skip = usize::from(keyword);
        let mut new = items[..skip].to_vec();
        for &item in &items[skip..] {
            new.push(self.expand(item, env)?);
        }
        Ok(if new == items { x } else { self.list(&new) })
    }

    fn keyword(&mut self, kw: Kw, x: Value, env: &mut Env) -> R<Value> {
        let located = || -> Fail { syntax(FAILED, x, Some(x)).into() };
        match kw {
            Kw::Quote => match self.items(x).as_deref() {
                Some(&[_, datum]) => {
                    let stripped = self.strip(datum);
                    if stripped == datum {
                        Ok(x)
                    } else {
                        let quote = self.kw(Kw::Quote);
                        Ok(self.form(quote, &[stripped]))
                    }
                }
                _ => Ok(x),
            },
            Kw::Quasiquote => match self.items(x).as_deref() {
                Some(&[_, template]) => {
                    let q = self.quasi(template, 0, env);
                    let code = self.emit(q);
                    self.expand(code, env)
                }
                _ => Err(located()),
            },
            Kw::Unquote | Kw::UnquoteSplicing => Err(SyntaxError {
                who: Some(if kw == Kw::Unquote {
                    "unquote"
                } else {
                    "unquote-splicing"
                }),
                message: "expression not valid outside of quasiquote",
                form: x,
                subform: Value::FALSE,
                at: Some(x),
            }
            .into()),
            Kw::Lambda => self.lambda(x, env),
            Kw::LambdaStar => {
                let code = self.lambda_star(x)?;
                self.expand(code, env)
            }
            // In an expression: the compiler's error.
            Kw::Define | Kw::DefinePublic => Ok(x),
            Kw::DefineStar | Kw::DefineStarPublic => {
                self.define_star(x, kw == Kw::DefineStarPublic)
            }
            Kw::Set => self.set(x, env),
            Kw::If | Kw::Begin | Kw::And | Kw::Or | Kw::When | Kw::Unless | Kw::CallValues => {
                self.operands(x, true, env)
            }
            Kw::Let => self.let_(x, env),
            Kw::LetStar | Kw::Letrec | Kw::LetrecStar => self.let_star(x, kw, env),
            Kw::Cond => self.cond(x, env),
            Kw::Case => self.case(x, env),
            Kw::Do => {
                let code = self.do_(x)?;
                self.expand(code, env)
            }
            Kw::While => {
                let code = self.while_(x)?;
                self.expand(code, env)
            }
            Kw::CaseLambda => {
                let code = self.case_lambda(x)?;
                self.expand(code, env)
            }
            Kw::Parameterize | Kw::WithFluids => self.parameterize(x, env),
            Kw::DefineModule | Kw::UseModules | Kw::Export | Kw::ReExport | Kw::At | Kw::AtAt => {
                let rest = self.cdr(x);
                let stripped = self.strip(rest);
                if stripped == rest {
                    Ok(x)
                } else {
                    let head = self.car(x);
                    Ok(self.cx.heap.cons(head, stripped))
                }
            }
            Kw::EvalWhen => match self.eval_when(x, false)? {
                Value::UNSPECIFIED => Ok(Value::UNSPECIFIED),
                y => self.expand(y, env),
            },
            Kw::DefineSyntax
            | Kw::DefineSyntaxPublic
            | Kw::DefineSyntaxRule
            | Kw::DefineSyntaxRulePublic
            | Kw::Defmacro
            | Kw::DefmacroPublic
            | Kw::DefmacroStar
            | Kw::DefmacroStarPublic
            | Kw::DefineMacro => Err(syntax(
                "definition in expression context, where definitions are not allowed,",
                x,
                Some(x),
            )
            .into()),
            Kw::LetSyntax | Kw::LetrecSyntax => self.let_syntax(x, kw, env),
            Kw::SyntaxRules => Err(located()),
            Kw::Dyn => match self.items(x).as_deref() {
                Some(&[_, kind, ref parts @ ..]) if parts.len() == 4 => {
                    let mut new = vec![kind];
                    for &p in parts {
                        new.push(self.expand(p, env)?);
                    }
                    let items = self.items(x).unwrap_or_default();
                    Ok(self.rebuild(x, &items[1..], &new))
                }
                _ => Ok(x),
            },
            Kw::SyntaxCase | Kw::Syntax | Kw::Quasisyntax | Kw::WithSyntax => {
                // Through the hooks' pointers, so that wasm-split moves the
                // part out of the core (D273).
                let hooks = core::hint::black_box(&syntax_case::HOOKS);
                let hook = match kw {
                    Kw::SyntaxCase => hooks.syntax_case,
                    Kw::Syntax => hooks.syntax,
                    Kw::Quasisyntax => hooks.quasisyntax,
                    _ => hooks.with_syntax,
                };
                hook(self, x, env)
            }
            Kw::Unsyntax | Kw::UnsyntaxSplicing => Err(SyntaxError {
                who: Some(if kw == Kw::Unsyntax {
                    "unsyntax"
                } else {
                    "unsyntax-splicing"
                }),
                message: "expression not valid outside of quasisyntax",
                form: x,
                subform: Value::FALSE,
                at: Some(x),
            }
            .into()),
            Kw::Procedure => Ok(x),
        }
    }

    /// Binds a `lambda`'s formals; the scope's length before them, or
    /// `None` if they are not symbols.
    fn bind_formals(&self, formals: Value, env: &mut Env) -> Option<usize> {
        let (names, tail) = self.split(formals);
        if !matches!(tail.tag(), Tag::Null | Tag::Symbol)
            || names.iter().any(|n| n.tag() != Tag::Symbol)
        {
            return None;
        }
        let mark = env.scope.len();
        for name in names
            .into_iter()
            .chain((tail != Value::NULL).then_some(tail))
        {
            env.scope.push((name, Value::UNDEFINED));
        }
        Some(mark)
    }

    fn lambda(&mut self, x: Value, env: &mut Env) -> R<Value> {
        let rest = self.cdr(x);
        if rest.tag() != Tag::Pair {
            return Ok(x);
        }
        let (formals, body) = (self.car(rest), self.cdr(rest));
        let Some(mark) = self.bind_formals(formals, env) else {
            return Ok(x);
        };
        let new = self.body(body, env);
        env.scope.truncate(mark);
        let new = new?;
        if new == body {
            return Ok(x);
        }
        let lambda = self.car(x);
        let rest = self.cx.heap.cons(formals, new);
        Ok(self.cx.heap.cons(lambda, rest))
    }

    /// A body: its macro uses expanded until each form is a definition, a
    /// macro definition, a `begin` (spliced) or an expression; then the
    /// definitions' values and the expressions, with every name it defines
    /// in scope.
    fn body(&mut self, forms: Value, env: &mut Env) -> R<Value> {
        let Some(items) = self.items(forms) else {
            return Ok(forms);
        };
        let mark = env.scope.len();
        let r = self.body_items(&items, env);
        env.scope.truncate(mark);
        let new = r?;
        Ok(if new == items { forms } else { self.list(&new) })
    }

    fn body_items(&mut self, items: &[Value], env: &mut Env) -> R<Vec<Value>> {
        let mut pending: Vec<Value> = items.iter().rev().copied().collect();
        let mut scanned = Vec::new();
        while let Some(f) = pending.pop() {
            let g = self.expand_head(f, env)?;
            let kw = self.head_keyword(g, env);
            if kw == Some(Kw::Begin)
                && let Some(inner) = self.items(self.cdr(g))
            {
                pending.extend(inner.into_iter().rev());
                continue;
            }
            match kw {
                Some(kw @ (Kw::Define | Kw::DefineStar)) => {
                    let d = if kw == Kw::DefineStar {
                        self.define_star(g, false)?
                    } else {
                        g
                    };
                    let target = self.car(self.cdr(d));
                    let name = if target.tag() == Tag::Pair {
                        self.car(target)
                    } else {
                        target
                    };
                    if name.tag() == Tag::Symbol {
                        env.scope.push((name, Value::UNDEFINED));
                    }
                    scanned.push((true, d));
                }
                Some(kw) if kw.defines_syntax() => self.define_syntax(g, kw, env, false)?,
                _ => scanned.push((false, g)),
            }
        }
        let mut new = Vec::with_capacity(scanned.len());
        for (definition, f) in scanned {
            new.push(if definition {
                self.definition(f, Kw::Define, env)?
            } else {
                self.expand(f, env)?
            });
        }
        Ok(new)
    }

    /// `(define name value)` or `(define (name . formals) body …)`, and
    /// `define-public`, `define*` and `define*-public` as one of them.
    fn definition(&mut self, x: Value, kw: Kw, env: &mut Env) -> R<Value> {
        let x = match kw {
            Kw::DefineStar | Kw::DefineStarPublic => {
                self.define_star(x, kw == Kw::DefineStarPublic)?
            }
            _ => x,
        };
        let rest = self.cdr(x);
        if rest.tag() != Tag::Pair {
            return Ok(x);
        }
        let (target, body) = (self.car(rest), self.cdr(rest));
        let new_body = match target.tag() {
            Tag::Symbol => match self.items(body).as_deref() {
                Some(&[value]) => {
                    let new = self.expand(value, env)?;
                    if new == value {
                        body
                    } else {
                        self.list(&[new])
                    }
                }
                _ => body,
            },
            Tag::Pair if self.car(target).tag() == Tag::Symbol => {
                let Some(mark) = self.bind_formals(self.cdr(target), env) else {
                    return Ok(x);
                };
                if env.depth >= MAX_NESTING {
                    env.scope.truncate(mark);
                    return Err(syntax("expression nested too deeply", x, None).into());
                }
                env.depth += 1;
                let new = self.body(body, env);
                env.depth -= 1;
                env.scope.truncate(mark);
                new?
            }
            _ => body,
        };
        if new_body == body {
            return Ok(x);
        }
        let symbol = self.car(x);
        let rest = self.cx.heap.cons(target, new_body);
        Ok(self.cx.heap.cons(symbol, rest))
    }

    /// `(define* (name . formals) body …)` as `(define name (lambda*
    /// formals body …))`, and `(define* name value)` as `define`;
    /// `define*-public` as `define-public`.
    fn define_star(&mut self, x: Value, public: bool) -> R<Value> {
        let rest = self.cdr(x);
        let target = self.car(rest);
        if rest.tag() != Tag::Pair {
            return Err(syntax(FAILED, x, Some(x)).into());
        }
        let define = self.kw(if public { Kw::DefinePublic } else { Kw::Define });
        if target.tag() != Tag::Pair {
            let tail = self.cdr(rest);
            let rest = self.cx.heap.cons(target, tail);
            return Ok(self.cx.heap.cons(define, rest));
        }
        let lambda_star = self.kw(Kw::LambdaStar);
        let formals = self.cdr(target);
        let body = self.cdr(rest);
        let tail = self.cx.heap.cons(formals, body);
        let lambda = self.cx.heap.cons(lambda_star, tail);
        let name = self.car(target);
        Ok(self.form(define, &[name, lambda]))
    }

    fn set(&mut self, x: Value, env: &mut Env) -> R<Value> {
        let Some(items) = self.items(x) else {
            return Ok(x);
        };
        let [_, target, value] = items[..] else {
            return Ok(x);
        };
        if target.tag() == Tag::Pair {
            // `(set! (f arg …) value)` as `((setter f) arg … value)`.
            let Some(place) = self.items(target) else {
                return Err(syntax(FAILED, x, Some(x)).into());
            };
            let setter = self.guile("setter");
            let procedure = self.list(&[setter, place[0]]);
            let mut args = place[1..].to_vec();
            args.push(value);
            let call = self.form(procedure, &args);
            return self.expand(call, env);
        }
        let new_target = match self.meaning(target, env) {
            Meaning::Lexical(found) => found,
            Meaning::Macro(_) | Meaning::Pattern(_) => {
                return Err(syntax(FAILED, x, Some(x)).into());
            }
            _ => target,
        };
        let new_value = self.expand(value, env)?;
        Ok(self.rebuild(x, &items[1..], &[new_target, new_value]))
    }

    /// `((name init) …)` as names and inits.
    fn bindings(&self, list: Value) -> Option<Vec<(Value, Value)>> {
        let mut out = Vec::new();
        for b in self.items(list)? {
            match self.items(b).as_deref() {
                Some(&[name, init]) if name.tag() == Tag::Symbol => out.push((name, init)),
                _ => return None,
            }
        }
        Some(out)
    }

    /// The bindings' list with new inits, the old one if none changed.
    fn new_bindings(&mut self, list: Value, old: &[(Value, Value)], inits: &[Value]) -> Value {
        if old.iter().zip(inits).all(|(o, n)| o.1 == *n) {
            return list;
        }
        let items: Vec<Value> = old
            .iter()
            .zip(inits)
            .map(|(&(name, _), &init)| self.cx.heap.list(&[name, init]))
            .collect();
        self.list(&items)
    }

    fn let_(&mut self, x: Value, env: &mut Env) -> R<Value> {
        let Some(items) = self.items(x) else {
            return Ok(x);
        };
        let named = items.get(1).is_some_and(|n| n.tag() == Tag::Symbol);
        let at = 1 + usize::from(named);
        let Some(bindings) = items.get(at).and_then(|&b| self.bindings(b)) else {
            return Ok(x);
        };
        let mut inits = Vec::new();
        for &(_, init) in &bindings {
            inits.push(self.expand(init, env)?);
        }
        let mark = env.scope.len();
        if named {
            env.scope.push((items[1], Value::UNDEFINED));
        }
        for &(name, _) in &bindings {
            env.scope.push((name, Value::UNDEFINED));
        }
        let body = self.list(&items[at + 1..]);
        let new_body = self.body(body, env);
        env.scope.truncate(mark);
        let new_body = new_body?;
        let list = self.new_bindings(items[at], &bindings, &inits);
        let mut new = items[1..at].to_vec();
        new.push(list);
        new.extend(self.items(new_body).unwrap_or_default());
        Ok(self.rebuild(x, &items[1..], &new))
    }

    /// `let*`, each init seeing the names before it; `letrec` and
    /// `letrec*`, all of them.
    fn let_star(&mut self, x: Value, kw: Kw, env: &mut Env) -> R<Value> {
        let Some(items) = self.items(x) else {
            return Ok(x);
        };
        let Some(bindings) = items.get(1).and_then(|&b| self.bindings(b)) else {
            return Ok(x);
        };
        let mark = env.scope.len();
        if kw != Kw::LetStar {
            for &(name, _) in &bindings {
                env.scope.push((name, Value::UNDEFINED));
            }
        }
        let r = self.let_star_parts(&items, &bindings, kw, env);
        env.scope.truncate(mark);
        let (inits, new_body) = r?;
        let list = self.new_bindings(items[1], &bindings, &inits);
        let mut new = vec![list];
        new.extend(self.items(new_body).unwrap_or_default());
        Ok(self.rebuild(x, &items[1..], &new))
    }

    fn let_star_parts(
        &mut self,
        items: &[Value],
        bindings: &[(Value, Value)],
        kw: Kw,
        env: &mut Env,
    ) -> R<(Vec<Value>, Value)> {
        let mut inits = Vec::new();
        for &(name, init) in bindings {
            inits.push(self.expand(init, env)?);
            if kw == Kw::LetStar {
                env.scope.push((name, Value::UNDEFINED));
            }
        }
        let body = self.list(&items[2..]);
        Ok((inits, self.body(body, env)?))
    }

    /// A clause's expressions after its test, `=>` kept as it is.
    fn clause_rest(&mut self, rest: &[Value], env: &mut Env) -> R<Vec<Value>> {
        let mut new = Vec::new();
        for (i, &e) in rest.iter().enumerate() {
            new.push(if i == 0 && self.is_free(e, self.expander.arrow, env) {
                e
            } else {
                self.expand(e, env)?
            });
        }
        Ok(new)
    }

    fn cond(&mut self, x: Value, env: &mut Env) -> R<Value> {
        let Some(items) = self.items(x) else {
            return Ok(x);
        };
        let mut new = Vec::new();
        for &clause in &items[1..] {
            let Some(parts) = self.items(clause).filter(|p| !p.is_empty()) else {
                new.push(clause);
                continue;
            };
            let test = if self.is_free(parts[0], self.expander.else_, env) {
                parts[0]
            } else {
                self.expand(parts[0], env)?
            };
            let mut new_parts = vec![test];
            new_parts.extend(self.clause_rest(&parts[1..], env)?);
            new.push(if new_parts == parts {
                clause
            } else {
                self.list(&new_parts)
            });
        }
        Ok(self.rebuild(x, &items[1..], &new))
    }

    fn case(&mut self, x: Value, env: &mut Env) -> R<Value> {
        let Some(items) = self.items(x) else {
            return Ok(x);
        };
        let Some(&key) = items.get(1) else {
            return Ok(x);
        };
        let mut new = vec![self.expand(key, env)?];
        for &clause in &items[2..] {
            let Some(parts) = self.items(clause).filter(|p| !p.is_empty()) else {
                new.push(clause);
                continue;
            };
            let data = if self.is_free(parts[0], self.expander.else_, env) {
                parts[0]
            } else {
                self.strip(parts[0])
            };
            let mut new_parts = vec![data];
            new_parts.extend(self.clause_rest(&parts[1..], env)?);
            new.push(if new_parts == parts {
                clause
            } else {
                self.list(&new_parts)
            });
        }
        Ok(self.rebuild(x, &items[1..], &new))
    }

    fn parameterize(&mut self, x: Value, env: &mut Env) -> R<Value> {
        let Some(items) = self.items(x) else {
            return Ok(x);
        };
        let Some(list) = items.get(1).and_then(|&b| self.items(b)) else {
            return Ok(x);
        };
        let mut pairs = Vec::new();
        for b in list {
            let Some(&[target, value]) = self.items(b).as_deref() else {
                return Ok(x);
            };
            let (t, v) = (self.expand(target, env)?, self.expand(value, env)?);
            pairs.push(if (t, v) == (target, value) {
                b
            } else {
                self.list(&[t, v])
            });
        }
        let pairs = self.list(&pairs);
        let pairs = if self.items(pairs) == self.items(items[1]) {
            items[1]
        } else {
            pairs
        };
        let body = self.list(&items[2..]);
        let new_body = self.body(body, env)?;
        let mut new = vec![pairs];
        new.extend(self.items(new_body).unwrap_or_default());
        Ok(self.rebuild(x, &items[1..], &new))
    }

    /// `(eval-when (situation …) body …)`: the body as a `begin` if it is
    /// evaluated, else nothing. At toplevel `expand` evaluates it too, as
    /// Guile's evaluator does; inside, only `eval`.
    fn eval_when(&mut self, x: Value, top: bool) -> R<Value> {
        let items = self.items(x).unwrap_or_default();
        let situations = items.get(1).and_then(|&s| self.items(s));
        let (Some(situations), true) = (situations, items.len() >= 3) else {
            return Err(syntax(FAILED, x, None).into());
        };
        let mut evaluated = false;
        for s in situations {
            let name = self.cx.heap.name(base(&self.cx.globals.aliases, s));
            let name = name.filter(|_| s.tag() == Tag::Symbol);
            match name {
                Some("eval") => evaluated = true,
                Some("expand") => evaluated |= top,
                Some("load" | "compile") => {}
                _ => {
                    return Err(SyntaxError {
                        who: Some("eval-when"),
                        message: "invalid situation",
                        form: x,
                        subform: s,
                        at: None,
                    }
                    .into());
                }
            }
        }
        if !evaluated {
            return Ok(Value::UNSPECIFIED);
        }
        let begin = self.kw(Kw::Begin);
        Ok(self.form(begin, &items[2..]))
    }

    fn let_syntax(&mut self, x: Value, kw: Kw, env: &mut Env) -> R<Value> {
        let Some(items) = self.items(x) else {
            return Err(syntax(FAILED, x, Some(x)).into());
        };
        let Some(bindings) = items.get(1).and_then(|&b| self.bindings(b)) else {
            return Err(syntax(FAILED, x, Some(x)).into());
        };
        let mark = env.scope.len();
        let depth = mark
            + if kw == Kw::LetrecSyntax {
                bindings.len()
            } else {
                0
            };
        let mut macros = Vec::new();
        for &(name, spec) in &bindings {
            macros.push(self.transformer(name, spec, env, depth as u32)?);
        }
        for (&(name, _), m) in bindings.iter().zip(macros) {
            env.scope.push((name, m));
        }
        let body = self.list(&items[2..]);
        let new_body = self.body(body, env);
        env.scope.truncate(mark);
        let new_body = new_body?;
        let let_ = self.kw(Kw::Let);
        Ok(self.form_star(let_, &[Value::NULL], new_body))
    }

    // --- macros

    /// Defines the macro of `x`, a `define-syntax` or one of its kind: in
    /// the module at toplevel, else in the body's scope.
    fn define_syntax(&mut self, x: Value, kw: Kw, env: &mut Env, top: bool) -> R<()> {
        let located = || -> Fail { syntax(FAILED, x, Some(x)).into() };
        let items = self.items(x).ok_or_else(located)?;
        let depth = if top { 0 } else { env.scope.len() as u32 + 1 };
        let lambda = self.kw(Kw::Lambda);
        let (name, value) = match kw {
            Kw::DefineSyntax | Kw::DefineSyntaxPublic => {
                let [_, name, spec] = items[..] else {
                    return Err(located());
                };
                if name.tag() != Tag::Symbol {
                    return Err(located());
                }
                (name, self.transformer(name, spec, env, depth)?)
            }
            Kw::DefineSyntaxRule | Kw::DefineSyntaxRulePublic => {
                let (pattern, template) = match items[..] {
                    [_, pattern, template] => (pattern, template),
                    [_, pattern, doc, template] if doc.tag() == Tag::String => (pattern, template),
                    _ => return Err(located()),
                };
                let name = self.car(pattern);
                if pattern.tag() != Tag::Pair || name.tag() != Tag::Symbol {
                    return Err(located());
                }
                let rule = self.list(&[pattern, template]);
                let syntax_rules = self.kw(Kw::SyntaxRules);
                let spec = self.form(syntax_rules, &[Value::NULL, rule]);
                (name, self.transformer(name, spec, env, depth)?)
            }
            Kw::Defmacro | Kw::DefmacroPublic | Kw::DefmacroStar | Kw::DefmacroStarPublic => {
                let [_, name, formals, ref body @ ..] = items[..] else {
                    return Err(located());
                };
                if name.tag() != Tag::Symbol || body.is_empty() {
                    return Err(located());
                }
                let body = self.list_of(body);
                // `(ice-9 optargs)`' `defmacro*` takes `lambda*`'s formals.
                let lambda = match kw {
                    Kw::DefmacroStar | Kw::DefmacroStarPublic => self.kw(Kw::LambdaStar),
                    _ => lambda,
                };
                let procedure = self.form_star(lambda, &[formals], body);
                (name, self.defmacro(name, procedure, env, depth)?)
            }
            _ => match items[..] {
                // `define-macro`.
                [_, target, ref body @ ..] if target.tag() == Tag::Pair && !body.is_empty() => {
                    let (name, formals) = (self.car(target), self.cdr(target));
                    if name.tag() != Tag::Symbol {
                        return Err(located());
                    }
                    let body = self.list_of(body);
                    let procedure = self.form_star(lambda, &[formals], body);
                    (name, self.defmacro(name, procedure, env, depth)?)
                }
                [_, name, expression] if name.tag() == Tag::Symbol => {
                    (name, self.defmacro(name, expression, env, depth)?)
                }
                _ => return Err(located()),
            },
        };
        if top {
            let slot = self.cx.globals.cell(env.module, name);
            self.cx.globals.define(slot, value);
            if kw.public() {
                self.cx.globals.export(env.module, name, name);
            }
        } else {
            env.scope.push((name, value));
        }
        Ok(())
    }

    fn list_of(&mut self, items: &[Value]) -> Value {
        self.cx.heap.list(items)
    }

    /// A macro from `define-syntax`'s transformer: `syntax-rules`, parsed;
    /// else, where `name` is a native macro, that macro (D60), its
    /// transformer not evaluated; else the procedure the transformer
    /// evaluates to, now, at toplevel in the module (D273).
    fn transformer(&mut self, name: Value, spec: Value, env: &mut Env, depth: u32) -> R<Value> {
        if self.head_keyword(spec, env) == Some(Kw::SyntaxRules) {
            let rules = Rules::parse(&mut self.cx.heap, &self.cx.globals.aliases, spec)?;
            return Ok(self.cx.heap.wrap(Macro {
                name,
                module: env.module,
                depth,
                kind: MacroKind::Rules(Rc::new(rules)),
            }));
        }
        if let Meaning::Macro(m) = self.meaning(name, env)
            && let Some(mac) = self.cx.heap.foreign::<Macro>(m)
            && matches!(mac.kind, MacroKind::Assert | MacroKind::SyntaxFunction)
        {
            return Ok(m);
        }
        let procedure = self.evaluate(spec, env.module)?;
        Ok(self.cx.heap.wrap(Macro {
            name,
            module: env.module,
            depth,
            kind: MacroKind::Procedure(procedure),
        }))
    }

    fn defmacro(&mut self, name: Value, procedure: Value, env: &mut Env, depth: u32) -> R<Value> {
        let procedure = self.evaluate(procedure, env.module)?;
        Ok(self.cx.heap.wrap(Macro {
            name,
            module: env.module,
            depth,
            kind: MacroKind::Defmacro(procedure),
        }))
    }

    /// Evaluates a transformer's expression now, at toplevel in `module`.
    fn evaluate(&mut self, expression: Value, module: u32) -> R<Value> {
        let mut env = Env::new(module, false);
        let expanded = self.expand_top(expression, &mut env)?;
        let code = compile::compile(
            &mut self.cx.heap,
            &mut self.cx.globals,
            &self.syntax,
            expanded,
            module,
            false,
        )?;
        let closure = self.cx.heap.closure(code, Default::default());
        self.apply(closure, &[]).map_err(Fail::Thrown)
    }

    /// The form a macro's use `x` expands to, once.
    fn transcribe(&mut self, m: Value, x: Value, env: &mut Env) -> R<Value> {
        let Some(mac) = self.cx.heap.foreign::<Macro>(m) else {
            return Ok(x);
        };
        let (kind, module, depth) = (mac.kind.clone(), mac.module, mac.depth);
        match kind {
            MacroKind::Rules(rules) => {
                let found = {
                    let bound = |v: Value| self.is_lexical(v, env);
                    rules.find(&self.cx.heap, &self.cx.globals.aliases, &bound, x)
                };
                let Some((bindings, template)) = found else {
                    return Err(syntax(FAILED, x, Some(x)).into());
                };
                let mut renamer = Renamer {
                    aliases: &mut self.cx.globals.aliases,
                    module,
                    depth,
                    done: Vec::new(),
                };
                Ok(rules.instantiate(&mut self.cx.heap, &mut renamer, template, &bindings)?)
            }
            MacroKind::Defmacro(procedure) => {
                let Some(args) = self.items(self.cdr(x)) else {
                    return Err(syntax(FAILED, x, Some(x)).into());
                };
                let args: Vec<Value> = args.into_iter().map(|a| self.strip(a)).collect();
                self.apply(procedure, &args).map_err(Fail::Thrown)
            }
            MacroKind::Procedure(procedure) => {
                // The templates it instantiates are this expansion's: each
                // symbol renamed once, in the macro's module (D273).
                self.cx.transcriptions.push(Transcription {
                    module,
                    depth,
                    done: Vec::new(),
                });
                let r = self.apply(procedure, &[x]);
                self.cx.transcriptions.pop();
                r.map_err(|e| {
                    // psyntax places a syntax error by its form: done
                    // where the form's place is known (`Vm::failed`).
                    let place = self.cx.heap.cdr(e.args).and_then(|a| self.cx.heap.cdr(a));
                    if self.cx.heap.name(e.key) == Some("syntax-error")
                        && place.and_then(|p| self.cx.heap.car(p)) == Some(Value::FALSE)
                    {
                        self.cx.unplaced = e.args;
                    }
                    Fail::Thrown(e)
                })
            }
            MacroKind::Assert => self.assert(x, module),
            MacroKind::SyntaxFunction => self.syntax_function(x, module),
            MacroKind::Curried(kw) => self.curried(x, kw),
            MacroKind::Include => self.include(x, module),
        }
    }

    /// `(include file)`: Guile's error for a file it cannot open, as the
    /// engine's Scheme reads no files; at run time, where Guile's reading
    /// at expansion would have thrown. A relative name is Guile's in the
    /// directory of a file at the top of the search path, `./`.
    fn include(&mut self, x: Value, module: u32) -> R<Value> {
        let file = match self.items(x).unwrap_or_default()[..] {
            [_, file] => self.strip(file),
            _ => return Err(syntax(FAILED, x, Some(x)).into()),
        };
        let file = match self.cx.heap.str(file).map(|s| String::from(s.as_str())) {
            Some(name) if !name.starts_with('/') => self.cx.heap.string(alloc::format!("./{name}")),
            _ => file,
        };
        let quote = self.kw(Kw::Quote);
        let key = self.cx.heap.intern("system-error");
        let key = self.list(&[quote, key]);
        let subr = self.cx.heap.string("open-file");
        let message = self.cx.heap.string("~A: ~S");
        let reason = self.cx.heap.string("No such file or directory");
        let list = self.module_ref("list", module);
        let args = self.form(list, &[reason, file]);
        let errno = self.cx.heap.integer(2);
        let errno = self.form(list, &[errno]);
        let throw = self.module_ref("throw", module);
        Ok(self.form(throw, &[key, subr, message, args, errno]))
    }

    /// `(define ((f a) b) body …)` as `lily/curried-definitions.scm` has
    /// it: `(define (f a) (lambda (b) body …))`, again until the name is
    /// reached, a docstring moved to the outermost procedure; `define*`
    /// makes `lambda*`s. `(define name value)` and `define-public`'s
    /// `(define-public target body …)` are the core keyword's. A form that
    /// fits none of the file's `syntax-case` patterns fails there, with its
    /// place: `(define x)` too, which Guile's own `define` takes.
    fn curried(&mut self, x: Value, kw: Kw) -> R<Value> {
        let (items, tail) = self.split(x);
        let target = items.get(1).copied().unwrap_or(Value::FALSE);
        let head = self.car(target);
        let public = matches!(kw, Kw::DefinePublic | Kw::DefineStarPublic);
        let fits = match public {
            true => items.len() >= 3,
            false => items.len() == 3 || (items.len() > 3 && target.tag() == Tag::Pair),
        };
        if tail != Value::NULL || !fits {
            return Err(syntax(FAILED, x, Some(x)).into());
        }
        if items.len() >= 3 && head.tag() == Tag::Pair {
            let lambda = match kw {
                Kw::DefineStar | Kw::DefineStarPublic => self.kw(Kw::LambdaStar),
                _ => self.kw(Kw::Lambda),
            };
            let body = &items[2..];
            let (doc, body) = match body {
                [doc, rest @ ..] if !rest.is_empty() && doc.tag() == Tag::String => {
                    (Some(*doc), rest)
                }
                _ => (None, body),
            };
            let mut inner = vec![self.cdr(target)];
            inner.extend_from_slice(body);
            let inner = self.form(lambda, &inner);
            let mut new = vec![head];
            new.extend(doc);
            new.push(inner);
            return Ok(self.form(items[0], &new));
        }
        let core = self.kw(kw);
        let rest = self.cdr(x);
        Ok(self.cx.heap.cons(core, rest))
    }

    // --- (lily)'s native macros

    /// `(assert condition [message])`, as `lily-library.scm`'s.
    fn assert(&mut self, x: Value, module: u32) -> R<Value> {
        let items = self.items(x).unwrap_or_default();
        let (condition, message) = match items[..] {
            [_, c] => (c, None),
            [_, c, m] => (c, Some(m)),
            _ => return Err(syntax(FAILED, x, Some(x)).into()),
        };
        let quote = self.kw(Kw::Quote);
        let quoted = self.list(&[quote, condition]);
        let text = match message {
            None => "assertion ~s failed",
            Some(_) => "assertion ~s failed with message: ~a",
        };
        let text = self.cx.heap.string(text);
        let format = self.module_ref("format", module);
        let mut args = vec![Value::FALSE, text, quoted];
        args.extend(message);
        let formatted = self.form(format, &args);
        let error = self.module_ref("error", module);
        let call = self.list(&[error, formatted]);
        let not = self.module_ref("not", module);
        let test = self.list(&[not, condition]);
        let when = self.kw(Kw::When);
        Ok(self.list(&[when, test, call]))
    }

    /// `(define-syntax-function type args (signature …) body …)`, as
    /// `music-functions.scm`'s: `(ly:make-music-function (list type
    /// signature …) procedure)`, the procedure's header the signature.
    fn syntax_function(&mut self, x: Value, module: u32) -> R<Value> {
        let located = || -> Fail { syntax(FAILED, x, Some(x)).into() };
        let rest = self.cdr(x);
        let (items, body) = {
            let mut items = Vec::new();
            let mut at = rest;
            while items.len() < 3 && at.tag() == Tag::Pair {
                items.push(self.car(at));
                at = self.cdr(at);
            }
            (items, at)
        };
        let [kind, args, signature] = items[..] else {
            return Err(located());
        };
        let Some(predicates) = self.items(signature) else {
            return Err(located());
        };
        let (doc, body) = self.docstring(body);
        let header = self.cx.heap.cons(kind, signature);
        let header = self.strip(header);
        let procedure = self.currying(args, doc, body, predicates.len(), header, module);
        let mut list = vec![self.module_ref("list", module)];
        for s in core::iter::once(kind).chain(predicates) {
            list.push(self.maybe_default(s, module));
        }
        let list = self.list(&list);
        let make = self.module_ref("ly:make-music-function", module);
        Ok(self.list(&[make, list, procedure]))
    }

    /// `take-body-docstring`: a docstring, `(_i "…")` or a string, before
    /// at least one more form; else `""`.
    fn docstring(&mut self, body: Value) -> (Value, Value) {
        let (first, rest) = (self.car(body), self.cdr(body));
        if body.tag() == Tag::Pair && rest.tag() == Tag::Pair {
            if first.tag() == Tag::String {
                return (first, rest);
            }
            if let Some(&[head, doc]) = self.items(first).as_deref()
                && doc.tag() == Tag::String
                && head.tag() == Tag::Symbol
                && base(&self.cx.globals.aliases, head) == self.expander.underscore_i
            {
                return (doc, rest);
            }
        }
        (self.cx.heap.string(""), body)
    }

    /// `maybe-default`: `(pred default)` as `(cons pred default)`, `(pred)`
    /// as `(cons pred #f)`.
    fn maybe_default(&mut self, s: Value, module: u32) -> Value {
        let (element, default) = match self.items(s).as_deref() {
            Some(&[e, d]) => (e, d),
            Some(&[e]) => (e, Value::FALSE),
            _ => return s,
        };
        let cons = self.module_ref("cons", module);
        self.list(&[cons, element, default])
    }

    /// `currying-lambda`: argument lists of lists curry, and two arguments
    /// more than the signature, or a dotted list from `parser`, are the
    /// old parser and location arguments.
    fn currying(
        &mut self,
        mut args: Value,
        doc: Value,
        mut body: Value,
        length: usize,
        header: Value,
        module: u32,
    ) -> Value {
        let lambda = self.kw(Kw::Lambda);
        while args.tag() == Tag::Pair && self.car(args).tag() == Tag::Pair {
            let inner = self.form_star(lambda, &[self.cdr(args)], body);
            body = self.list(&[inner]);
            args = self.car(args);
        }
        let (compatibility, args) = match self.items(args) {
            Some(list) if list.len() >= 2 && list.len() - 2 == length => {
                (Some((list[0], list[1])), self.cdr(self.cdr(args)))
            }
            Some(_) => (None, args),
            None if self.cdr(args).tag() == Tag::Pair
                && base(&self.cx.globals.aliases, self.car(args)) == self.expander.parser =>
            {
                let (p, l) = (self.car(args), self.car(self.cdr(args)));
                (Some((p, l)), self.cdr(self.cdr(args)))
            }
            None => (None, args),
        };
        // `final-lambda`.
        let body = match compatibility {
            Some((parser, location)) => {
                let get_parser = self.module_ref("*parser*", module);
                let get_location = self.module_ref("*location*", module);
                let p = self.list(&[get_parser]);
                let l = self.list(&[get_location]);
                let p = self.list(&[parser, p]);
                let l = self.list(&[location, l]);
                let bindings = self.list(&[p, l]);
                let let_ = self.kw(Kw::Let);
                let inner = self.form_star(let_, &[bindings], body);
                self.list(&[inner])
            }
            None => body,
        };
        let marker = self.syntax.procedure();
        let marker = self.list(&[marker, Value::FALSE, header]);
        let procedure = self.form_star(lambda, &[args, marker, doc], body);
        let result = self.fresh("result");
        let binding = self.list(&[result, procedure]);
        let bindings = self.list(&[binding]);
        let property = self.module_ref("syntax-function-procedure-arguments", module);
        let place = self.list(&[property, result]);
        let quote = self.kw(Kw::Quote);
        let quoted = self.list(&[quote, args]);
        let set = self.kw(Kw::Set);
        let set = self.list(&[set, place, quoted]);
        let let_ = self.kw(Kw::Let);
        self.list(&[let_, bindings, set, result])
    }

    // --- Guile's forms, native

    /// `lambda*`'s formals, as psyntax reads them.
    fn star_formals(&mut self, formals: Value) -> R<Star> {
        #[derive(PartialEq)]
        enum State {
            Required,
            Optional,
            Key,
        }
        let mut star = Star {
            required: Vec::new(),
            optional: Vec::new(),
            keys: None,
            allow_other_keys: false,
            rest: None,
        };
        let mut state = State::Required;
        let mut at = formals;
        let keyword = |heap: &Heap, v: Value, name: &str| {
            v.tag() == Tag::Keyword && heap.name(v) == Some(name)
        };
        loop {
            let error = |state: &State, at: Value| -> Fail {
                let message = match state {
                    State::Required => "invalid argument list",
                    State::Optional => "invalid optional argument list",
                    State::Key => "invalid keyword argument list",
                };
                SyntaxError {
                    who: Some("lambda*"),
                    message,
                    form: formals,
                    subform: at,
                    at: Some(formals),
                }
                .into()
            };
            match at.tag() {
                Tag::Null => break,
                Tag::Symbol => {
                    star.rest = Some(at);
                    break;
                }
                Tag::Pair => {}
                _ => return Err(error(&state, at)),
            }
            let (a, next) = (self.car(at), self.cdr(at));
            let heap = &self.cx.heap;
            if keyword(heap, a, "rest") {
                match self.items(next).as_deref() {
                    Some(&[r]) if r.tag() == Tag::Symbol => {
                        star.rest = Some(r);
                        break;
                    }
                    _ if state == State::Required => return Err(error(&state, at)),
                    _ => {
                        return Err(SyntaxError {
                            who: Some("lambda*"),
                            message: "invalid rest argument",
                            form: formals,
                            subform: next,
                            at: Some(formals),
                        }
                        .into());
                    }
                }
            }
            if a.tag() == Tag::Symbol {
                match state {
                    State::Required => star.required.push(a),
                    State::Optional => star.optional.push((a, Value::FALSE)),
                    State::Key => {
                        let name = String::from(heap.name(a).unwrap_or(""));
                        let k = self.cx.heap.keyword(&name);
                        if let Some(keys) = &mut star.keys {
                            keys.push((k, a, Value::FALSE));
                        }
                    }
                }
            } else if keyword(heap, a, "optional") && state == State::Required {
                state = State::Optional;
            } else if keyword(heap, a, "key") && state != State::Key {
                state = State::Key;
                star.keys = Some(Vec::new());
            } else if keyword(heap, a, "allow-other-keys") && state == State::Key {
                star.allow_other_keys = true;
                match next.tag() {
                    Tag::Null => break,
                    Tag::Symbol => {
                        star.rest = Some(next);
                        break;
                    }
                    _ if keyword(heap, self.car(next), "rest") => {}
                    _ => return Err(error(&state, at)),
                }
            } else {
                let parts = self.items(a);
                match (&state, parts.as_deref()) {
                    (State::Optional, Some(&[v, init])) if v.tag() == Tag::Symbol => {
                        star.optional.push((v, init));
                    }
                    (State::Key, Some(&[v, init])) if v.tag() == Tag::Symbol => {
                        let name = String::from(heap.name(v).unwrap_or(""));
                        let k = self.cx.heap.keyword(&name);
                        if let Some(keys) = &mut star.keys {
                            keys.push((k, v, init));
                        }
                    }
                    (State::Key, Some(&[v, init, k]))
                        if v.tag() == Tag::Symbol && k.tag() == Tag::Keyword =>
                    {
                        if let Some(keys) = &mut star.keys {
                            keys.push((k, v, init));
                        }
                    }
                    _ => return Err(error(&state, at)),
                }
            }
            at = next;
        }
        let mut names: Vec<Value> = star.required.clone();
        names.extend(star.optional.iter().map(|o| o.0));
        names.extend(star.rest);
        if let Some(keys) = &star.keys {
            names.extend(keys.iter().map(|k| k.1));
        }
        if names
            .iter()
            .enumerate()
            .any(|(i, n)| names[..i].contains(n))
        {
            return Err(SyntaxError {
                who: Some("lambda*"),
                message: "duplicate identifier in argument list",
                form: formals,
                subform: Value::FALSE,
                at: Some(formals),
            }
            .into());
        }
        Ok(star)
    }

    /// `(lambda* formals body …)`: a plain `lambda` when it has neither
    /// optional nor keyword arguments; else a procedure of its arguments as
    /// one list, which `let*` takes apart as Guile binds them, its errors
    /// Guile's.
    fn lambda_star(&mut self, x: Value) -> R<Value> {
        let rest = self.cdr(x);
        if rest.tag() != Tag::Pair {
            return Err(syntax(FAILED, x, Some(x)).into());
        }
        let (formals, body) = (self.car(rest), self.cdr(rest));
        let star = self.star_formals(formals)?;
        let lambda = self.kw(Kw::Lambda);
        let plain = star.optional.is_empty()
            && star
                .keys
                .as_ref()
                .is_none_or(|k| k.is_empty() && !star.allow_other_keys);
        if plain {
            let tail = star.rest.unwrap_or(Value::NULL);
            let list = star
                .required
                .iter()
                .rev()
                .fold(tail, |rest, &v| self.cx.heap.cons(v, rest));
            return Ok(self.form_star(lambda, &[list], body));
        }
        let [
            pair_p,
            car,
            cdr,
            not,
            keyword_p,
            wrong,
            eq,
            check,
            key_ref,
            no_key,
        ] = [
            "pair?",
            "car",
            "cdr",
            "not",
            "keyword?",
            "%wrong-number-of-args",
            "eq?",
            "%check-keys",
            "%key-ref",
            "%no-key",
        ]
        .map(|name| self.guile(name));
        let (if_, and, quote, let_) = (
            self.kw(Kw::If),
            self.kw(Kw::And),
            self.kw(Kw::Quote),
            self.kw(Kw::Let),
        );
        let args = self.fresh("args");
        let present = self.fresh("present");
        let mut bindings = Vec::new();
        let more = self.list(&[pair_p, args]);
        let first = self.list(&[car, args]);
        let rest_of = self.list(&[cdr, args]);
        let error = self.list(&[wrong]);
        for &r in &star.required {
            let value = self.list(&[if_, more, first, error]);
            bindings.push(self.list(&[r, value]));
            bindings.push(self.list(&[args, rest_of]));
        }
        let test = if star.keys.is_some() {
            let is_key = self.list(&[keyword_p, first]);
            let not_key = self.list(&[not, is_key]);
            self.list(&[and, more, not_key])
        } else {
            more
        };
        for &(o, init) in &star.optional {
            bindings.push(self.list(&[present, test]));
            let value = self.list(&[if_, present, first, init]);
            bindings.push(self.list(&[o, value]));
            let left = self.list(&[if_, present, rest_of, args]);
            bindings.push(self.list(&[args, left]));
        }
        let checked = self.fresh("checked");
        if star.keys.is_none() && star.rest.is_none() {
            let value = self.list(&[if_, more, error, Value::FALSE]);
            bindings.push(self.list(&[checked, value]));
        }
        if let Some(r) = star.rest {
            bindings.push(self.list(&[r, args]));
        }
        if let Some(keys) = star.keys {
            let names: Vec<Value> = keys.iter().map(|k| k.0).collect();
            let names = self.list(&names);
            let names = self.list(&[quote, names]);
            // With a rest argument, Guile skips what is not a keyword.
            let value = self.list(&[
                check,
                args,
                names,
                Value::bool(star.allow_other_keys),
                Value::bool(star.rest.is_some()),
            ]);
            bindings.push(self.list(&[checked, value]));
            let found = self.fresh("found");
            for (k, v, init) in keys {
                let lookup = self.list(&[key_ref, args, k]);
                let binding = self.list(&[found, lookup]);
                let inner = self.list(&[binding]);
                let missing = self.list(&[eq, found, no_key]);
                let choose = self.list(&[if_, missing, init, found]);
                let value = self.list(&[let_, inner, choose]);
                bindings.push(self.list(&[v, value]));
            }
        }
        let bindings = self.list(&bindings);
        let let_star = self.kw(Kw::LetStar);
        let inner = self.form_star(let_star, &[bindings], body);
        let marker = self.syntax.procedure();
        let marker = self.list(&[marker, Value::TRUE, Value::FALSE]);
        Ok(self.list(&[lambda, args, marker, inner]))
    }

    /// `(case-lambda (formals body …) …)`: the first clause whose formals
    /// take as many arguments, else Guile's error.
    fn case_lambda(&mut self, x: Value) -> R<Value> {
        let Some(items) = self.items(x) else {
            return Err(syntax(FAILED, x, Some(x)).into());
        };
        let args = self.fresh("args");
        let n = self.fresh("n");
        let [length, apply, eq, ge, wrong] =
            ["length", "apply", "=", ">=", "%wrong-number-of-args"].map(|name| self.guile(name));
        let lambda = self.kw(Kw::Lambda);
        let mut clauses = Vec::new();
        for &clause in &items[1..] {
            if self.cdr(clause).tag() != Tag::Pair {
                return Err(syntax(FAILED, x, Some(x)).into());
            }
            let formals = self.car(clause);
            let (required, tail) = self.split(formals);
            let count = self.cx.heap.integer(required.len() as i64);
            let test = match tail {
                Value::NULL => self.list(&[eq, n, count]),
                _ => self.list(&[ge, n, count]),
            };
            let procedure = self.cx.heap.cons(lambda, clause);
            let call = self.list(&[apply, procedure, args]);
            clauses.push(self.list(&[test, call]));
        }
        let error = self.list(&[wrong]);
        let else_ = self.module_ref("else", GUILE);
        let otherwise = self.list(&[else_, error]);
        clauses.push(otherwise);
        let cond = self.kw(Kw::Cond);
        let cond = self.form(cond, &clauses);
        let count = self.list(&[length, args]);
        let binding = self.list(&[n, count]);
        let bindings = self.list(&[binding]);
        let let_ = self.kw(Kw::Let);
        let body = self.list(&[let_, bindings, cond]);
        let marker = self.syntax.procedure();
        let marker = self.list(&[marker, Value::TRUE, Value::FALSE]);
        Ok(self.list(&[lambda, args, marker, body]))
    }

    /// `(do ((var init step) …) (test expr …) command …)` as Guile's named
    /// `let`.
    fn do_(&mut self, x: Value) -> R<Value> {
        let located = || -> Fail { syntax(FAILED, x, Some(x)).into() };
        let items = self.items(x).ok_or_else(located)?;
        let [_, specs, end, ref commands @ ..] = items[..] else {
            return Err(located());
        };
        let specs = self.items(specs).ok_or_else(located)?;
        let mut bindings = Vec::new();
        let mut steps = Vec::new();
        for spec in specs {
            match self.items(spec).as_deref() {
                Some(&[v, init]) if v.tag() == Tag::Symbol => {
                    bindings.push(self.list(&[v, init]));
                    steps.push(v);
                }
                Some(&[v, init, step]) if v.tag() == Tag::Symbol => {
                    bindings.push(self.list(&[v, init]));
                    steps.push(step);
                }
                _ => return Err(located()),
            }
        }
        let end = self
            .items(end)
            .filter(|e| !e.is_empty())
            .ok_or_else(located)?;
        let doloop = self.fresh("doloop");
        let again = self.form(doloop, &steps);
        let begin = self.kw(Kw::Begin);
        let mut loop_body = commands.to_vec();
        loop_body.push(again);
        let loop_body = self.form(begin, &loop_body);
        let if_ = self.kw(Kw::If);
        let test = if end.len() == 1 {
            let not = self.guile("not");
            let test = self.list(&[not, end[0]]);
            self.list(&[if_, test, loop_body])
        } else {
            let result = self.form(begin, &end[1..]);
            self.list(&[if_, end[0], result, loop_body])
        };
        let bindings = self.list(&bindings);
        let let_ = self.kw(Kw::Let);
        Ok(self.list(&[let_, doloop, bindings, test]))
    }

    /// `(while condition body …)` as `(%while (lambda () condition) (lambda
    /// (break continue) body …))`.
    fn while_(&mut self, x: Value) -> R<Value> {
        let items = self.items(x).unwrap_or_default();
        let [_, condition, ref body @ ..] = items[..] else {
            return Err(syntax(FAILED, x, Some(x)).into());
        };
        let lambda = self.kw(Kw::Lambda);
        let test = self.list(&[lambda, Value::NULL, condition]);
        let names = [
            self.cx.heap.intern("break"),
            self.cx.heap.intern("continue"),
        ];
        let names = self.list(&names);
        let body = if body.is_empty() {
            &[Value::UNSPECIFIED][..]
        } else {
            body
        };
        let body = self.form(lambda, &[&[names][..], body].concat());
        let run = self.guile("%while");
        Ok(self.list(&[run, test, body]))
    }

    // --- quasiquote

    /// `x` is `(keyword e)` for the free identifier `keyword`.
    fn is_form(&self, x: Value, keyword: Kw, env: &Env) -> bool {
        let symbol = self.expander.sym(keyword);
        matches!(self.items(x).as_deref(), Some(&[h, _]) if self.is_free(h, symbol, env))
    }

    fn quasi(&mut self, p: Value, level: usize, env: &Env) -> Q {
        if self.is_form(p, Kw::Unquote, env) {
            if level == 0 {
                return Q::Value(self.car(self.cdr(p)));
            }
            let inner = self.quasi(self.cdr(p), level - 1, env);
            let unquote = self.expander.sym(Kw::Unquote);
            return self.quasicons(Q::Quote(unquote), inner);
        }
        if self.is_form(p, Kw::Quasiquote, env) {
            let inner = self.quasi(self.cdr(p), level + 1, env);
            let quasiquote = self.expander.sym(Kw::Quasiquote);
            return self.quasicons(Q::Quote(quasiquote), inner);
        }
        match p.tag() {
            Tag::Pair => {
                let mut items = Vec::new();
                let mut at = p;
                while at.tag() == Tag::Pair
                    && (at == p
                        || !(self.is_form(at, Kw::Unquote, env)
                            || self.is_form(at, Kw::Quasiquote, env)))
                {
                    items.push(self.car(at));
                    at = self.cdr(at);
                }
                let mut acc = self.quasi(at, level, env);
                for &item in items.iter().rev() {
                    acc = self.quasi_item(item, acc, level, env);
                }
                acc
            }
            Tag::Vector => {
                let items = self.cx.heap.vector_items(p).unwrap_or_default().to_vec();
                let mut acc = Q::Quote(Value::NULL);
                for &item in items.iter().rev() {
                    acc = self.quasi_item(item, acc, level, env);
                }
                self.quasivector(acc)
            }
            _ => Q::Quote(p),
        }
    }

    /// One item of a list or vector before the quasiquotation `acc` of the
    /// rest.
    fn quasi_item(&mut self, h: Value, acc: Q, level: usize, env: &Env) -> Q {
        let head = self.car(h);
        for (kw, splicing) in [(Kw::Unquote, false), (Kw::UnquoteSplicing, true)] {
            let symbol = self.expander.sym(kw);
            if h.tag() != Tag::Pair || !self.is_free(head, symbol, env) {
                continue;
            }
            let Some(es) = self.items(self.cdr(h)) else {
                continue;
            };
            if level == 0 {
                let values: Vec<Q> = es.into_iter().map(Q::Value).collect();
                return if splicing {
                    self.quasiappend(values, acc)
                } else {
                    values
                        .into_iter()
                        .rev()
                        .fold(acc, |rest, v| self.quasicons(v, rest))
                };
            }
            let inner = self.quasi(self.cdr(h), level - 1, env);
            let head = self.quasicons(Q::Quote(symbol), inner);
            return self.quasicons(head, acc);
        }
        let first = self.quasi(h, level, env);
        self.quasicons(first, acc)
    }

    fn quasicons(&mut self, x: Q, y: Q) -> Q {
        match y {
            Q::Quote(dy) => match x {
                Q::Quote(dx) => Q::Quote(self.cx.heap.cons(dx, dy)),
                x if dy == Value::NULL => Q::List(vec![x]),
                x => Q::ListStar(vec![x], Box::new(Q::Quote(dy))),
            },
            Q::List(mut items) => {
                items.insert(0, x);
                Q::List(items)
            }
            Q::ListStar(mut items, last) => {
                items.insert(0, x);
                Q::ListStar(items, last)
            }
            y => Q::ListStar(vec![x], Box::new(y)),
        }
    }

    fn quasiappend(&mut self, mut xs: Vec<Q>, y: Q) -> Q {
        match y {
            Q::Quote(Value::NULL) => match xs.len() {
                0 => Q::Quote(Value::NULL),
                1 => xs.pop().unwrap_or(Q::Quote(Value::NULL)),
                _ => Q::Append(xs),
            },
            y if xs.is_empty() => y,
            y => {
                xs.push(y);
                Q::Append(xs)
            }
        }
    }

    fn quasivector(&mut self, x: Q) -> Q {
        if let Q::Quote(list) = x
            && let Some(items) = self.items(list)
        {
            return Q::Quote(self.cx.heap.vector(items));
        }
        let mut prefix = Vec::new();
        let mut y = x.clone();
        loop {
            match y {
                Q::Quote(list) => match self.items(list) {
                    Some(items) => {
                        prefix.extend(items.into_iter().map(Q::Quote));
                        return Q::Vector(prefix);
                    }
                    None => return Q::ListToVector(Box::new(x)),
                },
                Q::List(items) => {
                    prefix.extend(items);
                    return Q::Vector(prefix);
                }
                Q::ListStar(items, last) => {
                    prefix.extend(items);
                    y = *last;
                }
                _ => return Q::ListToVector(Box::new(x)),
            }
        }
    }

    fn emit(&mut self, q: Q) -> Value {
        let (name, items) = match q {
            Q::Quote(v) => {
                let quote = self.kw(Kw::Quote);
                return self.list(&[quote, v]);
            }
            Q::Value(v) => return v,
            Q::List(items) => ("list", items),
            Q::ListStar(mut items, last) => {
                items.push(*last);
                ("cons*", items)
            }
            Q::Append(items) => ("append", items),
            Q::Vector(items) => ("vector", items),
            Q::ListToVector(x) => ("list->vector", vec![*x]),
        };
        let mut code = vec![self.guile(name)];
        for item in items {
            code.push(self.emit(item));
        }
        self.list(&code)
    }
}

/// The procedures the native forms' expansions call, in `(guile)`.
pub(crate) const PRELUDE: &str = r#"
(define (%wrong-number-of-args)
  (scm-error 'wrong-number-of-args "eval" "Wrong number of arguments" '() #f))
(define %no-key (list 'no-key))
(define (%key-ref args key)
  (let loop ((args args) (found %no-key))
    (cond ((not (pair? args)) found)
          ((not (keyword? (car args))) (loop (cdr args) found))
          ((pair? (cdr args))
           (loop (cddr args) (if (eq? (car args) key) (cadr args) found)))
          (else found))))
(define (%check-keys args keys allow rest)
  (let loop ((at args))
    (if (pair? at)
        (let ((k (car at)))
          (cond ((not (keyword? k))
                 (if rest
                     (loop (cdr at))
                     (scm-error 'keyword-argument-error "eval" "Invalid keyword" '() (list k))))
                ((memq k keys)
                 (if (pair? (cdr at))
                     (loop (cddr at))
                     (scm-error 'keyword-argument-error "eval" "Keyword argument has no value"
                                '() (list k))))
                (allow (if (pair? (cdr at)) (loop (cddr at))))
                (else
                 (scm-error 'keyword-argument-error "eval" "Unrecognized keyword" '() (list k))))))))
(define (%while test body)
  (let ((break-tag (list 'break)))
    (%dyn prompt break-tag #f
      (let ((break (lambda args (%abort break-tag args))))
        (let next ()
          (let ((continue-tag (list 'continue)))
            (if (%dyn prompt continue-tag #f
                  (let ((continue (lambda () (%abort continue-tag '()))))
                    (let loop ()
                      (if (test) (begin (body break continue) (loop)) #f)))
                  (lambda () #t))
                (next)
                #f))))
      (lambda args (if (null? args) #t (apply values args))))))
"#;
