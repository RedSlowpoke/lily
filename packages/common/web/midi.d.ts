// Types of midi.js, the parser and player that the extension's webviews and
// Lily Studio play with (D24).

export interface Note {
  time: number
  duration: number
  /** The index of its track in `tracks`. */
  track: number
  channel: number
  key: number
  /** The note-on's velocity over 127: what dynamics and accents give it. */
  velocity: number
  program: number
  /** Volume × expression of its channel when it began, 0–1. */
  gain: number
  /** -1 (left) to 1. */
  pan: number
  /** Pitch bend, in cents. */
  detune: number
  /** It begins as the note before it on its track and channel ends, alone: a sung one glides to it (D150). */
  legato?: boolean
}
export interface Track {
  name: string
  channels: number[]
  programs: number[]
  notes: number
}
export interface Midi {
  title: string
  duration: number
  notes: Note[]
  tracks: Track[]
  division: number
}
export type PlayerState = 'stopped' | 'playing' | 'paused'
export function parseMidi(bytes: Uint8Array): Midi
export function formatTime(seconds: number): string
export function instrumentName(program: number): string
/** What a track plays: its programs' names, and `drums`. */
export function trackInstruments(track: Track): string
export function momentTime(midi: Midi, at: number, grace?: number): number
/**
 * The synthesizer the player plays on. On an OfflineAudioContext it renders
 * the music to a buffer: play every note at its time, then startRendering().
 */
export class Synth {
  /** `volumes`: track → its volume, 0–2; 1 when not there. */
  constructor(context: BaseAudioContext, volumes?: Map<number, number>)
  /** Schedules `note` to begin at context time `when`, `skipped` seconds into it. */
  play(note: Note, when: number, skipped?: number): void
}
export class Player {
  constructor(options?: { onChange?: () => void; createContext?: () => BaseAudioContext })
  readonly midi: Midi | undefined
  readonly state: PlayerState
  /** Where the music is as it is heard: the audio clock less the output latency (D51). */
  readonly position: number
  /** Where the music is on the audio clock, `latency` ahead of `position`; a pause goes on from here. */
  readonly scheduled: number
  /** `scheduled`, counting up through the lead-in of a play() or a seek instead of waiting at its start (D167). */
  readonly clock: number
  readonly latency: number
  readonly duration: number
  /** The last play() was refused, or the system took the audio away; not the rest of an idle player (D151). */
  readonly suspended: boolean
  /** A play() is waiting for the audio clock, and starts by itself (D151). */
  readonly starting: boolean
  load(midi: Midi | undefined): void
  play(): Promise<boolean>
  pause(): void
  stop(): void
  seek(seconds: number): Promise<void>
  /** Track `track`'s volume: 1 as the file has it, 0 silent, up to 2. */
  trackVolume(track: number): number
  /** Applies at once, to what sounds too, and outlasts load(). */
  setTrackVolume(track: number, volume: number): void
  /** The whole music's volume, 0–1, over each track's: 1 as the file has it. */
  readonly volume: number
  /** Applies at once and outlasts load(); outside 0–1 is the nearer end. */
  setVolume(volume: number): void
  dispose(): void
}
