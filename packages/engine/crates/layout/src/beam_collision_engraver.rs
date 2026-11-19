//! `beam-collision-engraver.cc` (MS4 step 4, ahead of step 5 since the
//! quanting reads what it gives): `Beam_collision_engraver`, a class of
//! layout's ([`BEAM_COLLISION_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108). Of the grobs it acknowledges (note
//! heads, stems, inline accidentals, clefs, key and time signatures, clef
//! modifiers, flags and beams), each with the context of the engraver that
//! made it, its `finalize` (`:96–182`) gives each beam as `covered-grobs`
//! those of its staves within its columns, sorted by their first column
//! (`Grob::less`, with libc++'s `std::sort`), but its own stems and their
//! heads, a beam that starts no later, and, with `collision-voice-only`,
//! another voice's; of the interfaces the beam's `collision-interfaces`
//! names.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{CtxId, FINALIZE, Kind, TranslatorClass, Tx, Work, grob_of};
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::grob_array;
use crate::item;
use crate::stem;

type Run<T> = Result<T, Exception>;

pub static BEAM_COLLISION_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Beam_collision_engraver",
    kind: Kind::Engraver,
    methods: FINALIZE,
    always: false,
    hears: &[],
    acknowledges: &[
        ("note-head-interface", false),
        ("stem-interface", false),
        ("accidental-interface", false),
        ("clef-interface", false),
        ("key-signature-interface", false),
        ("time-signature-interface", false),
        ("clef-modifier-interface", false),
        ("flag-interface", false),
        ("beam-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(BeamCollisionEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `beams_` and `covered_grobs_`: each grob with the context of the
/// engraver that made it.
#[derive(Clone, Default)]
struct BeamCollisionEngraver {
    beams: Vec<(Value, CtxId)>,
    covered_grobs: Vec<(Value, CtxId)>,
}

/// `covered_grob_has_interface`: whether the grob has an interface of the
/// beam's `collision-interfaces`.
fn covered_grob_has_interface(tx: &mut Tx<'_>, covered_grob: Value, beam: Value) -> Run<bool> {
    let interfaces = grob::property(tx.host, beam, "collision-interfaces")?;
    let heap = tx.heap();
    let mut l = interfaces;
    while let (Some(i), Some(rest)) = (heap.car(l), heap.cdr(l)) {
        if grob::has_any_interface(heap, covered_grob, &[i]) {
            return Ok(true);
        }
        l = rest;
    }
    Ok(false)
}

impl Work for BeamCollisionEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        for list in [&self.beams, &self.covered_grobs] {
            h.num(list.len() as i64);
            for &(_, c) in list {
                h.context(c);
            }
        }
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, id: u32) -> Run<()> {
        let g = tx.grob_handle(id);
        let context = tx.t.arena.borrow().records.grobs[id as usize].origin.0;
        match which {
            // An accidental only if it is inline.
            2 if !grob::has_interface(tx.heap(), g, "inline-accidental-interface") => {}
            8 => {
                self.beams.push((g, context));
                self.covered_grobs.push((g, context));
            }
            _ => self.covered_grobs.push((g, context)),
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.covered_grobs.is_empty() {
            return Ok(());
        }

        let heap = tx.heap();
        let first = |heap: &mut lily_engine_scheme::Heap, g: Value| {
            item::spanned_column_rank_interval(heap, g).0
        };
        let mut covered: Vec<(Value, CtxId, u64)> = self
            .covered_grobs
            .iter()
            .map(|&(g, c)| (g, c, first(heap, g)))
            .collect();
        let mut beams: Vec<(Value, CtxId, u64)> = self
            .beams
            .iter()
            .map(|&(g, c)| (g, c, first(heap, g)))
            .collect();
        lily_engine_skyline::sort(
            &mut covered,
            &|a: &(Value, CtxId, u64), b: &(Value, CtxId, u64)| a.2 < b.2,
        );
        lily_engine_skyline::sort(
            &mut beams,
            &|a: &(Value, CtxId, u64), b: &(Value, CtxId, u64)| a.2 < b.2,
        );
        let mut start = 0;

        for &(beam_grob, beam_context, _) in &beams {
            let stems = grob_array::extract_grob_array(tx.host, beam_grob, "normal-stems")?;
            let mut vertical_span = (i64::MAX, -i64::MAX);
            for &s in &stems {
                let vag = grob::get_vertical_axis_group_index(tx.host, s)?;
                if vag >= 0 {
                    vertical_span = (vertical_span.0.min(vag), vertical_span.1.max(vag));
                }
            }

            let beam_spanned_rank = item::spanned_column_rank_interval(tx.heap(), beam_grob);
            // Start considering grobs at the first grob whose end falls at
            // or after the beam's beginning.
            while start < covered.len()
                && item::spanned_column_rank_interval(tx.heap(), covered[start].0).1
                    < beam_spanned_rank.0
            {
                start += 1;
            }

            // Stop when the grob's beginning comes after the beam's end.
            for &(covered_grob, covered_grob_context, _) in &covered[start..] {
                let vag = grob::get_vertical_axis_group_index(tx.host, covered_grob)?;
                if !(vertical_span.0 <= vag && vag <= vertical_span.1) {
                    continue;
                }

                let covered_grob_spanned_rank =
                    item::spanned_column_rank_interval(tx.heap(), covered_grob);

                if covered_grob_spanned_rank.0 > beam_spanned_rank.1 {
                    break;
                }
                // Only consider grobs whose end falls at or after the beam's
                // beginning. If the grob is a beam, it cannot start before
                // beams_[i]. Also, if the user wants to check for
                // collisions only in the beam's voice, then make sure the
                // beam and the covered_grob are in the same voice.
                if covered_grob_spanned_rank.1 >= beam_spanned_rank.0
                    && !(grob::property(tx.host, beam_grob, "collision-voice-only")? == Value::TRUE
                        && covered_grob_context != beam_context)
                    && !(grob::has_interface(tx.heap(), covered_grob, "beam-interface")
                        && covered_grob_spanned_rank.0 <= beam_spanned_rank.0)
                    && covered_grob_has_interface(tx, covered_grob, beam_grob)?
                {
                    // Do not consider note heads attached to the beam.
                    if grob::has_interface(tx.heap(), covered_grob, "stem-interface")
                        && stem::get_beam(tx.host, covered_grob)?.is_some()
                    {
                        continue;
                    }

                    let sym = tx.heap().intern("stem");
                    let s = grob_array::object(tx.host, covered_grob, sym)?;
                    if grob_of(tx.heap(), s).is_some() {
                        let sym = tx.heap().intern("beam");
                        let b = grob_array::object(tx.host, s, sym)?;
                        if b == beam_grob {
                            continue;
                        }
                    }

                    let sym = tx.heap().intern("covered-grobs");
                    grob_array::add_grob(tx.host, beam_grob, sym, covered_grob)?;
                }
            }
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.beams.iter().map(|&(g, _)| g));
        out.extend(self.covered_grobs.iter().map(|&(g, _)| g));
    }
}
