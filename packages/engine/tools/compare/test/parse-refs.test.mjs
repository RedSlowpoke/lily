// P7's and P9's references (MS2 steps 14 and 20): the token sequences and
// music trees of the scores of 02 §6's P7 row, T13 (markup-syntax.ly) and
// T27 (bach-schenker.ly), of its P9 row's T17
// (optimal-page-breaking-hstretch.ly), T19 (midi/articulate-repeat.ly,
// after \articulate) and T29 (orchestra.ly, with the diagrams part's
// \harp-pedal), and of T02 (the extension's score.ly), as
// crates/parser/examples/parse.rs gives them, against LilyPond 2.26.0's
// (capture.mjs --parse); and the token sequences of P7's and P9's
// constructs in inputs of their own (crates/parser/tests/p7_markup.rs and
// p9_repeats.rs have their music and errors). The scores are parsed after
// the whole init (step 16), their includes read from disk (step 20).
// Skipped, and so unverified, without LilyPond 2.26.0, the corpus
// (tools/corpus/fetch.mjs) or cargo.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, before, test } from 'node:test'
import { corpus } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { capture, lilypondVersion } from '../capture.mjs'
import { compareMusic } from '../music.mjs'
import { compareTokens, lexed, readTokens, traceTokens } from '../tokens.mjs'

const lilypond = lilypondVersion() === '2.26.0' ? false : 'LilyPond 2.26.0 is not installed'
const cargo = spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'
const rows = Object.fromEntries(corpus().map((row) => [row.id, row]))

/** The scores whose tokens and trees are compared, each equal whole: T27's
 * tree since \clef runs ly:regex-exec (step 20). */
const IDS = ['T13', 'T27', 'T17', 'T19', 'T29', 'T02']
const ids = IDS
const skip = lilypond || cargo || (ids.every((id) => existsSync(rows[id].path)) ? false : 'the corpus is not fetched')

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-parse-refs-'))
after(() => rmSync(scratch, { recursive: true, force: true }))
const reference = (id, file) => readFileSync(path.join(scratch, 'ref', id, file), 'utf8')
before(async () => {
  if (!skip) await capture({ parse: true, ids, out: path.join(scratch, 'ref') })
})

/** The engine's tokens and trees of `file`, parsed from its directory. */
function parsed(file) {
  const out = mkdtempSync(path.join(scratch, 'engine-'))
  const run = runExample('parse', ['--tokens', out, '--music', out, path.basename(file)],
  { cwd: path.dirname(file), encoding: 'utf8' })
  // 1: the file failed, as LilyPond's may; 2: the runner could not run.
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
  const name = path.basename(file, '.ly')
  return {
    tokens: readTokens(readFileSync(path.join(out, `${name}.tokens`), 'utf8')),
    music: readFileSync(path.join(out, `${name}.music`), 'utf8'),
  }
}

test("the engine's tokens of P7's and P9's scores and T02 equal LilyPond's", { skip, timeout: 600_000 }, () => {
  for (const id of ids) {
    const want = lexed(readTokens(reference(id, 'tokens.txt')))
    const got = lexed(parsed(rows[id].path).tokens)
    assert.deepEqual(got, want, id)
  }
})

/** Each TextScriptEvent of a tree with its properties, dedented. */
function textScripts(tree) {
  const lines = tree.split('\n')
  const events = []
  for (let i = 0; i < lines.length; i++) {
    const node = /^( *)TextScriptEvent @/.exec(lines[i])
    if (!node) continue
    const block = [lines[i].trim()]
    while (i + 1 < lines.length && lines[i + 1].startsWith(`${node[1]} `)) block.push(lines[++i].slice(node[1].length))
    events.push(block.join('\n'))
  }
  return events
}

test("the engine's trees of P7's and P9's scores and T02 equal LilyPond's", { skip, timeout: 600_000 }, () => {
  for (const id of ids) {
    const want = reference(id, 'music.txt')
    const got = parsed(rows[id].path).music
    const result = compareMusic(want, got)
    assert.ok(result.equal, `${id}: ${JSON.stringify(result)}`)
  }
  // P7's: every text script's markup, origin and direction, in the file's
  // order, by themselves.
  for (const id of ['T13', 'T27']) {
    const scripts = textScripts(reference(id, 'music.txt'))
    assert.ok(scripts.length > 0, id)
    assert.deepEqual(textScripts(parsed(rows[id].path).music), scripts, id)
  }
})

/**
 * Inputs of P7's constructs: markup in scripts, heads over markups and
 * lists, Scheme markup and markup lists as variables, markup list commands,
 * markup as a value, a scalar and an argument, LilyPond in braces as an
 * argument, \markup name = …, toplevel markup with \score in it, a part's
 * command, and the errors of crates/parser/tests/p7_markup.rs that end
 * where LilyPond's parse ends: an unknown command (case 19's), arguments
 * of the wrong type (case 20's Guile error), and syntax errors, after which
 * the lexer reads on in markup mode, as no rule pops it.
 */
const P7_INPUTS = [
  String.raw`{ c4^\markup \bold x d4_\markup { a \italic b } e4-\markup \column { a b } f4^"s" g4-w a4^#(markup #:italic "y") }`,
  String.raw`{ c4^\markup \bold { a \italic b \line { c d } } d4^\markup \raise #0.5 \hbracket \bold \italic e
  e4^\markup \override #'(baseline-skip . 2) \column { a b } f4^\markup \combine "X" \bold "+" }`,
  String.raw`m = \markup \bold x
l = \markuplist { p \italic q }
{ c4^\markup { \m #(markup #:italic "y") #"z" #(if #f #f) \l } d4^\markup \column \l e4^\m }`,
  String.raw`{ c4^\markup \column { \wordwrap-lines { a b } c } d4^\markup \bold \wordwrap-lines { e f } }`,
  String.raw`t = \markup \italic t
{ \set Staff.instrumentName = \markup \bold Hi c4^\t \mark \markup \box A d4 }`,
  String.raw`{ c4^\markup \note {4.} #UP d4^\markup \note-by-number #2 #0 #UP }`,
  String.raw`\markup bolder = #bold-markup
{ c4^\markup \bolder x }`,
  String.raw`\markup \bold Title
\markuplist { a b }
\markup \score { { c4 } }
{ d4 }`,
  String.raw`{ c4^\markup \fret-diagram "6-x;5-x;4-o;3-2;2-3;1-2;" }`,
  String.raw`\markup { \boldd text }`,
  String.raw`\markup \draw-circle #"a" #"b" ##f`,
  String.raw`\markup \bold }`,
  String.raw`\markuplist \bold x`,
  String.raw`\markup { a \markuplist b }`,
  String.raw`\markup \combine { a } b`,
]

test("the engine's tokens of P7's constructs equal LilyPond's", { skip: lilypond || cargo, timeout: 600_000 }, () => {
  const dir = mkdtempSync(path.join(scratch, 'p7-'))
  const files = P7_INPUTS.map((text, i) => {
    const file = path.join(dir, `p${i + 1}.ly`)
    writeFileSync(file, `\\version "2.26.0"\n${text}\n`)
    return file
  })
  const out = mkdtempSync(path.join(scratch, 'engine-'))
  const run = runExample('parse', ['--tokens', out, ...files.map((f) => path.basename(f))],
  { cwd: dir, encoding: 'utf8' })
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
  for (const file of files) {
    const traced = spawnSync('lilypond', ['--loglevel=WARNING', '-ddebug-parser', '-dno-print-pages', path.basename(file)],
      { cwd: dir, encoding: 'utf8', env: { ...process.env, LANGUAGE: 'en' } })
    const got = readTokens(readFileSync(path.join(out, `${path.basename(file, '.ly')}.tokens`), 'utf8'))
    const result = compareTokens(traceTokens(traced.stderr), got)
    assert.ok(result.equal, `${readFileSync(file, 'utf8')}\n${JSON.stringify(result)}`)
  }
})

/**
 * Inputs of P9's constructs (crates/parser/tests/p9_repeats.rs has their
 * music and errors): each repeat type with \alternative after it and in
 * its body, counts and types of every kind, \tempo with a text, a mark,
 * both, a range and markup, \sequential and \simultaneous after \notemode
 * too, repeats and tempos as values, in contexts, as arguments and in
 * \midi, P8's --, __ and \lyricsto (brought forward for T02), and the
 * syntax errors of p9_repeats.rs.
 */
const P9_INPUTS = [
  String.raw`{ \repeat unfold 2 { c4 d } \repeat volta 2 { e4 } \alternative { { f4 } { g4 } } \repeat tremolo 4 { c16 d }
  \repeat percent 2 { c4 } \repeat segno 2 c4 \repeat volta 3 { c4 \alternative { { d4 } { e4 } } } }`,
  String.raw`n = 3
t = "unfold"
{ \repeat unfold \n c4 \repeat unfold #2 d4 \repeat "unfold" 2 f4 \repeat #"volta" 2 g4 \repeat \t 2 a4 \repeat unfold 2 <c e>4 }`,
  String.raw`{ \tempo 4 = 96 c4 \tempo "Allegro" d4 \tempo "Andante" 8. = 60 e4 \tempo \markup \bold Presto 2 = 40-50 f4
  \tempo #"Lento" g4 \tempo Adagio a4 \tempo 4 = #(+ 1 2) b4 }`,
  String.raw`\sequential { c4 d } \simultaneous { e4 g }
{ \notemode \sequential { c4 } \notemode \simultaneous { d4 e } \notemode << f4 >> }`,
  String.raw`rep = \repeat unfold 2 c4
tem = \tempo 4 = 60
alt = \alternative { d4 e }
\score { { \rep \tem \repeat volta 2 c4 \alt } \midi { \tempo 2 = 30 } \layout { } }
\new Staff \repeat unfold 2 c4
\relative c' \repeat unfold 2 { c d }`,
  String.raw`words = \lyricmode { a -- b __ c d4 e2. }
<< \new Voice = "v" { c4 d e f g } \new Lyrics \lyricsto "v" { \words } >>`,
  String.raw`{ \repeat unfold 2 }`,
  String.raw`{ \repeat unfold 2 #(make-music (quote NoteEvent)) }`,
  String.raw`{ \sequential c4 }`,
  String.raw`{ \tempo }`,
  String.raw`{ \alternative c4 }`,
  String.raw`{ \tempo #5 c4 \repeat #5 2 c4 \repeat unfold #-1 c4 }`,
]

test("the engine's tokens of P9's constructs equal LilyPond's", { skip: lilypond || cargo, timeout: 600_000 }, () => {
  const dir = mkdtempSync(path.join(scratch, 'p9-'))
  const files = P9_INPUTS.map((text, i) => {
    const file = path.join(dir, `p${i + 1}.ly`)
    writeFileSync(file, `\\version "2.26.0"\n${text}\n`)
    return file
  })
  const out = mkdtempSync(path.join(scratch, 'engine-'))
  const run = runExample('parse', ['--tokens', out, ...files.map((f) => path.basename(f))],
  { cwd: dir, encoding: 'utf8' })
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
  for (const file of files) {
    const traced = spawnSync('lilypond', ['--loglevel=WARNING', '-ddebug-parser', '-dno-print-pages', path.basename(file)],
      { cwd: dir, encoding: 'utf8', env: { ...process.env, LANGUAGE: 'en' } })
    const got = readTokens(readFileSync(path.join(out, `${path.basename(file, '.ly')}.tokens`), 'utf8'))
    const result = compareTokens(traceTokens(traced.stderr), got)
    assert.ok(result.equal, `${readFileSync(file, 'utf8')}\n${JSON.stringify(result)}`)
  }
})
