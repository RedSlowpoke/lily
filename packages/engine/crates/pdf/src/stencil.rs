//! The core's half (feature `stencils`): a page's stencil walked into the
//! display list, by `crates/svg`'s walk (`stencil-interpret.cc`), each
//! primitive as `output-svg.scm`'s procedure for it reads its arguments,
//! and `handle-metadata`'s Info entries from a book's `\header`.
//!
//! Text is the `glyph-string`s under each `utf-8-string`, which the walk
//! gives when no procedure takes the string (output-ps.scm's way): the
//! glyph ids and advances the core shaped, the characters each glyph's
//! cluster holds. A `glyph-string` in a music font, as SVG's, draws its
//! glyphs as outlines.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use lily_engine_layout::{Offset, Stencil, X, Y, font};
use lily_engine_scheme::{Exception, Heap, Value, Vm};
use lily_engine_svg::{Call, Sink, interpret};

use crate::list::{Command, Document, Op, Page, Segment, TextGlyph};

/// Millimetres in a point (PostScript's).
const MM_PER_POINT: f64 = 25.4 / 72.0;

/// A book's display list being made, a page at a time.
#[derive(Default)]
pub struct Book {
    doc: Document,
    /// Each music glyph's place in the list, by font and glyph name.
    glyphs: BTreeMap<(String, String), u32>,
    /// Each text font's place, by PostScript name.
    fonts: BTreeMap<String, u32>,
    pub warnings: Vec<String>,
}

impl Book {
    pub fn new() -> Book {
        Book::default()
    }

    /// The page whose stencil is `stencil`, at `output_scale` millimetres a
    /// staff space.
    pub fn page(
        &mut self,
        vm: &mut Vm,
        stencil: &Stencil,
        output_scale: f64,
    ) -> Result<(), Exception> {
        let (x, y) = (stencil.extent(X), stencil.extent(Y));
        let scale = output_scale / MM_PER_POINT;
        let mut walk = Walk {
            book: self,
            page: Page {
                width: (x.right - x.left).max(0.0) * scale,
                height: (y.right - y.left).max(0.0) * scale,
                scale,
                origin: (x.left, y.left),
                ops: Vec::new(),
            },
            unit_length: output_scale,
            translation: (0.0, 0.0),
            next_horiz_adv: 0.0,
            loaded: Vec::new(),
        };
        let depth = vm.heap().root_depth();
        vm.heap_mut().root(stencil.expr);
        let walked = interpret(vm, stencil.expr, &mut walk, (0.0, 0.0));
        vm.heap_mut().unroot_to(depth);
        walked?;
        let page = walk.page;
        self.doc.pages.push(page);
        Ok(())
    }

    /// `handle-metadata`'s entries of `header`, a `\header` module: each
    /// field's `pdf…` variable before its own, as `markup->string` gives
    /// it. Nothing for what is not a module or a field that fails.
    pub fn info(&mut self, vm: &mut Vm, header: Value) {
        const ENTRIES: &str = "(lambda (header)
  (if (module? header)
      (map (lambda (entry)
             (let ((v (or (ly:modules-lookup (list header) (car entry))
                          (ly:modules-lookup (list header) (cadr entry)))))
               (and v (cons (caddr entry)
                            (markup->string v #:props (headers-property-alist-chain (list header)))))))
           '((pdfauthor author \"Author\") (pdftitle title \"Title\") (pdfsubject subject \"Subject\")
             (pdfkeywords keywords \"Keywords\") (pdfmodDate modDate \"ModDate\")
             (pdfcreationDate creationDate \"CreationDate\") (pdfsubtitle subtitle \"Subtitle\")
             (pdfcomposer composer \"Composer\") (pdfarranger arranger \"Arranger\") (pdfpoet poet \"Poet\")
             (pdfcopyright copyright \"Copyright\")))
      '()))";
        let depth = vm.heap().root_depth();
        vm.heap_mut().root(header);
        let entries = vm
            .eval_text(ENTRIES, None)
            .and_then(|f| vm.apply(f, &[header]));
        vm.heap_mut().unroot_to(depth);
        let Ok(mut list) = entries else {
            self.warnings
                .push(String::from("pdf: the header's fields are not read"));
            return;
        };
        let heap = vm.heap();
        while let (Some(entry), Some(rest)) = (heap.car(list), heap.cdr(list)) {
            list = rest;
            let (Some(key), Some(value)) = (heap.car(entry), heap.cdr(entry)) else {
                continue;
            };
            if let (Some(key), Some(value)) = (heap.str(key), heap.str(value))
                && !value.as_str().is_empty()
            {
                self.doc
                    .info
                    .push((String::from(key.as_str()), String::from(value.as_str())));
            }
        }
    }

    pub fn finish(self) -> Document {
        self.doc
    }
}

struct Walk<'b> {
    book: &'b mut Book,
    page: Page,
    /// `lily-unit-length`: the paper's `output-scale`.
    unit_length: f64,
    translation: Offset,
    next_horiz_adv: f64,
    /// The music fonts of glyph strings, by `font-name-style`, as SVG's.
    loaded: Vec<(String, Option<Value>)>,
}

impl Sink for Walk<'_> {
    fn output(&mut self, vm: &mut Vm, call: Call) -> Result<bool, Exception> {
        let ops = &mut self.page.ops;
        match call {
            Call::SetTranslation(o) => self.translation = o,
            Call::Primitive(expr) => return self.primitive(vm, expr),
            Call::SetColor(rgba) => {
                let heap = vm.heap();
                let [r, g, b, _] = rgba.map(|v| number(heap, v));
                ops.push(Op::Save);
                ops.push(Op::Color([r, g, b]));
            }
            Call::SetRotation(angle, x, y) => {
                let a = number(vm.heap(), angle) * core::f64::consts::PI / 180.0;
                let (c, s) = (libm::cos(a), libm::sin(a));
                ops.push(Op::Save);
                ops.push(Op::Transform([
                    c,
                    s,
                    -s,
                    c,
                    x - c * x + s * y,
                    y - s * x - c * y,
                ]));
            }
            Call::SetScale(x, y) => {
                let heap = vm.heap();
                ops.push(Op::Save);
                ops.push(Op::Transform([
                    number(heap, x),
                    0.0,
                    0.0,
                    number(heap, y),
                    0.0,
                    0.0,
                ]));
            }
            Call::ResetColor | Call::ResetRotation | Call::ResetScale => ops.push(Op::Restore),
            Call::ResetTranslation
            | Call::GrobCause(..)
            | Call::NoOrigin
            | Call::StartGroupNode(_)
            | Call::EndGroupNode => {}
        }
        Ok(true)
    }
}

impl Walk<'_> {
    /// The primitive at the last translation; false for a head with no
    /// procedure here, `utf-8-string` among them, whose glyphs the walk
    /// then gives.
    fn primitive(&mut self, vm: &mut Vm, expr: Value) -> Result<bool, Exception> {
        let (tx, ty) = self.translation;
        let at = |x: f64, y: f64| (tx + x, ty + y);
        let heap = vm.heap();
        let head = heap.car(expr).and_then(|h| heap.name(h)).unwrap_or("");
        let arg = |i: usize| nth(heap, expr, i + 1);
        let n = |i: usize| number(heap, arg(i));
        let op = match head {
            "round-filled-box" => {
                let [b, w, d, h, blot] = [0, 1, 2, 3, 4].map(n);
                Op::Box {
                    x: tx - b,
                    y: ty - d,
                    w: b + w,
                    h: d + h,
                    blot,
                }
            }
            "draw-line" => {
                let [thick, x1, y1, x2, y2] = [0, 1, 2, 3, 4].map(n);
                Op::Line {
                    thick,
                    from: at(x1, y1),
                    to: at(x2, y2),
                    dash: None,
                }
            }
            "dashed-line" => {
                let [thick, on, off, dx, dy, phase] = [0, 1, 2, 3, 4, 5].map(n);
                let dash = (on + off > 0.0).then_some([on, off, phase]);
                Op::Line {
                    thick,
                    from: at(0.0, 0.0),
                    to: at(dx, dy),
                    dash,
                }
            }
            "circle" => {
                let [r, thick] = [0, 1].map(n);
                Op::Ellipse {
                    at: at(0.0, 0.0),
                    rx: r,
                    ry: r,
                    thick,
                    fill: arg(2) != Value::FALSE,
                }
            }
            "ellipse" => {
                let [rx, ry, thick] = [0, 1, 2].map(n);
                Op::Ellipse {
                    at: at(0.0, 0.0),
                    rx,
                    ry,
                    thick,
                    fill: arg(3) != Value::FALSE,
                }
            }
            "partial-ellipse" => {
                let [rx, ry, start, end, thick] = [0, 1, 2, 3, 4].map(n);
                let (connect, fill) = (arg(5) != Value::FALSE, arg(6) != Value::FALSE);
                Op::Arc {
                    at: at(0.0, 0.0),
                    rx,
                    ry,
                    start,
                    end,
                    thick,
                    connect,
                    fill,
                }
            }
            "polygon" => {
                let mut points = Vec::new();
                let mut coords = arg(0);
                while let (Some(x), Some(rest)) = (heap.car(coords), heap.cdr(coords)) {
                    let y = heap.car(rest).unwrap_or(Value::NULL);
                    points.push(at(number(heap, x), number(heap, y)));
                    coords = heap.cdr(rest).unwrap_or(Value::NULL);
                }
                Op::Polygon {
                    points,
                    blot: n(1),
                    fill: arg(2) != Value::FALSE,
                }
            }
            "path" => match path(heap, expr, (tx, ty)) {
                Ok(op) => op,
                Err(warning) => {
                    self.book.warnings.push(warning);
                    return Ok(true);
                }
            },
            "named-glyph" => {
                let (font, name) = (arg(0), arg(1));
                let (Some(metric), Some(name)) = (font::metric(heap, font), heap.str(name)) else {
                    self.book
                        .warnings
                        .push(String::from("named-glyph: no font metric and glyph name"));
                    return Ok(true);
                };
                let magnification = heap
                    .cdr(metric.description)
                    .and_then(|m| heap.to_f64(m))
                    .unwrap_or(1.0);
                let size = magnification * metric.font.design_size();
                let style = metric.font.postscript_name().to_lowercase();
                let name = String::from(name.as_str());
                match self.book.glyph(metric.font, &style, &name) {
                    Some(glyph) => Op::Glyph {
                        glyph,
                        at: at(0.0, 0.0),
                        scale: size / 1000.0,
                    },
                    None => return Ok(true),
                }
            }
            "glyph-string" => {
                self.glyph_string(vm, expr)?;
                self.next_horiz_adv = 0.0;
                return Ok(true);
            }
            "url-link" => {
                let url = heap
                    .str(arg(0))
                    .map(|s| String::from(s.as_str()))
                    .unwrap_or_default();
                let (x, y) = (pair(heap, arg(1)), pair(heap, arg(2)));
                Op::Link {
                    rect: [tx + x.0, ty + y.0, tx + x.1, ty + y.1],
                    url,
                }
            }
            "png-file" => {
                self.book
                    .warnings
                    .push(String::from("png-file: an image is not written"));
                return Ok(true);
            }
            _ => return Ok(false),
        };
        self.page.ops.push(op);
        Ok(true)
    }

    /// `glyph-string`: a music font's glyphs as outlines, as SVG's; a text
    /// font's as a run of its glyph ids.
    fn glyph_string(&mut self, vm: &mut Vm, expr: Value) -> Result<(), Exception> {
        let heap = vm.heap();
        let arg = |i: usize| nth(heap, expr, i + 1);
        let Some(ps_name) = heap.str(arg(1)).map(|s| String::from(s.as_str())) else {
            return Ok(());
        };
        let size = number(heap, arg(2)) / self.unit_length;
        let mut glyphs = Vec::new();
        let mut list = arg(4);
        while let (Some(glyph), Some(rest)) = (heap.car(list), heap.cdr(list)) {
            glyphs.push(glyph);
            list = rest;
        }
        let (text, mut clusters) = (arg(7), arg(8));
        let style = ps_name.to_lowercase();
        if let Some(font) = self.music_font(vm, &style)? {
            let heap = vm.heap();
            let Some(metric) = font::metric(heap, font) else {
                return Ok(());
            };
            for &glyph in &glyphs {
                let w = number(heap, nth(heap, glyph, 0));
                let (x, y) = (
                    number(heap, nth(heap, glyph, 2)),
                    number(heap, nth(heap, glyph, 3)),
                );
                let name = heap
                    .str(nth(heap, glyph, 5))
                    .map(|s| String::from(s.as_str()));
                if let Some(glyph) =
                    name.and_then(|name| self.book.glyph(metric.font, &style, &name))
                {
                    let (tx, ty) = self.translation;
                    let at = (tx + x + self.next_horiz_adv, ty + y);
                    self.page.ops.push(Op::Glyph {
                        glyph,
                        at,
                        scale: size / 1000.0,
                    });
                }
                self.next_horiz_adv += w;
            }
            return Ok(());
        }
        // A text font: each glyph's characters from its cluster, `(bytes . glyphs)`.
        let heap = vm.heap();
        let text = heap
            .str(text)
            .map(|s| String::from(s.as_str()))
            .unwrap_or_default();
        let mut texts: Vec<String> = Vec::with_capacity(glyphs.len());
        let mut byte = 0usize;
        while let (Some(cluster), Some(rest)) = (heap.car(clusters), heap.cdr(clusters)) {
            clusters = rest;
            let bytes = heap
                .car(cluster)
                .and_then(|b| heap.to_f64(b))
                .unwrap_or(0.0) as usize;
            let count = heap
                .cdr(cluster)
                .and_then(|c| heap.to_f64(c))
                .unwrap_or(0.0) as usize;
            let piece = text.get(byte..byte + bytes).unwrap_or("");
            byte += bytes;
            for i in 0..count {
                texts.push(if i == 0 {
                    String::from(piece)
                } else {
                    String::new()
                });
            }
        }
        let font = self.book.font(&ps_name);
        let mut run = Vec::with_capacity(glyphs.len());
        for (i, &glyph) in glyphs.iter().enumerate() {
            let advance = number(heap, nth(heap, glyph, 0));
            let offset = (
                number(heap, nth(heap, glyph, 2)),
                number(heap, nth(heap, glyph, 3)),
            );
            let gid = heap.to_f64(nth(heap, glyph, 4)).unwrap_or(0.0) as u16;
            let text = texts.get(i).cloned().unwrap_or_default();
            run.push(TextGlyph {
                gid,
                advance,
                offset,
                text,
            });
        }
        if !run.is_empty() {
            self.page.ops.push(Op::Text {
                font,
                size,
                at: self.translation,
                glyphs: run,
            });
        }
        Ok(())
    }

    /// The music font of the pack named `style`, through
    /// `ly:system-font-load`, once a page, as SVG's `font_of`: none for a
    /// text font, which no pack of the music's names.
    fn music_font(&mut self, vm: &mut Vm, style: &str) -> Result<Option<Value>, Exception> {
        if let Some((_, font)) = self.loaded.iter().find(|(s, _)| s == style) {
            return Ok(*font);
        }
        let load = vm
            .slot("lily", "ly:system-font-load")
            .and_then(|slot| vm.slot_value(slot));
        let font = match load {
            Some(load) => {
                let name = vm.heap_mut().string(style);
                vm.apply(load, &[name]).ok()
            }
            None => None,
        }
        .filter(|&f| font::metric(vm.heap(), f).is_some());
        self.loaded.push((String::from(style), font));
        Ok(font)
    }
}

impl Book {
    /// The place of `font`'s glyph `name` in the list, its outline added at
    /// its first use; none for a glyph without an outline, which draws
    /// nothing.
    fn glyph(&mut self, font: &lily_engine_fonts::Font, style: &str, name: &str) -> Option<u32> {
        let key = (String::from(style), String::from(name));
        if let Some(&g) = self.glyphs.get(&key) {
            return Some(g);
        }
        let Some(index) = font.glyphs().iter().position(|g| g.name == name) else {
            self.warnings.push(format!("no glyph {name} in {style}"));
            return None;
        };
        let outline: Vec<Segment> = font
            .outline(index)
            .map(|s| match s {
                lily_engine_fonts::Segment::Move(p) => Segment::Move(p),
                lily_engine_fonts::Segment::Line(p) => Segment::Line(p),
                lily_engine_fonts::Segment::Curve(a, b, c) => Segment::Curve(a, b, c),
            })
            .collect();
        if outline.is_empty() {
            return None;
        }
        let g = self.doc.glyphs.len() as u32;
        self.doc.glyphs.push(outline);
        self.glyphs.insert(key, g);
        Some(g)
    }

    fn font(&mut self, ps_name: &str) -> u32 {
        if let Some(&f) = self.fonts.get(ps_name) {
            return f;
        }
        let f = self.doc.fonts.len() as u32;
        self.doc.fonts.push(ps_name.to_string());
        self.fonts.insert(ps_name.to_string(), f);
        f
    }
}

/// `path`: `(path thick commands [cap join fill?])`, its commands made
/// absolute from `origin`.
fn path(heap: &Heap, expr: Value, origin: Offset) -> Result<Op, String> {
    let thick = number(heap, nth(heap, expr, 1));
    let style = |i: usize, styles: &[&str]| -> Result<u8, String> {
        let v = nth(heap, expr, i);
        if v == Value::NULL {
            return Ok(1);
        }
        match heap
            .name(v)
            .and_then(|n| styles.iter().position(|s| *s == n))
        {
            Some(s) => Ok(s as u8),
            None => Err(format!("unknown style {}", heap.name(v).unwrap_or("?"))),
        }
    };
    let cap = style(3, &["butt", "round", "square"])?;
    let join = style(4, &["miter", "round", "bevel"])?;
    let fill = nth(heap, expr, 5) != Value::FALSE && nth(heap, expr, 5) != Value::NULL;
    let mut commands = Vec::new();
    let (mut at, mut start) = ((0.0, 0.0), (0.0, 0.0));
    let mut list = nth(heap, expr, 2);
    while let Some(head) = heap.car(list) {
        let name = heap.name(head).unwrap_or("");
        let arity = match name {
            "rmoveto" | "rlineto" | "lineto" | "moveto" => 2,
            "rcurveto" | "curveto" => 6,
            "closepath" => 0,
            _ => return Err(format!("path: unknown command {name}")),
        };
        let mut rest = heap.cdr(list).unwrap_or(Value::NULL);
        let mut v = [0.0; 6];
        for x in v.iter_mut().take(arity) {
            *x = number(heap, heap.car(rest).unwrap_or(Value::NULL));
            rest = heap.cdr(rest).unwrap_or(Value::NULL);
        }
        let relative = name.starts_with('r');
        let base: Offset = if relative { at } else { (0.0, 0.0) };
        let p = |i: usize| (base.0 + v[i], base.1 + v[i + 1]);
        let page = |(x, y): Offset| (origin.0 + x, origin.1 + y);
        match arity {
            0 => {
                commands.push(Command::Close);
                at = start;
            }
            2 if name.ends_with("moveto") => {
                at = p(0);
                start = at;
                let (x, y) = page(at);
                commands.push(Command::Move(x, y));
            }
            2 => {
                at = p(0);
                let (x, y) = page(at);
                commands.push(Command::Line(x, y));
            }
            _ => {
                let (a, b, c) = (page(p(0)), page(p(2)), page(p(4)));
                at = p(4);
                commands.push(Command::Curve([a.0, a.1, b.0, b.1, c.0, c.1]));
            }
        }
        list = rest;
    }
    Ok(Op::Path {
        thick,
        cap,
        join,
        fill,
        commands,
    })
}

/// The item `i` of a list, or `()`.
fn nth(heap: &Heap, mut list: Value, i: usize) -> Value {
    for _ in 0..i {
        list = heap.cdr(list).unwrap_or(Value::NULL);
    }
    heap.car(list).unwrap_or(Value::NULL)
}

fn number(heap: &Heap, v: Value) -> f64 {
    heap.to_f64(v).unwrap_or(0.0)
}

fn pair(heap: &Heap, v: Value) -> Offset {
    let (a, d) = (
        heap.car(v).unwrap_or(Value::NULL),
        heap.cdr(v).unwrap_or(Value::NULL),
    );
    (number(heap, a), number(heap, d))
}
