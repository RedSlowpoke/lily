//! N8's checkpoints and convergence (04 §10.1, §10.2, §10.5, §10.6; MS6
//! steps 4 and 5): what a `\layout` or `\midi` translation keeps for the
//! next compile's, and what the next compile's finds changed.
//!
//! - **The music's shape** ([`Kept::shape`]): the score's music in
//!   preorder, a node's `element` before its other music (a repeat's body
//!   before its alternatives, as they are iterated), live or frozen, each
//!   node with the digest of its kind and own properties without `origin`
//!   and of its whole subtree, and the
//!   moment its iterator was made, a node inside a simple iterator's music
//!   with that iterator's. [`changed`] walks two shapes from the front and
//!   from the back: the first moment iteration can differ at, `m_e`, is the
//!   old start of the first node that differs, or of its nearest ancestor
//!   with one (§10.1); of the node itself where it has more or fewer
//!   pieces of music and is no plain sequence or simultaneous music (a
//!   repeat reads its alternatives when it starts), or reads its length
//!   when it starts and that differs (a repeat, a tuplet); of the element
//!   before it in a sequence where the changed element's grace time can
//!   differ (the sequence looks at it as the one before ends); from the
//!   start where the state after the first `Prepare` differs
//!   ([`Kept::start`]: the output definition changed).
//! - **Timesteps** ([`Step`]): each timestep's moment, a digest of the
//!   music whose iterators it made (its input: a simple iterator's whole
//!   music, another's own node), and, once the translation ends, a digest
//!   of the records made at it (its output).
//! - **Checkpoints** ([`Checkpoint`]): after the `Prepare` of each timestep
//!   that starts a bar (the Score's `measurePosition` zero), or the eighth
//!   since the last, but the first: the digest of the translation's state
//!   there. That is the context tree with each context's properties and
//!   overrides, each translator's flags, the Scheme values it holds and
//!   its Rust state (`Work::held`, D240), the moments translators asked
//!   for, and the iterators below the top with their starts, contexts,
//!   own nodes and how far a sequence is. Apart, the digest of the music
//!   they have left (its future): a simple iterator's whole music and the
//!   elements a sequence has not started. A value is digested by its
//!   contents: a grob as the moment it was made, its
//!   context and its place among the records that context made at that
//!   moment, a context by its key (`partition.rs`, D239), music by its
//!   digest, a place in the input not at all (an edit before it moves it),
//!   any other Scheme object as `write` writes it.
//! - **Partitions** ([`Kept::parts`], MS6 step 6): the same of each
//!   partition and the shared level, with the events its contexts heard,
//!   the records it made as each timestep ended and the shared properties
//!   it can read (`partition.rs`).
//!
//! A run converges (§10.5) at its first checkpoint after `m_e` past every
//! changed node of both runs whose state and future are the old run's at
//! the same moment.
//!
//! A checkpoint's state itself, the resume at one and the splice where a
//! resumed run converges (`Recording::converged`, `splice_tail`) are
//! `resume.rs`' (D241–D243); a partition is replayed only if asked (D258,
//! `replay.rs`). [`compare`] says where a run resumes, where it converges and
//! which partitions run, and [`verify_all`] checks the digests against a whole
//! run: equal before `m_e`, inputs, outputs and checkpoints equal from the
//! convergence on, and each partition's where it is replayed.
//!
//! **What a compile records (D258):** only what the next one can read: the
//! shape, the first state, each grob's digests for Y7's cells, and a
//! checkpoint's digests where a snapshot lets it be spliced at. The timesteps'
//! digests, the other checkpoints' and the partitions' rows are for
//! [`compare`], [`verify_all`] and a replay, and are made under N8's checks
//! or with a replay on (`Recording::full`). What a run does is the same either
//! way: a checkpoint with no snapshot of the old run is never spliced at.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Heap, Tag, Value};

use crate::context::{Arena, CtxId, GLOBAL};
use crate::dispatcher::StreamEvent;
use crate::music::{At, Item, Music};
use crate::override_::GrobProperties;
use crate::partition::{PartRun, Parts, Tracker};
use crate::scheme_translator::GrobHandle;
use crate::{ContextHandle, Input, Moment};

/// Timesteps after which a checkpoint is made where no bar starts (§10.2).
pub const INTERVAL: u32 = 8;

/// How many pairs, vectors' items and other values one value's digest
/// visits (a grob description's alist is long).
const BUDGET: u32 = 512;

/// A 64-bit FNV-1a digest.
#[derive(Clone, Copy)]
pub(crate) struct Digest(u64);

impl Digest {
    pub(crate) fn new(tag: u64) -> Digest {
        let mut d = Digest(0xcbf2_9ce4_8422_2325);
        d.u64(tag);
        d
    }

    pub(crate) fn u64(&mut self, x: u64) {
        self.0 = (self.0 ^ x).wrapping_mul(0x0100_0000_01b3);
    }

    pub(crate) fn str(&mut self, s: &str) {
        self.u64(s.len() as u64);
        for b in s.bytes() {
            self.u64(b.into());
        }
    }

    pub(crate) fn moment(&mut self, m: Moment) {
        for r in [m.main, m.grace] {
            self.u64(r.numerator() as u64);
            self.u64(r.denominator() as u64);
        }
    }

    pub(crate) fn get(self) -> u64 {
        self.0
    }
}

/// A node of a score's music, in preorder.
#[derive(Clone, Debug)]
pub struct ShapeNode {
    /// Of its kind and properties without `origin` and its music.
    pub own: u64,
    /// Of `own` and its music's subtrees.
    pub subtree: u64,
    /// Nodes in its subtree, itself included.
    pub size: u32,
    /// When its iterator, or the simple iterator whose music holds it, was
    /// made.
    pub start: Option<Moment>,
    /// Whether it is plain sequential or simultaneous music, whose
    /// iterator reads no more of its music than the element it is at, and
    /// whether it is a sequence, which looks at its next element's grace
    /// time as the element before it ends.
    pub plain: bool,
    pub sequence: bool,
    /// Whether its iterator reads its length when it starts (a repeat, an
    /// alternative, a tuplet), and the digest of the `length` cached on it.
    pub reads_length: bool,
    pub length: u64,
    /// Whether grace music is in its subtree.
    pub grace: bool,
}

/// A timestep of a translation.
#[derive(Clone, Debug)]
pub struct Step {
    pub moment: Moment,
    /// The music whose iterators it made.
    pub inputs: u64,
    /// The records made at it, as the translation ended.
    pub outputs: u64,
}

/// A checkpoint: after the `Prepare` of timestep `step`, at `moment`: the
/// digest of the state, and of the music its iterators have left.
#[derive(Clone, Copy, Debug)]
pub struct Checkpoint {
    pub step: u32,
    pub moment: Moment,
    pub digest: u64,
    pub future: u64,
}

/// What a translation keeps for the next compile's (§10.2), no Scheme value
/// among it, so it outlives the compile's undo (S5).
#[derive(Clone, Debug, Default)]
pub struct Kept {
    /// The state's digest after the first timestep's `Prepare`, its
    /// contexts made: what the output definition gave them.
    pub start: u64,
    pub shape: Vec<ShapeNode>,
    pub steps: Vec<Step>,
    pub checkpoints: Vec<Checkpoint>,
    /// Its partitions and the shared level, each with what it heard, made
    /// and held (`partition.rs`).
    pub parts: Parts,
    /// What layout's cells are keyed by (Y7, 05 §6.1): each grob record's
    /// digests, as the translation ended, by its number.
    pub grobs: Vec<GrobDigest>,
}

impl Kept {
    /// Its checkpoint at timestep `step`, by number (the first timestep has
    /// none: its digest is [`Kept::start`]).
    pub(crate) fn checkpoint_at(&self, step: usize) -> Option<usize> {
        self.checkpoints
            .iter()
            .position(|c| c.step as usize == step)
    }
}

/// A grob record as Y7's cells see it (05 §6.1).
#[derive(Clone, Copy, Debug, Default)]
pub struct GrobDigest {
    /// Its record, the moment it was made, its definition and the numbers of
    /// what layout's slot holds of it. Not the grobs the slot names, whose
    /// numbers a grob added before them moves: Y7's cells take those by
    /// their records.
    pub whole: u64,
    /// Its record alone: a column's rank, which an edit before it moves, is
    /// in the slot.
    pub record: u64,
    /// The stable name of the context that made it (`partition.rs`).
    pub context: u64,
    pub moment: Moment,
}

/// Where a node is: a handle's bits, or a frozen tree's and the node's
/// number (the iterators' `M`).
pub(crate) type Key = (u64, u32);

/// A handle's key.
pub(crate) fn handle_key(v: Value) -> Key {
    (v.to_bits(), u32::MAX)
}

/// A frozen node's key.
pub(crate) fn node_key(tree: Value, node: u32) -> Key {
    (tree.to_bits(), node)
}

/// Digests of grob properties' alists, each with the alist, by its bits.
pub(crate) type Alists = BTreeMap<u64, (Value, u64)>;

/// A translation's recording, in its arena while it runs.
#[derive(Clone)]
pub(crate) struct Recording {
    /// Whether it keeps all of [`Kept`], as N8's checks and a replay read it
    /// (the timesteps' digests, every checkpoint's, the partitions' rows). A
    /// next compile reads only the shape, the first state, the grobs' digests
    /// and the digests of the checkpoints it can splice at: without the
    /// checks only those are made (D258), which changes nothing it finds.
    full: bool,
    kept: Kept,
    /// Each node's place in the shape, by its key, and what the keys' bits
    /// are of, traced: an object the translation drops (music an engraver
    /// replaced) is not collected, so no other object takes its bits and is
    /// found as that node (OPEN C10).
    places: BTreeMap<Key, u32>,
    held: Vec<Value>,
    /// What the timestep being run made, and before the first timestep
    /// what the first is given.
    inputs: Digest,
    since: u32,
    measure_position: Value,
    /// Digests of the alists of grob properties, by the alist's bits, for
    /// the whole translation: an alist is pushed onto, not changed, and
    /// the recording traces each one it holds, so none is collected and
    /// its bits taken by another.
    alists: Alists,
    /// Its partitions (`partition.rs`).
    pub(crate) parts: Tracker,
    /// The grob and audio records made before the timestep being run.
    made: (usize, usize),
    /// Whether each grob type, by its name's bits, is a spanner; the
    /// spanners not ended at the last checkpoint and the records looked at
    /// by then.
    spanners: BTreeMap<u64, bool>,
    open: Vec<u32>,
    looked_at: usize,
    /// N8's splice (D243): the old run spliced in at its checkpoint by
    /// number, the timestep that checkpoint is at, and the grob records that
    /// are this run's, by number: the others are the old run's as it ended.
    tail: Option<(alloc::rc::Rc<Kept>, usize, usize, Vec<u32>)>,
    /// Where it takes its snapshots (`resume.rs`).
    pub(crate) place: crate::resume::Place,
}

// --- the music's shape -------------------------------------------------------------

/// A piece of music with its key and the object the key's bits are of.
type Piece = (Key, At, Value);

/// The music `item` holds, each piece with its key.
fn music_items(heap: &Heap, item: &Item) -> Vec<Piece> {
    let frozen = |at: At| match at {
        At::Frozen { tree, node } => (node_key(tree, node), at, tree),
        At::Live(live) => ((live.props.to_bits(), u32::MAX - 1), at, live.props),
    };
    match item {
        Item::Value(v) => {
            if let Some(at) = At::of(heap, *v) {
                return alloc::vec![(handle_key(*v), at, *v)];
            }
            let mut out = Vec::new();
            let mut p = *v;
            while let (Some(x), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                if let Some(at) = At::of(heap, x) {
                    out.push((handle_key(x), at, x));
                }
                p = rest;
            }
            out
        }
        Item::Music(at) => alloc::vec![frozen(*at)],
        Item::Musics(all) => all.iter().map(|&at| frozen(at)).collect(),
    }
}

/// Adds `at` and its music to `nodes`, unless it is there already (music
/// holding itself, or music met twice, is one node): its subtree's digest.
/// What each key's bits are of is put in `held`.
fn add(
    heap: &Heap,
    (key, at, of): Piece,
    nodes: &mut Vec<ShapeNode>,
    places: &mut BTreeMap<Key, u32>,
    held: &mut Vec<Value>,
) -> Option<u64> {
    if places.contains_key(&key) {
        return None;
    }
    held.push(of);
    let me = nodes.len();
    places.insert(key, me as u32);
    nodes.push(ShapeNode {
        own: 0,
        subtree: 0,
        size: 1,
        start: None,
        plain: false,
        sequence: false,
        reads_length: false,
        length: 0,
        grace: false,
    });
    let (own, counts, length, children) = own_digest(heap, at);
    let name = assq_name(heap, at.immutable(heap)).unwrap_or_default();
    let node = &mut nodes[me];
    node.plain = matches!(name, "SequentialMusic" | "SimultaneousMusic");
    node.sequence = name == "SequentialMusic";
    node.reads_length = ["Repeat", "Alternative", "TimeScaled"]
        .iter()
        .any(|kind| name.contains(kind));
    node.length = length;
    let mut subtree = Digest::new(own);
    subtree.u64(counts);
    for piece in children {
        if let Some(d) = add(heap, piece, nodes, places, held) {
            subtree.u64(d);
        }
    }
    let size = (nodes.len() - me) as u32;
    let grace = name == "GraceMusic" || nodes[me + 1..].iter().any(|n| n.grace);
    let node = &mut nodes[me];
    node.grace = grace;
    node.own = own;
    node.subtree = subtree.get();
    node.size = size;
    Some(node.subtree)
}

/// The digest of `at`'s kind and properties without `origin` and its
/// music, the digest of how many pieces of music each property holds, of
/// its cached `length`, and its music. The `length` cached on music that
/// holds music is its music's, and not in its own digest.
fn own_digest(heap: &Heap, at: At) -> (u64, u64, u64, Vec<Piece>) {
    let mut d = Digest::new(1);
    let mut counts = Digest::new(17);
    let mut length = None;
    d.str(assq_name(heap, at.immutable(heap)).unwrap_or_default());
    let mut children = Vec::new();
    for (key, item) in at.entries(heap) {
        let name = heap.name(key).unwrap_or_default();
        if name == "origin" {
            continue;
        }
        let music = music_items(heap, &item);
        if name == "length" && music.is_empty() {
            length = item.value();
            continue;
        }
        d.str(name);
        if music.is_empty() {
            d.u64(crate::frozen::digest(
                heap,
                item.value().unwrap_or(Value::NULL),
            ));
        } else {
            counts.str(name);
            counts.u64(music.len() as u64);
            // `element` first: a repeat's body is iterated before its
            // alternatives, which its properties hold before it, and the
            // walk from the front takes preorder for time (OPEN C15).
            if name == "element" {
                children.splice(0..0, music);
            } else {
                children.extend(music);
            }
        }
    }
    let length = length.map_or(0, |v| crate::frozen::digest(heap, v));
    if children.is_empty() {
        d.str("length");
        d.u64(length);
    }
    (d.get(), counts.get(), length, children)
}

/// The `name` of an immutable property list.
fn assq_name(heap: &Heap, mut alist: Value) -> Option<&str> {
    while let (Some(entry), Some(rest)) = (heap.car(alist), heap.cdr(alist)) {
        if heap.car(entry).and_then(|k| heap.name(k)) == Some("name") {
            return heap.name(heap.cdr(entry)?);
        }
        alist = rest;
    }
    None
}

/// The subtree digest of the music `v`.
fn music_digest(heap: &Heap, v: Value) -> u64 {
    let Some(at) = At::of(heap, v) else {
        return 0;
    };
    add(
        heap,
        (handle_key(v), at, v),
        &mut Vec::new(),
        &mut BTreeMap::new(),
        &mut Vec::new(),
    )
    .unwrap_or(0)
}

/// How a new shape lines up with the old (D241): the same, or equal
/// before the first node that differs and after the last.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Shift {
    Same,
    Around {
        first: usize,
        old_tail: usize,
        new_tail: usize,
    },
}

impl Shift {
    /// The new shape's node the old node `i` lines up with.
    pub(crate) fn new_of(&self, i: usize) -> Option<usize> {
        match *self {
            Shift::Same => Some(i),
            Shift::Around { first, .. } if i < first => Some(i),
            Shift::Around {
                old_tail, new_tail, ..
            } if i > old_tail => Some(i - old_tail + new_tail),
            Shift::Around { .. } => None,
        }
    }
}

// --- recording -----------------------------------------------------------------------

impl Recording {
    /// A recording of the translation of `music`.
    pub(crate) fn new(heap: &mut Heap, music: Value) -> Recording {
        let mut shape = Vec::new();
        let mut places = BTreeMap::new();
        let mut held = Vec::new();
        if let Some(at) = At::of(heap, music) {
            let root = (handle_key(music), at, music);
            add(heap, root, &mut shape, &mut places, &mut held);
        }
        Recording {
            full: crate::resume::checking() || crate::replay::on(),
            kept: Kept {
                start: 0,
                shape,
                steps: Vec::new(),
                checkpoints: Vec::new(),
                parts: Parts::default(),
                grobs: Vec::new(),
            },
            places,
            held,
            inputs: Digest::new(2),
            since: 0,
            measure_position: heap.intern("measurePosition"),
            alists: BTreeMap::new(),
            parts: Tracker::default(),
            made: (0, 0),
            spanners: BTreeMap::new(),
            open: Vec::new(),
            looked_at: 0,
            tail: None,
            place: crate::resume::Place::default(),
        }
    }

    /// An iterator of the music at `key` made at `now`, simple if `whole`:
    /// its node's start, and its music in the timestep's input.
    pub(crate) fn made(&mut self, heap: &Heap, key: Key, at: Option<At>, whole: bool, now: Moment) {
        match self.places.get(&key) {
            Some(&i) => {
                let node = &self.kept.shape[i as usize];
                self.inputs.u64(if whole { node.subtree } else { node.own });
                let end = i + if whole { node.size } else { 1 };
                for n in &mut self.kept.shape[i as usize..end as usize] {
                    n.start.get_or_insert(now);
                }
            }
            None if !self.full => {}
            // Music a callback made: its digest now.
            None => match at {
                Some(at) => {
                    let mut nodes = Vec::new();
                    let piece = (key, at, Value::NULL);
                    let d = add(
                        heap,
                        piece,
                        &mut nodes,
                        &mut BTreeMap::new(),
                        &mut Vec::new(),
                    );
                    let own = nodes.first().map_or(0, |n| n.own);
                    self.inputs.u64(if whole { d.unwrap_or(0) } else { own });
                }
                None => self.inputs.u64(3),
            },
        }
    }

    /// The digest of the music at `key`, its own or its subtree's, for the
    /// iterators' digests; music a callback made (`\repeat unfold`'s
    /// copies), which the shape has not got, digested now, at `at`.
    pub(crate) fn music(&self, heap: &Heap, key: Key, at: Option<At>, whole: bool) -> u64 {
        if let Some(&i) = self.places.get(&key) {
            let node = &self.kept.shape[i as usize];
            return if whole { node.subtree } else { node.own };
        }
        let Some(at) = at else {
            return 1;
        };
        let mut nodes = Vec::new();
        let piece = (key, at, Value::NULL);
        let d = add(
            heap,
            piece,
            &mut nodes,
            &mut BTreeMap::new(),
            &mut Vec::new(),
        );
        match nodes.first() {
            Some(n) if !whole => n.own,
            _ => d.unwrap_or(1),
        }
    }

    /// N8's resume (D241): `m_e` of this recording's music against the
    /// old run's `old` (`None` for the same music), and how its shape lines
    /// up with the old one; `None` where they do not (another first state
    /// is not seen here: `resume.rs` compares it).
    pub(crate) fn against(&self, old: &Kept) -> Option<(Option<Moment>, Shift)> {
        let (o, n) = (&old.shape[..], &self.kept.shape[..]);
        match differing_shapes(o, n, false) {
            None if o.len() == n.len() => Some((None, Shift::Same)),
            None => None,
            Some((first, last, at)) if first[0] == first[1] => Some((
                Some(moment_of_change(o, first[0], at)),
                Shift::Around {
                    first: first[0],
                    old_tail: last[0],
                    new_tail: last[1],
                },
            )),
            Some(_) => None,
        }
    }

    /// The new music of the old node at `key` (D241): its object, its node
    /// in a frozen tree (`u32::MAX` for a handle) and whether its subtree
    /// differs; `None` for a node the old shape has not got or the new
    /// music lacks.
    pub(crate) fn new_music(
        &self,
        (new, keys): (&Recording, &[Key]),
        shift: &Shift,
        key: Key,
    ) -> Option<(Value, u32, bool)> {
        let i = *self.places.get(&key)? as usize;
        let j = shift.new_of(i)?;
        let (old, node) = (self.kept.shape.get(i)?, new.kept.shape.get(j)?);
        let new_key = *keys.get(j)?;
        if new_key.1 == u32::MAX - 1 {
            return None;
        }
        Some((new.held[j], new_key.1, old.subtree != node.subtree))
    }

    /// Its shape's keys, by node.
    pub(crate) fn keys(&self) -> Vec<Key> {
        let mut keys = alloc::vec![(0, 0); self.kept.shape.len()];
        for (&k, &i) in &self.places {
            if let Some(slot) = keys.get_mut(i as usize) {
                *slot = k;
            }
        }
        keys
    }

    /// What it keeps as it is now: the old run's, for a resume.
    pub(crate) fn kept(&self) -> &Kept {
        &self.kept
    }

    /// Takes `new`'s shape, the music the translation goes on with (D241),
    /// each node's start the old node's it lines up with where that one
    /// started by `at`.
    pub(crate) fn take_shape(&mut self, mut new: Recording, shift: &Shift, at: Moment) {
        for (i, node) in self.kept.shape.iter().enumerate() {
            if let (Some(s), Some(j)) = (node.start, shift.new_of(i))
                && s.compare(at).is_le()
                && let Some(n) = new.kept.shape.get_mut(j)
            {
                n.start = Some(s);
            }
        }
        self.kept.shape = core::mem::take(&mut new.kept.shape);
        self.places = new.places;
        self.held = new.held;
        // The checkpoints before the snapshot are the old run's, whose
        // futures are of the old music ahead: unknown (0) for this one, so
        // that no later compile converges at one, nor splices from before
        // this run's own (MS6 step 7: T24's `pitch-1-5` after a run resumed
        // from `error-1-4`'s).
        for c in &mut self.kept.checkpoints {
            c.future = 0;
        }
        self.parts.forget_futures();
    }

    /// N8's replay (MS6 step 7b): the moment of timestep `step`.
    pub(crate) fn moment_of(&self, step: usize) -> Option<Moment> {
        self.kept.steps.get(step).map(|s| s.moment)
    }

    /// N8's replay: itself with the timestep being run ended, as `finish`
    /// ends it, for the check of that timestep.
    pub(crate) fn last_ended(&self, heap: &Heap, a: &Arena) -> Recording {
        let mut r = self.clone();
        if !r.kept.steps.is_empty() {
            r.end_step(heap, a, false);
        }
        r
    }

    /// N8's replay: the checkpoints from the `from`th on unknown (their
    /// digests and futures 0, their partitions' too): a replayed part's
    /// translators' state was not kept there.
    pub(crate) fn forget_from(&mut self, from: usize) {
        for c in self.kept.checkpoints.iter_mut().skip(from) {
            c.digest = 0;
            c.future = 0;
        }
        self.parts.forget_from(from);
    }

    /// The checkpoints taken before timestep `step`.
    pub(crate) fn checkpoints_before(&self, step: usize) -> usize {
        self.kept
            .checkpoints
            .iter()
            .take_while(|c| (c.step as usize) < step)
            .count()
    }

    /// The timesteps begun so far.
    pub(crate) fn steps(&self) -> usize {
        self.kept.steps.len()
    }

    /// N8's splice (D243): whether the checkpoint just taken converges with
    /// the old run `old` (§10.5, as [`compare`] finds it): past `m_e` and
    /// every changed node of both runs, its state and future the old run's
    /// at its moment. The old run's checkpoint there, by number.
    pub(crate) fn converged(&self, old: &Kept) -> Option<usize> {
        let new = &self.kept;
        let c = new.checkpoints.last()?;
        if c.step as usize + 1 != new.steps.len() || c.digest == 0 {
            return None;
        }
        let same =
            |o: &Checkpoint| o.moment == c.moment && o.digest == c.digest && o.future == c.future;
        let Some((first, last, _)) = differing(old, new) else {
            return old.checkpoints.iter().position(same);
        };
        let m_e = changed(old, new)?;
        let passed = |shape: &[ShapeNode], from: usize, to: usize| {
            (from..=to).all(|i| {
                shape
                    .get(i)
                    .is_some_and(|n| n.start.is_some_and(|s| s.compare(c.moment).is_lt()))
            })
        };
        if !(c.moment.compare(m_e).is_gt()
            && passed(&new.shape, first[1], last[1])
            && passed(&old.shape, first[0], last[0]))
        {
            return None;
        }
        old.checkpoints.iter().position(same)
    }

    /// N8's splice (D243): the digests of a grob record, an audio record and
    /// a value of its translation's arena `a` as N8 takes them (a context
    /// by its key, a record by its place), the same for another run's in
    /// the same place with the same contents; `alists` a cache to share
    /// between calls with nothing collected meanwhile.
    pub(crate) fn grob_record(
        &self,
        heap: &Heap,
        a: &Arena,
        g: &crate::records::GrobRecord,
        alists: &mut Alists,
    ) -> u64 {
        grob_digest(heap, a, &self.parts, g, alists)
    }

    pub(crate) fn audio_record(
        &self,
        heap: &Heap,
        a: &Arena,
        r: &crate::records::AudioRecord,
        alists: &mut Alists,
    ) -> u64 {
        audio_digest(heap, a, &self.parts, r, alists)
    }

    /// The place of the grob record `id` of `a`, its translation's (MS6
    /// step 7b: a grob's handle is its place's).
    pub(crate) fn grob_place(&self, a: &Arena, id: u32) -> u64 {
        grob_place(a, &self.parts, id)
    }

    pub(crate) fn value(&self, heap: &Heap, a: &Arena, v: Value, alists: &mut Alists) -> u64 {
        value(heap, a, &self.parts, v, alists)
    }

    /// N8's splice (D243): the old run `old` from its checkpoint
    /// `checkpoint` on, the timestep being run, where this run converged
    /// with it, its own: [`Recording::finish`] keeps its timesteps and
    /// checkpoints from there, and the digests of the grob records but
    /// `own`, this run's.
    pub(crate) fn splice_tail(
        &mut self,
        old: alloc::rc::Rc<Kept>,
        checkpoint: usize,
        own: Vec<u32>,
    ) {
        let step = old.checkpoints[checkpoint].step as usize;
        // The music it did not iterate, the old run did: its nodes' starts
        // the old ones', lined up as a resume lines them up.
        if let Some((_, shift)) = self.against(&old) {
            for (i, node) in old.shape.iter().enumerate() {
                if let (Some(s), Some(j)) = (node.start, shift.new_of(i))
                    && let Some(n) = self.kept.shape.get_mut(j)
                    && n.start.is_none()
                {
                    n.start = Some(s);
                }
            }
        }
        self.tail = Some((old, step, checkpoint, own));
    }

    /// Timestep `w` begins: what was made since the last began is that
    /// one's input, and what was made before the first is the first's.
    /// The last one ends.
    pub(crate) fn step(&mut self, heap: &Heap, a: &Arena, w: Moment) {
        if !self.kept.steps.is_empty() {
            self.end_step(heap, a, true);
        }
        let inputs = core::mem::replace(&mut self.inputs, Digest::new(2)).get();
        let first = match self.kept.steps.last_mut() {
            None => inputs,
            Some(last) => {
                last.inputs ^= inputs;
                0
            }
        };
        self.kept.steps.push(Step {
            moment: w,
            inputs: first,
            outputs: 0,
        });
    }

    /// Whether it keeps all of [`Kept`].
    pub(crate) fn full(&self) -> bool {
        self.full
    }

    /// The contexts made since it last looked seen; with the partitions'
    /// merges, which only a full recording keeps.
    pub(crate) fn see(&mut self, heap: &Heap, a: &Arena) {
        if self.full {
            self.parts.see(heap, a);
        } else {
            self.parts.see_new(heap, a);
        }
    }

    /// The timestep being run ends: the contexts made in it seen, and the
    /// shared level's properties it set or unset digested as they are now.
    fn end_step(&mut self, heap: &Heap, a: &Arena, take: bool) {
        self.see(heap, a);
        if !self.full {
            a.ended.borrow_mut().clear();
            return;
        }
        let touched = if take {
            core::mem::take(&mut *a.touched.borrow_mut())
        } else {
            a.touched.borrow().clone()
        };
        // Each write to the shared level as it was made, in order: a
        // property a partition sets and unsets in the timestep is read by
        // the others between (`forbidBreak`, OPEN C12).
        for &(c, sym, now) in &touched {
            if self.parts.is_shared(c) {
                let mut d = Digest::new(38);
                d.str(heap.name(sym).unwrap_or_default());
                d.u64(value(heap, a, &self.parts, now, &mut self.alists));
                self.parts.shared_write(c, d.get());
            }
        }
        let mut touched: Vec<(CtxId, Value)> =
            touched.into_iter().map(|(c, s, _)| (c, s)).collect();
        touched.sort_by_key(|&(c, sym)| (c, sym.to_bits()));
        touched.dedup();
        for (c, sym) in touched {
            if !self.parts.is_shared(c) {
                continue;
            }
            let d = a.here(c, sym).map_or(0, |v| {
                let mut d = Digest::new(34);
                d.str(heap.name(sym).unwrap_or_default());
                d.u64(value(heap, a, &self.parts, v, &mut self.alists));
                d.get()
            });
            self.parts.shared_property(c, sym, d);
        }
        // The grobs its translators wrote that their own contexts did not
        // make, by place (MS6 step 7a).
        let wrote = if take {
            core::mem::take(&mut *a.wrote.borrow_mut())
        } else {
            a.wrote.borrow().clone()
        };
        let of = |i: u32| a.records.grobs.get(i as usize).map(|g| g.origin.0);
        for (writer, id, added) in wrote {
            let target = id.and_then(of);
            if target == Some(writer) {
                continue;
            }
            let place = id.map_or(u64::MAX, |i| grob_place(a, &self.parts, i));
            self.parts.wrote(writer, target, place, added.and_then(of));
        }
        // What it made, as it ends, by the context that made it: the
        // records made at it, and the spanners it ended. A later timestep
        // may change them; that is its own output.
        let mut made = Vec::new();
        let (alists, keys) = (&mut self.alists, &self.parts);
        let (grobs, audio) = (&a.records.grobs, &a.records.audio);
        let mut ended = if take {
            core::mem::take(&mut *a.ended.borrow_mut())
        } else {
            a.ended.borrow().clone()
        };
        ended.sort_unstable();
        ended.dedup();
        for &i in ended.iter().filter(|&&i| (i as usize) < self.made.0) {
            let g = &grobs[i as usize];
            made.push((g.origin.0, grob_place(a, keys, i) ^ 0x5bd1_e995));
        }
        for g in grobs.iter_from(self.made.0) {
            made.push((g.origin.0, grob_digest(heap, a, keys, g, alists)));
        }
        for r in audio.iter_from(self.made.1) {
            made.push((r.origin.0, audio_digest(heap, a, keys, r, alists)));
        }
        self.made = (grobs.len(), audio.len());
        self.parts.end_step(made);
    }

    /// The contexts `targets` hear the event `ev`; a Scheme listener among
    /// them merges every partition.
    pub(crate) fn heard(
        &mut self,
        heap: &Heap,
        a: &Arena,
        ev: Value,
        digest: &mut Option<u64>,
        targets: &[crate::dispatcher::Listener],
    ) {
        use crate::dispatcher::Listener;
        for l in targets {
            let c = match *l {
                Listener::Context(c, _) | Listener::Translator(c, _, _) => c,
                Listener::Combine(n, _) => {
                    match a.combines.get(n as usize).and_then(|c| c.lyrics) {
                        Some(c) => c,
                        None => continue,
                    }
                }
                Listener::Scheme(_) => {
                    self.parts.merge_all("a Scheme listener");
                    continue;
                }
                Listener::Dispatch(_) => continue,
            };
            let d =
                *digest.get_or_insert_with(|| value(heap, a, &self.parts, ev, &mut self.alists));
            self.parts.heard(c, d);
        }
    }

    /// Traces the alists whose digests it keeps, and the shape's music.
    pub(crate) fn trace(&self, tracer: &mut lily_engine_scheme::Tracer<'_>) {
        for &(alist, _) in self.alists.values() {
            tracer.trace(alist);
        }
        for &v in &self.held {
            tracer.trace(v);
        }
    }

    /// Whether the timestep being run, its `Prepare` sent, gets a
    /// checkpoint: one that starts a bar, or the [`INTERVAL`]th since the
    /// last; the first's state is [`Kept::start`].
    pub(crate) fn wants_checkpoint(&mut self, heap: &Heap, a: &Arena) -> bool {
        match self.kept.steps.len() {
            0 => return false,
            1 => return true,
            _ => {}
        }
        self.since += 1;
        let bar = a.children(GLOBAL).first().is_some_and(|&score| {
            let pos = a.get(score, self.measure_position);
            heap.foreign::<Moment>(pos)
                .is_some_and(|m| m.main.is_zero() && m.grace.is_zero())
        });
        if bar || self.since >= INTERVAL {
            self.since = 0;
            return true;
        }
        false
    }

    /// The checkpoint of the timestep being run, or the first's
    /// [`Kept::start`]; the iterators' digests are given, and each
    /// iterator's by its context (`u32::MAX` for none).
    pub(crate) fn checkpoint(
        &mut self,
        heap: &Heap,
        a: &Arena,
        (iterators, future): (u64, u64),
        each: &[(CtxId, u64, u64)],
        skip: &dyn Fn(u64) -> bool,
    ) {
        let step = self.kept.steps.len() as u32 - 1;
        let moment = a.now();
        self.see(heap, a);
        let (digest, mut contexts) = self.state(heap, a, iterators, skip);
        if step == 0 {
            self.kept.start = digest;
        } else {
            self.kept.checkpoints.push(Checkpoint {
                step,
                moment,
                digest,
                future,
            });
        }
        if !self.full {
            return;
        }
        // Each part's: its contexts' and their iterators'; the shared
        // level's own the moment, the moments asked for and the iterators
        // of no context.
        let mut own = Digest::new(35);
        own.moment(moment);
        for &m in a.extra_moments() {
            own.moment(m);
        }
        let mut own_future = 0u64;
        for &(c, state, future) in each {
            match contexts.iter_mut().find(|x| x.0 == c) {
                Some(x) => {
                    x.1 = x.1.wrapping_add(state.wrapping_mul(3));
                    x.2 = x.2.wrapping_add(future);
                }
                None => {
                    own.u64(state);
                    own_future = own_future.wrapping_add(future);
                }
            }
        }
        self.parts.checkpoint(contexts, own.get(), own_future);
    }

    /// The checkpoint of the timestep being run, its digests not taken: no
    /// next compile can splice there.
    pub(crate) fn pass_checkpoint(&mut self, a: &Arena) {
        self.kept.checkpoints.push(Checkpoint {
            step: self.kept.steps.len() as u32 - 1,
            moment: a.now(),
            digest: 0,
            future: 0,
        });
    }

    /// The spanners each context made and has not ended, by their places:
    /// what its engravers hold of them is a record number in Rust, which
    /// the state's Scheme values do not show (a beam begun at another
    /// moment ends with the same state otherwise).
    fn open_spanners(&mut self, heap: &Heap, a: &Arena) -> BTreeMap<CtxId, u64> {
        let mut open: BTreeMap<CtxId, u64> = BTreeMap::new();
        let grobs = &a.records.grobs;
        for (i, g) in grobs.iter().enumerate().skip(self.looked_at) {
            if g.ended.is_some() {
                continue;
            }
            let spanner = *self.spanners.entry(g.name.to_bits()).or_insert_with(|| {
                let meta =
                    crate::music::assq_named(heap, "meta", g.definition).and_then(|m| heap.cdr(m));
                let class = meta
                    .and_then(|m| crate::music::assq_named(heap, "class", m))
                    .and_then(|c| heap.cdr(c));
                class.and_then(|c| heap.name(c)) == Some("Spanner")
            });
            if spanner {
                self.open.push(i as u32);
            }
        }
        self.looked_at = grobs.len();
        self.open.retain(|&i| grobs[i as usize].ended.is_none());
        for &i in &self.open {
            let place = grob_place(a, &self.parts, i);
            let sum = open.entry(grobs[i as usize].origin.0).or_insert(0);
            *sum = sum.wrapping_add(place);
        }
        open
    }

    /// The digest of the translation's state, and each context's, but those
    /// whose keys `skip` gives (a replayed part's: its translators hold
    /// nothing of the run).
    fn state(
        &mut self,
        heap: &Heap,
        a: &Arena,
        iterators: u64,
        skip: &dyn Fn(u64) -> bool,
    ) -> (u64, Vec<(CtxId, u64, u64)>) {
        let moment = a.now();
        let mut d = Digest::new(4);
        d.moment(moment);
        d.u64(iterators);
        for &m in a.extra_moments() {
            d.moment(m);
        }
        let open = self.open_spanners(heap, a);
        let (alists, keys) = (&mut self.alists, &self.parts);
        let mut each = Vec::new();
        for c in 0..a.context_count() as CtxId {
            if a.removed(c) {
                continue;
            }
            if skip(keys.key(c)) {
                each.push((c, 0, 0));
                continue;
            }
            let mut dc = Digest::new(36);
            dc.u64(keys.key(c));
            dc.u64(open.get(&c).copied().unwrap_or(0));
            dc.u64(a.parent(c).map_or(u64::MAX, |p| keys.key(p)));
            for &child in a.children(c) {
                dc.u64(keys.key(child));
            }
            dc.str(heap.name(a.name(heap, c)).unwrap_or_default());
            dc.str(a.id(c));
            // Properties in no order: a sum of each one's digest.
            let mut props = 0u64;
            for (sym, v) in a.properties(c) {
                let mut p = Digest::new(5);
                p.str(heap.name(sym).unwrap_or_default());
                p.u64(value(heap, a, keys, v, alists));
                props = props.wrapping_add(p.get());
            }
            dc.u64(props);
            if let Some(g) = a.group(c) {
                let mut values = Vec::new();
                let mut held = Sink {
                    a,
                    keys,
                    d: Digest::new(40),
                };
                for flags in g.state(&mut values, &mut held) {
                    dc.u64(flags);
                }
                dc.u64(held.d.get());
                for v in values {
                    dc.u64(value(heap, a, keys, v, alists));
                }
            }
            d.u64(dc.get());
            each.push((c, dc.get(), 0));
        }
        (d.get(), each)
    }

    /// What the translation keeps, its records' digests by timestep added.
    pub(crate) fn finish(mut self, heap: &Heap, a: &Arena) -> Kept {
        let tail = self.tail.take();
        match &tail {
            // Spliced (D243): from the timestep it converged at, the old
            // run's timesteps and checkpoints, the old run's records made
            // there theirs.
            Some((old, step, checkpoint, _)) => {
                self.kept.steps.pop();
                self.kept
                    .steps
                    .extend(old.steps.get(*step..).unwrap_or_default().iter().cloned());
                self.kept.checkpoints.extend(
                    old.checkpoints
                        .get(checkpoint + 1..)
                        .unwrap_or_default()
                        .iter()
                        .copied(),
                );
            }
            None => {
                // The arena's writes read, not taken: a copy of the
                // recording is finished for N8's checks too.
                if !self.kept.steps.is_empty() {
                    self.end_step(heap, a, false);
                }
                let last = core::mem::replace(&mut self.inputs, Digest::new(2)).get();
                if let Some(step) = self.kept.steps.last_mut() {
                    step.inputs ^= last;
                }
            }
        }
        if !self.full {
            // A splice's grobs are the old run's as it ended, but its own:
            // their digests the old run's (the slot's values by what they
            // hold, the same in both).
            let own = match &tail {
                Some((old, _, _, own)) if old.grobs.len() == a.records.grobs.len() => {
                    self.kept.grobs = old.grobs.clone();
                    own.clone()
                }
                _ => (0..a.records.grobs.len() as u32).collect(),
            };
            self.kept
                .grobs
                .resize(a.records.grobs.len(), GrobDigest::default());
            let mut definitions = BTreeMap::new();
            for i in own {
                let (_, grob) = self.grob_kept(heap, a, i as usize, &mut definitions);
                self.kept.grobs[i as usize] = grob;
            }
            return self.kept;
        }
        let mut outputs: BTreeMap<usize, Digest> = BTreeMap::new();
        let mut definitions: BTreeMap<u64, u64> = BTreeMap::new();
        let mut grobs = Vec::with_capacity(a.records.grobs.len());
        for i in 0..a.records.grobs.len() {
            let (d, grob) = self.grob_kept(heap, a, i, &mut definitions);
            let step = self.step_at(grob.moment);
            outputs.entry(step).or_insert(Digest::new(7)).u64(d);
            grobs.push(grob);
        }
        self.kept.grobs = grobs;
        for r in a.records.audio.iter() {
            let d = audio_digest(heap, a, &self.parts, r, &mut self.alists);
            let step = self.step_at(r.moment);
            outputs.entry(step).or_insert(Digest::new(7)).u64(d);
        }
        for (i, o) in outputs {
            if let Some(step) = self.kept.steps.get_mut(i) {
                step.outputs = o.get();
            }
        }
        let mut parts = self.parts.finish(a);
        if let Some((old, step, checkpoint, _)) = &tail {
            parts = parts.spliced(&old.parts, *step, checkpoint + 1);
        }
        self.kept.parts = parts;
        self.kept
    }

    /// The timestep a record made at `m` was made at.
    fn step_at(&self, m: Moment) -> usize {
        self.kept
            .steps
            .partition_point(|s| s.moment.compare(m).is_lt())
    }

    /// The digest of grob record `i` and what Y7's cells keep of it.
    fn grob_kept(
        &mut self,
        heap: &Heap,
        a: &Arena,
        i: usize,
        definitions: &mut BTreeMap<u64, u64>,
    ) -> (u64, GrobDigest) {
        let (alists, keys) = (&mut self.alists, &self.parts);
        let g = &a.records.grobs[i];
        let d = grob_digest(heap, a, keys, g, alists);
        let definition = match definitions.get(&g.definition.to_bits()) {
            Some(&h) => h,
            None => {
                let h = value(heap, a, keys, g.definition, alists);
                definitions.insert(g.definition.to_bits(), h);
                h
            }
        };
        let mut whole = Digest::new(61);
        whole.u64(d);
        whole.moment(g.moment);
        whole.u64(definition);
        if let Some(slot) = &a.grob_slot {
            whole.u64(slot.grob_digest(i, &mut |_| 0));
        }
        let grob = GrobDigest {
            whole: whole.get(),
            record: d,
            context: keys.key(g.origin.0),
            moment: g.moment,
        };
        (d, grob)
    }
}

/// What the translators hold in Rust, digested (D240): a record by its
/// place, a context by its key.
struct Sink<'a> {
    a: &'a Arena,
    keys: &'a Tracker,
    d: Digest,
}

impl crate::translator::Held for Sink<'_> {
    fn num(&mut self, x: i64) {
        self.d.u64(x as u64);
    }

    fn moment(&mut self, m: Moment) {
        self.d.moment(m);
    }

    fn record(&mut self, id: u32) {
        // A `\layout` translation makes grobs, a `\midi` one audio.
        self.d.u64(if self.a.records.grobs.is_empty() {
            audio_place(self.a, self.keys, id)
        } else {
            grob_place(self.a, self.keys, id)
        });
    }

    fn context(&mut self, c: CtxId) {
        self.d.u64(self.keys.key(c));
    }
}

/// The digest of a grob record.
fn grob_digest(
    heap: &Heap,
    a: &Arena,
    keys: &Tracker,
    g: &crate::records::GrobRecord,
    alists: &mut Alists,
) -> u64 {
    let mut d = Digest::new(6);
    d.str(heap.name(g.name).unwrap_or_default());
    d.u64(keys.key(g.origin.0));
    d.u64(g.origin.1.into());
    // By name: a translation numbers its interfaces as it first meets them,
    // so the same grob's numbers differ in a run that met another first
    // (OPEN C15). Summed, as the set has no order but the numbers'.
    let mut interfaces = 0u64;
    for n in 0..=255u8 {
        if g.interfaces.contains(n) {
            let mut name = Digest::new(7);
            name.str(
                heap.name(a.records.interfaces.symbol(n))
                    .unwrap_or_default(),
            );
            interfaces = interfaces.wrapping_add(name.get());
        }
    }
    d.u64(interfaces);
    d.u64(cause(heap, a, keys, g.cause, alists));
    if let Some(m) = g.ended {
        d.moment(m);
    }
    d.u64(value(heap, a, keys, g.props, alists));
    d.get()
}

/// The digest of an audio record.
fn audio_digest(
    heap: &Heap,
    a: &Arena,
    keys: &Tracker,
    r: &crate::records::AudioRecord,
    alists: &mut Alists,
) -> u64 {
    let mut d = Digest::new(8);
    d.u64(cause(heap, a, keys, r.cause, alists));
    audio(a, keys, &r.audio, &mut d);
    d.str(heap.name(r.name).unwrap_or_default());
    d.u64(keys.key(r.origin.0));
    d.u64(r.origin.1.into());
    d.u64(value(heap, a, keys, r.value, alists));
    // Its column, a number of the performance's, is its moment's.
    d.u64(r.channel.into());
    d.get()
}

/// Digests an audio record's element: the records a note or a staff names
/// by their places ([`audio_place`]).
fn audio(a: &Arena, keys: &Tracker, element: &crate::audio::Audio, d: &mut Digest) {
    use crate::audio::Audio;
    let mut back = |n: Option<u32>| d.u64(n.map_or(u64::MAX, |n| audio_place(a, keys, n)));
    match element {
        Audio::Note(n) => {
            back(n.dynamic);
            back(n.tied);
            d.u64(1);
            d.u64(n.tones.numerator() as u64);
            d.u64(n.tones.denominator() as u64);
            d.moment(n.length);
            d.u64(n.extra_velocity as u64);
            d.u64(n.tie_event.into());
        }
        Audio::Staff(s) => {
            for &item in &s.items {
                back(Some(item));
            }
            d.u64(2);
            d.moment(s.end);
            d.u64(
                u64::from(s.percussion)
                    | u64::from(s.merge_unisons) << 1
                    | u64::from(s.control) << 2,
            );
            d.str(&s.track_name);
        }
        other => d.str(&format!("{other:?}")),
    }
}

/// The digest of a record's cause.
fn cause(
    heap: &Heap,
    a: &Arena,
    keys: &Tracker,
    cause: crate::records::Cause,
    alists: &mut Alists,
) -> u64 {
    match cause {
        crate::records::Cause::None => 0,
        crate::records::Cause::Event(ev) => value(heap, a, keys, ev, alists),
        crate::records::Cause::Grob(n) => grob_place(a, keys, n),
        crate::records::Cause::Audio(n) => audio_place(a, keys, n),
    }
}

/// An audio record by the moment it was made, its context and its place
/// among those its context made at that moment, as [`grob_place`].
fn audio_place(a: &Arena, keys: &Tracker, id: u32) -> u64 {
    let audio = &a.records.audio;
    let Some(r) = audio.get(id as usize) else {
        return u64::MAX;
    };
    let first = audio.partition_point(id as usize, |o| o.moment.compare(r.moment).is_lt());
    let before = audio
        .iter_from(first)
        .take(id as usize - first)
        .filter(|o| o.origin.0 == r.origin.0)
        .count();
    let mut d = Digest::new(22);
    d.moment(r.moment);
    d.u64(keys.key(r.origin.0));
    d.u64(before as u64);
    d.get()
}

/// A grob by the moment it was made, its context and its place among those
/// its context made at that moment, which an edit before it, or in
/// another partition, does not change.
fn grob_place(a: &Arena, keys: &Tracker, id: u32) -> u64 {
    let grobs = &a.records.grobs;
    let Some(g) = grobs.get(id as usize) else {
        return u64::MAX;
    };
    let mut d = Digest::new(9);
    d.moment(g.moment);
    d.u64(keys.key(g.origin.0));
    d.u64(a.grob_rank(id).into());
    d.get()
}

/// The digest of a value of the translation's state, a context by its key
/// (`partition.rs`).
fn value(heap: &Heap, a: &Arena, keys: &Tracker, v: Value, alists: &mut Alists) -> u64 {
    let mut budget = BUDGET;
    digest_value(heap, a, keys, v, alists, &mut budget)
}

fn digest_value(
    heap: &Heap,
    a: &Arena,
    keys: &Tracker,
    v: Value,
    alists: &mut Alists,
    budget: &mut u32,
) -> u64 {
    if *budget == 0 {
        return 10;
    }
    *budget -= 1;
    let mut d = Digest::new(11);
    match v.tag() {
        Tag::Pair => {
            let (car, cdr) = (heap.car(v).unwrap_or(v), heap.cdr(v).unwrap_or(v));
            d.u64(digest_value(heap, a, keys, car, alists, budget));
            d.u64(digest_value(heap, a, keys, cdr, alists, budget));
        }
        Tag::Vector => {
            for &x in heap.vector_items(v).unwrap_or(&[]) {
                d.u64(digest_value(heap, a, keys, x, alists, budget));
            }
        }
        Tag::Symbol => d.str(heap.name(v).unwrap_or_default()),
        // Not written out, which is what the state's digest would spend most
        // on (OPEN C11).
        Tag::Flonum if heap.as_flonum(v).is_some() => {
            d.u64(heap.as_flonum(v).unwrap_or_default().to_bits());
        }
        Tag::String if heap.str(v).is_some() => {
            d.str(heap.str(v).map_or("", |s| s.as_str()));
        }
        // A value its bits hold, not written out.
        Tag::Fixnum
        | Tag::Null
        | Tag::False
        | Tag::True
        | Tag::Unspecified
        | Tag::Undefined
        | Tag::Eof
        | Tag::Char => d.u64(v.to_bits()),
        Tag::Closure if heap.closure_parts(v).is_some() => {
            let (code, free) = heap.closure_parts(v).unwrap_or_default();
            d.u64(code);
            for &x in free {
                d.u64(digest_value(heap, a, keys, x, alists, budget));
            }
        }
        _ => {
            if heap.foreign::<Input>(v).is_some() {
                // A place in the input, which an edit before it moves: the
                // splice moves it (02 §5.7's takeover moves messages).
                d.u64(18);
            } else if let Some(&m) = heap.foreign::<Moment>(v) {
                d.u64(17);
                d.moment(m);
            } else if let Some(g) = heap.foreign::<GrobHandle>(v) {
                d.u64(12);
                d.u64(grob_place(a, keys, g.id));
            } else if let Some(c) = heap.foreign::<ContextHandle>(v) {
                d.u64(13);
                d.u64(keys.key(c.id));
            } else if heap.foreign::<Music>(v).is_some() {
                d.u64(14);
                d.u64(music_digest(heap, v));
            } else if let Some(gp) = heap.foreign::<GrobProperties>(v) {
                d.u64(15);
                let alist = gp.alist();
                let h = match alists.get(&alist.to_bits()) {
                    Some(&(_, h)) => h,
                    None => {
                        let mut fresh = BUDGET;
                        let h = digest_value(heap, a, keys, alist, alists, &mut fresh);
                        alists.insert(alist.to_bits(), (alist, h));
                        h
                    }
                };
                d.u64(h);
            } else if let Some(u) = heap.foreign::<crate::unpure::UnpurePure>(v) {
                d.u64(19);
                for p in u.parts() {
                    d.u64(digest_value(heap, a, keys, p, alists, budget));
                }
            } else if let Some(ev) = heap.foreign::<StreamEvent>(v) {
                d.u64(16);
                let (props, music) = (ev.props, ev.music);
                d.u64(digest_value(heap, a, keys, props, alists, budget));
                d.u64(digest_value(heap, a, keys, music, alists, budget));
            } else {
                let mut text = String::new();
                heap.write(v, &mut text);
                d.str(&text);
                // A `\score` in a markup: the parse's own, as in the shape
                // (`frozen.rs`).
                if heap.foreign::<crate::book::Score>(v).is_some() {
                    d.u64(v.to_bits());
                }
            }
        }
    }
    d.get()
}

// --- comparing -------------------------------------------------------------------------

/// What a compile's translation finds against the last compile's (§10.1,
/// §10.5, §10.6).
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    /// `m_e`: the first moment iteration can differ at; `None` if the music
    /// is the same.
    pub changed: Option<Moment>,
    /// The last checkpoint of the old run before `m_e`, which translation
    /// resumes from: its number there. `None` for the start.
    pub resume: Option<usize>,
    /// The first checkpoint of the new run past every changed node whose
    /// state is the old run's at the same moment: its number there. `None`
    /// if none converges.
    pub converged: Option<usize>,
    /// The new run's timesteps, and those from the resumed checkpoint to
    /// the convergence, which run again.
    pub steps: u32,
    pub run: u32,
    /// The change list for layout, with the score as one partition: the
    /// moments from the resumed checkpoint to the convergence of the new
    /// run's timesteps whose output differs from the old run's at that
    /// moment, and of the old run's the new one has not.
    pub columns: Vec<Moment>,
    /// What runs with partitions (`partition.rs`); `None` where the music
    /// is the same or the two runs' partitions differ.
    pub parts: Option<PartRun>,
}

/// Where two runs' music differs: the first node that differs (the
/// deepest of the first differing path) and the last, in each, and the old
/// node whose start is `m_e`. Runs whose first state differs (the output
/// definition did) differ at the top.
fn differing(old: &Kept, new: &Kept) -> Option<([usize; 2], [usize; 2], usize)> {
    differing_shapes(&old.shape, &new.shape, old.start != new.start)
}

/// [`differing`] of two shapes, `start` whether their first states differ.
fn differing_shapes(
    old: &[ShapeNode],
    new: &[ShapeNode],
    start: bool,
) -> Option<([usize; 2], [usize; 2], usize)> {
    let (o0, n0) = (old.first()?, new.first()?);
    if start {
        return Some(([0, 0], [old.len() - 1, new.len() - 1], 0));
    }
    if o0.subtree == n0.subtree {
        return None;
    }
    // From the front: into the first child whose subtree differs, of a
    // node equal itself; where one has a child the other lacks, the old
    // one's, or the old node's last after an append.
    let children = |shape: &[ShapeNode], i: usize| {
        let end = i + shape[i].size as usize;
        let mut out = Vec::new();
        let mut c = i + 1;
        while c < end {
            out.push(c);
            c += shape[c].size as usize;
        }
        out
    };
    let (mut i, mut j) = (0usize, 0usize);
    // The path's sequences, the old child it went into in each, and
    // whether that child's grace time can differ: elements added or
    // removed, or grace music in it.
    let mut path: Vec<(usize, usize, bool)> = Vec::new();
    let first = loop {
        if old[i].own != new[j].own {
            break [i, j];
        }
        let (xs, ys) = (children(old, i), children(new, j));
        // An iterator that reads all its music when it starts (a repeat,
        // its alternatives) differs from its start when they are not as
        // many, or its length differs (a tuplet's span too).
        if (xs.len() != ys.len() && !old[i].plain)
            || (old[i].reads_length && old[i].length != new[j].length)
        {
            break [i, j];
        }
        let differs = xs
            .iter()
            .zip(&ys)
            .position(|(&x, &y)| old[x].subtree != new[y].subtree);
        // Elements added or removed: the change is where the two lists
        // part, before the elements that only moved.
        if let (Some(mut k), true) = (differs, xs.len() != ys.len()) {
            let added = ys.len() > xs.len();
            while k > 0
                && if added {
                    old[xs[k - 1]].subtree == new[ys[k]].subtree
                } else {
                    old[xs[k]].subtree == new[ys[k - 1]].subtree
                }
            {
                k -= 1;
            }
            let at = [xs[k.min(xs.len() - 1)], ys[k.min(ys.len() - 1)]];
            if old[i].sequence {
                path.push((i, at[0], true));
            }
            break at;
        }
        let next = match differs {
            Some(k) => [xs[k], ys[k]],
            None if xs.len() > ys.len() => [xs[ys.len()], j],
            None if ys.len() > xs.len() => [xs.last().copied().unwrap_or(i), ys[xs.len()]],
            None => break [i, j],
        };
        if old[i].sequence {
            let grace = xs.len() != ys.len() || old[next[0]].grace || new[next[1]].grace;
            path.push((i, next[0], grace));
        }
        if differs.is_none() {
            break next;
        }
        (i, j) = (next[0], next[1]);
    };
    // A sequence looks at an element's grace time as the one before it
    // ends: where that can differ, from the deepest sequence whose element
    // on the path is not its first, the element before it.
    let mut at = first[0];
    for &(seq, child, grace) in path.iter().rev() {
        let elements = children(old, seq);
        match elements.iter().position(|&c| c == child) {
            Some(k) if k > 0 && grace => {
                at = elements[k - 1];
                break;
            }
            _ => {}
        }
    }
    // From the back: equal subtrees at preorder's end.
    let (mut k, mut l) = (old.len(), new.len());
    while k > first[0] + 1 && l > first[1] + 1 && old[k - 1].subtree == new[l - 1].subtree {
        k -= 1;
        l -= 1;
    }
    Some((first, [k - 1, l - 1], at))
}

/// The start of node `i` of `shape`, or of its nearest ancestor with one.
fn start_of(shape: &[ShapeNode], i: usize) -> Option<Moment> {
    if let Some(m) = shape.get(i)?.start {
        return Some(m);
    }
    // Its ancestors are the nodes before it whose subtree holds it.
    (0..i)
        .rev()
        .filter(|&a| a + shape[a].size as usize > i)
        .find_map(|a| shape[a].start)
}

/// `m_e`: the first moment the translation `new` can differ from `old`
/// at; `None` if their music and first state are the same. A changed node
/// that was never iterated, nor any ancestor of it, differs from the
/// start.
pub fn changed(old: &Kept, new: &Kept) -> Option<Moment> {
    let (first, _, at) = differing(old, new)?;
    Some(moment_of_change(&old.shape, first[0], at))
}

/// `m_e` of the old shape `old`, whose node `first` is the first that differs
/// and `at` the node a sequence looks at for it.
fn moment_of_change(old: &[ShapeNode], first: usize, at: usize) -> Moment {
    let start = |i| start_of(old, i).unwrap_or_default();
    let (a, b) = (start(first), start(at));
    if b.compare(a).is_lt() { b } else { a }
}

/// What `new`'s translation finds against `old`'s, both run whole.
pub fn compare(old: &Kept, new: &Kept) -> Change {
    let steps = new.steps.len() as u32;
    let Some((first, last, _)) = differing(old, new) else {
        return Change {
            changed: None,
            resume: None,
            converged: None,
            steps,
            run: 0,
            columns: Vec::new(),
            parts: None,
        };
    };
    let m_e = changed(old, new).unwrap_or_default();
    // The last checkpoint strictly before `m_e`: a grace note's timestep
    // comes before its main moment's.
    let resume = old
        .checkpoints
        .iter()
        .rposition(|c| c.moment.compare(m_e).is_lt());
    // Every changed node of a run past: a node never iterated never is.
    let passed = |shape: &[ShapeNode], from: usize, to: usize, at: Moment| {
        (from..=to).all(|i| shape[i].start.is_some_and(|s| s.compare(at).is_lt()))
    };
    let converged = new.checkpoints.iter().position(|c| {
        c.moment.compare(m_e).is_gt()
            && passed(&new.shape, first[1], last[1], c.moment)
            && passed(&old.shape, first[0], last[0], c.moment)
            && old
                .checkpoints
                .iter()
                .any(|o| o.moment == c.moment && o.digest == c.digest && o.future == c.future)
    });
    let from = resume.map_or(0, |r| old.checkpoints[r].step as usize);
    let to = converged.map_or(new.steps.len(), |c| new.checkpoints[c].step as usize);
    let until = converged.map(|c| new.checkpoints[c].moment);
    let before_end = |m: Moment| until.is_none_or(|u| m.compare(u).is_lt());
    let mut columns: Vec<Moment> = new.steps[from.min(new.steps.len())..to]
        .iter()
        .filter(|s| {
            !old.steps
                .iter()
                .any(|o| o.moment == s.moment && o.outputs == s.outputs)
        })
        .map(|s| s.moment)
        .collect();
    columns.extend(
        old.steps
            .iter()
            .filter(|o| {
                o.moment.compare(m_e).is_ge()
                    && before_end(o.moment)
                    && !new.steps.iter().any(|s| s.moment == o.moment)
            })
            .map(|o| o.moment),
    );
    columns.sort_by(|a, b| a.compare(*b));
    let mut change = Change {
        changed: Some(m_e),
        resume,
        converged,
        steps,
        run: to.saturating_sub(from) as u32,
        columns,
        parts: None,
    };
    change.parts = crate::partition::plan(old, new, &change).map(|p| p.run);
    change
}

/// What of `change` whole runs contradict: a checkpoint before `m_e`
/// whose state differs, a timestep before it whose input does, and from
/// the convergence on a timestep or checkpoint that differs in its moment,
/// input, output or state. Empty if the digests hold.
pub fn verify(old: &Kept, new: &Kept, change: &Change) -> Vec<String> {
    let mut wrong = Vec::new();
    let m_e = change.changed;
    let before = |m: Moment| m_e.is_none_or(|e| m.compare(e).is_lt());
    for (o, n) in old.checkpoints.iter().zip(&new.checkpoints) {
        if !before(o.moment) || !before(n.moment) {
            break;
        }
        if o.moment != n.moment || o.digest != n.digest {
            wrong.push(format!(
                "the checkpoint at timestep {} ({:?}), before m_e, differs",
                o.step, o.moment
            ));
            break;
        }
    }
    for (k, (o, n)) in old.steps.iter().zip(&new.steps).enumerate() {
        if !before(o.moment) || !before(n.moment) {
            break;
        }
        if o.moment != n.moment || o.inputs != n.inputs {
            wrong.push(format!(
                "timestep {k} ({:?}), before m_e, differs",
                o.moment
            ));
            break;
        }
    }
    if m_e.is_none() {
        if old.steps.len() != new.steps.len()
            || old
                .steps
                .iter()
                .zip(&new.steps)
                .any(|(o, n)| o.moment != n.moment || o.outputs != n.outputs)
        {
            wrong.push("the same music made other records".into());
        }
        return wrong;
    }
    let Some(c) = change.converged else {
        return wrong;
    };
    let at = new.checkpoints[c].moment;
    let tail = |steps: &[Step]| -> Vec<(Moment, u64, u64)> {
        steps
            .iter()
            .filter(|s| s.moment.compare(at).is_ge())
            .map(|s| (s.moment, s.inputs, s.outputs))
            .collect()
    };
    let (o, n) = (tail(&old.steps), tail(&new.steps));
    if o.len() != n.len() {
        wrong.push(format!(
            "converged at {at:?}: {} timesteps after it, the old run {}",
            n.len(),
            o.len()
        ));
    } else if let Some(k) = o.iter().zip(&n).position(|(a, b)| a != b) {
        let (a, b) = (o[k], n[k]);
        let what = if a.0 != b.0 {
            "moment"
        } else if a.1 != b.1 {
            "input"
        } else {
            "output"
        };
        wrong.push(format!(
            "converged at {at:?}: the timestep at {:?} differs in its {what}",
            b.0
        ));
    }
    let later = |cs: &[Checkpoint]| -> Vec<(Moment, u64)> {
        cs.iter()
            .filter(|x| x.moment.compare(at).is_ge())
            .map(|x| (x.moment, x.digest))
            .collect()
    };
    if later(&old.checkpoints) != later(&new.checkpoints) {
        wrong.push(format!("converged at {at:?}: a later checkpoint differs"));
    }
    wrong
}

/// [`verify`] and what of the partitions' plan whole runs contradict
/// (`partition.rs`).
pub fn verify_all(old: &Kept, new: &Kept, change: &Change) -> Vec<String> {
    let mut wrong = verify(old, new, change);
    if let Some(plan) = crate::partition::plan(old, new, change) {
        wrong.extend(crate::partition::verify(old, new, &plan));
    }
    wrong
}
