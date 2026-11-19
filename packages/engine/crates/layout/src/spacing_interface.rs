//! `spacing-interface.cc` (part 3's): the minimum distance between two
//! columns from their separation items' skylines (MS3 step 30),
//! `skylines` (`spacing-interface.cc:33–97`, 65 lines) and
//! `minimum_distance` (`:99–105`, 7); and a spacing grob's columns (MS3
//! step 31), `right_column` (`:107–137`, 31), `left_column` (`:139–147`,
//! 9), `get_note_columns` (`:149–168`, 20), `right_note_columns` and
//! `left_note_columns` (`:170–182`, 13) and `extremal_break_aligned_grob`
//! (`:184–228`, 45). None has a Scheme face: their callers are the note
//! spacing, the staff spacing and the spacing spanner (step 31). The
//! skylines are each separation item's `horizontal-skylines`
//! (`ly:separation-item::calc-skylines`, `separation_item.rs`), and the
//! right one's `conditional_skyline`. The rods kept from the distances are
//! `rod.rs`' and `spaceable.rs`'.
//!
//! Where LilyPond reads through a null, a separation item whose
//! `horizontal-skylines` is no skyline pair has none after LilyPond's
//! programming error, and one whose elements have no common refpoint on
//! Y is not shifted; a note column that is no item is no note column; a
//! break-aligned grob that is no item is skipped, and one whose extent is
//! taken while the first is in no column is measured from its root.

use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Value};
use lily_engine_skyline::{Direction, Skyline, SkylinePair};

use crate::geometry::{self, Interval, X, Y};
use crate::grob;
use crate::grob_array;
use crate::item;
use crate::paper_column;
use crate::separation_item;
use crate::skylines::skyline_pair_in;
use crate::unpure::pure_relative_y_coordinate;

/// `INT_MAX`, the end of the pure coordinate's columns.
pub(crate) const VERY_LARGE: usize = 2_147_483_647;

/// `skylines`: the skyline bounding on the right the separation items of
/// `me`'s original's `left-items` in `me`'s column, first, and the one
/// bounding on the left those of its `right-items` in `right_col`, each
/// item taken as its piece broken as `me`, or `right_col`, is. Each
/// skyline is merged in at the pure Y coordinate of its item's elements'
/// common refpoint relative to `me`'s system, with each right item's
/// `conditional_skyline` given the first left item.
pub fn skylines(
    host: &mut dyn Host,
    me: Value,
    right_col: Value,
) -> Result<[Skyline; 2], Exception> {
    let heap = host.heap();
    let orig = grob::with_grob(heap, me, |g| g.original)
        .flatten()
        .unwrap_or(me);
    let break_dirs = [
        item::break_status_dir(heap, me),
        item::break_status_dir(heap, right_col),
    ];
    let mut skylines = [
        Skyline::empty(Direction::Up),
        Skyline::empty(Direction::Down),
    ];
    let items: [Vec<Value>; 2] = [
        grob_array::extract_grob_array(host, orig, "left-items")?,
        grob_array::extract_grob_array(host, orig, "right-items")?,
    ];
    let heap = host.heap();
    let system = item::get_system(heap, me).unwrap_or(Value::FALSE);
    let columns = [item::get_column(heap, me), Some(right_col)];

    // LEFT then RIGHT, each merging its items' skylines facing the other.
    for (s, facing) in [Direction::Up, Direction::Down].into_iter().enumerate() {
        for &it in &items[s] {
            let heap = host.heap();
            if !item::is_item(heap, it) {
                continue;
            }
            let g = item::find_prebroken_piece(heap, it, break_dirs[s]).unwrap_or(it);
            if !grob::has_interface(heap, g, "separation-item-interface")
                || item::get_column(heap, g) != columns[s]
            {
                continue;
            }
            let skyp_scm = grob::property(host, g, "horizontal-skylines")?;
            let heap = host.heap();
            let skyp = match skyline_pair_in(heap, skyp_scm) {
                Ok(Some(pair)) => pair,
                Ok(None) => {
                    grob::unlocated_programming_error(host, "separation item has no skyline")?;
                    SkylinePair::empty()
                }
                Err(message) => return Err(crate::misc_error(heap, "from_scm", message)),
            };
            let elts = grob_array::extract_grob_array(host, g, "elements")?;
            let ycommon = grob::common_refpoint_of_array(host.heap(), &elts, Some(g), Y);
            let shift = match ycommon {
                Some(c) => pure_relative_y_coordinate(host, c, system, 0, VERY_LARGE)?,
                None => 0.0,
            };

            skylines[s].shift(-shift);
            skylines[s].merge(&skyp[facing]);
            if s == 1 && !items[0].is_empty() {
                let cond = separation_item::conditional_skyline(host, it, items[0][0])?;
                skylines[s].merge(&cond);
            }
            skylines[s].shift(shift);
        }
    }
    Ok(skylines)
}

/// `minimum_distance`: how far `right`, a column, must be from `me`'s
/// column for their [`skylines`] not to meet, and at least 0.
pub fn minimum_distance(host: &mut dyn Host, me: Value, right: Value) -> Result<f64, Exception> {
    let [left, right] = skylines(host, me, right)?;
    let d = left.distance(&right, 0.0);
    Ok(if 0.0 < d { d } else { 0.0 })
}

/// `right_column`: the column of lowest rank, below `INT_MAX`, of the
/// items among `me`'s `right-items`; none for a dead grob.
pub fn right_column(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    if !grob::is_live_grob(host.heap(), me) {
        return Ok(None);
    }
    let mut mincol = None;
    let mut min_rank = VERY_LARGE as u64;
    for ri in grob_array::extract_grob_array(host, me, "right-items")? {
        let heap = host.heap();
        if !item::is_item(heap, ri) {
            continue;
        }
        if let Some(col) = item::get_column(heap, ri) {
            let rank = paper_column::rank(heap, col);
            if rank < min_rank {
                min_rank = rank;
                mincol = Some(col);
            }
        }
    }
    Ok(mincol)
}

/// `left_column`: the column of `me`, a live item; none for anything
/// else.
pub fn left_column(host: &mut dyn Host, me: Value) -> Option<Value> {
    let heap = host.heap();
    if !item::is_item(heap, me) || !grob::is_live_grob(heap, me) {
        return None;
    }
    item::get_column(heap, me)
}

/// `get_note_columns`: the note columns among `elts`, in order, each
/// separation item's elements' in its place.
fn get_note_columns(host: &mut dyn Host, elts: &[Value]) -> Result<Vec<Value>, Exception> {
    let mut ret = Vec::new();
    for &e in elts {
        let heap = host.heap();
        if grob::has_interface(heap, e, "note-column-interface") {
            if item::is_item(heap, e) {
                ret.push(e);
            }
        } else if grob::has_interface(heap, e, "separation-item-interface") {
            let more_elts = grob_array::extract_grob_array(host, e, "elements")?;
            ret.extend(get_note_columns(host, &more_elts)?);
        }
    }
    Ok(ret)
}

/// `right_note_columns`: the note columns of `me`'s `right-items`
/// ([`get_note_columns`]).
pub fn right_note_columns(host: &mut dyn Host, me: Value) -> Result<Vec<Value>, Exception> {
    let elts = grob_array::extract_grob_array(host, me, "right-items")?;
    get_note_columns(host, &elts)
}

/// `left_note_columns`: the note columns of `me`'s `left-items`
/// ([`get_note_columns`]).
pub fn left_note_columns(host: &mut dyn Host, me: Value) -> Result<Vec<Value>, Exception> {
    let elts = grob_array::extract_grob_array(host, me, "left-items")?;
    get_note_columns(host, &elts)
}

/// `extremal_break_aligned_grob`: of `me`'s `left-break-aligned` (for
/// `d` -1) or `right-break-aligned` (1), each taken as its piece broken
/// as `break_dir` says, those with a pair as `space-alist` and a
/// non-empty X extent relative to the first one's column broken as
/// `break_dir` says, the one sticking out furthest towards `-d`, with
/// that extent (LilyPond's `last_ext`, empty with none). The last of
/// them wins a tie: they are taken last first, and only one sticking out
/// further replaces it.
pub fn extremal_break_aligned_grob(
    host: &mut dyn Host,
    me: Value,
    d: geometry::Direction,
    break_dir: geometry::Direction,
) -> Result<Option<(Value, Interval)>, Exception> {
    let name = if d == -1 {
        "left-break-aligned"
    } else {
        "right-break-aligned"
    };
    let elts = grob_array::extract_grob_array(host, me, name)?;
    let mut col = None;
    let mut last: Option<(Value, Interval)> = None;

    for &e in elts.iter().rev() {
        let heap = host.heap();
        if !item::is_item(heap, e) {
            continue;
        }
        let break_item = if item::break_status_dir(heap, e) != break_dir {
            item::find_prebroken_piece(heap, e, break_dir)
        } else {
            Some(e)
        };
        let Some(break_item) = break_item else {
            continue;
        };
        let space_alist = grob::property(host, break_item, "space-alist")?;
        let heap = host.heap();
        if heap.car(space_alist).is_none() {
            continue;
        }

        if col.is_none() {
            col = match item::is_item(heap, elts[0]) {
                true => item::get_column(heap, elts[0])
                    .and_then(|c| item::find_prebroken_piece(heap, c, break_dir)),
                false => None,
            };
        }
        let ext = grob::extent(host, break_item, col.unwrap_or(Value::NULL), X)?;
        if ext.is_empty() {
            continue;
        }

        let further = match last {
            None => true,
            Some((_, last_ext)) => f64::from(d) * (ext.at(-d) - last_ext.at(-d)) < 0.0,
        };
        if further {
            last = Some((break_item, ext));
        }
    }
    Ok(last)
}
