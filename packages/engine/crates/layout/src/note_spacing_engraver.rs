//! `note-spacing-engraver.cc` (MS3 step 22): `Note_spacing_engraver`, a
//! class of layout's ([`NOTE_SPACING_ENGRAVER`]) that the core passes at
//! its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - Each note column and rhythmic grob acknowledged (`add_spacing_item`,
//!   `:62–79`): the timestep's `NoteSpacing`, made at the first and caused
//!   by it, gets it in its `left-items`, and the last note spacing made
//!   gets it in its `right-items`.
//! - `stop_translation_timestep` (`:109–128`): if `hasStaffSpacing` is
//!   true, `currentCommandColumn` added to the `right-items` of the last
//!   note spacing made while the context had its present parent; the
//!   timestep's spacing, if any, becomes the last, and the parent's last.
//!   As it reads `hasStaffSpacing` at every timestep, the class runs
//!   always.
//! - `finalize` (`:93–107`): the parent's last note spacing, if it has no
//!   `right-items` array, gets `currentCommandColumn` there.
//!
//! `last_spacings_` (keyed by the parent's `Context *`) is kept by the
//! parent's context number, which the arena never gives to another
//! context. Without a command column, where LilyPond's would add a null
//! grob, none is added. An acknowledged non-item, where LilyPond's
//! trampoline gives its type error, is taken as an item.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, CtxId, FINALIZE, Kind, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::grob_array::{self, GrobArray};
use crate::ledger_line_engraver::command_column;

type Run<T> = Result<T, Exception>;

pub static NOTE_SPACING_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Note_spacing_engraver",
    kind: Kind::Engraver,
    methods: STOP | FINALIZE,
    // `stop_translation_timestep` reads `hasStaffSpacing` every timestep.
    always: true,
    hears: &[],
    acknowledges: &[
        ("note-column-interface", false),
        ("rhythmic-grob-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(NoteSpacingEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `last_spacings_`, `last_spacing_` and `spacing_`, by their handles.
#[derive(Clone, Default)]
struct NoteSpacingEngraver {
    last_spacings: Vec<(Option<CtxId>, Value)>,
    last_spacing: Option<Value>,
    spacing: Option<Value>,
}

impl NoteSpacingEngraver {
    /// `last_spacings_[parent]`.
    fn last_of(&self, parent: Option<CtxId>) -> Option<Value> {
        self.last_spacings
            .iter()
            .find(|&&(c, _)| c == parent)
            .map(|&(_, g)| g)
    }

    fn add_spacing_item(&mut self, tx: &mut Tx<'_>, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        let spacing = match self.spacing {
            Some(sp) => sp,
            None => {
                let id = tx.make_defined_grob("NoteSpacing", Cause::Grob(grob))?;
                let sp = tx.grob_handle(id);
                self.spacing = Some(sp);
                sp
            }
        };

        let left_items = tx.heap().intern("left-items");
        grob_array::add_grob(tx.host, spacing, left_items, g)?;

        if let Some(last) = self.last_spacing {
            let right_items = tx.heap().intern("right-items");
            grob_array::add_grob(tx.host, last, right_items, g)?;
        }
        Ok(())
    }
}

/// The context's parent.
fn parent(tx: &Tx<'_>) -> Option<CtxId> {
    tx.t.arena.borrow().parent(tx.context)
}

impl Work for NoteSpacingEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.last_spacings.len() as i64);
        for &(c, _) in &self.last_spacings {
            h.maybe_context(c);
        }
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        self.add_spacing_item(tx, grob)
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(last) = self.last_of(parent(tx)) else {
            return Ok(());
        };
        let right_items = tx.heap().intern("right-items");
        let items = grob_array::object(tx.host, last, right_items)?;
        if tx.heap().foreign::<GrobArray>(items).is_none()
            && let Some(col) = command_column(tx)
        {
            grob_array::add_grob(tx.host, last, right_items, col)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let parent = parent(tx);
        if let Some(last) = self.last_of(parent)
            && tx.get("hasStaffSpacing") == Value::TRUE
            && let Some(col) = command_column(tx)
        {
            let right_items = tx.heap().intern("right-items");
            grob_array::add_grob(tx.host, last, right_items, col)?;
        }

        if let Some(sp) = self.spacing.take() {
            match self.last_spacings.iter_mut().find(|(c, _)| *c == parent) {
                Some(entry) => entry.1 = sp,
                None => self.last_spacings.push((parent, sp)),
            }
            self.last_spacing = Some(sp);
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.last_spacings.iter().map(|&(_, g)| g));
        out.extend(self.last_spacing);
        out.extend(self.spacing);
    }
}
