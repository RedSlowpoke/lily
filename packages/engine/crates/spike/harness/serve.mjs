// Serves the harness (index.html, harness.js), what build.sh left in
// packages/studio/dist/site/ (engine.worker.js, engine/), what l532.mjs
// wrote in target/l532/ (/l532/lilypond.json, /l532/spike.json), and
// preview.html, which shows those through the extension's preview.js, midi.js
// and preview.css as they are in this repository (B7 on a device), and
// midi.html, which plays LilyPond's .midi of T24 or T29 from the corpus's
// captures ($LILY_CORPUS/ref, as /ref/<id>.midi) through midi.js (M8), and
// scheme.html, which runs chapter 03 Appendix B's b1–b8 (tools/bench/, as
// /bench/<name>.scm) on the built core.wasm's Scheme (dist/engine/, as
// /scheme/core.wasm, with the split modules the manifest names beside it as
// /scheme/<module>.wasm, which core-split.js joins to it since MS4 step 20)
// in a worker and restores the interpreter's image
// (/scheme/scheme.image) to run /bench/probe-library.scm (MS1), and starts the
// core from its image (/scheme/core.image) through lily_* (K2), and
// translate.html, which times the first full parse of T02 and T29 and the
// translation of the 30 scores through the same core (MS2 steps 27, 40), or
// through /scheme/core.names.wasm, the build's core with its names
// (target/engine/, for a profiler; MS3 step 36): a
// score of the corpus as /translate/<ID>/<ID>.ly (tools/compare's path.ly as
// /translate/path/path.ly), a file beside it as
// /translate/<ID>/<path>, and the installed LilyPond's ly/ and scm/ as
// /translate/datadir/…, and the font packs of dist/engine/fonts as
// /scheme/fonts/<name>.pack with the build's /scheme/manifest.json (MS3
// step 36; with --offline only the packs the manifest preloads, the others
// 503 Service Unavailable, as a fetch fails offline: W5's diagnostic on a
// device, MS3 step 50), and restore.html, which times the first restore of
// /scheme/core.image and /scheme/scheme.image in fresh workers, part by part
// (MS2 step 31), and svg.html, which times the engine's SVG writer on the 11
// pages of step 6's references (MS3 step 9): their records as
// /svg/<ID>/pages.scm from $LILY_CORPUS/ref, W1's Emmentaler packs as
// /svg/fonts/<name>.pack from dist/engine/fonts, and the pages crates/svg's
// write example writes of them at start, into target/svg/, as
// /svg/<ID>/<file> with /svg/index.json, with
// chapter 08 §5.1's Cache-Control, without COOP/COEP (B9). A file is sent as the build
// left it: its .br, written when it was built (H5), when the browser accepts
// brotli, else the file itself; nothing is compressed here. A phone's results
// are POSTed to /result and appended to a file.
//
// With --studio (MS7 step 10, K7) it is Lily Studio's own page in front of D55's
// server at that origin: it signs in once as --user with --password and gives
// every request that session; the studio's page, its engine and whatever the
// harness does not serve are the server's, the socket's upgrade too, and the
// harness's own pages stay its own. The page at `/`
// with `measure` in its query gets studio-page.js first in its head, which
// measures 00 §6 through the page; what the server sent from that load on
// (path, status, encoding, bytes) goes into the page's result. --dist serves
// another build's dist/engine to the harness's pages (+simd128's, K7).
//
//   node serve.mjs [--port 8471] [--results spike-results.jsonl]
//                  [--cert cert.pem --key key.pem | --http] [--offline]
//                  [--studio http://127.0.0.1:<port> --user <name> --password <password>]
//                  [--dist <dist/engine>] [--host 127.0.0.1]
//
// --host is where it listens, every address by default; 127.0.0.1 behind
// `tailscale serve`, which holds the port on the tailnet's address and gives
// the page HTTPS with Tailscale's certificate (MS7 step 10).
// HTTPS by default, so that every device is in a secure context and has
// Cache Storage (H6): with --cert and --key, or else with a certificate that
// mkcert makes in target/certs/ for localhost and this machine's names and
// addresses. Its root CA is trusted here once `mkcert -install` ran; a phone
// needs `$(mkcert -CAROOT)/rootCA.pem` installed and trusted (iOS: Settings →
// General → About → Certificate Trust Settings). --http serves plain http,
// which is a secure context only as localhost (Android: adb reverse tcp:8471 tcp:8471).
import { spawnSync } from 'node:child_process'
import { appendFileSync, existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs'
import http from 'node:http'
import https from 'node:https'
import net from 'node:net'
import { hostname, networkInterfaces } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { lilyScm } from '../../../tools/bench/run.mjs'
import { corpus, corpusDir } from '../../../tools/corpus/fetch.mjs'
import { runExample } from '../../../tools/examples.mjs'
import { e1e3 } from '../../../tools/compare/edits.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
const engine = path.resolve(here, '../../..')
const site = path.resolve(engine, '../studio/dist/site')
const fixtures = path.join(engine, 'target/l532')
const ref = path.join(corpusDir(), 'ref')
const bench = path.join(engine, 'tools/bench')
const argAt = (name) => {
  const i = process.argv.indexOf(`--${name}`)
  return i > 0 ? process.argv[i + 1] : undefined
}
const dist = path.resolve(argAt('dist') ?? path.join(engine, 'dist/engine'))
const core = path.join(dist, 'core.wasm')
const webview = {
  '/preview.html': path.join(here, 'preview.html'),
  '/preview-page.js': path.join(here, 'preview-page.js'),
  '/preview.js': path.resolve(engine, '../common/web/preview.js'),
  '/midi.js': path.resolve(engine, '../common/web/midi.js'),
  '/preview.css': path.resolve(engine, '../vscode/media/preview.css'),
  '/midi.html': path.join(here, 'midi.html'),
  '/midi-page.js': path.join(here, 'midi-page.js'),
  '/scheme.html': path.join(here, 'scheme.html'),
  '/scheme-page.js': path.join(here, 'scheme-page.js'),
  '/scheme/core.wasm': core,
  '/scheme/core.names.wasm': path.join(engine, 'target/engine/core.names.wasm'),
  '/scheme/scheme.image': path.join(dist, 'scheme.image'),
  '/scheme/core.image': path.join(dist, 'core.image'),
  '/scheme/manifest.json': path.join(dist, 'manifest.json'),
  '/translate.html': path.join(here, 'translate.html'),
  '/translate-page.js': path.join(here, 'translate-page.js'),
  '/restore.html': path.join(here, 'restore.html'),
  '/restore-page.js': path.join(here, 'restore-page.js'),
  '/svg.html': path.join(here, 'svg.html'),
  '/svg-page.js': path.join(here, 'svg-page.js'),
  '/core-split.js': path.join(here, 'core-split.js'),
  '/b6.html': path.join(here, 'b6.html'),
  '/b6-page.js': path.join(here, 'b6-page.js'),
  '/studio-page.js': path.join(here, 'studio-page.js'),
}
const fonts = path.join(dist, 'fonts')
const svgPages = path.join(engine, 'target/svg')
/** The 10 scores of MS3's gate, whose 11 pages svg.html writes. */
const SVG = ['T01', 'T03', 'T05', 'T10', 'T13', 'T14', 'T16', 'T17', 'T18', 'T19']
const scores = Object.fromEntries(corpus().map((row) => [row.id, row.path]))
scores.path = path.join(engine, 'tools/compare/fixtures/path.ly')
scores.big = path.join(corpusDir(), 'ms6-split/b8/t29x26.ly')
const lily = lilyScm()
const datadir = lily && path.resolve(lily, '../..')
const arg = (name, fallback) => {
  const i = process.argv.indexOf(`--${name}`)
  return i > 0 ? process.argv[i + 1] : fallback
}
const port = Number(arg('port', 8471))
const results = path.resolve(arg('results', 'spike-results.jsonl'))
const offline = process.argv.includes('--offline')
/** The packs the build's manifest preloads, which --offline still sends. */
const preloaded = () => JSON.parse(readFileSync(webview['/scheme/manifest.json'], 'utf8')).core.preload
const addresses = Object.values(networkInterfaces()).flat().filter((a) => a.family === 'IPv4').map((a) => a.address)

/** mkcert's certificate for every name a device may use, made again when the names change. */
function localCertificate() {
  const dir = `${engine}/target/certs`
  const host = hostname().replace(/\.local$/, '')
  const names = [...new Set(['localhost', '127.0.0.1', '::1', host, `${host}.local`, ...addresses])]
  const files = { cert: `${dir}/cert.pem`, key: `${dir}/key.pem`, names: `${dir}/names` }
  if (!existsSync(files.cert) || !existsSync(files.names) || readFileSync(files.names, 'utf8') !== names.join(' ')) {
    mkdirSync(dir, { recursive: true })
    const made = spawnSync('mkcert', ['-cert-file', files.cert, '-key-file', files.key, ...names], { encoding: 'utf8' })
    if (made.error || made.status !== 0) {
      console.error(`serve: mkcert failed (${made.error?.message ?? made.stderr.trim()}).\n` +
        'Install it (brew install mkcert; mkcert -install), give --cert and --key, or serve --http.')
      process.exit(1)
    }
    writeFileSync(files.names, names.join(' '))
  }
  const caroot = spawnSync('mkcert', ['-CAROOT'], { encoding: 'utf8' }).stdout?.trim()
  if (caroot) console.log(`root CA for the phones: ${caroot}/rootCA.pem`)
  return files
}

const TYPES = { '.ly': 'text/plain; charset=utf-8', '.ily': 'text/plain; charset=utf-8', '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.json': 'application/json', '.wasm': 'application/wasm', '.midi': 'audio/midi', '.scm': 'text/plain; charset=utf-8' }

function file(url) {
  const name = decodeURIComponent(url.pathname)
  if (name === '/' || name === '/index.html') return path.join(here, 'index.html')
  if (name === '/harness.js') return path.join(here, 'harness.js')
  if (/^\/l532\/(lilypond|spike)\.json$/.test(name)) return path.join(fixtures, path.basename(name))
  if (Object.hasOwn(webview, name)) return webview[name]
  const edited = /^\/b6\/(T\d\d|big)\.json$/.exec(name)
  if (edited) return b6Edits(edited[1])
  // The core's split modules (K4) and S6's parts (I2), which the manifest names beside it.
  const split = /^\/scheme\/([\w-]+\.(wasm|part))$/.exec(name)
  if (split) return path.join(dist, split[1])
  const scm = /^\/bench\/(b\d[\w-]*\.scm|probe-library\.scm)$/.exec(name)
  if (scm) return path.join(bench, scm[1])
  const capture = /^\/ref\/(T\d\d)\.midi$/.exec(name)
  if (capture) return captured(capture[1])
  const pack = /^\/svg\/fonts\/(emmentaler-[\w-]+\.pack)$/.exec(name)
  if (pack) return path.join(fonts, pack[1])
  const font = /^\/scheme\/fonts\/([\w-]+\.pack)$/.exec(name)
  if (font) return path.join(fonts, font[1])
  if (name === '/svg/index.json') return path.join(svgPages, 'index.json')
  const page = /^\/svg\/(T\d\d)\/([^/]+)$/.exec(name)
  if (page && SVG.includes(page[1])) return page[2] === 'pages.scm' ? path.join(ref, page[1], page[2]) : path.join(svgPages, page[1], page[2])
  const translate = /^\/translate\/([^/]+)\/(.+)$/.exec(name)
  if (translate) return translated(translate[1], translate[2])
  const full = path.join(site, path.normalize(name))
  return full.startsWith(site) && (name === '/engine.worker.js' || name.startsWith('/engine/')) ? full : undefined
}

/**
 * b6.html's score: E1–E3 of `id` as tools/compare's edits find them, or for
 * `big`, B8's T29 × 26 ($LILY_CORPUS/ms6-split/), with one pitch changed.
 */
function b6Edits(id) {
  const out = path.join(engine, 'target/b6', `${id}.json`)
  mkdirSync(path.dirname(out), { recursive: true })
  const edits = id === 'big'
    ? [{ name: 'E1', file: scores.big, source: readFileSync(scores.big, 'utf8').replace(/\bc'4\b/, "d'4") }]
    : e1e3(id)
  writeFileSync(out, JSON.stringify({ root: `${id}.ly`, source: '', edits: edits.map(({ name, source }) => ({ name, source })) }))
  return out
}

/** The .midi that capture.mjs left in a score's directory of $LILY_CORPUS/ref, named after its .ly. */
function captured(id) {
  const dir = path.join(ref, id)
  const midi = existsSync(dir) ? readdirSync(dir).filter((f) => f.endsWith('.midi')) : []
  return midi.length === 1 ? path.join(dir, midi[0]) : undefined
}

/**
 * What translate.html asks for under /translate/: `<ID>/<ID>.ly` is the
 * score, `<ID>/<path>` a file under its directory, `datadir/<path>` one
 * under LilyPond's ly/ or scm/.
 */
function translated(head, rest) {
  if (head === 'datadir') {
    const full = datadir && path.join(datadir, path.normalize(rest))
    return full && ['ly', 'scm'].some((d) => full.startsWith(path.join(datadir, d) + path.sep)) ? full : undefined
  }
  if (!Object.hasOwn(scores, head)) return undefined
  if (rest === `${head}.ly`) return scores[head]
  const dir = path.dirname(scores[head])
  const full = path.join(dir, path.normalize(rest))
  return full.startsWith(dir + path.sep) ? full : undefined
}

/**
 * svg.html's pages: the 10's references written by crates/svg's write
 * example into target/svg/<ID>/, as tools/compare's svg.test.mjs writes
 * them, and target/svg/index.json naming the packs, and each score's tree
 * roots and pages. A reason when it cannot, else undefined.
 */
function writeSvgPages() {
  if (!existsSync(path.join(fonts, 'emmentaler-20.pack'))) return 'no dist/engine/fonts; run npm run build'
  const missing = SVG.find((id) => !scores[id] || !existsSync(path.join(ref, id, 'pages.scm')))
  if (missing) return `no corpus or no ${missing}/pages.scm in ${ref}; run npm run corpus, then npm run capture`
  const index = { packs: readdirSync(fonts).filter((f) => f.startsWith('emmentaler-') && f.endsWith('.pack')).sort(), scores: [] }
  for (const id of SVG) {
    const dir = path.join(svgPages, id)
    const roots = [path.dirname(scores[id]), path.join(ref, id)]
    const run = runExample('write', ['--packs', fonts, '--out', dir, ...roots.flatMap((r) => ['--root', r]), path.join(ref, id, 'pages.scm')],
      { cwd: engine, encoding: 'utf8' })
    if (run.status !== 0) return `the write example failed on ${id}: ${run.stderr.trim()}`
    const pages = run.stdout.trim().split('\n').map((line) => JSON.parse(line)).map(({ file, number, bytes }) => ({ file, number, bytes }))
    index.scores.push({ id, roots, pages })
  }
  writeFileSync(path.join(svgPages, 'index.json'), JSON.stringify(index))
  return undefined
}

/** Whether Accept-Encoding allows brotli: listed, and not with q=0. */
const acceptsBrotli = (header = '') =>
  header.split(',').some((part) => {
    const [coding, ...params] = part.trim().split(';')
    return coding.trim() === 'br' && !params.some((p) => /^\s*q=0(\.0*)?\s*$/.test(p))
  })

const stale = new Set()

async function handle(req, res) {
  const url = new URL(req.url, 'http://x')
  if (req.method === 'POST' && url.pathname === '/result') {
    let body = ''
    for await (const chunk of req) body += chunk
    const extra = studio ? { served: served.splice(0) } : {}
    appendFileSync(results, JSON.stringify({ at: new Date().toISOString(), ...JSON.parse(body), ...extra }) + '\n')
    console.log(`result from ${JSON.parse(body).label ?? req.headers['user-agent']}`)
    return res.writeHead(204).end()
  }
  const full = file(url)
  if (studio && (!full || STUDIO_PAGE.has(url.pathname) || url.pathname.startsWith('/engine'))) return proxy(req, res, url)
  if (!full || !existsSync(full)) return res.writeHead(404).end()
  if (offline && path.dirname(full) === fonts && url.pathname.startsWith('/scheme/') && !preloaded().includes(path.basename(full, '.pack'))) {
    return res.writeHead(503).end()
  }
  // Names under engine/ carry their hash: never stale. Everything else is checked on each load.
  const immutable = url.pathname.startsWith('/engine/') && !url.pathname.endsWith('manifest.json')
  const headers = {
    'content-type': TYPES[path.extname(full)] ?? 'application/octet-stream',
    'cache-control': immutable ? 'public, max-age=31536000, immutable' : 'no-cache',
  }
  let sent = full
  const br = `${full}.br`
  if (existsSync(br)) {
    headers.vary = 'accept-encoding'
    // A .br older than its file is from an earlier build: the file itself is right, if larger.
    if (statSync(br).mtimeMs < statSync(full).mtimeMs) {
      if (!stale.has(br)) console.warn(`serve: ${path.relative(site, br)} is older than its file; sending the file`)
      stale.add(br)
    } else if (acceptsBrotli(req.headers['accept-encoding'])) {
      sent = br
      headers['content-encoding'] = 'br'
    }
  }
  const body = readFileSync(sent)
  headers['content-length'] = body.length
  res.writeHead(200, headers).end(body)
}

// --- Lily Studio's page in front of D55's server (--studio) ----------------------

const studio = argAt('studio') && new URL(argAt('studio'))
/** The spike's page, which the studio's is in front of: the studio's engine is the server's too. */
const STUDIO_PAGE = new Set(['/', '/index.html', '/harness.js'])
/** What the server sent since the measured page was asked for. */
const served = []
let session = ''

/** Signs in to D55's server as a browser would; the session cookie then goes with every request. */
async function signIn() {
  const body = new URLSearchParams({ name: arg('user', 'ada'), password: arg('password', '') }).toString()
  const answer = await new Promise((resolve, reject) => {
    const req = http.request({ host: studio.hostname, port: studio.port, path: '/login', method: 'POST', headers: {
      host: studio.host, origin: studio.origin, 'content-type': 'application/x-www-form-urlencoded', 'content-length': Buffer.byteLength(body) } }, resolve)
    req.on('error', reject)
    req.end(body)
  })
  answer.resume()
  const cookie = [answer.headers['set-cookie'] ?? []].flat().map((c) => c.split(';')[0]).find((c) => c.startsWith('lily_session=') && c.length > 'lily_session='.length)
  if (!cookie) throw new Error(`serve: signing in to ${studio.origin} failed (${answer.statusCode})`)
  session = cookie
}

/** The browser's headers for the server: its own Host and Origin kept (the server checks they agree), the session's cookie. */
function upstreamHeaders(req) {
  const headers = { ...req.headers }
  headers.cookie = [headers.cookie, session].filter(Boolean).join('; ')
  return headers
}

function proxy(req, res, url) {
  const measured = req.method === 'GET' && url.pathname === '/' && url.searchParams.has('measure')
  const headers = upstreamHeaders(req)
  if (measured) {
    served.splice(0)
    // The page is changed here, so it comes whole and plain.
    delete headers['if-none-match']
    delete headers['if-modified-since']
    headers['accept-encoding'] = 'identity'
  }
  const up = http.request({ host: studio.hostname, port: studio.port, path: req.url, method: req.method, headers }, (answer) => {
    const chunks = []
    answer.on('data', (d) => chunks.push(d))
    answer.on('end', () => {
      let body = Buffer.concat(chunks)
      const out = { ...answer.headers }
      if (measured && answer.statusCode === 200) {
        body = Buffer.from(String(body).replace('<head>', '<head><script src="/studio-page.js"></script>'))
        out['content-length'] = body.length
        delete out['transfer-encoding']
        delete out.etag
        out['cache-control'] = 'no-store'
      }
      served.push({ at: Date.now(), path: url.pathname, status: answer.statusCode, encoding: answer.headers['content-encoding'] ?? 'identity', bytes: body.length })
      res.writeHead(answer.statusCode, out).end(body)
    })
  })
  up.on('error', () => res.writeHead(502).end())
  req.pipe(up)
}

/** The socket's upgrade, piped through with the session. */
function upgrade(req, socket, head) {
  const up = net.connect(Number(studio.port), studio.hostname, () => {
    const headers = upstreamHeaders(req)
    up.write(`${req.method} ${req.url} HTTP/1.1\r\n${Object.entries(headers).map(([k, v]) => `${k}: ${v}`).join('\r\n')}\r\n\r\n`)
    up.write(head)
    socket.pipe(up).pipe(socket)
  })
  up.on('error', () => socket.destroy())
  socket.on('error', () => up.destroy())
}

if (studio) await signIn()

const certificate = process.argv.includes('--http')
  ? undefined
  : arg('cert') && arg('key')
    ? { cert: arg('cert'), key: arg('key') }
    : localCertificate()
const server = certificate
  ? https.createServer({ cert: readFileSync(certificate.cert), key: readFileSync(certificate.key) }, handle)
  : http.createServer(handle)
if (studio) server.on('upgrade', upgrade)
server.listen(port, arg('host', '0.0.0.0'), () => {
  const scheme = certificate ? 'https' : 'http'
  for (const a of ['localhost', ...addresses]) console.log(`${scheme}://${a}:${port}/`)
  console.log('preview edits (B7): /preview.html, ?set=spike for the spike\'s pages')
  console.log('MIDI playback (M8): /midi.html, ?score=T29 for T29')
  console.log('Scheme benchmarks (MS1): /scheme.html, ?runs=5')
  console.log('parse and translation (MS2 parts 3 and 4): /translate.html, ?runs=5')
  console.log('the first restore in fresh workers (MS2 step 31): /restore.html, ?workers=3&runs=5&compile=streaming')
  console.log('the SVG phase (MS3 step 9): /svg.html, ?runs=20&thread=page')
  console.log('B6 by phase (MS6 step 12): /b6.html, ?runs=20&scores=T02,T12&b8=1')
  console.log(`site: ${site}`)
  if (studio) console.log(`Lily Studio's page: ${studio.origin} behind this, signed in; 00 §6 through it at /?measure=1 (MS7 step 10)`)
  if (!existsSync(`${site}/engine.worker.js`)) console.warn('serve: no engine.worker.js in the site; run build.sh')
  if (!existsSync(core)) console.warn('serve: no dist/engine/core.wasm for scheme.html; run npm run build')
  if (!existsSync(`${fixtures}/spike.json`)) console.warn('serve: no L532 pages in target/l532/; run l532.mjs after build.sh')
  if (!lily) console.warn('serve: no LilyPond 2.26.0 for translate.html\'s ly/ and scm/')
  if (!existsSync(scores.T29)) console.warn('serve: no corpus for translate.html; run npm run corpus')
  const svg = writeSvgPages()
  if (svg) console.warn(`serve: no pages for svg.html: ${svg}`)
  if (!captured('T24') || !captured('T29')) console.warn(`serve: no .midi of T24 or T29 in ${ref}; run npm run corpus, then npm run capture`)
  if (offline) console.log('offline: /scheme/fonts/ sends only the preloaded packs (W5)')
  console.log(`results: ${results}`)
})
