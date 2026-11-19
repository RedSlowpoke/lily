// The references of W3 and text (06 §3.3, MS3 step 6): stencils.ly's
// capture of the pages' stencils and the markups' of the 10 scores of MS3's
// gate, read back by the engine, and stencils.mjs' compare.
//
// - The compare (no LilyPond): a reference against itself has no
//   difference; a perturbed expression, extent or link has one of its kind;
//   four decimals hide what the fifth changes.
// - The capture: the 10 scores compiled by LilyPond 2.26.0 with the
//   capture's flags and capture.ly (its text in LilyPond's own fonts, D106)
//   give 11 pages and their markups, every record read by stencils.mjs and
//   written back as it was.
// - Run to run (MS3 step 12a): two captures of a score whose \header has 15
//   entries and two of T16 are the same byte for byte, the header's entries
//   written sorted by name and T16's links naming ref/T16 wherever the
//   capture goes.
// - Read back by the engine from the image the build shipped
//   (dist/engine/scheme.image) natively and in the built core.wasm: each
//   record's stencil read into layout's stencils and fonts
//   (%dump->stencil, given the build's packs as a host gives them) has the
//   capture's extents, and written back (%stencil->dump) is the capture's
//   record. crates/layout/tests/pages.rs reads $LILY_CORPUS/ref's natively
//   in a VM without the library.
//
// - Interpreted by the engine (MS3 step 8): every markup of the 10, its
//   markup and its chain read back (a procedure by its name in (lily) or
//   among the paper's title, header and footer markups), interpreted with
//   interpret-markup in the default \paper scaled as paper-book.cc scales a
//   book's, given the build's packs, with music-strings-to-paths as the
//   engine's svg backend sets it (MS3 step 19, lily.scm:562–566), from
//   source natively, from the image natively and in wasm: its
//   stencil LilyPond's to four decimals, a glyph string's font file by its
//   name only, but for the 8 recorded with their strings (Emmentaler's text,
//   which Pango kerns and alternates with what its packs do not hold).
// - Printed from the engine's own grobs (MS3 step 19): the 10's TextScripts
//   and DynamicTexts, made by the engine's engravers in a compile of each
//   score with capture.ly's text families (D106), each read through its
//   stencil, ly:text-interface::print, natively and in wasm: for the grob of
//   the same cause, the markup LilyPond printed, its chain's definition and
//   default, the properties it has LilyPond's, and the stencil to every
//   digit, but for step 8's Emmentaler strings among them.
// - The core given the manifest's C059 packs as modules measures text as
//   LilyPond does, natively and in wasm.
// - The shaping check (MS4 step 19, W6): every string of every reference in
//   $LILY_CORPUS/ref (the 42 and part 3's and 4's fixtures) made again from
//   its description and text over the build's packs, at its score's staff
//   size, from the image natively and in wasm, the same both ways and
//   LilyPond's, but T20's lyrics (D224, D227) and the strings of Y51, Y54 and
//   Y55 (typography-demo.ly, which asks for T20's families), recorded; T26's "New Century
//   Schoolbook", LilyPond's but for the face until TeX Gyre Schola's pack,
//   is LilyPond's (E23, D271); crates/layout/tests/markups.rs makes them
//   from source.
//
// Skipped, and so unverified, without LilyPond 2.26.0, without the corpus
// (fetch.mjs), without a core.wasm and a scheme.image as new as the sources
// (npm run build), or without cargo.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { runExample } from '../../examples.mjs'
import { corpus, corpusDir } from '../../corpus/fetch.mjs'
import { benchDir, coreWasm, lilyScm, runOnce, schemeImage } from '../../bench/run.mjs'
import { captureOne, linkDir, MARKUPS, PAGES } from '../capture.mjs'
import { compileInWasm } from '../core.mjs'
import { coreSkip } from '../core.mjs'
import { compareRecords, readCapture, readRecords, readSexp, writeSexp } from '../stencils.mjs'

const engine = path.resolve(benchDir, '../..')
const sources = ['crates/scheme/src', 'crates/music/src', 'crates/layout/src', 'crates/fonts/src', 'crates/parser/src', 'crates/parser/tests/support', 'crates/core/src']
  .map((d) => path.join(engine, d))
const newest = Math.max(...sources.flatMap((dir) => readdirSync(dir).map((f) => statSync(path.join(dir, f)).mtimeMs)))
/** The 10 scores of MS3's gate (README §5). */
const TEN = ['T01', 'T03', 'T05', 'T10', 'T13', 'T14', 'T16', 'T17', 'T18', 'T19']
const ten = corpus().filter((row) => TEN.includes(row.id))
const skipCapture = !lilyScm()
  ? 'LilyPond 2.26.0 is not installed'
  : ten.length === TEN.length && ten.every((row) => existsSync(row.path)) ? false : 'the corpus is not fetched: run node tools/corpus/fetch.mjs'
const skipEngine = skipCapture || [coreWasm, schemeImage].map((file) => !existsSync(file)
  ? `no dist/engine/${path.basename(file)}: run npm run build`
  : statSync(file).mtimeMs < newest ? `dist/engine/${path.basename(file)} is older than the sources: run npm run build` : false)
  .find(Boolean) || coreSkip() || (spawnSync('cargo', ['+stable', '--version']).status === 0 ? false : 'cargo +stable is not installed')

const scratch = mkdtempSync(path.join(tmpdir(), 'lily-stencils-ref-'))
after(() => rmSync(scratch, { recursive: true, force: true }))

/** A capture by hand: a page with a link, a glyph and text, and a markup. */
const PAGE = '(page "a.svg" 1 (0.0 . 119.5) (-169.0 . 0.0) (translate-stencil (8.5 . 0.0) (combine-stencil'
  + ' (grob-cause "textedit:///a.ly:3:4:5" (named-glyph (%font "emmentaler-20" "Emmentaler-20" 0.5690551181102361) "noteheads.s2"))'
  + ' (grob-cause #f (draw-line 0.1 0.0 0 1.0 0)) (utf-8-string "C059, 3.865234375" "x\\"y" ()))))'
const MARKUP = '(markup 1 (print TextScript "textedit:///a.ly:3:4:6") ((%procedure line-markup) ("a" "b"))'
  + ' (((font-size . -2) (span-factor . 1/2)) ((font-encoding . latin1))) (-0.0 . 2.5) (+inf.0 . -inf.0) ())'

test('stencils.mjs: a reference against itself has no difference, a perturbed expression, extent or link one of its kind', () => {
  const reference = readRecords(`${PAGE}\n${MARKUP}\n`)
  assert.equal(writeSexp(readSexp(PAGE)), PAGE)
  assert.equal(writeSexp(readSexp(MARKUP)), MARKUP)
  assert.deepEqual(compareRecords(reference, readRecords(`${PAGE}\n${MARKUP}\n`)), [])
  const kinds = (page, markup = MARKUP, options) =>
    compareRecords(reference, readRecords(`${page}\n${markup}\n`), options).map((d) => `${d.record}: ${d.kind} ${d.path} ${d.reference} ${d.got}`)
  assert.deepEqual(kinds(PAGE.replace('(draw-line 0.1', '(draw-line 0.2')), ['page "a.svg": expression /2/2/2/1 0.1 0.2'])
  assert.deepEqual(kinds(PAGE.replace('"noteheads.s2"', '"noteheads.s1"')), ['page "a.svg": expression /2/1/2/2 "noteheads.s2" "noteheads.s1"'])
  assert.deepEqual(kinds(PAGE.replace('0.5690551181102361', '0.5')), ['page "a.svg": expression /2/1/2/1/3 0.5690551181102361 0.5'])
  assert.deepEqual(kinds(PAGE.replace('(0.0 . 119.5)', '(0.0 . 119.6)')), ['page "a.svg": extent X/. 119.5 119.6'])
  assert.deepEqual(kinds(PAGE, MARKUP.replace('(+inf.0 . -inf.0)', '(0.0 . 0.0)')), ['markup 1 (print): extent Y/0 +inf.0 0.0'])
  assert.deepEqual(kinds(PAGE.replace(':3:4:5"', ':3:4:6"')), ['page "a.svg": link link 1 "textedit:///a.ly:3:4:5" "textedit:///a.ly:3:4:6"'])
  assert.deepEqual(kinds(PAGE.replace('(grob-cause #f (draw-line 0.1 0.0 0 1.0 0))', '(draw-line 0.1 0.0 0 1.0 0)')),
    ['page "a.svg": link  2 links 1 links', 'page "a.svg": expression /2/2 (grob-cause <link> (draw-line 0.1 0.0 0 1.0 0)) (draw-line 0.1 0.0 0 1.0 0)'])
  assert.deepEqual(kinds(PAGE, MARKUP.replace('(font-size . -2)', '(font-size . -1)')), ['markup 1 (print): input /3/0/0/. -2 -1'])
  assert.deepEqual(compareRecords(reference, readRecords(`${PAGE}\n`)).map((d) => d.kind), ['count'])
  // Four decimals hide the fifth's change, not the fourth's.
  assert.deepEqual(kinds(PAGE.replace('(draw-line 0.1 ', '(draw-line 0.10004 '), MARKUP, { digits: 4 }), [])
  assert.deepEqual(kinds(PAGE.replace('(draw-line 0.1 ', '(draw-line 0.1001 '), MARKUP, { digits: 4 }), ['page "a.svg": expression /2/2/2/1 0.1 0.1001'])
  assert.deepEqual(kinds(PAGE, MARKUP.replace('(-0.0 . 2.5)', '(0.0 . 2.50001)'), { digits: 4 }), [])
})

let captured
/** The 10 scores captured afresh into the scratch directory, once: their records, by ID. */
async function capture() {
  captured ??= Promise.all(ten.map(async (row) => {
    const dir = path.join(scratch, row.id)
    const result = await captureOne('lilypond', row, dir)
    assert.ok(existsSync(path.join(dir, PAGES)) && existsSync(path.join(dir, MARKUPS)), `${row.id}: no capture (exit ${result.exit})`)
    return [row.id, { dir, result, ...readCapture(dir) }]
  })).then((all) => new Map(all))
  return captured
}

test('the 10 scores give 11 pages and their markups, each record read and written back as it was', { skip: skipCapture, timeout: 300_000 }, async () => {
  const all = await capture()
  let pages = 0
  let markups = 0
  for (const [id, { dir, result, pages: p, markups: m }] of all) {
    // T03 exits 1, as it must, after its page is written.
    assert.equal(result.exit, id === 'T03' ? 1 : 0, id)
    assert.equal(p.length, result.pages, `${id}: a stencil for each page LilyPond wrote`)
    assert.ok(m.length > 0, `${id}: its markups`)
    for (const file of [PAGES, MARKUPS]) {
      for (const line of readFileSync(path.join(dir, file), 'utf8').split('\n').filter(Boolean)) assert.equal(writeSexp(readSexp(line)), line)
    }
    // Every link a page writes is a textedit link into the score's files, or none.
    for (const page of p) {
      const links = JSON.stringify(page.expr).match(/textedit:\/\/[^"\\]+/g) ?? []
      assert.ok(links.length > 0, `${id}: its links`)
    }
    pages += p.length
    markups += m.length
  }
  // One page each, T17's two.
  assert.equal(pages, 11)
  assert.ok(markups >= 10, `${markups} markups`)
})

/** A score whose \header has T24's 15 entries, which ly:module->alist gives in another order each run. */
const HEADER = `\\version "2.26.0"
\\header {
  mutopiatitle = "Jesu, meine Freude"
  mutopiacomposer = "BachJS"
  poet = "BWV 610"
  mutopiainstrument = "Organ"
  date = ""
  source = "Bach-Album"
  style = "Baroque"
  copyright = "Public Domain"
  maintainer = "Urs Metzger"
  maintainerEmail = "maintainer@example.org"
  lastupdated = "2006/Mar/15"
  title = "Jesu, meine Freude"
  composer = "Johann Sebastian Bach"
  footer = "Mutopia-2006/03/27-706"
  tagline = "t"
}
{ c'4 }
`

test('two captures of a score with a header of 15 entries and of T16 are the same byte for byte (MS3 step 12a)',
  { skip: skipCapture, timeout: 300_000 }, async () => {
    const file = path.join(scratch, 'header.ly')
    writeFileSync(file, HEADER)
    const twice = path.join(scratch, 'twice')
    for (const row of [{ id: 'header', path: file }, ten.find((r) => r.id === 'T16')]) {
      const dirs = ['a', 'b'].map((run) => path.join(twice, run, row.id))
      const results = await Promise.all(dirs.map((dir) => captureOne('lilypond', row, dir)))
      assert.deepEqual(results.map((r) => r.exit), [0, 0], row.id)
      // capture.json holds the time taken.
      const [a, b] = dirs.map((dir) => readdirSync(dir).filter((f) => f !== 'capture.json').sort())
      assert.deepEqual(b, a, `${row.id}: the same files`)
      for (const f of a) assert.ok(readFileSync(path.join(dirs[0], f)).equals(readFileSync(path.join(dirs[1], f))), `${row.id}/${f}`)
    }
    // The header's entries are written sorted by name.
    const alist = readFileSync(path.join(twice, 'a/header', MARKUPS), 'utf8').match(/\(((?:\(header:\S+ \. "[^"]*"\) ?){15})\)/)
    assert.ok(alist, 'the header\'s alist of 15')
    const names = [...alist[1].matchAll(/\(header:(\S+) /g)].map((m) => m[1])
    assert.deepEqual(names, [...names].sort())
    // T16's links name its included string below ref/T16, wherever the capture goes.
    const links = readFileSync(path.join(twice, 'a/T16/include-string.svg'), 'utf8').match(/textedit:\/\/[^"]*included[^"]*/g)
    assert.ok(links?.length > 0, 'T16\'s links to its included string')
    for (const link of links) assert.ok(decodeURIComponent(link).startsWith(`textedit://${linkDir('T16')}/<included string>:`), link)
  })

/** The core's examples/bench.rs with `args`: its JSON lines. */
function native(...args) {
  const run = runExample('bench', args, { release: true, cwd: engine, encoding: 'utf8', maxBuffer: 1 << 28 })
  assert.equal(run.status, 0, run.stderr)
  return run.stdout.trim().split('\n').map((line) => JSON.parse(line))
}

/** The packs the pages draw with, from the build's fonts/: Emmentaler-20 and the braces (T13). */
const PACKS = ['emmentaler-20', 'emmentaler-brace-0', 'emmentaler-brace-1', 'emmentaler-brace-2', 'emmentaler-brace-3']
const put = () => PACKS.map((name) =>
  `((@@ (lily) %put-font-pack) #vu8(${[...readFileSync(path.join(engine, 'dist/engine/fonts', `${name}.pack`))].join(' ')}))\n`).join('')

/**
 * A source that reads each record of `lines` back, writing for each the
 * font metrics in its stencil, a tab and the record written again, after
 * `!` if its extents are not the capture's.
 */
function readBack(lines) {
  const one = (line) => {
    const at = line.startsWith('(page ') ? 3 : 5
    return `(let* ((r '${line}) (d (list-tail r ${at})))
  (if (car d)
      (let ((s (apply %dump->stencil d)))
        (if (not (and (equal? (ly:stencil-extent s 0) (car d)) (equal? (ly:stencil-extent s 1) (cadr d)))) (display "!"))
        (display (fonts-in (ly:stencil-expr s)))
        (display "\\t")
        (write (append (list-head r ${at}) (%stencil->dump s))))
      (begin (display "0\\t") (write r)))
  (display "\\x1e\\n"))\n`
  }
  const fontsIn = `(define (fonts-in x)
  (cond ((ly:font-metric? x) 1) ((pair? x) (+ (fonts-in (car x)) (fonts-in (cdr x)))) (else 0)))\n`
  return `(set-current-module (resolve-module '(lily)))\n${put()}${fontsIn}${lines.map(one).join('')}`
}

test('the engine reads each page and markup of the 10 back into its stencils and fonts, from the image natively and in wasm',
  { skip: skipEngine, timeout: 600_000 }, async () => {
    const all = await capture()
    const mod = await WebAssembly.compile(readFileSync(coreWasm))
    const image = readFileSync(schemeImage)
    const fonts = {}
    for (const [id, { dir }] of all) {
      const lines = [PAGES, MARKUPS].flatMap((file) => readFileSync(path.join(dir, file), 'utf8').split('\n').filter(Boolean))
      const source = readBack(lines)
      const file = path.join(scratch, `${id}-read-back.scm`)
      writeFileSync(file, source)
      const [fromImage] = native('--image', schemeImage, file)
      const wasm = await runOnce(mod, Buffer.from(source), image)
      assert.equal(wasm.init, 0, 'lsb_restore takes the image')
      for (const [name, run] of [['from the image natively', fromImage], ['from the image in wasm', wasm]]) {
        assert.ok(run.ok, `${id} ${name}: ${run.output.slice(-400)}`)
        const got = run.output.split('\u001e\n')
        assert.equal(got.pop(), '', `${id} ${name}: the output ends a record`)
        assert.deepEqual(got.filter((record) => record.startsWith('!')), [], `${id} ${name}: extents that are not the capture's`)
        const back = got.map((record) => record.slice(record.indexOf('\t') + 1))
        assert.deepEqual(compareRecords(readRecords(lines.join('\n')), readRecords(back.join('\n'))), [], `${id} ${name}`)
        assert.deepEqual(back, lines, `${id} ${name}: every record written back as captured`)
        // Each font of a pack in a page read as a font metric; a markup's chain may name fonts too, which stay data.
        got.forEach((record, i) => {
          const [read, written] = [Number(record.split('\t')[0]), lines[i].split('(%font "').length - 1]
          assert.ok(lines[i].startsWith('(page ') ? read === written : read <= written, `${id} ${name}: ${read} fonts of ${written}`)
          fonts[name] = (fonts[name] ?? 0) + read
        })
      }
    }
    assert.ok(fonts['from the image natively'] > 0 && fonts['from the image natively'] === fonts['from the image in wasm'], JSON.stringify(fonts))
  })

/** The packs the markups draw with: Emmentaler-20 and -13 (T10's volta numbers), the braces and C059's three. */
const TEXT_PACKS = [...PACKS, 'emmentaler-13', 'C059-Roman', 'C059-Bold', 'C059-Italic']

/**
 * The markups of the 10 in Emmentaler's text (fetaText) that were not
 * LilyPond's until its packs held GPOS's kern and GSUB's features (W6, MS3
 * step 51, D209): T10's volta numbers, which \volta-number draws with cv47
 * and ss01 (fattened digits), and the dynamics of T13 ("sfzp") and T18
 * ("ff"), which kern spaces. Each is LilyPond's now, as every other is.
 */
const EMMENTALER_TEXT = ['T10 24', 'T10 25', 'T10 26', 'T10 27', 'T13 3', 'T18 3', 'T18 5', 'T18 7']

/** A glyph string's font file by its name only: the engine's are under <datadir>. */
const fontFiles = (text) => text.replace(/"[^"\n]*\/fonts\/otf\/([^"/]+\.otf)"/g, '"$1"')

/**
 * A source that interprets each markup record of `lines` with its chain and
 * writes the record with the engine's stencil, or `!` and the error.
 */
function interpretAll(lines) {
  const put = TEXT_PACKS.map((name) =>
    `((@@ (lily) %put-font-pack) #vu8(${[...readFileSync(path.join(engine, 'dist/engine/fonts', `${name}.pack`))].join(' ')}))\n`).join('')
  return `(set-current-module (resolve-module '(lily)))
${put};; paper.scm's scale-layout, as paper-book.cc scales the book's \\paper.
(define (probe-scale paper scale)
  (let* ((new (ly:output-def-clone paper))
         (old (ly:output-def-scope paper))
         (scope (ly:output-def-scope new)))
    (for-each (lambda (v)
                (if (module-defined? old v)
                    (let ((val (module-ref old v)))
                      (if (number? val) (module-define! scope v (/ val scale))))))
              (ly:output-def-lookup paper 'dimension-variables))
    (ly:output-def-set-variable! new 'cloned #t)
    new))
(define probe-paper
  (let ((paper (cdddr (assq '$defaultpaper lilypond-declarations))))
    (probe-scale paper (ly:output-def-lookup paper 'output-scale))))
;; The procedures the paper's titles, headers and footers hold, by name:
;; titling-init.ly's conditions, which no module exports.
(define probe-named '())
(define (probe-collect x)
  (cond ((procedure? x)
         (let ((name (procedure-name x)))
           (if (and name (not (assq name probe-named)))
               (set! probe-named (acons name x probe-named)))))
        ((pair? x) (probe-collect (car x)) (probe-collect (cdr x)))))
(for-each (lambda (v) (probe-collect (ly:output-def-lookup probe-paper v '())))
          '(bookTitleMarkup scoreTitleMarkup oddHeaderMarkup evenHeaderMarkup oddFooterMarkup evenFooterMarkup))
(define (probe-rebuild x)
  (cond ((and (pair? x) (eq? (car x) '%procedure) (pair? (cdr x)) (null? (cddr x)) (symbol? (cadr x)))
         (cond ((module-defined? (resolve-module '(lily)) (cadr x)) (module-ref (resolve-module '(lily)) (cadr x)))
               ((assq (cadr x) probe-named) => cdr)
               (else x)))
        ((pair? x) (cons (probe-rebuild (car x)) (probe-rebuild (cdr x))))
        ((vector? x) (list->vector (map probe-rebuild (vector->list x))))
        (else x)))
(define (probe-one r)
  (catch #t
    (lambda ()
      (write (append (list-head r 5)
                     (%stencil->dump (interpret-markup probe-paper (probe-rebuild (list-ref r 4))
                                                       (probe-rebuild (list-ref r 3)))))))
    (lambda (key . args) (display "!") (write (list (list-ref r 1) key args))))
  (display "\\x1e\\n"))
${lines.map((line) => `(probe-one '${line})\n`).join('')}`
}

test('the engine interprets each markup of the 10 with its chain into LilyPond\'s stencil, from source natively, from the image natively and in wasm',
  { skip: skipEngine, timeout: 900_000 }, async () => {
    const all = await capture()
    const mod = await WebAssembly.compile(readFileSync(coreWasm))
    const image = readFileSync(schemeImage)
    const differ = {}
    let markups = 0
    for (const [id, { dir }] of all) {
      const lines = readFileSync(path.join(dir, MARKUPS), 'utf8').split('\n').filter(Boolean)
      const source = interpretAll(lines)
      const file = path.join(scratch, `${id}-interpret.scm`)
      writeFileSync(file, source)
      const [fromSource] = native('--library', lilyScm(), file)
      const [fromImage] = native('--image', schemeImage, file)
      const wasm = await runOnce(mod, Buffer.from(source), image)
      assert.equal(wasm.init, 0, 'lsb_restore takes the image')
      const reference = readRecords(fontFiles(lines.join('\n')))
      for (const [name, run] of [['from source natively', fromSource], ['from the image natively', fromImage], ['from the image in wasm', wasm]]) {
        assert.ok(run.ok, `${id} ${name}: ${run.output.slice(-400)}`)
        const got = run.output.split('\u001e\n')
        assert.equal(got.pop(), '', `${id} ${name}: the output ends a record`)
        assert.deepEqual(got.filter((record) => record.startsWith('!')), [], `${id} ${name}: markups the engine could not interpret`)
        assert.equal(got.length, lines.length, `${id} ${name}: every markup`)
        const records = readRecords(fontFiles(got.join('\n')))
        // Each markup's own input, as captured.
        assert.deepEqual(compareRecords(reference, records).filter((d) => d.kind === 'input' || d.kind === 'count'), [], `${id} ${name}`)
        const differing = reference.filter((r, i) => compareRecords([r], [records[i]], { digits: 4 }).length > 0)
          .map((r) => `${id} ${r.input[0].text}`)
        differ[name] = [...(differ[name] ?? []), ...differing]
      }
      markups += lines.length
    }
    assert.equal(markups, 275, 'the 10\'s markups')
    for (const [name, records] of Object.entries(differ)) assert.deepEqual(records, [], name)
  })

/** The types of the 10's grobs that print with ly:text-interface::print and the engine makes (MS3 step 19). */
const TEXT_GROBS = ['TextScript', 'DynamicText']
/**
 * What LilyPond's layout has set on a text grob when it prints it, and the
 * engine's, of parts 3 to 5, not yet: the callbacks under way that asked for
 * the stencil, and the spacing's skylines.
 */
const LATER = ['Y-extent', 'X-offset', 'horizontal-skylines']

/**
 * A file that compiles `score`, in capture.ly's text families as the
 * engine's paper names them (D106, D210), and, in its Score's finalize, reads the stencil of each grob of TEXT_GROBS it
 * heard, whose definition's ly:text-interface::print is stencils.ly's
 * stencils-print here: each print written to `output` as stencils.ly writes
 * it, the markup the grob's text, the chain text_font_alist_chain's. A stream
 * event is written as LilyPond's printed form names it, a Prob (as the engine
 * prints it since MS8 step 11).
 */
function printDriver(score, output) {
  return `\\version "2.26.0"
#(begin
  (define stencils-records '())
  (define (stencils-type x)
    (let* ((s (format #f "~a" x))
           (end (string-index s (lambda (c) (memv c '(#\\space #\\> #\\:))) 2)))
      (if (string-prefix? "#<" s) (substring s 2 (or end (string-length s))) s)))
  (define (stencils-datum x)
    (cond ((or (number? x) (string? x) (symbol? x) (boolean? x) (char? x) (null? x) (keyword? x)) x)
          ((pair? x) (cons (stencils-datum (car x)) (stencils-datum (cdr x))))
          ((vector? x) (list->vector (map stencils-datum (vector->list x))))
          ((ly:font-metric? x) (list '%font (ly:font-file-name x) (ly:font-name x) (ly:font-magnification x)))
          ((procedure? x) (list '%procedure (procedure-name x)))
          ((ly:grob? x) (list '%grob (grob::name x)))
          ((ly:stream-event? x) (list '%object "Prob"))
          (else (list '%object (stencils-type x)))))
  (define (stencils-print grob)
    (let* ((markup (ly:grob-property grob 'text))
           (chain (ly:grob-alist-chain grob '((font-encoding . latin1))))
           (stencil (ly:text-interface::print grob))
           (link ((module-ref (resolve-module '(lily)) '%grob-cause-link) grob)))
      (set! stencils-records
            (cons (cons* 'markup (1+ (length stencils-records)) (list 'print (grob::name grob) link)
                         (stencils-datum markup) (stencils-datum chain)
                         ((module-ref (resolve-module '(lily)) '%stencil->dump) stencil))
                  stencils-records))
      stencil))
  (define stencils-grob-descriptions
    (map (lambda (description)
           (cons (car description)
                 (map (lambda (entry)
                        (if (and (eq? (car entry) 'stencil) (eq? (cdr entry) ly:text-interface::print))
                            (cons 'stencil stencils-print)
                            entry))
                      (cdr description))))
         all-grob-descriptions))
  (define stencils-heard '())
  (define (stencils-recorder context)
    (make-engraver
     (acknowledgers
      ((grob-interface engraver grob source-engraver)
       (when (memq (grob::name grob) '(${TEXT_GROBS.join(' ')}))
         (set! stencils-heard (cons grob stencils-heard)))))
     ((finalize engraver)
      (for-each (lambda (grob) (ly:grob-property grob 'stencil)) (reverse stencils-heard))
      (with-output-to-file "${output}"
        (lambda () (for-each (lambda (r) (write r) (newline)) (reverse stencils-records))))))))
\\layout {
  \\context { \\Global \\grobdescriptions #stencils-grob-descriptions }
  \\context { \\Score \\consists #stencils-recorder }
}
\\include "${score}"
`
}

/** An alist's entries, each key written to its value written. */
const entries = (alist) => new Map(alist.items.map((entry) => [writeSexp(entry.items[0]), writeSexp({ ...entry, items: entry.items.slice(1) })]))

test('the engine prints each text script and dynamic of the 10 from its own grob with the markup, chain and stencil LilyPond prints for the same cause, natively and in wasm',
  { skip: skipEngine, timeout: 900_000 }, async () => {
    const all = await capture()
    const printed = new RegExp(`^\\(markup \\d+ \\(print (${TEXT_GROBS.join('|')}) `)
    const references = new Map(ten.map((row) => [row.id, readFileSync(path.join(all.get(row.id).dir, MARKUPS), 'utf8')
      .split('\n').filter((line) => printed.test(line))]).filter(([, lines]) => lines.length > 0))
    const rows = ten.filter((row) => references.has(row.id))
    assert.deepEqual(rows.map((row) => row.id), ['T05', 'T10', 'T13', 'T17', 'T18'])
    const drivers = rows.map((row) => {
      const file = path.join(scratch, `${row.id}-print.ly`)
      writeFileSync(file, printDriver(row.path, `${row.id}-print.scm`))
      return file
    })
    const packs = TEXT_PACKS.map((name) => [name, path.join(engine, 'dist/engine/fonts', `${name}.pack`)])
    const out = mkdtempSync(path.join(scratch, 'print-'))
    const run = runExample('compile', [...packs.flatMap(([, pack]) => ['--pack', pack]), '--out', out, ...drivers],
      { cwd: out, encoding: 'utf8', maxBuffer: 1 << 26 })
    assert.equal(run.status, 0, run.stderr)
    const wasm = await compileInWasm(drivers, { modules: packs.map(([name, pack]) => [name, readFileSync(pack)]) })
    const differ = { natively: [], 'in wasm': [] }
    const labels = []
    let grobs = 0
    for (const [i, row] of rows.entries()) {
      assert.equal(readFileSync(path.join(out, `${row.id}-print.err`), 'utf8'), '', `${row.id} natively: stderr`)
      assert.equal(wasm[i].outputs.stderr.toString(), '', `${row.id} in wasm: stderr`)
      const reference = readRecords(fontFiles(references.get(row.id).join('\n')))
      labels.push(...reference.map((r) => `${row.id} ${writeSexp(r.input[0])}`))
      for (const [name, text] of [
        ['natively', readFileSync(path.join(out, `${row.id}-print.scm`), 'utf8')],
        ['in wasm', wasm[i].outputs[`${row.id}-print.scm`]?.toString() ?? ''],
      ]) {
        const got = readRecords(fontFiles(text))
        assert.equal(got.length, reference.length, `${row.id} ${name}: a print of each grob LilyPond prints`)
        for (const r of reference) {
          // The engine's print of the grob of the same cause: its type and link.
          const at = got.findIndex((g) => writeSexp(g.input[1]) === writeSexp(r.input[1]))
          assert.ok(at >= 0, `${row.id} ${name}: ${writeSexp(r.input[1])}`)
          const [g] = got.splice(at, 1)
          const label = `${row.id} ${writeSexp(r.input[0])}`
          assert.equal(writeSexp(g.input[2]), writeSexp(r.input[2]), `${label} ${name}: the markup`)
          // The chain: the definition and text_font_alist_chain's default as
          // LilyPond's; each property the grob has, LilyPond's grob has, unless
          // LilyPond's is under way, and LilyPond's others are LATER's.
          const [rc, gc] = [r.input[3].items, g.input[3].items]
          assert.equal(gc.length, 3, `${label} ${name}: a chain of three`)
          assert.equal(writeSexp(gc[1]), writeSexp(rc[1]), `${label} ${name}: the definition`)
          assert.equal(writeSexp(gc[2]), writeSexp(rc[2]), `${label} ${name}: the default`)
          const [rm, gm] = [entries(rc[0]), entries(gc[0])]
          for (const [key, value] of gm) {
            assert.ok(rm.get(key) === value || rm.get(key) === '( . calculation-in-progress)', `${label} ${name}: ${key} ${value}, LilyPond's ${rm.get(key)}`)
          }
          assert.deepEqual([...rm.keys()].filter((key) => !gm.has(key) && !LATER.includes(key)), [], `${label} ${name}`)
          // The stencil, every digit.
          if (compareRecords([{ ...r, input: [] }], [{ ...g, input: [] }]).length > 0) differ[name].push(label)
          grobs++
        }
      }
    }
    // 35 TextScripts (T05's 28, T10's 3, T13's and T17's 2) and T18's 5 DynamicTexts, each natively and in wasm.
    assert.equal(labels.length, 40)
    assert.equal(grobs, 2 * 40)
    // Step 8's Emmentaler strings among them, T13's "sfzp" and T18's three
    // "ff", LilyPond's since step 51 (W6), as every other is.
    const emmentaler = EMMENTALER_TEXT.filter((label) => labels.includes(label))
    assert.deepEqual(emmentaler, ['T13 3', 'T18 3', 'T18 5', 'T18 7'])
    for (const [name, differing] of Object.entries(differ)) assert.deepEqual(differing, [], name)
  })

/** A file that measures text in the three styles the core preloads, and warns each extent. */
const TEXT = `\\version "2.26.0"
#(define (text-show props string)
   (let ((stencil (interpret-markup $defaultpaper
                    (list props '((font-encoding . latin1)) (ly:output-def-lookup $defaultpaper 'property-defaults))
                    string)))
     (ly:warning "~a ~a ~a" string (ly:stencil-extent stencil 0) (ly:stencil-extent stencil 1))))
#(text-show '() "Tempo office fluff AVATAR")
#(text-show '((font-series . bold)) "Allegro, 2x 3alt")
#(text-show '((font-shape . italic) (font-size . -2)) "dolce: ff ffi")
`

test('the core measures text as LilyPond does with the C059 packs the manifest preloads, natively and in wasm',
  { skip: skipEngine, timeout: 600_000 }, async () => {
    const file = path.join(scratch, 'text.ly')
    writeFileSync(file, TEXT)
    const reference = spawnSync('lilypond', ['--loglevel=WARNING', '-o', path.join(scratch, 'text'), file],
      { cwd: scratch, encoding: 'utf8', env: { ...process.env, LANGUAGE: 'en' } })
    assert.equal(reference.status, 0, reference.stderr)
    assert.equal(reference.stderr.split('\n').filter((line) => line.startsWith('warning: ')).length, 3, reference.stderr)
    const dist = path.join(engine, 'dist/engine')
    const manifest = JSON.parse(readFileSync(path.join(dist, 'manifest.json'), 'utf8'))
    const text = ['C059-Roman', 'C059-Bold', 'C059-Italic']
    assert.deepEqual(manifest.core.preload, ['emmentaler-20', ...text])
    const modules = manifest.core.preload.map((name) => [name, path.join(dist, manifest.modules[name].file)])
    const out = path.join(scratch, 'text-native')
    mkdirSync(out)
    const run = runExample('compile', [...modules.flatMap(([, pack]) => ['--pack', pack]), '--out', out, file],
      { cwd: engine, encoding: 'utf8', maxBuffer: 1 << 26 })
    assert.equal(run.status, 0, run.stderr)
    assert.equal(readFileSync(path.join(out, 'text.err'), 'utf8'), reference.stderr, 'natively')
    const [wasm] = await compileInWasm([file], { modules: modules.map(([name, pack]) => [name, readFileSync(pack)]) })
    assert.equal(wasm.outputs.stderr.toString(), reference.stderr, 'in wasm')
  })

/** The references not at staff size 20 (crates/layout/tests/markups.rs' STAFF). */
const STAFF = { T21: 24, T23: 15, T25: 14, T28: 14, T29: 10, T30: 14, Y45: 26, Y51: 21, Y54: 21, Y55: 21 }
/** The references whose strings LilyPond drew in fonts no pack holds, and what of them is LilyPond's (markups.rs' APART). */
const APART = { T20: 'recorded', Y51: 'recorded', Y54: 'recorded', Y55: 'recorded' }

/**
 * A source that makes each `utf-8-string` of `lines` again, as markups.rs'
 * AGAIN does, in a paper of staff size `staff`, given every text pack of
 * the build: a line for each, `#t` if it is the capture's, `face` if it is
 * but for the face its glyph strings name, else the engine's string.
 */
function again(lines, staff) {
  const fonts = path.join(engine, 'dist/engine/fonts')
  const put = readdirSync(fonts).filter((f) => f.endsWith('.pack') && !f.startsWith('emmentaler')).map((f) =>
    `((@@ (lily) %put-font-pack) #vu8(${[...readFileSync(path.join(fonts, f))].join(' ')}))\n`).join('')
  return `(set-current-module (resolve-module '(lily)))
${put}(define (probe-strings x acc)
  (cond ((and (pair? x) (eq? (car x) 'utf-8-string) (pair? (cdr x)) (string? (cadr x))) (cons x acc))
        ((pair? x) (probe-strings (cdr x) (probe-strings (car x) acc)))
        (else acc)))
(define (probe-base s) (let ((i (string-rindex s #\\/))) (if i (substring s (1+ i)) s)))
(define (probe-norm x)
  (cond ((and (pair? x) (eq? (car x) 'glyph-string) (= (length x) 10))
         (append (list-head x 6) (list (probe-base (list-ref x 6))) (list-tail x 7)))
        ((pair? x) (cons (probe-norm (car x)) (probe-norm (cdr x))))
        (else x)))
(define (probe-faceless x)
  (cond ((and (pair? x) (eq? (car x) 'glyph-string) (= (length x) 10))
         (append (list (car x) (cadr x) (list-ref x 3) (list-ref x 4))
                 (list (map (lambda (g) (append (list-head g 4) (list-tail g 5))) (list-ref x 5)))
                 (list-tail x 7)))
        ((pair? x) (cons (probe-faceless (car x)) (probe-faceless (cdr x))))
        (else x)))
(define probe-layout
  (let ((layout (ly:make-output-def)) (height (* ${staff} (ly:pt 1))))
    (ly:output-def-set-variable! layout 'staff-height height)
    (ly:output-def-set-variable! layout 'output-scale (/ height 4))
    layout))
(define (probe-again node)
  (let* ((words (string-split (cadr node) #\\space))
         (size (inexact->exact (floor (+ (* (string->number (car (last-pair words))) 1024) 0.5))))
         (layout (ly:output-def-clone probe-layout)))
    (ly:output-def-set-variable! layout 'text-font-size (/ size 1024 (ly:pt 1)))
    (let* ((s (interpret-markup layout (list (list (cons 'font-name (cadr node)))) (caddr node)))
           (got (probe-norm (caddr (%stencil->dump s))))
           (want (probe-norm node)))
      (cond ((equal? got want) (display "#t"))
            ((equal? (probe-faceless got) (probe-faceless want)) (display "face"))
            (else (write got)))
      (newline))))
${lines.map((line) => `(for-each probe-again (probe-strings '${line} '()))\n`).join('')}`
}

test('every reference\'s strings made again from the image natively and in wasm are the same and LilyPond\'s, but T20\'s lyrics (MS4 step 19; T26\'s too since E23)',
  { skip: skipEngine, timeout: 900_000 }, async () => {
    const refs = path.join(corpusDir(), 'ref')
    const ids = readdirSync(refs).filter((id) => existsSync(path.join(refs, id, MARKUPS))).sort()
    assert.ok(ids.length >= 42, `${ids.length} references`)
    const mod = await WebAssembly.compile(readFileSync(coreWasm))
    const image = readFileSync(schemeImage)
    const kinds = {}
    let strings = 0
    for (const id of ids) {
      const lines = [PAGES, MARKUPS].flatMap((file) => readFileSync(path.join(refs, id, file), 'utf8').split('\n'))
        .filter((line) => line.includes('(utf-8-string '))
      if (lines.length === 0) continue
      const source = again(lines, STAFF[id] ?? 20)
      const file = path.join(scratch, `${id}-again.scm`)
      writeFileSync(file, source)
      const [natively] = native('--image', schemeImage, file)
      const wasm = await runOnce(mod, Buffer.from(source), image)
      assert.equal(wasm.init, 0, 'lsb_restore takes the image')
      assert.ok(natively.ok, `${id} natively: ${natively.output.slice(-400)}`)
      assert.ok(wasm.ok, `${id} in wasm: ${wasm.output.slice(-400)}`)
      assert.equal(wasm.output, natively.output, `${id}: in wasm as natively`)
      for (const result of natively.output.split('\n').filter(Boolean)) {
        const kind = result === '#t' ? 'LilyPond\'s' : result === 'face' ? 'LilyPond\'s but for the face' : 'other'
        if (kind !== 'LilyPond\'s' && !(APART[id] === 'face' && kind !== 'other') && APART[id] !== 'recorded') {
          assert.fail(`${id}: a string not LilyPond's: ${result}`)
        }
        if (APART[id]) kinds[`${id} ${kind}`] = (kinds[`${id} ${kind}`] ?? 0) + 1
        strings++
      }
    }
    // T26's 42, its time signatures in TeX Gyre Schola's pack among them, are LilyPond's (E23, D271).
    // T20's lyrics and typography-demo.ly's strings (Y51, Y54, Y55) in Times New Roman (D227).
    assert.deepEqual(kinds, { 'T20 other': 77, 'Y51 other': 122, 'Y54 other': 116, 'Y55 other': 116 })
    assert.ok(strings >= 1504, `${strings} strings`)
  })
