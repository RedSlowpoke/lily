//! pdf.wasm's own exports (06 §7.3, D270): the host writes its input into
//! the buffer `lily_pdf_input` gives, calls `lily_pdf_write`, and reads the
//! PDF and the warnings back. The input is the core's display list and the
//! text fonts, each a u32 little-endian length before its bytes:
//!
//!   list, count, then count times: name, otf
//!
//! Before it, with the display list alone as the input, `lily_pdf_fonts`
//! names the text fonts the list needs, which the host then fetches.
//!
//! The module keeps nothing between exports but its buffers, and is dropped
//! after an export, its memory with it (B4). The core gives the display list
//! when the ABI's `lily_pdf` asks for it (MS8 step 6, D271).
extern crate std;

use alloc::string::String;
use alloc::vec::Vec;

use crate::{Document, Font, write};

struct State {
    input: Vec<u8>,
    output: Vec<u8>,
    warnings: String,
}

/// The module's one state, in a cell the one thread of wasm32 reaches.
struct Cell(core::cell::UnsafeCell<State>);

// SAFETY: wasm32 runs one thread.
unsafe impl Sync for Cell {}

static STATE: Cell = Cell(core::cell::UnsafeCell::new(State {
    input: Vec::new(),
    output: Vec::new(),
    warnings: String::new(),
}));

fn state() -> &'static mut State {
    // SAFETY: one thread, and no export holds the state across another.
    unsafe { &mut *STATE.0.get() }
}

/// A buffer of `len` bytes for the input, which the next `lily_pdf_input`
/// replaces.
#[unsafe(no_mangle)]
pub extern "C" fn lily_pdf_input(len: u32) -> *mut u8 {
    let s = state();
    s.input = alloc::vec![0; len as usize];
    s.input.as_mut_ptr()
}

/// The PostScript names of the text fonts of the input, a display list
/// alone, one a line, as the output: 0 when they are, 1 when the input is
/// not a display list. The input stays for the next `lily_pdf_input`.
#[unsafe(no_mangle)]
pub extern "C" fn lily_pdf_fonts() -> u32 {
    let s = state();
    let Ok(doc) = Document::decode(&s.input) else {
        return 1;
    };
    s.output = doc.fonts.join("\n").into_bytes();
    0
}

/// Writes the PDF of the input: 0 when it is written, 1 when the input is
/// not a display list and its fonts.
#[unsafe(no_mangle)]
pub extern "C" fn lily_pdf_write() -> u32 {
    let s = state();
    let input = core::mem::take(&mut s.input);
    let Some((list, fonts)) = parse(&input) else {
        return 1;
    };
    let Ok(doc) = Document::decode(list) else {
        return 1;
    };
    let written = write(&doc, &fonts);
    s.output = written.pdf;
    s.warnings = written.warnings.join("\n");
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn lily_pdf_output() -> *const u8 {
    state().output.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn lily_pdf_output_len() -> u32 {
    state().output.len() as u32
}

/// The warnings, one a line, in UTF-8.
#[unsafe(no_mangle)]
pub extern "C" fn lily_pdf_warnings() -> *const u8 {
    state().warnings.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn lily_pdf_warnings_len() -> u32 {
    state().warnings.len() as u32
}

fn parse(input: &[u8]) -> Option<(&[u8], Vec<Font<'_>>)> {
    let mut r = Pieces { input, at: 0 };
    let list = r.bytes()?;
    let mut fonts = Vec::new();
    for _ in 0..r.u32()? {
        let name = core::str::from_utf8(r.bytes()?).ok()?;
        fonts.push(Font {
            name,
            otf: r.bytes()?,
        });
    }
    (r.at == input.len()).then_some((list, fonts))
}

struct Pieces<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> Pieces<'a> {
    fn u32(&mut self) -> Option<u32> {
        let v = u32::from_le_bytes(self.input.get(self.at..self.at + 4)?.try_into().ok()?);
        self.at += 4;
        Some(v)
    }

    fn bytes(&mut self) -> Option<&'a [u8]> {
        let n = self.u32()? as usize;
        let bytes = self.input.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(bytes)
    }
}
