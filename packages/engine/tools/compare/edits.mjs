// The edits of MS6's differential harness (README §5, MS6; the operator's
// answer 2 at R7's split): chapter 00 §6.2's E1–E3 on T02, T12, T24 and
// T29, and seeded typed edits of the 30, each a span of a score's own file
// replaced, found from LilyPond's capture of the score ($LILY_CORPUS/ref/<id>/)
// as $LILY_CORPUS/ms6-split/edits.mjs finds them.
//
//   node tools/compare/edits.mjs [--seed <n>] [--count <k>] --out <dir> <id>…
//                          writes each edit's source as <dir>/<id>-<name>.ly
//                          and lists the edits, one JSON line each
//
// E1 and E2 start from the middle event of the capture's first performance
// in time order (`<name>.timing.json`, stable): where its token has no note
// name (a duration alone, T24's `16`), the next event that has one. E1
// raises the note name a step (c→d … b→c), E2 inserts the note's whole token
// again before it, E3 appends a character to the first string of the first
// \header, or else of the first \markup.
//
// A typed edit is one of KINDS at a place drawn from the score's population:
// the event links of grobs.tsv into the score's own file whose token starts
// with a note name or a duration, and the strings of the file. `pitch` raises
// a note name a step; `duration` changes a note's or a duration's number (or
// gives a note one); `insert` puts a note's token again before it; `delete`
// removes a note (its name, octave marks, duration and dots); `string` puts a
// letter into a string; `bar` deletes from a note to the next bar check or
// the end of its line; `error` puts an undefined command before a note, or
// deletes the last `}` of the file. The draws are mulberry32's from `seed`
// and the score's ID, so an edit is found again from its seed.
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { corpus, corpusDir } from '../corpus/fetch.mjs'

/** The four scores chapter 00 §6.2 times B6 on. */
export const EDITED = ['T02', 'T12', 'T24', 'T29']
/** The 30 scores. */
export const THIRTY = Array.from({ length: 30 }, (_, i) => `T${String(i + 1).padStart(2, '0')}`)
/** The typed edits' kinds, in the order a score's draws cycle through them. */
export const KINDS = ['pitch', 'duration', 'insert', 'delete', 'string', 'bar', 'error']

/** A note name in any of LilyPond's languages' Dutch forms, at the start of a token. */
const NOTE = /^[a-g](?:isis|eses|ih|eh|is|es|s)*(?![a-z])/
/** A note's word: its name, octave marks, accidental marks, duration, dots and multiplier. */
const WORD = /^[a-g](?:isis|eses|ih|eh|is|es|s)*[',]*[!?]?(?:\d+\.*(?:\*\d+(?:\/\d+)?)?)?/
const DURATION = /^\d+/
const STRING = /"(?:[^"\\]|\\.)*"/g
/** The next note name up a step. */
const UP = { c: 'd', d: 'e', e: 'f', f: 'g', g: 'a', a: 'b', b: 'c' }
const LONGER = { 1: '2', 2: '4', 4: '8', 8: '16', 16: '8', 32: '16', 64: '32', 128: '64' }

const rows = () => Object.fromEntries(corpus().map((row) => [row.id, row]))
/** The capture of `id`. */
const refDir = (id) => path.join(corpusDir(), 'ref', id)

/** A link's file, line and char (0-based), its directory resolved against `root`'s. */
function link(href, root) {
  const m = /^(?:textedit:\/\/)?(.*):(\d+):(\d+):(\d+)$/.exec(href)
  if (!m) return null
  return { file: path.resolve(path.dirname(root), m[1]), line: +m[2], char: +m[3] }
}

/** The offset in `text` of line `line` (1-based) and char `char`. */
function offsetOf(text, line, char) {
  let at = 0
  for (let l = 1; l < line; l++) {
    at = text.indexOf('\n', at) + 1
    if (at === 0) return -1
  }
  return at + char
}

/** The token (up to the next space) at `at`, and where its note's name starts (after a chord's `<`). */
function tokenAt(text, at) {
  const token = /^\S*/.exec(text.slice(at))[0]
  const name = token.startsWith('<') ? at + 1 : at
  return { token, name }
}

/** An edit of `text`: [from, to) replaced by `insert`. */
const edit = (text, from, to, insert) => ({ from, to, insert, source: text.slice(0, from) + insert + text.slice(to) })

/** E1 at the note name at `at`: its first letter up a step. */
const raise = (text, at) => edit(text, at, at + 1, UP[text[at]])

/** E1–E3 on `id` (one of EDITED): each `{ id, name, file, line, char, from, to, insert, source }`. */
export function e1e3(id) {
  const { path: file } = rows()[id]
  const text = readFileSync(file, 'utf8')
  const dir = refDir(id)
  const timing = readdirSync(dir).find((f) => f.endsWith('.timing.json'))
  const events = JSON.parse(readFileSync(path.join(dir, timing), 'utf8'))[0].events
    .map((e, i) => ({ ...e, i }))
    .sort((a, b) => a.at - b.at || a.i - b.i)
  let note
  for (let k = Math.floor(events.length / 2); k < events.length && !note; k++) {
    const at = link(events[k].href, file)
    if (at?.file !== file) continue
    const offset = offsetOf(text, at.line, at.char)
    const { name } = tokenAt(text, offset)
    if (NOTE.test(text.slice(name))) note = { ...at, offset, name }
  }
  if (!note) throw new Error(`${id}: no event with a note name from the middle of its first performance`)
  const token = tokenAt(text, note.offset).token
  const place = { id, file, line: note.line, char: note.char }
  // E3: the first string of the first \header, else of the first \markup.
  let e3
  for (const block of ['\\header', '\\markup']) {
    const start = text.indexOf(block)
    const string = new RegExp(STRING.source, 'g')
    string.lastIndex = start
    const m = start < 0 ? null : string.exec(text)
    if (!m) continue
    const end = m.index + m[0].length - 1
    e3 = { id, file, line: text.slice(0, end).split('\n').length, char: end - text.lastIndexOf('\n', end - 1) - 1, ...edit(text, end, end, 'x') }
    break
  }
  if (!e3) throw new Error(`${id}: no string in a \\header or \\markup`)
  return [
    { ...place, name: 'E1', ...raise(text, note.name) },
    { ...place, name: 'E2', ...edit(text, note.offset, note.offset, `${token} `) },
    { ...e3, name: 'E3' },
  ]
}

/** mulberry32: a generator of numbers in [0, 1) from a 32-bit seed. */
function mulberry32(seed) {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

/** `id`'s population: the offsets of its notes and durations, and its strings' spans, in its own file. */
export function population(id) {
  const { path: file } = rows()[id]
  const text = readFileSync(file, 'utf8')
  const notes = new Set()
  const durations = new Set()
  for (const line of readFileSync(path.join(refDir(id), 'grobs.tsv'), 'utf8').split('\n')) {
    const at = link(line.split('\t')[2] ?? '', file)
    if (at?.file !== file) continue
    const offset = offsetOf(text, at.line, at.char)
    if (offset < 0) continue
    const { name } = tokenAt(text, offset)
    if (NOTE.test(text.slice(name))) notes.add(name)
    else if (DURATION.test(text.slice(offset))) durations.add(offset)
  }
  const strings = [...text.matchAll(STRING)].map((m) => [m.index, m.index + m[0].length])
  const sort = (set) => [...set].sort((a, b) => a - b)
  return { file, text, notes: sort(notes), durations: sort(durations), strings }
}

/** The steps of `typing`, in order: each changes the text the step before left. */
export const TYPED = ['pitch', 'pitch', 'again', 'insert', 'pitch', 'undo', 'string', 'error', 'undo', 'pitch']

/**
 * A person typing (MS7 step 1, D258): `count` edits of `id`, each of the text
 * the one before left, five at each of three places in turn: a note in the
 * middle, one early and one late. Each is a step of TYPED: a note raised, the
 * same text again, a note's token typed again before it, the last step undone,
 * a character in the first string of the header or the first markup, an
 * undefined command. It is what N8's snapshots near an edit are for, and what
 * seeded edits, each from the score's own text, do not reach.
 */
export function typing(id, { count = 12 } = {}) {
  const { file, text: first, notes } = population(id)
  if (!notes.length) return []
  let text = first
  let places = [0.5, 0.2, 0.8].map((f) => notes[Math.floor(notes.length * f)])
  const history = []
  const edits = []
  for (let k = 0; k < count; k++) {
    const kind = TYPED[k % TYPED.length]
    const place = Math.floor(k / 5) % places.length
    const at = places[place]
    const string = kind === 'string' && headerString(text)
    const e = kind === 'pitch' ? raise(text, at)
      : kind === 'insert' ? edit(text, at, at, `${WORD.exec(text.slice(at))[0]} `)
        : kind === 'error' ? edit(text, at, at, '\\nosuchcommand ')
          : string ? edit(text, string, string, 'x') : null
    history.push({ text, places })
    if (e) {
      const moved = e.insert.length - (e.to - e.from)
      places = places.map((p) => (p > e.from ? p + moved : p))
      text = e.source
    } else if (kind === 'undo') {
      ({ text, places } = history[history.length - 2])
    }
    edits.push({ id, file, name: `type-${k}-${kind}`, kind, source: text })
  }
  return edits
}

/** Where the first string of the first \\header, else of the first \\markup, ends. */
function headerString(text) {
  for (const block of ['\\header', '\\markup']) {
    const start = text.indexOf(block)
    const string = new RegExp(STRING.source, 'g')
    string.lastIndex = start
    const m = start < 0 ? null : string.exec(text)
    if (m) return m.index + m[0].length - 1
  }
  return 0
}

/** A typed edit of `kind` with the draws of `random`, or undefined when the population has no place for it. */
function typed(kind, { text, notes, durations, strings }, random) {
  const pick = (list) => list[Math.floor(random() * list.length)]
  const at = notes.length ? pick(notes) : undefined
  const word = at === undefined ? '' : WORD.exec(text.slice(at))[0]
  switch (kind) {
    case 'pitch':
      return at === undefined ? undefined : raise(text, at)
    case 'duration': {
      const from = durations.length && (!notes.length || random() < 0.25) ? pick(durations) : at
      if (from === undefined) return undefined
      const digits = /\d+/.exec(text.slice(from, from + (from === at ? word.length : 8)))
      if (!digits) {
        // A note without a duration: given one, after its name and octave marks.
        const name = /^[a-g](?:isis|eses|ih|eh|is|es|s)*[',]*/.exec(text.slice(from))[0]
        return edit(text, from + name.length, from + name.length, pick(['2', '8', '16']))
      }
      const start = from + digits.index
      return edit(text, start, start + digits[0].length, LONGER[digits[0]] ?? '4')
    }
    case 'insert':
      return at === undefined ? undefined : edit(text, at, at, `${word} `)
    case 'delete':
      return at === undefined ? undefined : edit(text, at, at + word.length, '')
    case 'string': {
      if (!strings.length) return undefined
      const [from, to] = pick(strings)
      const into = from + 1 + Math.floor(random() * (to - from - 1))
      return edit(text, into, into, pick(['a', 'e', 'o', 'x', ' ']))
    }
    case 'bar': {
      if (at === undefined) return undefined
      const line = text.indexOf('\n', at)
      const bar = /(?<=\s)\|(?=\s)/.exec(text.slice(at, line < 0 ? undefined : line))
      return edit(text, at, bar ? at + bar.index + 1 : line < 0 ? text.length : line, '')
    }
    case 'error': {
      const brace = text.lastIndexOf('}')
      if (at !== undefined && (brace < 0 || random() < 0.5)) return edit(text, at, at, '\\noSuchMusic ')
      return brace < 0 ? undefined : edit(text, brace, brace + 1, '')
    }
  }
  throw new Error(`no edit of kind ${kind}`)
}

/** The seed of `id`'s draws from `seed`: the run's seed with the score's ID in it. */
const seedOf = (id, seed) => [...id].reduce((h, c) => Math.imul(h ^ c.charCodeAt(0), 16777619) >>> 0, (seed ^ 2166136261) >>> 0)

/**
 * `count` seeded typed edits of `id`, the kinds in KINDS' order from one the
 * seed draws, a kind with no place in the score skipped: each `{ id, name,
 * kind, seed, file, line, char, from, to, insert, source }`, `name` `<kind>-<seed>-<n>`.
 */
export function seeded(id, { seed = 1, count = 10 } = {}) {
  const pop = population(id)
  const random = mulberry32(seedOf(id, seed))
  const first = Math.floor(random() * KINDS.length)
  const edits = []
  for (let n = 0, tries = 0; edits.length < count && tries < count * KINDS.length; tries++, n++) {
    const kind = KINDS[(first + n) % KINDS.length]
    const e = typed(kind, pop, random)
    if (!e || e.source === pop.text) continue
    const before = pop.text.slice(0, e.from)
    const line = before.split('\n').length
    edits.push({ id, name: `${kind}-${seed}-${edits.length + 1}`, kind, seed, file: pop.file, line, char: e.from - before.lastIndexOf('\n') - 1, ...e })
  }
  return edits
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2)
  const option = (name, fallback) => {
    const i = args.indexOf(name)
    return i < 0 ? fallback : args.splice(i, 2)[1]
  }
  const seed = +option('--seed', 1)
  const count = +option('--count', 10)
  const out = option('--out')
  if (!out || !args.length) {
    console.error('usage: edits.mjs [--seed <n>] [--count <k>] --out <dir> <id>…')
    process.exit(2)
  }
  mkdirSync(out, { recursive: true })
  for (const id of args) {
    for (const e of [...(EDITED.includes(id) ? e1e3(id) : []), ...seeded(id, { seed, count })]) {
      writeFileSync(path.join(out, `${id}-${e.name}.ly`), e.source)
      console.log(JSON.stringify({ ...e, source: undefined }))
    }
  }
}
