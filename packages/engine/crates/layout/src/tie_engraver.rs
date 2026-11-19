//! `tie-engraver.cc` (MS4's, ported first at MS3 step 17 for
//! `Accidental_engraver`'s tied entries): `Tie_engraver`, a class of
//! layout's ([`TIE_ENGRAVER`]) that the core passes at its `\layout` call
//! and that a group finds before the music crate's class of the name, which
//! keeps `tieMelismaBusy` without grobs (D98, D108). It sets
//! `tieMelismaBusy` as that class does, from the note heads it acknowledges
//! rather than the notes those are made of; in a context whose heads a
//! placeholder makes (`Tab_note_heads_engraver` where the translation has
//! no tablature's, a score in a markup's since MS4 step 7a;
//! `Drum_notes_engraver` until step 13), which no head reaches yet, it is
//! that class.
//!
//! - Each note head acknowledged (`acknowledge_note_head`, `:209–225`):
//!   tied to the first head waiting for it of the same pitch, else of the
//!   same tones (`tie_notehead`, `:140–207`), its tie's end announced and
//!   its bounds the heads; the timestep's `TieColumn` made of the first tie
//!   and each tie put in it (`Tie_column::add_tie`).
//! - `process_acknowledged` (`:248–335`): the ties typeset, the heads still
//!   waiting given up unless `tieWaitForNote`; a `Tie` made for each note
//!   head of now whose note has a tie, or of a tie heard on its own, which
//!   then waits for its next head till the note's end.
//! - `start_translation_timestep` (`:227–246`): the heads past their ends
//!   given up unless `tieWaitForNote` (`unterminated tie`), and
//!   `tieMelismaBusy`; `process_music` (`:128–138`) sets it where a tie is
//!   heard or a head waits.
//!
//! The ties' shapes, and their columns', are MS4's.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, Moment, PROCESS, PROCESS_ACKNOWLEDGED, Pitch, Rational, START, STOP, Scale,
    StreamEvent, TranslatorClass, Tx, Work, assign_event_once, event_property,
};
use lily_engine_scheme::{Exception, Heap, Value};

use crate::clef_engraver::is_true;
use crate::grob;
use crate::spanner;
use crate::tie;
use crate::tie_column;

type Run<T> = Result<T, Exception>;

pub static TIE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Tie_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS | PROCESS_ACKNOWLEDGED | STOP,
    // `start_translation_timestep` sets `tieMelismaBusy` every timestep.
    always: true,
    // The notes for the music crate's class only, in a context whose heads
    // a placeholder makes.
    hears: &["tie-event", "note-event"],
    acknowledges: &[("note-head-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(TieEngraver::default())),
    reads: &["skipTypesetting", "tieWaitForNote"],
    writes: &["tieMelismaBusy"],
};

/// `Head_event_tuple`: a head waiting for the head it is tied to, the end
/// of its note, its tie, and what caused that.
#[derive(Clone)]
struct Waiting {
    head: Value,
    end: Moment,
    tie_stream_event: Option<Value>,
    tie_event: Option<Value>,
    tie: (u32, Value),
    tie_from_chord_created: bool,
}

/// `event_processed_`, `event_`, `now_heads_`, `heads_to_tie_`, `ties_`
/// and `tie_column_`, the grobs by their handles; a waiting tie by its
/// record too, to announce its end.
#[derive(Clone, Default)]
struct TieEngraver {
    /// The music crate's class, where a placeholder makes the heads
    /// ([`placeholder_heads`]).
    placeholder: Option<Box<dyn Work>>,
    event_processed: bool,
    event: Option<Value>,
    now_heads: Vec<Value>,
    heads_to_tie: Vec<Waiting>,
    ties: Vec<Value>,
    tie_column: Option<Value>,
}

/// The classes that make note heads, which a placeholder of the music
/// crate's may stand for: `Tab_note_heads_engraver` where the translation
/// is not given `crates/tablature`'s (MS4 step 7a); `Drum_notes_engraver`
/// until MS4 step 13.
pub(crate) const PLACEHOLDER_HEADS: &[&str] = &["Tab_note_heads_engraver"];

/// Whether a placeholder makes `tx`'s context's heads.
pub(crate) fn placeholder_heads(tx: &Tx<'_>) -> bool {
    tx.t.arena
        .borrow()
        .placeholder_names(tx.context)
        .iter()
        .any(|n| PLACEHOLDER_HEADS.contains(n))
}

/// `Grob::event_cause`: the grob's cause if it is a stream event.
pub(crate) fn event_cause(tx: &mut Tx<'_>, g: Value) -> Run<Option<Value>> {
    let cause = grob::property(tx.host, g, "cause")?;
    Ok(tx
        .heap()
        .foreign::<StreamEvent>(cause)
        .is_some()
        .then_some(cause))
}

/// `in_event_class`: whether the event's classes have `class`.
pub(crate) fn in_event_class(heap: &mut Heap, ev: Value, class: &str) -> bool {
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

/// `has_autosplit_end`.
fn has_autosplit_end(heap: &mut Heap, ev: Value) -> bool {
    is_true(event_property(heap, ev, "autosplit-end"))
}

/// `get_event_length (e, now)`: the event's `length`, in grace time as its
/// grace part.
pub fn event_length(heap: &mut Heap, ev: Value, now: Moment) -> Moment {
    let v = event_property(heap, ev, "length");
    let len = heap.foreign::<Moment>(v).copied().unwrap_or_default();
    if now.grace.is_zero() {
        len
    } else {
        Moment::new(Rational::ZERO, len.main)
    }
}

/// Whether the pitches `p1` and `p2` are `equal?`, or, `enharmonic`, both
/// pitches of the same tones (`tone_pitch`).
fn same_pitch(heap: &Heap, p1: Value, p2: Value, enharmonic: bool) -> bool {
    if !enharmonic {
        return heap.equal(p1, p2);
    }
    let tones = |p: Value| {
        let p = heap.foreign::<Pitch>(p)?;
        p.tones(heap.foreign::<Scale>(p.scale)?)
    };
    matches!((tones(p1), tones(p2)), (Some(a), Some(b)) if a == b)
}

impl TieEngraver {
    /// `report_unterminated_tie`: unless a head of its chord found its own.
    fn report_unterminated_tie(tx: &mut Tx<'_>, w: &Waiting) -> Run<()> {
        if !w.tie_from_chord_created {
            grob::warning(tx.host, w.tie.1, "unterminated tie")?;
            grob::suicide(tx.heap(), w.tie.1);
        }
        Ok(())
    }

    /// `tie_notehead`: `h` tied to the first head waiting for it, if any.
    fn tie_notehead(&mut self, tx: &mut Tx<'_>, h: Value, enharmonic: bool) -> Run<bool> {
        for i in 0..self.heads_to_tie.len() {
            let th = self.heads_to_tie[i].head;
            let right_ev = event_cause(tx, h)?;
            let left_ev = event_cause(tx, th)?;

            // maybe should check positions too.
            let (Some(right_ev), Some(left_ev)) = (right_ev, left_ev) else {
                continue;
            };

            // Make a tie only if pitches are equal or if event end was not
            // generated by Completion_heads_engraver.
            let heap = tx.heap();
            let p1 = event_property(heap, left_ev, "pitch");
            let p2 = event_property(heap, right_ev, "pitch");
            if !same_pitch(heap, p1, p2, enharmonic) || has_autosplit_end(heap, left_ev) {
                continue;
            }
            let w = self.heads_to_tie.remove(i);
            let (id, p) = w.tie;
            let cause = w.tie_event.or(w.tie_stream_event);

            tx.announce_end_grob(id);

            tie::set_head(tx.host, p, -1, th)?;
            tie::set_head(tx.host, p, 1, h)?;

            if let Some(cause) = cause {
                let d = event_property(tx.heap(), cause, "direction");
                if matches!(d.as_fixnum(), Some(-1..=1)) {
                    grob::set_property(tx.heap(), p, "direction", d);
                }
            }

            self.ties.push(p);

            // Prevent all other tied notes ending at the same moment
            // (assume implicitly the notes have also started at the same
            // moment!) from triggering an "unterminated tie" warning.
            // Needed e.g. for <c e g>~ g
            for other in &mut self.heads_to_tie {
                if other.end == w.end {
                    other.tie_from_chord_created = true;
                }
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// `typeset_tie`: the tie's heads its bounds, one for both if it has
    /// one only.
    fn typeset_tie(tx: &mut Tx<'_>, her: Value) -> Run<()> {
        let mut left_head = tie::head(tx.host, her, -1)?;
        let mut right_head = tie::head(tx.host, her, 1)?;

        if left_head.is_none() || right_head.is_none() {
            tx.warning("lonely tie")?;
            if left_head.is_none() {
                left_head = right_head;
            } else {
                right_head = left_head;
            }
        }

        if let Some(h) = left_head {
            spanner::set_bound(tx.host, her, -1, h)?;
        }
        if let Some(h) = right_head {
            spanner::set_bound(tx.host, her, 1, h)?;
        }
        Ok(())
    }

    /// The heads waiting, each reported and given up.
    fn give_up_all(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for w in core::mem::take(&mut self.heads_to_tie) {
            Self::report_unterminated_tie(tx, &w)?;
        }
        Ok(())
    }
}

impl Work for TieEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        if let Some(w) = &self.placeholder {
            w.held(h);
        }
        h.flags(&[self.event_processed]);
        h.num(self.heads_to_tie.len() as i64);
        for w in &self.heads_to_tie {
            h.moment(w.end);
            h.flags(&[
                w.tie_from_chord_created,
                w.tie_stream_event.is_some(),
                w.tie_event.is_some(),
            ]);
            h.record(w.tie.0);
        }
    }

    fn connect(&mut self, tx: &mut Tx<'_>) {
        if placeholder_heads(tx) {
            let class = lily_engine_music::CLASSES
                .iter()
                .find(|c| c.name == TIE_ENGRAVER.name);
            self.placeholder = class.and_then(|c| c.make).map(|make| make());
        }
        if let Some(w) = &mut self.placeholder {
            w.connect(tx);
        }
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        if let Some(w) = &mut self.placeholder {
            return w.listen(tx, which, ev);
        }
        if which == 0 && !is_true(tx.get("skipTypesetting")) {
            assign_event_once(tx, &mut self.event, ev)?;
        }
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(w) = &mut self.placeholder {
            return w.start_translation_timestep(tx);
        }
        if !self.heads_to_tie.is_empty() && !is_true(tx.get("tieWaitForNote")) {
            let now = tx.now();
            for i in (0..self.heads_to_tie.len()).rev() {
                if now.compare(self.heads_to_tie[i].end).is_gt() {
                    let w = self.heads_to_tie.remove(i);
                    Self::report_unterminated_tie(tx, &w)?;
                }
            }
        }
        tx.set("tieMelismaBusy", Value::bool(!self.heads_to_tie.is_empty()));
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(w) = &mut self.placeholder {
            return w.process_music(tx);
        }
        let busy = self.event.is_some()
            || self
                .heads_to_tie
                .iter()
                .any(|w| w.tie_event.is_some() || w.tie_stream_event.is_some());
        if busy {
            tx.set("tieMelismaBusy", Value::TRUE);
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        if self.placeholder.is_some() {
            return Ok(());
        }
        let h = tx.grob_handle(grob);
        self.now_heads.push(h);

        if !self.tie_notehead(tx, h, false)? {
            self.tie_notehead(tx, h, true)?;
        }

        if let Some(&first) = self.ties.first()
            && self.tie_column.is_none()
        {
            let Some((_, first)) = lily_engine_music::grob_of(tx.heap(), first) else {
                return Ok(());
            };
            let id = tx.make_defined_grob("TieColumn", Cause::Grob(first))?;
            self.tie_column = Some(tx.grob_handle(id));
        }

        if let Some(column) = self.tie_column {
            for &t in &self.ties {
                tie_column::add_tie(tx.host, column, t)?;
            }
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(w) = &mut self.placeholder {
            return w.process_acknowledged(tx);
        }
        let wait = is_true(tx.get("tieWaitForNote"));
        if !self.ties.is_empty() {
            if !wait {
                self.give_up_all(tx)?;
            }
            for t in core::mem::take(&mut self.ties) {
                Self::typeset_tie(tx, t)?;
            }
            self.tie_column = None;
        }

        let mut new_heads_to_tie = Vec::new();
        for head in core::mem::take(&mut self.now_heads) {
            let Some(left_ev) = event_cause(tx, head)? else {
                // may happen for ambitus
                continue;
            };

            // We only want real notes to cause ties, not e.g. pitched
            // trills
            let heap = tx.heap();
            if !in_event_class(heap, left_ev, "note-event") {
                continue;
            }

            let mut tie_event = None;
            let tie_stream_event = self.event;
            let mut s = event_property(heap, left_ev, "articulations");
            while let (None, None, Some(ev), Some(rest)) =
                (tie_event, tie_stream_event, heap.car(s), heap.cdr(s))
            {
                if heap.foreign::<StreamEvent>(ev).is_some()
                    && in_event_class(heap, ev, "tie-event")
                {
                    tie_event = Some(ev);
                }
                s = rest;
            }

            let Some(cause) = tie_event.or(tie_stream_event) else {
                continue;
            };
            if has_autosplit_end(heap, left_ev) {
                continue;
            }
            self.event_processed = true;
            let id = tx.make_defined_grob("Tie", Cause::Event(cause))?;
            let tie = (id, tx.grob_handle(id));
            let now = tx.now();
            let len = event_length(tx.heap(), left_ev, now);
            let end = now
                .checked_add(len)
                .unwrap_or(Moment::new(Rational::INFINITY, Rational::ZERO));
            new_heads_to_tie.push(Waiting {
                head,
                end,
                tie_stream_event,
                tie_event,
                tie,
                tie_from_chord_created: false,
            });
        }

        if !wait && !new_heads_to_tie.is_empty() {
            self.give_up_all(tx)?;
        }

        self.heads_to_tie.extend(new_heads_to_tie);
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(w) = &mut self.placeholder {
            return w.stop_translation_timestep(tx);
        }
        // Discard event only if it has been processed with at least one
        // appropriate note.
        if self.event_processed {
            self.event = None;
        }
        self.event_processed = false;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        if let Some(w) = &self.placeholder {
            w.values(out);
        }
        out.extend(self.event);
        out.extend(&self.now_heads);
        for w in &self.heads_to_tie {
            out.extend([w.head, w.tie.1]);
            out.extend(w.tie_stream_event);
            out.extend(w.tie_event);
        }
        out.extend(&self.ties);
        out.extend(self.tie_column);
    }
}
