//! `script-row-engraver.cc` (MS3 step 19): `Script_row_engraver`, a class
//! of layout's ([`SCRIPT_ROW_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! The accidental placements acknowledged (`:86–90`), and the side
//! positioned items that are not non-musical (`:76–84`), are a timestep's
//! scripts; once there are two, at `process_acknowledged` (`:92–97`), a
//! `ScriptRow` caused by the first. At `stop_translation_timestep`
//! (`:56–74`) the row takes the placements and the scripts on the X axis
//! (`Script_column::add_side_positioned`), and both are forgotten.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::item;
use crate::script_column;
use crate::side_position;

type Run<T> = Result<T, Exception>;

pub static SCRIPT_ROW_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Script_row_engraver",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED | STOP,
    always: false,
    hears: &[],
    acknowledges: &[
        ("accidental-placement-interface", false),
        ("side-position-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(ScriptRowEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `script_row_` by its handle, and `scripts_` by their numbers and
/// handles.
#[derive(Clone, Default)]
struct ScriptRowEngraver {
    script_row: Option<Value>,
    scripts: Vec<(u32, Value)>,
}

impl Work for ScriptRowEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.scripts.len() as i64);
        for &(id, _) in &self.scripts {
            h.record(id);
        }
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(row) = self.script_row {
            for &(_, scr) in &self.scripts {
                if grob::has_interface(tx.heap(), scr, "accidental-placement-interface")
                    || side_position::is_on_x_axis(tx.host, scr)?
                {
                    script_column::add_side_positioned(tx.host, row, scr)?;
                }
            }

            self.script_row = None;
        }

        self.scripts.clear();
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let it = tx.grob_handle(grob);
        // `acknowledge_accidental_placement`, then `acknowledge_side_position`.
        let script =
            which == 0 || (item::is_item(tx.heap(), it) && !item::is_non_musical(tx.host, it)?);
        if script {
            self.scripts.push((grob, it));
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.script_row.is_none() && self.scripts.len() > 1 {
            let id = tx.make_defined_grob("ScriptRow", Cause::Grob(self.scripts[0].0))?;
            self.script_row = Some(tx.grob_handle(id));
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.script_row);
        out.extend(self.scripts.iter().map(|&(_, it)| it));
    }
}
