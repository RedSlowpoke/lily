//! The outputter (`paper-outputter.cc`, `paper-outputter-scheme.cc`) with
//! `output-svg.scm`'s primitives and `framework-svg.scm`'s page, in 06
//! §6.2's file form (the preview form, `preview.rs`, differs only in its
//! glyphs, each a `<path>` in place):
//!
//! - one group around the page sets `fill="currentColor"` and round joins
//!   and caps, and the page's `<style>` styles every `<a>`; elements write
//!   those attributes only where they differ (`fill="none"`, other caps);
//! - numbers are [`Fixed`]'s;
//! - an unlinked `<use>`, `<rect>` or `<line>` carries its position in its
//!   own attributes, its `translate` group folded into them; the first
//!   element of a link keeps its group, as chapter 00's `<a
//!   xlink:href="textedit://…">` whose first child is `<g
//!   transform="translate(x, y)">` (`point-and-click.cc`'s link, 06 W-b);
//! - a link names its file by its path in the engine's tree;
//! - each glyph is defined once per page in a `<defs>`, at its first use,
//!   under 06 §3.2's id, its `d` the outline of W1's pack (06 §5.1), and
//!   drawn with `<use>`;
//! - text is `<text>` with Pango's description taken apart, the browser's
//!   to draw (D1).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write;

use lily_engine_fonts::{Font, Segment};
use lily_engine_layout::{Offset, Stencil, X, Y, font, grob};
use lily_engine_scheme::{Exception, Heap, Value, Vm, percent_encode};

use crate::interpret::{Call, Sink, interpret, nth, number, pair};
use crate::number::{Fixed, compact};
use crate::{Page, Paper};

/// `extract-glyph`'s `units-per-em`, which output-svg.scm does not read
/// from the font.
const UNITS_PER_EM: f64 = 1000.0;

/// What ends a page.
pub(crate) const TAIL: &str = "</g>\n</svg>\n";

/// `framework-svg.scm`'s `output-stencil`: the page `stencil` written whole.
pub fn write(vm: &mut Vm, stencil: &Stencil, paper: &Paper<'_>) -> Result<Page, Exception> {
    let mut writer = Writer::new(head(stencil, paper), paper, false);
    // The walk may force a promise, and a collection must not free the page.
    let depth = vm.heap().root_depth();
    vm.heap_mut().root(stencil.expr);
    let walked = interpret(vm, stencil.expr, &mut writer, (0.0, 0.0));
    vm.heap_mut().unroot_to(depth);
    walked?;
    writer.out.push_str(TAIL);
    Ok(Page {
        svg: writer.out,
        warnings: writer.warnings,
    })
}

/// `svg-begin`, with the style of every link and the page's group: what
/// comes before the page's elements.
pub(crate) fn head(stencil: &Stencil, paper: &Paper<'_>) -> String {
    let x = stencil.extent(X);
    let y = stencil.extent(Y);
    let (left_x, top_y) = (x.left, y.right);
    let device_width = interval_length(x.left, x.right);
    let device_height = interval_length(y.left, y.right);
    let mut out = String::with_capacity(1 << 14);
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" version=\"1.2\" width=\"{}mm\" height=\"{}mm\" viewBox=\"{} {} {} {}\">",
        Fixed::with(paper.output_scale * device_width, 2),
        Fixed::with(paper.output_scale * device_height, 2),
        Fixed::of(left_x),
        Fixed::of(-top_y),
        Fixed::of(device_width),
        Fixed::of(device_height),
    );
    out.push_str(
        "<style type=\"text/css\">\n<![CDATA[\ntspan { white-space: pre; }\na { color: inherit }\n]]>\n</style>\n",
    );
    out.push_str("<g fill=\"currentColor\" stroke-linejoin=\"round\" stroke-linecap=\"round\">\n");
    out
}

/// `interval-length`.
fn interval_length(left: f64, right: f64) -> f64 {
    (right - left).max(0.0)
}

/// `Paper_outputter` with `output-svg.scm`'s state.
pub(crate) struct Writer {
    pub(crate) out: String,
    /// Whether glyphs are drawn in place, the preview form (06 §6.2).
    preview: bool,
    /// `lily-unit-length`.
    unit_length: f64,
    /// The tree's roots, percent-encoded as a link's path is.
    roots: Vec<String>,
    /// The last `settranslation`'s offset.
    translation: Offset,
    /// `have-grob-cause?`.
    linked: bool,
    /// Whether the next element is a link's first, which keeps its group.
    first_in_link: bool,
    /// `next-horiz-adv`.
    next_horiz_adv: f64,
    /// The ids of the glyphs this page has defined.
    defined: BTreeSet<String>,
    /// The fonts of glyph strings, by `font-name-style`: the font metric of
    /// the pack, or none.
    fonts: Vec<(String, Option<Value>)>,
    pub(crate) warnings: Vec<String>,
}

/// What a primitive wrote, and where its position is.
enum Placed {
    /// Its position is in its own attributes.
    Folded,
    /// It is drawn at the origin of the `translate` group around it.
    Grouped,
}

impl Sink for Writer {
    fn output(&mut self, vm: &mut Vm, call: Call) -> Result<bool, Exception> {
        let mut taken = true;
        match call {
            Call::SetTranslation(o) => self.translation = o,
            Call::ResetTranslation => {}
            Call::Primitive(expr) => taken = self.primitive(vm, expr)?,
            Call::GrobCause(_, grob) => self.grob_cause(vm, grob)?,
            Call::NoOrigin => {
                if self.linked {
                    self.linked = false;
                    self.out.push_str("</a>\n");
                }
                self.first_in_link = false;
            }
            Call::SetColor(rgba) => {
                let heap = vm.heap();
                let [r, g, b, a] = rgba.map(|v| Fixed::of(100.0 * number(heap, v)));
                self.open(format!("<g color=\"rgba({r}%, {g}%, {b}%, {a}%)\">\n"));
            }
            Call::SetRotation(angle, x, y) => {
                let angle = number(vm.heap(), angle);
                self.open(format!(
                    "<g transform=\"rotate({}, {}, {})\">\n",
                    Fixed::of(-angle),
                    Fixed::of(x),
                    Fixed::of(-y)
                ));
            }
            Call::SetScale(x, y) => {
                let heap = vm.heap();
                self.open(format!(
                    "<g transform=\"scale({}, {})\">\n",
                    Fixed::of(number(heap, x)),
                    Fixed::of(number(heap, y))
                ));
            }
            Call::StartGroupNode(attributes) => {
                let text = start_group_node(vm.heap(), attributes);
                self.open(text);
            }
            Call::ResetColor | Call::ResetRotation | Call::ResetScale | Call::EndGroupNode => {
                self.open(String::from("</g>\n"));
            }
        }
        Ok(taken)
    }
}

impl Writer {
    pub(crate) fn new(out: String, paper: &Paper<'_>, preview: bool) -> Writer {
        Writer {
            out,
            preview,
            // (set-unit-length output-scale), first on every page.
            unit_length: paper.output_scale,
            roots: paper
                .tree_roots
                .iter()
                .map(|root| percent_encode(root.trim_end_matches('/')))
                .collect(),
            translation: (0.0, 0.0),
            linked: false,
            first_in_link: false,
            next_horiz_adv: 0.0,
            defined: BTreeSet::new(),
            fonts: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Writes `text`, after which no element is a link's first.
    fn open(&mut self, text: String) {
        self.out.push_str(&text);
        self.first_in_link = false;
    }

    /// `grob-cause`: the link of a grob that has one. Its file is named by
    /// its absolute path, which the engine's names, below its root, are
    /// once `/` begins them (`ly-getcwd` is empty).
    fn grob_cause(&mut self, vm: &mut Vm, grob: Value) -> Result<(), Exception> {
        let Some(link) = cause_link(vm, grob)? else {
            return Ok(());
        };
        let link = self.tree_link(&link);
        self.linked = true;
        self.first_in_link = true;
        let _ = writeln!(self.out, "<a xlink:href=\"{link}\">");
        Ok(())
    }

    /// `link` with its file named by its path in the engine's tree: below
    /// the root it is in, `/` for the root (06 §6.2).
    fn tree_link(&self, link: &str) -> String {
        let Some(path) = link.strip_prefix("textedit://") else {
            return String::from(link);
        };
        for root in &self.roots {
            if let Some(rest) = path
                .strip_prefix(root.as_str())
                .filter(|rest| rest.starts_with('/'))
            {
                return format!("textedit://{rest}");
            }
        }
        String::from(link)
    }

    /// The primitive `expr` written at the last translation, as
    /// `stencil-dispatch-alist`'s procedure for its head writes it; false
    /// for a head it has none for, which writes nothing.
    fn primitive(&mut self, vm: &mut Vm, expr: Value) -> Result<bool, Exception> {
        let (tx, ty) = (
            Fixed::of(self.translation.0),
            Fixed::of(-self.translation.1),
        );
        // A link's first element keeps its translate group (06 W-b).
        let fold = (!self.first_in_link).then_some((tx, ty));
        let mut element = String::new();
        let heap = vm.heap();
        let head = heap.car(expr).and_then(|h| heap.name(h)).unwrap_or("");
        let args = |i: usize| nth(heap, expr, i + 1);
        let placed = match head {
            "round-filled-box" => {
                let [b, w, d, h, blot] = [0, 1, 2, 3, 4].map(|i| number(heap, args(i)));
                let (mut x, mut y) = (Fixed::of(-b), Fixed::of(-h));
                if let Some((fx, fy)) = fold {
                    (x, y) = (x + fx, y + fy);
                }
                let _ = writeln!(
                    element,
                    "<rect x=\"{x}\" y=\"{y}\" width=\"{}\" height=\"{}\" ry=\"{}\"/>",
                    Fixed::of(b + w),
                    Fixed::of(d + h),
                    Fixed::of(blot / 2.0)
                );
                placed(fold)
            }
            "draw-line" => {
                let [thick, x1, y1, x2, y2] = [0, 1, 2, 3, 4].map(|i| number(heap, args(i)));
                line(&mut element, fold, thick, (x1, y1), (x2, y2), None);
                placed(fold)
            }
            "dashed-line" => {
                let [thick, _, _, dx, dy] = [0, 1, 2, 3, 4].map(|i| number(heap, args(i)));
                // `(format #f "~a,~a" on off)`: Guile's display of each.
                let mut dash = String::new();
                heap.display(args(1), &mut dash);
                dash.push(',');
                heap.display(args(2), &mut dash);
                line(
                    &mut element,
                    fold,
                    thick,
                    (0.0, 0.0),
                    (dx, dy),
                    Some(&compact(&dash)),
                );
                placed(fold)
            }
            "circle" => {
                let [radius, thick] = [0, 1].map(|i| number(heap, args(i)));
                let _ = writeln!(
                    element,
                    "<circle{} stroke=\"currentColor\" stroke-width=\"{}\" r=\"{}\"/>",
                    fill(args(2)),
                    Fixed::of(thick),
                    Fixed::of(radius)
                );
                Placed::Grouped
            }
            "ellipse" => {
                let [rx, ry, thick] = [0, 1, 2].map(|i| number(heap, args(i)));
                let _ = writeln!(
                    element,
                    "<ellipse{} stroke=\"currentColor\" stroke-width=\"{}\" rx=\"{}\" ry=\"{}\"/>",
                    fill(args(3)),
                    Fixed::of(thick),
                    Fixed::of(rx),
                    Fixed::of(ry)
                );
                Placed::Grouped
            }
            "partial-ellipse" => {
                let [rx, ry, start, end, thick] = [0, 1, 2, 3, 4].map(|i| number(heap, args(i)));
                let connect = args(5) != Value::FALSE;
                partial_ellipse(
                    &mut element,
                    rx,
                    ry,
                    start,
                    end,
                    thick,
                    connect,
                    fill(args(6)),
                );
                Placed::Grouped
            }
            "polygon" => {
                let blot = number(heap, args(1));
                let mut points = Vec::new();
                let mut coords = args(0);
                while let (Some(x), Some(rest)) = (heap.car(coords), heap.cdr(coords)) {
                    let y = heap.car(rest).unwrap_or(Value::NULL);
                    points.push(point(number(heap, x), number(heap, y)));
                    coords = heap.cdr(rest).unwrap_or(Value::NULL);
                }
                let _ = writeln!(
                    element,
                    "<polygon stroke-width=\"{}\"{} stroke=\"currentColor\" points=\"{}\"/>",
                    Fixed::of(blot),
                    fill(args(2)),
                    points.join(" ")
                );
                Placed::Grouped
            }
            "path" => {
                match path(heap, expr) {
                    Ok(text) => element = text,
                    Err(warning) => self.warnings.push(warning),
                }
                Placed::Grouped
            }
            "named-glyph" => {
                let (font, name) = (args(0), args(1));
                if let (Some(metric), Some(name)) = (font::metric(heap, font), heap.str(name)) {
                    // font-smob-to-text: `modified-font-metric-font-scaling`.
                    let magnification = heap
                        .cdr(metric.description)
                        .and_then(|m| heap.to_f64(m))
                        .unwrap_or(1.0);
                    let size = magnification * metric.font.design_size();
                    let style = metric.font.postscript_name().to_lowercase();
                    let name = String::from(name.as_str());
                    self.glyph(&mut element, metric.font, &style, size, &name, None, fold);
                } else {
                    self.warnings
                        .push(String::from("named-glyph: no font metric and glyph name"));
                }
                placed(fold)
            }
            "glyph-string" => {
                let style = heap.str(args(1)).map(|s| s.as_str().to_lowercase());
                let size = number(heap, args(2));
                let mut glyphs = Vec::new();
                let mut list = args(4);
                while let (Some(glyph), Some(rest)) = (heap.car(list), heap.cdr(list)) {
                    glyphs.push(glyph);
                    list = rest;
                }
                if let Some(style) = style {
                    self.glyph_string(vm, &mut element, &style, size, &glyphs, fold)?;
                }
                self.next_horiz_adv = 0.0;
                placed(fold)
            }
            "utf-8-string" => {
                let (description, text) = (args(0), args(1));
                match (heap.str(description), heap.str(text)) {
                    (Some(description), Some(text)) => {
                        let text = text.as_str().replace('&', "&amp;").replace('<', "&lt;");
                        let tspan = if text.is_empty() {
                            String::from("<tspan/>\n")
                        } else {
                            format!("<tspan>{text}</tspan>\n")
                        };
                        match pango_description_to_text(description.as_str(), self.unit_length) {
                            Some(attributes) => {
                                let _ = writeln!(element, "<text{attributes}>\n{tspan}</text>");
                            }
                            None => self.warnings.push(format!(
                                "cannot decypher Pango description: {}",
                                description.as_str()
                            )),
                        }
                    }
                    _ => self
                        .warnings
                        .push(String::from("utf-8-string: no description and text")),
                }
                Placed::Grouped
            }
            "url-link" => {
                let url = heap
                    .str(args(0))
                    .map(|s| String::from(s.as_str()))
                    .unwrap_or_default();
                let (x, y) = (pair(heap, args(1)), pair(heap, args(2)));
                let _ = writeln!(
                    element,
                    "<a xlink:href=\"{url}\">\n<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"none\" stroke-width=\"0\"/>\n</a>",
                    Fixed::of(x.0),
                    Fixed::of(y.0),
                    Fixed::of(x.1 - x.0),
                    Fixed::of(y.1 - y.0)
                );
                Placed::Grouped
            }
            // An image's bytes are the host's file; not written yet (W3's
            // pages draw none).
            "png-file" => {
                self.warnings
                    .push(String::from("png-file: an image is not written"));
                Placed::Grouped
            }
            // Heads stencil-dispatch-alist has no procedure for, as
            // embedded-svg, write their group and nothing in it.
            _ => {
                self.write_placed(&element, Placed::Grouped, (tx, ty));
                return Ok(false);
            }
        };
        self.write_placed(&element, placed, (tx, ty));
        Ok(true)
    }

    /// Writes `element` in its `translate` group or folded; nothing for
    /// nothing written, unless it is a link's first, whose group is the
    /// link's position.
    fn write_placed(&mut self, element: &str, placed: Placed, (tx, ty): (Fixed, Fixed)) {
        let first = core::mem::replace(&mut self.first_in_link, false);
        if element.is_empty() && !first {
            return;
        }
        match placed {
            Placed::Folded => self.out.push_str(element),
            Placed::Grouped => {
                let _ = writeln!(
                    self.out,
                    "<g transform=\"translate({tx}, {ty})\">\n{element}</g>"
                );
            }
        }
    }

    /// `glyph-string` of a music font: `music-string-to-path` of each glyph,
    /// each at its offset after the advance of the ones before it.
    fn glyph_string(
        &mut self,
        vm: &mut Vm,
        element: &mut String,
        style: &str,
        size: f64,
        glyphs: &[Value],
        fold: Option<(Fixed, Fixed)>,
    ) -> Result<(), Exception> {
        let Some(font) = self.font_of(vm, style)? else {
            self.warnings
                .push(format!("cannot find SVG font {style}.svg"));
            return Ok(());
        };
        let heap = vm.heap();
        let Some(metric) = font::metric(heap, font) else {
            return Ok(());
        };
        let scaled = size / self.unit_length;
        for &glyph in glyphs {
            // (w (h . d) x y index name): `extract-glyph-info`.
            let w = number(heap, nth(heap, glyph, 0));
            let (x, y) = (
                number(heap, nth(heap, glyph, 2)),
                number(heap, nth(heap, glyph, 3)),
            );
            let mut last = glyph;
            while let Some(rest) = heap.cdr(last).filter(|r| heap.car(*r).is_some()) {
                last = rest;
            }
            let name = heap
                .car(last)
                .and_then(|n| heap.str(n))
                .map(|s| String::from(s.as_str()));
            if let Some(name) = name {
                // dump-path: (+ dx next-horiz-adv) and dy, not negated.
                let at = (Fixed::of(x + self.next_horiz_adv), Fixed::of(y));
                self.glyph(element, metric.font, style, scaled, &name, Some(at), fold);
            }
            self.next_horiz_adv += w;
        }
        Ok(())
    }

    /// The font metric of the pack named `style`, through
    /// `ly:system-font-load`, once a page.
    fn font_of(&mut self, vm: &mut Vm, style: &str) -> Result<Option<Value>, Exception> {
        if let Some((_, font)) = self.fonts.iter().find(|(s, _)| s == style) {
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
        };
        self.fonts.push((String::from(style), font));
        Ok(font)
    }

    /// One glyph of `font`, `style` its `font-name-style`, at `size`: its
    /// `<defs>` at its first use on the page, and its `<use>`, at `at` in
    /// the group (a glyph string's) and folded into its attributes when
    /// `fold` is the group's translation.
    #[allow(clippy::too_many_arguments)]
    fn glyph(
        &mut self,
        element: &mut String,
        font: &Font,
        style: &str,
        size: f64,
        name: &str,
        at: Option<(Fixed, Fixed)>,
        fold: Option<(Fixed, Fixed)>,
    ) {
        let Some(index) = font.glyphs().iter().position(|g| g.name == name) else {
            self.warnings.push(format!("no glyph {name} in {style}"));
            return;
        };
        let mut d = String::new();
        outline(&mut d, font, index);
        // A glyph without an outline, a space, draws nothing.
        if d.is_empty() {
            return;
        }
        // extract-glyph's font-scale, `~4f`, which 06 §3.2's id names.
        let scale = size / UNITS_PER_EM;
        let s = Fixed::of(scale);
        let (x, y) = match (at, fold) {
            (Some((ax, ay)), Some((fx, fy))) => (Some(ax + fx), Some(ay + fy)),
            (None, Some((fx, fy))) => (Some(fx), Some(fy)),
            (Some((ax, ay)), None) if ax != Fixed::ZERO || ay != Fixed::ZERO => {
                (Some(ax), Some(ay))
            }
            _ => (None, None),
        };
        // The preview form draws the glyph here, the `<defs>`' path moved.
        if self.preview {
            element.push_str("<path transform=\"");
            if let (Some(x), Some(y)) = (x, y) {
                let _ = write!(element, "translate({x}, {y}) ");
            }
            let _ = writeln!(element, "scale({s}, {})\" d=\"{d}\"/>", -s);
            return;
        }
        let printed = format!("{scale:.4}");
        let id = format!("lily-g-{}-{}-{printed}", id_part(style), id_part(name));
        if self.defined.insert(id.clone()) {
            let _ = writeln!(
                element,
                "<defs><path id=\"{id}\" transform=\"scale({s}, {})\" d=\"{d}\"/></defs>",
                -s
            );
        }
        let _ = write!(element, "<use xlink:href=\"#{id}\"");
        if let (Some(x), Some(y)) = (x, y) {
            let _ = write!(element, " x=\"{x}\" y=\"{y}\"");
        }
        element.push_str("/>\n");
    }
}

/// The link of the grob `grob-cause` holds. The references' data gives it
/// as the string LilyPond wrote (`crates/layout`'s `dump.rs`); a grob of the
/// engine's as layout's [`grob::cause_link`] makes it, from its cause's
/// origin.
pub(crate) fn cause_link(vm: &mut Vm, grob: Value) -> Result<Option<String>, Exception> {
    match vm.heap().str(grob) {
        Some(link) => Ok(Some(String::from(link.as_str()))),
        None => grob::cause_link(vm, grob),
    }
}

/// Where an element that folds is: in its attributes when `fold` is given.
fn placed(fold: Option<(Fixed, Fixed)>) -> Placed {
    if fold.is_some() {
        Placed::Folded
    } else {
        Placed::Grouped
    }
}

/// draw-line's `<line>`, its ends moved by `fold`.
fn line(
    out: &mut String,
    fold: Option<(Fixed, Fixed)>,
    thick: f64,
    (x1, y1): (f64, f64),
    (x2, y2): (f64, f64),
    dash: Option<&str>,
) {
    let (fx, fy) = fold.unwrap_or((Fixed::ZERO, Fixed::ZERO));
    let _ = write!(
        out,
        "<line stroke-width=\"{}\" stroke=\"currentColor\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"",
        Fixed::of(thick),
        Fixed::of(x1) + fx,
        Fixed::of(-y1) + fy,
        Fixed::of(x2) + fx,
        Fixed::of(-y2) + fy
    );
    if let Some(dash) = dash {
        let _ = write!(out, " stroke-dasharray=\"{dash}\"");
    }
    out.push_str("/>\n");
}

/// ` fill="none"` for what is not filled; the page's group fills the rest.
fn fill(filled: Value) -> &'static str {
    if filled == Value::FALSE {
        " fill=\"none\""
    } else {
        ""
    }
}

/// `offset->point`: `x -y`.
fn point(x: f64, y: f64) -> String {
    format!("{} {}", Fixed::of(x), Fixed::of(-y))
}

/// `partial-ellipse`.
#[allow(clippy::too_many_arguments)]
fn partial_ellipse(
    out: &mut String,
    rx: f64,
    ry: f64,
    start: f64,
    end: f64,
    thick: f64,
    connect: bool,
    fill: &str,
) {
    use core::f64::consts::PI;
    let pi_over_180 = PI / 180.0;
    let radius = |angle: f64| {
        (rx * ry)
            / libm::sqrt(
                (ry * ry) * (libm::cos(angle) * libm::cos(angle))
                    + (rx * rx) * (libm::sin(angle) * libm::sin(angle)),
            )
    };
    let start = pi_over_180 * angle_0_360(start);
    let start_radius = radius(start);
    let end = pi_over_180 * angle_0_360(end);
    let end_radius = radius(end);
    let epsilon = 1.5e-3;
    let x_end = end_radius * libm::cos(end) - start_radius * libm::cos(start);
    let y_end = end_radius * libm::sin(end) - start_radius * libm::sin(start);
    if x_end.abs() < epsilon && y_end.abs() < epsilon {
        let _ = writeln!(
            out,
            "<ellipse{fill} stroke=\"currentColor\" stroke-width=\"{}\" cx=\"0\" cy=\"0\" rx=\"{}\" ry=\"{}\"/>",
            Fixed::of(thick),
            Fixed::of(rx),
            Fixed::of(ry)
        );
        return;
    }
    let (sx, sy) = (
        start_radius * libm::cos(start),
        -(start_radius * libm::sin(start)),
    );
    let _ = write!(
        out,
        "<path{fill} stroke=\"currentColor\" stroke-width=\"{}\" d=\"M{} {}A{} {} 0 {} 0 {} {}",
        Fixed::of(thick),
        Fixed::of(sx),
        Fixed::of(sy),
        Fixed::of(rx),
        Fixed::of(ry),
        if 0.0 > start - end { 0 } else { 1 },
        Fixed::of(end_radius * libm::cos(end)),
        Fixed::of(-(end_radius * libm::sin(end)))
    );
    if connect {
        let _ = write!(out, "L{},{}", Fixed::of(sx), Fixed::of(sy));
    }
    out.push_str("\"/>\n");
}

/// `angle-0-360`: `cyclic-base-value` of 360.
fn angle_0_360(mut angle: f64) -> f64 {
    while angle < 0.0 {
        angle += 360.0;
    }
    while angle >= 360.0 {
        angle -= 360.0;
    }
    angle
}

/// `path`: `(path thick commands [cap join fill?])`, its commands as SVG's.
fn path(heap: &Heap, expr: Value) -> Result<String, String> {
    let thick = number(heap, nth(heap, expr, 1));
    let style = |i: usize, styles: &[&'static str]| -> Result<&'static str, String> {
        let v = nth(heap, expr, i);
        if v == Value::NULL {
            return Ok("round");
        }
        match heap.name(v).and_then(|n| styles.iter().find(|s| **s == n)) {
            Some(s) => Ok(s),
            None => Err(format!("unknown style {}", heap.name(v).unwrap_or("?"))),
        }
    };
    let cap = style(3, &["butt", "round", "square"])?;
    let join = style(4, &["miter", "round", "bevel"])?;
    let filled = nth(heap, expr, 5) != Value::FALSE && nth(heap, expr, 5) != Value::NULL;
    let mut d = String::new();
    let mut commands = nth(heap, expr, 2);
    while let Some(head) = heap.car(commands) {
        let name = heap.name(head).unwrap_or("");
        let (svg, arity) = match name {
            "rmoveto" => ("m", 2),
            "rlineto" => ("l", 2),
            "lineto" => ("L", 2),
            "moveto" => ("M", 2),
            "rcurveto" => ("c", 6),
            "curveto" => ("C", 6),
            "closepath" => ("z", 0),
            _ => return Err(format!("path: unknown command {name}")),
        };
        d.push_str(svg);
        let mut rest = heap.cdr(commands).unwrap_or(Value::NULL);
        let mut points = Vec::new();
        for _ in 0..arity / 2 {
            let x = number(heap, nth(heap, rest, 0));
            let y = number(heap, nth(heap, rest, 1));
            points.push(point(x, y));
            rest = heap
                .cdr(heap.cdr(rest).unwrap_or(Value::NULL))
                .unwrap_or(Value::NULL);
        }
        d.push_str(&points.join(" "));
        commands = rest;
    }
    let mut out = format!("<path stroke-width=\"{}\"", Fixed::of(thick));
    if join != "round" {
        let _ = write!(out, " stroke-linejoin=\"{join}\"");
    }
    if cap != "round" {
        let _ = write!(out, " stroke-linecap=\"{cap}\"");
    }
    out.push_str(" stroke=\"currentColor\"");
    if !filled {
        out.push_str(" fill=\"none\"");
    }
    let _ = writeln!(out, " d=\"{d}\"/>");
    Ok(out)
}

/// `start-group-node`: `<g>` with each attribute once, the first of a
/// name, as `ly:format`'s `~a` writes the value.
fn start_group_node(heap: &Heap, attributes: Value) -> String {
    let mut out = String::from("<g");
    let mut seen: Vec<Value> = Vec::new();
    let mut list = attributes;
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        list = rest;
        let (Some(key), Some(value)) = (heap.car(item), heap.cdr(item)) else {
            continue;
        };
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let _ = write!(
            out,
            " {}=\"{}\"",
            format_a(heap, key),
            format_a(heap, value)
        );
    }
    out.push_str(">\n");
    out
}

/// `ly:format`'s `~a`: an exact integer's digits, another number with eight
/// decimals (compacted), a string as it is, a symbol's name.
fn format_a(heap: &Heap, v: Value) -> String {
    if let Some(s) = heap.str(v) {
        return String::from(s.as_str());
    }
    if let Some(name) = heap.name(v) {
        return String::from(name);
    }
    if let Some(n) = v.as_fixnum() {
        return n.to_string();
    }
    match heap.to_f64(v) {
        Some(x) if x.is_finite() => compact(&format!("{x:.8}")),
        Some(_) => String::from("0"),
        None => String::new(),
    }
}

/// `pango-description-to-text`'s attributes: `"LilyPond Serif Bold 3.866"`
/// as its family, weight, style, variant and size, the size in staff
/// spaces; none for a description it cannot take apart.
fn pango_description_to_text(description: &str, unit_length: f64) -> Option<String> {
    // `( Bold)?( Italic)?( Small-Caps)?[ -]([0-9.]+)$`, after a comma when
    // the first of output-svg.scm's two expressions matches.
    let digits = description
        .bytes()
        .rev()
        .take_while(|b| b.is_ascii_digit() || *b == b'.')
        .count();
    if digits == 0 {
        return None;
    }
    let size_at = description.len() - digits;
    let size = &description[size_at..];
    let mut rest = description[..size_at]
        .strip_suffix(' ')
        .or_else(|| description[..size_at].strip_suffix('-'))?;
    let mut flags = [false; 3];
    for (i, word) in [" Small-Caps", " Italic", " Bold"].into_iter().enumerate() {
        if let Some(r) = rest.strip_suffix(word) {
            rest = r;
            flags[2 - i] = true;
        }
    }
    let family = rest.strip_suffix(',').unwrap_or(rest);
    let size: f64 = size.parse().ok()?;
    let mut out = format!(" font-family=\"{family}\"");
    if flags[0] {
        out.push_str(" font-weight=\"bold\"");
    }
    if flags[1] {
        out.push_str(" font-style=\"italic\"");
    }
    if flags[2] {
        out.push_str(" font-variant=\"small-caps\"");
    }
    let _ = write!(
        out,
        " font-size=\"{}\" text-anchor=\"start\"",
        Fixed::of(size / unit_length)
    );
    Some(out)
}

/// 06 §3.2's part of an id: letters, digits, `.` and `-` as they are,
/// anything else as `_hex_`.
fn id_part(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_alphanumeric() || c == '.' || c == '-' {
            out.push(c);
        } else {
            let _ = write!(out, "_{:x}_", c as u32);
        }
    }
    out
}

/// The glyph's outline in W1's pack (06 §5.1), in font units, as a path's
/// `d`: each contour from an absolute `M`, its pieces relative, closed.
fn outline(d: &mut String, font: &Font, index: usize) {
    let mut at = [0i32; 2];
    let mut open = false;
    for segment in font.outline(index) {
        match segment {
            Segment::Move(p) => {
                if open {
                    d.push('z');
                }
                open = true;
                d.push('M');
                numbers(d, &[p[0], p[1]]);
                at = p;
            }
            Segment::Line(p) => {
                let (dx, dy) = (p[0] - at[0], p[1] - at[1]);
                if dy == 0 {
                    d.push('h');
                    numbers(d, &[dx]);
                } else if dx == 0 {
                    d.push('v');
                    numbers(d, &[dy]);
                } else {
                    d.push('l');
                    numbers(d, &[dx, dy]);
                }
                at = p;
            }
            Segment::Curve(a, b, c) => {
                d.push('c');
                numbers(
                    d,
                    &[
                        a[0] - at[0],
                        a[1] - at[1],
                        b[0] - at[0],
                        b[1] - at[1],
                        c[0] - at[0],
                        c[1] - at[1],
                    ],
                );
                at = c;
            }
        }
    }
    if open {
        d.push('z');
    }
}

/// Integers after a path command, a space before each but the first and
/// the negative.
fn numbers(d: &mut String, ns: &[i32]) {
    for (i, n) in ns.iter().enumerate() {
        if i > 0 && *n >= 0 {
            d.push(' ');
        }
        let _ = write!(d, "{n}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pango_descriptions_taken_apart() {
        let unit = 1.757299;
        assert_eq!(
            pango_description_to_text("LilyPond Serif 3.866", unit).unwrap(),
            " font-family=\"LilyPond Serif\" font-size=\"2.2\" text-anchor=\"start\""
        );
        assert_eq!(
            pango_description_to_text("LilyPond Serif Bold Italic 3.866", unit).unwrap(),
            " font-family=\"LilyPond Serif\" font-weight=\"bold\" font-style=\"italic\" font-size=\"2.2\" text-anchor=\"start\""
        );
        assert_eq!(
            pango_description_to_text("C059, Italic Small-Caps 3.866", unit).unwrap(),
            " font-family=\"C059\" font-style=\"italic\" font-variant=\"small-caps\" font-size=\"2.2\" text-anchor=\"start\""
        );
        assert_eq!(
            pango_description_to_text("Foo-3.866", unit).unwrap(),
            " font-family=\"Foo\" font-size=\"2.2\" text-anchor=\"start\""
        );
        // Bold before Italic only: "Italic Bold" leaves Italic in the family.
        assert!(
            pango_description_to_text("A Italic Bold 3", unit)
                .unwrap()
                .contains("font-family=\"A Italic\"")
        );
        assert_eq!(pango_description_to_text("LilyPond Serif", unit), None);
    }

    #[test]
    fn ids_as_06_3_2_names_them() {
        assert_eq!(id_part("noteheads.s2"), "noteheads.s2");
        assert_eq!(id_part("a b/c"), "a_20_b_2f_c");
    }

    #[test]
    fn integers_of_a_path() {
        let mut d = String::new();
        numbers(&mut d, &[0, -46, 91, 116]);
        assert_eq!(d, "0-46 91 116");
    }
}
