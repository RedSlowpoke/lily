//! `span-bar-stub-engraver.cc` (MS3 step 42): `Span_bar_stub_engraver`, a
//! class of layout's ([`SPAN_BAR_STUB_ENGRAVER`]) that the core passes at
//! its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - The span bars acknowledged (`acknowledge_span_bar`, `:75–79`) are
//!   the timestep's; each axis group acknowledged
//!   (`acknowledge_hara_kiri_group_spanner`, `:81–87`) is kept with the
//!   context of the engraver that made it, the newest first.
//! - `process_acknowledged` (`:89–163`): for each span bar, past the
//!   score's start (an alignment above the first group kept), the indices
//!   of its `elements`' axis groups; each kept group of a context that is
//!   not removable and has a parent, whose index lies strictly between
//!   the first and the last of them and is none of them, gets a
//!   `SpanBarStub` made in its context (its definition seen from there,
//!   announced to the groups up from there, its origin this engraver), the
//!   span bar its cause and X parent, killed at once unless the
//!   `allow-span-bar` of the element at the place of the next index above
//!   the group's, less one, is `#t`. Then the span bars are forgotten.
//! - `stop_translation_timestep` (`:165–178`): the groups of contexts
//!   that are removable are forgotten. It runs at every timestep, as a
//!   context may be removable at one no grob wakes the engraver.
//!
//! Where LilyPond's would index past the end of no axis group indices (a
//! span bar none of whose elements is in an axis group), no stub is made.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, CtxId, Kind, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work,
};
use lily_engine_scheme::{Exception, Value};

use crate::clef_engraver::is_true;
use crate::geometry::X;
use crate::grob;
use crate::grob_array;

type Run<T> = Result<T, Exception>;

pub static SPAN_BAR_STUB_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Span_bar_stub_engraver",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED | STOP,
    always: true,
    hears: &[],
    acknowledges: &[
        ("span-bar-interface", false),
        ("hara-kiri-group-spanner-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(SpanBarStubEngraver::default())),
    reads: &[],
    writes: &[],
};

/// The acknowledgers' indices in [`SPAN_BAR_STUB_ENGRAVER`].
const SPAN_BAR: usize = 0;

/// `spanbars_`, by their handles with their record numbers, and
/// `axis_groups_`, each group's handle with its context.
#[derive(Clone, Default)]
struct SpanBarStubEngraver {
    spanbars: Vec<(Value, u32)>,
    axis_groups: Vec<(Value, CtxId)>,
}

/// `!c || c->is_removable ()`.
fn removable(tx: &Tx<'_>, c: CtxId) -> bool {
    tx.t.arena.borrow().is_removable(c)
}

impl Work for SpanBarStubEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.spanbars.len() as i64);
        for &(_, id) in &self.spanbars {
            h.record(id);
        }
        h.num(self.axis_groups.len() as i64);
        for &(_, c) in &self.axis_groups {
            h.context(c);
        }
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        if which == SPAN_BAR {
            self.spanbars.push((g, grob));
        } else {
            let c = tx.t.arena.borrow().records.grobs[grob as usize].origin.0;
            self.axis_groups.insert(0, (g, c));
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.spanbars.is_empty() {
            return Ok(());
        }

        let Some(&(first, _)) = self.axis_groups.first() else {
            return tx.programming_error(
                "At least one vertical axis group needs to be created in the first time step.",
            );
        };
        if grob::root_vertical_alignment(tx.heap(), first).is_none() {
            // we are at the beginning of a score, so no need for stubs
            return Ok(());
        }

        for &(spanbar, spanbar_id) in &self.spanbars {
            let bars = grob_array::extract_grob_array(tx.host, spanbar, "elements")?;
            let mut bar_axis_indices = Vec::new();
            for &bar in &bars {
                let i = grob::get_vertical_axis_group_index(tx.host, bar)?;
                if i >= 0 {
                    bar_axis_indices.push(i as u64);
                }
            }
            let (Some(&lowest), Some(&highest)) =
                (bar_axis_indices.first(), bar_axis_indices.last())
            else {
                continue;
            };
            let mut affected_contexts = Vec::new();
            let mut keep_extent = Vec::new();
            for &(g, c) in &self.axis_groups {
                if removable(tx, c) {
                    continue;
                }

                // As LilyPond's `vsize`: an index of -1 is past every other.
                let j = grob::get_vertical_axis_group_index(tx.host, g)? as u64;
                if j > lowest && j < highest && !bar_axis_indices.contains(&j) {
                    let k = bar_axis_indices.iter().take_while(|&&i| i <= j).count() - 1;

                    if tx.t.arena.borrow().parent(c).is_some() {
                        keep_extent.push(is_true(grob::property(
                            tx.host,
                            bars[k],
                            "allow-span-bar",
                        )?));
                        affected_contexts.push(c);
                    }
                }
            }

            for (&ctx, &keep) in affected_contexts.iter().zip(&keep_extent) {
                let id = tx.make_defined_grob_in(ctx, "SpanBarStub", Cause::Grob(spanbar_id))?;
                let it = tx.grob_handle(id);
                grob::set_parent(tx.heap(), it, spanbar, X);
                if !keep {
                    grob::suicide(tx.heap(), it);
                }
            }
        }

        self.spanbars.clear();
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // remove unused contexts
        self.axis_groups.retain(|&(_, c)| !removable(tx, c));
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.spanbars.iter().map(|&(g, _)| g));
        out.extend(self.axis_groups.iter().map(|&(g, _)| g));
    }
}
