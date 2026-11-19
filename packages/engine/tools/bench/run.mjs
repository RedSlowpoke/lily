// Times a Scheme interpreter built to chapter 03 Appendix B's bench ABI on
// b1–b8, as the appendix's runner did, in Node. The default module is the
// engine's core.wasm (scripts/build.mjs), whose lsb_* exports run the engine's
// Scheme (crates/core/src/bench.rs).
//
//   node tools/bench/run.mjs [module.wasm] [--image [file.image]] [bench ...]     RUNS=5 by default
//
// With --image, each run restores the interpreter from an image (by default
// dist/engine/scheme.image, which the build writes from the installed
// LilyPond's scm/lily/) with lsb_restore instead of lsb_init.
//
// ABI: exports memory, lsb_init() -> i32 (0 = ok), lsb_alloc(len) -> ptr,
// lsb_eval(ptr, len) -> i32 (0 = ok). Output goes to WASI stdout, to the
// import env.lsb_write(ptr, len), or, for the core, whose only import is
// env.lily_now, to lsb_output() -> ptr of [address, length]. The core also
// exports lsb_stats() -> ptr of f64s: the applications, collections, longest
// pause and the rest of bench.rs's STATS. env.lsb_now and env.lily_now are
// the clock. lsb_restore(ptr, len) -> i32 (0 = ok), of the core only, takes
// an image from lsb_alloc in place of lsb_init. Each run is a fresh instance:
// init is the interpreter's own start-up, eval the benchmark alone; times are
// medians.
import { spawnSync } from 'node:child_process'
import { closeSync, existsSync, openSync, readdirSync, readFileSync, realpathSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

export const benchDir = path.dirname(fileURLToPath(import.meta.url))
export const coreWasm = path.resolve(benchDir, '../../dist/engine/core.wasm')
/** The interpreter's image after lily-library.scm and music-functions.scm (scripts/build.mjs). */
export const schemeImage = path.resolve(benchDir, '../../dist/engine/scheme.image')
/** bench.rs's STATS, in its order. */
export const STATS = ['applications', 'collections', 'longestPauseMs', 'pauseMs', 'liveBytes', 'memory']

/** scm/lily/ of the installed LilyPond 2.26.0, or undefined: where crates/scheme/tests/image.rs finds it. */
export function lilyScm() {
  const version = spawnSync('lilypond', ['--version'], { encoding: 'utf8' })
  if (version.error || !version.stdout.includes('GNU LilyPond 2.26.0')) return undefined
  const binary = spawnSync('sh', ['-c', 'command -v lilypond'], { encoding: 'utf8' }).stdout.trim()
  const lily = path.resolve(path.dirname(realpathSync(binary)), '../share/lilypond/2.26.0/scm/lily')
  return existsSync(path.join(lily, 'music-functions.scm')) ? lily : undefined
}

/**
 * The split modules (K4, MS4 step 20) that dist/engine/manifest.json names
 * beside the core, compiled once; none without a manifest (CORE=spike).
 */
export const splitModules = (() => {
  let modules
  return () => {
    if (modules) return modules
    const dir = path.dirname(coreWasm)
    const manifest = existsSync(path.join(dir, 'manifest.json')) ? JSON.parse(readFileSync(path.join(dir, 'manifest.json'), 'utf8')) : { modules: {} }
    modules = Object.values(manifest.modules).filter((m) => m.kind === 'split').map((m) => new WebAssembly.Module(readFileSync(path.join(dir, m.file))))
    return modules
  }
})()

/**
 * Joins every split module to `instance`, a core's, as the studio's driver
 * instantiates one it loaded into the core: the whole core, as a driver that
 * loaded them all has it.
 */
export async function joinModules(instance) {
  for (const module of splitModules()) await WebAssembly.instantiate(module, { primary: instance.exports })
  return instance
}

/**
 * An instance of the core `mod` with `env`, its split modules' placeholders
 * (`placeholder.<module>`, which throw) and every split module joined
 * ([`joinModules`]).
 */
export async function instantiateCore(mod, env = { lily_now: () => performance.now() }) {
  const imports = { env }
  for (const i of WebAssembly.Module.imports(mod)) {
    if (!i.module.startsWith('placeholder.')) continue
    imports[i.module] ??= {}
    imports[i.module][i.name] = () => { throw new Error(`the core called ${i.module} ${i.name}, which is not loaded`) }
  }
  return joinModules(await WebAssembly.instantiate(mod, imports))
}

/** b1–b8 (or those whose names start with one of `only`), each with what its first line says it prints. */
export function benchmarks(only = []) {
  return readdirSync(benchDir)
    .filter((f) => /^b\d.*\.scm$/.test(f) && (only.length === 0 || only.some((o) => f.startsWith(o))))
    .sort()
    .map((name) => {
      const source = readFileSync(path.join(benchDir, name))
      const expected = /, prints (.*)$/m.exec(source.subarray(0, source.indexOf(10)).toString())?.[1]
      return { name, source, expected }
    })
}

/** A fresh instance of `mod`; `out.text` collects what it writes. */
async function instance(mod, out) {
  const imports = {}
  let wasi
  if (WebAssembly.Module.imports(mod).some((i) => i.module.startsWith('wasi'))) {
    // Imported here: node:wasi warns that it is experimental when it is loaded.
    const { WASI } = await import('node:wasi')
    wasi = new WASI({ version: 'preview1', args: ['lsb'], env: {}, returnOnExit: true, stdout: out.fd, stderr: 2 })
    Object.assign(imports, wasi.getImportObject())
  }
  let inst
  const env = {
    lsb_write: (p, n) => { out.text += Buffer.from(inst.exports.memory.buffer, p, n).toString() },
    lsb_now: () => performance.now(),
    lily_now: () => performance.now(),
  }
  imports.env = new Proxy(env, { get: (o, k) => o[k] ?? (() => { throw new Error(`import env.${String(k)} called`) }) })
  for (const i of WebAssembly.Module.imports(mod)) {
    if (i.module !== 'env' && !imports[i.module]) {
      imports[i.module] = new Proxy({}, { get: (_, k) => () => { throw new Error(`import ${i.module}.${String(k)} called`) } })
    }
  }
  inst = await WebAssembly.instantiate(mod, imports)
  // The core's split modules, joined: their placeholders are the proxies' above.
  if (WebAssembly.Module.imports(mod).some((i) => i.module.startsWith('placeholder.'))) await joinModules(inst)
  if (wasi) wasi.initialize(inst)
  return inst
}

/**
 * One run of `source` in a fresh instance: its output, its times and, from
 * the core, its stats. Given an image, the instance restores it (lsb_restore)
 * instead of lsb_init: restoreMs is that call, memoryAfterStart linear memory
 * after it. When the start fails (init is not 0), nothing is evaluated.
 */
export async function runOnce(mod, source, image) {
  const outPath = path.join(tmpdir(), `lsb-${process.pid}.out`)
  const out = { fd: openSync(outPath, 'w'), text: '' }
  try {
    const t0 = performance.now()
    const inst = await instance(mod, out)
    const { exports } = inst
    let init, restoreMs
    if (image) {
      const at = exports.lsb_alloc(image.length)
      new Uint8Array(exports.memory.buffer, at, image.length).set(image)
      const t = performance.now()
      init = exports.lsb_restore(at, image.length)
      restoreMs = performance.now() - t
    } else {
      init = exports.lsb_init()
    }
    const t1 = performance.now()
    const memoryAfterStart = exports.memory.buffer.byteLength
    // A refused image leaves no interpreter, and lsb_eval would trap.
    if (init !== 0) return { ok: false, init, output: '', initMs: t1 - t0, restoreMs, memoryAfterStart }
    const p = exports.lsb_alloc(source.length)
    new Uint8Array(exports.memory.buffer, p, source.length).set(source)
    const t2 = performance.now()
    const rc = exports.lsb_eval(p, source.length)
    const t3 = performance.now()
    closeSync(out.fd)
    out.fd = undefined
    let output = out.text + readFileSync(outPath, 'utf8')
    if (exports.lsb_output) {
      const [at, length] = new Uint32Array(exports.memory.buffer, exports.lsb_output(), 2)
      output += Buffer.from(exports.memory.buffer, at, length).toString()
    }
    let stats
    if (exports.lsb_stats) {
      const values = new Float64Array(exports.memory.buffer, exports.lsb_stats(), STATS.length)
      stats = Object.fromEntries(STATS.map((name, i) => [name, values[i]]))
    }
    return { ok: init === 0 && rc === 0, init, rc, output, initMs: t1 - t0, restoreMs, evalMs: t3 - t2, stats,
      memoryAfterStart, memory: exports.memory.buffer.byteLength }
  } finally {
    if (out.fd !== undefined) closeSync(out.fd)
    rmSync(outPath, { force: true })
  }
}

const median = (values) => [...values].sort((a, b) => a - b)[values.length >> 1]

/**
 * Every benchmark RUNS times, as the appendix's runner reports them, with the
 * core's stats beside; each run from `image` (a file) when it is given.
 */
export async function run(file = coreWasm, only = [], runs = 5, image) {
  const bytes = readFileSync(file)
  const imageBytes = image && readFileSync(image)
  const t = performance.now()
  const mod = await WebAssembly.compile(bytes)
  const results = { module: path.basename(file), bytes: bytes.length, compileMs: +(performance.now() - t).toFixed(1), benches: {} }
  if (image) results.image = { file: path.basename(image), bytes: imageBytes.length }
  for (const { name, source, expected } of benchmarks(only)) {
    const done = []
    for (let r = 0; r < runs; r++) done.push(await runOnce(mod, source, imageBytes))
    const last = done.at(-1)
    let output = last.output.trim()
    if (!last.ok) output = `ERROR init=${last.init} eval=${last.rc}: ${output}`
    const evalMs = median(done.map((d) => d.evalMs))
    const result = { output, expected: output === expected, initMs: +median(done.map((d) => d.initMs)).toFixed(2),
      evalMs: +evalMs.toFixed(1), memoryMB: +(Math.max(...done.map((d) => d.memory)) / 2 ** 20).toFixed(1) }
    if (image) {
      Object.assign(result, { restoreMs: +median(done.map((d) => d.restoreMs)).toFixed(2),
        memoryAfterRestoreMB: +(Math.max(...done.map((d) => d.memoryAfterStart)) / 2 ** 20).toFixed(1) })
    }
    if (last.stats) {
      const { applications } = last.stats
      Object.assign(result, {
        applications,
        nsPerApplication: applications ? +(evalMs * 1e6 / applications).toFixed(1) : null,
        collections: last.stats.collections,
        longestPauseMs: +Math.max(...done.map((d) => d.stats.longestPauseMs)).toFixed(2),
      })
    }
    results.benches[name] = result
    console.error(name, JSON.stringify(result))
  }
  return results
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2)
  const file = args[0]?.endsWith('.wasm') ? args.shift() : coreWasm
  let image
  if (args[0] === '--image') {
    args.shift()
    image = args[0]?.endsWith('.image') ? args.shift() : schemeImage
  }
  console.log(JSON.stringify(await run(file, args, +(process.env.RUNS ?? 5), image)))
}
