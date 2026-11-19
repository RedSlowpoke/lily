// C1 through the core (MS2 step 26, 04 §9.3, D87): the playback map of
// part 3's 14 scores, which the engine's built-in performers write as
// <name>.timing.json (crates/core/examples/compile.rs), scored by
// score.mjs's C1 against the MS0 capture's: 1.0 on each, and the whole map
// equal, bars and staves too, a link compared without its directory. Then
// G8: timing.ly included unchanged in each score, through the engine
// without its own performers (--no-playback), writes the same map. Since
// step 27 the same through the built core.wasm (core.mjs), the 14 in one
// instance as the studio compiles them: C1 = 1.0 and the whole map equal,
// and every output the native core's; the times of each score's parse and
// translation in wasm are reported. The tests skip themselves, and so
// verify nothing, without LilyPond 2.26.0 (whose scm/ the engine loads),
// the corpus and its capture (tools/corpus/fetch.mjs, capture.mjs) or
// cargo, and the last without a core.wasm as new as the sources. Since
// step 35 (N6) the grace scores T09, T12 and T22 are among them (trace.mjs'
// TRACED), since step 36 the \change scores T11, T25, T26 and T27, and
// since step 37 (N7) the volta repeats' T07, T10 and T24, since step 38
// T20, its lyrics combined with their voice, and since step 39 (N7) T02,
// T04, T23 and T30, whose melismas hold syllables back, and T28: the 30
// (trace.mjs' MAPPED, TRACED's and T23). Since MS3 step 35 a score's pages
// in wasm, if the core makes any, are the native compile's too. Since MS5
// step 8, R4's gate (README §5, MS5: C1 against timing.ly's map), M01–M12
// as well: the 42, each map LilyPond's with the engine's performers
// writing MIDI beside the map's, and timing.ly's beside them (G8).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { lilypondVersion } from '../capture.mjs'
import { compileInWasm, coreSkip } from '../core.mjs'
import { compare, key } from '../score.mjs'
import { MAPPED } from '../trace.mjs'

// The 30 and M01–M12.
const SCORES = [...MAPPED, ...Array.from({ length: 12 }, (_, i) => `M${String(i + 1).padStart(2, '0')}`)]

const timingLy = createRequire(import.meta.url).resolve('@lily/common/runtime/timing.ly')
const rows = Object.fromEntries(corpus().map((row) => [row.id, row]))
const ref = path.join(corpusDir(), 'ref')
const skip = lilypondVersion() !== '2.26.0' ? 'LilyPond 2.26.0 is not installed'
  : spawnSync('cargo', ['+stable', '--version']).status !== 0 ? 'cargo +stable is not installed'
    : SCORES.every((id) => existsSync(rows[id].path) && existsSync(path.join(ref, id))) ? false
      : 'the corpus or its capture is not there'
const scratch = mkdtempSync(path.join(tmpdir(), 'lily-c1-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** Compiles `files` in one engine with `flags`, its outputs into `out`. */
function compile(files, out, flags = []) {
  mkdirSync(out, { recursive: true })
  const run = runExample('compile', [...flags, '--out', out, ...files], { encoding: 'utf8' })
  // 1: a file failed, as T03 does in LilyPond and T15 at ly:expect-warning.
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
}

/** `id`'s map from `out`, `<name>.timing.json`, alone in a directory of its own for score.mjs. */
function candidate(out, id, name) {
  const dir = path.join(out, 'by-id', id)
  mkdirSync(dir, { recursive: true })
  cpSync(path.join(out, `${name}.timing.json`), path.join(dir, `${name}.timing.json`))
  return dir
}

/** The map in `dir`, its links without their directories. */
function map(dir) {
  const file = readdirSync(dir).find((f) => f.endsWith('.timing.json'))
  const performances = JSON.parse(readFileSync(path.join(dir, file), 'utf8'))
  return performances.map((p) => ({ ...p, events: p.events.map((e) => ({ ...e, href: key(e.href) })) }))
}

/** C1 = 1.0 and the whole map equal for each of SCORES', their maps in `dirs` by ID. */
function assertEqual(dirs) {
  assert.equal(new Set(SCORES).size, 42)
  for (const id of SCORES) {
    const result = compare(path.join(ref, id), dirs[id])
    assert.equal(result.events, 1, id)
    assert.equal(result.pass.C1, true, id)
    assert.deepEqual(map(dirs[id]), map(path.join(ref, id)), id)
  }
}

/** The native core's outputs of SCORES', compiled once, in one engine. */
const native = (() => {
  let out
  return () => {
    if (!out) {
      out = path.join(scratch, 'engine')
      compile(SCORES.map((id) => rows[id].path), out)
    }
    return out
  }
})()

test("the engine's playback map of the 30 and M01–M12 is LilyPond's: C1 = 1.0", { skip, timeout: 900_000 }, () => {
  const out = native()
  assertEqual(Object.fromEntries(SCORES.map((id) => [id, candidate(out, id, path.basename(rows[id].path, '.ly'))])))
})

test("core.wasm's playback map of the 30 and M01–M12 is LilyPond's, and its outputs the native core's", { skip: skip || coreSkip(), timeout: 900_000 }, async (t) => {
  const out = native()
  const results = await compileInWasm(SCORES.map((id) => rows[id].path))
  const wasm = path.join(scratch, 'wasm')
  const dirs = {}
  for (const [i, id] of SCORES.entries()) {
    const { outputs, times } = results[i]
    const stem = path.basename(rows[id].path, '.ly')
    const map = `${stem}.timing.json`
    // The core's pages since MS3 step 35, each the native compile's, and
    // its MIDI files since MS5 step 1 (c2.test.mjs).
    const pages = Object.keys(outputs).filter((name) => name.endsWith('.svg'))
    const midis = Object.keys(outputs).filter((name) => name.endsWith('.midi'))
    assert.deepEqual(Object.keys(outputs), ['stderr', 'diagnostics.json', ...pages, ...midis, map], id)
    for (const midi of midis) assert.deepEqual(outputs[midi], readFileSync(path.join(out, midi)), `${id} ${midi}`)
    for (const page of pages) assert.deepEqual(outputs[page], readFileSync(path.join(out, page)), `${id} ${page}`)
    assert.deepEqual(outputs.stderr, readFileSync(path.join(out, `${stem}.err`)), id)
    assert.deepEqual(outputs['diagnostics.json'], readFileSync(path.join(out, `${stem}.json`)), id)
    assert.deepEqual(outputs[map], readFileSync(path.join(out, map)), id)
    dirs[id] = path.join(wasm, id)
    mkdirSync(dirs[id], { recursive: true })
    writeFileSync(path.join(dirs[id], map), outputs[map])
    assert.ok(times.translations >= 1, id)
    t.diagnostic(`${id}: ${results[i].ms.toFixed(2)} ms; parse ${times.parseMs.toFixed(2)} ms, ` +
      `${times.translations} translation(s) ${times.translateMs.toFixed(2)} ms`)
  }
  assertEqual(dirs)
})

test('timing.ly run unchanged in the engine writes the same map (G8)', { skip, timeout: 900_000 }, () => {
  const out = path.join(scratch, 'g8')
  mkdirSync(out, { recursive: true })
  // Each score after timing.ly, as -dinclude-settings puts it first.
  const files = SCORES.map((id) => {
    const file = path.join(out, `${id}.ly`)
    writeFileSync(file, `\\include "${timingLy}"\n\\include "${rows[id].path}"\n`)
    return file
  })
  compile(files, out, ['--no-playback'])
  assertEqual(Object.fromEntries(SCORES.map((id) => [id, candidate(out, id, id)])))
})
