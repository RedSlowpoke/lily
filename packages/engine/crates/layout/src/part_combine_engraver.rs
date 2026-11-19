//! `part-combine-engraver.cc` (MS8 step 7, part 2, 162 lines; OPEN A44):
//! `Part_combine_engraver`, a class of layout's ([`PART_COMBINE_ENGRAVER`])
//! that shadows the music crate's placeholder of its name (D67, D108); the
//! part combiner's iterator is the music crate's (MS5 step 6a).
//!
//! - A `part-combine-event` heard is the timestep's (the first of two) and
//!   the one waiting to make a text (`:58–65`); a `note-event` heard is
//!   remembered for the timestep (`note_listener_`).
//! - `process_music` (`:91–104`): while one waits and
//!   `printPartCombineTexts` is `#t`, a `CombineTextScript` of
//!   `soloText`, `soloIIText` or `aDueText` by its `part-combine-status`
//!   (`solo1`, `solo2`, `unisono`), if that is a markup (`create_item`,
//!   `:72–89`), once a note is heard or at once without
//!   `partCombineTextsOnNote`. The engraver stays awake while one waits.
//! - The note heads acknowledged support the text, the first its Y parent
//!   if it is on the X axis and has none; the stems support it.
//! - `stop_translation_timestep` (`:125–131`) forgets the text, the new
//!   event and the note.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, assign_event_once, event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::clef_engraver::is_true;
use crate::geometry::Y;
use crate::grob;
use crate::side_position;
use crate::text::is_markup;

type Run<T> = Result<T, Exception>;

pub static PART_COMBINE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Part_combine_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["part-combine-event", "note-event"],
    acknowledges: &[("note-head-interface", false), ("stem-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(PartCombineEngraver::default())),
    reads: &[
        "printPartCombineTexts",
        "partCombineTextsOnNote",
        "soloText",
        "soloIIText",
        "aDueText",
    ],
    writes: &[],
};

const PART_COMBINE: usize = 0;
const NOTE_HEAD: usize = 0;

/// The engraver's members: the text by its handle, the events.
#[derive(Clone, Default)]
struct PartCombineEngraver {
    text: Option<Value>,
    new_event: Option<Value>,
    note_heard: bool,
    /// The event, possibly from an earlier moment, waiting to make a text.
    waiting_event: Option<Value>,
}

impl PartCombineEngraver {
    /// `create_item`: the text of the event's status, if it is a markup.
    fn create_item(&mut self, tx: &mut Tx<'_>, ev: Value) -> Run<()> {
        let what = event_property(tx.heap(), ev, "part-combine-status");
        let text = match tx.heap().name(what) {
            Some("solo1") => tx.get("soloText"),
            Some("solo2") => tx.get("soloIIText"),
            Some("unisono") => tx.get("aDueText"),
            _ => Value::NULL,
        };
        if is_markup(tx.host, text)? {
            let id = tx.make_defined_grob("CombineTextScript", Cause::Event(ev))?;
            let t = tx.grob_handle(id);
            grob::set_property(tx.heap(), t, "text", text);
            self.text = Some(t);
        }
        Ok(())
    }
}

impl Work for PartCombineEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[self.note_heard]);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        if which == PART_COMBINE {
            assign_event_once(tx, &mut self.new_event, ev)?;
            // If two events occur at the same moment, discard the second as
            // the warning indicates.
            self.waiting_event = self.new_event;
        } else {
            self.note_heard = true;
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(waiting) = self.waiting_event
            && is_true(tx.get("printPartCombineTexts"))
            && (self.note_heard || !is_true(tx.get("partCombineTextsOnNote")))
        {
            self.create_item(tx, waiting)?;
            self.waiting_event = None;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let Some(t) = self.text else {
            return Ok(());
        };
        let g = tx.grob_handle(grob);
        side_position::add_support(tx.host, t, g)?;
        if which == NOTE_HEAD
            && side_position::is_on_x_axis(tx.host, t)?
            && grob::get_parent(tx.heap(), t, Y).is_none()
        {
            grob::set_parent(tx.heap(), t, g, Y);
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.text = None;
        self.new_event = None;
        self.note_heard = false;
        // Awake at the next timestep while an event waits for its text.
        if self.waiting_event.is_some() {
            tx.stay_awake();
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.text);
        out.extend(self.new_event);
        out.extend(self.waiting_event);
    }
}
