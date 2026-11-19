//! LilyPond's `Context` and `Global_context` (`context.cc`,
//! `context-property.cc`, `global-context.cc`): N3's instances (04 §7,
//! D73, D79, D83), in one arena per translation.
//!
//! - **The arena** ([`Arena`]) holds a translation's contexts, linked by
//!   number, its dispatchers ([`Dispatchers`]) and event classes
//!   ([`EventClasses`]), the moment, and `\once`'s finalizations. A context
//!   removed stays in it, marked. Global is context 0. The arena is shared
//!   by the translation's Scheme object ([`Translation`]), which traces it,
//!   and by its contexts' and dispatchers' Scheme handles, which keep that
//!   object alive. It is never borrowed across a call into Scheme, where
//!   the collector may trace it.
//! - **Properties** are interned to numbers per translation. A context's
//!   own are a sorted array shared until it is written, with a 64-bit mask
//!   of the numbers it holds, so a lookup up the parents skips a context
//!   by one test; Global's and the Score's (a context that adopts) are a
//!   dense table, written rarely and read most.
//! - **Creation** ports `find_create_context`, `path_to_acceptable_context`
//!   and `create_context_from_event` as `context.cc` has them: a context is
//!   made by a `CreateContext` event its parent hears, and announced by
//!   `AnnounceNewContext`. **Removal** is `check_removal`'s.
//! - **What needs Scheme** (a property's type check, an `apply` operation,
//!   a listener of Scheme's) goes through a [`Host`], which the VM is, and
//!   so runs where Rust drives the VM: the iterators of step 23. A
//!   primitive cannot call Scheme, so `ly:context-set-property!` and the
//!   others that check a type are Scheme in [`PRELUDE`], which calls
//!   `%type-check`, and then a primitive that does not check.
//!
//! Overrides are `override.rs`'s. The Scheme face is LilyPond's `ly:context-*`
//! primitives, its dispatchers' (`ly:add-listener` and the rest) and its
//! stream events' (`dispatcher.rs`).

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use lily_engine_scheme::{Cx, Exception, Foreign, Heap, Tag, Thrown, Tracer, Value, Vm};

use crate::R;
use crate::context_def::ContextDef;
use crate::dispatcher::{
    DispId, Dispatchers, Handler, Listener, StreamEvent, event_property, make_event,
};
use crate::event_class::EventClasses;
use crate::output_def::host_lookup_variable;
use crate::override_::GrobProperties;
use crate::rational::{Moment, Rational};
use crate::records::{GrobSlot, Records};
use crate::translator::{Group, TranslatorClass};

/// A context's number in its translation's arena.
pub type CtxId = u32;
const NONE: CtxId = u32::MAX;
/// The Global context's.
pub const GLOBAL: CtxId = 0;

/// Where a recorded translation began or resumed, from which what it changes
/// of what is older than it is seen (`resume.rs`).
#[derive(Clone, Copy, Default)]
pub(crate) struct ResumeBase {
    /// The undo log's length and the epoch then.
    pub(crate) log: usize,
    pub(crate) since: u32,
    /// What moves whenever a global changes (`Host::globals_written`), then.
    pub(crate) globals: u64,
}

/// What a context's code needs of the VM it runs in.
pub trait Host {
    fn heap(&mut self) -> &mut Heap;
    /// Calls the procedure `proc`.
    fn call(&mut self, proc: Value, args: &[Value]) -> Result<Value, Exception>;
    /// [`Host::call`], which may return in the middle: `None` then, and
    /// [`Host::go_on_steps`] goes on with it (`Vm::begin_steps`).
    fn begin_steps(&mut self, proc: Value, args: &[Value]) -> Result<Option<Value>, Exception> {
        self.call(proc, args).map(Some)
    }
    fn go_on_steps(&mut self) -> Result<Option<Value>, Exception> {
        Ok(None)
    }
    /// `module-ref` of `symbol` in `module`, if bound.
    fn module_ref(&mut self, module: Value, symbol: Value) -> Option<Value>;
    /// What moves whenever a score's code changes a global.
    fn globals_written(&mut self) -> u64 {
        0
    }
    /// The binding of `name` in `(lily)`.
    fn lily(&mut self, name: &str) -> Option<Value>;
    /// Sets `(*location*)`, as `with_location` does around a call; the
    /// value it had.
    fn set_location(&mut self, location: Value) -> Value {
        let _ = location;
        Value::FALSE
    }
    /// How much has been written to the error port, and the port cut back
    /// to such a length (N8's snapshots, `resume.rs`).
    fn error_len(&mut self) -> usize {
        0
    }
    fn cut_errors(&mut self, len: usize) {
        let _ = len;
    }
    /// What has been written to the error port since it was `len` long,
    /// and `text` written to it again (N8's resume).
    fn errors_since(&mut self, len: usize) -> String {
        let _ = len;
        String::new()
    }
    fn write_errors(&mut self, text: &str) {
        let _ = text;
    }
}

impl Host for Vm {
    fn heap(&mut self) -> &mut Heap {
        self.heap_mut()
    }

    fn globals_written(&mut self) -> u64 {
        Vm::globals_written(self)
    }

    fn call(&mut self, proc: Value, args: &[Value]) -> Result<Value, Exception> {
        self.apply(proc, args)
    }

    fn begin_steps(&mut self, proc: Value, args: &[Value]) -> Result<Option<Value>, Exception> {
        Vm::begin_steps(self, proc, args)
    }

    fn go_on_steps(&mut self) -> Result<Option<Value>, Exception> {
        Vm::go_on_steps(self)
    }

    fn module_ref(&mut self, module: Value, symbol: Value) -> Option<Value> {
        let name = String::from(self.heap_mut().name(symbol)?);
        self.module_value(module, &name)
    }

    fn lily(&mut self, name: &str) -> Option<Value> {
        let slot = self.slot("lily", name)?;
        self.slot_value(slot)
    }

    fn set_location(&mut self, location: Value) -> Value {
        Vm::set_location(self, location)
    }

    fn error_len(&mut self) -> usize {
        Vm::error_len(self)
    }

    fn cut_errors(&mut self, len: usize) {
        Vm::cut_errors(self, len);
    }

    fn errors_since(&mut self, len: usize) -> String {
        Vm::errors_since(self, len)
    }

    fn write_errors(&mut self, text: &str) {
        Vm::write_errors(self, text);
    }
}

type Run<T> = Result<T, Exception>;

/// A context's own properties.
#[derive(Clone)]
enum Props {
    /// Sorted by number, shared until written; bit `n % 64` of the mask for
    /// each number `n` held.
    Sparse {
        mask: u64,
        entries: Rc<[(u16, Value)]>,
    },
    /// By number, `UNDEFINED` for none.
    Dense(Vec<Value>),
}

impl Props {
    fn get(&self, id: u16) -> Option<Value> {
        match self {
            Props::Sparse { mask, entries } => {
                if mask & (1 << (id % 64)) == 0 {
                    return None;
                }
                let at = entries.binary_search_by_key(&id, |&(i, _)| i).ok()?;
                Some(entries[at].1)
            }
            Props::Dense(values) => values
                .get(usize::from(id))
                .copied()
                .filter(|&v| v != Value::UNDEFINED),
        }
    }

    fn set(&mut self, id: u16, value: Value) {
        match self {
            Props::Sparse { mask, entries } => {
                let mut copy: Vec<(u16, Value)> = entries.to_vec();
                match copy.binary_search_by_key(&id, |&(i, _)| i) {
                    Ok(at) => copy[at].1 = value,
                    Err(at) => copy.insert(at, (id, value)),
                }
                *mask |= 1 << (id % 64);
                *entries = copy.into();
            }
            Props::Dense(values) => {
                if values.len() <= usize::from(id) {
                    values.resize(usize::from(id) + 1, Value::UNDEFINED);
                }
                values[usize::from(id)] = value;
            }
        }
    }

    fn remove(&mut self, id: u16) {
        match self {
            Props::Sparse { mask, entries } => {
                if let Ok(at) = entries.binary_search_by_key(&id, |&(i, _)| i) {
                    let mut copy = entries.to_vec();
                    copy.remove(at);
                    *mask = copy.iter().fold(0, |m, &(i, _)| m | 1 << (i % 64));
                    *entries = copy.into();
                }
            }
            Props::Dense(values) => {
                if let Some(v) = values.get_mut(usize::from(id)) {
                    *v = Value::UNDEFINED;
                }
            }
        }
    }

    fn values(&self) -> Vec<(u16, Value)> {
        match self {
            Props::Sparse { entries, .. } => entries.to_vec(),
            Props::Dense(values) => values
                .iter()
                .enumerate()
                .filter(|(_, v)| **v != Value::UNDEFINED)
                .map(|(i, &v)| (i as u16, v))
                .collect(),
        }
    }
}

/// A context.
#[derive(Clone)]
pub struct Context {
    /// Its [`ContextDef`], and the mods of its `\with` (`definition_mods_`).
    def: Value,
    mods: Value,
    id: String,
    parent: CtxId,
    children: Vec<CtxId>,
    props: Props,
    /// Its acceptance set: what it accepts, its default child first.
    accepts: Vec<Value>,
    default_child: Value,
    aliases: Value,
    /// Whether a context made below it may come into it (a Score's).
    adopts: bool,
    event_source: DispId,
    events_below: DispId,
    /// When it was added to the tree.
    init_mom: Moment,
    /// The iterators that hold it (`client_count_`).
    clients: u32,
    removed: bool,
    /// Its Scheme handle.
    handle: Value,
    /// Its translator group, once its parent's group made it.
    group: Option<Group>,
}

/// A `\once`'s undoing, at the end of the timestep.
#[derive(Clone, Copy)]
enum Finalization {
    Set(CtxId, Value, Value),
    Unset(CtxId, Value),
    /// `ly:context-matched-pop-property` of a grob's property and the token
    /// of a `\once \override` or `\once \revert`.
    MatchedPop(CtxId, Value, Value),
}

/// A translation's contexts and what they share.
pub struct Arena {
    contexts: Vec<Context>,
    pub dispatchers: Dispatchers,
    /// The handles of the dispatchers, by number.
    dispatcher_handles: Vec<Value>,
    pub classes: EventClasses,
    prop_ids: BTreeMap<u64, u16>,
    prop_syms: Vec<Value>,
    output_def: Value,
    now: Moment,
    prev: Moment,
    finalizations: Vec<Finalization>,
    /// The context `AnnounceNewContext` announced (`infant_event_`).
    infant: CtxId,
    /// What `ly:broadcast` was asked to send, by dispatcher, which the
    /// Rust that called Scheme sends when the call returns ([`call`]).
    broadcasts: Vec<(DispId, Value)>,
    /// The moments translators asked for (`extra_mom_pq_`), in order.
    extra: Vec<Moment>,
    /// The translation trace's recorder, if it is traced (`trace.rs`).
    pub(crate) recorder: Option<crate::trace::Recorder>,
    /// The grobs and audio items its translators made.
    pub records: Records,
    /// Translator classes of the translation's own, found before
    /// [`crate::translator::CLASSES`].
    pub(crate) extra_classes: Vec<&'static TranslatorClass>,
    /// The playback map its performers record, in a `\midi` translation
    /// that writes one (`playback.rs`).
    pub playback: Option<crate::playback::Playback>,
    /// What a `\midi` translation's Score keeps of its performance while it
    /// runs (`performance.rs`).
    pub(crate) performance: Option<crate::performance::State>,
    /// The paths [`path_to_acceptable_context`] found, by the definition
    /// searched from, the names it accepts and the name sought: the output
    /// definition's context definitions do not change while it runs, and
    /// T29's 66 contexts searched them again for each (MS2 step 27).
    paths: Vec<(Value, Vec<Value>, Value, Vec<Value>)>,
    /// What the lyric combine iterators' listeners share, by number
    /// (`lyrics.rs`).
    pub(crate) combines: Vec<crate::lyrics::Combine>,
    /// N8's recording of a `\layout` or `\midi` translation
    /// (`checkpoint.rs`), while it runs.
    pub(crate) n8: Option<Box<crate::checkpoint::Recording>>,
    /// The properties set, unset or overridden in the timestep being run,
    /// in order, each with what it was given (a value, `#<undefined>` for
    /// an unset, an override's alist), while it is recorded: N8's shared
    /// level reads them as it ends (`partition.rs`), a property set and
    /// unset in one timestep among them (OPEN C12).
    pub(crate) touched: RefCell<Vec<(CtxId, Value, Value)>>,
    /// The context whose translator is being called, and the grobs written
    /// in the timestep being run, each with the context whose translator
    /// wrote it (`None`: a grob array of no known grob), while it is
    /// recorded: N8's partitions' writes outside themselves (MS6 step 7a).
    pub(crate) writer: core::cell::Cell<Option<CtxId>>,
    pub(crate) wrote: RefCell<Vec<Written>>,
    /// Each grob record's rank among those its context made at its moment
    /// (N8's places), and the grob records ended in the timestep being run.
    pub(crate) ranks: RefCell<Ranks>,
    pub(crate) ended: RefCell<Vec<u32>>,
    /// N8's replay (MS6 step 7b): the log of what its translators' calls
    /// did, while it is recorded, shared by its copies.
    pub(crate) log: Option<crate::replay::Shared>,
    /// The parts its replay found could not be replayed, each with the
    /// last moment it failed (MS6 step 7c).
    pub(crate) failed_parts: Vec<(u32, Moment)>,
    /// A replay running (MS6 step 7b): the old run's calls made again in
    /// place of its replayed parts' translators' calls.
    pub(crate) replay: Option<Box<crate::replay::Replay>>,
    /// N8's snapshots at its checkpoints while it is recorded, and after
    /// (`resume.rs`), and where it began or resumed.
    pub(crate) snapshots: Vec<Rc<crate::resume::Snapshot>>,
    pub(crate) resume_base: ResumeBase,
    /// Whether, as it ended, it had changed nothing older than it for good
    /// (D258): a splice leaves out what it did after the checkpoint, which
    /// such a change would be lost with. Set in the copy kept as its end.
    pub(crate) end_pure: bool,
    /// The error port's length where the translation began, or resumed
    /// less what it said before (N8's resume).
    pub(crate) errors_base: usize,
    /// The placed messages said before it began or resumed (the list
    /// `%diagnostics` held then).
    pub(crate) diagnostics_base: Value,
    /// The placed messages it had said when it ended, as a snapshot keeps
    /// them (`resume.rs`), for a splice of it in the next compile.
    pub(crate) placed_at_end: Vec<([Value; 3], usize)>,
    /// What a resumed translation splices in where it converges (N8's
    /// splice, D243): the last compile's run of it, held here so that its
    /// objects live while this one runs.
    pub(crate) splice: Option<Box<crate::resume::Splice>>,
    /// Itself as its translation ended, before layout (N8's splice), and
    /// the generation and mark of the heap's log there.
    pub(crate) at_end: Option<Rc<Arena>>,
    pub(crate) end_mark: Option<(u32, usize)>,
    /// The arena a resume took the place of, as the last compile left it, and
    /// which a stop puts back (B24, D261). Here so that the collector reaches
    /// it. Gone once the compile ends.
    pub(crate) resumed_from: Option<Box<Arena>>,
    /// D108's slot: layout's state of its grobs, `None` until layout
    /// fills it.
    pub grob_slot: Option<Box<dyn GrobSlot>>,
    /// The translation's Scheme object.
    object: Value,
}

/// A grob written by a translator while it is recorded (N8, MS6 step 7a):
/// the context whose translator wrote it, the grob (`None`: a grob array
/// whose grob is not known) and, where the write added a grob of the same
/// translation to one of its grob arrays, that grob.
pub(crate) type Written = (CtxId, Option<u32>, Option<u32>);

/// Each grob record's rank among those its context made at its moment, as
/// far as they have been asked for.
#[derive(Clone, Default)]
pub(crate) struct Ranks {
    of: Vec<u32>,
    at: Option<Moment>,
    counts: BTreeMap<CtxId, u32>,
}

/// The grob record `id` of the translation whose arena is `arena` was
/// written by `op` (`None`: a grob array of it, whose grob is not known),
/// by the translator being called, if it is recorded (N8, MS6 step 7a):
/// layout's writes call it. `added`: the grob of the same translation the
/// write added to one of the grob's grob arrays, if it did. The call's log
/// keeps the write (MS6 step 7b).
pub fn grob_wrote(
    arena: &RefCell<Arena>,
    id: Option<u32>,
    op: crate::replay::Op,
    added: Option<u32>,
) {
    let a = arena.borrow();
    if a.noting()
        && let Some(c) = a.writer.get()
    {
        a.wrote.borrow_mut().push((c, id, added));
        match (id, op) {
            (Some(id), op) if op != crate::replay::Op::Unknown => {
                a.effect(crate::replay::Effect::Write(id, op));
            }
            _ => a.unreplayable(),
        }
    }
}

/// The grob record `id` ended by an announcement of a translator being
/// called: a write N8's partitions note (MS6 step 7a), which the call's
/// log keeps as the announcement.
pub(crate) fn grob_ended(arena: &RefCell<Arena>, id: u32) {
    let a = arena.borrow();
    if a.noting()
        && let Some(c) = a.writer.get()
    {
        a.wrote.borrow_mut().push((c, Some(id), None));
    }
}

/// Whether the translation whose arena is `arena` is logged for N8's
/// replay (MS6 step 7b): a write layout makes then keeps a copy of the
/// value it sets, as it is.
pub fn recorded(arena: &RefCell<Arena>) -> bool {
    let a = arena.borrow();
    a.n8.is_some() && a.log.is_some()
}

/// No writes noted till [`unquiet`] (an add's own setting of its array):
/// the writer to give back.
pub fn quiet(arena: &RefCell<Arena>) -> Option<CtxId> {
    arena.borrow().writer.replace(None)
}

/// The writer [`quiet`] took, given back.
pub fn unquiet(arena: &RefCell<Arena>, writer: Option<CtxId>) {
    arena.borrow().writer.set(writer);
}

/// A translation: its arena and its Scheme object, which the caller keeps
/// rooted while it runs.
#[derive(Clone)]
pub struct Translation {
    pub arena: Rc<RefCell<Arena>>,
    pub value: Value,
}

/// A translation's Scheme object: what traces its arena.
struct TranslationObject(Rc<RefCell<Arena>>);

impl Foreign for TranslationObject {
    fn type_name(&self) -> &'static str {
        "Translation"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        let arena = self
            .0
            .try_borrow()
            .expect("a translation borrowed across a call into Scheme");
        arena.trace(tracer);
    }
}

/// A context's Scheme handle (`ly:context?`).
pub struct ContextHandle {
    arena: Rc<RefCell<Arena>>,
    translation: Value,
    pub id: CtxId,
}

/// A dispatcher's Scheme handle (`ly:dispatcher?`).
pub struct DispatcherHandle {
    arena: Rc<RefCell<Arena>>,
    translation: Value,
    pub id: DispId,
}

impl Foreign for ContextHandle {
    fn type_name(&self) -> &'static str {
        "Context"
    }

    /// `#<Context Voice=one ()>`, as `Context::print_smob`, the children
    /// written as their handles.
    fn print(&self, heap: &Heap, out: &mut String) {
        let arena = self.arena.borrow();
        let c = &arena.contexts[self.id as usize];
        out.push_str("#<Context ");
        if let Some(def) = heap.foreign::<ContextDef>(c.def) {
            heap.display(def.name(), out);
        }
        if !c.id.is_empty() {
            out.push('=');
            out.push_str(&c.id);
        }
        out.push_str(" (");
        for (i, &child) in c.children.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            heap.display(arena.contexts[child as usize].handle, out);
        }
        out.push_str(") >");
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.translation);
    }
}

impl Foreign for DispatcherHandle {
    fn type_name(&self) -> &'static str {
        "Dispatcher"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.translation);
    }
}

/// Which way a search goes from a context (`Direction`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Center,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    FindCreate,
    FindOnly,
    CreateOnly,
}

impl Arena {
    /// Its state copied whole, for N8's snapshots (MS6 step 5, D241), its
    /// own snapshots left out.
    pub(crate) fn snapshot(&self) -> Arena {
        Arena {
            contexts: self.contexts.clone(),
            dispatchers: self.dispatchers.clone(),
            dispatcher_handles: self.dispatcher_handles.clone(),
            classes: self.classes.clone(),
            prop_ids: self.prop_ids.clone(),
            prop_syms: self.prop_syms.clone(),
            output_def: self.output_def,
            now: self.now,
            prev: self.prev,
            finalizations: self.finalizations.clone(),
            infant: self.infant,
            broadcasts: self.broadcasts.clone(),
            extra: self.extra.clone(),
            recorder: self.recorder.clone(),
            records: self.records.clone(),
            extra_classes: self.extra_classes.clone(),
            playback: self.playback.clone(),
            performance: self.performance.clone(),
            paths: self.paths.clone(),
            combines: self.combines.clone(),
            n8: self.n8.clone(),
            touched: RefCell::new(self.touched.borrow().clone()),
            writer: core::cell::Cell::new(self.writer.get()),
            wrote: RefCell::new(self.wrote.borrow().clone()),
            ranks: RefCell::new(self.ranks.borrow().clone()),
            ended: RefCell::new(self.ended.borrow().clone()),
            log: self.log.clone(),
            failed_parts: self.failed_parts.clone(),
            replay: None,
            snapshots: Vec::new(),
            resume_base: self.resume_base,
            end_pure: self.end_pure,
            errors_base: self.errors_base,
            diagnostics_base: self.diagnostics_base,
            placed_at_end: Vec::new(),
            splice: None,
            at_end: None,
            end_mark: None,
            resumed_from: None,
            grob_slot: self.grob_slot.as_ref().map(|g| g.clone_slot()),
            object: self.object,
        }
    }

    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        if let Some(log) = &self.log {
            log.borrow().trace(tracer);
        }
        self.trace_copy(tracer);
    }

    /// [`Arena::trace`] of a snapshot's copy: what its log holds is in the
    /// log of the arena it is a copy of, or of the run resumed from it.
    pub(crate) fn trace_copy(&self, tracer: &mut Tracer<'_>) {
        crate::resume::trace(&self.snapshots, tracer);
        if let Some(r) = &self.replay {
            r.trace(tracer);
        }
        for c in &self.contexts {
            for v in [c.def, c.mods, c.default_child, c.aliases, c.handle] {
                tracer.trace(v);
            }
            for &v in &c.accepts {
                tracer.trace(v);
            }
            for (_, v) in c.props.values() {
                tracer.trace(v);
            }
            if let Some(g) = &c.group {
                g.trace(tracer);
            }
        }
        self.records.trace(tracer);
        if let Some(slot) = &self.grob_slot {
            slot.trace(tracer);
        }
        for &v in self.dispatcher_handles.iter().chain(&self.prop_syms) {
            tracer.trace(v);
        }
        self.dispatchers.trace(tracer);
        tracer.trace(self.output_def);
        tracer.trace(self.object);
        tracer.trace(self.diagnostics_base);
        for (d, _) in &self.placed_at_end {
            for &v in d {
                tracer.trace(v);
            }
        }
        if let Some(s) = &self.splice {
            s.trace(tracer);
        }
        if let Some(a) = &self.resumed_from {
            a.trace(tracer);
        }
        if let Some(a) = &self.at_end {
            a.trace(tracer);
        }
        if let Some(r) = &self.recorder {
            r.trace(tracer);
        }
        if let Some(r) = &self.n8 {
            r.trace(tracer);
        }
        for &(_, sym, now) in self.touched.borrow().iter() {
            tracer.trace(sym);
            tracer.trace(now);
        }
        for f in &self.finalizations {
            match *f {
                Finalization::Set(_, s, v) | Finalization::MatchedPop(_, s, v) => {
                    tracer.trace(s);
                    tracer.trace(v);
                }
                Finalization::Unset(_, s) => tracer.trace(s),
            }
        }
        for &(_, ev) in &self.broadcasts {
            tracer.trace(ev);
        }
        for (def, accepts, name, path) in &self.paths {
            for &v in [def, name].into_iter().chain(accepts).chain(path) {
                tracer.trace(v);
            }
        }
        for c in &self.combines {
            c.trace(tracer);
        }
        if let Some(p) = &self.performance {
            p.trace(tracer);
        }
    }

    fn ctx(&self, c: CtxId) -> &Context {
        &self.contexts[c as usize]
    }

    /// The number of the property `sym`, made if `make`.
    fn prop_id(&mut self, sym: Value, make: bool) -> Option<u16> {
        if let Some(&id) = self.prop_ids.get(&sym.to_bits()) {
            return Some(id);
        }
        if !make {
            return None;
        }
        let id = u16::try_from(self.prop_syms.len()).ok()?;
        self.prop_ids.insert(sym.to_bits(), id);
        self.prop_syms.push(sym);
        Some(id)
    }

    /// `internal_here_defined`: the value `c` itself holds for `sym`.
    pub fn here(&self, c: CtxId, sym: Value) -> Option<Value> {
        let id = *self.prop_ids.get(&sym.to_bits())?;
        self.ctx(c).props.get(id)
    }

    /// `internal_where_defined`: the nearest of `c` and its parents that
    /// holds `sym`, and the value.
    pub fn where_defined(&self, mut c: CtxId, sym: Value) -> Option<(CtxId, Value)> {
        let id = *self.prop_ids.get(&sym.to_bits())?;
        while c != NONE {
            if let Some(v) = self.ctx(c).props.get(id) {
                return Some((c, v));
            }
            c = self.ctx(c).parent;
        }
        None
    }

    /// `internal_get_property`: `sym`'s value seen from `c`, or `'()`.
    pub fn get(&self, c: CtxId, sym: Value) -> Value {
        self.where_defined(c, sym).map_or(Value::NULL, |(_, v)| v)
    }

    /// Sets `sym` in `c`, unchecked.
    pub fn set(&mut self, c: CtxId, sym: Value, value: Value) {
        if let Some(id) = self.prop_id(sym, true) {
            self.contexts[c as usize].props.set(id, value);
            self.note(c, sym, value);
            self.effect(crate::replay::Effect::Property(c, sym, Some(value)));
        }
    }

    /// Unsets `sym` in `c`, unchecked.
    pub fn unset(&mut self, c: CtxId, sym: Value) {
        if let Some(id) = self.prop_id(sym, false) {
            self.contexts[c as usize].props.remove(id);
            self.note(c, sym, Value::UNDEFINED);
            self.effect(crate::replay::Effect::Property(c, sym, None));
        }
    }

    /// The grob properties `c` holds for `sym` changed to the alist `now`
    /// (an override or a revert): noted for N8 while the translation is
    /// recorded, a translator's call that did it not replayable.
    pub(crate) fn touch(&self, c: CtxId, sym: Value, now: Value) {
        self.note(c, sym, now);
        self.unreplayable();
    }

    /// `c`'s property `sym` changed to `now`: noted for N8 while the
    /// translation is recorded in full.
    fn note(&self, c: CtxId, sym: Value, now: Value) {
        if self.noting() {
            self.touched.borrow_mut().push((c, sym, now));
        }
    }

    /// Whether what its translators write and hear is noted: only N8's
    /// checks and a replay read it (D258).
    pub(crate) fn noting(&self) -> bool {
        self.n8.as_ref().is_some_and(|r| r.full())
    }

    /// The translator call running did `e` (N8's replay, MS6 step 7b).
    pub(crate) fn effect(&self, e: crate::replay::Effect) {
        if !crate::replay::on() {
            return;
        }
        if let Some(log) = &self.log {
            log.borrow_mut().push(e);
        }
    }

    /// `ly_assoc_prepend_x` of the alist `sym` is seen from `c`, set in `c`:
    /// `(acons key val (assoc-remove! alist key))`, the entry of `key`
    /// unlinked in place, which contexts sharing the alist see. The call's
    /// log keeps the change, not its value, for that (`localAlterations`,
    /// MS6 step 7c).
    pub fn prepend_property(
        &mut self,
        heap: &mut Heap,
        c: CtxId,
        sym: Value,
        key: Value,
        val: Value,
    ) {
        let alist = self.get(c, sym);
        let mut rest = alist;
        let mut at = alist;
        let mut prev: Option<Value> = None;
        while let (Some(entry), Some(next)) = (heap.car(at), heap.cdr(at)) {
            if heap.car(entry).is_some_and(|k| heap.equal(k, key)) {
                // `scm_delq1_x`: the first pair holding the entry.
                match prev {
                    Some(p) => {
                        heap.set_cdr(p, next);
                    }
                    None => rest = next,
                }
                break;
            }
            prev = Some(at);
            at = next;
        }
        let entry = heap.cons(key, val);
        let now = heap.cons(entry, rest);
        if let Some(id) = self.prop_id(sym, true) {
            self.contexts[c as usize].props.set(id, now);
            self.note(c, sym, now);
            self.effect(crate::replay::Effect::Prepend(c, sym, key, val));
        }
    }

    /// The grob record `id`'s rank among those its context made at its
    /// moment.
    pub(crate) fn grob_rank(&self, id: u32) -> u32 {
        let mut r = self.ranks.borrow_mut();
        while r.of.len() <= id as usize {
            let g = &self.records.grobs[r.of.len()];
            if r.at != Some(g.moment) {
                r.at = Some(g.moment);
                r.counts.clear();
            }
            let n = r.counts.entry(g.origin.0).or_insert(0);
            let rank = *n;
            *n += 1;
            r.of.push(rank);
        }
        r.of[id as usize]
    }

    /// The grob record `id`'s end announced now.
    pub(crate) fn end_grob(&mut self, id: u32) {
        let now = self.now();
        if let Some(g) = self.records.grobs.get_mut(id as usize) {
            g.ended = Some(now);
            self.ended.borrow_mut().push(id);
        }
    }

    /// The audio record `id` changed in the translator call running: its
    /// log keeps what it is now, a staff's items apart (MS6 step 7c).
    pub(crate) fn audio_changed(&self, id: u32) {
        if self.log.is_none() {
            return;
        }
        let r = &self.records.audio[id as usize];
        let audio = match &r.audio {
            crate::audio::Audio::Staff(s) => {
                let mut s = s.clone();
                s.items.clear();
                crate::audio::Audio::Staff(s)
            }
            other => other.clone(),
        };
        self.effect(crate::replay::Effect::AudioSet(
            id,
            Box::new(audio),
            r.channel,
        ));
    }

    /// [`Arena::unreplayable`] if a translator's call is running.
    pub(crate) fn unreplayable_in_call(&self) {
        if let Some(log) = &self.log
            && log.borrow().in_call()
        {
            log.borrow_mut().not_replayable();
        }
    }

    /// The translator call running did what a replay cannot do again.
    pub(crate) fn unreplayable(&self) {
        if let Some(log) = &self.log {
            log.borrow_mut().not_replayable();
        }
    }

    /// The properties `c` itself holds, as `(symbol, value)`, by number.
    pub fn properties(&self, c: CtxId) -> Vec<(Value, Value)> {
        let mut props = self.ctx(c).props.values();
        props.sort_by_key(|&(id, _)| id);
        props
            .into_iter()
            .map(|(id, v)| (self.prop_syms[usize::from(id)], v))
            .collect()
    }

    pub fn parent(&self, c: CtxId) -> Option<CtxId> {
        Some(self.ctx(c).parent).filter(|&p| p != NONE)
    }

    pub fn children(&self, c: CtxId) -> &[CtxId] {
        &self.ctx(c).children
    }

    pub fn handle(&self, c: CtxId) -> Value {
        self.ctx(c).handle
    }

    pub fn id(&self, c: CtxId) -> &str {
        &self.ctx(c).id
    }

    pub fn def(&self, c: CtxId) -> Value {
        self.ctx(c).def
    }

    /// The mods of its `\with` (`get_definition_mods`).
    pub fn mods(&self, c: CtxId) -> Value {
        self.ctx(c).mods
    }

    pub(crate) fn group(&self, c: CtxId) -> Option<&Group> {
        self.ctx(c).group.as_ref()
    }

    pub(crate) fn group_mut(&mut self, c: CtxId) -> Option<&mut Group> {
        self.contexts[c as usize].group.as_mut()
    }

    pub(crate) fn set_group(&mut self, c: CtxId, group: Group) {
        self.contexts[c as usize].group = Some(group);
    }

    /// How many contexts were made, removed ones among them: their numbers
    /// are below it.
    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }

    /// The names of `c`'s translators, in the order they run.
    pub fn translator_names(&self, c: CtxId) -> Vec<String> {
        self.group(c).map(Group::names).unwrap_or_default()
    }

    /// The names of `c`'s translators that are placeholders, classes with
    /// no work (D67).
    pub fn placeholder_names(&self, c: CtxId) -> Vec<&'static str> {
        self.group(c).map(Group::placeholders).unwrap_or_default()
    }

    /// The class of the translator `origin`, a context and its number
    /// there, as LilyPond prints it; empty if it has none.
    pub fn translator_name(&self, origin: crate::records::Origin) -> &'static str {
        self.group(origin.0)
            .and_then(|g| g.name(usize::from(origin.1)))
            .unwrap_or_default()
    }

    /// `add_alias`: `sym` an alias of `c` too.
    pub fn add_alias(&mut self, heap: &mut Heap, c: CtxId, sym: Value) {
        let aliases = self.ctx(c).aliases;
        self.contexts[c as usize].aliases = heap.cons(sym, aliases);
    }

    pub fn removed(&self, c: CtxId) -> bool {
        self.ctx(c).removed
    }

    pub fn init_mom(&self, c: CtxId) -> Moment {
        self.ctx(c).init_mom
    }

    /// Sets how many iterators hold `c`, which keep it from removal.
    pub fn set_clients(&mut self, c: CtxId, clients: u32) {
        self.contexts[c as usize].clients = clients;
    }

    /// How many iterators hold `c`.
    pub fn clients(&self, c: CtxId) -> u32 {
        self.ctx(c).clients
    }

    pub fn event_source(&self, c: CtxId) -> DispId {
        self.ctx(c).event_source
    }

    pub fn events_below(&self, c: CtxId) -> DispId {
        self.ctx(c).events_below
    }

    /// `now_mom`: the translation's moment.
    pub fn now(&self) -> Moment {
        self.now
    }

    /// `previous_moment`: the moment before it, or the first one.
    pub fn prev(&self) -> Moment {
        self.prev
    }

    /// `add_moment_to_process`: a moment the global loop is to stop at;
    /// false if it is before now (`trying to freeze in time`), which is
    /// added all the same.
    pub fn add_moment(&mut self, m: Moment) -> bool {
        self.effect(crate::replay::Effect::Moment(m));
        if let Err(at) = self.extra.binary_search_by(|x| x.compare(m)) {
            self.extra.insert(at, m);
        }
        !m.compare(self.now).is_lt()
    }

    /// `sneaky_insert_extra_moment`: the first moment asked for that is not
    /// after `w`, taken, or `w`.
    pub fn take_extra(&mut self, w: Moment) -> Moment {
        match self.extra.first() {
            Some(&m) if !m.compare(w).is_gt() => self.extra.remove(0),
            _ => w,
        }
    }

    /// The moments translators asked for and the loop has not taken.
    pub(crate) fn extra_moments(&self) -> &[Moment] {
        &self.extra
    }

    /// The name of `c`'s definition, a symbol.
    pub fn name(&self, heap: &Heap, c: CtxId) -> Value {
        heap.foreign::<ContextDef>(self.ctx(c).def)
            .map_or(Value::NULL, ContextDef::name)
    }

    /// `is_bottom_context`: whether it has no default child.
    pub fn is_bottom(&self, c: CtxId) -> bool {
        self.ctx(c).default_child == Value::NULL
    }

    /// `is_alias`: whether `sym` names `c`: its name, an alias, or `Bottom`
    /// for a bottom context.
    pub fn is_alias(&self, heap: &Heap, c: CtxId, sym: Value) -> bool {
        if heap.name(sym) == Some("Bottom") && sym.tag() == Tag::Symbol {
            return self.is_bottom(c);
        }
        if sym == self.name(heap, c) {
            return true;
        }
        let mut p = self.ctx(c).aliases;
        while let (Some(a), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            if a == sym {
                return true;
            }
            p = rest;
        }
        false
    }

    /// `matches`: of the type `name` (`'()` for any) and the id `id`
    /// (empty for any).
    fn matches(&self, heap: &Heap, c: CtxId, name: Value, id: &str) -> bool {
        (id.is_empty() || self.ctx(c).id == id)
            && (name == Value::NULL || self.is_alias(heap, c, name))
    }

    /// `is_removable`: no children, no iterator holding it, and its parent
    /// not Global.
    pub fn is_removable(&self, c: CtxId) -> bool {
        let ctx = self.ctx(c);
        ctx.children.is_empty() && ctx.clients == 0 && ctx.parent != GLOBAL
    }

    /// `core_find` in `FIND_ONLY` mode, which makes nothing.
    fn find_only(&self, heap: &Heap, c: CtxId, dir: Dir, name: Value, id: &str) -> Option<CtxId> {
        if self.matches(heap, c, name, id) {
            return Some(c);
        }
        if dir != Dir::Up {
            for &child in &self.ctx(c).children {
                if let Some(found) = self.find_only(heap, child, Dir::Down, name, id) {
                    return Some(found);
                }
            }
        }
        match self.parent(c) {
            Some(p) if dir != Dir::Down => self.find_only(heap, p, dir, name, id),
            _ => None,
        }
    }

    /// `find_context`: as [`find`] finds, making nothing.
    pub fn find_context(
        &self,
        heap: &Heap,
        c: CtxId,
        dir: Dir,
        name: Value,
        id: &str,
    ) -> Option<CtxId> {
        // `unchecked_find`: for `CENTER`, below and then above first, and
        // then the walk down from each context above (step 36's `\change`
        // to a sibling staff).
        let near = if dir == Dir::Center {
            self.find_only(heap, c, Dir::Down, name, id)
                .or_else(|| self.find_only(heap, c, Dir::Up, name, id))
        } else {
            None
        };
        let found = near.or_else(|| self.find_only(heap, c, dir, name, id));
        found.filter(|&f| f != GLOBAL)
    }
}

/// A [`Context`] for `def` with the mods `ops`, its acceptance its
/// definition's with the ops' `\accepts`, `\denies` and `\defaultchild`.
fn new_context(heap: &mut Heap, arena: &mut Arena, def: Value, ops: Value, dense: bool) -> CtxId {
    let d = heap.foreign::<ContextDef>(def);
    let accepts_list = d.map_or(Value::NULL, ContextDef::accepts);
    let mut default_child = d.map_or(Value::NULL, ContextDef::default_child);
    let aliases = d.map_or(Value::NULL, ContextDef::aliases);
    let group = d.map_or(Value::NULL, ContextDef::group_type);
    let adopts = matches!(heap.name(group), Some("Score_engraver" | "Score_performer"));
    let mut accepts = list_items(heap, accepts_list);
    for op in list_items(heap, ops) {
        let tag = heap.car(op).and_then(|t| heap.name(t).map(String::from));
        let Some(arg) = heap.cdr(op).and_then(|r| heap.car(r)) else {
            continue;
        };
        let sym = heap.intern_string(arg).unwrap_or(arg);
        match tag.as_deref() {
            // `Acceptance_set::accept`: after the default.
            Some("accepts") if sym != default_child => {
                let at = usize::from(default_child != Value::NULL).min(accepts.len());
                accepts.insert(at, sym);
            }
            Some("denies") => {
                accepts.retain(|&a| a != sym);
                if sym == default_child {
                    default_child = Value::NULL;
                }
            }
            Some("default-child") => {
                accepts.retain(|&a| a != sym);
                accepts.insert(0, sym);
                default_child = sym;
            }
            _ => {}
        }
    }
    let event_source = arena.dispatchers.make();
    let events_below = arena.dispatchers.make();
    let id = arena.contexts.len() as CtxId;
    for d in [event_source, events_below] {
        let handle = heap.wrap(DispatcherHandle {
            arena: arena_rc(heap, arena.object),
            translation: arena.object,
            id: d,
        });
        arena.dispatcher_handles.push(handle);
    }
    let handle = heap.wrap(ContextHandle {
        arena: arena_rc(heap, arena.object),
        translation: arena.object,
        id,
    });
    arena.contexts.push(Context {
        def,
        mods: ops,
        id: String::new(),
        parent: NONE,
        children: Vec::new(),
        props: if dense || adopts {
            Props::Dense(Vec::new())
        } else {
            Props::Sparse {
                mask: 0,
                entries: Rc::from([]),
            }
        },
        accepts,
        default_child,
        aliases,
        adopts,
        event_source,
        events_below,
        init_mom: Moment::default(),
        clients: 0,
        removed: false,
        handle,
        group: None,
    });
    id
}

/// The arena of the translation object `object`.
fn arena_rc(heap: &Heap, object: Value) -> Rc<RefCell<Arena>> {
    let t = heap
        .foreign::<TranslationObject>(object)
        .expect("a translation");
    Rc::clone(&t.0)
}

fn list_items(heap: &Heap, list: Value) -> Vec<Value> {
    let mut items = Vec::new();
    let mut p = list;
    while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        items.push(item);
        p = rest;
    }
    items
}

/// `find_context_def`: the definition named `name` in the output
/// definition `odef` or its parents.
fn find_def(host: &mut dyn Host, odef: Value, name: Value) -> Option<Value> {
    let def = host_lookup_variable(host, odef, name)?;
    host.heap().foreign::<ContextDef>(def).map(|_| def)
}

/// `Context::diagnostic_id`: the name written, and ` = id`.
pub(crate) fn diagnostic_id(heap: &Heap, name: Value, id: &str) -> String {
    let mut out = String::new();
    heap.write(name, &mut out);
    if !id.is_empty() {
        out.push_str(" = ");
        out.push_str(id);
    }
    out
}

/// A message of `kind` (`warning`, `programming-error`) through `(lily)`'s
/// `ly:warning` or `ly:programming-error`, as LilyPond's C++ gives it.
pub fn diagnose(host: &mut dyn Host, kind: &str, text: &str) -> Run<()> {
    let name = if kind == "warning" {
        "ly:warning"
    } else {
        "ly:programming-error"
    };
    if let Some(proc) = host.lily(name) {
        let text = host.heap().string(text.replace('~', "~~"));
        host.call(proc, &[text])?;
    }
    Ok(())
}

impl Translation {
    /// `ly:make-global-context` of `odef`: Global made from its definition,
    /// its default property operations applied, and each grob description
    /// a property of grob properties, with `property-defaults` after its
    /// own. `None` without a definition of Global.
    pub fn new(host: &mut dyn Host, odef: Value) -> Run<Option<Translation>> {
        let heap = host.heap();
        let global = heap.intern("Global");
        let Some(def) = find_def(host, odef, global) else {
            diagnose(
                host,
                "programming-error",
                "definition for Global context not found",
            )?;
            return Ok(None);
        };
        let heap = host.heap();
        let arena = Rc::new(RefCell::new(Arena {
            contexts: Vec::new(),
            dispatchers: Dispatchers::default(),
            dispatcher_handles: Vec::new(),
            classes: EventClasses::new(heap),
            prop_ids: BTreeMap::new(),
            prop_syms: Vec::new(),
            output_def: odef,
            now: Moment::new(Rational::new(-1, 0), Rational::ZERO),
            prev: Moment::new(Rational::new(-1, 0), Rational::ZERO),
            finalizations: Vec::new(),
            infant: NONE,
            broadcasts: Vec::new(),
            extra: Vec::new(),
            recorder: None,
            records: Records::default(),
            extra_classes: Vec::new(),
            playback: None,
            performance: None,
            paths: Vec::new(),
            combines: Vec::new(),
            n8: None,
            touched: RefCell::new(Vec::new()),
            writer: core::cell::Cell::new(None),
            wrote: RefCell::new(Vec::new()),
            ranks: RefCell::default(),
            ended: RefCell::default(),
            log: None,
            failed_parts: Vec::new(),
            replay: None,
            snapshots: Vec::new(),
            resume_base: ResumeBase::default(),
            end_pure: false,
            errors_base: 0,
            diagnostics_base: Value::NULL,
            placed_at_end: Vec::new(),
            splice: None,
            at_end: None,
            end_mark: None,
            resumed_from: None,
            grob_slot: None,
            object: Value::FALSE,
        }));
        let object = heap.wrap(TranslationObject(Rc::clone(&arena)));
        let t = Translation {
            arena,
            value: object,
        };
        let root = heap.root_depth();
        heap.root(object);
        {
            let mut a = t.arena.borrow_mut();
            a.object = object;
            let g = new_context(heap, &mut a, def, Value::NULL, true);
            let (source, below) = (a.event_source(g), a.events_below(g));
            a.add_listener(
                heap,
                source,
                Listener::Context(g, Handler::CreateContext),
                "CreateContext",
            );
            a.add_listener(
                heap,
                source,
                Listener::Context(g, Handler::Prepare),
                "Prepare",
            );
            a.dispatchers.register_as_listener(below, source);
        }
        let ops = default_ops(host.heap(), def);
        let ran = apply_property_operations(host, &t, GLOBAL, ops);
        if ran.is_ok() {
            let defaults = host.heap().intern("property-defaults");
            let defaults = host_lookup_variable(host, odef, defaults).unwrap_or(Value::NULL);
            let heap = host.heap();
            let descriptions = heap
                .foreign::<ContextDef>(def)
                .map_or(Value::NULL, ContextDef::grob_descriptions);
            for entry in list_items(heap, descriptions) {
                let (Some(name), Some(alist)) = (heap.car(entry), heap.cdr(entry)) else {
                    continue;
                };
                let mut items = list_items(heap, alist);
                let mut all = defaults;
                while let Some(item) = items.pop() {
                    all = heap.cons(item, all);
                }
                let props = GrobProperties::make(heap, all);
                t.arena.borrow_mut().set(GLOBAL, name, props);
            }
        }
        host.heap().unroot_to(root);
        ran.map(|()| Some(t))
    }

    /// `prepare`, Global's listener of `Prepare`: the moment `now` begins.
    pub fn prepare(&self, host: &mut dyn Host, now: Moment) -> Run<()> {
        let moment = host.heap().wrap(now);
        send(host, self, GLOBAL, "Prepare", &[("moment", moment)])
    }

    /// `apply_finalizations`: the `\once`s of the timestep undone.
    pub fn apply_finalizations(&self, host: &mut dyn Host) -> Run<()> {
        let all = core::mem::take(&mut self.arena.borrow_mut().finalizations);
        // Out of the arena, their values are kept by the root stack.
        let root = host.heap().root_depth();
        for f in &all {
            if let Finalization::Set(_, _, v) | Finalization::MatchedPop(_, _, v) = *f {
                host.heap().root(v);
            }
        }
        let ran = all.into_iter().try_for_each(|f| match f {
            Finalization::Set(c, sym, v) => set_property(host, self, c, sym, v),
            Finalization::Unset(c, sym) => unset_property(host, self, c, sym),
            Finalization::MatchedPop(c, sym, token) => {
                let heap = host.heap();
                crate::override_::matched_pop(heap, &self.arena.borrow(), c, sym, token);
                Ok(())
            }
        });
        host.heap().unroot_to(root);
        ran
    }
}

/// A definition's property operations, oldest first.
fn default_ops(heap: &mut Heap, def: Value) -> Value {
    let ops = heap
        .foreign::<ContextDef>(def)
        .map_or(Value::NULL, ContextDef::property_ops);
    let mut list = Value::NULL;
    for op in list_items(heap, ops) {
        list = heap.cons(op, list);
    }
    list
}

impl Arena {
    /// The output definition the translation runs in, its grobs' `layout`.
    pub fn output_def(&self) -> Value {
        self.output_def
    }

    /// N8's resume (D241): the new compile's output definition, which
    /// gives the contexts made from here on their definitions (the last
    /// compile's module is undone); the paths found with the last dropped.
    pub(crate) fn take_output_def(&mut self, heap: &mut Heap, odef: Value) {
        let old = core::mem::replace(&mut self.output_def, odef);
        if let Some(slot) = &mut self.grob_slot {
            slot.take_layout(heap, old, odef);
        }
        if let Some(p) = &self.performance {
            crate::performance::take_midi(heap, p.value, old, odef);
        }
        self.paths.clear();
    }

    /// `add_listener` of the class named `class`.
    pub(crate) fn add_listener(
        &mut self,
        heap: &mut Heap,
        d: DispId,
        listener: Listener,
        class: &str,
    ) {
        let sym = heap.intern(class);
        if let Some(class) = self.class_number(heap, sym) {
            self.dispatchers.add_listener(d, listener, class);
        }
    }

    /// The number of the class `sym`, made a class of its own if it has none.
    pub fn class_number(&mut self, heap: &mut Heap, sym: Value) -> Option<u8> {
        if let Some(class) = self.classes.number(sym) {
            return Some(class);
        }
        let list = heap.list(&[sym]);
        self.classes.class(heap, list)
    }

    /// `ly:make-event-class`'s list for the class `class`.
    pub fn class_list(&self, heap: &mut Heap, class: u8) -> Value {
        let set = self.classes.set(class);
        // Ancestors have smaller numbers: the class first, the root last.
        let mut list = Value::NULL;
        for c in set.iter() {
            list = heap.cons(self.classes.symbol(c), list);
        }
        list
    }
}

/// `send_stream_event`: an event of the class `class` with `props`,
/// broadcast by `c`'s event source.
pub fn send(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    class: &str,
    props: &[(&str, Value)],
) -> Run<()> {
    let heap = host.heap();
    let sym = heap.intern(class);
    let (d, list) = {
        let mut a = t.arena.borrow_mut();
        let number = a.class_number(heap, sym);
        let list = match number {
            Some(n) => a.class_list(heap, n),
            None => heap.list(&[sym]),
        };
        (a.event_source(c), list)
    };
    let mut alist = Value::NULL;
    for &(key, value) in props.iter().rev() {
        let key = heap.intern(key);
        let entry = heap.cons(key, value);
        alist = heap.cons(entry, alist);
    }
    let ev = make_event(heap, list, alist);
    dispatch(host, t, d, ev)
}

/// `Dispatcher::dispatch` of `ev` by `d`: each listener of its class, by
/// priority, those of another dispatcher when it is reached.
pub fn dispatch(host: &mut dyn Host, t: &Translation, d: DispId, ev: Value) -> Run<()> {
    dispatch_heard(host, t, d, ev, None)
}

/// [`dispatch`], with the event's digest for N8 once it is made: the
/// contexts that hear it are recorded (`partition.rs`).
fn dispatch_heard(
    host: &mut dyn Host,
    t: &Translation,
    d: DispId,
    ev: Value,
    mut digest: Option<u64>,
) -> Run<()> {
    let heap = host.heap();
    let root = heap.root_depth();
    heap.root(ev);
    heap.root(t.value);
    let class = heap.intern("class");
    let list = heap
        .foreign::<StreamEvent>(ev)
        .and_then(|e| e.property(heap, class));
    let targets = {
        let mut a = t.arena.borrow_mut();
        match list.and_then(|l| a.classes.class(heap, l)) {
            Some(n) => {
                let set = a.classes.set(n);
                Some(a.dispatchers.targets(d, set))
            }
            None => None,
        }
    };
    // An event a translator sent: no replay of its call.
    t.arena.borrow().unreplayable_in_call();
    if let Some(targets) = targets.as_ref().filter(|_| t.arena.borrow().noting()) {
        let recording = t.arena.borrow_mut().n8.take();
        if let Some(mut r) = recording {
            r.heard(host.heap(), &t.arena.borrow(), ev, &mut digest, targets);
            t.arena.borrow_mut().n8 = Some(r);
        }
    }
    let ran = match targets {
        None => diagnose(host, "warning", "Event class should be a list"),
        Some(targets) => targets.into_iter().try_for_each(|l| match l {
            Listener::Dispatch(d) => dispatch_heard(host, t, d, ev, digest),
            Listener::Context(c, handler) => handle(host, t, c, handler, ev),
            Listener::Translator(c, i, which) => {
                crate::translator::listen(host, t, c, i, which, ev)
            }
            Listener::Combine(n, which) => crate::lyrics::heard(host, t, n, which, ev),
            Listener::Scheme(proc) => call(host, t, proc, &[ev]).map(|_| ()),
        }),
    };
    host.heap().unroot_to(root);
    ran
}

/// Calls `proc` with `args` where the translation's Rust calls Scheme, then
/// sends what the call asked `ly:broadcast` to send, in order. A primitive
/// cannot dispatch, since a listener may be Scheme (D70), so `ly:broadcast`
/// queues; every caller of it in LilyPond's Scheme broadcasts last.
pub fn call(host: &mut dyn Host, t: &Translation, proc: Value, args: &[Value]) -> Run<Value> {
    let result = host.call(proc, args);
    let depth = host.heap().root_depth();
    let ran = match result {
        Ok(v) => {
            host.heap().root(v);
            loop {
                let queued = core::mem::take(&mut t.arena.borrow_mut().broadcasts);
                if queued.is_empty() {
                    break Ok(());
                }
                for &(_, ev) in &queued {
                    host.heap().root(ev);
                }
                if let Err(e) = queued
                    .into_iter()
                    .try_for_each(|(d, ev)| dispatch(host, t, d, ev))
                {
                    break Err(e);
                }
            }
        }
        Err(_) => Ok(()),
    };
    t.arena.borrow_mut().broadcasts.clear();
    host.heap().unroot_to(depth);
    ran.and(result)
}

/// A context's handler of an event.
fn handle(host: &mut dyn Host, t: &Translation, c: CtxId, handler: Handler, ev: Value) -> Run<()> {
    match handler {
        Handler::CreateContext => {
            // A context a translator made: no replay of that call.
            t.arena.borrow().unreplayable_in_call();
            create_context_from_event(host, t, c, ev)
        }
        Handler::RemoveContext => {
            let mut a = t.arena.borrow_mut();
            crate::translator::disconnect(host.heap(), &mut a, c);
            remove_context(&mut a, c);
            Ok(())
        }
        Handler::CreateChildTranslator => crate::translator::create_child_translator(host, t, ev),
        Handler::TimestepStart | Handler::OneTimeStep | Handler::Finish => {
            crate::translator::score_handler(host, t, c, handler)
        }
        Handler::ChangeParent => {
            let to = event_property(host.heap(), ev, "context");
            let to = host.heap().foreign::<ContextHandle>(to).map(|h| h.id);
            if let Some(to) = to {
                let mut a = t.arena.borrow_mut();
                disconnect_from_parent(&mut a, c);
                add_context(&mut a, to, c);
            }
            Ok(())
        }
        Handler::SetProperty | Handler::UnsetProperty => {
            let heap = host.heap();
            let sym = event_property(heap, ev, "symbol");
            // `from_scm<bool>`: only `#t` is true.
            let once = event_property(heap, ev, "once") == Value::TRUE;
            let value =
                (handler == Handler::SetProperty).then(|| event_property(heap, ev, "value"));
            let Some((sym, value)) = type_check(host, sym, value, "translation-type?")? else {
                return Ok(());
            };
            let mut a = t.arena.borrow_mut();
            if once {
                let f = match a.here(c, sym) {
                    Some(v) => Finalization::Set(c, sym, v),
                    None => Finalization::Unset(c, sym),
                };
                a.finalizations.push(f);
            }
            match value {
                Some(v) => a.set(c, sym, v),
                None => a.unset(c, sym),
            }
            Ok(())
        }
        Handler::Override | Handler::Revert => grob_property_event(host, t, c, handler, ev),
        Handler::AcknowledgeInfant => {
            let infant = event_property(host.heap(), ev, "context");
            let infant = host
                .heap()
                .foreign::<ContextHandle>(infant)
                .map_or(NONE, |h| h.id);
            t.arena.borrow_mut().infant = infant;
            Ok(())
        }
        Handler::Record => {
            crate::trace::heard(&mut t.arena.borrow_mut(), c, ev);
            Ok(())
        }
        Handler::Prepare => {
            let m = event_property(host.heap(), ev, "moment");
            if let Some(&m) = host.heap().foreign::<Moment>(m) {
                let mut a = t.arena.borrow_mut();
                a.prev = if a.prev.main.is_inf() && a.prev.main.is_negative() {
                    m
                } else {
                    a.now
                };
                a.now = m;
            }
            Ok(())
        }
    }
}

/// `Engraver_group::override` and `revert`: a push or pop of the event's
/// grob and path in `c`, or with `once` a temporary one, matched-popped at
/// the end of the timestep.
fn grob_property_event(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    handler: Handler,
    ev: Value,
) -> Run<()> {
    let heap = host.heap();
    let sym = event_property(heap, ev, "symbol");
    let path = event_property(heap, ev, "property-path");
    let once = event_property(heap, ev, "once") == Value::TRUE;
    let token = match (handler, once) {
        (Handler::Override, false) => {
            let value = event_property(heap, ev, "value");
            crate::override_::push(host, t, c, sym, path, value)?;
            return Ok(());
        }
        (Handler::Override, true) => {
            let value = event_property(heap, ev, "value");
            crate::override_::temporary_override(host, t, c, sym, path, value)?
        }
        (_, false) => {
            crate::override_::pop(heap, &mut t.arena.borrow_mut(), c, sym, path);
            return Ok(());
        }
        (_, true) => {
            crate::override_::temporary_revert(heap, &mut t.arena.borrow_mut(), c, sym, path)
        }
    };
    if token.tag() == Tag::Pair {
        let f = Finalization::MatchedPop(c, sym, token);
        t.arena.borrow_mut().finalizations.push(f);
    }
    Ok(())
}

/// `type_check_assignment` (`value` given) or `type_check_unset` of `sym`
/// by its `type_symbol` object property, as `%type-check` does it: the
/// symbol and value to set, or `None` with LilyPond's warning.
fn type_check(
    host: &mut dyn Host,
    sym: Value,
    value: Option<Value>,
    type_symbol: &str,
) -> Run<Option<(Value, Option<Value>)>> {
    // Without the prelude (a test's bare VM), any symbol is a property.
    let Some(check) = host.lily("%type-check") else {
        return Ok((sym.tag() == Tag::Symbol).then_some((sym, value)));
    };
    let ty = host.heap().intern(type_symbol);
    let unset = Value::bool(value.is_none());
    let result = host.call(check, &[sym, value.unwrap_or(Value::FALSE), ty, unset])?;
    let heap = host.heap();
    Ok(match (heap.car(result), heap.cdr(result)) {
        (Some(s), Some(v)) => Some((s, value.map(|_| v))),
        _ => None,
    })
}

/// `set_property`: `sym` set in `c` if its type allows `value`.
pub fn set_property(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    sym: Value,
    value: Value,
) -> Run<()> {
    if let Some((sym, Some(value))) = type_check(host, sym, Some(value), "translation-type?")? {
        t.arena.borrow_mut().set(c, sym, value);
    }
    Ok(())
}

/// `unset_property`: `sym` unset in `c` if it is a property.
pub fn unset_property(host: &mut dyn Host, t: &Translation, c: CtxId, sym: Value) -> Run<()> {
    if let Some((sym, _)) = type_check(host, sym, None, "translation-type?")? {
        t.arena.borrow_mut().unset(c, sym);
    }
    Ok(())
}

/// `apply_property_operations` of `ops`, oldest first: `push`, `pop`,
/// `assign`, `apply` and `unset`.
pub fn apply_property_operations(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    ops: Value,
) -> Run<()> {
    let root = host.heap().root_depth();
    host.heap().root(ops);
    let ran = (|| {
        for op in list_items(host.heap(), ops) {
            let heap = host.heap();
            let (Some(tag), Some(entry)) = (heap.car(op), heap.cdr(op)) else {
                continue;
            };
            let first = heap.car(entry).unwrap_or(Value::NULL);
            let rest = heap.cdr(entry).unwrap_or(Value::NULL);
            let second = heap.car(rest).unwrap_or(Value::NULL);
            match heap.name(tag) {
                Some("push") => {
                    let path = heap.cdr(rest).unwrap_or(Value::NULL);
                    crate::override_::push(host, t, c, first, path, second)?;
                }
                Some("pop") => {
                    let heap = host.heap();
                    crate::override_::pop(heap, &mut t.arena.borrow_mut(), c, first, rest);
                }
                Some("assign") => set_property(host, t, c, first, second)?,
                Some("unset") => unset_property(host, t, c, first)?,
                Some("apply") => {
                    let handle = t.arena.borrow().handle(c);
                    let mut args = alloc::vec![handle];
                    args.extend(list_items(host.heap(), rest));
                    call(host, t, first, &args)?;
                }
                _ => {}
            }
        }
        Ok(())
    })();
    host.heap().unroot_to(root);
    ran
}

/// `add_context`: `child` the last of `parent`'s children.
fn add_context(a: &mut Arena, parent: CtxId, child: CtxId) {
    a.contexts[parent as usize].children.push(child);
    a.contexts[child as usize].parent = parent;
    a.contexts[child as usize].init_mom = a.now;
    let (below, child_below) = (a.events_below(parent), a.events_below(child));
    a.dispatchers.register_as_listener(below, child_below);
}

/// `disconnect_from_parent`.
fn disconnect_from_parent(a: &mut Arena, c: CtxId) {
    let parent = a.ctx(c).parent;
    if parent == NONE {
        return;
    }
    let (below, child_below) = (a.events_below(parent), a.events_below(c));
    a.dispatchers.unregister_as_listener(below, child_below);
    a.contexts[parent as usize].children.retain(|&x| x != c);
    a.contexts[c as usize].parent = NONE;
}

/// `remove_context`: it leaves the tree, its record kept.
fn remove_context(a: &mut Arena, c: CtxId) {
    disconnect_from_parent(a, c);
    a.contexts[c as usize].removed = true;
}

/// `create_context_from_event`: the context the `CreateContext` event
/// `ev` asks for, made below `c`, its definition's and the event's property
/// operations applied, and announced.
fn create_context_from_event(host: &mut dyn Host, t: &Translation, c: CtxId, ev: Value) -> Run<()> {
    let heap = host.heap();
    let id = event_property(heap, ev, "id");
    let id = heap
        .str(id)
        .map(|s| String::from(s.as_str()))
        .unwrap_or_default();
    let ops = event_property(heap, ev, "ops");
    let ty = event_property(heap, ev, "type");
    let path = path_to_acceptable_context(host, t, c, ty);
    if path.len() != 1 {
        let name = String::from(host.heap().name(ty).unwrap_or_default());
        let text = format!("Invalid CreateContext event: Cannot create {name} context");
        return diagnose(host, "programming-error", &text);
    }
    let new = {
        let heap = host.heap();
        let mut a = t.arena.borrow_mut();
        let new = new_context(heap, &mut a, path[0], ops, false);
        a.contexts[new as usize].id = id;
        let source = a.event_source(new);
        for (handler, class) in [
            (Handler::CreateContext, "CreateContext"),
            (Handler::RemoveContext, "RemoveContext"),
            (Handler::ChangeParent, "ChangeParent"),
            (Handler::SetProperty, "SetProperty"),
            (Handler::UnsetProperty, "UnsetProperty"),
        ] {
            a.add_listener(heap, source, Listener::Context(new, handler), class);
        }
        let below = a.events_below(new);
        a.dispatchers.register_as_listener(below, source);
        add_context(&mut a, c, new);
        new
    };
    let defaults = default_ops(host.heap(), path[0]);
    apply_property_operations(host, t, new, defaults)?;
    apply_property_operations(host, t, new, ops)?;
    // The parent's group makes the new context's when it hears this.
    let handle = t.arena.borrow().handle(new);
    send(
        host,
        t,
        c,
        "AnnounceNewContext",
        &[("context", handle), ("creator", ev)],
    )
}

/// `path_to_acceptable_context`: the definitions of the contexts to make
/// below `c` down to one named `name`, or `Bottom`'s path.
pub fn path_to_acceptable_context(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    name: Value,
) -> Vec<Value> {
    let (odef, def, accepts, default) = {
        let a = t.arena.borrow();
        let ctx = a.ctx(c);
        (
            a.output_def,
            ctx.def,
            ctx.accepts.clone(),
            ctx.default_child,
        )
    };
    if name.tag() == Tag::Symbol && host.heap().name(name) == Some("Bottom") {
        return path_to_bottom_context(host, odef, default).unwrap_or_default();
    }
    let found = t.arena.borrow().paths.iter().find_map(|(d, a, n, path)| {
        (*d == def && *a == accepts && *n == name).then(|| path.clone())
    });
    if let Some(path) = found {
        return path;
    }
    let instantiable = find_def(host, odef, name).is_some();
    let mut search = Search {
        odef,
        name,
        instantiable,
        seen: Vec::new(),
        defs: Vec::new(),
    };
    let path = path_in(host, &mut search, def, &accepts);
    t.arena
        .borrow_mut()
        .paths
        .push((def, accepts, name, path.clone()));
    path
}

/// What [`path_in`] searches with: the output definition, the name sought
/// and whether a definition has it, the definitions on the way, and each
/// name's definition as [`find_def`] found it.
struct Search {
    odef: Value,
    name: Value,
    instantiable: bool,
    seen: Vec<Value>,
    defs: Vec<(Value, Option<Value>)>,
}

/// `Context_def::internal_path_to_acceptable_context` from the definition
/// `from` and the names it accepts, `accepted`: the shortest path, the
/// first found of its length.
fn path_in(
    host: &mut dyn Host,
    search: &mut Search,
    from: Value,
    accepted: &[Value],
) -> Vec<Value> {
    let mut accepteds = Vec::new();
    for &s in accepted {
        let def = match search.defs.iter().find(|(n, _)| *n == s) {
            Some(&(_, def)) => def,
            None => {
                let def = find_def(host, search.odef, s);
                search.defs.push((s, def));
                def
            }
        };
        accepteds.extend(def);
    }
    for &a in &accepteds {
        let heap = host.heap();
        let Some(d) = heap.foreign::<ContextDef>(a) else {
            continue;
        };
        let valid = if search.instantiable {
            heap.equal(d.name(), search.name)
        } else {
            d.is_alias(heap, search.name)
        };
        if valid {
            return alloc::vec![a];
        }
    }
    search.seen.push(from);
    let mut best = Vec::new();
    for &g in &accepteds {
        if !best.is_empty() && best.len() <= 2 {
            break;
        }
        if search.seen.contains(&g) {
            continue;
        }
        let heap = host.heap();
        let acc = list_items(
            heap,
            heap.foreign::<ContextDef>(g)
                .map_or(Value::NULL, ContextDef::accepts),
        );
        let result = path_in(host, search, g, &acc);
        if !result.is_empty() && (best.is_empty() || result.len() + 1 < best.len()) {
            best = alloc::vec![g];
            best.extend(result);
        }
    }
    search.seen.retain(|&s| s != from);
    best
}

/// `Context_def::path_to_bottom_context` from the default child `first`:
/// each default child down to a bottom context; `None` with LilyPond's
/// warning for a name with no definition or a cycle.
fn path_to_bottom_context(host: &mut dyn Host, odef: Value, first: Value) -> Option<Vec<Value>> {
    let mut path = Vec::new();
    let mut next = first;
    while next.tag() == Tag::Symbol {
        let Some(def) = find_def(host, odef, next) else {
            let text = format!(
                "cannot create default child context: {}",
                diagnostic_id(host.heap(), next, "")
            );
            let _ = diagnose(host, "warning", &text);
            return None;
        };
        if path.contains(&def) {
            let text = format!(
                "default child context begins a cycle: {}",
                diagnostic_id(host.heap(), next, "")
            );
            let _ = diagnose(host, "warning", &text);
            return None;
        }
        path.push(def);
        next = host
            .heap()
            .foreign::<ContextDef>(def)
            .map_or(Value::NULL, ContextDef::default_child);
    }
    Some(path)
}

/// `create_context`: a context of `def` with the id `id` and the mods
/// `ops`, made below `c` by a `CreateContext` event its event source
/// broadcasts, and found by the `AnnounceNewContext` that answers it.
pub fn create_context(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    def: Value,
    id: &str,
    ops: Value,
) -> Run<Option<CtxId>> {
    let heap = host.heap();
    let source = {
        let mut a = t.arena.borrow_mut();
        a.infant = NONE;
        let source = a.event_source(c);
        a.add_listener(
            heap,
            source,
            Listener::Context(c, Handler::AcknowledgeInfant),
            "AnnounceNewContext",
        );
        source
    };
    let name = heap
        .foreign::<ContextDef>(def)
        .map_or(Value::NULL, ContextDef::name);
    let id_value = heap.string(id);
    let sent = send(
        host,
        t,
        c,
        "CreateContext",
        &[("ops", ops), ("type", name), ("id", id_value)],
    );
    let infant = {
        let heap = host.heap();
        let mut a = t.arena.borrow_mut();
        let class = heap.intern("AnnounceNewContext");
        if let Some(class) = a.classes.number(class) {
            a.dispatchers.remove_listener(
                source,
                Listener::Context(c, Handler::AcknowledgeInfant),
                class,
            );
        }
        a.infant
    };
    sent?;
    if infant == NONE || t.arena.borrow().ctx(infant).parent != c {
        diagnose(
            host,
            "programming-error",
            "create_context: can't locate newly created context",
        )?;
        return Ok(None);
    }
    Ok(Some(infant))
}

/// `create_hierarchy`: the contexts of `path` made below `c`, a child that
/// adopts reused for each but the last, the others with
/// `intermediate_id`, the last with `leaf_id` and `leaf_ops`; `c` for no
/// path, `None` if one could not be made.
fn create_hierarchy(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    path: &[Value],
    intermediate_id: &str,
    leaf_id: &str,
    leaf_ops: Value,
) -> Run<Option<CtxId>> {
    let Some((&last, intermediate)) = path.split_last() else {
        return Ok(Some(c));
    };
    let mut leaf = c;
    for (i, &def) in intermediate.iter().enumerate() {
        let heap = host.heap();
        let name = |d: Value| {
            heap.foreign::<ContextDef>(d)
                .map_or(Value::NULL, ContextDef::name)
        };
        let (child, grandchild) = (name(def), name(path[i + 1]));
        leaf = match find_child_to_adopt_grandchild(host, t, leaf, child, grandchild) {
            Some(found) => found,
            None => match create_context(host, t, leaf, def, intermediate_id, Value::NULL)? {
                Some(made) => made,
                None => return Ok(None),
            },
        };
    }
    create_context(host, t, leaf, last, leaf_id, leaf_ops)
}

/// `find_child_to_adopt_grandchild`: a child of `c` named `child` (not an
/// alias) that adopts and accepts `grandchild` directly.
fn find_child_to_adopt_grandchild(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    child: Value,
    grandchild: Value,
) -> Option<CtxId> {
    let candidates: Vec<CtxId> = {
        let a = t.arena.borrow();
        let heap = host.heap();
        a.children(c)
            .iter()
            .copied()
            .filter(|&k| a.ctx(k).adopts && a.name(heap, k) == child)
            .collect()
    };
    candidates
        .into_iter()
        .find(|&k| path_to_acceptable_context(host, t, k, grandchild).len() == 1)
}

/// `core_find`: `c` itself, a context below it, a new one below it, or the
/// same from its parent, as `mode` and `dir` allow.
#[allow(clippy::too_many_arguments)]
fn core_find(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    mode: Mode,
    dir: Dir,
    name: Value,
    id: &str,
    ops: Value,
) -> Run<Option<CtxId>> {
    if mode != Mode::CreateOnly {
        let a = t.arena.borrow();
        let heap = host.heap();
        if a.matches(heap, c, name, id) {
            return Ok(Some(c));
        }
        if dir != Dir::Up {
            for &child in a.children(c) {
                if let Some(found) = a.find_only(heap, child, Dir::Down, name, id) {
                    return Ok(Some(found));
                }
            }
        }
    }
    if dir != Dir::Up && mode != Mode::FindOnly {
        let path = path_to_acceptable_context(host, t, c, name);
        if !path.is_empty() {
            let intermediate = if mode == Mode::CreateOnly {
                "\\new"
            } else {
                ""
            };
            return create_hierarchy(host, t, c, &path, intermediate, id, ops);
        }
    }
    let parent = t.arena.borrow().parent(c);
    match parent {
        Some(p) if dir != Dir::Down => core_find(host, t, p, mode, dir, name, id, ops),
        _ => Ok(None),
    }
}

/// `find`: `unchecked_find`, and a context the user may reach (not Global).
#[allow(clippy::too_many_arguments)]
fn find(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    mode: Mode,
    dir: Dir,
    name: Value,
    id: &str,
    ops: Value,
) -> Run<Option<CtxId>> {
    if mode != Mode::CreateOnly && dir == Dir::Center {
        let a = t.arena.borrow();
        let heap = host.heap();
        if let Some(found) = a
            .find_only(heap, c, Dir::Down, name, id)
            .or_else(|| a.find_only(heap, c, Dir::Up, name, id))
        {
            return Ok(Some(found).filter(|&f| f != GLOBAL));
        }
    }
    let found = if mode != Mode::FindOnly && name.tag() == Tag::Symbol {
        core_find(host, t, c, mode, dir, name, id, ops)?
    } else if mode != Mode::CreateOnly {
        core_find(host, t, c, Mode::FindOnly, dir, name, id, Value::NULL)?
    } else {
        None
    };
    Ok(found.filter(|&f| f != GLOBAL))
}

/// `find_create_context`: a context named `name` with the id `id` (empty
/// for any): `c`, one below it, one above it, a new one below it, or one
/// found or made below one above it, as far as `dir` goes.
pub fn find_create_context(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    dir: Dir,
    name: Value,
    id: &str,
    ops: Value,
) -> Run<Option<CtxId>> {
    find(host, t, c, Mode::FindCreate, dir, name, id, ops)
}

/// `create_unique_context`: [`find_create_context`] that only makes one.
pub fn create_unique_context(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    dir: Dir,
    name: Value,
    id: &str,
    ops: Value,
) -> Run<Option<CtxId>> {
    find(host, t, c, Mode::CreateOnly, dir, name, id, ops)
}

/// `get_default_interpreter`: `c` if it is a bottom context of the id
/// `id`, else a new bottom context below it, or `c` with LilyPond's
/// warning.
pub fn get_default_interpreter(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
    id: &str,
) -> Run<CtxId> {
    {
        let a = t.arena.borrow();
        if a.is_bottom(c) && (id.is_empty() || a.ctx(c).id == id) {
            return Ok(c);
        }
    }
    let bottom = host.heap().intern("Bottom");
    if let Some(found) = create_unique_context(host, t, c, Dir::Center, bottom, id, Value::NULL)? {
        return Ok(found);
    }
    let text = format!(
        "cannot find or create context: {}",
        diagnostic_id(host.heap(), bottom, id)
    );
    diagnose(host, "warning", &text)?;
    Ok(c)
}

/// `get_user_accessible_interpreter`: `c`, or for Global its Score, found
/// or made down its default children.
pub fn get_user_accessible_interpreter(
    host: &mut dyn Host,
    t: &Translation,
    c: CtxId,
) -> Run<Option<CtxId>> {
    if c != GLOBAL {
        return Ok(Some(c));
    }
    let (odef, default) = {
        let a = t.arena.borrow();
        (a.output_def, a.ctx(c).default_child)
    };
    let mut at = c;
    for def in path_to_bottom_context(host, odef, default).unwrap_or_default() {
        let name = host
            .heap()
            .foreign::<ContextDef>(def)
            .map_or(Value::NULL, ContextDef::name);
        match find_create_context(host, t, at, Dir::Down, name, "", Value::NULL)? {
            Some(found) if found != GLOBAL => return Ok(Some(found)),
            Some(found) => at = found,
            None => return Ok(None),
        }
    }
    Ok(None)
}

/// `check_removal`: each context below `c` whose iterators are done and
/// that has no children left is finalized and sent `RemoveContext`, the
/// deepest first.
pub fn check_removal(host: &mut dyn Host, t: &Translation, c: CtxId) -> Run<()> {
    let children = t.arena.borrow().children(c).to_vec();
    for child in children {
        check_removal(host, t, child)?;
        if t.arena.borrow().is_removable(child) {
            crate::translator::finalize(host, t, child)?;
            send(host, t, child, "RemoveContext", &[])?;
        }
    }
    Ok(())
}

// --- the Scheme face ------------------------------------------------------

/// The arena and number of the context `v`, if it is one.
pub(crate) fn handle_of(heap: &Heap, v: Value) -> Option<(Rc<RefCell<Arena>>, CtxId)> {
    heap.foreign::<ContextHandle>(v)
        .map(|h| (Rc::clone(&h.arena), h.id))
}

/// The translation of the context `v` if it is its context `id`
/// (`unsmob<Global_context>` for [`GLOBAL`]).
pub(crate) fn translation_of(heap: &Heap, v: Value, id: CtxId) -> Option<Translation> {
    heap.foreign::<ContextHandle>(v)
        .filter(|h| h.id == id)
        .map(|h| Translation {
            arena: Rc::clone(&h.arena),
            value: h.translation,
        })
}

/// The translation and number of the context `v`, or a type error naming
/// `subr` and the argument's position.
fn context(
    cx: &mut Cx,
    subr: &str,
    at: usize,
    v: Value,
) -> Result<(Rc<RefCell<Arena>>, CtxId), Thrown> {
    match cx.heap().foreign::<ContextHandle>(v) {
        Some(h) => Ok((Rc::clone(&h.arena), h.id)),
        None => Err(cx.wrong_type(subr, at, v, "Context")),
    }
}

fn symbol(cx: &mut Cx, subr: &str, at: usize, v: Value) -> Result<(), Thrown> {
    match v.tag() {
        Tag::Symbol => Ok(()),
        _ => Err(cx.wrong_type(subr, at, v, "symbol")),
    }
}

/// `(ly:context? x)`.
pub(crate) fn is_context(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<ContextHandle>(cx, args[0])
}

/// `(ly:context-alias? context name)`.
pub(crate) fn ly_context_alias(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context(cx, "ly:context-alias?", 1, args[0])?;
    symbol(cx, "ly:context-alias?", 2, args[1])?;
    Ok(Value::bool(a.borrow().is_alias(cx.heap(), c, args[1])))
}

/// `(ly:context-current-moment context)`.
pub(crate) fn ly_context_current_moment(cx: &mut Cx, args: &[Value]) -> R {
    let (a, _) = context(cx, "ly:context-current-moment", 1, args[0])?;
    let now = a.borrow().now;
    Ok(cx.heap_mut().wrap(now))
}

/// `(ly:context-id context)`.
pub(crate) fn ly_context_id(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context(cx, "ly:context-id", 1, args[0])?;
    let id = String::from(a.borrow().id(c));
    Ok(cx.heap_mut().string(id))
}

/// `(ly:context-name context)`.
pub(crate) fn ly_context_name(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context(cx, "ly:context-name", 1, args[0])?;
    Ok(a.borrow().name(cx.heap(), c))
}

/// `(ly:context-parent context)`: its parent, or `#f`.
pub(crate) fn ly_context_parent(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context(cx, "ly:context-parent", 1, args[0])?;
    let a = a.borrow();
    Ok(a.parent(c).map_or(Value::FALSE, |p| a.handle(p)))
}

/// `(ly:context-children context)`: a new list of its children.
pub(crate) fn ly_context_children(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context(cx, "ly:context-children", 1, args[0])?;
    let a = a.borrow();
    let handles: Vec<Value> = a.children(c).iter().map(|&k| a.handle(k)).collect();
    Ok(cx.heap_mut().list(&handles))
}

/// `(ly:context-find context name)`: `find_context_above`.
pub(crate) fn ly_context_find(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context(cx, "ly:context-find", 1, args[0])?;
    symbol(cx, "ly:context-find", 2, args[1])?;
    let a = a.borrow();
    Ok(a.find_context(cx.heap(), c, Dir::Up, args[1], "")
        .map_or(Value::FALSE, |f| a.handle(f)))
}

/// `(ly:context-event-source context)`.
pub(crate) fn ly_context_event_source(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context(cx, "ly:context-event-source", 1, args[0])?;
    let a = a.borrow();
    Ok(a.dispatcher_handles[a.event_source(c) as usize])
}

/// `(ly:context-events-below context)`.
pub(crate) fn ly_context_events_below(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context(cx, "ly:context-events-below", 1, args[0])?;
    let a = a.borrow();
    Ok(a.dispatcher_handles[a.events_below(c) as usize])
}

/// `(ly:context-output-def context)`.
pub(crate) fn ly_context_output_def(cx: &mut Cx, args: &[Value]) -> R {
    let (a, _) = context(cx, "ly:context-output-def", 1, args[0])?;
    Ok(a.borrow().output_def)
}

/// `(ly:context-property context name [alternative] [#:default d]
/// [#:search-ancestors? b])`: the value `name` has seen from `context`;
/// `alternative` for `'()`, `d` (or `alternative`) when it is not set.
pub(crate) fn ly_context_property(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:context-property";
    let (a, c) = context(cx, SUBR, 1, args[0])?;
    symbol(cx, SUBR, 2, args[1])?;
    let mut rest = &args[2..];
    let mut alternative = Value::NULL;
    if let Some(&first) = rest.first()
        && first.tag() != Tag::Keyword
    {
        alternative = first;
        rest = &rest[1..];
    }
    let (mut default, mut ancestors) = (alternative, true);
    while let [key, value, more @ ..] = rest {
        match cx.heap().name(*key) {
            Some("default") if key.tag() == Tag::Keyword => default = *value,
            Some("search-ancestors?") if key.tag() == Tag::Keyword => ancestors = !value.is_false(),
            _ => {
                return Err(cx.error(
                    "keyword-argument-error",
                    Some(SUBR),
                    "Unrecognized keyword",
                    &[],
                ));
            }
        }
        rest = more;
    }
    if !rest.is_empty() {
        return Err(cx.error("keyword-argument-error", Some(SUBR), "Invalid keyword", &[]));
    }
    let a = a.borrow();
    let found = a
        .where_defined(c, args[1])
        .filter(|&(at, _)| ancestors || at == c);
    Ok(match found {
        Some((_, v)) if v != Value::NULL => v,
        Some(_) => alternative,
        None => default,
    })
}

/// `(ly:context-property-where-defined context name [default])`.
pub(crate) fn ly_context_property_where_defined(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:context-property-where-defined";
    let (a, c) = context(cx, SUBR, 1, args[0])?;
    symbol(cx, SUBR, 2, args[1])?;
    let a = a.borrow();
    Ok(match a.where_defined(c, args[1]) {
        Some((at, _)) => a.handle(at),
        None => args.get(2).copied().unwrap_or(Value::NULL),
    })
}

/// `(%context-set-property! context name value)`, after `%type-check`.
pub(crate) fn context_set_property(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context(cx, "ly:context-set-property!", 1, args[0])?;
    a.borrow_mut().set(c, args[1], args[2]);
    Ok(Value::UNSPECIFIED)
}

/// `(%context-unset-property context name)`, after `%type-check`.
pub(crate) fn context_unset_property(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context(cx, "ly:context-unset-property", 1, args[0])?;
    a.borrow_mut().unset(c, args[1]);
    Ok(Value::UNSPECIFIED)
}

/// `(%context-check context subr)`: a type error unless `context` is one.
pub(crate) fn context_check(cx: &mut Cx, args: &[Value]) -> R {
    let subr = cx
        .heap()
        .str(args[1])
        .map(|s| String::from(s.as_str()))
        .unwrap_or_default();
    context(cx, &subr, 1, args[0])?;
    Ok(Value::UNSPECIFIED)
}

/// The translation and number of the dispatcher `v`.
fn dispatcher(
    cx: &mut Cx,
    subr: &str,
    at: usize,
    v: Value,
) -> Result<(Rc<RefCell<Arena>>, DispId), Thrown> {
    match cx.heap().foreign::<DispatcherHandle>(v) {
        Some(h) => Ok((Rc::clone(&h.arena), h.id)),
        None => Err(cx.wrong_type(subr, at, v, "Dispatcher")),
    }
}

/// `(ly:dispatcher? x)`.
pub(crate) fn is_dispatcher(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<DispatcherHandle>(cx, args[0])
}

/// `(ly:broadcast dispatcher event)`: `event` queued for `dispatcher` to
/// send when Scheme returns to the translation ([`call`]).
pub(crate) fn ly_broadcast(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:broadcast";
    let (a, d) = dispatcher(cx, SUBR, 1, args[0])?;
    if cx.heap().foreign::<StreamEvent>(args[1]).is_none() {
        return Err(cx.wrong_type(SUBR, 2, args[1], "Stream_event"));
    }
    a.borrow_mut().broadcasts.push((d, args[1]));
    Ok(Value::UNSPECIFIED)
}

/// `(ly:set-middle-C! context)` (`pitch-scheme.cc`): `middleCPosition`
/// set in `context` from `middleCClefPosition`, or `middleCCuePosition`
/// if it is a number, and `middleCOffset`.
pub(crate) fn ly_set_middle_c(cx: &mut Cx, args: &[Value]) -> R {
    let (a, c) = context(cx, "ly:set-middle-C!", 1, args[0])?;
    let heap = cx.heap_mut();
    let [clef, offset, cue] =
        ["middleCClefPosition", "middleCOffset", "middleCCuePosition"].map(|name| {
            let sym = heap.intern(name);
            a.borrow().get(c, sym)
        });
    let int = |v: Value| v.as_fixnum().unwrap_or(0);
    // `scm_is_number`, then `from_scm<int>`, which takes only integers.
    let clef = if heap.is_number(cue) {
        int(cue)
    } else {
        int(clef)
    };
    let position = Value::fixnum(clef + int(offset)).unwrap_or(Value::FALSE);
    let sym = heap.intern("middleCPosition");
    a.borrow_mut().set(c, sym, position);
    Ok(Value::UNSPECIFIED)
}

/// `(ly:add-listener callback dispatcher class …)`.
pub(crate) fn ly_add_listener(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:add-listener";
    if !cx.heap().is_procedure(args[0]) {
        return Err(cx.wrong_type(SUBR, 1, args[0], "procedure"));
    }
    let (a, d) = dispatcher(cx, SUBR, 2, args[1])?;
    for (i, &class) in args[2..].iter().enumerate() {
        symbol(cx, SUBR, i + 3, class)?;
        let mut a = a.borrow_mut();
        if let Some(n) = a.class_number(cx.heap_mut(), class) {
            a.dispatchers.add_listener(d, Listener::Scheme(args[0]), n);
        }
    }
    Ok(Value::UNSPECIFIED)
}

/// `(ly:connect-dispatchers to from)`: `to` hears `from`.
pub(crate) fn ly_connect_dispatchers(cx: &mut Cx, args: &[Value]) -> R {
    let (a, to) = dispatcher(cx, "ly:connect-dispatchers", 1, args[0])?;
    let (_, from) = dispatcher(cx, "ly:connect-dispatchers", 2, args[1])?;
    a.borrow_mut().dispatchers.register_as_listener(to, from);
    Ok(Value::UNSPECIFIED)
}

/// `(ly:disconnect-dispatchers to from)`.
pub(crate) fn ly_disconnect_dispatchers(cx: &mut Cx, args: &[Value]) -> R {
    let (a, to) = dispatcher(cx, "ly:disconnect-dispatchers", 1, args[0])?;
    let (_, from) = dispatcher(cx, "ly:disconnect-dispatchers", 2, args[1])?;
    a.borrow_mut().dispatchers.unregister_as_listener(to, from);
    Ok(Value::UNSPECIFIED)
}

/// `(ly:listened-event-types dispatcher)`: the classes it has listeners
/// for, as symbols.
pub(crate) fn ly_listened_event_types(cx: &mut Cx, args: &[Value]) -> R {
    let (a, d) = dispatcher(cx, "ly:listened-event-types", 1, args[0])?;
    let a = a.borrow();
    let symbols: Vec<Value> = a
        .dispatchers
        .listened(d)
        .iter()
        .map(|c| a.classes.symbol(c))
        .collect();
    Ok(cx.heap_mut().list(&symbols))
}

/// `(ly:listened-event-class? dispatcher class-list)`.
pub(crate) fn ly_listened_event_class(cx: &mut Cx, args: &[Value]) -> R {
    let (a, d) = dispatcher(cx, "ly:listened-event-class?", 1, args[0])?;
    let a = a.borrow();
    let listened = a.dispatchers.listened(d);
    let heap = cx.heap();
    Ok(Value::bool(list_items(heap, args[1]).into_iter().any(
        |s| a.classes.number(s).is_some_and(|n| listened.contains(n)),
    )))
}

/// The definitions in `(lily)`: `%type-check`, LilyPond's `internal_type_check`
/// (`lily-guile.cc`) without its deprecated properties, and the primitives
/// that check a property's type before they set it. `ly:context-property`
/// has `ly:context-set-property!` as its setter.
pub(crate) const PRELUDE: &str = r#"
(define (%print-scm-val val)
  (let ((s (call-with-output-string (lambda (port) (write val port)))))
    (if (> (string-length s) 200)
        (string-append (substring s 0 100) "\n :\n :\n" (substring s (- (string-length s) 100)))
        s)))
(define (%value-type-check sym val type-symbol type)
  (cond ((or (null? val) (not val) (unspecified? val)) #t)
        ((and (eq? type-symbol 'backend-type?) (procedure? val)) #t)
        ((and (eq? type-symbol 'backend-type?) (ly:unpure-pure-container? val))
         (and (%value-type-check sym (ly:unpure-pure-container-unpure-part val) type-symbol type)
              (%value-type-check sym (ly:unpure-pure-container-pure-part val) type-symbol type)))
        ((type val) #t)
        (else
         (ly:warning (G_ "the property '~a' must be of type '~a', ignoring invalid value '~a'")
                     sym (type-name type) (%print-scm-val val))
         #f)))
(define (%type-check sym val type-symbol unset)
  (and (symbol? sym)
       (let ((type (object-property sym type-symbol)))
         (if (procedure? type)
             (and (or unset (%value-type-check sym val type-symbol type))
                  (cons sym val))
             (begin
               (ly:warning (G_ "the property '~a' does not exist (perhaps a typing error)") sym)
               #f)))))
(define-public (ly:context-set-property! context name val)
  (%context-check context "ly:context-set-property!")
  (%check-type "ly:context-set-property!" 2 symbol? "symbol" name)
  (let ((checked (%type-check name val 'translation-type? #f)))
    (if checked (%context-set-property! context (car checked) (cdr checked))))
  *unspecified*)
(define-public (ly:context-unset-property context name)
  (%context-check context "ly:context-unset-property")
  (%check-type "ly:context-unset-property" 2 symbol? "symbol" name)
  (let ((checked (%type-check name #f 'translation-type? #t)))
    (if checked (%context-unset-property context (car checked))))
  *unspecified*)
(define ly:context-property
  (make-procedure-with-setter ly:context-property ly:context-set-property!))
(define-public (ly:context-pushpop-property context grob eltprop . val)
  (%context-check context "ly:context-pushpop-property")
  (%check-type "ly:context-pushpop-property" 2 symbol? "symbol" grob)
  (%check-type "ly:context-pushpop-property" 3 symbol? "symbol" eltprop)
  (if (pair? val)
      (if (%context-grob-create! context grob)
          (let ((checked (%type-check eltprop (car val) 'backend-type? #f)))
            (if checked
                (%context-push-property! context grob (list (car checked)) (cdr checked)))))
      (%context-pop-property! context grob (list eltprop)))
  *unspecified*)
"#;
