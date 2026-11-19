// O1 (MS3 steps 2 and 3): tools/bench/probe-stencils.scm's calls of
// stencils, boxes, transforms and lookup.cc's shapes give LilyPond 2.26.0's
// records (tools/compare/stencil-calls.ly) three ways: in (lily) loaded from
// source natively, restored natively from the image the build shipped
// (dist/engine/scheme.image), and restored from it in the built core.wasm
// (lsb_restore). Some calls read define-markup-commands.scm's empty-stencil
// and point-stencil, which the image holds as layout's stencils, and the
// last ones its markup commands and stencil.scm's procedures that draw the
// shapes. crates/layout/tests/stencils.rs runs the probe up to those
// natively in a VM without the library. Skipped, and so unverified, without
// LilyPond 2.26.0, without a core.wasm and a scheme.image as new as the
// sources (npm run build), or without cargo.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { runExample } from '../../examples.mjs'
import { benchDir, coreWasm, lilyScm, runOnce, schemeImage } from '../run.mjs'

const engine = path.resolve(benchDir, '../..')
const sources = ['crates/scheme/src', 'crates/music/src', 'crates/layout/src', 'crates/parser/src', 'crates/parser/tests/support', 'crates/core/src']
  .map((d) => path.join(engine, d))
const newest = Math.max(...sources.flatMap((dir) => readdirSync(dir).map((f) => statSync(path.join(dir, f)).mtimeMs)))
const lily = lilyScm()
const skip = !lily
  ? 'LilyPond 2.26.0 is not installed'
  : [coreWasm, schemeImage].map((file) => !existsSync(file)
    ? `no dist/engine/${path.basename(file)}: run npm run build`
    : statSync(file).mtimeMs < newest ? `dist/engine/${path.basename(file)} is older than the sources: run npm run build` : false)
    .find(Boolean) ?? (spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed')

const probe = path.join(benchDir, 'probe-stencils.scm')
const capture = path.join(engine, 'tools/compare/stencil-calls.ly')
const scratch = mkdtempSync(path.join(tmpdir(), 'lily-stencils-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** The probe's calls: its lines that begin with `(`, less its 12 definitions and the 6 of its markup commands' part. */
const CALLS = readFileSync(probe, 'utf8').split('\n').filter((line) => line.startsWith('(')).length - 18

/** The core's examples/bench.rs with `args`: its JSON lines. */
function native(...args) {
  const run = runExample('bench', args, { release: true, cwd: engine, encoding: 'utf8', maxBuffer: 1 << 26 })
  assert.equal(run.status, 0, run.stderr)
  return run.stdout.trim().split('\n').map((line) => JSON.parse(line))
}

/** A probe's records, one per call. */
const records = (output) => {
  const all = output.split('\u001e\n')
  assert.equal(all.pop(), '', 'the output ends a record')
  return all
}

test('stencils, transforms and shapes give LilyPond\'s records from source natively, from the image natively and in wasm',
  { skip, timeout: 600_000 }, async () => {
    const lilypond = spawnSync('lilypond', ['-s', capture], { cwd: scratch, encoding: 'utf8', maxBuffer: 1 << 26 })
    assert.equal(lilypond.status, 0, lilypond.stderr)
    const reference = records(lilypond.stdout)
    assert.equal(reference.length, CALLS, 'LilyPond ran every call')

    const [fromSource] = native('--library', lily, probe)
    const [fromImage] = native('--image', schemeImage, probe)
    const wasm = await runOnce(await WebAssembly.compile(readFileSync(coreWasm)), readFileSync(probe), readFileSync(schemeImage))
    assert.equal(wasm.init, 0, 'lsb_restore takes the image')
    for (const [name, run] of [['from source natively', fromSource], ['from the image natively', fromImage], ['from the image in wasm', wasm]]) {
      assert.ok(run.ok, `${name}: ${run.output.slice(-400)}`)
      const got = records(run.output)
      assert.equal(got.length, CALLS, `${name}: every call`)
      const differ = got.map((record, i) => [i + 1, record, reference[i]]).filter(([, a, b]) => a !== b)
      assert.deepEqual(differ, [], `${name}: the calls that differ, as [call, engine, LilyPond]`)
    }
  })
