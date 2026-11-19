// MS8 step 11 (runway/OPEN.md's inherited tail): what no regression score of the corpus checks, compiled natively and
// by LilyPond 2.26.0 (../inputs.mjs, each score performed by both, D103), their stderr equal byte for byte:
//
// - syntax-case (D273): a macro, its failed match and its syntax-violation placed at the use, as psyntax places them;
// - C6: music and stream events written as LilyPond's Probs, ` >` and a newline, music with its type's display-methods;
// - C8: \applyContext's procedure, run once a translation, its warning given by the \layout's and the \midi's;
// - C2.10: an expected warning whose text runs on past its message into the excerpt, as Input::message_string has it;
// - C3's regression scores: ly:check-expected-warnings in a translation, Timing_translator's \polymetric warnings
//   given by both translations, as LilyPond gives them (POLYMETRIC); module-map of an output definition;
// - B33 (MS8 step 11a, D275): an error while a \layout's Paper_score is processed ends the run as LilyPond's.
//
// And C2.3: a file of several performances' playback map, LilyPond's byte for byte (timing.ly's staves of every
// performance but the last without voices), against the corpus's captures of such files (MULTIPLE).
// Skipped, and so unverified, without LilyPond 2.26.0, cargo, the corpus or its captures.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { runExample } from '../../examples.mjs'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { lilypondVersion } from '../capture.mjs'
import { stderrDiffering } from '../inputs.mjs'

const lilypond = lilypondVersion() === '2.26.0' ? false : 'LilyPond 2.26.0 is not installed'
const cargo = spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'

/** Each input's name and its text after `\version "2.26.0"`. */
export const INPUTS = [
  ['syntax-case-macro', "#(define-syntax swap! (lambda (x) (syntax-case x () ((_ a b) #'(let ((tmp a)) (set! a b) (set! b tmp))))))\n"
    + '#(define tmp 1)\n#(define y 2)\n#(swap! tmp y)\n#(ly:warning "~a ~a" tmp y)\n{ c\'1 }'],
  ['syntax-case-fail', "#(define-syntax twice (lambda (x) (syntax-case x () ((_ e) #'(begin e e)))))\n#(twice)\n{ c'1 }"],
  ['syntax-violation', "#(define-syntax strict (lambda (x) (syntax-case x () ((_ n) (if (number? (syntax->datum #'n)) #'n "
    + "(syntax-violation 'strict \"not a number\" x #'n))))))\n#(ly:warning \"~a\" (strict 3))\n#(strict x)\n{ c'1 }"],
  ['write-music', "#(write (make-music 'NoteEvent 'duration (ly:make-duration 2)) (current-error-port))\n"
    + "#(display (make-music 'NoteEvent 'articulations (list (make-music 'ArticulationEvent 'articulation-type \"accent\"))) (current-error-port))\n{ c'1 }"],
  ['write-event', "#(display (ly:make-stream-event '(note-event) (list (cons 'duration (ly:make-duration 2)))) (current-error-port))\n"
    + "{ \\applyContext #(lambda (c) (display (ly:make-stream-event '(rest-event) '()) (current-error-port))) c'1 }"],
  ['apply-context', '\\score { { \\applyContext #(lambda (c) (ly:warning "applied")) c\'4 } }'],
  ['expect-excerpt', '#(ly:expect-warning "bar check failed at: 1/4\n{ c\'4 ")\n{ c\'4 | d\'1 }'],
  ['expect-excerpt-unmet', '#(ly:expect-warning "bar check failed at: 1/4\n{ d")\n{ c\'4 | d\'1 }'],
  ['check-expected', '#(ly:expect-warning "never given")\n#(ly:check-expected-warnings)\n#(ly:expect-warning "bar check failed")\n'
    + '{ c\'4 | d\'1 \\applyContext #(lambda (c) (ly:check-expected-warnings)) }'],
  ['module-map', '\\layout { #(ly:warning "~a" (length (filter symbol? (module-map (lambda (s v) (and (variable-bound? v) '
    + '(ly:context-def? (variable-ref v)) s)) (current-module))))) }\n{ c\'1 }'],
  // B33 (MS8 step 11a, D275): a layout's error ends the run as a translation's.
  ['layout-error', "\\header { title = \"B33\" }\n{ \\override NoteHead.before-line-breaking = #(lambda (g) (car '())) c'4 d'4 }"],
]

/**
 * C3's regression scores that the engine gives as LilyPond does: ly:check-expected-warnings in \applyContext. Not
 * here: polymetric-time-warning-mid-measure.ly and polymeter-ossia-partial.ly, whose warning LilyPond's
 * warning-as-error makes fatal, which is no option of the engine's (D99); polymetric-time-warning-(un)set-in-timing-
 * staff.ly, whose \time sets Score's properties after \enablePerStaffTiming (runway/OPEN.md C23).
 */
const POLYMETRIC = [
  'polymetric-time-warning-measure-length-event.ly',
  'polymetric-time-warning-measure-length-irregular.ly',
  'polymetric-time-warning-measure-length-scaled.ly',
  'polymetric-time-warning-measure-length-with.ly',
  'polymetric-time-warning-set-in-timing-score.ly',
  'polymetric-time-warning-unset-in-timing-score.ly',
]
const regression = path.join(corpusDir(), 'lilypond-src', 'input', 'regression')
const noCorpus = existsSync(regression) ? false : `no ${regression}: run fetch.mjs`

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-tail-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

test("the engine's stderr of step 11's inputs and C3's regression scores equals LilyPond's", { skip: lilypond || cargo || noCorpus, timeout: 900_000 }, () => {
  const inputs = [...INPUTS, ...POLYMETRIC.map((name) => ({ file: path.join(regression, name) }))]
  const failed = stderrDiffering(inputs, scratch)
  assert.deepEqual(failed, [], failed.join('\n'))
})

/** The corpus's scores of more than one performance, their captured maps of every performance. */
function multiple() {
  const refs = path.join(corpusDir(), 'ref')
  return corpus().flatMap((row) => {
    const dir = path.join(refs, row.id)
    const map = existsSync(dir) && readdirSync(dir).find((f) => f.endsWith('.timing.json'))
    if (!map) return []
    const performances = JSON.parse(readFileSync(path.join(dir, map), 'utf8')).length
    return performances > 1 && existsSync(row.path) ? [{ row, map: path.join(dir, map) }] : []
  })
}

test('a file of several performances: its playback map timing.ly\'s, the earlier performances\' staves without voices (C2.3)', { skip: cargo || noCorpus, timeout: 900_000 }, () => {
  const scores = multiple()
  assert.ok(scores.length > 0, 'no capture of a file of several performances')
  const out = mkdtempSync(path.join(scratch, 'maps-'))
  const run = runExample('compile', ['--out', out, ...scores.map((s) => s.row.path)], { encoding: 'utf8' })
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
  for (const { row, map } of scores) {
    const mine = path.join(out, path.basename(map))
    assert.equal(readFileSync(mine, 'utf8'), readFileSync(map, 'utf8'), row.id)
  }
})
