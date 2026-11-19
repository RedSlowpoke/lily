//! `note-heads-engraver.cc` (MS3 step 16): `Note_heads_engraver`, a class
//! of layout's ([`NOTE_HEADS_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! `process_music` (`:60–121`): a `NoteHead`, or an
//! `ApproximatePitchNoteHead` for a note whose `pitch-approximate` is `#t`,
//! of each note heard, its cause the note; its `staff-position` the
//! value of `staffLineLayoutFunction` called with the pitch, which the
//! image runs (`:89`), or the pitch's steps without a procedure, 0 without
//! a pitch, `middleCPosition` added if it is a number; its `style` from
//! `shapeNoteStyles` by the note's step above `tonic`. The notes are
//! forgotten at the timestep's end (`:123–127`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, PROCESS, Pitch, STOP, Scale, TranslatorClass, Tx, Work, event_property,
};
use lily_engine_scheme::{Exception, Heap, Tag, Value};

use crate::grob;
use crate::stem::to_int;

type Run<T> = Result<T, Exception>;

pub static NOTE_HEADS_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Note_heads_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["note-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(NoteHeadsEngraver::default())),
    reads: &["middleCPosition", "staffLineLayoutFunction"],
    writes: &[],
};

/// `note_evs_`.
#[derive(Clone, Default)]
struct NoteHeadsEngraver {
    notes: Vec<Value>,
}

/// `Pitch::steps`, in C++'s `int`s: the pitch's step and its octave in
/// steps of its scale.
pub(crate) fn steps(heap: &Heap, p: &Pitch) -> i32 {
    let count = heap
        .foreign::<Scale>(p.scale)
        .map_or(0, |s| s.tones.len() as i32);
    p.notename.wrapping_add(p.octave.wrapping_mul(count))
}

impl Work for NoteHeadsEngraver {
    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.notes.push(ev);
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let c0 = tx.get("middleCPosition");
        let layout_proc = tx.get("staffLineLayoutFunction");
        for ev in self.notes.clone() {
            let approximate = event_property(tx.heap(), ev, "pitch-approximate") == Value::TRUE;
            let name = if approximate {
                "ApproximatePitchNoteHead"
            } else {
                "NoteHead"
            };
            let id = tx.make_defined_grob(name, Cause::Event(ev))?;
            let note = tx.grob_handle(id);

            let pitch = event_property(tx.heap(), ev, "pitch");
            let pit = tx.heap().foreign::<Pitch>(pitch).copied();
            let mut pos = match pit {
                None => 0,
                Some(_) if tx.heap().is_procedure(layout_proc) => {
                    let v = tx.host.call(layout_proc, &[pitch])?;
                    to_int(tx.heap(), v)?
                }
                Some(p) => steps(tx.heap(), &p),
            };
            if tx.heap().is_number(c0) {
                pos = pos.wrapping_add(to_int(tx.heap(), c0)?);
            }
            let pos = tx.heap().integer(i64::from(pos));
            grob::set_property(tx.heap(), note, "staff-position", pos);

            // Shape note heads change on step of the scale.
            let shape_vector = tx.get("shapeNoteStyles");
            let Some(len) = tx.heap().vector_items(shape_vector).map(<[Value]>::len) else {
                continue;
            };
            // LilyPond reads a pitch it has not checked here.
            let Some(pit) = pit else { continue };
            let scm_tonic = tx.get("tonic");
            let tonic = tx
                .heap()
                .foreign::<Pitch>(scm_tonic)
                .map_or(0, |t| t.notename);
            // An `unsigned int` in C++: a negative remainder is past any vector.
            let delta = (pit.notename.wrapping_sub(tonic).wrapping_add(7) % 7) as u32 as usize;
            let heap = tx.heap();
            let style = match heap.vector_items(shape_vector) {
                Some(items) if delta < len => items[delta],
                _ => Value::NULL,
            };
            if style.tag() == Tag::Symbol {
                grob::set_property(heap, note, "style", style);
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.notes.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend_from_slice(&self.notes);
    }
}
