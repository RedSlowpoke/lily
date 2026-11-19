//! The engine's PDF writer (chapter 06 §7, W7; D270): a book's display list
//! ([`Document`], `list.rs`) written as one PDF, as 06 §7.3 designs it.
//!
//! - Each page's primitives in its content stream, at the page's scale,
//!   with the SVG writer's round caps and joins (06 §6.2), so that a page
//!   rasterizes as the engine's own SVG page does.
//! - **Music glyphs as form XObjects**: each outline once, drawn with `Do`
//!   at every use, the PDF's `<use>`; no music font program.
//! - **Text as CID fonts with ToUnicode maps**: each text font the host
//!   gives (the URW OTFs, under their PDF exception) subset by `cff.rs` to
//!   the glyphs its runs draw, `Identity-H`, so a PDF's lyrics and titles can
//!   be selected and searched; a run in a font the host did not give is
//!   left out with a warning (06 §7.3's open gap of other scripts).
//! - The `\header` fields as the Info dictionary, as `handle-metadata`
//!   (framework-ps.scm) puts them; no point-and-click links (D5); every
//!   stream deflated.
//!
//! The writer needs nothing of the VM, so pdf.wasm is this crate alone with
//! `module` (`module.rs`); `stencils` (`stencil.rs`) is the core's walk of
//! a page's stencil into the display list.
#![no_std]
extern crate alloc;

pub mod cff;
pub mod list;
#[cfg(feature = "module")]
mod module;
#[cfg(feature = "stencils")]
pub mod stencil;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::f64::consts::PI;

pub use list::{Command, Document, Malformed, Op, Page, Segment, TextGlyph};

/// A text font the host gives: its PostScript name and its OTF.
#[derive(Clone, Copy, Debug)]
pub struct Font<'a> {
    pub name: &'a str,
    pub otf: &'a [u8],
}

/// A PDF written, and what could not be written into it.
#[derive(Clone, Debug, Default)]
pub struct Written {
    pub pdf: Vec<u8>,
    pub warnings: Vec<String>,
}

/// What the Info dictionary says made the PDF.
pub const CREATOR: &str = "LilyPond 2.26.0 (Lily's engine)";

/// The PDF of `doc`, its text in `fonts`.
pub fn write(doc: &Document, fonts: &[Font<'_>]) -> Written {
    let mut warnings = Vec::new();
    // The text fonts, read once each; none for one not given or unreadable.
    let mut faces: Vec<Option<Face<'_>>> = Vec::with_capacity(doc.fonts.len());
    for name in &doc.fonts {
        let face = match fonts.iter().find(|f| f.name == name) {
            Some(f) => match Face::new(f.otf) {
                Ok(face) => Some(face),
                Err(e) => {
                    warnings.push(format!("pdf: the font {name} is not read: {}", e.0));
                    None
                }
            },
            None => None,
        };
        faces.push(face);
    }
    let mut warned = BTreeSet::new();
    let mut glyphs_used = BTreeSet::new();
    let mut contents = Vec::with_capacity(doc.pages.len());
    for page in &doc.pages {
        let mut c = Content::default();
        c.page(page, doc, &mut faces, &mut glyphs_used, &mut warned);
        contents.push(c);
    }
    for font in warned {
        let name = doc.fonts.get(font as usize).map_or("?", String::as_str);
        warnings.push(format!("pdf: no font {name}: its text is left out"));
    }

    // The objects: 1 the catalog, 2 the pages, 3 the Info, 4 the resources,
    // then each glyph's, each font's five and each page's two.
    let mut pdf = Pdf::default();
    let glyph_ids: BTreeMap<u32, usize> = glyphs_used
        .iter()
        .enumerate()
        .map(|(i, &g)| (g, 5 + i))
        .collect();
    let mut next = 5 + glyph_ids.len();
    // Each font its runs drew with, subset; one that cannot be is left out.
    let mut font_ids = BTreeMap::new();
    for (i, face) in faces.iter().enumerate() {
        let Some(face) = face.as_ref().filter(|f| !f.used.is_empty()) else {
            continue;
        };
        match face.cff.subset(&face.used.keys().copied().collect()) {
            Ok(subset) => {
                font_ids.insert(i, (next, subset));
                next += 5;
            }
            Err(e) => warnings.push(format!(
                "pdf: the font {} is not subset: {}",
                doc.fonts[i], e.0
            )),
        }
    }
    let page_ids: Vec<usize> = (0..doc.pages.len()).map(|i| next + 2 * i).collect();

    pdf.out.extend_from_slice(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n");
    pdf.object(1, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = page_ids.iter().map(|id| format!("{id} 0 R")).collect();
    let pages = format!(
        "<< /Type /Pages /Kids [{}] /Count {} >>",
        kids.join(" "),
        page_ids.len()
    );
    pdf.object(2, pages.as_bytes());
    let mut info = String::from("<< ");
    for (key, value) in &doc.info {
        info.push('/');
        info.push_str(key);
        info.push(' ');
        text_string(&mut info, value);
        info.push(' ');
    }
    info.push_str("/Creator ");
    text_string(&mut info, CREATOR);
    info.push_str(" >>");
    pdf.object(3, info.as_bytes());
    let mut resources = String::from("<< /XObject << ");
    for (g, id) in &glyph_ids {
        resources.push_str(&format!("/G{g} {id} 0 R "));
    }
    resources.push_str(">> /Font << ");
    for (f, (id, _)) in &font_ids {
        resources.push_str(&format!("/F{f} {id} 0 R "));
    }
    resources.push_str(">> >>");
    pdf.object(4, resources.as_bytes());

    for (&g, &id) in &glyph_ids {
        glyph_object(&mut pdf, id, &doc.glyphs[g as usize]);
    }
    for (&f, (id, subset)) in &font_ids {
        let face = faces[f].as_ref().expect("a font with glyphs is read");
        font_objects(&mut pdf, *id, &doc.fonts[f], face, subset);
    }
    for ((page, c), &id) in doc.pages.iter().zip(&contents).zip(&page_ids) {
        let mut dict = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources 4 0 R /Contents {} 0 R",
            Num(page.width),
            Num(page.height),
            id + 1
        );
        if !c.links.is_empty() {
            dict.push_str(" /Annots [");
            for (rect, url) in &c.links {
                dict.push_str(&format!(
                    " << /Type /Annot /Subtype /Link /Border [0 0 0] /Rect [{} {} {} {}] /A << /S /URI /URI ",
                    Num(rect[0]),
                    Num(rect[1]),
                    Num(rect[2]),
                    Num(rect[3])
                ));
                literal(&mut dict, url.as_bytes());
                dict.push_str(" >> >>");
            }
            dict.push_str(" ]");
        }
        dict.push_str(" >>");
        pdf.object(id, dict.as_bytes());
        pdf.stream(id + 1, "", &c.out);
    }
    Written {
        pdf: pdf.finish(next + 2 * doc.pages.len()),
        warnings,
    }
}

/// A text font read: its CFF, its metrics, and the glyphs the pages draw
/// with the characters each stands for.
struct Face<'a> {
    cff: cff::Cff<'a>,
    metrics: cff::Metrics,
    used: BTreeMap<u16, String>,
}

impl<'a> Face<'a> {
    fn new(otf: &'a [u8]) -> Result<Face<'a>, cff::Unreadable> {
        let font = cff::OpenType::new(otf)?;
        let cff_bytes = font.table(b"CFF ")?;
        Ok(Face {
            cff: cff::Cff::new(cff_bytes)?,
            metrics: font.metrics()?,
            used: BTreeMap::new(),
        })
    }

    /// A glyph's advance in thousandths of the em, as the `W` array gives it.
    fn width(&self, gid: u16) -> f64 {
        let upem = f64::from(self.metrics.units_per_em.max(1));
        libm::round(f64::from(self.metrics.advance(gid)) * 1000.0 / upem)
    }
}

/// A number in a content stream: four decimals at most, none that end in 0.
struct Num(f64);

impl core::fmt::Display for Num {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut buf = Vec::new();
        num(&mut buf, self.0);
        f.write_str(core::str::from_utf8(&buf).unwrap_or("0"))
    }
}

fn num(out: &mut Vec<u8>, x: f64) {
    let scaled = if x.is_finite() {
        libm::round(x * 10000.0) as i64
    } else {
        0
    };
    if scaled < 0 {
        out.push(b'-');
    }
    let n = scaled.unsigned_abs();
    let (whole, mut frac) = (n / 10000, n % 10000);
    int(out, whole);
    if frac != 0 {
        out.push(b'.');
        let mut digits = 4;
        while frac % 10 == 0 {
            frac /= 10;
            digits -= 1;
        }
        let mut d = [b'0'; 4];
        for i in (0..digits).rev() {
            d[i] = b'0' + (frac % 10) as u8;
            frac /= 10;
        }
        out.extend_from_slice(&d[..digits]);
    }
}

fn int(out: &mut Vec<u8>, mut n: u64) {
    let mut d = [0u8; 20];
    let mut i = d.len();
    loop {
        i -= 1;
        d[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    out.extend_from_slice(&d[i..]);
}

/// A page's content stream being written.
#[derive(Default)]
struct Content {
    out: Vec<u8>,
    links: Vec<([f64; 4], String)>,
}

impl Content {
    fn nums(&mut self, xs: &[f64], op: &str) {
        for &x in xs {
            num(&mut self.out, x);
            self.out.push(b' ');
        }
        self.out.extend_from_slice(op.as_bytes());
        self.out.push(b'\n');
    }

    fn page(
        &mut self,
        page: &Page,
        doc: &Document,
        faces: &mut [Option<Face<'_>>],
        glyphs: &mut BTreeSet<u32>,
        warned: &mut BTreeSet<u32>,
    ) {
        let k = page.scale;
        let (ox, oy) = page.origin;
        // The staff space with y up, the stencil's unit, and the SVG page group's caps and joins.
        self.nums(&[k, 0.0, 0.0, k, -ox * k, -oy * k], "cm");
        self.out.extend_from_slice(b"1 J 1 j\n");
        for op in &page.ops {
            match op {
                Op::Save => self.out.extend_from_slice(b"q\n"),
                Op::Restore => self.out.extend_from_slice(b"Q\n"),
                Op::Color([r, g, b]) => {
                    self.nums(&[*r, *g, *b], "rg");
                    self.nums(&[*r, *g, *b], "RG");
                }
                Op::Transform(m) => self.nums(m, "cm"),
                &Op::Box { x, y, w, h, blot } => {
                    if blot > 0.0 {
                        // The box inside half the blot, stroked by the blot with round joins.
                        let r = blot / 2.0;
                        self.out.extend_from_slice(b"q\n");
                        self.nums(&[blot], "w");
                        self.nums(
                            &[x + r, y + r, (w - blot).max(0.0), (h - blot).max(0.0)],
                            "re",
                        );
                        self.out.extend_from_slice(b"B\nQ\n");
                    } else {
                        self.nums(&[x, y, w, h], "re");
                        self.out.extend_from_slice(b"f\n");
                    }
                }
                &Op::Line {
                    thick,
                    from,
                    to,
                    dash,
                } => {
                    if thick <= 0.0 {
                        continue;
                    }
                    self.out.extend_from_slice(b"q\n");
                    self.nums(&[thick], "w");
                    if let Some([on, off, phase]) = dash {
                        self.out.push(b'[');
                        num(&mut self.out, on);
                        self.out.push(b' ');
                        num(&mut self.out, off);
                        self.out.extend_from_slice(b"] ");
                        self.nums(&[phase], "d");
                    }
                    self.nums(&[from.0, from.1], "m");
                    self.nums(&[to.0, to.1], "l");
                    self.out.extend_from_slice(b"S\nQ\n");
                }
                &Op::Ellipse {
                    at,
                    rx,
                    ry,
                    thick,
                    fill,
                } => {
                    self.arc(at, rx, ry, 0.0, 2.0 * PI);
                    self.out.extend_from_slice(b"h\n");
                    self.paint(thick, fill);
                }
                &Op::Arc {
                    at,
                    rx,
                    ry,
                    start,
                    end,
                    thick,
                    connect,
                    fill,
                } => {
                    let polar = |deg: f64| {
                        let a = angle_0_360(deg) * PI / 180.0;
                        libm::atan2(rx * libm::sin(a), ry * libm::cos(a))
                    };
                    let (t0, mut t1) = (polar(start), polar(end));
                    while t1 <= t0 + 1e-9 {
                        t1 += 2.0 * PI;
                    }
                    let whole = (t1 - t0 - 2.0 * PI).abs() < 1e-6
                        || (angle_0_360(start) - angle_0_360(end)).abs() < 1e-9;
                    if whole {
                        self.arc(at, rx, ry, 0.0, 2.0 * PI);
                        self.out.extend_from_slice(b"h\n");
                    } else {
                        self.arc(at, rx, ry, t0, t1);
                        if connect {
                            self.out.extend_from_slice(b"h\n");
                        }
                    }
                    self.paint(thick, fill);
                }
                Op::Polygon { points, blot, fill } => {
                    for (i, p) in points.iter().enumerate() {
                        self.nums(&[p.0, p.1], if i == 0 { "m" } else { "l" });
                    }
                    self.out.extend_from_slice(b"h\n");
                    self.paint(*blot, *fill);
                }
                Op::Path {
                    thick,
                    cap,
                    join,
                    fill,
                    commands,
                } => {
                    self.out.extend_from_slice(b"q\n");
                    self.nums(&[f64::from(*cap)], "J");
                    self.nums(&[f64::from(*join)], "j");
                    for c in commands {
                        match *c {
                            Command::Move(x, y) => self.nums(&[x, y], "m"),
                            Command::Line(x, y) => self.nums(&[x, y], "l"),
                            Command::Curve(p) => self.nums(&p, "c"),
                            Command::Close => self.out.extend_from_slice(b"h\n"),
                        }
                    }
                    self.paint(*thick, *fill);
                    self.out.extend_from_slice(b"Q\n");
                }
                &Op::Glyph { glyph, at, scale } => {
                    if doc.glyphs.get(glyph as usize).is_none_or(Vec::is_empty) {
                        continue;
                    }
                    glyphs.insert(glyph);
                    self.out.extend_from_slice(b"q\n");
                    self.nums(&[scale, 0.0, 0.0, scale, at.0, at.1], "cm");
                    self.out
                        .extend_from_slice(format!("/G{glyph} Do\nQ\n").as_bytes());
                }
                Op::Text {
                    font,
                    size,
                    at,
                    glyphs: run,
                } => {
                    let Some(face) = faces.get_mut(*font as usize).and_then(Option::as_mut) else {
                        warned.insert(*font);
                        continue;
                    };
                    self.text(face, *font, *size, *at, run);
                }
                Op::Link { rect, url } => {
                    let [x0, y0, x1, y1] = *rect;
                    let p = |x: f64, y: f64| ((x - ox) * k, (y - oy) * k);
                    let (a, b) = (p(x0, y0), p(x1, y1));
                    self.links.push((
                        [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)],
                        url.clone(),
                    ));
                }
            }
        }
    }

    /// Fills and strokes what was drawn: a stroke of no width draws nothing,
    /// as SVG's does not.
    fn paint(&mut self, thick: f64, fill: bool) {
        let op: &[u8] = match (fill, thick > 0.0) {
            (true, true) => b"B\n",
            (true, false) => b"f\n",
            (false, true) => b"S\n",
            (false, false) => b"n\n",
        };
        if thick > 0.0 {
            self.nums(&[thick], "w");
        }
        self.out.extend_from_slice(op);
    }

    /// The ellipse's arc from parameter `t0` to `t1`, in Béziers of a
    /// quarter at most, moving to its start.
    fn arc(&mut self, (cx, cy): (f64, f64), rx: f64, ry: f64, t0: f64, t1: f64) {
        let n = libm::ceil((t1 - t0) / (PI / 2.0) - 1e-9).max(1.0) as usize;
        let step = (t1 - t0) / n as f64;
        let h = 4.0 / 3.0 * libm::tan(step / 4.0);
        let at = |t: f64| (cx + rx * libm::cos(t), cy + ry * libm::sin(t));
        let start = at(t0);
        self.nums(&[start.0, start.1], "m");
        for i in 0..n {
            let (a, b) = (t0 + step * i as f64, t0 + step * (i + 1) as f64);
            let (p0, p3) = (at(a), at(b));
            let c1 = (p0.0 - h * rx * libm::sin(a), p0.1 + h * ry * libm::cos(a));
            let c2 = (p3.0 + h * rx * libm::sin(b), p3.1 - h * ry * libm::cos(b));
            self.nums(&[c1.0, c1.1, c2.0, c2.1, p3.0, p3.1], "c");
        }
    }

    /// A run of text: its glyph ids in `TJ`, each glyph's advance the core
    /// shaped with set against the font's by an adjustment, and a new `Tm`
    /// where a glyph is offset.
    fn text(
        &mut self,
        face: &mut Face<'_>,
        font: u32,
        size: f64,
        at: (f64, f64),
        run: &[TextGlyph],
    ) {
        let count = face.cff.glyph_count();
        self.out
            .extend_from_slice(format!("BT\n/F{font} ").as_bytes());
        self.nums(&[size], "Tf");
        let mut pen = at.0;
        let mut open = false;
        let mut placed: Option<(f64, f64)> = None;
        for g in run {
            if usize::from(g.gid) >= count {
                pen += g.advance;
                continue;
            }
            face.used.entry(g.gid).or_insert_with(|| g.text.clone());
            if placed != Some(g.offset) {
                if open {
                    self.out.extend_from_slice(b"] TJ\n");
                }
                self.nums(
                    &[1.0, 0.0, 0.0, 1.0, pen + g.offset.0, at.1 + g.offset.1],
                    "Tm",
                );
                self.out.push(b'[');
                open = true;
                placed = Some(g.offset);
            }
            self.out
                .extend_from_slice(format!("<{:04x}>", g.gid).as_bytes());
            let adjust = face.width(g.gid) - g.advance / size * 1000.0;
            if adjust.abs() >= 0.05 {
                num(&mut self.out, adjust);
                self.out.push(b' ');
            }
            pen += g.advance;
        }
        if open {
            self.out.extend_from_slice(b"] TJ\n");
        }
        self.out.extend_from_slice(b"ET\n");
    }
}

/// `angle-0-360`.
fn angle_0_360(mut angle: f64) -> f64 {
    while angle < 0.0 {
        angle += 360.0;
    }
    while angle >= 360.0 {
        angle -= 360.0;
    }
    angle
}

/// A music glyph's form XObject: its outline in font units, filled.
fn glyph_object(pdf: &mut Pdf, id: usize, outline: &[Segment]) {
    let mut c = Content::default();
    let (mut lo, mut hi) = ([i32::MAX; 2], [i32::MIN; 2]);
    let mut grow = |p: [i32; 2]| {
        for i in 0..2 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    };
    let f = |p: [i32; 2]| [f64::from(p[0]), f64::from(p[1])];
    let mut open = false;
    for s in outline {
        match *s {
            Segment::Move(p) => {
                if open {
                    c.out.extend_from_slice(b"h\n");
                }
                open = true;
                grow(p);
                c.nums(&f(p), "m");
            }
            Segment::Line(p) => {
                grow(p);
                c.nums(&f(p), "l");
            }
            Segment::Curve(a, b, d) => {
                [a, b, d].into_iter().for_each(&mut grow);
                let (a, b, d) = (f(a), f(b), f(d));
                c.nums(&[a[0], a[1], b[0], b[1], d[0], d[1]], "c");
            }
        }
    }
    c.out.extend_from_slice(b"h\nf\n");
    let dict = format!(
        "/Type /XObject /Subtype /Form /BBox [{} {} {} {}]",
        lo[0], lo[1], hi[0], hi[1]
    );
    pdf.stream(id, &dict, &c.out);
}

/// A text font's five objects from `id`: the Type 0 font, its CIDFont,
/// their descriptor, the subset CFF and the ToUnicode map.
fn font_objects(pdf: &mut Pdf, id: usize, name: &str, face: &Face<'_>, subset: &[u8]) {
    let keep: BTreeSet<u16> = face.used.keys().copied().collect();
    // The subset's tag (PDF 2.0 §9.6.4): six capitals from the glyphs kept.
    let mut hash: u32 = 0x811c_9dc5;
    for gid in &keep {
        for b in gid.to_be_bytes() {
            hash = (hash ^ u32::from(b)).wrapping_mul(0x0100_0193);
        }
    }
    let mut tag = String::new();
    for _ in 0..6 {
        tag.push(char::from(b'A' + (hash % 26) as u8));
        hash /= 26;
    }
    let base = format!("{tag}+{name}");
    let m = &face.metrics;
    let upem = f64::from(m.units_per_em.max(1));
    let em = |v: i32| libm::round(f64::from(v) * 1000.0 / upem);
    let mut widths = String::new();
    for &gid in &keep {
        widths.push_str(&format!("{gid} [{}] ", Num(face.width(gid))));
    }
    pdf.object(
        id,
        format!("<< /Type /Font /Subtype /Type0 /BaseFont /{base}-Identity-H /Encoding /Identity-H /DescendantFonts [{} 0 R] /ToUnicode {} 0 R >>", id + 1, id + 4).as_bytes(),
    );
    pdf.object(
        id + 1,
        format!("<< /Type /Font /Subtype /CIDFontType0 /BaseFont /{base} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {} 0 R /DW 0 /W [{widths}] >>", id + 2).as_bytes(),
    );
    let flags = 32 | if m.fixed_pitch { 1 } else { 0 } | if m.italic_angle != 0.0 { 64 } else { 0 };
    pdf.object(
        id + 2,
        format!(
            "<< /Type /FontDescriptor /FontName /{base} /Flags {flags} /FontBBox [{} {} {} {}] /ItalicAngle {} /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile3 {} 0 R >>",
            Num(em(m.bbox[0])),
            Num(em(m.bbox[1])),
            Num(em(m.bbox[2])),
            Num(em(m.bbox[3])),
            Num(m.italic_angle),
            Num(em(m.ascent)),
            Num(em(m.descent)),
            Num(em(m.cap_height)),
            id + 3
        )
        .as_bytes(),
    );
    pdf.stream(id + 3, "/Subtype /CIDFontType0C", subset);
    pdf.stream(id + 4, "", to_unicode(&face.used).as_bytes());
}

/// The ToUnicode CMap of a font's glyphs: each glyph id to the characters
/// it was shaped from.
fn to_unicode(used: &BTreeMap<u16, String>) -> String {
    let mut out = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<(&u16, &String)> = used.iter().filter(|(_, t)| !t.is_empty()).collect();
    for chunk in entries.chunks(100) {
        out.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (gid, text) in chunk {
            out.push_str(&format!("<{gid:04X}> <"));
            for unit in text.encode_utf16() {
                out.push_str(&format!("{unit:04X}"));
            }
            out.push_str(">\n");
        }
        out.push_str("endbfchar\n");
    }
    out.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    out
}

/// A PDF text string: a literal when it is printable ASCII, else UTF-16BE
/// with its byte order mark, as `pdf-encode` writes one.
fn text_string(out: &mut String, s: &str) {
    if s.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        literal(out, s.as_bytes());
    } else {
        out.push_str("<FEFF");
        for unit in s.encode_utf16() {
            out.push_str(&format!("{unit:04X}"));
        }
        out.push('>');
    }
}

fn literal(out: &mut String, bytes: &[u8]) {
    out.push('(');
    for &b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(char::from(b));
            }
            0x20..0x7f => out.push(char::from(b)),
            _ => out.push_str(&format!("\\{b:03o}")),
        }
    }
    out.push(')');
}

/// The file being written: its bytes and each object's offset.
#[derive(Default)]
struct Pdf {
    out: Vec<u8>,
    offsets: BTreeMap<usize, usize>,
}

impl Pdf {
    fn object(&mut self, id: usize, body: &[u8]) {
        self.offsets.insert(id, self.out.len());
        self.out
            .extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        self.out.extend_from_slice(body);
        self.out.extend_from_slice(b"\nendobj\n");
    }

    /// A stream of `data` deflated, `dict` its other entries.
    fn stream(&mut self, id: usize, dict: &str, data: &[u8]) {
        let deflated = miniz_oxide::deflate::compress_to_vec_zlib(data, 9);
        self.offsets.insert(id, self.out.len());
        let space = if dict.is_empty() { "" } else { " " };
        self.out.extend_from_slice(
            format!(
                "{id} 0 obj\n<< {dict}{space}/Filter /FlateDecode /Length {} >>\nstream\n",
                deflated.len()
            )
            .as_bytes(),
        );
        self.out.extend_from_slice(&deflated);
        self.out.extend_from_slice(b"\nendstream\nendobj\n");
    }

    /// The cross-reference table of objects `1..size` and the trailer.
    fn finish(mut self, size: usize) -> Vec<u8> {
        let xref = self.out.len();
        self.out
            .extend_from_slice(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
        for id in 1..size {
            match self.offsets.get(&id) {
                Some(at) => self
                    .out
                    .extend_from_slice(format!("{at:010} 00000 n \n").as_bytes()),
                None => self.out.extend_from_slice(b"0000000000 65535 f \n"),
            }
        }
        self.out.extend_from_slice(
            format!(
                "trailer\n<< /Size {size} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n"
            )
            .as_bytes(),
        );
        self.out
    }
}
