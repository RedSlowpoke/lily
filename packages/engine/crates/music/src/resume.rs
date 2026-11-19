//! N8's snapshots (04 §10.2, §10.4; MS6 step 5's rest, D241): a recorded
//! translation's state kept at each checkpoint, and the translation
//! restored at one and run on from it.
//!
//! - **A snapshot** ([`Snapshot`]) is taken where the checkpoint's digest
//!   is (after the timestep's `Prepare`): the arena copied (contexts and
//!   their properties, the translators' `Work`, the dispatchers, the trace
//!   recorder, the playback map, N8's recording, and in rows it shares with
//!   the run, [`Rows`], D259, the grob and audio records and layout's grobs),
//!   the iterators, in rows too, the timestep's moment and whether its iterators run,
//!   the error port's length, and a mark in the heap's undo log ([`Heap::checkpoint_mark`]): from it on the
//!   first change of each object made before it is logged, so undoing to
//!   it gives every object what it held there. Outside a logged item (a
//!   translation of no compile) there is no mark, and no restore.
//! - **A restore** ([`restore`]) undoes the heap to the snapshot's mark,
//!   puts the copied arena in place of the arena (the one every handle the
//!   translation made names) and the iterators in place of the
//!   iteration's, and the global loop goes on from the timestep's
//!   processing (`global.rs`). The snapshots after it are dropped: their
//!   marks are undone.
//! - **Across compiles** ([`Resumable`], [`resume`], D242): a translation
//!   is kept for the next compile's translation at its place, its object
//!   held by the engine, its objects' changes after each mark taken out of
//!   the compile's log as pictures (`Heap::keep_pictures`) before the
//!   compile is undone. The next translation probes its first state (a
//!   translation of the new music made for it and dropped, its messages
//!   taken back); where that is the old one's, the trace begun the same
//!   and the shapes line up, it resumes at the last pure snapshot before
//!   `m_e` (no older object changed for good by then, but the placed
//!   messages' cell) whose iterators the new music takes: the old
//!   translation's own objects given back what they held there
//!   (`Heap::give_back`, by the epochs it made them in), the arena the
//!   snapshot's with the new output definition (its grobs', Paper_score's
//!   and Performance's too), N8's recording on the new shape, and what it
//!   said before the snapshot said again. The engine passes the kept
//!   translations only to a compile of the same root (D244 moves the places
//!   of an edit), and drops them after a compile that failed. A compile a
//!   newer one stopped leaves them as they were (B24, D261): the old arena is
//!   kept whole beside the resumed one ([`Resumable::stopped`]), the links this
//!   run moved are copies, and the objects it gave back and the places the
//!   edit moved take what they held before it (`Heap::put_given_back`), so the
//!   next compile's edit is from the text they are of.
//! - **Where they are taken** ([`Place`], D258): only where a compile can use
//!   one. A run from the start spreads at most [`KEPT`] over the score,
//!   taking fewer as it goes rather than copying and dropping. A resumed run
//!   takes one where it begins, in place of the one it resumed from, so that
//!   an edit in one place never finds them aged out; one at the last
//!   checkpoint before its edit; and two past it, where the next edit there
//!   can splice.
//! - **The splice** ([`Splice`], [`splice`], D243): each recorded
//!   translation keeps its end ([`ended`]: its arena copied before layout,
//!   its placed messages, the heap's mark), and a resume keeps that, the
//!   snapshots after the one it resumed from and the end values of the
//!   objects it gave back. Where the resumed run converges with the old
//!   one at a checkpoint and the two line up there ([`lines_up`]), the
//!   translation ends as the old run did: its arena the old run's end, with
//!   this run's records where the edit changed them, and the rest spliced
//!   to match; else it runs on to its end. Where the music is the old run's
//!   it splices as it begins ([`splice_same`], D258).
//! - **The check** ([`CHECK`], the harness's `--n8`): each recorded
//!   translation is restored at its middle checkpoint and at its first and
//!   run on to its end each time, and what N8 keeps of it, its records and
//!   its trace must be the whole run's, a spliced one's too; what layout
//!   and the pages are made from is the last of those runs (so the
//!   harness's wasm run and its native one without `--n8` are what show
//!   layout of a splice).

use alloc::boxed::Box;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::Cell;
use core::sync::atomic::{AtomicBool, Ordering};

use lily_engine_scheme::{Heap, Rows, Tracer, Value};

use crate::Moment;
use crate::checkpoint::Kept;
use crate::context::{Arena, Host, Translation};
use crate::iterator::{It, Iteration};

/// Whether each recorded translation is restored and run on to check its
/// snapshots ([`check`]).
pub static CHECK: AtomicBool = AtomicBool::new(false);

/// Sets [`CHECK`].
pub fn set_check(on: bool) {
    CHECK.store(on, Ordering::Relaxed);
}

/// Never in wasm, where nothing sets [`CHECK`]: N8's checks are the
/// native harness's (MS7 step 5, D262).
pub(crate) fn checking() -> bool {
    !cfg!(target_arch = "wasm32") && CHECK.load(Ordering::Relaxed)
}

/// Where a run takes its snapshots (D258). Each costs its checkpoint's digests
/// and a mark that logs again the first change of every older object, what
/// keeps memory now that their records are shared (D259), and a compile can use
/// few: the last before its edit to resume from, and those after it to splice
/// at.
#[derive(Clone, Debug)]
pub(crate) enum Place {
    /// A run from the start, which knows of no edit: every `stride`th of the
    /// checkpoints met so far, spread over the score as they thin out.
    Even { stride: u32, met: u32 },
    /// A run resumed for an edit at `edit`: at the last checkpoint before it,
    /// `before`, if that is past where it resumed, where the next edit near
    /// it resumes from; and at the [`AFTER`]th checkpoints past it, where
    /// that one splices.
    Edit {
        before: Option<Moment>,
        edit: Moment,
        past: u32,
    },
    /// None but the one it began with.
    Nowhere,
}

impl Default for Place {
    fn default() -> Place {
        Place::Even { stride: 1, met: 0 }
    }
}

/// The checkpoints past its edit at which a resumed run takes a snapshot.
const AFTER: [u32; 2] = [2, 4];

impl Place {
    /// Whether the checkpoint at `moment`, the next it meets, gets a snapshot.
    pub(crate) fn wants(&mut self, moment: Moment) -> bool {
        match self {
            Place::Even { stride, met } => {
                *met += 1;
                (*met - 1).is_multiple_of(*stride)
            }
            Place::Edit { before, edit, past } => {
                if *before == Some(moment) {
                    return true;
                }
                if moment.compare(*edit).is_gt() {
                    *past += 1;
                    return AFTER.contains(past);
                }
                false
            }
            Place::Nowhere => false,
        }
    }

    fn is_before(&self, moment: Moment) -> bool {
        matches!(self, Place::Edit { before: Some(m), .. } if *m == moment)
    }
}

/// A translation at a checkpoint.
pub(crate) struct Snapshot {
    /// The timestep, by number, its moment, and whether its iterators
    /// run in it.
    pub(crate) step: usize,
    pub(crate) moment: Moment,
    pub(crate) ok: bool,
    arena: Box<Arena>,
    its: Rows<It>,
    /// The heap's undo log's length there, and the error port's; the
    /// generation of pictures its compile keeps (`Heap::keep_pictures`).
    mark: usize,
    errors: usize,
    generation: u32,
    /// The heap's epoch from the mark on: what is older than it, the run
    /// that resumes from it shares (OPEN C18).
    epoch: u32,
    /// Where its translation's log was there (MS6 step 7b).
    log: crate::replay::Mark,
    /// Whether the translation had changed nothing older than it for good
    /// by then: only such a snapshot is resumed from in another compile.
    pure: Cell<bool>,
    /// The placed messages it had said by then (`%diagnostics`' entries,
    /// the oldest first), each with its place in the error port from where
    /// the translation began.
    diagnostics: Vec<([Value; 3], usize)>,
}

impl Snapshot {
    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        self.arena.trace_copy(tracer);
        for (d, _) in &self.diagnostics {
            for &v in d {
                tracer.trace(v);
            }
        }
        Iteration::trace(&self.its, tracer);
    }
}

/// Takes the snapshot of timestep `step` at `moment`, if the heap is
/// logged: the arena as it is now, with N8's recording in it.
pub(crate) fn take(
    host: &mut dyn Host,
    t: &Translation,
    it: &Iteration,
    (step, moment, ok): (usize, Moment, bool),
) {
    // What the translation had said by now, from where it began.
    let errors = host
        .error_len()
        .saturating_sub(t.arena.borrow().errors_base);
    let pure = pure_since(host, t);
    let generation = host.heap().generation();
    let diagnostics = said(host, t);
    let Some(mark) = host.heap().checkpoint_mark() else {
        return;
    };
    let epoch = host.heap().epoch();
    let arena = Box::new(t.arena.borrow().snapshot());
    let log = arena
        .log
        .as_ref()
        .map_or_else(crate::replay::Mark::default, |l| l.borrow().mark());
    let snapshot = Snapshot {
        step,
        moment,
        ok,
        arena,
        its: it.state(),
        mark,
        errors,
        generation,
        epoch,
        log,
        pure: Cell::new(pure),
        diagnostics,
    };
    let mut a = t.arena.borrow_mut();
    a.snapshots.push(Rc::new(snapshot));
    // A replay's are few, and numbered by its points.
    if a.replay.is_none() {
        thin(&mut a, generation, moment);
    }
}

/// Whether the translation `t` has changed nothing older than it for good
/// since it began or resumed: neither an object, but the placed messages' cell,
/// which a resume says again ([`say_again`]), nor a global.
fn pure_since(host: &mut dyn Host, t: &Translation) -> bool {
    let base = t.arena.borrow().resume_base;
    let cell = diagnostics_cell(host).unwrap_or(Value::NULL);
    host.globals_written() == base.globals
        && !host.heap().changed_older_since(base.log, base.since, cell)
}

/// At most [`KEPT`] snapshots (a translation and the one kept for it are in
/// memory at once, and each keeps its mark's pictures: T24's 50 overran wasm's
/// 256 MiB, and at 16 they were most of a score's memory, D255). A run from
/// the start drops every other one, so they thin out over the score and it
/// takes every other one from then on. A resumed run drops the one it began
/// with once it has one nearer its edit, then the earliest of the last
/// compiles' (D258).
fn thin(a: &mut Arena, generation: u32, moment: Moment) {
    let Some(place) = a.n8.as_mut().map(|r| &mut r.place) else {
        return;
    };
    let n = a.snapshots.len();
    if place.is_before(moment) && n >= 2 && a.snapshots[n - 2].generation == generation {
        a.snapshots.remove(n - 2);
    }
    if a.snapshots.len() <= KEPT {
        return;
    }
    if let Place::Even { stride, .. } = place {
        *stride *= 2;
        let mut k = 0;
        a.snapshots.retain(|_| {
            k += 1;
            k % 2 == 1
        });
    } else {
        let older = a.snapshots.iter().position(|s| s.generation != generation);
        a.snapshots.remove(older.unwrap_or(0));
    }
}

/// The snapshots a translation keeps at most (D255: 16 before MS6 step 12).
/// With their records shared, more can be had, and buy nothing: 8 leaves every
/// edit of the four within 5 % of the engine's time and takes T29's memory
/// from 125 MiB to 140 (D259).
const KEPT: usize = 4;

/// The `(lily)` cell whose `car` is the placed messages said so far, the
/// newest first (the parser's `%diagnostics`).
fn diagnostics_cell(host: &mut dyn Host) -> Option<Value> {
    host.lily("%diagnostics")
        .filter(|&c| host.heap().car(c).is_some())
}

/// The placed messages the translation `t` has said, from where it began
/// (`Arena::diagnostics_base`), the oldest first, each with its place in
/// the error port from there.
fn said(host: &mut dyn Host, t: &Translation) -> Vec<([Value; 3], usize)> {
    let (base, errors) = {
        let a = t.arena.borrow();
        (a.diagnostics_base, a.errors_base)
    };
    let Some(cell) = diagnostics_cell(host) else {
        return Vec::new();
    };
    let heap = host.heap();
    let mut out = Vec::new();
    let mut p = heap.car(cell).unwrap_or(Value::NULL);
    while p != base
        && let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p))
    {
        if let Some(&[kind, location, message, mark]) = heap.vector_items(item) {
            let mark = mark.as_fixnum().map_or(0, |m| m.max(0) as usize);
            out.push(([kind, location, message], mark.saturating_sub(errors)));
        }
        p = rest;
    }
    out.reverse();
    out
}

/// The translation `t` ends (before layout): itself copied as it is, the
/// heap's mark and the placed messages it said, for a splice of it in the
/// next compile (D243). Only a recorded translation is kept.
pub(crate) fn ended(host: &mut dyn Host, t: &Translation) {
    if t.arena.borrow().n8.is_none() {
        return;
    }
    let placed = said(host, t);
    let end = (host.heap().generation(), host.heap().undo_len());
    let pure = pure_since(host, t);
    let mut a = t.arena.borrow_mut();
    let mut copy = a.snapshot();
    copy.placed_at_end = placed;
    copy.end_pure = pure;
    a.at_end = Some(Rc::new(copy));
    a.end_mark = Some(end);
}

/// Where a translation begins or resumes: the placed messages said before
/// it, for [`said`].
pub(crate) fn diagnostics_now(host: &mut dyn Host) -> Value {
    diagnostics_cell(host)
        .and_then(|c| host.heap().car(c))
        .unwrap_or(Value::NULL)
}

/// The placed messages said put back to `list`, what [`diagnostics_now`]
/// gave: a probe's undone.
pub(crate) fn unsay(host: &mut dyn Host, list: Value) {
    if let Some(cell) = diagnostics_cell(host) {
        host.heap().set_car(cell, list);
    }
}

/// The placed messages `said` said again, their places in the error port
/// from `errors` on (D241).
fn say_again(host: &mut dyn Host, said: &[([Value; 3], usize)], errors: usize) {
    let Some(cell) = diagnostics_cell(host) else {
        return;
    };
    for &([kind, location, message], at) in said {
        let heap = host.heap();
        let mark = heap.integer((errors + at) as i64);
        let item = heap.vector(alloc::vec![kind, location, message, mark]);
        let list = heap.car(cell).unwrap_or(Value::NULL);
        let list = heap.cons(item, list);
        heap.set_car(cell, list);
    }
}

/// N8's replay (MS6 step 7b): the snapshots after the `k`th not resumed
/// from in another compile (a replayed part's translators' state was not
/// kept in them).
pub(crate) fn impure_after(t: &Translation, k: usize) {
    for s in t.arena.borrow().snapshots.iter().skip(k + 1) {
        s.pure.set(false);
    }
}

/// The snapshots the translation took in the running compile, by number.
pub(crate) fn this_compiles(heap: &Heap, t: &Translation) -> Vec<usize> {
    let generation = heap.generation();
    let a = t.arena.borrow();
    (0..a.snapshots.len())
        .filter(|&k| a.snapshots[k].generation == generation)
        .collect()
}

/// Restores the translation at its snapshot `k`: the heap undone to its
/// mark, the arena and the iterators its copies; the snapshots after it
/// dropped. Its moment and whether its iterators run, for the loop, and
/// its timestep.
pub(crate) fn restore(
    host: &mut dyn Host,
    t: &Translation,
    it: &mut Iteration,
    k: usize,
) -> Option<((Moment, bool), usize)> {
    let mut snapshots = core::mem::take(&mut t.arena.borrow_mut().snapshots);
    let s = snapshots.get(k)?;
    host.heap().undo(s.mark);
    let base = t.arena.borrow().errors_base;
    host.cut_errors(base + s.errors);
    let at = ((s.moment, s.ok), s.step);
    if let Some(log) = &s.arena.log {
        log.borrow_mut().truncate(&s.log);
    }
    let mut arena = s.arena.snapshot();
    it.put_back(&s.its);
    snapshots.truncate(k + 1);
    arena.snapshots = snapshots;
    arena.resumed_from = t.arena.borrow_mut().resumed_from.take();
    // Layout's slot is filled after the translation, never before.
    *t.arena.borrow_mut() = arena;
    Some(at)
}

/// What the check compares of a translation that ran to its end: what N8
/// keeps of it, its records and its trace. Not the grob digests layout's
/// cells are keyed by (D248): one that differs only costs a cell's reuse.
pub(crate) fn summary(heap: &Heap, a: &Arena, with_kept: bool) -> String {
    let kept = a.n8.clone().filter(|_| with_kept).map(|r| {
        let mut kept = r.finish(heap, a);
        kept.grobs.clear();
        kept
    });
    let trace = a.recorder.clone().map(|r| r.into_trace());
    format!(
        "{kept:?}\n{} grobs {} audio\n{}{}",
        a.records.grobs.len(),
        a.records.audio.len(),
        trace.as_ref().map_or("", |t| t.text.as_str()),
        trace.as_ref().map_or("", |t| t.grobs.as_str()),
    )
}

/// The live values of `snapshots`, for an arena's trace.
pub(crate) fn trace(snapshots: &[Rc<Snapshot>], tracer: &mut Tracer<'_>) {
    for s in snapshots {
        s.trace(tracer);
    }
}

/// What the check found the restored run to differ in: where the two
/// summaries part, with a little of each.
pub(crate) fn differs(k: usize, step: usize, whole: &str, resumed: &str) -> Option<String> {
    if whole == resumed {
        return None;
    }
    let at = whole
        .bytes()
        .zip(resumed.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(whole.len().min(resumed.len()));
    let around = |s: &str| {
        let from = s.floor_char_boundary(at.saturating_sub(60));
        let to = s.floor_char_boundary((at + 60).min(s.len()));
        s[from..to].replace(['\n', '\t'], " ")
    };
    Some(format!(
        "restored at snapshot {k} (timestep {step}) and run on: differs from the whole run at byte {at} of its summary: {:?} against {:?}",
        around(whole),
        around(resumed)
    ))
}

/// What a kept translation began with, made and said: the trace it began
/// with (text and grobs), the epochs of its objects, its error port's text.
pub(crate) type Begun = (Option<(String, String)>, Vec<(u32, u32)>, String);

/// A translation kept for the next compile's (D241): its arena, with its
/// snapshots; what N8 kept of it; its top iterator; and the trace it began
/// with, if traced.
pub struct Resumable {
    t: Translation,
    kept: Rc<Kept>,
    top: crate::iterator::IterId,
    trace: Option<(String, String)>,
    /// The epochs its objects were made in, in each compile it ran in.
    epochs: Vec<(u32, u32)>,
    /// What it wrote to the error port, which a resume writes again as far
    /// as its snapshot.
    said: String,
}

impl Resumable {
    /// Whether what it found says a next compile's translation can be
    /// replayed (MS6 step 7c).
    pub(crate) fn replayable(&self) -> bool {
        let parts = &self.kept.parts;
        crate::replay::on() && parts.merged_all.is_none() && crate::replay::pays(parts.width())
    }

    pub(crate) fn new(
        t: Translation,
        kept: Kept,
        top: crate::iterator::IterId,
        (trace, epochs, said): Begun,
    ) -> Resumable {
        Resumable {
            t,
            kept: Rc::new(kept),
            top,
            trace,
            epochs,
            said,
        }
    }

    /// What a stopped compile's resume took the place of put back (B24, D261):
    /// the compile that ended is what the next one resumes.
    pub fn stopped(&self) {
        let from = self.t.arena.borrow_mut().resumed_from.take();
        if let Some(from) = from {
            *self.t.arena.borrow_mut() = *from;
        }
    }

    /// The compile that made it has ended: what it resumed is of no more use.
    pub fn ended(&self) {
        self.t.arena.borrow_mut().resumed_from = None;
    }

    /// Its translation's Scheme object, which the engine holds between
    /// compiles.
    pub fn value(&self) -> lily_engine_scheme::Value {
        self.t.value
    }

    /// The generation of pictures its earliest snapshot needs.
    pub fn generation(&self) -> u32 {
        let a = self.t.arena.borrow();
        a.snapshots
            .iter()
            .map(|s| s.generation)
            .min()
            .unwrap_or(u32::MAX)
    }

    /// Its snapshots from the last [`GENERATIONS`] compiles only, the
    /// running one `generation`'s among them. A resumed run keeps the
    /// snapshots before its edit, so without this the earliest of them
    /// would live for ever, and with it every compile's pictures and what
    /// they hold (OPEN C15: T02 0.7 MiB a compile, T24 2.5, until the
    /// instance traps). Those near the edit are taken again by each compile
    /// (D258); an edit before all that are left runs whole, which spreads
    /// them again.
    pub fn forget_old(&self, generation: u32) {
        let oldest = generation.saturating_sub(GENERATIONS - 1);
        self.t
            .arena
            .borrow_mut()
            .snapshots
            .retain(|s| s.generation >= oldest);
    }
}

/// The compiles a snapshot serves, so what its pictures hold is bounded by
/// them (D255).
const GENERATIONS: u32 = 4;

/// What a resumed translation splices in where it converges (D243): the
/// last compile's run of it as it ended, its snapshots after the one
/// resumed from, what N8 kept of it, the end values of its objects that
/// the resume gave back (a range of the heap's redo store), and what it
/// wrote to the error port.
pub(crate) struct Splice {
    end: Rc<Arena>,
    later: Vec<Rc<Snapshot>>,
    kept: Rc<Kept>,
    redo: core::ops::Range<usize>,
    /// The epoch of the snapshot resumed from: the objects older than it
    /// are this run's as well as the old run's (OPEN C18).
    since: u32,
    said: String,
    /// The old run's log whole, which a replay reads (MS6 step 7b).
    pub(crate) log: Option<crate::replay::Shared>,
}

impl Splice {
    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        self.end.trace(tracer);
        trace(&self.later, tracer);
    }

    /// Whether the old run has a snapshot at `moment` to splice at.
    pub(crate) fn has(&self, moment: Moment) -> bool {
        self.later.iter().any(|s| s.moment == moment)
    }

    /// The handle the old run made for its grob record `id`, if it did.
    pub(crate) fn handle(&self, id: u32) -> Option<Value> {
        let h = self.end.records.grobs.get(id as usize)?.handle;
        (h != Value::FALSE).then_some(h)
    }

    /// The old run's parts of this run's contexts `cs` (N8's replay, MS6
    /// step 7b), by their keys, the shared level's left out.
    pub(crate) fn parts_of(&self, a: &Arena, cs: &[crate::context::CtxId]) -> Vec<u32> {
        let (Some(new), Some(old)) = (a.n8.as_ref(), self.end.n8.as_ref()) else {
            return Vec::new();
        };
        let (part_of, _, _) = old.parts.by_key(&self.end);
        let mut parts: Vec<u32> = cs
            .iter()
            .filter_map(|&c| part_of.get(&new.parts.key(c)).copied())
            .filter(|&p| p != 0)
            .collect();
        parts.sort_unstable();
        parts.dedup();
        parts
    }

    /// The parts the old run's replay found could not be replayed, each with
    /// the last moment it failed (MS6 step 7c).
    pub(crate) fn failed_before(&self) -> Vec<(u32, Moment)> {
        self.end.failed_parts.clone()
    }

    /// A replay of the old run (MS6 step 7b), every partition replayed but
    /// `running`'s, if any can be; the old run's log's calls counted from
    /// this run's log's length `from` at its timestep `step`, where it had
    /// made so many grob and audio records.
    pub(crate) fn replay(
        &self,
        running: &[(u32, Moment)],
        step: u32,
        from: u32,
        made: (u32, u32),
    ) -> Option<Box<crate::replay::Replay>> {
        let log = self.log.clone()?;
        let mut r = crate::replay::Replay::new(&self.end, &self.kept, log, running)?;
        r.begin_at(step, from, made);
        Some(Box::new(r))
    }
}

/// The old run as [`lines_up`] reads it: its snapshot at the checkpoint,
/// `then`, with the generation and mark of the heap's log it was taken at;
/// its arena as it ended; the objects the resume gave back, a range of the
/// redo store; the epoch of the snapshot resumed from.
struct Old<'a> {
    then: &'a Arena,
    end: &'a Arena,
    at: (u32, usize),
    redo: core::ops::Range<usize>,
    since: u32,
}

/// Whether a run `new` at a checkpoint has made the records the old run
/// had made by its checkpoint at the same moment, `then`, each in the same
/// place (its type, its context and translator, its moment), in as many
/// contexts, so that the old run's numbers are this one's; and for each,
/// whose to splice (D243): where this run's has the contents the old run's
/// had then (by N8's digests, layout's grob with it), the old run's as it
/// ended (`end`), with what it did to it after; where not, this run's, if
/// the old run did nothing to it after but end a spanner or add to a
/// staff's items, which are carried over.
///
/// `then`'s records are read with the objects the resume gave back holding
/// what they held then (`Heap::with_held_at`, OPEN C18): as the resume left
/// them they hold what they held at the snapshot resumed from, and a change
/// the old run made between the two that this run did not make would
/// compare as the same. And as the splice gives them what they held as the
/// old run ended, one older than that snapshot, which this run's records
/// and state hold too, must hold what it held then: else this run's records
/// would end with the old run's changes, the same records or not.
fn lines_up(heap: &mut Heap, new: &Arena, old: &Old<'_>) -> Result<Choice, &'static str> {
    let (then, end) = (old.then, old.end);
    let (n, m) = (&new.records, &then.records);
    let place = |g: &crate::records::GrobRecord| (g.name, g.origin, g.moment);
    let sound = |r: &crate::records::AudioRecord| (r.name, r.origin, r.moment);
    if n.grobs.len() != m.grobs.len() {
        return Err("grobs count");
    }
    if n.audio.len() != m.audio.len() {
        return Err("audio count");
    }
    if new.context_count() != then.context_count() {
        return Err("contexts");
    }
    if new.grob_slot.is_some() != then.grob_slot.is_some() {
        return Err("slot some");
    }
    if !n
        .grobs
        .iter()
        .zip(m.grobs.iter())
        .all(|(a, b)| place(a) == place(b))
    {
        return Err("grob places");
    }
    if !n
        .audio
        .iter()
        .zip(m.audio.iter())
        .all(|(a, b)| sound(a) == sound(b))
    {
        return Err("audio places");
    }
    let e = &end.records;
    // A spanner open then may have been ended after: that is carried over.
    let unchanged = |a: &crate::records::GrobRecord, b: &crate::records::GrobRecord| {
        a.props == b.props
            && (a.ended == b.ended || b.ended.is_none())
            && a.interfaces == b.interfaces
            && a.definition == b.definition
            && a.cause == b.cause
    };
    // A staff's items may have been added to after: carried over too.
    let unchanged_audio = |a: &crate::records::AudioRecord, b: &crate::records::AudioRecord| {
        use crate::audio::Audio;
        a.value == b.value
            && a.column == b.column
            && a.channel == b.channel
            && a.cause == b.cause
            && match (&a.audio, &b.audio) {
                (Audio::Staff(x), Audio::Staff(y)) => {
                    x.items.starts_with(&y.items)
                        && (x.percussion, x.merge_unisons, x.control)
                            == (y.percussion, y.merge_unisons, y.control)
                        && x.track_name == y.track_name
                }
                (x, y) => format!("{x:?}") == format!("{y:?}"),
            }
    };
    let columns = |a: &Arena| a.performance.as_ref().map(|p| p.columns.clone());
    if e.grobs.len() < m.grobs.len() || e.audio.len() < m.audio.len() {
        return Err("end shorter");
    }
    if end.grob_slot.is_some() != then.grob_slot.is_some() {
        return Err("end's slot");
    }
    if columns(new) != columns(then) {
        return Err("columns");
    }
    if new.playback.is_some() != end.playback.is_some()
        || new.recorder.is_some() != end.recorder.is_some()
    {
        return Err("playback or recorder");
    }
    let (Some(rn), Some(rt)) = (new.n8.as_ref(), then.n8.as_ref()) else {
        return Err("recordings");
    };
    // Each record: the same contents as the old run's then, the old run's as
    // it ended; else the old run's untouched after then, this run's.
    let (mut an, mut at) = (
        crate::checkpoint::Alists::new(),
        crate::checkpoint::Alists::new(),
    );
    let mut choice = Choice {
        grobs: Vec::with_capacity(n.grobs.len()),
        audio: Vec::with_capacity(n.audio.len()),
    };
    // Records the same field by field hold the same objects, what a resume
    // took over from its snapshot, older than it: those of them the old run
    // changed hold what they held then (or this run does not splice, below),
    // and what this run alone changed the splice keeps. They are the same.
    let same_fields = |a: &crate::records::GrobRecord, b: &crate::records::GrobRecord| {
        a.props == b.props
            && a.ended == b.ended
            && a.interfaces == b.interfaces
            && a.definition == b.definition
            && a.cause == b.cause
    };
    let slot_digest = |heap: &Heap,
                       a: &Arena,
                       r: &crate::checkpoint::Recording,
                       i: usize,
                       al: &mut crate::checkpoint::Alists| {
        a.grob_slot
            .as_ref()
            .map(|s| s.grob_digest(i, &mut |v| r.value(heap, a, v, al)))
    };
    let both_slots = new.grob_slot.is_some() && then.grob_slot.is_some();
    // This run's digests, of the records not the same field by field (and
    // with layout's grob the same) as the old run's then.
    let mut digests: Vec<Option<(u64, Option<u64>)>> = Vec::with_capacity(n.grobs.len());
    for i in 0..n.grobs.len() {
        let slots_same = match (&new.grob_slot, &then.grob_slot) {
            (Some(a), Some(b)) => a.same_grob(b.as_ref(), i),
            _ => true,
        };
        digests.push(
            (!(same_fields(&n.grobs[i], &m.grobs[i]) && slots_same)).then(|| {
                let slot = both_slots
                    .then(|| slot_digest(heap, new, rn, i, &mut an))
                    .flatten();
                (rn.grob_record(heap, new, &n.grobs[i], &mut an), slot)
            }),
        );
    }
    let audio: Vec<u64> = n
        .audio
        .iter()
        .map(|r| rn.audio_record(heap, new, r, &mut an))
        .collect();
    // The old run's, as the heap was then; and of each object older than the
    // snapshot resumed from that held something else then, its digest then.
    let (then_digests, then_audio, older) =
        heap.with_held_at(old.at, old.redo.clone(), |heap, changed| {
            let grobs: Vec<Option<(u64, Option<u64>)>> = (0..m.grobs.len())
                .map(|i| {
                    digests[i].map(|_| {
                        let slot = both_slots
                            .then(|| slot_digest(heap, then, rt, i, &mut at))
                            .flatten();
                        (rt.grob_record(heap, then, &m.grobs[i], &mut at), slot)
                    })
                })
                .collect();
            let audio: Vec<u64> = m
                .audio
                .iter()
                .map(|r| rt.audio_record(heap, then, r, &mut at))
                .collect();
            let older: Vec<(Value, u64)> = changed
                .iter()
                .filter(|&&v| heap.epoch_of(v).is_some_and(|e| e < old.since))
                .map(|&v| (v, rt.value(heap, then, v, &mut at)))
                .collect();
            (grobs, audio, older)
        });
    if older
        .iter()
        .any(|&(v, then)| rn.value(heap, new, v, &mut an) != then)
    {
        return Err("an older object the old run changed holds otherwise");
    }
    for i in 0..n.grobs.len() {
        let same = match (digests[i], then_digests[i]) {
            (Some((dn, sn)), Some((dt, st))) => dn == dt && sn == st,
            _ => true,
        };
        let untouched = unchanged(&e.grobs[i], &m.grobs[i])
            && match (&end.grob_slot, &then.grob_slot) {
                (Some(a), Some(b)) => a.same_grob(b.as_ref(), i),
                _ => true,
            };
        match (same, untouched) {
            (true, _) => choice.grobs.push(true),
            (false, true) => choice.grobs.push(false),
            (false, false) => return Err("a grob changed after, and before by this run"),
        }
    }
    for i in 0..n.audio.len() {
        let same = audio[i] == then_audio[i];
        match (same, unchanged_audio(&e.audio[i], &m.audio[i])) {
            (true, _) => choice.audio.push(true),
            (false, true) => choice.audio.push(false),
            (false, false) => return Err("an audio item changed after, and before by this run"),
        }
    }
    Ok(choice)
}

/// Which of a splice's records before its checkpoint are the old run's as
/// it ended (`true`) and which this run's (D243), by number.
struct Choice {
    grobs: Vec<bool>,
    audio: Vec<bool>,
}

/// N8's splice (D243): where the resumed translation `t`'s checkpoint just
/// taken converges with the last compile's run of it (`Recording::converged`)
/// and the two runs line up there ([`lines_up`]), and the old run changed
/// nothing it made since the snapshot resumed from after that checkpoint,
/// the translation ends as the old run did: its arena the old run's as it
/// ended, with this run's records, layout's grobs, trace, playback map and
/// N8's timesteps to the checkpoint; the objects the resume gave back what
/// they held as the old run ended; and what the old run said from the
/// checkpoint said again. Whether it spliced: the global loop stops.
pub(crate) fn splice(host: &mut dyn Host, t: &Translation) -> bool {
    let found = (|| {
        let a = t.arena.borrow();
        let (sp, r) = (a.splice.as_deref()?, a.n8.as_ref()?);
        let checkpoint = sp.end.end_pure.then(|| r.converged(&sp.kept))??;
        let moment = sp.kept.checkpoints[checkpoint].moment;
        // The old run's snapshot there: its records and the rest as they
        // were then. A thinned one is not there; a later checkpoint may be.
        let at = sp.later.iter().position(|s| s.moment == moment)?;
        let then = &sp.later[at];
        let old = Old {
            then: &then.arena,
            end: &sp.end,
            at: (then.generation, then.mark),
            redo: sp.redo.clone(),
            since: sp.since,
        };
        let choice = lines_up(host.heap(), &a, &old).ok()?;
        let (recorder, playback) = spliced_outputs(&a, old.then, old.end)?;
        Some((checkpoint, at, recorder, playback, choice))
    })();
    found.is_some_and(|found| splice_at(host, t, found))
}

/// Whether the checkpoint just taken is one [`splice`] can take: the old run
/// has a snapshot there, and the two runs' digests agree (`steps` takes this
/// run's before it splices).
pub(crate) fn converges(t: &Translation, moment: Moment) -> bool {
    let a = t.arena.borrow();
    match (a.splice.as_deref(), a.n8.as_ref()) {
        (Some(sp), Some(r)) => sp.end.end_pure && sp.has(moment) && r.converged(&sp.kept).is_some(),
        _ => false,
    }
}

/// N8's splice where the resumed translation `t`'s music is the old run's
/// (D258): at once, at the snapshot it resumed from, every record the old
/// run's as it ended: the same music from the same state makes them again.
/// Whether it spliced; if not, it runs on from there as it would have.
pub(crate) fn splice_same(host: &mut dyn Host, t: &Translation) -> bool {
    let found = (|| {
        let a = t.arena.borrow();
        let sp = a.splice.as_deref()?;
        let then = sp.later.first().filter(|_| sp.end.end_pure)?;
        let checkpoint = sp.kept.checkpoint_at(then.step)?;
        let (recorder, playback) = spliced_outputs(&a, &then.arena, &sp.end)?;
        let choice = Choice {
            grobs: alloc::vec![true; a.records.grobs.len()],
            audio: alloc::vec![true; a.records.audio.len()],
        };
        Some((checkpoint, 0, recorder, playback, choice))
    })();
    found.is_some_and(|found| splice_at(host, t, found))
}

/// The trace and the playback map a splice ends with, each `None` if the
/// translation has none; `None` where they cannot be spliced.
#[allow(clippy::type_complexity)]
fn spliced_outputs(
    a: &Arena,
    then: &Arena,
    end: &Arena,
) -> Option<(
    Option<(crate::trace::Recorder, crate::trace::Heard, String)>,
    Option<crate::playback::Playback>,
)> {
    let recorder = match (&a.recorder, &then.recorder, &end.recorder) {
        (Some(new), Some(was), Some(old)) => Some(crate::trace::splice(new, was, old)?),
        (None, None, None) => None,
        _ => return None,
    };
    let playback = match (&a.playback, &then.playback, &end.playback) {
        (Some(new), Some(was), Some(old)) => {
            Some(crate::playback::Playback::splice(new, was, old)?)
        }
        (None, None, None) => None,
        _ => return None,
    };
    Some((recorder, playback))
}

/// What a splice found: the old run's checkpoint by number, its snapshot by
/// its place in `later`, the trace and playback map to end with, whose each
/// record is.
type Found = (
    usize,
    usize,
    Option<(crate::trace::Recorder, crate::trace::Heard, String)>,
    Option<crate::playback::Playback>,
    Choice,
);

/// The translation `t` ends as the old run did, from what [`splice`] found.
fn splice_at(host: &mut dyn Host, t: &Translation, found: Found) -> bool {
    let (checkpoint, at, recorder, playback, choice) = found;
    let Some(sp) = t.arena.borrow_mut().splice.take() else {
        return false;
    };
    let Splice {
        end,
        later,
        kept,
        redo,
        said,
        ..
    } = *sp;
    let then = &later[at];
    let heard = recorder
        .as_ref()
        .map(|(_, grobs, after)| (grobs.clone(), after.clone()));
    let odef;
    let mut out = end.snapshot();
    let placed_at_end = end.placed_at_end.clone();
    {
        let mut a = t.arena.borrow_mut();
        // This run's records to the checkpoint where the choice is this
        // run's, the numbers of their interfaces the old run's; the old
        // run's as it ended where it is the old run's.
        for (i, g) in a.records.grobs.iter().enumerate() {
            if choice.grobs[i] {
                // A handle only this run made for it.
                if out.records.grobs[i].handle == Value::FALSE {
                    out.records.grobs[i].handle = g.handle;
                }
                continue;
            }
            let mut g = g.clone();
            let mut set = crate::records::InterfaceSet::default();
            for k in 0..=255u8 {
                if g.interfaces.contains(k) {
                    let sym = a.records.interfaces.symbol(k);
                    if let Some(k) = out.records.interfaces.number(sym) {
                        set.insert(k);
                    }
                }
            }
            g.interfaces = set;
            // Ended after the checkpoint by the old run.
            if g.ended.is_none() {
                g.ended = out.records.grobs[i].ended;
            }
            out.records.grobs[i] = g;
            if let (Some(slot), Some(new)) = (out.grob_slot.as_mut(), a.grob_slot.as_ref()) {
                slot.put_grob(new.as_ref(), i);
            }
        }
        for (i, r) in a.records.audio.iter().enumerate() {
            use crate::audio::Audio;
            if choice.audio[i] {
                continue;
            }
            let mut r = r.clone();
            // A staff's items the old run added after the checkpoint.
            if let (Audio::Staff(s), Audio::Staff(old), Some(Audio::Staff(was))) = (
                &mut r.audio,
                &out.records.audio[i].audio,
                then.arena.records.audio.get(i).map(|w| &w.audio),
            ) {
                s.items
                    .extend_from_slice(old.items.get(was.items.len()..).unwrap_or_default());
                s.end = old.end;
            }
            out.records.audio[i] = r;
        }
        out.recorder = recorder.map(|(r, _, _)| r);
        out.playback = playback;
        let mut recording = a.n8.take();
        if let Some(r) = recording.as_mut() {
            let own = (0..choice.grobs.len() as u32)
                .filter(|&i| !choice.grobs[i as usize])
                .collect();
            r.splice_tail(kept, checkpoint, own);
            if let Some(old) = out.n8.as_ref() {
                r.parts.adopt(&old.parts);
            }
        }
        out.n8 = recording;
        out.snapshots = core::mem::take(&mut a.snapshots);
        out.resumed_from = a.resumed_from.take();
        out.resume_base = a.resume_base;
        out.errors_base = a.errors_base;
        out.diagnostics_base = a.diagnostics_base;
        odef = a.output_def();
        *a = out;
    }
    host.heap().redo(redo);
    // This compile's output definition, as a resume gives it, after the end
    // values: the Paper_score's would bring the last compile's back.
    t.arena.borrow_mut().take_output_def(host.heap(), odef);
    // What the old run said from the checkpoint, said again.
    let from = host.error_len();
    host.write_errors(said.get(then.errors..).unwrap_or_default());
    // The placed messages after those it had said by then, by count: their
    // places in the error port need not have moved on.
    let placed: Vec<([Value; 3], usize)> = placed_at_end
        .get(then.diagnostics.len()..)
        .unwrap_or_default()
        .iter()
        .map(|&(d, mark)| (d, mark.saturating_sub(then.errors)))
        .collect();
    say_again(host, &placed, from);
    if let Some((grobs, after)) = heard {
        crate::trace::spliced_grobs(host, t, grobs, &after);
    }
    true
}

/// The links to places the arena `a` holds moved to the text after an edit
/// (D244).
fn move_links(a: &mut Arena, moved: &crate::Moved) {
    if let Some(r) = a.recorder.as_mut() {
        r.move_links(moved);
    }
    if let Some(p) = a.playback.as_mut() {
        p.move_links(moved);
    }
}

/// Whether the arena `a` holds links to places, which an edit moves: a trace's
/// or a playback map's, which an untraced `\\layout` translation has neither of.
fn has_links(a: &Arena) -> bool {
    a.recorder.is_some() || a.playback.is_some()
}

impl Snapshot {
    /// A copy of it with its links moved (D244): the snapshot itself stays
    /// the old text's, for what a stop puts back (D261).
    fn moved(&self, moved: &crate::Moved) -> Snapshot {
        let mut arena = self.arena.snapshot();
        move_links(&mut arena, moved);
        Snapshot {
            step: self.step,
            moment: self.moment,
            ok: self.ok,
            arena: Box::new(arena),
            its: self.its.clone(),
            mark: self.mark,
            errors: self.errors,
            generation: self.generation,
            epoch: self.epoch,
            log: self.log.clone(),
            pure: Cell::new(self.pure.get()),
            diagnostics: self.diagnostics.clone(),
        }
    }
}

/// What a resume found to go on from.
pub(crate) struct Resumed {
    pub(crate) t: Translation,
    pub(crate) it: Iteration,
    pub(crate) at: (Moment, bool),
    pub(crate) snapshot: usize,
    /// The timestep of the snapshot it resumed from.
    pub(crate) step: usize,
    /// Whether its music is the old run's.
    pub(crate) same: bool,
    /// Its translation's epochs before this compile's.
    pub(crate) epochs: Vec<(u32, u32)>,
}

/// What finds a new translation's first state, `resume`'s probe, from a
/// recording of the new music.
pub(crate) type Probe<'a> = &'a mut dyn FnMut(
    &mut dyn Host,
    crate::checkpoint::Recording,
)
    -> Result<Option<u64>, lily_engine_scheme::Exception>;

/// The translation `old` resumed on `music` (D241), if it can be: the
/// trace begun the same, the shapes lined up, a snapshot before `m_e`, the
/// first state the old one's (`probe`, a translation of the new music's
/// first timestep, run only then), and the last pure snapshot before `m_e`
/// whose iterators the new music takes (the iterators put on it). Then the
/// heap gives the old translation's objects what they held there, the
/// arena is the snapshot's and N8's recording takes the new shape.
pub(crate) fn resume(
    host: &mut dyn Host,
    old: &Resumable,
    (music, odef): (lily_engine_scheme::Value, lily_engine_scheme::Value),
    probe: Probe<'_>,
    trace: Option<&crate::Trace>,
    moved: Option<&crate::Moved>,
) -> Result<Option<Resumed>, lily_engine_scheme::Exception> {
    // After an edit that moved bytes (D244): the old parse's places are
    // the new text's (the engine moved them), and so are the links the old
    // run wrote and what it said, which a resume writes again; a message
    // on a line the edit changed is not, nor one whose line's number grew
    // a digit.
    let said = match moved {
        Some(m) => match m.said(&old.said) {
            Some(said) if said.len() == old.said.len() => said,
            _ => return Ok(None),
        },
        None => old.said.clone(),
    };
    let links = |s: &String| moved.map_or_else(|| s.clone(), |m| m.links(s));
    let same_trace = match (&old.trace, trace) {
        (None, None) => true,
        (Some((text, grobs)), Some(t)) => links(text) == t.text && links(grobs) == t.grobs,
        _ => false,
    };
    if !same_trace {
        return Ok(None);
    }
    let new = crate::checkpoint::Recording::new(host.heap(), music);
    let Some((m_e, _)) = new.against(&old.kept) else {
        return Ok(None);
    };
    let t = &old.t;
    // The last pure snapshot before `m_e` whose iterators the new music
    // takes, the latest first.
    let candidates: Vec<usize> = {
        let a = t.arena.borrow();
        (0..a.snapshots.len())
            .rev()
            .filter(|&k| {
                let s = &a.snapshots[k];
                s.pure.get() && m_e.is_none_or(|m| s.moment.compare(m).is_lt())
            })
            .collect()
    };
    // The new translation's first state, probed only where a snapshot
    // could be resumed from (MS6 step 7: T29's E2 changes it before its
    // first), must be the old one's.
    if candidates.is_empty() || probe(host, new.clone())? != Some(old.kept.start) {
        return Ok(None);
    }
    let keys = new.keys();
    let mut it = Iteration::new(host.heap(), t);
    let mut found = None;
    for k in candidates {
        let s = t.arena.borrow().snapshots[k].clone();
        let Some(recorded) = s.arena.n8.as_ref() else {
            continue;
        };
        // The snapshot's iterators are on the music of the run that took
        // it, which a resumed run's earlier snapshots are not: they are
        // lined up with the new music by that run's shape, not the last
        // compile's (MS6 step 7: T24's E1 after its text after E1 kept the
        // first run's note).
        let lined_up = new.against(recorded.kept());
        let rebuilt = match &lined_up {
            Some((_, shift)) => it.rebuild(host, &s.its, &|key| {
                recorded.new_music((&new, &keys), shift, key)
            }),
            None => Ok(false),
        };
        if rebuilt? && let Some((_, shift)) = lined_up {
            let said = (s.generation, s.mark, s.errors, s.diagnostics.clone());
            found = Some((k, (s.moment, s.ok), said, shift));
            break;
        }
    }
    let Some((k, at, (generation, mark, written, placed), shift)) = found else {
        return Ok(None);
    };
    let mut last = it.length(old.top);
    if last.main.is_inf() {
        last = Moment::default();
    }
    it.top = Some((old.top, last));
    let owns = |e: u32| old.epochs.iter().any(|&(a, b)| (a..=b).contains(&e));
    let end_mark = t.arena.borrow().end_mark;
    let redo = host.heap().give_back(generation, mark, &owns, end_mark);
    // The snapshots after the one resumed from and the arena as the old run
    // ended are kept for a splice (D243), and the one resumed from where the
    // music is the same, which splices there (D258). A new one is taken in
    // its place as the run begins, so that the snapshots near an edit are
    // never as old as [`GENERATIONS`].
    let (mut arena, later, end, step, since) = {
        let a = t.arena.borrow();
        // The old arena stays the old text's, for what a stop puts back: what
        // this run takes of it is moved in copies.
        let mut before: Vec<Rc<Snapshot>> = a
            .snapshots
            .iter()
            .map(|s| match moved {
                Some(m) if has_links(&s.arena) => Rc::new(s.moved(m)),
                _ => s.clone(),
            })
            .collect();
        let mut later = before.split_off(k);
        let s = later[0].clone();
        let (step, since) = (s.step, s.epoch);
        let mut arena = s.arena.snapshot();
        if m_e.is_some() {
            later.remove(0);
        }
        // This run's log: the old one's as far as the snapshot, or one from
        // here (MS6 step 7b).
        arena.log = old.replayable().then(|| {
            let log = match &arena.log {
                Some(l) => {
                    let mut log = l.borrow().clone();
                    log.truncate(&s.log);
                    log
                }
                None => crate::replay::Log::from_step(step as u32),
            };
            Rc::new(core::cell::RefCell::new(log))
        });
        arena.snapshots = before;
        let end = match (&a.at_end, moved) {
            (Some(end), Some(m)) if has_links(end) => {
                let mut copy = end.snapshot();
                copy.placed_at_end = end.placed_at_end.clone();
                move_links(&mut copy, m);
                Some(Rc::new(copy))
            }
            (end, _) => end.clone(),
        };
        (arena, later, end, step, since)
    };
    if let Some(r) = arena.n8.as_mut() {
        r.take_shape(new, &shift, at.0);
        r.place = match m_e {
            Some(edit) => {
                let before = old
                    .kept
                    .checkpoints
                    .iter()
                    .rev()
                    .map(|c| c.moment)
                    .find(|m| m.compare(edit).is_lt())
                    .filter(|m| m.compare(at.0).is_gt());
                Place::Edit {
                    before,
                    edit,
                    past: 0,
                }
            }
            None => Place::Nowhere,
        };
    }
    arena.take_output_def(host.heap(), odef);
    // What it said before the snapshot, said again where it begins now.
    arena.errors_base = host.error_len();
    arena.diagnostics_base = diagnostics_now(host);
    host.write_errors(said.get(..written).unwrap_or(&said));
    say_again(host, &placed, arena.errors_base);
    arena.resume_base = crate::context::ResumeBase {
        log: host.heap().undo_len(),
        since: host.heap().epoch(),
        globals: host.globals_written(),
    };
    arena.splice = end.map(|end| {
        let log = end.log.clone();
        Box::new(Splice {
            end,
            later,
            kept: old.kept.clone(),
            redo,
            since,
            said,
            log,
        })
    });
    // The old arena whole, which a stop puts back (B24, D261).
    let from = core::mem::replace(&mut *t.arena.borrow_mut(), arena);
    t.arena.borrow_mut().resumed_from = Some(Box::new(from));
    Ok(Some(Resumed {
        t: t.clone(),
        it,
        at,
        snapshot: k,
        step,
        same: m_e.is_none(),
        epochs: old.epochs.clone(),
    }))
}
