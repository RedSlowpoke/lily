// MS6's differential harness (README §5, MS6; 00 §6.2, 05 §6.3): an
// incremental compile, in an engine that has compiled the text before the
// edit, against one from scratch of the edited text, compared layer by layer
// C0–C5. Until a layer is incremental both are from scratch, and the two
// must be equal byte for byte in every output. A layer's outputs are
// compared by their bytes, and scored with the comparator's own modules
// (score.mjs) to say how they differ.
//
//   node tools/compare/differential.mjs [--seed <n>] [--count <k>] [--wasm] [--typing]
//                          [--steps <ms> <stop>] [--stop-moved] [--keep <dir>] [<id>…]
//                          E1–E3 on the four and <k> seeded edits (10) of each
//                          score (the 30), natively, or in core.wasm with --wasm;
//                          one JSON line an edit, then a count, exit 1 on a difference;
//                          --steps: in steps of <ms>, each first stopped after <stop>
//                          steps by its place, and with --stop-moved by a compile of
//                          text moved from it (STEPS in its test; untraced, D261)
//
//   C0 diagnostics  stderr and diagnostics.json; compareStderr
//   C1 events       the playback map and the traced translation (trace.tsv,
//                   trace.log, grobs.tsv); the Dice of the events
//   C2 MIDI         every .midi; the Dice of the notes and the files' bytes
//   C3 breaks       the pages' names (their count) and where the systems
//                   break; the pages and the Dice of the break moments
//   C4 positions    the pages and the laid-out grobs (layout.tsv, columns.tsv,
//                   line.tsv); the links paired and |Δx|, |Δy|
//   C5 shapes       c5.tsv; the shapes paired and their distances
import { spawn } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { example } from '../examples.mjs'
import { compileInWasm } from './core.mjs'
import { EDITED, THIRTY, e1e3, seeded, typing } from './edits.mjs'
import { compare, compareStderr } from './score.mjs'

export const LAYERS = ['C0', 'C1', 'C2', 'C3', 'C4', 'C5']

/** The preview's own page code, which replaces a page's cells on screen (W8). */
const { replaceCells } = createRequire(import.meta.url)('@lily/common/web/preview.js')

/** The traced outputs, which the compile example writes as `<stem>.<name>`. */
const TRACED = ['trace.tsv', 'trace.log', 'grobs.tsv', 'layout.tsv', 'columns.tsv', 'line.tsv', 'c5.tsv']

/** The layer whose bytes an output is: one of LAYERS, or null for a file the score's Scheme wrote. */
export function layerOf(name) {
  if (name === 'stderr' || name === 'diagnostics.json') return 'C0'
  if (name.endsWith('.timing.json') || ['trace.tsv', 'trace.log', 'grobs.tsv'].includes(name)) return 'C1'
  if (/\.midi?$/.test(name)) return 'C2'
  if (name.endsWith('.svg') || ['layout.tsv', 'columns.tsv', 'line.tsv'].includes(name)) return 'C4'
  if (name === 'c5.tsv') return 'C5'
  return null
}

/** The outputs the compile example wrote for `stem` into `dir`, by the names the core gives them. */
export function readOutputs(dir, stem) {
  const outputs = {}
  for (const f of readdirSync(dir)) {
    if (f === `${stem}.skylines.tsv`) continue
    const name = f === `${stem}.err` ? 'stderr'
      : f === `${stem}.json` ? 'diagnostics.json'
        : TRACED.find((t) => f === `${stem}.${t}`) ?? f
    outputs[name] = readFileSync(path.join(dir, f))
  }
  return outputs
}

/**
 * How many columns' skylines in `a` and `b`, two directories' `skylines.tsv`,
 * differ. No layer's: a column off from scratch need not move a page, but
 * could move a rod (OPEN B27). `differential.test.mjs` gates on none (D268).
 */
export function skylinesOff(a, b, stem) {
  const rows = (dir) => {
    const file = path.join(dir, `${stem}.skylines.tsv`)
    return existsSync(file) ? readFileSync(file, 'utf8').split('\n') : []
  }
  const [x, y] = [rows(a), rows(b)]
  return x.filter((row, i) => row !== y[i]).length + Math.max(0, y.length - x.length)
}

/** Where two texts first differ: the line, and each side's line. */
export function firstDifference(a, b) {
  const [x, y] = [a, b].map((t) => (t ? t.toString('utf8') : '').split('\n'))
  const i = x.findIndex((line, i) => line !== y[i])
  const at = i < 0 ? x.length : i
  const cut = (s) => (s === undefined ? '(none)' : s.length > 120 ? `${s.slice(0, 120)}…` : s)
  return `line ${at + 1}: ${cut(x[at])} | ${cut(y[at])}`
}

/** `outputs` alone in a directory of its own under `root`, as a capture, for score.mjs. */
function asCapture(root, name, outputs) {
  const dir = path.join(root, name)
  mkdirSync(dir, { recursive: true })
  for (const [f, bytes] of Object.entries(outputs)) if (!f.includes('/')) writeFileSync(path.join(dir, f), bytes)
  return dir
}

/**
 * The incremental compile's outputs against the fresh one's, by layer: each
 * of LAYERS `{ equal, files, score }`, `files` what differs in it (an output
 * missing on one side, or its bytes and where), `score` the comparator's
 * figures for it; and `equal`, every layer equal and no other output differing.
 * `rootFile` receives the diagnostics with no location.
 */
export function differ(fresh, incremental, { rootFile = 'score.ly' } = {}) {
  const result = Object.fromEntries(LAYERS.map((l) => [l, { equal: true, files: [], score: {} }]))
  const other = []
  for (const name of [...new Set([...Object.keys(fresh), ...Object.keys(incremental)])].sort()) {
    const [a, b] = [fresh[name], incremental[name]]
    if (name === 'skylines.tsv' || (a && b && Buffer.compare(a, b) === 0)) continue
    const why = !a ? `${name}: only in the incremental compile` : !b ? `${name}: only in the fresh compile` : `${name}: ${firstDifference(a, b)}`
    const layer = layerOf(name)
    if (layer) result[layer].files.push(why)
    else other.push(why)
  }
  // C3 sees the pages' names: a page added or dropped, or one renamed.
  const pages = (outputs) => Object.keys(outputs).filter((f) => f.endsWith('.svg')).sort().join(' ')
  if (pages(fresh) !== pages(incremental)) result.C3.files.push(`pages: ${pages(fresh)} | ${pages(incremental)}`)
  const root = mkdtempSync(path.join(tmpdir(), 'lily-differential-'))
  try {
    const c0 = compareStderr(fresh.stderr?.toString('utf8') ?? '', incremental.stderr?.toString('utf8') ?? '', { rootFile })
    const c = compare(asCapture(root, 'fresh', fresh), asCapture(root, 'incremental', incremental))
    result.C0.score = { first: c0.first, errors: c0.errors, warnings: c0.warnings, all: c0.all }
    result.C1.score = { events: c.events }
    result.C2.score = { midiNotes: c.midiNotes, midiBytes: c.midiBytes }
    result.C3.score = { pages: c.pages, breaks: c.breaks }
    result.C4.score = { linksMatched: c.linksMatched, dxP95: c.dxP95, dyP95: c.dyP95 }
    result.C5.score = { shapesMatched: c.shapesMatched, shapeP95: c.shapeP95 }
    // What the comparator sees beyond the bytes: a layer it scores below
    // equal differs even if its files were not compared (none so far).
    const sees = {
      C0: !c0.pass.C0 || c0.all < 1,
      C1: c.events < 1,
      C2: c.midiNotes < 1 || c.midiBytes[0] !== c.midiBytes[2],
      C3: c.pages[0] !== c.pages[1] || c.breaks < 1,
      C4: c.linksMatched[0] !== c.linksMatched[1] || c.dxP95 > 0 || c.dyP95 > 0,
      C5: c.shapesMatched[0] !== c.shapesMatched[1] || c.shapeP95 > 0,
    }
    for (const l of LAYERS) {
      if (sees[l] && !result[l].files.length) result[l].files.push(`scored: ${JSON.stringify(result[l].score)}`)
      result[l].equal = !result[l].files.length
    }
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
  return { ...result, other, equal: LAYERS.every((l) => result[l].equal) && !other.length }
}

/**
 * The compile example's `<stem>.preview.tsv` (MS6 step 10): what the writer
 * did of each page of an incremental compile for a preview, and what it keeps.
 */
export function previewOf(text = '') {
  return text.split('\n').filter(Boolean).map((line) => {
    const [page, ...f] = line.split('\t')
    const [cells, sent, replaced, sentBytes, pageBytes, entries, taken, kept] = f.map(Number)
    return { page, cells, sent, replaced: replaced === 1, sentBytes, pageBytes, entries, taken, kept }
  })
}

/**
 * Takes from an incremental compile's `outputs` what a preview adds, and
 * replaces the cells of each page sent by them as the preview does: `sent`,
 * how many, and `wrong`, each that is not `fresh`'s page byte for byte
 * (`<page>.before` the page it replaces cells of).
 */
export function replacedPages(outputs, fresh) {
  const pages = Object.keys(outputs).filter((name) => name.endsWith('.cells')).map((name) => name.slice(0, -'.cells'.length))
  const wrong = []
  for (const page of pages) {
    const file = outputs[`${page}.cells`].toString('utf8')
    const line = file.indexOf('\n')
    const before = outputs[`${page}.before`].toString('utf8')
    const cells = file.slice(0, line).split(' ').map((cell) => ({ digest: cell.split(':')[0], y: cell.split(':')[1] }))
    const replaced = replaceCells(before, cells, file.slice(line + 1))
    if (replaced === undefined) wrong.push(`${page}: a digest not on the page before it`)
    else if (Buffer.compare(Buffer.from(replaced), fresh[`${page}.svg`] ?? Buffer.alloc(0)) !== 0) {
      wrong.push(`${page}: ${firstDifference(fresh[`${page}.svg`], replaced)}`)
    }
    delete outputs[`${page}.cells`]
    delete outputs[`${page}.before`]
  }
  return { sent: pages.length, wrong }
}

/** The layers of a `differ` result that differ, with what differs in each. */
export const differences = (r) => Object.fromEntries([...LAYERS.filter((l) => !r[l].equal).map((l) => [l, r[l].files]),
  ...(r.other.length ? [['other', r.other]] : [])])

/** `edits` written to `dir`, one file each: `<id>-<name>.ly`. */
function sources(dir, edits) {
  mkdirSync(dir, { recursive: true })
  return edits.map((e) => {
    const file = path.join(dir, `${e.id}-${e.name}.ly`)
    writeFileSync(file, e.source)
    return file
  })
}

/** The compile example (release) run with `args`; resolves when it exits with 0 or 1 (a file failed). */
function compile(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(example('compile', true), args, { stdio: ['ignore', 'ignore', 'pipe'] })
    let stderr = ''
    child.stderr.on('data', (data) => { stderr += data })
    child.on('error', reject)
    child.on('close', (status, signal) => (status === 0 || status === 1 ? resolve() : reject(new Error(`compile ${args.join(' ')}: ${status ?? signal}\n${stderr}`))))
  })
}

/**
 * The compile example's `--n8` file (MS6 step 4): each translation's
 * `Engine::changes` (m_e, the checkpoints it resumes from and converges at,
 * its timesteps, those run again, the changed columns), and what its whole
 * runs contradicted.
 */
export function n8Of(text = '') {
  const changes = []
  const parts = []
  const wrong = []
  for (const line of text.split('\n').filter(Boolean)) {
    const f = line.split('\t')
    if (f[0] === 'wrong') wrong.push(f.slice(1).join('\t'))
    else if (f[0] === 'parts') parts.push({ weights: f[2].split(',').map(Number), mergedAll: f[3] === '-' ? null : f[3], keys: f.slice(4).filter(Boolean) })
    else {
      // With partitions (MS6 step 6): the timesteps each partition runs, and
      // the translators times timesteps run, without partitions, and whole;
      // and were a partition's writes outside itself replayed (step 7a).
      const [runs, work, unpartitioned, whole, readsOnly] = (f[7] ?? '-').split(' ')
      const partitioned = f[7] && f[7] !== '-'
        ? { runs: runs ? runs.split(',').map(Number) : [], work: Number(work), unpartitioned: Number(unpartitioned), whole: Number(whole), readsOnly: Number(readsOnly) }
        : null
      // The snapshot it was resumed from (MS6 step 5's rest, D241).
      const resumed = f[8] && f[8] !== '-' ? Number(f[8]) : null
      // Whether it then converged and ended as the run before did (D243).
      const spliced = f[9] === 'spliced'
      // What it replayed: its attempts, the partitions' translator calls
      // made and those made again from the old run's log (MS6 step 7c).
      const [attempts, ran, replayed] = (f[10] ?? '0 0 0').split(' ').map(Number)
      changes.push({ changed: f[1], resume: f[2], converged: f[3], steps: Number(f[4]), run: Number(f[5]), columns: f[6] ? f[6].split(' ') : [], partitioned, resumed, spliced, attempts, ran, replayed })
    }
  }
  return { changes, parts, wrong }
}

/**
 * The compile example's `<stem>.cells.tsv` (MS6 step 8): what each `\layout`
 * translation's layout cells did, and what their callbacks read that the
 * cells' keys do not cover (`--check-cells`).
 */
export function cellsOf(text = '') {
  const reports = []
  const uncovered = []
  for (const line of text.split('\n').filter(Boolean)) {
    const [kind, ...f] = line.split('\t')
    if (kind === 'uncovered') uncovered.push(f.join('\t'))
    else {
      // Then line breaking's lines (MS6 step 9): those taken from the last
      // compile, and those `--check-cells` filled otherwise than taken; then
      // what came before the line breaks (D250): the lines given back, their
      // grobs, those kept, and the columns' skylines taken; then (D260) those
      // of them made again as the lines moved; then (D268) the lines not kept
      // as a grob of theirs was read in two places by the columns, and the
      // grobs kept with where they were first read.
      const [cells, reused, grobs, grobsReused, kept, noKey, noRange, impure, readsY, coupled, lines, linesReused, linesWrong,
        before, beforeReused, beforeGrobs, beforeGrobsReused, beforeKept, skylines, skylinesReused, skylinesMoved, beforeMoved = 0, beforePlaces = 0] = f.map(Number)
      reports.push({ cells, reused, grobs, grobsReused, kept, noKey, noRange, impure, readsY, coupled, lines, linesReused, linesWrong,
        before, beforeReused, beforeGrobs, beforeGrobsReused, beforeKept, skylines, skylinesReused, skylinesMoved, beforeMoved, beforePlaces })
    }
  }
  return { reports, uncovered }
}

/**
 * Each of `edits` (edits.mjs') compiled natively by the compile example in
 * release, incrementally and from scratch at once, traced, in `dir` (each
 * edit's source kept there); resolves to each edit with `differ`'s result.
 * Each run compiles all of them in one process: the incremental ones in one
 * engine, which compiles each edit's text before it first, and checks N8's
 * digests (`--n8`): each edit's `n8` is `n8Of`'s. With `check` false, no
 * `--n8`: the check restores each translation and runs it on, so layout
 * then lays out that run, not what a splice or a replay made (D243); without
 * it, as in wasm, layout lays out what the incremental compile made. `flags`
 * go to the incremental compile (`--replay-all`). With `chain` (`typing`'s
 * edits, MS7 step 1) an edit of the file of the edit before it goes on from
 * that one. With `preview` both compile
 * for a preview (MS6 step 10): their pages are the preview form, each page
 * sent by its cells is checked by `replacedPages`, as each edit's `replaced`,
 * and `pages` is `previewOf`'s. With `steps` (MS6 step 11), flags that make the
 * incremental compile return between its steps, untraced, as a traced compile
 * is one call: C1–C5 are then its pages, its MIDI files and its playback map.
 */
export async function nativeDifferential(edits, dir, { check = true, flags = [], preview = false, steps, chain = false } = {}) {
  const edited = sources(path.join(dir, 'sources'), edits)
  const run = async (name, given) => {
    const out = path.join(dir, name)
    await compile([...(steps ? [] : ['--trace']), ...given, '--out', out, ...edited.flatMap((f) => ['--edited', f]), ...edits.map((e) => e.file)])
    return out
  }
  const form = preview ? ['--preview'] : []
  const [fresh, incremental] = await Promise.all([run('fresh', ['--fresh', ...form]), run('incremental', [...(check ? ['--n8'] : []), ...(chain ? ['--chain'] : []), ...flags, ...form, ...(steps ?? [])])])
  return edits.map((e, n) => {
    const stem = path.basename(e.file, '.ly')
    const at = (root) => readOutputs(path.join(root, String(n + 1)), stem)
    const outputs = at(incremental)
    const n8 = n8Of(outputs[`${stem}.n8.tsv`]?.toString('utf8'))
    delete outputs[`${stem}.n8.tsv`]
    const cells = cellsOf(outputs[`${stem}.cells.tsv`]?.toString('utf8'))
    delete outputs[`${stem}.cells.tsv`]
    const pages = previewOf(outputs[`${stem}.preview.tsv`]?.toString('utf8'))
    delete outputs[`${stem}.preview.tsv`]
    const whole = at(fresh)
    const replaced = replacedPages(outputs, whole)
    const off = skylinesOff(path.join(fresh, String(n + 1)), path.join(incremental, String(n + 1)), stem)
    return { ...e, source: edited[n], result: differ(whole, outputs, { rootFile: e.file }), n8, cells, pages, replaced, skylinesOff: off }
  })
}

/**
 * As nativeDifferential, in core.wasm: the incremental compiles in one
 * instance, the fresh each in its own. With `preview` each compile is
 * performed after it (D264), so that its MIDI files and map are compared and
 * the incremental compiles resume across the performances between them.
 */
export async function wasmDifferential(edits, dir, { preview = false, chain = false } = {}) {
  const edited = sources(path.join(dir, 'sources'), edits)
  const goesOn = (n) => chain && edits[n - 1]?.file === edits[n].file
  const entries = edits.map((e, n) => ({ file: e.file, text: Buffer.from(e.source), before: goesOn(n) ? undefined : readFileSync(e.file) }))
  // A compile for a preview leaves its `\midi`s to its performance (D264): both perform after each compile.
  const fresh = await compileInWasm(entries, { trace: true, fresh: true, preview, perform: preview })
  const incremental = await compileInWasm(entries, { trace: true, preview, perform: preview })
  return edits.map((e, n) => {
    const replaced = replacedPages(incremental[n].outputs, fresh[n].outputs)
    return { ...e, source: edited[n], result: differ(fresh[n].outputs, incremental[n].outputs, { rootFile: e.file }), replaced }
  })
}

/** E1–E3 of those of `ids` that EDITED names, then `count` seeded edits of each of `ids`. */
export function editsOf(ids, { seed = 1, count = 10 } = {}) {
  return [...ids.filter((id) => EDITED.includes(id)).flatMap(e1e3), ...ids.flatMap((id) => seeded(id, { seed, count }))]
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2)
  const option = (name, fallback) => {
    const i = args.indexOf(name)
    return i < 0 ? fallback : args.splice(i, 2)[1]
  }
  const seed = +option('--seed', 1)
  const count = +option('--count', 10)
  const keep = option('--keep')
  const wasm = args.includes('--wasm')
  const stepsAt = args.indexOf('--steps')
  const ids = args.filter((a, i) => !a.startsWith('--') && !(stepsAt >= 0 && i > stepsAt && i <= stepsAt + 2))
  const dir = keep ?? mkdtempSync(path.join(tmpdir(), 'lily-differential-run-'))
  const chain = args.includes('--typing')
  const edits = chain ? (ids.length ? ids : THIRTY).flatMap((id) => typing(id, { count })) : editsOf(ids.length ? ids : THIRTY, { seed, count })
  const check = !args.includes('--no-check')
  const flags = args.includes('--check-cells') ? ['--check-cells'] : []
  const preview = args.includes('--preview')
  const at = args.indexOf('--steps')
  const steps = at < 0 ? undefined : ['--step-ms', args[at + 1], '--stop-at', args[at + 2], ...(args.includes('--stop-moved') ? ['--stop-moved'] : [])]
  const results = await (wasm ? wasmDifferential(edits, dir, { preview, chain }) : nativeDifferential(edits, dir, { check: check && !steps, flags, preview, chain, steps }))
  if (flags.length) {
    const seen = [...new Set(results.flatMap((r) => r.cells?.uncovered ?? []))].sort()
    for (const u of seen) console.log(`uncovered\t${u}`)
  }
  for (const { source, result, id, name, kind, line, char, skylinesOff: off } of results) {
    console.log(JSON.stringify({ id, name, kind, line, char, equal: result.equal, skylinesOff: off || undefined, source: keep ? source : undefined, ...differences(result) }))
  }
  const unequal = results.filter((r) => !r.result.equal || r.replaced?.wrong.length).length
  const reports = results.flatMap((r) => r.cells?.reports ?? [])
  const sum = (k) => reports.reduce((n, r) => n + r[k], 0)
  console.log(JSON.stringify({ edits: results.length, seed, count, equal: results.length - unequal,
    layers: Object.fromEntries(LAYERS.map((l) => [l, results.filter((r) => r.result[l].equal).length])),
    cells: Object.fromEntries(['cells', 'reused', 'grobs', 'grobsReused', 'kept', 'noKey', 'noRange', 'impure', 'readsY', 'coupled', 'lines', 'linesReused', 'linesWrong', 'before', 'beforeReused', 'beforeGrobs', 'beforeGrobsReused', 'skylines', 'skylinesReused', 'skylinesMoved', 'beforeMoved', 'beforePlaces'].map((k) => [k, sum(k)])),
    skylinesOff: results.reduce((n, r) => n + (r.skylinesOff ?? 0), 0),
    replaced: { sent: results.reduce((n, r) => n + (r.replaced?.sent ?? 0), 0), wrong: results.flatMap((r) => (r.replaced?.wrong ?? []).map((w) => `${r.id} ${r.name} ${w}`)) } }))
  if (!keep) rmSync(dir, { recursive: true, force: true })
  process.exitCode = unequal ? 1 : 0
}
