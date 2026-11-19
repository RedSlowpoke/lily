//! `episema-engraver.cc` (MS8 step 9, part 1; 183 lines):
//! `Episema_engraver`, a class of layout's ([`EPISEMA_ENGRAVER`]) that
//! shadows the music crate's placeholder of its name (D67, D108). Its
//! `Episema` is drawn by callbacks the engine has (`ly:line-spanner::print`
//! and the horizontal line spanner's bound infos).
//!
//! - An `episema-event` heard as a `Last_span_event_listener` hears it: the
//!   timestep's last start and last stop by their `span-direction` (an
//!   episema may be typeset over a single neume).
//! - `process_music` (`:58–84`): a start makes an `Episema` caused by it,
//!   with LilyPond's two warnings where one is running; a stop ends the
//!   running one, announced as ended, the note columns forgotten, with
//!   LilyPond's warning where there is none.
//! - `stop_translation_timestep` (`:101–114`): the running one's left bound
//!   its first note column, else `currentMusicalColumn`, where it has none;
//!   the ended one's right bound its last, else the column, where it has
//!   none (`typeset_all`, `:86–99`).
//! - `finalize` (`:116–125`): the ended one typeset; a running one
//!   unterminated, LilyPond's warning at it, and killed.
//! - Each note column acknowledged is kept (`:127–131`); each note head is
//!   the running episema's support and bound, or else the ended one's
//!   (`:133–146`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Held, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::dynamic_engraver::musical_column;
use crate::ledger_line_engraver::set_bound;
use crate::{grob, item, side_position, spanner};

type Run<T> = Result<T, Exception>;

pub static EPISEMA_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Episema_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    always: false,
    hears: &["episema-event"],
    acknowledges: &[
        ("note-column-interface", false),
        ("note-head-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(EpisemaEngraver::default())),
    reads: &["currentMusicalColumn"],
    writes: &[],
};

/// `episema_listener_`'s events, `span_` and `finished_` by their handles
/// and record numbers, and `note_columns_`.
#[derive(Clone, Default)]
struct EpisemaEngraver {
    start: Option<Value>,
    stop: Option<Value>,
    span: Option<(Value, u32)>,
    finished: Option<(Value, u32)>,
    note_columns: Vec<(Value, u32)>,
}

impl EpisemaEngraver {
    /// `typeset_all`.
    fn typeset_all(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((finished, _)) = self.finished.take()
            && spanner::get_bound(tx.host, finished, 1)?.is_none()
        {
            let col = match self.note_columns.last() {
                Some(&(c, _)) => Some(c),
                None => musical_column(tx),
            };
            set_bound(tx, finished, 1, col)?;
        }
        Ok(())
    }
}

impl Work for EpisemaEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        let d = event_property(tx.heap(), ev, "span-direction");
        match tx.heap().to_i64(d) {
            Some(-1) => self.start = Some(ev),
            Some(1) => self.stop = Some(ev),
            _ => tx.programming_error("event span-direction is not set")?,
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(starter) = self.start {
            if let Some((span, _)) = self.span {
                tx.event_warning(starter, "already have an episema")?;
                grob::warning(tx.host, span, "episema was started here")?;
            } else {
                let id = tx.make_defined_grob("Episema", Cause::Event(starter))?;
                self.span = Some((tx.grob_handle(id), id));
            }
        }

        if let Some(ender) = self.stop {
            match self.span.take() {
                None => tx.event_warning(ender, "cannot find start of episema")?,
                Some((span, id)) => {
                    self.finished = Some((span, id));
                    tx.announce_end_grob(id);
                    self.note_columns.clear();
                }
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((span, _)) = self.span
            && spanner::get_bound(tx.host, span, -1)?.is_none()
        {
            let col = match self.note_columns.first() {
                Some(&(c, _)) => Some(c),
                None => musical_column(tx),
            };
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
            grob::warning(tx.host, span, "unterminated episema")?;
            grob::suicide(tx.heap(), span);
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        if which == 0 {
            // acknowledge_note_column, an item's (`Grob_info_t<Item>`)
            if item::is_item(tx.heap(), g) {
                self.note_columns.push((g, grob));
            }
            return Ok(());
        }
        // acknowledge_note_head
        if let Some((span, _)) = self.span.or(self.finished) {
            side_position::add_support(tx.host, span, g)?;
            spanner::add_bound_item(tx.host, span, g)?;
        }
        Ok(())
    }

    fn held(&self, h: &mut dyn Held) {
        h.maybe_record(self.span.map(|s| s.1));
        h.maybe_record(self.finished.map(|s| s.1));
        let ids: Vec<u32> = self.note_columns.iter().map(|c| c.1).collect();
        h.records(&ids);
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.start);
        out.extend(self.stop);
        out.extend(self.span.map(|s| s.0));
        out.extend(self.finished.map(|s| s.0));
        out.extend(self.note_columns.iter().map(|c| c.0));
    }
}
