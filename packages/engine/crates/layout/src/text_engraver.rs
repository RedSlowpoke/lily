//! `text-engraver.cc` (MS3 step 19): `Text_engraver`, a class of layout's
//! ([`TEXT_ENGRAVER`]) that the core passes at its `\layout` call and that
//! a group finds before the music crate's placeholder of the name (D67,
//! D108).
//!
//! The text scripts heard (`listen_text_script`, `:51–55`); at
//! `process_music` (`:57–82`) a `TextScript` of each, its cause the event,
//! its `script-priority` its own (200 if it is no number) plus the event's
//! place among the timestep's, its direction the event's if it is up or
//! down, its `text` the event's. Each note column acknowledged
//! (`acknowledge_note_column`, `:84–100`), or its rest where it has no
//! heads, is made the X parent of the scripts that have none.
//! `stop_translation_timestep` (`:102–107`) forgets them.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, event_property, grob_of,
};
use lily_engine_scheme::{Exception, Value};

use crate::geometry::X;
use crate::grob;
use crate::grob_array;

type Run<T> = Result<T, Exception>;

pub static TEXT_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Text_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["text-script-event"],
    acknowledges: &[("note-column-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(TextEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `evs_` and `scripts_`, by their handles.
#[derive(Clone, Default)]
struct TextEngraver {
    evs: Vec<Value>,
    scripts: Vec<Value>,
}

impl Work for TextEngraver {
    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.evs.push(ev);
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for (i, &ev) in self.evs.iter().enumerate() {
            let id = tx.make_defined_grob("TextScript", Cause::Event(ev))?;
            let script = tx.grob_handle(id);
            self.scripts.push(script);

            /* see script-engraver.cc */
            let mut priority = grob::property(tx.host, script, "script-priority")?;
            let heap = tx.heap();
            if !heap.is_number(priority) {
                priority = heap.integer(200); // TODO: Explain magic.
            }
            let i = heap.integer(i as i64);
            // A number and an integer: `+` cannot fail.
            let priority = heap.add(priority, i).unwrap_or(priority);
            grob::set_property(heap, script, "script-priority", priority);

            let dir = event_property(heap, ev, "direction");
            if matches!(dir.as_fixnum(), Some(-1 | 1)) {
                grob::set_property(heap, script, "direction", dir);
            }

            let mark = event_property(heap, ev, "text");
            grob::set_property(heap, script, "text", mark);
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        // Make note column (or rest, if there are no heads) the parent of the script.
        let column = tx.grob_handle(grob);
        let heads = grob_array::extract_grob_array(tx.host, column, "note-heads")?;
        let x_parent = if heads.is_empty() {
            let sym = tx.heap().intern("rest");
            let rest = grob_array::object(tx.host, column, sym)?;
            grob_of(tx.heap(), rest).map(|_| rest)
        } else {
            Some(column)
        };

        let heap = tx.heap();
        for &el in &self.scripts {
            if let Some(p) = x_parent
                && grob::get_parent(heap, el, X).is_none()
            {
                grob::set_parent(heap, el, p, X);
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.evs.clear();
        self.scripts.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(&self.evs);
        out.extend(&self.scripts);
    }
}
