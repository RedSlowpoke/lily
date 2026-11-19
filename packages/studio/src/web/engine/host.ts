// The page's side of the engine (chapter 08 of docs/research/wasm-engine):
// one dedicated worker, engine.worker.js beside the page, and compiles that a
// newer one supersedes. Files the editors do not have are read with
// `readFile`; on the web, from the user's worker on the server (D55). The
// engine is given bytes (D102): an editor's text as UTF-8, a file as read.
// A compile's outcome holds its pages; `perform` asks for its music after
// them (M7, D264). `pdf` is a compile that also gives the score's PDFs, which
// the engine writes for Export PDF (W7, D271).
import type { CompileOutcome } from '../../ipc'
import type { EngineAbout, EnginePdf, FromEngine, PageUpdate, Performance, ToEngine } from './protocol'

const utf8 = new TextEncoder()

/** The worker, as far as the host uses it: a real Worker, or a test's fake. */
export interface EngineWorker {
  onmessage: ((event: MessageEvent<FromEngine>) => void) | null
  /** A worker whose script could not be loaded, or that threw outside a compile: the engine is gone (D171). */
  onerror?: ((event: ErrorEvent) => void) | null
  postMessage(message: ToEngine): void
  terminate(): void
}

/** What the host asks of the browser's storage (08 §5.4): `navigator.storage`, or a test's fake. */
export type EngineStorage = Pick<StorageManager, 'persist' | 'persisted'>

export class EngineHost {
  private revision = 0
  /** Whether the browser was asked to keep the engine's cache (D170): once, after the engine's first compile. */
  private askedToPersist = false
  /** Compiles not yet answered, by revision; superseded ones resolve with undefined. */
  private readonly pending = new Map<number, (outcome: CompileOutcome | undefined, pdf?: EnginePdf) => void>()
  /** Performances not yet answered, by revision (D264). */
  private readonly performing = new Map<number, (performance: Performance | undefined) => void>()
  private broken?: string
  private fail: (message: string) => void = () => {}
  /** Resolves with the milliseconds the engine took to start; rejects when it cannot. */
  readonly ready: Promise<number>
  /** Once started: the engine's source and licences, which the page links (D56, D171). */
  about?: EngineAbout

  constructor(
    private readonly worker: EngineWorker,
    /** A file's bytes as read, undecoded; null when there is none. */
    private readonly readFile: (path: string) => Promise<Uint8Array | null>,
    /** What changed of a page on screen, or the page shown whole, before its compile's outcome (W8, I7). */
    private readonly onCells?: (cells: PageUpdate) => void,
    private readonly storage: EngineStorage | undefined = globalThis.navigator?.storage,
  ) {
    let started: (ms: number) => void = () => {}
    let failed: (error: Error) => void = () => {}
    this.ready = new Promise((resolve, reject) => {
      started = resolve
      failed = reject
    })
    // Nothing need wait for `ready`: a failure is also each compile's.
    this.ready.catch(() => {})
    this.fail = (message) => {
      this.broken ??= message
      failed(new Error(message))
      this.pending.forEach((resolve) => resolve(undefined))
      this.pending.clear()
      this.performing.forEach((resolve) => resolve(undefined))
      this.performing.clear()
    }
    worker.onerror = () => this.fail('The engine’s worker stopped.')
    worker.onmessage = ({ data }) => {
      switch (data.type) {
        case 'ready':
          this.about = data.about
          return started(data.ms)
        case 'needs-file':
          return void this.answer(data.path)
        case 'cells':
        case 'page':
          if (this.pending.has(data.revision)) this.onCells?.(data)
          return
        case 'finished':
        case 'superseded':
          if (data.type === 'finished') this.persist()
          this.pending.get(data.revision)?.(data.type === 'finished' ? data.outcome : undefined, data.type === 'finished' ? data.pdf : undefined)
          this.pending.delete(data.revision)
          return
        case 'performed':
          this.performing.get(data.revision)?.(data.performance)
          this.performing.delete(data.revision)
          return
        case 'failed':
          return this.fail(data.message)
      }
    }
  }

  /** Starts engine.worker.js beside the page; with `debug`, the debug core (08 §1.3, D170). */
  static start(readFile: (path: string) => Promise<Uint8Array | null>, onCells?: (cells: PageUpdate) => void, debug = false): EngineHost {
    return new EngineHost(new Worker(new URL(debug ? 'engine.worker.js?debug' : 'engine.worker.js', location.href)), readFile, onCells)
  }

  /** True once the engine could not start or stopped for good: the server's LilyPond compiles instead. */
  get failed(): boolean {
    return this.broken !== undefined
  }

  /**
   * Compiles `rootFile` with `files`: an editor's text, encoded as UTF-8, a
   * file's bytes as read, or null for a file that is gone (D172). Undefined
   * when a newer compile superseded it, or the engine failed.
   */
  compile(rootFile: string, files: Record<string, string | Uint8Array | null>, visiblePage = 1): Promise<CompileOutcome | undefined> {
    return this.request(rootFile, files, visiblePage, false).then((done) => done?.outcome)
  }

  /**
   * A compile as `compile`'s that also gives the score's PDFs, one a book (W7,
   * D271), for Export PDF; its outcome is the newest compile's like any other.
   * Undefined when a newer compile superseded it, or the engine failed.
   */
  pdf(rootFile: string, files: Record<string, string | Uint8Array | null>, visiblePage = 1): Promise<{ outcome: CompileOutcome; pdf: EnginePdf } | undefined> {
    return this.request(rootFile, files, visiblePage, true).then((done) => done && { outcome: done.outcome, pdf: done.pdf ?? { files: [], warnings: [] } })
  }

  private request(
    rootFile: string,
    files: Record<string, string | Uint8Array | null>,
    visiblePage: number,
    pdf: boolean,
  ): Promise<{ outcome: CompileOutcome; pdf?: EnginePdf } | undefined> {
    if (this.broken !== undefined) return Promise.resolve(undefined)
    // The worker runs only the newest request it has; the ones in between are never answered.
    this.pending.forEach((resolve) => resolve(undefined))
    this.pending.clear()
    // A newer compile stops a performance (D264).
    this.performing.forEach((resolve) => resolve(undefined))
    this.performing.clear()
    const revision = ++this.revision
    const outcome = new Promise<{ outcome: CompileOutcome; pdf?: EnginePdf } | undefined>((resolve) =>
      this.pending.set(revision, (outcome, pdf) => resolve(outcome && { outcome, pdf })),
    )
    const bytes = Object.fromEntries(Object.entries(files).map(([path, file]) => [path, typeof file === 'string' ? utf8.encode(file) : file]))
    this.worker.postMessage({ type: 'compile', request: { revision, rootFile, files: bytes, visiblePage, ...(pdf ? { pdf } : {}) } })
    return outcome
  }

  /**
   * The performance of the newest compile, after its page (M7, D264): its MIDI
   * file and map. Undefined when a newer compile stopped it, that compile has
   * no outcome yet, or the engine failed. Asked twice, it is made once.
   */
  perform(): Promise<Performance | undefined> {
    if (this.broken !== undefined || this.revision === 0) return Promise.resolve(undefined)
    const revision = this.revision
    const asked = this.performing.get(revision)
    const performed = new Promise<Performance | undefined>((resolve) =>
      this.performing.set(revision, (performance) => {
        asked?.(performance)
        resolve(performance)
      }),
    )
    if (!asked) this.worker.postMessage({ type: 'perform', revision })
    return performed
  }

  dispose(): void {
    this.worker.terminate()
    this.pending.forEach((resolve) => resolve(undefined))
    this.pending.clear()
    this.performing.forEach((resolve) => resolve(undefined))
    this.performing.clear()
  }

  /**
   * Asks the browser to keep the engine's files (08 §5.4, D170): once the
   * engine has compiled a score, never on a visit that did not use it, and
   * not again once granted. A refusal changes nothing: the files are fetched
   * again after an eviction.
   */
  private persist(): void {
    if (this.askedToPersist || !this.storage) return
    this.askedToPersist = true
    const storage = this.storage
    void storage
      .persisted()
      .then((kept) => kept || storage.persist())
      .catch(() => {})
  }

  private async answer(path: string): Promise<void> {
    const bytes = await this.readFile(path).catch(() => null)
    this.worker.postMessage({ type: 'file', path, bytes })
  }
}

/**
 * The engine's own errors, which LilyPond would not give for the same score
 * (`crates/core/src/diag.rs`: `over_memory`, `unfetched`, and
 * `unfetched_outlines`, whose text is spaced otherwise than LilyPond's).
 */
const ENGINE_CANNOT = [
  /^this score needs more memory than the engine may use \(\d+ MiB\)$/,
  /^cannot find font '[^']*' \(its font pack \S+ could not be fetched\)$/,
  /^the outlines of font '[^']*' could not be fetched \(font pack \S+\)/,
]

/**
 * Why the engine could not do a compile whose outcome this is, which the
 * server should do instead (08 §7 item 5, D172): the compile did not run
 * (`state: 'error'`: it stopped, or a module of its code could not be
 * fetched), or it ended in an error of the engine's own. Undefined for a
 * compile the engine did, whatever its score's own errors.
 */
export function cannotDo(outcome: CompileOutcome): string | undefined {
  if (outcome.state === 'error') return outcome.message ?? 'the engine could not compile the score'
  return outcome.diagnostics.find((d) => ENGINE_CANNOT.some((pattern) => pattern.test(d.message)))?.message
}

/**
 * The page's outcome with its performance (D264): the music, and what the
 * `\midi` translations said after what the page's compile said, as LilyPond
 * says it of a score (the parse's is the page's alone).
 */
export function withPerformance(outcome: CompileOutcome, performed: Performance): CompileOutcome {
  const diagnostics = [...outcome.diagnostics, ...performed.diagnostics]
  const errorCount = diagnostics.filter((d) => d.severity === 'error').length
  const { performing: _, midiData: __, timing: ___, ...page } = outcome
  return {
    ...page,
    state: outcome.state === 'ok' && errorCount > 0 ? 'failed' : outcome.state,
    diagnostics,
    errorCount,
    warningCount: diagnostics.length - errorCount,
    midi: performed.midi,
    ...(performed.midiData ? { midiData: performed.midiData } : {}),
    ...(performed.timing ? { timing: performed.timing } : {}),
  }
}
