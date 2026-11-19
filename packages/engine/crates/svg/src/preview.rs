//! 06 §6.2's preview form with §6.3–6.4's cells (W4, MS6 step 10, D253): the
//! pages a preview shows, and what of them an edit changed.
//!
//! - **An entry** is what the page's stencil holds below its
//!   `combine-stencil`s and `translate-stencil`s, a grob's print stencil or a
//!   primitive of a markup. It is written alone, in the file form's markup
//!   but for its glyphs, each a `<path>` in place.
//! - **A cell** is Y7's unit (`crates/layout`'s `cell.rs`): the entries of one
//!   line of one system, `cell::print_line`; the rest of a system (columns,
//!   `SpanBar`s, marks) and the rest of a page are a cell each. It is a
//!   `<g data-lily-cell="DIGEST" transform="translate(0, Y)">`, its entries
//!   written below the height of its topmost one, cells from the top of the
//!   page down: a pitch that moves the staves under it (T29's E1, 26 of its
//!   29 lines) moves their groups, not their markup.
//! - **A cell's order** is by layer, as LilyPond's, then by text: its
//!   `std::sort` is not stable, so an edit shuffles the grobs of a layer in
//!   lines it did not touch. The rest of a page keeps its order, in which a
//!   box may lie under its text.
//! - **An entry's digest** is of all a write reads: the expression, its
//!   numbers bit for bit, the links of the grobs in it, its place in its cell
//!   as it is printed (to four decimals, unless the expression moves it on)
//!   and the paper's scale. What the last compile wrote under a digest is
//!   taken ([`Markup`]), not written again; a digest that rounded a number
//!   could take a text whose fourth decimal is another (OPEN B32). An entry
//!   with something no digest covers (a promise, anything but numbers,
//!   strings, symbols and fonts), or that warned, is written every time, so
//!   a warning is said every time.
//! - **A cell's digest** is of its text, so that it does not depend on how
//!   the text was come by: a page is the same bytes whatever was reused.
//! - **Paint order:** grobs of two cells paint in cell order, not in
//!   LilyPond's by layer, which only a grob drawn over another line shows (a
//!   `whiteout` reaching a staff below); the file form keeps LilyPond's. A
//!   number is rounded twice, in the entry and in the group: the preview's
//!   places are within 0.0002 of a staff space of the file form's.
//! - **What is sent** ([`Markup::page`]): against the page of that name the
//!   last compile wrote, each cell's digest and height, in order, and the
//!   text of those it lacks. A new page, one of another size, one with no
//!   cell in common, or one with nothing changed, has none.
//! - **What is kept** is one copy of each entry, §4.5's 4 MB at most, the
//!   pages nearest the one shown first; a page beyond them is written again.

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use lily_engine_layout::{Offset, Stencil, cell, font, grob};
use lily_engine_scheme::{Exception, Heap, Tag, Value, Vm};

use crate::interpret::{interpret, nth, pair};
use crate::number::Fixed;
use crate::page::{TAIL, Writer, cause_link, head};
use crate::{Page, Paper};

/// README §4.5: the writer's kept markup, L's Must.
const KEPT_BYTES: usize = 4 << 20;

fn mix(h: u64, x: u64) -> u64 {
    (h ^ x).wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(23)
}

fn mix_bytes(mut h: u64, bytes: &[u8]) -> u64 {
    h = mix(h, bytes.len() as u64);
    let mut words = bytes.chunks_exact(8);
    for word in &mut words {
        let mut w = [0; 8];
        w.copy_from_slice(word);
        h = mix(h, u64::from_le_bytes(w));
    }
    words
        .remainder()
        .iter()
        .fold(h, |h, &b| mix(h, u64::from(b)))
}

/// A page's shape: what a page of the next compile is compared with.
#[derive(Clone, Debug, Default)]
struct Shape {
    /// The digest of what comes before its cells.
    head: u64,
    /// Its cells' digests, and the height each `translate`s to, in order.
    cells: Vec<(u64, Fixed)>,
}

/// A page written by the compile running.
struct Written {
    shape: Shape,
    /// Each cell's place in the page's text.
    spans: Vec<(usize, usize)>,
    /// What may be kept of it, by digest.
    entries: Vec<(u64, Rc<str>)>,
    report: PageReport,
}

/// What a page's writing did, for the harness.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageReport {
    pub name: String,
    /// Its cells, and those sent as a page's changed cells.
    pub cells: usize,
    pub sent: usize,
    /// Whether it was sent by its cells, and their bytes.
    pub replaced: bool,
    pub sent_bytes: usize,
    pub page_bytes: usize,
    /// Its entries, and those taken from the last compile.
    pub entries: usize,
    pub taken: usize,
}

/// What the writer keeps between compiles (06 §6.3): each entry's markup by
/// digest, and each page's shape by its name.
#[derive(Default)]
pub struct Markup {
    entries: BTreeMap<u64, Rc<str>>,
    pages: BTreeMap<String, Shape>,
    bytes: usize,
    written: Vec<Written>,
    reports: Vec<PageReport>,
}

impl Markup {
    /// A compile starts writing: what a compile stopped for the host wrote
    /// is forgotten.
    pub fn begin(&mut self) {
        self.written.clear();
    }

    /// How many pages the compile running has written, for [`Markup::forget`].
    pub fn mark(&self) -> usize {
        self.written.len()
    }

    /// Forgets the pages written since `mark`: an error took them, and the
    /// pages named after them are not theirs.
    pub fn forget(&mut self, mark: usize) {
        self.written.truncate(mark);
    }

    /// The page `index` of the compile running has its name: what its
    /// `.cells` output holds, if it is sent by its cells (protocol.ts): each
    /// cell's `digest:height`, in order, on a line, then the text of those
    /// the page of that name did not have.
    pub fn page(&mut self, index: usize, name: &str, svg: &str) -> Option<Vec<u8>> {
        let written = self.written.get_mut(index)?;
        written.report.name = String::from(name);
        let (out, sent, sent_bytes) = self.cells(index, name, svg)?;
        let report = &mut self.written[index].report;
        report.sent += sent;
        report.sent_bytes += sent_bytes;
        report.replaced = true;
        Some(out)
    }

    /// What [`Markup::page`] gives for the page `index`, its report left as
    /// it is: the visible page handed over before the compile's end (OPEN
    /// I7, D269), the same bytes as its `.cells` output at the end.
    pub fn peek(&self, index: usize, name: &str, svg: &str) -> Option<Vec<u8>> {
        self.cells(index, name, svg).map(|(out, _, _)| out)
    }

    /// The `.cells` of the page `index` named `name`, how many cells it
    /// sends and their bytes.
    fn cells(&self, index: usize, name: &str, svg: &str) -> Option<(Vec<u8>, usize, usize)> {
        let written = self.written.get(index)?;
        let shape = &written.shape;
        let old = self.pages.get(name).filter(|old| old.head == shape.head)?;
        let kept = |digest: u64| old.cells.iter().any(|&(d, _)| d == digest);
        if old.cells == shape.cells || !shape.cells.iter().any(|&(d, _)| kept(d)) {
            return None;
        }
        let mut out = String::new();
        for (i, (digest, y)) in shape.cells.iter().enumerate() {
            let space = if i == 0 { "" } else { " " };
            let _ = write!(out, "{space}{digest:016x}:{y}");
        }
        out.push('\n');
        let (mut sent, mut sent_bytes) = (0, 0);
        for (&(digest, _), &(from, to)) in shape.cells.iter().zip(&written.spans) {
            if !kept(digest) {
                out.push_str(&svg[from..to]);
                sent += 1;
                sent_bytes += to - from;
            }
        }
        Some((out.into_bytes(), sent, sent_bytes))
    }

    /// The compile ended, the page at `visible` shown: its pages are what
    /// the next compile is compared with. One that wrote no page changes
    /// nothing: the preview keeps showing the last.
    pub fn end(&mut self, visible: usize) {
        self.reports = self.written.iter().map(|w| w.report.clone()).collect();
        if self.written.is_empty() {
            return;
        }
        self.entries.clear();
        self.pages.clear();
        self.bytes = 0;
        let mut written: Vec<_> = core::mem::take(&mut self.written)
            .into_iter()
            .enumerate()
            .collect();
        written.sort_by_key(|(i, _)| i.abs_diff(visible));
        let mut room = true;
        for (_, w) in written {
            let bytes: usize = w.entries.iter().map(|(_, text)| text.len()).sum();
            room &= self.bytes + bytes <= KEPT_BYTES;
            if room {
                self.bytes += bytes;
                self.entries.extend(w.entries);
            }
            self.pages.insert(w.report.name, w.shape);
        }
    }

    /// What the last compile did, a page each.
    pub fn reports(&self) -> &[PageReport] {
        &self.reports
    }

    /// The bytes of markup kept for the next compile (README §4.5).
    pub fn kept_bytes(&self) -> usize {
        self.bytes
    }
}

/// `output-stencil`, in the preview form (see above): the page `stencil`,
/// which [`Markup::page`] names.
pub fn write(
    vm: &mut Vm,
    stencil: &Stencil,
    paper: &Paper<'_>,
    markup: &mut Markup,
) -> Result<Page, Exception> {
    let head = head(stencil, paper);
    let mut walk = Walk {
        writer: Writer::new(String::new(), paper, true),
        seed: mix(0, paper.output_scale.to_bits()),
        old: &markup.entries,
        cells: Vec::new(),
        lines: BTreeMap::new(),
        pending: Vec::new(),
        entries: Vec::new(),
        count: 0,
        taken: 0,
    };
    // The walk may force a promise, and a collection must not free the page.
    let depth = vm.heap().root_depth();
    vm.heap_mut().root(stencil.expr);
    let walked = walk
        .walk(vm, stencil.expr, (0.0, 0.0))
        .and_then(|()| walk.write(vm));
    vm.heap_mut().unroot_to(depth);
    walked?;
    let mut shape = Shape {
        head: mix_bytes(0, head.as_bytes()),
        cells: Vec::with_capacity(walk.cells.len()),
    };
    walk.cells.sort_by(|a, b| b.origin.total_cmp(&a.origin));
    let texts = walk.cells.iter().flat_map(|cell| &cell.texts);
    let size: usize = texts.map(|(_, text)| text.len()).sum();
    let mut svg = head;
    svg.reserve(size + walk.cells.len() * 64 + TAIL.len());
    let mut spans = Vec::with_capacity(walk.cells.len());
    for cell in walk.cells.iter_mut().filter(|cell| !cell.texts.is_empty()) {
        if cell.sorted {
            cell.texts.sort();
        }
        let sum = |h, (_, text): &(i32, Rc<str>)| mix_bytes(h, text.as_bytes());
        let mut digest = cell.texts.iter().fold(0, sum);
        // Two cells of a page alike are told apart: a digest names one.
        while shape.cells.iter().any(|&(d, _)| d == digest) {
            digest = mix(digest, 1);
        }
        let y = Fixed::of(-cell.origin);
        let from = svg.len();
        let _ = writeln!(
            svg,
            "<g data-lily-cell=\"{digest:016x}\" transform=\"translate(0, {y})\">"
        );
        for (_, text) in &cell.texts {
            svg.push_str(text);
        }
        svg.push_str("</g>\n");
        spans.push((from, svg.len()));
        shape.cells.push((digest, y));
    }
    svg.push_str(TAIL);
    let report = PageReport {
        cells: shape.cells.len(),
        page_bytes: svg.len(),
        entries: walk.count,
        taken: walk.taken,
        ..PageReport::default()
    };
    let (entries, warnings) = (walk.entries, walk.writer.warnings);
    markup.written.push(Written {
        shape,
        spans,
        entries,
        report,
    });
    Ok(Page { svg, warnings })
}

/// A cell being written.
struct Cell {
    /// The height its entries are written below: its topmost's.
    origin: f64,
    /// Whether its grobs are ordered, by layer and text (see above).
    sorted: bool,
    /// Each entry's layer and text.
    texts: Vec<(i32, Rc<str>)>,
}

/// An entry found, to be written once its cell's height is known.
struct Pending {
    expr: Value,
    o: Offset,
    cell: usize,
    layer: i32,
    /// Its digest but for its place, if a digest covers it.
    sum: Option<u64>,
    /// Whether it translates, rotates or scales what it holds ([`Sum::moves`]).
    moves: bool,
}

/// The walk of a page's stencil into its entries.
struct Walk<'a> {
    writer: Writer,
    seed: u64,
    /// What the last compile kept.
    old: &'a BTreeMap<u64, Rc<str>>,
    /// The cells, in the order they begin.
    cells: Vec<Cell>,
    /// The cell of each line, by the grob `cell::print_line` gives.
    lines: BTreeMap<u64, usize>,
    pending: Vec<Pending>,
    entries: Vec<(u64, Rc<str>)>,
    count: usize,
    taken: usize,
}

impl Walk<'_> {
    /// `interpret_stencil_expression` down to the entries.
    fn walk(&mut self, vm: &mut Vm, mut expr: Value, mut o: Offset) -> Result<(), Exception> {
        loop {
            let heap = vm.heap();
            let Some(head) = heap.car(expr) else {
                return Ok(());
            };
            match heap.name(head).unwrap_or("") {
                "footnote" => return Ok(()),
                "translate-stencil" => {
                    let (x, y) = pair(heap, nth(heap, expr, 1));
                    o = (o.0 + x, o.1 + y);
                    expr = nth(heap, expr, 2);
                }
                "combine-stencil" => {
                    let mut parts = heap.cdr(expr).unwrap_or(Value::NULL);
                    while let Some(part) = vm.heap().car(parts) {
                        self.walk(vm, part, o)?;
                        parts = vm.heap().cdr(parts).unwrap_or(Value::NULL);
                    }
                    return Ok(());
                }
                _ => return self.entry(vm, expr, o),
            }
        }
    }

    /// One entry found: its digest but for its place, and its cell.
    fn entry(&mut self, vm: &mut Vm, expr: Value, o: Offset) -> Result<(), Exception> {
        let mut sum = Sum {
            h: self.seed,
            grob: None,
            moves: false,
        };
        let summed = sum.value(vm, expr)?;
        // What is no grob's is the rest of the page: a cell of no line.
        let grob = sum
            .grob
            .and_then(|g| Some((g, cell::print_line(vm.heap_mut(), g)?)));
        let mut layer = 0;
        if let Some((g, _)) = grob {
            let value = grob::property(vm, g, "layer")?;
            let number = value.as_fixnum().and_then(|n| i32::try_from(n).ok());
            layer = number.unwrap_or(1);
        }
        let key = grob.map_or(u64::MAX, |(_, line)| line.to_bits());
        let cells = &mut self.cells;
        let at = *self.lines.entry(key).or_insert_with(|| {
            cells.push(Cell {
                origin: o.1,
                sorted: grob.is_some(),
                texts: Vec::new(),
            });
            cells.len() - 1
        });
        cells[at].origin = cells[at].origin.max(o.1);
        self.pending.push(Pending {
            expr,
            o,
            cell: at,
            layer,
            sum: summed.then_some(sum.h),
            moves: sum.moves,
        });
        Ok(())
    }

    /// Each entry found, taken from the last compile or written, in the
    /// page's order, in which a page says what it says.
    fn write(&mut self, vm: &mut Vm) -> Result<(), Exception> {
        for entry in core::mem::take(&mut self.pending) {
            let (o, cell) = (entry.o, &mut self.cells[entry.cell]);
            let y = o.1 - cell.origin;
            // As the writer prints it (`Writer::primitive`'s translation),
            // unless the expression adds to it first.
            let place = if entry.moves {
                mix(o.0.to_bits(), y.to_bits())
            } else {
                mix(Fixed::of(o.0).0 as u64, Fixed::of(-y).0 as u64)
            };
            let mut digest = entry.sum.map(|sum| mix(sum, place));
            self.count += 1;
            let text = match digest.and_then(|d| self.old.get(&d)) {
                Some(text) => {
                    self.taken += 1;
                    text.clone()
                }
                None => {
                    let warned = self.writer.warnings.len();
                    interpret(vm, entry.expr, &mut self.writer, (o.0, y))?;
                    if self.writer.warnings.len() > warned {
                        digest = None;
                    }
                    let text = Rc::from(self.writer.out.as_str());
                    self.writer.out.clear();
                    text
                }
            };
            if let Some(d) = digest {
                self.entries.push((d, text.clone()));
            }
            if !text.is_empty() {
                cell.texts.push((entry.layer, text));
            }
        }
        Ok(())
    }
}

/// What an entry's text is made from, summed: never where a value lives,
/// so that a compile from scratch sums what a reuse sums.
struct Sum {
    h: u64,
    /// The first grob in it, whose line is the entry's.
    grob: Option<Value>,
    /// Whether it holds a `translate-stencil`, `rotate-stencil` or
    /// `scale-stencil`, which a write adds to its place before printing it:
    /// then the place is summed bit for bit.
    moves: bool,
}

impl Sum {
    /// Adds `v`; false for what no digest covers.
    fn value(&mut self, vm: &mut Vm, v: Value) -> Result<bool, Exception> {
        let heap = vm.heap();
        self.h = mix(self.h, v.tag() as u64);
        match v.tag() {
            Tag::Pair => return self.list(vm, v),
            Tag::Null | Tag::False | Tag::True | Tag::Unspecified => {}
            Tag::Fixnum | Tag::Flonum | Tag::Bignum | Tag::Ratio => {
                let x = heap.to_f64(v).unwrap_or(0.0);
                self.h = mix(self.h, x.to_bits());
            }
            Tag::Symbol => self.h = mix_bytes(self.h, heap.name(v).unwrap_or("").as_bytes()),
            Tag::String => {
                let text = heap.str(v).map_or("", |s| s.as_str());
                self.h = mix_bytes(self.h, text.as_bytes());
            }
            Tag::Foreign => return Ok(self.font(heap, v)),
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// A font metric, as `named-glyph` reads it.
    fn font(&mut self, heap: &Heap, v: Value) -> bool {
        let Some(metric) = font::metric(heap, v) else {
            return false;
        };
        let magnified = heap.cdr(metric.description).and_then(|m| heap.to_f64(m));
        self.h = mix_bytes(self.h, metric.font.postscript_name().as_bytes());
        self.h = mix(self.h, metric.font.design_size().to_bits());
        self.h = mix(self.h, metric.magnification.to_bits());
        self.h = mix(self.h, magnified.unwrap_or(1.0).to_bits());
        true
    }

    /// A list: its items, and of a grob's `grob-cause` the link it writes,
    /// not the grob.
    fn list(&mut self, vm: &mut Vm, mut list: Value) -> Result<bool, Exception> {
        let heap = vm.heap();
        match heap.car(list).and_then(|head| heap.name(head)) {
            Some("delay-stencil-evaluation") => return Ok(false),
            Some("translate-stencil" | "rotate-stencil" | "scale-stencil") => self.moves = true,
            Some("grob-cause") => {
                let (grob, inner) = (nth(heap, list, 1), nth(heap, list, 2));
                self.grob.get_or_insert(grob);
                let link = cause_link(vm, grob)?;
                self.h = mix_bytes(self.h, link.as_deref().unwrap_or("").as_bytes());
                return self.value(vm, inner);
            }
            // Its Pango font is not read: the style and size after it are.
            Some("glyph-string") => {
                let font = heap.cdr(list).unwrap_or(Value::NULL);
                list = heap.cdr(font).unwrap_or(Value::NULL);
            }
            _ => {}
        }
        while let Some(item) = vm.heap().car(list) {
            if !self.value(vm, item)? {
                return Ok(false);
            }
            list = vm.heap().cdr(list).unwrap_or(Value::NULL);
        }
        // A dotted pair's end.
        self.value(vm, list)
    }
}
