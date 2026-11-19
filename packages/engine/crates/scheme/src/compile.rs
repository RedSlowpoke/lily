//! The compiler (chapter 03 §5.3, R2): a datum to bytecode for the VM, in
//! two passes.
//!
//! The first recognises the special forms, resolves every variable to a
//! local of the procedure that binds it or to a global slot, and notes which
//! locals inner procedures capture and which are assigned. The second emits
//! [`Op`]s. Locals live in the procedure's frame on the VM's value stack;
//! a procedure captures the values of its free variables when it is made
//! (flat closures), so a local that is captured and also assigned, or
//! captured by a `letrec` procedure before it has its value, lives in a box
//! (a pair) that the frame and the closures share. A procedure bound by
//! `letrec` or a named `let` reaches itself as the running closure.
//!
//! Special forms: `quote`, `lambda`, `define`, `set!`, `if`, `begin`, `let`
//! (and named `let`), `let*`, `letrec`, `letrec*`, `cond` (with `=>`),
//! `case`, `and`, `or`, `when`, `unless`, `parameterize` and `with-fluids`,
//! with definitions inside bodies as `letrec*`; and Guile's module forms
//! (`module.rs`): `define-module`, `use-modules`, `export`, `re-export`,
//! `define-public`, `@` and `@@`. The VM's own Scheme also has
//! `%call-values` and `%dyn`, the dynamic extent of `catch`, escapes,
//! `dynamic-wind` and fluids (`control.rs`). Macros and Guile's other forms
//! are expanded into these before the compiler sees them (`expand.rs`).
//!
//! A toplevel variable is the slot of its symbol in the module being
//! compiled, a cell made for it if the module has none; the VM resolves a
//! cell that nothing defines to an import the first time it runs, and
//! points the instruction at it (`module.rs`), so a template's instructions
//! are cells. An identifier a macro introduced is an alias, an uninterned
//! symbol: bound, it is a local like any other; free, it is its name's
//! variable in the macro's module.
//!
//! A form the compiler refuses is a [`SyntaxError`] with psyntax's fields:
//! who, what, the form or subform, and whether Guile gives its place.
//! A call of `car`, `cdr`, `vector-length`, `vector-ref` or `vector-set!`
//! by its name inside a lexical environment is marked, because Guile words
//! its errors as compiled code's, not as the procedure's own (`errors.rs`).

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::Cell;

use crate::control::DynKind;
use crate::heap::{Gray, Heap};
use crate::module::{GUILE, Globals};
use crate::prims::{self, Inline};
use crate::value::{Tag, Value};

/// One instruction. Operands index the frame's locals, the closure's free
/// values, the template's constants or children, the global slots, or the
/// template's instructions (jumps). The VM rewrites the slot of `Global`,
/// `SetGlobal` and `Inline` once, when a cell resolves to an import.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Op {
    Const(u32),
    Fixnum(i32),
    Null,
    False,
    True,
    Unspecified,
    Local(u32),
    LocalBox(u32),
    Free(u32),
    FreeBox(u32),
    /// The running closure, a `letrec` procedure's reference to itself.
    Myself,
    Global(u32),
    SetLocal(u32),
    SetLocalBox(u32),
    SetFreeBox(u32),
    SetGlobal(u32),
    Define(u32),
    /// Puts a local's value into a new box.
    Box(u32),
    /// A new box with no value yet, for a `letrec` variable.
    NewBox(u32),
    Pop,
    Jump(u32),
    /// Pops the value and jumps if it is `#f`.
    JumpFalse(u32),
    /// Jumps if the value is `#f` (`#t`), keeping it; pops it otherwise.
    AndJump(u32),
    OrJump(u32),
    /// Pops a procedure's free values and pushes the closure of a child.
    Closure(u32),
    /// Calls with that many arguments; the result must be one value.
    Call(u32),
    /// The same, keeping multiple values (for an effect or `%call-values`).
    CallMulti(u32),
    TailCall(u32),
    /// Pops values and calls the procedure below them with them as its
    /// arguments, `call-with-values`.
    CallValues,
    TailCallValues,
    Return,
    /// A primitive inline while its global still holds it; the global's
    /// slot, and whether the call is a direct one (`Op::Direct`).
    Inline(Inline, u32, bool),
    /// `case`: whether the value is `eqv?` to one of the constant's list.
    Memv(u32),
    /// The next call calls a primitive by its name inside a lexical
    /// environment, where Guile words some errors as compiled code's.
    Direct,
    /// Pops the data and the extra value and pushes an entry of the
    /// dynamic stack whose landing is at the instruction.
    PushDyn(DynKind, u32),
    PopDyn,
    /// Pushes what a throw left for the landing.
    Pending,
}

/// A compiled procedure: what every closure of one `lambda` shares.
pub(crate) struct Template {
    /// The procedure's name, a symbol, or `#f`.
    pub name: Value,
    pub required: u32,
    pub rest: bool,
    /// The frame's slots: arguments, the rest list, then locals.
    pub frame: u32,
    /// How many free values each closure holds.
    pub free: u32,
    pub ops: Box<[Cell<Op>]>,
    pub consts: Vec<Value>,
    pub children: Vec<Rc<Template>>,
    /// A `lambda*` or `case-lambda` procedure, which takes its arguments as
    /// one list and prints as Guile's do.
    pub star: bool,
    /// What the expander recorded about the procedure: a music function's
    /// signature, the datum `(type predicate …)` (03 §5.5), or `#f`.
    pub header: Value,
    /// Its formals as written, `(a b . rest)`, for a named procedure of
    /// LilyPond's own Scheme, which prints them as Guile's compiled code
    /// does; `#f` for the rest, which print as Guile's evaluator names them
    /// (OPEN C6, [`crate::module::Globals::named`]).
    pub names: Value,
    /// The collection that last traced the constants.
    pub mark: Cell<u32>,
    /// Its digest once [`crate::Heap::closure_parts`] made it, or 0.
    pub digest: Cell<u64>,
}

impl Template {
    /// The code that runs before the first call.
    pub(crate) fn empty() -> Rc<Template> {
        Rc::new(Template {
            name: Value::FALSE,
            required: 0,
            rest: false,
            frame: 0,
            free: 0,
            ops: Box::new([]),
            consts: Vec::new(),
            children: Vec::new(),
            star: false,
            header: Value::FALSE,
            names: Value::FALSE,
            mark: Cell::new(0),
            digest: Cell::new(0),
        })
    }

    /// Hands the constants of this template and of its children, and the
    /// global slots their code names, to the collector once per collection
    /// `mark`.
    pub(crate) fn trace(&self, gray: &mut Gray, slots: &mut Vec<u32>, mark: u32) {
        if self.mark.replace(mark) == mark {
            return;
        }
        gray.extend(self.consts.iter().copied().filter(|v| v.is_heap()));
        if self.header.is_heap() {
            gray.push(self.header);
        }
        if self.names.is_heap() {
            gray.push(self.names);
        }
        // The global slots it names keep their modules (`Vm::collect`).
        slots.extend(self.ops.iter().filter_map(|op| match op.get() {
            Op::Global(slot) | Op::SetGlobal(slot) | Op::Define(slot) | Op::Inline(_, slot, _) => {
                Some(slot)
            }
            _ => None,
        }));
        for child in &self.children {
            child.trace(gray, slots, mark);
        }
    }
}

/// The symbols of the special forms, interned once.
pub(crate) struct Syntax {
    quote: Value,
    lambda: Value,
    define: Value,
    set: Value,
    if_: Value,
    begin: Value,
    let_: Value,
    let_star: Value,
    letrec: Value,
    letrec_star: Value,
    cond: Value,
    case: Value,
    and: Value,
    or: Value,
    when: Value,
    unless: Value,
    parameterize: Value,
    with_fluids: Value,
    define_public: Value,
    define_module: Value,
    use_modules: Value,
    export: Value,
    re_export: Value,
    at: Value,
    at_at: Value,
    else_: Value,
    arrow: Value,
    call_values: Value,
    dyn_: Value,
    /// `(%procedure star header)`, which the expander puts first in a
    /// `lambda`'s body for [`Template::star`] and [`Template::header`]; an
    /// uninterned symbol, which no source can name.
    procedure: Value,
}

impl Syntax {
    /// The symbols, interned, and `procedure`, the uninterned symbol of
    /// [`Syntax::procedure`]: a new VM's, or the one its image kept.
    pub(crate) fn new(heap: &mut Heap, procedure: Value) -> Syntax {
        Syntax {
            quote: heap.intern("quote"),
            lambda: heap.intern("lambda"),
            define: heap.intern("define"),
            set: heap.intern("set!"),
            if_: heap.intern("if"),
            begin: heap.intern("begin"),
            let_: heap.intern("let"),
            let_star: heap.intern("let*"),
            letrec: heap.intern("letrec"),
            letrec_star: heap.intern("letrec*"),
            cond: heap.intern("cond"),
            case: heap.intern("case"),
            and: heap.intern("and"),
            or: heap.intern("or"),
            when: heap.intern("when"),
            unless: heap.intern("unless"),
            parameterize: heap.intern("parameterize"),
            with_fluids: heap.intern("with-fluids"),
            define_public: heap.intern("define-public"),
            define_module: heap.intern("define-module"),
            use_modules: heap.intern("use-modules"),
            export: heap.intern("export"),
            re_export: heap.intern("re-export"),
            at: heap.intern("@"),
            at_at: heap.intern("@@"),
            else_: heap.intern("else"),
            arrow: heap.intern("=>"),
            call_values: heap.intern("%call-values"),
            dyn_: heap.intern("%dyn"),
            procedure,
        }
    }

    pub(crate) fn procedure(&self) -> Value {
        self.procedure
    }
}

/// A form the compiler refuses, which the VM throws as a `syntax-error`
/// with psyntax's arguments `(who what where form subform)`.
#[derive(Debug)]
pub(crate) struct SyntaxError {
    /// The keyword whose syntax it is, or none.
    pub who: Option<&'static str>,
    pub message: &'static str,
    pub form: Value,
    pub subform: Value,
    /// What Guile gives the place of: the subform, or the form, when it was
    /// read from source. None where psyntax gives none.
    pub at: Option<Value>,
}

/// Compiles a toplevel form of `module` into a procedure of no arguments.
/// `internal` lets the VM's own Scheme use `(%call-values consumer expr)`.
pub(crate) fn compile(
    heap: &mut Heap,
    globals: &mut Globals,
    syntax: &Syntax,
    form: Value,
    module: u32,
    internal: bool,
) -> core::result::Result<Rc<Template>, SyntaxError> {
    let mut c = Compiler {
        heap,
        globals,
        syn: syntax,
        module,
        internal,
        vars: Vec::new(),
        lambdas: vec![Lambda::new(None, Value::FALSE)],
        scope: Vec::new(),
        current: 0,
        depth: 0,
        lexical: 0,
    };
    let body = c.toplevel(form)?;
    c.lambdas[0].body = Some(body);
    Ok(Gen::template(&c, 0))
}

// --- the first pass ------------------------------------------------------------

type Var = u32;

/// How deeply expressions may nest. Both passes recurse once per level on
/// the native stack, wasm's 1 MiB, and a release build overflows 1 MiB at
/// 700–990 levels of `let`; the reader allows 1,000 levels of data. Guile
/// has no limit.
pub const MAX_NESTING: usize = 500;

/// The most arguments of `+`, `-` or `*` folded into inline ops.
const MAX_FOLD: usize = 8;

struct Variable {
    owner: u32,
    slot: u32,
    /// An inner procedure refers to it.
    captured: bool,
    assigned: bool,
    /// Bound by `letrec`, `letrec*`, a named `let` or a body's `define`.
    rec: bool,
    /// Has its value: false for a `rec` variable until its init is done.
    ready: bool,
    /// A procedure made before it had its value captured it.
    early: bool,
    /// The procedure that is its init, which reaches it as itself.
    self_of: Option<u32>,
}

struct Lambda {
    parent: Option<u32>,
    name: Value,
    required: u32,
    rest: bool,
    star: bool,
    header: Value,
    names: Value,
    params: Vec<Var>,
    /// The variables of enclosing procedures it refers to, its own
    /// children's included.
    free: Vec<Var>,
    body: Option<Node>,
    /// The next free slot, and the most slots used.
    next: u32,
    slots: u32,
}

impl Lambda {
    fn new(parent: Option<u32>, name: Value) -> Lambda {
        Lambda {
            parent,
            name,
            required: 0,
            rest: false,
            star: false,
            header: Value::FALSE,
            names: Value::FALSE,
            params: Vec::new(),
            free: Vec::new(),
            body: None,
            next: 0,
            slots: 0,
        }
    }
}

enum Node {
    Const(Value),
    Ref(Var),
    Global(u32),
    Set(Var, Box<Node>),
    /// Gives a variable its first value.
    Init(Var, Box<Node>),
    SetGlobal(u32, Box<Node>),
    Define(u32, Box<Node>),
    If(Box<[Node; 3]>),
    Lambda(u32),
    Seq(Vec<Node>),
    /// The procedure, then the arguments, and whether the call is a direct
    /// one (`Op::Direct`).
    Call(Vec<Node>, bool),
    Inline(Inline, u32, Vec<Node>, bool),
    /// Variables that the body binds; their boxes are made on entry.
    Scope(Vec<Var>, Box<Node>),
    And(Vec<Node>),
    Or(Vec<Node>),
    Memv(Box<Node>, Value),
    CallValues(Box<[Node; 2]>),
    /// `cond` and `case`: the first clause whose test is true, else the
    /// last node.
    Choose(Vec<Clause>, Box<Node>),
    /// `(%dyn kind data extra body handler)`.
    Dyn(DynKind, Box<[Node; 4]>),
}

struct Clause {
    test: Node,
    then: Then,
}

enum Then {
    Body(Node),
    /// `(test)`: the test's value.
    Test,
    /// `(test => receiver)`: the test is the variable's init, and the call
    /// gives it to the receiver.
    Arrow(Var, Node),
}

struct Compiler<'a> {
    heap: &'a mut Heap,
    globals: &'a mut Globals,
    syn: &'a Syntax,
    /// The module whose toplevel this is.
    module: u32,
    internal: bool,
    vars: Vec<Variable>,
    lambdas: Vec<Lambda>,
    /// The visible locals, the innermost last.
    scope: Vec<(Value, Var)>,
    current: u32,
    /// How many expressions the current one is inside.
    depth: usize,
    /// How many of Guile's lexical environments the current expression is
    /// inside, in the outermost procedure: a `let` with bindings, a
    /// `letrec`, a `case`'s clauses, a `define`'s value. None is Guile's
    /// toplevel environment.
    lexical: u32,
}

type Result<T> = core::result::Result<T, SyntaxError>;

impl Compiler<'_> {
    // --- data

    fn car(&self, x: Value) -> Value {
        self.heap.car(x).unwrap_or(Value::UNSPECIFIED)
    }

    fn cdr(&self, x: Value) -> Value {
        self.heap.cdr(x).unwrap_or(Value::UNSPECIFIED)
    }

    /// The items of a proper list, or a syntax error about `form`.
    fn items(&self, list: Value, form: Value) -> Result<Vec<Value>> {
        let mut items = Vec::new();
        let mut at = list;
        while at.tag() == Tag::Pair {
            items.push(self.car(at));
            at = self.cdr(at);
        }
        if at != Value::NULL {
            return Err(self.bad(form));
        }
        Ok(items)
    }

    /// Psyntax's error for a form that matches none of its patterns: the
    /// core forms' own messages, or `source expression failed to match any
    /// pattern`, with the place where Guile gives one.
    #[cold]
    fn bad(&self, form: Value) -> SyntaxError {
        let head = self.base(self.car(form));
        let s = self.syn;
        let (who, message, located) = match head {
            h if h == s.quote => (Some("quote"), "bad syntax", true),
            h if h == s.set => (Some("set!"), "bad set!", true),
            h if h == s.let_ => (Some("let"), "bad let", true),
            h if h == s.letrec => (Some("letrec"), "bad letrec", true),
            h if h == s.letrec_star => (Some("letrec*"), "bad letrec*", true),
            h if h == s.lambda => (Some("lambda"), "bad lambda", false),
            // The forms that are macros in Guile, whose failures it places.
            h if [s.let_star, s.cond, s.case, s.when, s.unless, s.and, s.or].contains(&h)
                || h == s.parameterize
                || h == s.with_fluids =>
            {
                (None, FAILED, true)
            }
            _ => (None, FAILED, false),
        };
        SyntaxError {
            who,
            message,
            form,
            subform: Value::FALSE,
            at: located.then_some(form),
        }
    }

    /// An error about `subform` of `form`, placed at the subform.
    #[cold]
    fn bad_part(
        &self,
        who: &'static str,
        message: &'static str,
        form: Value,
        subform: Value,
    ) -> SyntaxError {
        SyntaxError {
            who: Some(who),
            message,
            form,
            subform,
            at: Some(subform),
        }
    }

    /// Whether a call here is in Guile's toplevel environment.
    fn top(&self) -> bool {
        self.current == 0 && self.lexical == 0
    }

    /// Compiles `f` one lexical environment deeper.
    fn inner<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.lexical += 1;
        let r = f(self);
        self.lexical -= 1;
        r
    }

    // --- scopes

    fn lookup(&self, name: Value) -> Option<Var> {
        self.scope
            .iter()
            .rev()
            .find(|(n, _)| *n == name)
            .map(|&(_, v)| v)
    }

    /// Whether `head` names the special form `keyword` here.
    fn is(&self, head: Value, keyword: Value) -> bool {
        self.lookup(head).is_none() && self.base(head) == keyword
    }

    /// The symbol an alias stands for, through aliases of aliases: what a
    /// keyword a macro introduced names.
    fn base(&self, mut x: Value) -> Value {
        while x.tag() == Tag::Symbol
            && let Some(alias) = self.globals.aliases.get(&x.payload())
        {
            x = alias.name;
        }
        x
    }

    /// A new variable of the current procedure in a new slot; it is not
    /// visible until [`Compiler::bind`].
    fn new_var(&mut self, rec: bool) -> Var {
        let lambda = &mut self.lambdas[self.current as usize];
        let slot = lambda.next;
        lambda.next += 1;
        lambda.slots = lambda.slots.max(lambda.next);
        self.vars.push(Variable {
            owner: self.current,
            slot,
            captured: false,
            assigned: false,
            rec,
            ready: !rec,
            early: false,
            self_of: None,
        });
        self.vars.len() as Var - 1
    }

    fn bind(&mut self, name: Value, var: Var) {
        self.scope.push((name, var));
    }

    /// Ends the scopes opened since `mark`, freeing their slots.
    fn close(&mut self, mark: (usize, u32)) {
        self.scope.truncate(mark.0);
        self.lambdas[self.current as usize].next = mark.1;
    }

    fn mark(&self) -> (usize, u32) {
        (self.scope.len(), self.lambdas[self.current as usize].next)
    }

    fn reference(&mut self, name: Value) -> Node {
        match self.lookup(name) {
            Some(var) => {
                self.capture(var);
                Node::Ref(var)
            }
            None => Node::Global(self.globals.reference(self.module, name)),
        }
    }

    /// Records that the current procedure refers to `var`: every procedure
    /// between it and the variable's owner has it free.
    fn capture(&mut self, var: Var) {
        let owner = self.vars[var as usize].owner;
        if owner == self.current {
            return;
        }
        let mut at = self.current;
        let mut outermost = at;
        while at != owner {
            let lambda = &mut self.lambdas[at as usize];
            if !lambda.free.contains(&var) {
                lambda.free.push(var);
            }
            outermost = at;
            at = lambda.parent.unwrap_or(owner);
        }
        let v = &mut self.vars[var as usize];
        v.captured = true;
        if !v.ready && v.self_of != Some(outermost) {
            v.early = true;
        }
    }

    // --- forms

    fn toplevel(&mut self, x: Value) -> Result<Node> {
        if x.tag() == Tag::Pair {
            let head = self.car(x);
            if self.is(head, self.syn.define) {
                let (name, value) = self.inner(|c| c.definition(x))?;
                let slot = self.globals.cell(self.module, name);
                return Ok(Node::Define(slot, Box::new(value)));
            }
            if self.is(head, self.syn.define_public) {
                // `(begin (define …) (export name))`.
                let (name, value) = self.inner(|c| c.definition(x))?;
                let slot = self.globals.cell(self.module, name);
                let names = self.heap.list(&[name]);
                let module = Node::Const(self.globals.object(self.module));
                let export = Node::Call(
                    vec![self.guile("%export"), Node::Const(names), module],
                    false,
                );
                return Ok(Node::Seq(vec![Node::Define(slot, Box::new(value)), export]));
            }
            if self.is(head, self.syn.begin) {
                let forms = self.items(self.cdr(x), x)?;
                if forms.is_empty() {
                    return Ok(Node::Const(Value::UNSPECIFIED));
                }
                let nodes = forms
                    .into_iter()
                    .map(|f| self.toplevel(f))
                    .collect::<Result<Vec<_>>>()?;
                return Ok(Node::Seq(nodes));
            }
        }
        self.expr(x, Value::FALSE)
    }

    /// `(define name value)` or `(define (name . formals) body …)`: the name
    /// and the value's node, the procedure named after it.
    fn definition(&mut self, x: Value) -> Result<(Value, Node)> {
        let (target, rest) = self.target(x)?;
        let value = self.definition_value(x, target, rest, None)?;
        Ok((self.definition_name(target), value))
    }

    /// What a definition defines, and the rest of its form.
    fn target(&self, x: Value) -> Result<(Value, Value)> {
        let rest = self.cdr(x);
        if rest.tag() != Tag::Pair {
            return Err(self.bad(x));
        }
        let target = self.car(rest);
        let name = self.definition_name(target);
        if name.tag() != Tag::Symbol {
            return Err(self.bad(x));
        }
        Ok((target, self.cdr(rest)))
    }

    fn definition_name(&self, target: Value) -> Value {
        if target.tag() == Tag::Pair {
            self.car(target)
        } else {
            target
        }
    }

    fn definition_value(
        &mut self,
        x: Value,
        target: Value,
        rest: Value,
        var: Option<Var>,
    ) -> Result<Node> {
        if target.tag() == Tag::Pair {
            return self.lambda(self.car(target), self.cdr(target), rest, x, var);
        }
        match self.items(rest, x)?.as_slice() {
            [] => Ok(Node::Const(Value::UNSPECIFIED)),
            [value] => self.named(*value, target, var),
            _ => Err(self.bad(x)),
        }
    }

    /// An expression whose value is bound to `name`: a `lambda` takes the
    /// name, and reaches `var` as itself.
    fn named(&mut self, x: Value, name: Value, var: Option<Var>) -> Result<Node> {
        if x.tag() == Tag::Pair && self.is(self.car(x), self.syn.lambda) {
            let rest = self.cdr(x);
            if rest.tag() != Tag::Pair {
                return Err(self.bad(x));
            }
            return self.lambda(name, self.car(rest), self.cdr(rest), x, var);
        }
        self.expr(x, name)
    }

    fn expr(&mut self, x: Value, name: Value) -> Result<Node> {
        match x.tag() {
            Tag::Symbol => Ok(self.reference(x)),
            Tag::Pair => {
                if self.depth == MAX_NESTING {
                    return Err(syntax("expression nested too deeply", x, None));
                }
                self.depth += 1;
                let node = self.combination(x, name);
                self.depth -= 1;
                node
            }
            Tag::Null => Err(syntax("unexpected syntax", x, None)),
            _ => Ok(Node::Const(x)),
        }
    }

    fn exprs(&mut self, xs: &[Value]) -> Result<Vec<Node>> {
        xs.iter().map(|&x| self.expr(x, Value::FALSE)).collect()
    }

    /// A sequence of expressions, `(begin)`'s unspecified value when empty.
    fn seq(&mut self, xs: &[Value]) -> Result<Node> {
        let mut nodes = self.exprs(xs)?;
        Ok(match nodes.len() {
            0 => Node::Const(Value::UNSPECIFIED),
            1 => nodes.pop().unwrap_or(Node::Const(Value::UNSPECIFIED)),
            _ => Node::Seq(nodes),
        })
    }

    fn combination(&mut self, x: Value, name: Value) -> Result<Node> {
        let head = self.car(x);
        let rest = self.cdr(x);
        if head.tag() == Tag::Symbol && self.lookup(head).is_none() {
            let s = self.syn;
            match self.base(head) {
                h if h == s.quote => {
                    return match self.items(rest, x)?.as_slice() {
                        [datum] => Ok(Node::Const(*datum)),
                        _ => Err(self.bad(x)),
                    };
                }
                h if h == s.lambda => {
                    if rest.tag() != Tag::Pair {
                        return Err(self.bad(x));
                    }
                    return self.lambda(name, self.car(rest), self.cdr(rest), x, None);
                }
                h if h == s.define || h == s.define_public => {
                    return Err(syntax(
                        "definition in expression context, where definitions are not allowed,",
                        x,
                        Some(x),
                    ));
                }
                h if h == s.set => return self.set(x),
                h if h == s.if_ => {
                    let items = self.items(rest, x)?;
                    let (test, then, otherwise) = match items.as_slice() {
                        [t, a] => (*t, *a, None),
                        [t, a, b] => (*t, *a, Some(*b)),
                        _ => return Err(self.bad(x)),
                    };
                    let test = self.expr(test, Value::FALSE)?;
                    let then = self.expr(then, Value::FALSE)?;
                    let otherwise = match otherwise {
                        Some(b) => self.expr(b, Value::FALSE)?,
                        None => Node::Const(Value::UNSPECIFIED),
                    };
                    return Ok(Node::If(Box::new([test, then, otherwise])));
                }
                h if h == s.begin => {
                    let items = self.items(rest, x)?;
                    return self.seq(&items);
                }
                h if h == s.let_ => return self.let_(x),
                h if h == s.let_star => return self.let_star(x),
                h if h == s.letrec || h == s.letrec_star => return self.letrec(x),
                h if h == s.cond => return self.cond(x),
                h if h == s.case => return self.case(x),
                h if h == s.and || h == s.or => {
                    let items = self.items(rest, x)?;
                    let mut nodes = self.exprs(&items)?;
                    let and = h == s.and;
                    return Ok(match nodes.len() {
                        0 => Node::Const(Value::bool(and)),
                        1 => nodes.pop().unwrap_or(Node::Const(Value::TRUE)),
                        _ if and => Node::And(nodes),
                        _ => Node::Or(nodes),
                    });
                }
                h if h == s.when || h == s.unless => {
                    let items = self.items(rest, x)?;
                    let Some((&test, body)) = items.split_first() else {
                        return Err(self.bad(x));
                    };
                    let test = self.expr(test, Value::FALSE)?;
                    let body = self.seq(body)?;
                    let none = Node::Const(Value::UNSPECIFIED);
                    let arms = if h == s.when {
                        [test, body, none]
                    } else {
                        [test, none, body]
                    };
                    return Ok(Node::If(Box::new(arms)));
                }
                h if h == s.parameterize || h == s.with_fluids => {
                    return self.parameterize(x, h == s.parameterize);
                }
                h if h == s.define_module => return self.define_module(x),
                h if h == s.use_modules => return self.use_modules(x),
                h if h == s.export || h == s.re_export => {
                    let names = self.items(rest, x)?;
                    let named = |n: &Value| {
                        n.tag() == Tag::Symbol
                            || (self.heap.car(*n).is_some_and(|a| a.tag() == Tag::Symbol)
                                && self.heap.cdr(*n).is_some_and(|d| d.tag() == Tag::Symbol))
                    };
                    if !names.iter().all(named) {
                        return Err(self.bad(x));
                    }
                    let which = if h == s.export {
                        "%export"
                    } else {
                        "%re-export"
                    };
                    return Ok(Node::Call(
                        vec![self.guile(which), Node::Const(rest)],
                        false,
                    ));
                }
                h if h == s.at || h == s.at_at => return self.at(x, h == s.at),
                h if h == s.dyn_ && self.internal => return self.dyn_(x),
                h if h == s.call_values && self.internal => {
                    let items = self.items(rest, x)?;
                    let [consumer, producer] = items.as_slice() else {
                        return Err(self.bad(x));
                    };
                    let consumer = self.expr(*consumer, Value::FALSE)?;
                    let producer = self.expr(*producer, Value::FALSE)?;
                    return Ok(Node::CallValues(Box::new([consumer, producer])));
                }
                _ => {}
            }
        }
        let items = self.items(x, x)?;
        let mut nodes = self.exprs(&items)?;
        let name = match nodes[0] {
            Node::Global(_) => self.heap.name(head),
            _ => None,
        };
        let direct = !self.top() && name.is_some_and(|n| prims::direct_call(n, nodes.len() - 1));
        if let Node::Global(slot) = nodes[0]
            && let Some(op) = name.and_then(|n| prims::inline(n, nodes.len() - 1))
            && nodes.len() <= MAX_FOLD + 1
        {
            nodes.remove(0);
            return Ok(fold_inline(op, slot, nodes, direct));
        }
        Ok(Node::Call(nodes, direct))
    }

    /// `(parameterize ((p v) …) body …)` as `(%parameterize (list p …)
    /// (list v …) (lambda () body …))`, and `with-fluids` the same with
    /// `with-fluids*`.
    fn parameterize(&mut self, x: Value, parameters: bool) -> Result<Node> {
        let rest = self.cdr(x);
        if rest.tag() != Tag::Pair {
            return Err(self.bad(x));
        }
        let mut targets = vec![Node::Const(prims::primitive("list"))];
        let mut values = vec![Node::Const(prims::primitive("list"))];
        for b in self.items(self.car(rest), x)? {
            let [target, value] = self.items(b, x)?[..] else {
                return Err(self.bad(x));
            };
            targets.push(self.expr(target, Value::FALSE)?);
            values.push(self.expr(value, Value::FALSE)?);
        }
        let thunk = self.lambda(Value::FALSE, Value::NULL, self.cdr(rest), x, None)?;
        let name = if parameters {
            "%parameterize"
        } else {
            "with-fluids*"
        };
        let call = vec![
            self.guile(name),
            Node::Call(targets, false),
            Node::Call(values, false),
            thunk,
        ];
        Ok(Node::Call(call, false))
    }

    /// The `(guile)` procedure `name`, which the module forms call: its
    /// slot there, whatever the module being compiled binds.
    fn guile(&mut self, name: &str) -> Node {
        let symbol = self.heap.intern(name);
        Node::Global(self.globals.cell(GUILE, symbol))
    }

    /// Whether `x` is a module's name, a list of symbols.
    fn module_name(&self, x: Value) -> bool {
        x.tag() == Tag::Pair
            && self
                .items(x, x)
                .is_ok_and(|names| names.iter().all(|n| n.tag() == Tag::Symbol))
    }

    /// `(define-module name option …)` as `(%define-module 'name
    /// '(option …))`, which makes the module current when it runs.
    fn define_module(&mut self, x: Value) -> Result<Node> {
        let rest = self.cdr(x);
        if rest.tag() != Tag::Pair || !self.module_name(self.car(rest)) {
            return Err(self.bad(x));
        }
        self.items(rest, x)?;
        let call = vec![
            self.guile("%define-module"),
            Node::Const(self.car(rest)),
            Node::Const(self.cdr(rest)),
        ];
        Ok(Node::Call(call, false))
    }

    /// `(use-modules spec …)` as `(%use-modules '(spec …))`. A spec is a
    /// module's name, or a list of one and keywords with their arguments;
    /// psyntax's error names the specs from the first that is neither.
    fn use_modules(&mut self, x: Value) -> Result<Node> {
        let rest = self.cdr(x);
        let mut at = rest;
        while at.tag() == Tag::Pair {
            let spec = self.car(at);
            let options = self.cdr(spec);
            let valid = self.module_name(spec)
                || (self.module_name(self.car(spec))
                    && self.items(options, x).is_ok_and(|o| {
                        o.len() % 2 == 0 && o.iter().step_by(2).all(|k| k.tag() == Tag::Keyword)
                    }));
            if !valid {
                return Err(syntax(FAILED, at, None));
            }
            at = self.cdr(at);
        }
        if at != Value::NULL {
            return Err(syntax(FAILED, at, None));
        }
        Ok(Node::Call(
            vec![self.guile("%use-modules"), Node::Const(rest)],
            false,
        ))
    }

    /// `(@ module name)`, a module's exported variable, and `(@@ module
    /// name)`, any of its variables: the slot when the module has it now,
    /// else a lookup each time it runs.
    fn at(&mut self, x: Value, public: bool) -> Result<Node> {
        let items = self.items(self.cdr(x), x)?;
        let [module, name] = items[..] else {
            return Err(syntax(FAILED, x, None));
        };
        if !self.module_name(module) || name.tag() != Tag::Symbol {
            return Err(syntax(FAILED, x, None));
        }
        let mut path = Vec::new();
        let mut at = module;
        while let (Some(car), Some(cdr)) = (self.heap.car(at), self.heap.cdr(at)) {
            path.push(car.payload());
            at = cdr;
        }
        let found = self
            .globals
            .find(&path)
            .and_then(|m| self.globals.at(m, name, public));
        Ok(match found {
            Some(slot) => Node::Global(slot),
            None => Node::Call(
                vec![
                    self.guile("%at"),
                    Node::Const(module),
                    Node::Const(name),
                    Node::Const(Value::bool(public)),
                ],
                false,
            ),
        })
    }

    /// `(%dyn kind data extra body handler)`, the VM's own form.
    fn dyn_(&mut self, x: Value) -> Result<Node> {
        let items = self.items(self.cdr(x), x)?;
        let [kind, data, extra, body, handler] = items[..] else {
            return Err(self.bad(x));
        };
        let kind = match self.heap.name(kind) {
            Some("catch") => DynKind::Catch,
            Some("prompt") => DynKind::Prompt,
            Some("wind") => DynKind::Wind,
            Some("fluid") => DynKind::Fluid,
            _ => return Err(self.bad(x)),
        };
        let parts = [data, extra, body, handler].map(|p| self.expr(p, Value::FALSE));
        let [data, extra, body, handler] = parts;
        Ok(Node::Dyn(kind, Box::new([data?, extra?, body?, handler?])))
    }

    /// A `lambda`'s procedure, in a new procedure scope. `var` is the
    /// `letrec` variable it is the init of.
    fn lambda(
        &mut self,
        name: Value,
        formals: Value,
        body: Value,
        form: Value,
        var: Option<Var>,
    ) -> Result<Node> {
        // A definition's procedure comes here without `expr`.
        if self.depth == MAX_NESTING {
            return Err(syntax("expression nested too deeply", form, None));
        }
        if body == Value::NULL {
            return Err(self.bad(form));
        }
        self.depth += 1;
        let id = self.lambdas.len() as u32;
        self.lambdas.push(Lambda::new(Some(self.current), name));
        if let Some(var) = var {
            self.vars[var as usize].self_of = Some(id);
        }
        let outer = (self.current, self.scope.len());
        self.current = id;
        let mut at = formals;
        let mut names = Vec::new();
        while at.tag() == Tag::Pair {
            names.push(self.car(at));
            at = self.cdr(at);
        }
        let rest = (at.tag() == Tag::Symbol).then_some(at);
        let lambda = self.is(self.car(form), self.syn.lambda);
        if !matches!(at.tag(), Tag::Null | Tag::Symbol)
            || names.iter().any(|n| n.tag() != Tag::Symbol)
        {
            return Err(if lambda {
                self.bad_part("lambda", "invalid argument list", formals, formals)
            } else {
                self.bad(form)
            });
        }
        let all: Vec<Value> = names.iter().chain(&rest).copied().collect();
        if all.iter().enumerate().any(|(i, n)| all[..i].contains(n)) {
            return Err(SyntaxError {
                who: Some("lambda"),
                message: "duplicate identifier in argument list",
                form: formals,
                subform: Value::FALSE,
                at: Some(formals),
            });
        }
        for &n in names.iter().chain(&rest) {
            let var = self.new_var(false);
            self.lambdas[id as usize].params.push(var);
            self.bind(n, var);
        }
        let lambda = &mut self.lambdas[id as usize];
        lambda.required = names.len() as u32;
        lambda.rest = rest.is_some();
        let mut body = body;
        let first = self.car(body);
        if self.car(first) == self.syn.procedure {
            let star = self.heap.car(self.cdr(first)) == Some(Value::TRUE);
            let header = self.car(self.cdr(self.cdr(first)));
            let lambda = &mut self.lambdas[id as usize];
            (lambda.star, lambda.header) = (star, header);
            body = self.cdr(body);
        }
        // A named procedure of LilyPond's own Scheme keeps its formals to
        // print; an anonymous one prints its address and file in LilyPond,
        // which the engine does not give, and a `lambda*` prints `args`.
        let toplevel = self.lambdas[id as usize].parent == Some(0);
        let named = match self.globals.named {
            crate::module::NAMED_ALL => true,
            crate::module::NAMED_TOPLEVEL => toplevel,
            _ => false,
        };
        let lambda = &mut self.lambdas[id as usize];
        if named && lambda.name != Value::FALSE && !lambda.star {
            lambda.names = formals;
        }
        let body = self.body(body, form)?;
        self.lambdas[id as usize].body = Some(body);
        self.scope.truncate(outer.1);
        self.current = outer.0;
        self.depth -= 1;
        Ok(Node::Lambda(id))
    }

    /// A body: definitions, at its start or among its expressions, become
    /// `letrec*` variables visible in all of it.
    fn body(&mut self, forms: Value, form: Value) -> Result<Node> {
        let mut items = Vec::new();
        self.splice(forms, form, &mut items)?;
        let mark = self.mark();
        let mut defined = Vec::new();
        for item in &mut items {
            if let Item::Definition(x, var) = item {
                let (target, _) = self.target(*x)?;
                let name = self.definition_name(target);
                *var = self.new_var(true);
                self.bind(name, *var);
                defined.push(*var);
            }
        }
        if matches!(items.last(), None | Some(Item::Definition(..))) {
            // Guile names a definition's procedure as the lambda it makes.
            let shown = if self.is(self.car(form), self.syn.define) {
                let target = self.car(self.cdr(form));
                let formals = self.cdr(target);
                let lambda = self.heap.cons(formals, forms);
                self.heap.cons(self.syn.lambda, lambda)
            } else {
                form
            };
            return Err(syntax(
                "body should end with an expression",
                shown,
                Some(form),
            ));
        }
        // A body with definitions is a `letrec*`, a lexical environment.
        let lexical = u32::from(!defined.is_empty());
        self.lexical += lexical;
        let mut nodes = Vec::new();
        for item in items {
            nodes.push(match item {
                Item::Expression(x) => self.expr(x, Value::FALSE)?,
                Item::Definition(x, var) => {
                    let (target, rest) = self.target(x)?;
                    let value = self.definition_value(x, target, rest, Some(var))?;
                    self.vars[var as usize].ready = true;
                    Node::Init(var, Box::new(value))
                }
            });
        }
        self.lexical -= lexical;
        self.close(mark);
        let node = if nodes.len() == 1 {
            nodes.pop().unwrap_or(Node::Const(Value::UNSPECIFIED))
        } else {
            Node::Seq(nodes)
        };
        Ok(if defined.is_empty() {
            node
        } else {
            Node::Scope(defined, Box::new(node))
        })
    }

    /// The body's forms with `begin`s spliced in.
    fn splice(&self, forms: Value, form: Value, out: &mut Vec<Item>) -> Result<()> {
        for x in self.items(forms, form)? {
            if x.tag() == Tag::Pair {
                let head = self.car(x);
                if self.is(head, self.syn.define) {
                    out.push(Item::Definition(x, 0));
                    continue;
                }
                if self.is(head, self.syn.begin) {
                    self.splice(self.cdr(x), x, out)?;
                    continue;
                }
            }
            out.push(Item::Expression(x));
        }
        Ok(())
    }

    fn set(&mut self, x: Value) -> Result<Node> {
        let items = self.items(self.cdr(x), x)?;
        let [name, value] = items.as_slice() else {
            return Err(self.bad(x));
        };
        if name.tag() != Tag::Symbol {
            return Err(self.bad(x));
        }
        // Guile names the procedure a `lambda` set to a variable makes.
        let value = Box::new(self.inner(|c| c.expr(*value, *name))?);
        Ok(match self.lookup(*name) {
            Some(var) => {
                self.capture(var);
                self.vars[var as usize].assigned = true;
                Node::Set(var, value)
            }
            None => Node::SetGlobal(self.globals.reference(self.module, *name), value),
        })
    }

    /// `((name init) …)` as names and inits.
    fn bindings(&self, list: Value, form: Value) -> Result<Vec<(Value, Value)>> {
        let mut out = Vec::new();
        for b in self.items(list, form)? {
            match self.items(b, form)?.as_slice() {
                [name, init] if name.tag() == Tag::Symbol => out.push((*name, *init)),
                _ => return Err(self.bad(form)),
            }
        }
        Ok(out)
    }

    fn let_(&mut self, x: Value) -> Result<Node> {
        let rest = self.cdr(x);
        if rest.tag() != Tag::Pair {
            return Err(self.bad(x));
        }
        let first = self.car(rest);
        if first.tag() == Tag::Symbol {
            return self.named_let(x, first, self.cdr(rest));
        }
        let bindings = self.bindings(first, x)?;
        if self.cdr(rest) == Value::NULL {
            return Err(self.bad(x));
        }
        if bindings
            .iter()
            .enumerate()
            .any(|(i, b)| bindings[..i].iter().any(|a| a.0 == b.0))
        {
            return Err(SyntaxError {
                who: Some("let"),
                message: "duplicate bound variable",
                form: x,
                subform: Value::FALSE,
                at: None,
            });
        }
        let mark = self.mark();
        let vars: Vec<Var> = bindings.iter().map(|_| self.new_var(false)).collect();
        let mut nodes = Vec::new();
        for (&(name, init), &var) in bindings.iter().zip(&vars) {
            nodes.push(Node::Init(var, Box::new(self.named(init, name, None)?)));
        }
        for (&(name, _), &var) in bindings.iter().zip(&vars) {
            self.bind(name, var);
        }
        let lexical = u32::from(!bindings.is_empty());
        self.lexical += lexical;
        let body = self.body(self.cdr(rest), x);
        self.lexical -= lexical;
        nodes.push(body?);
        self.close(mark);
        Ok(Node::Scope(vars, Box::new(Node::Seq(nodes))))
    }

    /// `(let name ((var init) …) body …)`: `name` is a procedure of the
    /// vars, bound in the body only, and called with the inits.
    fn named_let(&mut self, x: Value, name: Value, rest: Value) -> Result<Node> {
        if rest.tag() != Tag::Pair {
            return Err(self.bad(x));
        }
        let bindings = self.bindings(self.car(rest), x)?;
        if self.cdr(rest) == Value::NULL {
            return Err(self.bad(x));
        }
        self.inner(|c| c.named_let_body(x, name, rest, bindings))
    }

    fn named_let_body(
        &mut self,
        x: Value,
        name: Value,
        rest: Value,
        bindings: Vec<(Value, Value)>,
    ) -> Result<Node> {
        let mark = self.mark();
        let var = self.new_var(true);
        let mut call = vec![Node::Ref(var)];
        for &(_, init) in &bindings {
            call.push(self.expr(init, Value::FALSE)?);
        }
        self.bind(name, var);
        let formals = bindings
            .iter()
            .rev()
            .fold(Value::NULL, |tail, &(n, _)| self.heap.cons(n, tail));
        let lambda = self.lambda(name, formals, self.cdr(rest), x, Some(var))?;
        self.vars[var as usize].ready = true;
        self.close(mark);
        let init = Node::Init(var, Box::new(lambda));
        Ok(Node::Scope(
            vec![var],
            Box::new(Node::Seq(vec![init, Node::Call(call, false)])),
        ))
    }

    fn let_star(&mut self, x: Value) -> Result<Node> {
        let rest = self.cdr(x);
        if rest.tag() != Tag::Pair {
            return Err(self.bad(x));
        }
        let bindings = self.bindings(self.car(rest), x)?;
        if self.cdr(rest) == Value::NULL {
            return Err(self.bad(x));
        }
        let mark = self.mark();
        let lexical = self.lexical;
        let mut vars = Vec::new();
        let mut nodes = Vec::new();
        for (name, init) in bindings {
            let init = self.named(init, name, None);
            let var = self.new_var(false);
            nodes.push(Node::Init(var, Box::new(init?)));
            self.bind(name, var);
            vars.push(var);
            // Each binding is a `let` around the rest.
            self.lexical += 1;
        }
        let body = self.body(self.cdr(rest), x);
        self.lexical = lexical;
        nodes.push(body?);
        self.close(mark);
        Ok(Node::Scope(vars, Box::new(Node::Seq(nodes))))
    }

    /// `letrec` and `letrec*`, both as `letrec*`.
    fn letrec(&mut self, x: Value) -> Result<Node> {
        let rest = self.cdr(x);
        if rest.tag() != Tag::Pair {
            return Err(self.bad(x));
        }
        let bindings = self.bindings(self.car(rest), x)?;
        if self.cdr(rest) == Value::NULL {
            return Err(self.bad(x));
        }
        let lexical = u32::from(!bindings.is_empty());
        self.lexical += lexical;
        let node = self.letrec_body(x, rest, bindings);
        self.lexical -= lexical;
        node
    }

    fn letrec_body(
        &mut self,
        x: Value,
        rest: Value,
        bindings: Vec<(Value, Value)>,
    ) -> Result<Node> {
        let mark = self.mark();
        let vars: Vec<Var> = bindings
            .iter()
            .map(|&(name, _)| {
                let var = self.new_var(true);
                self.bind(name, var);
                var
            })
            .collect();
        let mut nodes = Vec::new();
        for (&(name, init), &var) in bindings.iter().zip(&vars) {
            let init = self.named(init, name, Some(var))?;
            self.vars[var as usize].ready = true;
            nodes.push(Node::Init(var, Box::new(init)));
        }
        nodes.push(self.body(self.cdr(rest), x)?);
        self.close(mark);
        Ok(Node::Scope(vars, Box::new(Node::Seq(nodes))))
    }

    fn cond(&mut self, x: Value) -> Result<Node> {
        let clauses = self.items(self.cdr(x), x)?;
        if clauses.is_empty() {
            return Err(self.bad(x));
        }
        let lexical = self.lexical;
        let node = self.clauses(x, "cond", &clauses, None);
        self.lexical = lexical;
        node
    }

    /// `cond`'s clauses, or `case`'s with the key in `key`, as one flat
    /// chain, so that a long `cond` does not nest. After a `cond` clause with
    /// `=>`, the rest is inside the `let` of its value, as in Guile.
    fn clauses(
        &mut self,
        x: Value,
        who: &'static str,
        clauses: &[Value],
        key: Option<Var>,
    ) -> Result<Node> {
        let mut chain = Vec::new();
        let mut otherwise = Node::Const(Value::UNSPECIFIED);
        for (i, &clause) in clauses.iter().enumerate() {
            let invalid = || self.bad_part(who, "invalid clause", x, clause);
            let items = self.items(clause, x).map_err(|_| invalid())?;
            let Some((&head, body)) = items.split_first() else {
                return Err(invalid());
            };
            let arrow = body.first().is_some_and(|&b| self.is(b, self.syn.arrow));
            if self.is(head, self.syn.else_) {
                if i + 1 != clauses.len() {
                    return Err(self.bad_part(who, "else must be the last clause", x, clause));
                }
                if body.is_empty() {
                    return Err(invalid());
                }
                otherwise = match key {
                    Some(key) if arrow => self.arrow(x, who, clause, Node::Ref(key), body)?,
                    _ => self.seq(body)?,
                };
                break;
            }
            let Some(key) = key else {
                let test = self.expr(head, Value::FALSE)?;
                chain.push(if arrow {
                    self.lexical += 1;
                    let mark = self.mark();
                    let var = self.new_var(false);
                    let call = self.arrow(x, who, clause, Node::Ref(var), body)?;
                    self.close(mark);
                    Clause {
                        test: Node::Init(var, Box::new(test)),
                        then: Then::Arrow(var, call),
                    }
                } else if body.is_empty() {
                    Clause {
                        test,
                        then: Then::Test,
                    }
                } else {
                    Clause {
                        test,
                        then: Then::Body(self.seq(body)?),
                    }
                });
                continue;
            };
            if self.items(head, x).is_err() || body.is_empty() {
                return Err(invalid());
            }
            let then = if arrow {
                self.arrow(x, who, clause, Node::Ref(key), body)?
            } else {
                self.seq(body)?
            };
            chain.push(Clause {
                test: Node::Memv(Box::new(Node::Ref(key)), head),
                then: Then::Body(then),
            });
        }
        Ok(Node::Choose(chain, Box::new(otherwise)))
    }

    /// `(test => receiver)`: the receiver called with the value.
    fn arrow(
        &mut self,
        x: Value,
        who: &'static str,
        clause: Value,
        value: Node,
        body: &[Value],
    ) -> Result<Node> {
        let [_, receiver] = body else {
            return Err(self.bad_part(who, "wrong number of receiver expressions", x, clause));
        };
        Ok(Node::Call(
            vec![self.expr(*receiver, Value::FALSE)?, value],
            false,
        ))
    }

    fn case(&mut self, x: Value) -> Result<Node> {
        let items = self.items(self.cdr(x), x)?;
        let Some((&key, clauses)) = items.split_first() else {
            return Err(self.bad(x));
        };
        if clauses.is_empty() {
            return Err(self.bad(x));
        }
        let key = self.expr(key, Value::FALSE)?;
        let mark = self.mark();
        let var = self.new_var(false);
        // The clauses are inside the `let` of the key.
        let choice = self.inner(|c| c.clauses(x, "case", clauses, Some(var)))?;
        self.close(mark);
        let init = Node::Init(var, Box::new(key));
        Ok(Node::Scope(
            vec![var],
            Box::new(Node::Seq(vec![init, choice])),
        ))
    }
}

/// Psyntax's message for a form that matches no pattern.
pub(crate) const FAILED: &str = "source expression failed to match any pattern";

#[cold]
pub(crate) fn syntax(message: &'static str, form: Value, at: Option<Value>) -> SyntaxError {
    SyntaxError {
        who: None,
        message,
        form,
        subform: Value::FALSE,
        at,
    }
}

enum Item {
    Expression(Value),
    /// A `define` and the variable it binds, once made.
    Definition(Value, Var),
}

/// `(+ a b c)` as `(+ (+ a b) c)`, which is how Guile adds. Each argument
/// nests a node, so only a few are folded; more are a call.
fn fold_inline(op: Inline, slot: u32, mut args: Vec<Node>, direct: bool) -> Node {
    if args.len() <= 2 {
        return Node::Inline(op, slot, args, direct);
    }
    let last = args.pop().unwrap_or(Node::Const(Value::UNSPECIFIED));
    Node::Inline(
        op,
        slot,
        vec![fold_inline(op, slot, args, direct), last],
        direct,
    )
}

// --- the second pass -------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Ctx {
    /// One value is pushed.
    Value,
    /// A value is pushed; a call may leave multiple values.
    Multi,
    /// Nothing is pushed.
    Effect,
    /// The procedure returns.
    Tail,
}

struct Gen<'a> {
    c: &'a Compiler<'a>,
    lambda: u32,
    /// The free variables of this procedure, in its closures' order.
    free: Vec<Var>,
    ops: Vec<Op>,
    consts: Vec<Value>,
    children: Vec<Rc<Template>>,
}

impl<'a> Gen<'a> {
    fn template(c: &'a Compiler<'a>, lambda: u32) -> Rc<Template> {
        let l = &c.lambdas[lambda as usize];
        let mut g = Gen {
            c,
            lambda,
            free: free_values(c, lambda),
            ops: Vec::new(),
            consts: Vec::new(),
            children: Vec::new(),
        };
        for &p in &l.params {
            if g.boxed(p) {
                g.ops.push(Op::Box(c.vars[p as usize].slot));
            }
        }
        if let Some(body) = &l.body {
            g.node(body, Ctx::Tail);
        }
        Rc::new(Template {
            name: l.name,
            required: l.required,
            rest: l.rest,
            frame: l.slots,
            free: g.free.len() as u32,
            ops: g.ops.into_iter().map(Cell::new).collect(),
            consts: g.consts,
            children: g.children,
            star: l.star,
            header: l.header,
            names: l.names,
            mark: Cell::new(0),
            digest: Cell::new(0),
        })
    }

    fn boxed(&self, var: Var) -> bool {
        let v = &self.c.vars[var as usize];
        v.captured && (v.assigned || v.early)
    }

    fn here(&self) -> u32 {
        self.ops.len() as u32
    }

    fn patch(&mut self, at: u32) {
        let to = self.here();
        match &mut self.ops[at as usize] {
            Op::Jump(t) | Op::JumpFalse(t) | Op::AndJump(t) | Op::OrJump(t) => *t = to,
            _ => {}
        }
    }

    fn push_const(&mut self, v: Value) {
        let op = match v {
            Value::NULL => Op::Null,
            Value::FALSE => Op::False,
            Value::TRUE => Op::True,
            Value::UNSPECIFIED => Op::Unspecified,
            _ => match v.as_fixnum().and_then(|n| i32::try_from(n).ok()) {
                Some(n) => Op::Fixnum(n),
                None => {
                    self.consts.push(v);
                    Op::Const(self.consts.len() as u32 - 1)
                }
            },
        };
        self.ops.push(op);
    }

    /// After a value was pushed: what `ctx` does with it.
    fn finish(&mut self, ctx: Ctx) {
        match ctx {
            Ctx::Effect => self.ops.push(Op::Pop),
            Ctx::Tail => self.ops.push(Op::Return),
            Ctx::Value | Ctx::Multi => {}
        }
    }

    /// Pushes a variable's box if it has one, else its value.
    fn load_raw(&mut self, var: Var) {
        let v = &self.c.vars[var as usize];
        let op = if v.owner == self.lambda {
            Op::Local(v.slot)
        } else if is_self(self.c, var, self.lambda) {
            Op::Myself
        } else {
            Op::Free(self.free_index(var))
        };
        self.ops.push(op);
    }

    fn free_index(&self, var: Var) -> u32 {
        self.free.iter().position(|&f| f == var).unwrap_or(0) as u32
    }

    fn node(&mut self, n: &Node, ctx: Ctx) {
        match n {
            Node::Const(v) => {
                if ctx != Ctx::Effect {
                    self.push_const(*v);
                    self.finish(ctx);
                }
            }
            Node::Ref(var) => {
                if ctx == Ctx::Effect {
                    return;
                }
                let v = &self.c.vars[*var as usize];
                let boxed = self.boxed(*var);
                let op = if v.owner == self.lambda {
                    if boxed {
                        Op::LocalBox(v.slot)
                    } else {
                        Op::Local(v.slot)
                    }
                } else if is_self(self.c, *var, self.lambda) {
                    Op::Myself
                } else if boxed {
                    Op::FreeBox(self.free_index(*var))
                } else {
                    Op::Free(self.free_index(*var))
                };
                self.ops.push(op);
                self.finish(ctx);
            }
            Node::Global(slot) => {
                // Read even for effect: an unbound variable is an error.
                self.ops.push(Op::Global(*slot));
                self.finish(ctx);
            }
            Node::Set(var, value) => {
                self.node(value, Ctx::Value);
                let v = &self.c.vars[*var as usize];
                let op = match (v.owner == self.lambda, self.boxed(*var)) {
                    (true, true) => Op::SetLocalBox(v.slot),
                    (true, false) => Op::SetLocal(v.slot),
                    (false, _) => Op::SetFreeBox(self.free_index(*var)),
                };
                self.ops.push(op);
                self.unspecified(ctx);
            }
            Node::Init(var, value) => {
                self.node(value, Ctx::Value);
                let v = &self.c.vars[*var as usize];
                let op = match (self.boxed(*var), v.rec) {
                    (true, true) => Op::SetLocalBox(v.slot),
                    (true, false) => Op::SetLocal(v.slot),
                    (false, _) => Op::SetLocal(v.slot),
                };
                self.ops.push(op);
                if self.boxed(*var) && !v.rec {
                    self.ops.push(Op::Box(v.slot));
                }
                self.unspecified(ctx);
            }
            Node::SetGlobal(slot, value) | Node::Define(slot, value) => {
                self.node(value, Ctx::Value);
                self.ops.push(match n {
                    Node::Define(..) => Op::Define(*slot),
                    _ => Op::SetGlobal(*slot),
                });
                self.unspecified(ctx);
            }
            Node::If(arms) => {
                let [test, then, otherwise] = &**arms;
                self.node(test, Ctx::Value);
                let jump = self.here();
                self.ops.push(Op::JumpFalse(0));
                self.node(then, ctx);
                if ctx == Ctx::Tail {
                    self.patch(jump);
                    self.node(otherwise, ctx);
                } else {
                    let skip = self.here();
                    self.ops.push(Op::Jump(0));
                    self.patch(jump);
                    self.node(otherwise, ctx);
                    self.patch(skip);
                }
            }
            Node::Lambda(id) => {
                let child = Gen::template(self.c, *id);
                for var in free_values(self.c, *id) {
                    self.load_raw(var);
                }
                self.children.push(child);
                self.ops.push(Op::Closure(self.children.len() as u32 - 1));
                self.finish(ctx);
            }
            Node::Seq(nodes) => {
                let Some((last, first)) = nodes.split_last() else {
                    return;
                };
                for n in first {
                    self.node(n, Ctx::Effect);
                }
                self.node(last, ctx);
            }
            Node::Call(items, direct) => {
                for item in items {
                    self.node(item, Ctx::Value);
                }
                if *direct {
                    self.ops.push(Op::Direct);
                }
                self.call_op(items.len() as u32 - 1, ctx);
            }
            Node::Inline(op, slot, args, direct) => {
                for arg in args {
                    self.node(arg, Ctx::Value);
                }
                self.ops.push(Op::Inline(*op, *slot, *direct));
                self.finish(ctx);
            }
            Node::Scope(vars, body) => {
                for &var in vars {
                    let v = &self.c.vars[var as usize];
                    if v.rec && self.boxed(var) {
                        self.ops.push(Op::NewBox(v.slot));
                    }
                }
                self.node(body, ctx);
            }
            Node::And(nodes) | Node::Or(nodes) => {
                let and = matches!(n, Node::And(_));
                let mut jumps = Vec::new();
                let Some((last, first)) = nodes.split_last() else {
                    return;
                };
                for n in first {
                    self.node(n, Ctx::Value);
                    jumps.push(self.here());
                    self.ops
                        .push(if and { Op::AndJump(0) } else { Op::OrJump(0) });
                }
                let inner = if ctx == Ctx::Tail {
                    Ctx::Tail
                } else {
                    Ctx::Value
                };
                self.node(last, inner);
                for j in jumps {
                    self.patch(j);
                }
                if ctx == Ctx::Tail {
                    self.ops.push(Op::Return);
                } else {
                    self.finish(ctx);
                }
            }
            Node::Memv(key, datums) => {
                self.node(key, Ctx::Value);
                self.consts.push(*datums);
                self.ops.push(Op::Memv(self.consts.len() as u32 - 1));
                self.finish(ctx);
            }
            Node::Choose(clauses, otherwise) => {
                let mut ends = Vec::new();
                let mut hits = Vec::new();
                for clause in clauses {
                    let then = match &clause.then {
                        Then::Test => {
                            self.node(&clause.test, Ctx::Value);
                            hits.push(self.here());
                            self.ops.push(Op::OrJump(0));
                            continue;
                        }
                        Then::Body(then) => {
                            self.node(&clause.test, Ctx::Value);
                            then
                        }
                        Then::Arrow(var, call) => {
                            self.node(&clause.test, Ctx::Effect);
                            self.node(&Node::Ref(*var), Ctx::Value);
                            call
                        }
                    };
                    let next = self.here();
                    self.ops.push(Op::JumpFalse(0));
                    self.node(then, ctx);
                    if ctx != Ctx::Tail {
                        ends.push(self.here());
                        self.ops.push(Op::Jump(0));
                    }
                    self.patch(next);
                }
                self.node(otherwise, ctx);
                if !hits.is_empty() {
                    if ctx != Ctx::Tail {
                        ends.push(self.here());
                        self.ops.push(Op::Jump(0));
                    }
                    for hit in hits {
                        self.patch(hit);
                    }
                    self.finish(ctx);
                }
                for end in ends {
                    self.patch(end);
                }
            }
            Node::Dyn(kind, parts) => {
                let [data, extra, body, handler] = &**parts;
                self.node(data, Ctx::Value);
                self.node(extra, Ctx::Value);
                let push = self.here();
                self.ops.push(Op::PushDyn(*kind, 0));
                let inner = match ctx {
                    Ctx::Tail => Ctx::Multi,
                    other => other,
                };
                self.node(body, inner);
                self.ops.push(Op::PopDyn);
                let end = self.here();
                if ctx == Ctx::Tail {
                    self.ops.push(Op::Return);
                } else {
                    self.ops.push(Op::Jump(0));
                }
                // The landing: `(apply handler pending)`.
                let landing = self.here();
                if let Op::PushDyn(_, to) = &mut self.ops[push as usize] {
                    *to = landing;
                }
                self.push_const(Value::tagged(Tag::Primitive, prims::APPLY));
                self.node(handler, Ctx::Value);
                self.ops.push(Op::Pending);
                self.call_op(2, ctx);
                if ctx != Ctx::Tail {
                    self.patch(end);
                }
            }
            Node::CallValues(parts) => {
                let [consumer, producer] = &**parts;
                self.node(consumer, Ctx::Value);
                self.node(producer, Ctx::Multi);
                if ctx == Ctx::Tail {
                    self.ops.push(Op::TailCallValues);
                } else {
                    self.ops.push(Op::CallValues);
                    self.finish(ctx);
                }
            }
        }
    }

    /// The call of the procedure below `argc` arguments, as `ctx` takes it.
    fn call_op(&mut self, argc: u32, ctx: Ctx) {
        match ctx {
            Ctx::Tail => self.ops.push(Op::TailCall(argc)),
            Ctx::Value => self.ops.push(Op::Call(argc)),
            Ctx::Multi => self.ops.push(Op::CallMulti(argc)),
            Ctx::Effect => {
                self.ops.push(Op::CallMulti(argc));
                self.ops.push(Op::Pop);
            }
        }
    }

    /// The unspecified value of `set!` and `define`, where one is wanted.
    fn unspecified(&mut self, ctx: Ctx) {
        if ctx != Ctx::Effect {
            self.ops.push(Op::Unspecified);
            self.finish(ctx);
        }
    }
}

/// Whether `lambda` reaches `var` as itself: it is the init of the `letrec`
/// variable, which is never assigned.
fn is_self(c: &Compiler<'_>, var: Var, lambda: u32) -> bool {
    let v = &c.vars[var as usize];
    v.self_of == Some(lambda) && !v.assigned
}

/// What a closure of `lambda` holds: its free variables but itself.
fn free_values(c: &Compiler<'_>, lambda: u32) -> Vec<Var> {
    c.lambdas[lambda as usize]
        .free
        .iter()
        .copied()
        .filter(|&v| !is_self(c, v, lambda))
        .collect()
}
