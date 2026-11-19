// The font packs of chapter 06 §5.1 (W1): each of LilyPond's Emmentaler OTFs
// turned into the engine's own format, which crates/fonts reads. A pack holds
// the glyph names in the font's order, each glyph's advance (hmtx), its LILC
// entry (bbox, attachment, attachment-down, ledger-shortening-range), the LILY
// table, each outline: the CFF charstring flattened, in integer font units,
// and the OpenType layout Pango shapes Emmentaler's text with (W6, D209):
// GPOS's kern and GSUB's cv47, dlig, ss01 and tnum. The engine reads no OTF at
// run time.
//
//   node tools/packs/build.mjs [--otf dir] [--out dir]
//
//   --otf   LilyPond's fonts/otf (default: the installed LilyPond's)
//   --out   where the packs go (default dist/engine/fonts)
//
// It writes emmentaler-11.pack … emmentaler-26.pack, the eight optical sizes,
// and emmentaler-brace-0.pack … -3.pack, the 576 braces in four parts of 144
// (06 §5.2), each with a brotli-11 .br beside it as scripts/build.mjs writes
// them (H5).
//
// The format, every integer an unsigned LEB128 varint unless said otherwise:
//
//   "LILP" u8:3                   magic and version (3: the OpenType layout, D209)
//   string font                   "emmentaler-20", "emmentaler-brace"
//   string postscript             the CFF's name, "Emmentaler-20", which
//                                 LilyPond's ly:font-name gives
//   first                         the font's index of the pack's first glyph
//   units-per-em
//   digits                        of the LILC decimals below
//   count { string key, u8 digits, decimal }   LILY, in the table's order
//   count                         glyphs, then per glyph in each column:
//     string name
//     advance
//     character                   0 for a glyph the cmap does not map, else
//                                 zigzag(code - next) + 1, next being one past
//                                 the pack's last mapped code (0 at first)
//     u8 flags { bbox: 4 decimals }{ attachment: 2 }{ attachment-down: 2 }
//              { ledger-shortening-range: 2 }   bits 0–3, LILC's entry, its
//              fields in this order, bbox a list and the others pairs, as
//              LILC has them
//     length, outline
//   kern and GSUB                 FEATURES', in opentype.mjs' form; none in a
//                                 brace part (emmentaler-brace has neither)
//
//   string   = length, UTF-8 bytes
//   decimal  = magnitude << 1 | sign: ±magnitude / 10^digits, so that "-0.000"
//              stays -0.0 and every value is the one Guile reads from the text
//   outline  = { op, deltas }*: 0 move (dx dy), 1 line (dx dy), 2 curve
//              (dx1 dy1 dx2 dy2 dx3 dy3), each point relative to the one
//              before as in the charstring, zigzag varints; a contour ends at
//              the next move or the outline's end, closed as CFF closes it.
import { mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { brotliCompressSync, constants, inflateSync } from 'node:zlib'
import { kerning, layoutGlyphs, substitutions, writeLayout } from './opentype.mjs'

export const SIZES = [11, 13, 14, 16, 18, 20, 23, 26]
/** Braces per part of emmentaler-brace (06 §5.2). */
export const BRACES_PER_PART = 144
const FLAG = { bbox: 1, attachment: 2, 'attachment-down': 4, 'ledger-shortening-range': 8 }
const ARITY = { bbox: 4, attachment: 2, 'attachment-down': 2, 'ledger-shortening-range': 2 }
/** The GSUB features of Emmentaler's that LilyPond's markups turn on (define-markup-commands.scm, define-grobs.scm). */
export const FEATURES = ['cv47', 'dlig', 'ss01', 'tnum']

// --- sfnt and CFF ------------------------------------------------------------

/** The tables of an OTF, by tag. */
export function tables(otf) {
  const count = otf.readUInt16BE(4)
  const found = {}
  for (let i = 0; i < count; i++) {
    const at = 12 + 16 * i
    const offset = otf.readUInt32BE(at + 8)
    found[otf.toString('latin1', at, at + 4)] = otf.subarray(offset, offset + otf.readUInt32BE(at + 12))
  }
  return found
}

/** A CFF INDEX at `at`: its items and the offset after it. */
function index(cff, at) {
  const count = cff.readUInt16BE(at)
  if (count === 0) return { items: [], end: at + 2 }
  const size = cff[at + 2]
  const offset = (i) => cff.readUIntBE(at + 3 + i * size, size)
  const base = at + 2 + (count + 1) * size
  const items = []
  for (let i = 0; i < count; i++) items.push(cff.subarray(base + offset(i), base + offset(i + 1)))
  return { items, end: base + offset(count) }
}

/** A CFF DICT: operator (12 x as 1200 + x) to its operands. */
function dict(bytes) {
  const entries = {}
  let operands = []
  for (let i = 0; i < bytes.length;) {
    const b = bytes[i]
    if (b <= 21) {
      const op = b === 12 ? 1200 + bytes[i + 1] : b
      i += b === 12 ? 2 : 1
      entries[op] = operands
      operands = []
    } else if (b === 28) {
      operands.push(bytes.readInt16BE(i + 1))
      i += 3
    } else if (b === 29) {
      operands.push(bytes.readInt32BE(i + 1))
      i += 5
    } else if (b === 30) {
      // A real: nibbles 0–9, then . E E- (reserved) - and 15 to end it.
      let text = ''
      for (let end = false; !end;) {
        const byte = bytes[++i]
        for (const n of [byte >> 4, byte & 15]) {
          if (n === 15) end = true
          if (!end) text += ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '.', 'E', 'E-', '', '-'][n]
        }
      }
      i++
      operands.push(Number(text))
    } else if (b >= 32 && b <= 246) {
      operands.push(b - 139)
      i++
    } else if (b >= 247 && b <= 250) {
      operands.push((b - 247) * 256 + bytes[i + 1] + 108)
      i += 2
    } else if (b >= 251 && b <= 254) {
      operands.push(-(b - 251) * 256 - bytes[i + 1] - 108)
      i += 2
    } else {
      throw new Error(`CFF DICT: byte ${b}`)
    }
  }
  return entries
}

/** The CFF's glyph names and charstrings, with what running them needs. */
export function cff(table) {
  const name = index(table, table[2])
  const top = index(table, name.end)
  const strings = index(table, top.end)
  const global = index(table, strings.end)
  if (top.items.length !== 1) throw new Error('CFF: one font expected')
  const entries = dict(top.items[0])
  if (entries[1230]) throw new Error('CFF: CID-keyed fonts are not supported')
  const matrix = entries[1207] ?? [0.001, 0, 0, 0.001, 0, 0]
  if (matrix.join() !== '0.001,0,0,0.001,0,0') throw new Error(`CFF: FontMatrix ${matrix}`)
  const charstrings = index(table, entries[17][0]).items
  const [privateSize, privateAt] = entries[18]
  const priv = dict(table.subarray(privateAt, privateAt + privateSize))
  const local = priv[19] ? index(table, privateAt + priv[19][0]).items : []

  const sid = (n) => (n < STANDARD.length ? STANDARD[n] : strings.items[n - STANDARD.length].toString('latin1'))
  const names = ['.notdef']
  const charset = entries[15]?.[0] ?? 0
  if (charset <= 2) throw new Error('CFF: a predefined charset is not supported')
  const format = table[charset]
  for (let at = charset + 1; names.length < charstrings.length;) {
    if (format === 0) {
      names.push(sid(table.readUInt16BE(at)))
      at += 2
    } else {
      const first = table.readUInt16BE(at)
      const left = format === 1 ? table[at + 2] : table.readUInt16BE(at + 2)
      for (let i = 0; i <= left; i++) names.push(sid(first + i))
      at += format === 1 ? 3 : 4
    }
  }
  return { name: name.items[0].toString('latin1'), names, charstrings, global: global.items, local, nominalWidth: priv[21]?.[0] ?? 0, defaultWidth: priv[20]?.[0] ?? 0 }
}

const bias = (subrs) => (subrs.length < 1240 ? 107 : subrs.length < 33900 ? 1131 : 32768)

/**
 * A Type 2 charstring run for its path: [op, ...relative points] with op 0
 * move, 1 line, 2 curve, and the width it gives, if any. Hints are skipped;
 * the arithmetic operators and seac are not supported, nor fractional points.
 */
export function outline(font, glyph) {
  const path = []
  const stack = []
  let stems = 0
  let width
  const take = (parity) => {
    // The first stack-clearing operator may carry the width before its arguments.
    if (width === undefined) width = stack.length % 2 === parity ? font.defaultWidth : font.nominalWidth + stack.shift()
  }
  const point = (op, ...deltas) => {
    for (const d of deltas) if (!Number.isInteger(d)) throw new Error(`a fractional point in ${font.names[glyph]}`)
    path.push([op, ...deltas])
  }
  const move = (dx, dy) => point(0, dx, dy)
  const run = (code, depth) => {
    if (depth > 10) throw new Error('charstring: subroutines nested too deep')
    for (let i = 0; i < code.length;) {
      const b = code[i++]
      if (b >= 32) {
        if (b <= 246) stack.push(b - 139)
        else if (b <= 250) stack.push((b - 247) * 256 + code[i++] + 108)
        else if (b <= 254) stack.push(-(b - 251) * 256 - code[i++] - 108)
        else {
          stack.push(code.readInt32BE(i) / 65536)
          i += 4
        }
        continue
      }
      if (b === 28) {
        stack.push(code.readInt16BE(i))
        i += 2
        continue
      }
      switch (b) {
        case 1: case 3: case 18: case 23: // hstem vstem hstemhm vstemhm
          take(0)
          stems += stack.length >> 1
          stack.length = 0
          break
        case 19: case 20: // hintmask cntrmask: a vstem's arguments may precede them
          take(0)
          stems += stack.length >> 1
          stack.length = 0
          i += (stems + 7) >> 3
          break
        case 21: take(0); move(stack[0], stack[1]); stack.length = 0; break
        case 22: take(1); move(stack[0], 0); stack.length = 0; break
        case 4: take(1); move(0, stack[0]); stack.length = 0; break
        case 5:
          for (let k = 0; k < stack.length; k += 2) point(1, stack[k], stack[k + 1])
          stack.length = 0
          break
        case 6: case 7: // hlineto vlineto: alternating
          stack.forEach((d, k) => ((k % 2 === 0) === (b === 6) ? point(1, d, 0) : point(1, 0, d)))
          stack.length = 0
          break
        case 8:
          for (let k = 0; k < stack.length; k += 6) point(2, ...stack.slice(k, k + 6))
          stack.length = 0
          break
        case 24: { // rcurveline
          let k = 0
          for (; k + 6 <= stack.length - 2; k += 6) point(2, ...stack.slice(k, k + 6))
          point(1, stack[k], stack[k + 1])
          stack.length = 0
          break
        }
        case 25: { // rlinecurve
          let k = 0
          for (; k + 2 <= stack.length - 6; k += 2) point(1, stack[k], stack[k + 1])
          point(2, ...stack.slice(k, k + 6))
          stack.length = 0
          break
        }
        case 26: { // vvcurveto
          let k = 0
          let dx1 = 0
          if (stack.length % 4 === 1) dx1 = stack[k++]
          for (; k < stack.length; k += 4, dx1 = 0) point(2, dx1, stack[k], stack[k + 1], stack[k + 2], 0, stack[k + 3])
          stack.length = 0
          break
        }
        case 27: { // hhcurveto
          let k = 0
          let dy1 = 0
          if (stack.length % 4 === 1) dy1 = stack[k++]
          for (; k < stack.length; k += 4, dy1 = 0) point(2, stack[k], dy1, stack[k + 1], stack[k + 2], stack[k + 3], 0)
          stack.length = 0
          break
        }
        case 30: case 31: { // vhcurveto hvcurveto: alternating, a last odd argument ends the last curve
          let horizontal = b === 31
          for (let k = 0; k + 4 <= stack.length; k += 4, horizontal = !horizontal) {
            const last = k + 5 === stack.length ? stack[k + 4] : 0
            if (horizontal) point(2, stack[k], 0, stack[k + 1], stack[k + 2], last, stack[k + 3])
            else point(2, 0, stack[k], stack[k + 1], stack[k + 2], stack[k + 3], last)
          }
          stack.length = 0
          break
        }
        case 10: case 29: { // callsubr callgsubr
          const subrs = b === 10 ? font.local : font.global
          // An endchar in a subroutine ends the glyph (the URW fonts' do).
          if (run(subrs[stack.pop() + bias(subrs)], depth + 1) === 'end') return 'end'
          break
        }
        case 11: return
        case 14:
          take(0)
          if (stack.length) throw new Error(`seac in ${font.names[glyph]} is not supported`)
          return 'end'
        case 12: {
          const op = code[i++]
          const s = stack
          if (op === 35) { // flex
            point(2, ...s.slice(0, 6))
            point(2, ...s.slice(6, 12))
          } else if (op === 34) { // hflex
            point(2, s[0], 0, s[1], s[2], s[3], 0)
            point(2, s[4], 0, s[5], -s[2], s[6], 0)
          } else if (op === 36) { // hflex1
            point(2, s[0], s[1], s[2], s[3], s[4], 0)
            point(2, s[5], 0, s[6], s[7], s[8], -(s[1] + s[3] + s[7]))
          } else if (op === 37) { // flex1
            const dx = s[0] + s[2] + s[4] + s[6] + s[8]
            const dy = s[1] + s[3] + s[5] + s[7] + s[9]
            point(2, ...s.slice(0, 6))
            if (Math.abs(dx) > Math.abs(dy)) point(2, s[6], s[7], s[8], s[9], s[10], -dy)
            else point(2, s[6], s[7], s[8], s[9], -dx, s[10])
          } else {
            throw new Error(`charstring operator 12 ${op} in ${font.names[glyph]} is not supported`)
          }
          stack.length = 0
          break
        }
        default:
          throw new Error(`charstring operator ${b} in ${font.names[glyph]} is not supported`)
      }
    }
  }
  if (run(font.charstrings[glyph], 0) !== 'end') throw new Error(`${font.names[glyph]} has no endchar`)
  return { path, width: width ?? font.defaultWidth }
}

/**
 * The character of each glyph, as LilyPond's make_index_to_charcode_map
 * reads the cmap through FreeType: the Unicode charmap FreeType selects
 * (find_unicode_charmap: the last UCS-4 subtable, else the last Unicode
 * one), each glyph from 1 to `glyphs` that a character maps to given the
 * highest such character. Formats 4 and 12; glyph index to code.
 */
export function charcodes(table, glyphs) {
  const subtables = []
  for (let i = 0; i < table.readUInt16BE(2); i++) {
    subtables.push({ platform: table.readUInt16BE(4 + 8 * i), encoding: table.readUInt16BE(6 + 8 * i), at: table.readUInt32BE(8 + 8 * i) })
  }
  const ucs4 = (s) => (s.platform === 3 && s.encoding === 10) || (s.platform === 0 && s.encoding === 4)
  const unicode = (s) => s.platform === 0 || (s.platform === 3 && (s.encoding === 1 || s.encoding === 10))
  const chosen = subtables.findLast(ucs4) ?? subtables.findLast(unicode)
  if (!chosen) throw new Error('cmap: no Unicode subtable')
  const pairs = []
  const map = (code, glyph) => {
    if (glyph > 0 && glyph < glyphs) pairs.push([code, glyph])
  }
  const at = chosen.at
  const format = table.readUInt16BE(at)
  if (format === 4) {
    const segments = table.readUInt16BE(at + 6) / 2
    const ends = at + 14
    const starts = ends + 2 * segments + 2
    const deltas = starts + 2 * segments
    const ranges = deltas + 2 * segments
    for (let k = 0; k < segments; k++) {
      const [start, end] = [table.readUInt16BE(starts + 2 * k), table.readUInt16BE(ends + 2 * k)]
      const [delta, range] = [table.readUInt16BE(deltas + 2 * k), table.readUInt16BE(ranges + 2 * k)]
      for (let code = start; code <= end && code !== 0xffff; code++) {
        if (range === 0) map(code, (code + delta) & 0xffff)
        else {
          const glyph = table.readUInt16BE(ranges + 2 * k + range + 2 * (code - start))
          map(code, glyph === 0 ? 0 : (glyph + delta) & 0xffff)
        }
      }
    }
  } else if (format === 12) {
    for (let k = 0, n = table.readUInt32BE(at + 12); k < n; k++) {
      const group = at + 16 + 12 * k
      const [start, end, glyph] = [0, 4, 8].map((d) => table.readUInt32BE(group + d))
      for (let code = start; code <= end; code++) map(code, glyph + code - start)
    }
  } else {
    throw new Error(`cmap: format ${format} is not supported`)
  }
  const codes = new Map()
  for (const [code, glyph] of pairs.sort((a, b) => a[0] - b[0])) codes.set(glyph, code)
  return codes
}

// --- LilyPond's tables -------------------------------------------------------

/** A LILC, LILY or LILF table as text: zlib-compressed or not, as load_scheme_table reads it. */
export function schemeText(raw) {
  try {
    return inflateSync(raw).toString('latin1')
  } catch {
    return raw.toString('latin1')
  }
}

/** The table's data as s-expressions: lists as arrays, `(a . b)` as {car, cdr}, atoms as strings. */
export function sexps(text) {
  const tokens = text.match(/[().]|[^\s()]+/g) ?? []
  let at = 0
  const read = () => {
    const token = tokens[at++]
    if (token !== '(') return token
    const items = []
    while (tokens[at] !== ')') {
      if (tokens[at] === '.') {
        at++
        const cdr = read()
        at++
        return items.length === 1 ? { car: items[0], cdr } : { items, cdr }
      }
      items.push(read())
    }
    at++
    return items
  }
  const data = []
  while (at < tokens.length) data.push(read())
  return data
}

/** A decimal as LilyPond writes it: its sign, digits after the point, and magnitude at `digits`. */
function decimal(text, digits) {
  const match = /^([-+]?)(\d*)(?:\.(\d*))?$/.exec(text)
  if (!match) throw new Error(`not a decimal: ${text}`)
  const fraction = (match[3] ?? '').padEnd(digits, '0')
  if (fraction.length > digits) throw new Error(`${text} has more than ${digits} digits`)
  return { negative: match[1] === '-', magnitude: BigInt(match[2] + fraction) }
}

const places = (text) => text.split('.')[1]?.length ?? 0

/** LILC as glyph name to { key: [decimal texts] }; LILY as [key, decimal text]. */
export function lilyTables(otf) {
  const found = tables(otf)
  const lilc = new Map()
  for (const entry of sexps(schemeText(found.LILC))) {
    const fields = {}
    for (const field of entry.cdr) {
      if (!(field.car in ARITY)) throw new Error(`LILC: unknown field ${field.car} of ${entry.car}`)
      // The pack keeps no form: bbox is a list, the others pairs, in FLAG's order.
      if (Array.isArray(field.cdr) !== (field.car === 'bbox')) throw new Error(`LILC: ${field.car} of ${entry.car} is not as FLAG has it`)
      const values = Array.isArray(field.cdr) ? field.cdr : [field.cdr.car ?? field.cdr.items[0], field.cdr.cdr]
      if (values.length !== ARITY[field.car]) throw new Error(`LILC: ${field.car} of ${entry.car} has ${values.length} values`)
      // Each a real, as Guile reads a number with a point.
      if (values.some((v) => !v.includes('.'))) throw new Error(`LILC: ${field.car} of ${entry.car} has an exact number`)
      fields[field.car] = values
    }
    const keys = Object.keys(fields)
    if (keys.join() !== Object.keys(FLAG).filter((key) => key in fields).join()) throw new Error(`LILC: the fields of ${entry.car} are not in FLAG's order`)
    lilc.set(entry.car, fields)
  }
  const lily = sexps(schemeText(found.LILY)).map((entry) => [entry.car, entry.cdr])
  return { lilc, lily }
}

// --- the pack ----------------------------------------------------------------

export class Writer {
  bytes = []
  u8(value) {
    this.bytes.push(value)
  }
  varint(value) {
    let v = BigInt(value)
    if (v < 0n) throw new Error(`varint ${value}`)
    do {
      const low = Number(v & 0x7fn)
      v >>= 7n
      this.bytes.push(v ? low | 0x80 : low)
    } while (v)
  }
  zigzag(value) {
    this.varint(value < 0 ? -2 * value - 1 : 2 * value)
  }
  string(text) {
    const bytes = Buffer.from(text, 'utf8')
    this.varint(bytes.length)
    for (const b of bytes) this.bytes.push(b)
  }
  decimal(text, digits) {
    const { negative, magnitude } = decimal(text, digits)
    this.varint((magnitude << 1n) | (negative ? 1n : 0n))
  }
  buffer() {
    return Buffer.from(this.bytes)
  }
}

/** The font of one OTF: names, advances, LILC and LILY, outlines. */
export function readFont(otf) {
  const found = tables(otf)
  const font = cff(found['CFF '])
  const glyphs = font.names.length
  const metrics = found.hhea.readUInt16BE(34)
  const advances = font.names.map((_, i) => found.hmtx.readUInt16BE(4 * Math.min(i, metrics - 1)))
  const outlines = font.names.map((_, i) => outline(font, i))
  // The charstrings' widths agree with hmtx, the advances FreeType gives, except
  // emmentaler-brace's .notdef (-2 against 500).
  outlines.forEach(({ width }, i) => {
    if (i > 0 && width !== advances[i]) throw new Error(`${font.names[i]}: charstring width ${width}, hmtx ${advances[i]}`)
  })
  const { lilc, lily } = lilyTables(otf)
  for (const name of lilc.keys()) if (!font.names.includes(name)) throw new Error(`LILC names ${name}, which the font has not`)
  if (found.maxp.readUInt16BE(4) !== glyphs) throw new Error('maxp and CFF disagree on the glyphs')
  const codes = charcodes(found.cmap, glyphs)
  const pairs = kerning(found.GPOS)
  const gsub = substitutions(found.GSUB, FEATURES)
  for (const g of layoutGlyphs(pairs, gsub)) if (g >= glyphs) throw new Error(`glyph ${g} of ${glyphs}`)
  return { postscript: font.name, names: font.names, advances, codes, outlines: outlines.map((o) => o.path), lilc, lily, unitsPerEm: found.head.readUInt16BE(18), pairs, gsub }
}

/** Writes an outline as a pack holds it: its length, then { op, deltas }*. */
export function writeOutline(w, path) {
  const o = new Writer()
  for (const [op, ...deltas] of path) {
    o.u8(op)
    deltas.forEach((d) => o.zigzag(d))
  }
  w.varint(o.bytes.length)
  for (const b of o.bytes) w.u8(b)
}

/** The pack of `font`'s glyphs `from` to `to` (exclusive), named `name`. */
export function pack(font, name, from = 0, to = font.names.length) {
  const digits = Math.max(0, ...[...font.lilc.values()].flatMap((fields) => Object.values(fields).flat()).map(places))
  const w = new Writer()
  for (const c of 'LILP') w.u8(c.charCodeAt(0))
  w.u8(3)
  w.string(name)
  w.string(font.postscript)
  w.varint(from)
  w.varint(font.unitsPerEm)
  w.varint(digits)
  w.varint(font.lily.length)
  for (const [key, value] of font.lily) {
    w.string(key)
    w.u8(places(value))
    w.decimal(value, places(value))
  }
  const range = font.names.slice(from, to).map((_, k) => from + k)
  w.varint(range.length)
  for (const i of range) w.string(font.names[i])
  for (const i of range) w.varint(font.advances[i])
  let next = 0
  for (const i of range) {
    const code = font.codes.get(i)
    if (code === undefined) {
      w.varint(0)
    } else {
      const d = code - next
      w.varint((d < 0 ? -2 * d - 1 : 2 * d) + 1)
      next = code + 1
    }
  }
  for (const i of range) {
    const fields = font.lilc.get(font.names[i]) ?? {}
    w.u8(Object.keys(fields).reduce((flags, key) => flags | FLAG[key], 0))
    for (const key of Object.keys(FLAG)) for (const value of fields[key] ?? []) w.decimal(value, digits)
  }
  for (const i of range) writeOutline(w, font.outlines[i])
  // Glyph indices are the font's: only a pack of the whole font holds its layout.
  const whole = from === 0 && to === font.names.length
  if (!whole && (font.pairs.length || font.gsub.lookups.length)) throw new Error(`${name}: a part of a font with kern or GSUB`)
  writeLayout(w, font.pairs, font.gsub)
  return w.buffer()
}

/** The installed LilyPond's fonts/otf. */
export function installedFonts() {
  const lilypond = spawnSync('sh', ['-c', 'command -v lilypond'], { encoding: 'utf8' })
  if (lilypond.status !== 0) throw new Error('LilyPond is not installed; give its fonts/otf as --otf')
  const share = path.join(path.dirname(realpathSync(lilypond.stdout.trim())), '..', 'share', 'lilypond')
  const version = spawnSync('ls', [share], { encoding: 'utf8' }).stdout.split('\n')[0]
  return path.join(share, version, 'fonts', 'otf')
}

/** Writes `bytes` as `file` in `out`, with its brotli-11 .br beside it (H5); returns `file`. */
export function writePack(out, file, bytes) {
  mkdirSync(out, { recursive: true })
  writeFileSync(path.join(out, file), bytes)
  writeFileSync(path.join(out, `${file}.br`), brotliCompressSync(bytes, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }))
  return file
}

/** Writes every pack, each with its .br, from the OTFs in `otf` into `out`; returns the files written. */
export function buildPacks({ otf = installedFonts(), out }) {
  const written = []
  const write = (file, bytes) => written.push(writePack(out, file, bytes))
  const read = (name) => {
    try {
      return readFont(readFileSync(path.join(otf, `${name}.otf`)))
    } catch (error) {
      throw new Error(`${name}.otf: ${error.message}`)
    }
  }
  for (const size of SIZES) write(`emmentaler-${size}.pack`, pack(read(`emmentaler-${size}`), `emmentaler-${size}`))
  const braces = read('emmentaler-brace')
  // .notdef, glyph 0, goes with the first part.
  const parts = Math.ceil((braces.names.length - 1) / BRACES_PER_PART)
  for (let part = 0; part < parts; part++) {
    const from = part === 0 ? 0 : 1 + part * BRACES_PER_PART
    write(`emmentaler-brace-${part}.pack`, pack(braces, 'emmentaler-brace', from, Math.min(braces.names.length, 1 + (part + 1) * BRACES_PER_PART)))
  }
  return written
}

// The 391 standard strings of the CFF specification (Adobe TN 5176, Appendix A), SIDs 0–390.
const STANDARD = (
  '.notdef space exclam quotedbl numbersign dollar percent ampersand quoteright parenleft parenright ' +
  'asterisk plus comma hyphen period slash zero one two three four five six seven eight nine colon ' +
  'semicolon less equal greater question at A B C D E F G H I J K L M N O P Q R S T U V W X Y Z ' +
  'bracketleft backslash bracketright asciicircum underscore quoteleft a b c d e f g h i j k l m n o p ' +
  'q r s t u v w x y z braceleft bar braceright asciitilde exclamdown cent sterling fraction yen ' +
  'florin section currency quotesingle quotedblleft guillemotleft guilsinglleft guilsinglright fi fl ' +
  'endash dagger daggerdbl periodcentered paragraph bullet quotesinglbase quotedblbase quotedblright ' +
  'guillemotright ellipsis perthousand questiondown grave acute circumflex tilde macron breve ' +
  'dotaccent dieresis ring cedilla hungarumlaut ogonek caron emdash AE ordfeminine Lslash Oslash OE ' +
  'ordmasculine ae dotlessi lslash oslash oe germandbls onesuperior logicalnot mu trademark Eth ' +
  'onehalf plusminus Thorn onequarter divide brokenbar degree thorn threequarters twosuperior ' +
  'registered minus eth multiply threesuperior copyright Aacute Acircumflex Adieresis Agrave Aring ' +
  'Atilde Ccedilla Eacute Ecircumflex Edieresis Egrave Iacute Icircumflex Idieresis Igrave Ntilde ' +
  'Oacute Ocircumflex Odieresis Ograve Otilde Scaron Uacute Ucircumflex Udieresis Ugrave Yacute ' +
  'Ydieresis Zcaron aacute acircumflex adieresis agrave aring atilde ccedilla eacute ecircumflex ' +
  'edieresis egrave iacute icircumflex idieresis igrave ntilde oacute ocircumflex odieresis ograve ' +
  'otilde scaron uacute ucircumflex udieresis ugrave yacute ydieresis zcaron exclamsmall ' +
  'Hungarumlautsmall dollaroldstyle dollarsuperior ampersandsmall Acutesmall parenleftsuperior ' +
  'parenrightsuperior twodotenleader onedotenleader zerooldstyle oneoldstyle twooldstyle threeoldstyle ' +
  'fouroldstyle fiveoldstyle sixoldstyle sevenoldstyle eightoldstyle nineoldstyle commasuperior ' +
  'threequartersemdash periodsuperior questionsmall asuperior bsuperior centsuperior dsuperior ' +
  'esuperior isuperior lsuperior msuperior nsuperior osuperior rsuperior ssuperior tsuperior ff ffi ' +
  'ffl parenleftinferior parenrightinferior Circumflexsmall hyphensuperior Gravesmall Asmall Bsmall ' +
  'Csmall Dsmall Esmall Fsmall Gsmall Hsmall Ismall Jsmall Ksmall Lsmall Msmall Nsmall Osmall Psmall ' +
  'Qsmall Rsmall Ssmall Tsmall Usmall Vsmall Wsmall Xsmall Ysmall Zsmall colonmonetary onefitted ' +
  'rupiah Tildesmall exclamdownsmall centoldstyle Lslashsmall Scaronsmall Zcaronsmall Dieresissmall ' +
  'Brevesmall Caronsmall Dotaccentsmall Macronsmall figuredash hypheninferior Ogoneksmall Ringsmall ' +
  'Cedillasmall questiondownsmall oneeighth threeeighths fiveeighths seveneighths onethird twothirds ' +
  'zerosuperior foursuperior fivesuperior sixsuperior sevensuperior eightsuperior ninesuperior ' +
  'zeroinferior oneinferior twoinferior threeinferior fourinferior fiveinferior sixinferior ' +
  'seveninferior eightinferior nineinferior centinferior dollarinferior periodinferior commainferior ' +
  'Agravesmall Aacutesmall Acircumflexsmall Atildesmall Adieresissmall Aringsmall AEsmall ' +
  'Ccedillasmall Egravesmall Eacutesmall Ecircumflexsmall Edieresissmall Igravesmall Iacutesmall ' +
  'Icircumflexsmall Idieresissmall Ethsmall Ntildesmall Ogravesmall Oacutesmall Ocircumflexsmall ' +
  'Otildesmall Odieresissmall OEsmall Oslashsmall Ugravesmall Uacutesmall Ucircumflexsmall ' +
  'Udieresissmall Yacutesmall Thornsmall Ydieresissmall 001.000 001.001 001.002 001.003 Black Bold ' +
  'Book Light Medium Regular Roman Semibold '
).trim().split(' ')

if (process.argv[1] && realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2)
  const option = (flag) => {
    const at = args.indexOf(flag)
    return at < 0 ? undefined : args[at + 1]
  }
  const root = fileURLToPath(new URL('../..', import.meta.url))
  const out = path.resolve(option('--out') ?? path.join(root, 'dist/engine/fonts'))
  const otf = option('--otf')
  try {
    const files = buildPacks({ otf: otf && path.resolve(otf), out })
    console.log(`${files.length} packs in ${out}`)
  } catch (error) {
    console.error(`packs: ${error.message}`)
    process.exit(1)
  }
}
