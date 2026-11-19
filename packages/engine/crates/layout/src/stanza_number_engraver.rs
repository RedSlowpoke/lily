//! `stanza-number-engraver.cc` and `stanza-number-align-engraver.cc` (MS4
//! step 15): `Stanza_number_engraver` ([`STANZA_NUMBER_ENGRAVER`]) and
//! `Stanza_number_align_engraver` ([`STANZA_NUMBER_ALIGN_ENGRAVER`]),
//! classes of layout's that shadow the music crate's placeholders of their
//! names (D67, D108).
//!
//! - `Stanza_number_engraver::process_music` (`:51–61`): a
//!   `StanzaNumber` of `stanza` when it is a markup not `eq?` to the last
//!   one made; the timestep's syllables its supports (`:63–68`).
//! - `Stanza_number_align_engraver::stop_translation_timestep` (`:62–71`):
//!   each syllable of the timestep a support of each stanza number of it,
//!   both acknowledged in the Score's contexts below.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::side_position;
use crate::text::is_markup;

type Run<T> = Result<T, Exception>;

pub static STANZA_NUMBER_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Stanza_number_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    // `process_music` reads `stanza` every timestep.
    always: true,
    hears: &[],
    acknowledges: &[("lyric-syllable-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(StanzaNumberEngraver::default())),
    reads: &["stanza"],
    writes: &[],
};

/// `text_` by its handle and `last_stanza_`.
#[derive(Clone)]
struct StanzaNumberEngraver {
    text: Option<Value>,
    last_stanza: Value,
}

impl Default for StanzaNumberEngraver {
    fn default() -> StanzaNumberEngraver {
        StanzaNumberEngraver {
            text: None,
            last_stanza: Value::NULL,
        }
    }
}

impl Work for StanzaNumberEngraver {
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let stanza = tx.get("stanza");
        if is_markup(tx.host, stanza)? && stanza != self.last_stanza {
            self.last_stanza = stanza;
            let id = tx.make_defined_grob("StanzaNumber", Cause::None)?;
            let text = tx.grob_handle(id);
            grob::set_property(tx.heap(), text, "text", stanza);
            self.text = Some(text);
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        if let Some(text) = self.text {
            let g = tx.grob_handle(grob);
            side_position::add_support(tx.host, text, g)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.text = None;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.text);
        out.push(self.last_stanza);
    }
}

pub static STANZA_NUMBER_ALIGN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Stanza_number_align_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    always: false,
    hears: &[],
    acknowledges: &[
        ("lyric-syllable-interface", false),
        ("stanza-number-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(StanzaNumberAlignEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `lyrics_` and `stanza_numbers_`, by their handles.
#[derive(Clone, Default)]
struct StanzaNumberAlignEngraver {
    lyrics: Vec<Value>,
    stanza_numbers: Vec<Value>,
}

impl Work for StanzaNumberAlignEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let h = tx.grob_handle(grob);
        if which == 0 {
            self.lyrics.push(h);
        } else {
            self.stanza_numbers.push(h);
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for &lyric in self.lyrics.iter().rev() {
            for &number in self.stanza_numbers.iter().rev() {
                side_position::add_support(tx.host, number, lyric)?;
            }
        }
        self.stanza_numbers.clear();
        self.lyrics.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(&self.lyrics);
        out.extend(&self.stanza_numbers);
    }
}
