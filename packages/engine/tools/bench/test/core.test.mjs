// K2's first part (MS2 step 10): the built core.wasm is crates/core behind
// chapter 08 §4.2's ABI. Its only import is env.lily_now, and it exports the
// nine lily_* functions beside the bench's lsb_* and the parser's lsp_*.
// core.image is the core's image: scheme.image after a mark
// (lily_engine_core::write_image). Driven as the studio's driver drives it
// (packages/studio/src/web/engine/driver.ts), it starts from core.image, asks
// for a root it was not given, and compiles each case of the error corpus to
// two outputs, stderr and diagnostics.json (step 11), its stderr the one
// lsp_parse gives for it in the same instance from scheme.image, and since
// step 40 what its translations say (D99).
// tools/compare/test/c0-cases.test.mjs checks both against LilyPond's. Since
// step 19 a compile asks for the files a score includes and a part's files the
// same way (lily_step's 2 and lily_needs, kind 0), and goes on once lily_put
// gives them: case 07's missing include is LilyPond's .err, and T01's, T03's,
// T19's and T21's includes (T19's with the display part) compile through
// core.wasm, answered from disk as crates/core/examples/compile.rs answers,
// to the outputs the example gives natively. Since step 30 a part's files are
// asked for at one stop: T05's chords part and T29's diagrams part, three
// files each, through tools/compare/core.mjs and the example, with no name
// asked twice. Skipped, and so unverified,
// without LilyPond 2.26.0 or without a core.wasm and images as new as the
// sources (npm run build); the scores also without cargo or the corpus.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { corpus } from '../../corpus/fetch.mjs'
import { compileInWasm, moduleFile, preloadedModules } from '../../compare/core.mjs'
import { ERRORS, errorCases } from '../../compare/stderr.mjs'
import { runExample } from '../../examples.mjs'
import { benchDir, coreWasm, instantiateCore, lilyScm, schemeImage } from '../run.mjs'

const engine = path.resolve(benchDir, '../..')
const coreImage = path.join(engine, 'dist/engine/core.image')
const sources = ['crates/core/src', 'crates/scheme/src', 'crates/music/src', 'crates/layout/src', 'crates/parser/src', 'crates/parser/tests/support']
  .map((d) => path.join(engine, d))
const newest = Math.max(...sources.flatMap((dir) => readdirSync(dir).map((f) => statSync(path.join(dir, f)).mtimeMs)))
const lily = lilyScm()
const skip = !lily
  ? 'LilyPond 2.26.0 is not installed'
  : [coreWasm, coreImage, schemeImage].map((file) => !existsSync(file)
    ? `no dist/engine/${path.basename(file)}: run npm run build`
    : statSync(file).mtimeMs < newest ? `dist/engine/${path.basename(file)} is older than the sources: run npm run build` : false)
    .find(Boolean) ?? false

// protocol.ts' Step and Need.
const DONE = 0
const MORE = 1
const NEEDS = 2
const FILE = 0
const MODULE = 1

/** How the core names LilyPond's own files (`<datadir>/ly/…`, `<datadir>/scm/lily/…`). */
const DATADIR = '<datadir>'

/** A fresh instance of the core, with helpers over its memory. */
async function core() {
  const e = (await instantiateCore(new WebAssembly.Module(readFileSync(coreWasm)))).exports
  const put = (bytes) => {
    const at = e.lily_alloc(bytes.length)
    new Uint8Array(e.memory.buffer, at, bytes.length).set(bytes)
    return [at, bytes.length]
  }
  const words = (at, n) => [...new Uint32Array(e.memory.buffer, at, n)]
  const bytes = (at, length) => Buffer.from(e.memory.buffer, at, length)
  return { e, put, words, bytes }
}

test('core.wasm imports env.lily_now and its split modules\' placeholders only and exports 08 §4.2\'s ABI beside lsb_* and lsp_*', { skip }, () => {
  const mod = new WebAssembly.Module(readFileSync(coreWasm))
  const imports = WebAssembly.Module.imports(mod)
  assert.deepEqual(imports[0], { module: 'env', name: 'lily_now', kind: 'function' })
  // K4 (MS4 step 20, D230): the tablature's and the chord names' placeholders, functions each; since MS8 step 11
  // the syntax-case part's (D273).
  assert.deepEqual([...new Set(imports.slice(1).map((i) => `${i.module} ${i.kind}`))].sort(),
    ['placeholder.chord-names function', 'placeholder.syntax-case function', 'placeholder.tablature function'])
  const exports = WebAssembly.Module.exports(mod).map((x) => x.name)
  for (const name of ['memory', 'lily_alloc', 'lily_free', 'lily_start', 'lily_put', 'lily_compile', 'lily_step', 'lily_needs',
    'lily_outputs', 'lily_output', 'lsb_init', 'lsb_restore', 'lsb_eval', 'lsp_note_names', 'lsp_parse']) {
    assert.ok(exports.includes(name), name)
  }
})

test('core.image is scheme.image after its mark', { skip }, () => {
  assert.ok(readFileSync(coreImage).equals(Buffer.concat([Buffer.from('LILYCORE'), readFileSync(schemeImage)])))
})

/** A zero-duration score's warnings, each with its two lines of excerpt, and the cases that give them. */
const ZERO = /^.*: warning: (?:skipping zero-duration score|to suppress this, consider adding a spacer rest)\n.*\n.*\n/gm
const ZERO_CASES = ['17-function-argument', '18-optional-argument', '30-unattached']

test('the core compiles the error corpus through lily_* to the stderr lsp_parse gives, and its diagnostics.json', { skip, timeout: 120_000 }, async () => {
  const { e, put, words, bytes } = await core()
  assert.equal(e.lily_start(...put(readFileSync(coreImage))), 0, 'lily_start takes core.image')
  assert.equal(e.lsb_restore(...put(readFileSync(schemeImage))), 0, 'lsb_restore takes scheme.image')
  assert.equal(e.lsp_note_names(...put(readFileSync(path.join(lily, 'define-note-names.scm')))), 0)
  // The packs a host preloads (D105), which the cases' layouts draw with.
  for (const [name, pack] of preloadedModules()) e.lily_put(MODULE, ...put(Buffer.from(name)), ...put(pack))
  const out = e.lily_alloc(16)
  const compile = (revision, root) => {
    e.lily_compile(revision, ...put(Buffer.from(root)), 0)
    let step
    while ((step = e.lily_step(10)) === MORE);
    return step
  }
  const outputs = () => Array.from({ length: e.lily_outputs() }, (_, i) => {
    e.lily_output(i, out)
    const [name, nameLen, data, dataLen] = words(out, 4)
    return [bytes(name, nameLen).toString(), Buffer.from(bytes(data, dataLen))]
  })
  const cases = errorCases()
  assert.ok(cases.length >= 25)
  let revision = 0
  for (const name of cases) {
    const root = `${name}.ly`
    assert.equal(compile(++revision, root), NEEDS, `${root}: the root is asked for`)
    e.lily_needs(out)
    const [kind, at, length] = words(out, 3)
    assert.deepEqual([kind, bytes(at, length).toString()], [FILE, root])
    const text = readFileSync(path.join(ERRORS, root))
    e.lily_put(FILE, ...put(Buffer.from(root)), ...put(text))
    // An include the corpus does not have: none, as the page answers; a
    // text face's outlines, which a skyline asks for (D262), the manifest's
    // pack, as a host fetches it; no other module.
    const asked = []
    while (compile(revision, root) === NEEDS) {
      e.lily_needs(out)
      const [kind, at, length] = words(out, 3)
      const name = bytes(at, length).toString()
      if (kind === MODULE) {
        assert.match(name, /\.outlines$/, root)
        e.lily_put(kind, ...put(Buffer.from(name)), ...put(readFileSync(moduleFile(name))))
        continue
      }
      asked.push(name)
      e.lily_put(kind, ...put(Buffer.from(name)), 0, 0)
    }
    const [[output, stderr], [json, diagnostics], ...made] = outputs()
    // The playback map of a case with music, after the two (MS2 step 26),
    // and its pages before it, drawn with the preloaded packs (MS3 step 50),
    // and its MIDI file between them (MS5 step 1).
    const more = made.filter(([n]) => !n.endsWith('.svg'))
    assert.deepEqual([output, json, ...more.map(([n]) => n)],
      ['stderr', 'diagnostics.json', ...more.length ? [`${name}.midi`, `${name}.timing.json`] : []], root)
    assert.ok(Array.isArray(JSON.parse(diagnostics.toString())), root)
    assert.ok(stderr.length > 0, `${root}: a case prints something`)
    if (asked.length > 0) {
      // lsp_parse includes nothing: the missing include is LilyPond's own error.
      assert.deepEqual(asked, ['no-such-file.ily', `${DATADIR}/ly/no-such-file.ily`], root)
      assert.equal(stderr.toString(), readFileSync(path.join(ERRORS, `${name}.err`), 'utf8'), root)
      continue
    }
    // What the translations say besides (MS2 step 40, D99): a zero-duration
    // score's two warnings, for its \layout and for capture.ly's \midi.
    const said = stderr.toString().match(ZERO) ?? []
    assert.equal(said.length, ZERO_CASES.includes(name) ? 4 : 0, root)
    const [p, n] = words(e.lsp_parse(...put(Buffer.from(root)), ...put(text)), 2)
    assert.equal(stderr.toString().replace(ZERO, ''), bytes(p, n).toString(), root)
  }
  // A root the page does not have (lily_put's data 0): LilyPond's lines for it.
  e.lily_put(FILE, ...put(Buffer.from('nosuch.ly')), 0, 0)
  assert.equal(compile(++revision, 'nosuch.ly'), DONE)
  assert.equal(outputs()[0][1].toString(), 'warning: cannot find file: `nosuch.ly\'\nfatal error: failed files: "nosuch.ly"\n')
})

/** The scores whose includes part 3 needs (step 19), by their corpus ids. */
const INCLUDES = ['T01', 'T03', 'T19', 'T21']
const rows = Object.fromEntries(corpus().map((row) => [row.id, row]))
const cargo = spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed'
const scores = skip || cargo || (INCLUDES.every((id) => existsSync(rows[id].path)) ? false : 'the corpus is not fetched')
const scratch = mkdtempSync(path.join(tmpdir(), 'lily-core-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

test('core.wasm asks for T01\'s, T03\'s, T19\'s and T21\'s includes and parts, and gives what the core gives natively', { skip: scores, timeout: 600_000 }, async () => {
  const files = INCLUDES.map((id) => rows[id].path)
  const run = runExample('compile', ['--out', scratch, ...files], { encoding: 'utf8' })
  // 1: a score failed, as T03 does; 2: the runner could not run.
  assert.ok(run.status === 0 || run.status === 1, run.stderr)

  const { e, put, words, bytes } = await core()
  assert.equal(e.lily_start(...put(readFileSync(coreImage))), 0)
  for (const [name, pack] of preloadedModules()) e.lily_put(MODULE, ...put(Buffer.from(name)), ...put(pack))
  const out = e.lily_alloc(16)
  const datadir = path.resolve(lily, '../..')
  let revision = 0
  const asked = {}
  for (const [i, file] of files.entries()) {
    const id = INCLUDES[i]
    asked[id] = []
    e.lily_put(FILE, ...put(Buffer.from(file)), ...put(readFileSync(file)))
    e.lily_compile(++revision, ...put(Buffer.from(file)), 0)
    let step
    while ((step = e.lily_step(10)) !== DONE) {
      if (step === MORE) continue
      e.lily_needs(out)
      const [kind, at, length] = words(out, 3)
      const name = bytes(at, length).toString()
      // A pack beyond the preloaded (W5: T21's braces and size 26), as the compile example fetches it.
      if (kind === MODULE) {
        e.lily_put(MODULE, ...put(Buffer.from(name)), ...put(readFileSync(moduleFile(name))))
        e.lily_compile(revision, ...put(Buffer.from(file)), 0)
        continue
      }
      assert.equal(kind, FILE, name)
      assert.ok(!asked[id].includes(name), `${id}: ${name} is asked for twice`)
      asked[id].push(name)
      const disk = name.startsWith(`${DATADIR}/`) ? path.join(datadir, name.slice(DATADIR.length + 1)) : name
      e.lily_put(FILE, ...put(Buffer.from(name)), ...(existsSync(disk) ? put(readFileSync(disk)) : [0, 0]))
      e.lily_compile(revision, ...put(Buffer.from(file)), 0)
    }
    const outputs = Object.fromEntries(Array.from({ length: e.lily_outputs() }, (_, i) => {
      e.lily_output(i, out)
      const [at, length, data, size] = words(out, 4)
      return [bytes(at, length).toString(), Buffer.from(bytes(data, size))]
    }))
    const stem = path.basename(file, '.ly')
    assert.deepEqual(outputs.stderr, readFileSync(path.join(scratch, `${stem}.err`)), id)
    assert.deepEqual(outputs['diagnostics.json'], readFileSync(path.join(scratch, `${stem}.json`)), id)
    assert.doesNotMatch(outputs.stderr.toString(), /cannot find file/, id)
  }
  const dir = (id) => path.dirname(rows[id].path)
  // Each include's candidates at one stop since MS7 step 7 (OPEN F, D263): beside the score, then LilyPond's ly/,
  // which the studio's driver answers itself; T19's display part a module (D263).
  assert.deepEqual(asked, {
    T01: [path.join(dir('T01'), 'melody.ily'), `${DATADIR}/ly/melody.ily`],
    T03: [path.join(dir('T03'), 'parts/broken-part.ily'), `${DATADIR}/ly/parts/broken-part.ily`],
    T19: [path.join(dir('T19'), 'articulate.ly'), `${DATADIR}/ly/articulate.ly`],
    T21: [path.join(dir('T21'), 'example-header.ily'), `${DATADIR}/ly/example-header.ily`],
  })
  // T01 parses as LilyPond's does; T03's errors start in the file it includes.
  assert.equal(readFileSync(path.join(scratch, 'hello.err'), 'utf8'), '')
  assert.ok(readFileSync(path.join(scratch, 'broken.err'), 'utf8')
    .startsWith(`${path.join(dir('T03'), 'parts/broken-part.ily')}:1:19: error: unknown command: \`\\alsoUndefined'\n`))
})

/**
 * The scores that load a part of three files (step 30), by their corpus ids, with the part's files; T05's
 * `chords` asked for as its module since MS4 step 20 (I2, the core's PART_MODULES), the manifest's chords.part.
 */
const PARTS = {
  T05: ['chords'],
  // Its module since MS7 step 7 (D263), not its three files.
  T29: ['diagrams'],
}
const parts = skip || cargo || (Object.keys(PARTS).every((id) => existsSync(rows[id].path)) ? false : 'the corpus is not fetched')

test('core.wasm and the native example ask for T05\'s and T29\'s parts at one stop, no name twice', { skip: parts, timeout: 600_000 }, async () => {
  const files = Object.keys(PARTS).map((id) => rows[id].path)
  // Each in a fresh instance, and both in one, as c1.test.mjs compiles them.
  const fresh = await Promise.all(files.map(async (file) => (await compileInWasm([file]))[0]))
  for (const results of [fresh, await compileInWasm(files)]) {
    for (const [i, { asked, packs, stops, parses }] of results.entries()) {
      const id = Object.keys(PARTS)[i]
      if (id === 'T05') {
        // Its module, one stop for all three files, and none of LilyPond's own
        // files; since MS7 step 5 (D262) its Roman outlines a stop of their own.
        assert.deepEqual({ asked, parts: packs.filter(([name]) => name === 'chords'), parses }, { asked: [], parts: [['chords', true]], parses: 3 }, id)
        continue
      }
      // Its module, as T05's, and none of LilyPond's own files (D263).
      assert.deepEqual({ asked, parts: packs.filter(([name]) => name === 'diagrams') }, { asked: [], parts: [['diagrams', true]] }, id)
      // T29's staff size, braces and (since MS4 step 10, its ties drawn) its
      // bold italic text are a stop of their own, for their packs (W5); since
      // MS7 step 5 (D262) its text's outlines more: in steps a compile
      // stops where a skyline first needs them, Roman's before the bold
      // italic face is met, its outlines after; since MS8 step 3a (D272) its
      // lines' heights are read in steps of their own, so Italic's come with
      // Roman's or at a stop of their own, as the steps end (6 or 7 parses;
      // one call gathers them at its end: crates/core/tests/engine.rs, 5).
      assert.equal(stops, 0, id)
      assert.ok(parses === 6 || parses === 7, `${id}: ${parses} parses`)
    }
  }
  // --time: name, ms, the names asked, the stops, then the times; T05 and T29 ask for their parts' modules, no file.
  const run = runExample('compile', ['--time', '--out', scratch, ...files], { encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  const lines = run.stdout.trim().split('\n').map((line) => line.split('\t'))
  assert.deepEqual(lines.map(([file, , asked, stops]) => [file, asked, stops]), files.map((file) => [file, '0', '0']))
  // The times' last two (MS5 step 7): the \midi translations' and the MIDI files' writing, which every compile makes;
  // then the parse's segments done again and parsed (MS6 step 3); then 05 §7's steps (MS6 step 9, D251): the longest,
  // what ended it, how many, the visible page's time, what ended the step before it, the collections and their
  // time, and the longest with the collections in it.
  for (const line of lines) {
    assert.equal(line.length, 24, line.join(' '))
    assert.ok(Number(line[12]) > 0 && Number(line[13]) > 0 && Number(line[12]) <= Number(line[5]), line.join(' '))
  }
})
