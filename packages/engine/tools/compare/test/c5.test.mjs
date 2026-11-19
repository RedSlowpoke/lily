// C5, the shape layer (chapter 05 §9): a dump scores 0 against itself and more
// against a perturbed copy, on synthetic dumps, on perturbed copies of the
// nine beam and slur scores' reference dumps, and on LilyPond's own beam
// setting, which C4 cannot see (05 §2.4); and on the engine's own dumps (MS4
// step 6), the c5.tsv a traced compile writes, natively and in wasm, against
// LilyPond's on the scores laid out whole. The LilyPond tests skip when 2.26.0
// is not installed; those on the reference skip when it has not been captured.
import assert from 'node:assert/strict'
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, describe, test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { CAPTURE, capture, captureOne, lilypondVersion } from '../capture.mjs'
import { compileInWasm, coreSkip, preloaded } from '../core.mjs'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { GOAL, MUST, SHAPES, compare, readShapes } from '../score.mjs'

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-c5-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

const lilypond = lilypondVersion() === '2.26.0'
const references = path.join(corpusDir(), 'ref')
// 05 §2.4's nine scores with beams or slurs that matter.
const NINE = ['T08', 'T11', 'T12', 'T23', 'T24', 'T25', 'T26', 'T29', 'T30']

// A dump as c5dump.ly writes one: two systems; two beams, a slur and a tie; a stem and a note head C5 ignores.
const DUMP = [
  ['S', 0, 0, 10, 0, 12],
  ['G', 'Beam', 's.ly:3:5', 0, 10.5, -4, '(-2.19 -2.81)', 0],
  ['G', 'Slur', 's.ly:3:4', 0, 10.2, -4, '((0.5 . 1.5) (1.9 . 3.2) (8.8 . 4.3) (10.9 . 3.2))', 0],
  ['G', 'Stem', 's.ly:3:2', 0, 10.5, -4, '(-3.5 0.0)', 0],
  ['G', 'NoteHead', 's.ly:3:2', 0, 10.5, -4, '', 0],
  ['S', 1, 0, 30, 2, 9],
  ['G', 'Beam', 's.ly:4:9', 1, 3.1, -4, '(3.81 1.81)', 1],
  ['G', 'Tie', 's.ly:4:7', 1, 6.4, -4, '((0.43 . -3.045) (1.624 . -4.067) (4.511 . -4.067) (5.704 . -3.045))', 1],
]

function dumpDir(name, rows = DUMP) {
  const dir = path.join(scratch, name)
  mkdirSync(dir, { recursive: true })
  writeFileSync(path.join(dir, SHAPES), rows.map((r) => r.join('\t')).join('\n') + '\n')
  return dir
}

// The dump with the numbers of the `type` grobs' shapes moved by `by` (a beam's ends; a control point's y).
function perturb(rows, type, by, { only = 0 } = {}) {
  let seen = 0
  return rows.map((r) => {
    if (r[0] !== 'G' || r[1] !== type || (only && ++seen > only)) return r
    const digest = type === 'Beam'
      ? r[6].replace(/-?\d+(?:\.\d+)?/g, (v) => +(+v + by).toFixed(3))
      : r[6].replace(/\. (-?\d+(?:\.\d+)?)\)/g, (_, v) => `. ${+(+v + by).toFixed(3)})`)
    return [...r.slice(0, 6), digest, ...r.slice(7)]
  })
}

describe('C5 on synthetic dumps', () => {
  const ref = dumpDir('ref')

  test('reads the beams, slurs and ties, and nothing else', () => {
    const shapes = readShapes(path.join(ref, SHAPES))
    assert.deepEqual([...shapes.keys()], ['Beam|s.ly:3:5', 'Slur|s.ly:3:4', 'Beam|s.ly:4:9', 'Tie|s.ly:4:7'])
    assert.deepEqual(shapes.get('Beam|s.ly:3:5')[0].v, [-2.19, -2.81])
    assert.deepEqual(shapes.get('Tie|s.ly:4:7')[0].v[1], [1.624, -4.067])
    assert.equal(readShapes(path.join(scratch, 'none', SHAPES)), null)
  })

  test('scores a dump against itself as 0, within Must and Goal', () => {
    const r = compare(ref, ref)
    assert.deepEqual(r.shapesMatched, [4, 4])
    assert.deepEqual([r.shapeMedian, r.shapeP95], [0, 0])
    assert.equal(r.pass.C5, true)
    assert.equal(r.goal.C5, true)
  })

  test('measures a beam end by |Δ| and a control point by its distance', () => {
    // One beam end of 12 numbers moved: the p95 is that move.
    const beam = DUMP.map((r) => r[2] === 's.ly:3:5' ? [...r.slice(0, 6), '(-2.19 -2.71)', r[7]] : r)
    const b = compare(ref, dumpDir('beam', beam))
    assert.equal(+b.shapeP95.toFixed(3), 0.1)
    assert.equal(b.shapeMedian, 0)
    // A slur's control point moved 0.3 across and 0.4 up is 0.5 away.
    const slur = DUMP.map((r) => r[1] === 'Slur' ? [...r.slice(0, 6), '((0.5 . 1.5) (2.2 . 3.6) (8.8 . 4.3) (10.9 . 3.2))', r[7]] : r)
    assert.equal(+compare(ref, dumpDir('slur', slur)).shapeP95.toFixed(3), 0.5)
  })

  test('passes Must and misses Goal between 0.05 and 0.25, and fails Must above', () => {
    assert.deepEqual([MUST.shape, GOAL.shape], [0.25, 0.05])
    const near = compare(ref, dumpDir('near', perturb(DUMP, 'Tie', 0.04)))
    assert.deepEqual([near.pass.C5, near.goal.C5], [true, true])
    const off = compare(ref, dumpDir('off', perturb(DUMP, 'Slur', 0.2)))
    assert.deepEqual([off.pass.C5, off.goal.C5], [true, false])
    const far = compare(ref, dumpDir('far', perturb(DUMP, 'Beam', 0.3, { only: 1 })))
    assert.equal(+far.shapeP95.toFixed(3), 0.3)
    assert.deepEqual([far.pass.C5, far.goal.C5], [false, false])
  })

  test('pairs a link\'s grobs in order of system, X, Y and height, not of the dump', () => {
    // A chord's ties, as T22's, share their link, system, X and Y.
    const ties = [
      ['G', 'Tie', 's.ly:5:3', 0, 2, -4, '((0 . 1) (1 . 1.5) (3 . 1.5) (4 . 1))', 0],
      ['G', 'Tie', 's.ly:5:3', 0, 2, -4, '((0 . -1) (1 . -1.5) (3 . -1.5) (4 . -1))', 0],
      ['G', 'Tie', 's.ly:5:3', 0, 2, 2, '((0 . 1) (1 . 1.5) (3 . 1.5) (4 . 1))', 1],
    ]
    const r = compare(dumpDir('chord', ties), dumpDir('reversed', [...ties].reverse()))
    assert.deepEqual([r.shapesMatched, r.shapeP95], [[3, 3], 0])
  })

  test('leaves unpaired a grob in another system, or a link with another count of grobs', () => {
    const moved = DUMP.map((r) => r[1] === 'Tie' ? [...r.slice(0, 3), 0, ...r.slice(4)] : r)
    assert.deepEqual(compare(ref, dumpDir('moved', moved)).shapesMatched, [3, 4])
    const twice = [...DUMP, DUMP.find((r) => r[1] === 'Slur')]
    assert.deepEqual(compare(ref, dumpDir('twice', twice)).shapesMatched, [3, 4])
  })

  test('is not judged without the reference\'s dump, passes one without shapes, fails a candidate without one', () => {
    const none = path.join(scratch, 'none'); mkdirSync(none, { recursive: true })
    assert.equal(compare(none, ref).pass.C5, null)
    const empty = dumpDir('empty', DUMP.filter((r) => r[0] === 'S' || r[1] === 'Stem'))
    assert.deepEqual([compare(empty, ref).pass.C5, compare(empty, ref).shapesMatched], [true, [0, 0]])
    const r = compare(ref, none)
    assert.deepEqual([r.pass.C5, r.goal.C5, r.shapesMatched, r.shapeP95], [false, false, [0, 4], null])
  })
})

const captured = NINE.every((id) => existsSync(path.join(references, id, SHAPES)))

describe('C5 on the nine beam and slur scores', { skip: !captured && `no shape dumps in ${references}; run fetch.mjs and capture.mjs` }, () => {
  const rows = (id) => readFileSync(path.join(references, id, SHAPES), 'utf8').trim().split('\n').map((l) => l.split('\t'))
  // A copy of a reference capture whose dump has every `type` grob's shape moved by `by`.
  const copy = (id, name, edits) => {
    const dir = path.join(scratch, name, id)
    cpSync(path.join(references, id), dir, { recursive: true })
    let r = rows(id); for (const [type, by] of edits) r = perturb(r, type, by)
    writeFileSync(path.join(dir, SHAPES), r.map((f) => f.join('\t')).join('\n') + '\n')
    return dir
  }
  const all = (by) => [['Beam', by], ['Slur', by], ['PhrasingSlur', by], ['Tie', by]]

  for (const id of NINE) {
    test(`${id}: 0 against itself; Goal under 0.05; Must under 0.25; fails above, on C5 alone`, () => {
      const ref = path.join(references, id)
      const self = compare(ref, ref)
      assert.ok(self.shapesMatched[1] > 0, 'has shapes')
      assert.deepEqual([self.shapesMatched[0], self.shapeP95], [self.shapesMatched[1], 0])

      const near = compare(ref, copy(id, 'near', all(0.04)))
      assert.deepEqual([near.pass.C5, near.goal.C5], [true, true], `p95 ${near.shapeP95}`)
      const off = compare(ref, copy(id, 'off', all(0.2)))
      assert.deepEqual([off.pass.C5, off.goal.C5], [true, false], `p95 ${off.shapeP95}`)
      const far = compare(ref, copy(id, 'far', all(0.3)))
      assert.deepEqual([far.pass.C5, far.goal.C5], [false, false], `p95 ${far.shapeP95}`)
      assert.deepEqual(far.pass, { C1: true, C2: true, C3: true, C4: true, C5: false })
    })
  }

  test('T12: its slurs alone, moved 0.3, fail C5', () => {
    const r = compare(path.join(references, 'T12'), copy('T12', 'slurs', [['Slur', 0.3]]))
    assert.equal(r.pass.C5, false)
    assert.equal(+r.shapeP95.toFixed(3), 0.3)
  })
})

describe('C5 on LilyPond 2.26.0', { skip: !lilypond && 'lilypond 2.26.0 is not installed' }, () => {
  test('sees the beams that Beam.details.region-size = 1 moves, which C4 does not (05 §2.4, v2)', async () => {
    const plain = path.join(scratch, 'lily-plain'); const v2 = path.join(scratch, 'lily-v2')
    const settings = path.join(scratch, 'v2.ly')
    writeFileSync(settings, `\\include "${CAPTURE}"\n\\layout { \\context { \\Score \\override Beam.details.region-size = #1 } }\n`)
    await Promise.all([capture({ ids: ['T11', 'T30'], out: plain }), capture({ ids: ['T11', 'T30'], out: v2, settings })])
    for (const id of ['T11', 'T30']) {
      const r = compare(path.join(plain, id), path.join(v2, id))
      assert.deepEqual(r.pass, { C1: true, C2: true, C3: true, C4: true, C5: false }, id)
      assert.equal(r.shapesMatched[0], r.shapesMatched[1], id)
    }
    // 05 §2.4: 25 of T30's 67 beams move, by up to one staff space.
    const before = readShapes(path.join(plain, 'T30', SHAPES)); const after = readShapes(path.join(v2, 'T30', SHAPES))
    const beams = [...before].filter(([k]) => k.startsWith('Beam|')).flatMap(([k, list]) => list.map((g, i) => [g, after.get(k)[i]]))
    const moved = beams.map(([a, b]) => Math.max(...a.v.map((p, j) => Math.abs(p - b.v[j])))).filter((d) => d > 0.001)
    assert.deepEqual([moved.length, beams.length], [25, 67])
    assert.equal(+Math.max(...moved).toFixed(3), 1)
  })
})

// MS4's scores laid out whole, with their shapes' count: MS3's gate's 10 (no beam), T11 and T15 (part 1's), T27
// (its slurs scored since step 8), T12, T24 and T29 (their ties drawn since step 10; part 2's gate, step 11), T07,
// T08, T09, T25, T26 and T28 (their tuplets and drums since step 13; part 3's gate, step 14), T02, T04, T06, T21, T22,
// T23 and T30 (their lyrics, figures and tablature since steps 15–17; part 4's gate, step 18: all nine of C5's), and
// the beams, tremolos, slurs, ties, tuplets, drums, lyrics, figures and tablature fixtures, captured here. T20's
// lines are its fonts' (D227), not in it.
const LAID_OUT = {
  T01: 0, T03: 0, T05: 0, T10: 0, T13: 0, T14: 0, T16: 0, T17: 0, T18: 0, T19: 0, T11: 5, T15: 1, T27: 25, T12: 58, T24: 163, T29: 97,
  T07: 23, T08: 6, T09: 7, T25: 22, T26: 8, T28: 10, T02: 1, T04: 15, T06: 2, T21: 0, T22: 22, T23: 10, T30: 72,
}
const FIXTURES = { beams: 42, tremolos: 12, slurs: 36, ties: 89, tuplets: 29, drums: 15, 'lyric-grobs': 6, figures: 4, tablature: 11 }
/**
 * The scores whose `#(ly:set-option 'point-and-click #f)` capture.ly ignores, so that their references have links:
 * compiled as a copy with the option blanked, beside its include (layout.test.mjs' `withPointAndClick`).
 */
const POINT_AND_CLICK = ['T21', 'T23']
function withPointAndClick(row) {
  const dir = mkdtempSync(path.join(scratch, `point-and-click-${row.id}-`))
  const option = "#(ly:set-option 'point-and-click #f)"
  const text = readFileSync(row.path, 'utf8')
  assert.ok(text.includes(option), row.id)
  writeFileSync(path.join(dir, path.basename(row.path)), text.replace(option, '%'.padEnd(option.length)))
  writeFileSync(path.join(dir, 'example-header.ily'), readFileSync(path.join(path.dirname(row.path), 'example-header.ily')))
  return path.join(dir, path.basename(row.path))
}
const cargo = spawnSync('cargo', ['+stable', '--version']).status !== 0 && 'cargo +stable is not installed'
const packs = preloaded()
const laidOut = Object.keys(LAID_OUT).map((id) => corpus().find((row) => row.id === id))
const skipLaidOut = !laidOut.every((row) => row && existsSync(row.path) && existsSync(path.join(references, row.id, SHAPES))) &&
  `no shape dumps in ${references}; run fetch.mjs and capture.mjs`

describe("C5 on the engine's own dumps", { skip: (!lilypond && 'lilypond 2.26.0 is not installed') || cargo || (!packs.length && 'no preloaded font packs: run npm run build') || skipLaidOut }, () => {
  test("c5.tsv of the 10, T11, T15, T27, T12, T24, T29, T07, T08, T09, T25, T26, T28, T02, T04, T06, T21, T22, T23, T30 and the beams, tremolos, slurs, ties, tuplets, drums, lyrics, figures and tablature fixtures natively and in wasm: LilyPond's byte for byte, every beam, slur and tie paired, median and p95 0 (all nine of C5's since MS4 step 18)", { timeout: 1_200_000 }, async () => {
    const rows = laidOut.map((row) => ({ id: row.id, file: POINT_AND_CLICK.includes(row.id) ? withPointAndClick(row) : row.path,
      ref: path.join(references, row.id), beams: LAID_OUT[row.id] }))
    for (const [id, beams] of Object.entries(FIXTURES)) {
      const file = fileURLToPath(new URL(`../fixtures/${id}.ly`, import.meta.url))
      const ref = path.join(scratch, 'lilypond', id)
      assert.equal((await captureOne('lilypond', { id, file, path: file }, ref)).exit, 0, id)
      rows.push({ id, file, ref, beams })
    }
    const check = ({ id, ref, beams }, c5, where) => {
      assert.equal(c5, readFileSync(path.join(ref, SHAPES), 'utf8'), `${id} ${where}`)
      const r = compare(ref, dumpDir(path.join('engine', where, id), c5.trim().split('\n').map((l) => l.split('\t'))))
      assert.deepEqual([r.shapesMatched, r.pass.C5, r.goal.C5], [[beams, beams], true, true], `${id} ${where}`)
      if (beams) assert.deepEqual([r.shapeMedian, r.shapeP95], [0, 0], `${id} ${where}`)
    }
    for (const row of rows) {
      const out = mkdtempSync(path.join(scratch, `engine-${row.id}-`))
      const run = runExample('compile', ['--trace', ...packs.flatMap(([, f]) => ['--pack', f]), '--out', out, row.file],
        { cwd: path.dirname(row.file), encoding: 'utf8' })
      // LilyPond's exit: T03's error fails it.
      const exit = JSON.parse(readFileSync(path.join(row.ref, 'capture.json'), 'utf8')).exit
      assert.equal(run.status, exit, `${row.id}: ${run.stderr}`)
      check(row, readFileSync(path.join(out, `${path.basename(row.file, '.ly')}.${SHAPES}`), 'utf8'), 'natively')
    }
    if (coreSkip()) return
    const modules = packs.map(([name, file]) => [name, readFileSync(file)])
    const wasm = await compileInWasm(rows.map((row) => row.file), { trace: true, modules })
    for (const [i, row] of rows.entries()) check(row, wasm[i].outputs[SHAPES]?.toString(), 'in wasm')
  })
})
