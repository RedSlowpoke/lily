//! `horizontal-bracket-engraver.cc` (MS3 step 49, 158 lines):
//! `Horizontal_bracket_engraver`, a class of layout's
//! ([`HORIZONTAL_BRACKET_ENGRAVER`]) that the core passes at its `\layout`
//! call and that a group finds before the music crate's placeholder of the
//! name (D67, D108).
//!
//! - A `note-grouping-event` heard (`:51–69`): a stop counted, with
//!   LilyPond's warning where there are more stops than brackets; a start
//!   counted and kept; and LilyPond's warning at an event of a timestep
//!   that has both.
//! - Each note column acknowledged (`:71–82`) is each running bracket's
//!   support, one of its `columns` and a bound of it and of its text.
//! - `process_music` (`:84–112`): each start makes a `HorizontalBracket`
//!   caused by its event and a `HorizontalBracketText` caused by the
//!   bracket, the bracket's `bracket-text` and the text's `bracket`, the
//!   bracket the text's support and its parent on both axes; each new
//!   bracket and its text supports of the brackets already running, which
//!   are bigger.
//! - `stop_translation_timestep` (`:114–127`): a bracket and its text
//!   ended for each stop, the last begun first.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, event_property};
use lily_engine_scheme::{Exception, Value};

use crate::geometry::{X, Y};
use crate::{grob, grob_array, side_position, spanner};

type Run<T> = Result<T, Exception>;

pub static HORIZONTAL_BRACKET_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Horizontal_bracket_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["note-grouping-event"],
    acknowledges: &[("note-column-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(HorizontalBracketEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `bracket_stack_`, `text_stack_` and `events_` by their handles,
/// `pop_count_` and `push_count_`.
#[derive(Clone, Default)]
struct HorizontalBracketEngraver {
    bracket_stack: Vec<Value>,
    text_stack: Vec<Value>,
    events: Vec<Value>,
    pop_count: usize,
    push_count: usize,
}

impl Work for HorizontalBracketEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.pop_count as i64);
        h.num(self.push_count as i64);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        let d = event_property(tx.heap(), ev, "span-direction");

        if d.as_fixnum() == Some(1) {
            self.pop_count += 1;
            if self.pop_count > self.bracket_stack.len() {
                tx.event_warning(ev, "do not have that many brackets")?;
            }
        } else {
            self.push_count += 1;
            self.events.push(ev);
        }

        if self.pop_count != 0 && self.push_count != 0 {
            tx.event_warning(ev, "conflicting note group events")?;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let column = tx.grob_handle(grob);
        let columns = tx.heap().intern("columns");
        for (&bracket, &text) in self.bracket_stack.iter().zip(&self.text_stack) {
            side_position::add_support(tx.host, bracket, column)?;
            grob_array::add_grob(tx.host, bracket, columns, column)?;
            spanner::add_bound_item(tx.host, bracket, column)?;
            spanner::add_bound_item(tx.host, text, column)?;
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for k in 0..self.push_count {
            let id = tx.make_defined_grob("HorizontalBracket", Cause::Event(self.events[k]))?;
            let sp = tx.grob_handle(id);

            let text_id = tx.make_defined_grob("HorizontalBracketText", Cause::Grob(id))?;
            let hbt = tx.grob_handle(text_id);

            let heap = tx.heap();
            let sym = heap.intern("bracket-text");
            grob::set_object(heap, sp, sym, hbt);

            side_position::add_support(tx.host, hbt, sp)?;

            let heap = tx.heap();
            grob::set_parent(heap, hbt, sp, X);
            grob::set_parent(heap, hbt, sp, Y);
            let sym = heap.intern("bracket");
            grob::set_object(heap, hbt, sym, sp);

            for &bracket in &self.bracket_stack {
                /* sp is the smallest, it should be added to the bigger brackets.  */
                side_position::add_support(tx.host, bracket, sp)?;
                side_position::add_support(tx.host, bracket, hbt)?;
            }

            self.bracket_stack.push(sp);
            self.text_stack.push(hbt);
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        for _ in 0..self.pop_count {
            self.bracket_stack.pop();
            self.text_stack.pop();
        }
        self.pop_count = 0;
        self.push_count = 0;
        self.events.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(&self.bracket_stack);
        out.extend(&self.text_stack);
        out.extend(&self.events);
    }
}
