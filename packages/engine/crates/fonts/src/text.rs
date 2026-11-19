//! The text packs (chapter 06 §5.3, W1): what layout measures and outlines
//! text with, one pack per URW style, as `tools/packs/text.mjs` writes them
//! from LilyPond's OTFs and describes their format. A [`TextFont`] holds each
//! glyph's advance, name, outline and ink box, the best `cmap`, the pairs of
//! GPOS's `kern` feature and the lookups of GSUB's `liga`, all in font units
//! and as HarfBuzz applies them to Latin text, which [`TextFont::shape`] does
//! (MS3 step 8). Since MS7 step 5 (E14, D262) a text pack holds each
//! glyph's ink box and no outline: the outlines are a pack of their own
//! (`LILO`), which [`TextFont::add_outlines`] reads when a host gives it,
//! once a skyline first needs one. [`TextFont::from_music`] makes one of an Emmentaler pack, as
//! Pango reads Emmentaler for text (`fetaText`), with the `kern` and GSUB's
//! `cv47`, `dlig`, `ss01` and `tnum` its pack holds (W6, D209), in
//! `tools/packs/opentype.mjs`' form, which [`OpenType`] reads.
use alloc::string::String;
use alloc::vec::Vec;

use crate::unicode::{Space, space_fallback};
use crate::{Error, Font, Outline, Outlines, Reader, Segment};

/// A glyph's metrics, in font units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextGlyph {
    /// hmtx's.
    pub advance: u32,
    /// x min, y min, x max, y max: the control box of what the glyph draws,
    /// as HarfBuzz's CFF extents count it; all zero when it draws nothing.
    pub ink: [i32; 4],
}

/// One ligature of a ligature substitution: `glyphs` in a row become `ligature`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ligature {
    pub glyphs: Vec<u16>,
    pub ligature: u16,
}

/// A GSUB lookup of the kinds the packs hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lookup {
    /// Type 1: each glyph by its substitute, by glyph.
    Single(Vec<(u16, u16)>),
    /// Type 4: the ligatures in the order they are tried.
    Ligature(Vec<Ligature>),
}

/// GPOS's `kern` and GSUB's lookups with the features that name them, as a
/// pack holds them (`tools/packs/opentype.mjs`).
#[derive(Clone, Debug, Default)]
pub(crate) struct OpenType {
    /// By first glyph, then second: (first << 16 | second, x advance).
    kerning: Vec<(u32, i32)>,
    /// In the font's lookup order.
    lookups: Vec<Lookup>,
    /// By tag: the positions of its lookups, ascending.
    features: Vec<(String, Vec<usize>)>,
}

impl OpenType {
    /// The form at `r`, its glyphs below `glyphs`.
    pub(crate) fn read(r: &mut Reader, glyphs: usize) -> Result<OpenType, Error> {
        let glyph = |n: u32| -> Result<u16, Error> {
            u16::try_from(n)
                .ok()
                .filter(|&g| usize::from(g) < glyphs)
                .ok_or(Error::Truncated)
        };
        // Each list is in ascending order: a delta is from the one before, less one.
        let next = |r: &mut Reader, previous: Option<u32>| -> Result<u32, Error> {
            let delta = r.u32()?;
            previous
                .map_or(Some(delta), |p| p.checked_add(delta)?.checked_add(1))
                .ok_or(Error::Truncated)
        };
        let mut layout = OpenType::default();
        let mut first = None;
        for _ in 0..r.usize()? {
            let a = next(r, first)?;
            first = Some(a);
            let mut second = None;
            for _ in 0..r.usize()? {
                let b = next(r, second)?;
                second = Some(b);
                let key = u32::from(glyph(a)?) << 16 | u32::from(glyph(b)?);
                layout.kerning.push((key, r.i32()?));
            }
        }
        for _ in 0..r.usize()? {
            let lookup = match r.u8()? {
                1 => {
                    let mut pairs = Vec::new();
                    let mut previous = None;
                    for _ in 0..r.usize()? {
                        let g = next(r, previous)?;
                        previous = Some(g);
                        let to = i64::from(g) + i64::from(r.i32()?);
                        let to = u32::try_from(to).map_err(|_| Error::Truncated)?;
                        pairs.push((glyph(g)?, glyph(to)?));
                    }
                    Lookup::Single(pairs)
                }
                4 => {
                    let mut ligatures = Vec::new();
                    for _ in 0..r.usize()? {
                        let n = r.usize()?;
                        let mut glyphs = Vec::with_capacity(n.min(r.bytes.len()));
                        for _ in 0..n {
                            glyphs.push(glyph(r.u32()?)?);
                        }
                        let ligature = glyph(r.u32()?)?;
                        ligatures.push(Ligature { glyphs, ligature });
                    }
                    Lookup::Ligature(ligatures)
                }
                _ => return Err(Error::Truncated),
            };
            layout.lookups.push(lookup);
        }
        for _ in 0..r.usize()? {
            let tag = r.string()?;
            let mut positions = Vec::new();
            let mut previous = None;
            for _ in 0..r.usize()? {
                let p = next(r, previous)?;
                previous = Some(p);
                let p = p as usize;
                if p >= layout.lookups.len() {
                    return Err(Error::Truncated);
                }
                positions.push(p);
            }
            layout.features.push((tag, positions));
        }
        Ok(layout)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.kerning.is_empty() && self.lookups.is_empty() && self.features.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub struct TextFont {
    name: String,
    family: String,
    style: String,
    weight: u32,
    italic: bool,
    units_per_em: u32,
    glyphs: Vec<TextGlyph>,
    names: Vec<String>,
    /// By character.
    cmap: Vec<(char, u16)>,
    layout: OpenType,
    /// Each glyph's outline in `outline_bytes`, once its outlines pack is
    /// given ([`TextFont::add_outlines`]); none for an Emmentaler face,
    /// whose [`Font`] has them.
    outlines: Vec<(usize, usize)>,
    outline_bytes: Vec<u8>,
}

impl TextFont {
    pub fn parse(pack: &[u8]) -> Result<TextFont, Error> {
        let mut r = Reader { bytes: pack, at: 0 };
        if r.take(4)? != b"LILT" {
            return Err(Error::NotAPack);
        }
        match r.u8()? {
            4 => {}
            version => return Err(Error::Version(version)),
        }
        let mut font = TextFont {
            name: r.string()?,
            family: r.string()?,
            style: r.string()?,
            weight: r.u32()?,
            italic: match r.u8()? {
                0 => false,
                1 => true,
                _ => return Err(Error::Truncated),
            },
            units_per_em: r.u32()?,
            ..TextFont::default()
        };

        let count = r.usize()?;
        if count > 1 << 16 {
            return Err(Error::Truncated);
        }
        font.glyphs = Vec::with_capacity(count.min(pack.len()));
        for _ in 0..count {
            font.glyphs.push(TextGlyph {
                advance: r.u32()?,
                ink: [0; 4],
            });
        }
        font.names = Vec::with_capacity(count.min(pack.len()));
        for _ in 0..count {
            font.names.push(r.string()?);
        }
        let glyph = |r: &mut Reader, previous: u16| -> Result<u16, Error> {
            let delta = r.i32()?;
            u16::try_from(i32::from(previous) + delta)
                .ok()
                .filter(|&g| usize::from(g) < count)
                .ok_or(Error::Truncated)
        };
        let (mut code, mut previous) = (None::<u32>, 0);
        for _ in 0..r.usize()? {
            let delta = r.u32()?;
            let c = code
                .map_or(Some(delta), |p| p.checked_add(delta)?.checked_add(1))
                .ok_or(Error::Truncated)?;
            code = Some(c);
            previous = glyph(&mut r, previous)?;
            font.cmap
                .push((char::from_u32(c).ok_or(Error::Truncated)?, previous));
        }
        font.layout = OpenType::read(&mut r, count)?;
        for glyph in &mut font.glyphs {
            let (x, y) = (r.i32()?, r.i32()?);
            let (width, height) = (r.u32()?, r.u32()?);
            let end = |at: i32, length: u32| {
                i32::try_from(length)
                    .ok()
                    .and_then(|l| at.checked_add(l))
                    .ok_or(Error::Truncated)
            };
            glyph.ink = [x, y, end(x, width)?, end(y, height)?];
        }
        if r.at != pack.len() {
            return Err(Error::Truncated);
        }
        Ok(font)
    }

    /// The PostScript name of the text face an outlines pack (`LILO`) is of.
    pub fn outlines_name(pack: &[u8]) -> Result<String, Error> {
        let mut r = Reader { bytes: pack, at: 0 };
        if r.take(4)? != b"LILO" {
            return Err(Error::NotAPack);
        }
        match r.u8()? {
            1 => r.string(),
            version => Err(Error::Version(version)),
        }
    }

    /// Whether its outlines were given ([`TextFont::add_outlines`]).
    pub fn has_outlines(&self) -> bool {
        !self.outlines.is_empty() || self.glyphs.is_empty()
    }

    /// Reads the outlines pack of this face: one of another face, of
    /// another count of glyphs, or whose outline of a glyph does not draw
    /// the ink box the text pack gave it, does not continue it. Given twice,
    /// the second changes nothing.
    pub fn add_outlines(&mut self, pack: &[u8]) -> Result<(), Error> {
        if TextFont::outlines_name(pack)? != self.name {
            return Err(Error::NotTheNextPart);
        }
        if self.has_outlines() {
            return Ok(());
        }
        let mut r = Reader { bytes: pack, at: 5 };
        r.string()?;
        if r.usize()? != self.glyphs.len() {
            return Err(Error::NotTheNextPart);
        }
        let mut bytes = Vec::new();
        let mut outlines = Vec::with_capacity(self.glyphs.len());
        for _ in 0..self.glyphs.len() {
            outlines.push(r.outline(&mut bytes)?);
        }
        if r.at != pack.len() {
            return Err(Error::Truncated);
        }
        let drawn = |&(start, end): &(usize, usize)| {
            ink_box(Outline::new(&bytes[start..end]).map_while(|s| s))
        };
        if outlines
            .iter()
            .zip(&self.glyphs)
            .any(|(o, g)| drawn(o) != g.ink)
        {
            return Err(Error::NotTheNextPart);
        }
        self.outlines = outlines;
        self.outline_bytes = bytes;
        Ok(())
    }

    /// The PostScript name: `C059-Roman`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `C059`, `Nimbus Sans`, `Nimbus Mono PS`.
    pub fn family(&self) -> &str {
        &self.family
    }

    /// `Roman`, `Regular`, `Bold`, `Italic`, `Bold Italic`.
    pub fn style(&self) -> &str {
        &self.style
    }

    /// OS/2's weight class: 400, 700.
    pub fn weight(&self) -> u32 {
        self.weight
    }

    pub fn italic(&self) -> bool {
        self.italic
    }

    pub fn units_per_em(&self) -> u32 {
        self.units_per_em
    }

    /// Every glyph, in the font's order.
    pub fn glyphs(&self) -> &[TextGlyph] {
        &self.glyphs
    }

    /// The name of glyph `index`, CFF's: `four`, `f_i`, `.notdef`.
    pub fn glyph_name(&self, index: u16) -> &str {
        self.names
            .get(usize::from(index))
            .map_or("", |n| n.as_str())
    }

    /// The glyph of a character; `None` for one the font has not, which the
    /// worker measures with the browser's font (06 §5.3).
    pub fn glyph(&self, c: char) -> Option<u16> {
        let at = self.cmap.binary_search_by_key(&c, |&(k, _)| k).ok()?;
        Some(self.cmap[at].1)
    }

    /// The characters the font has, ascending, with their glyphs.
    pub fn characters(&self) -> &[(char, u16)] {
        &self.cmap
    }

    /// The x advance `kern` adds to `left` before `right`; 0 for no pair.
    pub fn kerning(&self, left: u16, right: u16) -> i32 {
        let key = u32::from(left) << 16 | u32::from(right);
        let kerning = &self.layout.kerning;
        kerning
            .binary_search_by_key(&key, |&(k, _)| k)
            .map_or(0, |at| kerning[at].1)
    }

    /// The kerning pairs: first glyph, second glyph, x advance, in order.
    pub fn pairs(&self) -> impl Iterator<Item = (u16, u16, i32)> + '_ {
        self.layout
            .kerning
            .iter()
            .map(|&(key, value)| ((key >> 16) as u16, key as u16, value))
    }

    /// The lookups of GSUB's feature `tag` in the order they apply.
    pub fn lookups(&self, tag: &str) -> Vec<&Lookup> {
        self.layout
            .features
            .iter()
            .filter(|(t, _)| t == tag)
            .flat_map(|(_, positions)| positions.iter().map(|&p| &self.layout.lookups[p]))
            .collect()
    }

    /// The GSUB features the font has, by tag.
    pub fn features(&self) -> impl Iterator<Item = &str> + '_ {
        self.layout.features.iter().map(|(tag, _)| tag.as_str())
    }
}

impl Outlines for TextFont {
    /// Glyph `index`'s outline in font units; none for an Emmentaler face's
    /// glyph (its [`Font`]'s) or one the font has not.
    fn outline(&self, index: usize) -> impl Iterator<Item = Segment> + '_ {
        let (start, end) = self.outlines.get(index).copied().unwrap_or((0, 0));
        Outline::new(&self.outline_bytes[start..end]).map_while(|segment| segment)
    }
}

/// Which features apply: `kern`, and GSUB's by tag. As HarfBuzz has them,
/// `kern` and `liga` are on and any other off unless a `font-features`
/// string turns them off (`-kern`, `liga=0`) or on (`cv47`, `+ss01`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Features {
    pub kern: bool,
    /// The GSUB features on, by tag.
    gsub: Vec<String>,
}

impl Default for Features {
    fn default() -> Features {
        Features {
            kern: true,
            gsub: alloc::vec![String::from("liga")],
        }
    }
}

impl Features {
    /// Turns `tag` on or off: `kern`, or a GSUB feature.
    pub fn set(&mut self, tag: &str, on: bool) {
        if tag == "kern" {
            self.kern = on;
            return;
        }
        let at = self.gsub.binary_search_by(|t| t.as_str().cmp(tag));
        match (at, on) {
            (Err(at), true) => self.gsub.insert(at, String::from(tag)),
            (Ok(at), false) => {
                self.gsub.remove(at);
            }
            _ => {}
        }
    }

    /// Whether GSUB's `tag` is on.
    pub fn is_on(&self, tag: &str) -> bool {
        self.gsub.binary_search_by(|t| t.as_str().cmp(tag)).is_ok()
    }
}

/// A glyph of a shaped string, as HarfBuzz's buffer holds it after GSUB's
/// `liga` and GPOS's `kern`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shaped {
    /// `None` for a character the font has not, which Pango gives with
    /// `PANGO_GLYPH_UNKNOWN_FLAG` when no other font has it either.
    pub glyph: Option<u16>,
    /// The first character of its cluster.
    pub character: char,
    /// The byte of the text its cluster begins at.
    pub cluster: usize,
    /// hmtx's advance, in font units.
    pub advance: u32,
    /// What `kern` adds to the advance before the next glyph, in font units.
    pub kerning: i32,
    /// For a space character the font has not, drawn with its space: how
    /// HarfBuzz sets its advance ([`TextFont::fallback_advance`]).
    pub space: Option<Space>,
}

impl TextFont {
    /// `text` shaped as HarfBuzz shapes Latin text with what the pack holds:
    /// each character's glyph by the cmap, a cluster each, or for one the
    /// font has not as HarfBuzz's normalizing falls back ([`Self::nominal`]);
    /// then the lookups
    /// of every GSUB feature `features` turns on, together in the font's
    /// lookup order, each over the whole string: a single substitution
    /// replaces each glyph it has, a ligature substitution at each glyph the
    /// first ligature whose glyphs follow, which takes their clusters'
    /// first; then each glyph's `kern` pair with the next. HarfBuzz's
    /// normalization and its other default features (`ccmp`, `locl`, `calt`,
    /// `clig`) are not applied: the packs hold none (D209), nor its
    /// decompositions but U+2000's and U+2001's.
    pub fn shape(&self, text: &str, features: &Features) -> Vec<Shaped> {
        let mut out: Vec<Shaped> = text
            .char_indices()
            .map(|(at, c)| {
                let (glyph, space) = self.nominal(c);
                Shaped {
                    glyph,
                    character: c,
                    cluster: at,
                    advance: glyph.map_or(0, |g| self.advance(g)),
                    kerning: 0,
                    space,
                }
            })
            .collect();
        let mut on: Vec<usize> = self
            .layout
            .features
            .iter()
            .filter(|(tag, _)| features.is_on(tag))
            .flat_map(|(_, positions)| positions.iter().copied())
            .collect();
        on.sort_unstable();
        on.dedup();
        for lookup in on.into_iter().map(|p| &self.layout.lookups[p]) {
            match lookup {
                Lookup::Single(map) => {
                    for s in &mut out {
                        let Some(g) = s.glyph else { continue };
                        if let Ok(at) = map.binary_search_by_key(&g, |&(k, _)| k) {
                            s.glyph = Some(map[at].1);
                            s.advance = self.advance(map[at].1);
                        }
                    }
                }
                Lookup::Ligature(ligatures) => {
                    let mut i = 0;
                    while i < out.len() {
                        let found = ligatures.iter().find(|l| {
                            out.len() - i >= l.glyphs.len()
                                && l.glyphs
                                    .iter()
                                    .zip(&out[i..])
                                    .all(|(&g, s)| s.glyph == Some(g))
                        });
                        if let Some(l) = found {
                            out[i].glyph = Some(l.ligature);
                            out[i].advance = self.advance(l.ligature);
                            out.drain(i + 1..i + l.glyphs.len());
                        }
                        i += 1;
                    }
                }
            }
        }
        if features.kern {
            for i in 1..out.len() {
                if let (Some(a), Some(b)) = (out[i - 1].glyph, out[i].glyph) {
                    out[i - 1].kerning = self.kerning(a, b);
                }
            }
        }
        out
    }

    fn advance(&self, glyph: u16) -> u32 {
        self.glyphs.get(usize::from(glyph)).map_or(0, |g| g.advance)
    }

    /// HarfBuzz's `decompose_current_character` of `c` without its
    /// canonical decompositions but the spaces': its glyph; for one the font
    /// has not, U+2000's and U+2001's singletons (U+2002, U+2003); a space
    /// character the space's glyph, with its [`Space`]; U+2011 U+2010's.
    fn nominal(&self, c: char) -> (Option<u16>, Option<Space>) {
        if let Some(g) = self.glyph(c) {
            return (Some(g), None);
        }
        let singleton = match c {
            '\u{2000}' => self.glyph('\u{2002}'),
            '\u{2001}' => self.glyph('\u{2003}'),
            _ => None,
        };
        if singleton.is_some() {
            return (singleton, None);
        }
        if let Some(space) = space_fallback(c)
            && let Some(g) = self.glyph(' ')
        {
            return (Some(g), Some(space));
        }
        if c == '\u{2011}' {
            return (self.glyph('\u{2010}'), None);
        }
        (None, None)
    }

    /// `_hb_ot_shape_fallback_spaces`: the advance HarfBuzz gives a space
    /// it drew with the space's glyph, at `x_scale` to the em, where `em`
    /// scales an advance in font units as HarfBuzz does and `own` is the
    /// space's so scaled.
    pub fn fallback_advance(
        &self,
        space: Space,
        x_scale: i64,
        em: impl Fn(u32) -> i64,
        own: i64,
    ) -> i64 {
        let scaled = |g: Option<u16>| g.map_or(own, |g| em(self.advance(g)));
        match space {
            Space::Space => own,
            Space::Em(n) => (x_scale + n / 2) / n,
            Space::FourEm18 => x_scale * 4 / 18,
            Space::Figure => scaled(('0'..='9').find_map(|d| self.glyph(d))),
            Space::Punctuation => scaled(self.glyph('.').or_else(|| self.glyph(','))),
            Space::Narrow => own / 2,
        }
    }

    /// The text face of an Emmentaler pack, as Pango measures Emmentaler's
    /// text (`fetaText`): its glyphs' advances and names, their ink boxes
    /// from the outlines as HarfBuzz's CFF extents count them, the cmap the
    /// pack gives (each glyph's highest character), and its `kern` and GSUB
    /// features (`cv47`, `dlig`, `ss01`, `tnum`), which LilyPond's
    /// `\number`, `\dynamic`, `\fingering` and `\volta-number` turn on.
    /// The outlines stay the [`Font`]'s.
    pub fn from_music(font: &Font) -> TextFont {
        let mut cmap: Vec<(char, u16)> = font
            .glyphs()
            .iter()
            .enumerate()
            .filter_map(|(i, g)| Some((char::from_u32(g.charcode?)?, u16::try_from(i).ok()?)))
            .collect();
        cmap.sort_unstable();
        TextFont {
            name: String::from(font.postscript_name()),
            family: String::from(font.postscript_name()),
            style: String::new(),
            weight: 400,
            italic: false,
            units_per_em: font.units_per_em(),
            glyphs: (0..font.glyphs().len())
                .map(|i| TextGlyph {
                    advance: font.glyphs()[i].advance,
                    ink: ink_box(font.outline(i)),
                })
                .collect(),
            names: font.glyphs().iter().map(|g| g.name.clone()).collect(),
            cmap,
            layout: font.layout().clone(),
            outlines: Vec::new(),
            outline_bytes: Vec::new(),
        }
    }
}

/// `tools/packs/text.mjs`' `inkBox` of an outline: every point of a line or
/// curve, and the point a contour starts from once something is drawn from
/// it; all zero if nothing is drawn.
fn ink_box(outline: impl Iterator<Item = Segment>) -> [i32; 4] {
    let mut point = [0, 0];
    let mut open = false;
    let mut b: Option<[i32; 4]> = None;
    let mut add = |[x, y]: [i32; 2]| {
        b = Some(match b {
            None => [x, y, x, y],
            Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
        });
    };
    for segment in outline {
        let points = match segment {
            Segment::Move(p) => {
                point = p;
                open = false;
                continue;
            }
            Segment::Line(p) => [Some(p), None, None],
            Segment::Curve(a, c, p) => [Some(a), Some(c), Some(p)],
        };
        if !open {
            add(point);
            open = true;
        }
        for p in points.into_iter().flatten() {
            add(p);
            point = p;
        }
    }
    b.unwrap_or([0; 4])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Font;
    use alloc::vec;

    /// A style `T-Roman` by hand: glyphs 0 (500 wide, no outline), 1 (250,
    /// a line from (-10, -20) to (90, 280)) and 2 (750, none); `A` is 1 and
    /// `B` 2; the pair 1 2 kerned by -50; a ligature substitution of 1 1
    /// into 2, `liga`'s, then a single substitution of 2 by 1, `ss01`'s; the
    /// glyphs named `.notdef`, `A` and `A_A`; glyph 1's ink box.
    const PACK: &[u8] = b"LILT\x04\x07T-Roman\x01T\x05Roman\x90\x03\x00\xe8\x07\x03\xf4\x03\xfa\x01\xee\x05\x07.notdef\x01A\x03A_A\x02A\x02\x00\x02\x01\x01\x01\x02c\x02\x04\x01\x02\x01\x01\x02\x01\x01\x02\x01\x02\x04liga\x01\x00\x04ss01\x01\x01\x00\x00\x00\x00\x13'\x64\xac\x02\x00\x00\x00\x00";
    /// Its outlines (`LILO`): glyph 1's line, none for 0 and 2.
    const OUTLINES: &[u8] = b"LILO\x01\x07T-Roman\x03\x00\x08\x00\x13'\x01\xc8\x01\xd8\x04\x00";

    #[test]
    fn a_pack_reads_back_into_its_metrics() {
        let font = TextFont::parse(PACK).unwrap();
        assert_eq!(
            (font.name(), font.family(), font.style()),
            ("T-Roman", "T", "Roman")
        );
        assert_eq!(
            (font.weight(), font.italic(), font.units_per_em()),
            (400, false, 1000)
        );
        assert_eq!(
            font.glyphs(),
            &[
                TextGlyph {
                    advance: 500,
                    ink: [0; 4]
                },
                TextGlyph {
                    advance: 250,
                    ink: [-10, -20, 90, 280]
                },
                TextGlyph {
                    advance: 750,
                    ink: [0; 4]
                },
            ]
        );
        assert_eq!(
            [0, 1, 2, 3].map(|g| font.glyph_name(g)),
            [".notdef", "A", "A_A", ""]
        );
        assert_eq!(font.characters(), &[('A', 1), ('B', 2)]);
        assert_eq!(
            (font.glyph('A'), font.glyph('B'), font.glyph('C')),
            (Some(1), Some(2), None)
        );
        assert_eq!((font.kerning(1, 2), font.kerning(2, 1)), (-50, 0));
        assert_eq!(font.pairs().collect::<Vec<_>>(), vec![(1, 2, -50)]);
        assert_eq!(
            font.lookups("liga"),
            [&Lookup::Ligature(vec![Ligature {
                glyphs: vec![1, 1],
                ligature: 2
            }])]
        );
        assert_eq!(font.lookups("ss01"), [&Lookup::Single(vec![(2, 1)])]);
        assert_eq!(font.features().collect::<Vec<_>>(), ["liga", "ss01"]);
        assert!(!font.has_outlines());
        assert_eq!(font.outline(1).count(), 0);
        let mut font = font;
        assert_eq!(TextFont::outlines_name(OUTLINES).unwrap(), "T-Roman");
        font.add_outlines(OUTLINES).unwrap();
        assert!(font.has_outlines());
        font.add_outlines(OUTLINES).unwrap();
        assert_eq!(
            font.outline(1).collect::<Vec<_>>(),
            vec![Segment::Move([-10, -20]), Segment::Line([90, 280])]
        );
        assert_eq!(font.outline(2).count(), 0);
    }

    #[test]
    fn what_is_not_a_whole_text_pack_is_an_error() {
        assert_eq!(TextFont::parse(b"LILP\x01").unwrap_err(), Error::NotAPack);
        assert_eq!(Font::parse(PACK).unwrap_err(), Error::NotAPack);
        assert_eq!(TextFont::parse(b"LILT\x02").unwrap_err(), Error::Version(2));
        assert_eq!(TextFont::parse(b"LILT\x03").unwrap_err(), Error::Version(3));
        for n in 4..PACK.len() {
            assert_eq!(
                TextFont::parse(&PACK[..n]).unwrap_err(),
                Error::Truncated,
                "{n} bytes"
            );
        }
        let mut long = PACK.to_vec();
        long.push(0);
        assert_eq!(TextFont::parse(&long).unwrap_err(), Error::Truncated);
        // A ligature into glyph 3 of 3.
        let mut beyond = PACK.to_vec();
        let at = PACK
            .windows(4)
            .position(|w| w == b"\x01\x01\x02\x01")
            .unwrap()
            + 2;
        beyond[at] = 3;
        assert_eq!(TextFont::parse(&beyond).unwrap_err(), Error::Truncated);
    }

    #[test]
    fn outlines_continue_only_their_own_face() {
        let font = || TextFont::parse(PACK).unwrap();
        assert_eq!(font().add_outlines(PACK).unwrap_err(), Error::NotAPack);
        assert_eq!(
            font().add_outlines(b"LILO\x02").unwrap_err(),
            Error::Version(2)
        );
        let other = OUTLINES
            .to_vec()
            .iter()
            .map(|&b| if b == b'R' { b'S' } else { b })
            .collect::<Vec<_>>();
        assert_eq!(
            font().add_outlines(&other).unwrap_err(),
            Error::NotTheNextPart
        );
        // Two glyphs, not three.
        let mut fewer = OUTLINES.to_vec();
        fewer[13] = 2;
        assert_eq!(
            font().add_outlines(&fewer).unwrap_err(),
            Error::NotTheNextPart
        );
        // Glyph 1's line ends at (90, 281): not the ink box the pack gave.
        let mut moved = OUTLINES.to_vec();
        let at = moved.len() - 3;
        moved[at] = 0xda;
        assert_eq!(
            font().add_outlines(&moved).unwrap_err(),
            Error::NotTheNextPart
        );
        for n in 13..OUTLINES.len() {
            assert!(font().add_outlines(&OUTLINES[..n]).is_err(), "{n} bytes");
        }
        let mut f = font();
        assert!(f.add_outlines(&OUTLINES[..OUTLINES.len() - 1]).is_err());
        assert!(!f.has_outlines());
    }

    /// The glyph, cluster, advance and kerning of each shaped glyph.
    fn shaped(
        font: &TextFont,
        text: &str,
        features: &Features,
    ) -> Vec<(Option<u16>, usize, u32, i32)> {
        font.shape(text, features)
            .iter()
            .map(|s| (s.glyph, s.cluster, s.advance, s.kerning))
            .collect()
    }

    #[test]
    fn a_string_shapes_by_the_cmap_then_liga_then_kern() {
        let font = TextFont::parse(PACK).unwrap();
        let on = &Features::default();
        assert_eq!(
            shaped(&font, "AB", on),
            vec![(Some(1), 0, 250, -50), (Some(2), 1, 750, 0)]
        );
        // A A becomes A_A, whose cluster is the first A's; the third A
        // stays, and kerns with B.
        assert_eq!(
            shaped(&font, "AAAB", on),
            vec![
                (Some(2), 0, 750, 0),
                (Some(1), 2, 250, -50),
                (Some(2), 3, 750, 0)
            ]
        );
        let mut off = Features::default();
        off.set("liga", false);
        off.set("kern", false);
        assert_eq!(
            shaped(&font, "AAB", &off),
            vec![
                (Some(1), 0, 250, 0),
                (Some(1), 1, 250, 0),
                (Some(2), 2, 750, 0)
            ]
        );
        // A character the font has not: no glyph, no ligature or kerning
        // across it, its cluster by bytes.
        assert_eq!(
            shaped(&font, "AéA", on),
            vec![(Some(1), 0, 250, 0), (None, 1, 0, 0), (Some(1), 3, 250, 0)]
        );
        // ss01 on: its lookup after liga's, as the font orders them, so A A
        // becomes A_A and then A, as B does.
        let mut ss01 = Features::default();
        ss01.set("ss01", true);
        assert!(ss01.is_on("ss01") && ss01.is_on("liga") && !on.is_on("ss01"));
        assert_eq!(
            shaped(&font, "AAB", &ss01),
            vec![(Some(1), 0, 250, 0), (Some(1), 2, 250, 0)]
        );
    }
}
