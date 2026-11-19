// Token sequences (tokens.mjs, capture.mjs --parse): the reading of Bison's
// -ddebug-parser trace, the compare's self-tests (a sequence against itself
// passes, a perturbed copy fails), and the engine's parse example against
// LilyPond 2.26.0's sequences: T14's and T16's (through
// ly:parser-include-string) equal, and music-functions-init.ly's as the
// init's parse reads it; P6's contexts (MS2 step 6) and property operations
// (step 7) in inputs of their own, equal, and in P6's scores, equal but for
// the \names of the init's part-2 files; P10's scores (T05, T06, T21) and
// its chord, drum and figure modes in inputs of their own (step 21), equal;
// P8's score (T20) and its lyrics in inputs of their own (step 32), equal;
// P11's scores (T23, T28) and #{ … #} templates in inputs of their own,
// their parses indented where they run (step 33), equal; T26 and
// packages/common's syntax.ly, the deutsch.ly it includes among it and
// alone (step 34), equal; and syntax.ly's music trees, its \book's scores
// among them (MS8 step 10, OPEN F), equal.
// Compared as the lexer gave them (tokens.mjs).
// The tests on LilyPond's captures skip themselves, and so leave the tools
// unverified, without LilyPond 2.26.0 or the corpus (tools/corpus/fetch.mjs);
// those on the engine also without cargo.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, before, test } from 'node:test'
import { corpus } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { capture, lilypondVersion, SYNTAX } from '../capture.mjs'
import { compareMusic } from '../music.mjs'
import { byFile, compareTokens, lexed, readTokens, tokenName, traceTokens } from '../tokens.mjs'

test('token names are unquoted as yytnamerr unquotes them', () => {
  assert.equal(tokenName('"\\\\header"'), '\\header')
  assert.equal(tokenName('"\\\\\\\\"'), '\\\\')
  assert.equal(tokenName('"end of input"'), 'end of input')
  assert.equal(tokenName('"(reparsed?)"'), '(reparsed?)')
  assert.equal(tokenName("'{'"), "'{'")
  assert.equal(tokenName("'\\''"), "'\\''")
  assert.equal(tokenName('NOTENAME_PITCH'), 'NOTENAME_PITCH')
})

// A trace as Bison writes it, cut down: the first token read before the
// trace was on, a lookahead looked at again, a reparse that pushes the
// lookahead back, a nested parse, marks, and init.ly's tail after the end.
const TRACE = `Parsing...Next token is token "\\\\header" (: )
Reducing stack by rule 65 (line 714):
Next token is token "\\\\header" (: )
Shifting token "\\\\header" (: )
Reading a token
Next token is token MUSIC_FUNCTION (: )
Shifting token MUSIC_FUNCTION (: )
Reading a token
Next token is token "scheme?" (: )
Shifting token "scheme?" (: )
Reading a token
Next token is token STRING (: )
Shifting token STRING (: )
Reading a token
Next token is token NOTENAME_PITCH (: )
Reducing stack by rule 300 (line 2000):
Reading a token
Next token is token "(reparsed?)" (: )
Shifting token "(reparsed?)" (: )
Reading a token
Next token is token SCM_ARG (: )
Shifting token SCM_ARG (: )
Reading a token
Next token is token NOTENAME_PITCH (: )
Shifting token NOTENAME_PITCH (: )

@@ begin x.ly
Reading a token
Next token is token SCM_TOKEN (: )
Starting parse
Entering state 0
Reading a token
Next token is token "#{" (: )
Shifting token "#{" (: )
Reading a token
Now at end of input.
Cleanup: popping token "end of input" (: )
Cleanup: popping nterm start_symbol (: )
Shifting token SCM_TOKEN (: )

@@ end x.ly
Reading a token
Now at end of input.
Now at end of input.
Cleanup: popping nterm start_symbol (: )
Starting parse
Reading a token
Next token is token SCM_TOKEN (: )
`

test('the tokens read are taken from the trace, pushes marked and nested parses indented', () => {
  const tokens = traceTokens(TRACE)
  assert.deepEqual(tokens, ['\\header', 'MUSIC_FUNCTION', 'scheme?', 'STRING', 'NOTENAME_PITCH',
    '(reparsed?)', '^SCM_ARG', '^NOTENAME_PITCH', '@@ begin x.ly', 'SCM_TOKEN', '  #{', '  end of input',
    '@@ end x.ly', 'end of input'])
  assert.deepEqual(lexed(tokens.filter((t) => !t.startsWith('@@'))), ['\\header', 'MUSIC_FUNCTION', 'scheme?', 'STRING',
    'NOTENAME_PITCH', 'SCM_TOKEN', '  #{', '  end of input', 'end of input'])
  assert.deepEqual([...byFile(tokens)], [['x.ly', ['SCM_TOKEN', '  #{', '  end of input']]])
})

/** The self-tests of the compare on `tokens`, a sequence of at least two lexed tokens. */
function selfTests(tokens) {
  assert.deepEqual(compareTokens(tokens, tokens), { equal: true, lengths: [lexed(tokens).length, lexed(tokens).length] })
  assert.ok(compareTokens(tokens, tokens, { full: true }).equal)
  const at = lexed(tokens).length >> 1
  const changed = lexed(tokens).with(at, 'NOT_A_TOKEN')
  const result = compareTokens(tokens, changed)
  assert.equal(result.equal, false)
  assert.equal(result.at, at)
  assert.equal(result.got, 'NOT_A_TOKEN')
  assert.equal(compareTokens(tokens, lexed(tokens).toSpliced(at, 1)).equal, false)
  assert.equal(compareTokens(tokens, [...lexed(tokens), 'end of input']).equal, false)
}

test('a sequence equals itself, and a perturbed copy differs', () => {
  selfTests(traceTokens(TRACE).filter((t) => !t.startsWith('@@')))
  // The parser's pushes count only when the whole sequence is compared.
  const tokens = ['STRING', '(reparsed?)', '^SCM_ARG', 'NOTENAME_PITCH']
  assert.ok(compareTokens(tokens, ['STRING', 'NOTENAME_PITCH']).equal)
  assert.equal(compareTokens(tokens, ['STRING', 'NOTENAME_PITCH'], { full: true }).equal, false)
})

const lilypond = lilypondVersion() === '2.26.0' ? false : 'LilyPond 2.26.0 is not installed'
const rows = Object.fromEntries(corpus().map((row) => [row.id, row]))
/**
 * P6's scores (02 §6) with contexts or properties. Since the whole init
 * (MS2 step 16) every \name they use is defined, as LilyPond's init
 * defines it, so their tokens equal LilyPond's; T01's and T03's since the
 * parse example reads their \include from disk (step 20), the included
 * file's tokens among them.
 */
const P6_SCORES = ['T01', 'T03', 'T08', 'T09', 'T11', 'T12', 'T15', 'T18', 'T22']
/** P10's scores (02 §6): chord mode (T05), figures (T06, T21). */
const P10_SCORES = ['T05', 'T06', 'T21']
/** P8's score (02 §6): T20's lyrics, words of four scripts, and its four \addlyrics. */
const P8_SCORES = ['T20']
/**
 * P11's scores (02 §6): T23's template in a function with markup, T28's
 * cary-layout.ily, and T26's \etc (MS2 step 34).
 */
const P11_SCORES = ['T23', 'T28', 'T26']
const fetched = ['T14', 'T16', ...P6_SCORES, ...P10_SCORES, ...P8_SCORES, ...P11_SCORES].every((id) => existsSync(rows[id].path)) ? false : 'the corpus is not fetched'
const skip = lilypond || fetched
const cargo = spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-tokens-'))
after(() => rmSync(scratch, { recursive: true, force: true }))
const reference = (id) => readTokens(readFileSync(path.join(scratch, 'ref', id, 'tokens.txt'), 'utf8'))
before(async () => {
  if (!skip) {
    await capture({ parse: true, ids: ['T14', 'T16', ...P6_SCORES, ...P10_SCORES, ...P8_SCORES, ...P11_SCORES, 'syntax', 'init'], out: path.join(scratch, 'ref') })
  }
})

test("LilyPond's sequences equal themselves, and perturbed copies differ", { skip }, () => {
  for (const id of ['T14', 'T16', 'init/music-functions-init.ly', 'init']) selfTests(reference(id))
  // T14 calls \myBar with a string, which Bison reparses as SCM_ARG.
  assert.deepEqual(reference('T14').slice(18, 25), ['MUSIC_FUNCTION', 'scheme?', 'EXPECT_NO_MORE_ARGS', 'STRING',
    '(reparsed?)', '^SCM_ARG', 'NOTENAME_PITCH'])
  // declarations-init.ly and the 21 files it includes.
  assert.equal(readTokens(readFileSync(path.join(scratch, 'ref/init/declarations-init.ly/tokens.txt'), 'utf8')).length,
    reference('init').length)
})

/** The engine's sequences of `files`, parsed by crates/parser/examples/parse.rs from their directories. */
function engineTokens(files) {
  const out = mkdtempSync(path.join(scratch, 'engine-'))
  for (const file of files) {
    const run = runExample('parse', ['--tokens', out, file], { cwd: path.dirname(file), encoding: 'utf8' })
    // 1: the file failed, as LilyPond's may; 2: the runner could not run.
    assert.ok(run.status === 0 || run.status === 1, run.stderr)
  }
  return files.map((file) => readTokens(readFileSync(path.join(out, `${path.basename(file, '.ly')}.tokens`), 'utf8')))
}

test("the engine's tokens of T14 and T16 equal LilyPond's", { skip: skip || cargo }, () => {
  const [t14, t16] = engineTokens([rows.T14.path, rows.T16.path])
  assert.deepEqual(compareTokens(reference('T14'), t14), { equal: true, lengths: [26, 26] })
  // The included string's tokens come after the form that includes it.
  assert.deepEqual(compareTokens(reference('T16'), t16), { equal: true, lengths: [20, 20] })
})

test("the engine's tokens of music-functions-init.ly equal those of LilyPond's init parse", { skip: skip || cargo }, () => {
  const ly = path.dirname(spawnSync('sh', ['-c', 'cd "$(dirname "$(realpath "$(command -v lilypond)")")/../share/lilypond/2.26.0/ly" && pwd'],
    { encoding: 'utf8' }).stdout.trim() + '/x')
  const [tokens] = engineTokens([path.join(ly, 'music-functions-init.ly')])
  // An included file ends without a token of its own.
  assert.equal(tokens.pop(), 'end of input')
  const result = compareTokens(reference('init/music-functions-init.ly'), tokens)
  assert.ok(result.equal, JSON.stringify(result))
})

/**
 * Inputs of P6's contexts (crates/parser/tests/p6_contexts.rs has their
 * music and errors): every prefix, `= id`, `\with` in braces, as a variable,
 * a `#` and a `$` form and an argument, every mod's keyword, `\context { … }`
 * in `\layout` and as a value, `\change`, and syntax errors in each; and of
 * P6's property operations (crates/parser/tests/p6_properties.rs): each in
 * music, `\once` and `\temporary` of them, `\revert` of a grob, of a context
 * and of Scheme, whose context pushes back an SCM_IDENTIFIER, split paths,
 * values, `\with`'s and `\context { … }`'s operations, and the recovery
 * from an unknown command in a definition.
 */
const P6_INPUTS = [
  String.raw`m = \with { \consists "Foo" \remove Bar_engraver \accepts Baz \denies "Q" \alias Staff \defaultchild Voice
  \type "Engraver_group" \name Who \description "d" \consists #'x }
\new Staff = "one" \with { \consists "Foo" } \with \m { c4 \change Staff = two d }
\context Voice c4
\new Staff \new Voice = x { c4 }`,
  String.raw`\new Staff r4 \new Staff R1 \new Staff <c e>4 \new Staff \new Voice c4-1 \context Staff = "a" \with { } d`,
  String.raw`{ \new Voice { c4 } -1 c4 }`,
  String.raw`x = \new Staff { c4 }
{ \x }`,
  String.raw`\new Staff = $"a" \with $(ly:make-context-mod) { c4 }
\new #'Staff \with { #(make-music 'NoteEvent) } { c4 }`,
  String.raw`fn = #(define-music-function (m x) (ly:context-mod? ly:music?) (context-spec-music x 'Staff #f m))
{ \fn \with { \consists "A" } c4 }`,
  String.raw`m = \with { \consists "A" }
dd = \context { \consists "x" \m }
\layout { \context { \dd \remove "y" \with \m } \context { } }
\new Staff \with \m { c4 }`,
  String.raw`\new Staff \with 5 { c4 }`,
  String.raw`\new Staff \with { 5 } { c4 }`,
  String.raw`\new Staff \with { \consists } { c4 }`,
  String.raw`\new Staff = { c4 }`,
  String.raw`\layout { \context { \with \with { } } }`,
  String.raw`\layout { \notemode c } { c4 }`,
  String.raw`{ \override Staff.Stem.color = #red \revert Stem.color \revert Staff.Stem.color \revert #'(Staff Stem color)
  \set Staff.instrumentName = "x" \unset Staff.instrumentName \set fontSize = -2 \unset fontSize c4 }`,
  String.raw`{ \once \override Stem.color = #red \temporary \override Beam.positions = #'(1 . 2) \once \revert Stem.color c4 }`,
  String.raw`{ \set Staff.instrumentName = Guitar \override Stem.length = 3.5 \set Staff.tonic = d'
  \set Staff.instrumentName = { c4 } \set Staff.instrumentName = Stem.color \set Staff.fontSize = $(+ 1 2) c4 }`,
  String.raw`{ \revert Staff #'color c4 }`,
  String.raw`{ \revert Stem.color.x \revert Staff.Stem #'color \override NoteHead #'color #'x = 1 c4 }`,
  String.raw`\new Staff \with { \override Stem #'color = #red \revert Stem.color instrumentName = "G" #'fontSize = 2
  \unset fontSize \override Stem = 1 } { c4 }`,
  String.raw`\layout { \context { \override Stem.color = #red \revert Beam.positions instrumentName = "x" } }`,
  String.raw`\layout { \context { \Foo \revert Arpeggio.stencil } }`,
  String.raw`myUp = \override Stem.direction = #UP
{ \myUp c4 \override Stem.color = - x }`,
]

test("the engine's tokens of P6's contexts equal LilyPond's", { skip: lilypond || cargo, timeout: 600_000 }, () => {
  const dir = mkdtempSync(path.join(scratch, 'p6-'))
  const files = P6_INPUTS.map((text, i) => {
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

test("the engine's tokens of P6's scores equal LilyPond's", { skip: skip || cargo }, () => {
  const sequences = engineTokens(P6_SCORES.map((id) => rows[id].path))
  for (const [i, id] of P6_SCORES.entries()) assert.deepEqual(lexed(sequences[i]), lexed(reference(id)), id)
})

test("the engine's tokens of P10's scores equal LilyPond's", { skip: skip || cargo }, () => {
  const sequences = engineTokens(P10_SCORES.map((id) => rows[id].path))
  for (const [i, id] of P10_SCORES.entries()) assert.deepEqual(lexed(sequences[i]), lexed(reference(id)), id)
})

/**
 * Inputs of P10's modes (crates/parser/tests/p10_modes.rs has their music
 * and errors): chords with durations, modifiers, steps, `^`, `/` and `/+`,
 * in `< >`, with post-events and bare; the heads with `\with`, in
 * contexts, under `\relative` and as values; drum notes and chords of them;
 * figures of every kind with brackets, alterations and modifications; and
 * the errors of p10_modes.rs, with a bracket before a bracketed alteration,
 * which is no part of it.
 */
const P10_INPUTS = [
  String.raw`\chordmode { c1 c:7 c:m7 c:maj7.9+ c:dim7 c:aug c:sus2 c:sus4.9 c:13^11 c:9^3.5
  c/e c/+d c:7/g c4.:m d e:7-.9+ f' g,8 c:5.3- c:1.5 c:m/+bes, }`,
  String.raw`\chordmode { <c e g>4 <c' e>8. c_"t" d:m_"u" e2_\markup \bold x f q r s R1 }`,
  String.raw`\chords { c1 g:7 }
\new Staff \chordmode { c d }
\chords \with { \override ChordName.color = #red } { c2 }
\relative c' \chordmode { c e } \transpose c d \chordmode { c:m }
\chordmode \sequential { c } \chordmode << c e >>`,
  String.raw`\drummode { bd4 sn8 hh hh4-> <bd sn>4 <bd-> hh>8-. r4 s4 bd4.:16 tomh tommh8 }
\drums { bd4 sn } \new DrumStaff \drummode { cymc1 }
\drums \with { \consists "Pitch_squash_engraver" } { bd4 }`,
  String.raw`\figuremode { <6>4 <6 4>8 <_+> <5+ [3-]> <"a" \markup \bold b 7\+ 6\! 5/ 4\\>
  <[3 5]> <3 _ 5> <#7 #"x"> s4 <6>4^"t" <6+! 5--> <6[+]> <3 [5 7]> <_!> }
\figures { <6>2 } \new FiguredBass \figuremode { <5>1 }`,
  String.raw`ch = \chordmode { c:m }
fig = \figuremode { <6> }
dr = \drummode { bd }
{ \ch \fig \dr }
\chordmode { \notemode { c4 } d \ch }`,
  String.raw`\figuremode { <#(- 1) 5> }`,
  String.raw`\figuremode { <6+\+ -> }`,
  String.raw`\figuremode { <6[[+]]> }`,
  String.raw`\chordmode { c:foo }`,
  String.raw`\drummode { foo4 }`,
  String.raw`\chordmode { c/+e/f }`,
  String.raw`\chordmode { c:7/ }`,
  String.raw`\figures c`,
  String.raw`\figures \with { } c`,
]

test("the engine's tokens of P10's modes equal LilyPond's", { skip: lilypond || cargo, timeout: 600_000 }, () => {
  const dir = mkdtempSync(path.join(scratch, 'p10-'))
  const files = P10_INPUTS.map((text, i) => {
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

test("the engine's tokens of P8's score equal LilyPond's", { skip: skip || cargo }, () => {
  const sequences = engineTokens(P8_SCORES.map((id) => rows[id].path))
  for (const [i, id] of P8_SCORES.entries()) assert.deepEqual(lexed(sequences[i]), lexed(reference(id)), id)
})

/**
 * Inputs of P8's lyrics (crates/parser/tests/p8_lyrics.rs has their music
 * and errors): words with durations, dots and multipliers, `--` and `__`,
 * `_`, strings, markup, words of non-ASCII letters, and digits and dots
 * as lyrics mode splits them; `\lyrics` and `\lyricmode` with each grouped
 * list; `\addlyrics` and `\lyricsto` in their forms; music and function
 * arguments in lyrics; `<<` and `>>` alone and in a word, `\"…"`,
 * `\version` and a `REAL`; and the errors of p8_lyrics.rs.
 */
const P8_INPUTS = [
  String.raw`\lyricmode { la4 -- li8. __ lu2*2/3 lo "quoted word" "d'un"8 \markup \bold bold _ a_b c1. Hello, world. 1st x.y end.4 à vo -- cê Жълтата いろはにほへど }`,
  String.raw`\lyrics { a b } \lyrics \with { \override LyricText.color = #red } { c } \new Lyrics \lyricmode { d }
\lyricmode \sequential { e } \lyricmode << f g >> \lyricmode \simultaneous { h i }`,
  String.raw`words = \lyricmode { q r }
mel = { c4 d }
{ c4 d } \addlyrics { a b }
\new Voice = "v" { c4 } \addlyrics { x } \addlyrics \with { \override LyricText.color = #red } { y }
\new Staff \new Voice { c4 } \addlyrics { z } \context Voice { c4 } \addlyrics { w }
\relative c' { c4 d } \addlyrics { a b } \addlyrics \words \mel \addlyrics \words
\new Voice c4 \addlyrics << { x y } >> \new Staff { \new Voice = "" { c4 } \addlyrics \sequential { m } }`,
  String.raw`words = \lyricmode { q r }
<< \new Voice = "v" { c4 d } \new Lyrics \lyricsto "v" { a b } \new Lyrics \lyricsto "v" \words
\new Lyrics \lyricsto v << { a b } >> \new Lyrics \lyricsto #"v" \sequential { c } >>`,
  String.raw`\lyricmode { \set stanza = "1." a4 \skip 4 b \override LyricText.color = #red c \once \override LyricText.font-size = #2 d
  { f g } << h i >> #"scm" #(markup #:bold "m") 8 j -- \markup { k l } \"skip" 4 m }`,
  String.raw`\lyricmode { \tag #'x e4 -- \tag #'y \markup \italic m2 \tag #'z "q" \tag #'w word_with_under 8. __ }`,
  String.raw`\lyricmode { << a b >> <<c d>> x<< >>y \version "2.26.0" a2.. b4 }`,
  String.raw`{ c4 } \addlyrics x`,
  String.raw`\lyricsto "v" x`,
  String.raw`\lyricmode x`,
  String.raw`\lyrics \with { } x`,
  String.raw`\lyricmode { a 1.5 b }`,
  String.raw`\lyricmode { a #'sym b }`,
  String.raw`{ c4 "s" d \markup x e }`,
  String.raw`\lyricmode { a4 b8 \addlyrics { c } }`,
  String.raw`\new Voice { c4 } \addlyrics { a } \addlyrics`,
]

test("the engine's tokens of P8's lyrics equal LilyPond's", { skip: lilypond || cargo, timeout: 600_000 }, () => {
  const dir = mkdtempSync(path.join(scratch, 'p8-'))
  const files = P8_INPUTS.map((text, i) => {
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

test("the engine's tokens of P11's scores equal LilyPond's", { skip: skip || cargo }, () => {
  const sequences = engineTokens(P11_SCORES.map((id) => rows[id].path))
  for (const [i, id] of P11_SCORES.entries()) assert.deepEqual(lexed(sequences[i]), lexed(reference(id)), id)
})

/**
 * Inputs of P11's templates (crates/parser/tests/p11_templates.rs has their
 * music, values and errors): templates in functions, nested, with closures
 * by byte and `$@`, in `$` and `#` forms at toplevel and in music, a
 * function after a call that runs one (its pushed tokens read after the
 * template's), and a template's errors.
 */
const P11_INPUTS = [
  String.raw`fn = #(define-music-function (m) (ly:music?) #{ \tuplet 3/2 { $m $m } #})
mk = #(define-music-function (p) (ly:pitch?) #{ $p 4. -. #})
nested = #(define-music-function (m) (ly:music?) #{ \fn { #m #(make-music 'RestEvent 'duration (ly:make-duration 3)) } #})
{ \fn c'8 \mk d' \nested e'16 \fn c \fn d }`,
  String.raw`#(define-macro (pattern args result)
  ${'`'}(define-music-function ,args ,(make-list (length args) 'ly:music?) #{ $@(list ,@result) #}))
swap = #(pattern (A B) (B A))
{ \swap { a' } { b' } #(let ((x #{ c4 #})) #{ \relative { #x d e } #}) }`,
  String.raw`{ c8 $#{ d e #} f }
#(define x #{ 4. #})
#(define y #{ \markup \bold x #})`,
  String.raw`m = #(define-music-function () () #{ c4 } #})
{ \m }
#(define z #{ c4 d \bar #})`,
]

test("the engine's tokens of P11's templates equal LilyPond's", { skip: lilypond || cargo, timeout: 600_000 }, () => {
  const dir = mkdtempSync(path.join(scratch, 'p11-'))
  const files = P11_INPUTS.map((text, i) => {
    const file = path.join(dir, `t${i + 1}.ly`)
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

test("the engine's music trees of syntax.ly, its \\book's scores among them, equal LilyPond's", { skip: skip || cargo }, () => {
  // OPEN F (MS8 step 10): capture.mjs --parse writes the trees music.ly's book handler gets, a \book's too.
  const out = mkdtempSync(path.join(scratch, 'engine-'))
  const run = runExample('parse', ['--music', out, path.basename(SYNTAX)], { cwd: path.dirname(SYNTAX), encoding: 'utf8' })
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
  const want = readFileSync(path.join(scratch, 'ref', 'syntax', 'music.txt'), 'utf8')
  const result = compareMusic(want, readFileSync(path.join(out, 'syntax.music'), 'utf8'))
  assert.ok(result.equal && result.nodes[0] > 0, JSON.stringify(result))
  assert.equal((want.match(/^score /gm) ?? []).length, 3)
})

test("the engine's tokens of syntax.ly and of the deutsch.ly it includes equal LilyPond's", { skip: skip || cargo }, () => {
  const [syntax] = engineTokens([SYNTAX])
  const result = compareTokens(reference('syntax'), syntax)
  assert.ok(result.equal, JSON.stringify(result))
  // deutsch.ly alone, as LilyPond's trace of a copy reads it.
  const ly = spawnSync('sh', ['-c', 'cd "$(dirname "$(realpath "$(command -v lilypond)")")/../share/lilypond/2.26.0/ly" && pwd'],
    { encoding: 'utf8' }).stdout.trim()
  const dir = mkdtempSync(path.join(scratch, 'deutsch-'))
  writeFileSync(path.join(dir, 'deutsch.ly'), readFileSync(path.join(ly, 'deutsch.ly')))
  const traced = spawnSync('lilypond', ['--loglevel=WARNING', '-ddebug-parser', '-dno-print-pages', 'deutsch.ly'],
    { cwd: dir, encoding: 'utf8', env: { ...process.env, LANGUAGE: 'en' } })
  const [deutsch] = engineTokens([path.join(dir, 'deutsch.ly')])
  const alone = compareTokens(traceTokens(traced.stderr), deutsch)
  assert.ok(alone.equal && alone.lengths[0] > 0, JSON.stringify(alone))
})
