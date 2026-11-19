//! `note-spacing.cc` (part 3's, MS3 step 31): a voice's spring from a
//! note column to the next column, the springs by duration
//! (`spacing_basic.rs`) adjusted by what is in the columns. `get_spacing`
//! (`note-spacing.cc:38–115`, 78 lines) measures the ideal distance from
//! the end of the left note heads, or rests, keeps the columns' skylines
//! apart and, before a non-musical column, measures to its bar line;
//! `stem_dir_correction` (`:199–315`, 117) corrects for the optical
//! illusion of stems up then down, or down then up, with its
//! `knee_correction`, `different_directions_correction` and
//! `same_direction_correction` (`:117–197`, 81). Rust with no Scheme
//! face: the caller is the spacing spanner's `musical_column_spacing`
//! (`spacing_spanner.rs`).
//!
//! LilyPond 2.26.0's clang fuses none of their arithmetic (otool: `fadd`,
//! `fsub`, `fmul`, `fnmul`, `fmaxnm` and compares), and none of
//! `Staff_spacing::bar_y_positions` (`staff_spacing.rs`); of the stem's
//! functions they call, the stem offset's are fused (`stem.rs`). The
//! programming errors are warn.cc's, at no location, the springs' given
//! as they arise. Where LilyPond reads through a null, a note column in
//! no column has no head in its ancestry (LilyPond's programming error),
//! and a knee's head in no column is measured from its root.

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Exception, Value};

use crate::direction;
use crate::geometry::{Direction, Interval, X, max, min};
use crate::grob;
use crate::grob_array;
use crate::item;
use crate::note_column;
use crate::paper_column;
use crate::simple_spacer::report;
use crate::spacing_interface::{self, VERY_LARGE};
use crate::spacing_options::double_or;
use crate::spring::{Errors, Spring};
use crate::staff_spacing;
use crate::stem;
use crate::unpure::pure_y_extent;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// A `Drul_array`'s index of `d`: 0 for `LEFT` or `DOWN`, 1 otherwise.
fn side(d: Direction) -> usize {
    usize::from(d > 0)
}

/// `directed_opposite`: one up and the other down.
fn directed_opposite(a: Direction, b: Direction) -> bool {
    a * b < 0
}

/// `directed_same`: both up or both down.
fn directed_same(a: Direction, b: Direction) -> bool {
    a * b > 0
}

/// `me`'s property `name` as a real, `fallback` if it is no number.
fn real_property(
    host: &mut dyn Host,
    me: Value,
    name: &str,
    fallback: f64,
) -> Result<f64, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(double_or(host, v, fallback))
}

/// `get_spacing`: `base`, the spring by duration from `me`'s column to
/// `right_col`, its ideal distance less the `increment` plus where the
/// last of `me`'s left note columns' rest, or first head, ends, and its
/// minimum distance that of the columns' skylines (at least 0) at
/// `right_col`'s `skyline-vertical-padding`. Before a non-musical column
/// with a skyline, if `me`'s `space-to-barline` is `#t`, the ideal
/// distance is to its staff bar's left edge, or, without one, to the
/// column's right edge but at least halfway to the minimum. Then
/// corrected for the stems ([`stem_dir_correction`]), and at least 0.
pub fn get_spacing(
    host: &mut dyn Host,
    me: Value,
    right_col: Value,
    mut base: Spring,
    increment: f64,
) -> Result<Spring, Exception> {
    let note_columns = spacing_interface::left_note_columns(host, me)?;
    let mut left_head_end = 0.0;
    let rest = host.heap().intern("rest");
    for nc in note_columns {
        let r = grob_array::object(host, nc, rest)?;
        let heap = host.heap();
        let mut g = grob_of(heap, r).is_some().then_some(r);
        let col = item::get_column(heap, nc);
        if g.is_none() {
            g = note_column::first_head(host, nc)?;
        }

        // Without a stem, the first note head is not known.
        if let Some(g) = g {
            match col.filter(|&c| grob::has_in_ancestry(host.heap(), g, c, X)) {
                None => grob::unlocated_programming_error(
                    host,
                    "Note_spacing::get_spacing (): Common refpoint incorrect",
                )?,
                Some(col) => left_head_end = grob::extent(host, g, col, X)?.right,
            }
        }
    }

    // The note head's width, or the rest's, sets the space most: a
    // quarter rest gets almost half a staff space less than a note.
    let mut ideal = base.ideal_distance - increment + left_head_end;
    let skys = spacing_interface::skylines(host, me, right_col)?;
    let padding = real_property(host, right_col, "skyline-vertical-padding", 0.0)?;
    let distance = skys[0].distance(&skys[1], padding);
    let min_dist = max(0.0, distance);
    let mut errors = Errors::new();
    base.set_min_distance(min_dist, &mut errors);
    report(host, &mut errors)?;

    // Before a non-musical column, the ideal distance is to its bar line,
    // if any, not to the column.
    if !paper_column::is_musical(host, right_col)?
        && !skys[1].is_empty()
        && grob::property(host, me, "space-to-barline")? == Value::TRUE
    {
        let sym = host.heap().intern("break-alignment");
        let break_alignment = grob_array::object(host, right_col, sym)?;
        let mut staff_bar_group = None;
        if item::is_item(host.heap(), break_alignment) {
            let sym = host.heap().intern("staff-bar");
            let g = crate::call_lily(
                host,
                "%find-nonempty-break-align-group",
                &[break_alignment, sym],
            )?;
            staff_bar_group = (g != Value::FALSE).then_some(g);
        }

        if let Some(group) = staff_bar_group {
            ideal -= grob::extent(host, group, right_col, X)?.left;
        } else {
            // To the column's right side, but at least half the gap to a
            // note.
            let min_desired_space = (ideal + min_dist) / 2.0;
            ideal -= grob::extent(host, right_col, right_col, X)?.right;
            ideal = max(ideal, min_desired_space);
        }
    }

    stem_dir_correction(host, me, right_col, increment, &mut ideal)?;

    base.set_ideal_distance(max(0.0, ideal), &mut errors);
    report(host, &mut errors)?;
    Ok(base)
}

/// `knee_correction`: for stems on one beam, up then down or down then
/// up, the right stem's support head's right edge in its column (the
/// `increment` if it has no head or an empty extent) less the stem's
/// thickness, against the right stem's direction, times `note_spacing`'s
/// `knee-spacing-correction` (0 if no number).
fn knee_correction(
    host: &mut dyn Host,
    note_spacing: Value,
    right_stem: Value,
    increment: f64,
) -> Result<f64, Exception> {
    let mut note_head_width = increment;
    let head = stem::support_head(host, right_stem)?;
    if let Some(head) = head.filter(|&h| item::is_item(host.heap(), h)) {
        let col = item::get_column(host.heap(), head);
        let head_extent = grob::extent(host, head, col.unwrap_or(Value::NULL), X)?;
        if !head_extent.is_empty() {
            note_head_width = head_extent.right;
        }
        note_head_width -= stem::thickness(host, right_stem)?;
    }

    let dir = direction::get_grob_direction(host, right_stem)?;
    let corr = real_property(host, note_spacing, "knee-spacing-correction", 0.0)?;
    Ok(-note_head_width * f64::from(dir) * corr)
}

/// `different_directions_correction`: where the two stems' positions
/// overlap, a seventh of the overlap (at most 1) towards the left stem's
/// direction, times `note_spacing`'s `stem-spacing-correction` (0 if no
/// number); 0 where they do not.
fn different_directions_correction(
    host: &mut dyn Host,
    note_spacing: Value,
    stem_posns: [Interval; 2],
    left_stem_dir: Direction,
) -> Result<f64, Exception> {
    let mut ret = 0.0;
    let mut intersect = stem_posns[0];
    intersect.intersect(stem_posns[1]);

    if !intersect.is_empty() {
        ret = intersect.length().abs();

        // The 7 is LilyPond's.
        let corr = real_property(host, note_spacing, "stem-spacing-correction", 0.0)?;
        ret = min(ret / 7.0, 1.0) * f64::from(left_stem_dir) * corr;
    }
    Ok(ret)
}

/// `same_direction_correction`: where the two sides' heads do not
/// overlap and the higher side's lowest head is more than 1 above the
/// lower side's highest, `note_spacing`'s `same-direction-correction` (0
/// if no number) away from the lower side; else 0.
fn same_direction_correction(
    host: &mut dyn Host,
    note_spacing: Value,
    head_posns: [Interval; 2],
) -> Result<f64, Exception> {
    // Stems the same way with heads stepping up or down: the middle one
    // moves towards the lower.
    let mut hp = head_posns[0];
    hp.intersect(head_posns[1]);
    if !hp.is_empty() {
        return Ok(0.0);
    }

    let lowest = if head_posns[0].left > head_posns[1].right {
        RIGHT
    } else {
        LEFT
    };
    let delta = head_posns[side(-lowest)].left - head_posns[side(lowest)].right;
    let corr = real_property(host, note_spacing, "same-direction-correction", 0.0)?;
    Ok(if delta > 1.0 {
        f64::from(-lowest) * corr
    } else {
        0.0
    })
}

/// `stem_dir_correction`: `space` corrected for the optical illusion of
/// the stems of `me`'s left items' note columns and its right items' in
/// `rcolumn` (Wanske, p. 138): an up-stem then a down-stem wants more
/// space, a down-stem then an up-stem less. A bar line in `rcolumn`
/// sticking out furthest left of `me`'s `right-break-aligned` stands for
/// a right stem against the left one, along the staff positions it
/// covers, at half the correction. Stems on one beam take the knee's
/// correction, stems the same way the same direction's, unless a right
/// note column has accidentals. No correction where a note column has no
/// stem, a dead or invisible one, the stems of a side point different
/// ways, or a left stem is flagged shorter than a quarter and unbeamed.
pub fn stem_dir_correction(
    host: &mut dyn Host,
    me: Value,
    rcolumn: Value,
    increment: f64,
    space: &mut f64,
) -> Result<(), Exception> {
    let mut stem_dirs: [Direction; 2] = [0, 0];
    let mut stem_posns = [Interval::EMPTY; 2];
    let mut head_posns = [Interval::EMPTY; 2];
    let syms = ["left-items", "right-items"].map(|name| host.heap().intern(name));
    let props = [
        grob_array::object(host, me, syms[0])?,
        grob_array::object(host, me, syms[1])?,
    ];
    let mut beams_drul: [Option<Value>; 2] = [None, None];
    let mut stems_drul: [Option<Value>; 2] = [None, None];
    let mut bar_yextent = Interval::EMPTY;
    let mut acc_right = false;

    let break_dir = item::break_status_dir(host.heap(), rcolumn);
    let bar = spacing_interface::extremal_break_aligned_grob(host, me, RIGHT, break_dir)?;
    if let Some((bar, _)) = bar
        && item::get_column(host.heap(), bar) == Some(rcolumn)
    {
        bar_yextent = staff_spacing::bar_y_positions(host, bar)?;
    }

    for (s, d) in [(0, LEFT), (1, RIGHT)] {
        let items = grob_array::grobs_of(host.heap(), props[s]);
        for it in items {
            let heap = host.heap();
            if !item::is_item(heap, it) || !grob::has_interface(heap, it, "note-column-interface") {
                continue;
            }
            if d == RIGHT && item::get_column(heap, it) != Some(rcolumn) {
                continue;
            }

            // Accidentals sticking out of the right side.
            if d == RIGHT && !acc_right {
                acc_right = note_column::accidentals(host, it)?.is_some();
            }

            let Some(stem) = note_column::get_stem(host, it)? else {
                return Ok(());
            };
            if !grob::is_live_grob(host.heap(), stem) || stem::is_invisible(host, stem)? {
                return Ok(());
            }

            stems_drul[s] = Some(stem);
            beams_drul[s] = stem::get_beam(host, stem)?;

            let stem_dir = direction::get_grob_direction(host, stem)?;
            if stem_dirs[s] != 0 && stem_dirs[s] != stem_dir {
                return Ok(());
            }
            stem_dirs[s] = stem_dir;

            // Not with a large flag hanging from the note.
            if d == LEFT
                && stem::duration_log(host, stem)? > 2
                && stem::get_beam(host, stem)?.is_none()
            {
                return Ok(());
            }

            let hp = stem::head_positions(host, stem)?;
            if !hp.is_empty() {
                let ss = crate::staff_symbol_referencer::staff_space(host, stem)?;
                let mut posns = pure_y_extent(host, stem, stem, 0, VERY_LARGE)?;
                posns.scale(2.0 / ss);
                stem_posns[s] = posns;
                head_posns[s].unite(hp);
            }
        }
    }

    let mut correction = 0.0;

    if !bar_yextent.is_empty() {
        stem_dirs[1] = -stem_dirs[0];
        stem_posns[1] = bar_yextent;
        stem_posns[1].scale(2.0);
    }

    if directed_opposite(stem_dirs[0], stem_dirs[1]) {
        // A beam's stems have their right stem set with it.
        match (beams_drul[0], stems_drul[1]) {
            (Some(beam), Some(right_stem)) if beams_drul[1] == Some(beam) => {
                correction = knee_correction(host, me, right_stem, increment)?;
            }
            _ => {
                correction = different_directions_correction(host, me, stem_posns, stem_dirs[0])?;
                if !bar_yextent.is_empty() {
                    correction *= 0.5;
                }
            }
        }
    } else if directed_same(stem_dirs[0], stem_dirs[1]) && !acc_right {
        // Only without accidentals sticking out of the right side.
        correction = same_direction_correction(host, me, head_posns)?;
    }

    *space += correction;
    Ok(())
}
