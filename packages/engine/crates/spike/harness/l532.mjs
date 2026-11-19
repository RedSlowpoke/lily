// L532 for the preview's page side (B7, H2): harness.js's score, its
// revision 1 and chapter 00 §6.2's edits as the preview gets them, each a
// `render` message's pages and hashes. Two engravings: LilyPond's (the
// installed lilypond, 2 pages, as the extension shows them) and the spike's
// (packages/engine/dist/engine/'s core in Node, 4 pages, as the studio's
// engine worker gives them). pages.mjs imports it; run alone, it writes both
// as JSON into target/l532/, which serve.mjs serves as /l532/<name>.json.
//
//   node l532.mjs [--out dir]
//
// A fixture is { score, engraver, base, edits: { all, E1, E2 } }, where base
// and each edit are { pages, hashes }. `all` is base's pages with every hash
// changed, as the spike's harness replaced all of them at once (09 §4.4); E1
// and E2 have their pages' real hashes, so that only the pages that changed
// are parsed (D25).
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = new URL('.', import.meta.url)
const engine = new URL('../../../', here)

const harness = readFileSync(new URL('harness.js', here), 'utf8')
const { score, edit } = new Function(
  `${harness.slice(harness.indexOf('function random('), harness.indexOf('// ---- The engine'))}\nreturn { score, edit }`,
)()
const L = score('L532', 532, 29)

// protocol.ts's Step and Need.
const [DONE, MORE] = [0, 1]
const [FILE, MODULE] = [0, 1]
const hash = (text) => createHash('sha256').update(text).digest('hex')
/** Revision 1, E1 and E2. */
const versions = [L.src, edit(L, 'E1').src, edit(L, 'E2').src]
/** Revision 1 and the edits, from the pages of each of `versions`. */
function fixture(engraver, engraved) {
  const [base, E1, E2] = engraved.map((pages) => ({ pages, hashes: pages.map(hash) }))
  return { score: L.name, engraver, base, edits: { all: { pages: base.pages, hashes: base.hashes.map((h) => `changed-${h}`) }, E1, E2 } }
}

/** LilyPond's pages: each version as the same file, so that the links of an unchanged page are the same. */
export function lilypond() {
  const dir = mkdtempSync(join(tmpdir(), 'lily-l532-'))
  const engrave = (src) => {
    for (const f of readdirSync(dir)) rmSync(join(dir, f))
    writeFileSync(join(dir, 'score.ly'), src)
    execFileSync('lilypond', ['-dbackend=svg', 'score.ly'], { cwd: dir, stdio: 'ignore' })
    const number = (f) => Number(f.match(/-(\d+)\.svg$/)?.[1] ?? 0)
    const files = readdirSync(dir).filter((f) => /^score(-\d+)?\.svg$/.test(f)).sort((a, b) => number(a) - number(b))
    return files.map((f) => readFileSync(join(dir, f), 'utf8'))
  }
  try {
    const version = execFileSync('lilypond', ['--version']).toString().split('\n')[0]
    return fixture(version, versions.map(engrave))
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

/**
 * The spike's pages, from the core that build.sh left in dist/engine/, driven
 * through chapter 08 §4.2's C ABI as the studio's driver.ts does. The first
 * page's `<!--lily-spike {…}-->`, the harness's timings, is taken out: it
 * differs on every compile and would mark that page changed by every edit.
 */
export async function spike() {
  const dist = new URL('dist/engine/', engine)
  const read = (file) => readFileSync(new URL(file, dist))
  let manifest
  try {
    manifest = JSON.parse(read('manifest.json'))
  } catch {
    throw new Error(`no engine in ${fileURLToPath(dist)}: run build.sh (or npm run build)`)
  }
  const wasm = read(manifest.core.wasm)
  const { instance } = await WebAssembly.instantiate(wasm, { env: { lily_now: () => performance.now() } })
  const core = instance.exports
  const utf8 = new TextEncoder()
  const text = new TextDecoder()
  const copy = (bytes) => {
    const at = core.lily_alloc(bytes.length)
    new Uint8Array(core.memory.buffer, at, bytes.length).set(bytes)
    return [at, bytes.length]
  }
  const put = (kind, name, bytes) => {
    const [at, len] = copy(utf8.encode(name))
    const [data, dataLen] = bytes ? copy(bytes) : [0, 0]
    core.lily_put(kind, at, len, data, dataLen)
    core.lily_free(at, len)
    if (bytes) core.lily_free(data, dataLen)
  }
  const [image, imageLen] = copy(read(manifest.core.image))
  if (core.lily_start(image, imageLen) !== 0) throw new Error("the engine's image did not start")
  core.lily_free(image, imageLen)
  for (const name of manifest.core.preload) put(MODULE, name, read(manifest.modules[name].file))

  const engrave = (src) => {
    put(FILE, L.path, utf8.encode(src))
    const [root, rootLen] = copy(utf8.encode(L.path))
    core.lily_compile(1, root, rootLen, 1)
    core.lily_free(root, rootLen)
    let step
    while ((step = core.lily_step(1000)) === MORE);
    if (step !== DONE) throw new Error(`the spike asked for a file or module compiling ${L.name}`)
    const out = core.lily_alloc(16)
    const files = []
    for (let i = 0; i < core.lily_outputs(); i++) {
      core.lily_output(i, out)
      const [name, nameLen, data, dataLen] = new Uint32Array(core.memory.buffer, out, 4)
      files.push([text.decode(new Uint8Array(core.memory.buffer, name, nameLen)), text.decode(new Uint8Array(core.memory.buffer, data, dataLen))])
    }
    core.lily_free(out, 16)
    const diagnostics = JSON.parse(files.find(([name]) => name === 'diagnostics.json')[1])
    if (diagnostics.some((d) => d.severity === 'error')) throw new Error(`${L.name}: ${diagnostics[0].message}`)
    return files.filter(([name]) => name.endsWith('.svg')).map(([, svg]) => svg.replace(/<!--lily-spike \{.*?\}-->\n/, ''))
  }
  return fixture(`the spike, ${manifest.core.wasm} ${hash(wasm).slice(0, 12)}`, versions.map(engrave))
}

const changed = (f, e) => e.hashes.filter((h, i) => h !== f.base.hashes[i]).length
const kb = (pages) => (pages.reduce((n, p) => n + p.length, 0) / 1024).toFixed(0)
/** What a fixture holds, a line each: its pages, and how many each edit changes. */
export const describe = (f) => [
  `${f.score} by ${f.engraver}: ${f.base.pages.length} pages, ${kb(f.base.pages)} KB`,
  ...Object.entries(f.edits).map(([name, e]) => `  ${name}: ${e.pages.length} pages, ${changed(f, e)} changed`),
]

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const i = process.argv.indexOf('--out')
  const out = i > 0 ? process.argv[i + 1] : fileURLToPath(new URL('target/l532/', engine))
  mkdirSync(out, { recursive: true })
  for (const [name, f] of [['lilypond', lilypond()], ['spike', await spike()]]) {
    writeFileSync(join(out, `${name}.json`), JSON.stringify(f))
    console.log(describe(f).join('\n'))
    console.log(`  → ${join(out, `${name}.json`)}`)
  }
}
