// The engine in its worker (chapter 08 of docs/research/wasm-engine): starts
// the core from its image, runs one compile at a time in steps, and between
// steps lets a newer request supersede it (B8: within 50 ms, without shared
// memory). What a compile lacks — a file of the page, a module — it fetches,
// then starts that compile again; the engine reuses what it already did.
// LilyPond's own files (`<datadir>/…`) are the manifest's, never the page's;
// a pack or part that cannot be fetched is given as none, so that the core
// says so in the score's diagnostics (I4, D172).
// The page the preview shows comes before the compile's end, as soon as the
// core has written it (OPEN I7, D269): by its cells, or whole.
// A compile's outcome holds its pages; its performance comes after, only when
// the page asks for it (`perform`), in steps a newer compile stops (M7, 07
// §3.8, D264): the MIDI files and the map, the map only with a MIDI file
// (D222), and what the performers said.
// A compile for Export PDF (`pdf`, W7, D271) also gives each book's display
// list, which the PDF module, fetched on the first export with the fonts the
// list names, writes as the book's PDF, in an instance of its own dropped
// after it (B4); its outcome is a compile's like any other.
// worker.ts gives it the cache and the worker's messages, the tests fakes.
import type { TimedLyric } from '@lily/common/types'
import type { CompileOutcome, PlaybackTiming } from '../../ipc'
import { modulesFor } from './modules'
import {
  type CompileRequest,
  DATADIR,
  type EngineExports,
  type EngineManifest,
  type EngineOutputs,
  type EnginePdf,
  type FromEngine,
  type LyDiagnostic,
  MAX_MEMORY,
  Need,
  PDF_MODULE,
  type PageCells,
  type PdfExports,
  type Performance,
  Step,
  type ToEngine,
} from './protocol'

/** How long one `lily_step` may work before the worker reads its messages: well inside B8's 50 ms. */
export const STEP_MS = 10

export interface Loader {
  /** A wasm file of the engine, compiled (by streaming, from the cache). */
  module(file: string): Promise<WebAssembly.Module>
  /** Any other file of the engine. */
  bytes(file: string): Promise<Uint8Array>
}

/** What a placeholder of a split module throws: the core called a function it does not have yet. */
class Missing extends Error {
  constructor(readonly module: string) {
    super(`needs ${module}`)
  }
}

const utf8 = new TextEncoder()
const text = new TextDecoder()

/** The core's `RESERVE` (D259): what it keeps free of `MAX_MEMORY`. */
const CORE_RESERVE = 40 * 2 ** 20

export class Driver {
  private core?: WebAssembly.Module
  private image?: Uint8Array
  private engine?: EngineExports
  /** Split modules, compiled, by name: instantiated again with every new core instance. */
  private readonly split = new Map<string, WebAssembly.Module>()
  /** Parts and packs, by name: put again into every new core instance. */
  private readonly data = new Map<string, Uint8Array>()
  /** LilyPond's own files fetched (`file` modules), by the name the engine reads them as: put again into every new instance. */
  private readonly datadir = new Map<string, Uint8Array>()
  private latest?: CompileRequest
  /** The newest compile that finished, which a `perform` may name, and its performance once made. */
  private done?: CompileRequest
  private performance?: Performance
  /** A performance the page asked for, not yet begun. */
  private wanted?: CompileRequest
  private running = false
  private readonly waiting = new Map<string, (bytes: Uint8Array | null | undefined) => void>()
  /** What the debug core said as it panicked (`env.lily_panic`), until the trap that follows is reported. */
  private panicked?: string

  constructor(
    private readonly manifest: EngineManifest,
    private readonly loader: Loader,
    private readonly post: (message: FromEngine) => void,
    /** Returns to the worker's event loop, so that messages are read. */
    private readonly pause: () => Promise<void>,
    /** Runs the manifest's debug core, unsplit, whose panics say why (08 §1.3, D170), if it has one. */
    private readonly debug = false,
  ) {}

  /** Fetches the core, its image and the modules preloaded with it, and starts it. */
  async start(): Promise<void> {
    const started = performance.now()
    const { image, preload } = this.manifest.core
    const wasm = this.debug && this.manifest.core.debug ? this.manifest.core.debug : this.manifest.core.wasm
    const [core, bytes] = await Promise.all([this.loader.module(wasm), this.loader.bytes(image)])
    this.core = core
    this.image = bytes
    await this.instantiate()
    await Promise.all(preload.map((name) => this.load(name)))
    const { source, licenses } = this.manifest
    this.post({ type: 'ready', ms: performance.now() - started, about: { source, licenses } })
  }

  receive(message: ToEngine): void {
    if (message.type === 'file') {
      this.waiting.get(message.path)?.(message.bytes)
      this.waiting.delete(message.path)
      return
    }
    if (message.type === 'perform') {
      const { revision } = message
      if (this.done?.revision !== revision || this.latest) return this.post({ type: 'performed', revision })
      if (this.performance) return this.post({ type: 'performed', revision, performance: this.performance })
      this.wanted = this.done
      if (!this.running) void this.run()
      return
    }
    // A performance not yet begun is of an older compile: none.
    if (this.wanted) this.post({ type: 'performed', revision: this.wanted.revision })
    this.wanted = undefined
    this.latest = message.request
    // A compile waiting for a file stops waiting: the newer request wins.
    for (const resolve of this.waiting.values()) resolve(undefined)
    this.waiting.clear()
    if (!this.running) void this.run()
  }

  private async run(): Promise<void> {
    this.running = true
    try {
      for (;;) {
        const request = this.latest
        if (request) {
          const started = performance.now()
          try {
            await this.compile(request)
          } catch (error) {
            // A module that could not be fetched (offline, never cached), or outputs that do not read:
            // the compile did not run, and the page compiles elsewhere (chapter 08 §7).
            this.finish(request, failure(request.rootFile, `The engine could not compile the score: ${messageOf(error)}`, started))
          }
          if (this.latest === request) this.latest = undefined
          continue
        }
        const wanted = this.wanted
        if (!wanted) break
        this.wanted = undefined
        await this.perform(wanted)
      }
    } finally {
      this.running = false
    }
  }

  private async compile(request: CompileRequest): Promise<void> {
    const started = performance.now()
    this.done = undefined
    this.performance = undefined
    // What the sources name is fetched before the first step, not on the engine's request.
    // Read as text for that alone: the engine is given their bytes (D102).
    const sources = Object.values(request.files)
      .flatMap((bytes) => (bytes ? [text.decode(bytes)] : []))
      .join('\n')
    // A trigger only fetches early: one that fails is asked for again by the engine, if it needs it.
    await Promise.all(modulesFor(sources, this.manifest).map((name) => this.load(name).catch(() => {})))
    let handed: string | undefined
    const ran = await this.steps(
      request,
      () => {
        // Started again after a stop, it hands its page over again.
        handed = undefined
        this.begin(request)
      },
      () => this.latest !== request,
      (page) => (handed = this.handOver(request, page)),
    )
    if (ran === undefined) return this.post({ type: 'superseded', revision: request.revision })
    if (typeof ran === 'string') return this.finish(request, failure(request.rootFile, ran, started))
    // The page handed over is the outcome's, byte for byte: its cells are not sent again.
    const shown = handed === undefined ? -1 : [...ran.keys()].filter((name) => name.endsWith('.svg')).indexOf(handed)
    for (const cells of cellsOf(request.revision, ran)) if (cells.page !== shown) this.post(cells)
    const outcome = outcomeOf(request.rootFile, ran, performance.now() - started)
    // A page has its performance to come, unless the compile did not get as far as one (D264).
    this.done = request
    const pdf = request.pdf ? await this.writePdf(ran) : undefined
    this.finish(request, { ...outcome, performing: true }, pdf)
  }

  /**
   * Each book's PDF from its display list (W7, D271): the PDF module, fetched
   * once, in an instance of its own dropped after this export with its memory
   * (B4), given each list and the OTFs of the text fonts it names. A font the
   * build does not have is none, and the writer leaves its text out with a
   * warning; one that cannot be fetched, or the module, is no PDF.
   */
  private async writePdf(outputs: EngineOutputs): Promise<EnginePdf> {
    const lists = [...outputs].filter(([name]) => name.endsWith('.pdf.list'))
    const module = this.manifest.modules[PDF_MODULE]
    if (lists.length === 0) return { files: [], warnings: [] }
    if (module?.kind !== 'wasm') return { files: [], warnings: [], message: 'The engine was built without its PDF module.' }
    try {
      const instance = await WebAssembly.instantiate(await this.loader.module(module.file), {})
      const writer = instance.exports as unknown as PdfExports
      const pdf: EnginePdf = { files: [], warnings: [] }
      for (const [name, list] of lists) {
        pdfInput(writer, list)
        if (writer.lily_pdf_fonts() !== 0) throw new Error(`${name} is not a display list`)
        const names = text.decode(pdfOutput(writer)).split('\n').filter(Boolean)
        const fonts = await Promise.all(names.map(async (font) => [font, await this.otf(font)] as const))
        const given = fonts.filter((font): font is readonly [string, Uint8Array] => font[1] !== null)
        pdfInput(writer, framed([list, ...given.flatMap(([font, otf]) => [utf8.encode(font), otf])], given.length))
        if (writer.lily_pdf_write() !== 0) throw new Error(`${name} is not a display list`)
        pdf.files.push({ name: name.slice(0, -'.list'.length), data: pdfOutput(writer) })
        const warnings = text.decode(new Uint8Array(writer.memory.buffer, writer.lily_pdf_warnings(), writer.lily_pdf_warnings_len()))
        pdf.warnings.push(...warnings.split('\n').filter(Boolean))
      }
      return pdf
    } catch (error) {
      return { files: [], warnings: [], message: `The engine could not write the PDF: ${messageOf(error)}` }
    }
  }

  /** The OTF of the text font `name` (its PostScript name), or null when the build has none. */
  private async otf(name: string): Promise<Uint8Array | null> {
    const module = this.manifest.modules[`${name}.otf`]
    return module?.kind === 'font' ? this.loader.bytes(module.file) : null
  }

  /**
   * The visible page the core handed over before the compile's end (D269),
   * sent to the page by its cells, or whole: its name.
   */
  private handOver(request: CompileRequest, page: EngineOutputs): string | undefined {
    const name = [...page.keys()].find((file) => file.endsWith('.svg'))
    if (name === undefined) return undefined
    const index = request.visiblePage - 1
    const [cells] = cellsOf(request.revision, page)
    if (cells) this.post({ ...cells, page: index })
    else this.post({ type: 'page', revision: request.revision, page: index, svg: text.decode(page.get(name)) })
    return name
  }

  /** The performance of `request`'s compile, after its page (D264); a newer compile stops it. */
  private async perform(request: CompileRequest): Promise<void> {
    const started = performance.now()
    let performed: Performance | undefined
    try {
      const ran = await this.steps(request, () => this.beginPerformance(request), () => this.latest !== undefined)
      if (typeof ran === 'string') performed = { midi: [], diagnostics: [], durationMs: performance.now() - started, message: ran }
      else if (ran) performed = performanceOf(ran, performance.now() - started)
    } catch (error) {
      performed = { midi: [], diagnostics: [], durationMs: performance.now() - started, message: `The engine could not perform the score: ${messageOf(error)}` }
    }
    if (performed && this.done === request) this.performance = performed
    this.post({ type: 'performed', revision: request.revision, ...(performed ? { performance: performed } : {}) })
  }

  /**
   * Runs what `begin` starts in steps to its end, answering what it asks for
   * and starting it again: its outputs; undefined when `superseded`; a message
   * when it did not run (the engine stopped, or the memory's cap). `handed`
   * takes a page the core hands over before the end (D269).
   */
  private async steps(
    request: CompileRequest,
    begin: () => void,
    superseded: () => boolean,
    handed?: (page: EngineOutputs) => void,
  ): Promise<EngineOutputs | string | undefined> {
    begin()
    for (;;) {
      if (superseded()) return undefined
      let step: number
      try {
        step = this.exports().lily_step(STEP_MS)
      } catch (error) {
        // A placeholder, or the engine stopped (`panic = "immediate-abort"` traps). Its state
        // cannot be trusted after either: a new instance starts from the image. The core stops
        // a compile near the cap with a diagnostic of its own (D259), so a trap up here is a
        // block that memory in pieces could not give, short of the cap itself.
        const full = this.exports().memory.buffer.byteLength > MAX_MEMORY - CORE_RESERVE
        const panicked = this.panicked
        this.panicked = undefined
        await this.instantiate()
        // The new instance has done no compile: there is nothing of one to perform.
        this.done = undefined
        if (!(error instanceof Missing)) {
          return full
            ? `The score needs more memory than the engine may use (${MAX_MEMORY / 2 ** 20} MiB).`
            : `The engine stopped: ${panicked ?? messageOf(error)}`
        }
        await this.load(error.module)
        begin()
        continue
      }
      if (step === Step.Done) return this.outputs()
      if (step === Step.Needs) {
        const [kind, name] = this.needs()
        if (kind === Need.File && name.startsWith(`${DATADIR}/`)) {
          // LilyPond's own file: the manifest's, or none; the page has none of them.
          const module = `${name.slice(DATADIR.length + 1)}`
          const bytes = this.manifest.modules[module]?.kind === 'file' ? await this.loadFile(module) : null
          this.put(Need.File, name, bytes)
        } else if (kind === Need.File) {
          const answer = await this.ask(request, name)
          if (answer === undefined) continue
          this.put(Need.File, name, answer)
        } else if (this.manifest.modules[name]?.kind === 'split') {
          await this.load(name)
        } else {
          // A pack or a part that cannot be fetched is none: the core's diagnostic says so (I4).
          await this.load(name).catch(() => this.put(Need.Module, name, null))
        }
        begin()
        continue
      }
      if (handed && this.exports().lily_outputs() > 0) handed(this.outputs())
      await this.pause()
    }
  }

  private finish(request: CompileRequest, outcome: CompileOutcome, pdf?: EnginePdf): void {
    this.post({ type: 'finished', revision: request.revision, outcome, ...(request.pdf ? { pdf: pdf ?? { files: [], warnings: [] } } : {}) })
  }

  /**
   * Gives the engine the request's files, their bytes unchanged (D102), and
   * starts its compile, with each book's display list for Export PDF (D271).
   */
  private begin(request: CompileRequest): void {
    for (const [path, bytes] of Object.entries(request.files)) this.put(Need.File, path, bytes)
    const engine = this.exports()
    const [root, rootLen] = this.copy(utf8.encode(request.rootFile))
    engine.lily_pdf(request.pdf ? 1 : 0)
    engine.lily_compile(request.revision, root, rootLen, request.visiblePage)
    engine.lily_free(root, rootLen)
  }

  /**
   * Gives the engine the request's files again, which an instance made since
   * its compile lacks, and starts its performance (D264).
   */
  private beginPerformance(request: CompileRequest): void {
    for (const [path, bytes] of Object.entries(request.files)) this.put(Need.File, path, bytes)
    const engine = this.exports()
    const [root, rootLen] = this.copy(utf8.encode(request.rootFile))
    engine.lily_perform(request.revision, root, rootLen)
    engine.lily_free(root, rootLen)
  }

  /** Asks the page for a file; undefined when a newer request came first. */
  private ask(request: CompileRequest, path: string): Promise<Uint8Array | null | undefined> {
    return new Promise((resolve) => {
      this.waiting.set(path, resolve)
      this.post({ type: 'needs-file', revision: request.revision, path })
    })
  }

  /** Fetches a module of the manifest once; split modules join the core, the rest is put into it. */
  private async load(name: string): Promise<void> {
    if (this.split.has(name) || this.data.has(name) || this.datadir.has(`${DATADIR}/${name}`)) return
    const module = this.manifest.modules[name]
    if (!module) throw new Error(`The engine asked for ${name}, which it was not built with.`)
    if (module.kind === 'split') {
      // The debug core is not split: it has every function already.
      const core = this.core
      if (core && !WebAssembly.Module.imports(core).some((entry) => entry.module.startsWith('placeholder.'))) return
      const compiled = await this.loader.module(module.file)
      this.split.set(name, compiled)
      await WebAssembly.instantiate(compiled, { primary: this.exports() as unknown as WebAssembly.ModuleImports })
    } else if (module.kind === 'part' || module.kind === 'pack') {
      const bytes = await this.loader.bytes(module.file)
      this.data.set(name, bytes)
      this.put(Need.Module, name, bytes)
    } else if (module.kind === 'file') {
      const bytes = await this.loader.bytes(module.file)
      this.datadir.set(`${DATADIR}/${name}`, bytes)
      this.put(Need.File, `${DATADIR}/${name}`, bytes)
    } else {
      throw new Error(`${name} is a module of its own, not a part of a compile.`)
    }
  }

  /** A new instance of the core from its image, with every split module and part loaded so far. */
  private async instantiate(): Promise<void> {
    const core = this.core
    const image = this.image
    if (!core || !image) throw new Error('The engine has not been started.')
    // A placeholder of split module `m` is imported from `placeholder.m` (wasm-split's namespaces).
    // The debug core passes a panic's message before it traps (08 §1.3); the shipped core imports no lily_panic.
    const lily_panic = (at: number, len: number) => {
      this.panicked = text.decode(new Uint8Array(this.exports().memory.buffer, at, len))
    }
    const imports: WebAssembly.Imports = { env: { lily_now: () => performance.now(), lily_panic } }
    for (const entry of WebAssembly.Module.imports(core)) {
      if (!entry.module.startsWith('placeholder.')) continue
      const name = entry.module.slice('placeholder.'.length)
      imports[entry.module] ??= {}
      imports[entry.module][entry.name] = () => {
        throw new Missing(name)
      }
    }
    this.engine = (await WebAssembly.instantiate(core, imports)).exports as unknown as EngineExports
    for (const module of this.split.values()) await WebAssembly.instantiate(module, { primary: this.engine as unknown as WebAssembly.ModuleImports })
    const [at, len] = this.copy(image)
    const started = this.engine.lily_start(at, len)
    this.engine.lily_free(at, len)
    if (started !== 0) throw new Error(`The engine's image did not start (${started}).`)
    for (const [name, bytes] of this.data) this.put(Need.Module, name, bytes)
    for (const [name, bytes] of this.datadir) this.put(Need.File, name, bytes)
  }

  /** LilyPond's own file `ly/<name>`, fetched once; null when it cannot be fetched. */
  private async loadFile(module: string): Promise<Uint8Array | null> {
    try {
      await this.load(module)
    } catch {
      return null
    }
    return this.datadir.get(`${DATADIR}/${module}`) ?? null
  }

  private exports(): EngineExports {
    if (!this.engine) throw new Error('The engine has not been started.')
    return this.engine
  }

  /** Copies `bytes` into memory the engine allocated; the caller frees it. */
  private copy(bytes: Uint8Array): [number, number] {
    const engine = this.exports()
    const at = engine.lily_alloc(bytes.length)
    new Uint8Array(engine.memory.buffer, at, bytes.length).set(bytes)
    return [at, bytes.length]
  }

  /** Gives the engine a file or a module; `null`: there is no such file. */
  private put(kind: number, name: string, bytes: Uint8Array | null): void {
    const engine = this.exports()
    const [at, len] = this.copy(utf8.encode(name))
    const [data, dataLen] = bytes ? this.copy(bytes) : [0, 0]
    engine.lily_put(kind, at, len, data, dataLen)
    engine.lily_free(at, len)
    if (bytes) engine.lily_free(data, dataLen)
  }

  /** What `Step.Needs` asks for: its kind and name. */
  private needs(): [number, string] {
    const engine = this.exports()
    const out = engine.lily_alloc(12)
    engine.lily_needs(out)
    const [kind, at, len] = new Uint32Array(engine.memory.buffer, out, 3)
    const name = text.decode(new Uint8Array(engine.memory.buffer, at, len))
    engine.lily_free(out, 12)
    return [kind, name]
  }

  /** The output files, copied out of the engine's memory. */
  private outputs(): EngineOutputs {
    const engine = this.exports()
    const out = engine.lily_alloc(16)
    const files: EngineOutputs = new Map()
    for (let i = 0; i < engine.lily_outputs(); i++) {
      engine.lily_output(i, out)
      const [name, nameLen, data, dataLen] = new Uint32Array(engine.memory.buffer, out, 4)
      files.set(text.decode(new Uint8Array(engine.memory.buffer, name, nameLen)), new Uint8Array(engine.memory.buffer, data, dataLen).slice())
    }
    engine.lily_free(out, 16)
    return files
  }
}

/** Writes `bytes` as the PDF module's input. */
function pdfInput(writer: PdfExports, bytes: Uint8Array): void {
  const at = writer.lily_pdf_input(bytes.length)
  new Uint8Array(writer.memory.buffer, at, bytes.length).set(bytes)
}

/** The PDF module's output, copied out of its memory. */
function pdfOutput(writer: PdfExports): Uint8Array {
  return new Uint8Array(writer.memory.buffer, writer.lily_pdf_output(), writer.lily_pdf_output_len()).slice()
}

/**
 * The PDF module's input for `lily_pdf_write` (module.rs): the list, the
 * number of fonts, then each font's name and OTF; each piece a u32
 * little-endian length before its bytes.
 */
export function framed([list, ...fonts]: Uint8Array[], count: number): Uint8Array {
  // The count, then a length before each piece.
  const out = new Uint8Array(4 + [list, ...fonts].reduce((sum, piece) => sum + 4 + piece.length, 0))
  const view = new DataView(out.buffer)
  let at = 0
  const u32 = (value: number) => {
    view.setUint32(at, value, true)
    at += 4
  }
  const piece = (bytes: Uint8Array) => {
    u32(bytes.length)
    out.set(bytes, at)
    at += bytes.length
  }
  piece(list)
  u32(count)
  fonts.forEach(piece)
  return out
}

/** The pages of a compile's outputs that go by their cells: each `<page>.cells`, by `<page>.svg`'s place. */
export function cellsOf(revision: number, outputs: EngineOutputs): PageCells[] {
  const pages = [...outputs.keys()].filter((name) => name.endsWith('.svg'))
  return [...outputs].flatMap(([name, bytes]) => {
    if (!name.endsWith('.cells')) return []
    const file = text.decode(bytes)
    const line = file.indexOf('\n')
    const page = pages.indexOf(`${name.slice(0, -'.cells'.length)}.svg`)
    const cells = file.slice(0, line).split(' ').map((cell) => {
      const [digest, y] = cell.split(':')
      return { digest, y }
    })
    return [{ type: 'cells', revision, page, cells, markup: file.slice(line + 1) } as const]
  })
}

/**
 * A compile's outputs as the renderer takes a compile from the Rust side
 * (ipc.ts): diagnostics in the shape `parseStderr` gives them, pages in the
 * order the engine wrote them, the first MIDI file and its playback map's
 * entry 0, the map only with a MIDI file, as `read_playback` gives it (G1,
 * D222), with the words beside it. `stderr` is left to logs and to chapter
 * 00's comparator (C0).
 */
export function outcomeOf(rootFile: string, outputs: EngineOutputs, durationMs: number): CompileOutcome {
  const read = (name: string) => text.decode(outputs.get(name))
  const names = [...outputs.keys()]
  const pages = names.filter((name) => name.endsWith('.svg'))
  const { midi, midiData, timing, diagnostics } = performanceOf(outputs, durationMs)
  const errorCount = diagnostics.filter((d) => d.severity === 'error').length
  return {
    state: errorCount > 0 ? 'failed' : 'ok',
    rootFile,
    diagnostics,
    errorCount,
    warningCount: diagnostics.length - errorCount,
    pages,
    svg: pages.map(read),
    midi,
    ...(midiData ? { midiData } : {}),
    ...(timing ? { timing } : {}),
    durationMs,
  }
}

/**
 * A performance's outputs (D264), as `outcomeOf` reads a compile's: the MIDI
 * files, the first one's bytes, the map's entry 0 only with a MIDI file
 * (D222), and the diagnostics.
 */
export function performanceOf(outputs: EngineOutputs, durationMs: number): Performance {
  const names = [...outputs.keys()]
  const midi = names.filter((name) => /\.midi?$/.test(name))
  const read = (name: string) => text.decode(outputs.get(name))
  const diagnostics: LyDiagnostic[] = outputs.has('diagnostics.json') ? JSON.parse(read('diagnostics.json')) : []
  const map = names.find((name) => name.endsWith('.timing.json'))
  const timing = map && midi.length > 0 ? timingOf(read(map), read(map.replace(/timing\.json$/, 'words.json'))) : undefined
  return {
    midi,
    ...(midi.length > 0 ? { midiData: outputs.get(midi[0]) } : {}),
    ...(timing ? { timing } : {}),
    diagnostics,
    durationMs,
  }
}


/**
 * Entry 0 of the playback map `timing.ly` writes, one entry a performance, as
 * the Rust side reads it (`read_timing`, D103): none when that entry has no
 * events and bars or the map cannot be parsed, which only costs the playhead.
 * `words` is the file `words.ly` writes beside it, empty when the score did not
 * run that.
 */
function timingOf(json: string, words: string): PlaybackTiming | undefined {
  let map: unknown
  try {
    map = JSON.parse(json)
  } catch {
    return undefined
  }
  const entry: { events?: unknown; bars?: unknown; staves?: unknown } | undefined = Array.isArray(map) ? map[0] : undefined
  if (!Array.isArray(entry?.events) || !Array.isArray(entry.bars)) return undefined
  const { events, bars, staves } = entry
  const lyrics = wordsOf(words)
  return { events, bars, ...(Array.isArray(staves) && staves.length > 0 ? { staves } : {}), ...(lyrics.length > 0 ? { lyrics } : {}) }
}

/**
 * The syllables of entry 0 of the words, as the Rust side reads them
 * (`read_words`, D133): none when the file cannot be parsed or has no such
 * entry, which only costs the words.
 */
function wordsOf(json: string): TimedLyric[] {
  try {
    const words: unknown = JSON.parse(json)
    const entry: unknown = Array.isArray(words) ? words[0] : undefined
    return Array.isArray(entry) ? entry : []
  } catch {
    return []
  }
}

const messageOf = (error: unknown) => (error instanceof Error ? error.message : String(error))

/** A compile that did not run, as the renderer shows one: `message` says why. */
function failure(rootFile: string, message: string, started: number): CompileOutcome {
  return {
    state: 'error',
    rootFile,
    diagnostics: [],
    errorCount: 0,
    warningCount: 0,
    pages: [],
    svg: [],
    midi: [],
    durationMs: performance.now() - started,
    message,
  }
}
