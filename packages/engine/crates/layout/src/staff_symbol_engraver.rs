//! `staff-symbol-engraver.cc` (MS3 step 18): `Staff_symbol_engraver`, a
//! class of layout's ([`STAFF_SYMBOL_ENGRAVER`]) that the core passes at
//! its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - `\startStaff` and `\stopStaff` heard (`staff_span_listener_`, a
//!   `Unique_span_event_listener`, `span-event-listener.hh:54–85`): a
//!   timestep's first start and first stop by their `span-direction`, with
//!   `assign_event_once`'s warnings, and LilyPond's programming error for
//!   an event with none.
//! - `process_music` (`:65–79`): a stop ends the `StaffSymbol`; a start,
//!   or the first timestep that does not stop, makes one
//!   (`start_spanner`, `:81–92`), with no cause, `currentCommandColumn`
//!   its left bound and itself its `staff-symbol`. As it must run at the
//!   first timestep whether or not the engraver heard anything, the class
//!   runs always.
//! - `stop_translation_timestep` (`:114–122`) and `finalize` (`:124–130`)
//!   end the one that was stopped (`stop_spanner`, `:94–112`):
//!   `currentCommandColumn` its right bound unless it has one, its end
//!   announced. LilyPond gives the stop event as the end's cause, which
//!   `Engraver::make_grob_info` (`engraver.cc:92–105`) would set on a grob
//!   with none, but both callers have emptied the listener first: the
//!   cause is always `'()`.
//! - Each grob acknowledged (`:135–143`) gets the running or the stopped
//!   staff symbol as its `staff-symbol`.
//!
//! Without a command column, where LilyPond's would crash, no bound is set
//! (`ledger_line_engraver::set_bound`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, assign_event_once,
    event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::ledger_line_engraver::{command_column, set_bound};
use crate::spanner;

type Run<T> = Result<T, Exception>;

pub static STAFF_SYMBOL_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Staff_symbol_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    always: true,
    hears: &["staff-span-event"],
    acknowledges: &[("grob-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(StaffSymbolEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `staff_span_listener_`'s start and stop events, `span_` and
/// `finished_span_` by their handles and record numbers, and
/// `first_start_`.
#[derive(Clone)]
struct StaffSymbolEngraver {
    start: Option<Value>,
    stop: Option<Value>,
    span: Option<(Value, u32)>,
    finished: Option<(Value, u32)>,
    first_start: bool,
}

impl Default for StaffSymbolEngraver {
    fn default() -> Self {
        StaffSymbolEngraver {
            start: None,
            stop: None,
            span: None,
            finished: None,
            first_start: true,
        }
    }
}

impl StaffSymbolEngraver {
    fn start_spanner(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.span.is_none() {
            let id = tx.make_defined_grob("StaffSymbol", Cause::None)?;
            let span = tx.grob_handle(id);
            self.span = Some((span, id));
            let col = command_column(tx);
            set_bound(tx, span, -1, col)?;
            // A StaffSymbol's staff symbol is itself.
            let sym = tx.heap().intern("staff-symbol");
            grob::set_object(tx.heap(), span, sym, span);
        }
        Ok(())
    }

    fn stop_spanner(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some((span, id)) = self.finished else {
            return Ok(());
        };
        if spanner::get_bound(tx.host, span, 1)?.is_none() {
            let col = command_column(tx);
            set_bound(tx, span, 1, col)?;
        }
        tx.announce_end_grob(id);
        self.finished = None;
        Ok(())
    }
}

impl Work for StaffSymbolEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.maybe_record(self.span.map(|s| s.1));
        h.maybe_record(self.finished.map(|s| s.1));
        h.flags(&[self.first_start]);
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
        if self.stop.is_some() {
            self.finished = self.span.take();
            if self.first_start {
                self.first_start = false;
            }
        }

        if self.start.is_some() || (self.first_start && self.stop.is_none()) {
            self.start_spanner(tx)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if (self.start.is_some() || self.first_start) && self.span.is_some() {
            self.first_start = false;
        }

        self.start = None;
        self.stop = None;
        self.stop_spanner(tx)
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.finished = self.span.take();
        self.stop_spanner(tx)
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        if let Some((my, _)) = self.span.or(self.finished) {
            let g = tx.grob_handle(grob);
            let sym = tx.heap().intern("staff-symbol");
            grob::set_object(tx.heap(), g, sym, my);
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
