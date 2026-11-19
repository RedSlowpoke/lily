//! `dot-column-engraver.cc` (MS3 step 16): `Dot_column_engraver`, a class
//! of layout's ([`DOT_COLUMN_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! Each rhythmic head acknowledged (`acknowledge_rhythmic_head`, `:51–62`)
//! that has dots: the timestep's `DotColumn`, with no cause, made at the
//! first, and the head added to it (`Dot_column::add_head`).
//! `stop_translation_timestep` (`:45–49`) forgets the column.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, STOP, TranslatorClass, Tx, Work, grob_of};
use lily_engine_scheme::{Exception, Value};

use crate::dot_column;
use crate::grob_array;

type Run<T> = Result<T, Exception>;

pub static DOT_COLUMN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Dot_column_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    always: false,
    hears: &[],
    acknowledges: &[("rhythmic-head-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(DotColumnEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `dotcol_`, by its handle.
#[derive(Clone, Default)]
struct DotColumnEngraver {
    dotcol: Option<Value>,
}

impl Work for DotColumnEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let head = tx.grob_handle(grob);
        let sym = tx.heap().intern("dot");
        let d = grob_array::object(tx.host, head, sym)?;
        if grob_of(tx.heap(), d).is_none() {
            return Ok(());
        }
        let dotcol = match self.dotcol {
            Some(c) => c,
            None => {
                let id = tx.make_defined_grob("DotColumn", Cause::None)?;
                let c = tx.grob_handle(id);
                self.dotcol = Some(c);
                c
            }
        };
        dot_column::add_head(tx.host, dotcol, head)
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.dotcol = None;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.dotcol);
    }
}
