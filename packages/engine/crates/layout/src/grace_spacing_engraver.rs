//! `grace-spacing-engraver.cc` (MS3 step 22): `Grace_spacing_engraver`, a
//! class of layout's ([`GRACE_SPACING_ENGRAVER`]) that the core passes at
//! its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - `process_music` (`:45–68`): a grace timestep after one that was not
//!   starts a `GraceSpacing`, with no cause; while one runs, a grace
//!   timestep and the one after the last grace timestep add
//!   `currentMusicalColumn` to its `columns`, make it the column's
//!   `grace-spacing` and its left bound if it has none, else its right.
//! - `stop_translation_timestep` (`:70–77`): the timestep kept as the
//!   last; at one that is not grace, the spacing forgotten. As the class
//!   reads every timestep's moment, it runs always.
//!
//! Without a musical column, where LilyPond's would crash, nothing is
//! added or set.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, Moment, PROCESS, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::dynamic_engraver::musical_column;
use crate::grob;
use crate::grob_array;
use crate::spanner;

type Run<T> = Result<T, Exception>;

pub static GRACE_SPACING_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Grace_spacing_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    // `stop_translation_timestep` keeps every timestep's moment.
    always: true,
    hears: &[],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(GraceSpacingEngraver::default())),
    reads: &["currentMusicalColumn"],
    writes: &[],
};

/// `last_moment_`, and `grace_spacing_` by its handle.
#[derive(Clone, Default)]
struct GraceSpacingEngraver {
    last_moment: Moment,
    grace_spacing: Option<Value>,
}

impl Work for GraceSpacingEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.moment(self.last_moment);
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let now = tx.now();
        if self.last_moment.grace.is_zero() && !now.grace.is_zero() {
            let id = tx.make_defined_grob("GraceSpacing", Cause::None)?;
            self.grace_spacing = Some(tx.grob_handle(id));
        }

        if let Some(sp) = self.grace_spacing
            && (!now.grace.is_zero() || !self.last_moment.grace.is_zero())
            && let Some(column) = musical_column(tx)
        {
            let columns = tx.heap().intern("columns");
            grob_array::add_grob(tx.host, sp, columns, column)?;

            let sym = tx.heap().intern("grace-spacing");
            grob::set_object(tx.heap(), column, sym, sp);

            spanner::add_bound_item(tx.host, sp, column)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.last_moment = tx.now();

        if self.last_moment.grace.is_zero() {
            self.grace_spacing = None;
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.grace_spacing);
    }
}
