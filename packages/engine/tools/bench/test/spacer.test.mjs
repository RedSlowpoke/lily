// The spacer (MS3 step 29): tools/bench/probe-spacer.scm's calls of the
// springs' smob and of ly:solve-spring-rod-problem give LilyPond 2.26.0's
// lines (tools/compare/spacer-calls.ly) three ways: in (lily) loaded from
// source natively, restored natively from the image the build shipped
// (dist/engine/scheme.image), and restored from it in the built core.wasm
// (lsb_restore). Each line the same words, the same number of values and
// each number within 1e-4 of LilyPond's (four decimals), an infinity and a
// boolean the same. Skipped, and so unverified, without LilyPond 2.26.0,
// without a core.wasm and a scheme.image as new as the sources (npm run
// build), or without cargo.
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

const probe = path.join(benchDir, 'probe-spacer.scm')
const capture = path.join(engine, 'tools/compare/spacer-calls.ly')
const scratch = mkdtempSync(path.join(tmpdir(), 'lily-spacer-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** The probe's calls: the lines of its call list that begin with `(`. */
const CALLS = (() => {
  const text = readFileSync(probe, 'utf8')
  return text.slice(text.indexOf("(for-each probe-run '(")).split('\n').slice(1)
    .filter((line) => line.startsWith('(')).length
})()

const TOLERANCE = 1e-4

/** A token as Guile writes it: a number or an infinity; else a word, such as a boolean. */
const value = (token) => token === '+inf.0' ? Infinity : token === '-inf.0' ? -Infinity
  : token === '' || Number.isNaN(Number(token)) ? token : Number(token)

/** The calls whose lines differ beyond four decimals, as [call, ours, LilyPond's]; and how many lines are equal as text. */
function compare(ours, theirs) {
  const differ = []
  let exact = 0
  theirs.forEach((line, i) => {
    if (ours[i] === line) return exact++
    const [a, b] = [ours[i] ?? '', line].map((l) => l.split(' ').map(value))
    const same = a.length === b.length && a.every((x, j) => typeof x === 'string' || typeof b[j] === 'string' ? x === b[j]
      : x === b[j] || Math.abs(x - b[j]) <= TOLERANCE)
    if (!same) differ.push([i + 1, ours[i], line])
  })
  if (ours.length > theirs.length) differ.push(...ours.slice(theirs.length).map((line, i) => [theirs.length + i + 1, line, undefined]))
  return { differ, exact }
}

/** A run's lines. */
const lines = (output) => {
  const all = output.split('\n')
  assert.equal(all.pop(), '', 'the output ends a line')
  return all
}

test('the comparison takes four decimals and nothing else', () => {
  const theirs = ['#f 0.0 0.33333333333333337 2.0', 'error out-of-range Value out of range 0 to< 18446744073709551615: -1']
  assert.deepEqual(compare(theirs, theirs), { differ: [], exact: 2 })
  assert.deepEqual(compare(['#f 0.0 0.33337 2.0', theirs[1]], theirs).differ, [])
  assert.equal(compare(['#f 0.0 0.3336 2.0', theirs[1]], theirs).differ.length, 1)
  assert.equal(compare(['0.0 0.0 0.33333333333333337 2.0', theirs[1]], theirs).differ.length, 1)
  assert.equal(compare(['#f 0.0 0.33333333333333337', theirs[1]], theirs).differ.length, 1)
  assert.equal(compare([theirs[0], 'error out-of-range Value out of range 0 to< 9223372036854775807: -1'], theirs).differ.length, 1)
  assert.equal(compare(theirs.slice(0, 1), theirs).differ.length, 1)
  assert.equal(compare([...theirs, '1.0'], theirs).differ.length, 1)
})

test('springs and spring-rod problems give LilyPond\'s lines from source natively, from the image natively and in wasm',
  { skip, timeout: 600_000 }, async (t) => {
    const lilypond = spawnSync('lilypond', ['-s', capture], { cwd: scratch, encoding: 'utf8' })
    assert.equal(lilypond.status, 0, lilypond.stderr)
    assert.equal(lilypond.stderr, '', 'LilyPond warns nothing')
    const theirs = lines(lilypond.stdout)
    assert.equal(theirs.length, CALLS, 'LilyPond ran every call')

    /** The core's examples/bench.rs from `start`: its run of the probe. */
    const native = (...start) => {
      const run = runExample('bench', [...start, probe], { release: true, cwd: engine, encoding: 'utf8' })
      assert.equal(run.status, 0, run.stderr)
      return JSON.parse(run.stdout.trim().split('\n')[0])
    }
    const wasm = await runOnce(await WebAssembly.compile(readFileSync(coreWasm)), readFileSync(probe), readFileSync(schemeImage))
    assert.equal(wasm.init, 0, 'lsb_restore takes the image')
    for (const [name, run] of [['from source natively', native('--library', lily)],
      ['from the image natively', native('--image', schemeImage)], ['from the image in wasm', wasm]]) {
      assert.ok(run.ok, `${name}: ${run.output.slice(-400)}`)
      const ours = lines(run.output)
      assert.equal(ours.length, CALLS, `${name}: every call`)
      const { differ, exact } = compare(ours, theirs)
      assert.deepEqual(differ, [], `${name}: the calls that differ, as [call, engine, LilyPond]`)
      t.diagnostic(`${name}: ${exact} of ${CALLS} calls equal as written`)
    }
  })
