//! `spacing-determine-loose-columns.cc` (part 3's, MS3 step 32's file,
//! pulled forward in step 31 for the spacing spanner's `set_springs`,
//! `spacing_spanner.rs`): which columns are loose, `is_loose_column`
//! (`:43–131`, 89 lines), their rods, `set_distances_for_loose_col`
//! (`:133–180`, 48), and their pruning, `prune_loose_columns`
//! (`:182–264`, 83); and the columns' neighbours, by the spacing wishes,
//! `set_explicit_neighbor_columns` (`:266–311`, 46), and else by their
//! order, `set_implicit_neighbor_columns` (`:313–336`, 24). Rust with no
//! Scheme face: their one caller is `set_springs`.
//!
//! No float arithmetic but `std::max` and one sum. Where LilyPond reads
//! through a null, a spacing wish that is no item or in no column, and a
//! right item in no column, set no neighbours; a loose column's
//! neighbour that is no paper column gives its side no distance.
//! `from_scm<bool>` is `#t` only.

use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Heap, Value};

use crate::geometry::{X, max};
use crate::grob;
use crate::grob_array;
use crate::item::{self, Class};
use crate::note_spacing;
use crate::paper_column;
use crate::rod::Rod;
use crate::spacing_basic;
use crate::spacing_interface;
use crate::spacing_options::SpacingOptions;
use crate::staff_spacing;

/// `INT_MAX`, the rank no right item's column reaches.
const INT_MAX: u64 = i32::MAX as u64;

/// `unsmob<Paper_column>` of `v`.
pub(crate) fn paper_column_of(heap: &mut Heap, v: Value) -> Option<Value> {
    (item::class(heap, v) == Some(Class::PaperColumn)).then_some(v)
}

/// `unsmob<Grob>` of `v`.
pub(crate) fn grob_of(heap: &Heap, v: Value) -> Option<Value> {
    grob::with_grob(heap, v, |_| ()).map(|()| v)
}

/// `me`'s object `name`.
pub(crate) fn object(host: &mut dyn Host, me: Value, name: &str) -> Result<Value, Exception> {
    let sym = host.heap().intern(name);
    grob_array::object(host, me, sym)
}

/// `me`'s object `name` set to `v`.
fn set_object(heap: &mut Heap, me: Value, name: &str, v: Value) {
    let sym = heap.intern(name);
    grob::set_object(heap, me, sym, v);
}

/// `from_scm<bool>` of `me`'s property `name`: whether it is `#t`.
pub(crate) fn is_true(host: &mut dyn Host, me: Value, name: &str) -> Result<bool, Exception> {
    Ok(grob::property(host, me, name)? == Value::TRUE)
}

/// The length of `g`'s X extent relative to itself, 0 if empty.
pub(crate) fn x_length(host: &mut dyn Host, g: Value) -> Result<f64, Exception> {
    Ok(grob::extent(host, g, g, X)?.length())
}

/// `is_loose_column`: whether `col`, between `l` and `r`, is fixed to no
/// neighbour: it allows loose spacing, and is a grace column when grace
/// columns or non-musical ones float; or is not musical, has musical or
/// breakable neighbours (not `l` and `r` both if it is not empty), and no
/// staff bar group of its break alignment that is not empty.
fn is_loose_column(
    host: &mut dyn Host,
    l: Value,
    col: Value,
    r: Value,
    options: &SpacingOptions,
) -> Result<bool, Exception> {
    if !is_true(host, col, "allow-loose-spacing")? {
        return Ok(false);
    }

    if (options.float_nonmusical_columns || options.float_grace_columns)
        && !paper_column::when_mom(host, col)?.grace.is_zero()
    {
        return Ok(true);
    }

    if paper_column::is_musical(host, col)? {
        return Ok(false);
    }

    // Without a neighbour, it should be loose, but spacing it is more
    // than the spacing can do.
    let v = object(host, col, "right-neighbor")?;
    let Some(r_neighbor) = paper_column_of(host.heap(), v) else {
        return Ok(false);
    };
    let v = object(host, col, "left-neighbor")?;
    let Some(l_neighbor) = paper_column_of(host.heap(), v) else {
        return Ok(false);
    };

    // In series with its neighbours, and not empty (`\bar ""`).
    if l == l_neighbor && r == r_neighbor && x_length(host, col)? > 0.0 {
        return Ok(false);
    }

    for neighbor in [l_neighbor, r_neighbor] {
        if !paper_column::is_musical(host, neighbor)?
            && !paper_column::is_breakable(host, neighbor)?
        {
            return Ok(false);
        }
    }

    // Bar lines do not move.
    let v = object(host, col, "break-alignment")?;
    if item::is_item(host.heap(), v) {
        let staff_bar = host.heap().intern("staff-bar");
        let group = crate::call_lily(host, "%find-nonempty-break-align-group", &[v, staff_bar])?;
        if group != Value::FALSE && x_length(host, group)? > 0.0 {
            return Ok(false);
        }
    }

    Ok(true)
}

/// `set_distances_for_loose_col`: a rod between the loose column `c`'s
/// neighbours `next_door`, as long as the largest minimum distance of
/// the spacing wishes from the left one to `c` and of those from `c` to
/// the right one; LilyPond's programming error for a wish that is
/// neither note nor staff spacing.
fn set_distances_for_loose_col(
    host: &mut dyn Host,
    me: Value,
    c: Value,
    next_door: [Option<Value>; 2],
    options: &SpacingOptions,
) -> Result<(), Exception> {
    let mut dists = [0.0; 2];

    for (side, dist) in dists.iter_mut().enumerate() {
        let (lc, rc) = if side == 0 {
            (next_door[0], Some(c))
        } else {
            (Some(c), next_door[1])
        };
        let heap = host.heap();
        let lc = lc.and_then(|g| paper_column_of(heap, g));
        let rc = rc.and_then(|g| paper_column_of(heap, g));
        let Some(lc) = lc else {
            continue;
        };

        let wishes = grob_array::extract_grob_array(host, lc, "spacing-wishes")?;
        for &sp in wishes.iter().rev() {
            if spacing_interface::left_column(host, sp) != Some(lc)
                || spacing_interface::right_column(host, sp)? != rc
            {
                continue;
            }
            let Some(rc) = rc else {
                continue;
            };

            if grob::has_interface(host.heap(), sp, "note-spacing-interface") {
                // The note spacing taken from the musical columns.
                let base = spacing_basic::note_spacing(host, me, lc, rc, options)?;
                let spring = note_spacing::get_spacing(host, sp, rc, base, options.increment)?;
                *dist = max(*dist, spring.min_distance);
            } else if grob::has_interface(host.heap(), sp, "staff-spacing-interface") {
                let spring = staff_spacing::get_spacing(host, sp, rc, 0.0)?;
                *dist = max(*dist, spring.min_distance);
            } else {
                grob::unlocated_programming_error(host, "Subversive spacing wish")?;
            }
        }
    }

    let mut r = Rod {
        distance: dists[0] + dists[1],
        item_drul: next_door,
    };
    r.add_to_cols(host)
}

/// `prune_loose_columns`: the loose columns of `cols` taken out, each
/// given `between-cols`, its neighbours (else, with LilyPond's
/// programming error, the columns around it), and a rod between them. A
/// loose breakable column, and a column that is not breakable with
/// `labels` and no `elements`, stay, marked `maybe-loose`.
pub fn prune_loose_columns(
    host: &mut dyn Host,
    me: Value,
    cols: &mut Vec<Value>,
    options: &SpacingOptions,
) -> Result<(), Exception> {
    let mut pruning = Pruning::default();
    for _ in 0..cols.len() {
        pruning.next(host, me, cols, options)?;
    }
    pruning.end(cols);
    Ok(())
}

/// [`prune_loose_columns`] under way, a column at a time (B23).
#[derive(Default)]
pub struct Pruning {
    rp: usize,
    wp: usize,
    lastcol: Option<Value>,
}

impl Pruning {
    /// Whether every column of `cols` has been seen.
    pub fn done(&self, cols: &[Value]) -> bool {
        self.rp >= cols.len()
    }

    /// `cols` as what was not loose left them.
    pub fn end(self, cols: &mut Vec<Value>) {
        cols.truncate(self.wp);
    }

    /// The next column of `cols`.
    pub fn next(
        &mut self,
        host: &mut dyn Host,
        me: Value,
        cols: &mut [Value],
        options: &SpacingOptions,
    ) -> Result<(), Exception> {
        let rp = self.rp;
        self.rp += 1;
        let lastcol = self.lastcol;
        let c = cols[rp];
        let next = cols.get(rp + 1).copied();

        let mut loose = match (lastcol, next) {
            (Some(l), Some(r)) => is_loose_column(host, l, c, r, options)?,
            _ => false,
        };

        // Breakable columns stay: their broken pieces are not loose.
        if loose && paper_column::is_breakable(host, c)? {
            loose = false;
            grob::set_property(host.heap(), c, "maybe-loose", Value::TRUE);
        }
        // Nor do columns of page labels only, which the system collects.
        if !loose && !paper_column::is_breakable(host, c)? {
            let labels = grob::property(host, c, "labels")?;
            if host.heap().car(labels).is_some()
                && grob_array::extract_grob_array(host, c, "elements")?.is_empty()
            {
                grob::set_property(host.heap(), c, "maybe-loose", Value::TRUE);
            }
        }

        if loose {
            let v = object(host, c, "right-neighbor")?;
            let right_neighbor = grob_of(host.heap(), v);
            let v = object(host, c, "left-neighbor")?;
            let left_neighbor = grob_of(host.heap(), v);

            // Either may be missing if the score ends early.
            if let (Some(left), Some(right)) = (left_neighbor, right_neighbor) {
                let heap = host.heap();
                let between = heap.cons(left, right);
                set_object(heap, c, "between-cols", between);

                let next_door = [left, right].map(|g| item::is_item(heap, g).then_some(g));
                set_distances_for_loose_col(host, me, c, next_door, options)?;
            } else {
                grob::programming_error(
                    host,
                    c,
                    "Cannot determine neighbors for floating column.",
                )?;
                let (Some(l), Some(r)) = (lastcol, next) else {
                    unreachable!("a loose column has columns around it");
                };
                let heap = host.heap();
                let between = heap.cons(l, r);
                set_object(heap, c, "between-cols", between);
            }
        } else {
            cols[self.wp] = c;
            self.wp += 1;
        }

        self.lastcol = Some(c);
        Ok(())
    }
}

/// `set_explicit_neighbor_columns`: of each spacing wish of `cols`, its
/// column given the lowest of its right items' columns as
/// `right-neighbor`, and each of those given it as `left-neighbor` if it
/// has none or one of lower rank. The wishes and their right items are
/// taken last first.
pub fn set_explicit_neighbor_columns(host: &mut dyn Host, cols: &[Value]) -> Result<(), Exception> {
    for &col in cols {
        set_explicit_neighbors(host, col)?;
    }
    Ok(())
}

/// [`set_explicit_neighbor_columns`] of one column.
pub fn set_explicit_neighbors(host: &mut dyn Host, col: Value) -> Result<(), Exception> {
    let wishes = grob_array::extract_grob_array(host, col, "spacing-wishes")?;
    for &wish in wishes.iter().rev() {
        let heap = host.heap();
        let left_col = item::is_item(heap, wish)
            .then(|| item::get_column(heap, wish))
            .flatten();
        let Some(left_col) = left_col else {
            continue;
        };
        let left_rank = paper_column::rank(heap, left_col);
        let mut min_right_rank = INT_MAX;

        let right_items = grob_array::extract_grob_array(host, wish, "right-items")?;
        for &ri in right_items.iter().rev() {
            let heap = host.heap();
            let right_col = item::is_item(heap, ri)
                .then(|| item::get_column(heap, ri))
                .flatten();
            let Some(right_col) = right_col else {
                continue;
            };
            let right_rank = paper_column::rank(heap, right_col);

            if right_rank < min_right_rank {
                set_object(heap, left_col, "right-neighbor", right_col);
                min_right_rank = right_rank;
            }

            let v = object(host, right_col, "left-neighbor")?;
            let heap = host.heap();
            let old_left_neighbor = paper_column_of(heap, v);
            if old_left_neighbor.is_none_or(|old| left_rank > paper_column::rank(heap, old)) {
                set_object(heap, right_col, "left-neighbor", left_col);
            }
        }
    }
    Ok(())
}

/// `set_implicit_neighbor_columns`: each breakable or musical column of
/// `cols` without a `left-neighbor` or `right-neighbor` given the column
/// before or after it.
pub fn set_implicit_neighbor_columns(host: &mut dyn Host, cols: &[Value]) -> Result<(), Exception> {
    for i in 0..cols.len() {
        set_implicit_neighbors(host, cols, i)?;
    }
    Ok(())
}

/// [`set_implicit_neighbor_columns`] of `cols[i]`.
pub fn set_implicit_neighbors(
    host: &mut dyn Host,
    cols: &[Value],
    i: usize,
) -> Result<(), Exception> {
    let it = cols[i];
    if !paper_column::is_breakable(host, it)? && !paper_column::is_musical(host, it)? {
        return Ok(());
    }

    if i > 0 {
        let v = object(host, it, "left-neighbor")?;
        if grob_of(host.heap(), v).is_none() {
            set_object(host.heap(), it, "left-neighbor", cols[i - 1]);
        }
    }
    if i + 1 < cols.len() {
        let v = object(host, it, "right-neighbor")?;
        if grob_of(host.heap(), v).is_none() {
            set_object(host.heap(), it, "right-neighbor", cols[i + 1]);
        }
    }
    Ok(())
}
