// B8 and memory in core.wasm (MS6 step 11, README §4.3, §4.5; 00 §6.1): the
// built core driven in Node as the studio's driver drives it, in steps of
// STEP_MS, one instance a trial. A newer edit reaches a worker only between
// its calls (08 §4.3), so B8 is how long after the edit arrived the step
// running then ended, and the first compile's stop is what the next put does
// (Engine::stop_running).
//
//   node tools/bench/b8.mjs [--after <ms,ms,…>] [--edit <from>=<to>] <file.ly>…
//
// For each file: one compile whole, with each step's milliseconds and the
// memory after it; then, for each `--after`, that compile stopped so long
// into it by the edited text, which then runs to its end; then the same
// after a compile that finished, stopped 20 ms into the edit.
// A compile that traps is reported as what it is: the score needs more than
// B4's 256 MiB, and the driver's answer is a new instance.
import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { moduleFile, preloadedModules } from '../compare/core.mjs'
import { benchDir, coreWasm, instantiateCore, lilyScm } from './run.mjs'

const coreImage = path.resolve(benchDir, '../../dist/engine/core.image')
/** driver.ts' STEP_MS, protocol.ts' Step and Need, and the core's name for LilyPond's files. */
export const STEP_MS = 10
const [DONE, MORE] = [0, 1]
const [FILE, MODULE] = [0, 1]
const DATADIR = '<datadir>'

/** A fresh instance of core.wasm with the preloaded packs, and how to compile in it. */
export async function instance(wasm) {
  const e = (await instantiateCore(wasm)).exports
  const put = (bytes) => {
    const at = e.lily_alloc(bytes.length)
    new Uint8Array(e.memory.buffer, at, bytes.length).set(bytes)
    return [at, bytes.length]
  }
  if (e.lily_start(...put(readFileSync(coreImage))) !== 0) throw new Error('lily_start refused core.image')
  for (const [name, bytes] of preloadedModules()) e.lily_put(MODULE, ...put(Buffer.from(name)), ...put(bytes))
  const out = e.lily_alloc(16)
  const datadir = path.resolve(lilyScm(), '../..')
  const words = (at, n) => [...new Uint32Array(e.memory.buffer, at, n)]
  /** Answers what the engine asks for, as the driver does, and starts the compile again. */
  const answer = (name, revision) => {
    e.lily_needs(out)
    const [kind, at, length] = words(out, 3)
    const asked = Buffer.from(e.memory.buffer, at, length).toString()
    if (kind === MODULE) {
      const pack = moduleFile(asked)
      e.lily_put(MODULE, ...put(Buffer.from(asked)), ...(pack ? put(readFileSync(pack)) : [0, 0]))
    } else {
      const disk = asked.startsWith(`${DATADIR}/`) ? path.join(datadir, asked.slice(DATADIR.length + 1)) : asked
      e.lily_put(FILE, ...put(Buffer.from(asked)), ...(existsSync(disk) ? put(readFileSync(disk)) : [0, 0]))
    }
    e.lily_compile(revision, ...put(Buffer.from(name)), 1)
  }
  /**
   * Puts `text` under `name` and compiles it as `revision` until `until`
   * says stop, given the milliseconds since it began; each step's
   * milliseconds, the memory after the last, and whether it ended.
   */
  const compile = (name, text, revision, until = () => false, steps = []) => {
    const start = performance.now()
    e.lily_put(FILE, ...put(Buffer.from(name)), ...put(text))
    e.lily_compile(revision, ...put(Buffer.from(name)), 1)
    let done = false
    for (;;) {
      if (until(performance.now() - start)) break
      const at = performance.now()
      const step = e.lily_step(STEP_MS)
      steps.push(performance.now() - at)
      if (step === DONE) {
        done = true
        break
      }
      if (step !== MORE) answer(name, revision)
    }
    return { steps, done, ms: performance.now() - start, memory: e.memory.buffer.byteLength }
  }
  return { e, put, compile }
}

const quantile = (values, q) => [...values].sort((a, b) => a - b)[Math.min(values.length - 1, Math.floor(values.length * q))] ?? 0
const round = (x) => Math.round(x * 10) / 10

/** What a run of steps shows. */
export function summary({ steps, done, ms, memory }) {
  return {
    done,
    ms: round(ms),
    steps: steps.length,
    p50: round(quantile(steps, 0.5)),
    p99: round(quantile(steps, 0.99)),
    longest: round(Math.max(0, ...steps)),
    over50: steps.filter((s) => s > 50).length,
    MiB: round(memory / 2 ** 20),
  }
}

/** The compile whole, its memory; `{ trapped }` at B4's cap, with how far it got. */
export async function whole(wasm, name, text) {
  const { e, compile } = await instance(wasm)
  const steps = []
  let elapsed = 0
  try {
    const run = compile(name, text, 1, (at) => ((elapsed = at), false), steps)
    return { ...summary(run), times: [...new Float64Array(e.memory.buffer, e.lsb_times(), 12)].map(round) }
  } catch (error) {
    return { ...summary({ steps, done: false, ms: elapsed, memory: e.memory.buffer.byteLength }), trapped: String(error.message ?? error) }
  }
}

/**
 * B8: `text` compiled, stopped `after` ms into it by `edited`, as a worker
 * sees the edit: when the step running then ends. `waited` is that, `stop` the
 * first put of the newer compile, which undoes the old; `first`, if given, is
 * compiled to its end before, so that the compile stopped is an edit's.
 */
export async function superseded(wasm, name, text, edited, after, first) {
  const { put, e, compile } = await instance(wasm)
  let revision = 1
  const trial = { after }
  try {
    if (first) compile(name, first, revision++)
    const run = compile(name, text, revision++, (at) => at >= after)
    trial.stopped = !run.done
    trial.waited = round(run.ms - after)
    trial.longest = round(Math.max(0, ...run.steps))
    const started = performance.now()
    e.lily_put(FILE, ...put(Buffer.from(name)), ...put(edited))
    trial.stop = round(performance.now() - started)
    trial.MiB = round(e.memory.buffer.byteLength / 2 ** 20)
    const next = compile(name, edited, revision++)
    return { ...trial, next: round(next.ms), done: next.done, MiB: round(next.memory / 2 ** 20) }
  } catch (error) {
    return { ...trial, trapped: String(error.message ?? error), MiB: round(e.memory.buffer.byteLength / 2 ** 20) }
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const args = process.argv.slice(2)
  const option = (flag, fallback) => {
    const i = args.indexOf(flag)
    return i < 0 ? fallback : args.splice(i, 2)[1]
  }
  const afters = option('--after', '20').split(',').map(Number)
  const [from, to] = option('--edit', "c'=d'").split('=')
  const wasm = await WebAssembly.compile(readFileSync(coreWasm))
  for (const file of args) {
    const text = readFileSync(file)
    const edited = Buffer.from(text.toString().replace(from, to))
    if (edited.equals(text)) throw new Error(`${file}: no ${from} to edit`)
    const name = path.basename(file)
    console.log(JSON.stringify({ file: name, whole: await whole(wasm, name, text) }))
    for (const after of afters) console.log(JSON.stringify({ file: name, first: await superseded(wasm, name, text, edited, after) }))
    console.log(JSON.stringify({ file: name, edit: await superseded(wasm, name, edited, text, 20, text) }))
  }
}
