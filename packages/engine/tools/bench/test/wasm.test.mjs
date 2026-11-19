// "Natively and in wasm, with the same results" for part 1 of MS1: b1–b8,
// an error, (gc) and forms nested MAX_NESTING deep, each evaluated by a fresh
// interpreter in the built core.wasm (lsb_*) and natively by the same code
// (crates/core/examples/bench.rs), give the same output and the same count
// of applications. Then, on the heap after init (scheme.image): lsb_collect
// collects once, and small embedded forms, as P4 will evaluate them, give
// the same natively and in wasm; their cost is printed (step 17a). Skipped,
// and so unverified, without a core.wasm (and a scheme.image) as new as the
// interpreter's sources (npm run build) or without cargo.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { runExample } from '../../examples.mjs'
import { benchDir, benchmarks, coreWasm, instantiateCore, runOnce, schemeImage, STATS } from '../run.mjs'

const engine = path.resolve(benchDir, '../..')
const sources = ['crates/scheme/src', 'crates/music/src', 'crates/layout/src', 'crates/core/src'].map((d) => path.join(engine, d))
const newest = Math.max(...sources.flatMap((dir) => readdirSync(dir).map((f) => statSync(path.join(dir, f)).mtimeMs)))
const staleness = (file) => !existsSync(file)
  ? `no dist/engine/${path.basename(file)}: run npm run build`
  : statSync(file).mtimeMs < newest ? `dist/engine/${path.basename(file)} is older than the interpreter: run npm run build` : false
const stale = staleness(coreWasm)
const cargo = spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-lsb-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

const MAX_NESTING = +/pub const MAX_NESTING: usize = (\d+);/.exec(readFileSync(path.join(engine, 'crates/scheme/src/compile.rs'), 'utf8'))[1]
const nested = (open, close, n) => `${open.repeat(n)}0${close.repeat(n)}`
/** Programs beside the benchmarks: an error after output, and step 4's nesting shapes either side of the limit. */
const programs = [
  { name: 'error.scm', source: '(display "before") (newline) (car \'())', ok: false },
  { name: 'gc.scm', source: '(define l (list 1 "a" (vector 2))) (gc) (display l) (apply gc \'())', ok: true, collections: 2 },
  ...[['(+ 1 ', ')'], ['(let ((x 1)) ', ')'], ['(define (f) ', ') (f)']].flatMap(([open, close], i) => [
    { name: `deep-${i}-under.scm`, source: nested(open, close, MAX_NESTING - 1), ok: true },
    { name: `deep-${i}-over.scm`, source: nested(open, close, MAX_NESTING + 1), ok: false },
  ]),
].map((p) => ({ ...p, file: path.join(scratch, p.name) }))

/** The native runs, one JSON line per file. */
function native(files) {
  const run = runExample('bench', files, { release: true, cwd: engine, encoding: 'utf8', maxBuffer: 1 << 26 })
  assert.equal(run.status, 0, run.stderr)
  return run.stdout.trim().split('\n').map((line) => JSON.parse(line))
}

test('b1–b8, an error and the nesting limit give the same in wasm as natively', { skip: stale || cargo, timeout: 600_000 }, async () => {
  for (const p of programs) writeFileSync(p.file, p.source)
  const benches = benchmarks()
  assert.deepEqual(benches.map((b) => b.name), ['b1-fib.scm', 'b2-tak.scm', 'b3-loop.scm', 'b4-alist.scm', 'b5-lists.scm',
    'b6-strings.scm', 'b7-closures.scm', 'b8-load.scm'])
  const cases = [
    ...benches.map((b) => ({ ...b, file: path.join(benchDir, b.name), ok: true })),
    ...programs.map((p) => ({ ...p, source: Buffer.from(p.source) })),
  ]
  const natively = native(cases.map((c) => c.file))
  const mod = await WebAssembly.compile(readFileSync(coreWasm))
  for (const [i, c] of cases.entries()) {
    const wasm = await runOnce(mod, c.source)
    const here = natively[i]
    assert.equal(wasm.output, here.output, `${c.name}: the output in wasm`)
    assert.equal(wasm.ok, here.ok, `${c.name}: whether it failed`)
    assert.equal(wasm.ok, c.ok, `${c.name}: ${wasm.output.slice(0, 200)}`)
    assert.equal(wasm.stats.applications, here.applications, `${c.name}: the applications`)
    if (c.expected !== undefined) assert.equal(wasm.output.trim(), c.expected, c.name)
    if (c.collections !== undefined) assert.deepEqual([wasm.stats.collections, here.collections], [c.collections, c.collections], c.name)
    if (c.name.endsWith('-over.scm')) assert.match(wasm.output, /expression nested too deeply/, c.name)
  }
  assert.equal(natively.find((n) => n.file.endsWith('error.scm')).output,
    'before\nIn procedure car: Wrong type (expecting pair): ()\n')
})

/** Small forms as a score's `#` and `$` give them to P4, each run in (lily) on the heap after init. */
const FORMS = ['0.5', 'UP', "'(1 . 2)", '(ly:make-moment 1/4)', '(make-duration-of-length (ly:make-moment 3/8))',
  '(lambda (x) (cons x LEFT))', '(define-public x 3)']
const COPIES = 1000
const RUNS = 5
const median = (values) => [...values].sort((a, b) => a - b)[values.length >> 1]

test('the heap after init collects once, and small embedded forms give the same natively and in wasm',
  { skip: stale || staleness(schemeImage) || cargo, timeout: 600_000 }, async (t) => {
    const image = readFileSync(schemeImage)
    const mod = await WebAssembly.compile(readFileSync(coreWasm))
    const { exports } = await instantiateCore(mod)
    const at = exports.lsb_alloc(image.length)
    new Uint8Array(exports.memory.buffer, at, image.length).set(image)
    assert.equal(exports.lsb_restore(at, image.length), 0, 'lsb_restore takes the image')
    assert.equal(exports.lsb_collect(), 0, 'lsb_collect')
    const values = new Float64Array(exports.memory.buffer, exports.lsb_stats(), STATS.length)
    const stats = Object.fromEntries(STATS.map((key, i) => [key, values[i]]))
    assert.equal(stats.collections, 1, 'one collection')
    assert.ok(stats.liveBytes > 0, 'the heap after init is live')
    t.diagnostic(`lsb_collect on the heap after init: ${stats.pauseMs.toFixed(2)} ms, ${stats.liveBytes} bytes live`)

    // Each form as COPIES toplevel forms, each read, expanded, compiled and
    // run, and as a loop that compiles it once and runs it COPIES times.
    const header = "(set-current-module (resolve-module '(lily)))\n"
    const cases = FORMS.flatMap((form, i) => [
      { form, kind: 'copies', source: header + `${form}\n`.repeat(COPIES) },
      ...form.startsWith('(define') ? [] : [{ form, kind: 'loop',
        source: header + `(let loop ((i 0)) (if (< i ${COPIES}) (begin ${form} (loop (+ i 1)))))\n` }],
    ].map((c) => ({ ...c, file: path.join(scratch, `form-${i}-${c.kind}.scm`) })))
    for (const c of cases) writeFileSync(c.file, c.source)
    const natively = native(['--image', schemeImage, ...Array.from({ length: RUNS }, () => cases.map((c) => c.file)).flat()])
    const us = (ms) => +(ms * 1000 / COPIES).toFixed(2)
    for (const [i, c] of cases.entries()) {
      const here = natively.filter((n, j) => j % cases.length === i)
      const wasm = []
      for (let r = 0; r < RUNS; r++) wasm.push(await runOnce(mod, Buffer.from(c.source), image))
      for (const run of [here[0], wasm[0]]) assert.ok(run.ok, `${c.form}: ${run.output.slice(0, 200)}`)
      assert.equal(wasm[0].output, here[0].output, `${c.form}: the output in wasm`)
      assert.equal(wasm[0].stats.applications, here[0].applications, `${c.form}: the applications`)
      c.native = us(median(here.map((n) => n.ms)))
      c.wasm = us(median(wasm.map((w) => w.evalMs)))
    }
    for (const form of FORMS) {
      const [copies, loop] = ['copies', 'loop'].map((kind) => cases.find((c) => c.form === form && c.kind === kind))
      t.diagnostic(`${form}: ${copies.native} µs natively, ${copies.wasm} µs in wasm a form` +
        (loop ? `; run alone ${loop.native} and ${loop.wasm} µs` : ''))
    }
  })
