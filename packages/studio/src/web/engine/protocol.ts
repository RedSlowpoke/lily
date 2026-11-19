// What the page, the engine's worker and the engine's wasm exchange (chapter 08
// of docs/research/wasm-engine). Types, and the numbers of the wasm ABI.
import type { LyDiagnostic } from '@lily/common/types'
import type { CompileOutcome, PlaybackTiming } from '../../ipc'

/**
 * `engine/manifest.json`, written by `esbuild.mjs --site` from what the
 * engine's build left in dist/engine/. Every file name carries a hash of its
 * bytes, so a name is never reused for other bytes and a cached file never
 * goes stale.
 */
export interface EngineManifest {
  version: 1
  /**
   * The core, fetched before the first compile, and the modules fetched with
   * it: the default font pack and the text metrics (chapter 06 §5.2).
   * `debug`: the same core unsplit, whose panics say why (08 §1.3, D170),
   * fetched instead of `wasm` only when the page asks for it.
   */
  core: { wasm: string; image: string; preload: string[]; debug?: string }
  /** What is fetched only when a score needs it, by name. */
  modules: Record<string, EngineModule>
  /** The engine's source (D56): the repository and the commit it was built from, `dirty` if the tree had changes. */
  source?: { repository: string; commit: string; dirty: boolean }
  /** The licences of what is served (D170), by what they cover, each a file beside the manifest whose name never changes. */
  licenses?: Record<string, string>
  /**
   * The faces of the text families the engine's pages name (`LilyPond Serif`, `LilyPond Sans Serif`,
   * `LilyPond Monospace`), as web fonts (E17, D263); `esbuild.mjs --site` writes their `@font-face` rules
   * into engine/fonts.css. Not the worker's.
   */
  webFonts?: { family: string; weight: number; style: 'normal' | 'italic'; file: string }[]
}

/**
 * `split`: functions `wasm-split` took out of the core, instantiated into the
 * core's memory and table; the core imports their placeholders from
 * `placeholder.<name>`. `wasm`: a module with its own memory, such as the
 * PDF writer, dropped after use. `part`: compiled Scheme for the core's heap.
 * `pack`: a font pack. `file`: one of LilyPond's own files, `ly/<name>`,
 * which the engine reads as `<datadir>/ly/<name>` (D263): the driver gives it
 * as a file, and never asks the page for one under `<datadir>`. `font`: a text
 * face's OTF, named `<PostScript name>.otf`, which the PDF module embeds
 * subsets of (W7, D271); fetched only for an export.
 */
export type EngineModuleKind = 'split' | 'wasm' | 'part' | 'pack' | 'file' | 'font'

/** The manifest's PDF writer (W7, D270, D271): a `wasm` module, fetched on the first export. */
export const PDF_MODULE = 'pdf'

/** Where the engine names LilyPond's own files: `<datadir>/ly/…` (the parser's `source::DATADIR`). */
export const DATADIR = '<datadir>'

export interface EngineModule {
  kind: EngineModuleKind
  file: string
}

/** What the page says of the engine it runs (D56, D171): its source, and the licences beside the manifest. */
export type EngineAbout = Pick<EngineManifest, 'source' | 'licenses'>

/** The page to the worker. */
export type ToEngine =
  | { type: 'compile'; request: CompileRequest }
  /** A file the engine asked for with `needs-file`, its bytes as read; null when there is none (D102). */
  | { type: 'file'; path: string; bytes: Uint8Array | null }
  /**
   * Performs the compile of `revision` after its page (M7, D264): its MIDI
   * file and playback map. Only the newest compile that finished has one; a
   * newer compile stops it.
   */
  | { type: 'perform'; revision: number }

export interface CompileRequest {
  /** Increases with every request; a newer one supersedes the running compile (B8). */
  revision: number
  /** Absolute path of the score, as the page names it. */
  rootFile: string
  /**
   * The files the page has, as bytes (D102): the open editors' texts, unsaved
   * or not, as UTF-8, and any other file as read; null for a file that is
   * gone, which the engine had (D172). Others are asked for.
   */
  files: Record<string, Uint8Array | null>
  /** The page the preview shows, 1-based: laid out first (chapter 05 §7.2). */
  visiblePage: number
  /**
   * Export PDF (W7, D271): the compile also gives each book's PDF, which its
   * `finished` holds. Its outcome is a compile's like any other.
   */
  pdf?: boolean
}

/**
 * The PDFs of a compile that asked for them (W7, D271): one a book, named as
 * LilyPond names it, which pdf.wasm wrote from the core's display list.
 */
export interface EnginePdf {
  files: { name: string; data: Uint8Array }[]
  /** What the writer left out (a run in a font it was not given, an image). */
  warnings: string[]
  /** It could not be written, and why: the PDF module or a font could not be fetched, or it refused the list. */
  message?: string
}

/** The worker to the page. */
export type FromEngine =
  /** Started; with the manifest's `source` and `licenses`, which the page links (D56, D171). */
  | { type: 'ready'; ms: number; about: EngineAbout }
  | { type: 'needs-file'; revision: number; path: string }
  | PageCells
  | PageWhole
  /**
   * The compile's outcome: its pages; with `performing`, its MIDI and map to come by `perform` (D264);
   * with `pdf`, for a request that asked for it, its PDFs (D271).
   */
  | { type: 'finished'; revision: number; outcome: CompileOutcome; pdf?: EnginePdf }
  | { type: 'superseded'; revision: number }
  /** What `perform` gave; none when a newer compile stopped it, or there was nothing to perform. */
  | { type: 'performed'; revision: number; performance?: Performance }
  /** The engine could not be started or crashed; the page falls back to the server's LilyPond. */
  | { type: 'failed'; message: string }

/**
 * What changed of a page the preview shows, before the `finished` that holds
 * the page whole (W8, chapter 06 §6.4, the engine's D232). A page with none of
 * its cells kept, a new page and an unchanged page have no such message.
 */
export interface PageCells {
  type: 'cells'
  revision: number
  /** The page's index in `outcome.svg`. */
  page: number
  /** Its cells, in order: each `data-lily-cell` and the height its group is at. */
  cells: { digest: string; y: string }[]
  /** The `<g data-lily-cell>` of each digest the page had not, in order. */
  markup: string
}

/**
 * The page the preview shows, whole, before its compile's end (OPEN I7, D269):
 * the core hands it over as soon as it is written, and it is sent so when it
 * does not go by its cells (a new page, one of another size, one with no cell
 * kept); else a `cells` message sends it, also before the compile's end. The
 * `finished` that follows holds the same page.
 */
export interface PageWhole {
  type: 'page'
  revision: number
  /** The page's index in `outcome.svg`: the request's `visiblePage` less one. */
  page: number
  svg: string
}

/** What the page is sent of a page on screen before its compile's outcome: its cells (W8), or itself whole (I7). */
export type PageUpdate = PageCells | PageWhole

/**
 * A performance after the page (M7, 07 §3.8, D264): what a compile with its
 * `\midi`s would have put in its outcome, by `outcomeOf`'s rules: the MIDI
 * files, the first's bytes, entry 0 of the map only with a MIDI file (D222),
 * and what its `\midi` translations said, which the page's outcome lacks.
 */
export interface Performance {
  midi: string[]
  midiData?: Uint8Array
  timing?: PlaybackTiming
  diagnostics: LyDiagnostic[]
  durationMs: number
  /** It did not run, and why: the engine stopped, or what it needed could not be fetched. */
  message?: string
}

/** The core is linked with `--max-memory=268435456`: B4's declared maximum, 256 MiB. */
export const MAX_MEMORY = 256 * 2 ** 20

/** What `lily_step` returns. */
export const Step = { Done: 0, More: 1, Needs: 2 } as const

/** What `lily_needs` names, and what `lily_put` gives. */
export const Need = { File: 0, Module: 1 } as const

/**
 * The core's exports: a C ABI over linear memory, without wasm-bindgen.
 * Strings are UTF-8, given as a pointer and a length the engine allocated.
 */
export interface EngineExports {
  memory: WebAssembly.Memory
  lily_alloc(len: number): number
  lily_free(ptr: number, len: number): void
  /** Restores the heap image; every compile after a crash starts again from here. 0 when it worked. */
  lily_start(image: number, len: number): number
  /**
   * A source file (`Need.File`) or a part or pack (`Need.Module`); the engine
   * copies the bytes. `data` 0 says there is no such file.
   */
  lily_put(kind: number, name: number, nameLen: number, data: number, dataLen: number): void
  /**
   * Starts compiling `root`; work already done for an earlier revision is reused (chapter 02 §5.7).
   * With a page visible (`visiblePage` > 0) its `\midi`s are left to `lily_perform` (D264).
   */
  lily_compile(revision: number, root: number, rootLen: number, visiblePage: number): void
  /**
   * Starts the performance of the compile of `root` at `revision` (M7, D264): its `\midi`
   * translations alone, whole; its outputs the MIDI files, the map, `stderr` and
   * `diagnostics.json`. `lily_step` runs it and a newer `lily_put` or `lily_compile` stops it.
   */
  lily_perform(revision: number, root: number, rootLen: number): void
  /**
   * Whether the compiles from here give each book's PDF display list (W7, D271):
   * `<book>.pdf.list` among their outputs, which the PDF module writes as the
   * book's PDF. 1 on; a new instance has it off.
   */
  lily_pdf(on: number): void
  /** Works for about `budgetMs`, then returns at a point where it can be superseded. */
  lily_step(budgetMs: number): number
  /** After `Step.Needs`: writes the kind, name pointer and name length at `out`, three u32. */
  lily_needs(out: number): void
  /**
   * After `Step.Done`: how many output files there are. After `Step.More`, none, or the
   * visible page a compile for a page has written, handed over before its end (OPEN I7,
   * D269): `<page>.svg`, and `<page>.cells` when it goes by them, the bytes the outputs at
   * the end hold, until the next call.
   */
  lily_outputs(): number
  /** Writes the name pointer and length, then the data pointer and length, of output `i` at `out`. */
  lily_output(i: number, out: number): void
}

/**
 * The PDF module's exports (crates/pdf/src/module.rs, D270): its own memory,
 * no imports. The host writes a display list into `lily_pdf_input`'s buffer
 * and asks `lily_pdf_fonts` for the fonts it names (the output, one a line),
 * then writes the list and those fonts, each a u32 little-endian length before
 * its bytes (`list`, a count, then each font's name and OTF), and calls
 * `lily_pdf_write`: the PDF is the output, the warnings one a line. Each
 * returns 0 when it worked.
 */
export interface PdfExports {
  memory: WebAssembly.Memory
  lily_pdf_input(len: number): number
  lily_pdf_fonts(): number
  lily_pdf_write(): number
  lily_pdf_output(): number
  lily_pdf_output_len(): number
  lily_pdf_warnings(): number
  lily_pdf_warnings_len(): number
}

/**
 * The output files of a compile, as LilyPond names them beside the score:
 * `<base>.svg` or `<base>-<n>.svg`, `<base>.midi`, `<base>.timing.json`, and
 * `stderr`, written in LilyPond's own format (D6) so that every parser of it
 * reads the engine's as well, and `diagnostics.json`, the same as
 * `LyDiagnostic[]`. With a page visible the pages are the preview's, and beside
 * one sent by its cells `<page>.cells`: `PageCells`' cells, `digest:y` separated
 * by spaces, on a line, then its markup. A compile that asked for its PDF has
 * `<book>.pdf.list` too, each book's display list (D271).
 */
export type EngineOutputs = Map<string, Uint8Array>

export type { LyDiagnostic }
