//! `score-engraver.cc`'s layout part (MS3 step 15, D108): what
//! `Score_engraver` does beyond running the timestep protocol over the
//! tree, which the music crate's Score group does. The music crate runs
//! [`SCORE_ENGRAVER`], a class of layout's that the core passes at its
//! `\layout` call, in a Score's `Score_engraver` group:
//!
//! - `initialize` (`score-engraver.cc:71–87`): a `Paper_score` with the
//!   translation's layout definition (`paper_score.rs`), set as the Score's
//!   `output`; a System made from the Score's `System` properties
//!   (`system.rs`), not announced, typeset in it; its root system set as
//!   `rootSystem`.
//! - `add_grob_to_announce_locally_only` (`:156–166`): each grob whose
//!   start is announced to the Score typeset into the root system and kept
//!   for `typeset_all`, with its class (`item.rs`). The music crate gives it once the method that made
//!   the grob has returned.
//! - `typeset_all` (`:168–179`), after each timestep (`:153`) and after the
//!   translators' `finalize` (`:127–133`): each kept grob without a Y parent
//!   made an element of the System (`axis_group.rs`).
//!
//! The finish's `finalize` over the tree (`:59–64`) is the music crate's;
//! the System's bounds are `Paper_column_engraver`'s
//! (`paper_column_engraver.rs`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, FINALIZE, INITIALIZE, Kind, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::axis_group;
use crate::geometry::Y;
use crate::grob;
use crate::item;
use crate::paper_score::{self, PaperScore};
use crate::system;

type Run<T> = Result<T, Exception>;

/// `Score_engraver`'s layout part, under the name the music crate's Score
/// group finds it by.
pub static SCORE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: lily_engine_music::SCORE_ENGRAVER,
    kind: Kind::Engraver,
    methods: INITIALIZE | STOP | FINALIZE,
    always: true,
    hears: &[],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(ScoreEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `pscore_`, `system_` and `elems_`, by their handles.
#[derive(Clone, Default)]
struct ScoreEngraver {
    pscore: Option<Value>,
    system: Option<Value>,
    elems: Vec<Value>,
}

impl Work for ScoreEngraver {
    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let layout = tx.t.arena.borrow().output_def();
        let pscore = tx.heap().wrap(PaperScore::new(layout));
        tx.set("output", pscore);
        self.pscore = Some(pscore);
        let name = tx.heap().intern("System");
        let id = tx.make_unannounced_grob(name, Cause::None);
        let made = tx.grob_handle(id);
        system::make(tx.heap(), made);
        paper_score::typeset_system(tx.heap(), pscore, made);
        let system = paper_score::root_system(tx.heap(), pscore).unwrap_or(made);
        tx.set("rootSystem", system);
        self.system = Some(system);
        Ok(())
    }

    /// `add_grob_to_announce_locally_only` of a start. The grob takes its
    /// class first, which `choose_grob_type` gives it before it is
    /// announced, so that the acknowledgers of its class's interfaces
    /// (`item-interface`) hear it.
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let elem = tx.grob_handle(grob);
        item::class(tx.heap(), elem);
        let root = self
            .pscore
            .and_then(|ps| paper_score::root_system(tx.heap(), ps));
        if let Some(root) = root {
            system::typeset_grob(tx.host, root, elem)?;
        }
        self.elems.push(elem);
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.typeset_all(tx)
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.typeset_all(tx)
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.pscore);
        out.extend(self.system);
        out.extend_from_slice(&self.elems);
    }
}

impl ScoreEngraver {
    /// `typeset_all`: each kept grob without a Y parent an element of the
    /// System.
    fn typeset_all(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(system) = self.system else {
            self.elems.clear();
            return Ok(());
        };
        for elem in core::mem::take(&mut self.elems) {
            if grob::get_parent(tx.heap(), elem, Y).is_none() {
                axis_group::add_element(tx.host, system, elem)?;
            }
        }
        Ok(())
    }
}
