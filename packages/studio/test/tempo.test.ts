import * as assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { parseMidi } from '@lily/common/web/midi.js'
import { withScoreLines } from '../src/renderer/files'
import { atTempo } from '../src/renderer/playbackSetup'
import type { TextEdit } from '../src/renderer/midiBlock'
import { FOUND, scoreMidi } from './fakes'
import { barOneAt, barOneTime, bpmAt, scoreMeter, tempoLines, tempoProgress, tempoStatus, tickAt, timeSignatureOf } from '../src/renderer/tempo'

// From the recording (D159). Runs under `node --test` from out/test/ (npm run test:unit).

/** A pickup of a sixteenth at ♩ = `pickup`, then ♩ = `main` from bar 1, 480 ticks to the quarter (`ticks` = a sixteenth). */
function twoTempos(pickup: number, main: number, ticks: number): Uint8Array {
  const micros = (bpm: number) => Math.round(60e6 / bpm)
  const tempo = (bpm: number) => [0xff, 0x51, 3, (micros(bpm) >> 16) & 0xff, (micros(bpm) >> 8) & 0xff, micros(bpm) & 0xff]
  const events = [0, ...tempo(pickup), 0, 0x90, 60, 100, ticks, 0x80, 60, 0, 0, ...tempo(main), 0, 0x90, 62, 100, 0x83, 0x60, 0x80, 62, 0, 0, 0xff, 0x2f, 0]
  return new Uint8Array([0x4d, 0x54, 0x68, 0x64, 0, 0, 0, 6, 0, 0, 0, 1, 0x01, 0xe0, 0x4d, 0x54, 0x72, 0x6b, 0, 0, 0, events.length, ...events])
}

/** `text` with `edits` applied in one step, as the editor applies them. */
const applied = (text: string, edits: TextEdit[]) =>
  [...edits].sort((a, b) => b.offset - a.offset).reduce((result, { offset, length, text: put }) => result.slice(0, offset) + put + result.slice(offset + length), text)

/** What one undo of `edits` puts back: each edit's new text replaced by the text it replaced, as Monaco's inverse edits do. */
function undone(text: string, edits: TextEdit[]): string {
  const sorted = [...edits].sort((a, b) => a.offset - b.offset)
  let shift = 0
  const inverse = sorted.map(({ offset, length, text: put }) => {
    const back = { offset: offset + shift, length: put.length, text: text.slice(offset, offset + length) }
    shift += put.length - length
    return back
  })
  return applied(applied(text, edits), inverse)
}

describe('the score\'s meter, for lily-transcribe (D159)', () => {
  test('the first time signature is read from the MIDI file', () => {
    assert.deepEqual(timeSignatureOf(scoreMidi(114)), { numerator: 4, denominator: 4 })
    assert.deepEqual(timeSignatureOf(scoreMidi(90, 6, 8)), { numerator: 6, denominator: 8 })
    assert.equal(timeSignatureOf(new Uint8Array([0x4d, 0x54, 0x68, 0x64])), undefined)
    assert.equal(timeSignatureOf(undefined), undefined)
  })

  test('a compound meter counts dotted beats, in quarter notes', () => {
    assert.deepEqual(scoreMeter(114, { numerator: 4, denominator: 4 }), { bpm: 114, beatsPerBar: 4, beat: 1 })
    assert.deepEqual(scoreMeter(90, { numerator: 6, denominator: 8 }), { bpm: 90, beatsPerBar: 2, beat: 1.5 })
    assert.deepEqual(scoreMeter(60, { numerator: 12, denominator: 8 }), { bpm: 60, beatsPerBar: 4, beat: 1.5 })
    assert.deepEqual(scoreMeter(100, { numerator: 3, denominator: 4 }), { bpm: 100, beatsPerBar: 3, beat: 1 })
    assert.deepEqual(scoreMeter(70, { numerator: 2, denominator: 2 }), { bpm: 70, beatsPerBar: 2, beat: 2 })
    // lilypond's 4/4 without one.
    assert.deepEqual(scoreMeter(120, undefined), { bpm: 120, beatsPerBar: 4, beat: 1 })
  })
})

describe('the lines From the recording writes (D159)', () => {
  const midi = parseMidi(scoreMidi(114))

  test('bar 1 is at the start, or where a pickup ends, at the new tempo', () => {
    assert.equal(barOneTime([{ at: 0, number: 1 }, { at: 1, number: 2 }], midi), 0)
    // \partial 4: lilypond's bar 1 begins a quarter in.
    assert.ok(Math.abs(barOneTime([{ at: 0.25, number: 1 }, { at: 1.25, number: 2 }], midi) - 60 / 114) < 1e-5, "µs a quarter, as the file has it")
    assert.ok(Math.abs(barOneAt([{ at: 0.25, number: 1 }], midi)(113.76) - 60 / 113.76) < 1e-5)
    // A score counting from bar 5 starts with it; no map is no pickup.
    assert.equal(barOneTime([{ at: 0, number: 5 }], midi), 0)
    assert.equal(barOneTime([], midi), 0)
  })

  test('the tempo to the hundredth, and the offset that puts the score\'s bar 1 on the recording\'s', () => {
    assert.deepEqual(tempoLines(FOUND, { first: 114, main: 114 }, barOneAt([{ at: 0, number: 1 }], midi)), { BPM: '113.76', 'Audio offset': '-0.32' })
    // With a pickup of a quarter at ♩ = 113.76 (0.527 s), the recording waits 0.21 s.
    assert.deepEqual(tempoLines(FOUND, { first: 114, main: 114 }, barOneAt([{ at: 0.25, number: 1 }], midi)), { BPM: '113.76', 'Audio offset': '0.21' })
  })

  test('a time is a tick of the file, across a tempo change and at another tempo', () => {
    const transcribed = parseMidi(twoTempos(118.13, 88, 120))
    assert.equal(tickAt(transcribed, 0), 0)
    // The sixteenth at ♩ = 118.13, then a quarter at ♩ = 88.
    assert.ok(Math.abs(tickAt(transcribed, (0.25 * 60) / 118.13) - 120) < 1e-3)
    assert.ok(Math.abs(tickAt(transcribed, (0.25 * 60) / 118.13 + 60 / 88) - 600) < 1e-3)
    const slow = barOneAt([{ at: 0.0625, number: 1 }], transcribed)(59.065)
    assert.ok(Math.abs(tickAt(atTempo(transcribed, 59.065), slow) - 120) < 1e-3)
  })

  test('a pickup at a tempo of its own: % BPM: is the first tempo, scaled as bar 1\'s should be', () => {
    // D113's transcribed score: the pickup at ♩ = 118.13 (\tempo 64 = 1890), bar 1 at ♩ = 88.
    const transcribed = parseMidi(twoTempos(118.13, 88, 120))
    assert.equal(bpmAt(transcribed, 0), 118.13)
    assert.equal(bpmAt(transcribed, barOneTime([{ at: 0.0625, number: 1 }], transcribed)), 88)
    // 88.2 at bar 1 is 118.4 at the first tempo, and the pickup is then 0.127 s long, where the recording's bar 1 is,
    // as Song B's piano.ly gets.
    const lines = tempoLines({ bpm: 88.2, barOne: 0.127 }, { first: 118.13, main: 88 }, barOneAt([{ at: 0.0625, number: 1 }], transcribed))
    assert.deepEqual(lines, { BPM: '118.4', 'Audio offset': undefined })
  })

  test('the score\'s own tempo and an offset of 0 take their lines out', () => {
    assert.deepEqual(tempoLines({ bpm: 114, barOne: 0.004 }, { first: 114, main: 114 }, () => 0), { BPM: undefined, 'Audio offset': undefined })
  })

  test('both lines are one edit, which one undo takes back exactly, added or changed', () => {
    const lines = tempoLines(FOUND, { first: 114, main: 114 }, () => 0)
    const added = '\\version "2.24.0"\n% Audio: song.mp3\n{ c4 }\n'
    const addedEdits = withScoreLines(added, lines)
    assert.equal(applied(added, addedEdits), '\\version "2.24.0"\n% Audio: song.mp3\n% BPM: 113.76\n% Audio offset: -0.32\n{ c4 }\n')
    assert.equal(undone(added, addedEdits), added)
    const changed = '\\version "2.24.0"\r\n% Audio: song.mp3\r\n% Audio offset: -0.25\r\n% BPM: 120\r\n{ c4 }\r\n'
    const changedEdits = withScoreLines(changed, lines)
    assert.equal(applied(changed, changedEdits), '\\version "2.24.0"\r\n% Audio: song.mp3\r\n% Audio offset: -0.32\r\n% BPM: 113.76\r\n{ c4 }\r\n')
    assert.equal(undone(changed, changedEdits), changed)
    // A line taken out comes back too.
    const removed = withScoreLines(changed, { BPM: undefined, 'Audio offset': '-0.32' })
    assert.equal(applied(changed, removed), '\\version "2.24.0"\r\n% Audio: song.mp3\r\n% Audio offset: -0.32\r\n{ c4 }\r\n')
    assert.equal(undone(changed, removed), changed)
  })
})

describe('what the status line says (D159)', () => {
  const meter = { bpm: 114, beatsPerBar: 4, beat: 1 }
  const four = { numerator: 4, denominator: 4 }

  test('what was found and how sure it is', () => {
    assert.equal(tempoStatus(FOUND, meter, four), '♩ = 113.76, 4/4, bar 1 at 0.32 s; 99 % of the beats on one tempo')
  })

  test('another metrical level, another meter heard, a tempo that drifts', () => {
    const doubled = { ...FOUND, heardBpm: 227.52, heardBeatsPerBar: 2, steady: false, share: 0.31, residualMs: 620.6 }
    assert.equal(
      tempoStatus(doubled, meter, four),
      '♩ = 113.76, 4/4, bar 1 at 0.32 s; 31 % of the beats on one tempo; heard ♩ = 227.52, taken as 113.76 to match the score\'s 114; ' +
        'the downbeats heard make 2 beats a bar, not the score\'s 4; the tempo drifts: one tempo keeps it within 0.62 s',
    )
  })

  test('its stages', () => {
    assert.equal(tempoProgress('song.mp3', 'decode'), 'Listening to song.mp3: reading it…')
    assert.equal(tempoProgress('song.mp3', 'beats'), 'Listening to song.mp3: finding the beats…')
  })
})
