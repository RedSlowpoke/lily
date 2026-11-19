//! `tuplet-number.cc` (MS4 step 13): the TupletNumber's callbacks, the
//! music crate's placeholders until then (D67): `ly:tuplet-number::print`
//! (`:225–243`), `::calc-x-offset` (`:267–339`) and `::calc-y-offset`
//! (`:341–459`), each the bracket's middle, or against a kneed beam where
//! the bracket is hidden and the number fits between its neighbours
//! there; with `select_reference_stem` (`:43–107`),
//! `adjacent_note_columns` (`:109–144`), `knee_position_against_beam`
//! (`:146–223`) and `calc_beam_y_shift` (`:245–261`).
//!
//! The callbacks are subrs that print as LilyPond's and call their Rust
//! over `%host-call` (D127, [`HOSTS`]), in 05 §5.1's callback table
//! (`callbacks.rs`) too. Where clang fuses a product into a sum in
//! LilyPond's arm64 binary, the sum is [`fma`]'s.

use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::bracket::real_drul;
use crate::callbacks::Callback;
use crate::direction::get_grob_direction;
use crate::geometry::{Direction, Interval, X, Y, fma, max, min};
use crate::grob;
use crate::grob_array;
use crate::hairpin::generic_bound_extent;
use crate::note_column;
use crate::spacing_options::double_or;
use crate::spanner;
use crate::staff_symbol_referencer as ssr;
use crate::stem;
use crate::stencil::Stencil;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const DOWN: Direction = -1;
const UP: Direction = 1;

type Run<T> = Result<T, Exception>;

/// A `Drul_array`'s index of `d`: 0 for `LEFT`, 1 for `RIGHT`.
fn side(d: Direction) -> usize {
    usize::from(d == RIGHT)
}

/// `from_scm<double> (get_property (me, name), fallback)`.
fn real_or(host: &mut dyn Host, me: Value, name: &str, fallback: f64) -> Run<f64> {
    let v = grob::property(host, me, name)?;
    Ok(double_or(host, v, fallback))
}

/// `from_scm<bool> (get_property (me, name))`: `#t` only.
fn is_true(host: &mut dyn Host, me: Value, name: &str) -> Run<bool> {
    Ok(grob::property(host, me, name)? == Value::TRUE)
}

/// The grob in the object `name` of `me`, if it is one.
fn grob_object(host: &mut dyn Host, me: Value, name: &str) -> Run<Option<Value>> {
    let sym = host.heap().intern(name);
    let v = grob::object(host, me, sym)?;
    Ok(grob_of(host.heap(), v).map(|_| v))
}

/// The number's `bracket`, read through as LilyPond's is.
fn bracket(host: &mut dyn Host, me: Value) -> Run<Value> {
    Ok(grob_object(host, me, "bracket")?.unwrap_or(Value::NULL))
}

/// `select_reference_stem`: the stem whose side of the beam the number is
/// not on: of an odd number of columns the middle one's; of an even
/// number, of the middle two, the left where they point the same way or
/// have as many beams towards each other, the one pointing the number's
/// `direction`, else the one with more.
///
/// The reference stem is used to determine on which side of the beam to place
/// the tuplet number when it is positioned independently of a bracket.  (The number
/// is always placed on the opposite side of this stem.)
fn select_reference_stem(host: &mut dyn Host, me: Value, cols: &[Value]) -> Run<Option<Value>> {
    let col_count = cols.len();

    if col_count == 0 {
        return Ok(None);
    }

    /*
      When we have an odd number of stems, we choose the middle stem as
      our reference.
    */
    let ref_stem = note_column::get_stem(host, cols[col_count / 2])?;

    if col_count % 2 == 1 {
        return Ok(ref_stem);
    }

    /*
      When we have an even number of stems, we choose between the central
      two stems.
    */
    let v = grob::property(host, me, "direction")?;
    let me_dir = match v.as_fixnum() {
        Some(d @ -1..=1) => d as Direction,
        _ => UP,
    };
    let bounding_stems = [
        note_column::get_stem(host, cols[col_count / 2 - 1])?,
        ref_stem,
    ];

    let (left, right) = match bounding_stems {
        [None, r] => return Ok(r),
        [l, None] => return Ok(l),
        [Some(l), Some(r)] => (l, r),
    };

    /*
      If the central stems point in opposite directions, the number may
      be placed on either side unless there is a fractional beam, in which
      case the number goes opposite to the partial beam.

      When there is an option, we use the setting of TupletNumber.direction.

      If the central stems are in the same direction, it doesn't matter
      which is used as the reference.  We use the one on the left.
    */
    let dir_left = get_grob_direction(host, left)?;
    let dir_right = get_grob_direction(host, right)?;

    let ref_stem = if dir_left == dir_right {
        left
    } else {
        let beam_count_l_r = stem::get_beaming(host, left, RIGHT)?;
        let beam_count_r_l = stem::get_beaming(host, right, LEFT)?;
        if beam_count_l_r == beam_count_r_l {
            if dir_left == me_dir { left } else { right }
        } else if beam_count_l_r > beam_count_r_l {
            left
        } else {
            right
        }
    };

    Ok(Some(ref_stem))
}

/// `adjacent_note_columns`: the bracket's columns either side of the
/// reference stem's whose stems point the other way.
///
/// When we place the number close to the beam, we need to consider the note
/// columns adjoining the tuplet number on the same side of the beam.  The
/// number may not fit in the available space, or may need to be shifted
/// horizontally out of the way of stems and ledger lines.
fn adjacent_note_columns(
    host: &mut dyn Host,
    me: Value,
    ref_stem: Value,
) -> Run<[Option<Value>; 2]> {
    let tuplet = bracket(host, me)?;

    let columns = grob_array::extract_grob_array(host, tuplet, "note-columns")?;
    let ref_col = grob::get_parent(host.heap(), ref_stem, X); // X-parent of Stem = NoteColumn
    let ref_stem_dir = get_grob_direction(host, ref_stem)?;
    let mut filtered_cols: Vec<Value> = Vec::new();
    let mut ref_pos: usize = 0;

    let mut counter = 0;
    for &col in &columns {
        if let Some(stem) = note_column::get_stem(host, col)?
            && get_grob_direction(host, stem)? == -ref_stem_dir
        {
            filtered_cols.push(col);
            counter += 1;
        }
        if Some(col) == ref_col {
            filtered_cols.push(col);
            ref_pos = counter;
        }
    }

    Ok([
        ref_pos.checked_sub(1).map(|k| filtered_cols[k]),
        (ref_pos + 1 < filtered_cols.len()).then(|| filtered_cols[ref_pos + 1]),
    ])
}

/// `knee_position_against_beam`: whether the number goes against its
/// reference stem's beam.
///
/// We determine whether our tuplet number will be put next to the beam
/// independently of the positioning of the associated tuplet bracket.
///
/// Draw next to the beam if:
/// --bracket isn't visible, AND
/// --there is a beam above or below the number, AND
/// --this beam is kneed, AND
/// --the tuplet number will fit between adjoining note columns
fn knee_position_against_beam(host: &mut dyn Host, me: Value, ref_stem: Value) -> Run<bool> {
    let tuplet = bracket(host, me)?;

    let bracket_visible = is_true(host, me, "bracket-visibility")?
        || !grob::extent(host, tuplet, tuplet, Y)?.is_empty();

    if bracket_visible || !is_true(host, me, "knee-to-beam")? {
        return Ok(false);
    }

    let beam = stem::get_beam(host, ref_stem)?;

    let Some(beam) = beam else {
        return Ok(false);
    };
    if !is_true(host, beam, "knee")? {
        return Ok(false);
    }

    let Some(commonx) = spanner::get_system(host.heap(), me) else {
        grob::unlocated_programming_error(
            host,
            "Tuplet_number::knee_position_against_beam called before line breaking",
        )?;
        return Ok(true);
    };

    let number_ext = grob::extent(host, me, commonx, X)?;

    let adj_cols = adjacent_note_columns(host, me, ref_stem)?;

    let bounds = [
        spanner::get_bound(host, me, LEFT)?,
        spanner::get_bound(host, me, RIGHT)?,
    ];
    let [Some(_), Some(_)] = bounds else {
        return Ok(false);
    };

    let mut available_ext = Interval::EMPTY;
    let padding = real_or(host, me, "padding", 0.5)?;

    /*
       If there is no note column on a given side of the tuplet number, we use
       a paper column instead to determine the available space.  Padding is only
       considered in the case of a note column.
    */
    for d in [LEFT, RIGHT] {
        *available_ext.at_mut(d) = match adj_cols[side(d)] {
            Some(adj) => {
                let e = generic_bound_extent(host, adj, commonx)?.at(-d);
                fma(f64::from(-d), padding, e)
            }
            None => {
                let b = bounds[side(d)].unwrap_or(Value::NULL);
                generic_bound_extent(host, b, commonx)?.at(-d)
            }
        };
    }

    if number_ext.length() > available_ext.length() {
        grob::unlocated_programming_error(host, "not enough space for tuplet number against beam")?;
        return Ok(false);
    }

    Ok(true)
}

/// `print`: the number's text centred on both axes; a number whose bracket
/// is dead is killed.
pub fn print(host: &mut dyn Host, me: Value) -> Run<Value> {
    let tuplet = grob_object(host, me, "bracket")?;

    if !tuplet.is_some_and(|t| grob::is_live_grob(host.heap(), t)) {
        grob::suicide(host.heap(), me);
        return Ok(Value::NULL);
    }

    let v = crate::call_lily(host, "ly:text-interface::print", &[me])?;
    let heap = host.heap();
    let mut stc = heap.foreign::<Stencil>(v).copied().unwrap_or_default();

    stc.align_to(heap, X, 0.0);
    stc.align_to(heap, Y, 0.0);

    Ok(heap.wrap(stc))
}

/// `calc_beam_y_shift`: how far the reference stem's beam rises over `dx`.
///
/// For a given horizontal displacement of the tuplet number, how much
/// vertical shift is necessary to keep it the same distance from the beam?
fn calc_beam_y_shift(host: &mut dyn Host, ref_stem: Value, dx: f64) -> Run<f64> {
    let beam = stem::get_beam(host, ref_stem)?.unwrap_or(Value::NULL);
    let v = grob::property(host, beam, "X-positions")?;
    let x_pos = real_drul(host.heap(), v, [0.0, 0.0]);
    let v = grob::property(host, beam, "quantized-positions")?;
    let y_pos = real_drul(host.heap(), v, [0.0, 0.0]);
    let beam_dx = Interval::new(x_pos[0], x_pos[1]).length();
    let beam_dy = y_pos[1] - y_pos[0];
    let slope = if beam_dx != 0.0 {
        beam_dy / beam_dx
    } else {
        0.0
    };

    Ok(slope * dx)
}

/// `calc_x_offset`: the middle of the bracket's `X-positions`; against a
/// kneed beam, the middle between its bounds' stems, moved off the
/// neighbouring columns it overlaps.
///
/// The X- and Y-offset of the tuplet number are calculated in relation either
/// to the bracket associated with it, or with the beam it is placed against.
pub fn calc_x_offset(host: &mut dyn Host, me: Value) -> Run<Value> {
    let mut bounds = [
        spanner::get_bound(host, me, LEFT)?.unwrap_or(Value::NULL),
        spanner::get_bound(host, me, RIGHT)?.unwrap_or(Value::NULL),
    ];
    let left_bound = bounds[0];

    let tuplet = bracket(host, me)?;

    let Some(commonx) = spanner::get_system(host.heap(), me) else {
        grob::unlocated_programming_error(
            host,
            "TupletNumber.X-offset accessed before line breaking",
        )?;
        return Ok(host.heap().integer(0));
    };

    let mut bound_poss = Interval::new(0.0, 0.0);

    for d in [LEFT, RIGHT] {
        let b = bounds[side(d)];
        if grob::has_interface(host.heap(), b, "note-column-interface")
            && let Some(s) = note_column::get_stem(host, b)?
        {
            bounds[side(d)] = s;
        }
        *bound_poss.at_mut(d) = generic_bound_extent(host, bounds[side(d)], commonx)?.at(-d);
    }

    let cols = grob_array::extract_grob_array(host, tuplet, "note-columns")?;
    let ref_stem = select_reference_stem(host, me, &cols)?;

    /*
      Return bracket-based positioning.
    */
    let against_beam = match ref_stem {
        Some(r) => knee_position_against_beam(host, me, r)?,
        None => false,
    };
    let Some(ref_stem) = ref_stem.filter(|_| against_beam) else {
        let v = grob::property(host, tuplet, "X-positions")?;
        let x_positions = real_drul(host.heap(), v, [0.0, 0.0]);
        let center = Interval::new(x_positions[0], x_positions[1]).center();
        return Ok(host.heap().flonum(center));
    };

    /*
       Horizontally center the number on the beam.
    */
    let col_pos = grob::relative_coordinate(host, left_bound, commonx, X)?;
    let mut x_offset = bound_poss.center() - col_pos;

    /*
      Consider possible collisions with adjacent note columns.
    */
    let adj_cols = adjacent_note_columns(host, me, ref_stem)?;
    let mut number_ext = grob::extent(host, me, commonx, X)?;
    number_ext.translate(x_offset);
    let padding = real_or(host, me, "padding", 0.5)?;
    number_ext.widen(padding);

    let mut cor = [0.0, 0.0];

    for d in [LEFT, RIGHT] {
        if let Some(adj) = adj_cols[side(d)] {
            let nc_ext = grob::extent(host, adj, commonx, X)?;
            let mut overlap = nc_ext;
            overlap.intersect(number_ext);
            if !overlap.is_empty() {
                cor[side(d)] = overlap.length() * f64::from(-d);
            }
            x_offset += cor[side(d)];
        }
    }

    Ok(host.heap().flonum(x_offset))
}

/// `calc_y_offset`: the middle of the bracket's `positions`; against a
/// kneed beam, its padding past the reference stem's end along the beam,
/// moved into the staff rather than over its ledger lines, and off the
/// accidentals of the column after it.
pub fn calc_y_offset(host: &mut dyn Host, me: Value) -> Run<Value> {
    let tuplet = bracket(host, me)?;
    let v = grob::property(host, tuplet, "positions")?;
    let positions = real_drul(host.heap(), v, [0.0, 0.0]);
    let to_bracket = (positions[0] + positions[1]) / 2.0;

    let Some(commonx) = spanner::get_system(host.heap(), me) else {
        grob::unlocated_programming_error(
            host,
            "TupletBracket.Y-offset accessed before line breaking",
        )?;
        return Ok(host.heap().integer(0));
    };
    let x_coord = grob::relative_coordinate(host, me, commonx, X)?;
    let columns = grob_array::extract_grob_array(host, tuplet, "note-columns")?;
    let ref_stem = select_reference_stem(host, me, &columns)?;

    let against_beam = match ref_stem {
        Some(r) => knee_position_against_beam(host, me, r)?,
        None => false,
    };
    let Some(ref_stem) = ref_stem.filter(|_| against_beam) else {
        return Ok(host.heap().flonum(to_bracket));
    };

    /*
      First, we calculate the Y-offset of the tuplet number as if it
      is positioned at the reference stem.
    */
    let heap = host.heap();
    let commony = grob::common_refpoint_of_array(heap, &columns, Some(tuplet), Y);
    let mut commony = commony
        .and_then(|c| grob::common_refpoint(heap, c, me, Y))
        .unwrap_or(Value::NULL);
    let tuplets = grob_array::extract_grob_array(host, me, "tuplets")?;
    commony = grob::common_refpoint_of_array(host.heap(), &tuplets, Some(commony), Y)
        .unwrap_or(Value::NULL);
    if let Some(st) = ssr::staff_symbol(host, me)? {
        commony = grob::common_refpoint(host.heap(), st, commony, Y).unwrap_or(Value::NULL);
    }

    let ref_stem_ext = grob::extent(host, ref_stem, commony, Y)?;
    let tuplet_y = grob::relative_coordinate(host, tuplet, commony, Y)?;
    let ref_stem_dir = get_grob_direction(host, ref_stem)?;
    let fdir = f64::from(ref_stem_dir);

    let mut y_offset = ref_stem_ext.at(ref_stem_dir) - tuplet_y;
    let padding = real_or(host, me, "padding", 0.5)?;
    let num_height = grob::extent(host, me, commony, Y)?.length();

    y_offset = fma(fdir, padding + num_height / 2.0, y_offset);

    /*
      Now we adjust the vertical position of the number to reflect
      its actual horizontal placement along the beam.
    */
    let ref_stem_x = grob::relative_coordinate(host, ref_stem, commonx, X)?;
    y_offset += calc_beam_y_shift(host, ref_stem, x_coord - ref_stem_x)?;

    /*
      Check if the number is between the beam and the staff.  If so, it will collide
      with ledger lines.  Move it into the staff.
    */
    if let Some(st) = ssr::staff_symbol(host, ref_stem)? {
        let staff_ext_y = grob::extent(host, st, commony, Y)?;
        let move_ = if ref_stem_dir == DOWN {
            ref_stem_ext.left > staff_ext_y.right
        } else {
            staff_ext_y.left > ref_stem_ext.right
        };
        if move_ {
            let ledger_domain = Interval::new(
                min(staff_ext_y.right, ref_stem_ext.right),
                max(staff_ext_y.left, ref_stem_ext.left),
            );
            let mut num_y = grob::extent(host, me, commony, Y)?;
            num_y.translate(y_offset);
            let mut num_ledger_overlap = num_y;
            num_ledger_overlap.intersect(ledger_domain);
            let line_thickness = ssr::line_thickness(host, st)?;
            let staff_space = ssr::staff_space(host, st)?;
            // Number will touch outer staff line.
            if !num_ledger_overlap.is_empty() && num_ledger_overlap.length() > (staff_space / 2.0) {
                let d = staff_ext_y.at(-ref_stem_dir) - num_y.at(-ref_stem_dir);
                y_offset += fma(line_thickness, fdir, d);
            }
        }
    }

    /*
      Now consider possible collisions with accidentals on the right.  We
      move the accidental away from the beam.
    */
    let adj_cols = adjacent_note_columns(host, me, ref_stem)?;

    let Some(right) = adj_cols[1] else {
        return Ok(host.heap().flonum(y_offset));
    };

    /*
      Collect Y-extents of accidentals that overlap the number
      along the X-axis.
    */
    let heads = grob_array::extract_grob_array(host, right, "note-heads")?;
    let mut colliding_acc_ext_y = Interval::EMPTY;

    for &head in &heads {
        if let Some(acc) = grob_object(host, head, "accidental-grob")? {
            commony = grob::common_refpoint(host.heap(), commony, acc, Y).unwrap_or(Value::NULL);
            let acc_ext_y = grob::extent(host, acc, commony, Y)?;

            let mut num_ext_x = grob::extent(host, me, commonx, X)?;
            num_ext_x.widen(padding);
            let mut overlap_x = num_ext_x;
            let acc_x = grob::extent(host, acc, commonx, X)?;
            overlap_x.intersect(acc_x);

            if !overlap_x.is_empty() {
                colliding_acc_ext_y.unite(acc_ext_y);
            }
        }
    }
    /*
      Does our number intersect vertically with the accidental Y-extents we
      combined above?  If so, move it.
    */
    let mut overlap_acc_y = colliding_acc_ext_y;
    let mut num_ext_y = grob::extent(host, me, commony, Y)?;
    num_ext_y.translate(y_offset);
    overlap_acc_y.intersect(num_ext_y);

    if !overlap_acc_y.is_empty() {
        let d = colliding_acc_ext_y.at(ref_stem_dir) - num_ext_y.at(-ref_stem_dir);
        y_offset += fma(padding, fdir, d);
    }

    Ok(host.heap().flonum(y_offset))
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:tuplet-number::` primitives (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "tuplet-number",
    f: host_tuplet_number,
}];

/// The `ly:tuplet-number::` callbacks [`HOSTS`] runs, by name, which the
/// callback table (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 3] = [
    ("calc-x-offset", calc_x_offset),
    ("calc-y-offset", calc_y_offset),
    ("print", print),
];

fn host_tuplet_number(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such tuplet number callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// A `ly:tuplet-number::` subr: its grob checked under the callback's
/// name, where LilyPond's reads what it is given as a spanner, and
/// `%host-call` tail-called with its name.
fn host_call(cx: &mut Cx, name: &str, args: &[Value]) -> R {
    crate::tie::host_call_of(cx, "tuplet-number", "ly:tuplet-number::", name, args)
}

pub(crate) fn ly_calc_x_offset(cx: &mut Cx, args: &[Value]) -> R {
    host_call(cx, "calc-x-offset", args)
}

pub(crate) fn ly_calc_y_offset(cx: &mut Cx, args: &[Value]) -> R {
    host_call(cx, "calc-y-offset", args)
}

pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    host_call(cx, "print", args)
}
