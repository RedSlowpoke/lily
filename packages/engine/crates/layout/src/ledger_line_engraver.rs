//! `ledger-line-engraver.cc` (MS3 step 16): `Ledger_line_engraver`, a class
//! of layout's ([`LEDGER_LINE_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - `process_music` (`:79–87`): the `LedgerLineSpanner`, with no cause,
//!   made at the first timestep, `currentCommandColumn` its left bound
//!   (`start_spanner`, `:52–60`). As it runs whether or not the engraver
//!   heard anything, the class runs always.
//! - Each staff symbol acknowledged (`:106–114`) whose left bound is not
//!   the spanner's: the spanner ended and another started, as at each
//!   `\startStaff` after the first staff symbol (MS3 step 18's probe).
//! - The ledgered grobs acknowledged (`:116–120`) added to the spanner's
//!   `note-heads` at the timestep's end unless their `no-ledgers` is `#t`
//!   (`stop_translation_timestep`, `:62–77`).
//! - `finalize` (`:89–93`): `currentCommandColumn` the spanner's right
//!   bound (`stop_spanner`, `:95–104`).
//!
//! LilyPond's acknowledger of a staff symbol asserts a `Spanner`; a staff
//! symbol that is no spanner is taken here as one without a left bound.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, FINALIZE, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, grob_of};
use lily_engine_scheme::{Exception, Value};

use crate::geometry::Direction;
use crate::grob;
use crate::grob_array;
use crate::spanner;

type Run<T> = Result<T, Exception>;

pub static LEDGER_LINE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Ledger_line_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    always: true,
    hears: &[],
    acknowledges: &[
        ("ledgered-interface", false),
        ("staff-symbol-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(LedgerLineEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `span_` and `ledgered_grobs_`, by their handles.
#[derive(Clone, Default)]
struct LedgerLineEngraver {
    span: Option<Value>,
    ledgered: Vec<Value>,
}

/// `unsmob<Grob> (get_property (this, "currentCommandColumn"))`.
pub(crate) fn command_column(tx: &mut Tx<'_>) -> Option<Value> {
    let col = tx.get("currentCommandColumn");
    grob_of(tx.heap(), col).map(|_| col)
}

/// `set_bound` of `col`; without a command column, where LilyPond's
/// crashes, nothing.
pub(crate) fn set_bound(tx: &mut Tx<'_>, span: Value, d: Direction, col: Option<Value>) -> Run<()> {
    match col {
        Some(col) => spanner::set_bound(tx.host, span, d, col),
        None => Ok(()),
    }
}

impl LedgerLineEngraver {
    fn start_spanner(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let id = tx.make_defined_grob("LedgerLineSpanner", Cause::None)?;
        let span = tx.grob_handle(id);
        self.span = Some(span);
        let col = command_column(tx);
        set_bound(tx, span, -1, col)
    }

    fn stop_spanner(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(span) = self.span.take() {
            let col = command_column(tx);
            set_bound(tx, span, 1, col)?;
        }
        Ok(())
    }
}

impl Work for LedgerLineEngraver {
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.span.is_none() {
            self.start_spanner(tx)?;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        if which == 0 {
            self.ledgered.push(g);
            return Ok(());
        }
        let restart = match self.span {
            None => true,
            Some(span) => {
                let left = spanner::get_bound(tx.host, span, -1)?;
                let theirs = if spanner::is_spanner(tx.heap(), g) {
                    spanner::get_bound(tx.host, g, -1)?
                } else {
                    None
                };
                left != theirs
            }
        };
        if restart {
            self.stop_spanner(tx)?;
            self.start_spanner(tx)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let ledgered = core::mem::take(&mut self.ledgered);
        let Some(span) = self.span else {
            return Ok(());
        };
        let sym = tx.heap().intern("note-heads");
        for g in ledgered {
            if grob::property(tx.host, g, "no-ledgers")? != Value::TRUE {
                grob_array::add_grob(tx.host, span, sym, g)?;
            }
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.stop_spanner(tx)
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.span);
        out.extend(&self.ledgered);
    }
}
