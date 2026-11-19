// W7's PDF (MS8 step 5, D270) on 06 §7.1's four scores, T02, T20, T24 and
// T29, and T26, whose "New Century Schoolbook" is TeX Gyre Schola (E23,
// MS8 step 6, D271): each compiled natively by the core's compile example
// with --pdf, which writes the book's display list (<book>.pdf.list), its
// SVG pages and its PDF written by crates/pdf natively, the URW fonts and
// TeX Gyre Schola from LilyPond 2.26.0's fonts/otf; then, as 06 §7.3 checks it:
//
// - pdf.wasm, the build's module, writes the same PDF from the same list
//   and fonts, byte for byte, natively and in wasm alike;
// - pages: the PDF's, by pdf.js, are LilyPond's own PDF's and the engine's
//   SVG pages';
// - pixels: each page rasterized by pdf.js (the studio's pdfjs-dist, with
//   @napi-rs/canvas) against the engine's own SVG page by rsvg-convert, at
//   the same size: of the dark pixels of either (under half grey), those
//   with no ink within a pixel in the other (an eighth grey or more, so that
//   a thin line one antialiases lighter than the other still matches), at
//   most 0.1 %, where the same page moved three pixels has over 1 %. Both without their text,
//   the PDF written by pdf.wasm given no font and the SVG's <text> taken
//   out, since rsvg draws a page's text in whatever face fontconfig finds
//   for its family, which the text check covers instead;
// - text: every string of the SVG's <tspan>s, the lyrics and titles, is in
//   the text pdf.js extracts from the PDF, which its ToUnicode maps give;
//   its Info's Title and Composer are LilyPond's PDF's;
// - size: the PDF within 150 % of LilyPond's (README §5's MS8 gate; the
//   split's pdf.txt), recorded with the ratio;
// - no warning from the writer.
// And core.wasm, given lily_pdf (D271), gives each score's list as the
// native compile does, byte for byte, for a preview's compile and the file
// form's.
//
// LilyPond's PDFs are $LILY_CORPUS/ms8-split/pdf/<id>/ (the split's
// pdf.mjs made them), or made here with lilypond --pdf. Skipped, and so
// unverified, without LilyPond 2.26.0, the corpus, cargo or the build's
// pdf.wasm (npm run build); the pixels also without rsvg-convert, and all
// but the bytes without pdfjs-dist and @napi-rs/canvas (npm install).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { lilypondVersion } from '../capture.mjs'
import { compileInWasm, coreSkip, outlinePacks, preloadedModules } from '../core.mjs'

const engine = path.resolve(path.dirname(new URL(import.meta.url).pathname), '../../..')
const SCORES = ['T02', 'T20', 'T24', 'T26', 'T29']
const rows = corpus().filter((row) => SCORES.includes(row.id))
const wasmFile = path.join(engine, 'dist/engine/pdf.wasm')
// pdfjs-dist is the studio's dependency, which npm hoists to the workspace's node_modules.
const require = createRequire(import.meta.url)
const resolves = (name) => {
  try {
    return require.resolve(name)
  } catch {
    return undefined
  }
}
/** LilyPond's fonts/otf, beside the scm/lily the tools find. */
const otfDir = () => {
  const which = spawnSync('sh', ['-c', 'command -v lilypond'], { encoding: 'utf8' })
  const dir = which.status === 0 ? path.resolve(path.dirname(realpath(which.stdout.trim())), '../share/lilypond/2.26.0/fonts/otf') : ''
  return existsSync(path.join(dir, 'C059-Roman.otf')) ? dir : undefined
}
const realpath = (file) => spawnSync('realpath', [file], { encoding: 'utf8' }).stdout.trim() || file
const otf = lilypondVersion() === '2.26.0' ? otfDir() : undefined

const skip = !otf ? 'LilyPond 2.26.0 is not installed'
  : rows.length !== SCORES.length || !rows.every((row) => existsSync(row.path)) ? 'the corpus is not fetched: run node tools/corpus/fetch.mjs'
  : !existsSync(wasmFile) ? 'no dist/engine/pdf.wasm: run npm run build'
  : spawnSync('cargo', ['+stable', '--version']).status !== 0 ? 'cargo +stable is not installed' : false
const pdfjsPath = resolves('pdfjs-dist/legacy/build/pdf.mjs')
const canvasPath = resolves('@napi-rs/canvas')
const skipJs = skip || (pdfjsPath && canvasPath ? false : 'pdfjs-dist or @napi-rs/canvas is not installed: run npm install')
const skipPixels = skipJs || (spawnSync('rsvg-convert', ['--version']).status === 0 ? false : 'rsvg-convert is not installed (Homebrew): the pixels are unverified')

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-pdf-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** The 12 URW OTFs and TeX Gyre Schola's 4, by PostScript name, as a host would give them all. */
const SCHOLA = { regular: 'Regular', bold: 'Bold', italic: 'Italic', bolditalic: 'BoldItalic' }
const textFonts = () => readdirSync(otf).filter((f) => /^(C059|NimbusSans|NimbusMonoPS)-.*\.otf$|^texgyreschola-/.test(f))
  .map((f) => [f.replace(/\.otf$/, '').replace(/^texgyreschola-(\w+)$/, (_, style) => `TeXGyreSchola-${SCHOLA[style]}`), readFileSync(path.join(otf, f))])

let module
/** pdf.wasm's PDF of `list` with `fonts`, and its warnings: a fresh instance each time, as the host drops it after an export. */
async function wasmPdf(list, fonts) {
  module ??= await WebAssembly.compile(readFileSync(wasmFile))
  const { exports } = await WebAssembly.instantiate(module, {})
  const u32 = (n) => { const b = Buffer.alloc(4); b.writeUInt32LE(n); return b }
  const piece = (bytes) => [u32(bytes.length), bytes]
  const input = Buffer.concat([...piece(list), u32(fonts.length), ...fonts.flatMap(([name, bytes]) => [...piece(Buffer.from(name)), ...piece(bytes)])])
  const at = exports.lily_pdf_input(input.length)
  new Uint8Array(exports.memory.buffer, at, input.length).set(input)
  assert.equal(exports.lily_pdf_write(), 0, 'pdf.wasm takes the list')
  const out = Buffer.from(new Uint8Array(exports.memory.buffer, exports.lily_pdf_output(), exports.lily_pdf_output_len()))
  const warnings = Buffer.from(new Uint8Array(exports.memory.buffer, exports.lily_pdf_warnings(), exports.lily_pdf_warnings_len())).toString('utf8')
  return { pdf: out, warnings: warnings ? warnings.split('\n') : [] }
}

/** LilyPond's own PDF of `row`: the split's, or made here. */
function lilypondPdf(row) {
  const dir = path.join(corpusDir(), 'ms8-split/pdf', row.id)
  const found = existsSync(dir) && readdirSync(dir).find((f) => f.endsWith('.pdf'))
  if (found) return path.join(dir, found)
  const out = path.join(scratch, `lilypond-${row.id}`)
  mkdirSync(out)
  const run = spawnSync('lilypond', ['--pdf', '-o', path.join(out, row.id), row.path], { cwd: path.dirname(row.path), encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  return path.join(out, readdirSync(out).find((f) => f.endsWith('.pdf')))
}

/** The engine's outputs of `row`: its directory, pages, list, PDF and the writer's log. */
function compile(row) {
  const out = path.join(scratch, row.id)
  mkdirSync(out)
  const run = runExample('compile', ['--pdf', otf, '--out', out, path.basename(row.path)], { cwd: path.dirname(row.path), encoding: 'utf8', release: true })
  assert.equal(run.status, 0, `${row.id}: ${run.stderr}`)
  const book = readdirSync(out).find((f) => f.endsWith('.pdf.list')).replace(/\.list$/, '')
  return {
    out,
    svgs: compileSvgs(out),
    list: readFileSync(path.join(out, `${book}.list`)),
    pdf: readFileSync(path.join(out, book)),
    log: readFileSync(path.join(out, `${book}.log`), 'utf8'),
  }
}

let pdfjs
let canvas
async function open(bytes) {
  pdfjs ??= await import(pdfjsPath)
  canvas ??= require(canvasPath)
  return pdfjs.getDocument({ data: new Uint8Array(bytes), verbosity: 0, isEvalSupported: false }).promise
}

/** A page of `doc` rasterized `width` pixels wide: each pixel's grey, 0 white to 765 black. */
async function rasterPdf(doc, number, width) {
  const page = await doc.getPage(number)
  const viewport = page.getViewport({ scale: width / page.getViewport({ scale: 1 }).width })
  const c = canvas.createCanvas(width, Math.round(viewport.height))
  const ctx = c.getContext('2d')
  ctx.fillStyle = 'white'
  ctx.fillRect(0, 0, c.width, c.height)
  await page.render({ canvasContext: ctx, canvas: c, viewport }).promise
  return dark(ctx, c.width, c.height)
}

/** An SVG page without its text rasterized by rsvg-convert `width` pixels wide. */
async function rasterSvg(file, width) {
  const svg = readFileSync(file, 'utf8').replace(/<text[^>]*>[\s\S]*?<\/text>\n?/g, '')
  const bare = file.replace(/\.svg$/, '.bare.svg')
  writeFileSync(bare, svg)
  const png = bare.replace(/\.svg$/, '.png')
  const run = spawnSync('rsvg-convert', ['-w', String(width), '-b', 'white', bare, '-o', png], { encoding: 'utf8' })
  assert.equal(run.status, 0, run.stderr)
  const image = await canvas.loadImage(readFileSync(png))
  const c = canvas.createCanvas(image.width, image.height)
  const ctx = c.getContext('2d')
  ctx.drawImage(image, 0, 0)
  return dark(ctx, c.width, c.height)
}

function dark(ctx, w, h) {
  const data = ctx.getImageData(0, 0, w, h).data
  const m = new Uint16Array(w * h)
  for (let i = 0; i < m.length; i++) m[i] = 765 - (data[4 * i] + data[4 * i + 1] + data[4 * i + 2])
  return { m, w, h }
}

/** Of `a`'s dark pixels, how many and those with no ink of `b` within a pixel, `b` moved `shift` pixels right. */
function unmatched(a, b, shift = 0) {
  let all = 0
  let lone = 0
  for (let y = 0; y < a.h; y++) {
    for (let x = 0; x < a.w; x++) {
      if (a.m[y * a.w + x] <= 765 / 2) continue
      all++
      let hit = false
      for (let dy = -1; dy <= 1 && !hit; dy++) {
        for (let dx = -1; dx <= 1 && !hit; dx++) {
          const [X, Y] = [x + dx - shift, y + dy]
          hit = X >= 0 && Y >= 0 && X < b.w && Y < b.h && b.m[Y * b.w + X] >= 765 / 8
        }
      }
      if (!hit) lone++
    }
  }
  return { all, lone }
}

const decode = (s) => s.replaceAll('&lt;', '<').replaceAll('&gt;', '>').replaceAll('&quot;', '"').replaceAll('&amp;', '&')
const squeeze = (s) => s.replace(/\s+/g, '')

for (const id of SCORES) {
  test(`${id}: the engine's PDF, its pages, pixels, text and size`, { skip: skip || skipJs }, async (t) => {
    const row = rows.find((r) => r.id === id)
    const engineOut = compile(row)
    assert.equal(engineOut.log, '', `${id}: the writer warns`)
    const fonts = textFonts()
    assert.equal(fonts.length, 16)
    const wasm = await wasmPdf(engineOut.list, fonts)
    assert.deepEqual(wasm.warnings, [])
    assert.ok(wasm.pdf.equals(engineOut.pdf), `${id}: pdf.wasm writes the native PDF`)
    // T26's time signatures in LilyPond's face, TeX Gyre Schola, embedded (E23, D271).
    if (id === 'T26') assert.ok(engineOut.pdf.includes('TeXGyreSchola-Regular'), 'T26: TeX Gyre Schola is embedded')

    const reference = lilypondPdf(row)
    const [ours, theirs] = await Promise.all([open(engineOut.pdf), open(readFileSync(reference))])
    assert.equal(ours.numPages, theirs.numPages, `${id}: LilyPond's pages`)
    assert.equal(ours.numPages, engineOut.svgs.length, `${id}: the SVG's pages`)

    // Text: every <tspan> of the SVG pages in the PDF's text, page by page.
    for (let n = 1; n <= ours.numPages; n++) {
      const page = await ours.getPage(n)
      const text = squeeze((await page.getTextContent()).items.map((item) => item.str).join(''))
      const svg = readFileSync(engineOut.svgs[n - 1], 'utf8')
      const strings = [...svg.matchAll(/<tspan>([^<]*)<\/tspan>/g)].map((m) => squeeze(decode(m[1]))).filter(Boolean)
      assert.ok(strings.length > 0, `${id} page ${n} has text`)
      for (const s of strings) assert.ok(text.includes(s), `${id} page ${n}: "${s}" is in the PDF's text`)
    }
    const [info, theirInfo] = await Promise.all([ours.getMetadata(), theirs.getMetadata()])
    for (const key of ['Title', 'Composer']) assert.equal(info.info[key], theirInfo.info[key], `${id}: the Info's ${key}`)

    const bytes = engineOut.pdf.length
    const lily = statSync(reference).size
    t.diagnostic(`${id}: ${bytes} bytes, LilyPond's ${lily}: ${(100 * bytes / lily).toFixed(1)} % (Must 150 %, Goal 110 %); ${ours.numPages} page(s)`)
    assert.ok(bytes <= 1.5 * lily, `${id}: within 150 % of LilyPond's PDF`)
  })

  test(`${id}: each PDF page rasterized by pdf.js against the engine's SVG page`, { skip: skipPixels }, async (t) => {
    const out = path.join(scratch, id)
    const row = rows.find((r) => r.id === id)
    const engineOut = existsSync(out) ? {
      list: readFileSync(path.join(out, readdirSync(out).find((f) => f.endsWith('.pdf.list')))),
      svgs: compileSvgs(out),
    } : compile(row)
    const bare = await wasmPdf(engineOut.list, [])
    const doc = await open(bare.pdf)
    for (let n = 1; n <= doc.numPages; n++) {
      const width = 1240
      // The same page size, in points; the rasters' heights may differ by the one pixel each rounds to (T26's
      // 210 × 70 mm page is 413.3 pixels high: pdf.js rounds it, rsvg-convert rounds it up).
      const [view, page] = [(await doc.getPage(n)).view, readFileSync(engineOut.svgs[n - 1], 'utf8').match(/width="([\d.]+)mm" height="([\d.]+)mm"/)]
      const pt = (mm) => (Number(mm) * 72) / 25.4
      assert.ok(Math.abs(view[2] - pt(page[1])) < 0.01 && Math.abs(view[3] - pt(page[2])) < 0.01, `${id} page ${n}: the SVG page's size`)
      const [pdf, svg] = [await rasterPdf(doc, n, width), await rasterSvg(engineOut.svgs[n - 1], width)]
      assert.ok(Math.abs(pdf.h - svg.h) <= 1, `${id} page ${n}: the same height (${pdf.h}, ${svg.h})`)
      const [a, b] = [unmatched(pdf, svg), unmatched(svg, pdf)]
      t.diagnostic(`${id} page ${n}: ${a.lone} of the PDF's ${a.all} dark pixels unmatched, ${b.lone} of the SVG's ${b.all}`)
      assert.ok(a.lone <= 0.001 * a.all && b.lone <= 0.001 * b.all, `${id} page ${n}: the PDF draws the SVG page`)
      // The measure tells: the same page three pixels to the right does not pass.
      const moved = unmatched(pdf, svg, 3)
      assert.ok(moved.lone > 0.01 * moved.all, `${id} page ${n}: a page moved by 3 pixels is told apart (${moved.lone} of ${moved.all})`)
    }
  })
}

// core.wasm's display lists (lily_pdf, MS8 step 6, D271): the native compile's byte for byte, for a page's compile
// (a preview, as Export PDF's on the web) as for the file form's.
const wasmSkip = skip || coreSkip()
test('core.wasm gives each score\'s display list as the native compile does, with a preview or without', { skip: wasmSkip }, async () => {
  const modules = [...preloadedModules(), ...outlinePacks().map(([name, file]) => [name, readFileSync(file)])]
  for (const id of SCORES) {
    const row = rows.find((r) => r.id === id)
    const out = path.join(scratch, id)
    if (!existsSync(out)) compile(row)
    const native = readdirSync(out).find((f) => f.endsWith('.pdf.list'))
    const want = readFileSync(path.join(out, native))
    for (const preview of [false, true]) {
      const [wasm] = await compileInWasm([row.path], { modules, preview, pdf: true })
      const lists = Object.keys(wasm.outputs).filter((name) => name.endsWith('.pdf.list'))
      assert.deepEqual(lists, [native], `${id}: one list, named as the native one`)
      assert.ok(wasm.outputs[native].equals(want), `${id}${preview ? ' for a preview' : ''}: core.wasm's list is the native one`)
    }
  }
})

/** The SVG pages already written into `out`, in order. */
function compileSvgs(out) {
  const files = readdirSync(out)
  const stem = files.find((f) => f.endsWith('.pdf.list')).replace(/\.pdf\.list$/, '')
  return files.filter((f) => f.endsWith('.svg') && !f.endsWith('.bare.svg') && (f === `${stem}.svg` || f.startsWith(`${stem}-`)))
    .sort((a, b) => (+(a.match(/-(\d+)\.svg$/)?.[1] ?? 0)) - (+(b.match(/-(\d+)\.svg$/)?.[1] ?? 0)))
    .map((f) => path.join(out, f))
}
