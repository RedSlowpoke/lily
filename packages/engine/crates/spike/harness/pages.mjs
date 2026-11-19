// B7 for the preview's page side (H2): the extension's webview script,
// packages/common/web/preview.js, takes the pages of an edit to L532 in
// Playwright's Chromium, and the longest main-thread task while it does is
// read from a DevTools trace (the Long Tasks API sees only tasks over 50 ms).
// L532 is harness.js's score, engraved by the installed lilypond into SVG
// pages as the extension shows them (l532.mjs). Run it from a directory where
// `playwright` is installed, with lilypond on the PATH.
//
//   node <path>/pages.mjs [--preview file] [--cpu rate] [--runs n] [--no-checks]
//
//   --preview   the preview.js to time, default this repository's; an older
//               one (git show <rev>:packages/common/web/preview.js) for a before
//   --cpu       DevTools' CPU throttling rate, default 1 (none)
//   --runs      renders per edit, the first discarded (chapter 00 §6.1); default 11
//   --no-checks times without the checks, which a preview.js before H2 fails
//
// Each run shows L532 (revision 1), then sends the edit (revision 2): `all`
// marks every page changed, as the spike's harness replaced all four at once
// (09 §4.4); E1 and E2 are chapter 00 §6.2's edits with the pages' real
// hashes, so that only the pages that changed are parsed (D25). Before timing,
// it checks the order (the page in view first) and that a newer revision stops
// an older one's insertion.
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { describe, lilypond } from './l532.mjs'

const playwright = createRequire(`${process.cwd()}/`)('playwright')
const value = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`)
  return i > 0 ? process.argv[i + 1] : fallback
}
const here = new URL('.', import.meta.url)
const common = new URL('../../../../common/web/', here)
const previewFile = value('preview', new URL('preview.js', common).pathname)
const cpu = Number(value('cpu', 1))
const runs = Number(value('runs', 11))
const checks = !process.argv.includes('--no-checks')

// ---- L532 and its edits, engraved by LilyPond (l532.mjs) ----

const L532 = lilypond()
const { base, edits } = L532
const changed = (e) => e.hashes.filter((h, i) => h !== base.hashes[i]).length
console.log(describe(L532).join('\n'))
console.log(`preview.js: ${previewFile}; CPU throttling ${cpu}×\n`)

// ---- The webview: preview.html's elements, preview.css, midi.js and preview.js ----

const html = `<!doctype html><html><head><meta charset="utf-8">
<style>${readFileSync(new URL('../../../../vscode/media/preview.css', here), 'utf8')}</style></head>
<body data-colors="paper"><div id="progress" hidden></div><main id="pages"></main><p id="empty"></p><p id="note" hidden></p>
<div id="toolbar"><button id="refresh"></button><span id="pager" hidden><button id="page-previous"></button>
<span id="page-label"></span><button id="page-next"></button></span><button id="zoom-out"></button><button id="zoom-fit"></button>
<button id="zoom-in"></button><button id="midi-play"></button><button id="midi-stop"></button>
<input id="midi-seek" type="range" min="0" max="1000" value="0"><span id="midi-time"></span><span id="midi-bar"></span>
<button id="export-pdf"></button><button id="export-midi"></button></div>
<script>
  window.posted = []
  window.inserted = []
  // Before preview.js's listener: where in the insertions each render arrived.
  addEventListener('message', ({ data }) => data.type === 'render' && inserted.push('r' + data.revision))
  acquireVsCodeApi = () => ({ postMessage: (m) => posted.push(m), getState: () => undefined, setState: () => {} })
  new MutationObserver((records) => {
    for (const r of records) for (const n of r.addedNodes) if (n.querySelector?.('svg')) inserted.push(Number(n.dataset.page) - 1)
  }).observe(document.getElementById('pages'), { childList: true })
</script>
<script>${readFileSync(new URL('midi.js', common), 'utf8')}</script>
<script>${readFileSync(previewFile, 'utf8')}</script>
</body></html>`

const browser = await playwright.chromium.launch()
const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1.5 })

async function open() {
  const page = await context.newPage()
  if (cpu !== 1) await (await context.newCDPSession(page)).send('Emulation.setCPUThrottlingRate', { rate: cpu })
  await page.setContent(html)
  return page
}
const send = (page, revision, { pages, hashes }) =>
  page.evaluate((m) => window.postMessage(m, '*'), { type: 'render', revision, pages, hashes })
const rendered = (page, revision) =>
  page.waitForFunction((r) => posted.find((m) => m.type === 'rendered' && m.revision === r), revision).then((h) => h.jsonValue())
/** Each page's path count, on screen and as expected: the pages are the edit's, in order, and none is a placeholder. */
const sameAs = (page, { pages }) =>
  page.evaluate((texts) => {
    const count = (node) => node.querySelectorAll('path').length
    const shown = [...document.getElementById('pages').children].map(count).join()
    return shown === texts.map((t) => count(new DOMParser().parseFromString(t, 'image/svg+xml'))).join()
  }, pages)

// ---- Checks ----

if (checks) {
  const page = await open()
  await send(page, 1, base)
  await rendered(page, 1)
  // The last page in view: it goes in first, then outwards.
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight))
  await page.evaluate(() => (inserted.length = 0))
  await send(page, 2, edits.all)
  await rendered(page, 2)
  const order = (await page.evaluate(() => inserted)).filter(Number.isInteger)
  const last = base.pages.length - 1
  if (order[0] !== last || order.length !== base.pages.length) throw new Error(`insertion order ${order}, not from page ${last}`)
  if (!(await sameAs(page, edits.all))) throw new Error('the pages on screen are not the render\'s')
  // A newer revision right after an older one: once it arrives, only its pages
  // go in (a slow frame may still let the older one finish), and only it is rendered.
  const newer = { pages: edits.E2.pages, hashes: edits.E2.hashes.map((h) => `again-${h}`) }
  await page.evaluate(() => (inserted.length = 0))
  await page.evaluate(([older, newer]) => {
    window.postMessage({ type: 'render', revision: 3, ...older }, '*')
    window.postMessage({ type: 'render', revision: 4, ...newer }, '*')
  }, [edits.E1, newer])
  await rendered(page, 4)
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))))
  if (await page.evaluate(() => posted.some((m) => m.type === 'rendered' && m.revision === 3))) throw new Error('revision 3 was rendered after revision 4')
  const both = await page.evaluate(() => inserted)
  const after = both.slice(both.indexOf('r4') + 1)
  if (after.length !== newer.pages.length) throw new Error(`revision 3 went on inserting: ${both}`)
  if (!(await sameAs(page, edits.E2))) throw new Error('the pages on screen are not the newer revision\'s')
  console.log(`checks: insertion order ${order.join(', ')} from page ${last + 1} in view; revision 3 then 4: ${both.join(', ')}\n`)
  await page.close()
}

// ---- Timing ----

/** The longest task on the page's main thread in a trace, in ms. */
function longestTask(trace) {
  const events = JSON.parse(trace.toString()).traceEvents
  const main = events.filter((e) => e.name === 'thread_name' && e.args?.name === 'CrRendererMain').map((e) => `${e.pid}:${e.tid}`)
  const tasks = events.filter((e) => e.ph === 'X' && e.name === 'RunTask' && main.includes(`${e.pid}:${e.tid}`))
  return Math.max(0, ...tasks.map((e) => e.dur / 1000))
}
const quantile = (values, q) => {
  const v = [...values].sort((a, b) => a - b)
  return v[Math.min(v.length - 1, Math.ceil(q * v.length) - 1)]
}

console.log('edit   pages  longest task p50 / p95 / max (ms)   rendered p50 (ms)   over 50 ms')
for (const [name, e] of Object.entries(edits)) {
  const longest = []
  const ms = []
  for (let run = 0; run < runs; run++) {
    const page = await open()
    await send(page, 1, base)
    await rendered(page, 1)
    await page.evaluate(() => new Promise((resolve) => setTimeout(resolve, 100)))
    await browser.startTracing(page, { categories: ['disabled-by-default-devtools.timeline', 'toplevel'] })
    await send(page, 2, e)
    const done = await rendered(page, 2)
    await page.evaluate(() => new Promise((resolve) => setTimeout(resolve, 50)))
    const trace = await browser.stopTracing()
    await page.close()
    if (run === 0) continue
    longest.push(longestTask(trace))
    ms.push(done.renderMs)
  }
  const f = (v) => v.toFixed(1)
  console.log(
    `${name.padEnd(6)} ${String(changed(e)).padStart(2)}/${e.pages.length}   ` +
      `${f(quantile(longest, 0.5))} / ${f(quantile(longest, 0.95))} / ${f(Math.max(...longest))}`.padEnd(35) +
      `${f(quantile(ms, 0.5))}`.padEnd(20) +
      `${longest.filter((v) => v > 50).length} of ${longest.length}`,
  )
}
await browser.close()
