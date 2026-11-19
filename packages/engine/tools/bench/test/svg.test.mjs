// The SVG phase (MS3 step 9; README §4.3: S 20 / 7 ms, L 40 / 15 ms on the
// Android reference, iOS 0.6 times): the 11 pages of step 6's references
// ($LILY_CORPUS/ref/<id>/pages.scm) written in the file form by the core's
// bench (crates/core/src/bench.rs' Bench::svg), RUNS times each, natively
// (examples/bench.rs --svg, release) and in the built core.wasm (lsb_restore
// of scheme.image, lsb_pack, lsb_svg), as the harness's svg.html does in a
// browser. Both write the bytes crates/svg's write example writes. The
// times, each page's first, median and p90, are recorded against README
// §4.3, not judged: the budgets are the tablet's.
//
// Skipped, and so unverified, without LilyPond 2.26.0, the corpus, the
// references (npm run capture), a core.wasm and a scheme.image as new as the
// sources (npm run build), or cargo.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { benchDir, coreWasm, instantiateCore, lilyScm, schemeImage } from '../run.mjs'

const engine = path.resolve(benchDir, '../..')
/** The 10 scores of MS3's gate (README §5), all of class S. */
const TEN = ['T01', 'T03', 'T05', 'T10', 'T13', 'T14', 'T16', 'T17', 'T18', 'T19']
const ten = corpus().filter((row) => TEN.includes(row.id))
const refs = path.join(corpusDir(), 'ref')
const packs = path.join(engine, 'dist/engine/fonts')
const RUNS = 20
const sources = ['scheme', 'music', 'layout', 'fonts', 'svg', 'core'].map((c) => path.join(engine, 'crates', c, 'src'))
const newest = Math.max(...sources.flatMap((dir) => readdirSync(dir).map((f) => statSync(path.join(dir, f)).mtimeMs)))
const skip = !lilyScm()
  ? 'LilyPond 2.26.0 is not installed'
  : ten.length !== TEN.length ? 'the corpus is not fetched: run node tools/corpus/fetch.mjs'
  : !TEN.every((id) => existsSync(path.join(refs, id, 'pages.scm'))) ? `no pages.scm in ${refs}: run npm run capture`
  : [coreWasm, schemeImage, path.join(packs, 'emmentaler-20.pack')].map((file) => !existsSync(file)
    ? `no ${path.relative(engine, file)}: run npm run build`
    : statSync(file).mtimeMs < newest ? `${path.relative(engine, file)} is older than the sources: run npm run build` : false)
    .find(Boolean) ?? (spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed')

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-svg-phase-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** Nearest rank: the value at or below which a share `p` of `values` lies. */
const rank = (values, p) => [...values].sort((a, b) => a - b)[Math.ceil(p * values.length) - 1]
const summary = (ms) => ({ first: +ms[0].toFixed(3), median: +rank(ms, 0.5).toFixed(3), p90: +rank(ms, 0.9).toFixed(3) })
/** The tree's roots of a score's links: its directory and its capture's (T16's included string), as svg.test.mjs gives them. */
const roots = (row) => [path.dirname(row.path), path.join(refs, row.id)]
/** W1's Emmentaler packs, the brace's parts in order, as the write example gives them. */
const emmentaler = () => readdirSync(packs).filter((f) => f.startsWith('emmentaler-') && f.endsWith('.pack')).sort()

/** The core's lsb_* in a fresh instance restored from scheme.image, given the packs. */
async function wasmBench() {
  const mod = await WebAssembly.compile(readFileSync(coreWasm))
  const { exports: e } = await instantiateCore(mod)
  const put = (bytes) => {
    const at = e.lsb_alloc(bytes.length)
    new Uint8Array(e.memory.buffer, at, bytes.length).set(bytes)
    return [at, bytes.length]
  }
  const output = () => {
    const [at, length] = new Uint32Array(e.memory.buffer, e.lsb_output(), 2)
    return Buffer.from(e.memory.buffer, at, length).toString()
  }
  assert.equal(e.lsb_restore(...put(readFileSync(schemeImage))), 0, 'scheme.image restores')
  for (const name of emmentaler()) {
    const [at, length] = put(readFileSync(path.join(packs, name)))
    assert.equal(e.lsb_pack(at, length), 0, `${name}: ${output()}`)
    e.lily_free(at, length)
  }
  return {
    /** Writes `record` once; its milliseconds and SVG. */
    svg(record, roots) {
      const [r, rl] = put(Buffer.from(record))
      const [d, dl] = put(Buffer.from(roots.join('\n')))
      const ms = e.lsb_svg(r, rl, d, dl, 1)
      e.lily_free(r, rl)
      e.lily_free(d, dl)
      assert.ok(ms >= 0, output())
      return { ms, svg: output() }
    },
  }
}

test('the 11 pages written by the core\'s bench natively and in wasm, the write example\'s bytes, timed', { skip, timeout: 600_000 }, async (t) => {
  const core = await wasmBench()
  const rows = []
  for (const row of ten) {
    const pages = path.join(refs, row.id, 'pages.scm')
    const dirs = roots(row).flatMap((dir) => ['--root', dir])
    const written = path.join(scratch, 'write', row.id)
    const write = runExample('write', ['--packs', packs, '--out', written, ...dirs, pages], { cwd: engine, encoding: 'utf8' })
    assert.equal(write.status, 0, `${row.id}: ${write.stderr}`)
    const benched = path.join(scratch, 'bench', row.id)
    const bench = runExample('bench', ['--svg', schemeImage, packs, String(RUNS), ...dirs, '--out', benched, pages],
      { release: true, cwd: engine, encoding: 'utf8' })
    assert.equal(bench.status, 0, `${row.id}: ${bench.stderr}`)
    const native = bench.stdout.trim().split('\n').map((line) => JSON.parse(line))
    const records = readFileSync(pages, 'utf8').split('\n').filter((line) => line.startsWith('(page '))
    assert.equal(native.length, records.length, `${row.id}: a line for each page`)
    for (const [i, record] of records.entries()) {
      const { file } = native[i]
      const expected = readFileSync(path.join(written, file), 'utf8')
      assert.equal(readFileSync(path.join(benched, file), 'utf8'), expected, `${row.id} ${file}: natively, the write example's bytes`)
      const ms = []
      for (let r = 0; r < RUNS; r++) {
        const page = core.svg(record, roots(row))
        if (r === 0) assert.equal(page.svg, expected, `${row.id} ${file}: in wasm, the write example's bytes`)
        ms.push(page.ms)
      }
      rows.push({ id: row.id, file, bytes: Buffer.byteLength(expected), native: summary(native[i].ms), wasm: summary(ms) })
    }
  }
  assert.equal(rows.length, 11, 'the 11 pages')
  for (const r of rows) {
    t.diagnostic(`${r.id} ${r.file} ${r.bytes} B: native first ${r.native.first}, median ${r.native.median}, p90 ${r.native.p90} ms; ` +
      `wasm first ${r.wasm.first}, median ${r.wasm.median}, p90 ${r.wasm.p90} ms (README §4.3 S 20 / 7 ms)`)
  }
})
