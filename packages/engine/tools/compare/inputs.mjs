// The harness of p12.test.mjs and tail.test.mjs (MS8 steps 10 and 11): .ly files compiled natively through the core
// (crates/core/examples/compile.rs, one instance for them all) and by LilyPond 2.26.0, their stderr compared byte for
// byte, LilyPond's data directory written `<datadir>` as the engine names it. Each score is performed by LilyPond too
// (PERFORM, capture.ly's \midi), as the engine performs it for the playback map (D103): a translation's warning is
// then given twice by both.
import { spawnSync } from 'node:child_process'
import { copyFileSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { runExample } from '../examples.mjs'

/** capture.ly's \midi for every score, without its traces. */
export const PERFORM = `#(define (p12-score! score)
   (let ((defs (ly:score-output-defs score)))
     (unless (any (lambda (def) (eq? (ly:output-def-lookup def 'output-def-kind #f) 'midi)) defs)
       (when (null? defs) (ly:score-add-output-def! score (ly:output-def-clone $defaultlayout)))
       (ly:score-add-output-def! score (ly:output-def-clone $defaultmidi)))))
#(define (p12-book! book)
   (for-each (lambda (score) (when (ly:score? score) (p12-score! score))) (ly:book-scores book))
   (for-each p12-book! (ly:book-book-parts book)))
#(define p12-book-handler toplevel-book-handler)
#(define (toplevel-book-handler book) (p12-book! book) (p12-book-handler book))
`

/** LilyPond's data directory, which the engine names `<datadir>`. */
export function datadir() {
  return spawnSync('sh', ['-c', 'cd "$(dirname "$(realpath "$(command -v lilypond)")")/../share/lilypond/2.26.0" && pwd'],
    { encoding: 'utf8' }).stdout.trim()
}

/**
 * The inputs where the engine's stderr is not LilyPond's, each with both, in `scratch`: `inputs` are `[name, text]`
 * (the text after `\version "2.26.0"`) or `{ file }`, a .ly file copied in.
 */
export function stderrDiffering(inputs, scratch) {
  const dir = mkdtempSync(path.join(scratch, 'in-'))
  const files = inputs.map((input) => {
    if (input.file) {
      const name = path.basename(input.file)
      copyFileSync(input.file, path.join(dir, name))
      return name
    }
    const [name, text] = input
    writeFileSync(path.join(dir, `${name}.ly`), `\\version "2.26.0"\n${text}\n`)
    return `${name}.ly`
  })
  const out = mkdtempSync(path.join(scratch, 'engine-'))
  const run = runExample('compile', ['--out', out, ...files], { cwd: dir, encoding: 'utf8' })
  if (run.status !== 0 && run.status !== 1) throw new Error(`compile: ${run.status} ${run.stderr}`)
  const data = datadir()
  const perform = path.join(scratch, 'perform.ly')
  writeFileSync(perform, PERFORM)
  const failed = []
  for (const file of files) {
    const ran = spawnSync('lilypond', ['--loglevel=WARNING', '--svg', `-dinclude-settings=${perform}`, file], { cwd: dir, encoding: 'utf8',
      env: { ...process.env, LANGUAGE: 'en' } })
    const want = ran.stderr.split(data).join('<datadir>')
    const got = readFileSync(path.join(out, `${path.basename(file, '.ly')}.err`), 'utf8')
    if (got !== want) failed.push(`${file}:\n${readFileSync(path.join(dir, file), 'utf8')}\nengine:\n${got}\nlilypond:\n${want}`)
  }
  return failed
}
