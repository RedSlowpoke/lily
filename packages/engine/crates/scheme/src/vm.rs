//! The VM (chapter 03 §5.3, R2, R3): one loop over the compiler's
//! bytecode with its own value stack and return stack, so that no Scheme
//! call, in tail position or not, uses the native (wasm) stack. `apply`,
//! `call-with-values`, `map` and `for-each` run inside the loop: `apply` is
//! a primitive the loop spreads, and the other three are Scheme, compiled
//! from [`PRELUDE`] when the VM is made.
//!
//! A call's frame on the value stack is the procedure, its arguments, then
//! its other locals; temporaries go above. The return stack holds the
//! caller's code, instruction and frame, and what the caller's continuation
//! takes: one value, any number (an effect, or `call-with-values`), or the
//! value of [`Vm::apply`] itself. A [`Tag::Values`] object given to a
//! continuation that takes one value gives its first, as Guile 3 does.
//!
//! The collector runs at a procedure's entry, and at once for `(gc)`: its
//! roots are the value stack, which holds every running closure and so its
//! code's constants, the global slots, the modules and the heap's root
//! stack.
//!
//! A global's slot is read with one load. A slot with no value may be a
//! cell that stands for an import (`module.rs`): the first time it runs, the
//! instruction is pointed at the imported variable, as Guile memoizes a
//! toplevel reference.
//!
//! Errors are Guile's throws, a key and its arguments `(subr message args
//! rest)` ([`Exception`], worded by `errors.rs`). A throw unwinds to the
//! innermost `catch` of its key, running `dynamic-wind`'s `after` thunks on
//! the way (`control.rs`), or to the caller of [`Vm::eval`] or [`Vm::apply`].

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use crate::compile::{Op, Syntax, SyntaxError, Template};
use crate::control::{self, CANCEL_INTERVAL, CancelCheck, Dyn};
use crate::errors::{
    APPLY_TO_NON_LIST, STACK_OVERFLOW, UNBOUND_VARIABLE, WRONG_NUMBER_OF_ARGS, WRONG_TYPE_TO_APPLY,
    ZERO_VALUES,
};
use crate::expand::{self, Expander, Fail};
use crate::hash;
use crate::heap::Heap;
use crate::lists;
use crate::module::{self, GUILE, GUILE_USER, Globals, LILY, ModuleObject};
use crate::ports;
use crate::prims::{self, Inline, PRIMS, Prim, REST};
use crate::reader::{LilyScan, Position, Read, ReadError, Span, read};
use crate::strings;
use crate::value::{Tag, Value};

/// How many frames and values the stacks may hold before a call is a
/// `stack-overflow`: a million non-tail calls deep fit. Guile 3 grows its
/// stack until memory runs out.
pub const MAX_FRAMES: usize = 2 << 20;
pub const MAX_STACK: usize = 8 << 20;

/// `call-with-values`, `map` and `for-each`, as Guile 3's `boot-9.scm`
/// defines `map` and `for-each`, without `case-lambda`.
const PRELUDE: &str = r#"
(define (call-with-values producer consumer)
  (%call-values consumer (producer)))
(define (map f l . ls)
  (cond
   ((null? ls)
    (if (not (list? l))
        (scm-error 'wrong-type-arg "map" "Not a list: ~S" (list l) #f))
    (let map1 ((l l))
      (if (pair? l)
          (cons (f (car l)) (map1 (cdr l)))
          '())))
   ((null? (cdr ls))
    (let ((l1 l) (l2 (car ls)))
      (if (not (= (length l1) (length l2)))
          (scm-error 'wrong-type-arg "map" "List of wrong length: ~S" (list l2) #f))
      (let map2 ((l1 l1) (l2 l2))
        (if (pair? l1)
            (cons (f (car l1) (car l2)) (map2 (cdr l1) (cdr l2)))
            '()))))
   (else
    (let ((len (length l)))
      (let mapn ((rest ls))
        (or (null? rest)
            (if (= (length (car rest)) len)
                (mapn (cdr rest))
                (scm-error 'wrong-type-arg "map" "List of wrong length: ~S"
                           (list (car rest)) #f)))))
    (let mapn ((l1 l) (rest ls))
      (if (pair? l1)
          (cons (apply f (car l1) (map car rest))
                (mapn (cdr l1) (map cdr rest)))
          '())))))
(define (for-each f l . ls)
  (cond
   ((null? ls)
    (if (not (list? l))
        (scm-error 'wrong-type-arg "for-each" "Not a list: ~S" (list l) #f))
    (let for-each1 ((l l))
      (if (not (null? l))
          (begin
            (f (car l))
            (for-each1 (cdr l))))))
   ((null? (cdr ls))
    (let ((l1 l) (l2 (car ls)))
      (if (not (= (length l1) (length l2)))
          (scm-error 'wrong-type-arg "for-each" "List of wrong length: ~S" (list l2) #f))
      (let for-each2 ((l1 l1) (l2 l2))
        (if (not (null? l1))
            (begin
              (f (car l1) (car l2))
              (for-each2 (cdr l1) (cdr l2)))))))
   (else
    (let ((len (length l)))
      (let for-eachn ((rest ls))
        (or (null? rest)
            (if (= (length (car rest)) len)
                (for-eachn (cdr rest))
                (scm-error 'wrong-type-arg "for-each" "List of wrong length: ~S"
                           (list (car rest)) #f)))))
    (let for-eachn ((l1 l) (rest ls))
      (if (pair? l1)
          (begin
            (apply f (car l1) (map car rest))
            (for-eachn (cdr l1) (map cdr rest))))))))
"#;

// --- errors ------------------------------------------------------------------------

/// A Guile throw: its key and arguments. Errors have the arguments `(subr
/// message args rest)`: the procedure's name or `#f`, the message with `~A`
/// and `~S` for `args`, and the rest (`#f`, or data for the handler).
/// [`Exception::text`] words it as Guile prints it.
///
/// Its values are not rooted: they live until the VM's next safe point, so
/// the caller reads them, or roots them, before evaluating again.
#[derive(Clone, Copy, Debug)]
pub struct Exception {
    pub key: Value,
    pub args: Value,
}

/// A text the reader read from, as Guile's port sees it: named `file`,
/// with its byte `offset` at the line and column `at`. LilyPond reads an
/// embedded form from the score's port, set to the form's line and to its
/// column less one (`parse-scm.cc`); a loaded file starts at byte 0, line 0,
/// column 0 ([`Source::new`]).
#[derive(Clone, Copy, Debug)]
pub struct Source<'t> {
    pub text: &'t str,
    pub file: Option<&'t str>,
    pub offset: usize,
    pub at: Position,
}

impl<'t> Source<'t> {
    /// The whole of `text`, from its first byte.
    pub fn new(text: &'t str, file: Option<&'t str>) -> Source<'t> {
        Source {
            text,
            file,
            offset: 0,
            at: Position::default(),
        }
    }

    /// Where the port is at the byte `byte`, counted from `offset`.
    pub fn position(&self, byte: usize) -> Position {
        let end = byte.min(self.text.len()).max(self.offset);
        self.at
            .advance(self.text.get(self.offset..end).unwrap_or(""))
    }
}

/// That an exception is being thrown; it waits in [`Cx::error`] (or, for an
/// escape or a cancellation, [`Cx::escape`] and [`Cx::cancelled`]). Only
/// what sets one of them returns it: a primitive of another crate gets it
/// from [`Cx`]'s methods that raise.
#[derive(Debug)]
pub struct Thrown;

/// What primitives work on: the heap, the output and the pending throw. A
/// primitive of another crate ([`Vm::define_primitives`]) reaches them
/// through its public methods.
pub struct Cx {
    pub(crate) heap: Heap,
    /// What `display`, `write` and `newline` wrote, for the host to take.
    pub(crate) out: String,
    pub(crate) error: Option<Exception>,
    /// An escape procedure's call: its prompt's tag and the values.
    pub(crate) escape: Option<(Value, Value)>,
    /// The host's cancel check said stop.
    pub(crate) cancelled: bool,
    /// The run was cancelled for a file the host must be asked for
    /// ([`Vm::waiting`]).
    pub(crate) waiting: bool,
    /// The primitive being called was called by its name inside a lexical
    /// environment, where Guile words some errors as compiled code's
    /// (`errors.rs`).
    pub(crate) direct: bool,
    /// The global slots and the modules.
    pub(crate) globals: Globals,
    /// What the standard error port got, for the host to take.
    pub(crate) errors: String,
    /// The bytes of `errors` that `force-output` flushed: Guile's error
    /// port is buffered, so what follows them waits for the next flush.
    pub(crate) flushed: usize,
    /// The fluids of the current output, error and input ports.
    pub(crate) output: Value,
    pub(crate) error_port: Value,
    pub(crate) input: Value,
    /// How many symbols `gensym` made.
    pub(crate) gensyms: u64,
    /// `read-hash-extend` gave `#\{` to LilyPond's reader, as
    /// `parser-ly-from-scheme.scm` does: `#{` then reads as
    /// [`crate::LilyScan`] reads it, in [`Vm::eval_text`] and `read`.
    pub(crate) lily_braces: bool,
    /// The host's source of a part's files (D74), which `%part-source`
    /// asks; not in the image.
    pub(crate) part_source: Option<crate::lily::PartSource>,
    /// Where the files a template's `\include` reads come from.
    pub(crate) file_source: Option<crate::lily::FileSource>,
    /// A procedure and its arguments that the primitive returning calls in
    /// its place ([`Cx::tail_call`]).
    pub(crate) tail: Option<(Value, Vec<Value>)>,
    /// The host's procedure for `%host-call` ([`Vm::set_host`]); not in the
    /// image.
    pub(crate) host: Option<crate::lily::HostFn>,
    /// Other crates' handlers for `%host-call`, by its `what`, before the
    /// host ([`Vm::add_host_handlers`]); not in the image.
    pub(crate) handlers: Vec<crate::lily::HostHandler>,
    /// `%host-call` returned: the loop calls this with its arguments.
    pub(crate) hosting: Option<crate::lily::HostFn>,
    /// How many [`Vm::apply`] calls are running, and whether the outermost is
    /// a call in steps ([`Vm::begin_steps`]); the primitive returning asks it
    /// to return to the host ([`Cx::yield_step`]).
    pub(crate) applying: u32,
    pub(crate) stepping: bool,
    pub(crate) yielding: bool,
    /// The procedure transformers running, innermost last (D273).
    pub(crate) transcriptions: Vec<crate::expand::Transcription>,
    /// The arguments of a syntax error a transformer threw with no place,
    /// which [`Vm::failed`] places by its form, as psyntax does.
    pub(crate) unplaced: Value,
}

impl Cx {
    /// With the heap, the globals and the fluids of the standard output,
    /// error and input ports.
    pub(crate) fn new(heap: Heap, globals: Globals, ports: [Value; 3]) -> Cx {
        let [output, error_port, input] = ports;
        Cx {
            heap,
            out: String::new(),
            error: None,
            escape: None,
            cancelled: false,
            waiting: false,
            direct: false,
            globals,
            errors: String::new(),
            flushed: 0,
            output,
            error_port,
            input,
            gensyms: 0,
            lily_braces: false,
            part_source: None,
            file_source: None,
            tail: None,
            host: None,
            handlers: Vec::new(),
            hosting: None,
            applying: 0,
            stepping: false,
            yielding: false,
            transcriptions: Vec::new(),
            unplaced: Value::FALSE,
        }
    }

    /// The primitive returning asks a call in steps ([`Vm::begin_steps`]) to
    /// return to the host, if Scheme called it from that call's own frames:
    /// nothing in Rust is between them. Elsewhere it asks nothing.
    pub fn yield_step(&mut self) {
        self.yielding = self.stepping && self.applying == 1;
    }

    /// How many bytes of what the standard error port got since the host
    /// last took it `force-output` flushed ([`Vm::take_error_port`]): what a
    /// message the host keeps in order follows.
    pub fn error_port_flushed(&self) -> usize {
        self.flushed
    }

    /// Makes the primitive that returns this call `proc` with `args` in its
    /// place, as a call in tail position: the procedure's values are the
    /// primitive's. What the primitive returns with it is ignored. So a
    /// primitive can leave to Scheme what it cannot do natively, which it
    /// could not call itself.
    pub fn tail_call(&mut self, proc: Value, args: &[Value]) -> Value {
        self.tail = Some((proc, args.to_vec()));
        Value::UNSPECIFIED
    }

    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    /// Whether a run was cancelled for the host ([`Vm::waiting`]): a
    /// `%host-call` made now would be cancelled too (D127).
    pub fn waiting(&self) -> bool {
        self.waiting
    }

    pub fn heap_mut(&mut self) -> &mut Heap {
        &mut self.heap
    }

    /// The value of `name` in the module `module` (named as in
    /// [`Vm::provide_module`]), its own or imported; `None` if there is no
    /// such module or it is unbound.
    pub fn global(&mut self, module: &str, name: &str) -> Option<Value> {
        let module = self.globals.named(&mut self.heap, module)?;
        let symbol = self.heap.intern(name);
        let slot = self.globals.lookup(module, symbol)?;
        let value = self.globals.values[slot as usize];
        (value != Value::UNDEFINED).then_some(value)
    }

    /// [`Vm::make_module`], for a primitive.
    pub fn make_module(&mut self, uses: &[&str]) -> Option<Value> {
        let mut modules = alloc::vec![GUILE];
        for name in uses {
            let path: Vec<u32> = name
                .split_whitespace()
                .map(|s| self.heap.intern(s).payload())
                .collect();
            modules.push(self.globals.find(&path)?);
        }
        let index = self.globals.anonymous(&mut self.heap, &modules);
        Some(self.globals.object(index))
    }

    /// [`Vm::module_copy`], for a primitive.
    pub fn module_copy(&mut self, to: Value, from: Value) -> bool {
        let heap = &self.heap;
        let (Some(to), Some(from)) = (
            heap.foreign::<ModuleObject>(to).map(|m| m.index),
            heap.foreign::<ModuleObject>(from).map(|m| m.index),
        ) else {
            return false;
        };
        for (symbol, value) in self.globals.locals(from) {
            let slot = self.globals.cell(to, symbol);
            self.globals.define(slot, value);
        }
        true
    }

    /// The value of the symbol `symbol` in the module `module`, its own or
    /// imported, as [`Vm::module_value`] finds it; `None` if it is unbound
    /// or `module` is no module.
    pub fn module_ref(&self, module: Value, symbol: Value) -> Option<Value> {
        let index = self.heap.foreign::<ModuleObject>(module)?.index;
        let slot = self.globals.lookup(index, symbol)?;
        let value = self.globals.values[slot as usize];
        (value != Value::UNDEFINED).then_some(value)
    }

    /// Defines the symbol `symbol` in the module `module`, as Guile's
    /// `module-define!` does; false if `module` is no module.
    pub fn module_define(&mut self, module: Value, symbol: Value, value: Value) -> bool {
        let Some(index) = self.heap.foreign::<ModuleObject>(module).map(|m| m.index) else {
            return false;
        };
        let slot = self.globals.cell(index, symbol);
        self.globals.define(slot, value);
        true
    }

    /// The variables `module` binds itself, as [`Vm::module_locals`] gives
    /// them; `None` if `module` is no module.
    pub fn module_locals(&self, module: Value) -> Option<Vec<(Value, Value)>> {
        let index = self.heap.foreign::<ModuleObject>(module)?.index;
        Some(self.globals.locals(index))
    }
}

// --- the VM --------------------------------------------------------------------------

/// What the caller's continuation takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cont {
    /// One value: multiple values give their first, zero is an error.
    Single,
    /// Any number of values.
    Multi,
    /// The caller is [`Vm::apply`].
    Exit,
    /// No frame of its own: the call is in tail position.
    Tail,
}

/// The caller's state, saved by a call that is not in tail position.
pub(crate) struct Frame {
    code: Rc<Template>,
    pc: u32,
    fp: u32,
    cont: Cont,
}

/// A global variable's slot: its value, its version and its module
/// (R10).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Slot(u32);

/// Where an item began in the undo logs of the heap and of the slots
/// (R11): what [`Vm::undo`] goes back to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mark {
    heap: usize,
    slots: usize,
}

impl Mark {
    /// The heap's undo log's length at it.
    pub fn heap_len(&self) -> usize {
        self.heap
    }
}

/// What [`Vm::undo_keeping`] kept of the items it undid, which
/// [`Vm::redo`] does again: ranges of the heap's and the slots' redo
/// stores, valid until [`Vm::forget_redo`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Redo {
    heap: core::ops::Range<usize>,
    slots: core::ops::Range<usize>,
}

/// What a toplevel item depended on and did (02 §5.7, R10): the slots it
/// read, each with its version at its first read, and those it defined,
/// set or exported, in order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Item {
    pub mark: Mark,
    pub reads: Vec<(Slot, u32)>,
    pub writes: Vec<Slot>,
}

/// The interpreter: the heap, the global slots and modules, and the stacks.
pub struct Vm {
    pub(crate) cx: Cx,
    pub(crate) syntax: Syntax,
    pub(crate) expander: Expander,
    /// The primitive behind each [`Inline`] op, which its global must still
    /// hold for the op to run inline.
    inline: Vec<Value>,
    pub(crate) stack: Vec<Value>,
    pub(crate) frames: Vec<Frame>,
    pub(crate) code: Rc<Template>,
    pub(crate) pc: usize,
    /// Where the running procedure's arguments start; its closure is below.
    pub(crate) fp: usize,
    /// The entries of `catch`, escapes, `dynamic-wind` and fluids.
    pub(crate) dynamic: Vec<Dyn>,
    /// What a throw left for the landing it went to.
    pub(crate) pending: Value,
    /// `(lily)`'s `%location`, whose fluid `(*location*)` reads.
    pub(crate) location: u32,
    /// `%in-module`'s slot in `(guile)`: `eval` runs its form in it.
    pub(crate) in_module: u32,
    /// Calls and backward jumps until the next cancel check.
    pub(crate) ticks: u32,
    /// Procedures applied: closures entered and primitives called, not the
    /// calls the compiler made inline (chapter 03 §2.3's count).
    pub(crate) applications: u64,
    pub(crate) cancel: Option<CancelCheck>,
    /// What says the memory left is too little to go on, the collections
    /// when it first said so and the times asked since (D259).
    pub(crate) short: Option<fn(usize) -> bool>,
    pub(crate) pressed: Option<(u32, u32)>,
    /// The call in steps under way, and whether it has just returned to the
    /// host ([`Vm::begin_steps`]).
    steps: Option<Steps>,
    yielded: bool,
    /// Which modules the collection under way has reached.
    collecting_live: Vec<bool>,
}

/// Where a call in steps began: what [`Vm::apply`] keeps in its frame.
#[derive(Clone, Copy)]
struct Steps {
    stack: usize,
    frames: usize,
    base: usize,
    /// What says the step's time is spent, asked at the cancel check
    /// ([`Vm::begin_steps_due`]).
    due: Option<fn() -> bool>,
}

impl Default for Vm {
    fn default() -> Vm {
        Vm::new()
    }
}

impl Vm {
    /// A VM with the primitives and the preludes defined in `(guile)`,
    /// `(lily)`'s first definitions, and `(guile-user)` current.
    pub fn new() -> Vm {
        let mut heap = Heap::new();
        let procedure = heap.uninterned("%procedure");
        let syntax = Syntax::new(&mut heap, procedure);
        let expander = Expander::new(&mut heap, procedure);
        let mut globals = Globals::new(&mut heap);
        let ports = ports::standard(&mut heap);
        let homes: Vec<u32> = prims::MODULES
            .iter()
            .map(|(path, _, _)| match path {
                ["lily"] => LILY,
                _ => globals.builtin(&mut heap, path),
            })
            .collect();
        let mut vm = Vm::assemble(Cx::new(heap, globals, ports), syntax, expander);
        let [output, error_port, input] = ports;
        for (index, prim) in PRIMS.iter().enumerate() {
            let symbol = vm.cx.heap.intern(prim.name);
            let home = prims::MODULES
                .iter()
                .position(|(_, names, _)| names.contains(&prim.name))
                .map_or(GUILE, |i| homes[i]);
            let slot = vm.cx.globals.cell(home, symbol);
            vm.cx
                .globals
                .define(slot, Value::tagged(Tag::Primitive, index as u32));
        }
        let fluid = vm.cx.globals.current;
        vm.define_in(GUILE, "%module-fluid", fluid);
        vm.define_in(GUILE, "%output-fluid", output);
        vm.define_in(GUILE, "%error-fluid", error_port);
        vm.define_in(GUILE, "%input-fluid", input);
        let curried = prims::MODULES
            .iter()
            .position(|(path, _, _)| *path == ["lily", "curried-definitions"])
            .map_or(LILY, |i| homes[i]);
        for (module, name, value) in expand::natives(&mut vm.cx.heap, curried) {
            vm.define_in(module, name, value);
        }
        // The preludes are the crate's own and have no errors; the tests
        // read all of them.
        let guile = [
            PRELUDE,
            control::PRELUDE,
            module::PRELUDE,
            expand::PRELUDE,
            prims::PRELUDE,
            lists::PRELUDE,
            hash::PRELUDE,
            strings::PRELUDE,
            ports::PRELUDE,
            crate::array::PRELUDE,
        ]
        .map(|prelude| (GUILE, prelude));
        let modules = prims::MODULES
            .iter()
            .zip(&homes)
            .map(|((_, _, prelude), &home)| (home, *prelude));
        for (module, prelude) in guile.into_iter().chain(modules) {
            let object = vm.cx.globals.object(module);
            vm.cx.globals.set_current(&mut vm.cx.heap, object);
            let mut at = 0;
            while let Ok(r) = read(&mut vm.cx.heap, prelude, at, None)
                && r.datum != Value::EOF
            {
                at = r.end;
                let compiled = vm.compile_form(r.datum, module, true);
                let ran = compiled.ok().map(|code| {
                    let closure = vm.cx.heap.closure(code, Default::default());
                    vm.apply(closure, &[])
                });
                debug_assert!(matches!(ran, Some(Ok(_))), "the prelude runs");
            }
            debug_assert_eq!(at, prelude.trim_end().len(), "the prelude is read");
        }
        let user = vm.cx.globals.object(GUILE_USER);
        vm.cx.globals.set_current(&mut vm.cx.heap, user);
        let location = vm.cx.heap.intern("%location");
        vm.location = vm.cx.globals.cell(LILY, location);
        let in_module = vm.cx.heap.intern("%in-module");
        vm.in_module = vm.cx.globals.cell(GUILE, in_module);
        vm
    }

    /// A VM of a heap and globals made or restored (`image.rs`), with empty
    /// stacks.
    pub(crate) fn assemble(cx: Cx, syntax: Syntax, expander: Expander) -> Vm {
        Vm {
            cx,
            syntax,
            expander,
            inline: Inline::ALL
                .iter()
                .map(|op| {
                    let index = PRIMS.iter().position(|p| p.name == op.name());
                    index.map_or(Value::UNDEFINED, |i| {
                        Value::tagged(Tag::Primitive, i as u32)
                    })
                })
                .collect(),
            stack: Vec::new(),
            frames: Vec::new(),
            code: Template::empty(),
            pc: 0,
            fp: 0,
            dynamic: Vec::new(),
            pending: Value::NULL,
            location: 0,
            in_module: 0,
            ticks: CANCEL_INTERVAL,
            applications: 0,
            cancel: None,
            short: None,
            pressed: None,
            steps: None,
            yielded: false,
            collecting_live: Vec::new(),
        }
    }

    fn define_in(&mut self, module: u32, name: &str, value: Value) {
        let symbol = self.cx.heap.intern(name);
        let slot = self.cx.globals.cell(module, symbol);
        self.cx.globals.define(slot, value);
    }

    pub fn heap(&self) -> &Heap {
        &self.cx.heap
    }

    pub fn heap_mut(&mut self) -> &mut Heap {
        &mut self.cx.heap
    }

    /// How many procedures were applied since the VM was made, the
    /// preludes' definitions included.
    pub fn applications(&self) -> u64 {
        self.applications
    }

    /// Takes what `display`, `write` and `newline` have written.
    pub fn take_output(&mut self) -> String {
        core::mem::take(&mut self.cx.out)
    }

    /// Whether a run was cancelled since the last call because the host
    /// must be asked for a part's file ([`crate::Hosted::Ask`]): what it
    /// did is to be dropped and the run made again once the host has
    /// answered. Taking it clears it.
    pub fn take_waiting(&mut self) -> bool {
        core::mem::take(&mut self.cx.waiting)
    }

    /// Whether a run was cancelled for the host and [`Vm::take_waiting`]
    /// was not called since.
    pub fn waiting(&self) -> bool {
        self.cx.waiting
    }

    /// Takes what was written to the standard error port since the last
    /// time.
    pub fn take_errors(&mut self) -> String {
        self.take_error_port().0
    }

    /// Guile's `Stack overflow`, for a host that nests runs too deeply
    /// (`%host-call`).
    pub fn stack_overflow_error(&mut self) -> Exception {
        self.stack_overflow();
        self.cx.error.take().unwrap_or(Exception {
            key: Value::FALSE,
            args: Value::NULL,
        })
    }

    /// How many bytes of what the standard error port got since it was
    /// last taken `force-output` flushed ([`Cx::error_port_flushed`]).
    pub fn error_port_flushed(&self) -> usize {
        self.cx.flushed
    }

    /// Whether nothing was written to the standard error port since it was
    /// last taken.
    pub fn error_port_empty(&self) -> bool {
        self.cx.errors.is_empty()
    }

    /// Takes what was written to the standard error port since the last
    /// time, and how many of its bytes `force-output` flushed.
    /// How much has been written to the error port and not taken.
    pub fn error_len(&self) -> usize {
        self.cx.errors.len()
    }

    /// What has been written to the error port since it was `len` long
    /// (N8's resume, D241).
    pub fn errors_since(&self, len: usize) -> String {
        self.cx.errors.get(len..).unwrap_or("").into()
    }

    /// `text` written to the error port as it was written once before:
    /// what a translation resumed at a checkpoint said before it (D241).
    pub fn write_errors(&mut self, text: &str) {
        self.cx.errors.push_str(text);
    }

    /// The error port's text cut back to `len` bytes (N8's snapshots,
    /// D241): what a translation restored at a checkpoint wrote after it.
    pub fn cut_errors(&mut self, len: usize) {
        if len <= self.cx.errors.len() && self.cx.errors.is_char_boundary(len) {
            self.cx.errors.truncate(len);
        }
    }

    pub fn take_error_port(&mut self) -> (String, usize) {
        let flushed = core::mem::take(&mut self.cx.flushed);
        (core::mem::take(&mut self.cx.errors), flushed)
    }

    /// A module of no name, as LilyPond's `ly_make_module` makes a score's:
    /// it exports every variable it defines, and uses `(guile)` and then
    /// each module of `uses` (named as in [`Vm::provide_module`]), the last
    /// searched first. `None` if a module of `uses` does not exist.
    pub fn make_module(&mut self, uses: &[&str]) -> Option<Value> {
        self.cx.make_module(uses)
    }

    /// The value of `name` in `module`, its own or imported, as Guile's
    /// `module-variable` finds it; `None` if it is unbound or `module` is
    /// no module.
    pub fn module_value(&mut self, module: Value, name: &str) -> Option<Value> {
        let index = self.cx.heap.foreign::<ModuleObject>(module)?.index;
        let symbol = self.cx.heap.intern(name);
        let slot = self.cx.globals.lookup(index, symbol)?;
        // A host's lookup, as the parser's of a `\name`, is an item's
        // dependency (R10, MS6 step 3).
        self.cx.globals.track(slot);
        let value = self.cx.globals.values[slot as usize];
        (value != Value::UNDEFINED).then_some(value)
    }

    /// Defines `name` in `module`, as Guile's `module-define!` does; false
    /// if `module` is no module.
    /// [`Vm::module_define`] that no item logs: for a variable that Rust
    /// sets and always gives back itself, as the parser's `%parser`, which
    /// an undo must not give a value that has gone since (D235).
    pub fn module_define_unlogged(&mut self, module: Value, name: &str, value: Value) -> bool {
        let logging = core::mem::replace(&mut self.cx.globals.logging, false);
        let defined = self.module_define(module, name, value);
        self.cx.globals.logging = logging;
        defined
    }

    pub fn module_define(&mut self, module: Value, name: &str, value: Value) -> bool {
        match self.cx.heap.foreign::<ModuleObject>(module) {
            Some(m) => {
                let index = m.index;
                self.define_in(index, name, value);
                true
            }
            None => false,
        }
    }

    /// Makes `module` use `used` too, as LilyPond's `ly_use_module` does
    /// with Guile's `module-use!`; false if either is no module.
    pub fn module_use(&mut self, module: Value, used: Value) -> bool {
        let heap = &self.cx.heap;
        match (
            heap.foreign::<ModuleObject>(module),
            heap.foreign::<ModuleObject>(used),
        ) {
            (Some(m), Some(u)) => {
                let (m, u) = (m.index, u.index);
                self.cx.globals.use_module(m, u);
                true
            }
            _ => false,
        }
    }

    /// Defines in `to` each variable that `from` binds itself, with its
    /// value, as LilyPond's `ly_module_copy` does; false if either is no
    /// module.
    pub fn module_copy(&mut self, to: Value, from: Value) -> bool {
        self.cx.module_copy(to, from)
    }

    /// The variables `module` binds itself, as symbols and their values, as
    /// Guile's `module-for-each` visits them; `None` if `module` is no
    /// module.
    pub fn module_locals(&self, module: Value) -> Option<Vec<(Value, Value)>> {
        let index = self.cx.heap.foreign::<ModuleObject>(module)?.index;
        Some(self.cx.globals.locals(index))
    }

    /// Makes `module` current, as `set-current-module` does, and returns
    /// the module that was; nothing changes if `module` is no module.
    pub fn set_current_module(&mut self, module: Value) -> Value {
        if self.cx.heap.foreign::<ModuleObject>(module).is_none() {
            return self
                .cx
                .globals
                .object(self.cx.globals.current_module(&self.cx.heap));
        }
        self.cx.globals.set_current(&mut self.cx.heap, module)
    }

    /// The value of the global `name` in the current module, its own or
    /// imported; `None` if it is unbound.
    pub fn global(&mut self, name: &str) -> Option<Value> {
        let module = self.cx.globals.current_module(&self.cx.heap);
        let symbol = self.cx.heap.intern(name);
        let slot = self.cx.globals.lookup(module, symbol)?;
        let value = self.cx.globals.values[slot as usize];
        (value != Value::UNDEFINED).then_some(value)
    }

    /// Defines the global `name` in the current module.
    pub fn define(&mut self, name: &str, value: Value) {
        let module = self.cx.globals.current_module(&self.cx.heap);
        self.define_in(module, name, value);
    }

    /// Gives the code of the module `name` (its symbols separated by
    /// spaces: `"srfi srfi-1"`), which is loaded when a `use-modules`, an
    /// `@` or a `resolve-module` first asks for it, in place of the file
    /// Guile would load.
    pub fn provide_module(&mut self, name: &str, source: impl Into<String>) {
        let path = name
            .split_whitespace()
            .map(|s| self.cx.heap.intern(s).payload())
            .collect();
        self.cx.globals.provide(path, source.into());
    }

    /// Defines the primitives of another crate (the music model's `ly:`
    /// functions) in the module `module`, named as in [`Vm::provide_module`]
    /// and made if there is none, as the crate's own library modules are,
    /// and exports them; then evaluates `prelude` there. They are numbered after the
    /// primitives defined before them, so a VM numbers them the same only if
    /// it is given the same tables in the same order.
    /// Gives `+`, `-`, `*`, `/` and the comparisons `f` for arguments
    /// that are no numbers, as GOOPS's methods extend Guile's primitive
    /// generics (D70). An image restored keeps it if the extension it is
    /// given has it as [`crate::Extension::operators`].
    pub fn set_operators(&mut self, f: crate::OperatorFn) {
        self.cx.heap.operators = Some(f);
    }

    pub fn define_primitives(
        &mut self,
        module: &str,
        prims: &'static [Prim],
        prelude: &str,
    ) -> Result<(), Exception> {
        let names: Vec<&str> = module.split_whitespace().collect();
        let path: Vec<u32> = names
            .iter()
            .map(|s| self.cx.heap.intern(s).payload())
            .collect();
        let module = match self.cx.globals.find(&path) {
            Some(module) => module,
            None => self.cx.globals.builtin(&mut self.cx.heap, &names),
        };
        for prim in prims {
            let index = PRIMS.len() + self.cx.heap.natives.len();
            self.cx.heap.natives.push(prim);
            self.define_in(
                module,
                prim.name,
                Value::tagged(Tag::Primitive, index as u32),
            );
            // Exported, as `LY_DEFINE` exports LilyPond's from `(lily)`.
            let name = self.cx.heap.intern(prim.name);
            self.cx.globals.export(module, name, name);
        }
        let object = self.cx.globals.object(module);
        let previous = self.cx.globals.set_current(&mut self.cx.heap, object);
        let ran = self.eval_text(prelude, None);
        self.cx.globals.set_current(&mut self.cx.heap, previous);
        ran.map(|_| ())
    }

    /// The slot of the variable `name` in the module `module` (named as in
    /// [`Vm::provide_module`]), its own or imported; `None` if there is no
    /// such module or variable.
    pub fn slot(&mut self, module: &str, name: &str) -> Option<Slot> {
        let module = self.cx.globals.named(&mut self.cx.heap, module)?;
        let symbol = self.cx.heap.intern(name);
        self.cx.globals.lookup(module, symbol).map(Slot)
    }

    /// The slot's value; `None` if it is unbound.
    pub fn slot_value(&self, slot: Slot) -> Option<Value> {
        let value = self.cx.globals.values[slot.0 as usize];
        (value != Value::UNDEFINED).then_some(value)
    }

    /// The slot's version (R10): a `define` or `set!` gives it one no slot
    /// has had, which a redo gives back with the value (D235).
    pub fn slot_version(&self, slot: Slot) -> u32 {
        self.cx.globals.versions[slot.0 as usize]
    }

    /// The module the slot belongs to, as a Scheme value.
    pub fn slot_module(&self, slot: Slot) -> Value {
        let owner = self.cx.globals.owners[slot.0 as usize];
        self.cx.globals.object(owner)
    }

    /// Turns read tracking on or off (R10): while it is on, each slot that
    /// compiled code reads is recorded once, until [`Vm::take_reads`].
    pub fn track_reads(&mut self, on: bool) {
        self.cx.globals.tracking = on;
    }

    /// The slots read while tracking since the last call.
    pub fn take_reads(&mut self) -> Vec<Slot> {
        self.cx
            .globals
            .take_reads()
            .into_iter()
            .map(|(slot, _)| Slot(slot))
            .collect()
    }

    // --- items: the write barrier and the undo log (R10, R11)

    /// Starts a toplevel item (02 §5.7): a new epoch, the write barrier on
    /// in the heap and the slots, and reads tracked, until
    /// [`Vm::end_item`]. Reads before it are not its.
    pub fn begin_item(&mut self) -> Mark {
        self.cx.globals.take_reads();
        self.cx.globals.tracking = true;
        self.cx.globals.begin_item();
        Mark {
            heap: self.cx.heap.begin_item(),
            slots: self.cx.globals.log.len(),
        }
    }

    /// Ends the item begun at `mark`: the barrier off, and what it read and
    /// wrote.
    pub fn end_item(&mut self, mark: Mark) -> Item {
        self.cx.heap.end_item();
        let globals = &mut self.cx.globals;
        globals.logging = false;
        globals.tracking = false;
        let reads = globals
            .take_reads()
            .into_iter()
            .map(|(slot, version)| (Slot(slot), version))
            .collect();
        let writes = globals.log[mark.slots.min(globals.log.len())..]
            .iter()
            .map(|&(slot, _, _)| Slot(slot))
            .collect();
        Item {
            mark,
            reads,
            writes,
        }
    }

    /// Whether an item is running.
    pub fn in_item(&self) -> bool {
        self.cx.heap.in_item()
    }

    /// Undoes what the items since `mark` changed of what is older than
    /// each, the latest change first: every object and slot as it was when
    /// the first of them began. Each slot given back has a new version.
    pub fn undo(&mut self, mark: Mark) {
        self.cx.heap.undo(mark.heap);
        self.cx.globals.undo(mark.slots);
    }

    /// [`Vm::undo`], keeping what each object and slot it gives back is
    /// now (02 §5.7's reuse): undone the latest item first, each kept apart,
    /// the items can be done again the oldest first.
    pub fn undo_keeping(&mut self, mark: Mark) -> Redo {
        Redo {
            heap: self.cx.heap.undo_keeping(mark.heap),
            slots: self.cx.globals.undo_keeping(mark.slots),
        }
    }

    /// Does again what [`Vm::undo_keeping`] kept as `redo`: every object
    /// and slot it changed of what was older than it as it was at its end,
    /// logged as the current item's changes.
    pub fn redo(&mut self, redo: &Redo) {
        self.cx.heap.redo(redo.heap.clone());
        self.cx.globals.redo(redo.slots.clone());
    }

    /// Keeps `value` alive until [`Vm::forget_redo`].
    pub fn keep_for_redo(&mut self, value: Value) {
        self.cx.heap.keep_for_redo(value);
    }

    /// Drops what [`Vm::undo_keeping`] kept: no [`Redo`] is valid after.
    pub fn forget_redo(&mut self) {
        self.cx.heap.forget_redo();
        self.cx.globals.redo.clear();
    }

    /// Whether an object the heap's undo log has logged since it was
    /// `mark` long (the first of [`Vm::undo_len`]) is not what it was then.
    pub fn changed_since(&self, mark: usize) -> bool {
        self.cx.heap.changed_since(mark)
    }

    /// What moves whenever a score's code changes a global: a part's load
    /// is not logged, so does not move it (N8's snapshots, D258).
    pub fn globals_written(&self) -> u64 {
        self.cx.globals.written()
    }

    /// The entries of the undo logs, the heap's and the slots'.
    pub fn undo_len(&self) -> (usize, usize) {
        (self.cx.heap.undo_len(), self.cx.globals.log.len())
    }

    /// Collects now, with the VM's stacks and the modules a name finds as
    /// roots. An anonymous module (a score's, a scope's) is kept while
    /// something reaches it ([`Globals::keep`]), and freed after.
    pub fn collect(&mut self) {
        self.drop_collecting();
        self.begin_collecting();
        self.go_on_collecting(f64::INFINITY);
    }

    /// A collection begun, to go on with [`Vm::go_on_collecting`]: only while
    /// nothing else runs on the heap.
    pub fn begin_collecting(&mut self) {
        let dynamic = self.dynamic_roots();
        let mut live = Vec::new();
        let modules = self.cx.globals.roots(&mut live);
        let globals = &self.cx.globals;
        // The slots the undo log names stay theirs, and what it gives back
        // stays alive.
        let logs = || {
            let log = globals.log.iter().map(|&(slot, value, _)| (slot, value));
            log.chain(
                globals
                    .redo
                    .iter()
                    .map(|&(slot, value, _, _)| (slot, value)),
            )
        };
        let logged: Vec<u32> = logs().map(|(slot, _)| slot).collect();
        let mut kept = globals.keep(&mut live, &[], &logged);
        kept.extend(logs().map(|(_, value)| value));
        self.cx
            .heap
            .begin_collecting(&[&self.stack, &dynamic, &modules, &kept]);
        self.collecting_live = live;
    }

    /// The collection under way goes on until `until`, by the clock: whether
    /// it is over (B20, D261).
    pub fn go_on_collecting(&mut self, until: f64) -> bool {
        let globals = &self.cx.globals;
        let live = &mut self.collecting_live;
        let over = self
            .cx
            .heap
            .go_on_collecting(until, |reached, slots| globals.keep(live, reached, slots));
        if over {
            let live = core::mem::take(&mut self.collecting_live);
            let freed = self.cx.globals.free_dead(&live);
            if !freed.is_empty() {
                self.expander.forget(&freed);
            }
        }
        over
    }

    /// Drops the collection under way, a newer compile having come; one that
    /// has begun to free things ends, with what that frees of the modules.
    pub fn drop_collecting(&mut self) {
        match self.cx.heap.sweeping() {
            true => {
                self.go_on_collecting(f64::INFINITY);
            }
            false => self.cx.heap.drop_collecting(),
        }
    }

    /// Evaluates a datum at toplevel in the current module, as Guile's
    /// `primitive-eval` does.
    pub fn eval(&mut self, form: Value) -> Result<Value, Exception> {
        self.eval_read(form, &[], None)
    }

    /// Evaluates the datum that [`read`] read from `source`, as LilyPond
    /// evaluates a `#` or `$` form of a score: `(*location*)` gives
    /// `location` while it runs, as `scm_c_with_fluid` binds it, and its
    /// syntax errors are placed in `source` by the spans of its pairs.
    pub fn eval_at(
        &mut self,
        read: &Read,
        source: &Source,
        location: Value,
    ) -> Result<Value, Exception> {
        let previous = self.set_location(location);
        let root = self.cx.heap.root_depth();
        self.cx.heap.root(previous);
        let result = self.eval_read(read.datum, &read.spans, Some(source));
        self.cx.heap.unroot_to(root);
        self.set_location(previous);
        result
    }

    /// Evaluates a datum read from `file` with the spans of its pairs, which
    /// place its syntax errors as Guile places them. The forms of a toplevel
    /// `begin`, a macro's included, are expanded, compiled and run one after
    /// the other, so that a `define-module` among them decides the module of
    /// the next.
    fn eval_read(
        &mut self,
        form: Value,
        spans: &[Span],
        place: Option<&Source>,
    ) -> Result<Value, Exception> {
        let module = self.cx.globals.current_module(&self.cx.heap);
        let (form, begin) = match self.expand_toplevel_head(form, module) {
            Ok(top) => top,
            Err(e) => return Err(self.failed(e, spans, place)),
        };
        let heap = &self.cx.heap;
        if begin
            && let Some(forms) = heap.cdr(form).and_then(|f| list(heap, f))
            && !forms.is_empty()
        {
            let root = self.cx.heap.root_depth();
            for &f in &forms {
                self.cx.heap.root(f);
            }
            let mut last = Ok(Value::UNSPECIFIED);
            for f in forms {
                last = self.eval_read(f, spans, place);
                if last.is_err() {
                    break;
                }
            }
            self.cx.heap.unroot_to(root);
            return last;
        }
        match self.compile_form(form, module, false) {
            Ok(code) => {
                let closure = self.cx.heap.closure(code, Default::default());
                self.apply(closure, &[])
            }
            Err(e) => Err(self.failed(e, spans, place)),
        }
    }

    /// The exception of an expansion or a compilation that failed.
    fn failed(&mut self, e: Fail, spans: &[Span], place: Option<&Source>) -> Exception {
        match e {
            Fail::Syntax(e) => {
                self.syntax_error(e, spans, place);
                self.cx.error.take().unwrap_or(Exception {
                    key: Value::FALSE,
                    args: Value::NULL,
                })
            }
            Fail::Thrown(mut e) => {
                let unplaced = core::mem::replace(&mut self.cx.unplaced, Value::FALSE);
                if e.args == unplaced {
                    self.place_thrown(&mut e, spans, place);
                }
                e
            }
        }
    }

    /// A syntax error a transformer threw, `(who what #f form subform)`,
    /// with the place of its subform, else of its form, where `spans` know
    /// it, as psyntax's `syntax-violation` takes it from their source; the
    /// form and subform as data.
    fn place_thrown(&mut self, e: &mut Exception, spans: &[Span], place: Option<&Source>) {
        let heap = &self.cx.heap;
        let mut items = Vec::new();
        let mut at = e.args;
        while let (Some(car), Some(cdr)) = (heap.car(at), heap.cdr(at)) {
            items.push(car);
            at = cdr;
        }
        let [who, what, _, form, subform] = items[..] else {
            return;
        };
        let found = |v: Value| spans.iter().find(|s| s.value == v);
        let span = match subform {
            Value::FALSE => found(form),
            subform => found(subform).or_else(|| found(form)),
        };
        let place = match (span, place) {
            (Some(span), Some(source)) => self.place_alist(source, span.start),
            _ => Value::FALSE,
        };
        let (form, subform) = (self.strip(form), self.strip(subform));
        e.args = self.cx.heap.list(&[who, what, place, form, subform]);
    }

    /// psyntax's place of `start` in `source`: `((line . l) (column . c)
    /// [(filename . f)])`.
    fn place_alist(&mut self, source: &Source, start: usize) -> Value {
        let heap = &mut self.cx.heap;
        let at = source.position(start);
        let line = heap.intern("line");
        let line_no = heap.integer(at.line as i64);
        let column = heap.intern("column");
        let column_no = heap.integer(at.column as i64);
        let mut entries = alloc::vec![heap.cons(line, line_no), heap.cons(column, column_no)];
        if let Some(file) = source.file {
            let filename = heap.intern("filename");
            let name = heap.string(file);
            entries.push(heap.cons(filename, name));
        }
        heap.list(&entries)
    }

    /// Reads and evaluates each datum of `text` in turn, as Guile's
    /// `primitive-load` does; the last one's value. `file` names the text in
    /// read errors.
    pub fn eval_text(&mut self, text: &str, file: Option<&str>) -> Result<Value, Exception> {
        let source = Source::new(text, file);
        let mut at = 0;
        let mut last = Value::UNSPECIFIED;
        let mut scan = LilyScan {
            file: file.map(String::from),
        };
        loop {
            let embedded = match self.cx.lily_braces {
                true => Some(&mut scan as &mut dyn crate::Embedded),
                false => None,
            };
            let r = match read(&mut self.cx.heap, text, at, embedded) {
                Ok(r) => r,
                Err(e) => return Err(self.read_error(&e, &source)),
            };
            if r.datum == Value::EOF {
                return Ok(last);
            }
            at = r.end;
            last = self.eval_read(r.datum, &r.spans, Some(&source))?;
        }
    }

    /// Calls `proc` with `args` and runs it to its end. Its values are
    /// returned as they are: a [`Tag::Values`] object for other than one.
    pub fn apply(&mut self, proc: Value, args: &[Value]) -> Result<Value, Exception> {
        let (stack, frames, base) = (self.stack.len(), self.frames.len(), self.dynamic.len());
        self.cx.applying += 1;
        self.stack.push(proc);
        self.stack.extend_from_slice(args);
        let mut result = match self.call(args.len(), Cont::Exit) {
            Ok(Some(value)) => Ok(value),
            Ok(None) => self.run(),
            Err(thrown) => Err(thrown),
        };
        // A throw that a catch takes goes on at its landing.
        while result.is_err() && self.handle(base) {
            result = self.run();
        }
        self.cx.applying -= 1;
        result.map_err(|Thrown| self.unwound(stack, frames))
    }

    /// The frames from `frames` and the stack from `stack` left, with
    /// [`Vm::apply`]'s caller's state put back; what was thrown.
    fn unwound(&mut self, stack: usize, frames: usize) -> Exception {
        if let Some(exit) = self.frames.drain(frames..).next() {
            self.code = exit.code;
            self.pc = exit.pc as usize;
            self.fp = exit.fp as usize;
        }
        self.stack.truncate(stack);
        self.cx.error.take().unwrap_or(Exception {
            key: Value::FALSE,
            args: Value::NULL,
        })
    }

    /// [`Vm::apply`], which may return to the host in the middle (B23, D261):
    /// `None` when `proc` asked it to ([`Cx::yield_step`]), its frames left on
    /// the VM's stacks, where they are rooted, until [`Vm::go_on_steps`] goes
    /// on with them or [`Vm::abandon_steps`] drops them. A call inside another
    /// is an ordinary one: Rust frames are under it.
    pub fn begin_steps(&mut self, proc: Value, args: &[Value]) -> Result<Option<Value>, Exception> {
        self.begin_steps_with(proc, args, None)
    }

    /// [`Vm::begin_steps`], which also returns to the host at the cancel
    /// check after `due` says the step's time is spent (B23, D272): for
    /// Scheme of LilyPond's own, which asks no `%queue-check`. It returns at
    /// the next primitive's value given outside any dynamic extent.
    pub fn begin_steps_due(
        &mut self,
        proc: Value,
        args: &[Value],
        due: fn() -> bool,
    ) -> Result<Option<Value>, Exception> {
        self.begin_steps_with(proc, args, Some(due))
    }

    fn begin_steps_with(
        &mut self,
        proc: Value,
        args: &[Value],
        due: Option<fn() -> bool>,
    ) -> Result<Option<Value>, Exception> {
        if self.cx.applying != 0 || self.steps.is_some() {
            return self.apply(proc, args).map(Some);
        }
        let steps = Steps {
            stack: self.stack.len(),
            frames: self.frames.len(),
            base: self.dynamic.len(),
            due,
        };
        self.steps = Some(steps);
        self.cx.stepping = true;
        self.cx.applying = 1;
        self.stack.push(proc);
        self.stack.extend_from_slice(args);
        let result = match self.call(args.len(), Cont::Exit) {
            Ok(Some(value)) => Ok(value),
            Ok(None) => self.run(),
            Err(thrown) => Err(thrown),
        };
        self.stepped(steps, result)
    }

    /// The call in steps goes on from where it returned.
    pub fn go_on_steps(&mut self) -> Result<Option<Value>, Exception> {
        let Some(steps) = self.steps else {
            return Ok(None);
        };
        self.cx.applying = 1;
        let result = self.run();
        self.stepped(steps, result)
    }

    /// What a run of the call in steps came to.
    fn stepped(
        &mut self,
        steps: Steps,
        mut result: Result<Value, Thrown>,
    ) -> Result<Option<Value>, Exception> {
        loop {
            if result.is_err() && core::mem::take(&mut self.yielded) {
                self.cx.applying = 0;
                return Ok(None);
            }
            if !(result.is_err() && self.handle(steps.base)) {
                break;
            }
            result = self.run();
        }
        self.cx.applying = 0;
        self.cx.stepping = false;
        self.steps = None;
        result
            .map(Some)
            .map_err(|Thrown| self.unwound(steps.stack, steps.frames))
    }

    /// Whether the call in steps under way asks its step's time and it is
    /// spent.
    pub(crate) fn step_due(&self) -> bool {
        self.steps.and_then(|s| s.due).is_some_and(|due| due())
    }

    /// Whether a call in steps is under way, between its steps.
    pub fn calling(&self) -> bool {
        self.steps.is_some()
    }

    /// Drops the call in steps under way, if there is one: a newer compile
    /// stopped it between its steps.
    pub fn abandon_steps(&mut self) {
        let Some(steps) = self.steps.take() else {
            return;
        };
        self.cx.stepping = false;
        self.dynamic.truncate(steps.base);
        let _ = self.unwound(steps.stack, steps.frames);
    }

    /// Throws a compiler's error as psyntax throws it: `(who what where
    /// form subform)`, `where` the place of the form in `source` when its
    /// span is known.
    #[cold]
    fn syntax_error(&mut self, e: SyntaxError, spans: &[Span], place: Option<&Source>) -> Thrown {
        let span = e.at.and_then(|at| {
            let found = |v: Value| spans.iter().find(|s| s.value == v);
            found(at).or_else(|| found(e.form))
        });
        let place = match (span, place) {
            (Some(span), Some(source)) => self.place_alist(source, span.start),
            _ => Value::FALSE,
        };
        let heap = &mut self.cx.heap;
        let key = heap.intern("syntax-error");
        let who = match e.who {
            Some(who) => heap.intern(who),
            None => Value::FALSE,
        };
        let message = heap.string(e.message);
        let args = heap.list(&[who, message, place, e.form, e.subform]);
        self.cx.error = Some(Exception { key, args });
        Thrown
    }

    /// The exception Guile throws for a read error in `source`, its place in
    /// the message.
    #[cold]
    pub fn read_error(&mut self, e: &ReadError, source: &Source) -> Exception {
        read_exception(&mut self.cx.heap, e, source)
    }

    // --- the loop

    fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    pub(crate) fn pop(&mut self) -> Value {
        self.stack.pop().unwrap_or(Value::UNSPECIFIED)
    }

    fn top(&self) -> Value {
        self.stack.last().copied().unwrap_or(Value::UNSPECIFIED)
    }

    fn unbox(&self, b: Value) -> Value {
        self.cx.heap.car(b).unwrap_or(Value::UNDEFINED)
    }

    fn closure_free(&self, i: u32) -> Value {
        self.cx.heap.closure_free(self.stack[self.fp - 1])[i as usize]
    }

    /// Runs until the frame of [`Vm::apply`]'s call returns.
    fn run(&mut self) -> Result<Value, Thrown> {
        loop {
            let op = self.code.ops[self.pc].get();
            self.pc += 1;
            match op {
                Op::Const(i) => {
                    let v = self.code.consts[i as usize];
                    self.push(v);
                }
                Op::Fixnum(n) => self.push(Value::fixnum(n as i64).unwrap_or(Value::UNSPECIFIED)),
                Op::Null => self.push(Value::NULL),
                Op::False => self.push(Value::FALSE),
                Op::True => self.push(Value::TRUE),
                Op::Unspecified => self.push(Value::UNSPECIFIED),
                Op::Local(i) => {
                    let v = self.stack[self.fp + i as usize];
                    self.push(v);
                }
                Op::LocalBox(i) => {
                    let v = self.unbox(self.stack[self.fp + i as usize]);
                    self.push(v);
                }
                Op::Free(i) => {
                    let v = self.closure_free(i);
                    self.push(v);
                }
                Op::FreeBox(i) => {
                    let v = self.unbox(self.closure_free(i));
                    self.push(v);
                }
                Op::Myself => self.push(self.stack[self.fp - 1]),
                Op::Global(slot) => {
                    let mut v = self.cx.globals.values[slot as usize];
                    if v == Value::UNDEFINED {
                        v = self.resolve(slot)?.1;
                    } else {
                        self.cx.globals.track(slot);
                    }
                    self.push(v);
                }
                Op::SetLocal(i) => {
                    let v = self.pop();
                    self.stack[self.fp + i as usize] = v;
                }
                Op::SetLocalBox(i) => {
                    let v = self.pop();
                    self.cx.heap.set_car(self.stack[self.fp + i as usize], v);
                }
                Op::SetFreeBox(i) => {
                    let v = self.pop();
                    self.cx.heap.set_car(self.closure_free(i), v);
                }
                Op::SetGlobal(mut slot) => {
                    if self.cx.globals.is_reference(slot) {
                        slot = self.resolve(slot)?.0;
                    }
                    let v = self.pop();
                    self.cx.globals.set(slot, v);
                }
                Op::Define(slot) => {
                    let v = self.pop();
                    self.cx.globals.define(slot, v);
                }
                Op::Box(i) => {
                    let at = self.fp + i as usize;
                    self.stack[at] = self.cx.heap.cons(self.stack[at], Value::NULL);
                }
                Op::NewBox(i) => {
                    self.stack[self.fp + i as usize] =
                        self.cx.heap.cons(Value::UNDEFINED, Value::NULL);
                }
                Op::Pop => {
                    self.stack.pop();
                }
                Op::Jump(to) => {
                    // A backward jump is a loop's: count it for the cancel
                    // check.
                    if (to as usize) < self.pc {
                        self.tick()?;
                    }
                    self.pc = to as usize;
                }
                Op::JumpFalse(to) => {
                    if self.pop().is_false() {
                        self.pc = to as usize;
                    }
                }
                Op::AndJump(to) => {
                    if self.top().is_false() {
                        self.pc = to as usize;
                    } else {
                        self.stack.pop();
                    }
                }
                Op::OrJump(to) => {
                    if self.top().is_false() {
                        self.stack.pop();
                    } else {
                        self.pc = to as usize;
                    }
                }
                Op::Closure(i) => {
                    let child = self.code.children[i as usize].clone();
                    let at = self.stack.len() - child.free as usize;
                    let free = self.stack[at..].into();
                    self.stack.truncate(at);
                    let closure = self.cx.heap.closure(child, free);
                    self.push(closure);
                }
                Op::Call(argc) => {
                    self.call(argc as usize, Cont::Single)?;
                }
                Op::CallMulti(argc) => {
                    self.call(argc as usize, Cont::Multi)?;
                }
                Op::TailCall(argc) => {
                    if let Some(v) = self.call(argc as usize, Cont::Tail)? {
                        return Ok(v);
                    }
                }
                Op::CallValues | Op::TailCallValues => {
                    let values = self.pop();
                    let argc = if values.tag() == Tag::Values {
                        let items = self.cx.heap.values_items(values);
                        self.stack.extend_from_slice(items);
                        items.len()
                    } else {
                        self.push(values);
                        1
                    };
                    let cont = match op {
                        Op::CallValues => Cont::Single,
                        _ => Cont::Tail,
                    };
                    if let Some(v) = self.call(argc, cont)? {
                        return Ok(v);
                    }
                }
                Op::Return => {
                    let v = self.pop();
                    if let Some(v) = self.ret(v)? {
                        return Ok(v);
                    }
                }
                Op::Inline(op, slot, direct) => self.inline(op, slot, direct)?,
                Op::Direct => self.cx.direct = true,
                Op::PushDyn(kind, landing) => self.push_dyn(kind, landing),
                Op::PopDyn => {
                    self.pop_dyn();
                }
                Op::Pending => {
                    let v = core::mem::replace(&mut self.pending, Value::NULL);
                    self.push(v);
                }
                Op::Memv(i) => {
                    let key = self.pop();
                    let mut list = self.code.consts[i as usize];
                    let mut found = false;
                    while let (Some(car), Some(cdr)) =
                        (self.cx.heap.car(list), self.cx.heap.cdr(list))
                    {
                        if self.cx.heap.eqv(car, key) {
                            found = true;
                            break;
                        }
                        list = cdr;
                    }
                    self.push(Value::bool(found));
                }
            }
        }
    }

    /// Calls the procedure below the top `argc` values. `Ok(Some(value))`
    /// when the call returned to [`Vm::apply`].
    fn call(&mut self, mut argc: usize, cont: Cont) -> Result<Option<Value>, Thrown> {
        // Whether the compiler marked this call as a direct one.
        let direct = core::mem::take(&mut self.cx.direct);
        self.applications += 1;
        loop {
            let at = self.stack.len() - argc - 1;
            let proc = self.stack[at];
            match proc.tag() {
                Tag::Closure => {
                    self.enter(proc, at, argc, cont)?;
                    return Ok(None);
                }
                Tag::Primitive if proc.payload() <= prims::GC => match proc.payload() {
                    prims::APPLY => argc = self.spread(at, argc)?,
                    prims::GC => {
                        if argc != 0 {
                            return Err(self.wrong_args(proc));
                        }
                        self.collect();
                        self.stack.truncate(at);
                        return self.deliver(Value::UNSPECIFIED, cont);
                    }
                    _ => argc = self.compile_call(proc, at, argc)?,
                },
                Tag::Primitive => {
                    let Some(prim) = prims::get(&self.cx.heap, proc.payload()) else {
                        return Err(self.cx.raise(&WRONG_TYPE_TO_APPLY, None, &[proc]));
                    };
                    if argc < prim.min as usize || (prim.max != REST && argc > prim.max as usize) {
                        return Err(self.wrong_args(proc));
                    }
                    self.cx.direct = direct;
                    let value = (prim.f)(&mut self.cx, &self.stack[at + 1..]);
                    self.cx.direct = false;
                    let tail = self.cx.tail.take();
                    let mut value = value?;
                    if let Some(host) = self.cx.hosting.take() {
                        value = self.host_call(host, at)?;
                    }
                    self.stack.truncate(at);
                    if let Some((proc, args)) = tail {
                        self.stack.push(proc);
                        self.stack.extend_from_slice(&args);
                        argc = args.len();
                        continue;
                    }
                    let delivered = self.deliver(value, cont)?;
                    // Its value given: the next `run` goes on from the next op.
                    if core::mem::take(&mut self.cx.yielding)
                        && delivered.is_none()
                        && self.steps.is_some_and(|s| s.base == self.dynamic.len())
                    {
                        self.yielded = true;
                        return Err(Thrown);
                    }
                    return Ok(delivered);
                }
                // A parameter, or a procedure with a setter, applies as its
                // procedure.
                Tag::Foreign => match control::applicable(&self.cx.heap, proc) {
                    Some(inner) => self.stack[at] = inner,
                    // An applicable smob's procedure, given the smob first.
                    None => match self
                        .cx
                        .heap
                        .foreign_object(proc)
                        .and_then(|f| f.applicable())
                    {
                        Some(inner) => {
                            self.stack[at] = inner;
                            self.stack.insert(at + 1, proc);
                            argc += 1;
                        }
                        None => return Err(self.cx.raise(&WRONG_TYPE_TO_APPLY, None, &[proc])),
                    },
                },
                _ => {
                    return Err(self.cx.raise(&WRONG_TYPE_TO_APPLY, None, &[proc]));
                }
            }
        }
    }

    /// `(eval form module)` or `(primitive-eval form)`: compiles the form
    /// in the module, or the current one, and calls its procedure in place
    /// of the call: `eval`'s through `(%in-module module procedure)`, which
    /// makes the module current while it runs.
    fn compile_call(&mut self, proc: Value, at: usize, argc: usize) -> Result<usize, Thrown> {
        let eval = proc.payload() == prims::EVAL;
        if argc != if eval { 2 } else { 1 } {
            return Err(self.wrong_args(proc));
        }
        let module = match eval {
            true => {
                let object = self.stack[at + 2];
                match self.cx.heap.foreign::<ModuleObject>(object) {
                    Some(m) => m.index,
                    None => return Err(self.cx.wrong_type("eval", 2, object, "module")),
                }
            }
            false => self.cx.globals.current_module(&self.cx.heap),
        };
        let form = self.stack[at + 1];
        match self.compile_form(form, module, false) {
            Ok(code) => {
                self.stack.truncate(at);
                let closure = self.cx.heap.closure(code, Default::default());
                if !eval {
                    self.push(closure);
                    return Ok(0);
                }
                self.push(self.cx.globals.values[self.in_module as usize]);
                self.push(self.cx.globals.object(module));
                self.push(closure);
                Ok(2)
            }
            Err(Fail::Syntax(e)) => Err(self.syntax_error(e, &[], None)),
            Err(Fail::Thrown(e)) => {
                self.cx.error = Some(e);
                Err(Thrown)
            }
        }
    }

    /// `%host-call`'s `host`, its handler or the host, called with the VM
    /// and the arguments above `at`, which stay on the stack, rooted, while
    /// it runs. A run it leaves waiting for the host ([`Vm::waiting`])
    /// cancels this one too.
    #[cold]
    fn host_call(&mut self, host: crate::lily::HostFn, at: usize) -> Result<Value, Thrown> {
        let args = self.stack[at + 1..].to_vec();
        let result = host(self, &args);
        if self.cx.waiting {
            self.cx.cancelled = true;
            return Err(Thrown);
        }
        result.map_err(|e| {
            self.cx.error = Some(e);
            Thrown
        })
    }

    /// Gives a primitive's value to the continuation.
    fn deliver(&mut self, value: Value, cont: Cont) -> Result<Option<Value>, Thrown> {
        match cont {
            Cont::Single => {
                let v = self.single(value)?;
                self.push(v);
                Ok(None)
            }
            Cont::Multi => {
                self.push(value);
                Ok(None)
            }
            Cont::Exit => Ok(Some(value)),
            Cont::Tail => self.ret(value),
        }
    }

    /// Enters a closure: checks its arguments, saves the caller unless the
    /// call is in tail position, makes the frame and reaches a safe point.
    fn enter(&mut self, proc: Value, at: usize, argc: usize, cont: Cont) -> Result<(), Thrown> {
        self.tick()?;
        let Some(code) = self.cx.heap.closure_code(proc).cloned() else {
            return Err(self.wrong_args(proc));
        };
        let required = code.required as usize;
        if argc != required && !(code.rest && argc > required) {
            return Err(self.wrong_args(proc));
        }
        let frame = code.frame as usize;
        let at = if cont == Cont::Tail {
            let base = self.fp - 1;
            self.stack.copy_within(at..=at + argc, base);
            self.stack.truncate(base + argc + 1);
            self.code = code;
            base
        } else {
            if self.frames.len() >= MAX_FRAMES {
                return Err(self.stack_overflow());
            }
            let caller = core::mem::replace(&mut self.code, code);
            self.frames.push(Frame {
                code: caller,
                pc: self.pc as u32,
                fp: self.fp as u32,
                cont,
            });
            at
        };
        self.fp = at + 1;
        self.pc = 0;
        if self.code.rest {
            let start = self.fp + required;
            let mut rest = Value::NULL;
            for i in (start..self.stack.len()).rev() {
                rest = self.cx.heap.cons(self.stack[i], rest);
            }
            self.stack.truncate(start);
            self.push(rest);
        }
        if self.fp + frame > MAX_STACK {
            return Err(self.stack_overflow());
        }
        self.stack.resize(self.fp + frame, Value::UNSPECIFIED);
        if self.cx.heap.collection_due() || self.cx.heap.collection_forced() {
            self.collect();
        }
        Ok(())
    }

    /// Returns from the running procedure. `Ok(Some(value))` when it was
    /// called by [`Vm::apply`].
    fn ret(&mut self, value: Value) -> Result<Option<Value>, Thrown> {
        let Some(frame) = self.frames.pop() else {
            return Ok(Some(value));
        };
        self.stack.truncate(self.fp - 1);
        self.code = frame.code;
        self.pc = frame.pc as usize;
        self.fp = frame.fp as usize;
        match frame.cont {
            Cont::Exit => Ok(Some(value)),
            cont => self.deliver(value, cont),
        }
    }

    /// One value for a continuation that takes one.
    fn single(&mut self, value: Value) -> Result<Value, Thrown> {
        if value.tag() != Tag::Values {
            return Ok(value);
        }
        match self.cx.heap.values_items(value).first() {
            Some(&first) => Ok(first),
            None => Err(self.cx.raise(&ZERO_VALUES, None, &[])),
        }
    }

    /// `(apply f a … list)`: the arguments spread on the stack in place of
    /// `apply`'s; how many there are now.
    fn spread(&mut self, at: usize, argc: usize) -> Result<usize, Thrown> {
        if argc < 2 {
            return Err(self.wrong_args(self.stack[at]));
        }
        let list = self.pop();
        self.stack.remove(at);
        let mut argc = argc - 2;
        let mut rest = list;
        while let (Some(car), Some(cdr)) = (self.cx.heap.car(rest), self.cx.heap.cdr(rest)) {
            self.stack.push(car);
            argc += 1;
            rest = cdr;
        }
        if rest != Value::NULL {
            return Err(self.cx.raise(&APPLY_TO_NON_LIST, Some("apply"), &[list]));
        }
        Ok(argc)
    }

    /// An [`Inline`] op: inline while its global holds the primitive and
    /// the arguments are fixnums or pairs, else the primitive, or a call of
    /// whatever the global holds now.
    fn inline(&mut self, op: Inline, slot: u32, direct: bool) -> Result<(), Thrown> {
        let argc = op.arity();
        let mut proc = self.cx.globals.values[slot as usize];
        if proc == Value::UNDEFINED {
            proc = self.resolve(slot)?.1;
        } else {
            self.cx.globals.track(slot);
        }
        if proc != self.inline[op as usize] {
            let at = self.stack.len() - argc;
            self.stack.insert(at, proc);
            self.cx.direct = direct;
            self.call(argc, Cont::Single)?;
            return Ok(());
        }
        let n = self.stack.len();
        let a = self.stack[n - argc];
        let b = self.top();
        let fixnums = a.as_fixnum().zip(b.as_fixnum());
        let value = match op {
            Inline::Add => fixnums.and_then(|(x, y)| Value::fixnum(x + y)),
            Inline::Sub => fixnums.and_then(|(x, y)| Value::fixnum(x - y)),
            Inline::Mul => fixnums.and_then(|(x, y)| Value::fixnum(x.checked_mul(y)?)),
            Inline::NumEq => fixnums.map(|(x, y)| Value::bool(x == y)),
            Inline::Lt => fixnums.map(|(x, y)| Value::bool(x < y)),
            Inline::Gt => fixnums.map(|(x, y)| Value::bool(x > y)),
            Inline::Le => fixnums.map(|(x, y)| Value::bool(x <= y)),
            Inline::Ge => fixnums.map(|(x, y)| Value::bool(x >= y)),
            Inline::Car => self.cx.heap.car(a),
            Inline::Cdr => self.cx.heap.cdr(a),
            Inline::Cons => Some(self.cx.heap.cons(a, b)),
            Inline::IsNull => Some(Value::bool(a == Value::NULL)),
            Inline::IsPair => Some(Value::bool(a.tag() == Tag::Pair)),
            Inline::Not => Some(Value::bool(a.is_false())),
            Inline::IsEq => Some(Value::bool(a == b)),
            Inline::IsZero => a.as_fixnum().map(|x| Value::bool(x == 0)),
        };
        let at = n - argc;
        let value = match value {
            Some(v) => v,
            None => {
                let index = self.inline[op as usize].payload() as usize;
                self.cx.direct = direct;
                let value = (PRIMS[index].f)(&mut self.cx, &self.stack[at..]);
                self.cx.direct = false;
                value?
            }
        };
        self.stack.truncate(at);
        self.push(value);
        Ok(())
    }

    // --- errors

    /// A slot with no value read or set by the instruction before `pc`:
    /// if it is a cell for an import, the imported slot and its value, and
    /// the instruction now names that slot; else `Unbound variable`.
    #[cold]
    fn resolve(&mut self, slot: u32) -> Result<(u32, Value), Thrown> {
        let Some(found) = self.cx.globals.resolve(slot) else {
            return Err(self.unbound(slot));
        };
        let cell = &self.code.ops[self.pc - 1];
        cell.set(match cell.get() {
            Op::Global(_) => Op::Global(found),
            Op::SetGlobal(_) => Op::SetGlobal(found),
            Op::Inline(op, _, direct) => Op::Inline(op, found, direct),
            other => other,
        });
        self.cx.globals.track(found);
        Ok((found, self.cx.globals.values[found as usize]))
    }

    #[cold]
    fn unbound(&mut self, slot: u32) -> Thrown {
        let name = self.cx.globals.names[slot as usize];
        self.cx.raise(&UNBOUND_VARIABLE, None, &[name])
    }

    #[cold]
    fn wrong_args(&mut self, proc: Value) -> Thrown {
        self.cx.direct = false;
        self.cx.raise(&WRONG_NUMBER_OF_ARGS, None, &[proc])
    }

    #[cold]
    fn stack_overflow(&mut self) -> Thrown {
        self.cx.raise(&STACK_OVERFLOW, None, &[])
    }
}

/// The items of a proper list.
fn list(heap: &Heap, list: Value) -> Option<Vec<Value>> {
    let mut items = Vec::new();
    let mut at = list;
    while let (Some(car), Some(cdr)) = (heap.car(at), heap.cdr(at)) {
        items.push(car);
        at = cdr;
    }
    (at == Value::NULL).then_some(items)
}

/// A read error as Guile throws it, the place in its message.
#[cold]
pub(crate) fn read_exception(heap: &mut Heap, e: &ReadError, source: &Source) -> Exception {
    let mut message = String::new();
    if let Some(at) = e.at {
        let place = source.position(at);
        let file = source.file.unwrap_or("#<unknown port>");
        let _ = write!(message, "{file}:{}:{}: ", place.line + 1, place.column + 1);
    }
    message.push_str(e.message);
    let key = heap.intern(e.key);
    let subr = match e.subr {
        Some(subr) => heap.string(subr),
        None => Value::FALSE,
    };
    let message = heap.string(message);
    let args = heap.list(&e.args);
    let args = heap.list(&[subr, message, args, Value::FALSE]);
    Exception { key, args }
}
