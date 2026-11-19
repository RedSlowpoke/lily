//! `constrained-breaking.cc` (part 5's, MS3): the lines a Paper_score's
//! columns can be broken into, and the choice among them. MS3 step 32
//! set it up: the two constructors (`constrained-breaking.cc:364–369`,
//! `:371–375`) and `initialize` (`:390–499`), 121 lines, and
//! `space_line` (`:126–152`), 27, with `line_dimension_interval`
//! (`output-def.cc:248–254`) borrowed. `initialize` finds the forces of
//! every line from one break to a later one and the breaks each starting
//! column begins at, and fills the details of each line that fits
//! (`fill_line_details`, `:505–565`). `space_line` spaces the line
//! between two of the breaks; its loose columns are placed after it, by
//! `set_loose_columns` (`spacing_loose_columns.rs`).
//!
//! The choice of breaks (MS3 step 61): the dynamic programme `state_`
//! over the lines (`resize`, `:154–173`, and `calc_subproblem`,
//! `:79–124`, with `combine_demerits`, `:567–574`), the lines of a given
//! number of systems ([`solve`](ConstrainedBreaking::solve), `:175–218`,
//! and their details, [`line_details`](ConstrainedBreaking::line_details),
//! `:256–309`), the best number of them
//! ([`best_solution`](ConstrainedBreaking::best_solution), `:220–254`)
//! between `min_system_count` and `max_system_count` (`:311–337`), and
//! `prepare_solution` (`:339–356`); `min_permission` (`:377–386`);
//! `Line_details`' `full_height`, `tallness` and `spring_length`
//! (`:640–669`) and `Line_shape`'s constructor and `piggyback`
//! (`:671–685`), which the page breakers read; and `Line_details`'
//! constructor of a title's or a markup's prob (`:576–638`), which the
//! page breaker reads (MS3 step 62, ahead of step 63's line).
//!
//! Since MS6 step 9 (D249) the details of a line whose key a line of the
//! last compile had are taken from it ([`crate::cell::LineKeys`]), not
//! filled again: 05 §6.2's lines recomputed only where a changed grob is.
//! The forces are found whole, as they cost what the columns' descriptions
//! they come from cost, and the dynamic programme runs whole, as it costs
//! less than a millisecond.
//!
//! LilyPond's `Constrained_breaking` lives on the stack of
//! `Paper_score::calc_breaking` or in a `Page_breaking`
//! (`page_breaking.rs`); its values are the Paper_score's and its
//! system's columns, which the caller keeps.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use lily_engine_music::{Host, host_lookup_variable};
use lily_engine_scheme::{Exception, Heap, Value};

use crate::geometry::{Direction, Interval, Y};
use crate::grob::{self, to_double};
use crate::paper_column;
use crate::paper_score;
use crate::simple_spacer::{ColumnXPositions, get_line_configuration};
use crate::spacing_spec::read_spacing_spec;
use crate::stencil::Stencil;
use crate::system;

const DOWN: Direction = -1;
const UP: Direction = 1;

/// `VPOS`: an `end` past the last starting column, to break to the end.
pub const VPOS: usize = usize::MAX;

/// flower's `Matrix`: `rank` rows of each of its columns, `at (row, col)` the
/// item `row` of column `col`. A column to a block, so that it needs no one
/// block as long as itself, which memory near the cap may not have (D259).
#[derive(Clone, Debug)]
pub struct Matrix<T> {
    data: Vec<Vec<T>>,
    rank: usize,
}

/// A [`Matrix`] of as many columns as rows, of which each column has few
/// items that are not the same one (D259): `lines`, where only a line that
/// fits on a line is written, and a staff of 609 bars' 609 × 609 were one
/// block of 91 MiB, the largest thing a long score allocates.
#[derive(Clone, Debug)]
pub struct Band<T> {
    /// The first row written and the rows from it on.
    columns: Vec<(usize, Vec<T>)>,
    blank: Option<Box<T>>,
}

impl<T> Default for Band<T> {
    fn default() -> Band<T> {
        Band {
            columns: Vec::new(),
            blank: None,
        }
    }
}

impl<T: Clone> Band<T> {
    /// The number of rows, as of columns.
    pub fn rows(&self) -> usize {
        self.columns.len()
    }

    /// `size` × `size` copies of `blank`.
    pub fn new(size: usize, blank: T) -> Band<T> {
        Band {
            columns: vec![(0, Vec::new()); size],
            blank: Some(Box::new(blank)),
        }
    }

    /// `at (row, col)`.
    pub fn at(&self, row: usize, col: usize) -> &T {
        let (first, items) = &self.columns[col];
        row.checked_sub(*first)
            .and_then(|i| items.get(i))
            .or(self.blank.as_deref())
            .expect("a band with columns has its blank")
    }

    /// `at (row, col)`, to change it, which keeps the rows from the first
    /// written to it.
    pub fn at_mut(&mut self, row: usize, col: usize) -> &mut T {
        let blank = self
            .blank
            .as_deref()
            .expect("a band with columns has its blank");
        let (first, items) = &mut self.columns[col];
        if items.is_empty() {
            *first = row;
        } else if row < *first {
            let before = core::iter::repeat_n(blank.clone(), *first - row);
            items.splice(0..0, before);
            *first = row;
        }
        let i = row - *first;
        if i >= items.len() {
            items.resize(i + 1, blank.clone());
        }
        &mut items[i]
    }
}

impl<T> Default for Matrix<T> {
    /// `Matrix ()`: no rows.
    fn default() -> Matrix<T> {
        Matrix {
            data: Vec::new(),
            rank: 0,
        }
    }
}

impl<T: Clone> Matrix<T> {
    /// `Matrix (rows, columns, t)`: `rows` × `columns` copies of `t`.
    pub fn new(rows: usize, columns: usize, t: T) -> Matrix<T> {
        Matrix {
            data: vec![vec![t; rows]; columns],
            rank: rows,
        }
    }

    /// `at (row, col)`.
    pub fn at(&self, row: usize, col: usize) -> &T {
        &self.data[col][row]
    }

    /// `at (row, col)`, to change it.
    pub fn at_mut(&mut self, row: usize, col: usize) -> &mut T {
        &mut self.data[col][row]
    }

    /// `resize (rows, columns, t)`: `rows` × `columns` items, those of
    /// the rows and columns it had kept at their places, new ones copies of
    /// `t`.
    pub fn resize(&mut self, rows: usize, columns: usize, t: T) {
        if rows != self.rank {
            for column in &mut self.data {
                column.resize(rows, t.clone());
            }
            self.rank = rows;
        }
        self.data.resize(columns, vec![t; rows]);
    }

    /// The number of rows.
    pub fn rows(&self) -> usize {
        self.rank
    }

    /// The number of columns.
    pub fn columns(&self) -> usize {
        if self.rank == 0 { 0 } else { self.data.len() }
    }
}

/// `Line_shape`: the line's Y extent at its beginning and over the rest of
/// it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineShape {
    pub begin: Interval,
    pub rest: Interval,
}

impl LineShape {
    /// `Line_shape (begin, rest)`.
    pub fn new(begin: Interval, rest: Interval) -> LineShape {
        LineShape { begin, rest }
    }

    /// `piggyback`: this shape with `mount` stacked on top of it, `padding`
    /// apart, at the least elevation that keeps each part of `mount` above
    /// the same part of this.
    pub fn piggyback(&self, mount: LineShape, padding: f64) -> LineShape {
        let elevation = f64::max(
            self.begin.at(UP) - mount.begin.at(DOWN),
            self.rest.at(UP) - mount.rest.at(DOWN),
        );
        let begin = Interval::new(
            self.begin.at(DOWN),
            elevation + mount.begin.at(UP) + padding,
        );
        let rest = Interval::new(self.rest.at(DOWN), elevation + mount.rest.at(UP) + padding);
        LineShape::new(begin, rest)
    }
}

impl Default for LineShape {
    /// `Line_shape ()`: both empty, as an `Interval` is made.
    fn default() -> LineShape {
        LineShape {
            begin: Interval::EMPTY,
            rest: Interval::EMPTY,
        }
    }
}

/// `Line_details`: a line from one break to another, as breaking and the
/// page breakers weigh it.
#[derive(Clone, Debug, PartialEq)]
pub struct LineDetails {
    /// `last_column_`: the column it ends at, none until known.
    pub last_column: Option<Value>,
    /// `force_`: the force spacing it takes, its penalty
    /// ([`crate::simple_spacer::get_line_forces`]).
    pub force: f64,
    pub shape: LineShape,
    /// The heights of the footnotes at the bottom of the page.
    pub footnote_heights: Vec<f64>,
    /// The heights of the in-notes under the line.
    pub in_note_heights: Vec<f64>,
    /// The refpoints of its first and last spaceable staves.
    pub refpoint_extent: Interval,
    /// Its Y extent by its shape.
    pub tallness: f64,
    /// The space required after it, unless last on its page.
    pub padding: f64,
    pub title_padding: f64,
    pub min_distance: f64,
    pub title_min_distance: f64,
    pub bottom_padding: f64,
    /// The length of the spring after it.
    pub space: f64,
    pub title_space: f64,
    pub inverse_hooke: f64,
    pub break_permission: Value,
    pub page_permission: Value,
    pub turn_permission: Value,
    pub break_penalty: f64,
    pub page_penalty: f64,
    pub turn_penalty: f64,
    pub title: bool,
    /// What the page breaker keeps of lines it compresses into one.
    pub compressed_lines_count: i32,
    pub compressed_nontitle_lines_count: i32,
    pub last_markup_line: bool,
    pub first_markup_line: bool,
    pub tight_spacing: bool,
}

impl LineDetails {
    /// `Line_details ()`: no column, an infinite force, permissions
    /// `allow`, an inverse Hooke of 1 and one line; 0 and false otherwise.
    pub fn new(heap: &mut Heap) -> LineDetails {
        let allow = heap.intern("allow");
        LineDetails {
            last_column: None,
            force: f64::INFINITY,
            shape: LineShape::default(),
            footnote_heights: Vec::new(),
            in_note_heights: Vec::new(),
            refpoint_extent: Interval::new(0.0, 0.0),
            tallness: 0.0,
            padding: 0.0,
            title_padding: 0.0,
            min_distance: 0.0,
            title_min_distance: 0.0,
            bottom_padding: 0.0,
            space: 0.0,
            title_space: 0.0,
            inverse_hooke: 1.0,
            break_permission: allow,
            page_permission: allow,
            turn_permission: allow,
            break_penalty: 0.0,
            page_penalty: 0.0,
            turn_penalty: 0.0,
            title: false,
            compressed_lines_count: 1,
            compressed_nontitle_lines_count: 1,
            last_markup_line: false,
            first_markup_line: false,
            tight_spacing: false,
        }
    }

    /// `Line_details (pb, paper)` (`constrained-breaking.cc:576–638`, MS3
    /// step 62): the details of the paper system `pb`, a title or a markup
    /// line: the paper's `markup-system-spacing` and, before a title,
    /// `markup-markup-spacing` (their basic distance, padding and minimum
    /// distance; 0 for none); its footnotes' heights (each footnote's
    /// stencil, LilyPond's programming error for one that is none); no
    /// force; its shape its stencil's Y extent all the way across (0 to 0
    /// for an empty one or none); its page and turn permissions and
    /// penalties (0 for no number), whether it is a title, and its markup
    /// line's and tight spacing's flags as the prob has them.
    pub fn of_prob(host: &mut dyn Host, pb: Value, paper: Value) -> Result<LineDetails, Exception> {
        let mut d = LineDetails::new(host.heap());
        let spec = paper_score::c_variable(host, paper, "markup-system-spacing");
        let title_spec = paper_score::c_variable(host, paper, "markup-markup-spacing");
        let heap = host.heap();
        let read = |heap: &mut Heap, spec, name| -> Result<f64, Exception> {
            Ok(read_spacing_spec(heap, spec, name)?.unwrap_or(0.0))
        };
        d.space = read(heap, spec, "basic-distance")?;
        d.title_space = read(heap, title_spec, "basic-distance")?;
        d.padding = read(heap, spec, "padding")?;
        d.title_padding = read(heap, title_spec, "padding")?;
        d.min_distance = read(heap, spec, "minimum-distance")?;
        d.title_min_distance = read(heap, title_spec, "minimum-distance")?;

        let prop = |heap: &mut Heap, name: &str| {
            let sym = heap.intern(name);
            lily_engine_music::prob_get(heap, pb, sym)
        };
        let mut footnotes = prop(heap, "footnotes");
        while let (Some(f), Some(rest)) = (host.heap().car(footnotes), host.heap().cdr(footnotes)) {
            let heap = host.heap();
            let sten = heap
                .cdr(f)
                .and_then(|r| heap.cdr(r))
                .and_then(|r| heap.car(r))
                .and_then(|s| heap.foreign::<Stencil>(s).copied());
            match sten {
                Some(sten) => d.footnote_heights.push(sten.extent(Y).length()),
                None => {
                    grob::unlocated_programming_error(host, "expecting stencil, got empty pointer")?
                }
            }
            footnotes = rest;
        }

        let heap = host.heap();
        d.last_column = None;
        d.force = 0.0;
        let stencil = prop(heap, "stencil");
        let stencil_extent = match heap.foreign::<Stencil>(stencil) {
            Some(st) if !st.is_empty_on(Y) => st.extent(Y),
            _ => Interval::new(0.0, 0.0),
        };
        // Pretend it goes all the way across.
        d.shape = LineShape::new(stencil_extent, stencil_extent);
        d.tallness = 0.0;
        d.bottom_padding = 0.0;
        d.inverse_hooke = 1.0;
        d.break_permission = heap.intern("allow");
        d.page_permission = prop(heap, "page-break-permission");
        d.turn_permission = prop(heap, "page-turn-permission");
        d.break_penalty = 0.0;
        let page_penalty = prop(heap, "page-break-penalty");
        d.page_penalty = heap.to_f64(page_penalty).unwrap_or(0.0);
        let turn_penalty = prop(heap, "page-turn-penalty");
        d.turn_penalty = heap.to_f64(turn_penalty).unwrap_or(0.0);
        d.title = prop(heap, "is-title") == Value::TRUE;
        d.compressed_lines_count = 1;
        d.compressed_nontitle_lines_count = if d.title { 0 } else { 1 };
        d.last_markup_line = prop(heap, "last-markup-line") == Value::TRUE;
        d.first_markup_line = prop(heap, "first-markup-line") == Value::TRUE;
        d.tight_spacing = prop(heap, "tight-spacing") == Value::TRUE;
        d.refpoint_extent = Interval::new(0.0, 0.0);
        Ok(d)
    }

    /// `full_height`: the length of its shape's two parts united.
    pub fn full_height(&self) -> f64 {
        let mut ret = Interval::EMPTY;
        ret.unite(self.shape.begin);
        ret.unite(self.shape.rest);
        ret.length()
    }

    /// `tallness`.
    pub fn tallness(&self) -> f64 {
        self.tallness
    }

    /// `spring_length`: the stretchable space between the bottom of this
    /// line's extent and the top of `next_line`'s: its spring (`space`,
    /// or `title_space` before a title), which goes from this line's
    /// bottom refpoint to the next's top one, less the distance between
    /// those refpoints, and not below 0.
    pub fn spring_length(&self, next_line: &LineDetails) -> f64 {
        let refpoint_dist =
            self.tallness + self.refpoint_extent.at(DOWN) - next_line.refpoint_extent.at(UP);
        let space = if next_line.title {
            self.title_space
        } else {
            self.space
        };
        f64::max(0.0, space - refpoint_dist)
    }
}

/// `Constrained_break_node`: the break before a line (`prev_`, none for
/// `VPOS`), the demerits of all lines up to it, and its details.
#[derive(Clone, Debug, PartialEq)]
pub struct BreakNode {
    pub prev: Option<usize>,
    pub demerits: f64,
    pub details: LineDetails,
}

impl BreakNode {
    /// `Constrained_break_node ()`: no break before it, infinite
    /// demerits, the default details.
    pub fn new(heap: &mut Heap) -> BreakNode {
        BreakNode {
            prev: None,
            demerits: f64::INFINITY,
            details: LineDetails::new(heap),
        }
    }
}

/// `Constrained_breaking`: the lines a Paper_score's columns can be
/// broken into, and the state of choosing among them.
#[derive(Clone, Debug)]
pub struct ConstrainedBreaking {
    /// `pscore_`: none for none, and for what is no Paper_score.
    pub pscore: Option<Value>,
    pub valid_systems: usize,
    pub systems: usize,
    pub ragged_right: bool,
    pub ragged_last: bool,
    pub system_system_min_distance: f64,
    pub system_system_padding: f64,
    pub system_system_space: f64,
    pub system_markup_space: f64,
    pub score_system_min_distance: f64,
    pub score_system_padding: f64,
    pub score_markup_min_distance: f64,
    pub score_markup_padding: f64,
    /// `lines_`: at `(j, i)` the line from break `i` to break `j`.
    pub lines: Band<LineDetails>,
    /// `state_`: for each starting column, the best way to fit the first
    /// lines into the first systems; empty until `resize`.
    pub state: Vec<Matrix<BreakNode>>,
    /// `start_`: the columns breaking may be asked to start at.
    pub start: Vec<usize>,
    /// `starting_breakpoints_`: their indices into `breaks`.
    pub starting_breakpoints: Vec<usize>,
    /// `all_`: the root system's used columns.
    pub all: Vec<Value>,
    /// `breaks_`: the indices into `all` of the Paper_score's breaks.
    pub breaks: Vec<usize>,
}

/// The lines' details under way (H3, D254), the next line from break `i` to
/// break `j`, and what `initialize` found for them.
pub struct Filling {
    keys: Option<crate::cell::LineKeys>,
    ranks: Vec<u64>,
    forces: Vec<f64>,
    n: usize,
    i: usize,
    j: usize,
    starts: Vec<usize>,
}

impl ConstrainedBreaking {
    /// `Constrained_breaking (Paper_score *)`: breaking `ps` from its
    /// first column.
    pub fn new(host: &mut dyn Host, ps: Option<Value>) -> Result<ConstrainedBreaking, Exception> {
        ConstrainedBreaking::with_starts(host, ps, &[0])
    }

    /// `Constrained_breaking (Paper_score *, start)`: breaking `ps` from
    /// each of the columns `start`.
    pub fn with_starts(
        host: &mut dyn Host,
        ps: Option<Value>,
        start: &[usize],
    ) -> Result<ConstrainedBreaking, Exception> {
        let (mut cb, mut filling) = ConstrainedBreaking::starting(host, ps, start)?;
        while let Some(f) = &mut filling {
            if cb.fill_on(host, f)? {
                break;
            }
        }
        Ok(cb)
    }

    /// [`with_starts`](Self::with_starts) as far as the forces, the rest
    /// left to [`fill_on`](Self::fill_on).
    pub fn starting(
        host: &mut dyn Host,
        ps: Option<Value>,
        start: &[usize],
    ) -> Result<(ConstrainedBreaking, Option<Filling>), Exception> {
        let mut cb = ConstrainedBreaking {
            pscore: None,
            valid_systems: 0,
            systems: 0,
            ragged_right: false,
            ragged_last: false,
            system_system_min_distance: 0.0,
            system_system_padding: 0.0,
            system_system_space: 0.0,
            system_markup_space: 0.0,
            score_system_min_distance: 0.0,
            score_system_padding: 0.0,
            score_markup_min_distance: 0.0,
            score_markup_padding: 0.0,
            lines: Band::default(),
            state: Vec::new(),
            start: Vec::new(),
            starting_breakpoints: Vec::new(),
            all: Vec::new(),
            breaks: Vec::new(),
        };
        let filling = cb.initialize(host, ps, start)?;
        Ok((cb, filling))
    }

    /// `initialize`: with no Paper_score, nothing, not ragged. Otherwise
    /// the layout's `ragged-right` and `ragged-last` (`#t` alone is true),
    /// the system and score spacings from its spacing specs, and the
    /// force of each line from one break of `ps` to a later one: spaced
    /// for the layout's `line-width` less its `short-indent`, the first
    /// line for `line-width` less its `indent`; a ragged last line 0, or
    /// infinite if squeezed and not between neighbouring breaks. A line
    /// that does not fit (infinite) ends the lines from its first break.
    /// Then, for each
    /// column of `pagebreak_col_indices`, the first break not before it
    /// (the last break at most) to start at.
    ///
    /// Defined answers where LilyPond reads through a null or past the
    /// end: a Paper_score with no root system has no columns, no columns
    /// no forces, and with no breaks there is nothing to start at.
    fn initialize(
        &mut self,
        host: &mut dyn Host,
        ps: Option<Value>,
        pagebreak_col_indices: &[usize],
    ) -> Result<Option<Filling>, Exception> {
        self.valid_systems = 0;
        self.systems = 0;
        self.system_system_space = 0.0;
        self.system_markup_space = 0.0;
        self.system_system_padding = 0.0;
        self.system_system_min_distance = 0.0;
        self.score_system_padding = 0.0;
        self.score_system_min_distance = 0.0;
        self.score_markup_padding = 0.0;
        self.score_markup_min_distance = 0.0;

        let found = ps.and_then(|ps| paper_score::layout(host.heap(), ps).map(|l| (ps, l)));
        let Some((ps, l)) = found else {
            self.pscore = None;
            self.ragged_right = false;
            self.ragged_last = false;
            return Ok(None);
        };
        self.pscore = Some(ps);

        self.ragged_right = paper_score::c_variable(host, l, "ragged-right") == Value::TRUE;
        self.ragged_last = paper_score::c_variable(host, l, "ragged-last") == Value::TRUE;

        let spacing_spec = paper_score::c_variable(host, l, "system-system-spacing");
        let between_scores_spec = paper_score::c_variable(host, l, "score-system-spacing");
        let title_spec = paper_score::c_variable(host, l, "score-markup-spacing");
        let page_breaking_spacing_spec =
            paper_score::c_variable(host, l, "page-breaking-system-system-spacing");

        let heap = host.heap();
        let read = |heap: &mut Heap, spec, out: &mut f64, sym| {
            if let Some(v) = read_spacing_spec(heap, spec, sym)? {
                *out = v;
            }
            Ok::<(), Exception>(())
        };
        read(
            heap,
            spacing_spec,
            &mut self.system_system_space,
            "basic-distance",
        )?;
        read(
            heap,
            page_breaking_spacing_spec,
            &mut self.system_system_space,
            "basic-distance",
        )?;
        read(
            heap,
            title_spec,
            &mut self.system_markup_space,
            "basic-distance",
        )?;

        read(
            heap,
            spacing_spec,
            &mut self.system_system_padding,
            "padding",
        )?;
        read(
            heap,
            between_scores_spec,
            &mut self.score_system_padding,
            "padding",
        )?;
        read(
            heap,
            page_breaking_spacing_spec,
            &mut self.system_system_padding,
            "padding",
        )?;
        read(heap, title_spec, &mut self.score_markup_padding, "padding")?;

        read(
            heap,
            between_scores_spec,
            &mut self.score_system_min_distance,
            "minimum-distance",
        )?;
        read(
            heap,
            spacing_spec,
            &mut self.system_system_min_distance,
            "minimum-distance",
        )?;
        read(
            heap,
            page_breaking_spacing_spec,
            &mut self.system_system_min_distance,
            "minimum-distance",
        )?;
        read(
            heap,
            title_spec,
            &mut self.score_markup_min_distance,
            "minimum-distance",
        )?;

        let first_line = line_dimension_interval(host, l, 0)?;
        let other_lines = line_dimension_interval(host, l, 1)?;
        // Do all the rod/spring problems.
        self.breaks = paper_score::get_break_indices(host, ps)?;
        self.all = match paper_score::root_system(host.heap(), ps) {
            Some(sys) => system::used_columns_in_range(host, sys, 0, usize::MAX)?,
            None => Vec::new(),
        };
        let n = self.breaks.len();
        self.lines = Band::new(n, LineDetails::new(host.heap()));
        let keys = crate::cell::line_keys(host.heap(), ps);
        let ranks: Vec<u64> = self
            .breaks
            .iter()
            .map(|&b| paper_column::rank(host.heap(), self.all[b]))
            .collect();
        let forces = if self.all.is_empty() {
            Vec::new()
        } else {
            crate::simple_spacer::get_line_forces(
                host,
                &self.all,
                other_lines.length(),
                other_lines.length() - first_line.length(),
                self.ragged_right,
            )?
        };
        Ok(Some(Filling {
            keys,
            ranks,
            forces,
            n,
            i: 0,
            j: 1,
            starts: pagebreak_col_indices.to_vec(),
        }))
    }

    /// Goes on with what [`initialize`](Self::initialize) leaves (H3, D254):
    /// the lines' details, then the starting indices; whether they are done,
    /// false when the step's time is spent, between lines.
    pub fn fill_on(&mut self, host: &mut dyn Host, f: &mut Filling) -> Result<bool, Exception> {
        let n = f.n;
        while f.i + 1 < n {
            let i = f.i;
            while f.j < n {
                let j = f.j;
                let last = j == n - 1;
                let ragged = self.ragged_right || (last && self.ragged_last);
                let mut force = f.forces[i * n + j];
                if ragged && last && !force.is_infinite() {
                    force = if force < 0.0 && j > i + 1 {
                        f64::INFINITY
                    } else {
                        0.0
                    };
                }
                self.lines.at_mut(j, i).force = force;
                if force.is_infinite() {
                    break;
                }

                let mut line = self.lines.at(j, i).clone();
                match &f.keys {
                    Some(keys) => {
                        let key = keys.key(f.ranks[i], f.ranks[j], last);
                        self.kept_line_details(host, keys, key, &mut line, i, j)?;
                    }
                    None => self.fill_line_details(host, &mut line, i, j)?,
                }
                *self.lines.at_mut(j, i) = line;
                f.j += 1;
                if lily_engine_music::returns::due() {
                    return Ok(false);
                }
            }
            f.i += 1;
            f.j = f.i + 1;
        }

        // Work out all the starting indices.
        if n == 0 {
            return Ok(true);
        }
        self.start.reserve(f.starts.len());
        for &pb_col in &f.starts {
            // pagebreak_col_indices may repeat an entry, as when a score
            // starts with a \pageBreak.
            let mut j = 0;
            while j + 1 < n && self.breaks[j] < pb_col {
                j += 1;
            }
            self.starting_breakpoints.push(j);
            self.start.push(self.breaks[j]);
        }
        self.state = vec![Matrix::default(); self.start.len()];
        Ok(true)
    }

    /// `space_line (i, j)` (`constrained-breaking.cc:126–152`): the line
    /// from break `i` to break `j` (`i <= j < breaks.len ()`, as LilyPond's
    /// callers pass), its columns `all[breaks[i]..=breaks[j]]` spaced by
    /// [`get_line_configuration`] for the line
    /// [`line_dimension_interval`] gives line `i`, from its left. Ragged
    /// if the layout's `ragged-right` is `#t`, or `ragged-last` and the
    /// line is the last, both read again here; and a score's only line,
    /// from the first break to the last, is ragged too when neither is a
    /// boolean and its force is not negative. That force is read at
    /// `lines.at (i, j)`, which [`initialize`](Self::initialize) never
    /// writes (it keeps the line at `(j, i)`): it is infinite, so the
    /// only line is ragged whenever neither is a boolean, as in LilyPond.
    ///
    /// Defined answer where LilyPond reads through a null: with no
    /// Paper_score, no columns.
    pub fn space_line(
        &self,
        host: &mut dyn Host,
        i: usize,
        j: usize,
    ) -> Result<ColumnXPositions, Exception> {
        let Some(l) = self
            .pscore
            .and_then(|ps| paper_score::layout(host.heap(), ps))
        else {
            return Ok(ColumnXPositions::default());
        };
        let ragged_right = paper_score::c_variable(host, l, "ragged-right") == Value::TRUE;
        let ragged_last = paper_score::c_variable(host, l, "ragged-last") == Value::TRUE;

        let line = &self.all[self.breaks[i]..=self.breaks[j]];
        let line_dims = line_dimension_interval(host, l, i)?;
        let last = j == self.breaks.len() - 1;
        let mut ragged = ragged_right || (last && ragged_last);

        // As a special case, if there is only one line in the score and
        // ragged-right hasn't been specifically forbidden and the line is
        // stretched, use ragged spacing.
        if last
            && i == 0
            && self.lines.at(i, j).force >= 0.0
            && !is_bool(paper_score::c_variable(host, l, "ragged-right"))
            && !is_bool(paper_score::c_variable(host, l, "ragged-last"))
        {
            ragged = true;
        }

        get_line_configuration(
            host,
            line,
            line_dims.right - line_dims.left,
            line_dims.left,
            ragged,
        )
    }

    /// `resize`: `systems` the number of systems to consider; if more
    /// than the dynamic programme holds and there is a Paper_score, each
    /// starting column's matrix grown to them and filled out
    /// ([`calc_subproblem`](Self::calc_subproblem)), each system count's
    /// breaks until one cannot be reached, as it is too cramped already.
    fn resize(&mut self, heap: &mut Heap, systems: usize) {
        self.systems = systems;

        if self.pscore.is_some() && self.systems > self.valid_systems {
            let node = BreakNode::new(heap);
            for i in 0..self.state.len() {
                let rows = self.breaks.len() - self.starting_breakpoints[i];
                self.state[i].resize(rows, self.systems, node.clone());
            }

            // Fill out the matrices.
            for i in 0..self.state.len() {
                for j in self.valid_systems..self.systems {
                    for k in self.starting_breakpoints[i] + j + 1..self.breaks.len() {
                        if !self.calc_subproblem(i, j, k) {
                            break;
                        }
                    }
                }
            }
            self.valid_systems = self.systems;
        }
    }

    /// `calc_subproblem (start, sys, brk)`: the best way to end the
    /// system `sys` (from 0) at the break `brk`, breaking from the starting
    /// column `start`: the line from each earlier break that fits after
    /// the best way to end system `sys - 1` there, weighed by its
    /// demerits ([`combine_demerits`](Self::combine_demerits)) and its
    /// break's penalty, the latest first; the first system starts at the
    /// starting column. Whether a way was found.
    fn calc_subproblem(&mut self, start: usize, sys: usize, brk: usize) -> bool {
        let mut found_something = false;
        let start_col = self.starting_breakpoints[start];
        let max_index = brk - start_col;
        for j in (sys..max_index).rev() {
            if sys == 0 && j > 0 {
                // The first line cannot have its first break after the
                // beginning.
                continue;
            }

            let cur = self.lines.at(brk, j + start_col);
            if cur.force.is_infinite() {
                break;
            }

            let st = &self.state[start];
            let (prev_f, prev_dem) = if sys > 0 {
                let prev = st.at(j, sys - 1);
                (prev.details.force, prev.demerits)
            } else {
                (0.0, 0.0)
            };
            if prev_dem.is_infinite() {
                continue;
            }

            let dem = self.combine_demerits(cur.force, prev_f) + prev_dem + cur.break_penalty;
            if dem < st.at(max_index, sys).demerits {
                found_something = true;
                let details = cur.clone();
                let n = self.state[start].at_mut(max_index, sys);
                n.demerits = dem;
                n.details = details;
                n.prev = Some(j);
            }
        }
        found_something
    }

    /// `solve (start, end, sys_count)`: the lines of `sys_count` systems
    /// from the starting column `start` to the starting column `end`
    /// ([`VPOS`] or `start.len ()` for the end), each spaced
    /// ([`space_line`](Self::space_line)). If no `sys_count` systems
    /// satisfy the constraints, the best of fewer, the last of them
    /// spaced to the end, with LilyPond's warning; if none, one line to
    /// the end, warned of too, and none for no systems.
    ///
    /// Defined answer where LilyPond reads past the end: with no
    /// Paper_score or no starting column `start`, no lines.
    pub fn solve(
        &mut self,
        host: &mut dyn Host,
        start: usize,
        end: usize,
        sys_count: usize,
    ) -> Result<Vec<ColumnXPositions>, Exception> {
        if self.pscore.is_none() || start >= self.state.len() {
            return Ok(Vec::new());
        }
        let start_brk = self.starting_breakpoints[start];
        let end_brk = self.prepare_solution(host.heap(), start, end, sys_count);
        let mut ret = Vec::new();

        // Find the first solution that satisfies constraints.
        let Some((brk, sys)) = self.first_solution(start, end_brk, sys_count) else {
            // If we get to here, just put everything on one line.
            if sys_count > 0 {
                grob::unlocated_warning(host, CANNOT_BREAK)?;
                ret.push(self.space_line(host, 0, end_brk)?);
            }
            return Ok(ret);
        };
        let (mut brk, mut cur) = (brk, Some(sys));
        if brk != end_brk {
            brk = self.prev(start, brk, sys);
            cur = sys.checked_sub(1);
            grob::unlocated_warning(host, CANNOT_BREAK)?;
            ret.push(self.space_line(host, brk, end_brk)?);
        }

        // Build up the good part of the solution.
        while let Some(cur_sys) = cur {
            let prev_brk = self.prev(start, brk, cur_sys);
            ret.push(self.space_line(host, prev_brk + start_brk, brk + start_brk)?);
            brk = prev_brk;
            cur = cur_sys.checked_sub(1);
        }
        ret.reverse();
        Ok(ret)
    }

    /// The loops of `solve` and `line_details` that find the first
    /// solution satisfying the constraints: from `sys_count` systems down,
    /// the latest break not after `end_brk` that the system ends at with
    /// a finite force, and that system; none if no system does.
    fn first_solution(
        &self,
        start: usize,
        end_brk: usize,
        sys_count: usize,
    ) -> Option<(usize, usize)> {
        let st = &self.state[start];
        (0..sys_count).rev().find_map(|sys| {
            (0..=end_brk)
                .rev()
                .find(|&brk| !st.at(brk, sys).details.force.is_infinite())
                .map(|brk| (brk, sys))
        })
    }

    /// The break before the system `sys` ending at `brk`, from the
    /// starting column `start`; it has one wherever its force is finite,
    /// where [`first_solution`](Self::first_solution) and the systems
    /// before lead.
    fn prev(&self, start: usize, brk: usize, sys: usize) -> usize {
        self.state[start].at(brk, sys).prev.unwrap_or(0)
    }

    /// `best_solution (start, end)`: the lines from the starting column
    /// `start` to `end` in the number of systems, from
    /// [`min_system_count`](Self::min_system_count) to
    /// [`max_system_count`](Self::max_system_count), with the least
    /// demerits ([`solve`](Self::solve)); a count past the best whose
    /// lines are none of them squeezed has too many lines, and ends the
    /// search. If no count was best, the lines of the largest.
    pub fn best_solution(
        &mut self,
        host: &mut dyn Host,
        start: usize,
        end: usize,
    ) -> Result<Vec<ColumnXPositions>, Exception> {
        if self.pscore.is_none() || start >= self.state.len() {
            return Ok(Vec::new());
        }
        let min_systems = self.min_system_count(host.heap(), start, end);
        let max_systems = self.max_system_count(start, end);
        let mut best_demerits = f64::INFINITY;
        let mut best_so_far = Vec::new();

        for i in min_systems..=max_systems {
            let brk = self.prepare_solution(host.heap(), start, end, i);
            let dem = self.state[start].at(brk, i - 1).demerits;

            if dem < best_demerits {
                best_demerits = dem;
                best_so_far = self.solve(host, start, end, i)?;
            } else {
                let cur = self.solve(host, start, end, i)?;
                let too_many_lines = !cur.iter().any(|c| c.force < 0.0);
                if too_many_lines {
                    return Ok(best_so_far);
                }
            }
        }
        if !best_so_far.is_empty() {
            return Ok(best_so_far);
        }
        self.solve(host, start, end, max_systems)
    }

    /// `line_details (start, end, sys_count)`: the details of the lines
    /// [`solve`](Self::solve) gives, as the dynamic programme keeps them;
    /// those of a line it had no need to fill, as it does not fit
    /// (`initialize` fills only those that do), filled here
    /// ([`fill_line_details`](Self::fill_line_details)).
    pub fn line_details(
        &mut self,
        host: &mut dyn Host,
        start: usize,
        end: usize,
        sys_count: usize,
    ) -> Result<Vec<LineDetails>, Exception> {
        if self.pscore.is_none() || start >= self.state.len() {
            return Ok(Vec::new());
        }
        let end_brk = self.prepare_solution(host.heap(), start, end, sys_count);
        let mut ret = Vec::new();

        // This loop structure is C&Ped from solve().
        let Some((brk, sys)) = self.first_solution(start, end_brk, sys_count) else {
            // If we get to here, just put everything on one line.
            if sys_count > 0 {
                let mut details = LineDetails::new(host.heap());
                self.fill_line_details(host, &mut details, 0, end_brk)?;
                ret.push(details);
            }
            return Ok(ret);
        };
        let (mut brk, mut cur) = (brk, Some(sys));
        if brk != end_brk {
            let mut details = LineDetails::new(host.heap());
            brk = self.prev(start, brk, sys);
            cur = sys.checked_sub(1);
            self.fill_line_details(host, &mut details, brk, end_brk)?;
            ret.push(details);
        }

        // Build up the good part of the solution.
        while let Some(cur_sys) = cur {
            let prev_brk = self.prev(start, brk, cur_sys);
            ret.push(self.state[start].at(brk, cur_sys).details.clone());
            brk = prev_brk;
            cur = cur_sys.checked_sub(1);
        }
        ret.reverse();
        Ok(ret)
    }

    /// `min_system_count (start, end)`: the fewest systems that satisfy
    /// the constraints from the starting column `start` to `end`, the
    /// dynamic programme grown three systems at a time to find them; 1 if
    /// none do.
    ///
    /// Defined answer where LilyPond reads past the end: 1 with no
    /// starting column `start`.
    pub fn min_system_count(&mut self, heap: &mut Heap, start: usize, end: usize) -> usize {
        if start >= self.state.len() {
            return 1;
        }
        let brk = self.prepare_solution(heap, start, end, 1);
        let rank = self.breaks.len() - self.starting_breakpoints[start];

        // sys_count < rank: rank is the number of breakpoints, we can't
        // have more systems.
        for sys_count in 0..rank {
            if sys_count >= self.valid_systems {
                self.resize(heap, sys_count + 3);
            }
            if !self.state[start]
                .at(brk, sys_count)
                .details
                .force
                .is_infinite()
            {
                return sys_count + 1;
            }
        }
        // No possible breaks satisfy constraints.
        1
    }

    /// `max_system_count (start, end)`: the most systems from the
    /// starting column `start` to `end`, one a break.
    ///
    /// Defined answer where LilyPond reads past the end: 0 with no
    /// starting column `start`.
    pub fn max_system_count(&self, start: usize, end: usize) -> usize {
        if start >= self.state.len() {
            return 0;
        }
        let brk = if end >= self.start.len() {
            self.breaks.len() - 1
        } else {
            self.starting_breakpoints[end]
        };
        brk - self.starting_breakpoints[start]
    }

    /// `prepare_solution (start, end, sys_count)`: the dynamic programme
    /// sized for `sys_count` systems ([`resize`](Self::resize)), and the
    /// break `end` ends at, counted from `start`'s first break: the last
    /// for `start.len ()` or [`VPOS`].
    fn prepare_solution(
        &mut self,
        heap: &mut Heap,
        start: usize,
        end: usize,
        sys_count: usize,
    ) -> usize {
        self.resize(heap, sys_count);
        let brk = if end >= self.start.len() {
            self.breaks.len() - 1
        } else {
            self.starting_breakpoints[end]
        };
        brk - self.starting_breakpoints[start]
    }

    /// `fill_line_details (out, start, end)`: all of the information in
    /// `out` but its horizontal spacing, for the line from break `start`
    /// to break `end`: the column it ends at and that column's penalties
    /// and permissions (a page's no looser than its line's, a turn's no
    /// looser than its page's: [`min_permission`]); its shape, the
    /// root System's pure heights at the beginning of the line and in the
    /// rest of it, `(0, 0)` for an empty one or one with a NaN end; the
    /// spacings `initialize` read, a score's last line's padding and
    /// minimum distance the score's; its inverse Hooke's constant, its
    /// full height and the systems' space; its footnotes' and in-notes'
    /// heights; and the refpoints of its first and last spaceable staves,
    /// `(0, 0)` for none.
    ///
    /// Defined answer where LilyPond reads through a null: with no
    /// Paper_score or no root System, `out` unchanged.
    pub fn fill_line_details(
        &self,
        host: &mut dyn Host,
        out: &mut LineDetails,
        start: usize,
        end: usize,
    ) -> Result<(), Exception> {
        let Some(sys) = self
            .pscore
            .and_then(|ps| paper_score::root_system(host.heap(), ps))
        else {
            return Ok(());
        };
        let start_rank = paper_column::rank(host.heap(), self.all[self.breaks[start]]);
        let end_rank = paper_column::rank(host.heap(), self.all[self.breaks[end]]);
        let begin_of_line_extent =
            system::begin_of_line_pure_height(host, sys, start_rank, end_rank)?;
        let rest_of_line_extent =
            system::rest_of_line_pure_height(host, sys, start_rank, end_rank)?;
        let last = end == self.breaks.len() - 1;

        let c = self.all[self.breaks[end]];
        out.last_column = Some(c);
        out.break_penalty = number_or_0(host, c, "line-break-penalty")?;
        out.page_penalty = number_or_0(host, c, "page-break-penalty")?;
        out.turn_penalty = number_or_0(host, c, "page-turn-penalty")?;
        out.break_permission = grob::property(host, c, "line-break-permission")?;
        out.page_permission = grob::property(host, c, "page-break-permission")?;
        out.turn_permission = grob::property(host, c, "page-turn-permission")?;

        // Turn permission should always be stricter than page permission
        // and page permission should always be stricter than line
        // permission.
        let heap = host.heap();
        out.page_permission = min_permission(heap, out.break_permission, out.page_permission);
        out.turn_permission = min_permission(heap, out.page_permission, out.turn_permission);

        out.shape = LineShape::new(or_zero(begin_of_line_extent), or_zero(rest_of_line_extent));
        out.padding = if last {
            self.score_system_padding
        } else {
            self.system_system_padding
        };
        out.title_padding = self.score_markup_padding;
        out.min_distance = if last {
            self.score_system_min_distance
        } else {
            self.system_system_min_distance
        };
        out.title_min_distance = self.score_markup_min_distance;
        out.space = self.system_system_space;
        out.title_space = self.system_markup_space;
        out.inverse_hooke = out.full_height() + self.system_system_space;

        out.footnote_heights =
            system::get_footnote_heights_in_range(host, sys, start_rank, end_rank)?;
        out.in_note_heights =
            system::get_in_note_heights_in_range(host, sys, start_rank, end_rank)?;

        out.refpoint_extent = system::pure_refpoint_extent(host, sys, start_rank, end_rank)?;
        if out.refpoint_extent.is_empty() {
            out.refpoint_extent = Interval::new(0.0, 0.0);
        }
        Ok(())
    }

    /// [`fill_line_details`](Self::fill_line_details) of `out`, or what a
    /// line of the same `key` had (D249), with `out`'s force and its last
    /// column; with the check, filled too and compared, a difference counted
    /// and what was taken kept.
    fn kept_line_details(
        &self,
        host: &mut dyn Host,
        keys: &crate::cell::LineKeys,
        key: u64,
        out: &mut LineDetails,
        start: usize,
        end: usize,
    ) -> Result<(), Exception> {
        let force = out.force;
        let reused = keys.is_old(key);
        let mut wrong = false;
        match keys.kept(key) {
            Some(mut kept) => {
                kept.force = force;
                kept.last_column = Some(self.all[self.breaks[end]]);
                if keys.check() {
                    self.fill_line_details(host, out, start, end)?;
                    wrong = *out != kept;
                }
                *out = kept;
            }
            None => self.fill_line_details(host, out, start, end)?,
        }
        keys.keep(key, out, reused, wrong);
        Ok(())
    }

    /// `combine_demerits`: a line's demerits, the square of its force,
    /// and unless ragged right, the square of its force's change from the
    /// line before.
    fn combine_demerits(&self, force: f64, prev_force: f64) -> f64 {
        if self.ragged_right {
            return force * force;
        }
        force * force + (prev_force - force) * (prev_force - force)
    }

    /// What `Paper_score::calc_breaking` gives for breaks a caller chose
    /// (MS3 step 60), until `solve` chooses them: a line from each of the
    /// breaks `at` (ascending indices into `breaks`) to the next, from
    /// the first break and to the last, each spaced
    /// ([`space_line`](Self::space_line)) as `solve` spaces the lines it
    /// chose; no line for fewer than two breaks.
    pub fn given_lines(
        &self,
        host: &mut dyn Host,
        at: &[usize],
    ) -> Result<Vec<ColumnXPositions>, Exception> {
        let Some(last) = self.breaks.len().checked_sub(1).filter(|&l| l > 0) else {
            return Ok(Vec::new());
        };
        let mut lines = Vec::new();
        let mut i = 0;
        for &j in at.iter().chain([&last]) {
            if j > i && j <= last {
                lines.push(self.space_line(host, i, j)?);
                i = j;
            }
        }
        Ok(lines)
    }
}

/// LilyPond's warning where no breaks satisfy the constraints.
const CANNOT_BREAK: &str = "cannot find line breaking that satisfies constraints";

/// `min_permission (perm1, perm2)`: the stricter of two permissions,
/// `force` looser than `allow`, `allow` looser than anything else; `'()`
/// (none) where neither is `force` and the first is not `allow`.
pub fn min_permission(heap: &mut Heap, perm1: Value, perm2: Value) -> Value {
    let force = heap.intern("force");
    let allow = heap.intern("allow");
    if perm1 == force {
        return perm2;
    }
    if perm1 == allow && perm2 != force {
        return perm2;
    }
    Value::NULL
}

/// `from_scm<double> (get_property (c, name), 0)`: the column's property
/// `name` if it is a number, 0 otherwise.
fn number_or_0(host: &mut dyn Host, c: Value, name: &str) -> Result<f64, Exception> {
    let v = grob::property(host, c, name)?;
    Ok(host.heap().to_f64(v).unwrap_or(0.0))
}

/// An extent as `fill_line_details` keeps it: `(0, 0)` for an empty one
/// or one with a NaN end.
fn or_zero(iv: Interval) -> Interval {
    if iv.is_empty() || iv.left.is_nan() || iv.right.is_nan() {
        Interval::new(0.0, 0.0)
    } else {
        iv
    }
}

/// `scm_is_bool`: `#t` or `#f`.
fn is_bool(v: Value) -> bool {
    v == Value::TRUE || v == Value::FALSE
}

/// `line_dimension_interval` (`output-def.cc:248–254`), borrowed: from
/// the layout `def`'s `indent` for the first line (`n` 0), its
/// `short-indent` otherwise, to its `line-width`, read first; Guile's
/// type error for a dimension that is no real, unbound included.
pub fn line_dimension_interval(
    host: &mut dyn Host,
    def: Value,
    n: usize,
) -> Result<Interval, Exception> {
    let lw = get_dimension(host, def, "line-width")?;
    let ind = get_dimension(host, def, if n != 0 { "short-indent" } else { "indent" })?;
    Ok(Interval::new(ind, lw))
}

/// `Output_def::get_dimension`: `from_scm<double>` of `name` in `def` or
/// its parents.
fn get_dimension(host: &mut dyn Host, def: Value, name: &str) -> Result<f64, Exception> {
    let sym = host.heap().intern(name);
    let v = host_lookup_variable(host, def, sym).unwrap_or(Value::UNDEFINED);
    to_double(host.heap(), v)
}
