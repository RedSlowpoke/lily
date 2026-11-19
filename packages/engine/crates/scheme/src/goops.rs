//! `(oop goops)` as LilyPond's library files use it (chapter 03 §2.6):
//! `define-class` makes a record type, as `lily-library.scm`'s
//! `<regex-match>` needs, with `make`, `is-a?`, `slot-ref`, `slot-set!`,
//! `slot-bound?` and `class-name`, and GOOPS's errors; and generic
//! functions as `part-combiner.scm` uses them (MS5 step 6a):
//! `define-method` and a slot's `#:accessor` and `#:getter`.
//!
//! A class is a foreign object with its name, its direct superclasses and
//! its slots, a subclass's after those it inherits; an instance holds its
//! class and a value per slot, [`Value::UNDEFINED`] while it is unbound.
//! A slot's `#:init-keyword`, `#:init-value`, `#:init-thunk` and
//! `#:init-form` are kept; its other options are not.
//!
//! A generic is a procedure with a setter ([`PRELUDE`]), so an accessor's
//! `(set! (accessor x) v)` is its setter's call: each applies the most
//! specific of its methods whose classes the arguments are of. As Guile
//! 3.0's, `define-method` on a name bound to what is no generic is GOOPS's
//! error, but for `write` and `display`, Guile's primitive generics, which
//! become the generic's method for any arguments; an accessor keeps any
//! procedure so (`ensure-accessor`).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::errors::list_items;
use crate::heap::{Foreign, Heap, Tracer};
use crate::image::{ImageReader, ImageWriter};
use crate::value::{Tag, Value};
use crate::vm::{Cx, Exception, Thrown};

type R = Result<Value, Thrown>;

#[derive(Clone, Copy)]
struct SlotDef {
    name: Value,
    /// Its `#:init-keyword`, or `#f`.
    keyword: Value,
    /// Its `#:init-value`, or undefined.
    value: Value,
    /// Its `#:init-thunk` (or `#:init-form` made one), or `#f`.
    thunk: Value,
}

pub(crate) struct Class {
    name: Value,
    supers: Vec<Value>,
    slots: Vec<SlotDef>,
}

impl Foreign for Class {
    fn type_name(&self) -> &'static str {
        "class"
    }

    /// `#<<class> <regex-match>>`, without the address Guile prints.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<<class> ");
        heap.display(self.name, out);
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.name);
        for &s in &self.supers {
            tracer.trace(s);
        }
        for slot in &self.slots {
            for v in [slot.name, slot.keyword, slot.value, slot.thunk] {
                tracer.trace(v);
            }
        }
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.value(self.name);
        out.values(&self.supers);
        out.count(self.slots.len());
        for slot in &self.slots {
            for v in [slot.name, slot.keyword, slot.value, slot.thunk] {
                out.value(v);
            }
        }
        true
    }
}

pub(crate) fn load_class(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    let name = r.value()?;
    let supers = r.values()?;
    let slots = (0..r.count()?)
        .map(|_| {
            Some(SlotDef {
                name: r.value()?,
                keyword: r.value()?,
                value: r.value()?,
                thunk: r.value()?,
            })
        })
        .collect::<Option<_>>()?;
    Some(Box::new(Class {
        name,
        supers,
        slots,
    }))
}

#[derive(Clone)]
pub(crate) struct Instance {
    class: Value,
    values: Vec<Value>,
}

impl Foreign for Instance {
    fn type_name(&self) -> &'static str {
        "instance"
    }

    /// `#<<regex-match>>`, without the address Guile prints.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<");
        if let Some(class) = heap.foreign::<Class>(self.class) {
            heap.display(class.name, out);
        }
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.class);
        for &v in &self.values {
            tracer.trace(v);
        }
    }

    fn snapshot(&self) -> Option<Box<dyn Foreign>> {
        Some(Box::new(self.clone()))
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.value(self.class);
        out.values(&self.values);
        true
    }
}

pub(crate) fn load_instance(r: &mut ImageReader<'_>) -> Option<Box<dyn Foreign>> {
    Some(Box::new(Instance {
        class: r.value()?,
        values: r.values()?,
    }))
}

/// `(%make-class name supers slots)`: each slot is `(name option value
/// ...)`.
pub(crate) fn make_class(cx: &mut Cx, args: &[Value]) -> R {
    let supers = list_items(&cx.heap, args[1]).unwrap_or_default();
    let mut slots = Vec::new();
    for &s in &supers {
        let Some(class) = cx.heap.foreign::<Class>(s) else {
            return Err(cx.wrong_type("define-class", 2, s, "class"));
        };
        slots.extend_from_slice(&class.slots);
    }
    for spec in list_items(&cx.heap, args[2]).unwrap_or_default() {
        let items = list_items(&cx.heap, spec).unwrap_or_default();
        let Some(&name) = items.first().filter(|n| n.tag() == Tag::Symbol) else {
            return Err(cx.wrong_type("define-class", 3, spec, "slot definition"));
        };
        let mut slot = SlotDef {
            name,
            keyword: Value::FALSE,
            value: Value::UNDEFINED,
            thunk: Value::FALSE,
        };
        for pair in items[1..].chunks(2) {
            let (option, value) = (pair[0], pair.get(1).copied().unwrap_or(Value::FALSE));
            match cx
                .heap
                .name(option)
                .filter(|_| option.tag() == Tag::Keyword)
            {
                Some("init-keyword") => slot.keyword = value,
                Some("init-value") => slot.value = value,
                Some("init-thunk") => slot.thunk = value,
                _ => {}
            }
        }
        // A slot redefined replaces the one inherited.
        match slots.iter().position(|s| s.name == name) {
            Some(i) => slots[i] = slot,
            None => slots.push(slot),
        }
    }
    let class = Class {
        name: args[0],
        supers,
        slots,
    };
    Ok(cx.heap.wrap(class))
}

/// `(%make-instance class initargs)`: an instance whose slots have their
/// keyword's value or their `#:init-value`; those with a thunk are left
/// for `make` to fill ([`PRELUDE`]).
pub(crate) fn make_instance(cx: &mut Cx, args: &[Value]) -> R {
    let initargs = list_items(&cx.heap, args[1]).unwrap_or_default();
    if !initargs.len().is_multiple_of(2) || initargs.chunks(2).any(|p| p[0].tag() != Tag::Keyword) {
        let key = cx.heap.intern("wrong-type-arg");
        let message = cx.heap.string("Invalid initargs: ~S");
        let culprit = cx.heap.list(&[args[1]]);
        let args = cx
            .heap
            .list(&[Value::FALSE, message, culprit, Value::FALSE]);
        cx.error = Some(Exception { key, args });
        return Err(Thrown);
    }
    let Some(class) = cx.heap.foreign::<Class>(args[0]) else {
        return Err(cx.wrong_type("make", 1, args[0], "class"));
    };
    let values = class
        .slots
        .iter()
        .map(|slot| {
            let given = initargs
                .chunks(2)
                .find(|p| slot.keyword != Value::FALSE && p[0] == slot.keyword);
            given.map_or(slot.value, |p| p[1])
        })
        .collect();
    Ok(cx.heap.wrap(Instance {
        class: args[0],
        values,
    }))
}

/// `(%class-thunks class)`: `(name . thunk)` of the slots a thunk fills.
pub(crate) fn class_thunks(cx: &mut Cx, args: &[Value]) -> R {
    let Some(class) = cx.heap.foreign::<Class>(args[0]) else {
        return Err(cx.wrong_type("make", 1, args[0], "class"));
    };
    let pairs: Vec<(Value, Value)> = class
        .slots
        .iter()
        .filter(|s| s.thunk != Value::FALSE)
        .map(|s| (s.name, s.thunk))
        .collect();
    let items: Vec<Value> = pairs
        .into_iter()
        .map(|(name, thunk)| cx.heap.cons(name, thunk))
        .collect();
    Ok(cx.heap.list(&items))
}

/// Whether `class` is `target` or inherits from it.
fn inherits(heap: &Heap, class: Value, target: Value) -> bool {
    let mut pending = alloc::vec![class];
    while let Some(class) = pending.pop() {
        if class == target {
            return true;
        }
        if let Some(c) = heap.foreign::<Class>(class) {
            pending.extend_from_slice(&c.supers);
        }
    }
    false
}

/// `(is-a? object class)`: whether the object's class is `class` or
/// inherits from it.
pub(crate) fn is_a(cx: &mut Cx, args: &[Value]) -> R {
    let Some(instance) = cx.heap.foreign::<Instance>(args[0]) else {
        return Ok(Value::FALSE);
    };
    Ok(Value::bool(inherits(&cx.heap, instance.class, args[1])))
}

/// `(%subclass? class other)`: whether `class` is `other` or inherits
/// from it, which orders a generic's methods.
pub(crate) fn is_subclass(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(inherits(&cx.heap, args[0], args[1])))
}

/// `(class-name class)`.
pub(crate) fn class_name(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.foreign::<Class>(args[0]) {
        Some(class) => Ok(class.name),
        None => Err(cx.wrong_type("class-name", 1, args[0], "class")),
    }
}

/// GOOPS's error: `(goops-error #f message args ())`.
fn goops_error(cx: &mut Cx, message: &str, items: &[Value]) -> Thrown {
    let key = cx.heap.intern("goops-error");
    let message = cx.heap.string(message);
    let items = cx.heap.list(items);
    let args = cx.heap.list(&[Value::FALSE, message, items, Value::NULL]);
    cx.error = Some(Exception { key, args });
    Thrown
}

/// The index of the slot `name` of `object`, or GOOPS's error.
fn slot_index(cx: &mut Cx, subr: &str, object: Value, name: Value) -> Result<usize, Thrown> {
    let Some(instance) = cx.heap.foreign::<Instance>(object) else {
        return Err(cx.wrong_type(subr, 1, object, "instance"));
    };
    let index = cx
        .heap
        .foreign::<Class>(instance.class)
        .and_then(|c| c.slots.iter().position(|s| s.name == name));
    match index {
        Some(i) => Ok(i),
        None => Err(goops_error(
            cx,
            "No slot with name `~S' in object ~S",
            &[name, object],
        )),
    }
}

fn slot_value(cx: &Cx, object: Value, index: usize) -> Value {
    cx.heap
        .foreign::<Instance>(object)
        .map_or(Value::UNDEFINED, |i| i.values[index])
}

/// `(slot-ref object name)`.
pub(crate) fn slot_ref(cx: &mut Cx, args: &[Value]) -> R {
    let index = slot_index(cx, "slot-ref", args[0], args[1])?;
    match slot_value(cx, args[0], index) {
        Value::UNDEFINED => Err(goops_error(cx, "Unbound slot in object ~S", &[args[0]])),
        value => Ok(value),
    }
}

/// `(slot-set! object name value)`.
pub(crate) fn slot_set(cx: &mut Cx, args: &[Value]) -> R {
    let index = slot_index(cx, "slot-set!", args[0], args[1])?;
    if let Some(instance) = cx.heap.foreign_mut::<Instance>(args[0]) {
        instance.values[index] = args[2];
    }
    Ok(Value::UNSPECIFIED)
}

/// `(slot-bound? object name)`.
pub(crate) fn is_slot_bound(cx: &mut Cx, args: &[Value]) -> R {
    let index = slot_index(cx, "slot-bound?", args[0], args[1])?;
    Ok(Value::bool(
        slot_value(cx, args[0], index) != Value::UNDEFINED,
    ))
}

/// The primitives of `(oop goops)`.
pub(crate) const NAMES: &[&str] = &[
    "%make-class",
    "%make-instance",
    "%class-thunks",
    "is-a?",
    "%subclass?",
    "class-name",
    "slot-ref",
    "slot-set!",
    "slot-bound?",
];

/// `define-class` and `make`, in `(oop goops)`: a slot's `#:init-form`
/// becomes a thunk, its `#:accessor` and `#:getter` methods of generics.
/// `define-method` and the generics: each generic's methods, and its
/// setter's, are `(classes . procedure)` in `%generics`, a class `#t` for
/// an argument not specialized, none for a procedure kept as a method.
pub(crate) const PRELUDE: &str = r#"
(define-syntax define-class
  (syntax-rules ()
    ((_ name (super ...) slot ...)
     (begin
       (define name (%make-class 'name (list super ...) (list (%slot-spec slot) ...)))
       (%slot-accessors name slot) ...))))
(define-syntax %slot-accessors
  (syntax-rules ()
    ((_ class (name option ...)) (%slot-accessors* class name option ...))
    ((_ class name) (if #f #f))))
(define-syntax %slot-accessors*
  (syntax-rules ()
    ((_ class name) (if #f #f))
    ((_ class name #:accessor a . rest)
     (begin
       (define a (%accessor (%generic-for 'a #f) class 'name #t))
       (%slot-accessors* class name . rest)))
    ((_ class name #:getter a . rest)
     (begin
       (define a (%accessor (%generic-for 'a #f) class 'name #f))
       (%slot-accessors* class name . rest)))
    ((_ class name k v . rest) (%slot-accessors* class name . rest))))
(define-syntax define-method
  (syntax-rules ()
    ((_ (name . formals) body ...) (%method name formals () () body ...))))
(define-syntax %method
  (syntax-rules ()
    ((_ name ((arg class) . rest) (spec ...) (plain ...) body ...)
     (%method name rest (spec ... class) (plain ... arg) body ...))
    ((_ name (arg . rest) (spec ...) (plain ...) body ...)
     (%method name rest (spec ... #t) (plain ... arg) body ...))
    ((_ name () (spec ...) (plain ...) body ...)
     (define name
       (%add-method! (%generic-for 'name #t) 1 (list spec ...) (lambda (plain ...) body ...))))
    ((_ name rest (spec ...) (plain ...) body ...)
     (define name
       (%add-method! (%generic-for 'name #t) 1 (list spec ...)
                     (lambda (plain ... . rest) body ...))))))
(define %generics (make-weak-key-hash-table))
(define (%generic-for name method?)
  (let ((current (and (defined? name) (module-ref (current-module) name))))
    (if (and method? current (not (hashq-ref %generics current))
             (not (memq current (list write display))))
        (scm-error 'goops-error #f "~S is not a valid generic function" (list current) '()))
    (if (and current (hashq-ref %generics current))
        current
        (let* ((v (vector name (if (procedure? current) (list (cons '() current)) '()) '()))
               (g (make-procedure-with-setter
                   (lambda args (%apply-method v 1 args))
                   (lambda args (%apply-method v 2 args)))))
          (hashq-set! %generics g v)
          g))))
(define (%add-method! g which classes proc)
  (let ((v (hashq-ref %generics g)))
    (vector-set! v which
                 (cons (cons classes proc)
                       (filter (lambda (m) (not (equal? (car m) classes)))
                               (vector-ref v which))))
    g))
(define (%accessor g class name setter?)
  (%add-method! g 1 (list class) (lambda (o) (slot-ref o name)))
  (if setter?
      (%add-method! g 2 (list class #t) (lambda (o value) (slot-set! o name value))))
  g)
(define (%applicable? classes args)
  (or (null? classes)
      (and (pair? args)
           (or (eq? (car classes) #t) (is-a? (car args) (car classes)))
           (%applicable? (cdr classes) (cdr args)))))
(define (%more-specific? a b)
  (cond ((null? b) (pair? a))
        ((null? a) #f)
        ((eq? (car a) (car b)) (%more-specific? (cdr a) (cdr b)))
        ((eq? (car b) #t) #t)
        ((eq? (car a) #t) #f)
        (else (%subclass? (car a) (car b)))))
(define (%apply-method v which args)
  (let loop ((methods (vector-ref v which)) (best #f))
    (cond ((pair? methods)
           (loop (cdr methods)
                 (if (and (%applicable? (caar methods) args)
                          (or (not best) (%more-specific? (caar methods) (car best))))
                     (car methods)
                     best)))
          (best (apply (cdr best) args))
          (else
           (scm-error 'goops-error #f "No applicable method for ~S in call ~S"
                      (list (vector-ref v 0) (cons (vector-ref v 0) args)) #f)))))
(define-syntax %slot-spec
  (syntax-rules ()
    ((_ (name option ...)) (%slot-options 'name () option ...))
    ((_ name) (list 'name))))
(define-syntax %slot-options
  (syntax-rules ()
    ((_ name (acc ...)) (list name acc ...))
    ((_ name (acc ...) #:init-form e . rest)
     (%slot-options name (acc ... #:init-thunk (lambda () e)) . rest))
    ((_ name (acc ...) #:accessor a . rest) (%slot-options name (acc ...) . rest))
    ((_ name (acc ...) #:getter a . rest) (%slot-options name (acc ...) . rest))
    ((_ name (acc ...) k v . rest) (%slot-options name (acc ... k v) . rest))))
(define (make class . initargs)
  (let ((object (%make-instance class initargs)))
    (for-each (lambda (slot)
                (if (not (slot-bound? object (car slot)))
                    (slot-set! object (car slot) ((cdr slot)))))
              (%class-thunks class))
    object))
"#;
