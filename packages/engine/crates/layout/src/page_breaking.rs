//! `page-breaking.cc` (part 5's): what a page breaker asks of the line
//! breakers of a book's scores (its line side, MS3 step 61), and the
//! spacing of their lines on pages (its page side, MS3 step 62).
//!
//! The line side: `Page_breaking`'s
//! constructor (`page-breaking.cc:253–312`) with the paper's settings and
//! their accessors (`:318–392`), the book's system specs
//! (`create_system_list`, `:747–767`), the chunks and breaks a page
//! breaker may break between, each score's [`ConstrainedBreaking`] from
//! the columns that start them (`find_chunks_and_breaks`, `:779–873`,
//! `chunk_list`, `:875–893`, `next_system`, `:241–251`,
//! `line_breaker_args`, `:429–447`), the numbers of systems a chunk may
//! take (`min_system_count`, `max_system_count`,
//! `system_count_bounds`, `:895–949`), the configurations of a number of
//! systems among the chunks (`set_current_breakpoints`,
//! `set_to_ideal_line_configuration`, `line_divisions_rec`,
//! `current_configuration_count`, `current_configuration`,
//! `:951–1037`, `:1091–1138`, `:1756–1760`), their lines' details,
//! compressed where no page break is allowed and with each line's height
//! on the page (`cache_line_details`, `clear_line_details_cache`,
//! `calc_line_heights`, `compress_lines`, `uncompress_solution`,
//! `:154–238`, `:1044–1089`, `:1140–1185`), and the lines of a
//! configuration broken into the scores' systems (`break_into_pieces`,
//! `systems`, `breakpoint_property`, `is_last`, `ends_score`,
//! `last_break_position`, `:449–504`, `:543–554`, `:1762–1780`).
//!
//! The page side ([`PageBreaking::page_height`] and what follows it): the
//! line counts' penalties (`too_many_lines`, `too_few_lines`,
//! `line_count_penalty`, `line_count_status`, `:393–428`), the pages'
//! heights (`paper_height`, `page_height`, `:505–542`), the fewest pages
//! a configuration takes (`min_page_count`, `:1185–1281`), its lines on
//! a number of pages, on one more, on the best number, a number a page
//! or packed (`space_systems_on_n_pages`, `blank_page_penalty`,
//! `space_systems_on_n_or_one_more_pages`, `space_systems_on_best_pages`,
//! `space_systems_with_fixed_number_per_page`,
//! `pack_systems_on_least_pages`, `:1282–1546`), their demerits
//! (`finalize_spacing_result`, `:1548–1590`), one and two pages
//! (`space_systems_on_1_page`, `space_systems_on_2_pages`,
//! `:1592–1745`), `all_lines_stretched` (`:1747–1755`) and the least
//! whitespace at a page's top and bottom (`:1783–1824`), over the page
//! spacer (`page_spacing.rs`). `ly:optimal-breaking` drives both sides
//! (`optimal_page_breaking.rs`); `make_pages` draws its pages (`page.rs`).
//!
//! A `Page_breaking` holds its book's system specs, the Paper_scores and
//! probs of [`paper_book::get_system_specs`], which [`PageBreaking::new`]
//! roots until its caller unroots to its depth; their columns are held by
//! their systems.

use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

use lily_engine_music::{Host, Prob};
use lily_engine_scheme::{Exception, Heap, Tag, Value};

use crate::call_lily;
use crate::constrained_breaking::{ConstrainedBreaking, LineDetails, VPOS};
use crate::geometry::{Direction, Interval};
use crate::grob;
use crate::page_spacing::{
    BAD_SPACING_PENALTY, PageSpacer, PageSpacing, PageSpacingResult, SYSTEM_COUNT_OK,
    SYSTEM_COUNT_TOO_FEW, SYSTEM_COUNT_TOO_MANY, TERRIBLE_SPACING_PENALTY,
};
use crate::paper_book::{self, PaperBook};
use crate::paper_score;
use crate::simple_spacer::ColumnXPositions;
use crate::spacing_spec::read_spacing_spec;
use crate::system;

const DOWN: Direction = -1;
const UP: Direction = 1;

/// `System_spec`: a Paper_score, or a prob (a title or a markup), or
/// neither, the one spec of an empty book.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SystemSpec {
    pub pscore: Option<Value>,
    pub prob: Option<Value>,
}

/// `Break_position`: a place a page breaker may break at. Its system
/// spec ([`VPOS`] for the start of the book), the break of the spec's
/// line breaker it is at (counted from its starting columns), the column
/// it breaks at, whether it ends its score, and the fixed number of lines
/// from the start of its chunk, 0 for none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreakPosition {
    pub system_spec_index: usize,
    pub score_break: usize,
    pub col: Option<Value>,
    pub score_ender: bool,
    pub forced_line_count: usize,
}

impl BreakPosition {
    /// `Break_position (s, brk, g, end)`.
    pub fn new(s: usize, brk: usize, col: Option<Value>, end: bool) -> BreakPosition {
        BreakPosition {
            system_spec_index: s,
            score_break: brk,
            col,
            score_ender: end,
            forced_line_count: 0,
        }
    }

    /// `Break_position ()`: the start of the book.
    pub fn start() -> BreakPosition {
        BreakPosition::new(VPOS, VPOS, None, false)
    }

    /// `operator<`: lexicographic in the spec and the break, the start of
    /// the book first.
    pub fn lt(&self, other: &BreakPosition) -> bool {
        (self.system_spec_index == VPOS && other.system_spec_index != VPOS)
            || self.system_spec_index < other.system_spec_index
            || (self.system_spec_index == other.system_spec_index
                && self.score_break < other.score_break)
    }

    /// `operator<=`, as LilyPond writes it.
    pub fn le(&self, other: &BreakPosition) -> bool {
        self.system_spec_index == VPOS
            || (self.system_spec_index < other.system_spec_index && other.system_spec_index != VPOS)
            || (self.system_spec_index == other.system_spec_index
                && self.score_break <= other.score_break)
    }
}

/// `Line_division`: the number of systems of each chunk.
pub type LineDivision = Vec<usize>;

/// `Break_predicate`: whether a column is a break a page breaker may
/// stop at; none for no breaks but the book's end.
pub type BreakPredicate = fn(&mut dyn Host, Value) -> Result<bool, Exception>;

/// `Page_breaking`: a book's page-breaking problem, its line side.
pub struct PageBreaking {
    /// `book_`: the `Paper_book`.
    pub book: Value,
    pub ragged: bool,
    pub ragged_last: bool,
    pub systems_per_page: i64,
    pub max_systems_per_page: i64,
    pub min_systems_per_page: i64,
    pub orphan_penalty: i64,
    pub paper_height: f64,
    pub footnote_separator_stencil_height: f64,
    pub footnote_padding: f64,
    pub in_note_padding: f64,
    pub in_note_system_padding: f64,
    pub footnote_footer_padding: f64,
    pub footnote_number_raise: f64,
    pub system_specs: Vec<SystemSpec>,
    pub breaks: Vec<BreakPosition>,
    pub chunks: Vec<BreakPosition>,
    pub line_breaking: Vec<ConstrainedBreaking>,
    pub system_count: usize,
    pub current_configurations: Vec<LineDivision>,
    pub current_chunks: Vec<BreakPosition>,
    pub current_start_breakpoint: usize,
    pub current_end_breakpoint: usize,
    pub cached_configuration_index: usize,
    pub cached_line_details: Vec<LineDetails>,
    pub uncompressed_line_details: Vec<LineDetails>,
    /// `page_height_cache_` and `last_page_height_cache_`: the heights of
    /// pages by number, negative for none yet.
    pub page_height_cache: Vec<f64>,
    pub last_page_height_cache: Vec<f64>,
    /// The `minimum-distance` (−∞ for none) and `padding` (0) of the
    /// paper's `top-system-spacing`, `top-markup-spacing` and
    /// `last-bottom-spacing`, which `min_whitespace_at_top_of_page` and
    /// `min_whitespace_at_bottom_of_page` read.
    pub top_system_spacing: (f64, f64),
    pub top_markup_spacing: (f64, f64),
    pub last_bottom_spacing: (f64, f64),
    /// The symbol `force`, a forced break's permission.
    pub force_symbol: Value,
    /// The Paper_scores [`break_into_pieces`](Self::break_into_pieces) has
    /// broken, which a later call breaks on (the page-turn breaker's, one
    /// call a turn).
    pub broken: Vec<Value>,
}

/// [`PageBreaking::substitute_on`] under way (H3, D254): the system spec it
/// is at.
#[derive(Default)]
pub struct Substitutions {
    spec: usize,
    substituting: system::Substituting,
}

/// [`PageBreaking::find_on`] under way (H3, D254): the next system spec, and
/// its line breaker with what it has left to fill.
#[derive(Default)]
pub struct Finding {
    begun: bool,
    i: usize,
    filling: Option<(
        ConstrainedBreaking,
        Option<crate::constrained_breaking::Filling>,
    )>,
}

impl PageBreaking {
    /// `Page_breaking (pb, is_break, prob_break)`: the paper's
    /// `paper-height` (1 if no number), `ragged-bottom` and
    /// `ragged-last-bottom` (`#t` alone true), `systems-per-page`,
    /// `max-systems-per-page` and `min-systems-per-page` (exact integers,
    /// not below 0; 0 otherwise), `orphan-penalty` (100000 if no exact
    /// integer), the footnote separator's height and the footnotes'
    /// paddings and raise (0 if no number); `systems-per-page` beside
    /// either bound, or a minimum above the maximum, warned of and both
    /// bounds dropped; then the book's system specs and their chunks and
    /// breaks, the columns `is_break` takes and the probs `prob_break`
    /// takes among them (only the page-turn breaker passes either).
    pub fn new(
        host: &mut dyn Host,
        book: Value,
        is_break: Option<BreakPredicate>,
        prob_break: Option<BreakPredicate>,
    ) -> Result<PageBreaking, Exception> {
        let mut pb = PageBreaking::begin(host, book)?;
        let mut finding = Finding::default();
        while !pb.find_on(host, is_break, prob_break, &mut finding)? {}
        Ok(pb)
    }

    /// [`PageBreaking::new`] as far as the system specs, its chunks and
    /// breaks left to [`find_on`](Self::find_on) (H3, D254).
    pub fn begin(host: &mut dyn Host, book: Value) -> Result<PageBreaking, Exception> {
        let paper = book_paper(host.heap(), book);
        let mut pb = PageBreaking::for_pages(host, book);
        pb.paper_height = number_or(host, paper, "paper-height", 1.0);
        pb.systems_per_page = int_or(host, paper, "systems-per-page", 0).max(0);
        pb.max_systems_per_page = int_or(host, paper, "max-systems-per-page", 0).max(0);
        pb.min_systems_per_page = int_or(host, paper, "min-systems-per-page", 0).max(0);
        pb.orphan_penalty = int_or(host, paper, "orphan-penalty", 100000);

        let footnote_separator = call_lily(host, "%footnote-separator-stencil", &[paper])?;
        if call_lily(host, "ly:stencil-empty?", &[footnote_separator])? == Value::FALSE {
            let y = Value::fixnum(1).unwrap_or(Value::FALSE);
            let extent = call_lily(host, "ly:stencil-extent", &[footnote_separator, y])?;
            pb.footnote_separator_stencil_height = interval_of(host.heap(), extent).length();
        }

        pb.footnote_padding = number_or(host, paper, "footnote-padding", 0.0);
        pb.in_note_padding = number_or(host, paper, "in-note-padding", 0.0);
        pb.in_note_system_padding = number_or(host, paper, "in-note-system-padding", 0.0);
        pb.footnote_footer_padding = number_or(host, paper, "footnote-footer-padding", 0.0);
        pb.footnote_number_raise = number_or(host, paper, "footnote-number-raise", 0.0);
        pb.top_system_spacing = spacing_of(host, paper, "top-system-spacing")?;
        pb.top_markup_spacing = spacing_of(host, paper, "top-markup-spacing")?;
        pb.last_bottom_spacing = spacing_of(host, paper, "last-bottom-spacing")?;

        if pb.systems_per_page != 0
            && (pb.max_systems_per_page != 0 || pb.min_systems_per_page != 0)
        {
            grob::unlocated_warning(
                host,
                "ignoring min-systems-per-page and max-systems-per-page because systems-per-page was set",
            )?;
            pb.min_systems_per_page = 0;
            pb.max_systems_per_page = 0;
        }
        if pb.max_systems_per_page != 0 && pb.min_systems_per_page > pb.max_systems_per_page {
            grob::unlocated_warning(
                host,
                "min-systems-per-page is larger than max-systems-per-page, ignoring both values",
            )?;
            pb.min_systems_per_page = 0;
            pb.max_systems_per_page = 0;
        }

        pb.create_system_list(host)?;
        Ok(pb)
    }

    /// The `Page_breaking` of the `Paper_book` `book` before the paper's
    /// other settings and its system specs are read ([`PageBreaking::new`]):
    /// its book and the paper's `ragged-bottom` and `ragged-last-bottom`,
    /// what `draw_page` and `make_pages` read (`page.rs`).
    pub fn for_pages(host: &mut dyn Host, book: Value) -> PageBreaking {
        let paper = book_paper(host.heap(), book);
        let ragged = paper_score::c_variable(host, paper, "ragged-bottom") == Value::TRUE;
        let ragged_last = paper_score::c_variable(host, paper, "ragged-last-bottom") == Value::TRUE;
        PageBreaking {
            book,
            ragged,
            ragged_last,
            systems_per_page: 0,
            max_systems_per_page: 0,
            min_systems_per_page: 0,
            orphan_penalty: 100000,
            paper_height: 1.0,
            footnote_separator_stencil_height: 0.0,
            footnote_padding: 0.0,
            in_note_padding: 0.0,
            in_note_system_padding: 0.0,
            footnote_footer_padding: 0.0,
            footnote_number_raise: 0.0,
            system_specs: Vec::new(),
            breaks: Vec::new(),
            chunks: Vec::new(),
            line_breaking: Vec::new(),
            system_count: 0,
            current_configurations: Vec::new(),
            current_chunks: Vec::new(),
            current_start_breakpoint: 0,
            current_end_breakpoint: 0,
            cached_configuration_index: VPOS,
            cached_line_details: Vec::new(),
            uncompressed_line_details: Vec::new(),
            page_height_cache: Vec::new(),
            last_page_height_cache: Vec::new(),
            top_system_spacing: (f64::NEG_INFINITY, 0.0),
            top_markup_spacing: (f64::NEG_INFINITY, 0.0),
            last_bottom_spacing: (f64::NEG_INFINITY, 0.0),
            force_symbol: host.heap().intern("force"),
            broken: Vec::new(),
        }
    }

    /// `max_systems_per_page`: `systems-per-page` if set, else the
    /// maximum.
    pub fn max_systems_per_page(&self) -> i64 {
        if self.systems_per_page != 0 {
            return self.systems_per_page;
        }
        self.max_systems_per_page
    }

    /// `min_systems_per_page`: `systems-per-page` if set, else the
    /// minimum.
    pub fn min_systems_per_page(&self) -> i64 {
        if self.systems_per_page != 0 {
            return self.systems_per_page;
        }
        self.min_systems_per_page
    }

    /// `next_system`: the system spec that starts the page after the
    /// break `break_pos`: the first at the start of the book, the break's
    /// own if it is in a score it does not end, else the next.
    pub fn next_system(&self, break_pos: &BreakPosition) -> usize {
        let sys = break_pos.system_spec_index;

        if sys == VPOS {
            // Beginning of the book.
            return 0;
        }
        if self.system_specs[sys].pscore.is_some() && !break_pos.score_ender {
            // The score overflows the previous page.
            return sys;
        }
        // This page starts with a new System_spec.
        sys + 1
    }

    /// `line_breaker_args (sys, start, end)`: the starting columns of the
    /// line breaker of the score `sys` to break from `start` to `end`:
    /// `start`'s break if it is in the score, else its first; `end`'s if
    /// it is, else [`VPOS`], its end.
    fn line_breaker_args(
        &self,
        sys: usize,
        start: &BreakPosition,
        end: &BreakPosition,
    ) -> (usize, usize) {
        let line_breaker_start = if start.system_spec_index == sys {
            start.score_break
        } else {
            0
        };
        let line_breaker_end = if end.system_spec_index == sys {
            end.score_break
        } else {
            VPOS
        };
        (line_breaker_start, line_breaker_end)
    }

    /// `break_into_pieces (start_break, end_break, div)`: each score
    /// between the two breaks broken into the systems of its chunks'
    /// lines, `div`'s number of them each (`solve`), or their best number
    /// (`best_solution`) when `div` is not one for these chunks, LilyPond's
    /// programming error. A score is broken by this page breaker only (its
    /// Paper_score's `broken`, which `Paper_score::get_paper_systems` then
    /// keeps from breaking it again), each of its chunks in turn, a later
    /// call's systems after an earlier's; where a test gave the breaks
    /// (D217), its lines are those between them, at its first chunk, in
    /// place of its chunks'.
    pub fn break_into_pieces(
        &mut self,
        host: &mut dyn Host,
        start_break: usize,
        end_break: usize,
        div: &[usize],
    ) -> Result<(), Exception> {
        let chunks = self.chunk_list(start_break, end_break);
        let mut ignore_div = false;
        if chunks.len() != div.len() + 1 {
            grob::unlocated_programming_error(
                host,
                "did not find a valid page breaking configuration",
            )?;
            ignore_div = true;
        }

        for i in 0..chunks.len().saturating_sub(1) {
            let sys = self.next_system(&chunks[i]);
            let Some(pscore) = self.system_specs[sys].pscore else {
                continue;
            };
            let first = !self.broken.contains(&pscore);
            if first {
                match host.heap().foreign_mut::<paper_score::PaperScore>(pscore) {
                    Some(ps) if !ps.broken => ps.broken = true,
                    _ => continue,
                }
                self.broken.push(pscore);
            }
            let (start, end) = self.line_breaker_args(sys, &chunks[i], &chunks[i + 1]);
            let given = paper_score::given_breaks(host, &self.line_breaking[sys])?;
            let pos: Vec<ColumnXPositions> = if let Some(at) = given {
                if !first {
                    continue;
                }
                self.line_breaking[sys].given_lines(host, &at)?
            } else if ignore_div {
                self.line_breaking[sys].best_solution(host, start, end)?
            } else {
                self.line_breaking[sys].solve(host, start, end, div[i])?
            };
            if let Some(root) = paper_score::root_system(host.heap(), pscore) {
                system::break_into_pieces(host, root, &pos)?;
            }
        }
        Ok(())
    }

    /// `systems`: the book's systems in order, a list: each score's broken
    /// systems, its grobs substituted first
    /// ([`system::do_break_substitution_and_fixup_refpoints`]), and each
    /// prob as it is.
    pub fn systems(&self, host: &mut dyn Host) -> Result<Value, Exception> {
        let mut substituting = Substitutions::default();
        while !self.substitute_on(host, &mut substituting)? {}
        Ok(self.lines(host))
    }

    /// The substitution of [`systems`](Self::systems), which goes on from
    /// `s`: whether it is done, false when the step's time is spent (H3).
    pub fn substitute_on(
        &self,
        host: &mut dyn Host,
        s: &mut Substitutions,
    ) -> Result<bool, Exception> {
        while let Some(spec) = self.system_specs.get(s.spec) {
            let root = spec
                .pscore
                .and_then(|p| paper_score::root_system(host.heap(), p).map(|root| (p, root)));
            let Some((pscore, root)) = root else {
                s.spec += 1;
                continue;
            };
            if !s.substituting.on(host, root)? {
                return Ok(false);
            }
            paper_score::substituted(host, pscore);
            s.spec += 1;
            s.substituting = system::Substituting::default();
        }
        Ok(true)
    }

    /// The book's systems in order, each score's substituted.
    pub fn lines(&self, host: &mut dyn Host) -> Value {
        let mut ret = Vec::new();
        for spec in &self.system_specs {
            if let Some(pscore) = spec.pscore {
                let Some(root) = paper_score::root_system(host.heap(), pscore) else {
                    continue;
                };
                let lines = system::get_broken_system_grobs(host.heap(), root);
                let mut s = lines;
                while let (Some(l), Some(rest)) = (host.heap().car(s), host.heap().cdr(s)) {
                    ret.push(l);
                    s = rest;
                }
            } else if let Some(prob) = spec.prob {
                ret.push(prob);
            }
        }
        host.heap().list(&ret)
    }

    /// `breakpoint_property (breakpoint, str)`: the property `name` of the
    /// break's column, or of its prob; `'()` at the start of the book.
    pub fn breakpoint_property(
        &self,
        host: &mut dyn Host,
        breakpoint: usize,
        name: &str,
    ) -> Result<Value, Exception> {
        let pos = self.breaks[breakpoint];

        if pos.system_spec_index == VPOS {
            return Ok(Value::NULL);
        }
        let spec = self.system_specs[pos.system_spec_index];
        if spec.pscore.is_some() {
            return match pos.col {
                Some(col) => grob::property(host, col, name),
                None => Ok(Value::NULL),
            };
        }
        let prob = spec.prob.unwrap_or(Value::FALSE);
        let sym = host.heap().intern(name);
        Ok(lily_engine_music::prob_get(host.heap(), prob, sym))
    }

    /// `create_system_list`: the book's system specs
    /// ([`paper_book::get_system_specs`]), each a Paper_score or a prob,
    /// rooted; one of neither for none.
    fn create_system_list(&mut self, host: &mut dyn Host) -> Result<(), Exception> {
        let specs = paper_book::get_system_specs(host, self.book)?;
        host.heap().root(specs);
        let mut s = specs;
        while let (Some(spec), Some(rest)) = (host.heap().car(s), host.heap().cdr(s)) {
            if host
                .heap()
                .foreign::<paper_score::PaperScore>(spec)
                .is_some()
            {
                self.system_specs.push(SystemSpec {
                    pscore: Some(spec),
                    prob: None,
                });
            } else if host.heap().foreign::<Prob>(spec).is_some() {
                self.system_specs.push(SystemSpec {
                    pscore: None,
                    prob: Some(spec),
                });
            }
            s = rest;
        }
        if self.system_specs.is_empty() {
            self.system_specs.push(SystemSpec::default());
        }
        Ok(())
    }

    /// `find_chunks_and_breaks (is_break, prob_is_break)`: the chunks,
    /// which end at each column whose page break is forced and at each
    /// score's end and each prob, and the breaks, the columns `is_break`
    /// takes (not a score's first), the probs `prob_is_break` takes and
    /// the book's end; each score's line
    /// breaker, starting at its first column and at each break or chunk
    /// end that is not its last. With the layout's `system-count`, the
    /// score's line configuration is fixed: only the columns its lines
    /// end at are considered, and each break and chunk end counts the
    /// lines since its chunk's start. Goes on from `finding`: whether it is
    /// done, false when the step's time is spent, between a line breaker's
    /// lines (H3, D254).
    pub fn find_on(
        &mut self,
        host: &mut dyn Host,
        is_break: Option<BreakPredicate>,
        prob_is_break: Option<BreakPredicate>,
        finding: &mut Finding,
    ) -> Result<bool, Exception> {
        let force_sym = host.heap().intern("force");

        if !finding.begun {
            finding.begun = true;
            self.chunks.push(BreakPosition::start());
            self.breaks.push(BreakPosition::start());
        }

        while finding.i < self.system_specs.len() {
            let i = finding.i;
            if let Some((breaking, filling)) = &mut finding.filling {
                let filled = match filling {
                    Some(filling) => breaking.fill_on(host, filling)?,
                    None => true,
                };
                if !filled {
                    return Ok(false);
                }
                if let Some((breaking, _)) = finding.filling.take() {
                    self.line_breaking.push(breaking);
                }
                finding.i += 1;
            } else if let Some(pscore) = self.system_specs[i].pscore {
                let cols = match paper_score::root_system(host.heap(), pscore) {
                    Some(root) => system::used_columns_in_range(host, root, 0, usize::MAX)?,
                    None => Vec::new(),
                };
                let mut forced_line_break_cols = Vec::new();

                let system_count = match paper_score::layout(host.heap(), pscore) {
                    Some(l) => paper_score::c_variable(host, l, "system-count"),
                    None => Value::UNDEFINED,
                };
                let counted = host.heap().num(system_count).is_some();
                if counted {
                    // With system-count given, the line configuration for
                    // this score is fixed. We need to ensure that chunk
                    // boundaries only occur at line breaks.
                    let count = to_vsize(host.heap(), system_count)?;
                    let mut breaking = ConstrainedBreaking::new(host, Some(pscore))?;
                    let details = breaking.line_details(host, 0, VPOS, count)?;
                    forced_line_break_cols.extend(details.iter().map(|d| d.last_column));
                }

                let mut last_forced_line_break_idx = 0;
                let mut forced_line_break_idx = 0;
                let mut line_breaker_columns = vec![0];

                for (j, &col) in cols.iter().enumerate() {
                    if !forced_line_break_cols.is_empty() {
                        if forced_line_break_idx >= forced_line_break_cols.len()
                            || forced_line_break_cols[forced_line_break_idx] != Some(col)
                        {
                            continue;
                        }
                        forced_line_break_idx += 1;
                    }

                    let last = j == cols.len() - 1;
                    let break_point = match is_break {
                        Some(is_break) if j > 0 => is_break(host, col)?,
                        _ => false,
                    };
                    let chunk_end =
                        grob::property(host, col, "page-break-permission")? == force_sym;
                    let mut cur_pos =
                        BreakPosition::new(i, line_breaker_columns.len(), Some(col), last);

                    // Even in the breaks list, forced_line_count refers
                    // to the number of lines between a Break_position and
                    // the start of that chunk, so that
                    // system_count_bounds can mix the two.
                    if counted {
                        cur_pos.forced_line_count =
                            forced_line_break_idx - last_forced_line_break_idx;
                    }

                    if break_point || (i == self.system_specs.len() - 1 && last) {
                        self.breaks.push(cur_pos);
                    }
                    if chunk_end || last {
                        self.chunks.push(cur_pos);
                        last_forced_line_break_idx = forced_line_break_idx;
                    }

                    if (break_point || chunk_end) && !last {
                        line_breaker_columns.push(j);
                    }
                }
                finding.filling = Some(ConstrainedBreaking::starting(
                    host,
                    Some(pscore),
                    &line_breaker_columns,
                )?);
            } else if let Some(prob) = self.system_specs[i].prob {
                let break_point = match prob_is_break {
                    Some(prob_is_break) => prob_is_break(host, prob)?,
                    None => false,
                };
                if break_point || i == self.system_specs.len() - 1 {
                    self.breaks.push(BreakPosition::new(i, VPOS, None, false));
                }

                self.chunks.push(BreakPosition::new(i, VPOS, None, false));

                self.line_breaking
                    .push(ConstrainedBreaking::new(host, None)?);
                finding.i += 1;
            } else {
                finding.i += 1;
            }
        }
        Ok(true)
    }

    /// `chunk_list (start_index, end_index)`: the break `start_index`, the
    /// chunks' ends after it and before the break `end_index`, and that
    /// break.
    pub fn chunk_list(&self, start_index: usize, end_index: usize) -> Vec<BreakPosition> {
        let start = self.breaks[start_index];
        let end = self.breaks[end_index];

        let mut i = 0;
        while i < self.chunks.len() && self.chunks[i].le(&start) {
            i += 1;
        }

        let mut ret = vec![start];
        while i < self.chunks.len() && self.chunks[i].lt(&end) {
            ret.push(self.chunks[i]);
            i += 1;
        }
        ret.push(end);
        ret
    }

    /// `min_system_count (start, end)`: the fewest non-title lines
    /// between the two breaks.
    pub fn min_system_count(
        &mut self,
        host: &mut dyn Host,
        start: usize,
        end: usize,
    ) -> Result<usize, Exception> {
        let chunks = self.chunk_list(start, end);
        Ok(self.system_count_bounds(host, &chunks, true)?.iter().sum())
    }

    /// `max_system_count (start, end)`: the most non-title lines between
    /// the two breaks.
    pub fn max_system_count(
        &mut self,
        host: &mut dyn Host,
        start: usize,
        end: usize,
    ) -> Result<usize, Exception> {
        let chunks = self.chunk_list(start, end);
        Ok(self.system_count_bounds(host, &chunks, false)?.iter().sum())
    }

    /// `system_count_bounds (chunks, min)`: the fewest (`min`) or most
    /// non-title lines of each chunk: its fixed count, or its score's line
    /// breaker's; 0 for a prob's.
    fn system_count_bounds(
        &mut self,
        host: &mut dyn Host,
        chunks: &[BreakPosition],
        min: bool,
    ) -> Result<LineDivision, Exception> {
        let mut ret = vec![0; chunks.len().saturating_sub(1)];

        for i in 0..ret.len() {
            let sys = self.next_system(&chunks[i]);

            if chunks[i + 1].forced_line_count != 0 {
                ret[i] = chunks[i + 1].forced_line_count;
            } else if self.system_specs[sys].pscore.is_some() {
                let (start, end) = self.line_breaker_args(sys, &chunks[i], &chunks[i + 1]);
                ret[i] = if min {
                    self.line_breaking[sys].min_system_count(host.heap(), start, end)
                } else {
                    self.line_breaking[sys].max_system_count(start, end)
                };
            }
        }

        Ok(ret)
    }

    /// `set_current_breakpoints (start, end, system_count, lower_bound,
    /// upper_bound)`: the configurations of `system_count` lines among the
    /// chunks between the two breaks, each chunk's count between its
    /// bounds (its fewest and most lines for none given); past 5 of them,
    /// the 5 whose lines have the least demerits (the squares of their
    /// forces and their breaks' penalties), libc++'s sort of the pairs.
    pub fn set_current_breakpoints(
        &mut self,
        host: &mut dyn Host,
        start: usize,
        end: usize,
        system_count: usize,
        mut lower_bound: LineDivision,
        mut upper_bound: LineDivision,
    ) -> Result<(), Exception> {
        self.system_count = system_count;
        self.current_chunks = self.chunk_list(start, end);
        self.current_start_breakpoint = start;
        self.current_end_breakpoint = end;
        self.clear_line_details_cache();

        let chunks = self.current_chunks.clone();
        if lower_bound.is_empty() {
            lower_bound = self.system_count_bounds(host, &chunks, true)?;
        }
        if upper_bound.is_empty() {
            upper_bound = self.system_count_bounds(host, &chunks, false)?;
        }

        let mut work_in_progress = Vec::new();
        self.current_configurations.clear();
        self.line_divisions_rec(
            system_count,
            &lower_bound,
            &upper_bound,
            &mut work_in_progress,
        );

        // We only consider a constant number of configurations. Otherwise,
        // this becomes slow when there are many small scores. The constant
        // 5 is somewhat arbitrary.
        if self.current_configurations.len() > 5 {
            let mut dems_and_indices = Vec::new();

            for i in 0..self.current_configurations.len() {
                self.cache_line_details(host, i)?;
                let mut dem = 0.0;
                for line in &self.cached_line_details {
                    dem += line.force * line.force + line.break_penalty;
                }
                dems_and_indices.push((dem, i));
            }
            lily_engine_skyline::sort(
                &mut dems_and_indices,
                &|a: &(f64, usize), b: &(f64, usize)| {
                    // std::pair's operator<.
                    a.0 < b.0 || (b.0.partial_cmp(&a.0) != Some(Ordering::Less) && a.1 < b.1)
                },
            );

            let best_5_configurations = dems_and_indices[..5]
                .iter()
                .map(|&(_, i)| self.current_configurations[i].clone())
                .collect();

            self.clear_line_details_cache();
            self.current_configurations = best_5_configurations;
        }
        Ok(())
    }

    /// `set_to_ideal_line_configuration (start, end)`: the one
    /// configuration of the chunks between the two breaks in which each
    /// takes the lines its line breaker likes best (`best_solution`), or
    /// its fixed count; a prob's none.
    pub fn set_to_ideal_line_configuration(
        &mut self,
        host: &mut dyn Host,
        start: usize,
        end: usize,
    ) -> Result<(), Exception> {
        self.current_chunks = self.chunk_list(start, end);
        self.current_start_breakpoint = start;
        self.current_end_breakpoint = end;
        self.clear_line_details_cache();
        self.system_count = 0;

        let mut div = Vec::new();
        for i in 0..self.current_chunks.len().saturating_sub(1) {
            let sys = self.next_system(&self.current_chunks[i]);

            if self.current_chunks[i + 1].forced_line_count != 0 {
                div.push(self.current_chunks[i + 1].forced_line_count);
            } else if self.system_specs[sys].pscore.is_some() {
                let (start, end) = self.line_breaker_args(
                    sys,
                    &self.current_chunks[i],
                    &self.current_chunks[i + 1],
                );
                div.push(
                    self.line_breaking[sys]
                        .best_solution(host, start, end)?
                        .len(),
                );
            } else {
                div.push(0);
            }

            self.system_count += div.last().copied().unwrap_or(0);
        }
        self.current_configurations.clear();
        self.current_configurations.push(div);
        Ok(())
    }

    /// `current_configuration (configuration_index)`.
    pub fn current_configuration(&self, configuration_index: usize) -> LineDivision {
        self.current_configurations[configuration_index].clone()
    }

    /// `cache_line_details (configuration_index)`: unless they are the
    /// configuration's already, the details of its lines, each score
    /// chunk's from its line breaker (`line_details`), a prob's of the
    /// prob and the book's paper ([`LineDetails::of_prob`], MS3 step 62)
    /// and the default ones of an empty book's spec, then compressed
    /// ([`compress_lines`]) and given their heights on the page
    /// ([`calc_line_heights`](Self::calc_line_heights)).
    pub fn cache_line_details(
        &mut self,
        host: &mut dyn Host,
        configuration_index: usize,
    ) -> Result<(), Exception> {
        if self.cached_configuration_index != configuration_index {
            self.cached_configuration_index = configuration_index;

            let div = self.current_configurations[configuration_index].clone();
            self.uncompressed_line_details.clear();
            let chunks = self.current_chunks.len().saturating_sub(1);
            for (i, &count) in div.iter().enumerate().take(chunks) {
                let sys = self.next_system(&self.current_chunks[i]);
                if self.system_specs[sys].pscore.is_some() {
                    let (start, end) = self.line_breaker_args(
                        sys,
                        &self.current_chunks[i],
                        &self.current_chunks[i + 1],
                    );
                    let details = self.line_breaking[sys].line_details(host, start, end, count)?;
                    self.uncompressed_line_details.extend(details);
                } else if let Some(prob) = self.system_specs[sys].prob {
                    let paper = book_paper(host.heap(), self.book);
                    let details = LineDetails::of_prob(host, prob, paper)?;
                    self.uncompressed_line_details.push(details);
                } else {
                    self.uncompressed_line_details
                        .push(LineDetails::new(host.heap()));
                }
            }
            self.cached_line_details = compress_lines(&self.uncompressed_line_details);
            self.calc_line_heights();
        }
        Ok(())
    }

    /// `clear_line_details_cache`.
    fn clear_line_details_cache(&mut self) {
        self.cached_configuration_index = VPOS;
        self.cached_line_details.clear();
        self.uncompressed_line_details.clear();
    }

    /// `line_divisions_rec (system_count, min_sys, max_sys, cur_division)`:
    /// each way to give the chunks from `cur_division`'s on
    /// `system_count` lines, each chunk's between its bounds, after
    /// `cur_division`; none for an unsolvable problem.
    fn line_divisions_rec(
        &mut self,
        system_count: usize,
        min_sys: &[usize],
        max_sys: &[usize],
        cur_division: &mut LineDivision,
    ) {
        let my_index = cur_division.len();
        let mut others_min = 0;
        let mut others_max = 0;

        for i in my_index + 1..min_sys.len() {
            others_min += min_sys[i];
            others_max += max_sys[i];
        }
        others_max = others_max.min(system_count);
        let real_min = min_sys[my_index].max(system_count - others_max);

        if system_count < others_min {
            // This should never happen within a recursive call; at all, it
            // means an unsolvable problem, with no result.
            return;
        }

        let real_max = max_sys[my_index].min(system_count - others_min);

        if real_min > real_max {
            // Likewise.
            return;
        }

        for i in real_min..=real_max {
            cur_division.push(i);
            if my_index == min_sys.len() - 1 {
                self.current_configurations.push(cur_division.clone());
            } else {
                self.line_divisions_rec(system_count - i, min_sys, max_sys, cur_division);
            }
            cur_division.pop();
        }
    }

    /// `calc_line_heights`: each cached line's `tallness`, how far its
    /// bottom hangs below the line before's: its top placed under the
    /// bottom of the line before by its shape's two parts, then by the
    /// line before's padding (a title's after a title), unless it is
    /// tightly spaced, and no nearer to the line before's lowest
    /// refpoint than the minimum distance between their refpoints.
    fn calc_line_heights(&mut self) {
        let mut prev_hanging = 0.0;
        let mut prev_hanging_begin = 0.0;
        let mut prev_hanging_rest = 0.0;

        // refpoint_hanging is the y coordinate of the origin of this
        // system. It may not be the same as refpoint_extent[UP], which is
        // the refpoint of the first spaceable staff in this system.
        let mut prev_refpoint_hanging = 0.0;
        for i in 0..self.cached_line_details.len() {
            let cur = &self.cached_line_details[i];
            let shape = cur.shape;
            let a = shape.begin.at(UP);
            let b = shape.rest.at(UP);
            let title = cur.title;
            let mut refpoint_hanging = f64::max(prev_hanging_begin + a, prev_hanging_rest + b);

            if i > 0 {
                let prev = &self.cached_line_details[i - 1];
                let padding = if cur.tight_spacing {
                    0.0
                } else if title {
                    prev.title_padding
                } else {
                    prev.padding
                };
                let min_dist = if title {
                    prev.title_min_distance
                } else {
                    prev.min_distance
                };
                refpoint_hanging = f64::max(
                    refpoint_hanging + padding,
                    prev_refpoint_hanging - prev.refpoint_extent.at(DOWN)
                        + cur.refpoint_extent.at(UP)
                        + min_dist,
                );
            }

            let hanging_begin = refpoint_hanging - shape.begin.at(DOWN);
            let hanging_rest = refpoint_hanging - shape.rest.at(DOWN);
            let hanging = f64::max(hanging_begin, hanging_rest);
            self.cached_line_details[i].tallness = hanging - prev_hanging;
            prev_hanging = hanging;
            prev_hanging_begin = hanging_begin;
            prev_hanging_rest = hanging_rest;
            prev_refpoint_hanging = refpoint_hanging;
        }
    }

    /// `is_last`: whether the current breaks end at the book's end.
    pub fn is_last(&self) -> bool {
        self.current_end_breakpoint == self.last_break_position()
    }

    /// `ends_score`: whether the current end break ends its score.
    pub fn ends_score(&self) -> bool {
        self.breaks[self.current_end_breakpoint].score_ender
    }

    /// `last_break_position`: the book's end, its last break.
    pub fn last_break_position(&self) -> usize {
        self.breaks.len() - 1
    }
}

/// The page side (MS3 step 62): the lines of a configuration spaced on
/// pages (`page-breaking.cc:393–428`, `:505–542`, `:1185–1755`,
/// `:1783–1829`).
impl PageBreaking {
    /// `too_many_lines (line_count)`: more than `max-systems-per-page`, if
    /// set.
    pub fn too_many_lines(&self, line_count: i32) -> bool {
        self.max_systems_per_page() > 0 && i64::from(line_count) > self.max_systems_per_page()
    }

    /// `too_few_lines (line_count)`: fewer than `min-systems-per-page`.
    pub fn too_few_lines(&self, line_count: i32) -> bool {
        i64::from(line_count) < self.min_systems_per_page()
    }

    /// `line_count_penalty (line_count)`: [`TERRIBLE_SPACING_PENALTY`] for
    /// each line too many or too few on a page.
    pub fn line_count_penalty(&self, line_count: i32) -> f64 {
        let line_count = i64::from(line_count);
        if self.too_many_lines(line_count as i32) {
            return (line_count - self.max_systems_per_page()) as f64 * TERRIBLE_SPACING_PENALTY;
        }
        if self.too_few_lines(line_count as i32) {
            return (self.min_systems_per_page() - line_count) as f64 * TERRIBLE_SPACING_PENALTY;
        }
        0.0
    }

    /// `line_count_status (line_count)`.
    pub fn line_count_status(&self, line_count: i32) -> i32 {
        if self.too_many_lines(line_count) {
            return SYSTEM_COUNT_TOO_MANY;
        }
        if self.too_few_lines(line_count) {
            return SYSTEM_COUNT_TOO_FEW;
        }
        SYSTEM_COUNT_OK
    }

    /// `page_height (page_num, last)`: the printable height of the page
    /// numbered `page_num`, the book part's last if `last`, as page.scm's
    /// `calc-printable-height` finds it of a page made for it
    /// (`make-page`); kept by number for a non-negative one, apart for
    /// last pages.
    pub fn page_height(
        &mut self,
        host: &mut dyn Host,
        page_num: i64,
        last: bool,
    ) -> Result<f64, Exception> {
        let cache = if last {
            &self.last_page_height_cache
        } else {
            &self.page_height_cache
        };
        if let Ok(n) = usize::try_from(page_num)
            && let Some(&h) = cache.get(n)
            && h >= 0.0
        {
            return Ok(h);
        }
        let depth = host.heap().root_depth();
        let number = host.heap().integer(page_num);
        let page = call_lily(host, "%make-page", &[self.book, number, Value::bool(last)]);
        let height = page.and_then(|page| {
            host.heap().root(page);
            call_lily(host, "%calc-printable-height", &[page])
        });
        host.heap().unroot_to(depth);
        let height = height?;
        let height = match host.heap().to_f64(height) {
            Some(h) => h,
            None => return Err(crate::wrong_type(host.heap(), None, 0, height, "real")),
        };

        if let Ok(n) = usize::try_from(page_num) {
            let cache = if last {
                &mut self.last_page_height_cache
            } else {
                &mut self.page_height_cache
            };
            if cache.len() <= n {
                cache.resize(n + 1, -1.0);
            }
            cache[n] = height;
        }
        Ok(height)
    }

    /// `current_configuration_count`.
    pub fn current_configuration_count(&self) -> usize {
        self.current_configurations.len()
    }

    /// `min_page_count (configuration, first_page_num)`: the fewest pages
    /// the configuration's lines fill, each page taking lines while they
    /// fit (their rods, and their springs on a ragged page, and the least
    /// whitespace at its top and bottom), up to `max-systems-per-page` and
    /// to a forced page break; one more if the last page, now known to be
    /// last, does not hold its lines and has more than one.
    pub fn min_page_count(
        &mut self,
        host: &mut dyn Host,
        configuration: usize,
        first_page_num: i64,
    ) -> Result<usize, Exception> {
        let mut ret = 1;
        let mut page_starter = 0;
        let mut cur_rod_height = 0.0;
        let mut cur_spring_height = 0.0;
        let mut cur_page_height = self.page_height(host, first_page_num, false)?;
        let mut line_count = 0;

        self.cache_line_details(host, configuration)?;

        if let Some(first) = self.cached_line_details.first() {
            cur_page_height -= self.min_whitespace_at_top_of_page(first);
        }

        for i in 0..self.cached_line_details.len() {
            let cur = &self.cached_line_details[i];
            let prev = i.checked_sub(1).map(|p| &self.cached_line_details[p]);
            let ext_len = if cur_rod_height > 0.0 {
                cur.tallness
            } else {
                cur.full_height()
            };

            let spring_len = prev.map_or(0.0, |p| p.spring_length(cur));
            let next_rod_height = cur_rod_height + ext_len;
            let next_spring_height = cur_spring_height + spring_len;
            let next_height = next_rod_height
                + if self.ragged { next_spring_height } else { 0.0 }
                + self.min_whitespace_at_bottom_of_page(cur);
            let next_line_count = line_count + cur.compressed_nontitle_lines_count;
            let forced = prev.is_some_and(|p| p.page_permission == self.force_symbol);

            if (!self.too_few_lines(line_count)
                && (next_height > cur_page_height && cur_rod_height > 0.0))
                || self.too_many_lines(next_line_count)
                || forced
            {
                line_count = cur.compressed_nontitle_lines_count;
                cur_rod_height = cur.full_height();
                cur_spring_height = 0.0;
                page_starter = i;
                let top = self.min_whitespace_at_top_of_page(cur);

                cur_page_height = self.page_height(host, first_page_num + ret as i64, false)?;
                cur_page_height -= top;

                ret += 1;
            } else {
                cur_rod_height = next_rod_height;
                cur_spring_height = next_spring_height;
                line_count = next_line_count;
            }
        }

        // Two potential problems with the last page, which was not known to
        // be the last until all the systems were fitted to it: ragged-last
        // with a compressed spring, or a last page shorter than the others
        // that the lines do not fit. Either way one more page will do: the
        // last line fits on it, and the page before is then no last page.
        if self.is_last()
            && let Some(back) = self.cached_line_details.last()
        {
            let back_height = back.full_height();
            let back_lines = back.compressed_nontitle_lines_count;
            let bottom = self.min_whitespace_at_bottom_of_page(back);
            let top = self.min_whitespace_at_top_of_page(&self.cached_line_details[page_starter]);
            cur_page_height = self.page_height(host, first_page_num + ret as i64 - 1, true)?;
            cur_page_height -= top;
            cur_page_height -= bottom;

            if !self.too_few_lines(line_count - back_lines)
                && cur_rod_height > cur_page_height
                // Not for a last page of only one system.
                && cur_rod_height > back_height
            {
                ret += 1;
            }
        }

        Ok(ret)
    }

    /// `space_systems_on_n_pages (configuration, n, first_page_num)`: the
    /// configuration's lines on `n` pages, warned of if they cannot take
    /// so few or so many; with `systems-per-page`, its number on each page,
    /// penalized if that is not `n` pages.
    pub fn space_systems_on_n_pages(
        &mut self,
        host: &mut dyn Host,
        configuration: usize,
        n: usize,
        first_page_num: i64,
    ) -> Result<PageSpacingResult, Exception> {
        if self.systems_per_page > 0 {
            let mut ret =
                self.space_systems_with_fixed_number_per_page(host, configuration, first_page_num)?;
            ret.demerits += if ret.force.len() == n {
                0.0
            } else {
                BAD_SPACING_PENALTY
            };
            return Ok(ret);
        }

        self.cache_line_details(host, configuration)?;

        let min = self.min_page_count(host, configuration, first_page_num)?;
        let max = self.cached_line_details.len();

        let mut valid_n = true;

        if n < min {
            let text = format!("too few pages: {n} (should have at least {min})");
            grob::unlocated_warning(host, &text)?;
            valid_n = false;
        }
        if n > max {
            let text = format!("too many pages: {n} (should have at most {max})");
            grob::unlocated_warning(host, &text)?;
            valid_n = false;
        }

        let ret = if n == 1 && valid_n {
            let height = self.page_height(host, first_page_num, self.is_last())?;
            let rag = self.ragged || (self.is_last() && self.ragged_last);
            let lines = self.cached_line_details.clone();
            self.space_systems_on_1_page(host.heap(), &lines, height, rag)
        } else if n == 2 && valid_n {
            self.space_systems_on_2_pages(host, configuration, first_page_num)?
        } else {
            let mut ps = PageSpacer::new(&self.cached_line_details, first_page_num, self);
            ps.solve(host, self, n)?
        };

        self.finalize_spacing_result(host, configuration, ret)
    }

    /// `blank_page_penalty`: the `blank-last-page-penalty` at the book's
    /// end, the `blank-after-score-page-penalty` after a score, else the
    /// `blank-page-penalty`, of the layout of the score the current end
    /// break is in, or of the paper; 0 if no number.
    pub fn blank_page_penalty(&mut self, host: &mut dyn Host) -> f64 {
        let penalty_sym = if self.is_last() {
            "blank-last-page-penalty"
        } else if self.ends_score() {
            "blank-after-score-page-penalty"
        } else {
            "blank-page-penalty"
        };

        let pos = self.breaks[self.current_end_breakpoint];
        let def = self
            .system_specs
            .get(pos.system_spec_index)
            .and_then(|spec| spec.pscore)
            .and_then(|ps| paper_score::layout(host.heap(), ps))
            .unwrap_or_else(|| book_paper(host.heap(), self.book));
        let v = paper_score::c_variable(host, def, penalty_sym);
        host.heap().to_f64(v).unwrap_or(0.0)
    }

    /// `space_systems_on_n_or_one_more_pages (configuration, n,
    /// first_page_num, penalty_for_fewer_pages)`: the better of the
    /// configuration's lines on `n` pages, penalized by
    /// `penalty_for_fewer_pages` (weighted by `page-spacing-weight`, 10 if
    /// no number), and on `n` + 1; with `systems-per-page`, its number on
    /// each page, penalized if that is neither.
    pub fn space_systems_on_n_or_one_more_pages(
        &mut self,
        host: &mut dyn Host,
        configuration: usize,
        n: usize,
        first_page_num: i64,
        penalty_for_fewer_pages: f64,
    ) -> Result<PageSpacingResult, Exception> {
        let mut n_res = PageSpacingResult::default();
        let mut m_res = PageSpacingResult::default();

        if self.systems_per_page > 0 {
            let mut ret =
                self.space_systems_with_fixed_number_per_page(host, configuration, first_page_num)?;
            ret.demerits += if ret.force.len() == n || ret.force.len() == n.wrapping_sub(1) {
                0.0
            } else {
                BAD_SPACING_PENALTY
            };
            return Ok(ret);
        }

        self.cache_line_details(host, configuration)?;
        let min_p_count = self.min_page_count(host, configuration, first_page_num)?;
        let valid_n = n >= min_p_count || n <= self.cached_line_details.len();

        if !valid_n {
            grob::unlocated_programming_error(host, "both page counts are out of bounds")?;
        }

        if n == 1 && valid_n {
            let rag = self.ragged || (self.is_last() && self.ragged_last);
            let height = self.page_height(host, first_page_num, self.is_last())?;

            if 1 >= min_p_count {
                let lines = self.cached_line_details.clone();
                n_res = self.space_systems_on_1_page(host.heap(), &lines, height, rag);
            }
            if 1 < self.cached_line_details.len() {
                m_res = self.space_systems_on_2_pages(host, configuration, first_page_num)?;
            }
        } else {
            let mut ps = PageSpacer::new(&self.cached_line_details, first_page_num, self);

            if n >= min_p_count || !valid_n {
                n_res = ps.solve(host, self, n)?;
            }
            if n < self.cached_line_details.len() || !valid_n {
                m_res = ps.solve(host, self, n + 1)?;
            }
        }

        m_res = self.finalize_spacing_result(host, configuration, m_res)?;
        n_res = self.finalize_spacing_result(host, configuration, n_res)?;

        let paper = book_paper(host.heap(), self.book);
        let page_spacing_weight = number_or(host, paper, "page-spacing-weight", 10.0);
        n_res.demerits += penalty_for_fewer_pages * page_spacing_weight;

        if let Some(f) = n_res.force.last_mut() {
            *f += penalty_for_fewer_pages;
        }

        Ok(if m_res.demerits < n_res.demerits {
            m_res
        } else {
            n_res
        })
    }

    /// `space_systems_on_best_pages (configuration, first_page_num)`: the
    /// configuration's lines on their best number of pages
    /// ([`PageSpacer::solve_best`]); with `systems-per-page`, its number on
    /// each page.
    pub fn space_systems_on_best_pages(
        &mut self,
        host: &mut dyn Host,
        configuration: usize,
        first_page_num: i64,
    ) -> Result<PageSpacingResult, Exception> {
        if self.systems_per_page > 0 {
            return self.space_systems_with_fixed_number_per_page(
                host,
                configuration,
                first_page_num,
            );
        }

        self.cache_line_details(host, configuration)?;
        let mut ps = PageSpacer::new(&self.cached_line_details, first_page_num, self);
        let ret = ps.solve_best(host, self)?;
        self.finalize_spacing_result(host, configuration, ret)
    }

    /// `space_systems_with_fixed_number_per_page (configuration,
    /// first_page_num)`: `systems-per-page` lines on each page, fewer
    /// before a forced page break, each page that has not that number
    /// penalized by [`TERRIBLE_SPACING_PENALTY`] a line; the last page's
    /// force found again as the last page's. The pages are counted from
    /// the page after `first_page_num`, as LilyPond's.
    pub fn space_systems_with_fixed_number_per_page(
        &mut self,
        host: &mut dyn Host,
        configuration: usize,
        first_page_num: i64,
    ) -> Result<PageSpacingResult, Exception> {
        let mut res = PageSpacingResult::default();
        let height = self.page_height(host, first_page_num, false)?;
        let mut space = PageSpacing::new(host.heap(), height);
        let mut line = 0;
        let mut page_num = first_page_num;
        let mut page_first_line = 0;

        self.cache_line_details(host, configuration)?;
        while line < self.cached_line_details.len() {
            page_num += 1;
            space.clear();
            let height = self.page_height(host, page_num, false)?;
            space.resize(self, height);

            let mut system_count_on_this_page = 0;
            while i64::from(system_count_on_this_page) < self.systems_per_page
                && line < self.cached_line_details.len()
            {
                let cur_line = self.cached_line_details[line].clone();
                space.append_system(self, &cur_line);
                system_count_on_this_page += cur_line.compressed_nontitle_lines_count;
                line += 1;

                if cur_line.page_permission == self.force_symbol {
                    break;
                }
            }

            res.systems_per_page.push(line - page_first_line);

            res.force.push(space.force);
            res.penalty += self.cached_line_details[line - 1].page_penalty;
            let count = i64::from(system_count_on_this_page);
            if count != self.systems_per_page {
                res.penalty +=
                    (count - self.systems_per_page).abs() as f64 * TERRIBLE_SPACING_PENALTY;
                res.system_count_status |= if count < self.systems_per_page {
                    SYSTEM_COUNT_TOO_FEW
                } else {
                    SYSTEM_COUNT_TOO_MANY
                };
            }

            page_first_line = line;
        }

        // The last page's force again, now that it is known to be last.
        let height = self.page_height(host, page_num, true)?;
        space.resize(self, height);
        if let Some(f) = res.force.last_mut() {
            *f = space.force;
        }
        self.finalize_spacing_result(host, configuration, res)
    }

    /// `pack_systems_on_least_pages (configuration, first_page_num)`: as
    /// many lines on each page as fit, a page ending at a forced page
    /// break; the last line on a page of its own if the last page, now
    /// known to be last, does not hold it.
    pub fn pack_systems_on_least_pages(
        &mut self,
        host: &mut dyn Host,
        configuration: usize,
        first_page_num: i64,
    ) -> Result<PageSpacingResult, Exception> {
        let mut res = PageSpacingResult::default();
        let mut page_num = first_page_num;
        let mut page_first_line = 0;
        let height = self.page_height(host, page_num, false)?;
        let mut space = PageSpacing::new(host.heap(), height);

        self.cache_line_details(host, configuration)?;
        let count = self.cached_line_details.len();
        for line in 0..count {
            let prev_force = space.force;
            let cur = self.cached_line_details[line].clone();
            space.append_system(self, &cur);
            if line > page_first_line
                && (space.force.is_infinite()
                    || (line > 0
                        && self.cached_line_details[line - 1].page_permission == self.force_symbol))
            {
                res.systems_per_page.push(line - page_first_line);
                res.force.push(prev_force);
                res.penalty += self.cached_line_details[line - 1].page_penalty;
                page_num += 1;
                let height = self.page_height(host, page_num, false)?;
                space.resize(self, height);
                space.clear();
                space.append_system(self, &cur);
                page_first_line = line;
            }

            if line == count - 1 {
                // The last line. The last page's height was found before it
                // was known to be the last: if its systems no longer fit,
                // the last one goes on a new page.
                let height = self.page_height(host, page_num, true)?;
                space.resize(self, height);
                if line > page_first_line && space.force.is_infinite() {
                    res.systems_per_page.push(line - page_first_line);
                    res.force.push(prev_force);
                    // The last page, holding the last line.
                    let height = self.page_height(host, page_num + 1, true)?;
                    space.resize(self, height);
                    space.clear();
                    space.append_system(self, &cur);
                    res.systems_per_page.push(1);
                    res.force.push(space.force);
                    res.penalty += self.cached_line_details[line - 1].page_penalty;
                    res.penalty += self.cached_line_details[line].page_penalty;
                } else {
                    res.systems_per_page.push(line + 1 - page_first_line);
                    res.force.push(space.force);
                    res.penalty += self.cached_line_details[line].page_penalty;
                }
            }
        }
        self.finalize_spacing_result(host, configuration, res)
    }

    /// `finalize_spacing_result (configuration, res)`: `res`' lines per
    /// page counted in the uncompressed lines, and its demerits: the
    /// squares of the lines' forces, their break penalties, and the pages'
    /// penalty and the squares of their forces (each at most
    /// [`BAD_SPACING_PENALTY`]; on a ragged-bottom paper only the last
    /// page's, and not a ragged last page's) weighted by
    /// `page-spacing-weight` (10 if no number). A result of no pages as it
    /// is.
    pub fn finalize_spacing_result(
        &mut self,
        host: &mut dyn Host,
        configuration: usize,
        mut res: PageSpacingResult,
    ) -> Result<PageSpacingResult, Exception> {
        if res.force.is_empty() {
            return Ok(res);
        }

        self.cache_line_details(host, configuration)?;
        res.systems_per_page =
            uncompress_solution(&res.systems_per_page, &self.cached_line_details);

        let mut line_force = 0.0;
        let mut line_penalty = 0.0;
        let mut page_demerits = res.penalty;
        let paper = book_paper(host.heap(), self.book);
        let page_weighting = number_or(host, paper, "page-spacing-weight", 10.0);

        for line in &self.uncompressed_line_details {
            line_force += line.force * line.force;
            line_penalty += line.break_penalty;
        }

        let start = if self.ragged { res.force.len() - 1 } else { 0 };
        let end = res.force.len() - usize::from(self.is_last() && self.ragged_last);
        for &f in res.force.iter().take(end).skip(start) {
            page_demerits += f64::min(f * f, BAD_SPACING_PENALTY);
        }

        // Summed, not averaged across pages: a single page with a very bad
        // force (a forced page break) would otherwise be averaged out over
        // many pages.
        res.demerits = line_force + line_penalty + page_demerits * page_weighting;
        Ok(res)
    }

    /// `space_systems_on_1_page (lines, page_height, ragged)`: `lines` on
    /// one page of `page_height`, a ragged page's force no more than 0,
    /// penalized for its number of lines and by its last line's page and
    /// turn penalties; its demerits not found (an internal function's).
    pub fn space_systems_on_1_page(
        &self,
        heap: &mut Heap,
        lines: &[LineDetails],
        page_height: f64,
        ragged: bool,
    ) -> PageSpacingResult {
        let mut space = PageSpacing::new(heap, page_height);
        let mut ret = PageSpacingResult::default();
        let mut line_count = 0;

        for line in lines {
            space.append_system(self, line);
            line_count += line.compressed_nontitle_lines_count;
        }

        ret.systems_per_page.push(lines.len());
        ret.force.push(if ragged {
            f64::min(space.force, 0.0)
        } else {
            space.force
        });
        let back = lines.last();
        ret.penalty = self.line_count_penalty(line_count)
            + back.map_or(0.0, |l| l.page_penalty)
            + back.map_or(0.0, |l| l.turn_penalty);
        ret.system_count_status |= self.line_count_status(line_count);
        ret
    }

    /// `space_systems_on_2_pages (configuration, first_page_num)`: the
    /// configuration's lines on two pages, two one-page problems at a
    /// forced page break, else the break between them with the least
    /// demerits (the squares of the two pages' forces, their line counts'
    /// penalties and the page penalties at the break and at the end); a
    /// ragged first page that is compressed is not taken, unless it has
    /// one line. Its demerits not found (an internal function's).
    pub fn space_systems_on_2_pages(
        &mut self,
        host: &mut dyn Host,
        configuration: usize,
        first_page_num: i64,
    ) -> Result<PageSpacingResult, Exception> {
        let page1_height = self.page_height(host, first_page_num, false)?;
        let last = self.is_last();
        let page2_height = self.page_height(host, first_page_num + 1, last)?;
        let ragged1 = self.ragged;
        let ragged2 = self.ragged || (self.is_last() && self.ragged_last);

        // A forced break makes it two one-page problems.
        self.cache_line_details(host, configuration)?;
        let lines = self.cached_line_details.clone();
        for i in 0..lines.len().saturating_sub(1) {
            if lines[i].page_permission == self.force_symbol {
                let lines1 = &lines[..i + 1];
                let lines2 = &lines[i + 1..];
                let mut p1 =
                    self.space_systems_on_1_page(host.heap(), lines1, page1_height, ragged1);
                let p2 = self.space_systems_on_1_page(host.heap(), lines2, page2_height, ragged2);

                p1.systems_per_page.push(p2.systems_per_page[0]);
                p1.force.push(p2.force[0]);
                p1.penalty += p2.penalty - lines[i].turn_penalty;
                p1.system_count_status |= p2.system_count_status;
                return Ok(p1);
            }
        }

        let breaks = lines.len().saturating_sub(1);
        let mut page1_force = vec![f64::INFINITY; breaks];
        let mut page2_force = vec![f64::INFINITY; breaks];

        // The penalties and statuses of min-systems-per-page and
        // max-systems-per-page.
        let mut page1_penalty = vec![f64::INFINITY; breaks];
        let mut page2_penalty = vec![f64::INFINITY; breaks];
        let mut page1_status = vec![0; breaks];
        let mut page2_status = vec![0; breaks];

        let mut page1 = PageSpacing::new(host.heap(), page1_height);
        let mut page2 = PageSpacing::new(host.heap(), page2_height);
        let mut page1_line_count = 0;
        let mut page2_line_count = 0;

        // The page 1 and page 2 forces of each break.
        for i in 0..breaks {
            let back = &lines[lines.len() - 1 - i];
            page1.append_system(self, &lines[i]);
            page2.prepend_system(self, back);
            page1_line_count += lines[i].compressed_nontitle_lines_count;
            page2_line_count += back.compressed_nontitle_lines_count;

            page1_force[i] = if ragged1 && page1.force < 0.0 && i > 0 {
                f64::INFINITY
            } else {
                page1.force
            };
            page1_penalty[i] = self.line_count_penalty(page1_line_count);
            page1_status[i] = self.line_count_status(page1_line_count);

            let j = breaks - 1 - i;
            page2_force[j] = if ragged1 {
                if page2.force < 0.0 && i + 1 < breaks {
                    f64::INFINITY
                } else {
                    0.0
                }
            } else if ragged2 && page2.force > 0.0 {
                0.0
            } else {
                page2.force
            };
            page2_penalty[j] = self.line_count_penalty(page2_line_count);
            page2_status[j] = self.line_count_status(page2_line_count);
        }

        // The break with the least sum of the pages' forces; the line
        // counts' are soft constraints, harshly penalized.
        let back = &lines[lines.len() - 1];
        let mut best_sys_count = 1;
        let mut best_demerits = f64::INFINITY;
        for i in 0..breaks {
            let f = page1_force[i] * page1_force[i] + page2_force[i] * page2_force[i];
            let dem = f
                + page1_penalty[i]
                + page2_penalty[i]
                + lines[i + 1].page_penalty
                + back.page_penalty
                + back.turn_penalty;
            if dem < best_demerits {
                best_demerits = dem;
                best_sys_count = i + 1;
            }
        }

        let mut ret = PageSpacingResult::default();
        let b = best_sys_count - 1;
        ret.systems_per_page.push(best_sys_count);
        ret.systems_per_page.push(lines.len() - best_sys_count);
        ret.force.push(page1_force[b]);
        ret.force.push(page2_force[b]);
        ret.system_count_status = page1_status[b] | page2_status[b];
        ret.penalty = lines[b].page_penalty
            + back.page_penalty
            + back.turn_penalty
            + page1_penalty[b]
            + page2_penalty[b];
        Ok(ret)
    }

    /// `all_lines_stretched (configuration)`: whether no line of the
    /// configuration is compressed.
    pub fn all_lines_stretched(
        &mut self,
        host: &mut dyn Host,
        configuration: usize,
    ) -> Result<bool, Exception> {
        self.cache_line_details(host, configuration)?;
        Ok(self.cached_line_details.iter().all(|l| l.force >= 0.0))
    }

    /// `min_whitespace_at_top_of_page (line)`: the least distance from the
    /// top of the printable area to `line`'s extent, by the paper's
    /// `top-system-spacing` (`top-markup-spacing` for a title): its
    /// padding, or its minimum distance less how high the line reaches.
    pub fn min_whitespace_at_top_of_page(&self, line: &LineDetails) -> f64 {
        let (min_distance, padding) = if line.title {
            self.top_markup_spacing
        } else {
            self.top_system_spacing
        };
        let translate = f64::max(line.shape.begin.at(UP), line.shape.rest.at(UP));
        f64::max(0.0, f64::max(padding, min_distance - translate))
    }

    /// `min_whitespace_at_bottom_of_page (line)`: the same at the bottom,
    /// by the paper's `last-bottom-spacing`.
    pub fn min_whitespace_at_bottom_of_page(&self, line: &LineDetails) -> f64 {
        let (min_distance, padding) = self.last_bottom_spacing;
        let translate = f64::min(line.shape.begin.at(DOWN), line.shape.rest.at(DOWN));
        f64::max(0.0, f64::max(padding, min_distance + translate))
    }
}

/// `compress_lines (orig)`: the lines with each line after a forbidden
/// page break (a page permission that is no symbol) merged into the line
/// before: its shape stacked on the one before's (`piggyback`) with the
/// one before's padding (a title's padding before a title) unless it is
/// tightly spaced, its refpoints' top the one before's and its bottom
/// moved by how much higher the shape's rest reaches, its spring's length
/// and inverse Hooke's constant the sums, the count of lines merged into
/// it, whether it is a title the first line's, and the footnotes' and
/// in-notes' heights of both.
pub fn compress_lines(orig: &[LineDetails]) -> Vec<LineDetails> {
    let mut ret: Vec<LineDetails> = Vec::new();

    for line in orig {
        match ret.last_mut() {
            Some(old) if old.page_permission.tag() != Tag::Symbol => {
                let mut compressed = line.clone();
                // The padding between the lines that we are compressing
                // comes from "old", the upper system here; if a system
                // has tight-spacing, the padding before it is ignored.
                let padding = if line.tight_spacing {
                    0.0
                } else if line.title {
                    old.title_padding
                } else {
                    old.padding
                };

                compressed.shape = old.shape.piggyback(line.shape, padding);
                *compressed.refpoint_extent.at_mut(UP) = old.refpoint_extent.at(UP);
                *compressed.refpoint_extent.at_mut(DOWN) +=
                    compressed.shape.rest.at(UP) - old.shape.rest.at(UP);
                compressed.space += old.space;
                compressed.inverse_hooke += old.inverse_hooke;

                compressed.compressed_lines_count = old.compressed_lines_count + 1;
                compressed.compressed_nontitle_lines_count =
                    old.compressed_nontitle_lines_count + if compressed.title { 0 } else { 1 };

                // compressed.title is true if and only if the first of its
                // compressed lines was a title.
                compressed.title = old.title;

                // Adds footnotes of one line to the footnotes of another.
                compressed
                    .footnote_heights
                    .splice(0..0, old.footnote_heights.iter().copied());
                compressed
                    .in_note_heights
                    .splice(0..0, old.in_note_heights.iter().copied());

                *old = compressed;
            }
            _ => ret.push(line.clone()),
        }
    }
    ret
}

/// `uncompress_solution (systems_per_page, compressed)`: the number of
/// systems on each page counted in the uncompressed lines.
pub fn uncompress_solution(systems_per_page: &[usize], compressed: &[LineDetails]) -> Vec<usize> {
    let mut ret = Vec::new();
    let mut start_sys = 0;

    for &n in systems_per_page {
        let compressed_count: i64 = compressed[start_sys..start_sys + n]
            .iter()
            .map(|line| i64::from(line.compressed_lines_count) - 1)
            .sum();

        ret.push((n as i64 + compressed_count) as usize);
        start_sys += n;
    }
    ret
}

/// The paper of the `Paper_book` `book`, `#f` for what is none.
fn book_paper(heap: &Heap, book: Value) -> Value {
    heap.foreign::<PaperBook>(book)
        .map_or(Value::FALSE, |b| b.paper)
}

/// `from_scm<double> (paper->c_variable (name), fallback)`.
fn number_or(host: &mut dyn Host, paper: Value, name: &str, fallback: f64) -> f64 {
    let v = paper_score::c_variable(host, paper, name);
    host.heap().to_f64(v).unwrap_or(fallback)
}

/// `from_scm (paper->c_variable (name), fallback)` of an `int`: an exact
/// integer an `int` holds, `fallback` otherwise.
fn int_or(host: &mut dyn Host, paper: Value, name: &str, fallback: i64) -> i64 {
    let v = paper_score::c_variable(host, paper, name);
    host.heap()
        .to_i64(v)
        .filter(|n| i32::try_from(*n).is_ok())
        .unwrap_or(fallback)
}

/// The `minimum-distance` (−∞ for none) and `padding` (0 for none) of
/// the paper's spacing spec `name` (`read_spacing_spec`).
fn spacing_of(host: &mut dyn Host, paper: Value, name: &str) -> Result<(f64, f64), Exception> {
    let spec = paper_score::c_variable(host, paper, name);
    let heap = host.heap();
    let min_distance = read_spacing_spec(heap, spec, "minimum-distance")?;
    let padding = read_spacing_spec(heap, spec, "padding")?;
    Ok((
        min_distance.unwrap_or(f64::NEG_INFINITY),
        padding.unwrap_or(0.0),
    ))
}

/// `from_scm<vsize>` of a number: Guile's type error for what is no
/// exact non-negative integer.
fn to_vsize(heap: &mut Heap, v: Value) -> Result<usize, Exception> {
    match heap.to_i64(v).and_then(|n| usize::try_from(n).ok()) {
        Some(n) => Ok(n),
        None => Err(crate::wrong_type(heap, None, 0, v, "exact integer")),
    }
}

/// The interval of a pair of reals, empty for what is none.
fn interval_of(heap: &Heap, v: Value) -> Interval {
    let end = |e: Option<Value>| e.and_then(|e| heap.to_f64(e));
    match (end(heap.car(v)), end(heap.cdr(v))) {
        (Some(left), Some(right)) => Interval::new(left, right),
        _ => Interval::EMPTY,
    }
}
