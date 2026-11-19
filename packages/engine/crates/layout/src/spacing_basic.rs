//! `spacing-basic.cc` (part 3's, MS3 step 31): the spacing spanner's
//! simple-minded springs, which the note and staff spacing then adjust:
//! between breakable columns and columns at the same moment,
//! `standard_breakable_column_spacing` (`spacing-basic.cc:40–83`, 44
//! lines), and between musical columns by the duration alone,
//! `note_spacing` (`:108–183`, 76), with the measure length it caps the
//! time between them by, `get_measure_length` (`:85–105`, 21). Rust with
//! no Scheme face: their callers are the spacing spanner's
//! `breakable_column_spacing` and `musical_column_spacing`
//! (`spacing_spanner.rs`).
//!
//! LilyPond 2.26.0's clang fuses none of their arithmetic (otool: `fmul`,
//! `fadd`, `fsub` and `fmaxnm` only, `fmaxnm` being `std::max` with a
//! constant first). Its programming errors are warn.cc's, at no location,
//! the springs' given as they arise. A moment or a ratio of durations
//! that does not fit, where LilyPond's would wrap, is
//! `numerical-overflow` (D61). Where LilyPond reads through a null or
//! past its columns, a column with no X parent has no measure length, nor
//! does a rank beyond its system's `columns`.

use core::cmp::Ordering;

use lily_engine_music::{Host, Moment, Rational};
use lily_engine_scheme::{Exception, Value};

use crate::geometry::{X, max};
use crate::grob;
use crate::grob_array;
use crate::paper_column;
use crate::paper_column_engraver::rational;
use crate::simple_spacer::report;
use crate::spacing_options::{SpacingOptions, double_or};
use crate::spring::{Errors, Spring};

/// `std::min` of two moments: `b` only when `b < a`.
fn min(a: Moment, b: Moment) -> Moment {
    if b.compare(a) == Ordering::Less { b } else { a }
}

/// The moment `m` of `v`, if it is one.
fn moment_of(host: &mut dyn Host, v: Value) -> Option<Moment> {
    host.heap().foreign::<Moment>(v).copied()
}

/// `a - b`, or `numerical-overflow` from `subr`.
fn minus(host: &mut dyn Host, a: Moment, b: Moment, subr: &str) -> Result<Moment, Exception> {
    a.checked_sub(b)
        .ok_or_else(|| crate::overflow(host.heap(), subr))
}

/// `double (a / b)`, or `numerical-overflow` from `subr`.
fn ratio(host: &mut dyn Host, a: Rational, b: Rational, subr: &str) -> Result<f64, Exception> {
    match a.checked_div(b) {
        Some(r) => Ok(r.to_f64()),
        None => Err(crate::overflow(host.heap(), subr)),
    }
}

/// `standard_breakable_column_spacing`: between two breakable columns, a
/// spring as long as their minimum distance (at least 0) and 0.8 times
/// `me`'s `spacing-increment` (1 if no number) per shortest duration in
/// `l`'s `measure-length` (1 if no moment), stretching as that space
/// alone; between others, the minimum distance and 0.5 if they are at
/// the same moment, else the space of the main part of the time between
/// them.
pub fn standard_breakable_column_spacing(
    host: &mut dyn Host,
    me: Value,
    l: Value,
    r: Value,
    options: &SpacingOptions,
) -> Result<Spring, Exception> {
    const SUBR: &str = "standard_breakable_column_spacing";
    let min_dist = max(0.0, paper_column::minimum_distance(host, l, r)?);
    let mut errors = Errors::new();

    if paper_column::is_breakable(host, l)? && paper_column::is_breakable(host, r)? {
        let dt = grob::property(host, l, "measure-length")?;
        let mlen = moment_of(host, dt).unwrap_or(Moment::new(Rational::int(1), Rational::ZERO));

        let v = grob::property(host, me, "spacing-increment")?;
        let incr = double_or(host, v, 1.0);
        let space = incr * ratio(host, mlen.main, options.global_shortest, SUBR)? * 0.8;
        let mut spring = Spring::new(min_dist + space, min_dist, &mut errors);

        // The stretch not scaled with the minimum distance, so that an
        // empty first measure (a clef's) stretches as one later does.
        spring.set_inverse_stretch_strength(space, &mut errors);
        report(host, &mut errors)?;
        return Ok(spring);
    }

    let rwhen = paper_column::when_mom(host, r)?;
    let lwhen = paper_column::when_mom(host, l)?;
    let dt = minus(host, rwhen, lwhen, SUBR)?;
    let ideal = if dt == Moment::default() {
        // The staff spacing's job.
        min_dist + 0.5
    } else {
        min_dist + options.get_duration_space(host, dt.main)?
    };
    let spring = Spring::new(ideal, min_dist, &mut errors);
    report(host, &mut errors)?;
    Ok(spring)
}

/// `get_measure_length`: the `measure-length` of `column`, or of the
/// nearest column before it on its system (its X parent's `columns`, by
/// rank) that has one; infinite if none has.
fn get_measure_length(host: &mut dyn Host, column: Value) -> Result<Moment, Exception> {
    let infinity = Moment::new(Rational::INFINITY, Rational::ZERO);
    let Some(sys) = grob::get_parent(host.heap(), column, X) else {
        return Ok(infinity);
    };
    let cols = grob_array::extract_grob_array(host, sys, "columns")?;
    // From the column's rank down; a rank beyond them, from the last.
    let rank = paper_column::rank(host.heap(), column);
    let end = usize::try_from(rank).map_or(cols.len(), |r| r.saturating_add(1));
    for &col in cols[..end.min(cols.len())].iter().rev() {
        let len = grob::property(host, col, "measure-length")?;
        if let Some(len) = moment_of(host, len) {
            return Ok(len);
        }
    }
    Ok(infinity)
}

/// `note_spacing`: the spring between the musical columns `lc` and `rc`
/// by duration alone. The time between them, at most a measure, is a
/// fraction of `lc`'s `shortest-playing-duration` (at most a measure; 1
/// after a programming error if it is not above 0): the spring is that
/// fraction of the duration's space, at least that fraction of the
/// increment, and stretches as that fraction of their difference (at
/// least 0.1). After grace notes, or between them, it is the grace
/// spacing's space of the grace time between them, at least its
/// increment, stretching as half that; without a `grace-spacing`, half
/// the shortest duration's space and half the increment. Otherwise it is
/// `Spring ()`.
pub fn note_spacing(
    host: &mut dyn Host,
    _me: Value,
    lc: Value,
    rc: Value,
    options: &SpacingOptions,
) -> Result<Spring, Exception> {
    const SUBR: &str = "note_spacing";
    let v = grob::property(host, lc, "shortest-playing-duration")?;
    let mut shortest_playing_len = rational(host.heap(), v, Rational::ZERO);
    if shortest_playing_len.compare(Rational::ZERO) != Ordering::Greater {
        let mut text = alloc::string::String::from("cannot find a ruling note at: ");
        paper_column::when_mom(host, lc)?.write(&mut text);
        grob::unlocated_programming_error(host, &text)?;
        shortest_playing_len = Rational::int(1);
    }

    let lwhen = paper_column::when_mom(host, lc)?;
    let rwhen = paper_column::when_mom(host, rc)?;
    let mut delta_t = minus(host, rwhen, lwhen, SUBR)?;

    // A multi-measure rest may span several measures to a non-musical
    // column: no more than one counts.
    let mlen = get_measure_length(host, lc)?;
    delta_t = min(delta_t, mlen);
    if mlen.main.compare(shortest_playing_len) == Ordering::Less {
        shortest_playing_len = mlen.main;
    }

    let mut errors = Errors::new();
    let mut ret = Spring::default();
    if !delta_t.main.is_zero() && lwhen.grace.is_zero() {
        // The controlling duration's spring, and the minimum a canonical
        // notehead's width.
        let len = options.get_duration_space(host, shortest_playing_len)?;
        let min = options.increment;

        // The part of it the time between the columns is.
        let fraction = ratio(host, delta_t.main, shortest_playing_len, SUBR)?;
        ret = Spring::new(fraction * len, fraction * min, &mut errors);

        // Stretching as the space between bare noteheads.
        ret.set_inverse_stretch_strength(fraction * max(0.1, len - min), &mut errors);
    } else if !delta_t.grace.is_zero() {
        let sym = host.heap().intern("grace-spacing");
        let grace_spacing = grob_array::object(host, lc, sym)?;
        if grob::with_grob(host.heap(), grace_spacing, |_| ()).is_some() {
            let mut grace_opts = SpacingOptions::default();
            grace_opts.init_from_grob(host, grace_spacing)?;
            let len = grace_opts.get_duration_space(host, delta_t.grace)?;
            let min = grace_opts.increment;
            ret = Spring::new(len, min, &mut errors);
            // Grace notes hardly stretch.
            ret.set_inverse_stretch_strength(grace_opts.increment / 2.0, &mut errors);
        } else {
            // The old grace spacing: half the shortest note's.
            let len = options.get_duration_space(host, options.global_shortest)?;
            ret = Spring::new(len / 2.0, options.increment / 2.0, &mut errors);
        }
    }
    report(host, &mut errors)?;
    Ok(ret)
}
