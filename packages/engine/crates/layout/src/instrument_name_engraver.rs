//! `instrument-name-engraver.cc` (MS3 step 43): `Instrument_name_engraver`,
//! a class of layout's ([`INSTRUMENT_NAME_ENGRAVER`]) that the core passes
//! at its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - `process_music` (`:67–71`, `consider_start_spanner`, `:73–99`), at
//!   every timestep: `instrumentName` and `shortInstrumentName`, or
//!   `vocalName` and `shortVocalName` when neither is a markup
//!   (`text.rs`' `is_markup`); when one of the two is a markup and no
//!   name runs or either is not the running one's (`eq?`), the running
//!   name stops and an `InstrumentName` starts (`start_spanner`,
//!   `:101–119`): made with no cause, `currentCommandColumn` its left
//!   bound, the short name its `text` and the long its `long-text`, an
//!   element of `rootSystem` (else LilyPond's programming error "cannot
//!   find root system" at the grob).
//! - Each axis group acknowledged (`:121–133`) is kept, a spaceable one
//!   (`page_layout_problem.rs`' `is_spaceable`) among the supports, any
//!   other among the backups, used when no spaceable one is.
//! - `stop_spanner` (`:142–159`), at a new name and at `finalize`
//!   (`:135–140`): the kept groups (or the backups) added to the name's
//!   `elements`, `currentCommandColumn` its right bound, the array
//!   unordered. The groups are kept across names, as in LilyPond; the
//!   name's end is not announced.
//!
//! The name's stencil and offsets are output-lib.scm's
//! `system-start-text::print`, `::calc-x-offset` and `::calc-y-offset`,
//! from the image. Without a command column, where LilyPond's would set a
//! null bound, none is set (`ledger_line_engraver::set_bound`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, FINALIZE, Kind, PROCESS, TranslatorClass, Tx, Work, grob_of};
use lily_engine_scheme::{Exception, Value};

use crate::axis_group;
use crate::grob;
use crate::grob_array;
use crate::ledger_line_engraver::{command_column, set_bound};
use crate::page_layout_problem::is_spaceable;
use crate::text::is_markup;

type Run<T> = Result<T, Exception>;

pub static INSTRUMENT_NAME_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Instrument_name_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | FINALIZE,
    // `process_music` reads the names at every timestep.
    always: true,
    hears: &[],
    acknowledges: &[("hara-kiri-group-spanner-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(InstrumentNameEngraver::new())),
    reads: &[
        "currentCommandColumn",
        "instrumentName",
        "shortInstrumentName",
        "shortVocalName",
        "vocalName",
    ],
    writes: &[],
};

/// `text_spanner_` by its handle, `long_text_`, `short_text_`,
/// `axis_groups_` and `backup_axis_groups_`.
#[derive(Clone)]
struct InstrumentNameEngraver {
    text_spanner: Option<Value>,
    long_text: Value,
    short_text: Value,
    axis_groups: Vec<Value>,
    backup_axis_groups: Vec<Value>,
}

impl InstrumentNameEngraver {
    fn new() -> InstrumentNameEngraver {
        InstrumentNameEngraver {
            text_spanner: None,
            long_text: Value::NULL,
            short_text: Value::NULL,
            axis_groups: Vec::new(),
            backup_axis_groups: Vec::new(),
        }
    }

    fn consider_start_spanner(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let mut long_text = tx.get("instrumentName");
        let mut short_text = tx.get("shortInstrumentName");

        if !(is_markup(tx.host, long_text)? || is_markup(tx.host, short_text)?) {
            long_text = tx.get("vocalName");
            short_text = tx.get("shortVocalName");
        }

        if (is_markup(tx.host, long_text)? || is_markup(tx.host, short_text)?)
            && (self.text_spanner.is_none()
                || self.short_text != short_text
                || self.long_text != long_text)
        {
            if self.text_spanner.is_some() {
                self.stop_spanner(tx)?;
            }

            self.short_text = short_text;
            self.long_text = long_text;

            self.start_spanner(tx)?;
        }
        Ok(())
    }

    fn start_spanner(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let id = tx.make_defined_grob("InstrumentName", Cause::None)?;
        let span = tx.grob_handle(id);
        self.text_spanner = Some(span);

        let col = command_column(tx);
        set_bound(tx, span, -1, col)?;
        grob::set_property(tx.heap(), span, "text", self.short_text);
        grob::set_property(tx.heap(), span, "long-text", self.long_text);

        /*
          UGH, should handle this in Score_engraver.
        */
        let system = tx.get("rootSystem");
        if grob_of(tx.heap(), system).is_some() {
            axis_group::add_element(tx.host, system, span)
        } else {
            grob::programming_error(tx.host, span, "cannot find root system")
        }
    }

    fn stop_spanner(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(span) = self.text_spanner.take() else {
            return Ok(());
        };
        if self.axis_groups.is_empty() {
            self.axis_groups = self.backup_axis_groups.clone();
        }

        let elements = tx.heap().intern("elements");
        for &g in &self.axis_groups {
            grob_array::add_grob(tx.host, span, elements, g)?;
        }

        let col = command_column(tx);
        set_bound(tx, span, 1, col)?;

        grob_array::set_ordered(tx.host, span, elements, false)
    }
}

impl Work for InstrumentNameEngraver {
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.consider_start_spanner(tx)
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        if is_spaceable(tx.host, g)? {
            self.axis_groups.push(g);
        } else {
            // By default, don't include non-spaceable staves in the
            // support of an instrument name.  However, if the only staves
            // are non-spaceable, we'll fall back to using them.
            self.backup_axis_groups.push(g);
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.stop_spanner(tx)
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.text_spanner);
        out.push(self.long_text);
        out.push(self.short_text);
        out.extend(&self.axis_groups);
        out.extend(&self.backup_axis_groups);
    }
}
