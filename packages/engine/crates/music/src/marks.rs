//! LilyPond's `Mark_tracking_translator` (`mark-tracking-translator.cc`),
//! in the music crate as `Timing_translator` is (MS3 step 20): it names no
//! grob, and `engraver-init.ly` consists it in `\layout`'s Score as
//! `performer-init.ly` does in `\midi`'s.
//!
//! It chooses which marks the mark engravers and performers take, a
//! timestep's rehearsal or ad-hoc mark and its coda mark, segno mark or
//! section label, with LilyPond's warnings for a second one; keeps them in
//! `currentRehearsalMarkEvent` and `currentPerformanceMarkEvent` for the
//! timestep; and counts the marks in `rehearsalMark`, `codaMarkCount` and
//! `segnoMarkCount` at its end, the last two set to 0 at the first one.
//! The marks' grobs are layout's `Mark_engraver`'s (MS8 step 7), whose two
//! getters, which `Mark_performer` shares, are here:
//! [`current_rehearsal_mark`] and [`current_performance_mark`].

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_scheme::{Exception, Heap, Value};

use crate::dispatcher::event_property;
use crate::timing::assign_event_once;
use crate::translator::{Kind, STOP, TranslatorClass, Tx, Work};

type Run<T> = Result<T, Exception>;

pub(crate) const CLASS: TranslatorClass = TranslatorClass {
    name: "Mark_tracking_translator",
    kind: Kind::Translator,
    methods: STOP,
    // `stop_translation_timestep` sets the counts at the first timestep.
    always: true,
    hears: &[
        "ad-hoc-mark-event",
        "coda-mark-event",
        "rehearsal-mark-event",
        "section-label-event",
        "segno-mark-event",
    ],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(MarkTracking::default())),
    reads: &["codaMarkCount", "rehearsalMark", "segnoMarkCount"],
    writes: &[
        "codaMarkCount",
        "currentPerformanceMarkEvent",
        "currentRehearsalMarkEvent",
        "rehearsalMark",
        "segnoMarkCount",
    ],
};

/// The index of each event class in [`CLASS`]' `hears`.
const AD_HOC_MARK: usize = 0;
const CODA_MARK: usize = 1;
const REHEARSAL_MARK: usize = 2;
const SECTION_LABEL: usize = 3;
const SEGNO_MARK: usize = 4;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum EventType {
    #[default]
    None,
    AdHocMark,
    DefaultCodaMark,
    DefaultRehearsalMark,
    DefaultSegnoMark,
    SectionLabel,
    SpecificCodaMark,
    SpecificRehearsalMark,
    SpecificSegnoMark,
}

#[derive(Clone)]
struct MarkTracking {
    /// Rehearsal marks, ad-hoc marks.
    rehearsal_ev: Option<Value>,
    rehearsal_ev_type: EventType,
    /// Coda marks, section labels, segno marks.
    performance_ev: Option<Value>,
    performance_ev_type: EventType,
    first_time: bool,
}

impl Default for MarkTracking {
    fn default() -> MarkTracking {
        MarkTracking {
            rehearsal_ev: None,
            rehearsal_ev_type: EventType::None,
            performance_ev: None,
            performance_ev_type: EventType::None,
            first_time: true,
        }
    }
}

/// `from_scm<size_t> (v, 0)`: an exact integer that is no less than 0.
fn size(heap: &Heap, v: Value) -> i64 {
    heap.to_i64(v).filter(|&n| n >= 0).unwrap_or(0)
}

/// An event's `label`, as `from_scm<size_t>` reads it.
fn label(tx: &mut Tx<'_>, ev: Value) -> i64 {
    let v = event_property(tx.heap(), ev, "label");
    size(tx.heap(), v)
}

/// `get_coda_mark_label` and `get_segno_mark_label`: the event's label, or
/// one past the context's `count`.
pub(crate) fn counted_mark_label(tx: &mut Tx<'_>, ev: Value, count: &str) -> i64 {
    let n = label(tx, ev);
    if n >= 1 {
        return n;
    }
    let v = tx.get(count);
    size(tx.heap(), v) + 1
}

/// `get_rehearsal_mark_label`: the event's label, or the context's
/// `rehearsalMark`.
pub(crate) fn rehearsal_mark_label(tx: &mut Tx<'_>, ev: Value) -> i64 {
    let n = label(tx, ev);
    if n >= 1 {
        return n;
    }
    let v = tx.get("rehearsalMark");
    size(tx.heap(), v)
}

impl MarkTracking {
    fn clear_events(&mut self, tx: &mut Tx<'_>) {
        if self.performance_ev_type != EventType::None {
            self.performance_ev = None;
            self.performance_ev_type = EventType::None;
            tx.unset("currentPerformanceMarkEvent");
        }
        if self.rehearsal_ev_type != EventType::None {
            self.rehearsal_ev = None;
            self.rehearsal_ev_type = EventType::None;
            tx.unset("currentRehearsalMarkEvent");
        }
    }

    fn set_performance_event(&mut self, tx: &mut Tx<'_>, ty: EventType, ev: Value) {
        self.performance_ev = Some(ev);
        self.performance_ev_type = ty;
        tx.set("currentPerformanceMarkEvent", ev);
    }

    fn set_rehearsal_event(&mut self, tx: &mut Tx<'_>, ty: EventType, ev: Value) {
        self.rehearsal_ev = Some(ev);
        self.rehearsal_ev_type = ty;
        tx.set("currentRehearsalMarkEvent", ev);
    }

    fn set_performance_event_once(&mut self, tx: &mut Tx<'_>, ty: EventType, ev: Value) -> Run<()> {
        let mut slot = self.performance_ev;
        let assigned = assign_event_once(tx, &mut slot, ev)?;
        self.performance_ev = slot;
        if assigned {
            self.set_performance_event(tx, ty, ev);
        }
        Ok(())
    }

    fn set_rehearsal_event_once(&mut self, tx: &mut Tx<'_>, ty: EventType, ev: Value) -> Run<()> {
        let mut slot = self.rehearsal_ev;
        let assigned = assign_event_once(tx, &mut slot, ev)?;
        self.rehearsal_ev = slot;
        if assigned {
            self.set_rehearsal_event(tx, ty, ev);
        }
        Ok(())
    }

    /// `listen_coda_mark` and `listen_segno_mark`, which differ in their
    /// event types only.
    fn listen_structural(
        &mut self,
        tx: &mut Tx<'_>,
        ev: Value,
        default: EventType,
        specific: EventType,
    ) -> Run<()> {
        let label = event_property(tx.heap(), ev, "label");
        let ty = self.performance_ev_type;
        if !tx.heap().is_integer(label) {
            // A default mark at the beginning of a piece is ignored, and one
            // after any of its kind silently.
            if !self.first_time && ty != default && ty != specific {
                self.set_performance_event_once(tx, default, ev)?;
            }
        } else if ty == default {
            // A specific mark silently replaces a default one.
            self.set_performance_event(tx, specific, ev);
        } else {
            self.set_performance_event_once(tx, specific, ev)?;
        }
        Ok(())
    }

    fn listen_rehearsal_mark(&mut self, tx: &mut Tx<'_>, ev: Value) -> Run<()> {
        let label = event_property(tx.heap(), ev, "label");
        let ty = self.rehearsal_ev_type;
        if !tx.heap().is_integer(label) {
            // A default mark after any rehearsal mark is ignored silently.
            if ty != EventType::DefaultRehearsalMark && ty != EventType::SpecificRehearsalMark {
                self.set_rehearsal_event_once(tx, EventType::DefaultRehearsalMark, ev)?;
            }
        } else if ty == EventType::DefaultRehearsalMark {
            self.set_rehearsal_event(tx, EventType::SpecificRehearsalMark, ev);
        } else {
            self.set_rehearsal_event_once(tx, EventType::SpecificRehearsalMark, ev)?;
        }
        Ok(())
    }
}

impl Work for MarkTracking {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.num(self.rehearsal_ev_type as i64);
        h.num(self.performance_ev_type as i64);
        h.flags(&[self.first_time]);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        match which {
            // Ad-hoc marks make RehearsalMark grobs, so any other mark
            // conflicts with one.
            AD_HOC_MARK => self.set_rehearsal_event_once(tx, EventType::AdHocMark, ev),
            CODA_MARK => self.listen_structural(
                tx,
                ev,
                EventType::DefaultCodaMark,
                EventType::SpecificCodaMark,
            ),
            REHEARSAL_MARK => self.listen_rehearsal_mark(tx, ev),
            SECTION_LABEL => self.set_performance_event_once(tx, EventType::SectionLabel, ev),
            SEGNO_MARK => self.listen_structural(
                tx,
                ev,
                EventType::DefaultSegnoMark,
                EventType::SpecificSegnoMark,
            ),
            _ => Ok(()),
        }
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // No longer at the beginning.
        if self.first_time {
            let zero = tx.heap().integer(0);
            tx.set("codaMarkCount", zero);
            tx.set("segnoMarkCount", zero);
        }
        // The coda and segno counts move on at the end of the timestep, so
        // that they hold during iteration and `process_music`; the
        // rehearsal mark's is `rehearsalMark`, which the user may set.
        let count = match self.performance_ev_type {
            EventType::DefaultCodaMark | EventType::SpecificCodaMark => Some("codaMarkCount"),
            EventType::DefaultSegnoMark | EventType::SpecificSegnoMark => Some("segnoMarkCount"),
            _ => None,
        };
        if let (Some(count), Some(ev)) = (count, self.performance_ev) {
            let label = counted_mark_label(tx, ev, count);
            if label > 0 {
                let v = tx.heap().integer(label);
                tx.set(count, v);
            }
        }
        if matches!(
            self.rehearsal_ev_type,
            EventType::DefaultRehearsalMark | EventType::SpecificRehearsalMark
        ) && let Some(ev) = self.rehearsal_ev
        {
            let label = rehearsal_mark_label(tx, ev);
            if label > 0 {
                let v = tx.heap().integer(label + 1);
                tx.set("rehearsalMark", v);
            }
        }
        self.clear_events(tx);
        self.first_time = false;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.rehearsal_ev);
        out.extend(self.performance_ev);
    }
}

/// `in_event_class`.
fn in_class(tx: &mut Tx<'_>, ev: Value, class: &str) -> bool {
    let heap = tx.heap();
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

/// The formatter `formatter`'s text of the label `label`, if it is one
/// and the formatter a procedure; `'()` else.
fn format_label(tx: &mut Tx<'_>, formatter: &str, label: i64) -> Run<Value> {
    if label <= 0 {
        return Ok(Value::NULL);
    }
    let proc = tx.get(formatter);
    if !tx.heap().is_procedure(proc) {
        return Ok(Value::NULL);
    }
    let label = tx.heap().integer(label);
    let context = tx.t.arena.borrow().handle(tx.context);
    tx.apply(proc, &[label, context])
}

/// `Mark_engraver::get_current_performance_mark` of the event `ev`,
/// `currentPerformanceMarkEvent`: its grob's name and text, a coda's or a
/// segno's formatted label, a section label's text.
pub fn current_performance_mark(tx: &mut Tx<'_>, ev: Value) -> Run<(&'static str, Value)> {
    if in_class(tx, ev, "coda-mark-event") {
        let label = counted_mark_label(tx, ev, "codaMarkCount");
        Ok(("CodaMark", format_label(tx, "codaMarkFormatter", label)?))
    } else if in_class(tx, ev, "section-label-event") {
        Ok(("SectionLabel", event_property(tx.heap(), ev, "text")))
    } else if in_class(tx, ev, "segno-mark-event") {
        let label = counted_mark_label(tx, ev, "segnoMarkCount");
        Ok(("SegnoMark", format_label(tx, "segnoMarkFormatter", label)?))
    } else {
        Ok(("", Value::NULL))
    }
}

/// `Mark_engraver::get_current_rehearsal_mark` of the event `ev`,
/// `currentRehearsalMarkEvent`: a `RehearsalMark` with a rehearsal mark's
/// formatted label or an ad-hoc mark's text.
pub fn current_rehearsal_mark(tx: &mut Tx<'_>, ev: Value) -> Run<(&'static str, Value)> {
    if in_class(tx, ev, "rehearsal-mark-event") {
        let label = rehearsal_mark_label(tx, ev);
        Ok((
            "RehearsalMark",
            format_label(tx, "rehearsalMarkFormatter", label)?,
        ))
    } else {
        Ok(("RehearsalMark", event_property(tx.heap(), ev, "text")))
    }
}
