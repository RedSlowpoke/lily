// score.mjs against captures of its own: a capture scores 1.0 against itself
// and less against a perturbed copy, on synthetic captures and on LilyPond's
// (chapter 00 §5.1, §5.3). The LilyPond tests skip when 2.26.0 is not
// installed; those on the fetched corpus skip when it has not been captured.
import assert from 'node:assert/strict'
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, describe, test } from 'node:test'
import { CAPTURE, capture, lilypondVersion } from '../capture.mjs'
import { corpusDir } from '../../corpus/fetch.mjs'
import { compare, compareAll, key, read, same } from '../score.mjs'

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-score-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

const lilypond = lilypondVersion() === '2.26.0'
const references = path.join(corpusDir(), 'ref')

// A one-track SMF at 120 bpm: each note [key, velocity, start, length] in ticks of 384 a quarter.
function smf(notes) {
  const events = notes.flatMap(([k, v, start, length]) => [[start, [0x90, k, v]], [start + length, [0x80, k, 0]]])
    .sort((a, b) => a[0] - b[0])
  const varlen = (n) => { const out = [n & 0x7f]; while ((n >>= 7)) out.unshift((n & 0x7f) | 0x80); return out }
  let now = 0
  const body = [0, 0xff, 0x51, 3, 0x07, 0xa1, 0x20]
  for (const [at, bytes] of events) { body.push(...varlen(at - now), ...bytes); now = at }
  body.push(0, 0xff, 0x2f, 0)
  const u32 = (n) => [n >>> 24, (n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff]
  return Buffer.from([0x4d, 0x54, 0x68, 0x64, ...u32(6), 0, 1, 0, 1, 1, 128,
    0x4d, 0x54, 0x72, 0x6b, ...u32(body.length), ...body])
}

// A capture as LilyPond writes one: linked elements on SVG pages, the playback map and a .midi.
function synthetic(dir, { shift = 0, velocity = 90, drop = 0, pages = 1 } = {}) {
  mkdirSync(dir, { recursive: true })
  const href = (line, char) => `textedit:///abs/s.ly:${line}:${char}:${char + 1}`
  const notes = Array.from({ length: 8 }, (_, i) => ({ line: 3 + (i >> 2), char: 2 + 3 * (i % 4), at: i / 4, x: 10 + 12 * (i % 4) }))
  const link = (n, dx) => `<a xlink:href="${href(n.line, n.char)}">\n<g transform="translate(${n.x + dx}, ${n.line * 10})">\n<path d=""/></g></a>`
  for (let p = 0; p < pages; p++) {
    const name = pages > 1 ? `s-${p + 1}.svg` : 's.svg'
    const body = p === 0 ? notes.map((n, i) => link(n, i === 7 ? shift : 0)).join('\n') : ''
    writeFileSync(path.join(dir, name), `<svg xmlns:xlink="http://www.w3.org/1999/xlink">\n${body}\n</svg>\n`)
  }
  const events = notes.slice(0, notes.length - drop).map((n) => ({ href: href(n.line, n.char), at: n.at, grace: 0, length: 0.25 }))
  writeFileSync(path.join(dir, 's.timing.json'), JSON.stringify([{ events }]))
  writeFileSync(path.join(dir, 's.midi'), smf(notes.map((n, i) => [60 + i, i === 0 ? velocity : 90, i * 384, 384])))
  return dir
}

describe('the comparator', () => {
  test('keys a link by file:line:char, without its directory and column', () => {
    assert.equal(key('textedit:///Users/a/b%20c/score.ly:15:2:3'), 'score.ly:15:2')
  })

  test('scores multisets with their multiplicities', () => {
    assert.equal(same([], []), 1)
    assert.equal(same(['a', 'a', 'b'], ['a', 'a', 'b']), 1)
    assert.equal(same(['a', 'a'], ['a']), 2 / 3)
    // 00 Appendix A: dropping one of T05's 105 events scores C1 at 0.9952.
    const events = Array.from({ length: 105 }, (_, i) => String(i % 100))
    assert.equal(same(events, events.slice(1)).toFixed(4), '0.9952')
  })

  const ref = synthetic(path.join(scratch, 'ref'))

  test('scores a capture against itself as 1.0 on every layer', () => {
    const r = compare(ref, ref)
    assert.deepEqual(r.pages, [1, 1])
    assert.equal(r.events, 1)
    assert.equal(r.midiNotes, 1)
    assert.equal(r.breaks, 1)
    assert.deepEqual(r.linksMatched, [8, 8])
    assert.deepEqual([r.dxMedian, r.dxP95, r.dyMedian, r.dyP95], [0, 0, 0, 0])
    // No shape dump in a synthetic capture: C5 is not judged (c5.test.mjs).
    assert.deepEqual(r.pass, { C1: true, C2: true, C3: true, C4: true, C5: null })
  })

  test('scores a copy lower on the layer that was changed, and only there', () => {
    const moved = compare(ref, synthetic(path.join(scratch, 'moved'), { shift: 3 }))
    assert.equal(moved.dxP95, 3)
    assert.equal(moved.dxMedian, 0)
    assert.equal(moved.pass.C4, false)
    assert.equal(moved.events, 1)
    assert.equal(moved.midiNotes, 1)

    const louder = compare(ref, synthetic(path.join(scratch, 'louder'), { velocity: 100 }))
    assert.equal(louder.midiNotes, 2 * 7 / 16)
    assert.equal(louder.pass.C2, false)
    assert.equal(louder.events, 1)
    assert.equal(louder.dxP95, 0)

    const fewer = compare(ref, synthetic(path.join(scratch, 'fewer'), { drop: 1 }))
    assert.equal(fewer.events, 2 * 7 / 15)
    assert.equal(fewer.pass.C1, false)

    const longer = compare(ref, synthetic(path.join(scratch, 'longer'), { pages: 2 }))
    assert.deepEqual(longer.pages, [1, 2])
    assert.equal(longer.pass.C3, false)
  })

  test('finds a system break where the columns jump back left', () => {
    // The second line's first note (x 10, after 46) starts a system: one break, at moment 1.
    assert.deepEqual([...read(ref).breaks], ['1|0'])
    // Moved on to the right of the first line, the second line no longer starts a system.
    const flat = path.join(scratch, 'flat')
    cpSync(ref, flat, { recursive: true })
    const svg = readFileSync(path.join(ref, 's.svg'), 'utf8')
    writeFileSync(path.join(flat, 's.svg'), svg.replace(/translate\((\d+), 40\)/g, (_, x) => `translate(${+x + 48}, 40)`))
    const r = compare(ref, flat)
    assert.equal(r.breaks, 0)
    assert.equal(r.pass.C3, false)
    assert.equal(r.events, 1)
  })

  test('fails every layer of a score with no candidate', () => {
    const root = path.join(scratch, 'roots')
    synthetic(path.join(root, 'ref', 'T01'))
    mkdirSync(path.join(root, 'cand'), { recursive: true })
    assert.deepEqual(compareAll(path.join(root, 'ref'), path.join(root, 'cand')),
      [{ id: 'T01', missing: true, pass: { C1: false, C2: false, C3: false, C4: false, C5: false }, goal: { C5: false, C2b: false } }])
  })
})

describe('LilyPond 2.26.0 captures', { skip: !lilypond && 'lilypond 2.26.0 is not installed' }, () => {
  test('are deterministic, and move only the layout layers when the line is shorter', async () => {
    const plain = path.join(scratch, 'plain'); const again = path.join(scratch, 'again'); const short = path.join(scratch, 'short')
    const settings = path.join(scratch, 'short.ly')
    writeFileSync(settings, `\\include "${CAPTURE}"\n\\paper { line-width = 150\\mm }\n`)
    const results = await Promise.all([
      capture({ ids: ['T01', 'T02'], out: plain }),
      capture({ ids: ['T02'], out: again }),
      capture({ ids: ['T02'], out: short, settings }),
    ])
    assert.deepEqual(results.flat().map((r) => [r.id, r.exit, r.timing]), [['T01', 0, true], ['T02', 0, true], ['T02', 0, true], ['T02', 0, true]])

    const self = compare(path.join(plain, 'T02'), path.join(again, 'T02'))
    assert.deepEqual(self.pass, { C1: true, C2: true, C3: true, C4: true, C5: true })
    assert.deepEqual([self.events, self.midiNotes, self.breaks, self.dxP95, self.dyP95], [1, 1, 1, 0, 0])
    assert.deepEqual(self.linksMatched, [51, 51])

    // 00 §5.3: T02 keeps C1, C2 and its breaks; its notes move 5.4 / 8.5 staff spaces across.
    const r = compare(path.join(plain, 'T02'), path.join(short, 'T02'))
    assert.deepEqual([r.events, r.midiNotes, r.breaks], [1, 1, 1])
    assert.ok(r.dxMedian > 5 && r.dxMedian < 6, `dxMedian ${r.dxMedian}`)
    assert.ok(r.dxP95 > 8 && r.dxP95 < 9, `dxP95 ${r.dxP95}`)
    assert.equal(r.pass.C4, false)
  })

  test('of all 42 scores each score 1.0 against themselves', { skip: !existsSync(references) && `no captures in ${references}; run fetch.mjs and capture.mjs` }, () => {
    const results = compareAll(references, references)
    assert.equal(results.length, 42)
    for (const r of results) {
      assert.deepEqual(r.pass, { C1: true, C2: true, C3: true, C4: true, C5: true }, r.id)
      assert.deepEqual([r.events, r.midiNotes, r.breaks], [1, 1, 1], r.id)
    }
  })
})
