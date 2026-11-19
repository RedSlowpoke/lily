// O2 (MS3 steps 4 and 5): LilyPond's font metrics and font selection on W1's
// packs give LilyPond 2.26.0's records three ways: in (lily) loaded from
// source natively, restored natively from the image the build shipped
// (dist/engine/scheme.image), and restored from it in the built core.wasm
// (lsb_restore). The test puts W1's packs, which tools/packs/build.mjs builds
// here from the installed LilyPond's OTFs, with (lily)'s %put-font-pack
// first.
//
// - Step 4: tools/bench/probe-fonts.scm's calls of the font metrics on
//   emmentaler-20, emmentaler-13 and emmentaler-brace's four parts
//   (tools/compare/font-calls.ly). crates/layout/tests/fonts.rs runs the
//   probe natively in a VM without the library.
// - Step 5: the fonts the 10 scores' grobs and markups select, captured by
//   compiling them in LilyPond with tools/compare/font-select.ly, each
//   selected again by tools/bench/probe-select.scm's ly:paper-get-font in
//   LilyPond (font-calls.ly with FONT_PROBE), which must give each the font
//   the score got, then the probe's grid of staff sizes and font sizes over
//   Emmentaler's eight sizes and the braces, in the engine as in LilyPond.
// - Step 5: the core given packs as the driver gives modules (lily_put's
//   kind 1: the manifest's emmentaler-20.pack and fonts/' emmentaler-13 and
//   brace parts), natively by examples/compile.rs' --pack and in wasm,
//   selects in a compile what LilyPond selects, its warnings LilyPond's.
//
// Skipped, and so unverified, without LilyPond 2.26.0, without a core.wasm,
// a core.image and a scheme.image as new as the sources (npm run build),
// without cargo, or, for the 10 scores, without the corpus (fetch.mjs).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { runExample } from '../../examples.mjs'
import { buildPacks } from '../../packs/build.mjs'
import { corpus } from '../../corpus/fetch.mjs'
import { captureOne } from '../../compare/capture.mjs'
import { compileInWasm, coreSkip } from '../../compare/core.mjs'
import { benchDir, coreWasm, lilyScm, runOnce, schemeImage } from '../run.mjs'

const engine = path.resolve(benchDir, '../..')
const sources = ['crates/scheme/src', 'crates/music/src', 'crates/layout/src', 'crates/fonts/src', 'crates/parser/src', 'crates/parser/tests/support', 'crates/core/src']
  .map((d) => path.join(engine, d))
const newest = Math.max(...sources.flatMap((dir) => readdirSync(dir).map((f) => statSync(path.join(dir, f)).mtimeMs)))
const lily = lilyScm()
const skip = !lily
  ? 'LilyPond 2.26.0 is not installed'
  : [coreWasm, schemeImage].map((file) => !existsSync(file)
    ? `no dist/engine/${path.basename(file)}: run npm run build`
    : statSync(file).mtimeMs < newest ? `dist/engine/${path.basename(file)} is older than the sources: run npm run build` : false)
    .find(Boolean) || coreSkip() || (spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed')
/** The 10 scores of MS3's gate (README §5). */
const TEN = ['T01', 'T03', 'T05', 'T10', 'T13', 'T14', 'T16', 'T17', 'T18', 'T19']
const ten = corpus().filter((row) => TEN.includes(row.id))
const skipTen = skip || (ten.every((row) => existsSync(row.path)) ? false : 'the corpus is not fetched: run node tools/corpus/fetch.mjs')

const capture = path.join(engine, 'tools/compare/font-calls.ly')
const dist = path.join(engine, 'dist/engine')
const scratch = mkdtempSync(path.join(tmpdir(), 'lily-fonts-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** The packs the probes load, in the order a host gives them. */
const PACKS = ['emmentaler-20', 'emmentaler-13', 'emmentaler-brace-0', 'emmentaler-brace-1', 'emmentaler-brace-2', 'emmentaler-brace-3']
/** And every optical size, for step 5's grid. */
const ALL = ['emmentaler-11', 'emmentaler-14', 'emmentaler-16', 'emmentaler-18', 'emmentaler-23', 'emmentaler-26', ...PACKS]

let built
/** The packs build.mjs builds into the scratch directory, once. */
function packs() {
  built ??= (buildPacks({ out: path.join(scratch, 'packs') }), path.join(scratch, 'packs'))
  return built
}

/** `names`' packs given to (lily)'s %put-font-pack, as Scheme. */
const put = (names) => names.map((name) => `((@@ (lily) %put-font-pack) #vu8(${[...readFileSync(path.join(packs(), `${name}.pack`))].join(' ')}))\n`).join('')

/** The core's examples/bench.rs with `args`: its JSON lines. */
function native(...args) {
  const run = runExample('bench', args, { release: true, cwd: engine, encoding: 'utf8', maxBuffer: 1 << 26 })
  assert.equal(run.status, 0, run.stderr)
  return run.stdout.trim().split('\n').map((line) => JSON.parse(line))
}

/** A probe's records, one per call. */
const records = (output) => {
  const all = output.split('\u001e\n')
  assert.equal(all.pop(), '', 'the output ends a record')
  return all
}

/** LilyPond's records of font-calls.ly with `env`. */
function lilypond(env = {}) {
  const run = spawnSync('lilypond', ['-s', capture], { cwd: scratch, encoding: 'utf8', maxBuffer: 1 << 26, env: { ...process.env, ...env } })
  assert.equal(run.status, 0, run.stderr)
  return records(run.stdout)
}

/** `source` run from source natively, from the image natively and in wasm: each run's records, by name. */
async function engineRuns(name, source) {
  const file = path.join(scratch, name)
  writeFileSync(file, source)
  const [fromSource] = native('--library', lily, file)
  const [fromImage] = native('--image', schemeImage, file)
  const wasm = await runOnce(await WebAssembly.compile(readFileSync(coreWasm)), Buffer.from(source), readFileSync(schemeImage))
  assert.equal(wasm.init, 0, 'lsb_restore takes the image')
  return [['from source natively', fromSource], ['from the image natively', fromImage], ['from the image in wasm', wasm]]
}

/** Each run's records equal `reference`'s. */
function same(runs, reference) {
  for (const [name, run] of runs) {
    assert.ok(run.ok, `${name}: ${run.output.slice(-400)}`)
    const got = records(run.output)
    assert.equal(got.length, reference.length, `${name}: every call`)
    const differ = got.map((record, i) => [i + 1, record, reference[i]]).filter(([, a, b]) => a !== b)
    assert.deepEqual(differ.slice(0, 10), [], `${name}: the calls that differ, as [call, engine, LilyPond]`)
  }
}

test('the font metrics give LilyPond\'s records on W1\'s packs from source natively, from the image natively and in wasm',
  { skip, timeout: 600_000 }, async () => {
    const reference = lilypond()
    // The written calls, then each font and its glyphs: 664, 664 and 577.
    assert.ok(reference.length > 3 + 664 + 664 + 577, 'LilyPond ran every call')
    const source = put(PACKS) + readFileSync(path.join(benchDir, 'probe-fonts.scm'), 'utf8')
    same(await engineRuns('probe-fonts.scm', source), reference)
  })

test('the fonts the 10 scores select are LilyPond\'s from source natively, from the image natively and in wasm',
  { skip: skipTen, timeout: 600_000 }, async () => {
    // What each score's grobs and markups selected, as LilyPond compiled it.
    const selected = await Promise.all(ten.map(async (row) => {
      const dir = path.join(scratch, 'select', row.id)
      mkdirSync(dir, { recursive: true })
      const out = path.join(dir, 'fonts.tsv')
      const result = await captureOne('lilypond', row, dir, path.join(engine, 'tools/compare/font-select.ly'), { FONT_SELECT_OUT: out })
      assert.ok(existsSync(out), `${row.id}: no selection written (exit ${result.exit})`)
      return readFileSync(out, 'utf8').split('\n').filter(Boolean).map((line) => [row.id, ...line.split('\t')])
    }))
    // A case: staff-height, font-size, font-encoding, font-family, font-name; the font the score got.
    const cases = new Map()
    for (const [id, , ...fields] of selected.flat()) {
      const key = fields.slice(0, 5).join(' ')
      const font = `(${fields.slice(5).join(' ')})`
      assert.equal(cases.get(key)?.font ?? font, font, `${id}: ${key} selected two fonts`)
      cases.set(key, { font, ids: [...new Set([...(cases.get(key)?.ids ?? []), id])] })
    }
    assert.ok(cases.size > 0 && TEN.every((id) => selected.flat().some(([at]) => at === id)), 'every score selected a music font')
    const head = `(set-current-module (resolve-module '(lily)))\n(define probe-cases '(${[...cases.keys()].map((key) => `(${key})`).join(' ')}))\n`
    const probe = head + readFileSync(path.join(benchDir, 'probe-select.scm'), 'utf8')
    const file = path.join(scratch, 'probe-select.scm')
    writeFileSync(file, probe)
    const reference = lilypond({ FONT_PROBE: file })
    // ly:paper-get-font gives each case the font the score got.
    assert.deepEqual(reference.slice(0, cases.size), [...cases.values()].map(({ font }) => `\t${font}`))
    // The cases, 10 written calls and the grid's 12 staff sizes of 15 calls.
    assert.equal(reference.length, cases.size + 10 + 12 * 15, 'LilyPond ran every call')
    const paper = '(module-define! (resolve-module \'(lily)) \'probe-paper (cdddr (assq \'$defaultpaper (@@ (lily) lilypond-declarations))))\n'
    same(await engineRuns('probe-select-engine.scm', put(ALL) + paper + probe), reference)
  })

/** A file that selects three fonts in the default \paper and warns what each is. */
const SELECT = `\\version "2.26.0"
#(define (select-show size encoding glyph)
   (let* ((font (ly:paper-get-font $defaultpaper
                  (list (list (cons 'font-size size) (cons 'font-encoding encoding))
                        (ly:output-def-lookup $defaultpaper 'property-defaults))))
          (stencil (ly:font-get-glyph font glyph)))
     (ly:warning "~a ~a ~a ~a ~a" (ly:font-file-name font) (ly:font-name font) (ly:font-magnification font)
                 (ly:stencil-extent stencil 0) (ly:stencil-extent stencil 1))))
#(select-show 0 'fetaMusic "noteheads.s2")
#(select-show -4 'fetaMusic "accidentals.sharp")
#(select-show 0 'fetaBraces "brace100")
`

test('the core selects LilyPond\'s fonts from the packs a host puts as modules, natively and in wasm',
  { skip, timeout: 600_000 }, async () => {
    const file = path.join(scratch, 'select.ly')
    writeFileSync(file, SELECT)
    const reference = spawnSync('lilypond', ['--loglevel=WARNING', '-o', path.join(scratch, 'select'), file],
      { cwd: scratch, encoding: 'utf8', env: { ...process.env, LANGUAGE: 'en' } })
    assert.equal(reference.status, 0, reference.stderr)
    assert.equal(reference.stderr.split('\n').filter((line) => line.startsWith('warning: emmentaler-')).length, 3, reference.stderr)
    // The manifest's preloaded pack, then the build's fonts/ the host would ask for (W5's).
    const manifest = JSON.parse(readFileSync(path.join(dist, 'manifest.json'), 'utf8'))
    assert.deepEqual(manifest.core.preload, ['emmentaler-20', 'C059-Roman', 'C059-Bold', 'C059-Italic'])
    const modules = [['emmentaler-20', path.join(dist, manifest.modules['emmentaler-20'].file)],
      ...PACKS.slice(1).map((name) => [name, path.join(dist, 'fonts', `${name}.pack`)])]
    const out = path.join(scratch, 'select-native')
    mkdirSync(out)
    const run = runExample('compile', [...modules.flatMap(([, pack]) => ['--pack', pack]), '--out', out, file],
      { cwd: engine, encoding: 'utf8', maxBuffer: 1 << 26 })
    assert.equal(run.status, 0, run.stderr)
    assert.equal(readFileSync(path.join(out, 'select.err'), 'utf8'), reference.stderr, 'natively')
    const [wasm] = await compileInWasm([file], { modules: modules.map(([name, pack]) => [name, readFileSync(pack)]) })
    assert.equal(wasm.outputs.stderr.toString(), reference.stderr, 'in wasm')
  })
