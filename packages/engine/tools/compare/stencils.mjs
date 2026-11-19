// The references of W3 and text (06 §3.3, MS3 step 6): reads the pages.scm
// and markups.scm that stencils.ly writes into a capture's directory, one
// datum per line as Guile's `write` writes it, and compares two such
// captures record by record (the engine's with LilyPond's, from step 7 on):
//
//   (page <file> <number> <X extent> <Y extent> <expression>)
//   (markup <number> <source> <markup> <property chain> <X extent> <Y extent> <expression>)
//
// A difference is of a kind: `count` (the records), `input` (a page's file
// and number, a markup's number, source, markup or chain, compared as
// written), `extent`, `link` (the expression's grob-cause links, in order)
// or `expression` (the rest of the expression). Numbers compare as written,
// or with `digits` rounded to that many decimals, as the writer prints them
// (06 §6.2: four).
//
//   node tools/compare/stencils.mjs [--digits n] <reference dir> <dir>
//       prints the differences as JSON; exits 1 if there are any
import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { MARKUPS, PAGES } from './capture.mjs'

/** A node of a datum: `list` (items, and `tail` if dotted), `vector`, `number`, `string`, `symbol` or `other` (#t, a character, a keyword). */
const DELIMITER = /[\s()";]/
const NUMBER = /^[+-]?(\d+(\.\d*)?|\.\d+)(e[+-]?\d+)?$|^[+-]?\d+\/\d+$|^[+-](inf|nan)\.0$/

/** Reads one datum of Guile's `write` from `text` at `at`: [node, next]. */
function readDatum(text, at) {
  const skip = () => {
    while (at < text.length && /\s/.test(text[at])) at++
  }
  const datum = () => {
    skip()
    const c = text[at]
    if (c === undefined) throw new Error('the datum ends early')
    if (c === '(' || (c === '#' && text[at + 1] === '(')) {
      const vector = c === '#'
      at += vector ? 2 : 1
      const items = []
      let tail
      for (;;) {
        skip()
        if (text[at] === ')') {
          at++
          break
        }
        if (!vector && text[at] === '.' && DELIMITER.test(text[at + 1] ?? ' ')) {
          at++
          tail = datum()
          skip()
          if (text[at++] !== ')') throw new Error(`a dotted list goes on at ${at - 1}`)
          break
        }
        items.push(datum())
      }
      return vector ? { type: 'vector', items } : { type: 'list', items, tail }
    }
    if (c === ')') throw new Error(`an unopened ) at ${at}`)
    if (c === '"') {
      let value = ''
      for (at++; text[at] !== '"'; at++) {
        if (at >= text.length) throw new Error('a string ends early')
        if (text[at] !== '\\') {
          value += text[at]
          continue
        }
        const e = text[++at]
        if (e === 'x') {
          const end = text.indexOf(';', at)
          value += String.fromCodePoint(parseInt(text.slice(at + 1, end), 16))
          at = end
        } else value += { n: '\n', t: '\t', r: '\r', a: '\x07', f: '\f', v: '\v', 0: '\0' }[e] ?? e
      }
      at++
      return { type: 'string', value }
    }
    const start = at
    if (c === '#' && text[at + 1] === '\\') at += 3
    while (at < text.length && !DELIMITER.test(text[at])) at++
    const token = text.slice(start, at)
    if (token.startsWith('#')) return { type: 'other', text: token }
    if (NUMBER.test(token)) return { type: 'number', text: token }
    return { type: 'symbol', name: token }
  }
  const node = datum()
  return [node, at]
}

/** The datum `text` holds, alone. */
export function readSexp(text) {
  const [node, at] = readDatum(text, 0)
  if (text.slice(at).trim()) throw new Error(`more after the datum at ${at}`)
  return node
}

/** `node` written as Guile writes it. */
export function writeSexp(node) {
  switch (node.type) {
    case 'list': return `(${node.items.map(writeSexp).join(' ')}${node.tail ? ` . ${writeSexp(node.tail)}` : ''})`
    case 'vector': return `#(${node.items.map(writeSexp).join(' ')})`
    case 'number': return node.text
    case 'string': return JSON.stringify(node.value).replace(/\\u00([0-9a-f]{2})/g, '\\x$1;')
    case 'symbol': return node.name
    default: return node.text
  }
}

/** A number's value: an integer, a real, a ratio, ±inf.0 or +nan.0. */
export function numberValue(text) {
  if (/^[+-]inf\.0$/.test(text)) return text[0] === '-' ? -Infinity : Infinity
  if (/^[+-]nan\.0$/.test(text)) return NaN
  const ratio = text.match(/^([+-]?\d+)\/(\d+)$/)
  return ratio ? Number(ratio[1]) / Number(ratio[2]) : Number(text)
}

/** The records of a pages.scm's or markups.scm's text. */
export function readRecords(text) {
  return text.split('\n').filter((line) => line.trim()).map((line, i) => {
    const node = readSexp(line)
    const head = node.type === 'list' && node.items[0]?.name
    const at = { page: 3, markup: 5 }[head]
    if (!at || node.items.length !== at + 3 || node.tail) throw new Error(`record ${i + 1} is no page or markup: ${line.slice(0, 80)}`)
    const [x, y, expr] = node.items.slice(at)
    return { kind: head, label: `${head} ${writeSexp(node.items[1])}${head === 'page' ? '' : ` (${writeSexp(node.items[2].items?.[0] ?? node.items[2])})`}`,
      input: node.items.slice(1, at), x, y, expr }
  })
}

/** The records of the capture directory `dir`: its pages and its markups. */
export function readCapture(dir) {
  const read = (file) => existsSync(path.join(dir, file)) ? readRecords(readFileSync(path.join(dir, file), 'utf8')) : []
  return { pages: read(PAGES), markups: read(MARKUPS) }
}

/** Whether the numbers written `a` and `b` are equal, as written or to `digits` decimals. */
function sameNumber(a, b, digits) {
  if (digits === undefined) return a === b
  const [x, y] = [numberValue(a), numberValue(b)]
  if (!Number.isFinite(x) || !Number.isFinite(y)) return Object.is(x, y)
  const fixed = (v) => v.toFixed(digits).replace(/^-(0\.?0*)$/, '$1')
  return fixed(x) === fixed(y)
}

/** The grob-cause links of an expression, in order, and the expression without them. */
function links(node, out = []) {
  if (node.type !== 'list') return [out, node]
  if (node.items[0]?.name === 'grob-cause' && node.items.length === 3) {
    out.push(writeSexp(node.items[1]))
    const [, inner] = links(node.items[2], out)
    return [out, { ...node, items: [node.items[0], { type: 'other', text: '<link>' }, inner] }]
  }
  const items = node.items.map((item) => links(item, out)[1])
  return [out, { ...node, items, tail: node.tail && links(node.tail, out)[1] }]
}

/** The first place `a` and `b` differ, as [path, a's, b's], or undefined. */
function firstDifference(a, b, digits, at = '') {
  if (a.type !== b.type) return [at, writeSexp(a), writeSexp(b)]
  switch (a.type) {
    case 'list':
    case 'vector': {
      if (a.items.length !== b.items.length || !a.tail !== !b.tail) return [at, writeSexp(a), writeSexp(b)]
      for (let i = 0; i < a.items.length; i++) {
        const d = firstDifference(a.items[i], b.items[i], digits, `${at}/${i}`)
        if (d) return d
      }
      return a.tail ? firstDifference(a.tail, b.tail, digits, `${at}/.`) : undefined
    }
    case 'number': return sameNumber(a.text, b.text, digits) ? undefined : [at, a.text, b.text]
    default: return writeSexp(a) === writeSexp(b) ? undefined : [at, writeSexp(a), writeSexp(b)]
  }
}

/**
 * The differences of the records `got` from `reference` (readRecords'), each
 * `{ record, kind, path, reference, got }`, in order; numbers as written or,
 * with `digits`, to that many decimals.
 */
export function compareRecords(reference, got, { digits } = {}) {
  const differences = []
  const add = (record, kind, [at, a, b]) => differences.push({ record, kind, path: at, reference: a, got: b })
  if (reference.length !== got.length) add('', 'count', ['', String(reference.length), String(got.length)])
  for (let i = 0; i < Math.min(reference.length, got.length); i++) {
    const [r, g] = [reference[i], got[i]]
    const input = firstDifference({ type: 'list', items: r.input }, { type: 'list', items: g.input }, undefined)
    if (input) add(r.label, 'input', input)
    for (const axis of ['x', 'y']) {
      const d = firstDifference(r[axis], g[axis], digits, axis.toUpperCase())
      if (d) add(r.label, 'extent', d)
    }
    const [rl, re] = links(r.expr)
    const [gl, ge] = links(g.expr)
    if (rl.length !== gl.length) add(r.label, 'link', ['', `${rl.length} links`, `${gl.length} links`])
    for (let j = 0; j < Math.min(rl.length, gl.length); j++) {
      if (rl[j] !== gl[j]) add(r.label, 'link', [`link ${j + 1}`, rl[j], gl[j]])
    }
    const d = firstDifference(re, ge, digits)
    if (d) add(r.label, 'expression', d)
  }
  return differences
}

/** The differences of the capture `dir` from the capture `reference` (both directories): its pages', then its markups'. */
export function compareCaptures(reference, dir, options) {
  const [a, b] = [readCapture(reference), readCapture(dir)]
  return [...compareRecords(a.pages, b.pages, options), ...compareRecords(a.markups, b.markups, options)]
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2)
  const options = {}
  if (argv[0] === '--digits') {
    options.digits = Number(argv[1])
    argv.splice(0, 2)
  }
  if (argv.length !== 2) {
    console.error('usage: node tools/compare/stencils.mjs [--digits n] <reference dir> <dir>')
    process.exit(2)
  }
  const differences = compareCaptures(argv[0], argv[1], options)
  console.log(JSON.stringify(differences, null, 1))
  process.exit(differences.length ? 1 : 0)
}
