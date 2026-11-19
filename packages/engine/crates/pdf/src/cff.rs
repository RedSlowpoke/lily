//! 06 §7.3's own CFF subsetter, and what the PDF's font dictionaries read
//! of an OpenType font. The URW fonts LilyPond ships are static OpenType
//! fonts whose outlines are a name-keyed CFF table, so a subset is the same
//! CFF with the charstrings of the glyphs no text draws, and the
//! subroutines no kept charstring calls, each made one operator long: every
//! glyph keeps its id, so a CID of the PDF's `Identity-H` text is the
//! glyph id the core shaped (PDF 2.0 §9.7.4.2: a CFF that is not CID-keyed
//! has its glyph ids for CIDs). The charset, the strings and the Top and
//! Private DICTs are copied, their offsets written again.

use alloc::collections::BTreeSet;
use alloc::vec;
use alloc::vec::Vec;

/// What a font does not hold, or holds in a form this module does not take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unreadable(pub &'static str);

type R<T> = Result<T, Unreadable>;

fn u16_at(d: &[u8], at: usize) -> R<u16> {
    d.get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or(Unreadable("a table ends early"))
}

fn i16_at(d: &[u8], at: usize) -> R<i16> {
    u16_at(d, at).map(|v| v as i16)
}

fn u32_at(d: &[u8], at: usize) -> R<u32> {
    d.get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(Unreadable("a table ends early"))
}

/// An OpenType font's tables, and what a PDF's font descriptor and widths
/// take from them.
pub struct OpenType<'a> {
    data: &'a [u8],
    tables: Vec<([u8; 4], usize, usize)>,
}

/// A font descriptor's numbers, in the font's units scaled to 1,000.
#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub units_per_em: u16,
    pub bbox: [i32; 4],
    pub ascent: i32,
    pub descent: i32,
    pub cap_height: i32,
    pub italic_angle: f64,
    pub fixed_pitch: bool,
    /// Each glyph's advance, in the font's units.
    pub advances: Vec<u16>,
}

impl<'a> OpenType<'a> {
    pub fn new(data: &'a [u8]) -> R<OpenType<'a>> {
        if u32_at(data, 0)? != 0x4f54_544f {
            return Err(Unreadable("not an OpenType font with CFF outlines"));
        }
        let count = usize::from(u16_at(data, 4)?);
        let mut tables = Vec::with_capacity(count);
        for i in 0..count {
            let at = 12 + 16 * i;
            let tag = data
                .get(at..at + 4)
                .ok_or(Unreadable("the table directory ends early"))?;
            let (offset, length) = (
                u32_at(data, at + 8)? as usize,
                u32_at(data, at + 12)? as usize,
            );
            if offset
                .checked_add(length)
                .is_none_or(|end| end > data.len())
            {
                return Err(Unreadable("a table past the font's end"));
            }
            tables.push(([tag[0], tag[1], tag[2], tag[3]], offset, length));
        }
        Ok(OpenType { data, tables })
    }

    pub fn table(&self, tag: &[u8; 4]) -> R<&'a [u8]> {
        self.tables
            .iter()
            .find(|t| &t.0 == tag)
            .map(|&(_, offset, length)| &self.data[offset..offset + length])
            .ok_or(Unreadable("a table the PDF needs is missing"))
    }

    pub fn metrics(&self) -> R<Metrics> {
        let head = self.table(b"head")?;
        let hhea = self.table(b"hhea")?;
        let hmtx = self.table(b"hmtx")?;
        let post = self.table(b"post")?;
        let units_per_em = u16_at(head, 18)?;
        let bbox = [36, 38, 40, 42].map(|at| i16_at(head, at).map(i32::from));
        let [a, b, c, d] = bbox;
        let metrics = u16_at(hhea, 34)?;
        let mut advances = Vec::with_capacity(usize::from(metrics));
        for i in 0..usize::from(metrics) {
            advances.push(u16_at(hmtx, 4 * i)?);
        }
        let cap_height = match self.table(b"OS/2") {
            Ok(os2) if u16_at(os2, 0)? >= 2 => i32::from(i16_at(os2, 88)?),
            _ => i32::from(i16_at(hhea, 4)?),
        };
        Ok(Metrics {
            units_per_em,
            bbox: [a?, b?, c?, d?],
            ascent: i32::from(i16_at(hhea, 4)?),
            descent: i32::from(i16_at(hhea, 6)?),
            cap_height,
            italic_angle: f64::from(u32_at(post, 4)? as i32) / 65536.0,
            fixed_pitch: u32_at(post, 12)? != 0,
            advances,
        })
    }
}

impl Metrics {
    /// A glyph's advance; the last metric's for a glyph past them, as
    /// `hmtx` gives it.
    pub fn advance(&self, gid: u16) -> u16 {
        let i = usize::from(gid).min(self.advances.len().saturating_sub(1));
        self.advances.get(i).copied().unwrap_or(0)
    }
}

/// A CFF INDEX: where its items are, and where it ends.
struct Index {
    items: Vec<(usize, usize)>,
    end: usize,
}

fn index(d: &[u8], at: usize) -> R<Index> {
    let count = usize::from(u16_at(d, at)?);
    if count == 0 {
        return Ok(Index {
            items: Vec::new(),
            end: at + 2,
        });
    }
    let size = usize::from(*d.get(at + 2).ok_or(Unreadable("an INDEX ends early"))?);
    if !(1..=4).contains(&size) {
        return Err(Unreadable("an INDEX's offset size"));
    }
    let offset = |i: usize| -> R<usize> {
        let o = at + 3 + i * size;
        let bytes = d
            .get(o..o + size)
            .ok_or(Unreadable("an INDEX ends early"))?;
        Ok(bytes.iter().fold(0, |n, &b| (n << 8) | usize::from(b)))
    };
    let base = at + 2 + (count + 1) * size;
    let mut items = Vec::with_capacity(count);
    let mut start = offset(0)?;
    for i in 1..=count {
        let end = offset(i)?;
        if end < start || base + end > d.len() {
            return Err(Unreadable("an INDEX's offsets"));
        }
        items.push((base + start, base + end));
        start = end;
    }
    Ok(Index {
        items,
        end: base + start,
    })
}

/// An INDEX of `items`, with four-byte offsets when they need it.
fn write_index(out: &mut Vec<u8>, items: &[&[u8]]) {
    out.extend_from_slice(&(items.len() as u16).to_be_bytes());
    if items.is_empty() {
        return;
    }
    let total: usize = items.iter().map(|i| i.len()).sum::<usize>() + 1;
    let size = match total {
        0..=0xff => 1,
        0x100..=0xffff => 2,
        0x1_0000..=0xff_ffff => 3,
        _ => 4,
    };
    out.push(size as u8);
    let mut offset = 1usize;
    let put =
        |out: &mut Vec<u8>, o: usize| out.extend_from_slice(&(o as u32).to_be_bytes()[4 - size..]);
    put(out, offset);
    for item in items {
        offset += item.len();
        put(out, offset);
    }
    for item in items {
        out.extend_from_slice(item);
    }
}

fn index_len(items: &[&[u8]]) -> usize {
    let data: usize = items.iter().map(|i| i.len()).sum();
    let size = match data + 1 {
        0..=0xff => 1,
        0x100..=0xffff => 2,
        0x1_0000..=0xff_ffff => 3,
        _ => 4,
    };
    if items.is_empty() {
        2
    } else {
        3 + (items.len() + 1) * size + data
    }
}

fn byte(d: &[u8], at: usize) -> R<u8> {
    d.get(at)
        .copied()
        .ok_or(Unreadable("a DICT or charstring ends early"))
}

/// A DICT's entries: each operator (12 x as 1200 + x) with its operands'
/// bytes and their first integer.
struct Entry {
    op: u16,
    operands: Vec<u8>,
    ints: Vec<i64>,
}

fn dict(d: &[u8]) -> R<Vec<Entry>> {
    let mut entries = Vec::new();
    let (mut at, mut start) = (0, 0);
    let mut ints = Vec::new();
    while at < d.len() {
        let b0 = d[at];
        let get = |i: usize| byte(d, at + i);
        match b0 {
            0..=21 => {
                let op = if b0 == 12 {
                    1200 + u16::from(get(1)?)
                } else {
                    u16::from(b0)
                };
                let operands = d[start..at].to_vec();
                at += if b0 == 12 { 2 } else { 1 };
                start = at;
                entries.push(Entry {
                    op,
                    operands,
                    ints: core::mem::take(&mut ints),
                });
            }
            28 => {
                ints.push(i64::from(i16::from_be_bytes([get(1)?, get(2)?])));
                at += 3;
            }
            29 => {
                ints.push(i64::from(i32::from_be_bytes([
                    get(1)?,
                    get(2)?,
                    get(3)?,
                    get(4)?,
                ])));
                at += 5;
            }
            30 => {
                // A real: nibbles to the one that ends it.
                at += 1;
                loop {
                    let b = byte(d, at)?;
                    at += 1;
                    if b & 0x0f == 0x0f || b >> 4 == 0x0f {
                        break;
                    }
                }
                ints.push(0);
            }
            32..=246 => {
                ints.push(i64::from(b0) - 139);
                at += 1;
            }
            247..=250 => {
                ints.push((i64::from(b0) - 247) * 256 + i64::from(get(1)?) + 108);
                at += 2;
            }
            251..=254 => {
                ints.push(-(i64::from(b0) - 251) * 256 - i64::from(get(1)?) - 108);
                at += 2;
            }
            _ => return Err(Unreadable("a DICT's operand")),
        }
    }
    Ok(entries)
}

/// An integer in a DICT's five-byte form, so that offsets have their size
/// before they are known.
fn int5(out: &mut Vec<u8>, v: usize) {
    out.push(29);
    out.extend_from_slice(&(v as i32).to_be_bytes());
}

fn op(out: &mut Vec<u8>, op: u16) {
    if op >= 1200 {
        out.extend_from_slice(&[12, (op - 1200) as u8]);
    } else {
        out.push(op as u8);
    }
}

/// The subroutine bias of Type 2 charstrings.
fn bias(count: usize) -> i64 {
    match count {
        0..1240 => 107,
        1240..33900 => 1131,
        _ => 32768,
    }
}

/// A walk of Type 2 charstrings for the subroutines they call: the
/// operands counted and the numbers kept for `callsubr`, the stems counted
/// for `hintmask`'s bytes.
struct Scan<'d> {
    data: &'d [u8],
    global: &'d [(usize, usize)],
    local: &'d [(usize, usize)],
    used_global: Vec<bool>,
    used_local: Vec<bool>,
    stack: Vec<i64>,
    stems: usize,
    /// Whether a charstring ended with `endchar`'s four operands of `seac`,
    /// which a subset would have to follow to the accent's glyphs.
    seac: bool,
}

impl Scan<'_> {
    /// Walks `range`; true once `endchar` ends the glyph.
    fn walk(&mut self, (start, end): (usize, usize), depth: u8) -> R<bool> {
        if depth > 10 {
            return Err(Unreadable("subroutines nested past the limit"));
        }
        let d = &self.data[start..end];
        let mut at = 0;
        while at < d.len() {
            let b0 = d[at];
            let get = |i: usize| byte(d, at + i);
            match b0 {
                28 => {
                    self.stack
                        .push(i64::from(i16::from_be_bytes([get(1)?, get(2)?])));
                    at += 3;
                }
                32..=246 => {
                    self.stack.push(i64::from(b0) - 139);
                    at += 1;
                }
                247..=250 => {
                    self.stack
                        .push((i64::from(b0) - 247) * 256 + i64::from(get(1)?) + 108);
                    at += 2;
                }
                251..=254 => {
                    self.stack
                        .push(-(i64::from(b0) - 251) * 256 - i64::from(get(1)?) - 108);
                    at += 2;
                }
                255 => {
                    let fixed = i32::from_be_bytes([get(1)?, get(2)?, get(3)?, get(4)?]);
                    self.stack.push(i64::from(fixed >> 16));
                    at += 5;
                }
                1 | 3 | 18 | 23 => {
                    self.stems += self.stack.len() / 2;
                    self.stack.clear();
                    at += 1;
                }
                19 | 20 => {
                    self.stems += self.stack.len() / 2;
                    self.stack.clear();
                    at += 1 + self.stems.div_ceil(8);
                }
                10 | 29 => {
                    let local = b0 == 10;
                    let subrs = if local { self.local } else { self.global };
                    let n = self
                        .stack
                        .pop()
                        .ok_or(Unreadable("callsubr without its number"))?;
                    let i = usize::try_from(n + bias(subrs.len()))
                        .ok()
                        .filter(|&i| i < subrs.len())
                        .ok_or(Unreadable("callsubr past the subroutines"))?;
                    let range = subrs[i];
                    if local {
                        self.used_local[i] = true;
                    } else {
                        self.used_global[i] = true;
                    }
                    at += 1;
                    if self.walk(range, depth + 1)? {
                        return Ok(true);
                    }
                }
                11 => return Ok(false),
                14 => {
                    if self.stack.len() >= 4 {
                        self.seac = true;
                    }
                    return Ok(true);
                }
                12 => {
                    self.stack.clear();
                    at += 2;
                }
                _ => {
                    self.stack.clear();
                    at += 1;
                }
            }
        }
        Ok(false)
    }
}

/// A CFF table read: the pieces a subset copies or writes again.
pub struct Cff<'a> {
    data: &'a [u8],
    name: Index,
    top: Vec<Entry>,
    strings: Index,
    global: Index,
    charstrings: Index,
    charset: Option<(usize, usize)>,
    private: Vec<Entry>,
    local: Index,
}

impl<'a> Cff<'a> {
    pub fn new(data: &'a [u8]) -> R<Cff<'a>> {
        if data.first() != Some(&1) {
            return Err(Unreadable("a CFF of another major version"));
        }
        let header = usize::from(*data.get(2).ok_or(Unreadable("a CFF without its header"))?);
        let name = index(data, header)?;
        let tops = index(data, name.end)?;
        let &(top_start, top_end) = tops
            .items
            .first()
            .ok_or(Unreadable("a CFF without a font"))?;
        let top = dict(&data[top_start..top_end])?;
        let strings = index(data, tops.end)?;
        let global = index(data, strings.end)?;
        let int = |entries: &[Entry], op: u16, n: usize| {
            entries
                .iter()
                .find(|e| e.op == op)
                .and_then(|e| e.ints.get(n).copied())
        };
        if top.iter().any(|e| e.op == 1230) {
            return Err(Unreadable("a CID-keyed CFF"));
        }
        if int(&top, 1206, 0).is_some_and(|t| t != 2) {
            return Err(Unreadable("charstrings of a type other than 2"));
        }
        let at = |v: Option<i64>| v.and_then(|v| usize::try_from(v).ok());
        let charstrings = index(
            data,
            at(int(&top, 17, 0)).ok_or(Unreadable("a CFF without charstrings"))?,
        )?;
        let glyphs = charstrings.items.len();
        let charset = match at(int(&top, 15, 0)).unwrap_or(0) {
            0..=2 => None,
            offset => Some((offset, offset + charset_len(data, offset, glyphs)?)),
        };
        let (size, private_at) = (
            at(int(&top, 18, 0)).ok_or(Unreadable("a CFF without its Private DICT"))?,
            at(int(&top, 18, 1)).ok_or(Unreadable("a CFF without its Private DICT"))?,
        );
        let private = dict(
            data.get(private_at..private_at + size)
                .ok_or(Unreadable("the Private DICT"))?,
        )?;
        let local = match at(int(&private, 19, 0)) {
            Some(offset) => index(data, private_at + offset)?,
            None => Index {
                items: Vec::new(),
                end: 0,
            },
        };
        Ok(Cff {
            data,
            name,
            top,
            strings,
            global,
            charstrings,
            charset,
            private,
            local,
        })
    }

    pub fn glyph_count(&self) -> usize {
        self.charstrings.items.len()
    }

    /// A glyph's charstring.
    pub fn charstring(&self, gid: usize) -> Option<&'a [u8]> {
        self.charstrings
            .items
            .get(gid)
            .map(|&(s, e)| &self.data[s..e])
    }

    /// The global and local subroutines that are not one `return`.
    pub fn subroutines(&self) -> (usize, usize) {
        let live = |index: &Index| {
            index
                .items
                .iter()
                .filter(|&&(s, e)| self.data[s..e] != [11])
                .count()
        };
        (live(&self.global), live(&self.local))
    }

    /// Whether any charstring ends with `seac`'s operands.
    pub fn has_seac(&self) -> R<bool> {
        let mut scan = self.scan();
        for &range in &self.charstrings.items {
            scan.stack.clear();
            scan.stems = 0;
            scan.walk(range, 0)?;
        }
        Ok(scan.seac)
    }

    fn scan(&self) -> Scan<'_> {
        Scan {
            data: self.data,
            global: &self.global.items,
            local: &self.local.items,
            used_global: vec![false; self.global.items.len()],
            used_local: vec![false; self.local.items.len()],
            stack: Vec::new(),
            stems: 0,
            seac: false,
        }
    }

    /// The CFF with only `keep`'s charstrings, `.notdef`'s always, and the
    /// subroutines they call.
    pub fn subset(&self, keep: &BTreeSet<u16>) -> R<Vec<u8>> {
        let d = self.data;
        let mut scan = self.scan();
        let kept = |gid: usize| gid == 0 || u16::try_from(gid).is_ok_and(|g| keep.contains(&g));
        for (gid, &range) in self.charstrings.items.iter().enumerate() {
            if kept(gid) {
                scan.stack.clear();
                scan.stems = 0;
                scan.walk(range, 0)?;
            }
        }
        if scan.seac {
            return Err(Unreadable("a kept glyph is a seac accent"));
        }
        let slices =
            |index: &Index, used: &dyn Fn(usize) -> bool, empty: &'static [u8]| -> Vec<&[u8]> {
                let items = index.items.iter().enumerate();
                items
                    .map(|(i, &(s, e))| if used(i) { &d[s..e] } else { empty })
                    .collect()
            };
        let charstrings = slices(&self.charstrings, &kept, &[14]);
        let global = slices(&self.global, &|i| scan.used_global[i], &[11]);
        let local = slices(&self.local, &|i| scan.used_local[i], &[11]);
        let strings: Vec<&[u8]> = self.strings.items.iter().map(|&(s, e)| &d[s..e]).collect();
        let names: Vec<&[u8]> = self.name.items.iter().map(|&(s, e)| &d[s..e]).collect();

        // The Private DICT with its Subrs after it, at its own length.
        let mut private = Vec::new();
        for e in self.private.iter().filter(|e| e.op != 19) {
            private.extend_from_slice(&e.operands);
            op(&mut private, e.op);
        }
        if !local.is_empty() {
            let subrs = private.len() + 6;
            int5(&mut private, subrs);
            op(&mut private, 19);
        }
        // The Top DICT: its entries but the offsets, which follow at five bytes each, so
        // that its length is known before they are.
        let predefined = self
            .top
            .iter()
            .find(|e| e.op == 15 && self.charset.is_none());
        let top_of = |charset_at: usize, charstrings_at: usize, private_at: usize| {
            let mut top = Vec::new();
            for e in self
                .top
                .iter()
                .filter(|e| ![15, 16, 17, 18].contains(&e.op))
            {
                top.extend_from_slice(&e.operands);
                op(&mut top, e.op);
            }
            match (self.charset, predefined) {
                (Some(_), _) => {
                    int5(&mut top, charset_at);
                    op(&mut top, 15);
                }
                // A predefined charset keeps its number.
                (None, Some(e)) => {
                    top.extend_from_slice(&e.operands);
                    op(&mut top, 15);
                }
                (None, None) => {}
            }
            int5(&mut top, charstrings_at);
            op(&mut top, 17);
            int5(&mut top, private.len());
            int5(&mut top, private_at);
            op(&mut top, 18);
            top
        };
        let charset = self.charset.map(|(s, e)| &d[s..e]).unwrap_or(&[]);
        let top_len = top_of(0, 0, 0).len();
        let charset_at = 4
            + index_len(&names)
            + index_len(&[&vec![0; top_len]])
            + index_len(&strings)
            + index_len(&global);
        let charstrings_at = charset_at + charset.len();
        let private_at = charstrings_at + index_len(&charstrings);
        let top = top_of(charset_at, charstrings_at, private_at);
        let mut out = Vec::with_capacity(d.len());
        out.extend_from_slice(&[1, 0, 4, 4]);
        write_index(&mut out, &names);
        write_index(&mut out, &[&top]);
        write_index(&mut out, &strings);
        write_index(&mut out, &global);
        debug_assert_eq!(out.len(), charset_at);
        out.extend_from_slice(charset);
        write_index(&mut out, &charstrings);
        debug_assert_eq!(out.len(), private_at);
        out.extend_from_slice(&private);
        write_index(&mut out, &local);
        Ok(out)
    }
}

/// The bytes of a charset of `glyphs` glyphs at `at`.
fn charset_len(d: &[u8], at: usize, glyphs: usize) -> R<usize> {
    let format = *d.get(at).ok_or(Unreadable("the charset"))?;
    match format {
        0 => Ok(1 + 2 * glyphs.saturating_sub(1)),
        1 | 2 => {
            let (mut covered, mut len) = (1, 1);
            let width = if format == 1 { 1 } else { 2 };
            while covered < glyphs {
                let left = if format == 1 {
                    usize::from(*d.get(at + len + 2).ok_or(Unreadable("the charset"))?)
                } else {
                    usize::from(u16_at(d, at + len + 2)?)
                };
                covered += left + 1;
                len += 2 + width;
            }
            Ok(len)
        }
        _ => Err(Unreadable("a charset's format")),
    }
}
