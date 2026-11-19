//! `script-column-engraver.cc` (part 4's, MS3 step 53, 111 lines):
//! `Script_column_engraver`, a class of layout's
//! ([`SCRIPT_COLUMN_ENGRAVER`]) that the core passes at its `\layout` call
//! and that a group finds before the music crate's placeholder of the name
//! (D67, D108).
//!
//! The side positioned items acknowledged that are not non-musical
//! (`:61–68`) are a timestep's scripts; once there are two, at
//! `process_acknowledged` (`:70–75`), a `ScriptColumn` caused by the
//! first. At `stop_translation_timestep` (`:44–59`) the column takes the
//! scripts on the Y axis (`Script_column::add_side_positioned`), and both
//! are forgotten.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::item;
use crate::script_column;
use crate::side_position;

type Run<T> = Result<T, Exception>;

pub static SCRIPT_COLUMN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Script_column_engraver",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED | STOP,
    always: false,
    hears: &[],
    acknowledges: &[("side-position-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(ScriptColumnEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `script_column_` by its handle, and `scripts_` by their numbers and
/// handles.
#[derive(Clone, Default)]
struct ScriptColumnEngraver {
    script_column: Option<Value>,
    scripts: Vec<(u32, Value)>,
}

impl Work for ScriptColumnEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.scripts.len() as i64);
        for &(id, _) in &self.scripts {
            h.record(id);
        }
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(column) = self.script_column.take() {
            for &(_, scr) in &self.scripts {
                if side_position::is_on_y_axis(tx.host, scr)? {
                    script_column::add_side_positioned(tx.host, column, scr)?;
                }
            }
        }

        self.scripts.clear();
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let it = tx.grob_handle(grob);
        if item::is_item(tx.heap(), it) && !item::is_non_musical(tx.host, it)? {
            self.scripts.push((grob, it));
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.script_column.is_none() && self.scripts.len() > 1 {
            let id = tx.make_defined_grob("ScriptColumn", Cause::Grob(self.scripts[0].0))?;
            self.script_column = Some(tx.grob_handle(id));
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.script_column);
        out.extend(self.scripts.iter().map(|&(_, it)| it));
    }
}
