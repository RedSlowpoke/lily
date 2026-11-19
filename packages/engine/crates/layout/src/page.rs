//! A book's pages (`page-breaking.cc`, part 5's, MS3 step 35): a page
//! drawn from a set of systems (`Page_breaking::draw_page`, `:556–613`)
//! and the pages of a book whose lines are already divided among them
//! (`Page_breaking::make_pages`, `:615–744`), over the page-layout problem
//! (`page_layout_problem.rs`' prelude). Where to break, which gives
//! `make_pages` its lines per page, is the page breaker's
//! (`optimal_page_breaking.rs`, MS3 step 62).
//!
//! A page is `page.scm`'s `make-page` of the `(lily page)` module, as
//! `Page::make_page` calls it (`lily-imports.cc`), through [`PRELUDE`]'s
//! `%make-page`, and its printable height that module's
//! `calc-printable-height` (`Page::calc_printable_height`,
//! `%calc-printable-height`), which the page breaker reads; its number
//! string is `(lily)`'s `number-format`.

use alloc::format;
use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of, prob_get, prob_set};
use lily_engine_scheme::{Exception, Heap, Value};

use crate::call_lily;
use crate::grob;
use crate::item::{self, Class};
use crate::page_breaking::PageBreaking;
use crate::paper_book::{self, PaperBook};
use crate::paper_score;
use crate::stencil::Stencil;
use crate::system;

/// `%make-page paper-book page-number is-bookpart-last-page`:
/// `Page::make_page`, the `(lily page)` module's `make-page`;
/// `%page-stencil page`: `Page::page_stencil`, its `page-stencil`; and
/// `%calc-printable-height page`: `Page::calc_printable_height`, its
/// `calc-printable-height`; each looked up when it is called.
pub(crate) const PRELUDE: &str = r#"
(define (%make-page paper-book page-number last)
  ((@ (lily page) make-page) paper-book page-number last))
(define (%page-stencil page)
  ((@ (lily page) page-stencil) page))
(define (%calc-printable-height page)
  ((@ (lily page) calc-printable-height) page))
"#;

/// `Page_breaking::draw_page`: the page `page_num` holding `systems`, a
/// list of Systems and paper systems, the book part's last if `last`. Its
/// systems are spaced by the page-layout problem, as a page that is not
/// ragged, as a ragged one when the paper is `ragged-bottom` or `last`
/// and `ragged-last-bottom`, but by `last_page_force`, the force of the
/// page before, when the page is ragged as the last only
/// (`fixed_force_solution`); a page of no systems has no configuration
/// and the problem is not solved (LilyPond makes it without solving it).
/// A compressed page, or one with a negative force when the paper is
/// `ragged-bottom`, is warned of; any other's force is the next page's
/// `last_page_force`. The page is given its `lines`, each System's paper
/// system (`System::get_paper_system`, which LilyPond makes of any grob
/// it takes for a System), its `configuration`, and its `foot-stencil`
/// with the lines' footnotes added.
pub fn draw_page(
    host: &mut dyn Host,
    pb: &PageBreaking,
    systems: Value,
    page_num: i64,
    last: bool,
    last_page_force: &mut f64,
) -> Result<Value, Exception> {
    let mut drawing = PageDrawing::begin(host, pb, systems, page_num, last, *last_page_force)?;
    loop {
        if let Some(page) = drawing.on(host, pb)? {
            *last_page_force = drawing.force;
            return Ok(page);
        }
    }
}

/// [`draw_page`] under way (H3, D254): its page, its spacing problem, which
/// returns between systems and staves (B23, D261), then its lines, returning
/// between systems. What it makes is rooted until the page is drawn.
struct PageDrawing {
    depth: usize,
    page: Value,
    config: Value,
    /// The spacing problem is under way, for the page of this number.
    solving: Option<i64>,
    /// The force of the page before, then this page's unless it is compressed.
    force: f64,
    systems: Value,
    lines: Vec<Value>,
    next: usize,
    /// The paper system under way.
    system: Option<system::PaperSystem>,
    paper_systems: Vec<Value>,
}

impl PageDrawing {
    fn begin(
        host: &mut dyn Host,
        pb: &PageBreaking,
        systems: Value,
        page_num: i64,
        last: bool,
        force: f64,
    ) -> Result<PageDrawing, Exception> {
        let depth = host.heap().root_depth();
        let begun = PageDrawing::solve(host, pb, systems, page_num, last, force);
        if begun.is_err() {
            host.heap().unroot_to(depth);
        }
        let (page, solved) = begun?;
        let mut drawing = PageDrawing {
            depth,
            page,
            config: Value::NULL,
            solving: Some(page_num),
            force,
            systems,
            lines: items(host.heap(), systems),
            next: 0,
            system: None,
            paper_systems: Vec::new(),
        };
        if let Some((config, force)) = solved {
            let done = drawing.solved(host, pb, config, force);
            if done.is_err() {
                host.heap().unroot_to(depth);
            }
            done?;
        }
        Ok(drawing)
    }

    /// The page made and its spacing problem begun: `(offsets . force)`, if it
    /// is done, or `None` if it goes on.
    fn solve(
        host: &mut dyn Host,
        pb: &PageBreaking,
        systems: Value,
        page_num: i64,
        last: bool,
        last_page_force: f64,
    ) -> Result<(Value, Option<(Value, f64)>), Exception> {
        host.heap().root(systems);
        let paper = book_paper(host.heap(), pb.book);
        let number = host.heap().integer(page_num);
        let page = call_lily(host, "%make-page", &[pb.book, number, Value::bool(last)])?;
        host.heap().root(page);

        let rag = pb.ragged || (last && pb.ragged_last);
        if host.heap().car(systems).is_none() {
            return Ok((page, Some((Value::NULL, 0.0))));
        }
        let f = crate::lily_procedure(host, "%page-layout-problem")?;
        let solved = if rag && !pb.ragged {
            // The last page with the force of the page before.
            let force = host.heap().flonum(last_page_force);
            host.begin_steps(f, &[paper, page, systems, Value::TRUE, force])?
        } else {
            host.begin_steps(f, &[paper, page, systems, Value::bool(rag)])?
        };
        Ok((page, solved.map(|solved| outcome(host.heap(), solved))))
    }

    /// The spacing problem is done: the page's configuration and force.
    fn solved(
        &mut self,
        host: &mut dyn Host,
        pb: &PageBreaking,
        config: Value,
        force: f64,
    ) -> Result<(), Exception> {
        let page_num = self.solving.take().unwrap_or(0);
        host.heap().root(config);
        self.config = config;
        if (pb.ragged && force < 0.0) || force.is_infinite() {
            grob::unlocated_warning(host, &format!("page {page_num} has been compressed"))?;
        } else {
            self.force = force;
        }
        Ok(())
    }

    /// Goes on with its lines; the page, once it is drawn.
    fn on(&mut self, host: &mut dyn Host, pb: &PageBreaking) -> Result<Option<Value>, Exception> {
        let drawn = self.draw(host, pb);
        if !matches!(drawn, Ok(None)) {
            host.heap().unroot_to(self.depth);
        }
        drawn
    }

    fn draw(&mut self, host: &mut dyn Host, pb: &PageBreaking) -> Result<Option<Value>, Exception> {
        if self.solving.is_some() {
            let Some(solved) = host.go_on_steps()? else {
                return Ok(None);
            };
            let (config, force) = outcome(host.heap(), solved);
            self.solved(host, pb, config, force)?;
            if lily_engine_music::returns::due() {
                return Ok(None);
            }
        }
        while let Some(&s) = self.lines.get(self.next) {
            if self.system.is_none() && grob_of(host.heap(), s).is_some() {
                self.system = Some(system::PaperSystem::begin(host, s));
            }
            let time_up = lily_engine_music::returns::time_up;
            let paper_system = match &mut self.system {
                Some(system) => match system.on(host, &time_up)? {
                    Some(made) => made,
                    None => return Ok(None),
                },
                None => s,
            };
            self.system = None;
            self.next += 1;
            host.heap().root(paper_system);
            self.paper_systems.push(paper_system);
            if lily_engine_music::returns::due() {
                return Ok(None);
            }
        }
        let paper = book_paper(host.heap(), pb.book);
        let page = self.page;
        let heap = host.heap();
        let lines = heap.list(&self.paper_systems);
        set_prob(heap, page, "lines", lines);
        set_prob(heap, page, "configuration", self.config);

        let sym = heap.intern("foot-stencil");
        let foot = heap.foreign::<Stencil>(prob_get(heap, page, sym)).copied();
        let foot = heap.wrap(foot.unwrap_or_default());
        host.heap().root(foot);
        let footnotes = call_lily(host, "%footnotes-from-lines", &[self.systems])?;
        host.heap().root(footnotes);
        let foot = call_lily(host, "%add-footnotes-to-footer", &[footnotes, foot, paper])?;
        let heap = host.heap();
        let foot = heap.foreign::<Stencil>(foot).copied().unwrap_or_default();
        let foot = heap.wrap(foot);
        set_prob(heap, page, "foot-stencil", foot);
        Ok(Some(page))
    }
}

/// `Page_breaking::make_pages`: the pages of `systems`, a list of Systems
/// and paper systems, the first `lines_per_page[0]` on the first page and
/// so on, numbered from the paper's `first-page-number` (1 if it is no
/// integer), each drawn ([`draw_page`]) after its Systems are given their
/// `rank-on-page` and `page-number` and its lines their footnotes, which
/// are numbered on from the page before unless the top paper is
/// `reset-footnotes-on-new-page`. `'()` for no systems. The top paper's
/// `label-page-table`, `label-absolute-page-table` and
/// `label-page-string-table` get each line's `labels` with the page's
/// number, its number counted over all of the book's parts (the top
/// paper's `absolute-page-counter`, which is kept) and its string
/// (`number-format` of the paper's `page-number-type`), in the lines'
/// order. The lines a page takes are as many as `systems` has left, where
/// LilyPond's `scm_list_head` throws.
pub fn make_pages(
    host: &mut dyn Host,
    pb: &PageBreaking,
    lines_per_page: &[usize],
    systems: Value,
) -> Result<Value, Exception> {
    let mut drawing = Drawing::begin(host, pb, lines_per_page, systems)?;
    loop {
        if let Some(pages) = drawing.on(host, pb)? {
            return Ok(pages);
        }
    }
}

/// What `%page-layout-problem` gave: the configuration and the force.
fn outcome(heap: &mut Heap, solved: Value) -> (Value, f64) {
    let config = heap.car(solved).unwrap_or(Value::NULL);
    let force = heap.cdr(solved).and_then(|f| heap.to_f64(f)).unwrap_or(0.0);
    (config, force)
}

/// [`make_pages`] under way (H3, D254): [`Drawing::on`] goes on until the
/// pages are made or the step's time is spent, between systems and pages.
/// What it has made is rooted until they are.
pub struct Drawing {
    depth: usize,
    /// No systems: no pages.
    empty: bool,
    paper: Value,
    top: Value,
    label_page_table: Value,
    label_absolute_page_table: Value,
    label_page_string_table: Value,
    absolute_page_counter: i64,
    prepared: Vec<(Vec<Value>, Value, i64, bool)>,
    order: Vec<usize>,
    natural: bool,
    /// The next of `order` to draw, and the page being drawn with where
    /// what is said stood when it began.
    next: usize,
    page: Option<(PageDrawing, Option<(usize, Value)>)>,
    forces: Vec<f64>,
    pages: Vec<Value>,
    said: Vec<Option<crate::queue::Said>>,
}

impl Drawing {
    /// As far as the first page: each page's lines, numbers and footnotes,
    /// in the pages' order, and the queue's order to draw them in.
    pub fn begin(
        host: &mut dyn Host,
        pb: &PageBreaking,
        lines_per_page: &[usize],
        systems: Value,
    ) -> Result<Drawing, Exception> {
        let depth = host.heap().root_depth();
        let begun = Drawing::prepare(host, pb, lines_per_page, systems, depth);
        if begun.is_err() {
            host.heap().unroot_to(depth);
        }
        begun
    }

    fn prepare(
        host: &mut dyn Host,
        pb: &PageBreaking,
        lines_per_page: &[usize],
        systems: Value,
        depth: usize,
    ) -> Result<Drawing, Exception> {
        let top = paper_book::top_paper(host.heap(), pb.book);
        let paper = book_paper(host.heap(), pb.book);
        let mut drawing = Drawing {
            depth,
            empty: systems == Value::NULL,
            paper,
            top,
            label_page_table: Value::NULL,
            label_absolute_page_table: Value::NULL,
            label_page_string_table: Value::NULL,
            absolute_page_counter: 0,
            prepared: Vec::new(),
            order: Vec::new(),
            natural: true,
            next: 0,
            page: None,
            forces: Vec::new(),
            pages: Vec::new(),
            said: Vec::new(),
        };
        if drawing.empty {
            return Ok(drawing);
        }
        host.heap().root(systems);
        drawing.label_page_table = table(host, top, "label-page-table");
        drawing.label_absolute_page_table = table(host, top, "label-absolute-page-table");
        drawing.label_page_string_table = table(host, top, "label-page-string-table");
        for t in [
            drawing.label_page_table,
            drawing.label_absolute_page_table,
            drawing.label_page_string_table,
        ] {
            host.heap().root(t);
        }
        let counter = paper_score::c_variable(host, top, "absolute-page-counter");
        drawing.absolute_page_counter = int_or(host.heap(), counter, 0);
        let first = paper_score::c_variable(host, paper, "first-page-number");
        let first_page_number = int_or(host.heap(), first, 1);
        let reset_footnotes_on_new_page =
            paper_score::c_variable(host, top, "reset-footnotes-on-new-page") == Value::TRUE;

        // The staves are laid out and the configurations found, but nothing
        // is drawn: the pages' stencils are made once all are laid out, since
        // some grobs look at their neighbours while drawing. Each page's
        // lines, numbers and footnotes first, in the pages' order; then the
        // pages in the queue's (`queue.rs`, D251), the visible one first, and
        // what each said put back in the pages' order; then the labels.
        let mut footnote_count = 0;
        let mut rest = items(host.heap(), systems).into_iter();
        for (i, &line_count) in lines_per_page.iter().enumerate() {
            let page_num = first_page_number + i as i64;
            let bookpart_last_page = i == lines_per_page.len() - 1;
            let lines: Vec<Value> = rest.by_ref().take(line_count).collect();
            let heap = host.heap();
            let list = heap.list(&lines);
            heap.root(list);

            let mut rank_on_page = 0;
            for &line in &lines {
                if item::class(heap, line) == Some(Class::System) {
                    let rank = heap.integer(rank_on_page);
                    grob::set_property(heap, line, "rank-on-page", rank);
                    let number = heap.integer(page_num);
                    grob::set_property(heap, line, "page-number", number);
                    rank_on_page += 1;
                }
            }

            let fn_lines = call_lily(host, "%footnote-count", &[list])?;
            let fn_lines = host.heap().to_i64(fn_lines).unwrap_or(0);
            let counter = if reset_footnotes_on_new_page {
                0
            } else {
                footnote_count
            };
            let counter = host.heap().integer(counter);
            call_lily(
                host,
                "%add-footnotes-to-lines!",
                &[list, counter, paper, top],
            )?;
            footnote_count += fn_lines;
            drawing
                .prepared
                .push((lines, list, page_num, bookpart_last_page));
        }

        crate::queue::check(crate::queue::Step::LineBreaking);
        let n = drawing.prepared.len();
        drawing.order = crate::queue::order(n);
        drawing.natural = drawing.order.iter().enumerate().all(|(k, &i)| k == i);
        // Each page's force, or NaN where it was compressed and kept the
        // force of the page before: the last page's is the last of those
        // before it.
        drawing.forces = alloc::vec![f64::NAN; n];
        drawing.pages = alloc::vec![Value::NULL; n];
        drawing.said = (0..n).map(|_| None).collect();
        Ok(drawing)
    }

    /// Goes on; the pages, as a list, once they are made.
    pub fn on(
        &mut self,
        host: &mut dyn Host,
        pb: &PageBreaking,
    ) -> Result<Option<Value>, Exception> {
        let drawn = self.draw(host, pb);
        if !matches!(drawn, Ok(None)) {
            host.heap().unroot_to(self.depth);
        }
        drawn
    }

    fn draw(&mut self, host: &mut dyn Host, pb: &PageBreaking) -> Result<Option<Value>, Exception> {
        if self.empty {
            return Ok(Some(Value::NULL));
        }
        while let Some(&i) = self.order.get(self.next) {
            let (_, list, page_num, last) = self.prepared[i];
            if self.page.is_none() {
                // Only the last page reads the force before it (`draw_page`),
                // and [`crate::queue::order`] draws it last.
                let force = match last {
                    true => self.forces[..i]
                        .iter()
                        .rev()
                        .find(|f| !f.is_nan())
                        .copied()
                        .unwrap_or(0.0),
                    false => f64::NAN,
                };
                let mark = (!self.natural).then(|| crate::queue::mark(host));
                let drawing = PageDrawing::begin(host, pb, list, page_num, last, force)?;
                self.page = Some((drawing, mark));
                crate::queue::check(crate::queue::Step::Page);
                if lily_engine_music::returns::due() {
                    return Ok(None);
                }
            }
            let Some((drawing, mark)) = &mut self.page else {
                continue;
            };
            let Some(page) = drawing.on(host, pb)? else {
                return Ok(None);
            };
            if let Some(mark) = *mark {
                self.said[i] = Some(crate::queue::take_since(host, mark));
            }
            if !last {
                self.forces[i] = drawing.force;
            }
            self.page = None;
            host.heap().root(page);
            self.pages[i] = page;
            crate::queue::drawn(i, &self.order);
            self.next += 1;
            if lily_engine_music::returns::due() {
                return Ok(None);
            }
        }
        if !self.natural {
            let said = core::mem::take(&mut self.said);
            crate::queue::say_again(host, said.into_iter().flatten().collect())?;
        }

        let paper = self.paper;
        let top = self.top;
        let mut label_page_table = self.label_page_table;
        let mut label_absolute_page_table = self.label_absolute_page_table;
        let mut label_page_string_table = self.label_page_string_table;
        let mut absolute_page_counter = self.absolute_page_counter;
        let mut ret = Vec::new();
        for (i, (lines, _, page_num, _)) in
            core::mem::take(&mut self.prepared).into_iter().enumerate()
        {
            let page = self.pages[i];
            // The labels.
            absolute_page_counter += 1;
            let page_num_scm = host.heap().integer(page_num);
            let absolute_page_num = host.heap().integer(absolute_page_counter);
            let page_number_type = paper_score::c_variable(host, paper, "page-number-type");
            let page_string = call_lily(host, "number-format", &[page_number_type, page_num_scm])?;
            host.heap().root(page_string);
            // The lines backwards, so that the labels come out in their order.
            for &line in lines.iter().rev() {
                let labels = if item::class(host.heap(), line) == Some(Class::System) {
                    grob::property(host, line, "labels")?
                } else {
                    let heap = host.heap();
                    let sym = heap.intern("labels");
                    prob_get(heap, line, sym)
                };
                let heap = host.heap();
                // The labels are in reverse order: consed one by one, they
                // come out in forward order.
                for label in items(heap, labels) {
                    label_page_table = acons(heap, label, page_num_scm, label_page_table);
                    label_absolute_page_table =
                        acons(heap, label, absolute_page_num, label_absolute_page_table);
                    label_page_string_table =
                        acons(heap, label, page_string, label_page_string_table);
                }
                heap.root(label_page_table);
                heap.root(label_absolute_page_table);
                heap.root(label_page_string_table);
            }

            ret.push(page);
        }

        let counter = host.heap().integer(absolute_page_counter);
        for (name, v) in [
            ("label-page-table", label_page_table),
            ("label-absolute-page-table", label_absolute_page_table),
            ("label-page-string-table", label_page_string_table),
            ("absolute-page-counter", counter),
        ] {
            let sym = host.heap().intern(name);
            call_lily(host, "ly:output-def-set-variable!", &[top, sym, v])?;
        }
        Ok(Some(host.heap().list(&ret)))
    }
}

/// The paper of the `Paper_book` `book`, `#f` for what is none.
pub(crate) fn book_paper(heap: &Heap, book: Value) -> Value {
    heap.foreign::<PaperBook>(book)
        .map_or(Value::FALSE, |b| b.paper)
}

/// The top paper's table `name`, `'()` if it has none.
fn table(host: &mut dyn Host, top: Value, name: &str) -> Value {
    match paper_score::c_variable(host, top, name) {
        Value::UNDEFINED => Value::NULL,
        v => v,
    }
}

/// `from_scm (v, fallback)` of an `int`: `v` if it is an exact integer an
/// `int` holds.
pub(crate) fn int_or(heap: &Heap, v: Value, fallback: i64) -> i64 {
    heap.to_i64(v)
        .filter(|n| i32::try_from(*n).is_ok())
        .unwrap_or(fallback)
}

/// `scm_acons`.
fn acons(heap: &mut Heap, key: Value, value: Value, alist: Value) -> Value {
    let entry = heap.cons(key, value);
    heap.cons(entry, alist)
}

/// `set_property` of the prob `prob`'s property `name`.
fn set_prob(heap: &mut Heap, prob: Value, name: &str, v: Value) {
    let sym = heap.intern(name);
    prob_set(heap, prob, sym, v);
}

/// The items of the list `list`.
fn items(heap: &Heap, mut list: Value) -> Vec<Value> {
    let mut out = Vec::new();
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        out.push(item);
        list = rest;
    }
    out
}
