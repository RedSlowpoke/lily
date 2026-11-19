import * as assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import * as fs from 'node:fs'
import { tmpdir } from 'node:os'
import * as path from 'node:path'
import { after, describe, test } from 'node:test'
import { formatTime, momentTime, parseMidi } from '@lily/common/web/midi.js'
import type { CompileOutcome } from '../src/ipc'
import { playbackChange } from '../src/renderer/player'
import { Driver, type Loader } from '../src/web/engine/driver'
import { EngineHost, type EngineWorker, withPerformance } from '../src/web/engine/host'
import { type EngineExports, type EngineManifest, type FromEngine, Need, type ToEngine } from '../src/web/engine/protocol'

// Runs under `node --test` from out/test/ (npm run test:unit in packages/studio/).
// The engine's built core.wasm and core.image, driven by the studio's own
// driver and host in Node (K2, chapter 08 §9 of docs/research/wasm-engine; MS2
// step 41, D101): the worker's side reads the engine's files from its build
// output, the page's side answers needs-file from disk, a `<datadir>/` name from
// LilyPond 2.26.0's data directory, with the files' bytes as read (D102). The engine is GPL and the studio MIT
// (D56): nothing of the engine's is imported, its build output is read as
// files, from LILY_ENGINE_DIST or packages/engine/dist/engine/ (where
// `esbuild.mjs --site` takes it from). Without that build, or without
// LilyPond 2.26.0 (or LILY_DATADIR), the tests skip themselves.
//
// LILY_ENGINE_CORPUS names a JSON file of runs, `{ out, runs: [{ dir, files }] }`:
// each run's files are compiled one after another in one engine, a name
// relative to `dir`, and each one's outputs, as lily_output gives them, go to
// `<out>/<its name without .ly>/` with `outcome.json`, what the driver made of
// them. The engine's tools/compare/test/worker.test.mjs writes it for the 30
// scores and the 30 error cases, and compares what lands there with the native
// core's and LilyPond's. A run with `modules: true` is K4's gate too (08 §2.3,
// the engine's D223): each of its files compiled again in an engine of its own
// with only the modules `modulesFor` names and no restart, and with each split
// module among them left out once, which changes no output and costs one
// restart if the score calls the module (a module a trigger fetched may go
// uncalled: T05's `\chordmode` makes no ChordName); what each loaded and
// needed goes to `<out>/<its name>/modules.json`. Since MS7 step 8 (M7, D264)
// a compile's outputs are its page's, and each file is performed after it:
// its MIDI files and map are written beside them.

/** packages/studio/, as out/test/ lies two levels below it. */
const studioDir = path.resolve(__dirname, '..', '..')
const dist = process.env.LILY_ENGINE_DIST ?? path.join(studioDir, '..', 'engine', 'dist', 'engine')

/** How the core names LilyPond's own files (DECISIONS D80). */
const DATADIR = '<datadir>'

/** share/lilypond/2.26.0/ of the lilypond on the PATH, or undefined. */
function installedDatadir(): string | undefined {
  const version = spawnSync('lilypond', ['--version'], { encoding: 'utf8' })
  if (version.error || !version.stdout.includes('GNU LilyPond 2.26.0')) return undefined
  const binary = spawnSync('sh', ['-c', 'command -v lilypond'], { encoding: 'utf8' }).stdout.trim()
  const dir = path.resolve(path.dirname(fs.realpathSync(binary)), '../share/lilypond/2.26.0')
  return fs.existsSync(path.join(dir, 'scm', 'lily')) ? dir : undefined
}
const datadir = process.env.LILY_DATADIR ?? installedDatadir()

const skip = !['manifest.json', 'core.wasm', 'core.image'].every((file) => fs.existsSync(path.join(dist, file)))
  ? `no engine build in ${dist}: run npm run build in packages/engine, or set LILY_ENGINE_DIST`
  : datadir
    ? false
    : 'LilyPond 2.26.0 is not installed, and LILY_DATADIR is not set'

const scratch = fs.mkdtempSync(path.join(tmpdir(), 'lily-engine-corpus-'))
after(() => fs.rmSync(scratch, { recursive: true, force: true }))

/**
 * Each instance the driver makes passes through `watch`, which may give the
 * driver other exports: so the tests see the core's instances and their
 * lily_compile calls, as engine.test.ts replaces the core.
 */
let watch: (module: WebAssembly.Module, exports: EngineExports) => EngineExports = (_, exports) => exports
const realInstantiate = WebAssembly.instantiate
WebAssembly.instantiate = (async (module: WebAssembly.Module, imports: WebAssembly.Imports) => {
  const instance = (await realInstantiate(module, imports)) as unknown as WebAssembly.Instance
  return { exports: watch(module, instance.exports as unknown as EngineExports) }
}) as unknown as typeof WebAssembly.instantiate
after(() => {
  WebAssembly.instantiate = realInstantiate
})

/**
 * What a compile asked of the page, the font packs the driver loaded when the
 * engine asked for them (W5, the engine's D208), and the revisions and roots
 * the driver compiled it with.
 */
interface Trail {
  asked: string[]
  packs: string[]
  revisions: number[]
  roots: string[]
  /** The split modules' files the driver loaded, and the core's instances it made (a restart makes one). */
  split: string[]
  instances: number
  /** LilyPond's own files the driver answered itself once the compile had started, never the page (D172, D263). */
  datadir: Set<string>
}

const noTrail = (): Trail => ({ asked: [], packs: [], revisions: [], roots: [], split: [], instances: 0, datadir: new Set() })

/**
 * The engine started by the studio's driver from the build in `dist`, its
 * worker and page in this process, their messages cloned and delivered a task
 * later as a worker's are. Each name the engine asks for is read from `dir` (a
 * `<datadir>/` name from LilyPond's). `outputs()` reads, through the ABI, the
 * outputs of the instance the driver compiled with last, as its own
 * `outputs()` does; `trail` what the compile running asked and ran.
 */
async function startEngine(dir: string, { leftOut }: { leftOut?: string } = {}) {
  const read: EngineManifest = JSON.parse(fs.readFileSync(path.join(dist, 'manifest.json'), 'utf8'))
  // A module left out is one `modulesFor` does not see (`name in modules`), which the driver still loads by name.
  const modules = new Proxy(read.modules, { has: (target, name) => name !== leftOut && name in target })
  const manifest: EngineManifest = { ...read, modules }
  const cores = new Set<WebAssembly.Module>()
  const loader: Loader = {
    module: async (file) => {
      const module = await WebAssembly.compile(fs.readFileSync(path.join(dist, file)))
      if (file === manifest.core.wasm) cores.add(module)
      else trail.split.push(file)
      return module
    },
    bytes: async (file) => {
      // A pack loaded once the compile has started is one the engine asked for, not one prefetched;
      // LilyPond's own files are counted where the driver puts them.
      const kind = Object.values(manifest.modules).find((module) => module.file === file)?.kind
      if (trail.revisions.length > 0 && kind !== 'file') trail.packs.push(file)
      return new Uint8Array(fs.readFileSync(path.join(dist, file)))
    },
  }
  let trail = noTrail()
  let engine: EngineExports | undefined
  watch = (module, exports) => {
    if (!cores.has(module)) return exports
    trail.instances++
    // What a new instance is given before its first compile is what the last had, and what a compile is given
    // before it starts (a trigger's fetch) is not asked for: neither is an answer.
    let started = false
    engine = {
      ...exports,
      lily_put: (kind, name, nameLen, data, dataLen) => {
        const named = new TextDecoder().decode(new Uint8Array(exports.memory.buffer, name, nameLen))
        if (kind === Need.File && named.startsWith(`${DATADIR}/`) && started && trail.revisions.length > 0) trail.datadir.add(named)
        exports.lily_put(kind, name, nameLen, data, dataLen)
      },
      lily_compile: (revision, root, rootLen, visiblePage) => {
        started = true
        trail.revisions.push(revision)
        trail.roots.push(new TextDecoder().decode(new Uint8Array(exports.memory.buffer, root, rootLen)))
        exports.lily_compile(revision, root, rootLen, visiblePage)
      },
    }
    return engine
  }

  const readFile = async (name: string) => {
    trail.asked.push(name)
    const file = name.startsWith(`${DATADIR}/`) ? path.join(datadir ?? '', name.slice(DATADIR.length + 1)) : path.resolve(dir, name)
    return fs.existsSync(file) ? new Uint8Array(fs.readFileSync(file)) : null
  }
  const later = (deliver: () => void) => void setImmediate(deliver)
  const worker: EngineWorker = {
    onmessage: null,
    postMessage: (message: ToEngine) => later(() => driver.receive(structuredClone(message))),
    terminate: () => {},
  }
  const post = (message: FromEngine) => later(() => worker.onmessage?.({ data: structuredClone(message) } as MessageEvent<FromEngine>))
  const driver = new Driver(manifest, loader, post, () => new Promise((resolve) => setImmediate(resolve)))
  const host = new EngineHost(worker, readFile)
  await driver.start()
  await host.ready

  return {
    manifest: read,
    /** Compiles `root` with its bytes from `dir`, or `files` as the page has them: what the driver made of it, and its trail. */
    async compile(root: string, files: Record<string, string | Uint8Array> = { [root]: new Uint8Array(fs.readFileSync(path.resolve(dir, root))) }) {
      trail = noTrail()
      const outcome = await host.compile(root, files)
      return { outcome, trail }
    },
    /** The newest compile's performance, after its page (D264). */
    perform: () => host.perform(),
    outputs(): Map<string, Uint8Array> {
      const e = engine
      if (!e) throw new Error('the driver made no instance of the core')
      const out = e.lily_alloc(16)
      const files = new Map<string, Uint8Array>()
      for (let i = 0; i < e.lily_outputs(); i++) {
        e.lily_output(i, out)
        const [name, nameLen, data, dataLen] = new Uint32Array(e.memory.buffer, out, 4)
        files.set(new TextDecoder().decode(new Uint8Array(e.memory.buffer, name, nameLen)), new Uint8Array(e.memory.buffer, data, dataLen).slice())
      }
      e.lily_free(out, 16)
      return files
    },
  }
}

/** The driver's outcome agrees with the outputs it was made of, and every start of the compile had its revision and root (D89). */
function assertOutcome(name: string, outcome: CompileOutcome | undefined, outputs: Map<string, Uint8Array>, trail: Trail): asserts outcome {
  assert.ok(outcome, `${name}: superseded, or the engine failed`)
  assert.notEqual(outcome.state, 'error', `${name}: ${outcome.message}`)
  const json = (file: string) => JSON.parse(new TextDecoder().decode(outputs.get(file)))
  assert.deepEqual(outcome.diagnostics, json('diagnostics.json'), name)
  // The map's entry 0, as the Rust side gives the renderer (read_timing, D103).
  const timing = [...outputs.keys()].find((file) => file.endsWith('.timing.json'))
  const first = timing ? json(timing)[0] : undefined
  // With the words' entry 0, when the score ran words.ly (read_words, D133).
  const words = timing?.replace(/timing\.json$/, 'words.json')
  const lyrics = words && outputs.has(words) ? json(words)[0] : []
  const expected = first && { events: first.events, bars: first.bars, ...(first.staves.length > 0 ? { staves: first.staves } : {}), ...(lyrics.length > 0 ? { lyrics } : {}) }
  assert.deepEqual(outcome.timing, expected, name)
  // The page's answers, the packs and LilyPond's own files the driver gave (an include's candidates at one stop, D263).
  assert.equal(trail.revisions.length, trail.asked.length + trail.packs.length + trail.datadir.size + 1, `${name}: a compile again after each answer`)
  assert.ok(!trail.asked.some((asked) => asked.startsWith(`${DATADIR}/`)), `${name}: the page is asked for none of LilyPond's own files`)
  assert.equal(new Set(trail.revisions).size, 1, `${name}: one revision for every start (D89)`)
  assert.deepEqual([...new Set(trail.roots)], [outcome.rootFile], `${name}: its root for every start (D89)`)
}

/** The performance agrees with the outputs it was made of: the first MIDI file, and the map's entry 0 only with one (D222, D264). */
function assertPerformance(name: string, performance: Awaited<ReturnType<EngineHost['perform']>>, outputs: Map<string, Uint8Array>): asserts performance {
  assert.ok(performance, `${name}: not performed`)
  assert.equal(performance.message, undefined, name)
  const midi = [...outputs.keys()].filter((file) => /\.midi?$/.test(file))
  assert.deepEqual(performance.midi, midi, name)
  assert.deepEqual(performance.midiData, midi.length > 0 ? outputs.get(midi[0]) : undefined, name)
  const json = (file: string) => JSON.parse(new TextDecoder().decode(outputs.get(file)))
  const timing = [...outputs.keys()].find((file) => file.endsWith('.timing.json'))
  const first = timing && midi.length > 0 ? json(timing)[0] : undefined
  // With the words' entry 0, when the score ran words.ly (read_words, D133).
  const words = timing?.replace(/timing\.json$/, 'words.json')
  const lyrics = words && outputs.has(words) ? json(words)[0] : []
  assert.deepEqual(performance.timing, first && { events: first.events, bars: first.bars, ...(first.staves.length > 0 ? { staves: first.staves } : {}), ...(lyrics.length > 0 ? { lyrics } : {}) }, name)
  assert.deepEqual(performance.diagnostics, outputs.has('diagnostics.json') ? json('diagnostics.json') : [], name)
}

describe('core.wasm through the studio’s driver', { skip }, () => {
  // player.test.ts' real score, by the engine: the page first, then its music (M7, D264).
  test('the music of a real score: the page keeps the music it has, then plays, every note a link on the pages', async () => {
    const dir = fs.mkdtempSync(path.join(scratch, 'song-'))
    const score = path.join(dir, 'song.ly')
    fs.writeFileSync(score, '\\version "2.24.0"\n\\score {\n  { \\tempo 4 = 120 c\'4 d\' e\'2 | f\'1 }\n  \\layout { }\n  \\midi { }\n}\n')
    const engine = await startEngine(dir)
    const { outcome } = await engine.compile(score)
    assert.ok(outcome)
    assert.equal(outcome.state, 'ok', outcome.message)
    assert.equal(outcome.performing, true)
    assert.equal(outcome.midiData, undefined)
    const loaded = { rootFile: score, midi: new Uint8Array([1]), timing: undefined }
    assert.deepEqual(playbackChange(outcome, loaded), { kind: 'keep' }, 'the player keeps its music until the performance')
    const performed = await engine.perform()
    assert.ok(performed)
    const whole = withPerformance(outcome, performed)
    assert.ok(whole.midiData, 'no MIDI was performed')
    assert.equal(playbackChange(whole, loaded).kind, 'load')
    const midi = parseMidi(whole.midiData)
    assert.equal(midi.notes.length, 4)
    assert.equal(formatTime(midi.duration), '0:04')
    assert.ok(whole.timing, 'no playback map was performed')
    const hrefs = whole.timing.events.map((event) => event.href)
    assert.equal(hrefs.length, 4)
    for (const href of hrefs) assert.ok(whole.svg.some((page) => page.includes(href)), `${href} is on no page`)
    // Bar 2 begins two seconds in.
    const bar2 = whole.timing.bars.find((bar) => bar.number === 2)
    assert.ok(bar2)
    assert.equal(momentTime(midi, bar2.at), 2)
    // Performed again: the same bytes, which the player plays on (D24).
    const again = await engine.perform()
    assert.deepEqual(again?.midiData, performed.midiData)
    assert.deepEqual(playbackChange(withPerformance(outcome, again!), { rootFile: score, midi: whole.midiData, timing: whole.timing }), { kind: 'keep' })
  })

  test('compiles a score, asks the page for its include, and reports a syntax error', async () => {
    const dir = fs.mkdtempSync(path.join(scratch, 'score-'))
    const root = path.join(dir, 'score.ly')
    fs.writeFileSync(root, '\\version "2.26.0"\n\\include "melody.ily"\n\\score { \\melody \\layout { } \\midi { } }\n')
    fs.writeFileSync(path.join(dir, 'melody.ily'), "melody = \\relative { c'4 d e f | g1 }\n")
    const engine = await startEngine(dir)

    const { outcome, trail } = await engine.compile(root)
    assertOutcome('score.ly', outcome, engine.outputs(), trail)
    assert.equal(outcome.state, 'ok')
    assert.deepEqual(outcome.diagnostics, [])
    assert.deepEqual(trail.asked, [path.join(dir, 'melody.ily')])
    // The engine's map, as timing.ly writes it: one entry a performance, which comes after the page (D264).
    assert.ok(!engine.outputs().has('score.timing.json'))
    assertPerformance('score.ly', await engine.perform(), engine.outputs())
    const [performance] = JSON.parse(new TextDecoder().decode(engine.outputs().get('score.timing.json')))
    assert.deepEqual(
      performance.events.map((event: { at: number }) => event.at),
      [0, 0.25, 0.5, 0.75, 1],
    )

    const broken = await engine.compile(root, { [root]: '\\version "2.26.0"\n{ c4 \\nosuch }\n' })
    assertOutcome('score.ly, broken', broken.outcome, engine.outputs(), broken.trail)
    assert.equal(broken.outcome.state, 'failed')
    // LilyPond 2.26.0's two errors.
    assert.deepEqual(
      broken.outcome.diagnostics.map((d) => [d.file, d.line, d.column, d.severity, d.message]),
      [
        [root, 2, 6, 'error', 'unknown command: `\\nosuch\''],
        [root, 2, 6, 'error', 'string outside of text script or \\lyricmode'],
      ],
    )
  })

  test('gives the engine a file that is not UTF-8 as its bytes: LilyPond’s warnings (D102)', async () => {
    const dir = fs.mkdtempSync(path.join(scratch, 'latin1-'))
    const root = path.join(dir, 'score.ly')
    // Latin-1 `é` in the score, the page's bytes as read, and in the include it asks for.
    const latin1 = (text: string) => Buffer.from(text, 'latin1')
    fs.writeFileSync(root, latin1('\\version "2.26.0"\n% é\n\\include "words.ily"\n{ c4 }\n'))
    fs.writeFileSync(path.join(dir, 'words.ily'), latin1('% è\n'))
    const engine = await startEngine(dir)

    const { outcome, trail } = await engine.compile(root)
    assertOutcome('score.ly', outcome, engine.outputs(), trail)
    assert.equal(outcome.state, 'ok')
    assert.deepEqual(trail.asked, [path.join(dir, 'words.ily')])
    assert.deepEqual(
      outcome.diagnostics.map((d) => [path.basename(d.file), d.line, d.column, d.severity, d.message]),
      [
        ['score.ly', 2, 3, 'warning', 'non-UTF-8 input'],
        ['words.ily', 1, 3, 'warning', 'non-UTF-8 input'],
      ],
    )
  })

  test('reads the words of a score that runs words.ly, this compile’s and not the last one’s (D133)', async () => {
    const dir = fs.mkdtempSync(path.join(scratch, 'words-'))
    const root = path.join(dir, 'score.ly')
    fs.copyFileSync(path.join(studioDir, 'runtime', 'words.ly'), path.join(dir, 'words.ly'))
    const score = (words: string) =>
      `\\version "2.26.0"\n\\include "words.ly"\n\\score {\n  <<\n    \\new Staff \\new Voice = "v" { c'4 d' e'2 }\n    \\new Lyrics \\lyricsto "v" { ${words} }\n  >>\n  \\layout { }\n  \\midi { }\n}\n`
    const engine = await startEngine(dir)
    const sung = async (words: string) => {
      const { outcome, trail } = await engine.compile(root, { [root]: score(words) })
      assertOutcome('score.ly', outcome, engine.outputs(), trail)
      assert.deepEqual(outcome.diagnostics, [])
      // The music, and the words with it, come after the page (D264).
      const performed = await engine.perform()
      assertPerformance('score.ly', performed, engine.outputs())
      return performed.timing?.lyrics
    }

    assert.deepEqual(await sung('la -- la \\markup \\italic bo'), [
      { at: 0, text: 'la', hyphen: true, extender: false, line: 0, voice: 'v', italic: false },
      { at: 0.25, text: 'la', hyphen: false, extender: false, line: 0, voice: 'v', italic: false },
      { at: 0.5, text: 'bo', hyphen: false, extender: false, line: 0, voice: 'v', italic: true },
    ])
    // The words edited, in the same engine, which keeps what it did (chapter 02 §5.7).
    assert.deepEqual(
      (await sung('do re mi'))?.map((lyric) => lyric.text),
      ['do', 're', 'mi'],
    )
  })

  test('a compile that starts again after its performers ran keeps each performance’s words once (D133)', async () => {
    const dir = fs.mkdtempSync(path.join(scratch, 'words-again-'))
    const root = path.join(dir, 'score.ly')
    fs.copyFileSync(path.join(studioDir, 'runtime', 'words.ly'), path.join(dir, 'words.ly'))
    const sung = (words: string) => `\\score {\n  <<\n    \\new Staff \\new Voice = "v" { c'4 d' }\n    \\new Lyrics \\lyricsto "v" { ${words} }\n  >>\n  \\layout { }\n  \\midi { }\n}\n`
    const engine = await startEngine(dir)

    // A text font the core may fetch: the engine compiles again.
    const { outcome, trail } = await engine.compile(root, { [root]: `\\version "2.26.0"\n\\include "words.ly"\n${sung('one two')}\\markup \\sans "x"\n${sung('three four')}` })
    assertOutcome('score.ly', outcome, engine.outputs(), trail)
    assert.equal(outcome.state, 'ok')
    // The performers run after the page (D264).
    assertPerformance('score.ly', await engine.perform(), engine.outputs())
    const outputs = engine.outputs()
    // It starts again (the include, the text font); the performers run once, after the page (D264).
    assert.ok(trail.revisions.length > 1, `no compile again: ${JSON.stringify(trail)}`)
    const words: { text: string }[][] = JSON.parse(new TextDecoder().decode(outputs.get('score.words.json')))
    assert.deepEqual(
      words.map((entry) => entry.map((lyric) => lyric.text)),
      [
        ['one', 'two'],
        ['three', 'four'],
      ],
    )
  })

  const corpus = process.env.LILY_ENGINE_CORPUS
  test('compiles the files of LILY_ENGINE_CORPUS, their outputs written for the engine’s tools', { skip: !corpus && 'LILY_ENGINE_CORPUS is not set: the engine’s worker.test.mjs sets it', timeout: 900_000 }, async (t) => {
    const { out, runs }: { out: string; runs: { dir: string; files: string[]; modules?: boolean }[] } = JSON.parse(fs.readFileSync(corpus ?? '', 'utf8'))
    for (const { dir, files } of runs) {
      const engine = await startEngine(dir)
      for (const name of files) {
        const started = performance.now()
        const { outcome, trail } = await engine.compile(name)
        const ms = performance.now() - started
        const outputs = engine.outputs()
        assertOutcome(name, outcome, outputs, trail)
        const performed = await engine.perform()
        const performedOutputs = engine.outputs()
        assertPerformance(name, performed, performedOutputs)
        const target = path.join(out, path.basename(name, '.ly'))
        assert.ok(!fs.existsSync(target), `${name}: two files of one name`)
        fs.mkdirSync(target, { recursive: true })
        for (const [file, bytes] of outputs) fs.writeFileSync(path.join(target, file), bytes)
        // The performance's MIDI files and map beside the page's outputs, and what it said, as the native example's --perform writes them.
        for (const [file, bytes] of performedOutputs) {
          const as = file === 'stderr' || file === 'diagnostics.json' ? `perform.${file}` : file
          if (/^perform\.|\.(midi?|timing\.json)$/.test(as)) fs.writeFileSync(path.join(target, as), bytes)
        }
        const { state, errorCount, warningCount } = outcome
        fs.writeFileSync(path.join(target, 'outcome.json'), JSON.stringify({ state, errorCount, warningCount, ms, ...trail }))
        t.diagnostic(`${name}: ${state}, ${ms.toFixed(1)} ms, asked ${trail.asked.length}`)
      }
    }
    for (const { dir, files, modules } of runs) {
      if (!modules) continue
      for (const name of files) {
        const written = path.join(out, path.basename(name, '.ly'), 'modules.json')
        fs.writeFileSync(written, JSON.stringify(await triggered(dir, name)))
      }
    }
  })
})

/**
 * K4's gate for `name` (08 §2.3, D223): compiled in an engine of its own, it
 * loads only the split modules `modulesFor` names and starts no compile again
 * but for the page's files and the packs the engine asks for; compiled with
 * each of those modules left out, its outputs are the same, and either it
 * hit the module's placeholder once, which cost a restart and loaded the
 * module (the module is `needed`), or it never called the module.
 */
async function triggered(dir: string, name: string) {
  const engine = await startEngine(dir)
  const { outcome, trail } = await engine.compile(name)
  const outputs = engine.outputs()
  assertOutcome(name, outcome, outputs, trail)
  assert.equal(trail.instances, 0, `${name}: a restart`)
  const split = Object.entries(engine.manifest.modules).filter(([, module]) => module.kind === 'split')
  const loaded = split.filter(([, module]) => trail.split.includes(module.file)).map(([module]) => module)
  const needed: string[] = []
  for (const module of loaded) {
    const without = await startEngine(dir, { leftOut: module })
    const left = await without.compile(name)
    assert.ok(left.outcome && left.outcome.state !== 'error', `${name} without ${module}: ${left.outcome?.message}`)
    assert.ok(left.trail.instances <= 1, `${name} without ${module}: ${left.trail.instances} restarts`)
    // The others before the first step, as before; this one last, at its placeholder, if it was called.
    const file = engine.manifest.modules[module].file
    const others = trail.split.filter((f) => f !== file)
    const restarted = left.trail.instances === 1
    assert.deepEqual(left.trail.split, restarted ? [...others, file] : others, `${name} without ${module}: loaded at its placeholder`)
    const { asked, packs, revisions, instances, datadir } = left.trail
    assert.equal(revisions.length, asked.length + packs.length + datadir.size + 1 + instances, `${name} without ${module}: a compile again after each answer and its restart`)
    assert.deepEqual(without.outputs(), outputs, `${name} without ${module}: the same outputs`)
    if (restarted) needed.push(module)
  }
  return { modules: loaded, needed, asked: trail.asked.length, packs: trail.packs }
}
