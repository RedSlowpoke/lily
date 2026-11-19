//! The engine's core (docs/research/wasm-engine/08-integration.md, K2): the
//! [`Engine`] behind chapter 08 §4.2's C ABI (`abi.rs`), which the studio's
//! engine worker (`packages/studio/src/web/engine/`) runs as core.wasm, over
//! the engine's parser (`lily_engine_parser`) and Scheme
//! (`lily_engine_scheme`) with the music model's values in its `(lily)`
//! (`lily_engine_music`), and layout's after them (`lily_engine_layout`,
//! D104). Its only import is `env.lily_now`, the clock.
//!
//! K2's first part (MS2 step 10): `lily_start` restores the core's image
//! ([`write_image`]), `lily_put` keeps the source files it is given, and a
//! compile parses its root file, asking for the root when it has not been
//! given. Its second part (step 11, `compile.rs`): a compile writes
//! `stderr` and `diagnostics.json` from one list of diagnostics
//! (`diag.rs`, 08 §6), and what it made is collected before the next.
//! Since MS2 step 19 a compile asks the host the same way for each file
//! a score includes and for each file of a part a stub loads (D74), the
//! host's files and LilyPond's own, these named under
//! [`lily_engine_parser::source::DATADIR`]. Since step 23 a traced compile
//! ([`Engine::set_trace`]) translates its scores, since step 26 every
//! compile translates their `\midi` performances for the playback map, and
//! since step 40 their `\layout`s too, what the translations say going to
//! `stderr` (D99). Since MS3's part 1 step 5 it keeps the modules' data
//! `lily_put` gives (`MODULE`), and gives layout the font packs among them,
//! which the manifest preloads (W1's `emmentaler-20`); each compile ends
//! with `ly:reset-all-fonts`, as `lilypond-all` ends each file. Since MS3
//! step 35 a compile writes its books' pages, `<name>.svg` and
//! `<name>-<n>.svg`, which layout makes and `crates/svg` writes. Since MS3
//! step 50 (W5, D208) a compile that meets a font of the build's packs the
//! host has not given asks for its packs (`MODULE`), and a pack the host
//! cannot give is a diagnostic on the score. Since MS4 step 20 (I2) the
//! files of S6's `chords` and `fretted` parts are asked for as their part's
//! module ([`PART_MODULES`]), which the build writes ([`write_part`]) and the
//! manifest names, not as LilyPond's own files; since MS7 step 7 (D263)
//! those of every part.
//!
//! Beside `lily_*` the core exports chapter 03 Appendix B's bench ABI
//! (`lsb_*`) over the engine's Scheme ([`bench`]), so that MS1's benchmarks
//! and probes time the interpreter in the same core.wasm, with `lsb_restore`,
//! which starts it from `scheme.image`, and `lsb_collect`, which collects once
//! for the pause on that heap, and `lsb_pack` and `lsb_svg`, which give it
//! W1's packs and time the engine's SVG writer (`lily_engine_svg`) on a page
//! of the references (MS3 step 9); and `lsp_parse`, which runs the parser on a file
//! and gives the stderr LilyPond would print for it, with the pitch names
//! `lsp_note_names` reads from the installed `define-note-names.scm`'s text
//! and `\name` classified against the bench interpreter's `(lily)`.
//! `examples/bench.rs` writes both images natively.

pub mod bench;
mod compile;
pub mod diag;

#[cfg(target_arch = "wasm32")]
mod abi;

use lily_engine_music::{Change, Kept};
use lily_engine_parser::source::DATADIR;
use lily_engine_parser::{NoteNames, Reuse};
use lily_engine_scheme::{Exception, Extension, HostHandler, Hosted, Vm};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// Defines the `ly:` functions of the music model, of layout, of the
/// figured bass (D223, since MS4 step 16) and of the chord names (since
/// step 17) in `vm`'s `(lily)`, in that order, and the core's own
/// `%host-call` handlers ([`HOSTS`]): what a VM of the core's images has.
pub fn install(vm: &mut Vm) -> Result<(), Exception> {
    lily_engine_music::install(vm)?;
    lily_engine_layout::install(vm)?;
    lily_engine_figured_bass::install(vm)?;
    lily_engine_chord_names::install(vm)?;
    vm.add_host_handlers(&HOSTS);
    Ok(())
}

/// The core's `%host-call` handlers, over layout's of the same names: a
/// `\score` in a markup translated with the notation crates' classes too
/// (A47, MS4 step 17).
static HOSTS: [HostHandler; 1] = [HostHandler {
    what: "score-embedded-format",
    f: compile::host_score_embedded_format,
}];

/// What restoring an image of a VM [`install`] was given needs of the
/// core itself: its handlers, after the crates'.
static CORE: Extension = Extension {
    prims: &[],
    operators: None,
    hosts: &HOSTS,
    loaders: &[],
};

/// What restoring an image of a VM [`install`] was given needs, in the same
/// order.
pub static EXTENSIONS: &[&Extension] = &[
    &lily_engine_music::EXTENSION,
    &lily_engine_layout::EXTENSION,
    &lily_engine_figured_bass::EXTENSION,
    &lily_engine_chord_names::EXTENSION,
    &CORE,
];

/// S6's parts the host gives as modules (I2, MS4 step 20), each with its
/// files in `lily.scm`'s order, as the image's `%parts` names them: a file
/// of one is asked for as its part's module (`MODULE`), whose data
/// ([`write_part`]) holds them all. Since MS7 step 7 (D263) every part of
/// the image is one, so that a page needs none of LilyPond's `scm/` files;
/// any other file is asked for as `<datadir>/scm/lily/<file>` (`FILE`).
/// After a part's own files come those of the modules of their own its data
/// holds, which a `use-modules` loads, not the part: `diagrams`' `(lily
/// accreg)` (D273).
pub const PART_MODULES: [(&str, &[&str]); 7] = [
    (
        "chords",
        &[
            "chord-entry.scm",
            "chord-ignatzek-names.scm",
            "chord-name.scm",
        ],
    ),
    (
        "display",
        &["display-lily.scm", "define-music-display-methods.scm"],
    ),
    (
        "fretted",
        &[
            "fret-diagrams.scm",
            "tablature.scm",
            "predefined-fretboards.scm",
        ],
    ),
    (
        "diagrams",
        &[
            "harp-pedals.scm",
            "define-woodwind-diagrams.scm",
            "display-woodwind-diagrams.scm",
            "accreg.scm",
        ],
    ),
    ("partcombine", &["part-combiner.scm", "autochange.scm"]),
    ("modal", &["modal-transforms.scm"]),
    ("qr", &["qr-code.scm"]),
];

/// The first bytes of a part's module.
const PART_MAGIC: &[u8; 8] = b"LILYPART";

/// A part's module: [`PART_MAGIC`], then each file's name and text, each a
/// little-endian u32 length and its bytes.
pub fn write_part(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut data = PART_MAGIC.to_vec();
    for (name, text) in files {
        for bytes in [name.as_bytes(), text] {
            data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            data.extend_from_slice(bytes);
        }
    }
    data
}

/// The files of a part's module ([`write_part`]), or None if `data` is none.
fn read_part(data: &[u8]) -> Option<Vec<(String, Vec<u8>)>> {
    let mut rest = data.strip_prefix(PART_MAGIC)?;
    let mut take = || {
        let (len, after) = rest.split_first_chunk::<4>()?;
        let len = u32::from_le_bytes(*len) as usize;
        let (bytes, after) = (after.get(..len)?, &after[len..]);
        rest = after;
        Some(bytes)
    };
    let mut files = Vec::new();
    while let Some(name) = take() {
        files.push((String::from_utf8(name.to_vec()).ok()?, take()?.to_vec()));
    }
    rest.is_empty().then_some(files)
}

/// A part file's name as the core asks for it (`<datadir>/scm/lily/<file>`).
fn part_file(file: &str) -> String {
    format!("{DATADIR}/scm/lily/{file}")
}

/// What the host is asked for to give the compile `name`: a file of
/// [`PART_MODULES`] as its part's module, any other as itself.
fn need_of(name: &str) -> (u32, String) {
    let part = PART_MODULES
        .iter()
        .find(|(_, files)| files.iter().any(|file| part_file(file) == name));
    match part {
        Some((part, _)) => (MODULE, String::from(*part)),
        None => (FILE, name.into()),
    }
}

/// `lily_step`'s answers (protocol.ts `Step`).
pub const DONE: u32 = 0;
pub const MORE: u32 = 1;
pub const NEEDS: u32 = 2;
/// `lily_put`'s and `lily_needs`' kinds (protocol.ts `Need`).
pub const FILE: u32 = 0;
pub const MODULE: u32 = 1;

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn lily_now() -> f64;
}

/// Milliseconds, from the host's clock.
#[cfg(target_arch = "wasm32")]
pub fn now() -> f64 {
    unsafe { lily_now() }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Bytes of linear memory: its high-water mark, since it never shrinks (B4).
pub fn memory() -> usize {
    #[cfg(target_arch = "wasm32")]
    return core::arch::wasm32::memory_size(0) * 65536;
    #[cfg(not(target_arch = "wasm32"))]
    0
}

/// The memory `--max-memory` lets the core have (`scripts/build.mjs`, B4).
pub const MAX_MEMORY: usize = 256 << 20;

/// What a compile keeps free of [`MAX_MEMORY`] to go on (D259): more than the
/// most it allocates between two places that can stop it.
pub const RESERVE: usize = 40 << 20;

/// Every byte the core has allocated, which the memory's size does not say:
/// that is a high-water mark, and most of it may be free. So the room left is
/// counted, not looked for: a block of that size may be missing from memory
/// that is mostly free, and a small score must not be told otherwise.
#[cfg(target_arch = "wasm32")]
mod counted {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub(crate) static LIVE: AtomicUsize = AtomicUsize::new(0);

    /// What a block of `size` costs: dlmalloc's header, rounded up to 8, 16 at
    /// least.
    fn cost(size: usize) -> usize {
        ((size + 4 + 7) & !7).max(16)
    }

    struct Counted;

    unsafe impl GlobalAlloc for Counted {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            LIVE.fetch_add(cost(layout.size()), Ordering::Relaxed);
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            LIVE.fetch_add(cost(layout.size()), Ordering::Relaxed);
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn dealloc(&self, at: *mut u8, layout: Layout) {
            LIVE.fetch_sub(cost(layout.size()), Ordering::Relaxed);
            unsafe { System.dealloc(at, layout) }
        }

        unsafe fn realloc(&self, at: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            LIVE.fetch_add(cost(size), Ordering::Relaxed);
            LIVE.fetch_sub(cost(layout.size()), Ordering::Relaxed);
            unsafe { System.realloc(at, layout, size) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: Counted = Counted;
}

/// Whether a compile that is about to allocate `extra` bytes more has too
/// little left to go on. Asked where a compile can stop (`returns::short`),
/// since an allocation that fails cannot be come back from.
#[cfg(target_arch = "wasm32")]
pub(crate) fn short_of_memory(extra: usize) -> bool {
    use std::sync::atomic::Ordering;
    counted::LIVE.load(Ordering::Relaxed) + extra + RESERVE > MAX_MEMORY
}

/// The first bytes of the core's image.
const MAGIC: &[u8; 8] = b"LILYCORE";

/// The parts of [`Engine::start`], in order: the Scheme image's
/// ([`lily_engine_scheme::RESTORE_MARKS`]), then the pitch names read from
/// `(lily)`, then the engine around the VM.
pub const START_MARKS: [&str; 8] = [
    "code", "symbols", "arenas", "foreign", "globals", "vm", "names", "engine",
];

/// The core's image, which `lily_start` restores: [`MAGIC`], then the
/// interpreter's image (`Vm::write_image`), the same as `scheme.image`. Its
/// `(lily)` has loaded `define-note-names.scm`, whose tables the parser
/// takes its pitch names from (`NoteNames::of_vm`).
pub fn write_image(scheme: &[u8]) -> Vec<u8> {
    let mut image = Vec::with_capacity(MAGIC.len() + scheme.len());
    image.extend_from_slice(MAGIC);
    image.extend_from_slice(scheme);
    image
}

/// The source files by name; `None`: the host has no such file. Shared
/// with the interpreter's part source.
type Files = Rc<RefCell<BTreeMap<String, Option<Vec<u8>>>>>;

/// The modules' data the host gave (`MODULE`), by name.
type Modules = BTreeMap<String, Vec<u8>>;

/// The files a compile lacked at its last stop, which the host is to be
/// asked for, in the order asked: an include's name, or every file of a
/// part not given yet (MS2 step 30).
type Asked = Rc<RefCell<Vec<String>>>;

/// What the host gave of `name`; one it was not asked for yet is queued in
/// `asked`, once.
fn hosted(files: &Files, asked: &Asked, name: &str) -> Hosted<Vec<u8>> {
    match files.borrow().get(name) {
        Some(Some(text)) => Hosted::Text(text.clone()),
        Some(None) => Hosted::Missing,
        None => {
            let mut asked = asked.borrow_mut();
            if !asked.iter().any(|n| n == name) {
                asked.push(name.into());
            }
            Hosted::Ask
        }
    }
}

/// One engine per instance: an interpreter restored from the core's image,
/// the files the host gave it, and the compile it is running.
pub struct Engine {
    vm: Vm,
    names: NoteNames,
    files: Files,
    modules: Modules,
    asked: Asked,
    /// The packs the compile is to ask the host for, in order (W5), and
    /// those the host could not give it.
    packs: Vec<String>,
    unfetched: BTreeSet<String>,
    /// The root of the compile running, until it is done, and its revision.
    root: Option<String>,
    revision: u32,
    /// How many times it parsed its root.
    parses: u32,
    /// What its last parse left for the next (02 §5.7, MS6 step 3).
    reuse: Reuse,
    needs: Option<(u32, String)>,
    outputs: Vec<(String, Vec<u8>)>,
    /// Whether a compile translates its scores and writes their trace.
    trace: bool,
    /// Whether its performances write the playback map.
    playback: bool,
    /// Whether a compile translates its `\midi` definitions (M7, D264): off,
    /// a compile for a page leaves them to [`Engine::perform`].
    performs: bool,
    /// Whether the compile running is a performance ([`Engine::perform`]):
    /// its `\midi` translations only, and nothing kept of it.
    performing: bool,
    /// The definitions N8's kept translations were made with: none of them
    /// is resumed by a compile of other definitions.
    kept_defs: compile::Defs,
    /// What the last parse and its translations took.
    times: Times,
    /// Whether an error ended the last compile's run (D132, D220).
    ended: bool,
    /// N8 (04 §10.2, MS6 step 4): what the translations of the last
    /// compile of `n8_root` that ran to its end kept, in order, what each of
    /// the last compile's found against them, and, if it checks them
    /// ([`Engine::set_check_n8`]), what a whole run contradicted.
    n8: Vec<Kept>,
    n8_root: Option<String>,
    changes: Vec<Change>,
    check_n8: bool,
    n8_wrong: Vec<String>,
    /// N8's translations kept for the next compile's to resume (D241), the
    /// root and the text they were made from, and the snapshot each
    /// translation of the last compile was resumed from.
    resumables: Vec<Option<lily_engine_music::Resumable>>,
    resume_text: Option<(String, Vec<u8>)>,
    /// The epoch the first of the compiles stopped since then began in.
    stopped_at: Option<u32>,
    resumed_at: Vec<Option<usize>>,
    /// Whether each of them converged with it and ended as it did (D243).
    spliced: Vec<bool>,
    /// What each of them replayed (N8's replay, MS6 step 7c), in the same
    /// order.
    replayed: Vec<lily_engine_music::Replayed>,
    /// Layout's cells of each translation of the last compile (Y7, D248),
    /// held for the next, whether a compile looks in them, and what each
    /// `\layout` translation's cells did.
    cells: Vec<Option<lily_engine_scheme::Value>>,
    use_cells: bool,
    cell_reports: Vec<lily_engine_layout::cell::Report>,
    /// Whether compiles write their pages for a preview (W4, D253), what
    /// the writer kept of the last one and whose it is, and the page shown.
    preview: bool,
    /// Whether compiles give each book's PDF display list (W7, D270, D271).
    pdf: bool,
    markup: lily_engine_svg::Markup,
    markup_root: Option<String>,
    visible_page: u32,
    /// Whether compiles return between their steps (H3, D254), and the
    /// compile under way.
    returns: bool,
    /// Whether a compile in steps collects at each step's start.
    collect_every_step: bool,
    running: Option<Box<compile::Running>>,
    /// The compile's parse under way, between its steps (D267).
    parsing: Option<Box<compile::Parsing>>,
}

/// What the last step that parsed took (MS2 step 27), in milliseconds by
/// the host's clock: its parse, and its translations from frozen music to
/// the playback map's events, each score's `\midi` (and `\layout` if
/// traced). A compile that asked the host for files parsed once per
/// answer; only the last parse is here. Since MS3 step 36 the layout
/// apart (progress.md, *How layout per grob is measured*): what of the
/// translations laying out and breaking each `\layout`'s Paper_score took
/// (D130), the books' pages' stencils, the SVG writer, and the live grobs
/// laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Times {
    pub parse_ms: f64,
    /// All the translations', and how many ran.
    pub translate_ms: f64,
    pub translations: u32,
    /// The longest of them.
    pub longest_ms: f64,
    /// Of `translate_ms`, the Paper_scores processed and broken.
    pub lay_out_ms: f64,
    /// The books' pages made (`paper_book::output`), the writer excluded.
    pub stencils_ms: f64,
    /// The pages written as SVG.
    pub write_ms: f64,
    /// The Paper_scores' live grobs after their pages' stencils
    /// ([`lily_engine_layout::dump::live_grobs`]).
    pub live_grobs: u32,
    /// Of `translate_ms`, the `\midi` translations' (MS5 step 7): with
    /// `midi_ms`, README §4.4's performance and file after an edit.
    pub perform_ms: f64,
    /// The books' MIDI files written (`paper_book::output_midis`).
    pub midi_ms: f64,
    /// Bytes of linear memory after the translations, and after the
    /// books' pages made and written ([`memory`], 0 natively): a host
    /// splits a compile's growth of the memory by them (MS4 step 6, OPEN
    /// D7).
    pub translated_bytes: f64,
    pub paged_bytes: f64,
    /// 05 §7's steps (D251): the longest time between two of layout's
    /// checks, what ended it and the step before it, how many there were,
    /// and the milliseconds from the translations' start until the visible
    /// page was drawn.
    pub longest_step_ms: f64,
    pub longest_step: &'static str,
    pub longest_from: &'static str,
    /// The collections from the translations' start to the pages' end, and
    /// their milliseconds: a collection is a step of its own.
    pub collections: u32,
    pub collect_ms: f64,
    /// The longest step with the collections in it.
    pub longest_wall_ms: f64,
    pub steps: u32,
    pub visible_ms: f64,
}

impl Times {
    /// As `lsb_times` gives them, in this order.
    pub fn values(&self) -> [f64; 12] {
        [
            self.parse_ms,
            self.translate_ms,
            f64::from(self.translations),
            self.longest_ms,
            self.lay_out_ms,
            self.stencils_ms,
            self.write_ms,
            f64::from(self.live_grobs),
            self.perform_ms,
            self.midi_ms,
            self.translated_bytes,
            self.paged_bytes,
        ]
    }
}

impl Engine {
    /// Restores the core's image ([`write_image`]).
    pub fn start(image: &[u8]) -> Result<Engine, String> {
        Engine::start_marked(image, &mut |_| {})
    }

    /// [`Engine::start`], calling `mark` with each of [`START_MARKS`] as
    /// that part of the start ends (MS2 step 31).
    pub fn start_marked(
        image: &[u8],
        mark: &mut dyn FnMut(&'static str),
    ) -> Result<Engine, String> {
        let scheme = image
            .strip_prefix(MAGIC)
            .ok_or("not an image of the core")?;
        let mut vm = Vm::restore_marked(scheme, EXTENSIONS, mark).map_err(|e| e.to_string())?;
        vm.heap_mut().set_clock(now);
        lily_engine_music::returns::set_clock(now);
        let names = NoteNames::of_vm(&mut vm)?;
        mark("names");
        let files = Files::default();
        let asked = Asked::default();
        // A part's file, by its name in `scm/lily/`, as `lily.scm` loads it.
        let (part_files, part_asked) = (files.clone(), asked.clone());
        vm.set_part_source(Some(Box::new(move |name| {
            let name = part_file(name);
            match hosted(&part_files, &part_asked, &name) {
                Hosted::Text(text) => Hosted::Text(String::from_utf8_lossy(&text).into_owned()),
                Hosted::Missing => Hosted::Missing,
                Hosted::Ask => Hosted::Ask,
            }
        })));
        // A file a template's `\include` reads (D92, MS6 step 3).
        let (template_files, template_asked) = (files.clone(), asked.clone());
        vm.set_file_source(Some(Box::new(move |name| {
            hosted(&template_files, &template_asked, name)
        })));
        mark("engine");
        Ok(Engine {
            vm,
            names,
            files,
            modules: Modules::new(),
            asked,
            packs: Vec::new(),
            unfetched: BTreeSet::new(),
            root: None,
            revision: 0,
            parses: 0,
            reuse: Reuse::default(),
            needs: None,
            outputs: Vec::new(),
            trace: false,
            playback: true,
            performs: true,
            performing: false,
            kept_defs: compile::Defs::All,
            ended: false,
            times: Times::default(),
            n8: Vec::new(),
            n8_root: None,
            changes: Vec::new(),
            check_n8: false,
            n8_wrong: Vec::new(),
            resumables: Vec::new(),
            resume_text: None,
            stopped_at: None,
            resumed_at: Vec::new(),
            spliced: Vec::new(),
            replayed: Vec::new(),
            cells: Vec::new(),
            use_cells: true,
            cell_reports: Vec::new(),
            preview: false,
            pdf: false,
            markup: lily_engine_svg::Markup::default(),
            markup_root: None,
            visible_page: 0,
            returns: false,
            collect_every_step: false,
            running: None,
            parsing: None,
        })
    }

    /// A source file (`FILE`; `None`: there is no such file) or a module's
    /// data (`MODULE`). A file of LilyPond's own is named under
    /// [`DATADIR`]: `<datadir>/ly/…` and `<datadir>/scm/lily/…`. A module's
    /// data is kept by its name; one that is a font pack of W1's (the
    /// manifest's `emmentaler-20`, the parts of `emmentaler-brace`) is
    /// given to layout's fonts ([`lily_engine_layout::font::put_pack`])
    /// once: the same name put again with the same data is no new pack. A
    /// pack layout refuses (a part that continues no font) is kept and not
    /// read. A module of [`PART_MODULES`] gives its files as the host
    /// gives files (I2). A module put as none is one the host could not
    /// give (W5: a pack it could not fetch), which the compile running does
    /// not ask for again: a part's, its files as none, which no later
    /// compile of this instance asks for either, as a part's file put as
    /// none (`%load-part!` has marked the part loaded). The studio's driver
    /// puts no module as none: a module it cannot fetch ends the compile.
    pub fn put(&mut self, kind: u32, name: &str, data: Option<&[u8]>) {
        // A newer compile's: the one under way stops first (H3).
        self.stop_running();
        if kind == FILE {
            let old = self
                .files
                .borrow_mut()
                .insert(name.into(), data.map(<[u8]>::to_vec));
            // A file the host had and now has otherwise: the next parse
            // does nothing again from the last that read it (MS6 step 3).
            if old.is_some_and(|old| old.as_deref() != data) {
                self.reuse.changed(&mut self.vm, name);
            }
            return;
        }
        if kind != MODULE {
            return;
        }
        let part = PART_MODULES.iter().find(|(part, _)| *part == name);
        let Some(data) = data else {
            self.unfetched.insert(name.into());
            for file in part.map_or(&[][..], |(_, files)| files) {
                self.files.borrow_mut().insert(part_file(file), None);
            }
            return;
        };
        if let Some(files) = part.and_then(|_| read_part(data)) {
            let mut given = self.files.borrow_mut();
            for (file, text) in files {
                given.insert(part_file(&file), Some(text));
            }
        }
        self.unfetched.remove(name);
        if self.modules.get(name).is_some_and(|kept| kept == data) {
            return;
        }
        self.modules.insert(name.into(), data.to_vec());
        if lily_engine_layout::font::is_pack(data) {
            let _ = lily_engine_layout::font::put_pack(&mut self.vm, data);
        }
    }

    /// The data of the module `name`, as the host gave it.
    pub fn module(&self, name: &str) -> Option<&[u8]> {
        self.modules.get(name).map(Vec::as_slice)
    }

    pub fn needs(&self) -> Option<(u32, &str)> {
        self.needs.as_ref().map(|(k, n)| (*k, n.as_str()))
    }

    pub fn outputs(&self) -> &[(String, Vec<u8>)] {
        &self.outputs
    }

    /// Whether [`Engine::step`] keeps to its budget, returning [`MORE`]
    /// between timesteps, grobs, lines, systems and pages (H3, D254), as
    /// `lily_start` has it; off, a compile is one call. A traced compile is
    /// one call either way. A compile under way is stopped by the next
    /// [`Engine::put`] or [`Engine::compile`] (B8).
    pub fn set_returns(&mut self, on: bool) {
        self.returns = on;
    }

    /// A test's: a compile in steps collects at every step's start, which finds
    /// what is held between its steps and not rooted (B20, D261), and whatever
    /// depends on when collections come.
    pub fn set_collect_every_step(&mut self, on: bool) {
        self.collect_every_step = on;
    }

    /// What says the memory left is too little to go on (D259): a compile is
    /// stopped with a diagnostic where a step returns and where Scheme runs,
    /// before an allocation fails.
    pub fn set_room(&mut self, short: fn(usize) -> bool) {
        lily_engine_music::returns::set_room(short);
        self.vm.set_room(Some(short));
    }

    /// Whether a compile is under way between its steps.
    pub fn running(&self) -> bool {
        self.running.is_some() || self.parsing.is_some()
    }

    /// Whether a collection is under way between a compile's steps.
    pub fn collecting(&self) -> bool {
        self.vm.heap().collecting()
    }

    /// Whether a call of Scheme is under way between a compile's steps.
    pub fn calling(&self) -> bool {
        self.vm.calling()
    }

    /// What the compile under way goes on with next, for the steps' timings:
    /// `""` when there is none.
    pub fn stage(&self) -> &'static str {
        match (&self.running, &self.parsing) {
            (Some(r), _) => r.stage(),
            (None, Some(_)) => "parsing",
            (None, None) => "",
        }
    }

    /// Whether each compile writes its `\layout` translations' trace as
    /// the outputs `trace.tsv` and `trace.log` (`compile.rs`, MS2 step 23),
    /// their grobs as `grobs.tsv` (MS3 step 12), their laid-out grobs as
    /// `layout.tsv` (MS3 step 25), their columns' springs and rods as
    /// `columns.tsv` (MS3 step 31), their one line's columns' X offsets
    /// as `line.tsv` (MS3 step 32) and their shapes for C5 as `c5.tsv`
    /// (MS4 step 6).
    /// Not through the ABI. Since step 40 every compile translates its
    /// `\layout`s (D99).
    pub fn set_trace(&mut self, on: bool) {
        self.trace = on;
    }

    /// The breaks the compiles break their lines at (MS3 step 60), each a
    /// column's `when` as grobs.tsv writes it, until the engine chooses
    /// them (`lily_engine_layout::paper_score::give_breaks`); none for
    /// the one line. A test's, which gives LilyPond's. Not through the ABI.
    pub fn set_breaks(&mut self, whens: &[String]) {
        lily_engine_layout::paper_score::give_breaks(&mut self.vm, whens);
    }

    /// Whether a compile's `\midi` performances write the playback map,
    /// `<name>.timing.json` (MS2 step 26), as they do unless told not to:
    /// without it a score that includes `timing.ly` writes the map itself
    /// (G8). Not through the ABI.
    pub fn set_playback(&mut self, on: bool) {
        self.playback = on;
    }

    /// Whether a compile translates its `\midi` definitions, as it does
    /// unless told not to (M7, D264): off, it translates its `\layout`s and
    /// writes its pages, and no MIDI file, playback map or performer's
    /// message, which [`Engine::perform`] gives after it. The ABI's
    /// `lily_compile` has it off for a page (`visiblePage` > 0).
    pub fn set_performs(&mut self, on: bool) {
        self.performs = on;
    }

    /// What the last step that parsed took ([`Times`]).
    pub fn times(&self) -> Times {
        self.times
    }

    /// What each translation of the last compile found against the same
    /// translation of the compile of its root before it (N8, 04 §10.1,
    /// §10.5, §10.6), with the check on ([`Engine::set_check_n8`]; D258):
    /// none for a first compile.
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// What each translation of the last compile that ran to its end kept
    /// (N8): its partitions among it (04 §10.3, MS6 step 6).
    pub fn kept(&self) -> &[Kept] {
        &self.n8
    }

    /// The snapshot each translation of the last compile that ran to its
    /// end was resumed from (D241), in order: `None` for one run whole.
    pub fn resumed_at(&self) -> &[Option<usize>] {
        &self.resumed_at
    }

    /// Whether each of those resumed converged with the last compile's run
    /// and ended as it did (N8's splice, D243), in the same order.
    pub fn spliced(&self) -> &[bool] {
        &self.spliced
    }

    /// What each of those translations replayed (N8's replay, MS6 step 7c),
    /// in the same order.
    pub fn replayed(&self) -> &[lily_engine_music::Replayed] {
        &self.replayed
    }

    /// Whether a compile checks what its translations found against their
    /// whole runs (`lily_engine_music::verify`), and each translation's
    /// snapshots by restoring it at two and running it on (D241, a setting
    /// of the process), for the differential harness: [`Engine::n8_wrong`].
    pub fn set_check_n8(&mut self, on: bool) {
        self.check_n8 = on;
        lily_engine_music::set_check_resume(on);
    }

    /// What the last compile's whole runs contradicted of its
    /// [`Engine::changes`], each line naming its translation.
    pub fn n8_wrong(&self) -> &[String] {
        &self.n8_wrong
    }

    /// Whether compiles keep layout's cells and reuse them (Y7, D248): off,
    /// every compile is laid out whole.
    pub fn set_cells(&mut self, on: bool) {
        self.use_cells = on;
    }

    /// The page the host shows, 0 the first, which the compiles from here
    /// draw first (05 §7.2, D251).
    pub fn set_visible_page(&mut self, page: u32) {
        self.visible_page = page;
        lily_engine_layout::queue::set_visible_page(page);
    }

    /// Whether the compiles from here write their pages for a preview (W4,
    /// D253): `<page>.svg` in 06 §6.2's preview form, each line of a system
    /// a cell, and `<page>.cells` beside a page that can be sent by its
    /// changed cells (`lily_engine_svg::Markup::page`). Off, the file form.
    pub fn set_preview(&mut self, on: bool) {
        self.preview = on;
    }

    /// Whether a compile's outputs hold each book's PDF display list (W7,
    /// D270): `<book>.pdf.list` beside its pages, named after the book as
    /// LilyPond names its PDF, which `lily_engine_pdf::write` makes the PDF
    /// of: the ABI's `lily_pdf` (MS8 step 6, D271), pdf.wasm's on the web.
    pub fn set_pdf(&mut self, on: bool) {
        self.pdf = on;
    }

    /// What the writer did of each page of the last compile for a preview.
    pub fn page_reports(&self) -> &[lily_engine_svg::PageReport] {
        self.markup.reports()
    }

    /// The bytes of markup the writer keeps for the next compile (README
    /// §4.5).
    pub fn markup_bytes(&self) -> usize {
        self.markup.kept_bytes()
    }

    /// What each `\layout` translation's cells did in the last compile (Y7).
    pub fn cell_reports(&self) -> &[lily_engine_layout::cell::Report] {
        &self.cell_reports
    }

    /// Of the last parse's segments (02 §5.7, MS6 step 3): how many were
    /// done again from the parse before it, and how many were parsed.
    pub fn reused(&self) -> (usize, usize) {
        self.reuse.reused()
    }

    /// How many braced blocks the last parse took from the parse before it
    /// (02 §5.7's step 3).
    pub fn blocks_taken(&self) -> usize {
        self.reuse.blocks_taken()
    }

    /// How many times the compile running, or the last, parsed its root:
    /// once, and once more after each stop that asked the host for files.
    pub fn parses(&self) -> u32 {
        self.parses
    }

    /// Whether an error of a translation, of `Book::process` or of the
    /// pages ended the last compile's run, as it ends LilyPond's with 1
    /// (D132, D220), a `\book` block's among them, whose report names no
    /// expression.
    pub fn ended(&self) -> bool {
        self.ended
    }

    /// Collects now, and gives the heap's counts after it: what a compile
    /// left live (`tests/engine.rs`).
    pub fn collect(&mut self) -> lily_engine_scheme::Stats {
        self.vm.collect();
        self.vm.heap().stats()
    }
}
