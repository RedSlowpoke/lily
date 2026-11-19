//! `Paper_book` (`paper-book.cc`, part 5's) and `Book::process`
//! (`book.cc:197–330`), which makes one of a book (MS3 step 35): the
//! book's paper scaled, its scores' outputs and its parts' `Paper_book`s in
//! the order they are printed. So far what a compile fills and reads: the
//! constructor, `top_paper`, `add_score` and `add_bookpart`, the systems
//! it prints ([`get_system_specs`], with the page markers' permissions and
//! labels since MS3 step 64, and [`systems`]), its pages ([`pages`])
//! and their stencils for the writer ([`output`]), and `ly:paper-book-paper` and `ly:paper-book-scopes`
//! (`paper-book-scheme.cc`), which `page.scm` and `titling.scm` call;
//! since MS3 step 62 the rest of that file's face but the performances
//! (MS5's): `ly:paper-book-pages`, `ly:paper-book-systems` and
//! `ly:paper-book-header`. Since MS5 step 1 its performances, which
//! [`add_performance`] adds and [`output_midis`] writes.
//!
//! [`process`] makes the tree before any score is translated, and lists
//! each item of each book or part with the `Paper_book` it goes to, in
//! LilyPond's order of `process_score`; the caller translates a score
//! there and adds its outputs ([`add_score`]), as `process_score` does,
//! and adds an item that is no score as it is. Only the scores' outputs
//! are added after the tree is made, each to its own `Paper_book`, so the
//! print elements are those of LilyPond's, which processes each score as
//! it makes the tree: a part's `Paper_book` is in its book's print
//! elements before its scores' outputs are in its own, and nothing reads
//! them before the pages.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{
    Book, Host, OutputDef, Prob, add_scores_to_bookpart, normalize, paper_book, prob_get, prob_set,
};
use lily_engine_scheme::{
    Cx, Exception, Foreign, Heap, HostHandler, Tag, Thrown, Tracer, Value, Vm,
};

use crate::geometry::{Direction, Y};
use crate::grob;
use crate::item;
use crate::page_marker::PageMarker;
use crate::paper_score::{self, PaperScore};
use crate::stencil::Stencil;
use crate::system::{make_paper_system, paper_system_set_stencil};
use crate::{R, call_lily, lily_procedure};

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const UP: Direction = 1;

/// A `Paper_book`: its paper (`paper_`, its book's scaled), the part's
/// book's `Paper_book` (`parent_`, `#f` for a book), the book's header,
/// the first score's header found (`header_0_`), its pages and systems
/// (`#f` until made), what it prints (`print_elements_`: headers, Paper
/// scores, texts, or the `Paper_book`s of its parts) and whether those are
/// its parts' (`print_bookparts_`).
pub struct PaperBook {
    pub paper: Value,
    pub parent: Value,
    pub header: Value,
    pub header_0: Value,
    pub pages: Value,
    pub systems: Value,
    pub print_elements: Vec<Value>,
    pub print_bookparts: bool,
    /// `performances_`: its scores' `Performance`s, in order (MS5 step 1).
    pub performances: Vec<Value>,
}

impl Foreign for PaperBook {
    fn type_name(&self) -> &'static str {
        "Paper_book"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        for v in [
            self.paper,
            self.parent,
            self.header,
            self.header_0,
            self.pages,
            self.systems,
        ] {
            tracer.trace(v);
        }
        for &v in self.print_elements.iter().chain(&self.performances) {
            tracer.trace(v);
        }
    }
}

/// `Paper_book::Paper_book` (`paper-book.cc:45–69`): a `Paper_book` whose
/// paper is `paper` scaled by its own `output-scale`, parented to the
/// paper of `parent_part`'s, a part's book's `Paper_book`.
pub fn make(vm: &mut Vm, paper: Value, parent_part: Option<Value>) -> Result<Value, Exception> {
    let above = parent_part.and_then(|p| vm.heap().foreign::<PaperBook>(p).map(|b| b.paper));
    let paper = paper_book(vm, paper, above)?;
    Ok(vm.heap_mut().wrap(PaperBook {
        paper,
        parent: parent_part.unwrap_or(Value::FALSE),
        header: Value::NULL,
        header_0: Value::NULL,
        pages: Value::FALSE,
        systems: Value::FALSE,
        print_elements: Vec::new(),
        print_bookparts: false,
        performances: Vec::new(),
    }))
}

/// `Paper_book::top_paper` (`paper-book.cc:92–99`): the root of the
/// parents of `book`'s paper.
pub fn top_paper(heap: &Heap, book: Value) -> Value {
    let Some(mut paper) = heap.foreign::<PaperBook>(book).map(|b| b.paper) else {
        return Value::FALSE;
    };
    while let Some(parent) = heap.foreign::<OutputDef>(paper).and_then(|d| d.parent) {
        paper = parent;
    }
    paper
}

/// `Paper_book::add_score` (`paper-book.cc:118–122`): `s`, a header, a
/// Paper_score or a text, printed after what `book` prints so far.
pub fn add_score(heap: &mut Heap, book: Value, s: Value) {
    if let Some(b) = heap.foreign_mut::<PaperBook>(book) {
        b.print_elements.push(s);
    }
}

/// `Book::process_score`'s performance (`book.cc:240–257`):
/// `performance`, a `\midi` translation's, added to `book`
/// (`Paper_book::add_performance`), with the headers its name is found in
/// pushed onto it: the book's first score's, the book's and the score's
/// `header`, each that is a module.
pub fn add_performance(heap: &mut Heap, book: Value, header: Value, performance: Value) {
    let Some(b) = heap.foreign_mut::<PaperBook>(book) else {
        return;
    };
    b.performances.push(performance);
    let (header_0, book_header) = (b.header_0, b.header);
    for h in [header_0, book_header, header] {
        if is_module(heap, h) {
            lily_engine_music::push_header(heap, performance, h);
        }
    }
}

/// `Paper_book::output_aux`'s MIDI (`paper-book.cc:137–160`): `book`'s
/// performances written by `write-performances-midis` after `basename`,
/// numbered from `count` (`<basename>.midi`, then `<basename>-1.midi` and
/// on), then those of its parts in the order it prints them; the files
/// they wrote, in order.
pub fn output_midis(
    vm: &mut Vm,
    book: Value,
    basename: &str,
    count: &mut i64,
) -> Result<Vec<(String, Vec<u8>)>, Exception> {
    let Some((performances, parts)) = vm.heap().foreign::<PaperBook>(book).map(|b| {
        (
            b.performances.clone(),
            b.print_bookparts.then(|| b.print_elements.clone()),
        )
    }) else {
        return Ok(Vec::new());
    };
    let mut written = Vec::new();
    if !performances.is_empty() {
        let heap = vm.heap_mut();
        let list = heap.list(&performances);
        let name = heap.string(basename);
        let n = heap.integer(*count);
        call_lily(vm, "write-performances-midis", &[list, name, n])?;
        for &p in &performances {
            written.extend(lily_engine_music::take_written(vm.heap_mut(), p));
        }
        *count += performances.len() as i64;
    }
    for part in parts.unwrap_or_default() {
        if vm.heap().foreign::<PaperBook>(part).is_some() {
            written.extend(output_midis(vm, part, basename, count)?);
        }
    }
    Ok(written)
}

/// `Paper_book::add_bookpart` (`paper-book.cc:124–129`): the `Paper_book`
/// of a part, printed after what `book` prints so far; `book` prints its
/// parts.
pub fn add_bookpart(heap: &mut Heap, book: Value, part: Value) {
    if let Some(b) = heap.foreign_mut::<PaperBook>(book) {
        b.print_bookparts = true;
        b.print_elements.push(part);
    }
}

/// `Paper_book::book_title` (`paper-book.cc:438–461`): what the paper's
/// `book-title` procedure draws of the book's header, if that is a module
/// (titling.scm's `marked-up-title`), aligned on its top; empty if the
/// paper has no such procedure or it gives no stencil.
fn book_title(host: &mut dyn Host, paper: Value, header: Value) -> Result<Stencil, Exception> {
    let scopes: &[Value] = if is_module(host.heap(), header) {
        &[header]
    } else {
        &[]
    };
    title(host, paper, "book-title", scopes)
}

/// `Paper_book::score_title` (`paper-book.cc:463–488`): what the paper's
/// `score-title` procedure draws of the score's header `header` and the
/// book's `book_header`, each if it is a module, the score's first.
fn score_title(
    host: &mut dyn Host,
    paper: Value,
    book_header: Value,
    header: Value,
) -> Result<Stencil, Exception> {
    let mut scopes = Vec::new();
    for h in [header, book_header] {
        if is_module(host.heap(), h) {
            scopes.push(h);
        }
    }
    title(host, paper, "score-title", &scopes)
}

/// The title `book_title` and `score_title` draw: the procedure `name` of
/// `paper` called on `paper` and the list `scopes`, its stencil aligned on
/// its top if it is not empty.
fn title(
    host: &mut dyn Host,
    paper: Value,
    name: &str,
    scopes: &[Value],
) -> Result<Stencil, Exception> {
    let title_func = paper_score::c_variable(host, paper, name);
    let mut title = Stencil::default();
    if host.heap().is_procedure(title_func) {
        let scopes = host.heap().list(scopes);
        let tit = host.call(title_func, &[paper, scopes])?;
        if let Some(st) = host.heap().foreign::<Stencil>(tit) {
            title = *st;
        }
    }
    if !title.is_empty() {
        title.align_to(host.heap(), Y, f64::from(UP));
    }
    Ok(title)
}

/// `set_page_permission` (`paper-book.cc:490–505`): `name` set to
/// `permission` on the last column of a Paper_score `sys` and on that
/// column's piece on the left, or on a paper system `sys`.
fn set_page_permission(
    host: &mut dyn Host,
    sys: Value,
    name: &str,
    permission: Value,
) -> Result<(), Exception> {
    if host.heap().foreign::<PaperScore>(sys).is_some() {
        if let Some(&col) = paper_score::get_columns(host, sys)?.last() {
            let heap = host.heap();
            grob::set_property(heap, col, name, permission);
            if let Some(piece) = item::find_prebroken_piece(heap, col, LEFT) {
                grob::set_property(heap, piece, name, permission);
            }
        }
    } else if host.heap().foreign::<Prob>(sys).is_some() {
        let heap = host.heap();
        let sym = heap.intern(name);
        prob_set(heap, sys, sym, permission);
    }
    Ok(())
}

/// `set_labels` (`paper-book.cc:520–540`): `labels` appended to the
/// `labels` of the first column of a Paper_score `sys` and of that
/// column's piece on the right, or of a paper system `sys`.
fn set_labels(host: &mut dyn Host, sys: Value, labels: Value) -> Result<(), Exception> {
    if host.heap().foreign::<PaperScore>(sys).is_some() {
        if let Some(&col) = paper_score::get_columns(host, sys)?.first() {
            let right = item::find_prebroken_piece(host.heap(), col, RIGHT);
            for col in core::iter::once(col).chain(right) {
                let own = grob::property(host, col, "labels")?;
                let all = append(host.heap(), own, labels);
                grob::set_property(host.heap(), col, "labels", all);
            }
        }
    } else if host.heap().foreign::<Prob>(sys).is_some() {
        let heap = host.heap();
        let sym = heap.intern("labels");
        let own = prob_get(heap, sys, sym);
        let all = append(heap, own, labels);
        prob_set(heap, sys, sym, all);
    }
    Ok(())
}

/// `ly_append` of two lists: a copy of `a` ending in `b`.
fn append(heap: &mut Heap, a: Value, b: Value) -> Value {
    let mut items = Vec::new();
    push_items(heap, a, &mut items);
    items
        .into_iter()
        .rev()
        .fold(b, |tail, x| heap.cons(x, tail))
}

/// `set_system_penalty` (`paper-book.cc:507–533`): a score's header's
/// `breakbefore`, if it is a boolean, set on `sys`, the system spec before
/// the score: `#t` forces a page and a line break there, `#f` takes away
/// the page break's permission.
fn set_system_penalty(host: &mut dyn Host, sys: Value, header: Value) -> Result<(), Exception> {
    if !is_module(host.heap(), header) {
        return Ok(());
    }
    let sym = host.heap().intern("breakbefore");
    match host.module_ref(header, sym) {
        Some(Value::TRUE) => {
            let force = host.heap().intern("force");
            set_page_permission(host, sys, "page-break-permission", force)?;
            set_page_permission(host, sys, "line-break-permission", force)
        }
        Some(Value::FALSE) => set_page_permission(host, sys, "page-break-permission", Value::NULL),
        _ => Ok(()),
    }
}

/// `Paper_book::get_score_title` (`paper-book.cc:557–576`): a paper system
/// of the score title of `header`, with the paper's
/// `score-title-properties`, or `#f` if the title is empty.
fn get_score_title(
    host: &mut dyn Host,
    paper: Value,
    book_header: Value,
    header: Value,
) -> Result<Value, Exception> {
    let title = score_title(host, paper, book_header, header)?;
    if title.is_empty() {
        return Ok(Value::FALSE);
    }
    let props = paper_score::c_variable(host, paper, "score-title-properties");
    let ps = make_paper_system(host.heap(), props);
    paper_system_set_stencil(host.heap(), ps, title);
    Ok(ps)
}

/// `get_footnotes` (`paper-system.cc:42–77`), borrowed from part 5: the
/// footnotes in the stencil expression `expr`, through its combined and
/// translated stencils, each `footnote`'s arguments a list.
fn get_footnotes(heap: &mut Heap, expr: Value) -> Value {
    let (Some(head), Some(rest)) = (heap.car(expr), heap.cdr(expr)) else {
        return Value::NULL;
    };
    if head == heap.intern("combine-stencil") {
        let mut out = Vec::new();
        let mut x = rest;
        while let (Some(item), Some(next)) = (heap.car(x), heap.cdr(x)) {
            let mut footnotes = get_footnotes(heap, item);
            while let (Some(f), Some(more)) = (heap.car(footnotes), heap.cdr(footnotes)) {
                out.push(f);
                footnotes = more;
            }
            x = next;
        }
        heap.list(&out)
    } else if head == heap.intern("translate-stencil") {
        match heap.cdr(rest).and_then(|r| heap.car(r)) {
            Some(inner) => get_footnotes(heap, inner),
            None => Value::NULL,
        }
    } else if head == heap.intern("footnote") {
        heap.list(&[rest])
    } else {
        // `delay-stencil-evaluation`'s too.
        Value::NULL
    }
}

/// `Paper_book::get_system_specs` (`paper-book.cc:578–710`): what `book`
/// prints, as a list of system specs: its book title's paper system if it
/// is not empty, then for each Paper_score its score title's paper system,
/// if any, and the Paper_score itself, and for each markup list a paper
/// system of each of its lines. A score's header (the last module before
/// it) gives its title and its `breakbefore` to the spec before it; the
/// first header is `header_0`. A page marker (`page_marker.rs`, MS3 step
/// 64) sets its permission on the spec before it and gives its label to
/// the next Paper_score or markup line ([`set_labels`]). A performance,
/// which LilyPond ignores, is never added to a `Paper_book` in the engine.
pub fn get_system_specs(host: &mut dyn Host, book: Value) -> Result<Value, Exception> {
    let Some((paper, header, elements)) = host
        .heap()
        .foreign::<PaperBook>(book)
        .map(|b| (b.paper, b.header, b.print_elements.clone()))
    else {
        return Ok(Value::NULL);
    };
    let depth = host.heap().root_depth();
    let specs =
        system_specs(host, book, paper, header, &elements).map(|specs| host.heap().list(&specs));
    host.heap().unroot_to(depth);
    specs
}

/// The specs of [`get_system_specs`] of `book`, its paper `paper` and its
/// header `book_header`, each rooted as it is made.
fn system_specs(
    host: &mut dyn Host,
    book: Value,
    paper: Value,
    book_header: Value,
    elements: &[Value],
) -> Result<Vec<Value>, Exception> {
    let mut specs = Vec::new();
    let mut last_system_spec = Value::FALSE;

    let title = book_title(host, paper, book_header)?;
    if !title.is_empty() {
        let props = paper_score::c_variable(host, paper, "book-title-properties");
        let ps = make_paper_system(host.heap(), props);
        paper_system_set_stencil(host.heap(), ps, title);
        host.heap().root(ps);
        specs.push(ps);
        last_system_spec = ps;
    }

    let page_properties = call_lily(host, "layout-extract-page-properties", &[paper])?;
    host.heap().root(page_properties);

    let mut header = Value::NULL;
    // The labels of the markers since the last spec, the last first.
    let mut labels = Value::NULL;
    for &elem in elements {
        if is_module(host.heap(), elem) {
            header = elem;
            if let Some(b) = host.heap().foreign_mut::<PaperBook>(book)
                && b.header_0 == Value::NULL
            {
                b.header_0 = header;
            }
        } else if let Some(m) = host.heap().foreign::<PageMarker>(elem) {
            let (symbol, permission, label) = (m.symbol, m.permission, m.label);
            if symbol.tag() == Tag::Symbol && last_system_spec != Value::FALSE {
                let name = host.heap().name(symbol).map(String::from);
                set_page_permission(
                    host,
                    last_system_spec,
                    &name.unwrap_or_default(),
                    permission,
                )?;
            }
            if label.tag() == Tag::Symbol {
                labels = host.heap().cons(label, labels);
                host.heap().root(labels);
            }
        } else if host.heap().foreign::<PaperScore>(elem).is_some() {
            let title = get_score_title(host, paper, book_header, header)?;
            if last_system_spec != Value::FALSE {
                // The spec before the score.
                set_system_penalty(host, last_system_spec, header)?;
            }
            if title != Value::FALSE {
                host.heap().root(title);
                specs.push(title);
            }
            header = Value::NULL;
            specs.push(elem);
            last_system_spec = elem;
            if labels != Value::NULL {
                set_labels(host, elem, labels)?;
                labels = Value::NULL;
            }
        } else if is_markup_list(host, elem)? {
            let texts = call_lily(
                host,
                "interpret-markup-list",
                &[paper, page_properties, elem],
            )?;
            host.heap().root(texts);
            let (mut first, mut last) = (None, None);
            let mut list = texts;
            while let (Some(t), Some(rest)) = (host.heap().car(list), host.heap().cdr(list)) {
                let heap = host.heap();
                let t = heap.foreign::<Stencil>(t).copied().unwrap_or_default();
                let ps = make_paper_system(heap, Value::NULL);
                heap.root(ps);
                let allow = heap.intern("allow");
                set_prob(heap, ps, "page-break-permission", allow);
                set_prob(heap, ps, "page-turn-permission", allow);
                set_prob(heap, ps, "last-markup-line", Value::FALSE);
                set_prob(heap, ps, "first-markup-line", Value::FALSE);
                paper_system_set_stencil(heap, ps, t);
                let footnotes = get_footnotes(heap, t.expr);
                set_prob(heap, ps, "footnotes", footnotes);
                set_prob(heap, ps, "is-title", Value::TRUE);
                if list == texts {
                    first = Some(ps);
                } else {
                    // A later line of a paragraph, close to the one before.
                    last = Some(ps);
                    set_prob(heap, ps, "tight-spacing", Value::TRUE);
                }
                specs.push(ps);
                last_system_spec = ps;
                if labels != Value::NULL {
                    set_labels(host, ps, labels)?;
                    labels = Value::NULL;
                }
                list = rest;
            }
            // The first and last lines of a paragraph of several, kept
            // from being widowed or orphaned.
            if let (Some(first), Some(last)) = (first, last) {
                let heap = host.heap();
                set_prob(heap, last, "last-markup-line", Value::TRUE);
                set_prob(heap, first, "first-markup-line", Value::TRUE);
            }
        }
    }
    Ok(specs)
}

/// `Paper_book::systems` (`paper-book.cc:712–749`): the systems `book`
/// prints, as a list, made the first time: its parts' in order if it
/// prints its parts, else its system specs with each Paper_score's paper
/// systems in its place ([`paper_score::get_paper_systems`]). Kept only
/// once they are all made, where LilyPond keeps what it has made when an
/// error ends it; `'()` for what is no `Paper_book`.
pub fn systems(host: &mut dyn Host, book: Value) -> Result<Value, Exception> {
    let Some((made, bookparts, elements)) = host
        .heap()
        .foreign::<PaperBook>(book)
        .map(|b| (b.systems, b.print_bookparts, b.print_elements.clone()))
    else {
        return Ok(Value::NULL);
    };
    if made != Value::FALSE {
        return Ok(made);
    }
    let depth = host.heap().root_depth();
    let made = book_systems(host, book, bookparts, &elements).map(|s| host.heap().list(&s));
    host.heap().unroot_to(depth);
    let made = made?;
    if let Some(b) = host.heap().foreign_mut::<PaperBook>(book) {
        b.systems = made;
    }
    Ok(made)
}

/// The systems of [`systems`], each held by a part, a Paper_score or the
/// specs, which are rooted.
fn book_systems(
    host: &mut dyn Host,
    book: Value,
    bookparts: bool,
    elements: &[Value],
) -> Result<Vec<Value>, Exception> {
    let mut out = Vec::new();
    if bookparts {
        for &p in elements {
            if host.heap().foreign::<PaperBook>(p).is_some() {
                let sys = systems(host, p)?;
                push_items(host.heap(), sys, &mut out);
            }
        }
    } else {
        let specs = get_system_specs(host, book)?;
        host.heap().root(specs);
        let mut s = specs;
        while let (Some(spec), Some(rest)) = (host.heap().car(s), host.heap().cdr(s)) {
            if host.heap().foreign::<PaperScore>(spec).is_some() {
                let systems = paper_score::get_paper_systems(host, spec)?;
                out.extend_from_slice(host.heap().vector_items(systems).unwrap_or_default());
            } else {
                out.push(spec);
            }
            s = rest;
        }
    }
    Ok(out)
}

/// `Paper_book::pages` (`paper-book.cc:751–806`): the pages `book`
/// prints, as a list, made the first time: its parts' in order if it
/// prints its parts, else, if it prints anything, its pages, each given
/// its stencil (`(lily page)`'s `page-stencil`, `Page::page_stencil`), then
/// the paper's `page-post-process` called on the paper and the pages if it
/// is a procedure, and the book's systems, if they are not made yet, set
/// to the pages' lines in order. The pages are what the paper's
/// `page-breaking` procedure gives of `book`, `ly:optimal-breaking` by
/// default (`optimal_page_breaking.rs`, MS3 step 62), which breaks the
/// book's scores into their lines. As LilyPond's, the pages are `'()`
/// while they are made and kept from when the breaking gives them, if a
/// page's stencil or the post-processing fails after. `'()` for what is
/// no `Paper_book`.
pub fn pages(host: &mut dyn Host, book: Value) -> Result<Value, Exception> {
    let Some((made, paper, bookparts, elements)) =
        host.heap().foreign::<PaperBook>(book).map(|b| {
            (
                b.pages,
                b.paper,
                b.print_bookparts,
                b.print_elements.clone(),
            )
        })
    else {
        return Ok(Value::NULL);
    };
    if made != Value::FALSE {
        return Ok(made);
    }
    set_pages(host.heap(), book, Value::NULL);
    let depth = host.heap().root_depth();
    let made = book_pages(host, book, paper, bookparts, &elements);
    host.heap().unroot_to(depth);
    made
}

/// The work of [`pages`]: what it makes is rooted until the caller
/// unroots it, or held by `book`.
fn book_pages(
    host: &mut dyn Host,
    book: Value,
    paper: Value,
    bookparts: bool,
    elements: &[Value],
) -> Result<Value, Exception> {
    if bookparts {
        let mut out = Vec::new();
        for &p in elements {
            if host.heap().foreign::<PaperBook>(p).is_some() {
                let pages = pages(host, p)?;
                push_items(host.heap(), pages, &mut out);
            }
        }
        let pages = host.heap().list(&out);
        set_pages(host.heap(), book, pages);
        return Ok(pages);
    }
    if elements.is_empty() {
        return Ok(Value::NULL);
    }
    let page_breaking = paper_score::c_variable(host, paper, "page-breaking");
    let pages = host.call(page_breaking, &[book])?;
    set_pages(host.heap(), book, pages);

    // The pages' stencils.
    let mut list = pages;
    while let (Some(p), Some(rest)) = (host.heap().car(list), host.heap().cdr(list)) {
        call_lily(host, "%page-stencil", &[p])?;
        crate::queue::check(crate::queue::Step::Pages);
        list = rest;
    }
    after_stencils(host, book, paper, pages)?;
    Ok(pages)
}

/// What [`book_pages`] does after the pages' stencils.
fn after_stencils(
    host: &mut dyn Host,
    book: Value,
    paper: Value,
    pages: Value,
) -> Result<(), Exception> {
    // The user's post-processing.
    let post_process = paper_score::c_variable(host, paper, "page-post-process");
    if host.heap().is_procedure(post_process) {
        host.call(post_process, &[paper, pages])?;
    }

    // The systems, from the pages.
    if host.heap().foreign::<PaperBook>(book).map(|b| b.systems) == Some(Value::FALSE) {
        let mut systems = Vec::new();
        let lines = host.heap().intern("lines");
        let mut list = pages;
        while let (Some(p), Some(rest)) = (host.heap().car(list), host.heap().cdr(list)) {
            let sys = prob_get(host.heap(), p, lines);
            push_items(host.heap(), sys, &mut systems);
            list = rest;
        }
        let systems = host.heap().list(&systems);
        if let Some(b) = host.heap().foreign_mut::<PaperBook>(book) {
            b.systems = systems;
        }
    }
    Ok(())
}

/// `book`'s pages set to `pages`.
fn set_pages(heap: &mut Heap, book: Value, pages: Value) {
    if let Some(b) = heap.foreign_mut::<PaperBook>(book) {
        b.pages = pages;
    }
}

/// `Paper_book::output` (`paper-book.cc:226–264`) as far as the pages it
/// gives the backend with `print-pages`: the paper's `line-width` set to
/// its `paper-width` if it has none, the pages made ([`output_aux`]), then
/// each page's stencil in order, with the number of the first, the
/// paper's `first-page-number` (1 if it is no integer), which
/// `framework-svg.scm`'s `output-stencils` numbers their files from. No
/// pages if the book prints nothing. A performance is not written here:
/// the core writes the playback map of its own.
pub fn output(vm: &mut Vm, book: Value) -> Result<(i64, Vec<Stencil>), Exception> {
    let Some(paper) = vm.heap().foreign::<PaperBook>(book).map(|b| b.paper) else {
        return Ok((1, Vec::new()));
    };
    let first = paper_score::c_variable(vm, paper, "first-page-number");
    let first_page_number = vm.heap().to_i64(first).unwrap_or(1);
    if paper_score::c_variable(vm, paper, "line-width") == Value::UNDEFINED {
        let width = paper_score::c_variable(vm, paper, "paper-width");
        set_variable(vm, paper, "line-width", width);
    }
    let mut next = first_page_number;
    if output_aux(vm, book, true, &mut next)? == 0 {
        return Ok((first_page_number, Vec::new()));
    }
    let pages = pages(vm, book)?;
    let stencil = vm.heap().intern("stencil");
    let mut list = Vec::new();
    push_items(vm.heap(), pages, &mut list);
    let mut stencils = Vec::new();
    for page in list {
        let s = prob_get(vm.heap(), page, stencil);
        if let Some(&s) = vm.heap().foreign::<Stencil>(s) {
            stencils.push(s);
        }
    }
    Ok((first_page_number, stencils))
}

/// `Paper_book::output_aux` (`paper-book.cc:136–180`) but its
/// performances: the pages of `book`, made by [`pages`], the last of the
/// output if `is_last`, numbered from `first_page_number`, which is
/// counted on past them; for a book of parts each part's in turn, the
/// last element printed the last part. Before its pages a book that is no
/// book of parts has the number set as its paper's `first-page-number`,
/// unless the paper numbers each part's pages on their own
/// (`bookpart-level-page-numbering`), and `is-last-bookpart` set, which
/// `page.scm`'s `make-page` reads. How many pages there are.
fn output_aux(
    vm: &mut Vm,
    book: Value,
    is_last: bool,
    first_page_number: &mut i64,
) -> Result<i64, Exception> {
    let Some((paper, bookparts, elements)) = vm
        .heap()
        .foreign::<PaperBook>(book)
        .map(|b| (b.paper, b.print_bookparts, b.print_elements.clone()))
    else {
        return Ok(0);
    };
    let mut page_number = 0;
    if bookparts {
        for (i, &p) in elements.iter().enumerate() {
            if vm.heap().foreign::<PaperBook>(p).is_some() {
                let is_last_part = is_last && i + 1 == elements.len();
                page_number += output_aux(vm, p, is_last_part, first_page_number)?;
            }
        }
    } else {
        if elements.is_empty() {
            return Ok(0);
        }
        let per_part = paper_score::c_variable(vm, paper, "bookpart-level-page-numbering");
        let pgnums_per_bookpart = per_part == Value::TRUE;
        if !pgnums_per_bookpart {
            let n = vm.heap_mut().integer(*first_page_number);
            set_variable(vm, paper, "first-page-number", n);
        }
        set_variable(vm, paper, "is-last-bookpart", Value::bool(is_last));
        let pages = pages(vm, book)?;
        let mut list = Vec::new();
        push_items(vm.heap(), pages, &mut list);
        page_number = list.len() as i64;
        if !pgnums_per_bookpart {
            *first_page_number += page_number;
        }
    }
    Ok(page_number)
}

/// Where an [`Output`] is in the book it is at.
enum Leaf {
    Starting,
    /// The default page breaker running, what it rooted above `depth`.
    Solving(Box<crate::optimal_page_breaking::Solving>, usize),
    /// The stencils of the pages left, of `pages`.
    Stencils {
        pages: Value,
        left: Value,
    },
}

/// [`output`] under way (H3, D254): each book that holds pages, in
/// [`output_aux`]'s order, made as [`pages`] makes it, returning between
/// lines, systems and page stencils when the step's time is spent. Then
/// [`output`] itself, which finds them made. A paper whose `page-breaking`
/// is not the default breaker may be Scheme of the user's, which no return
/// can leave: its pages are one step.
pub struct Output {
    book: Value,
    depth: usize,
    leaves: Vec<(Value, bool)>,
    next: usize,
    /// The number the next book's pages start at.
    first_page_number: i64,
    leaf: Leaf,
}

/// The books of `book` that hold pages, each with whether it is the last of
/// the output, in [`output_aux`]'s order.
fn leaves(heap: &Heap, book: Value, is_last: bool, out: &mut Vec<(Value, bool)>) {
    let Some(b) = heap.foreign::<PaperBook>(book) else {
        return;
    };
    if b.print_bookparts {
        let parts = b.print_elements.clone();
        for (i, &p) in parts.iter().enumerate() {
            leaves(heap, p, is_last && i + 1 == parts.len(), out);
        }
    } else {
        out.push((book, is_last));
    }
}

impl Output {
    /// [`output`] as far as the first book's pages.
    pub fn begin(vm: &mut Vm, book: Value) -> Output {
        let mut out = Vec::new();
        leaves(vm.heap(), book, true, &mut out);
        let mut first_page_number = 1;
        if let Some(paper) = vm.heap().foreign::<PaperBook>(book).map(|b| b.paper) {
            let first = paper_score::c_variable(vm, paper, "first-page-number");
            first_page_number = vm.heap().to_i64(first).unwrap_or(1);
            if paper_score::c_variable(vm, paper, "line-width") == Value::UNDEFINED {
                let width = paper_score::c_variable(vm, paper, "paper-width");
                set_variable(vm, paper, "line-width", width);
            }
        }
        Output {
            book,
            depth: vm.heap().root_depth(),
            leaves: out,
            next: 0,
            first_page_number,
            leaf: Leaf::Starting,
        }
    }

    /// Its book.
    pub fn book(&self) -> Value {
        self.book
    }

    /// What it goes on with, for the steps' timings.
    pub fn stage(&self) -> &'static str {
        match &self.leaf {
            Leaf::Starting => "book",
            Leaf::Solving(solving, _) => solving.stage(),
            Leaf::Stencils { .. } => "page stencils",
        }
    }

    /// Goes on; what [`output`] gives, once the pages are made.
    pub fn on(&mut self, vm: &mut Vm) -> Result<Option<(i64, Vec<Stencil>)>, Exception> {
        let on = self.go(vm);
        if !matches!(on, Ok(None)) {
            vm.heap_mut().unroot_to(self.depth);
        }
        on
    }

    fn go(&mut self, vm: &mut Vm) -> Result<Option<(i64, Vec<Stencil>)>, Exception> {
        while let Some(&(book, is_last)) = self.leaves.get(self.next) {
            let paper = vm.heap().foreign::<PaperBook>(book).map(|b| b.paper);
            let Some(paper) = paper else {
                self.next += 1;
                continue;
            };
            match core::mem::replace(&mut self.leaf, Leaf::Starting) {
                Leaf::Starting => {
                    if !self.start(vm, book, paper, is_last)? {
                        continue;
                    }
                }
                Leaf::Solving(mut solving, depth) => match solving.on(vm)? {
                    Some(pages) => {
                        vm.heap_mut().unroot_to(depth);
                        set_pages(vm.heap(), book, pages);
                        self.leaf = Leaf::Stencils { pages, left: pages };
                    }
                    None => {
                        self.leaf = Leaf::Solving(solving, depth);
                        return Ok(None);
                    }
                },
                Leaf::Stencils { pages, left } => {
                    let (Some(p), Some(rest)) = (vm.heap().car(left), vm.heap().cdr(left)) else {
                        after_stencils(vm, book, paper, pages)?;
                        self.ended(vm, book, paper);
                        continue;
                    };
                    call_lily(vm, "%page-stencil", &[p])?;
                    crate::queue::check(crate::queue::Step::Pages);
                    self.leaf = Leaf::Stencils { pages, left: rest };
                }
            }
            if lily_engine_music::returns::due() {
                return Ok(None);
            }
        }
        output(vm, self.book).map(Some)
    }

    /// [`output_aux`] and [`pages`] for the next book, as far as its
    /// breaker: whether it is under way, else it is done with.
    fn start(
        &mut self,
        vm: &mut Vm,
        book: Value,
        paper: Value,
        is_last: bool,
    ) -> Result<bool, Exception> {
        let (made, empty) = match vm.heap().foreign::<PaperBook>(book) {
            Some(b) => (b.pages, b.print_elements.is_empty()),
            None => (Value::NULL, true),
        };
        if empty {
            self.next += 1;
            return Ok(false);
        }
        let per_part = paper_score::c_variable(vm, paper, "bookpart-level-page-numbering");
        if per_part != Value::TRUE {
            let n = vm.heap_mut().integer(self.first_page_number);
            set_variable(vm, paper, "first-page-number", n);
        }
        set_variable(vm, paper, "is-last-bookpart", Value::bool(is_last));
        let breaking = paper_score::c_variable(vm, paper, "page-breaking");
        if made != Value::FALSE || vm.lily("ly:optimal-breaking") != Some(breaking) {
            pages(vm, book)?;
            self.ended(vm, book, paper);
            return Ok(false);
        }
        set_pages(vm.heap(), book, Value::NULL);
        let depth = vm.heap().root_depth();
        vm.heap_mut().root(book);
        let solving = crate::optimal_page_breaking::Solving::begin(vm, book)?;
        self.leaf = Leaf::Solving(Box::new(solving), depth);
        Ok(true)
    }

    /// The book's pages are made: the next starts after them.
    fn ended(&mut self, vm: &mut Vm, book: Value, paper: Value) {
        let per_part = paper_score::c_variable(vm, paper, "bookpart-level-page-numbering");
        let pages = vm.heap().foreign::<PaperBook>(book).map(|b| b.pages);
        let mut list = Vec::new();
        push_items(vm.heap(), pages.unwrap_or(Value::NULL), &mut list);
        if per_part != Value::TRUE {
            self.first_page_number += list.len() as i64;
        }
        self.next += 1;
        self.leaf = Leaf::Starting;
    }
}

/// `Output_def::set_variable`: `name` defined as `value` in the scope of
/// the output definition `def`.
fn set_variable(vm: &mut Vm, def: Value, name: &str, value: Value) {
    if let Some(scope) = vm.heap().foreign::<OutputDef>(def).map(|d| d.scope) {
        vm.module_define(scope, name, value);
    }
}

/// `Paper_book::get_scopes` (`paper-book.cc:815–826`): the headers of
/// `book` and of the books above it that are modules, its own first.
pub fn get_scopes(heap: &mut Heap, book: Value) -> Value {
    let Some((parent, header)) = heap
        .foreign::<PaperBook>(book)
        .map(|b| (b.parent, b.header))
    else {
        return Value::NULL;
    };
    let scopes = get_scopes(heap, parent);
    if is_module(heap, header) {
        heap.cons(header, scopes)
    } else {
        scopes
    }
}

/// `(ly:paper-book-paper pb)` (`paper-book-scheme.cc:61–69`): the paper of
/// the `Paper_book` `pb`.
pub(crate) fn ly_paper_book_paper(cx: &mut Cx, args: &[Value]) -> R {
    let paper = book_arg(cx, "ly:paper-book-paper", args[0])?;
    Ok(paper)
}

/// `(ly:paper-book-scopes pb)` (`paper-book-scheme.cc:32–40`): the
/// headers [`get_scopes`] gives of the `Paper_book` `pb`.
pub(crate) fn ly_paper_book_scopes(cx: &mut Cx, args: &[Value]) -> R {
    book_arg(cx, "ly:paper-book-scopes", args[0])?;
    Ok(get_scopes(cx.heap_mut(), args[0]))
}

/// `(ly:paper-book-header pb)` (`paper-book-scheme.cc:71–79`): the header
/// of the `Paper_book` `pb`.
pub(crate) fn ly_paper_book_header(cx: &mut Cx, args: &[Value]) -> R {
    book_arg(cx, "ly:paper-book-header", args[0])?;
    Ok(cx
        .heap()
        .foreign::<PaperBook>(args[0])
        .map_or(Value::NULL, |b| b.header))
}

/// `ly:paper-book-pages` (`paper-book-scheme.cc:22–30`) and
/// `ly:paper-book-systems` (`:52–59`): [`pages`] and [`systems`] of a
/// `Paper_book`, over `%host-call` (D127, [`HOSTS`]).
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:paper-book-pages pb)
  (%host-call 'paper-book-pages pb))
(define-public (ly:paper-book-systems pb)
  (%host-call 'paper-book-systems pb))
"#;

/// What `%host-call` runs for [`PRELUDE`].
pub(crate) const HOSTS: [HostHandler; 2] = [
    HostHandler {
        what: "paper-book-pages",
        f: host_pages,
    },
    HostHandler {
        what: "paper-book-systems",
        f: host_systems,
    },
];

/// [`pages`] of the `Paper_book` `args[1]`; `LY_ASSERT_SMOB`'s type error
/// for what is none.
fn host_pages(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    host_book(vm, "ly:paper-book-pages", args[1])?;
    pages(vm, args[1])
}

/// [`systems`] of the `Paper_book` `args[1]`, as [`host_pages`].
fn host_systems(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    host_book(vm, "ly:paper-book-systems", args[1])?;
    systems(vm, args[1])
}

/// `LY_ASSERT_SMOB (Paper_book, v, 1)` of a `%host-call`.
fn host_book(vm: &mut Vm, subr: &str, v: Value) -> Result<(), Exception> {
    if vm.heap().foreign::<PaperBook>(v).is_some() {
        return Ok(());
    }
    Err(crate::wrong_type(
        vm.heap_mut(),
        Some(subr),
        1,
        v,
        "Paper_book",
    ))
}

/// `LY_ASSERT_SMOB (Paper_book, v, 1)`: the paper of the `Paper_book` `v`.
fn book_arg(cx: &mut Cx, subr: &str, v: Value) -> Result<Value, Thrown> {
    match cx.heap().foreign::<PaperBook>(v) {
        Some(b) => Ok(b.paper),
        None => Err(cx.wrong_type(subr, 1, v, "Paper_book")),
    }
}

/// `Text_interface::is_markup_list` (`text-interface.cc:372–376`):
/// `markup-list?` of `x`.
fn is_markup_list(host: &mut dyn Host, x: Value) -> Result<bool, Exception> {
    let markup_list_p = lily_procedure(host, "markup-list?")?;
    Ok(host.call(markup_list_p, &[x])? != Value::FALSE)
}

/// `ly_is_module`.
fn is_module(heap: &Heap, v: Value) -> bool {
    heap.foreign_object(v).map(|f| f.type_name()) == Some("module")
}

/// `set_property` of the paper system `prob`'s `name`.
fn set_prob(heap: &mut Heap, prob: Value, name: &str, v: Value) {
    let sym = heap.intern(name);
    prob_set(heap, prob, sym, v);
}

/// The items of the list `list` pushed onto `out`.
fn push_items(heap: &Heap, mut list: Value, out: &mut Vec<Value>) {
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        out.push(item);
        list = rest;
    }
}

/// `Book::process` of a toplevel `book` (`book.cc:197–202`, 303–330):
/// its `Paper_book`, its paper `book`'s or `default_paper` if it has none,
/// or `None` for no book; each item of it and of its parts, in the order
/// `process_score` gets them, pushed onto `out` with the `Paper_book` it
/// goes to. A book with parts has the scores added after its last part in
/// a part of their own (`process_bookparts`) and prints its parts' books;
/// a book or part without parts has its `Paper_book`'s paper normalized
/// (`Output_def::normalize`) before its items. A book whose scores have
/// errors is processed as one with none: the caller skips each such
/// score, as `Score::book_rendering` renders nothing of it, where LilyPond
/// processes no part of a toplevel book that has one (`error_found`).
pub fn process(
    vm: &mut Vm,
    book: Value,
    default_paper: Value,
    out: &mut Vec<(Value, Value)>,
) -> Result<Option<Value>, Exception> {
    process_part(vm, book, default_paper, None, out)
}

/// `Book::process` (`book.cc:303–330`) of `book`, a part if `parent_part`
/// is its book's `Paper_book`.
fn process_part(
    vm: &mut Vm,
    book: Value,
    default_paper: Value,
    parent_part: Option<Value>,
    out: &mut Vec<(Value, Value)>,
) -> Result<Option<Value>, Exception> {
    let Some(paper) = vm.heap().foreign::<Book>(book).map(|b| b.paper) else {
        return Ok(None);
    };
    let paper = if vm.heap().foreign::<OutputDef>(paper).is_some() {
        paper
    } else {
        default_paper
    };
    let printed = make(vm, paper, parent_part)?;
    // Rooted while it is made: its parts' are reached through it.
    let depth = vm.heap().root_depth();
    vm.heap_mut().root(printed);
    let made = fill(vm, book, paper, printed, out);
    vm.heap_mut().unroot_to(depth);
    made.map(|()| Some(printed))
}

/// The rest of `Book::process` of `book`, whose paper is `paper`, for its
/// `Paper_book` `printed`: its header, then its parts' or its items.
fn fill(
    vm: &mut Vm,
    book: Value,
    paper: Value,
    printed: Value,
    out: &mut Vec<(Value, Value)>,
) -> Result<(), Exception> {
    let header = vm
        .heap()
        .foreign::<Book>(book)
        .map_or(Value::FALSE, |b| b.header);
    if let Some(b) = vm.heap_mut().foreign_mut::<PaperBook>(printed) {
        b.header = header;
    }
    let parts = vm
        .heap()
        .foreign::<Book>(book)
        .map_or(Value::NULL, |b| b.parts);
    if vm.heap().car(parts).is_some() {
        // `process_bookparts` (`book.cc:204–223`).
        add_scores_to_bookpart(vm, book);
        let parts = vm
            .heap()
            .foreign::<Book>(book)
            .map_or(Value::NULL, |b| b.parts);
        for part in items(vm.heap(), parts) {
            if let Some(p) = process_part(vm, part, paper, Some(printed), out)? {
                add_bookpart(vm.heap_mut(), printed, p);
            }
        }
    } else {
        if let Some(own) = vm.heap().foreign::<PaperBook>(printed).map(|b| b.paper) {
            normalize(vm, own)?;
        }
        let scores = vm
            .heap()
            .foreign::<Book>(book)
            .map_or(Value::NULL, |b| b.scores);
        out.extend(items(vm.heap(), scores).into_iter().map(|s| (s, printed)));
    }
    Ok(())
}

/// The items of `list`, which holds the last added first, in the order
/// they were added (`scm_reverse`).
fn items(heap: &Heap, mut list: Value) -> Vec<Value> {
    let mut items = Vec::new();
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        items.push(item);
        list = rest;
    }
    items.reverse();
    items
}
