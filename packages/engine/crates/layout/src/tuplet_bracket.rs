//! `tuplet-bracket.cc` (MS4 step 12): the TupletBracket's callbacks,
//! the music crate's placeholders until then (D67):
//! `ly:tuplet-bracket::calc-x-positions` (`:132–219`), `::print`
//! (`:265–401`, with `make_tuplet_slur`, `:221–257`),
//! `::calc-direction` (`:737–745`), `::calc-positions` (`:747–761`, over
//! `calc_position_and_height`, `:423–735`, which follows the beam it is
//! parallel to, else the outer columns of notes, and clears the columns,
//! the brackets inside it, their numbers and the scripts) and
//! `::calc-cross-staff` (`:849–881`); with `get_x_bound_item`,
//! `equal_bounds` and `bracket_basic_visibility` (`:66–130`), `get_bounds`
//! (`:403–417`), `get_default_dir` (`:766–805`) and what
//! `Tuplet_engraver` calls, `add_column`, `add_script` and
//! `add_tuplet_bracket` (`:807–847`). Its `beam` and `potential-beam` are
//! output-lib.scm's, in the image.
//!
//! The callbacks are subrs that print as LilyPond's and call their Rust
//! over `%host-call` (D127, [`HOSTS`]), in 05 §5.1's callback table
//! (`callbacks.rs`) too. Where clang fuses a product into a sum in
//! LilyPond's arm64 binary, the sum is [`fma`]'s.

use lily_engine_music::{Host, Moment, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Tag, Value, Vm};

use crate::bezier;
use crate::bracket::{drul_property, make_bracket, real_drul};
use crate::callbacks::Callback;
use crate::direction::get_grob_direction;
use crate::geometry::{Direction, Interval, Offset, X, Y, fma, length, max, min};
use crate::grob;
use crate::grob_array;
use crate::hairpin::generic_bound_extent;
use crate::item;
use crate::lookup;
use crate::note_column;
use crate::spacing_options::double_or;
use crate::spanner;
use crate::staff_symbol_referencer as ssr;
use crate::stem;
use crate::stencil::Stencil;
use crate::text;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const DOWN: Direction = -1;
const UP: Direction = 1;

type Run<T> = Result<T, Exception>;

/// A `Drul_array`'s index of `d`: 0 for `LEFT`, 1 for `RIGHT`.
fn side(d: Direction) -> usize {
    usize::from(d == RIGHT)
}

/// `sign`: -1, 0 or 1.
fn sign(x: f64) -> Direction {
    if x < 0.0 {
        -1
    } else if x > 0.0 {
        1
    } else {
        0
    }
}

/// `from_scm<double> (get_property (me, name), fallback)`.
fn real_or(host: &mut dyn Host, me: Value, name: &str, fallback: f64) -> Run<f64> {
    let v = grob::property(host, me, name)?;
    Ok(double_or(host, v, fallback))
}

/// `scm_is_pair`.
fn is_pair(v: Value) -> bool {
    v.tag() == Tag::Pair
}

/// `scm_is_bool`.
fn is_bool(v: Value) -> bool {
    v == Value::TRUE || v == Value::FALSE
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

/// `get_bound (d)`, read through as LilyPond's does: none is `NULL`, which
/// has no interface.
fn bound(host: &mut dyn Host, me: Value, d: Direction) -> Run<Value> {
    Ok(spanner::get_bound(host, me, d)?.unwrap_or(Value::NULL))
}

/// `from_scm (get_property (col, "when"), Moment (0))`.
fn when(host: &mut dyn Host, col: Value) -> Run<Moment> {
    let v = grob::property(host, col, "when")?;
    Ok(host
        .heap()
        .foreign::<Moment>(v)
        .copied()
        .unwrap_or_default())
}

/// `from_scm (get_property (me, "connect-to-neighbor"), Drul_array<bool>
/// ())`: a pair, each `#t` only; neither otherwise.
fn connect_to_neighbor(host: &mut dyn Host, me: Value) -> Run<[bool; 2]> {
    let v = grob::property(host, me, "connect-to-neighbor")?;
    let heap = host.heap();
    Ok(match (heap.car(v), heap.cdr(v)) {
        (Some(l), Some(r)) if is_bool(l) && is_bool(r) => [l == Value::TRUE, r == Value::TRUE],
        _ => [false, false],
    })
}

/// `get_x_bound_item`: the bound at `hdir`, or its note column's stem
/// where that is visible, has a stencil and points to `my_dir`.
fn get_x_bound_item(
    host: &mut dyn Host,
    me: Value,
    hdir: Direction,
    my_dir: Direction,
) -> Run<Value> {
    let mut g = bound(host, me, hdir)?;
    if grob::has_interface(host.heap(), g, "note-column-interface")
        && let Some(s) = note_column::get_stem(host, g)?
        && note_column::dir(host, g)? == my_dir
        && !stem::is_invisible(host, s)?
    {
        let st = grob::property(host, s, "stencil")?;
        if host.heap().foreign::<Stencil>(st).is_some() {
            g = s;
        }
    }
    Ok(g)
}

/// `equal_bounds`: whether both spanners' bounds are in the same columns.
fn equal_bounds(host: &mut dyn Host, s1: Option<Value>, s2: Value) -> Run<bool> {
    let Some(s1) = s1 else {
        return Ok(false);
    };
    for d in [LEFT, RIGHT] {
        let (b1, b2) = (bound(host, s1, d)?, bound(host, s2, d)?);
        let heap = host.heap();
        if item::get_column(heap, b1) != item::get_column(heap, b2) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `bracket_basic_visibility`: hidden over a beam as long as it, unless
/// `bracket-visibility` says otherwise (`#t`, `#f`, `if-no-beam`); shown
/// over the heads opposite a beam that is no knee where
/// `visible-over-note-heads`; hidden for a tuplet of no time unless forced.
pub(crate) fn bracket_basic_visibility(host: &mut dyn Host, me: Value) -> Run<bool> {
    let par_beam = grob_object(host, me, "beam")?.filter(|&b| spanner::is_spanner(host.heap(), b)); // NB may be null
    let equally_long = equal_bounds(host, par_beam, me)?;
    let mut bracket_visibility = !(par_beam.is_some() && equally_long);

    let bracket_vis_prop = grob::property(host, me, "bracket-visibility")?;
    let bracket_prop = bracket_vis_prop != Value::FALSE;
    let heap = host.heap();
    let if_no_beam = Some(bracket_vis_prop) == Some(heap.intern("if-no-beam"));
    let vis_is_bool = is_bool(bracket_vis_prop);

    if vis_is_bool {
        bracket_visibility = bracket_prop;
    } else if if_no_beam {
        bracket_visibility = par_beam.is_none();
    }

    if !vis_is_bool && !bracket_visibility {
        let bracket_over_heads = is_true(host, me, "visible-over-note-heads")?;
        let beam = par_beam.unwrap_or(Value::NULL);
        if bracket_over_heads && !is_true(host, beam, "knee")? {
            let v = grob::property(host, beam, "direction")?;
            let default_dir = match v.as_fixnum() {
                Some(d @ -1..=1) => d as Direction,
                _ => 0,
            };
            let dir = get_grob_direction(host, me)?;

            if default_dir != dir {
                bracket_visibility = true;
            }
        }
    }

    /*  if the tuplet does not span any time, i.e. a single-note tuplet, hide
        the bracket, but still let the number be displayed.
        Only do this if the user has not explicitly specified bracket-visibility = #t.
    */
    if bracket_vis_prop != Value::TRUE {
        let (l, r) = (bound(host, me, LEFT)?, bound(host, me, RIGHT)?);
        let start_col = item::get_column(host.heap(), l).unwrap_or(Value::NULL);
        let end_col = item::get_column(host.heap(), r).unwrap_or(Value::NULL);
        let start_mom = when(host, start_col)?;
        let end_mom = when(host, end_col)?;
        if start_mom.compare(end_mom).is_eq() && item::break_status_dir(host.heap(), l) == 0 {
            bracket_visibility = false;
        }
    }

    Ok(bracket_visibility)
}

/// `calc_x_positions`: from the left bound's (or its stem's) extent to the
/// right one's, the note columns' with `span-all-note-heads`; at a side
/// connected to its neighbour, past the line's edge by `break-overshoot`;
/// at the right of a full-length bracket, its column's left, less
/// `full-length-padding`; relative to the left bound.
pub fn calc_x_positions(host: &mut dyn Host, me: Value) -> Run<Value> {
    let columns = grob_array::extract_grob_array(host, me, "note-columns")?;

    let Some(commonx) = spanner::get_system(host.heap(), me) else {
        // FIXME: ->get_system () should also work before line breaking
        grob::unlocated_programming_error(
            host,
            "TupletBracket.X-positions requested before line breaking",
        )?;
        return Ok(crate::offset(host.heap(), (0.0, 0.0)));
    };

    let dir = get_grob_direction(host, me)?;

    let bounds = [
        get_x_bound_item(host, me, LEFT, dir)?,
        get_x_bound_item(host, me, RIGHT, dir)?,
    ];

    let connect_to_other = connect_to_neighbor(host, me)?;

    let span_note_heads = is_true(host, me, "span-all-note-heads")?;
    let mut bracket_visibility = false;

    if span_note_heads {
        bracket_visibility = bracket_basic_visibility(host, me)?;
    }

    let mut x_span = Interval::new(0.0, 0.0);
    for d in [LEFT, RIGHT] {
        let b = bounds[side(d)];
        let parent = grob::get_parent(host.heap(), b, X);
        *x_span.at_mut(d) = match parent {
            Some(p)
                if bracket_visibility
                    && grob::has_interface(host.heap(), p, "note-column-interface") =>
            {
                generic_bound_extent(host, p, commonx)?.at(d)
            }
            _ => generic_bound_extent(host, b, commonx)?.at(d),
        };

        if connect_to_other[side(d)] {
            let v = grob::property(host, me, "break-overshoot")?;
            let overshoot = real_drul(host.heap(), v, [-0.5, 0.0]);

            if d == RIGHT {
                *x_span.at_mut(d) += f64::from(d) * overshoot[side(d)];
            } else {
                let edge = if item::break_status_dir(host.heap(), b) != 0 {
                    generic_bound_extent(host, b, commonx)?.at(-d)
                } else {
                    grob::robust_relative_extent(host, b, commonx, X)?.at(-d)
                };
                *x_span.at_mut(d) = edge - overshoot[0];
            }
        } else if d == RIGHT {
            let column = item::get_column(host.heap(), b);
            let last = columns
                .last()
                .and_then(|&c| item::get_column(host.heap(), c));
            if columns.is_empty() || column != last {
                /*
                  We're connecting to a column, for the last bit of a broken
                  fullLength bracket.
                */
                let mut padding = real_or(host, me, "full-length-padding", 1.0)?;

                if item::break_status_dir(host.heap(), b) != 0 {
                    padding = 0.0;
                }

                let mut coord = grob::relative_coordinate(host, b, commonx, X)?;
                if is_true(host, me, "full-length-to-extent")? {
                    coord = grob::robust_relative_extent(host, b, commonx, X)?.left;
                }

                coord = max(coord, x_span.left);

                *x_span.at_mut(d) = coord - padding;
            }
        }
    }

    let left = bound(host, me, LEFT)?;
    x_span.translate(-grob::relative_coordinate(host, left, commonx, X)?);
    Ok(crate::offset(host.heap(), (x_span.left, x_span.right)))
}

/// `make_tuplet_slur`: a slur from `left_cp` to `right_cp`, shortened by
/// `shorten` along it, on the bracket's side, its number's `padding` (0.3)
/// further out.
fn make_tuplet_slur(
    host: &mut dyn Host,
    me: Value,
    mut left_cp: Offset,
    mut right_cp: Offset,
    shorten: [f64; 2],
) -> Run<Stencil> {
    let dz = (right_cp.0 - left_cp.0, right_cp.1 - left_cp.1);
    let len = length(dz);

    let l = shorten[0] / len;
    left_cp = (left_cp.0 + l * dz.0, left_cp.1 + l * dz.1);
    let r = shorten[1] / len;
    right_cp = (right_cp.0 - r * dz.0, right_cp.1 - r * dz.1);

    let shortened_dz = (right_cp.0 - left_cp.0, right_cp.1 - left_cp.1);
    let shortened_length = length(shortened_dz);

    // First, get a horizontal curve.  Will point upwards.
    let height_limit = 1.5;
    let ratio = 0.33;
    let mut curve = bezier::slur_shape(shortened_length, height_limit, ratio);

    // Flip curve if needed.
    let dir = get_grob_direction(host, me)?;
    bezier::scale(&mut curve, 1.0, f64::from(dir));

    // Rotate curve to proper incline.
    let height = right_cp.1 - left_cp.1;
    let slope = height / shortened_length;
    bezier::rotate(
        &mut curve,
        libm::atan(slope) * 180.0 / core::f64::consts::PI,
    );

    // Move rotated curve to correct starting point.
    let start = curve[0];
    bezier::translate(&mut curve, (left_cp.0 - start.0, left_cp.1 - start.1));

    let dash_definition = grob::property(host, me, "dash-definition")?;
    let lt = grob::layout_dimension(host, me, "line-thickness")?;
    let mut errors = alloc::vec::Vec::new();
    let mut mol = lookup::slur(host.heap(), curve, lt, lt, dash_definition, &mut errors);
    host.heap().root(mol.expr);
    for text in &errors {
        grob::unlocated_programming_error(host, text)?;
    }

    if let Some(number_grob) = grob_object(host, me, "tuplet-number")? {
        let padding = real_or(host, number_grob, "padding", 0.3)?;
        mol.translate_axis(host.heap(), padding * f64::from(dir), Y);
    }

    Ok(mol)
}

/// `print`: the bracket from `X-positions` at `positions`, with a gap
/// for its number (the bracket hidden where the number is wider, unless
/// forced), its edges `edge-height` high and flared by `bracket-flare`,
/// shortened by `shorten-pair`; at a side connected to its neighbour no
/// edge, and its `edge-text` there; or a slur for `tuplet-slur`. Without
/// both positions the bracket is killed.
///
/// TODO:
///
/// in the case that there is no bracket, but there is a (single) beam,
/// follow beam precisely for determining tuplet number location.
pub fn print(host: &mut dyn Host, me: Value) -> Run<Value> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Run<Value> {
    let tuplet_slur = grob::property(host, me, "tuplet-slur")? != Value::FALSE;

    let bracket_visibility = bracket_basic_visibility(host, me)?;
    let bracket_vis_prop = grob::property(host, me, "bracket-visibility")?;

    /*
      Don't print a tuplet bracket and number if
      no X or Y positions were calculated.
    */
    let scm_x_span = grob::property(host, me, "X-positions")?;
    let scm_positions = grob::property(host, me, "positions")?;
    if !is_pair(scm_x_span) || !is_pair(scm_positions) {
        grob::suicide(host.heap(), me);
        return Ok(Value::NULL);
    }

    let zero = Interval::new(0.0, 0.0);
    let x_span = grob::number_pair(host.heap(), scm_x_span).unwrap_or(zero);
    let positions = grob::number_pair(host.heap(), scm_positions).unwrap_or(zero);
    print_at(
        host,
        me,
        x_span,
        positions,
        bracket_visibility,
        bracket_vis_prop,
        tuplet_slur,
    )
}

/// What `print` does once it has both positions.
#[allow(clippy::too_many_arguments)]
fn print_at(
    host: &mut dyn Host,
    me: Value,
    x_span: Interval,
    positions: Interval,
    mut bracket_visibility: bool,
    bracket_vis_prop: Value,
    tuplet_slur: bool,
) -> Run<Value> {
    let mut mol = Stencil::default();
    let points = [
        (x_span.left, positions.left),
        (x_span.right, positions.right),
    ];

    let number_grob = grob_object(host, me, "tuplet-number")?;

    /*
      Don't print the bracket when it would be smaller than the number.
      ...Unless the user has coded bracket-visibility = #t, that is.
    */
    let mut gap = 0.0;
    if bracket_visibility && let Some(number_grob) = number_grob {
        let ext = grob::extent(host, number_grob, number_grob, X)?;
        if !ext.is_empty() {
            gap = ext.length() + 1.0;

            if bracket_vis_prop != Value::TRUE && gap > x_span.length() {
                bracket_visibility = false;
            }
        }
    }

    if bracket_visibility {
        let zero = [0.0, 0.0];

        let mut shorten = drul_property(host, me, "shorten-pair", zero)?;

        let ss = ssr::staff_space(host, me)?;
        shorten = shorten.map(|s| s * ss);

        if tuplet_slur {
            let brack = make_tuplet_slur(host, me, points[0], points[1], shorten)?;
            mol.add_stencil(host.heap(), &brack);
        } else {
            let mut edge_stencils = [Stencil::default(), Stencil::default()];

            let mut height = drul_property(host, me, "edge-height", zero)?;
            let mut flare = drul_property(host, me, "bracket-flare", zero)?;
            let dir = get_grob_direction(host, me)?;

            height = height.map(|h| h * (-ss * f64::from(dir)));
            flare = flare.map(|f| f * ss);

            let connect_to_other = connect_to_neighbor(host, me)?;

            for d in [LEFT, RIGHT] {
                if connect_to_other[side(d)] {
                    height[side(d)] = 0.0;
                    flare[side(d)] = 0.0;
                    shorten[side(d)] = 0.0;

                    let edge_text = grob::property(host, me, "edge-text")?;

                    if is_pair(edge_text) {
                        let heap = host.heap();
                        let t = if d == LEFT {
                            heap.car(edge_text)
                        } else {
                            heap.cdr(edge_text)
                        };
                        let t = t.unwrap_or(Value::FALSE);
                        if text::is_markup(host, t)? {
                            let v = crate::call_lily(host, "%grob-interpret-markup", &[me, t])?;
                            host.heap().root(v);
                            let mut es = host
                                .heap()
                                .foreign::<Stencil>(v)
                                .copied()
                                .unwrap_or_default();
                            es.translate_axis(host.heap(), x_span.at(d) - x_span.left, X);
                            host.heap().root(es.expr);
                            edge_stencils[side(d)] = es;
                        }
                    }
                }
            }

            /*
              0.1 = more space at right due to italics
              TODO: use italic correction of font.
            */
            let mut gap_iv = Interval::new(-0.5, 0.5);
            gap_iv.scale(gap);
            gap_iv.translate(0.1);
            let dz = (points[1].0 - points[0].0, points[1].1 - points[0].1);
            let mut brack = make_bracket(host, me, Y, dz, height, gap_iv, flare, shorten)?;

            let heap = host.heap();
            for es in &edge_stencils {
                if !es.is_empty() {
                    brack.add_stencil(heap, es);
                }
            }

            mol.add_stencil(heap, &brack);
            mol.translate(heap, points[0]);
        }
    }
    Ok(host.heap().wrap(mol))
}

/// `get_bounds`: the first and last of the note columns that have no
/// rests, if there are any.
pub(crate) fn get_bounds(host: &mut dyn Host, me: Value) -> Run<Option<[Value; 2]>> {
    let columns = grob_array::extract_grob_array(host, me, "note-columns")?;
    let mut l = 0;
    while l < columns.len() && note_column::has_rests(host, columns[l])? {
        l += 1;
    }

    let mut r = columns.len();
    while r > l && note_column::has_rests(host, columns[r - 1])? {
        r -= 1;
    }

    Ok((l < r).then(|| [columns[l], columns[r - 1]]))
}

/// `calc_position_and_height`: the bracket's offset at its left and its
/// rise across. Its slope from the stems' ends of the beam it follows, or
/// from the outer columns of notes, kept to the beam's or
/// `max-slope-factor`; then it is moved out past each column's extent, the
/// staff's with `staff-padding`, the brackets inside it and their numbers
/// and the scripts it avoids, by `padding`; a horizontal one inside the
/// staff put on a staff space, not a line.
///
/// use first -> last note for slope, and then correct for disturbing
/// notes in between.
fn calc_position_and_height(host: &mut dyn Host, me: Value) -> Run<(f64, f64)> {
    let columns = grob_array::extract_grob_array(host, me, "note-columns")?;
    let tuplets = grob_array::extract_grob_array(host, me, "tuplets")?;

    let heap = host.heap();
    let commony = grob::common_refpoint_of_array(heap, &columns, Some(me), Y);
    let commony = grob::common_refpoint_of_array(heap, &tuplets, commony, Y);
    let mut commony = commony.unwrap_or(me);
    if let Some(st) = ssr::staff_symbol(host, me)? {
        commony = grob::common_refpoint(host.heap(), st, commony, Y).unwrap_or(Value::NULL);
    }
    let my_offset = grob::relative_coordinate(host, me, commony, Y)?;

    let (mut offset, mut dy) = (0.0, 0.0);

    let Some(commonx) = spanner::get_system(host.heap(), me) else {
        // FIXME: ->get_system () should also work before line breaking
        grob::unlocated_programming_error(
            host,
            "TupletBracket.positions requested before line breaking",
        )?;
        return Ok((offset, dy));
    };

    let mut staff = Interval::EMPTY;
    let st = ssr::staff_symbol(host, me)?;

    /* staff-padding doesn't work correctly on cross-staff tuplets
    because it only considers one staff symbol. Until this works,
    disable it. */
    if let Some(st) = st
        && !is_true(host, me, "cross-staff")?
    {
        let pad = real_or(host, me, "staff-padding", -1.0)?;
        if pad >= 0.0 {
            staff = grob::extent(host, st, commony, Y)?;
            staff.translate(-my_offset);
            staff.widen(pad);
        }
    }

    let dir = get_grob_direction(host, me)?;
    let fdir = f64::from(dir);

    let par_beam = grob_object(host, me, "beam")?; // NB may be null

    let lgr = get_x_bound_item(host, me, LEFT, dir)?;
    let rgr = get_x_bound_item(host, me, RIGHT, dir)?;
    let x0 = grob::robust_relative_extent(host, lgr, commonx, X)?.left;
    let x1 = grob::robust_relative_extent(host, rgr, commonx, X)?.right;

    let max_slope_factor = real_or(host, me, "max-slope-factor", 0.0)?;

    let follow_beam = match par_beam {
        Some(b) => get_grob_direction(host, b)? == dir && !is_true(host, b, "knee")?,
        None => false,
    };

    let mut points: alloc::vec::Vec<Offset> = alloc::vec::Vec::new();
    let stems = if !columns.is_empty() && follow_beam {
        let first = note_column::get_stem(host, columns[0])?;
        let last = note_column::get_stem(host, columns[columns.len() - 1])?;
        first.zip(last)
    } else {
        None
    };
    if let Some((first, last)) = stems {
        let stems = [first, last];

        let mut poss = Interval::new(0.0, 0.0);
        for s in [LEFT, RIGHT] {
            let stem = stems[side(s)];
            // Trigger setting of stem lengths if necessary.
            if let Some(beam) = stem::get_beam(host, stem)? {
                grob::property(host, beam, "quantized-positions")?;
            }
            let ext = grob::extent(host, stem, stem, Y)?;
            let stem_dir = get_grob_direction(host, stem)?;
            let parent_relative = match grob::get_parent(host.heap(), stem, Y) {
                Some(p) => grob::relative_coordinate(host, p, commony, Y)?,
                None => 0.0,
            };
            *poss.at_mut(s) = ext.at(stem_dir) + parent_relative;
        }

        dy = poss.right - poss.left;

        let lx = grob::relative_coordinate(host, stems[0], commonx, X)?;
        points.push((lx - x0, poss.left));
        let rx = grob::relative_coordinate(host, stems[1], commonx, X)?;
        points.push((rx - x0, poss.right));
    } else {
        /*
          Use outer non-rest columns to determine slope
        */
        if let Some([left_col, right_col]) = get_bounds(host, me)? {
            let mut rv = cross_staff_extent(host, right_col, commony)?;
            let mut lv = cross_staff_extent(host, left_col, commony)?;
            rv.unite(staff);
            lv.unite(staff);

            let graphical_dy = rv.at(dir) - lv.at(dir);

            let ls = note_column::head_positions_interval(host, left_col)?;
            let rs = note_column::head_positions_interval(host, right_col)?;

            let musical_dy = [
                (i64::from(rs[0]) - i64::from(ls[0])) as f64,
                (i64::from(rs[1]) - i64::from(ls[1])) as f64,
            ];

            dy = if sign(musical_dy[1]) != sign(musical_dy[0])
                || sign(graphical_dy) != sign(musical_dy[0])
            {
                0.0
            } else {
                graphical_dy
            };
        } else {
            dy = 0.0;
        }

        let mut x = 0.0;
        for &col in &columns {
            let note_ext = cross_staff_extent(host, col, commony)?;
            x = grob::relative_coordinate(host, col, commonx, X)? - x0;

            points.push((x, note_ext.at(dir)));
        }

        let last_x = x;

        if dy != 0.0 {
            let mut slope = (dy / (x1 - x0)).abs();
            let max_slope;
            let max_dy = max_slope_factor * last_x * f64::from(sign(dy));
            let mut sub_x0 = 0.0;
            let mut sub_x1 = 0.0;

            let mut beam_positions = [0.0, 0.0];

            if let Some(par_beam) = par_beam {
                let v = grob::property(host, par_beam, "quantized-positions")?;
                beam_positions = real_drul(host.heap(), v, [0.0, 0.0]);
            } else {
                for &col in columns.iter().rev() {
                    let Some(stem) = note_column::get_stem(host, col)? else {
                        continue;
                    };
                    if let Some(beam) = stem::get_beam(host, stem)? {
                        let v = grob::property(host, beam, "quantized-positions")?;
                        beam_positions = real_drul(host.heap(), v, [0.0, 0.0]);
                        sub_x0 = grob::robust_relative_extent(host, beam, commonx, X)?.left;
                        sub_x1 = grob::robust_relative_extent(host, beam, commonx, X)?.right;
                        break;
                    }
                }
            }

            let beam_dy = beam_positions[1] - beam_positions[0];

            if beam_dy != 0.0 {
                let beam_slope = if sub_x1 == 0.0 {
                    (beam_dy / (x1 - x0)).abs()
                } else {
                    (beam_dy / (sub_x1 - sub_x0)).abs()
                };

                max_slope = if beam_slope != 0.0 {
                    max(beam_slope, max_slope_factor)
                } else {
                    max_slope_factor
                };

                slope = min(slope, max_slope);

                if dy.abs() > max_dy.abs() {
                    if (dy * slope).abs() <= max_dy.abs() {
                        dy *= slope;
                    } else {
                        dy = max_dy;
                    }
                }
            } else if dy.abs() > max_dy.abs() {
                dy = max_dy;
            }
        }
    }

    if !follow_beam {
        // LilyPond's `x0 - x0`, NaN where `x0` is infinite.
        #[allow(clippy::eq_op)]
        points.push((x0 - x0, staff.at(dir)));
        points.push((x1 - x0, staff.at(dir)));
    }

    /*
      This is a slight hack. We compute two encompass points from the
      bbox of the smaller tuplets.

      We assume that the smaller bracket is 1.0 space high.
    */
    let ss = ssr::staff_space(host, me)?;
    for &t in &tuplets {
        let tuplet_x = grob::extent(host, t, commonx, X)?;
        let tuplet_y = grob::extent(host, t, commony, Y)?;

        if !grob::is_live_grob(host.heap(), t) {
            continue;
        }

        let v = grob::property(host, t, "positions")?;
        let positions = real_drul(host.heap(), v, [0.0, 0.0]);

        let other_dy = positions[1] - positions[0];

        for d in [LEFT, RIGHT] {
            let y = tuplet_y.linear_combination(f64::from(d * sign(other_dy)));

            /*
              We don't take padding into account for nested tuplets.
              the edges can come very close to the stems, likewise for
              nested tuplets?
            */

            points.push((tuplet_x.at(d) - x0, y));
        }

        // Check for number-on-bracket collisions
        if let Some(number) = grob_object(host, t, "tuplet-number")? {
            let x_ext = grob::robust_relative_extent(host, number, commonx, X)?;
            let y_ext = grob::robust_relative_extent(host, number, commony, Y)?;
            points.push((x_ext.center() - x0, y_ext.at(dir)));
        }
    }

    if is_true(host, me, "avoid-scripts")? {
        let osp = grob::property(host, me, "outside-staff-priority")?;
        if !host.heap().is_number(osp) {
            let scripts = grob_array::extract_grob_array(host, me, "scripts")?;
            for &s in &scripts {
                if !grob::is_live_grob(host.heap(), s) {
                    continue;
                }
                let osp = grob::property(host, s, "outside-staff-priority")?;
                if host.heap().is_number(osp) {
                    continue;
                }

                // assume that if a script is avoiding slurs, it should not get placed
                // under a tuplet bracket
                if grob_object(host, s, "slur")?.is_some() {
                    continue;
                }

                let script_x = grob::robust_relative_extent(host, s, commonx, X)?;
                let script_y = grob::robust_relative_extent(host, s, commony, Y)?;

                points.push((script_x.center() - x0, script_y.at(dir)));
            }
        }
    }

    offset = -fdir * f64::INFINITY;
    let factor = if columns.len() > 1 {
        1.0 / (x1 - x0)
    } else {
        1.0
    };
    for &(x, y) in &points {
        let tuplety = fma(dy * x, factor, my_offset);

        if y * fdir > (offset + tuplety) * fdir {
            offset = y - tuplety;
        }
    }

    let padding = grob::property(host, me, "padding")?;
    let padding = grob::to_double(host.heap(), padding)?;
    offset = fma(padding, fdir, offset);

    /*
      horizontal brackets should not collide with staff lines.

      This doesn't seem to support cross-staff tuplets atm.
    */
    if dy.abs() < 0.01 {
        // quantize, then do collision check.
        offset /= 0.5 * ss;

        let mut staff_span = ssr::staff_span(host, me)?;

        /* Include in the staff span also tuplet brackets that might
        collide with the extremal staff lines. */
        staff_span.widen(ss);

        if staff_span.left <= offset && offset <= staff_span.right {
            // Round to staff line or middle of staff space //
            offset = libm::rint(offset);
            if ssr::on_line(host, me, offset as i32)? {
                offset += fdir;
            }
        }

        offset *= 0.5 * ss;
    }
    Ok((offset, dy))
}

/// `Note_column::cross_staff_extent`: the column's Y extent in `refp`,
/// with its stem's, which a cross-staff stem's column does not count.
pub(crate) fn cross_staff_extent(host: &mut dyn Host, me: Value, refp: Value) -> Run<Interval> {
    let mut iv = grob::extent(host, me, refp, Y)?;
    if let Some(s) = note_column::get_stem(host, me)? {
        iv.unite(grob::extent(host, s, refp, Y)?);
    }
    Ok(iv)
}

/// `calc_direction`: [`get_default_dir`].
pub fn calc_direction(host: &mut dyn Host, me: Value) -> Run<Value> {
    let dir = get_default_dir(host, me)?;
    Ok(host.heap().integer(i64::from(dir)))
}

/// `calc_positions`: the bracket's offset at its left and at its right.
pub fn calc_positions(host: &mut dyn Host, me: Value) -> Run<Value> {
    let (offset, dy) = calc_position_and_height(host, me)?;
    let heap = host.heap();
    let (a, b) = (heap.flonum(offset), heap.flonum(offset + dy));
    Ok(heap.cons(a, b))
}

/// `get_default_dir`: the direction most of its columns of notes' stems
/// have; at a tie, up without any, else the side whose extremal heads are
/// further inside the staff, up for no staff.
///
/// similar to beam ?
fn get_default_dir(host: &mut dyn Host, me: Value) -> Run<Direction> {
    let mut dirs = [0i32; 2];
    let columns = grob_array::extract_grob_array(host, me, "note-columns")?;
    for &nc in &columns {
        if note_column::has_rests(host, nc)? {
            continue;
        }
        let d = note_column::dir(host, nc)?;
        if d != 0 {
            dirs[side(d)] += 1;
        }
    }

    if dirs[1] == dirs[0] {
        if dirs[1] == 0 {
            return Ok(UP);
        }
        let Some(staff) = ssr::staff_symbol(host, me)? else {
            return Ok(UP);
        };
        let staff_extent = grob::extent(host, staff, staff, Y)?;
        let mut extremal_positions = Interval::EMPTY;
        for &col in &columns {
            let d = note_column::dir(host, col)?;
            let hp = note_column::head_positions_interval(host, col)?;
            // A Drul_array's CENTER is its LEFT, and `minmax` takes the
            // minimum for it.
            let k = if d == UP { UP } else { DOWN };
            let p = f64::from(hp[side(k)]);
            let at = extremal_positions.at_mut(k);
            *at = if d == UP { max(p, *at) } else { min(p, *at) };
        }
        for d in [DOWN, UP] {
            *extremal_positions.at_mut(d) =
                -f64::from(d) * (staff_extent.at(d) - extremal_positions.at(d));
        }

        return Ok(if extremal_positions.right <= extremal_positions.left {
            UP
        } else {
            DOWN
        });
    }

    Ok(if dirs[1] > dirs[0] { UP } else { DOWN })
}

/// `add_column`: `n` one of its `note-columns` and a bound.
pub(crate) fn add_column(host: &mut dyn Host, me: Value, n: Value) -> Run<()> {
    let sym = host.heap().intern("note-columns");
    grob_array::add_grob(host, me, sym, n)?;
    spanner::add_bound_item(host, me, n)
}

/// `add_script`: `s` one of its `scripts`.
pub(crate) fn add_script(host: &mut dyn Host, me: Value, s: Value) -> Run<()> {
    let sym = host.heap().intern("scripts");
    grob_array::add_grob(host, me, sym, s)
}

/// `add_tuplet_bracket`: `bracket` one of its `tuplets`.
pub(crate) fn add_tuplet_bracket(host: &mut dyn Host, me: Value, bracket: Value) -> Run<()> {
    let sym = host.heap().intern("tuplets");
    grob_array::add_grob(host, me, sym, bracket)
}

/// `calc_cross_staff`: whether a vertical alignment is between it and the
/// common refpoint of its columns, the brackets inside it and its staff;
/// else whether its `potential-beam` or a stem of its columns is
/// cross-staff.
pub fn calc_cross_staff(host: &mut dyn Host, me: Value) -> Run<Value> {
    let cols = grob_array::extract_grob_array(host, me, "note-columns")?;
    let tuplets = grob_array::extract_grob_array(host, me, "tuplets")?;

    let heap = host.heap();
    let commony = grob::common_refpoint_of_array(heap, &cols, Some(me), Y);
    let commony = grob::common_refpoint_of_array(heap, &tuplets, commony, Y);
    let mut commony = commony.unwrap_or(me);
    if let Some(st) = ssr::staff_symbol(host, me)? {
        commony = grob::common_refpoint(host.heap(), st, commony, Y).unwrap_or(Value::NULL);
    }
    if grob::check_cross_staff(host.heap(), me, commony) {
        return Ok(Value::TRUE);
    }

    // Whether we want to use a parallel beam depends on whether we are going to
    // be broken.  However, we will only have that information after line
    // breaking, while cross-staff needs to be known before line breaking.
    // Therefore we conservatively mark as cross-staff if there is a potential
    // beam.
    let par_beam = grob_object(host, me, "potential-beam")?; // NB may be null

    if let Some(b) = par_beam
        && is_true(host, b, "cross-staff")?
    {
        return Ok(Value::TRUE);
    }

    for &col in &cols {
        if let Some(stem) = grob_object(host, col, "stem")?
            && is_true(host, stem, "cross-staff")?
        {
            return Ok(Value::TRUE);
        }
    }

    Ok(Value::FALSE)
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:tuplet-bracket::` primitives (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "tuplet-bracket",
    f: host_tuplet_bracket,
}];

/// The `ly:tuplet-bracket::` callbacks [`HOSTS`] runs, by name, which the
/// callback table (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 5] = [
    ("calc-cross-staff", calc_cross_staff),
    ("calc-direction", calc_direction),
    ("calc-positions", calc_positions),
    ("calc-x-positions", calc_x_positions),
    ("print", print),
];

fn host_tuplet_bracket(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such tuplet bracket callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// A `ly:tuplet-bracket::` subr: its spanner checked under the callback's
/// name, as LilyPond's `LY_ASSERT_SMOB` gives it (a grob for
/// `calc-direction`), and `%host-call` tail-called with its name.
fn host_call(cx: &mut Cx, name: &str, args: &[Value]) -> R {
    let method = alloc::format!("ly:tuplet-bracket::{name}");
    if name == "calc-direction" {
        grob::grob_arg(cx, &method, 1, args[0])?;
    } else {
        spanner::spanner_arg(cx, &method, 1, args[0])?;
    }
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("tuplet-bracket");
    let name = cx.heap_mut().intern(name);
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}

macro_rules! callbacks {
    ($($f:ident $name:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            host_call(cx, $name, args)
        })*
    };
}

callbacks! {
    ly_calc_cross_staff "calc-cross-staff",
    ly_calc_direction "calc-direction",
    ly_calc_positions "calc-positions",
    ly_calc_x_positions "calc-x-positions",
    ly_print "print",
}
