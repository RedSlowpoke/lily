// The references of part 3 of MS3 (progress.md, "How part 3's gate reads";
// MS3 step 25): reads the layout.tsv that layout.ly writes into a capture's
// directory, one live grob a line after page breaking, each system's
// all-elements in their order,
//
//   type  cause  link  columns  context  engraver  system  page  x  y  x-extent  y-extent  stencil
//
// and compares two of them per score (the engine's with LilyPond's, from
// step 25's traced compile on). Since step 60 each system's own line comes
// before its grobs (OPEN H9): type System and grobs.tsv's System line's
// fields, so the systems pair in order as a broken grob's copies do; its
// index, its page, its place on its page (x and y), its extents and its
// stencil flag. The engine writes its place unreached, reading no page; a
// System is in no COMPARED until it does, and --all compares it. The six fields of grobs.tsv (grobs.mjs'
// FIELDS) pair a grob with the other side's: the nth grob of a key with the
// nth of the same key, so that the copies of a broken grob pair system by
// system. Each grob left over or moved is one of three:
//
//   missing  the reference's, with nothing on the other side
//   extra    the other side's, with nothing in the reference
//   order    paired, but off the longest common subsequence of the pairs
//
// and each paired grob's layout fields are compared, one `differ` entry a
// field: the system, the page and the stencil flag as written, the offsets
// and both ends of the extents to TOLERANCE (- for an empty extent, +inf
// and -inf as written).
//
// Only the types of COMPARED are compared, each on the layout fields it
// lists: the steps of part 3 grow it as they make a type's fields
// computable, as part 2's steps grew grobs.mjs' list of types. A field the
// engine writes as UNREACHED (its read did not complete: its layout reached
// a placeholder, PRIMS' misc-error by name, another error, or a stop for
// the host; D126) is not compared; it is counted, per field, over every
// line of the other side, whatever COMPARED holds.
//
// It compares the columns.tsv that layout.ly writes beside it too (MS3
// step 31, D128): a line a column after each SpacingSpanner's set-springs,
//
//   column  spacing-wishes  springs  rods
//
// paired by the column's name (type@columns@break), and compared as
// written but the springs' and rods' numbers, to TOLERANCE; the engine's
// UNREACHED fields counted and not compared, as layout.tsv's. The engine
// makes the prebroken pieces since step 32 (D129), so every line is
// compared.
//
// And the line.tsv beside them (MS3 step 32, D129): a line a column of each
// system's line, its X offset relative to the system,
//
//   system  column  x
//
// paired by the system and the column's name, the x compared to TOLERANCE;
// the engine's UNREACHED x counted and not compared.
//
//   node tools/compare/layout.mjs [--all] <reference> <other>
//       each a layout.tsv, a capture's directory, or a directory holding one
//       capture per ID (then every ID of the reference); prints each
//       score's differences as a JSON line and exits 1 if there are any.
//       --all compares every type on every field, COMPARED aside
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { LAYOUT } from './capture.mjs'
import { FIELDS as GROB_FIELDS, common } from './grobs.mjs'

/** The layout fields of a line, after grobs.tsv's six. */
export const PLACED = ['system', 'page', 'x', 'y', 'x-extent', 'y-extent', 'stencil']
/** A line's fields, in order. */
export const FIELDS = [...GROB_FIELDS, ...PLACED]
/** The value of a field whose read did not complete in the engine (D126). */
export const UNREACHED = '?'
/** How far apart two offsets or two ends of extents may be. */
export const TOLERANCE = 1e-4
/**
 * The types compared and the layout fields compared of each: none at step
 * 25, where every grob of the five is unreached. Each step of part 3 adds
 * the types and fields it makes computable. Step 28: the Y extents of the
 * staff symbol, the axis groups (their suicide too: a dead group and its
 * elements have no line) and the alignment (D126). The referencers'
 * (NoteHead, Stem, Clef, KeySignature) wait for breaking (step 33), with
 * their Y offsets: LilyPond lists them after the spanners, and the unbroken
 * clefs and key signatures in mid-line are dead. Step 30: BreakAlignment's,
 * BreakAlignGroup's and LeftEdge's X offsets wait for step 33 too (the
 * user's answer, step 30's session 7): x is relative to the System, so it
 * holds the columns' positions, which spacing and breaking give, and
 * LilyPond's lines at the first and last columns are the broken pieces,
 * the engine's the unbroken originals. Step 33: the same three types' X
 * and Y offsets, relative to their broken system (D126), which the engine
 * reads once it has broken the System into its lines; unreached where it
 * has not. The other types wait for the five to be broken: until then the
 * engine's lines are the unbroken System's, its breakable items' pieces
 * more than LilyPond's and the referencers in another order. Where the
 * engine breaks the System (fixtures/path.ly), every type is LilyPond's.
 * Step 37: the note heads', rests', dots', dot columns' and note columns'
 * offsets and Y extents, and the paper columns' X offsets, which the five
 * reach once they are broken (step 40): until then their offsets are
 * unreached, the note columns' Y extents too (their stems, step 38), and
 * the unbroken System's columns and pieces are more than LilyPond's.
 * Step 38: the stems' and the stem stubs' offsets and Y extents; the note
 * columns' Y extents are read from here on. Step 39: the accidentals' and
 * the accidental placements' offsets and Y extents (T19's), the
 * placements' X offsets reached when the five are broken (step 40); and
 * the ledger line spanners' (one a staff, its Y extent none, `#f`, as
 * LilyPond's), their offsets reached when the five are broken. Step 40:
 * the clefs' and the key signatures' offsets and Y extents, which T14 and
 * T16, broken into their lines from then on, give as LilyPond's (no live
 * key signature is among the five's); the unbroken three's unbroken clef
 * and key signatures, read before breaking, wait for it. And the system
 * start bars' and bar numbers' offsets and Y extents, with which all five
 * are broken (ly:system-start-delimiter::print, a BarNumber's X-offset):
 * none is live among the five's, each killed as LilyPond's;
 * fixtures/bar-number.ly's bar numbers are. Step 41: the flags' offsets
 * and Y extents (none among the five's): fixtures/y1-unreached.ly's three
 * give their Y extents as LilyPond's, their offsets unreached until it is
 * broken (its collisions, multi-measure rests and span bars, steps 41–43).
 * And the note collisions' (none among the five's): y1-unreached's three
 * give their Y extents as LilyPond's, fixtures/collisions.ly's their
 * offsets too. And the rest collisions' (none among the five's), which
 * have no extent: y1-unreached's one unreached until it is broken,
 * fixtures/rest-collisions.ly's at LilyPond's offsets, with its rests.
 * Step 42: the multi-measure rests' and their numbers', texts' and
 * scripts' (none among the five's): y1-unreached's eight live ones give
 * their Y extents as LilyPond's, their offsets unreached until it is
 * broken. And the clef modifiers' (none among the five's):
 * fixtures/clef-modifiers.ly's at LilyPond's offsets and Y extents. And
 * the span bars' and their stubs' (none among the five's), with the bar
 * lines' they widen (OPEN A11): fixtures/span-bars.ly's at LilyPond's
 * offsets and Y extents, y1-unreached's bar lines and span bars too.
 * y1-unreached, broken since step 42 (session 4), has its flags',
 * collisions' and multi-measure rests' offsets reached too, LilyPond's
 * but below the absent Lyrics' line (A10); compared on every type, its
 * unreached grobs were the InstrumentNames (step 43) and the Lyrics'.
 * Step 43: the instrument names' (none among the five's):
 * fixtures/instrument-names.ly's at LilyPond's offsets and Y extents;
 * y1-unreached's two at LilyPond's Y offsets (the second's below the
 * Lyrics' line) and Y extents, their X offsets unreached (the brace's
 * extent, whose font no preloaded pack holds), and its unreached grobs
 * the Lyrics' alone.
 */
export const COMPARED = {
  StaffSymbol: ['x', 'y', 'y-extent'],
  VerticalAxisGroup: ['x', 'y', 'y-extent'],
  VerticalAlignment: ['x', 'y', 'y-extent'],
  NoteHead: ['x', 'y', 'y-extent'],
  Rest: ['x', 'y', 'y-extent'],
  Dots: ['x', 'y', 'y-extent'],
  DotColumn: ['x', 'y', 'y-extent'],
  NoteColumn: ['x', 'y', 'y-extent'],
  Stem: ['x', 'y', 'y-extent'],
  StemStub: ['x', 'y', 'y-extent'],
  Accidental: ['x', 'y', 'y-extent'],
  AccidentalPlacement: ['x', 'y', 'y-extent'],
  LedgerLineSpanner: ['x', 'y', 'y-extent'],
  Clef: ['x', 'y', 'y-extent'],
  KeySignature: ['x', 'y', 'y-extent'],
  SystemStartBar: ['x', 'y', 'y-extent'],
  BarNumber: ['x', 'y', 'y-extent'],
  Flag: ['x', 'y', 'y-extent'],
  NoteCollision: ['x', 'y', 'y-extent'],
  RestCollision: ['x', 'y', 'y-extent'],
  MultiMeasureRest: ['x', 'y', 'y-extent'],
  MultiMeasureRestNumber: ['x', 'y', 'y-extent'],
  MultiMeasureRestText: ['x', 'y', 'y-extent'],
  MultiMeasureRestScript: ['x', 'y', 'y-extent'],
  ClefModifier: ['x', 'y', 'y-extent'],
  BarLine: ['x', 'y', 'y-extent'],
  SpanBar: ['x', 'y', 'y-extent'],
  SpanBarStub: ['x', 'y', 'y-extent'],
  InstrumentName: ['x', 'y', 'y-extent'],
  PaperColumn: ['x'],
  NonMusicalPaperColumn: ['x'],
}

/** The grobs of a layout.tsv's `text`, in order: each { line, type, cause, …, stencil }. */
export function readLayout(text) {
  const grobs = []
  for (const [i, line] of text.split('\n').entries()) {
    if (!line) continue
    const values = line.split('\t')
    if (values.length !== FIELDS.length) throw new Error(`line ${i + 1} has ${values.length} fields, not ${FIELDS.length}: ${line}`)
    grobs.push({ line: i + 1, ...Object.fromEntries(FIELDS.map((f, j) => [f, values[j]])) })
  }
  return grobs
}

/** The number of grobs of each type in `grobs`. */
export function countTypes(grobs) {
  const counts = new Map()
  for (const grob of grobs) counts.set(grob.type, (counts.get(grob.type) ?? 0) + 1)
  return counts
}

/** Whether `a` and `b`, two numbers as layout.ly writes them, are within TOLERANCE. */
const near = (a, b) => a === b || (Number.isFinite(+a) && Number.isFinite(+b) && Math.abs(+a - +b) <= TOLERANCE + 1e-9)
/** Whether two values of `field` agree. */
function agree(field, a, b) {
  if (field === 'x' || field === 'y') return near(a, b)
  if (field === 'x-extent' || field === 'y-extent') {
    if (a === '-' || b === '-') return a === b
    const [x, y] = [a.split(' '), b.split(' ')]
    return x.length === 2 && y.length === 2 && near(x[0], y[0]) && near(x[1], y[1])
  }
  return a === b
}

/**
 * The differences of `got` from `reference`, two layout.tsv texts: { equal,
 * counts: [reference's, got's] grobs compared, missing, extra, order,
 * differ, unreached: got's UNREACHED fields counted by field }, on the
 * types and fields of `compared` ({ type: [field, …] }), or on every type
 * and field if it is null. Each entry names the lines of both sides; a
 * `differ` entry its field and both values.
 */
export function compareLayout(reference, got, { compared = COMPARED } = {}) {
  const [all, b] = [readLayout(reference), readLayout(got)]
  const unreached = {}
  for (const grob of b) {
    for (const field of FIELDS) if (grob[field] === UNREACHED) unreached[field] = (unreached[field] ?? 0) + 1
  }
  const fieldsOf = (type) => compared ? compared[type] : PLACED
  // Each grob's key: its six fields and its occurrence among those of the same six.
  const keyed = (grobs) => {
    const seen = new Map()
    return grobs.filter((g) => fieldsOf(g.type)).map((g) => {
      const key = GROB_FIELDS.map((f) => g[f]).join('\t')
      const n = seen.get(key) ?? 0
      seen.set(key, n + 1)
      return { grob: g, key, id: `${key}\t${n}` }
    })
  }
  const [x, y] = [keyed(all), keyed(b)]
  const result = { equal: true, counts: [x.length, y.length], missing: [], extra: [], order: [], differ: [], unreached }
  const ids = new Map(y.map((e) => [e.id, e]))
  const paired = new Set(x.filter((e) => ids.has(e.id)).map((e) => e.id))
  for (const e of x) if (!paired.has(e.id)) result.missing.push({ line: e.grob.line, grob: e.key })
  for (const e of y) if (!paired.has(e.id)) result.extra.push({ line: e.grob.line, grob: e.key })
  const [px, py] = [x.filter((e) => paired.has(e.id)), y.filter((e) => paired.has(e.id))]
  const inOrder = new Set(common(px.map((e) => e.id), py.map((e) => e.id)).map(([i]) => px[i].id))
  for (const e of px) {
    const other = ids.get(e.id).grob
    if (!inOrder.has(e.id)) result.order.push({ line: e.grob.line, got: other.line, grob: e.key })
    for (const field of fieldsOf(e.grob.type)) {
      const [r, v] = [e.grob[field], other[field]]
      if (v === UNREACHED || agree(field, r, v)) continue
      result.differ.push({ line: e.grob.line, got: other.line, field, reference: r, value: v })
    }
  }
  result.equal = !['missing', 'extra', 'order', 'differ'].some((k) => result[k].length)
  return result
}

/** columns.tsv's fields (MS3 step 31, D128): a column and its spacing wishes, springs and rods. */
export const COLUMN_FIELDS = ['column', 'spacing-wishes', 'springs', 'rods']
/** How many numbers follow the other column in an entry of springs and of rods. */
const NUMBERS = { springs: 4, rods: 1 }

/** The columns of a columns.tsv's `text`, in order: each { line, column, 'spacing-wishes', springs, rods }. */
export function readColumns(text) {
  const columns = []
  for (const [i, line] of text.split('\n').entries()) {
    if (!line) continue
    const values = line.split('\t')
    if (values.length !== COLUMN_FIELDS.length) throw new Error(`line ${i + 1} has ${values.length} fields, not ${COLUMN_FIELDS.length}: ${line}`)
    columns.push({ line: i + 1, ...Object.fromEntries(COLUMN_FIELDS.map((f, j) => [f, values[j]])) })
  }
  return columns
}

/** The entries of a springs or rods field: each [other column, …numbers]; a column's name has spaces in it. */
export function readEntries(field, value) {
  if (value === '-') return []
  return value.split('|').map((entry) => {
    const words = entry.split(' ')
    const n = words.length - NUMBERS[field]
    if (n < 1) throw new Error(`${field} entry ${entry} has no column`)
    return [words.slice(0, n).join(' '), ...words.slice(n)]
  })
}

/** Whether two values of columns.tsv's `field` agree: the springs' and rods' numbers to TOLERANCE. */
function agreeColumns(field, a, b) {
  if (!(field in NUMBERS) || a === b) return a === b
  const [x, y] = [readEntries(field, a), readEntries(field, b)]
  return x.length === y.length && x.every((e, i) => e[0] === y[i][0] && e.slice(1).every((v, j) => near(v, y[i][j + 1])))
}

/**
 * The differences of `got` from `reference`, two columns.tsv texts (D128),
 * as compareLayout gives them: { equal, counts, missing, extra, order,
 * differ, unreached }. A column pairs with the other side's of the same
 * name, the nth with the nth (each SpacingSpanner's set-springs writes the
 * columns again); its spacing wishes are compared as written, its springs
 * and rods entry by entry, each the same other column in the same place
 * and its numbers to TOLERANCE. A field `got` writes as UNREACHED is not
 * compared and is counted.
 */
export function compareColumns(reference, got) {
  return compareRows(readColumns(reference), readColumns(got), COLUMN_FIELDS, (c) => c.column, agreeColumns)
}

/**
 * The differences of the rows `b` from the rows `a` ({ line, …fields }),
 * each named by `name`: the nth of a name pairs with the other side's
 * nth; the fields after the first are compared by `agree`, an UNREACHED
 * one of `b` counted and not compared.
 */
function compareRows(a, b, fields, name, agree) {
  const unreached = {}
  for (const c of b) {
    for (const field of fields) if (c[field] === UNREACHED) unreached[field] = (unreached[field] ?? 0) + 1
  }
  const keyed = (rows) => {
    const seen = new Map()
    return rows.map((c) => {
      const n = seen.get(name(c)) ?? 0
      seen.set(name(c), n + 1)
      return { c, id: `${name(c)}\t${n}` }
    })
  }
  const [x, y] = [keyed(a), keyed(b)]
  const result = { equal: true, counts: [x.length, y.length], missing: [], extra: [], order: [], differ: [], unreached }
  const ids = new Map(y.map((e) => [e.id, e]))
  const paired = new Set(x.filter((e) => ids.has(e.id)).map((e) => e.id))
  for (const e of x) if (!paired.has(e.id)) result.missing.push({ line: e.c.line, column: e.c.column })
  for (const e of y) if (!paired.has(e.id)) result.extra.push({ line: e.c.line, column: e.c.column })
  const [px, py] = [x.filter((e) => paired.has(e.id)), y.filter((e) => paired.has(e.id))]
  const inOrder = new Set(common(px.map((e) => e.id), py.map((e) => e.id)).map(([i]) => px[i].id))
  for (const e of px) {
    const other = ids.get(e.id).c
    if (!inOrder.has(e.id)) result.order.push({ line: e.c.line, got: other.line, column: e.c.column })
    for (const field of fields.slice(1)) {
      const [r, v] = [e.c[field], other[field]]
      if (v === UNREACHED || agree(field, r, v)) continue
      result.differ.push({ line: e.c.line, got: other.line, field, reference: r, value: v })
    }
  }
  result.equal = !['missing', 'extra', 'order', 'differ'].some((k) => result[k].length)
  return result
}

/** line.tsv's fields (MS3 step 32, D129): a system's index, a column of its line and its X offset. */
export const LINE_FIELDS = ['system', 'column', 'x']

/** The columns of a line.tsv's `text`, in order: each { line, system, column, x }. */
export function readLine(text) {
  const columns = []
  for (const [i, line] of text.split('\n').entries()) {
    if (!line) continue
    const values = line.split('\t')
    if (values.length !== LINE_FIELDS.length) throw new Error(`line ${i + 1} has ${values.length} fields, not ${LINE_FIELDS.length}: ${line}`)
    columns.push({ line: i + 1, ...Object.fromEntries(LINE_FIELDS.map((f, j) => [f, values[j]])) })
  }
  return columns
}

/**
 * The differences of `got` from `reference`, two line.tsv texts (D129),
 * as compareColumns gives them: a column pairs with the other side's of
 * the same name on the system of the same index, the nth with the nth;
 * its X offset is compared to TOLERANCE, and counted, not compared, where
 * `got` writes it UNREACHED. The engine has one line, system 0.
 */
export function compareLine(reference, got) {
  const name = (c) => `${c.system}\t${c.column}`
  return compareRows(readLine(reference), readLine(got), ['column', 'x'], name, (_, a, b) => near(a, b))
}

/** The layout.tsv of `file`: itself, or a capture directory's. */
const layoutOf = (file) => statSync(file).isDirectory() ? path.join(file, LAYOUT) : file
const ID = /^[A-Z]\d\d$/

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2)
  const options = {}
  if (args[0] === '--all') {
    args.shift()
    options.compared = null
  }
  if (args.length !== 2 || args.some((a) => a.startsWith('--'))) {
    console.error('usage: node tools/compare/layout.mjs [--all] <reference> <other>')
    process.exit(2)
  }
  const [reference, other] = args
  const ids = statSync(reference).isDirectory() && !existsSync(path.join(reference, LAYOUT))
    ? readdirSync(reference).filter((f) => ID.test(f) && existsSync(path.join(reference, f, LAYOUT))).sort()
    : [undefined]
  let equal = true
  for (const id of ids) {
    const [r, o] = id ? [path.join(reference, id, LAYOUT), path.join(other, id, LAYOUT)] : [layoutOf(reference), layoutOf(other)]
    const result = existsSync(o)
      ? compareLayout(readFileSync(r, 'utf8'), readFileSync(o, 'utf8'), options)
      : { equal: false, absent: o }
    equal &&= result.equal
    console.log(JSON.stringify(id ? { id, ...result } : result))
  }
  process.exit(equal ? 0 : 1)
}
