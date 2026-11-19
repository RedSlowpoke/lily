// C2b (07 §4.3): each .midi compared by its bytes, the creator text left out,
// as it is and with the note-offs at one tick sorted. Synthetic files, then
// the fetched corpus's references against themselves (skipped when it has not
// been captured), then R4's gate (MS5 step 8): the engine's files of the 42,
// compiled natively, scored by compare() against LilyPond's, equal as they
// are (skipped without LilyPond 2.26.0, cargo and the capture).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, describe, test } from 'node:test'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { lilypondVersion } from '../capture.mjs'
import { compare, compareAll, smfEvents } from '../score.mjs'

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-c2b-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

const references = path.join(corpusDir(), 'ref')

const u32 = (n) => [n >>> 24, (n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff]
const chunk = (tag, body) => [...Buffer.from(tag), ...u32(body.length), ...body]
const text = (s) => [0, 0xff, 0x01, s.length, ...Buffer.from(s)]

// LilyPond's shape: a control track with the creator text, then one staff of
// a chord whose offs come in `offs` order at 384, then `extra` events.
function file({ creator = 'LilyPond 2.26.0', offs = [0x40, 0x43], extra = [] } = {}) {
  const control = [...text('creator: '), ...text(creator.padEnd(30)), 0, 0xff, 0x51, 3, 0x07, 0xa1, 0x20, 0, 0xff, 0x2f, 0]
  const staff = [0, 0x90, 0x40, 0x5a, 0, 0x90, 0x43, 0x5a, ...offs.flatMap((k, i) => [...(i ? [0] : [0x83, 0x00]), 0x90, k, 0]), ...extra, 0, 0xff, 0x2f, 0]
  return Buffer.from([...chunk('MThd', [0, 1, 0, 2, 1, 0x80]), ...chunk('MTrk', control), ...chunk('MTrk', staff)])
}

function capture(name, files) {
  const dir = path.join(scratch, name)
  mkdirSync(dir, { recursive: true })
  for (const [f, bytes] of Object.entries(files)) writeFileSync(path.join(dir, f), bytes)
  return dir
}

describe('C2b on synthetic files', () => {
  const ref = capture('ref', { 'score.midi': file() })

  test('a file against itself is equal as it is', () => {
    const r = compare(ref, ref)
    assert.deepEqual([r.midiBytes, r.goal.C2b], [[1, 1, 1], true])
  })

  test('the creator text is left out', () => {
    const r = compare(ref, capture('creator', { 'score.midi': file({ creator: 'lily-engine 0.1.0' }) }))
    assert.deepEqual([r.midiBytes, r.goal.C2b], [[1, 1, 1], true])
    const events = smfEvents(file())
    assert.ok(!events.includes(Buffer.from('creator: ').toString('hex')) && !events.includes(Buffer.from('LilyPond').toString('hex')))
    assert.ok(events.includes('0 ff5103'), 'the rest of the control track is kept')
  })

  test('note-offs at one tick in another order are equal once sorted', () => {
    const r = compare(ref, capture('swapped', { 'score.midi': file({ offs: [0x43, 0x40] }) }))
    assert.deepEqual([r.midiBytes, r.goal.C2b, r.midiNotes], [[0, 1, 1], true, 1])
  })

  test('any other difference is not', () => {
    const later = file({ offs: [0x40], extra: [1, 0x90, 0x43, 0] })
    let r = compare(ref, capture('later', { 'score.midi': later }))
    assert.deepEqual([r.midiBytes, r.goal.C2b], [[0, 0, 1], false])
    const louder = file(); louder[louder.indexOf(0x5a)] = 0x5b
    r = compare(ref, capture('louder', { 'score.midi': louder }))
    assert.deepEqual([r.midiBytes, r.goal.C2b], [[0, 0, 1], false])
    // Running status writes the same notes in other bytes.
    const running = Buffer.from([...chunk('MThd', [0, 1, 0, 2, 1, 0x80]), ...chunk('MTrk', [...text('creator: '), ...text('x'.padEnd(30)), 0, 0xff, 0x2f, 0]),
      ...chunk('MTrk', [0, 0x90, 0x40, 0x5a, 0, 0x43, 0x5a, 0x83, 0x00, 0x40, 0, 0, 0x43, 0, 0, 0xff, 0x2f, 0])])
    r = compare(ref, capture('running', { 'score.midi': running }))
    assert.deepEqual([r.midiBytes, r.goal.C2b, r.midiNotes], [[0, 0, 1], false, 1])
  })

  test('a file missing on either side is not equal', () => {
    let r = compare(ref, capture('none', {}))
    assert.deepEqual([r.midiBytes, r.goal.C2b], [[0, 0, 1], false])
    r = compare(ref, capture('more', { 'score.midi': file(), 'score-1.midi': file() }))
    assert.deepEqual([r.midiBytes, r.goal.C2b], [[1, 1, 2], false])
    // No MIDI in either: nothing to judge, and nothing wrong.
    r = compare(capture('silent', {}), capture('silent2', {}))
    assert.deepEqual([r.midiBytes, r.goal.C2b], [[0, 0, 0], true])
  })
})

describe('C2b on the references', { skip: !existsSync(references) && 'the corpus has not been captured' }, () => {
  test('each reference is equal to itself as it is', () => {
    const ids = readdirSync(references).filter((id) => /^[TM]\d\d$/.test(id))
    assert.equal(ids.length, 42)
    for (const id of ids) {
      const r = compare(path.join(references, id), path.join(references, id))
      assert.ok(r.midiBytes[2] >= 1 && r.midiBytes[0] === r.midiBytes[2] && r.goal.C2b, `${id} ${r.midiBytes}`)
    }
  })

  test('two note-offs of T24 swapped at one tick are equal only once sorted', () => {
    const dir = path.join(scratch, 'T24')
    cpSync(path.join(references, 'T24'), dir, { recursive: true })
    const name = readdirSync(dir).find((f) => f.endsWith('.midi'))
    const bytes = readFileSync(path.join(dir, name))
    // The first pair of note-offs with no delta between them, on the same channel.
    let at = -1
    for (let p = 0; p + 7 < bytes.length && at < 0; p++) {
      const [a, k1, v1, d, b, k2, v2] = bytes.subarray(p, p + 7)
      if ((a & 0xf0) === 0x90 && a === b && v1 === 0 && d === 0 && v2 === 0 && k1 !== k2 && k1 < 0x80 && k2 < 0x80) at = p
    }
    assert.ok(at > 0)
    ;[bytes[at + 1], bytes[at + 5]] = [bytes[at + 5], bytes[at + 1]]
    writeFileSync(path.join(dir, name), bytes)
    const r = compare(path.join(references, 'T24'), dir)
    assert.deepEqual([r.midiBytes, r.goal.C2b, r.pass.C2], [[0, 1, 1], true, true])
  })
})

const gate = lilypondVersion() !== '2.26.0' ? 'LilyPond 2.26.0 is not installed'
  : spawnSync('cargo', ['+stable', '--version']).status !== 0 ? 'cargo +stable is not installed'
    : !existsSync(references) ? 'the corpus has not been captured' : false

test("R4's gate: the engine's MIDI files of the 42 are LilyPond's as they are (C2b), and C2 = 1.0", { skip: gate, timeout: 900_000 }, () => {
  // R4's 42; Y01–Y90 (MS8) are y8.test.mjs'.
  const rows = corpus().filter((row) => /^[TM]\d\d$/.test(row.id) && existsSync(path.join(references, row.id)))
  assert.equal(rows.length, 42)
  const out = path.join(scratch, 'engine')
  mkdirSync(out, { recursive: true })
  const run = runExample('compile', ['--out', out, ...rows.map((row) => row.path)], { encoding: 'utf8' })
  // 1: a file failed, as T03 does in LilyPond and T15 at ly:expect-warning.
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
  // Each score's files alone in a directory of its ID, as compareAll takes them.
  const candidates = path.join(scratch, 'candidates')
  for (const row of rows) {
    const stem = path.basename(row.path, '.ly')
    const dir = path.join(candidates, row.id)
    mkdirSync(dir, { recursive: true })
    for (const f of readdirSync(out).filter((f) => f === `${stem}.midi` || (f.startsWith(`${stem}-`) && f.endsWith('.midi')))) copyFileSync(path.join(out, f), path.join(dir, f))
  }
  const results = compareAll(references, candidates)
  assert.equal(results.length, 42)
  for (const r of results) {
    assert.ok(r.midiBytes[2] >= 1, r.id)
    assert.deepEqual([r.midiBytes[0], r.goal.C2b, r.pass.C2], [r.midiBytes[2], true, true], `${r.id} ${r.midiBytes}`)
  }
})
