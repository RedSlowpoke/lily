// The columns' skylines (MS3 step 30): tools/bench/probe-columns.scm's
// lines of the paper columns of tools/compare/fixtures/columns.ly, their
// 'horizontal-skylines (ly:separation-item::calc-skylines) as LilyPond
// 2.26.0 gives them in tools/compare/columns-calls.ly's Score, from the
// engine's compile of the same from its image natively
// (crates/core/examples/compile.rs) and in core.wasm: each line the same
// words, the same number of points and each number within 1e-4 of
// LilyPond's (four decimals), an infinity the same. And the same of
// tools/compare/fixtures/columns-notes.ly's columns of clefs, keys and
// notes (MS3 step 48, OPEN A4), the engine's driver run by LilyPond too.
// Skipped, and so
// unverified, without LilyPond 2.26.0, without cargo, or without a build as
// new as the sources.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { compileInWasm, coreSkip, preloaded } from '../../compare/core.mjs'
import { runExample } from '../../examples.mjs'
import { benchDir, lilyScm } from '../run.mjs'

const engine = path.resolve(benchDir, '../..')
const skip = !lilyScm()
  ? 'LilyPond 2.26.0 is not installed'
  : spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'

const probe = path.join(benchDir, 'probe-columns.scm')
const capture = path.join(engine, 'tools/compare/columns-calls.ly')
const fixture = path.join(engine, 'tools/compare/fixtures/columns.ly')
const notes = path.join(engine, 'tools/compare/fixtures/columns-notes.ly')
const scratch = mkdtempSync(path.join(tmpdir(), 'lily-columns-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** The file the probe's lines go to. */
const COLUMNS = 'columns.txt'
/** fixtures/columns.ly's columns: 19 timesteps' two. */
const COUNT = 38
/** Those of them with boxes: the bar lines' and time signatures', and the note names'. */
const FILLED = 25
/** fixtures/columns-notes.ly's: 13 timesteps' two, and those with boxes: the clefs', keys', time signatures', bar lines' and notes'. */
const NOTES_COUNT = 26
const NOTES_FILLED = 17
const TOLERANCE = 1e-4

/** A token as Guile writes it: a number or an infinity; else a word. */
const value = (token) => token === '+inf.0' ? Infinity : token === '-inf.0' ? -Infinity
  : token === '' || Number.isNaN(Number(token)) ? token : Number(token)

/** The lines that differ beyond four decimals, as [line, ours, LilyPond's]; and how many are equal as text. */
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

/** A text's lines, which it ends. */
const lines = (text, name) => {
  const all = text.split('\n')
  assert.equal(all.pop(), '', `${name}: the output ends a line`)
  return all
}

test('the comparison takes four decimals and nothing else', () => {
  const theirs = ['PaperColumn 0 horizontal-skylines left +inf.0 -inf.0 +inf.0 -2.685 0.045000000000000207 right -inf.0 -inf.0 -inf.0 +inf.0']
  assert.deepEqual(compare(theirs, theirs), { differ: [], exact: 1 })
  assert.deepEqual(compare(['PaperColumn 0 horizontal-skylines left +inf.0 -inf.0 +inf.0 -2.68501 0.045 right -inf.0 -inf.0 -inf.0 +inf.0'], theirs).differ, [])
  assert.equal(compare(['PaperColumn 0 horizontal-skylines left +inf.0 -inf.0 +inf.0 -2.6852 0.045 right -inf.0 -inf.0 -inf.0 +inf.0'], theirs).differ.length, 1)
  assert.equal(compare(['PaperColumn 1 horizontal-skylines left +inf.0 -inf.0 +inf.0 -2.685 0.045000000000000207 right -inf.0 -inf.0 -inf.0 +inf.0'], theirs).differ.length, 1)
  assert.equal(compare(['PaperColumn 0 horizontal-skylines right +inf.0 -inf.0 +inf.0 -2.685 0.045000000000000207 left -inf.0 -inf.0 -inf.0 +inf.0'], theirs).differ.length, 1)
  assert.equal(compare(['PaperColumn 0 horizontal-skylines left +inf.0 -inf.0 +inf.0 -2.685 0.045000000000000207 right -inf.0 -inf.0 -inf.0 -inf.0'], theirs).differ.length, 1)
  assert.equal(compare(['PaperColumn 0 horizontal-skylines left +inf.0 -inf.0 +inf.0 -2.685 right -inf.0 -inf.0 -inf.0 +inf.0'], theirs).differ.length, 1)
  assert.equal(compare(['PaperColumn 0 horizontal-skylines error misc-error'], theirs).differ.length, 1)
  assert.equal(compare([], theirs).differ.length, 1)
  assert.equal(compare([...theirs, theirs[0]], theirs).differ.length, 1)
})

/**
 * columns-calls.ly for the engine, in `dir`: the same recorder in the
 * Score of `included` (fixtures/columns.ly), unchanged, and the probe,
 * evaluated in (lily) in its finalize, before its lines are written to
 * columns.txt. The engine has no file system, so the probe's text is in
 * the file. Since MS3 step 33 the engine breaks the System as LilyPond
 * does (D130), so it expects columns-calls.ly's programming error: the
 * fixture has no vertical alignment.
 */
function driver(dir, included = fixture) {
  const text = readFileSync(probe, 'utf8')
  const file = path.join(dir, 'columns.ly')
  writeFileSync(file, `\\version "2.26.0"
#(ly:expect-warning "didn't find a vertical alignment in this system")
#(define columns-probe "${text.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}")
#(define (columns-recorder context)
   (let ((columns '()))
     (make-engraver
      ((stop-translation-timestep engraver)
       (for-each (lambda (property)
                   (let ((column (ly:context-property context property)))
                     (if (and (ly:grob? column) (not (memq column columns)))
                         (set! columns (cons column columns)))))
                 '(currentCommandColumn currentMusicalColumn)))
      ((finalize engraver)
       (save-module-excursion
        (lambda ()
          (call-with-input-string columns-probe
            (lambda (port)
              (let next ((form (read port)))
                (unless (eof-object? form)
                  (primitive-eval form)
                  (next (read port))))))))
       (with-output-to-file "${COLUMNS}"
         (lambda () ((@@ (lily) columns-run) (reverse columns))))))))
\\layout {
  \\context { \\Score \\consists #columns-recorder }
}
\\include "${included}"
`)
  return file
}

/** The lines of the engine's columns.txt of `file`, natively and in wasm, each checked against `theirs` with an empty stderr. */
async function compareEngine(t, file, theirs) {
  const packs = preloaded()
  assert.ok(packs.length > 0 && packs.every(([, pack]) => existsSync(pack)), 'npm run build writes dist/engine')
  const out = mkdtempSync(path.join(scratch, 'engine-'))
  const run = runExample('compile', [...packs.flatMap(([, pack]) => ['--pack', pack]), '--out', out, file], { cwd: out, encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  const [wasm] = await compileInWasm([file], { modules: packs.map(([name, pack]) => [name, readFileSync(pack)]) })
  for (const [name, text, err] of [
    ['from the image natively', readFileSync(path.join(out, COLUMNS), 'utf8'), readFileSync(path.join(out, 'columns.err'), 'utf8')],
    ['in wasm', wasm.outputs[COLUMNS]?.toString() ?? '', wasm.outputs.stderr.toString()],
  ]) {
    assert.equal(err, '', `${name}: stderr`)
    const ours = lines(text, name)
    const { differ, exact } = compare(ours, theirs)
    assert.deepEqual(differ, [], `${name}: the lines that differ, as [line, engine, LilyPond]`)
    t.diagnostic(`${name}: ${exact} of ${theirs.length} lines equal as written`)
  }
}

/** A non-musical and a musical column a timestep, numbered in turn, each a pair of skylines in LilyPond; `count` of them, `filled` with boxes. */
function assertColumns(theirs, count, filled) {
  theirs.forEach((line, i) => {
    const [type, n, property, side] = line.split(' ')
    assert.equal(`${type} ${n} ${property} ${side}`,
      `${i % 2 ? 'PaperColumn' : 'NonMusicalPaperColumn'} ${Math.floor(i / 2)} horizontal-skylines left`, line)
    assert.ok(line.includes(' right '), line)
  })
  assert.equal(theirs.length, count, 'the columns')
  // An empty column's line: its two skylines at their infinities alone.
  assert.equal(theirs.filter((line) => line.split(' ').length > 13).length, filled, 'the columns with boxes')
}

test("the skylines of the fixture's paper columns give LilyPond's points from the image natively and in wasm",
  { skip: skip || coreSkip(), timeout: 600_000 }, async (t) => {
    const lilypond = spawnSync('lilypond', ['-s', capture], { cwd: scratch, encoding: 'utf8' })
    assert.equal(lilypond.status, 0, lilypond.stderr)
    assert.equal(lilypond.stderr, '', 'LilyPond warns nothing')
    const theirs = lines(readFileSync(path.join(scratch, COLUMNS), 'utf8'), 'LilyPond')
    assertColumns(theirs, COUNT, FILLED)
    await compareEngine(t, driver(mkdtempSync(path.join(scratch, 'driver-'))), theirs)
  })

test("the skylines of the paper columns of clefs, keys and notes give LilyPond's points from the image natively and in wasm (OPEN A4)",
  { skip: skip || coreSkip(), timeout: 600_000 }, async (t) => {
    const file = driver(mkdtempSync(path.join(scratch, 'notes-')), notes)
    const dir = mkdtempSync(path.join(scratch, 'lilypond-'))
    const lilypond = spawnSync('lilypond', ['-s', '-dno-print-pages', file], { cwd: dir, encoding: 'utf8' })
    assert.equal(lilypond.status, 0, lilypond.stderr)
    assert.equal(lilypond.stderr, '', 'LilyPond warns nothing')
    const theirs = lines(readFileSync(path.join(dir, COLUMNS), 'utf8'), 'LilyPond')
    assertColumns(theirs, NOTES_COUNT, NOTES_FILLED)
    await compareEngine(t, file, theirs)
  })
