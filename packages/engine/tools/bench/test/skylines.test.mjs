// Skylines (MS3 step 26): tools/bench/probe-skylines.scm's ly:skyline-*
// calls on skylines built from segments give LilyPond 2.26.0's numbers
// (tools/compare/skyline-calls.ly) in crates/skyline's port, which
// tests/skyline.rs' the_probe_s_skyline_calls makes natively: each line the
// same numbers and booleans, a number within 1e-4 of LilyPond's (four
// decimals), an infinity the same; and the same calls with the probe's
// Scheme face (ly:make-skyline's and the others' checks, the stencils'
// skylines; MS3 step 48) evaluated in the engine's (lily) from its image
// natively and in core.wasm, each line LilyPond's (an error's word for
// word), with LilyPond's warnings, but E8's two: the partial ellipses,
// whose arcs keep a zero-width building in LilyPond that the libm crate's
// sin, a last bit off Apple's, merges away (the user's answer of
// 2026-10-23 at MS3 step 48), equal once zero-width buildings are left
// out of both. And the default skylines of
// fixtures/grobs.ly's grobs (the probe's last section), 'vertical-skylines
// and 'horizontal-skylines as LilyPond gives them in skyline-calls.ly's
// Score, from the engine's compile of the same from its image natively
// (crates/core/examples/compile.rs) and in core.wasm: each line the same
// words, the same number of points and each number within 1e-4. Skipped,
// and so unverified, without LilyPond 2.26.0 or without cargo, and the
// engine's without a build as new as the sources.
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

const capture = path.join(engine, 'tools/compare/skyline-calls.ly')
const scratch = mkdtempSync(path.join(tmpdir(), 'lily-skylines-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

const PROBE = readFileSync(path.join(benchDir, 'probe-skylines.scm'), 'utf8')
/** Where the probe's section of its Scheme face begins, and that of the grobs' default skylines. */
const FACE = ';;; The Scheme face'
const SECTION = ";;; The grobs' default skylines"
/** The calls of a part of the probe: its lines that begin with `(ly:`, `(list ` or `(skyline-face-`. */
const calls = (text) => text.split('\n')
  .filter((line) => ['(ly:', '(list ', '(skyline-face-stencil '].some((head) => line.startsWith(head))).length
/** The probe's calls on built skylines, which the port makes too, and those of its Scheme face. */
const CALLS = calls(PROBE.slice(0, PROBE.indexOf(FACE)))
const FACE_CALLS = calls(PROBE.slice(PROBE.indexOf(FACE), PROBE.indexOf(SECTION)))
/** The lines of E8's calls, from 1, as compare numbers them: the partial ellipses' skylines. */
const E8 = PROBE.slice(PROBE.indexOf(FACE), PROBE.indexOf(SECTION)).split('\n')
  .filter((line) => line.startsWith('(skyline-face-stencil ') || line.startsWith('(ly:') || line.startsWith('(list '))
  .flatMap((line, i) => line.includes('make-partial-ellipse-stencil') ? [CALLS + i + 1] : [])
/** A line without its zero-width buildings: a building is two points, four numbers, and one of no width repeats its point. */
const withoutZeroWidth = (line) => line.split(' ').flatMap((_, i, all) => i % 4 ? []
  : all[i] === all[i + 2] && all[i + 1] === all[i + 3] ? [] : all.slice(i, i + 4)).join(' ')
/** The warnings LilyPond writes of the probe's calls: the two negative paddings'. */
const WARNINGS = Array(2).fill('warning: Cannot have negative horizon padding.  Junking.\n').join('')

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

test('the comparison takes four decimals and nothing else', () => {
  const theirs = ['-inf.0 0.0 1.0 #t', '2.6666666666666665']
  assert.deepEqual(compare(theirs, theirs), { differ: [], exact: 2 })
  assert.deepEqual(compare(['-inf.0 0.00009 1.0 #t', '2.6667'], theirs).differ, [])
  assert.equal(compare(['-inf.0 0.0002 1.0 #t', '2.6666666666666665'], theirs).differ.length, 1)
  assert.equal(compare(['+inf.0 0.0 1.0 #t', '2.6666666666666665'], theirs).differ.length, 1)
  assert.equal(compare(['-inf.0 0.0 1.0 #f', '2.6666666666666665'], theirs).differ.length, 1)
  assert.equal(compare(['-inf.0 0.0 1.0', '2.6666666666666665'], theirs).differ.length, 1)
  assert.equal(compare(theirs.slice(0, 1), theirs).differ.length, 1)
  assert.equal(compare([...theirs, '1.0'], theirs).differ.length, 1)
  const grob = ['Fingering 0 vertical-skylines down -inf.0 +inf.0 0.0 -0.0 up 1.13599850824311']
  assert.deepEqual(compare(['Fingering 0 vertical-skylines down -inf.0 +inf.0 0.0 0.0 up 1.1360'], grob).differ, [])
  assert.equal(compare(['Fingering 1 vertical-skylines down -inf.0 +inf.0 0.0 -0.0 up 1.13599850824311'], grob).differ.length, 1)
  assert.equal(compare(['Fingering 0 vertical-skylines up -inf.0 +inf.0 0.0 -0.0 down 1.13599850824311'], grob).differ.length, 1)
  assert.equal(compare(['Fingering 0 vertical-skylines down -inf.0 +inf.0 0.0 -0.0 up'], grob).differ.length, 1)
})

let captured
/** LilyPond's run of skyline-calls.ly in scratch, made once: its standard output's lines and skyline-grobs.txt's. */
const captureLilyPond = () => captured ??= (() => {
  const lilypond = spawnSync('lilypond', ['-l', 'WARNING', capture], { cwd: scratch, encoding: 'utf8' })
  assert.equal(lilypond.status, 0, lilypond.stderr)
  assert.equal(lilypond.stderr, WARNINGS, 'LilyPond warns of the negative paddings and nothing else')
  const lines = (text) => {
    const all = text.split('\n')
    assert.equal(all.pop(), '', 'the output ends a line')
    return all
  }
  return { calls: lines(lilypond.stdout), grobs: lines(readFileSync(path.join(scratch, GROBS), 'utf8')) }
})()

test('skyline calls on built skylines give LilyPond\'s numbers in the port', { skip, timeout: 600_000 }, (t) => {
  const all = captureLilyPond().calls
  assert.equal(all.length, CALLS + FACE_CALLS, 'LilyPond ran every call')
  const theirs = all.slice(0, CALLS)
  assert.deepEqual(theirs.filter((line) => line.startsWith('error')), [], 'no call on built skylines is an error in LilyPond')

  const out = path.join(scratch, 'port.txt')
  const port = spawnSync('cargo', ['+stable', 'test', '-q', '--manifest-path', path.join(engine, 'Cargo.toml'),
    '-p', 'lily-engine-skyline', '--test', 'skyline', '--', '--exact', 'the_probe_s_skyline_calls'],
  { env: { ...process.env, SKYLINE_CALLS_OUT: out }, encoding: 'utf8' })
  assert.equal(port.status, 0, port.stdout + port.stderr)
  const ours = readFileSync(out, 'utf8').split('\n')
  assert.equal(ours.pop(), '', 'the port\'s output ends a line')

  const { differ, exact } = compare(ours, theirs)
  assert.deepEqual(differ, [], 'the calls that differ, as [call, port, LilyPond]')
  t.diagnostic(`${exact} of ${CALLS} calls equal as written`)
})

/** The file the engine's run of the probe's calls writes. */
const CALLS_OUT = 'skyline-calls.txt'

/**
 * The probe's calls for the engine, in `dir`: the probe up to its section of
 * the grobs' default skylines, its built skylines and its Scheme face,
 * evaluated in (lily) at the top level with its lines written to
 * skyline-calls.txt. The engine has no file system, so the text is in the
 * file.
 */
function callsDriver(dir) {
  const text = PROBE.slice(0, PROBE.indexOf(SECTION))
  const file = path.join(dir, 'skyline-calls.ly')
  writeFileSync(file, `\\version "2.26.0"
#(define skyline-calls-probe "${text.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}")
#(with-output-to-file "${CALLS_OUT}"
   (lambda ()
     (save-module-excursion
      (lambda ()
        (call-with-input-string skyline-calls-probe
          (lambda (port)
            (let next ((form (read port)))
              (unless (eof-object? form)
                (primitive-eval form)
                (next (read port))))))))))
`)
  return file
}

test("the probe's calls and its Scheme face give LilyPond's lines in the engine natively and in wasm",
  { skip: skip || coreSkip(), timeout: 600_000 }, async (t) => {
    const theirs = captureLilyPond().calls
    assert.equal(theirs.length, CALLS + FACE_CALLS, 'LilyPond ran every call')
    const packs = preloaded()
    const file = callsDriver(mkdtempSync(path.join(scratch, 'calls-')))
    const out = mkdtempSync(path.join(scratch, 'engine-'))
    const run = runExample('compile', [...packs.flatMap(([, pack]) => ['--pack', pack]), '--out', out, file], { cwd: out, encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const [wasm] = await compileInWasm([file], { modules: packs.map(([name, pack]) => [name, readFileSync(pack)]) })
    for (const [name, text, err] of [
      ['from the image natively', readFileSync(path.join(out, CALLS_OUT), 'utf8'), readFileSync(path.join(out, 'skyline-calls.err'), 'utf8')],
      ['in wasm', wasm.outputs[CALLS_OUT]?.toString() ?? '', wasm.outputs.stderr.toString()],
    ]) {
      assert.equal(err, WARNINGS, `${name}: stderr, LilyPond's warnings`)
      const ours = text.split('\n')
      assert.equal(ours.pop(), '', `${name}: the output ends a line`)
      assert.equal(ours.length, theirs.length, `${name}: a line a call`)
      const errors = theirs.map((line, i) => [line, ours[i]]).filter(([line]) => line.startsWith('error'))
      assert.deepEqual(errors.map(([, line]) => line), errors.map(([line]) => line), `${name}: the errors word for word`)
      const { differ, exact } = compare(ours, theirs)
      assert.deepEqual(differ.map(([i]) => i), E8, `${name}: the calls that differ, E8's alone: ${JSON.stringify(differ.filter(([i]) => !E8.includes(i)))}`)
      const e8 = (all) => E8.map((i) => withoutZeroWidth(all[i - 1]))
      assert.deepEqual(compare(e8(ours), e8(theirs)).differ, [], `${name}: E8's calls without their zero-width buildings`)
      t.diagnostic(`${name}: ${exact} of ${theirs.length} calls equal as written; E8's ${E8.length} by their buildings of some width`)
    }
  })

/** The file skyline-calls.ly's grobs' lines go to. */
const GROBS = 'skyline-grobs.txt'
/** The lines the probe writes of each grob: its two properties. */
const PROPERTIES = ['vertical-skylines', 'horizontal-skylines']
/** A line's grob and property: its first three words. */
const key = (line) => line.split(' ', 3).join(' ')
/**
 * skyline-calls.ly's grobs for the engine, in `dir`: the same recorder in
 * the Score of fixtures/grobs.ly, included unchanged, and the probe's
 * section of the grobs' default skylines, from its heading on, evaluated in
 * (lily) in its finalize, before its lines are written to skyline-grobs.txt.
 * The engine has no file system, so the section's text is in the file; the
 * probe's built skylines are callsDriver's.
 */
function driver(dir) {
  const section = PROBE.slice(PROBE.indexOf(SECTION))
  const file = path.join(dir, 'skyline-grobs.ly')
  writeFileSync(file, `\\version "2.26.0"
#(define skyline-grobs-probe "${section.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}")
#(define (skyline-grobs-recorder context)
   (let ((heard '()))
     (make-engraver
      (acknowledgers
       ((grob-interface engraver grob source-engraver)
        (set! heard (cons grob heard))))
      ((finalize engraver)
       (save-module-excursion
        (lambda ()
          (set-current-module (resolve-module '(lily)))
          (call-with-input-string skyline-grobs-probe
            (lambda (port)
              (let next ((form (read port)))
                (unless (eof-object? form)
                  (primitive-eval form)
                  (next (read port))))))))
       (with-output-to-file "${GROBS}"
         (lambda () ((@@ (lily) skyline-grobs-run) (reverse heard))))))))
\\layout {
  \\context { \\Score \\consists #skyline-grobs-recorder }
}
\\include "${path.join(engine, 'tools/compare/fixtures/grobs.ly')}"
`)
  return file
}

test("the default skylines of the fixture's grobs give LilyPond's points from the image natively and in wasm",
  { skip: skip || coreSkip(), timeout: 600_000 }, async (t) => {
    const theirs = captureLilyPond().grobs
    // Every grob of skyline-grobs-types the fixture makes, its two lines in
    // turn, and each a pair of skylines in LilyPond.
    const counts = {}
    theirs.forEach((line, i) => {
      const [type, n, property, side] = line.split(' ')
      assert.equal(property, PROPERTIES[i % 2], line)
      if (i % 2 === 0) counts[type] = (counts[type] ?? 0) + 1
      assert.equal(`${type} ${n}`, `${type} ${counts[type] - 1}`, line)
      assert.ok(side === 'down' && line.includes(' up '), line)
    })
    assert.deepEqual(counts, { Fingering: 6, DynamicText: 2, StrokeFinger: 7, StringNumber: 6 })

    // The pack of Fingering's font-size of -5 at staff size 20 (MS3 step 14a), and the core's.
    const packs = [...preloaded(), ['emmentaler-11', path.join(engine, 'dist/engine/fonts/emmentaler-11.pack')]]
    assert.ok(packs.every(([, pack]) => existsSync(pack)), 'npm run build writes dist/engine')
    const file = driver(mkdtempSync(path.join(scratch, 'driver-')))
    const out = mkdtempSync(path.join(scratch, 'engine-'))
    const run = runExample('compile', [...packs.flatMap(([, pack]) => ['--pack', pack]), '--out', out, file], { cwd: out, encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const [wasm] = await compileInWasm([file], { modules: packs.map(([name, pack]) => [name, readFileSync(pack)]) })
    for (const [name, text, err] of [
      ['from the image natively', readFileSync(path.join(out, GROBS), 'utf8'), readFileSync(path.join(out, 'skyline-grobs.err'), 'utf8')],
      ['in wasm', wasm.outputs[GROBS]?.toString() ?? '', wasm.outputs.stderr.toString()],
    ]) {
      assert.equal(err, '', `${name}: stderr`)
      const ours = text.split('\n')
      assert.equal(ours.pop(), '', `${name}: the output ends a line`)
      assert.deepEqual(ours.map(key), theirs.map(key), `${name}: a line of each grob and property LilyPond's`)
      // Every line: DynamicText's 4, StrokeFinger's 14 and StringNumber's 6
      // vertical skylines, since step 51 Fingering's 6, whose font-features
      // (cv47, ss01) pick Emmentaler's alternate digits (W6, D209), and since
      // step 53 Fingering's and StringNumber's 12 horizontal ones, whose
      // cross-staff's callback calls ly:script-interface::calc-cross-staff
      // (OPEN A5).
      assert.equal(theirs.length, 42, `${name}: the lines compared`)
      const { differ, exact } = compare(ours, theirs)
      assert.deepEqual(differ, [], `${name}: the lines that differ, as [line, engine, LilyPond]`)
      t.diagnostic(`${name}: ${exact} of 42 lines compared equal as written`)
    }
  })
