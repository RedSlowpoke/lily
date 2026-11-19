//! `spacing-loose-columns.cc` (part 3's, MS3 step 32): `set_loose_columns`
//! (`:33–222`, 190 lines), the loose columns of a spaced line put on its
//! system and placed, after `space_line` (`constrained_breaking.rs`) has
//! placed the others, between the columns they hang between
//! (`between-cols`, which `prune_loose_columns` sets,
//! `spacing_determine_loose_columns.rs`). Rust with no Scheme face:
//! LilyPond's one caller is `System::break_into_pieces`, whose part for a
//! line's columns `system::place_line` is; a traced compile places its
//! one line (`dump.rs`, D129).
//!
//! A clique, a loose column's left neighbour and the loose columns after
//! it up to the first column that is not loose, is spaced at the ideal
//! distances of the springs between them, scaled down towards their
//! minimum distances as far as the room between the neighbours needs, and
//! placed from its right end leftwards. `tight + (ideal − tight) × scale`
//! is one `fmadd` in LilyPond 2.26.0's binary, so [`fma`] here.
//!
//! Where LilyPond reads through a null: a clique whose left neighbour is
//! on no system and has no right piece, or that has no left neighbour, or
//! that has no right one and a system with no right bound, is not placed;
//! a system with no right bound or no original searches no columns for a
//! right one. A loose column with no spacing object is spaced by the
//! default options with `'()` as the spacing grob, which a breakable
//! column's spacing cannot read a property of.

use alloc::vec;
use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Tag, Value};

use crate::geometry::{X, fma, max, min};
use crate::grob;
use crate::grob_array;
use crate::item;
use crate::note_spacing;
use crate::paper_column;
use crate::simple_spacer::ColumnXPositions;
use crate::spacing_basic;
use crate::spacing_determine_loose_columns::{grob_of, object, paper_column_of, x_length};
use crate::spacing_options::SpacingOptions;
use crate::spanner;
use crate::spring::Spring;

/// `set_loose_columns`: the loose columns of `posns` put on the system
/// `which`, then each one's clique, from its left neighbour (its right
/// piece if the neighbour is on no system) to its right one (its left
/// piece on `which`, else `which`'s right bound if that ranks before it,
/// else, with LilyPond's programming error, the last column of
/// `which`'s original on `which` before that bound), spaced into the room
/// between them.
pub fn set_loose_columns(
    host: &mut dyn Host,
    which: Value,
    posns: &ColumnXPositions,
) -> Result<(), Exception> {
    if posns.loose_cols.is_empty() {
        return Ok(());
    }

    for &c in &posns.loose_cols {
        paper_column::set_system(host.heap(), c, which);
    }

    'cols: for &first in &posns.loose_cols {
        let mut loose = first;
        let mut left = None;
        let mut right = None;

        let mut clique = Vec::new();
        loop {
            let between = object(host, loose, "between-cols")?;
            let heap = host.heap();
            if between.tag() != Tag::Pair {
                break;
            }

            // If the line was broken at one of the loose columns, split
            // the clique at that column.
            if item::get_system(heap, loose).is_none() {
                break;
            }

            // A column's `get_column` is itself.
            let le = heap.car(between).and_then(|v| paper_column_of(heap, v));
            let re = heap.cdr(between).and_then(|v| paper_column_of(heap, v));
            let (Some(le), Some(re)) = (le, re) else {
                break;
            };

            if left.is_none() {
                let mut l = le;
                if item::get_system(heap, l).is_none() {
                    let Some(piece) = item::find_prebroken_piece(heap, l, 1) else {
                        continue 'cols;
                    };
                    l = piece;
                }
                left = Some(l);
                clique.push(l);
            }

            clique.push(loose);

            loose = re;
            right = Some(re);
        }

        if right.is_none() {
            grob::unlocated_programming_error(
                host,
                "Can't attach loose column sensibly.  Attaching to end of system.",
            )?;
            right = spanner::get_bound(host, which, 1)?;
        }
        let (Some(left), Some(mut right)) = (left, right) else {
            continue;
        };

        let heap = host.heap();
        if item::get_system(heap, right).is_some() {
            // Do nothing.
        } else if let Some(piece) = item::find_prebroken_piece(heap, right, -1)
            && item::get_system(heap, piece) == Some(which)
        {
            right = piece;
        } else if let Some(bound) = spanner::get_bound(host, which, 1)?
            && paper_column::rank(host.heap(), bound) < paper_column::rank(host.heap(), right)
        {
            right = bound;
        } else {
            let last = clique[clique.len() - 1];
            grob::programming_error(
                host,
                last,
                "Loose column does not have right side to attach to.",
            )?;
            let heap = host.heap();
            let base_system = grob::with_grob(heap, which, |g| g.original).flatten();
            let j = paper_column::rank(heap, last) + 1;
            let end_rank = match spanner::get_bound(host, which, 1)? {
                Some(bound) => paper_column::rank(host.heap(), bound),
                None => 0,
            };
            if let Some(base_system) = base_system {
                let base_cols = grob_array::extract_grob_array(host, base_system, "columns")?;
                for j in j..end_rank {
                    let Some(&col) = usize::try_from(j).ok().and_then(|j| base_cols.get(j)) else {
                        break;
                    };
                    if item::get_system(host.heap(), col) == Some(which) {
                        right = col;
                    }
                }
            }
        }

        let common = grob::common_refpoint(host.heap(), right, left, X).unwrap_or(Value::NULL);

        clique.push(right);

        // `clique_spacing` keeps the ideal spaces, `clique_tight_spacing`
        // the minimum ones. The loose columns' rods are tight
        // (`set_distances_for_loose_col`), but other rods may widen the
        // room: aim for the ideal spaces, and fall back on the tight ones
        // as the room gets smaller.
        let mut clique_spacing = vec![0.0];
        let mut clique_tight_spacing = vec![0.0];
        for j in 1..clique.len() - 1 {
            let loose_col = clique[j];
            let next_col = clique[j + 1];

            let v = object(host, loose_col, "spacing")?;
            let mut spacing = grob_of(host.heap(), v);
            let v = object(host, loose_col, "grace-spacing")?;
            if let Some(grace_spacing) = grob_of(host.heap(), v) {
                spacing = Some(grace_spacing);
            }

            let mut options = SpacingOptions::default();
            match spacing {
                Some(spacing) => options.init_from_grob(host, spacing)?,
                None => grob::unlocated_programming_error(host, "Column without spacing object")?,
            }
            let me = spacing.unwrap_or(Value::NULL);

            let spring = if paper_column::is_musical(host, next_col)?
                && paper_column::is_musical(host, loose_col)?
            {
                if spacing.is_some()
                    && grob::has_interface(host.heap(), me, "note-spacing-interface")
                {
                    note_spacing::get_spacing(
                        host,
                        me,
                        next_col,
                        Spring::default(),
                        options.increment,
                    )?
                } else {
                    spacing_basic::note_spacing(host, me, loose_col, next_col, &options)?
                }
            } else {
                spacing_basic::standard_breakable_column_spacing(
                    host, me, loose_col, next_col, &options,
                )?
            };

            let loose_col_horizontal_length = x_length(host, loose_col)?;
            clique_spacing.push(max(spring.ideal_distance, loose_col_horizontal_length));
            clique_tight_spacing.push(max(spring.min_distance, loose_col_horizontal_length));
        }

        let last = clique[clique.len() - 1];
        let permissible_distance = grob::relative_coordinate(host, last, common, X)?
            - grob::robust_relative_extent(host, clique[0], common, X)?.right;
        // LilyPond's first right point, never read.
        grob::robust_relative_extent(host, last, common, X)?;
        let mut finished_right_column = last;

        let mut sum_tight_spacing = 0.0;
        let mut sum_spacing = 0.0;
        // A magic number: what grob would hold it?
        let left_padding = 0.15;
        for (tight, ideal) in clique_tight_spacing.iter().zip(&clique_spacing) {
            sum_tight_spacing += tight;
            sum_spacing += ideal;
        }
        let scale_factor = max(
            0.0,
            min(
                1.0,
                (permissible_distance - left_padding - sum_tight_spacing)
                    / (sum_spacing - sum_tight_spacing),
            ),
        );
        for j in (1..clique.len() - 1).rev() {
            let clique_col = clique[j];

            let right_point = grob::relative_coordinate(host, finished_right_column, common, X)?;

            let tight = clique_tight_spacing[j];
            let distance_to_next = fma(clique_spacing[j] - tight, scale_factor, tight);

            let my_offset = right_point - distance_to_next;

            let y = my_offset - grob::relative_coordinate(host, clique_col, common, X)?;
            if let Err(text) = grob::translate_axis(host.heap(), clique_col, y, X) {
                grob::programming_error(host, clique_col, text)?;
            }

            finished_right_column = clique_col;
        }
    }
    Ok(())
}
