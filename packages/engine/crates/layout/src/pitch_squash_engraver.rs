//! `pitch-squash-engraver.cc` (R9's in MS4's split, pulled forward at MS4
//! step 10 by the operator's answer for T29's RhythmicStaffs):
//! `Pitch_squash_engraver`, a class of layout's that shadows the music
//! crate's placeholder of its name (D67, D108): each note head acknowledged
//! is put at `squashedPosition`, if that is a number.

use alloc::boxed::Box;

use lily_engine_music::{Kind, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::grob;

pub static PITCH_SQUASH_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Pitch_squash_engraver",
    kind: Kind::Engraver,
    methods: 0,
    always: false,
    hears: &[],
    acknowledges: &[("note-head-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(PitchSquashEngraver)),
    reads: &["squashedPosition"],
    writes: &[],
};

#[derive(Clone)]
struct PitchSquashEngraver;

impl Work for PitchSquashEngraver {
    /// `acknowledge_note_head`.
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Result<(), Exception> {
        let newpos = tx.get("squashedPosition");
        if tx.heap().is_number(newpos) {
            let g = tx.grob_handle(grob);
            grob::set_property(tx.heap(), g, "staff-position", newpos);
        }
        Ok(())
    }

    fn values(&self, _: &mut alloc::vec::Vec<Value>) {}
}
