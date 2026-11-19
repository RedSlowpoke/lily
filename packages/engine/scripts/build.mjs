// Builds the engine as chapter 08 §1.3 says (K1) into dist/engine/, where the
// studio's `esbuild.mjs --site` takes it from, then prints the size report
// (scripts/sizes.mjs). The core is crates/core's cdylib (K2, MS2 step 10).
//
//   node scripts/build.mjs      or `npm run build` in packages/engine
//
//   WASM_OPT   Binaryen's wasm-opt (default: the one on the PATH; 133 measured)
//   FONT       emmentaler-20.svg, the spike's pack, with CORE=spike (default: the installed LilyPond's)
//   OTF        LilyPond's fonts/otf, the font and text packs' (default: the installed LilyPond's)
//   LILY_SCM   LilyPond 2.26.0's scm/lily, the interpreter's image's (default: the installed LilyPond's)
//   VARIANT    ia (default): rust-toolchain.toml's nightly, -Cpanic=immediate-abort
//              and -Zbuild-std, as §1.3; stable: panic = "abort" on stable, with
//              panic strings; simd: ia with +simd128
//   DEBUG_CORE 1 (default): core-debug.wasm too (08 §1.3, D262); 0: not
//   CORE       core (default): crates/core; spike: the spike's toy engine with its
//              prelude as core.image and its own pack, which MS0's harness pages
//              (index.html, preview.html, l532.mjs) need for their SVG pages
//
// Since MS7 step 5 (E14, D262) each text face's outlines are a pack of their own,
// `<face>.outlines`, which the manifest names on demand and the core asks for
// when a skyline first needs one; the core's download keeps the faces' metrics.
// Beside the core it writes core-debug.wasm, the same core unsplit, on stable
// with panic = "abort" and its panic messages passed to `env.lily_panic`
// (08 §1.3), which the manifest names as `core.debug` and no budget counts;
// the commit it was built from as the manifest's `source` (D56, OPEN I5); and
// the licences of what it serves, from licenses/, as the manifest's `licenses`.
// The manifest names pdf.wasm as `pdf` (kind `wasm`) and each text face's OTF as
// `<face>.otf` (kind `font`), which a host fetches on the first Export PDF and
// gives the module with the display list the core's `lily_pdf` gives (D271).
// It writes core.wasm (wasm-opt -Oz with the names stripped), pdf.wasm (W7's module, D270), core.image,
// scheme.image, emmentaler-20.pack, the three C059 packs and manifest.json, each with a brotli-11 .br beside it
// that a server sends as it is (H5, 09 §8 P5), and target/engine/core.names.wasm,
// the same core with its names, for the report. Since MS4 step 20 (K4, D223) core.wasm is
// wasm-split's primary: tablature.wasm, figured-bass.wasm and chord-names.wasm, the
// functions reachable only from their crates (scripts/split.mjs), are split from it
// and named by the manifest as `split`, and S6's parts (chords.part, fretted.part and,
// since MS7 step 7, the other five; I2, D263) as `part`, and LilyPond's own ly/ files, each
// ly-<name> beside the core, as `file` modules `ly/<name>`, which the studio's driver gives
// the engine as `<datadir>/ly/<name>` (D263). The engine's font packs and text
// metric packs (tools/packs/build.mjs and text.mjs, W1) go into fonts/;
// emmentaler-20.pack is fonts/' W1 pack of Emmentaler-20, which the manifest
// preloads as the module emmentaler-20 and the core gives layout's fonts (06 §5.2,
// D105), and C059-Roman.pack, C059-Bold.pack and C059-Italic.pack its text packs,
// preloaded under those names (MS3 step 8). Since MS3 step 50 (W5, D208) every other
// pack of fonts/ is copied beside them and named by the manifest too, not preloaded:
// each Emmentaler size, the four parts of emmentaler-brace and every text face, which
// a host fetches when the core asks for it (lily_needs' MODULE). With CORE=spike the manifest's emmentaler-20 is the spike's
// own pack (crates/spike/harness/pack.mjs), emmentaler-20.spike.pack, which its
// core reads.
// scheme.image is the engine's interpreter after lily.scm's library files and
// init data (color.scm, lily-library.scm, output-lib.scm, markup-macros.scm,
// define-note-names.scm, define-grobs.scm and the rest; MS2 step 12), and after
// ly/declarations-init.ly and the files it includes parsed and run
// by the engine's parser (chapter 03 §5.6, S4; MS2 step 16), written natively
// by the core's examples/bench.rs from LilyPond's own files, which the
// repository never holds; the bench ABI's lsb_restore reads it (tools/bench/run.mjs,
// scheme.html). core.image, which the manifest names and lily_start restores,
// is the core's image: the same interpreter's image after a mark
// (lily_engine_core::write_image).
import { spawnSync } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs'
import { dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { brotliCompressSync, constants } from 'node:zlib'
import { buildPacks, installedFonts, SIZES } from '../tools/packs/build.mjs'
import { buildTextPacks, CORE_STYLES, otfFile, outlinesModule, STYLES } from '../tools/packs/text.mjs'
import { lilyScm } from '../tools/bench/run.mjs'
import { report } from './sizes.mjs'
import { manifestText, shortenNames, splitSets } from './split.mjs'

const root = fileURLToPath(new URL('..', import.meta.url)).replace(/\/$/, '')
const harness = `${root}/crates/spike/harness`
const CORE = { core: 'lily-engine-core', spike: 'lily-engine-spike' }[process.env.CORE ?? 'core']
const TARGET = 'wasm32-unknown-unknown'
// Chapter 00 §1.1's features, as rustc enables them for this target; wasm-opt must be given the same (08 §1.1).
const FEATURES = ['bulk-memory', 'sign-ext', 'mutable-globals', 'nontrapping-float-to-int', 'multivalue', 'reference-types']
// B4's 256 MiB, so a too-large score is an error, not a killed tab (08 §1.3).
const MEMORY = '-Clink-arg=--max-memory=268435456'

const variant = process.env.VARIANT ?? 'ia'
const wasmOpt = process.env.WASM_OPT ?? 'wasm-opt'
const pin = readFileSync(`${root}/rust-toolchain.toml`, 'utf8').match(/^channel\s*=\s*"([^"]+)"/m)[1]
const toolchain = variant === 'stable' ? 'stable' : pin

function fail(message) {
  console.error(`build: ${message}`)
  process.exit(1)
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: root, encoding: 'utf8', ...options })
  if (result.error) return { ok: false, error: result.error }
  return { ok: result.status === 0, stdout: result.stdout?.trim() ?? '', raw: result.stdout, stderr: result.stderr?.trim() }
}

// --- the tools ---------------------------------------------------------------

if (!['ia', 'stable', 'simd'].includes(variant)) fail(`unknown VARIANT ${variant}`)
if (!CORE) fail(`unknown CORE ${process.env.CORE}`)

// rustup would install a missing toolchain on first use; say what is missing instead.
const sysroot = run('rustc', [`+${toolchain}`, '--print', 'sysroot'], { env: { ...process.env, RUSTUP_AUTO_INSTALL: '0' } })
const install = `rustup toolchain install ${toolchain} --component rust-src --target ${TARGET}`
if (!sysroot.ok) fail(`the ${toolchain} toolchain is not installed: ${install}`)
if (!existsSync(`${sysroot.stdout}/lib/rustlib/${TARGET}`)) fail(`${toolchain} lacks the ${TARGET} target: ${install}`)
if (toolchain === pin && !existsSync(`${sysroot.stdout}/lib/rustlib/src/rust/library`)) fail(`${pin} lacks rust-src, which -Zbuild-std needs: ${install}`)

const binaryen = run(wasmOpt, ['--version'])
if (!binaryen.ok) {
  fail(`Binaryen's wasm-opt is needed, on the PATH or as WASM_OPT (${wasmOpt} did not run). ` +
    'Chapter 08 used version 133: https://github.com/WebAssembly/binaryen/releases/tag/version_133')
}
if (!/version 133\b/.test(binaryen.stdout)) console.warn(`build: ${binaryen.stdout}; chapter 08 measured with Binaryen 133`)

let font = process.env.FONT
if (!font && CORE === 'lily-engine-spike') {
  const lilypond = run('sh', ['-c', 'command -v lilypond'])
  if (!lilypond.ok) fail('LilyPond is not installed; give emmentaler-20.svg as FONT')
  const share = `${dirname(realpathSync(lilypond.stdout))}/../share/lilypond`
  const version = run('ls', [share]).stdout.split('\n')[0]
  font = `${share}/${version}/fonts/svg/emmentaler-20.svg`
}
if (font && !existsSync(font)) fail(`no font at ${font}; give emmentaler-20.svg as FONT`)
let otf = process.env.OTF
try {
  otf ??= installedFonts()
} catch (error) {
  fail(`${error.message.replace('--otf', 'OTF')}`)
}
if (!existsSync(`${otf}/emmentaler-20.otf`)) fail(`no Emmentaler in ${otf}; give LilyPond's fonts/otf as OTF`)
const scm = process.env.LILY_SCM ?? lilyScm()
if (scm && !existsSync(`${scm}/music-functions.scm`)) fail(`no music-functions.scm in ${scm}; give LilyPond 2.26.0's scm/lily as LILY_SCM`)

// --- the build ---------------------------------------------------------------

// The wasm-names profile, as §1.3: wasm-split (K4) will need the names, and the report needs them now.
const flags = variant === 'stable' ? [MEMORY] : ['-Zunstable-options', '-Cpanic=immediate-abort', MEMORY]
if (variant === 'simd') {
  flags.push('-Ctarget-feature=+simd128')
  FEATURES.push('simd')
}
const cargo = run('cargo', [`+${toolchain}`, 'build', '-q', '-p', CORE, '--lib', '--profile', 'wasm-names', '--target', TARGET,
  ...(variant === 'stable' ? [] : ['-Zbuild-std=std,core,alloc,panic_abort'])], { env: { ...process.env, RUSTFLAGS: flags.join(' ') }, stdio: 'inherit' })
if (!cargo.ok) fail('cargo build failed')

const built = `${root}/target/${TARGET}/wasm-names/${CORE.replaceAll('-', '_')}.wasm`
const out = `${root}/dist/engine`
rmSync(out, { recursive: true, force: true })
mkdirSync(out, { recursive: true })
mkdirSync(`${root}/target/engine`, { recursive: true })
const features = FEATURES.map((feature) => `--enable-${feature}`)
// K4 (08 §2.3, D223): the notation modules split from the core by call graph (split.mjs), with the names,
// before wasm-opt; then each output through wasm-opt -Oz. The core imports a module's placeholders from
// `placeholder.<module>`, which the driver's instance answers until the module is loaded; a module imports
// the core's memory, tables, stack pointer and functions from `primary`. None for the spike.
const split = `${root}/target/engine/split`
rmSync(split, { recursive: true, force: true })
mkdirSync(split, { recursive: true })
let primary = built
let modules = []
if (CORE === 'lily-engine-core') {
  // Binaryen's other tools, beside the wasm-opt given, as sizes.mjs finds wasm-dis.
  const tool = (name) => (process.env.WASM_OPT ? `${dirname(wasmOpt)}/${name}` : name)
  let sets
  try {
    sets = splitSets(built, tool('wasm-dis')).sets
  } catch (error) {
    fail(`the split's call graph: ${error.message}`)
  }
  modules = Object.keys(sets).filter((module) => sets[module].length)
  writeFileSync(`${split}/manifest.txt`, manifestText(sets))
  primary = `${split}/core.wasm`
  const splitting = run(tool('wasm-split'), ['--multi-split', built, '--manifest', `${split}/manifest.txt`, '--out-prefix', `${split}/`, '-o', primary,
    '--placeholder-namespace-prefix', 'placeholder', '--export-prefix', '%', '-g', ...features], { stdio: 'inherit' })
  if (!splitting.ok) fail('wasm-split failed')
  const short = shortenNames(readFileSync(primary), modules.map((module) => readFileSync(`${split}/${module}.wasm`)))
  writeFileSync(primary, short.primary)
  modules.forEach((module, i) => writeFileSync(`${split}/${module}.wasm`, short.secondaries[i]))
}
// core-debug.wasm (08 §1.3, D262): the core unsplit, on stable with panic = "abort", in a target directory of
// its own so that neither build undoes the other's; its names kept, for a trace in the browser's tools.
const debugCore = CORE === 'lily-engine-core' && process.env.DEBUG_CORE !== '0'
if (debugCore) {
  const debug = run('cargo', ['+stable', 'build', '-q', '-p', CORE, '--lib', '--profile', 'wasm-names', '--target', TARGET,
    '--features', 'debug', '--target-dir', `${root}/target/debug-core`], { env: { ...process.env, RUSTFLAGS: MEMORY }, stdio: 'inherit' })
  if (!debug.ok) fail('cargo build of core-debug failed')
}
for (const [input, output, keep] of [[primary, `${out}/core.wasm`, '--strip-debug'], [primary, `${root}/target/engine/core.names.wasm`, '--debuginfo'],
  ...(debugCore ? [[`${root}/target/debug-core/${TARGET}/wasm-names/${CORE.replaceAll('-', '_')}.wasm`, `${out}/core-debug.wasm`, '--debuginfo']] : []),
  ...modules.map((module) => [`${split}/${module}.wasm`, `${out}/${module}.wasm`, '--strip-debug'])]) {
  const optimised = run(wasmOpt, ['-Oz', ...features, keep, '--strip-producers', input, '-o', output], { stdio: 'inherit' })
  if (!optimised.ok) fail('wasm-opt failed')
}
// W7's PDF module (06 §7.3, D270): the PDF crate alone as a cdylib (its rlib stays no_std for the core), with its own
// memory, which the host fetches on the first export and drops after it, given the display list the core's ABI gives
// (`lily_pdf`, MS8 step 6, D271).
if (CORE === 'lily-engine-core') {
  const pdf = run('cargo', [`+${toolchain}`, 'rustc', '-q', '-p', 'lily-engine-pdf', '--lib', '--features', 'module', '--profile', 'wasm',
    '--target', TARGET, '--crate-type', 'cdylib', ...(variant === 'stable' ? [] : ['-Zbuild-std=std,core,alloc,panic_abort'])],
  { env: { ...process.env, RUSTFLAGS: flags.join(' ') }, stdio: 'inherit' })
  if (!pdf.ok) fail('cargo build of pdf.wasm failed')
  const optimised = run(wasmOpt, ['-Oz', ...features, '--strip-debug', '--strip-producers', `${root}/target/${TARGET}/wasm/lily_engine_pdf.wasm`,
    '-o', `${out}/pdf.wasm`], { stdio: 'inherit' })
  if (!optimised.ok) fail('wasm-opt of pdf.wasm failed')
}
// Natively, on stable, from the same sources as the core: an image restores only into the primitives that wrote it.
const writer = (...args) => {
  const written = run('cargo', ['+stable', 'run', '-q', '--release', '-p', 'lily-engine-core', '--example', 'bench', '--', ...args],
    { stdio: ['ignore', 'pipe', 'inherit'] })
  if (!written.ok) fail('the image writer failed')
}
if (scm) writer('--write-image', scm, `${out}/scheme.image`)
if (CORE === 'lily-engine-spike') copyFileSync(`${harness}/prelude.scm`, `${out}/core.image`)
else if (scm) writer('--write-core-image', `${out}/scheme.image`, `${out}/core.image`)
else fail('LilyPond 2.26.0 is not installed: the core\'s image needs its scm/lily (give it as LILY_SCM)')
if (!scm) console.warn('build: LilyPond 2.26.0 is not installed: no scheme.image (give its scm/lily as LILY_SCM)')
// S6's parts the host gives as modules (I2, MS4 step 20; the core's PART_MODULES, all seven since MS7 step 7),
// from the same scm/lily: a page then needs none of LilyPond's own files for them. None for the spike.
const parts = CORE === 'lily-engine-core' && scm ? ['chords', 'display', 'fretted', 'diagrams', 'partcombine', 'modal', 'qr'] : []
for (const part of parts) writer('--write-part', scm, part, `${out}/${part}.part`)
// LilyPond's ly/ files, beside its scm/lily, which a score may include (`\include "articulate.ly"`): each a
// module fetched when the engine reads it (D263), so that a page needs none of LilyPond's files. None for the spike.
const lyDir = scm ? `${scm}/../../ly` : undefined
const lyFiles = CORE === 'lily-engine-core' && lyDir && existsSync(lyDir) ? readdirSync(lyDir).filter((file) => file.endsWith('.ly')).sort() : []
for (const file of lyFiles) copyFileSync(`${lyDir}/${file}`, `${out}/ly-${file}`)
try {
  buildPacks({ otf, out: `${out}/fonts` })
  buildTextPacks({ otf, out: `${out}/fonts` })
} catch (error) {
  fail(`font packs: ${error.message}`)
}
// The packs the manifest preloads, beside the core, where the studio's --site build copies the manifest's files from:
// Emmentaler-20 and, but for the spike, the text packs of 06 §5.2 (C059 Roman, Bold and Italic; MS3 step 8).
let preload = 'emmentaler-20.pack'
if (CORE === 'lily-engine-spike') {
  const pack = run('node', [`${harness}/pack.mjs`, font], { maxBuffer: 1 << 26 })
  if (!pack.ok) fail(`pack.mjs failed: ${pack.stderr ?? pack.error}`)
  preload = 'emmentaler-20.spike.pack'
  writeFileSync(`${out}/${preload}`, pack.raw)
} else {
  copyFileSync(`${out}/fonts/emmentaler-20.pack`, `${out}/${preload}`)
}
const text = CORE === 'lily-engine-spike' ? [] : CORE_STYLES
// The packs fetched on demand (W5), beside the core as the preloaded are, since the --site build copies no
// directory: the sizes but 20, the brace's parts in order, the text faces but the preloaded; none for the spike.
const braces = readdirSync(`${out}/fonts`).filter((file) => /^emmentaler-brace-\d+\.pack$/.test(file)).sort()
  .map((file) => file.slice(0, -'.pack'.length))
const demand = CORE === 'lily-engine-spike' ? [] : [...SIZES.filter((size) => size !== 20).map((size) => `emmentaler-${size}`),
  ...braces, ...STYLES.filter((style) => !text.includes(style)), ...STYLES.map(outlinesModule)]
for (const name of [...text, ...demand]) copyFileSync(`${out}/fonts/${name}.pack`, `${out}/${name}.pack`)
// The text faces as web fonts (E17, D263), so that the page draws a page's <text font-family="LilyPond Serif"> in the
// face the engine measured, as LilyPond's own fontconfig maps its three families, and "New Century Schoolbook" as its
// metric alias does (E23, D271); the OTFs as LilyPond ships them, which are also their source (URW's licence with its
// exception; TeX Gyre's the GUST Font License), named by PostScript name. The studio's --site build writes their
// @font-face. The same files are the PDF module's fonts (`font` modules, D271).
const FAMILIES = { C059: 'LilyPond Serif', NimbusSans: 'LilyPond Sans Serif', NimbusMonoPS: 'LilyPond Monospace', TeXGyreSchola: 'New Century Schoolbook' }
const webFonts = CORE === 'lily-engine-spike' ? [] : STYLES.map((style) => {
  const [face, variant] = style.split('-')
  copyFileSync(`${otf}/${otfFile(style)}`, `${out}/${style}.otf`)
  return { family: FAMILIES[face], weight: /Bold|BdIta/.test(variant) ? 700 : 400, style: /Italic|BdIta/.test(variant) ? 'italic' : 'normal', file: `${style}.otf` }
})
// The licences of what is served (licenses/NOTICE.txt says which is whose), under their own names.
const licenses = { notice: 'NOTICE.txt', engine: 'LICENSE.txt', lilypond: 'LILYPOND-LICENSE.txt', ofl: 'LILYPOND-LICENSE.OFL.txt',
  urw: 'URW-LICENSE.txt', agpl: 'URW-COPYING.txt', gust: 'TEX-GYRE-LICENSE.txt', lppl: 'LPPL-1.3c.txt' }
for (const file of Object.values(licenses)) copyFileSync(file === 'LICENSE.txt' ? `${root}/LICENSE` : `${root}/licenses/${file}`, `${out}/${file}`)
// The commit the engine was built from, which its source is (D56); `dirty` when the tree had changes.
const head = run('git', ['rev-parse', 'HEAD'])
const status = run('git', ['status', '--porcelain', '--', '.'])
const source = { repository: 'https://github.com/velet5/lily', commit: head.ok ? head.stdout : 'unknown', dirty: !status.ok || status.stdout !== '' }
// The format of packages/studio/src/web/engine/protocol.ts's EngineManifest.
const manifest = {
  version: 1,
  core: { wasm: 'core.wasm', image: 'core.image', preload: ['emmentaler-20', ...text], ...(debugCore ? { debug: 'core-debug.wasm' } : {}) },
  source,
  licenses,
  webFonts,
  modules: {
    'emmentaler-20': { kind: 'pack', file: preload },
    ...Object.fromEntries([...text, ...demand].map((name) => [name, { kind: 'pack', file: `${name}.pack` }])),
    ...Object.fromEntries(modules.map((name) => [name, { kind: 'split', file: `${name}.wasm` }])),
    ...Object.fromEntries(parts.map((name) => [name, { kind: 'part', file: `${name}.part` }])),
    ...Object.fromEntries(lyFiles.map((file) => [`ly/${file}`, { kind: 'file', file: `ly-${file}` }])),
    ...(CORE === 'lily-engine-core' ? { pdf: { kind: 'wasm', file: 'pdf.wasm' } } : {}),
    ...Object.fromEntries(webFonts.map((font) => [font.file, { kind: 'font', file: font.file }])),
  },
}
writeFileSync(`${out}/manifest.json`, JSON.stringify(manifest) + '\n')
// Precompressed here, so that no server compresses on a first visit (09 §7: ~150 ms for core.wasm);
// with sizes.mjs's parameters, so each .br is the size the report gives. Not fonts/, which no server sends yet.
for (const file of readdirSync(out, { withFileTypes: true }).filter((entry) => entry.isFile()).map((entry) => entry.name)) {
  writeFileSync(`${out}/${file}.br`, brotliCompressSync(readFileSync(`${out}/${file}`), { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }))
}

console.log(`${run('rustc', [`+${toolchain}`, '-V']).stdout}, ${binaryen.stdout}, variant ${variant}\n`)
// The report never fails the build, not even when it cannot read the core.
try {
  report(root, CORE.replaceAll('-', '_'))
} catch (error) {
  console.warn(`build: no size report: ${error.stack ?? error}`)
}
