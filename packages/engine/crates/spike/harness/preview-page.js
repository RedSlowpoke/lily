// preview.html's side of the webview's host, for B7 on a device: L532's pages
// (l532.mjs, served as /l532/<set>.json) go to packages/common/web/preview.js
// as the extension sends them, revision 1 first, then an edit on a button, as
// pages.mjs does in Playwright. Time an edit with remote DevTools' Performance
// panel: record, press it, stop at "rendered", and read the longest task on
// the main thread. Each edit is marked in the trace's Timings ("E1 #2" from
// the message to `rendered`). Where the Long Tasks API exists (Chromium), the
// page also says which tasks over 50 ms the edit took.
//
//   /preview.html?set=lilypond   LilyPond's 2 pages (the default)
//   /preview.html?set=spike      the spike's 4
//
// An edit is sent from revision 1 only: after one, "Revision 1" shows it
// again, which parses the pages that edit changed, and enables the edits.

;(() => {
  const set = new URLSearchParams(location.search).get('set') === 'spike' ? 'spike' : 'lilypond'
  const buttons = ['base', 'E1', 'E2', 'all'].map((id) => document.getElementById(id))
  const log = document.getElementById('log')
  const lines = []
  const say = (line) => {
    lines.push(line)
    log.textContent = lines.slice(-6).join('\n')
  }

  let fixture
  let revision = 0
  /** What is on screen: 'base' or an edit's name, and its hashes. */
  let shown
  let hashes = []
  /** The render in flight: its name, mark and when it was sent. */
  let sent

  const longTasks = []
  const observed = PerformanceObserver.supportedEntryTypes?.includes('longtask')
  if (observed) new PerformanceObserver((list) => longTasks.push(...list.getEntries())).observe({ type: 'longtask' })

  let ready
  const previewReady = new Promise((resolve) => (ready = resolve))
  // The host's half of preview.js's messages: only `ready` and `rendered` matter here.
  window.acquireVsCodeApi = () => ({
    postMessage: (message) => {
      if (message.type === 'ready') ready()
      else if (message.type === 'rendered' && message.revision === revision) {
        performance.measure(sent.mark, sent.mark)
        // A task that ends the render may still be reported after it.
        setTimeout(() => rendered(message), 100)
      }
    },
    getState: () => undefined,
    setState: () => {},
  })

  function send(name) {
    const { pages, hashes: next } = name === 'base' ? fixture.base : fixture.edits[name]
    const changed = next.filter((h, i) => h !== hashes[i]).length
    revision++
    sent = { name, changed, of: next.length, mark: `${name === 'base' ? 'revision 1' : name} #${revision}`, at: performance.now() }
    for (const b of buttons) b.disabled = true
    performance.mark(sent.mark)
    window.postMessage({ type: 'render', revision, pages, hashes: next }, '*')
    hashes = next
  }

  function rendered({ renderMs, reusedPages }) {
    const over = longTasks.filter((t) => t.startTime >= sent.at).map((t) => t.duration.toFixed(0))
    const tasks = observed ? `tasks over 50 ms: ${over.length ? over.join(', ') + ' ms' : 'none'}` : 'no Long Tasks API here'
    say(`${sent.mark}: ${sent.changed} of ${sent.of} pages changed, ${reusedPages} reused, rendered ${renderMs.toFixed(0)} ms; ${tasks}`)
    shown = sent.name
    sent = undefined
    buttons[0].disabled = false
    for (const b of buttons.slice(1)) b.disabled = shown !== 'base'
  }

  for (const b of buttons) b.addEventListener('click', () => send(b.id))
  const select = document.getElementById('set')
  select.value = set
  select.addEventListener('change', () => (location.search = `?set=${select.value}`))

  fetch(`l532/${set}.json`)
    .then((response) => {
      if (!response.ok) throw new Error(`l532/${set}.json: ${response.status}; run l532.mjs after build.sh`)
      return response.json()
    })
    .then(async (f) => {
      fixture = f
      await previewReady
      const kb = (f.base.pages.reduce((n, p) => n + p.length, 0) / 1024).toFixed(0)
      say(`${f.score} by ${f.engraver}: ${f.base.pages.length} pages, ${kb} KB`)
      send('base')
    })
    .catch((error) => say(String(error.message ?? error)))
})()
