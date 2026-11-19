import * as assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { describe, test } from 'node:test'
import type { CompileOutcome } from '../src/ipc'
import { exportQuestion, leaveOutLabel } from '../src/web/dialogs'
import { engineMode } from '../src/web/engine/engine'
import { cannotDo, withPerformance } from '../src/web/engine/host'
import type { EnginePdf, PageCells, Performance } from '../src/web/engine/protocol'
import { aboutLinks, EngineLive, type LiveEngine, PDF_WITH_ERRORS, pageAt, refusedSearch, wireOutcome } from '../src/web/live'
import { keyCommand } from '../src/web/menubar'
import { SEPARATION_COMMANDS, SEPARATION_UNAVAILABLE, separation, separationStub } from '../src/web/separation'

// Runs under `node --test` from out/test/ (npm run test:unit in packages/studio/).
// The web's page (DECISIONS D55); the server's side is crates/server's tests.

const key = (code: string, keys: { meta?: boolean; ctrl?: boolean; alt?: boolean; shift?: boolean } = {}) => ({
  code,
  metaKey: keys.meta ?? false,
  ctrlKey: keys.ctrl ?? false,
  altKey: keys.alt ?? false,
  shiftKey: keys.shift ?? false,
})

describe('keyCommand', () => {
  test('the app menu’s shortcuts, with ⌘ on a Mac and Ctrl elsewhere', () => {
    assert.equal(keyCommand(key('KeyS', { meta: true }), true), 'save')
    assert.equal(keyCommand(key('KeyS', { meta: true, alt: true }), true), 'save-all')
    assert.equal(keyCommand(key('KeyO', { meta: true }), true), 'open-file')
    assert.equal(keyCommand(key('KeyO', { meta: true, shift: true }), true), 'open-folder')
    assert.equal(keyCommand(key('KeyS', { ctrl: true }), false), 'save')
    assert.equal(keyCommand(key('KeyB', { meta: true }), true), 'toggle-sidebar')
    assert.equal(keyCommand(key('Enter', { meta: true, alt: true }), true), 'preview-full-width')
    assert.equal(keyCommand(key('Enter', { ctrl: true, alt: true }), false), 'preview-full-width')
  })

  test('other keys, and the wrong modifier, are left to the page', () => {
    assert.equal(keyCommand(key('KeyS', { ctrl: true }), true), undefined)
    assert.equal(keyCommand(key('KeyS', { meta: true }), false), undefined)
    assert.equal(keyCommand(key('KeyS', { meta: true, shift: true }), true), undefined)
    assert.equal(keyCommand(key('KeyZ', { meta: true }), true), undefined)
    assert.equal(keyCommand(key('KeyB', { meta: true, shift: true }), true), undefined)
    assert.equal(keyCommand(key('Enter', { meta: true, shift: true }), true), undefined)
    assert.equal(keyCommand(key('KeyS'), true), undefined)
  })
})

describe('exportQuestion', () => {
  test('Export MIDI and Export for GarageBand ask only while parts are muted (D54, D162)', () => {
    assert.equal(exportQuestion('Export for GarageBand', '/scores/Song.ly', []), undefined)
    assert.deepEqual(exportQuestion('Export for GarageBand', '/scores/Song.ly', ['Violin']), {
      title: 'Export for GarageBand',
      text: 'Song.mid is saved in your downloads.',
      buttons: ['Export', 'Cancel'],
      checkbox: { label: 'Leave out the muted part Violin', checked: true },
    })
    assert.equal(exportQuestion('Export MIDI', '/scores/Song.ly', ['Alto', 'Tenor'])?.checkbox?.label, 'Leave out the muted parts Alto and Tenor')
  })

  test('the File menu offers both exports', () => {
    const menubar = readFileSync(path.join(__dirname, '../../src/web/menubar.ts'), 'utf8')
    assert.match(menubar, /'Export MIDI…', run: run\('export-midi'\)/)
    assert.match(menubar, /'Export for GarageBand…', run: run\('export-garageband'\)/)
  })
})

describe('leaveOutLabel', () => {
  test('as the app’s save panel says it (midi_export.rs)', () => {
    assert.equal(leaveOutLabel(['Tenor']), 'Leave out the muted part Tenor')
    assert.equal(leaveOutLabel(['Alto', 'Tenor']), 'Leave out the muted parts Alto and Tenor')
    assert.equal(leaveOutLabel(['Soprano', 'Alto', 'Tenor']), 'Leave out the muted parts Soprano, Alto and Tenor')
    assert.equal(leaveOutLabel(['S', 'A', 'T', 'B']), 'Leave out the 4 muted parts')
  })
})

describe('separation on the web (D112)', () => {
  const studioDir = path.resolve(__dirname, '..', '..')
  const read = (file: string) => readFileSync(path.join(studioDir, file), 'utf8')

  test('is unavailable, and says why', () => {
    assert.deepEqual(separation, { available: false, reason: 'Separating stems needs the Mac app.' })
  })

  test('every separation command rejects with the reason, as the app’s Rust side rejects', async () => {
    for (const command of SEPARATION_COMMANDS) {
      await assert.rejects(separationStub(command) ?? Promise.resolve(), (error) => error === SEPARATION_UNAVAILABLE, command)
    }
  })

  test('other commands go on to the worker', () => {
    for (const command of ['open_sample', 'read_file', 'chat_list', 'separation', 'separate_all']) {
      assert.equal(separationStub(command), undefined, command)
    }
  })

  test('the stub answers each command the app registers (src-tauri/src/lib.rs)', () => {
    const registered = [...read('src-tauri/src/lib.rs').matchAll(/^\s*separate::(\w+),$/gm)].map((match) => match[1])
    assert.ok(registered.length > 0)
    assert.deepEqual([...SEPARATION_COMMANDS].sort(), registered.sort())
  })

  test('the worker refuses the same commands, should a page ask it (crates/server)', () => {
    const desktopOnly = read('crates/server/src/worker/commands.rs').match(/const DESKTOP_ONLY[^=]*= &\[([^\]]*)\]/)?.[1] ?? ''
    for (const command of SEPARATION_COMMANDS) assert.ok(desktopOnly.includes(`"${command}"`), command)
  })
})

describe('live preview by the engine (D36, D171)', () => {
  /** How long a page waits for a pause before its music, here. */
  const IDLE = 15
  const outcome = (rootFile: string, midiData?: Uint8Array): CompileOutcome => ({
    state: 'ok',
    rootFile,
    diagnostics: [],
    errorCount: 0,
    warningCount: 0,
    pages: ['score.svg'],
    svg: ['<svg/>'],
    midi: midiData ? ['score.midi'] : [],
    ...(midiData ? { midiData } : {}),
    durationMs: 1,
  })
  const text = (bytes: string | Uint8Array | null) =>
    bytes === null ? 'gone' : typeof bytes === 'string' ? bytes : `bytes:${new TextDecoder().decode(bytes)}`

  /**
   * A page with live preview, a fake engine and a fake server: what each was
   * asked. `disk` holds the files there are; `refused`, those the server will
   * not give; `online`, whether there is a connection.
   */
  function page(options: { start?: 'ready' | 'refused'; disk?: Record<string, string>; refused?: string[] } = {}) {
    let started: (ms: number) => void = () => {}
    let refused: (error: Error) => void = () => {}
    const compiles: { rootFile: string; files: Record<string, string>; visiblePage: number; answer(outcome: CompileOutcome | undefined): void }[] = []
    const performs: ((performance: Performance | undefined) => void)[] = []
    /** Compiles for Export PDF (D271): `compiles`' entries too, answered with the PDF. */
    const pdfs: { rootFile: string; files: Record<string, string>; answer(done: { outcome: CompileOutcome; pdf: EnginePdf } | undefined): void }[] = []
    const engine: LiveEngine & { failed: boolean } = {
      ready: new Promise<number>((resolve, reject) => ((started = resolve), (refused = reject))),
      failed: false,
      compile: (rootFile, files, visiblePage = 1) =>
        new Promise((answer) =>
          compiles.push({ rootFile, files: Object.fromEntries(Object.entries(files).map(([file, bytes]) => [file, text(bytes)])), visiblePage, answer }),
        ),
      perform: () => new Promise((answer) => performs.push(answer)),
      pdf: (rootFile, files) =>
        new Promise((answer) => pdfs.push({ rootFile, files: Object.fromEntries(Object.entries(files).map(([file, bytes]) => [file, text(bytes)])), answer })),
    }
    engine.ready.catch(() => {})
    const server: (boolean | string)[] = []
    const reads: string[] = []
    const events: { type: string; event?: { kind: string; rootFile?: string; outcome?: { rootFile: string; midiData?: string } }; rootFile?: string }[] = []
    let visible = 1
    const disk = options.disk ?? {}
    const connection = { online: true }
    const live = new EngineLive({
      engine,
      serverLive: (on) => server.push(on),
      serverEdited: (file, text) => server.push(`${file}=${text}`),
      serverCompileSaved: (file) => server.push(`compile ${file}`),
      online: () => connection.online,
      readFile: async (path) => {
        reads.push(path)
        if (!connection.online || options.refused?.includes(path)) throw new Error(`${path} cannot be read`)
        return path in disk ? new TextEncoder().encode(disk[path]) : null
      },
      emit: (event) => events.push(event as (typeof events)[number]),
      visiblePage: () => visible,
      idleMs: IDLE,
    })
    const settle = () => new Promise((resolve) => setTimeout(resolve, 0))
    return {
      live,
      engine,
      compiles,
      performs,
      pdfs,
      server,
      reads,
      events,
      settle,
      connection,
      disk,
      show: (page: number) => (visible = page),
      start: async () => (started(3), await settle()),
      refuse: async () => (refused(new Error('no engine')), await settle()),
    }
  }

  test('the server compiles until the engine runs, then the engine, and the server’s live compiles go off', async () => {
    const { live, compiles, server, start, settle } = page()
    live.setLive(true)
    live.showScore('/s/score.ly')
    live.edited('/s/score.ly', '{ c }')
    await settle()
    assert.deepEqual(compiles, [], 'not before the engine has started')
    assert.deepEqual(server, [true])
    await start()
    // The server forgets the unsaved text first: its switch going off would compile the saved score.
    assert.deepEqual(server, [true, '/s/score.ly=null', false])
    assert.equal(live.serverOn, false)
    // What was typed before it started is compiled at once.
    assert.deepEqual(
      compiles.map(({ rootFile, files }) => [rootFile, files]),
      [['/s/score.ly', { '/s/score.ly': '{ c }' }]],
    )
  })

  test('an edit compiles its score with every unsaved text and the page shown; the outcome is a compile event', async () => {
    const { live, compiles, events, start, settle, show } = page()
    live.showScore('/s/score.ly')
    await start()
    live.edited('/s/part.ily', 'c4')
    live.edited('/s/score.ly', '\\include "part.ily"')
    show(3)
    await settle()
    // Two edits in one task: one compile.
    assert.equal(compiles.length, 1)
    assert.deepEqual(compiles[0].files, { '/s/part.ily': 'c4', '/s/score.ly': '\\include "part.ily"' })
    assert.equal(compiles[0].visiblePage, 3)
    compiles[0].answer(outcome('/s/score.ly', new Uint8Array([0x4d, 0x54])))
    await settle()
    assert.deepEqual(events, [
      { type: 'compile', event: { kind: 'started', rootFile: '/s/score.ly' } },
      { type: 'compile', event: { kind: 'finished', outcome: wireOutcome(outcome('/s/score.ly', new Uint8Array([0x4d, 0x54]))) } },
    ])
    assert.equal(events[1].event?.outcome?.midiData, 'TVQ=', 'MIDI as base64, as the socket sends it')
  })

  test('a superseded compile says nothing; the newer one does', async () => {
    const { live, compiles, events, start, settle } = page()
    await start()
    live.edited('/s/score.ly', '{ c }')
    await settle()
    live.edited('/s/score.ly', '{ d }')
    await settle()
    compiles[0].answer(undefined)
    compiles[1].answer(outcome('/s/score.ly'))
    await settle()
    assert.deepEqual(
      events.map(({ event }) => event?.kind),
      ['started', 'started', 'finished'],
    )
  })

  test('an include compiles the score shown; with none shown, nothing', async () => {
    const { live, compiles, start, settle } = page()
    await start()
    live.edited('/s/part.ily', 'c4')
    await settle()
    assert.equal(compiles.length, 0)
    live.showScore('/s/score.ly')
    live.edited('/s/part.ily', 'd4')
    await settle()
    assert.deepEqual(
      compiles.map(({ rootFile }) => rootFile),
      ['/s/score.ly'],
    )
  })

  test('a file the engine holds is read again after a save or another program’s change; others are not', async () => {
    const disk = { '/s/score.ly': 'saved', '/s/part.ily': 'changed' }
    const { live, compiles, reads, start, settle } = page({ disk })
    live.showScore('/s/score.ly')
    await start()
    assert.deepEqual(await live.read('/s/part.ily'), new TextEncoder().encode('changed'))
    live.edited('/s/score.ly', 'typed')
    await settle()
    live.edited('/s/score.ly', null)
    live.changed(['/s/part.ily', '/s/other.ily'])
    reads.length = 0
    live.edited('/s/next.ly', 'x')
    await settle()
    assert.deepEqual(reads.sort(), ['/s/part.ily', '/s/score.ly'])
    assert.deepEqual(compiles.at(-1)?.files, { '/s/score.ly': 'bytes:saved', '/s/part.ily': 'bytes:changed', '/s/next.ly': 'x' })
  })

  test('live preview off: no compile for an edit, and the score shown as saved; on again: the unsaved texts', async () => {
    const { live, compiles, server, start, settle } = page({ disk: { '/s/score.ly': 'saved' } })
    live.showScore('/s/score.ly')
    await start()
    live.edited('/s/score.ly', 'typed')
    await settle()
    live.setLive(false)
    await settle()
    live.edited('/s/score.ly', 'typed more')
    await settle()
    assert.deepEqual(
      compiles.map(({ files }) => files),
      [{ '/s/score.ly': 'typed' }, { '/s/score.ly': 'bytes:saved' }],
    )
    live.setLive(true)
    await settle()
    assert.deepEqual(compiles.at(-1)?.files, { '/s/score.ly': 'typed more' })
    assert.deepEqual(server, [false], 'the server’s switch stays off while the engine runs')
  })

  test('an engine that does not start leaves live preview to the server; one that stops gives it back', async () => {
    const refused = page()
    refused.live.setLive(false)
    await refused.refuse()
    refused.live.setLive(true)
    assert.deepEqual(refused.server, [false, true])
    assert.equal(refused.live.serverOn, true)

    const stopped = page()
    await stopped.start()
    stopped.live.edited('/s/score.ly', '{ c }')
    await stopped.settle()
    stopped.engine.failed = true
    stopped.compiles[0].answer(undefined)
    await stopped.settle()
    // The server is given the unsaved texts back, then compiles them.
    assert.deepEqual(stopped.server, [false, '/s/score.ly={ c }', true])
    assert.equal(stopped.live.serverOn, true)
    stopped.live.edited('/s/score.ly', '{ d }')
    await stopped.settle()
    assert.equal(stopped.compiles.length, 1, 'the server compiles it')
  })

  test('the server’s compiles give way to the engine’s while there are unsaved texts, which they do not read', async () => {
    const { live, compiles, start, settle } = page()
    assert.equal(live.serverCompile('started', '/s/score.ly'), true, 'before the engine runs')
    await start()
    assert.equal(live.serverCompile('finished', '/s/score.ly'), true, 'nothing unsaved: the server’s is the saved score')
    live.edited('/s/part.ily', 'c4')
    assert.equal(live.serverCompile('started', '/s/score.ly'), false)
    assert.equal(live.serverCompile('finished', '/s/score.ly'), false)
    await settle()
    assert.deepEqual(
      compiles.map(({ rootFile, files }) => [rootFile, files]),
      [['/s/score.ly', { '/s/part.ily': 'c4' }]],
      'the engine compiles that score instead, once',
    )
    live.setLive(false)
    assert.equal(live.serverCompile('started', '/s/score.ly'), true, 'live preview off: the saved files, as the server compiles them')
  })

  test('a page’s cells go to the preview with the score of the newest compile', async () => {
    const { live, events, start, settle } = page()
    const cells: PageCells = { type: 'cells', revision: 1, page: 0, cells: [], markup: '' }
    live.cells(cells)
    assert.deepEqual(events, [], 'none before a compile')
    await start()
    live.edited('/s/score.ly', '{ c }')
    await settle()
    live.cells(cells)
    assert.deepEqual(events.at(-1), { type: 'cells', rootFile: '/s/score.ly', cells })
  })

  test('the page shown: the first whose bottom is past the middle of the pane', () => {
    const pages = [
      { top: 0, bottom: 100 },
      { top: 110, bottom: 210 },
      { top: 220, bottom: 320 },
    ]
    assert.equal(pageAt(pages, 50), 1)
    assert.equal(pageAt(pages, 105), 2)
    assert.equal(pageAt(pages, 400), 3)
    assert.equal(pageAt([], 50), 1)
  })

  test('the engine’s source and licences, linked from Help (D56)', () => {
    assert.deepEqual(
      aboutLinks(
        { source: { repository: 'https://github.com/velet5/lily', commit: 'abc', dirty: false }, licenses: { notice: 'NOTICE.txt' } },
        'https://lily.example/engine/',
      ),
      [
        { label: 'Engine Source Code', url: 'https://github.com/velet5/lily/tree/abc' },
        { label: 'Engine Licenses', url: 'https://lily.example/engine/NOTICE.txt' },
      ],
    )
    assert.deepEqual(aboutLinks(undefined, 'https://lily.example/engine/'), [])
  })

  test('Compile on the Server: the page again with ?engine=0, its other parameters kept (D174)', () => {
    assert.equal(refusedSearch(''), '?engine=0')
    assert.equal(refusedSearch('?engine=1'), '?engine=0')
    assert.equal(refusedSearch('?x=a%20b&engine=debug'), '?x=a+b&engine=0')
    assert.equal(engineMode(refusedSearch('?x=2'), true), 'off')
  })
  const failedOutcome = (rootFile: string, message: string): CompileOutcome => ({
    ...outcome(rootFile),
    state: 'failed',
    pages: [],
    svg: [],
    errorCount: 1,
    diagnostics: [{ file: rootFile, line: 1, severity: 'error', message }],
  })
  const stopped = (rootFile: string): CompileOutcome => ({ ...outcome(rootFile), state: 'error', pages: [], svg: [], message: 'The engine stopped: unreachable' })

  const page_ = (rootFile: string): CompileOutcome => ({ ...outcome(rootFile), performing: true })
  const music: Performance = {
    midi: ['score.midi'],
    midiData: new Uint8Array([0x4d, 0x54]),
    timing: { events: [], bars: [{ at: 0, number: 1 }] },
    diagnostics: [{ file: '/s/score.ly', line: 2, column: 3, severity: 'warning', message: 'no such MIDI instrument: `x\'' }],
    durationMs: 4,
  }
  const pause = () => new Promise((resolve) => setTimeout(resolve, IDLE * 3))

  test('the music comes after the page, once it has been shown for a pause: the same compile’s outcome with it (M7, D264)', async () => {
    const { live, compiles, performs, events, start, settle } = page()
    live.showScore('/s/score.ly')
    await start()
    live.edited('/s/score.ly', '{ c }')
    await settle()
    compiles[0].answer(page_('/s/score.ly'))
    await settle()
    assert.equal(events.length, 2)
    assert.equal(performs.length, 0, 'not at once')
    await pause()
    assert.equal(performs.length, 1)
    performs[0](music)
    await settle()
    assert.deepEqual(events[2], { type: 'compile', event: { kind: 'finished', outcome: wireOutcome(withPerformance(page_('/s/score.ly'), music)) } })
    assert.equal(events[2].event?.outcome?.midiData, 'TVQ=')
    await pause()
    assert.equal(performs.length, 1, 'once')
  })

  test('an edit before the pause performs nothing; while the music plays, each page’s is performed at once (D264)', async () => {
    const { live, compiles, performs, events, start, settle } = page()
    live.showScore('/s/score.ly')
    await start()
    live.edited('/s/score.ly', '{ c }')
    await settle()
    compiles[0].answer(page_('/s/score.ly'))
    await settle()
    live.edited('/s/score.ly', '{ d }')
    await pause()
    assert.equal(performs.length, 0, 'the edit made the music stale, and nothing more')
    live.playing(true)
    compiles[1].answer(page_('/s/score.ly'))
    await settle()
    assert.equal(performs.length, 1, 'at once')
    // A newer compile stopped it: nothing shown; that compile's own music comes.
    live.edited('/s/score.ly', '{ e }')
    await settle()
    performs[0](undefined)
    await settle()
    assert.deepEqual(events.map(({ event }) => event?.kind), ['started', 'finished', 'started', 'finished', 'started'])
  })

  test('Export MIDI waits for the music, performed at once (D264)', async () => {
    const { live, compiles, performs, events, start, settle } = page()
    live.showScore('/s/score.ly')
    await start()
    live.edited('/s/score.ly', '{ c }')
    await settle()
    compiles[0].answer(page_('/s/score.ly'))
    await settle()
    let here = false
    const waited = live.music('/s/score.ly').then(() => (here = true))
    await settle()
    assert.equal(performs.length, 1)
    assert.equal(here, false)
    performs[0](music)
    await waited
    assert.equal(events.at(-1)?.event?.outcome?.midiData, 'TVQ=', 'the music is the renderer’s before it resolves')
    await live.music('/s/score.ly')
    await live.music('/s/other.ly')
    assert.equal(performs.length, 1, 'nothing to wait for')
  })

  test('a performance the engine cannot do sends the compile to the server (D172, D264)', async () => {
    const { live, compiles, performs, server, events, start, settle } = page()
    live.showScore('/s/score.ly')
    await start()
    live.edited('/s/score.ly', '{ c }')
    await settle()
    compiles[0].answer(page_('/s/score.ly'))
    await settle()
    server.length = 0
    void live.music('/s/score.ly')
    await settle()
    performs[0]({ midi: [], diagnostics: [], durationMs: 1, message: 'The engine stopped: unreachable' })
    await settle()
    assert.deepEqual(server, ['/s/score.ly={ c }', true], 'the texts, then the switch on')
    assert.equal(events.length, 2, 'the page stays, with the music it had')
  })

  test('a compile the engine cannot do goes to the server, given the unsaved texts and its switch; its outcome is the server’s (D172)', async () => {
    const { live, compiles, server, events, start, settle } = page()
    live.showScore('/s/score.ly')
    await start()
    live.edited('/s/part.ily', 'c4')
    live.edited('/s/score.ly', '\\include "part.ily"')
    await settle()
    server.length = 0
    compiles[0].answer(stopped('/s/score.ly'))
    await settle()
    assert.deepEqual(server, ['/s/part.ily=c4', '/s/score.ly=\\include "part.ily"', true], 'the texts, then the switch on: the server compiles them')
    assert.deepEqual(
      events.map(({ event }) => event?.kind),
      ['started'],
      'the engine’s outcome is not shown',
    )
    // The server's compile is shown; once it has finished, the server forgets the texts and its switch goes off.
    assert.equal(live.serverCompile('started', '/s/score.ly'), true)
    assert.equal(live.serverCompile('finished', '/s/score.ly'), true)
    assert.deepEqual(server.slice(3), ['/s/part.ily=null', '/s/score.ly=null', false])
    // The next edit is the engine's again; the server's compiles give way as before.
    live.edited('/s/score.ly', '{ d }')
    await settle()
    assert.equal(compiles.length, 2)
    assert.equal(live.serverCompile('started', '/s/score.ly'), false)
  })

  test('a newer edit takes live preview back from the server before its compile has finished', async () => {
    const { live, compiles, server, start, settle } = page()
    await start()
    live.edited('/s/score.ly', '{ c }')
    await settle()
    compiles[0].answer(stopped('/s/score.ly'))
    await settle()
    assert.equal(live.serverCompile('started', '/s/score.ly'), true)
    server.length = 0
    live.edited('/s/score.ly', '{ d }')
    await settle()
    assert.deepEqual(server, ['/s/score.ly=null', false], 'the server forgets the text before its switch goes off, so it compiles nothing')
    assert.equal(compiles.length, 2)
    assert.equal(live.serverCompile('finished', '/s/score.ly'), false, 'the server’s late outcome is not shown over the engine’s')
  })

  test('what the engine cannot do: it stopped, a module or file it could not have, too much memory; not the score’s own errors', async () => {
    assert.equal(cannotDo(outcome('/s/score.ly')), undefined)
    assert.equal(cannotDo(failedOutcome('/s/score.ly', 'syntax error, unexpected }')), undefined)
    assert.equal(cannotDo(stopped('/s/score.ly')), 'The engine stopped: unreachable')
    for (const message of [
      'this score needs more memory than the engine may use (256 MiB)',
      "cannot find font 'emmentaler-23' (its font pack emmentaler-23 could not be fetched)",
      "the outlines of font 'C059-Roman' could not be fetched (font pack C059-Roman.outlines): its text is spaced by its glyphs' boxes",
    ]) {
      assert.equal(cannotDo(failedOutcome('/s/score.ly', message)), message)
    }
    // The texts are the core's own (crates/core/src/diag.rs), which these patterns follow.
    const diag = readFileSync(path.resolve(__dirname, '..', '..', '..', 'engine', 'crates', 'core', 'src', 'diag.rs'), 'utf8')
    for (const text of [
      '"this score needs more memory than the engine may use ({} MiB)"',
      `"cannot find font '{font}' (its font pack {pack} could not be fetched)"`,
      `"the outlines of font '{face}' could not be fetched (font pack {pack}): its text is spaced by its glyphs' boxes"`,
    ]) {
      assert.ok(diag.includes(text), text)
    }

    const { live, compiles, server, events, start, settle } = page()
    await start()
    live.edited('/s/score.ly', '{ c }')
    await settle()
    server.length = 0
    compiles[0].answer(failedOutcome('/s/score.ly', 'this score needs more memory than the engine may use (256 MiB)'))
    await settle()
    assert.deepEqual(server, ['/s/score.ly={ c }', true])
    live.edited('/s/score.ly', '{ c')
    await settle()
    server.length = 0
    compiles[1].answer(failedOutcome('/s/score.ly', 'syntax error'))
    await settle()
    assert.deepEqual(server, [], 'a score’s own error stays the engine’s')
    assert.equal(events.at(-1)?.event?.kind, 'finished')
  })

  test('with live preview off or nothing unsaved, the server compiles the score as saved', async () => {
    const { live, compiles, server, start, settle } = page({ disk: { '/s/score.ly': 'saved' } })
    live.showScore('/s/score.ly')
    await start()
    live.edited('/s/score.ly', 'typed')
    await settle()
    live.setLive(false)
    await settle()
    server.length = 0
    compiles[1].answer(stopped('/s/score.ly'))
    await settle()
    assert.deepEqual(server, ['compile /s/score.ly'])
    assert.equal(live.serverCompile('finished', '/s/score.ly'), true)
    assert.deepEqual(server, ['compile /s/score.ly'], 'no texts were given, so none are forgotten')
  })

  test('a file the engine asked for that cannot be read sends its compile to the server, which can read it', async () => {
    const { live, compiles, server, start, settle } = page({ refused: ['/s/notes.txt'] })
    live.showScore('/s/score.ly')
    await start()
    live.edited('/s/score.ly', '\\include "notes.txt"')
    await settle()
    assert.equal(await live.read('/s/notes.txt'), null, 'none to the engine')
    assert.equal(await live.read('/s/missing.ily'), null)
    server.length = 0
    compiles[0].answer(failedOutcome('/s/score.ly', 'cannot find file: `notes.txt`'))
    await settle()
    assert.deepEqual(server, ['/s/score.ly=\\include "notes.txt"', true])
  })

  test('a file gone is none to the engine; one that cannot be read keeps what the engine had', async () => {
    const { live, compiles, start, settle, disk, connection } = page({ disk: { '/s/part.ily': 'c4', '/s/other.ily': 'd4' } })
    live.showScore('/s/score.ly')
    await start()
    await live.read('/s/part.ily')
    await live.read('/s/other.ily')
    delete disk['/s/part.ily']
    live.changed(['/s/part.ily'])
    live.edited('/s/score.ly', 'x')
    await settle()
    assert.deepEqual(compiles.at(-1)?.files, { '/s/part.ily': 'gone', '/s/score.ly': 'x' })
    connection.online = false
    live.changed(['/s/other.ily'])
    live.edited('/s/score.ly', 'y')
    await settle()
    assert.deepEqual(compiles.at(-1)?.files, { '/s/score.ly': 'y' }, 'offline: the engine keeps its copy')
    connection.online = true
    live.edited('/s/score.ly', 'z')
    await settle()
    assert.deepEqual(compiles.at(-1)?.files, { '/s/other.ily': 'bytes:d4', '/s/score.ly': 'z' }, 'read again once it can be')
  })

  test('offline, the engine goes on and its outcome is shown; once the connection is back the server is given what it could not do', async () => {
    const { live, compiles, server, events, start, settle, connection } = page()
    await start()
    connection.online = false
    live.edited('/s/score.ly', '{ c }')
    await settle()
    assert.equal(compiles.length, 1, 'the engine compiles without the server')
    compiles[0].answer(outcome('/s/score.ly'))
    await settle()
    live.edited('/s/score.ly', '{ d }')
    await settle()
    server.length = 0
    compiles[1].answer(stopped('/s/score.ly'))
    await settle()
    assert.deepEqual(server, [], 'no server to give it to')
    assert.deepEqual(
      events.filter(({ event }) => event?.kind === 'finished').length,
      2,
      'the engine’s outcome is shown',
    )
    connection.online = true
    live.reconnected()
    assert.deepEqual(server, ['/s/score.ly={ d }', true])
  })
  const madePdf: EnginePdf = { files: [{ name: 'score.pdf', data: new TextEncoder().encode('%PDF-1.7') }], warnings: [] }

  test('Export PDF: the engine compiles the score as live preview has it, shows that compile, and gives its PDFs (W7, D271)', async () => {
    const { live, compiles, pdfs, events, start, settle } = page()
    live.showScore('/s/score.ly')
    await start()
    live.edited('/s/score.ly', '{ c }')
    await settle()
    compiles[0].answer(outcome('/s/score.ly'))
    await settle()
    const exported = live.exportPdf('/s/score.ly')
    await settle()
    assert.equal(compiles.length, 1, 'no compile of its own beside the PDF’s')
    assert.deepEqual(pdfs.map(({ rootFile, files }) => [rootFile, files]), [['/s/score.ly', { '/s/score.ly': '{ c }' }]])
    pdfs[0].answer({ outcome: outcome('/s/score.ly'), pdf: madePdf })
    assert.deepEqual(await exported, madePdf)
    await settle()
    assert.equal(events.filter(({ event }) => event?.kind === 'finished').length, 2, 'its outcome is the preview’s')
  })

  test('Export PDF of a score with errors rejects as the server’s does; one the engine cannot do or write is the server’s (D271)', async () => {
    const { live, pdfs, start, settle } = page()
    await start()
    const failed: CompileOutcome = { ...outcome('/s/score.ly'), state: 'failed', errorCount: 1 }
    const errors = live.exportPdf('/s/score.ly')
    await settle()
    pdfs[0].answer({ outcome: failed, pdf: madePdf })
    await assert.rejects(errors, new Error(PDF_WITH_ERRORS))
    const cannot = live.exportPdf('/s/score.ly')
    await settle()
    pdfs[1].answer({ outcome: stopped('/s/score.ly'), pdf: { files: [], warnings: [] } })
    assert.equal(await cannot, undefined)
    const unwritten = live.exportPdf('/s/score.ly')
    await settle()
    pdfs[2].answer({ outcome: outcome('/s/score.ly'), pdf: { files: [], warnings: [], message: 'The engine could not write the PDF: pdf.1.wasm: 503' } })
    assert.equal(await unwritten, undefined)
  })

  test('Export PDF superseded by an edit is compiled once more with it, then left to the server; none before the engine runs', async () => {
    const { live, pdfs, start, settle } = page()
    assert.equal(await live.exportPdf('/s/score.ly'), undefined, 'the engine has not started')
    await start()
    const exported = live.exportPdf('/s/score.ly')
    await settle()
    live.edited('/s/score.ly', '{ d }')
    pdfs[0].answer(undefined)
    await settle()
    assert.deepEqual(pdfs[1].files, { '/s/score.ly': '{ d }' })
    pdfs[1].answer(undefined)
    assert.equal(await exported, undefined)
  })
})
