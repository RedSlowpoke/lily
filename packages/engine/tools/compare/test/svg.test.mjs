// The SVG writer (MS3 step 7, W3) on the 11 pages of the 10 scores of
// MS3's gate: step 6's references ($LILY_CORPUS/ref/<id>/pages.scm, written
// by npm run capture) written in the file form by crates/svg's write
// example, given the build's packs, and compared with LilyPond's own pages
// (06 §3.3, part 1's How part 1's gate reads):
//
// - Links: every link's file, line, char and column and its translate are
//   LilyPond's page's, in order, and score.mjs' read takes the same links
//   from both; the page's size and viewBox are LilyPond's; no warnings.
// - Pixels: both pages rendered with rsvg-convert -z 2 -b white, then magick
//   compare: PAE one grey level or less, AE recorded.
// - Sizes: each score's bytes against 06 §3.3's compact KB, and B10 per
//   page, the writer's bytes over LilyPond's (Must ≤ 100 %, Goal ≤ 50 %),
//   recorded, not judged.
//
// Skipped, and so unverified, without LilyPond 2.26.0, without the
// references or the build's packs (npm run capture, npm run build), or
// without cargo; the pixels also without Homebrew's rsvg-convert and magick.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { lilyScm } from '../../bench/run.mjs'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { runExample } from '../../examples.mjs'
import { PAGES } from '../capture.mjs'
import { read } from '../score.mjs'

const engine = path.resolve(path.dirname(new URL(import.meta.url).pathname), '../../..')
/** The 10 scores of MS3's gate (README §5), with 06 §3.3's compact KB. */
const COMPACT_KB = { T01: 5, T03: 8, T05: 35, T10: 13, T13: 13, T14: 5, T16: 4, T17: 21, T18: 12, T19: 7 }
const TEN = Object.keys(COMPACT_KB)
const ten = corpus().filter((row) => TEN.includes(row.id))
const refs = path.join(corpusDir(), 'ref')
const packs = path.join(engine, 'dist/engine/fonts')
const skip = !lilyScm()
  ? 'LilyPond 2.26.0 is not installed'
  : ten.length !== TEN.length ? 'the corpus is not fetched: run node tools/corpus/fetch.mjs'
  : !TEN.every((id) => existsSync(path.join(refs, id, PAGES))) ? `no ${PAGES} in ${refs}: run npm run capture`
  : !existsSync(path.join(packs, 'emmentaler-20.pack')) ? 'no dist/engine/fonts: run npm run build'
  : spawnSync('cargo', ['+stable', '--version']).status !== 0 ? 'cargo +stable is not installed' : false
const tools = ['rsvg-convert', 'magick'].filter((tool) => spawnSync(tool, ['--version']).status !== 0)
const skipPixels = skip || (tools.length ? `${tools.join(' and ')} not installed (Homebrew): the pixels are unverified` : false)

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-svg-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

const pagesOf = (dir) => readdirSync(dir).filter((f) => f.endsWith('.svg'))
  .sort((a, b) => (+(a.match(/-(\d+)\.svg$/)?.[1] ?? 0)) - (+(b.match(/-(\d+)\.svg$/)?.[1] ?? 0)))

let written
/** The 10's pages written once into the scratch directory: by ID, the directory and the example's lines. */
function write() {
  written ??= new Map(ten.map((row) => {
    const dir = path.join(scratch, row.id)
    const ref = path.join(refs, row.id)
    // The score's directory and the capture's (T16's included string) are the tree's roots.
    const run = runExample('write', ['--packs', packs, '--out', dir, '--root', path.dirname(row.path), '--root', ref, path.join(ref, PAGES)],
      { cwd: engine, encoding: 'utf8' })
    assert.equal(run.status, 0, `${row.id}: ${run.stderr}`)
    return [row.id, { dir, ref, lines: run.stdout.trim().split('\n').map((line) => JSON.parse(line)) }]
  }))
  return written
}

/** Each link of a page in order: its file's name, line, char and column, and its first child's translate. */
const links = (svg) => [...svg.matchAll(/xlink:href="textedit:\/\/[^"]*?([^/"]*:\d+:\d+:\d+)">\s*<g transform="translate\(([-\d.]+), ([-\d.]+)\)">/g)]
  .map((m) => `${m[1]} ${+m[2]} ${+m[3]}`)
/** The `<svg>` element's width, height and viewBox, as numbers. */
const size = (svg) => svg.match(/<svg [^>]*>/)[0].match(/(?:width|height|viewBox)="[^"]*"/g)
  .map((a) => a.replace(/-?[\d.]+/g, (n) => String(+n)))

test('the 11 pages written with LilyPond\'s links, in order, and its page size', { skip, timeout: 300_000 }, () => {
  let pages = 0
  let linked = 0
  for (const [id, { dir, ref, lines }] of write()) {
    assert.deepEqual(lines.map((l) => l.file), pagesOf(ref), `${id}: a page for each of LilyPond's`)
    for (const { file, warnings } of lines) {
      assert.deepEqual(warnings, [], `${id} ${file}`)
      const [ours, theirs] = [dir, ref].map((d) => readFileSync(path.join(d, file), 'utf8'))
      assert.deepEqual(links(ours), links(theirs), `${id} ${file}: the links`)
      assert.deepEqual(size(ours), size(theirs), `${id} ${file}: the page's size`)
      const ids = [...ours.matchAll(/<path id="([^"]+)"/g)].map((m) => m[1])
      assert.equal(new Set(ids).size, ids.length, `${id} ${file}: each glyph defined once`)
      pages++
      linked += links(ours).length
    }
    assert.deepEqual(read(dir).links, read(ref).links, `${id}: score.mjs' links`)
  }
  // One page each, T17's two; step 6 counted 245 links.
  assert.deepEqual([pages, linked], [11, 245])
})

test('the 11 pages\' pixels: PAE one grey level or less against LilyPond\'s', { skip: skipPixels, timeout: 300_000 }, (t) => {
  const over = []
  for (const [id, { dir, ref, lines }] of write()) {
    for (const { file } of lines) {
      const png = (from, to) => {
        const run = spawnSync('rsvg-convert', ['-z', '2', '-b', 'white', from, '-o', to], { encoding: 'utf8' })
        assert.equal(run.status, 0, `${id} ${file}: rsvg-convert: ${run.stderr}`)
        return to
      }
      const [a, b] = [png(path.join(ref, file), path.join(dir, `${file}.ref.png`)), png(path.join(dir, file), path.join(dir, `${file}.png`))]
      // `<absolute> (<normalized>)` on stderr; exit 1 when they differ.
      const metric = (name) => {
        const run = spawnSync('magick', ['compare', '-metric', name, a, b, 'null:'], { encoding: 'utf8' })
        assert.ok(run.status === 0 || run.status === 1, `${id} ${file}: magick compare: ${run.stderr}`)
        const [, absolute, normalized] = run.stderr.match(/^([\d.e+-]+) \(([\d.e+-]+)\)/)
        return { absolute: +absolute, normalized: +normalized }
      }
      const pae = metric('PAE')
      const ae = metric('AE')
      const grey = pae.normalized * 255
      t.diagnostic(`${id} ${file}: PAE ${pae.absolute} (${grey.toFixed(3)} grey levels), AE ${ae.absolute} pixels`)
      if (grey > 1 + 1e-6) over.push(`${id} ${file}: PAE ${grey.toFixed(3)} grey levels, AE ${ae.absolute}`)
    }
  }
  // Over one grey level is step 7's stop point when every difference is on a glyph.
  assert.deepEqual(over, [])
})

test('the 11 pages\' sizes: each score against 06 §3.3\'s compact KB, and B10 per page', { skip, timeout: 300_000 }, (t) => {
  let ours = 0
  let theirs = 0
  for (const [id, { dir, ref, lines }] of write()) {
    let bytes = 0
    for (const { file, bytes: n } of lines) {
      assert.equal(n, statSync(path.join(dir, file)).size, `${id} ${file}: the bytes the example wrote`)
      const lilypond = statSync(path.join(ref, file)).size
      t.diagnostic(`${id} ${file}: ${n} bytes, LilyPond's ${lilypond}: B10 ${(100 * n / lilypond).toFixed(1)} % (Must ≤ 100, Goal ≤ 50)`)
      bytes += n
      ours += n
      theirs += lilypond
    }
    t.diagnostic(`${id}: ${(bytes / 1024).toFixed(1)} KB against 06 §3.3's compact ${COMPACT_KB[id]} KB`)
  }
  t.diagnostic(`the 11 pages: ${ours} bytes, LilyPond's ${theirs}: ${(100 * ours / theirs).toFixed(1)} %`)
  assert.ok(ours > 0)
})
