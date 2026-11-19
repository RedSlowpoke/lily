//! The engine's music fonts (chapter 06 §5.1, W1). A [`Font`] is read from the
//! packs `tools/packs/build.mjs` writes from LilyPond's Emmentaler OTFs, one
//! per optical size and four for the braces, whose format that file describes:
//! the font's PostScript name, the glyph names in the font's order, each
//! glyph's advance, character, `LILC` entry and outline, the `LILY` table,
//! and the OpenType layout Pango shapes its text with (D209). Extents are
//! LilyPond's own, bit for bit:
//! [`Font::char_dimensions`] is `Open_type_font::get_indexed_char_dimensions`
//! and [`Font::find_by_name`] the extent of `Font_metric::find_by_name`'s
//! stencil, which `ly:font-get-glyph` returns. The text fonts' metric packs
//! are [`text`]'s, and the Unicode data their text is shaped with
//! [`unicode`]'s.
#![no_std]
extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

mod names;
pub mod text;
pub mod unicode;
use text::OpenType;
pub use text::{Features, Ligature, Lookup, Shaped, TextFont, TextGlyph};

// dimensions.hh, in LilyPond's internal unit, the millimetre.
const INCH_TO_PT: f64 = 72.270;
const CM_TO_PT: f64 = INCH_TO_PT / 2.54;
const MM_TO_PT: f64 = CM_TO_PT / 10.0;
const PT_TO_PT: f64 = 1.0;
const PT_TO_MM: f64 = 1.0 / MM_TO_PT;
/// `point_constant`: one point in LilyPond's internal unit.
pub const POINT: f64 = 1.0 * (PT_TO_PT * PT_TO_MM);

/// LilyPond's `Interval`: from `left` to `right`, empty when `left > right`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub left: f64,
    pub right: f64,
}

impl Interval {
    /// What `Interval ()` constructs, and `ly:stencil-extent` of an empty stencil.
    pub const EMPTY: Interval = Interval {
        left: f64::INFINITY,
        right: f64::NEG_INFINITY,
    };

    pub fn new(left: f64, right: f64) -> Interval {
        Interval { left, right }
    }

    pub fn is_empty(self) -> bool {
        self.left > self.right
    }

    /// `Interval *= s`: both ends times `s`, swapped when `s` is negative.
    pub fn scaled(self, s: f64) -> Interval {
        let (left, right) = (self.left * s, self.right * s);
        if s < 0.0 {
            Interval::new(right, left)
        } else {
            Interval::new(left, right)
        }
    }
}

/// LilyPond's `Box`: an interval on each axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box {
    pub x: Interval,
    pub y: Interval,
}

impl Box {
    pub const EMPTY: Box = Box {
        x: Interval::EMPTY,
        y: Interval::EMPTY,
    };

    /// `Box::scale`: each axis times `s`, as a `Modified_font_metric`
    /// scales its font's extents by its magnification.
    pub fn scaled(self, s: f64) -> Box {
        Box {
            x: self.x.scaled(s),
            y: self.y.scaled(s),
        }
    }
}

/// A glyph's `LILC` entry, the numbers as Guile reads them from the table's
/// text: points at the font's design size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// `bbox`: x min, y min, x max, y max.
    pub bbox: [f64; 4],
    pub attachment: Option<[f64; 2]>,
    pub attachment_down: Option<[f64; 2]>,
    pub ledger_shortening_range: Option<[f64; 2]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub name: String,
    /// In font units (hmtx).
    pub advance: u32,
    /// The character the font's Unicode cmap maps to it, the highest of
    /// several (`make_index_to_charcode_map`); `None` for a glyph none maps.
    pub charcode: Option<u32>,
    /// `None` for the glyphs `LILC` leaves out (`.notdef`, `backslash`).
    pub metrics: Option<Metrics>,
    /// Its outline's bytes in [`Font`]'s.
    outline: (usize, usize),
}

/// A piece of an outline, in font units; a contour begins at each `Move` and
/// is closed at the next or at the end, as CFF closes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment {
    Move([i32; 2]),
    Line([i32; 2]),
    Curve([i32; 2], [i32; 2], [i32; 2]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// It does not begin with `LILP` (a text pack: `LILT`).
    NotAPack,
    /// A version of the format this reader does not know.
    Version(u8),
    /// A field runs past the end, or a number or an outline is malformed.
    Truncated,
    /// A part that does not continue the font: another font, or not the next glyphs.
    NotTheNextPart,
}

#[derive(Clone, Debug, Default)]
pub struct Font {
    name: String,
    postscript_name: String,
    units_per_em: u32,
    global: Vec<(String, f64)>,
    glyphs: Vec<Glyph>,
    outlines: Vec<u8>,
    by_name: BTreeMap<String, usize>,
    /// GPOS's `kern` and GSUB's features, the first pack's: a part has none.
    layout: OpenType,
}

/// What has glyph outlines in the packs' encoding: an Emmentaler [`Font`],
/// a [`TextFont`] (D209), which the skylines read alike.
pub trait Outlines {
    /// Glyph `index`'s outline in font units.
    fn outline(&self, index: usize) -> impl Iterator<Item = Segment> + '_;
}

impl Outlines for Font {
    fn outline(&self, index: usize) -> impl Iterator<Item = Segment> + '_ {
        Font::outline(self, index)
    }
}

impl Font {
    /// The font of a pack whose glyphs begin with the font's first.
    pub fn parse(pack: &[u8]) -> Result<Font, Error> {
        let mut font = Font::default();
        font.read(pack, true)?;
        Ok(font)
    }

    /// The name of the font a pack is of, which [`Font::parse`] or, for a
    /// part that continues it, [`Font::add`] reads.
    pub fn pack_name(pack: &[u8]) -> Result<String, Error> {
        let mut r = Reader { bytes: pack, at: 0 };
        if r.take(4)? != b"LILP" {
            return Err(Error::NotAPack);
        }
        match r.u8()? {
            3 => r.string(),
            version => Err(Error::Version(version)),
        }
    }

    /// Adds a pack that continues the font, as emmentaler-brace's parts do.
    pub fn add(&mut self, pack: &[u8]) -> Result<(), Error> {
        self.read(pack, false)
    }

    fn read(&mut self, pack: &[u8], first: bool) -> Result<(), Error> {
        let mut r = Reader { bytes: pack, at: 0 };
        if r.take(4)? != b"LILP" {
            return Err(Error::NotAPack);
        }
        match r.u8()? {
            3 => {}
            version => return Err(Error::Version(version)),
        }
        let name = r.string()?;
        let postscript_name = r.string()?;
        let start = r.usize()?;
        let units_per_em = r.u32()?;
        let digits = r.u8()?;
        if first {
            if start != 0 {
                return Err(Error::NotTheNextPart);
            }
            self.name = name;
            self.postscript_name = postscript_name;
            self.units_per_em = units_per_em;
        } else if name != self.name
            || postscript_name != self.postscript_name
            || units_per_em != self.units_per_em
            || start != self.glyphs.len()
        {
            return Err(Error::NotTheNextPart);
        }
        let mut global = Vec::new();
        for _ in 0..r.usize()? {
            let key = r.string()?;
            let digits = r.u8()?;
            global.push((key, r.decimal(digits)?));
        }
        if first {
            self.global = global;
        }

        let count = r.usize()?;
        let mut glyphs = Vec::with_capacity(count.min(pack.len()));
        for _ in 0..count {
            glyphs.push(Glyph {
                name: r.string()?,
                advance: 0,
                charcode: None,
                metrics: None,
                outline: (0, 0),
            });
        }
        for glyph in &mut glyphs {
            glyph.advance = r.u32()?;
        }
        let mut next = 0i64;
        for glyph in &mut glyphs {
            let n = r.varint()?;
            if n == 0 {
                continue;
            }
            let n = n - 1;
            let code = next + ((n >> 1) as i64 ^ -((n & 1) as i64));
            let code = u32::try_from(code).map_err(|_| Error::Truncated)?;
            glyph.charcode = Some(code);
            next = i64::from(code) + 1;
        }
        for glyph in &mut glyphs {
            let flags = r.u8()?;
            if flags == 0 {
                continue;
            }
            // Every entry of LILC has a bbox.
            if flags & 1 == 0 || flags > 15 {
                return Err(Error::Truncated);
            }
            let mut bbox = [0.0; 4];
            for value in &mut bbox {
                *value = r.decimal(digits)?;
            }
            let mut pair = |bit: u8| -> Result<Option<[f64; 2]>, Error> {
                Ok(if flags & bit != 0 {
                    Some([r.decimal(digits)?, r.decimal(digits)?])
                } else {
                    None
                })
            };
            glyph.metrics = Some(Metrics {
                bbox,
                attachment: pair(2)?,
                attachment_down: pair(4)?,
                ledger_shortening_range: pair(8)?,
            });
        }
        for glyph in &mut glyphs {
            glyph.outline = r.outline(&mut self.outlines)?;
        }
        let layout = OpenType::read(&mut r, self.glyphs.len() + count)?;
        if r.at != pack.len() {
            return Err(Error::Truncated);
        }
        if first {
            self.layout = layout;
        } else if !layout.is_empty() {
            return Err(Error::NotTheNextPart);
        }
        for glyph in glyphs {
            self.by_name.insert(glyph.name.clone(), self.glyphs.len());
            self.glyphs.push(glyph);
        }
        Ok(())
    }

    /// The font's name, as LilyPond loads it: `emmentaler-20`, `emmentaler-brace`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its PostScript name, the CFF's, which `Open_type_font::font_name`
    /// gives: `Emmentaler-20`, `Emmentaler-Brace`.
    pub fn postscript_name(&self) -> &str {
        &self.postscript_name
    }

    pub fn units_per_em(&self) -> u32 {
        self.units_per_em
    }

    /// `Open_type_font::index_to_charcode`'s map: the glyph's character, if
    /// the cmap maps one to it.
    pub fn charcode(&self, index: usize) -> Option<u32> {
        self.glyphs.get(index)?.charcode
    }

    /// `Open_type_font::count`: how many glyphs the cmap maps a character to.
    pub fn count(&self) -> usize {
        self.glyphs.iter().filter(|g| g.charcode.is_some()).count()
    }

    /// The glyphs read so far, in the font's order (`ly:otf-glyph-list`).
    pub fn glyphs(&self) -> &[Glyph] {
        &self.glyphs
    }

    /// A value of the `LILY` table: `staff_space`, `blot_diameter`, `design_size`, ….
    pub fn global(&self, key: &str) -> Option<f64> {
        self.global
            .iter()
            .find(|(k, _)| k == key)
            .map(|&(_, value)| value)
    }

    /// `Open_type_font::design_size`: `LILY`'s `design_size`, 1 without one, in internal units.
    pub fn design_size(&self) -> f64 {
        self.global("design_size").unwrap_or(1.0) * POINT
    }

    /// `FT_Get_Name_Index`, whose 0 LilyPond takes for a missing glyph, so
    /// that `.notdef` is never found.
    pub fn name_to_index(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied().filter(|&index| index != 0)
    }

    /// The `LILC` entry of the glyph `name`, as LilyPond's character table
    /// keys it: by the name alone, glyph 0's too.
    pub fn metrics(&self, name: &str) -> Option<Metrics> {
        self.glyphs[*self.by_name.get(name)?].metrics
    }

    /// `get_indexed_char_dimensions`: the `LILC` bbox in internal units, or
    /// for a glyph without one, FreeType's unscaled box of its outline scaled
    /// by the design size per font unit.
    pub fn char_dimensions(&self, index: usize) -> Box {
        let glyph = &self.glyphs[index];
        if let Some(metrics) = glyph.metrics {
            let [x0, y0, x1, y1] = metrics.bbox;
            return Box {
                x: Interval::new(x0, x1),
                y: Interval::new(y0, y1),
            }
            .scaled(POINT);
        }
        self.control_box(index)
            .scaled(self.design_size() / self.units_per_em as f64)
    }

    /// `Font_metric::find_by_name`'s glyph and the extent of its stencil;
    /// `None` is the empty stencil of a glyph the font has not. A `-` in the
    /// name is read as `M` (`rests.M3` for `rests.-3`).
    pub fn find_by_name(&self, name: &str) -> Option<(usize, Box)> {
        let index = if name.contains('-') {
            self.name_to_index(&name.replace('-', "M"))
        } else {
            self.name_to_index(name)
        }?;
        Some((index, self.char_dimensions(index)))
    }

    /// GPOS's `kern` and GSUB's features, which [`TextFont::from_music`] shapes with.
    pub(crate) fn layout(&self) -> &OpenType {
        &self.layout
    }

    /// The glyph's outline in font units.
    pub fn outline(&self, index: usize) -> impl Iterator<Item = Segment> + '_ {
        let (start, end) = self.glyphs[index].outline;
        Outline::new(&self.outlines[start..end]).map_while(|segment| segment)
    }

    /// `ly_FT_get_unscaled_indexed_char_dimensions`: FreeType's metrics of the
    /// glyph loaded unscaled, the control box of its outline's points, all
    /// zero for an empty outline as `FT_Outline_Get_CBox` gives it.
    fn control_box(&self, index: usize) -> Box {
        let mut points = self.outline(index).flat_map(|segment| match segment {
            Segment::Move(p) | Segment::Line(p) => [Some(p), None, None],
            Segment::Curve(a, b, c) => [Some(a), Some(b), Some(c)],
        });
        let Some(first) = points.next().flatten() else {
            return Box {
                x: Interval::new(0.0, 0.0),
                y: Interval::new(0.0, 0.0),
            };
        };
        let (mut low, mut high) = (first, first);
        for [x, y] in points.flatten() {
            low = [low[0].min(x), low[1].min(y)];
            high = [high[0].max(x), high[1].max(y)];
        }
        Box {
            x: Interval::new(low[0] as f64, high[0] as f64),
            y: Interval::new(low[1] as f64, high[1] as f64),
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&end| end <= self.bytes.len());
        let bytes = &self.bytes[self.at..end.ok_or(Error::Truncated)?];
        self.at += n;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    /// An outline, appended to `outlines`: where it is in them. Checked once
    /// here, so that an outline read later cannot fail.
    fn outline(&mut self, outlines: &mut Vec<u8>) -> Result<(usize, usize), Error> {
        let length = self.usize()?;
        let bytes = self.take(length)?;
        if Outline::new(bytes).any(|segment| segment.is_none()) {
            return Err(Error::Truncated);
        }
        let start = outlines.len();
        outlines.extend_from_slice(bytes);
        Ok((start, outlines.len()))
    }

    fn varint(&mut self) -> Result<u64, Error> {
        varint(self.bytes, &mut self.at).ok_or(Error::Truncated)
    }

    fn u32(&mut self) -> Result<u32, Error> {
        u32::try_from(self.varint()?).map_err(|_| Error::Truncated)
    }

    fn usize(&mut self) -> Result<usize, Error> {
        usize::try_from(self.varint()?).map_err(|_| Error::Truncated)
    }

    /// A zigzag varint.
    fn i32(&mut self) -> Result<i32, Error> {
        let n = self.varint()?;
        i32::try_from((n >> 1) as i64 ^ -((n & 1) as i64)).map_err(|_| Error::Truncated)
    }

    fn string(&mut self) -> Result<String, Error> {
        let n = self.usize()?;
        let bytes = self.take(n)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| Error::Truncated)
    }

    /// `±magnitude / 10^digits`: for a magnitude below 2^53 and up to 22
    /// digits both are exact doubles, so the division rounds the decimal's
    /// value once, to the double Guile's reader makes of the same text.
    fn decimal(&mut self, digits: u8) -> Result<f64, Error> {
        let value = self.varint()?;
        let magnitude = value >> 1;
        if magnitude >= 1 << 53 || digits > 22 {
            return Err(Error::Truncated);
        }
        let mut scale = 1.0;
        for _ in 0..digits {
            scale *= 10.0;
        }
        let x = magnitude as f64 / scale;
        Ok(if value & 1 == 1 { -x } else { x })
    }
}

/// An unsigned LEB128 number at `*at`, at most 64 bits.
fn varint(bytes: &[u8], at: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *bytes.get(*at)?;
        *at += 1;
        value |= u64::from(byte & 0x7f).checked_shl(shift)?;
        if byte < 0x80 {
            return Some(value);
        }
    }
    None
}

/// An outline's segments, absolute; `None` once for malformed bytes.
pub(crate) struct Outline<'a> {
    bytes: &'a [u8],
    at: usize,
    point: [i32; 2],
    failed: bool,
}

impl<'a> Outline<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Outline<'a> {
        Outline {
            bytes,
            at: 0,
            point: [0, 0],
            failed: false,
        }
    }

    fn next_point(&mut self) -> Option<[i32; 2]> {
        let mut delta = || {
            let n = varint(self.bytes, &mut self.at)?;
            i32::try_from((n >> 1) as i64 ^ -((n & 1) as i64)).ok()
        };
        let (dx, dy) = (delta()?, delta()?);
        self.point = [
            self.point[0].checked_add(dx)?,
            self.point[1].checked_add(dy)?,
        ];
        Some(self.point)
    }

    fn segment(&mut self) -> Option<Segment> {
        let op = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(match op {
            0 => Segment::Move(self.next_point()?),
            1 => Segment::Line(self.next_point()?),
            2 => Segment::Curve(self.next_point()?, self.next_point()?, self.next_point()?),
            _ => return None,
        })
    }
}

impl Iterator for Outline<'_> {
    type Item = Option<Segment>;

    fn next(&mut self) -> Option<Option<Segment>> {
        if self.failed || self.at == self.bytes.len() {
            return None;
        }
        let segment = self.segment();
        self.failed = segment.is_none();
        Some(segment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Two glyphs by hand of a font whose PostScript name is `Test`: `a`,
    /// the character 97, with LILC's bbox (-0.000 -3.125 7.500 0.000) and
    /// ledger-shortening-range (-4.000 . 5.000) and an empty outline; `b`,
    /// which no character maps, without LILC, a move to (0, -4) and a line
    /// to (1, -3); no kerning or GSUB.
    const PACK: &[u8] = b"LILP\x03\x04test\x04Test\x00\xe8\x07\x03\x01\x0bdesign_size\x01\x90\x03\x02\x01a\x01b\xf4\x03\x00\xc3\x01\x00\x09\x01\xeb\x30\x98\x75\x00\xc1\x3e\x90\x4e\x00\x00\x06\x00\x00\x07\x01\x02\x02\x00\x00\x00";
    /// One glyph `a`, the character 2, and the part after it, `b`, the
    /// character 1.
    const ONE: &[u8] =
        b"LILP\x03\x01f\x01F\x00\xe8\x07\x00\x00\x01\x01a\x00\x05\x00\x00\x00\x00\x00";
    const NEXT: &[u8] =
        b"LILP\x03\x01f\x01F\x01\xe8\x07\x00\x00\x01\x01b\x00\x03\x00\x00\x00\x00\x00";

    #[test]
    fn a_pack_reads_back_into_its_glyphs() {
        let font = Font::parse(PACK).unwrap();
        assert_eq!((font.name(), font.postscript_name()), ("test", "Test"));
        assert_eq!(font.units_per_em(), 1000);
        assert_eq!(
            (font.charcode(0), font.charcode(1), font.charcode(2)),
            (Some(97), None, None)
        );
        assert_eq!(font.count(), 1);
        assert_eq!(font.global("design_size"), Some(20.0));
        assert_eq!(font.design_size(), 20.0 * POINT);
        let a = &font.glyphs()[0];
        assert_eq!((a.name.as_str(), a.advance), ("a", 500));
        let m = a.metrics.unwrap();
        // -0.000 stays negative, as Guile reads it.
        assert_eq!(
            m.bbox.map(f64::to_bits),
            [-0.0, -3.125, 7.5, 0.0].map(f64::to_bits)
        );
        assert_eq!(m.ledger_shortening_range, Some([-4.0, 5.0]));
        assert_eq!((m.attachment, m.attachment_down), (None, None));
        assert_eq!(font.glyphs()[1].metrics, None);
        assert_eq!(font.outline(0).count(), 0);
        assert_eq!(
            font.outline(1).collect::<Vec<_>>(),
            vec![Segment::Move([0, -4]), Segment::Line([1, -3])]
        );
    }

    #[test]
    fn extents_are_lilc_s_bbox_or_freetype_s_box_of_the_outline() {
        let font = Font::parse(PACK).unwrap();
        assert_eq!(
            font.char_dimensions(0),
            Box {
                x: Interval::new(-0.0 * POINT, 7.5 * POINT),
                y: Interval::new(-3.125 * POINT, 0.0),
            }
        );
        // Glyph 0 is FreeType's missing glyph.
        assert_eq!(font.find_by_name("a"), None);
        let s = 20.0 * POINT / 1000.0;
        let b = Box {
            x: Interval::new(0.0, s),
            y: Interval::new(-4.0 * s, -3.0 * s),
        };
        assert_eq!(font.find_by_name("b"), Some((1, b)));
        assert_eq!(font.find_by_name("c"), None);
    }

    #[test]
    fn a_part_must_continue_the_font() {
        let mut font = Font::parse(ONE).unwrap();
        assert_eq!(Font::parse(NEXT).unwrap_err(), Error::NotTheNextPart);
        let mut other = NEXT.to_vec();
        other[8] = b'G';
        assert_eq!(font.add(&other).unwrap_err(), Error::NotTheNextPart);
        font.add(NEXT).unwrap();
        assert_eq!(font.name_to_index("b"), Some(1));
        // Each part's characters are its own: b's is one past nothing.
        assert_eq!((font.charcode(0), font.charcode(1)), (Some(2), Some(1)));
        assert_eq!(font.add(NEXT).unwrap_err(), Error::NotTheNextPart);
        assert_eq!(font.add(PACK).unwrap_err(), Error::NotTheNextPart);
        // A part with a kerning pair of its own: a part holds no layout.
        let mut font = Font::parse(ONE).unwrap();
        let mut kerned = NEXT.to_vec();
        kerned.truncate(NEXT.len() - 3);
        kerned.extend([1, 0, 1, 1, 2, 0, 0]);
        assert_eq!(font.add(&kerned).unwrap_err(), Error::NotTheNextPart);
    }

    #[test]
    fn what_is_not_a_whole_pack_is_an_error() {
        assert_eq!(Font::parse(b"OTTO").unwrap_err(), Error::NotAPack);
        assert_eq!(Font::parse(b"LILP\x02").unwrap_err(), Error::Version(2));
        for n in 4..PACK.len() {
            assert_eq!(
                Font::parse(&PACK[..n]).unwrap_err(),
                Error::Truncated,
                "{n} bytes"
            );
        }
        let mut long = ONE.to_vec();
        long.push(0);
        assert_eq!(Font::parse(&long).unwrap_err(), Error::Truncated);
        // An outline with an op that is not 0, 1 or 2.
        let mut bad = ONE.to_vec();
        bad.truncate(ONE.len() - 4);
        bad.extend([1, 3, 0, 0, 0]);
        assert_eq!(Font::parse(&bad).unwrap_err(), Error::Truncated);
    }

    #[test]
    fn intervals_scale_as_lilypond_s() {
        assert_eq!(
            Interval::new(-1.0, 2.0).scaled(-2.0),
            Interval::new(-4.0, 2.0)
        );
        assert!(Interval::EMPTY.is_empty());
        assert!(Box::EMPTY.scaled(2.0).x.is_empty());
        assert!((POINT - 25.4 / 72.27).abs() < 1e-15);
    }
}
