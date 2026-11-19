import * as assert from 'node:assert/strict'
import { afterEach, beforeEach, describe, mock, test } from 'node:test'
import { Player, type Midi } from '@lily/common/web/midi.js'
import { AudioTrack, STUCK_MS } from '../src/renderer/audioTrack'
import { FakeAudio, FakeElement, fakeContext } from './fakes'

// Runs under `node --test` from out/test/, in packages/studio/ (npm run test:unit).

describe('the audio track and the transport (D155)', () => {
  /** Between the transport's play and its first sample (midi.js). */
  const LEAD_IN = 0.06
  let audio: FakeAudio
  let now: number
  let players: Player[]
  let errors: string[]

  beforeEach(() => {
    now = 100_000
    players = []
    errors = []
    mock.method(performance, 'now', () => now)
    const createElement = (tag: string) => (tag === 'audio' ? (audio = new FakeAudio()) : new FakeElement())
    Object.assign(globalThis, { document: { createElement } })
  })

  afterEach(() => {
    for (const player of players) player.dispose()
    mock.restoreAll()
    assert.deepEqual(errors, [])
  })

  /** A frame of the window, in seconds. */
  const FRAME = 0.02

  /** Where the recording was sent, to the millisecond. */
  const sent = () => audio.sent.map((place) => Math.round(place * 1000) / 1000)

  /** A track starting `offset` s into 180 s of music on the real player, whose seeks count as ScorePlayer's do. */
  function track(offset: number, latency = 0) {
    const { context, clock } = fakeContext()
    clock.latency = latency
    let seeks = 0
    const shown = new AudioTrack({ onError: (message) => errors.push(message) })
    let hold = 0
    let sends = 0
    let wasPaused = true
    const follow = () => shown.follow(player.clock, player.latency, player.state === 'playing', seeks)
    const player = new Player({ createContext: () => context, onChange: follow })
    players.push(player)
    player.load({ notes: [], duration: 180 } as unknown as Midi)
    shown.load({ path: '/s/song.mp3', url: 'song' })
    shown.offset = offset
    audio.loaded()
    return {
      track: shown,
      player,
      /** The music has played for `seconds`, on its clock, the wall's and the recording's. */
      pass: (seconds: number) => {
        clock.time += seconds
        now += seconds * 1000
        if (!audio.paused) audio.reach(audio.currentTime + seconds)
        follow()
      },
      /** ▶, and a frame once its lead-in has passed. */
      play: async () => {
        await player.play()
        clock.time += LEAD_IN
        follow()
      },
      /**
       * The music has played for `seconds`, frame by frame, and the recording
       * as AVFoundation plays it (D167): sent somewhere or played, its time
       * holds for `cost`, the output's latency and some 0.1 s more, then moves.
       */
      heard: (seconds: number, cost: number) => {
        for (let left = seconds; left > 1e-9; left -= FRAME) {
          if (audio.sent.length !== sends || (wasPaused && !audio.paused)) hold = cost
          sends = audio.sent.length
          wasPaused = audio.paused
          clock.time += FRAME
          now += FRAME * 1000
          if (!audio.paused) {
            audio.reach(audio.currentTime + Math.max(FRAME - hold, 0))
            hold = Math.max(hold - FRAME, 0)
          }
          follow()
        }
      },
      /** How far the recording is heard from the music, which is `latency` behind its audio clock. */
      apart: () => audio.currentTime - (player.scheduled - latency - offset),
      seekTransport: (seconds: number) => {
        seeks++
        return player.seek(seconds)
      },
      follow,
    }
  }

  test('a later start: the recording waits at its start until the music reaches it', async () => {
    const { play, pass } = track(2.5)
    await play()
    pass(1)
    assert.equal(audio.paused, true)
    assert.equal(audio.currentTime, 0)
    pass(1.6)
    assert.equal(audio.paused, false)
    assert.deepEqual(sent(), [0.1])
    pass(5)
    assert.equal(audio.currentTime, 5.1)
    assert.deepEqual(sent(), [0.1])
  })

  test('an earlier start: at the music\'s start the recording is that far in', async () => {
    const { play, pass } = track(-1.25)
    await play()
    pass(0.1)
    assert.equal(audio.paused, false)
    // Sent as the lead-in began, to be that far in as the music starts (D167).
    assert.deepEqual(sent(), [1.19])
  })

  test('a seek of the transport takes it along at once; pause pauses it, and ▶ goes on from the music\'s place', async () => {
    const { play, pass, seekTransport, player } = track(1)
    await play()
    pass(3)
    audio.sent = []
    await seekTransport(30)
    // Through the music's lead-in the recording moves already (D167).
    assert.deepEqual(sent(), [28.94])
    player.pause()
    assert.equal(audio.paused, true)
    await seekTransport(60)
    await play()
    pass(0.01)
    assert.deepEqual(sent(), [28.94, 58.94])
    assert.equal(audio.paused, false)
  })

  test('one that strays is put back, then left alone for 2 s', async () => {
    const { play, pass } = track(0)
    await play()
    pass(5)
    audio.reach(5.2)
    pass(0.1)
    assert.deepEqual(sent(), [])
    audio.reach(6)
    pass(0.1)
    assert.deepEqual(sent(), [5.2])
    audio.reach(7)
    pass(1)
    assert.deepEqual(sent(), [5.2])
    pass(1.1)
    assert.deepEqual(sent(), [5.2, 7.3])
  })

  test('past its end it stays silent, and comes back for a seek into it', async () => {
    const { play, pass, seekTransport } = track(-150)
    await play()
    pass(1)
    assert.equal(audio.paused, false)
    await seekTransport(60)
    assert.equal(audio.paused, true)
    await seekTransport(10)
    pass(0.01)
    assert.equal(audio.paused, false)
    assert.equal(sent().at(-1), 160)
  })

  test('an offset changed during playback moves it at once', async () => {
    const { track: shown, play, pass, follow } = track(0)
    await play()
    pass(10)
    assert.deepEqual(sent(), [])
    shown.offset = -0.5
    follow()
    assert.deepEqual(sent(), [10.5])
  })

  test('its volume is its own under the whole music\'s, and a mute silences it', () => {
    const { track: shown } = track(0)
    shown.volume = 0.5
    assert.equal(audio.volume, 0.5)
    shown.masterVolume = 0.6
    assert.equal(audio.volume, 0.3)
    shown.muted = true
    assert.equal(audio.volume, 0)
    shown.muted = false
    assert.equal(audio.volume, 0.3)
  })

  test('without a recording it follows nothing, and another recording starts paused', async () => {
    const { track: shown, play, pass } = track(0)
    await play()
    pass(1)
    assert.equal(audio.paused, false)
    shown.load(undefined)
    assert.equal(audio.paused, true)
    pass(1)
    assert.equal(audio.paused, true)
    assert.equal(shown.file, undefined)
  })

  // WebKit plays an <audio> on AVFoundation, which stopped a recording at its end
  // when ▶ came with a seek away from it still under way (D158).
  test('played to its end, ▶ from the start plays it once the seek has landed, and it is heard', async () => {
    const { play, pass, player } = track(0)
    await play()
    pass(5)
    audio.finish()
    player.stop()
    audio.slowSeeks = true
    audio.sent = []
    await play()
    pass(0.01)
    // Sent back to the music's place, and not played until it is there.
    assert.equal(audio.seeking, true)
    assert.equal(audio.paused, true)
    audio.landSeek()
    assert.equal(audio.paused, false)
    assert.equal(audio.stalled, false)
    pass(2)
    assert.ok(audio.currentTime > 1.9, `at ${audio.currentTime}`)
  })

  test('past its end it waits at its start, not at its end', async () => {
    const { play, pass, seekTransport } = track(-150)
    await play()
    pass(1)
    await seekTransport(60)
    assert.equal(audio.paused, true)
    assert.equal(audio.currentTime, 0)
  })

  test('counting as playing, its time stopped for a second: it is started again where the music is', async () => {
    const { play, pass } = track(0)
    await play()
    pass(3)
    const reach = audio.reach.bind(audio)
    audio.reach = () => {}
    pass(0.5)
    assert.equal(audio.paused, false)
    for (let i = 0; i < 8; i++) pass(STUCK_MS / 5000)
    audio.reach = reach
    assert.equal(audio.paused, false)
    // Sent where the music was a second after it stopped moving, and playing.
    const restarted = sent().filter((place) => place > 4.3)
    assert.equal(restarted.length, 1, `sent ${sent()}`)
  })

  test('its duration shrunk to where it stopped: the file is loaded again and played where the music is', async () => {
    const { play, pass, follow } = track(0)
    await play()
    pass(3)
    let loads = 0
    const load = audio.load.bind(audio)
    audio.load = () => {
      loads++
      load()
    }
    audio.stalled = true
    audio.duration = 0.27
    audio.fire('durationchange')
    assert.equal(loads, 1)
    assert.equal(audio.duration, 200)
    follow()
    assert.equal(audio.paused, false)
    assert.ok(Math.abs(audio.currentTime - 3) < 0.1, `at ${audio.currentTime}`)
  })

  // AVFoundation's time is the recording as heard: on a Bluetooth amp, 0.264 s
  // of latency, a seek took 0.38 s to be heard. Kept to the audio clock, each
  // seek landed 0.33 s behind, past DRIFT, and it was put back every 2 s.
  test('on a Bluetooth output it is kept to the music as heard, and put back once for a stall, not every 2 s (D167)', async () => {
    const { player, heard, apart } = track(0, 0.264)
    // Its lead-in passes frame by frame, as it does for a seek.
    await player.play()
    heard(4, 0.38)
    assert.ok(Math.abs(apart()) < 0.15, `heard ${apart()} s from the music`)
    audio.sent = []
    // A stall of the recording: it is put back, once.
    audio.reach(audio.currentTime - 0.4)
    heard(20, 0.38)
    assert.equal(audio.sent.length, 1, `sent ${sent()}`)
    assert.ok(Math.abs(apart()) < 0.03, `heard ${apart()} s from the music`)
  })

  test('what a seek takes is learned, so a seek of the transport is heard where the music is (D167)', async () => {
    const { player, heard, apart, seekTransport } = track(1, 0.264)
    // Its lead-in passes frame by frame, as it does for a seek.
    await player.play()
    heard(3, 0.38)
    await seekTransport(30)
    heard(1.5, 0.38)
    assert.ok(Math.abs(apart()) < 0.03, `heard ${apart()} s from the music`)
  })

  test('a later start on a Bluetooth output: it starts early by what a play() takes, and is heard with the music (D167)', async () => {
    const { player, heard, apart } = track(2.5, 0.264)
    // Its lead-in passes frame by frame, as it does for a seek.
    await player.play()
    heard(1, 0.38)
    assert.equal(audio.paused, true)
    heard(4, 0.38)
    assert.equal(audio.sent.length, 1, `sent ${sent()}`)
    assert.ok(Math.abs(apart()) < 0.15, `heard ${apart()} s from the music`)
  })
})
