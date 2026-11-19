// MS6's differential harness (README §5, MS6; MS6 step 1): chapter 00 §6.2's
// E1–E3 on T02, T12, T24 and T29 and seeded typed edits of the 30
// (edits.mjs), each compiled incrementally, in an engine that compiled the
// text before the edit, and from scratch, and compared layer by layer C0–C5
// (differential.mjs). Until a layer is incremental both compiles are from
// scratch, and every edit's two must be equal byte for byte, but KNOWN's.
// Natively (the compile example in release) E1–E3 and COUNT edits of each
// score; in one core.wasm instance against an instance each, E1–E3 and
// WASM_COUNT of each, the big four's ten seconds an edit there. The larger
// seeded run of each part's close and MS6's gate:
//
//   LILY_DIFFERENTIAL=<edits a score> [LILY_DIFFERENTIAL_SEED=<n>] npm run test:tools
//
// And the harness sees a planted difference in each layer: one diagnostic
// changed (C0), one event dropped (C1), one MIDI byte (C2), a page dropped
// (C3), one grob moved (C4), one shape (C5). The tests skip themselves, and
// so verify nothing, without LilyPond 2.26.0 (whose scm/lily the core loads),
// cargo, the corpus and its capture, and the wasm ones without a core.wasm
// as new as the sources.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, describe, test } from 'node:test'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { lilypondVersion } from '../capture.mjs'
import { coreSkip } from '../core.mjs'
import { LAYERS, differ, differences, editsOf, layerOf, nativeDifferential, readOutputs, replacedPages, wasmDifferential } from '../differential.mjs'
import { EDITED, KINDS, THIRTY, TYPED, e1e3, population, seeded, typing } from '../edits.mjs'

const rows = Object.fromEntries(corpus().map((row) => [row.id, row]))
const ref = path.join(corpusDir(), 'ref')
const skip = lilypondVersion() !== '2.26.0' ? 'LilyPond 2.26.0 is not installed'
  : spawnSync('cargo', ['+stable', '--version']).status !== 0 ? 'cargo +stable is not installed'
    : !THIRTY.every((id) => existsSync(rows[id].path) && existsSync(path.join(ref, id))) ? 'the corpus or its capture is not there'
      : false
const scratch = mkdtempSync(path.join(tmpdir(), 'lily-differential-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** Seeded edits of each score natively, and in wasm; the larger run's from LILY_DIFFERENTIAL. */
const larger = Number(process.env.LILY_DIFFERENTIAL) || 0
const COUNT = larger || 10
const WASM_COUNT = larger || 3
const SEED = Number(process.env.LILY_DIFFERENTIAL_SEED) || 1

/**
 * The edits whose two compiles differ, by `<id> <name>` (the name holds the seed), with the
 * layers that differ and why: each is a from-scratch compile's, and is
 * asserted to differ until the step that owns it makes it equal.
 */
const KNOWN = {
  // None: T27's string-1-5 went at MS6 step 2, T24's string-2-19 (OPEN C9) at
  // step 12.
}

/**
 * Edits whose N8 digests whole runs contradict, by `<id> <name>`, with what
 * they contradict: none since MS6 step 5's rest (D240). T09's `delete-1-3`
 * and T12's `insert-1-10` (OPEN C10) hold in every run since step 6; T04's
 * `delete-1-9` (C12) holds since the shared level's writes in a timestep
 * are its staves' input (`forbidBreak`, set and unset at 1/1).
 */
const N8_KNOWN = {
  // Of the larger run, seed 2 (runway/OPEN.md C15): each compile equal to its
  // fresh one. The plan's figure: the `\midi`'s audio records at 0 of a staff
  // the edit did not touch. (T26's `insert-2-23` holds since a record's
  // interfaces are digested by name, MS8 step 1.)
  'T04 error-2-9': 'its records differ',
  // `m_e` late: `)` after a \grace block attaches inside it.
  'T09 delete-2-30': 'before m_e',
}

describe('the edits', { skip }, () => {
  test("E1–E3 on the four where $LILY_CORPUS/ms6-split/edits.txt finds them", () => {
    const found = Object.fromEntries(EDITED.map((id) => [id, e1e3(id).map((e) => `${e.name} ${e.line}:${e.char} ${JSON.stringify(e.insert)}`)]))
    assert.deepEqual(found, {
      T02: ['E1 17:2 "c"', 'E2 17:2 "b4 "', 'E3 7:17 "x"'],
      T12: ['E1 43:5 "a"', 'E2 43:5 "g "', 'E3 11:0 "x"'],
      // The middle event, 110:18, is a duration alone, `16`: the next with a note name.
      T24: ['E1 86:9 "a"', 'E2 86:9 "g "', 'E3 6:35 "x"'],
      // A chord's note: `bes,>-.` is its token, `b` raised.
      T29: ['E1 220:63 "c"', 'E2 220:63 "bes,>-. "', 'E3 5:38 "x"'],
    })
    // E3 appends to the \header's first string: before its closing quote.
    for (const id of EDITED) {
      const e3 = e1e3(id)[2]
      assert.equal(e3.source[e3.from + 1], '"', id)
    }
  })

  test('the population: note names and durations of the score’s own file, and its strings', () => {
    const counts = Object.fromEntries(['T02', 'T24', 'T29'].map((id) => {
      const p = population(id)
      return [id, [p.notes.length, p.durations.length, p.strings.length]]
    }))
    // edits.txt counts links (a chord's notes one each); these are distinct offsets.
    assert.deepEqual(counts, { T02: [26, 0, 7], T24: [426, 28, 41], T29: [385, 13, 110] })
    // T01's notes are in melody.ily, which the edits leave alone.
    assert.equal(population('T01').notes.length, 0)
  })

  test('seeded edits are found again from their seed, each changing the text, every kind among them', () => {
    const a = THIRTY.flatMap((id) => seeded(id, { seed: 1, count: 10 }))
    const b = THIRTY.flatMap((id) => seeded(id, { seed: 1, count: 10 }))
    assert.deepEqual(a.map((e) => [e.name, e.from, e.to, e.insert]), b.map((e) => [e.name, e.from, e.to, e.insert]))
    assert.equal(a.length, 300)
    assert.notDeepEqual(seeded('T24', { seed: 2, count: 10 }).map((e) => e.from), seeded('T24', { seed: 1, count: 10 }).map((e) => e.from))
    for (const e of a) {
      const text = readFileSync(e.file, 'utf8')
      assert.notEqual(e.source, text, `${e.id} ${e.name}`)
      assert.equal(e.source, text.slice(0, e.from) + e.insert + text.slice(e.to), `${e.id} ${e.name}`)
    }
    assert.deepEqual([...new Set(a.map((e) => e.kind))].sort(), [...KINDS].sort())
  })
})

/** The outputs of a native compile pair (T02's E1), to plant differences in. */
let planted
const pair = async () => {
  planted ??= nativeDifferential([e1e3('T02')[0]], path.join(scratch, 'planted')).then(([e]) => {
    const dir = path.join(scratch, 'planted')
    return { e, fresh: readOutputs(path.join(dir, 'fresh/1'), 'score'), incremental: readOutputs(path.join(dir, 'incremental/1'), 'score') }
  })
  return planted
}
/**
 * The layers `differ` finds between T02's fresh compile and its incremental
 * one with `plant` applied, and the comparator's figures of `layer`.
 */
async function caught(plant, layer) {
  const { fresh, incremental } = await pair()
  const copy = Object.fromEntries(Object.entries(incremental).map(([k, v]) => [k, Buffer.from(v)]))
  plant(copy)
  const r = differ(fresh, copy, { rootFile: rows.T02.path })
  return [LAYERS.filter((l) => !r[l].equal), r[layer].score]
}

describe('a planted difference is caught in each layer', { skip }, () => {
  test('the pair is equal as compiled, every output in a layer', async () => {
    const { e, fresh } = await pair()
    assert.equal(e.result.equal, true, JSON.stringify(differences(e.result)))
    assert.deepEqual(Object.keys(fresh).filter((name) => !layerOf(name)), [])
  })

  test('C0: one diagnostic changed', async () => {
    const [layers, score] = await caught((o) => { o.stderr = Buffer.concat([o.stderr, Buffer.from(`${rows.T02.path}:17:3: warning: planted\n`)]) }, 'C0')
    assert.deepEqual(layers, ['C0'])
    assert.deepEqual(score.warnings, [0, 1])
  })

  test('C1: one event dropped', async () => {
    const [layers, score] = await caught((o) => {
      const timing = JSON.parse(o['score.timing.json'])
      const events = timing[0].events
      // An event at a moment another shares, so that the breaks C3 reads from the events stay.
      const k = events.findIndex((e, i) => events.some((f, j) => j !== i && f.at === e.at && f.grace === e.grace))
      events.splice(k, 1)
      o['score.timing.json'] = Buffer.from(JSON.stringify(timing))
    }, 'C1')
    assert.deepEqual(layers, ['C1'])
    assert.ok(score.events < 1)
  })

  test('C2: one MIDI byte', async () => {
    const [layers, score] = await caught((o) => {
      const midi = o['score.midi']
      // The first note-on's key, a step up.
      const at = midi.findIndex((b, i) => i > 22 && (b & 0xf0) === 0x90 && midi[i + 1] < 127 && midi[i + 2] > 0)
      midi[at + 1]++
    }, 'C2')
    assert.deepEqual(layers, ['C2'])
    assert.ok(score.midiNotes < 1)
  })

  test('C3: a page dropped', async () => {
    const [layers, score] = await caught((o) => { delete o['score-2.svg'] }, 'C3')
    assert.deepEqual(layers, ['C3', 'C4'])
    assert.deepEqual(score.pages, [2, 1])
  })

  test('C4: one grob moved', async () => {
    const [layers] = await caught((o) => {
      o['score-1.svg'] = Buffer.from(o['score-1.svg'].toString('utf8')
        .replace(/(xlink:href="textedit:[^"]+">\s*<g transform="translate\()([-\d.]+)/, (_, a, x) => `${a}${(+x + 0.5).toFixed(4)}`))
    }, 'C4')
    // One position of hundreds: the bytes see it, the p95 does not.
    assert.deepEqual(layers, ['C4'])
  })

  test('C5: one shape', async () => {
    const [layers, score] = await caught((o) => {
      o['c5.tsv'] = Buffer.from(o['c5.tsv'].toString('utf8').replace(/^(G\t(?:Beam|Slur|PhrasingSlur|Tie)\t[^\t]+\t[^\t]+\t[^\t]+\t[^\t]+\t[^\d-]*)(-?\d+(?:\.\d+)?)/m,
        (_, a, v) => `${a}${(+v + 0.5).toFixed(3)}`))
    }, 'C5')
    assert.deepEqual(layers, ['C5'])
    assert.ok(score.shapeP95 > 0)
  })
})

test('a page replaced by its cells that is not the page from scratch is caught', () => {
  const cell = (digest, y, body) => `<g data-lily-cell="${digest}" transform="translate(0, ${y})">\n${body}\n</g>\n`
  const page = (...cells) => Buffer.from(`<svg>\n<g fill="currentColor">\n${cells.join('')}</g>\n</svg>\n`)
  const [a, b, c] = [cell('aa', '1', '<rect/>'), cell('bb', '2', '<path/>'), cell('cc', '3', '<a/>')]
  const sent = () => ({ 'p.cells': Buffer.from(`aa:1 cc:3\n${c}`), 'p.before': page(a, b) })
  assert.deepEqual(replacedPages(sent(), { 'p.svg': page(a, c) }), { sent: 1, wrong: [] })
  assert.equal(replacedPages(sent(), { 'p.svg': page(a, b) }).wrong.length, 1)
  // A digest the page before it lacks.
  const lacking = { 'p.cells': Buffer.from('zz:1\n'), 'p.before': page(a) }
  assert.deepEqual(replacedPages(lacking, { 'p.svg': page(a) }).wrong, ['p: a digest not on the page before it'])
})

/** That each edit's two compiles are equal in every layer, but KNOWN's, which differ in their layers only. */
function judge(results, t) {
  const unequal = {}
  for (const { id, name, result } of results) {
    const known = KNOWN[`${id} ${name}`]
    const layers = LAYERS.filter((l) => !result[l].equal)
    if (known) {
      assert.deepEqual(layers, known, `${id} ${name}: KNOWN's layers, now ${JSON.stringify(differences(result))}`)
      assert.deepEqual(result.other, [], `${id} ${name}`)
    } else if (!result.equal) unequal[`${id} ${name}`] = differences(result)
  }
  assert.deepEqual(unequal, {}, `${Object.keys(unequal).length} of ${results.length} edits differ (sources kept as named; seed ${SEED})`)
  t.diagnostic(`${results.length} edits, ${results.filter((r) => r.result.equal).length} equal in every layer (seed ${SEED})`)
}

/**
 * That N8's digests hold for each edit (MS6 step 4): the checkpoints and the
 * timesteps' input before m_e as the compile before it had them, and from
 * the convergence on its timesteps and checkpoints the old run's.
 */
function n8(results, t) {
  const wrong = Object.fromEntries(results.filter((r) => r.n8.wrong.length).map((r) => [`${r.id} ${r.name}`, r.n8.wrong]))
  const held = []
  for (const [edit, what] of Object.entries(N8_KNOWN)) {
    if (!wrong[edit]) held.push(edit)
    else assert.ok(wrong[edit].every((w) => w.includes(what)), `${edit}: N8_KNOWN's ${what}, now ${JSON.stringify(wrong[edit])}`)
    delete wrong[edit]
  }
  if (held.length) t.diagnostic(`N8_KNOWN that held: ${held.join(', ')}`)
  assert.deepEqual(wrong, {}, `${Object.keys(wrong).length} of ${results.length} edits contradict N8's digests (seed ${SEED})`)
  const changes = results.flatMap((r) => r.n8.changes)
  const changed = changes.filter((c) => c.changed !== '-')
  const sum = (cs, f) => cs.reduce((s, c) => s + f(c), 0)
  t.diagnostic(`N8: ${changes.length} translations, ${changed.length} changed, ${changed.filter((c) => c.converged !== '-').length} converged; ${sum(changed, (c) => c.run)} of their ${sum(changed, (c) => c.steps)} timesteps run again`)
  // Resumed from a snapshot of the compile before (MS6 step 5's rest, D242):
  // their compiles are among those asserted equal above.
  const resumed = changes.filter((c) => c.resumed !== null && c.resumed !== undefined)
  t.diagnostic(`N8's resume: ${resumed.length} of ${changes.length} translations resumed from a snapshot of the compile before`)
  assert.ok(resumed.length > 0, 'no translation resumed')
  // Of those, converged with the run before and ended as it did (D243).
  const spliced = changes.filter((c) => c.spliced)
  t.diagnostic(`N8's splice: ${spliced.length} of the ${resumed.length} resumed converged and ended as the run before`)
  // What was replayed: the calls of partitions' translators made, and made
  // again from the old run's log (MS6 step 7c); off but with `--replay-all`
  // (D258), which asks for it.
  t.diagnostic(`N8's replay: ${changes.filter((c) => c.attempts > 0).length} translations, ${changes.filter((c) => c.attempts > 1).length} failed and went back; ${sum(changes, (c) => c.ran)} calls made, ${sum(changes, (c) => c.replayed)} replayed`)
  // With partitions (MS6 step 6): translators times timesteps.
  const parted = changed.filter((c) => c.partitioned)
  t.diagnostic(`N8's partitions: ${parted.length} translations with the same partitions run ${sum(parted, (c) => c.partitioned.work)} of the ${sum(parted, (c) => c.partitioned.unpartitioned)} translator-timesteps the score as one partition would (${sum(parted, (c) => c.partitioned.whole)} whole); ${sum(parted, (c) => c.partitioned.readsOnly)} were a partition's writes outside itself replayed (MS6 step 7a)`)
}

/**
 * Each of the 30's `\layout` partitions (MS6 step 6): its line contexts at
 * least as many as LilyPond's largest system has VerticalAxisGroups
 * ($LILY_CORPUS/ms6-split/cells.txt), and T29's 28.
 */
function partitions(results, t) {
  const cells = path.join(corpusDir(), 'ms6-split', 'cells.txt')
  if (!existsSync(cells)) return t.diagnostic(`no ${cells}: the partitions not checked`)
  const largest = Object.fromEntries(readFileSync(cells, 'utf8').split('\n').filter((l) => /^T\d/.test(l)).map((l) => l.split('\t')).map((f) => [f[0], Number(f[4])]))
  const lines = {}
  for (const r of results) lines[r.id] ??= r.n8.parts[0]?.keys.reduce((n, k) => n + k.split('+').length, 0)
  const short = THIRTY.filter((id) => lines[id] !== undefined && lines[id] < largest[id])
  assert.deepEqual(short, [], `fewer line contexts than LilyPond's largest system has VerticalAxisGroups: ${short.map((id) => `${id} ${lines[id]} < ${largest[id]}`).join(', ')}`)
  assert.equal(lines.T29, 28)
  const merged = results.filter((r, i) => results.findIndex((x) => x.id === r.id) === i && r.n8.parts.some((p) => p.mergedAll))
  t.diagnostic(`partitions of the \\layout: ${THIRTY.map((id) => `${id} ${results.find((r) => r.id === id)?.n8.parts[0]?.keys.length ?? '?'}`).join(', ')}; all one in ${merged.map((r) => `${r.id} (${r.n8.parts.find((p) => p.mergedAll).mergedAll})`).join(', ') || 'none'}`)
}

/**
 * What layout's cells' callbacks read that no line's range covers (MS6 step
 * 8, 05 §6.3; `--check-cells`), by the grob read: each is in the cells' keys
 * (D248). A column by its place among the system's, its X and its record; the
 * break alignment's grobs by their X and records; the system, the alignment
 * of its lines and a spacing spanner by their records; a group's `-`, the
 * grobs it holds, by the cell. Where the alignment places a line (`Y-offset`,
 * `minimum-translations-alist`) keeps the cell from being reused.
 */
const COVERED = {
  PaperColumn: 'its place, X and record',
  NonMusicalPaperColumn: 'its place, X and record',
  BreakAlignGroup: 'its X and record',
  BreakAlignment: 'its X and record',
  VerticalAlignment: 'its record; where it places a line, no reuse',
  System: 'its record',
  SpacingSpanner: 'its record',
}

/**
 * The cells of `results`' incremental compiles: some reused, T29's E1 among
 * them, and what their callbacks read that no range covers in COVERED; and
 * line breaking's lines (MS6 step 9, D249): some taken from the compile
 * before, T12's E1 among them, and none that `--check-cells` filled otherwise.
 */
function cells(results, t) {
  const reports = results.flatMap((r) => r.cells.reports)
  const sum = (k) => reports.reduce((n, r) => n + r[k], 0)
  t.diagnostic(`cells: ${sum('reused')} of ${sum('cells')} reused, ${sum('grobsReused')} of ${sum('grobs')} grobs; not kept: ${sum('impure')} ran the score's procedures, ${sum('readsY')} read where a line is, ${sum('coupled')} wrote across a line`)
  assert.ok(sum('reused') > 0, 'no cell was reused')
  const t29 = results.find((r) => r.id === 'T29' && r.name === 'E1')
  assert.ok(t29.cells.reports[0].reused > 0, `T29's E1: ${JSON.stringify(t29.cells.reports)}`)
  const reads = [...new Set(results.flatMap((r) => r.cells.uncovered).filter((u) => u.startsWith('read\t')))]
  const uncovered = reads.map((u) => u.split('\t')).filter(([, , grob, sym]) => !COVERED[grob] && sym !== '-')
  assert.deepEqual(uncovered.map((u) => u.slice(1).join(' ')), [], 'read by a cell, in no key')
  t.diagnostic(`lines: ${sum('linesReused')} of ${sum('lines')} taken from the compile before, ${sum('linesWrong')} wrong`)
  // T29's E1 takes none: each of its lines has a bar line whose neighbours
  // reach the edited measure.
  const t12 = results.find((r) => r.id === 'T12' && r.name === 'E1')
  assert.ok(t12.cells.reports[0].linesReused > 0, `T12's E1 took no line: ${JSON.stringify(t12.cells.reports)}`)
  const wrong = results.filter((r) => r.cells.reports.some((c) => c.linesWrong > 0)).map((r) => `${r.id} ${r.name}`)
  assert.deepEqual(wrong, [], 'a line taken that is filled otherwise')
  // What came before the line breaks (D250): most of T29's lines given back
  // on its E1, which edits one staff of 28.
  t.diagnostic(`before the breaks: ${sum('beforeReused')} of ${sum('before')} lines given back, ${sum('beforeGrobsReused')} of ${sum('beforeGrobs')} grobs; ${sum('skylinesReused')} of ${sum('skylines')} columns' skylines taken`)
  const before = t29.cells.reports[0]
  assert.ok(before.beforeReused * 2 > before.before, `T29's E1 gave back too few lines: ${JSON.stringify(before)}`)
  // Columns made again from their boxes as the lines moved (D260): T29's E1
  // grows a staff. No column of any edit off from scratch (OPEN B27, D268):
  // T02's E1 and T29's E1 and E3 had 1, 9 and 1 (dots read before their column
  // put them), seeds 1 and 2 had T23's and T25's (a column taken skipped what
  // computing it wrote; a stem's pure height across the staves kept as steady).
  const off = results.filter((r) => r.skylinesOff).map((r) => `${r.id} ${r.name}: ${r.skylinesOff}`)
  t.diagnostic(`skylines: ${sum('skylinesReused')} of ${sum('skylines')} taken, ${sum('skylinesMoved')} made again from their boxes, ${off.length} edits with columns off from scratch; ${sum('beforePlaces')} grobs kept where a column first read them, ${sum('beforeMoved')} lines not kept as one was read in two places`)
  assert.ok(before.skylinesMoved > 0, `T29's E1 made no column from its boxes: ${JSON.stringify(before)}`)
  assert.deepEqual(off, [], 'columns off from scratch')
}

/**
 * That each page sent by its cells, replaced as the preview replaces them, is
 * the page written from scratch byte for byte (MS6 step 10, 06 §6.4), and the
 * writer's work when native `pages` tell it: T29's E1, which moves the staves
 * below the edit, sends its staff and its system's rest; its E3, a string of
 * the header, a cell.
 */
function replaced(results, t) {
  const wrong = results.flatMap((r) => r.replaced.wrong.map((w) => `${r.id} ${r.name} ${w}`))
  assert.deepEqual(wrong, [], 'a page replaced by its cells that is not the page from scratch')
  const sent = results.reduce((n, r) => n + r.replaced.sent, 0)
  assert.ok(sent > 0, 'no page was sent by its cells')
  t.diagnostic(`pages sent by their cells: ${sent}, each the page from scratch byte for byte`)
  if (!results[0].pages) return
  const pages = results.flatMap((r) => r.pages)
  const sum = (cs, k) => cs.reduce((n, p) => n + p[k], 0)
  const by = pages.filter((p) => p.replaced)
  t.diagnostic(`of ${pages.length} pages ${by.length} by their cells: ${sum(by, 'sent')} of ${sum(by, 'cells')} cells, ${sum(by, 'sentBytes')} of ${sum(by, 'pageBytes')} bytes; ${sum(pages, 'taken')} of ${sum(pages, 'entries')} entries taken`)
  const t29 = (name) => results.find((r) => r.id === 'T29' && r.name === name)?.pages[0]
  // Typed edits have no E1 or E3 of their own.
  if (!t29('E1')) return
  assert.ok(t29('E1').replaced && t29('E1').sent <= 2, `T29's E1: ${JSON.stringify(t29('E1'))}`)
  assert.ok(t29('E3').replaced && t29('E3').sent === 1, `T29's E3: ${JSON.stringify(t29('E3'))}`)
  assert.ok(t29('E1').taken * 2 > t29('E1').entries, `T29's E1: ${JSON.stringify(t29('E1'))}`)
  const kept = Math.max(...pages.map((p) => p.kept))
  assert.ok(kept <= 4 << 20, `README §4.5: ${kept} bytes of markup kept`)
  t.diagnostic(`markup kept: T29 ${t29('E1').kept} bytes (README §4.5's S 1 MB, L 4 MB), at most ${kept}`)
}

// What a compile does as it ships (D258), which `--n8` does not: its recording
// keeps only what a next compile can use, and layout lays out what it made.
test(`E1–E3 on the four and ${COUNT} seeded edits of each of the 30 natively, as a compile runs in the studio: equal in C0–C5`,
  { skip, timeout: 3_600_000 }, async (t) => {
    judge(await nativeDifferential(editsOf(THIRTY, { seed: SEED, count: COUNT }), path.join(scratch, 'shipped'), { check: false }), t)
  })

test(`E1–E3 on the four and ${COUNT} seeded edits of each of the 30 natively: incremental and from scratch equal in C0–C5`,
  { skip, timeout: 3_600_000 }, async (t) => {
    const results = await nativeDifferential(editsOf(THIRTY, { seed: SEED, count: COUNT }), path.join(scratch, 'native'), { flags: ['--check-cells'] })
    assert.equal(results.length, 12 + THIRTY.length * COUNT)
    judge(results, t)
    n8(results, t)
    partitions(results, t)
    cells(results, t)
  })

/** A score whose translation changes `cell`, made by `made`, by `change`, late, and whose layout warns of `read`; the edit its `part`. */
const older = (made, read, change, part) => (v) => `\\version "2.26.0"
\\header { title = "${part === 'title' ? v : 'T'}" }
#(define cell ${made})
bars = { c'4 d' e' f' | c'4 d' e' f' | c'4 d' e' f' | c'4 d' e' f' }
\\score { { \\override Score.BarLine.after-line-breaking = #(lambda (g) (ly:warning "cell is ~a" ${read}))
  \\bars \\bars ${part === 'note' ? v : "c'4"} d' e' f' | \\bars \\applyContext #(lambda (c) ${change}) \\bars \\bars } \\layout { } }
`

// Scores of N8's own (MS6 step 7c), each an edit of a text in `scratch`.
const OWN = [
  // A `\score` in a markup in a variable, which the shape's digest sees by
  // its object: its edit is the music's that holds the markup.
  ['score-in-markup', (name) => `\\version "2.26.0"
inc = \\markup { \\score { { \\set Staff.instrumentName = "${name}" c'1 } \\layout { } } }
\\score { \\new Staff { \\set Staff.instrumentName = \\inc c'1 } \\layout { } }
`, 'Tenor', 'Tenor '],
  // Three staves replayed around an edit: the alterations of a group changed
  // in place, a slur, a tie and a hairpin past the edit, a staff that ends
  // after it, an alteration the other staves read. The bars before it are
  // what leaves a snapshot between the start and the edit (D255 keeps four).
  ['three-staves', (note) => `\\version "2.26.0"
\\score { \\new PianoStaff <<
  \\new Staff { \\accidentalStyle piano ${"c'4 d' e' f' | ".repeat(16)}c'4 d' e' f' | ${note} a' b' c'' | c'4 d' e' f' | g' a' b' c'' }
  \\new Staff { ${"c'1 | ".repeat(16)}c'4( d' e' f') | g' a' b' c''~ | c''4 cis'' d'' e'' | f''1\\< | g''\\! }
  \\new Staff { \\clef bass ${'c1 | '.repeat(16)}c8 d e f g a b c' | c'1 | c8 d e f g a b c' | c'1 | c }
>> \\layout { } \\midi { } }
`, "g'4", "gis'4"],
  // What a translation changes of what is older than it, after every snapshot
  // an edit resumes from, read by layout (D258): a splice must not leave it
  // out. The first two by an edit of the header, the music the same, the
  // others by a pitch before the change; a pair's cell, a global.
  ['older-header', older('(list 0)', '(car cell)', '(set-car! cell (1+ (car cell)))', 'title'), 'T', 'Tx'],
  ['global-header', older('0', 'cell', '(set! cell (1+ cell))', 'title'), 'T', 'Tx'],
  ['older-pitch', older('(list 0)', '(car cell)', '(set-car! cell (1+ (car cell)))', 'note'), "c'4", "d'4"],
  ['global-pitch', older('0', 'cell', '(set! cell (1+ cell))', 'note'), "c'4", "d'4"],
]

test('N8\'s own scores: an edit in a \\score in a markup translated again; three staves replayed around an edit; what a translation changes of what is older than it',
  { skip, timeout: 600_000 }, async (t) => {
    const dir = path.join(scratch, 'own')
    mkdirSync(dir, { recursive: true })
    const edits = OWN.flatMap(([id, text, before, after]) => {
      const file = path.join(dir, `${id}.ly`)
      writeFileSync(file, text(before))
      // The first compile logs nothing, the second what the third replays.
      return [['there', after], ['back', before], ['there again', after]].map(([name, v]) => ({ id, name, file, source: text(v) }))
    })
    const results = await nativeDifferential(edits, path.join(dir, 'laid-out'), { check: false })
    for (const r of results) assert.ok(r.result.equal, `${r.id} ${r.name}: ${JSON.stringify(differences(r.result))}`)
    // For a preview too: a markup's \score writes grobs of its own, in no
    // system of the page (MS6 step 10).
    const previewed = await nativeDifferential(edits, path.join(dir, 'previewed'), { check: false, preview: true })
    for (const r of previewed) {
      assert.ok(r.result.equal, `${r.id} ${r.name}, for a preview: ${JSON.stringify(differences(r.result))}`)
      assert.deepEqual(r.replaced.wrong, [], `${r.id} ${r.name}`)
    }
    const checked = await nativeDifferential(edits, path.join(dir, 'checked'), { flags: ['--replay-all'] })
    const replayed = checked.filter((r) => r.id === 'three-staves').flatMap((r) => r.n8.changes)
    t.diagnostic(`three staves: ${replayed.map((c) => `${c.ran} calls made, ${c.replayed} replayed`).join('; ')}`)
    assert.ok(replayed.some((c) => c.replayed > 0), 'nothing was replayed')
  })

// Replay is off (D258) and kept right: N8's checks of every translation it can
// be, and what it made laid out below.
test(`E1–E3 on the four and ${COUNT} seeded edits of each of the 30 natively, every translation it can replayed: N8's checks hold`,
  { skip, timeout: 3_600_000 }, async (t) => {
    const results = await nativeDifferential(editsOf(THIRTY, { seed: SEED, count: COUNT }), path.join(scratch, 'replayed'), { flags: ['--replay-all'] })
    judge(results, t)
    n8(results, t)
    const changes = results.flatMap((r) => r.n8.changes)
    assert.ok(changes.some((c) => c.replayed > 0), 'nothing was replayed')
  })

// What a splice or a replay made laid out, where `--n8` lays out the run its
// check ran after them (`nativeDifferential`), and every translation of two
// partitions replayed, where the engine replays those of three (MS6 step 7c).
test(`E1–E3 on the four and ${COUNT} seeded edits of each of the 30 natively, every translation it can replayed: what the incremental compile made laid out, equal in C0–C5`,
  { skip, timeout: 3_600_000 }, async (t) => {
    const results = await nativeDifferential(editsOf(THIRTY, { seed: SEED, count: COUNT }), path.join(scratch, 'laid-out'), { check: false, flags: ['--replay-all'] })
    judge(results, t)
  })

test(`E1–E3 on the four and ${WASM_COUNT} seeded edits of each of the 30 in core.wasm: one instance against an instance each, equal in C0–C5`,
  { skip: skip || coreSkip(), timeout: 3_600_000 }, async (t) => {
    const results = await wasmDifferential(editsOf(THIRTY, { seed: SEED, count: WASM_COUNT }), path.join(scratch, 'wasm'))
    assert.equal(results.length, 12 + THIRTY.length * WASM_COUNT)
    judge(results, t)
  })

// C5's pages (MS6 step 10): both compiles for a preview, the preview form's
// pages equal, and each page sent by its cells the page from scratch.
test(`E1–E3 on the four and ${COUNT} seeded edits of each of the 30 natively, for a preview: each page sent by its cells the page from scratch`,
  { skip, timeout: 3_600_000 }, async (t) => {
    const results = await nativeDifferential(editsOf(THIRTY, { seed: SEED, count: COUNT }), path.join(scratch, 'preview'), { check: false, preview: true })
    judge(results, t)
    replaced(results, t)
  })

test(`E1–E3 on the four and ${WASM_COUNT} seeded edits of each of the 30 in core.wasm, for a preview: each page sent by its cells the page from scratch`,
  { skip: skip || coreSkip(), timeout: 3_600_000 }, async (t) => {
    const results = await wasmDifferential(editsOf(THIRTY, { seed: SEED, count: WASM_COUNT }), path.join(scratch, 'wasm-preview'), { preview: true })
    judge(results, t)
    replaced(results, t)
  })

// A person typing (MS7 step 1, D258): each edit on the text the one before left,
// where the snapshots near an edit are taken and read, not the score's own text.
test('typing\'s steps: each of the text the step before left, one undone back to it',
  { skip, timeout: 60_000 }, () => {
    const edits = typing('T24', { count: 12 })
    assert.deepEqual(edits.map((e) => e.kind), Array.from({ length: 12 }, (_, k) => TYPED[k % TYPED.length]))
    const text = readFileSync(rows.T24.path, 'utf8')
    assert.notEqual(edits[0].source, text)
    assert.equal(edits[2].source, edits[1].source, 'the same text again')
    assert.equal(edits[5].source, edits[3].source, 'an edit undone')
    assert.equal(edits[8].source, edits[6].source, 'an error undone')
  })

const typed = (ids) => ids.flatMap((id) => typing(id, { count: COUNT + 2 }))
test(`${COUNT + 2} edits typed on each of the 30 natively: N8's checks hold, and each compile the one from scratch`,
  { skip, timeout: 3_600_000 }, async (t) => {
    const results = await nativeDifferential(typed(THIRTY), path.join(scratch, 'typed'), { chain: true })
    judge(results, t)
    n8(results, t)
  })

test(`${COUNT + 2} edits typed on each of the 30 natively, as a compile runs in the studio: equal in C0–C5`,
  { skip, timeout: 3_600_000 }, async (t) => {
    judge(await nativeDifferential(typed(THIRTY), path.join(scratch, 'typed-shipped'), { chain: true, check: false }), t)
  })

test(`${COUNT + 2} edits typed on each of the 30 natively, for a preview: each page sent by its cells the page from scratch`,
  { skip, timeout: 3_600_000 }, async (t) => {
    const results = await nativeDifferential(typed(THIRTY), path.join(scratch, 'typed-preview'), { chain: true, check: false, preview: true })
    judge(results, t)
    replaced(results, t)
  })

test(`${COUNT + 2} edits typed on each of the four in core.wasm: one instance against an instance each, equal in C0–C5`,
  { skip: skip || coreSkip(), timeout: 3_600_000 }, async (t) => {
    judge(await wasmDifferential(typed(EDITED), path.join(scratch, 'typed-wasm'), { chain: true }), t)
  })

// H3 (MS6 step 11): a compile returning at every point it can, and first
// stopped by a newer compile at a point that moves with the edit, equals one
// from scratch. Untraced, as a traced compile is one call: C1–C5 are its pages,
// MIDI files and playback map. The wasm tests' compiles keep to STEP_MS.
const STEPS = ['--step-ms', '0', '--stop-at', '7']
test(`E1–E3 on the four and ${COUNT} seeded edits of each of the 30 natively, in steps and stopped by a newer compile: what it writes the compile from scratch's`,
  { skip, timeout: 3_600_000 }, async (t) => {
    judge(await nativeDifferential(editsOf(THIRTY, { seed: SEED, count: COUNT }), path.join(scratch, 'steps'), { check: false, steps: STEPS }), t)
  })

// A compile stopped leaves what the last that ended kept (B24, D261), of the
// text it was made from: `--stop-moved` stops each compile of a text one line
// longer at the edit, in steps of no time, so that it does stop: one that ended
// would keep its own (and OPEN B31, a move of two places, is not this test's).
test(`E1–E3 on the four and ${COUNT} seeded edits of each of the 30 natively, stopped with another text: what it writes the compile from scratch's`,
  { skip, timeout: 3_600_000 }, async (t) => {
    judge(await nativeDifferential(editsOf(THIRTY, { seed: SEED, count: COUNT }), path.join(scratch, 'steps-moved'), { check: false, steps: ['--step-ms', '0', '--stop-at', '3', '--stop-moved'] }), t)
  })

test(`E1–E3 on the four and ${COUNT} seeded edits of each of the 30 natively, in steps and stopped, for a preview: each page sent by its cells the page from scratch`,
  { skip, timeout: 3_600_000 }, async (t) => {
    const results = await nativeDifferential(editsOf(THIRTY, { seed: SEED, count: COUNT }), path.join(scratch, 'steps-preview'), { check: false, preview: true, steps: STEPS })
    judge(results, t)
    replaced(results, t)
  })
