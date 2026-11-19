//! N8's partitions (04 §10.3, §10.4; MS6 step 6): a translation's contexts
//! split into partitions and the shared level, what each part heard, made
//! and held, and which of them a compile after an edit runs again.
//!
//! - **Partitions** ([`Tracker`], while the translation runs): a *line
//!   context*, one whose translators make a `VerticalAxisGroup`
//!   (`Axis_group_engraver`) or, in a `\midi` translation, a staff's track
//!   (`Staff_performer`), the topmost of its line, with every context made
//!   below it. The shared level is Global, the Score and every context
//!   with no line context above it. A context stays in the line it was
//!   made in, and partitions are merged where contexts depend on each
//!   other: where a context moves into another line (`\change Staff`), a
//!   lyric combine joins a Lyrics context with its voice (`\lyricsto`,
//!   `associatedVoice`), and all of them where a context's procedures
//!   might read other contexts: a translator the score gives as a procedure
//!   or an alist, a Scheme listener (`ly:add-listener`), or an
//!   `\applyContext` procedure in a partition whose code is not the image's
//!   (LilyPond's own Scheme, whose procedures touch their own context and
//!   the shared level: `ly:set-middle-C!`, `\time`'s, `\partial`'s). Its
//!   contexts are named by their parents', their names, ids and how many
//!   of their kind came before them there ([`Tracker::key`]), which an
//!   edit that makes a context in another line does not change.
//! - **What each part recorded** ([`Parts`], kept): per timestep, a digest
//!   of the events its contexts heard, and of the shared level's
//!   properties as the timestep left them with every write made to them
//!   in it (a property set and unset in one timestep, D240); per timestep
//!   the digest of its records; per checkpoint, of its state (its contexts' properties,
//!   translators and iterators) and of its future (the music its iterators
//!   have left). Part 0 is the shared level.
//! - **The run after an edit** ([`plan`], §10.4): from the checkpoint
//!   translation resumes from to the convergence the shared level runs. A
//!   partition starts running at the first timestep whose events differ
//!   from the old run's at that moment, or whose shared level's properties
//!   do, caught up from its own last checkpoint, and stops at a checkpoint
//!   where its state and future are the old run's. Otherwise its old
//!   records are replayed. A timestep only the new run has replays nothing
//!   where the partition heard nothing and the shared level did not change
//!   at it; one only the old run has needs the partition to run where it
//!   heard or made anything there.
//! - **The check** ([`verify`]): whole runs against the plan: each
//!   replayed timestep's records the old run's at its moment, and each
//!   replayed checkpoint's state the old run's (D239).
//!
//! Replaying them is `replay.rs`'s, by what it checks as it runs; the plan
//! is its figure and the check of the digests against whole runs.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Heap, Value};

use crate::Moment;
use crate::checkpoint::{Change, Digest, Kept};
use crate::context::{Arena, CtxId, GLOBAL};

/// The translators that make a context a line context.
const LINE_TRANSLATORS: [&str; 2] = ["Axis_group_engraver", "Staff_performer"];

/// A context as the tracker first saw it.
#[derive(Clone, Copy)]
struct Seen {
    key: u64,
    /// The line it was made in, by number, or the shared level.
    line: Option<u32>,
    /// Its translators.
    weight: u32,
}

/// A line: its top context's name, the contexts above it, and the line it
/// is merged into.
#[derive(Clone)]
struct Line {
    name: String,
    above: Vec<CtxId>,
    up: u32,
}

/// A grob written by a translator of another context than the one that
/// made it: the writer, the grob's context (`None` for a grob not known),
/// its place, and the context of the grob the write added to one of its
/// grob arrays, if it did.
type Write = (CtxId, Option<CtxId>, u64, Option<CtxId>);

/// A timestep's digests by context.
pub(crate) type Row = [(CtxId, u64)];

/// What a translation holds at a checkpoint: each context's state and
/// future, and the shared level's own.
type Held = (Vec<(CtxId, u64, u64)>, u64, u64);

/// What a translation's recording keeps of its partitions while it runs.
#[derive(Clone, Default)]
pub(crate) struct Tracker {
    seen: Vec<Seen>,
    /// Contexts made with each key's parent, name and id so far.
    kinds: BTreeMap<u64, u32>,
    lines: Vec<Line>,
    /// Why every partition is one, if it is.
    merged_all: Option<&'static str>,
    /// Contexts that ran an `\applyContext` procedure of the score's own.
    applied: Vec<CtxId>,
    /// What contexts heard at the timestep being run, and at each before.
    heard: Vec<(CtxId, u64)>,
    steps: Vec<Vec<(CtxId, u64)>>,
    /// What each timestep made, as it ended, by context.
    made: Vec<Vec<(CtxId, u64)>>,
    /// The shared level's properties: each one's digest by context and
    /// symbol, their sum by context, and the sums as each timestep ended.
    shared_props: BTreeMap<(CtxId, u64), u64>,
    shared_sums: BTreeMap<CtxId, u64>,
    shared: Vec<Vec<(CtxId, u64)>>,
    /// The writes to each shared context in the timestep being run, in
    /// order.
    shared_writes: BTreeMap<CtxId, Digest>,
    /// The grobs written in the timestep being run by a translator of
    /// another context than the one that made them (the writer, the
    /// grob's context or none for a grob not known, its place), and those
    /// of each timestep before (MS6 step 7a).
    writes: Vec<Write>,
    wrote: Vec<Vec<Write>>,
    /// Per checkpoint, each context's state and future, and the shared
    /// level's own (the moment and what belongs to no context).
    checkpoints: Vec<Held>,
    /// The same of the first timestep, after its `Prepare`.
    start: Option<Held>,
    /// Each part's shared contexts above it, once its partitions are known.
    above: Vec<Vec<CtxId>>,
}

impl Tracker {
    /// The stable name of context `c`, its number for one not seen.
    pub(crate) fn key(&self, c: CtxId) -> u64 {
        self.seen
            .get(c as usize)
            .map_or(u64::from(c) | 1 << 40, |s| s.key)
    }

    /// Whether `c` has been seen.
    pub(crate) fn knows(&self, c: CtxId) -> bool {
        (c as usize) < self.seen.len()
    }

    /// Whether `c` is of the shared level.
    pub(crate) fn is_shared(&self, c: CtxId) -> bool {
        self.seen.get(c as usize).is_none_or(|s| s.line.is_none())
    }

    /// Sees the contexts made since the last time, and merges the lines a
    /// context moved between and those a lyric combine joins.
    pub(crate) fn see(&mut self, heap: &Heap, a: &Arena) {
        self.see_new(heap, a);
        // A context in another line than it was made in: theirs merged.
        for c in 0..self.seen.len() as CtxId {
            if a.removed(c) {
                continue;
            }
            let now = a.parent(c).and_then(|p| self.seen.get(p as usize)?.line);
            let was = self.seen[c as usize].line;
            let top = was.is_some() && a.parent(c).is_some_and(|p| self.is_shared(p));
            match (was, now) {
                _ if top => {}
                (Some(x), Some(y)) => self.union(x, y),
                (None, None) => {}
                _ => self.merge_all("a context moved between a line and the shared level"),
            }
        }
        for combine in &a.combines {
            if let (Some(l), Some(m)) = (combine.lyrics, combine.music) {
                let line = |c: CtxId| self.seen.get(c as usize).and_then(|s| s.line);
                if let (Some(x), Some(y)) = (line(l), line(m)) {
                    self.union(x, y);
                }
            }
        }
    }

    /// Sees the contexts made since the last time: their keys.
    pub(crate) fn see_new(&mut self, heap: &Heap, a: &Arena) {
        for c in self.seen.len() as CtxId..a.context_count() as CtxId {
            let parent = a.parent(c);
            let up = parent.and_then(|p| self.seen.get(p as usize).copied());
            let mut d = Digest::new(30);
            d.u64(up.map_or(0, |s| s.key));
            d.str(heap.name(a.name(heap, c)).unwrap_or_default());
            d.str(a.id(c));
            let kind = d.get();
            let n = self.kinds.entry(kind).or_insert(0);
            d.u64((*n).into());
            *n += 1;
            let names = a.translator_names(c);
            let is_line = names.iter().any(|n| LINE_TRANSLATORS.contains(&n.as_str()));
            let line = match up.and_then(|s| s.line) {
                Some(l) => Some(l),
                None if is_line && c != GLOBAL => {
                    let l = self.lines.len() as u32;
                    let mut name = String::new();
                    self.path(heap, a, c, &mut name);
                    if *self.kinds.get(&kind).unwrap_or(&1) > 1 {
                        name.push_str(&format!("#{}", self.kinds[&kind] - 1));
                    }
                    let mut above = Vec::new();
                    let mut p = parent;
                    while let Some(x) = p {
                        above.push(x);
                        p = a.parent(x);
                    }
                    self.lines.push(Line { name, above, up: l });
                    Some(l)
                }
                None => None,
            };
            self.seen.push(Seen {
                key: d.get(),
                line,
                weight: names.len() as u32,
            });
        }
    }

    /// N8's splice (D243): the contexts and lines `old`, the old run's as it
    /// ended, saw after those this one has seen, which are the same: its
    /// keys for what the old run made after the splice.
    pub(crate) fn adopt(&mut self, old: &Tracker) {
        let (contexts, lines) = (self.seen.len(), self.lines.len());
        self.seen
            .extend_from_slice(old.seen.get(contexts..).unwrap_or_default());
        // The lines it merged after, a voice changing staff among them.
        for (line, was) in self.lines.iter_mut().zip(&old.lines) {
            line.up = was.up;
        }
        for line in old.lines.get(lines..).unwrap_or_default() {
            self.lines.push(Line {
                name: line.name.clone(),
                above: line.above.clone(),
                up: line.up,
            });
        }
        self.kinds = old.kinds.clone();
        if self.merged_all.is_none() {
            self.merged_all = old.merged_all;
        }
        for &c in &old.applied {
            self.applied(c);
        }
    }

    /// `c`'s contexts from the Score down, as `Staff=one/Voice`.
    fn path(&self, heap: &Heap, a: &Arena, c: CtxId, out: &mut String) {
        if let Some(p) = a.parent(c).filter(|&p| p != GLOBAL) {
            self.path(heap, a, p, out);
            out.push('/');
        }
        out.push_str(heap.name(a.name(heap, c)).unwrap_or_default());
        if !a.id(c).is_empty() {
            out.push('=');
            out.push_str(a.id(c));
        }
    }

    fn find(&mut self, mut l: u32) -> u32 {
        while self.lines[l as usize].up != l {
            let up = self.lines[l as usize].up;
            self.lines[l as usize].up = self.lines[up as usize].up;
            l = up;
        }
        l
    }

    fn union(&mut self, x: u32, y: u32) {
        let (x, y) = (self.find(x), self.find(y));
        let (lo, hi) = (x.min(y), x.max(y));
        self.lines[hi as usize].up = lo;
    }

    /// Every partition is merged into one, for `why`.
    pub(crate) fn merge_all(&mut self, why: &'static str) {
        self.merged_all.get_or_insert(why);
    }

    /// Context `c` ran an `\applyContext` procedure not of the image's code.
    pub(crate) fn applied(&mut self, c: CtxId) {
        if !self.applied.contains(&c) {
            self.applied.push(c);
        }
    }

    /// Context `c` heard the event whose digest is `d`.
    pub(crate) fn heard(&mut self, c: CtxId, d: u64) {
        self.heard.push((c, d));
    }

    /// The shared level's property `sym` of `c` is `d` now (0: unset).
    pub(crate) fn shared_property(&mut self, c: CtxId, sym: Value, d: u64) {
        let old = self.shared_props.insert((c, sym.to_bits()), d).unwrap_or(0);
        let sum = self.shared_sums.entry(c).or_insert(0);
        *sum = sum.wrapping_sub(old).wrapping_add(d);
    }

    /// The shared context `c` was written to in the timestep being run:
    /// `d` the write's digest.
    pub(crate) fn shared_write(&mut self, c: CtxId, d: u64) {
        self.shared_writes
            .entry(c)
            .or_insert_with(|| Digest::new(39))
            .u64(d);
    }

    /// A translator of `writer` wrote to the grob of `target` at `place`
    /// (`None`: a grob array whose grob is not known), adding a grob of
    /// `added` to one of its grob arrays if it did.
    pub(crate) fn wrote(
        &mut self,
        writer: CtxId,
        target: Option<CtxId>,
        place: u64,
        added: Option<CtxId>,
    ) {
        self.writes.push((writer, target, place, added));
    }

    /// The timestep being run ends, having made `made`: each record's
    /// digest by the context that made it. The shared contexts' row is
    /// their properties as it ends and the writes made to them in it.
    pub(crate) fn end_step(&mut self, made: Vec<(CtxId, u64)>) {
        self.steps.push(core::mem::take(&mut self.heard));
        self.made.push(made);
        self.wrote.push(core::mem::take(&mut self.writes));
        let mut row: BTreeMap<CtxId, u64> = self.shared_sums.clone();
        for (c, w) in core::mem::take(&mut self.shared_writes) {
            let d = row.entry(c).or_insert(0);
            *d = d.wrapping_add(w.get().wrapping_mul(5));
        }
        self.shared.push(row.into_iter().collect());
    }

    /// A checkpoint, or the first timestep's state: each context's state
    /// and future, and the shared level's own.
    pub(crate) fn checkpoint(&mut self, contexts: Vec<(CtxId, u64, u64)>, own: u64, future: u64) {
        let state = (contexts, own, future);
        if self.start.is_none() {
            self.start = Some(state);
        } else {
            self.checkpoints.push(state);
        }
    }

    /// The checkpoints from the `from`th on unknown: each context's state
    /// and future 0, the shared level's too (N8's replay, MS6 step 7b).
    pub(crate) fn forget_from(&mut self, from: usize) {
        for (contexts, own, future) in self.checkpoints.iter_mut().skip(from) {
            *own = 0;
            *future = 0;
            for c in contexts {
                c.1 = 0;
                c.2 = 0;
            }
        }
    }

    /// The futures held so far unknown (0): a resumed run's, taken over
    /// from the old run (`Recording::take_shape`).
    pub(crate) fn forget_futures(&mut self) {
        for (contexts, _, future) in self.checkpoints.iter_mut().chain(&mut self.start) {
            *future = 0;
            for c in contexts {
                c.2 = 0;
            }
        }
    }

    /// N8's replay (MS6 step 7b): each context's part by its key, the
    /// shared contexts above each part by their keys, and how many parts,
    /// as the translation's parts are now.
    pub(crate) fn by_key(&self, a: &Arena) -> (BTreeMap<u64, u32>, Vec<Vec<u64>>, usize) {
        let mut t = self.clone();
        let (part, parts) = t.parts(a);
        let of: BTreeMap<u64, u32> = (0..t.seen.len())
            .map(|c| (t.seen[c].key, part.get(c).copied().unwrap_or(0)))
            .collect();
        let above = t
            .above
            .iter()
            .map(|cs| cs.iter().map(|&c| t.key(c)).collect())
            .collect();
        (of, above, parts.weights.len())
    }

    /// What the contexts heard at timestep `step`, and the shared level's
    /// row as it ended, if it has ended.
    pub(crate) fn rows(&self, step: usize) -> Option<(&Row, &Row)> {
        Some((self.steps.get(step)?, self.shared.get(step)?))
    }

    /// Part `p`'s state and future at the last checkpoint, its contexts
    /// those `part_of` gives `p` by key, as `finish` sums them (N8's
    /// replay, MS6 step 7b).
    pub(crate) fn last_state(&self, part_of: &BTreeMap<u64, u32>, p: u32) -> Option<(u64, u64)> {
        let (contexts, _, _) = self.checkpoints.last()?;
        let mut sum = (0u64, 0u64);
        for &(c, state, future) in contexts {
            if part_of.get(&self.key(c)) == Some(&p) {
                let mut d = Digest::new(33);
                d.u64(self.key(c));
                d.u64(state);
                sum.0 = sum.0.wrapping_add(d.get());
                sum.1 = sum.1.wrapping_add(future);
            }
        }
        Some(sum)
    }

    /// The context of key `key`, if one has been seen.
    pub(crate) fn context_of(&self, key: u64) -> Option<CtxId> {
        self.seen
            .iter()
            .position(|s| s.key == key)
            .map(|c| c as CtxId)
    }

    /// Each context's part, 0 the shared level, and the parts' names and
    /// translators; all partitions one where they were merged.
    fn parts(&mut self, a: &Arena) -> (Vec<u32>, Parts) {
        for &c in &self.applied {
            if !self.is_shared(c) {
                self.merged_all
                    .get_or_insert("an \\applyContext procedure of the score's own");
            }
        }
        let mut number: BTreeMap<u32, u32> = BTreeMap::new();
        let mut names: Vec<Vec<String>> = Vec::new();
        for l in 0..self.lines.len() as u32 {
            let root = if self.merged_all.is_some() {
                0
            } else {
                self.find(l)
            };
            let n = *number.entry(root).or_insert_with(|| {
                names.push(Vec::new());
                names.len() as u32
            });
            names[n as usize - 1].push(self.lines[l as usize].name.clone());
        }
        let of_line: Vec<u32> = (0..self.lines.len() as u32)
            .map(|l| {
                let root = if self.merged_all.is_some() {
                    0
                } else {
                    self.find(l)
                };
                number[&root]
            })
            .collect();
        let part: Vec<u32> = (0..a.context_count())
            .map(|c| {
                self.seen
                    .get(c)
                    .and_then(|s| s.line)
                    .map_or(0, |l| of_line[l as usize])
            })
            .collect();
        let mut weights = alloc::vec![0u32; names.len() + 1];
        for (c, s) in self.seen.iter().enumerate() {
            weights[part[c] as usize] += s.weight;
        }
        // The shared contexts above each partition's lines: the only ones
        // whose properties its contexts read.
        let mut above: Vec<Vec<CtxId>> = alloc::vec![Vec::new(); names.len() + 1];
        for (l, line) in self.lines.iter().enumerate() {
            let p = &mut above[of_line[l] as usize];
            for &c in &line.above {
                if !p.contains(&c) {
                    p.push(c);
                }
            }
        }
        self.above = above;
        let parts = Parts {
            keys: names.into_iter().map(|n| n.join("+")).collect(),
            weights,
            merged_all: self.merged_all.map(String::from),
            ..Parts::default()
        };
        (part, parts)
    }

    /// The parts as the translation ended: what each heard, made and held.
    pub(crate) fn finish(mut self, a: &Arena) -> Parts {
        let (part, mut parts) = self.parts(a);
        let width = parts.weights.len();
        let of = |c: CtxId| part.get(c as usize).copied().unwrap_or(0) as usize;
        let fold = |each: &[(CtxId, u64)], tag: u64| {
            let mut ds: Vec<Digest> = (0..width).map(|_| Digest::new(tag)).collect();
            let mut any = alloc::vec![false; width];
            for &(c, d) in each {
                ds[of(c)].u64(self.key(c));
                ds[of(c)].u64(d);
                any[of(c)] = true;
            }
            ds.into_iter()
                .zip(any)
                .map(|(d, any)| if any { d.get() } else { 0 })
                .collect::<Vec<u64>>()
        };
        for step in &self.steps {
            parts.inputs.extend(fold(step, 31));
        }
        for step in &self.made {
            parts.outputs.extend(fold(step, 32));
        }
        // Per timestep, what each partition wrote outside itself: to a
        // grob of the shared level, of another partition or not known; and
        // apart, the grobs of its own it added to the shared level's grob
        // arrays (`spacing-wishes` of a column), which a replay can add
        // again.
        for step in &self.wrote {
            let mut ds: Vec<Digest> = (0..width).map(|_| Digest::new(40)).collect();
            let mut adds: Vec<Digest> = (0..width).map(|_| Digest::new(41)).collect();
            let mut any = alloc::vec![(false, false); width];
            for &(writer, target, place, added) in step {
                let p = of(writer);
                if p == 0 || target.is_some_and(|t| of(t) == p) {
                    continue;
                }
                let own = target.is_some_and(|t| of(t) == 0) && added.is_some_and(|c| of(c) == p);
                let d = if own { &mut adds[p] } else { &mut ds[p] };
                d.u64(target.map_or(u64::MAX, |t| self.key(t)));
                d.u64(place);
                if own {
                    any[p].1 = true;
                } else {
                    any[p].0 = true;
                }
            }
            for ((d, add), (out, added)) in ds.into_iter().zip(adds).zip(any) {
                parts.outs.push(if out { d.get() } else { 0 });
                parts.adds.push(if added { add.get() } else { 0 });
            }
        }
        // Per timestep, each partition's shared contexts' properties; the
        // shared level's all of them.
        for sums in &self.shared {
            for p in 0..width {
                let mut d = Digest::new(37);
                for &(c, sum) in sums {
                    if p == 0 || self.above[p].contains(&c) {
                        d.u64(self.key(c));
                        d.u64(sum);
                    }
                }
                parts.shared.push(d.get());
            }
        }
        let states = |(contexts, own, future): &Held| {
            let mut states = alloc::vec![(0u64, 0u64); width];
            states[0] = (*own, *future);
            for &(c, state, future) in contexts {
                let mut d = Digest::new(33);
                d.u64(self.key(c));
                d.u64(state);
                let p = &mut states[of(c)];
                p.0 = p.0.wrapping_add(d.get());
                p.1 = p.1.wrapping_add(future);
            }
            states
        };
        for checkpoint in &self.checkpoints {
            parts.states.extend(states(checkpoint));
        }
        parts.start = self.start.as_ref().map(states).unwrap_or_default();
        parts
    }
}

/// What a translation's parts recorded (§10.2, §10.3): part 0 the shared
/// level, then each partition; per timestep and per checkpoint a row of
/// one digest each.
#[derive(Clone, Debug, Default)]
pub struct Parts {
    /// Each partition's line contexts' names, joined by `+`.
    pub keys: Vec<String>,
    /// Each part's translators.
    pub weights: Vec<u32>,
    /// Why every partition was merged into one, if it was.
    pub merged_all: Option<String>,
    /// Per timestep, the events each part heard (0: none).
    pub inputs: Vec<u64>,
    /// Per timestep, the properties of the shared contexts above each part
    /// as it ended, the shared level's all of them.
    pub shared: Vec<u64>,
    /// Per timestep, each part's records (0: none).
    pub outputs: Vec<u64>,
    /// Per timestep, what each partition's translators wrote outside it
    /// (0: nothing), which a replay of it would miss, but the grobs of its
    /// own they added to the shared level's grob arrays, apart, which a
    /// replay adds again (MS6 step 7a).
    pub outs: Vec<u64>,
    pub adds: Vec<u64>,
    /// Per checkpoint, each part's state and future, and the same after
    /// the first timestep's `Prepare`.
    pub states: Vec<(u64, u64)>,
    pub start: Vec<(u64, u64)>,
}

impl Parts {
    /// N8's replay (MS6 step 7b): part `p`'s row at timestep `step`: what
    /// it heard, its shared contexts' properties, what it wrote outside
    /// itself.
    pub(crate) fn row(&self, step: usize, p: usize) -> (u64, u64, u64) {
        (self.input(step, p), self.shared(step, p), self.out(step, p))
    }

    pub(crate) fn width(&self) -> usize {
        self.weights.len()
    }

    /// N8's splice (D243): these rows, a run's to timestep `step` and its
    /// checkpoint before `checkpoint`, then the old run's `old` from there;
    /// none where their partitions are not the same (the old run's may have
    /// lines this one had not made yet).
    pub(crate) fn spliced(mut self, old: &Parts, step: usize, checkpoint: usize) -> Parts {
        if self.keys != old.keys || self.width() != old.width() {
            return Parts::default();
        }
        let w = self.width();
        self.weights = old.weights.clone();
        self.merged_all = old.merged_all.clone();
        let rows = |v: &[u64], from: usize| v.get(from * w..).unwrap_or_default().to_vec();
        self.inputs.extend(rows(&old.inputs, step));
        self.outputs.extend(rows(&old.outputs, step));
        self.outs.extend(rows(&old.outs, step));
        self.adds.extend(rows(&old.adds, step));
        self.shared.extend(rows(&old.shared, step));
        self.states
            .extend_from_slice(old.states.get(checkpoint * w..).unwrap_or_default());
        self
    }

    fn input(&self, step: usize, p: usize) -> u64 {
        self.inputs
            .get(step * self.width() + p)
            .copied()
            .unwrap_or(0)
    }

    /// The shared properties part `p` reads as timestep `step` ended, and
    /// as the one before it did (0 before the first).
    fn shared(&self, step: usize, p: usize) -> u64 {
        self.shared
            .get(step * self.width() + p)
            .copied()
            .unwrap_or(0)
    }

    fn shared_before(&self, step: usize, p: usize) -> u64 {
        step.checked_sub(1).map_or(0, |s| self.shared(s, p))
    }

    fn output(&self, step: usize, p: usize) -> u64 {
        self.outputs
            .get(step * self.width() + p)
            .copied()
            .unwrap_or(0)
    }

    fn out(&self, step: usize, p: usize) -> u64 {
        self.outs.get(step * self.width() + p).copied().unwrap_or(0)
    }

    pub(crate) fn state(&self, checkpoint: usize, p: usize) -> Option<(u64, u64)> {
        self.states.get(checkpoint * self.width() + p).copied()
    }
}

/// What a compile's translation runs with partitions (§10.4), against the
/// old run, from the checkpoint it resumes from to its convergence.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartRun {
    /// The partitions, and why all were merged into one if they were.
    pub partitions: u32,
    pub merged_all: Option<String>,
    /// The timesteps each partition runs, its catch-up included.
    pub runs: Vec<u32>,
    /// Translators times timesteps: those run with partitions, those run
    /// without them (every context from the resumed checkpoint to the
    /// convergence), and the whole translation's.
    pub work: u64,
    pub unpartitioned: u64,
    pub whole: u64,
    /// The work were a partition's writes outside itself replayed too
    /// (step 6's plan, which ran a partition only for what it heard and
    /// read; MS6 step 7a).
    pub reads_only: u64,
}

/// Which timesteps of the new run each partition runs at, from `from` to
/// `to`, by [`plan`].
pub(crate) struct Plan {
    from: usize,
    to: usize,
    /// Per partition (index 0 the first), per timestep from `from`.
    running: Vec<Vec<bool>>,
    pub(crate) run: PartRun,
}

/// The old run's timestep at `m`.
fn at(kept: &Kept, m: Moment) -> Option<usize> {
    let i = kept.steps.partition_point(|s| s.moment.compare(m).is_lt());
    (kept.steps.get(i)?.moment == m).then_some(i)
}

/// The old run's checkpoint at `m`.
fn checkpoint_at(kept: &Kept, m: Moment) -> Option<usize> {
    kept.checkpoints.iter().position(|c| c.moment == m)
}

/// Which partitions `new`'s translation runs at each timestep from the
/// checkpoint `change` resumes from to its convergence; `None` where the
/// music is the same or the two runs' partitions differ.
pub(crate) fn plan(old: &Kept, new: &Kept, change: &Change) -> Option<Plan> {
    change.changed?;
    let (op, np) = (&old.parts, &new.parts);
    if op.keys != np.keys || np.weights.is_empty() {
        return None;
    }
    let from = change
        .resume
        .map_or(0, |r| old.checkpoints[r].step as usize)
        .min(new.steps.len());
    let to = change
        .converged
        .map_or(new.steps.len(), |c| new.checkpoints[c].step as usize);
    // Each new timestep's checkpoint, and the old timestep and checkpoint
    // at its moment.
    let checkpoint_of = |s: usize| new.checkpoints.iter().position(|c| c.step as usize == s);
    let rows: Vec<(Option<usize>, Option<usize>, Option<usize>)> = (from..to)
        .map(|s| {
            let m = new.steps[s].moment;
            (checkpoint_of(s), at(old, m), checkpoint_at(old, m))
        })
        .collect();
    // The old run's timesteps the new one lacks, each with the first new
    // timestep after it.
    let lost: Vec<(usize, usize)> = (0..old.steps.len())
        .filter(|&q| at(new, old.steps[q].moment).is_none())
        .map(|q| {
            let m = old.steps[q].moment;
            (
                q,
                new.steps.partition_point(|s| s.moment.compare(m).is_lt()),
            )
        })
        .collect();
    // What a partition holds where translation resumes, its iterators made
    // on the new music: one whose music changed differs there.
    let resumed = |parts: &Parts, k: Option<usize>, p: usize| match k {
        Some(k) => parts.state(k, p),
        None => parts.start.get(p).copied(),
    };
    let new_resume = change
        .resume
        .and_then(|r| checkpoint_of(old.checkpoints[r].step as usize));
    let mut running = Vec::new();
    let mut runs = Vec::new();
    let mut work = 0u64;
    let mut reads_only = 0u64;
    for p in 1..np.width() {
        let (row, n) = part_plan(
            op,
            np,
            &rows,
            &lost,
            (from, to),
            p,
            |s| s == from && resumed(op, change.resume, p) != resumed(np, new_resume, p),
            true,
        );
        let (_, free) = part_plan(
            op,
            np,
            &rows,
            &lost,
            (from, to),
            p,
            |s| s == from && resumed(op, change.resume, p) != resumed(np, new_resume, p),
            false,
        );
        work += u64::from(n) * u64::from(np.weights[p]);
        reads_only += u64::from(free) * u64::from(np.weights[p]);
        runs.push(n);
        running.push(row);
    }
    let all: u64 = np.weights.iter().map(|&w| u64::from(w)).sum();
    let span = to.saturating_sub(from) as u64;
    work += span * u64::from(np.weights[0]);
    reads_only += span * u64::from(np.weights[0]);
    Some(Plan {
        from,
        to,
        running,
        run: PartRun {
            partitions: np.keys.len() as u32,
            merged_all: np.merged_all.clone(),
            runs,
            work,
            unpartitioned: span * all,
            whole: new.steps.len() as u64 * all,
            reads_only,
        },
    })
}

/// The timesteps from `from` to `to` partition `p` runs at (and how many,
/// its catch-up included): from where its music changed (`changed`), what
/// it heard or its ancestors' properties differ, or, with `outs`, where
/// the old run's partition wrote outside itself, to a checkpoint where its
/// state and future are the old run's.
#[allow(clippy::too_many_arguments)]
fn part_plan(
    op: &Parts,
    np: &Parts,
    rows: &[(Option<usize>, Option<usize>, Option<usize>)],
    lost: &[(usize, usize)],
    (from, to): (usize, usize),
    p: usize,
    changed: impl Fn(usize) -> bool,
    outs: bool,
) -> (Vec<bool>, u32) {
    let mut on = false;
    let mut last = from;
    let mut row = alloc::vec![false; to.saturating_sub(from)];
    let mut n = 0u32;
    for s in from..to {
        let (k, o, ko) = rows[s - from];
        if s > from
            && let Some(k) = k
        {
            if on && ko.is_some_and(|ko| op.state(ko, p) == np.state(k, p)) {
                on = false;
            }
            last = s;
        }
        // An old timestep the new run lacks before this one, at which
        // the partition heard or made something.
        let lost = lost
            .iter()
            .any(|&(q, next)| next == s && (op.input(q, p) != 0 || op.output(q, p) != 0));
        let needs = lost
            || changed(s)
            || match o {
                Some(o) => {
                    np.input(s, p) != op.input(o, p)
                        || np.shared(s, p) != op.shared(o, p)
                        || np.shared_before(s, p) != op.shared_before(o, p)
                        || (outs && op.out(o, p) != 0)
                }
                None => np.input(s, p) != 0 || np.shared(s, p) != np.shared_before(s, p),
            };
        if needs && !on {
            on = true;
            n += (s - last) as u32;
        }
        if on {
            n += 1;
            row[s - from] = true;
        }
    }
    (row, n)
}

/// What of `plan` whole runs contradict: a partition's records at a
/// timestep it replays, or its state at a checkpoint it replays, not the
/// old run's at that moment.
pub(crate) fn verify(old: &Kept, new: &Kept, plan: &Plan) -> Vec<String> {
    let (op, np) = (&old.parts, &new.parts);
    let mut wrong = Vec::new();
    for (i, row) in plan.running.iter().enumerate() {
        let p = i + 1;
        let name = &np.keys[i];
        for s in plan.from..plan.to {
            if row[s - plan.from] {
                continue;
            }
            let m = new.steps[s].moment;
            let old_output = at(old, m).map_or(0, |o| op.output(o, p));
            if np.output(s, p) != old_output {
                wrong.push(format!(
                    "partition {name}, replayed at {m:?}: its records differ"
                ));
                break;
            }
            // Its state, not its future: the music an edit changed may lie
            // ahead of a partition that has not reached it.
            let checkpoint = new.checkpoints.iter().position(|c| c.step as usize == s);
            if let (Some(k), Some(o)) = (checkpoint, checkpoint_at(old, m)) {
                let state = |parts: &Parts, k| parts.state(k, p).map(|s| s.0);
                if s > plan.from && state(np, k) != state(op, o) {
                    wrong.push(format!(
                        "partition {name}, replayed at {m:?}: its state differs"
                    ));
                    break;
                }
            }
        }
    }
    wrong
}
