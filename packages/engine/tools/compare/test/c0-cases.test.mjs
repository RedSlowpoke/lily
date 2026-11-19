// The error corpus's cases through the engine's core (02 §6, 08 §6): each
// case of the steps done so far, compiled natively through the core's Engine
// by crates/core/examples/compile.rs, one instance for them all, passes C0
// against LilyPond's .err with --parse-only (D64), its stderr is LilyPond's
// byte for byte but where INEXACT says, and parseStderr of its stderr is its
// diagnostics.json (08 §6). core.wasm gives the same two outputs through
// lily_put, lily_compile and lily_step from the build's core.image (MS2
// step 11). Both with the pitch names of the installed LilyPond 2.26.0's
// define-note-names.scm, and \name classified against a (lily) that loaded
// lily.scm's library files and init data (MS2 step 12), with what the parser
// needs besides, and the music functions of its
// ly/music-functions-init.ly, parsed and run as an init file
// (crates/parser/tests/support): from source natively, from the image in
// wasm. CASES holds all 22 cases of P1–P5 since step 18b, P6's case 15 since
// MS2 step 6, and its cases 16 and 29 since step 7, which need color.scm's
// #red and the grobs' names, and P7's cases 19 and 20 since MS2 step 14,
// case 20 with the Guile error that ends its run, and P11's cases 24 (a
// #{ … #} template's error) and 25 (ly:parser-include-string) since step 33, and 07 (an \include
// whose file is missing, which core.wasm asks its host for) since step 34. Since MS3 step 35 (D132)
// the two fixtures of ENDED, a \layout translation's error and a Scheme engraver's finalize error,
// end the run as LilyPond's: its stderr byte for byte, at init.ly, an error in diagnostics.json,
// exit 1 and no page or playback map, natively and in wasm; since MS3 step 64 (D220) the two of
// PAGE_ENDED, an error of a page at toplevel and in a first \book block, as LilyPond's too, the
// toplevel book's MIDI file, written before its pages, kept as its playback map. The last test times
// embedded forms through the parser (step 17c). Skipped, and so unverified, without cargo or that
// lilypond; the wasm tests also without a core.wasm and images as new as the
// parser's, the interpreter's and the core's sources (npm run build).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { fileURLToPath } from 'node:url'
import { instantiateCore } from '../../bench/run.mjs'
import { runExample } from '../../examples.mjs'
import { compileInWasm, moduleFile, preloadedModules } from '../core.mjs'
import { compareErrors } from '../score.mjs'
import { ERRORS, parseStderr } from '../stderr.mjs'

const engine = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
const coreWasm = path.join(engine, 'dist/engine/core.wasm')
const coreImage = path.join(engine, 'dist/engine/core.image')
const schemeImage = path.join(engine, 'dist/engine/scheme.image')

/** The cases each step is gated on (02 §6). */
const CASES = {
  P1: ['03-string-eof', '04-comment-eof', '05-version-string', '06-non-utf8', '26-tab-column', '27-utf8-column'],
  P2: ['10-octave-after-duration', '13-not-a-note-name', '14-not-a-duration',
    '08-unexpected-brace', '09-unexpected-eof', '12-string-in-notes', '30-unattached'],
  P3: ['01-unknown-command', '02-shorthand'],
  P4: ['21-scheme-read', '22-scheme-eval', '23-dollar-unbound'],
  P5: ['17-function-argument', '18-optional-argument', '11-paper-in-score', '28-error-count'],
  P6: ['15-context-mod', '16-bad-grob-path', '29-deprecated-path'],
  P7: ['19-markup-unknown', '20-markup-argument'],
  P11: ['24-embedded-lily', '25-include-string', '07-include-missing'],
}
const cases = Object.values(CASES).flat()
/** Cases whose stderr is not yet LilyPond's byte for byte, and why; C0 still passes. */
// A zero-duration score's warnings are printed for its \layout and for
// capture.ly's \midi, as LilyPond prints them with capture.ly (D99); the .err,
// captured without it, has them once; --parse-only drops them.
const ZERO = 'the processing warnings of a zero-duration score, twice, which --parse-only drops'
const INEXACT = { '30-unattached': ZERO, '17-function-argument': ZERO, '18-optional-argument': ZERO }

/** scm/lily/define-note-names.scm of the installed LilyPond 2.26.0, or null. */
function installedNoteNames() {
  const version = spawnSync('lilypond', ['--version'], { encoding: 'utf8' })
  if (version.status !== 0 || !version.stdout.includes('GNU LilyPond 2.26.0')) return null
  const binary = spawnSync('sh', ['-c', 'command -v lilypond'], { encoding: 'utf8' }).stdout.trim()
  const file = path.join(path.dirname(realpathSync(binary)), '../share/lilypond/2.26.0/scm/lily/define-note-names.scm')
  return existsSync(file) ? file : null
}
const noteNames = installedNoteNames()
/** That LilyPond's data directory, which its messages name and the engine's `<datadir>`. */
const datadir = noteNames && path.resolve(path.dirname(noteNames), '../..')

const cargo = spawnSync('cargo', ['+stable', '--version']).status !== 0
  ? 'cargo +stable is not installed'
  : noteNames ? false : 'LilyPond 2.26.0 is not installed'
const sources = ['crates/parser/src', 'crates/parser/tests/support', 'crates/scheme/src', 'crates/music/src', 'crates/layout/src', 'crates/core/src']
  .map((d) => path.join(engine, d))
const newest = Math.max(...sources.flatMap((dir) => readdirSync(dir).map((f) => statSync(path.join(dir, f)).mtimeMs)))
const built = [coreWasm, coreImage, schemeImage]
const stale = !built.every(existsSync)
  ? 'no dist/engine/core.wasm, core.image or scheme.image: run npm run build'
  : built.some((f) => statSync(f).mtimeMs < newest) ? 'dist/engine is older than the sources: run npm run build' : false

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-c0-cases-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/**
 * The core's runner on `cases`, from the corpus's directory so that locations name the files
 * as lilypond's do: each case's stderr and diagnostics.json.
 */
let compiled
function native() {
  if (compiled) return compiled
  const run = runExample('compile', ['--out', scratch, ...cases.map((c) => `${c}.ly`)],
  { cwd: ERRORS, encoding: 'utf8' })
  // 1: a case failed, as it should; 2: the runner could not run.
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
  compiled = Object.fromEntries(cases.map((c) => [c, {
    stderr: readFileSync(path.join(scratch, `${c}.err`)),
    json: readFileSync(path.join(scratch, `${c}.json`)),
  }]))
  return compiled
}

// The .err files were captured as UTF-8 text, so 06's Latin-1 bytes are U+FFFD there.
const text = (bytes) => new TextDecoder().decode(bytes)

test('the cases compiled natively through the core pass C0', { skip: cargo, timeout: 600_000 }, () => {
  native()
  for (const result of compareErrors(scratch, { cases, parseOnly: true })) {
    assert.ok(result.pass.C0, JSON.stringify(result))
  }
})

test('their stderr is LilyPond\'s but where INEXACT says', { skip: cargo, timeout: 600_000 }, () => {
  for (const c of cases) {
    const ref = readFileSync(path.join(ERRORS, `${c}.err`), 'utf8')
    if (INEXACT[c]) assert.notEqual(text(native()[c].stderr), ref, `${c} now matches: drop it from INEXACT`)
    else assert.equal(text(native()[c].stderr), ref, c)
  }
})

test('parseStderr of their stderr is their diagnostics.json', { skip: cargo, timeout: 600_000 }, () => {
  for (const c of cases) {
    const { stderr, json } = native()[c]
    // The core names files as the host put them, here relative to the corpus's
    // directory, which parseStderr resolves them against; the studio puts absolute paths.
    const diagnostics = JSON.parse(json.toString()).map((d) => ({ ...d, file: path.resolve(ERRORS, d.file) }))
    assert.deepEqual(diagnostics, parseStderr(text(stderr), { rootFile: path.join(ERRORS, `${c}.ly`) }), c)
    assert.ok(diagnostics.length > 0, `${c}: a case has a diagnostic`)
  }
})

test('core.wasm compiles them through lily_* to the same stderr and diagnostics.json', { skip: stale || cargo, timeout: 600_000 }, async () => {
  const { exports: e } = await instantiateCore(await WebAssembly.compile(readFileSync(coreWasm)))
  const put = (bytes) => {
    const at = e.lily_alloc(bytes.length)
    new Uint8Array(e.memory.buffer, at, bytes.length).set(bytes)
    return [at, bytes.length]
  }
  assert.equal(e.lily_start(...put(readFileSync(coreImage))), 0)
  // The packs a host preloads (D105), lily_put's kind 1.
  for (const [name, pack] of preloadedModules()) e.lily_put(1, ...put(Buffer.from(name)), ...put(pack))
  const out = e.lily_alloc(16)
  let revision = 0
  for (const c of cases) {
    const name = Buffer.from(`${c}.ly`)
    e.lily_put(0, ...put(name), ...put(readFileSync(path.join(ERRORS, `${c}.ly`))))
    e.lily_compile(++revision, ...put(name), 0)
    let step
    while ((step = e.lily_step(10)) !== 0) {
      if (step === 1) continue
      // A file it asks for, answered from the corpus's directory, missing if
      // it is not there; the same compile, its revision and root, started
      // again (D89).
      e.lily_needs(out)
      const [kind, at, length] = new Uint32Array(e.memory.buffer, out, 3)
      const asked = Buffer.from(e.memory.buffer, at, length).toString()
      // Or a text face's outlines, which a skyline asks for (D262): the
      // manifest's pack, as a host fetches it; no other module.
      if (kind === 1) {
        assert.match(asked, /\.outlines$/, c)
        e.lily_put(1, ...put(Buffer.from(asked)), ...put(readFileSync(moduleFile(asked))))
        e.lily_compile(revision, ...put(name), 0)
        continue
      }
      assert.equal(kind, 0, c)
      const disk = path.join(ERRORS, asked)
      e.lily_put(0, ...put(Buffer.from(asked)), ...(existsSync(disk) ? put(readFileSync(disk)) : [0, 0]))
      e.lily_compile(revision, ...put(name), 0)
    }
    const outputs = Object.fromEntries(Array.from({ length: e.lily_outputs() }, (_, i) => {
      e.lily_output(i, out)
      const [at, length, data, size] = new Uint32Array(e.memory.buffer, out, 4)
      return [Buffer.from(e.memory.buffer, at, length).toString(), Buffer.from(e.memory.buffer, data, size)]
    }))
    // The playback map of a case with music after the two (MS2 step 26),
    // and its pages, drawn with the preloaded packs (MS3 step 50), and its
    // MIDI files (MS5 step 1).
    assert.deepEqual(Object.keys(outputs).slice(0, 2), ['stderr', 'diagnostics.json'], c)
    assert.deepEqual(Object.keys(outputs).slice(2).filter((n) => !/\.(timing\.json|svg|midi)$/.test(n)), [], c)
    assert.deepEqual(outputs.stderr, native()[c].stderr, c)
    assert.deepEqual(outputs['diagnostics.json'], native()[c].json, c)
  }
})

/** The fixtures whose translation's error ends the run (MS3 step 35, D132). */
const ENDED = ['translation-error', 'finalize-error']
const fixtures = path.join(engine, 'tools/compare/fixtures')

test("a translation's error and a Scheme engraver's finalize error end the run as LilyPond's", { skip: cargo, timeout: 600_000 }, async () => {
  const out = mkdtempSync(path.join(scratch, 'ended-'))
  const native = {}
  for (const f of ENDED) {
    const ly = spawnSync('lilypond', ['--loglevel=WARNING', '-o', path.join(out, `ly-${f}`), `${f}.ly`], { cwd: fixtures, encoding: 'utf8' })
    assert.equal(ly.status, 1, f)
    const run = runExample('compile', ['--out', out, `${f}.ly`], { cwd: fixtures, encoding: 'utf8' })
    assert.equal(run.status, 1, `${f}: ${run.stderr}`)
    const stderr = readFileSync(path.join(out, `${f}.err`), 'utf8')
    assert.equal(stderr, ly.stderr.replaceAll(datadir, '<datadir>'), f)
    const json = JSON.parse(readFileSync(path.join(out, `${f}.json`), 'utf8'))
    const diagnostics = json.map((d) => ({ ...d, file: path.resolve(fixtures, d.file) }))
    assert.deepEqual(diagnostics, parseStderr(stderr, { rootFile: path.join(fixtures, `${f}.ly`) }), f)
    assert.deepEqual(json.filter((d) => d.severity === 'error').map(({ file, line, column }) => [file, line, column]),
      [['<datadir>/ly/init.ly', 66, 2]], f)
    native[f] = stderr
  }
  assert.deepEqual(readdirSync(out).filter((n) => /\.(svg|pdf|midi?|timing\.json)$/.test(n)), [], 'no page or playback map')
  if (stale) return
  const results = await compileInWasm(ENDED.map((f) => path.join(fixtures, `${f}.ly`)))
  for (const [i, f] of ENDED.entries()) {
    const { outputs } = results[i]
    assert.deepEqual(Object.keys(outputs), ['stderr', 'diagnostics.json'], f)
    assert.equal(outputs.stderr.toString(), native[f], f)
    const diagnostics = JSON.parse(outputs['diagnostics.json'].toString()).map((d) => ({ ...d, file: path.resolve(fixtures, d.file) }))
    assert.deepEqual(diagnostics, parseStderr(native[f], { rootFile: path.join(fixtures, `${f}.ly`) }), f)
  }
})

/** Pages whose own error ends the run (MS3 step 64, D220): at toplevel, and in a first \book block. */
const PAGE_ENDED = ['page-error', 'book-page-error']

test("an error of the pages ends the run as LilyPond's: the book's MIDI kept, no page, a \\book block's later books not translated", { skip: cargo, timeout: 600_000 }, async () => {
  const out = mkdtempSync(path.join(scratch, 'page-ended-'))
  const native = {}
  for (const f of PAGE_ENDED) {
    const ly = spawnSync('lilypond', ['--loglevel=WARNING', '-o', path.join(out, `ly-${f}`), `${f}.ly`], { cwd: fixtures, encoding: 'utf8' })
    assert.equal(ly.status, 1, f)
    const run = runExample('compile', ['--out', out, `${f}.ly`], { cwd: fixtures, encoding: 'utf8' })
    assert.equal(run.status, 1, `${f}: ${run.stderr}`)
    const stderr = readFileSync(path.join(out, `${f}.err`), 'utf8')
    // A \book block's report without the frame in Guile's own files (D132).
    assert.equal(stderr, ly.stderr.replaceAll(datadir, '<datadir>').replace(/^ice-9\/[^:]+:\d+:\d+: /m, ''), f)
    const json = JSON.parse(readFileSync(path.join(out, `${f}.json`), 'utf8'))
    const diagnostics = json.map((d) => ({ ...d, file: path.resolve(fixtures, d.file) }))
    assert.deepEqual(diagnostics, parseStderr(stderr, { rootFile: path.join(fixtures, `${f}.ly`) }), f)
    native[f] = stderr
  }
  // The toplevel book's error at init.ly. Each failing book's MIDI file, made before its pages and
  // kept, the engine's of a score without \midi too, as capture.ly gives each a \midi (MS5 step 1),
  // and its playback map; the later books' none.
  const json = JSON.parse(readFileSync(path.join(out, 'page-error.json'), 'utf8'))
  assert.deepEqual(json.filter((d) => d.severity === 'error').map(({ file, line, column }) => [file, line, column]),
    [['<datadir>/ly/init.ly', 66, 2]])
  assert.deepEqual(readdirSync(out).filter((n) => /\.(svg|pdf|midi?|timing\.json)$/.test(n)).sort(),
    ['book-page-error.midi', 'book-page-error.timing.json', 'ly-page-error.midi', 'page-error.midi', 'page-error.timing.json'])
  if (stale) return
  const results = await compileInWasm(PAGE_ENDED.map((f) => path.join(fixtures, `${f}.ly`)))
  for (const [i, f] of PAGE_ENDED.entries()) {
    const { outputs } = results[i]
    assert.deepEqual(Object.keys(outputs), ['stderr', 'diagnostics.json', `${f}.midi`, `${f}.timing.json`], f)
    assert.equal(outputs.stderr.toString(), native[f], f)
  }
})

/**
 * Step 17a's small forms and the forms of cases 21–23 (21's closed), each as
 * FORM_COPIES forms of a score, `#` but for 23's `$`: what a form costs
 * through the parser, read, run and its errors reported, in µs, natively in
 * release and by core.wasm's lsp_parse, an empty score's parse taken off.
 * Printed for P4 to compare with 17a's, not held to a budget.
 */
const FORMS = [
  ['#', '0.5'], ['#', 'UP'], ['#', "'(1 . 2)"], ['#', '(ly:make-moment 1/4)'],
  ['#', '(make-duration-of-length (ly:make-moment 3/8))'], ['#', '(lambda (x) (cons x LEFT))'],
  ['#', '(define-public x 3)'], ['#', '(define x (list 1 2))'], ['#', "(car '())"], ['$', '(no-such-thing)'],
]
const FORM_COPIES = 1000
const RUNS = 5
const median = (values) => [...values].sort((a, b) => a - b)[values.length >> 1]

test('embedded forms through the parser, natively and in wasm', { skip: stale || cargo, timeout: 600_000 }, async (t) => {
  const dir = mkdtempSync(path.join(scratch, 'forms-'))
  const version = '\\version "2.26.0"\n'
  const files = [{ name: 'empty.ly', text: version },
    ...FORMS.map(([sigil, form], i) => ({ name: `form-${i}.ly`, text: version + `${sigil}${form}\n`.repeat(FORM_COPIES) }))]
  for (const f of files) writeFileSync(path.join(dir, f.name), f.text)
  const run = runExample('parse', ['--note-names', noteNames, '--out', dir, '--time',
    ...Array.from({ length: RUNS }, () => files.map((f) => f.name)).flat()], { release: true, cwd: dir, encoding: 'utf8', maxBuffer: 1 << 26 })
  assert.ok(run.status === 0 || run.status === 1, run.stderr)
  const native = Object.fromEntries(files.map((f) => [f.name, []]))
  for (const line of run.stdout.trim().split('\n')) {
    const [name, ms] = line.split('\t')
    native[name].push(Number(ms))
  }

  const { exports } = await instantiateCore(await WebAssembly.compile(readFileSync(coreWasm)))
  const put = (bytes) => {
    const at = exports.lily_alloc(bytes.length)
    new Uint8Array(exports.memory.buffer, at, bytes.length).set(bytes)
    return [at, bytes.length]
  }
  assert.equal(exports.lsp_note_names(...put(readFileSync(noteNames))), 0)
  const image = readFileSync(schemeImage)
  const at = exports.lsb_alloc(image.length)
  new Uint8Array(exports.memory.buffer, at, image.length).set(image)
  assert.equal(exports.lsb_restore(at, image.length), 0)
  const wasm = Object.fromEntries(files.map((f) => [f.name, []]))
  for (let r = 0; r < RUNS; r++) {
    for (const f of files) {
      const name = put(Buffer.from(f.name))
      const text = put(Buffer.from(f.text))
      const start = performance.now()
      const out = exports.lsp_parse(...name, ...text)
      wasm[f.name].push(performance.now() - start)
      const [at, length] = new Uint32Array(exports.memory.buffer, out, 2)
      const stem = f.name.slice(0, -'.ly'.length)
      assert.deepEqual(Buffer.from(exports.memory.buffer, at, length), readFileSync(path.join(dir, `${stem}.err`)), f.name)
    }
  }
  const us = (times, f) => +((median(times[f]) - median(times['empty.ly'])) * 1000 / FORM_COPIES).toFixed(2)
  for (const [i, [sigil, form]] of FORMS.entries()) {
    t.diagnostic(`${sigil}${form}: ${us(native, `form-${i}.ly`)} µs natively, ${us(wasm, `form-${i}.ly`)} µs in wasm a form`)
  }
})
