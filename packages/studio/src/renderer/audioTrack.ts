// A score's audio track (DECISIONS D155): the recording its `% Audio:` line
// names, played along with the MIDI as a track of its own. It starts at its
// offset in the music's seconds, later for a positive one, already that far in
// for a negative one; it has a volume and a mute of its own in the Parts fold,
// under the whole music's (D154). The transport moves it: ▶, pause, a seek
// and ■. It is kept to the music's place as a backing track is (D135).
//
// WebKit plays it on AVFoundation, which plays at once from where it was: a
// play() with a seek still under way, from the recording's end, stops at that
// end as the seek lands, while the element still counts as playing and its
// duration shrinks to where it stopped. The recording is then silent under
// the music. So a play() after a seek waits for it, a recording past its end
// waits at its start, and one stopped by itself is started again (D158).
//
// AVFoundation's `currentTime` is the recording as heard: a sample sent now
// is heard the device's latency later, and a seek or a play() holds the time
// that long and some 0.1 s more (D167). So the recording is kept to the music
// as heard, and sent that far ahead of it. Kept to the audio clock instead, a
// seek on a Bluetooth output landed 0.33 s behind, past DRIFT, and the
// recording was put back every 2 s for good.

/**
 * How far, in seconds, a recording that follows the score may stray before
 * it is put back, and how long after it is put back it is left alone (D135):
 * each jump is heard, so only a real drift is worth one. A seek of the
 * transport does not wait: the user asked for that jump.
 */
export const DRIFT = 0.25
export const SETTLE_MS = 2000
/** Closer than this, a recording sent to a place is left where it is. */
const NEAR = 0.03
/** Counting as playing, its time not moving this long, it has stopped by itself (D158). */
export const STUCK_MS = 1000
/** A seek not landed in this long is not waited for any longer before playing. */
const SEEK_WAIT_MS = 2000
/** How long after it was sent somewhere the recording's distance from the music is measured: past AVFoundation's hold (D167). */
export const MEASURE_MS = 1000
/** What AVFoundation takes beyond the device's latency, as far as it is believed. */
const MOST_PIPELINE = 1

/** The recording, as the score names it and as `<audio>` plays it. */
export interface TrackAudio {
  path: string
  url: string
}

export interface AudioTrackOptions {
  onError(message: string): void
  /** The recording's length became known, or changed. */
  onDuration?(): void
}

export class AudioTrack {
  private readonly audio = document.createElement('audio')
  private path: string | undefined
  private start = 0
  private gain = 1
  private master = 1
  private silent = false
  /** When the recording was last put back where the score is, by performance.now(). */
  private corrected = -Infinity
  /** The score's seeks at the last follow(). */
  private seeks = 0
  /** Whether the last follow() found the music playing. */
  private playing = false
  /** Since when, by performance.now(), a play() has waited for a seek to land (D158). */
  private waitingSince: number | undefined
  /** The recording's time when it last moved while playing, and when that was. */
  private lastTime = NaN
  private lastMoved = 0
  /** The longest the recording was known to last: one that shrinks has stopped by itself (D158). */
  private known = 0
  /**
   * What a seek or a play() takes, beyond the output's latency, before the
   * recording is heard where it was sent: learned from each (D167), 0.1 s or so.
   */
  private pipeline = 0
  /** When, by performance.now(), the recording was last sent somewhere or played, until its distance is measured. */
  private sentAt: number | undefined

  constructor(private readonly options: AudioTrackOptions) {
    this.audio.preload = 'auto'
    this.audio.addEventListener('error', () => {
      if (this.path) this.options.onError(`The audio track ${baseName(this.path)} cannot be played`)
    })
    this.audio.addEventListener('durationchange', () => {
      const { duration } = this
      // Shrunk to where AVFoundation stopped it (D158): the file again, which follow() puts back in place.
      if (this.known > 0 && duration > 0 && duration < this.known - 0.5) {
        this.waitingSince = undefined
        this.audio.load()
        return
      }
      this.known = Math.max(this.known, duration)
      this.options.onDuration?.()
    })
  }

  /** The recording to play, or none. */
  load(audio: TrackAudio | undefined): void {
    if (audio?.path === this.path) return
    this.audio.pause()
    this.path = audio?.path
    this.playing = false
    this.waitingSince = undefined
    this.known = 0
    if (audio) this.audio.src = audio.url
    else {
      this.audio.removeAttribute('src')
      this.audio.load()
    }
  }

  /** The recording's file, while there is one. */
  get file(): string | undefined {
    return this.path
  }

  /** How long the recording is; 0 until it is known. */
  get duration(): number {
    return Number.isFinite(this.audio.duration) ? this.audio.duration : 0
  }

  /** Where in the music, in seconds, the recording starts; below 0 it is that far in at the music's 0. */
  get offset(): number {
    return this.start
  }

  set offset(seconds: number) {
    if (seconds === this.start) return
    this.start = seconds
    // Put where the music is now at the next follow, at once.
    this.corrected = -Infinity
    this.seeks = NaN
  }

  /** Its own volume, 0–1. */
  get volume(): number {
    return this.gain
  }

  set volume(volume: number) {
    this.gain = volume
    this.applyVolume()
  }

  /** The whole music's volume, 0–1 (D154). */
  set masterVolume(volume: number) {
    this.master = volume
    this.applyVolume()
  }

  set muted(muted: boolean) {
    this.silent = muted
    this.applyVolume()
  }

  /**
   * The score's music started, stopped or moved: the recording keeps to its
   * place, the music's as heard less the offset. `clock` is the music's place
   * on its audio clock, `latency` ahead of what is heard (D51), counting up
   * through its lead-in (midi.js); the recording's `currentTime` is what is
   * heard of it (D167). Before its start the recording waits at 0; past its
   * end it stays silent.
   */
  follow(clock: number, latency: number, playing: boolean, seeks: number): void {
    const moved = seeks !== this.seeks
    this.seeks = seeks
    const joining = playing && !this.playing
    this.playing = playing
    if (!this.path) return
    const { audio } = this
    const place = clock - latency - this.start
    // Sent this far ahead, it is heard at `place` once AVFoundation has it out.
    const lead = Math.max(latency + this.pipeline, 0)
    const target = place + lead
    if (!playing || target < 0) {
      if (!audio.paused) audio.pause()
      if (playing && audio.currentTime !== 0) audio.currentTime = 0
      return
    }
    const duration = this.duration
    if (duration > 0 && place >= duration) {
      if (!audio.paused) audio.pause()
      // Not left at its end, where the next play() could stop at once (D158).
      if (audio.currentTime !== 0 && !audio.seeking) audio.currentTime = 0
      return
    }
    const now = performance.now()
    // Counting as playing, not moving: stopped by itself (D158); paused, it is started again below.
    if (!audio.paused && !audio.seeking && this.waitingSince === undefined) {
      if (audio.currentTime !== this.lastTime) {
        this.lastTime = audio.currentTime
        this.lastMoved = now
      } else if (now - this.lastMoved > STUCK_MS) {
        audio.pause()
        this.corrected = -Infinity
      }
    }
    this.measure(place, now)
    const off = Math.abs(audio.currentTime - place)
    const strayed = !audio.seeking && now - this.corrected > SETTLE_MS && off > DRIFT
    if (strayed || ((moved || joining || audio.paused) && Math.abs(audio.currentTime - target) > NEAR)) {
      audio.currentTime = target
      this.corrected = now
      this.sentAt = now
    }
    if (this.waitingSince !== undefined && now - this.waitingSince > SEEK_WAIT_MS) this.waitingSince = undefined
    if (audio.paused && this.waitingSince === undefined) this.playOnceSeeked(now)
  }

  /**
   * Once the recording has played a while from where it was sent, how far it
   * is heard from the music says how much more, or less, the next seek should
   * lead by (D167). A recording that is not moving says nothing.
   */
  private measure(place: number, now: number): void {
    const { audio } = this
    if (this.sentAt === undefined || audio.paused || audio.seeking || this.waitingSince !== undefined) return
    if (now - this.sentAt < MEASURE_MS || now - this.lastMoved > STUCK_MS / 2) return
    this.sentAt = undefined
    const behind = place - audio.currentTime
    if (Math.abs(behind) < DRIFT * 2) this.pipeline = Math.min(Math.max(this.pipeline + behind, -MOST_PIPELINE), MOST_PIPELINE)
  }

  /** Plays the recording, once a seek under way has landed (D158). */
  private playOnceSeeked(now: number): void {
    const { audio } = this
    const play = () => {
      this.waitingSince = undefined
      if (!this.playing || !audio.paused || !this.path) return
      this.lastTime = NaN
      this.sentAt = performance.now()
      audio.play().catch((error: unknown) => {
        // A pause or another source came first: nothing failed.
        if (error instanceof Error && error.name === 'AbortError') return
        this.options.onError(`The audio track could not be played: ${error instanceof Error ? error.message : String(error)}`)
      })
    }
    if (!audio.seeking) return play()
    this.waitingSince = now
    audio.addEventListener('seeked', play, { once: true })
  }

  private applyVolume(): void {
    this.audio.volume = this.silent ? 0 : Math.min(Math.max(this.gain * this.master, 0), 1)
  }
}

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path
}
