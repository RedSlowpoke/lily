// The references of part 3 of MS3 (progress.md, "How part 3's gate reads";
// MS3 step 25): layout.ly's capture of the live grobs after page breaking,
// and layout.mjs' compare.
//
// - The compare (no LilyPond): a capture against itself passes; a perturbed
//   offset, extent, stencil flag, system or page differs in that field, the
//   offsets and the extents' ends to 1e-4; a moved grob is out of order, a
//   dropped one missing, an added one extra, each apart; the copies of a
//   broken grob pair in order; a field the engine marks unreached is
//   counted and not compared; only COMPARED's types and fields are
//   compared, none at step 25, the vertical ones' Y extents at step 28,
//   their X and Y offsets at step 33.
// - The fixtures of part 3 (fixtures/path.ly and fixtures/y1-unreached.ly),
//   captured with LilyPond 2.26.0: each line pairs with a grobs.tsv line,
//   y1-unreached has Y1's 11 unreached types live, and each capture against
//   itself passes while a perturbed copy of it fails.
// - The five's ref/<id>/layout.tsv: 05 §2.1's census (T01 60, T03 74, T14
//   32, T16 43, T19 90; 299 together), and each against itself passes.
// - The engine's layout.tsv of the five, written by a native traced compile
//   (crates/core/examples/compile.rs --trace, <name>.layout.tsv) from
//   layout's state: a line a live grob of the System, which pairs with its
//   grobs.tsv line, the prebroken pieces (MS3 step 32, D129) with their
//   originals', every grob heard there, every placed field unreached but
//   the Y extent, read since step 28 and unreached where the read did not
//   complete (D126); against ref/ it passes, COMPARED's StaffSymbol,
//   VerticalAxisGroup and VerticalAlignment paired, six fields of each line
//   counted unreached and the Y extents left unreached; every Y extent
//   read, whatever its type, is LilyPond's, and every grob of LilyPond's is
//   there. Since step 37 compiled with the build's preloaded packs, and
//   COMPARED's note heads, rests, dots, dot columns, note columns and paper
//   columns paired too: the heads', rests' and dots' Y extents read and
//   LilyPond's, the note columns' unreached until their stems (step 38),
//   since step 38 the stems' and stem stubs' too and every one of those
//   Y extents read and LilyPond's, since step 39 T19's accidentals' and
//   accidental placements' (none) too and the ledger line spanners' (one a
//   staff, none: its Y-extent is #f), the
//   offsets unreached until the five are broken (step 40), the unbroken
//   System's columns and pieces extra, and the staff's spanners and its
//   first column's items in another order than LilyPond's: until step 39
//   its three spanners were off their common order, since the ledger line
//   spanner makes them four, the three items they are listed beside.
//   Since step 40 (ly:clef::print, then
//   ly:break-alignable-interface::self-align-callback, a BarNumber's
//   X-offset, and ly:system-start-delimiter::print) all five are broken
//   into their lines: every grob of LilyPond's, and no other, in its order
//   at its X and Y offsets and Y extent, on COMPARED and on every type,
//   the system, page, X extent and stencil unreached; the SystemStartBar
//   killed by its print as LilyPond's (one staff) and the BarNumbers by
//   their visibility; their columns.tsv's springs and rods and their
//   line.tsv's X offsets reached, LilyPond's. The live grobs the engine
//   counts after the pages (lsb_times, the compile example's --time; D204)
//   are each score's census and its layout.tsv's lines, 299 together,
//   natively and in wasm: R1's µs per live grob divides by them.
// - fixtures/bar-number.ly (MS3 step 40): a BarNumber whose text is '()
//   prints LilyPond's "Trying to interpret a non-markup object: ()", the
//   engine's stderr LilyPond's line for line natively and in wasm; its
//   layout.tsv natively and in wasm LilyPond's, its three BarNumbers at
//   LilyPond's offsets; its page's C3 and C4 pass.
// - The engine's layout.tsv of fixtures/y1-unreached.ly, natively, against
//   LilyPond's: VerticalAxisGroup's suicide (MS3 step 28). The groups the
//   reads kill, and their elements, have no line, as in LilyPond's; the
//   one more, the Lyrics', dies for want of LyricText (Lyric_engraver is a
//   placeholder, D67). Their breakable items' prebroken pieces have lines
//   until breaking gives the groups their pieces (step 33). Since step 41
//   compiled with the build's preloaded packs, natively and in wasm: its
//   three flags' Y extents LilyPond's (ly:flag::print), their offsets
//   unreached until it is broken (steps 41–43); its three note
//   collisions' grobs.tsv lines LilyPond's (Collision_engraver), and
//   their Y extents; its rest collision's grobs.tsv line LilyPond's
//   (Rest_collision_engraver). Since step 42 its multi-measure rests'
//   grobs.tsv lines LilyPond's (Multi_measure_rest_engraver: the rests,
//   their numbers, the one-measure rests' killed, a text and a script),
//   the eight live ones' Y extents too. Since step 42's session 4 (the
//   clef modifiers' alignment no placeholder) it is broken into its line:
//   the dead groups' pieces on no line, as in LilyPond's; the flags at
//   LilyPond's offsets; the columns that in LilyPond's hold only
//   LyricTexts empty and on no line, the four staves below the Lyrics'
//   line 2.5691 higher (2.5690 since step 51), each difference recorded until step 42's last
//   session records Y1's unreached grobs. Since step 42's session 5 its
//   span bars (Span_bar_engraver): the bar lines' Y extents LilyPond's,
//   widened towards them (OPEN A11), the span bars at LilyPond's offsets,
//   and with them every X offset LilyPond's (the collisions and
//   multi-measure rests were 0.0020 to 0.0056 right); their stubs' grobs.tsv
//   lines LilyPond's (Span_bar_stub_engraver), the stubs on no line with
//   the Lyrics'. Since step 42's last session Y1's unreached grobs are
//   recorded on every type: of LilyPond's 309 the engine's lacks the two
//   InstrumentNames (step 43) and the Lyrics' 16 (A10), every offset
//   LilyPond's but those below the Lyrics' line and the treble clef's
//   modifier (W1's outlines); its page is pages.test.mjs'. Since step 43
//   its two InstrumentNames are laid out (Instrument_name_engraver): their
//   Y offsets and Y extents reached, their X offsets unreached (the
//   brace's extent, whose font no preloaded pack holds); of LilyPond's 309
//   the engine's lacks the Lyrics' 16 alone. Since MS4 step 15 (A10) the
//   Lyrics' group lives with its LyricTexts, LyricSpaces, columns and
//   stubs, and the whole layout.tsv is LilyPond's.
// - fixtures/collisions.ly (MS3 step 41): the note columns of two to four
//   voices shifted, merged and meshed by the note collisions
//   (ly:note-collision-interface::calc-positioning-done), a score a line;
//   natively and in wasm its grobs.tsv LilyPond's line for line, its
//   layout.tsv LilyPond's on every type at its offsets and Y extents (43
//   note collisions, 6 flags), its stderr LilyPond's (the warning of a
//   voice without a shift); its page's C3 and C4 in pages.test.mjs.
// - fixtures/rest-collisions.ly (MS3 step 41): the rests of two and three
//   voices moved clear of each other and of the other voices' notes by
//   the rest collisions (ly:rest-collision::calc-positioning-done), a
//   score a line; natively and in wasm its grobs.tsv LilyPond's line for
//   line, its layout.tsv LilyPond's on every type at its offsets and Y
//   extents (27 rest collisions, 2 flags), its stderr LilyPond's (rests
//   without a direction, too many colliding rests); its page's C3 and C4
//   in pages.test.mjs.
// - fixtures/multi-measure-rests.ly (MS3 step 42): multi-measure rests
//   engraved (Multi_measure_rest_engraver), printed and spaced
//   (multi-measure-rest.cc), a score a line: their numbers (killed at one
//   measure and at restNumberThreshold, set by hand), measure lengths, a
//   text, scripts stacked over and under them; natively and in wasm its
//   grobs.tsv LilyPond's line for line, its layout.tsv LilyPond's on every
//   type at its offsets and Y extents, its stderr LilyPond's (none); its
//   page's C3 and C4 in pages.test.mjs.
// - fixtures/clef-modifiers.ly (MS3 step 42): the clef modifiers of
//   transposed clefs (Clef_engraver) aligned on their clefs
//   (ly:clef-modifier::calc-parent-alignment) below and above, a score a
//   line: G, F and C clefs, 8 and 15, parenthesized and bracketed, a clef
//   forced, a full-size change, clef-alignments set; natively and in wasm
//   its grobs.tsv LilyPond's line for line, its layout.tsv LilyPond's on
//   every type at its offsets and Y extents, its stderr LilyPond's (none);
//   its page's C3 and C4 in pages.test.mjs.
// - fixtures/span-bars.ly (MS3 step 42): span bars over a group's staves
//   (Span_bar_engraver), the bar lines widened towards them (OPEN A11),
//   and stubs in the staves without bar lines between them
//   (Span_bar_stub_engraver), a score a line: groups in groups, repeat
//   bars, a staff that allows no span bar (a stub killed), staves out of
//   their order and of other spaces and lines, measures of their own in
//   each staff; natively and in wasm its grobs.tsv LilyPond's line for
//   line, its layout.tsv LilyPond's on every type at its offsets and Y
//   extents, its stderr LilyPond's (none); its page's C3 and C4 in
//   pages.test.mjs.
// - fixtures/instrument-names.ly (MS3 step 43): instrument and vocal names
//   and their short names (Instrument_name_engraver) printed and placed
//   at the system's start (system-start-text::print, ::calc-x-offset and
//   ::calc-y-offset, the image's), a score a line: names of staves and of
//   groups in groups, a markup name, vocalName where no instrument name
//   is set, a short name alone and a name set mid-line (each killed by its
//   print, as on LilyPond's one line), aligned on their right, left and
//   top, padded, wider than no indent; natively and in wasm its grobs.tsv
//   LilyPond's line for line, its layout.tsv LilyPond's on every type at
//   its offsets and Y extents, its stderr LilyPond's (none); its page's C3
//   and C4 in pages.test.mjs.
// - The same lines from core.wasm's traced compile of the five in one
//   instance (core.mjs, lsb_trace; not 08 §4.2's ABI), and of
//   y1-unreached.ly in another: the native compile's line for line, and
//   against ref/ and LilyPond's capture they pass.
// - columns.tsv (MS3 step 31, D128): the compare of a capture by hand (a
//   perturbed column, wish, spring or rod differs in that field, the
//   numbers to 1e-4; unreached springs and rods counted); LilyPond's of
//   fixtures/path.ly, a line each column and piece named by a column
//   grobs.tsv heard; the five's in ref/ against themselves.
// - The engine's columns.tsv of path.ly natively, with the build's
//   preloaded packs: LilyPond's, each column and its prebroken pieces
//   (MS3 step 32, D129) with LilyPond's spacing wishes, springs and rods;
//   the five's: LilyPond's columns, pieces and wishes, their springs and
//   rods unreached (ly:clef::print, steps 37–40). core.wasm's of both, the
//   native compile's line for line.
// - line.tsv (MS3 step 32, D129): the compare of a capture by hand (a
//   perturbed X offset differs, to 1e-4; a column missing, extra or out of
//   order; an unreached X counted); LilyPond's of fixtures/path.ly, its one
//   line's seven non-musical columns, a piece at each end, at layout.tsv's
//   X offsets; the engine's natively and in wasm, LilyPond's; the five's,
//   every X offset unreached (their springs are), in wasm the native
//   compile's line for line.
// - The X and Y offsets (MS3 step 33, D126): COMPARED's three types gain
//   them; the engine's layout.tsv of path.ly natively and in wasm, with the
//   build's preloaded packs, against LilyPond's on COMPARED and on every
//   type and field: each grob of its broken system in LilyPond's order at
//   LilyPond's offsets and Y extent, the system, page, X extent and stencil
//   unreached. The five's and y1-unreached's offsets stay unreached: they
//   are not broken into lines. Since step 37 COMPARED holds path.ly's seven
//   non-musical columns' X offsets too; y1-unreached's suicide is compared
//   on the three vertical types.
// - Part 4's references (MS3 step 47): LilyPond's captures of its fixtures
//   (scripts, fingerings, arpeggios, line-spanners, brackets, metronome,
//   bar-numbers-staves; Y2's grobs the 10 do not reach, A21's BarNumbers
//   over several staves): each line pairs with grobs.tsv's, each score one
//   line of one page, Y2's types live, each against itself passes, and
//   ref/'s capture of each the same; capture.mjs keeping trace.mjs'
//   trace.tsv (OPEN H5); T10's, T13's and T18's ref/ layout.tsv (309 live
//   grobs); the engine's layout.tsv of the three natively and in wasm, the
//   same line for line, read back against ref/ where each stops: T10 in
//   breaking (no line), T13 at the brace's font and T18 in pre-processing
//   (their unbroken System's grobs, every one of LilyPond's among them),
//   T13 with the brace's packs LilyPond's on every grob.
// - Hairpins and brackets (MS3 step 49): each print of bracket-prints.ly's
//   hairpins, volta, ottava and analysis brackets LilyPond's stencil,
//   natively and in wasm, the volta numbers Emmentaler shapes among them
//   since step 51 (W6); fixtures/brackets.ly with the build's packs
//   LilyPond's on every grob, its two OttavaBrackets' Y offsets since the
//   text packs' outlines (E7, step 51); T10 and T18 laid out whole with
//   their pages, LilyPond's on every grob since step 51 (T10's TextScript's
//   outlines, E7; T18's dynamics' kerning, E6).
// - Line spanners (MS3 step 52): each print of line-spanner-prints.ly's
//   text spanners, glissandi, dynamic text spanners and voice followers,
//   and ly:line-interface::line on note heads in each style (OPEN A23),
//   LilyPond's stencil, natively and in wasm; fixtures/line-spanners.ly
//   with the build's packs LilyPond's grobs.tsv, stderr and layout.tsv on
//   every grob, its page passing C3 and C4, natively and in wasm.
// - Scripts and fingerings (MS3 step 53): fixtures/scripts.ly and
//   fixtures/fingerings.ly with the build's packs LilyPond's grobs.tsv,
//   stderr and layout.tsv on every grob (Script, ScriptColumn, Fingering,
//   StringNumber, StrokeFinger and FingeringColumn among them), each page
//   passing C3 and C4, natively and in wasm.
// - Arpeggios and metronome marks (MS3 step 54): fixtures/arpeggios.ly
//   with the build's packs LilyPond's grobs.tsv, stderr and layout.tsv on
//   every grob (Arpeggio as a chord bracket and a chord slur, solid and
//   dashed, and across a group's staves among them), and
//   fixtures/metronome.ly's the same, its "60 – 72" mark's U+2009 (THIN
//   SPACE), which C059 lacks, drawn as HarfBuzz falls back for it since
//   MS4 step 19 (W6, OPEN E18); each page passing C3 and C4, natively and
//   in wasm.
// - The systems' own lines (MS3 step 60, OPEN H9): each system's line of
//   layout.tsv before its grobs, paired with grobs.tsv's System line, so
//   the 42 references and part 3's and 4's fixtures in ref/ have one a
//   system; the engine writes it with its place on the page unreached,
//   and the census counts grobs, the systems' lines aside.
// - Broken spanners (MS3 step 60, OPEN A40): fixtures/broken-spanners.ly,
//   five systems at its \breaks, captured; the engine given LilyPond's
//   breaks from its line.tsv (the compile example's --break, core.wasm's
//   lsb_breaks; not 08 §4.2's ABI) until step 61 chooses them: natively
//   and in wasm its grobs.tsv, stderr, line.tsv and layout.tsv on every
//   grob LilyPond's, its hairpins', ottava, volta and analysis brackets',
//   text spanners', glissando's and dynamic text spanners' pieces at
//   LilyPond's offsets and Y extents, its page passing C3 and C4; and
//   since step 61 the same at the engine's own breaks.
// - The line breaker (MS3 step 61, OPEN B1's line half): T05 in
//   LilyPond's two systems, natively and in wasm: its grobs.tsv, stderr,
//   line.tsv and layout.tsv with both systems' lines LilyPond's.
// - The page breaker (MS3 step 62, OPEN B1's page half, H9):
//   ly:optimal-breaking breaks the lines and the pages, and layout.tsv is
//   written after the pages, each line with its system and page and each
//   System line with its place on its page; T17 in LilyPond's four systems
//   on two pages and T02 in its three on two, natively and in wasm: their
//   grobs.tsv, stderr, line.tsv and layout.tsv LilyPond's, their pages
//   passing C3 and C4.
// - Titles and top-level markups as page elements (MS3 step 63):
//   fixtures/titles.ly, a book's titles over its first score, each score's
//   piece and opus, top-level markups before and between the scores, a
//   tagline of the book's own and a \bookpart, natively and in wasm: its
//   grobs.tsv, stderr, line.tsv (each system numbered across the book, as
//   layout.ly numbers it) and layout.tsv with each system's page and place
//   LilyPond's, its two pages passing C3 and C4 at Δ 0; their pixels are
//   pages.test.mjs'.
// - Page markers and labels (MS3 step 64): fixtures/page-markers.ly, a
//   toplevel \label before each score and a markup, a \pageBreak and a
//   \noPageBreak between them, a \label in the music, \page-ref and
//   \with-link reading the labels' pages and roman-lower page numbers (B7),
//   natively and in wasm: its grobs.tsv, stderr, line.tsv and layout.tsv
//   with each system's page and place LilyPond's, its two pages passing C3
//   and C4 at Δ 0; their pixels are pages.test.mjs'.
// - MS3's gate on the 10 (MS3 step 65): T01, T03, T05, T10, T13, T14,
//   T16, T17, T18 and T19 traced natively and in one core.wasm instance,
//   with the preloaded packs and the others fetched: their layout.tsv
//   LilyPond's on every field of their 1,461 live grobs and each System's
//   line, in wasm the native compile's line for line; their pages are
//   pages.test.mjs'.
//
// Skipped, and so unverified, without LilyPond 2.26.0, the five's without
// their ref/ capture (tools/corpus/fetch.mjs, capture.mjs), the engine's
// without cargo +stable, and core.wasm's without a build as new as the
// sources (npm run build).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { captureOne, COLUMNS, GROBS, KEPT, LAYOUT, LAYOUT_FIXTURES, LINE, lilypondVersion } from '../capture.mjs'
import { compileInWasm, coreSkip, outlinePacks, preloaded } from '../core.mjs'
import { compareGrobs, readGrobs } from '../grobs.mjs'
import { compare } from '../score.mjs'
import {
  COLUMN_FIELDS, COMPARED, FIELDS, LINE_FIELDS, PLACED, TOLERANCE, UNREACHED, compareColumns, compareLayout, compareLine,
  countTypes, readColumns, readEntries, readLayout, readLine,
} from '../layout.mjs'

const lilypond = lilypondVersion() === '2.26.0' ? false : 'LilyPond 2.26.0 is not installed'
const cargo = spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'
const references = path.join(corpusDir(), 'ref')
/** 05 §2.1's census of Y1's five: their live grobs after page breaking. */
const CENSUS = { T01: 60, T03: 74, T14: 32, T16: 43, T19: 90 }
const skipFive = Object.keys(CENSUS).every((id) => existsSync(path.join(references, id, LAYOUT)))
  ? false : `no layout.tsv in ${references}; run fetch.mjs and capture.mjs`

// The preloaded packs and every text face's outlines (D262): W5's packs are what these compiles ask for.
const packs = [...preloaded(), ...outlinePacks()]
const skipPacks = packs.length ? false : 'no preloaded font packs: run npm run build'
/** The preloaded packs as compileInWasm's modules. */
const packModules = () => packs.map(([name, file]) => [name, readFileSync(file)])

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-layout-ref-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** A capture by hand: T01's first lines, then a tie's two copies on two systems of two pages. */
const CAPTURE = `StaffSymbol\t-\t-\t0 0;2 0\tStaff#0\tStaff_symbol_engraver\t0\t0\t8.5358\t-3.7760\t0.0500 30.8074\t-2.0500 2.0500\t1
VerticalAxisGroup\t-\t-\t0 0;2 0\tStaff#0\tAxis_group_engraver\t0\t0\t8.5358\t-3.7760\t0.0000 30.8574\t-3.5500 3.8000\t0
SpacingSpanner\t-\t-\t0 0;2 0\tScore#0=\\new\tSpacing_engraver\t0\t0\t8.5358\t0.0000\t-\t-\t0
NoteHead\t<ev note-event>\tmelody.ily:2:2:2\t0 0\tVoice#0\tNote_heads_engraver\t0\t0\t17.1208\t-6.7760\t0.0000 1.3042\t-0.5450 0.5450\t1
Clef\t-\t-\t0 0\tStaff#0\tClef_engraver\t0\t0\t9.3358\t-4.7760\t0.0000 2.5650\t-2.5500 4.8000\t1
Stem\t<NoteHead>\tmelody.ily:2:2:2\t0 0\tVoice#0\tStem_engraver\t0\t0\t18.3600\t-3.7760\t-0.0650 0.0650\t-2.8138 0.5000\t1
Tie\t<ev tie-event>\tmelody.ily:3:5:5\t1 0;2 0\tVoice#0\tTie_engraver\t0\t0\t28.0000\t-6.5000\t0.0000 2.8000\t-0.4000 0.3000\t1
Tie\t<ev tie-event>\tmelody.ily:3:5:5\t1 0;2 0\tVoice#0\tTie_engraver\t1\t1\t9.0000\t-6.5000\t0.0000 1.9000\t-0.4000 0.3000\t1
`
const LINES = CAPTURE.split('\n')
/** CAPTURE with line `n` (from 1) given `values` by field. */
function perturb(n, values, text = CAPTURE) {
  const lines = text.split('\n')
  const cells = lines[n - 1].split('\t')
  for (const [field, value] of Object.entries(values)) cells[FIELDS.indexOf(field)] = value
  lines[n - 1] = cells.join('\t')
  return lines.join('\n')
}
const all = { compared: null }

/** The lines of the layout.tsv `text` that are grobs: its systems' own lines (MS3 step 60, OPEN H9) aside. */
const grobLines = (text) => readLayout(text).filter((g) => g.type !== 'System')
/**
 * The fields the engine's traced compile leaves unreached of a layout.tsv like `reference`, counted: every line's
 * X extent and stencil, and each System line's Y extent too. Since MS3 step 62 it writes each line's system and
 * page and each System line's place on its page (OPEN H9), from the pages.
 */
function unreachedOf(reference) {
  const lines = readLayout(reference)
  const n = lines.length
  const s = lines.filter((g) => g.type === 'System').length
  return { 'x-extent': n, stencil: n, ...s && { 'y-extent': s } }
}

test('layout.mjs: a capture against itself passes; a perturbed offset, extent, stencil flag, system or page fails', () => {
  assert.deepEqual(compareLayout(CAPTURE, CAPTURE, all), { equal: true, counts: [8, 8], missing: [], extra: [], order: [], differ: [], unreached: {} })
  assert.equal(readLayout(CAPTURE).length, 8)
  assert.equal(readLayout(CAPTURE)[3].link, 'melody.ily:2:2:2')
  assert.equal(readLayout(CAPTURE)[2]['x-extent'], '-')
  assert.equal(countTypes(readLayout(CAPTURE)).get('Tie'), 2)
  assert.throws(() => readLayout(LINES[0].split('\t').slice(0, 12).join('\t')), /12 fields, not 13/)
  // Each perturbation on the line it changes, and the field it changes.
  const fields = {
    system: [4, '1'], page: [4, '1'], x: [6, '18.3602'], y: [5, '-4.7762'],
    'x-extent': [1, '0.0500 30.8076'], 'y-extent': [2, '-3.5502 3.8000'], stencil: [6, '0'],
  }
  assert.deepEqual(Object.keys(fields), PLACED)
  for (const [field, [line, value]] of Object.entries(fields)) {
    const copy = perturb(line, { [field]: value })
    assert.notEqual(copy, CAPTURE, field)
    const result = compareLayout(CAPTURE, copy, all)
    assert.equal(result.equal, false, field)
    assert.deepEqual([result.missing, result.extra, result.order], [[], [], []], field)
    const reference = LINES[line - 1].split('\t')[FIELDS.indexOf(field)]
    assert.deepEqual(result.differ, [{ line, got: line, field, reference, value }], field)
  }
  // Offsets and the extents' ends to TOLERANCE; an empty extent against one that is not.
  assert.equal(TOLERANCE, 1e-4)
  assert.equal(compareLayout(CAPTURE, perturb(6, { x: '18.3601', y: '-3.7759' }), all).equal, true)
  assert.equal(compareLayout(CAPTURE, perturb(1, { 'x-extent': '0.0499 30.8075' }), all).equal, true)
  assert.equal(compareLayout(CAPTURE, perturb(1, { 'y-extent': '-2.0500 2.0502' }), all).equal, false)
  assert.equal(compareLayout(CAPTURE, perturb(3, { 'x-extent': '0.0000 0.0000' }), all).differ.length, 1)
  assert.equal(compareLayout(CAPTURE, perturb(1, { 'x-extent': '-' }), all).differ.length, 1)
  assert.equal(compareLayout(CAPTURE, perturb(6, { x: '+inf' }), all).differ.length, 1)
  assert.equal(compareLayout(perturb(6, { x: '+inf' }), perturb(6, { x: '+inf' }), all).equal, true)
})

test('layout.mjs: a missing grob, an extra one and one out of order are reported apart; broken copies pair in order', () => {
  const dropped = compareLayout(CAPTURE, LINES.filter((_, i) => i !== 4).join('\n'), all)
  assert.deepEqual([dropped.missing.map((m) => m.line), dropped.extra, dropped.order, dropped.differ], [[5], [], [], []])
  assert.equal(dropped.missing[0].grob, LINES[4].split('\t').slice(0, 6).join('\t'))
  const added = compareLayout(CAPTURE, CAPTURE + perturb(1, { type: 'BarLine' }).split('\n')[0] + '\n', all)
  assert.deepEqual([added.extra.map((m) => m.line), added.missing, added.order, added.differ], [[9], [], [], []])
  // The Clef moved before the note head: one grob out of order, its fields still compared.
  const moved = [...LINES.slice(0, 3), LINES[4], LINES[3], ...LINES.slice(5)].join('\n')
  const order = compareLayout(CAPTURE, moved, all)
  assert.deepEqual([order.missing, order.extra, order.differ], [[], [], []])
  assert.equal(order.order.length, 1)
  assert.ok([[4, 5], [5, 4]].some(([line, got]) => order.order[0].line === line && order.order[0].got === got))
  assert.equal(compareLayout(CAPTURE, perturb(4, { x: '0.0000' }, moved), all).differ[0].field, 'x')
  // A broken tie's copies pair first with first: swapped, each differs from its own in its system, page, x and x-extent.
  const swapped = [...LINES.slice(0, 6), LINES[7], LINES[6], ''].join('\n')
  const ties = compareLayout(CAPTURE, swapped, all)
  assert.deepEqual([ties.missing, ties.extra, ties.order], [[], [], []])
  assert.deepEqual(ties.differ.map((d) => [d.line, d.got, d.field]), [
    [7, 7, 'system'], [7, 7, 'page'], [7, 7, 'x'], [7, 7, 'x-extent'],
    [8, 8, 'system'], [8, 8, 'page'], [8, 8, 'x'], [8, 8, 'x-extent'],
  ])
  // One copy dropped: the second is missing, the first pairs.
  assert.deepEqual(compareLayout(CAPTURE, LINES.filter((_, i) => i !== 7).join('\n'), all).missing.map((m) => m.line), [8])
})

test('layout.mjs: unreached fields are counted and not compared; only COMPARED is compared, the vertical Y extents at step 28, their offsets at step 33, the clef, the start bar and the bar number at step 40, the flag and the note and rest collisions at step 41, the multi-measure rests, the clef modifier, the bar line, the span bar and its stub at step 42, the instrument name at step 43', () => {
  const marked = PLACED.reduce((text, field) => perturb(6, { [field]: UNREACHED }, text), perturb(4, { x: UNREACHED, y: UNREACHED }))
  const result = compareLayout(CAPTURE, marked, all)
  assert.equal(result.equal, true)
  assert.deepEqual(result.unreached, { system: 1, page: 1, x: 2, y: 2, 'x-extent': 1, 'y-extent': 1, stencil: 1 })
  // Counted on every line of the other side, whatever is compared; never on the reference's.
  assert.deepEqual(compareLayout(CAPTURE, marked, { compared: {} }).unreached, result.unreached)
  assert.deepEqual(compareLayout(marked, CAPTURE, all).unreached, {})
  // A field reached and wrong beside unreached ones still differs.
  assert.deepEqual(compareLayout(CAPTURE, perturb(4, { y: '0.0000' }, perturb(4, { x: UNREACHED })), all).differ.map((d) => d.field), ['y'])
  // COMPARED: the types compared and the fields of each; the rest is neither paired nor compared.
  const vertical = ['x', 'y', 'y-extent']
  assert.deepEqual(COMPARED, {
    StaffSymbol: vertical, VerticalAxisGroup: vertical, VerticalAlignment: vertical,
    NoteHead: vertical, Rest: vertical, Dots: vertical, DotColumn: vertical, NoteColumn: vertical,
    Stem: vertical, StemStub: vertical, Accidental: vertical, AccidentalPlacement: vertical,
    LedgerLineSpanner: vertical, Clef: vertical, KeySignature: vertical, SystemStartBar: vertical, BarNumber: vertical,
    Flag: vertical, NoteCollision: vertical, RestCollision: vertical,
    MultiMeasureRest: vertical, MultiMeasureRestNumber: vertical, MultiMeasureRestText: vertical, MultiMeasureRestScript: vertical,
    ClefModifier: vertical, BarLine: vertical, SpanBar: vertical, SpanBarStub: vertical, InstrumentName: vertical,
    PaperColumn: ['x'], NonMusicalPaperColumn: ['x'],
  })
  // The Clef too since step 40.
  const two = compareLayout(CAPTURE, [...LINES.slice(0, 2), ...LINES.slice(3, 6)].join('\n'))
  assert.deepEqual([two.equal, two.counts], [true, [5, 5]])
  assert.deepEqual(compareLayout(CAPTURE, LINES.slice(0, 2).join('\n')).missing.map((m) => m.line), [4, 5, 6])
  assert.deepEqual(compareLayout(CAPTURE, perturb(4, { x: '17.1210' })).differ.map((d) => [d.line, d.field]), [[4, 'x']])
  assert.equal(compareLayout(CAPTURE, perturb(1, { system: '1', page: '1', 'x-extent': '-', stencil: '0' })).equal, true)
  assert.deepEqual(compareLayout(CAPTURE, perturb(2, { 'y-extent': '-' })).differ.map((d) => [d.line, d.field]), [[2, 'y-extent']])
  assert.deepEqual(compareLayout(CAPTURE, perturb(1, { x: '8.5360', y: '-3.7758' })).differ.map((d) => [d.line, d.field]), [[1, 'x'], [1, 'y']])
  assert.deepEqual(compareLayout(CAPTURE, LINES.slice(1, 2).join('\n')).missing.map((m) => m.line), [1, 4, 5, 6])
  const stem = { compared: { Stem: ['x'], Tie: [] } }
  assert.deepEqual(compareLayout(CAPTURE, CAPTURE, stem).counts, [3, 3])
  assert.equal(compareLayout(CAPTURE, perturb(6, { y: '0.0000' }), stem).equal, true)
  assert.equal(compareLayout(CAPTURE, perturb(7, { x: '0.0000' }), stem).equal, true)
  assert.deepEqual(compareLayout(CAPTURE, perturb(6, { x: '0.0000' }), stem).differ.map((d) => [d.line, d.field]), [[6, 'x']])
  assert.equal(compareLayout(CAPTURE, LINES.filter((_, i) => i !== 3).join('\n'), stem).equal, true)
  // A compared type with no fields: its grobs paired alone, the tie's second copy missing.
  assert.deepEqual(compareLayout(CAPTURE, LINES.filter((_, i) => i !== 6).join('\n'), stem).missing.map((m) => m.line), [8])
})

test('layout.mjs on the command line: exit 0 on a capture against itself, 1 on a perturbed one, 2 on bad arguments', () => {
  const dir = (name, text) => {
    const d = path.join(scratch, 'cli', name)
    mkdirSync(d, { recursive: true })
    writeFileSync(path.join(d, LAYOUT), text)
    return d
  }
  const [a, b] = [dir('a', CAPTURE), dir('b', perturb(6, { stencil: '0' }))]
  const cli = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../layout.mjs')
  const run = (...args) => spawnSync(process.execPath, [cli, ...args], { encoding: 'utf8' })
  assert.equal(run('--all', a, a).status, 0)
  const differs = run('--all', a, b)
  assert.equal(differs.status, 1)
  assert.deepEqual(JSON.parse(differs.stdout).differ.map((d) => d.field), ['stencil'])
  // Without --all only COMPARED's: the stem's stencil is not.
  assert.equal(run(a, b).status, 0)
  assert.equal(run('--all', a).status, 2)
})

/** LilyPond's capture of part 3's fixture `id` into scratch/<id>, made once each: its directory. */
const fixtures = new Map()
const captureFixture = async (id) => {
  const dir = path.join(scratch, id)
  if (!fixtures.has(id)) fixtures.set(id, captureOne('lilypond', LAYOUT_FIXTURES.find((row) => row.id === id), dir))
  const result = await fixtures.get(id)
  assert.equal(result.exit, 0, id)
  assert.equal(readFileSync(path.join(dir, 'stderr.txt'), 'utf8'), '', id)
  return dir
}

/** Every one of `text`'s perturbations of PLACED's fields fails against it, on the line of `type`. */
function assertPerturbedFail(text, type) {
  const lines = text.split('\n')
  const n = lines.findIndex((l) => l.startsWith(`${type}\t`)) + 1
  assert.ok(n > 0, type)
  const cells = lines[n - 1].split('\t')
  const shift = (v) => (+v + 2 * TOLERANCE).toFixed(4)
  const extent = (e) => e === '-' ? '0.0000 1.0000' : e.split(' ').map((v, i) => i ? shift(v) : v).join(' ')
  const at = (f) => cells[FIELDS.indexOf(f)]
  const values = {
    system: `${+at('system') + 1}`, page: `${+at('page') + 1}`, x: shift(at('x')), y: shift(at('y')),
    'x-extent': extent(at('x-extent')), 'y-extent': extent(at('y-extent')), stencil: at('stencil') === '1' ? '0' : '1',
  }
  for (const [field, value] of Object.entries(values)) {
    const result = compareLayout(text, perturb(n, { [field]: value }, text), all)
    assert.deepEqual(result.differ.map((d) => [d.line, d.field]), [[n, field]], `${type} ${field}`)
  }
}

test("LilyPond captures part 3's fixtures: each line pairs with grobs.tsv's; Y1's 11 unreached types live; the compare on them", { skip: lilypond, timeout: 120_000 }, async () => {
  for (const id of ['path', 'y1-unreached']) {
    const dir = await captureFixture(id)
    const text = readFileSync(path.join(dir, LAYOUT), 'utf8')
    const grobs = readLayout(text)
    const announced = new Set(readGrobs(readFileSync(path.join(dir, GROBS), 'utf8')).flat().map((g) => FIELDS.slice(0, 6).map((f) => g[f]).join('\t')))
    for (const g of grobs) assert.ok(announced.has(FIELDS.slice(0, 6).map((f) => g[f]).join('\t')), `${id}:${g.line}`)
    assert.ok(grobs.every((g) => g.system === '0' && g.page === '0'), id)
    assert.deepEqual(compareLayout(text, text, all).counts, [grobs.length, grobs.length], id)
    assert.equal(compareLayout(text, text, all).equal, true, id)
    assertPerturbedFail(text, id === 'path' ? 'BarLine' : 'Flag')
  }
  const skips = countTypes(grobLines(readFileSync(path.join(scratch, 'path', LAYOUT), 'utf8')))
  assert.equal([...skips.values()].reduce((a, b) => a + b), 43)
  for (const type of ['Clef', 'KeySignature', 'LedgerLineSpanner', 'SystemStartBar', 'BarNumber']) assert.equal(skips.get(type), undefined, type)
  const y1 = countTypes(readLayout(readFileSync(path.join(scratch, 'y1-unreached', LAYOUT), 'utf8')))
  assert.deepEqual(Object.fromEntries(['Flag', 'NoteCollision', 'RestCollision', 'MultiMeasureRest', 'MultiMeasureRestNumber',
    'MultiMeasureRestText', 'MultiMeasureRestScript', 'ClefModifier', 'InstrumentName', 'SpanBar', 'SpanBarStub'].map((t) => [t, y1.get(t)])), {
    Flag: 3, NoteCollision: 3, RestCollision: 1, MultiMeasureRest: 4, MultiMeasureRestNumber: 2,
    MultiMeasureRestText: 1, MultiMeasureRestScript: 1, ClefModifier: 2, InstrumentName: 2, SpanBar: 5, SpanBarStub: 4,
  })
})

test("Y1's five's layout.tsv: 05 §2.1's census, 299 live grobs, and each against itself passes", { skip: skipFive }, () => {
  const counts = {}
  for (const id of Object.keys(CENSUS)) {
    const text = readFileSync(path.join(references, id, LAYOUT), 'utf8')
    counts[id] = grobLines(text).length
    assert.equal(compareLayout(text, text, all).equal, true, id)
    assertPerturbedFail(text, 'Stem')
  }
  assert.deepEqual(counts, CENSUS)
  assert.equal(Object.values(counts).reduce((a, b) => a + b), 299)
})

const five = corpus().filter((row) => row.id in CENSUS)
/**
 * The engine's traced compile of the five natively with the build's preloaded packs, made once: each id to its
 * outputs' text by name, and `live`, the live grobs --time counts (its last column).
 */
let compiledFive
const compileFive = () => compiledFive ??= Object.fromEntries(five.map((row) => {
  const out = mkdtempSync(path.join(scratch, `engine-${row.id}-`))
  const run = runExample('compile', ['--trace', '--time', ...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, row.path],
    { cwd: path.dirname(row.path), encoding: 'utf8' })
  // 1: the file failed, as T03 does in LilyPond.
  assert.equal(run.status, row.id === 'T03' ? 1 : 0, `${row.id}: ${run.stderr}`)
  const read = (name) => readFileSync(path.join(out, `${path.basename(row.path, '.ly')}.${name}`), 'utf8')
  // --time's twelfth field, the live grobs (the \midi's and the MIDI files' after it since MS5 step 7).
  const live = +run.stdout.trim().split('\t')[11]
  return [row.id, { ...Object.fromEntries(['trace.log', GROBS, LAYOUT, COLUMNS, LINE].map((name) => [name, read(name)])), live }]
}))

/**
 * The engine's layout.tsv of five's `id`, given its trace.log: broken into
 * its lines since step 40, against ref/ on COMPARED and on every type and
 * field LilyPond's, each grob of LilyPond's in its order at its X and Y
 * offsets and Y extent and no other (the SystemStartBar killed by its
 * print), the system, page, X extent and stencil unreached.
 */
function assertEngineLayout(id, outputs, where) {
  assert.doesNotMatch(outputs['trace.log'], /layout\.tsv/, `${id} ${where}`)
  const text = outputs[LAYOUT]
  const reference = readFileSync(path.join(references, id, LAYOUT), 'utf8')
  const n = readLayout(reference).length
  const unreached = unreachedOf(reference)
  const m = readLayout(reference).filter((g) => g.type in COMPARED).length
  assert.deepEqual(compareLayout(reference, text), {
    equal: true, counts: [m, m], missing: [], extra: [], order: [], differ: [], unreached,
  }, `${id} ${where}`)
  assert.deepEqual(compareLayout(reference, text, all), {
    equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [], unreached,
  }, `${id} ${where}`)
}

test("the engine's layout.tsv of Y1's five, from a native traced compile: LilyPond's, every grob at its offsets and Y extent",
  { skip: skipFive || cargo || skipPacks, timeout: 300_000 }, () => {
    for (const [id, outputs] of Object.entries(compileFive())) assertEngineLayout(id, outputs, 'natively')
  })

test("the live grobs the engine counts of Y1's five, natively and in core.wasm: 05 §2.1's census and layout.tsv's lines, 299",
  { skip: skipFive || cargo || skipPacks || coreSkip(), timeout: 300_000 }, async () => {
    const wasm = await compileInWasm(five.map((row) => row.path), { modules: packModules() })
    const counts = (live) => Object.fromEntries(five.map((row, i) => [row.id, live(row, i)]))
    const lines = counts((row) => grobLines(compileFive()[row.id][LAYOUT]).length)
    assert.deepEqual(lines, CENSUS)
    assert.deepEqual(counts((row) => compileFive()[row.id].live), CENSUS)
    assert.deepEqual(counts((_, i) => wasm[i].times.liveGrobs), CENSUS)
  })

const y1Unreached = LAYOUT_FIXTURES.find((row) => row.id === 'y1-unreached').path
/**
 * The engine's traced compile of fixtures/y1-unreached.ly natively, made once, with the build's preloaded packs since
 * step 41 (the flags' glyphs): its outputs' text by name.
 */
let compiledY1
const compileY1 = () => compiledY1 ??= (() => {
  const out = mkdtempSync(path.join(scratch, 'engine-y1-unreached-'))
  const run = runExample('compile', ['--trace', ...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, y1Unreached],
    { cwd: path.dirname(y1Unreached), encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  return Object.fromEntries(['trace.log', GROBS, LAYOUT].map((name) => [name, readFileSync(path.join(out, `y1-unreached.${name}`), 'utf8')]))
})()

/**
 * The engine's layout.tsv of y1-unreached.ly, given its trace.log and grobs.tsv, against LilyPond's `reference` and,
 * since step 41, its grobs.tsv `grobs`.
 */
function assertSuicide(reference, grobs, outputs, where) {
  assert.doesNotMatch(outputs['trace.log'], /layout\.tsv/, where)
  const key = (g) => FIELDS.slice(0, 6).map((f) => g[f]).join('\t')
  const keys = (grobs) => grobs.map(key).reduce((m, k) => m.set(k, (m.get(k) ?? 0) + 1), new Map())
  const text = outputs[LAYOUT]
  const [heard, laid, lilypond] = [keys(readGrobs(outputs[GROBS]).flat().slice(0, -1)), keys(readLayout(text)), keys(readLayout(reference))]
  // What the reads killed: the layer-2 staff (make-dead-when) and the
  // staff of skips with all their grobs, which LilyPond's has none of.
  // Until MS4 step 15 the Lyrics' group too, the musical columns of 2 and
  // 5 that hold only its LyricTexts, and its four SpanBarStubs (A10);
  // since, all three live, as in LilyPond's.
  const dead = [...heard].filter(([k, n]) => n > (laid.get(k) ?? 0)).map(([k]) => k)
  assert.deepEqual(dead.filter((k) => lilypond.has(k)), [], where)
  const groups = (m) => [...m.keys()].filter((k) => k.startsWith('VerticalAxisGroup\t')).map((k) => k.split('\t')[4])
  assert.deepEqual(groups(heard), ['Staff#0', 'Lyrics#0', 'Staff#1', 'Staff#2', 'Staff#3', 'Staff#4', 'Staff#5', 'Staff#6'], where)
  assert.deepEqual(groups(lilypond), ['Staff#0', 'Lyrics#0', 'Staff#1', 'Staff#2', 'Staff#3', 'Staff#4'], where)
  assert.deepEqual(groups(laid), groups(lilypond), where)
  // Their breakable items' prebroken pieces (MS3 step 32, D129) outlived
  // them while the fixture was not broken; broken (step 42), no piece of
  // theirs is on the line, as in LilyPond's.
  for (const staff of ['Staff#5', 'Staff#6']) {
    assert.ok(dead.some((k) => k.startsWith(`StaffSymbol\t`) && k.split('\t')[4] === staff), `${staff} ${where}`)
    assert.deepEqual([...laid.keys()].filter((k) => k.split('\t')[4] === staff), [], `${staff} ${where}`)
  }
  // Against LilyPond's on the vertical types: every group, staff and Y
  // extent LilyPond's since MS4 step 15 (the Lyrics' line, A10; until
  // then the four staves below it 2.5690 higher and the alignment as much
  // shorter).
  const vertical = Object.fromEntries(['StaffSymbol', 'VerticalAxisGroup', 'VerticalAlignment'].map((t) => [t, COMPARED[t]]))
  const result = compareLayout(reference, text, { compared: vertical })
  assert.deepEqual([result.missing, result.extra, result.order, result.differ], [[], [], [], []], where)
  assert.deepEqual(result.counts, [12, 12], where)
  const compared = readLayout(text).filter((g) => g.type in vertical)
  assert.deepEqual(compared.filter((g) => [g.x, g.y, g['y-extent']].includes(UNREACHED)), [], where)
  assertFlags(reference, text, where)
  assertCollisions(reference, grobs, outputs, where)
  assertMultiMeasureRests(reference, grobs, outputs, where)
  assertSpanBars(reference, grobs, outputs, where)
  assertUnreached(reference, text, where)
}

/**
 * Y1's unreached grobs (MS3 step 42's last session), on every type and
 * placed field of y1-unreached's layout.tsv against LilyPond's: of its
 * 309 live grobs the engine's lacks 16, the Lyrics' (A10: its group, its
 * five LyricTexts and four LyricSpaces, the four SpanBarStubs on its
 * line, the two columns that hold only LyricTexts), and until MS3 step 43
 * the two InstrumentNames too; none extra or out of order. Every offset
 * reached LilyPond's but the grobs below the Lyrics' line, 2.5690 high
 * (2.5691 until the text packs' outlines, MS3 step 51, which also put the
 * treble clef's ClefModifier, 0.0370 off its clef's outline before, at
 * LilyPond's: A27); the Y extents that span that line as much shorter, the
 * brace's among them. Until MS3 step 50 the brace's X offset and Y extent were
 * unreached, no preloaded pack holding its font, and with them the
 * InstrumentNames' X offsets (system-start-text::calc-x-offset reads the
 * delimiters' extents); since then its packs are fetched (W5) and every
 * field is reached, those X offsets LilyPond's. Since MS4 step 15 the
 * Lyrics' 16 are made and laid out (A10), and every grob and field is
 * LilyPond's.
 */
function assertUnreached(reference, text, where) {
  const result = compareLayout(reference, text, { compared: null })
  const types = (list) => Object.fromEntries(countTypes(list.map((m) => ({ type: m.grob.split('\t')[0] }))))
  // Its 309 live grobs and its system's own line (MS3 step 60), all of
  // them since MS4 step 15 (A10), every field LilyPond's.
  assert.deepEqual(result.counts, [310, 310], where)
  assert.deepEqual([types(result.missing), result.extra, result.order, result.differ], [{}, [], [], []], where)
  assert.deepEqual(grobLines(text).filter((g) => g.x === UNREACHED || g['y-extent'] === UNREACHED), [], where)
}

/** COMPARED's types of span bars (MS3 step 42), with the bar lines they widen. */
const SPAN_BARS = ['BarLine', 'SpanBar', 'SpanBarStub']

/**
 * y1-unreached's span bars (MS3 step 42): their and their stubs' grobs.tsv
 * lines LilyPond's (Span_bar_engraver, Span_bar_stub_engraver); the five
 * span bars at LilyPond's offsets; the bar lines' Y extents LilyPond's,
 * the StaffGroup's widened towards its span bars (OPEN A11: -2.05 and
 * 2.05, -2.0 until they were made), their X offsets LilyPond's, those
 * below the Lyrics' line 2.5690 higher and the four stubs, the Lyrics',
 * on no line with it until MS4 step 15 (A10); every one LilyPond's since.
 */
function assertSpanBars(reference, grobs, outputs, where) {
  const lines = (text) => text.split('\n').filter((line) => line.startsWith('SpanBar'))
  assert.deepEqual(lines(outputs[GROBS]), lines(grobs), where)
  assert.deepEqual(lines(grobs).map((line) => line.split('\t').filter((_, i) => i === 0 || i === 3).join(' ')), [
    'SpanBar 1 0', 'SpanBarStub 1 0', 'SpanBar 2 0', 'SpanBarStub 2 0', 'SpanBar 5 0', 'SpanBarStub 5 0',
    'SpanBar 6 0', 'SpanBarStub 6 0', 'SpanBar 7 0',
  ], where)
  const compared = Object.fromEntries(SPAN_BARS.map((t) => [t, COMPARED[t]]))
  const result = compareLayout(reference, outputs[LAYOUT], { compared })
  // The Lyrics' four stubs on its line since MS4 step 15 (A10), every bar
  // line at LilyPond's offset.
  assert.deepEqual([result.missing, result.extra, result.order, result.differ, result.counts], [[], [], [], [], [25, 25]], where)
  const extents = (text) => readLayout(text).filter((g) => g.type === 'BarLine' && g.context.match(/^Staff#[01]$/))
    .map((g) => g['y-extent'])
  assert.deepEqual(extents(outputs[LAYOUT]), Array(5).fill(['-2.0500 2.0000', '-2.0000 2.0500']).flat(), where)
  assert.deepEqual(readLayout(outputs[LAYOUT]).filter((g) => g.type === 'SpanBar').map((g) => [g.x, g.y, g['y-extent']].join(' ')),
    ['33.9458', '44.8006', '57.4606', '65.3506', '78.4961'].map((x) => `${x} 0.0000 -`), where)
}

/** COMPARED's multi-measure rest types (MS3 step 42). */
const MULTI_MEASURE = ['MultiMeasureRest', 'MultiMeasureRestNumber', 'MultiMeasureRestText', 'MultiMeasureRestScript']

/**
 * y1-unreached's multi-measure rests (MS3 step 42): their grobs.tsv lines
 * LilyPond's (Multi_measure_rest_engraver), the two compressed rests of
 * three measures with their numbers, one with its text, the two of one
 * measure with their numbers killed (no columns), one with its script;
 * the live ones' Y extents LilyPond's. Broken (step 42, session 4), their
 * offsets reached: those of the second staff 2.5690 high (no Lyrics'
 * line), the others LilyPond's; since its span bars (step 42, session 5)
 * every X offset LilyPond's (0.0055 to 0.0056 right until then); since
 * MS4 step 15 (the Lyrics' line, A10) every offset LilyPond's.
 */
function assertMultiMeasureRests(reference, grobs, outputs, where) {
  const lines = (text) => text.split('\n').filter((line) => line.startsWith('MultiMeasureRest'))
  assert.deepEqual(lines(outputs[GROBS]), lines(grobs), where)
  assert.deepEqual(lines(grobs).map((line) => line.split('\t').filter((_, i) => i === 0 || i === 3).join(' ')), [
    'MultiMeasureRest 2 0;5 0', 'MultiMeasureRestNumber 2 0;5 0', 'MultiMeasureRestText 2 0;5 0',
    'MultiMeasureRest 2 0;5 0', 'MultiMeasureRestNumber 2 0;5 0',
    'MultiMeasureRest 5 0;6 0', 'MultiMeasureRestNumber -', 'MultiMeasureRestScript 5 0;6 0',
    'MultiMeasureRest 5 0;6 0', 'MultiMeasureRestNumber -',
  ], where)
  const compared = Object.fromEntries(MULTI_MEASURE.map((t) => [t, COMPARED[t]]))
  const result = compareLayout(reference, outputs[LAYOUT], { compared })
  assert.deepEqual([result.missing, result.extra, result.order, result.counts], [[], [], [], [8, 8]], where)
  assert.deepEqual(result.differ, [], where)
  const laid = readLayout(outputs[LAYOUT]).filter((g) => MULTI_MEASURE.includes(g.type))
  assert.deepEqual(laid.filter((g) => [g.x, g.y].includes(UNREACHED)), [], where)
  assert.deepEqual(laid.map((g) => g['y-extent']),
    readLayout(reference).filter((g) => MULTI_MEASURE.includes(g.type)).map((g) => g['y-extent']), where)
}

/**
 * y1-unreached's three note collisions (MS3 step 41): their grobs.tsv
 * lines LilyPond's (Collision_engraver), their Y extents LilyPond's,
 * their offsets reached since the fixture is broken (step 42), and
 * LilyPond's since its span bars (step 42, session 5; the third's X was
 * 0.0020 right). And its rest collision: its grobs.tsv line LilyPond's
 * (Rest_collision_engraver), no extent, at LilyPond's offsets.
 */
function assertCollisions(reference, grobs, outputs, where) {
  const lines = (text) => text.split('\n').filter((line) => line.startsWith('NoteCollision\t'))
  assert.deepEqual(lines(outputs[GROBS]), lines(grobs), where)
  assert.equal(lines(grobs).length, 3, where)
  const result = compareLayout(reference, outputs[LAYOUT], { compared: { NoteCollision: COMPARED.NoteCollision } })
  assert.deepEqual([result.missing, result.extra, result.order, result.counts], [[], [], [], [3, 3]], where)
  assert.deepEqual(result.differ, [], where)
  assert.deepEqual(readLayout(outputs[LAYOUT]).filter((g) => g.type === 'NoteCollision').map((g) => [g.x === UNREACHED, g.y === UNREACHED, g['y-extent']]),
    [[false, false, '-4.6667 2.5000'], [false, false, '-4.3333 3.0000'], [false, false, '-4.0000 3.3333']], where)
  const rests = (text) => text.split('\n').filter((line) => line.startsWith('RestCollision\t'))
  assert.deepEqual(rests(outputs[GROBS]), rests(grobs), where)
  assert.deepEqual(rests(grobs), ['RestCollision\t-\t-\t1 0\tStaff#0\tRest_collision_engraver'], where)
  const rest = compareLayout(reference, outputs[LAYOUT], { compared: { RestCollision: COMPARED.RestCollision } })
  assert.deepEqual([rest.missing, rest.extra, rest.order, rest.counts], [[], [], [], [1, 1]], where)
  assert.deepEqual(rest.differ, [], where)
  assert.deepEqual(readLayout(outputs[LAYOUT]).filter((g) => g.type === 'RestCollision').map((g) => [g.x === UNREACHED, g.y === UNREACHED, g['y-extent']]),
    [[false, false, '-']], where)
}

/**
 * y1-unreached's three flags (MS3 step 41): against LilyPond's, their Y
 * extents LilyPond's, and since the fixture is broken (step 42, session
 * 4) their offsets too.
 */
function assertFlags(reference, text, where) {
  const result = compareLayout(reference, text, { compared: { Flag: COMPARED.Flag } })
  assert.deepEqual([result.missing, result.extra, result.order, result.differ, result.counts], [[], [], [], [], [3, 3]], where)
  assert.deepEqual(readLayout(text).filter((g) => g.type === 'Flag').map((g) => [g.x === UNREACHED, g.y === UNREACHED, g['y-extent']]),
    [[false, false, '-3.0502 0.0650'], [false, false, '-3.5502 0.0650'], [false, false, '-3.5502 0.0650']], where)
}

test("the engine's layout.tsv of y1-unreached.ly, natively: VerticalAxisGroup's suicide as LilyPond's, the dead groups' elements and pieces without a line; the flags, note collisions, multi-measure rests and span bars at LilyPond's X offsets and Y extents, the bar lines widened (A11), the Y offsets' differences recorded",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const dir = await captureFixture('y1-unreached')
    const read = (name) => readFileSync(path.join(dir, name), 'utf8')
    assertSuicide(read(LAYOUT), read(GROBS), compileY1(), 'natively')
  })

test("core.wasm's layout.tsv of Y1's five, from a traced compile in one instance, is the native compile's line for line; against ref/ it passes",
  { skip: skipFive || cargo || skipPacks || coreSkip(), timeout: 300_000 }, async () => {
    const wasm = await compileInWasm(five.map((row) => row.path), { trace: true, modules: packModules() })
    const native = compileFive()
    for (const [i, row] of five.entries()) {
      const outputs = Object.fromEntries(['trace.log', GROBS, LAYOUT, COLUMNS].map((name) => [name, wasm[i].outputs[name]?.toString()]))
      assert.equal(outputs[LAYOUT], native[row.id][LAYOUT], row.id)
      assert.equal(outputs[COLUMNS], native[row.id][COLUMNS], row.id)
      assertEngineLayout(row.id, outputs, 'in wasm')
    }
  })

test("core.wasm's layout.tsv of y1-unreached.ly is the native compile's line for line: the same suicide, flags, note collisions, multi-measure rests and span bars as LilyPond's",
  { skip: lilypond || cargo || skipPacks || coreSkip(), timeout: 300_000 }, async () => {
    const dir = await captureFixture('y1-unreached')
    const read = (name) => readFileSync(path.join(dir, name), 'utf8')
    const [wasm] = await compileInWasm([y1Unreached], { trace: true, modules: packModules() })
    const outputs = Object.fromEntries(['trace.log', GROBS, LAYOUT].map((name) => [name, wasm.outputs[name]?.toString()]))
    assert.equal(outputs[LAYOUT], compileY1()[LAYOUT])
    assertSuicide(read(LAYOUT), read(GROBS), outputs, 'in wasm')
  })

/** fixtures/collisions.ly's row, as capture.mjs takes one. */
const collisions = { id: 'collisions', file: 'tools/compare/fixtures/collisions.ly', path: fileURLToPath(new URL('../fixtures/collisions.ly', import.meta.url)) }
/** LilyPond's warning at the second and third columns of collisions.ly's voice without a shift. */
const NO_SHIFT = [30, 27].map((column) => `${collisions.path}:40:${column}: warning: this Voice needs a \\voiceXx or \\shiftXx setting\n`)
/**
 * collisions.ly's outputs against LilyPond's capture in `dir`: its grobs.tsv LilyPond's line for line, its layout.tsv
 * on every type at LilyPond's offsets and Y extents, the system, page, X extent and stencil unreached; its stderr
 * LilyPond's.
 */
function assertCollisionsFixture(dir, { grobs, layout, stderr }, where) {
  const read = (name) => readFileSync(path.join(dir, name), 'utf8')
  assert.equal(stderr, read('stderr.txt'), where)
  assert.equal(stderr.split('\n').filter((line) => line.includes('warning:')).map((line) => `${line}\n`).join(''), NO_SHIFT.join(''), where)
  assert.deepEqual(compareGrobs(read(GROBS), grobs), { equal: true, counts: [1119, 1119], missing: [], extra: [], order: [], differ: [] }, where)
  const reference = read(LAYOUT)
  const n = readLayout(reference).length
  const unreached = unreachedOf(reference)
  assert.deepEqual(compareLayout(reference, layout, all), {
    equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [], unreached,
  }, where)
  // LilyPond's five lines; the note columns of 24 moments shifted apart; 43 note collisions and 6 flags.
  assert.deepEqual([...new Set(readLayout(reference).map((g) => g.system))], ['0', '1', '2', '3', '4'], where)
  const at = new Map()
  for (const g of readLayout(reference).filter((g) => g.type === 'NoteColumn')) {
    const key = `${g.system} ${g.columns}`
    at.set(key, new Set([...(at.get(key) ?? []), g.x]))
  }
  assert.equal([...at.values()].filter((xs) => xs.size > 1).length, 24, where)
  const types = countTypes(readLayout(layout))
  assert.deepEqual([types.get('NoteCollision'), types.get('Flag')], [43, 6], where)
}

test("fixtures/collisions.ly natively and in wasm: the note collisions' grobs.tsv, layout.tsv and stderr LilyPond's",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'collisions')
    const result = await captureOne('lilypond', collisions, dir)
    assert.equal(result.exit, 0)
    const out = mkdtempSync(path.join(scratch, 'engine-collisions-'))
    const run = runExample('compile', ['--trace', ...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, collisions.path],
      { cwd: path.dirname(collisions.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `collisions.${name}`), 'utf8')
    const native = { grobs: read(GROBS), layout: read(LAYOUT), stderr: read('err') }
    assertCollisionsFixture(dir, native, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([collisions.path], { trace: true, modules: packModules() })
    const inWasm = { grobs: wasm.outputs[GROBS]?.toString(), layout: wasm.outputs[LAYOUT]?.toString(), stderr: wasm.outputs.stderr?.toString() }
    assert.equal(inWasm.layout, native.layout)
    assertCollisionsFixture(dir, inWasm, 'in wasm')
  })

/** fixtures/rest-collisions.ly's row, as capture.mjs takes one. */
const restCollisions = {
  id: 'rest-collisions', file: 'tools/compare/fixtures/rest-collisions.ly',
  path: fileURLToPath(new URL('../fixtures/rest-collisions.ly', import.meta.url)),
}
/**
 * LilyPond's warnings on rest-collisions.ly: twice at the first column of two rests without a direction, then at the
 * first rest of one direction only and at the second of two rests moved up past a note.
 */
const REST_WARNINGS = [
  [35, 37, 'cannot resolve rest collision: rest direction not set'],
  [35, 37, 'cannot resolve rest collision: rest direction not set'],
  [36, 57, 'too many colliding rests'],
  [42, 8, 'too many colliding rests'],
].map(([line, column, text]) => `${restCollisions.path}:${line}:${column}: warning: ${text}\n`)
/**
 * rest-collisions.ly's outputs against LilyPond's capture in `dir`: its grobs.tsv LilyPond's line for line, its
 * layout.tsv on every type at LilyPond's offsets and Y extents, the system, page, X extent and stencil unreached; its
 * stderr LilyPond's.
 */
function assertRestCollisionsFixture(dir, { grobs, layout, stderr }, where) {
  const read = (name) => readFileSync(path.join(dir, name), 'utf8')
  assert.equal(stderr, read('stderr.txt'), where)
  assert.equal(stderr.split('\n').filter((line) => line.includes('warning:')).map((line) => `${line}\n`).join(''), REST_WARNINGS.join(''), where)
  assert.deepEqual(compareGrobs(read(GROBS), grobs), { equal: true, counts: [626, 626], missing: [], extra: [], order: [], differ: [] }, where)
  const reference = read(LAYOUT)
  const n = readLayout(reference).length
  const unreached = unreachedOf(reference)
  assert.deepEqual(compareLayout(reference, layout, all), {
    equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [], unreached,
  }, where)
  // LilyPond's three lines, 41 rests, 27 rest collisions and 2 flags.
  assert.deepEqual([...new Set(readLayout(reference).map((g) => g.system))], ['0', '1', '2'], where)
  const types = countTypes(readLayout(layout))
  assert.deepEqual([types.get('Rest'), types.get('RestCollision'), types.get('Flag')], [41, 27, 2], where)
}

test("fixtures/rest-collisions.ly natively and in wasm: the rest collisions' grobs.tsv, layout.tsv and stderr LilyPond's",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'rest-collisions')
    const result = await captureOne('lilypond', restCollisions, dir)
    assert.equal(result.exit, 0)
    const out = mkdtempSync(path.join(scratch, 'engine-rest-collisions-'))
    const run = runExample('compile', ['--trace', ...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, restCollisions.path],
      { cwd: path.dirname(restCollisions.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `rest-collisions.${name}`), 'utf8')
    const native = { grobs: read(GROBS), layout: read(LAYOUT), stderr: read('err') }
    assertRestCollisionsFixture(dir, native, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([restCollisions.path], { trace: true, modules: packModules() })
    const inWasm = { grobs: wasm.outputs[GROBS]?.toString(), layout: wasm.outputs[LAYOUT]?.toString(), stderr: wasm.outputs.stderr?.toString() }
    assert.equal(inWasm.layout, native.layout)
    assertRestCollisionsFixture(dir, inWasm, 'in wasm')
  })

/** fixtures/multi-measure-rests.ly's row, as capture.mjs takes one. */
const multiMeasureRests = {
  id: 'multi-measure-rests', file: 'tools/compare/fixtures/multi-measure-rests.ly',
  path: fileURLToPath(new URL('../fixtures/multi-measure-rests.ly', import.meta.url)),
}
/**
 * multi-measure-rests.ly's outputs against LilyPond's capture in `dir`: its grobs.tsv LilyPond's line for line, its
 * layout.tsv on every type at LilyPond's offsets and Y extents, the system, page, X extent and stencil unreached; its
 * stderr LilyPond's, none.
 */
function assertMultiMeasureRestsFixture(dir, { grobs, layout, stderr }, where) {
  const read = (name) => readFileSync(path.join(dir, name), 'utf8')
  assert.equal(stderr, read('stderr.txt'), where)
  assert.equal(stderr, '', where)
  assert.deepEqual(compareGrobs(read(GROBS), grobs), { equal: true, counts: [479, 479], missing: [], extra: [], order: [], differ: [] }, where)
  const reference = read(LAYOUT)
  const n = readLayout(reference).length
  const unreached = unreachedOf(reference)
  assert.deepEqual(compareLayout(reference, layout, all), {
    equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [], unreached,
  }, where)
  // LilyPond's four lines; 22 rests, 15 numbers of the 22 live, 2 texts and 5 scripts.
  assert.deepEqual([...new Set(readLayout(reference).map((g) => g.system))], ['0', '1', '2', '3'], where)
  const types = countTypes(readLayout(layout))
  assert.deepEqual(MULTI_MEASURE.map((type) => types.get(type)), [22, 15, 2, 5], where)
}

test("fixtures/multi-measure-rests.ly natively and in wasm: the multi-measure rests' grobs.tsv, layout.tsv and stderr LilyPond's",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'multi-measure-rests')
    const result = await captureOne('lilypond', multiMeasureRests, dir)
    assert.equal(result.exit, 0)
    const out = mkdtempSync(path.join(scratch, 'engine-multi-measure-rests-'))
    const run = runExample('compile', ['--trace', ...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, multiMeasureRests.path],
      { cwd: path.dirname(multiMeasureRests.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `multi-measure-rests.${name}`), 'utf8')
    const native = { grobs: read(GROBS), layout: read(LAYOUT), stderr: read('err') }
    assertMultiMeasureRestsFixture(dir, native, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([multiMeasureRests.path], { trace: true, modules: packModules() })
    const inWasm = { grobs: wasm.outputs[GROBS]?.toString(), layout: wasm.outputs[LAYOUT]?.toString(), stderr: wasm.outputs.stderr?.toString() }
    assert.equal(inWasm.layout, native.layout)
    assertMultiMeasureRestsFixture(dir, inWasm, 'in wasm')
  })

/** fixtures/clef-modifiers.ly's row, as capture.mjs takes one. */
const clefModifiers = {
  id: 'clef-modifiers', file: 'tools/compare/fixtures/clef-modifiers.ly',
  path: fileURLToPath(new URL('../fixtures/clef-modifiers.ly', import.meta.url)),
}
/**
 * clef-modifiers.ly's outputs against LilyPond's capture in `dir`: its grobs.tsv LilyPond's line for line, its
 * layout.tsv on every type at LilyPond's offsets and Y extents, the system, page, X extent and stencil unreached; its
 * stderr LilyPond's, none.
 */
function assertClefModifiersFixture(dir, { grobs, layout, stderr }, where) {
  const read = (name) => readFileSync(path.join(dir, name), 'utf8')
  assert.equal(stderr, read('stderr.txt'), where)
  assert.equal(stderr, '', where)
  assert.deepEqual(compareGrobs(read(GROBS), grobs), { equal: true, counts: [1161, 1161], missing: [], extra: [], order: [], differ: [] }, where)
  const reference = read(LAYOUT)
  const n = readLayout(reference).length
  const unreached = unreachedOf(reference)
  assert.deepEqual(compareLayout(reference, layout, all), {
    equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [], unreached,
  }, where)
  // LilyPond's six lines; 24 modifiers below and above, of 24 clefs.
  assert.deepEqual([...new Set(readLayout(reference).map((g) => g.system))], ['0', '1', '2', '3', '4', '5'], where)
  const types = countTypes(readLayout(layout))
  assert.deepEqual(['Clef', 'ClefModifier'].map((type) => types.get(type)), [24, 24], where)
}

test("fixtures/clef-modifiers.ly natively and in wasm: the clef modifiers' grobs.tsv, layout.tsv and stderr LilyPond's",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'clef-modifiers')
    const result = await captureOne('lilypond', clefModifiers, dir)
    assert.equal(result.exit, 0)
    const out = mkdtempSync(path.join(scratch, 'engine-clef-modifiers-'))
    const run = runExample('compile', ['--trace', ...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, clefModifiers.path],
      { cwd: path.dirname(clefModifiers.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `clef-modifiers.${name}`), 'utf8')
    const native = { grobs: read(GROBS), layout: read(LAYOUT), stderr: read('err') }
    assertClefModifiersFixture(dir, native, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([clefModifiers.path], { trace: true, modules: packModules() })
    const inWasm = { grobs: wasm.outputs[GROBS]?.toString(), layout: wasm.outputs[LAYOUT]?.toString(), stderr: wasm.outputs.stderr?.toString() }
    assert.equal(inWasm.layout, native.layout)
    assertClefModifiersFixture(dir, inWasm, 'in wasm')
  })

/** fixtures/span-bars.ly's row, as capture.mjs takes one. */
const spanBars = {
  id: 'span-bars', file: 'tools/compare/fixtures/span-bars.ly',
  path: fileURLToPath(new URL('../fixtures/span-bars.ly', import.meta.url)),
}
/**
 * span-bars.ly's outputs against LilyPond's capture in `dir`: its grobs.tsv LilyPond's line for line (no line of the
 * stubs killed as they are made), its layout.tsv on every type at LilyPond's offsets and Y extents, the system, page,
 * X extent and stencil unreached; its stderr LilyPond's, none.
 */
function assertSpanBarsFixture(dir, { grobs, layout, stderr }, where) {
  const read = (name) => readFileSync(path.join(dir, name), 'utf8')
  assert.equal(stderr, read('stderr.txt'), where)
  assert.equal(stderr, '', where)
  assert.deepEqual(compareGrobs(read(GROBS), grobs), { equal: true, counts: [1145, 1145], missing: [], extra: [], order: [], differ: [] }, where)
  const reference = read(LAYOUT)
  const n = readLayout(reference).length
  const unreached = unreachedOf(reference)
  assert.deepEqual(compareLayout(reference, layout, all), {
    equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [], unreached,
  }, where)
  // LilyPond's four lines; 21 span bars over 45 bar lines, 8 stubs.
  assert.deepEqual([...new Set(readLayout(reference).map((g) => g.system))], ['0', '1', '2', '3'], where)
  const types = countTypes(readLayout(layout))
  assert.deepEqual(SPAN_BARS.map((type) => types.get(type)), [45, 21, 8], where)
}

test("fixtures/span-bars.ly natively and in wasm: the span bars' and their stubs' grobs.tsv, layout.tsv and stderr LilyPond's",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'span-bars')
    const result = await captureOne('lilypond', spanBars, dir)
    assert.equal(result.exit, 0)
    const out = mkdtempSync(path.join(scratch, 'engine-span-bars-'))
    const run = runExample('compile', ['--trace', ...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, spanBars.path],
      { cwd: path.dirname(spanBars.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `span-bars.${name}`), 'utf8')
    const native = { grobs: read(GROBS), layout: read(LAYOUT), stderr: read('err') }
    assertSpanBarsFixture(dir, native, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([spanBars.path], { trace: true, modules: packModules() })
    const inWasm = { grobs: wasm.outputs[GROBS]?.toString(), layout: wasm.outputs[LAYOUT]?.toString(), stderr: wasm.outputs.stderr?.toString() }
    assert.equal(inWasm.layout, native.layout)
    assertSpanBarsFixture(dir, inWasm, 'in wasm')
  })

/** fixtures/instrument-names.ly's row, as capture.mjs takes one. */
const instrumentNames = {
  id: 'instrument-names', file: 'tools/compare/fixtures/instrument-names.ly',
  path: fileURLToPath(new URL('../fixtures/instrument-names.ly', import.meta.url)),
}
/**
 * instrument-names.ly's outputs against LilyPond's capture in `dir`: its grobs.tsv LilyPond's line for line, eleven
 * InstrumentNames made; its layout.tsv on every type at LilyPond's offsets and Y extents, the system, page, X extent and
 * stencil unreached, nine of the names live (the name set mid-line and the short name alone killed by their print); its
 * stderr LilyPond's, none.
 */
function assertInstrumentNamesFixture(dir, { grobs, layout, stderr }, where) {
  const read = (name) => readFileSync(path.join(dir, name), 'utf8')
  assert.equal(stderr, read('stderr.txt'), where)
  assert.equal(stderr, '', where)
  assert.deepEqual(compareGrobs(read(GROBS), grobs), { equal: true, counts: [497, 497], missing: [], extra: [], order: [], differ: [] }, where)
  const names = grobs.split('\n').filter((line) => line.startsWith('InstrumentName\t')).map((line) => line.split('\t').slice(3, 5).join(' '))
  assert.deepEqual(names, [
    '0 0;2 0 Staff#0', '0 0;3 0 Staff#1', '2 0;3 0 Staff#0', '0 0;2 0 Staff#2', '0 0;2 0 Staff#3', '0 0;2 0 Staff#4',
    '0 0;2 0 ChoirStaff#0', '0 0;2 0 Staff#5', '0 0;2 0 StaffGroup#0', '0 0;2 0 Staff#6', '0 0;2 0 Staff#7',
  ], where)
  const reference = read(LAYOUT)
  const n = readLayout(reference).length
  const unreached = unreachedOf(reference)
  assert.deepEqual(compareLayout(reference, layout, all), {
    equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [], unreached,
  }, where)
  // LilyPond's three lines; the nine live names' contexts and X offsets.
  assert.deepEqual([...new Set(readLayout(reference).map((g) => g.system))], ['0', '1', '2'], where)
  assert.deepEqual(readLayout(layout).filter((g) => g.type === 'InstrumentName').map((g) => `${g.context} ${g.x}`), [
    'Staff#0 0.8862', 'Staff#1 -2.7842', 'Staff#2 -4.2257', 'Staff#3 -2.7746', 'ChoirStaff#0 -1.4431', 'Staff#5 0.7933',
    'StaffGroup#0 -2.3308', 'Staff#6 -16.6172', 'Staff#7 -5.6522',
  ], where)
}

test("fixtures/instrument-names.ly natively and in wasm: the instrument names' grobs.tsv, layout.tsv and stderr LilyPond's",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, 'instrument-names')
    const result = await captureOne('lilypond', instrumentNames, dir)
    assert.equal(result.exit, 0)
    const out = mkdtempSync(path.join(scratch, 'engine-instrument-names-'))
    const run = runExample('compile', ['--trace', ...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, instrumentNames.path],
      { cwd: path.dirname(instrumentNames.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `instrument-names.${name}`), 'utf8')
    const native = { grobs: read(GROBS), layout: read(LAYOUT), stderr: read('err') }
    assertInstrumentNamesFixture(dir, native, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([instrumentNames.path], { trace: true, modules: packModules() })
    const inWasm = { grobs: wasm.outputs[GROBS]?.toString(), layout: wasm.outputs[LAYOUT]?.toString(), stderr: wasm.outputs.stderr?.toString() }
    assert.equal(inWasm.layout, native.layout)
    assertInstrumentNamesFixture(dir, inWasm, 'in wasm')
  })

/** fixtures/bar-number.ly's row, as capture.mjs takes one. */
const barNumber = { id: 'bar-number', file: 'tools/compare/fixtures/bar-number.ly', path: fileURLToPath(new URL('../fixtures/bar-number.ly', import.meta.url)) }
/** LilyPond's capture of fixtures/bar-number.ly, made once: its directory. */
let barNumberCapture
const captureBarNumber = () => barNumberCapture ??= (async () => {
  const dir = path.join(scratch, 'bar-number')
  const result = await captureOne('lilypond', barNumber, dir)
  assert.equal(result.exit, 0)
  assert.equal(result.pages, 1)
  return dir
})()
/** The engine's compile of fixtures/bar-number.ly natively with the build's packs, traced or not: its outputs by name. */
function compileBarNumber(trace) {
  const out = mkdtempSync(path.join(scratch, 'engine-bar-number-'))
  const run = runExample('compile', [...(trace ? ['--trace'] : []), ...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, barNumber.path],
    { cwd: path.dirname(barNumber.path), encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  return { out, outputs: Object.fromEntries(readdirSync(out).map((name) => [name, readFileSync(path.join(out, name), 'utf8')])) }
}
/** LilyPond's programming error at each read of a BarNumber's stencil whose text is '(): seven, three bar numbers' prints and their skylines'. */
const NON_MARKUP = 'programming error: Trying to interpret a non-markup object: ()\ncontinuing, cross fingers\n'.repeat(7)

test("a BarNumber whose text is '() prints LilyPond's programming error, natively and in wasm; its layout.tsv and page LilyPond's",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const dir = await captureBarNumber()
    assert.equal(readFileSync(path.join(dir, 'stderr.txt'), 'utf8'), NON_MARKUP)
    const { out, outputs } = compileBarNumber(false)
    assert.equal(outputs['bar-number.err'], NON_MARKUP)
    // Its page: C3 and C4 pass.
    const result = compare(dir, out)
    assert.deepEqual(result.pages, [1, 1])
    assert.deepEqual([result.pass.C3, result.pass.C4], [true, true])
    // Its layout.tsv: every grob of LilyPond's at its offsets and Y
    // extent, the three BarNumbers among them, the SystemStartBar killed.
    const reference = readFileSync(path.join(dir, LAYOUT), 'utf8')
    const traced = compileBarNumber(true).outputs
    assert.equal(traced['bar-number.err'], NON_MARKUP)
    const text = traced[`bar-number.${LAYOUT}`]
    const n = readLayout(reference).length
    const unreached = unreachedOf(reference)
    assert.deepEqual(compareLayout(reference, text, all), {
      equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [], unreached,
    })
    assert.deepEqual(readLayout(text).filter((g) => g.type === 'BarNumber').map((g) => g.x), ['8.5358', '23.2759', '31.4259'])
    assert.equal(countTypes(readLayout(text)).get('SystemStartBar'), undefined)
    if (coreSkip()) return
    const [wasm] = await compileInWasm([barNumber.path], { trace: true, modules: packModules() })
    assert.equal(wasm.outputs.stderr?.toString(), NON_MARKUP)
    assert.equal(wasm.outputs[LAYOUT]?.toString(), text)
  })

/** A columns.tsv by hand: path.ly's first column, its pieces and a musical column, the last unreached. */
const COLUMNS_CAPTURE = `NonMusicalPaperColumn@0 0@0\tStaffSpacing@0 0@Staff#0\tNonMusicalPaperColumn@3/4 0@-1 6.5347 2.6947 3.8400 3.8400|NonMusicalPaperColumn@3/4 0@0 6.3447 2.5047 3.8400 3.8400\tNonMusicalPaperColumn@3/4 0@-1 2.7947|NonMusicalPaperColumn@3/4 0@0 2.6047
NonMusicalPaperColumn@0 0@-1\tStaffSpacing@0 0@Staff#0\t-\t-
NonMusicalPaperColumn@0 0@1\tStaffSpacing@0 0@Staff#0\tNonMusicalPaperColumn@3/4 0@-1 7.5347 3.6947 3.8400 3.8400|NonMusicalPaperColumn@3/4 0@0 7.3447 3.5047 3.8400 3.8400\tNonMusicalPaperColumn@3/4 0@-1 3.7947|NonMusicalPaperColumn@3/4 0@0 3.6047
PaperColumn@0 0@0\tNoteSpacing@0 0@Voice#0|NoteSpacing@0 0@Voice#1\tPaperColumn@1/4 0@0 3.0022 1.5042 1.6980 1.6980\tPaperColumn@1/4 0@0 1.6042
`
const COLUMN_LINES = COLUMNS_CAPTURE.split('\n')
/** COLUMNS_CAPTURE with line `n` (from 1) given `values` by field. */
function perturbColumn(n, values, text = COLUMNS_CAPTURE) {
  const lines = text.split('\n')
  const cells = lines[n - 1].split('\t')
  for (const [field, value] of Object.entries(values)) cells[COLUMN_FIELDS.indexOf(field)] = value
  lines[n - 1] = cells.join('\t')
  return lines.join('\n')
}

test('layout.mjs: columns.tsv against itself passes; a perturbed wish, spring or rod differs in its field', () => {
  assert.deepEqual(compareColumns(COLUMNS_CAPTURE, COLUMNS_CAPTURE), { equal: true, counts: [4, 4], missing: [], extra: [], order: [], differ: [], unreached: {} })
  assert.equal(readColumns(COLUMNS_CAPTURE)[3].column, 'PaperColumn@0 0@0')
  assert.deepEqual(readEntries('springs', readColumns(COLUMNS_CAPTURE)[0].springs)[1], ['NonMusicalPaperColumn@3/4 0@0', '6.3447', '2.5047', '3.8400', '3.8400'])
  assert.deepEqual(readEntries('rods', '-'), [])
  assert.throws(() => readEntries('springs', 'PaperColumn@0 0@0 1.0000'), /no column/)
  assert.throws(() => readColumns('a\tb\tc'), /3 fields, not 4/)
  // Each field of line 4 perturbed: that field differs, the numbers past 1e-4.
  const fields = {
    'spacing-wishes': 'NoteSpacing@0 0@Voice#0',
    springs: 'PaperColumn@1/4 0@0 3.0024 1.5042 1.6980 1.6980',
    rods: 'PaperColumn@1/4 0@0 1.6040',
  }
  for (const [field, value] of Object.entries(fields)) {
    const result = compareColumns(COLUMNS_CAPTURE, perturbColumn(4, { [field]: value }))
    assert.deepEqual([result.equal, result.missing, result.extra, result.order], [false, [], [], []], field)
    assert.deepEqual(result.differ.map((d) => [d.line, d.field, d.value]), [[4, field, value]], field)
  }
  // Each of a spring's four numbers; within 1e-4; another column, or one entry more or less.
  for (const i of [0, 1, 2, 3]) {
    const numbers = ['3.0022', '1.5042', '1.6980', '1.6980']
    numbers[i] = (+numbers[i] + 2 * TOLERANCE).toFixed(4)
    assert.equal(compareColumns(COLUMNS_CAPTURE, perturbColumn(4, { springs: `PaperColumn@1/4 0@0 ${numbers.join(' ')}` })).differ.length, 1, `${i}`)
  }
  assert.equal(compareColumns(COLUMNS_CAPTURE, perturbColumn(4, { springs: 'PaperColumn@1/4 0@0 3.0023 1.5041 1.6980 1.6980', rods: 'PaperColumn@1/4 0@0 1.6043' })).equal, true)
  for (const rods of ['PaperColumn@1/2 0@0 1.6042', '-', 'PaperColumn@1/4 0@0 1.6042|PaperColumn@1/4 0@0 1.6042']) {
    assert.deepEqual(compareColumns(COLUMNS_CAPTURE, perturbColumn(4, { rods })).differ.map((d) => d.field), ['rods'], rods)
  }
  // A column missing, an extra one, one out of order, a renamed one both.
  assert.deepEqual(compareColumns(COLUMNS_CAPTURE, COLUMN_LINES.filter((_, i) => i !== 1).join('\n')).missing, [{ line: 2, column: 'NonMusicalPaperColumn@0 0@-1' }])
  assert.deepEqual(compareColumns(COLUMNS_CAPTURE, COLUMNS_CAPTURE + COLUMN_LINES[3] + '\n').extra, [{ line: 5, column: 'PaperColumn@0 0@0' }])
  const moved = compareColumns(COLUMNS_CAPTURE, [COLUMN_LINES[3], ...COLUMN_LINES.slice(0, 3), ''].join('\n'))
  assert.deepEqual([moved.missing, moved.extra, moved.differ, moved.order.length], [[], [], [], 1])
  const renamed = compareColumns(COLUMNS_CAPTURE, perturbColumn(4, { column: 'PaperColumn@1/4 0@0' }))
  assert.deepEqual([renamed.missing.length, renamed.extra.length], [1, 1])
  // Unreached springs and rods are counted and not compared; wishes still are.
  const unreached = perturbColumn(1, { springs: UNREACHED, rods: UNREACHED }, perturbColumn(4, { springs: UNREACHED, rods: UNREACHED }))
  assert.deepEqual(compareColumns(COLUMNS_CAPTURE, unreached), { equal: true, counts: [4, 4], missing: [], extra: [], order: [], differ: [], unreached: { springs: 2, rods: 2 } })
  assert.equal(compareColumns(COLUMNS_CAPTURE, perturbColumn(4, { 'spacing-wishes': '-' }, unreached)).differ[0].field, 'spacing-wishes')
})

/** path.ly's columns: twelve moments, each a non-musical column, its two pieces and a musical column. */
const PATH_MOMENTS = ['0', '3/4', '1', '3/2', '13/8', '2', '9/4', '11/4', '3', '25/8', '13/4', '15/4']

test("LilyPond's columns.tsv of path.ly: each column and its pieces after it, named by a column grobs.tsv heard; the compare on it", { skip: lilypond, timeout: 120_000 }, async () => {
  const dir = await captureFixture('path')
  const text = readFileSync(path.join(dir, COLUMNS), 'utf8')
  const columns = readColumns(text)
  assert.deepEqual(columns.map((c) => c.column), PATH_MOMENTS.flatMap((m) => [
    ...[0, -1, 1].map((d) => `NonMusicalPaperColumn@${m} 0@${d}`), `PaperColumn@${m} 0@0`,
  ]))
  const heard = new Set(readGrobs(readFileSync(path.join(dir, GROBS), 'utf8')).flat().map((g) => `${g.type}@${g.columns}`))
  for (const c of columns) assert.ok(heard.has(c.column.replace(/@-?\d$/, '')), c.column)
  // The springs and rods: to the next non-musical column and its piece ending a line, from each but the last and its pieces beginning a line.
  for (const c of columns) {
    const named = (field) => readEntries(field, c[field]).map((e) => e[0])
    const right = c.column.endsWith('@-1') || c.column.startsWith('PaperColumn') || c.column.includes('@15/4 ')
    if (!right && c['spacing-wishes'] !== '-') {
      assert.equal(named('springs').length, 2, c.column)
      assert.deepEqual(named('springs').map((n) => n.slice(n.lastIndexOf('@'))), ['@-1', '@0'], c.column)
      assert.deepEqual(named('rods'), named('springs'), c.column)
    } else assert.deepEqual([c.springs, c.rods], ['-', '-'], c.column)
  }
  assert.equal(compareColumns(text, text).equal, true)
  const n = columns.find((c) => c.springs !== '-').line
  const first = readEntries('springs', columns[n - 1].springs)[0]
  const shifted = `${first[0]} ${first.slice(1).map((v, i) => i ? v : (+v + 2 * TOLERANCE).toFixed(4)).join(' ')}`
  const lines = text.split('\n')
  lines[n - 1] = lines[n - 1].replace(first.join(' '), shifted)
  assert.deepEqual(compareColumns(text, lines.join('\n')).differ.map((d) => [d.line, d.field]), [[n, 'springs']])
})

/** The five's columns.tsv lines in ref/, as capture.mjs writes them since step 31. */
const COLUMN_CENSUS = { T01: 28, T03: 48, T14: 12, T16: 20, T19: 36 }
const skipColumns = Object.keys(CENSUS).every((id) => existsSync(path.join(references, id, COLUMNS)))
  ? false : `no columns.tsv in ${references}; run capture.mjs ${Object.keys(CENSUS).join(' ')}`

test("Y1's five's columns.tsv in ref/: a line each column and piece, and each against itself passes", { skip: skipColumns }, () => {
  const counts = {}
  for (const id of Object.keys(CENSUS)) {
    const text = readFileSync(path.join(references, id, COLUMNS), 'utf8')
    counts[id] = readColumns(text).length
    assert.equal(compareColumns(text, text).equal, true, id)
    assert.ok(readColumns(text).some((c) => c.springs !== '-'), id)
  }
  assert.deepEqual(counts, COLUMN_CENSUS)
})

/** The five's columns.tsv natively or in wasm: LilyPond's columns, pieces, wishes, springs and rods (since step 40). */
function assertFiveColumns(id, text, where) {
  const reference = readFileSync(path.join(references, id, COLUMNS), 'utf8')
  const n = readColumns(reference).length
  assert.deepEqual(compareColumns(reference, text), {
    equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [], unreached: {},
  }, `${id} ${where}`)
}

test("the engine's columns.tsv of Y1's five, natively: LilyPond's columns, pieces, spacing wishes, springs and rods",
  { skip: skipColumns || cargo || skipPacks, timeout: 300_000 }, () => {
    for (const [id, outputs] of Object.entries(compileFive())) assertFiveColumns(id, outputs[COLUMNS], 'natively')
  })

const pathLy = LAYOUT_FIXTURES.find((row) => row.id === 'path').path
/** The engine's columns.tsv of path.ly natively with the build's preloaded packs, made once. */
let compiledPath
const compilePath = () => compiledPath ??= (() => {
  const out = mkdtempSync(path.join(scratch, 'engine-path-'))
  const run = runExample('compile', ['--trace', ...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, pathLy],
    { cwd: path.dirname(pathLy), encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  return Object.fromEntries(['trace.log', LAYOUT, COLUMNS, LINE].map((name) => [name, readFileSync(path.join(out, `path.${name}`), 'utf8')]))
})()

/**
 * The engine's columns.tsv of path.ly against LilyPond's: every column and
 * prebroken piece, its wishes, springs and rods, LilyPond's (MS3 step 32,
 * D129): the unbroken first column's spring and rod without its LeftEdge,
 * killed before breaking (begin-of-line-visible), and the pieces' with
 * their own bar lines and time signatures.
 */
function assertPathColumns(reference, text, where) {
  assert.doesNotMatch(compilePath()['trace.log'], /columns\.tsv|prebroken/, where)
  assert.deepEqual(compareColumns(reference, text), {
    equal: true, counts: [48, 48], missing: [], extra: [], order: [], differ: [], unreached: {},
  }, where)
  const first = readColumns(text)[0]
  assert.equal(first.rods, 'NonMusicalPaperColumn@3/4 0@-1 2.7947|NonMusicalPaperColumn@3/4 0@0 2.6047', where)
}

test("the engine's columns.tsv of path.ly, natively with the build's packs: LilyPond's, each column and prebroken piece",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const reference = readFileSync(path.join(await captureFixture('path'), COLUMNS), 'utf8')
    assertPathColumns(reference, compilePath()[COLUMNS], 'natively')
  })

test("core.wasm's columns.tsv of path.ly with the preloaded packs is the native compile's line for line",
  { skip: lilypond || cargo || skipPacks || coreSkip(), timeout: 300_000 }, async () => {
    const reference = readFileSync(path.join(await captureFixture('path'), COLUMNS), 'utf8')
    const modules = packModules()
    const [wasm] = await compileInWasm([pathLy], { trace: true, modules })
    const text = wasm.outputs[COLUMNS]?.toString()
    assert.equal(text, compilePath()[COLUMNS])
    assertPathColumns(reference, text, 'in wasm')
  })

test("core.wasm's columns.tsv of Y1's five, in one instance, as natively: LilyPond's columns, pieces, springs and rods",
  { skip: skipColumns || cargo || skipPacks || coreSkip(), timeout: 300_000 }, async () => {
    const wasm = await compileInWasm(five.map((row) => row.path), { trace: true, modules: packModules() })
    for (const [i, row] of five.entries()) assertFiveColumns(row.id, wasm[i].outputs[COLUMNS]?.toString(), 'in wasm')
  })

/** A line.tsv by hand: path.ly's line, its two pieces at the ends. */
const LINE_CAPTURE = `0\tNonMusicalPaperColumn@0 0@1\t8.5358
0\tNonMusicalPaperColumn@3/4 0@0\t15.8806
0\tNonMusicalPaperColumn@15/4 0@-1\t37.3353
`
const LINE_LINES = LINE_CAPTURE.split('\n')

test('layout.mjs: line.tsv against itself passes; a perturbed X offset differs; a column missing, extra or out of order; an unreached one counted', () => {
  assert.deepEqual(compareLine(LINE_CAPTURE, LINE_CAPTURE), { equal: true, counts: [3, 3], missing: [], extra: [], order: [], differ: [], unreached: {} })
  assert.deepEqual(LINE_FIELDS, ['system', 'column', 'x'])
  assert.deepEqual(readLine(LINE_CAPTURE)[2], { line: 3, system: '0', column: 'NonMusicalPaperColumn@15/4 0@-1', x: '37.3353' })
  assert.throws(() => readLine('0\tNonMusicalPaperColumn@0 0@1'), /2 fields, not 3/)
  const x = (n, value) => LINE_LINES.map((l, i) => i === n - 1 ? l.replace(/[^\t]*$/, value) : l).join('\n')
  assert.deepEqual(compareLine(LINE_CAPTURE, x(2, '15.8808')).differ, [{ line: 2, got: 2, field: 'x', reference: '15.8806', value: '15.8808' }])
  assert.equal(compareLine(LINE_CAPTURE, x(2, '15.8807')).equal, true)
  assert.deepEqual(compareLine(LINE_CAPTURE, x(2, UNREACHED)), { equal: true, counts: [3, 3], missing: [], extra: [], order: [], differ: [], unreached: { x: 1 } })
  // Another break direction or system is another column.
  const renamed = compareLine(LINE_CAPTURE, LINE_CAPTURE.replace('15/4 0@-1', '15/4 0@0'))
  assert.deepEqual([renamed.missing.length, renamed.extra.length], [1, 1])
  assert.deepEqual(compareLine(LINE_CAPTURE, LINE_CAPTURE.replace(/^0\t/m, '1\t')).missing, [{ line: 1, column: 'NonMusicalPaperColumn@0 0@1' }])
  assert.deepEqual(compareLine(LINE_CAPTURE, LINE_LINES.filter((_, i) => i !== 1).join('\n')).missing, [{ line: 2, column: 'NonMusicalPaperColumn@3/4 0@0' }])
  const moved = compareLine(LINE_CAPTURE, [LINE_LINES[1], LINE_LINES[0], LINE_LINES[2], ''].join('\n'))
  assert.deepEqual([moved.missing, moved.extra, moved.differ, moved.order.length], [[], [], [], 1])
})

/** path.ly's line: its non-musical columns but the five no staff spacing reaches, the first and last their pieces. */
const PATH_LINE = ['0 0@1', '3/4 0@0', '3/2 0@0', '9/4 0@0', '11/4 0@0', '13/4 0@0', '15/4 0@-1'].map((c) => `NonMusicalPaperColumn@${c}`)

test("LilyPond's line.tsv of path.ly: its one line's columns, a piece at each end, at layout.tsv's X offsets", { skip: lilypond, timeout: 120_000 }, async () => {
  const dir = await captureFixture('path')
  const line = readLine(readFileSync(path.join(dir, LINE), 'utf8'))
  assert.deepEqual(line.map((c) => c.column), PATH_LINE)
  assert.ok(line.every((c) => c.system === '0'))
  const columns = readLayout(readFileSync(path.join(dir, LAYOUT), 'utf8')).filter((g) => g.type === 'NonMusicalPaperColumn')
  assert.deepEqual(line.map((c) => c.x), columns.map((g) => g.x))
  assert.equal(line[0].x, '8.5358')
})

/** The engine's line.tsv of path.ly against LilyPond's: every column at LilyPond's X offset. */
function assertPathLine(reference, text, where) {
  assert.doesNotMatch(compilePath()['trace.log'], /line\.tsv/, where)
  assert.deepEqual(compareLine(reference, text), {
    equal: true, counts: [7, 7], missing: [], extra: [], order: [], differ: [], unreached: {},
  }, where)
  assert.equal(text, reference, where)
}

test("the engine's line.tsv of path.ly, natively with the build's packs: LilyPond's columns at LilyPond's X offsets",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const reference = readFileSync(path.join(await captureFixture('path'), LINE), 'utf8')
    assertPathLine(reference, compilePath()[LINE], 'natively')
  })

test("core.wasm's line.tsv of path.ly with the preloaded packs is the native compile's line for line",
  { skip: lilypond || cargo || skipPacks || coreSkip(), timeout: 300_000 }, async () => {
    const reference = readFileSync(path.join(await captureFixture('path'), LINE), 'utf8')
    const modules = packModules()
    const [wasm] = await compileInWasm([pathLy], { trace: true, modules })
    const text = wasm.outputs[LINE]?.toString()
    assert.equal(text, compilePath()[LINE])
    assertPathLine(reference, text, 'in wasm')
  })

/**
 * The engine's layout.tsv of path.ly against LilyPond's (MS3 step 33): on
 * COMPARED (its paper columns' X offsets since step 37) and on every type
 * and field, each grob of its broken system in
 * LilyPond's order at LilyPond's X and Y offsets and Y extent; the system,
 * page, X extent and stencil unreached.
 */
function assertPathLayout(reference, outputs, where) {
  assert.doesNotMatch(outputs['trace.log'], /layout\.tsv/, where)
  const text = outputs[LAYOUT]
  const unreached = unreachedOf(reference)
  // COMPARED's: the vertical three, the seven non-musical columns, at
  // LilyPond's X offsets (no musical column is live, OPEN H8), and since
  // step 42 the six bar lines.
  assert.deepEqual(compareLayout(reference, text), {
    equal: true, counts: [16, 16], missing: [], extra: [], order: [], differ: [], unreached,
  }, where)
  assert.deepEqual(compareLayout(reference, text, all), {
    equal: true, counts: [44, 44], missing: [], extra: [], order: [], differ: [], unreached,
  }, where)
  const placed = (t) => grobLines(t).map((g) => [g.type, g.x, g.y, g['y-extent']].join(' '))
  assert.deepEqual(placed(text), placed(reference), where)
  assert.equal(placed(text)[1], 'VerticalAxisGroup 8.5358 -2.0500 -2.0500 2.0500', where)
}

test("the engine's layout.tsv of path.ly, natively with the build's packs: LilyPond's grobs in its order at its X and Y offsets",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const reference = readFileSync(path.join(await captureFixture('path'), LAYOUT), 'utf8')
    assertPathLayout(reference, compilePath(), 'natively')
  })

test("core.wasm's layout.tsv of path.ly with the preloaded packs is the native compile's line for line",
  { skip: lilypond || cargo || skipPacks || coreSkip(), timeout: 300_000 }, async () => {
    const reference = readFileSync(path.join(await captureFixture('path'), LAYOUT), 'utf8')
    const modules = packModules()
    const [wasm] = await compileInWasm([pathLy], { trace: true, modules })
    const outputs = Object.fromEntries(['trace.log', LAYOUT].map((name) => [name, wasm.outputs[name]?.toString()]))
    assert.equal(outputs[LAYOUT], compilePath()[LAYOUT])
    assertPathLayout(reference, outputs, 'in wasm')
  })

/** The five's line.tsv natively or in wasm: a line from the first piece beginning one, every X offset reached (since step 40). */
function assertFiveLine(id, text, where) {
  const line = readLine(text)
  assert.ok(line.length > 1, `${id} ${where}`)
  assert.ok(line[0].column.endsWith('@1') && line.at(-1).column.endsWith('@-1'), `${id} ${where}`)
  // Its non-musical columns' X offsets, as layout.tsv places them,
  // LilyPond's.
  assert.ok(line.every((c) => c.system === '0' && c.x !== UNREACHED), `${id} ${where}`)
}

test("the engine's line.tsv of Y1's five, natively: every X offset reached",
  { skip: skipFive || cargo || skipPacks, timeout: 300_000 }, () => {
    for (const [id, outputs] of Object.entries(compileFive())) assertFiveLine(id, outputs[LINE], 'natively')
  })

test("core.wasm's line.tsv of Y1's five, in one instance, is the native compile's line for line",
  { skip: skipFive || cargo || skipPacks || coreSkip(), timeout: 300_000 }, async () => {
    const wasm = await compileInWasm(five.map((row) => row.path), { trace: true, modules: packModules() })
    for (const [i, row] of five.entries()) {
      const text = wasm[i].outputs[LINE]?.toString()
      assert.equal(text, compileFive()[row.id][LINE], row.id)
      assertFiveLine(row.id, text, 'in wasm')
    }
  })

/** Part 4's fixtures (MS3 step 47) and, by type, the live grobs of Y2's that none of the 10 reaches in LilyPond's capture of each. */
const Y2_FIXTURES = {
  scripts: { Script: 36, ScriptColumn: 3 },
  fingerings: { Fingering: 26, StringNumber: 7, StrokeFinger: 6, FingeringColumn: 5 },
  arpeggios: { Arpeggio: 16 },
  'line-spanners': { TextSpanner: 3, Glissando: 4, DynamicTextSpanner: 2, VoiceFollower: 2 },
  brackets: { OttavaBracket: 3, HorizontalBracket: 4, HorizontalBracketText: 1 },
  metronome: { MetronomeMark: 5 },
  'bar-numbers-staves': { BarNumber: 20 },
}

test("LilyPond captures part 4's fixtures: each line pairs with grobs.tsv's; each score one line of one page; Y2's unreached types live; the compare on them; ref/'s copy byte for byte",
  { skip: lilypond, timeout: 300_000 }, async () => {
    for (const [id, types] of Object.entries(Y2_FIXTURES)) {
      const dir = await captureFixture(id)
      const read = (name, at = dir) => readFileSync(path.join(at, name), 'utf8')
      const text = read(LAYOUT)
      const grobs = readLayout(text)
      const announced = new Set(readGrobs(read(GROBS)).flat().map((g) => FIELDS.slice(0, 6).map((f) => g[f]).join('\t')))
      for (const g of grobs) assert.ok(announced.has(FIELDS.slice(0, 6).map((f) => g[f]).join('\t')), `${id}:${g.line}`)
      const scores = read(LAYOUT_FIXTURES.find((row) => row.id === id).path, '/').match(/\\score\b/g).length
      assert.equal(new Set(grobs.map((g) => g.system)).size, scores, id)
      assert.ok(grobs.every((g) => g.page === '0'), id)
      assert.deepEqual(readdirSync(dir).filter((f) => f.endsWith('.svg')), [`${id}.svg`], id)
      const counts = countTypes(grobs)
      assert.deepEqual(Object.fromEntries(Object.keys(types).map((t) => [t, counts.get(t)])), types, id)
      assert.equal(compareLayout(text, text, all).equal, true, id)
      assertPerturbedFail(text, Object.keys(types)[0])
      // ref/'s capture of it (capture.mjs, MS3 step 47), where there is one: the same (D109).
      const reference = path.join(references, id)
      if (existsSync(reference)) {
        for (const name of [GROBS, LAYOUT, COLUMNS, LINE, 'stderr.txt', `${id}.svg`]) assert.equal(read(name), read(name, reference), `${id} ${name}`)
      }
    }
    const engravers = (id, type) => countTypes(readLayout(readFileSync(path.join(scratch, id, LAYOUT), 'utf8')).filter((g) => g.type === type)
      .map((g) => ({ type: `${g.context.replace(/#.*/, '')} ${g.engraver}` })))
    // The span arpeggios across the group's two staves; the Score's bar
    // numbers over a group (A21) and each of three staves' own.
    assert.equal(engravers('arpeggios', 'Arpeggio').get('StaffGroup Span_arpeggio_engraver'), 2)
    assert.deepEqual(Object.fromEntries(engravers('bar-numbers-staves', 'BarNumber')), {
      'Score Bar_number_engraver': 5, 'Staff Bar_number_engraver': 15,
    })
  })

/**
 * The engine's traced outputs of part 4's fixture `id` against LilyPond's capture in `dir`: its grobs.tsv LilyPond's
 * line for line, its layout.tsv on every type at LilyPond's offsets and Y extents (the system, page, X extent and
 * stencil unreached), its stderr LilyPond's.
 */
function assertY2Fixture(id, dir, { grobs, layout, stderr }, where, known = {}) {
  const read = (name) => readFileSync(path.join(dir, name), 'utf8')
  assert.equal(stderr, (known.stderr ?? '') + read('stderr.txt'), `${id} ${where}`)
  const heard = compareGrobs(read(GROBS), grobs)
  const n = heard.counts[0]
  assert.deepEqual(heard, { equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [] }, `${id} ${where}`)
  const reference = read(LAYOUT)
  const lines = readLayout(reference).length
  const unreached = unreachedOf(reference)
  const result = compareLayout(reference, layout, all)
  // The differences recorded for it, each its line, field and how far.
  const expected = known.differ ?? []
  assert.deepEqual(result.differ.map((d) => [d.line, d.field]), expected.map((d) => [d.line, d.field]), `${id} ${where}`)
  result.differ.forEach((d, i) => assert.ok(Math.abs(Number(d.value) - Number(d.reference)) <= expected[i].within, `${id} ${where}`))
  assert.deepEqual({ ...result, equal: true, differ: [] }, {
    equal: true, counts: [lines, lines], missing: [], extra: [], order: [], differ: [], unreached,
  }, `${id} ${where}`)
}

test("fixtures/bar-numbers-staves.ly natively and in wasm: the Score's BarNumbers over a group, supported by stavesFound's staff symbols, and three staves' own at LilyPond's offsets (OPEN A21)",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const id = 'bar-numbers-staves'
    const dir = await captureFixture(id)
    const row = LAYOUT_FIXTURES.find((r) => r.id === id)
    const out = mkdtempSync(path.join(scratch, `engine-${id}-`))
    const run = runExample('compile', ['--trace', ...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, row.path],
      { cwd: path.dirname(row.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `${id}.${name}`), 'utf8')
    const native = { grobs: read(GROBS), layout: read(LAYOUT), stderr: read('err') }
    assertY2Fixture(id, dir, native, 'natively')
    // Each BarNumber over the staves: the Score's 5 at the group's top
    // staff, the three staves' 15 each above its own.
    assert.equal(readLayout(native.layout).filter((g) => g.type === 'BarNumber').length, 20)
    if (coreSkip()) return
    const [wasm] = await compileInWasm([row.path], { trace: true, modules: packModules() })
    const inWasm = { grobs: wasm.outputs[GROBS]?.toString(), layout: wasm.outputs[LAYOUT]?.toString(), stderr: wasm.outputs.stderr?.toString() }
    assert.equal(inWasm.layout, native.layout)
    assertY2Fixture(id, dir, inWasm, 'in wasm')
  })

/** fixtures/magnify-staff.ly's row, as capture.mjs takes one. */
const magnifyStaff = {
  id: 'magnify-staff', file: 'tools/compare/fixtures/magnify-staff.ly',
  path: fileURLToPath(new URL('../fixtures/magnify-staff.ly', import.meta.url)),
}
/** The build's Emmentaler sizes beside the core's, from its fonts/: the fixture's 13, 14 and 16 among them (W5's packs on demand, step 50). */
const emmentalerSizes = () => readdirSync(path.join(path.dirname(packs[0][1]), 'fonts')).filter((f) => /^emmentaler-\d+\.pack$/.test(f))
  .map((f) => [f.replace(/\.pack$/, ''), path.join(path.dirname(packs[0][1]), 'fonts', f)])

test("fixtures/magnify-staff.ly natively and in wasm: fontSize, \\magnifyStaff, \\small and \\teeny in each grob's font-size, a magnified staff at LilyPond's Y offsets (OPEN A34)",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const dir = path.join(scratch, magnifyStaff.id)
    const result = await captureOne('lilypond', magnifyStaff, dir)
    assert.equal(result.exit, 0)
    assert.equal(readFileSync(path.join(dir, 'stderr.txt'), 'utf8'), '')
    const given = [...packs, ...emmentalerSizes()]
    const out = mkdtempSync(path.join(scratch, 'engine-magnify-staff-'))
    const run = runExample('compile', ['--trace', ...given.flatMap(([, file]) => ['--pack', file]), '--out', out, magnifyStaff.path],
      { cwd: path.dirname(magnifyStaff.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name) => readFileSync(path.join(out, `magnify-staff.${name}`), 'utf8')
    const native = { grobs: read(GROBS), layout: read(LAYOUT), stderr: read('err') }
    assertY2Fixture(magnifyStaff.id, dir, native, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([magnifyStaff.path], { trace: true, modules: given.map(([name, file]) => [name, readFileSync(file)]) })
    const inWasm = { grobs: wasm.outputs[GROBS]?.toString(), layout: wasm.outputs[LAYOUT]?.toString(), stderr: wasm.outputs.stderr?.toString() }
    assert.equal(inWasm.layout, native.layout)
    assertY2Fixture(magnifyStaff.id, dir, inWasm, 'in wasm')
  })

test("capture.mjs keeps the trace.tsv trace.mjs wrote into a capture's directory, and nothing else of it (OPEN H5)", { skip: lilypond, timeout: 120_000 }, async () => {
  const dir = path.join(scratch, 'kept')
  mkdirSync(dir, { recursive: true })
  writeFileSync(path.join(dir, 'trace.tsv'), 'kept\n')
  writeFileSync(path.join(dir, 'stale.txt'), 'gone\n')
  const result = await captureOne('lilypond', LAYOUT_FIXTURES.find((row) => row.id === 'path'), dir)
  assert.equal(result.exit, 0)
  assert.deepEqual(KEPT, ['trace.tsv'])
  assert.equal(readFileSync(path.join(dir, 'trace.tsv'), 'utf8'), 'kept\n')
  assert.equal(existsSync(path.join(dir, 'stale.txt')), false)
  assert.ok(existsSync(path.join(dir, LAYOUT)))
})

/** 05 §2.1's census of Y2's three beyond the five (T10, T13, T18): their live grobs after page breaking. */
const Y2_CENSUS = { T10: 121, T13: 45, T18: 143 }
const skipY2 = Object.keys(Y2_CENSUS).every((id) => existsSync(path.join(references, id, LAYOUT)))
  ? false : `no layout.tsv in ${references}; run fetch.mjs and capture.mjs`

test("Y2's three's layout.tsv in ref/: 05 §2.1's census, 309 live grobs, and each against itself passes", { skip: skipY2 }, () => {
  const counts = {}
  for (const id of Object.keys(Y2_CENSUS)) {
    const text = readFileSync(path.join(references, id, LAYOUT), 'utf8')
    counts[id] = grobLines(text).length
    assert.equal(compareLayout(text, text, all).equal, true, id)
    assertPerturbedFail(text, 'Stem')
  }
  assert.deepEqual(counts, Y2_CENSUS)
  assert.equal(Object.values(counts).reduce((a, b) => a + b), 309)
})

const y2 = corpus().filter((row) => row.id in Y2_CENSUS)
/** The engine's error for a font whose pack the host could not fetch (W5, D208). */
const unfetched = (font, pack) => `error: cannot find font '${font}' (its font pack ${pack} could not be fetched)\n`
/** What Y2's three say offline, with the build's preloaded packs alone: T10 lacks its volta numbers' Emmentaler-13, T13 its brace's packs. */
const Y2_OFFLINE = { T10: unfetched('emmentaler-13', 'emmentaler-13'), T13: unfetched('emmentaler-brace', 'emmentaler-brace-0'), T18: '' }
/**
 * The engine's traced compile of Y2's three natively, made once: with the
 * build's preloaded packs, the others fetched as the engine asks for them
 * (W5, step 50: T10's volta numbers' Emmentaler-13, T13's brace), each id
 * to its outputs' text by name; and offline, `<id>+offline`, where none is
 * fetched.
 */
let compiledY2
const compileY2 = () => compiledY2 ??= Object.fromEntries([...y2.map((row) => [row.id, row, []]),
  ...y2.filter((row) => row.id !== 'T18').map((row) => [`${row.id}+offline`, row, ['--offline']])].map(([id, row, more]) => {
  const out = mkdtempSync(path.join(scratch, `engine-${id}-`))
  const run = runExample('compile', ['--trace', ...packs.flatMap(([, file]) => ['--pack', file]), ...more, '--out', out, row.path],
    { cwd: path.dirname(row.path), encoding: 'utf8' })
  assert.equal(run.status, 0, `${id}: ${run.stderr}`)
  const read = (name) => readFileSync(path.join(out, `${path.basename(row.path, '.ly')}.${name}`), 'utf8')
  return [id, { ...Object.fromEntries(['trace.log', GROBS, LAYOUT, 'err'].map((name) => [name, read(name)])), page: existsSync(path.join(out, `${path.basename(row.path, '.ly')}.svg`)) }]
}))

/** The differences of `result` (compareLayout's) by type and field: { type: { field: n } }. */
function differByType(reference, result) {
  const types = new Map(readLayout(reference).map((g) => [g.line, g.type]))
  const by = {}
  for (const d of result.differ) {
    const t = (by[types.get(d.line)] ??= {})
    t[d.field] = (t[d.field] ?? 0) + 1
  }
  return by
}

/**
 * The engine's layout.tsv of Y2's `id` against ref/ on every type and field
 * (MS3 steps 47 and 49; progress.md's entries have the probe of each first
 * error). With their packs fetched (W5, step 50), all three are
 * LilyPond's on every grob, as the five are, since step 51: T18's `ff`s
 * kerned (E6, W6), which moved their X offsets, the first hairpin's, the
 * dynamics' line spanners and the staff's extent below; T10's staff no
 * longer 0.9908 lower, its first volta clearing its TextScript's outlines
 * (E7), where the text had no skyline but its spaces'.
 * Offline, where each stops: T10's breaking at its volta numbers'
 * Emmentaler-13, once its System's grobs are in its pieces, which are
 * read, their offsets unreached; T13's at the brace's font, its unbroken
 * System's grobs written, every one of LilyPond's among them, their offsets
 * unreached.
 */
function assertY2Layout(id, outputs, where) {
  assert.doesNotMatch(outputs['trace.log'], /layout\.tsv/, `${id} ${where}`)
  const reference = readFileSync(path.join(references, id.replace(/\+.*/, ''), LAYOUT), 'utf8')
  const n = readLayout(reference).length
  const result = compareLayout(reference, outputs[LAYOUT], all)
  const whole = unreachedOf(reference)
  if (id === 'T13' || id === 'T18' || id === 'T10') {
    assert.deepEqual(result, { equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [], unreached: whole }, `${id} ${where}`)
    return
  }
  const lines = readLayout(outputs[LAYOUT]).length
  assert.deepEqual([result.counts, result.missing, result.unreached.x, result.unreached.y], [[n, lines], [], lines, lines], `${id} ${where}`)
  assert.deepEqual({ 'T10+offline': lines - n, 'T13+offline': lines - n }[id], { 'T10+offline': 1, 'T13+offline': 46 }[id], `${id} ${where}: the unbroken System's own`)
}

test("the engine's layout.tsv of T10, T13 and T18, from a native traced compile, their packs fetched: read back against ref/; offline, where each stops, with W5's diagnostic",
  { skip: skipY2 || cargo || skipPacks, timeout: 300_000 }, () => {
    for (const [id, outputs] of Object.entries(compileY2())) {
      assertY2Layout(id, outputs, 'natively')
      assert.equal(outputs.page, !id.endsWith('+offline'), id)
      assert.equal(outputs.err, id.endsWith('+offline') ? Y2_OFFLINE[id.replace(/\+.*/, '')] : '', id)
    }
  })

test("core.wasm's layout.tsv of T10, T13 and T18, from a traced compile in one instance, their packs fetched and offline, is the native compile's line for line",
  { skip: skipY2 || cargo || skipPacks || coreSkip(), timeout: 300_000 }, async () => {
    const native = compileY2()
    const online = y2.map((row) => row.path)
    const offline = y2.filter((row) => row.id !== 'T18')
    const wasm = await compileInWasm(online, { trace: true, modules: packModules() })
    const unfetched = await compileInWasm(offline.map((row) => row.path), { trace: true, modules: packModules(), offline: true })
    for (const [id, result] of [...y2.map((row, i) => [row.id, wasm[i]]), ...offline.map((row, i) => [`${row.id}+offline`, unfetched[i]])]) {
      const outputs = Object.fromEntries(['trace.log', LAYOUT].map((name) => [name, result.outputs[name]?.toString()]))
      assert.equal(outputs[LAYOUT], native[id][LAYOUT], id)
      assert.equal(result.outputs.stderr.toString(), native[id].err, id)
      assertY2Layout(id, outputs, 'in wasm')
    }
  })

/** Every pack of the build's fonts/, by name: the text faces the ottava's and the fixtures' texts ask for among them (W5, step 50). */
const everyPack = () => readdirSync(path.join(path.dirname(packs[0][1]), 'fonts')).filter((f) => f.endsWith('.pack'))
  .map((f) => [f.replace(/\.pack$/, ''), path.join(path.dirname(packs[0][1]), 'fonts', f)])

/** bracket-prints.ly's `PRINT type expr` lines of `text`, sorted, the text faces' family and data directory named alike. */
const prints = (text) => text.split('\n').filter((l) => l.startsWith('PRINT ')).map((l) => l
  .replace(/"(LilyPond Serif|serif)([^"]*)"/g, '"FAMILY$2"').replace(/"[^"]*\/fonts\/otf\//g, '"DATADIR/')).sort()

const bracketPrints = fileURLToPath(new URL('../bracket-prints.ly', import.meta.url))

/**
 * The engine's prints of bracket-prints.ly against LilyPond's: each the
 * same stencil, but the numbered volta brackets', whose numbers' glyph
 * strings are LilyPond's since step 51 (W6: `fattened.one`, where the
 * engine had `one`) and whose brackets are LilyPond's, but which LilyPond's
 * PostScript backend wraps in a `utf-8-string` and the engine's SVG one
 * draws as paths (`music-strings-to-paths`).
 */
function assertPrints(lilypond, engine, where) {
  const [a, b] = [prints(lilypond), prints(engine)]
  const count = (l) => Object.fromEntries([...new Set(l.map((x) => x.split(' ')[1]))].map((t) => [t, l.filter((x) => x.split(' ')[1] === t).length]))
  assert.deepEqual(count(a), { Hairpin: 13, HorizontalBracket: 4, OttavaBracket: 3, VoltaBracket: 4 }, where)
  assert.deepEqual(count(b), count(a), where)
  const numbered = (l) => l.includes('Emmentaler-13')
  assert.deepEqual(b.filter((l) => !numbered(l)), a.filter((l) => !numbered(l)), where)
  const bracket = (l) => l.slice(l.indexOf(' (draw-line'))
  const glyphs = (l) => l.slice(l.indexOf('(glyph-string'), l.indexOf(' "DATADIR/'))
  for (const part of [bracket, glyphs]) assert.deepEqual(b.filter(numbered).map(part), a.filter(numbered).map(part), where)
  assert.equal(a.filter(numbered).length, 2, where)
  assert.ok(b.filter(numbered).every((l) => l.includes('"fattened.')), where)
}

test('the prints of hairpins, volta, ottava and analysis brackets are LilyPond\'s stencils (bracket-prints.ly), natively and in wasm',
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const lp = spawnSync('lilypond', ['-s', '-dno-print-pages', path.basename(bracketPrints)], { cwd: path.dirname(bracketPrints), encoding: 'utf8' })
    assert.equal(lp.status, 0, lp.stderr)
    assert.equal(lp.stderr.split('\n').filter((l) => l && !l.startsWith('PRINT ')).join('\n'), '')
    const given = everyPack()
    const out = mkdtempSync(path.join(scratch, 'engine-bracket-prints-'))
    const run = runExample('compile', [...given.flatMap(([, file]) => ['--pack', file]), '--out', out, bracketPrints],
      { cwd: path.dirname(bracketPrints), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const native = readFileSync(path.join(out, 'bracket-prints.err'), 'utf8')
    assert.equal(native.split('\n').filter((l) => l && !l.startsWith('PRINT ')).join('\n'), '')
    assert.ok(existsSync(path.join(out, 'bracket-prints.svg')))
    assertPrints(lp.stderr, native, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([bracketPrints], { modules: given.map(([name, file]) => [name, readFileSync(file)]) })
    assert.equal(wasm.outputs.stderr?.toString(), native)
  })

test("fixtures/brackets.ly natively and in wasm, with the build's packs: LilyPond's grobs, OttavaBrackets and HorizontalBrackets at LilyPond's offsets and Y extents",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const id = 'brackets'
    const dir = await captureFixture(id)
    const row = LAYOUT_FIXTURES.find((r) => r.id === id)
    const given = everyPack()
    const out = mkdtempSync(path.join(scratch, `engine-${id}-`))
    const run = runExample('compile', ['--trace', ...given.flatMap(([, file]) => ['--pack', file]), '--out', out, row.path],
      { cwd: path.dirname(row.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name, at = out) => readFileSync(path.join(at, at === out ? `${id}.${name}` : name), 'utf8')
    assert.ok(existsSync(path.join(out, `${id}.svg`)))
    const check = (outputs, where) => {
      assert.equal(outputs.stderr, read('stderr.txt', dir), where)
      const heard = compareGrobs(read(GROBS, dir), outputs.grobs)
      assert.equal(heard.equal, true, where)
      const reference = read(LAYOUT, dir)
      const n = readLayout(reference).length
      const result = compareLayout(reference, outputs.layout, all)
      assert.deepEqual([result.counts, result.missing, result.extra, result.order, result.unreached],
        [[n, n], [], [], [], unreachedOf(reference)], where)
      // Every field LilyPond's: until the text packs' outlines (E7, step
      // 51) the 8 and 15 of the ottavas below and above had no skyline,
      // and the two lay nearer the staff.
      assert.deepEqual(differByType(reference, result), {}, where)
    }
    const native = { grobs: read(GROBS), layout: read(LAYOUT), stderr: read('err') }
    check(native, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([row.path], { trace: true, modules: given.map(([name, file]) => [name, readFileSync(file)]) })
    const inWasm = { grobs: wasm.outputs[GROBS]?.toString(), layout: wasm.outputs[LAYOUT]?.toString(), stderr: wasm.outputs.stderr?.toString() }
    assert.equal(inWasm.layout, native.layout)
    check(inWasm, 'in wasm')
  })

const lineSpannerPrints = fileURLToPath(new URL('../line-spanner-prints.ly', import.meta.url))

test("the prints of text spanners, glissandi, dynamic text spanners and voice followers, and ly:line-interface::line on note heads, are LilyPond's stencils (line-spanner-prints.ly), natively and in wasm",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const lp = spawnSync('lilypond', ['-s', '-dno-print-pages', path.basename(lineSpannerPrints)],
      { cwd: path.dirname(lineSpannerPrints), encoding: 'utf8' })
    assert.equal(lp.status, 0, lp.stderr)
    assert.equal(lp.stderr.split('\n').filter((l) => l && !l.startsWith('PRINT ')).join('\n'), '')
    const given = everyPack()
    const out = mkdtempSync(path.join(scratch, 'engine-line-spanner-prints-'))
    const run = runExample('compile', [...given.flatMap(([, file]) => ['--pack', file]), '--out', out, lineSpannerPrints],
      { cwd: path.dirname(lineSpannerPrints), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const native = readFileSync(path.join(out, 'line-spanner-prints.err'), 'utf8')
    assert.equal(native.split('\n').filter((l) => l && !l.startsWith('PRINT ')).join('\n'), '')
    assert.ok(existsSync(path.join(out, 'line-spanner-prints.svg')))
    const [a, b] = [prints(lp.stderr), prints(native)]
    const count = (l) => Object.fromEntries([...new Set(l.map((x) => x.split(' ')[1]))].map((t) => [t, l.filter((x) => x.split(' ')[1] === t).length]))
    assert.deepEqual(count(a), { DynamicTextSpanner: 3, Glissando: 9, LINE: 12, TextSpanner: 5, VoiceFollower: 4 })
    assert.deepEqual(b, a)
    if (coreSkip()) return
    const [wasm] = await compileInWasm([lineSpannerPrints], { modules: given.map(([name, file]) => [name, readFileSync(file)]) })
    assert.equal(wasm.outputs.stderr?.toString(), native)
  })

const slurPrints = fileURLToPath(new URL('../slur-prints.ly', import.meta.url))

/**
 * The key and control points of each `PRINT type key control-points expr` line of `text` (slur-prints.ly's), as an
 * alist's entries in Scheme: each a balanced list after the type.
 */
function slurControlPoints(text) {
  const list = (line, from) => {
    let depth = 0
    for (let i = line.indexOf('(', from); i < line.length; i++) {
      if (line[i] === '(') depth++
      else if (line[i] === ')' && --depth === 0) return [line.slice(line.indexOf('(', from), i + 1), i + 1]
    }
    return ['()', line.length]
  }
  return text.split('\n').filter((l) => l.startsWith('PRINT ')).map((line) => {
    const [key, end] = list(line, 0)
    const [points] = list(line, end)
    return `(${key} . ${points})`
  })
}

test("the prints of slurs and phrasing slurs from LilyPond's control points are LilyPond's stencils (slur-prints.ly over fixtures/slurs.ly), natively and in wasm",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const lp = spawnSync('lilypond', ['-s', '-dno-print-pages', path.basename(slurPrints)],
      { cwd: path.dirname(slurPrints), encoding: 'utf8' })
    assert.equal(lp.status, 0, lp.stderr)
    assert.equal(lp.stderr.split('\n').filter((l) => l && !l.startsWith('PRINT ')).join('\n'), '')
    // The drawing apart from the slur scoring (MS4 step 8): LilyPond's control points, by key.
    const dir = mkdtempSync(path.join(scratch, 'slur-prints-'))
    const given = path.join(dir, 'slur-prints-given.ly')
    writeFileSync(given, `\\version "2.26.0"\n#(define slur-control-points (quote (${slurControlPoints(lp.stderr).join('\n')})))\n` +
      `\\include "${slurPrints}"\n`)
    const packs = everyPack()
    const out = mkdtempSync(path.join(scratch, 'engine-slur-prints-'))
    const run = runExample('compile', [...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, given],
      { cwd: path.dirname(slurPrints), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const native = readFileSync(path.join(out, 'slur-prints-given.err'), 'utf8')
    // fixtures/slurs.ly's six warnings, each with its source line and caret, which -s keeps from LilyPond's.
    assert.equal(native.split('\n').filter((l) => l && !l.startsWith('PRINT ')).length, 18)
    assert.ok(existsSync(path.join(out, 'slur-prints-given.svg')))
    const [a, b] = [prints(lp.stderr), prints(native)]
    const count = (l) => Object.fromEntries([...new Set(l.map((x) => x.split(' ')[1]))].map((t) => [t, l.filter((x) => x.split(' ')[1] === t).length]))
    // A broken slur's two pieces: the 26 slurs and 3 phrasing slurs that live, 30 stencils.
    assert.deepEqual(count(a), { PhrasingSlur: 3, Slur: 27 })
    assert.deepEqual(b, a)
    if (coreSkip()) return
    const [wasm] = await compileInWasm([given], { modules: packs.map(([name, file]) => [name, readFileSync(file)]) })
    assert.equal(wasm.outputs.stderr?.toString(), native)
  })

test("fixtures/line-spanners.ly natively and in wasm, with the build's packs: LilyPond's grobs, TextSpanners, Glissandi, DynamicTextSpanners and VoiceFollowers at LilyPond's offsets and Y extents; its page passes C3 and C4",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async (t) => {
    const id = 'line-spanners'
    const dir = await captureFixture(id)
    const row = LAYOUT_FIXTURES.find((r) => r.id === id)
    const given = everyPack()
    const out = mkdtempSync(path.join(scratch, `engine-${id}-`))
    const run = runExample('compile', ['--trace', ...given.flatMap(([, file]) => ['--pack', file]), '--out', out, row.path],
      { cwd: path.dirname(row.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (name, at = out) => readFileSync(path.join(at, at === out ? `${id}.${name}` : name), 'utf8')
    const check = (outputs, where) => {
      assertY2Fixture(id, dir, outputs, where)
      const reference = read(LAYOUT, dir)
      const result = compareLayout(reference, outputs.layout, all)
      assert.deepEqual(differByType(reference, result), {}, where)
      const counts = countTypes(readLayout(outputs.layout))
      assert.deepEqual(Object.fromEntries(Object.keys(Y2_FIXTURES[id]).map((type) => [type, counts.get(type)])), Y2_FIXTURES[id], where)
    }
    /** The page of `outputs` scored against LilyPond's: one page, every link paired, C3 and C4. */
    const score = (svg, timing, where) => {
      const cand = mkdtempSync(path.join(scratch, `score-${id}-`))
      writeFileSync(path.join(cand, `${id}.svg`), svg)
      writeFileSync(path.join(cand, `${id}.timing.json`), timing)
      const result = compare(dir, cand)
      assert.deepEqual(result.pages, [1, 1], where)
      assert.equal(result.linksMatched[0], result.linksMatched[1], where)
      assert.deepEqual([result.pass.C3, result.pass.C4], [true, true], where)
      t.diagnostic(`${id} ${where}: C4 ${result.linksMatched[0]} links, |Δx| median ${result.dxMedian} p95 ${result.dxP95},` +
        ` |Δy| median ${result.dyMedian} p95 ${result.dyP95}`)
    }
    const native = { grobs: read(GROBS), layout: read(LAYOUT), stderr: read('err') }
    check(native, 'natively')
    score(read('svg'), read('timing.json'), 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([row.path], { trace: true, modules: given.map(([name, file]) => [name, readFileSync(file)]) })
    const o = wasm.outputs
    const inWasm = { grobs: o[GROBS]?.toString(), layout: o[LAYOUT]?.toString(), stderr: o.stderr?.toString() }
    assert.equal(inWasm.layout, native.layout)
    check(inWasm, 'in wasm')
    score(o[`${id}.svg`], o[`${id}.timing.json`], 'in wasm')
  })


for (const [id, what] of [
  ['scripts', 'Scripts and ScriptColumns'],
  ['fingerings', 'Fingerings, StringNumbers, StrokeFingers, their ScriptColumns and FingeringColumns'],
  ['arpeggios', 'Arpeggios, as chord brackets and chord slurs too, and the span arpeggios'],
  ['metronome', 'MetronomeMarks'],
]) {
  test(`fixtures/${id}.ly natively and in wasm, with the build's packs: LilyPond's grobs, ${what} at LilyPond's offsets and Y extents; its page passes C3 and C4`,
    { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async (t) => {
      const dir = await captureFixture(id)
      const row = LAYOUT_FIXTURES.find((r) => r.id === id)
      const given = everyPack()
      const out = mkdtempSync(path.join(scratch, `engine-${id}-`))
      const run = runExample('compile', ['--trace', ...given.flatMap(([, file]) => ['--pack', file]), '--out', out, row.path],
        { cwd: path.dirname(row.path), encoding: 'utf8' })
      assert.equal(run.status, 0, run.stderr)
      const read = (name) => readFileSync(path.join(out, `${id}.${name}`), 'utf8')
      const check = (outputs, where) => {
        assertY2Fixture(id, dir, outputs, where)
        const counts = countTypes(readLayout(outputs.layout))
        assert.deepEqual(Object.fromEntries(Object.keys(Y2_FIXTURES[id]).map((type) => [type, counts.get(type)])), Y2_FIXTURES[id], where)
      }
      /** The page scored against LilyPond's: one page, every link paired, C3 and C4. */
      const score = (svg, timing, where) => {
        const cand = mkdtempSync(path.join(scratch, `score-${id}-`))
        writeFileSync(path.join(cand, `${id}.svg`), svg)
        writeFileSync(path.join(cand, `${id}.timing.json`), timing)
        const result = compare(dir, cand)
        assert.deepEqual(result.pages, [1, 1], where)
        assert.equal(result.linksMatched[0], result.linksMatched[1], where)
        assert.deepEqual([result.pass.C3, result.pass.C4], [true, true], where)
        t.diagnostic(`${id} ${where}: C4 ${result.linksMatched[0]} links, |Δx| median ${result.dxMedian} p95 ${result.dxP95},` +
          ` |Δy| median ${result.dyMedian} p95 ${result.dyP95}`)
      }
      const native = { grobs: read(GROBS), layout: read(LAYOUT), stderr: read('err') }
      check(native, 'natively')
      score(read('svg'), read('timing.json'), 'natively')
      if (coreSkip()) return
      const [wasm] = await compileInWasm([row.path], { trace: true, modules: given.map(([name, file]) => [name, readFileSync(file)]) })
      const o = wasm.outputs
      const inWasm = { grobs: o[GROBS]?.toString(), layout: o[LAYOUT]?.toString(), stderr: o.stderr?.toString() }
      assert.equal(inWasm.layout, native.layout)
      check(inWasm, 'in wasm')
      score(o[`${id}.svg`], o[`${id}.timing.json`], 'in wasm')
    })
}

// --- broken spanners at LilyPond's breaks (MS3 step 60) ----------------------------------

/** The breaks of the line.tsv `text`: the `when` of each system's last column, `main grace`, but the last system's. */
function breaksOf(text) {
  return readLine(text).filter((c) => c.column.endsWith('@-1')).slice(0, -1).map((c) => c.column.split('@')[1])
}

/** The broken spanners of fixtures/broken-spanners.ly by type, their pieces counted (OPEN A40). */
const BROKEN_SPANNERS = {
  Hairpin: 2, OttavaBracket: 2, TextSpanner: 4, Glissando: 1, DynamicTextSpanner: 2, VoltaBracket: 3, HorizontalBracket: 4,
}

test("LilyPond captures fixtures/broken-spanners.ly in five systems at its \\breaks: each system's own line first, paired with grobs.tsv's System line (OPEN H9); the compare on them; ref/'s copy byte for byte",
  { skip: lilypond, timeout: 120_000 }, async () => {
    const id = 'broken-spanners'
    const dir = await captureFixture(id)
    const read = (name, at = dir) => readFileSync(path.join(at, name), 'utf8')
    const text = read(LAYOUT)
    const grobs = readLayout(text)
    const key = (g) => FIELDS.slice(0, 6).map((f) => g[f]).join('\t')
    const announced = new Set(readGrobs(read(GROBS)).flat().map(key))
    for (const g of grobs) assert.ok(announced.has(key(g)), `${id}:${g.line}`)
    const systems = grobs.filter((g) => g.type === 'System')
    assert.deepEqual(systems.map((g) => g.system), ['0', '1', '2', '3', '4'])
    assert.ok(grobs.every((g) => g.page === '0'))
    // Each system's line before its grobs, and on its page from the top down.
    assert.deepEqual(systems.map((g) => g.line), grobs.filter((g, i) => i === 0 || g.system !== grobs[i - 1].system).map((g) => g.line))
    assert.ok(systems.every((g, i) => i === 0 || +g.y > +systems[i - 1].y))
    assert.deepEqual(breaksOf(read(LINE)), ['2 0', '5 0', '7 0', '9 0'])
    const counts = countTypes(grobs)
    assert.deepEqual(Object.fromEntries(Object.keys(BROKEN_SPANNERS).map((t) => [t, counts.get(t)])), BROKEN_SPANNERS)
    assert.equal(compareLayout(text, text, all).equal, true)
    assertPerturbedFail(text, 'System')
    assertPerturbedFail(text, 'TextSpanner')
    const reference = path.join(references, id)
    if (existsSync(reference)) {
      for (const name of [GROBS, LAYOUT, COLUMNS, LINE, 'stderr.txt', `${id}.svg`]) assert.equal(read(name), read(name, reference), `${id} ${name}`)
    }
  })

test("fixtures/broken-spanners.ly natively and in wasm at LilyPond's breaks, with the build's packs: its five systems and its spanners' pieces LilyPond's, at LilyPond's offsets and Y extents; its page passes C3 and C4 (OPEN A40)",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async (t) => {
    const id = 'broken-spanners'
    const dir = await captureFixture(id)
    const row = LAYOUT_FIXTURES.find((r) => r.id === id)
    const given = everyPack()
    const breaks = breaksOf(readFileSync(path.join(dir, LINE), 'utf8'))
    const compile = (args) => {
      const out = mkdtempSync(path.join(scratch, `engine-${id}-`))
      const run = runExample('compile', ['--trace', ...args, ...given.flatMap(([, file]) => ['--pack', file]), '--out', out, row.path],
        { cwd: path.dirname(row.path), encoding: 'utf8' })
      assert.equal(run.status, 0, run.stderr)
      return (name) => readFileSync(path.join(out, `${id}.${name}`), 'utf8')
    }
    const read = compile(breaks.flatMap((b) => ['--break', b]))
    const ref = (name) => readFileSync(path.join(dir, name), 'utf8')
    const reference = ref(LAYOUT)
    const n = readLayout(reference).length
    const check = (outputs, where) => {
      assert.equal(outputs.stderr, ref('stderr.txt'), where)
      assert.equal(compareGrobs(ref(GROBS), outputs.grobs).equal, true, where)
      const result = compareLayout(reference, outputs.layout, all)
      assert.deepEqual([result.counts, result.missing, result.extra, result.order, result.unreached],
        [[n, n], [], [], [], unreachedOf(reference)], where)
      assert.deepEqual(differByType(reference, result), {}, where)
      assert.deepEqual(compareLine(ref(LINE), outputs.line), {
        equal: true, counts: [60, 60], missing: [], extra: [], order: [], differ: [], unreached: {},
      }, where)
    }
    const score = (svg, timing, where) => {
      const cand = mkdtempSync(path.join(scratch, `score-${id}-`))
      writeFileSync(path.join(cand, `${id}.svg`), svg)
      writeFileSync(path.join(cand, `${id}.timing.json`), timing)
      const result = compare(dir, cand)
      assert.deepEqual(result.pages, [1, 1], where)
      assert.equal(result.linksMatched[0], result.linksMatched[1], where)
      assert.deepEqual([result.pass.C3, result.pass.C4], [true, true], where)
      t.diagnostic(`${id} ${where}: C4 ${result.linksMatched[0]} links, |Δx| median ${result.dxMedian} p95 ${result.dxP95},` +
        ` |Δy| median ${result.dyMedian} p95 ${result.dyP95}`)
    }
    const native = { grobs: read(GROBS), layout: read(LAYOUT), line: read(LINE), stderr: read('err') }
    check(native, 'natively')
    score(read('svg'), read('timing.json'), 'natively')
    // Without the breaks, the engine's own (step 61): LilyPond's.
    const own = compile([])
    check({ grobs: own(GROBS), layout: own(LAYOUT), line: own(LINE), stderr: own('err') }, 'natively at its own breaks')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([row.path], { trace: true, breaks, modules: given.map(([name, file]) => [name, readFileSync(file)]) })
    const o = wasm.outputs
    const inWasm = { grobs: o[GROBS]?.toString(), layout: o[LAYOUT]?.toString(), line: o[LINE]?.toString(), stderr: o.stderr?.toString() }
    assert.equal(inWasm.layout, native.layout)
    check(inWasm, 'in wasm')
    score(o[`${id}.svg`], o[`${id}.timing.json`], 'in wasm')
  })

// --- the line breaker (MS3 step 61) -----------------------------------------------------

test("T05 natively and in wasm, with the build's packs: the engine's breaks LilyPond's two systems, its grobs.tsv, stderr, line.tsv and layout.tsv with both systems' lines LilyPond's (OPEN B1)",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const row = corpus().find((r) => r.id === 'T05')
    const dir = path.join(references, 'T05')
    if (!existsSync(path.join(dir, LAYOUT))) return
    const name = path.basename(row.path, '.ly')
    const given = everyPack()
    const out = mkdtempSync(path.join(scratch, 'engine-T05-'))
    const run = runExample('compile', ['--trace', ...given.flatMap(([, file]) => ['--pack', file]), '--out', out, row.path],
      { cwd: path.dirname(row.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (n) => readFileSync(path.join(out, `${name}.${n}`), 'utf8')
    const ref = (n) => readFileSync(path.join(dir, n), 'utf8')
    const reference = ref(LAYOUT)
    const n = readLayout(reference).length
    assert.deepEqual(readLayout(reference).filter((g) => g.type === 'System').map((g) => g.system), ['0', '1'])
    assert.deepEqual(breaksOf(ref(LINE)), ['12 0'])
    const check = (outputs, where) => {
      assert.equal(outputs.stderr, ref('stderr.txt'), where)
      assert.equal(compareGrobs(ref(GROBS), outputs.grobs).equal, true, where)
      const result = compareLayout(reference, outputs.layout, all)
      assert.deepEqual([result.counts, result.missing, result.extra, result.order, result.differ, result.unreached],
        [[n, n], [], [], [], [], unreachedOf(reference)], where)
      const line = compareLine(ref(LINE), outputs.line)
      assert.deepEqual([line.equal, line.counts], [true, [58, 58]], where)
    }
    const native = { grobs: read(GROBS), layout: read(LAYOUT), line: read(LINE), stderr: read('err') }
    check(native, 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([row.path], { trace: true, modules: given.map(([id, file]) => [id, readFileSync(file)]) })
    const o = wasm.outputs
    const inWasm = { grobs: o[GROBS]?.toString(), layout: o[LAYOUT]?.toString(), line: o[LINE]?.toString(), stderr: o.stderr?.toString() }
    assert.equal(inWasm.layout, native.layout)
    check(inWasm, 'in wasm')
  })

// --- the page breaker (MS3 step 62) -----------------------------------------------------

// T02's Lyrics are MS4's (R5): until MS4 step 15 its grobs.tsv lacked them, and its layout.tsv their lines; whole
// since.
for (const [id, systems, pages, lines, whole] of [['T17', ['0', '1', '2', '3'], ['0', '0', '0', '1'], 54, true], ['T02', ['0', '1', '2'], ['0', '0', '1'], 29, true]]) {
  test(`${id} natively and in wasm, with the build's packs: ly:optimal-breaking breaks LilyPond's ${systems.length} systems on ${new Set(pages).size} pages, its grobs.tsv, stderr, line.tsv and layout.tsv with each system's page and place LilyPond's, its pages C3 and C4 (OPEN B1, H9)`,
    { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
      const row = corpus().find((r) => r.id === id)
      const dir = path.join(references, id)
      if (!existsSync(path.join(dir, LAYOUT))) return
      const name = path.basename(row.path, '.ly')
      const given = everyPack()
      const out = mkdtempSync(path.join(scratch, `engine-${id}-`))
      const run = runExample('compile', ['--trace', ...given.flatMap(([, file]) => ['--pack', file]), '--out', out, row.path],
        { cwd: path.dirname(row.path), encoding: 'utf8' })
      assert.equal(run.status, 0, run.stderr)
      const read = (n) => readFileSync(path.join(out, `${name}.${n}`), 'utf8')
      const ref = (n) => readFileSync(path.join(dir, n), 'utf8')
      const reference = ref(LAYOUT)
      const systemLines = readLayout(reference).filter((g) => g.type === 'System')
      assert.deepEqual([systemLines.map((g) => g.system), systemLines.map((g) => g.page)], [systems, pages])
      const check = (outputs, where) => {
        assert.equal(outputs.stderr, ref('stderr.txt'), where)
        assert.equal(compareGrobs(ref(GROBS), outputs.grobs).equal, whole, where)
        const result = compareLayout(reference, outputs.layout, all)
        const placed = readLayout(outputs.layout).filter((g) => g.type === 'System')
        assert.deepEqual(placed.map((g) => [g.system, g.page, g.x, g.y]), systemLines.map((g) => [g.system, g.page, g.x, g.y]), where)
        // Every grob both have LilyPond's.
        assert.deepEqual([result.extra, result.order, result.differ], [[], [], []], where)
        if (whole) {
          const n = readLayout(reference).length
          assert.deepEqual([result.counts, result.missing, result.unreached], [[n, n], [], unreachedOf(reference)], where)
        } else {
          assert.ok(result.missing.every((m) => /\tLyrics#/.test(m.grob)), where)
        }
        const line = compareLine(ref(LINE), outputs.line)
        assert.deepEqual([line.equal, line.counts], [true, [lines, lines]], where)
      }
      const page = (svgs, timing, where) => {
        const t = mkdtempSync(path.join(scratch, `pages-${id}-`))
        for (const [n, svg] of Object.entries(svgs)) writeFileSync(path.join(t, n), svg)
        writeFileSync(path.join(t, `${name}.timing.json`), timing)
        const c = compare(dir, t)
        assert.deepEqual([c.pages, c.pass.C3, c.pass.C4, c.dxP95, c.dyP95], [[2, 2], true, true, 0, 0], where)
      }
      const svgsIn = (files) => Object.fromEntries(Object.entries(files).filter(([n]) => n.startsWith(`${name}-`) && n.endsWith('.svg')))
      const native = { grobs: read(GROBS), layout: read(LAYOUT), line: read(LINE), stderr: read('err') }
      check(native, 'natively')
      const files = Object.fromEntries(readdirSync(out).map((n) => [n, readFileSync(path.join(out, n))]))
      page(svgsIn(files), read('timing.json'), 'natively')
      if (coreSkip()) return
      const [wasm] = await compileInWasm([row.path], { trace: true, modules: given.map(([gid, file]) => [gid, readFileSync(file)]) })
      const o = wasm.outputs
      const inWasm = { grobs: o[GROBS]?.toString(), layout: o[LAYOUT]?.toString(), line: o[LINE]?.toString(), stderr: o.stderr?.toString() }
      assert.equal(inWasm.layout, native.layout)
      check(inWasm, 'in wasm')
      page(svgsIn(o), o[`${name}.timing.json`], 'in wasm')
    })
}

// --- titles and top-level markups as page elements (MS3 step 63) ------------------------

test("fixtures/titles.ly natively and in wasm, with the build's packs: its titles, top-level markups, tagline and \\bookpart spaced as LilyPond's on its two pages; its grobs.tsv, stderr, line.tsv and layout.tsv with each system's page and place LilyPond's, its pages C3 and C4 at Δ 0",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const id = 'titles'
    const dir = await captureFixture(id)
    const row = LAYOUT_FIXTURES.find((r) => r.id === id)
    const given = everyPack()
    const out = mkdtempSync(path.join(scratch, `engine-${id}-`))
    const run = runExample('compile', ['--trace', ...given.flatMap(([, file]) => ['--pack', file]), '--out', out, row.path],
      { cwd: path.dirname(row.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (n) => readFileSync(path.join(out, `${id}.${n}`), 'utf8')
    const ref = (n) => readFileSync(path.join(dir, n), 'utf8')
    const reference = ref(LAYOUT)
    const n = readLayout(reference).length
    // Two scores under the book's titles and markups on its first page, the part's one on its second.
    const systems = readLayout(reference).filter((g) => g.type === 'System')
    assert.deepEqual(systems.map((g) => [g.system, g.page]), [['0', '0'], ['1', '0'], ['2', '1']])
    const check = (outputs, where) => {
      assert.equal(outputs.stderr, ref('stderr.txt'), where)
      assert.equal(compareGrobs(ref(GROBS), outputs.grobs).equal, true, where)
      const result = compareLayout(reference, outputs.layout, all)
      assert.deepEqual([result.counts, result.missing, result.extra, result.order, result.differ, result.unreached],
        [[n, n], [], [], [], [], unreachedOf(reference)], where)
      const placed = readLayout(outputs.layout).filter((g) => g.type === 'System')
      assert.deepEqual(placed.map((g) => [g.system, g.page, g.x, g.y]), systems.map((g) => [g.system, g.page, g.x, g.y]), where)
      assert.deepEqual(compareLine(ref(LINE), outputs.line), {
        equal: true, counts: [22, 22], missing: [], extra: [], order: [], differ: [], unreached: {},
      }, where)
    }
    const page = (svgs, timing, where) => {
      const t = mkdtempSync(path.join(scratch, `pages-${id}-`))
      for (const [name, svg] of Object.entries(svgs)) writeFileSync(path.join(t, name), svg)
      writeFileSync(path.join(t, `${id}.timing.json`), timing)
      const c = compare(dir, t)
      assert.deepEqual([c.pages, c.pass.C3, c.pass.C4, c.linksMatched, c.dxP95, c.dyP95], [[2, 2], true, true, [13, 13], 0, 0], where)
    }
    const svgsIn = (files) => Object.fromEntries(Object.entries(files).filter(([name]) => /^titles-\d\.svg$/.test(name)))
    check({ grobs: read(GROBS), layout: read(LAYOUT), line: read(LINE), stderr: read('err') }, 'natively')
    page(svgsIn(Object.fromEntries(readdirSync(out).map((name) => [name, readFileSync(path.join(out, name))]))), read('timing.json'), 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([row.path], { trace: true, modules: given.map(([gid, file]) => [gid, readFileSync(file)]) })
    const o = wasm.outputs
    const inWasm = { grobs: o[GROBS]?.toString(), layout: o[LAYOUT]?.toString(), line: o[LINE]?.toString(), stderr: o.stderr?.toString() }
    assert.equal(inWasm.layout, read(LAYOUT))
    check(inWasm, 'in wasm')
    page(svgsIn(o), o[`${id}.timing.json`], 'in wasm')
  })

// --- page markers and labels (MS3 step 64) ---------------------------------------------

test("fixtures/page-markers.ly natively and in wasm, with the build's packs: its toplevel \\label, \\pageBreak and \\noPageBreak placing its scores and markups on LilyPond's two pages; its grobs.tsv, stderr, line.tsv and layout.tsv with each system's page and place LilyPond's, its pages C3 and C4 at Δ 0",
  { skip: lilypond || cargo || skipPacks, timeout: 300_000 }, async () => {
    const id = 'page-markers'
    const dir = await captureFixture(id)
    const row = LAYOUT_FIXTURES.find((r) => r.id === id)
    const given = everyPack()
    const out = mkdtempSync(path.join(scratch, `engine-${id}-`))
    const run = runExample('compile', ['--trace', ...given.flatMap(([, file]) => ['--pack', file]), '--out', out, row.path],
      { cwd: path.dirname(row.path), encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    const read = (n) => readFileSync(path.join(out, `${id}.${n}`), 'utf8')
    const ref = (n) => readFileSync(path.join(dir, n), 'utf8')
    const reference = ref(LAYOUT)
    const n = readLayout(reference).length
    // The first score on the first page, the second after the \pageBreak on the second.
    const systems = readLayout(reference).filter((g) => g.type === 'System')
    assert.deepEqual(systems.map((g) => [g.system, g.page]), [['0', '0'], ['1', '1']])
    const check = (outputs, where) => {
      assert.equal(outputs.stderr, ref('stderr.txt'), where)
      assert.equal(compareGrobs(ref(GROBS), outputs.grobs).equal, true, where)
      const result = compareLayout(reference, outputs.layout, all)
      assert.deepEqual([result.counts, result.missing, result.extra, result.order, result.differ, result.unreached],
        [[n, n], [], [], [], [], unreachedOf(reference)], where)
      const placed = readLayout(outputs.layout).filter((g) => g.type === 'System')
      assert.deepEqual(placed.map((g) => [g.system, g.page, g.x, g.y]), systems.map((g) => [g.system, g.page, g.x, g.y]), where)
      assert.deepEqual(compareLine(ref(LINE), outputs.line), {
        equal: true, counts: [16, 16], missing: [], extra: [], order: [], differ: [], unreached: {},
      }, where)
    }
    const page = (svgs, timing, where) => {
      const t = mkdtempSync(path.join(scratch, `pages-${id}-`))
      for (const [name, svg] of Object.entries(svgs)) writeFileSync(path.join(t, name), svg)
      writeFileSync(path.join(t, `${id}.timing.json`), timing)
      const c = compare(dir, t)
      assert.deepEqual([c.pages, c.pass.C3, c.pass.C4, c.linksMatched, c.dxP95, c.dyP95], [[2, 2], true, true, [10, 10], 0, 0], where)
    }
    const svgsIn = (files) => Object.fromEntries(Object.entries(files).filter(([name]) => /^page-markers-\d\.svg$/.test(name)))
    check({ grobs: read(GROBS), layout: read(LAYOUT), line: read(LINE), stderr: read('err') }, 'natively')
    page(svgsIn(Object.fromEntries(readdirSync(out).map((name) => [name, readFileSync(path.join(out, name))]))), read('timing.json'), 'natively')
    if (coreSkip()) return
    const [wasm] = await compileInWasm([row.path], { trace: true, modules: given.map(([gid, file]) => [gid, readFileSync(file)]) })
    const o = wasm.outputs
    const inWasm = { grobs: o[GROBS]?.toString(), layout: o[LAYOUT]?.toString(), line: o[LINE]?.toString(), stderr: o.stderr?.toString() }
    assert.equal(inWasm.layout, read(LAYOUT))
    check(inWasm, 'in wasm')
    page(svgsIn(o), o[`${id}.timing.json`], 'in wasm')
  })

// --- MS3's gate on the 10 (MS3 step 65) -------------------------------------------------

/** MS3's 10 (part 5's gate): Y2's eight, T05's two systems and T17's two pages. */
const TEN = ['T01', 'T03', 'T05', 'T10', 'T13', 'T14', 'T16', 'T17', 'T18', 'T19']
const tenRows = TEN.map((id) => corpus().find((r) => r.id === id))
const skipTen = tenRows.every((row) => row && existsSync(path.join(references, row.id, LAYOUT))) ? false
  : `the 10's layout.tsv is not in ${references}; run fetch.mjs and capture.mjs`

test("MS3's gate: the 10's layout.tsv LilyPond's on every field of their 1,461 live grobs and each System's line, natively and in one core.wasm instance, with the preloaded packs and the others fetched",
  { skip: lilypond || cargo || skipPacks || skipTen, timeout: 600_000 }, async () => {
    const rows = tenRows
    const check = (row, text, where) => {
      const reference = readFileSync(path.join(references, row.id, LAYOUT), 'utf8')
      const n = readLayout(reference).length
      assert.deepEqual(compareLayout(reference, text, all),
        { equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [], unreached: unreachedOf(reference) }, `${row.id} ${where}`)
      return grobLines(reference).length
    }
    const native = rows.map((row) => {
      const out = mkdtempSync(path.join(scratch, `engine-gate-${row.id}-`))
      runExample('compile', ['--trace', ...packs.flatMap(([, file]) => ['--pack', file]), '--out', out, row.path],
        { cwd: path.dirname(row.path), encoding: 'utf8' })
      return readFileSync(path.join(out, `${path.basename(row.path, '.ly')}.${LAYOUT}`), 'utf8')
    })
    assert.equal(rows.reduce((sum, row, i) => sum + check(row, native[i], 'natively'), 0), 1461)
    if (coreSkip()) return
    const wasm = await compileInWasm(rows.map((row) => row.path), { trace: true, modules: packModules() })
    for (const [i, row] of rows.entries()) {
      assert.equal(wasm[i].outputs[LAYOUT]?.toString(), native[i], row.id)
      check(row, native[i], 'in wasm')
    }
  })

/** MS4 step 3: the probe of the Beam's callbacks before quanting, and what it is run on, with its lines in LilyPond's run. */
const BEAM_PROBE = fileURLToPath(new URL('../beam-probe.ly', import.meta.url))
const BEAM_PROBED = { beams: 84, T04: 26, T08: 12, T09: 12, T11: 10, T15: 2 }
const beamProbed = Object.keys(BEAM_PROBED).map((id) => id === 'beams'
  ? fileURLToPath(new URL('../fixtures/beams.ly', import.meta.url))
  : corpus().find((row) => row.id === id)?.path)
const skipBeamProbed = beamProbed.every((f) => f && existsSync(f)) ? false : 'the corpus is not fetched: run node tools/corpus/fetch.mjs'

test("beam-probe.ly on the beams fixture, T04, T08, T09, T11 and T15: each Beam's direction, knee, shorten, cross-staff, normal stems, its stems' directions and beaming, X-positions and segments LilyPond's to the last bit, natively and in wasm",
  { skip: lilypond || cargo || skipBeamProbed, timeout: 600_000 }, async () => {
    const files = []
    for (const [i, id] of Object.keys(BEAM_PROBED).entries()) {
      const dir = mkdtempSync(path.join(scratch, `beam-probe-${id}-`))
      const file = path.join(dir, 'probe.ly')
      writeFileSync(file, `\\include "${BEAM_PROBE}"\n\\include "${beamProbed[i]}"\n`)
      const reference = spawnSync('lilypond', ['--loglevel=ERROR', '-dno-print-pages', file], { cwd: dir, encoding: 'utf8', env: { ...process.env, LANGUAGE: 'en' } })
      assert.equal(reference.status, 0, `${id}: ${reference.stderr}`)
      const want = readFileSync(path.join(dir, 'beam-probe.txt'), 'utf8')
      // A line before line breaking and one after for each Beam of the score.
      const lines = want.split('\n').filter(Boolean)
      assert.equal(lines.length, BEAM_PROBED[id], id)
      assert.equal(lines.filter((l) => l.split('\t')[1] === 'before').length, BEAM_PROBED[id] / 2, id)
      const out = path.join(dir, 'engine')
      mkdirSync(out)
      const run = runExample('compile', ['--out', out, file], { cwd: dir, encoding: 'utf8' })
      assert.equal(run.status, 0, `${id}: ${run.stderr}`)
      assert.equal(readFileSync(path.join(out, 'beam-probe.txt'), 'utf8'), want, `${id} natively`)
      files.push([id, file, want])
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(files.map(([, file]) => file))
    for (const [i, [id, , want]] of files.entries()) assert.equal(wasm[i].outputs['beam-probe.txt']?.toString(), want, `${id} in wasm`)
  })

/** MS4 step 9: the probe of the ties' formatting, what it is run on with its ties in LilyPond's run, and the ties whose
 * control points may be an ulp off LilyPond's in Y, the `libm` crate's `atan` against macOS's (D226, OPEN E21). */
const TIE_PROBE = fileURLToPath(new URL('../tie-probe.ly', import.meta.url))
const TIE_PROBED = { ties: 82, T24: 32 }
const ATAN_ULP = { ties: ['ties.ly:64:39:39\t-1\t0'], T24: ['bach-bwv610.ly:105:35:35\t-1\t0'] }
const tieProbed = Object.keys(TIE_PROBED).map((id) => id === 'ties'
  ? fileURLToPath(new URL('../fixtures/ties.ly', import.meta.url))
  : corpus().find((row) => row.id === id)?.path)
const skipTieProbed = tieProbed.every((f) => f && existsSync(f)) ? false : 'the corpus is not fetched: run node tools/corpus/fetch.mjs'

/** Whether tie-probe.txt's `got` is `want`, its lines in any order (the after-line-breaking order of the columns is the
 * systems'), each to the last bit but the ties `ulp` names, whose numbers may be 1e-12 apart. */
function sameTies (want, got, ulp) {
  const lines = (t) => t.split('\n').filter(Boolean).sort()
  const [w, g] = [lines(want), lines(got)]
  if (w.length !== g.length) return `${g.length} lines for ${w.length}`
  for (const [i, line] of w.entries()) {
    if (line === g[i]) continue
    const key = line.split('\t').slice(0, 3).join('\t')
    if (!ulp.includes(key) || g[i].split('\t').slice(0, 4).join('\t') !== line.split('\t').slice(0, 4).join('\t')) return `${g[i]} for ${line}`
    const [a, b] = [line, g[i]].map((l) => l.split('\t')[4].split(' ').map(Number))
    if (a.some((x, k) => Math.abs(x - b[k]) > 1e-12)) return `${g[i]} for ${line}`
  }
  return ''
}

test("tie-probe.ly on fixtures/ties.ly and T24: each tie column's ties, positioned by the formatting problem (tie-formatting-problem.cc), at LilyPond's directions and control points to the last bit but atan's (D226), natively and in wasm",
  { skip: lilypond || cargo || skipTieProbed || skipPacks, timeout: 600_000 }, async () => {
    const files = []
    for (const [i, id] of Object.keys(TIE_PROBED).entries()) {
      const dir = mkdtempSync(path.join(scratch, `tie-probe-${id}-`))
      const file = path.join(dir, 'probe.ly')
      writeFileSync(file, `\\include "${TIE_PROBE}"\n\\include "${tieProbed[i]}"\n`)
      const reference = spawnSync('lilypond', ['--loglevel=ERROR', '-dno-print-pages', file], { cwd: dir, encoding: 'utf8', env: { ...process.env, LANGUAGE: 'en' } })
      assert.equal(reference.status, 0, `${id}: ${reference.stderr}`)
      const want = readFileSync(path.join(dir, 'tie-probe.txt'), 'utf8')
      assert.equal(want.split('\n').filter(Boolean).length, TIE_PROBED[id], id)
      const out = path.join(dir, 'engine')
      mkdirSync(out)
      const run = runExample('compile', [...packs.flatMap(([, f]) => ['--pack', f]), '--out', out, file], { cwd: dir, encoding: 'utf8' })
      assert.equal(run.status, 0, `${id}: ${run.stderr}`)
      assert.equal(sameTies(want, readFileSync(path.join(out, 'tie-probe.txt'), 'utf8'), ATAN_ULP[id]), '', `${id} natively`)
      files.push([id, file, want])
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(files.map(([, file]) => file), { modules: packModules() })
    for (const [i, [id, , want]] of files.entries()) {
      assert.equal(sameTies(want, wasm[i].outputs['tie-probe.txt']?.toString() ?? '', ATAN_ULP[id]), '', `${id} in wasm`)
    }
  })

/** fixtures/beams.ly's row, as capture.mjs takes one (MS4 step 2). */
const beamsFixture = { id: 'beams', file: 'tools/compare/fixtures/beams.ly', path: fileURLToPath(new URL('../fixtures/beams.ly', import.meta.url)) }
/** fixtures/tremolos.ly's row (MS4 step 5). */
const tremolosFixture = { id: 'tremolos', file: 'tools/compare/fixtures/tremolos.ly', path: fileURLToPath(new URL('../fixtures/tremolos.ly', import.meta.url)) }
/** The scores whose beams MS4 step 4 lays out whole, with their files' stems. */
const QUANTED = { T11: 'beam-cross-staff', T15: 'scheme-engraver' }
const quanted = Object.keys(QUANTED).map((id) => corpus().find((row) => row.id === id))
const skipQuanted = quanted.every((row) => row && existsSync(row.path) && existsSync(path.join(references, row.id, LAYOUT)))
  ? false : `no layout.tsv in ${references}; run fetch.mjs and capture.mjs`

/**
 * The engine's outputs of `id` against LilyPond's capture in `dir`: its layout.tsv LilyPond's on every compared field,
 * its page's C3 and C4 with every Δ 0 (`outputs` the candidate's files by name, its page among them), and its stderr
 * LilyPond's, T15's whole since MS8 step 11 (its events printed as LilyPond's, OPEN C6).
 */
function assertQuanted(id, dir, { layout, stderr, files }, where) {
  const reference = readFileSync(path.join(dir, 'stderr.txt'), 'utf8')
  assert.equal(stderr, reference, `${id} ${where}`)
  const laidOut = readFileSync(path.join(dir, LAYOUT), 'utf8')
  const n = readLayout(laidOut).length
  assert.deepEqual(compareLayout(laidOut, layout, all), {
    equal: true, counts: [n, n], missing: [], extra: [], order: [], differ: [], unreached: unreachedOf(laidOut),
  }, `${id} ${where}`)
  const cand = mkdtempSync(path.join(scratch, `quanted-${id}-`))
  for (const [name, bytes] of Object.entries(files)) writeFileSync(path.join(cand, name), bytes)
  const r = compare(dir, cand)
  assert.deepEqual([r.pages[0], r.pages[1], r.breaks, r.dxMedian, r.dxP95, r.dyMedian, r.dyP95], [r.pages[0], r.pages[0], 1, 0, 0, 0, 0], `${id} ${where}`)
  assert.equal(r.linksMatched[0], r.linksMatched[1], `${id} ${where}`)
}

test("fixtures/beams.ly, fixtures/tremolos.ly, T11 and T15 natively and in wasm, quanted: layout.tsv LilyPond's on every field, the pages' C3 and C4 with every Δ 0, the stderr LilyPond's (T15's but its printed events)",
  { skip: lilypond || cargo || skipPacks || skipQuanted, timeout: 600_000 }, async () => {
    const dir = path.join(scratch, 'beams')
    const result = await captureOne('lilypond', beamsFixture, dir)
    assert.equal(result.exit, 0)
    const tremolos = path.join(scratch, 'tremolos')
    assert.equal((await captureOne('lilypond', tremolosFixture, tremolos)).exit, 0)
    // The tremolos' 12 StemTremolos and their 12 beams, 6 of them the chord
    // tremolos', LilyPond's in grobs.tsv as in layout.tsv.
    assert.deepEqual(Object.fromEntries(countTypes(readLayout(readFileSync(path.join(tremolos, LAYOUT), 'utf8'))
      .filter((g) => ['StemTremolo', 'Beam'].includes(g.type)))), { StemTremolo: 12, Beam: 12 })
    const rows = [[beamsFixture.id, 'beams', beamsFixture.path, dir], [tremolosFixture.id, 'tremolos', tremolosFixture.path, tremolos],
      ...quanted.map((row) => [row.id, QUANTED[row.id], row.path, path.join(references, row.id)])]
    const natively = {}
    for (const [id, stem, file, ref] of rows) {
      const out = mkdtempSync(path.join(scratch, `engine-quanted-${id}-`))
      const run = runExample('compile', ['--trace', ...packs.flatMap(([, f]) => ['--pack', f]), '--out', out, file],
        { cwd: path.dirname(file), encoding: 'utf8' })
      assert.equal(run.status, 0, `${id}: ${run.stderr}`)
      const files = Object.fromEntries(readdirSync(out).filter((f) => f.endsWith('.svg')).map((f) => [f, readFileSync(path.join(out, f))]))
      const layout = readFileSync(path.join(out, `${stem}.${LAYOUT}`), 'utf8')
      const stderr = readFileSync(path.join(out, `${stem}.err`), 'utf8')
      natively[id] = layout
      if (id === tremolosFixture.id) {
        const heard = compareGrobs(readFileSync(path.join(ref, GROBS), 'utf8'), readFileSync(path.join(out, `${stem}.${GROBS}`), 'utf8'))
        assert.equal(heard.equal, true, JSON.stringify(heard))
      }
      assertQuanted(id, ref, { layout, stderr, files }, 'natively')
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(rows.map(([, , file]) => file), { trace: true, modules: packModules() })
    for (const [i, [id, , , ref]] of rows.entries()) {
      const outputs = wasm[i].outputs
      const files = Object.fromEntries(Object.entries(outputs).filter(([f]) => f.endsWith('.svg')))
      assert.equal(outputs[LAYOUT]?.toString(), natively[id], id)
      assertQuanted(id, ref, { layout: outputs[LAYOUT].toString(), stderr: outputs.stderr.toString(), files }, 'in wasm')
    }
  })

/** fixtures/slurs.ly's row (MS4 step 7). */
const slursFixture = { id: 'slurs', file: 'tools/compare/fixtures/slurs.ly', path: fileURLToPath(new URL('../fixtures/slurs.ly', import.meta.url)) }
/** The scores whose slurs MS4 step 8 lays out whole (census.txt: T27 needs beams and slurs only), with their files' stems. */
const SCORED = { T27: 'bach-schenker' }
const scored = Object.keys(SCORED).map((id) => corpus().find((row) => row.id === id))
const skipScored = scored.every((row) => row && existsSync(row.path) && existsSync(path.join(references, row.id, LAYOUT)))
  ? false : `no layout.tsv in ${references}; run fetch.mjs and capture.mjs`

test("fixtures/slurs.ly and T27 natively and in wasm, their slurs scored: layout.tsv LilyPond's on every field, the pages' C3 and C4 with every Δ 0, the stderr LilyPond's",
  { skip: lilypond || cargo || skipPacks || skipScored, timeout: 600_000 }, async () => {
    const dir = path.join(scratch, 'slurs-scored')
    assert.equal((await captureOne('lilypond', slursFixture, dir)).exit, 0)
    const rows = [[slursFixture.id, 'slurs', slursFixture.path, dir],
      ...scored.map((row) => [row.id, SCORED[row.id], row.path, path.join(references, row.id)])]
    const natively = {}
    for (const [id, stem, file, ref] of rows) {
      const out = mkdtempSync(path.join(scratch, `engine-scored-${id}-`))
      const run = runExample('compile', ['--trace', ...packs.flatMap(([, f]) => ['--pack', f]), '--out', out, file],
        { cwd: path.dirname(file), encoding: 'utf8' })
      assert.equal(run.status, 0, `${id}: ${run.stderr}`)
      const files = Object.fromEntries(readdirSync(out).filter((f) => f.endsWith('.svg')).map((f) => [f, readFileSync(path.join(out, f))]))
      const layout = readFileSync(path.join(out, `${stem}.${LAYOUT}`), 'utf8')
      natively[id] = layout
      assertQuanted(id, ref, { layout, stderr: readFileSync(path.join(out, `${stem}.err`), 'utf8'), files }, 'natively')
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(rows.map(([, , file]) => file), { trace: true, modules: packModules() })
    for (const [i, [id, , , ref]] of rows.entries()) {
      const outputs = wasm[i].outputs
      const files = Object.fromEntries(Object.entries(outputs).filter(([f]) => f.endsWith('.svg')))
      assert.equal(outputs[LAYOUT]?.toString(), natively[id], id)
      assertQuanted(id, ref, { layout: outputs[LAYOUT].toString(), stderr: outputs.stderr.toString(), files }, 'in wasm')
    }
  })

/** fixtures/ties.ly's row (MS4 step 9). */
const tiesFixture = { id: 'ties', file: 'tools/compare/fixtures/ties.ly', path: fileURLToPath(new URL('../fixtures/ties.ly', import.meta.url)) }
/** The scores MS4 step 10 lays out whole with their ties (census.txt; T29's RhythmicStaffs with Pitch_squash_engraver,
 * pulled forward by the operator's answer), with their files' stems. */
const TIED = { T12: 'slur-scoring', T24: 'bach-bwv610', T29: 'orchestra' }
const tied = Object.keys(TIED).map((id) => corpus().find((row) => row.id === id))
const skipTied = tied.every((row) => row && existsSync(row.path) && existsSync(path.join(references, row.id, LAYOUT)))
  ? false : `no layout.tsv in ${references}; run fetch.mjs and capture.mjs`

test("fixtures/ties.ly, T12, T24 and T29 natively and in wasm, their ties drawn: layout.tsv LilyPond's on every field, the pages' C3 and C4 with every Δ 0, the stderr LilyPond's",
  { skip: lilypond || cargo || skipPacks || skipTied, timeout: 900_000 }, async () => {
    const dir = path.join(scratch, 'ties-drawn')
    assert.equal((await captureOne('lilypond', tiesFixture, dir)).exit, 0)
    const rows = [[tiesFixture.id, 'ties', tiesFixture.path, dir],
      ...tied.map((row) => [row.id, TIED[row.id], row.path, path.join(references, row.id)])]
    const natively = {}
    for (const [id, stem, file, ref] of rows) {
      const out = mkdtempSync(path.join(scratch, `engine-tied-${id}-`))
      const run = runExample('compile', ['--trace', ...packs.flatMap(([, f]) => ['--pack', f]), '--out', out, file],
        { cwd: path.dirname(file), encoding: 'utf8' })
      assert.equal(run.status, 0, `${id}: ${run.stderr}`)
      // The pages with the playback map, which C3's breaks are read from (T12's systems).
      const files = Object.fromEntries(readdirSync(out).filter((f) => f.endsWith('.svg') || f.endsWith('.timing.json')).map((f) => [f, readFileSync(path.join(out, f))]))
      const layout = readFileSync(path.join(out, `${stem}.${LAYOUT}`), 'utf8')
      natively[id] = layout
      assertQuanted(id, ref, { layout, stderr: readFileSync(path.join(out, `${stem}.err`), 'utf8'), files }, 'natively')
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(rows.map(([, , file]) => file), { trace: true, modules: packModules() })
    for (const [i, [id, , , ref]] of rows.entries()) {
      const outputs = wasm[i].outputs
      const files = Object.fromEntries(Object.entries(outputs).filter(([f]) => f.endsWith('.svg') || f.endsWith('.timing.json')))
      assert.equal(outputs[LAYOUT]?.toString(), natively[id], id)
      assertQuanted(id, ref, { layout: outputs[LAYOUT].toString(), stderr: outputs.stderr.toString(), files }, 'in wasm')
    }
  })

/** fixtures/tuplets.ly's and fixtures/drums.ly's rows (MS4 steps 12 and 13). */
const tupletsFixture = { id: 'tuplets', file: 'tools/compare/fixtures/tuplets.ly', path: fileURLToPath(new URL('../fixtures/tuplets.ly', import.meta.url)) }
const drumsFixture = { id: 'drums', file: 'tools/compare/fixtures/drums.ly', path: fileURLToPath(new URL('../fixtures/drums.ly', import.meta.url)) }
/** The scores MS4 step 13 lays out whole with their tuplets and drums (census.txt), with their files' stems. */
const TUPLETED = { T07: 'drums', T08: 'tuplet-nest', T09: 'grace', T25: 'granados', T26: 'stockhausen-klavierstueckII', T28: 'cary' }
const tupleted = Object.keys(TUPLETED).map((id) => corpus().find((row) => row.id === id))
const skipTupleted = tupleted.every((row) => row && existsSync(row.path) && existsSync(path.join(references, row.id, LAYOUT)))
  ? false : `no layout.tsv in ${references}; run fetch.mjs and capture.mjs`

/**
 * T25 as capture.ly compiles it: its `#(ly:set-option 'point-and-click #f)` blanked in a copy beside its include,
 * whose links C4 then pairs (capture.ly ignores the option, so that every reference has its links).
 */
function withPointAndClick(row) {
  const dir = mkdtempSync(path.join(scratch, `point-and-click-${row.id}-`))
  const option = "#(ly:set-option 'point-and-click #f)"
  const text = readFileSync(row.path, 'utf8')
  assert.ok(text.includes(option), row.id)
  writeFileSync(path.join(dir, path.basename(row.path)), text.replace(option, '%'.padEnd(option.length)))
  writeFileSync(path.join(dir, 'example-header.ily'), readFileSync(path.join(path.dirname(row.path), 'example-header.ily')))
  return path.join(dir, path.basename(row.path))
}

test("fixtures/tuplets.ly, fixtures/drums.ly, T07, T08, T09, T25, T26 and T28 natively and in wasm, their tuplets and drums laid out: layout.tsv LilyPond's on every field, the pages' C3 and C4 with every Δ 0, the stderr LilyPond's",
  { skip: lilypond || cargo || skipPacks || skipTupleted, timeout: 900_000 }, async () => {
    const tupletsDir = path.join(scratch, 'tuplets-laid-out')
    assert.equal((await captureOne('lilypond', tupletsFixture, tupletsDir)).exit, 0)
    const drumsDir = path.join(scratch, 'drums-laid-out')
    assert.equal((await captureOne('lilypond', drumsFixture, drumsDir)).exit, 0)
    const rows = [[tupletsFixture.id, 'tuplets', tupletsFixture.path, tupletsDir], [drumsFixture.id, 'drums', drumsFixture.path, drumsDir],
      ...tupleted.map((row) => [row.id, TUPLETED[row.id], row.id === 'T25' ? withPointAndClick(row) : row.path, path.join(references, row.id)])]
    const natively = {}
    for (const [id, stem, file, ref] of rows) {
      const out = mkdtempSync(path.join(scratch, `engine-tupleted-${id}-`))
      const run = runExample('compile', ['--trace', ...packs.flatMap(([, f]) => ['--pack', f]), '--out', out, file],
        { cwd: path.dirname(file), encoding: 'utf8' })
      assert.equal(run.status, 0, `${id}: ${run.stderr}`)
      const files = Object.fromEntries(readdirSync(out).filter((f) => f.endsWith('.svg') || f.endsWith('.timing.json')).map((f) => [f, readFileSync(path.join(out, f))]))
      const layout = readFileSync(path.join(out, `${stem}.${LAYOUT}`), 'utf8')
      natively[id] = layout
      assertQuanted(id, ref, { layout, stderr: readFileSync(path.join(out, `${stem}.err`), 'utf8'), files }, 'natively')
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(rows.map(([, , file]) => file), { trace: true, modules: packModules() })
    for (const [i, [id, , , ref]] of rows.entries()) {
      const outputs = wasm[i].outputs
      const files = Object.fromEntries(Object.entries(outputs).filter(([f]) => f.endsWith('.svg') || f.endsWith('.timing.json')))
      assert.equal(outputs[LAYOUT]?.toString(), natively[id], id)
      assertQuanted(id, ref, { layout: outputs[LAYOUT].toString(), stderr: outputs.stderr.toString(), files }, 'in wasm')
    }
  })

/** The 30 of the corpus, in their order. */
const thirty = corpus().filter((row) => /^T\d\d$/.test(row.id))
const skipThirty = thirty.length === 30 && thirty.every((row) => existsSync(row.path)) ? false : 'the corpus is not fetched: run node tools/corpus/fetch.mjs'
/** The point-and-click links of a compile's pages. */
const linksOf = (svgs) => svgs.reduce((n, svg) => n + (svg.match(/xlink:href="textedit:/g) ?? []).length, 0)

test("the 30 in one core.wasm instance (C2.2, MS4 step 14): T21's, T23's and T25's point-and-click #f end with each, as lilypond-all's ly:reset-options ends a file's options; every score's links and stderr its own compile's",
  { skip: cargo || skipPacks || skipThirty || coreSkip(), timeout: 900_000 }, async () => {
    const alone = {}
    for (const row of thirty) {
      const out = mkdtempSync(path.join(scratch, `alone-${row.id}-`))
      runExample('compile', [...packs.flatMap(([, f]) => ['--pack', f]), '--out', out, row.path], { cwd: path.dirname(row.path), encoding: 'utf8' })
      const svgs = readdirSync(out).filter((f) => f.endsWith('.svg')).map((f) => readFileSync(path.join(out, f), 'utf8'))
      alone[row.id] = { links: linksOf(svgs), stderr: readFileSync(path.join(out, `${path.basename(row.path, '.ly')}.err`), 'utf8') }
    }
    // The three that set it have none of their own; the scores after them, laid out, have theirs.
    assert.deepEqual(['T21', 'T23', 'T25'].map((id) => alone[id].links), [0, 0, 0])
    assert.ok(['T24', 'T26', 'T27', 'T28', 'T29', 'T30'].every((id) => alone[id].links > 0))
    const wasm = await compileInWasm(thirty.map((row) => row.path), { modules: packModules() })
    for (const [i, row] of thirty.entries()) {
      const { outputs } = wasm[i]
      const svgs = Object.entries(outputs).filter(([f]) => f.endsWith('.svg')).map(([, b]) => b.toString())
      assert.deepEqual({ links: linksOf(svgs), stderr: outputs.stderr.toString() }, alone[row.id], row.id)
    }
  })

/** fixtures/lyric-grobs.ly's row (MS4 step 15). */
const lyricGrobsFixture = { id: 'lyric-grobs', file: 'tools/compare/fixtures/lyric-grobs.ly', path: fileURLToPath(new URL('../fixtures/lyric-grobs.ly', import.meta.url)) }
/**
 * The scores MS4 step 15 lays out whole with their lyrics (census.txt), with their files' stems; T02 is the page
 * breaker's test's, whole since. T20's text is in a font the capture machine's fontconfig chose (D224, D227), so it is
 * grobs.test.mjs' alone till step 19.
 */
const LYRICS = { T04: 'lyric-combine', T23: 'aucun-snippet', T30: 'sesto-full' }
const lyricScores = Object.keys(LYRICS).map((id) => corpus().find((row) => row.id === id))
const skipLyrics = lyricScores.every((row) => row && existsSync(row.path) && existsSync(path.join(references, row.id, LAYOUT)))
  ? false : `no layout.tsv in ${references}; run fetch.mjs and capture.mjs`

test("fixtures/lyric-grobs.ly, T04, T23 and T30 natively and in wasm, their lyrics laid out: layout.tsv LilyPond's on every field, the pages' C3 and C4 with every Δ 0, the stderr LilyPond's (MS4 step 15)",
  { skip: lilypond || cargo || skipPacks || skipLyrics, timeout: 900_000 }, async () => {
    const dir = path.join(scratch, 'lyric-grobs-laid-out')
    assert.equal((await captureOne('lilypond', lyricGrobsFixture, dir)).exit, 0)
    // A hyphen and an extender across the \break: the extender in a piece on each line, the hyphen's second killed.
    const extenders = readLayout(readFileSync(path.join(dir, LAYOUT), 'utf8')).filter((g) => g.type === 'LyricExtender')
    assert.equal(new Set(extenders.map((g) => g.system)).size, 2)
    const rows = [[lyricGrobsFixture.id, 'lyric-grobs', lyricGrobsFixture.path, dir],
      ...lyricScores.map((row) => [row.id, LYRICS[row.id], row.id === 'T23' ? withPointAndClick(row) : row.path, path.join(references, row.id)])]
    const natively = {}
    for (const [id, stem, file, ref] of rows) {
      const out = mkdtempSync(path.join(scratch, `engine-lyrics-${id}-`))
      const run = runExample('compile', ['--trace', ...packs.flatMap(([, f]) => ['--pack', f]), '--out', out, file],
        { cwd: path.dirname(file), encoding: 'utf8' })
      assert.equal(run.status, 0, `${id}: ${run.stderr}`)
      const files = Object.fromEntries(readdirSync(out).filter((f) => f.endsWith('.svg') || f.endsWith('.timing.json')).map((f) => [f, readFileSync(path.join(out, f))]))
      const layout = readFileSync(path.join(out, `${stem}.${LAYOUT}`), 'utf8')
      natively[id] = layout
      assertQuanted(id, ref, { layout, stderr: readFileSync(path.join(out, `${stem}.err`), 'utf8'), files }, 'natively')
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(rows.map(([, , file]) => file), { trace: true, modules: packModules() })
    for (const [i, [id, , , ref]] of rows.entries()) {
      const outputs = wasm[i].outputs
      const files = Object.fromEntries(Object.entries(outputs).filter(([f]) => f.endsWith('.svg') || f.endsWith('.timing.json')))
      assert.equal(outputs[LAYOUT]?.toString(), natively[id], id)
      assertQuanted(id, ref, { layout: outputs[LAYOUT].toString(), stderr: outputs.stderr.toString(), files }, 'in wasm')
    }
  })

/** fixtures/figures.ly's row (MS4 step 16). */
const figuresFixture = { id: 'figures', file: 'tools/compare/fixtures/figures.ly', path: fileURLToPath(new URL('../fixtures/figures.ly', import.meta.url)) }
/** The scores MS4 step 16 lays out whole with their figures (census.txt), with their files' stems. */
const FIGURES = { T06: 'figured-bass', T21: 'theory' }
const figureScores = Object.keys(FIGURES).map((id) => corpus().find((row) => row.id === id))
const skipFigures = figureScores.every((row) => row && existsSync(row.path) && existsSync(path.join(references, row.id, LAYOUT)))
  ? false : `no layout.tsv in ${references}; run fetch.mjs and capture.mjs`

test("fixtures/figures.ly, T06 and T21 natively and in wasm, their figures laid out: layout.tsv LilyPond's on every field, the pages' C3 and C4 with every Δ 0, the stderr LilyPond's (MS4 step 16)",
  { skip: lilypond || cargo || skipPacks || skipFigures, timeout: 900_000 }, async () => {
    const dir = path.join(scratch, 'figures-laid-out')
    assert.equal((await captureOne('lilypond', figuresFixture, dir)).exit, 0)
    // The 10 continuations in 12 pieces on the three systems: those across the first score's \break a piece on each
    // line.
    const continuations = readLayout(readFileSync(path.join(dir, LAYOUT), 'utf8')).filter((g) => g.type === 'BassFigureContinuation')
    assert.deepEqual([continuations.length, new Set(continuations.map((g) => g.system)).size], [12, 3])
    const rows = [[figuresFixture.id, 'figures', figuresFixture.path, dir],
      ...figureScores.map((row) => [row.id, FIGURES[row.id], row.id === 'T21' ? withPointAndClick(row) : row.path, path.join(references, row.id)])]
    const natively = {}
    for (const [id, stem, file, ref] of rows) {
      const out = mkdtempSync(path.join(scratch, `engine-figures-${id}-`))
      const run = runExample('compile', ['--trace', ...packs.flatMap(([, f]) => ['--pack', f]), '--out', out, file],
        { cwd: path.dirname(file), encoding: 'utf8' })
      assert.equal(run.status, 0, `${id}: ${run.stderr}`)
      const files = Object.fromEntries(readdirSync(out).filter((f) => f.endsWith('.svg') || f.endsWith('.timing.json')).map((f) => [f, readFileSync(path.join(out, f))]))
      const layout = readFileSync(path.join(out, `${stem}.${LAYOUT}`), 'utf8')
      natively[id] = layout
      assertQuanted(id, ref, { layout, stderr: readFileSync(path.join(out, `${stem}.err`), 'utf8'), files }, 'natively')
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(rows.map(([, , file]) => file), { trace: true, modules: packModules() })
    for (const [i, [id, , , ref]] of rows.entries()) {
      const outputs = wasm[i].outputs
      const files = Object.fromEntries(Object.entries(outputs).filter(([f]) => f.endsWith('.svg') || f.endsWith('.timing.json')))
      assert.equal(outputs[LAYOUT]?.toString(), natively[id], id)
      assertQuanted(id, ref, { layout: outputs[LAYOUT].toString(), stderr: outputs.stderr.toString(), files }, 'in wasm')
    }
  })

/** fixtures/tablature.ly's and fixtures/tab-note-heads.ly's rows (MS4 steps 17 and 7a). */
const tablatureFixtures = ['tablature', 'tab-note-heads']
  .map((id) => ({ id, file: `tools/compare/fixtures/${id}.ly`, path: fileURLToPath(new URL(`../fixtures/${id}.ly`, import.meta.url)) }))
const t22Row = corpus().find((row) => row.id === 'T22')
const skipT22Layout = t22Row && existsSync(t22Row.path) && existsSync(path.join(references, 'T22', LAYOUT))
  ? false : `no layout.tsv in ${references}; run fetch.mjs and capture.mjs`

test("fixtures/tablature.ly, fixtures/tab-note-heads.ly and T22 natively and in wasm, the tablature laid out (A46: the fretted part loaded before the layout, D228; ly:bezier-extent; A47: a markup's TabStaff): layout.tsv LilyPond's on every field, the pages' C3 and C4 with every Δ 0, the stderr LilyPond's (MS4 step 17)",
  { skip: lilypond || cargo || skipPacks || skipT22Layout, timeout: 900_000 }, async () => {
    const rows = []
    for (const fixture of tablatureFixtures) {
      const dir = path.join(scratch, `${fixture.id}-laid-out`)
      assert.equal((await captureOne('lilypond', fixture, dir)).exit, 0)
      rows.push([fixture.id, fixture.id, fixture.path, dir])
    }
    rows.push(['T22', 'tab-example', t22Row.path, path.join(references, 'T22')])
    const natively = {}
    for (const [id, stem, file, ref] of rows) {
      const out = mkdtempSync(path.join(scratch, `engine-tablature-${id}-`))
      const run = runExample('compile', ['--trace', ...packs.flatMap(([, f]) => ['--pack', f]), '--out', out, file],
        { cwd: path.dirname(file), encoding: 'utf8' })
      assert.equal(run.status, 0, `${id}: ${run.stderr}`)
      const files = Object.fromEntries(readdirSync(out).filter((f) => f.endsWith('.svg') || f.endsWith('.timing.json')).map((f) => [f, readFileSync(path.join(out, f))]))
      const layout = readFileSync(path.join(out, `${stem}.${LAYOUT}`), 'utf8')
      natively[id] = layout
      assertQuanted(id, ref, { layout, stderr: readFileSync(path.join(out, `${stem}.err`), 'utf8'), files }, 'natively')
    }
    if (coreSkip()) return
    const wasm = await compileInWasm(rows.map(([, , file]) => file), { trace: true, modules: packModules() })
    for (const [i, [id, , , ref]] of rows.entries()) {
      const outputs = wasm[i].outputs
      const files = Object.fromEntries(Object.entries(outputs).filter(([f]) => f.endsWith('.svg') || f.endsWith('.timing.json')))
      assert.equal(outputs[LAYOUT]?.toString(), natively[id], id)
      assertQuanted(id, ref, { layout: outputs[LAYOUT].toString(), stderr: outputs.stderr.toString(), files }, 'in wasm')
    }
  })
