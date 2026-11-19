//! `fretboard-engraver.cc` (MS4 step 17): `Fretboard_engraver`
//! ([`FRETBOARD_ENGRAVER`]), which `FretBoards` consists.
//!
//! - The notes, string numbers and fingerings heard (`:73–89`), forgotten
//!   at the timestep's end (`:118–125`).
//! - `process_music` (`:91–116`): with a note, a `FretBoard` caused by the
//!   first, given to `noteToFretFunction`, if a procedure, with the
//!   context, the notes and the string numbers and fingers of each
//!   ([`articulation_list`]) and the FretBoard: the image's
//!   `determine-frets` sets its `dot-placement-list`, from
//!   `predefinedDiagramTable` where that has the chord, and its stencil is
//!   the `fretted` part's `fret-board::calc-stencil`. With
//!   `chordChanges`, a FretBoard whose
//!   placements are those of the one before is visible only at the
//!   beginning of a line (`begin-of-line-visible`), as
//!   `ly:chord-name::after-line-breaking` reads it.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_layout::grob;
use lily_engine_music::{Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::tab_note_heads_engraver::articulation_list;

type Run<T> = Result<T, Exception>;

pub static FRETBOARD_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Fretboard_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["note-event", "string-number-event", "fingering-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(FretboardEngraver::default())),
    reads: &[
        "chordChanges",
        "defaultStrings",
        "highStringOne",
        "maximumFretStretch",
        "minimumFret",
        "noteToFretFunction",
        "predefinedDiagramTable",
        "stringTunings",
        "tablatureFormat",
    ],
    writes: &[],
};

/// `note_events_`, `tabstring_events_`, `fingering_events_` and
/// `last_placements_`.
#[derive(Clone)]
struct FretboardEngraver {
    notes: Vec<Value>,
    strings: Vec<Value>,
    fingerings: Vec<Value>,
    last_placements: Value,
}

impl Default for FretboardEngraver {
    fn default() -> FretboardEngraver {
        FretboardEngraver {
            notes: Vec::new(),
            strings: Vec::new(),
            fingerings: Vec::new(),
            last_placements: Value::FALSE,
        }
    }
}

impl Work for FretboardEngraver {
    fn listen(&mut self, _: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        match which {
            0 => self.notes.push(ev),
            1 => self.strings.push(ev),
            _ => self.fingerings.push(ev),
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.notes.is_empty() {
            return Ok(());
        }
        let depth = tx.heap().root_depth();
        let ran = self.make_fret_board(tx);
        tx.heap().unroot_to(depth);
        ran
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.notes.clear();
        self.strings.clear();
        self.fingerings.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend_from_slice(&self.notes);
        out.extend_from_slice(&self.strings);
        out.extend_from_slice(&self.fingerings);
        out.push(self.last_placements);
    }
}

impl FretboardEngraver {
    /// `process_music` with a note, its values rooted by the caller's
    /// depth.
    fn make_fret_board(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let heap = tx.heap();
        let tab_strings =
            articulation_list(heap, &self.notes, &self.strings, "string-number-event");
        heap.root(tab_strings);
        let fingers = articulation_list(heap, &self.notes, &self.fingerings, "fingering-event");
        heap.root(fingers);
        let id = tx.make_defined_grob("FretBoard", Cause::Event(self.notes[0]))?;
        let fret_board = tx.grob_handle(id);
        let fret_notes = tx.heap().list(&self.notes);
        tx.heap().root(fret_notes);
        let proc = tx.get("noteToFretFunction");
        if tx.heap().is_procedure(proc) {
            let context = tx.t.arena.borrow().handle(tx.context);
            let specified = tx.heap().list(&[tab_strings, fingers]);
            tx.heap().root(specified);
            tx.host
                .call(proc, &[context, fret_notes, specified, fret_board])?;
        }
        let changes = tx.get("chordChanges");
        let placements = grob::property(tx.host, fret_board, "dot-placement-list")?;
        if changes == Value::TRUE && tx.heap().equal(self.last_placements, placements) {
            grob::set_property(tx.heap(), fret_board, "begin-of-line-visible", Value::TRUE);
        }

        self.last_placements = placements;
        Ok(())
    }
}
