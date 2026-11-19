//! `mark-engraver.cc` (MS8 step 7, 268 lines): `Mark_engraver`, a class of
//! layout's ([`MARK_ENGRAVER`]) that shadows the music crate's placeholder
//! of its name (D67, D108).
//!
//! - `process_music` (`:205–228`): the mark `Mark_tracking_translator`
//!   chose for the timestep, its rehearsal or ad-hoc mark and its coda
//!   mark, segno mark or section label (`currentRehearsalMarkEvent`,
//!   `currentPerformanceMarkEvent`), each makes its grob (a
//!   `RehearsalMark`, `CodaMark`, `SegnoMark` or `SectionLabel`) caused by
//!   the event, its `text` the getter's (`marks.rs`, which
//!   `Mark_performer` shares), or LilyPond's warning when that is not a
//!   markup.
//! - `stop_translation_timestep` (`:44–70`): a mark of the first timestep
//!   visible wherever it is broken; each mark's `side-support-elements`
//!   the grob array of `stavesFound`; kept as the last mark until the next
//!   timestep starts.
//! - `finalize` (`:72–89`): a mark of the last timestep visible wherever
//!   it is broken too.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Held, Kind, PROCESS, START, STOP, StreamEvent, TranslatorClass, Tx, Work,
    current_performance_mark, current_rehearsal_mark,
};
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::grob_array::grob_list_to_grob_array;
use crate::text::is_markup;

type Run<T> = Result<T, Exception>;

pub static MARK_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Mark_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS | STOP | FINALIZE,
    // It hears nothing: it reads what `Mark_tracking_translator` chose.
    always: true,
    hears: &[],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| {
        Box::new(MarkEngraver {
            first_time: true,
            ..MarkEngraver::default()
        })
    }),
    reads: &[
        "codaMarkFormatter",
        "currentPerformanceMarkEvent",
        "currentRehearsalMarkEvent",
        "rehearsalMarkFormatter",
        "segnoMarkFormatter",
        "stavesFound",
    ],
    writes: &[],
};

/// `Mark_state`: the mark made this timestep (`text_`) and the one made at
/// the last (`final_text_`), by their handles.
#[derive(Clone, Copy, Default)]
struct MarkState {
    text: Option<Value>,
    final_text: Option<Value>,
}

/// `performance_mark_state_`, `rehearsal_mark_state_` and `first_time_`.
#[derive(Clone, Default)]
struct MarkEngraver {
    performance: MarkState,
    rehearsal: MarkState,
    first_time: bool,
}

type Getter = fn(&mut Tx<'_>, Value) -> Run<(&'static str, Value)>;

/// `break-visibility` `#(#t #t #t)`.
fn visible_everywhere(tx: &mut Tx<'_>, mark: Value) {
    let all = tx.heap().vector(vec![Value::TRUE; 3]);
    grob::set_property(tx.heap(), mark, "break-visibility", all);
}

impl MarkEngraver {
    /// `process_mark` of `process_music`: the grob of the mark in
    /// `property`, if there is one and none made yet.
    fn process_mark(tx: &mut Tx<'_>, mark: &mut MarkState, property: &str, get: Getter) -> Run<()> {
        if mark.text.is_some() {
            return Ok(());
        }
        let ev = tx.get(property);
        if tx.heap().foreign::<StreamEvent>(ev).is_none() {
            return Ok(());
        }
        let (name, text) = get(tx, ev)?;
        let depth = tx.heap().root_depth();
        tx.heap().root(text);
        let made = Self::make_mark(tx, ev, name, text);
        tx.heap().unroot_to(depth);
        mark.text = Some(made?);
        Ok(())
    }

    fn make_mark(tx: &mut Tx<'_>, ev: Value, name: &str, text: Value) -> Run<Value> {
        let id = tx.make_defined_grob(name, Cause::Event(ev))?;
        let g = tx.grob_handle(id);
        if is_markup(tx.host, text)? {
            grob::set_property(tx.heap(), g, "text", text);
        } else {
            tx.event_warning(ev, "mark label must be a markup object")?;
        }
        Ok(g)
    }

    /// `process_mark` of `stop_translation_timestep`.
    fn stop_mark(tx: &mut Tx<'_>, mark: &mut MarkState, first_time: bool) -> Run<()> {
        let Some(text) = mark.text else {
            return Ok(());
        };
        if first_time {
            // A mark created at the very beginning is always visible even
            // if it would not be visible at the beginning of a broken line.
            visible_everywhere(tx, text);
        }
        let staves = tx.get("stavesFound");
        let (array, warnings) = grob_list_to_grob_array(tx.heap(), staves);
        let sym = tx.heap().intern("side-support-elements");
        grob::set_object(tx.heap(), text, sym, array);
        for w in warnings {
            tx.warning(&w)?;
        }
        mark.final_text = Some(text);
        mark.text = None;
        Ok(())
    }
}

impl Work for MarkEngraver {
    fn start_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.performance = MarkState::default();
        self.rehearsal = MarkState::default();
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        Self::process_mark(
            tx,
            &mut self.performance,
            "currentPerformanceMarkEvent",
            current_performance_mark,
        )?;
        Self::process_mark(
            tx,
            &mut self.rehearsal,
            "currentRehearsalMarkEvent",
            current_rehearsal_mark,
        )
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        Self::stop_mark(tx, &mut self.performance, self.first_time)?;
        Self::stop_mark(tx, &mut self.rehearsal, self.first_time)?;
        self.first_time = false;
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for mark in [&mut self.performance, &mut self.rehearsal] {
            // A mark created at the very end is always visible even if it
            // would not be visible at the end of a broken line.
            if let Some(last) = mark.final_text.take() {
                visible_everywhere(tx, last);
            }
        }
        Ok(())
    }

    fn held(&self, h: &mut dyn Held) {
        h.flags(&[self.first_time]);
    }

    fn values(&self, out: &mut Vec<Value>) {
        for mark in [&self.performance, &self.rehearsal] {
            out.extend(mark.text);
            out.extend(mark.final_text);
        }
    }
}
