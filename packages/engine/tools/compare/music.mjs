// Music trees for MS2's steps (04 §6): music.ly's dump of each score's music
// as the book handler gets it, compared line by line with another dump of
// the same format (the engine's, from step 4 on). A difference is reported
// with the path to it: the score, the nodes and the properties above it.
//
//   node tools/compare/music.mjs <reference music.txt> <music.txt>
//                                          exits 1 if the trees differ
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

/** A dump's lines, each with its depth (two spaces a level) and its text. */
export function readMusic(text) {
  return text.split('\n').filter((line) => line !== '').map((line) => {
    const depth = line.match(/^ */)[0].length >> 1
    return { depth, text: line.slice(depth * 2) }
  })
}

/** What a line contributes to a path: a node's name and origin, a property's name. */
const label = (text) => text.replace(/^= /, '').replace(/ = .*$/s, '').replace(/:$/, '')

/** The path to `lines[at]`: its score, and the labels of the lines above it, one a level. */
function pathTo(lines, at) {
  let score = null
  const path = []
  for (let i = 0; i <= at && i < lines.length; i++) {
    if (lines[i].depth === 0 && /^score \d+$/.test(lines[i].text)) {
      score = lines[i].text
      path.length = 0
      if (i === at) break
      continue
    }
    path.length = lines[i].depth
    path.push(label(lines[i].text))
  }
  return [score, ...path].filter(Boolean).join(' › ')
}

/**
 * Compares the dump `got` with `reference`, both texts: whether they are
 * equal, their nodes, and the first line that differs, with its path.
 */
export function compareMusic(reference, got) {
  const [a, b] = [readMusic(reference), readMusic(got)]
  const nodes = (lines) => lines.filter((l) => /^(= )?[A-Z]\w* @/.test(l.text)).length
  let at = 0
  while (at < a.length && at < b.length && a[at].depth === b[at].depth && a[at].text === b[at].text) at++
  const equal = at === a.length && at === b.length
  const line = (l) => l ? '  '.repeat(l.depth) + l.text : null
  return {
    equal, nodes: [nodes(a), nodes(b)], lines: [a.length, b.length],
    ...(equal ? {} : { at, path: pathTo(a.length > at ? a : b, at), expected: line(a[at]), got: line(b[at]) }),
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const files = process.argv.slice(2)
  if (files.length !== 2) {
    console.error('usage: music.mjs <reference music.txt> <music.txt>')
    process.exit(2)
  }
  const result = compareMusic(...files.map((f) => readFileSync(f, 'utf8')))
  console.log(JSON.stringify(result))
  process.exit(result.equal ? 0 : 1)
}
