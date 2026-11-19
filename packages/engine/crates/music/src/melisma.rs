//! The melisma properties (04 §13, N7's third part, MS2 step 39, D98):
//! `melisma_busy` (`context.cc`), which the lyric combine iterator asks
//! before it gives a voice's note a syllable, and the parts of the
//! translators that set what it reads, `melismaBusyProperties`: of
//! `Slur_engraver`, `Tie_engraver`, `Beam_engraver` and
//! `Grace_beam_engraver` in `\layout`, and of `Slur_performer`,
//! `Tie_performer` and `Beam_performer` in `\midi`. Their grobs and audio
//! items, and their warnings, are chapter 05's and 07's (D67); what each
//! keeps is what decides its property:
//!
//! - a slur engraver the spanner ids of its open slurs, started and ended
//!   by `slur-event`s, on their own or among a note's articulations;
//! - a beam engraver whether its beam is open, `beamMelismaBusy` set only
//!   when `autoBeaming` is off, in main time (`Beam_engraver`) or in grace
//!   time (`Grace_beam_engraver`);
//! - a slur or beam performer the last start and stop it heard now;
//! - a tie engraver or performer the notes waiting for the notes they are
//!   tied to, each with its end. LilyPond's know them as the note heads
//!   `Note_heads_engraver` makes and the audio notes `Note_performer`
//!   makes. The heads are placeholders here: the engraver hears the note
//!   events they would be made of (D98), in a context that consists one of
//!   them. The performer acknowledges `Note_performer`'s audio notes and
//!   ties them (`Audio_note::tie_to`, MS5 step 4).
//!
//! LilyPond 2.26.0 has no `Melisma_translator`: `\melisma` and
//! `\melismaEnd` set and unset `melismaBusy` (`declarations-init.ly`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_scheme::{Exception, Heap, Value};

use crate::Pitch;
use crate::audio::{Audio, tie_to};
use crate::context::{Arena, CtxId};
use crate::dispatcher::event_property;
use crate::pitch::Scale;
use crate::rational::{Moment, Rational};
use crate::records::Cause;
use crate::translator::{
    ACKNOWLEDGE_AUDIO, FINALIZE, Kind, PROCESS, PROCESS_ACKNOWLEDGED, START, STOP, TranslatorClass,
    Tx, Work,
};

type Run<T> = Result<T, Exception>;

/// `melisma_busy`: whether a melisma holds `c`. A context with children is
/// held if all of them are; one without, if any property of
/// `melismaBusyProperties` is true there. Layout's lyric engravers ask it
/// of the voice their lyrics follow (MS4 step 15).
pub fn melisma_busy(heap: &mut Heap, a: &Arena, c: CtxId) -> bool {
    let children = a.children(c);
    if !children.is_empty() {
        return children.iter().all(|&k| melisma_busy(heap, a, k));
    }
    let list = heap.intern("melismaBusyProperties");
    let mut p = a.get(c, list);
    while let (Some(sym), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if a.get(c, sym) == Value::TRUE {
            return true;
        }
        p = rest;
    }
    false
}

/// `in_event_class`: whether the event `ev`'s classes have `class`.
fn is_class(heap: &mut Heap, ev: Value, class: &str) -> bool {
    let class = heap.intern(class);
    let mut p = event_property(heap, ev, "class");
    while let (Some(c), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if c == class {
            return true;
        }
        p = rest;
    }
    false
}

/// `from_scm<Direction> (get_property (ev, "span-direction"))`.
fn span_direction(heap: &mut Heap, ev: Value) -> i64 {
    let d = event_property(heap, ev, "span-direction");
    heap.to_i64(d).unwrap_or(0)
}

/// `get_property (this, name)`, true only if `#t` (`from_scm<bool>`).
fn is_set(tx: &mut Tx<'_>, name: &str) -> bool {
    tx.get(name) == Value::TRUE
}

/// `get_event_length (e, now)`: the event's `length`, in grace time its
/// grace part.
fn event_length(heap: &mut Heap, ev: Value, now: Moment) -> Moment {
    let v = event_property(heap, ev, "length");
    let len = heap.foreign::<Moment>(v).copied().unwrap_or_default();
    if now.grace.is_zero() {
        len
    } else {
        Moment::new(Rational::ZERO, len.main)
    }
}

/// `now + len`, or no end if it overflows.
fn end_of(now: Moment, len: Moment) -> Moment {
    now.checked_add(len)
        .unwrap_or(Moment::new(Rational::INFINITY, Rational::ZERO))
}

/// The tie engravers' and performers' match of a later note `right` to a
/// note `left` waiting for it: their pitches `equal?`, or, `enharmonic`,
/// both pitches of the same tones.
fn same_pitch(heap: &mut Heap, left: Value, right: Value, enharmonic: bool) -> bool {
    let (p1, p2) = (
        event_property(heap, left, "pitch"),
        event_property(heap, right, "pitch"),
    );
    if !enharmonic {
        return heap.equal(p1, p2);
    }
    let tones = |p: Value| {
        let p = heap.foreign::<Pitch>(p)?;
        p.tones(heap.foreign::<Scale>(p.scale)?)
    };
    matches!((tones(p1), tones(p2)), (Some(a), Some(b)) if a == b)
}

/// Whether the context's group consists one of `names`.
fn consists(tx: &Tx<'_>, names: &[&str]) -> bool {
    tx.t.arena
        .borrow()
        .translator_names(tx.context)
        .iter()
        .any(|n| names.contains(&n.as_str()))
}

// --- slurs -----------------------------------------------------------------
//
// Since MS4 step 7 layout's `Slur_engraver` (`slur_engraver.rs`) shadows
// this in `\layout`, making the slurs and setting `slurMelismaBusy` as
// LilyPond's does; this one stays where a placeholder makes the heads
// (`Drum_notes_engraver`, `Tab_note_heads_engraver`), and without layout.

pub(crate) const SLUR_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Slur_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    // `process_music` sets `slurMelismaBusy` every timestep.
    always: true,
    hears: &["slur-event", "note-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(SlurEngraver::default())),
    reads: &["slurMelismaBusy", "doubleSlurs"],
    writes: &[],
};

/// `Slur_engraver`: of its slurs, their spanner ids.
#[derive(Clone, Default)]
struct SlurEngraver {
    starts: Vec<Value>,
    stops: Vec<Value>,
    /// `slurs_`, by spanner id: a start of an id already open adds no slur
    /// the melisma sees, as `can_create_slur` lets it add none or one
    /// beside it.
    open: Vec<Value>,
}

impl SlurEngraver {
    /// `listen_note_slur`.
    fn heard(&mut self, heap: &mut Heap, ev: Value) {
        match span_direction(heap, ev) {
            -1 => self.starts.push(ev),
            1 => self.stops.push(ev),
            _ => {}
        }
    }
}

impl Work for SlurEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        let heap = tx.heap();
        if which == 0 {
            self.heard(heap, ev);
            return Ok(());
        }
        // A slur among a note's articulations, in a chord.
        let mut p = event_property(heap, ev, "articulations");
        while let (Some(art), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            if is_class(heap, art, "slur-event") {
                self.heard(heap, art);
            }
            p = rest;
        }
        Ok(())
    }

    /// `process_music`: the slurs ended, then those started, and
    /// `slurMelismaBusy` whether any is open.
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let heap = tx.host.heap();
        for &ev in &self.stops {
            let id = event_property(heap, ev, "spanner-id");
            self.open.retain(|&o| !heap.equal(o, id));
        }
        for &ev in self.starts.iter().rev() {
            let id = event_property(heap, ev, "spanner-id");
            if !self.open.iter().any(|&o| heap.equal(o, id)) {
                self.open.push(id);
            }
        }
        let busy = Value::bool(!self.open.is_empty());
        tx.set("slurMelismaBusy", busy);
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.starts.clear();
        self.stops.clear();
        Ok(())
    }

    fn finalize(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.open.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(&self.starts);
        out.extend(&self.stops);
        out.extend(&self.open);
    }
}

// --- beams -----------------------------------------------------------------

pub(crate) const BEAM_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Beam_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS | STOP,
    // Awake while its beam is open.
    always: false,
    hears: &["beam-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(BeamEngraver::default())),
    reads: &[
        "beamMelismaBusy",
        "beatBase",
        "beatStructure",
        "subdivideBeams",
    ],
    writes: &[],
};

pub(crate) const GRACE_BEAM_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Grace_beam_engraver",
    make: Some(|| {
        Box::new(BeamEngraver {
            grace: true,
            ..BeamEngraver::default()
        })
    }),
    ..BEAM_ENGRAVER
};

/// `Beam_engraver`, and `Grace_beam_engraver` (`grace`), which takes the
/// beams of grace time.
#[derive(Clone, Default)]
struct BeamEngraver {
    grace: bool,
    start: bool,
    stop: bool,
    /// `beam_`.
    open: bool,
}

impl BeamEngraver {
    /// `valid_start_point`: in main time, or for `Grace_beam_engraver` in
    /// grace time.
    fn valid_start(&self, tx: &Tx<'_>) -> bool {
        tx.now().grace.is_zero() != self.grace
    }

    /// `set_melisma`: `beamMelismaBusy` set unless `autoBeaming`.
    fn set_melisma(tx: &mut Tx<'_>, busy: bool) {
        if !is_set(tx, "autoBeaming") {
            tx.set("beamMelismaBusy", Value::bool(busy));
        }
    }
}

impl Work for BeamEngraver {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.flags(&[self.grace, self.start, self.stop, self.open]);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        let d = span_direction(tx.heap(), ev);
        if d == -1 && self.valid_start(tx) {
            self.start = true;
        } else if d == 1 && (!self.grace || self.open) && self.valid_start(tx) {
            // `valid_end_point`, which for grace beams needs one open.
            self.stop = true;
        }
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.open {
            Self::set_melisma(tx, true);
        }
        Ok(())
    }

    /// `process_music`: a beam started, unless one is open (LilyPond's
    /// `already have a beam`, chapter 05's).
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.start && !self.open {
            Self::set_melisma(tx, true);
            self.open = true;
        }
        Ok(())
    }

    /// Its start forgotten here, where LilyPond's forgets it at the next
    /// timestep's start, which a sleeping translator is not called for.
    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.start = false;
        if self.stop {
            self.stop = false;
            self.open = false;
            Self::set_melisma(tx, false);
        }
        if self.open {
            tx.stay_awake();
        }
        Ok(())
    }
}

// --- the slur and beam performers ------------------------------------------

pub(crate) const SLUR_PERFORMER: TranslatorClass = TranslatorClass {
    name: "Slur_performer",
    kind: Kind::Performer,
    methods: PROCESS | STOP,
    always: false,
    hears: &["slur-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(SpanPerformer::default())),
    reads: &[],
    writes: &[],
};

pub(crate) const BEAM_PERFORMER: TranslatorClass = TranslatorClass {
    name: "Beam_performer",
    hears: &["beam-event"],
    make: Some(|| {
        Box::new(SpanPerformer {
            beam: true,
            ..SpanPerformer::default()
        })
    }),
    ..SLUR_PERFORMER
};

/// `Slur_performer`, and `Beam_performer` (`beam`): the last start and stop
/// heard now (`Last_span_event_listener`), forgotten at the end of the
/// timestep, where LilyPond's forget them at the start of the next.
#[derive(Clone, Default)]
struct SpanPerformer {
    beam: bool,
    start: bool,
    stop: bool,
}

impl Work for SpanPerformer {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.flags(&[self.beam, self.start, self.stop]);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        match span_direction(tx.heap(), ev) {
            -1 => self.start = true,
            1 => self.stop = true,
            _ => {}
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if !self.start && !self.stop {
            return Ok(());
        }
        let busy = Value::bool(self.start);
        if !self.beam {
            tx.set("slurMelismaBusy", busy);
        } else if !is_set(tx, "autoBeaming") {
            tx.set("beamMelismaBusy", busy);
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.start = false;
        self.stop = false;
        Ok(())
    }
}

// --- ties ------------------------------------------------------------------

pub(crate) const TIE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Tie_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS | PROCESS_ACKNOWLEDGED | STOP,
    // `start_translation_timestep` sets `tieMelismaBusy` every timestep.
    always: true,
    // The notes, for the heads `Note_heads_engraver` would make (D98).
    hears: &["tie-event", "note-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(TieEngraver::default())),
    reads: &["skipTypesetting", "tieWaitForNote"],
    writes: &["tieMelismaBusy"],
};

/// The classes that make a note head of each note they hear.
const HEADS: &[&str] = &[
    "Note_heads_engraver",
    "Drum_notes_engraver",
    "Tab_note_heads_engraver",
];

/// `Tie_engraver`.
#[derive(Clone, Default)]
struct TieEngraver {
    /// Whether its context makes note heads (D98).
    heads: bool,
    /// `event_`, the tie heard on its own, kept till a note takes it.
    event: Option<Value>,
    taken: bool,
    /// The notes heard now, the heads to be acknowledged.
    notes: Vec<Value>,
    /// `heads_to_tie_`: each note and its end. Whether a note of its chord
    /// found its own (`tie_from_chord_created`) is for chapter 05's
    /// `unterminated tie`.
    waiting: Vec<(Value, Moment)>,
}

impl TieEngraver {
    /// `tie_notehead`: the first note waiting for `note`, if any, tied to it.
    fn tie(&mut self, heap: &mut Heap, note: Value, enharmonic: bool) -> bool {
        let found = self.waiting.iter().position(|&(left, _)| {
            same_pitch(heap, left, note, enharmonic)
                && event_property(heap, left, "autosplit-end") != Value::TRUE
        });
        found.map(|i| self.waiting.remove(i)).is_some()
    }
}

impl Work for TieEngraver {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.flags(&[self.heads, self.taken]);
        h.num(self.waiting.len() as i64);
        for &(_, m) in &self.waiting {
            h.moment(m);
        }
    }

    fn connect(&mut self, tx: &mut Tx<'_>) {
        self.heads = consists(tx, HEADS);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        if which == 1 {
            if self.heads {
                self.notes.push(ev);
            }
        } else if !is_set(tx, "skipTypesetting") && self.event.is_none() {
            self.event = Some(ev);
        }
        Ok(())
    }

    /// The notes past their ends forgotten, unless `tieWaitForNote`, and
    /// `tieMelismaBusy` whether any note waits.
    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if !self.waiting.is_empty() && !is_set(tx, "tieWaitForNote") {
            let now = tx.now();
            // Their `unterminated tie` is chapter 05's.
            self.waiting.retain(|w| !now.compare(w.1).is_gt());
        }
        tx.set("tieMelismaBusy", Value::bool(!self.waiting.is_empty()));
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.event.is_some() || !self.waiting.is_empty() {
            tx.set("tieMelismaBusy", Value::TRUE);
        }
        Ok(())
    }

    /// The heads acknowledged (`acknowledge_note_head`: each tied to a note
    /// waiting for it), then `process_acknowledged`: if any was, the others
    /// waiting forgotten, unless `tieWaitForNote`; and the notes of a tie
    /// now made to wait, the others forgotten if there are any.
    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.notes.is_empty() {
            return Ok(());
        }
        let notes = core::mem::take(&mut self.notes);
        let heap = tx.host.heap();
        let mut tied = false;
        for &note in &notes {
            if self.tie(heap, note, false) || self.tie(heap, note, true) {
                tied = true;
            }
        }
        let wait = is_set(tx, "tieWaitForNote");
        if tied && !wait {
            self.waiting.clear();
        }
        let now = tx.now();
        let heap = tx.host.heap();
        let mut new = Vec::new();
        for &note in &notes {
            let mut tie = self.event.is_some();
            let mut p = event_property(heap, note, "articulations");
            while let (false, Some(art), Some(rest)) = (tie, heap.car(p), heap.cdr(p)) {
                tie = is_class(heap, art, "tie-event");
                p = rest;
            }
            if tie && event_property(heap, note, "autosplit-end") != Value::TRUE {
                self.taken = true;
                new.push((note, end_of(now, event_length(heap, note, now))));
            }
        }
        if !wait && !new.is_empty() {
            self.waiting.clear();
        }
        self.waiting.extend(new);
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        if self.taken {
            self.event = None;
        }
        self.taken = false;
        // Heads are made only in a timestep that is typeset.
        self.notes.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.event);
        out.extend(&self.notes);
        out.extend(self.waiting.iter().map(|w| w.0));
    }
}

pub(crate) const TIE_PERFORMER: TranslatorClass = TranslatorClass {
    name: "Tie_performer",
    kind: Kind::Performer,
    methods: START | PROCESS | STOP | ACKNOWLEDGE_AUDIO,
    always: true,
    hears: &["tie-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(TiePerformer::default())),
    reads: &["tieWaitForNote"],
    writes: &["tieMelismaBusy"],
};

/// `Tie_performer`: its audio notes are `Note_performer`'s, which it
/// acknowledges and ties (MS5 step 4), each kept with its end
/// (`Head_audio_event_tuple`).
#[derive(Clone, Default)]
struct TiePerformer {
    /// `event_`.
    event: Option<Value>,
    /// `now_heads_` and `now_tied_heads_`: the notes acknowledged now,
    /// untied and with a tie.
    heads: Vec<(u32, Moment)>,
    tied_heads: Vec<(u32, Moment)>,
    /// `heads_to_tie_`.
    waiting: Vec<(u32, Moment)>,
}

impl Work for TiePerformer {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        for list in [&self.heads, &self.tied_heads, &self.waiting] {
            h.num(list.len() as i64);
            for &(id, m) in list {
                h.record(id);
                h.moment(m);
            }
        }
    }

    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.event = Some(ev);
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        tx.set("tieMelismaBusy", Value::bool(!self.waiting.is_empty()));
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.event.is_some() {
            tx.set("tieMelismaBusy", Value::TRUE);
        }
        Ok(())
    }

    /// `acknowledge_audio_element`: an audio note kept with its end, and
    /// tied to the first note waiting for it of equal pitch, else of the
    /// same tones, given the time between that one's end and now.
    fn acknowledge_audio(&mut self, tx: &mut Tx<'_>, audio: u32) -> Run<()> {
        let now = tx.now();
        let (end, tied, right) = {
            let a = tx.t.arena.borrow();
            let r = &a.records.audio[audio as usize];
            let Audio::Note(n) = &r.audio else {
                return Ok(());
            };
            (end_of(now, n.length), n.tie_event, event_of(r.cause))
        };
        if tied {
            self.tied_heads.push((audio, end));
        } else {
            self.heads.push((audio, end));
        }
        let Some(right) = right else {
            return Ok(());
        };
        let found = {
            let heap = tx.host.heap();
            let a = tx.t.arena.borrow();
            let mut find = |enharmonic| {
                self.waiting.iter().position(|&(left, _)| {
                    event_of(a.records.audio[left as usize].cause)
                        .is_some_and(|left| same_pitch(heap, left, right, enharmonic))
                })
            };
            match find(false) {
                None => find(true),
                found => found,
            }
        };
        if let Some(i) = found {
            let (left, left_end) = self.waiting.remove(i);
            let skip = now.checked_sub(left_end).unwrap_or_default();
            let mut a = tx.t.arena.borrow_mut();
            tie_to(&mut a.records.audio, audio, left, skip);
            let head = crate::audio::tie_head(&a.records.audio, audio);
            a.audio_changed(audio);
            a.audio_changed(head);
        }
        Ok(())
    }

    /// The notes past their ends forgotten, unless `tieWaitForNote`, and
    /// those of now made to wait: the tied ones, and all if a tie was
    /// heard on its own.
    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if !is_set(tx, "tieWaitForNote") {
            let now = tx.now();
            self.waiting.retain(|w| w.1.compare(now).is_gt());
        }
        let heads = core::mem::take(&mut self.heads);
        if self.event.take().is_some() {
            self.waiting.extend(heads);
        }
        self.waiting.append(&mut self.tied_heads);
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.event);
    }
}

/// The stream event an audio item was made for, if any.
fn event_of(cause: Cause) -> Option<Value> {
    match cause {
        Cause::Event(ev) => Some(ev),
        _ => None,
    }
}
