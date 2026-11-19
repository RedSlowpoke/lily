//! `non-musical-script-column-engraver.cc` (MS3 step 22):
//! `Non_musical_script_column_engraver`, a class of layout's
//! ([`NON_MUSICAL_SCRIPT_COLUMN_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! The scripts acknowledged that are non-musical items (`:73–81`) are a
//! timestep's scripts; once there are two, at `process_acknowledged`
//! (`:83–91`), a `ScriptColumn` caused by the first, with `non-musical`
//! `#t`. At `stop_translation_timestep` (`:53–71`) the column takes the
//! live scripts on the Y axis (`Script_column::add_side_positioned`), and
//! both are forgotten.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::item;
use crate::script_column;
use crate::side_position;

type Run<T> = Result<T, Exception>;

pub static NON_MUSICAL_SCRIPT_COLUMN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Non_musical_script_column_engraver",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED | STOP,
    always: false,
    hears: &[],
    acknowledges: &[("script-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(NonMusicalScriptColumnEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `script_column_` by its handle, and `scripts_` by their numbers and
/// handles.
#[derive(Clone, Default)]
struct NonMusicalScriptColumnEngraver {
    script_column: Option<Value>,
    scripts: Vec<(u32, Value)>,
}

impl Work for NonMusicalScriptColumnEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.scripts.len() as i64);
        for &(id, _) in &self.scripts {
            h.record(id);
        }
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(column) = self.script_column.take() {
            for &(_, scr) in &self.scripts {
                if grob::is_live_grob(tx.heap(), scr) && side_position::is_on_y_axis(tx.host, scr)?
                {
                    script_column::add_side_positioned(tx.host, column, scr)?;
                }
            }
        }

        self.scripts.clear();
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let it = tx.grob_handle(grob);
        if item::is_item(tx.heap(), it) && item::is_non_musical(tx.host, it)? {
            self.scripts.push((grob, it));
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.script_column.is_none() && self.scripts.len() > 1 {
            let id = tx.make_defined_grob("ScriptColumn", Cause::Grob(self.scripts[0].0))?;
            let column = tx.grob_handle(id);
            grob::set_property(tx.heap(), column, "non-musical", Value::TRUE);
            self.script_column = Some(column);
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.script_column);
        out.extend(self.scripts.iter().map(|&(_, it)| it));
    }
}
