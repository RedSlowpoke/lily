// The page phase with W8's cells (MS6 step 10, 06 §6.4): the extension's
// webview script, packages/common/web/preview.js, takes an edit of T02, T12,
// T24 and T29 as cells and as a whole page, on the preview form the engine
// writes, in Playwright's Chromium and WebKit. Run it from a directory where
// `playwright` is installed, after `npm run build`.
//
//   node <path>/cells.mjs [--runs n]
//
//   --runs   renders per edit, the first discarded (chapter 00 §6.1); default 11
//
// Each run shows the page before the edit, then gives it the edit's `cells`
// message or its `render`, and times the message and the layout it forces, on
// the main thread. Before timing, it checks in each browser that the cells
// replaced leave the DOM a whole render of the page leaves, cell for cell.
import { mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { nativeDifferential } from '../../../tools/compare/differential.mjs'
import { e1e3, EDITED } from '../../../tools/compare/edits.mjs'

const playwright = createRequire(`${process.cwd()}/`)('playwright')
const value = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`)
  return i > 0 ? process.argv[i + 1] : fallback
}
const runs = Number(value('runs', 11))
const here = new URL('.', import.meta.url)
const common = new URL('../../../../common/web/', here)

// ---- Each edit's pages sent by their cells ----

const dir = mkdtempSync(path.join(tmpdir(), 'lily-cells-'))
const edits = EDITED.flatMap(e1e3)
const results = await nativeDifferential(edits, dir, { check: false, preview: true })
const fixtures = results.flatMap((r, n) => r.pages.filter((p) => p.replaced).map((p) => {
  const file = (name) => readFileSync(path.join(dir, 'incremental', String(n + 1), name), 'utf8')
  const page = p.page.slice(0, -'.svg'.length)
  const cells = file(`${page}.cells`)
  const line = cells.indexOf('\n')
  return {
    name: `${r.id} ${r.name}`,
    before: file(`${page}.before`),
    after: file(p.page),
    cells: cells.slice(0, line).split(' ').map((c) => ({ digest: c.split(':')[0], y: c.split(':')[1] })),
    markup: cells.slice(line + 1),
    share: `${p.sent}/${p.cells}`,
  }
}))
rmSync(dir, { recursive: true, force: true })

// ---- The webview, as pages.mjs writes it ----

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
  acquireVsCodeApi = () => ({ postMessage: (m) => posted.push(m), getState: () => undefined, setState: () => {} })
  /** Gives preview.js a message now, and the time to it and the layout it forces. */
  window.timed = (data) => {
    const started = performance.now()
    dispatchEvent(new MessageEvent('message', { data }))
    document.documentElement.getBoundingClientRect()
    return performance.now() - started
  }
  /** The cells on screen, each as its markup. */
  window.shown = () => [...document.querySelectorAll('[data-lily-cell]')].map((g) => g.outerHTML)
</script>
<script>${readFileSync(new URL('midi.js', common), 'utf8')}</script>
<script>${readFileSync(new URL('preview.js', common), 'utf8')}</script>
</body></html>`

const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)]

for (const type of ['chromium', 'webkit']) {
  const browser = await playwright[type].launch()
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1.5 })
  const open = async (fixture) => {
    const page = await context.newPage()
    await page.setContent(html)
    await page.evaluate((m) => window.postMessage(m, '*'), { type: 'render', revision: 1, pages: [fixture.before], hashes: ['before'] })
    await page.waitForFunction(() => posted.find((m) => m.type === 'rendered' && m.revision === 1))
    await page.evaluate(() => new Promise((resolve) => setTimeout(resolve, 50)))
    return page
  }
  const asCells = ({ cells, markup }) => ({ type: 'cells', revision: 2, page: 0, cells, markup, hash: 'after' })
  const whole = ({ after }) => ({ type: 'render', revision: 2, pages: [after], hashes: ['after'] })

  console.log(`\n${type} ${browser.version()}\nedit     cells   markup KB   page KB   cells p50 (ms)   whole p50 (ms)`)
  for (const fixture of fixtures) {
    const replaced = await open(fixture)
    await replaced.evaluate((m) => timed(m), asCells(fixture))
    const rendered = await open(fixture)
    await rendered.evaluate((m) => timed(m), whole(fixture))
    const [a, b] = [await replaced.evaluate(() => shown()), await rendered.evaluate(() => shown())]
    if (a.join('\n') !== b.join('\n')) {
      const at = a.findIndex((cell, i) => cell !== b[i])
      throw new Error(`${type} ${fixture.name}: cell ${at} of ${a.length} is not a whole render's: ${a[at]?.slice(0, 200)} | ${b[at]?.slice(0, 200)}`)
    }
    await replaced.close()
    await rendered.close()

    const time = async (message) => {
      const times = []
      for (let run = 0; run < runs; run++) {
        const page = await open(fixture)
        const ms = await page.evaluate((m) => timed(m), message)
        await page.close()
        if (run > 0) times.push(ms)
      }
      return median(times).toFixed(1)
    }
    const kb = (text) => (text.length / 1024).toFixed(0).padStart(6)
    console.log(`${fixture.name.padEnd(8)} ${fixture.share.padEnd(7)} ${kb(fixture.markup)}      ${kb(fixture.after)}    ${(await time(asCells(fixture))).padStart(8)}         ${(await time(whole(fixture))).padStart(8)}`)
  }
  await browser.close()
}
