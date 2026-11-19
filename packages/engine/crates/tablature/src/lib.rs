//! The engine's tablature with its fret diagrams (D223), 08 §2.4's
//! `tablature` module, which K4 splits from the core (MS4 step 20). Since
//! MS4 step 7a (pulled forward from step 17, by the user's answer):
//!
//! - `Tab_note_heads_engraver` (`tab_note_heads_engraver.rs`): a
//!   `TabNoteHead` of each note given a string, its fret's label and its
//!   staff position from the context's procedures.
//! - `Tab_staff_symbol_engraver` (`tab_staff_symbol_engraver.rs`): a
//!   `TabStaff`'s staff symbol given a line for each string.
//! - Since MS4 step 17, `Fretboard_engraver` (`fretboard_engraver.rs`): a
//!   `FretBoard` of each chord, its dots the `noteToFretFunction`'s.
//!
//! Their classes ([`CLASSES`]) are given to a `\layout` translation beside
//! layout's, each found before the music crate's placeholder of its name
//! (D67, D108). What their grobs draw with is the `fretted` part's Scheme
//! (`tablature.scm`, `fret-diagrams.scm`), loaded before their layout
//! (A46, D228).
#![no_std]
extern crate alloc;

pub mod fretboard_engraver;
pub mod tab_note_heads_engraver;
pub mod tab_staff_symbol_engraver;

/// The tablature's translator classes, which the core runs a `\layout`
/// translation with after layout's (`lily_engine_layout::CLASSES`).
pub static CLASSES: &[&lily_engine_music::TranslatorClass] = &[
    &tab_note_heads_engraver::TAB_NOTE_HEADS_ENGRAVER,
    &tab_staff_symbol_engraver::TAB_STAFF_SYMBOL_ENGRAVER,
    &fretboard_engraver::FRETBOARD_ENGRAVER,
];
