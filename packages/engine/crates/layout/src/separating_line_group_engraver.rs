//! `separating-line-group-engraver.cc` (MS3 step 22):
//! `Separating_line_group_engraver`, a class of layout's
//! ([`SEPARATING_LINE_GROUP_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - Each item acknowledged (`:73–112`): one with `note-spacing-interface`
//!   is kept among the timestep's note spacings. Else the timestep's first
//!   non-musical one (`Item::is_non_musical`), if `createSpacing` is true,
//!   makes its `StaffSpacing`, with no cause, sets `hasStaffSpacing` in the
//!   engraver's context and adds `currentCommandColumn` to its
//!   `left-items`; if the last timestep that had spacings had a staff
//!   spacing and no note spacings, that one's `right-items` become the
//!   command column alone.
//! - Each break-aligned item acknowledged (`:114–119`) is kept.
//! - `start_translation_timestep` (`:121–125`): `hasStaffSpacing` unset,
//!   at every timestep, so the class runs always.
//! - `stop_translation_timestep` (`:127–154`): the kept break-aligned items
//!   added to the staff spacing's `left-break-aligned` and to the
//!   `right-break-aligned` of the last spacings' note spacings; the
//!   timestep's spacings, if any, become the last; `currentMusicalColumn`
//!   added to the staff spacing's `right-items`; the timestep's spacings
//!   and items forgotten.
//!
//! Without a command column, where LilyPond's would add a null grob, none
//! is added. An acknowledged non-item, where LilyPond's trampoline gives
//! its type error, is taken as an item.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, START, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::dynamic_engraver::musical_column;
use crate::grob;
use crate::grob_array::{self};
use crate::item;
use crate::ledger_line_engraver::command_column;

type Run<T> = Result<T, Exception>;

pub static SEPARATING_LINE_GROUP_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Separating_line_group_engraver",
    kind: Kind::Engraver,
    methods: START | STOP,
    // `start_translation_timestep` unsets `hasStaffSpacing` every timestep,
    // whoever set it.
    always: true,
    hears: &[],
    acknowledges: &[
        ("item-interface", false),
        ("break-aligned-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(SeparatingLineGroupEngraver::default())),
    reads: &["createSpacing"],
    writes: &["hasStaffSpacing"],
};

/// `Spacings`: a staff spacing and note spacings, by their handles.
#[derive(Clone, Default)]
struct Spacings {
    staff_spacing: Option<Value>,
    note_spacings: Vec<Value>,
}

impl Spacings {
    fn is_empty(&self) -> bool {
        self.staff_spacing.is_none() && self.note_spacings.is_empty()
    }

    fn clear(&mut self) {
        self.staff_spacing = None;
        self.note_spacings.clear();
    }
}

/// `current_spacings_`, `last_spacings_` and `break_aligned_`.
#[derive(Clone, Default)]
struct SeparatingLineGroupEngraver {
    current_spacings: Spacings,
    last_spacings: Spacings,
    break_aligned: Vec<Value>,
}

impl Work for SeparatingLineGroupEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        for s in [&self.current_spacings, &self.last_spacings] {
            h.flags(&[s.staff_spacing.is_some()]);
            h.num(s.note_spacings.len() as i64);
        }
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        tx.unset("hasStaffSpacing");
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let left_break_aligned = tx.heap().intern("left-break-aligned");
        let right_break_aligned = tx.heap().intern("right-break-aligned");
        for &smob in &self.break_aligned {
            if let Some(sp) = self.current_spacings.staff_spacing {
                grob_array::add_grob(tx.host, sp, left_break_aligned, smob)?;
            }

            for &ns in &self.last_spacings.note_spacings {
                grob_array::add_grob(tx.host, ns, right_break_aligned, smob)?;
            }
        }

        if !self.current_spacings.is_empty() {
            self.last_spacings = self.current_spacings.clone();
        }

        if let Some(sp) = self.current_spacings.staff_spacing
            && let Some(col) = musical_column(tx)
        {
            let right_items = tx.heap().intern("right-items");
            grob_array::add_grob(tx.host, sp, right_items, col)?;
        }

        self.current_spacings.clear();
        self.break_aligned.clear();
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let it = tx.grob_handle(grob);
        if which == 1 {
            self.break_aligned.push(it);
            return Ok(());
        }

        if grob::has_interface(tx.heap(), it, "note-spacing-interface") {
            self.current_spacings.note_spacings.push(it);
            return Ok(());
        }

        if item::is_non_musical(tx.host, it)?
            && self.current_spacings.staff_spacing.is_none()
            && tx.get("createSpacing") == Value::TRUE
        {
            let col = command_column(tx);

            let id = tx.make_defined_grob("StaffSpacing", Cause::None)?;
            let sp = tx.grob_handle(id);
            self.current_spacings.staff_spacing = Some(sp);
            tx.set("hasStaffSpacing", Value::TRUE);

            let left_items = tx.heap().intern("left-items");
            if let Some(col) = col {
                grob_array::add_grob(tx.host, sp, left_items, col)?;
            }

            if self.last_spacings.note_spacings.is_empty()
                && let Some(last) = self.last_spacings.staff_spacing
            {
                let right_items = tx.heap().intern("right-items");
                grob_array::reset(tx.host, last, right_items, col)?;
            }
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        for s in [&self.current_spacings, &self.last_spacings] {
            out.extend(s.staff_spacing);
            out.extend(&s.note_spacings);
        }
        out.extend(&self.break_aligned);
    }
}
