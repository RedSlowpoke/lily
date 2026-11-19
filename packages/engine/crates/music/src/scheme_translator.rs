//! Translators written in Scheme (`scheme-engraver.cc`, 04 §9.2, D79): N5's
//! second part, MS2 step 26.
//!
//! - **A Scheme translator** is the alist `make-engraver`,
//!   `make-performer` or `make-translator` gives, which a group takes from
//!   its definition's `consists` (a procedure called with the context, or
//!   a name `ly:register-translator` keeps, as `scheme-engravers.scm`'s 33
//!   are). [`SchemeTranslator`] wraps it as `Scheme_engraver` does: its
//!   methods, its listeners by event class, its acknowledgers and
//!   end-acknowledgers by interface, `is-midi`, `is-layout` and
//!   `must-be-last`. Each method calls the procedure with the translator's
//!   handle, a listener with the event too, an acknowledger with the grob
//!   and the translator that made it. It runs with the classes' wake rules
//!   (`translator.rs`): a timestep method makes it run always, and a
//!   translator with only listeners and acknowledgers runs when they are
//!   called. Its state is its closures', on the Scheme heap.
//! - **Grobs a Scheme engraver makes** (`ly:engraver-make-grob`) are grob
//!   records (`records.rs`) with their definition from the context
//!   (`Grob_property_info::updated`) and its interfaces, announced as
//!   `Engraver::announce_grob` does; a record's handle is `ly:grob?` and
//!   holds the properties `ly:grob-set-property!` sets. Their layout,
//!   callbacks and type checks are chapter 05's (MS3).

use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use lily_engine_scheme::{Cx, Exception, Foreign, Heap, Tag, Tracer, Value};

use crate::R;
use crate::context::{Arena, CtxId};
use crate::dispatcher::StreamEvent;
use crate::music::assq;
use crate::records::{Cause, GrobRecord, Origin};
use crate::translator::{
    FINALIZE, INITIALIZE, PRE_PROCESS, PROCESS, PROCESS_ACKNOWLEDGED, START, STOP,
    TranslatorHandle, Tx, Work, announce_grob, translator_handle,
};

type Run<T> = Result<T, Exception>;

/// The alist's keys of the protocol's methods, by bit.
const METHODS: [(&str, u8); 7] = [
    ("initialize", INITIALIZE),
    ("start-translation-timestep", START),
    ("pre-process-music", PRE_PROCESS),
    ("process-music", PROCESS),
    ("process-acknowledged", PROCESS_ACKNOWLEDGED),
    ("stop-translation-timestep", STOP),
    ("finalize", FINALIZE),
];

/// `Scheme_engraver`: a Scheme translator's procedures.
#[derive(Clone)]
pub struct SchemeTranslator {
    /// Each method's procedure, by its place in [`METHODS`], or `#f`.
    methods: [Value; 7],
    /// Its listeners, event class and procedure, in the order they are
    /// connected: the alist's, reversed (`per_instance_listeners_`).
    pub(crate) listeners: Vec<(Value, Value)>,
    /// Its acknowledgers: interface, procedure, and true for an end. An
    /// interface named twice keeps the last, as the hash does.
    pub(crate) acknowledgers: Vec<(Value, Value, bool)>,
    pub(crate) layout: bool,
    pub(crate) midi: bool,
    pub(crate) must_be_last: bool,
}

/// `ly_assoc_get`: the value of the entry `name` of `alist`, or `#f`.
fn get(heap: &mut Heap, name: &str, alist: Value) -> Value {
    let key = heap.intern(name);
    assq(heap, key, alist)
        .and_then(|e| heap.cdr(e))
        .unwrap_or(Value::FALSE)
}

/// The items of `list`, a proper list or not.
fn items(heap: &Heap, list: Value) -> Vec<Value> {
    let mut out = Vec::new();
    let mut p = list;
    while let (Some(x), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        out.push(x);
        p = rest;
    }
    out
}

/// The `(symbol . procedure)` entries of `alist`, the others skipped.
fn procedures(heap: &Heap, alist: Value) -> Vec<(Value, Value)> {
    items(heap, alist)
        .into_iter()
        .filter_map(|e| Some((heap.car(e)?, heap.cdr(e)?)))
        .filter(|&(k, p)| k.tag() == Tag::Symbol && heap.is_procedure(p))
        .collect()
}

impl SchemeTranslator {
    /// `Scheme_engraver (definition, context)`.
    pub(crate) fn new(heap: &mut Heap, def: Value) -> SchemeTranslator {
        let mut methods = [Value::FALSE; 7];
        for (slot, (name, _)) in methods.iter_mut().zip(METHODS) {
            let v = get(heap, name, def);
            if heap.is_procedure(v) {
                *slot = v;
            }
        }
        let midi = get(heap, "is-midi", def) != Value::FALSE;
        let key = heap.intern("is-layout");
        let layout = match assq(heap, key, def).and_then(|e| heap.cdr(e)) {
            Some(v) => v != Value::FALSE,
            None => !midi,
        };
        let must_be_last = get(heap, "must-be-last", def) != Value::FALSE;
        let listeners = get(heap, "listeners", def);
        let mut listeners = procedures(heap, listeners);
        listeners.reverse();
        let mut acknowledgers: Vec<(Value, Value, bool)> = Vec::new();
        for (key, end) in [("acknowledgers", false), ("end-acknowledgers", true)] {
            let alist = get(heap, key, def);
            for (iface, proc) in procedures(heap, alist) {
                match acknowledgers
                    .iter_mut()
                    .find(|(i, _, e)| *i == iface && *e == end)
                {
                    Some(entry) => entry.1 = proc,
                    None => acknowledgers.push((iface, proc, end)),
                }
            }
        }
        SchemeTranslator {
            methods,
            listeners,
            acknowledgers,
            layout,
            midi,
            must_be_last,
        }
    }

    /// The protocol methods it has.
    pub(crate) fn methods(&self) -> u8 {
        METHODS
            .iter()
            .zip(self.methods)
            .filter(|&(_, p)| p != Value::FALSE)
            .fold(0, |bits, ((_, bit), _)| bits | bit)
    }

    /// Calls the method `bit` with the translator's handle.
    fn method(&self, tx: &mut Tx<'_>, bit: u8) -> Run<()> {
        let i = METHODS.iter().position(|&(_, b)| b == bit).unwrap_or(0);
        call(tx, self.methods[i], &[])
    }
}

/// Calls `proc` with the translator's handle and `args`, as a translation
/// calls Scheme (D85).
fn call(tx: &mut Tx<'_>, proc: Value, args: &[Value]) -> Run<()> {
    if proc == Value::FALSE {
        return Ok(());
    }
    let mut all = Vec::with_capacity(args.len() + 1);
    all.push(tx.handle());
    all.extend_from_slice(args);
    crate::context::call(tx.host, tx.t, proc, &all).map(|_| ())
}

impl Work for SchemeTranslator {
    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.method(tx, INITIALIZE)
    }
    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.method(tx, START)
    }
    fn pre_process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.method(tx, PRE_PROCESS)
    }
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.method(tx, PROCESS)
    }
    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.method(tx, PROCESS_ACKNOWLEDGED)
    }
    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.method(tx, STOP)
    }
    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.method(tx, FINALIZE)
    }
    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        let proc = self.listeners.get(which).map_or(Value::FALSE, |l| l.1);
        call(tx, proc, &[ev])
    }
    /// `Engraver_dispatch_list::apply`: the grob and the translator that
    /// made it.
    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let Some(&(_, proc, _)) = self.acknowledgers.get(which) else {
            return Ok(());
        };
        let (c, slot) = {
            let a = tx.t.arena.borrow();
            a.records.grobs[grob as usize].origin
        };
        let t = tx.t;
        let source = translator_handle(tx.heap(), t, c, slot).unwrap_or(Value::FALSE);
        let grob = grob_handle(tx.heap(), &t.arena, t.value, grob);
        call(tx, proc, &[grob, source])
    }
    fn values(&self, out: &mut Vec<Value>) {
        out.extend_from_slice(&self.methods);
        for &(class, proc) in &self.listeners {
            out.extend_from_slice(&[class, proc]);
        }
        for &(iface, proc, _) in &self.acknowledgers {
            out.extend_from_slice(&[iface, proc]);
        }
    }
    fn scheme(&self) -> Option<&SchemeTranslator> {
        Some(self)
    }
}

// --- grobs -----------------------------------------------------------------

/// A grob record's Scheme handle (`ly:grob?`).
pub struct GrobHandle {
    arena: Rc<RefCell<Arena>>,
    pub(crate) translation: Value,
    pub id: u32,
}

impl GrobHandle {
    /// Its translation's arena.
    pub(crate) fn arena(&self) -> Rc<RefCell<Arena>> {
        Rc::clone(&self.arena)
    }
}

impl Foreign for GrobHandle {
    fn type_name(&self) -> &'static str {
        "Grob"
    }

    /// `#<Grob TextScript >`.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<Grob ");
        if let Some(g) = self.arena.borrow().records.grobs.get(self.id as usize) {
            heap.display(g.name, out);
        }
        out.push_str(" >");
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.translation);
    }
}

/// The handle of the grob record `id`, made the first time.
pub(crate) fn grob_handle(
    heap: &mut Heap,
    arena: &Rc<RefCell<Arena>>,
    translation: Value,
    id: u32,
) -> Value {
    let made = arena.borrow().records.grobs[id as usize].handle;
    if made != Value::FALSE {
        return made;
    }
    // A resumed translation names its grob by the handle the last
    // compile's run made for its number, so that a splice of that run leaves
    // one handle for each grob (D243).
    let old = arena.borrow().splice.as_ref().and_then(|s| s.handle(id));
    if let Some(handle) = old {
        arena.borrow_mut().records.grobs[id as usize].handle = handle;
        return handle;
    }
    let handle = heap.wrap(GrobHandle {
        arena: Rc::clone(arena),
        translation,
        id,
    });
    arena.borrow_mut().records.grobs[id as usize].handle = handle;
    handle
}

/// The translation and number of the grob record whose handle is `v`: how
/// layout finds a grob's state in D108's slot.
pub fn grob_of(heap: &Heap, v: Value) -> Option<(Rc<RefCell<Arena>>, u32)> {
    heap.foreign::<GrobHandle>(v)
        .map(|h| (Rc::clone(&h.arena), h.id))
}

/// The record of a copy of the grob `v` (`Grob::Grob (Grob const &)`,
/// which breaking's `clone` calls): its original's type, origin, cause,
/// moments, interfaces and definition, and a copy of what is set on it
/// (`ly_alist_copy`: the alist and each of its pairs copied, the values
/// shared). The copy's handle; `None` for what is not a grob. Layout's
/// part of the copy is layout's (`item.rs`' `clone`).
pub fn clone_grob(heap: &mut Heap, v: Value) -> Option<Value> {
    let h = heap.foreign::<GrobHandle>(v)?;
    let (arena, translation, id) = (Rc::clone(&h.arena), h.translation, h.id);
    let (copy, props) = {
        let a = arena.borrow();
        let g = &a.records.grobs[id as usize];
        let copy = GrobRecord {
            name: g.name,
            origin: g.origin,
            cause: g.cause,
            moment: g.moment,
            ended: g.ended,
            interfaces: g.interfaces,
            props: Value::NULL,
            definition: g.definition,
            handle: Value::FALSE,
        };
        (copy, g.props)
    };
    let mut entries = Vec::new();
    let mut p = props;
    while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        entries.push(match (heap.car(entry), heap.cdr(entry)) {
            (Some(key), Some(value)) => heap.cons(key, value),
            _ => entry,
        });
        p = rest;
    }
    let props = entries
        .into_iter()
        .rev()
        .fold(Value::NULL, |tail, entry| heap.cons(entry, tail));
    let copy_id = {
        let mut a = arena.borrow_mut();
        let copy_id = a.records.grobs.len() as u32;
        a.records.grobs.push(GrobRecord { props, ..copy });
        copy_id
    };
    Some(grob_handle(heap, &arena, translation, copy_id))
}

/// The cause `Engraver::announce_grob` sets on the grob whose handle is
/// `v`, as its `cause` property gives it: a stream event or a grob's
/// handle, or `None` for no cause (or no grob).
pub fn grob_cause(heap: &mut Heap, v: Value) -> Option<Value> {
    let h = heap.foreign::<GrobHandle>(v)?;
    let (arena, translation, id) = (Rc::clone(&h.arena), h.translation, h.id);
    let cause = arena.borrow().records.grobs[id as usize].cause;
    match cause {
        Cause::Event(ev) => Some(ev),
        Cause::Grob(n) => Some(grob_handle(heap, &arena, translation, n)),
        Cause::None | Cause::Audio(_) => None,
    }
}

/// The translator a handle names: its arena, translation, context and slot.
type Of = (Rc<RefCell<Arena>>, Value, CtxId, u16);

fn engraver(cx: &mut Cx, subr: &str, v: Value) -> Result<Of, lily_engine_scheme::Thrown> {
    match cx.heap().foreign::<TranslatorHandle>(v) {
        Some(h) => Ok((Rc::clone(&h.arena), h.translation, h.context, h.slot)),
        None => Err(cx.wrong_type(subr, 1, v, "Engraver")),
    }
}

fn grob(
    cx: &mut Cx,
    subr: &str,
    pos: usize,
    v: Value,
) -> Result<(Rc<RefCell<Arena>>, u32), lily_engine_scheme::Thrown> {
    match cx.heap().foreign::<GrobHandle>(v) {
        Some(h) => Ok((Rc::clone(&h.arena), h.id)),
        None => Err(cx.wrong_type(subr, pos, v, "Grob")),
    }
}

/// `ly_is_grob_cause`: `'()`, a stream event or a grob, as a [`Cause`].
fn cause(cx: &mut Cx, subr: &str, v: Value) -> Result<Cause, lily_engine_scheme::Thrown> {
    let heap = cx.heap();
    if v == Value::NULL {
        Ok(Cause::None)
    } else if heap.foreign::<StreamEvent>(v).is_some() {
        Ok(Cause::Event(v))
    } else if let Some(g) = heap.foreign::<GrobHandle>(v) {
        Ok(Cause::Grob(g.id))
    } else {
        Err(cx.wrong_type(subr, 3, v, "grob cause"))
    }
}

/// `(ly:grob? x)`.
pub(crate) fn is_grob(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<GrobHandle>(cx, args[0])
}

/// `(ly:engraver-make-grob engraver grob-name cause)`: `internal_make_grob`
/// of a record, announced to the engraver's group and those above it.
pub(crate) fn ly_engraver_make_grob(cx: &mut Cx, args: &[Value]) -> R {
    make_grob(cx, "ly:engraver-make-grob", args, None)
}

/// `(ly:engraver-make-item engraver grob-name cause)` (MS8 step 9, part 2,
/// for the image's `Custos_engraver`): `internal_make_item`, a grob of the
/// class `Item`. `choose_grob_type<Item>` gives LilyPond's programming
/// error where the definition's `classes` lack `Item`; the grob is made
/// all the same. Layout classifies it by its definition, as any grob of
/// one class (every definition `make-item` reaches is of `Item` alone).
pub(crate) fn ly_engraver_make_item(cx: &mut Cx, args: &[Value]) -> R {
    make_grob(cx, "ly:engraver-make-item", args, Some("Item"))
}

/// `internal_make_grob`, of the class `class` where one is asked for.
fn make_grob(cx: &mut Cx, subr: &'static str, args: &[Value], class: Option<&str>) -> R {
    let (arena, translation, c, slot) = engraver(cx, subr, args[0])?;
    if args[1].tag() != Tag::Symbol {
        return Err(cx.wrong_type(subr, 2, args[1], "symbol"));
    }
    let cause = cause(cx, subr, args[2])?;
    let (id, props) = {
        let heap = cx.heap_mut();
        let mut a = arena.borrow_mut();
        let props = crate::override_::updated(heap, &a, c, args[1]);
        if props.tag() != Tag::Pair {
            drop(a);
            let mut name = String::new();
            cx.heap().display(args[1], &mut name);
            let text = alloc::format!("No grob definition found for `{name}'.");
            return Err(cx.error("misc-error", Some(subr), &text, &[]));
        }
        let id = push_record(heap, &mut a, (c, slot), args[1], cause, props);
        announce_grob(&mut a, c, id, false);
        (id, props)
    };
    if let Some(class) = class {
        // choose_grob_type<T>
        let heap = cx.heap_mut();
        let meta = get(heap, "meta", props);
        let classes = get(heap, "classes", meta);
        let sym = heap.intern(class);
        if !items(heap, classes).contains(&sym) {
            let (mut name, mut list) = (String::new(), String::new());
            heap.display(args[1], &mut name);
            heap.write(classes, &mut list);
            let text = alloc::format!(
                "grob {name} created with disallowed class {class} (expected any class in the list {list})"
            );
            crate::warn::diagnose(cx, "programming-error", &text);
        }
    }
    Ok(grob_handle(cx.heap_mut(), &arena, translation, id))
}

/// A new grob record of the type `name` that `origin` made, its definition
/// `props` and its interfaces the definition's; its number.
pub(crate) fn push_record(
    heap: &mut Heap,
    a: &mut Arena,
    origin: Origin,
    name: Value,
    cause: Cause,
    props: Value,
) -> u32 {
    let meta = get(heap, "meta", props);
    let interfaces = get(heap, "interfaces", meta);
    let interfaces = a.records.interfaces.set(heap, interfaces);
    let id = a.records.grobs.len() as u32;
    let moment = a.now();
    a.records.grobs.push(GrobRecord {
        name,
        origin,
        cause,
        moment,
        ended: None,
        interfaces,
        props: Value::NULL,
        definition: props,
        handle: Value::FALSE,
    });
    let made = a.records.grobs[id as usize].clone();
    a.effect(crate::replay::Effect::Grob(
        id,
        alloc::boxed::Box::new(made),
    ));
    id
}

/// `(ly:engraver-announce-end-grob engraver grob cause)`.
pub(crate) fn ly_engraver_announce_end_grob(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:engraver-announce-end-grob";
    let (arena, _, c, _) = engraver(cx, SUBR, args[0])?;
    let (_, id) = grob(cx, SUBR, 2, args[1])?;
    cause(cx, SUBR, args[2])?;
    crate::context::grob_ended(&arena, id);
    let mut a = arena.borrow_mut();
    if a.parent(c).is_none() {
        drop(a);
        crate::warn::diagnose(
            cx,
            "programming-error",
            "context for engraver has been detached",
        );
        return Ok(Value::UNSPECIFIED);
    }
    a.end_grob(id);
    announce_grob(&mut a, c, id, true);
    Ok(Value::UNSPECIFIED)
}

/// `(ly:grob-property grob sym [val])`: what was set, else its cause for
/// `cause` (which `Engraver::announce_grob` sets, an event or a grob), else
/// its definition's, else `val` or `'()`; a callback is not called. Layout
/// replaces it under its name (D108), with chapter 05's lookup; this one
/// is the music crate's without layout.
pub(crate) fn ly_grob_property(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-property";
    let (arena, id) = grob(cx, SUBR, 1, args[0])?;
    if args[1].tag() != Tag::Symbol {
        return Err(cx.wrong_type(SUBR, 2, args[1], "symbol"));
    }
    let (props, definition) = {
        let a = arena.borrow();
        let g = &a.records.grobs[id as usize];
        (g.props, g.definition)
    };
    let heap = cx.heap_mut();
    let set = |heap: &Heap, alist| assq(heap, args[1], alist).and_then(|e| heap.cdr(e));
    if let Some(v) = set(heap, props) {
        return Ok(v);
    }
    if heap.name(args[1]) == Some("cause")
        && let Some(cause) = grob_cause(heap, args[0])
    {
        return Ok(cause);
    }
    let found = set(heap, definition);
    Ok(found.unwrap_or(args.get(2).copied().unwrap_or(Value::NULL)))
}

/// `(ly:grob-set-property! grob sym val)`: set on the grob, unchecked; layout
/// replaces it (D108).
pub(crate) fn ly_grob_set_property(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-set-property!";
    let (arena, id) = grob(cx, SUBR, 1, args[0])?;
    if args[1].tag() != Tag::Symbol {
        return Err(cx.wrong_type(SUBR, 2, args[1], "symbol"));
    }
    crate::context::grob_wrote(
        &arena,
        Some(id),
        crate::replay::Op::Record(args[1], args[2]),
        None,
    );
    set_record_property(cx.heap_mut(), &arena, id, args[1], args[2]);
    Ok(Value::UNSPECIFIED)
}

/// `sym` set to `v` among what is set on the grob record `id`.
pub(crate) fn set_record_property(
    heap: &mut Heap,
    arena: &RefCell<Arena>,
    id: u32,
    sym: Value,
    v: Value,
) {
    let props = arena.borrow().records.grobs[id as usize].props;
    let props = match assq(heap, sym, props) {
        Some(entry) => {
            heap.set_cdr(entry, v);
            props
        }
        None => {
            let entry = heap.cons(sym, v);
            heap.cons(entry, props)
        }
    };
    arena.borrow_mut().records.grobs[id as usize].props = props;
}

/// What a default Scheme engraver calls of a grob's C++ besides the
/// primitives above: `Breathing_sign::set_breath_properties`, the
/// `breathMarkDefinitions` entry of `breath-type` set on the grob where it
/// holds nothing of the property's type, read as `get_property_data` reads
/// it, its callback not called (MS8 step 9's part 2: a `Divisio`'s
/// `ly:text-interface::print` had been called on no text, and its stencil
/// kept over the divisio's). Its warning is LilyPond's text, without the
/// grob's location.
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:breathing-sign::set-breath-properties grob context breath-type)
  (%check-type "ly:breathing-sign::set-breath-properties" 1 ly:grob? "Grob" grob)
  (%check-type "ly:breathing-sign::set-breath-properties" 2 ly:context? "Context" context)
  (%check-type "ly:breathing-sign::set-breath-properties" 3 symbol? "symbol" breath-type)
  (let ((s (assq breath-type (ly:context-property context 'breathMarkDefinitions))))
    (if (not s)
        (ly:warning (G_ "do not know how to interpret breath type: ~a") breath-type)
        (for-each
         (lambda (prop)
           (let ((type (object-property (car prop) 'backend-type?)))
             (if (procedure? type)
                 (if (or (null? (cdr prop)) (not (type (ly:grob-property-data grob (car prop)))))
                     (ly:grob-set-property! grob (car prop) (cdr prop)))
                 (ly:programming-error
                  (string-append "invalid grob property name in breath definition: "
                                 (object->string (car prop)))))))
         (cdr s)))))
"#;
