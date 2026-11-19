// From the recording (DECISIONS D159): the score's `% BPM:` and `% Audio
// offset:` (D155) from the tempo and bar 1 that lily-transcribe finds in its
// `% Audio:` file. What the score tells it (its tempo, meter and beat), the
// lines written from the answer, and what the status line says of it.
import { momentTime, type Midi } from '@lily/common/web/midi.js'
import type { ScoreMeter, TempoFound, TranscriptionEvent } from '../ipc'
import { hundredths, lineNumber } from './files'
import { atTempo, barTime } from './playbackSetup'

/** A time signature, as the MIDI file's first one has it. */
export interface TimeSignature {
  numerator: number
  denominator: number
}

/**
 * The first time signature of a Standard MIDI File (meta event 0x58), as
 * lilypond writes it in the control track at the start; undefined without one
 * or for what is not a MIDI file.
 */
export function timeSignatureOf(bytes: Uint8Array | undefined): TimeSignature | undefined {
  if (!bytes || bytes.length < 14 || String.fromCharCode(...bytes.subarray(0, 4)) !== 'MThd') return undefined
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  for (let chunk = 8 + view.getUint32(4); chunk + 8 <= bytes.length; ) {
    const end = Math.min(chunk + 8 + view.getUint32(chunk + 4), bytes.length)
    if (String.fromCharCode(...bytes.subarray(chunk, chunk + 4)) === 'MTrk') {
      const found = trackTimeSignature(bytes, chunk + 8, end)
      if (found) return found
    }
    chunk = end
  }
  return undefined
}

function trackTimeSignature(bytes: Uint8Array, start: number, end: number): TimeSignature | undefined {
  let at = start
  let status = 0
  const quantity = () => {
    let value = 0
    for (let i = 0; i < 4 && at < end; i++) {
      const byte = bytes[at++]
      value = (value << 7) | (byte & 0x7f)
      if (!(byte & 0x80)) break
    }
    return value
  }
  while (at < end) {
    quantity()
    let byte = bytes[at]
    if (byte & 0x80) at++
    else byte = status
    if (byte === 0xff) {
      const type = bytes[at++]
      const length = quantity()
      if (type === 0x58 && length >= 2 && at + 1 < end) return { numerator: bytes[at], denominator: 2 ** bytes[at + 1] }
      if (type === 0x2f) return undefined
      at += length
    } else if (byte === 0xf0 || byte === 0xf7) {
      at += quantity()
    } else if (byte & 0x80) {
      status = byte
      at += (byte & 0xf0) === 0xc0 || (byte & 0xf0) === 0xd0 ? 1 : 2
    } else return undefined
  }
  return undefined
}

/**
 * What the score tells lily-transcribe: its tempo in quarter notes a minute,
 * which chooses the metrical level; its beats a bar; and its beat in quarter
 * notes, a dotted one in a compound meter (6/8, 9/8, 12/8: ×1.5). 4/4 without
 * a time signature, as lilypond has it.
 */
export function scoreMeter(bpm: number, signature: TimeSignature | undefined): ScoreMeter {
  const { numerator, denominator } = signature ?? { numerator: 4, denominator: 4 }
  const compound = numerator > 3 && numerator % 3 === 0 && denominator >= 8
  return compound ? { bpm, beatsPerBar: numerator / 3, beat: (3 * 4) / denominator } : { bpm, beatsPerBar: numerator, beat: 4 / denominator }
}

/**
 * When the score's bar 1 sounds, in seconds of `midi`: 0 without a pickup;
 * with `\partial`, the pickup's length. lilypond's bar 1 begins where the
 * pickup ends, so the map's bar 1, or its first bar when it counts from
 * another number, is it.
 */
export function barOneTime(bars: { at: number; number: number }[], midi: Midi): number {
  const timed = bars.map(({ at, number }) => ({ time: momentTime(midi, at), number })).sort((a, b) => a.time - b.time)
  return Math.max(0, barTime(timed, 1) ?? timed[0]?.time ?? 0)
}

/**
 * The tick of `midi` that sounds `seconds` in, by midi.js's tempo map (♩ = 120
 * without one); not rounded. The map's ticks are the file's, so music played
 * at another tempo (atTempo) gives the same tick for its own seconds.
 */
export function tickAt(midi: Midi, seconds: number): number {
  const tempos = (midi as Midi & { tempos?: { tick: number; seconds: number; tempo: number }[] }).tempos ?? []
  let entry = tempos[0] ?? { tick: 0, seconds: 0, tempo: 500_000 }
  for (const next of tempos) if (next.seconds <= seconds + 1e-9) entry = next
  return entry.tick + ((seconds - entry.seconds) * 1e6 * midi.division) / entry.tempo
}

/**
 * The tempo `midi` plays at `seconds` in, in quarter notes a minute: the
 * entry of midi.js's tempo map in force there.
 */
export function bpmAt(midi: Midi, seconds: number): number {
  const tempos = (midi as Midi & { tempos?: { seconds: number; tempo: number }[] }).tempos ?? []
  let tempo = tempos[0]?.tempo ?? 500_000
  for (const entry of tempos) if (entry.seconds <= seconds + 1e-6) tempo = entry.tempo
  return Math.round(6e9 / tempo) / 100
}

/**
 * The score's lines from what was found (D155's labels): `% BPM:` and
 * `% Audio offset:`, the offset putting the score's bar 1 on the recording's.
 * `% BPM:` is the score's first tempo, `first`, as atTempo scales by it; the
 * tempo found is bar 1's, `main`, which differs when a pickup has a tempo of
 * its own (D113's `\tempo 64 = …`). The music is the recording's place plus
 * the offset, so the offset is bar 1's time in the music at the new tempo less
 * its time in the recording. A value that is the default, the score's own
 * tempo or 0, takes its line out.
 */
export function tempoLines(
  found: Pick<TempoFound, 'bpm' | 'barOne'>,
  { first, main }: { first: number; main: number },
  barOne: (bpm: number) => number,
): Record<'BPM' | 'Audio offset', string | undefined> {
  const bpm = hundredths((found.bpm * first) / main)
  const offset = hundredths(barOne(bpm) - found.barOne)
  return { BPM: bpm === first ? undefined : lineNumber(bpm), 'Audio offset': offset === 0 ? undefined : lineNumber(offset) }
}

/** The status line while lily-transcribe listens to the recording. */
export function tempoProgress(name: string, stage: (TranscriptionEvent & { event: 'progress' })['stage']): string {
  const doing = stage === 'decode' ? 'reading it' : stage === 'beats' ? 'finding the beats' : 'listening'
  return `Listening to ${name}: ${doing}…`
}

/**
 * What the status line says was found and how sure it is, with what is off:
 * a meter that is not the score's, another metrical level than was heard, a
 * tempo that drifts.
 */
export function tempoStatus(found: TempoFound, meter: ScoreMeter, signature: TimeSignature | undefined): string {
  const bpm = lineNumber(found.bpm)
  const shown = signature ? `${signature.numerator}/${signature.denominator}` : `${found.beatsPerBar} beats a bar`
  const said = [`♩ = ${bpm}, ${shown}, bar 1 at ${lineNumber(found.barOne)} s; ${Math.round(found.share * 100)} % of the beats on one tempo`]
  if (Math.abs(found.heardBpm - found.bpm) >= 0.5) said.push(`heard ♩ = ${lineNumber(found.heardBpm)}, taken as ${bpm} to match the score's ${lineNumber(meter.bpm ?? found.bpm)}`)
  if (found.heardBeatsPerBar !== found.beatsPerBar) said.push(`the downbeats heard make ${found.heardBeatsPerBar} beats a bar, not the score's ${found.beatsPerBar}`)
  if (!found.steady) said.push(`the tempo drifts: one tempo keeps it within ${lineNumber(Math.max(found.residualMs / 1000, 0.01))} s`)
  return said.join('; ')
}

/** When the score's bar 1 sounds with `midi`, the score's own MIDI, played at `bpm` (atTempo). */
export function barOneAt(bars: { at: number; number: number }[], midi: Midi): (bpm: number) => number {
  return (bpm) => barOneTime(bars, atTempo(midi, bpm))
}
