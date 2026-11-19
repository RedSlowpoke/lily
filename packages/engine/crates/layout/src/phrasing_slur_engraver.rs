//! `phrasing-slur-engraver.cc` (MS4 step 7): `Phrasing_slur_engraver`,
//! `Slur_engraver` over phrasing slurs (`slur_engraver.rs`): it hears
//! `phrasing-slur-event`s, makes `PhrasingSlur`s, warns of a "phrasing
//! slur", never reads `doubleSlurs` nor sets a melisma property, and also
//! keeps the slurs among the objects it acknowledges. A class of layout's
//! ([`PHRASING_SLUR_ENGRAVER`]) that shadows the music crate's placeholder
//! of the name (D67).

use alloc::boxed::Box;

use lily_engine_music::{FINALIZE, Kind, PROCESS, STOP, TranslatorClass};

use crate::slur_engraver::{SlurEngraver, Slurs};

/// `Phrasing_slur_engraver::boot`'s acknowledgers.
const ACKNOWLEDGES: &[(&str, bool)] = &[
    ("inline-accidental-interface", false),
    ("fingering-interface", false),
    ("note-column-interface", false),
    ("slur-interface", false),
    ("script-interface", false),
    ("dots-interface", false),
    ("text-script-interface", false),
    ("tie-interface", true),
    ("tuplet-number-interface", false),
];

const PHRASING_SLURS: Slurs = Slurs {
    name: "Phrasing_slur_engraver",
    event: "phrasing-slur-event",
    grob: "PhrasingSlur",
    object: "phrasing slur",
    phrasing: true,
    acknowledges: ACKNOWLEDGES,
};

pub static PHRASING_SLUR_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Phrasing_slur_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    always: true,
    hears: &["phrasing-slur-event", "note-event"],
    acknowledges: ACKNOWLEDGES,
    must_be_last: false,
    make: Some(|| Box::new(SlurEngraver::new(&PHRASING_SLURS))),
    reads: &[],
    writes: &[],
};
