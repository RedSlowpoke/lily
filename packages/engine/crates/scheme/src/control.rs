//! Escapes (chapter 03 §5.3, R9, R14): `catch` and `throw` with Guile's keys
//! and arguments, `call-with-escape-continuation`, `dynamic-wind`, fluids
//! and parameters, `*location*` (whose fluid is `(lily)`'s, `module.rs`), and
//! the cancel check. Full `call/cc` is not provided (03 §2.6).
//!
//! All of them unwind the VM's own stacks. A `catch`, an escape's prompt, a
//! `dynamic-wind` and a fluid's binding each push an entry on the dynamic
//! stack ([`Dyn`]) while their body runs: the compiler's internal form
//! `(%dyn kind data extra body handler)` pushes it, runs `body`, pops it, and
//! has a landing that calls `handler`. An entry with a landing records where
//! the stacks were, so a throw cuts them back to it and goes on at the
//! landing, which calls the handler with what the throw carried ([`Signal`]).
//! On the way, a `dynamic-wind`'s entry lands first: its landing calls the
//! `after` thunk and resumes the throw, so `after` runs as Scheme in the loop,
//! outside the wind's extent, as Guile runs it. A fluid's entry restores the
//! fluid's outer value.
//!
//! Fluids are shallow-bound: a fluid holds its current value, and its entry
//! the value it had outside. A parameter is a fluid, a converter and the
//! procedure that reads and sets it.
//!
//! The cancel check (R14, B8): every [`CANCEL_INTERVAL`] calls and backward
//! jumps, the loop asks the host's check whether to stop. A cancelled run
//! unwinds to its caller past every `catch` without running `after` thunks
//! (fluids are restored), and the VM stays usable.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::compile::Template;
use crate::errors::{
    ABORT_TO_UNKNOWN_PROMPT, CANCELLED, NOT_A_PARAMETER, WRONG_TYPE_PLAIN, WRONG_TYPE_SCHEME,
};
use crate::heap::{Foreign, Heap, Tracer};
use crate::image::{ImageReader, ImageWriter};
use crate::value::{Tag, Value};
use crate::vm::{Cx, Exception, Thrown, Vm};

/// How many calls and backward jumps run between two cancel checks.
pub const CANCEL_INTERVAL: u32 = 4096;

/// How many times [`Vm::short_of_memory`] is asked waiting for a collection.
const PRESSED: u32 = 64;

/// The host's cancel check: true to stop.
pub type CancelCheck = Box<dyn FnMut() -> bool>;

// --- fluids and parameters --------------------------------------------------------

/// A fluid: its value in the current dynamic extent.
#[derive(Clone)]
pub(crate) struct Fluid {
    pub value: Value,
}

impl Foreign for Fluid {
    fn type_name(&self) -> &'static str {
        "fluid"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.value);
    }

    fn snapshot(&self) -> Option<Box<dyn Foreign>> {
        Some(Box::new(self.clone()))
    }

    fn same(&self, other: &dyn Foreign) -> bool {
        (other as &dyn core::any::Any)
            .downcast_ref::<Fluid>()
            .is_some_and(|other| other.value == self.value)
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.value(self.value);
        true
    }
}

pub(crate) fn load_fluid(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    Some(Box::new(Fluid { value: r.value()? }))
}

/// A parameter: Guile's `<parameter>`, which applies as its procedure.
pub(crate) struct Parameter {
    pub fluid: Value,
    pub converter: Value,
    pub procedure: Value,
}

impl Foreign for Parameter {
    fn type_name(&self) -> &'static str {
        "<parameter>"
    }

    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<<parameter> proc: ");
        heap.write(self.procedure, out);
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.fluid);
        tracer.trace(self.converter);
        tracer.trace(self.procedure);
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.value(self.fluid);
        out.value(self.converter);
        out.value(self.procedure);
        true
    }
}

pub(crate) fn load_parameter(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    Some(Box::new(Parameter {
        fluid: r.value()?,
        converter: r.value()?,
        procedure: r.value()?,
    }))
}

// --- promises ------------------------------------------------------------------------

/// Guile 3's promise (`promises.c`, MS3 step 64): its thunk until it is
/// forced, then its value. `delay` and `force` are [`PRELUDE`]'s.
#[derive(Clone)]
pub(crate) struct Promise {
    computed: bool,
    data: Value,
}

impl Foreign for Promise {
    fn type_name(&self) -> &'static str {
        "promise"
    }

    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<promise ");
        heap.write(self.data, out);
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.data);
    }

    fn snapshot(&self) -> Option<Box<dyn Foreign>> {
        Some(Box::new(self.clone()))
    }
}

/// `(make-promise thunk)`: a promise of `thunk`, a procedure.
pub(crate) fn make_promise(cx: &mut Cx, args: &[Value]) -> R {
    if !cx.heap.is_procedure(args[0]) {
        return Err(cx.wrong_type_at("make-promise", 1, args[0]));
    }
    Ok(cx.heap.wrap(Promise {
        computed: false,
        data: args[0],
    }))
}

pub(crate) fn is_promise(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(cx.heap.foreign::<Promise>(args[0]).is_some()))
}

/// `(%promise-data p)`: the promise's thunk or value; `force`'s type error
/// for what is no promise.
pub(crate) fn promise_data(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.foreign::<Promise>(args[0]) {
        Some(p) => Ok(p.data),
        None => Err(cx.wrong_type_at("force", 1, args[0])),
    }
}

/// `(%promise-computed? p)`: whether the promise has its value.
pub(crate) fn promise_computed(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        cx.heap
            .foreign::<Promise>(args[0])
            .is_some_and(|p| p.computed),
    ))
}

/// `(%promise-set! p value)`: the promise's value, once computed.
pub(crate) fn promise_set(cx: &mut Cx, args: &[Value]) -> R {
    if let Some(p) = cx.heap.foreign_mut::<Promise>(args[0]) {
        p.computed = true;
        p.data = args[1];
    }
    Ok(Value::UNSPECIFIED)
}

// --- the dynamic stack ---------------------------------------------------------------

/// What an entry of the dynamic stack is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DynKind {
    /// A `catch` of a key, or of every key (`#t`).
    Catch,
    /// An escape procedure's prompt, found by its tag.
    Prompt,
    /// A `dynamic-wind`, whose landing runs its `after` thunk.
    Wind,
    /// A fluid's binding, which restores the outer value.
    Fluid,
}

/// An entry of the dynamic stack, and where its landing goes on.
pub(crate) struct Dyn {
    pub kind: DynKind,
    /// The catch's key, the prompt's tag, the wind's `after`, the fluid.
    pub data: Value,
    /// The fluid's outer value.
    pub saved: Value,
    pub code: Rc<Template>,
    pub landing: u32,
    pub fp: u32,
    pub frames: u32,
    pub stack: u32,
}

/// What is unwinding the stacks.
pub(crate) enum Signal {
    Throw(Exception),
    /// An escape procedure's call: its prompt's tag and the values.
    Escape(Value, Value),
    Cancel,
}

impl Vm {
    /// `PushDyn`: the entry for a `%dyn` whose landing is at `landing`, with
    /// `data` and `extra` (a fluid's new value) from the stack.
    pub(crate) fn push_dyn(&mut self, kind: DynKind, landing: u32) {
        let extra = self.pop();
        let data = self.pop();
        let mut saved = Value::UNSPECIFIED;
        if kind == DynKind::Fluid
            && let Some(fluid) = self.cx.heap.foreign_mut::<Fluid>(data)
        {
            saved = core::mem::replace(&mut fluid.value, extra);
        }
        self.dynamic.push(Dyn {
            kind,
            data,
            saved,
            code: self.code.clone(),
            landing,
            fp: self.fp as u32,
            frames: self.frames.len() as u32,
            stack: self.stack.len() as u32,
        });
    }

    /// `PopDyn`, and each entry a throw passes: a fluid gets its outer value.
    pub(crate) fn pop_dyn(&mut self) -> Option<Dyn> {
        let entry = self.dynamic.pop()?;
        if entry.kind == DynKind::Fluid
            && let Some(fluid) = self.cx.heap.foreign_mut::<Fluid>(entry.data)
        {
            fluid.value = entry.saved;
        }
        Some(entry)
    }

    /// The values the dynamic stack holds, for the collector.
    pub(crate) fn dynamic_roots(&self) -> Vec<Value> {
        let mut roots = Vec::with_capacity(self.dynamic.len() * 2 + 2);
        for entry in &self.dynamic {
            roots.push(entry.data);
            roots.push(entry.saved);
        }
        roots.push(self.pending);
        roots
    }

    /// After a throw: unwinds to the landing that takes it, true, or, when
    /// nothing above `base` takes it, to `base`, false, and the signal is
    /// left in [`Cx::error`] for the caller.
    pub(crate) fn handle(&mut self, base: usize) -> bool {
        let signal = match self.cx.signal() {
            Some(signal) => signal,
            None => return false,
        };
        let above = &self.dynamic[base.min(self.dynamic.len())..];
        let target = match &signal {
            Signal::Cancel => None,
            Signal::Throw(e) => above.iter().rposition(|d| {
                d.kind == DynKind::Catch && (d.data == Value::TRUE || d.data == e.key)
            }),
            Signal::Escape(tag, _) => above
                .iter()
                .rposition(|d| d.kind == DynKind::Prompt && d.data == *tag),
        }
        .map(|i| base + i);
        // An escape whose prompt is gone is Guile's `abort` error.
        if let (Signal::Escape(tag, _), None) = (&signal, target) {
            let tag = self.cx.heap.list(&[*tag]);
            self.cx
                .raise(&ABORT_TO_UNKNOWN_PROMPT, Some("abort"), &[tag]);
            return self.handle(base);
        }
        let floor = target.map_or(base, |t| t + 1);
        let wind = match signal {
            Signal::Cancel => None,
            _ => self.dynamic[floor.min(self.dynamic.len())..]
                .iter()
                .rposition(|d| d.kind == DynKind::Wind)
                .map(|i| floor + i),
        };
        let Some(to) = wind.or(target) else {
            while self.dynamic.len() > base {
                self.pop_dyn();
            }
            self.cx.restore(signal);
            return false;
        };
        while self.dynamic.len() > to + 1 {
            self.pop_dyn();
        }
        let Some(entry) = self.pop_dyn() else {
            return false;
        };
        let heap = &mut self.cx.heap;
        self.pending = match (entry.kind, signal) {
            // The wind's handler resumes the throw with this token.
            (DynKind::Wind, Signal::Throw(e)) => {
                let token = heap.cons(e.key, e.args);
                let token = heap.cons(Value::TRUE, token);
                heap.list(&[token])
            }
            (DynKind::Wind, Signal::Escape(tag, values)) => {
                let token = heap.cons(tag, values);
                let token = heap.cons(Value::FALSE, token);
                heap.list(&[token])
            }
            // The catch's handler takes the key and the arguments.
            (_, Signal::Throw(e)) => heap.cons(e.key, e.args),
            // The prompt's handler takes the values.
            (_, Signal::Escape(_, values)) => values,
            (_, Signal::Cancel) => Value::NULL,
        };
        self.frames.truncate(entry.frames as usize);
        self.stack.truncate(entry.stack as usize);
        self.code = entry.code;
        self.fp = entry.fp as usize;
        self.pc = entry.landing as usize;
        true
    }

    /// Counts a call or a backward jump, and every [`CANCEL_INTERVAL`] asks
    /// the host's check.
    #[inline]
    pub(crate) fn tick(&mut self) -> Result<(), Thrown> {
        self.ticks -= 1;
        if self.ticks == 0 {
            return self.check_cancel();
        }
        Ok(())
    }

    #[cold]
    fn check_cancel(&mut self) -> Result<(), Thrown> {
        self.ticks = CANCEL_INTERVAL;
        if self.short_of_memory() {
            // As a run cancelled for the host: past every `catch`, and
            // whatever Rust called it ends as it does for that.
            self.heap_mut().over_memory();
            self.cx.waiting = true;
            self.cx.cancelled = true;
            return Err(Thrown);
        }
        let stop = self.cancel.as_mut().is_some_and(|check| check());
        if stop {
            self.cx.cancelled = true;
            return Err(Thrown);
        }
        // A call in steps whose step's time is spent returns at the next
        // primitive's value (B23, D272).
        if self.step_due() {
            self.cx.yield_step();
        }
        Ok(())
    }

    /// Sets the host's cancel check, or none.
    pub fn set_cancel(&mut self, check: Option<CancelCheck>) {
        self.cancel = check;
    }

    /// Sets what says the memory left is too little to go on (D259), or
    /// none. An allocation that fails cannot be come back from in wasm, so a
    /// run is cancelled before one does.
    pub fn set_room(&mut self, short: Option<fn(usize) -> bool>) {
        self.short = short;
    }

    /// Whether the memory left is too little to go on: still short after a
    /// collection, which the first time asks for, or after [`PRESSED`] asks
    /// with none, which comes only at a safe point.
    pub fn short_of_memory(&mut self) -> bool {
        if !self.short.is_some_and(|short| short(0)) {
            self.pressed = None;
            return false;
        }
        let collections = self.heap().stats().collections;
        match &mut self.pressed {
            Some((at, asked)) if *at == collections && *asked < PRESSED => {
                *asked += 1;
                false
            }
            Some(_) => {
                self.pressed = None;
                true
            }
            None => {
                self.pressed = Some((collections, 0));
                self.heap_mut().collect_soon();
                false
            }
        }
    }

    /// A compile stops for want of memory (D259), at its next return.
    pub fn over_memory(&mut self) {
        self.heap_mut().over_memory();
    }

    /// Whether a compile was to stop for want of memory since the last call.
    pub fn take_over_memory(&mut self) -> bool {
        self.heap_mut().take_over_memory()
    }

    /// Sets where the text of a part's file comes from (D74), or none: a
    /// part's stub that is called asks it for each of the part's files by
    /// name (`display-lily.scm`), and fails if it has none, or waits for
    /// the host if it must be asked ([`Vm::waiting`]).
    pub fn set_part_source(&mut self, source: Option<crate::lily::PartSource>) {
        self.cx.part_source = source;
    }

    /// Sets where the files `\include` reads in a parse Scheme runs come
    /// from, a template's (D92, MS6 step 3), or none.
    pub fn set_file_source(&mut self, source: Option<crate::lily::FileSource>) {
        self.cx.file_source = source;
    }

    /// What the file source has of the file `name`; None without one.
    pub fn host_file(&mut self, name: &str) -> Option<crate::Hosted<alloc::vec::Vec<u8>>> {
        self.cx.file_source.as_mut().map(|source| source(name))
    }

    /// Whether there is a file source.
    pub fn has_file_source(&self) -> bool {
        self.cx.file_source.is_some()
    }

    /// Leaves the VM waiting for the host, as a part's file it must be
    /// asked for does: the run of the `%host-call` that called the host is
    /// cancelled when it returns.
    pub fn wait_for_host(&mut self) {
        self.cx.waiting = true;
    }

    /// Sets the host's procedure that `%host-call` calls, or none (MS2
    /// step 33, D92): the parser's, for LilyPond's `#{ … #}`.
    pub fn set_host(&mut self, host: Option<crate::HostFn>) {
        self.cx.host = host;
    }

    /// Adds `handlers` for `%host-call`, each for its `what` before the
    /// host and before a handler added earlier for the same `what` (D127).
    /// Not in the image: [`crate::Extension::hosts`] gives them again.
    pub fn add_host_handlers(&mut self, handlers: &[crate::HostHandler]) {
        for &h in handlers {
            self.cx.handlers.retain(|old| old.what != h.what);
            self.cx.handlers.push(h);
        }
    }

    /// Sets the value of `(*location*)` (R9), as the parser does around each
    /// `#` form; the value it had. It sets the fluid that `(lily)`'s
    /// `%location` holds now, which is `lily.scm`'s own once that has loaded.
    pub fn set_location(&mut self, location: Value) -> Value {
        let fluid = self.cx.globals.values[self.location as usize];
        match self.cx.heap.foreign_mut::<Fluid>(fluid) {
            Some(fluid) => core::mem::replace(&mut fluid.value, location),
            None => Value::FALSE,
        }
    }

    /// The value of `(*location*)`.
    pub fn location(&self) -> Value {
        let fluid = self.cx.globals.values[self.location as usize];
        self.cx
            .heap
            .foreign::<Fluid>(fluid)
            .map_or(Value::FALSE, |f| f.value)
    }
}

impl Cx {
    /// Takes the pending throw, escape or cancellation.
    fn signal(&mut self) -> Option<Signal> {
        if core::mem::take(&mut self.cancelled) {
            self.escape = None;
            self.error = None;
            return Some(Signal::Cancel);
        }
        if let Some((tag, values)) = self.escape.take() {
            return Some(Signal::Escape(tag, values));
        }
        self.error.take().map(Signal::Throw)
    }

    /// Leaves a signal nothing took for the caller of [`Vm::apply`].
    fn restore(&mut self, signal: Signal) {
        match signal {
            Signal::Throw(e) => self.error = Some(e),
            Signal::Cancel => {
                self.raise(&CANCELLED, None, &[]);
            }
            Signal::Escape(..) => {}
        }
    }
}

// --- primitives ------------------------------------------------------------------------

type R = Result<Value, Thrown>;

/// `(throw key . args)`.
pub(crate) fn throw(cx: &mut Cx, args: &[Value]) -> R {
    let key = args[0];
    if key.tag() != Tag::Symbol {
        let position = cx.heap.integer(1);
        return Err(cx.raise(&WRONG_TYPE_SCHEME, Some("throw"), &[position, key]));
    }
    let args = cx.heap.list(&args[1..]);
    cx.error = Some(Exception { key, args });
    Err(Thrown)
}

/// `(scm-error key subr message args rest)`: Guile's throw of an error.
pub(crate) fn scm_error(cx: &mut Cx, args: &[Value]) -> R {
    throw(cx, args)
}

/// `(%abort tag values)`: an escape procedure's call.
pub(crate) fn abort(cx: &mut Cx, args: &[Value]) -> R {
    cx.escape = Some((args[0], args[1]));
    Err(Thrown)
}

/// `(%resume token)`: a `dynamic-wind`'s landing goes on with the throw or
/// escape it stopped.
pub(crate) fn resume(cx: &mut Cx, args: &[Value]) -> R {
    let token = args[0];
    let (Some(throw), Some(rest)) = (cx.heap.car(token), cx.heap.cdr(token)) else {
        return Err(Thrown);
    };
    let (Some(key), Some(args)) = (cx.heap.car(rest), cx.heap.cdr(rest)) else {
        return Err(Thrown);
    };
    if throw == Value::TRUE {
        cx.error = Some(Exception { key, args });
    } else {
        cx.escape = Some((key, args));
    }
    Err(Thrown)
}

pub(crate) fn make_fluid(cx: &mut Cx, args: &[Value]) -> R {
    let value = args.first().copied().unwrap_or(Value::FALSE);
    Ok(cx.heap.wrap(Fluid { value }))
}

pub(crate) fn is_fluid(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(cx.heap.foreign::<Fluid>(args[0]).is_some()))
}

pub(crate) fn fluid_ref(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.foreign::<Fluid>(args[0]) {
        Some(fluid) => Ok(fluid.value),
        None => Err(cx.wrong_type("fluid-ref", 1, args[0], "fluid")),
    }
}

pub(crate) fn fluid_set(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.foreign_mut::<Fluid>(args[0]) {
        Some(fluid) => {
            fluid.value = args[1];
            Ok(Value::UNSPECIFIED)
        }
        None => Err(cx.wrong_type("fluid-set!", 1, args[0], "fluid")),
    }
}

/// `(%check-fluid fluid)`: `with-fluid*`'s check.
pub(crate) fn check_fluid(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.foreign::<Fluid>(args[0]) {
        Some(_) => Ok(args[0]),
        None => Err(cx.raise(&WRONG_TYPE_PLAIN, Some("with-fluid*"), &[args[0]])),
    }
}

/// `(%make-parameter fluid converter procedure)`.
pub(crate) fn make_parameter(cx: &mut Cx, args: &[Value]) -> R {
    Ok(cx.heap.wrap(Parameter {
        fluid: args[0],
        converter: args[1],
        procedure: args[2],
    }))
}

pub(crate) fn is_parameter(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(cx.heap.foreign::<Parameter>(args[0]).is_some()))
}

/// `(%parameter-parts p)`: the parameter's fluid and converter as a pair,
/// or `parameterize`'s error.
pub(crate) fn parameter_parts(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.foreign::<Parameter>(args[0]) {
        Some(p) => {
            let (fluid, converter) = (p.fluid, p.converter);
            Ok(cx.heap.cons(fluid, converter))
        }
        None => Err(cx.raise(&NOT_A_PARAMETER, Some("parameterize"), &[args[0]])),
    }
}

/// The procedure a call of a parameter, or of a procedure with a setter,
/// calls.
#[cold]
pub(crate) fn applicable(heap: &Heap, value: Value) -> Option<Value> {
    parameter_procedure(heap, value).or_else(|| crate::hash::with_setter_procedure(heap, value))
}

/// The parameter's procedure, which a call of the parameter calls.
pub(crate) fn parameter_procedure(heap: &Heap, value: Value) -> Option<Value> {
    heap.foreign::<Parameter>(value).map(|p| p.procedure)
}

/// The control forms as Guile's `boot-9.scm` defines them, over `%dyn`;
/// and `delay`, boot-9's, and `force`, `promises.c`'s, over [`Promise`].
pub(crate) const PRELUDE: &str = r#"
(define (catch key thunk handler . pre-unwind)
  (if (not (or (symbol? key) (eq? key #t)))
      (scm-error 'wrong-type-arg "catch" "Wrong type argument in position ~a: ~a"
                 (list 2 key) (list key)))
  (if (not (procedure? handler))
      (scm-error 'wrong-type-arg "catch" "Wrong type argument in position ~a: ~a"
                 (list 3 handler) (list handler)))
  (%dyn catch key #f (thunk) handler))
(define (error . args)
  (if (null? args)
      (scm-error 'misc-error #f "?" #f #f)
      (scm-error 'misc-error #f
                 (apply string-append "~A" (map (lambda (x) " ~S") (cdr args)))
                 args #f)))
(define (dynamic-wind before thunk after)
  (before)
  (%call-values (lambda vals (after) (apply values vals))
                (%dyn wind after #f (thunk) (lambda (token) (after) (%resume token)))))
(define (call-with-escape-continuation proc)
  (let ((tag (list 'call/ec)))
    (%dyn prompt tag #f (proc (lambda vals (%abort tag vals))) values)))
(define call/ec call-with-escape-continuation)
(define (with-fluid* fluid value thunk)
  (%dyn fluid (%check-fluid fluid) value (thunk) #f))
(define (with-fluids* fluids values thunk)
  (if (not (list? fluids))
      (scm-error 'wrong-type-arg "scm_c_with_fluids" "Wrong type argument in position ~A: ~S"
                 (list 1 fluids) (list fluids)))
  (let bind ((fluids fluids) (values values))
    (if (null? fluids)
        (thunk)
        (with-fluid* (car fluids) (car values)
                     (lambda () (bind (cdr fluids) (cdr values)))))))
(define (make-parameter init . conv)
  (let* ((conv (if (pair? conv) (car conv) (lambda (x) x)))
         (fluid (make-fluid (conv init))))
    (%make-parameter fluid conv
      (lambda args
        (if (null? args)
            (fluid-ref fluid)
            (let ((prev (fluid-ref fluid)))
              (fluid-set! fluid (conv (car args)))
              prev))))))
(define-syntax-rule (delay exp)
  (make-promise (lambda () exp)))
(define (force promise)
  (let ((data (%promise-data promise)))
    (if (%promise-computed? promise)
        data
        (let ((ans (data)))
          (if (not (%promise-computed? promise))
              (%promise-set! promise ans))
          (%promise-data promise)))))
(define (%parameterize params values thunk)
  (let ((parts (map %parameter-parts params)))
    (with-fluids* (map car parts)
                  (map (lambda (part value) ((cdr part) value)) parts values)
                  thunk)))
"#;
