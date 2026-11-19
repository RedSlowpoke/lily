//! `spacing-spanner.cc` (part 3's, MS3 step 31): the spacing spanner's
//! springs and rods between all the columns it spans. Its two callbacks,
//! `ly:spacing-spanner::set-springs` (`spacing-spanner.cc:60–80`, 21
//! lines) and `::calc-common-shortest-duration` (`:93–169`, 77), are
//! Scheme ([`PRELUDE`]) that reaches the port through `%host-call`
//! (D127, [`HOSTS`]); under them, in Rust, `get_columns` (`:41–58`, 18),
//! `generate_pair_spacing` (`:171–221`, 51), `set_column_rods`
//! (`:223–290`, 68), `generate_springs` (`:292–310`, 19),
//! `musical_column_spacing` (`:315–439`, 125), `fills_measure`
//! (`:444–473`, 30) and `breakable_column_spacing` (`:478–539`, 62). The
//! springs come from `spacing_basic.rs`, `note_spacing.rs` and
//! `staff_spacing.rs`, the loose columns from
//! `spacing_determine_loose_columns.rs` and the rods from
//! `separation_item.rs`' `set_distance`.
//!
//! The arithmetic is sums, `std::max` and `std::min`, and `Spring`'s
//! `*= 0.8`: nothing to fuse. Moments that do not fit, where LilyPond's
//! would wrap, are `numerical-overflow` (D61). Where LilyPond reads
//! through a null: no columns (a bound that is no paper column, or no
//! system at the top of the Y parents) make no springs and no rods, and
//! a column with no root system does not fill its measure. LilyPond's
//! `assert` that a staff spacing is in its left column is not in its
//! release build, nor here. A right item in no column matches a right
//! column that is not a broken piece (`original` and `get_column` both
//! null), as in LilyPond.

use alloc::vec::Vec;
use core::cmp::Ordering;

use lily_engine_music::{Host, Moment, Rational};
use lily_engine_scheme::{Exception, HostHandler, Value, Vm};

use crate::geometry::{Direction, X, max, min};
use crate::grob;
use crate::grob_array;
use crate::item::{self, Class};
use crate::note_spacing;
use crate::paper_column;
use crate::paper_column_engraver::rational;
use crate::separation_item;
use crate::simple_spacer::report;
use crate::spaceable;
use crate::spacing_basic;
use crate::spacing_determine_loose_columns::{
    Pruning, is_true, set_explicit_neighbors, set_implicit_neighbors,
};
use crate::spacing_interface;
use crate::spacing_options::{SpacingOptions, double_or};
use crate::spanner;
use crate::spring::{Errors, merge_springs};
use crate::staff_spacing;
use crate::system;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// `std::min` of two rationals: `b` only when `b < a`.
fn min_rational(a: Rational, b: Rational) -> Rational {
    if b.compare(a) == Ordering::Less { b } else { a }
}

/// Whether `g` is a paper column (`dynamic_cast<Paper_column *>`).
fn is_paper_column(host: &mut dyn Host, g: Value) -> bool {
    item::class(host.heap(), g) == Some(Class::PaperColumn)
}

/// `get_columns`: the used columns of `me`'s root system from its left
/// bound's rank to its right bound's ([`system::used_columns_in_range`]);
/// none, with LilyPond's programming error, if a bound is no paper
/// column.
pub fn get_columns(host: &mut dyn Host, me: Value) -> Result<Vec<Value>, Exception> {
    let l_bound = spanner::get_bound(host, me, LEFT)?.filter(|&b| is_paper_column(host, b));
    let Some(l_bound) = l_bound else {
        grob::unlocated_programming_error(host, "spanner's left bound is not a paper column")?;
        return Ok(Vec::new());
    };
    let r_bound = spanner::get_bound(host, me, RIGHT)?.filter(|&b| is_paper_column(host, b));
    let Some(r_bound) = r_bound else {
        grob::unlocated_programming_error(host, "spanner's right bound is not a paper column")?;
        return Ok(Vec::new());
    };
    let Some(sys) = system::get_root_system(host.heap(), me) else {
        return Ok(Vec::new());
    };
    let heap = host.heap();
    let start = paper_column::rank(heap, l_bound) as usize;
    let end = (paper_column::rank(heap, r_bound) as usize).wrapping_add(1);
    system::used_columns_in_range(host, sys, start, end)
}

/// `set_springs`: the spacing options of `me`, its columns' neighbours,
/// its loose columns pruned, and the springs and rods between the rest.
pub fn set_springs(host: &mut dyn Host, me: Value) -> Result<(), Exception> {
    let mut spacing = Spacing::begin(host, me)?;
    while !spacing.on(host, &|| false)? {}
    Ok(())
}

/// [`set_springs`] under way (B23, D261): a score's columns are one read, and
/// this is that read column by column, its frames in the struct, so that a
/// compile in steps returns inside it (`system.rs`' [`crate::system::Processing`]).
/// A read in a Scheme frame runs it whole.
pub struct Spacing {
    me: Value,
    options: SpacingOptions,
    cols: Vec<Value>,
    stage: Stage,
    /// The next column.
    i: usize,
}

/// What [`Spacing`] is doing.
enum Stage {
    Explicit,
    Pruning(Pruning),
    Implicit,
    /// The springs, from the column before.
    Springs(Value),
    Rods(Rods),
}

/// [`set_column_rods`]' lists, column by column.
struct Rods {
    padding: f64,
    distances: Vec<f64>,
    overhangs: Vec<f64>,
}

impl Spacing {
    /// The read of `me`'s columns begun.
    pub fn begin(host: &mut dyn Host, me: Value) -> Result<Spacing, Exception> {
        let mut options = SpacingOptions::default();
        options.init_from_grob(host, me)?;
        let cols = get_columns(host, me)?;
        Ok(Spacing {
            me,
            options,
            cols,
            stage: Stage::Explicit,
            i: 0,
        })
    }

    /// The read's next stage, from its first column.
    fn stage(&mut self, stage: Stage) {
        self.stage = stage;
        self.i = 0;
    }

    /// Goes on, a column at a time until `stop` says so; whether it is done.
    pub fn on(&mut self, host: &mut dyn Host, stop: &dyn Fn() -> bool) -> Result<bool, Exception> {
        loop {
            match &mut self.stage {
                Stage::Explicit => match self.cols.get(self.i).copied() {
                    Some(col) => {
                        set_explicit_neighbors(host, col)?;
                        self.i += 1;
                    }
                    None => {
                        self.stage(Stage::Pruning(Pruning::default()));
                        continue;
                    }
                },
                Stage::Pruning(pruning) => {
                    if pruning.done(&self.cols) {
                        if let Stage::Pruning(pruning) =
                            core::mem::replace(&mut self.stage, Stage::Implicit)
                        {
                            pruning.end(&mut self.cols);
                        }
                        self.i = 0;
                        continue;
                    }
                    pruning.next(host, self.me, &mut self.cols, &self.options)?;
                }
                Stage::Implicit => {
                    if self.i >= self.cols.len() {
                        let Some(&first) = self.cols.first() else {
                            return Ok(true);
                        };
                        self.stage(Stage::Springs(first));
                        self.i = 1;
                        continue;
                    }
                    set_implicit_neighbors(host, &self.cols, self.i)?;
                    self.i += 1;
                }
                // `generate_springs`: the springs between each pair of
                // columns ([`generate_pair_spacing`]), then their rods, at the
                // last column's `padding` (0.1 if no number). None of either
                // without columns.
                Stage::Springs(prev) => {
                    let prev = *prev;
                    match self.cols.get(self.i).copied() {
                        Some(col) => {
                            let next = self.cols.get(self.i + 1).copied();
                            generate_pair_spacing(host, self.me, prev, col, next, &self.options)?;
                            self.stage = Stage::Springs(col);
                            self.i += 1;
                        }
                        None => {
                            let v = grob::property(host, prev, "padding")?;
                            let padding = double_or(host, v, 0.1);
                            let n = self.cols.len();
                            self.stage(Stage::Rods(Rods {
                                padding,
                                distances: alloc::vec![0.0; n],
                                overhangs: alloc::vec![0.0; n],
                            }));
                            continue;
                        }
                    }
                }
                Stage::Rods(rods) => {
                    if self.i >= self.cols.len() {
                        return Ok(true);
                    }
                    set_column_rods(host, &self.cols, self.i, rods)?;
                    self.i += 1;
                }
            }
            // Each column's springs and rods a step (05 §7.3, D251).
            crate::queue::check(crate::queue::Step::SpringsAndRods);
            if stop() {
                return Ok(false);
            }
        }
    }
}

/// `calc_common_shortest_duration`: of the shortest starter durations of
/// the musical columns of each measure (grace columns left out), the one
/// most measures have, the shortest of a tie; at most `me`'s
/// `base-shortest-duration` (1/8 if no moment).
pub fn calc_common_shortest_duration(host: &mut dyn Host, me: Value) -> Result<Moment, Exception> {
    let cols = get_columns(host, me)?;

    // Ascending in duration.
    let mut durations: Vec<Rational> = Vec::new();
    let mut counts: Vec<i32> = Vec::new();

    let mut shortest_in_measure = Rational::INFINITY;

    for &col in &cols {
        if paper_column::is_musical(host, col)? {
            let when = grob::property(host, col, "when")?;
            let when = host.heap().foreign::<Moment>(when).copied();

            // Grace notes are not shortest notes.
            if when.is_some_and(|w| !w.grace.is_zero()) {
                continue;
            }

            let st = grob::property(host, col, "shortest-starter-duration")?;
            let this_shortest = rational(host.heap(), st, Rational::INFINITY);
            shortest_in_measure = min_rational(shortest_in_measure, this_shortest);
        } else if !shortest_in_measure.is_inf() && paper_column::is_breakable(host, col)? {
            let mut j = 0;
            while j < durations.len() {
                match durations[j].compare(shortest_in_measure) {
                    Ordering::Greater => {
                        counts.insert(j, 1);
                        durations.insert(j, shortest_in_measure);
                        break;
                    }
                    _ if durations[j] == shortest_in_measure => {
                        counts[j] += 1;
                        break;
                    }
                    _ => j += 1,
                }
            }

            if durations.len() == j {
                durations.push(shortest_in_measure);
                counts.push(1);
            }

            shortest_in_measure = Rational::INFINITY;
        }
    }

    let mut max_idx = None;
    let mut max_count = 0;
    for i in (0..durations.len()).rev() {
        if counts[i] >= max_count {
            max_idx = Some(i);
            max_count = counts[i];
        }
    }

    let bsd = grob::property(host, me, "base-shortest-duration")?;
    let mut d = Rational::new(1, 8);
    if let Some(m) = host.heap().foreign::<Moment>(bsd) {
        d = m.main;
    }

    if let Some(i) = max_idx {
        d = min_rational(d, durations[i]);
    }

    Ok(Moment::new(d, Rational::ZERO))
}

/// `generate_pair_spacing`: the springs from `left_col` to `right_col`
/// and their broken pieces. After a musical column, to the musical
/// column after a non-musical one that floats (`strict-note-spacing` or
/// `maybe-loose`), which gets `between-cols`, else to `right_col`; and
/// to its piece broken on the left. After another, between each of the
/// columns and pieces ([`breakable_column_spacing`]).
fn generate_pair_spacing(
    host: &mut dyn Host,
    me: Value,
    left_col: Value,
    right_col: Value,
    after_right_col: Option<Value>,
    options: &SpacingOptions,
) -> Result<(), Exception> {
    if paper_column::is_musical(host, left_col)? {
        let floats = !paper_column::is_musical(host, right_col)?
            && (options.float_nonmusical_columns || is_true(host, right_col, "maybe-loose")?);
        let after = match after_right_col {
            Some(after) if floats && paper_column::is_musical(host, after)? => Some(after),
            _ => None,
        };
        if let Some(after) = after {
            // TODO in LilyPond: rods to prevent collisions.
            musical_column_spacing(host, me, left_col, after, options)?;
            let heap = host.heap();
            let between = heap.cons(left_col, after);
            let sym = heap.intern("between-cols");
            grob::set_object(heap, right_col, sym, between);
        } else {
            musical_column_spacing(host, me, left_col, right_col, options)?;
        }

        if let Some(rb) = item::find_prebroken_piece(host.heap(), right_col, LEFT) {
            musical_column_spacing(host, me, left_col, rb, options)?;
        }
    } else {
        // The right one broken too is rare, but happens (a single empty
        // measure, a staff ending a tad early).
        let lb = item::find_prebroken_piece(host.heap(), left_col, RIGHT);
        let rb = item::find_prebroken_piece(host.heap(), right_col, LEFT);

        breakable_column_spacing(host, me, left_col, right_col, options)?;

        if let Some(lb) = lb {
            breakable_column_spacing(host, me, lb, right_col, options)?;
        }

        if let Some(rb) = rb {
            breakable_column_spacing(host, me, left_col, rb, options)?;
        }

        if let (Some(lb), Some(rb)) = (lb, rb) {
            breakable_column_spacing(host, me, lb, rb, options)?;
        }
    }
    Ok(())
}

/// `set_column_rods`, of `cols[i]`: rods, `padding` apart, from each column
/// (and its piece broken on the right) to each later one (and its piece
/// broken on the left) it can reach when each column is as far left as it can
/// be: `distances[i]` from `cols[i - 1]` to `cols[i]`, `overhangs[j]` how far
/// `cols[0..=j]` reach past `cols[j]`. Columns empty, broken piece and all,
/// are skipped.
fn set_column_rods(
    host: &mut dyn Host,
    cols: &[Value],
    i: usize,
    rods: &mut Rods,
) -> Result<(), Exception> {
    let Rods {
        padding,
        distances,
        overhangs,
    } = rods;
    let padding = *padding;
    let r = cols[i];
    let rb = item::find_prebroken_piece(host.heap(), r, LEFT);

    if separation_item::is_empty(host, r)? {
        let rb_empty = match rb {
            Some(rb) => separation_item::is_empty(host, rb)?,
            None => true,
        };
        if rb_empty {
            return Ok(());
        }
    }

    let skyp = separation_item::horizontal_skylines(host, r)?;
    overhangs[i] = skyp
        .as_ref()
        .map_or(0.0, |s| s[lily_engine_skyline::Direction::Up].max_height());

    if i == 0 {
        return Ok(());
    }

    // min rather than max: the stickout is below 0 where the right
    // column sticks out a lot to the left.
    let left_height = skyp.as_ref().map_or(0.0, |s| {
        s[lily_engine_skyline::Direction::Down].max_height()
    });
    let conditional = separation_item::conditional_skyline(host, r, cols[i - 1])?.max_height();
    let stickout = min(left_height, conditional);

    let mut prev_distances = 0.0;

    // Quadratic at worst, but it goes on only while there is a rod to
    // add.
    for j in (0..i).rev() {
        if overhangs[j] + padding <= prev_distances + distances[i] + stickout {
            // cols[0..=j] cannot reach cols[i].
            break;
        }

        let l = cols[j];
        let lb = item::find_prebroken_piece(host.heap(), l, RIGHT);

        let mut dist = separation_item::set_distance(host, l, r, padding)?;
        distances[i] = max(distances[i], dist - prev_distances);

        if let Some(lb) = lb {
            dist = separation_item::set_distance(host, lb, r, padding)?;
            // The piece broken on the left may reach further right
            // than the column, by extending further or nesting more
            // closely; checked the first time each is seen.
            if j == i - 1 {
                let right = grob::extent(host, lb, lb, X)?.right;
                overhangs[j] = max(overhangs[j], right + distances[i] - dist);
            }
        }
        if let Some(rb) = rb {
            separation_item::set_distance(host, l, rb, padding)?;
        }
        if let (Some(lb), Some(rb)) = (lb, rb) {
            separation_item::set_distance(host, lb, rb, padding)?;
        }

        prev_distances += distances[j];
    }
    overhangs[i] = max(overhangs[i], overhangs[i - 1] - distances[i]);
    Ok(())
}

/// Whether the moment's grace part is not 0.
fn is_grace(m: Moment) -> bool {
    !m.grace.is_zero()
}

/// `musical_column_spacing`: the spring from the musical column
/// `left_col` to `right_col`. By the durations ([`spacing_basic::
/// note_spacing`]); stretching uniformly, with no minimum and the default
/// strengths; else merged from the note spacings among `left_col`'s
/// spacing wishes that start there and end at `right_col` (or its
/// original), each at the grace spacing's increment after a grace note,
/// and with no minimum before a musical column if there is none. 0.8 as
/// long into a grace note from a main note; in packed spacing, as long
/// as `left_col` is wide (unless a ligature left it) and at least the
/// minimum, stretching as 1. Kept on `left_col`.
fn musical_column_spacing(
    host: &mut dyn Host,
    me: Value,
    left_col: Value,
    right_col: Value,
    options: &SpacingOptions,
) -> Result<(), Exception> {
    let mut errors = Errors::new();
    let mut spring = spacing_basic::note_spacing(host, me, left_col, right_col, options)?;

    if options.stretch_uniformly {
        spring.set_min_distance(0.0, &mut errors);
        spring.set_default_strength();
        report(host, &mut errors)?;
    } else {
        let mut springs = Vec::new();
        let wishes = grob_array::extract_grob_array(host, left_col, "spacing-wishes")?;

        for wish in wishes {
            if spacing_interface::left_column(host, wish) != Some(left_col) {
                // Ancient music's spacing does this.
                continue;
            }

            let right_items = grob_array::extract_grob_array(host, wish, "right-items")?;
            let heap = host.heap();
            let original = grob::with_grob(heap, right_col, |g| g.original).flatten();
            let mut found_matching_column = false;
            for it in right_items {
                if item::is_item(heap, it) {
                    let column = item::get_column(heap, it);
                    if column == Some(right_col) || original == column {
                        found_matching_column = true;
                    }
                }
            }

            // A waste of time in polyphonic music, says LilyPond.
            if found_matching_column && grob::has_interface(heap, wish, "note-spacing-interface") {
                let mut inc = options.increment;
                let sym = heap.intern("grace-spacing");
                let gsp = grob_array::object(host, left_col, sym)?;
                if grob::with_grob(host.heap(), gsp, |_| ()).is_some()
                    && is_grace(paper_column::when_mom(host, left_col)?)
                {
                    let mut grace_opts = SpacingOptions::default();
                    grace_opts.init_from_grob(host, gsp)?;
                    inc = grace_opts.increment;
                }
                springs.push(note_spacing::get_spacing(
                    host, wish, right_col, spring, inc,
                )?);
            }
        }

        if springs.is_empty() {
            if paper_column::is_musical(host, right_col)? {
                // No spacing wishes: likely polyphonic hemiolas.
                spring.set_min_distance(0.0, &mut errors);
            }
        } else {
            spring = merge_springs(&springs, &mut errors);
        }
        report(host, &mut errors)?;
    }

    if is_grace(paper_column::when_mom(host, right_col)?)
        && !is_grace(paper_column::when_mom(host, left_col)?)
    {
        // 0.8 is arbitrary, says LilyPond.
        spring *= 0.8;
    }

    if options.packed {
        // Not the column's extent after a column a ligature engraver
        // left, whose extent includes heads that are not there.
        if paper_column::is_extraneous_column_from_ligature(host, left_col)? {
            spring.set_ideal_distance(spring.min_distance, &mut errors);
        } else {
            let right = grob::extent(host, left_col, left_col, X)?.right;
            spring.set_ideal_distance(max(right, spring.min_distance), &mut errors);
        }

        spring.set_inverse_stretch_strength(1.0, &mut errors);
        report(host, &mut errors)?;
    }

    spaceable::add_spring(host, left_col, right_col, &spring)
}

/// `fills_measure`: whether the musical column `col`, after the
/// non-musical `left`, is the only one of its measure: the next column
/// is a used, non-musical, broken (or breakable piece) one more than
/// half `left`'s `measure-length` later.
pub fn fills_measure(
    host: &mut dyn Host,
    me: Value,
    left: Value,
    col: Value,
) -> Result<bool, Exception> {
    let Some(sys) = system::get_root_system(host.heap(), me) else {
        return Ok(false);
    };
    let heap = host.heap();
    let Some(column) = item::get_column(heap, col) else {
        return Ok(false);
    };
    let rank = paper_column::rank(heap, column).wrapping_add(1);
    let Some(next) = system::column(host, sys, rank)? else {
        return Ok(false);
    };

    if paper_column::is_musical(host, next)?
        || paper_column::is_musical(host, left)?
        || !paper_column::is_musical(host, col)?
        || !paper_column::is_used(host, next)?
    {
        return Ok(false);
    }

    let next_when = paper_column::when_mom(host, next)?;
    let col_when = paper_column::when_mom(host, col)?;
    let dt = next_when
        .checked_sub(col_when)
        .ok_or_else(|| crate::overflow(host.heap(), "fills_measure"))?;

    let len = grob::property(host, left, "measure-length")?;
    let Some(len) = host.heap().foreign::<Moment>(len).copied() else {
        return Ok(false);
    };

    // Not the exact measure length: pickups shorten last measures.
    let half = len
        .main
        .checked_div(Rational::int(2))
        .ok_or_else(|| crate::overflow(host.heap(), "fills_measure"))?;
    let heap = host.heap();
    Ok(dt.main.compare(half) == Ordering::Greater
        && (item::is_broken(heap, next) || item::break_status_dir(heap, next) != 0))
}

/// `breakable_column_spacing`: the spring from `l` to `r`. At the same
/// moment, merged from the staff spacings among `l`'s spacing wishes,
/// with `l`'s `full-measure-extra-space` (1.0 if no number) when `r` is
/// a musical column that fills the measure after the unbroken `l`; else,
/// or with none, the standard one ([`spacing_basic::
/// standard_breakable_column_spacing`]). 0.8 as long into a grace note;
/// stretching uniformly, unless `l` is broken on the right, with no
/// minimum and the default strengths. Kept on `l`.
fn breakable_column_spacing(
    host: &mut dyn Host,
    me: Value,
    l: Value,
    r: Value,
    options: &SpacingOptions,
) -> Result<(), Exception> {
    let mut springs = Vec::new();
    let mut errors = Errors::new();

    let mut full_measure_space = 0.0;
    if paper_column::is_musical(host, r)?
        && item::break_status_dir(host.heap(), l) == 0
        && fills_measure(host, me, l, r)?
    {
        let v = grob::property(host, l, "full-measure-extra-space")?;
        full_measure_space = double_or(host, v, 1.0);
    }

    let rwhen = paper_column::when_mom(host, r)?;
    let lwhen = paper_column::when_mom(host, l)?;
    let dt = rwhen
        .checked_sub(lwhen)
        .ok_or_else(|| crate::overflow(host.heap(), "breakable_column_spacing"))?;

    if dt == Moment::default() {
        let wishes = grob_array::extract_grob_array(host, l, "spacing-wishes")?;

        for spacing_grob in wishes {
            let heap = host.heap();
            if !item::is_item(heap, spacing_grob)
                || !grob::has_interface(heap, spacing_grob, "staff-spacing-interface")
            {
                continue;
            }

            springs.push(staff_spacing::get_spacing(
                host,
                spacing_grob,
                r,
                full_measure_space,
            )?);
        }
    }

    let mut spring = if springs.is_empty() {
        spacing_basic::standard_breakable_column_spacing(host, me, l, r, options)?
    } else {
        let s = merge_springs(&springs, &mut errors);
        report(host, &mut errors)?;
        s
    };

    if is_grace(paper_column::when_mom(host, r)?) {
        // Correct for grace notes; 0.8 is arbitrary, says LilyPond.
        spring *= 0.8;
    }

    if options.stretch_uniformly && item::break_status_dir(host.heap(), l) != RIGHT {
        spring.set_min_distance(0.0, &mut errors);
        spring.set_default_strength();
        report(host, &mut errors)?;
    }

    spaceable::add_spring(host, l, r, &spring)
}

/// What `%host-call` runs for the spacing spanner's callbacks (D127):
/// `set_springs` and `calc_common_shortest_duration` of the spanner
/// given, with the VM as their host.
pub(crate) const HOSTS: [HostHandler; 2] = [
    HostHandler {
        what: "spacing-spanner-set-springs",
        f: host_set_springs,
    },
    HostHandler {
        what: "spacing-spanner-calc-common-shortest-duration",
        f: host_calc_common_shortest_duration,
    },
];

fn host_set_springs(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    set_springs(vm, args[1])?;
    Ok(Value::UNSPECIFIED)
}

fn host_calc_common_shortest_duration(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let d = calc_common_shortest_duration(vm, args[1])?;
    Ok(vm.heap_mut().wrap(d))
}

/// `ly:spacing-spanner::set-springs` and
/// `::calc-common-shortest-duration`: the spanner checked, where
/// LilyPond's `unsmob<Spanner>` gives a null it reads through, then the
/// port ([`HOSTS`]).
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:spacing-spanner::set-springs smob)
  (%check-type "ly:spacing-spanner::set-springs" 1 ly:spanner? "Spanner" smob)
  (%host-call 'spacing-spanner-set-springs smob))
(define-public (ly:spacing-spanner::calc-common-shortest-duration smob)
  (%check-type "ly:spacing-spanner::calc-common-shortest-duration" 1 ly:spanner? "Spanner" smob)
  (%host-call 'spacing-spanner-calc-common-shortest-duration smob))
"#;
