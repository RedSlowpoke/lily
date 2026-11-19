// Music trees (music.ly, music.mjs): the compare's self-tests (a tree against
// itself passes, a perturbed copy fails: a node, a property, a value, an
// origin), T14's tree as LilyPond 2.26.0 gives it to the book handler,
// written by music.ly through capture.mjs --parse, and the engine's trees of
// T14 and T16 (crates/parser/examples/parse.rs --music, MS2 steps 4 and 5)
// against LilyPond's, and of P6's nine scores (step 7), each equal whole
// since MS2 step 20, of P10's three (step 21), equal whole, of P8's
// lyric scores, T20, T02, T04 and T30 (step 32), equal whole, and of P11's
// T23 and T28, past their #{ … #} templates (step 33), and T26, past its
// \etc (step 34), equal whole. The tests
// on captures skip themselves, and so verify nothing, without LilyPond
// 2.26.0, the corpus (tools/corpus/fetch.mjs) or cargo.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { corpus } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { capture, lilypondVersion } from '../capture.mjs'
import { compareMusic, readMusic } from '../music.mjs'

// T14's tree, as music.ly writes it.
const T14 = `score 1
SequentialMusic @music-function.ly:15:0-17:1
  elements:
    NoteEvent @music-function.ly:16:4-16:6
      duration = #<Duration 4 >
      length = #<Mom 1/4>
      pitch = #<Pitch d >
    BarEvent @music-function.ly:16:7-16:19
      bar-type = ".|:"
      length = #<Mom 0>
    NoteEvent @music-function.ly:16:20-16:22
      duration = #<Duration 4 >
      length = #<Mom 1/4>
      pitch = #<Pitch d >
  length = #<Mom 1/2>
`

/** The self-tests of the compare on `tree`, a dump with a BarEvent of T14's. */
function selfTests(tree) {
  const nodes = readMusic(tree).filter((l) => /^[A-Z]\w* @/.test(l.text)).length
  const lines = readMusic(tree).length
  assert.deepEqual(compareMusic(tree, tree), { equal: true, nodes: [nodes, nodes], lines: [lines, lines] })
  const perturbed = {
    node: tree.replace('BarEvent @', 'TextScriptEvent @'),
    property: tree.replace('bar-type = ', 'bar-kind = '),
    value: tree.replace('".|:"', '"|."'),
    origin: tree.replace('@music-function.ly:16:7-16:19', '@music-function.ly:16:7-16:18'),
    dropped: tree.replace(/\n *bar-type = [^\n]*/, ''),
  }
  for (const [what, copy] of Object.entries(perturbed)) {
    assert.notEqual(copy, tree, what)
    const result = compareMusic(tree, copy)
    assert.equal(result.equal, false, what)
    assert.match(result.path, /^score 1 › SequentialMusic @music-function\.ly:15:0-17:1 › elements › /, what)
  }
  assert.equal(compareMusic(tree, perturbed.value).expected.trim(), 'bar-type = ".|:"')
  assert.match(compareMusic(tree, perturbed.value).path, / › BarEvent @[^ ]+ › bar-type$/)
}

test('a tree equals itself, and a perturbed copy differs where it was changed', () => selfTests(T14))

const lilypond = lilypondVersion() === '2.26.0' ? false : 'LilyPond 2.26.0 is not installed'
const rows = Object.fromEntries(corpus().map((row) => [row.id, row]))
const t14 = rows.T14
const skip = lilypond || (['T14', 'T16'].every((id) => existsSync(rows[id].path)) ? false : 'the corpus is not fetched')
const cargo = spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-music-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

test("music.ly writes T14's tree as the book handler gets it", { skip }, async () => {
  const [result] = await capture({ parse: true, ids: ['T14'], out: scratch })
  assert.equal(result.exit, 0)
  assert.equal(result.scores, 1)
  const tree = readFileSync(path.join(scratch, 'T14/music.txt'), 'utf8')
  assert.equal(tree, T14)
  selfTests(tree)
})

/** The engine's trees of `file`, parsed by crates/parser/examples/parse.rs from its directory. */
function engineMusic(file) {
  const out = mkdtempSync(path.join(scratch, 'engine-'))
  const run = runExample('parse', ['--music', out, file], { cwd: path.dirname(file), encoding: 'utf8' })
  // 1: the file failed, as LilyPond's may; 2: the runner could not run.
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
  return readFileSync(path.join(out, `${path.basename(file, '.ly')}.music`), 'utf8')
}

test("the engine's trees of T14 and T16 equal LilyPond's", { skip: skip || cargo }, async () => {
  const ref = path.join(scratch, 'ref')
  await capture({ parse: true, ids: ['T14', 'T16'], out: ref })
  const reference = (id) => readFileSync(path.join(ref, id, 'music.txt'), 'utf8')
  assert.deepEqual(compareMusic(reference('T14'), engineMusic(t14.path)), { equal: true, nodes: [4, 4], lines: [15, 15] })
  // T16's \relative, from a string ly:parser-include-string includes, runs
  // natively since MS2 step 5.
  const t16 = compareMusic(reference('T16'), engineMusic(rows.T16.path))
  assert.deepEqual(t16, { equal: true, nodes: [6, 6], lines: [23, 23] })
})

/**
 * P6's scores (02 §6), whose trees equal LilyPond's whole: T15's since
 * lily.scm's init data and output-lib.scm load (MS2 step 12), T08's, T09's
 * and T18's since the whole init (step 16), and since step 20 T01's and
 * T03's, whose \include the parse example reads from disk, and T11's,
 * T12's and T22's, whose \clef runs ly:regex-exec.
 */
const P6_EQUAL = ['T01', 'T03', 'T08', 'T09', 'T11', 'T12', 'T15', 'T18', 'T22']
const p6 = lilypond || (P6_EQUAL.every((id) => existsSync(rows[id].path)) ? false : 'the corpus is not fetched')

test("the engine's trees of P6's scores equal LilyPond's", { skip: p6 || cargo, timeout: 600_000 }, async () => {
  const ref = path.join(scratch, 'p6')
  await capture({ parse: true, ids: P6_EQUAL, out: ref })
  const compare = (id) => compareMusic(readFileSync(path.join(ref, id, 'music.txt'), 'utf8'), engineMusic(rows[id].path))
  for (const id of P6_EQUAL) {
    const result = compare(id)
    assert.equal(result.equal, true, `${id}: ${JSON.stringify(result)}`)
  }
})

/**
 * P10's scores (02 §6), whose trees equal LilyPond's whole since MS2 step
 * 21: T05's chords, made by the chords part's construct-chord-elements, and
 * T06's and T21's figures, T21's after its \include and ly:set-option.
 */
const P10_EQUAL = ['T05', 'T06', 'T21']
const p10 = lilypond || (P10_EQUAL.every((id) => existsSync(rows[id].path)) ? false : 'the corpus is not fetched')

test("the engine's trees of P10's scores equal LilyPond's", { skip: p10 || cargo, timeout: 600_000 }, async () => {
  const ref = path.join(scratch, 'p10')
  await capture({ parse: true, ids: P10_EQUAL, out: ref })
  for (const id of P10_EQUAL) {
    const result = compareMusic(readFileSync(path.join(ref, id, 'music.txt'), 'utf8'), engineMusic(rows[id].path))
    assert.equal(result.equal, true, `${id}: ${JSON.stringify(result)}`)
  }
})

/**
 * P8's scores (02 §6), whose trees equal LilyPond's whole since MS2 step
 * 32: T20's \lyricmode variables and four \addlyrics after \relative's
 * music, and the \lyricsto of T02, T04 and T30, T30's strings placed at
 * their closing quotes. T23's \lyricsto equal too; its tree is P11's.
 */
const P8_EQUAL = ['T20', 'T02', 'T04', 'T30']
const p8 = lilypond || (P8_EQUAL.every((id) => existsSync(rows[id].path)) ? false : 'the corpus is not fetched')

test("the engine's trees of P8's scores equal LilyPond's", { skip: p8 || cargo, timeout: 600_000 }, async () => {
  const ref = path.join(scratch, 'p8')
  await capture({ parse: true, ids: P8_EQUAL, out: ref })
  for (const id of P8_EQUAL) {
    const result = compareMusic(readFileSync(path.join(ref, id, 'music.txt'), 'utf8'), engineMusic(rows[id].path))
    assert.equal(result.equal, true, `${id}: ${JSON.stringify(result)}`)
  }
})

/**
 * P11's scores (02 §6), whose trees equal LilyPond's whole since MS2 step
 * 33: T23's \MakeSlash, a #{ … #} template with closures in markup and a
 * stencil, and T28's cary-layout.ily, a template a function; since step 34,
 * T26, whose \noHorzSpace is a \tweak cut short by \etc.
 */
const P11_EQUAL = ['T23', 'T28', 'T26']
const p11 = lilypond || (P11_EQUAL.every((id) => existsSync(rows[id].path)) ? false : 'the corpus is not fetched')

test("the engine's trees of P11's scores equal LilyPond's", { skip: p11 || cargo, timeout: 600_000 }, async () => {
  const ref = path.join(scratch, 'p11')
  await capture({ parse: true, ids: P11_EQUAL, out: ref })
  for (const id of P11_EQUAL) {
    const result = compareMusic(readFileSync(path.join(ref, id, 'music.txt'), 'utf8'), engineMusic(rows[id].path))
    assert.equal(result.equal, true, `${id}: ${JSON.stringify(result)}`)
  }
})
