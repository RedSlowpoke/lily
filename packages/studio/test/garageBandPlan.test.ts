import * as assert from 'node:assert/strict'
import * as fs from 'node:fs/promises'
import * as os from 'node:os'
import * as path from 'node:path'
import { after, before, describe, test } from 'node:test'
import { parseMidi, type Midi } from '@lily/common/web/midi.js'
import type { TimedStaff } from '@lily/common/types'
import { garageBandAudio, garageBandExported, garageBandPlan, leadIn, type PlanSource } from '../src/renderer/garageBandPlan'
import { bpmOfScore } from '../src/renderer/files'
import { timeSignatureOf } from '../src/renderer/tempo'
import type { GarageBandPlan } from '../src/ipc'
import { scoreMidi } from './fakes'
import { realOutcome } from './outcome'

// Export for GarageBand's plan (D162). Runs under `node --test` from out/test/,
// in packages/studio/ (npm run test:unit).

/** packages/studio/, as out/test/ lies two levels below it. */
const studioDir = path.resolve(__dirname, '..', '..')
/** The Rust rewrite's fixture: the sample score and the plan its tests rewrite lilypond's MIDI by. */
const fixtures = path.join(studioDir, 'crates', 'session', 'tests', 'fixtures')

let scratch: string

before(async () => {
  scratch = await fs.realpath(await fs.mkdtemp(path.join(os.tmpdir(), 'lily-studio-garageband-')))
})

after(async () => {
  if (scratch) await fs.rm(scratch, { recursive: true, force: true })
})

function staff(fields: Partial<TimedStaff>): TimedStaff {
  return { id: '', name: '', shortName: '', group: '', groupIndex: -1, clef: 'clefs.G', clefTransposition: 0, voices: [''], ...fields }
}

/**
 * A score of 3/4 at ♩ = 112, 480 ticks a quarter, as lilypond writes it: the
 * control track, a violin, the words, an unnamed piano part with no program
 * written, and drums.
 */
function score(): Midi {
  const midi = parseMidi(scoreMidi(112, 3, 4))
  const note = (track: number, channel: number, program: number) => ({ time: 0, duration: 0.5, track, channel, key: 60, velocity: 0.7, program, gain: 1, pan: 0, detune: 0 })
  return {
    ...midi,
    tracks: [
      { name: 'control track', channels: [], programs: [], notes: 0 },
      { name: 'violin:v', channels: [0], programs: [40], notes: 1 },
      { name: 'lyrics', channels: [], programs: [], notes: 0 },
      { name: 'piano:p', channels: [1], programs: [], notes: 1 },
      { name: 'drums:d', channels: [9], programs: [], notes: 1 },
    ],
    notes: [note(1, 0, 40), note(3, 1, 0), note(4, 9, 0)],
  }
}

const STAVES = [staff({ id: 'violin', name: 'Violin' }), staff({ id: 'words', clef: '', voices: [] }), staff({ id: 'piano' }), staff({ id: 'drums', clef: 'clefs.percussion' })]

/** `\partial 4`: bar 1 a quarter in. */
const PICKUP = [
  { at: 0.25, number: 1 },
  { at: 1, number: 2 },
]

function source(fields: Partial<PlanSource> = {}): PlanSource {
  return { midi: score(), staves: STAVES, bars: PICKUP, meter: { numerator: 3, denominator: 4 }, bpm: undefined, setup: undefined, leaveOutMuted: true, ...fields }
}

describe('the plan of Export for GarageBand (D162)', () => {
  test('the words are left out; every part is named and gets a program; a bar less the pickup leads in', () => {
    assert.deepEqual(garageBandPlan(source()), {
      drop: [2],
      parts: [
        { track: 1, name: 'Violin', program: 40 },
        // Written without a program: the piano, as lilypond plays it.
        { track: 3, name: 'Piano', program: 0 },
        { track: 4, name: 'Drums', program: 0 },
      ],
      tempoRatio: 1,
      // 3/4 less a quarter, at 480 ticks a quarter.
      leadIn: 960,
    } satisfies GarageBandPlan)
  })

  test('the Parts fold\'s instrument wins over the written one; drums keep their kit', () => {
    const plan = garageBandPlan(source({ setup: { parts: { 1: { program: 42 }, 3: { program: 4, volume: 50 }, 4: { program: 48 } } } }))
    assert.deepEqual(
      plan.parts.map(({ track, program }) => [track, program]),
      [
        [1, 42],
        [3, 4],
        [4, 0],
      ],
    )
  })

  test('muted parts are left out behind D54\'s checkbox, unless every part is muted', () => {
    const setup = { parts: { 3: { muted: true } } }
    const without = garageBandPlan(source({ setup }))
    assert.deepEqual(without.drop, [2, 3])
    assert.deepEqual(
      without.parts.map((part) => part.track),
      [1, 4],
    )
    const kept = garageBandPlan(source({ setup, leaveOutMuted: false }))
    assert.deepEqual(kept.drop, [2])
    assert.deepEqual(
      kept.parts.map((part) => part.track),
      [1, 3, 4],
    )
    const all = garageBandPlan(source({ setup: { parts: { 1: { muted: true }, 3: { muted: true }, 4: { muted: true } } } }))
    assert.deepEqual(all.drop, [2])
    assert.equal(all.parts.length, 3)
  })

  test('% BPM: scales the tempos by atTempo\'s ratio: the first tempo over it', () => {
    assert.equal(garageBandPlan(source({ bpm: 113.76 })).tempoRatio, 112 / 113.76)
    assert.equal(garageBandPlan(source({ bpm: 56 })).tempoRatio, 2)
    assert.equal(garageBandPlan(source({ bpm: 112 })).tempoRatio, 1)
    assert.equal(garageBandPlan(source({ bpm: 0 })).tempoRatio, 1)
  })
})

describe('the lead-in (D162)', () => {
  test('none without a pickup, or with no bars known', () => {
    const midi = score()
    assert.equal(leadIn(midi, [{ at: 0, number: 1 }, { at: 0.75, number: 2 }], { numerator: 3, denominator: 4 }), 0)
    assert.equal(leadIn(midi, [], { numerator: 3, denominator: 4 }), 0)
    // A score counting from bar 5 starts with it.
    assert.equal(leadIn(midi, [{ at: 0, number: 5 }], undefined), 0)
  })

  test('a bar of the first meter less the pickup, 4/4 without a meter', () => {
    const midi = score()
    // An eighth before 6/8's bar 1: five eighths.
    assert.equal(leadIn(midi, [{ at: 0.125, number: 1 }], { numerator: 6, denominator: 8 }), 5 * 240)
    assert.equal(leadIn(midi, [{ at: 0.25, number: 1 }], undefined), 3 * 480)
    // A pickup of a whole bar needs none.
    assert.equal(leadIn(midi, [{ at: 0.75, number: 1 }], { numerator: 3, denominator: 4 }), 0)
  })

  test('a pickup at a tempo of its own is counted in ticks, not seconds', () => {
    // A sixteenth at ♩ = 118.13, then ♩ = 88 from bar 1, 480 ticks a quarter (as tempo.test.ts's D113 score).
    const micros = (bpm: number) => Math.round(60e6 / bpm)
    const tempo = (bpm: number) => [0xff, 0x51, 3, (micros(bpm) >> 16) & 0xff, (micros(bpm) >> 8) & 0xff, micros(bpm) & 0xff]
    const events = [0, ...tempo(118.13), 0, 0x90, 60, 100, 120, 0x80, 60, 0, 0, ...tempo(88), 0, 0x90, 62, 100, 0x83, 0x60, 0x80, 62, 0, 0, 0xff, 0x2f, 0]
    const bytes = new Uint8Array([0x4d, 0x54, 0x68, 0x64, 0, 0, 0, 6, 0, 0, 0, 1, 0x01, 0xe0, 0x4d, 0x54, 0x72, 0x6b, 0, 0, 0, events.length, ...events])
    assert.equal(leadIn(parseMidi(bytes), [{ at: 0.0625, number: 1 }], undefined), 4 * 480 - 120)
  })
})

describe('the plan of a real score (D162)', () => {
  test('lilypond\'s MIDI of the Rust rewrite\'s sample score gives the plan its tests rewrite by, but a name', async (t) => {
    const score = path.join(scratch, 'garageband.ly')
    const text = await fs.readFile(path.join(fixtures, 'garageband.ly'), 'utf8')
    await fs.writeFile(score, text)
    const outcome = await realOutcome(t, score)
    if (!outcome) return
    assert.equal(outcome.state, 'ok')
    const plan = garageBandPlan({
      midi: parseMidi(outcome.midiData!),
      staves: outcome.timing?.staves,
      bars: outcome.timing?.bars ?? [],
      meter: timeSignatureOf(outcome.midiData),
      bpm: bpmOfScore(text),
      setup: undefined,
      leaveOutMuted: true,
    })
    const expected = JSON.parse(await fs.readFile(path.join(fixtures, 'garageband.plan.json'), 'utf8')) as GarageBandPlan
    // The map has no staff for the words or the drums, so partsOf names the
    // parts from the tracks, as the Parts fold does: `voice:v` is "V", where
    // the fixture has the spike's "Voice".
    assert.equal(plan.parts[0].name, 'V')
    assert.deepEqual({ ...plan, parts: plan.parts.map((part, index) => (index === 0 ? { ...part, name: 'Voice' } : part)) }, expected)
  })
})

describe('the folder for GarageBand (D162 stage 2)', () => {
  test('sends the files the score names beside it, the offset and the fold\'s mute', () => {
    const text = [
      '\\version "2.24.0"',
      '% Audio: take.wav',
      '% Audio offset: -2.4',
      '% Backing: Song - instrumental.flac',
      '% Music: Song - no_vocals.wav',
      '% Stem: Song - vocals.flac',
      '{ c4 }',
    ].join('\n')
    assert.deepEqual(garageBandAudio(text, true), {
      audio: 'take.wav',
      audioOffset: -2.4,
      audioMuted: true,
      backing: 'Song - instrumental.flac',
      music: 'Song - no_vocals.wav',
      stem: 'Song - vocals.flac',
    })
    // Without lines, nothing; a path is no file beside the score.
    assert.deepEqual(garageBandAudio('% Audio: ../elsewhere.wav\n{ c4 }', false), {
      audio: undefined,
      audioOffset: 0,
      audioMuted: false,
      backing: undefined,
      music: undefined,
      stem: undefined,
    })
  })

  test('says what the export wrote and what it left out', () => {
    const folder = '/songs/Song (GarageBand)'
    assert.equal(garageBandExported({ file: folder, leftOut: 0, wavs: [], leftOutFiles: [] }), 'Exported Song (GarageBand).')
    assert.equal(
      garageBandExported({ file: folder, leftOut: 1, wavs: ['take.wav', 'Instrumental.wav', 'Vocals.wav'], leftOutFiles: [] }),
      'Exported Song (GarageBand) with take.wav, Instrumental.wav and Vocals.wav, without the muted part.',
    )
    assert.equal(
      garageBandExported({
        file: folder,
        leftOut: 2,
        wavs: ['take.wav'],
        leftOutFiles: [{ file: 'Song - vocals.flac', why: 'ffmpeg is not installed (brew install ffmpeg)' }],
      }),
      'Exported Song (GarageBand) with take.wav, without the 2 muted parts. Left out Song - vocals.flac: ffmpeg is not installed (brew install ffmpeg).',
    )
    // The web downloads the .mid alone.
    assert.equal(garageBandExported({ file: 'Song.mid', leftOut: 0 }), 'Exported Song.mid for GarageBand')
  })
})
