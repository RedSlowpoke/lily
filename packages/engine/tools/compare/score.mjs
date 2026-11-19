// Chapter 00's comparator (§5.2, Appendix A): scores a candidate's output
// against LilyPond's reference capture (capture.mjs), layer by layer.
//
//   node tools/compare/score.mjs <reference dir> <candidate dir>
//                                          or `npm run score -- <ref> <cand>` in packages/engine
//
// Each directory is one score's capture, or holds one per ID (T01…, M01…):
// then every ID of the reference is scored, one JSON line each, and a last
// line counts the scores that pass each layer's Must.
//
//   C1 events     Dice of the playback map's events (performance, link, moment, grace, length)
//   C2 midiNotes  Dice of every .midi's notes (file, track, channel, key, program,
//                 velocity, time and duration to 0.1 ms)
//   C2b midiBytes every .midi by name, its header and each track's events (tick and
//                 bytes as written) with the creator text left out: the files equal
//                 as they are, and once each run of note-offs at one tick is sorted,
//                 which LilyPond's heap orders (07 §4.2, §4.3); a Goal, not a Must
//   C3 pages      page counts; breaks: Dice of the moments where a system starts
//   C4 dx, dy     median and p95 of |Δx| and |Δy| in staff spaces, of each linked
//                 element paired in document order per link, on the same page
//   C5 shape      median and p95 in staff spaces of each beam's two positions
//                 (|Δ|) and each slur's, phrasing slur's and tie's control points
//                 (distance), from chapter 05's dump (c5dump.ly, c5.tsv), of each
//                 grob paired by type and its event's link, in order of system, X,
//                 Y and height (a beam's left end, a curve's first point), in the
//                 same system (05 §9)
//
// A link is file:line:char, without its directory and derived column.
//
// C0, the diagnostics (00 §5.2, D64), scores two stderrs instead, each through
// stderr.mjs's copy of the extension's parseStderr:
//
//   node tools/compare/score.mjs [--parse-only] <reference stderr> <candidate stderr>
//   node tools/compare/score.mjs [--parse-only] --errors <candidate dir> [case…]
//
// The second form scores each case of the error corpus (chapter 02 §7) against
// <candidate dir>/<case>.err, one JSON line each, then a count of those that pass.
//   C0 diagnostics  the first diagnostic's file (without its directory), line, column,
//                   severity and message equal; the same number of errors and of warnings.
//                   --parse-only leaves out on both sides what LilyPond warns while it
//                   processes a score (PROCESSING), which a parser alone does not reach
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { ERRORS, errorCases, parseStderr } from './stderr.mjs'

const { parseMidi } = createRequire(import.meta.url)('@lily/common/web/midi.js')

// Each layer's Must (§5.2): a score passes a layer when it meets it.
export const MUST = { events: 1, midiNotes: 1, breaks: 0.9, median: 0.5, p95: 2, shape: 0.25 }
// C5's Goal (05 §9) and C2b's (07 §4.3); the other layers' Goals are not judged here.
export const GOAL = { shape: 0.05 }
// The shape dump capture.mjs has c5dump.ly write into each capture.
export const SHAPES = 'c5.tsv'
// The grobs whose shapes C5 compares, as c5dump.ly writes them.
const SHAPED = new Set(['Beam', 'Slur', 'PhrasingSlur', 'Tie'])

const files = (dir, re) => readdirSync(dir).filter((f) => re.test(f))
  .sort((a, b) => (+(a.match(/-(\d+)\.svg$/)?.[1] ?? 0)) - (+(b.match(/-(\d+)\.svg$/)?.[1] ?? 0)))
// A link without its directory and its derived COLUMN: file:line:char.
export const key = (href) => href.replace(/^textedit:\/\//, '').replace(/^.*\//, '').replace(/:\d+$/, '')

/**
 * C5's shapes from a c5dump.ly dump, by type|file:line:char, each key's grobs in order of
 * system, X, Y and height (a chord's ties share the rest): a beam's positions [left, right],
 * a slur's or tie's control points [[x, y] × 4]. null without a dump.
 */
export function readShapes(file) {
  if (!existsSync(file)) return null
  const shapes = new Map()
  for (const line of readFileSync(file, 'utf8').split('\n')) {
    const [kind, type, link, sys, x, y, digest = ''] = line.split('\t')
    if (kind !== 'G' || !SHAPED.has(type)) continue
    const v = (digest.match(/-?\d+(?:\.\d+)?(?:e-?\d+)?/g) ?? []).map(Number)
    if (!v.length) continue
    const k = `${type}|${link}`; if (!shapes.has(k)) shapes.set(k, [])
    shapes.get(k).push({ sys: +sys, x: +x, y: +y, v: type === 'Beam' ? v : v.flatMap((_, i) => i % 2 ? [] : [[v[i], v[i + 1]]]) })
  }
  const height = (g) => typeof g.v[0] === 'number' ? g.v[0] : g.v[0][1]
  for (const list of shapes.values()) list.sort((a, b) => a.sys - b.sys || a.x - b.x || a.y - b.y || height(a) - height(b))
  return shapes
}

/**
 * C2b's view of a Standard MIDI File: its header and, per track, each event as `tick hex`,
 * without the control track's `creator: ` text and the version text after it; with
 * `sortTies`, each run of note-offs at one tick in order of their bytes. null if it does not parse.
 */
export function smfEvents(bytes, { sortTies = false } = {}) {
  if (bytes.length < 14 || bytes.toString('latin1', 0, 4) !== 'MThd') return null
  const hex = (from, to) => bytes.subarray(from, to).toString('hex')
  const tracks = [hex(8, 8 + bytes.readUInt32BE(4))]
  for (let at = 8 + bytes.readUInt32BE(4); at + 8 <= bytes.length;) {
    const end = at + 8 + bytes.readUInt32BE(at + 4)
    if (end > bytes.length) return null
    if (bytes.toString('latin1', at, at + 4) !== 'MTrk') { at = end; continue }
    const events = []; let p = at + 8; let tick = 0; let status = 0
    const varlen = () => { let v = 0; let c; do { c = bytes[p++]; v = v * 128 + (c & 0x7f) } while (c & 0x80 && p < end); return v }
    while (p < end) {
      tick += varlen()
      const from = p
      if (bytes[p] & 0x80) status = bytes[p++]
      if (status === 0xff) p++
      // A meta or system exclusive event's length, then its data.
      const length = status >= 0xf0 ? varlen() : (status & 0xe0) === 0xc0 ? 1 : 2
      p += length
      if (p > end) return null
      events.push({ tick, bytes: bytes.subarray(from, p) })
    }
    tracks.push(events)
    at = end
  }
  // The creator text: `FF 01` "creator: ", then the program's name and version.
  const control = tracks[1] ?? []
  const creator = control.findIndex((e) => e.bytes.toString('latin1', 0, 2) === '\xff\x01' && e.bytes.toString('latin1', 3) === 'creator: ')
  if (creator >= 0) control.splice(creator, control[creator + 1]?.bytes[1] === 0x01 ? 2 : 1)
  const off = (e) => ((e.bytes[0] & 0xf0) === 0x90 && e.bytes[2] === 0) || (e.bytes[0] & 0xf0) === 0x80
  return tracks.map((events, t) => {
    if (t === 0) return events
    const lines = events.map((e) => `${e.tick} ${e.bytes.toString('hex')}`)
    if (sortTies) {
      for (let i = 0; i < events.length;) {
        let j = i + 1
        if (off(events[i])) while (j < events.length && off(events[j]) && events[j].tick === events[i].tick) j++
        lines.splice(i, j - i, ...lines.slice(i, j).sort())
        i = j
      }
    }
    return lines.join('\n')
  }).join('\n--\n')
}

/** One capture's layers: page count, links by key, events, MIDI notes, system breaks and shapes. */
export function read(dir) {
  const links = new Map() // key → [{page, x, y}] in document order
  files(dir, /\.svg$/).forEach((page, p) => {
    const svg = readFileSync(path.join(dir, page), 'utf8')
    for (const m of svg.matchAll(/xlink:href="(textedit:[^"]+)">\s*<g transform="translate\(([-\d.]+), ([-\d.]+)\)">/g)) {
      const k = key(m[1]); if (!links.has(k)) links.set(k, [])
      links.get(k).push({ page: p, x: +m[2], y: +m[3] })
    }
  })
  const timing = files(dir, /\.timing\.json$/).map((f) => JSON.parse(readFileSync(path.join(dir, f), 'utf8')))[0] ?? []
  const events = timing.flatMap((perf, i) => perf.events.map((e) => `${i}|${key(e.href)}|${e.at}|${e.grace}|${e.length}`))
  const midi = new Map(files(dir, /\.midi?$/).map((f) => [f, readFileSync(path.join(dir, f))]))
  const notes = files(dir, /\.midi?$/).flatMap((f) => parseMidi(readFileSync(path.join(dir, f))).notes
    .map((n) => `${f}|${n.track}|${n.channel}|${n.key}|${n.program}|${Math.round(n.velocity * 127)}|${n.time.toFixed(4)}|${n.duration.toFixed(4)}`))
  // A system break is where the music's columns jump back to the left, or onto a new page.
  const moments = new Map()
  for (const perf of timing.slice(0, 1)) for (const e of perf.events) {
    const at = links.get(key(e.href))?.[0]; if (!at) continue
    const m = `${e.at}|${e.grace}`; const c = moments.get(m) ?? { at: e.at, grace: e.grace, x: Infinity, page: at.page }
    c.x = Math.min(c.x, at.x); moments.set(m, c)
  }
  const order = [...moments.values()].sort((a, b) => a.at - b.at || a.grace - b.grace)
  const breaks = new Set(order.filter((c, i) => i > 0 && (c.page !== order[i - 1].page || c.x < order[i - 1].x - 10))
    .map((c) => `${c.at}|${c.grace}`))
  return { pages: files(dir, /\.svg$/).length, links, events, notes, midi, breaks, shapes: readShapes(path.join(dir, SHAPES)) }
}

// Dice coefficient of two multisets: the same music may be engraved twice (ChordNames and Staff).
export const same = (a, b) => {
  const left = new Map(); for (const x of b) left.set(x, (left.get(x) ?? 0) + 1)
  const hit = a.filter((x) => left.get(x) > 0 && left.set(x, left.get(x) - 1)).length
  return a.length + b.length === 0 ? 1 : 2 * hit / (a.length + b.length)
}

const q = (v, p) => v.length ? v.sort((a, b) => a - b)[Math.min(v.length - 1, Math.floor(p * v.length))] : null

/** Scores the capture in `candDir` against the one in `refDir`. */
export function compare(refDir, candDir) {
  const ref = read(refDir); const cand = read(candDir)
  const dx = []; const dy = []
  for (const [k, r] of ref.links) {
    const c = cand.links.get(k); if (!c || c.length !== r.length) continue
    r.forEach((a, i) => { if (a.page === c[i].page) { dx.push(Math.abs(a.x - c[i].x)); dy.push(Math.abs(a.y - c[i].y)) } })
  }
  const links = [...ref.links.values()].flat().length
  // C5: a beam's |Δ| per end, a slur's or tie's distance per control point.
  const ds = []; let paired = 0
  for (const [k, r] of ref.shapes ?? []) {
    const c = cand.shapes?.get(k); if (!c || c.length !== r.length) continue
    r.forEach((a, i) => {
      const b = c[i]
      if (a.sys !== b.sys || a.v.length !== b.v.length) return
      paired++
      a.v.forEach((p, j) => ds.push(typeof p === 'number' ? Math.abs(p - b.v[j]) : Math.hypot(p[0] - b.v[j][0], p[1] - b.v[j][1])))
    })
  }
  const shapes = [...ref.shapes?.values() ?? []].flat().length
  // C2b: every file of either capture, by name; one missing on either side is not equal.
  const named = [...new Set([...ref.midi.keys(), ...cand.midi.keys()])]
  const equal = (sortTies) => named.filter((f) => {
    const [r, c] = [ref.midi.get(f), cand.midi.get(f)].map((b) => b && smfEvents(b, { sortTies }))
    return r != null && r === c
  }).length
  const result = {
    pages: [ref.pages, cand.pages],
    events: same(ref.events, cand.events),
    midiNotes: same(ref.notes, cand.notes),
    midiBytes: [equal(false), equal(true), named.length],
    breaks: same([...ref.breaks], [...cand.breaks]),
    linksMatched: [dx.length, links],
    dxMedian: q(dx, 0.5), dxP95: q(dx, 0.95), dyMedian: q(dy, 0.5), dyP95: q(dy, 0.95),
    shapesMatched: [paired, shapes],
    shapeMedian: q(ds, 0.5), shapeP95: q(ds, 0.95),
  }
  // C4 has nothing to judge without links in the reference; a candidate that matches none of them fails it.
  const within = (median, p95) => median !== null && median <= MUST.median && p95 <= MUST.p95
  result.pass = {
    C1: result.events >= MUST.events,
    C2: result.midiNotes >= MUST.midiNotes,
    C3: ref.pages === cand.pages && result.breaks >= MUST.breaks,
    C4: links === 0 || (within(result.dxMedian, result.dxP95) && within(result.dyMedian, result.dyP95)),
    C5: shapeWithin(ref, shapes, result.shapeP95, MUST.shape),
  }
  result.goal = { C5: shapeWithin(ref, shapes, result.shapeP95, GOAL.shape), C2b: result.midiBytes[1] === named.length }
  return result
}

// C5 is not judged (null) without the reference's dump, passes a reference without shapes, and
// fails a candidate that pairs none of them.
const shapeWithin = (ref, shapes, p95, limit) => ref.shapes === null ? null : shapes === 0 || (p95 !== null && p95 <= limit)

const ID = /^[TM]\d\d$/
const ids = (dir) => readdirSync(dir).filter((f) => ID.test(f) && statSync(path.join(dir, f)).isDirectory()).sort()

/** Scores every ID of `refRoot` against the same ID in `candRoot`; a missing candidate fails every layer. */
export function compareAll(refRoot, candRoot) {
  return ids(refRoot).map((id) => {
    const cand = path.join(candRoot, id)
    if (!existsSync(cand)) return { id, missing: true, pass: { C1: false, C2: false, C3: false, C4: false, C5: false }, goal: { C5: false, C2b: false } }
    return { id, ...compare(path.join(refRoot, id), cand) }
  })
}

// What LilyPond warns while it processes a score, after parsing (cases 17, 18 and 30):
// --parse-only leaves these out (D64).
export const PROCESSING = new Set(['skipping zero-duration score', 'to suppress this, consider adding a spacer rest'])

/**
 * C0's view of a stderr: parseStderr's diagnostics, each file without its directory; with
 * `parseOnly`, without PROCESSING's warnings. `rootFile` receives the messages with no location.
 */
export function diagnostics(stderr, { rootFile = 'score.ly', parseOnly = false } = {}) {
  return parseStderr(stderr, { rootFile })
    .map((d) => ({ ...d, file: path.basename(d.file) }))
    .filter((d) => !(parseOnly && d.severity === 'warning' && PROCESSING.has(d.message)))
}

const diagnosticKey = (d) => d ? JSON.stringify([d.file, d.line, d.column ?? null, d.severity, d.message]) : null

/**
 * C0: scores the stderr `cand` against `ref`. It passes when the first diagnostics are equal
 * (or both absent) and the counts of errors and of warnings are; `all`, the Dice of every
 * diagnostic, is reported and not judged.
 */
export function compareStderr(ref, cand, options = {}) {
  const [r, c] = [diagnostics(ref, options), diagnostics(cand, options)]
  const count = (list, severity) => list.filter((d) => d.severity === severity).length
  const result = {
    first: [r[0] ?? null, c[0] ?? null],
    errors: [count(r, 'error'), count(c, 'error')],
    warnings: [count(r, 'warning'), count(c, 'warning')],
    all: same(r.map(diagnosticKey), c.map(diagnosticKey)),
  }
  result.pass = {
    C0: diagnosticKey(r[0]) === diagnosticKey(c[0]) && result.errors[0] === result.errors[1] && result.warnings[0] === result.warnings[1],
  }
  return result
}

/**
 * C0 on the error corpus: each case's .err (or those of `cases`) against `<candRoot>/<case>.err`,
 * the case's .ly receiving the messages with no location; a missing candidate fails.
 */
export function compareErrors(candRoot, { cases = errorCases(), parseOnly = false } = {}) {
  return cases.map((id) => {
    const cand = path.join(candRoot, `${id}.err`)
    if (!existsSync(cand)) return { id, missing: true, pass: { C0: false } }
    const read = (file) => readFileSync(file, 'utf8')
    return { id, ...compareStderr(read(path.join(ERRORS, `${id}.err`)), read(cand), { rootFile: `${id}.ly`, parseOnly }) }
  })
}

const round = (v, digits) => typeof v === 'number' ? +v.toFixed(digits) : v
export const format = (r) => JSON.stringify({
  ...r,
  events: round(r.events, 4), midiNotes: round(r.midiNotes, 4), breaks: round(r.breaks, 4),
  dxMedian: round(r.dxMedian, 3), dxP95: round(r.dxP95, 3), dyMedian: round(r.dyMedian, 3), dyP95: round(r.dyP95, 3),
  shapeMedian: round(r.shapeMedian, 3), shapeP95: round(r.shapeP95, 3),
})

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2)
  const parseOnly = args[0] === '--parse-only'
  if (parseOnly) args.shift()
  const [refDir, candDir] = args
  if (!refDir || !candDir) {
    console.error('usage: score.mjs <reference dir> <candidate dir>\n'
      + '       score.mjs [--parse-only] <reference stderr> <candidate stderr>\n'
      + '       score.mjs [--parse-only] --errors <candidate dir> [case…]')
    process.exit(2)
  }
  if (refDir === '--errors') {
    const results = compareErrors(candDir, { parseOnly, ...(args.length > 2 ? { cases: args.slice(2) } : {}) })
    for (const r of results) console.log(JSON.stringify(r))
    console.log(JSON.stringify({ cases: results.length, passing: { C0: results.filter((r) => r.pass.C0).length } }))
  } else if (statSync(refDir).isFile()) {
    console.log(JSON.stringify(compareStderr(readFileSync(refDir, 'utf8'), readFileSync(candDir, 'utf8'), { parseOnly })))
  } else if (ids(refDir).length) {
    const results = compareAll(refDir, candDir)
    for (const r of results) console.log(format(r))
    const passing = Object.fromEntries(['C1', 'C2', 'C3', 'C4', 'C5'].map((layer) => [layer, results.filter((r) => r.pass[layer]).length]))
    console.log(JSON.stringify({ scores: results.length, passing, goal: { C5: results.filter((r) => r.goal?.C5).length, C2b: results.filter((r) => r.goal?.C2b).length } }))
  } else {
    console.log(format(compare(refDir, candDir)))
  }
}
