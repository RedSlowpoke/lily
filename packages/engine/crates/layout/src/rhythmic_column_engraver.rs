//! `rhythmic-column-engraver.cc` (MS3 step 16): `Rhythmic_column_engraver`,
//! a class of layout's ([`RHYTHMIC_COLUMN_ENGRAVER`]) that the core passes
//! at its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! The stems, flags and rhythmic heads acknowledged (`:108–124`) are glued
//! together in `process_acknowledged` (`:77–106`): the timestep's
//! `NoteColumn`, caused by the first head, made then; each head without a
//! parent on X added to it (`Note_column::add_head`), a stem without one
//! set as its stem (`Note_column::set_stem`), a flag made its element.
//! `stop_translation_timestep` (`:126–132`) forgets the column, the stem
//! and the flag.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::axis_group;
use crate::geometry::X;
use crate::grob;
use crate::note_column;

type Run<T> = Result<T, Exception>;

pub static RHYTHMIC_COLUMN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Rhythmic_column_engraver",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED | STOP,
    always: false,
    hears: &[],
    acknowledges: &[
        ("stem-interface", false),
        ("flag-interface", false),
        ("rhythmic-head-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(RhythmicColumnEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `rheads_` by their records, `stem_`, `flag_` and `note_column_` by
/// their handles.
#[derive(Clone, Default)]
struct RhythmicColumnEngraver {
    rheads: Vec<u32>,
    stem: Option<Value>,
    flag: Option<Value>,
    note_column: Option<Value>,
}

impl Work for RhythmicColumnEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.records(&self.rheads);
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        match which {
            0 => self.stem = Some(tx.grob_handle(grob)),
            1 => self.flag = Some(tx.grob_handle(grob)),
            _ => self.rheads.push(grob),
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(&first) = self.rheads.first() {
            let column = match self.note_column {
                Some(c) => c,
                None => {
                    let id = tx.make_defined_grob("NoteColumn", Cause::Grob(first))?;
                    let c = tx.grob_handle(id);
                    self.note_column = Some(c);
                    c
                }
            };
            for head in core::mem::take(&mut self.rheads) {
                let head = tx.grob_handle(head);
                if grob::get_parent(tx.heap(), head, X).is_none() {
                    note_column::add_head(tx.host, column, head)?;
                }
            }
        }
        let Some(column) = self.note_column else {
            return Ok(());
        };
        if let Some(stem) = self.stem
            && grob::get_parent(tx.heap(), stem, X).is_none()
        {
            note_column::set_stem(tx.host, column, stem)?;
            self.stem = None;
        }
        if let Some(flag) = self.flag.take() {
            axis_group::add_element(tx.host, column, flag)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.note_column = None;
        self.stem = None;
        self.flag = None;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.stem);
        out.extend(self.flag);
        out.extend(self.note_column);
    }
}
