//! `bar-number-engraver.cc` (MS3 step 18): `Bar_number_engraver`, a class
//! of layout's ([`BAR_NUMBER_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - `process_music` (`:154–163`): where a break is allowed (`break_allowed`)
//!   and `centerBarNumbers` is false, a bar number considered.
//! - A bar line acknowledged (`:170–174`), or the start of the piece, where
//!   `saw_bar_line_` starts true: `process_acknowledged` (`:176–181`)
//!   considers a bar number there if `process_music` did not.
//! - `consider_creating_bar_number` (`:114–152`): the last centered bar
//!   number ended at `currentCommandColumn`; then, if `barNumberVisibility`
//!   is a procedure and `currentBarNumber` a number, the procedure called
//!   with the number and `measurePosition`, and where it gives `#t`,
//!   `barNumberFormatter` called with them, the alternative's number less
//!   one (`get_alt_number`, `:216–235`: `alternativeNumber` where
//!   `alternativeNumberingStyle` is `numbers-with-letters`, else 0) and the
//!   context, its text the new grob's (`create_bar_number`, `:96–112`): a
//!   `CenteredBarNumber` with `currentCommandColumn` its left bound under
//!   `centerBarNumbers`, else a `BarNumber`. Both procedures run from the
//!   image (`translation-functions.scm`).
//! - `stop_translation_timestep` (`:183–214`): the `BarNumber`'s
//!   `side-support-elements` the grob array of `stavesFound`, and, made
//!   where a break is allowed but no bar line was, its `break-visibility`
//!   copied with the middle `#f` (`#(#t #t #t)` if it is no vector; one
//!   shorter than 2, where Guile's `vector-set!` would throw, is copied
//!   unchanged).
//!
//! Without a command column, where LilyPond's would crash, no bound is set
//! (`ledger_line_engraver::set_bound`). `stavesFound` is
//! `Staff_collecting_engraver`'s, which no step of part 2 ports: until it
//! is, the array is empty where LilyPond's holds the staff symbols.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, Moment, PROCESS, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work,
};
use lily_engine_scheme::{Exception, Value};

use crate::bar_engraver::long_or;
use crate::clef_engraver::{break_allowed, is_true};
use crate::grob;
use crate::grob_array::grob_list_to_grob_array;
use crate::ledger_line_engraver::{command_column, set_bound};

type Run<T> = Result<T, Exception>;

pub static BAR_NUMBER_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Bar_number_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | PROCESS_ACKNOWLEDGED | STOP,
    always: true,
    hears: &[],
    acknowledges: &[("bar-line-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(BarNumberEngraver::default())),
    reads: &[
        "alternativeNumber",
        "alternativeNumberingStyle",
        "barNumberFormatter",
        "barNumberVisibility",
        "centerBarNumbers",
        "currentBarNumber",
        "currentCommandColumn",
        "forbidBreak",
        "forceBreak",
        "measurePosition",
        "stavesFound",
    ],
    writes: &[],
};

/// `text_` and `span_` by their handles (the spanner with its record's
/// number), `considered_numbering_`, `saw_bar_line_` and
/// `break_allowed_now_`.
#[derive(Clone)]
struct BarNumberEngraver {
    text: Option<Value>,
    span: Option<(Value, u32)>,
    considered_numbering: bool,
    saw_bar_line: bool,
    break_allowed_now: bool,
}

impl Default for BarNumberEngraver {
    fn default() -> Self {
        BarNumberEngraver {
            text: None,
            span: None,
            considered_numbering: false,
            // There is no bar line at the beginning of the piece, and a
            // break isn't allowed either, but we want to allow a bar number
            // nevertheless.
            saw_bar_line: true,
            break_allowed_now: false,
        }
    }
}

impl BarNumberEngraver {
    fn create_bar_number(&mut self, tx: &mut Tx<'_>, text: Value) -> Run<()> {
        if is_true(tx.get("centerBarNumbers")) {
            let column = command_column(tx);
            let id = tx.make_defined_grob("CenteredBarNumber", Cause::None)?;
            let span = tx.grob_handle(id);
            self.span = Some((span, id));
            set_bound(tx, span, -1, column)?;
            grob::set_property(tx.heap(), span, "text", text);
        } else {
            let id = tx.make_defined_grob("BarNumber", Cause::None)?;
            let item = tx.grob_handle(id);
            self.text = Some(item);
            grob::set_property(tx.heap(), item, "text", text);
        }
        Ok(())
    }

    fn consider_creating_bar_number(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.considered_numbering = true;

        // Time to terminate the previous spanner if applicable.
        if let Some((span, id)) = self.span.take() {
            let column = command_column(tx);
            set_bound(tx, span, 1, column)?;
            tx.announce_end_grob(id);
        }

        let vis = tx.get("barNumberVisibility");
        if !tx.heap().is_procedure(vis) {
            return Ok(());
        }
        let bn = tx.get("currentBarNumber");
        if !tx.heap().is_number(bn) {
            return Ok(());
        }
        let v = tx.get("measurePosition");
        let mp = tx.heap().foreign::<Moment>(v).copied().unwrap_or_default();

        let depth = tx.heap().root_depth();
        let ran = self.call_procedures(tx, vis, bn, mp);
        tx.heap().unroot_to(depth);
        ran
    }

    /// `barNumberVisibility`, and where it gives `#t` `barNumberFormatter`,
    /// called; what is made is rooted until the caller unroots.
    fn call_procedures(&mut self, tx: &mut Tx<'_>, vis: Value, bn: Value, mp: Moment) -> Run<()> {
        let heap = tx.heap();
        heap.root(vis);
        heap.root(bn);
        let mp_scm = heap.wrap(mp);
        heap.root(mp_scm);

        if !is_true(tx.host.call(vis, &[bn, mp_scm])?) {
            return Ok(());
        }
        let formatter = tx.get("barNumberFormatter");
        let mut formatted_text = Value::NULL;
        if tx.heap().is_procedure(formatter) {
            let alt = get_alt_number(tx) - 1;
            let alt = tx.heap().integer(alt);
            let context = tx.t.arena.borrow().handle(tx.context);
            formatted_text = tx.host.call(formatter, &[bn, mp_scm, alt, context])?;
            tx.heap().root(formatted_text);
        }
        self.create_bar_number(tx, formatted_text)
    }
}

/// `get_alt_number`: `alternativeNumber` (0 if it is no integer), but 0
/// unless `alternativeNumberingStyle` is `numbers-with-letters`.
fn get_alt_number(tx: &mut Tx<'_>) -> i64 {
    let v = tx.get("alternativeNumber");
    let alt_num = long_or(tx, v, 0);
    let style = tx.get("alternativeNumberingStyle");
    if alt_num > 0 && style != tx.heap().intern("numbers-with-letters") {
        return 0;
    }
    alt_num
}

impl Work for BarNumberEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[
            self.considered_numbering,
            self.saw_bar_line,
            self.break_allowed_now,
        ]);
        h.maybe_record(self.span.map(|s| s.1));
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.break_allowed_now = break_allowed(tx);
        if self.break_allowed_now && !is_true(tx.get("centerBarNumbers")) {
            self.consider_creating_bar_number(tx)?;
        }
        Ok(())
    }

    fn acknowledge(&mut self, _: &mut Tx<'_>, _: usize, _: u32) -> Run<()> {
        self.saw_bar_line = true;
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if !self.considered_numbering && self.saw_bar_line {
            self.consider_creating_bar_number(tx)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(text) = self.text.take() {
            let staves = tx.get("stavesFound");
            let (array, warnings) = grob_list_to_grob_array(tx.heap(), staves);
            let sym = tx.heap().intern("side-support-elements");
            grob::set_object(tx.heap(), text, sym, array);
            for w in warnings {
                tx.warning(&w)?;
            }

            if self.break_allowed_now && !self.saw_bar_line && !is_true(tx.get("centerBarNumbers"))
            {
                let bk_vis = grob::property(tx.host, text, "break-visibility")?;
                // The general default for break-visibility is all-visible.
                let mut items = match tx.heap().vector_items(bk_vis) {
                    Some(items) => items.to_vec(),
                    None => alloc::vec![Value::TRUE; 3],
                };
                if let Some(middle) = items.get_mut(1) {
                    *middle = Value::FALSE;
                }
                let bk_vis = tx.heap().vector(items);
                grob::set_property(tx.heap(), text, "break-visibility", bk_vis);
            }
        }

        self.considered_numbering = false;
        self.saw_bar_line = false;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.text);
        out.extend(self.span.map(|(v, _)| v));
    }
}
