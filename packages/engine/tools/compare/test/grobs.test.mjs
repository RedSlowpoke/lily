// The references of part 2 of MS3 (progress.md, "How part 2's gate reads";
// MS3 step 12): grobs.ly's capture of the grobs each translation announces,
// and grobs.mjs' compare.
//
// - The compare (no LilyPond): a capture against itself passes; a changed
//   type, cause, link, moment, context or engraver differs in that field, a
//   swapped grob is out of order, a dropped one missing, an added one extra;
//   every type unless asked for some (MS3 step 22), and their columns from
//   step 15 on unless asked not to.
// - The fixture (fixtures/grobs.ly): LilyPond 2.26.0 captures the grobs its
//   Scheme engravers make with ly:engraver-make-grob, from events and from
//   grobs, with their causes, links, moments, contexts and engravers.
// - The engine's grobs of the fixture, written by a native traced compile
//   (crates/core/examples/compile.rs --trace, <name>.grobs.tsv) from its
//   grob records, equal LilyPond's in every field, their columns too (MS3
//   step 15: the paper columns and the System among them; step 16: the
//   note heads, rests, stems, stem stubs, the flag, the note columns and
//   the ledger line spanners; step 17: the time signatures), on every line
//   from step 22 on but the three of the two engravers no part of MS3's
//   part 2 makes; and so do core.wasm's, a traced compile's grobs.tsv in
//   one instance (core.mjs, lsb_trace).
// - grob-calls.ly (MS3 step 13), LilyPond 2.26.0's answers to calls on the
//   fixture's grobs, part B's reference: two captures the same byte for
//   byte; the fixture's grobs as the fixture's own capture has them; the
//   properties, callbacks, chains, cycles, parents, offsets, extents and
//   suicide's answers; MS3 step 14's calls (interfaces and items, grob
//   arrays, pointer groups and units, pure and unpure calls, the rest of
//   grob-scheme.cc's) after step 13's 163, step 15's columns, systems and
//   ranks among them; the warnings at the grobs'
//   origins in the calls' order, and grob arrays' two; and each grob's link
//   equal to the one its print stencil has on the page, inside whiteout and
//   color.
// - tools/bench/probe-grobs.scm (MS3 step 13), grob-calls.ly's calls run by
//   the engine on its own grobs of the fixture: a file that includes the
//   fixture and runs the probe in its Score's finalize, as grob-calls.ly
//   runs its calls, compiled from the core's image natively
//   (crates/core/examples/compile.rs) and in core.wasm, gives LilyPond's
//   297 records, the 10 links the engine's writer's (crates/svg's
//   grob-cause, read through layout's %grob-cause-link) in LilyPond's
//   <a> (MS3 step 13a), and LilyPond's stderr byte for byte, the programming errors at the
//   grobs' origins with their source lines, as the parser crate's report
//   prints a located one (MS3 step 13a). crates/layout/tests/grobs.rs
//   runs two of its sections natively in a VM without the library.
// - The 10 scores of MS3's gate, captured afresh with the capture's flags
//   and capture.ly, read back and counted per type, engraver and context
//   against makers.txt (MS3 step 11, $LILY_CORPUS/ms3-part2-split): 1,903
//   grobs announced and 10 Systems, one translation each.
// - The engine's grobs of the 10, every line from MS3 step 22 on, from a
//   native traced compile and from core.wasm's in one instance, equal
//   LilyPond's with their columns (MS3 step 15): 278 paper columns and 10
//   Systems, and
//   from step 16 198 NoteHeads, 4 Rests, 125 Stems and 125 StemStubs,
//   T03's Dots and DotColumn, 125 NoteColumns and 10 LedgerLineSpanners,
//   from step 17 10 TimeSignatures, 28 Accidentals and 22
//   AccidentalPlacements, from step 18 10 StaffSymbols, 61 BarLines,
//   57 BarNumbers and 10 SystemStartBars, and from step 19 35 TextScripts,
//   T05's 18 ScriptRows and T18's 5 DynamicTexts, 3 Hairpins and 5
//   DynamicLineSpanners, and from step 20 T10's 4 VoltaBrackets and 2
//   VoltaBracketSpanners, and from step 21 10 VerticalAxisGroups, 10
//   VerticalAlignments, 71 BreakAlignments, 270 BreakAlignGroups and 71
//   LeftEdges (61 from Bar_engraver, 10 from Clef_engraver), and from step
//   22 69 Clefs, 59 KeySignatures, 71 StaffSpacings, 10 SpacingSpanners and
//   125 NoteSpacings: 1,913 lines, each type's count makers.txt's on both
//   sides.
// - The 10 with nothing forbidding a break (MS3 step 17), captured so and
//   compiled by the engine natively and in core.wasm: their grobs equal
//   LilyPond's with their columns, every line from step 22 on, 139 Clefs,
//   139 KeySignatures and 139 StaffSpacings, one of each at each command
//   column (each of the 10 has one staff).
// - The 10 with Bar_engraver the only one to forbid a break (MS3 step 18),
//   as in the engine: every line equals LilyPond's with its columns,
//   natively and in wasm, 71 Clefs and 61 KeySignatures among them; in
//   LilyPond's own layout,
//   Forbid_line_break_engraver (step 21) takes off T03's two of each, at
//   its bar lines under sustained notes, leaving makers.txt's 69 and 59.
// - T25's OttavaBracket (MS3 step 20), the one of the corpus, which none of
//   the 10 has: LilyPond's capture has it at granados.ly:56, from 2 to
//   35/16 in the Staff named high, and the engine's, natively and in wasm,
//   equals that line with its columns.
// - ly:rest::width (MS3 step 16, rest.cc), which a dotted rest's dot column
//   takes: the engine's widths of rests of each duration log, of the styles
//   glyph_name names and of a breve at font-size 3, given the build's
//   packs, equal LilyPond's natively and in wasm.
// - Part 2's 36 classes at work (MS3 step 22): a file of three scores in
//   the default contexts writes, for each class, the grobs it announced or
//   what it set (forbidBreak, rehearsalMark, busyGrobs, keep-alive-with,
//   neighbors); LilyPond's records show each at work, and the engine's,
//   natively and in wasm, equal them. crates/layout/tests/grobs.rs checks
//   that each name resolves to a class with work, 35 of them layout's.
//
// Skipped, and so unverified, without LilyPond 2.26.0, the engine's without
// cargo or a build's packs, core.wasm's without a build as new as the
// sources, and the 10's without the corpus (tools/corpus/fetch.mjs).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { captureOne, GROB_CALLS, GROB_CALLS_LY, GROBS, lilypondVersion, PAGES } from '../capture.mjs'
import { compileInWasm, coreSkip } from '../core.mjs'
import { COLUMNS, compareGrobs, countTypes, readGrobs } from '../grobs.mjs'

/** The 10 scores of MS3's gate (README §5). */
const TEN = ['T01', 'T03', 'T05', 'T10', 'T13', 'T14', 'T16', 'T17', 'T18', 'T19']
const ten = corpus().filter((row) => TEN.includes(row.id))
const lilypond = lilypondVersion() === '2.26.0' ? false : 'LilyPond 2.26.0 is not installed'
const cargo = spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'
const skipTen = lilypond
  || (ten.length === TEN.length && ten.every((row) => existsSync(row.path)) ? false : 'the corpus is not fetched: run node tools/corpus/fetch.mjs')
const t25 = corpus().find((row) => row.id === 'T25')
const skipT25 = lilypond || (t25 && existsSync(t25.path) ? false : 'the corpus is not fetched: run node tools/corpus/fetch.mjs')

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-grobs-ref-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** A capture by hand: T01's first timestep but for its paper columns, then a translation of the fixture's grobs. */
const CAPTURE = `Clef\t-\t-\t0 0\tStaff#0\tClef_engraver
NoteHead\t<ev note-event>\tmelody.ily:2:2:2\t0 0\tVoice#0\tNote_heads_engraver
StaffSymbol\t-\t-\t0 0;2 0\tStaff#0\tStaff_symbol_engraver
Stem\t<NoteHead>\tmelody.ily:2:2:2\t0 0\tVoice#0\tStem_engraver
BreakAlignGroup\t<Clef>\t-\t0 0\tScore#0=\\new\tBreak_align_engraver
System\t-\t-\t0 0;2 0\tScore#0=\\new\tScore_engraver
Fingering\t<ev note-event>\tgrobs.ly:51:44:44\t0 0\tVoice#0=one\tScheme_engraver
StrokeFinger\t-\t-\t0 0\tScore#0\tScheme_engraver
StringNumber\t<Fingering>\tgrobs.ly:51:44:44\t0 0\tStaff#0\tScheme_engraver
Fingering\t<ev note-event>\tgrobs.ly:51:55:55\t1/4 -1/8\tVoice#0=one\tScheme_engraver
System\t-\t-\t0 0;1 0\tScore#0\tScore_engraver
`

test('grobs.mjs: a capture against itself passes; a changed field, order, a dropped or an added grob fails', () => {
  const all = { types: null, columns: true }
  const self = compareGrobs(CAPTURE, CAPTURE, all)
  assert.deepEqual(self, { equal: true, counts: [11, 11], missing: [], extra: [], order: [], differ: [] })
  assert.deepEqual(readGrobs(CAPTURE).map((t) => t.length), [6, 5])
  assert.equal(readGrobs(CAPTURE)[0][1].link, 'melody.ily:2:2:2')
  assert.equal(countTypes(readGrobs(CAPTURE)).get('System'), 2)
  // Each perturbation on the line it changes, and the field it changes.
  const fields = {
    type: [CAPTURE.replace('Stem\t<NoteHead>', 'Flag\t<NoteHead>'), 4, 'Stem', 'Flag'],
    cause: [CAPTURE.replace('StringNumber\t<Fingering>', 'StringNumber\t<ev note-event>'), 9, '<Fingering>', '<ev note-event>'],
    link: [CAPTURE.replace('Fingering\t<ev note-event>\tgrobs.ly:51:55:55', 'Fingering\t<ev note-event>\tgrobs.ly:51:56:56'), 10, 'grobs.ly:51:55:55', 'grobs.ly:51:56:56'],
    columns: [CAPTURE.replace('51:55:55\t1/4 -1/8', '51:55:55\t1/4 0'), 10, '1/4 -1/8', '1/4 0'],
    context: [CAPTURE.replace('Voice#0=one', 'Voice#0'), 7, 'Voice#0=one', 'Voice#0'],
    engraver: [CAPTURE.replace('Staff_symbol_engraver', 'Axis_group_engraver'), 3, 'Staff_symbol_engraver', 'Axis_group_engraver'],
  }
  for (const [field, [copy, line, reference, value]] of Object.entries(fields)) {
    assert.notEqual(copy, CAPTURE, field)
    const result = compareGrobs(CAPTURE, copy, all)
    assert.equal(result.equal, false, field)
    assert.deepEqual([result.missing, result.extra, result.order], [[], [], []], field)
    assert.deepEqual(result.differ, [{ translation: line > 6 ? 2 : 1, line, got: line, field, reference, value }], field)
  }
  // Out of order: the Clef after the note head; then each translation's own lines.
  const lines = CAPTURE.split('\n')
  const swapped = [lines[1], lines[0], ...lines.slice(2)].join('\n')
  const order = compareGrobs(CAPTURE, swapped, all)
  assert.equal(order.equal, false)
  assert.deepEqual([order.missing, order.extra, order.differ], [[], [], []])
  assert.equal(order.order.length, 1)
  assert.ok([1, 2].includes(order.order[0].line))
  const dropped = compareGrobs(CAPTURE, lines.filter((_, i) => i !== 8).join('\n'), all)
  assert.deepEqual([dropped.missing.map((m) => [m.translation, m.line]), dropped.extra, dropped.order, dropped.differ], [[[2, 9]], [], [], []])
  const added = compareGrobs(CAPTURE, CAPTURE.replace('System\t-\t-\t0 0;1 0', `${lines[6]}\nSystem\t-\t-\t0 0;1 0`), all)
  assert.deepEqual([added.extra.map((m) => [m.translation, m.line]), added.missing, added.order, added.differ], [[[2, 11]], [], [], []])
  // A translation missing: each of its grobs.
  const first = compareGrobs(CAPTURE, lines.slice(0, 6).join('\n'), all)
  assert.deepEqual(first.missing.map((m) => m.line), [7, 8, 9, 10, 11])
  // From step 15 on the columns are compared unless asked not to; from
  // step 22 on every type unless asked for some.
  assert.equal(COLUMNS, true)
  assert.equal(compareGrobs(CAPTURE, fields.columns[0]).equal, false)
  assert.equal(compareGrobs(CAPTURE, fields.columns[0], { columns: false }).equal, true)
  const clef = CAPTURE.replace('Clef\t-', 'KeySignature\t-')
  assert.deepEqual(compareGrobs(CAPTURE, clef).differ.map((d) => [d.line, d.field, d.reference, d.value]), [[1, 'type', 'Clef', 'KeySignature']])
  assert.equal(compareGrobs(CAPTURE, clef, { types: ['NoteHead', 'Stem'] }).equal, true)
  assert.deepEqual(compareGrobs(CAPTURE, CAPTURE, { types: ['NoteHead', 'Stem'] }).counts, [2, 2])
  assert.deepEqual(compareGrobs(CAPTURE, CAPTURE).counts, [11, 11])
})

const fixture = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../fixtures/grobs.ly')
const engine = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
let captured
/** LilyPond's capture of the fixture into scratch/grobs, made once: its result. */
const captureFixture = () => captured ??= captureOne('lilypond', { id: 'grobs', path: fixture }, path.join(scratch, 'grobs'))

test("LilyPond captures the grobs the fixture's Scheme engravers make, from events and from grobs", { skip: lilypond, timeout: 120_000 }, async () => {
  const dir = path.join(scratch, 'grobs')
  const result = await captureFixture()
  assert.equal(result.exit, 0)
  assert.equal(readFileSync(path.join(dir, 'stderr.txt'), 'utf8'), '')
  const text = readFileSync(path.join(dir, GROBS), 'utf8')
  const translations = readGrobs(text)
  assert.equal(translations.length, 1)
  assert.equal(translations[0].length, result.grobs)
  // Every line, its columns too: the two paper columns of each timestep, a
  // NoteHead of each note and a Rest of each rest, a Stem, a StemStub, a
  // NoteSpacing and a NoteColumn of each timestep's first note or rest in
  // a voice, caused by it, and a Flag of the grace eighth's stem, the
  // GraceSpacing from the grace's columns to the next, a LedgerLineSpanner,
  // a StaffSymbol and a VerticalAxisGroup of each staff from the first
  // command column to the last, a Clef and a StaffSpacing of each staff at
  // both, a TimeSignature at the first and a KeySignature at the last, the
  // Score's SystemStartBar, SpacingSpanner and VerticalAlignment from the
  // first command column to the last, a BarLine of each staff and the
  // Score's BarNumber at the last, a BreakAlignment at each command column
  // with the first staff's LeftEdge (Clef_engraver's at the first,
  // Bar_engraver's at the last) and a BreakAlignGroup of each break-aligned
  // type there (the LeftEdge, the Clef and the TimeSignature; the LeftEdge,
  // the BarLine, the Clef and the KeySignature), the ScriptColumn and
  // the two FingeringColumns at the chord, a Fingering of each
  // note (the grace note's at 1/4 -1/8, the chord's two), a StringNumber of
  // each, a ScriptRow of each staff's timestep with a Fingering and its
  // StringNumber (two side positioned items), caused by its first
  // Fingering, the third staff's DynamicText of each of its two dynamics
  // and its Hairpin from the first rest to the last, which the last's
  // dynamic ends, made before the first DynamicText, the
  // DynamicLineSpanner over them, caused by the Hairpin, a StrokeFinger of
  // each StringNumber,
  // linked through both causes to the note, and one with no cause; the
  // System from the first command column to the last.
  const lines = translations[0].map((g) => Object.values(g).slice(1).join('\t'))
  const columns = (moment) => ['NonMusicalPaperColumn', 'PaperColumn'].map((type) => `${type}\t-\t-\t${moment}\tScore#0\tPaper_column_engraver`)
  assert.deepEqual(lines, [
    ...columns('0 0'),
    'NoteHead\t<ev note-event>\tgrobs.ly:51:44:44\t0 0\tVoice#0=one\tNote_heads_engraver',
    'Fingering\t<ev note-event>\tgrobs.ly:51:44:44\t0 0\tVoice#0=one\tScheme_engraver',
    'Clef\t-\t-\t0 0\tStaff#0\tClef_engraver',
    'TimeSignature\t-\t-\t0 0\tStaff#0\tTime_signature_engraver',
    'LedgerLineSpanner\t-\t-\t0 0;1 0\tStaff#0\tLedger_line_engraver',
    'StaffSymbol\t-\t-\t0 0;1 0\tStaff#0\tStaff_symbol_engraver',
    'VerticalAxisGroup\t-\t-\t0 0;1 0\tStaff#0\tAxis_group_engraver',
    'Rest\t<ev rest-event>\tgrobs.ly:52:25:25\t0 0\tVoice#1\tRest_engraver',
    'Clef\t-\t-\t0 0\tStaff#1\tClef_engraver',
    'TimeSignature\t-\t-\t0 0\tStaff#1\tTime_signature_engraver',
    'LedgerLineSpanner\t-\t-\t0 0;1 0\tStaff#1\tLedger_line_engraver',
    'StaffSymbol\t-\t-\t0 0;1 0\tStaff#1\tStaff_symbol_engraver',
    'VerticalAxisGroup\t-\t-\t0 0;1 0\tStaff#1\tAxis_group_engraver',
    'Rest\t<ev rest-event>\tgrobs.ly:56:15:15\t0 0\tVoice#2\tRest_engraver',
    'Hairpin\t<ev crescendo-event>\tgrobs.ly:56:19:19\t0 0;1/2 0\tVoice#2\tDynamic_engraver',
    'DynamicText\t<ev absolute-dynamic-event>\tgrobs.ly:56:17:17\t0 0\tVoice#2\tDynamic_engraver',
    'Clef\t-\t-\t0 0\tStaff#2\tClef_engraver',
    'TimeSignature\t-\t-\t0 0\tStaff#2\tTime_signature_engraver',
    'LedgerLineSpanner\t-\t-\t0 0;1 0\tStaff#2\tLedger_line_engraver',
    'StaffSymbol\t-\t-\t0 0;1 0\tStaff#2\tStaff_symbol_engraver',
    'VerticalAxisGroup\t-\t-\t0 0;1 0\tStaff#2\tAxis_group_engraver',
    'SystemStartBar\t-\t-\t0 0;1 0\tScore#0\tSystem_start_delimiter_engraver',
    'SpacingSpanner\t-\t-\t0 0;1 0\tScore#0\tSpacing_engraver',
    'VerticalAlignment\t-\t-\t0 0;1 0\tScore#0\tVertical_align_engraver',
    'StrokeFinger\t-\t-\t0 0\tScore#0\tScheme_engraver',
    'Stem\t<NoteHead>\tgrobs.ly:51:44:44\t0 0\tVoice#0=one\tStem_engraver',
    'StemStub\t<NoteHead>\tgrobs.ly:51:44:44\t0 0\tVoice#0=one\tStem_engraver',
    'NoteSpacing\t<NoteHead>\tgrobs.ly:51:44:44\t0 0\tVoice#0=one\tNote_spacing_engraver',
    'NoteColumn\t<NoteHead>\tgrobs.ly:51:44:44\t0 0\tVoice#0=one\tRhythmic_column_engraver',
    'StringNumber\t<Fingering>\tgrobs.ly:51:44:44\t0 0\tStaff#0\tScheme_engraver',
    'StaffSpacing\t-\t-\t0 0\tStaff#0\tSeparating_line_group_engraver',
    'ScriptRow\t<Fingering>\tgrobs.ly:51:44:44\t0 0\tStaff#0\tScript_row_engraver',
    'Stem\t<Rest>\tgrobs.ly:52:25:25\t0 0\tVoice#1\tStem_engraver',
    'StemStub\t<Rest>\tgrobs.ly:52:25:25\t0 0\tVoice#1\tStem_engraver',
    'NoteSpacing\t<Rest>\tgrobs.ly:52:25:25\t0 0\tVoice#1\tNote_spacing_engraver',
    'NoteColumn\t<Rest>\tgrobs.ly:52:25:25\t0 0\tVoice#1\tRhythmic_column_engraver',
    'StaffSpacing\t-\t-\t0 0\tStaff#1\tSeparating_line_group_engraver',
    'Stem\t<Rest>\tgrobs.ly:56:15:15\t0 0\tVoice#2\tStem_engraver',
    'StemStub\t<Rest>\tgrobs.ly:56:15:15\t0 0\tVoice#2\tStem_engraver',
    'NoteSpacing\t<Rest>\tgrobs.ly:56:15:15\t0 0\tVoice#2\tNote_spacing_engraver',
    'DynamicLineSpanner\t<Hairpin>\tgrobs.ly:56:19:19\t0 0;1/2 0\tVoice#2\tDynamic_align_engraver',
    'NoteColumn\t<Rest>\tgrobs.ly:56:15:15\t0 0\tVoice#2\tRhythmic_column_engraver',
    'StaffSpacing\t-\t-\t0 0\tStaff#2\tSeparating_line_group_engraver',
    'BreakAlignment\t-\t-\t0 0\tScore#0\tBreak_align_engraver',
    'LeftEdge\t-\t-\t0 0\tStaff#0\tClef_engraver',
    'BreakAlignGroup\t<LeftEdge>\t-\t0 0\tScore#0\tBreak_align_engraver',
    'BreakAlignGroup\t<Clef>\t-\t0 0\tScore#0\tBreak_align_engraver',
    'BreakAlignGroup\t<TimeSignature>\t-\t0 0\tScore#0\tBreak_align_engraver',
    'StrokeFinger\t<StringNumber>\tgrobs.ly:51:44:44\t0 0\tScore#0\tScheme_engraver',
    ...columns('1/4 -1/8'),
    'NoteHead\t<ev note-event>\tgrobs.ly:51:55:55\t1/4 -1/8\tVoice#0=one\tNote_heads_engraver',
    'Fingering\t<ev note-event>\tgrobs.ly:51:55:55\t1/4 -1/8\tVoice#0=one\tScheme_engraver',
    'GraceSpacing\t-\t-\t1/4 -1/8;1/4 0\tScore#0\tGrace_spacing_engraver',
    'Stem\t<NoteHead>\tgrobs.ly:51:55:55\t1/4 -1/8\tVoice#0=one\tStem_engraver',
    'StemStub\t<NoteHead>\tgrobs.ly:51:55:55\t1/4 -1/8\tVoice#0=one\tStem_engraver',
    'Flag\t<Stem>\tgrobs.ly:51:55:55\t1/4 -1/8\tVoice#0=one\tStem_engraver',
    'NoteSpacing\t<NoteHead>\tgrobs.ly:51:55:55\t1/4 -1/8\tVoice#0=one\tNote_spacing_engraver',
    'NoteColumn\t<NoteHead>\tgrobs.ly:51:55:55\t1/4 -1/8\tVoice#0=one\tRhythmic_column_engraver',
    'StringNumber\t<Fingering>\tgrobs.ly:51:55:55\t1/4 -1/8\tStaff#0\tScheme_engraver',
    'ScriptRow\t<Fingering>\tgrobs.ly:51:55:55\t1/4 -1/8\tStaff#0\tScript_row_engraver',
    'StrokeFinger\t<StringNumber>\tgrobs.ly:51:55:55\t1/4 -1/8\tScore#0\tScheme_engraver',
    ...columns('1/4 0'),
    'NoteHead\t<ev note-event>\tgrobs.ly:51:58:58\t1/4 0\tVoice#0=one\tNote_heads_engraver',
    'Fingering\t<ev note-event>\tgrobs.ly:51:58:58\t1/4 0\tVoice#0=one\tScheme_engraver',
    'NoteHead\t<ev note-event>\tgrobs.ly:52:28:28\t1/4 0\tVoice#1\tNote_heads_engraver',
    'Fingering\t<ev note-event>\tgrobs.ly:52:28:28\t1/4 0\tVoice#1\tScheme_engraver',
    'Rest\t<ev rest-event>\tgrobs.ly:56:22:22\t1/4 0\tVoice#2\tRest_engraver',
    'Stem\t<NoteHead>\tgrobs.ly:51:58:58\t1/4 0\tVoice#0=one\tStem_engraver',
    'StemStub\t<NoteHead>\tgrobs.ly:51:58:58\t1/4 0\tVoice#0=one\tStem_engraver',
    'NoteSpacing\t<NoteHead>\tgrobs.ly:51:58:58\t1/4 0\tVoice#0=one\tNote_spacing_engraver',
    'NoteColumn\t<NoteHead>\tgrobs.ly:51:58:58\t1/4 0\tVoice#0=one\tRhythmic_column_engraver',
    'StringNumber\t<Fingering>\tgrobs.ly:51:58:58\t1/4 0\tStaff#0\tScheme_engraver',
    'ScriptRow\t<Fingering>\tgrobs.ly:51:58:58\t1/4 0\tStaff#0\tScript_row_engraver',
    'Stem\t<NoteHead>\tgrobs.ly:52:28:28\t1/4 0\tVoice#1\tStem_engraver',
    'StemStub\t<NoteHead>\tgrobs.ly:52:28:28\t1/4 0\tVoice#1\tStem_engraver',
    'NoteSpacing\t<NoteHead>\tgrobs.ly:52:28:28\t1/4 0\tVoice#1\tNote_spacing_engraver',
    'NoteColumn\t<NoteHead>\tgrobs.ly:52:28:28\t1/4 0\tVoice#1\tRhythmic_column_engraver',
    'StringNumber\t<Fingering>\tgrobs.ly:52:28:28\t1/4 0\tStaff#1\tScheme_engraver',
    'ScriptRow\t<Fingering>\tgrobs.ly:52:28:28\t1/4 0\tStaff#1\tScript_row_engraver',
    'Stem\t<Rest>\tgrobs.ly:56:22:22\t1/4 0\tVoice#2\tStem_engraver',
    'StemStub\t<Rest>\tgrobs.ly:56:22:22\t1/4 0\tVoice#2\tStem_engraver',
    'NoteSpacing\t<Rest>\tgrobs.ly:56:22:22\t1/4 0\tVoice#2\tNote_spacing_engraver',
    'NoteColumn\t<Rest>\tgrobs.ly:56:22:22\t1/4 0\tVoice#2\tRhythmic_column_engraver',
    'StrokeFinger\t<StringNumber>\tgrobs.ly:51:58:58\t1/4 0\tScore#0\tScheme_engraver',
    'StrokeFinger\t<StringNumber>\tgrobs.ly:52:28:28\t1/4 0\tScore#0\tScheme_engraver',
    ...columns('1/2 0'),
    'NoteHead\t<ev note-event>\tgrobs.ly:51:62:62\t1/2 0\tVoice#0=one\tNote_heads_engraver',
    'NoteHead\t<ev note-event>\tgrobs.ly:51:64:64\t1/2 0\tVoice#0=one\tNote_heads_engraver',
    'Fingering\t<ev note-event>\tgrobs.ly:51:62:62\t1/2 0\tVoice#0=one\tScheme_engraver',
    'Fingering\t<ev note-event>\tgrobs.ly:51:64:64\t1/2 0\tVoice#0=one\tScheme_engraver',
    'Rest\t<ev rest-event>\tgrobs.ly:52:31:31\t1/2 0\tVoice#1\tRest_engraver',
    'Rest\t<ev rest-event>\tgrobs.ly:56:25:25\t1/2 0\tVoice#2\tRest_engraver',
    'DynamicText\t<ev absolute-dynamic-event>\tgrobs.ly:56:27:27\t1/2 0\tVoice#2\tDynamic_engraver',
    'Stem\t<NoteHead>\tgrobs.ly:51:62:62\t1/2 0\tVoice#0=one\tStem_engraver',
    'StemStub\t<NoteHead>\tgrobs.ly:51:62:62\t1/2 0\tVoice#0=one\tStem_engraver',
    'NoteSpacing\t<NoteHead>\tgrobs.ly:51:62:62\t1/2 0\tVoice#0=one\tNote_spacing_engraver',
    'ScriptColumn\t<Fingering>\tgrobs.ly:51:62:62\t1/2 0\tVoice#0=one\tScript_column_engraver',
    'NoteColumn\t<NoteHead>\tgrobs.ly:51:62:62\t1/2 0\tVoice#0=one\tRhythmic_column_engraver',
    'StringNumber\t<Fingering>\tgrobs.ly:51:62:62\t1/2 0\tStaff#0\tScheme_engraver',
    'StringNumber\t<Fingering>\tgrobs.ly:51:64:64\t1/2 0\tStaff#0\tScheme_engraver',
    'ScriptRow\t<Fingering>\tgrobs.ly:51:62:62\t1/2 0\tStaff#0\tScript_row_engraver',
    'FingeringColumn\t-\t-\t-\tStaff#0\tFingering_column_engraver',
    'FingeringColumn\t-\t-\t-\tStaff#0\tFingering_column_engraver',
    'Stem\t<Rest>\tgrobs.ly:52:31:31\t1/2 0\tVoice#1\tStem_engraver',
    'StemStub\t<Rest>\tgrobs.ly:52:31:31\t1/2 0\tVoice#1\tStem_engraver',
    'NoteSpacing\t<Rest>\tgrobs.ly:52:31:31\t1/2 0\tVoice#1\tNote_spacing_engraver',
    'NoteColumn\t<Rest>\tgrobs.ly:52:31:31\t1/2 0\tVoice#1\tRhythmic_column_engraver',
    'Stem\t<Rest>\tgrobs.ly:56:25:25\t1/2 0\tVoice#2\tStem_engraver',
    'StemStub\t<Rest>\tgrobs.ly:56:25:25\t1/2 0\tVoice#2\tStem_engraver',
    'NoteSpacing\t<Rest>\tgrobs.ly:56:25:25\t1/2 0\tVoice#2\tNote_spacing_engraver',
    'NoteColumn\t<Rest>\tgrobs.ly:56:25:25\t1/2 0\tVoice#2\tRhythmic_column_engraver',
    'StrokeFinger\t<StringNumber>\tgrobs.ly:51:62:62\t1/2 0\tScore#0\tScheme_engraver',
    'StrokeFinger\t<StringNumber>\tgrobs.ly:51:64:64\t1/2 0\tScore#0\tScheme_engraver',
    ...columns('1 0'),
    'BarLine\t-\t-\t1 0\tStaff#0\tBar_engraver',
    'Clef\t-\t-\t1 0\tStaff#0\tClef_engraver',
    'KeySignature\t-\t-\t1 0\tStaff#0\tKey_engraver',
    'BarLine\t-\t-\t1 0\tStaff#1\tBar_engraver',
    'Clef\t-\t-\t1 0\tStaff#1\tClef_engraver',
    'KeySignature\t-\t-\t1 0\tStaff#1\tKey_engraver',
    'BarLine\t-\t-\t1 0\tStaff#2\tBar_engraver',
    'Clef\t-\t-\t1 0\tStaff#2\tClef_engraver',
    'KeySignature\t-\t-\t1 0\tStaff#2\tKey_engraver',
    'BarNumber\t-\t-\t1 0\tScore#0\tBar_number_engraver',
    'StaffSpacing\t-\t-\t1 0\tStaff#0\tSeparating_line_group_engraver',
    'StaffSpacing\t-\t-\t1 0\tStaff#1\tSeparating_line_group_engraver',
    'StaffSpacing\t-\t-\t1 0\tStaff#2\tSeparating_line_group_engraver',
    'BreakAlignment\t-\t-\t1 0\tScore#0\tBreak_align_engraver',
    'LeftEdge\t-\t-\t1 0\tStaff#0\tBar_engraver',
    'BreakAlignGroup\t<LeftEdge>\t-\t1 0\tScore#0\tBreak_align_engraver',
    'BreakAlignGroup\t<BarLine>\t-\t1 0\tScore#0\tBreak_align_engraver',
    'BreakAlignGroup\t<Clef>\t-\t1 0\tScore#0\tBreak_align_engraver',
    'BreakAlignGroup\t<KeySignature>\t-\t1 0\tScore#0\tBreak_align_engraver',
    'System\t-\t-\t0 0;1 0\tScore#0\tScore_engraver',
  ])
  assert.equal(compareGrobs(text, text).equal, true)
})

test("the engine's grobs of the fixture, from a native traced compile, equal LilyPond's on every line, their columns too (none for the FingeringColumns, as LilyPond's)", { skip: lilypond || cargo, timeout: 300_000 }, async () => {
  const result = await captureFixture()
  assert.equal(result.exit, 0)
  const reference = readFileSync(path.join(scratch, 'grobs', GROBS), 'utf8')
  const out = mkdtempSync(path.join(scratch, 'engine-'))
  const run = runExample('compile', ['--trace', '--out', out, fixture], { cwd: path.dirname(fixture), encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  const read = (ext) => readFileSync(path.join(out, `grobs.${ext}`), 'utf8')
  // Its layout completes since MS3 step 53, made with the script and
  // fingering columns: no read of the dump warns, as LilyPond does not
  // (the hairpin's "crescendo too small" of step 49's Open gone).
  assert.deepEqual([read('err'), read('trace.log')], ['', ''])
  const text = read('grobs.tsv')
  // The engine makes every grob of LilyPond's, each on its columns but the
  // two FingeringColumns, which have none, as LilyPond's.
  const grobs = readGrobs(text)
  assert.equal(grobs.length, 1)
  assert.deepEqual(grobs[0].filter((g) => g.columns === '-').map((g) => g.type), ['FingeringColumn', 'FingeringColumn'])
  assert.deepEqual(compareGrobs(reference, text), { equal: true, counts: [140, 140], missing: [], extra: [], order: [], differ: [] })
})

test("core.wasm's grobs of the fixture, from a traced compile in one instance, equal LilyPond's on every line, their columns too (none for the FingeringColumns, as LilyPond's)", { skip: lilypond || coreSkip(), timeout: 300_000 }, async () => {
  const result = await captureFixture()
  assert.equal(result.exit, 0)
  const reference = readFileSync(path.join(scratch, 'grobs', GROBS), 'utf8')
  const [{ outputs }] = await compileInWasm([fixture], { trace: true })
  assert.equal(outputs['trace.log'].toString(), '')
  assert.ok(GROBS in outputs, Object.keys(outputs).join(' '))
  const text = outputs[GROBS].toString()
  const grobs = readGrobs(text)
  assert.equal(grobs.length, 1)
  assert.deepEqual(grobs[0].filter((g) => g.columns === '-').map((g) => g.type), ['FingeringColumn', 'FingeringColumn'])
  assert.deepEqual(compareGrobs(reference, text), { equal: true, counts: [140, 140], missing: [], extra: [], order: [], differ: [] })
})

/** Some of grob-calls.ly's calls, as Guile writes them, and their values in the order the calls run. */
const ANSWERS = [
  // A property set on the grob before its definition's; ly:grob-property's default for none and for '().
  ["(ly:grob-property (fingering 0) (quote direction))", ['1']],
  ["(ly:grob-property (fingering 0) (quote font-size))", ['-5']],
  ["(ly:grob-property (fingering 0) (quote no-such-property) (quote fallback))", ['fallback']],
  ["(ly:grob-property (fingering 0) (quote staff-padding) (quote fallback))", ['fallback']],
  ["(ly:grob-property (string-number 0) (quote cause))", ['(grob Fingering 0)']],
  // A callback is called once and its value kept; a procedure it returns is called at the next read.
  ["(ly:grob-property (fingering 1) (quote Y-offset))", ['-0.5', '-0.5']],
  ['grob-calls-count', ['1', '1']],
  ["(ly:grob-property (fingering 1) (quote staff-padding))", ['procedure', '2.5']],
  ["(ly:grob-property (fingering 1) (quote extra-offset))", ['(0.75 . 0.5)']],
  ["(ly:grob-property (fingering 1) (quote minimum-Y-extent))", ['(1 . 3)', '(1 . 3)']],
  ["(ly:grob-property (fingering 1) (quote slur-padding))", ['(error grob-calls-thrown)', 'calculation-in-progress', '0.2']],
  // The chains.
  ["(ly:grob-property (fingering 2) (quote staff-padding))", ['3.5']],
  ["(ly:grob-property (fingering 2) (quote X-padding))", ['2']],
  ["(ly:grob-property (fingering 2) (quote Y-extent))", ['(0.0 . 2.0)']],
  ["(ly:grob-property (string-number 2) (quote X-offset))", ['6']],
  ["(ly:grob-property (string-number 2) (quote padding))", ['3']],
  // The cycles.
  ["(ly:grob-property (fingering 3) (quote X-offset))", ['calculation-in-progress', 'calculation-in-progress']],
  ["(ly:grob-property (string-number 3) (quote Y-offset))", ['calculation-in-progress']],
  // Parents, offsets and extents.
  ["(ly:grob-property (fingering 0) (quote X-extent))", ['(0.0 . 1.0)']],
  ["(ly:grob-relative-coordinate (stroke-finger 1) (fingering 0) Y)", ['3.0']],
  ["(ly:grob-extent (stroke-finger 1) (fingering 0) Y)", ['(1.0 . 3.5)']],
  ["(ly:grob-extent (stroke-finger 0) (fingering 0) X)", ['(+inf.0 . -inf.0)']],
  ["(ly:grob-property (stroke-finger 0) (quote X-offset))", ['()']],
  ['grob-calls-positioning', ['1']],
  ["(ly:grob-common-refpoint (stroke-finger 1) (stroke-finger 0) X)", ['(grob Fingering 0)']],
  ["(ly:grob-relative-coordinate (stroke-finger 1) (fingering 0) X)", ['2.25', '2.75']],
  ["(ly:grob-extent (string-number 0) (fingering 0) X)", ['(1.0 . 2.0)', '(1.0 . 2.0)']],
  ["(ly:grob-relative-coordinate (stroke-finger 2) (string-number 1) X)", ['1.0']],
  ["(ly:grob-extent (fingering 4) (fingering 0) Y)", ['(0.0 . 1.0)']],
  // A dead grob keeps its cause and takes no property.
  ["(ly:grob-property (fingering 4) (quote cause))", ['(event note-event)']],
  ["(grob::name (fingering 4))", ['#f']],
  ["(ly:grob-property (fingering 4) (quote text))", ['()', '()']],
  // MS3 step 14: the class's interfaces in front of the definition's, a
  // dead grob's none; a grob array in its order, a pointer group's; a
  // container's pure part not kept; nested properties consed in front.
  ["(ly:grob-interfaces (fingering 0))", ['(item-interface grob-interface finger-interface font-interface outside-staff-interface self-alignment-interface side-position-interface text-interface text-script-interface)']],
  ["(ly:grob-interfaces (fingering 4))", ['()']],
  ["(ly:grob-list->grob-array (list (fingering 0) (string-number 0) (fingering 0)))", ['(grob-array (grob Fingering 0) (grob StringNumber 0) (grob Fingering 0))']],
  ["(ly:grob-object (fingering 3) (quote grob-calls-elements))", ['()', '(grob-array (grob StringNumber 3) (grob StrokeFinger 3))']],
  ["(ly:grob-pure-property (string-number 5) (quote X-padding) 2 3)", ['5.5', '1']],
  ["(ly:grob-property (fingering 3) (quote details))", ['((grob-calls (a . 1)))', '((grob-calls (a . 3) (b . 2) (a . 1)))', '(chained ((grob-calls (a . 3) (b . 2) (a . 1))))']],
  ["(ly:grob-pure-height (stroke-finger 1) (string-number 0) 0 1)", ['(-1.0 . 1.5)']],
  // MS3 step 15: a grob's column, dead or alive, the one its X parents
  // reach; no column's system before line breaking; a column's rank.
  ["(ly:item-get-column (fingering 4))", ['(grob PaperColumn)']],
  ["(eq? (ly:item-get-column (fingering 0)) (ly:item-get-column (stroke-finger 0)))", ['#t']],
  ["(ly:grob-property (ly:item-get-column (fingering 1)) (quote when))", ['#<Mom 1/4G-1/8>']],
  ["(ly:grob-system (fingering 0))", ['()']],
  ["(ly:grob-spanned-column-rank-interval (fingering 4))", ['(7 . 7)']],
  ["(ly:grob-spanned-column-rank-interval (ly:item-get-column (fingering 1)))", ['(3 . 3)']],
  // MS3 step 14a: a grob's default font, through its layout's parent.
  ["(ly:font-name (ly:grob-default-font (fingering 3)))", ['"unknown"']],
  ["(ly:font-magnification (ly:grob-default-font (fingering 3)))", ['1.0']],
]

/** grob-calls.ly's warnings, in the calls' order, the fixture's directory taken out. */
const WARNINGS = [
  'grobs.ly:51:56: programming error: cyclic dependency: calculation-in-progress encountered for Fingering.slur-padding',
  'grobs.ly:52:29: programming error: cyclic dependency: calculation-in-progress encountered for Fingering.X-offset',
  'grobs.ly:52:29: programming error: cyclic dependency: calculation-in-progress encountered for Fingering.X-offset',
  'grobs.ly:52:29: programming error: cyclic dependency: calculation-in-progress encountered for StringNumber.X-offset',
  'grobs.ly:52:29: programming error: cyclic dependency: calculation-in-progress encountered for StringNumber.Y-offset',
  'grobs.ly:51:63: warning: ignored infinite Y-offset',
  'grobs.ly:51:63: warning: ignored infinite X-offset',
  'warning: ignored infinite Y-offset',
  'grobs.ly:51:65: programming error: Infinity or NaN encountered',
  'warning: ly:grob-list->grob-array encountered a non-grob object',
  'warning: ly:grob-list->grob-array expected a list',
]

test("LilyPond answers grob-calls.ly's calls on the fixture's grobs, the same twice, with the links of their print stencils",
  { skip: lilypond, timeout: 120_000 }, async () => {
    const row = { id: 'grob-calls', path: GROB_CALLS_LY }
    const dirs = [path.join(scratch, 'grob-calls-1'), path.join(scratch, 'grob-calls-2')]
    const [result] = await Promise.all(dirs.map((dir) => captureOne('lilypond', row, dir)))
    assert.equal(result.exit, 0)
    const read = (file, dir = dirs[0]) => readFileSync(path.join(dir, file), 'utf8')
    for (const file of [GROB_CALLS, 'stderr.txt', PAGES, GROBS, 'grob-calls.svg']) {
      assert.equal(read(file, dirs[1]), read(file), `${file} the same twice`)
    }
    // The fixture's grobs as the fixture's own capture has them, but the
    // three the calls kill, which have no column left (grobs.ly).
    await captureFixture()
    const fixtureGrobs = readFileSync(path.join(scratch, 'grobs', GROBS), 'utf8')
    assert.equal(compareGrobs(fixtureGrobs, read(GROBS), { columns: false }).equal, true)
    const dead = compareGrobs(fixtureGrobs, read(GROBS))
    assert.deepEqual([dead.missing, dead.extra, dead.order], [[], [], []])
    assert.deepEqual(dead.differ.map((d) => [d.field, d.reference, d.value]), [['columns', '0 0', '-'], ['columns', '1/2 0', '-'], ['columns', '1/2 0', '-']])

    const records = read(GROB_CALLS).split('\n').filter(Boolean).map((line) => line.split('\t'))
    assert.ok(records.every((record) => record.length === 2))
    assert.equal(records.length, 297)
    const answers = (call) => records.filter(([c]) => c === call).map(([, value]) => value)
    for (const [call, values] of ANSWERS) assert.deepEqual(answers(call), values, call)

    const fixtures = path.join(path.dirname(fixture), path.sep)
    const warnings = read('stderr.txt').split('\n').filter((line) => /(warning|error): /.test(line))
    assert.deepEqual(warnings.map((line) => line.replaceAll(fixtures, '')), WARNINGS)

    // Each grob's link as output-svg.scm's grob-cause makes it: a Fingering's
    // names its note, a grob caused by a grob or by nothing has none; the
    // page's print stencil has it inside whiteout and color.
    const link = (grob) => JSON.parse(answers(`(grob-calls-link (${grob}))`)[0]).match(/xlink:href="([^"]*)"/)?.[1] ?? ''
    assert.equal(link('fingering 0'), `textedit://${fixtures}grobs.ly:51:44:45`)
    assert.deepEqual(['string-number 0', 'stroke-finger 0', 'stroke-finger 1'].map(link), ['', '', ''])
    const page = read(PAGES)
    assert.ok(page.includes(`(color (1 0 0 1.0) (grob-cause "${link('fingering 0')}" (combine-stencil (color (1 1 1 1.0) (round-filled-box -0.0 1.0 -0.0 2.0 0.0)) (draw-line 0.1 0 0 1 2))))`))
    assert.ok(page.includes('(color (0.0 0.5 0.0 1.0) (grob-cause #f (combine-stencil (color (0 0 1 1.0) (round-filled-box 0.7 0.7 0.19999999999999998 1.2 0.39999999999999997)) (round-filled-box 0.5 0.5 0 1 0.1))))'))
    assert.ok(page.includes(`(output-attributes ((id . "grob-calls")) (color (0.2 0.4 0.6 0.5) (rotate-stencil (30.0 (0.25 . 0.5)) (grob-cause "${link('fingering 1')}" (round-filled-box 0 0.5 0 1 0)))))`))
    // The transparent grob's stencil is not drawn, nor the dead grob's: its
    // note's link is on the note head only, the next note's on its Fingering too.
    assert.ok(!page.includes('(circle 0.5 0.1 #t)'))
    const links = (grob) => page.split(`(grob-cause "${link(grob)}" `).length - 1
    assert.deepEqual([links('fingering 4'), links('fingering 5')], [1, 2])
  })

/**
 * A file that includes the fixture and, as grob-calls.ly does, hears its
 * grobs in its Score and runs probe-grobs.scm in the Score's finalize, in
 * (lily), its records written to grob-calls.txt: the engine has no file
 * system, so the probe's text is in the file. It binds grob-calls-link to
 * the link the engine's writer gives a grob's print stencil, layout's
 * %grob-cause-link, in output-svg.scm's <a>, as grob-calls.ly binds it to
 * output-svg.scm's grob-cause.
 */
function probeDriver(dir) {
  const probe = readFileSync(path.resolve(path.dirname(fixture), '../../bench/probe-grobs.scm'), 'utf8')
  const file = path.join(dir, 'probe-grobs.ly')
  writeFileSync(file, `\\version "2.26.0"
#(define grob-calls-heard '())
#(define (grob-calls-link grob)
   (let ((link ((module-ref (resolve-module '(lily)) '%grob-cause-link) grob)))
     (if link
         (format #f "<a style=\\"color:inherit;\\" xlink:href=\\"~a\\">\\n" link)
         "")))
#(define grob-calls-probe "${probe.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}")
#(define (grob-calls-recorder context)
   (make-engraver
    (acknowledgers
     ((grob-interface engraver grob source-engraver)
      (when (memq (grob::name grob) '(Fingering StringNumber StrokeFinger))
        (set! grob-calls-heard (cons (cons grob (grob::name grob)) grob-calls-heard)))))
    ((finalize engraver)
     (module-define! (resolve-module '(lily)) 'grob-calls-heard (reverse grob-calls-heard))
     (module-define! (resolve-module '(lily)) 'grob-calls-link grob-calls-link)
     (with-output-to-file "${GROB_CALLS}"
       (lambda ()
         (save-module-excursion
          (lambda ()
            (call-with-input-string grob-calls-probe
              (lambda (port)
                (let next ((form (read port)))
                  (unless (eof-object? form)
                    (primitive-eval form)
                    (next (read port)))))))))))))
\\layout { \\context { \\Score \\consists #grob-calls-recorder } }
\\include "${fixture}"
`)
  return file
}

test("the probe's calls on the engine's grobs of the fixture give LilyPond's records from the image natively and in wasm",
  { skip: lilypond || cargo || coreSkip(), timeout: 600_000 }, async () => {
    const dir = path.join(scratch, 'probe-grobs')
    const result = await captureOne('lilypond', { id: 'grob-calls', path: GROB_CALLS_LY }, dir)
    assert.equal(result.exit, 0)
    const reference = readFileSync(path.join(dir, GROB_CALLS), 'utf8').split('\n').filter(Boolean)
    assert.equal(reference.length, 297)
    // The engine runs every call, the links too, in grob-calls.ly's order.
    assert.equal(reference.filter((record) => record.startsWith('(grob-calls-link ')).length, 10)
    const stderr = readFileSync(path.join(dir, 'stderr.txt'), 'utf8')
    assert.equal(stderr.split('\n').filter((line) => / programming error: /.test(line)).length, 6)

    // The pack of the default font's text face, which LilyPond's Pango
    // finds on its own (MS3 step 14a): emmentaler-11 for (fingering 3)'s
    // font-size of -5 at staff size 20.
    const pack = path.join(engine, 'dist/engine/fonts/emmentaler-11.pack')
    assert.ok(existsSync(pack), 'npm run build writes dist/engine/fonts')
    const driver = probeDriver(mkdtempSync(path.join(scratch, 'driver-')))
    const out = mkdtempSync(path.join(scratch, 'engine-'))
    const run = runExample('compile', ['--pack', pack, '--out', out, driver], { cwd: out, encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const [{ outputs }] = await compileInWasm([driver], { modules: [['emmentaler-11', readFileSync(pack)]] })
    for (const [name, got, err] of [
      ['from the image natively', readFileSync(path.join(out, GROB_CALLS), 'utf8'), readFileSync(path.join(out, 'probe-grobs.err'), 'utf8')],
      ['in wasm', outputs[GROB_CALLS]?.toString() ?? '', outputs.stderr.toString()],
    ]) {
      assert.deepEqual(got.split('\n').filter(Boolean), reference, `${name}: the records`)
      assert.equal(err, stderr, `${name}: stderr`)
    }
  })

/**
 * The grobs the 10's engravers announce, by type, engraver and context
 * name, summed over makers.txt's lines (MS3 step 11), and the System of
 * each translation.
 */
const MAKERS = [
  ['Accidental', 'Note_heads_engraver', 'Voice', 28],
  ['AccidentalPlacement', 'Accidental_engraver', 'Staff', 22],
  ['BarLine', 'Bar_engraver', 'Staff', 61],
  ['BarNumber', 'Bar_number_engraver', 'Score', 57],
  ['BreakAlignGroup', 'Break_align_engraver', 'Score', 270],
  ['BreakAlignment', 'Break_align_engraver', 'Score', 71],
  ['Clef', 'Clef_engraver', 'Staff', 69],
  ['DotColumn', 'Dot_column_engraver', 'Staff', 1],
  ['Dots', 'Dots_engraver', 'Voice', 1],
  ['DynamicLineSpanner', 'Dynamic_align_engraver', 'Voice', 5],
  ['DynamicText', 'Dynamic_engraver', 'Voice', 5],
  ['Hairpin', 'Dynamic_engraver', 'Voice', 3],
  ['KeySignature', 'Key_engraver', 'Staff', 59],
  ['LedgerLineSpanner', 'Ledger_line_engraver', 'Staff', 10],
  ['LeftEdge', 'Bar_engraver', 'Staff', 61],
  ['LeftEdge', 'Clef_engraver', 'Staff', 10],
  ['NonMusicalPaperColumn', 'Paper_column_engraver', 'Score', 139],
  ['NoteColumn', 'Rhythmic_column_engraver', 'Voice', 125],
  ['NoteHead', 'Note_heads_engraver', 'Voice', 198],
  ['NoteSpacing', 'Note_spacing_engraver', 'Voice', 125],
  ['PaperColumn', 'Paper_column_engraver', 'Score', 139],
  ['Rest', 'Rest_engraver', 'Voice', 4],
  ['ScriptRow', 'Script_row_engraver', 'Staff', 18],
  ['SpacingSpanner', 'Spacing_engraver', 'Score', 10],
  ['StaffSpacing', 'Separating_line_group_engraver', 'Staff', 71],
  ['StaffSymbol', 'Staff_symbol_engraver', 'Staff', 10],
  ['Stem', 'Stem_engraver', 'Voice', 125],
  ['StemStub', 'Stem_engraver', 'Voice', 125],
  ['SystemStartBar', 'System_start_delimiter_engraver', 'Score', 10],
  ['TextScript', 'Text_engraver', 'Voice', 35],
  ['TimeSignature', 'Time_signature_engraver', 'Staff', 10],
  ['VerticalAlignment', 'Vertical_align_engraver', 'Score', 10],
  ['VerticalAxisGroup', 'Axis_group_engraver', 'Staff', 10],
  ['VoltaBracket', 'Volta_engraver', 'Score', 4],
  ['VoltaBracketSpanner', 'Volta_engraver', 'Score', 2],
]
/** Each score's grobs announced, matrix.txt's total row (MS3 step 11). */
const ANNOUNCED = { T01: 80, T03: 100, T05: 685, T10: 165, T13: 61, T14: 40, T16: 55, T17: 408, T18: 185, T19: 124 }
let capturedTen
/** LilyPond's captures of the 10 into scratch/<id>, made once: each [id, grobs.tsv's text]. */
const captureTen = () => capturedTen ??= Promise.all(ten.map(async (row) => {
  const dir = path.join(scratch, row.id)
  const result = await captureOne('lilypond', row, dir)
  // T03 exits 1, as it must, after its page is written.
  assert.equal(result.exit, row.id === 'T03' ? 1 : 0, row.id)
  return [row.id, readFileSync(path.join(dir, GROBS), 'utf8')]
}))

test('the 10 scores announce 1,903 grobs of 34 types as makers.txt counts them, and 10 Systems', { skip: skipTen, timeout: 300_000 }, async () => {
  const all = (await captureTen()).map(([id, text]) => [id, readGrobs(text)])
  const counts = new Map()
  let types = new Set()
  for (const [id, translations] of all) {
    assert.equal(translations.length, 1, id)
    const grobs = translations[0]
    assert.equal(grobs.at(-1).type, 'System', id)
    assert.equal(grobs.length - 1, ANNOUNCED[id], id)
    for (const g of grobs) {
      const key = [g.type, g.engraver, g.context.replace(/#.*/, '')].join(' ')
      counts.set(key, (counts.get(key) ?? 0) + 1)
    }
    types = types.union(new Set(countTypes(translations).keys()))
  }
  const expected = new Map(MAKERS.map(([type, engraver, context, n]) => [`${type} ${engraver} ${context}`, n]))
  expected.set('System Score_engraver Score', 10)
  assert.deepEqual(new Map([...counts].sort()), new Map([...expected].sort()))
  assert.equal(MAKERS.reduce((n, m) => n + m[3], 0), 1903)
  types.delete('System')
  assert.equal(types.size, 34)
})

/**
 * The engine's grobs of `rows`, compiled natively and by core.wasm in one
 * instance, compared with `references` (each row's id to LilyPond's
 * grobs.tsv) on `types` (every type if null) with their columns, each score
 * equal: each compared type's count, natively and in wasm.
 */
async function compareTen(rows, references, types) {
  const wasm = await compileInWasm(rows.map((row) => row.path), { trace: true })
  const counted = { natively: new Map(), 'in wasm': new Map() }
  for (const [i, row] of rows.entries()) {
    const out = mkdtempSync(path.join(scratch, `engine-${row.id}-`))
    const run = runExample('compile', ['--trace', '--out', out, row.path], { cwd: path.dirname(row.path), encoding: 'utf8' })
    // 1: the file failed, as T03 does in LilyPond.
    assert.equal(run.status, row.id === 'T03' ? 1 : 0, `${row.id}: ${run.stderr}`)
    for (const [name, text] of [
      ['natively', readFileSync(path.join(out, `${path.basename(row.path, '.ly')}.${GROBS}`), 'utf8')],
      ['in wasm', wasm[i].outputs[GROBS].toString()],
    ]) {
      const result = compareGrobs(references[row.id], text, { types })
      assert.deepEqual([result.missing, result.extra, result.order, result.differ], [[], [], [], []], `${row.id} ${name}`)
      assert.ok(result.equal && result.counts[0] === result.counts[1], `${row.id} ${name}`)
      const grobs = readGrobs(text).flat().filter((g) => !types || types.includes(g.type))
      assert.ok(grobs.every((g) => g.columns !== '-'), `${row.id} ${name}`)
      for (const g of grobs) counted[name].set(g.type, (counted[name].get(g.type) ?? 0) + 1)
    }
  }
  return counted
}

/**
 * makers.txt's count of each of the 10's 34 types, summed over its
 * engravers (LeftEdge's two), and a System a score.
 */
const announced = () => MAKERS.reduce((counts, [type, , , n]) => counts.set(type, (counts.get(type) ?? 0) + n), new Map([['System', 10]]))

test("the engine's grobs of the 10, every line, natively and in wasm, equal LilyPond's with their columns",
  { skip: skipTen || cargo || coreSkip(), timeout: 600_000 }, async () => {
    const counted = await compareTen(ten, Object.fromEntries(await captureTen()), null)
    // 139 columns of each type, 198 NoteHeads, 4 Rests, 125 Stems,
    // StemStubs, NoteSpacings and NoteColumns, T03's Dots and DotColumn, a
    // LedgerLineSpanner, a TimeSignature and a StaffSymbol a score, 28
    // Accidentals in 22 AccidentalPlacements, 61 BarLines, 57 BarNumbers,
    // a SystemStartBar a score, 35 TextScripts (T05's 28, T10's 3, T13's
    // and T17's 2), T05's 18 ScriptRows, T18's 5 DynamicTexts, 3
    // Hairpins and 5 DynamicLineSpanners, T10's 4 VoltaBrackets and 2
    // VoltaBracketSpanners, a VerticalAxisGroup and a VerticalAlignment a
    // score, 71 BreakAlignments with 270 BreakAlignGroups, 71 LeftEdges
    // (61 from Bar_engraver, 10 from Clef_engraver) and 71 StaffSpacings,
    // 69 Clefs, 59 KeySignatures and a SpacingSpanner a score; 1,913 in
    // all, every line of the 10.
    const expected = new Map([...announced()].sort())
    assert.deepEqual([...expected.values()].reduce((a, b) => a + b), 1913)
    for (const name of ['natively', 'in wasm']) assert.deepEqual(new Map([...counted[name]].sort()), expected, name)
  })

/**
 * Nothing forbidding a break, as Bar_engraver between bar lines (step 18)
 * and Forbid_line_break_engraver under sustained notes (step 21) forbid it
 * in LilyPond, so that Clef_engraver and Key_engraver make their grobs at
 * the same timesteps on both sides (MS3 step 17).
 */
const UNFORBIDDEN = String.raw`
\layout {
  \context { \Score forbidBreakBetweenBarLines = ##f }
  \context { \Voice \remove Forbid_line_break_engraver \remove Spanner_break_forbid_engraver }
}
`
/**
 * Bar_engraver the only one to forbid a break (MS3 step 18), without
 * Forbid_line_break_engraver (step 21) and Spanner_break_forbid_engraver
 * (step 22), in LilyPond and in the engine.
 */
const BAR_FORBIDDEN = String.raw`
\layout { \context { \Voice \remove Forbid_line_break_engraver \remove Spanner_break_forbid_engraver } }
`

/**
 * The 10 with `layout` after each one's \version, in scratch/<name>/<id>
 * beside the files it includes from its own directory, and LilyPond's
 * captures of them: [rows, each row's id to its grobs.tsv's text].
 */
async function captureTenWith(name, layout) {
  const rows = ten.map((row) => {
    const dir = path.join(scratch, name, row.id)
    const source = readFileSync(row.path, 'utf8')
    mkdirSync(dir, { recursive: true })
    assert.match(source, /\\version "[^"]*"/, row.id)
    for (const [, name] of source.matchAll(/\\include "([^"]+)"/g)) {
      const from = path.join(path.dirname(row.path), name)
      if (existsSync(from)) cpSync(from, path.join(dir, name))
    }
    const file = path.join(dir, path.basename(row.path))
    writeFileSync(file, source.replace(/(\\version "[^"]*")/, `$1${layout}`))
    return { id: row.id, path: file }
  })
  const references = Object.fromEntries(await Promise.all(rows.map(async (row) => {
    const dir = path.join(path.dirname(row.path), 'lily')
    const result = await captureOne('lilypond', row, dir)
    assert.equal(result.exit, row.id === 'T03' ? 1 : 0, row.id)
    return [row.id, readFileSync(path.join(dir, GROBS), 'utf8')]
  })))
  return [rows, references]
}

test("the engine's grobs of the 10 with nothing forbidding a break, every line, natively and in wasm, equal LilyPond's with their columns",
  { skip: skipTen || cargo || coreSkip(), timeout: 600_000 }, async () => {
    const [rows, references] = await captureTenWith('unforbidden', UNFORBIDDEN)
    const counted = await compareTen(rows, references, null)
    // The 10's as they have them but the breakable ones, which follow the
    // command columns: a Clef, a KeySignature, a StaffSpacing, a
    // BreakAlignment and a LeftEdge (61 from Bar_engraver, 78 from
    // Clef_engraver) at each of the 139, each of the 10 having one staff,
    // and 488 BreakAlignGroups: 2,485 in all.
    const counts = new Map([...announced(), ['Clef', 139], ['KeySignature', 139], ['StaffSpacing', 139], ['BreakAlignment', 139], ['BreakAlignGroup', 488], ['LeftEdge', 139]])
    const expected = new Map([...counts].sort())
    assert.deepEqual([...expected.values()].reduce((a, b) => a + b), 2485)
    for (const name of ['natively', 'in wasm']) assert.deepEqual(new Map([...counted[name]].sort()), expected, name)
  })

test("the engine's grobs of the 10 with Bar_engraver forbidding breaks between bar lines, every line, natively and in wasm, equal LilyPond's with their columns, and Forbid_line_break_engraver takes off T03's clefs and key signatures at 2 and 3",
  { skip: skipTen || cargo || coreSkip(), timeout: 600_000 }, async () => {
    const [rows, references] = await captureTenWith('bar-forbidden', BAR_FORBIDDEN)
    const counted = await compareTen(rows, references, null)
    // The 10's as they have them, and 71 Clefs and 61 KeySignatures,
    // makers.txt's 69 and 59 and T03's two of each that LilyPond's
    // Forbid_line_break_engraver forbids, with their 4 BreakAlignGroups
    // besides makers.txt's 270: 1,921 in all.
    const counts = new Map([...announced(), ['Clef', 71], ['KeySignature', 61], ['BreakAlignGroup', 274]])
    const expected = new Map([...counts].sort())
    assert.deepEqual([...expected.values()].reduce((a, b) => a + b), 1921)
    for (const name of ['natively', 'in wasm']) assert.deepEqual(new Map([...counted[name]].sort()), expected, name)
    // In LilyPond's own layout the clefs and key signatures are these but
    // T03's at its bar lines at 2 and 3, under its c2. and c1 held across
    // them: Forbid_line_break_engraver's, with Spanner_break_forbid_engraver
    // forbidding none of them. Only the clefs and key signatures are
    // compared: the variants' links are lines below the originals'.
    const own = Object.fromEntries(await captureTen())
    const left = ['Clef', 'KeySignature'].flatMap((type) => [2, 3].map((at) => `${type}\t-\t-\t${at} 0\tStaff#0\t${type === 'Clef' ? 'Clef' : 'Key'}_engraver`))
    for (const row of rows) {
      const result = compareGrobs(own[row.id], references[row.id], { types: ['Clef', 'KeySignature'] })
      assert.deepEqual([result.missing, result.order, result.differ], [[], [], []], row.id)
      assert.deepEqual(result.extra.map((e) => e.grob).sort(), row.id === 'T03' ? left.sort() : [], row.id)
    }
  })

/** T25's OttavaBracket as LilyPond 2.26.0 captures it: upperVoiceOne's \ottava #1 in bar 3, unterminated, so ended at the last note column. */
const T25_OTTAVA = 'OttavaBracket\t<ev ottava-event>\tgranados.ly:56:2:2\t2 0;35/16 0\tStaff#0=high\tOttava_spanner_engraver'

test("the engine's ottava bracket of T25, natively and in wasm, equals LilyPond's capture with its columns",
  { skip: skipT25 || cargo || coreSkip(), timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'T25')
    const result = await captureOne('lilypond', t25, dir)
    assert.equal(result.exit, 0)
    const reference = readFileSync(path.join(dir, GROBS), 'utf8')
    assert.deepEqual(reference.split('\n').filter((line) => line.startsWith('OttavaBracket\t')), [T25_OTTAVA])
    const counted = await compareTen([t25], { T25: reference }, ['OttavaBracket'])
    for (const name of ['natively', 'in wasm']) assert.deepEqual(counted[name], new Map([['OttavaBracket', 1]]), name)
  })

/** Rests whose widths a Scheme engraver writes as it acknowledges them; a breve's is made, as the engine reads \breve as a whole note. */
const REST_WIDTHS = String.raw`\version "2.26.0"
#(define (show-rest context)
   (make-engraver
    (acknowledgers
     ((rest-interface engraver grob source-engraver)
      (ly:warning "~a ~a ~a" (ly:grob-property grob 'duration-log) (ly:grob-property grob 'style)
                  (ly:rest::width grob))))))
breveRest = #(make-music 'RestEvent 'duration (ly:make-duration -1))
\layout { \context { \Voice \consists #show-rest } }
{
  \breveRest r1 r2. r4 r8. r16 r32 r64 r128
  \override Rest.style = #'classical r4 r8
  \override Rest.style = #'mensural r4 r128
  \override Rest.style = #'z r4
  \override Rest.style = #'default r4
  \override Rest.font-size = #3 \breveRest
}
`

test("the engine's rests are as wide as LilyPond's, natively and in wasm", { skip: lilypond || cargo || coreSkip(), timeout: 300_000 }, async () => {
  const dir = mkdtempSync(path.join(scratch, 'rest-widths-'))
  const file = path.join(dir, 'rests.ly')
  writeFileSync(file, REST_WIDTHS)
  const reference = spawnSync('lilypond', ['--loglevel=WARNING', '-o', path.join(dir, 'rests'), file],
    { cwd: dir, encoding: 'utf8', env: { ...process.env, LANGUAGE: 'en' } })
  assert.equal(reference.status, 0, reference.stderr)
  // A line a rest: its duration log, its style and its width.
  const lines = reference.stderr.split('\n').filter(Boolean)
  assert.equal(lines.length, 16, reference.stderr)
  assert.deepEqual([lines[0], lines[3], lines[15]],
    ['warning: -1 () (0.0 . 0.5999999999999999)', 'warning: 2 () (-0.0 . 0.95)', 'warning: -1 default (0.0 . 0.8485281374238569)'])
  // Every pack of the build's: font-size 3 is emmentaler-26's.
  const fonts = path.join(engine, 'dist/engine/fonts')
  const packs = readdirSync(fonts).filter((f) => f.endsWith('.pack')).map((f) => [path.basename(f, '.pack'), path.join(fonts, f)])
  const out = mkdtempSync(path.join(scratch, 'engine-'))
  const run = runExample('compile', [...packs.flatMap(([, pack]) => ['--pack', pack]), '--out', out, file], { cwd: dir, encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  assert.equal(readFileSync(path.join(out, 'rests.err'), 'utf8'), reference.stderr, 'natively')
  const [wasm] = await compileInWasm([file], { modules: packs.map(([name, pack]) => [name, readFileSync(pack)]) })
  assert.equal(wasm.outputs.stderr.toString(), reference.stderr, 'in wasm')
})

/**
 * Part 2's 36 translator classes (MS3 steps 15–22): the part2 rows of
 * $LILY_CORPUS/ms3-part2-split/classes.txt.
 */
const PART2 = [
  'Accidental_engraver', 'Axis_group_engraver', 'Bar_engraver', 'Bar_number_engraver',
  'Break_align_engraver', 'Clef_engraver', 'Dot_column_engraver', 'Dots_engraver',
  'Dynamic_align_engraver', 'Dynamic_engraver', 'Forbid_line_break_engraver',
  'Grace_spacing_engraver', 'Grob_pq_engraver', 'Keep_alive_together_engraver',
  'Key_engraver', 'Ledger_line_engraver', 'Mark_tracking_translator',
  'Non_musical_script_column_engraver', 'Note_heads_engraver',
  'Note_spacing_engraver', 'Ottava_spanner_engraver', 'Paper_column_engraver',
  'Pure_from_neighbor_engraver', 'Rest_engraver', 'Rhythmic_column_engraver',
  'Script_row_engraver', 'Separating_line_group_engraver', 'Spacing_engraver',
  'Spanner_break_forbid_engraver', 'Staff_symbol_engraver', 'Stem_engraver',
  'System_start_delimiter_engraver', 'Text_engraver', 'Time_signature_engraver',
  'Vertical_align_engraver', 'Volta_engraver',
]

/** A file whose translations write, for each of PART2, the times it did its work (part2.txt). */
const PART2_WORK = String.raw`\version "2.26.0"
% Each of part 2's 36 translator classes at work in a \layout translation
% of the default contexts (MS3 step 22). Writes part2.txt: for each class
% of part2, in its order,
% (name work detail), work the times it did its work. A recorder in each
% Score counts the grobs announced by each class (the translator's printed
% name, as tools/compare/grobs.ly reads it), notes the moments where
% forbidBreak is #t at process-music and, at its finalize, the Score's
% rehearsalMark, the keep-alive-with of each VerticalAxisGroup and the
% grobs with neighbors; one in each Voice the most busyGrobs at a stop.
% Bar_engraver forbids breaks between bar lines unless
% forbidBreakBetweenBarLines is #f, as it is in scores 2 and 3.
% Score 1: a StaffGroup of a staff with most of the notation the 10 have and
% a PianoStaff; score 2: a staff of two voices whose half notes sustain over
% the quarters, Spanner_break_forbid_engraver removed (forbidBreak is
% Forbid_line_break_engraver's); score 3: a voice whose maker makes a Beam
% from 0 to 1/2, Forbid_line_break_engraver removed (forbidBreak is
% Spanner_break_forbid_engraver's), in a staff whose maker makes two
% Scripts on the first command column (a ScriptColumn, non-musical).
#(define part2
   '(Accidental_engraver Axis_group_engraver Bar_engraver Bar_number_engraver
     Break_align_engraver Clef_engraver Dot_column_engraver Dots_engraver
     Dynamic_align_engraver Dynamic_engraver Forbid_line_break_engraver
     Grace_spacing_engraver Grob_pq_engraver Keep_alive_together_engraver
     Key_engraver Ledger_line_engraver Mark_tracking_translator
     Non_musical_script_column_engraver Note_heads_engraver
     Note_spacing_engraver Ottava_spanner_engraver Paper_column_engraver
     Pure_from_neighbor_engraver Rest_engraver Rhythmic_column_engraver
     Script_row_engraver Separating_line_group_engraver Spacing_engraver
     Spanner_break_forbid_engraver Staff_symbol_engraver Stem_engraver
     System_start_delimiter_engraver Text_engraver Time_signature_engraver
     Vertical_align_engraver Volta_engraver))
#(define (mom m) (list (ly:moment-main m) (ly:moment-grace m)))
#(define (class translator)
   (let ((s (format #f "~a" translator)))
     (string->symbol
      (string-trim-both (string-drop (string-drop-right s 1) (string-length "#<Translator "))))))
#(define work (map (lambda (name) (list name 0 '())) part2))
#(define (did! name n detail)
   (let ((entry (assq name work)))
     (set-car! (cdr entry) (+ (cadr entry) n))
     (if detail (set-car! (cddr entry) (append (caddr entry) (list detail))))))
#(define scores 0)
#(define busiest 0)
#(define (score-recorder context)
   (let ((groups '()) (pure '()) (forbidden '()))
     (make-engraver
      ((initialize engraver) (set! scores (1+ scores)))
      (acknowledgers
       ((grob-interface engraver grob source-engraver)
        (let ((name (class source-engraver)))
          (if (assq name work) (did! name 1 #f)))
        (case (grob::name grob)
          ((VerticalAxisGroup) (set! groups (cons grob groups)))
          ((BarLine Clef KeySignature TimeSignature) (set! pure (cons grob pure))))))
      ((process-music engraver)
       (if (eq? #t (ly:context-property context 'forbidBreak))
           (set! forbidden (cons (mom (ly:context-current-moment context)) forbidden))))
      ((finalize engraver)
       (let ((n scores)
             (alive (map (lambda (g)
                           (let ((a (ly:grob-object g 'keep-alive-with)))
                             (if (ly:grob-array? a) (ly:grob-array-length a) 0)))
                         (reverse groups)))
             (neighbors (filter (lambda (g)
                                  (let ((a (ly:grob-object g 'neighbors)))
                                    (and (ly:grob-array? a) (> (ly:grob-array-length a) 0))))
                                pure)))
         (case n
           ((1)
            (did! 'Mark_tracking_translator
                  (1- (ly:context-property context 'rehearsalMark)) #f)
            (did! 'Keep_alive_together_engraver (apply + alive) alive)
            (did! 'Pure_from_neighbor_engraver (length neighbors) #f))
           ((2) (did! 'Forbid_line_break_engraver (length forbidden) (reverse forbidden)))
           ((3) (did! 'Spanner_break_forbid_engraver (length forbidden) (reverse forbidden))))
         (if (= n 3)
             (begin
              (did! 'Grob_pq_engraver busiest #f)
              (with-output-to-file "part2.txt"
                (lambda () (for-each (lambda (entry) (write entry) (newline)) work))))))))))
#(define (voice-recorder context)
   (make-engraver
    ((stop-translation-timestep engraver)
     (let ((busy (ly:context-property context 'busyGrobs '())))
       (set! busiest (max busiest (length busy)))))))
% A Beam from the first timestep to 1/2.
#(define (beam-maker context)
   (let ((beam #f))
     (make-engraver
      ((process-music engraver)
       (let ((now (ly:moment-main (ly:context-current-moment context)))
             (col (ly:context-property context 'currentMusicalColumn)))
         (cond ((not beam)
                (set! beam (ly:engraver-make-grob engraver 'Beam '()))
                (ly:spanner-set-bound! beam LEFT col))
               ((equal? now 1/2)
                (ly:spanner-set-bound! beam RIGHT col)
                (ly:engraver-announce-end-grob engraver beam '()))))))))
% Two Scripts on the first command column.
#(define (script-maker context)
   (let ((done #f))
     (make-engraver
      ((process-music engraver)
       (unless done
         (set! done #t)
         (for-each
          (lambda (priority)
            (let ((g (ly:engraver-make-grob engraver 'Script '())))
              (ly:grob-set-property! g 'side-axis Y)
              (ly:grob-set-property! g 'direction UP)
              (ly:grob-set-property! g 'script-priority priority)
              (ly:grob-set-property! g 'stencil #f)
              (ly:grob-set-parent! g X (ly:context-property context 'currentCommandColumn))))
          '(0 1)))))))
\layout {
  \context { \Score \consists #score-recorder }
  \context { \Voice \consists #voice-recorder }
}
\score {
  \new StaffGroup <<
    \new Staff \relative c' {
      \key g \major \time 3/4 \mark \default
      c4.\p\< d8 cis4\! | e4^"a"^"b" r4 c''4 |
      \ottava #1 c4 d e \ottava #0 |
      \repeat volta 2 { f2. } \alternative { { g2. } { a2. } }
      \grace b,16 c2. \bar "|."
    }
    \new PianoStaff <<
      \new Staff { c'2.*7 }
      \new Staff { \clef bass c2.*7 }
    >>
  >>
}
\score {
  \new Staff \with { forbidBreakBetweenBarLines = ##f } <<
    \new Voice \with { \remove "Spanner_break_forbid_engraver" } { \voiceOne c'2 c'2 }
    \new Voice { \voiceTwo e'4 e' e' e' }
  >>
}
\score {
  \new Staff \with { forbidBreakBetweenBarLines = ##f \consists #script-maker }
  \new Voice \with { \remove "Forbid_line_break_engraver" \consists #beam-maker } { c'4 c' c' c' }
}
`

test("each of part 2's 36 classes works in a \\layout translation as LilyPond's does, natively and in wasm",
  { skip: lilypond || cargo || coreSkip(), timeout: 300_000 }, async () => {
    const dir = mkdtempSync(path.join(scratch, 'part2-'))
    const file = path.join(dir, 'part2.ly')
    writeFileSync(file, PART2_WORK)
    const reference = spawnSync('lilypond', ['--loglevel=WARNING', '-dno-print-pages', '-o', path.join(dir, 'part2'), file],
      { cwd: dir, encoding: 'utf8', env: { ...process.env, LANGUAGE: 'en' } })
    assert.equal(reference.status, 0, reference.stderr)
    assert.equal(reference.stderr, '')
    // A line a class: (name work detail), its work more than none.
    const lines = readFileSync(path.join(dir, 'part2.txt'), 'utf8').split('\n').filter(Boolean)
    assert.deepEqual(lines.map((line) => line.slice(1).split(' ')[0]), PART2)
    for (const line of lines) assert.ok(Number(line.split(' ')[1]) > 0, line)
    assert.equal(lines.find((line) => line.startsWith('(Forbid_line_break_engraver ')), '(Forbid_line_break_engraver 2 (((1/4 0) (3/4 0))))')
    assert.equal(lines.find((line) => line.startsWith('(Spanner_break_forbid_engraver ')), '(Spanner_break_forbid_engraver 2 (((1/4 0) (1/2 0))))')

    const out = mkdtempSync(path.join(scratch, 'engine-'))
    const run = runExample('compile', ['--out', out, file], { cwd: dir, encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const [wasm] = await compileInWasm([file])
    for (const [name, got, err] of [
      ['natively', readFileSync(path.join(out, 'part2.txt'), 'utf8'), readFileSync(path.join(out, 'part2.err'), 'utf8')],
      ['in wasm', wasm.outputs['part2.txt']?.toString() ?? '', wasm.outputs.stderr.toString()],
    ]) {
      assert.deepEqual(got.split('\n').filter(Boolean), lines, `${name}: the records`)
      assert.equal(err, '', `${name}: stderr`)
    }
  })

/** fixtures/beams.ly's row, as capture.mjs takes one (MS4 step 2). */
const beams = { id: 'beams', file: 'tools/compare/fixtures/beams.ly', path: fileURLToPath(new URL('../fixtures/beams.ly', import.meta.url)) }
/**
 * LilyPond's warnings on beams.ly's last score: a beam started inside one, a stem too long for its beam and where
 * that beam began, and a beam left unterminated.
 */
const BEAM_WARNINGS = [
  [48, 10, 'already have a beam'],
  [48, 21, 'stem does not fit in beam'],
  [48, 19, 'beam was started here'],
  [48, 35, 'unterminated beam'],
].map(([line, column, text]) => `${beams.path}:${line}:${column}: warning: ${text}\n`)

/**
 * beams.ly's grobs.tsv and stderr against LilyPond's capture in `dir`: every line LilyPond's with its columns, the
 * Beams of the four beam engravers and the flags of the beamed stems killed among them.
 */
function assertBeamsFixture(dir, { grobs, stderr }, where) {
  const read = (name) => readFileSync(path.join(dir, name), 'utf8')
  assert.equal(stderr, read('stderr.txt'), where)
  assert.equal(stderr.split('\n').filter((line) => line.includes('warning:')).map((line) => `${line}\n`).join(''), BEAM_WARNINGS.join(''), where)
  const reference = read(GROBS)
  assert.deepEqual(compareGrobs(reference, grobs, { types: null, columns: true }), { equal: true, counts: [1868, 1868], missing: [], extra: [], order: [], differ: [] }, where)
  const lines = readGrobs(grobs).flat()
  const engravers = countTypes(lines.filter((g) => g.type === 'Beam').map((g) => ({ type: g.engraver })))
  assert.deepEqual(Object.fromEntries(engravers), {
    Beam_engraver: 19, Auto_beam_engraver: 21, Grace_auto_beam_engraver: 2, Grace_beam_engraver: 1,
  }, where)
  // The unterminated beam killed; the flags of the stems under a beam too,
  // nine kept.
  assert.equal(lines.filter((g) => g.type === 'Beam' && g.columns === '-').length, 1, where)
  assert.equal(lines.filter((g) => g.type === 'Flag' && g.columns !== '-').length, 9, where)
}

test("fixtures/beams.ly natively and in wasm: the Beams of Beam_engraver, Auto_beam_engraver and their grace kin, grobs.tsv and the warnings LilyPond's",
  { skip: lilypond || cargo, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'beams')
    const result = await captureOne('lilypond', beams, dir)
    assert.equal(result.exit, 0)
    const out = mkdtempSync(path.join(scratch, 'engine-beams-'))
    const run = runExample('compile', ['--trace', '--out', out, beams.path], { cwd: path.dirname(beams.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `beams.${name}`), 'utf8')
    const native = { grobs: read(GROBS), stderr: read('err') }
    assertBeamsFixture(dir, native, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([beams.path], { trace: true })
    assertBeamsFixture(dir, { grobs: wasm.outputs[GROBS]?.toString(), stderr: wasm.outputs.stderr?.toString() }, 'in wasm')
  })

const t29 = corpus().find((row) => row.id === 'T29')
const skipT29 = lilypond || (t29 && existsSync(t29.path) && existsSync(path.join(corpusDir(), 'ref', 'T29', GROBS))
  ? false : 'the corpus is not fetched or captured: run node tools/corpus/fetch.mjs and capture.mjs')

test("T29's StemTremolos, Beams (its chord tremolos' among them) and Flags, natively and in wasm, are LilyPond's in grobs.tsv (MS4 step 5)",
  { skip: skipT29 || cargo, timeout: 300_000 }, async () => {
    const reference = readFileSync(path.join(corpusDir(), 'ref', 'T29', GROBS), 'utf8')
    const types = ['StemTremolo', 'Beam', 'Flag']
    const out = mkdtempSync(path.join(scratch, 'engine-T29-'))
    const run = runExample('compile', ['--trace', '--out', out, t29.path], { cwd: path.dirname(t29.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const stem = path.basename(t29.path, '.ly')
    const native = readFileSync(path.join(out, `${stem}.${GROBS}`), 'utf8')
    const counts = (text) => Object.fromEntries(countTypes(readGrobs(text).map((t) => t.filter((g) => types.includes(g.type)))))
    assert.deepEqual(counts(reference), counts(native))
    assert.equal(counts(reference).StemTremolo, 7)
    assert.equal(readGrobs(reference).flat().filter((g) => g.type === 'Beam' && g.engraver === 'Chord_tremolo_engraver').length, 3)
    const result = compareGrobs(reference, native, { types, columns: true })
    assert.equal(result.equal, true, JSON.stringify(result))
    if (coreSkip()) return
    const [wasm] = await compileInWasm([t29.path], { trace: true })
    assert.equal(wasm.outputs[GROBS]?.toString(), native)
  })

/** fixtures/slurs.ly's row, as capture.mjs takes one (MS4 step 7). */
const slurs = { id: 'slurs', file: 'tools/compare/fixtures/slurs.ly', path: fileURLToPath(new URL('../fixtures/slurs.ly', import.meta.url)) }
/**
 * LilyPond's warnings on slurs.ly's last score: a slur started inside one of its id, stops with no slur and no
 * phrasing slur, and a slur and a phrasing slur left unterminated.
 */
const SLUR_WARNINGS = [
  [42, 10, 'already have slur'],
  [42, 16, 'cannot end slur'],
  [42, 22, 'cannot end slur'],
  [42, 25, 'cannot end phrasing slur'],
  [42, 36, 'unterminated phrasing slur'],
  [42, 31, 'unterminated slur'],
].map(([line, column, text]) => `${slurs.path}:${line}:${column}: warning: ${text}\n`)

/** slurs.ly's grobs.tsv and stderr against LilyPond's capture in `dir`: every line LilyPond's with its columns. */
function assertSlursFixture(dir, { grobs, stderr }, where) {
  const read = (name) => readFileSync(path.join(dir, name), 'utf8')
  assert.equal(stderr, read('stderr.txt'), where)
  assert.equal(stderr.split('\n').filter((line) => line.includes('warning:')).map((line) => `${line}\n`).join(''), SLUR_WARNINGS.join(''), where)
  const reference = read(GROBS)
  assert.deepEqual(compareGrobs(reference, grobs, { types: null, columns: true }), { equal: true, counts: [1169, 1169], missing: [], extra: [], order: [], differ: [] }, where)
  const lines = readGrobs(grobs).flat()
  const types = (type) => lines.filter((g) => g.type === type)
  // The two left unterminated killed; the double slurs two of each.
  assert.deepEqual([types('Slur').length, types('PhrasingSlur').length], [27, 4], where)
  assert.deepEqual([types('Slur').filter((g) => g.columns === '-').length, types('PhrasingSlur').filter((g) => g.columns === '-').length], [1, 1], where)
}

test("fixtures/slurs.ly natively and in wasm: the Slurs and PhrasingSlurs of the slur engravers, grobs.tsv and the warnings LilyPond's",
  { skip: lilypond || cargo, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'slurs')
    const result = await captureOne('lilypond', slurs, dir)
    assert.equal(result.exit, 0)
    const out = mkdtempSync(path.join(scratch, 'engine-slurs-'))
    const run = runExample('compile', ['--trace', '--out', out, slurs.path], { cwd: path.dirname(slurs.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `slurs.${name}`), 'utf8')
    assertSlursFixture(dir, { grobs: read(GROBS), stderr: read('err') }, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([slurs.path], { trace: true })
    assertSlursFixture(dir, { grobs: wasm.outputs[GROBS]?.toString(), stderr: wasm.outputs.stderr?.toString() }, 'in wasm')
  })

/**
 * The scores of the 30 with slurs (MS4 step 7), each Slur's and PhrasingSlur's count in LilyPond's grobs.tsv; T22's
 * TabVoice slurs among them since MS4 step 7a, which makes the tablature's heads.
 */
const SLURRED = { T02: 1, T04: 2, T09: 1, T12: 34, T22: 10, T25: 6, T27: 19, T29: 21 }
const slurred = corpus().filter((row) => row.id in SLURRED)
const skipSlurred = lilypond || (slurred.length === 8 && slurred.every((row) => existsSync(row.path) && existsSync(path.join(corpusDir(), 'ref', row.id, GROBS)))
  ? false : 'the corpus is not fetched or captured: run node tools/corpus/fetch.mjs and capture.mjs')

test("the Slurs and PhrasingSlurs of T02, T04, T09, T12, T22, T25, T27 and T29, natively and in wasm, are LilyPond's in grobs.tsv (MS4 steps 7 and 7a)",
  { skip: skipSlurred || cargo, timeout: 600_000 }, async () => {
    const types = ['Slur', 'PhrasingSlur']
    const natives = []
    for (const row of slurred) {
      const reference = readFileSync(path.join(corpusDir(), 'ref', row.id, GROBS), 'utf8')
      const out = mkdtempSync(path.join(scratch, `engine-${row.id}-`))
      const run = runExample('compile', ['--trace', '--out', out, row.path], { cwd: path.dirname(row.path), encoding: 'utf8' })
      assert.equal(run.status, 0, `${row.id}: ${run.stderr}`)
      const native = readFileSync(path.join(out, `${path.basename(row.path, '.ly')}.${GROBS}`), 'utf8')
      natives.push(native)
      const count = readGrobs(native).flat().filter((g) => types.includes(g.type)).length
      assert.equal(count, SLURRED[row.id], row.id)
      const result = compareGrobs(reference, native, { types, columns: true })
      assert.equal(result.equal, true, `${row.id}: ${JSON.stringify(result)}`)
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(slurred.map((row) => row.path), { trace: true })
    wasm.forEach((result, i) => assert.equal(result.outputs[GROBS]?.toString(), natives[i], slurred[i].id))
  })

/** fixtures/tab-note-heads.ly's row, as capture.mjs takes one (MS4 step 7a). */
const tabHeads = { id: 'tab-note-heads', file: 'tools/compare/fixtures/tab-note-heads.ly', path: fileURLToPath(new URL('../fixtures/tab-note-heads.ly', import.meta.url)) }

/**
 * tab-note-heads.ly's grobs.tsv and stderr against LilyPond's capture in `dir`: every line LilyPond's with its
 * columns, and determine-frets' two warnings.
 */
function assertTabHeadsFixture(dir, { grobs, stderr }, where) {
  const read = (name) => readFileSync(path.join(dir, name), 'utf8')
  assert.equal(stderr, read('stderr.txt'), where)
  assert.deepEqual(stderr.split('\n').filter((line) => line.includes('warning:')).map((line) => line.replace(/^.*warning: /, '')),
    ['Open fret on string 1 has finger of 3', 'No string for pitch #<Pitch c, > (given frets ())'], where)
  const reference = read(GROBS)
  assert.deepEqual(compareGrobs(reference, grobs, { types: null, columns: true }), { equal: true, counts: [454, 454], missing: [], extra: [], order: [], differ: [] }, where)
  const lines = readGrobs(grobs).flat()
  const tab = (type) => lines.filter((g) => g.type === type && g.context.startsWith('TabVoice#')).length
  // A head of each note a string reaches; the TabVoice's slur and tie bound by them.
  assert.deepEqual([tab('TabNoteHead'), tab('Slur'), tab('Tie')], [31, 1, 1], where)
}

test("fixtures/tab-note-heads.ly natively and in wasm: the TabNoteHeads of Tab_note_heads_engraver, grobs.tsv and the warnings LilyPond's (MS4 step 7a)",
  { skip: lilypond || cargo, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'tab-note-heads')
    const result = await captureOne('lilypond', tabHeads, dir)
    assert.equal(result.exit, 0)
    const out = mkdtempSync(path.join(scratch, 'engine-tab-note-heads-'))
    const run = runExample('compile', ['--trace', '--out', out, tabHeads.path], { cwd: path.dirname(tabHeads.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `tab-note-heads.${name}`), 'utf8')
    assertTabHeadsFixture(dir, { grobs: read(GROBS), stderr: read('err') }, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([tabHeads.path], { trace: true })
    assertTabHeadsFixture(dir, { grobs: wasm.outputs[GROBS]?.toString(), stderr: wasm.outputs.stderr?.toString() }, 'in wasm')
  })

const t22 = corpus().find((row) => row.id === 'T22')
const skipT22 = lilypond || (t22 && existsSync(t22.path) && existsSync(path.join(corpusDir(), 'ref', 'T22', GROBS))
  ? false : 'the corpus is not fetched or captured: run node tools/corpus/fetch.mjs and capture.mjs')

test("T22's grobs, natively and in wasm, are LilyPond's in grobs.tsv, every line with its columns: its 33 TabNoteHeads and the TabVoices' grobs they bring (MS4 step 7a)",
  { skip: skipT22 || cargo, timeout: 300_000 }, async () => {
    const reference = readFileSync(path.join(corpusDir(), 'ref', 'T22', GROBS), 'utf8')
    const out = mkdtempSync(path.join(scratch, 'engine-T22-'))
    const run = runExample('compile', ['--trace', '--out', out, t22.path], { cwd: path.dirname(t22.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const native = readFileSync(path.join(out, `${path.basename(t22.path, '.ly')}.${GROBS}`), 'utf8')
    assert.deepEqual(compareGrobs(reference, native, { types: null, columns: true }), { equal: true, counts: [565, 565], missing: [], extra: [], order: [], differ: [] })
    const lines = readGrobs(native).flat()
    assert.equal(lines.filter((g) => g.type === 'TabNoteHead' && g.engraver === 'Tab_note_heads_engraver').length, 33)
    assert.equal(lines.filter((g) => g.context.startsWith('TabVoice#')).length, 171)
    if (coreSkip()) return
    const [wasm] = await compileInWasm([t22.path], { trace: true })
    assert.equal(wasm.outputs[GROBS]?.toString(), native)
  })

/** fixtures/tuplets.ly's row, as capture.mjs takes one (MS4 step 12). */
const tuplets = { id: 'tuplets', file: 'tools/compare/fixtures/tuplets.ly', path: fileURLToPath(new URL('../fixtures/tuplets.ly', import.meta.url)) }

/**
 * tuplets.ly's grobs.tsv and stderr against LilyPond's capture in `dir`: every line LilyPond's with its columns, a
 * TupletBracket and a TupletNumber of each tuplet, those of the tuplet of no note killed.
 */
function assertTupletsFixture(dir, { grobs, stderr }, where) {
  const read = (name) => readFileSync(path.join(dir, name), 'utf8')
  assert.equal(stderr, read('stderr.txt'), where)
  assert.equal(stderr, '', where)
  const reference = read(GROBS)
  assert.deepEqual(compareGrobs(reference, grobs, { types: null, columns: true }), { equal: true, counts: [2399, 2399], missing: [], extra: [], order: [], differ: [] }, where)
  const lines = readGrobs(grobs).flat()
  const types = (type) => lines.filter((g) => g.type === type && g.engraver === 'Tuplet_engraver')
  assert.deepEqual([types('TupletBracket').length, types('TupletNumber').length], [64, 64], where)
  // \tuplet 3/2 { s8 s s }: no column, so no bounds.
  assert.deepEqual([types('TupletBracket').filter((g) => g.columns === '-').length, types('TupletNumber').filter((g) => g.columns === '-').length], [1, 1], where)
}

test("fixtures/tuplets.ly natively and in wasm: the TupletBrackets and TupletNumbers of Tuplet_engraver, nested, full length and of no note, grobs.tsv LilyPond's (MS4 step 12)",
  { skip: lilypond || cargo, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'tuplets')
    const result = await captureOne('lilypond', tuplets, dir)
    assert.equal(result.exit, 0)
    const out = mkdtempSync(path.join(scratch, 'engine-tuplets-'))
    const run = runExample('compile', ['--trace', '--out', out, tuplets.path], { cwd: path.dirname(tuplets.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `tuplets.${name}`), 'utf8')
    assertTupletsFixture(dir, { grobs: read(GROBS), stderr: read('err') }, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([tuplets.path], { trace: true })
    assertTupletsFixture(dir, { grobs: wasm.outputs[GROBS]?.toString(), stderr: wasm.outputs.stderr?.toString() }, 'in wasm')
  })

/** The scores of the 30 with tuplets (census.txt, MS4 step 12), each one's TupletBrackets in LilyPond's grobs.tsv. */
const TUPLETED = { T04: 1, T08: 14, T09: 1, T23: 2, T25: 4, T26: 14, T28: 28 }
const tupleted = corpus().filter((row) => row.id in TUPLETED)
const skipTupleted = lilypond || (tupleted.length === 7 && tupleted.every((row) => existsSync(row.path) && existsSync(path.join(corpusDir(), 'ref', row.id, GROBS)))
  ? false : 'the corpus is not fetched or captured: run node tools/corpus/fetch.mjs and capture.mjs')

test("the TupletBrackets and TupletNumbers of T04, T08, T09, T23, T25, T26 and T28, natively and in wasm, are LilyPond's in grobs.tsv, and the grobs of T08, T09, T25, T26 and T28 every line (MS4 step 12)",
  { skip: skipTupleted || cargo, timeout: 600_000 }, async () => {
    const types = ['TupletBracket', 'TupletNumber']
    const natives = []
    for (const row of tupleted) {
      const reference = readFileSync(path.join(corpusDir(), 'ref', row.id, GROBS), 'utf8')
      const out = mkdtempSync(path.join(scratch, `engine-${row.id}-`))
      const run = runExample('compile', ['--trace', '--out', out, row.path], { cwd: path.dirname(row.path), encoding: 'utf8' })
      assert.equal(run.status, 0, `${row.id}: ${run.stderr}`)
      const native = readFileSync(path.join(out, `${path.basename(row.path, '.ly')}.${GROBS}`), 'utf8')
      natives.push(native)
      const brackets = readGrobs(native).flat().filter((g) => g.type === 'TupletBracket').length
      assert.equal(brackets, TUPLETED[row.id], row.id)
      const result = compareGrobs(reference, native, { types, columns: true })
      assert.equal(result.equal, true, `${row.id}: ${JSON.stringify(result)}`)
      // T04's and T23's lyrics are part 4's.
      if (!['T04', 'T23'].includes(row.id)) {
        const whole = compareGrobs(reference, native, { types: null, columns: true })
        assert.equal(whole.equal, true, `${row.id}: ${JSON.stringify(whole)}`)
      }
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(tupleted.map((row) => row.path), { trace: true })
    wasm.forEach((result, i) => assert.equal(result.outputs[GROBS]?.toString(), natives[i], tupleted[i].id))
  })

/** fixtures/lyric-grobs.ly's row, as capture.mjs takes one (MS4 step 15). */
const lyricGrobs = { id: 'lyric-grobs', file: 'tools/compare/fixtures/lyric-grobs.ly', path: fileURLToPath(new URL('../fixtures/lyric-grobs.ly', import.meta.url)) }

/** The lyric engravers' grob types (MS4 step 15). */
const LYRIC_TYPES = ['LyricText', 'LyricExtender', 'LyricHyphen', 'LyricSpace', 'StanzaNumber']

test("fixtures/lyric-grobs.ly natively and in wasm: the LyricTexts, extenders, hyphens, spaces and stanza numbers of the lyric engravers, grobs.tsv LilyPond's (MS4 step 15)",
  { skip: lilypond || cargo, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'lyric-grobs')
    assert.equal((await captureOne('lilypond', lyricGrobs, dir)).exit, 0)
    const reference = readFileSync(path.join(dir, GROBS), 'utf8')
    const counts = Object.fromEntries(LYRIC_TYPES.map((type) => [type, readGrobs(reference).flat().filter((g) => g.type === type).length]))
    assert.deepEqual(counts, { LyricText: 52, LyricExtender: 7, LyricHyphen: 13, LyricSpace: 40, StanzaNumber: 2 })
    const out = mkdtempSync(path.join(scratch, 'engine-lyric-grobs-'))
    const run = runExample('compile', ['--trace', '--out', out, lyricGrobs.path], { cwd: path.dirname(lyricGrobs.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `lyric-grobs.${name}`), 'utf8')
    const n = readGrobs(reference).flat().length
    assert.deepEqual(compareGrobs(reference, read(GROBS), { types: null, columns: true }), { equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [] })
    assert.equal(read('err'), readFileSync(path.join(dir, 'stderr.txt'), 'utf8'))
    if (coreSkip()) return
    const [wasm] = await compileInWasm([lyricGrobs.path], { trace: true })
    assert.equal(wasm.outputs[GROBS]?.toString(), read(GROBS))
  })

/** The scores of the 30 with lyrics (census.txt, MS4 step 15), each one's lyric grobs in LilyPond's grobs.tsv. */
const LYRICAL = { T02: 35, T04: 23, T20: 64, T23: 98, T30: 28 }
const lyrical = corpus().filter((row) => row.id in LYRICAL)
const skipLyrical = lilypond || (lyrical.length === 5 && lyrical.every((row) => existsSync(row.path) && existsSync(path.join(corpusDir(), 'ref', row.id, GROBS)))
  ? false : 'the corpus is not fetched or captured: run node tools/corpus/fetch.mjs and capture.mjs')

test("the lyric grobs of T02, T04, T20, T23 and T30, natively and in wasm, are LilyPond's in grobs.tsv, and their grobs every line (MS4 step 15)",
  { skip: skipLyrical || cargo, timeout: 600_000 }, async () => {
    const natives = []
    for (const row of lyrical) {
      const reference = readFileSync(path.join(corpusDir(), 'ref', row.id, GROBS), 'utf8')
      const out = mkdtempSync(path.join(scratch, `engine-${row.id}-`))
      const run = runExample('compile', ['--trace', '--out', out, row.path], { cwd: path.dirname(row.path), encoding: 'utf8' })
      assert.equal(run.status, 0, `${row.id}: ${run.stderr}`)
      const native = readFileSync(path.join(out, `${path.basename(row.path, '.ly')}.${GROBS}`), 'utf8')
      natives.push(native)
      const lyrics = readGrobs(native).flat().filter((g) => LYRIC_TYPES.includes(g.type)).length
      assert.equal(lyrics, LYRICAL[row.id], row.id)
      const whole = compareGrobs(reference, native, { types: null, columns: true })
      assert.equal(whole.equal, true, `${row.id}: ${JSON.stringify(whole)}`)
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(lyrical.map((row) => row.path), { trace: true })
    wasm.forEach((result, i) => assert.equal(result.outputs[GROBS]?.toString(), natives[i], lyrical[i].id))
  })

/** fixtures/figures.ly's row, as capture.mjs takes one (MS4 step 16). */
const figures = { id: 'figures', file: 'tools/compare/fixtures/figures.ly', path: fileURLToPath(new URL('../fixtures/figures.ly', import.meta.url)) }

/** The figured bass engravers' grob types (MS4 step 16, crates/figured-bass). */
const FIGURE_TYPES = ['BassFigure', 'BassFigureAlignment', 'BassFigureAlignmentPositioning', 'BassFigureBracket', 'BassFigureContinuation', 'BassFigureLine']

test("fixtures/figures.ly natively and in wasm: the figures, lines, alignments, positionings, brackets and continuations of the figured bass engravers, grobs.tsv LilyPond's (MS4 step 16)",
  { skip: lilypond || cargo, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'figures')
    assert.equal((await captureOne('lilypond', figures, dir)).exit, 0)
    const reference = readFileSync(path.join(dir, GROBS), 'utf8')
    const counts = Object.fromEntries(FIGURE_TYPES.map((type) => [type, readGrobs(reference).flat().filter((g) => g.type === type).length]))
    assert.deepEqual(counts, {
      BassFigure: 58, BassFigureAlignment: 16, BassFigureAlignmentPositioning: 5, BassFigureBracket: 2, BassFigureContinuation: 10, BassFigureLine: 32,
    })
    const out = mkdtempSync(path.join(scratch, 'engine-figures-'))
    const run = runExample('compile', ['--trace', '--out', out, figures.path], { cwd: path.dirname(figures.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `figures.${name}`), 'utf8')
    const n = readGrobs(reference).flat().length
    assert.deepEqual(compareGrobs(reference, read(GROBS), { types: null, columns: true }), { equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [] })
    assert.equal(read('err'), readFileSync(path.join(dir, 'stderr.txt'), 'utf8'))
    if (coreSkip()) return
    const [wasm] = await compileInWasm([figures.path], { trace: true })
    assert.equal(wasm.outputs[GROBS]?.toString(), read(GROBS))
  })

/** The scores of the 30 with figures (census.txt, MS4 step 16), each one's figure grobs in LilyPond's grobs.tsv. */
const FIGURED = { T06: 78, T21: 11 }
const figured = corpus().filter((row) => row.id in FIGURED)
const skipFigured = lilypond || (figured.length === 2 && figured.every((row) => existsSync(row.path) && existsSync(path.join(corpusDir(), 'ref', row.id, GROBS)))
  ? false : 'the corpus is not fetched or captured: run node tools/corpus/fetch.mjs and capture.mjs')

test("the figure grobs of T06 and T21, natively and in wasm, are LilyPond's in grobs.tsv, and their grobs every line (MS4 step 16)",
  { skip: skipFigured || cargo, timeout: 600_000 }, async () => {
    const natives = []
    for (const row of figured) {
      const reference = readFileSync(path.join(corpusDir(), 'ref', row.id, GROBS), 'utf8')
      const out = mkdtempSync(path.join(scratch, `engine-${row.id}-`))
      const run = runExample('compile', ['--trace', '--out', out, row.path], { cwd: path.dirname(row.path), encoding: 'utf8' })
      assert.equal(run.status, 0, `${row.id}: ${run.stderr}`)
      const native = readFileSync(path.join(out, `${path.basename(row.path, '.ly')}.${GROBS}`), 'utf8')
      natives.push(native)
      const made = readGrobs(native).flat().filter((g) => FIGURE_TYPES.includes(g.type)).length
      assert.equal(made, FIGURED[row.id], row.id)
      const whole = compareGrobs(reference, native, { types: null, columns: true })
      assert.equal(whole.equal, true, `${row.id}: ${JSON.stringify(whole)}`)
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(figured.map((row) => row.path), { trace: true })
    wasm.forEach((result, i) => assert.equal(result.outputs[GROBS]?.toString(), natives[i], figured[i].id))
  })

/** fixtures/tablature.ly's row, as capture.mjs takes one (MS4 step 17). */
const tablature = { id: 'tablature', file: 'tools/compare/fixtures/tablature.ly', path: fileURLToPath(new URL('../fixtures/tablature.ly', import.meta.url)) }

test("fixtures/tablature.ly natively and in wasm: the FretBoards of Fretboard_engraver (determine-frets' barres in Guile's arrays, D229), the ChordNames and the TabNoteHeads, grobs.tsv LilyPond's (MS4 step 17)",
  { skip: lilypond || cargo, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'tablature')
    assert.equal((await captureOne('lilypond', tablature, dir)).exit, 0)
    const reference = readFileSync(path.join(dir, GROBS), 'utf8')
    const counts = Object.fromEntries(['FretBoard', 'ChordName', 'TabNoteHead'].map((type) => [type, readGrobs(reference).flat().filter((g) => g.type === type).length]))
    assert.deepEqual(counts, { FretBoard: 6, ChordName: 4, TabNoteHead: 14 })
    const out = mkdtempSync(path.join(scratch, 'engine-tablature-'))
    const run = runExample('compile', ['--trace', '--out', out, tablature.path], { cwd: path.dirname(tablature.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `tablature.${name}`), 'utf8')
    const n = readGrobs(reference).flat().length
    assert.deepEqual(compareGrobs(reference, read(GROBS), { types: null, columns: true }), { equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [] })
    assert.equal(read('err'), readFileSync(path.join(dir, 'stderr.txt'), 'utf8'))
    if (coreSkip()) return
    const [wasm] = await compileInWasm([tablature.path], { trace: true })
    assert.equal(wasm.outputs[GROBS]?.toString(), read(GROBS))
  })
