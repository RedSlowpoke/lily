//! `chord-tremolo-engraver.cc` (MS4 step 5): `Chord_tremolo_engraver`, a
//! class of layout's ([`CHORD_TREMOLO_ENGRAVER`]) that the core passes at
//! its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108). A tremolo repeat's span
//! (`listen_tremolo_span`, `:71–87`) makes a `Beam` (`process_music`,
//! `:89–96`) over the stems in it (`acknowledge_stem`, `:108–150`), each
//! pair beamed whole with the tremolo's flags, the gaps of a repeat of
//! notes other than halves, and the beam's end announced after each note
//! but the first, so that a line may break after it; LilyPond's warnings
//! for an end of no tremolo, a stem of no rhythm and a tremolo left
//! unterminated (`finalize`, `:98–106`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Kind, PROCESS, TranslatorClass, Tx, Work, assign_event_once, event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::beam;
use crate::dynamic_align_engraver::direction as to_direction;
use crate::geometry::Direction;
use crate::grob;
use crate::stem;
use crate::stem_engraver::{int_or, intlog2};
use crate::tie_engraver::in_event_class;

type Run<T> = Result<T, Exception>;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

pub static CHORD_TREMOLO_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Chord_tremolo_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | FINALIZE,
    always: false,
    hears: &["tremolo-span-event"],
    acknowledges: &[("stem-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(ChordTremoloEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `repeat_`, `beam_` by its record and handle, and `previous_stem_`.
#[derive(Clone, Default)]
struct ChordTremoloEngraver {
    repeat: Option<Value>,
    beam: Option<(u32, Value)>,
    previous_stem: Option<Value>,
}

impl Work for ChordTremoloEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        let span_dir = to_direction(event_property(tx.heap(), ev, "span-direction"));
        if span_dir == -1 {
            assign_event_once(tx, &mut self.repeat, ev)?;
        } else if span_dir == 1 {
            if self.repeat.is_none() {
                tx.event_warning(ev, "No tremolo to end")?;
            }
            self.repeat = None;
            self.beam = None;
            self.previous_stem = None;
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let (Some(repeat), None) = (self.repeat, self.beam) {
            let id = tx.make_defined_grob("Beam", Cause::Event(repeat))?;
            self.beam = Some((id, tx.grob_handle(id)));
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((id, beam)) = self.beam {
            if let Some(repeat) = self.repeat {
                tx.event_warning(repeat, "unterminated chord tremolo")?;
            }
            tx.announce_end_grob(id);
            grob::suicide(tx.heap(), beam);
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let (Some((id, beam)), Some(repeat)) = (self.beam, self.repeat) else {
            return Ok(());
        };
        let heap = tx.heap();
        let v = event_property(heap, repeat, "tremolo-type");
        let tremolo_type = int_or(heap, v, 1);
        let flags = (intlog2(heap, tremolo_type)? - 2).max(0);
        let v = event_property(heap, repeat, "repeat-count");
        let repeat_count = int_or(heap, v, 1);
        let gap_count = flags.min(intlog2(heap, repeat_count)? + 1);

        let s = tx.grob_handle(grob);
        if let Some(previous) = self.previous_stem {
            // FIXME: We know that the beam has ended only in
            // listen_tremolo_span but then it is too late for
            // Spanner_break_forbid_engraver to allow a line break... So, as
            // a nasty hack, announce the spanner's end after each note
            // except the first. The only "drawback" is that for multi-note
            // tremolos a break would theoretically be allowed after the
            // second note (but since that note is typically not at a
            // barline, I don't think anyone will ever notice!)
            tx.announce_end_grob(id);
            // Create the whole beam between previous and current note
            stem::set_beaming(tx.host, previous, flags, RIGHT)?;
            stem::set_beaming(tx.host, s, flags, LEFT)?;
        }

        if stem::duration_log(tx.host, s)? != 1 {
            let heap = tx.heap();
            let v = heap.integer(i64::from(gap_count));
            grob::set_property(heap, beam, "gap-count", v);
        }

        let ev = grob::ultimate_event_cause(tx.heap(), s);
        if ev.is_some_and(|ev| in_event_class(tx.heap(), ev, "rhythmic-event")) {
            beam::add_stem(tx.host, beam, s)?;
        } else {
            grob::warning(tx.host, s, "stem must have Rhythmic structure")?;
        }

        // Store current grob, so we can possibly end the spanner here (and
        // reset the beam direction to RIGHT)
        self.previous_stem = Some(s);
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.repeat);
        out.extend(self.beam.map(|(_, b)| b));
        out.extend(self.previous_stem);
    }
}
