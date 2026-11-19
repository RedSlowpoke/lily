//! `text-spanner-engraver.cc` (MS3 step 52, 177 lines; in no part of the
//! split, ported here by the user's answer to the step's stop):
//! `Text_spanner_engraver`, a class of layout's ([`TEXT_SPANNER_ENGRAVER`])
//! that the core passes at its `\layout` call and that a group finds
//! before the music crate's placeholder of the name (D67, D108).
//!
//! - A `text-span-event` heard, the timestep's first start and stop by
//!   their `span-direction` (`text_span_listener_`, a
//!   `Unique_span_event_listener`, as `dynamic_engraver.rs` hears its own).
//! - `process_music` (`:56–89`): a stop ends the running spanner, with
//!   LilyPond's warning where there is none; a start makes a `TextSpanner`
//!   caused by it, its `direction` the event's where that is one, on the Y
//!   axis of its side position, with LilyPond's two warnings where one is
//!   running.
//! - `stop_translation_timestep` (`:91–116`): the running spanner's left
//!   bound and the ended one's right are `currentMusicalColumn` where they
//!   have none.
//! - `finalize` (`:118–128`): a running spanner is unterminated, LilyPond's
//!   warning at it, and killed.
//! - Each note column acknowledged (`:130–148`) is one of the running
//!   spanner's `note-columns` and its left bound where it has none, or
//!   else the ended one's and its right bound.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, assign_event_once,
    event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::dynamic_engraver::musical_column;
use crate::geometry::Y;
use crate::ledger_line_engraver::set_bound;
use crate::{grob, grob_array, side_position, spanner};

type Run<T> = Result<T, Exception>;

pub static TEXT_SPANNER_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Text_spanner_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    always: false,
    hears: &["text-span-event"],
    acknowledges: &[("note-column-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(TextSpannerEngraver::default())),
    reads: &["currentMusicalColumn"],
    writes: &[],
};

/// `text_span_listener_`'s start and stop events, and `span_` and
/// `finished_` by their handles and record numbers.
#[derive(Clone, Default)]
struct TextSpannerEngraver {
    start: Option<Value>,
    stop: Option<Value>,
    span: Option<(Value, u32)>,
    finished: Option<(Value, u32)>,
}

impl TextSpannerEngraver {
    /// `typeset_all`: the ended spanner's right bound
    /// `currentMusicalColumn` where it has none, and it forgotten.
    fn typeset_all(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((finished, _)) = self.finished.take()
            && spanner::get_bound(tx.host, finished, 1)?.is_none()
        {
            let col = musical_column(tx);
            set_bound(tx, finished, 1, col)?;
        }
        Ok(())
    }
}

impl Work for TextSpannerEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.maybe_record(self.span.map(|s| s.1));
        h.maybe_record(self.finished.map(|s| s.1));
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        let d = event_property(tx.heap(), ev, "span-direction");
        match tx.heap().to_i64(d) {
            Some(-1) => assign_event_once(tx, &mut self.start, ev).map(drop),
            Some(1) => assign_event_once(tx, &mut self.stop, ev).map(drop),
            _ => tx.programming_error("event span-direction is not set"),
        }
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(ender) = self.stop {
            match self.span.take() {
                None => tx.event_warning(ender, "cannot find start of text spanner")?,
                Some((span, id)) => {
                    self.finished = Some((span, id));
                    tx.announce_end_grob(id);
                }
            }
        }

        if let Some(starter) = self.start {
            if let Some((span, _)) = self.span {
                tx.event_warning(starter, "already have a text spanner")?;
                grob::warning(tx.host, span, "text spanner was started here")?;
            } else {
                let id = tx.make_defined_grob("TextSpanner", Cause::Event(starter))?;
                let span = tx.grob_handle(id);
                self.span = Some((span, id));

                let d_scm = event_property(tx.heap(), starter, "direction");
                if matches!(d_scm.as_fixnum(), Some(-1 | 1)) {
                    grob::set_property(tx.heap(), span, "direction", d_scm);
                }

                side_position::set_axis(tx.host, span, Y)?;
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((span, _)) = self.span
            && spanner::get_bound(tx.host, span, -1)?.is_none()
        {
            let col = musical_column(tx);
            set_bound(tx, span, -1, col)?;
        }

        self.typeset_all(tx)?;
        self.start = None;
        self.stop = None;
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.typeset_all(tx)?;
        if let Some((span, _)) = self.span.take() {
            grob::warning(tx.host, span, "unterminated text spanner")?;
            grob::suicide(tx.heap(), span);
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let column = tx.grob_handle(grob);
        let sym = tx.heap().intern("note-columns");
        if let Some((span, _)) = self.span {
            grob_array::add_grob(tx.host, span, sym, column)?;
            if spanner::get_bound(tx.host, span, -1)?.is_none() {
                spanner::add_bound_item(tx.host, span, column)?;
            }
        } else if let Some((finished, _)) = self.finished {
            grob_array::add_grob(tx.host, finished, sym, column)?;
            if spanner::get_bound(tx.host, finished, 1)?.is_none() {
                spanner::add_bound_item(tx.host, finished, column)?;
            }
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.start);
        out.extend(self.stop);
        out.extend(self.span.map(|(v, _)| v));
        out.extend(self.finished.map(|(v, _)| v));
    }
}
