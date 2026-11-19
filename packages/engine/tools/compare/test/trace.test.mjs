// Translation traces (trace.ly, trace.mjs; MS2 step 22, 04 Appendix A):
// the compare's self-tests (a trace against itself passes; a moved event, a
// renamed context, a changed property, a context made in another order
// fail; a property left for later is reported by name), T14's trace as
// LilyPond 2.26.0 writes it, and the 30 scores traced twice, each equal to
// itself (part 3's 14 until step 35). Since MS2 step 24, the engine's traces of the 14
// (crates/core/examples/compile.rs --trace) equal LilyPond's: contexts,
// events and the properties at the bar starts, which the engine's
// Timing_translator finds since step 25, with its timing properties, but
// for the properties translators of later steps set, named; so do those of
// fixtures/iterators.ly, which has the iterators the 14 do not reach.
// Since step 27 so do the traces of the 14 compiled one after another in
// one instance of the built core.wasm (core.mjs), as the studio compiles.
// Since step 35 (N6) so do those of the grace scores T09, T12 and T22
// (trace.mjs' TRACED), and of fixtures/grace.ly, whose \applyContext calls
// write the graceSettings Grace_engraver pushes and pops as LilyPond's.
// Since step 36 so do those of the \change scores T11, T25 (its grammar
// too), T26 and T27, their voices moved between staves as LilyPond moves
// them. Since step 37 (N7) so do those of the volta repeats' T07, T10 and
// T24, repeatCommands among the properties compared (the engine's
// Repeat_acknowledge_engraver), and of fixtures/volta.ly, whose repeats
// and alternatives the 30 do not reach; T10's and T15's expected warnings
// (ly:expect-warning) are not printed. Since step 38 (N7) so does T20's,
// its four \addlyrics combined with their voice as LilyPond combines them,
// and that of fixtures/lyrics.ly, whose lyrics meet rests, graces, a voice
// made later, associatedVoice, repeats and \change, with no melisma (the
// other lyric scores' traces differ from their first melisma on, step 39).
// Since step 39 (N7) so do those of T02, T04 and T30, their syllables held
// back by melismas as LilyPond holds them, and T28's, slurMelismaBusy and
// tieMelismaBusy compared in every trace (the engine's Slur_engraver and
// Tie_engraver set them, as its Beam_engraver sets beamMelismaBusy), and
// that of fixtures/melisma.ly, whose \midi map is LilyPond's too (the
// performers' melismas). T23's trace has the S lines of its incipits,
// \scores in its instrumentName markups, which LilyPond's layout translates
// when it prints them (D100): since MS3 step 43 the engine translates such
// a \score when its name prints, as LilyPond's ly:score-embedded-format
// does (the trace of fixtures/incipits.ly, T23's three incipits on a score
// without ties, equals LilyPond's natively and in core.wasm, their S lines
// after the score's X lines), and since MS4 step 10 (A26) T23's own layout,
// its ties ported, reaches its names: it is one of TRACED, printed with
// emmentaler-16 (its staff size, 15), which no manifest preloads.
// Since MS3 step 16 the engine compiles with the font packs the manifest
// preloads, as the studio's host puts them (D105): a dotted rest's dot
// column takes the rest's width from its glyph (T26).
// The tests on captures skip themselves, and so verify nothing, without
// LilyPond 2.26.0, the corpus (tools/corpus/fetch.mjs), cargo or a build's
// packs, and the last without a core.wasm as new as the sources.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { compileInWasm, coreSkip, preloaded } from '../core.mjs'
import { lilypondVersion } from '../capture.mjs'
import { key } from '../score.mjs'
import { SCORES, TRACED, captureTraces, compareTrace, readTrace, traceOne } from '../trace.mjs'

// T14's trace but its Score's S lines, as trace.ly writes it.
const T14 = `P\t1\tlayout
C\t0\t0\tScore#0=\\new\tGlobal#0
C\t0\t0\tStaff#0=\\new\tScore#0=\\new
C\t0\t0\tVoice#0\tStaff#0=\\new
E\t0\t0\tVoice#0\tnote-event\t#<Pitch d > #<Duration 4 > () () () () () () ()
S\t0\t0\tStaff#0=\\new\tcreateSpacing\t#t
S\t0\t0\tStaff#0=\\new\thasAxisGroup\t#t
S\t0\t0\tStaff#0=\\new\tignoreFiguredBassRest\t#f
S\t0\t0\tStaff#0=\\new\tinstrumentName\t()
S\t0\t0\tStaff#0=\\new\tkeyAlterations\t()
S\t0\t0\tStaff#0=\\new\tlastKeyAlterations\t()
S\t0\t0\tStaff#0=\\new\tlocalAlterations\t(((-1 . 1) 0 1 . #<Mom 1/4>))
S\t0\t0\tStaff#0=\\new\tottavationMarkups\t((4 . "29") (3 . "22") (2 . "15") (1 . "8") (-1 . "8") (-2 . "15") (-3 . "22") (-4 . "29"))
S\t0\t0\tStaff#0=\\new\tshortInstrumentName\t()
S\t0\t0\tStaff#0=\\new\ttonic\t#<Pitch c' >
S\t0\t0\tVoice#0\tcurrentTupletDescription\t()
S\t0\t0\tVoice#0\tlocalAlterations\t(((-1 . 1) 0 1 . #<Mom 1/4>))
S\t0\t0\tVoice#0\tslurMelismaBusy\t#f
E\t1/4\t0\tVoice#0\tbar-event\t() () () () () () () () ()
E\t1/4\t0\tVoice#0\tnote-event\t#<Pitch d > #<Duration 4 > () () () () () () ()
X\t1/2\t0\tVoice#0
X\t1/2\t0\tStaff#0=\\new
X\t1/2\t0\tScore#0=\\new
`

/** The self-tests of the compare on `trace`, which has T14's lines. */
function selfTests(trace) {
  const self = compareTrace(trace, trace)
  assert.equal(self.equal, true)
  assert.deepEqual(self.counts[0], self.counts[1])
  assert.deepEqual(self.differ, [])
  const perturbed = {
    // The bar event a timestep earlier.
    moved: [trace.replace('E\t1/4\t0\tVoice#0\tbar-event', 'E\t0\t0\tVoice#0\tbar-event'), 'events'],
    renamed: [trace.replaceAll('Voice#0', 'Voice#0=one'), 'contexts'],
    property: [trace.replace('tonic\t#<Pitch c\' >', 'tonic\t#<Pitch d\' >'), 'states'],
    dropped: [trace.replace(/S\t0\t0\tVoice#0\tslurMelismaBusy\t#f\n/, ''), 'states'],
    removed: [trace.replace('X\t1/2\t0\tStaff#0=\\new\n', ''), 'removed'],
    order: [trace.replace(/(C\t0\t0\tStaff#0=\\new\t[^\n]*\n)(C\t0\t0\tVoice#0\t[^\n]*\n)/, '$2$1'), 'contexts'],
  }
  for (const [what, [copy, kind]] of Object.entries(perturbed)) {
    assert.notEqual(copy, trace, what)
    const result = compareTrace(trace, copy)
    assert.equal(result.equal, false, what)
    assert.equal(result.differ[0].what, kind, what)
  }
  const moved = compareTrace(trace, perturbed.moved[0]).differ
  assert.deepEqual(moved.map((d) => [d.at, d.context]), [['0 0', 'Voice#0'], ['1/4 0', 'Voice#0'], ['1/4 0', 'Voice#0']])
  assert.deepEqual(compareTrace(trace, perturbed.property[0]).differ, [{
    pass: 1, what: 'states', at: '0 0', context: 'Staff#0=\\new', property: 'tonic',
    expected: '#<Pitch c\' >', got: '#<Pitch d\' >',
  }])
  // A property an engraver of chapter 05 sets is named, not failed.
  const later = compareTrace(trace, perturbed.property[0], { later: ['tonic'] })
  assert.deepEqual([later.equal, later.later], [true, ['tonic']])
}

test('a trace equals itself; a moved event, a renamed context, a changed property differ', () => {
  selfTests(T14)
  const [pass] = readTrace(T14)
  assert.deepEqual(pass.contexts, ['0 0 Score#0=\\new Global#0', '0 0 Staff#0=\\new Score#0=\\new', '0 0 Voice#0 Staff#0=\\new'])
  assert.deepEqual(pass.events.get('1/4 0|Voice#0'), [
    'bar-event\t() () () () () () () () ()',
    'note-event\t#<Pitch d > #<Duration 4 > () () () () () () ()',
  ])
  assert.equal(pass.states.get('0 0|Staff#0=\\new').get('createSpacing'), '#t')
})

const lilypond = lilypondVersion() === '2.26.0' ? false : 'LilyPond 2.26.0 is not installed'
const rows = Object.fromEntries(corpus().map((row) => [row.id, row]))
const skip = lilypond || (SCORES.every((id) => existsSync(rows[id].path)) ? false : 'the corpus is not fetched')
const scratch = mkdtempSync(path.join(tmpdir(), 'lily-trace-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

test("trace.ly writes T14's translation as LilyPond translates it", { skip }, async () => {
  const [result] = await captureTraces({ ids: ['T14'], out: path.join(scratch, 'T14') })
  assert.equal(result.exit, 0, result.stderr)
  assert.equal(result.passes, 1)
  const trace = readFileSync(path.join(scratch, 'T14/T14/trace.tsv'), 'utf8')
  const lines = trace.split('\n').filter((line) => !line.includes('\tScore#0=\\new\t') || !line.startsWith('S\t'))
  assert.equal(lines.join('\n'), T14)
  // The Score's own properties at the bar start: its definition's and Timing_translator's.
  const score = readTrace(trace)[0].states.get('0 0|Score#0=\\new')
  assert.equal(score.get('measurePosition'), '#<Mom 0>')
  assert.equal(score.get('currentBarNumber'), '1')
  assert.ok(score.size > 100, `${score.size} properties`)
  selfTests(trace)
})

test('the 30 scores trace the same twice', { skip, timeout: 600_000 }, async () => {
  const [first, second] = [path.join(scratch, 'a'), path.join(scratch, 'b')]
  const results = await captureTraces({ out: first })
  await captureTraces({ out: second })
  assert.deepEqual(results.map((r) => r.id), SCORES)
  for (const r of results) {
    // T03's error, after its lines are written.
    assert.equal(r.exit, r.id === 'T03' ? 1 : 0, `${r.id}: ${r.stderr}`)
    assert.equal(r.passes, 1, r.id)
    const [a, b] = [first, second].map((dir) => readFileSync(path.join(dir, r.id, 'trace.tsv'), 'utf8'))
    const result = compareTrace(a, b)
    assert.deepEqual(result.differ, [], r.id)
    assert.ok(result.counts[0].contexts >= 3 && result.counts[0].events > 0, r.id)
  }
  // T29's 66 contexts below Global (04 §2.2's census), MarkLine and
  // SmallMarkLine among them, which T29 defines from scratch.
  const t29 = readTrace(readFileSync(path.join(first, 'T29/trace.tsv'), 'utf8'))[0]
  assert.equal(t29.contexts.length, 66)
  assert.ok(t29.contexts.some((c) => c.includes(' MarkLine#0 ')))
})

const cargo = spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'
const packs = preloaded()
/** Why the engine's compiles cannot run: no cargo, or no build's packs. */
const engineSkip = cargo || (packs.length ? false : 'no dist/engine/manifest.json: run npm run build')

/**
 * The engine's trace of `file`, its log and its stderr, compiled from its
 * directory by the core's compile example with the preloaded packs and
 * the files `extra` of others.
 */
function engineTrace(file, out = mkdtempSync(path.join(scratch, 'engine-')), extra = []) {
  const run = runExample('compile', [...[...packs.map(([, pack]) => pack), ...extra].flatMap((pack) => ['--pack', pack]), '--trace', '--out', out, file],
  { cwd: path.dirname(file), encoding: 'utf8' })
  // 1: the file failed, as T03 does in LilyPond; 2: the runner could not run.
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
  const read = (ext) => readFileSync(path.join(out, `${path.basename(file, '.ly')}.${ext}`), 'utf8')
  return { trace: read('trace.tsv'), log: read('trace.log'), err: read('err') }
}

/** The moments of a trace's S lines, `main grace`, in order. */
function barStarts(trace) {
  const moments = trace.split('\n').filter((l) => l.startsWith('S\t')).map((l) => l.split('\t').slice(1, 3).join(' '))
  return [...new Set(moments)]
}

/**
 * The properties of the trace that translators of later steps set, by the
 * translator that sets each in LilyPond 2.26.0: the engine's contexts lack
 * them, or hold them as their definitions or the music left them.
 * Timing_translator's are the engine's since step 25, and
 * Mark_tracking_translator's codaMarkCount and segnoMarkCount and
 * Ottava_spanner_engraver's ottavaStartNow, middleCOffset and T25's
 * middleCPosition since MS3 step 20, Axis_group_engraver's hasAxisGroup
 * since step 21, and Tuplet_engraver's currentTupletDescription, the last,
 * since MS4 step 12: none is left.
 */
const TRANSLATED = {}

/** LilyPond's traces of TRACED's scores, captured once, by ID. */
const references = (() => {
  let traces
  return async () => {
    if (!traces) {
      const ref = path.join(scratch, 'engine-ref')
      await captureTraces({ ids: TRACED, out: ref })
      traces = Object.fromEntries(TRACED.map((id) => [id, readFileSync(path.join(ref, id, 'trace.tsv'), 'utf8')]))
    }
    return traces
  }
})()

/** fonts/' emmentaler-16, which no manifest preloads: T23's staff size, 15, prints with it (D132). */
const emmentaler16 = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../dist/engine/fonts/emmentaler-16.pack')
const skip16 = existsSync(emmentaler16) ? false : 'no dist/engine/fonts/emmentaler-16.pack: run npm run build'

/** Asserts that the engine's `trace` of `id`, with its `log`, equals LilyPond's `reference` but for TRANSLATED's. */
function assertTrace(id, reference, { trace, log }) {
  const unported = log.split('\n').filter((l) => l.startsWith('unported: '))
  assert.deepEqual(unported, [], id)
  assert.deepEqual(barStarts(trace), barStarts(reference), id)
  const result = compareTrace(reference, trace, { later: Object.values(TRANSLATED).flat() })
  assert.deepEqual(result.differ, [], id)
  const [want, have] = result.counts
  assert.deepEqual([have.contexts, have.events], [want.contexts, want.events], id)
  assert.deepEqual(result.later, [], id)
}

test("the engine's traces of part 3's, the grace, the \\change, the volta and the lyric scores equal LilyPond's, timing included, but what later translators set", { skip: skip || engineSkip || skip16, timeout: 900_000 }, async () => {
  const reference = await references()
  for (const id of TRACED) assertTrace(id, reference[id], engineTrace(rows[id].path, undefined, [emmentaler16]))
})

test("core.wasm's traces of part 3's, the grace, the \\change, the volta and the lyric scores, in one instance, equal LilyPond's", { skip: skip || coreSkip() || skip16, timeout: 900_000 }, async () => {
  const reference = await references()
  const modules = [...packs.map(([name, pack]) => [name, readFileSync(pack)]), ['emmentaler-16', readFileSync(emmentaler16)]]
  const results = await compileInWasm(TRACED.map((id) => rows[id].path), { trace: true, modules })
  for (const [i, id] of TRACED.entries()) {
    const { outputs } = results[i]
    assertTrace(id, reference[id], { trace: outputs['trace.tsv'].toString(), log: outputs['trace.log'].toString() })
  }
})

test("the engine's trace of the iterators the 14 do not reach equals LilyPond's", { skip: lilypond || engineSkip, timeout: 300_000 }, async () => {
  const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../fixtures/iterators.ly')
  const out = path.join(scratch, 'iterators', 'trace.tsv')
  const result = await traceOne('lilypond', { id: 'iterators', path: file }, out)
  assert.equal(result.exit, 0, result.stderr)
  const reference = readFileSync(out, 'utf8')
  const { trace, log } = engineTrace(file)
  // Timing_translator's warning and Time_signature_engraver's of a
  // mid-measure \time (MS3 step 17), which LilyPond gives at the same places.
  assert.equal(log, `${file}:16:75: warning: bar check failed at: 1/4\n${file}:17:48: warning: mid-measure time signature without \\partial\n`)
  // Bars only Timing_translator's moments reach among them: the second of
  // R2.*2, and the one \time 2/4 starts within the half note across it.
  assert.deepEqual(barStarts(trace), barStarts(reference))
  assert.ok(['3 0', '5 0'].every((b) => barStarts(trace).includes(b)))
  const compared = compareTrace(reference, trace, { later: Object.values(TRANSLATED).flat() })
  assert.deepEqual(compared.differ, [])
  const classes = new Set(readTrace(trace)[0].events.values().flatMap((l) => l.map((e) => e.split('\t')[0])))
  for (const c of ['partial-event', 'Override', 'Revert', 'SetProperty', 'UnsetProperty', 'tuplet-span-event',
    'reference-time-signature-event', 'multi-measure-rest-event', 'tremolo-span-event']) assert.ok(classes.has(c), c)
})

test("the engine's grace notes trace as LilyPond's, and graceSettings come and go as its Grace_engraver's", { skip: lilypond || engineSkip, timeout: 300_000 }, async () => {
  const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../fixtures/grace.ly')
  const out = path.join(scratch, 'grace')
  const result = await traceOne('lilypond', { id: 'grace', path: file }, path.join(out, 'trace.tsv'))
  assert.equal(result.exit, 0, result.stderr)
  const reference = readFileSync(path.join(out, 'trace.tsv'), 'utf8')
  // The file the fixture's \applyContext calls write, in LilyPond's output
  // directory, where it writes it.
  const ly = path.join(out, 'ly')
  mkdirSync(ly, { recursive: true })
  const run = spawnSync('lilypond', ['--loglevel=ERROR', '-dno-print-pages', '-o', path.join(ly, 'grace'), file], { encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  const engine = mkdtempSync(path.join(scratch, 'engine-'))
  const { trace, log } = engineTrace(file, engine)
  assert.equal(log, '')
  assert.deepEqual(barStarts(trace), barStarts(reference))
  const compared = compareTrace(reference, trace, { later: Object.values(TRANSLATED).flat() })
  assert.deepEqual(compared.differ, [])
  // Grace times: the start's two before 0, an acciaccatura's, an
  // \afterGrace's at 3/4 of its half note and one at 7/8, a grace in a
  // tuplet (its sixteenth scaled to 1/24).
  const moments = new Set(readTrace(trace)[0].events.keys().map((k) => k.split('|')[0]))
  for (const m of ['0 -1/4', '0 -1/8', '1/4 -1/16', '11/8 -1/8', '31/16 -1/8', '25/12 -1/24']) assert.ok(moments.has(m), m)
  const dirs = readFileSync(path.join(ly, 'dirs.txt'), 'utf8')
  assert.equal(readFileSync(path.join(engine, 'dirs.txt'), 'utf8'), dirs)
  // In grace time Stem points up at font size -3; \voiceTwo's settings
  // leave the direction alone; a voice in main time while another is in
  // grace time takes them too, till its next timestep.
  assert.ok(dirs.startsWith('one #<Mom 0G-1/4> 1 -3\n'))
  assert.ok(dirs.includes('two #<Mom 1/2G-1/16> -1 -3\n'))
  assert.ok(dirs.includes('two #<Mom 1/4> -1 -3\n'))
})

test("the engine's volta repeats and alternatives trace as LilyPond's", { skip: lilypond || engineSkip, timeout: 300_000 }, async () => {
  const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../fixtures/volta.ly')
  const out = path.join(scratch, 'volta', 'trace.tsv')
  const result = await traceOne('lilypond', { id: 'volta', path: file }, out)
  assert.equal(result.exit, 0, result.stderr)
  const reference = readFileSync(out, 'utf8')
  const { trace, log } = engineTrace(file)
  // The alternative \volta 1 leaves without one, where LilyPond warns.
  assert.equal(log, `${file}:24:59: warning: missing volta specification on alternative element\n`)
  assert.deepEqual(barStarts(trace), barStarts(reference))
  const compared = compareTrace(reference, trace, { later: Object.values(TRANSLATED).flat() })
  assert.deepEqual(compared.differ, [])
  assert.deepEqual(compared.counts[1].events, compared.counts[0].events)
  const classes = new Set(readTrace(trace)[0].events.values().flatMap((l) => l.map((e) => e.split('\t')[0])))
  for (const c of ['volta-repeat-start-event', 'volta-repeat-end-event', 'alternative-event', 'volta-span-event',
    'segno-mark-event', 'coda-mark-event', 'dal-segno-event']) assert.ok(classes.has(c), c)
  // The repeats' commands, and the alternatives' bar numbers with letters.
  const score = readTrace(trace)[0].states
  const values = (name) => [...score.entries()].filter(([k]) => k.endsWith('|Score#0=\\new')).map(([, s]) => s.get(name))
  assert.ok(values('repeatCommands').includes('((start-repeat 2))'))
  assert.ok(values('alternativeNumber').includes('2'))
})

test('the expected warnings of T10 and T15 (ly:expect-warning) are not printed', { skip: skip || engineSkip, timeout: 300_000 }, () => {
  // T10's two, which its \alternatives give as the parser reduces them:
  // its stderr is LilyPond's, which is empty.
  const t10 = engineTrace(rows.T10.path)
  assert.equal(t10.err, readFileSync(path.join(corpusDir(), 'ref', 'T10', 'stderr.txt'), 'utf8'))
  assert.equal(t10.err, '')
  // T15's two, which its Scheme engraver gives in the \layout translation;
  // what it writes to the error port is in its stderr as in LilyPond's
  // (since step 40, D99), but its acknowledgers' lines (D87).
  const t15 = engineTrace(rows.T15.path)
  const ly15 = readFileSync(path.join(corpusDir(), 'ref', 'T15', 'stderr.txt'), 'utf8')
  for (const err of [t15.err, ly15, t15.log]) {
    assert.ok(!err.includes('warning:'), err)
    assert.ok(err.includes('#<Mom 0>        : (initialize)\n'), err)
  }
})

test("the engine's lyrics combine with their voices as LilyPond's", { skip: lilypond || engineSkip, timeout: 300_000 }, async () => {
  const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../fixtures/lyrics.ly')
  const out = path.join(scratch, 'lyrics', 'trace.tsv')
  const result = await traceOne('lilypond', { id: 'lyrics', path: file }, out)
  assert.equal(result.exit, 0, result.stderr)
  const reference = readFileSync(out, 'utf8')
  const { trace, log } = engineTrace(file)
  // LilyPond's warnings, at its places: the lyrics with no Lyrics context,
  // their voice never followed, and the voice that never comes.
  assert.equal(log, [
    `${file}:39:22: warning: argument of \\lyricsto should contain Lyrics context`,
    `${file}:39:5: warning: cannot find context: Voice = bare`,
    `${file}:29:17: warning: cannot find context: Voice = nowhere`,
  ].map((l) => `${l}\n`).join(''))
  assert.deepEqual(barStarts(trace), barStarts(reference))
  const compared = compareTrace(reference, trace, { later: Object.values(TRANSLATED).flat() })
  assert.deepEqual(compared.differ, [])
  assert.deepEqual(compared.counts[1].events, compared.counts[0].events)
  const lines = trace.split('\n')
  const syllable = (at, context, text) => lines.some((l) => l.startsWith(`E\t${at}\t${context}\tlyric-event\t`) && l.includes(`"${text}"`))
  // A grace note's syllable at its main note, or before it with
  // includeGraceNotes; a voice made at 1/2 found then; associatedVoice
  // followed to the voice two; the voice moved to the other staff.
  assert.ok(syllable('3/4\t0', 'Lyrics#0', 'c') && syllable('3/4\t-1/16', 'Lyrics#1', 'c'))
  assert.ok(lines.includes('C\t1/2\t0\tVoice#8=late\tStaff#1'))
  assert.ok(syllable('1/2\t0', 'Lyrics#2', 'la') && syllable('7/4\t0', 'Lyrics#2', 'lo'))
  assert.ok(syllable('3/4\t0', 'Lyrics#4', 'd'))
})

/** The playback map in `dir`, its links without their directories. */
function playbackMap(dir) {
  const file = readdirSync(dir).find((f) => f.endsWith('.timing.json'))
  const performances = JSON.parse(readFileSync(path.join(dir, file), 'utf8'))
  return performances.map((p) => ({ ...p, events: p.events.map((e) => ({ ...e, href: key(e.href) })) }))
}

test("the engine's melismas hold syllables back as LilyPond's, in \\layout and in \\midi", { skip: lilypond || engineSkip, timeout: 300_000 }, async () => {
  const file = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../fixtures/melisma.ly')
  const out = path.join(scratch, 'melisma')
  const result = await traceOne('lilypond', { id: 'melisma', path: file }, path.join(out, 'trace.tsv'))
  assert.equal(result.exit, 0, result.stderr)
  const reference = readFileSync(path.join(out, 'trace.tsv'), 'utf8')
  // LilyPond's playback map: timing.ly's performers', of its \midi.
  const runtime = path.dirname(createRequire(import.meta.url).resolve('@lily/common/runtime/timing.ly'))
  const ly = path.join(out, 'ly')
  mkdirSync(ly, { recursive: true })
  const run = spawnSync('lilypond', ['--loglevel=ERROR', '-dno-print-pages', '-I', runtime, '-dinclude-settings=timing.ly',
    '-o', path.join(ly, 'melisma'), file], { cwd: path.dirname(file), encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  const dir = mkdtempSync(path.join(scratch, 'engine-'))
  const { trace, log } = engineTrace(file, dir)
  // LilyPond's `unterminated tie`, at the tie of d4~ e, which its
  // Tie_engraver's grob gives (MS3 step 17); since MS4 step 10 its layout
  // completes, its ties drawn, so no traced read meets an unbroken beam.
  assert.equal(log, `${file}:19:9: warning: unterminated tie\n`)
  assert.deepEqual(barStarts(trace), barStarts(reference))
  const compared = compareTrace(reference, trace, { later: Object.values(TRANSLATED).flat() })
  assert.deepEqual(compared.differ, [])
  assert.deepEqual(compared.counts[1].events, compared.counts[0].events)
  // The melismas hold syllables back but where ignoreMelismata is set;
  // includeGraceNotes gives the grace notes theirs, one held by its beam.
  const sung = (n) => trace.split('\n').filter((l) => l.startsWith('E\t') && l.includes(`\tLyrics#${n}\tlyric-event\t`)).length
  assert.deepEqual([sung(0), sung(1), sung(2)], [33, 46, 34])
  for (const p of ['slurMelismaBusy', 'tieMelismaBusy', 'beamMelismaBusy']) {
    const values = [...readTrace(trace)[0].states.values()].map((s) => s.get(p))
    assert.ok(values.includes('#t') && values.includes('#f'), p)
  }
  assert.deepEqual(playbackMap(dir), playbackMap(ly))
})

const incipitsLy = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../fixtures/incipits.ly')
/** fonts/' emmentaler-26, which no manifest preloads: the incipits' clefs, at font-size 3, print with it. */
const emmentaler26 = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../dist/engine/fonts/emmentaler-26.pack')
const skipIncipits = lilypond || engineSkip || (existsSync(emmentaler26) ? false : 'no dist/engine/fonts/emmentaler-26.pack: run npm run build')
/** LilyPond's trace of fixtures/incipits.ly, captured once. */
const incipitsReference = (() => {
  let reference
  return async () => {
    if (!reference) {
      const out = path.join(scratch, 'incipits', 'trace.tsv')
      const result = await traceOne('lilypond', { id: 'incipits', path: incipitsLy }, out)
      assert.equal(result.exit, 0, result.stderr)
      reference = readFileSync(out, 'utf8')
    }
    return reference
  }
})()

/**
 * Asserts that the engine's `trace` of fixtures/incipits.ly, with its
 * `log`, equals LilyPond's `reference` but for TRANSLATED's: the score's
 * and its three incipits', the incipits translated once each when their
 * names print, after the score's X lines, by their Score's translator
 * alone (S lines of Score#1–#3, Staff#3–#5 and Voice#3–#5, as LilyPond's).
 */
function assertIncipits(reference, { trace, log }, label) {
  assert.equal(log, '', label)
  assert.deepEqual(barStarts(trace), barStarts(reference), label)
  const later = Object.values(TRANSLATED).flat()
  const compared = compareTrace(reference, trace, { later })
  assert.deepEqual(compared.differ, [], label)
  assert.deepEqual(compared.counts[1].contexts, compared.counts[0].contexts, label)
  assert.deepEqual(compared.counts[1].events, compared.counts[0].events, label)
  const incipits = (text) => {
    const lines = text.split('\n')
    const at = lines.findLastIndex((l) => l.startsWith('X\t')) + 1
    return [at, lines.slice(at).filter((l) => l && !later.includes(l.split('\t')[4]))]
  }
  const [[at, want], [from, have]] = [incipits(reference), incipits(trace)]
  assert.deepEqual(have, want, label)
  assert.ok(at > 0 && from > 0 && want.length === 921 && want.every((l) => /^S\t[^\t]+\t0\t(Score#[1-3]|Staff#[3-5]|Voice#[3-5])\t/.test(l)), label)
  assert.deepEqual(['Triplum', 'Motetus', 'Tenor'].map((n) => want.some((l) => l.endsWith(`\tinstrumentName\t"${n}"`))), [true, true, true], label)
}

test("the engine's trace of T23's incipits, \\scores in instrument names, equals LilyPond's", { skip: skipIncipits, timeout: 300_000 }, async () => {
  assertIncipits(await incipitsReference(), engineTrace(incipitsLy, undefined, [emmentaler26]), 'natively')
})

test("core.wasm's trace of T23's incipits equals LilyPond's", { skip: skipIncipits || coreSkip(), timeout: 300_000 }, async () => {
  const reference = await incipitsReference()
  const modules = [...packs.map(([name, pack]) => [name, readFileSync(pack)]), ['emmentaler-26', readFileSync(emmentaler26)]]
  const [{ outputs }] = await compileInWasm([incipitsLy], { trace: true, modules })
  assertIncipits(reference, { trace: outputs['trace.tsv'].toString(), log: outputs['trace.log'].toString() }, 'in wasm')
})


