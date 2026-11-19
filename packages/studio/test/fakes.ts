// Fakes of the DOM and the audio clock, as much as the players touch (player.test.ts, audioTrack.test.ts),
// and a score's MIDI and lily-transcribe's tempo for From the recording (tempo.test.ts, D159).
import type { TempoFound } from '../src/ipc'

/** As much of an element as the Stem strip touches. */
export class FakeElement {
  dataset: Record<string, string> = {}
  hidden = false
  title = ''
  textContent = ''
  value = ''
  placeholder = ''
  disabled = false
  className = ''
  type = ''
  min = ''
  max = ''
  children: FakeElement[] = []
  attributes = new Map<string, string>()
  classList = { add() {}, remove() {}, toggle() {} }
  style = {}
  private readonly listeners = new Map<string, { listener: () => void; once: boolean }[]>()

  addEventListener(type: string, listener: () => void, options?: { once?: boolean }): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), { listener, once: !!options?.once }])
  }

  fire(type: string): void {
    const listeners = this.listeners.get(type) ?? []
    this.listeners.set(type, listeners.filter(({ once }) => !once))
    for (const { listener } of listeners) listener()
  }

  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value)
  }

  removeAttribute(name: string): void {
    this.attributes.delete(name)
  }

  replaceChildren(...children: FakeElement[]): void {
    this.children = children
  }

  append(...children: FakeElement[]): void {
    this.children.push(...children)
  }

  remove(): void {}

  querySelector(): null {
    return null
  }

  /** The element of this class, itself or below it. */
  find(className: string): FakeElement | undefined {
    // Text appended among the children is not an element.
    return this.className === className ? this : this.children.map((child) => (child instanceof FakeElement ? child.find(className) : undefined)).find(Boolean)
  }
}

export class FakeAudio extends FakeElement {
  readonly HAVE_METADATA = 1
  readyState = 0
  paused = true
  ended = false
  duration = 200
  seeking = false
  preload = ''
  volume = 1
  /** A source still loading: play() waits, as it does on a real one. */
  buffering = false
  /** Every place it was sent to, in order. */
  sent: number[] = []
  /** Its seeks land only at landSeek(), as a real one's take a while. */
  slowSeeks = false
  /** WebKit over AVFoundation (D158): played during a seek away from its end, it stopped there as the seek landed, still counting as playing. */
  stalled = false
  private place = 0
  private seekFrom: number | undefined
  private readonly fullDuration = this.duration
  private waiting: ((reason: DOMException) => void) | undefined

  get currentTime(): number {
    return this.place
  }

  set currentTime(place: number) {
    if (this.slowSeeks) {
      this.seekFrom ??= this.place
      this.seeking = true
    }
    this.place = place
    this.sent.push(place)
  }

  /** The seek under way landed. */
  landSeek(): void {
    const from = this.seekFrom
    this.seeking = false
    this.seekFrom = undefined
    if (!this.paused && from !== undefined && from >= this.duration - 0.1) {
      this.stalled = true
      this.duration = this.place
      this.fire('durationchange')
    }
    this.fire('seeked')
  }

  /** The media load algorithm: a new source pauses it, at 0, knowing nothing yet. */
  set src(_: string) {
    this.abort()
    this.paused = true
    this.ended = false
    this.place = 0
    this.readyState = 0
  }

  /** Its metadata arrived. */
  loaded(): void {
    this.readyState = this.HAVE_METADATA
    this.fire('durationchange')
    this.fire('loadedmetadata')
  }

  /** Where it has got to by playing, which nobody set. */
  reach(place: number): void {
    if (!this.stalled) this.place = place
  }

  /** It played to its end. */
  finish(): void {
    this.place = this.duration
    this.paused = true
    this.ended = true
    this.fire('ended')
  }

  /** Loads its source again: a stalled one is whole again, paused at its start. */
  load(): void {
    if (!this.stalled) return
    this.abort()
    this.stalled = false
    this.duration = this.fullDuration
    this.paused = true
    this.place = 0
    this.fire('durationchange')
  }

  pause(): void {
    this.abort()
    this.paused = true
  }

  /** At its end it starts again from its start. */
  async play(): Promise<void> {
    if (this.ended) this.place = 0
    this.ended = false
    this.paused = false
    if (this.buffering) await new Promise((_, reject) => (this.waiting = reject))
  }

  /** What a pause or another source does to a play still waiting. */
  private abort(): void {
    this.waiting?.(new DOMException('The operation was aborted.', 'AbortError'))
    this.waiting = undefined
  }
}

/** An audio graph that goes nowhere, on a clock the test moves. */
export function fakeContext() {
  const node = (): unknown => new Proxy(() => {}, { get: (_, key) => (key === 'then' ? undefined : key === 'connect' ? (next: unknown) => next : node()), apply: () => node(), set: () => true })
  const clock = { time: 0, state: 'running', resumeMs: 0, /** outputLatency, in seconds. */ latency: 0 }
  const context = new Proxy({} as AudioContext, {
    get: (_, key) => {
      if (key === 'currentTime') return clock.time
      if (key === 'state') return clock.state
      if (key === 'suspend') return async () => void (clock.state = 'suspended')
      if (key === 'resume') {
        return () =>
          new Promise<void>((resolve) =>
            setTimeout(() => {
              clock.state = 'running'
              resolve()
            }, clock.resumeMs),
          )
      }
      if (key === 'addEventListener') return () => {}
      if (key === 'outputLatency') return clock.latency
      if (key === 'baseLatency') return 0
      return () => node()
    },
  })
  return { context, clock }
}

/** One quarter note at ♩ = `bpm` in `numerator`/`denominator`, 480 ticks to the quarter, as lilypond writes the control track. */
export function scoreMidi(bpm: number, numerator = 4, denominator = 4): Uint8Array {
  const tempo = Math.round(60e6 / bpm)
  const events = [
    // Running status after the first note-on, and a text event first, as a reader must skip them.
    0, 0xff, 0x03, 4, 0x6c, 0x69, 0x6c, 0x79,
    0, 0xff, 0x58, 4, numerator, Math.log2(denominator), 0x18, 0x08,
    0, 0xff, 0x51, 3, (tempo >> 16) & 0xff, (tempo >> 8) & 0xff, tempo & 0xff,
    0, 0x90, 60, 100,
    0x83, 0x60, 60, 0,
    0, 0xff, 0x2f, 0,
  ]
  return new Uint8Array([0x4d, 0x54, 0x68, 0x64, 0, 0, 0, 6, 0, 0, 0, 1, 0x01, 0xe0, 0x4d, 0x54, 0x72, 0x6b, 0, 0, 0, events.length, ...events])
}

export const FOUND: TempoFound = {
  bpm: 113.76,
  heardBpm: 113.76,
  steady: true,
  share: 0.99,
  residualMs: 19.2,
  beatsPerBar: 4,
  heardBeatsPerBar: 4,
  barOne: 0.322,
  seconds: 216.7,
}
