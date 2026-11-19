//! `arpeggio-engraver.cc` (part 4's, MS3 step 54, 190 lines):
//! `Arpeggio_engraver`, a class of layout's ([`ARPEGGIO_ENGRAVER`]) that
//! the core passes at its `\layout` call and that a group finds before the
//! music crate's placeholder of the name (D67, D108).
//!
//! An arpeggio, chord slur or non-arpeggiato event heard, the first of a
//! timestep's (`assign_event_once`, `:73–98`), makes at `process_music`
//! (`:139–157`) an `Arpeggio`, `ChordSlur` or `ChordBracket` caused by it.
//! Each stem acknowledged (`:100–110`) is one of its `stems`, the first its
//! Y parent; each rhythmic head (`:112–120`) one of its side supports; each
//! note column (`:122–127`) takes it as a conditional item.
//! `stop_translation_timestep` (`:159–164`) forgets both.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, assign_event_once};
use lily_engine_scheme::{Exception, Value};

use crate::geometry::Y;
use crate::{grob, grob_array, separation_item, side_position};

type Run<T> = Result<T, Exception>;

pub static ARPEGGIO_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Arpeggio_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["arpeggio-event", "chord-slur-event", "non-arpeggiato-event"],
    acknowledges: &[
        ("stem-interface", false),
        ("rhythmic-head-interface", false),
        ("note-column-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(ArpeggioEngraver::default())),
    reads: &[],
    writes: &[],
};

/// The grob an event makes, by the listener that heard it.
const ITEM_NAMES: [&str; 3] = ["Arpeggio", "ChordSlur", "ChordBracket"];

/// `arpeggio_` and `arpeggio_event_` by their handles, and
/// `arpeggio_type_` as the index of its name in [`ITEM_NAMES`].
#[derive(Clone, Default)]
struct ArpeggioEngraver {
    arpeggio: Option<Value>,
    arpeggio_event: Option<Value>,
    arpeggio_type: usize,
}

impl Work for ArpeggioEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.arpeggio_type as i64);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        if assign_event_once(tx, &mut self.arpeggio_event, ev)? {
            self.arpeggio_type = which;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let Some(arpeggio) = self.arpeggio else {
            return Ok(());
        };
        let g = tx.grob_handle(grob);
        match which {
            // acknowledge_stem
            0 => {
                if grob::get_parent(tx.heap(), arpeggio, Y).is_none() {
                    grob::set_parent(tx.heap(), arpeggio, g, Y);
                }
                let sym = tx.heap().intern("stems");
                grob_array::add_grob(tx.host, arpeggio, sym, g)
            }
            /*
              We can't catch local key items (accidentals) from Voice context,
              see Local_key_engraver
            */
            1 => side_position::add_support(tx.host, arpeggio, g),
            _ => separation_item::add_conditional_item(tx.host, g, arpeggio),
        }
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(ev) = self.arpeggio_event {
            let id = tx.make_defined_grob(ITEM_NAMES[self.arpeggio_type], Cause::Event(ev))?;
            self.arpeggio = Some(tx.grob_handle(id));
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.arpeggio = None;
        self.arpeggio_event = None;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.arpeggio);
        out.extend(self.arpeggio_event);
    }
}
