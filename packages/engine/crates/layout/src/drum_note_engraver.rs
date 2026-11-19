//! `drum-note-engraver.cc` (MS4 step 13): `Drum_notes_engraver`, a class
//! of layout's ([`DRUM_NOTES_ENGRAVER`]) that shadows the music crate's
//! placeholder of its name (D67, D108).
//!
//! - `process_music` (`:57–121`): a `NoteHead` of each note heard, with
//!   the `staff-position` and `style` its `drum-type` has in
//!   `drumStyleTable`, and, where the table gives the drum a script (with
//!   its direction), a `Script` made from it on the head
//!   (`make_script_from_event`, `script_engraver.rs`): its Y parent and a
//!   support; LilyPond's error at the note for a script that
//!   `scriptDefinitions` lacks.
//! - The acknowledgers (`:123–148`): a stem each script's support, and
//!   its `direction-source` where it has a `side-relative-direction`; a
//!   note column each script's support, and the X parent of one on the Y
//!   axis that has none.
//! - `stop_translation_timestep` (`:150–155`) forgets them.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, event_property};
use lily_engine_scheme::{Exception, Tag, Value};

use crate::geometry::{X, Y};
use crate::script_engraver::make_script_from_event;
use crate::{grob, side_position};

type Run<T> = Result<T, Exception>;

pub static DRUM_NOTES_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Drum_notes_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["note-event"],
    acknowledges: &[("stem-interface", false), ("note-column-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(DrumNotesEngraver::default())),
    reads: &["drumStyleTable"],
    writes: &[],
};

/// `scripts_` and `events_`.
#[derive(Clone, Default)]
struct DrumNotesEngraver {
    scripts: Vec<Value>,
    events: Vec<Value>,
}

/// `ly_scm_write_string`: `v` as `write` prints it.
fn written(tx: &mut Tx<'_>, v: Value) -> String {
    let mut out = String::new();
    tx.heap().write(v, &mut out);
    out
}

impl Work for DrumNotesEngraver {
    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.events.push(ev);
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.events.is_empty() {
            return Ok(());
        }

        let tab = tx.get("drumStyleTable");
        for i in 0..self.events.len() {
            let ev = self.events[i];
            let id = tx.make_defined_grob("NoteHead", Cause::Event(ev))?;
            let note = tx.grob_handle(id);

            let drum_type = event_property(tx.heap(), ev, "drum-type");

            let mut defn = Value::NULL;

            if tx.call("hash-table?", &[tab])? == Value::TRUE {
                defn = tx.call("hashq-ref", &[tab, drum_type, Value::NULL])?;
            }

            let heap = tx.heap();
            let (Some(style), Some(rest)) = (heap.car(defn), heap.cdr(defn)) else {
                continue;
            };
            let (Some(mut script), Some(rest)) = (heap.car(rest), heap.cdr(rest)) else {
                continue;
            };
            let pos = heap.car(rest).unwrap_or(Value::NULL);

            if heap.is_exact_integer(pos) {
                grob::set_property(heap, note, "staff-position", pos);
            }
            if style.tag() == Tag::Symbol {
                grob::set_property(heap, note, "style", style);
            }

            let mut dir = Value::NULL;
            if let (Some(s), Some(d)) = (heap.car(script), heap.cdr(script)) {
                dir = d;
                script = s;
            }

            if script != Value::FALSE {
                // Error out if script doesn't exist
                let defs = tx.get("scriptDefinitions");
                if tx.call("assoc", &[script, defs])? == Value::FALSE {
                    let text = format!("unrecognised percussion sign: \"{}\"", written(tx, script));
                    let origin = event_property(tx.heap(), ev, "origin");
                    let kind = tx.heap().intern("error");
                    let text = tx.heap().string(text.as_str());
                    tx.call("%diagnose", &[kind, origin, text])?;
                }

                let id = tx.make_defined_grob("Script", Cause::Event(ev))?;
                let p = tx.grob_handle(id);
                make_script_from_event(tx, p, script, 0)?;
                if matches!(dir.as_fixnum(), Some(-1..=1)) {
                    grob::set_property(tx.heap(), p, "direction", dir);
                }
                grob::set_parent(tx.heap(), p, note, Y);
                side_position::add_support(tx.host, p, note)?;
                self.scripts.push(p);
            }
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        for &e in &self.scripts {
            if which == 0 {
                // acknowledge_stem
                let d = grob::property(tx.host, e, "side-relative-direction")?;
                if matches!(d.as_fixnum(), Some(-1 | 1)) {
                    let heap = tx.heap();
                    let sym = heap.intern("direction-source");
                    grob::set_object(heap, e, sym, g);
                }
            } else if grob::get_parent(tx.heap(), e, X).is_none()
                && side_position::is_on_y_axis(tx.host, e)?
            {
                // acknowledge_note_column
                grob::set_parent(tx.heap(), e, g, X);
            }

            side_position::add_support(tx.host, e, g)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.scripts.clear();
        self.events.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(&self.scripts);
        out.extend(&self.events);
    }
}
