import * as assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import type { Midi } from '@lily/common/web/midi.js'
import type { KaraokeScene } from '../src/renderer/karaoke'
import { VideoExporter, videoFrames, videoLength, wav, type VideoApi, type VideoRender, type VideoSource } from '../src/renderer/video'

// Runs under `node --test` from out/test/ (npm run test:unit).

describe('Export Video (D140)', () => {
  test('the video lasts as long as the score, or the backing track when that is longer', () => {
    const backing = (duration: number) => ({ path: '/s/b.mp3', duration, offset: 0, volume: 1, lengthens: true })
    assert.equal(videoLength(150, []), 150)
    assert.equal(videoLength(150, [backing(182.5)]), 182.5)
    assert.equal(videoLength(150, [backing(120)]), 150)
    // The audio track does not: playback ends with it if it is to (D155).
    assert.equal(videoLength(150, [{ path: '/s/a.mp3', duration: 300, offset: 2, volume: 0.5 }]), 150)
  })

  test('the frames run from the score’s % Start: to the end (D149)', () => {
    assert.deepEqual(videoFrames(0, 8, 30), { start: 0, frames: 240 })
    assert.deepEqual(videoFrames(31.88, 186.13, 30), { start: 31.88, frames: 4628 })
    assert.deepEqual(videoFrames(-4.09, 213.46, 30), { start: -4.09, frames: 6527 })
    assert.deepEqual(videoFrames(200, 8, 30), { start: 0, frames: 240 }, 'past the end starts at 0')
  })

  test('the melody is a 16-bit PCM WAV, its channels interleaved and clipped', () => {
    const bytes = wav([new Float32Array([0, 1, -2]), new Float32Array([0.5, -1, 2])], 48000)
    const view = new DataView(bytes.buffer)
    const text = (at: number) => String.fromCharCode(...bytes.subarray(at, at + 4))
    assert.deepEqual([text(0), text(8), text(12), text(36)], ['RIFF', 'WAVE', 'fmt ', 'data'])
    assert.equal(view.getUint32(4, true), 36 + 12)
    assert.equal(view.getUint16(22, true), 2)
    assert.equal(view.getUint32(24, true), 48000)
    assert.equal(view.getUint32(28, true), 48000 * 4)
    assert.equal(view.getUint32(40, true), 12)
    const samples = Array.from({ length: 6 }, (_, i) => view.getInt16(44 + i * 2, true))
    assert.deepEqual(samples, [0, 16384, 32767, -32767, -32767, 32767])
  })

  describe('Export Video pressed again cancels it at any step', () => {
    /** A studio that logs its calls; `finish` and `melody` are left pending until a cancel when asked. */
    function exporter(hold: { melody?: boolean; frame?: number; finish?: boolean } = {}) {
      const calls: string[] = []
      const finished: unknown[] = []
      const statuses: string[] = []
      const progress: (number | undefined)[] = []
      let cancelled = () => {}
      const cancel = new Promise<void>((resolve) => (cancelled = resolve))
      let frames = 0
      const studio: VideoApi = {
        videoStart: async () => (calls.push('start'), '/songs/song (karaoke).mp4'),
        videoAudio: async () => void calls.push('audio'),
        videoFrame: async () => {
          calls.push('frame')
          if (++frames === hold.frame) {
            await cancel
            throw new Error('The export was stopped.')
          }
        },
        videoFinish: async (recordings, start, seconds) => {
          calls.push('finish')
          finished.push({ recordings, start, seconds })
          if (!hold.finish) return '/songs/song (karaoke).mp4'
          await cancel
          return undefined
        },
        videoCancel: async () => {
          calls.push('cancel')
          cancelled()
          return true
        },
      }
      const render: VideoRender = {
        melody: async () => (hold.melody ? new Promise<undefined>(() => {}) : wav([new Float32Array(4), new Float32Array(4)], 48000)),
        frames: () => async () => new Uint8Array([0xff, 0xd8]),
      }
      const source: VideoSource = {
        rootFile: '/songs/song.ly',
        scene: {} as KaraokeScene,
        duration: 2,
        recordings: [{ path: '/songs/song.mp3', duration: 3, offset: -0.5, volume: 0.4 }],
        melody: { midi: { notes: [] } as unknown as Midi, volumes: new Map([[0, 1]]) },
      }
      const video = new VideoExporter({
        studio,
        source: () => source,
        onProgress: (part) => progress.push(part),
        onStatus: (message) => statuses.push(message),
        render,
      })
      return { video, calls, statuses, progress, finished }
    }

    const settle = async (statuses: string[]) => {
      for (let tries = 0; !statuses.length && tries < 1000; tries++) await new Promise((resolve) => setTimeout(resolve, 1))
    }
    const waitFor = async (calls: string[], call: string) => {
      for (let tries = 0; !calls.includes(call) && tries < 1000; tries++) await new Promise((resolve) => setTimeout(resolve, 1))
    }

    test('an export runs to the end: the melody, the frames, the file', async () => {
      const { video, calls, statuses, progress, finished } = exporter()
      video.toggle()
      await settle(statuses)
      assert.deepEqual(statuses, ['Exported song (karaoke).mp4'])
      assert.deepEqual([calls[0], calls[1], calls.filter((call) => call === 'frame').length, calls.at(-1)], ['start', 'audio', 60, 'finish'])
      assert.deepEqual([progress[0], progress.at(-2), progress.at(-1)], [0, 1, undefined])
      // The recordings chosen go to ffmpeg at their offsets and volumes, cut at the frames' end (D155).
      assert.deepEqual(finished, [{ recordings: [{ path: '/songs/song.mp3', offset: -0.5, volume: 0.4 }], start: 0, seconds: 2 }])
    })

    test('while the melody renders, a cancel stops at once and its sound is not used', async () => {
      const { video, calls, statuses, progress } = exporter({ melody: true })
      video.toggle()
      await waitFor(calls, 'start')
      await new Promise((resolve) => setTimeout(resolve, 5))
      video.toggle()
      await settle(statuses)
      assert.deepEqual(statuses, ['Stopped exporting the video'])
      assert.ok(calls.includes('cancel'))
      assert.ok(!calls.includes('audio') && !calls.includes('frame') && !calls.includes('finish'))
      assert.equal(progress.at(-1), undefined)
    })

    test('a frame cut short by the cancel is no error', async () => {
      const { video, calls, statuses } = exporter({ frame: 1 })
      video.toggle()
      await waitFor(calls, 'frame')
      video.toggle()
      await settle(statuses)
      assert.deepEqual(statuses, ['Stopped exporting the video'])
      assert.ok(!calls.includes('finish'))
    })

    test('while ffmpeg puts the file together, a cancel reaches it and nothing is written', async () => {
      const { video, calls, statuses } = exporter({ finish: true })
      video.toggle()
      await waitFor(calls, 'finish')
      video.toggle()
      await settle(statuses)
      assert.deepEqual(statuses, ['Stopped exporting the video'])
      assert.deepEqual(calls.slice(-2), ['finish', 'cancel'])
    })
  })
})
