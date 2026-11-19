// translate.html's runner, for the budgets of parts 3 and 4 on the devices
// (progress.md, MS2 › The plan, steps 28 and 42): the first full parse of
// T02 and T29 and the translation of the 30 scores (part 3's 14 until MS2
// step 40), through the built core.wasm's
// chapter 08 §4.2 ABI as the studio's driver runs it, in a worker. This
// file is both the page's script and, loaded by `new Worker`, the worker's.
//
//   /translate.html?runs=5         runs of each, each in a fresh instance (default 5)
//   /translate.html?label=…        names the device or browser in the result
//   /translate.html?core=names     runs target/engine/core.names.wasm, the same
//                                  core with its name section, for a profiler's
//                                  function names (MS3 step 36)
//   /translate.html?scores=T12,T29     compiles and times only these of the 30, for
//                                  a profile of one score (MS3 step 36); `path`
//                                  among them adds tools/compare's path.ly
//   /translate.html?packs=none     puts no font pack, as before MS3 step 36
//   /translate.html?packs=preload,emmentaler-16   the default: the font packs put
//                                  in each instance after lily_start, `preload`
//                                  the manifest's (D105), as the studio's host
//                                  puts them, and any other by name from
//                                  dist/engine/fonts, as the compile example's
//                                  --pack (T23's emmentaler-16, which no build
//                                  preloads; D132)
//
// The core is /scheme/core.wasm started from /scheme/core.image (serve.mjs),
// the packs /scheme/fonts/<name>.pack and the manifest /scheme/manifest.json.
// A score is put as <ID>/<ID>.ly and served from the corpus as
// /translate/<ID>/<ID>.ly; what the engine asks for (lily_needs) is served
// as /translate/<name>, LilyPond's own files (<datadir>/…) as
// /translate/datadir/…, and one the server lacks is put as none; a font
// pack it asks for (MODULE, W5) as /scheme/fonts/<name>.pack, put as none
// when the server does not send it (serve.mjs --offline), and a part's module
// (I2, MS4 step 20) as the manifest's file, /scheme/<name>.part. A first
// pass compiles all the scores in one instance and fetches what they ask
// for; after it every answer comes from memory, so no fetch is timed.
//
// The first full parse (T02 40 / 10 ms, T29 150 / 80 ms on Android): in a
// fresh instance, the milliseconds from lily_compile until lily_step is
// done, the answers to the engine's asks included (a part's files stop the
// parse, which runs again once they are put: T29 parses twice since MS2
// step 30), less the translations that follow it, which lsb_times gives.
// The translation of a score (00's classes: S 60 / 20 ms for T01–T23, L
// 250 / 100 ms for T24–T30): lsb_times' translations, its \layout's and
// its \midi's (every compile translates both since MS2 step 40, D99), from
// frozen music to the playback map's events, the 30 compiled one after
// another in one instance as the studio compiles them, a fresh instance
// each run. Each is the median of
// the runs, with the largest beside it. Since MS3 step 36 the layout
// apart from the writer (progress.md, *How layout per grob is measured*):
// of each score's runs, the medians of lsb_times' layout and breaking of
// its Paper_scores (within its translations, D130) and its pages'
// stencils, which together are the layout, of the SVG writer, and the
// live grobs laid out, with the layout's µs a live grob (R1's 120 / 40 on
// the Android device) for a score whose pages were made: one whose layout
// stopped (a placeholder, a font not put) counts grobs a whole layout
// would kill. Since MS5 step 7 the performance and file after an edit
// (README §4.4: T29 300 / 100 ms): of the same compiles, each score's
// \midi translations and its MIDI files' writing (lsb_times' last two), in
// the instance warm from the scores before it as after an edit; logged and
// in the result. Since MS4 step 6 (OPEN D7) the linear memory of each
// run's instance split: after lily_start and its packs (`image`), then of
// each compile's growth the part by the end of its translations
// (`translation`: the music, contexts and grobs), by the end of its pages
// (`pages`: breaking, stencils and the SVG) and after (`rest`: the
// outputs, the MIDI files), by lsb_times' two byte counts; the run whose
// memory is the largest is kept, beside `memory`. Since MS4 step 18, B5b
// (00 §2: engine ready → T02's first page in the DOM, 800 / 400 ms on
// Android): in a fresh instance each run, from the end of lily_start and
// its packs (the engine ready) to the end of the page's first
// requestAnimationFrame callback after T02's first page was put in the
// page's #render (00 §6.1), the clocks of the worker and the page compared
// through their timeOrigins; the files from memory, as the parses'. The result is shown, POSTed to
// /result (serve.mjs) and left in window.spikeResult, where measure.mjs
// waits for it.
// The core's split modules, joined to each instance (core-split.js, K4).
if (typeof importScripts === 'function') importScripts('core-split.js')

const FIRST = { T02: [40, 10], T29: [150, 80] }
/** The 30 scores of corpus.tsv, T01–T30 (chapter 00 §4). */
const SCORES = Array.from({ length: 30 }, (_, i) => `T${String(i + 1).padStart(2, '0')}`)
/** Scores timed only when ?scores= names them: tools/compare's path.ly, which lays out whole (MS3 step 35). */
const EXTRA = ['path']
/** Chapter 00's classes, and the translation's Must and Goal for each. */
const scoreClass = (id) => (Number(id.slice(1)) >= 24 ? 'L' : 'S')
const TRANSLATION = { S: [60, 20], L: [250, 100] }
/** Layout per live grob, µs, Must / Goal on the Android device (README §5, MS3; R1). */
const LAYOUT = [120, 40]
/** The performance and file after an edit, ms, Must / Goal (README §4.4): T29's. */
const PERFORMANCE = { T29: [300, 100] }
/** B5b, the first render, ms, Must / Goal on Android (00 §2): T02's. */
const RENDER = { T02: [800, 400] }

if (typeof document === 'undefined') {
  // ---- The worker ----
  // protocol.ts' Step and Need.
  const DONE = 0
  const MORE = 1
  const FILE = 0
  const MODULE = 1
  const DATADIR = '<datadir>'
  const utf8 = new TextEncoder()
  const median = (values) => [...values].sort((a, b) => a - b)[values.length >> 1]
  const round = (ms) => +ms.toFixed(2)
  const root = (id) => `${id}/${id}.ly`
  const url = (name) => (name.startsWith(`${DATADIR}/`) ? `translate/datadir/${name.slice(DATADIR.length + 1)}` : `translate/${name}`)
    .split('/').map(encodeURIComponent).join('/')

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

  /** A fresh instance of the core started from `image`, the font packs `packs` ([name, bytes] each) put. */
  async function start(mod, image, packs) {
    const { exports: e } = await instantiateCore(mod)
    /** Calls `f` with each of `buffers` in the core's memory as (address, length), freed after. */
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
    const t = performance.now()
    const rc = withBytes([image], e.lily_start)
    if (rc !== 0) throw new Error(`lily_start: ${rc}`)
    const startMs = performance.now() - t
    for (const [name, bytes] of packs) withBytes([utf8.encode(name), bytes], (n, nl, d, dl) => e.lily_put(MODULE, n, nl, d, dl))
    return { e, withBytes, startMs, out: e.lily_alloc(16) }
  }

  /**
   * Compiles `id` in `core`, the engine's asks answered from `files` (a
   * name's bytes, or null for none); `{ need }` if it asks for one `files`
   * lacks, else what it took and gave, with its first page's text if
   * `page`.
   */
  function compile(core, id, revision, files, page = false) {
    const { e, withBytes, out } = core
    const name = utf8.encode(root(id))
    withBytes([name, files.get(root(id))], (n, nl, d, dl) => e.lily_put(FILE, n, nl, d, dl))
    let asked = 0
    const t = performance.now()
    withBytes([name], (n, nl) => e.lily_compile(revision, n, nl, 0))
    for (;;) {
      const step = e.lily_step(10)
      if (step === DONE) break
      if (step === MORE) continue
      e.lily_needs(out)
      const [kind, at, length] = new Uint32Array(e.memory.buffer, out, 3)
      const need = new TextDecoder().decode(new Uint8Array(e.memory.buffer, at, length))
      if (kind !== FILE && kind !== MODULE) throw new Error(`${id}: the engine needs ${need} of kind ${kind}`)
      const key = kind === MODULE ? `module:${need}` : need
      if (!files.has(key)) return { need: key }
      if (kind === FILE) asked++
      withBytes([utf8.encode(need), files.get(key)], (n, nl, d, dl) => e.lily_put(kind, n, nl, d, dl))
      withBytes([name], (n, nl) => e.lily_compile(revision, n, nl, 0))
    }
    const ms = performance.now() - t
    const [parseMs, translateMs, translations, longestMs, layOutMs, stencilsMs, writeMs, live, performMs, midiMs,
      translatedBytes, pagedBytes] = new Float64Array(e.memory.buffer, e.lsb_times(), 12)
    const outputs = {}
    let first
    for (let i = 0; i < e.lily_outputs(); i++) {
      e.lily_output(i, out)
      const [n, nl, d, dl] = new Uint32Array(e.memory.buffer, out, 4)
      const named = new TextDecoder().decode(new Uint8Array(e.memory.buffer, n, nl))
      outputs[named] = dl
      if (page && (named === `${id}.svg` || named === `${id}-1.svg`)) first = new TextDecoder().decode(new Uint8Array(e.memory.buffer, d, dl))
    }
    return { ms, asked, parseMs, translateMs, translations, longestMs, layOutMs, stencilsMs, writeMs, live, performMs, midiMs,
      translatedBytes, pagedBytes, memory: e.memory.buffer.byteLength, pages: Object.keys(outputs).filter((n) => n.endsWith('.svg')).length, map: outputs[`${id}.timing.json`] ?? 0, first }
  }

  /** The page's answer to a first render: resolved with its milliseconds. */
  let answer
  /** `svg` put in the page's DOM, the engine ready at `ready` (the epoch's ms): the milliseconds to the end of the frame. */
  function rendered(ready, svg) {
    postMessage({ page: svg, ready })
    return new Promise((resolve) => (answer = resolve))
  }

  async function fetchBytes(name) {
    const module = name.startsWith('module:') && name.slice('module:'.length)
    const { modules } = module ? await (await fetch('scheme/manifest.json')).json() : {}
    const file = module && (modules[module]?.kind === 'part' ? `scheme/${encodeURIComponent(modules[module].file)}`
      : `scheme/fonts/${encodeURIComponent(module)}.pack`)
    const response = await fetch(file || url(name))
    return response.ok ? new Uint8Array(await response.arrayBuffer()) : null
  }

  /** The font packs named by `asked` ([name, bytes] each), `preload` the manifest's. */
  async function fetchPacks(asked) {
    const names = []
    for (const name of asked) {
      if (name === 'preload') names.push(...(await (await fetch('scheme/manifest.json')).json()).core.preload)
      else names.push(name)
    }
    return Promise.all([...new Set(names)].map(async (name) => {
      const response = await fetch(`scheme/fonts/${encodeURIComponent(name)}.pack`)
      if (!response.ok) throw new Error(`no font pack ${name}: ${response.status}`)
      return [name, new Uint8Array(await response.arrayBuffer())]
    }))
  }

  async function run(runs, core, only, asked) {
    const timed = only ? [...SCORES, ...EXTRA].filter((id) => only.includes(id)) : SCORES
    let t = performance.now()
    const mod = await WebAssembly.compile(await (await fetch(`scheme/${core}`)).arrayBuffer())
    const image = new Uint8Array(await (await fetch('scheme/core.image')).arrayBuffer())
    const packs = await fetchPacks(asked)
    const result = { runs, core, ...(only && { only }), packs: packs.map(([name]) => name),
      packBytes: packs.reduce((n, [, b]) => n + b.length, 0), clockStepMs: +clockStep().toFixed(4),
      loadMs: round(performance.now() - t), imageBytes: image.length, scores: {}, parses: {}, translations: {}, layouts: {}, performances: {} }

    // Every score once, fetching what it asks for: a file fetched for an
    // earlier score (a part's) is not asked for again in this instance.
    const files = new Map()
    const scores = [...new Set([...Object.keys(FIRST), ...timed])]
    const first = await start(mod, image, packs)
    for (const [i, id] of scores.entries()) {
      files.set(root(id), await fetchBytes(root(id)))
      if (!files.get(root(id))) {
        result.scores[id] = { error: `no ${url(root(id))}: fetch the corpus (npm run corpus)` }
        postMessage({ progress: `${id}: not served` })
        continue
      }
      let done
      let fetched = 0
      while ((done = compile(first, id, i + 1, files)).need) {
        files.set(done.need, await fetchBytes(done.need))
        fetched++
      }
      result.scores[id] = { fetched, map: done.map, translations: done.translations, live: done.live, pages: done.pages }
      postMessage({ progress: `${id}: ${fetched} files fetched, a map of ${done.map} bytes, ${done.live} live grobs, ${done.pages} pages` })
    }
    const served = (id) => !result.scores[id].error

    for (const [id, [must, goal]] of Object.entries(FIRST)) {
      if (!served(id)) continue
      const done = []
      for (let r = 0; r < runs; r++) {
        const core = await start(mod, image, packs)
        const c = compile(core, id, 1, files)
        done.push({ ms: c.ms - c.translateMs, last: c.parseMs, asked: c.asked, startMs: core.startMs })
      }
      const ms = median(done.map((d) => d.ms))
      result.parses[id] = { ms: round(ms), max: round(Math.max(...done.map((d) => d.ms))), asked: done[0].asked,
        lastParseMs: round(median(done.map((d) => d.last))), startMs: round(median(done.map((d) => d.startMs))),
        all: done.map((d) => round(d.ms)), must, goal }
      postMessage({ parse: id, row: result.parses[id] })
    }

    result.renders = {}
    for (const [id, [must, goal]] of Object.entries(RENDER)) {
      if (!served(id)) continue
      const done = []
      for (let r = 0; r < runs; r++) {
        const core = await start(mod, image, packs)
        const ready = performance.timeOrigin + performance.now()
        const c = compile(core, id, 1, files, true)
        if (!c.first) throw new Error(`${id}: no first page`)
        done.push({ ms: await rendered(ready, c.first), compileMs: c.ms })
      }
      const ms = median(done.map((d) => d.ms))
      result.renders[id] = { ms: round(ms), max: round(Math.max(...done.map((d) => d.ms))),
        compileMs: round(median(done.map((d) => d.compileMs))), all: done.map((d) => round(d.ms)), must, goal }
      postMessage({ render: id, row: result.renders[id] })
    }

    const times = Object.fromEntries(timed.map((id) => [id, []]))
    const layouts = Object.fromEntries(timed.map((id) => [id, []]))
    let memory = 0
    let split = null
    for (let r = 0; r < runs; r++) {
      const core = await start(mod, image, packs)
      const parts = { image: core.e.memory.buffer.byteLength, translation: 0, pages: 0, rest: 0 }
      let before = parts.image
      for (const [i, id] of timed.entries()) {
        if (!served(id)) continue
        const c = compile(core, id, i + 1, files)
        times[id].push(c.translateMs)
        layouts[id].push(c)
        // Each phase's growth of the high-water mark.
        const translated = Math.max(before, c.translatedBytes)
        const paged = Math.max(translated, c.pagedBytes)
        parts.translation += translated - before
        parts.pages += paged - translated
        parts.rest += c.memory - paged
        before = c.memory
      }
      if (core.e.memory.buffer.byteLength >= memory) split = parts
      memory = Math.max(memory, core.e.memory.buffer.byteLength)
    }
    for (const id of timed) {
      if (!served(id)) continue
      const [must, goal] = TRANSLATION[scoreClass(id)]
      result.translations[id] = { class: scoreClass(id), ms: round(median(times[id])), max: round(Math.max(...times[id])),
        all: times[id].map(round), must, goal }
      postMessage({ translation: id, row: result.translations[id] })
    }
    for (const id of timed) {
      if (!served(id)) continue
      const of = (key) => round(median(layouts[id].map((c) => c[key])))
      const layout = layouts[id].map((c) => c.layOutMs + c.stencilsMs)
      const { live, pages } = layouts[id][0]
      const ms = median(layout)
      result.layouts[id] = { live, ms: round(ms), max: round(Math.max(...layout)), layOutMs: of('layOutMs'), stencilsMs: of('stencilsMs'),
        writeMs: of('writeMs'), pages, usPerLive: live && pages ? +(ms * 1000 / live).toFixed(1) : null,
        all: layout.map(round), must: LAYOUT[0], goal: LAYOUT[1] }
      postMessage({ layout: id, row: result.layouts[id] })
    }
    for (const id of timed) {
      if (!served(id)) continue
      const of = (key) => round(median(layouts[id].map((c) => c[key])))
      const both = layouts[id].map((c) => c.performMs + c.midiMs)
      const [must, goal] = PERFORMANCE[id] ?? []
      result.performances[id] = { ms: round(median(both)), max: round(Math.max(...both)), performMs: of('performMs'), midiMs: of('midiMs'),
        all: both.map(round), ...(must && { must, goal }) }
      postMessage({ performance: id, row: result.performances[id] })
    }
    result.memory = memory
    result.memorySplit = split
    result.modules = splitLoads
    postMessage({ result })
  }

  onmessage = ({ data }) => {
    if (data.rendered !== undefined) return answer(data.rendered)
    run(data.runs, data.core, data.only, data.packs).catch((error) => postMessage({ error: String(error.stack ?? error) }))
  }
} else {
  // ---- The page ----
  const params = new URLSearchParams(location.search)
  const runs = Math.max(1, Number(params.get('runs') ?? 5) || 5)
  const label = params.get('label') ?? ''
  const core = params.get('core') === 'names' ? 'core.names.wasm' : 'core.wasm'
  const only = params.get('scores')?.split(',')
  const packs = (params.get('packs') ?? 'preload,emmentaler-16').split(',').filter((name) => name && name !== 'none')
  const $ = (id) => document.getElementById(id)
  const [again, status, parses, translations, layouts, renders, render, log] =
    ['again', 'status', 'parses', 'translations', 'layouts', 'renders', 'render', 'log'].map($)
  const say = (line) => (log.textContent += `${(performance.now() / 1000).toFixed(1).padStart(6)} s  ${line}\n`)
  const verdict = (ms, must, goal) => (ms <= goal ? 'within Goal' : ms <= must ? 'within Must' : 'over Must')

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

  function begin() {
    again.disabled = true
    parses.replaceChildren()
    translations.replaceChildren()
    layouts.replaceChildren()
    renders.replaceChildren()
    status.textContent = `Running ${core}, ${runs} runs each, packs ${packs.join(', ') || 'none'}…`
    const worker = new Worker('translate-page.js')
    worker.onmessage = ({ data }) => {
      if (data.progress) return say(data.progress)
      if (data.page !== undefined) {
        // The page's node replaced, then the end of the first frame after it (00 §6.1).
        render.innerHTML = data.page
        return requestAnimationFrame(() => worker.postMessage({ rendered: performance.timeOrigin + performance.now() - data.ready }))
      }
      if (data.render) {
        const b = data.row
        const v = verdict(b.ms, b.must, b.goal)
        renders.append(row([data.render, b.ms, b.max, b.compileMs, `${b.must} / ${b.goal}`, v], v === 'over Must'))
        return say(`${data.render} first render (B5b): ${b.all.join(', ')} ms`)
      }
      if (data.parse) {
        const p = data.row
        const v = verdict(p.ms, p.must, p.goal)
        parses.append(row([data.parse, p.ms, p.asked, `${p.must} / ${p.goal}`, v], v === 'over Must'))
        return say(`${data.parse} first parse: ${p.all.join(', ')} ms`)
      }
      if (data.translation) {
        const t = data.row
        const v = verdict(t.ms, t.must, t.goal)
        translations.append(row([data.translation, t.class, t.ms, t.max, `${t.must} / ${t.goal}`, v], v === 'over Must'))
        return say(`${data.translation} translation: ${t.all.join(', ')} ms`)
      }
      if (data.performance) {
        const p = data.row
        const budget = p.must ? `, ${verdict(p.ms, p.must, p.goal)} (${p.must} / ${p.goal})` : ''
        return say(`${data.performance} performance and file: ${p.ms} ms (\\midi ${p.performMs}, file ${p.midiMs})${budget}`)
      }
      if (data.layout) {
        const l = data.row
        layouts.append(row([data.layout, l.live, l.ms, l.layOutMs, l.stencilsMs, l.writeMs, l.pages, l.usPerLive ?? '–', `${l.must} / ${l.goal}`]))
        return say(`${data.layout} layout: ${l.all.join(', ')} ms, ${l.live} live grobs`)
      }
      worker.terminate()
      again.disabled = false
      if (data.error) {
        status.textContent = 'Failed.'
        return say(data.error)
      }
      const result = { page: 'translate', label, userAgent: navigator.userAgent, ...data.result }
      status.textContent = `core loaded in ${result.loadMs} ms; packs ${result.packs.join(', ') || 'none'}; clock step ${result.clockStepMs} ms; ` +
        `linear memory after the 30 ${(result.memory / 2 ** 20).toFixed(1)} MB (` +
        Object.entries(result.memorySplit ?? {}).map(([k, v]) => `${k} ${(v / 2 ** 20).toFixed(1)}`).join(', ') + ')'
      window.spikeResult = result
      fetch('result', { method: 'POST', body: JSON.stringify(result) })
        .then((r) => say(r.ok ? 'posted to /result' : `/result: ${r.status}`))
        .catch((error) => say(`/result: ${error.message}`))
    }
    worker.onerror = (event) => {
      status.textContent = 'Failed.'
      say(`worker: ${event.message}`)
    }
    worker.postMessage({ runs, core, only, packs })
  }

  again.addEventListener('click', begin)
  begin()
}
