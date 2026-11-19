//! `collision-engraver.cc` (MS3 step 41): `Collision_engraver`, a class
//! of layout's ([`COLLISION_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! The note columns acknowledged (`acknowledge_note_column`, `:54–67`)
//! that have no rest, no X parent and no `ignore-collision` `#t` are a
//! timestep's; once there are two, at `process_acknowledged` (`:42–52`),
//! a `NoteCollision`, with no cause, takes them
//! (`Note_collision_interface::add_column`); columns acknowledged after
//! it are not added. `stop_translation_timestep` (`:69–74`) forgets both.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::geometry::X;
use crate::grob;
use crate::note_collision;
use crate::note_column;

type Run<T> = Result<T, Exception>;

pub static COLLISION_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Collision_engraver",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED | STOP,
    always: false,
    hears: &[],
    acknowledges: &[("note-column-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(CollisionEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `col_` and `note_columns_`, by their handles.
#[derive(Clone, Default)]
struct CollisionEngraver {
    col: Option<Value>,
    note_columns: Vec<Value>,
}

impl Work for CollisionEngraver {
    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.col.is_some() || self.note_columns.len() < 2 {
            return Ok(());
        }
        let id = tx.make_defined_grob("NoteCollision", Cause::None)?;
        let col = tx.grob_handle(id);
        self.col = Some(col);
        for &c in &self.note_columns {
            note_collision::add_column(tx.host, col, c)?;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let item = tx.grob_handle(grob);
        if note_column::has_rests(tx.host, item)? || grob::get_parent(tx.heap(), item, X).is_some()
        {
            return Ok(());
        }
        if grob::property(tx.host, item, "ignore-collision")? == Value::TRUE {
            return Ok(());
        }
        self.note_columns.push(item);
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.col = None;
        self.note_columns.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.col);
        out.extend(self.note_columns.iter().copied());
    }
}
