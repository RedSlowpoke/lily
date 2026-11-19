//! `note-name-engraver.cc` (MS8 step 8, part 1, 119 lines):
//! `Note_name_engraver`, a class of layout's ([`NOTE_NAME_ENGRAVER`]) that
//! shadows the music crate's placeholder of its name (D67, D108).
//!
//! Each `note-event` heard in a timestep is kept. `process_music`
//! (`:47–75`): each note's name is `noteNameFunction` called with its
//! `pitch` and the context, after the first each preceded by
//! `noteNameSeparator` if that is a markup, else `" "`; LilyPond's
//! programming error where `noteNameFunction` is no procedure. With a name,
//! a `NoteName` caused by the first note, its `text` the names'
//! `make-concat-markup`. `stop_translation_timestep` forgets the notes.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, event_property};
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::text::is_markup;

type Run<T> = Result<T, Exception>;

pub static NOTE_NAME_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Note_name_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["note-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(NoteNameEngraver::default())),
    reads: &[
        "noteNameFunction",
        "noteNameSeparator",
        "printAccidentalNames",
        "printNotesLanguage",
        "printOctaveNames",
    ],
    writes: &[],
};

/// `events_`.
#[derive(Clone, Default)]
struct NoteNameEngraver {
    events: Vec<Value>,
}

impl NoteNameEngraver {
    /// The names' markups, in order, rooted from `depth` on.
    fn names(&self, tx: &mut Tx<'_>) -> Run<Vec<Value>> {
        let context = tx.t.arena.borrow().handle(tx.context);
        let mut markups = Vec::new();
        for (i, &ev) in self.events.iter().enumerate() {
            let pitch = event_property(tx.heap(), ev, "pitch");
            let proc = tx.get("noteNameFunction");
            let sep = tx.get("noteNameSeparator");

            if i > 0 {
                let sep = if is_markup(tx.host, sep)? {
                    sep
                } else {
                    tx.heap().string(" ")
                };
                tx.heap().root(sep);
                markups.push(sep);
            }

            if tx.heap().is_procedure(proc) {
                let pitch_name = tx.apply(proc, &[pitch, context])?;
                tx.heap().root(pitch_name);
                markups.push(pitch_name);
            } else {
                tx.programming_error("No translation function defined as noteNameFunction.")?;
            }
        }
        Ok(markups)
    }
}

impl Work for NoteNameEngraver {
    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.events.push(ev);
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(&first) = self.events.first() else {
            return Ok(());
        };
        let depth = tx.heap().root_depth();
        let made = self.names(tx).and_then(|markups| {
            if markups.is_empty() {
                return Ok(());
            }
            let id = tx.make_defined_grob("NoteName", Cause::Event(first))?;
            let n = tx.grob_handle(id);
            let list = tx.heap().list(&markups);
            tx.heap().root(list);
            let text = tx.call("make-concat-markup", &[list])?;
            grob::set_property(tx.heap(), n, "text", text);
            Ok(())
        });
        tx.heap().unroot_to(depth);
        made
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.events.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(&self.events);
    }
}
