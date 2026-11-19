// C2 through the core (MS5 steps 1 to 8, 07 §4): the MIDI files the
// engine's performers write (crates/music's performers.rs,
// dynamic_performer.rs, control_performers.rs, mark_performers.rs,
// melisma.rs' Tie_performer and performance.rs, crates/smf), <name>.midi and <name>-1.midi as write-performances-midis
// names them, scored by score.mjs against LilyPond 2.26.0's. First the
// fixtures, captured here: fixtures/midi-notes.ly (two scores, staves named
// and not, rests, chords, a unison, grace notes and a tuplet),
// fixtures/midi-dynamic-spans.ly (Dynamic_performer's hairpins, absolute
// dynamics, volume limits and the instrument equalizer),
// fixtures/midi-controls.ly (tempo, key and time signature changes,
// programs, the five controllers, pedals, microtones and their warnings; a
// second score's key and pedals in skipped timesteps),
// fixtures/midi-marks.ly (markers, ties, breaths and lyrics),
// fixtures/midi-channels.ly (drums, percussion on channel 10, the
// 16-channel wrap and its warnings, the three midiChannelMapping values)
// fixtures/midi-beats.ly (scheme-performers.scm's Beat_performer)
// fixtures/midi-repeats.ly (\unfoldRepeats over each repeat type,
// repeats played folded, articulate.ly's \articulate, \fine and its
// warnings), fixtures/midi-part-combine.ly (\partCombine's voices,
// forced and automatic, and \partCombineUp on a channel a voice) and
// fixtures/midi-walker.ly (Midi_walker going back in MIDI time, and notes
// shorter than a tick: step 8's),
// natively and in one core.wasm instance; C2 = 1.0, velocities with it,
// their files' bytes (C2b) LilyPond's, stderr LilyPond's and the
// diagnostics parseStderr's of it. Then R4's gate (README §5, MS5; step
// 8): the 42 references, natively and in one core.wasm instance, C2 = 1.0
// with every velocity LilyPond's, their bytes LilyPond's as they are, and
// stderr LilyPond's but where layout makes the difference (NOT_MIDI); C2b
// through score.mjs is c2b.test.mjs', C1 c1.test.mjs'. The tests skip themselves,
// and so verify nothing, without LilyPond 2.26.0, cargo, the corpus and
// its capture, and the wasm ones without a core.wasm as new as the
// sources.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { after, test } from 'node:test'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { captureOne, lilypondVersion } from '../capture.mjs'
import { compileInWasm, coreSkip } from '../core.mjs'
import { compare, read, smfEvents } from '../score.mjs'
import { parseStderr } from '../stderr.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
const fixture = (id) => ({ id, file: `tools/compare/fixtures/${id}.ly`, path: path.join(here, `../fixtures/${id}.ly`) })
// Each fixture and its files.
const FIXTURES = [
  [fixture('midi-notes'), ['midi-notes-1.midi', 'midi-notes.midi']],
  [fixture('midi-dynamic-spans'), ['midi-dynamic-spans.midi']],
  [fixture('midi-controls'), ['midi-controls-1.midi', 'midi-controls.midi']],
  [fixture('midi-marks'), ['midi-marks-1.midi', 'midi-marks.midi']],
  [fixture('midi-channels'), ['midi-channels-1.midi', 'midi-channels-2.midi', 'midi-channels-3.midi', 'midi-channels.midi']],
  [fixture('midi-beats'), ['midi-beats.midi']],
  [fixture('midi-repeats'), ['midi-repeats-1.midi', 'midi-repeats-2.midi', 'midi-repeats-3.midi', 'midi-repeats.midi']],
  [fixture('midi-part-combine'), ['midi-part-combine-1.midi', 'midi-part-combine.midi']],
  [fixture('midi-walker'), ['midi-walker.midi']],
]
// R4's gate: the 42, in the order of the steps whose performers they first passed with
// (refs.txt): steps 1 to 5's, M12 at step 6a, then step 6's M10 and T19.
const GATE = [
  'M11', 'T01', 'T03', 'T05', 'T06', 'T08', 'T09', 'T10', 'T11', 'T13', 'T14', 'T15', 'T16', 'T17',
  'M08', 'T18', 'T25',
  'M01', 'M02', 'M03', 'M04', 'M07', 'M09', 'T21', 'T27', 'T28',
  'M05', 'T02', 'T04', 'T12', 'T20', 'T22', 'T23', 'T24', 'T26', 'T30',
  'M06', 'M12', 'T07', 'T29',
  'M10', 'T19',
]
// The references whose stderr differs from LilyPond's in what layout writes, not MIDI, and
// why; each is R5's or later (runway/OPEN.md A41 and C6, the display-methods item). T25's stderr
// is LilyPond's since MS4 step 2: its note collision's \voiceXx warning (A41) went with its
// beams, which it lays out with since step 4; T29's since step 10, laid out whole with its ties
// (its VerticalAxisGroups' cyclic dependency LilyPond's); T26's since step 13, laid out whole
// with its tuplets (D99's compression by 4.6 staff spaces LilyPond's).
// T15's since MS8 step 11: its events and music printed as LilyPond's Probs, with their display-methods (C6).
const NOT_MIDI = {
  // Since MS4 step 15 its lyrics are drawn: C059's "no glyph" warnings for
  // its Hebrew and Japanese, which LilyPond took from the system's fonts
  // (D224, D227; step 19's fallback, R8's OffscreenCanvas).
  T20: 'lyrics in fonts no pack holds',
}
const rows = Object.fromEntries(corpus().map((row) => [row.id, row]))
const ref = path.join(corpusDir(), 'ref')
const lilypond = lilypondVersion() !== '2.26.0' && 'LilyPond 2.26.0 is not installed'
const cargo = spawnSync('cargo', ['+stable', '--version']).status !== 0 && 'cargo +stable is not installed'
const ids = GATE
const references = !ids.every((id) => existsSync(rows[id]?.path ?? '') && existsSync(path.join(ref, id))) && 'the corpus or its capture is not there'
const scratch = mkdtempSync(path.join(tmpdir(), 'lily-c2-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** A file's C2b view: smfEvents, its note-offs at one tick sorted. */
const bytesOf = (bytes) => smfEvents(bytes, { sortTies: true })

/** `files` ({ name: bytes }) alone in a directory of their own, for score.mjs. */
function candidate(name, files) {
  const dir = path.join(scratch, name)
  mkdirSync(dir, { recursive: true })
  for (const [f, bytes] of Object.entries(files)) writeFileSync(path.join(dir, f), bytes)
  return dir
}

/** The .midi files of `dir`, or of an outputs object, by name. */
function midis(from) {
  if (typeof from === 'string') return Object.fromEntries(readdirSync(from).filter((f) => f.endsWith('.midi')).map((f) => [f, readFileSync(path.join(from, f))]))
  return Object.fromEntries(Object.entries(from).filter(([f]) => f.endsWith('.midi')))
}

/** C2 of `files` against the capture `refDir`, and the names whose bytes (C2b) are LilyPond's. */
function score(refDir, files, name) {
  const result = compare(refDir, candidate(name, files))
  const names = Object.keys(midis(refDir))
  const equal = names.filter((f) => files[f] && bytesOf(readFileSync(path.join(refDir, f))) === bytesOf(files[f]))
  return { C2: result.midiNotes, names: Object.keys(files).sort(), reference: names.sort(), equal: equal.sort() }
}

/**
 * The velocities of `files` against the capture `refDir`: LilyPond's notes each paired with one
 * of the same file, track, channel, key and time; how many pair, and those whose velocities differ.
 */
function velocities(refDir, files, name) {
  const split = (n) => { const f = n.split('|'); return [[f[0], f[1], f[2], f[3], f[6]].join('|'), f[5]] }
  const engine = new Map()
  for (const n of read(candidate(name, files)).notes) { const [k, v] = split(n); engine.set(k, [...(engine.get(k) ?? []), v]) }
  let paired = 0
  const differ = []
  for (const n of read(refDir).notes) {
    const [k, v] = split(n)
    const vs = engine.get(k)
    if (!vs?.length) continue
    paired++
    const i = vs.indexOf(v)
    if (i >= 0) vs.splice(i, 1)
    else differ.push(`${k}: ${v}, not ${vs.shift()}`)
  }
  return { paired, differ }
}

const captured = {}
/** LilyPond's capture of the fixture `f`. */
const capture = (f) => {
  captured[f.id] ??= (async () => {
    const dir = path.join(scratch, `ref-${f.id}`)
    const result = await captureOne('lilypond', f, dir)
    assert.equal(result.exit, 0)
    return dir
  })()
  return captured[f.id]
}

let compiled
/** The fixtures and the references, compiled natively in one engine. */
const native = () => {
  if (!compiled) {
    compiled = path.join(scratch, 'engine')
    mkdirSync(compiled, { recursive: true })
    const files = [...FIXTURES.map(([f]) => f.path), ...(references ? [] : ids.map((id) => rows[id].path))]
    const run = runExample('compile', ['--out', compiled, ...files], { encoding: 'utf8' })
    // 1: a file failed, as T03 does in LilyPond and T15 at ly:expect-warning.
    assert.ok(run.status === 0 || run.status === 1, run.stderr)
  }
  return compiled
}

const ofStem = (dir, stem) => Object.fromEntries(Object.entries(midis(dir)).filter(([f]) => f === `${stem}.midi` || f.startsWith(`${stem}-`)))

for (const [f, names] of FIXTURES) {
  test(`fixtures/${f.id}.ly's MIDI files natively: C2 = 1.0, their bytes LilyPond's, stderr LilyPond's`,
    { skip: lilypond || cargo, timeout: 600_000 }, async () => {
      const dir = await capture(f)
      const out = native()
      const files = ofStem(out, f.id)
      const r = score(dir, files, `native-${f.id}`)
      assert.deepEqual(r, { C2: 1, names, reference: names, equal: names })
      assert.deepEqual(velocities(dir, files, `native-${f.id}-velocities`).differ, [])
      // Whole: but the creator text, every byte as it is, the note-offs in LilyPond's order too.
      for (const [name, bytes] of Object.entries(files)) assert.equal(smfEvents(bytes), smfEvents(readFileSync(path.join(dir, name))), name)
      const stderr = readFileSync(path.join(out, `${f.id}.err`), 'utf8')
      assert.equal(stderr, readFileSync(path.join(dir, 'stderr.txt'), 'utf8'))
      assert.deepEqual(JSON.parse(readFileSync(path.join(out, `${f.id}.json`), 'utf8')), parseStderr(stderr, { rootFile: f.path }))
    })
}

test("the fixtures in core.wasm: the native core's MIDI files", { skip: lilypond || cargo || coreSkip(), timeout: 600_000 }, async () => {
  const out = native()
  const wasm = await compileInWasm(FIXTURES.map(([f]) => f.path))
  for (const [i, [f, names]] of FIXTURES.entries()) {
    const dir = await capture(f)
    const files = midis(wasm[i].outputs)
    assert.deepEqual(Object.keys(files).sort(), names)
    for (const [name, bytes] of Object.entries(files)) assert.deepEqual(Buffer.from(bytes), readFileSync(path.join(out, name)), name)
    assert.equal(score(dir, files, `wasm-${f.id}`).C2, 1)
  }
  // The MIDI files after the pages and before the playback map.
  assert.deepEqual(Object.keys(wasm[0].outputs), ['stderr', 'diagnostics.json', 'midi-notes.svg', 'midi-notes.midi', 'midi-notes-1.midi', 'midi-notes.timing.json'])
})

test("R4's gate: the 42 references natively and in core.wasm, C2 = 1.0 with LilyPond's velocities, their bytes LilyPond's, stderr LilyPond's but layout's",
  { skip: lilypond || cargo || references, timeout: 900_000 }, async (t) => {
    const out = native()
    const wasm = coreSkip() ? null : await compileInWasm(GATE.map((id) => rows[id].path))
    assert.equal(new Set(GATE).size, 42)
    for (const [i, id] of GATE.entries()) {
      const stem = path.basename(rows[id].path, '.ly')
      const files = ofStem(out, stem)
      const r = score(path.join(ref, id), files, `native-${id}`)
      assert.equal(r.C2, 1, id)
      assert.deepEqual(r.names, r.reference, id)
      assert.deepEqual(r.equal, r.reference, id)
      const v = velocities(path.join(ref, id), files, `native-${id}-velocities`)
      assert.equal(v.paired, read(path.join(ref, id)).notes.length, id)
      assert.deepEqual(v.differ, [], id)
      // Whole: the note-offs at one tick in LilyPond's order too.
      for (const [name, bytes] of Object.entries(files)) assert.equal(smfEvents(bytes), smfEvents(readFileSync(path.join(ref, id, name))), `${id} ${name}`)
      const stderr = readFileSync(path.join(out, `${stem}.err`), 'utf8')
      const theirs = readFileSync(path.join(ref, id, 'stderr.txt'), 'utf8')
      if (NOT_MIDI[id]) assert.notEqual(stderr, theirs, `${id}: ${NOT_MIDI[id]}, now LilyPond's`)
      else assert.equal(stderr, theirs, id)
      if (wasm) {
        const inWasm = midis(wasm[i].outputs)
        assert.deepEqual(Object.keys(inWasm).sort(), r.names, id)
        for (const [f, bytes] of Object.entries(inWasm)) assert.deepEqual(Buffer.from(bytes), files[f], `${id} ${f}`)
        assert.equal(Buffer.from(wasm[i].outputs.stderr).toString('utf8'), stderr, id)
      }
    }
    if (!wasm) t.diagnostic(`core.wasm not compared: ${coreSkip()}`)
  })
