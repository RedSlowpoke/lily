//! The engine's figured bass (D223), 08 §2.4's `figured-bass` module,
//! which K4 splits from the core (MS4 step 20). Since MS4 step 16:
//!
//! - `Figured_bass_engraver` (`figured_bass_engraver.rs`): a `BassFigure`
//!   of each figure heard, on a `BassFigureLine` of its place in the
//!   chord, the lines stacked by a `BassFigureAlignment`; figures held
//!   over as `BassFigureContinuation`s, figures in brackets enclosed by a
//!   `BassFigureBracket`.
//! - `Figured_bass_position_engraver`
//!   (`figured_bass_position_engraver.rs`): a figured bass on a staff
//!   placed above its notes by a `BassFigureAlignmentPositioning`.
//! - `enclosing-bracket.cc` and `figured-bass-continuation.cc`
//!   (`enclosing_bracket.rs`, `figured_bass_continuation.rs`): the
//!   bracket's print and width, and the continuation's Y offset, centred
//!   on its figures; Rust under primitives that print as LilyPond's subrs
//!   and call them over `%host-call` (D127, [`HOSTS`]). The music crate
//!   keeps their placeholders (D67), which a VM without this crate loads
//!   `define-grobs.scm` with; [`install`] replaces them, as layout's
//!   replace its own. They are not in layout's callback table (05 §5.1),
//!   which names layout's alone: a Rust read of one calls it through the
//!   VM.
//!
//! The figures' text is the image's `figuredBassFormatter`
//! (`format-bass-figure`), and the continuation's stencil its
//! `figured-bass-continuation::print`.
#![no_std]
extern crate alloc;

pub mod enclosing_bracket;
pub mod figured_bass_continuation;
pub mod figured_bass_engraver;
pub mod figured_bass_position_engraver;

use lily_engine_scheme::{Exception, Extension, HostHandler, Prim, Vm};

/// The figured bass's translator classes, which the core runs a `\layout`
/// translation with after layout's (`lily_engine_layout::CLASSES`), each
/// found before the music crate's placeholder of its name (D67, D108).
pub static CLASSES: &[&lily_engine_music::TranslatorClass] = &[
    &figured_bass_engraver::FIGURED_BASS_ENGRAVER,
    &figured_bass_position_engraver::FIGURED_BASS_POSITION_ENGRAVER,
];

/// Defines the figured bass's primitives in `vm`'s `(lily)`, over the
/// music crate's placeholders. Layout's [`lily_engine_layout::install`]
/// comes first.
pub fn install(vm: &mut Vm) -> Result<(), Exception> {
    vm.define_primitives("lily", PRIMS, "")?;
    vm.add_host_handlers(&HOSTS);
    Ok(())
}

/// What [`lily_engine_scheme::Vm::restore`] needs of a VM that [`install`]
/// was given, after layout's.
pub static EXTENSION: Extension = Extension {
    prims: &[PRIMS],
    operators: None,
    hosts: &HOSTS,
    loaders: &[],
};

/// What `%host-call` runs for the figured bass's callbacks (D127).
static HOSTS: [HostHandler; 2] = {
    let [eb] = enclosing_bracket::HOSTS;
    let [fbc] = figured_bass_continuation::HOSTS;
    [eb, fbc]
};

const fn p(name: &'static str, f: lily_engine_scheme::PrimFn) -> Prim {
    Prim {
        name,
        min: 1,
        max: 1,
        f,
    }
}

static PRIMS: &[Prim] = &[
    p("ly:enclosing-bracket::print", enclosing_bracket::ly_print),
    p("ly:enclosing-bracket::width", enclosing_bracket::ly_width),
    p(
        "ly:figured-bass-continuation::center-on-figures",
        figured_bass_continuation::ly_center_on_figures,
    ),
];
