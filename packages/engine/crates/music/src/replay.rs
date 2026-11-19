//! N8's replay (04 §10.4; MS6 steps 7b and 7c): what each translator call
//! of a recorded translation did, kept so that a later compile can do it
//! again in place of the call where the call's partition is replayed.
//!
//! - **The log** ([`Log`]): every call of a translator of a partition's
//!   context (`call_slot`) is named by its timestep, its context's key and
//!   how many calls of that context's translators came before it in the
//!   timestep; what it did is a run of [`Effect`]s: a grob or audio record
//!   made, announced or ended, a grob or an audio record written (`Op`,
//!   layout's writes and the record's own properties), a context property
//!   set or unset or an alist of one changed in place, a moment asked for,
//!   the playback map's staff. A call that did anything else (said
//!   something, broadcast an event, overrode a property, gave out a shared
//!   channel, wrote a grob array of no known grob) is not replayable. Each
//!   timestep's calls are counted by context.
//! - The log is shared by a run's copies of its arena (snapshots, its end)
//!   and cut back where a snapshot is restored; a resumed run starts its
//!   own from the old one's as far as the snapshot it resumed from, the
//!   timesteps before it shared.
//! - **Replay** ([`Replay`], [`instead`], [`check`], [`converged`]): at
//!   each timestep's end each replayed part must have heard and read what
//!   the old run's did, and as many calls been made of each of its contexts
//!   as there. A part that fails runs: the run goes back to the last
//!   [`Point`] at which its translators hold what they ran to (the first
//!   where the replay began; a part's translators are not kept while it
//!   is replayed), with it running past where it failed, and it goes back
//!   to replay at a checkpoint where its state and future are the old
//!   run's, from the next timestep. The next compile starts the parts that
//!   failed running.
//! - It pays only for many partitions ([`PARTITIONS`]): a replayed part's
//!   iterators, its events and the shared level's acknowledgements of its
//!   grobs all still run.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use lily_engine_scheme::{Tracer, Value};

use crate::Moment;
use crate::audio::Audio;
use crate::context::CtxId;
use crate::records::{AudioRecord, Cause, GrobRecord};

/// Whether translations are logged and their partitions replayed: off since
/// D258, as D247 said it would be if B6's translation phase needed it. A
/// replay saves nothing measurable, and a replayed run neither splices nor
/// can be resumed from, so that it costs what a resume near the edit gives.
/// The harness keeps it right (`--replay-all`).
pub static ON: AtomicBool = AtomicBool::new(false);

/// How many partitions a translation needs for a replay of it to pay:
/// with fewer, what the log and the check cost is more than what the other
/// partition's translators would (MS6 step 7c).
pub static PARTITIONS: AtomicU64 = AtomicU64::new(3);

/// Sets [`ON`].
pub fn set_replay(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// Sets [`PARTITIONS`] (the harness replays every translation it can).
pub fn set_replay_partitions(n: u64) {
    PARTITIONS.store(n, Ordering::Relaxed);
}

/// Never in wasm, where nothing sets [`ON`]: so the core.wasm a page runs
/// holds no replay, which D258 turned off (MS7 step 5, D262).
pub(crate) fn on() -> bool {
    !cfg!(target_arch = "wasm32") && ON.load(Ordering::Relaxed)
}

/// Whether a translation of `parts` parts, the shared level one of them, is
/// worth replaying.
pub(crate) fn pays(parts: usize) -> bool {
    parts as u64 > PARTITIONS.load(Ordering::Relaxed)
}

/// What a resumed translation replayed (MS6 step 7c): the attempts, and
/// over all of them the calls of its partitions' translators made and those
/// made again from the old run's log.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Replayed {
    pub attempts: u64,
    pub run: u64,
    pub replayed: u64,
}

static ATTEMPTS: AtomicU64 = AtomicU64::new(0);
static RUN: AtomicU64 = AtomicU64::new(0);
static REPLAYED: AtomicU64 = AtomicU64::new(0);

pub(crate) fn count_begin() {
    for c in [&ATTEMPTS, &RUN, &REPLAYED] {
        c.store(0, Ordering::Relaxed);
    }
}

pub(crate) fn attempted() {
    ATTEMPTS.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn counted() -> Replayed {
    Replayed {
        attempts: ATTEMPTS.load(Ordering::Relaxed),
        run: RUN.load(Ordering::Relaxed),
        replayed: REPLAYED.load(Ordering::Relaxed),
    }
}

/// A write to a grob, as layout makes it (`grob_written`'s callers), or to
/// its record's properties.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// `set_property` (`internal_set_property`), unchecked.
    Property(Value, Value),
    /// `set_object` (`internal_set_object`).
    Object(Value, Value),
    /// `set_parent` on an axis (0 X, 1 Y), `#f` for none.
    Parent(u8, Value),
    /// `add_grob` (ordered) or `add_unordered_grob`.
    Add(Value, Value, bool),
    /// `set_ordered`.
    Order(Value, bool),
    /// The grob array holding the grob alone, or nothing for `#f`.
    Reset(Value, Value),
    /// `ly:grob-set-property!` on the record.
    Record(Value, Value),
    /// A spanner's bound on a side (0 left, 1 right).
    Bound(u8, Value),
    /// `suicide`.
    Suicide,
    /// A write replay cannot make again: a grob array changed from Scheme.
    Unknown,
}

/// What a translator call did, in order.
#[derive(Clone)]
pub(crate) enum Effect {
    /// A grob record made, its number and the record as it was made.
    Grob(u32, Box<GrobRecord>),
    /// The grob `grob` announced in `ctx` (to its group and those above
    /// it, or to its group only), its start or its end.
    Announce {
        grob: u32,
        ctx: CtxId,
        end: bool,
        local: bool,
    },
    /// A grob written.
    Write(u32, Op),
    /// A context property set, or unset (`None`).
    Property(CtxId, Value, Option<Value>),
    /// [`crate::context::Arena::prepend_property`]: a key and its value
    /// put in front of a context's alist, whose old entry is taken out in
    /// place.
    Prepend(CtxId, Value, Value, Value),
    /// A moment asked for.
    Moment(Moment),
    /// The translator asked to stay awake for the next timestep.
    Keep,
    /// An audio record made and announced, its number and the record as it
    /// was made.
    Audio(u32, Box<AudioRecord>),
    /// An audio record changed: what it is now (a staff's items apart) and
    /// its channel.
    AudioSet(u32, Box<Audio>, u8),
    /// The item added to the audio staff.
    StaffItem(u32, u32),
    /// A `Staff_performer` initialized (true) or finalized.
    StaffPerformer(bool),
    /// The playback map's entry of a staff as it is now.
    Playback(CtxId, Box<crate::playback::Staff>),
}

/// A call's name: its timestep, its context's key and its rank among the
/// calls of that context's translators in the timestep.
pub(crate) type Call = (u32, u64, u32);

/// What a timestep's calls did: their effects, and for each call that did
/// anything or cannot be replayed, by its context's key and rank, where its
/// effects are and whether it can be. Runs share the timesteps they share.
#[derive(Clone, Default)]
pub(crate) struct Chunk {
    effects: Vec<Effect>,
    calls: BTreeMap<(u64, u32), (u32, u32, bool)>,
    /// The calls made of each context's translators, by its key.
    made: Vec<(u64, u32)>,
}

/// A run's log of its calls' effects, by timestep.
#[derive(Clone, Default)]
pub(crate) struct Log {
    steps: Vec<Rc<Chunk>>,
    /// The calls running, innermost last: each one's name, where its
    /// effects begin, the error port's length then and whether it is
    /// replayable so far.
    open: Vec<(Call, u32, usize, bool)>,
    /// The calls begun of each context's translators in the timestep being
    /// run, by context, with the timestep they were counted in, and each
    /// context's key.
    ranks: Vec<(u32, u32)>,
    keys: Vec<u64>,
    /// The timestep being run.
    pub(crate) step: u32,
}

/// Where a log was at a snapshot (`Log::mark`).
#[derive(Clone, Default)]
pub(crate) struct Mark {
    pub(crate) effects: usize,
    step: u32,
    ranks: Vec<(u32, u32)>,
}

/// A log shared by a run's arena and its copies.
pub(crate) type Shared = Rc<RefCell<Log>>;

impl Log {
    /// A log that begins at the timestep `step`, a run resumed there.
    pub(crate) fn from_step(step: u32) -> Log {
        Log {
            step,
            ..Log::default()
        }
    }

    /// Where the log is, for a snapshot: the effects of the timestep being
    /// run, the timestep and the calls counted in it by context.
    pub(crate) fn mark(&self) -> Mark {
        Mark {
            effects: self
                .steps
                .get(self.step as usize)
                .map_or(0, |c| c.effects.len()),
            step: self.step,
            ranks: self.ranks.clone(),
        }
    }

    /// A new timestep, `step`, begins.
    pub(crate) fn step(&mut self, step: u32) {
        self.count();
        self.step = step;
    }

    /// The timestep being run's calls counted by context, for the check.
    pub(crate) fn count(&mut self) {
        let step = self.step;
        let made: Vec<(u64, u32)> = self
            .ranks
            .iter()
            .zip(&self.keys)
            .filter(|(r, _)| r.0 == step)
            .map(|(r, &key)| (key, r.1))
            .collect();
        self.chunk().made = made;
    }

    /// The timestep being run's own chunk.
    fn chunk(&mut self) -> &mut Chunk {
        let step = self.step as usize;
        if self.steps.len() <= step {
            self.steps.resize_with(step + 1, Rc::default);
        }
        Rc::make_mut(&mut self.steps[step])
    }

    /// The name of the next call of a translator of the context `c`, of key
    /// `key`, counted.
    pub(crate) fn next(&mut self, c: CtxId, key: u64) -> Call {
        let c = c as usize;
        if self.ranks.len() <= c {
            self.ranks.resize(c + 1, (u32::MAX, 0));
            self.keys.resize(c + 1, 0);
        }
        self.keys[c] = key;
        let r = &mut self.ranks[c];
        if r.0 != self.step {
            *r = (self.step, 0);
        }
        r.1 += 1;
        (self.step, key, r.1 - 1)
    }

    /// The call `call` begins; the error port is `errors` long.
    pub(crate) fn begin(&mut self, call: Call, errors: usize) {
        let at = self.chunk().effects.len() as u32;
        self.open.push((call, at, errors, true));
    }

    /// The innermost call ends; the error port is `errors` long. Kept if it
    /// did anything.
    pub(crate) fn end(&mut self, heap: &mut lily_engine_scheme::Heap, errors: usize) {
        if let Some(((_, key, rank), from, said, ok)) = self.open.pop() {
            let ok = ok && errors == said;
            let chunk = self.chunk();
            let to = chunk.effects.len() as u32;
            // What it set as the call left it: a translator changes a value
            // in place (`localAlterations`, a stem's `beaming`), and what
            // holds it goes on changing it after.
            for e in &mut chunk.effects[from as usize..] {
                match e {
                    Effect::Property(_, _, Some(v))
                    | Effect::Write(_, Op::Property(_, v) | Op::Object(_, v) | Op::Record(_, v)) => {
                        *v = kept_value(heap, *v);
                    }
                    _ => {}
                }
            }
            if to > from || !ok {
                chunk.calls.insert((key, rank), (from, to, ok));
            }
        }
    }

    /// Whether a call is running.
    pub(crate) fn in_call(&self) -> bool {
        !self.open.is_empty()
    }

    /// The running call did `e`.
    pub(crate) fn push(&mut self, e: Effect) {
        if self.in_call() {
            self.chunk().effects.push(e);
        }
    }

    /// The running call did what replay cannot do again; and every call
    /// around it, which caused it.
    pub(crate) fn not_replayable(&mut self) {
        for c in &mut self.open {
            c.3 = false;
        }
    }

    /// Cut back to `mark`, a snapshot's.
    pub(crate) fn truncate(&mut self, mark: &Mark) {
        let step = mark.step as usize;
        self.steps.truncate(step + 1);
        self.open.clear();
        self.step = mark.step;
        self.ranks = mark.ranks.clone();
        if self.steps.len() > step && self.steps[step].effects.len() > mark.effects {
            let chunk = Rc::make_mut(&mut self.steps[step]);
            chunk.effects.truncate(mark.effects);
            let cut = mark.effects as u32;
            chunk.calls.retain(|_, c| c.0 < cut);
        }
    }

    /// The old call `call`, if it had effects or cannot be replayed:
    /// whether it can be, its timestep's chunk and where its effects are.
    fn call(&self, (step, key, rank): Call) -> Option<(bool, Rc<Chunk>, u32, u32)> {
        let chunk = self.steps.get(step as usize)?;
        let &(from, to, ok) = chunk.calls.get(&(key, rank))?;
        Some((ok, Rc::clone(chunk), from, to))
    }

    /// The calls made of each context's translators in the timestep `step`.
    fn made(&self, step: u32) -> &[(u64, u32)] {
        self.steps.get(step as usize).map_or(&[], |c| &c.made)
    }

    /// `f` of each call with effects of the timestep `step`: its context's
    /// key and where its effects begin.
    fn each_call(&self, step: u32, mut f: impl FnMut(u64, u32)) {
        if let Some(c) = self.steps.get(step as usize) {
            for (&(key, _), &(from, _, _)) in &c.calls {
                f(key, from);
            }
        }
    }

    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        for e in self.steps.iter().flat_map(|c| &c.effects) {
            match *e {
                Effect::Grob(_, ref g) => {
                    tracer.trace(g.name);
                    tracer.trace(g.props);
                    tracer.trace(g.definition);
                    if let Cause::Event(ev) = g.cause {
                        tracer.trace(ev);
                    }
                }
                Effect::Audio(_, ref r) => {
                    tracer.trace(r.name);
                    tracer.trace(r.value);
                    if let Cause::Event(ev) = r.cause {
                        tracer.trace(ev);
                    }
                }
                Effect::Write(_, op) => match op {
                    Op::Property(a, b) | Op::Object(a, b) | Op::Record(a, b) => {
                        tracer.trace(a);
                        tracer.trace(b);
                    }
                    Op::Parent(_, v) | Op::Order(v, _) | Op::Bound(_, v) => tracer.trace(v),
                    Op::Add(a, b, _) | Op::Reset(a, b) => {
                        tracer.trace(a);
                        tracer.trace(b);
                    }
                    Op::Unknown | Op::Suicide => {}
                },
                Effect::Property(_, sym, v) => {
                    tracer.trace(sym);
                    if let Some(v) = v {
                        tracer.trace(v);
                    }
                }
                Effect::Prepend(_, sym, key, val) => {
                    tracer.trace(sym);
                    tracer.trace(key);
                    tracer.trace(val);
                }
                Effect::Announce { .. }
                | Effect::Moment(_)
                | Effect::Keep
                | Effect::AudioSet(..)
                | Effect::StaffItem(..)
                | Effect::StaffPerformer(_)
                | Effect::Playback(..) => {}
            }
        }
    }
}

/// A call of a translator of the context `c` of the translation `t`
/// begins: named and opened in its log if `c` is a partition's and the
/// translation is logged; the log, to end it.
pub(crate) fn begin(
    host: &mut dyn crate::context::Host,
    t: &crate::context::Translation,
    c: CtxId,
) -> Option<Shared> {
    if !on() {
        return None;
    }
    let errors = host.error_len();
    let heap = host.heap();
    let mut a = t.arena.borrow_mut();
    let log = a.log.clone()?;
    let mut r = a.n8.take()?;
    if !r.parts.knows(c) {
        r.parts.see(heap, &a);
    }
    let shared = r.parts.is_shared(c);
    let key = r.parts.key(c);
    a.n8 = Some(r);
    if shared {
        return None;
    }
    let call = log.borrow_mut().next(c, key);
    log.borrow_mut().begin(call, errors);
    RUN.fetch_add(1, Ordering::Relaxed);
    Some(log)
}

// --- replay ----------------------------------------------------------------

/// A replay running, in its translation's arena: the old run's log, its
/// timesteps, its parts and their rows, which parts are replayed, and the
/// old run's grobs and contexts here.
#[derive(Clone)]
pub(crate) struct Replay {
    log: Shared,
    moments: Rc<Vec<Moment>>,
    rows: Rc<crate::partition::Parts>,
    part_of: Rc<BTreeMap<u64, u32>>,
    above: Rc<Vec<Vec<u64>>>,
    /// The parts replayed now, by the old run's numbers (a part that runs
    /// is replayed again from a checkpoint where it holds what the old run's
    /// did); the old timestep from which each one's calls are counted.
    replayed: Vec<bool>,
    since: Vec<u32>,
    /// The old run's checkpoints' moments.
    checkpoints: Rc<Vec<Moment>>,
    old_keys: Rc<Vec<u64>>,
    old_places: Rc<Vec<u64>>,
    /// Each old grob's type, which this run's at its place must have, and
    /// the old run's numbers of the interfaces: each run numbers them as it
    /// meets them.
    old_names: Rc<Vec<Value>>,
    old_interfaces: Rc<crate::records::Interfaces>,
    /// The old run's grobs' numbers here.
    grobs: BTreeMap<u32, u32>,
    /// This run's grobs by place, as far as `scanned`.
    places: BTreeMap<u64, u32>,
    scanned: usize,
    /// The old run's audio records' numbers here, and how many records it
    /// began with, which have theirs.
    audio: BTreeMap<u32, u32>,
    audio_base: u32,
    /// How many grob records it began with, which have theirs.
    grob_base: u32,
    /// Each of this run's contexts' part ([`Replay::part`]), as far as
    /// they have been asked for.
    by_context: Vec<u32>,
    /// The moment last looked up and its old timestep.
    at: Option<(Moment, Option<u32>)>,
    /// `%host-call` and the name of a write, once looked up.
    writer: Option<(Value, Value)>,
    /// The old calls applied since the last check, by part.
    applied: Vec<u32>,
    /// The old timestep the last check matched; the old timestep the
    /// replay began in, and its log's length there (the calls before it
    /// were made before the snapshot it resumed from).
    matched: Option<u32>,
    start: (u32, u32),
    /// The parts found not replayable.
    pub(crate) failed: Vec<(u32, Moment)>,
    /// The moment each part that failed before runs till, before it may be
    /// replayed again; how often each has failed.
    held: Vec<Option<Moment>>,
    fails: Vec<u8>,
    last_failed: Vec<(u32, Moment)>,
    /// Where a part that fails can run again from: the snapshots taken
    /// where the replay began and where a part went back to replay, each
    /// with the old timestep it was taken at and the replay as it stood.
    points: Vec<Point>,
}

/// A place to go back to: a snapshot of this run, the old timestep it was
/// taken at, and the replay there.
#[derive(Clone)]
pub(crate) struct Point {
    snapshot: usize,
    step: u32,
    replay: Box<Replay>,
}

/// [`Replay::part`]: a context the old run had not, and one not asked for.
const NO_PART: u32 = u32::MAX;
const UNKNOWN: u32 = u32::MAX - 1;

/// The points a replay keeps past its first: a point costs a snapshot.
const POINTS: usize = 2;

/// The failures after which a part runs to the end.
const FAILS: u8 = 4;

impl Replay {
    /// A replay of the old run `old` (its arena as it ended), whose log is
    /// `log`, timesteps `kept`'s, every partition replayed but `running`'s;
    /// `None` where its partitions are one, or none would be replayed.
    pub(crate) fn new(
        old: &crate::context::Arena,
        kept: &crate::checkpoint::Kept,
        log: Shared,
        running: &[(u32, Moment)],
    ) -> Option<Replay> {
        let r = old.n8.as_ref()?;
        if kept.parts.merged_all.is_some() {
            return None;
        }
        let (part_of, above, width) = r.parts.by_key(old);
        let (part_of, above) = (Rc::new(part_of), Rc::new(above));
        if width != kept.parts.width() {
            return None;
        }
        let mut replayed = alloc::vec![true; width];
        replayed[0] = false;
        let mut held = alloc::vec![None; width];
        for &(p, m) in running {
            if let Some(x) = replayed.get_mut(p as usize) {
                *x = false;
                held[p as usize] = Some(m);
            }
        }
        // A part that runs from the start may be replayed later.
        if replayed.len() < 2 {
            return None;
        }
        let since = alloc::vec![0; width];
        let old_keys: Vec<u64> = (0..old.context_count() as CtxId)
            .map(|c| r.parts.key(c))
            .collect();
        let old_places: Vec<u64> = (0..old.records.grobs.len() as u32)
            .map(|i| r.grob_place(old, i))
            .collect();
        Some(Replay {
            log,
            moments: Rc::new(kept.steps.iter().map(|s| s.moment).collect()),
            rows: Rc::new(kept.parts.clone()),
            part_of,
            above,
            replayed,
            since,
            checkpoints: Rc::new(kept.checkpoints.iter().map(|c| c.moment).collect()),
            old_keys: Rc::new(old_keys),
            old_places: Rc::new(old_places),
            old_names: Rc::new(old.records.grobs.iter().map(|g| g.name).collect()),
            old_interfaces: Rc::new(old.records.interfaces.clone()),
            grobs: BTreeMap::new(),
            places: BTreeMap::new(),
            scanned: 0,
            audio: BTreeMap::new(),
            audio_base: 0,
            grob_base: 0,
            by_context: Vec::new(),
            at: None,
            writer: None,
            applied: alloc::vec![0; width],
            matched: None,
            start: (0, 0),
            failed: Vec::new(),
            held,
            fails: alloc::vec![0; width],
            last_failed: Vec::new(),
            points: Vec::new(),
        })
    }

    /// It begins at this run's timestep `step` (the old run's at its
    /// moment), its log `from` effects long, so many grob and audio records
    /// made.
    pub(crate) fn begin_at(&mut self, step: u32, from: u32, (grobs, audio): (u32, u32)) {
        self.start = (step, from);
        self.grob_base = grobs;
        self.audio_base = audio;
        self.matched = step.checked_sub(1);
        for s in &mut self.since {
            *s = step;
        }
    }

    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        self.log.borrow().trace(tracer);
    }

    /// Whether the context of key `key` is of a part replayed now.
    pub(crate) fn is_replayed(&self, key: u64) -> bool {
        self.replays(key).is_some()
    }

    /// This replay as it stands is a place to go back to: the snapshot
    /// `snapshot`, taken at this run's moment `m`. The snapshot of the point
    /// it takes the place of, to drop, if the points are full.
    pub(crate) fn add_point(&mut self, mut snapshot: usize, m: Moment) -> Option<usize> {
        let step = self.old_step(m).unwrap_or(0);
        let mut replay = self.clone();
        replay.points = Vec::new();
        let dropped = (self.points.len() > POINTS).then(|| self.points.remove(1).snapshot);
        if let Some(d) = dropped {
            for p in &mut self.points {
                p.snapshot -= usize::from(p.snapshot > d);
            }
            snapshot -= usize::from(snapshot > d);
        }
        self.points.push(Point {
            snapshot,
            step,
            replay: Box::new(replay),
        });
        dropped
    }

    /// Where to go back to for the parts that failed: the last point at
    /// which they all held what they ran to (the first always, where every
    /// part did), and the replay to run from there, with them running past
    /// where they failed. The first point's snapshot and `None` after too
    /// many failures of one part's.
    pub(crate) fn back(mut self) -> (usize, Option<Box<Replay>>) {
        let failed = core::mem::take(&mut self.failed);
        let mut fails = self.fails.clone();
        for &(p, _) in &failed {
            fails[p as usize] = fails[p as usize].saturating_add(1);
        }
        let held = |rp: &Replay, step: u32| {
            failed.iter().all(|&(p, _)| {
                let p = p as usize;
                !rp.replayed[p] || rp.since[p] > step
            })
        };
        let i = (1..self.points.len())
            .rev()
            .find(|&i| held(&self.points[i].replay, self.points[i].step))
            .unwrap_or(0);
        self.points.truncate(i + 1);
        let Some(point) = self.points.last() else {
            return (0, None);
        };
        let snapshot = point.snapshot;
        let mut next = point.replay.clone();
        next.points = self.points;
        for &(p, m) in &failed {
            let p = p as usize;
            next.replayed[p] = false;
            next.held[p] = Some(if fails[p] >= FAILS {
                crate::iterator::INFINITY
            } else {
                m
            });
        }
        next.last_failed = self.last_failed;
        for &(p, m) in &failed {
            match next.last_failed.iter_mut().find(|f| f.0 == p) {
                Some(f) => f.1 = m,
                None => next.last_failed.push((p, m)),
            }
        }
        next.fails = fails;
        (snapshot, Some(next))
    }

    /// The parts that have failed, each with the last moment it did.
    pub(crate) fn last_failed(&self) -> &[(u32, Moment)] {
        &self.last_failed
    }

    fn fail(&mut self, p: u32, m: Moment) {
        if !self.failed.iter().any(|f| f.0 == p) {
            self.failed.push((p, m));
        }
    }

    fn has_failed(&self, p: u32) -> bool {
        self.failed.iter().any(|f| f.0 == p)
    }

    /// The part of the context of key `key`, if it is replayed.
    fn replays(&self, key: u64) -> Option<u32> {
        let p = *self.part_of.get(&key)?;
        (p != 0 && self.replayed[p as usize] && !self.has_failed(p)).then_some(p)
    }

    /// The part of this run's context `c` of key `key`, [`NO_PART`] if the
    /// old run had none of its key.
    fn part(&mut self, c: CtxId, key: u64) -> u32 {
        let i = c as usize;
        if self.by_context.len() <= i {
            self.by_context.resize(i + 1, UNKNOWN);
        }
        if self.by_context[i] == UNKNOWN {
            self.by_context[i] = self.part_of.get(&key).copied().unwrap_or(NO_PART);
        }
        self.by_context[i]
    }

    fn old_step(&mut self, m: Moment) -> Option<u32> {
        if let Some((at, o)) = self.at
            && at == m
        {
            return o;
        }
        let i = self.moments.partition_point(|x| x.compare(m).is_lt());
        let o = (self.moments.get(i) == Some(&m)).then_some(i as u32);
        self.at = Some((m, o));
        o
    }
}

/// The context of this run whose key is the old run's context `c`'s.
fn context(a: &crate::context::Arena, rp: &Replay, c: CtxId) -> Option<CtxId> {
    let key = *rp.old_keys.get(c as usize)?;
    a.n8.as_ref()?.parts.context_of(key)
}

/// This run's grob for the old run's grob `old`: the one a replay copied in
/// for it, or the one at its place.
fn grob(a: &crate::context::Arena, rp: &mut Replay, old: u32) -> Option<u32> {
    if old < rp.grob_base {
        return Some(old);
    }
    if let Some(&g) = rp.grobs.get(&old) {
        return Some(g);
    }
    let place = *rp.old_places.get(old as usize)?;
    let r = a.n8.as_ref()?;
    while rp.scanned < a.records.grobs.len() {
        let i = rp.scanned as u32;
        rp.places.insert(r.grob_place(a, i), i);
        rp.scanned += 1;
    }
    let new = rp.places.get(&place).copied()?;
    (a.records.grobs[new as usize].name == *rp.old_names.get(old as usize)?).then_some(new)
}

/// This run's audio record for the old run's `old`.
fn audio(rp: &Replay, old: u32) -> Option<u32> {
    if old < rp.audio_base {
        return Some(old);
    }
    rp.audio.get(&old).copied()
}

/// The audio element `e` of the old run with the audio records it names
/// this run's; a staff's items as `items`.
fn audio_here(rp: &Replay, e: Audio) -> Option<Audio> {
    let map = |o: Option<u32>| match o {
        Some(o) => audio(rp, o).map(Some),
        None => Some(None),
    };
    Some(match e {
        Audio::Note(mut n) => {
            n.dynamic = map(n.dynamic)?;
            n.tied = map(n.tied)?;
            Audio::Note(n)
        }
        Audio::Staff(mut s) => {
            s.items = s
                .items
                .iter()
                .map(|&i| audio(rp, i))
                .collect::<Option<Vec<u32>>>()?;
            Audio::Staff(s)
        }
        other => other,
    })
}

/// N8's replay (MS6 step 7b): in place of a call of a translator of the
/// context `c`, the old run's same call's effects, if `c`'s part is
/// replayed; whether it was.
pub(crate) fn instead(
    host: &mut dyn crate::context::Host,
    t: &crate::context::Translation,
    c: CtxId,
    slot: u16,
) -> Result<bool, lily_engine_scheme::Exception> {
    if !on() {
        return Ok(false);
    }
    let found = {
        let heap = host.heap();
        let mut a = t.arena.borrow_mut();
        let Some(mut rp) = a.replay.take() else {
            return Ok(false);
        };
        let found = (|| {
            let mut r = a.n8.take()?;
            if !r.parts.knows(c) {
                r.parts.see(heap, &a);
            }
            let key = r.parts.key(c);
            a.n8 = Some(r);
            let p = rp.part(c, key);
            let now = a.now();
            let from = |p: u32, rp: &mut Replay| {
                // A part that goes back to replay runs out the timestep it
                // does at.
                rp.old_step(now).is_none_or(|o| o >= rp.since[p as usize])
            };
            let live = p != NO_PART
                && p != 0
                && rp.replayed[p as usize]
                && (rp.failed.is_empty() || !rp.has_failed(p))
                && from(p, &mut rp);
            if !live {
                return None;
            }
            let log = a.log.clone()?;
            let call = log.borrow_mut().next(c, key);
            Some((p, key, call, log))
        })();
        let found = found.map(|(p, key, call, log)| {
            let now = a.now();
            let old = rp.old_step(now);
            let effects = old.and_then(|o| rp.log.borrow().call((o, key, call.2)));
            match (old, &effects) {
                (None, _) | (Some(_), Some((false, ..))) => rp.fail(p, now),
                (Some(_), None) => {}
                (Some(_), Some((true, ..))) => rp.applied[p as usize] += 1,
            }
            (p, call, log, effects.filter(|e| e.0))
        });
        a.replay = Some(rp);
        found
    };
    let Some((p, call, log, effects)) = found else {
        return Ok(false);
    };
    REPLAYED.fetch_add(1, Ordering::Relaxed);
    let Some((_, chunk, from, to)) = effects else {
        return Ok(true);
    };
    let errors = host.error_len();
    log.borrow_mut().begin(call, errors);
    let was = t.arena.borrow().writer.replace(Some(c));
    let mut ok = true;
    for e in &chunk.effects[from as usize..to as usize] {
        if !apply(host, t, c, slot, e.clone())? {
            ok = false;
            break;
        }
    }
    t.arena.borrow().writer.set(was);
    let errors = host.error_len();
    log.borrow_mut().end(host.heap(), errors);
    let now = t.arena.borrow().now();
    if !ok && let Some(rp) = t.arena.borrow_mut().replay.as_mut() {
        rp.fail(p, now);
    }
    Ok(true)
}

/// The old run's effect `e` made again in the call of the translator
/// `slot` of `c`; false where it cannot be.
fn apply(
    host: &mut dyn crate::context::Host,
    t: &crate::context::Translation,
    c: CtxId,
    slot: u16,
    e: Effect,
) -> Result<bool, lily_engine_scheme::Exception> {
    match e {
        Effect::Grob(old, g) => {
            let mut g = *g;
            let mut a = t.arena.borrow_mut();
            let Some(mut rp) = a.replay.take() else {
                return Ok(false);
            };
            let ok = (|| {
                g.origin.0 = context(&a, &rp, g.origin.0)?;
                g.cause = match g.cause {
                    Cause::Grob(n) => Cause::Grob(grob(&a, &mut rp, n)?),
                    Cause::Audio(_) => return None,
                    other => other,
                };
                g.moment = a.now();
                g.handle = Value::FALSE;
                let mut interfaces = crate::records::InterfaceSet::default();
                for k in 0..=255u8 {
                    if g.interfaces.contains(k) {
                        let sym = rp.old_interfaces.symbol(k);
                        interfaces.insert(a.records.interfaces.number(sym)?);
                    }
                }
                g.interfaces = interfaces;
                let id = a.records.grobs.len() as u32;
                a.records.grobs.push(g.clone());
                a.effect(Effect::Grob(id, Box::new(g)));
                rp.grobs.insert(old, id);
                Some(())
            })();
            a.replay = Some(rp);
            Ok(ok.is_some())
        }
        Effect::Announce {
            grob: g,
            ctx,
            end,
            local,
        } => {
            let mut a = t.arena.borrow_mut();
            let Some(mut rp) = a.replay.take() else {
                return Ok(false);
            };
            let found = grob(&a, &mut rp, g).zip(context(&a, &rp, ctx));
            a.replay = Some(rp);
            let Some((g, ctx)) = found else {
                return Ok(false);
            };
            if end {
                let now = a.now();
                a.records.grobs[g as usize].ended = Some(now);
            }
            if local {
                a.effect(Effect::Announce {
                    grob: g,
                    ctx,
                    end,
                    local,
                });
                crate::translator::announce_locally_in(&mut a, ctx, g, end);
            } else {
                crate::translator::announce_grob(&mut a, ctx, g, end);
            }
            Ok(true)
        }
        Effect::Write(id, op) => {
            let target = {
                let mut a = t.arena.borrow_mut();
                let Some(mut rp) = a.replay.take() else {
                    return Ok(false);
                };
                let target = grob(&a, &mut rp, id);
                a.replay = Some(rp);
                target
            };
            let Some(target) = target else {
                return Ok(false);
            };
            write(host, t, target, op)
        }
        Effect::Property(ctx, sym, v) => {
            let v = match v {
                // A copy: what holds it may change it in place.
                Some(v) => match resolve(host.heap(), v) {
                    Some(v) => Some(kept_value(host.heap(), v)),
                    None => return Ok(false),
                },
                None => None,
            };
            let mut a = t.arena.borrow_mut();
            let Some(rp) = a.replay.take() else {
                return Ok(false);
            };
            let ctx = context(&a, &rp, ctx);
            a.replay = Some(rp);
            let Some(ctx) = ctx else {
                return Ok(false);
            };
            match v {
                Some(v) => a.set(ctx, sym, v),
                None => a.unset(ctx, sym),
            }
            Ok(true)
        }
        Effect::Prepend(ctx, sym, key, val) => {
            let mut a = t.arena.borrow_mut();
            let Some(rp) = a.replay.take() else {
                return Ok(false);
            };
            let ctx = context(&a, &rp, ctx);
            a.replay = Some(rp);
            let Some(ctx) = ctx else {
                return Ok(false);
            };
            a.prepend_property(host.heap(), ctx, sym, key, val);
            Ok(true)
        }
        Effect::Moment(m) => {
            t.arena.borrow_mut().add_moment(m);
            Ok(true)
        }
        Effect::Keep => {
            let mut a = t.arena.borrow_mut();
            a.effect(Effect::Keep);
            crate::translator::keep_awake(&mut a, c, slot);
            Ok(true)
        }
        Effect::Audio(old, r) => {
            let mut r = *r;
            let mut a = t.arena.borrow_mut();
            let Some(mut rp) = a.replay.take() else {
                return Ok(false);
            };
            let ok = (|| {
                r.origin.0 = context(&a, &rp, r.origin.0)?;
                r.cause = match r.cause {
                    Cause::Grob(n) => Cause::Grob(grob(&a, &mut rp, n)?),
                    Cause::Audio(n) => Cause::Audio(audio(&rp, n)?),
                    other => other,
                };
                r.audio = audio_here(&rp, r.audio)?;
                r.moment = a.now();
                r.column = None;
                let id = crate::translator::add_audio(&mut a, r);
                rp.audio.insert(old, id);
                Some(())
            })();
            a.replay = Some(rp);
            Ok(ok.is_some())
        }
        Effect::AudioSet(old, e, channel) => {
            let mut a = t.arena.borrow_mut();
            let Some(rp) = a.replay.take() else {
                return Ok(false);
            };
            let found = audio(&rp, old).zip(audio_here(&rp, *e));
            a.replay = Some(rp);
            let Some((id, mut e)) = found else {
                return Ok(false);
            };
            let r = &mut a.records.audio[id as usize];
            if let (Audio::Staff(new), Audio::Staff(was)) = (&mut e, &mut r.audio) {
                new.items = core::mem::take(&mut was.items);
            }
            r.audio = e;
            r.channel = channel;
            a.audio_changed(id);
            Ok(true)
        }
        Effect::StaffItem(staff, item) => {
            let mut a = t.arena.borrow_mut();
            let Some(rp) = a.replay.take() else {
                return Ok(false);
            };
            let found = audio(&rp, staff).zip(audio(&rp, item));
            a.replay = Some(rp);
            let Some((staff, item)) = found else {
                return Ok(false);
            };
            crate::performers::add_to_staff(&mut a, staff, item);
            Ok(true)
        }
        Effect::StaffPerformer(begun) => {
            crate::performance::staff_performer(&mut t.arena.borrow_mut(), begun);
            Ok(true)
        }
        Effect::Playback(ctx, staff) => {
            let mut a = t.arena.borrow_mut();
            let Some(rp) = a.replay.take() else {
                return Ok(false);
            };
            let ctx = context(&a, &rp, ctx);
            a.replay = Some(rp);
            let Some(ctx) = ctx else {
                return Ok(false);
            };
            crate::playback::replayed(&mut a, ctx, *staff);
            Ok(true)
        }
    }
}

/// The value `v` of an old run's effect as this run holds it: the handle of
/// one of its grobs, which names it by the old run's number, the handle of
/// this run's grob for it (copied in by a replay, or in its place); `None`
/// where there is none. Other values are taken as they are.
pub fn resolve(heap: &mut lily_engine_scheme::Heap, v: Value) -> Option<Value> {
    let mut budget = NODES;
    resolve_in(heap, v, &mut budget)
}

/// The pairs and vectors a value of the log is followed through, at most.
const NODES: usize = 4096;

fn resolve_in(heap: &mut lily_engine_scheme::Heap, v: Value, budget: &mut usize) -> Option<Value> {
    *budget = budget.checked_sub(1)?;
    if let (Some(car), Some(cdr)) = (heap.car(v), heap.cdr(v)) {
        let (a, d) = (
            resolve_in(heap, car, budget)?,
            resolve_in(heap, cdr, budget)?,
        );
        return Some(if (a, d) == (car, cdr) {
            v
        } else {
            heap.cons(a, d)
        });
    }
    if let Some(items) = heap.vector_items(v) {
        let items = items.to_vec();
        let mut out = Vec::with_capacity(items.len());
        for &x in &items {
            out.push(resolve_in(heap, x, budget)?);
        }
        return Some(if out == items { v } else { heap.vector(out) });
    }
    let Some(h) = heap.foreign::<crate::scheme_translator::GrobHandle>(v) else {
        return Some(v);
    };
    let (arena, translation, old) = (h.arena(), h.translation, h.id);
    let new = {
        let mut a = arena.borrow_mut();
        let Some(mut rp) = a.replay.take() else {
            return Some(v);
        };
        let new = grob(&a, &mut rp, old);
        a.replay = Some(rp);
        new?
    };
    Some(crate::scheme_translator::grob_handle(
        heap,
        &arena,
        translation,
        new,
    ))
}

/// `v` as the log keeps it: its pairs and vectors copied, since what holds
/// them may change them after (layout adds its broken copies); `v` itself
/// past [`NODES`] of them.
fn kept_value(heap: &mut lily_engine_scheme::Heap, v: Value) -> Value {
    let mut budget = NODES;
    copy_in(heap, v, &mut budget).unwrap_or(v)
}

fn copy_in(heap: &mut lily_engine_scheme::Heap, v: Value, budget: &mut usize) -> Option<Value> {
    *budget = budget.checked_sub(1)?;
    if let (Some(car), Some(cdr)) = (heap.car(v), heap.cdr(v)) {
        let (a, d) = (copy_in(heap, car, budget)?, copy_in(heap, cdr, budget)?);
        return Some(heap.cons(a, d));
    }
    if let Some(items) = heap.vector_items(v) {
        let items = items.to_vec();
        let mut out = Vec::with_capacity(items.len());
        for &x in &items {
            out.push(copy_in(heap, x, budget)?);
        }
        return Some(heap.vector(out));
    }
    Some(v)
}

/// The write `op` made again to this run's grob `target`.
fn write(
    host: &mut dyn crate::context::Host,
    t: &crate::context::Translation,
    target: u32,
    op: Op,
) -> Result<bool, lily_engine_scheme::Exception> {
    let resolve = |host: &mut dyn crate::context::Host, v: Value| resolve(host.heap(), v);
    let handle = crate::scheme_translator::grob_handle(host.heap(), &t.arena, t.value, target);
    let (kind, a, b, flag) = match op {
        Op::Record(sym, v) => {
            let Some(v) = resolve(host, v) else {
                return Ok(false);
            };
            crate::context::grob_wrote(&t.arena, Some(target), Op::Record(sym, v), None);
            crate::scheme_translator::set_record_property(host.heap(), &t.arena, target, sym, v);
            return Ok(true);
        }
        Op::Property(sym, v) => (0, sym, v, false),
        Op::Object(sym, v) => (1, sym, v, false),
        Op::Parent(axis, p) => (2, host.heap().integer(i64::from(axis)), p, false),
        Op::Add(sym, p, ordered) => (3, sym, p, ordered),
        Op::Order(sym, ordered) => (4, sym, Value::FALSE, ordered),
        Op::Bound(d, g) => (5, host.heap().integer(i64::from(d)), g, false),
        Op::Suicide => (6, Value::FALSE, Value::FALSE, false),
        Op::Reset(sym, g) => (7, sym, g, false),
        Op::Unknown => return Ok(false),
    };
    let Some(b) = resolve(host, b) else {
        return Ok(false);
    };
    let writer = t.arena.borrow().replay.as_ref().and_then(|r| r.writer);
    let (f, what) = match writer {
        Some(w) => w,
        None => {
            let Some(f) = host.lily("%host-call") else {
                return Ok(false);
            };
            let what = host.heap().intern("n8-write");
            if let Some(r) = t.arena.borrow_mut().replay.as_mut() {
                r.writer = Some((f, what));
            }
            (f, what)
        }
    };
    let heap = host.heap();
    let flag = if flag { Value::TRUE } else { Value::FALSE };
    let kind = heap.integer(kind);
    Ok(host.call(f, &[what, handle, kind, a, b, flag])? != Value::FALSE)
}

/// N8's replay's check of the timestep `step` of this run, just ended:
/// each replayed part heard and read what the old run's did at its moment,
/// and made the calls it made; one that did not is failed.
pub(crate) fn check(a: &mut crate::context::Arena, step: usize, m: Moment) {
    if !on() {
        return;
    }
    let Some(mut rp) = a.replay.take() else {
        return;
    };
    if let Some(r) = a.n8.as_ref()
        && let Some((heard, shared)) = r.parts.rows(step)
    {
        let width = rp.replayed.len();
        let old = rp.old_step(m);
        let at = |rp: &Replay, p: usize, o: u32| {
            rp.replayed[p] && o >= rp.since[p] && !rp.has_failed(p as u32)
        };
        let mut failed = Vec::new();
        match old {
            None => {
                for p in 1..width {
                    if rp.replayed[p] && !rp.has_failed(p as u32) {
                        failed.push(p as u32);
                    }
                }
            }
            Some(o) => {
                // What each heard.
                let mut input = alloc::vec![(crate::checkpoint::Digest::new(31), false); width];
                for &(c, x) in heard {
                    let key = r.parts.key(c);
                    let p = rp.part(c, key);
                    if p != NO_PART {
                        let i = &mut input[p as usize];
                        i.0.u64(key);
                        i.0.u64(x);
                        i.1 = true;
                    }
                }
                // The old calls with effects of each one's contexts, at the
                // old timesteps from the one after the last matched to this
                // one, all applied.
                let mut calls = alloc::vec![0u32; width];
                let from = rp.matched.map_or(0, |x| x + 1);
                let mut last = (u64::MAX, NO_PART);
                for s in from..=o {
                    rp.log.borrow().each_call(s, |key, at| {
                        if s == rp.start.0 && at < rp.start.1 {
                            return;
                        }
                        if last.0 != key {
                            last = (key, rp.part_of.get(&key).copied().unwrap_or(NO_PART));
                        }
                        let p = last.1;
                        if p != NO_PART && s >= rp.since[p as usize] {
                            calls[p as usize] += 1;
                        }
                    });
                }
                // As many calls of each one's contexts as the old run made:
                // a call it did not make did nothing it can make again.
                let mut made = alloc::vec![0u64; width];
                let mut sum = |rp: &mut Replay, list: &[(u64, u32)], sign: u64| {
                    for &(key, n) in list {
                        if let Some(&p) = rp.part_of.get(&key) {
                            let d = key.wrapping_mul(u64::from(n) + 1).wrapping_mul(sign);
                            made[p as usize] = made[p as usize].wrapping_add(d);
                        }
                    }
                };
                if let Some(log) = a.log.clone() {
                    let l = log.borrow();
                    sum(&mut rp, l.made(step as u32), 1);
                }
                let old_log = Rc::clone(&rp.log);
                sum(&mut rp, old_log.borrow().made(o), u64::MAX);
                // Its shared contexts' row, for each set of them once.
                let mut sums: Vec<Option<u64>> = alloc::vec![None; width];
                for p in 1..width {
                    if !at(&rp, p, o) {
                        continue;
                    }
                    let same = (1..p).find(|&q| rp.above[q] == rp.above[p]);
                    let sum = match same.and_then(|q| sums[q]) {
                        Some(sum) => sum,
                        None => {
                            let mut d = crate::checkpoint::Digest::new(37);
                            for &(c, sum) in shared {
                                if rp.above[p].contains(&r.parts.key(c)) {
                                    d.u64(r.parts.key(c));
                                    d.u64(sum);
                                }
                            }
                            d.get()
                        }
                    };
                    sums[p] = Some(sum);
                    let (i, any) = (input[p].0.get(), input[p].1);
                    let (old_input, old_shared, _) = rp.rows.row(o as usize, p);
                    if (if any { i } else { 0 }) != old_input
                        || sum != old_shared
                        || calls[p] != rp.applied[p]
                        || made[p] != 0
                    {
                        failed.push(p as u32);
                    }
                }
            }
        }
        for p in failed {
            rp.fail(p, m);
        }
        rp.applied.fill(0);
        if old.is_some() {
            rp.matched = old;
        }
    }
    a.replay = Some(rp);
}

/// N8's replay at a checkpoint of this run just taken, at `m`: each part
/// that runs, whose state and future there are the old run's at its
/// checkpoint of the same moment, replayed from the next timestep on;
/// whether one was.
pub(crate) fn converged(a: &mut crate::context::Arena, m: Moment) -> bool {
    let Some(mut rp) = a.replay.take() else {
        return false;
    };
    let mut any = false;
    if let (Some(r), Some(k)) = (a.n8.as_ref(), rp.checkpoints.iter().position(|c| *c == m))
        && let Some(o) = rp.old_step(m)
    {
        for p in 1..rp.replayed.len() as u32 {
            if rp.replayed[p as usize] || rp.has_failed(p) {
                continue;
            }
            let old = rp.rows.state(k, p as usize);
            let held = rp.held[p as usize].is_some_and(|h| !m.compare(h).is_gt());
            if !held && old.is_some() && r.parts.last_state(&rp.part_of, p) == old {
                rp.replayed[p as usize] = true;
                rp.since[p as usize] = o + 1;
                any = true;
            }
        }
    }
    a.replay = Some(rp);
    any
}
