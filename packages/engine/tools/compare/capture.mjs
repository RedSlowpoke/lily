// Captures LilyPond's reference for each score of the corpus (chapter 00
// §5.1): one cold compile per score with capture.ly as the settings file,
// into <out>/<ID>/: the SVG pages with their textedit links, the .midi files,
// the playback map (.timing.json), chapter 05's shape dump (c5.tsv, from
// c5dump.ly), the pages' stencils and the markups' (pages.scm and
// markups.scm, from stencils.ly; stencils.mjs reads them), the grobs each
// translation announces (grobs.tsv, from grobs.ly), the live grobs of each
// system after page breaking (layout.tsv, from layout.ly), the springs and
// rods between the columns after set-springs (columns.tsv, from layout.ly;
// MS3 step 31, D128), each system's columns and their X offsets (line.tsv,
// from layout.ly; MS3 step 32, D129), stderr.txt, and
// capture.json with the exit status and the time taken. Text is set in
// LilyPond's own fonts, not the machine's (D106). score.mjs compares two such
// directories. The ID `grob-calls`, captured only when named, is
// grob-calls.ly, whose calls on fixtures/grobs.ly's grobs also write
// grob-calls.txt (MS3 step 13). So are `path` and `y1-unreached`, part 3's
// fixtures (MS3 step 25): fixtures/path.ly, the layout path's staff of skips,
// and fixtures/y1-unreached.ly, Y1's grobs that none of the 10 reaches; and
// part 4's (MS3 step 47), Y2's grobs that none of the 10 reaches:
// fixtures/scripts.ly, fingerings.ly, arpeggios.ly, line-spanners.ly,
// brackets.ly, metronome.ly and bar-numbers-staves.ly. A capture keeps the
// files of KEPT another tool wrote into its directory (OPEN H5).
//
// With --parse, the references of MS2's parser steps instead (02 §5, 04 §6),
// into <out>/<ID>/: tokens.txt, the tokens Bison read in the main file's
// parse, from -ddebug-parser's trace (tokens.mjs), music.txt, each score's
// music trees as the book handler gets them (music.ly; music.mjs compares
// two), stderr.txt without the trace, and capture.json. Nothing is laid out.
// Its IDs are those of the corpus, `syntax` (packages/common's syntax.ly) and
// `init`; the default is T01–T30, syntax and init.
//
// `init` is the parse of declarations-init.ly that starts every session
// (ly:parse-init), into <out>/init/<file>/tokens.txt for it and each file it
// includes, those it includes with it. The trace starts at \maininput only,
// so ly:parse-init is given a file that starts with \maininput (whose main
// file it does not find) and includes declarations-init.ly; each \include
// of the init is wrapped in `$` forms of no value, which give no token and
// write `@@ begin <file>` and `@@ end <file>` into the trace (copies of the
// files that include, in <out>/init/wrap/).
//
//   node tools/compare/capture.mjs [--parse] [--out dir] [--settings file.ly] [--corpus dir] [ID…]
//                                          or `npm run capture` in packages/engine
//
//   --parse      the parser's references (default out <corpus dir>/parse)
//   --out        where the captures go (default <corpus dir>/ref)
//   --settings   the settings file instead of capture.ly, e.g. one that includes
//                capture.ly and adds a change to score against the reference
//   --corpus     the corpus directory of fetch.mjs (default LILY_CORPUS or
//                ~/.cache/lily-engine); run fetch.mjs first for groups B and C
//   ID…          only these scores (default all 132 of corpus.tsv); grob-calls for grob-calls.ly,
//                path and y1-unreached for part 3's fixtures, scripts, fingerings,
//                arpeggios, line-spanners, brackets, metronome and
//                bar-numbers-staves for part 4's
//   LILYPOND     the lilypond to run (default the one on the PATH); it must be 2.26.0
import { spawn, spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from 'node:fs'
import { availableParallelism } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { createRequire } from 'node:module'
import { corpus, corpusDir } from '../corpus/fetch.mjs'
import { SHAPES } from './score.mjs'
import { BISON_LINE, byFile, traceTokens } from './tokens.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
export const CAPTURE = path.join(here, 'capture.ly')
export const MUSIC = path.join(here, 'music.ly')
/** The files stencils.ly writes into a capture's directory. */
export const PAGES = 'pages.scm'
export const MARKUPS = 'markups.scm'
/** The file grobs.ly writes into a capture's directory. */
export const GROBS = 'grobs.tsv'
/** The file layout.ly writes into a capture's directory (MS3 step 25). */
export const LAYOUT = 'layout.tsv'
/** The file of the columns' springs and rods layout.ly writes (MS3 step 31, D128). */
export const COLUMNS = 'columns.tsv'
/** The file of each system's line's columns and their X offsets layout.ly writes (MS3 step 32, D129). */
export const LINE = 'line.tsv'
/** The files another tool writes into a capture's directory, which a capture keeps: trace.mjs' trace.tsv (OPEN H5). */
export const KEPT = ['trace.tsv']
/** The calls on the fixture's grobs (MS3 step 13) and the file they write into a capture's directory. */
export const GROB_CALLS_LY = path.join(here, 'grob-calls.ly')
export const GROB_CALLS = 'grob-calls.txt'
export const SYNTAX = fileURLToPath(import.meta.resolve('@lily/common/test/grammar/syntax.ly'))
const INCLUDE = /^(\s*)\\include "([^"]+)"/gm
// timing.ly, which capture.ly includes by name.
const runtime = path.dirname(createRequire(import.meta.url).resolve('@lily/common/runtime/timing.ly'))
const VERSION = '2.26.0'

export function lilypondVersion(lilypond = process.env.LILYPOND ?? 'lilypond') {
  const run = spawnSync(lilypond, ['--version'], { encoding: 'utf8' })
  return run.status === 0 ? run.stdout.match(/GNU LilyPond (\S+)/)?.[1] : undefined
}

/**
 * The directory the links of `id`'s capture name a file given relatively
 * under (T16's included string): <corpus dir>/ref/<ID>, the one a capture
 * into ref/ writes into, wherever the capture goes (capture.ly, MS3 step 12a).
 */
export const linkDir = (id, corpus) => path.join(corpusDir(corpus), 'ref', id)

/** Captures `row` ({ id, path }) into `dir` with `settings` and `env` added; resolves to its result. */
export function captureOne(lilypond, row, dir, settings = CAPTURE, env = {}) {
  const kept = KEPT.map((f) => path.join(dir, f)).filter((f) => existsSync(f)).map((f) => [f, readFileSync(f)])
  rmSync(dir, { recursive: true, force: true })
  mkdirSync(dir, { recursive: true })
  for (const [file, bytes] of kept) writeFileSync(file, bytes)
  const base = path.basename(row.path, '.ly')
  const args = ['--loglevel=WARNING', '--svg', '-dpoint-and-click', `-dinclude-settings=${settings}`,
    '-I', runtime, '-I', here, '-o', path.join(dir, base), row.path]
  const start = performance.now()
  return new Promise((resolve) => {
    const child = spawn(lilypond, args, { cwd: path.dirname(row.path), env: {
      ...process.env, LANGUAGE: 'en', C5_OUT: path.join(dir, SHAPES), PAGES_OUT: path.join(dir, PAGES), MARKUPS_OUT: path.join(dir, MARKUPS),
      GROBS_OUT: path.join(dir, GROBS), LAYOUT_OUT: path.join(dir, LAYOUT), COLUMNS_OUT: path.join(dir, COLUMNS),
      LINE_OUT: path.join(dir, LINE),
      GROB_CALLS_OUT: path.join(dir, GROB_CALLS), CAPTURE_CWD: linkDir(row.id), ...env,
    } })
    const stderr = []
    child.stderr.on('data', (chunk) => stderr.push(chunk))
    child.on('error', (error) => stderr.push(Buffer.from(String(error))))
    child.on('close', (exit) => {
      const ms = Math.round(performance.now() - start)
      writeFileSync(path.join(dir, 'stderr.txt'), Buffer.concat(stderr))
      const files = readdirSync(dir)
      const result = {
        id: row.id, file: row.file, exit, ms,
        pages: files.filter((f) => f.endsWith('.svg')).length,
        midi: files.filter((f) => /\.midi?$/.test(f)).length,
        timing: files.some((f) => f.endsWith('.timing.json')),
        shapes: files.includes(SHAPES),
        stencils: count(path.join(dir, PAGES)),
        markups: count(path.join(dir, MARKUPS)),
        grobs: count(path.join(dir, GROBS)),
        ...files.includes(GROB_CALLS) && { calls: count(path.join(dir, GROB_CALLS)) },
      }
      writeFileSync(path.join(dir, 'capture.json'), JSON.stringify({ ...result, lilypond: VERSION, settings }, null, 1) + '\n')
      resolve(result)
    })
  })
}

/** The records of a file stencils.ly, grobs.ly or grob-calls.ly wrote: one a line. */
function count(file) {
  return existsSync(file) ? readFileSync(file, 'utf8').split('\n').filter(Boolean).length : 0
}

/** Runs lilypond with -ddebug-parser on `file` from its directory; resolves to its exit, time and stderr. */
function traced(lilypond, args, file, env) {
  const start = performance.now()
  return new Promise((resolve) => {
    const child = spawn(lilypond, ['--loglevel=WARNING', '-ddebug-parser', '-dno-print-pages', ...args, file],
      { cwd: path.dirname(file), env: { ...process.env, LANGUAGE: 'en', ...env } })
    const stderr = []
    child.stderr.on('data', (chunk) => stderr.push(chunk))
    child.on('error', (error) => stderr.push(Buffer.from(String(error))))
    child.on('close', (exit) => resolve({ exit, ms: Math.round(performance.now() - start), trace: Buffer.concat(stderr).toString('utf8') }))
  })
}

/** Writes `tokens`, a trace's stderr without it, and capture.json for `result` into `dir`. */
function writeParse(dir, tokens, trace, result) {
  writeFileSync(path.join(dir, 'tokens.txt'), tokens.map((t) => t + '\n').join(''))
  if (trace !== undefined) writeFileSync(path.join(dir, 'stderr.txt'), trace.split('\n').filter((l) => !BISON_LINE.test(l)).join('\n'))
  writeFileSync(path.join(dir, 'capture.json'), JSON.stringify({ ...result, lilypond: VERSION }, null, 1) + '\n')
}

/** The parser's references for the score `row` into `dir`: its tokens and music trees (--parse). */
async function compileParse(lilypond, row, dir) {
  rmSync(dir, { recursive: true, force: true })
  mkdirSync(dir, { recursive: true })
  const music = path.join(dir, 'music.txt')
  const args = [`-dinclude-settings=${MUSIC}`, '-o', path.join(dir, path.basename(row.path, '.ly'))]
  const { exit, ms, trace } = await traced(lilypond, args, row.path, { MUSIC_OUT: music })
  const tokens = traceTokens(trace)
  const scores = existsSync(music) ? readFileSync(music, 'utf8').split('\n').filter((l) => l.startsWith('score ')).length : 0
  const result = { id: row.id, file: row.file, exit, ms, tokens: tokens.length, scores }
  writeParse(dir, tokens, trace, result)
  return result
}

/** The `ly` directory of `lilypond`'s 2.26.0. */
function lyDir(lilypond) {
  const binary = spawnSync('sh', ['-c', `command -v "${lilypond}"`], { encoding: 'utf8' }).stdout.trim()
  return path.join(path.dirname(realpathSync(binary)), `../share/lilypond/${VERSION}/ly`)
}

/** The tokens of the init's parse into `dir`/<file>/ (--parse init); resolves to its result. */
async function captureInit(lilypond, dir) {
  rmSync(dir, { recursive: true, force: true })
  const wrap = path.join(dir, 'wrap')
  mkdirSync(wrap, { recursive: true })
  const ly = lyDir(lilypond)
  const mark = (what, file) =>
    `$(begin (display "\\n@@ ${what} ${file}\\n" (current-error-port)) (force-output (current-error-port)) *unspecified*)`
  const included = (indent, file) =>
    `${indent}${mark('begin', file)}\n${indent}\\include "${copy(file)}"\n${indent}${mark('end', file)}`
  // The file to include for `file`: a copy with its includes wrapped, if it has any.
  function copy(file) {
    const text = readFileSync(path.join(ly, file), 'utf8')
    if (!text.match(INCLUDE)) return path.join(ly, file)
    writeFileSync(path.join(wrap, file), text.replace(INCLUDE, (_, indent, name) => included(indent, name)))
    return path.join(wrap, file)
  }
  const root = path.join(wrap, 'root.ly')
  writeFileSync(root, `\\maininput\n${included('', 'declarations-init.ly')}\n`)
  const main = path.join(dir, 'main.ly')
  writeFileSync(main, `\\version "${VERSION}"\n`)
  const parseInit = `(let* ((m (resolve-module '(lily))) (parse (module-ref m 'ly:parse-init)))
    (module-set! m 'ly:parse-init (lambda (file) (parse ${JSON.stringify(root)}))))`
  const { exit, ms, trace } = await traced(lilypond, ['-e', parseInit], main)
  const files = byFile(traceTokens(trace))
  for (const [file, tokens] of files) {
    mkdirSync(path.join(dir, file), { recursive: true })
    writeParse(path.join(dir, file), tokens, undefined, { id: `init/${file}`, file, tokens: tokens.length })
  }
  const all = files.get('declarations-init.ly') ?? []
  const result = { id: 'init', file: 'declarations-init.ly', exit, ms, tokens: all.length, scores: 0, files: files.size }
  writeParse(dir, all, trace, result)
  return result
}

/** grob-calls.ly's row, which capture() takes only when named. */
const GROB_CALLS_ROW = { id: 'grob-calls', file: 'tools/compare/grob-calls.ly', path: GROB_CALLS_LY }
/**
 * Part 3's fixtures' rows (MS3 step 25), part 4's (MS3 step 47) and broken-spanners.ly, broken into lines (MS3 step
 * 60), which capture() takes only when named.
 */
export const LAYOUT_FIXTURES = ['path', 'y1-unreached',
  'scripts', 'fingerings', 'arpeggios', 'line-spanners', 'brackets', 'metronome', 'bar-numbers-staves', 'broken-spanners', 'titles', 'page-markers']
  .map((id) => ({ id, file: `tools/compare/fixtures/${id}.ly`, path: path.join(here, 'fixtures', `${id}.ly`) }))

/** The rows --parse takes for `ids`: the corpus's, syntax.ly's and the init's. */
function parseRows(ids, dir) {
  const syntax = { id: 'syntax', file: 'packages/common/test/grammar/syntax.ly', path: SYNTAX }
  const rows = [...corpus(dir), syntax, { id: 'init', file: 'declarations-init.ly', init: true }]
  if (!ids?.length) return rows.filter((row) => /^T\d\d$/.test(row.id) || row.id === 'syntax' || row.init)
  return rows.filter((row) => ids.includes(row.id))
}

/** Runs `run` on each of `rows`, a few at a time; resolves to the results, by ID. */
async function each(rows, run) {
  const results = []
  let next = 0
  const worker = async () => {
    while (next < rows.length) results.push(await run(rows[next++]))
  }
  await Promise.all(Array.from({ length: Math.min(rows.length, Math.max(1, availableParallelism() >> 1)) }, worker))
  return results.sort((a, b) => a.id.localeCompare(b.id))
}

/**
 * Captures the scores `ids` (default all) into `out`/<ID>, a few at a time;
 * resolves to each one's result. With `parse`, the parser's references.
 */
export async function capture({ ids, out, settings = CAPTURE, corpus: dir, parse = false, lilypond = process.env.LILYPOND ?? 'lilypond' } = {}) {
  const version = lilypondVersion(lilypond)
  if (version !== VERSION) throw new Error(`the reference is LilyPond ${VERSION}; ${lilypond} is ${version ?? 'not installed'}`)
  const rows = parse ? parseRows(ids, dir) : [...corpus(dir), ...[GROB_CALLS_ROW, ...LAYOUT_FIXTURES].filter((row) => ids?.includes(row.id))]
    .filter((row) => !ids?.length || ids.includes(row.id))
  const unknown = (ids ?? []).filter((id) => !rows.some((row) => row.id === id))
  if (unknown.length) throw new Error(`not in corpus.tsv: ${unknown.join(', ')}`)
  const missing = rows.filter((row) => !row.init && !existsSync(row.path))
  if (missing.length) throw new Error(`${missing.map((row) => row.id).join(', ')} not fetched; run tools/corpus/fetch.mjs`)
  out = path.resolve(out ?? path.join(corpusDir(dir), parse ? 'parse' : 'ref'))
  if (parse) {
    return each(rows, (row) => row.init ? captureInit(lilypond, path.join(out, 'init')) : compileParse(lilypond, row, path.join(out, row.id)))
  }
  settings = path.resolve(settings)
  return each(rows, (row) => captureOne(lilypond, row, path.join(out, row.id), settings, { CAPTURE_CWD: linkDir(row.id, dir) }))
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const options = { ids: [] }
  const argv = process.argv.slice(2)
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i].match(/^--(out|settings|corpus)$/)?.[1]
    if (flag) options[flag] = argv[++i]
    else if (argv[i] === '--parse') options.parse = true
    else options.ids.push(argv[i])
  }
  try {
    const results = await capture(options)
    if (options.parse) {
      console.log('ID\texit\tms\ttokens\tscores\tfile')
      for (const r of results) console.log(`${r.id}\t${r.exit}\t${r.ms}\t${r.tokens}\t${r.scores}\t${r.file}`)
      process.exit(0)
    }
    console.log('ID\texit\tms\tpages\tmidi\ttiming\tshapes\tstencils\tmarkups\tgrobs\tfile')
    const yes = (b) => b ? 'yes' : 'no'
    for (const r of results) {
      console.log(`${r.id}\t${r.exit}\t${r.ms}\t${r.pages}\t${r.midi}\t${yes(r.timing)}\t${yes(r.shapes)}\t${r.stencils}\t${r.markups}\t${r.grobs}\t${r.file}`)
    }
  } catch (error) {
    console.error(error.message)
    process.exit(1)
  }
}
