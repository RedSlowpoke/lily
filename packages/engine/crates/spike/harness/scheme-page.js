// scheme.html's runner, for MS1's budgets on the phones (progress.md, MS1's
// plan): chapter 03 Appendix B's b1–b8 run by the engine's Scheme in the
// built core.wasm (its lsb_* exports, crates/core/src/bench.rs), in a
// worker, as the engine will run. This file is both the page's script and,
// loaded by `new Worker`, the worker's.
//
//   /scheme.html?runs=5            runs per benchmark, each in a fresh instance (default 5)
//   /scheme.html?label=…           names the device or browser in the result
//
// For each benchmark: the median time of lsb_eval alone, the applications the
// VM counted (closures entered and primitives called, not the calls compiled
// inline), ns per application, the longest collector pause timed by the
// core's clock (lily_now, which is performance.now() here and as coarse: the
// result gives the clock's step), and linear memory after the run. The
// summary's ns per application is over b1–b7; b8 is reading and compiling.
//
// Then the interpreter's image (/scheme/scheme.image, after the library files
// and music-functions-init.ly, which the build writes): restored with lsb_restore
// as many times as each benchmark runs, each in a fresh instance, for the
// median ms of lsb_restore alone and linear memory after it, and collected
// once after each restore (lsb_collect) for the pause on the heap after init
// and the bytes live after it; then tools/bench/probe-library.scm in the
// last, whose output is in the result to compare with a native run's
// (tools/bench/test/image.test.mjs). Then the core itself through chapter
// 08 §4.2's ABI (K2): lily_start of /scheme/core.image in fresh instances,
// for its median ms and linear memory after it, and in the last a compile of
// SCORE through lily_put, lily_compile, lily_step and lily_output, as the
// studio's driver runs it, with the stderr it wrote. The result is
// shown, POSTed to /result (serve.mjs) and left in window.spikeResult, where
// measure.mjs waits for it.
// The core's split modules, joined to each instance (core-split.js, K4).
if (typeof importScripts === 'function') importScripts('core-split.js')

const BENCHES = ['b1-fib', 'b2-tak', 'b3-loop', 'b4-alist', 'b5-lists', 'b6-strings', 'b7-closures', 'b8-load']
// bench.rs's STATS, in its order.
const STATS = ['applications', 'collections', 'longestPauseMs', 'pauseMs', 'liveBytes', 'memory']
// The score the core compiles: no error, so its stderr is empty.
const SCORE = '\\version "2.26.0"\n\\relative { c\'4 d e f | g1 }\n'

if (typeof document === 'undefined') {
  // ---- The worker ----
  const median = (values) => [...values].sort((a, b) => a - b)[values.length >> 1]

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

  async function run(runs) {
    const started = performance.now()
    const bytes = await (await fetch('scheme/core.wasm')).arrayBuffer()
    const fetchedMs = performance.now() - started
    let t = performance.now()
    const mod = await WebAssembly.compile(bytes)
    const compileMs = performance.now() - t
    const result = { coreBytes: bytes.byteLength, fetchMs: +fetchedMs.toFixed(1), compileMs: +compileMs.toFixed(1), runs,
      clockStepMs: +clockStep().toFixed(4), benches: {} }
    for (const name of BENCHES) {
      const source = new Uint8Array(await (await fetch(`bench/${name}.scm`)).arrayBuffer())
      const head = new TextDecoder().decode(source.subarray(0, source.indexOf(10)))
      const expected = /, prints (.*)$/.exec(head)?.[1]
      const done = []
      for (let r = 0; r < runs; r++) {
        t = performance.now()
        const { exports } = await instantiateCore(mod)
        const init = exports.lsb_init()
        const initMs = performance.now() - t
        const p = exports.lsb_alloc(source.length)
        new Uint8Array(exports.memory.buffer, p, source.length).set(source)
        t = performance.now()
        const rc = exports.lsb_eval(p, source.length)
        const evalMs = performance.now() - t
        const [at, length] = new Uint32Array(exports.memory.buffer, exports.lsb_output(), 2)
        const output = new TextDecoder().decode(new Uint8Array(exports.memory.buffer, at, length)).trim()
        const values = new Float64Array(exports.memory.buffer, exports.lsb_stats(), STATS.length)
        const stats = Object.fromEntries(STATS.map((key, i) => [key, values[i]]))
        done.push({ ok: init === 0 && rc === 0, output, initMs, evalMs, stats, memory: exports.memory.buffer.byteLength })
      }
      const last = done.at(-1)
      const ms = median(done.map((d) => d.evalMs))
      const applications = last.stats.applications
      result.benches[name] = {
        ms: +ms.toFixed(1),
        initMs: +median(done.map((d) => d.initMs)).toFixed(2),
        applications,
        nsPerApplication: +(ms * 1e6 / applications).toFixed(1),
        collections: last.stats.collections,
        longestPauseMs: +Math.max(...done.map((d) => d.stats.longestPauseMs)).toFixed(2),
        memory: Math.max(...done.map((d) => d.memory)),
        output: last.ok ? last.output : `ERROR: ${last.output}`,
        expected: last.ok && last.output === expected,
        allMs: done.map((d) => +d.evalMs.toFixed(1)),
      }
      postMessage({ progress: name, bench: result.benches[name] })
    }
    result.image = await restore(mod, runs)
    if (result.image) postMessage({ image: result.image })
    result.core = await start(mod, runs)
    if (result.core) postMessage({ core: result.core })
    const calls = BENCHES.slice(0, 7).map((name) => result.benches[name])
    const all = Object.values(result.benches)
    result.summary = {
      ms: +all.reduce((sum, b) => sum + b.ms, 0).toFixed(1),
      nsPerApplication: +(calls.reduce((sum, b) => sum + b.ms, 0) * 1e6 / calls.reduce((sum, b) => sum + b.applications, 0)).toFixed(1),
      worstNsPerApplication: Math.max(...calls.map((b) => b.nsPerApplication)),
      longestPauseMs: Math.max(...all.map((b) => b.longestPauseMs)),
      memory: Math.max(...all.map((b) => b.memory)),
      allExpected: all.every((b) => b.expected),
    }
    postMessage({ result })
  }

  /** The image restored `runs` times, and the probe run in the last; undefined without an image. */
  async function restore(mod, runs) {
    let t = performance.now()
    const response = await fetch('scheme/scheme.image')
    if (!response.ok) return undefined
    const image = new Uint8Array(await response.arrayBuffer())
    const fetchMs = performance.now() - t
    const probe = new Uint8Array(await (await fetch('bench/probe-library.scm')).arrayBuffer())
    const done = []
    let exports
    for (let r = 0; r < runs; r++) {
      exports = (await instantiateCore(mod)).exports
      const at = exports.lsb_alloc(image.length)
      new Uint8Array(exports.memory.buffer, at, image.length).set(image)
      t = performance.now()
      const rc = exports.lsb_restore(at, image.length)
      const ms = performance.now() - t
      if (rc !== 0) return { bytes: image.length, error: `lsb_restore: ${rc}` }
      const memory = exports.memory.buffer.byteLength
      if (exports.lsb_collect() !== 0) return { bytes: image.length, error: 'lsb_collect failed' }
      const values = new Float64Array(exports.memory.buffer, exports.lsb_stats(), STATS.length)
      const [pauseMs, liveBytes] = ['pauseMs', 'liveBytes'].map((key) => values[STATS.indexOf(key)])
      done.push({ ms, memory, pauseMs, liveBytes })
    }
    const p = exports.lsb_alloc(probe.length)
    new Uint8Array(exports.memory.buffer, p, probe.length).set(probe)
    const rc = exports.lsb_eval(p, probe.length)
    const [at, length] = new Uint32Array(exports.memory.buffer, exports.lsb_output(), 2)
    const output = new TextDecoder().decode(new Uint8Array(exports.memory.buffer, at, length))
    return {
      bytes: image.length,
      fetchMs: +fetchMs.toFixed(1),
      restoreMs: +median(done.map((d) => d.ms)).toFixed(2),
      allRestoreMs: done.map((d) => +d.ms.toFixed(2)),
      memory: Math.max(...done.map((d) => d.memory)),
      longestPauseMs: +Math.max(...done.map((d) => d.pauseMs)).toFixed(2),
      allPauseMs: done.map((d) => +d.pauseMs.toFixed(2)),
      liveBytes: done.at(-1).liveBytes,
      probe: { ok: rc === 0, lines: output.trim().split('\n').length, output },
    }
  }

  /** core.image started with lily_start `runs` times, and SCORE compiled in the last; undefined without it. */
  async function start(mod, runs) {
    const response = await fetch('scheme/core.image')
    if (!response.ok) return undefined
    const image = new Uint8Array(await response.arrayBuffer())
    const done = []
    let exports
    const put = (bytes) => {
      const at = exports.lily_alloc(bytes.length)
      new Uint8Array(exports.memory.buffer, at, bytes.length).set(bytes)
      return [at, bytes.length]
    }
    for (let r = 0; r < runs; r++) {
      exports = (await instantiateCore(mod)).exports
      const [at, length] = put(image)
      const t = performance.now()
      const rc = exports.lily_start(at, length)
      const ms = performance.now() - t
      if (rc !== 0) return { bytes: image.length, error: `lily_start: ${rc}` }
      exports.lily_free(at, length)
      done.push({ ms, memory: exports.memory.buffer.byteLength })
    }
    const utf8 = new TextEncoder()
    const root = utf8.encode('score.ly')
    exports.lily_put(0, ...put(root), ...put(utf8.encode(SCORE)))
    const t = performance.now()
    exports.lily_compile(1, ...put(root), 1)
    let step
    while ((step = exports.lily_step(10)) === 1);
    const compileMs = performance.now() - t
    const out = exports.lily_alloc(16)
    const outputs = {}
    for (let i = 0; i < exports.lily_outputs(); i++) {
      exports.lily_output(i, out)
      const [name, nameLength, data, dataLength] = new Uint32Array(exports.memory.buffer, out, 4)
      const text = (at, n) => new TextDecoder().decode(new Uint8Array(exports.memory.buffer, at, n))
      outputs[text(name, nameLength)] = text(data, dataLength)
    }
    return {
      bytes: image.length,
      startMs: +median(done.map((d) => d.ms)).toFixed(2),
      allStartMs: done.map((d) => +d.ms.toFixed(2)),
      memory: Math.max(...done.map((d) => d.memory)),
      compileMs: +compileMs.toFixed(2),
      ok: step === 0 && outputs.stderr === '',
      outputs,
    }
  }

  onmessage = ({ data }) => run(data.runs).catch((error) => postMessage({ error: String(error.stack ?? error) }))
} else {
  // ---- The page ----
  const params = new URLSearchParams(location.search)
  const runs = Math.max(1, Number(params.get('runs') ?? 5) || 5)
  const label = params.get('label') ?? ''
  const $ = (id) => document.getElementById(id)
  const [again, status, rows, total, imageLine, coreLine, log] = ['again', 'status', 'rows', 'total', 'image', 'core', 'log'].map($)
  const say = (line) => (log.textContent += `${(performance.now() / 1000).toFixed(1).padStart(6)} s  ${line}\n`)
  const mb = (bytes) => (bytes / 2 ** 20).toFixed(1)

  function row(cells, bad) {
    const tr = document.createElement('tr')
    for (const [i, text] of cells.entries()) {
      const td = document.createElement('td')
      td.textContent = text
      if (bad && i === cells.length - 1) td.className = 'bad'
      tr.append(td)
    }
    return tr
  }

  function start() {
    again.disabled = true
    rows.replaceChildren()
    total.replaceChildren()
    imageLine.textContent = ''
    coreLine.textContent = ''
    status.textContent = `Running b1–b8, ${runs} runs each…`
    const worker = new Worker('scheme-page.js')
    worker.onmessage = ({ data }) => {
      if (data.progress) {
        const b = data.bench
        rows.append(row([data.progress, b.ms, b.applications.toLocaleString('en'), b.nsPerApplication, b.longestPauseMs, mb(b.memory),
          b.output], !b.expected))
        say(`${data.progress}: ${b.allMs.join(', ')} ms`)
        return
      }
      if (data.core) {
        const c = data.core
        coreLine.className = c.error || !c.ok ? 'bad' : ''
        coreLine.textContent = c.error
          ? `core.image: ${c.error}`
          : `core.image ${c.bytes.toLocaleString('en')} bytes: lily_start in ${c.startMs} ms, linear memory after it ` +
            `${mb(c.memory)} MB; a score compiled through lily_* in ${c.compileMs} ms, ${c.ok ? 'without errors' : 'FAILED'}`
        say(c.error ?? `lily_start: ${c.allStartMs.join(', ')} ms${c.ok ? '' : `; ${JSON.stringify(c.outputs)}`}`)
        return
      }
      if (data.image) {
        const i = data.image
        imageLine.className = i.error || !i.probe.ok ? 'bad' : ''
        imageLine.textContent = i.error
          ? `scheme.image: ${i.error}`
          : `scheme.image ${i.bytes.toLocaleString('en')} bytes: restored in ${i.restoreMs} ms, linear memory after it ` +
            `${mb(i.memory)} MB; collected in at most ${i.longestPauseMs} ms, ${i.liveBytes.toLocaleString('en')} bytes live; ` +
            `probe ${i.probe.ok ? 'ran' : 'FAILED'}, ${i.probe.lines} lines`
        say(i.error ?? `restore: ${i.allRestoreMs.join(', ')} ms; collection: ${i.allPauseMs.join(', ')} ms`)
        return
      }
      worker.terminate()
      again.disabled = false
      if (data.error) {
        status.textContent = 'Failed.'
        say(data.error)
        return
      }
      const result = { page: 'scheme', label, userAgent: navigator.userAgent, ...data.result }
      const s = result.summary
      total.append(row(['all', s.ms, '', `${s.nsPerApplication} (b1–b7)`, s.longestPauseMs, mb(s.memory), s.allExpected ? 'all expected' : 'NOT ALL EXPECTED'],
        !s.allExpected))
      status.textContent = `core.wasm ${result.coreBytes.toLocaleString('en')} bytes, compiled in ${result.compileMs} ms; clock step ${result.clockStepMs} ms`
      if (!result.image) imageLine.textContent = 'No scheme.image: build with LilyPond 2.26.0 installed.'
      if (!result.core) coreLine.textContent = 'No core.image: build with LilyPond 2.26.0 installed.'
      window.spikeResult = result
      fetch('result', { method: 'POST', body: JSON.stringify(result) })
        .then((r) => say(r.ok ? 'posted to /result' : `/result: ${r.status}`))
        .catch((error) => say(`/result: ${error.message}`))
    }
    worker.onerror = (event) => {
      status.textContent = 'Failed.'
      say(`worker: ${event.message}`)
    }
    worker.postMessage({ runs })
  }

  again.addEventListener('click', start)
  start()
}
