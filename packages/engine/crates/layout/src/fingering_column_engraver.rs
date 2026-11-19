//! `fingering-column-engraver.cc` (part 4's, MS3 step 53, 131 lines):
//! `Fingering_column_engraver`, a class of layout's
//! ([`FINGERING_COLUMN_ENGRAVER`]) that the core passes at its `\layout`
//! call and that a group finds before the music crate's placeholder of the
//! name (D67, D108).
//!
//! The fingered items acknowledged (`acknowledge_finger`, `:85–89`) are a
//! timestep's possibles; once there are two, at `process_acknowledged`
//! (`:91–99`), a `FingeringColumn` of no cause on each side. At
//! `stop_translation_timestep` (`:44–83`) the possibles that are not
//! non-musical and are on the X axis go to the side of their `direction`,
//! with LilyPond's warning for one of none; a side's column with fewer
//! than two is killed, and else takes them (`Fingering_column::add_fingering`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::{fingering_column, grob, item, side_position};

type Run<T> = Result<T, Exception>;

pub static FINGERING_COLUMN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Fingering_column_engraver",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED | STOP,
    always: false,
    hears: &[],
    acknowledges: &[("finger-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(FingeringColumnEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `fingering_columns_` and `scripts_`, left and right, and
/// `possibles_`, by their handles.
#[derive(Clone, Default)]
struct FingeringColumnEngraver {
    fingering_columns: [Option<Value>; 2],
    scripts: [Vec<Value>; 2],
    possibles: Vec<Value>,
}

impl Work for FingeringColumnEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        for d in 0..2 {
            h.flags(&[self.fingering_columns[d].is_some()]);
            h.num(self.scripts[d].len() as i64);
        }
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for &item in &self.possibles {
            if !item::is_non_musical(tx.host, item)? && side_position::is_on_x_axis(tx.host, item)?
            {
                let d = grob::property(tx.host, item, "direction")?;
                match crate::dynamic_align_engraver::direction(d) {
                    0 => {
                        grob::warning(tx.host, item, "Cannot add a fingering without a direction.")?
                    }
                    d => self.scripts[usize::from(d > 0)].push(item),
                }
            }
        }

        for d in 0..2 {
            if self.scripts[d].len() < 2
                && let Some(column) = self.fingering_columns[d].take()
            {
                grob::suicide(tx.heap(), column);
            }
            if let Some(column) = self.fingering_columns[d] {
                for &f in &self.scripts[d] {
                    fingering_column::add_fingering(tx.host, column, f)?;
                }
            }
            self.scripts[d].clear();
            self.fingering_columns[d] = None;
        }
        self.possibles.clear();
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        // `Grob_info_t<Item>`: an item's alone.
        let g = tx.grob_handle(grob);
        if item::is_item(tx.heap(), g) {
            self.possibles.push(g);
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for d in 0..2 {
            if self.possibles.len() > 1 && self.fingering_columns[d].is_none() {
                let id = tx.make_defined_grob("FingeringColumn", Cause::None)?;
                self.fingering_columns[d] = Some(tx.grob_handle(id));
            }
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.fingering_columns.iter().flatten());
        out.extend(self.scripts.iter().flatten());
        out.extend(&self.possibles);
    }
}
