//! `page-spacing.cc` and `page-spacing-result.cc` (part 5's, MS3 step
//! 62): the vertical spacing problem of a page breaker's lines on pages.
//! `Page_spacing` (`page-spacing.cc:28–131`), the rods and springs of the
//! lines on one page and their force; `Page_spacer` (`:133–405`), the
//! dynamic programme that puts the lines on the best number of pages, or
//! on a given number; and `Page_spacing_result` (`page-spacing-result.cc`)
//! but its debug print, what they give the page breaker
//! (`page_breaking.rs`).
//!
//! Each reads the page breaker's settings and asks it the height of a
//! page (`page_height`, which makes a page of `page.scm` and may call
//! Scheme), so it is given the [`PageBreaking`] and a [`Host`] where
//! LilyPond keeps a pointer to its breaker.

use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Heap, Value};

use crate::constrained_breaking::{LineDetails, Matrix, VPOS};
use crate::page_breaking::PageBreaking;

/// `BAD_SPACING_PENALTY`: a spacing bad enough to be worse than any
/// proper one, but not discarded.
pub const BAD_SPACING_PENALTY: f64 = 1e6;
/// `TERRIBLE_SPACING_PENALTY`: a user's override not met
/// (`min-systems-per-page`, `max-systems-per-page`).
pub const TERRIBLE_SPACING_PENALTY: f64 = 1e8;

/// `System_count_status`, a bitfield over the pages of a result.
pub const SYSTEM_COUNT_OK: i32 = 0;
pub const SYSTEM_COUNT_TOO_MANY: i32 = 1;
pub const SYSTEM_COUNT_TOO_FEW: i32 = 2;

/// `Page_spacing_result`: the lines of each page, each page's force, the
/// penalty and demerits of the whole, and whether a page has too many or
/// too few lines.
#[derive(Clone, Debug, PartialEq)]
pub struct PageSpacingResult {
    pub systems_per_page: Vec<usize>,
    pub force: Vec<f64>,
    pub penalty: f64,
    pub demerits: f64,
    pub system_count_status: i32,
}

impl Default for PageSpacingResult {
    /// `Page_spacing_result ()`: no pages, infinite demerits.
    fn default() -> PageSpacingResult {
        PageSpacingResult {
            systems_per_page: Vec::new(),
            force: Vec::new(),
            penalty: 0.0,
            demerits: f64::INFINITY,
            system_count_status: SYSTEM_COUNT_OK,
        }
    }
}

impl PageSpacingResult {
    /// `page_count`.
    pub fn page_count(&self) -> usize {
        self.systems_per_page.len()
    }

    /// `average_force`: the pages' forces' mean (NaN for no pages, as
    /// LilyPond's 0 / 0).
    pub fn average_force(&self) -> f64 {
        let mut average_force = 0.0;
        for i in 0..self.page_count() {
            average_force += self.force[i];
        }
        average_force / self.page_count() as f64
    }
}

/// `Page_spacing`: the rods and springs of lines stacked on a page of
/// `page_height`, and the force that fills it.
#[derive(Clone, Debug)]
pub struct PageSpacing {
    pub force: f64,
    pub page_height: f64,
    pub rod_height: f64,
    pub spring_len: f64,
    pub inverse_spring_k: f64,
    pub has_footnotes: bool,
    pub last_line: LineDetails,
    pub first_line: LineDetails,
}

impl PageSpacing {
    /// `Page_spacing (page_height, breaker)`: no lines.
    pub fn new(heap: &mut Heap, page_height: f64) -> PageSpacing {
        PageSpacing {
            force: 0.0,
            page_height,
            rod_height: 0.0,
            spring_len: 0.0,
            inverse_spring_k: 0.0,
            has_footnotes: false,
            last_line: LineDetails::new(heap),
            first_line: LineDetails::new(heap),
        }
    }

    /// `calc_force`: the force that stretches the springs to fill the page
    /// less the least whitespace at its top and bottom; −∞ if the rods and
    /// the last line's bottom padding fill it already.
    pub fn calc_force(&mut self, pb: &PageBreaking) {
        let height = self.page_height
            - pb.min_whitespace_at_top_of_page(&self.first_line)
            - pb.min_whitespace_at_bottom_of_page(&self.last_line);

        if self.rod_height + self.last_line.bottom_padding >= height {
            self.force = f64::NEG_INFINITY;
        } else {
            self.force =
                (height - self.rod_height - self.last_line.bottom_padding - self.spring_len)
                    / f64::max(0.1, self.inverse_spring_k);
        }
    }

    /// `resize (new_height)`.
    pub fn resize(&mut self, pb: &PageBreaking, new_height: f64) {
        self.page_height = new_height;
        self.calc_force(pb);
    }

    /// `append_system (line)`: `line` under the lines so far.
    pub fn append_system(&mut self, pb: &PageBreaking, line: &LineDetails) {
        if self.rod_height != 0.0 {
            self.rod_height += line.tallness;
            self.spring_len += self.last_line.spring_length(line);
        } else {
            self.rod_height += line.full_height();
            self.first_line = line.clone();
        }

        self.rod_height += self.account_for_footnotes(pb, line);
        self.inverse_spring_k += line.inverse_hooke;

        self.last_line = line.clone();

        self.calc_force(pb);
    }

    /// `account_for_footnotes (line)`: the height `line`'s in-notes and
    /// footnotes take, with their paddings, the footnotes' separator before
    /// the page's first.
    pub fn account_for_footnotes(&mut self, pb: &PageBreaking, line: &LineDetails) -> f64 {
        let mut footnote_height = 0.0;
        let mut in_note_height = 0.0;
        let mut has_in_notes = false;
        for &h in &line.in_note_heights {
            in_note_height += if has_in_notes {
                0.0
            } else {
                pb.in_note_system_padding
            };
            has_in_notes = true;
            in_note_height += h;
            in_note_height += pb.in_note_padding;
        }

        for &h in &line.footnote_heights {
            footnote_height += if self.has_footnotes {
                0.0
            } else {
                pb.footnote_separator_stencil_height
                    + pb.footnote_padding
                    + pb.footnote_number_raise
            };
            self.has_footnotes = true;
            footnote_height += h;
            footnote_height += pb.footnote_padding;
        }

        (in_note_height
            + if has_in_notes {
                -pb.in_note_padding + pb.in_note_system_padding
            } else {
                0.0
            })
            + (footnote_height
                + if self.has_footnotes {
                    -pb.footnote_padding + pb.footnote_footer_padding
                } else {
                    0.0
                })
    }

    /// `prepend_system (line)`: `line` above the lines so far.
    pub fn prepend_system(&mut self, pb: &PageBreaking, line: &LineDetails) {
        if self.rod_height != 0.0 {
            self.spring_len += line.spring_length(&self.first_line);
        } else {
            self.last_line = line.clone();
        }

        self.rod_height -= self.first_line.full_height();
        self.rod_height += self.first_line.tallness;
        self.rod_height += line.full_height();
        self.rod_height += self.account_for_footnotes(pb, line);
        self.inverse_spring_k += line.inverse_hooke;

        self.first_line = line.clone();

        self.calc_force(pb);
    }

    /// `clear`: no lines, the page's height and last lines kept.
    pub fn clear(&mut self) {
        self.force = 0.0;
        self.rod_height = 0.0;
        self.spring_len = 0.0;
        self.inverse_spring_k = 0.0;
        self.has_footnotes = false;
    }
}

/// `Page_spacing_node`: the best way found to put the lines up to one on
/// the pages up to one.
#[derive(Clone, Copy, Debug)]
struct PageSpacingNode {
    demerits: f64,
    force: f64,
    penalty: f64,
    prev: usize,
    page: i64,
    system_count_status: i32,
}

impl Default for PageSpacingNode {
    fn default() -> PageSpacingNode {
        PageSpacingNode {
            demerits: f64::INFINITY,
            force: f64::INFINITY,
            penalty: f64::INFINITY,
            prev: VPOS,
            page: 0,
            system_count_status: SYSTEM_COUNT_OK,
        }
    }
}

/// `Page_spacer`: the lines put on pages by a dynamic programme like
/// constrained breaking's: on a given number of pages over `state`
/// (lines × pages), or on any number over `simple_state`.
pub struct PageSpacer {
    first_page_num: i64,
    lines: Vec<LineDetails>,
    state: Matrix<PageSpacingNode>,
    simple_state: Vec<PageSpacingNode>,
    max_page_count: usize,
    ragged: bool,
    ragged_last: bool,
}

impl PageSpacer {
    /// `Page_spacer (lines, first_page_num, breaker)`: the breaker's
    /// `ragged-bottom`, and `ragged-last-bottom` if its current breaks end
    /// the book.
    pub fn new(lines: &[LineDetails], first_page_num: i64, pb: &PageBreaking) -> PageSpacer {
        PageSpacer {
            first_page_num,
            lines: lines.to_vec(),
            state: Matrix::default(),
            simple_state: Vec::new(),
            max_page_count: 0,
            ragged: pb.ragged,
            ragged_last: pb.is_last() && pb.ragged_last,
        }
    }

    /// `solve ()`: the lines on their best number of pages; no pages for
    /// no lines.
    pub fn solve_best(
        &mut self,
        host: &mut dyn Host,
        pb: &mut PageBreaking,
    ) -> Result<PageSpacingResult, Exception> {
        if self.simple_state.is_empty() {
            self.simple_state
                .resize(self.lines.len(), PageSpacingNode::default());
            for i in 0..self.lines.len() {
                self.calc_subproblem(host, pb, VPOS, i)?;
            }
        }

        let mut ret = PageSpacingResult::default();
        let (Some(last_node), Some(last_line)) = (self.simple_state.last(), self.lines.last())
        else {
            return Ok(ret);
        };

        ret.penalty = last_node.penalty + last_line.page_penalty + last_line.turn_penalty;
        ret.system_count_status = last_node.system_count_status;

        let mut system = self.lines.len() - 1;
        while system != VPOS {
            let cur = self.simple_state[system];
            let system_count = if cur.prev == VPOS {
                system + 1
            } else {
                system - cur.prev
            };

            ret.force.push(cur.force);
            ret.systems_per_page.push(system_count);
            ret.demerits += cur.force * cur.force;
            system = cur.prev;
        }

        ret.force.reverse();
        ret.systems_per_page.reverse();
        Ok(ret)
    }

    /// `solve (page_count)`: the lines on `page_count` pages, warned of
    /// when that cannot be done well: then as many lines as fit properly
    /// on them and the rest on the last page, or as many pages as can be
    /// filled and the rest blank, each such page's force
    /// [`BAD_SPACING_PENALTY`]; no pages if nothing can be salvaged.
    pub fn solve(
        &mut self,
        host: &mut dyn Host,
        pb: &mut PageBreaking,
        mut page_count: usize,
    ) -> Result<PageSpacingResult, Exception> {
        if page_count == 0 || self.lines.is_empty() {
            return Ok(PageSpacingResult::default());
        }
        if page_count > self.max_page_count {
            self.resize(host, pb, page_count)?;
        }

        let mut ret = PageSpacingResult::default();

        let mut system = self.lines.len() - 1;
        let mut extra_systems = 0;
        let mut extra_pages = 0;

        if self.state.at(system, page_count - 1).demerits.is_infinite() {
            crate::grob::unlocated_warning(
                host,
                "tried to space systems on a bad number of pages",
            )?;
            // Usually, this means that we tried to cram too many systems
            // into too few pages (can happen if the requested page-count
            // is unreasonable). To avoid crashing, we look for the largest
            // number of systems that we can fit properly onto the right
            // number of pages. All the systems that don't fit get tacked
            // onto the last page.
            let mut i = system;
            while self.state.at(i, page_count - 1).demerits.is_infinite() && i != 0 {
                i -= 1;
            }

            if i != 0 {
                extra_systems = system - i;
                system = i;
            } else {
                // Try chopping off pages from the end.
                let mut j = page_count;
                while j != 0 && self.state.at(system, j - 1).demerits.is_infinite() {
                    j -= 1;
                }

                if j != 0 {
                    extra_pages = page_count - j;
                    page_count = j;
                } else {
                    // Couldn't salvage it.
                    return Ok(PageSpacingResult::default());
                }
            }
        }

        ret.force.resize(page_count, 0.0);
        ret.systems_per_page.resize(page_count, 0);
        let last = self.state.at(system, page_count - 1);
        ret.system_count_status = last.system_count_status;
        let last_line = &self.lines[self.lines.len() - 1];
        ret.penalty = last.penalty + last_line.page_penalty + last_line.turn_penalty;

        ret.demerits = 0.0;
        for p in (0..page_count).rev() {
            let ps = *self.state.at(system, p);
            ret.force[p] = ps.force;
            ret.demerits += ps.force * ps.force;
            if p == 0 {
                ret.systems_per_page[p] = system + 1;
            } else {
                ret.systems_per_page[p] = system.wrapping_sub(ps.prev);
            }
            system = ps.prev;
        }

        if extra_systems != 0 {
            if let Some(n) = ret.systems_per_page.last_mut() {
                *n += extra_systems;
            }
            if let Some(f) = ret.force.last_mut() {
                *f = BAD_SPACING_PENALTY;
            }
        }
        if extra_pages != 0 {
            ret.force
                .extend(core::iter::repeat_n(BAD_SPACING_PENALTY, extra_pages));
            ret.systems_per_page
                .extend(core::iter::repeat_n(0, extra_pages));
        }

        Ok(ret)
    }

    /// `resize (page_count)`: the programme solved for up to `page_count`
    /// pages.
    fn resize(
        &mut self,
        host: &mut dyn Host,
        pb: &mut PageBreaking,
        page_count: usize,
    ) -> Result<(), Exception> {
        if self.max_page_count >= page_count {
            return Ok(());
        }

        self.state
            .resize(self.lines.len(), page_count, PageSpacingNode::default());
        for page in self.max_page_count..page_count {
            for line in page..self.lines.len() {
                if !self.calc_subproblem(host, pb, page, line)? {
                    break;
                }
            }
        }

        self.max_page_count = page_count;
        Ok(())
    }

    /// `calc_subproblem (page, line)`: one step of the programme, the best
    /// way to put the lines up to `line` on `page` + 1 pages, given the
    /// best ways to put fewer lines on `page` pages; on any number of pages
    /// for `page` [`VPOS`] (`simple_state`), the pages' heights then
    /// found as the pages are counted. Whether a way was found.
    fn calc_subproblem(
        &mut self,
        host: &mut dyn Host,
        pb: &mut PageBreaking,
        page: usize,
        line: usize,
    ) -> Result<bool, Exception> {
        let last = line == self.lines.len() - 1;

        // If page == VPOS we don't know yet which page number we're working
        // on, so the page height is found again in the loop, and the loop
        // stops only where a page without margins or headers and footers
        // would be overfilled (input/regression/page-spacing-tall-headfoot.ly).
        let page_num = if page == VPOS { 0 } else { page };
        let paper_height = pb.paper_height;
        // `static_cast<int> (page)`: -1 for VPOS.
        let height = pb.page_height(host, self.first_page_num + i64::from(page as i32), last)?;
        let mut space = PageSpacing::new(host.heap(), height);
        let mut cur = if page == VPOS {
            self.simple_state[line]
        } else {
            *self.state.at(line, page)
        };
        let ragged = self.ragged || (self.ragged_last && last);
        let mut line_count = 0;

        let mut page_start = line + 1;
        while page_start > page_num {
            page_start -= 1;
            let mut prev = None;

            if page == VPOS {
                if page_start > 0 {
                    let p = self.simple_state[page_start - 1];
                    prev = Some(p);
                    let height = pb.page_height(host, p.page + 1, last)?;
                    space.resize(pb, height);
                } else {
                    let height = pb.page_height(host, self.first_page_num, last)?;
                    space.resize(pb, height);
                }
            } else if page > 0 {
                prev = Some(*self.state.at(page_start - 1, page - 1));
            }

            space.prepend_system(pb, &self.lines[page_start]);

            let overfull = space.rod_height > paper_height
                || (self.ragged && (space.rod_height + space.spring_len > paper_height));
            // Not considered if it is overfull unless it is the first with
            // this start point, or the one before had fewer lines than
            // min-systems-per-page.
            if !pb.too_few_lines(line_count) && page_start < line && overfull {
                break;
            }

            line_count += self.lines[page_start].compressed_nontitle_lines_count;
            if page > 0 || page_start == 0 {
                // A ragged last page's force is zero, so that it is left
                // half-empty rather than balanced.
                if line == self.lines.len() - 1 && ragged && last && space.force > 0.0 {
                    space.force = 0.0;
                }

                let mut demerits = space.force * space.force;

                // Clamped at BAD_SPACING_PENALTY even if the page is
                // overfull, so that TERRIBLE_SPACING_PENALTY takes
                // precedence over overfull pages.
                demerits = f64::min(demerits, BAD_SPACING_PENALTY);
                demerits += prev.map_or(0.0, |p| p.demerits);

                let mut penalty = pb.line_count_penalty(line_count);
                if page_start > 0 {
                    let before = &self.lines[page_start - 1];
                    penalty += before.page_penalty
                        + if page.is_multiple_of(2) {
                            before.turn_penalty
                        } else {
                            0.0
                        };
                }

                // Widow and orphan lines: the last line of a paragraph
                // first on the new page, or its first line last on the
                // page before.
                if page_start > 0
                    && page_start < self.lines.len()
                    && self.lines[page_start].last_markup_line
                {
                    penalty += pb.orphan_penalty as f64;
                }
                if page_start > 0
                    && page_start < self.lines.len()
                    && self.lines[page_start - 1].first_markup_line
                {
                    penalty += pb.orphan_penalty as f64;
                }

                demerits += penalty;
                if demerits < cur.demerits || page_start == line {
                    cur.demerits = demerits;
                    cur.force = space.force;
                    cur.penalty = penalty + prev.map_or(0.0, |p| p.penalty);
                    cur.system_count_status = pb.line_count_status(line_count)
                        | prev.map_or(0, |p| p.system_count_status);
                    cur.prev = page_start.wrapping_sub(1);
                    cur.page = prev.map_or(self.first_page_num, |p| p.page + 1);
                }
            }

            if page_start > 0 && is_force(host.heap(), self.lines[page_start - 1].page_permission) {
                break;
            }
        }
        if page == VPOS {
            self.simple_state[line] = cur;
        } else {
            *self.state.at_mut(line, page) = cur;
        }
        Ok(!cur.demerits.is_infinite())
    }
}

/// `scm_is_eq (permission, ly_symbol2scm ("force"))`.
pub fn is_force(heap: &mut Heap, permission: Value) -> bool {
    permission == heap.intern("force")
}
