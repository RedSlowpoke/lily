// svg.html's runner, for README §4.3's SVG phase on the devices (MS3 step
// 9; S 20 / 7 ms, L 40 / 15 ms on the Android reference, iOS 0.6 times; the
// 10 scores are S): the 11 pages of step 6's references written in the file
// form by the built core.wasm's bench (lsb_svg, crates/core/src/bench.rs), in
// a worker. This file is both the page's script and, loaded by `new Worker`,
// the worker's.
//
//   /svg.html?runs=20         samples of each page (default 20)
//   /svg.html?steps=10        a sample lasts at least this many steps of the
//                             clock (default 10): a page written `reps` times
//                             in one call, its time the call's over `reps`,
//                             where the clock is coarse (WebKit's 1 ms)
//   /svg.html?thread=page     runs on the page's own thread instead: DevTools'
//                             CPU throttling reaches a page, not its workers
//   /svg.html?label=…         names the device or browser in the result
//
// The worker compiles /scheme/core.wasm, restores /scheme/scheme.image
// (lsb_restore), gives it W1's Emmentaler packs (/svg/fonts/, lsb_pack) as
// crates/svg's write example gives them, and reads each score's records
// (/svg/<ID>/pages.scm). Each page is written once (`first`), its SVG equal
// to the write example's (/svg/<ID>/<file>, serve.mjs) or the page fails;
// then `runs` samples, each `reps` writes. The record is read into layout's
// stencil before lsb_svg's clock starts. Median and p90 are nearest ranks.
// The result is shown, POSTed to /result (serve.mjs) and left in
// window.spikeResult.
// The core's split modules, joined to each instance (core-split.js, K4).
if (typeof importScripts === 'function') importScripts('core-split.js')

/** README §4.3's SVG writing, Must / Goal in ms, for class S on the Android reference. */
const MUST = 20
const GOAL = 7
const rank = (values, p) => [...values].sort((a, b) => a - b)[Math.ceil(p * values.length) - 1]
const round = (ms) => +ms.toFixed(3)

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

async function run({ runs, steps }, post) {
  const utf8 = new TextEncoder()
  const fetchOk = async (url) => {
    const response = await fetch(url)
    if (!response.ok) throw new Error(`${url}: ${response.status}`)
    return response
  }
  let t = performance.now()
  const index = await (await fetchOk('svg/index.json')).json()
  const mod = await WebAssembly.compile(await (await fetchOk('scheme/core.wasm')).arrayBuffer())
  const image = new Uint8Array(await (await fetchOk('scheme/scheme.image')).arrayBuffer())
  const packs = await Promise.all(index.packs.map(async (name) => new Uint8Array(await (await fetchOk(`svg/fonts/${name}`)).arrayBuffer())))
  const loadMs = performance.now() - t
  const { exports: e } = await instantiateCore(mod)
  const put = (bytes) => {
    const at = e.lsb_alloc(bytes.length)
    new Uint8Array(e.memory.buffer, at, bytes.length).set(bytes)
    return [at, bytes.length]
  }
  const output = () => {
    const [at, length] = new Uint32Array(e.memory.buffer, e.lsb_output(), 2)
    return new TextDecoder().decode(new Uint8Array(e.memory.buffer, at, length))
  }
  t = performance.now()
  if (e.lsb_restore(...put(image)) !== 0) throw new Error('lsb_restore refused scheme.image')
  for (const [i, pack] of packs.entries()) {
    const [at, length] = put(pack)
    if (e.lsb_pack(at, length) !== 0) throw new Error(`${index.packs[i]}: ${output()}`)
    e.lily_free(at, length)
  }
  const startMs = performance.now() - t
  /** Writes `record` `reps` times: lsb_svg's milliseconds. */
  const write = (record, roots, reps) => {
    const [r, rl] = put(record)
    const [d, dl] = put(roots)
    const ms = e.lsb_svg(r, rl, d, dl, reps)
    e.lily_free(r, rl)
    e.lily_free(d, dl)
    if (ms < 0) throw new Error(output())
    return ms
  }

  const step = clockStep()
  const result = { runs, steps, clockStepMs: +step.toFixed(4), loadMs: round(loadMs), startMs: round(startMs),
    packs: index.packs, pages: [] }
  for (const score of index.scores) {
    const text = await (await fetchOk(`svg/${score.id}/pages.scm`)).text()
    const records = text.split('\n').filter((line) => line.startsWith('(page '))
    if (records.length !== score.pages.length) throw new Error(`${score.id}: ${records.length} records, ${score.pages.length} pages`)
    const roots = utf8.encode(score.roots.join('\n'))
    for (const [i, line] of records.entries()) {
      const page = score.pages[i]
      const record = utf8.encode(line)
      const expected = await (await fetchOk(`svg/${score.id}/${encodeURIComponent(page.file)}`)).text()
      const first = write(record, roots, 1)
      if (output() !== expected) throw new Error(`${score.id} ${page.file}: not the write example's bytes`)
      // The reps that make a sample last `steps` steps of the clock, from a second write's time.
      const second = write(record, roots, 1)
      const reps = Math.min(1000, Math.max(1, Math.ceil((steps * step) / Math.max(second, step / 2))))
      const ms = []
      for (let r = 0; r < runs; r++) ms.push(write(record, roots, reps) / reps)
      const row = { id: score.id, file: page.file, bytes: utf8.encode(expected).length, first: round(first), reps,
        median: round(rank(ms, 0.5)), p90: round(rank(ms, 0.9)), all: ms.map(round) }
      result.pages.push(row)
      post({ page: row })
    }
  }
  result.memory = e.memory.buffer.byteLength
  return result
}

if (typeof document === 'undefined') {
  // ---- The worker ----
  onmessage = ({ data }) => run(data, postMessage).then((result) => postMessage({ result }), (error) => postMessage({ error: String(error.stack ?? error) }))
} else {
  // ---- The page ----
  const params = new URLSearchParams(location.search)
  const runs = Math.max(1, Number(params.get('runs') ?? 20) || 20)
  const steps = Math.max(1, Number(params.get('steps') ?? 10) || 10)
  const thread = params.get('thread') === 'page' ? 'page' : 'worker'
  const label = params.get('label') ?? ''
  // README §4.3: iOS gets 0.6 times each figure.
  const factor = /iPhone|iPad|iPod/.test(navigator.userAgent) ? 0.6 : 1
  const [must, goal] = [MUST * factor, GOAL * factor].map(round)
  const $ = (id) => document.getElementById(id)
  const [again, status, pages, log] = ['again', 'status', 'pages', 'log'].map($)
  const say = (line) => (log.textContent += `${(performance.now() / 1000).toFixed(1).padStart(6)} s  ${line}\n`)
  const verdict = (ms) => (ms <= goal ? 'within Goal' : ms <= must ? 'within Must' : 'over Must')

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

  const show = (p) => {
    const v = verdict(p.median)
    pages.append(row([p.id, p.file, p.bytes, p.first, p.reps, p.median, p.p90, `${must} / ${goal}`, v], v === 'over Must'))
    say(`${p.id} ${p.file}: ${p.all.join(', ')} ms`)
  }

  function done(data) {
    again.disabled = false
    if (data.error) {
      status.textContent = 'Failed.'
      return say(data.error)
    }
    const result = { page: 'svg', label, userAgent: navigator.userAgent, thread, factor, must, goal, ...data.result }
    status.textContent = `on the ${thread}'s thread; core and packs loaded in ${result.loadMs} ms, restored and given ` +
      `the packs in ${result.startMs} ms; clock step ${result.clockStepMs} ms`
    window.spikeResult = result
    fetch('result', { method: 'POST', body: JSON.stringify(result) })
      .then((r) => say(r.ok ? 'posted to /result' : `/result: ${r.status}`))
      .catch((error) => say(`/result: ${error.message}`))
  }

  function begin() {
    again.disabled = true
    pages.replaceChildren()
    status.textContent = `Running, ${runs} samples a page, on the ${thread}'s thread…`
    if (thread === 'page') {
      run({ runs, steps }, (data) => show(data.page)).then((result) => done({ result }), (error) => done({ error: String(error.stack ?? error) }))
      return
    }
    const worker = new Worker('svg-page.js')
    worker.onmessage = ({ data }) => {
      if (data.page) return show(data.page)
      worker.terminate()
      done(data)
    }
    worker.onerror = (event) => {
      status.textContent = 'Failed.'
      say(`worker: ${event.message}`)
    }
    worker.postMessage({ runs, steps })
  }

  again.addEventListener('click', begin)
  begin()
}
