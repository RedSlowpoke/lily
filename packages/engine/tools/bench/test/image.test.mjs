// MS1's gate, the image's half (S4, steps 12b and 19), MS2 step 3's
// frozen music, step 12's init data and step 16's whole init: lily.scm's
// library files and init data from the installed LilyPond 2.26.0's
// scm/lily/, and its ly/declarations-init.ly with the files it includes
// parsed and run after them.
// The probes are run after them three ways, with the same output: loaded from
// source natively, restored natively from the image the build shipped
// (dist/engine/scheme.image), and restored from it in the built core.wasm
// (lsb_restore). tools/bench/probe-library.scm probes the library files;
// tools/bench/probe-init.scm gives each music function of the init file
// with its signature, which must also be what LilyPond 2.26.0 gives;
// tools/bench/probe-music.scm gives lengths, starts, equal? and walkers of
// music frozen into an image it was written with, which must also be what
// LilyPond 2.26.0 gives; tools/bench/probe-data.scm gives what the init data
// defines, which must also be what LilyPond 2.26.0 gives. The shipped image
// is also what the native writer
// writes now. Skipped, and so
// unverified, without LilyPond 2.26.0, without a core.wasm and a
// scheme.image as new as the sources (npm run build), or without cargo.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { runExample } from '../../examples.mjs'
import { benchDir, coreWasm, lilyScm, runOnce, schemeImage } from '../run.mjs'

const engine = path.resolve(benchDir, '../..')
const sources = ['crates/scheme/src', 'crates/music/src', 'crates/layout/src', 'crates/parser/src', 'crates/parser/tests/support', 'crates/core/src']
  .map((d) => path.join(engine, d))
const newest = Math.max(...sources.flatMap((dir) => readdirSync(dir).map((f) => statSync(path.join(dir, f)).mtimeMs)))
const lily = lilyScm()
const skip = !lily
  ? 'LilyPond 2.26.0 is not installed'
  : [coreWasm, schemeImage].map((file) => !existsSync(file)
    ? `no dist/engine/${path.basename(file)}: run npm run build`
    : statSync(file).mtimeMs < newest ? `dist/engine/${path.basename(file)} is older than the interpreter: run npm run build` : false)
    .find(Boolean) ?? (spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed')

const probe = path.join(benchDir, 'probe-library.scm')
const probeInit = path.join(benchDir, 'probe-init.scm')
const probeMusic = path.join(benchDir, 'probe-music.scm')
const probeData = path.join(benchDir, 'probe-data.scm')
const scratch = mkdtempSync(path.join(tmpdir(), 'lily-image-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** The core's examples/bench.rs with `args`: its JSON lines. */
function native(...args) {
  const run = runExample('bench', args, { release: true, cwd: engine, encoding: 'utf8', maxBuffer: 1 << 26 })
  assert.equal(run.status, 0, run.stderr)
  return run.stdout.trim().split('\n').map((line) => JSON.parse(line))
}

test('the library files probe the same from source natively, from the image natively and from the image in wasm',
  { skip, timeout: 600_000 }, async () => {
    const written = path.join(scratch, 'scheme.image')
    native('--write-image', lily, written)
    const image = readFileSync(schemeImage)
    assert.ok(readFileSync(written).equals(image), 'the shipped scheme.image is what the writer writes from these sources')

    const [fromSource] = native('--library', lily, probe)
    const [fromImage] = native('--image', schemeImage, probe)
    const wasm = await runOnce(await WebAssembly.compile(readFileSync(coreWasm)), readFileSync(probe), image)
    assert.equal(wasm.init, 0, 'lsb_restore takes the image')
    for (const [name, run] of [['from source natively', fromSource], ['from the image natively', fromImage], ['from the image in wasm', wasm]]) {
      assert.ok(run.ok, `${name}: ${run.output.slice(-400)}`)
    }
    const probes = readFileSync(probe, 'utf8').split('\n').filter((line) => line.startsWith('(probe ')).length
    assert.equal(fromSource.output.trim().split('\n').length, probes, 'one line per probe')
    assert.equal(fromImage.output, fromSource.output, 'from the image natively')
    assert.equal(wasm.output, fromSource.output, 'from the image in wasm')
    assert.equal(wasm.stats.applications, fromImage.applications, 'the same applications natively and in wasm')
  })

test('the whole init\'s variables probe the same from source natively, from the image natively and from the image in wasm',
  { skip, timeout: 600_000 }, async () => {
    const [fromSource] = native('--library', lily, probeInit)
    const [fromImage] = native('--image', schemeImage, probeInit)
    const wasm = await runOnce(await WebAssembly.compile(readFileSync(coreWasm)), readFileSync(probeInit), readFileSync(schemeImage))
    for (const [name, run] of [['from source natively', fromSource], ['from the image natively', fromImage], ['from the image in wasm', wasm]]) {
      assert.ok(run.ok, `${name}: ${run.output.slice(-400)}`)
    }
    assert.equal(fromSource.output.trim().split('\n').length, INIT_VARIABLES, 'one line per variable')
    assert.equal(fromImage.output, fromSource.output, 'from the image natively')
    assert.equal(wasm.output, fromSource.output, 'from the image in wasm')
  })

/** The variables declarations-init.ly and its includes record (MS2 step 16),
 * as LilyPond 2.26.0 records them but EXPORTS; 199 are music functions
 * (crates/parser/tests/session.rs). */
const INIT_VARIABLES = 654

/** The variables of define-session-public, which LilyPond's
 * session-start-record adds to the init's module, so that it records them
 * too; the engine's (lily) exports them (lily.rs). */
const EXPORTS = ['accidental-styles', 'all-grob-descriptions', 'bar-glyph-alist', 'default-language', 'input-language',
  'language-pitch-names', 'pitch-names-german-type', 'pitchnames', 'span-bar-glyph-alist', 'toplevel-music-functions']

test('the whole init\'s variables, music functions\' signatures among them, are LilyPond\'s', { skip, timeout: 600_000 }, () => {
  const ly = path.join(scratch, 'probe-init.ly')
  writeFileSync(ly, `\\version "2.26.0"\n#(let ((module (current-module)))
  (primitive-load ${JSON.stringify(probeInit)})
  (set-current-module module))\n`)
  const lilypond = spawnSync('lilypond', ['-s', '-o', scratch, ly], { encoding: 'utf8', maxBuffer: 1 << 26 })
  assert.equal(lilypond.status, 0, lilypond.stderr)
  // Guile writes an uninterned symbol with its address; the engine writes its name (#{…}#).
  const reference = lilypond.stdout.replace(/#<uninterned-symbol (.*?) [0-9a-f]+>/g, '$1')
    .split('\n').filter((line) => !EXPORTS.includes(line.split(' ')[0])).join('\n')
  const [fromSource] = native('--library', lily, probeInit)
  const lines = reference.trim().split('\n')
  assert.equal(lines.length, INIT_VARIABLES)
  assert.equal(fromSource.output, reference)
})

test('frozen music gives LilyPond\'s lengths, starts, equal? and walkers from source natively, from an image natively and in wasm',
  { skip, timeout: 600_000 }, async () => {
    // The music is made, and frozen, before the image is written; the probes
    // read it from the image.
    const text = readFileSync(probeMusic, 'utf8')
    const marker = text.indexOf('\n;;; probes\n')
    assert.ok(marker > 0, 'probe-music.scm has its ;;; probes line')
    const setup = path.join(scratch, 'music-setup.scm')
    const probes = path.join(scratch, 'music-probes.scm')
    writeFileSync(setup, text.slice(0, marker + 1))
    const image = path.join(scratch, 'music.image')
    native('--write-image', lily, image, setup)

    const ly = path.join(scratch, 'probe-music.ly')
    writeFileSync(ly, `\\version "2.26.0"\n#(let ((module (current-module)))
  (primitive-load ${JSON.stringify(probeMusic)})
  (set-current-module module))\n`)
    const lilypond = spawnSync('lilypond', ['-s', '-o', scratch, ly], { encoding: 'utf8', maxBuffer: 1 << 26 })
    assert.equal(lilypond.status, 0, lilypond.stderr)
    const reference = lilypond.stdout
    assert.equal(reference.trim().split('\n').length, MUSIC_PROBES, 'one line per probe')

    // The engine's runs end by checking that the music is still frozen:
    // nothing but the walkers' copies was thawed.
    const frozen = '(write (map (lambda (e) (%music-frozen? (cdr e))) probe-frozen))\n'
    const all = `(${Array.from({ length: MUSIC_TYPES }, () => '#t').join(' ')})`
    const source = path.join(scratch, 'music-source.scm')
    writeFileSync(source, text + frozen)
    writeFileSync(probes, text.slice(marker + 1) + frozen)
    const [fromSource] = native('--library', lily, source)
    const [fromImage] = native('--image', image, probes)
    const wasm = await runOnce(await WebAssembly.compile(readFileSync(coreWasm)), readFileSync(probes), readFileSync(image))
    assert.equal(wasm.init, 0, 'lsb_restore takes the image')
    for (const [name, run] of [['from source natively', fromSource], ['from the image natively', fromImage], ['from the image in wasm', wasm]]) {
      assert.ok(run.ok, `${name}: ${run.output.slice(-400)}`)
      assert.equal(run.output, reference + all, name)
    }
  })

/** The music probe-music.scm makes, and its lines: one per music and eight more. */
const MUSIC_TYPES = 23
const MUSIC_PROBES = MUSIC_TYPES + 8

test('lily.scm\'s init data is LilyPond\'s from source natively, from the image natively and in wasm',
  { skip, timeout: 600_000 }, async () => {
    const ly = path.join(scratch, 'probe-data.ly')
    writeFileSync(ly, `\\version "2.26.0"\n#(let ((module (current-module)))
  (primitive-load ${JSON.stringify(probeData)})
  (set-current-module module))\n`)
    const lilypond = spawnSync('lilypond', ['-s', '-o', scratch, ly], { encoding: 'utf8', maxBuffer: 1 << 26 })
    assert.equal(lilypond.status, 0, lilypond.stderr)
    // LilyPond's C++ adds interfaces of its own (ADD_INTERFACE), which come
    // with layout's classes: those define-grob-interfaces.scm adds are compared.
    const added = new Set([...readFileSync(path.join(lily, 'define-grob-interfaces.scm'), 'utf8')
      .matchAll(/\(ly:add-interface\s+'([^\s()]+)/g)].map((m) => m[1]))
    assert.equal(added.size, DATA_INTERFACES)
    const reference = lilypond.stdout.split('\n')
      .filter((line) => !line.startsWith('interface (') || added.has(line.slice('interface ('.length).split(' ')[0]))
      .join('\n')
    assert.equal(reference.split('\n').filter((line) => line.startsWith('grob ')).length, DATA_GROBS)

    const [fromSource] = native('--library', lily, probeData)
    const [fromImage] = native('--image', schemeImage, probeData)
    const wasm = await runOnce(await WebAssembly.compile(readFileSync(coreWasm)), readFileSync(probeData), readFileSync(schemeImage))
    assert.equal(wasm.init, 0, 'lsb_restore takes the image')
    for (const [name, run] of [['from source natively', fromSource], ['from the image natively', fromImage], ['from the image in wasm', wasm]]) {
      assert.ok(run.ok, `${name}: ${run.output.slice(-400)}`)
      assert.equal(run.output, reference, name)
    }
  })

/** The interfaces define-grob-interfaces.scm adds, and the grobs of define-grobs.scm. */
const DATA_INTERFACES = 88
const DATA_GROBS = 167

test('lsb_restore refuses an image cut short', { skip, timeout: 60_000 }, async () => {
  const mod = await WebAssembly.compile(readFileSync(coreWasm))
  const image = readFileSync(schemeImage)
  const cut = await runOnce(mod, Buffer.from('(display 1)'), image.subarray(0, image.length - 1))
  assert.equal(cut.init, 1)
  assert.equal(cut.ok, false)
})
