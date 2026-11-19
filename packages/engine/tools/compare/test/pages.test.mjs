// The core's pages (MS3 step 35): a \layout compile's book written as
// <base>.svg by the core (Paper_book's pages, page.scm's make-page and
// page-stencil from the image, crates/svg per page), scored against
// LilyPond's page of the same file (progress.md, "How part 3's gate reads").
//
// - fixtures/path.ly, the layout path's fixture, natively (the compile
//   example's --out) and in core.wasm (core.mjs' compileInWasm, as
//   c1.test.mjs drives it), both with the build's preloaded packs: one
//   page, path.svg, the same bytes, and nothing said.
// - Its C3 and C4 (score.mjs) against LilyPond's page, captured with
//   capture.ly (D106's fonts): one page each, no break, its two links (the
//   time signatures') at LilyPond's translate; C4's Goal (0.1, 0.5)
//   recorded.
// - Its pixels (rsvg-convert -z 2 -b white, magick compare AE and PAE) and
//   B10 (its bytes over LilyPond's: Must ≤ 100 %, Goal ≤ 50 %), recorded,
//   not judged: part 3 gates neither. The engine compiles the file as the
//   studio does, its text in D106's LilyPond Serif since step 51 (B9,
//   D210), as LilyPond's page names it (until then the SVG backend's
//   serif, and the tagline's pixels differed); the page compiled with
//   D106's fonts set by a wrapper is recorded beside it.
// - Y1's five (T01, T03, T14, T16, T19), with the preloaded packs, natively
//   and in one core.wasm instance: a page each since step 40, in wasm
//   every output the native compile's, the page among them. Until then no
//   page, their layout stopping at a placeholder unreported (D130). Each
//   stopped at the LedgerLineSpanner's springs-and-rods,
//   ly:ledger-line-spanner::set-spacing-rods, until step 39, and since
//   then at the clef's stencil, ly:clef::print (step 40's), both in
//   System::pre_processing, found with a probe build that reported the
//   layout's error (progress.md, MS3 steps 35 and 39). Since step 40's
//   clefs T14 and T16 were laid out and broken, still without a page;
//   T01, T03 and T19 stopped at
//   ly:break-alignable-interface::self-align-callback (a BarNumber's
//   X-offset, output-lib.scm's
//   self-alignment-interface::self-aligned-on-breakable), and all five
//   then at ly:system-start-delimiter::print, Paper_book's.
// - The five's pages against LilyPond's in ref/<id>/ (capture.mjs, D106's
//   fonts), natively and in wasm (MS3 step 40, part 3's gate): C3 and C4
//   pass, a page each, no break, every link paired; C4's Goal (0.1, 0.5)
//   recorded. Since step 51 Y2's eight, the five and T10, T13 and T18, their
//   packs fetched as the engine asks (W5): T10's staff no longer 0.9908
//   lower (the text packs' outlines, E7) and T18's dynamics kerned (W6).
// - MS3's gate on the 10 (MS3 step 65): the eight, T05's two systems and
//   T17's two pages, natively and in one core.wasm instance, every output
//   of the one the other's: C3 and C4 pass on every page, every link
//   paired, C4's Goal recorded; each score's stderr LilyPond's bytes; W3's
//   pixels on every page PAE one grey level or less and B10 within its
//   Must (100 %), AE, B10 and the pixels with D106's fonts set by a
//   wrapper recorded. Until step 65 the eight's, with their pixels, B10
//   and stderr recorded, not judged.
// - Step 41's fixtures, fixtures/collisions.ly and fixtures/rest-collisions.ly,
//   step 42's fixtures/multi-measure-rests.ly, fixtures/clef-modifiers.ly
//   and fixtures/span-bars.ly, and step 43's fixtures/instrument-names.ly
//   (layout.test.mjs, which compares their grobs.tsv and layout.tsv), with
//   the preloaded packs, natively and in one core.wasm instance: every
//   output the native compile's, the page among them; their pages against
//   LilyPond's (captureOne, D106's fonts): C3 and C4 pass, a page each, no
//   break, every link paired, their stderr LilyPond's; C4's Goal recorded
//   beside the flags, note collisions, rest collisions, multi-measure
//   rests, clef modifiers, span bars and stubs and instrument names each
//   page holds.
// - fixtures/y1-unreached.ly (MS3 step 42's last session: broken into its
//   line since its session 4), natively and in one core.wasm instance,
//   with the preloaded packs, the four parts of emmentaler-brace fetched
//   as the engine asks for them (its PianoStaffs' braces; W5, MS3 step
//   50: before it the tests put them from the build's fonts/, and with the
//   preloaded packs alone its pages ended at the brace's font, unreported):
//   a page, every output the native compile's; offline no page, and W5's
//   diagnostic. Against
//   LilyPond's (captureOne, D106's fonts), its stderr LilyPond's, C3 and
//   C4 passing since MS4 step 15 (A10: the Lyrics' line and its five
//   LyricTexts, Lyric_engraver's): all 44 links paired, every Δ 0. Until
//   then C4 failed by the Lyrics alone, the four staves below their line
//   2.569 high. Y1's unreached grobs are layout.test.mjs'.
// - W5's check (06 §9, MS3 step 50): T06, T09, T22 and T29, the scores
//   of sizes other than 20, and T10 and T13, in fresh core.wasm instances
//   with the preloaded packs: the packs each asks for (T06 its figures'
//   Emmentaler-11 and -16 since MS4 step 16, none until then: E20), fetched
//   from the build's manifest at one stop, every one LilyPond's page draws
//   with; offline, the same asked for and none given, each font's
//   diagnostic and no page; natively the same stderr both ways.
// - A traced compile's pages are the untraced ones (MS3 step 44): path.ly,
//   T19 of the five and fixtures/span-bars.ly (staves the
//   VerticalAlignment's positioning-done moves), natively and in one
//   core.wasm instance. Until then the dump's reads computed the staves' Y
//   offsets before page layout, which then added its own to them, and a
//   traced page's staff lay 3.776 staff spaces lower (MS3 step 40's Open).
// - fixtures/titles.ly (MS3 step 63; its layout is layout.test.mjs'), with
//   the build's packs: its two pages natively and in one core.wasm
//   instance the same bytes, and their pixels against LilyPond's pages
//   (captureOne, D106's fonts) PAE one grey level or less, the titles,
//   markups and tagline among them; AE and B10 recorded.
// - fixtures/page-markers.ly (MS3 step 64; its layout is layout.test.mjs'),
//   with the build's packs: its two pages natively and in one core.wasm
//   instance the same bytes, and their pixels against LilyPond's PAE one
//   grey level or less, the \page-ref numbers in roman-lower among them;
//   AE and B10 recorded.
//
// Skipped, and so unverified, without LilyPond 2.26.0, cargo +stable or the
// build's preloaded packs (npm run build); core.wasm's also without a build
// as new as the sources; the pixels without Homebrew's rsvg-convert and
// magick; the 10 without the corpus (tools/corpus/fetch.mjs), and their
// scores also without its ref/ capture (capture.mjs).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { captureOne, LAYOUT, LAYOUT_FIXTURES, lilypondVersion } from '../capture.mjs'
import { compileInWasm, coreSkip, outlinePacks, preloaded } from '../core.mjs'
import { countTypes, readLayout } from '../layout.mjs'
import { compare, compareStderr } from '../score.mjs'

// The preloaded packs and every text face's outlines (D262): W5's packs are what these compiles ask for.
const packs = [...preloaded(), ...outlinePacks()]
const skip = lilypondVersion() !== '2.26.0' ? 'LilyPond 2.26.0 is not installed'
  : spawnSync('cargo', ['+stable', '--version']).status !== 0 ? 'cargo +stable is not installed'
    : packs.length ? false : 'no preloaded font packs: run npm run build'
const tools = ['rsvg-convert', 'magick'].filter((tool) => spawnSync(tool, ['--version']).status !== 0)
const skipPixels = skip || (tools.length ? `${tools.join(' and ')} not installed (Homebrew): the pixels are unverified` : false)
/** Y2's eight (MS3 step 51): Y1's five, then T10, T13 and T18. */
const EIGHT = ['T01', 'T03', 'T14', 'T16', 'T19', 'T10', 'T13', 'T18']
/** MS3's 10 (MS3 step 65): the eight, then T05's two systems and T17's two pages. */
const TEN = [...EIGHT, 'T05', 'T17']
const ten = TEN.map((id) => corpus().find((row) => row.id === id)).filter(Boolean)
const skipTen = skip || (ten.length === TEN.length && ten.every((row) => existsSync(row.path)) ? false
  : 'the corpus is not fetched: run node tools/corpus/fetch.mjs')
const references = path.join(corpusDir(), 'ref')
const skipScores = skipTen || (TEN.every((id) => existsSync(path.join(references, id, 'capture.json'))) ? false
  : "the 10's ref/ capture is missing: run node tools/compare/capture.mjs")

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-pages-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

const pathLy = LAYOUT_FIXTURES.find((row) => row.id === 'path').path
const packFlags = packs.flatMap(([, file]) => ['--pack', file])
const modules = () => packs.map(([name, file]) => [name, readFileSync(file)])

/** Compiles `file` natively with the packs and `more` (more flags) into a directory of its own: its outputs by name and the exit. */
function compileNative(file, more = []) {
  const out = mkdtempSync(path.join(scratch, `engine-${path.basename(file, '.ly')}-`))
  const run = runExample('compile', [...packFlags, ...more, '--out', out, file], { cwd: path.dirname(file), encoding: 'utf8' })
  return { out, status: run.status, outputs: Object.fromEntries(readdirSync(out).map((name) => [name, readFileSync(path.join(out, name))])) }
}

/** path.ly's outputs natively, made once. */
let native
const nativePath = () => native ??= compileNative(pathLy)

/** LilyPond's capture of path.ly (its page path.svg among it), made once. */
let reference
const capturePath = () => reference ??= (async () => {
  const dir = path.join(scratch, 'lilypond')
  const result = await captureOne('lilypond', LAYOUT_FIXTURES.find((row) => row.id === 'path'), dir)
  assert.equal(result.exit, 0)
  assert.equal(result.pages, 1)
  return dir
})()

test("path.ly's page natively, with the preloaded packs: one page, path.svg, and nothing said", { skip, timeout: 300_000 }, () => {
  const { status, outputs } = nativePath()
  assert.equal(status, 0)
  assert.deepEqual(Object.keys(outputs).sort(), ['path.err', 'path.json', 'path.midi', 'path.svg', 'path.timing.json'])
  assert.equal(outputs['path.err'].toString(), '')
})

test("path.ly's page in core.wasm, with the preloaded packs: every output the native compile's, path.svg among them",
  { skip: skip || coreSkip(), timeout: 300_000 }, async () => {
    const { outputs } = nativePath()
    const [wasm] = await compileInWasm([pathLy], { modules: modules() })
    assert.deepEqual(Object.keys(wasm.outputs), ['stderr', 'diagnostics.json', 'path.svg', 'path.midi', 'path.timing.json'])
    assert.deepEqual(wasm.outputs.stderr, outputs['path.err'])
    assert.deepEqual(wasm.outputs['diagnostics.json'], outputs['path.json'])
    assert.deepEqual(wasm.outputs['path.svg'], outputs['path.svg'])
    assert.deepEqual(wasm.outputs['path.timing.json'], outputs['path.timing.json'])
  })

test("path.ly's page against LilyPond's: C3 and C4 pass, its two links at LilyPond's translate",
  { skip, timeout: 300_000 }, async (t) => {
    const ref = await capturePath()
    const cand = path.join(scratch, 'candidate')
    mkdirSync(cand, { recursive: true })
    for (const name of ['path.svg', 'path.timing.json']) writeFileSync(path.join(cand, name), nativePath().outputs[name])
    const result = compare(ref, cand)
    assert.deepEqual(result.pages, [1, 1])
    assert.equal(result.breaks, 1)
    assert.deepEqual(result.linksMatched, [2, 2])
    assert.deepEqual([result.pass.C3, result.pass.C4], [true, true])
    const goal = result.dxMedian <= 0.1 && result.dxP95 <= 0.5 && result.dyMedian <= 0.1 && result.dyP95 <= 0.5
    t.diagnostic(`C4 |Δx| median ${result.dxMedian} p95 ${result.dxP95}, |Δy| median ${result.dyMedian} p95 ${result.dyP95}` +
      ` staff spaces (Must 0.5, 2; Goal 0.1, 0.5: ${goal ? 'met' : 'missed'})`)
  })

/** `from`'s pixels as 06 Appendix A takes them, `<from>.png`. */
function png(from) {
  const to = `${from}.png`
  const run = spawnSync('rsvg-convert', ['-z', '2', '-b', 'white', from, '-o', to], { encoding: 'utf8' })
  assert.equal(run.status, 0, `rsvg-convert ${from}: ${run.stderr}`)
  return to
}

/** magick compare's metric `name` of two PNGs: `<absolute> (<normalized>)` on stderr; exit 1 when they differ. */
function metric(name, a, b) {
  const run = spawnSync('magick', ['compare', '-metric', name, a, b, 'null:'], { encoding: 'utf8' })
  assert.ok(run.status === 0 || run.status === 1, `magick compare: ${run.stderr}`)
  const [, absolute, normalized] = run.stderr.match(/^([\d.e+-]+) \(([\d.e+-]+)\)/)
  return { absolute: +absolute, normalized: +normalized }
}

/** The page `svg`'s pixels against LilyPond's page `ref`: AE and PAE. */
function pixels(ref, svg) {
  const [a, b] = [png(ref), png(svg)]
  return `AE ${metric('AE', a, b).absolute} pixels, PAE ${(255 * metric('PAE', a, b).normalized).toFixed(3)} grey levels`
}

/** The page `svg`'s bytes against LilyPond's page `ref`: B10. */
function b10(ref, svg) {
  const [bytes, lilypond] = [readFileSync(svg).length, readFileSync(ref).length]
  return `${bytes} bytes, LilyPond's ${lilypond}: B10 ${(100 * bytes / lilypond).toFixed(1)} % (Must ≤ 100, Goal ≤ 50)`
}

/** `file`'s page compiled natively with D106's fonts, which capture.ly sets, through a wrapper `<stem>-d106.ly`, and its exit. */
function compileD106(file) {
  const stem = path.basename(file, '.ly')
  const dir = mkdtempSync(path.join(scratch, `d106-${stem}-`))
  const wrapper = path.join(dir, `${stem}-d106.ly`)
  writeFileSync(wrapper, `\\version "2.26.0"\n\\paper {\n  property-defaults.fonts.serif = "LilyPond Serif"\n` +
    `  property-defaults.fonts.sans = "LilyPond Sans Serif"\n  property-defaults.fonts.typewriter = "LilyPond Monospace"\n}\n` +
    `\\include "${file}"\n`)
  const { status, out } = compileNative(wrapper)
  return { status, page: path.join(out, `${stem}-d106.svg`) }
}

test("path.ly's page: its pixels against LilyPond's and B10, recorded", { skip: skipPixels, timeout: 300_000 }, async (t) => {
  const ref = path.join(await capturePath(), 'path.svg')
  const page = path.join(nativePath().out, 'path.svg')
  t.diagnostic(`path.svg: ${pixels(ref, page)} (its text in LilyPond Serif, D210)`)
  const d106 = compileD106(pathLy)
  assert.equal(d106.status, 0)
  t.diagnostic(`path.svg with D106's fonts: ${pixels(ref, d106.page)}`)
  t.diagnostic(`path.svg: ${b10(ref, page)}`)
})

/** The 10's outputs natively with the packs, the rest fetched (W5), made once: by ID, the exit and the outputs by name. */
let compiledTen
const nativeTen = () => compiledTen ??= Object.fromEntries(ten.map((row) => [row.id, compileNative(row.path)]))
/** A score's pages as the compile example names them: `<stem>.svg`, or `<stem>-<n>.svg` each when there are more. */
const pagesOf = (stem, outputs) => Object.keys(outputs).filter((name) => name === `${stem}.svg` || new RegExp(`^${stem}-\\d+\\.svg$`).test(name)).sort()
/** A score's MIDI files as the compile example names them (MS5 step 1): `<stem>.midi`, then `<stem>-<n>.midi`, in order. */
const midisOf = (stem, outputs) => Object.keys(outputs).filter((name) => name === `${stem}.midi` || new RegExp(`^${stem}-\\d+\\.midi$`).test(name))
  .sort((a, b) => (+(a.match(/-(\d+)\.midi$/)?.[1] ?? 0)) - (+(b.match(/-(\d+)\.midi$/)?.[1] ?? 0)))
/**
 * A score's outputs: core.wasm's names to the compile example's files, its pages (by default its one) and its MIDI
 * files (by default its one) among them.
 */
const names = (stem, pages = [`${stem}.svg`], midis = [`${stem}.midi`]) => ({
  stderr: `${stem}.err`, 'diagnostics.json': `${stem}.json`, ...Object.fromEntries(pages.map((page) => [page, page])),
  ...Object.fromEntries(midis.map((midi) => [midi, midi])), [`${stem}.timing.json`]: `${stem}.timing.json`,
})
/** LilyPond's pages of a score, by ref/<id>/'s capture. */
const PAGES = { T17: 2 }

test("MS3's 10 natively, with the preloaded packs and the others fetched: LilyPond's pages each", { skip: skipTen, timeout: 600_000 }, () => {
  for (const row of ten) {
    const { status, outputs } = nativeTen()[row.id]
    const stem = path.basename(row.path, '.ly')
    // 1: the file failed, as T03 does in LilyPond.
    assert.equal(status, row.id === 'T03' ? 1 : 0, row.id)
    const pages = pagesOf(stem, outputs)
    assert.equal(pages.length, PAGES[row.id] ?? 1, row.id)
    assert.deepEqual(Object.keys(outputs).sort(), Object.values(names(stem, pages, midisOf(stem, outputs))).sort(), row.id)
  }
})

/** The 10 compiled in one core.wasm instance with the packs, the rest fetched, made once: core.mjs' results, in the 10's order. */
let compiledWasm
const wasmTen = () => compiledWasm ??= compileInWasm(ten.map((row) => row.path), { modules: modules() })

test("MS3's 10 in one core.wasm instance, with the preloaded packs and the others fetched: every output the native compile's, the pages among them",
  { skip: skipTen || coreSkip(), timeout: 600_000 }, async () => {
    const wasm = await wasmTen()
    for (const [i, row] of ten.entries()) {
      const { outputs } = nativeTen()[row.id]
      const stem = path.basename(row.path, '.ly')
      const files = names(stem, pagesOf(stem, outputs), midisOf(stem, outputs))
      assert.deepEqual(Object.keys(wasm[i].outputs).sort(), Object.keys(files).sort(), row.id)
      for (const [name, file] of Object.entries(files)) assert.deepEqual(wasm[i].outputs[name], outputs[file], `${row.id} ${name}`)
    }
  })

/**
 * A score's pages scored against LilyPond's capture in `ref` (one of the 10's ref/<id>/): `outputs` by the compile example's
 * names, its pages and timing among them.
 */
function scorePage(row, outputs, label, ref = path.join(references, row.id)) {
  const stem = path.basename(row.path, '.ly')
  const cand = mkdtempSync(path.join(scratch, `${label}-${row.id}-`))
  for (const name of [...pagesOf(stem, outputs), `${stem}.timing.json`]) writeFileSync(path.join(cand, name), outputs[name])
  return compare(ref, cand)
}

/** Asserts a score's C3 and C4 over its pages and records C4 against its Goal. */
function gate(t, row, result, label) {
  const pages = PAGES[row.id] ?? 1
  assert.deepEqual(result.pages, [pages, pages], row.id)
  assert.equal(result.breaks, 1, row.id)
  assert.equal(result.linksMatched[0], result.linksMatched[1], `${row.id}: every link paired`)
  assert.deepEqual([result.pass.C3, result.pass.C4], [true, true], row.id)
  const goal = result.dxMedian <= 0.1 && result.dxP95 <= 0.5 && result.dyMedian <= 0.1 && result.dyP95 <= 0.5
  t.diagnostic(`${row.id} ${label}: C3 pages ${result.pages.join(' / ')}; C4 ${result.linksMatched[0]} links,` +
    ` |Δx| median ${result.dxMedian} p95 ${result.dxP95}, |Δy| median ${result.dyMedian} p95 ${result.dyP95}` +
    ` staff spaces (Must 0.5, 2; Goal 0.1, 0.5: ${goal ? 'met' : 'missed'})`)
}

test("MS3's gate: the 10's pages natively against LilyPond's, C3 and C4 pass, C4's Goal recorded",
  { skip: skipScores, timeout: 600_000 }, (t) => {
    for (const row of ten) gate(t, row, scorePage(row, nativeTen()[row.id].outputs, 'native'), 'natively')
  })

test("MS3's gate: the 10's pages in core.wasm against LilyPond's, C3 and C4 pass, C4's Goal recorded",
  { skip: skipScores || coreSkip(), timeout: 600_000 }, async (t) => {
    const wasm = await wasmTen()
    for (const [i, row] of ten.entries()) gate(t, row, scorePage(row, wasm[i].outputs, 'wasm'), 'in wasm')
  })

test("MS3's gate: the 10's stderr LilyPond's bytes", { skip: skipScores, timeout: 600_000 }, (t) => {
  for (const row of ten) {
    const ref = readFileSync(path.join(references, row.id, 'stderr.txt'), 'utf8')
    const err = nativeTen()[row.id].outputs[`${path.basename(row.path, '.ly')}.err`].toString()
    const { errors, warnings, pass } = compareStderr(ref, err)
    t.diagnostic(`${row.id}: ${err === ref ? 'the same bytes as' : 'not the bytes of'} LilyPond's (${ref.length});` +
      ` errors ${errors.join(' / ')}, warnings ${warnings.join(' / ')}; C0 ${pass.C0 ? 'passes' : 'fails'}`)
    assert.equal(err, ref, row.id)
  }
})

// W3's pixels and B10 (MS3 step 65): PAE one grey level or less and B10 within its Must on every page, natively;
// core.wasm's pages are the same bytes (above). AE is recorded (OPEN E13: rsvg-convert rounds a rounded rect's
// corners apart when its translate is folded into its x and y, as the engine's compact pages write it).
test("MS3's gate: the 10's pages' pixels against LilyPond's (PAE ≤ 1) and B10 (Must ≤ 100 %), AE and D106's fonts recorded",
  { skip: skipScores || skipPixels, timeout: 600_000 }, (t) => {
    for (const row of ten) {
      const stem = path.basename(row.path, '.ly')
      const { out, outputs, status } = nativeTen()[row.id]
      const d106 = compileD106(row.path)
      assert.equal(d106.status, status, row.id)
      for (const name of pagesOf(stem, outputs)) {
        const ref = path.join(references, row.id, name)
        const page = path.join(out, name)
        const [a, b] = [png(ref), png(page)]
        const pae = 255 * metric('PAE', a, b).normalized
        const bytes = 100 * readFileSync(page).length / readFileSync(ref).length
        const withD106 = path.join(path.dirname(d106.page), name.replace(stem, `${stem}-d106`))
        t.diagnostic(`${row.id} ${name}: AE ${metric('AE', a, b).absolute} pixels, PAE ${pae.toFixed(3)} grey levels; with D106's fonts ${pixels(ref, withD106)}; ${b10(ref, page)}`)
        assert.ok(pae <= 1 + 1e-6, `${row.id} ${name}: PAE ${pae.toFixed(3)} grey levels`)
        assert.ok(bytes <= 100, `${row.id} ${name}: B10 ${bytes.toFixed(1)} %`)
      }
    }
  })

/** Steps 41 and 42's fixtures, laid out whole: rows as capture.mjs takes them. */
const FIXTURES = ['collisions', 'rest-collisions', 'multi-measure-rests', 'clef-modifiers', 'span-bars', 'instrument-names']
  .map((id) => ({ id, file: `tools/compare/fixtures/${id}.ly`, path: path.join(path.dirname(pathLy), `${id}.ly`) }))
/** The types steps 41 to 43 place, counted on each fixture's page. */
const PLACED = [
  'Flag', 'NoteCollision', 'RestCollision',
  'MultiMeasureRest', 'MultiMeasureRestNumber', 'MultiMeasureRestText', 'MultiMeasureRestScript', 'ClefModifier',
  'SpanBar', 'SpanBarStub', 'InstrumentName',
]

/** The fixtures' outputs natively with the packs, made once: by ID, the exit and the outputs by name. */
let compiledFixtures
const nativeFixtures = () => compiledFixtures ??= Object.fromEntries(FIXTURES.map((row) => [row.id, compileNative(row.path)]))
/** The fixtures compiled in one core.wasm instance with the packs, made once: core.mjs' results, in FIXTURES' order. */
let fixturesInWasm
const wasmFixtures = () => fixturesInWasm ??= compileInWasm(FIXTURES.map((row) => row.path), { modules: modules() })
/** LilyPond's captures of the fixtures, made once: by ID, the capture's directory. */
let fixtureCaptures
const captureFixtures = () => fixtureCaptures ??= (async () => {
  const dirs = {}
  for (const row of FIXTURES) {
    const dir = path.join(scratch, `lilypond-${row.id}`)
    const result = await captureOne('lilypond', row, dir)
    assert.equal(result.exit, 0, row.id)
    assert.equal(result.pages, 1, row.id)
    dirs[row.id] = dir
  }
  return dirs
})()

test("steps 41 to 43's fixtures natively and in one core.wasm instance, with the preloaded packs: a page each, every output the native compile's",
  { skip: skip || coreSkip(), timeout: 600_000 }, async () => {
    const wasm = await wasmFixtures()
    for (const [i, row] of FIXTURES.entries()) {
      const { status, outputs } = nativeFixtures()[row.id]
      const files = names(row.id, undefined, midisOf(row.id, outputs))
      assert.equal(status, 0, row.id)
      assert.deepEqual(Object.keys(outputs).sort(), Object.values(files).sort(), row.id)
      assert.deepEqual(Object.keys(wasm[i].outputs), Object.keys(files), row.id)
      for (const [name, file] of Object.entries(files)) assert.deepEqual(wasm[i].outputs[name], outputs[file], `${row.id} ${name}`)
    }
  })

test("steps 41 to 43's fixtures' pages natively and in core.wasm against LilyPond's: C3 and C4 pass, C4's Goal recorded beside the flags and collisions",
  { skip: skip || coreSkip(), timeout: 600_000 }, async (t) => {
    const refs = await captureFixtures()
    const wasm = await wasmFixtures()
    for (const [i, row] of FIXTURES.entries()) {
      const ref = refs[row.id]
      const types = countTypes(readLayout(readFileSync(path.join(ref, LAYOUT), 'utf8')))
      const placed = PLACED.map((type) => `${types.get(type) ?? 0} ${type}`).join(', ')
      const { outputs } = nativeFixtures()[row.id]
      assert.equal(outputs[`${row.id}.err`].toString(), readFileSync(path.join(ref, 'stderr.txt'), 'utf8'), row.id)
      for (const [label, files] of [['natively', outputs], ['in wasm', {
        [`${row.id}.svg`]: wasm[i].outputs[`${row.id}.svg`], [`${row.id}.timing.json`]: wasm[i].outputs[`${row.id}.timing.json`],
      }]]) {
        const result = scorePage(row, files, label.replace(' ', '-'), ref)
        gate(t, row, result, `${label} (${placed})`)
      }
    }
  })

test("steps 41 to 43's fixtures' pages: their pixels against LilyPond's and B10, recorded (E13)",
  { skip: skipPixels, timeout: 600_000 }, async (t) => {
    const refs = await captureFixtures()
    for (const row of FIXTURES) {
      const ref = path.join(refs[row.id], `${row.id}.svg`)
      const page = path.join(nativeFixtures()[row.id].out, `${row.id}.svg`)
      t.diagnostic(`${row.id}.svg: ${pixels(ref, page)}; ${b10(ref, page)}`)
    }
  })

/** fixtures/y1-unreached.ly, broken into its line since step 42: its row, as capture.mjs takes one. */
const y1 = LAYOUT_FIXTURES.find((row) => row.id === 'y1-unreached')
/** The engine's error for a font whose pack the host could not fetch (W5, D208). */
const unfetched = (font, pack) => `error: cannot find font '${font}' (its font pack ${pack} could not be fetched)\n`
/** The four parts of emmentaler-brace, which the engine asks for in order. */
const BRACES = [0, 1, 2, 3].map((i) => `emmentaler-brace-${i}`)

/** y1-unreached's outputs natively with the preloaded packs, the brace's fetched, made once. */
let compiledY1
const nativeY1 = () => compiledY1 ??= compileNative(y1.path)
/** y1-unreached compiled in a core.wasm instance with the preloaded packs, made once: core.mjs' result. */
let y1InWasm
const wasmY1 = () => y1InWasm ??= compileInWasm([y1.path], { modules: modules() }).then(([result]) => result)

test("y1-unreached.ly's page natively and in one core.wasm instance, with the preloaded packs and the brace's fetched: every output the native compile's; offline, no page and W5's diagnostic",
  { skip: skip || coreSkip(), timeout: 300_000 }, async () => {
    const { status, outputs } = nativeY1()
    const files = names('y1-unreached')
    assert.equal(status, 0)
    assert.deepEqual(Object.keys(outputs).sort(), Object.values(files).sort())
    const wasm = await wasmY1()
    assert.deepEqual(wasm.packs, BRACES.map((name) => [name, true]))
    assert.deepEqual(Object.keys(wasm.outputs), Object.keys(files))
    for (const [name, file] of Object.entries(files)) assert.deepEqual(wasm.outputs[name], outputs[file], name)
    // Offline the pages end at the brace's font, as they did before W5
    // with the preloaded packs alone, and the compile says why.
    const alone = compileNative(y1.path, ['--offline'])
    assert.equal(alone.status, 0)
    assert.deepEqual(Object.keys(alone.outputs).sort(), ['y1-unreached.err', 'y1-unreached.json', 'y1-unreached.midi', 'y1-unreached.timing.json'])
    assert.equal(alone.outputs['y1-unreached.err'].toString(), unfetched('emmentaler-brace', BRACES[0]))
  })

/** W5's check (06 §9): each score with the packs it asks for, which LilyPond's page draws with too. */
const W5 = {
  T06: ['emmentaler-11', 'emmentaler-16'], T09: ['emmentaler-14', 'emmentaler-13'], T22: ['emmentaler-11', 'emmentaler-14', 'emmentaler-13', 'NimbusSans-Bold'],
  T29: ['emmentaler-11', ...BRACES, 'C059-BdIta'], T10: ['emmentaler-13'], T13: BRACES,
}
/**
 * The scores that meet fonts while they are translated, each a stop of its own, where
 * offline the first font refused ends the compile: T22's string numbers'
 * heads, whose staff-position New_fingering_engraver reads (MS3 step 53), need
 * emmentaler-11 and then emmentaler-14 before its layout wants emmentaler-13;
 * since MS4 step 17, its layout drawn (A46, D228), its tab clef's `TAB` wants
 * NimbusSans-Bold after them (E22).
 */
const W5_TRANSLATED = { T22: { stops: 4, offline: ['emmentaler-11'] } }
/**
 * The scores whose layout meets a font after the packs W5 predicts, a stop of its own (OPEN E22: the prediction
 * skips text grobs, whose font is their markup's), where offline the first stop refused ends the compile before
 * the later one is asked: T29's bold italic text (since MS4 step 10, its ties drawn) after its music fonts, and
 * T06's Emmentaler-16, a figure's `+` sign (since MS4 step 16, its figures made: E20), after its figures'
 * Emmentaler-11.
 */
const W5_LATER = { T29: { stops: 2, offline: ['emmentaler-11', ...BRACES] }, T06: { stops: 2, offline: ['emmentaler-11'] } }
const w5 = corpus().filter((row) => row.id in W5)
const skipW5 = skip || coreSkip() || (w5.length === 6 && w5.every((row) => existsSync(row.path)) ? false : 'the corpus is not fetched: run node tools/corpus/fetch.mjs')
/** The fonts of `packs`, each once: the brace's parts are emmentaler-brace's. */
const fontsOf = (packs) => [...new Set(packs.map((p) => (BRACES.includes(p) ? 'emmentaler-brace' : p)))]
/** S6's parts the core asks for as modules since MS4 step 20 (I2: the core's PART_MODULES; all seven since MS7 step 7, D263), each a stop as its files were. */
const PART_MODULES = ['chords', 'display', 'fretted', 'diagrams', 'partcombine', 'modal', 'qr']
const isPart = ([name]) => PART_MODULES.includes(name)
/** The diagnostics offline: one a font, its first pack named. */
const offlineSaid = (packs) => fontsOf(packs).map((font) => unfetched(font, packs.find((p) => p.startsWith(font))))

test("W5's check: T06, T09, T22, T29, T10 and T13 ask for their packs, fetched at one stop; offline the same, each font's diagnostic and no page; natively the same stderr",
  { skip: skipW5, timeout: 600_000 }, async (t) => {
    for (const row of w5) {
      const packs = W5[row.id]
      const translated = W5_TRANSLATED[row.id]
      const later = W5_LATER[row.id]
      const refused = translated?.offline ?? later?.offline ?? packs
      const [online] = await compileInWasm([row.path], { modules: modules() })
      const [offline] = await compileInWasm([row.path], { modules: modules(), offline: true })
      assert.deepEqual(online.packs.filter((p) => !isPart(p)), packs.map((name) => [name, true]), row.id)
      assert.deepEqual(offline.packs.filter((p) => !isPart(p)), refused.map((name) => [name, false]), row.id)
      // A parse, one more a stop for files or a part, and one for all the packs (or
      // one a stop where they are met as it is translated; offline one).
      const stops = (r) => r.stops + r.packs.filter(isPart).length
      assert.equal(online.parses, 1 + stops(online) + (packs.length ? (translated?.stops ?? later?.stops ?? 1) : 0), row.id)
      assert.equal(offline.parses, 1 + stops(offline) + (packs.length ? 1 : 0), row.id)
      // Every font it asks for is one LilyPond's page draws with.
      const drawn = readFileSync(path.join(references, row.id, 'pages.scm'), 'utf8')
      for (const font of fontsOf(packs)) assert.ok(drawn.includes(font), `${row.id}: ${font}`)
      const said = (r) => r.outputs.stderr.toString().split('\n').filter((l) => l.includes('could not be fetched')).map((l) => `${l}\n`)
      assert.deepEqual(said(online), [], row.id)
      assert.deepEqual(said(offline), offlineSaid(refused), row.id)
      const pages = (r) => Object.keys(r.outputs).filter((n) => n.endsWith('.svg')).length
      if (packs.length) assert.equal(pages(offline), 0, row.id)
      for (const [label, result, more] of [['online', online, []], ['offline', offline, ['--offline']]]) {
        const native = compileNative(row.path, more)
        assert.deepEqual(result.outputs.stderr, native.outputs[`${path.basename(row.path, '.ly')}.err`], `${row.id} ${label}`)
      }
      t.diagnostic(`${row.id}: ${packs.join(', ') || 'no pack'}; ${online.parses} parses; pages online ${pages(online)}, offline ${pages(offline)}`)
    }
  })

test("y1-unreached.ly's page natively and in core.wasm against LilyPond's: C3 and C4 pass, the Lyrics' line drawn (A10, MS4 step 15), every link paired with every Δ 0, Y1's unreached grobs recorded",
  { skip: skip || coreSkip(), timeout: 300_000 }, async (t) => {
    const ref = path.join(scratch, 'lilypond-y1-unreached')
    const captured = await captureOne('lilypond', y1, ref)
    assert.deepEqual([captured.exit, captured.pages], [0, 1])
    const { outputs } = nativeY1()
    assert.equal(outputs['y1-unreached.err'].toString(), readFileSync(path.join(ref, 'stderr.txt'), 'utf8'))
    const wasm = await wasmY1()
    for (const [label, files] of [['natively', outputs], ['in wasm', {
      'y1-unreached.svg': wasm.outputs['y1-unreached.svg'], 'y1-unreached.timing.json': wasm.outputs['y1-unreached.timing.json'],
    }]]) {
      const result = scorePage(y1, files, label.replace(' ', '-'), ref)
      assert.deepEqual([result.pages, result.breaks, result.pass.C3, result.pass.C4], [[1, 1], 1, true, true], label)
      // LilyPond's 44 links paired, the LyricTexts' five (line 27) among
      // them since Lyric_engraver is the engine's (A10).
      assert.deepEqual(result.linksMatched, [44, 44], label)
      assert.deepEqual([result.dxMedian, result.dxP95, result.dyMedian, result.dyP95], [0, 0, 0, 0], label)
      t.diagnostic(`y1-unreached ${label}: C3 pages ${result.pages.join(' / ')}; C4 ${result.linksMatched.join(' of ')} links,` +
        ` |Δx| median ${result.dxMedian} p95 ${result.dxP95}, |Δy| median ${result.dyMedian} p95 ${result.dyP95} staff spaces`)
    }
    const types = countTypes(readLayout(readFileSync(path.join(ref, LAYOUT), 'utf8')))
    t.diagnostic(`y1-unreached: LilyPond's ${[...types.values()].reduce((s, n) => s + n)} live grobs; ` +
      `${PLACED.map((type) => `${types.get(type) ?? 0} ${type}`).join(', ')}`)
  })

test("a traced compile's pages are the untraced ones: path.ly, T19 and span-bars.ly, natively and in one core.wasm instance",
  { skip: skipTen || coreSkip(), timeout: 600_000 }, async () => {
    const rows = [
      [pathLy, () => nativePath()],
      [ten.find((row) => row.id === 'T19').path, () => nativeTen().T19],
      [FIXTURES.find((row) => row.id === 'span-bars').path, () => nativeFixtures()['span-bars']],
    ]
    const wasm = await compileInWasm(rows.map(([file]) => file), { trace: true, modules: modules() })
    for (const [i, [file, untraced]] of rows.entries()) {
      const page = `${path.basename(file, '.ly')}.svg`
      const { outputs } = untraced()
      const traced = compileNative(file, ['--trace']).outputs
      assert.ok(traced[page.replace(/svg$/, 'layout.tsv')], `${page}: traced natively`)
      assert.ok(wasm[i].outputs[LAYOUT], `${page}: traced in core.wasm`)
      assert.deepEqual(traced[page], outputs[page], `${page} natively`)
      assert.deepEqual(wasm[i].outputs[page], outputs[page], `${page} in core.wasm`)
    }
  })

/** Every pack of the build's fonts/, by name, as layout.test.mjs gives them: the text faces the titles ask for among them. */
const everyPack = () => readdirSync(path.join(path.dirname(packs[0][1]), 'fonts')).filter((f) => f.endsWith('.pack'))
  .map((f) => [f.replace(/\.pack$/, ''), path.join(path.dirname(packs[0][1]), 'fonts', f)])

test("fixtures/titles.ly's two pages natively and in one core.wasm instance, with the build's packs: the same bytes, their titles' and markups' pixels LilyPond's (PAE ≤ 1, AE and B10 recorded; MS3 step 63)",
  { skip: skipPixels, timeout: 300_000 }, async (t) => {
    const row = LAYOUT_FIXTURES.find((r) => r.id === 'titles')
    const ref = path.join(scratch, 'titles-lilypond')
    const captured = await captureOne('lilypond', row, ref)
    assert.deepEqual([captured.exit, captured.pages], [0, 2])
    const given = everyPack()
    const { status, out, outputs } = compileNative(row.path, given.flatMap(([, file]) => ['--pack', file]))
    assert.equal(status, 0)
    const names = ['titles-1.svg', 'titles-2.svg']
    assert.deepEqual(Object.keys(outputs).filter((n) => n.endsWith('.svg')).sort(), names)
    if (!coreSkip()) {
      const [wasm] = await compileInWasm([row.path], { modules: given.map(([name, file]) => [name, readFileSync(file)]) })
      for (const name of names) assert.deepEqual(wasm.outputs[name], outputs[name], `${name} in core.wasm`)
    }
    for (const name of names) {
      const [a, b] = [png(path.join(ref, name)), png(path.join(out, name))]
      const pae = 255 * metric('PAE', a, b).normalized
      t.diagnostic(`${name}: AE ${metric('AE', a, b).absolute} pixels, PAE ${pae.toFixed(3)} grey levels; ` +
        b10(path.join(ref, name), path.join(out, name)))
      assert.ok(pae <= 1 + 1e-6, `${name}: PAE ${pae.toFixed(3)} grey levels`)
    }
  })

test("fixtures/page-markers.ly's two pages natively and in one core.wasm instance, with the build's packs: the same bytes, their page-refs' roman page numbers and links' pixels LilyPond's (PAE ≤ 1, AE and B10 recorded; MS3 step 64)",
  { skip: skipPixels, timeout: 300_000 }, async (t) => {
    const row = LAYOUT_FIXTURES.find((r) => r.id === 'page-markers')
    const ref = path.join(scratch, 'page-markers-lilypond')
    const captured = await captureOne('lilypond', row, ref)
    assert.deepEqual([captured.exit, captured.pages], [0, 2])
    const given = everyPack()
    const { status, out, outputs } = compileNative(row.path, given.flatMap(([, file]) => ['--pack', file]))
    assert.equal(status, 0)
    const names = ['page-markers-1.svg', 'page-markers-2.svg']
    assert.deepEqual(Object.keys(outputs).filter((n) => n.endsWith('.svg')).sort(), names)
    if (!coreSkip()) {
      const [wasm] = await compileInWasm([row.path], { modules: given.map(([name, file]) => [name, readFileSync(file)]) })
      for (const name of names) assert.deepEqual(wasm.outputs[name], outputs[name], `${name} in core.wasm`)
    }
    for (const name of names) {
      const [a, b] = [png(path.join(ref, name)), png(path.join(out, name))]
      const pae = 255 * metric('PAE', a, b).normalized
      t.diagnostic(`${name}: AE ${metric('AE', a, b).absolute} pixels, PAE ${pae.toFixed(3)} grey levels; ` +
        b10(path.join(ref, name), path.join(out, name)))
      assert.ok(pae <= 1 + 1e-6, `${name}: PAE ${pae.toFixed(3)} grey levels`)
    }
  })
