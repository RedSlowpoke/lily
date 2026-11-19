// 00 §6 through Lily Studio's own page (MS7 step 10, K7): serve.mjs --studio
// puts this script first in the head of the studio's page, which D55's server
// serves behind it, when the page's query has `measure`. It runs before
// engine.js and wraps the Worker the engine starts, so it sees each message
// to the engine and each answer as the page's EngineHost does, without
// changing the app. Then, once the page has loaded:
//
//   B5a / B5c   navigation start → the engine's `ready` reaching the page;
//               the worker's own start (`ready`'s ms) beside it
//   Cache       what Cache Storage holds, `persisted()` and the estimate
//   B5b         opening T02 from the file tree → its first page in the DOM
//               (the server's compile: opening a score stays the server's,
//               D172), then the engine's first compile of it in this
//               worker → the page it changed (`firstEngine`)
//   B6          T02, T12, T24 and T29 × E1–E3 × `runs` (the first dropped,
//               00 §6.1), each made as the editor makes one, through
//               `window.studio.edited`, so live.ts compiles it with the
//               engine as typing would, with no debounce (00 §6.2): the call
//               → the first change of the preview's pages, then the frame
//               after it; the compile's own time (the message posted → its
//               answer) and every page in beside it. A run first goes back
//               to the text as saved (not timed), as b6.html.
//   B7          the longest main-thread task from the edit to every page in
//               (Long Tasks API); where there is none (Safari), the longest
//               gap a MessageChannel loop sees, an upper bound
//   B8          `b8` edits each stopped 20 ms after its compile was posted
//               by the text as saved: that post → the worker's `superseded`
//               for the first (or its `finished`, when it was done first)
//
// `scores=big` opens T29 × 26 and makes its one edit: the engine's
// diagnostic and the server's compile in its place (D257's first hard item,
// D172). The result is POSTed to /result (serve.mjs adds what it served the
// page) and left in window.spikeResult for measure.mjs.
//
//   /?measure=1&scores=T02,T12,T24,T29&runs=11&b8=5&label=…
//   /?measure=1&scores=&label=…    the start and Cache Storage only
//   …&forget                         Cache Storage emptied after the result
;(() => {
  const params = new URLSearchParams(location.search)
  if (!params.has('measure')) return
  const label = params.get('label') ?? navigator.userAgent
  const scores = (params.get('scores') ?? 'T02,T12,T24,T29').split(',').filter(Boolean)
  const runs = Number(params.get('runs') ?? 11)
  const b8Trials = Number(params.get('b8') ?? 5)
  const settleMs = Number(params.get('settle') ?? 1500)
  const now = () => performance.now()
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const frame = () => new Promise((resolve) => requestAnimationFrame(() => resolve(now())))
  const result = { label, harness: 'studio-page', userAgent: navigator.userAgent, href: location.href, errors: [] }
  window.addEventListener('error', (e) => result.errors.push(String(e.message).slice(0, 300)))
  window.addEventListener('unhandledrejection', (e) => result.errors.push(String(e.reason).slice(0, 300)))

  // --- the engine's worker, seen as EngineHost sees it ------------------------
  const engine = { readyAt: undefined, readyMs: undefined, posts: [], answers: new Map(), performed: [], needsFile: 0, failed: undefined }
  const waiters = []
  const notify = () => waiters.splice(0).forEach((w) => w())
  const changed = () => new Promise((resolve) => waiters.push(resolve))
  const RealWorker = window.Worker
  window.Worker = class extends RealWorker {
    constructor(url, options) {
      super(url, options)
      if (!String(url).includes('engine.worker.js')) return
      this.addEventListener('message', ({ data }) => {
        const at = now()
        if (data.type === 'ready') Object.assign(engine, { readyAt: at, readyMs: data.ms })
        else if (data.type === 'needs-file') engine.needsFile++
        else if (data.type === 'failed') engine.failed = data.message
        else if (data.type === 'performed') engine.performed.push({ revision: data.revision, at })
        else if (data.revision !== undefined) {
          const answer = engine.answers.get(data.revision) ?? {}
          if (data.type === 'cells') answer.cellsAt ??= at
          else answer[data.type] = at
          if (data.type === 'finished') answer.state = data.outcome?.state
          if (data.type === 'finished') answer.svgs = data.outcome?.svg?.map((t) => t.length)
          if (data.type === 'finished') answer.message = data.outcome?.diagnostics?.find((d) => d.severity === 'error')?.message
          engine.answers.set(data.revision, answer)
        }
        notify()
      })
      const post = this.postMessage.bind(this)
      this.postMessage = (message, transfer) => {
        if (message?.type === 'compile') engine.posts.push({ revision: message.request.revision, root: message.request.rootFile, at: now() })
        notify()
        return post(message, transfer)
      }
    }
  }
  const until = async (test, ms, what) => {
    const end = now() + ms
    for (;;) {
      const value = test()
      if (value) return value
      if (now() > end) throw new Error(`timed out waiting for ${what}`)
      await Promise.race([changed(), sleep(50)])
    }
  }

  // --- the main thread's long tasks (B7) ---------------------------------------
  const longTasks = []
  const hasLongTasks = PerformanceObserver.supportedEntryTypes?.includes('longtask')
  if (hasLongTasks) new PerformanceObserver((list) => longTasks.push(...list.getEntries().map((e) => ({ start: e.startTime, ms: e.duration })))).observe({ type: 'longtask', buffered: true })
  /** Where there are no long tasks: the longest gap between turns of a MessageChannel loop, from `start()` to `stop()`. */
  function gapProbe() {
    const channel = new MessageChannel()
    let last = now()
    let longest = 0
    let on = true
    channel.port1.onmessage = () => {
      const t = now()
      longest = Math.max(longest, t - last)
      last = t
      if (on) setTimeout(() => channel.port2.postMessage(0), 0)
    }
    channel.port2.postMessage(0)
    return () => ((on = false), channel.port1.close(), longest)
  }

  // --- the preview's pages ------------------------------------------------------
  let mutations = []
  // A page's nodes, not its attributes: the preview marks its pages as an edit begins (D58), before any compile. Cells
  // that go in hidden are no change until the frame that shows them (D176), which takes their `visibility` off.
  const hidden = (node) => node.nodeType === 1 && node.getAttribute('visibility') === 'hidden'
  const observer = new MutationObserver((records) => {
    const seen = (r) => (r.type === 'attributes' ? r.attributeName === 'visibility' : ![...r.addedNodes].every(hidden) || r.removedNodes.length > 0)
    if (records.some(seen)) mutations.push(now())
  })
  const pagesOf = () => document.querySelector('[data-pane="preview"] .preview-pages')
  const observe = () => {
    const pages = pagesOf()
    if (pages && !pages.dataset.measured) {
      observer.observe(pages, { childList: true, subtree: true, attributes: true, characterData: true })
      pages.dataset.measured = ''
    }
  }
  /** Waits until the preview has not changed for two frames and `settleMs`, with nothing pending in the worker. */
  async function settled(since) {
    for (;;) {
      const count = mutations.length
      await frame()
      await frame()
      await sleep(settleMs)
      const last = engine.posts.at(-1)
      const answered = !last || engine.answers.get(last.revision)?.finished || engine.answers.get(last.revision)?.superseded
      if (mutations.length === count && answered) return mutations.filter((t) => t >= since).at(-1)
    }
  }

  // --- the studio's UI ----------------------------------------------------------
  const click = (element) => element.dispatchEvent(new MouseEvent('click', { bubbles: true }))
  const byText = (selector, text) => [...document.querySelectorAll(selector)].find((e) => e.textContent.trim() === text)
  async function openFolder() {
    if (document.querySelector('li.file[data-file]')) return
    const open = await until(() => byText('button', 'Open Folder…'), 30_000, 'Open Folder…')
    click(open)
    const dialog = await until(() => document.querySelector('.web-browser'), 10_000, 'the folder dialog')
    await until(() => dialog.querySelector('[role="option"], .web-empty'), 10_000, 'the folder listing')
    click(dialog.querySelector('button.primary'))
    await until(() => document.querySelector('li.file[data-file]'), 20_000, 'the file tree')
  }
  const fileOf = (id) => [...document.querySelectorAll('li.file[data-file]')].find((e) => e.dataset.file.endsWith(`/${id}.ly`))
  const pageText = () => [...document.querySelectorAll('[data-pane="preview"] .preview-page svg')].map((s) => s.outerHTML.length).join(',')

  /** Opens `id` from the file tree: the click → its first page in the DOM, and the frame after. */
  async function openScore(id) {
    const item = await until(() => fileOf(id), 10_000, `${id}.ly in the file tree`)
    const before = pageText()
    const clicked = now()
    click(item)
    // Its own pages, from its own compile: the status says so once it has finished.
    await until(() => document.querySelector('[data-pane="preview"] .preview-page svg') && pageText() !== before &&
      document.querySelector('.status-compile')?.dataset.state === 'ok', 300_000, `${id}'s first page`)
    const shown = now()
    const painted = await frame()
    observe()
    await settled(clicked)
    return { file: item.dataset.file, shownMs: shown - clicked, paintedMs: painted - clicked }
  }

  /** The text the studio was last given of each file: live.ts compiles nothing for the same text again. */
  const given = new Map()
  /** One edit as the editor makes it: the text to the studio, timed until the preview changes. */
  async function edit(file, text, { timed = true } = {}) {
    if (given.get(file) === text) return undefined
    given.set(file, text)
    mutations = []
    observe()
    const from = longTasks.length
    const stop = hasLongTasks ? undefined : gapProbe()
    const posted = engine.posts.length
    const at = now()
    window.studio.edited(file, text)
    const post = await until(() => engine.posts[posted], 10_000, 'the compile to be posted').catch(() => undefined)
    if (!post) return { at, noCompile: true }
    const answer = await until(() => {
      const a = engine.answers.get(post.revision)
      return (a?.finished || a?.superseded) && a
    }, 300_000, `revision ${post.revision}'s answer`)
    const first = await until(() => mutations.find((t) => t >= at), 30_000, 'the page to change').catch(() => undefined)
    const painted = first === undefined ? undefined : await frame()
    const last = await settled(at)
    // The page's music, after it (M7): its outcome shown again is not the edit's.
    const performedAt = engine.performed.find((p) => p.revision === post.revision)?.at
    const allIn = performedAt === undefined ? last : mutations.filter((t) => t >= at && t <= performedAt).at(-1)
    const gap = stop?.()
    if (!timed) return undefined
    const tasks = longTasks.slice(from).filter((t) => t.start + t.ms >= at)
    return {
      revision: post.revision,
      root: post.root.split('/').pop(),
      pages: document.querySelectorAll('[data-pane="preview"] .preview-page').length,
      state: answer.state,
      outcomePages: answer.svgs,
      message: answer.message,
      // B6: the edit → the changed page in the DOM, then the frame after it.
      shownMs: first === undefined ? null : first - at,
      paintedMs: painted === undefined ? null : painted - at,
      // The worker: posted → the first cells, the outcome.
      postMs: post.at - at,
      compileMs: (answer.finished ?? answer.superseded) - post.at,
      cellsMs: answer.cellsAt === undefined ? null : answer.cellsAt - post.at,
      // Every page in, from the edit.
      allInMs: allIn === undefined ? null : allIn - at,
      longestTaskMs: hasLongTasks ? Math.max(0, ...tasks.map((t) => t.ms)) : null,
      longTasks: hasLongTasks ? tasks.length : null,
      longestGapMs: gap ?? null,
    }
  }

  /** B8: an edit, then 20 ms after its compile was posted the text as saved; the wait for the first's answer. */
  async function b8(file, text, original) {
    mutations = []
    observe()
    const posted = engine.posts.length
    given.set(file, original)
    window.studio.edited(file, text)
    const first = await until(() => engine.posts[posted], 10_000, 'the compile to be posted')
    await sleep(Math.max(0, first.at + 20 - now()))
    const sent = now()
    window.studio.edited(file, original)
    const second = await until(() => engine.posts[posted + 1], 10_000, 'the second compile to be posted')
    await until(() => engine.answers.get(second.revision)?.finished || engine.answers.get(second.revision)?.superseded, 300_000, 'the second compile')
    await settled(sent)
    // A compile the worker had not begun is never answered (host.ts): no wait to count.
    const a = engine.answers.get(first.revision) ?? {}
    const answered = a.superseded ?? a.finished
    return {
      stopped: Boolean(a.superseded),
      // From the newer edit's post to the older compile's answer: what B8 counts.
      waitMs: answered === undefined ? null : answered - second.at,
      intoMs: second.at - first.at,
    }
  }

  async function caches_() {
    const out = {}
    try {
      if (typeof caches === 'undefined') return { none: true }
      for (const name of await caches.keys()) out[name] = (await (await caches.open(name)).keys()).length
      out.persisted = await navigator.storage?.persisted?.()
      const estimate = await navigator.storage?.estimate?.()
      out.usage = estimate?.usage
    } catch (error) {
      out.error = String(error)
    }
    return out
  }

  async function run() {
    const nav = performance.getEntriesByType('navigation')[0]
    result.navigation = nav && { responseEnd: nav.responseEnd, domContentLoaded: nav.domContentLoadedEventEnd, load: nav.loadEventEnd, transferSize: nav.transferSize }
    await until(() => engine.readyAt !== undefined || engine.failed, 120_000, 'the engine to be ready')
    if (engine.failed) throw new Error(`the engine failed: ${engine.failed}`)
    // B5a / B5c: navigation start (timeOrigin) → ready in the page.
    result.readyMs = engine.readyAt
    result.workerReadyMs = engine.readyMs
    result.engine = Boolean(window.lilyEngine)
    result.cacheStorage = await caches_()
    if (!scores.length) return
    await openFolder()
    result.scores = {}
    for (const id of scores) {
      const r = (result.scores[id] = { runs: {} })
      const edits = await (await fetch(`/b6/${id}.json`)).json()
      r.open = await openScore(id)
      const original = await (await fetch(`/translate/${id}/${id}.ly`)).text()
      // The engine's first compile of this score in this worker (B5b's engine half).
      r.firstEngine = await edit(r.open.file, original)
      for (const { name, source } of edits.edits) {
        r.runs[name] = []
        for (let i = 0; i < runs; i++) {
          await edit(r.open.file, original, { timed: false })
          r.runs[name].push(await edit(r.open.file, source))
        }
      }
      await edit(r.open.file, original, { timed: false })
      if (b8Trials && edits.edits.length) {
        r.b8 = []
        for (let i = 0; i < b8Trials; i++) r.b8.push(await b8(r.open.file, edits.edits[0].source, original))
      }
    }
    result.performed = engine.performed.length
    result.needsFile = engine.needsFile
  }

  window.addEventListener('load', () => {
    run()
      .catch((error) => (result.failed = String(error?.stack ?? error).slice(0, 1000)))
      .then(async () => {
        result.longTasksAll = longTasks.length
        result.hasLongTasks = hasLongTasks
        try {
          await fetch('/result', { method: 'POST', body: JSON.stringify(result) })
        } catch (error) {
          result.posted = String(error)
        }
        // `forget`: this origin's Cache Storage emptied after the result, so a device is left as found (iOS has no DevTools to do it).
        if (params.has('forget') && typeof caches !== 'undefined') {
          for (const name of await caches.keys().catch(() => [])) await caches.delete(name).catch(() => {})
        }
        window.spikeResult = result
      })
  })
})()
