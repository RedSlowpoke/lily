//! `simple-spacer.cc` (part 3's, MS3 step 29) and `column-x-positions.cc`:
//! the solver of a chain of springs and rods, which `page-layout-problem.cc`
//! spaces the staves with and a line spaces its columns with. Its solve was
//! ported first by MS3 step 28; step 29 adds the rods (`add_rod`,
//! `rod_force`), the force's penalty and the line's columns: each
//! column's spring to the next and rods ([`get_line_configuration`],
//! [`get_line_forces`]). The Scheme face is `simple-spacer-scheme.cc`'s.
//!
//! The chain is stretched or compressed by one force until it is as long
//! as the line; a spring that the force would make shorter than its
//! minimum distance blocks at it. A rod makes the springs it spans block
//! at the force that makes them as long as the rod, or, over springs that
//! cannot move, lengthens them.
//!
//! LilyPond's programming errors are warn.cc's, at no location: the
//! spacer's pushed to [`Errors`], the columns' reported as they arise.

use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, Heap, Tag, Thrown, Value};

use crate::R;
use crate::geometry::{Direction, Interval, X, max};
use crate::grob;
use crate::grob_array;
use crate::item::{self, Class};
use crate::paper_column;
use crate::spaceable;
use crate::spring::{Errors, Spring, SpringObject, reported};

/// A `Simple_spacer`: its springs, in order.
#[derive(Clone, Debug, Default)]
pub struct SimpleSpacer {
    pub springs: Vec<Spring>,
}

/// `Simple_spacer::Solution`: the force, and whether the line fits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solution {
    pub force: f64,
    pub fits: bool,
}

impl SimpleSpacer {
    /// `add_spring`.
    pub fn add_spring(&mut self, sp: Spring) {
        self.springs.push(sp);
    }

    /// `rod_force`: the force that makes the springs `left..right` as long
    /// as `dist`, infinite if they cannot move that way.
    fn rod_force(&self, left: usize, right: usize, dist: f64, errors: &mut Errors) -> f64 {
        let ideal_length = self.range_ideal_len(left, right);
        let stiffness = self.range_stiffness(left, right, dist > ideal_length);
        if stiffness.is_infinite() {
            return stiffness;
        }
        self.range_solve(left, right, dist, false, errors).force
    }

    /// `add_rod`: the springs `left..right` kept at least `dist` long
    /// together: each blocks at the force that makes them so, or, if they
    /// cannot move that way, their ideal distances are scaled up to it
    /// (shared evenly if all are 0). A rod shorter than the springs at
    /// their shortest changes nothing; a rod that is not finite is
    /// LilyPond's programming error. A rod from a spring to itself or back, which LilyPond's loops
    /// do not enter, and one past the last spring, which they read past
    /// the vector for, change nothing.
    pub fn add_rod(&mut self, left: usize, right: usize, dist: f64, errors: &mut Errors) {
        if !dist.is_finite() {
            errors.push("ignoring weird minimum distance");
            return;
        }
        if left >= right || right > self.springs.len() {
            return;
        }
        if self.range_len(left, right, f64::NEG_INFINITY, errors) > dist {
            return;
        }
        let block_force = self.rod_force(left, right, dist, errors);
        if block_force.is_infinite() {
            let spring_dist = self.range_ideal_len(left, right);
            if spring_dist < dist {
                let factor = if spring_dist != 0.0 {
                    dist / spring_dist
                } else {
                    dist / (right - left) as f64
                };
                for s in &mut self.springs[left..right] {
                    if spring_dist > 0.0 {
                        s.set_ideal_distance(s.ideal_distance * factor, errors);
                    } else {
                        s.set_ideal_distance(factor, errors);
                    }
                }
            }
            return;
        }
        for s in &mut self.springs[left..right] {
            s.set_blocking_force(max(block_force, s.blocking_force), errors);
        }
    }

    /// `range_len`: the length of the springs `left..right` under `force`.
    fn range_len(&self, left: usize, right: usize, force: f64, errors: &mut Errors) -> f64 {
        let mut d = 0.0;
        for s in &self.springs[left..right] {
            d += s.length(force, errors);
        }
        d
    }

    /// `configuration_length`: the chain's length under `force`.
    pub fn configuration_length(&self, force: f64, errors: &mut Errors) -> f64 {
        self.range_len(0, self.springs.len(), force, errors)
    }

    fn range_ideal_len(&self, left: usize, right: usize) -> f64 {
        let mut d = 0.0;
        for s in &self.springs[left..right] {
            d += s.ideal_distance;
        }
        d
    }

    /// `range_stiffness`: the springs' strength together, stretching or
    /// compressing; infinite if none can move that way.
    fn range_stiffness(&self, left: usize, right: usize, stretch: bool) -> f64 {
        let mut den = 0.0;
        for s in &self.springs[left..right] {
            den += if stretch {
                s.inverse_stretch_strength
            } else {
                s.inverse_compress_strength
            };
        }
        1.0 / den
    }

    fn range_max_block_force(&self, left: usize, right: usize) -> f64 {
        let mut result = 0.0;
        for s in &self.springs[left..right] {
            result = max(result, s.blocking_force);
        }
        result
    }

    /// `solve`: the force that makes the chain `line_len` long; with
    /// `ragged`, a line that must be compressed does not fit.
    pub fn solve(&self, line_len: f64, ragged: bool, errors: &mut Errors) -> Solution {
        self.range_solve(0, self.springs.len(), line_len, ragged, errors)
    }

    fn range_solve(
        &self,
        left: usize,
        right: usize,
        line_len: f64,
        ragged: bool,
        errors: &mut Errors,
    ) -> Solution {
        let max_block_force = self.range_max_block_force(left, right);
        let max_block_force_len = self.range_len(left, right, max_block_force, errors);
        let mut sol = if max_block_force_len < line_len {
            self.expand_line(left, right, line_len, max_block_force_len, max_block_force)
        } else if max_block_force_len > line_len {
            self.compress_line(
                left,
                right,
                line_len,
                max_block_force_len,
                max_block_force,
                errors,
            )
        } else {
            Solution {
                force: max_block_force,
                fits: true,
            }
        };
        if ragged && sol.force < 0.0 {
            sol.fits = false;
        }
        sol
    }

    /// `expand_line`: every spring stretched, a very large force for
    /// springs that cannot stretch.
    fn expand_line(
        &self,
        left: usize,
        right: usize,
        line_len: f64,
        max_block_force_len: f64,
        max_block_force: f64,
    ) -> Solution {
        let mut inv_hooke = 0.0;
        for s in &self.springs[left..right] {
            inv_hooke += s.inverse_stretch_strength;
        }
        if inv_hooke == 0.0 {
            inv_hooke = 1e-6;
        }
        Solution {
            force: (line_len - max_block_force_len) / inv_hooke + max_block_force,
            fits: true,
        }
    }

    /// `compress_line`: the force lowered from the highest blocking force,
    /// the springs blocking in turn (by their blocking forces, highest
    /// first, sorted as libc++'s `std::sort` sorts them), until the chain
    /// is short enough; it does not fit if every spring blocks first.
    fn compress_line(
        &self,
        left: usize,
        right: usize,
        line_len: f64,
        max_block_force_len: f64,
        max_block_force: f64,
        errors: &mut Errors,
    ) -> Solution {
        let neutral_length = self.range_len(left, right, 0.0, errors);
        let compressed = neutral_length > line_len;
        let mut cur = Solution {
            force: if compressed { 0.0 } else { max_block_force },
            fits: true,
        };
        let mut cur_len = if compressed {
            neutral_length
        } else {
            max_block_force_len
        };
        let mut sorted: Vec<&Spring> = self.springs[left..right].iter().collect();
        lily_engine_skyline::sort(&mut sorted, &|a: &&Spring, b: &&Spring| {
            a.blocking_force > b.blocking_force
        });
        let flexibility = |s: &Spring| {
            if compressed {
                s.inverse_compress_strength
            } else {
                s.inverse_stretch_strength
            }
        };
        let mut inv_hooke = 0.0;
        let mut i = sorted.len();
        while i > 0 && sorted[i - 1].blocking_force < cur.force {
            inv_hooke += flexibility(sorted[i - 1]);
            i -= 1;
        }
        for sp in &sorted[i..] {
            if sp.blocking_force.is_infinite() {
                break;
            }
            let block_dist = (cur.force - sp.blocking_force) * inv_hooke;
            if cur_len - block_dist < line_len {
                cur.force += (line_len - cur_len) / inv_hooke;
                return cur;
            }
            cur_len -= block_dist;
            inv_hooke -= flexibility(sp);
            cur.force = sp.blocking_force;
        }
        cur.fits = false;
        cur
    }

    /// `spring_positions`: 0, then where each spring ends under `force`;
    /// with `ragged`, a stretching force is taken as 0.
    pub fn spring_positions(&self, force: f64, ragged: bool, errors: &mut Errors) -> Vec<f64> {
        let f = if ragged && force > 0.0 { 0.0 } else { force };
        let mut ret = Vec::with_capacity(self.springs.len() + 1);
        ret.push(0.0);
        for s in &self.springs {
            let next = ret[ret.len() - 1] + s.length(f, errors);
            ret.push(next);
        }
        ret
    }

    /// `force_penalty`: how bad the line is under `force`: the force, a
    /// compression penalised more as its fourth power; when `ragged`, the
    /// room left at the end of the line.
    pub fn force_penalty(
        &self,
        line_len: f64,
        force: f64,
        ragged: bool,
        errors: &mut Errors,
    ) -> f64 {
        if ragged {
            return max(0.0, line_len - self.configuration_length(0.0, errors));
        }
        let f = force;
        f - if f < 0.0 { f * f * f * f * 2.0 } else { 0.0 }
    }
}

/// `Column_x_positions`: a line's columns, its loose ones, where the
/// columns are, the line's force and whether it satisfies its rods and
/// breaks.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnXPositions {
    pub cols: Vec<Value>,
    pub loose_cols: Vec<Value>,
    pub config: Vec<f64>,
    pub force: f64,
    pub satisfies_constraints: bool,
}

impl Default for ColumnXPositions {
    fn default() -> ColumnXPositions {
        ColumnXPositions {
            cols: Vec::new(),
            loose_cols: Vec::new(),
            config: Vec::new(),
            force: 0.0,
            satisfies_constraints: true,
        }
    }
}

/// `Rod_description`: a rod to the column `right`, by its index.
#[derive(Clone, Copy, Debug, Default)]
struct RodDescription {
    right: usize,
    dist: f64,
}

/// `Column_description`: a column's spring to the next spaceable column
/// and to the next column's left piece (`end_spring`), its rods, those to
/// a left piece apart (`end_rods`), its `line-break-permission` and, if it
/// keeps inside the line, its extent.
#[derive(Clone, Debug)]
struct ColumnDescription {
    rods: Vec<RodDescription>,
    end_rods: Vec<RodDescription>,
    spring: Spring,
    end_spring: Spring,
    break_permission: Value,
    keep_inside_line: Interval,
}

impl Default for ColumnDescription {
    fn default() -> ColumnDescription {
        ColumnDescription {
            rods: Vec::new(),
            end_rods: Vec::new(),
            spring: Spring::default(),
            end_spring: Spring::default(),
            break_permission: Value::NULL,
            keep_inside_line: Interval::EMPTY,
        }
    }
}

/// Each of `errors`, LilyPond's programming error at no location.
pub(crate) fn report(host: &mut dyn Host, errors: &mut Errors) -> Result<(), Exception> {
    for text in errors.drain(..) {
        grob::unlocated_programming_error(host, text)?;
    }
    Ok(())
}

/// `is_loose`: whether the column's `between-cols` is a pair.
fn is_loose(host: &mut dyn Host, g: Value) -> Result<bool, Exception> {
    let sym = host.heap().intern("between-cols");
    let v = grob_array::object(host, g, sym)?;
    Ok(v.tag() == Tag::Pair)
}

/// `maybe_find_prebroken_piece`: the column's broken copy at `d`, else
/// the column.
fn maybe_find_prebroken_piece(host: &mut dyn Host, col: Value, d: Direction) -> Value {
    item::find_prebroken_piece(host.heap(), col, d).unwrap_or(col)
}

/// `next_spaceable_column`: the first column after `starting` that is not
/// loose.
fn next_spaceable_column(
    host: &mut dyn Host,
    list: &[Value],
    starting: usize,
) -> Result<Option<Value>, Exception> {
    for &c in &list[starting + 1..] {
        if !is_loose(host, c)? {
            return Ok(Some(c));
        }
    }
    Ok(None)
}

/// libc++'s `std::lower_bound` of `cols[first..]` by `Paper_column::rank_less`:
/// the first column from `first` whose rank is not below `rank`.
fn lower_bound_rank(heap: &Heap, cols: &[Value], first: usize, rank: u64) -> usize {
    let mut first = first;
    let mut len = cols.len() - first;
    while len != 0 {
        let l2 = len / 2;
        let m = first + l2;
        if paper_column::rank(heap, cols[m]) < rank {
            first = m + 1;
            len -= l2 + 1;
        } else {
            len = l2;
        }
    }
    first
}

/// `get_column_description`: the column `cols[col_index]`'s spring to
/// the next spaceable column and to the next column's left piece, its rods
/// to the columns of `cols` from it on (to one of their left pieces apart),
/// its `line-break-permission` and, unless it starts the line, its X
/// extent if it keeps inside the line. A line's first column is its right
/// piece, if it has one. A rod to what is not a paper column is
/// LilyPond's programming error.
fn get_column_description(
    host: &mut dyn Host,
    cols: &[Value],
    col_index: usize,
    line_starter: bool,
) -> Result<ColumnDescription, Exception> {
    let mut col = cols[col_index];
    if line_starter {
        col = maybe_find_prebroken_piece(host, col, 1);
    }
    let mut description = ColumnDescription::default();
    if let Some(next_col) = next_spaceable_column(host, cols, col_index)? {
        description.spring = spaceable::get_spring(host, col, next_col)?;
    }
    if col_index + 1 < cols.len()
        && let Some(end_col) = item::find_prebroken_piece(host.heap(), cols[col_index + 1], -1)
    {
        description.end_spring = spaceable::get_spring(host, col, end_col)?;
    }
    let mut s = spaceable::get_minimum_distances(host, col)?;
    while let (Some(entry), Some(rest)) = (host.heap().car(s), host.heap().cdr(s)) {
        let other = host.heap().car(entry).unwrap_or(Value::NULL);
        if item::class(host.heap(), other) == Some(Class::PaperColumn) {
            let heap = host.heap();
            let rank = paper_column::rank(heap, other);
            let j = lower_bound_rank(heap, cols, col_index, rank);
            if j != cols.len() && paper_column::rank(heap, cols[j]) == rank {
                let dist = heap.cdr(entry).and_then(|d| heap.to_f64(d)).unwrap_or(0.0);
                let rod = RodDescription { right: j, dist };
                if cols[j] == other {
                    description.rods.push(rod);
                } else {
                    description.end_rods.push(rod);
                }
            }
        } else {
            grob::unlocated_programming_error(
                host,
                "minimum-distances holds an object that is not a paper column",
            )?;
        }
        s = rest;
    }
    if !line_starter && grob::property(host, col, "keep-inside-line")? == Value::TRUE {
        description.keep_inside_line = grob::extent(host, col, col, X)?;
    }
    description.break_permission = grob::property(host, col, "line-break-permission")?;
    Ok(description)
}

/// `get_line_forces`: the force penalty of each line from a breakable
/// column of `columns` to a later one (`start * breaks + end`), the first
/// line `indent` shorter; infinite where it does not fit, -200000 for a
/// line between two neighbouring breaks that does not fit. A line stops
/// growing at one that does not fit or ends where a break is forced.
pub fn get_line_forces(
    host: &mut dyn Host,
    columns: &[Value],
    line_len: f64,
    indent: f64,
    ragged: bool,
) -> Result<Vec<f64>, Exception> {
    let force_break = host.heap().intern("force");
    let mut non_loose = Vec::new();
    for &c in columns {
        if !is_loose(host, c)? || paper_column::is_breakable(host, c)? {
            non_loose.push(c);
        }
    }
    let mut breaks = Vec::new();
    let mut cols = Vec::new();
    breaks.push(0);
    cols.push(ColumnDescription::default());
    let mut i = 1;
    while i + 1 < non_loose.len() {
        if paper_column::is_breakable(host, non_loose[i])? {
            breaks.push(cols.len());
        }
        cols.push(get_column_description(host, &non_loose, i, false)?);
        i += 1;
    }
    breaks.push(cols.len());
    let mut force = alloc::vec![f64::INFINITY; breaks.len() * breaks.len()];
    let mut errors = Errors::new();
    for b in 0..breaks.len() - 1 {
        cols[breaks[b]] = get_column_description(host, &non_loose, breaks[b], true)?;
        let st = breaks[b];
        for c in b + 1..breaks.len() {
            let end = breaks[c];
            let mut spacer = SimpleSpacer::default();
            for col in &cols[breaks[b]..end - 1] {
                spacer.add_spring(col.spring);
            }
            spacer.add_spring(cols[end - 1].end_spring);
            for (i, col) in cols.iter().enumerate().take(end).skip(st) {
                for rod in &col.rods {
                    if rod.right < end {
                        spacer.add_rod(i - st, rod.right - st, rod.dist, &mut errors);
                    }
                }
                for rod in &col.end_rods {
                    if rod.right == end {
                        spacer.add_rod(i - st, end - st, rod.dist, &mut errors);
                    }
                }
                let keep = col.keep_inside_line;
                if !keep.is_empty() {
                    spacer.add_rod(i - st, end - st, keep.right, &mut errors);
                    spacer.add_rod(0, i - st, -keep.left, &mut errors);
                }
                report(host, &mut errors)?;
            }
            let len = if b == 0 { line_len - indent } else { line_len };
            let sol = spacer.solve(len, ragged, &mut errors);
            force[b * breaks.len() + c] =
                spacer.force_penalty(line_len, sol.force, ragged, &mut errors);
            report(host, &mut errors)?;
            if !sol.fits {
                force[b * breaks.len() + c] = if c == b + 1 { -200000.0 } else { f64::INFINITY };
                break;
            }
            if end < cols.len() && cols[end].break_permission == force_break {
                break;
            }
        }
    }
    Ok(force)
}

/// `get_line_configuration`: the line of `columns`, from the first's
/// right piece to the last's left piece, its loose columns apart, spaced
/// by its columns' springs and rods for `line_len`: the columns'
/// positions from `indent`, the force's penalty, and whether the rods fit
/// and no break inside the line is forced. An end column without its
/// piece, which LilyPond takes as a null column, is `'()`.
pub fn get_line_configuration(
    host: &mut dyn Host,
    columns: &[Value],
    line_len: f64,
    indent: f64,
    ragged: bool,
) -> Result<ColumnXPositions, Exception> {
    let mut ret = ColumnXPositions::default();
    let first = item::find_prebroken_piece(host.heap(), columns[0], 1);
    ret.cols.push(first.unwrap_or(Value::NULL));
    for &c in columns.iter().take(columns.len().saturating_sub(1)).skip(1) {
        if is_loose(host, c)? {
            ret.loose_cols.push(c);
        } else {
            ret.cols.push(c);
        }
    }
    let last = item::find_prebroken_piece(host.heap(), columns[columns.len() - 1], -1);
    ret.cols.push(last.unwrap_or(Value::NULL));

    let mut cols = Vec::new();
    let mut spacer = SimpleSpacer::default();
    for i in 0..ret.cols.len() - 1 {
        cols.push(get_column_description(host, &ret.cols, i, i == 0)?);
        spacer.add_spring(cols[i].spring);
    }
    let mut errors = Errors::new();
    for (i, col) in cols.iter().enumerate() {
        for rod in &col.rods {
            spacer.add_rod(i, rod.right, rod.dist, &mut errors);
        }
        let keep = col.keep_inside_line;
        if !keep.is_empty() {
            spacer.add_rod(i, cols.len(), keep.right, &mut errors);
            spacer.add_rod(0, i, -keep.left, &mut errors);
        }
        report(host, &mut errors)?;
    }
    let sol = spacer.solve(line_len, ragged, &mut errors);
    ret.force = spacer.force_penalty(line_len, sol.force, ragged, &mut errors);
    ret.config = spacer.spring_positions(sol.force, ragged, &mut errors);
    report(host, &mut errors)?;
    for x in &mut ret.config {
        *x += indent;
    }
    ret.satisfies_constraints = sol.fits;
    let force_break = host.heap().intern("force");
    for &c in &ret.cols[1..ret.cols.len() - 1] {
        if grob::property(host, c, "line-break-permission")? == force_break {
            ret.satisfies_constraints = false;
        }
    }
    Ok(ret)
}

/// `(ly:solve-spring-rod-problem springs rods length ragged)`
/// (`simple-spacer-scheme.cc`): springs `(ideal inverse-hooke)` from 0,
/// each with that inverse strength both ways, and rods
/// `(left right distance)` solved for `length`, as
/// `(force position …)`, the force `#f` if the line does not fit. No
/// springs give `(0.0 0.0)`, whatever the rest is; `ragged` is only `#t`
/// (`from_scm<bool>`). A rod that is not a list of three, which LilyPond
/// reads unchecked and crashes on, throws `car`'s error; a rod past the
/// last spring, which LilyPond reads past its springs for, changes
/// nothing ([`SimpleSpacer::add_rod`]). The programming errors of springs
/// read before a later one or a rod throws are not reported, where
/// LilyPond prints them first: a primitive reports through a tail call,
/// which its throw drops.
pub(crate) fn ly_solve_spring_rod_problem(cx: &mut Cx, args: &[Value]) -> R {
    const FUNCTION: &str = "ly_solve_spring_rod_problem";
    let springs = match list_items(cx, args[0]) {
        Some(springs) if springs.is_empty() => {
            let zero = cx.heap_mut().flonum(0.0);
            return Ok(cx.heap_mut().list(&[zero, zero]));
        }
        Some(springs) => springs,
        None => return Err(cx.wrong_type(FUNCTION, 1, args[0], "list of springs")),
    };
    let rods = match list_items(cx, args[1]) {
        Some(rods) if !rods.is_empty() => rods,
        _ => return Err(cx.wrong_type(FUNCTION, 1, args[1], "list of rods")),
    };
    let line_len = crate::number_arg(cx, "ly:solve-spring-rod-problem", 3, args[2])?;
    let ragged = args[3] == Value::TRUE;
    let mut errors = Errors::new();
    let spacer = spring_rod_spacer(cx, &springs, &rods, &mut errors)?;
    let sol = spacer.solve(line_len, ragged, &mut errors);
    let positions = spacer.spring_positions(sol.force, ragged, &mut errors);
    let heap = cx.heap_mut();
    let mut out = Vec::with_capacity(positions.len() + 1);
    out.push(if sol.fits {
        heap.flonum(sol.force)
    } else {
        Value::FALSE
    });
    out.extend(positions.into_iter().map(|p| heap.flonum(p)));
    let v = heap.list(&out);
    reported(cx, v, &errors)
}

/// The `Simple_spacer` of `ly:solve-spring-rod-problem`'s springs and
/// rods, read in LilyPond's order, with Guile's errors.
fn spring_rod_spacer(
    cx: &mut Cx,
    springs: &[Value],
    rods: &[Value],
    errors: &mut Errors,
) -> Result<SimpleSpacer, Thrown> {
    let mut spacer = SimpleSpacer::default();
    for &s in springs {
        let ideal = car(cx, "caar", s)?;
        let ideal = crate::to_double(cx, ideal)?;
        let cdar = cdr(cx, "cadar", s)?;
        let inv_hooke = car(cx, "cadar", cdar)?;
        let inv_hooke = crate::to_double(cx, inv_hooke)?;
        let mut sp = Spring::new(ideal, 0.0, errors);
        sp.set_inverse_compress_strength(inv_hooke, errors);
        sp.set_inverse_stretch_strength(inv_hooke, errors);
        spacer.add_spring(sp);
    }
    for &entry in rods {
        let l = car(cx, "car", entry)?;
        let l = vsize(cx, l)?;
        let cdr_entry = cdr(cx, "cadr", entry)?;
        let r = car(cx, "cadr", cdr_entry)?;
        let r = vsize(cx, r)?;
        let cddr = cdr(cx, "cddr", cdr_entry)?;
        let distance = car(cx, "car", cddr)?;
        let distance = crate::to_double(cx, distance)?;
        spacer.add_rod(l, r, distance, errors);
    }
    Ok(spacer)
}

/// The car of `v`, else Guile's error for `subr` (`scm_caar` and the
/// others name themselves).
fn car(cx: &mut Cx, subr: &str, v: Value) -> Result<Value, Thrown> {
    match cx.heap().car(v) {
        Some(x) => Ok(x),
        None => Err(cx.wrong_type_vm(Some(subr), v, "pair")),
    }
}

/// The cdr of `v`, else Guile's error for `subr`.
fn cdr(cx: &mut Cx, subr: &str, v: Value) -> Result<Value, Thrown> {
    match cx.heap().cdr(v) {
        Some(x) => Ok(x),
        None => Err(cx.wrong_type_vm(Some(subr), v, "pair")),
    }
}

/// `from_scm<vsize>` (`scm_to_ulong`): an exact integer from 0, else
/// Guile's error, from no procedure. One of 2⁶³ and more is out of range
/// here, where LilyPond takes it up to 2⁶⁴ − 1; one past `usize` is
/// `usize::MAX`, past any spring either way.
pub(crate) fn vsize(cx: &mut Cx, v: Value) -> Result<usize, Thrown> {
    match cx.heap().to_i64(v) {
        Some(n @ 0..) => Ok(usize::try_from(n).unwrap_or(usize::MAX)),
        Some(_) => Err(cx.out_of_bounds(0, u64::MAX, v)),
        None if cx.heap().is_exact_integer(v) => Err(cx.out_of_bounds(0, u64::MAX, v)),
        None => Err(cx.wrong_type_vm(None, v, "exact integer")),
    }
}

/// The items of `list` if it is a proper list (`scm_ilength` from 0);
/// none for an improper or circular one.
fn list_items(cx: &Cx, list: Value) -> Option<Vec<Value>> {
    let heap = cx.heap();
    let mut items = Vec::new();
    let (mut rest, mut slow) = (list, list);
    while let (Some(item), Some(next)) = (heap.car(rest), heap.cdr(rest)) {
        items.push(item);
        rest = next;
        if items.len() % 2 == 0 {
            slow = heap.cdr(slow).unwrap_or(Value::NULL);
            if slow == rest {
                return None;
            }
        }
    }
    (rest == Value::NULL).then_some(items)
}

/// `(%spacer-solve springs line-len ragged [fixed-force])`: a
/// `Simple_spacer` of the list `springs` solved for `line-len` (`solve`),
/// as `(force fits positions length)`: the force, whether it fits, the
/// `spring_positions` under the force and the `configuration_length`
/// under it. With a `fixed-force` that is finite and `ragged`, the
/// fixed-force branch of `Page_layout_problem::solve_rod_spring_problem`
/// (`page-layout-problem.cc:788–798`): the springs solved as not ragged,
/// which stretches them, the force `fixed-force` if they are no longer
/// than `line-len` under it, and the positions not ragged.
pub(crate) fn solve(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "%spacer-solve";
    let mut spacer = SimpleSpacer::default();
    let mut l = args[0];
    while let (Some(s), Some(rest)) = (cx.heap().car(l), cx.heap().cdr(l)) {
        let Some(spring) = cx.heap().foreign::<SpringObject>(s).map(|s| s.0) else {
            return Err(cx.wrong_type(SUBR, 1, args[0], "list of springs"));
        };
        spacer.springs.push(spring);
        l = rest;
    }
    let line_len = cx.heap().to_f64(args[1]).unwrap_or(0.0);
    let ragged = args[2] != Value::FALSE;
    let fixed = args.get(3).and_then(|&f| cx.heap().to_f64(f));
    let mut errors = Errors::new();
    let (sol, ragged) = match fixed {
        Some(fixed) if ragged && !fixed.is_infinite() => {
            let mut sol = spacer.solve(line_len, false, &mut errors);
            if spacer.configuration_length(fixed, &mut errors) <= line_len {
                sol.force = fixed;
            }
            (sol, false)
        }
        _ => (spacer.solve(line_len, ragged, &mut errors), ragged),
    };
    let positions = spacer.spring_positions(sol.force, ragged, &mut errors);
    let length = spacer.configuration_length(sol.force, &mut errors);
    let heap = cx.heap_mut();
    let positions: Vec<Value> = positions.into_iter().map(|p| heap.flonum(p)).collect();
    let positions = heap.list(&positions);
    let (force, length) = (heap.flonum(sol.force), heap.flonum(length));
    let v = heap.list(&[force, Value::bool(sol.fits), positions, length]);
    reported(cx, v, &errors)
}
