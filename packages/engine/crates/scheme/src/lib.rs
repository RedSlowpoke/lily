//! The engine's Scheme (chapter 03 §5): an interpreter of the Guile subset
//! that LilyPond's Scheme and the scores use, written for the engine rather
//! than ported, so that its heap has the roots, epochs and foreign objects
//! the parser and the music model need (03 §4.5).
//!
//! S1 so far: [`Value`], one 8-byte word with fixnums and immediates, and the
//! [`Heap`]: interned symbols and keywords, typed arenas in chunks with an
//! epoch per chunk, pairs, strings ([`Str`]), vectors and [`Foreign`]
//! objects, a root stack for Rust, and a precise mark-sweep collector that
//! runs at safe points only (03 §5.1, R3, R11, R12). Numbers as Guile 3 has
//! them: fixnums, [`BigInt`] bignums, exact ratios, flonums and complex
//! numbers, with Guile's primitives, printer and reader of them (R13).
//!
//! S2 so far: [`read`], Guile 3's reader at a byte offset, with the spans of
//! what it read, Guile's read errors and LilyPond's `#{ … #}` (R7, R8); the
//! compiler to bytecode and the [`Vm`] that runs it with proper tail calls
//! on stacks of its own, never the native one (R2), with multiple values and
//! the primitives chapter 03 Appendix B's benchmarks use; `catch` and
//! `throw`, escapes, `dynamic-wind`, fluids and parameters, `*location*`,
//! the cancel check (R9, R14), `eval`, and Guile 3's wording of its errors
//! (R7).
//!
//! S3 so far: global slots with versions, in Guile's modules (`(guile)`,
//! `(guile-user)`, `(lily)` and those `define-module` makes), with
//! `use-modules`, `export`, `define-public`, `@`, `@@` and `eval` in a
//! module, and R10's read tracking, off unless the host turns it on. An
//! expander before the compiler: Guile's extended forms (`lambda*`,
//! `define*`, `case-lambda`, `do`, `while`, `quasiquote`, `eval-when`),
//! hygienic `syntax-rules`, `defmacro` and `define-macro`, and `(lily)`'s
//! native `assert` and `define-syntax-function` (D60). The Guile primitives
//! LilyPond's library files call: Guile's list, string and character
//! procedures, `(srfi srfi-1)`, hash tables with weak keys, object and
//! procedure properties, procedures with setters, string ports,
//! `simple-format` and `(ice-9 format)`'s subset. The Guile modules the two
//! library files import (`library/`, `goops.rs`) and `(lily)`'s own
//! definitions (`lily.rs`), so that `lily-library.scm` and
//! `music-functions.scm` load from source.
//!
//! S4 so far, natively: [`Vm::write_image`] writes a VM's heap, code,
//! slots and modules as an image, with indices where pointers would be, and
//! [`Vm::restore`] makes a VM of it again with no relocation (03 §5.6, R5).
//!
//! S5 so far (MS6 step 2): the write barrier and the undo log (R11) in the
//! heap's mutable accessors and the global slots, on while an item runs
//! ([`Vm::begin_item`] to [`Vm::end_item`]), its reads recorded with their
//! versions and its writes in order ([`Item`], R10), and [`Vm::undo`]; the
//! scores' item boundaries are the parser's (02 §5.7), the core's compile
//! one item until then.
//!
//! Other crates add primitives of their own ([`Vm::define_primitives`]):
//! the music model's `ly:` functions over its [`Foreign`] values. They work
//! on a [`Cx`] through its public methods and raise errors with them, and
//! restoring an image needs their [`Extension`].
#![no_std]
extern crate alloc;

mod array;
mod bignum;
mod compile;
mod control;
mod errors;
mod expand;
mod format;
mod goops;
mod hash;
mod heap;
mod image;
mod library;
mod lily;
mod lists;
mod module;
mod number;
mod ports;
mod prims;
mod reader;
mod regex;
mod rows;
mod strings;
mod syntax_rules;
mod value;
mod vm;

pub use bignum::BigInt;
pub use compile::MAX_NESTING;
pub use control::{CANCEL_INTERVAL, CancelCheck};
pub use heap::{CHUNK, Foreign, Heap, Root, Stats, Tracer, collecting_ms, set_collect_every};
pub use image::{Extension, ImageError, ImageReader, ImageWriter, Loader, RESTORE_MARKS};
pub use lily::{FileSource, HostFn, HostHandler, Hosted, PartSource, percent_encode};
pub use number::{Num, NumError, NumResult, Rounding, parse as parse_number};
pub use prims::{OperatorFn, Prim, PrimFn, REST};
pub use reader::{
    Array, Bytevector, Embedded, LilyScan, MAX_DEPTH, Position, Read, ReadError, Span, read,
};
pub use rows::{ROW, Rows};
pub use value::{Str, Tag, Value};
pub use vm::{Cx, Exception, Item, MAX_FRAMES, MAX_STACK, Mark, Redo, Slot, Source, Thrown, Vm};
