//! `optimal-page-breaking.cc` (part 5's, MS3 step 62) and
//! page-breaking-scheme.cc's `ly:optimal-breaking` (`:36–46`), the paper's
//! `page-breaking` by default (`paper-defaults-init.ly`): a book broken
//! into lines and pages together, to the least badness of both their
//! horizontal and their vertical spacing (`Optimal_page_breaking::solve`,
//! `:39–254`), over the page breaker's line and page sides
//! (`page_breaking.rs`, `page_spacing.rs`). Its pages are drawn by
//! `make_pages` (`page.rs`).
//!
//! `ly:optimal-breaking` is Scheme over `%host-call` ([`PRELUDE`],
//! [`HOSTS`], D127). LilyPond's progress ("Finding the ideal number of
//! pages...", "Fitting music on … pages...", "Drawing systems...") and its
//! `debug-page-breaking-scoring` messages are not written, as the engine
//! writes no progress; its warnings are.

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, HostHandler, Value, Vm};

use crate::grob;
use crate::page;
use crate::page_breaking::{Finding, LineDivision, PageBreaking, Substitutions};
use crate::page_spacing::{
    BAD_SPACING_PENALTY, PageSpacingResult, SYSTEM_COUNT_TOO_FEW, SYSTEM_COUNT_TOO_MANY,
};
use crate::paper_book::PaperBook;
use crate::paper_score;

/// `(ly:optimal-breaking paper-book)`: [`solve`] of the `Paper_book`
/// `paper-book`, over `%host-call`.
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:optimal-breaking paper-book)
  (%host-call 'optimal-breaking paper-book))
"#;

/// What `%host-call` runs for [`PRELUDE`] (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "optimal-breaking",
    f: host_optimal_breaking,
}];

/// `ly_optimal_breaking`: `LY_ASSERT_SMOB (Paper_book …)`'s type error for
/// what is no `Paper_book`, else [`solve`] of it.
fn host_optimal_breaking(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    if vm.heap().foreign::<PaperBook>(args[1]).is_none() {
        return Err(crate::wrong_type(
            vm.heap_mut(),
            Some("ly:optimal-breaking"),
            1,
            args[1],
            "Paper_book",
        ));
    }
    solve(vm, args[1])
}

/// `Optimal_page_breaking (pb)` and its `solve`: the pages of the
/// `Paper_book` `book`, its lines and pages chosen together. From the
/// ideal line configuration (each chunk's best lines) on its best number
/// of pages, or on the paper's `page-count` if that is an integer (warned
/// of and ignored if it is not positive; with `systems-per-page`, that
/// many systems a page, unless the two cannot be met together, warned of),
/// fewer systems are tried down to a lower bound (a page's worth fewer
/// than the ideal, at least 1) while the pages are not already too few and
/// stretched or too badly spaced, then more up to the most there can be
/// while the spacing is not infinitely bad; each count's configurations
/// spaced on pages, the best kept. Then the book's scores are broken into
/// the best division's lines (`break_into_pieces`), and their systems
/// (`systems`) drawn on the best result's pages (`make_pages`). What it
/// makes is rooted until it returns.
pub fn solve(host: &mut dyn Host, book: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    host.heap().root(book);
    let pages = Solving::begin(host, book).and_then(|mut solving| {
        loop {
            if let Some(pages) = solving.on(host)? {
                break Ok(pages);
            }
        }
    });
    host.heap().unroot_to(depth);
    pages
}

/// Where a [`Solving`] is.
enum Stage {
    /// The chunks and breaks, and each score's lines.
    Finding(Finding),
    /// The best division of them, chosen.
    Choosing,
    /// Broken into the division's lines, its grobs substituted, then drawn.
    Breaking(PageSpacingResult, LineDivision),
    Substituting(PageSpacingResult, Substitutions),
    Drawing(page::Drawing),
}

/// [`solve`] under way (H3, D254): [`Solving::on`] goes on until the pages
/// are made or the step's time is spent, between lines and between systems.
/// What it has rooted stays rooted until the pages are made.
pub struct Solving {
    pb: PageBreaking,
    stage: Option<Stage>,
}

impl Solving {
    pub fn begin(host: &mut dyn Host, book: Value) -> Result<Solving, Exception> {
        Ok(Solving {
            pb: PageBreaking::begin(host, book)?,
            stage: Some(Stage::Finding(Finding::default())),
        })
    }

    /// What it goes on with, for the steps' timings.
    pub fn stage(&self) -> &'static str {
        match self.stage {
            Some(Stage::Finding(_)) => "lines",
            Some(Stage::Choosing) => "choosing",
            Some(Stage::Breaking(..)) => "breaking",
            Some(Stage::Substituting(..)) => "substituting",
            Some(Stage::Drawing(_)) => "drawing",
            None => "",
        }
    }

    /// Goes on; the pages, once they are made.
    pub fn on(&mut self, host: &mut dyn Host) -> Result<Option<Value>, Exception> {
        let pb = &mut self.pb;
        loop {
            let stage = self.stage.take();
            match stage {
                Some(Stage::Finding(mut finding)) => {
                    let found = pb.find_on(host, None, None, &mut finding)?;
                    self.stage = Some(if found {
                        Stage::Choosing
                    } else {
                        Stage::Finding(finding)
                    });
                    if !found {
                        return Ok(None);
                    }
                }
                Some(Stage::Choosing) => {
                    let (best, division) = choose(host, pb)?;
                    self.stage = Some(Stage::Breaking(best, division));
                }
                Some(Stage::Breaking(best, division)) => {
                    let end = pb.last_break_position();
                    pb.break_into_pieces(host, 0, end, &division)?;
                    self.stage = Some(Stage::Substituting(best, Substitutions::default()));
                }
                Some(Stage::Substituting(best, mut substitutions)) => {
                    if !pb.substitute_on(host, &mut substitutions)? {
                        self.stage = Some(Stage::Substituting(best, substitutions));
                        return Ok(None);
                    }
                    let lines = pb.lines(host);
                    host.heap().root(lines);
                    let drawing = page::Drawing::begin(host, pb, &best.systems_per_page, lines)?;
                    self.stage = Some(Stage::Drawing(drawing));
                }
                Some(Stage::Drawing(mut drawing)) => {
                    let pages = drawing.on(host, pb)?;
                    if pages.is_none() {
                        self.stage = Some(Stage::Drawing(drawing));
                    }
                    return Ok(pages);
                }
                None => return Ok(None),
            }
            if lily_engine_music::returns::due() {
                return Ok(None);
            }
        }
    }
}

/// [`solve`]'s choice: the best spacing and the division of lines it is of.
fn choose(
    host: &mut dyn Host,
    pb: &mut PageBreaking,
) -> Result<(PageSpacingResult, LineDivision), Exception> {
    let paper = page::book_paper(host.heap(), pb.book);
    let end = pb.last_break_position();
    let mut max_sys_count = pb.max_system_count(host, 0, end)?;
    let first = paper_score::c_variable(host, paper, "first-page-number");
    let first_page_num = page::int_or(host.heap(), first, 1);

    pb.set_to_ideal_line_configuration(host, 0, end)?;

    let mut best: PageSpacingResult;
    let forced_page_count = paper_score::c_variable(host, paper, "page-count");
    let is_integer = host.heap().is_integer(forced_page_count);
    let mut page_count_is_forced = is_integer;
    let mut page_count = 0;
    let mut ideal_line_division = pb.current_configuration(0);
    let mut best_division = ideal_line_division.clone();
    let mut min_sys_count;

    // Note that system_count only counts non-title systems.
    let mut ideal_sys_count = pb.system_count;

    if page_count_is_forced {
        if !host.heap().is_positive(forced_page_count).unwrap_or(false) {
            grob::unlocated_warning(host, "page-count must be positive")?;
            page_count_is_forced = false;
        } else {
            page_count = to_vsize(host, forced_page_count)?;
        }
    }

    if page_count_is_forced {
        // If systems-per-page and page-count are both given, we know
        // exactly how many systems there should be.
        if pb.systems_per_page > 0 {
            ideal_sys_count = pb.systems_per_page as usize * page_count;

            if ideal_sys_count > pb.max_system_count(host, 0, end)?
                || ideal_sys_count < pb.min_system_count(host, 0, end)?
            {
                grob::unlocated_warning(
                    host,
                    "could not satisfy systems-per-page and page-count at the same time, ignoring systems-per-page",
                )?;
                ideal_sys_count = pb.system_count;
                min_sys_count = page_count;
            } else {
                pb.set_current_breakpoints(
                    host,
                    0,
                    end,
                    ideal_sys_count,
                    LineDivision::new(),
                    LineDivision::new(),
                )?;
                min_sys_count = ideal_sys_count;
                max_sys_count = ideal_sys_count;
                ideal_line_division = pb.current_configuration(0);
                best_division = ideal_line_division.clone();
            }
        } else {
            min_sys_count = page_count;
        }

        // This will give programming errors if the ideal line spacing
        // doesn't fit on page_count pages.
        best = pb.space_systems_on_n_pages(host, 0, page_count, first_page_num)?;
    } else {
        // The ideal number of pages.
        best = pb.space_systems_on_best_pages(host, 0, first_page_num)?;

        page_count = best.systems_per_page.len();
        if page_count == 0 {
            min_sys_count = 0;
        } else {
            min_sys_count = ideal_sys_count.wrapping_sub(best.systems_per_page[page_count - 1]);

            if page_count > 1 && best.systems_per_page[page_count - 2] > 1 {
                min_sys_count = min_sys_count.wrapping_sub(best.systems_per_page[page_count - 2]);
            }

            // A subtraction that wrapped around.
            if min_sys_count > ideal_sys_count || min_sys_count == 0 {
                min_sys_count = 1;
            }
        }
    }

    // Fewer systems than the ideal number for line breaking.
    let mut bound = ideal_line_division.clone();
    let mut sys_count = ideal_sys_count + 1;
    loop {
        sys_count = sys_count.wrapping_sub(1);
        if sys_count < min_sys_count {
            break;
        }
        let mut best_for_this_sys_count = PageSpacingResult::default();
        pb.set_current_breakpoints(host, 0, end, sys_count, LineDivision::new(), bound.clone())?;

        for i in 0..pb.current_configuration_count() {
            let cur = if is_integer {
                pb.space_systems_on_n_pages(host, i, page_count, first_page_num)?
            } else {
                pb.space_systems_on_best_pages(host, i, first_page_num)?
            };

            if cur.demerits < best_for_this_sys_count.demerits {
                best_for_this_sys_count = cur;
                bound = pb.current_configuration(i);
            }
        }

        if best_for_this_sys_count.demerits < best.demerits {
            best = best_for_this_sys_count.clone();
            best_division = bound.clone();
        }

        // Too few systems already, if one page fewer than the ideal number
        // is stretched on average, or the spacing is worse than
        // BAD_SPACING_PENALTY; unless max-systems-per-page asks for fewer.
        if best.system_count_status & SYSTEM_COUNT_TOO_MANY == 0 {
            if best_for_this_sys_count.page_count() < page_count
                && best_for_this_sys_count.average_force() > 0.0
            {
                break;
            }

            if best_for_this_sys_count.demerits >= BAD_SPACING_PENALTY {
                break;
            }
        }
    }

    // More systems than the ideal number for line breaking.
    bound = ideal_line_division;
    let mut prev_actual_sys_count = 0;
    let mut sys_count = ideal_sys_count + 1;
    while sys_count <= max_sys_count {
        let mut best_demerits_for_this_sys_count = f64::INFINITY;
        pb.set_current_breakpoints(host, 0, end, sys_count, bound.clone(), LineDivision::new())?;

        for i in 0..pb.current_configuration_count() {
            let min_p_count = pb.min_page_count(host, i, first_page_num)?;

            if min_p_count > page_count {
                continue;
            }
            let cur = if is_integer {
                pb.space_systems_on_n_pages(host, i, page_count, first_page_num)?
            } else {
                pb.space_systems_on_best_pages(host, i, first_page_num)?
            };

            if cur.demerits < best.demerits {
                best = cur.clone();
                best_division = pb.current_configuration(i);
            }

            if cur.demerits < best_demerits_for_this_sys_count {
                best_demerits_for_this_sys_count = cur.demerits;
                bound = pb.current_configuration(i);
            }
        }

        let actual_sys_count: usize = best.systems_per_page.iter().sum();
        // With infinitely bad results, stop if there are not too few
        // systems (a good result is had, and more systems will not help),
        // or if there are but more are not coming.
        if best_demerits_for_this_sys_count >= BAD_SPACING_PENALTY
            && (best.system_count_status & SYSTEM_COUNT_TOO_FEW == 0
                || actual_sys_count == prev_actual_sys_count)
        {
            break;
        }
        prev_actual_sys_count = actual_sys_count;
        sys_count += 1;
    }

    Ok((best, best_division))
}

/// `from_scm<vsize>` of a positive integer: Guile's type error for an
/// inexact one.
fn to_vsize(host: &mut dyn Host, v: Value) -> Result<usize, Exception> {
    let exact = host.heap().is_exact_integer(v);
    match host
        .heap()
        .to_i64(v)
        .filter(|_| exact)
        .and_then(|n| usize::try_from(n).ok())
    {
        Some(n) => Ok(n),
        None => Err(crate::wrong_type(host.heap(), None, 0, v, "exact integer")),
    }
}
