// The context definitions of $defaultlayout and $defaultmidi (N3, 04 §7):
// contexts.ly's dump of them, as LilyPond 2.26.0 writes it after its init
// and as the engine writes it after its own (crates/parser/examples/parse.rs,
// whose library ran declarations-init.ly), compared definition by definition. A difference is
// reported with its definition, the line LilyPond wrote and the engine's.
//
//   node tools/compare/contexts.mjs     prints the comparison as JSON;
//                                       exits 1 if the definitions differ
import { spawnSync } from 'node:child_process'
import { mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { runExample } from '../examples.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
const dump = path.join(here, 'contexts.ly')

/** What LilyPond writes for contexts.ly: its dump, on stdout, and its stderr. */
export function lilypondContexts(lilypond = process.env.LILYPOND ?? 'lilypond') {
  const out = mkdtempSync(path.join(tmpdir(), 'lily-contexts-'))
  try {
    const run = spawnSync(lilypond, ['--loglevel=WARNING', '-dno-print-pages', '-o', out, dump],
      { encoding: 'utf8', env: { ...process.env, LANGUAGE: 'en' } })
    return { status: run.status, stdout: run.stdout, stderr: run.stderr }
  } finally {
    rmSync(out, { recursive: true, force: true })
  }
}

/** What the engine writes for contexts.ly, after the init files' context definitions. */
export function engineContexts() {
  const out = mkdtempSync(path.join(tmpdir(), 'lily-contexts-'))
  try {
    const run = runExample('parse', ['--display', out, path.basename(dump)],
    { cwd: here, encoding: 'utf8' })
    let stdout = ''
    try {
      stdout = readFileSync(path.join(out, 'contexts.out'), 'utf8')
    } catch {}
    return { status: run.status, stdout, stderr: run.stderr }
  } finally {
    rmSync(out, { recursive: true, force: true })
  }
}

/** A dump's definitions: each head line (`layout Staff engraver-init.ly:59:1`) and the lines below it. */
export function readContexts(text) {
  const definitions = new Map()
  let lines = null
  for (const line of text.split('\n')) {
    if (line === '') continue
    if (!line.startsWith(' ')) definitions.set(line, (lines = []))
    else lines?.push(line.trim())
  }
  return definitions
}

/** A head line's kind and name, which name a definition in either dump. */
const nameOf = (head) => head.split(' ').slice(0, 2).join(' ')

/**
 * Compares the dump `got` with `reference`: whether they are equal, how
 * many definitions each has, and each difference: the definition, and the
 * line of each (its place in the head's), the first that differs in it.
 */
export function compareContexts(reference, got) {
  const [a, b] = [readContexts(reference), readContexts(got)]
  const byName = (defs) => new Map([...defs].map(([head, lines]) => [nameOf(head), [head, ...lines]]))
  const [want, have] = [byName(a), byName(b)]
  const differ = []
  for (const name of new Set([...want.keys(), ...have.keys()])) {
    const [x = [], y = []] = [want.get(name), have.get(name)]
    for (let at = 0; at < Math.max(x.length, y.length); at++) {
      if (x[at] !== y[at]) differ.push({ definition: name, expected: x[at] ?? null, got: y[at] ?? null })
    }
  }
  return { equal: differ.length === 0, definitions: [a.size, b.size], differ }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [reference, got] = [lilypondContexts(), engineContexts()]
  if (reference.status !== 0 || got.status !== 0) {
    console.error(reference.stderr, got.stderr)
    process.exit(2)
  }
  const result = compareContexts(reference.stdout, got.stdout)
  console.log(JSON.stringify(result, null, 2))
  process.exit(result.equal ? 0 : 1)
}
