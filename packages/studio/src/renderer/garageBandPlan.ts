// Export for GarageBand's plan (DECISIONS D162; docs/research/garageband
// README §6.3): what the Rust side's `for_garageband` does to the compiled
// MIDI, worked out here, where the parts, their names and instruments,
// `% BPM:` and bar 1 are known; the files the folder holds beside it, as the
// score's lines name them (stage 2); and what the status line says of the
// export. Pure: player.ts and index.ts gather what they read.
import type { Midi } from '@lily/common/web/midi.js'
import type { TimedBar, TimedStaff } from '@lily/common/types'
import type { ExportedGarageBand, GarageBandAudio, GarageBandPlan, PlaybackSetup } from '../ipc'
import { audioOffsetOfScore, audioOfScore, backingOfScore, musicOfScore, stemOfScore } from './files'
import { mutedParts, partsOf, tempoStretch } from './playbackSetup'
import { barOneTime, tickAt, type TimeSignature } from './tempo'

/** What the plan is made from: the score as compiled and as the Parts fold plays it. */
export interface PlanSource {
  /** The compiled MIDI as parsed, at the score's own tempo (not atTempo's). */
  midi: Midi
  /** The playback map's staves and bars (D26, D48). */
  staves?: TimedStaff[]
  bars: TimedBar[]
  /** The MIDI's first time signature (timeSignatureOf); 4/4 without one. */
  meter: TimeSignature | undefined
  /** The score's `% BPM:` (D155), if it has one. */
  bpm: number | undefined
  /** The Parts fold's choices (D45). */
  setup: PlaybackSetup | undefined
  /** D54's checkbox: leave out the parts the fold mutes. */
  leaveOutMuted: boolean
}

/**
 * The plan for `source`:
 * - `drop`: every track after the first without notes, which are the words'
 *   tracks (and a staff that plays nothing) and GarageBand would make into
 *   empty piano tracks (§4.3), and the muted parts when D54's checkbox says
 *   so (none when every part is muted, as for Export MIDI);
 * - `parts`: each part kept, in track order, named by `partNames` and on the
 *   program the fold chose, else the first one written (0, the piano, when
 *   none is); drums on 0, as the fold leaves their kit;
 * - `tempoRatio`: atTempo's stretch for `% BPM:`;
 * - `leadIn`: after a pickup, a bar of the first meter less the pickup, in
 *   ticks, so the score's bar 1 is GarageBand's bar 2; 0 without one.
 */
export function garageBandPlan(source: PlanSource): GarageBandPlan {
  const { midi, setup } = source
  const parts = partsOf(midi, source.staves)
  const muted = source.leaveOutMuted ? mutedParts(parts, setup).map(({ track }) => track) : []
  const silent = midi.tracks.flatMap((track, index) => (index > 0 && track.notes === 0 ? [index] : []))
  const kept = parts.filter((part) => !muted.includes(part.track))
  return {
    drop: [...silent, ...muted].sort((a, b) => a - b),
    parts: kept.map(({ track, name, drums }) => ({
      track,
      name,
      program: drums ? 0 : (setup?.parts?.[track]?.program ?? midi.tracks[track].programs[0] ?? 0),
    })),
    tempoRatio: tempoStretch(midi, source.bpm),
    leadIn: leadIn(midi, source.bars, source.meter),
  }
}

/**
 * A bar of `meter` less the pickup, in ticks of `midi`: the pickup is how
 * long before bar 1 sounds (barOneTime), turned into ticks by the tempo map,
 * which also counts a pickup with a tempo of its own. 0 without a pickup, or
 * with one a bar long or longer.
 */
export function leadIn(midi: Midi, bars: TimedBar[], meter: TimeSignature | undefined): number {
  const pickup = Math.round(tickAt(midi, barOneTime(bars, midi)))
  if (pickup <= 0) return 0
  const { numerator, denominator } = meter ?? { numerator: 4, denominator: 4 }
  const bar = (midi.division * 4 * numerator) / denominator
  return Math.max(0, Math.round(bar - pickup))
}

/**
 * The files of the score `text` that Export for GarageBand puts in its folder
 * (§6.3), each beside the score: the `% Audio:` recording at its
 * `% Audio offset:`, left out by the Rust side when the Parts fold mutes it
 * (`audioMuted`), and `% Backing:`, `% Music:` and `% Stem:` (D133, D135).
 */
export function garageBandAudio(text: string, audioMuted: boolean): GarageBandAudio {
  return {
    audio: audioOfScore(text),
    audioOffset: audioOffsetOfScore(text),
    audioMuted,
    backing: backingOfScore(text),
    music: musicOfScore(text),
    stem: stemOfScore(text),
  }
}

/**
 * What the status line says Export for GarageBand wrote: the folder and its
 * WAVs, the muted parts left out, and each of the score's files that could
 * not go in, with why. The web's `.mid` alone is named as Export MIDI's is.
 */
export function garageBandExported(exported: ExportedGarageBand): string {
  const name = exported.file.split(/[\\/]/).pop()
  const parts = withoutMuted(exported.leftOut)
  if (!exported.wavs) return `Exported ${name} for GarageBand${parts}`
  const wavs = exported.wavs.length === 0 ? '' : ` with ${listed(exported.wavs)}`
  const leftOut = (exported.leftOutFiles ?? []).map(({ file, why }) => ` Left out ${file}: ${why}.`).join('')
  return `Exported ${name}${wavs}${parts}.${leftOut}`
}

/** What an export says of the `leftOut` muted parts it left out, Export MIDI's too (D54). */
export function withoutMuted(leftOut: number): string {
  return leftOut === 0 ? '' : leftOut === 1 ? ', without the muted part' : `, without the ${leftOut} muted parts`
}

/** `a`, `a and b`, `a, b and c`. */
function listed(names: string[]): string {
  return names.length < 2 ? names.join('') : `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`
}
