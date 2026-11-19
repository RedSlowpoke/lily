//! `ottava-engraver.cc` (MS3 step 20): `Ottava_spanner_engraver`, a class
//! of layout's ([`OTTAVA_SPANNER_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - `start_translation_timestep` (`:75–78`) sets `ottavaStartNow` to `'()`.
//! - An `ottava-event` heard (`:80–88`): its `ottava-number` kept, the
//!   context's `middleCOffset` set to −7 times it and `middleCPosition`
//!   from it by `set_middle_C` (`pitch.cc:315`), which is the image's
//!   `ly:set-middle-C!` (`context.rs`), as `\clef`'s Scheme sets it.
//! - `process_music` (`:121–133`): at an event the running bracket is
//!   finished, and an ottavation other than 0 sets `ottavaStartNow` to
//!   `#t` and makes an `OttavaBracket` caused by the event
//!   (`create_spanner`, `:90–119`): its `text` the context's `ottavation`,
//!   else the ottavation's entry of `ottavationMarkups` (LilyPond's warning
//!   and `""` where it has none), and its `direction` down where
//!   `middleCOffset` is above 0, else up, each only where the bracket has
//!   none (a tweak or override).
//! - Each note column acknowledged (`:135–147`) is the running bracket's
//!   support and, where it has none, its left bound; a percent repeat
//!   (`:149–155`) disqualifies the last note column as the right bound,
//!   and a trill spanner the timestep's.
//! - `stop_translation_timestep` (`:187–208`) gives the running bracket
//!   `currentMusicalColumn` where it has no left bound, and the finished
//!   one its right bound, the last note column or else
//!   `currentCommandColumn` (`typeset_all`, `:163–185`), or kills it where
//!   it has no left bound. `finalize` (`:210–216`) finishes the running
//!   bracket so.
//!
//! The print, `ly:ottava-bracket::print`, is part 4's. Without a column,
//! where LilyPond's would set a null bound, none is set
//! (`ledger_line_engraver::set_bound`). An `ottava-number` beyond `int`,
//! Guile's out-of-range error, is given as its type error.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Kind, PROCESS, START, STOP, TranslatorClass, Tx, Work, event_property, grob_of,
};
use lily_engine_scheme::{Exception, Value};

use crate::accidental_placement::assoc;
use crate::dynamic_engraver::musical_column;
use crate::grob;
use crate::ledger_line_engraver::{command_column, set_bound};
use crate::side_position;
use crate::spanner;

type Run<T> = Result<T, Exception>;

pub static OTTAVA_SPANNER_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Ottava_spanner_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS | STOP | FINALIZE,
    // `start_translation_timestep` sets `ottavaStartNow` at every timestep,
    // the first among them.
    always: true,
    hears: &["ottava-event"],
    acknowledges: &[
        ("note-column-interface", false),
        ("percent-repeat-interface", false),
        ("trill-spanner-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(OttavaSpannerEngraver::default())),
    reads: &["middleCOffset", "ottavation", "currentMusicalColumn"],
    writes: &[],
};

/// The acknowledgers' indices in [`OTTAVA_SPANNER_ENGRAVER`].
const NOTE_COLUMN: usize = 0;
const PERCENT_REPEAT: usize = 1;

/// `ottava_ev_`, `ottavation_`, `note_col_`, `last_note_col_`,
/// `acked_trill_`, and `span_` and `finished_` by their handles.
#[derive(Clone)]
struct OttavaSpannerEngraver {
    ottava_ev: Option<Value>,
    ottavation: Value,
    note_col: Option<Value>,
    last_note_col: Option<Value>,
    acked_trill: bool,
    span: Option<Value>,
    finished: Option<Value>,
}

impl Default for OttavaSpannerEngraver {
    fn default() -> OttavaSpannerEngraver {
        OttavaSpannerEngraver {
            ottava_ev: None,
            ottavation: Value::NULL,
            note_col: None,
            last_note_col: None,
            acked_trill: false,
            span: None,
            finished: None,
        }
    }
}

/// `from_scm<int>`: Guile's type error for what is no exact integer.
fn to_int(tx: &mut Tx<'_>, v: Value) -> Run<i64> {
    let heap = tx.heap();
    match heap.to_i64(v) {
        Some(n) if heap.is_exact_integer(v) && i32::try_from(n).is_ok() => Ok(n),
        _ => Err(crate::wrong_type(heap, None, 0, v, "exact integer")),
    }
}

/// `get_property_data (grob, name)`: no callback called.
fn property_data(tx: &mut Tx<'_>, grob: Value, name: &str) -> Value {
    let heap = tx.heap();
    let sym = heap.intern(name);
    match grob_of(heap, grob) {
        Some((arena, id)) => grob::property_data(heap, &arena, id, grob, sym),
        None => Value::NULL,
    }
}

impl OttavaSpannerEngraver {
    /// `create_spanner` (`:90–119`).
    fn create_spanner(&mut self, tx: &mut Tx<'_>, ev: Value) -> Run<()> {
        let id = tx.make_defined_grob("OttavaBracket", Cause::Event(ev))?;
        let span = tx.grob_handle(id);
        self.span = Some(span);

        // Respect user tweaks.
        if property_data(tx, span, "text") == Value::NULL {
            let mut ott = tx.get("ottavation");
            if ott == Value::NULL {
                let markups = tx.get("ottavationMarkups");
                let heap = tx.heap();
                ott = assoc(heap, self.ottavation, markups)
                    .and_then(|entry| heap.cdr(entry))
                    .unwrap_or(Value::NULL);
                if ott == Value::NULL {
                    let n = to_int(tx, self.ottavation)?;
                    tx.warning(&format!(
                        "Could not find ottavation markup for {n} octaves up."
                    ))?;
                    ott = tx.heap().string("");
                }
            }
            grob::set_property(tx.heap(), span, "text", ott);
        }

        if property_data(tx, span, "direction") == Value::NULL {
            let offset = tx.get("middleCOffset").as_fixnum().unwrap_or(0);
            let d = if offset > 0 { -1 } else { 1 };
            grob::set_property(tx.heap(), span, "direction", Value::fixnum(d).unwrap());
        }
        Ok(())
    }

    /// `typeset_all` (`:163–185`).
    fn typeset_all(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(finished) = self.finished.take() {
            if spanner::get_bound(tx.host, finished, -1)?.is_some() {
                if spanner::get_bound(tx.host, finished, 1)?.is_none() {
                    // Usually, end the bracket just after the last note head.
                    let col = match self.last_note_col {
                        Some(col) => Some(col),
                        None => command_column(tx),
                    };
                    set_bound(tx, finished, 1, col)?;
                }
            } else {
                grob::suicide(tx.heap(), finished);
            }
            self.last_note_col = None;
        }
        Ok(())
    }
}

impl Work for OttavaSpannerEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[self.acked_trill]);
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        tx.set("ottavaStartNow", Value::NULL);
        Ok(())
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.ottavation = event_property(tx.heap(), ev, "ottava-number");
        let n = to_int(tx, self.ottavation)?;
        tx.set("middleCOffset", Value::fixnum(-7 * n).unwrap());
        let context = tx.t.arena.borrow().handle(tx.context);
        tx.call("ly:set-middle-C!", &[context])?;
        self.ottava_ev = Some(ev);
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(ev) = self.ottava_ev {
            self.finished = self.span.take();

            if self.ottavation != Value::fixnum(0).unwrap() {
                tx.set("ottavaStartNow", Value::TRUE);
                self.create_spanner(tx, ev)?;
            }
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        match which {
            NOTE_COLUMN => {
                if let Some(span) = self.span {
                    let it = tx.grob_handle(grob);
                    side_position::add_support(tx.host, span, it)?;

                    if spanner::get_bound(tx.host, span, -1)?.is_none() {
                        spanner::set_bound(tx.host, span, -1, it)?;
                    }

                    self.note_col = Some(it);
                }
            }
            // A percent repeat disqualifies a preceding note from use as the right bound
            // of the ottava bracket.
            PERCENT_REPEAT => self.last_note_col = None,
            _ => self.acked_trill = true,
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // The head of a trilled note that is notated with a wavy line is not eligible
        // for use as the right bound of an ottava bracket.
        if self.acked_trill {
            self.note_col = None;
        }

        if let Some(span) = self.span
            && spanner::get_bound(tx.host, span, -1)?.is_none()
        {
            let e = musical_column(tx);
            set_bound(tx, span, -1, e)?;
        }

        self.typeset_all(tx)?;
        self.acked_trill = false;
        self.ottava_ev = None;
        if let Some(col) = self.note_col.take() {
            self.last_note_col = Some(col);
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.typeset_all(tx)?;
        if self.span.is_some() {
            self.finished = self.span;
        }
        self.typeset_all(tx)
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.ottava_ev);
        out.push(self.ottavation);
        out.extend(self.note_col);
        out.extend(self.last_note_col);
        out.extend(self.span);
        out.extend(self.finished);
    }
}
