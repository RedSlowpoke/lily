// restore.html's runner, for the first restore in a fresh worker (03 §7's
// restore, 50 / 20 ms; progress.md, MS2 step 31, D88): what a worker that
// has just compiled core.wasm pays for its first restore over a warm one,
// and in which parts. It replaces MS1's scratch probe (device-runs,
// probe-serve.mjs). This file is both the page's script and, loaded by
// `new Worker`, the worker's.
//
//   /restore.html?workers=3        fresh workers for each mode (default 3)
//   /restore.html?runs=5           warm and pregrown restores in each worker (default 5)
//   /restore.html?modes=start      start: lily_start of /scheme/core.image, as the
//                                  studio's driver starts the core; restore: lsb_restore
//                                  of /scheme/scheme.image, MS1's probe's (default both)
//   /restore.html?compile=streaming  the worker compiles /scheme/core.wasm by
//                                  WebAssembly.compileStreaming, as the studio's worker
//                                  does, which a browser's code cache may serve; by
//                                  default it compiles the bytes the page fetched
//                                  (WebAssembly.compile), which nothing caches
//   /restore.html?thread=page      runs one fresh worker's work of the first mode on the
//                                  page's own thread instead, once: DevTools' CPU
//                                  throttling reaches a page, not its workers. A
//                                  visit in a fresh browser context is a fresh worker
//   /restore.html?label=…          names the device or browser in the result
//
// Each fresh worker compiles the core, then times, in ms by performance.now():
//
// - first: a fresh instance (instantiate), the image copied into its memory
//   (lily_alloc and the copy: copy), and the call alone, with its parts from
//   lsb_marks (crates/core/src/abi.rs): code, symbols, arenas, foreign,
//   globals, vm, then names and engine for lily_start. What the call took
//   beyond its marks is `call`;
// - second: the same call again in the same instance, its code compiled and
//   its memory grown (the first's engine is dropped after it);
// - warm: `runs` fresh instances of the compiled module, each restored once;
// - pregrown: as warm, but the memory grown first, untimed, by as much as the
//   first restore grew it (lily_alloc and lily_free of that many bytes), so
//   that warm against pregrown is what the memory's growth costs;
// - grow: memory.grow by the same pages in one call, in one more fresh instance.
//
// The first calls (the code's compilation and its first, slower tier) are
// first against warm, part by part. The result is shown, POSTed to /result
// (serve.mjs) and left in window.spikeResult.
// The core's split modules, joined to each instance (core-split.js, K4).
if (typeof importScripts === 'function') importScripts('core-split.js')

/** lsb_marks' parts, in order (START_MARKS in crates/core/src/lib.rs); lsb_restore ends after `vm`. */
const MARKS = ['code', 'symbols', 'arenas', 'foreign', 'globals', 'vm', 'names', 'engine']
const MODES = {
  start: { image: 'scheme/core.image', call: 'lily_start', marks: 8 },
  restore: { image: 'scheme/scheme.image', call: 'lsb_restore', marks: 6 },
}
/** 03 §7's restore, Must / Goal in ms. */
const MUST = 50
const GOAL = 20
const median = (values) => [...values].sort((a, b) => a - b)[values.length >> 1]
const round = (ms) => +ms.toFixed(2)

// ---- The worker's, and the page's with ?thread=page ----
/** The smallest step performance.now() takes here. */
function clockStep() {
  let step = Infinity
  for (let i = 0; i < 20; i++) {
    const t = performance.now()
    let u
    while ((u = performance.now()) === t);
    step = Math.min(step, u - t)
  }
  return step
}

async function instance(mod) {
  const t = performance.now()
  const { exports } = await instantiateCore(mod)
  return { e: exports, instantiateMs: performance.now() - t }
}

/** `mode`'s call on `image` in `e`, timed, with its parts. */
function restore(e, mode, image) {
  const { call, marks } = MODES[mode]
  const memoryBefore = e.memory.buffer.byteLength
  let t = performance.now()
  const at = e.lily_alloc(image.length)
  new Uint8Array(e.memory.buffer, at, image.length).set(image)
  const copyMs = performance.now() - t
  t = performance.now()
  const rc = e[call](at, image.length)
  const ms = performance.now() - t
  // lsb_restore frees the image itself; lily_start leaves it to the host, as driver.ts does after it.
  if (call === 'lily_start') e.lily_free(at, image.length)
  if (rc !== 0) throw new Error(`${call}: ${rc}`)
  const m = new Float64Array(e.memory.buffer, e.lsb_marks(), 2 + MARKS.length)
  if (m[0] !== marks) throw new Error(`${call} ended ${m[0]} parts, not ${marks}`)
  const parts = {}
  for (let i = 0; i < marks; i++) parts[MARKS[i]] = round(m[2 + i] - m[1 + i])
  parts.call = round(ms - (m[1 + marks] - m[1]))
  return { copyMs: round(copyMs), ms: round(ms), parts, memoryBefore, memoryAfter: e.memory.buffer.byteLength }
}

async function run({ mode, compile, wasm, image, runs }) {
  let t = performance.now()
  const mod = compile === 'streaming'
    ? await WebAssembly.compileStreaming(fetch('scheme/core.wasm'))
    : await WebAssembly.compile(wasm)
  const compileMs = performance.now() - t
  const fresh = await instance(mod)
  const first = { instantiateMs: round(fresh.instantiateMs), ...restore(fresh.e, mode, image) }
  const second = restore(fresh.e, mode, image)
  const grown = first.memoryAfter - first.memoryBefore
  const warm = []
  const pregrown = []
  for (let r = 0; r < runs; r++) {
    const w = await instance(mod)
    warm.push({ instantiateMs: round(w.instantiateMs), ...restore(w.e, mode, image) })
    const p = await instance(mod)
    const at = p.e.lily_alloc(grown)
    p.e.lily_free(at, grown)
    pregrown.push(restore(p.e, mode, image))
  }
  const g = await instance(mod)
  t = performance.now()
  g.e.memory.grow(grown / 65536)
  const growMs = performance.now() - t
  return { mode, compile, clockStepMs: +clockStep().toFixed(4), compileMs: round(compileMs), imageBytes: image.length,
    grownBytes: grown, first, second, warm, pregrown, growMs: round(growMs) }
}


if (typeof document === 'undefined') {
  // ---- The worker ----
  onmessage = ({ data }) => run(data).then((result) => postMessage({ result }), (error) => postMessage({ error: String(error.stack ?? error) }))
} else {
  // ---- The page ----
  const params = new URLSearchParams(location.search)
  const workers = Math.max(1, Number(params.get('workers') ?? 3) || 3)
  const runs = Math.max(1, Number(params.get('runs') ?? 5) || 5)
  const modes = (params.get('modes') ?? 'start,restore').split(',').filter((m) => Object.hasOwn(MODES, m))
  const compile = params.get('compile') === 'streaming' ? 'streaming' : 'bytes'
  const thread = params.get('thread') === 'page' ? 'page' : 'worker'
  const label = params.get('label') ?? ''
  const $ = (id) => document.getElementById(id)
  const [again, status, rows, partsHead, parts, log] = ['again', 'status', 'workers', 'partsHead', 'parts', 'log'].map($)
  const say = (line) => (log.textContent += `${(performance.now() / 1000).toFixed(1).padStart(6)} s  ${line}\n`)
  const verdict = (ms) => (ms <= GOAL ? 'within Goal' : ms <= MUST ? 'within Must' : 'over Must')

  function row(cells, bad) {
    const tr = document.createElement('tr')
    for (const [i, text] of cells.entries()) {
      const td = document.createElement(i === 0 && bad === undefined ? 'th' : 'td')
      td.textContent = text
      if (bad && i === cells.length - 1) td.className = 'bad'
      tr.append(td)
    }
    return tr
  }

  /** One fresh worker's run of `mode`. */
  function inWorker(message) {
    return new Promise((resolve, reject) => {
      const worker = new Worker('restore-page.js')
      worker.onmessage = ({ data }) => {
        worker.terminate()
        if (data.error) reject(new Error(data.error))
        else resolve(data.result)
      }
      worker.onerror = (event) => {
        worker.terminate()
        reject(new Error(`worker: ${event.message}`))
      }
      worker.postMessage(message, message.wasm ? [message.wasm] : [])
    })
  }

  /** A mode's workers summed up: each part's median, first against warm. */
  function summary(mode, done) {
    const names = [...MARKS.slice(0, MODES[mode].marks), 'call']
    const warm = done.flatMap((w) => w.warm)
    const firstParts = Object.fromEntries(names.map((n) => [n, round(median(done.map((w) => w.first.parts[n])))]))
    const warmParts = Object.fromEntries(names.map((n) => [n, round(median(warm.map((d) => d.parts[n])))]))
    return {
      firstMs: done.map((w) => w.first.ms),
      firstMedianMs: round(median(done.map((w) => w.first.ms))),
      secondMs: done.map((w) => w.second.ms),
      warmMedianMs: round(median(warm.map((d) => d.ms))),
      warmRangeMs: [Math.min(...warm.map((d) => d.ms)), Math.max(...warm.map((d) => d.ms))],
      pregrownMedianMs: round(median(done.flatMap((w) => w.pregrown.map((d) => d.ms)))),
      growMs: done.map((w) => w.growMs),
      compileMs: done.map((w) => w.compileMs),
      instantiateMs: done.map((w) => w.first.instantiateMs),
      copyMs: done.map((w) => w.first.copyMs),
      memoryAfter: done[0].first.memoryAfter,
      firstParts,
      warmParts,
    }
  }

  async function begin() {
    again.disabled = true
    rows.replaceChildren()
    parts.replaceChildren()
    partsHead.replaceChildren(row(['mode', 'run', ...MARKS, 'call']))
    status.textContent = thread === 'page' ? 'Running on the page\'s thread…' : `Running, ${workers} fresh workers a mode…`
    try {
      const fetchBytes = async (name) => {
        const response = await fetch(name)
        if (!response.ok) throw new Error(`${name}: ${response.status}; run npm run build`)
        return response.arrayBuffer()
      }
      const wasm = await fetchBytes('scheme/core.wasm')
      // On the page's thread, a second compile of the same bytes could be given the first's code: one run only.
      const [modesRun, workersRun] = thread === 'page' ? [modes.slice(0, 1), 1] : [modes, workers]
      const result = { page: 'restore', label, userAgent: navigator.userAgent, compile, thread, workers: workersRun, runs, modes: {} }
      for (const mode of modesRun) {
        const image = new Uint8Array(await fetchBytes(MODES[mode].image))
        const done = []
        for (let w = 0; w < workersRun; w++) {
          const message = { mode, compile, wasm: compile === 'bytes' ? wasm.slice(0) : null, image, runs }
          const r = thread === 'page' ? await run(message) : await inWorker(message)
          done.push(r)
          const warmMs = round(median(r.warm.map((d) => d.ms)))
          rows.append(row([mode, r.compileMs, r.first.instantiateMs, r.first.copyMs, r.first.ms, r.second.ms, warmMs,
            round(median(r.pregrown.map((d) => d.ms))), r.growMs, verdict(r.first.ms)], r.first.ms > MUST))
          say(`${mode} worker ${w + 1}: first ${r.first.ms} ms ${JSON.stringify(r.first.parts)}; warm ${r.warm.map((d) => d.ms).join(', ')}; ` +
            `memory ${r.first.memoryBefore} → ${r.first.memoryAfter}`)
        }
        const s = summary(mode, done)
        result.modes[mode] = { summary: s, workers: done }
        result.clockStepMs = done[0].clockStepMs
        const names = [...MARKS, 'call']
        const cells = (p) => names.map((n) => (n in p ? p[n] : ''))
        parts.append(row([mode, 'first', ...cells(s.firstParts)]), row([mode, 'warm', ...cells(s.warmParts)]),
          row([mode, 'first − warm', ...cells(Object.fromEntries(Object.keys(s.firstParts).map((n) => [n, round(s.firstParts[n] - s.warmParts[n])])))]))
      }
      status.textContent = `${compile} compile, on the ${thread}'s thread; clock step ${result.clockStepMs} ms`
      window.spikeResult = result
      fetch('result', { method: 'POST', body: JSON.stringify(result) })
        .then((r) => say(r.ok ? 'posted to /result' : `/result: ${r.status}`))
        .catch((error) => say(`/result: ${error.message}`))
    } catch (error) {
      status.textContent = 'Failed.'
      say(String(error.stack ?? error))
      window.spikeResult = { page: 'restore', label, error: String(error) }
    }
    again.disabled = false
  }

  again.addEventListener('click', begin)
  begin()
}
