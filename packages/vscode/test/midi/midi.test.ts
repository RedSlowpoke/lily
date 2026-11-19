import * as assert from 'node:assert'
import * as fs from 'node:fs'
import * as path from 'node:path'
import { describe, mock, test } from 'node:test'

// The pure half of @lily/common's web/midi.js (DECISIONS D24), loaded as the webviews load it:
// the file itself, not a bundle of it. There is no AudioContext here, so the
// player runs against a clock that the tests turn.

interface Note {
  time: number
  duration: number
  track: number
  channel: number
  key: number
  velocity: number
  program: number
  gain: number
  pan: number
  detune: number
  legato: boolean
}

interface Midi {
  format: number
  division: number
  title: string
  duration: number
  notes: Note[]
  tracks: Array<{ name: string; channels: number[]; programs: number[]; notes: number }>
}

interface Player {
  state: 'stopped' | 'playing' | 'paused'
  position: number
  scheduled: number
  latency: number
  duration: number
  suspended: boolean
  starting: boolean
  load(midi: Midi | undefined): void
  play(): Promise<boolean>
  pause(): void
  stop(): void
  seek(seconds: number): Promise<void>
  trackVolume(track: number): number
  setTrackVolume(track: number, volume: number): void
  volume: number
  setVolume(volume: number): void
  dispose(): void
}

interface LilyMidi {
  parseMidi(bytes: Uint8Array): Midi
  instrumentName(program: number): string
  trackInstruments(track: Midi['tracks'][number]): string
  voiceOf(program: number): string
  drumOf(key: number): { sound: string; pitch?: number }
  frequencyOf(key: number, detune?: number): number
  firstNoteFrom(notes: Array<{ time: number }>, time: number): number
  formatTime(seconds: number): string
  momentTime(midi: Midi, at: number, grace?: number): number
  Player: new (options: { createContext: () => unknown; onChange?: () => void }) => Player
  Synth: new (context: unknown) => { play(note: Note, when: number, skipped?: number): void }
}

const root = path.resolve(__dirname, '../../..')
// eslint-disable-next-line @typescript-eslint/no-require-imports
const midi: LilyMidi = require(path.join(root, '../common/web/midi.js'))

// ---- a Standard MIDI File writer, just enough for the tests ---------------------

function variable(value: number): number[] {
  const bytes = [value & 0x7f]
  while ((value >>= 7) > 0) bytes.unshift((value & 0x7f) | 0x80)
  return bytes
}

/** `events` are `[delta, ...bytes]`; the end of track is added. */
function track(...events: number[][]): number[] {
  const body = [...events.flatMap(([delta, ...bytes]) => [...variable(delta), ...bytes]), 0, 0xff, 0x2f, 0]
  return [0x4d, 0x54, 0x72, 0x6b, 0, 0, body.length >> 8, body.length & 0xff, ...body]
}

function file(division: number, tracks: number[][], format = 1): Uint8Array {
  const header = [0x4d, 0x54, 0x68, 0x64, 0, 0, 0, 6, 0, format, 0, tracks.length, division >> 8, division & 0xff]
  return Uint8Array.from([...header, ...tracks.flat()])
}

const tempo = (delta: number, bpm: number): number[] => {
  const micros = Math.round(60e6 / bpm)
  return [delta, 0xff, 0x51, 3, micros >> 16, (micros >> 8) & 0xff, micros & 0xff]
}
const name = (text: string): number[] => [0, 0xff, 0x03, text.length, ...Buffer.from(text)]

describe('parseMidi', () => {
  test('reads what lilypond writes: title, tracks, tempo, notes in time order', () => {
    const parsed = midi.parseMidi(fs.readFileSync(path.join(root, 'test/fixtures/sample.midi')))
    assert.deepStrictEqual(
      [parsed.format, parsed.division, parsed.title, parsed.duration, parsed.notes.length],
      [1, 384, 'Sample', 15, 26],
    )
    // Two staves of eight bars in 3/4 at 96 to the crotchet; the lyrics are a silent track.
    assert.deepStrictEqual(parsed.tracks.map((entry) => entry.notes), [0, 18, 0, 8])
    assert.deepStrictEqual(parsed.tracks[1], { name: ':tune', channels: [0], programs: [0], notes: 18 })
    const [first, bass] = parsed.notes
    assert.deepStrictEqual([first.time, first.duration, first.key], [0, 0.625, 67])
    assert.deepStrictEqual([bass.time, bass.duration, bass.key, bass.channel], [0, 1.875, 43, 2])
    assert.ok(parsed.notes.every((note, index) => index === 0 || parsed.notes[index - 1].time <= note.time))
  })

  test('a tempo change applies from its tick on, in every track', () => {
    const parsed = midi.parseMidi(
      file(480, [
        track(tempo(0, 120), tempo(960, 60)),
        track([0, 0x90, 60, 100], [480, 0x80, 60, 0], [480, 0x90, 62, 100], [480, 0x80, 62, 0]),
      ]),
    )
    assert.deepStrictEqual(
      parsed.notes.map(({ time, duration }) => [time, duration]),
      [
        [0, 0.5],
        [1, 1],
      ],
    )
    assert.strictEqual(parsed.duration, 2)
  })

  test('a moment of the score is heard through the tempo map; a grace note shortly before its note', () => {
    const parsed = midi.parseMidi(
      file(480, [
        track(tempo(0, 120), tempo(960, 60)),
        track([0, 0x90, 60, 100], [480, 0x80, 60, 0], [480, 0x90, 62, 100], [480, 0x80, 62, 0]),
      ]),
    )
    // Two crotchets at 120, then one at 60 (DECISIONS D26).
    assert.deepStrictEqual([0, 0.25, 0.5, 0.75].map((at) => midi.momentTime(parsed, at)), [0, 0.5, 1, 2])
    // A grace quaver takes 11/48 of its half second.
    const grace = midi.momentTime(parsed, 0.5, -1 / 8)
    assert.ok(Math.abs(grace - (1 - (0.25 * 11) / 48)) < 1e-9, String(grace))
    // Bar 2 of the 3/4 sample at 96 to the crotchet begins where its bass note ends.
    const sample = midi.parseMidi(fs.readFileSync(path.join(root, 'test/fixtures/sample.midi')))
    assert.strictEqual(midi.momentTime(sample, 0.75), 1.875)
  })

  test('running status, and a note-on of velocity 0 as the note-off', () => {
    const parsed = midi.parseMidi(file(96, [track([0, 0x90, 60, 64], [96, 60, 0], [0, 64, 127], [96, 64, 0])], 0))
    assert.deepStrictEqual(
      parsed.notes.map(({ key, time, duration, velocity }) => [key, time, duration, velocity]),
      [
        [60, 0, 0.5, 64 / 127],
        [64, 0.5, 0.5, 1],
      ],
    )
  })

  test('a note begins with the state of its channel: program, volume, pan, bend', () => {
    const parsed = midi.parseMidi(
      file(96, [
        track(
          name('violin:one'),
          [0, 0xc1, 40],
          [0, 0xb1, 7, 127],
          [0, 0xb1, 11, 64],
          [0, 0xb1, 10, 0],
          [0, 0xe1, 0, 0x50], // a quarter tone up: half of the 200 cents above 8192
          [0, 0x91, 69, 127],
          [96, 0x81, 69, 0],
          [0, 0x99, 38, 100],
          [10, 0x89, 38, 0],
        ),
      ]),
    )
    const [violin, snare] = parsed.notes
    assert.deepStrictEqual(
      [violin.program, violin.gain, violin.pan, violin.detune],
      [40, 64 / 127, -1, 50],
    )
    assert.strictEqual(snare.channel, 9)
    assert.deepStrictEqual(parsed.tracks[0], { name: 'violin:one', channels: [1, 9], programs: [40], notes: 2 })
    assert.strictEqual(midi.trackInstruments(parsed.tracks[0]), 'violin, drums')
    assert.strictEqual(parsed.title, 'violin:one')
  })

  test('a program set in one track is heard in another that uses the channel', () => {
    const parsed = midi.parseMidi(
      file(96, [track(name('control track'), [0, 0xc0, 73]), track([0, 0x90, 72, 90], [96, 0x80, 72, 0])]),
    )
    assert.strictEqual(parsed.notes[0].program, 73)
    assert.strictEqual(parsed.title, '', "lilypond's name for an untitled score is not a title")
  })

  test('the sustain pedal holds a released note until it is lifted', () => {
    const parsed = midi.parseMidi(
      file(96, [
        track(
          [0, 0xb0, 64, 127],
          [0, 0x90, 60, 90],
          [96, 0x80, 60, 0],
          [0, 0x90, 60, 90], // struck again under the pedal: the first one ends here
          [96, 0x80, 60, 0],
          [96, 0xb0, 64, 0],
        ),
      ]),
    )
    assert.deepStrictEqual(
      parsed.notes.map(({ time, duration }) => [time, duration]),
      [
        [0, 1.5],
        [0.5, 1],
      ],
    )
  })

  test('a note that begins as the one before it ends, alone on its track and channel, is legato (D150)', () => {
    const parsed = midi.parseMidi(
      file(96, [
        track(
          [0, 0x90, 60, 90], [96, 0x80, 60, 0], [0, 0x90, 62, 90], // a line
          [96, 0x80, 62, 0], [48, 0x90, 64, 90], // after a rest
          [96, 0x80, 64, 0], [0, 0x90, 65, 90], [0, 0x90, 69, 90], // into a chord
          [96, 0x80, 65, 0], [0, 0x80, 69, 0], [0, 0x90, 67, 90], // out of it
          [96, 0x80, 67, 0],
        ),
        track([192, 0x91, 50, 90], [96, 0x81, 50, 0]), // another track, begins as the line's second note ends
      ]),
    )
    assert.deepStrictEqual(
      parsed.notes.map((note) => [note.key, note.legato]),
      [[60, false], [62, true], [50, false], [64, false], [65, false], [69, false], [67, false]],
    )
  })

  test('a note that is never switched off lasts to the end', () => {
    const parsed = midi.parseMidi(file(96, [track([0, 0x90, 60, 90]), track([192, 0xff, 0x06, 0])]))
    assert.deepStrictEqual([parsed.notes[0].duration, parsed.duration], [1, 1])
  })

  test('says why a file cannot be played', () => {
    const smpte = file(96, [track()])
    smpte[12] = 0xe7
    for (const [bytes, reason] of [
      [Uint8Array.from(Buffer.from('RIFF....WAVEfmt ')), /MThd header is missing/],
      [file(96, [track()], 2), /format 2 is not supported/],
      [smpte, /SMPTE/],
      [file(96, []), /no tracks/],
      [file(96, [track([0, 0x90, 60, 90])]).slice(0, -6), /ends inside an event/],
      [file(96, [track([0, 60, 90])]), /without a status byte/],
    ] as const) {
      assert.throws(() => midi.parseMidi(bytes), reason)
    }
  })
})

describe('General MIDI', () => {
  test('programs have the names that midiInstrument takes', () => {
    assert.deepStrictEqual([0, 40, 73, 127].map(midi.instrumentName), ['acoustic grand', 'violin', 'flute', 'gunshot'])
    assert.strictEqual(midi.instrumentName(200), 'program 200')
  })

  test('every program and every percussion key has a sound', () => {
    const voices = new Set(Array.from({ length: 128 }, (_, program) => midi.voiceOf(program)))
    assert.ok(!voices.has(undefined as never))
    assert.deepStrictEqual(
      [0, 19, 24, 40, 45, 56, 71, 73].map(midi.voiceOf),
      ['piano', 'organ', 'pluck', 'strings', 'pluck', 'brass', 'reed', 'flute'],
    )
    assert.deepStrictEqual([36, 38, 42, 45, 49, 76].map((key) => midi.drumOf(key).sound), [
      'kick', 'snare', 'hat', 'tom', 'cymbal', 'click',
    ])
  })

  test('pitch: A4 is 440 Hz, and detune is in cents', () => {
    assert.strictEqual(midi.frequencyOf(69), 440)
    assert.strictEqual(midi.frequencyOf(57), 220)
    assert.ok(Math.abs(midi.frequencyOf(68, 100) - 440) < 1e-9)
  })
})

describe('Player', () => {
  test('formatTime and firstNoteFrom', () => {
    assert.deepStrictEqual([0, 9.9, 75, 3600, NaN].map(midi.formatTime), ['0:00', '0:09', '1:15', '60:00', '0:00'])
    // Before the music, Karaoke's silence counts down (D149).
    assert.deepStrictEqual([-4.09, -1, -0.2].map(midi.formatTime), ['-0:05', '-0:01', '-0:01'])
    const notes = [0, 1, 1, 2.5].map((time) => ({ time }))
    assert.deepStrictEqual([0, 0.5, 1, 2.6].map((time) => midi.firstNoteFrom(notes, time)), [0, 1, 1, 4])
  })

  // Web Audio as far as the synthesizer touches it. Every node is the same
  // stand-in; what is recorded is when oscillators start, which is the schedule.
  function fakeContext(state: 'running' | 'suspended') {
    const starts: number[] = []
    const param = () => ({
      value: 0,
      targets: [] as number[],
      setValueAtTime() {},
      linearRampToValueAtTime() {},
      exponentialRampToValueAtTime() {},
      setTargetAtTime(value: number) {
        this.targets.push(value)
      },
      cancelScheduledValues() {},
    })
    const node = (): Record<string, unknown> => ({
      gain: param(), frequency: param(), pan: param(), Q: param(), threshold: param(), knee: param(),
      ratio: param(), attack: param(), release: param(),
      connect: (next: unknown) => next,
      disconnect() {},
      setPeriodicWave() {},
      start: (when: number) => starts.push(Math.round(when * 1000) / 1000),
      stop() {},
    })
    const context = {
      state,
      currentTime: 10,
      sampleRate: 8000,
      destination: node(),
      createGain: node, createOscillator: node, createBiquadFilter: node, createStereoPanner: node,
      createDynamicsCompressor: node, createBufferSource: node,
      createPeriodicWave: () => ({}),
      createBuffer: () => ({ getChannelData: () => new Float32Array(8) }),
      resume: () => new Promise<void>(() => {}), // what a browser does while it waits for a click
      close: async () => {},
    }
    return { context, starts }
  }

  const song = () =>
    midi.parseMidi(
      file(96, [track(tempo(0, 60), [0, 0x90, 60, 90], [96, 0x80, 60, 0], [0, 0x90, 62, 90], [96, 0x80, 62, 0], [0, 0x90, 64, 90], [96, 0x80, 64, 0])]),
    )

  test('a sung line is one tone gliding from note to note; a struck one starts each (D150)', () => {
    for (const [program, expected] of [[53, [0]], [0, [0, 1, 2]]] as const) {
      const { context, starts } = fakeContext('running')
      const notes = song().notes.map((note) => ({ ...note, program }))
      const synth = new midi.Synth(context)
      for (const note of notes) synth.play(note, note.time)
      assert.deepStrictEqual(starts, expected, `program ${program}`)
    }
  })

  test('a pan that a sung line brings mid-line still applies to its tone (D150)', () => {
    const { context, starts } = fakeContext('running')
    const pans: number[] = []
    const panner = context.createStereoPanner
    const synth = new midi.Synth({
      ...context,
      createStereoPanner: () => {
        const node = panner()
        return { ...node, pan: { ...(node.pan as object), setValueAtTime: (value: number) => pans.push(value) } }
      },
    })
    const notes = song().notes.map((note, i) => ({ ...note, program: 53, pan: i === 1 ? 0.5 : 0 }))
    for (const note of notes) synth.play(note, note.time)
    assert.deepStrictEqual([starts, pans], [[0], [0, 0.5, 0]])
  })

  test('schedules ahead of the audio clock, pauses, seeks and stops at the end', async () => {
    mock.timers.enable({ apis: ['setInterval', 'setTimeout'] })
    try {
      const { context, starts } = fakeContext('running')
      const states: string[] = []
      const player = new midi.Player({ createContext: () => context, onChange: () => states.push(player.state) })
      assert.strictEqual(await player.play(), false, 'nothing is loaded')

      player.load(song())
      assert.deepStrictEqual([player.state, player.position, player.duration], ['stopped', 0, 3])
      assert.strictEqual(await player.play(), true)
      // Music time 0 is a little after "now"; only the first note is within the look-ahead.
      assert.deepStrictEqual(starts, [10.06])

      context.currentTime = 10.06 + 0.7
      mock.timers.tick(50)
      assert.deepStrictEqual(starts, [10.06, 11.06])
      assert.ok(Math.abs(player.position - 0.7) < 1e-9)

      player.pause()
      context.currentTime = 50
      assert.deepStrictEqual([player.state, Math.round(player.position * 10)], ['paused', 7])

      // Into the last note: it is struck, so it is not started in the middle.
      await player.seek(2.5)
      assert.deepStrictEqual([player.state, player.position], ['paused', 2.5])
      assert.strictEqual(await player.play(), true)
      assert.strictEqual(starts.length, 2)

      context.currentTime = 50.06 + 0.5
      mock.timers.tick(50)
      assert.deepStrictEqual([player.state, player.position], ['stopped', 0])
      assert.deepStrictEqual([...new Set(states)].sort(), ['paused', 'playing', 'stopped'])
    } finally {
      mock.timers.reset()
    }
  })

  test('plays a silence before the music from a place below 0 (D149)', async () => {
    mock.timers.enable({ apis: ['setInterval', 'setTimeout'] })
    try {
      const { context, starts } = fakeContext('running')
      const player = new midi.Player({ createContext: () => context })
      player.load(song())
      await player.seek(-2)
      assert.deepStrictEqual([player.state, player.position], ['paused', -2])
      assert.strictEqual(await player.play(), true)
      assert.deepStrictEqual(starts, [], 'nothing within the look-ahead yet')
      context.currentTime = 10.06 + 1.9
      mock.timers.tick(50)
      assert.deepStrictEqual(starts, [12.06], 'the first note 2 s after the start')
      assert.ok(Math.abs(player.position + 0.1) < 1e-9)
      await player.seek(-100)
      assert.strictEqual(player.position, -60, 'a minute at most')
    } finally {
      mock.timers.reset()
    }
  })

  test('shows the position as heard, the output latency behind the audio clock', async () => {
    mock.timers.enable({ apis: ['setInterval', 'setTimeout'] })
    try {
      const { context, starts } = fakeContext('running')
      // What WebKit reports for Bluetooth headphones on macOS (D51).
      Object.assign(context, { outputLatency: 0.16, baseLatency: 0.0025 })
      const player = new midi.Player({ createContext: () => context })
      player.load(song())
      assert.strictEqual(await player.play(), true)
      assert.deepStrictEqual(starts, [10.06], 'the schedule does not move')
      const near = (a: number, b: number) => Math.abs(a - b) < 1e-9

      // Beat one is rendered but not heard yet: the position waits at the start.
      context.currentTime = 10.06 + 0.1
      assert.ok(near(player.scheduled, 0.1) && player.position === 0)
      context.currentTime = 10.06 + 1.5
      mock.timers.tick(50)
      assert.ok(near(player.latency, 0.1625))
      assert.ok(near(player.scheduled, 1.5) && near(player.position, 1.3375))
      assert.deepStrictEqual(starts, [10.06, 11.06], "scheduled on the clock, not on what is heard")

      // A pause goes on from what was rendered, which is heard out after it.
      player.pause()
      assert.ok(near(player.position, 1.5))
      await player.seek(2.9)
      assert.strictEqual(await player.play(), true)
      // The clock has passed the end; the last of it has not been heard yet.
      context.currentTime += 0.06 + 0.25
      mock.timers.tick(50)
      assert.ok(player.state === 'playing' && player.scheduled > 3 && near(player.position, 2.9875))
      context.currentTime += 0.1
      mock.timers.tick(50)
      assert.strictEqual(player.state, 'stopped')

      // Where the browser does not say, as in older engines, there is none.
      Object.assign(context, { outputLatency: undefined, baseLatency: NaN })
      assert.strictEqual(player.latency, 0)
    } finally {
      mock.timers.reset()
    }
  })

  test('says so when the browser wants a click first, and stays stopped', async () => {
    mock.timers.enable({ apis: ['setInterval', 'setTimeout'] })
    try {
      const { context, starts } = fakeContext('suspended')
      const player = new midi.Player({ createContext: () => context })
      player.load(song())
      const playing = player.play()
      mock.timers.tick(300)
      assert.strictEqual(await playing, false)
      assert.deepStrictEqual([player.state, player.suspended, starts], ['stopped', true, []])

      // The click came.
      context.state = 'running'
      assert.strictEqual(await player.play(), true)
      assert.strictEqual(player.suspended, false)
    } finally {
      mock.timers.reset()
    }
  })

  test('pauses where the music was when the system takes the audio away', async () => {
    mock.timers.enable({ apis: ['setInterval', 'setTimeout'] })
    try {
      const { context, starts } = fakeContext('running')
      const listeners: Array<() => void> = []
      Object.assign(context, { addEventListener: (type: string, listener: () => void) => type === 'statechange' && listeners.push(listener) })
      const states: string[] = []
      const player = new midi.Player({ createContext: () => context, onChange: () => states.push(player.state) })
      player.load(song())
      assert.strictEqual(await player.play(), true)
      assert.strictEqual(listeners.length, 1)
      context.currentTime = 10.06 + 1.2
      mock.timers.tick(50)

      // A phone call: Safari's `interrupted`, which stops the audio clock.
      Object.assign(context, { state: 'interrupted' })
      listeners[0]()
      assert.deepStrictEqual([player.state, Math.round(player.position * 10), player.suspended], ['paused', 12, true])
      assert.strictEqual(states[states.length - 1], 'paused')
      mock.timers.tick(1000)
      assert.strictEqual(starts.length, 2, 'nothing is scheduled while paused')

      // ▶ again once it is over; a change of state while not playing does nothing.
      Object.assign(context, { state: 'running' })
      listeners[0]()
      assert.strictEqual(await player.play(), true)
      assert.strictEqual(listeners.length, 1, 'one listener per context')
    } finally {
      mock.timers.reset()
    }
  })

  test('asks to be played as music where the browser has audio sessions', async () => {
    const session = { type: 'auto' }
    Object.defineProperty(globalThis.navigator, 'audioSession', { value: session, configurable: true })
    try {
      const { context } = fakeContext('running')
      const player = new midi.Player({ createContext: () => context })
      player.load(song())
      assert.strictEqual(session.type, 'auto', 'not before the first play')
      assert.strictEqual(await player.play(), true)
      assert.strictEqual(session.type, 'playback')
      player.dispose()
    } finally {
      Reflect.deleteProperty(globalThis.navigator, 'audioSession')
    }
  })

  test("a track's volume applies to what sounds and to what comes, and outlasts another load", async () => {
    mock.timers.enable({ apis: ['setInterval', 'setTimeout'] })
    try {
      const { context } = fakeContext('running')
      // What each track's panner goes into is its volume.
      const volumes: Array<{ gain: { value: number; targets: number[] } }> = []
      const panner = context.createStereoPanner
      Object.assign(context, {
        createStereoPanner: () => ({ ...panner(), connect: (next: (typeof volumes)[number]) => (volumes.push(next), next) }),
      })
      const duet = () =>
        midi.parseMidi(
          file(96, [track(tempo(0, 60)), track([0, 0x90, 60, 90], [96, 0x80, 60, 0]), track([0, 0x91, 64, 90], [96, 0x81, 64, 0])]),
        )
      const player = new midi.Player({ createContext: () => context })
      player.load(duet())
      player.setTrackVolume(2, 0.5)
      assert.deepStrictEqual([player.trackVolume(1), player.trackVolume(2)], [1, 0.5])
      assert.strictEqual(await player.play(), true)
      assert.deepStrictEqual(volumes.map((volume) => volume.gain.value).sort(), [0.5, 1])

      // Silenced while it sounds; beyond 2 is 2.
      player.setTrackVolume(1, 0)
      const first = volumes.find((volume) => volume.gain.value === 1)!
      assert.deepStrictEqual(first.gain.targets, [0])
      player.setTrackVolume(1, 9)
      assert.strictEqual(player.trackVolume(1), 2)

      player.load(duet())
      assert.deepStrictEqual([player.trackVolume(1), player.trackVolume(2), player.trackVolume(3)], [2, 0.5, 1])
    } finally {
      mock.timers.reset()
    }
  })

  test('the whole music’s volume: set before the sound starts, while it sounds, and kept between 0 and 1', async () => {
    mock.timers.enable({ apis: ['setInterval', 'setTimeout'] })
    try {
      const { context } = fakeContext('running')
      // The synthesizer's gains in the order made: its output, then the master after the compressor.
      const gains: Array<{ gain: { value: number; targets: number[] } }> = []
      const gain = context.createGain
      Object.assign(context, { createGain: () => (gains.push(gain() as (typeof gains)[number]), gains.at(-1)) })
      const player = new midi.Player({ createContext: () => context })
      assert.strictEqual(player.volume, 1)
      player.setVolume(0.4)
      player.load(song())
      assert.strictEqual(await player.play(), true)
      const master = gains[1]!
      assert.strictEqual(master.gain.value, 0.4)
      player.setVolume(0.7)
      assert.deepStrictEqual(master.gain.targets, [0.7])
      player.setVolume(5)
      assert.strictEqual(player.volume, 1)
      player.setVolume(-1)
      assert.strictEqual(player.volume, 0)
    } finally {
      mock.timers.reset()
    }
  })

  describe('rests the audio device while nothing plays (D151)', () => {
    const REST = 10000
    const RESUME_LIMIT = 5000
    const settled = () => new Promise((resolve) => setImmediate(resolve))
    /** The clock moves, and what was waiting for it has its turn. */
    async function pass(ms: number) {
      await settled()
      mock.timers.tick(ms)
      await settled()
    }

    /** A context as a browser has one: suspending and resuming take time, then say so. */
    function device(resumeMs = 20) {
      const { context, starts } = fakeContext('running')
      const listeners: Array<() => void> = []
      const calls: string[] = []
      const become = (state: 'running' | 'suspended') => {
        context.state = state
        for (const listener of listeners) listener()
      }
      const after = (ms: number, then: () => void) =>
        new Promise<void>((resolve) =>
          setTimeout(() => {
            then()
            resolve()
          }, ms),
        )
      const controls = { resumeMs, suspendMs: 5, become, listeners, calls }
      let resuming: Promise<void> | undefined
      Object.assign(context, {
        addEventListener: (_: string, listener: () => void) => listeners.push(listener),
        suspend() {
          calls.push('suspend')
          return after(controls.suspendMs, () => become('suspended'))
        },
        resume() {
          calls.push('resume')
          // A second resume settles with the first.
          resuming ??= after(controls.resumeMs, () => {
            resuming = undefined
            become('running')
          })
          return resuming
        },
      })
      return { context, starts, ...controls, controls }
    }

    /** A player that has played and is paused, its device at rest. */
    async function rested(resumeMs?: number) {
      const fake = device(resumeMs)
      const player = new midi.Player({ createContext: () => fake.context })
      player.load(song())
      assert.strictEqual(await player.play(), true)
      player.pause()
      await pass(REST)
      await pass(5)
      assert.strictEqual(fake.context.state, 'suspended')
      fake.calls.length = 0
      return { ...fake, player }
    }

    function clocked(run: () => Promise<void>) {
      return async () => {
        mock.timers.enable({ apis: ['setInterval', 'setTimeout'] })
        try {
          await run()
        } finally {
          mock.timers.reset()
        }
      }
    }

    test('suspends the context after a pause, and not before the grace is over', clocked(async () => {
      const { context, calls } = device()
      const player = new midi.Player({ createContext: () => context })
      player.load(song())
      await player.play()
      await pass(REST * 2)
      assert.deepStrictEqual(calls, [], 'not while it plays')
      player.pause()
      await pass(REST - 1)
      assert.deepStrictEqual(calls, [])
      assert.strictEqual(player.suspended, false, 'our own rest is no refusal')
      await pass(1)
      assert.deepStrictEqual(calls, ['suspend'])
      await pass(5)
      assert.deepStrictEqual([context.state, player.suspended], ['suspended', false], 'resting, it is still no refusal')
    }))

    test('after ■ and at the end of the music too', clocked(async () => {
      for (const end of ['stop', 'the end'] as const) {
        const { context, calls } = device()
        const player = new midi.Player({ createContext: () => context })
        player.load(song())
        await player.play()
        if (end === 'stop') player.stop()
        else {
          context.currentTime += 4
          mock.timers.tick(50)
          assert.strictEqual(player.state, 'stopped')
        }
        await pass(REST)
        assert.deepStrictEqual(calls, ['suspend'], end)
      }
    }))

    test('a play inside the grace calls it off, and the device is not touched', clocked(async () => {
      const { context, calls, starts } = device()
      const player = new midi.Player({ createContext: () => context })
      player.load(song())
      await player.play()
      player.pause()
      await pass(REST - 1)
      assert.strictEqual(await player.play(), true)
      await pass(REST * 2)
      assert.deepStrictEqual(calls, [])
      assert.strictEqual(player.state, 'playing')
      assert.strictEqual(starts.length, 2)
    }))

    test('a seek, or a reload that goes on, never lets a suspend in between', clocked(async () => {
      const { context, calls, listeners } = device()
      const player = new midi.Player({ createContext: () => context })
      const music = song()
      player.load(music)
      await player.play()
      await player.seek(1)
      assert.strictEqual(player.state, 'playing')
      // What ScorePlayer.remix() does.
      player.load(music)
      await player.seek(1.5)
      await player.play()
      await pass(REST * 2)
      assert.deepStrictEqual(calls, [])
      assert.strictEqual(player.state, 'playing')
      assert.strictEqual(listeners.length, 1)
    }))

    test('a seek while paused leaves it to rest, and moves the music all the same', clocked(async () => {
      const { player, context, calls } = await rested()
      await player.seek(2)
      await pass(REST * 2)
      assert.deepStrictEqual(calls, [], 'nothing to suspend, and no reason to resume')
      assert.deepStrictEqual([player.state, player.position, context.state], ['paused', 2, 'suspended'])
    }))

    test('a play after it resumes and sounds, on the clock as it is then', clocked(async () => {
      const { player, context, calls, starts } = await rested()
      const before = starts.length
      context.currentTime = 12
      const playing = player.play()
      await pass(20)
      assert.strictEqual(await playing, true)
      assert.deepStrictEqual(calls, ['resume'])
      assert.deepStrictEqual([player.state, player.suspended], ['playing', false])
      assert.deepStrictEqual(starts.slice(before), [12.06], 'the lead-in is the usual one')
    }))

    test('a slow resume is no failure, however the first start was on Bluetooth', clocked(async () => {
      for (const resumeMs of [550, 3000]) {
        const { player } = await rested(resumeMs)
        let done = false
        const playing = player.play().then((started) => ((done = true), started))
        await pass(300)
        assert.deepStrictEqual([done, player.state, player.starting], [false, 'paused', true], 'the race of a gesture is not run here')
        await pass(resumeMs - 300)
        assert.strictEqual(await playing, true)
        assert.deepStrictEqual([player.state, player.suspended, player.starting], ['playing', false, false])
      }
    }))

    test('a device that does not come back is reported, once it had a fair time', clocked(async () => {
      const { player, context } = await rested(60000)
      const playing = player.play()
      await pass(RESUME_LIMIT)
      assert.strictEqual(await playing, false)
      assert.deepStrictEqual([player.state, player.suspended], ['paused', true])
      // It came back after all: nothing plays, so it rests again.
      await pass(60000 - RESUME_LIMIT)
      assert.strictEqual(context.state, 'running')
      await pass(REST)
      await pass(5)
      assert.strictEqual(context.state, 'suspended')
    }))

    test('a play that lands on a suspend still on its way waits for it: nothing pauses it', clocked(async () => {
      const { context, calls, starts, controls } = device()
      // Slower than the resume: a resume sent past it would lose to it.
      controls.suspendMs = 30
      const player = new midi.Player({ createContext: () => context })
      player.load(song())
      await player.play()
      player.pause()
      await pass(REST)
      assert.deepStrictEqual([calls, context.state], [['suspend'], 'running'], 'it says running until it lands')
      const before = starts.length
      const playing = player.play()
      assert.strictEqual(player.state, 'paused')
      await pass(30)
      assert.deepStrictEqual(calls, ['suspend', 'resume'], 'the resume waited for it')
      await pass(20)
      assert.strictEqual(await playing, true)
      assert.deepStrictEqual(calls, ['suspend', 'resume'])
      assert.deepStrictEqual([player.state, context.state, starts.length], ['playing', 'running', before + 1])
    }))

    test('the end of our suspend does not pause what plays', clocked(async () => {
      const { player, become } = await rested()
      const playing = player.play()
      become('suspended') // A late event of the rest, while the resume is on its way.
      await pass(20)
      assert.strictEqual(await playing, true)
      assert.strictEqual(player.state, 'playing')
    }))

    test('■ or ❚❚ during a slow resume calls it off without a failure, and it rests again', clocked(async () => {
      for (const press of ['stop', 'pause'] as const) {
        const { player, context, starts } = await rested(550)
        const before = starts.length
        const playing = player.play()
        await pass(100)
        player[press]()
        await pass(450)
        assert.strictEqual(await playing, false)
        assert.deepStrictEqual([player.suspended, player.starting, starts.length], [false, false, before])
        assert.strictEqual(context.state, 'running')
        await pass(REST)
        await pass(5)
        assert.strictEqual(context.state, 'suspended', press)
      }
    }))

    test('▶ twice during a slow resume starts once, and neither press fails', clocked(async () => {
      const { player, starts } = await rested(550)
      const before = starts.length
      const first = player.play()
      await pass(200)
      const second = player.play()
      await pass(350)
      assert.deepStrictEqual([await first, player.suspended], [false, false])
      assert.strictEqual(await second, true)
      assert.strictEqual(starts.length, before + 1)
    }))

    test('a seek during a slow resume keeps the press, from where it was sent', clocked(async () => {
      const { player, starts } = await rested(550)
      const before = starts.length
      const playing = player.play()
      await pass(100)
      const seeking = player.seek(1)
      await pass(550)
      await seeking
      assert.strictEqual(await playing, false, 'that play gave way to the next')
      assert.deepStrictEqual([player.state, player.suspended, starts.length], ['playing', false, before + 1])
    }))

    test('a reload during a slow resume calls it off', clocked(async () => {
      const { player, starts } = await rested(550)
      const before = starts.length
      const playing = player.play()
      await pass(100)
      player.load(song())
      await pass(450)
      assert.strictEqual(await playing, false)
      assert.deepStrictEqual([player.state, player.suspended, starts.length], ['stopped', false, before])
    }))

    test('dispose ends the wait for the grace with the context', clocked(async () => {
      const { context, calls } = device()
      let closed = 0
      Object.assign(context, { close: async () => void closed++ })
      const player = new midi.Player({ createContext: () => context })
      player.load(song())
      await player.play()
      player.pause()
      player.dispose()
      await pass(REST * 2)
      assert.deepStrictEqual([calls, closed], [[], 1])
    }))

    test('a context without suspend simply stays running', clocked(async () => {
      const { context, starts } = fakeContext('running')
      const player = new midi.Player({ createContext: () => context })
      player.load(song())
      await player.play()
      player.pause()
      await pass(REST * 2)
      assert.strictEqual(context.state, 'running')
      assert.strictEqual(await player.play(), true)
      assert.strictEqual(starts.length, 2)
    }))

    test('the system giving the audio back, with nothing to play, is rested too', clocked(async () => {
      const { context, calls, become, listeners } = device()
      const player = new midi.Player({ createContext: () => context })
      player.load(song())
      await player.play()
      Object.assign(context, { state: 'interrupted' })
      listeners[0]()
      assert.deepStrictEqual([player.state, player.suspended], ['paused', true])
      await pass(REST)
      assert.deepStrictEqual(calls, [], 'it is stopped already')
      become('running')
      await pass(REST)
      assert.deepStrictEqual(calls, ['suspend'])
    }))

    test('a wireless output is given time to start streaming; a wired one is not', clocked(async () => {
      const wired = await rested()
      Object.assign(wired.context, { outputLatency: 0.01 })
      const fast = wired.player.play()
      await pass(20)
      assert.strictEqual(await fast, true)

      // What WebKit reports for Bluetooth headphones on macOS (D51).
      const wireless = await rested()
      Object.assign(wireless.context, { outputLatency: 0.16 })
      const before = wireless.starts.length
      const slow = wireless.player.play()
      await pass(20)
      assert.deepStrictEqual([wireless.player.state, wireless.player.starting, wireless.starts.length], ['paused', true, before])
      await pass(449)
      assert.strictEqual(wireless.player.state, 'paused')
      await pass(1)
      assert.strictEqual(await slow, true)
      assert.deepStrictEqual(wireless.starts.slice(before), [10.06], 'the schedule itself has the usual lead-in')
    }))

    test('the first play is patient too once the page has been clicked; before that it is not', clocked(async () => {
      Object.defineProperty(globalThis.navigator, 'userActivation', { value: { hasBeenActive: true }, configurable: true })
      try {
        const { context } = device(550)
        context.state = 'suspended'
        const player = new midi.Player({ createContext: () => context })
        player.load(song())
        const playing = player.play()
        await pass(550)
        assert.strictEqual(await playing, true)
      } finally {
        Reflect.deleteProperty(globalThis.navigator, 'userActivation')
      }
    }))

    test('■ while the browser holds a first press back is no refusal', clocked(async () => {
      const { context } = device(60000)
      context.state = 'suspended'
      const player = new midi.Player({ createContext: () => context })
      player.load(song())
      const playing = player.play()
      await pass(100)
      player.stop()
      await pass(200)
      assert.strictEqual(await playing, false)
      assert.deepStrictEqual([context.state, player.suspended], ['suspended', false])
    }))

    test('a press the browser holds back is still given up on at 300 ms', clocked(async () => {
      Object.defineProperty(globalThis.navigator, 'userActivation', { value: { hasBeenActive: false }, configurable: true })
      try {
        const { context } = device(60000)
        context.state = 'suspended'
        const player = new midi.Player({ createContext: () => context })
        player.load(song())
        const playing = player.play()
        await pass(300)
        assert.strictEqual(await playing, false)
        assert.strictEqual(player.suspended, true)
      } finally {
        Reflect.deleteProperty(globalThis.navigator, 'userActivation')
      }
    }))
  })
})
