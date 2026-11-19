//! Chapter 08 §4.2's C ABI over one [`Engine`] per instance, and beside it
//! chapter 03 Appendix B's bench ABI (`lsb_*`) and the parser's (`lsp_*`),
//! as the spike exported them until K2 (MS2 step 10).

use crate::bench::Bench;
use crate::{Engine, START_MARKS};
use std::alloc::{Layout, alloc, dealloc};
use std::cell::UnsafeCell;

struct Global<T>(UnsafeCell<T>);
// A wasm instance without threads: one caller at a time.
unsafe impl<T> Sync for Global<T> {}
static ENGINE: Global<Option<Engine>> = Global(UnsafeCell::new(None));

fn engine() -> &'static mut Engine {
    // The host calls lily_start first; anything else before it is a bug, and a trap.
    unsafe {
        (*ENGINE.0.get())
            .as_mut()
            .unwrap_or_else(|| core::arch::wasm32::unreachable())
    }
}

unsafe fn bytes<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if len == 0 {
        &[]
    } else {
        unsafe { core::slice::from_raw_parts(ptr, len) }
    }
}

unsafe fn text<'a>(ptr: *const u8, len: usize) -> &'a str {
    core::str::from_utf8(unsafe { bytes(ptr, len) }).unwrap_or("")
}

#[unsafe(no_mangle)]
pub extern "C" fn lily_alloc(len: usize) -> *mut u8 {
    unsafe { alloc(Layout::from_size_align_unchecked(len.max(1), 1)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lily_free(ptr: *mut u8, len: usize) {
    unsafe { dealloc(ptr, Layout::from_size_align_unchecked(len.max(1), 1)) }
}

/// Grows the memory once by most of what restoring an image of `len` bytes
/// takes, before the restore, which dlmalloc would otherwise grow 64 KiB
/// at a time: a block allocated and freed is the allocator's to use again
/// (MS3 step 56, D215). Twice the length is less than a restore of
/// `core.image` grows the memory by, so the memory after it is what it
/// would have been.
fn reserve(len: usize) {
    let size = len.saturating_mul(2);
    if size > 0 {
        let block = lily_alloc(size);
        if !block.is_null() {
            unsafe { lily_free(block, size) };
        }
    }
}

/// The debug build's (`core-debug.wasm`, 08 §1.3, D262): a panic's message,
/// which `panic = "immediate-abort"` drops, is passed to the host before
/// the trap.
#[cfg(feature = "debug")]
fn pass_panics_on() {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        fn lily_panic(message: *const u8, len: usize);
    }
    std::panic::set_hook(Box::new(|info| {
        let message = info.to_string();
        unsafe { lily_panic(message.as_ptr(), message.len()) }
    }));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lily_start(image: *const u8, len: usize) -> u32 {
    #[cfg(feature = "debug")]
    pass_panics_on();
    reserve(len);
    match marked(|mark| Engine::start_marked(unsafe { bytes(image, len) }, mark)) {
        Ok(mut e) => {
            // A worker reads its messages only between calls (H3, 08 §4.3).
            e.set_returns(true);
            // An allocation that fails is a trap: a compile near the cap stops first (D259).
            e.set_room(crate::short_of_memory);
            unsafe { *ENGINE.0.get() = Some(e) };
            0
        }
        Err(_) => 1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lily_put(
    kind: u32,
    name: *const u8,
    name_len: usize,
    data: *const u8,
    data_len: usize,
) {
    let data = if data.is_null() {
        None
    } else {
        Some(unsafe { bytes(data, data_len) })
    };
    engine().put(kind, unsafe { text(name, name_len) }, data);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lily_compile(
    revision: u32,
    root: *const u8,
    root_len: usize,
    visible_page: u32,
) {
    // protocol.ts' `visiblePage`, from 1: the page a preview shows, for which
    // the pages are written (W4, D253); the harness's 0 is no preview. A
    // page's compile leaves its performance to `lily_perform` (M7, D264).
    engine().set_preview(visible_page > 0);
    engine().set_performs(visible_page == 0);
    engine().set_visible_page(visible_page.saturating_sub(1));
    engine().compile(revision, unsafe { text(root, root_len) });
}

/// Starts the performance of the compile of `root` at `revision`, which a
/// page's compile left out (M7, D264): `lily_step` runs it, and its outputs
/// are the MIDI files, the playback map, and `stderr` and `diagnostics.json`
/// of what its `\midi` translations said; a
/// newer `lily_put` or `lily_compile` stops it, and the compile after it
/// resumes what the compile before it kept.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lily_perform(revision: u32, root: *const u8, root_len: usize) {
    engine().perform(revision, unsafe { text(root, root_len) });
}

/// Whether the compiles from here give each book's PDF display list (W7,
/// D271): `<book>.pdf.list` among their outputs, named as LilyPond names the
/// book's PDF, which pdf.wasm writes as the PDF (`lily_pdf_write`). `1` on;
/// a new instance has it off. A performance gives none.
#[unsafe(no_mangle)]
pub extern "C" fn lily_pdf(on: u32) {
    engine().set_pdf(on != 0);
}

/// Runs the compile for about `budget_ms`: `1` when it goes on at the next
/// call (H3, D254). A newer `lily_put` or `lily_compile` stops it.
#[unsafe(no_mangle)]
pub extern "C" fn lily_step(budget_ms: f64) -> u32 {
    engine().step(budget_ms)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lily_needs(out: *mut u32) {
    let (kind, name) = engine().needs().unwrap_or((0, ""));
    unsafe { *out = kind };
    unsafe { *out.add(1) = name.as_ptr() as u32 };
    unsafe { *out.add(2) = name.len() as u32 };
}

/// The outputs of the compile `lily_step` ended (`DONE`); after a `MORE`,
/// none, or the visible page a preview's compile has written, handed over
/// before its end (OPEN I7, D269): `<page>.svg` and `<page>.cells` as the
/// outputs at its end hold them, until the next call.
#[unsafe(no_mangle)]
pub extern "C" fn lily_outputs() -> u32 {
    engine().outputs().len() as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lily_output(i: u32, out: *mut u32) {
    let (name, data) = &engine().outputs()[i as usize];
    unsafe { *out = name.as_ptr() as u32 };
    unsafe { *out.add(1) = name.len() as u32 };
    unsafe { *out.add(2) = data.as_ptr() as u32 };
    unsafe { *out.add(3) = data.len() as u32 };
}

// --- chapter 03 Appendix B's bench ABI over the engine's Scheme. Its
// runner's env.lsb_write and env.lsb_now are not imported: the core's
// only import stays lily_now (08 §4.2), which is the collector's clock,
// and the output is read back with lsb_output.

static BENCH: Global<Option<Bench>> = Global(UnsafeCell::new(None));
static OUTPUT: Global<[u32; 2]> = Global(UnsafeCell::new([0; 2]));

fn bench() -> &'static mut Bench {
    // lsb_init comes first, as lily_start does for lily_*.
    unsafe {
        (*BENCH.0.get())
            .as_mut()
            .unwrap_or_else(|| core::arch::wasm32::unreachable())
    }
}

/// A fresh interpreter with its standard environment; 0 when it is ready.
#[unsafe(no_mangle)]
pub extern "C" fn lsb_init() -> i32 {
    unsafe { *BENCH.0.get() = Some(Bench::new()) };
    0
}

/// An interpreter restored from the image of `len` bytes at `ptr`, from
/// lsb_alloc, which it frees: in place of lsb_init. 0 when it is ready, 1
/// when the image is refused (bench::Bench::restore).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsb_restore(ptr: *mut u8, len: usize) -> i32 {
    reserve(len);
    let restored = marked(|mark| Bench::restore_marked(unsafe { bytes(ptr, len) }, mark));
    unsafe { lily_free(ptr, len) };
    match restored {
        Ok(b) => {
            unsafe { *BENCH.0.get() = Some(b) };
            0
        }
        Err(_) => 1,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn lsb_alloc(len: usize) -> *mut u8 {
    lily_alloc(len)
}

/// Reads and evaluates the UTF-8 text at `ptr`; 0 when it ran without error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsb_eval(ptr: *const u8, len: usize) -> i32 {
    match core::str::from_utf8(unsafe { bytes(ptr, len) }) {
        Ok(text) => !bench().eval(text) as i32,
        Err(_) => 2,
    }
}

/// Collects once, as `(gc)` does, so that lsb_stats gives the pause on the
/// heap as it is, and the bytes live after it; 0 when it ran.
#[unsafe(no_mangle)]
pub extern "C" fn lsb_collect() -> i32 {
    !bench().eval("(gc)") as i32
}

/// The last lsb_eval's output, then its error if it failed: a pointer to its address and length.
#[unsafe(no_mangle)]
pub extern "C" fn lsb_output() -> *const u32 {
    let output = bench().output();
    unsafe { *OUTPUT.0.get() = [output.as_ptr() as u32, output.len() as u32] };
    OUTPUT.0.get() as *const u32
}

/// The last lsb_eval's counts, as many f64s as bench::STATS names, in its order.
#[unsafe(no_mangle)]
pub extern "C" fn lsb_stats() -> *const f64 {
    bench().stats().as_ptr()
}

/// Gives the bench's layout W1's pack of `len` bytes at `ptr`
/// (`Bench::pack`); 0 when it was taken, 1 when refused (its error in
/// lsb_output).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsb_pack(ptr: *const u8, len: usize) -> i32 {
    !bench().pack(unsafe { bytes(ptr, len) }) as i32
}

/// Writes the page of the record at `record` in the file form `reps` times
/// (`Bench::svg`, MS3 step 9), links named below the roots at `roots`, one
/// a line; the milliseconds the writes took by lily_now, or -1 if the
/// record or a write failed. lsb_output gives the SVG, or the error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsb_svg(
    record: *const u8,
    record_len: usize,
    roots: *const u8,
    roots_len: usize,
    reps: u32,
) -> f64 {
    let roots: Vec<&str> = unsafe { text(roots, roots_len) }.lines().collect();
    bench()
        .svg(unsafe { text(record, record_len) }, &roots, reps)
        .unwrap_or(-1.0)
}

// --- the engine's last compile, timed, and traced: not 08 §4.2's ABI,
// which the studio's driver runs, but the harness's translate.html's and
// the tests' (MS2 step 27).

static TIMES: Global<[f64; 12]> = Global(UnsafeCell::new([0.0; 12]));

/// What the engine's last step that parsed took, as twelve f64s
/// (`Times::values`): its parse, its translations, how many, and the
/// longest, in milliseconds by lily_now; then (MS3 step 36) the
/// translations' layout and breaking, the pages' stencils, the SVG
/// writer, and the live grobs laid out; then (MS5 step 7) the `\midi`
/// translations' part of the translations and the MIDI files' writing;
/// then (MS4 step 6) the bytes of linear memory after the translations
/// and after the pages.
#[unsafe(no_mangle)]
pub extern "C" fn lsb_times() -> *const f64 {
    unsafe { *TIMES.0.get() = engine().times().values() };
    TIMES.0.get() as *const f64
}

/// Whether the engine's compiles translate their `\layout`s too and write
/// their trace (`Engine::set_trace`), for tools/compare's c1.test.mjs: 1 on.
#[unsafe(no_mangle)]
pub extern "C" fn lsb_trace(on: u32) {
    engine().set_trace(on != 0);
}

/// The breaks the engine's compiles break their lines at
/// (`Engine::set_breaks`, MS3 step 60), for tools/compare's tests: the
/// UTF-8 text of `len` bytes at `ptr`, a column's `when` a line (`main
/// grace`), none for the one line.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsb_breaks(ptr: *const u8, len: usize) {
    let whens: Vec<String> = unsafe { text(ptr, len) }
        .lines()
        .map(String::from)
        .collect();
    engine().set_breaks(&whens);
}

/// How many times the last compile parsed its root (`Engine::parses`): one
/// more than the stops at which it asked the host for files (MS2 step 30).
#[unsafe(no_mangle)]
pub extern "C" fn lsb_parses() -> u32 {
    engine().parses()
}

// --- the parts of the last lily_start or lsb_restore, timed: for the
// harness's restore.html (MS2 step 31).

static MARKS: Global<[f64; 2 + START_MARKS.len()]> =
    Global(UnsafeCell::new([0.0; 2 + START_MARKS.len()]));

/// Runs `start` with a mark that records lily_now at the end of each part.
fn marked<R>(start: impl FnOnce(&mut dyn FnMut(&'static str)) -> R) -> R {
    let marks = unsafe { &mut *MARKS.0.get() };
    let mut count = 0;
    marks[1] = crate::now();
    let started = start(&mut |_| {
        count += 1;
        if let Some(at) = marks.get_mut(1 + count) {
            *at = crate::now();
        }
    });
    marks[0] = count as f64;
    started
}

/// The last lily_start's or lsb_restore's parts, as f64s: how many parts
/// ended, lily_now when it began, then lily_now as each part ended, in the
/// order of `START_MARKS` (lsb_restore's are its first six).
#[unsafe(no_mangle)]
pub extern "C" fn lsb_marks() -> *const f64 {
    MARKS.0.get() as *const f64
}

// --- the engine's parser.

static PARSED: Global<(Vec<u8>, [u32; 2])> = Global(UnsafeCell::new((Vec::new(), [0; 2])));
static NOTE_NAMES: Global<Option<lily_engine_parser::NoteNames>> = Global(UnsafeCell::new(None));

/// Reads the pitch names of the `len` bytes at `text`, a
/// `define-note-names.scm`, for the files `lsp_parse` parses next.
/// Returns 0, or 1 when the text holds no tables, which leaves none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsp_note_names(text: *const u8, len: usize) -> u32 {
    let names = lily_engine_parser::NoteNames::read(unsafe { bytes(text, len) }).ok();
    let read = u32::from(names.is_none());
    unsafe { *NOTE_NAMES.0.get() = names };
    read
}

/// Parses the `len` bytes at `source` as the file named by the UTF-8 at
/// `name`, with the pitch names `lsp_note_names` read (none before it),
/// in a module of its own that uses the bench interpreter's `(lily)`:
/// lsb_restore's, lsb_init's, or a fresh one's made here.
/// Returns a pointer to the address and length of the stderr
/// LilyPond would print for it, valid until the next call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lsp_parse(
    name: *const u8,
    name_len: usize,
    source: *const u8,
    len: usize,
) -> *const u32 {
    let interpreter = unsafe { &mut *BENCH.0.get() }.get_or_insert_with(Bench::new);
    let parse = lily_engine_parser::parse(
        unsafe { text(name, name_len) },
        unsafe { bytes(source, len) }.to_vec(),
        unsafe { &*NOTE_NAMES.0.get() }
            .as_ref()
            .unwrap_or(&lily_engine_parser::NoteNames::default()),
        interpreter.vm(),
    );
    let parsed = unsafe { &mut *PARSED.0.get() };
    parsed.0 = parse.stderr();
    parsed.1 = [parsed.0.as_ptr() as u32, parsed.0.len() as u32];
    parsed.1.as_ptr()
}
