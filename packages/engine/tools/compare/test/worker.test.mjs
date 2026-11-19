// K2 (08 §9, MS2 step 41, D101): the corpus through the studio's worker in
// Node. The 30 scores and the 30 error cases are compiled by the studio's own
// driver and host (packages/studio/src/web/engine/) over the built core.wasm
// and core.image: the studio's test/engine-corpus.test.ts, run by its
// `test:engine-corpus` script with LILY_ENGINE_DIST (this package's
// dist/engine/), LILY_DATADIR (the installed LilyPond 2.26.0's data
// directory, for `<datadir>/` names) and LILY_ENGINE_CORPUS (the runs and
// where their outputs go). Neither package imports the other's code (D56):
// this test reads the outputs the studio's test wrote. Each output is the
// native core's byte for byte (crates/core/examples/compile.rs, the scores
// by their paths, the cases from their directory, as c1.test.mjs and
// c0-cases.test.mjs compile them, with the build's preloaded packs the
// driver puts, D105: since MS3 step 40 nine of the 30, the five among them,
// and 24 of the cases lay out whole and have a page each way), but where
// TEXT says; C1 = 1.0 on the 30, the whole map equal; C0 on T03 against
// LilyPond's stderr, byte for byte, and on the 30 cases (D64's
// --parse-only). K4's gate (MS4 step 20, D223, D230): the studio's test
// compiles each of the 30, and fixtures/tablature.ly (the only one with chord
// names), again in an engine of its own with only the modules its triggers
// name and no restart, and with each split module of them left out once,
// which restarts if the score calls it and changes no output; MODULES says
// which each loads and needs. Since MS7 step 8 (M7, D264) a compile for a page
// leaves its `\midi`s out and the driver performs after it: both sides write
// the page's outputs and the performance's MIDI files and map (the example's
// `--perform`), and these are the compile's with its `\midi`s byte for byte,
// on the 30 and on the extension's sample, whose MIDI file is its fixture
// `sample.midi` (LilyPond's). The tests skip themselves, and so verify nothing, without
// LilyPond 2.26.0, cargo, npm, the corpus and its capture, or a core.wasm
// as new as the sources (npm run build).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { lilyScm } from '../../bench/run.mjs'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { lilypondVersion } from '../capture.mjs'
import { coreSkip, preloaded } from '../core.mjs'
import { compare, compareErrors, compareStderr, key } from '../score.mjs'
import { ERRORS, errorCases } from '../stderr.mjs'
import { SCORES } from '../trace.mjs'

const engine = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
const repo = path.resolve(engine, '../..')
const rows = Object.fromEntries(corpus().map((row) => [row.id, row]))
const ref = path.join(corpusDir(), 'ref')
const cases = errorCases()
const skip = lilypondVersion() !== '2.26.0' ? 'LilyPond 2.26.0 is not installed'
  : spawnSync('cargo', ['+stable', '--version']).status !== 0 ? 'cargo +stable is not installed'
    : spawnSync('npm', ['--version']).status !== 0 ? 'npm is not installed'
      : !SCORES.every((id) => existsSync(rows[id].path) && existsSync(path.join(ref, id))) ? 'the corpus or its capture is not there'
        : coreSkip()
const scratch = mkdtempSync(path.join(tmpdir(), 'lily-worker-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/**
 * Files whose outputs through the studio's driver are not the native
 * core's, and why. None since step 41a (D102): the page gives the engine a
 * file's bytes (protocol.ts' CompileRequest.files and the `file` answer), as
 * the native core reads them, so case 06 (Latin-1) gives LilyPond's two
 * `non-UTF-8 input` warnings through the driver too.
 */
const TEXT = {}

/**
 * The split modules (K4) each file loads by its triggers, and those it calls,
 * which leaving out costs a restart: T05's `\chordmode` makes no ChordName;
 * every other of the 30 loads none. Since MS4 step 20 (D230).
 */
const MODULES = {
  T05: { modules: ['chord-names'], needed: [] },
  T22: { modules: ['tablature'], needed: ['tablature'] },
  'tablature.ly': { modules: ['tablature', 'chord-names'], needed: ['tablature', 'chord-names'] },
}
const FIXTURES = path.join(engine, 'tools/compare/fixtures')

/**
 * The native compile example on `files` from `cwd`, in one engine with the
 * packs the driver preloads, for a preview as the driver's compiles are (a
 * visible page, MS6 step 10), each performed after its page as the driver
 * performs it (D264), unless `inline`: its outputs' directory.
 */
function compileNatively(files, cwd, out, { inline = false } = {}) {
  mkdirSync(out, { recursive: true })
  const packs = preloaded().flatMap(([, file]) => ['--pack', file])
  const run = runExample('compile', [...packs, '--preview', ...(inline ? [] : ['--perform']), '--out', out, ...files], { cwd, encoding: 'utf8' })
  // 1: a file failed, as T03 and the cases do.
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
  return out
}

/** The studio's driver on the 30 and the 30 cases, once: its outputs by name, `<out>/<name>/`. */
const throughDriver = (() => {
  let out
  return () => {
    if (out) return out
    out = path.join(scratch, 'driver')
    const spec = path.join(scratch, 'corpus.json')
    writeFileSync(spec, JSON.stringify({
      out,
      runs: [
        { dir: engine, files: SCORES.map((id) => rows[id].path), modules: true },
        { dir: ERRORS, files: cases.map((c) => `${c}.ly`) },
        { dir: FIXTURES, files: ['tablature.ly'], modules: true },
      ],
    }))
    // The studio's node --test runs as a runner of its own, not as this one's child.
    const { NODE_TEST_CONTEXT: _, ...env } = process.env
    const run = spawnSync('npm', ['run', '-s', 'test:engine-corpus', '-w', 'lily-studio'], {
      cwd: repo,
      encoding: 'utf8',
      maxBuffer: 1 << 26,
      env: {
        ...env,
        LILY_ENGINE_DIST: path.join(engine, 'dist/engine'),
        LILY_DATADIR: path.resolve(lilyScm(), '../..'),
        LILY_ENGINE_CORPUS: spec,
      },
    })
    assert.equal(run.status, 0, run.stdout + run.stderr)
    writeFileSync(path.join(scratch, 'driver.log'), run.stdout)
    return out
  }
})()

/** The native core's outputs of the 30 and of the cases, compiled once each. */
const native = (() => {
  let out
  return () => out ??= {
    scores: compileNatively(SCORES.map((id) => rows[id].path), engine, path.join(scratch, 'native-scores')),
    cases: compileNatively(cases.map((c) => `${c}.ly`), ERRORS, path.join(scratch, 'native-cases')),
  }
})()

/** The driver's outputs of `stem` and the native core's, by the driver's names. */
function pair(stem, nativeDir) {
  const dir = path.join(throughDriver(), stem)
  assert.ok(existsSync(path.join(dir, 'outcome.json')), `${stem}: not compiled through the driver`)
  const names = readdirSync(dir).filter((f) => f !== 'outcome.json' && f !== 'modules.json').sort()
  // A book of several pages writes `<stem>-<n>.svg` (protocol.ts' names), as T02 since MS3 step 62.
  const page = new RegExp(`^${stem.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}-\\d+\\.svg$`)
  const nativeNames = readdirSync(nativeDir).filter((f) => f.startsWith(`${stem}.`) || page.test(f)).sort()
  const native = { stderr: `${stem}.err`, 'diagnostics.json': `${stem}.json`, 'perform.stderr': `${stem}.perform.err`, 'perform.diagnostics.json': `${stem}.perform.json` }
  const nativeName = (name) => native[name] ?? name
  assert.deepEqual(names.map(nativeName).sort(), nativeNames, stem)
  return names.map((name) => [name, readFileSync(path.join(dir, name)), readFileSync(path.join(nativeDir, nativeName(name)))])
}

test("the 30 scores through the studio's driver: every output the native core's", { skip, timeout: 900_000 }, (t) => {
  for (const id of SCORES) {
    const stem = path.basename(rows[id].path, '.ly')
    for (const [name, got, want] of pair(stem, native().scores)) assert.ok(got.equals(want), `${id}: ${name}`)
    const { state, ms, asked } = JSON.parse(readFileSync(path.join(throughDriver(), stem, 'outcome.json'), 'utf8'))
    t.diagnostic(`${id}: ${state}, ${ms.toFixed(1)} ms, ${asked.length} asked`)
  }
})

test('the performance after the page: the 30\'s MIDI files and maps those of the compile with its \\midi, byte for byte', { skip, timeout: 900_000 }, (t) => {
  const inline = compileNatively(SCORES.map((id) => rows[id].path), engine, path.join(scratch, 'native-inline'), { inline: true })
  const after = native().scores
  const performed = (dir) => readdirSync(dir).filter((f) => /\.(midi?|timing\.json)$/.test(f)).sort()
  assert.deepEqual(performed(after), performed(inline))
  for (const file of performed(inline)) assert.ok(readFileSync(path.join(after, file)).equals(readFileSync(path.join(inline, file))), file)
  t.diagnostic(`${performed(inline).length} files`)
})

test("the extension's sample performed after its page: its fixture sample.midi, LilyPond's", { skip, timeout: 300_000 }, () => {
  const dir = path.join(scratch, 'sample')
  mkdirSync(dir, { recursive: true })
  cpSync(path.join(repo, 'packages/vscode/test/e2e/workspace/score.ly'), path.join(dir, 'score.ly'))
  const out = compileNatively(['score.ly'], dir, path.join(dir, 'out'))
  assert.ok(readFileSync(path.join(out, 'score.midi')).equals(readFileSync(path.join(repo, 'packages/vscode/test/fixtures/sample.midi'))))
})

test("the 30 cases through the studio's driver: every output the native core's, but TEXT's", { skip, timeout: 900_000 }, () => {
  assert.equal(cases.length, 30)
  for (const c of cases) {
    const outputs = pair(c, native().cases)
    if (TEXT[c]) assert.ok(outputs.some(([, got, want]) => !got.equals(want)), `${c} now matches: drop it from TEXT`)
    else for (const [name, got, want] of outputs) assert.ok(got.equals(want), `${c}: ${name}`)
  }
})

test("C1 = 1.0 on the 30 through the studio's driver, the whole map equal", { skip, timeout: 900_000 }, () => {
  const map = (dir) => JSON.parse(readFileSync(path.join(dir, readdirSync(dir).find((f) => f.endsWith('.timing.json'))), 'utf8'))
    .map((p) => ({ ...p, events: p.events.map((e) => ({ ...e, href: key(e.href) })) }))
  for (const id of SCORES) {
    const stem = path.basename(rows[id].path, '.ly')
    // The map alone in a directory, as score.mjs reads a candidate.
    const dir = path.join(scratch, 'c1', id)
    mkdirSync(dir, { recursive: true })
    cpSync(path.join(throughDriver(), stem, `${stem}.timing.json`), path.join(dir, `${stem}.timing.json`))
    const result = compare(path.join(ref, id), dir)
    assert.equal(result.events, 1, id)
    assert.equal(result.pass.C1, true, id)
    assert.deepEqual(map(dir), map(path.join(ref, id)), id)
  }
})

/**
 * What `stem` said through the driver (D264): its page's stderr, then what its
 * performance's translations said, before the run's `fatal error: failed
 * files:`, as LilyPond says it of one score.
 */
function saidThroughDriver(stem) {
  const page = readFileSync(path.join(throughDriver(), stem, 'stderr'), 'utf8')
  // A run a translation's error ended performs nothing.
  const said = path.join(throughDriver(), stem, 'perform.stderr')
  const performed = existsSync(said) ? readFileSync(said, 'utf8') : ''
  const failed = page.lastIndexOf('fatal error: failed files:')
  return failed < 0 ? page + performed : page.slice(0, failed) + performed + page.slice(failed)
}

test("C0 on T03 and the 30 cases through the studio's driver", { skip, timeout: 900_000 }, () => {
  const stem = path.basename(rows.T03.path, '.ly')
  const t03 = saidThroughDriver(stem)
  const reference = readFileSync(path.join(ref, 'T03', 'stderr.txt'), 'utf8')
  assert.equal(t03, reference)
  assert.ok(compareStderr(reference, t03).pass.C0)
  // Each case's stderr as `<case>.err`, as compareErrors reads a candidate.
  const dir = path.join(scratch, 'c0')
  mkdirSync(dir, { recursive: true })
  for (const c of cases) writeFileSync(path.join(dir, `${c}.err`), saidThroughDriver(c))
  const failed = compareErrors(dir, { cases, parseOnly: true }).filter((r) => !r.pass.C0)
  assert.deepEqual(failed.map((r) => r.id), Object.keys(TEXT), JSON.stringify(failed))
})

test("K4 through the studio's driver: each of the 30 with only its triggered modules, no restart, and a module left out a restart", { skip, timeout: 900_000 }, (t) => {
  const read = (stem) => JSON.parse(readFileSync(path.join(throughDriver(), stem, 'modules.json'), 'utf8'))
  for (const id of SCORES) {
    const { modules, needed, asked } = read(path.basename(rows[id].path, '.ly'))
    assert.deepEqual({ modules, needed }, MODULES[id] ?? { modules: [], needed: [] }, id)
    // I2: S6's chords and fretted parts are the manifest's, fetched by the triggers: none of LilyPond's files asked.
    if (MODULES[id]) t.diagnostic(`${id}: ${modules.join(', ')}; asked ${asked}`)
  }
  const fixture = read('tablature')
  assert.deepEqual({ modules: fixture.modules, needed: fixture.needed }, MODULES['tablature.ly'])
})
