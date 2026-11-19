//! The display list (06 §7.3): what the core gives the PDF module of a
//! book, its pages' primitives with their positions resolved, the music
//! glyphs' outlines once each, and the `\header` fields of the Info
//! dictionary; and its bytes, which cross from the core to the module.
//!
//! Positions are in staff spaces with y up, the stencil's own unit, each
//! primitive placed at its `at`; a page's `scale` turns them into PDF
//! points. The bytes: a tag or a count as an unsigned LEB128 varint, every
//! number an `f64` little-endian, a string as its length and UTF-8 bytes.

use alloc::string::String;
use alloc::vec::Vec;

/// A book's pages, as one PDF.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    /// The Info dictionary's entries, in order: `Title`, `Author` and the
    /// rest of `handle-metadata`'s (framework-ps.scm).
    pub info: Vec<(String, String)>,
    /// The music glyphs' outlines in font units (1,000 to the em), each
    /// once; a [`Op::Glyph`] names one by its place here.
    pub glyphs: Vec<Vec<Segment>>,
    /// The PostScript names of the text fonts; a [`Op::Text`] names one by
    /// its place here.
    pub fonts: Vec<String>,
    pub pages: Vec<Page>,
}

/// One piece of an outline, as `crates/fonts`' packs give it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment {
    Move([i32; 2]),
    Line([i32; 2]),
    Curve([i32; 2], [i32; 2], [i32; 2]),
}

/// A page: its size in points, the points in a staff space, the staff
/// space at its lower left corner, and what it draws.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    pub origin: (f64, f64),
    pub ops: Vec<Op>,
}

/// A path's piece, absolute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Move(f64, f64),
    Line(f64, f64),
    Curve([f64; 6]),
    Close,
}

/// A glyph of a text run: its glyph id in the font, its advance and its
/// offset in staff spaces, and the characters it stands for.
#[derive(Clone, Debug, PartialEq)]
pub struct TextGlyph {
    pub gid: u16,
    pub advance: f64,
    pub offset: (f64, f64),
    pub text: String,
}

/// What a page draws. `Save` and `Restore` bracket a colour, a rotation
/// and a scale, as the walk opens and closes them.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Save,
    Restore,
    /// The colour to fill and stroke with, red, green and blue in 0–1.
    Color([f64; 3]),
    /// A matrix applied to what follows, until its `Restore`.
    Transform([f64; 6]),
    /// `round-filled-box`: the box from `(x, y)`, its corners rounded by
    /// half the blot.
    Box {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        blot: f64,
    },
    /// `draw-line` and `dashed-line`, the dash as on, off and phase.
    Line {
        thick: f64,
        from: (f64, f64),
        to: (f64, f64),
        dash: Option<[f64; 3]>,
    },
    /// `circle` and `ellipse`, centred on `at`.
    Ellipse {
        at: (f64, f64),
        rx: f64,
        ry: f64,
        thick: f64,
        fill: bool,
    },
    /// `partial-ellipse`, from `start` to `end` degrees.
    Arc {
        at: (f64, f64),
        rx: f64,
        ry: f64,
        start: f64,
        end: f64,
        thick: f64,
        connect: bool,
        fill: bool,
    },
    /// `polygon`, its points absolute.
    Polygon {
        points: Vec<(f64, f64)>,
        blot: f64,
        fill: bool,
    },
    /// `path`: cap 0 butt, 1 round, 2 square; join 0 miter, 1 round, 2
    /// bevel.
    Path {
        thick: f64,
        cap: u8,
        join: u8,
        fill: bool,
        commands: Vec<Command>,
    },
    /// A music glyph of [`Document::glyphs`], at `at`, its font units
    /// scaled by `scale`.
    Glyph {
        glyph: u32,
        at: (f64, f64),
        scale: f64,
    },
    /// A run of a text font of [`Document::fonts`] at `size` staff spaces,
    /// its first glyph's origin at `at`.
    Text {
        font: u32,
        size: f64,
        at: (f64, f64),
        glyphs: Vec<TextGlyph>,
    },
    /// `url-link`: the rectangle, x0 y0 x1 y1, opens `url`.
    Link {
        rect: [f64; 4],
        url: String,
    },
}

/// What a display list's bytes do not hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Malformed;

const MAGIC: &[u8; 4] = b"LILD";
const VERSION: u8 = 1;

impl Document {
    /// The display list's bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Out(Vec::with_capacity(1 << 16));
        w.0.extend_from_slice(MAGIC);
        w.0.push(VERSION);
        w.n(self.info.len());
        for (key, value) in &self.info {
            w.s(key);
            w.s(value);
        }
        w.n(self.glyphs.len());
        for glyph in &self.glyphs {
            w.n(glyph.len());
            for segment in glyph {
                match *segment {
                    Segment::Move(p) => w.points(0, &[p]),
                    Segment::Line(p) => w.points(1, &[p]),
                    Segment::Curve(a, b, c) => w.points(2, &[a, b, c]),
                }
            }
        }
        w.n(self.fonts.len());
        for font in &self.fonts {
            w.s(font);
        }
        w.n(self.pages.len());
        for page in &self.pages {
            w.fs(&[
                page.width,
                page.height,
                page.scale,
                page.origin.0,
                page.origin.1,
            ]);
            w.n(page.ops.len());
            for op in &page.ops {
                w.op(op);
            }
        }
        w.0
    }

    /// The display list `bytes` hold.
    pub fn decode(bytes: &[u8]) -> Result<Document, Malformed> {
        let mut r = In { bytes, at: 0 };
        if r.take(4)? != MAGIC || r.byte()? != VERSION {
            return Err(Malformed);
        }
        let mut doc = Document::default();
        for _ in 0..r.count()? {
            doc.info.push((r.s()?, r.s()?));
        }
        for _ in 0..r.count()? {
            let mut glyph = Vec::new();
            for _ in 0..r.count()? {
                glyph.push(match r.byte()? {
                    0 => Segment::Move(r.point()?),
                    1 => Segment::Line(r.point()?),
                    2 => Segment::Curve(r.point()?, r.point()?, r.point()?),
                    _ => return Err(Malformed),
                });
            }
            doc.glyphs.push(glyph);
        }
        for _ in 0..r.count()? {
            doc.fonts.push(r.s()?);
        }
        for _ in 0..r.count()? {
            let [width, height, scale, x, y] = r.fs()?;
            let mut page = Page {
                width,
                height,
                scale,
                origin: (x, y),
                ops: Vec::new(),
            };
            for _ in 0..r.count()? {
                page.ops.push(r.op()?);
            }
            doc.pages.push(page);
        }
        if r.at != bytes.len() {
            return Err(Malformed);
        }
        Ok(doc)
    }
}

struct Out(Vec<u8>);

impl Out {
    fn n(&mut self, mut n: usize) {
        loop {
            let byte = (n & 0x7f) as u8;
            n >>= 7;
            if n == 0 {
                self.0.push(byte);
                return;
            }
            self.0.push(byte | 0x80);
        }
    }
    fn f(&mut self, x: f64) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn fs(&mut self, xs: &[f64]) {
        xs.iter().for_each(|&x| self.f(x));
    }
    fn s(&mut self, s: &str) {
        self.n(s.len());
        self.0.extend_from_slice(s.as_bytes());
    }
    fn b(&mut self, b: bool) {
        self.0.push(b as u8);
    }
    fn points(&mut self, tag: u8, points: &[[i32; 2]]) {
        self.0.push(tag);
        for p in points {
            for v in p {
                self.n(((v << 1) ^ (v >> 31)) as u32 as usize);
            }
        }
    }
    fn op(&mut self, op: &Op) {
        match op {
            Op::Save => self.n(0),
            Op::Restore => self.n(1),
            Op::Color(c) => {
                self.n(2);
                self.fs(c);
            }
            Op::Transform(m) => {
                self.n(3);
                self.fs(m);
            }
            &Op::Box { x, y, w, h, blot } => {
                self.n(4);
                self.fs(&[x, y, w, h, blot]);
            }
            &Op::Line {
                thick,
                from,
                to,
                dash,
            } => {
                self.n(5);
                self.fs(&[thick, from.0, from.1, to.0, to.1]);
                self.b(dash.is_some());
                if let Some(dash) = dash {
                    self.fs(&dash);
                }
            }
            &Op::Ellipse {
                at,
                rx,
                ry,
                thick,
                fill,
            } => {
                self.n(6);
                self.fs(&[at.0, at.1, rx, ry, thick]);
                self.b(fill);
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
                self.n(7);
                self.fs(&[at.0, at.1, rx, ry, start, end, thick]);
                self.b(connect);
                self.b(fill);
            }
            Op::Polygon { points, blot, fill } => {
                self.n(8);
                self.n(points.len());
                points.iter().for_each(|p| self.fs(&[p.0, p.1]));
                self.f(*blot);
                self.b(*fill);
            }
            Op::Path {
                thick,
                cap,
                join,
                fill,
                commands,
            } => {
                self.n(9);
                self.f(*thick);
                self.0.extend_from_slice(&[*cap, *join]);
                self.b(*fill);
                self.n(commands.len());
                for c in commands {
                    match *c {
                        Command::Move(x, y) => {
                            self.n(0);
                            self.fs(&[x, y]);
                        }
                        Command::Line(x, y) => {
                            self.n(1);
                            self.fs(&[x, y]);
                        }
                        Command::Curve(p) => {
                            self.n(2);
                            self.fs(&p);
                        }
                        Command::Close => self.n(3),
                    }
                }
            }
            &Op::Glyph { glyph, at, scale } => {
                self.n(10);
                self.n(glyph as usize);
                self.fs(&[at.0, at.1, scale]);
            }
            Op::Text {
                font,
                size,
                at,
                glyphs,
            } => {
                self.n(11);
                self.n(*font as usize);
                self.fs(&[*size, at.0, at.1]);
                self.n(glyphs.len());
                for g in glyphs {
                    self.n(usize::from(g.gid));
                    self.fs(&[g.advance, g.offset.0, g.offset.1]);
                    self.s(&g.text);
                }
            }
            Op::Link { rect, url } => {
                self.n(12);
                self.fs(rect);
                self.s(url);
            }
        }
    }
}

struct In<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> In<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or(Malformed)?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }
    fn byte(&mut self) -> Result<u8, Malformed> {
        Ok(self.take(1)?[0])
    }
    fn n(&mut self) -> Result<usize, Malformed> {
        let mut n = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            n |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return usize::try_from(n).map_err(|_| Malformed);
            }
        }
        Err(Malformed)
    }
    /// A count, which no more than the bytes left can hold.
    fn count(&mut self) -> Result<usize, Malformed> {
        let n = self.n()?;
        if n > self.bytes.len() - self.at {
            return Err(Malformed);
        }
        Ok(n)
    }
    fn f(&mut self) -> Result<f64, Malformed> {
        let mut b = [0; 8];
        b.copy_from_slice(self.take(8)?);
        Ok(f64::from_le_bytes(b))
    }
    fn fs<const N: usize>(&mut self) -> Result<[f64; N], Malformed> {
        let mut xs = [0.0; N];
        for x in &mut xs {
            *x = self.f()?;
        }
        Ok(xs)
    }
    fn s(&mut self) -> Result<String, Malformed> {
        let n = self.count()?;
        core::str::from_utf8(self.take(n)?)
            .map(String::from)
            .map_err(|_| Malformed)
    }
    fn b(&mut self) -> Result<bool, Malformed> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Malformed),
        }
    }
    fn point(&mut self) -> Result<[i32; 2], Malformed> {
        let mut p = [0; 2];
        for v in &mut p {
            let z = u32::try_from(self.n()?).map_err(|_| Malformed)?;
            *v = ((z >> 1) as i32) ^ -((z & 1) as i32);
        }
        Ok(p)
    }
    fn op(&mut self) -> Result<Op, Malformed> {
        Ok(match self.n()? {
            0 => Op::Save,
            1 => Op::Restore,
            2 => Op::Color(self.fs()?),
            3 => Op::Transform(self.fs()?),
            4 => {
                let [x, y, w, h, blot] = self.fs()?;
                Op::Box { x, y, w, h, blot }
            }
            5 => {
                let [thick, x1, y1, x2, y2] = self.fs()?;
                let dash = if self.b()? { Some(self.fs()?) } else { None };
                Op::Line {
                    thick,
                    from: (x1, y1),
                    to: (x2, y2),
                    dash,
                }
            }
            6 => {
                let [x, y, rx, ry, thick] = self.fs()?;
                Op::Ellipse {
                    at: (x, y),
                    rx,
                    ry,
                    thick,
                    fill: self.b()?,
                }
            }
            7 => {
                let [x, y, rx, ry, start, end, thick] = self.fs()?;
                let (connect, fill) = (self.b()?, self.b()?);
                Op::Arc {
                    at: (x, y),
                    rx,
                    ry,
                    start,
                    end,
                    thick,
                    connect,
                    fill,
                }
            }
            8 => {
                let mut points = Vec::new();
                for _ in 0..self.count()? {
                    let [x, y] = self.fs()?;
                    points.push((x, y));
                }
                Op::Polygon {
                    points,
                    blot: self.f()?,
                    fill: self.b()?,
                }
            }
            9 => {
                let thick = self.f()?;
                let (cap, join, fill) = (self.byte()?, self.byte()?, self.b()?);
                let mut commands = Vec::new();
                for _ in 0..self.count()? {
                    commands.push(match self.n()? {
                        0 => {
                            let [x, y] = self.fs()?;
                            Command::Move(x, y)
                        }
                        1 => {
                            let [x, y] = self.fs()?;
                            Command::Line(x, y)
                        }
                        2 => Command::Curve(self.fs()?),
                        3 => Command::Close,
                        _ => return Err(Malformed),
                    });
                }
                Op::Path {
                    thick,
                    cap,
                    join,
                    fill,
                    commands,
                }
            }
            10 => {
                let glyph = u32::try_from(self.n()?).map_err(|_| Malformed)?;
                let [x, y, scale] = self.fs()?;
                Op::Glyph {
                    glyph,
                    at: (x, y),
                    scale,
                }
            }
            11 => {
                let font = u32::try_from(self.n()?).map_err(|_| Malformed)?;
                let [size, x, y] = self.fs()?;
                let mut glyphs = Vec::new();
                for _ in 0..self.count()? {
                    let gid = u16::try_from(self.n()?).map_err(|_| Malformed)?;
                    let [advance, dx, dy] = self.fs()?;
                    glyphs.push(TextGlyph {
                        gid,
                        advance,
                        offset: (dx, dy),
                        text: self.s()?,
                    });
                }
                Op::Text {
                    font,
                    size,
                    at: (x, y),
                    glyphs,
                }
            }
            12 => Op::Link {
                rect: self.fs()?,
                url: self.s()?,
            },
            _ => return Err(Malformed),
        })
    }
}
