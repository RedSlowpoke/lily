// The text packs of chapter 06 §5.3 (W1): each of the twelve URW styles
// LilyPond ships in its fonts/otf (C059, Nimbus Sans and Nimbus Mono PS,
// four styles each), and TeX Gyre Schola's four, which fontconfig gives
// "New Century Schoolbook" (OPEN E23, D271), reduced to what layout measures
// and outlines text with,
// as Pango shapes it through HarfBuzz: each glyph's advance, name and
// outline, the best cmap, the pair kerning of GPOS's `kern` feature and the
// ligatures of GSUB's `liga`, both as the `latn` script's default language
// system selects them. The SVG names the font and the browser draws it
// (D1); the outlines are the skylines' (E7, D209). crates/fonts/src/text.rs
// reads them.
//
//   node tools/packs/text.mjs [--otf dir] [--out dir]
//
//   --otf   LilyPond's fonts/otf (default: the installed LilyPond's)
//   --out   where the packs go (default dist/engine/fonts)
//
// It writes C059-Roman.pack … TeXGyreSchola-BoldItalic.pack, each named after
// its PostScript name (the URW OTFs' own; texgyreschola-regular.otf's is
// TeXGyreSchola-Regular), and beside each its outlines, C059-Roman.outlines.pack …, which a
// host gives only when a skyline first needs one (E14, D262), each with a
// brotli-11 .br beside it, as build.mjs writes the music packs.
//
// The format follows build.mjs's: every integer an unsigned LEB128 varint
// unless said otherwise, strings as length and UTF-8 bytes, zigzag varints for
// signed numbers, and each column of a table written whole before the next.
//
// A text pack, what layout measures text with:
//
//   "LILT" u8:4                   magic and version (2: the glyph names, D107;
//                                 3: the outlines and opentype.mjs' form, D209;
//                                 4: the ink boxes, the outlines a pack of
//                                 their own, D262)
//   string font                   the PostScript name (name 6): "C059-Roman"
//   string family                 name 1: "C059", "Nimbus Sans"
//   string style                  name 2: "Roman", "Bold Italic"
//   weight                        OS/2's usWeightClass
//   u8 italic                     OS/2's fsSelection bit 0
//   units-per-em
//   count                         glyphs, in the font's order; per glyph:
//     advance                     hmtx
//     string name                 per glyph, CFF's charset: the name Pango's
//                                 glyph strings carry (FT_Get_Glyph_Name)
//   count                         characters, by code point; per character:
//     code point - previous - 1   (the first: the code point)
//     zigzag glyph - previous glyph
//   kern and liga                 in opentype.mjs' form
//   per glyph: the ink box        zigzag x min, zigzag y min, width, height:
//                                 the control box of every point the outline
//                                 draws, as HarfBuzz's CFF extents count it
//                                 (inkBox; crates/fonts' ink_box reads it from
//                                 the outline and checks it); all 0 for none
//
// Its outlines, what the skylines read (E7, D209):
//
//   "LILO" u8:1                   magic and version
//   string font                   the text pack's PostScript name
//   count                         its glyphs
//   per glyph: length, outline    as build.mjs writes them
import { readdirSync, readFileSync, realpathSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { cff, installedFonts, outline, tables, writeOutline, Writer, writePack } from './build.mjs'
import { kerning, layoutGlyphs, substitutions, writeLayout } from './opentype.mjs'

/**
 * The text styles of LilyPond's fonts/otf by family (06 §5.3), by PostScript name: the URW faces, each an OTF of
 * that name, and TeX Gyre Schola's (E23, D271), whose OTFs are named in lower case (otfFile).
 */
export const STYLES = [
  'C059-Roman', 'C059-Bold', 'C059-Italic', 'C059-BdIta',
  'NimbusSans-Regular', 'NimbusSans-Bold', 'NimbusSans-Italic', 'NimbusSans-BoldItalic',
  'NimbusMonoPS-Regular', 'NimbusMonoPS-Bold', 'NimbusMonoPS-Italic', 'NimbusMonoPS-BoldItalic',
  'TeXGyreSchola-Regular', 'TeXGyreSchola-Bold', 'TeXGyreSchola-Italic', 'TeXGyreSchola-BoldItalic',
]

/** The OTF of `style` in LilyPond's fonts/otf: its PostScript name, or TeX Gyre's in lower case. */
export const otfFile = (style) => `${style.startsWith('TeXGyre') ? style.toLowerCase() : style}.otf`
/** The three styles in the core (06 §5.2); the others are fetched on demand. */
export const CORE_STYLES = ['C059-Roman', 'C059-Bold', 'C059-Italic']

// --- sfnt tables ---------------------------------------------------------------

/** A name of the name table: Windows Unicode (US English) or else Macintosh Roman. */
function name(table, id) {
  const count = table.readUInt16BE(2)
  const strings = table.readUInt16BE(4)
  let mac
  for (let i = 0; i < count; i++) {
    const at = 6 + 12 * i
    const [platform, encoding, language, nameId, length, offset] = [0, 2, 4, 6, 8, 10].map((k) => table.readUInt16BE(at + k))
    if (nameId !== id) continue
    const bytes = table.subarray(strings + offset, strings + offset + length)
    if (platform === 3 && encoding === 1 && language === 0x409) return Buffer.from(bytes).swap16().toString('utf16le')
    if (platform === 1 && encoding === 0 && language === 0) mac = bytes.toString('latin1')
  }
  if (mac === undefined) throw new Error(`no name ${id}`)
  return mac
}

/**
 * The best cmap, as HarfBuzz picks it: a format 12 subtable for the whole of
 * Unicode, or else a format 4 for the BMP. Code point to glyph, ascending.
 */
export function cmap(table) {
  const subtables = []
  for (let i = 0; i < table.readUInt16BE(2); i++) {
    const at = 4 + 8 * i
    const offset = table.readUInt32BE(at + 4)
    subtables.push({ platform: table.readUInt16BE(at), encoding: table.readUInt16BE(at + 2), format: table.readUInt16BE(offset), offset })
  }
  const pick = (platform, encoding, format) => subtables.find((s) => s.platform === platform && s.encoding === encoding && s.format === format)
  const best = pick(3, 10, 12) ?? pick(0, 6, 12) ?? pick(0, 4, 12) ?? pick(3, 1, 4) ?? pick(0, 3, 4)
  if (!best) throw new Error(`cmap: no Unicode subtable of format 4 or 12 (${subtables.map((s) => `${s.platform},${s.encoding} format ${s.format}`).join('; ')})`)
  const map = new Map()
  const at = best.offset
  if (best.format === 12) {
    for (let g = 0, groups = table.readUInt32BE(at + 12); g < groups; g++) {
      const [start, end, glyph] = [0, 4, 8].map((k) => table.readUInt32BE(at + 16 + 12 * g + k))
      for (let c = start; c <= end; c++) map.set(c, glyph + c - start)
    }
  } else {
    const segments = table.readUInt16BE(at + 6) / 2
    const ends = at + 14
    const starts = ends + 2 * segments + 2
    const deltas = starts + 2 * segments
    const ranges = deltas + 2 * segments
    for (let s = 0; s < segments; s++) {
      const [end, start, delta, range] = [ends, starts, deltas, ranges].map((base) => table.readUInt16BE(base + 2 * s))
      for (let c = start; c <= end && c !== 0xffff; c++) {
        let glyph = range === 0 ? c : table.readUInt16BE(ranges + 2 * s + range + 2 * (c - start))
        if (range !== 0 && glyph === 0) continue
        glyph = (glyph + delta) & 0xffff
        if (glyph !== 0) map.set(c, glyph)
      }
    }
  }
  return [...map].sort(([a], [b]) => a - b)
}

// --- the pack --------------------------------------------------------------------

/** What a text pack holds, read from one OTF. */
export function readTextFont(otf) {
  const found = tables(otf)
  const font = cff(found['CFF '])
  const glyphs = font.names.length
  if (found.maxp.readUInt16BE(4) !== glyphs) throw new Error('maxp and CFF disagree on the glyphs')
  const metrics = found.hhea.readUInt16BE(34)
  const advances = font.names.map((_, i) => found.hmtx.readUInt16BE(4 * Math.min(i, metrics - 1)))
  const outlines = font.names.map((_, i) => outline(font, i).path)
  const names = font.names
  const characters = cmap(found.cmap)
  const pairs = kerning(found.GPOS)
  const gsub = substitutions(found.GSUB, ['liga'])
  for (const g of [...characters.map(([, g]) => g), ...layoutGlyphs(pairs, gsub)]) if (g >= glyphs) throw new Error(`glyph ${g} of ${glyphs}`)
  return {
    font: name(found.name, 6),
    family: name(found.name, 1),
    style: name(found.name, 2),
    weight: found['OS/2'].readUInt16BE(4),
    italic: found['OS/2'].readUInt16BE(62) & 1,
    unitsPerEm: found.head.readUInt16BE(18),
    advances,
    names,
    characters,
    pairs,
    gsub,
    outlines,
  }
}

/**
 * The ink box of an outline as build.mjs' outline gives it, as crates/fonts'
 * ink_box reads it: the box of every point drawn, a move's counted once a line
 * or curve follows it; [0, 0, 0, 0] for none.
 */
export function inkBox(path) {
  let box
  const add = ([x, y]) => {
    box = box ? [Math.min(box[0], x), Math.min(box[1], y), Math.max(box[2], x), Math.max(box[3], y)] : [x, y, x, y]
  }
  let pen = [0, 0]
  let open = false
  for (const [op, ...deltas] of path) {
    if (op === 0) {
      pen = [pen[0] + deltas[0], pen[1] + deltas[1]]
      open = false
      continue
    }
    if (!open) add(pen)
    open = true
    for (let i = 0; i < deltas.length; i += 2) {
      pen = [pen[0] + deltas[i], pen[1] + deltas[i + 1]]
      add(pen)
    }
  }
  return box ?? [0, 0, 0, 0]
}

/** The text pack of `font`, as readTextFont gives it: its metrics, without the outlines (outlinesPack). */
export function textPack(font) {
  const w = new Writer()
  for (const c of 'LILT') w.u8(c.charCodeAt(0))
  w.u8(4)
  w.string(font.font)
  w.string(font.family)
  w.string(font.style)
  w.varint(font.weight)
  w.u8(font.italic)
  w.varint(font.unitsPerEm)
  w.varint(font.advances.length)
  font.advances.forEach((advance) => w.varint(advance))
  font.names.forEach((name) => w.string(name))
  w.varint(font.characters.length)
  let previous = [-1, 0]
  for (const [c, g] of font.characters) {
    w.varint(c - previous[0] - 1)
    w.zigzag(g - previous[1])
    previous = [c, g]
  }
  writeLayout(w, font.pairs, font.gsub)
  for (const path of font.outlines) {
    const [x0, y0, x1, y1] = inkBox(path)
    w.zigzag(x0)
    w.zigzag(y0)
    w.varint(x1 - x0)
    w.varint(y1 - y0)
  }
  return w.buffer()
}

/** The outlines of `font`, as readTextFont gives it: the pack a host gives when a skyline first needs one. */
export function outlinesPack(font) {
  const w = new Writer()
  for (const c of 'LILO') w.u8(c.charCodeAt(0))
  w.u8(1)
  w.string(font.font)
  w.varint(font.outlines.length)
  font.outlines.forEach((path) => writeOutline(w, path))
  return w.buffer()
}

/** The module of the outlines of the text face `style` (`C059-Roman.outlines`), its file that and `.pack`. */
export const outlinesModule = (style) => `${style}.outlines`

/** Writes the text pack and the outlines of each of `styles`, each with its .br, from the OTFs in `otf` into `out`; returns the files written. */
export function buildTextPacks({ otf = installedFonts(), out, styles = STYLES }) {
  const available = new Set(readdirSync(otf))
  return styles.flatMap((style) => {
    if (!available.has(otfFile(style))) throw new Error(`no ${otfFile(style)} in ${otf}`)
    try {
      const font = readTextFont(readFileSync(path.join(otf, otfFile(style))))
      if (font.font !== style) throw new Error(`its PostScript name is ${font.font}`)
      return [writePack(out, `${style}.pack`, textPack(font)), writePack(out, `${outlinesModule(style)}.pack`, outlinesPack(font))]
    } catch (error) {
      throw new Error(`${otfFile(style)}: ${error.message}`)
    }
  })
}

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
    const files = buildTextPacks({ otf: otf && path.resolve(otf), out })
    console.log(`${files.length / 2} text packs and their outlines in ${out}`)
  } catch (error) {
    console.error(`text packs: ${error.message}`)
    process.exit(1)
  }
}
