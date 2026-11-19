// Fetches the test scores of corpus.tsv that come from LilyPond's repository
// (chapter 00 §4, chapter 07 §2.3): a shallow, sparse checkout of tag v2.26.0
// with input/regression and Documentation/ly-examples (added to what the
// checkout already has), outside this repository, since the scores are never
// copied into it. Then it checks that
// every score of corpus.tsv is there and writes <dir>/corpus.tsv with `ID path`
// and absolute paths, the form the chapters' recipes read.
//
//   node tools/corpus/fetch.mjs [dir]      or `npm run corpus` in packages/engine
//
//   dir, LILY_CORPUS   where the checkout goes (default ~/.cache/lily-engine):
//                      <dir>/lilypond-src, <dir>/corpus.tsv
//
// A checkout already there is reused if it is at the tag's commit.
import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { homedir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

export const TAG = 'v2.26.0'
export const COMMIT = '3596756be0fac1f81066a99d2054329212172b2d'
const REMOTE = 'https://gitlab.com/lilypond/lilypond.git'
const SPARSE = ['input/regression', 'Documentation/ly-examples']

const here = path.dirname(fileURLToPath(import.meta.url))
const repo = path.resolve(here, '../../../..')

export const corpusDir = (dir) => path.resolve(dir ?? process.env.LILY_CORPUS ?? path.join(homedir(), '.cache/lily-engine'))

/** corpus.tsv's rows, each with its absolute path under `dir`'s checkout or this repository. */
export function corpus(dir) {
  const src = path.join(corpusDir(dir), 'lilypond-src')
  return readFileSync(path.join(here, 'corpus.tsv'), 'utf8').split('\n')
    .filter((line) => line && !line.startsWith('#'))
    .map((line) => {
      const [id, source, file] = line.split('\t')
      if (source !== 'repo' && source !== 'lilypond') throw new Error(`corpus.tsv: ${id}: unknown source ${source}`)
      return { id, source, file, path: path.join(source === 'repo' ? repo : src, file) }
    })
}

function git(args, options = {}) {
  const run = spawnSync('git', args, { encoding: 'utf8', stdio: options.quiet ? 'pipe' : ['ignore', 'inherit', 'inherit'] })
  if (run.status !== 0) throw new Error(`git ${args.join(' ')} failed${run.stderr ? `: ${run.stderr.trim()}` : ''}`)
  return run.stdout?.trim()
}

export function fetch(dir) {
  const root = corpusDir(dir)
  const src = path.join(root, 'lilypond-src')
  if (src.startsWith(repo + path.sep)) throw new Error(`${src} is inside the repository; LilyPond's scores are never copied into it`)
  mkdirSync(root, { recursive: true })
  if (!existsSync(src)) {
    console.log(`Cloning ${REMOTE} at ${TAG} into ${src}`)
    git(['-c', 'advice.detachedHead=false', 'clone', '--depth', '1', '--branch', TAG, '--filter=blob:none', '--sparse', REMOTE, src])
  }
  const head = git(['-C', src, 'rev-parse', 'HEAD'], { quiet: true })
  if (head !== COMMIT) throw new Error(`${src} is at ${head}, not ${TAG} (${COMMIT}); remove it to fetch again`)
  // `add`, not `set`: a checkout widened by hand (the splits read its lily/ and flower/) keeps what it has.
  git(['-C', src, 'sparse-checkout', 'add', ...SPARSE])

  const rows = corpus(root)
  const missing = rows.filter((row) => !existsSync(row.path))
  if (missing.length) throw new Error(`missing from the corpus: ${missing.map((row) => `${row.id} ${row.path}`).join(', ')}`)
  writeFileSync(path.join(root, 'corpus.tsv'), rows.map((row) => `${row.id}\t${row.path}\n`).join(''))
  console.log(`${rows.length} scores; ${path.join(root, 'corpus.tsv')} lists them`)
  return rows
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    fetch(process.argv[2])
  } catch (error) {
    console.error(error.message)
    process.exit(1)
  }
}
