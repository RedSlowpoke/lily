//! `instrument-switch-engraver.cc` (MS8 step 7, part 2, 104 lines):
//! `Instrument_switch_engraver`, a class of layout's
//! ([`INSTRUMENT_SWITCH_ENGRAVER`]) that shadows the music crate's
//! placeholder of its name (D67, D108). LilyPond deprecates it.
//!
//! `process_music` (`:52–66`), at every timestep as LilyPond's: when
//! `instrumentCueName` is not `eq?` to the last one read (`'()` at the
//! start), an `InstrumentSwitch` of its text if that is a markup. Its
//! `stop_translation_time_step` is misspelt, so no timestep calls it, and
//! nothing here does.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::text::is_markup;

type Run<T> = Result<T, Exception>;

pub static INSTRUMENT_SWITCH_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Instrument_switch_engraver",
    kind: Kind::Engraver,
    methods: PROCESS,
    always: true,
    hears: &[],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(InstrumentSwitchEngraver::default())),
    reads: &["instrumentCueName"],
    writes: &[],
};

/// `cue_name_`, the last `instrumentCueName` read.
#[derive(Clone)]
struct InstrumentSwitchEngraver {
    cue_name: Value,
}

impl Default for InstrumentSwitchEngraver {
    fn default() -> InstrumentSwitchEngraver {
        InstrumentSwitchEngraver {
            cue_name: Value::NULL,
        }
    }
}

impl Work for InstrumentSwitchEngraver {
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let cue_text = tx.get("instrumentCueName");
        if cue_text != self.cue_name {
            if is_markup(tx.host, cue_text)? {
                let id = tx.make_defined_grob("InstrumentSwitch", Cause::None)?;
                let text = tx.grob_handle(id);
                grob::set_property(tx.heap(), text, "text", cue_text);
            }
            self.cue_name = cue_text;
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.push(self.cue_name);
    }
}
