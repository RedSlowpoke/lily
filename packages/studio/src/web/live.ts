// Live preview in the web's page by the engine (D36; chapter 08 §7 item 4 of
// docs/research/wasm-engine; D171). While the engine runs, an editor's edit
// is compiled here, through EngineHost, and its outcome goes to the renderer
// as the socket's compile event would; the server's own live compiles are
// off (`set_live`), so that a score does not compile twice. Edits still go to
// the server, which keeps the unsaved texts as before: should the engine stop,
// its live compiles come back on and take them up. Saved compiles (D31),
// opening a score, Export PDF (D33) and Export MIDI (D54) stay the server's.
// As the engine takes over, the server forgets the unsaved texts first: its
// live switch going off would otherwise compile the saved score over the
// engine's page. Should the engine stop, they are given back with the switch.
// The server's other compiles then read no unsaved text either, so while
// there is one they give way to the engine's compile of the same score.
// A compile the engine fails or cannot do (`cannotDo`: it stopped, a file or
// a module it could not have, a score over its memory) goes to the server
// for that revision (08 §7 item 5, D172): the server is given the unsaved
// texts and its live switch, compiles them, and its outcome is shown; the
// engine's next compile takes live preview back. Without a connection the
// engine's outcome is shown, and the server is asked once it is back.
// The engine's outcome holds the pages; their music comes after (M7, 07 §3.8,
// D264), when it is wanted: once the page has been shown for `idleMs` with no
// newer edit, at once while the music plays, and before Export MIDI. It is
// shown as the same compile's outcome with the music and the performers'
// messages; a performance the engine cannot do sends the compile to the
// server as a compile does.
// Export PDF is the engine's too (W7, D271): a compile of the score as live
// preview has it that also gives its PDFs, shown as the newest compile; what
// the engine cannot export (it stopped, it could not do the compile or write
// the PDF) the server exports, as before.
import type { CompileOutcome } from '../ipc'
import { cannotDo, withPerformance } from './engine/host'
import type { EngineAbout, EnginePdf, PageUpdate, Performance } from './engine/protocol'

/** What Export PDF says of a score with errors, as the server's export says it (compile_service.rs). */
export const PDF_WITH_ERRORS = 'The score has errors, so there is no PDF to export. Fix them first.'

/** How long a page is shown with no newer edit before its music is performed (D264): a pause in the typing. */
export const PERFORM_IDLE_MS = 500

/** The engine as live preview uses it: EngineHost, or a test's fake. */
export interface LiveEngine {
  readonly ready: Promise<number>
  readonly failed: boolean
  compile(rootFile: string, files: Record<string, string | Uint8Array | null>, visiblePage?: number): Promise<CompileOutcome | undefined>
  /** The newest compile's music, after its page (D264); undefined when a newer compile stopped it. */
  perform(): Promise<Performance | undefined>
  /** A compile that also gives the score's PDFs (D271); undefined when a newer compile superseded it. */
  pdf(rootFile: string, files: Record<string, string | Uint8Array | null>, visiblePage?: number): Promise<{ outcome: CompileOutcome; pdf: EnginePdf } | undefined>
}

/** An Export PDF waiting for the engine's compile of its score (D271). */
interface PdfWanted {
  rootFile: string
  resolve(pdf: EnginePdf | undefined): void
  reject(error: Error): void
  /** Whether a newer edit has superseded its compile once already. */
  again: boolean
}

export interface LiveOptions {
  engine: LiveEngine
  /** Turns the server's own live compiles on or off (`set_live`). */
  serverLive(on: boolean): void
  /** Gives the server an unsaved text, or forgets it (`edited`). */
  serverEdited(file: string, text: string | null): void
  /**
   * A file's bytes as the server has them; null when there is none (D102).
   * Rejects when it cannot be read: the server refused it, or there is no
   * connection, which it must say at once rather than wait for one.
   */
  readFile(path: string): Promise<Uint8Array | null>
  /** Compiles a score on the server as saved (`show_score`), when there is no unsaved text to give it. */
  serverCompileSaved(rootFile: string): void
  /** Whether the server can be reached now. */
  online(): boolean
  /** An event for the renderer, as the socket brings them (bridge.ts' StudioEvent). */
  emit(event: unknown): void
  /** The page the preview shows, from 1 (chapter 05 §7.2). */
  visiblePage(): number
  /** How long a page is shown before its music is performed; `PERFORM_IDLE_MS` unless a test says. */
  idleMs?: number
}

export function base64(data: Uint8Array): string {
  let binary = ''
  for (let i = 0; i < data.length; i += 0x8000) binary += String.fromCharCode(...data.subarray(i, i + 0x8000))
  return btoa(binary)
}

/** A compile event's outcome as the Rust side sends it: its MIDI as base64 (bridge.ts' WireOutcome). */
export function wireOutcome(outcome: CompileOutcome): Omit<CompileOutcome, 'midiData'> & { midiData?: string } {
  const { midiData, ...rest } = outcome
  return midiData === undefined ? rest : { ...rest, midiData: base64(midiData) }
}

/**
 * The page the preview shows: the first whose bottom is below the middle of
 * the pane, from 1. `pages` are the pages' boxes in order, as on screen.
 */
export function pageAt(pages: { top: number; bottom: number }[], middle: number): number {
  const at = pages.findIndex((page) => page.bottom >= middle)
  return at >= 0 ? at + 1 : Math.max(pages.length, 1)
}

/**
 * The engine's source and licences, as links for the Help menu (D56, D171).
 * `engineDir` is engine/ beside the page.
 */
export function aboutLinks(about: EngineAbout | undefined, engineDir: string): { label: string; url: string }[] {
  const links: { label: string; url: string }[] = []
  const source = about?.source
  if (source) links.push({ label: 'Engine Source Code', url: `${source.repository}/tree/${source.commit}` })
  const notice = about?.licenses?.notice
  if (notice) links.push({ label: 'Engine Licenses', url: new URL(notice, engineDir).href })
  return links
}

/**
 * The page's query that refuses the engine, its other parameters kept: Help's
 * *Compile on the Server* (D174). engine.js reads it (`engineMode`).
 */
export function refusedSearch(search: string): string {
  const query = new URLSearchParams(search)
  query.set('engine', '0')
  return `?${query}`
}

export class EngineLive {
  /** The editors' unsaved texts by file, as the server's `LiveTexts` keeps them. */
  private readonly texts = new Map<string, string>()
  /** Files the engine was given or has read: it keeps each until it is given another. */
  private readonly held = new Set<string>()
  /** Of those, the ones that may now differ from what the engine has: read again for the next compile. */
  private readonly stale = new Set<string>()
  /** Read again, for the next compile. */
  private readonly fresh = new Map<string, Uint8Array | null>()
  /** The user's switch (D36); on until the renderer says. */
  private live = true
  private running = false
  /** The score the preview shows, as `show_score` answered. */
  private shown?: string
  /** The score of the newest compile sent, which its cells are of. */
  private compiling?: string
  private scheduled?: string
  /** Compiles are prepared one at a time, in order; a newer edit replaces one not yet sent. */
  private chain: Promise<void> = Promise.resolve()
  private queued = false
  /** The score whose compile the server has, as the engine could not do it; with whether it was given the texts. */
  private turn?: { rootFile: string; texts: boolean }
  /** A score to give the server once it can be reached: the engine's newest compile could not be done. */
  private owed?: string
  /** The files the newest compile asked for that could not be read (not: that do not exist). */
  private unread = new Set<string>()
  /** The engine's outcome shown whose music is still to come (D264), its performance asked for, and the wait for a pause. */
  private awaiting?: CompileOutcome
  private performing?: Promise<void>
  private idle?: ReturnType<typeof setTimeout>
  /** Whether the music plays: its performance is not waited for. */
  private isPlaying = false
  /** Export PDF's, each until a compile of its score gives its PDF (D271). */
  private pdfWanted: PdfWanted[] = []

  constructor(private readonly options: LiveOptions) {
    options.engine.ready.then(
      () => this.start(),
      () => {},
    )
  }

  /** What the server's live switch should be: the user's, unless the engine has the live compiles. */
  get serverOn(): boolean {
    return this.live && !this.running
  }

  /** The score the preview shows, or none: an include's edits compile it. */
  showScore(rootFile: string | null | undefined): void {
    this.shown = rootFile ?? undefined
  }

  /** An editor's text, unsaved, or null when saved or reloaded (D36), as `edited` says it to the server. */
  edited(file: string, text: string | null): void {
    if (text === null) {
      // A save compiles on the server (D31); the engine reads the file as saved next time.
      this.texts.delete(file)
      if (this.held.has(file)) this.stale.add(file)
      return
    }
    if (this.texts.get(file) === text) return
    this.texts.set(file, text)
    this.stale.delete(file)
    if (this.running && this.live) this.schedule(this.rootOf(file))
  }

  /**
   * A compile event of the server's (a save, a score shown): passed on unless
   * the engine has live preview and there are unsaved texts, which the
   * server's compiles no longer read; then the engine compiles the score.
   */
  serverCompile(kind: 'started' | 'finished', rootFile: string): boolean {
    if (this.turn) {
      // The server's compile of what the engine could not do: shown, and the turn ends with it.
      if (kind === 'finished' && rootFile === this.turn.rootFile) this.endTurn()
      return true
    }
    if (!this.running || !this.live || this.texts.size === 0) return true
    if (kind === 'started') this.schedule(rootFile)
    return false
  }

  /** The connection is back (D55): what the engine holds is read again, and a compile it could not do goes to the server. */
  reconnected(): void {
    if (!this.running) return
    for (const file of this.held) if (!this.texts.has(file)) this.stale.add(file)
    const owed = this.owed
    if (owed !== undefined) this.toServer(owed)
  }

  /** Files another program changed (D34): those the engine holds are read again. */
  changed(files: string[]): void {
    for (const file of files) if (this.held.has(file) && !this.texts.has(file)) this.stale.add(file)
  }

  /** The user's live switch (D36). */
  setLive(on: boolean): void {
    const was = this.live
    this.live = on
    if (!this.running) return this.options.serverLive(on)
    if (on === was || this.texts.size === 0) return
    // On: what the unsaved files belong to. Off: the score shown as saved, as the server does.
    if (!on) for (const file of this.texts.keys()) this.stale.add(file)
    this.schedule(on ? this.rootOf([...this.texts.keys()].at(-1) ?? '') : this.shown)
  }

  /**
   * A file the engine asks for, read from the server; it holds it from then
   * on. One that cannot be read is none to the engine, and its compile goes to
   * the server, which can read it (D172).
   */
  async read(path: string): Promise<Uint8Array | null> {
    this.held.add(path)
    const unread = this.unread
    return this.options.readFile(path).catch(() => {
      unread.add(path)
      // Read again for the next compile: it may be readable then.
      this.stale.add(path)
      return null
    })
  }

  /**
   * Resolves once the music of the newest compile of `rootFile` is the
   * renderer's (D264): its performance made now if it is still to come.
   */
  async music(rootFile: string): Promise<void> {
    if (this.awaiting?.rootFile === rootFile) await this.perform()
  }

  /**
   * Export PDF from the engine (W7, D271): the PDFs of a compile of `rootFile`
   * with the texts live preview gives it, which is shown as its newest
   * compile. Undefined when the engine cannot export it: it is not running, it
   * could not do the compile or write the PDF, or newer edits superseded it
   * twice; the server exports it instead. Rejects for a score with errors, as
   * the server's export does.
   */
  exportPdf(rootFile: string): Promise<EnginePdf | undefined> {
    if (!this.running) return Promise.resolve(undefined)
    return new Promise((resolve, reject) => {
      this.pdfWanted.push({ rootFile, resolve, reject, again: false })
      this.schedule(rootFile)
    })
  }

  /** Whether the music plays; while it does, a page's music is performed at once (D264). */
  playing(on: boolean): void {
    this.isPlaying = on
    if (on && this.awaiting) void this.perform()
  }

  /** The cells of a page of the newest compile, or the page whole, for the preview to take before its outcome (W8, I7). */
  cells(cells: PageUpdate): void {
    if (this.compiling !== undefined) this.options.emit({ type: 'cells', rootFile: this.compiling, cells })
  }

  private start(): void {
    if (this.options.engine.failed) return
    this.running = true
    for (const file of this.texts.keys()) this.options.serverEdited(file, null)
    this.options.serverLive(false)
    if (this.live && this.texts.size > 0) this.schedule(this.shown)
  }

  /** The engine stopped for good: the server's live compiles come back, and compile the unsaved texts. */
  private stop(): void {
    if (!this.running) return
    this.awaitNothing()
    this.endTurn()
    this.owed = undefined
    this.running = false
    // The server exports what was asked of the engine.
    for (const wanted of this.pdfWanted.splice(0)) wanted.resolve(undefined)
    for (const [file, text] of this.texts) this.options.serverEdited(file, text)
    this.options.serverLive(this.live)
  }

  /**
   * Gives the server the compile of `rootFile` the engine could not do (D172):
   * the unsaved texts and its live switch, which compiles what they belong to,
   * or with live preview off or nothing unsaved the score as saved. False when
   * the server cannot be reached; it is asked once it can.
   */
  private toServer(rootFile: string): boolean {
    if (!this.options.online()) {
      this.owed = rootFile
      return false
    }
    this.owed = undefined
    this.endTurn()
    const texts = this.live && this.texts.size > 0
    this.turn = { rootFile, texts }
    if (texts) {
      for (const [file, text] of this.texts) this.options.serverEdited(file, text)
      this.options.serverLive(true)
    } else {
      this.options.serverCompileSaved(rootFile)
    }
    return true
  }

  /** The engine has live preview again: the server forgets the texts, then its switch goes off, as at the start. */
  private endTurn(): void {
    const turn = this.turn
    this.turn = undefined
    if (!turn?.texts) return
    for (const file of this.texts.keys()) this.options.serverEdited(file, null)
    this.options.serverLive(false)
  }

  /** A `.ly` file is its own score; any other is compiled as part of the score shown. */
  private rootOf(file: string): string | undefined {
    return /\.ly$/i.test(file) ? file : this.shown
  }

  /** No music to wait for: a newer compile is coming, or the engine stopped. */
  private awaitNothing(): void {
    clearTimeout(this.idle)
    this.idle = undefined
    this.awaiting = undefined
  }

  /** The music of the outcome shown is wanted: now while it plays, else after a pause (D264). */
  private wantMusic(outcome: CompileOutcome): void {
    this.awaitNothing()
    if (!outcome.performing) return
    this.awaiting = outcome
    if (this.isPlaying) return void this.perform()
    this.idle = setTimeout(() => void this.perform(), this.options.idleMs ?? PERFORM_IDLE_MS)
  }

  /**
   * Performs the outcome awaiting its music, once: shown with it, or, if the
   * engine could not do it, the compile given to the server (D172).
   */
  private perform(): Promise<void> {
    clearTimeout(this.idle)
    this.idle = undefined
    const outcome = this.awaiting
    if (!outcome) return Promise.resolve()
    this.performing ??= this.options.engine.perform().then((performed) => {
      this.performing = undefined
      // A newer compile stopped it, or is shown: its own music comes.
      if (!performed || this.awaiting !== outcome) return
      this.awaiting = undefined
      const whole = withPerformance(outcome, performed)
      // Offline, the server is asked once it is back, and the page keeps the music it has.
      if ((performed.message ?? cannotDo(whole)) !== undefined) return void this.toServer(outcome.rootFile)
      this.options.emit({ type: 'compile', event: { kind: 'finished', outcome: wireOutcome(whole) } })
    })
    return this.performing
  }

  private schedule(rootFile: string | undefined): void {
    if (rootFile === undefined) return
    this.scheduled = rootFile
    if (this.queued) return
    this.queued = true
    this.chain = this.chain.then(() => {
      this.queued = false
      return this.send()
    })
  }

  private async send(): Promise<void> {
    const rootFile = this.scheduled
    this.scheduled = undefined
    if (rootFile === undefined || !this.running) return
    const stale = [...this.stale]
    this.stale.clear()
    // A file that cannot be read now (no connection) keeps what the engine has, and is read again later.
    const read = await Promise.all(
      stale.map((file) =>
        this.options.readFile(file).then(
          (bytes) => bytes,
          () => (this.stale.add(file), undefined),
        ),
      ),
    )
    stale.forEach((file, i) => read[i] !== undefined && this.fresh.set(file, read[i]))
    // A newer edit came while they were read: its compile, next in the chain, takes them.
    if (this.scheduled !== undefined) return
    const files: Record<string, string | Uint8Array | null> = {}
    // A file gone is none to the engine too (null).
    for (const [file, bytes] of this.fresh) files[file] = bytes
    this.fresh.clear()
    if (this.live) {
      for (const [file, text] of this.texts) {
        files[file] = text
        this.held.add(file)
      }
    }
    this.compiling = rootFile
    // Its music is this compile's to come.
    this.awaitNothing()
    // The engine has live preview again: a compile the server had, or owed it, is the engine's now.
    this.endTurn()
    this.owed = undefined
    const unread = (this.unread = new Set())
    // Export PDF's of this score go with this compile (D271).
    const wanted = this.pdfWanted.filter((pdf) => pdf.rootFile === rootFile)
    this.pdfWanted = this.pdfWanted.filter((pdf) => pdf.rootFile !== rootFile)
    this.options.emit({ type: 'compile', event: { kind: 'started', rootFile } })
    const compiled =
      wanted.length > 0
        ? this.options.engine.pdf(rootFile, files, this.options.visiblePage())
        : this.options.engine.compile(rootFile, files, this.options.visiblePage()).then((outcome) => outcome && { outcome, pdf: undefined })
    void compiled.then((done) => {
      if (!done) {
        if (this.options.engine.failed) this.stop()
        // Superseded by a newer edit: once more with the newest texts; the server's after that.
        for (const pdf of wanted) {
          if (this.running && !pdf.again) {
            this.pdfWanted.push({ ...pdf, again: true })
            this.schedule(pdf.rootFile)
          } else pdf.resolve(undefined)
        }
        return
      }
      const { outcome } = done
      // What the engine could not do is the server's, which shows its own outcome; offline, the engine's is shown.
      const cannot = cannotDo(outcome) ?? (unread.size > 0 ? `could not read ${[...unread].join(', ')}` : undefined)
      for (const pdf of wanted) {
        if (cannot === undefined && outcome.state === 'failed') pdf.reject(new Error(PDF_WITH_ERRORS))
        else pdf.resolve(cannot === undefined ? exported(done.pdf) : undefined)
      }
      if (cannot !== undefined && this.toServer(rootFile)) return
      this.options.emit({ type: 'compile', event: { kind: 'finished', outcome: wireOutcome(outcome) } })
      if (this.compiling === rootFile && !this.scheduled) this.wantMusic(outcome)
    })
  }
}

/** The PDFs the engine wrote, or undefined when it wrote none: the server's export is tried then. */
function exported(pdf: EnginePdf | undefined): EnginePdf | undefined {
  if (!pdf || pdf.message !== undefined || pdf.files.length === 0) {
    if (pdf?.message) console.warn(pdf.message)
    return undefined
  }
  if (pdf.warnings.length > 0) console.warn(`The engine's PDF: ${pdf.warnings.join('; ')}`)
  return pdf
}
