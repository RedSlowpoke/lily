//! The page breakers beside `ly:optimal-breaking` (MS8 step 9, part 3),
//! page-breaking-scheme.cc's other five, each a paper's `page-breaking`
//! over the page breaker's two sides (`page_breaking.rs`) and `make_pages`
//! (`page.rs`):
//!
//! - `ly:minimal-breaking` (`minimal-page-breaking.cc`): the ideal lines,
//!   as many on a page as fit before the next ([`minimal`]).
//! - `ly:one-page-breaking` (`one-page-breaking.cc`): the ideal lines on a
//!   single page whose `paper-height` is then fitted to them ([`one_page`]).
//! - `ly:one-line-breaking` (`one-line-page-breaking.cc`): each score on a
//!   single line of its own page, the `paper-width` widened to the widest
//!   ([`one_line`]); `ly:one-line-auto-height-breaking`
//!   (`one-line-auto-height-breaking.cc`), the `paper-height` also fitted
//!   to the tallest.
//! - `ly:page-turn-breaking` (`page-turn-page-breaking.cc`): lines and
//!   pages chosen together so that pages turn only where a column's or a
//!   prob's `page-turn-permission` allows it, which `Page_turn_engraver`
//!   sets (`page_turn_engraver.rs`), by dynamic programming over the
//!   turns ([`page_turn`]).
//!
//! Each is Scheme over `%host-call` ([`PRELUDE`], [`HOSTS`], D127), and
//! makes its pages in one step (`paper_book.rs`' `Output` steps the
//! default breaker alone). LilyPond's progress messages and the page-turn
//! breaker's `debug-page-breaking-scoring` output are not written, as the
//! engine writes no progress; its warnings are.

use alloc::vec::Vec;

use lily_engine_music::{Host, Prob, prob_get};
use lily_engine_scheme::{Exception, HostHandler, Tag, Value, Vm};

use crate::call_lily;
use crate::constrained_breaking::VPOS;
use crate::geometry::{Axis, X, Y};
use crate::grob;
use crate::page;
use crate::page_breaking::{LineDivision, PageBreaking};
use crate::paper_book::PaperBook;
use crate::paper_score;
use crate::simple_spacer::get_line_configuration;
use crate::stencil::Stencil;
use crate::system;

/// The five breakers' procedures over `%host-call`.
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:page-turn-breaking paper-book)
  (%host-call 'page-turn-breaking paper-book))
(define-public (ly:minimal-breaking paper-book)
  (%host-call 'minimal-breaking paper-book))
(define-public (ly:one-page-breaking paper-book)
  (%host-call 'one-page-breaking paper-book))
(define-public (ly:one-line-breaking paper-book)
  (%host-call 'one-line-breaking paper-book))
(define-public (ly:one-line-auto-height-breaking paper-book)
  (%host-call 'one-line-auto-height-breaking paper-book))
"#;

/// What `%host-call` runs for [`PRELUDE`] (D127).
pub(crate) const HOSTS: [HostHandler; 5] = [
    HostHandler {
        what: "page-turn-breaking",
        f: |vm, args| host(vm, args, "ly:page-turn-breaking", page_turn),
    },
    HostHandler {
        what: "minimal-breaking",
        f: |vm, args| host(vm, args, "ly:minimal-breaking", minimal),
    },
    HostHandler {
        what: "one-page-breaking",
        f: |vm, args| host(vm, args, "ly:one-page-breaking", one_page),
    },
    HostHandler {
        what: "one-line-breaking",
        f: |vm, args| host(vm, args, "ly:one-line-breaking", one_line),
    },
    HostHandler {
        what: "one-line-auto-height-breaking",
        f: |vm, args| {
            host(
                vm,
                args,
                "ly:one-line-auto-height-breaking",
                one_line_auto_height,
            )
        },
    },
];

type Solve = fn(&mut dyn Host, Value) -> Result<Value, Exception>;

/// `LY_ASSERT_SMOB (Paper_book …)`'s type error for what is no
/// `Paper_book`, else `solve` of it, what it makes rooted until it returns.
fn host(vm: &mut Vm, args: &[Value], name: &str, solve: Solve) -> Result<Value, Exception> {
    let book = args[1];
    if vm.heap().foreign::<PaperBook>(book).is_none() {
        return Err(crate::wrong_type(
            vm.heap_mut(),
            Some(name),
            1,
            book,
            "Paper_book",
        ));
    }
    let depth = vm.heap().root_depth();
    vm.heap_mut().root(book);
    let pages = solve(vm, book);
    vm.heap_mut().unroot_to(depth);
    pages
}

/// `Output_def::set_variable` of the paper `paper`.
fn set_variable(
    host: &mut dyn Host,
    paper: Value,
    name: &str,
    value: Value,
) -> Result<(), Exception> {
    let sym = host.heap().intern(name);
    call_lily(host, "ly:output-def-set-variable!", &[paper, sym, value])?;
    Ok(())
}

/// `from_scm<double> (paper->c_variable (name), 0.0)`.
fn number(host: &mut dyn Host, paper: Value, name: &str) -> f64 {
    let v = paper_score::c_variable(host, paper, name);
    host.heap().to_f64(v).unwrap_or(0.0)
}

/// `from_scm (paper->c_variable ("first-page-number"), 1)`.
fn first_page_number(host: &mut dyn Host, paper: Value) -> i64 {
    let v = paper_score::c_variable(host, paper, "first-page-number");
    page::int_or(host.heap(), v, 1)
}

/// `Grob::extent (system, a)` of a system relative to itself, its length.
fn system_length(host: &mut dyn Host, system: Value, a: Axis) -> Result<f64, Exception> {
    Ok(grob::extent(host, system, system, a)?.length())
}

/// The Y length of the stencil property `name` of the prob `prob`, 0 for
/// none (where LilyPond reads through a null).
fn stencil_height(host: &mut dyn Host, prob: Value, name: &str) -> f64 {
    let heap = host.heap();
    let sym = heap.intern(name);
    let s = prob_get(heap, prob, sym);
    heap.foreign::<Stencil>(s)
        .map_or(0.0, |s| s.extent(Y).length())
}

/// The items of the list `list`.
fn items(host: &mut dyn Host, mut list: Value) -> Vec<Value> {
    let heap = host.heap();
    let mut out = Vec::new();
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        out.push(item);
        list = rest;
    }
    out
}

// --- minimal-page-breaking.cc ---------------------------------------------------------

/// `Minimal_page_breaking::solve`: the ideal line configuration, broken
/// into its lines, packed on the fewest pages from the paper's
/// `first-page-number`.
pub fn minimal(host: &mut dyn Host, book: Value) -> Result<Value, Exception> {
    let mut pb = PageBreaking::new(host, book, None, None)?;
    let end = pb.last_break_position();

    pb.set_to_ideal_line_configuration(host, 0, end)?;
    let div = pb.current_configuration(0);
    pb.break_into_pieces(host, 0, end, &div)?;

    let paper = page::book_paper(host.heap(), book);
    let first_page_num = first_page_number(host, paper);
    let res = pb.pack_systems_on_least_pages(host, 0, first_page_num)?;
    let lines = pb.systems(host)?;
    host.heap().root(lines);
    page::make_pages(host, &pb, &res.systems_per_page, lines)
}

// --- one-page-breaking.cc -------------------------------------------------------------

/// `read_spacing_alist (spec, sym)`: the number of `sym` in the alist
/// `spec`, 0 if it has none.
fn read_spacing_alist(host: &mut dyn Host, spec: Value, sym: &str) -> f64 {
    let heap = host.heap();
    let sym = heap.intern(sym);
    let mut p = spec;
    while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if heap.car(entry) == Some(sym) {
            return match heap.cdr(entry) {
                Some(v) if heap.is_real(v) => heap.to_f64(v).unwrap_or(0.0),
                _ => 0.0,
            };
        }
        p = rest;
    }
    0.0
}

/// `One_page_breaking::solve`: the `paper-height` set to 1e6 for the
/// while (Stencil::translate's largest for the tagline), the ideal lines
/// broken and spaced on one page; then the `paper-height` fitted to them:
/// the lowest bound of any line on the page (its position and its height,
/// a system's Y extent or a markup's stencil's), with
/// `last-bottom-spacing`'s `padding`, or the last line's position past its
/// staff's refpoint by the greater of `basic-distance` and
/// `minimum-distance` if that is lower, and the foot's height and both
/// margins added. The page's `bottom-edge`, where its footer goes, is the
/// height less the bottom margin.
pub fn one_page(host: &mut dyn Host, book: Value) -> Result<Value, Exception> {
    let mut pb = PageBreaking::new(host, book, None, None)?;
    let paper = page::book_paper(host.heap(), book);
    // Stencil::translate throws a programming error (for the tagline's
    // position) at any larger height.
    let tall = host.heap().flonum(1e6);
    set_variable(host, paper, "paper-height", tall)?;

    // The lines.
    let end = pb.last_break_position();
    pb.set_to_ideal_line_configuration(host, 0, end)?;
    let div = pb.current_configuration(0);
    pb.break_into_pieces(host, 0, end, &div)?;

    // The page.
    let first_page_num = first_page_number(host, paper);
    let res = pb.space_systems_on_n_pages(host, 0, 1, first_page_num)?;
    let lines = pb.systems(host)?;
    host.heap().root(lines);
    let pages = page::make_pages(host, &pb, &res.systems_per_page, lines)?;
    host.heap().root(pages);
    let Some(page_pb) = host.heap().car(pages) else {
        return Ok(pages);
    };

    // The lines' positions, larger lower on the page: the last is not
    // necessarily the lowest.
    let config_sym = host.heap().intern("configuration");
    let config = prob_get(host.heap(), page_pb, config_sym);
    let mut line_posns = Vec::new();
    let mut lowest_line_pos = 0.0;
    for this_pos in items(host, config) {
        let pos = host.heap().to_f64(this_pos).unwrap_or(0.0);
        line_posns.push(pos);
        if pos > lowest_line_pos {
            lowest_line_pos = pos;
        }
    }

    // The lowest lower bound of the lines on the page.
    let mut line_heights = Vec::new();
    for spec in pb.system_specs.clone() {
        if let Some(pscore) = spec.pscore {
            // Musical systems.
            let Some(root) = paper_score::root_system(host.heap(), pscore) else {
                continue;
            };
            let broken = system::get_broken_system_grobs(host.heap(), root);
            for s in items(host, broken) {
                line_heights.push(system_length(host, s, Y)?);
            }
        } else if let Some(prob) = spec.prob {
            // Top-level markups.
            line_heights.push(stencil_height(host, prob, "stencil"));
        }
    }
    let mut lowest_bound: f64 = 0.0;
    for (height, pos) in line_heights.iter().zip(&line_posns) {
        let low_bound = height + pos;
        if low_bound > lowest_bound {
            lowest_bound = low_bound;
        }
    }

    // `last-bottom-spacing`.
    let last_bottom = paper_score::c_variable(host, paper, "last-bottom-spacing");
    lowest_bound += read_spacing_alist(host, last_bottom, "padding");
    let basic_dist = read_spacing_alist(host, last_bottom, "basic-distance");
    let minimum_dist = read_spacing_alist(host, last_bottom, "minimum-distance");
    let max_dist = basic_dist.max(minimum_dist);

    // A musical system's distance from its refpoint to its upper bound;
    // a top-level markup's refpoint is 0.
    let lines_sym = host.heap().intern("lines");
    let lines_probs = prob_get(host.heap(), page_pb, lines_sym);
    let mut refpoint_dist = 0.0;
    if let Some(&last_line_pb) = items(host, lines_probs).last() {
        let sym = host.heap().intern("staff-refpoint-extent");
        let heap = host.heap();
        let refpoint_extent = prob_get(heap, last_line_pb, sym);
        if let Some(car) = heap.car(refpoint_extent)
            && heap.is_real(car)
        {
            refpoint_dist = -heap.to_f64(car).unwrap_or(0.0);
        }
    }
    let last_bottom_bound = lowest_line_pos + (refpoint_dist + max_dist);
    if last_bottom_bound > lowest_bound {
        lowest_bound = last_bottom_bound;
    }

    // The paper's height.
    let foot_height = stencil_height(host, page_pb, "foot-stencil");
    let top_margin = paper_score::c_variable(host, paper, "top-margin");
    let bottom_margin = paper_score::c_variable(host, paper, "bottom-margin");
    let heap = host.heap();
    let (top_margin, bottom_margin) = (
        heap.to_f64(top_margin).unwrap_or(0.0),
        heap.to_f64(bottom_margin).unwrap_or(0.0),
    );
    let margins = top_margin + bottom_margin;
    let ppr_height = margins + (lowest_bound + foot_height);
    let v = heap.flonum(ppr_height);
    set_variable(host, paper, "paper-height", v)?;

    // `bottom-edge` places the footer (the tagline, footnotes).
    let heap = host.heap();
    let edge = heap.flonum(ppr_height - bottom_margin);
    let sym = heap.intern("bottom-edge");
    lily_engine_music::prob_set(heap, page_pb, sym, edge);
    Ok(pages)
}

// --- one-line-page-breaking.cc and one-line-auto-height-breaking.cc -------------------

/// `One_line_page_breaking::solve_and_provide_max_height`: each score's
/// used columns on one line (no indent, the largest line width, ragged),
/// broken into that line and drawn on a page of its own; the paper's
/// `paper-width` the widest line's width with the left and right margins.
/// The pages, and the tallest line's height.
fn solve_one_line(host: &mut dyn Host, book: Value) -> Result<(Value, f64), Exception> {
    let pb = PageBreaking::new(host, book, None, None)?;
    let mut max_width: f64 = 0.0;
    let mut max_height: f64 = 0.0;
    let mut all_pages = Vec::new();
    for spec in pb.system_specs.clone() {
        let Some(pscore) = spec.pscore else {
            continue;
        };
        let Some(root) = paper_score::root_system(host.heap(), pscore) else {
            continue;
        };
        let cols = system::used_columns_in_range(host, root, 0, usize::MAX)?;
        if cols.is_empty() {
            continue;
        }
        let pos = get_line_configuration(host, &cols, f64::MAX, 0.0, true)?;
        if let Some(ps) = host.heap().foreign_mut::<paper_score::PaperScore>(pscore) {
            ps.broken = true;
        }
        system::break_into_pieces(host, root, &[pos])?;
        paper_score::substitute(host, pscore, root)?;
        let broken = system::get_broken_system_grobs(host.heap(), root);
        let Some(s) = host.heap().car(broken) else {
            continue;
        };

        let systems = host.heap().list(&[s]);
        host.heap().root(systems);
        let pages = page::make_pages(host, &pb, &[1], systems)?;
        host.heap().root(pages);

        max_width = max_width.max(system_length(host, s, X)?);
        max_height = max_height.max(system_length(host, s, Y)?);
        if let Some(p) = host.heap().car(pages) {
            all_pages.push(p);
        }
    }

    // The paper's width fits every system.
    let paper = page::book_paper(host.heap(), book);
    let right_margin = number(host, paper, "right-margin");
    let left_margin = number(host, paper, "left-margin");
    let width = max_width + right_margin + left_margin;
    let v = host.heap().flonum(width);
    set_variable(host, paper, "paper-width", v)?;

    let pages = host.heap().list(&all_pages);
    Ok((pages, max_height))
}

/// `One_line_page_breaking::solve`.
pub fn one_line(host: &mut dyn Host, book: Value) -> Result<Value, Exception> {
    Ok(solve_one_line(host, book)?.0)
}

/// `One_line_auto_height_breaking::solve`: as [`one_line`], and the
/// paper's `paper-height` the tallest line's height with the top and
/// bottom margins.
pub fn one_line_auto_height(host: &mut dyn Host, book: Value) -> Result<Value, Exception> {
    let (pages, max_height) = solve_one_line(host, book)?;
    host.heap().root(pages);
    let paper = page::book_paper(host.heap(), book);
    let top_margin = number(host, paper, "top-margin");
    let bottom_margin = number(host, paper, "bottom-margin");
    let height = max_height + top_margin + bottom_margin;
    let v = host.heap().flonum(height);
    set_variable(host, paper, "paper-height", v)?;
    Ok(pages)
}

// --- page-turn-page-breaking.cc -------------------------------------------------------

/// `is_break<T>`: whether a page may turn at the column or prob `g`: its
/// `page-turn-permission` is a symbol, and its page and line breaks are
/// allowed, else LilyPond's programming error and not.
fn is_turn(host: &mut dyn Host, g: Value) -> Result<bool, Exception> {
    let read = |host: &mut dyn Host, name: &str| -> Result<Value, Exception> {
        if host.heap().foreign::<Prob>(g).is_some() {
            let sym = host.heap().intern(name);
            Ok(prob_get(host.heap(), g, sym))
        } else {
            grob::property(host, g, name)
        }
    };
    let is_symbol = |v: Value| v.tag() == Tag::Symbol;

    let mut turnable = is_symbol(read(host, "page-turn-permission")?);
    if turnable {
        let page_breakable = is_symbol(read(host, "page-break-permission")?);
        let line_breakable = is_symbol(read(host, "line-break-permission")?);
        if !page_breakable || !line_breakable {
            grob::unlocated_programming_error(
                host,
                "found a page-turnable place which was not breakable",
            )?;
            turnable = false;
        }
    }
    Ok(turnable)
}

/// `Page_turn_page_breaking::Break_node`.
#[derive(Clone)]
struct BreakNode {
    /// The node before, [`VPOS`] for none.
    prev: usize,
    first_page_number: i64,
    page_count: usize,
    /// Every score here too widely spaced.
    too_many_lines: bool,
    demerits: f64,
    /// An index into the breaks.
    break_pos: usize,
    div: LineDivision,
    /// The systems of each page.
    system_count: Vec<usize>,
}

impl Default for BreakNode {
    fn default() -> BreakNode {
        BreakNode {
            prev: VPOS,
            first_page_number: 0,
            page_count: 0,
            too_many_lines: false,
            demerits: f64::INFINITY,
            break_pos: VPOS,
            div: LineDivision::new(),
            system_count: Vec::new(),
        }
    }
}

/// `total_page_count`: the pages a node takes with its blank page, if it
/// has one, in LilyPond's unsigned arithmetic.
fn total_page_count(b: &BreakNode) -> u64 {
    let first = b.first_page_number as u64;
    let end = first.wrapping_add(b.page_count as u64);
    end.wrapping_sub(1)
        .wrapping_add(end % 2)
        .wrapping_sub(first)
}

/// `Page_turn_page_breaking`: the page breaker and its `state_`.
struct PageTurn {
    pb: PageBreaking,
    state: Vec<BreakNode>,
}

impl PageTurn {
    /// The paper's `auto-first-page-number`, `#t` alone true.
    fn auto_first(&self, host: &mut dyn Host) -> bool {
        let paper = page::book_paper(host.heap(), self.pb.book);
        paper_score::c_variable(host, paper, "auto-first-page-number") == Value::TRUE
    }

    /// `put_systems_on_pages (start, end, configuration, page_number)`:
    /// the systems between the breaks spaced on pages (an odd count of
    /// them, or one more with the blank page's penalty, by the side the
    /// first starts on), with the demerits of the best node ending at the
    /// break before `start`; none (infinite demerits) where the span holds
    /// a break and would take more than two pages.
    fn put_systems_on_pages(
        &mut self,
        host: &mut dyn Host,
        start: usize,
        end: usize,
        configuration: usize,
        page_number: i64,
    ) -> Result<BreakNode, Exception> {
        let min_p_count = self.pb.min_page_count(host, configuration, page_number)?;
        let auto_first = self.auto_first(host);

        // Without a breakpoint between them, a bad turn may be the only
        // solution: the page count is not held against it.
        if start + 1 < end && min_p_count as i64 + if auto_first { 0 } else { page_number % 2 } > 2
        {
            return Ok(BreakNode::default());
        }

        // An odd page number starts on a right-hand page: an odd number of
        // pages, or an even number and a blank page; an even one, the
        // other way round. The first is taken where `min_p_count` has the
        // evenness it needs.
        let result = if start == 0 && auto_first {
            if !min_p_count.is_multiple_of(2) {
                self.pb.space_systems_on_n_or_one_more_pages(
                    host,
                    configuration,
                    min_p_count,
                    page_number,
                    0.0,
                )?
            } else {
                self.pb
                    .space_systems_on_n_pages(host, configuration, min_p_count, page_number)?
            }
        } else if (page_number % 2 == 0) == min_p_count.is_multiple_of(2) {
            self.pb
                .space_systems_on_n_pages(host, configuration, min_p_count, page_number)?
        } else {
            let penalty = self.pb.blank_page_penalty(host);
            self.pb.space_systems_on_n_or_one_more_pages(
                host,
                configuration,
                min_p_count,
                page_number,
                penalty,
            )?
        };

        let mut ret = BreakNode {
            prev: start.wrapping_sub(1),
            break_pos: end,
            page_count: result.force.len(),
            first_page_number: page_number,
            ..BreakNode::default()
        };
        if auto_first && start == 0 && ret.page_count.is_multiple_of(2) {
            ret.first_page_number += 1;
        }

        ret.div = self.pb.current_configuration(configuration);
        ret.system_count = result.systems_per_page;

        ret.too_many_lines = self.pb.all_lines_stretched(host, configuration)?;
        ret.demerits = result.demerits;
        if start > 0 {
            ret.demerits += self.state[start - 1].demerits;
        }
        Ok(ret)
    }

    /// `calc_subproblem (ending_breakpoint)`: the best node ending at the
    /// break after `ending_breakpoint`, from each start back to a forced
    /// turn, each start's page number following its node before (even,
    /// a left-hand page, but possibly the first), and each number of
    /// systems from the fewest (at least the last best's) up while the
    /// pages are not infinitely bad nor more than the best's.
    fn calc_subproblem(
        &mut self,
        host: &mut dyn Host,
        ending_breakpoint: usize,
    ) -> Result<(), Exception> {
        let end = ending_breakpoint + 1;
        let mut best = BreakNode::default();
        let mut this_start_best = BreakNode::default();
        let mut prev_best_system_count = 0;
        let force = self.pb.force_symbol;
        let paper = page::book_paper(host.heap(), self.pb.book);

        let mut start = end;
        while start > 0 {
            start -= 1;
            if start + 1 < end
                && self
                    .pb
                    .breakpoint_property(host, start + 1, "page-turn-permission")?
                    == force
            {
                break;
            }

            if start > 0 && best.demerits < self.state[start - 1].demerits {
                continue;
            }

            let mut p_num = first_page_number(host, paper);
            if start > 0 {
                // But possibly for the first page, a node's first page is
                // even, a left-hand page.
                let before = &self.state[start - 1];
                p_num = before.first_page_number;
                p_num += before.page_count as i64;
                p_num += p_num % 2;
            }

            let mut min_division = LineDivision::new();
            let max_division = LineDivision::new();

            let mut min_sys_count = self.pb.min_system_count(host, start, end)?;
            let max_sys_count = self.pb.max_system_count(host, start, end)?;
            this_start_best.demerits = f64::INFINITY;

            let mut ok_page = true;

            // Having just added a breakpoint, at least as many systems as
            // before are needed.
            min_sys_count = min_sys_count.max(prev_best_system_count);
            let mut sys_count = min_sys_count;
            while sys_count <= max_sys_count && ok_page {
                self.pb.set_current_breakpoints(
                    host,
                    start,
                    end,
                    sys_count,
                    min_division.clone(),
                    max_division.clone(),
                )?;
                let mut found = false;

                for i in 0..self.pb.current_configuration_count() {
                    let cur = self.put_systems_on_pages(host, start, end, i, p_num)?;

                    if cur.demerits.is_infinite()
                        || (cur.page_count as i64 + p_num % 2 > 2
                            && !this_start_best.demerits.is_infinite()
                            && total_page_count(&cur) > total_page_count(&this_start_best))
                    {
                        ok_page = false;
                        break;
                    }

                    if cur.demerits < this_start_best.demerits {
                        found = true;
                        this_start_best = cur;
                        prev_best_system_count = sys_count;

                        // With more systems, the division is bounded below
                        // by the best so far.
                        min_division = self.pb.current_configuration(i);
                    }
                }
                if !found && this_start_best.too_many_lines {
                    break;
                }
                sys_count += 1;
            }

            if this_start_best.demerits.is_infinite() {
                break;
            }

            if start == 0
                && end == 1
                && this_start_best.first_page_number == 1
                && this_start_best.page_count > 1
            {
                grob::unlocated_warning(
                    host,
                    "cannot fit the first page turn onto a single page.  Consider setting first-page-number to an even number.",
                )?;
            }

            if this_start_best.demerits < best.demerits {
                best = this_start_best.clone();
            }
        }
        self.state.push(best);
        Ok(())
    }

    /// `make_lines`: each node's scores broken into its division's lines;
    /// the book's systems.
    fn make_lines(&mut self, host: &mut dyn Host, soln: &[BreakNode]) -> Result<Value, Exception> {
        for (n, node) in soln.iter().enumerate() {
            let start = if n > 0 { soln[n - 1].break_pos } else { 0 };
            self.pb
                .break_into_pieces(host, start, node.break_pos, &node.div)?;
        }
        self.pb.systems(host)
    }

    /// `make_pages`: the nodes' pages, a blank page after a node that ends
    /// on a right-hand page but the last; the paper's `first-page-number`
    /// the first node's (which only `auto-first-page-number` changes).
    fn make_pages(
        &mut self,
        host: &mut dyn Host,
        soln: &[BreakNode],
        systems: Value,
    ) -> Result<Value, Exception> {
        if systems == Value::NULL || soln.is_empty() {
            return Ok(Value::NULL);
        }
        let mut lines_per_page = Vec::new();
        for (i, node) in soln.iter().enumerate() {
            lines_per_page.extend_from_slice(&node.system_count[..node.page_count]);
            if i + 1 < soln.len() && (node.first_page_number + node.page_count as i64) % 2 != 0 {
                // A blank page.
                lines_per_page.push(0);
            }
        }

        let paper = page::book_paper(host.heap(), self.pb.book);
        let first = host.heap().integer(soln[0].first_page_number);
        set_variable(host, paper, "first-page-number", first)?;
        page::make_pages(host, &self.pb, &lines_per_page, systems)
    }
}

/// `Page_turn_page_breaking::solve`: the best node ending at each break in
/// turn, then the chain of nodes back from the last, broken into lines and
/// drawn on pages.
pub fn page_turn(host: &mut dyn Host, book: Value) -> Result<Value, Exception> {
    let pb = PageBreaking::new(host, book, Some(is_turn), Some(is_turn))?;
    let mut solver = PageTurn {
        pb,
        state: Vec::new(),
    };
    for i in 0..solver.pb.last_break_position() {
        solver.calc_subproblem(host, i)?;
    }

    let mut breaking = Vec::new();
    let mut i = solver.state.len().wrapping_sub(1);
    while i != VPOS && i < solver.state.len() {
        breaking.push(solver.state[i].clone());
        i = solver.state[i].prev;
    }
    breaking.reverse();

    let systems = solver.make_lines(host, &breaking)?;
    host.heap().root(systems);
    solver.make_pages(host, &breaking, systems)
}
