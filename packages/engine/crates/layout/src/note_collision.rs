//! `note-collision.cc` (part 3's, MS3 step 41): the note collision's
//! positioning, `ly:note-collision-interface::calc-positioning-done`
//! (`note-collision.cc:403–471`, 69 lines), the music crate's placeholder
//! until then (D67), with `check_meshing_chords` (`:39–401`, 363),
//! `get_clash_groups` (`:473–498`, 26), `automatic_shift` (`:500–605`,
//! 106) and `forced_shift` (`:607–622`, 16): Rust under a primitive that
//! prints as LilyPond's subr and calls it over `%host-call` (D127,
//! [`HOSTS`]), as the stem's do. And `add_column` (`:624–629`, 6), which
//! `Collision_engraver` calls (`collision_engraver.rs`).
//! `note_head_positions` (`:631–645`) goes with its caller, the rest
//! collision.
//!
//! LilyPond sorts the clash groups with `std::sort`, which is not stable:
//! the port sorts with libc++'s ([`lily_engine_skyline::sort`]), each
//! column's `horizontal-shift` read once before, where LilyPond reads it
//! at each comparison (the same error, if one throws). Where LilyPond
//! reads through a null or past a vector (meshed columns without a first
//! or support head, or whose filtered head positions are empty), the port
//! takes them as too far apart to collide, or as off the staff lines.

use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, Heap, HostHandler, Value, Vm};

use crate::axis_group;
use crate::geometry::{Direction, X};
use crate::grob;
use crate::grob_array;
use crate::note_column;
use crate::rhythmic_head;
use crate::side_position;
use crate::stem;
use crate::stem::int_or;

const DOWN: Direction = -1;
const UP: Direction = 1;
const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// A `Drul_array`'s index of `d`: 0 down (left), 1 up (right).
fn ix(d: Direction) -> usize {
    usize::from(d > 0)
}

/// `from_scm<bool> (get_property (me, name))`: `#t` only.
fn is_true(host: &mut dyn Host, me: Value, name: &str) -> Result<bool, Exception> {
    Ok(grob::property(host, me, name)? == Value::TRUE)
}

/// `from_scm (v, fallback)` of a `Direction`: an exact -1, 0 or 1, else
/// the fallback.
pub(crate) fn direction_or(v: Value, fallback: Direction) -> Direction {
    match v.as_fixnum() {
        Some(d @ -1..=1) => d as Direction,
        _ => fallback,
    }
}

/// `scm_memq`: whether `x` is `eq?` to an element of `list`.
fn memq(heap: &Heap, x: Value, list: Value) -> bool {
    let mut at = list;
    while let (Some(e), Some(next)) = (heap.car(at), heap.cdr(at)) {
        if e == x {
            return true;
        }
        at = next;
    }
    false
}

/// `Staff_symbol_referencer::on_line`: `%staff-on-line?`, the ledgers
/// asked.
fn on_line(host: &mut dyn Host, me: Value, pos: i32) -> Result<bool, Exception> {
    let p = host.heap().integer(i64::from(pos));
    Ok(crate::call_lily(host, "%staff-on-line?", &[me, p])? != Value::FALSE)
}

/// `get_object (me, name)` if it is a grob.
fn grob_object(host: &mut dyn Host, me: Value, name: &str) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern(name);
    let g = grob_array::object(host, me, sym)?;
    Ok(grob_of(host.heap(), g).is_some().then_some(g))
}

/// The dots of `head` and their X parent, if that is a dot column.
fn dot_column_of(host: &mut dyn Host, head: Value) -> Result<Option<Value>, Exception> {
    let Some(d) = grob_object(host, head, "dot")? else {
        return Ok(None);
    };
    Ok(grob::get_parent(host.heap(), d, X)
        .filter(|&p| grob::has_interface(host.heap(), p, "dot-column-interface")))
}

/// The `note-heads` of the `stem` of `head`.
fn stem_heads(host: &mut dyn Host, head: Value) -> Result<Vec<Value>, Exception> {
    match grob_object(host, head, "stem")? {
        Some(stem) => grob_array::extract_grob_array(host, stem, "note-heads"),
        None => Ok(Vec::new()),
    }
}

/// `check_meshing_chords`: how far, in widths of the down-stem column's
/// support head, the up-stem column `clash_up` shifts right of the
/// down-stem column `clash_down` (left for less than 0); 0 for columns
/// too far apart to collide or merged. Merged heads are made transparent,
/// the dots of one of them killed; the dot columns are given the heads
/// they must clear and the dots their direction.
fn check_meshing_chords(
    host: &mut dyn Host,
    me: Value,
    clash_up: Value,
    clash_down: Value,
) -> Result<f64, Exception> {
    // Every note column should have a stem, but avoid a crash.
    let Some(stem_up) = note_column::get_stem(host, clash_up)? else {
        return Ok(0.0);
    };
    let Some(stem_down) = note_column::get_stem(host, clash_down)? else {
        return Ok(0.0);
    };
    let fh_up = note_column::first_head(host, clash_up)?;
    let fh_down = note_column::first_head(host, clash_down)?;
    let sh_up = note_column::support_head(host, clash_up)?;
    let sh_down = note_column::support_head(host, clash_down)?;
    let (Some(fh_up), Some(fh_down), Some(sh_up), Some(sh_down)) = (fh_up, fh_down, sh_up, sh_down)
    else {
        return Ok(0.0);
    };
    let extent_up = grob::extent(host, sh_up, sh_up, X)?;
    let extent_down = grob::extent(host, sh_down, sh_down, X)?;

    // Staff positions of all note heads on each stem.
    let ups = stem::note_head_positions(host, stem_up, false)?;
    let dps = stem::note_head_positions(host, stem_down, false)?;
    let (Some(&up0), Some(&dlast)) = (ups.first(), dps.last()) else {
        return Ok(0.0);
    };
    let v = grob::property(host, me, "note-collision-threshold")?;
    let threshold = int_or(host.heap(), v, 1);

    // Too far apart to collide.
    if up0 > dlast + threshold {
        return Ok(0.0);
    }

    // If the chords just 'touch' their extreme note heads, their stems
    // can be aligned.
    let mut touch = up0 >= dlast
        && (dps.len() < 2 || up0 > dps[dps.len() - 2] + threshold)
        && (ups.len() < 2 || ups[1] > dlast + threshold);

    // The heads off the main column take no part in the collision.
    let ups = stem::note_head_positions(host, stem_up, true)?;
    let dps = stem::note_head_positions(host, stem_down, true)?;

    // Merge heads if the notes lie on the same line, or if the up-stem
    // note is above the down-stem note.
    let mut merge_possible = match (ups.first(), ups.last(), dps.first(), dps.last()) {
        (Some(u0), Some(un), Some(d0), Some(dn)) => u0 >= d0 && un >= dn,
        _ => false,
    };

    // Do not merge notes typeset in different style.
    let up_style = grob::property(host, fh_up, "style")?;
    let down_style = grob::property(host, fh_down, "style")?;
    if !host.heap().equal(up_style, down_style) {
        merge_possible = false;
    }

    let up_ball_type = rhythmic_head::duration_log(host, fh_up)?;
    let down_ball_type = rhythmic_head::duration_log(host, fh_down)?;

    // Do not merge whole notes (or longer, like breve, longa, maxima).
    if merge_possible && (up_ball_type <= 0 || down_ball_type <= 0) {
        merge_possible = false;
    }

    if merge_possible
        && rhythmic_head::dot_count(host, fh_up)? != rhythmic_head::dot_count(host, fh_down)?
        && !is_true(host, me, "merge-differently-dotted")?
    {
        merge_possible = false;
    }

    // Different heads merge only if merge-differently-headed is set.
    if merge_possible
        && up_ball_type != down_ball_type
        && !is_true(host, me, "merge-differently-headed")?
    {
        merge_possible = false;
    }

    // Never merge quarter and half notes, which would make them
    // indistinguishable.
    if merge_possible {
        let (up_log, down_log) = (
            stem::duration_log(host, stem_up)?,
            stem::duration_log(host, stem_down)?,
        );
        if (up_log == 1 && down_log == 2) || (up_log == 2 && down_log == 1) {
            merge_possible = false;
        }
    }

    // A distant half collision (the heads a second apart, the up-stem's
    // lower) or a close one (the up-stem's higher), whose heads may be
    // closer.
    let mut close_half_collide = false;
    let mut distant_half_collide = false;
    let mut full_collide = false;
    let (mut i, mut j) = (0, 0);
    while i < ups.len() && j < dps.len() {
        if ups[i] == dps[j] {
            full_collide = true;
        } else if (ups[i] - dps[j]).abs() <= threshold {
            merge_possible = false;
            if ups[i] > dps[j] {
                close_half_collide = true;
            } else {
                distant_half_collide = true;
            }
        } else if (ups[i] > dps[0] && ups[i] < dps[dps.len() - 1])
            || (dps[j] > ups[0] && dps[j] < ups[ups.len() - 1])
        {
            merge_possible = false;
        }

        if ups[i] < dps[j] {
            i += 1;
        } else if ups[i] > dps[j] {
            j += 1;
        } else {
            i += 1;
            j += 1;
        }
    }

    full_collide = full_collide
        || (close_half_collide && distant_half_collide)
        // like full for wholes and longer
        || (distant_half_collide && (up_ball_type <= 0 || down_ball_type <= 0));

    // Which chord goes left and which right: the up-stem usually right,
    // but chords that just touch can share a stem line; in a collision
    // the right-hand heads may obscure dots, so the dotted heads go right.
    let mut shift_amount: f64 = 1.0;
    let mut stem_to_stem = false;
    if (full_collide
        || ((close_half_collide || distant_half_collide)
            && is_true(host, me, "prefer-dotted-right")?))
        && rhythmic_head::dot_count(host, fh_up)? < rhythmic_head::dot_count(host, fh_down)?
    {
        shift_amount = -1.0;
        if !touch {
            // Clearance between the stems.
            stem_to_stem = true;
        }
    } else if touch {
        // An up-stem note on a line has a raised dot: no risk of collision.
        let is_on_staff_line = |host: &mut dyn Host| -> Result<bool, Exception> {
            match (grob_object(host, me, "staff-symbol")?, ups.first()) {
                (Some(staff), Some(&up0)) => on_line(host, staff, up0),
                _ => Ok(false),
            }
        };
        if (full_collide || (!is_on_staff_line(host)? && is_true(host, me, "prefer-dotted-right")?))
            && rhythmic_head::dot_count(host, fh_up)? > rhythmic_head::dot_count(host, fh_down)?
        {
            touch = false;
        } else {
            shift_amount = -1.0;
        }
    }

    // The fa shape note heads are triangles turned by the stem's
    // direction: of two merged, one is hidden, so that they do not look
    // like a rectangular block.
    let fa_styles = grob::property(host, me, "fa-styles")?;
    let up_style = grob::property(host, fh_up, "style")?;
    let down_style = grob::property(host, fh_down, "style")?;
    if merge_possible
        && memq(host.heap(), up_style, fa_styles)
        && memq(host.heap(), down_style, fa_styles)
    {
        // Which shape is shown.
        let v = grob::property(host, me, "fa-merge-direction")?;
        let d = direction_or(v, DOWN);
        let heap = host.heap();
        grob::set_property(
            heap,
            if d == UP { fh_down } else { fh_up },
            "transparent",
            Value::TRUE,
        );
        // The stem attached smoothly to the glyph.
        let up_att = crate::offset(heap, (0.0, if d == UP { 0.5 } else { -1.0 }));
        let down_att = crate::offset(heap, (0.0, if d == DOWN { -0.5 } else { 1.0 }));
        grob::set_property(heap, fh_up, "stem-attachment", up_att);
        grob::set_property(heap, fh_down, "stem-attachment", down_att);
    }

    if merge_possible {
        shift_amount = 0.0;

        // If possible, no head is wiped; else the shortest head, or the
        // one with fewer dots. Merging different heads, the smaller one's
        // dots disappear; merging identical heads, the down-stem head's.
        let mut wipe_ball = None;
        let mut dot_wipe_head = fh_up;
        if up_ball_type == down_ball_type {
            // A hidden head was taken to affect the layout so far; now
            // that one is eliminated, one already hidden will do.
            let down_dots = visible_dot_count(host, fh_down)?;
            let up_dots = visible_dot_count(host, fh_up)?;
            if down_dots < up_dots {
                wipe_ball = Some(fh_down);
                dot_wipe_head = fh_down;
            } else if down_dots > up_dots {
                dot_wipe_head = fh_up;
                wipe_ball = Some(fh_up);
            } else {
                dot_wipe_head = fh_down;
            }
        } else if down_ball_type > up_ball_type {
            wipe_ball = Some(fh_down);
            dot_wipe_head = fh_down;
        } else if down_ball_type < up_ball_type {
            wipe_ball = Some(fh_up);
            dot_wipe_head = fh_up;
            // An eighth or shorter on a half note: shifted by the
            // difference of the open and filled heads' widths, else the
            // upper stem is slightly misaligned.
            if stem::duration_log(host, stem_down)? == 1 && stem::duration_log(host, stem_up)? >= 3
            {
                shift_amount = (1.0 - extent_up.right / extent_down.right) * 0.5;
            }
        }

        if let Some(d) = grob_object(host, dot_wipe_head, "dot")? {
            grob::suicide(host.heap(), d);
        }
        if let Some(w) = wipe_ball
            && grob::is_live_grob(host.heap(), w)
        {
            grob::set_property(host.heap(), w, "transparent", Value::TRUE);
        }
    }
    // These numbers are LilyPond's magic.
    else if stem_to_stem {
        shift_amount *= 0.65;
    } else if touch {
        shift_amount *= 0.5;
    } else if close_half_collide {
        shift_amount *= 0.52;
    } else if full_collide {
        shift_amount *= 0.5;
    } else if distant_half_collide {
        shift_amount *= 0.4;
    }
    // Meshing.
    else if rhythmic_head::dot_count(host, fh_up)? != 0
        || rhythmic_head::dot_count(host, fh_down)? != 0
    {
        shift_amount *= 0.1;
    } else {
        shift_amount *= 0.17;
    }

    // calc_positioning_done multiplies the shift by the width of the
    // down-stem note; the shift that clears a collision depends on the
    // extents of the heads on the sides that meet.
    if shift_amount < 0.0 {
        // The down-stem shifts right.
        shift_amount *= (extent_up.at(RIGHT) - extent_down.at(LEFT)) / extent_down.length();
    } else {
        // The up-stem shifts right.
        shift_amount *= (extent_down.at(RIGHT) - extent_up.at(LEFT)) / extent_down.length();
    }

    // Dotted notes on the left: their dot column avoids the heads on the
    // right.
    if shift_amount < -1e-6 && rhythmic_head::dot_count(host, fh_up)? != 0 {
        if let Some(parent) = dot_column_of(host, fh_up)? {
            side_position::add_support(host, parent, fh_down)?;
        }
    } else if rhythmic_head::dot_count(host, fh_down)? != 0
        && let Some(parent) = dot_column_of(host, fh_down)?
    {
        // All heads on the up-stem, to clear any suspended on its right.
        for h in stem_heads(host, fh_up)? {
            side_position::add_support(host, parent, h)?;
        }
    }

    // Meshed chords with dots on the left: the dots' direction.
    if shift_amount > 1e-6 && rhythmic_head::dot_count(host, fh_down)? != 0 {
        let dot_down = grob_object(host, fh_down, "dot")?;
        let col_down = dot_down.and_then(|d| grob::get_parent(host.heap(), d, X));
        let mut dir = UP;
        if rhythmic_head::dot_count(host, fh_up)? != 0 {
            let dot_up = grob_object(host, fh_up, "dot")?;
            let col_up = dot_up.and_then(|d| grob::get_parent(host.heap(), d, X));
            if col_up == col_down {
                // The common dot column arranges the dots.
                dir = 0;
            } else if let Some(dot_up) = dot_up {
                // As the dots of the up-stem chord.
                let v = grob::property(host, dot_up, "direction")?;
                dir = direction_or(v, UP);
            }
        }
        if dir != 0 {
            let v = host.heap().integer(i64::from(dir));
            for h in stem_heads(host, fh_down)? {
                if let Some(dot) = grob_object(host, h, "dot")? {
                    grob::set_property(host.heap(), dot, "direction", v);
                }
            }
        }
    }

    Ok(shift_amount)
}

/// The `dot-count` of `head`'s dots (`from_scm (…, 0)`), 0 for none or
/// transparent ones.
fn visible_dot_count(host: &mut dyn Host, head: Value) -> Result<i32, Exception> {
    let Some(dots) = rhythmic_head::get_dots(host, head)? else {
        return Ok(0);
    };
    if is_true(host, dots, "transparent")? {
        return Ok(0);
    }
    let v = grob::property(host, dots, "dot-count")?;
    Ok(int_or(host.heap(), v, 0))
}

/// `calc_positioning_done`: the note columns of `me` shifted on X apart
/// from each other, by `force-hshift` where it is set, else by
/// [`automatic_shift`], in widths of the last clash group's first
/// column's support head, the automatic leftmost at 0. `#t`, and `me`'s
/// `positioning-done` set to it first.
pub fn calc_positioning_done(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    grob::set_property(host.heap(), me, "positioning-done", Value::TRUE);
    let clash_groups = get_clash_groups(host, me)?;
    for d in [UP, DOWN] {
        // The columns' own positioning triggered.
        for &c in clash_groups[ix(d)].iter().rev() {
            grob::extent(host, c, me, X)?;
        }
    }

    let autos = automatic_shift(host, me, &clash_groups)?;
    let hand = forced_shift(host, me)?;

    let mut wid = 0.0;
    for d in [UP, DOWN] {
        if let Some(&h) = clash_groups[ix(d)].first()
            && let Some(sh) = note_column::support_head(host, h)?
        {
            wid = grob::extent(host, sh, h, X)?.length();
        }
    }

    let mut done = Vec::new();
    let mut amounts = Vec::new();
    let mut left_most = 0.0;
    for (s, force) in hand {
        let amount = host.heap().to_f64(force).unwrap_or(0.0) * wid;
        done.push(s);
        amounts.push(amount);
    }
    for (s, offset) in autos {
        let amount = offset * wid;
        if !done.contains(&s) {
            done.push(s);
            amounts.push(amount);
            if amount < left_most {
                left_most = amount;
            }
        }
    }

    for (&g, &amount) in done.iter().zip(&amounts) {
        if let Err(text) = grob::translate_axis(host.heap(), g, amount - left_most, X) {
            grob::programming_error(host, g, text)?;
        }
    }
    Ok(Value::TRUE)
}

/// `get_clash_groups`: `me`'s note columns by direction (down, up), each
/// sorted by `horizontal-shift`; one without a direction is left out
/// after LilyPond's programming error.
fn get_clash_groups(host: &mut dyn Host, me: Value) -> Result<[Vec<Value>; 2], Exception> {
    let mut clash_groups: [Vec<Value>; 2] = [Vec::new(), Vec::new()];
    for se in grob_array::extract_grob_array(host, me, "elements")? {
        if grob::has_interface(host.heap(), se, "note-column-interface") {
            let d = note_column::dir(host, se)?;
            if d == 0 {
                grob::programming_error(host, se, "note-column has no direction")?;
            } else {
                clash_groups[ix(d)].push(se);
            }
        }
    }
    for clashes in &mut clash_groups {
        if clashes.len() < 2 {
            continue;
        }
        let mut keyed = Vec::with_capacity(clashes.len());
        for &c in clashes.iter() {
            keyed.push((note_column::horizontal_shift(host, c)?, c));
        }
        lily_engine_skyline::sort(&mut keyed, &|a: &(i32, Value), b: &(i32, Value)| a.0 < b.0);
        *clashes = keyed.into_iter().map(|(_, c)| c).collect();
    }
    Ok(clash_groups)
}

/// `automatic_shift`: each note column's shift, in head widths, from the
/// meshing of the first columns up and down ([`check_meshing_chords`]):
/// each further column of a direction moves away from the stems by a
/// head, or half a one, unless its `horizontal-shift` equals the one
/// before, with LilyPond's warning where it has none. The dot columns of
/// the up-stem columns are given the stems of the columns after them. In
/// LilyPond's list's order: the last down-stem column first.
fn automatic_shift(
    host: &mut dyn Host,
    me: Value,
    clash_groups: &[Vec<Value>; 2],
) -> Result<Vec<(Value, f64)>, Exception> {
    let mut extents: [Vec<[i32; 2]>; 2] = [Vec::new(), Vec::new()];
    let mut extent_union = [[i32::MAX, -i32::MAX]; 2];
    let mut stems: [Vec<Option<Value>>; 2] = [Vec::new(), Vec::new()];
    for d in [UP, DOWN] {
        for &c in &clash_groups[ix(d)] {
            let [l, r] = note_column::head_positions_interval(host, c)?;
            let s = [l - 1, r + 1];
            extents[ix(d)].push(s);
            let u = &mut extent_union[ix(d)];
            *u = [u[0].min(s[0]), u[1].max(s[1])];
            stems[ix(d)].push(note_column::get_stem(host, c)?);
        }
    }
    // A slice's end `e`.
    let at = |s: [i32; 2], e: Direction| s[ix(e)];
    let valid = |host: &mut dyn Host, s: Option<Value>| match s {
        Some(s) => stem::is_valid_stem(host, s),
        None => Ok(false),
    };

    let inner_offset = match (clash_groups[ix(UP)].first(), clash_groups[ix(DOWN)].first()) {
        (Some(&up), Some(&down)) => check_meshing_chords(host, me, up, down)?,
        _ => 0.0,
    };

    // The horizontal shifts of each direction.
    let mut offsets: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
    for d in [UP, DOWN] {
        let (k, o) = (ix(d), ix(-d));
        let mut offset = inner_offset;
        let mut shifts = Vec::new();
        for (i, &col) in clash_groups[k].iter().enumerate() {
            let sh = grob::property(host, col, "horizontal-shift")?;
            shifts.push(int_or(host.heap(), sh, 0));
            if i == 0 {
                offset = inner_offset;
            } else {
                let explicit_shift = host.heap().is_number(sh);
                if !explicit_shift {
                    grob::warning(
                        host,
                        col,
                        "this Voice needs a \\voiceXx or \\shiftXx setting",
                    )?;
                }
                let (e, prev) = (extents[k][i], extents[k][i - 1]);
                if explicit_shift && shifts[i] == shifts[i - 1] {
                    // The previous note column's offset.
                } else if at(e, UP) > at(prev, DOWN) && at(e, DOWN) < at(prev, UP) {
                    // Clear of the previous note column's heads.
                    offset += 1.0;
                } else if d * at(e, -d) >= d * at(prev, d) {
                    // Across the previous note column.
                    offset += if valid(host, stems[k][i - 1])? {
                        1.0
                    } else {
                        0.5
                    };
                } else if valid(host, stems[k][i])? {
                    offset += 0.5;
                }

                // Across the opposite-stemmed voices.
                if d * at(e, -d) < d * at(extent_union[o], d) && offset < 0.5 {
                    offset = 0.5;
                }
                if let Some(&other) = extents[o].first()
                    && at(e, UP) > at(other, DOWN)
                    && at(e, DOWN) < at(other, UP)
                    && offset < 1.0
                {
                    offset = 1.0;
                }
            }
            offsets[k].push(f64::from(d) * offset);
        }
    }

    // input/regression/dot-up-voice-collision.ly
    let ups = &clash_groups[ix(UP)];
    for (i, &g) in ups.iter().enumerate() {
        if let Some(dc) = note_column::dot_column(host, g)? {
            for s in stems[ix(UP)][i + 1..].iter().flatten() {
                side_position::add_support(host, dc, *s)?;
            }
        }
    }

    let mut tups = Vec::new();
    for d in [UP, DOWN] {
        for (&c, &offset) in clash_groups[ix(d)].iter().zip(&offsets[ix(d)]) {
            tups.push((c, offset));
        }
    }
    tups.reverse();
    Ok(tups)
}

/// `forced_shift`: the elements of `me` with a number as their
/// `force-hshift`, and it, in LilyPond's list's order (the last first).
fn forced_shift(host: &mut dyn Host, me: Value) -> Result<Vec<(Value, Value)>, Exception> {
    let mut tups = Vec::new();
    for se in grob_array::extract_grob_array(host, me, "elements")? {
        let force = grob::property(host, se, "force-hshift")?;
        if host.heap().is_number(force) {
            tups.push((se, force));
        }
    }
    tups.reverse();
    Ok(tups)
}

/// `add_column`: the note column `ncol` made an element of the collision
/// `me`, its `X-offset` `ly:grob::x-parent-positioning`.
pub fn add_column(host: &mut dyn Host, me: Value, ncol: Value) -> Result<(), Exception> {
    let proc = crate::lily_procedure(host, "ly:grob::x-parent-positioning")?;
    grob::set_property(host.heap(), ncol, "X-offset", proc);
    axis_group::add_element(host, me, ncol)
}

/// What `%host-call` runs for
/// `ly:note-collision-interface::calc-positioning-done` (D127):
/// [`calc_positioning_done`] of the grob given, with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "note-collision",
    f: host_collision,
}];

fn host_collision(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    calc_positioning_done(vm, args[1])
}

/// `(ly:note-collision-interface::calc-positioning-done grob)`:
/// LilyPond's subr, a primitive here so that it prints as LilyPond's; it
/// checks its grob and tail-calls `%host-call` ([`HOSTS`]).
pub(crate) fn ly_calc_positioning_done(
    cx: &mut Cx,
    args: &[Value],
) -> Result<Value, lily_engine_scheme::Thrown> {
    const SUBR: &str = "ly:note-collision-interface::calc-positioning-done";
    grob::grob_arg(cx, SUBR, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("note-collision");
    Ok(cx.tail_call(f, &[what, args[0]]))
}
