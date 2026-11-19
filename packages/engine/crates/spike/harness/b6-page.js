// B6 on a device (MS6 step 12, README §4.3; 00 §6.2): T02, T12, T24 and T29,
// each edit E1–E3 from the score's own text, 20 runs of each, by phase, in one
// instance of core.wasm in a worker, as the studio's driver runs it: put, then
// lily_compile for the first page and lily_step(10) to the end. An edit's cells
// and pages go to packages/common/web/preview.js on the main thread as the
// studio sends them (W8, D232), so the page phase is the device's own: what was
// the fixture OPEN H14 lacked is made here, by the engine, on the device.
//
//   /b6.html?runs=20&scores=T02,T12,T24,T29&b8=1&label=…
//
// A run is the text compiled (not timed), then the edit; the first of an edit's
// runs is dropped (00 §6.1). tools/bench/b6.mjs is the same measure in Node.
// `b8` adds B8: after those, 5 edits each stopped 20 ms in by the text, the
// wait until the step running ended. `scores=big` is T29 × 26, B8's score.
// A compile that traps ends its score's rows, with `trapped`: B4's cap reached.
// The result is shown, POSTed to /result and left in window.spikeResult.
//
// The phases are lsb_times': parse; translation with its layout taken out;
// layout with the pages' stencils; svg. §4.3's Scheme row is inside them (a
// callback is timed where it runs). `page` runs from the compile's end to the
// changed page in the DOM with its layout forced, the cells or the page sent to
// the main thread and handled there; a page sent whole counts to `rendered`,
// which preview.js says two frames after its insertion, so it is an upper bound.
if (typeof document === 'undefined') {
  const [DONE, MORE, FILE, MODULE] = [0, 1, 0, 1]
  const DATADIR = '<datadir>'
  const utf8 = new TextEncoder()
  const text = new TextDecoder()
  const url = (name) => (name.startsWith(`${DATADIR}/`) ? `translate/datadir/${name.slice(DATADIR.length + 1)}` : `translate/${name}`)
    .split('/').map(encodeURIComponent).join('/')
  importScripts('core-split.js')

  const manifest = () => fetch('scheme/manifest.json').then((r) => r.json())
  async function fetchBytes(name) {
    const module = name.startsWith('module:') && name.slice('module:'.length)
    const { modules } = module ? await manifest() : {}
    const file = module && (modules[module]?.kind === 'part' ? `scheme/${encodeURIComponent(modules[module].file)}`
      : `scheme/fonts/${encodeURIComponent(module)}.pack`)
    const response = await fetch(file || url(name))
    return response.ok ? new Uint8Array(await response.arrayBuffer()) : null
  }

  /** FNV-1a of a page's text: what preview.js compares, as the studio's hash. */
  function hash(s) {
    let h = 0x811c9dc5
    for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 0x01000193)
    return h >>> 0
  }

  /** The main thread's answer to the last page sent. */
  let answer
  const shown = (message, transfer) => {
    postMessage(message, transfer)
    return new Promise((resolve) => (answer = resolve))
  }
  const trial = () => new Promise((resolve) => setTimeout(resolve, 0))

  async function instance(mod, image, packs) {
    const { exports: e } = await instantiateCore(mod)
    const withBytes = (buffers, f) => {
      const args = buffers.flatMap((b) => {
        if (b === null) return [0, 0]
        const at = e.lily_alloc(b.length)
        new Uint8Array(e.memory.buffer, at, b.length).set(b)
        return [at, b.length]
      })
      const result = f(...args)
      for (let i = 0; i < args.length; i += 2) if (args[i]) e.lily_free(args[i], args[i + 1])
      return result
    }
    if (withBytes([image], e.lily_start) !== 0) throw new Error('lily_start')
    for (const [name, bytes] of packs) withBytes([utf8.encode(name), bytes], (n, nl, d, dl) => e.lily_put(MODULE, n, nl, d, dl))
    return { e, withBytes, out: e.lily_alloc(16) }
  }

  /** Files asked for and fetched once, a stop being what a first compile pays, never an edit's run. */
  const files = new Map()
  /** Over all the scores: preview.js drops a render older than the last it saw, `clear` or not. */
  let revision = 1

  /** Puts `bytes` as the root and compiles it; `until(ms)` stops it. The steps' milliseconds, whether it ended. */
  async function compile(core, root, bytes, revision, until = () => false) {
    const { e, withBytes, out } = core
    const name = utf8.encode(root)
    const begin = () => withBytes([name], (n, nl) => e.lily_compile(revision, n, nl, 1))
    const started = performance.now()
    withBytes([name, bytes], (n, nl, d, dl) => e.lily_put(FILE, n, nl, d, dl))
    begin()
    const steps = []
    let stops = 0
    for (;;) {
      if (until(performance.now() - started)) return { steps, done: false, ms: performance.now() - started }
      const at = performance.now()
      const step = e.lily_step(10)
      steps.push(performance.now() - at)
      if (step === DONE) return { steps, done: true, ms: performance.now() - started, stops }
      if (step === MORE) continue
      stops++
      e.lily_needs(out)
      const [kind, where, length] = new Uint32Array(e.memory.buffer, out, 3)
      const need = text.decode(new Uint8Array(e.memory.buffer, where, length).slice())
      const key = kind === MODULE ? `module:${need}` : need
      if (!files.has(key)) files.set(key, await fetchBytes(key))
      withBytes([utf8.encode(need), files.get(key)], (n, nl, d, dl) => e.lily_put(kind, n, nl, d, dl))
      begin()
    }
  }

  /** The compile's pages and cells, in the order of its page numbers, as worker.ts reads them. */
  function outputs({ e, out }, id) {
    const pages = []
    const cells = new Map()
    for (let i = 0; i < e.lily_outputs(); i++) {
      e.lily_output(i, out)
      const [n, nl, d, dl] = new Uint32Array(e.memory.buffer, out, 4)
      const named = text.decode(new Uint8Array(e.memory.buffer, n, nl).slice())
      if (named.endsWith('.svg')) pages.push({ named, svg: text.decode(new Uint8Array(e.memory.buffer, d, dl).slice()) })
      else if (named.endsWith('.cells')) cells.set(named.slice(0, -'.cells'.length), text.decode(new Uint8Array(e.memory.buffer, d, dl).slice()))
    }
    const number = (p) => Number(/-(\d+)\.svg$/.exec(p.named)?.[1] ?? 1)
    pages.sort((a, b) => number(a) - number(b))
    const sent = []
    pages.forEach((p, index) => {
      const file = cells.get(p.named.slice(0, -'.svg'.length))
      if (!file) return
      const line = file.indexOf('\n')
      const list = file.slice(0, line).split(' ').map((c) => ({ digest: c.split(':')[0], y: c.split(':')[1] }))
      sent.push({ page: index, cells: list, markup: file.slice(line + 1), hash: hash(p.svg) })
    })
    return { pages: pages.map((p) => p.svg), hashes: pages.map((p) => hash(p.svg)), sent, id }
  }

  const median = (values) => [...values].sort((a, b) => a - b)[values.length >> 1]
  const round = (x) => +x.toFixed(1)

  async function score(mod, image, packs, id, runs, b8) {
    const { root, source, edits } = await (await fetch(`b6/${id}.json`)).json()
    const core = await instance(mod, image, packs)
    const { e } = core
    const bytes = await fetchBytes(`${id}/${root}`) ?? utf8.encode(source)
    const file = `${id}/${root}`
    const show = async () => {
      const done = performance.timeOrigin + performance.now()
      return shown({ show: outputs(core, id), revision, done })
    }
    const row = { id, rows: [] }
    const send = async (label, body) => {
      const run = await compile(core, file, body, ++revision)
      if (!run.done) throw new Error(`${label} did not end`)
      const values = [...new Float64Array(e.memory.buffer, e.lsb_times(), 12)]
      const page = await show()
      return { run, values, page }
    }
    try {
      const first = await send('text', bytes)
      row.first = { ms: round(first.run.ms), stops: first.run.stops, pages: first.page.pages, MiB: round(e.memory.buffer.byteLength / 2 ** 20) }
      postMessage({ progress: `${id}: first ${row.first.ms} ms, ${row.first.pages} pages, ${row.first.MiB} MiB` })
      for (const edit of edits) {
        const body = utf8.encode(edit.source)
        const kept = []
        for (let k = 0; k <= runs; k++) {
          await send('text', bytes)
          const { run, values, page } = await send(edit.name, body)
          await trial()
          if (k === 0) continue
          const [parse, translate, , , layOut, stencils, svg] = values
          kept.push({ parse, translation: translate - layOut, layout: layOut + stencils, svg, engine: run.ms, longest: Math.max(...run.steps),
            page: page.ms, total: run.ms + page.ms, cells: page.cells, whole: page.whole })
        }
        const r = { id, edit: edit.name, runs: kept.length }
        for (const phase of ['parse', 'translation', 'layout', 'svg', 'engine', 'longest', 'page', 'total']) r[phase] = round(median(kept.map((m) => m[phase])))
        r.totalWorst = round(Math.max(...kept.map((m) => m.total)))
        r.totalAll = kept.map((m) => round(m.total))
        r.cells = kept[kept.length - 1].cells
        r.whole = kept[kept.length - 1].whole
        r.MiB = round(e.memory.buffer.byteLength / 2 ** 20)
        row.rows.push(r)
        postMessage({ progress: JSON.stringify({ ...r, totalAll: undefined }) })
      }
      if (b8) {
        row.b8 = []
        const body = utf8.encode(edits[0].source)
        for (let k = 0; k < 5; k++) {
          await compile(core, file, bytes, ++revision)
          // A newer edit arrives 20 ms into this one: it is seen when the step running ends.
          const run = await compile(core, file, body, ++revision, (at) => at >= 20)
          const waited = run.done ? null : run.ms - 20
          const t = performance.now()
          core.withBytes([utf8.encode(file), bytes], (n, nl, d, dl) => e.lily_put(FILE, n, nl, d, dl))
          row.b8.push({ waited: waited === null ? null : round(waited), stop: round(performance.now() - t) })
          const next = await compile(core, file, bytes, ++revision)
          row.b8[k].next = round(next.ms)
        }
        postMessage({ progress: `${id} B8: ${JSON.stringify(row.b8)}` })
      }
    } catch (error) {
      row.trapped = String(error.message ?? error)
      postMessage({ progress: `${id}: trapped: ${row.trapped}` })
    }
    row.MiB = round(e.memory.buffer.byteLength / 2 ** 20)
    await shown({ clear: true })
    return row
  }

  onmessage = async ({ data }) => {
    if (data.page !== undefined) return answer?.(data.page)
    if (!data.scores) return
    try {
      const t = performance.now()
      const mod = await WebAssembly.compile(await (await fetch('scheme/core.wasm')).arrayBuffer())
      const image = new Uint8Array(await (await fetch('scheme/core.image')).arrayBuffer())
      const names = (await manifest()).core.preload
      const packs = await Promise.all(names.map(async (name) => [name, new Uint8Array(await (await fetch(`scheme/fonts/${encodeURIComponent(name)}.pack`)).arrayBuffer())]))
      const result = { runs: data.runs, loadMs: round(performance.now() - t), scores: [] }
      for (const id of data.scores) {
        files.clear()
        result.scores.push(await score(mod, image, packs, id, data.runs, data.b8))
      }
      postMessage({ result })
    } catch (error) {
      postMessage({ error: String(error.stack ?? error) })
    }
  }
} else {
  // ---- The page ----
  const query = new URLSearchParams(location.search)
  const label = query.get('label') ?? ''
  const log = document.getElementById('b6')
  const lines = []
  const say = (line) => {
    lines.push(line)
    log.textContent = lines.slice(-14).join('\n')
  }
  /** preview.js's `rendered`, for the revision sent last. */
  let rendered
  window.acquireVsCodeApi = () => ({
    postMessage: (m) => { if (m.type === 'rendered') rendered?.(m) },
    getState: () => undefined,
    setState: () => {},
  })
  const give = (data) => dispatchEvent(new MessageEvent('message', { data }))
  const wall = () => performance.timeOrigin + performance.now()

  async function page({ show, revision, done }, worker) {
    const { pages, hashes, sent } = show
    for (const c of sent) give({ type: 'cells', revision, page: c.page, cells: c.cells, markup: c.markup, hash: c.hash })
    document.documentElement.getBoundingClientRect()
    const cellsAt = wall()
    const answer = new Promise((resolve) => (rendered = resolve))
    give({ type: 'render', revision, pages, hashes })
    // preview.js answers two frames on; a frame a hidden tab never paints is no run to count.
    const m = await Promise.race([answer, new Promise((resolve) => setTimeout(() => resolve({ hidden: true }), 10000))])
    // A page sent whole is on screen when `rendered` says; else the cells were, at `cellsAt`.
    const whole = pages.length - (m.reusedPages ?? 0)
    const ms = whole > 0 ? wall() - done : cellsAt - done
    worker.postMessage({ page: { ms, cells: sent.reduce((n, c) => n + c.cells.length, 0), whole, pages: pages.length, hidden: m.hidden } })
  }

  function begin() {
    const runs = Number(query.get('runs') ?? 20)
    const scores = (query.get('scores') ?? 'T02,T12,T24,T29').split(',')
    const worker = new Worker('b6-page.js')
    worker.onmessage = ({ data }) => {
      if (data.show) return page(data, worker)
      if (data.clear) {
        give({ type: 'clear' })
        return worker.postMessage({ page: {} })
      }
      if (data.progress) return say(data.progress)
      worker.terminate()
      if (data.error) return say(`failed: ${data.error}`)
      const result = { page: 'b6', label, userAgent: navigator.userAgent, ...data.result }
      say('done')
      window.spikeResult = result
      fetch('result', { method: 'POST', body: JSON.stringify(result) })
        .then((r) => say(r.ok ? 'posted to /result' : `/result: ${r.status}`))
        .catch((error) => say(`/result: ${error.message}`))
    }
    worker.onerror = (event) => say(`worker: ${event.message}`)
    worker.postMessage({ runs, scores, b8: query.has('b8') })
  }
  addEventListener('load', begin)
}
