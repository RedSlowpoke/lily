//! `staff-spacing.cc` (part 3's, MS3 step 31): the spring from a
//! non-musical column's prefatory matter, a clef or a bar line, to the
//! next column. `get_spacing` (`staff-spacing.cc:112–221`, 110 lines)
//! takes the distances of the `space-alist` of the break-aligned grob
//! sticking out furthest towards it, corrected by `next_notes_correction`
//! (`:95–110`, 16) for the optical illusion of a down-stem after a bar
//! line, `optical_correction` (`:36–67`, 32), along the staff positions
//! the bar line covers, `bar_y_positions` (`:68–93`, 26), which the note
//! spacing calls too. Rust with no Scheme face: the caller is the spacing
//! spanner's `breakable_column_spacing` (`spacing_spanner.rs`).
//!
//! LilyPond 2.26.0's clang fuses none of their arithmetic (otool: `fadd`,
//! `fsub`, `fmul`, `fdiv`, `fabs`, `fmaxnm` with a constant 0 first, and
//! `fcmp`/`fcsel` for the other `std::max` and `std::min`).
//! The programming error is warn.cc's, at no location, the spring's given
//! as they arise. A `space-alist` entry whose value is no pair is `cdr`'s
//! `wrong-type-arg`, a distance that is no real `scm_to_double`'s, as
//! Guile's. Where LilyPond reads through a null, a bar line with no staff
//! symbol is measured from its root, in staff spaces of 1, and a staff
//! spacing in no column throws `ly:grob-property`'s `wrong-type-arg` for
//! its minimum distance.

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Heap, Value};

use crate::direction;
use crate::geometry::{Direction, Interval, Y, max, min};
use crate::grob;
use crate::item;
use crate::note_column;
use crate::paper_column;
use crate::simple_spacer::report;
use crate::spacing_interface::{self, VERY_LARGE};
use crate::spacing_options::double_or;
use crate::spring::{Errors, Spring};
use crate::stem;
use crate::unpure::pure_y_extent;

const LEFT: Direction = -1;
const DOWN: Direction = -1;

/// The types of a `space-alist` entry's distance.
const TYPES: [&str; 7] = [
    "fixed-space",
    "extra-space",
    "semi-fixed-space",
    "minimum-space",
    "minimum-fixed-space",
    "shrink-space",
    "semi-shrink-space",
];

/// `optical_correction`: for a note column `g` whose stem is a normal
/// one pointing down, a seventh of how far it overlaps `bar_height` (at
/// most 1), times `me`'s `stem-spacing-correction` (1 if no number); 0
/// for no note column, no stem, another stem or an empty `bar_height`.
fn optical_correction(
    host: &mut dyn Host,
    me: Value,
    g: Value,
    bar_height: Interval,
) -> Result<f64, Exception> {
    if !grob::has_interface(host.heap(), g, "note-column-interface") {
        return Ok(0.0);
    }

    let stem = note_column::get_stem(host, g)?;
    let mut ret = 0.0;

    if let Some(stem) = stem.filter(|_| !bar_height.is_empty()) {
        let d = direction::get_grob_direction(host, stem)?;
        if stem::is_normal_stem(host, stem)? && d == DOWN {
            let mut stem_posns = pure_y_extent(host, stem, stem, 0, VERY_LARGE)?;
            stem_posns.intersect(bar_height);

            ret = min((stem_posns.length() / 7.0).abs(), 1.0);
            let corr = grob::property(host, me, "stem-spacing-correction")?;
            ret *= double_or(host, corr, 1.0);
        }
    }
    Ok(ret)
}

/// `next_notes_correction`: the largest [`optical_correction`], at
/// least 0, of the note columns of `me`'s original's `right-items`,
/// along the staff positions `last_grob` covers if it is a bar line.
fn next_notes_correction(
    host: &mut dyn Host,
    me: Value,
    last_grob: Value,
) -> Result<f64, Exception> {
    let bar_size = bar_y_positions(host, last_grob)?;
    let orig = grob::with_grob(host.heap(), me, |g| g.original)
        .flatten()
        .unwrap_or(me);
    let note_columns = spacing_interface::right_note_columns(host, orig)?;

    let mut max_optical = 0.0;
    for nc in note_columns {
        max_optical = max(max_optical, optical_correction(host, me, nc, bar_size)?);
    }
    Ok(max_optical)
}

/// `bar_y_positions`: for a bar line whose `glyph-name` begins with `|`
/// or `.`, its Y extent from its common refpoint with its staff symbol,
/// in staff spaces; empty for any other grob or glyph.
pub fn bar_y_positions(host: &mut dyn Host, bar_grob: Value) -> Result<Interval, Exception> {
    let mut bar_size = Interval::EMPTY;
    if grob::has_interface(host.heap(), bar_grob, "bar-line-interface") {
        let glyph = grob::property(host, bar_grob, "glyph-name")?;
        let staff_sym = crate::call_lily(host, "%staff-symbol", &[bar_grob])?;
        let heap = host.heap();
        let glyph_string = heap.str(glyph).map_or("", |s| s.as_str());
        if glyph_string.starts_with('|') || glyph_string.starts_with('.') {
            let common = if staff_sym == Value::FALSE {
                None
            } else {
                grob::common_refpoint(heap, bar_grob, staff_sym, Y)
            };
            bar_size = grob::extent(host, bar_grob, common.unwrap_or(Value::NULL), Y)?;
            let ss = crate::call_lily(host, "%staff-space", &[bar_grob])?;
            let ss = host.heap().to_f64(ss).unwrap_or(1.0);
            bar_size.scale(1.0 / ss);
        }
    }
    Ok(bar_size)
}

/// `ly_is_list`: a proper list.
pub(crate) fn is_list(heap: &Heap, v: Value) -> bool {
    // Floyd's: a cycle is no list.
    let (mut slow, mut fast) = (v, v);
    loop {
        for _ in 0..2 {
            match heap.cdr(fast) {
                Some(next) => fast = next,
                None => return fast == Value::NULL,
            }
        }
        slow = heap.cdr(slow).unwrap_or(Value::NULL);
        if slow == fast {
            return false;
        }
    }
}

/// `scm_cdr` or `scm_car` of `v`, Guile's `wrong-type-arg` from `subr`
/// for what is no pair.
fn pair_part(heap: &mut Heap, v: Value, subr: &str) -> Result<Value, Exception> {
    let part = if subr == "car" {
        heap.car(v)
    } else {
        heap.cdr(v)
    };
    part.ok_or_else(|| crate::wrong_type(heap, Some(subr), 0, v, "pair"))
}

/// `get_spacing`: the spring from `me`'s column to `right_col` after the
/// break-aligned grob of `me`'s `left-break-aligned` sticking out
/// furthest right, by its `space-alist`'s `first-note` entry (its
/// `next-note` one, if it has one, for an unbroken `me`): from the
/// grob's right edge, its fixed distance there and its ideal one 1
/// further, each moved by the entry's distance as its type says, the
/// stretchability their difference unless the type shrinks only. The
/// ideal distance grows by `situational_space`, both by the
/// [`next_notes_correction`], and the fixed one to leave a gap of 0.3 at
/// least past the columns' minimum distance, the ideal one at least the
/// fixed one; compressible by their difference. The default spring
/// without such a grob, with no list as its `space-alist`, or with
/// neither entry (LilyPond's programming error).
pub fn get_spacing(
    host: &mut dyn Host,
    me: Value,
    right_col: Value,
    situational_space: f64,
) -> Result<Spring, Exception> {
    let left_col = item::get_column(host.heap(), me);

    let break_dir = item::break_status_dir(host.heap(), me);
    let last = spacing_interface::extremal_break_aligned_grob(host, me, LEFT, break_dir)?;
    // No warning: it would be given for many a staff without a clef.
    let Some((last_grob, last_ext)) = last else {
        return Ok(Spring::default());
    };

    let alist = grob::property(host, last_grob, "space-alist")?;
    if !is_list(host.heap(), alist) {
        return Ok(Spring::default());
    }

    let heap = host.heap();
    let [first_note, next_note] = ["first-note", "next-note"].map(|n| heap.intern(n));
    let mut space_def = grob::assq(heap, first_note, alist);
    if item::break_status_dir(heap, me) == 0 {
        let nndef = grob::assq(heap, next_note, alist);
        if nndef.is_some() {
            space_def = nndef;
        }
    }

    let Some(space_def) = space_def else {
        grob::unlocated_programming_error(host, "unknown prefatory spacing")?;
        return Ok(Spring::default());
    };

    let heap = host.heap();
    let space_def = pair_part(heap, space_def, "cdr")?;
    let distance = pair_part(heap, space_def, "cdr")?;
    let distance = grob::to_double(heap, distance)?;
    let ty = pair_part(heap, space_def, "car")?;
    // The type's name, if it is one of them (`scm_is_eq`).
    let ty = TYPES.into_iter().find(|&t| heap.intern(t) == ty);
    let mut is_stretchable = true;

    let mut fixed = last_ext.right;
    let mut ideal = fixed + 1.0;

    match ty.unwrap_or("") {
        "fixed-space" => {
            fixed += distance;
            ideal = fixed;
        }
        "extra-space" => ideal = fixed + distance,
        "semi-fixed-space" => {
            fixed += distance / 2.0;
            ideal = fixed + distance / 2.0;
        }
        "minimum-space" => ideal = last_ext.left + max(last_ext.length(), distance),
        "minimum-fixed-space" => {
            fixed = last_ext.left + max(last_ext.length(), distance);
            ideal = fixed;
        }
        "shrink-space" => {
            ideal = fixed + distance;
            is_stretchable = false;
        }
        "semi-shrink-space" => {
            fixed += distance / 2.0;
            ideal = fixed + distance / 2.0;
            is_stretchable = false;
        }
        _ => {}
    }

    let stretchability = if is_stretchable { ideal - fixed } else { 0.0 };

    // The caller's, which can include the full measure's extra space.
    ideal += situational_space;

    let optical_correction = next_notes_correction(host, me, last_grob)?;
    fixed += optical_correction;
    ideal += optical_correction;

    let min_dist =
        paper_column::minimum_distance(host, left_col.unwrap_or(Value::NULL), right_col)?;

    // The fixed distance leaves a gap of 0.3 staff spaces at least.
    let min_dist_correction = max(0.0, 0.3 + min_dist - fixed);
    fixed += min_dist_correction;
    ideal = max(ideal, fixed);

    let mut errors = Errors::new();
    let mut ret = Spring::new(ideal, min_dist, &mut errors);
    ret.set_inverse_stretch_strength(max(0.0, stretchability), &mut errors);
    ret.set_inverse_compress_strength(max(0.0, ideal - fixed), &mut errors);
    report(host, &mut errors)?;
    Ok(ret)
}
