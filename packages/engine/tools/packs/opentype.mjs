// The OpenType layout both kinds of pack hold (D209): GPOS's `kern` pairs
// and GSUB's lookups with the features that name them, read from an OTF as
// HarfBuzz selects them for Latin text, and written in the packs' shared
// form. build.mjs writes Emmentaler's (`cv47`, `dlig`, `ss01`, `tnum`),
// text.mjs the URW fonts' (`liga`); crates/fonts reads both.
//
// The form, after build.mjs's and text.mjs' columns: every integer an
// unsigned LEB128 varint unless said otherwise, zigzag varints for signed
// numbers, strings as length and UTF-8 bytes.
//
//   count                         first glyphs of kerning pairs, ascending; per first:
//     first - previous first - 1, count of its pairs, then per pair:
//     second - previous second - 1, zigzag x advance
//   count                         lookups, in the font's lookup order; per lookup:
//     u8 type                     1 or 4
//     type 1, a single substitution: count, then per glyph, ascending:
//       glyph - previous glyph - 1, zigzag substitute - glyph
//     type 4, a ligature substitution: count, then per ligature, in the
//       font's order (the first that matches wins):
//       count of glyphs, the glyphs (the first one first), the ligature glyph
//   count                         features, by tag; per feature:
//     string tag                  "liga", "cv47"
//     count, then the positions of its lookups in the list above, ascending:
//       position - previous position - 1

/** A Coverage table: its glyphs in coverage order. */
function coverage(table, at) {
  const format = table.readUInt16BE(at)
  const count = table.readUInt16BE(at + 2)
  const glyphs = []
  for (let i = 0; i < count; i++) {
    if (format === 1) glyphs.push(table.readUInt16BE(at + 4 + 2 * i))
    else {
      const [start, end] = [0, 2].map((k) => table.readUInt16BE(at + 4 + 6 * i + k))
      for (let g = start; g <= end; g++) glyphs.push(g)
    }
  }
  return glyphs
}

/**
 * The lookups of GSUB's or GPOS's `feature` that HarfBuzz applies to text of
 * the `latn` script in its default language system (`DFLT` if the font has no
 * `latn`), in the lookup list's order, each as { index, type, subtables: [offset] }
 * with extension lookups (`extension`) resolved.
 */
function featureLookups(table, feature, extension) {
  const [scripts, features, lookups] = [4, 6, 8].map((k) => table.readUInt16BE(k))
  const tag = (at) => table.toString('latin1', at, at + 4)
  let script
  for (let i = 0, n = table.readUInt16BE(scripts); i < n; i++) {
    const at = scripts + 2 + 6 * i
    if (tag(at) === 'latn' || (tag(at) === 'DFLT' && script === undefined)) script = scripts + table.readUInt16BE(at + 4)
  }
  if (script === undefined) return []
  const langSys = table.readUInt16BE(script)
  if (langSys === 0) return []
  const lang = script + langSys
  const required = table.readUInt16BE(lang + 2)
  const indices = []
  for (let i = 0, n = table.readUInt16BE(lang + 4); i < n; i++) indices.push(table.readUInt16BE(lang + 6 + 2 * i))
  if (required !== 0xffff) indices.push(required)
  const chosen = new Set()
  for (const index of indices) {
    const at = features + 2 + 6 * index
    if (tag(at) !== feature) continue
    const record = features + table.readUInt16BE(at + 4)
    for (let i = 0, n = table.readUInt16BE(record + 2); i < n; i++) chosen.add(table.readUInt16BE(record + 4 + 2 * i))
  }
  return [...chosen].sort((a, b) => a - b).map((index) => {
    const at = lookups + table.readUInt16BE(lookups + 2 + 2 * index)
    const subtables = []
    for (let i = 0, n = table.readUInt16BE(at + 4); i < n; i++) subtables.push(at + table.readUInt16BE(at + 6 + 2 * i))
    if (table.readUInt16BE(at) !== extension) return { index, type: table.readUInt16BE(at), subtables }
    // An extension lookup: each subtable points at one of the real type, the same for all.
    const types = new Set(subtables.map((sub) => table.readUInt16BE(sub + 2)))
    if (types.size !== 1) throw new Error(`lookup ${index}: extension subtables of ${types.size} types`)
    return { index, type: [...types][0], subtables: subtables.map((sub) => sub + table.readUInt32BE(sub + 4)) }
  })
}

/** GPOS's `kern` pairs: [first, second, x advance], sorted; only one lookup of pair adjustments with an x advance on the first glyph. */
export function kerning(table) {
  if (!table) return []
  const lookups = featureLookups(table, 'kern', 9)
  if (lookups.length > 1) throw new Error(`GPOS: ${lookups.length} kern lookups; only one is supported`)
  const pairs = []
  const seen = new Set()
  for (const { index, type, subtables } of lookups) {
    if (type !== 2) throw new Error(`GPOS: kern lookup ${index} has type ${type}; only pair adjustments are supported`)
    for (const at of subtables) {
      const [format, cover, value1, value2] = [0, 2, 4, 6].map((k) => table.readUInt16BE(at + k))
      if (format !== 1) throw new Error(`GPOS: kern lookup ${index} has a pair adjustment of format ${format}; only format 1 is supported`)
      if (value1 !== 4 || value2 !== 0) throw new Error(`GPOS: kern lookup ${index} has value formats ${value1}, ${value2}; only an x advance on the first glyph is supported`)
      coverage(table, at + cover).forEach((first, i) => {
        const set = at + table.readUInt16BE(at + 10 + 2 * i)
        for (let k = 0, n = table.readUInt16BE(set); k < n; k++) {
          const record = set + 2 + 4 * k
          const second = table.readUInt16BE(record)
          // A pair an earlier subtable has wins, as HarfBuzz stops at the first that matches.
          if (seen.has(first * 0x10000 + second)) continue
          seen.add(first * 0x10000 + second)
          pairs.push([first, second, table.readInt16BE(record + 2)])
        }
      })
    }
  }
  return pairs.sort(([a, b], [c, d]) => a - c || b - d)
}

/** A ligature substitution's ligatures, its subtables' in order: { glyphs, ligature }. */
function ligatureSubstitution(table, index, subtables) {
  const found = []
  for (const at of subtables) {
    if (table.readUInt16BE(at) !== 1) throw new Error(`GSUB: lookup ${index}: a ligature substitution of format ${table.readUInt16BE(at)}`)
    coverage(table, at + table.readUInt16BE(at + 2)).forEach((first, i) => {
      const set = at + table.readUInt16BE(at + 6 + 2 * i)
      for (let k = 0, n = table.readUInt16BE(set); k < n; k++) {
        const liga = set + table.readUInt16BE(set + 2 + 2 * k)
        const glyphs = [first]
        for (let c = 1, components = table.readUInt16BE(liga + 2); c < components; c++) glyphs.push(table.readUInt16BE(liga + 2 + 2 * c))
        found.push({ glyphs, ligature: table.readUInt16BE(liga) })
      }
    })
  }
  return found
}

/** A single substitution's [glyph, substitute] pairs, ascending; a glyph an earlier subtable covers keeps its substitute. */
function singleSubstitution(table, index, subtables) {
  const map = new Map()
  for (const at of subtables) {
    const format = table.readUInt16BE(at)
    const glyphs = coverage(table, at + table.readUInt16BE(at + 2))
    glyphs.forEach((glyph, i) => {
      if (map.has(glyph)) return
      if (format === 1) map.set(glyph, (glyph + table.readInt16BE(at + 4)) & 0xffff)
      else if (format === 2) map.set(glyph, table.readUInt16BE(at + 6 + 2 * i))
      else throw new Error(`GSUB: lookup ${index}: a single substitution of format ${format}`)
    })
  }
  return [...map].sort(([a], [b]) => a - b)
}

/**
 * GSUB's lookups of `features` (tags), as HarfBuzz selects them for Latin
 * text: { lookups: [{ type: 1, pairs } | { type: 4, ligatures }] in the
 * font's lookup order, features: [[tag, [position]]] } for the features the
 * font has, by tag.
 */
export function substitutions(table, features) {
  const lookups = new Map()
  const named = []
  for (const tag of [...features].sort()) {
    const found = table ? featureLookups(table, tag, 7) : []
    if (!found.length) continue
    for (const { index, type, subtables } of found) {
      if (lookups.has(index)) continue
      if (type === 1) lookups.set(index, { type, pairs: singleSubstitution(table, index, subtables) })
      else if (type === 4) lookups.set(index, { type, ligatures: ligatureSubstitution(table, index, subtables) })
      else throw new Error(`GSUB: ${tag} lookup ${index} has type ${type}; only single and ligature substitutions are supported`)
    }
    named.push([tag, found.map((l) => l.index)])
  }
  const order = [...lookups.keys()].sort((a, b) => a - b)
  return {
    lookups: order.map((index) => lookups.get(index)),
    features: named.map(([tag, indices]) => [tag, indices.map((index) => order.indexOf(index))]),
  }
}

/** The glyphs `pairs` and `gsub` name, to check against the font's count. */
export function layoutGlyphs(pairs, gsub) {
  return [
    ...pairs.flatMap(([a, b]) => [a, b]),
    ...gsub.lookups.flatMap((l) => (l.type === 1 ? l.pairs.flat() : l.ligatures.flatMap((g) => [...g.glyphs, g.ligature]))),
  ]
}

/** Writes `pairs` (kerning's) and `gsub` (substitutions') in the shared form with `w`, build.mjs' Writer. */
export function writeLayout(w, pairs, gsub) {
  const firsts = Map.groupBy(pairs, ([first]) => first)
  w.varint(firsts.size)
  let first = -1
  for (const [glyph, seconds] of firsts) {
    w.varint(glyph - first - 1)
    first = glyph
    w.varint(seconds.length)
    let second = -1
    for (const [, b, value] of seconds) {
      w.varint(b - second - 1)
      second = b
      w.zigzag(value)
    }
  }
  w.varint(gsub.lookups.length)
  for (const lookup of gsub.lookups) {
    w.u8(lookup.type)
    if (lookup.type === 1) {
      w.varint(lookup.pairs.length)
      let previous = -1
      for (const [glyph, substitute] of lookup.pairs) {
        w.varint(glyph - previous - 1)
        previous = glyph
        w.zigzag(substitute - glyph)
      }
    } else {
      w.varint(lookup.ligatures.length)
      for (const { glyphs, ligature } of lookup.ligatures) {
        w.varint(glyphs.length)
        glyphs.forEach((g) => w.varint(g))
        w.varint(ligature)
      }
    }
  }
  w.varint(gsub.features.length)
  for (const [tag, positions] of gsub.features) {
    w.string(tag)
    w.varint(positions.length)
    let previous = -1
    for (const position of positions) {
      w.varint(position - previous - 1)
      previous = position
    }
  }
}
