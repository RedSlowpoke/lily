//! LilyPond's music iterators (`music-iterator.cc` and its subclasses),
//! N4 (04 §8, MS2 steps 23–24): one enum in an arena, indexed by number,
//! with LilyPond's protocol: `create_children` when an iterator is made,
//! `init_context` and `create_contexts`, `pending_moment`, `ok`,
//! `run_always`, `process` and `quit`.
//!
//! - **N4's 13 iterators:** the base iterator (nothing, pending never), the
//!   simple music iterator and its event, rhythmic and event chord kinds,
//!   sequential and simultaneous music, the music wrapper, context-specced
//!   music (which finds or makes its context) and initial context music
//!   (step 23); the property operations (`\set`, `\unset`, `\override`,
//!   `\revert`), which send their events to their context, `\applyContext`
//!   and tuplets (step 24). Repeats, `\partial`, `\time` and multi-measure
//!   rests are sequences whose elements their callbacks give.
//! - **N6's grace iterator** (step 35, `grace-iterator.cc`): a wrapper
//!   whose child's main time is its own grace time, and which sends
//!   `GraceChange` to the child's context when grace time begins and ends
//!   (`Grace_engraver`, `grace.rs`, hears it).
//! - **N6's change iterator** (step 36, `change-iterator.cc`): `\change
//!   Type = id`, a simple iterator that, when it is processed, finds the
//!   context of the type above its own and the one to change to near it,
//!   and moves the branch below the first, over each iterator of its scope
//!   (`where_tagged`, `preorder_walk`), to the second by a `ChangeParent`
//!   event (`context.rs`), or takes an iterator held by the first to the
//!   second; LilyPond's warnings when either is not found. So an iterator
//!   knows its parent, as LilyPond's do.
//! - **N7's volta iterators** (step 37, `volta-repeat-iterator.cc`,
//!   `alternative-sequence-iterator.cc`, `volta-specced-music-iterator.cc`,
//!   `repeat-styler.cc`): `\repeat volta` and `\repeat segno` are a kind of
//!   sequence whose repeat styler announces the repeat's start, its
//!   returns and its alternatives by events to the repeat's context;
//!   `\alternative` a sequence that asks its repeat's styler, saves
//!   `alternativeRestores` and restores them after each alternative but the
//!   last; a volta-specced music a wrapper that sends `VoltaSpanEvent`s
//!   where its alternatives get brackets.
//! - **N7's lyric combine iterator** (step 38,
//!   `lyric-combine-music-iterator.cc`): `\lyricsto` and `\addlyrics`, a
//!   kind that runs always while its lyrics last and its voice stays, and
//!   processes its lyrics' iterator one syllable at a time, at the moments
//!   the voice it follows starts a note. Its listeners, which a dispatch
//!   calls, keep what they share in the arena (`lyrics.rs`).
//! - **The percent repeat and fine iterators** (MS5 step 6,
//!   `percent-repeat-iterator.cc`, `fine-iterator.cc`): `\repeat percent`
//!   a sequence of its body and placeholders that reports a percent,
//!   double percent or repeat slash event at each repetition; `\fine` a
//!   simple iterator that reports a copy of its music, `fine-folded`
//!   inside a folded repeat.
//! - A type's `iterator-ctor` names its iterator by LilyPond's primitive
//!   (D67's placeholders, still placeholders when Scheme calls them);
//!   music without one is an event's or a simple one. Any other
//!   constructor's music is iterated as simple music, which keeps its
//!   time, and named in [`Iteration::unported`].
//! - **Music is read where it is** ([`At`]): a frozen tree's nodes are not
//!   thawed, and live music is read through its handle. A sequence's
//!   elements are its `elements-callback`'s, which for `SequentialMusic`
//!   are its `elements`, read natively; another type's is called in
//!   Scheme, and what it gives is rooted till the translation ends.
//! - **Events refer to their music** (`Music::to_event`,
//!   [`crate::dispatcher::music_event`]) and are broadcast by the contexts'
//!   event sources as LilyPond's are.
//!
//! An iterator holds its context as a `Context_handle` does: the context
//! counts it among its clients, which keep it from removal.

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Exception, Heap, Rows, Tag, Tracer, Value};

use crate::context::{
    Dir, GLOBAL, Host, Translation, call, create_unique_context, diagnose, diagnostic_id, dispatch,
    find_create_context, get_default_interpreter, get_user_accessible_interpreter, send,
};
use crate::dispatcher::{Listener, StreamEvent, music_event, set_event_property};
use crate::lyrics::{self, Combine, Heard};
use crate::music::{At, Item, Music};
use crate::{CtxId, Duration, Input, Moment, Rational};

type Run<T> = Result<T, Exception>;

/// An iterator's number in its [`Iteration`].
pub type IterId = u32;

/// `Moment::infinity ()`.
pub const INFINITY: Moment = Moment {
    main: Rational::INFINITY,
    grace: Rational::ZERO,
};

const ZERO: Moment = Moment {
    main: Rational::ZERO,
    grace: Rational::ZERO,
};

fn add(a: Moment, b: Moment) -> Moment {
    a.checked_add(b).unwrap_or(INFINITY)
}

fn sub(a: Moment, b: Moment) -> Moment {
    a.checked_sub(b).unwrap_or(INFINITY)
}

fn lt(a: Moment, b: Moment) -> bool {
    a.compare(b).is_lt()
}

fn min(a: Moment, b: Moment) -> Moment {
    if lt(b, a) { b } else { a }
}

/// Music an iterator reads: a handle, or a node of a frozen tree.
#[derive(Clone, Copy, PartialEq, Eq)]
enum M {
    Value(Value),
    Node(Value, u32),
}

impl M {
    fn at(self, heap: &Heap) -> Option<At> {
        match self {
            M::Value(v) => At::of(heap, v),
            M::Node(tree, node) => Some(At::Frozen { tree, node }),
        }
    }

    /// Its handle: the value, or a new handle of the node.
    fn handle(self, heap: &mut Heap) -> Value {
        match self {
            M::Value(v) => v,
            M::Node(tree, node) => heap.wrap(Music::frozen(tree, node)),
        }
    }
}

/// Its key in N8's shape of the music (`checkpoint.rs`).
fn key(m: M) -> crate::checkpoint::Key {
    match m {
        M::Value(v) => crate::checkpoint::handle_key(v),
        M::Node(tree, node) => crate::checkpoint::node_key(tree, node),
    }
}

/// The music `item` is.
fn music_of(heap: &Heap, item: &Item) -> Option<M> {
    match *item {
        Item::Value(v) => heap.foreign::<Music>(v).map(|_| M::Value(v)),
        Item::Music(At::Frozen { tree, node }) => Some(M::Node(tree, node)),
        _ => None,
    }
}

/// The items of the list `item` is, each music or `None`.
fn musics_of(heap: &Heap, item: &Item) -> Vec<Option<M>> {
    match item {
        Item::Value(v) => {
            let mut items = Vec::new();
            let mut p = *v;
            while let (Some(x), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                items.push(heap.foreign::<Music>(x).map(|_| M::Value(x)));
                p = rest;
            }
            items
        }
        Item::Music(_) => Vec::new(),
        Item::Musics(all) => all
            .iter()
            .map(|at| match *at {
                At::Frozen { tree, node } => Some(M::Node(tree, node)),
                At::Live(_) => None,
            })
            .collect(),
    }
}

/// What a simple music iterator does when it starts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Simple {
    /// `Simple_music_iterator`: nothing.
    Plain,
    /// `Event_iterator`: reports its music.
    Event,
    /// `Rhythmic_music_iterator`: broadcasts its music's event and the
    /// articulations heard apart.
    Rhythmic,
    /// `Event_chord_iterator`: reports its elements and articulations.
    Chord,
    /// `Property_iterator`: `SetProperty` to its context, each time.
    Set,
    /// `Property_unset_iterator`: `UnsetProperty`.
    Unset,
    /// `Push_property_iterator`: `Override` (after `Revert` for
    /// `pop-first`), of a grob.
    Push,
    /// `Pop_property_iterator`: `Revert`, of a grob.
    Pop,
    /// `Apply_context_iterator`: its procedure called with its context.
    Apply,
    /// `Change_iterator`: `\change Type = id`.
    Change,
    /// `Fine_iterator`: a copy of its music, `fine-folded` inside a folded
    /// repeat, reported when it starts but inside combined lyrics.
    Fine,
}

/// What a music wrapper iterator is besides one.
#[derive(Clone)]
enum Wrap {
    Plain,
    /// `Context_specced_music_iterator`.
    Specced,
    /// `Tuplet_iterator`: `TupletSpanEvent`s at the start and end of each
    /// bracket, one every `tupletSpannerDuration` if that is set.
    Tuplet {
        /// `spanner_duration_`, negative till the first `process`.
        spanner: Moment,
        /// `next_split_mom_`.
        next: Moment,
        /// `tuplet_handler_`: the context the bracket started in, a
        /// client of it.
        handler: Option<CtxId>,
    },
    /// `Grace_iterator`: its child's time is the grace part of its own;
    /// `GraceChange` to the child's context when grace time begins or
    /// ends.
    Grace {
        /// `in_grace_`.
        in_grace: bool,
    },
    /// `Volta_specced_music_iterator`: `VoltaSpanEvent`s at its start and
    /// end when its parent, an `\alternative`, has brackets.
    Volta {
        /// `event_handler_`, a client of its context.
        handler: Option<CtxId>,
        depth: usize,
        started: bool,
        stopped: bool,
    },
    /// `Quote_iterator`: the events of its `quoted-events` that fall within
    /// its music's time, broadcast to its cue context besides its child.
    Quote(Quote),
}

/// `Quote_iterator`'s own state. Its `event_vector_` is its music's
/// `quoted-events`, read where it is used.
#[derive(Clone)]
struct Quote {
    /// `quote_handle_`: the context its events go to, a client of it.
    handler: Option<CtxId>,
    /// `zero_mom_`: its music's zero in the score's time, `-∞` till its
    /// first `process`.
    zero: Moment,
    /// `event_idx_` (closed) and `end_idx_` (open).
    idx: usize,
    end: usize,
    first_time: bool,
    /// The moment of the entry at `idx` while `idx < end`, for `pending`;
    /// none where that entry has none.
    next: Option<Moment>,
}

/// `Sequential_iterator`'s state.
#[derive(Clone)]
struct Seq {
    elements: Vec<Option<M>>,
    /// `remaining_music_` and `ahead_music_`: indices of `elements`.
    remaining: usize,
    ahead: usize,
    iter: Option<IterId>,
    iter_start: Moment,
    ahead_mom: Moment,
    /// What it is besides one.
    volta: SeqKind,
}

#[derive(Clone)]
enum SeqKind {
    Plain,
    /// `Volta_repeat_iterator`.
    Repeat {
        styler: Styler,
        started: bool,
        stopped: bool,
    },
    /// `Alternative_sequence_iterator`.
    Alternatives(Alternatives),
    /// `Percent_repeat_iterator`: its body, then an element for each
    /// repetition, at whose start it reports a percent event.
    Percent(Percent),
}

/// `Percent_repeat_iterator`'s own state.
#[derive(Clone)]
struct Percent {
    done: i64,
    reps: i64,
    /// `body_length_`.
    body: Moment,
    /// `event_type_`, chosen at the first repetition.
    event: Option<&'static str>,
    /// `slash_count_`: a repeat slash's, rooted.
    slashes: Option<Value>,
}

/// `Alternative_sequence_iterator`'s own state.
#[derive(Clone)]
struct Alternatives {
    /// The volta repeat iterator above whose styler it uses; none is a
    /// null styler's.
    repeat: Option<IterId>,
    first_time: bool,
    /// `volta_brackets_enabled_` and `volta_bracket_depth_`.
    brackets: bool,
    depth: usize,
    done: usize,
    /// `alt_restores_`: `(context symbol value)`, the last saved first.
    restores: Vec<(CtxId, Value, Value)>,
    /// Each alternative's `return_count`.
    returns: Vec<i64>,
}

/// What a `Repeat_styler` is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Null,
    Segno { coda_marks: bool },
    Volta,
}

/// `Repeat_styler`: a volta repeat iterator's, which its alternatives use.
#[derive(Clone)]
struct Styler {
    style: Style,
    /// `spanned_time_`'s ends.
    spanned: (Moment, Moment),
    repeat_count: i64,
    alternative_depth: usize,
    reported_return_depth: usize,
    reported_return: bool,
}

#[derive(Clone)]
enum Kind {
    /// `Music_iterator`.
    Base,
    Simple {
        how: Simple,
        pending: Moment,
    },
    Sequential(Seq),
    Simultaneous(Vec<IterId>),
    /// `Music_wrapper_iterator` and its kinds.
    Wrapper {
        child: Option<IterId>,
        wrap: Wrap,
    },
    /// `Initial_context_music_iterator`.
    Initial {
        child: Option<IterId>,
    },
    /// `Lyric_combine_music_iterator`.
    Combine(Combined),
}

/// `Lyric_combine_music_iterator`'s own state; what its listeners share is
/// the arena's (`lyrics.rs`).
#[derive(Clone)]
struct Combined {
    /// `lyric_iter_`.
    lyric: Option<IterId>,
    /// Its number among the arena's, once its contexts are made.
    n: Option<u32>,
    /// `music_found_` and `lyrics_found_`.
    music_found: bool,
    lyrics_found: bool,
    /// `pending_grace_moment_`: a grace note's main moment, whose syllable
    /// waits for it.
    pending_grace: Moment,
}

#[derive(Clone)]
pub(crate) struct It {
    kind: Kind,
    /// `parent_`: the iterator that made it.
    parent: Option<IterId>,
    music: M,
    length: Moment,
    start: Moment,
    /// Its own context (`handle_`).
    context: Option<CtxId>,
    /// A `Part_combine_iterator`, a simultaneous kind.
    part_combine: bool,
}

/// The property names the iterators read.
#[derive(Clone)]
struct Keys {
    elements: Value,
    element: Value,
    articulations: Value,
    iterator_ctor: Value,
    elements_callback: Value,
    name: Value,
    types: Value,
    origin: Value,
    context_type: Value,
    context_id: Value,
    property_operations: Value,
    search_direction: Value,
    create_new: Value,
    class: Value,
    length: Value,
    music_cause: Value,
    symbol: Value,
    value: Value,
    once: Value,
    grob_property: Value,
    grob_property_path: Value,
    grob_value: Value,
    pop_first: Value,
    procedure: Value,
    duration: Value,
    numerator: Value,
    denominator: Value,
    tweaks: Value,
    tuplet_spanner_duration: Value,
    change_to_type: Value,
    change_to_id: Value,
    change_tag: Value,
    tags: Value,
    repeat_count: Value,
    volta_numbers: Value,
    associated_context: Value,
    associated_context_type: Value,
    quoted_events: Value,
    quoted_context_type: Value,
    quoted_context_id: Value,
    quoted_transposition: Value,
}

/// The iterators of one translation.
#[derive(Clone)]
pub struct Iteration {
    /// In rows, which N8's snapshots share (D259).
    its: Rows<It>,
    /// The top iterator and the music's length, once made.
    pub(crate) top: Option<(IterId, Moment)>,
    t: Translation,
    keys: Keys,
    /// The names a length or start reads.
    lengths: crate::length::Keys,
    /// What its translation's start asked ahead for `create_top` (B23,
    /// D272), empty after it.
    pub(crate) asked: crate::length::Asked,
    /// The constructors of the iterators not ported yet that music asked
    /// for, each once.
    pub unported: Vec<&'static str>,
    /// The contexts of the iterators `rebuild` put on music that changed,
    /// each the nearest of its own and those above it (N8's replay, MS6
    /// step 7b).
    pub(crate) changed: Vec<CtxId>,
    /// H3 (D254): whether the global loop may return between timesteps
    /// ([`crate::returns`]), whether it just did, and whether it has run a
    /// timestep before that return.
    pub(crate) returns: bool,
    pub(crate) paused: bool,
    pub(crate) under_way: bool,
    /// A timestep the global loop returned in the middle of (B23, D272): its
    /// moment, whether its iterators run, and whether they have run.
    pub(crate) mid: Option<(Moment, bool, bool)>,
}

impl Iteration {
    /// Its iterators, copied for N8's snapshots (`resume.rs`).
    pub(crate) fn state(&self) -> Rows<It> {
        self.its.clone()
    }

    /// Its iterators as a snapshot had them.
    pub(crate) fn put_back(&mut self, its: &Rows<It>) {
        self.its = its.clone();
    }

    /// The live music the iterators of `its` hold, and the values beside.
    pub(crate) fn trace(its: &Rows<It>, tracer: &mut Tracer<'_>) {
        let music = |m: &M| match *m {
            M::Value(v) | M::Node(v, _) => v,
        };
        its.trace(tracer, |it, tracer| {
            tracer.trace(music(&it.music));
            if let Kind::Sequential(seq) = &it.kind {
                for m in seq.elements.iter().flatten() {
                    tracer.trace(music(m));
                }
                match &seq.volta {
                    SeqKind::Percent(p) => {
                        if let Some(v) = p.slashes {
                            tracer.trace(v);
                        }
                    }
                    SeqKind::Alternatives(a) => {
                        for &(_, s, v) in &a.restores {
                            tracer.trace(s);
                            tracer.trace(v);
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    /// N8's resume (`resume.rs`, D241): the iterators a snapshot made
    /// (`its`), put on the new music. `to_new` gives the new music of a
    /// node by its key: its object, its node in a frozen tree (`u32::MAX`
    /// for a handle) and whether its subtree differs from the old. An
    /// iterator's music and a sequence's elements are moved; one whose
    /// music changed has its length read again, and a sequence its
    /// elements, which must be the old ones as far as it has looked; music
    /// the new shape has not got (a callback's) is kept where nothing above
    /// it changed, but a sequence's elements its callback made, which are
    /// made again from the new music, as many, and the iterators made on
    /// them moved to them by place (MS6 step 7: an old repeat's elements
    /// held the old music's places). False where the iterators cannot go on
    /// on the new music.
    pub(crate) fn rebuild(
        &mut self,
        host: &mut dyn Host,
        its: &Rows<It>,
        to_new: &dyn Fn(crate::checkpoint::Key) -> Option<(Value, u32, bool)>,
    ) -> Run<bool> {
        // A callback's old elements and the new ones made in their place.
        let made: core::cell::RefCell<Vec<(M, M)>> = core::cell::RefCell::new(Vec::new());
        let moved = |m: M| -> (M, Option<bool>) {
            if let Some(&(_, new)) = made.borrow().iter().find(|(old, _)| *old == m) {
                return (new, Some(false));
            }
            match to_new(key(m)) {
                Some((v, u32::MAX, changed)) => (M::Value(v), Some(changed)),
                Some((tree, node, changed)) => (M::Node(tree, node), Some(changed)),
                None => (m, None),
            }
        };
        self.its = its.clone();
        // Whether each iterator's music, or an iterator's above it, changed.
        let mut changed = alloc::vec![false; self.its.len()];
        for id in 0..self.its.len() {
            let was = self.its[id].music;
            let (m, c) = moved(was);
            let above = self.its[id]
                .parent
                .is_some_and(|p| changed.get(p as usize).copied().unwrap_or(false));
            let c = match c {
                Some(c) => c,
                // Music the shape has not got under changed music.
                None if above => return Ok(false),
                None => false,
            };
            changed[id] = c;
            // Only what differs is written: a write copies a row (D259).
            if m != was {
                self.its[id].music = m;
            }
            if !c {
                let Kind::Sequential(seq) = &self.its[id].kind else {
                    continue;
                };
                let old = seq.elements.clone();
                let elements = if old.iter().flatten().any(|&e| moved(e).1.is_none()) {
                    let new = self.calc_elements(host, m)?;
                    if new.len() != old.len() {
                        return Ok(false);
                    }
                    for (o, n) in old.iter().zip(&new) {
                        match (o, n) {
                            (Some(o), Some(n)) => made.borrow_mut().push((*o, *n)),
                            (None, None) => {}
                            _ => return Ok(false),
                        }
                    }
                    new
                } else {
                    old.iter().map(|e| e.map(|e| moved(e).0)).collect()
                };
                if old != elements
                    && let Kind::Sequential(seq) = &mut self.its[id].kind
                {
                    seq.elements = elements;
                }
                continue;
            }
            let (length, start) = (self.time(host, m, false)?, self.time(host, m, true)?);
            if start != self.its[id].start {
                return Ok(false);
            }
            if length != self.its[id].length {
                self.its[id].length = length;
            }
            match &self.its[id].kind {
                Kind::Sequential(seq) => {
                    let (was, all, ahead) = (
                        seq.elements[..seq.ahead.min(seq.elements.len())].to_vec(),
                        seq.elements.len(),
                        seq.ahead_mom,
                    );
                    let old: Vec<Option<M>> = was.iter().map(|e| e.map(|e| moved(e).0)).collect();
                    let new = self.calc_elements(host, m)?;
                    if new.len() < old.len() || new[..old.len()] != old[..] {
                        return Ok(false);
                    }
                    // The elements it has looked at are summed in its next
                    // element's moment: an edit inside one moves it (MS6
                    // step 7: an insertion in the element being played).
                    let mut grown = Rational::ZERO;
                    for (w, n) in was.iter().zip(&new) {
                        if let (Some(w), Some(n)) = (w, n) {
                            let (a, b) = (self.time(host, *w, false)?, self.time(host, *n, false)?);
                            grown = grown
                                .checked_add(
                                    b.main.checked_sub(a.main).unwrap_or(Rational::INFINITY),
                                )
                                .unwrap_or(Rational::INFINITY);
                        }
                    }
                    if ahead == INFINITY && new.len() != all || grown == Rational::INFINITY {
                        return Ok(false);
                    }
                    if let Kind::Sequential(seq) = &mut self.its[id].kind {
                        seq.elements = new;
                        if ahead != INFINITY {
                            seq.ahead_mom.main =
                                ahead.main.checked_add(grown).unwrap_or(Rational::INFINITY);
                        }
                    }
                    if ahead != INFINITY {
                        self.ahead_grace(host, id as IterId)?;
                    }
                }
                // As many elements as the music it was made on: its children
                // that ended are gone, and the others' music moved above.
                Kind::Simultaneous(_) => {
                    let heap = host.heap();
                    let count = |m: M| {
                        m.at(heap)
                            .and_then(|at| at.property(heap, self.keys.elements))
                            .map_or(0, |item| musics_of(heap, &item).len())
                    };
                    if count(m) != count(was) {
                        return Ok(false);
                    }
                }
                _ => {}
            }
        }
        self.changed = (0..self.its.len())
            .filter(|&id| changed[id])
            .filter_map(|mut id| {
                loop {
                    if let Some(c) = self.its[id].context {
                        break Some(c);
                    }
                    id = self.its[id].parent? as usize;
                }
            })
            .collect();
        Ok(true)
    }

    pub fn new(heap: &mut Heap, t: &Translation) -> Iteration {
        let mut k = |name| heap.intern(name);
        let keys = Keys {
            elements: k("elements"),
            element: k("element"),
            articulations: k("articulations"),
            iterator_ctor: k("iterator-ctor"),
            elements_callback: k("elements-callback"),
            name: k("name"),
            types: k("types"),
            origin: k("origin"),
            context_type: k("context-type"),
            context_id: k("context-id"),
            property_operations: k("property-operations"),
            search_direction: k("search-direction"),
            create_new: k("create-new"),
            class: k("class"),
            length: k("length"),
            music_cause: k("music-cause"),
            symbol: k("symbol"),
            value: k("value"),
            once: k("once"),
            grob_property: k("grob-property"),
            grob_property_path: k("grob-property-path"),
            grob_value: k("grob-value"),
            pop_first: k("pop-first"),
            procedure: k("procedure"),
            duration: k("duration"),
            numerator: k("numerator"),
            denominator: k("denominator"),
            tweaks: k("tweaks"),
            tuplet_spanner_duration: k("tupletSpannerDuration"),
            change_to_type: k("change-to-type"),
            change_to_id: k("change-to-id"),
            change_tag: k("change-tag"),
            tags: k("tags"),
            repeat_count: k("repeat-count"),
            volta_numbers: k("volta-numbers"),
            associated_context: k("associated-context"),
            associated_context_type: k("associated-context-type"),
            quoted_events: k("quoted-events"),
            quoted_context_type: k("quoted-context-type"),
            quoted_context_id: k("quoted-context-id"),
            quoted_transposition: k("quoted-transposition"),
        };
        Iteration {
            its: Rows::default(),
            top: None,
            t: t.clone(),
            keys,
            lengths: crate::length::Keys::new(heap),
            asked: crate::length::Asked::default(),
            unported: Vec::new(),
            changed: Vec::new(),
            returns: false,
            paused: false,
            under_way: false,
            mid: None,
        }
    }

    fn it(&self, id: IterId) -> &It {
        &self.its[id as usize]
    }

    fn it_mut(&mut self, id: IterId) -> &mut It {
        &mut self.its[id as usize]
    }

    /// The value of `m`'s property `key`, `'()` for none or for music.
    fn value(&self, heap: &Heap, m: M, key: Value) -> Value {
        m.at(heap)
            .and_then(|at| at.property(heap, key))
            .and_then(|item| item.value())
            .unwrap_or(Value::NULL)
    }

    /// `get_length`, or `start_mom` if `start`: in Rust, which calls
    /// Scheme for the procedures of Scheme it meets; through
    /// `ly:music-length` or `ly:music-start` for what is no music or does
    /// not fit, which they report.
    fn time(&self, host: &mut dyn Host, m: M, start: bool) -> Run<Moment> {
        if let Some(at) = m.at(host.heap()) {
            let handle = match m {
                M::Value(v) => Some(v),
                M::Node(..) => None,
            };
            if let Some(moment) =
                crate::length::hosted(host, &self.lengths, &self.asked, at, handle, start)?
            {
                return Ok(moment);
            }
        }
        let name = if start {
            "ly:music-start"
        } else {
            "ly:music-length"
        };
        let handle = m.handle(host.heap());
        let depth = host.heap().root_depth();
        host.heap().root(handle);
        let result = match host.lily(name) {
            Some(proc) => host.call(proc, &[handle]),
            None => Ok(Value::NULL),
        };
        host.heap().unroot_to(depth);
        Ok(host
            .heap()
            .foreign::<Moment>(result?)
            .copied()
            .unwrap_or(ZERO))
    }

    /// `Diagnostics::warning`: at the music's origin, or nowhere.
    fn warning(&self, host: &mut dyn Host, m: M, text: &str) -> Run<()> {
        let heap = host.heap();
        let origin = self.value(heap, m, self.keys.origin);
        if heap.foreign::<Input>(origin).is_none() {
            return diagnose(host, "warning", text);
        }
        if let Some(proc) = host.lily("ly:input-warning") {
            let text = host.heap().string(text.replace('~', "~~"));
            host.call(proc, &[origin, text])?;
        }
        Ok(())
    }

    /// N8's digest of the iterators below `top` (`checkpoint.rs`): each
    /// one's kind, start and context, its own node by `music`, and of a
    /// sequence how far it is. Not what music is left, nor a length: the
    /// timesteps' inputs and the changed nodes' starts say whether that is
    /// the old run's.
    ///
    /// And apart, what music is left (its future): a simple iterator's
    /// whole music and the elements a sequence has not started, by `whole`.
    /// Each iterator's two digests are put in `each` too, by its context
    /// (`u32::MAX` for none), for N8's partitions (`partition.rs`).
    pub(crate) fn digest(
        &self,
        top: IterId,
        heap: &Heap,
        music: &dyn Fn(crate::checkpoint::Key, Option<At>) -> u64,
        whole: &dyn Fn(crate::checkpoint::Key, Option<At>) -> u64,
        context: &dyn Fn(CtxId) -> u64,
        each: &mut Vec<(CtxId, u64, u64)>,
    ) -> (u64, u64) {
        use crate::checkpoint::Digest;
        let mut d = Digest::new(20);
        let mut future = Digest::new(21);
        let mut todo = alloc::vec![top];
        while let Some(id) = todo.pop() {
            let Some(it) = self.its.get(id as usize) else {
                continue;
            };
            let (mut own, mut left) = (Digest::new(20), Digest::new(21));
            own.u64(music(key(it.music), it.music.at(heap)));
            own.moment(it.start);
            own.u64(it.context.map_or(u64::MAX, context));
            match &it.kind {
                Kind::Base => own.u64(1),
                Kind::Simple { pending, .. } => {
                    own.u64(2);
                    own.moment(*pending);
                    left.u64(whole(key(it.music), it.music.at(heap)));
                }
                Kind::Sequential(seq) => {
                    own.u64(3);
                    own.u64(seq.remaining as u64);
                    left.u64(seq.elements.len().saturating_sub(seq.remaining) as u64);
                    for m in seq.elements.iter().skip(seq.remaining) {
                        left.u64(m.map_or(0, |m| whole(key(m), m.at(heap))));
                    }
                    own.moment(seq.iter_start);
                    todo.extend(seq.iter);
                }
                Kind::Simultaneous(children) => {
                    own.u64(4);
                    todo.extend(children.iter().rev());
                }
                Kind::Wrapper { child, wrap } => {
                    own.u64(5);
                    if let Wrap::Quote(q) = wrap {
                        own.u64(q.idx as u64);
                        own.u64(q.end as u64);
                        own.u64(u64::from(q.first_time));
                        own.moment(q.zero);
                    }
                    todo.extend(*child);
                }
                Kind::Initial { child } => {
                    own.u64(6);
                    todo.extend(*child);
                }
                Kind::Combine(c) => {
                    own.u64(7);
                    own.u64(u64::from(c.music_found) | u64::from(c.lyrics_found) << 1);
                    own.moment(c.pending_grace);
                    todo.extend(c.lyric);
                }
            }
            let (own, left) = (own.get(), left.get());
            d.u64(own);
            future.u64(left);
            each.push((it.context.unwrap_or(u32::MAX), own, left));
        }
        (d.get(), future.get())
    }

    // --- creation --------------------------------------------------------

    /// `create_top_iterator` of `music`, with what was asked ahead of it.
    pub fn create_top(&mut self, host: &mut dyn Host, music: Value) -> Run<IterId> {
        let top = self.create(host, M::Value(music), None);
        self.asked.clear();
        top
    }

    /// Asks ahead of `create_top` (B23, D272): whether it is done, else the
    /// step's time is spent.
    pub(crate) fn ask_ahead(&mut self, host: &mut dyn Host, music: Value) -> Run<bool> {
        if self.asked.is_new() {
            self.asked = crate::length::Asked::begin(host, music);
        }
        let mut asked = core::mem::take(&mut self.asked);
        let done = asked.on(host, &self.lengths, &crate::returns::time_up);
        self.asked = asked;
        done
    }

    /// `create_iterator`: the iterator its type's constructor names, made
    /// with its length and start by `parent`, and its children
    /// (`create_children`).
    fn create(&mut self, host: &mut dyn Host, m: M, parent: Option<IterId>) -> Run<IterId> {
        if let M::Value(v) = m {
            // Live music: rooted till the translation ends.
            host.heap().root(v);
        }
        let heap = host.heap();
        let ctor = self.value(heap, m, self.keys.iterator_ctor);
        let kind = match heap.primitive_name(ctor) {
            Some("ly:music-iterator::constructor") => Kind::Base,
            Some("ly:simple-music-iterator::constructor") => simple(Simple::Plain),
            Some("ly:rhythmic-music-iterator::constructor") => simple(Simple::Rhythmic),
            Some("ly:event-chord-iterator::constructor") => simple(Simple::Chord),
            Some("ly:sequential-iterator::constructor") => sequential(SeqKind::Plain),
            Some("ly:volta-repeat-iterator::constructor") => sequential(SeqKind::Repeat {
                styler: Styler::new(Style::Null),
                started: false,
                stopped: false,
            }),
            Some("ly:alternative-sequence-iterator::constructor") => {
                sequential(SeqKind::Alternatives(Alternatives {
                    repeat: None,
                    first_time: true,
                    brackets: true,
                    depth: 1,
                    done: 0,
                    restores: Vec::new(),
                    returns: Vec::new(),
                }))
            }
            Some("ly:volta-specced-music-iterator::constructor") => wrapper(Wrap::Volta {
                handler: None,
                depth: 0,
                started: false,
                stopped: false,
            }),
            Some(
                "ly:simultaneous-music-iterator::constructor"
                | "ly:part-combine-iterator::constructor",
            ) => Kind::Simultaneous(Vec::new()),
            Some("ly:music-wrapper-iterator::constructor") => wrapper(Wrap::Plain),
            Some("ly:context-specced-music-iterator::constructor") => wrapper(Wrap::Specced),
            Some("ly:tuplet-iterator::constructor") => wrapper(Wrap::Tuplet {
                spanner: Moment::new(Rational::int(-1), Rational::ZERO),
                next: ZERO,
                handler: None,
            }),
            Some("ly:grace-iterator::constructor") => wrapper(Wrap::Grace { in_grace: false }),
            Some("ly:quote-iterator::constructor") => wrapper(Wrap::Quote(Quote {
                handler: None,
                zero: Moment::new(-Rational::INFINITY, Rational::ZERO),
                idx: 0,
                end: 0,
                first_time: true,
                next: None,
            })),
            Some("ly:property-iterator::constructor") => simple(Simple::Set),
            Some("ly:property-unset-iterator::constructor") => simple(Simple::Unset),
            Some("ly:push-property-iterator::constructor") => simple(Simple::Push),
            Some("ly:pop-property-iterator::constructor") => simple(Simple::Pop),
            Some("ly:apply-context-iterator::constructor") => simple(Simple::Apply),
            Some("ly:change-iterator::constructor") => simple(Simple::Change),
            Some("ly:fine-iterator::constructor") => simple(Simple::Fine),
            Some("ly:percent-repeat-iterator::constructor") => {
                sequential(SeqKind::Percent(Percent {
                    done: 0,
                    reps: 0,
                    body: ZERO,
                    event: None,
                    slashes: None,
                }))
            }
            Some("ly:initial-context-music-iterator::constructor") => Kind::Initial { child: None },
            Some("ly:lyric-combine-music-iterator::constructor") => Kind::Combine(Combined {
                lyric: None,
                n: None,
                music_found: false,
                lyrics_found: false,
                pending_grace: INFINITY,
            }),
            Some(other) => {
                if !self.unported.contains(&other) {
                    self.unported.push(other);
                }
                simple(Simple::Plain)
            }
            None if heap.is_procedure(ctor) => {
                if !self.unported.contains(&"a procedure") {
                    self.unported.push("a procedure");
                }
                simple(Simple::Plain)
            }
            None if self.is_event(heap, m) => simple(Simple::Event),
            None => simple(Simple::Plain),
        };
        let part_combine =
            heap.primitive_name(ctor) == Some("ly:part-combine-iterator::constructor");
        let length = self.time(host, m, false)?;
        let start = self.time(host, m, true)?;
        let id = self.its.len() as IterId;
        self.its.push(It {
            kind,
            parent,
            music: m,
            length,
            start,
            context: None,
            part_combine,
        });
        let whole = matches!(self.its[id as usize].kind, Kind::Simple { .. });
        let heap = host.heap();
        let (recording, now) = {
            let mut a = self.t.arena.borrow_mut();
            (a.n8.take(), a.now())
        };
        if let Some(mut r) = recording {
            r.made(heap, key(m), m.at(heap), whole, now);
            self.t.arena.borrow_mut().n8 = Some(r);
        }
        self.create_children(host, id)?;
        Ok(id)
    }

    /// `is_mus_type ("event")`.
    fn is_event(&self, heap: &Heap, m: M) -> bool {
        self.is_type(heap, m, "event")
    }

    /// `is_mus_type (name)`.
    fn is_type(&self, heap: &Heap, m: M, name: &str) -> bool {
        let Some(at) = m.at(heap) else {
            return false;
        };
        let mut p = at
            .immutable_property(heap, self.keys.types)
            .unwrap_or(Value::NULL);
        while let (Some(t), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            if t.tag() == Tag::Symbol && heap.name(t) == Some(name) {
                return true;
            }
            p = rest;
        }
        false
    }

    /// `find_above_by_music_type`: `id` or the nearest of its ancestors
    /// whose music is of the type `name`.
    fn find_above(&self, heap: &Heap, id: IterId, name: &str) -> Option<IterId> {
        let mut at = Some(id);
        while let Some(i) = at {
            if self.is_type(heap, self.it(i).music, name) {
                return Some(i);
            }
            at = self.it(i).parent;
        }
        None
    }

    /// `get_property (iterator, key)` (`internal_where_defined`): the
    /// music's of `id` or of the nearest ancestor that sets it, or `'()`.
    fn inherited(&self, heap: &Heap, id: IterId, key: Value) -> Value {
        let mut at = Some(id);
        while let Some(i) = at {
            let v = self.value(heap, self.it(i).music, key);
            if v != Value::NULL {
                return v;
            }
            at = self.it(i).parent;
        }
        Value::NULL
    }

    fn create_children(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let (m, start) = (self.it(id).music, self.it(id).start);
        match &mut self.it_mut(id).kind {
            Kind::Simple { pending, .. } => *pending = start,
            Kind::Sequential(_) => {
                // `Volta_repeat_iterator::create_children`: its styler, a
                // null one inside lyrics combined with a voice.
                if matches!(self.seq(id).volta, SeqKind::Repeat { .. }) {
                    let heap = host.heap();
                    let style = if self.is_type(heap, m, "segno-repeated-music") {
                        Style::Segno { coda_marks: true }
                    } else if self.is_type(heap, m, "volta-repeated-music") {
                        Style::Volta
                    } else {
                        diagnose(
                            host,
                            "programming-error",
                            "no repeat styler for this type of music",
                        )?;
                        Style::Null
                    };
                    let accurate = self
                        .find_above(host.heap(), id, "lyric-combine-music")
                        .is_none();
                    if let SeqKind::Repeat { styler, .. } = &mut self.seq(id).volta {
                        styler.style = if accurate { style } else { Style::Null };
                    }
                }
                let elements = self.calc_elements(host, m)?;
                if let Kind::Sequential(s) = &mut self.it_mut(id).kind {
                    s.elements = elements;
                    s.iter_start = start;
                    s.ahead_mom = start;
                }
                self.pop_element(host, id)?;
                // `Alternative_sequence_iterator::create_children`: the
                // styler of the nearest folded repeat above.
                if matches!(self.seq(id).volta, SeqKind::Alternatives(_)) {
                    let repeat = self
                        .find_above(host.heap(), id, "folded-repeated-music")
                        .filter(|&r| self.styler(r).is_some());
                    if let SeqKind::Alternatives(a) = &mut self.seq(id).volta {
                        a.repeat = repeat;
                    }
                }
            }
            Kind::Simultaneous(_) => {
                let heap = host.heap();
                let elements = m
                    .at(heap)
                    .and_then(|at| at.property(heap, self.keys.elements))
                    .map_or_else(Vec::new, |item| musics_of(heap, &item));
                let mut children = Vec::new();
                for e in elements.into_iter().flatten() {
                    children.push(self.create(host, e, Some(id))?);
                }
                self.it_mut(id).kind = Kind::Simultaneous(children);
            }
            Kind::Wrapper { .. } | Kind::Initial { .. } | Kind::Combine(_) => {
                let heap = host.heap();
                let element = m
                    .at(heap)
                    .and_then(|at| at.property(heap, self.keys.element))
                    .and_then(|item| music_of(heap, &item));
                if let Some(e) = element {
                    let made = self.create(host, e, Some(id))?;
                    match &mut self.it_mut(id).kind {
                        Kind::Wrapper { child, .. } | Kind::Initial { child } => {
                            *child = Some(made);
                        }
                        Kind::Combine(c) => c.lyric = Some(made),
                        _ => {}
                    }
                }
            }
            Kind::Base => {}
        }
        Ok(())
    }

    /// `Calculated_sequential_music::calc_elements`: what the music's
    /// `elements-callback` gives. `SequentialMusic`'s own gives its
    /// `elements`, which are read here.
    fn calc_elements(&mut self, host: &mut dyn Host, m: M) -> Run<Vec<Option<M>>> {
        let heap = host.heap();
        let Some(at) = m.at(heap) else {
            return Ok(Vec::new());
        };
        let k = &self.keys;
        let own = at.mutable(heap, k.elements_callback).is_none()
            && at
                .immutable_property(heap, k.name)
                .and_then(|n| heap.name(n))
                == Some("SequentialMusic");
        if own {
            return Ok(at
                .property(heap, k.elements)
                .map_or_else(Vec::new, |item| musics_of(heap, &item)));
        }
        let callback = self.value(heap, m, k.elements_callback);
        if !heap.is_procedure(callback) {
            diagnose(
                host,
                "programming-error",
                "calculated sequential music cannot find elements-callback",
            )?;
            return Ok(Vec::new());
        }
        let kept = match m {
            M::Value(v) => self.asked.list_at(at, Some(v)),
            M::Node(..) => self.asked.list_at(at, None),
        };
        if let Some(list) = kept {
            return Ok(musics_of(host.heap(), &Item::Value(list)));
        }
        let handle = m.handle(host.heap());
        let depth = host.heap().root_depth();
        host.heap().root(handle);
        let result = host.call(callback, &[handle]);
        host.heap().unroot_to(depth);
        let list = result?;
        // Rooted till the translation ends, as the iterators hold it.
        host.heap().root(list);
        Ok(musics_of(host.heap(), &Item::Value(list)))
    }

    // --- contexts --------------------------------------------------------

    /// `set_own_context`: the context it holds, a client of it.
    fn set_own(&mut self, id: IterId, c: Option<CtxId>) {
        let old = self.it(id).context;
        if old == c {
            return;
        }
        let mut a = self.t.arena.borrow_mut();
        if let Some(o) = old {
            let n = a.clients(o);
            a.set_clients(o, n.saturating_sub(1));
        }
        if let Some(n) = c {
            let k = a.clients(n);
            a.set_clients(n, k + 1);
        }
        drop(a);
        self.it_mut(id).context = c;
    }

    /// `get_context`: a wrapper's is its child's.
    pub fn get_context(&self, id: IterId) -> Option<CtxId> {
        match self.it(id).kind {
            Kind::Wrapper { child, .. } => child.and_then(|c| self.get_context(c)),
            _ => self.it(id).context,
        }
    }

    /// `set_context`: a wrapper's child's too.
    fn set_context(&mut self, id: IterId, c: Option<CtxId>) {
        if let Kind::Wrapper {
            child: Some(child), ..
        } = self.it(id).kind
        {
            self.set_context(child, c);
        }
        self.set_own(id, c);
    }

    /// `init_context`: its context `report`, then `create_contexts`.
    pub fn init_context(&mut self, host: &mut dyn Host, id: IterId, report: CtxId) -> Run<()> {
        if self.it(id).context.is_some() {
            return diagnose(
                host,
                "programming-error",
                "context already initialized; skipping",
            );
        }
        self.set_own(id, Some(report));
        self.create_contexts(host, id)
    }

    fn create_contexts(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let own = self.it(id).context;
        match &self.it(id).kind {
            Kind::Base => {}
            Kind::Simple { how, .. } => {
                if matches!(
                    how,
                    Simple::Event | Simple::Rhythmic | Simple::Chord | Simple::Fine
                ) {
                    self.descend_to_bottom(host, id)?;
                }
            }
            Kind::Sequential(s) => {
                let percent = matches!(s.volta, SeqKind::Percent(_));
                if percent {
                    self.percent_contexts(host, id)?;
                }
                if let (Some(child), Some(own)) = (self.seq(id).iter, own) {
                    self.init_context(host, child, own)?;
                    let reported = self.get_context(child);
                    self.descend_to_child(id, reported);
                }
                if percent {
                    self.descend_to_bottom(host, id)?;
                }
            }
            Kind::Simultaneous(children) => {
                let mut kept = Vec::new();
                for child in children.clone() {
                    if let Some(own) = own {
                        self.init_context(host, child, own)?;
                    }
                    // A sequence with no elements is done at once.
                    if self.ok(child) {
                        kept.push(child);
                    } else {
                        self.quit(host, child)?;
                    }
                }
                self.it_mut(id).kind = Kind::Simultaneous(kept);
                self.descend_to_user_accessible(host, id)?;
            }
            Kind::Wrapper { child, wrap } => {
                let child = *child;
                let tuplet = matches!(wrap, Wrap::Tuplet { .. });
                let quote = matches!(wrap, Wrap::Quote(_));
                if matches!(wrap, Wrap::Specced) {
                    self.find_specced_context(host, id)?;
                }
                if let (Some(child), Some(own)) = (child, self.it(id).context) {
                    self.init_context(host, child, own)?;
                }
                if tuplet {
                    self.tuplet_descend(id);
                }
                if quote {
                    self.quote_contexts(host, id)?;
                }
            }
            &Kind::Initial { child } => {
                if let (Some(child), Some(own)) = (child, own) {
                    self.init_context(host, child, own)?;
                    let reached = self.get_context(child);
                    self.set_own(id, reached);
                    self.quit(host, child)?;
                    self.it_mut(id).kind = Kind::Initial { child: None };
                }
            }
            Kind::Combine(c) => {
                let lyric = c.lyric;
                self.combined_contexts(host, id, lyric)?;
            }
        }
        Ok(())
    }

    /// `Context_specced_music_iterator::create_contexts`: the context its
    /// music names, found or made from its own, before its child's.
    fn find_specced_context(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let Some(own) = self.it(id).context else {
            return Ok(());
        };
        let m = self.it(id).music;
        let heap = host.heap();
        let k = &self.keys;
        let ty = self.value(heap, m, k.context_type);
        let ci = self.value(heap, m, k.context_id);
        let ci = heap
            .str(ci)
            .map(|s| String::from(s.as_str()))
            .unwrap_or_default();
        let ops = self.value(heap, m, k.property_operations);
        let dir = match self.value(heap, m, k.search_direction).as_fixnum() {
            Some(-1) => Dir::Down,
            Some(1) => Dir::Up,
            _ => Dir::Center,
        };
        let create_new = self.value(heap, m, k.create_new) == Value::TRUE;
        let t = self.t.clone();
        let found = if create_new {
            let found = create_unique_context(host, &t, own, dir, ty, &ci, ops)?;
            if found.is_none() {
                let text = alloc::format!(
                    "cannot create context: {}",
                    diagnostic_id(host.heap(), ty, &ci)
                );
                self.warning(host, m, &text)?;
            }
            found
        } else {
            let found = find_create_context(host, &t, own, dir, ty, &ci, ops)?;
            let what = match dir {
                _ if found.is_some() => None,
                Dir::Center => Some("cannot find or create context"),
                // LilyPond keeps quiet when DOWN fails.
                Dir::Down => None,
                Dir::Up => Some("cannot find context"),
            };
            if let Some(what) = what {
                let text = alloc::format!("{what}: {}", diagnostic_id(host.heap(), ty, &ci));
                self.warning(host, m, &text)?;
            }
            found
        };
        if found.is_some() {
            self.set_own(id, found);
        }
        Ok(())
    }

    /// `descend_to_bottom_context`.
    fn descend_to_bottom(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let Some(c) = self.get_context(id) else {
            return Ok(());
        };
        if !self.t.arena.borrow().is_bottom(c) {
            let t = self.t.clone();
            let bottom = get_default_interpreter(host, &t, c, "")?;
            self.set_context(id, Some(bottom));
        }
        Ok(())
    }

    /// `descend_to_child`: to `reported` if it is below its context.
    fn descend_to_child(&mut self, id: IterId, reported: Option<CtxId>) {
        let (Some(me), Some(child)) = (self.get_context(id), reported) else {
            return;
        };
        let a = self.t.arena.borrow();
        let mut at = Some(child);
        while let Some(c) = at
            && c != me
        {
            at = a.parent(c);
        }
        drop(a);
        if at == Some(me) {
            self.set_context(id, Some(child));
        }
    }

    /// `descend_to_user_accessible_context`: Global's Score for Global.
    fn descend_to_user_accessible(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let Some(c) = self.get_context(id) else {
            return Ok(());
        };
        if self.t.arena.borrow().parent(c).is_some() {
            return Ok(());
        }
        let t = self.t.clone();
        match get_user_accessible_interpreter(host, &t, c)? {
            Some(found) => self.set_context(id, Some(found)),
            None => diagnose(
                host,
                "programming-error",
                "cannot find an accessible context",
            )?,
        }
        Ok(())
    }

    // --- time --------------------------------------------------------------

    /// `pending_moment`: when it next has something to process.
    pub fn pending(&self, id: IterId) -> Moment {
        let it = self.it(id);
        match &it.kind {
            Kind::Base | Kind::Combine(_) => INFINITY,
            Kind::Simple { pending, .. } => *pending,
            Kind::Sequential(s) => {
                let Some(child) = s.iter else {
                    // Alive till the end if it ended early.
                    return if lt(s.iter_start, it.length) {
                        it.length
                    } else {
                        INFINITY
                    };
                };
                let c = self.it(child);
                let zero = sub(s.iter_start, c.start);
                let end = add(zero, c.length);
                let pend = add(zero, self.pending(child));
                min(pend, min(end, s.ahead_mom))
            }
            Kind::Simultaneous(children) => children
                .iter()
                .fold(INFINITY, |m, &c| min(m, self.pending(c))),
            Kind::Wrapper { child, wrap } => {
                let pending = child.map_or(INFINITY, |c| self.pending(c));
                match wrap {
                    Wrap::Tuplet { next, .. } if lt(pending, INFINITY) => min(pending, *next),
                    Wrap::Quote(q) => match q.next {
                        Some(next) if q.idx < q.end => min(pending, sub(next, q.zero)),
                        _ => pending,
                    },
                    Wrap::Grace { .. } if pending.main.is_finite() => Moment::new(
                        Rational::ZERO,
                        it.start
                            .grace
                            .checked_add(pending.main)
                            .unwrap_or(Rational::INFINITY),
                    ),
                    _ => pending,
                }
            }
            Kind::Initial { child } => {
                if child.is_some() {
                    ZERO
                } else {
                    INFINITY
                }
            }
        }
    }

    /// `run_always`.
    pub fn run_always(&self, id: IterId) -> bool {
        match &self.it(id).kind {
            Kind::Sequential(s) => s.iter.is_some_and(|c| self.run_always(c)),
            Kind::Simultaneous(children) => children.iter().any(|&c| self.run_always(c)),
            Kind::Wrapper { child, .. } => child.is_some_and(|c| self.run_always(c)),
            // Till its lyrics end or the voice can be removed.
            Kind::Combine(c) => {
                let a = self.t.arena.borrow();
                let music = c.n.and_then(|n| a.combines[n as usize].music);
                c.lyric.is_some_and(|l| self.ok(l)) && !music.is_some_and(|m| a.is_removable(m))
            }
            _ => false,
        }
    }

    /// `ok`: it has something left to process.
    pub fn ok(&self, id: IterId) -> bool {
        lt(self.pending(id), INFINITY) || self.run_always(id)
    }

    /// `music_get_length` and `music_start_mom`.
    pub fn length(&self, id: IterId) -> Moment {
        self.it(id).length
    }

    pub fn start(&self, id: IterId) -> Moment {
        self.it(id).start
    }

    // --- processing ------------------------------------------------------

    /// `process`: all it has at `until`, a moment of its own timeline.
    pub fn process(&mut self, host: &mut dyn Host, id: IterId, until: Moment) -> Run<()> {
        match &self.it(id).kind {
            Kind::Base | Kind::Initial { .. } => Ok(()),
            Kind::Combine(_) => self.process_combined(host, id),
            &Kind::Simple { how, pending } => {
                let (m, start, length) = (self.it(id).music, self.it(id).start, self.it(id).length);
                if lt(start, pending)
                    && matches!(
                        how,
                        Simple::Event | Simple::Rhythmic | Simple::Chord | Simple::Fine
                    )
                {
                    // Started already.
                } else {
                    match how {
                        Simple::Plain => {}
                        Simple::Set | Simple::Unset | Simple::Push | Simple::Pop => {
                            self.property(host, id, how)?;
                        }
                        Simple::Apply => self.apply_context(host, id)?,
                        Simple::Change => self.change(host, id)?,
                        Simple::Event => self.report_event(host, id, m)?,
                        Simple::Fine => self.fine(host, id, m)?,
                        Simple::Rhythmic => self.rhythmic(host, id, m)?,
                        Simple::Chord => {
                            let heap = host.heap();
                            let at = m.at(heap);
                            let list = |key| {
                                at.and_then(|at| at.property(heap, key))
                                    .map_or_else(Vec::new, |item| musics_of(heap, &item))
                            };
                            let mut all = list(self.keys.elements);
                            all.extend(list(self.keys.articulations));
                            for e in all.into_iter().flatten() {
                                self.report_event(host, id, e)?;
                            }
                        }
                    }
                }
                let next = if lt(until, length) { length } else { INFINITY };
                if let Kind::Simple { pending, .. } = &mut self.it_mut(id).kind {
                    *pending = next;
                }
                Ok(())
            }
            Kind::Sequential(s) => match s.volta {
                SeqKind::Plain | SeqKind::Percent(_) => self.process_sequential(host, id, until),
                SeqKind::Repeat { .. } => self.process_repeat(host, id, until),
                SeqKind::Alternatives(_) => self.process_alternatives(host, id, until),
            },
            Kind::Simultaneous(_) if self.it(id).part_combine => {
                self.process_part_combine(host, id, until)
            }
            Kind::Simultaneous(_) => self.process_simultaneous(host, id, until),
            Kind::Wrapper {
                wrap: Wrap::Grace { .. },
                ..
            } => self.process_grace(host, id, until),
            Kind::Wrapper {
                wrap: Wrap::Volta { .. },
                ..
            } => self.process_volta_specced(host, id, until),
            Kind::Wrapper {
                wrap: Wrap::Quote(_),
                ..
            } => self.process_quote(host, id, until),
            Kind::Wrapper { wrap, .. } => {
                let tuplet = matches!(wrap, Wrap::Tuplet { .. });
                if tuplet {
                    self.tuplet_split(host, id, until)?;
                }
                if let Kind::Wrapper { child: Some(c), .. } = self.it(id).kind {
                    self.process(host, c, until)?;
                }
                if tuplet {
                    self.tuplet_descend(id);
                }
                Ok(())
            }
        }
    }

    /// `Simultaneous_music_iterator::process`.
    fn process_simultaneous(&mut self, host: &mut dyn Host, id: IterId, until: Moment) -> Run<()> {
        let Kind::Simultaneous(children) = &self.it(id).kind else {
            return Ok(());
        };
        let finite = self.pending(id).main.is_finite();
        let mut kept = Vec::new();
        for child in children.clone() {
            if self.run_always(child) || self.pending(child) == until {
                self.process(host, child, until)?;
            }
            if self.ok(child) {
                kept.push(child);
            } else {
                self.quit(host, child)?;
            }
        }
        self.it_mut(id).kind = Kind::Simultaneous(kept);
        // Definite children all done: the rest go with them.
        if finite
            && !self.pending(id).main.is_finite()
            && let Kind::Simultaneous(rest) = &mut self.it_mut(id).kind
        {
            let rest = core::mem::take(rest);
            for child in rest {
                self.quit(host, child)?;
            }
        }
        Ok(())
    }

    /// `Part_combine_iterator::process` (part-combine-iterator.cc): the
    /// parts processed as `Simultaneous_music_iterator`'s, which may change
    /// their contexts (`ContextChange`s tagged `$partCombine`); then a
    /// multi-measure rest of no duration sent to each context a part left,
    /// which ends a rest begun there (`kill_mmrest`).
    fn process_part_combine(&mut self, host: &mut dyn Host, id: IterId, until: Moment) -> Run<()> {
        let children = |it: &Self| match &it.it(id).kind {
            Kind::Simultaneous(c) => c.clone(),
            _ => Vec::new(),
        };
        let before: Vec<CtxId> = children(self)
            .into_iter()
            .filter_map(|c| self.get_context(c))
            .collect();
        self.process_simultaneous(host, id, until)?;
        let after: Vec<CtxId> = children(self)
            .into_iter()
            .filter_map(|c| self.get_context(c))
            .collect();
        let t = self.t.clone();
        for c in before.into_iter().filter(|c| !after.contains(c)) {
            send(
                host,
                &t,
                c,
                "multi-measure-rest-event",
                &[("duration", Value::NULL)],
            )?;
        }
        Ok(())
    }

    fn seq(&mut self, id: IterId) -> &mut Seq {
        match &mut self.it_mut(id).kind {
            Kind::Sequential(s) => s,
            _ => unreachable!("a sequential iterator"),
        }
    }

    /// `Sequential_iterator::process`.
    fn process_sequential(&mut self, host: &mut dyn Host, id: IterId, until: Moment) -> Run<()> {
        while let Some(child) = self.seq(id).iter {
            let (iter_start, ahead) = (self.seq(id).iter_start, self.seq(id).ahead_mom);
            let zero = sub(iter_start, self.start(child));
            let end = add(zero, self.length(child));
            if self.ok(child) {
                // Time to advance the main part: finish what came before.
                let fast_forward = !lt(until, ahead);
                let proc_mom = if fast_forward { end } else { until };
                self.process(host, child, sub(proc_mom, zero))?;
                if self.ok(child) {
                    return Ok(());
                }
            }
            let next = min(end, ahead);
            if lt(until, next) && lt(next, INFINITY) {
                let m = self.it(child).music;
                return self.warning(host, m, "music is shorter than anticipated");
            }
            self.seq(id).iter_start = next;
            let reported = self.get_context(child);
            self.descend_to_child(id, reported);
            self.quit(host, child)?;
            self.seq(id).iter = None;
            self.pop_element(host, id)?;
            if let (Some(new), Some(own)) = (self.seq(id).iter, self.it(id).context) {
                self.init_context(host, new, own)?;
            }
            // `next_element`: an `\alternative`'s ends and starts.
            if matches!(self.seq(id).volta, SeqKind::Alternatives(_)) {
                self.next_alternative(host, id)?;
            }
            if matches!(self.seq(id).volta, SeqKind::Percent(_)) {
                self.next_percent(host, id)?;
            }
        }
        self.seq(id).iter_start = until;
        Ok(())
    }

    /// `look_ahead`: past the elements with no main length, then past the
    /// first with one.
    fn look_ahead(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        loop {
            let s = self.seq(id);
            let Some(&e) = s.elements.get(s.ahead) else {
                break;
            };
            s.ahead += 1;
            if let Some(e) = e {
                let end = self.time(host, e, false)?;
                if end.main.compare(Rational::ZERO).is_gt() {
                    let s = self.seq(id);
                    s.ahead_mom.main = s
                        .ahead_mom
                        .main
                        .checked_add(end.main)
                        .unwrap_or(Rational::INFINITY);
                    break;
                }
            }
        }
        self.ahead_grace(host, id)
    }

    /// `Music_sequence::first_start` of what is ahead: the grace part of
    /// the next element's moment.
    fn ahead_grace(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let rest: Vec<Option<M>> = {
            let s = self.seq(id);
            s.elements[s.ahead..].to_vec()
        };
        let mut grace = Rational::ZERO;
        for e in rest {
            let Some(e) = e else {
                diagnose(
                    host,
                    "programming-error",
                    "Music sequence should have music elements",
                )?;
                break;
            };
            let start = self.time(host, e, true)?;
            grace = grace.checked_add(start.grace).unwrap_or(grace);
            let length = self.time(host, e, false)?;
            if length != ZERO {
                break;
            }
        }
        self.seq(id).ahead_mom.grace = grace;
        Ok(())
    }

    /// `pop_element`: the next element taken and its iterator made.
    fn pop_element(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        self.seq(id).iter = None;
        let ready = |s: &mut Seq| s.remaining != s.ahead;
        if !ready(self.seq(id)) {
            self.look_ahead(host, id)?;
        }
        if ready(self.seq(id)) {
            let s = self.seq(id);
            let element = s.elements[s.remaining];
            s.remaining += 1;
            if s.remaining == s.elements.len() {
                s.ahead_mom = INFINITY;
            }
            if let Some(e) = element {
                let made = self.create(host, e, Some(id))?;
                self.seq(id).iter = Some(made);
            }
        }
        let length = self.length(id);
        if self.seq(id).iter.is_none() && self.seq(id).iter_start != length {
            let m = self.it(id).music;
            self.warning(
                host,
                m,
                "total length of sequential music elements is different than anticipated",
            )?;
        }
        Ok(())
    }

    /// `quit`: its children's, and its context let go; a tuplet's handler
    /// too, and lyrics that found no voice warned of.
    pub fn quit(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        self.set_handler(id, None);
        if let Kind::Combine(_) = self.it(id).kind {
            self.quit_combined(host, id)?;
        }
        let children: Vec<IterId> = match &self.it(id).kind {
            Kind::Sequential(s) => s.iter.into_iter().collect(),
            Kind::Simultaneous(children) => children.clone(),
            Kind::Wrapper { child, .. } | Kind::Initial { child } => {
                child.iter().copied().collect()
            }
            Kind::Combine(c) => c.lyric.into_iter().collect(),
            _ => Vec::new(),
        };
        for child in children {
            self.quit(host, child)?;
        }
        self.set_own(id, None);
        Ok(())
    }

    // --- property operations and \applyContext --------------------------

    /// `send_stream_event` of `class` by its context, at its music's
    /// origin.
    fn send_event(
        &mut self,
        host: &mut dyn Host,
        id: IterId,
        class: &str,
        props: &[(&str, Value)],
    ) -> Run<()> {
        let Some(c) = self.get_context(id) else {
            return Ok(());
        };
        let heap = host.heap();
        let origin = self.value(heap, self.it(id).music, self.keys.origin);
        let mut all = Vec::with_capacity(props.len() + 1);
        if heap.foreign::<Input>(origin).is_some() {
            all.push(("origin", origin));
        }
        all.extend_from_slice(props);
        let t = self.t.clone();
        send(host, &t, c, class, &all)
    }

    /// The `process` of the four property iterators (`property-iterator.cc`).
    fn property(&mut self, host: &mut dyn Host, id: IterId, how: Simple) -> Run<()> {
        let m = self.it(id).music;
        let heap = host.heap();
        let k = &self.keys;
        let [sym, once, value, prop, path, grob_value, pop_first] = [
            k.symbol,
            k.once,
            k.value,
            k.grob_property,
            k.grob_property_path,
            k.grob_value,
            k.pop_first,
        ]
        .map(|key| self.value(heap, m, key));
        match how {
            Simple::Set => {
                let props = [("symbol", sym), ("value", value), ("once", once)];
                return self.send_event(host, id, "SetProperty", &props);
            }
            Simple::Unset => {
                let props = [("symbol", sym), ("once", once)];
                return self.send_event(host, id, "UnsetProperty", &props);
            }
            _ => {}
        }
        if !self.check_grob(host, m, sym)? {
            return Ok(());
        }
        let heap = host.heap();
        // `get_property_path`: `grob-property` as a path of one.
        let path = if prop.tag() == Tag::Symbol {
            heap.list(&[prop])
        } else {
            path
        };
        let depth = heap.root_depth();
        heap.root(path);
        let ran = if how == Simple::Push {
            let mut ran = Ok(());
            if pop_first == Value::TRUE && once != Value::TRUE {
                let props = [("symbol", sym), ("property-path", path)];
                ran = self.send_event(host, id, "Revert", &props);
            }
            ran.and_then(|()| {
                let props = [
                    ("symbol", sym),
                    ("property-path", path),
                    ("once", once),
                    ("value", grob_value),
                ];
                self.send_event(host, id, "Override", &props)
            })
        } else {
            let props = [("symbol", sym), ("once", once), ("property-path", path)];
            self.send_event(host, id, "Revert", &props)
        };
        host.heap().unroot_to(depth);
        ran
    }

    /// `check_grob`: whether `sym` names a grob (its `is-grob?` object
    /// property), or a warning at the music.
    fn check_grob(&mut self, host: &mut dyn Host, m: M, sym: Value) -> Run<bool> {
        let is = match host.lily("object-property") {
            Some(proc) => {
                let key = host.heap().intern("is-grob?");
                host.call(proc, &[sym, key])?
            }
            None => Value::FALSE,
        };
        // `from_scm<bool>`: only `#t` is true.
        if is == Value::TRUE {
            return Ok(true);
        }
        let mut name = String::new();
        host.heap().display(sym, &mut name);
        self.warning(host, m, &alloc::format!("not a grob name, `{name}'"))?;
        Ok(false)
    }

    /// `Apply_context_iterator::process`: its procedure called with its
    /// context, at its origin (`with_location`).
    fn apply_context(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let m = self.it(id).music;
        let heap = host.heap();
        let proc = self.value(heap, m, self.keys.procedure);
        if !heap.is_procedure(proc) {
            return self.warning(host, m, "\\applycontext argument is not a procedure");
        }
        let Some(c) = self.get_context(id) else {
            return Ok(());
        };
        let origin = self.value(heap, m, self.keys.origin);
        // N8's partitions trust LilyPond's own procedures to touch only
        // their context and the shared level (`partition.rs`).
        if heap.primitive_name(proc).is_none()
            && !heap.is_image_code(proc)
            && let Some(r) = &mut self.t.arena.borrow_mut().n8
        {
            r.parts.applied(c);
        }
        let handle = self.t.arena.borrow().handle(c);
        let t = self.t.clone();
        let old = host.set_location(origin);
        let ran = call(host, &t, proc, &[handle]);
        host.set_location(old);
        ran.map(|_| ())
    }

    // --- \change ------------------------------------------------------------

    /// `Change_iterator::process`: the context of its music's type above
    /// its own, the one of that type and id near it, and each iterator of
    /// its scope changed from the first to the second.
    fn change(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let m = self.it(id).music;
        let heap = host.heap();
        let k = &self.keys;
        let to_type = self.value(heap, m, k.change_to_type);
        let to_id = self.value(heap, m, k.change_to_id);
        let to_id = heap
            .str(to_id)
            .map(|s| String::from(s.as_str()))
            .unwrap_or_default();
        let tag = self.value(heap, m, k.change_tag);
        let Some(here) = self.get_context(id) else {
            return Ok(());
        };
        let (last, dest) = {
            let a = self.t.arena.borrow();
            let last = a.find_context(heap, here, Dir::Up, to_type, "");
            let dest = last.and_then(|_| a.find_context(heap, here, Dir::Center, to_type, &to_id));
            (last, dest)
        };
        let (Some(last), Some(dest)) = (last, dest) else {
            let (what, to_id) = match last {
                None => ("from", ""),
                Some(_) => ("to", to_id.as_str()),
            };
            let text = alloc::format!(
                "cannot find context to change {what}: {}",
                diagnostic_id(host.heap(), to_type, to_id)
            );
            return self.warning(host, m, &text);
        };
        let scope = self.where_tagged(host.heap(), id, tag);
        let mut walk = Vec::new();
        self.preorder_walk(scope, &mut walk);
        for it in walk {
            self.change_one(host, it, m, last, dest)?;
        }
        Ok(())
    }

    /// `Change_iterator::change` of the iterator `it`: its own context if
    /// that is `source` (`substitute_context`), or else the branch of its
    /// context's ancestry whose parent is `source` moved to `target` by a
    /// `ChangeParent` event at the music `m`'s origin, unless it is below
    /// `target` already.
    fn change_one(
        &mut self,
        host: &mut dyn Host,
        it: IterId,
        m: M,
        source: CtxId,
        target: CtxId,
    ) -> Run<()> {
        let Some(c) = self.get_context(it) else {
            return Ok(());
        };
        if c == source {
            if source != target {
                if self.it(it).context == Some(source) {
                    self.set_own(it, Some(target));
                }
                if let Kind::Combine(Combined { n: Some(n), .. }) = self.it(it).kind {
                    self.substitute_combined(host.heap(), n, source, target);
                }
            }
            return Ok(());
        }
        let branch = {
            let a = self.t.arena.borrow();
            let mut branch = c;
            loop {
                match a.parent(branch) {
                    Some(p) if p == target => break None,
                    Some(p) if p == source => break Some(branch),
                    Some(p) => branch = p,
                    None => break None,
                }
            }
        };
        let Some(branch) = branch else {
            return Ok(());
        };
        let heap = host.heap();
        let origin = self.value(heap, m, self.keys.origin);
        let to = self.t.arena.borrow().handle(target);
        let mut props = Vec::with_capacity(2);
        if heap.foreign::<Input>(origin).is_some() {
            props.push(("origin", origin));
        }
        props.push(("context", to));
        let t = self.t.clone();
        send(host, &t, branch, "ChangeParent", &props)
    }

    /// `where_tagged`: the first of `id` and its ancestors whose music's
    /// `tags`, the nearest set, hold `tag`, or `id` itself.
    fn where_tagged(&self, heap: &Heap, id: IterId, tag: Value) -> IterId {
        if tag.tag() != Tag::Symbol {
            return id;
        }
        let mut candidate = Some(id);
        while let Some(c) = candidate {
            // `where_defined` of `tags`.
            let mut at = Some(c);
            let mut tags = Value::NULL;
            while let Some(i) = at {
                tags = self.value(heap, self.it(i).music, self.keys.tags);
                if tags != Value::NULL {
                    break;
                }
                at = self.it(i).parent;
            }
            let Some(found) = at else {
                break;
            };
            let mut p = tags;
            while let (Some(t), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                if t == tag {
                    return found;
                }
                p = rest;
            }
            candidate = self.it(found).parent;
        }
        id
    }

    /// `preorder_walk`: `id` and the iterators below it that are alive.
    fn preorder_walk(&self, id: IterId, out: &mut Vec<IterId>) {
        out.push(id);
        match &self.it(id).kind {
            Kind::Sequential(s) => {
                if let Some(c) = s.iter {
                    self.preorder_walk(c, out);
                }
            }
            Kind::Simultaneous(children) => {
                for &c in children {
                    self.preorder_walk(c, out);
                }
            }
            Kind::Wrapper { child, .. } | Kind::Initial { child } => {
                if let Some(c) = *child {
                    self.preorder_walk(c, out);
                }
            }
            Kind::Combine(c) => {
                if let Some(l) = c.lyric {
                    self.preorder_walk(l, out);
                }
            }
            _ => {}
        }
    }

    // --- grace notes ----------------------------------------------------------

    /// `Grace_iterator::process`: `GraceChange` sent by the child's context
    /// when `until` goes into or out of grace time, the child processed at
    /// the grace part as its main time, and the context descended to the
    /// child's.
    fn process_grace(&mut self, host: &mut dyn Host, id: IterId, until: Moment) -> Run<()> {
        let Kind::Wrapper {
            child,
            wrap: Wrap::Grace { in_grace },
        } = self.it(id).kind
        else {
            unreachable!("a grace iterator");
        };
        let now_in_grace = !until.grace.is_zero();
        if in_grace != now_in_grace && child.is_some_and(|c| self.get_context(c).is_some()) {
            self.send_event(host, id, "GraceChange", &[])?;
        }
        if let Kind::Wrapper {
            wrap: Wrap::Grace { in_grace },
            ..
        } = &mut self.it_mut(id).kind
        {
            *in_grace = now_in_grace;
        }
        let main = until
            .grace
            .checked_sub(self.it(id).start.grace)
            .unwrap_or(Rational::INFINITY);
        if let Some(c) = child {
            self.process(host, c, Moment::new(main, Rational::ZERO))?;
            let reported = self.get_context(c);
            self.descend_to_child(id, reported);
        }
        Ok(())
    }

    // --- volta repeats ------------------------------------------------------

    /// The styler of the volta repeat iterator `id`.
    fn styler(&mut self, id: IterId) -> Option<&mut Styler> {
        match &mut self.it_mut(id).kind {
            Kind::Sequential(Seq {
                volta: SeqKind::Repeat { styler, .. },
                ..
            }) => Some(styler),
            _ => None,
        }
    }

    fn alternatives(&mut self, id: IterId) -> &mut Alternatives {
        match &mut self.seq(id).volta {
            SeqKind::Alternatives(a) => a,
            _ => unreachable!("an alternative sequence iterator"),
        }
    }

    /// `from_scm (value, 1)` of an integer.
    fn count(&self, heap: &Heap, v: Value) -> i64 {
        heap.to_i64(v).unwrap_or(1)
    }

    /// `Volta_repeat_iterator::process`: the repeat's start reported the
    /// first time, its return once it has ended, unless its alternatives
    /// reported one.
    fn process_repeat(&mut self, host: &mut dyn Host, id: IterId, until: Moment) -> Run<()> {
        let (start_mom, length) = (self.start(id), self.length(id));
        let empty = length == ZERO && start_mom.grace.is_zero();
        let heap = host.heap();
        let count = self.value(heap, self.it(id).music, self.keys.repeat_count);
        let count = self.count(heap, count);
        let SeqKind::Repeat { started, .. } = self.seq(id).volta else {
            unreachable!("a volta repeat iterator");
        };
        if !started {
            if !empty {
                let now = self.t.arena.borrow().now();
                let end = add(now, sub(length, start_mom));
                if let Some(s) = self.styler(id) {
                    s.spanned = (now, end);
                    s.repeat_count = count;
                }
                self.report_start(host, id)?;
            }
            if let SeqKind::Repeat { started, .. } = &mut self.seq(id).volta {
                *started = true;
            }
        }
        self.process_sequential(host, id, until)?;
        let SeqKind::Repeat {
            started, stopped, ..
        } = self.seq(id).volta
        else {
            unreachable!("a volta repeat iterator");
        };
        if started && !stopped && until == length {
            let reported = self.styler(id).is_some_and(|s| s.reported_return);
            if !empty && !reported {
                // No return for the final volta.
                self.report_return(host, id, 0, count - 1)?;
            }
            if let SeqKind::Repeat { stopped, .. } = &mut self.seq(id).volta {
                *stopped = true;
            }
        }
        Ok(())
    }

    /// `Alternative_sequence_iterator::process`: the first time, how the
    /// alternatives are presented, the properties to restore saved and the
    /// first alternative's start reported.
    fn process_alternatives(&mut self, host: &mut dyn Host, id: IterId, until: Moment) -> Run<()> {
        if self.alternatives(id).first_time {
            self.alternatives(id).first_time = false;
            self.analyze(host, id)?;
            if self.alternatives(id).returns.len() > 1 && self.timing(host, id) {
                self.save_restores(host, id);
            }
            self.start_alternative(host, id)?;
        }
        self.process_sequential(host, id, until)
    }

    /// `Alternative_sequence_iterator::analyze`: whether the alternatives
    /// start and end with the repeat, each one's returns, whether they are
    /// in order; then the styler's brackets and their depth.
    fn analyze(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let repeat = self.alternatives(id).repeat;
        let spanned = repeat
            .and_then(|r| self.styler(r))
            .map_or((INFINITY, INFINITY), |s| s.spanned);
        let now = self.t.arena.borrow().now();
        let end = add(now, sub(self.length(id), self.start(id)));
        let start_dir = if now == spanned.0 { START } else { CENTER };
        let end_dir = if end == spanned.1 { STOP } else { CENTER };
        let heap = host.heap();
        let count = self.inherited(heap, id, self.keys.repeat_count);
        let count = self.count(heap, count);
        let m = self.it(id).music;
        let alts = m
            .at(heap)
            .and_then(|at| at.property(heap, self.keys.elements))
            .map_or_else(Vec::new, |item| musics_of(heap, &item));
        let mut in_order = true;
        let mut next = 1;
        let mut returns = Vec::new();
        for alt in alts.into_iter().flatten() {
            let heap = host.heap();
            let nums = self.value(heap, alt, self.keys.volta_numbers);
            let mut ret = 0;
            if nums.tag() != Tag::Pair {
                returns.push(ret);
                self.warning(
                    host,
                    alt,
                    "missing volta specification on alternative element",
                )?;
                in_order = false;
                continue;
            }
            let mut sorted = Vec::new();
            let mut p = nums;
            while let (Some(n), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                sorted.push(heap.to_i64(n).unwrap_or(0));
                p = rest;
            }
            sorted.sort_unstable();
            for num in sorted {
                // Tail alternatives return after each volta but the last.
                if end_dir == STOP && num < count {
                    ret += 1;
                }
                if num == next {
                    next += 1;
                } else {
                    in_order = false;
                }
            }
            returns.push(ret);
        }
        if in_order && next == count + 1 && returns.len() == 1 {
            // One alternative for all voltas: a bracket, not a coda mark.
            in_order = false;
        }
        self.alternatives(id).returns = returns;
        let brackets = match repeat {
            Some(r) => self.group_start(r, start_dir, end_dir, in_order),
            None => false,
        };
        let mut depth = usize::from(brackets);
        let mut at = self.it(id).parent;
        while let Some(p) = at {
            if let Kind::Sequential(Seq {
                volta: SeqKind::Alternatives(a),
                ..
            }) = &self.it(p).kind
            {
                depth += a.depth;
                break;
            }
            at = self.it(p).parent;
        }
        let a = self.alternatives(id);
        a.brackets = brackets;
        a.depth = depth;
        Ok(())
    }

    /// `timing` seen from its context: true only if `#t`.
    fn timing(&self, host: &mut dyn Host, id: IterId) -> bool {
        let Some(c) = self.get_context(id) else {
            return false;
        };
        let sym = host.heap().intern("timing");
        self.t.arena.borrow().get(c, sym) == Value::TRUE
    }

    /// `save_context_properties`: each of `alternativeRestores` where it
    /// is defined, with its value, the last first.
    fn save_restores(&mut self, host: &mut dyn Host, id: IterId) {
        let Some(c) = self.get_context(id) else {
            return;
        };
        let heap = host.heap();
        let key = heap.intern("alternativeRestores");
        let mut syms = self.t.arena.borrow().get(c, key);
        let mut saved = Vec::new();
        while let (Some(sym), Some(rest)) = (heap.car(syms), heap.cdr(syms)) {
            if let Some((at, v)) = self.t.arena.borrow().where_defined(c, sym) {
                // Rooted till the translation ends, as the iterator holds it.
                heap.root(v);
                saved.push((at, sym, v));
            }
            syms = rest;
        }
        saved.reverse();
        self.alternatives(id).restores = saved;
    }

    /// `restore_context_properties`: each saved value set again, but
    /// `measurePosition` at this alternative's grace time.
    fn restore(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let saved = self.alternatives(id).restores.clone();
        let now = self.t.arena.borrow().now();
        let t = self.t.clone();
        for (c, sym, v) in saved {
            let heap = host.heap();
            let v = match heap.foreign::<Moment>(v) {
                Some(mp) if heap.name(sym) == Some("measurePosition") => {
                    heap.wrap(Moment::new(mp.main, now.grace))
                }
                _ => v,
            };
            let depth = heap.root_depth();
            heap.root(v);
            let ran = crate::context::set_property(host, &t, c, sym, v);
            host.heap().unroot_to(depth);
            ran?;
        }
        Ok(())
    }

    /// `start_alternative`: the current alternative's start reported.
    fn start_alternative(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let a = self.alternatives(id);
        let (done, total, repeat, depth) = (a.done, a.returns.len(), a.repeat, a.depth);
        if done >= total {
            return Ok(());
        }
        let child = self.seq(id).iter.map(|c| self.it(c).music);
        let heap = host.heap();
        let nums = child.map_or(Value::NULL, |m| {
            self.value(heap, m, self.keys.volta_numbers)
        });
        let nums = if nums.tag() == Tag::Pair {
            nums
        } else {
            Value::NULL
        };
        let Some(r) = repeat else {
            return Ok(());
        };
        let (style, count, alt_depth) = self.styler_state(r);
        let alt_num = done as i64 + 1;
        let dir = if alt_num == 1 { START } else { CENTER };
        match style {
            Style::Volta if count >= 1 => self.alternative_event(host, r, child, dir, depth, nums),
            Style::Segno { coda_marks } if count >= 2 => {
                if coda_marks && alt_depth < 2 {
                    let empty = match child {
                        Some(m) => {
                            let length = self.time(host, m, false)?;
                            let start = self.time(host, m, true)?;
                            length == ZERO && start.grace.is_zero()
                        }
                        None => true,
                    };
                    if !empty && let Some(m) = child {
                        return self.mark(host, r, m, "CodaMarkEvent");
                    }
                    Ok(())
                } else {
                    self.alternative_event(host, r, child, dir, depth, nums)
                }
            }
            _ => Ok(()),
        }
    }

    /// `Alternative_sequence_iterator::next_element`, after the next
    /// alternative is taken: the one done ended (`end_alternative`) and the
    /// next started.
    fn next_alternative(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        self.alternatives(id).done += 1;
        let a = self.alternatives(id);
        let (done, total, repeat, depth) = (a.done, a.returns.len(), a.repeat, a.depth);
        if done <= total {
            let returns = self.alternatives(id).returns[done - 1];
            if let Some(r) = repeat {
                if returns > 0 {
                    self.report_return(host, r, done as i64, returns)?;
                }
                if done == total {
                    let m = self.it(id).music;
                    self.group_end(host, r, m, depth)?;
                }
            }
            if done < total && self.timing(host, id) {
                self.restore(host, id)?;
            }
        }
        self.start_alternative(host, id)
    }

    fn styler_state(&mut self, r: IterId) -> (Style, i64, usize) {
        self.styler(r).map_or((Style::Null, 2, 0), |s| {
            (s.style, s.repeat_count, s.alternative_depth)
        })
    }

    /// `report_start`: a volta repeat's `VoltaRepeatStartEvent`, a segno
    /// repeat's segno mark.
    fn report_start(&mut self, host: &mut dyn Host, r: IterId) -> Run<()> {
        let (style, count, _) = self.styler_state(r);
        let m = self.it(r).music;
        match style {
            Style::Volta => {
                let ev = self.make(host, Some(m), "VoltaRepeatStartEvent")?;
                if count > 0 {
                    let heap = host.heap();
                    let v = heap.integer(count);
                    crate::music::set_property(heap, ev, self.keys.repeat_count, v);
                }
                self.send_made(host, r, ev)
            }
            Style::Segno { .. } if count >= 2 => self.mark(host, r, m, "SegnoMarkEvent"),
            _ => Ok(()),
        }
    }

    /// `report_alternative_group_start`: whether its alternatives get
    /// volta brackets.
    fn group_start(&mut self, r: IterId, start: i64, end: i64, in_order: bool) -> bool {
        let Some(s) = self.styler(r) else {
            return false;
        };
        s.alternative_depth += 1;
        match s.style {
            Style::Null => false,
            Style::Volta => s.repeat_count >= 1,
            Style::Segno { .. } if s.repeat_count < 2 => false,
            Style::Segno { mut coda_marks } => {
                if s.alternative_depth == 1 {
                    coda_marks = start != START && end == STOP && in_order;
                    s.style = Style::Segno { coda_marks };
                }
                // Nested alternatives always get brackets.
                !(coda_marks && s.alternative_depth < 2)
            }
        }
    }

    /// `report_return`: the deepest of nested end-aligned groups reports,
    /// a volta repeat's `VoltaRepeatEndEvent` or a segno repeat's
    /// `DalSegnoEvent`.
    fn report_return(&mut self, host: &mut dyn Host, r: IterId, alt: i64, returns: i64) -> Run<()> {
        let Some(s) = self.styler(r) else {
            return Ok(());
        };
        if s.alternative_depth < s.reported_return_depth {
            return Ok(());
        }
        s.reported_return_depth = s.alternative_depth;
        s.reported_return = true;
        let (style, count, depth) = (s.style, s.repeat_count, s.alternative_depth);
        match style {
            Style::Null => Ok(()),
            Style::Volta => {
                let reps = if alt < 1 { count } else { 0 };
                self.end_event(host, r, "VoltaRepeatEndEvent", alt, reps, returns)
            }
            Style::Segno { .. } if count < 2 => Ok(()),
            Style::Segno { coda_marks } => {
                // Volta brackets: no redundant details in the D.S.
                let (alt, reps, returns) = if coda_marks && depth < 2 {
                    (alt, count, returns)
                } else {
                    (-1, -1, -1)
                };
                self.end_event(host, r, "DalSegnoEvent", alt, reps, returns)
            }
        }
    }

    /// `report_alternative_group_end`: the last alternative's bracket
    /// stopped.
    fn group_end(&mut self, host: &mut dyn Host, r: IterId, m: M, depth: usize) -> Run<()> {
        let (style, count, alt_depth) = self.styler_state(r);
        let ran = match style {
            Style::Volta if count >= 1 => {
                self.alternative_event(host, r, Some(m), STOP, depth, Value::NULL)
            }
            Style::Segno { coda_marks } if count >= 2 && !(coda_marks && alt_depth < 2) => {
                self.alternative_event(host, r, Some(m), STOP, depth, Value::NULL)
            }
            _ => Ok(()),
        };
        if let Some(s) = self.styler(r)
            && s.alternative_depth > 0
        {
            s.alternative_depth -= 1;
            if s.alternative_depth == 0 {
                s.reported_return_depth = 0;
            }
        }
        ran
    }

    /// `report_alternative_event`: an `AlternativeEvent` at `element`'s
    /// origin.
    fn alternative_event(
        &mut self,
        host: &mut dyn Host,
        r: IterId,
        element: Option<M>,
        dir: i64,
        depth: usize,
        nums: Value,
    ) -> Run<()> {
        let ev = self.make(host, element, "AlternativeEvent")?;
        let heap = host.heap();
        let props = [
            ("alternative-dir", Value::fixnum(dir).unwrap_or(Value::NULL)),
            ("volta-depth", heap.integer(depth as i64)),
            ("volta-numbers", nums),
        ];
        for (key, v) in props {
            let key = heap.intern(key);
            crate::music::set_property(heap, ev, key, v);
        }
        self.send_made(host, r, ev)
    }

    /// `report_end_event`: `name` at the repeat's origin, with what it
    /// carries.
    fn end_event(
        &mut self,
        host: &mut dyn Host,
        r: IterId,
        name: &str,
        alt: i64,
        reps: i64,
        returns: i64,
    ) -> Run<()> {
        let m = self.it(r).music;
        let left = self.styler(r).map_or(INFINITY, |s| s.spanned.0);
        let ev = self.make(host, Some(m), name)?;
        let heap = host.heap();
        let mut props = Vec::new();
        if alt > 0 {
            props.push(("alternative-number", heap.integer(alt)));
        }
        if reps > 0 {
            props.push(("repeat-count", heap.integer(reps)));
        }
        if returns >= 0 {
            props.push(("return-count", heap.integer(returns)));
        }
        props.push(("repeat-body-start-moment", heap.wrap(left)));
        for (key, v) in props {
            let key = heap.intern(key);
            crate::music::set_property(heap, ev, key, v);
        }
        self.send_made(host, r, ev)
    }

    /// `Segno_repeat_styler::report_mark`: `name` at `m`'s origin.
    fn mark(&mut self, host: &mut dyn Host, r: IterId, m: M, name: &str) -> Run<()> {
        let ev = self.make(host, Some(m), name)?;
        self.send_made(host, r, ev)
    }

    /// `make-music` of `name`, at the origin of `at` if it has one.
    fn make(&mut self, host: &mut dyn Host, at: Option<M>, name: &str) -> Run<Value> {
        let Some(make) = host.lily("make-music") else {
            return Ok(Value::FALSE);
        };
        let sym = host.heap().intern(name);
        let ev = host.call(make, &[sym])?;
        let heap = host.heap();
        if let Some(at) = at {
            let origin = self.value(heap, at, self.keys.origin);
            if heap.foreign::<Input>(origin).is_some() {
                crate::music::set_property(heap, ev, self.keys.origin, origin);
            }
        }
        Ok(ev)
    }

    /// `send_to_context` of `ev`, made by [`Iteration::make`], by the
    /// context of the repeat iterator `r`.
    fn send_made(&mut self, host: &mut dyn Host, r: IterId, ev: Value) -> Run<()> {
        match self.get_context(r) {
            Some(c) if ev != Value::FALSE => self.send_to(host, c, ev),
            _ => Ok(()),
        }
    }

    /// `Music::send_to_context`: `ev`'s event broadcast by `c`'s event
    /// source.
    fn send_to(&mut self, host: &mut dyn Host, c: CtxId, ev: Value) -> Run<()> {
        let depth = host.heap().root_depth();
        host.heap().root(ev);
        let ran = self.event_of(host, M::Value(ev)).and_then(|ev| {
            let d = self.t.arena.borrow().event_source(c);
            let t = self.t.clone();
            dispatch(host, &t, d, ev)
        });
        host.heap().unroot_to(depth);
        ran
    }

    /// `Volta_specced_music_iterator::process`: under an `\alternative`
    /// with brackets, a `VoltaSpanEvent` started by its context the first
    /// time and stopped by it once its music has ended.
    fn process_volta_specced(&mut self, host: &mut dyn Host, id: IterId, until: Moment) -> Run<()> {
        let Kind::Wrapper {
            child,
            wrap:
                Wrap::Volta {
                    mut started,
                    mut stopped,
                    mut depth,
                    ..
                },
        } = self.it(id).kind
        else {
            unreachable!("a volta-specced music iterator");
        };
        if !started {
            // Brackets for the children of an `\alternative` only.
            match self.it(id).parent.map(|p| &self.it(p).kind) {
                Some(Kind::Sequential(Seq {
                    volta: SeqKind::Alternatives(a),
                    ..
                })) => {
                    depth = a.depth;
                    stopped = !a.brackets;
                }
                _ => stopped = true,
            }
            started = stopped;
        }
        if !started {
            started = true;
            let c = self.get_context(id);
            self.set_handler(id, c);
            self.set_volta(id, started, stopped, depth);
            self.volta_span(host, id, START)?;
        }
        self.set_volta(id, started, stopped, depth);
        if let Some(c) = child {
            self.process(host, c, until)?;
        }
        if started && !stopped && until == self.length(id) {
            self.set_volta(id, started, true, depth);
            self.volta_span(host, id, STOP)?;
            self.set_handler(id, None);
        }
        Ok(())
    }

    fn set_volta(&mut self, id: IterId, now_started: bool, now_stopped: bool, now_depth: usize) {
        if let Kind::Wrapper {
            wrap:
                Wrap::Volta {
                    started,
                    stopped,
                    depth,
                    ..
                },
            ..
        } = &mut self.it_mut(id).kind
        {
            (*started, *stopped, *depth) = (now_started, now_stopped, now_depth);
        }
    }

    /// `create_event`: a `VoltaSpanEvent` of direction `d` at the music's
    /// origin, sent to the handler.
    fn volta_span(&mut self, host: &mut dyn Host, id: IterId, d: i64) -> Run<()> {
        let Kind::Wrapper {
            wrap:
                Wrap::Volta {
                    handler: Some(h),
                    depth,
                    ..
                },
            ..
        } = self.it(id).kind
        else {
            return Ok(());
        };
        let Some(make) = host.lily("make-span-event") else {
            return Ok(());
        };
        let heap = host.heap();
        let class = heap.intern("VoltaSpanEvent");
        let dir = Value::fixnum(d).unwrap_or(Value::NULL);
        let ev = host.call(make, &[class, dir])?;
        let heap = host.heap();
        let depth_value = heap.integer(depth as i64);
        let m = self.it(id).music;
        let k = &self.keys;
        let props = [
            (k.origin, self.value(heap, m, k.origin)),
            (k.repeat_count, self.inherited(heap, id, k.repeat_count)),
            (heap.intern("volta-depth"), depth_value),
            (k.volta_numbers, self.value(heap, m, k.volta_numbers)),
        ];
        for (key, v) in props {
            crate::music::set_property(heap, ev, key, v);
        }
        self.send_to(host, h, ev)
    }

    // --- lyrics combined with a voice ------------------------------------

    fn combined(&mut self, id: IterId) -> &mut Combined {
        match &mut self.it_mut(id).kind {
            Kind::Combine(c) => c,
            _ => unreachable!("a lyric combine music iterator"),
        }
    }

    /// `Lyric_combine_music_iterator::create_contexts`: its lyrics'
    /// iterator in its context, the `Lyrics` context there or below, the
    /// voice it follows, and its listener of each context made.
    fn combined_contexts(
        &mut self,
        host: &mut dyn Host,
        id: IterId,
        lyric: Option<IterId>,
    ) -> Run<()> {
        let Some(lyric) = lyric else {
            return Ok(());
        };
        if let Some(own) = self.it(id).context {
            self.init_context(host, lyric, own)?;
        }
        let m = self.it(id).music;
        let heap = host.heap();
        let name = heap.intern("Lyrics");
        let lyrics = self.get_context(lyric).and_then(|c| {
            self.t
                .arena
                .borrow()
                .find_context(heap, c, Dir::Down, name, "")
        });
        if lyrics.is_none() {
            let element = m
                .at(heap)
                .and_then(|at| at.property(heap, self.keys.element))
                .and_then(|item| music_of(heap, &item));
            if let Some(e) = element {
                let text = "argument of \\lyricsto should contain Lyrics context";
                self.warning(host, e, text)?;
            }
        }
        let heap = host.heap();
        let voice = self.value(heap, m, self.keys.associated_context);
        let mut kind = self.value(heap, m, self.keys.associated_context_type);
        if kind.tag() != Tag::Symbol {
            kind = heap.intern("Voice");
        }
        let mut combine = Combine::new(lyrics, voice, kind);
        combine.lyric_ok = self.ok(lyric);
        let mut a = self.t.arena.borrow_mut();
        let n = a.combines.len() as u32;
        a.combines.push(combine);
        lyrics::follow_voice(heap, &mut a, n);
        // A voice made later, implicitly too, is found when it is made, so
        // that its first note gets its syllable.
        let below = a.events_below(GLOBAL);
        let listener = Listener::Combine(n, Heard::NewContext);
        a.add_listener(heap, below, listener, "CreateContext");
        drop(a);
        self.combined(id).n = Some(n);
        Ok(())
    }

    /// `Lyric_combine_music_iterator::process`: the voice it follows found
    /// again, and its lyrics' next syllable processed when the voice starts
    /// a note now, unless that is a grace note and the lyrics take none
    /// (`includeGraceNotes`), whose syllable then waits for its main note.
    fn process_combined(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let Some(n) = self.combined(id).n else {
            return Ok(());
        };
        let t = self.t.clone();
        lyrics::follow_voice(host.heap(), &mut t.arena.borrow_mut(), n);
        self.combined(id).lyrics_found = true;
        let Some(music) = t.arena.borrow().combines[n as usize].music else {
            return Ok(());
        };
        let heap = host.heap();
        let associated = heap.intern("associatedVoiceContext");
        if t.arena.borrow().parent(music).is_none() {
            // The melody has died; the lyrics die too.
            let lyrics = t.arena.borrow().combines[n as usize].lyrics;
            if let Some(l) = lyrics {
                crate::context::unset_property(host, &t, l, associated)?;
            }
            if let Some(l) = self.combined(id).lyric.take() {
                self.quit(host, l)?;
            }
            let mut a = t.arena.borrow_mut();
            a.combines[n as usize].lyric_ok = false;
            lyrics::set_music_context(host.heap(), &mut a, n, None);
        }
        let (music, lyrics, start, busy, now) = {
            let a = t.arena.borrow();
            let c = &a.combines[n as usize];
            let start = lyrics::start_new_syllable(host.heap(), &a, n);
            (c.music, c.lyrics, start, c.busy, a.now())
        };
        let Combined {
            lyric,
            pending_grace,
            ..
        } = *self.combined(id);
        if let (Some(music), Some(lyric)) = (music, lyric)
            && (start || !busy.compare(pending_grace).is_lt())
            && self.ok(lyric)
        {
            let heap = host.heap();
            let graces = heap.intern("includeGraceNotes");
            let graces = lyrics.is_some_and(|l| t.arena.borrow().get(l, graces) == Value::TRUE);
            if !now.grace.is_zero() && !graces {
                self.combined(id).pending_grace = Moment::new(now.main, Rational::ZERO);
                return Ok(());
            }
            self.combined(id).pending_grace.main = Rational::INFINITY;
            let m = self.pending(lyric);
            if let Some(l) = lyrics {
                let voice = t.arena.borrow().handle(music);
                crate::context::set_property(host, &t, l, associated, voice)?;
            }
            self.process(host, lyric, m)?;
            let ok = self.ok(lyric);
            t.arena.borrow_mut().combines[n as usize].lyric_ok = ok;
            self.combined(id).music_found = true;
        }
        lyrics::follow_voice(host.heap(), &mut t.arena.borrow_mut(), n);
        Ok(())
    }

    /// `Lyric_combine_music_iterator::do_quit`: LilyPond's warning if it
    /// had lyrics and never found the voice, and its context let go.
    fn quit_combined(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let Combined {
            n,
            music_found,
            lyrics_found,
            ..
        } = *self.combined(id);
        if lyrics_found && !music_found {
            let m = self.it(id).music;
            let heap = host.heap();
            let mut kind = self.value(heap, m, self.keys.associated_context_type);
            if kind.tag() != Tag::Symbol {
                kind = heap.intern("Voice");
            }
            let voice = self.value(heap, m, self.keys.associated_context);
            let voice = heap
                .str(voice)
                .map(|s| String::from(s.as_str()))
                .unwrap_or_default();
            let text = alloc::format!(
                "cannot find context: {}",
                diagnostic_id(host.heap(), kind, &voice)
            );
            self.warning(host, m, &text)?;
        }
        if let Some(n) = n {
            self.t.arena.borrow_mut().combines[n as usize].held = false;
        }
        Ok(())
    }

    /// `Lyric_combine_music_iterator::substitute_context`: its lyrics' and
    /// its voice's contexts too.
    fn substitute_combined(&mut self, heap: &mut Heap, n: u32, from: CtxId, to: CtxId) {
        let mut a = self.t.arena.borrow_mut();
        let c = &mut a.combines[n as usize];
        if c.lyrics == Some(from) {
            c.lyrics = Some(to);
        }
        if c.music == Some(from) {
            lyrics::set_music_context(heap, &mut a, n, Some(to));
        }
    }

    // --- quotes ---------------------------------------------------------------

    fn quote(&mut self, id: IterId) -> &mut Quote {
        match &mut self.it_mut(id).kind {
            Kind::Wrapper {
                wrap: Wrap::Quote(q),
                ..
            } => q,
            _ => unreachable!("a quote iterator"),
        }
    }

    /// `Quote_iterator::create_contexts`, after its child's: the context of
    /// its music's `quoted-context-type` and `quoted-context-id` found or
    /// made from its context, else that context's default bottom one, as
    /// its handle.
    fn quote_contexts(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let Some(own) = self.get_context(id).or(self.it(id).context) else {
            return Ok(());
        };
        let m = self.it(id).music;
        let heap = host.heap();
        let name = self.value(heap, m, self.keys.quoted_context_type);
        let mut cue = None;
        if heap.name(name).is_some() {
            let ci = self.value(heap, m, self.keys.quoted_context_id);
            let ci = heap
                .str(ci)
                .map(|s| String::from(s.as_str()))
                .unwrap_or_default();
            let t = self.t.clone();
            cue = find_create_context(host, &t, own, Dir::Center, name, &ci, Value::NULL)?;
            if cue.is_none() {
                let text = alloc::format!(
                    "cannot find or create context: {}",
                    diagnostic_id(host.heap(), name, &ci)
                );
                diagnose(host, "warning", &text)?;
            }
        }
        let cue = match cue {
            Some(c) => c,
            None => {
                let t = self.t.clone();
                get_default_interpreter(host, &t, own, "")?
            }
        };
        self.set_handler(id, Some(cue));
        Ok(())
    }

    /// `Quote_iterator::process`: its child's when that is due, then the
    /// entries of its `quoted-events` up to `until` (its own time), each
    /// event its context accepts broadcast by its handle, transposed from
    /// the quoted voice's transposition to its own. Its zero, and the
    /// entries in its time, are found at its first `process`.
    fn process_quote(&mut self, host: &mut dyn Host, id: IterId, until: Moment) -> Run<()> {
        if let Kind::Wrapper { child: Some(c), .. } = self.it(id).kind
            && !lt(until, self.pending(c))
        {
            self.process(host, c, until)?;
        }
        let m = self.it(id).music;
        let events = self.value(host.heap(), m, self.keys.quoted_events);
        let entries: Vec<Value> = host
            .heap()
            .vector_items(events)
            .map(<[Value]>::to_vec)
            .unwrap_or_default();
        if self.quote(id).first_time {
            let start = self.t.arena.borrow().now();
            let zero = sub(start, self.it(id).start);
            let length = self.it(id).length;
            let heap = host.heap();
            let (mut idx, mut end) = (0, 0);
            if heap.vector_items(events).is_some() {
                idx = lower_bound(heap, &entries, start);
                // Its end, but grace notes leading to a note not quoted.
                let end_mom = Moment::new(
                    zero.main
                        .checked_add(length.main)
                        .unwrap_or(Rational::INFINITY),
                    -Rational::INFINITY,
                );
                end = lower_bound(heap, &entries, end_mom);
            }
            let q = self.quote(id);
            q.first_time = false;
            q.zero = zero;
            q.idx = idx;
            q.end = end;
        }
        let until = add(self.quote(id).zero, until);
        let ran = self.quote_entries(host, id, m, &entries, until);
        let q = self.quote(id);
        q.next = entries
            .get(q.idx)
            .filter(|_| q.idx < q.end)
            .and_then(|&e| entry_moment(host.heap(), e));
        ran
    }

    /// `Quote_iterator::process`'s loop over the entries due by `until`.
    fn quote_entries(
        &mut self,
        host: &mut dyn Host,
        id: IterId,
        m: M,
        entries: &[Value],
        until: Moment,
    ) -> Run<()> {
        while self.quote(id).idx < self.quote(id).end {
            let entry = entries[self.quote(id).idx];
            let heap = host.heap();
            let first = heap.car(entry).unwrap_or(Value::NULL);
            match entry_moment(heap, entry) {
                Some(at) if lt(until, at) => return Ok(()),
                Some(_) => {}
                None => {
                    let mut text = String::from("expected moment in event vector: ");
                    heap.write(heap.car(first).unwrap_or(Value::NULL), &mut text);
                    diagnose(host, "programming-error", &text)?;
                    self.quote(id).idx += 1;
                    continue;
                }
            }
            self.quote_entry(host, id, m, entry)?;
            self.quote(id).idx += 1;
        }
        Ok(())
    }

    /// One entry's events: `((moment . pitch) (event . #t) …)`, `pitch` the
    /// quoted voice's `instrumentTransposition`.
    fn quote_entry(&mut self, host: &mut dyn Host, id: IterId, m: M, entry: Value) -> Run<()> {
        let context = self.get_context(id);
        let heap = host.heap();
        let first = heap.car(entry).unwrap_or(Value::NULL);
        let quote_pitch = heap
            .cdr(first)
            .and_then(|p| heap.foreign::<crate::Pitch>(p))
            .copied();
        // The pitch that sounds when a written central C is played.
        // `quoted-transposition` has the other sign.
        let transposition = self.value(heap, m, self.keys.quoted_transposition);
        let scale = host.lily("default-global-scale").unwrap_or(Value::FALSE);
        let heap = host.heap();
        let me_pitch = match heap.foreign::<crate::Pitch>(transposition).copied() {
            Some(p) => Some(negated(heap, p, scale)),
            None => {
                let sym = heap.intern("instrumentTransposition");
                let v = context.map_or(Value::NULL, |c| self.t.arena.borrow().get(c, sym));
                host.heap().foreign::<crate::Pitch>(v).copied().map(Some)
            }
        };
        let heap = host.heap();
        let cid = self.value(heap, m, self.keys.quoted_context_id);
        let is_cue = heap.str(cid).is_some_and(|s| s.as_str() == "cue");
        let mut evs = Vec::new();
        let mut p = heap.cdr(entry).unwrap_or(Value::NULL);
        while let (Some(acc), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            evs.push(heap.car(acc).unwrap_or(Value::NULL));
            p = rest;
        }
        for ev in evs {
            if host.heap().foreign::<StreamEvent>(ev).is_none() {
                diagnose(host, "programming-error", "no music found in quote")?;
                continue;
            }
            if !self.accepts(host, context, ev, is_cue) {
                continue;
            }
            let depth = host.heap().root_depth();
            let mut ev = ev;
            if quote_pitch.is_some() || me_pitch.is_some_and(|p| p.is_some()) {
                let heap = host.heap();
                let zero = crate::Pitch::zero(scale);
                let qp = quote_pitch.unwrap_or(zero);
                let mp = me_pitch.flatten().unwrap_or(zero);
                let Some(copy) = crate::dispatcher::clone_event(heap, ev) else {
                    continue;
                };
                heap.root(copy);
                let done = interval(heap, mp, qp, scale)
                    .and_then(|diff| crate::transpose::transpose_event(heap, copy, diff, scale));
                if done.is_none() {
                    // A pitch past what the engine holds, where LilyPond's would wrap (D61).
                    host.heap().unroot_to(depth);
                    diagnose(host, "programming-error", "Numerical overflow")?;
                    continue;
                }
                ev = copy;
            }
            let ran = match self.quote(id).handler {
                Some(h) => {
                    let d = self.t.arena.borrow().event_source(h);
                    let t = self.t.clone();
                    dispatch(host, &t, d, ev)
                }
                None => Ok(()),
            };
            host.heap().unroot_to(depth);
            ran?;
        }
        Ok(())
    }

    /// `accept_music_type`: whether `ev` is of a class of the context's
    /// `quotedCueEventTypes` (for a cue, where set) or `quotedEventTypes`.
    fn accepts(
        &self,
        host: &mut dyn Host,
        context: Option<CtxId>,
        ev: Value,
        is_cue: bool,
    ) -> bool {
        let Some(c) = context else {
            return false;
        };
        let heap = host.heap();
        let mut accept = Value::NULL;
        if is_cue {
            let sym = heap.intern("quotedCueEventTypes");
            accept = self.t.arena.borrow().get(c, sym);
        }
        if accept == Value::NULL {
            let sym = heap.intern("quotedEventTypes");
            accept = self.t.arena.borrow().get(c, sym);
        }
        let classes = heap
            .foreign::<StreamEvent>(ev)
            .and_then(|e| e.property(heap, self.keys.class))
            .unwrap_or(Value::NULL);
        let mut a = accept;
        while let (Some(class), Some(rest)) = (heap.car(a), heap.cdr(a)) {
            let mut p = classes;
            while let (Some(c), Some(more)) = (heap.car(p), heap.cdr(p)) {
                if c == class {
                    return true;
                }
                p = more;
            }
            a = rest;
        }
        false
    }

    // --- tuplets -------------------------------------------------------------

    fn tuplet(&mut self, id: IterId) -> (&mut Moment, &mut Moment, Option<CtxId>) {
        match &mut self.it_mut(id).kind {
            Kind::Wrapper {
                wrap:
                    Wrap::Tuplet {
                        spanner,
                        next,
                        handler,
                    },
                ..
            } => (spanner, next, *handler),
            _ => unreachable!("a tuplet iterator"),
        }
    }

    /// `tuplet_handler_`, `event_handler_` or `quote_handle_` set, a client
    /// of its context.
    fn set_handler(&mut self, id: IterId, c: Option<CtxId>) {
        let Kind::Wrapper {
            wrap:
                Wrap::Tuplet { handler, .. }
                | Wrap::Volta { handler, .. }
                | Wrap::Quote(Quote { handler, .. }),
            ..
        } = &mut self.it_mut(id).kind
        else {
            return;
        };
        let old = core::mem::replace(handler, c);
        let mut a = self.t.arena.borrow_mut();
        if let Some(o) = old {
            let n = a.clients(o);
            a.set_clients(o, n.saturating_sub(1));
        }
        if let Some(n) = c {
            let k = a.clients(n);
            a.set_clients(n, k + 1);
        }
    }

    /// `descend_to_child` of its child's context, if the child is not
    /// done.
    fn tuplet_descend(&mut self, id: IterId) {
        if let Kind::Wrapper { child: Some(c), .. } = self.it(id).kind
            && self.ok(c)
        {
            let reported = self.get_context(c);
            self.descend_to_child(id, reported);
        }
    }

    /// `Tuplet_iterator::process` before its child's: the spanner's length
    /// the first time, then at each split the bracket stopped and the next
    /// started.
    fn tuplet_split(&mut self, host: &mut dyn Host, id: IterId, until: Moment) -> Run<()> {
        if self.tuplet(id).0.main.is_negative() {
            let heap = host.heap();
            let m = self.it(id).music;
            let duration = self.value(heap, m, self.keys.duration);
            let length = match heap.foreign::<Duration>(duration) {
                Some(d) => d.length().unwrap_or(Rational::INFINITY),
                None => {
                    let sym = self.keys.tuplet_spanner_duration;
                    let v = self
                        .get_context(id)
                        .map_or(Value::NULL, |c| self.t.arena.borrow().get(c, sym));
                    rational_of(heap, v).unwrap_or(Rational::INFINITY)
                }
            };
            *self.tuplet(id).0 = Moment::new(length, Rational::ZERO);
        }
        let (spanner, next, handler) = {
            let (s, n, h) = self.tuplet(id);
            (*s, *n, h)
        };
        let at = Moment::new(until.main, Rational::ZERO);
        if spanner == ZERO || at != next {
            return Ok(());
        }
        if let Some(h) = handler {
            let ev = self.span_event(host, id, STOP, spanner)?;
            let depth = host.heap().root_depth();
            host.heap().root(ev);
            let ran = self.event_of(host, M::Value(ev)).and_then(|ev| {
                let d = self.t.arena.borrow().event_source(h);
                let t = self.t.clone();
                dispatch(host, &t, d, ev)
            });
            host.heap().unroot_to(depth);
            ran?;
        }
        let length = self.length(id);
        if lt(
            Moment::new(until.main, Rational::ZERO),
            Moment::new(length.main, Rational::ZERO),
        ) {
            let spanner = min(sub(length, next), spanner);
            *self.tuplet(id).0 = spanner;
            let c = self.get_context(id);
            self.set_handler(id, c);
            let ev = self.span_event(host, id, START, spanner)?;
            let depth = host.heap().root_depth();
            host.heap().root(ev);
            let ran = self.report_event(host, id, M::Value(ev));
            host.heap().unroot_to(depth);
            ran?;
            *self.tuplet(id).1 = add(next, spanner);
        } else {
            self.set_handler(id, None);
        }
        Ok(())
    }

    /// `create_event`: a `TupletSpanEvent` of direction `d` at the tuplet's
    /// origin, a start with its fraction, tweaks and the spanner's length.
    fn span_event(
        &mut self,
        host: &mut dyn Host,
        id: IterId,
        d: i64,
        spanner: Moment,
    ) -> Run<Value> {
        let Some(make) = host.lily("make-span-event") else {
            return Ok(Value::FALSE);
        };
        let heap = host.heap();
        let class = heap.intern("TupletSpanEvent");
        let dir = Value::fixnum(d).unwrap_or(Value::NULL);
        let ev = host.call(make, &[class, dir])?;
        let heap = host.heap();
        let m = self.it(id).music;
        let k = &self.keys;
        let origin = self.value(heap, m, k.origin);
        crate::music::set_property(heap, ev, k.origin, origin);
        if d == START {
            for key in [k.numerator, k.denominator, k.tweaks] {
                let v = self.value(heap, m, key);
                crate::music::set_property(heap, ev, key, v);
            }
            let length = heap.wrap(spanner);
            crate::music::set_property(heap, ev, k.length, length);
        }
        Ok(ev)
    }

    // --- percent repeats and \fine -----------------------------------------

    /// `Percent_repeat_iterator::create_contexts`, before the sequence's:
    /// its body's length and its repeat count.
    fn percent_contexts(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let m = self.it(id).music;
        let heap = host.heap();
        let body = m
            .at(heap)
            .and_then(|at| at.property(heap, self.keys.element))
            .and_then(|item| music_of(heap, &item));
        let length = match body {
            Some(b) => Some(self.time(host, b, false)?),
            None => None,
        };
        let reps = self.value(host.heap(), m, self.keys.repeat_count);
        let reps = host.heap().to_i64(reps).unwrap_or(0);
        if let SeqKind::Percent(p) = &mut self.seq(id).volta {
            p.body = length.unwrap_or(p.body);
            p.reps = reps;
        }
        Ok(())
    }

    /// `Percent_repeat_iterator::next_element`, after the sequence's: at the
    /// start of each repetition, a `PercentEvent` if the body is a measure
    /// long, a `DoublePercentEvent` if two, else a `RepeatSlashEvent`.
    fn next_percent(&mut self, host: &mut dyn Host, id: IterId) -> Run<()> {
        let SeqKind::Percent(p) = &mut self.seq(id).volta else {
            return Ok(());
        };
        p.done += 1;
        let (done, body, event) = (p.done, p.body, p.event);
        if done >= p.reps {
            return Ok(());
        }
        let m = self.it(id).music;
        let name = match event {
            Some(name) => name,
            None => {
                let measure = self
                    .get_context(id)
                    .and_then(|c| {
                        let sym = host.heap().intern("measureLength");
                        self.t.arena.borrow().where_defined(c, sym)
                    })
                    .and_then(|(_, v)| rational_of(host.heap(), v))
                    .unwrap_or(Rational::int(1));
                let name = if body.main == measure {
                    "PercentEvent"
                } else if Some(body.main) == measure.checked_add(measure) {
                    "DoublePercentEvent"
                } else {
                    let heap = host.heap();
                    let element = m
                        .at(heap)
                        .and_then(|at| at.property(heap, self.keys.element))
                        .and_then(|item| music_of(heap, &item));
                    if let (Some(e), Some(count)) = (element, host.lily("calc-repeat-slash-count"))
                    {
                        let handle = e.handle(host.heap());
                        let depth = host.heap().root_depth();
                        host.heap().root(handle);
                        let slashes = host.call(count, &[handle]);
                        host.heap().unroot_to(depth);
                        let slashes = slashes?;
                        // Held till the translation ends.
                        host.heap().root(slashes);
                        if let SeqKind::Percent(p) = &mut self.seq(id).volta {
                            p.slashes = Some(slashes);
                        }
                    }
                    "RepeatSlashEvent"
                };
                if let SeqKind::Percent(p) = &mut self.seq(id).volta {
                    p.event = Some(name);
                }
                name
            }
        };
        let slashes = match &self.seq(id).volta {
            SeqKind::Percent(p) => p.slashes,
            _ => None,
        };
        let ev = self.make(host, Some(m), name)?;
        let heap = host.heap();
        let mut props = alloc::vec![
            ("length", heap.wrap(body)),
            ("repeat-count", heap.integer(done + 1)),
        ];
        if let Some(s) = slashes {
            props.push(("slash-count", s));
        }
        for (key, v) in props {
            let key = heap.intern(key);
            crate::music::set_property(heap, ev, key, v);
        }
        let depth = host.heap().root_depth();
        host.heap().root(ev);
        let ran = self.report_event(host, id, M::Value(ev));
        host.heap().unroot_to(depth);
        ran
    }

    /// `Fine_iterator::process` when it starts: a copy of its music,
    /// `fine-folded` if a folded repeat is above it, reported unless
    /// lyrics combined with a voice are above it.
    fn fine(&mut self, host: &mut dyn Host, id: IterId, m: M) -> Run<()> {
        if self
            .find_above(host.heap(), id, "lyric-combine-music")
            .is_some()
        {
            return Ok(());
        }
        let folded = self
            .find_above(host.heap(), id, "folded-repeated-music")
            .is_some();
        let heap = host.heap();
        let handle = m.handle(heap);
        let depth = heap.root_depth();
        heap.root(handle);
        let copy = crate::music::deep_copy(heap, handle);
        heap.root(copy);
        let key = heap.intern("fine-folded");
        crate::music::set_property(heap, copy, key, Value::bool(folded));
        let ran = self.report_event(host, id, M::Value(copy));
        host.heap().unroot_to(depth);
        ran
    }

    // --- events ------------------------------------------------------------

    /// `report_event`: `m`'s event broadcast by the bottom context.
    fn report_event(&mut self, host: &mut dyn Host, id: IterId, m: M) -> Run<()> {
        self.descend_to_bottom(host, id)?;
        let Some(c) = self.get_context(id) else {
            return Ok(());
        };
        let ev = self.event_of(host, m)?;
        let d = self.t.arena.borrow().event_source(c);
        let t = self.t.clone();
        dispatch(host, &t, d, ev)
    }

    /// `Rhythmic_music_iterator::process` when it starts: the event, then
    /// each articulation the context's event source has listeners for, as
    /// an event of its own.
    fn rhythmic(&mut self, host: &mut dyn Host, id: IterId, m: M) -> Run<()> {
        self.descend_to_bottom(host, id)?;
        let Some(c) = self.get_context(id) else {
            return Ok(());
        };
        let ev = self.event_of(host, m)?;
        let depth = host.heap().root_depth();
        host.heap().root(ev);
        let heap = host.heap();
        let arts = heap
            .foreign::<StreamEvent>(ev)
            .and_then(|e| e.property(heap, self.keys.articulations))
            .unwrap_or(Value::NULL);
        let t = self.t.clone();
        let source = t.arena.borrow().event_source(c);
        let mut listened = Vec::new();
        if arts.tag() == Tag::Pair {
            let mut unlistened = Vec::new();
            {
                let mut a = t.arena.borrow_mut();
                let heard = a.dispatchers.listened(source);
                let mut p = arts;
                while let (Some(art), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                    let class = heap
                        .foreign::<StreamEvent>(art)
                        .and_then(|e| e.property(heap, self.keys.class))
                        .unwrap_or(Value::NULL);
                    let is = a
                        .classes
                        .class(heap, class)
                        .is_some_and(|n| a.classes.set(n).intersects(&heard));
                    if is {
                        listened.push(art);
                    } else {
                        unlistened.push(art);
                    }
                    p = rest;
                }
            }
            for &art in &listened {
                heap.root(art);
            }
            let unlistened = heap.list(&unlistened);
            set_event_property(heap, ev, self.keys.articulations, unlistened);
        }
        let mut ran = dispatch(host, &t, source, ev);
        for art in listened {
            if ran.is_ok() {
                ran = dispatch(host, &t, source, art);
            }
        }
        host.heap().unroot_to(depth);
        ran
    }

    /// `Music::to_event`: an event of the class its name gives, referring
    /// to `m`, with its length if it has one, its articulations as events,
    /// and `music-cause`. Music a frozen node holds (other than the
    /// articulations) is given handles among the event's own properties.
    fn event_of(&mut self, host: &mut dyn Host, m: M) -> Run<Value> {
        let length = self.time(host, m, false)?;
        let heap = host.heap();
        let arts = m
            .at(heap)
            .and_then(|at| at.mutable(heap, self.keys.articulations))
            .map_or_else(Vec::new, |item| musics_of(heap, &item));
        let depth = heap.root_depth();
        let mut events = Vec::new();
        for a in arts.into_iter().flatten() {
            let ev = self.event_of(host, a);
            match ev {
                Ok(ev) => {
                    host.heap().root(ev);
                    events.push(ev);
                }
                Err(e) => {
                    host.heap().unroot_to(depth);
                    return Err(e);
                }
            }
        }
        let heap = host.heap();
        let k = &self.keys;
        let handle = m.handle(heap);
        let mut props = Value::NULL;
        if let Some(at @ At::Frozen { tree, .. }) = m.at(heap) {
            for (key, item) in at.entries(heap) {
                let v = match item {
                    _ if key == k.articulations => continue,
                    Item::Music(At::Frozen { node, .. }) => heap.wrap(Music::frozen(tree, node)),
                    Item::Musics(all) => {
                        let handles: Vec<Value> = all
                            .into_iter()
                            .filter_map(|at| match at {
                                At::Frozen { node, .. } => {
                                    Some(heap.wrap(Music::frozen(tree, node)))
                                }
                                At::Live(_) => None,
                            })
                            .collect();
                        heap.list(&handles)
                    }
                    _ => continue,
                };
                let entry = heap.cons(key, v);
                props = heap.cons(entry, props);
            }
        }
        if !events.is_empty() {
            let list = heap.list(&events);
            let entry = heap.cons(k.articulations, list);
            props = heap.cons(entry, props);
        }
        if length != ZERO {
            let length = heap.wrap(length);
            let entry = heap.cons(k.length, length);
            props = heap.cons(entry, props);
        }
        let entry = heap.cons(k.music_cause, handle);
        props = heap.cons(entry, props);
        // `ly_camel_case_2_lisp_identifier` of its name, as a class.
        let name = self.value(heap, m, k.name);
        let mut lisp = String::new();
        for (i, ch) in heap.name(name).unwrap_or_default().chars().enumerate() {
            if ch.is_ascii_uppercase() {
                if i > 0 {
                    lisp.push('-');
                }
                lisp.push(ch.to_ascii_lowercase());
            } else {
                lisp.push(ch);
            }
        }
        let sym = heap.intern(&lisp);
        let class = {
            let mut a = self.t.arena.borrow_mut();
            match a.class_number(heap, sym) {
                Some(n) => a.class_list(heap, n),
                None => heap.list(&[sym]),
            }
        };
        let ev = music_event(heap, class, props, handle);
        heap.unroot_to(depth);
        Ok(ev)
    }
}

/// `Direction`'s `START`, `CENTER` and `STOP`.
const START: i64 = -1;
const CENTER: i64 = 0;
const STOP: i64 = 1;

const fn simple(how: Simple) -> Kind {
    Kind::Simple { how, pending: ZERO }
}

const fn wrapper(wrap: Wrap) -> Kind {
    Kind::Wrapper { child: None, wrap }
}

const fn sequential(volta: SeqKind) -> Kind {
    Kind::Sequential(Seq {
        elements: Vec::new(),
        remaining: 0,
        ahead: 0,
        iter: None,
        iter_start: ZERO,
        ahead_mom: INFINITY,
        volta,
    })
}

impl Styler {
    const fn new(style: Style) -> Styler {
        Styler {
            style,
            spanned: (INFINITY, INFINITY),
            repeat_count: 2,
            alternative_depth: 0,
            reported_return_depth: 0,
            reported_return: false,
        }
    }
}

/// `from_scm<Rational>` with a fallback: an exact number or an infinity.
fn rational_of(heap: &Heap, v: Value) -> Option<Rational> {
    if let Some(x) = heap.as_flonum(v) {
        return x.is_infinite().then(|| {
            if x > 0.0 {
                Rational::INFINITY
            } else {
                -Rational::INFINITY
            }
        });
    }
    match heap.as_ratio(v) {
        Some((n, d)) => Some(Rational::new(heap.to_i64(n)?, heap.to_i64(d)?)),
        None => Some(Rational::int(heap.to_i64(v)?)),
    }
}

/// The moment of an entry of `quoted-events`, `((moment . pitch) …)`.
fn entry_moment(heap: &Heap, entry: Value) -> Option<Moment> {
    let first = heap.car(entry)?;
    heap.foreign::<Moment>(heap.car(first)?).copied()
}

/// `binsearch_scm_vector` with `moment_less`: the index of the first entry
/// whose moment is not less than `key`.
fn lower_bound(heap: &Heap, entries: &[Value], key: Moment) -> usize {
    let (mut lo, mut hi) = (0, entries.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if entry_moment(heap, entries[mid]).is_some_and(|m| lt(m, key)) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// `pitch_interval (from, to)` in the default scale `scale`, or None if it
/// did not fit.
fn interval(
    heap: &Heap,
    from: crate::Pitch,
    to: crate::Pitch,
    scale: Value,
) -> Option<crate::Pitch> {
    let steps = heap.foreign::<crate::Scale>(scale)?;
    let from_scale = heap.foreign::<crate::Scale>(from.scale)?;
    let to_scale = heap.foreign::<crate::Scale>(to.scale)?;
    crate::Pitch::interval((from, from_scale), (to, to_scale), (scale, steps))
}

/// `Pitch::negated`: `pitch_interval (p, Pitch ())`, or None.
fn negated(heap: &Heap, p: crate::Pitch, scale: Value) -> Option<crate::Pitch> {
    interval(heap, p, crate::Pitch::zero(scale), scale)
}
