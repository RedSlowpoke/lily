//! `figured-bass-position-engraver.cc` (MS4 step 16):
//! `Figured_bass_position_engraver` ([`FIGURED_BASS_POSITION_ENGRAVER`]),
//! which every `Staff` consists, beside `Figured_bass_engraver`.
//!
//! - Each `BassFigureAlignment` acknowledged (`:142–148`) gets a
//!   `BassFigureAlignmentPositioning` caused by it, from its left bound,
//!   which holds it as an axis group; at its end (`:136–140`) and at
//!   `finalize` the positioning ends at its right bound, unless it has one.
//! - The note columns and stems acknowledged, and the ties at their ends
//!   (`:73–106`), the positioning's supports at the timestep's end
//!   (`:108–118`); the slurs begun and not ended are, at each timestep's
//!   end again.
//!
//! It is woken at each timestep a positioning lives, as LilyPond calls it.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_layout::{axis_group, side_position, spanner};
use lily_engine_music::{Cause, FINALIZE, Kind, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

type Run<T> = Result<T, Exception>;

pub static FIGURED_BASS_POSITION_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Figured_bass_position_engraver",
    kind: Kind::Engraver,
    methods: STOP | FINALIZE,
    always: false,
    hears: &[],
    acknowledges: &[
        ("note-column-interface", false),
        ("slur-interface", false),
        ("stem-interface", false),
        ("slur-interface", true),
        ("tie-interface", true),
        ("bass-figure-alignment-interface", false),
        ("bass-figure-alignment-interface", true),
    ],
    must_be_last: false,
    make: Some(|| Box::new(FiguredBassPositionEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `bass_figure_alignment_`, `positioner_`, `support_` and
/// `span_support_`, by their handles.
#[derive(Clone, Default)]
struct FiguredBassPositionEngraver {
    bass_figure_alignment: Option<Value>,
    positioner: Option<Value>,
    support: Vec<Value>,
    span_support: Vec<Value>,
}

impl FiguredBassPositionEngraver {
    /// `start_spanner`.
    fn start_spanner(&mut self, tx: &mut Tx<'_>, alignment: u32) -> Run<()> {
        let Some(bfa) = self.bass_figure_alignment else {
            return Ok(());
        };
        let id = tx.make_defined_grob("BassFigureAlignmentPositioning", Cause::Grob(alignment))?;
        let positioner = tx.grob_handle(id);
        self.positioner = Some(positioner);
        if let Some(left) = spanner::get_bound(tx.host, bfa, -1)? {
            spanner::set_bound(tx.host, positioner, -1, left)?;
        }
        axis_group::add_element(tx.host, positioner, bfa)
    }

    /// `stop_spanner`.
    fn stop_spanner(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(positioner) = self.positioner
            && spanner::get_bound(tx.host, positioner, 1)?.is_none()
            && let Some(bfa) = self.bass_figure_alignment
            && let Some(right) = spanner::get_bound(tx.host, bfa, 1)?
        {
            spanner::set_bound(tx.host, positioner, 1, right)?;
        }

        self.positioner = None;
        self.bass_figure_alignment = None;
        Ok(())
    }
}

impl Work for FiguredBassPositionEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        match which {
            // acknowledge_note_column, acknowledge_stem,
            // acknowledge_end_tie
            0 | 2 | 4 => self.support.push(g),
            // acknowledge_slur
            1 => self.span_support.push(g),
            // acknowledge_end_slur
            3 => {
                if let Some(i) = self.span_support.iter().position(|&s| s == g) {
                    self.span_support.remove(i);
                }
            }
            // acknowledge_bass_figure_alignment
            5 => {
                self.bass_figure_alignment = Some(g);
                self.start_spanner(tx, grob)?;
            }
            // acknowledge_end_bass_figure_alignment
            _ => self.stop_spanner(tx)?,
        }
        if self.positioner.is_some() {
            tx.stay_awake();
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(positioner) = self.positioner {
            for &s in self.span_support.iter().chain(&self.support) {
                side_position::add_support(tx.host, positioner, s)?;
            }
            tx.stay_awake();
        }

        self.support.clear();
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.stop_spanner(tx)
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.bass_figure_alignment);
        out.extend(self.positioner);
        out.extend_from_slice(&self.support);
        out.extend_from_slice(&self.span_support);
    }
}
