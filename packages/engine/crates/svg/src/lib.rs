//! The engine's SVG writer (chapter 06 §6, W3; D104): a page's stencil
//! written as LilyPond's SVG backend writes it, in 06 §6.2's file form.
//! O1's `stencil-interpret.cc` walks the stencil's expression
//! (`interpret.rs`); O3's outputter, `paper-outputter.cc` with
//! `output-svg.scm`'s primitives and `framework-svg.scm`'s page, writes
//! what the walk gives (`page.rs`), its numbers as `number.rs` prints them.
//!
//! The file form is the form of exported pages and of chapter 00's
//! comparator: each glyph defined once per page and drawn with `<use>`
//! (06 §3.2), its outline from W1's packs (06 §5.1). The preview form,
//! every glyph drawn in place, is W4's (MS6 step 10, `preview.rs`): each
//! line of a system a `<g data-lily-cell="DIGEST">`, the markup of what has
//! not changed kept from the compile before ([`Markup`]).
//!
//! The writer reads stencils and font metrics, `crates/layout`'s, and the
//! packs' outlines, `crates/fonts`'; of layout's grobs, only what
//! `grob-cause` links: a grob's cause and its origin.
#![no_std]
extern crate alloc;

pub mod interpret;
pub mod number;
mod page;
mod preview;

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_layout::Stencil;
use lily_engine_scheme::{Exception, Vm};

pub use interpret::{Call, Sink, interpret};
pub use number::Fixed;
pub use preview::{Markup, PageReport, write as write_preview};

/// What `framework-svg.scm` reads of the paper, and where the engine's file
/// tree is.
#[derive(Clone, Copy, Debug)]
pub struct Paper<'a> {
    /// `output-scale`: millimetres in a staff space, the page's unit
    /// (`layout-set-absolute-staff-size-in-module`'s staff height over 4).
    pub output_scale: f64,
    /// The directories a link's file is named below (06 §6.2): a link into
    /// one names its file by its path below it, as the engine's tree does
    /// (`textedit:///score.ly:…`). A grob of the engine's names its file so
    /// already; LilyPond's pages, whose links the references hold, name
    /// absolute paths.
    pub tree_roots: &'a [&'a str],
}

impl Paper<'_> {
    /// The `output-scale` of a staff size in points: 20 is
    /// `set-global-staff-size`'s default.
    pub fn output_scale_of(staff_size: f64) -> f64 {
        staff_size * lily_engine_fonts::POINT / 4.0
    }
}

/// A page written: its SVG, and LilyPond's warnings while writing it.
#[derive(Clone, Debug, Default)]
pub struct Page {
    pub svg: String,
    pub warnings: Vec<String>,
}

/// `output-stencil`: the page whose stencil is `stencil`, in the file form.
pub fn write_page(vm: &mut Vm, stencil: &Stencil, paper: &Paper<'_>) -> Result<Page, Exception> {
    page::write(vm, stencil, paper)
}
