// Y8's grobs (README §5's MS8, MS8 steps 7, 8 and 9), each with a score of LilyPond's
// input/regression/ that covers what no other of the corpus does,
// corpus.tsv's Y01–Y90. Step 7's part 1, its first five: KeyCancellation
// (key-signature-cancellation.ly), RehearsalMark (rehearsal-mark-letter.ly,
// Mark_engraver's), TextMark (text-mark.ly), PhrasingSlur
// (phrasing-slur-dash.ly, its dash patterns) and RepeatTie (repeat-tie.ly,
// Repeat_tie_engraver's). Its rest: CodaMark (coda-mark-formatters.ly),
// SegnoMark (segno-style.ly, the segno bar lines too), SectionLabel
// (section-label-style.ly), Mark_engraver's; JumpScript, Jump_engraver's:
// D.S. al Coda (repeat-ds-al-coda.ly), ad-hoc jumps on dotted bar lines
// (jump-unaligned.ly, make-dotted-bar-line's (iota 3.0)), D.C. al Fine with
// a Fine killed at the end (repeat-dc-al-fine.ly) and one kept
// (repeat-dc-v-fine-end-visible.ly); and A37's ly:bezier-extent, which
// make-path-stencil calls for a curve (markup-path.ly, path-exercise.ly:
// markups alone, no grob). Step 7's part 2: the percent repeats' six grobs (repeat-percent.ly,
// repeat-percent-count-visibility.ly, repeat-slash-mixed.ly), CaesuraScript and the caesura's
// BreathingSign (caesura-script-multiple.ly, caesura-style-comma-or-fermata.ly, at a bar line),
// CueClef and CueEndClef (cue-clef-manually.ly), InstrumentSwitch (instrument-switch.ly),
// CombineTextScript (part-combine-text.ly, A44) and Parentheses (parenthesize.ly). Step 8's part 1: the piano pedals'
// PianoPedalBracket, UnaCordaPedal and their line spanners (pedal-bracket.ly, the bracket and mixed styles across a
// line break), SustainPedal (pedal-ped.ly, the text style), a bracket run to the end (pedal-end.ly), the pitched
// trills' four grobs with A19's trill line (trill-spanner-pitched-consecutive.ly, trill-spanner-pitched.ly), NoteName
// (note-names-context.ly), MelodyItem (stem-direction-context.ly) and VowelTransition's rods (vowel-transition.ly).
// Step 8's part 2: Ambitus with its AccidentalPlacement made at finalize (ambitus.ly), ClusterSpanner (cluster.ly),
// BalloonText (balloon.ly), Footnote on items and spanners (footnote.ly, two pages), GridLine and GridPoint
// (grid-lines.ly), MeasureGrouping (measure-grouping.ly), MeasureSpanner (measure-spanner.ly), and A44's
// completion engravers' NoteHeads and Rests (completion-heads.ly, completion-rest.ly). Step 9's part 1: LigatureBracket
// (ligature-bracket.ly, its right end apart from other voices; ligature-bracket-X-positions.ly, on stems and heads),
// MensuralLigature with its heads' shapes, flexae, dots and joins and its warnings (mensural-ligatures.ly, two
// scores), and one beside an Ambitus (ambitus-with-ligature.ly). Step 9's part 2: the episema's score, a VaticanaLigature
// with Episemas (episema.ly), Custos in every style and a VaticanaScore of pes, flexae and an augmentum (custos.ly), a
// Custos_engraver that ignores unpitched notes (custos-unpitched.ly), a VaticanaLigature's dots in a DotColumn of their
// own (augmentum.ly), A38's divisiones over staves of every kind (breathing-sign-ancient.ly), and a KievanLigature with
// dots (kievan-notation-dots.ly). A VaticanaScore's grobs are counted in layout.tsv: grobs.ly records the context named
// Score alone, so its grobs.tsv is empty. Step 9's part 3: the page breakers no other score reaches, each counted by
// its Systems in layout.tsv: ly:one-page-breaking (one-page-breaking.ly), ly:minimal-breaking (page-minimal-page-breaking.ly,
// its forced and forbidden page breaks between scores; page-minimal-page-breaking-last-page.ly, markups alone, the last
// moved past the tagline), ly:one-line-breaking (one-line-breaking.ly), ly:one-line-auto-height-breaking
// (one-line-auto-height-breaking.ly), and ly:page-turn-breaking with Page_turn_engraver in a Staff, a Score and a Voice
// (page-turn-page-breaking.ly, -score.ly, -voice.ly), over repeats (-repeats.ly), at a first column (-first-column.ly),
// with auto-first-page-number (-auto-first-page.ly, -auto-first-page2.ly), its bad turns (-badturns.ly) and a toplevel
// \pageTurn between scores (page-break-turn-toplevel.ly). MS8 step 10: Quote_iterator, each counted by its NoteHeads,
// the cue notes among them (quote.ly, quote-during.ly, quote-cue-during.ly, quote-cue-event-types.ly,
// quote-transposition.ly, quote-grace.ly, quote-tie.ly, quote-tuplet.ly, quote-tuplet-end.ly, quote-cyclic.ly,
// quote-kill-cues.ly, quote-overrides.ly, quote-during-subvoice.ly); \fine's fine-event (fine.ly, OPEN C4); and P12's
// and OPEN F's gaps a regression score has: \lyricsto Type = id (lyric-combine-repeat-count.ly), a number list 2,2,2,2
// (tuplet-subdivision.ly), expect-error and ly:parser-clear-error (modify-tag-group-errors.ly, no music), and the
// tagged \change of \autoChange (auto-change.ly, autochange-keep-with-tag.ly). MS8 step 11: syntax-case (D273),
// accreg.ly's define-register-set, each register a TextScript; S6's qr part, markups alone (qr-code.ly,
// qr-code-version.ly, qr-code-error-correction-level.ly), and its modal part (modal-transforms.ly, retrograde.ly);
// OPEN C2.6's completionBusy, lyrics on Completion_heads_engraver's split notes (completion-heads-lyrics.ly).
// C0–C5 on each, as for the 30: compiled natively
// with the trace and in one core.wasm instance, against LilyPond's capture
// ($LILY_CORPUS/ref/<id>/, npm run capture -- Y01 … Y90):
//
// - C0: stderr LilyPond's byte for byte, and diagnostics.json its parse.
// - C1–C5 through score.mjs: every layer's Must, C1, C2 and C3 at 1.0 and
//   C2b's Goal (the .midi LilyPond's as it is), every link paired with every
//   Δ 0, the shape dump c5.tsv LilyPond's byte for byte; grobs.tsv and
//   layout.tsv LilyPond's on every line.
// - core.wasm's outputs the native compile's, the traced files among them.
// - The pixels: PAE one grey level or less against LilyPond's (Y07's two, FORM), recorded.
//
// Y51, Y54 and Y55 include typography-demo.ly, whose second verse is set in the families T20 asks for, which the
// capturing machine's fontconfig resolved outside the packs: they are judged as T20 is (D227, FONTS). Their stderr is
// LilyPond's but for C059's "no glyph" warnings of the Hebrew and Hiragana, their C1 and C2 pass and their grobs.tsv is
// LilyPond's; C3–C5, c5.tsv, layout.tsv and the pixels, which the lyrics' widths move, are recorded. Their breakers are
// at Δ 0 on every layer where the verse is Latin (the step's notes, MS8 step 9's part 3).
//
// Skipped, and so unverified, without LilyPond 2.26.0, cargo, the corpus
// fetched and captured, or the build (npm run build) for core.wasm; the
// pixels also without Homebrew's rsvg-convert and magick.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { lilypondVersion } from '../capture.mjs'
import { compileInWasm, coreSkip } from '../core.mjs'
import { compareGrobs } from '../grobs.mjs'
import { compareLayout } from '../layout.mjs'
import { compare, compareStderr } from '../score.mjs'
import { parseStderr } from '../stderr.mjs'

const engine = path.resolve(path.dirname(new URL(import.meta.url).pathname), '../../..')
/** Each score's grob, and how many of LilyPond's grobs of that type it has (in layout.tsv, live, where a third item says
 * so); none for a markup alone. */
const SCORES = {
  Y01: ['KeyCancellation', 3], Y02: ['RehearsalMark', 9], Y03: ['TextMark', 2], Y04: ['PhrasingSlur', 8], Y05: ['RepeatTie', 5],
  Y06: ['CodaMark', 6], Y07: ['SegnoMark', 15], Y08: ['SectionLabel', 5], Y09: ['JumpScript', 2], Y10: ['JumpScript', 2],
  Y11: [null, 0], Y12: [null, 0], Y13: ['JumpScript', 5], Y14: ['JumpScript', 3],
  Y15: ['RepeatSlash', 3], Y16: ['PercentRepeat', 9], Y17: ['DoubleRepeatSlash', 4], Y18: ['CaesuraScript', 10],
  Y19: ['CaesuraScript', 56], Y20: ['CueClef', 1], Y21: ['InstrumentSwitch', 1], Y22: ['CombineTextScript', 2],
  Y23: ['Parentheses', 3],
  Y24: ['PianoPedalBracket', 7], Y25: ['SustainPedal', 8], Y26: ['SustainPedalLineSpanner', 1], Y27: ['TrillPitchHead', 6],
  Y28: ['TrillSpanner', 1], Y29: ['NoteName', 52], Y30: ['MelodyItem', 4], Y31: ['VowelTransition', 2],
  Y32: ['Ambitus', 3], Y33: ['ClusterSpanner', 1], Y34: ['BalloonText', 4], Y35: ['Footnote', 4], Y36: ['GridLine', 13],
  Y37: ['MeasureGrouping', 6], Y38: ['MeasureSpanner', 4], Y39: ['NoteHead', 24], Y40: ['Rest', 18],
  Y41: ['LigatureBracket', 2], Y42: ['LigatureBracket', 5], Y43: ['MensuralLigature', 69], Y44: ['MensuralLigature', 1],
  Y45: ['Episema', 3, 'layout.tsv'], Y46: ['Custos', 23], Y47: ['Custos', 0], Y48: ['DotColumn', 4, 'layout.tsv'],
  Y49: ['Divisio', 30], Y50: ['KievanLigature', 1],
  Y51: ['System', 5, 'layout.tsv'], Y52: ['System', 4, 'layout.tsv'], Y53: [null, 0], Y54: ['System', 1, 'layout.tsv'],
  Y55: ['System', 1, 'layout.tsv'], Y56: ['System', 6, 'layout.tsv'], Y57: ['System', 6, 'layout.tsv'],
  Y58: ['System', 6, 'layout.tsv'], Y59: ['System', 8, 'layout.tsv'], Y60: ['System', 1, 'layout.tsv'],
  Y61: ['System', 16, 'layout.tsv'], Y62: ['System', 8, 'layout.tsv'], Y63: ['System', 3, 'layout.tsv'],
  Y64: ['System', 6, 'layout.tsv'],
  Y65: ['NoteHead', 15], Y66: ['NoteHead', 15], Y67: ['NoteHead', 17], Y68: ['NoteHead', 28], Y69: ['NoteHead', 12],
  Y70: ['NoteHead', 12], Y71: ['NoteHead', 2], Y72: ['NoteHead', 12], Y73: ['NoteHead', 5], Y74: ['NoteHead', 16],
  Y75: ['NoteHead', 12], Y76: ['NoteHead', 20], Y77: ['NoteHead', 19],
  Y78: ['JumpScript', 2], Y79: ['LyricRepeatCount', 4], Y80: ['TupletBracket', 8], Y81: [null, 0], Y82: ['NoteHead', 7],
  Y83: ['NoteHead', 2],
  Y84: ['TextScript', 12], Y85: [null, 0], Y86: [null, 0], Y87: [null, 0], Y88: ['NoteHead', 40],
  Y89: ['NoteHead', 24], Y90: ['LyricText', 2],
}
/** The scores judged as T20 is (D227): text in fonts the capturing machine's fontconfig chose, outside the packs. */
const FONTS = new Set(['Y51', 'Y54', 'Y55'])
/** C059's warning of a character no pack's face has, which LilyPond took from the machine's fonts. */
const NO_GLYPH = /^warning: no glyph for character .*\n/gm
const IDS = Object.keys(SCORES)
/**
 * The grey levels a page may differ by beyond W3's one, from the SVG form alone: Y07's boxed segni
 * (format-mark-box-numbers), whose top edges LilyPond writes as a translate and a rect inside it and the engine
 * as one absolute rect, the same numbers, render 2 levels apart on 139 pixels. LilyPond's own page with one such
 * rect rewritten in the engine's form renders that box as the engine's does
 * ($LILY_CORPUS/device-runs/2026-11-08-ms8-step7-rest/y07-form.sh). Y85's QR squares the same (173 pixels): LilyPond's
 * own page with its 2,516 squares' rects in the engine's form renders as the engine's, PAE 0
 * ($LILY_CORPUS/device-runs/2026-11-11-ms8-step11/y85-form.sh).
 */
const FORM = { Y07: 2, Y85: 2 }
const rows = IDS.map((id) => corpus().find((row) => row.id === id)).filter(Boolean)
const references = path.join(corpusDir(), 'ref')
const skip = lilypondVersion() !== '2.26.0' ? 'LilyPond 2.26.0 is not installed'
  : spawnSync('cargo', ['+stable', '--version']).status !== 0 ? 'cargo +stable is not installed'
    : rows.length !== IDS.length || !rows.every((row) => existsSync(row.path)) ? 'the corpus is not fetched: run node tools/corpus/fetch.mjs'
      : !IDS.every((id) => existsSync(path.join(references, id, 'capture.json'))) ? `no capture of Y01–Y90 in ${references}: run npm run capture -- ${IDS.join(' ')}`
        : false
const tools = ['rsvg-convert', 'magick'].filter((tool) => spawnSync(tool, ['--version']).status !== 0)
const skipPixels = skip || (tools.length ? `${tools.join(' and ')} not installed (Homebrew): the pixels are unverified` : false)

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-y8-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

const stem = (row) => path.basename(row.path, '.ly')
/**
 * Whether the output file `f` is `row`'s: named `<stem>.…` or `<stem>-…`, and no longer stem of the corpus's rows
 * owns it (Y15's repeat-percent.ly beside Y16's repeat-percent-count-visibility.ly).
 */
const owns = (row, f) => {
  const mine = (r) => f.startsWith(`${stem(r)}.`) || f.startsWith(`${stem(r)}-`)
  return mine(row) && !rows.some((r) => stem(r).length > stem(row).length && mine(r))
}
/** The trace's files the compile example writes as `<name>.<file>`, which a capture names `<file>`. */
const TRACED = ['grobs.tsv', 'layout.tsv', 'columns.tsv', 'line.tsv', 'c5.tsv']

let native
/** The scores compiled natively with the trace, once, each into `<scratch>/native/<id>/` with a capture's names. */
function compiled() {
  if (native) return native
  const out = path.join(scratch, 'native')
  mkdirSync(out, { recursive: true })
  const run = runExample('compile', ['--trace', '--out', out, ...rows.map((row) => row.path)], { cwd: engine, encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  native = Object.fromEntries(rows.map((row) => {
    const files = Object.fromEntries(readdirSync(out).filter((f) => owns(row, f))
      .map((f) => [f, readFileSync(path.join(out, f))]))
    const dir = path.join(out, row.id)
    mkdirSync(dir)
    for (const f of Object.keys(files)) {
      const traced = TRACED.find((t) => f === `${stem(row)}.${t}`)
      copyFileSync(path.join(out, f), path.join(dir, traced ?? f))
    }
    return [row.id, { dir, files }]
  }))
  return native
}

const read = (dir, file) => readFileSync(path.join(dir, file), 'utf8')

test("Y01–Y90 natively: C0, stderr LilyPond's bytes (FONTS' but C059's no glyph) and diagnostics.json its parse", { skip, timeout: 600_000 }, () => {
  for (const row of rows) {
    const { dir } = compiled()[row.id]
    const ref = path.join(references, row.id)
    const stderr = read(dir, `${stem(row)}.err`)
    const judged = FONTS.has(row.id) ? stderr.replace(NO_GLYPH, '') : stderr
    if (FONTS.has(row.id)) assert.notEqual(judged, stderr, `${row.id}: C059's no glyph warnings`)
    assert.equal(judged, read(ref, 'stderr.txt'), row.id)
    assert.equal(compareStderr(read(ref, 'stderr.txt'), judged, { rootFile: row.path }).pass.C0, true, row.id)
    assert.deepEqual(JSON.parse(read(dir, `${stem(row)}.json`)), parseStderr(stderr, { rootFile: row.path }), row.id)
  }
})

test("Y01–Y90 natively: C1–C5 pass, every link paired with every Δ 0, c5.tsv, grobs.tsv and layout.tsv LilyPond's, each its grob",
  { skip, timeout: 600_000 }, (t) => {
    for (const row of rows) {
      const { dir } = compiled()[row.id]
      const ref = path.join(references, row.id)
      const r = compare(ref, dir)
      if (FONTS.has(row.id)) {
        // D227: C1, C2 and grobs.tsv held, the rest recorded.
        assert.deepEqual([r.pass.C1, r.pass.C2], [true, true], row.id)
        assert.equal(r.pages[0], r.pages[1], row.id)
        const grobs = compareGrobs(read(ref, 'grobs.tsv'), read(dir, 'grobs.tsv'))
        assert.equal(grobs.equal, true, `${row.id}: grobs.tsv ${JSON.stringify(grobs)}`)
        const layout = compareLayout(read(ref, 'layout.tsv'), read(dir, 'layout.tsv'))
        t.diagnostic(`${row.id} (D227): ${JSON.stringify(r.pass)}, links ${r.linksMatched}, dx ${r.dxMedian}/${r.dxP95}, dy ${r.dyMedian}/${r.dyP95}, c5.tsv ${read(dir, 'c5.tsv') === read(ref, 'c5.tsv')}, layout.tsv ${layout.equal}`)
        continue
      }
      assert.deepEqual(r.pass, { C1: true, C2: true, C3: true, C4: true, C5: true }, row.id)
      const [type, count, counted = 'grobs.tsv'] = SCORES[row.id]
      // A markup alone has no performance, no .midi and no system.
      if (type) assert.deepEqual([r.events, r.midiNotes, r.breaks], [1, 1, 1], row.id)
      // C2b, a Goal: the .midi LilyPond's as it is but the creator text.
      assert.equal(r.goal.C2b, true, `${row.id}: C2b`)
      assert.equal(r.pages[0], r.pages[1], row.id)
      assert.equal(r.linksMatched[0], r.linksMatched[1], `${row.id}: every link paired`)
      assert.deepEqual([r.dxMedian, r.dxP95, r.dyMedian, r.dyP95], type ? [0, 0, 0, 0] : [null, null, null, null], row.id)
      assert.equal(read(dir, 'c5.tsv'), read(ref, 'c5.tsv'), `${row.id}: c5.tsv`)
      const grobs = compareGrobs(read(ref, 'grobs.tsv'), read(dir, 'grobs.tsv'))
      assert.equal(grobs.equal, true, `${row.id}: grobs.tsv ${JSON.stringify(grobs)}`)
      const layout = compareLayout(read(ref, 'layout.tsv'), read(dir, 'layout.tsv'))
      assert.equal(layout.equal, true, `${row.id}: layout.tsv ${JSON.stringify(layout).slice(0, 400)}`)
      const made = read(dir, counted).split('\n').filter((l) => type && l.startsWith(`${type}\t`)).length
      assert.equal(made, count, `${row.id}: its ${type}s`)
      t.diagnostic(`${row.id} ${type} × ${count}: ${r.pages[0]} page, ${r.linksMatched[0]} links, ${grobs.counts[0]} grobs, ${layout.counts[0]} live`)
    }
  })

test("Y01–Y90 in one core.wasm instance, traced: every output the native compile's", { skip: skip || coreSkip(), timeout: 600_000 }, async () => {
  const wasm = await compileInWasm(rows.map((row) => row.path), { trace: true })
  for (const [i, row] of rows.entries()) {
    const { files } = compiled()[row.id]
    const outputs = wasm[i].outputs
    const named = (name) => name === 'stderr' ? `${stem(row)}.err` : name === 'diagnostics.json' ? `${stem(row)}.json`
      : name.startsWith(`${stem(row)}.`) || name.startsWith(`${stem(row)}-`) ? name : `${stem(row)}.${name}`
    assert.deepEqual(Object.keys(outputs).map(named).sort(), Object.keys(files).sort(), row.id)
    for (const [name, bytes] of Object.entries(outputs)) assert.deepEqual(Buffer.from(bytes), files[named(name)], `${row.id} ${name}`)
  }
})

test("Y01–Y90's pages' pixels: PAE one grey level or less against LilyPond's (FONTS' recorded), AE recorded", { skip: skipPixels, timeout: 600_000 }, (t) => {
  const over = []
  for (const row of rows) {
    const { dir } = compiled()[row.id]
    const ref = path.join(references, row.id)
    for (const file of readdirSync(ref).filter((f) => f.endsWith('.svg'))) {
      const png = (from, to) => {
        const run = spawnSync('rsvg-convert', ['-z', '2', '-b', 'white', from, '-o', to], { encoding: 'utf8' })
        assert.equal(run.status, 0, `${row.id} ${file}: rsvg-convert: ${run.stderr}`)
        return to
      }
      const [a, b] = [png(path.join(ref, file), path.join(dir, `${file}.ref.png`)), png(path.join(dir, file), path.join(dir, `${file}.png`))]
      const metric = (name) => {
        const run = spawnSync('magick', ['compare', '-metric', name, a, b, 'null:'], { encoding: 'utf8' })
        assert.ok(run.status === 0 || run.status === 1, `${row.id} ${file}: magick compare: ${run.stderr}`)
        const [, absolute, normalized] = run.stderr.match(/^([\d.e+-]+) \(([\d.e+-]+)\)/)
        return { absolute: +absolute, normalized: +normalized }
      }
      const grey = metric('PAE').normalized * 255
      const ae = metric('AE').absolute
      t.diagnostic(`${row.id} ${file}: PAE ${grey.toFixed(3)} grey levels, AE ${ae} pixels`)
      if (!FONTS.has(row.id) && grey > (FORM[row.id] ?? 1) + 1e-6) over.push(`${row.id} ${file}: PAE ${grey.toFixed(3)}, AE ${ae}`)
    }
  }
  assert.deepEqual(over, [])
})
