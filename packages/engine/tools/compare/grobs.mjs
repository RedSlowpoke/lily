// The references of part 2 of MS3 (progress.md, "How part 2's gate reads";
// MS3 step 12): reads the grobs.tsv that grobs.ly writes into a capture's
// directory, one grob a line in the order the Score hears them and a
// System line ending each translation,
//
//   type  cause  link  columns  context  engraver
//
// and compares two of them per score (the engine's with LilyPond's, from
// step 12's traced compile on). Translations are paired in order. Within
// one, the grobs of the compared types are aligned on their fields (the
// longest common subsequence), and each grob left over is one of four:
//
//   order    the same grob on the other side, elsewhere
//   differ   a grob at its place on the other side, of the same type or
//            differing in its type alone, one entry for each field
//   missing  the reference's, with nothing on the other side
//   extra    the other side's, with nothing in the reference
//
// Every type is compared unless asked for some: the steps of part 2 grew a
// list of the types compared as their engravers made them, until step 22
// made the 10's 34 types and dropped it. The columns are compared too from
// step 15 on (COLUMNS).
//
//   node tools/compare/grobs.mjs [--no-columns] <reference> <other>
//       each a grobs.tsv, a capture's directory, or a directory holding one
//       capture per ID (then every ID of the reference); prints each
//       score's differences as a JSON line and exits 1 if there are any.
//       --no-columns compares all but the columns
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { GROBS } from './capture.mjs'

/** A line's fields, in order. */
export const FIELDS = ['type', 'cause', 'link', 'columns', 'context', 'engraver']
/** Whether the columns' moments are compared: from step 15 on. */
export const COLUMNS = true

/** The translations of a grobs.tsv's `text`: each a list of grobs ({ line, type, cause, … }), the last its System. */
export function readGrobs(text) {
  const translations = []
  let current = []
  for (const [i, line] of text.split('\n').entries()) {
    if (!line) continue
    const values = line.split('\t')
    if (values.length !== FIELDS.length) throw new Error(`line ${i + 1} has ${values.length} fields, not ${FIELDS.length}: ${line}`)
    const grob = { line: i + 1, ...Object.fromEntries(FIELDS.map((f, j) => [f, values[j]])) }
    current.push(grob)
    if (grob.type === 'System') {
      translations.push(current)
      current = []
    }
  }
  if (current.length) translations.push(current)
  return translations
}

/** The number of grobs of each type in `translations`, the Systems among them. */
export function countTypes(translations) {
  const counts = new Map()
  for (const grob of translations.flat()) counts.set(grob.type, (counts.get(grob.type) ?? 0) + 1)
  return counts
}

/** The pairs [i, j] of a longest common subsequence of `a` and `b`, in order. */
export function common(a, b) {
  let start = 0
  while (start < a.length && start < b.length && a[start] === b[start]) start++
  let end = 0
  while (end < a.length - start && end < b.length - start && a[a.length - 1 - end] === b[b.length - 1 - end]) end++
  const pairs = Array.from({ length: start }, (_, i) => [i, i])
  const [n, m] = [a.length - start - end, b.length - start - end]
  // lengths[i * (m + 1) + j]: the subsequence's length of a's and b's middles from i and j on.
  const lengths = new Uint32Array((n + 1) * (m + 1))
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lengths[i * (m + 1) + j] = a[start + i] === b[start + j]
        ? lengths[(i + 1) * (m + 1) + j + 1] + 1
        : Math.max(lengths[(i + 1) * (m + 1) + j], lengths[i * (m + 1) + j + 1])
    }
  }
  for (let i = 0, j = 0; i < n && j < m;) {
    if (a[start + i] === b[start + j]) pairs.push([start + i++, start + j++])
    else if (lengths[(i + 1) * (m + 1) + j] >= lengths[i * (m + 1) + j + 1]) i++
    else j++
  }
  for (let k = end; k > 0; k--) pairs.push([a.length - k, b.length - k])
  return pairs
}

/**
 * The differences of `got` from `reference`, two grobs.tsv texts: { equal,
 * counts: [reference's, got's] grobs compared, missing, extra, order,
 * differ }, on `types` if given, else every type. Each entry names its
 * translation (from 1) and the lines of both sides; a `differ` entry its
 * field and both values.
 */
export function compareGrobs(reference, got, { types = null, columns = COLUMNS } = {}) {
  const fields = FIELDS.filter((f) => columns || f !== 'columns')
  const key = (grob) => fields.map((f) => grob[f]).join('\t')
  const keep = types ? new Set(types) : undefined
  const pick = (text) => readGrobs(text).map((t) => keep ? t.filter((g) => keep.has(g.type)) : t)
  const [a, b] = [pick(reference), pick(got)]
  const result = { equal: true, counts: [a.flat().length, b.flat().length], missing: [], extra: [], order: [], differ: [] }
  for (let t = 0; t < Math.max(a.length, b.length); t++) {
    const [x, y] = [a[t] ?? [], b[t] ?? []]
    const [kx, ky] = [x.map(key), y.map(key)]
    const pairs = common(kx, ky)
    // Each grob's gap, the number of aligned pairs before it, or -1 if it is aligned.
    const gaps = (length, side) => {
      let p = 0
      return Array.from({ length }, (_, i) => {
        while (p < pairs.length && pairs[p][side] < i) p++
        return p < pairs.length && pairs[p][side] === i ? -1 : p
      })
    }
    const [gx, gy] = [gaps(x.length, 0), gaps(y.length, 1)]
    const left = new Set(y.keys().filter((j) => gy[j] >= 0))
    const translation = t + 1
    const unpaired = []
    for (const i of x.keys().filter((i) => gx[i] >= 0)) {
      const j = [...left].find((j) => ky[j] === kx[i])
      if (j === undefined) {
        unpaired.push(i)
        continue
      }
      left.delete(j)
      result.order.push({ translation, line: x[i].line, got: y[j].line, grob: kx[i] })
    }
    for (const i of unpaired) {
      const differs = (j) => fields.filter((f) => x[i][f] !== y[j][f])
      const j = [...left].find((j) => gy[j] === gx[i] && (y[j].type === x[i].type || differs(j).join() === 'type'))
      if (j === undefined) {
        result.missing.push({ translation, line: x[i].line, grob: kx[i] })
        continue
      }
      left.delete(j)
      for (const field of differs(j)) {
        result.differ.push({ translation, line: x[i].line, got: y[j].line, field, reference: x[i][field], value: y[j][field] })
      }
    }
    for (const j of left) result.extra.push({ translation, line: y[j].line, grob: ky[j] })
  }
  result.equal = !['missing', 'extra', 'order', 'differ'].some((k) => result[k].length)
  return result
}

/** The grobs.tsv of `file`: itself, or a capture directory's. */
const grobsOf = (file) => statSync(file).isDirectory() ? path.join(file, GROBS) : file
const ID = /^[A-Z]\d\d$/

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2)
  const options = {}
  if (args[0] === '--no-columns') {
    args.shift()
    options.columns = false
  }
  if (args.length !== 2 || args.some((a) => a.startsWith('--'))) {
    console.error('usage: node tools/compare/grobs.mjs [--no-columns] <reference> <other>')
    process.exit(2)
  }
  const [reference, other] = args
  const ids = statSync(reference).isDirectory() && !existsSync(path.join(reference, GROBS))
    ? readdirSync(reference).filter((f) => ID.test(f) && existsSync(path.join(reference, f, GROBS))).sort()
    : [undefined]
  let equal = true
  for (const id of ids) {
    const [r, o] = id ? [path.join(reference, id, GROBS), path.join(other, id, GROBS)] : [grobsOf(reference), grobsOf(other)]
    const result = existsSync(o)
      ? compareGrobs(readFileSync(r, 'utf8'), readFileSync(o, 'utf8'), options)
      : { equal: false, absent: o }
    equal &&= result.equal
    console.log(JSON.stringify(id ? { id, ...result } : result))
  }
  process.exit(equal ? 0 : 1)
}
