//! `completion-note-heads-engraver.cc` and `completion-rest-engraver.cc`
//! (MS8 step 8, part 2, OPEN A44; 341 and 296 lines):
//! `Completion_heads_engraver` and `Completion_rest_engraver`, classes of
//! layout's ([`COMPLETION_HEADS_ENGRAVER`], [`COMPLETION_REST_ENGRAVER`])
//! that shadow the music crate's placeholders of their names (D67, D108).
//! LilyPond's two are copies of each other; here they are one, the heads'
//! ties its own.
//!
//! A note (a rest) heard is kept until its predicted end. At each timestep
//! with one to typeset (`process_music`): a duration no longer than what is
//! left to do and, with `timing`, not past the bar line or the
//! `completionUnit` (`next_moment`), at the original's `completionFactor`
//! (a procedure called with the context and the duration); each event,
//! cloned where the duration is not its own, given it, makes a `NoteHead`
//! (a `Rest`) at its pitch's staff position from `middleCPosition`; the
//! heads are tied to the last timestep's when they are as many, the ties in
//! a `TieColumn`; the rest is asked for as a moment to process. Grace notes
//! are not split. `start_translation_timestep` forgets the events past
//! their end and sets `completionBusy` (`restCompletionBusy`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Duration, Held, INITIALIZE, Kind, Moment, PROCESS, Pitch, Rational, START, STOP,
    TranslatorClass, Tx, Work, clone_event, event_property, set_event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::note_heads_engraver::steps;
use crate::paper_column_engraver::rational;
use crate::tie_engraver::event_length;
use crate::{grob, tie, tie_column};

type Run<T> = Result<T, Exception>;

pub static COMPLETION_HEADS_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Completion_heads_engraver",
    kind: Kind::Engraver,
    methods: INITIALIZE | START | PROCESS | STOP,
    always: true,
    hears: &["note-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(Completion::new(true))),
    reads: &[
        "completionFactor",
        "completionUnit",
        "measureLength",
        "measurePosition",
        "middleCPosition",
        "timing",
    ],
    writes: &["completionBusy"],
};

pub static COMPLETION_REST_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Completion_rest_engraver",
    kind: Kind::Engraver,
    methods: INITIALIZE | START | PROCESS | STOP,
    always: true,
    hears: &["rest-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(Completion::new(false))),
    reads: &[
        "completionFactor",
        "completionUnit",
        "measureLength",
        "measurePosition",
        "middleCPosition",
        "timing",
    ],
    writes: &["restCompletionBusy"],
};

/// `notes_` (`rests_`), `prev_notes_`, `ties_` and `tie_column_` by their
/// handles and record numbers, `note_events_`, `note_end_mom_`,
/// `is_first_`, `left_to_do_`, `do_nothing_until_` and `factor_`; `heads`
/// for the note heads' engraver.
#[derive(Clone)]
struct Completion {
    heads: bool,
    notes: Vec<(Value, u32)>,
    prev_notes: Vec<(Value, u32)>,
    ties: Vec<(Value, u32)>,
    tie_column: Option<Value>,
    note_events: Vec<Value>,
    note_end_mom: Moment,
    is_first: bool,
    left_to_do: Rational,
    do_nothing_until: Rational,
    factor: Rational,
}

impl Completion {
    fn new(heads: bool) -> Completion {
        Completion {
            heads,
            notes: Vec::new(),
            prev_notes: Vec::new(),
            ties: Vec::new(),
            tie_column: None,
            note_events: Vec::new(),
            note_end_mom: Moment::default(),
            is_first: false,
            left_to_do: Rational::ZERO,
            do_nothing_until: Rational::ZERO,
            factor: Rational::int(1),
        }
    }

    /// `next_moment`: the duration until the next bar line or completion
    /// unit, 0 without `timing`.
    fn next_moment(tx: &mut Tx<'_>, note_len: Rational) -> Run<Rational> {
        let mut result = Rational::ZERO;
        if tx.get("timing") != Value::TRUE {
            return Ok(result);
        }
        let v = tx.get("measureLength");
        let mlen = rational(tx.heap(), v, Rational::int(1));
        let v = tx.get("measurePosition");
        let position = tx.heap().foreign::<Moment>(v).copied().unwrap_or_default();
        let mpos = crate::bar_engraver::measure_position_of(tx, position, mlen)?;
        result = sub(mlen, mpos.main);
        let v = tx.get("completionUnit");
        let unit = rational(tx.heap(), v, Rational::ZERO);
        if !unit.compare(Rational::ZERO).is_gt() {
            return Ok(result);
        }
        let now_unit = div(mpos.main, unit);
        if now_unit.denominator() > 1 {
            // within a unit - go to the end of that
            let trunc = Rational::int(now_unit.numerator() / now_unit.denominator());
            result = mul(unit, sub(Rational::int(1), sub(now_unit, trunc)));
        } else {
            // at the beginning of a unit: take a power-of-two number of
            // units, but not more than required, since then the Duration
            // constructor destroys the unit structure
            if note_len.compare(result).is_lt() {
                result = note_len;
            }
            let step_unit = div(result, unit);
            if step_unit.denominator() < step_unit.numerator() {
                let n = step_unit.numerator() / step_unit.denominator();
                let log2 = 63 - n.leading_zeros() as i32;
                result = mul(unit, Rational::int(1i64 << log2));
            }
        }
        Ok(result)
    }

    /// `make_note_head` (`make_rest`): a `NoteHead` at the event's pitch,
    /// or 0 without one; a `Rest` at it only with one.
    fn make_head(&self, tx: &mut Tx<'_>, ev: Value) -> Run<(Value, u32)> {
        let name = if self.heads { "NoteHead" } else { "Rest" };
        let id = tx.make_defined_grob(name, Cause::Event(ev))?;
        let g = tx.grob_handle(id);
        let pitch = event_property(tx.heap(), ev, "pitch");
        let pit = tx.heap().foreign::<Pitch>(pitch).copied();
        if self.heads || pit.is_some() {
            let mut pos = pit.map_or(0, |p| steps(tx.heap(), &p));
            let c0 = tx.get("middleCPosition");
            if tx.heap().is_number(c0) {
                pos = pos.wrapping_add(crate::stem::to_int(tx.heap(), c0)?);
            }
            let pos = tx.heap().integer(i64::from(pos));
            grob::set_property(tx.heap(), g, "staff-position", pos);
        }
        Ok((g, id))
    }

    /// `make_tie`.
    fn make_tie(&mut self, tx: &mut Tx<'_>, left: Value, right: Value) -> Run<()> {
        let id = tx.make_defined_grob("Tie", Cause::None)?;
        let p = tx.grob_handle(id);
        tie::set_head(tx.host, p, -1, left)?;
        tie::set_head(tx.host, p, 1, right)?;
        tx.announce_end_grob(id);
        self.ties.push((p, id));
        Ok(())
    }

    fn process(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if !self.is_first && self.left_to_do.is_zero() {
            return Ok(());
        }
        self.is_first = false;

        let now = tx.now();
        if self.do_nothing_until.compare(now.main).is_gt() {
            return Ok(());
        }

        let mut orig: Option<Duration> = None;
        let mut note_dur = if !self.left_to_do.is_zero() {
            // note that note_dur may be strictly less than left_to_do_
            // (say, if left_to_do_ == 5/8)
            scaled(div(self.left_to_do, self.factor), self.factor)
        } else {
            let Some(&first) = self.note_events.first() else {
                return Ok(());
            };
            let v = event_property(tx.heap(), first, "duration");
            let Some(o) = tx.heap().foreign::<Duration>(v).copied() else {
                return Ok(());
            };
            orig = Some(o);
            let mut factor = tx.get("completionFactor");
            if tx.heap().is_procedure(factor) {
                let context = tx.t.arena.borrow().handle(tx.context);
                let d = tx.heap().wrap(o);
                let depth = tx.heap().root_depth();
                tx.heap().root(d);
                let called = tx.apply(factor, &[context, d]);
                tx.heap().unroot_to(depth);
                factor = called?;
            }
            self.factor = rational(tx.heap(), factor, o.factor());
            self.left_to_do = o.length().unwrap_or(Rational::ZERO);
            o
        };

        let nb = Self::next_moment(tx, length(note_dur))?;
        if !nb.is_zero() && nb.compare(length(note_dur)).is_lt() {
            note_dur = scaled(div(nb, self.factor), self.factor);
        }

        self.do_nothing_until = add(now.main, length(note_dur));

        let heap = tx.heap();
        let keys = [
            "pitch",
            "duration",
            "length",
            "duration-log",
            "autosplit-end",
        ]
        .map(|k| heap.intern(k));
        let mut i = 0;
        while !self.left_to_do.is_zero() && i < self.note_events.len() {
            let need_clone =
                orig.is_none_or(|o| o.compare(note_dur) != Some(core::cmp::Ordering::Equal));
            let original = self.note_events[i];
            let heap = tx.heap();
            let event = if need_clone {
                clone_event(heap, original).unwrap_or(original)
            } else {
                original
            };
            let depth = heap.root_depth();
            heap.root(event);
            let pits = event_property(heap, original, "pitch");
            set_event_property(heap, event, keys[0], pits);
            let d = heap.wrap(note_dur);
            set_event_property(heap, event, keys[1], d);
            let len = heap.wrap(Moment::new(length(note_dur), Rational::ZERO));
            set_event_property(heap, event, keys[2], len);
            let log = heap.integer(i64::from(note_dur.log()));
            set_event_property(heap, event, keys[3], log);
            if self.heads {
                // The Completion_heads_engraver splits an event into a
                // group of consecutive events. For each event in the
                // group, the property "autosplit-end" denotes whether the
                // current event was truncated during splitting. Based on
                // "autosplit-end", the Tie_engraver decides whether a tie
                // event should be processed.
                let end = Value::bool(self.left_to_do.compare(length(note_dur)).is_gt());
                set_event_property(heap, event, keys[4], end);
            }
            let made = self.make_head(tx, event);
            tx.heap().unroot_to(depth);
            self.notes.push(made?);
            i += 1;
        }

        if self.heads {
            if self.prev_notes.len() == self.notes.len() {
                for k in 0..self.notes.len() {
                    let (left, right) = (self.prev_notes[k].0, self.notes[k].0);
                    self.make_tie(tx, left, right)?;
                }
            }

            if let Some(&(_, first)) = self.ties.first()
                && self.tie_column.is_none()
            {
                let id = tx.make_defined_grob("TieColumn", Cause::Grob(first))?;
                self.tie_column = Some(tx.grob_handle(id));
            }
            if let Some(column) = self.tie_column {
                for &(t, _) in self.ties.iter().rev() {
                    tie_column::add_tie(tx.host, column, t)?;
                }
            }
        }

        self.left_to_do = sub(self.left_to_do, length(note_dur));
        if !self.left_to_do.is_zero() {
            tx.add_moment(Moment::new(add(now.main, length(note_dur)), Rational::ZERO))?;
        }

        // don't do complicated arithmetic with grace notes.
        if orig.is_some() && !now.grace.is_zero() {
            self.left_to_do = Rational::ZERO;
        }
        Ok(())
    }
}

/// `Duration (r, false).compressed (factor)`, a quarter where either has
/// no answer in the engine's rationals.
fn scaled(r: Rational, factor: Rational) -> Duration {
    Duration::from_length(r, false)
        .and_then(|d| d.compressed(factor))
        .unwrap_or(Duration::new(2, 0))
}

/// `Rational (d)`.
fn length(d: Duration) -> Rational {
    d.length().unwrap_or(Rational::ZERO)
}

fn add(a: Rational, b: Rational) -> Rational {
    a.checked_add(b).unwrap_or(a)
}

fn sub(a: Rational, b: Rational) -> Rational {
    a.checked_sub(b).unwrap_or(a)
}

fn mul(a: Rational, b: Rational) -> Rational {
    a.checked_mul(b).unwrap_or(a)
}

fn div(a: Rational, b: Rational) -> Rational {
    a.checked_div(b).unwrap_or(a)
}

impl Work for Completion {
    fn initialize(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.is_first = false;
        Ok(())
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.note_events.push(ev);
        self.is_first = true;
        let now = tx.now();
        let musiclen = event_length(tx.heap(), ev, now);
        let end = now.checked_add(musiclen).unwrap_or(now);
        if end.compare(self.note_end_mom).is_gt() {
            self.note_end_mom = end;
        }
        self.do_nothing_until = Rational::ZERO;
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let now = tx.now();
        if !self.note_end_mom.main.compare(now.main).is_gt() {
            self.note_events.clear();
            self.prev_notes.clear();
        }
        let busy = Value::bool(!self.note_events.is_empty());
        tx.set(
            if self.heads {
                "completionBusy"
            } else {
                "restCompletionBusy"
            },
            busy,
        );
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.process(tx)
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.ties.clear();
        self.tie_column = None;
        if self.heads && !self.notes.is_empty() {
            self.prev_notes = core::mem::take(&mut self.notes);
        }
        self.notes.clear();
        Ok(())
    }

    fn held(&self, h: &mut dyn Held) {
        let prev: Vec<u32> = self.prev_notes.iter().map(|n| n.1).collect();
        h.records(&prev);
        h.moment(self.note_end_mom);
        h.flags(&[self.is_first]);
        h.rational(self.left_to_do);
        h.rational(self.do_nothing_until);
        h.rational(self.factor);
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.notes.iter().map(|n| n.0));
        out.extend(self.prev_notes.iter().map(|n| n.0));
        out.extend(self.ties.iter().map(|t| t.0));
        out.extend(self.tie_column);
        out.extend(&self.note_events);
    }
}
