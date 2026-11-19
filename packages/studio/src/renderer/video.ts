// Export Video (DECISIONS D140): Karaoke as an MP4 file, to share. The frames
// are drawn as Karaoke draws them, at 1280 × 720 scaled to 1920 × 1080, one
// for each thirtieth of a second, faster or slower than the music plays, and
// go to ffmpeg as JPEGs (src-tauri/src/video.rs). The sound is what the user
// ticks (D155), as heard by default: any of the score's parts at their volumes,
// rendered here on an OfflineAudioContext and sent as a WAV, and any of its
// recordings, the backing track while it follows the score (D135) and the
// score's audio track at its offset and volume, which ffmpeg sums in. The video
// lasts as long as playback, or the backing track when that is longer, from the
// score's `% Start:` (D149): past a long intro, or after a silence before the music.
import { Synth, type Midi } from '@lily/common/web/midi.js'
import { drawKaraoke, type KaraokeScene } from './karaoke'
import type { StudioApi } from './bridge'

/** The frames: their size in pixels, how many a second, and what Karaoke's size is scaled by to fill them. */
export const VIDEO = { width: 1920, height: 1080, fps: 30, scale: 1.5 } as const
const SAMPLE_RATE = 48000
const JPEG_QUALITY = 0.92
/** The WAV goes in parts this big, so no call carries all of it. */
const AUDIO_CHUNK = 3 * 1024 * 1024

/** What the export asks the Rust side for. */
export type VideoApi = Pick<StudioApi, 'videoStart' | 'videoAudio' | 'videoFrame' | 'videoFinish' | 'videoCancel'>

/** A recording summed into the video's sound (D155). */
export interface VideoRecording {
  path: string
  /** Its length in seconds, 0 when not known. */
  duration: number
  /** Where in the score it starts, in seconds. */
  offset: number
  /** 0–1. */
  volume: number
  /** The video lasts to its end, as to a backing track's (D140). */
  lengthens?: boolean
}

/** What the export is of, as Karaoke has it when Export Video is pressed. */
export interface VideoSource {
  rootFile: string
  scene: KaraokeScene
  /** Where the video starts, in seconds of the score: below 0 a silence before the music (D149); 0 when not given. */
  start?: number
  /** How long playback lasts, in seconds: the MIDI, or the audio track when it ends with it (D155). */
  duration: number
  /** The recordings chosen, summed in. */
  recordings: VideoRecording[]
  /** The parts chosen and their tracks' volumes; undefined for none. */
  melody: { midi: Midi; volumes: Map<number, number> } | undefined
}

/** The video's length: playback's, or a recording's that lengthens it when that is longer. */
export function videoLength(score: number, recordings: readonly VideoRecording[]): number {
  return Math.max(score, ...recordings.filter((recording) => recording.lengthens).map((recording) => recording.offset + recording.duration))
}

/** The frames from `start` to `length`, `fps` a second; a start past the end starts at 0. */
export function videoFrames(start: number, length: number, fps: number): { start: number; frames: number } {
  const from = start < length ? start : 0
  return { start: from, frames: Math.max(1, Math.ceil((length - from) * fps)) }
}

/** `channels`, `rate` samples a second each, as a 16-bit PCM WAV file. */
export function wav(channels: readonly Float32Array[], rate: number): Uint8Array {
  const count = channels.length
  const frames = channels[0]?.length ?? 0
  const size = frames * count * 2
  const bytes = new Uint8Array(44 + size)
  const view = new DataView(bytes.buffer)
  const text = (at: number, value: string) => [...value].forEach((char, i) => view.setUint8(at + i, char.charCodeAt(0)))
  text(0, 'RIFF')
  view.setUint32(4, 36 + size, true)
  text(8, 'WAVE')
  text(12, 'fmt ')
  view.setUint32(16, 16, true)
  view.setUint16(20, 1, true)
  view.setUint16(22, count, true)
  view.setUint32(24, rate, true)
  view.setUint32(28, rate * count * 2, true)
  view.setUint16(32, count * 2, true)
  view.setUint16(34, 16, true)
  text(36, 'data')
  view.setUint32(40, size, true)
  let at = 44
  for (let i = 0; i < frames; i++) {
    for (const channel of channels) {
      const sample = Math.max(-1, Math.min(1, channel[i]!))
      view.setInt16(at, Math.round(sample * 0x7fff), true)
      at += 2
    }
  }
  return bytes
}

/** `midi` at `volumes` for `length` seconds, as a stereo WAV; undefined when every track is silent. */
async function renderMelody(midi: Midi, volumes: Map<number, number>, length: number): Promise<Uint8Array | undefined> {
  if (![...volumes.values()].some((volume) => volume > 0) || length <= 0) return undefined
  const context = new OfflineAudioContext(2, Math.ceil(length * SAMPLE_RATE), SAMPLE_RATE)
  const synth = new Synth(context, volumes)
  for (const note of midi.notes) if (note.time < length) synth.play(note, note.time)
  const buffer = await context.startRendering()
  return wav([buffer.getChannelData(0), buffer.getChannelData(1)], SAMPLE_RATE)
}

function jpegOf(canvas: HTMLCanvasElement): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    canvas.toBlob(
      (blob) => (blob ? blob.arrayBuffer().then((buffer) => resolve(new Uint8Array(buffer)), reject) : reject(new Error('A frame could not be drawn.'))),
      'image/jpeg',
      JPEG_QUALITY,
    )
  })
}

/** How the export makes its sound and its frames. */
export interface VideoRender {
  melody(midi: Midi, volumes: Map<number, number>, length: number): Promise<Uint8Array | undefined>
  /** Draws frames of `scene` on a canvas of its own; each call answers the JPEG of the moment `now`, in seconds. */
  frames(scene: KaraokeScene): (now: number) => Promise<Uint8Array>
}

/** Karaoke's own drawing, at 1920 × 1080, and the melody on an OfflineAudioContext. */
const pageRender: VideoRender = {
  melody: renderMelody,
  frames(scene) {
    const { width, height, scale } = VIDEO
    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const context = canvas.getContext('2d')
    if (!context) throw new Error('No canvas to draw on.')
    context.setTransform(scale, 0, 0, scale, 0, 0)
    return (now) => {
      drawKaraoke(context, { ...scene, width: width / scale, height: height / scale, now })
      return jpegOf(canvas)
    }
  },
}

export interface VideoExporterOptions {
  studio: VideoApi
  /** What to export now, with the sounds `chosen` (D155); a message saying why not when there is nothing. */
  source(chosen: ReadonlySet<string> | undefined): VideoSource | string
  /** How far the export is, 0 to 1; undefined once it is over. */
  onProgress(part: number | undefined): void
  onStatus(message: string): void
  /** The page's own drawing and sound unless a test gives its own. */
  render?: VideoRender
}

/**
 * One export at a time: Export Video starts one, and pressed again cancels
 * it at once, at any step: while the melody renders (its sound is then
 * thrown away), while the frames go, and while ffmpeg puts the file together,
 * which the cancel kills.
 */
export class VideoExporter {
  private running = false
  private stopped = false
  /** Resolves when Export Video is pressed to cancel. */
  private stop: Promise<void> = Promise.resolve()
  private onStop: () => void = () => {}

  constructor(private readonly options: VideoExporterOptions) {}

  /** Starts an export with the sounds `chosen` (D155), or cancels the one running. */
  toggle(chosen?: ReadonlySet<string>): void {
    if (!this.running) return void this.run(chosen)
    if (this.stopped) return
    this.stopped = true
    this.onStop()
    void this.options.studio.videoCancel().catch(() => false)
  }

  /** Whether an export is running. */
  get busy(): boolean {
    return this.running
  }

  private async run(chosen: ReadonlySet<string> | undefined): Promise<void> {
    const { studio, onStatus, onProgress } = this.options
    const source = this.options.source(chosen)
    if (typeof source === 'string') return onStatus(source)
    this.running = true
    this.stopped = false
    this.stop = new Promise((resolve) => (this.onStop = resolve))
    try {
      const written = await this.export(source)
      if (written) onStatus(`Exported ${written.split(/[\\/]/).pop()}`)
      else if (this.stopped) onStatus('Stopped exporting the video')
    } catch (error) {
      await studio.videoCancel().catch(() => false)
      // A cancel fails the step it cut short; that is no error.
      if (this.stopped) onStatus('Stopped exporting the video')
      else onStatus(`The video could not be exported: ${error instanceof Error ? error.message : String(error)}`)
    } finally {
      this.running = false
      onProgress(undefined)
    }
  }

  /** The file written, or undefined when the panel was cancelled or the export stopped. */
  private async export(source: VideoSource): Promise<string | undefined> {
    const { studio, onProgress } = this.options
    const render = this.options.render ?? pageRender
    const { width, height, fps } = VIDEO
    if (!(await studio.videoStart(source.rootFile, { width, height, fps }))) return undefined
    if (this.stopped) return this.cancel()
    onProgress(0)
    const length = videoLength(source.duration, source.recordings)
    const rendering = source.melody ? render.melody(source.melody.midi, source.melody.volumes, length) : Promise.resolve(undefined)
    // The OfflineAudioContext cannot be stopped: a cancel leaves it, and its sound is not used.
    const melody = await Promise.race([rendering, this.stop.then(() => undefined)])
    if (this.stopped) return this.cancel()
    for (let at = 0; melody && at < melody.length; at += AUDIO_CHUNK) {
      if (this.stopped) return this.cancel()
      await studio.videoAudio(melody.subarray(at, at + AUDIO_CHUNK))
    }
    const draw = render.frames(source.scene)
    const { start, frames } = videoFrames(source.start ?? 0, length, fps)
    // A frame is drawn while the one before is on its way.
    let sending = Promise.resolve()
    for (let frame = 0; frame < frames; frame++) {
      if (this.stopped) {
        await sending.catch(() => undefined)
        return this.cancel()
      }
      const jpeg = await draw(start + frame / fps)
      await sending
      sending = studio.videoFrame(jpeg)
      if (frame % fps === 0) onProgress(frame / frames)
    }
    await sending
    if (this.stopped) return this.cancel()
    onProgress(1)
    // ffmpeg puts the file together; a cancel now kills it and the file is not written.
    const recordings = source.recordings.map(({ path, offset, volume }) => ({ path, offset, volume }))
    return studio.videoFinish(recordings, start, frames / fps)
  }

  private async cancel(): Promise<undefined> {
    await this.options.studio.videoCancel()
    return undefined
  }
}
