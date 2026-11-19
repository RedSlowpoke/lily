// Translation traces for MS2 (04 Appendix A, §13): trace.ly's record of
// each \layout translation of a score, as LilyPond 2.26.0 writes it,
// captured into <corpus dir>/ref/<ID>/trace.tsv beside the MS0 capture
// (never committed), and compared with another trace of the same format
// (the engine's, from step 23 on). Part 3 captured its 14 scores; since
// step 35 all 30 are captured.
//
// The compare reads each pass (P line) on its own: the contexts made (C
// lines), in order, with their moments and parents; the contexts finalized
// (X), in order; the events each context heard per timestep (E), in order;
// and each context's own properties at each bar start (S), one property at
// a time. A property named in `later` (one an engraver of chapter 05 sets,
// MS3) is reported apart, by name, and does not fail the compare.
//
//   node tools/compare/trace.mjs [--out dir] [--corpus dir] [ID…]
//                                   captures the 30 scores, or these
//   node tools/compare/trace.mjs --compare <reference trace.tsv> <trace.tsv>
//                                   prints the comparison as JSON; exits 1
//                                   if the traces differ
//   LILYPOND   the lilypond to run (default the one on the PATH); it must be 2.26.0
import { spawn } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, rmSync } from 'node:fs'
import { availableParallelism } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { corpus, corpusDir } from '../corpus/fetch.mjs'
import { lilypondVersion } from './capture.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
export const TRACE = path.join(here, 'trace.ly')
/** Part 3's scores (progress.md, MS2 › The plan › Part 3). */
export const PART3 = ['T01', 'T03', 'T05', 'T06', 'T08', 'T13', 'T14', 'T15', 'T16', 'T17', 'T18', 'T19', 'T21', 'T29']
/** The 30 scores of corpus.tsv, T01–T30 (chapter 00 §4). */
export const SCORES = Array.from({ length: 30 }, (_, i) => `T${String(i + 1).padStart(2, '0')}`)
/**
 * The scores whose traces and playback maps the engine gives as LilyPond
 * does: part 3's 14, the grace notes' three since step 35 (N6), since step
 * 36 the four of \change whose music has no volta repeat, since step 37
 * (N7) the volta repeats' three, since step 38 T20, whose lyrics meet no
 * melisma, and since step 39 the other lyric scores but T23 and T28, whose
 * manual beams set beamMelismaBusy. T23 joined at MS4 step 10 (A26, D100):
 * since MS3 step 43 the engine translates its incipits as LilyPond does
 * when their names print (fixtures/incipits.ly), and since its ties are
 * ported its layout reaches them; it prints with emmentaler-16, which no
 * manifest preloads (trace.test.mjs gives it).
 */
export const TRACED = [...PART3, 'T09', 'T12', 'T22', 'T11', 'T25', 'T26', 'T27', 'T07', 'T10', 'T24', 'T20',
  'T02', 'T04', 'T28', 'T30', 'T23'].sort()
/**
 * The scores whose playback maps the engine gives as LilyPond does:
 * TRACED's (T23's since MS3 step 39, before its trace joined at MS4 step
 * 10).
 */
export const MAPPED = TRACED

/** Runs lilypond on `row` from its directory with trace.ly, writing `file`; resolves to its result. */
export function traceOne(lilypond, row, file) {
  rmSync(file, { force: true })
  mkdirSync(path.dirname(file), { recursive: true })
  const out = path.join(path.dirname(file), 'trace-out')
  mkdirSync(out, { recursive: true })
  const args = ['--loglevel=ERROR', '-dno-print-pages', `-dinclude-settings=${TRACE}`, '-o', path.join(out, 'trace'), row.path]
  const start = performance.now()
  return new Promise((resolve) => {
    const child = spawn(lilypond, args, { cwd: path.dirname(row.path), env: { ...process.env, LANGUAGE: 'en', TRACE_OUT: file } })
    const stderr = []
    child.stderr.on('data', (chunk) => stderr.push(chunk))
    child.on('error', (error) => stderr.push(Buffer.from(String(error))))
    child.on('close', (exit) => {
      rmSync(out, { recursive: true, force: true })
      const text = existsSync(file) ? readFileSync(file, 'utf8') : ''
      const passes = readTrace(text)
      resolve({
        id: row.id, exit, ms: Math.round(performance.now() - start), stderr: Buffer.concat(stderr).toString('utf8'),
        passes: passes.length, lines: text.split('\n').filter(Boolean).length,
      })
    })
  })
}

/**
 * Captures the traces of `ids` (default the 30) into `out`/<ID>/trace.tsv
 * (default <corpus dir>/ref), a few at a time, leaving the rest of the
 * directory as it is; resolves to each one's result, by ID. T03 exits 1, as
 * it must, after its lines are written.
 */
export async function captureTraces({ ids, out, corpus: dir, lilypond = process.env.LILYPOND ?? 'lilypond' } = {}) {
  const version = lilypondVersion(lilypond)
  if (version !== '2.26.0') throw new Error(`the reference is LilyPond 2.26.0; ${lilypond} is ${version ?? 'not installed'}`)
  const want = ids?.length ? ids : SCORES
  const rows = corpus(dir).filter((row) => want.includes(row.id))
  const unknown = want.filter((id) => !rows.some((row) => row.id === id))
  if (unknown.length) throw new Error(`not in corpus.tsv: ${unknown.join(', ')}`)
  const missing = rows.filter((row) => !existsSync(row.path))
  if (missing.length) throw new Error(`${missing.map((row) => row.id).join(', ')} not fetched; run tools/corpus/fetch.mjs`)
  out = path.resolve(out ?? path.join(corpusDir(dir), 'ref'))
  const results = []
  let next = 0
  const worker = async () => {
    while (next < rows.length) {
      const row = rows[next++]
      results.push(await traceOne(lilypond, row, path.join(out, row.id, 'trace.tsv')))
    }
  }
  await Promise.all(Array.from({ length: Math.min(rows.length, Math.max(1, availableParallelism() >> 1)) }, worker))
  return results.sort((a, b) => a.id.localeCompare(b.id))
}

/** A map's list at `key`, made empty if it has none. */
function at(map, key, make = () => []) {
  if (!map.has(key)) map.set(key, make())
  return map.get(key)
}

/**
 * A trace's passes, each `{ kind, contexts, removed, events, states }`:
 * `contexts` the C lines as `moment context parent`, `removed` the X lines
 * as `moment context`, `events` per `moment|context` the E lines' class and
 * arguments, and `states` per `moment|context` a map of property to value.
 * A moment is `main grace`. Lines before the first P are pass 0's.
 */
export function readTrace(text) {
  const passes = []
  let pass = null
  const start = (kind) => {
    pass = { kind, contexts: [], removed: [], events: new Map(), states: new Map() }
    passes.push(pass)
  }
  for (const line of text.split('\n')) {
    const f = line.split('\t')
    if (f[0] === 'P') {
      start(f[2])
      continue
    }
    if (!'CXES'.includes(f[0]) || f.length < 4 || f[0] === '') continue
    if (!pass) start('')
    const moment = `${f[1]} ${f[2]}`
    if (f[0] === 'C') pass.contexts.push(`${moment} ${f[3]} ${f[4]}`)
    else if (f[0] === 'X') pass.removed.push(`${moment} ${f[3]}`)
    else if (f[0] === 'E') at(pass.events, `${moment}|${f[3]}`).push(f.slice(4).join('\t'))
    else at(pass.states, `${moment}|${f[3]}`, () => new Map()).set(f[4], f.slice(5).join('\t'))
  }
  return passes
}

/** The differences of two lists, in order: each place where they differ. */
function listDiffer(want, have) {
  const differ = []
  for (let i = 0; i < Math.max(want.length, have.length); i++) {
    if (want[i] !== have[i]) differ.push({ index: i, expected: want[i] ?? null, got: have[i] ?? null })
  }
  return differ
}

/**
 * Compares the trace `got` with `reference`, both texts: whether they are
 * equal, how many passes, contexts, events and properties each has, each
 * difference (`{ pass, what, at, context, property?, expected, got }`, `what`
 * one of contexts, removed, events, states), and `later`, the properties
 * named in `later` that differ, which do not make the traces unequal.
 */
export function compareTrace(reference, got, { later = [] } = {}) {
  const [a, b] = [readTrace(reference), readTrace(got)]
  const differ = []
  const deferred = new Set()
  const count = (passes) => ({
    passes: passes.length,
    contexts: passes.reduce((n, p) => n + p.contexts.length, 0),
    events: passes.reduce((n, p) => n + [...p.events.values()].reduce((m, l) => m + l.length, 0), 0),
    states: passes.reduce((n, p) => n + [...p.states.values()].reduce((m, s) => m + s.size, 0), 0),
  })
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    const [x, y] = [a[i], b[i]]
    if (!x || !y || x.kind !== y.kind) {
      differ.push({ pass: i + 1, what: 'pass', expected: x?.kind ?? null, got: y?.kind ?? null })
      continue
    }
    for (const what of ['contexts', 'removed']) {
      for (const d of listDiffer(x[what], y[what])) differ.push({ pass: i + 1, what, ...d })
    }
    const keys = (m, n) => [...new Set([...m.keys(), ...n.keys()])].sort(byStep)
    for (const key of keys(x.events, y.events)) {
      const [moment, context] = key.split('|')
      for (const d of listDiffer(x.events.get(key) ?? [], y.events.get(key) ?? [])) {
        differ.push({ pass: i + 1, what: 'events', at: moment, context, ...d })
      }
    }
    for (const key of keys(x.states, y.states)) {
      const [moment, context] = key.split('|')
      const [s, t] = [x.states.get(key) ?? new Map(), y.states.get(key) ?? new Map()]
      for (const property of [...new Set([...s.keys(), ...t.keys()])].sort()) {
        if (s.get(property) === t.get(property)) continue
        if (later.includes(property)) deferred.add(property)
        else differ.push({ pass: i + 1, what: 'states', at: moment, context, property, expected: s.get(property) ?? null, got: t.get(property) ?? null })
      }
    }
  }
  return { equal: differ.length === 0, counts: [count(a), count(b)], differ, later: [...deferred].sort() }
}

/** Orders `main grace|context` keys by moment, then context. */
function byStep(p, q) {
  const frac = (s) => {
    const [n, d] = s.split('/')
    return d ? Number(n) / Number(d) : Number(n)
  }
  const [[m, c], [n, e]] = [p, q].map((k) => k.split('|'))
  const [[m1, m2], [n1, n2]] = [m, n].map((s) => s.split(' ').map(frac))
  return m1 - n1 || m2 - n2 || c.localeCompare(e)
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2)
  if (argv[0] === '--compare') {
    if (argv.length !== 3) {
      console.error('usage: trace.mjs --compare <reference trace.tsv> <trace.tsv>')
      process.exit(2)
    }
    const result = compareTrace(...argv.slice(1).map((f) => readFileSync(f, 'utf8')))
    console.log(JSON.stringify({ ...result, differ: result.differ.slice(0, 20), differences: result.differ.length }, null, 1))
    process.exit(result.equal ? 0 : 1)
  }
  const options = { ids: [] }
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i].match(/^--(out|corpus)$/)?.[1]
    if (flag) options[flag] = argv[++i]
    else options.ids.push(argv[i])
  }
  try {
    const results = await captureTraces(options)
    console.log('ID\texit\tms\tpasses\tlines')
    for (const r of results) console.log(`${r.id}\t${r.exit}\t${r.ms}\t${r.passes}\t${r.lines}`)
  } catch (error) {
    console.error(error.message)
    process.exit(1)
  }
}
