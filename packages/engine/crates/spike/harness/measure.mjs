// Runs the harness in Playwright's Chromium and WebKit on this machine, as
// the desktop rows of 09-spike.md: a first visit, then cached visits. Run it
// from a directory where `playwright` is installed, with serve.mjs running.
// It drives any page that leaves its result in window.spikeResult, as
// translate.html does (MS3 step 36).
//
//   node <path>/measure.mjs [url] [visits] [--first] [--throttle] [--browsers chromium,webkit]
//                           [--cdp port] [--label name] [--timeout s]
//
//   url          default https://localhost:8471/ (serve.mjs's default)
//   visits       default 4
//   --first      every visit in a new browser context: nothing cached, as
//                B5c's first visit; without it the visits after the first are cached
//   --throttle   chapter 00's B5c network, 10 Mbit/s each way and 100 ms RTT,
//                through DevTools' throttling; Chromium only (WebKit has none)
//   --browsers   which browsers, default chromium,webkit
//   --cdp        in place of --browsers, the Chrome already running with its
//                DevTools on this port (an Android device or emulator through
//                `adb forward tcp:<port> localabstract:chrome_devtools_remote`),
//                a new page of its context each visit (its cache kept, so
//                not with --first)
//   --label      the device's name in each visit's label, default the browser's
//   --timeout    seconds a visit may take, default 300
//   --forget     after the visits, the page's origin's site data cleared
//                (Cache Storage, storage, cookies; DevTools' Storage domain),
//                so a device is left as found (MS7 step 10)
//
// A visit whose tab is killed (Playwright's `crash`) prints `KILLED` and its
// label, and the run goes on: D257's "no killed tab" on the Android floor.
// The page may be Lily Studio's own through serve.mjs --studio, whose
// studio-page.js leaves its result as the harness's pages do (MS7 step 10).
//
// It prints each visit's result, the content-encoding and body bytes of each
// engine response (a file from Cache Storage makes none), and the visits'
// ready times against chapter 00's budgets: B5a for cached visits, B5c for
// first visits throttled; first visits on a fast network have no budget.
// For translate.html's result, each score's layout apart from the writer,
// with its live grobs (MS3 step 36), and R1 (MS3 step 46): the five's
// (T01, T03, T14, T16, T19) layout per live grob together, each visit's
// and the median of the visits', against R1's Must 120 and Goal 40 µs.
import { createRequire } from 'node:module'

const playwright = createRequire(`${process.cwd()}/`)('playwright')
const flags = new Set(process.argv.slice(2).filter((a) => a.startsWith('--')))
const value = (name) => {
  const i = process.argv.indexOf(`--${name}`)
  return i > 0 ? process.argv[i + 1] : undefined
}
const VALUES = ['--browsers', '--cdp', '--label', '--timeout']
const positional = process.argv.slice(2).filter((a, i, all) => !a.startsWith('--') && !VALUES.includes(all[i - 1]))
const url = positional[0] ?? 'https://localhost:8471/'
const visits = Number(positional[1] ?? 4)
const first = flags.has('--first')
const forget = flags.has('--forget')
const throttle = flags.has('--throttle')
const cdp = value('cdp')
const browsers = cdp ? ['cdp'] : (value('browsers') ?? 'chromium,webkit').split(',')
const timeout = Number(value('timeout') ?? 300) * 1000
if (cdp && first) {
  // A device's own context keeps its cache: no visit there is a first one.
  console.error('measure.mjs: --first needs a browser of its own, not --cdp')
  process.exit(2)
}
// Chapter 00 §2 B5c: 10 Mbit/s, 100 ms RTT. DevTools' latency is added to each request.
const B5C_NETWORK = { offline: false, latency: 100, downloadThroughput: 10e6 / 8, uploadThroughput: 10e6 / 8 }
const network = throttle ? '10mbit-100ms-devtools' : 'unthrottled'
// B5a and B5c, Must / Goal in ms, desktop rows as Android's (chapter 00 §2).
const BUDGET = !first ? { name: 'B5a', must: 1200, goal: 600 } : throttle ? { name: 'B5c', must: 3000, goal: 2000 } : undefined

/** What --forget clears of the page's origin. */
const FORGET = 'cookies,local_storage,indexeddb,cache_storage,service_workers,file_systems'

// R1's five scores (MS3 step 46).
const FIVE = ['T01', 'T03', 'T14', 'T16', 'T19']
const median = (values) => {
  const v = [...values].sort((a, b) => a - b)
  return v.length % 2 ? v[(v.length - 1) / 2] : (v[v.length / 2 - 1] + v[v.length / 2]) / 2
}

for (const name of browsers) {
  if (throttle && name === 'webkit') {
    console.log(`${name}: skipped, --throttle needs Chromium's DevTools protocol`)
    continue
  }
  const browser = cdp ? await playwright.chromium.connectOverCDP(`http://localhost:${cdp}`) : await playwright[name].launch()
  const device = value('label') ?? name
  let context
  let page
  const ready = []
  const r1 = []
  for (let visit = 1; visit <= visits; visit++) {
    let fresh = true
    if (cdp) {
      // The device's own context, which the run must not close.
      await page?.close()
      context = browser.contexts()[0]
      page = await context.newPage()
    } else if (first || !context) {
      await context?.close()
      context = await browser.newContext()
      page = await context.newPage()
    } else fresh = false
    if (throttle && fresh) {
      const session = await context.newCDPSession(page)
      await session.send('Network.enable')
      await session.send('Network.emulateNetworkConditions', B5C_NETWORK)
    }
    // What each engine file arrived as: the worker's requests are the context's.
    const arrived = []
    const onResponse = async (response) => {
      const path = new URL(response.url()).pathname
      if (!path.startsWith('/engine') || response.request().method() !== 'GET') return
      const sizes = await response.request().sizes().catch(() => null)
      arrived.push(`${path.split('/').pop()} ${response.status()} ${response.headers()['content-encoding'] ?? 'identity'}` +
        // Chromium gives no size for the worker's own script.
        (sizes?.responseBodySize ? ` ${sizes.responseBodySize}` : ''))
    }
    context.on('response', onResponse)
    const label = `${device} visit ${visit}${first ? ' first' : ''}${throttle ? ' throttled' : ''}`
    const at = new URL(url)
    at.searchParams.set('label', label)
    at.searchParams.set('network', network)
    let killed
    const crashed = new Promise((resolve) => page.once('crash', () => resolve((killed = true))))
    await page.goto(at.href)
    const result = await Promise.race([
      page.waitForFunction(() => window.spikeResult, null, { timeout, polling: 1000 }).then((h) => h.jsonValue()),
      crashed,
    ])
    context.off('response', onResponse)
    if (killed) {
      console.log(`KILLED: ${label}: the tab was killed`)
      page = undefined
      continue
    }
    console.log(JSON.stringify(result))
    console.log(`  ${label}: ${arrived.join(', ') || 'no engine file from the network'}`)
    for (const [id, l] of Object.entries(result.layouts ?? {})) {
      console.log(`  ${id} layout ${l.ms} ms (${l.layOutMs} in its translations, ${l.stencilsMs} stencils), ` +
        `writer ${l.writeMs} ms, ${l.live} live grobs, ${l.usPerLive ?? '–'} µs a live grob, ${l.pages} pages`)
    }
    const five = FIVE.map((id) => result.layouts?.[id]).filter((l) => l?.usPerLive !== undefined)
    if (five.length === FIVE.length) {
      const us = (1000 * five.reduce((n, l) => n + l.ms, 0)) / five.reduce((n, l) => n + l.live, 0)
      r1.push(us)
      console.log(`  the five: ${us.toFixed(1)} µs a live grob`)
    }
    if (result.b5aReadyMs !== undefined) ready.push(result.b5aReadyMs)
  }
  if (forget && cdp) {
    // The page's own session, the types named: Silk answers `all` from the browser's with an internal error.
    const origin = new URL(url).origin
    page ??= await context.newPage()
    try {
      const session = await context.newCDPSession(page)
      await session.send('Storage.clearDataForOrigin', { origin, storageTypes: FORGET })
      console.log(`forgot ${origin}'s site data`)
    } catch (error) {
      console.log(`could not forget ${origin}'s site data: ${error.message}`)
    }
  }
  // Disconnected, not closed: closing would close the device's browser.
  if (cdp) await page?.close()
  else await browser.close()
  if (r1.length) {
    const m = median(r1)
    const verdict = m <= 40 ? 'within Goal' : m <= 120 ? 'within Must' : 'over Must'
    console.log(`${name} R1, the five: ${r1.map((us) => us.toFixed(1)).join(', ')} µs a live grob; ` +
      `median ${m.toFixed(1)} against 120 / 40: ${verdict}`)
  }
  if (!ready.length) continue
  const kept = first ? ready : ready.slice(1)
  const m = median(kept)
  const verdict = !BUDGET ? 'no budget' : m <= BUDGET.goal ? 'within Goal' : m <= BUDGET.must ? 'within Must' : 'over Must'
  console.log(`${name} ${network}${first ? ', every visit first' : ', cached visits'}: ready ${kept.map((ms) => ms.toFixed(0)).join(', ')} ms; ` +
    `median ${m.toFixed(0)}` + (BUDGET ? ` against ${BUDGET.name} ${BUDGET.must} / ${BUDGET.goal}: ${verdict}` : ''))
}
process.exit(0)
