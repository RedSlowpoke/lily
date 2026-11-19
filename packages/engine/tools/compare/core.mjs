// The built core.wasm driven in Node as the studio's driver drives it
// (packages/studio/src/web/engine/driver.ts), for the tests that compare its
// outputs with LilyPond's (MS2 step 27): one instance started from
// core.image, each file put under its path and compiled through lily_put,
// lily_compile, lily_step and lily_output, what the engine asks for
// (lily_needs) answered from disk as crates/core/examples/compile.rs answers
// (a name under <datadir>/ from the installed LilyPond's data directory), and
// each compile's times read with lsb_times. `modules`, a font pack's name
// and bytes each, are put first as the driver puts the modules it loaded
// (lily_put's kind 1, MS3 step 5). Each name is answered as it is
// asked and the compile started again, as the driver does; the names one stop
// asked for, a part's files all at once (MS2 step 30), come one a step with no
// parse between them, which lsb_parses counts. lsb_trace turns the trace on
// (Engine::set_trace): trace.tsv, trace.log, since MS3 step 12 grobs.tsv
// since step 25 layout.tsv and since step 31 columns.tsv among a compile's
// outputs. Since step 35 a \layout compile's book is among them as the
// core's pages, <base>.svg or <base>-<n>.svg (protocol.ts' names), after
// stderr and diagnostics.json and before the playback map, as the compile
// example writes them with --out (pages.test.mjs). Since step 50 (W5, D208) a font
// pack the engine asks for (lily_needs' MODULE) is answered as the driver loads it,
// with the file the build's manifest names, and put as none, as a host that cannot
// fetch it answers, when the manifest names none or `offline`.
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import path from 'node:path'
import { benchDir, coreWasm, instantiateCore, lilyScm } from '../bench/run.mjs'

const engine = path.resolve(benchDir, '../..')
const coreImage = path.join(engine, 'dist/engine/core.image')
const manifest = path.join(engine, 'dist/engine/manifest.json')
const sources = ['crates/core/src', 'crates/scheme/src', 'crates/music/src', 'crates/layout/src', 'crates/parser/src', 'crates/svg/src']
  .map((d) => path.join(engine, d))

/** Why core.wasm cannot be compared: no LilyPond 2.26.0, or no build as new as the sources; else false. */
export function coreSkip() {
  if (!lilyScm()) return 'LilyPond 2.26.0 is not installed'
  const newest = Math.max(...sources.flatMap((dir) => readdirSync(dir).map((f) => statSync(path.join(dir, f)).mtimeMs)))
  return [coreWasm, coreImage].map((file) => !existsSync(file)
    ? `no dist/engine/${path.basename(file)}: run npm run build`
    : statSync(file).mtimeMs < newest ? `dist/engine/${path.basename(file)} is older than the sources: run npm run build` : false)
    .find(Boolean) ?? false
}

/**
 * The font packs the build's manifest preloads (D105), `[name, file]` each,
 * which the studio's host puts before it compiles; none without a build.
 */
export function preloaded() {
  if (!existsSync(manifest)) return []
  const { core, modules } = JSON.parse(readFileSync(manifest, 'utf8'))
  return core.preload.map((name) => [name, path.join(path.dirname(manifest), modules[name].file)])
}

/**
 * Every text face's outlines pack the build's manifest names (D262), `[name,
 * file]` each: a test of something else gives them first, as it gives the
 * preloaded packs, so that a skyline's ask for them is no stop of its own.
 */
export function outlinePacks() {
  if (!existsSync(manifest)) return []
  const { modules } = JSON.parse(readFileSync(manifest, 'utf8'))
  return Object.keys(modules).filter((name) => name.endsWith('.outlines')).map((name) => [name, path.join(path.dirname(manifest), modules[name].file)])
}

/** The preloaded packs with their bytes, `[name, bytes]` each, as a host puts them after lily_start. */
export function preloadedModules() {
  return preloaded().map(([name, file]) => [name, readFileSync(file)])
}

/** The file of the module `name` the build's manifest names (W5's packs among them), or undefined. */
export function moduleFile(name) {
  if (!existsSync(manifest)) return undefined
  const file = JSON.parse(readFileSync(manifest, 'utf8')).modules[name]?.file
  return file && path.join(path.dirname(manifest), file)
}

// protocol.ts' Step and Need.
const DONE = 0
const MORE = 1
const FILE = 0
const MODULE = 1
/** How the core names LilyPond's own files. */
const DATADIR = '<datadir>'
/** lsb_times' values, in order (crates/core Times::values). */
const TIMES = ['parseMs', 'translateMs', 'translations', 'longestMs', 'layOutMs', 'stencilsMs', 'writeMs', 'liveGrobs', 'performMs', 'midiMs',
  'translatedBytes', 'pagedBytes']

/**
 * Compiles `files` one after another in one fresh instance of core.wasm,
 * traced if `trace`; resolves to each one's `{ file, outputs, asked, stops,
 * parses, ms, times, packs }`: its outputs by name, the names the engine asked for
 * and at how many stops, how many times it parsed the file, the
 * milliseconds from lily_compile to done (the answers included),
 * lsb_times', and the packs it asked for, each `[name, fetched]`. `modules`
 * ([name, bytes] each, by default the manifest's preloaded packs) are put
 * after lily_start; with `offline` no pack it asks for is fetched. With
 * `breaks` (columns' whens, `main grace`) its lines break there
 * (lsb_breaks, MS3 step 60), until the engine chooses its breaks.
 *
 * A file may be `{ file, text, before }` (MS6's differential harness): `text`
 * put under the name `file` in place of the file's bytes and, with `before`,
 * that text compiled first in the same instance and its outputs dropped, the
 * incremental compile, as the compile example's `--edited`. With `fresh`
 * each file is compiled in an instance of its own, and `before` is not
 * compiled: the compile from scratch the incremental one must equal. With
 * `preview` a compile has a visible page, as the driver's, its pages the
 * preview's (MS6 step 10), and beside each `<page>.cells` output comes
 * `<page>.before`, that page as `before` compiled it. A compile for a preview
 * leaves its `\midi`s out (D264); with `perform` each compile, `before`'s too,
 * is performed after it (lily_perform), and the performance's MIDI files and
 * map join its outputs, `performMs` the milliseconds it took. With `pdf` each
 * compile gives each book's PDF display list too, `<book>.pdf.list`
 * (lily_pdf, MS8 step 6, D271), as Export PDF's.
 */
export async function compileInWasm(files, { trace = false, modules = preloadedModules(), offline = false, breaks = [], fresh = false, preview = false, perform = false, pdf = false } = {}) {
  const wasm = await WebAssembly.compile(readFileSync(coreWasm))
  const datadir = path.resolve(lilyScm(), '../..')
  let e, out
  const put = (bytes) => {
    const at = e.lily_alloc(bytes.length)
    new Uint8Array(e.memory.buffer, at, bytes.length).set(bytes)
    return [at, bytes.length]
  }
  const visible = preview ? 1 : 0
  const words = (at, n) => [...new Uint32Array(e.memory.buffer, at, n)]
  const bytes = (at, length) => Buffer.from(e.memory.buffer, at, length)
  const instance = async () => {
    e = (await instantiateCore(wasm)).exports
    if (e.lily_start(...put(readFileSync(coreImage))) !== 0) throw new Error('lily_start refused core.image')
    e.lsb_trace(trace ? 1 : 0)
    if (pdf) e.lily_pdf(1)
    if (breaks.length) e.lsb_breaks(...put(Buffer.from(breaks.join('\n'))))
    for (const [name, bytes] of modules) e.lily_put(MODULE, ...put(Buffer.from(name)), ...put(bytes))
    out = e.lily_alloc(16)
  }
  /**
   * Compiles `file` as `revision` until done, or with `performing` performs
   * that compile (D264), each name asked answered; what it asked and the packs.
   */
  const run = (revision, file, performing = false) => {
    const asked = []
    const packs = []
    // The parses made by the last name asked: a name asked after another
    // parse is a stop of its own.
    let stops = 0
    let parsed = 0
    const begin = () => performing ? e.lily_perform(revision, ...put(Buffer.from(file))) : e.lily_compile(revision, ...put(Buffer.from(file)), visible)
    begin()
    const started = performance.now()
    // The visible page a preview's compile hands over before its end (OPEN I7, D269).
    let handed
    let step
    while ((step = e.lily_step(10)) !== DONE) {
      if (step === MORE) {
        if (e.lily_outputs() > 0) {
          if (handed) throw new Error(`${file}: a page handed over twice`)
          handed = { ms: performance.now() - started, files: outputs() }
        }
        continue
      }
      // A compile stopped for the host after its page starts again, and hands it over again.
      handed = undefined
      e.lily_needs(out)
      const [kind, at, length] = words(out, 3)
      const name = bytes(at, length).toString()
      if (kind === MODULE && !packs.some(([n]) => n === name)) {
        const pack = offline ? undefined : moduleFile(name)
        packs.push([name, Boolean(pack)])
        e.lily_put(MODULE, ...put(Buffer.from(name)), ...(pack ? put(readFileSync(pack)) : [0, 0]))
        begin()
        continue
      }
      if (kind !== FILE || asked.includes(name)) throw new Error(`${file}: the engine asks for ${name} (kind ${kind}) again`)
      asked.push(name)
      if (e.lsb_parses() !== parsed) {
        stops++
        parsed = e.lsb_parses()
      }
      const disk = name.startsWith(`${DATADIR}/`) ? path.join(datadir, name.slice(DATADIR.length + 1)) : name
      e.lily_put(FILE, ...put(Buffer.from(name)), ...(existsSync(disk) ? put(readFileSync(disk)) : [0, 0]))
      begin()
    }
    if (handed) {
      const end = outputs()
      for (const [name, bytes] of Object.entries(handed.files)) {
        if (!end[name]?.equals(bytes)) throw new Error(`${file}: the page handed over, ${name}, is not the one at the compile's end`)
      }
    }
    return { asked, stops, packs, handedMs: handed?.ms }
  }
  const outputs = () => Object.fromEntries(Array.from({ length: e.lily_outputs() }, (_, j) => {
    e.lily_output(j, out)
    const [at, length, data, size] = words(out, 4)
    return [bytes(at, length).toString(), Buffer.from(bytes(data, size))]
  }))
  await instance()
  let revision = 0
  const results = []
  for (const [i, entry] of files.entries()) {
    const { file, text = readFileSync(file), before } = typeof entry === 'string' ? { file: entry } : entry
    if (fresh && i > 0) await instance()
    let shown = {}
    if (before !== undefined && !fresh) {
      e.lily_put(FILE, ...put(Buffer.from(file)), ...put(Buffer.from(before)))
      run(++revision, file)
      if (preview) shown = outputs()
      if (perform) run(revision, file, true)
    }
    e.lily_put(FILE, ...put(Buffer.from(file)), ...put(Buffer.from(text)))
    const start = performance.now()
    const { asked, stops, packs, handedMs } = run(++revision, file)
    const ms = performance.now() - start
    const values = new Float64Array(e.memory.buffer, e.lsb_times(), TIMES.length)
    const times = Object.fromEntries(TIMES.map((key, j) => [key, values[j]]))
    if (handedMs !== undefined) times.handedMs = handedMs
    const made = outputs()
    for (const name of Object.keys(made).filter((f) => f.endsWith('.cells'))) {
      const page = name.slice(0, -'.cells'.length)
      made[`${page}.before`] = shown[`${page}.svg`] ?? Buffer.alloc(0)
    }
    if (perform) {
      const performing = performance.now()
      run(revision, file, true)
      times.performMs = performance.now() - performing
      for (const [name, bytes] of Object.entries(outputs())) if (/\.(midi?|timing\.json)$/.test(name)) made[name] = bytes
    }
    results.push({ file, outputs: made, asked, stops, parses: e.lsb_parses(), ms, times, packs })
  }
  return results
}
