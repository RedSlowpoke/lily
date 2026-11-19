//! `line-spanner.cc` (part 4's, MS3 step 52, 758 lines): the line
//! spanner's bound info (`calc_bound_info`, `:96–418`, and its four
//! callbacks `ly:line-spanner::calc-left-bound-info` and
//! `::calc-right-bound-info`, `ly:horizontal-line-spanner::…` the same for
//! a horizontal one, and the left one with its `text`, `:420–480`), its
//! cross-staff (`ly:line-spanner::calc-cross-staff`, `:484–496`) and its
//! print (`ly:line-spanner::print`, `:498–628`), the music crate's
//! placeholders until then but the left info with its text of a line
//! spanner that is not horizontal, which no grob reads (D67). Rust under
//! primitives that print as LilyPond's subrs and call it over `%host-call`
//! (D127, [`HOSTS`]), in 05 §5.1's callback table (`callbacks.rs`) too.
//!
//! A bound's `text` is interpreted on the spanner
//! (`%grob-interpret-markup`, `text.rs`), a paper column's extent is
//! `axis_group.rs`' `%generic-bound-extent`, and whether a spanner is
//! unfinished at a bound is LilyPond's `unbroken-or-first-broken-spanner?`
//! or `unbroken-or-last-broken-spanner?`, called as LilyPond calls them.
//! The line and its arrows are `Line_interface`'s on the spanner (`line.rs`'
//! [`grob_line`] and [`grob_arrows`]). The bound infos check their spanner
//! as `calc_bound_info`'s `LY_ASSERT_SMOB (Spanner, smob, 0)` does, the
//! print under its full name, where LilyPond's dereferences what is none;
//! the cross-staff of what is no spanner is `#f`, as LilyPond's. A spanner
//! without a bound, which LilyPond dereferences, throws `misc-error`.
//!
//! LilyPond's clang fuses the print's two moves of the ends by
//! `normalized-endpoints` (2.26.0's binary): [`fma`] here; the magnification
//! is `exp2`'s, as its `pow (2, …)` is.

use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::{Direction, Interval, Offset, X, Y, direction, fma, length, max, min};
use crate::line::{grob_arrows, grob_line};
use crate::note_collision::direction_or;
use crate::spacing_options::double_or;
use crate::staff_symbol_referencer as ssr;
use crate::stencil::Stencil;
use crate::{grob, grob_array, item, note_column, spanner, text};

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const CENTER: Direction = 0;

/// A `Drul_array`'s index of `d`: 0 for `LEFT`, 1 for `RIGHT`.
fn side(d: Direction) -> usize {
    usize::from(d == RIGHT)
}

/// `get_bound (d)`, which LilyPond reads through: a spanner without one
/// throws `misc-error`.
fn bound(host: &mut dyn Host, me: Value, d: Direction) -> Result<Value, Exception> {
    spanner::get_bound(host, me, d)?.ok_or_else(|| {
        crate::misc_error(
            host.heap(),
            "ly:line-spanner",
            "line spanner without a bound",
        )
    })
}

/// `ly_assoc_get (key, alist, #f)`: `key`'s value in `alist`, `None` for
/// no entry.
fn assoc_get(host: &mut dyn Host, key: &str, alist: Value) -> Option<Value> {
    let heap = host.heap();
    let key = heap.intern(key);
    grob::assq_ref(heap, key, alist)
}

/// `scm_acons (key, value, alist)`, rooted until the caller unroots it.
fn acons(host: &mut dyn Host, key: &str, value: Value, alist: Value) -> Value {
    let heap = host.heap();
    let key = heap.intern(key);
    let entry = heap.cons(key, value);
    let alist = heap.cons(entry, alist);
    heap.root(alist);
    alist
}

/// `ly_append (extra, details)`: `extra`'s items copied before `details`.
fn append(host: &mut dyn Host, extra: Value, details: Value) -> Value {
    let heap = host.heap();
    let mut items = Vec::new();
    let mut at = extra;
    while let (Some(x), Some(rest)) = (heap.car(at), heap.cdr(at)) {
        items.push(x);
        at = rest;
    }
    let mut list = details;
    for &x in items.iter().rev() {
        list = heap.cons(x, list);
    }
    heap.root(list);
    list
}

/// Whether `v` is a real number, as `scm_is_number` of an alist's value.
fn is_number(host: &mut dyn Host, v: Option<Value>) -> bool {
    v.is_some_and(|v| host.heap().is_number(v))
}

/// The common refpoint on X of the spanner's bounds and the spanner.
fn common_x(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let (l, r) = (bound(host, me, LEFT)?, bound(host, me, RIGHT)?);
    let heap = host.heap();
    let commonx = grob::common_refpoint(heap, l, r, X);
    let commonx = commonx.and_then(|c| grob::common_refpoint(heap, me, c, X));
    Ok(commonx.unwrap_or(Value::NULL))
}

/// `offsets_maybe` (`:73–94`): the Y coordinates of the two grobs, where
/// each is, relative to their common refpoint, or of the one relative to
/// itself; and that refpoint.
fn offsets_maybe(
    host: &mut dyn Host,
    grobs: [Option<Value>; 2],
) -> Result<([f64; 2], Option<Value>), Exception> {
    match grobs {
        [Some(g1), Some(g2)] => {
            let common = grob::common_refpoint(host.heap(), g1, g2, Y);
            let refp = common.unwrap_or(Value::NULL);
            let coord1 = grob::relative_coordinate(host, g1, refp, Y)?;
            let coord2 = grob::relative_coordinate(host, g2, refp, Y)?;
            Ok(([coord1, coord2], common))
        }
        [Some(g1), None] => {
            let coord1 = grob::relative_coordinate(host, g1, g1, Y)?;
            // The 0.0 shouldn't get used.
            Ok(([coord1, 0.0], Some(g1)))
        }
        [None, Some(g2)] => {
            let coord2 = grob::relative_coordinate(host, g2, g2, Y)?;
            Ok(([0.0, coord2], Some(g2)))
        }
        [None, None] => Ok(([0.0, 0.0], None)),
    }
}

/// `calc_bound_info`: the bound's details of `bound-details` (its
/// `left-broken` or `right-broken` first where the spanner is unfinished
/// there, else `default`), with the `stencil` of its `text`, its `X` from
/// its bound (or the first or last note column, `end-on-note`) by its
/// `attach-dir`, the accidentals' (`end-on-accidental`), the dot's
/// (`start-at-dot`) or the neighbour's (`adjust-on-neighbor`); for a
/// spanner that is not horizontal its `Y`, the bound's centre on its
/// vertical axis group (a broken one's, as the two systems' staves are
/// placed), with `extra-dy`; and the `common-Y` it is relative to.
fn calc_bound_info(
    host: &mut dyn Host,
    me: Value,
    dir: Direction,
    horizontal: bool,
) -> Result<Value, Exception> {
    let bound_item = bound(host, me, dir)?;

    let bound_details = grob::property(host, me, "bound-details")?;
    host.heap().root(bound_details);

    let mut details = assoc_get(
        host,
        if dir == LEFT { "left" } else { "right" },
        bound_details,
    )
    .unwrap_or(Value::FALSE);

    // Don't use bound_item->break_status_dir (): a spanner running to the end of
    // the piece has a broken right bound, but should not get details from
    // right-broken.
    let checker = if dir == LEFT {
        "unbroken-or-first-broken-spanner?"
    } else {
        "unbroken-or-last-broken-spanner?"
    };
    let unfinished_at_bound = crate::call_lily(host, checker, &[me])? != Value::TRUE;
    if unfinished_at_bound {
        let key = if dir == LEFT {
            "left-broken"
        } else {
            "right-broken"
        };
        let extra = assoc_get(host, key, bound_details).unwrap_or(Value::NULL);

        let rest = if details != Value::FALSE {
            details
        } else {
            Value::NULL
        };
        details = append(host, extra, rest);
    }

    if details == Value::FALSE {
        details = assoc_get(host, "default", bound_details).unwrap_or(Value::NULL);
    }
    host.heap().root(details);

    let text = assoc_get(host, "text", details).unwrap_or(Value::FALSE);
    if text::is_markup(host, text)? {
        let stencil = crate::call_lily(host, "%grob-interpret-markup", &[me, text])?;
        details = acons(host, "stencil", stencil, details);
    }

    let x = assoc_get(host, "X", details);
    if !is_number(host, x) {
        let commonx = common_x(host, me)?;

        let attach = assoc_get(host, "attach-dir", details).unwrap_or(Value::FALSE);
        let attach = f64::from(direction_or(attach, CENTER));

        let mut bound_grob = bound_item;
        let end_note = assoc_get(host, "end-on-note", details);
        if end_note == Some(Value::TRUE) && unfinished_at_bound {
            let columns = grob_array::extract_grob_array(host, me, "note-columns")?;
            if !columns.is_empty() {
                bound_grob = if dir == LEFT {
                    columns[0]
                } else {
                    columns[columns.len() - 1]
                };
            }
        }

        let extent = if grob::has_interface(host.heap(), bound_grob, "paper-column-interface") {
            let axis = host.heap().integer(0);
            let e = crate::call_lily(host, "%generic-bound-extent", &[bound_grob, commonx, axis])?;
            grob::number_pair(host.heap(), e).unwrap_or(Interval::EMPTY)
        } else {
            grob::robust_relative_extent(host, bound_grob, commonx, X)?
        };
        let mut x_coord = extent.linear_combination(attach);

        let end_acc = assoc_get(host, "end-on-accidental", details);
        if end_acc == Some(Value::TRUE) {
            let mut maybe_note_column = None;
            // If the bound is already a note column, use it.
            if grob::has_interface(host.heap(), bound_grob, "note-column-interface") {
                maybe_note_column = Some(bound_grob);
            } else {
                /* Our bound may be a note head or rest, so try the parent
                axis group. */
                let sym = host.heap().intern("axis-group-parent-Y");
                let ag = grob::object(host, bound_grob, sym)?;
                if grob_of(host.heap(), ag).is_some()
                    && grob::has_interface(host.heap(), ag, "note-column-interface")
                {
                    maybe_note_column = Some(ag);
                }
            }
            if let Some(note_column) = maybe_note_column
                && let Some(acc_placement) = note_column::accidentals(host, note_column)?
            {
                x_coord = grob::robust_relative_extent(host, acc_placement, commonx, X)?
                    .linear_combination(attach);
            }
        }

        let sym = host.heap().intern("dot");
        let dot = grob::object(host, bound_grob, sym)?;
        if grob_of(host.heap(), dot).is_some()
            && assoc_get(host, "start-at-dot", details) == Some(Value::TRUE)
        {
            x_coord =
                grob::robust_relative_extent(host, dot, commonx, X)?.linear_combination(attach);
        }

        let adj = assoc_get(host, "adjust-on-neighbor", details);
        if adj == Some(Value::TRUE) {
            let name = if dir == LEFT {
                "left-neighbor"
            } else {
                "right-neighbor"
            };
            let sym = host.heap().intern(name);
            let neighbor = grob::object(host, me, sym)?;
            if grob_of(host.heap(), neighbor).is_some() {
                let neighbor_ext = grob::extent(host, neighbor, commonx, X)?;
                let neighbor_x = neighbor_ext.at(-dir);
                x_coord = if dir == LEFT {
                    max(x_coord, neighbor_x)
                } else {
                    min(x_coord, neighbor_x)
                };
            }
        }

        let x = host.heap().flonum(x_coord);
        details = acons(host, "X", x, details);
    }

    let common_y;
    if horizontal {
        common_y = me;
    } else {
        let y = assoc_get(host, "Y", details);
        let y_needed = !is_number(host, y);
        // Even when we don't need to compute a Y value, run through part
        // of the code below in order to convey a reference point.  The
        // purpose is to make user tweaks to the Y value relative to the
        // relevant staff in the case of cross-staff line spanners.  Note
        // that this is not relevant for horizontal line spanners, the Y
        // value is always relative to the spanner itself.
        let mut y = 0.0;

        if unfinished_at_bound {
            /*
              We want to compute the slope of something like a glissando
              when broken across several systems.  We make it continuous,
              giving the same slope to all pieces and choosing it such that
              visually it could be one glissando line if the systems were
              stuck together in a row.  To this end, we have to compute the
              vertical coordinates on two different systems and bring them
              to a common coordinate reference: relative to the
              VerticalAxisGroup of the staff for normal glissandi, and for
              cross-staff ones the choice of Solomon (line-spanner.cc's
              drawing): align the middles of each pair of staves.
            */
            let orig = grob::original(host.heap(), me).unwrap_or(me);
            let sys_here = spanner::get_system(host.heap(), me);
            let pieces = spanner::broken_intos(host.heap(), orig);
            let mut extreme_bounds = [Value::NULL; 2];
            let mut extreme_bound_groups = [Value::NULL; 2];
            for d in [LEFT, RIGHT] {
                let extreme = if d == LEFT {
                    pieces.first()
                } else {
                    pieces.last()
                };
                let extreme = extreme.copied().unwrap_or(me);
                extreme_bounds[side(d)] = bound(host, extreme, d)?;
                match grob::vertical_axis_group(host.heap(), extreme_bounds[side(d)]) {
                    Some(group) => extreme_bound_groups[side(d)] = group,
                    None => {
                        grob::unlocated_programming_error(
                            host,
                            "extremal broken spanner's bound has no parent vertical axis group",
                        )?;
                        return Ok(details);
                    }
                }
            }
            let sys_there = item::get_system(host.heap(), extreme_bounds[side(dir)]);
            let mut extreme_bound_groups_here = [None; 2];
            let mut extreme_bound_groups_there = [None; 2];
            for d in [LEFT, RIGHT] {
                let heap = host.heap();
                let group = extreme_bound_groups[side(d)];
                let original = grob::original(heap, group).unwrap_or(group);
                // This one can be null if the corresponding staff ended
                // prematurely or started after the beginning of the score.
                extreme_bound_groups_here[side(d)] =
                    sys_here.and_then(|s| grob::find_broken_piece(heap, original, s));
                // Can be null for the direction other than dir.
                extreme_bound_groups_there[side(d)] =
                    sys_there.and_then(|s| grob::find_broken_piece(heap, original, s));
            }
            let (offsets_here, common_here) = offsets_maybe(host, extreme_bound_groups_here)?;
            let (offsets_there, common_there) = offsets_maybe(host, extreme_bound_groups_there)?;

            let here = |d: Direction| extreme_bound_groups_here[side(d)].is_some();
            let there = |d: Direction| extreme_bound_groups_there[side(d)].is_some();
            if y_needed {
                // Here we have all weird edge cases that can happen
                // when staves are added or removed midway.  To continue
                // the example above, if the lower staff was removed on
                // the second system, we would align the upper staves.
                if !here(dir) && !here(-dir) {
                    // If neither of the staves is present on this system, just
                    // disappear.  This can happen with contorted input that starts
                    // a glissando, stops that staff, then later spawns another
                    // staff and ends the glissando there.
                    grob::suicide(host.heap(), me);
                    return Ok(Value::UNSPECIFIED);
                }
                let (offset_here, offset_there);
                if there(-dir) {
                    if here(dir) && here(-dir) {
                        offset_here = (offsets_here[0] + offsets_here[1]) / 2.0;
                        offset_there = (offsets_there[0] + offsets_there[1]) / 2.0;
                    } else if here(dir) {
                        offset_here = offsets_here[side(dir)];
                        offset_there = offsets_there[side(dir)];
                    } else {
                        offset_here = offsets_here[side(-dir)];
                        offset_there = offsets_there[side(-dir)];
                    }
                } else if here(dir) {
                    offset_here = offsets_here[side(dir)];
                    offset_there = offsets_there[side(dir)];
                } else {
                    offset_here = offsets_here[side(-dir)];
                    offset_there = offsets_there[side(dir)];
                }
                let refp = common_there.unwrap_or(Value::NULL);
                let extent = grob::extent(host, extreme_bounds[side(dir)], refp, Y)?;
                let coord_there = extent.center();
                y = coord_there - offset_there + offset_here;
            }
            if let Some(group) = extreme_bound_groups_here[side(dir)] {
                common_y = group;
                if y_needed {
                    y -= offsets_here[side(dir)];
                }
            } else {
                common_y = common_here.unwrap_or(me);
            }
        } else {
            common_y = match grob::vertical_axis_group(host.heap(), bound_item) {
                Some(group) => group,
                None => {
                    grob::unlocated_programming_error(
                        host,
                        "bound item has no parent vertical axis group",
                    )?;
                    bound_item
                }
            };
            if y_needed {
                let ii = grob::extent(host, bound_item, common_y, Y)?;
                if !ii.is_empty() {
                    y = ii.center();
                }
            }
        }

        if y_needed {
            let extra_dy = grob::property(host, me, "extra-dy")?;
            let extra_dy = double_or(host, extra_dy, 0.0);
            y += f64::from(dir) * extra_dy / 2.0;
            let y = host.heap().flonum(y);
            details = acons(host, "Y", y, details);
        }
    }
    Ok(acons(host, "common-Y", common_y, details))
}

/// `calc_left_bound_info_and_text`: the left bound's info, with the
/// spanner's `text` as its `stencil` where it has none and the spanner is
/// not a broken piece after the first.
fn calc_left_bound_info_and_text(
    host: &mut dyn Host,
    me: Value,
    horizontal: bool,
) -> Result<Value, Exception> {
    let mut alist = calc_bound_info(host, me, LEFT, horizontal)?;
    host.heap().root(alist);

    let text = grob::property(host, me, "text")?;
    host.heap().root(text);
    if text::is_markup(host, text)?
        && crate::call_lily(host, "unbroken-or-first-broken-spanner?", &[me])? == Value::TRUE
        && assoc_get(host, "stencil", alist).is_none_or(|s| s == Value::FALSE)
    {
        let stencil = crate::call_lily(host, "%grob-interpret-markup", &[me, text])?;
        alist = acons(host, "stencil", stencil, alist);
    }

    Ok(alist)
}

/// Each callback with its bound info rooted while it is made.
macro_rules! rooted {
    ($host:ident, $e:expr) => {{
        let depth = $host.heap().root_depth();
        let v = $e;
        $host.heap().unroot_to(depth);
        v
    }};
}

pub fn calc_left_bound_info(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    rooted!(host, calc_bound_info(host, me, LEFT, false))
}

pub fn calc_right_bound_info(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    rooted!(host, calc_bound_info(host, me, RIGHT, false))
}

pub fn calc_left_bound_info_and_text_of(
    host: &mut dyn Host,
    me: Value,
) -> Result<Value, Exception> {
    rooted!(host, calc_left_bound_info_and_text(host, me, false))
}

pub fn horizontal_calc_left_bound_info(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    rooted!(host, calc_bound_info(host, me, LEFT, true))
}

pub fn horizontal_calc_right_bound_info(
    host: &mut dyn Host,
    me: Value,
) -> Result<Value, Exception> {
    rooted!(host, calc_bound_info(host, me, RIGHT, true))
}

pub fn horizontal_calc_left_bound_info_and_text(
    host: &mut dyn Host,
    me: Value,
) -> Result<Value, Exception> {
    rooted!(host, calc_left_bound_info_and_text(host, me, true))
}

// TODO: for horizontal line spanners, avoid looking at the
// right bound, and never mark cross-staff.
/// `calc_cross_staff`: whether the bounds' staff symbols differ; `#f` for
/// what is no spanner.
pub fn calc_cross_staff(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    if grob_of(host.heap(), me).is_none() || !spanner::is_spanner(host.heap(), me) {
        return Ok(Value::FALSE);
    }

    let lb = spanner::get_bound(host, me, LEFT)?;
    let rb = spanner::get_bound(host, me, RIGHT)?;
    let mut staves = [None; 2];
    for (staff, b) in staves.iter_mut().zip([lb, rb]) {
        if let Some(b) = b {
            *staff = ssr::staff_symbol(host, b)?;
        }
    }
    Ok(if staves[0] != staves[1] {
        Value::TRUE
    } else {
        Value::FALSE
    })
}

/// `print`: the line between the two bounds' `X` and `Y`, each relative
/// to its `common-Y`, the ends moved by `normalized-endpoints`, each end's
/// `stencil` put there (`stencil-align-dir-y`, `stencil-offset`) after its
/// `padding`, the line after the stencil and shortened for an `arrow`,
/// with its arrows; `'()` where the paddings are longer than the line.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    rooted!(host, print_rooted(host, me))
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let left_info = grob::property(host, me, "left-bound-info")?;
    host.heap().root(left_info);
    let right_info = grob::property(host, me, "right-bound-info")?;
    host.heap().root(right_info);
    let bounds = [left_info, right_info];

    let commonx = common_x(host, me)?;

    let mut span_points: [Offset; 2] = [(0.0, 0.0); 2];

    for d in [LEFT, RIGHT] {
        let i = side(d);
        let x = assoc_get(host, "X", bounds[i]).unwrap_or(Value::FALSE);
        let y = assoc_get(host, "Y", bounds[i]).unwrap_or(Value::FALSE);
        span_points[i] = (double_or(host, x, 0.0), double_or(host, y, 0.0));
    }

    let mut gaps = [0.0; 2];
    let mut arrows = [false; 2];
    let mut stencils: [Option<Stencil>; 2] = [None; 2];
    let mut common_y = [me; 2];

    // For scaling of 'padding and 'stencil-offset
    let font_size = grob::property(host, me, "font-size")?;
    let magstep = libm::exp2(double_or(host, font_size, 0.0) / 6.0);

    for d in [LEFT, RIGHT] {
        let i = side(d);
        let padding = assoc_get(host, "padding", bounds[i]).unwrap_or(Value::FALSE);
        gaps[i] = double_or(host, padding, 0.0);
        arrows[i] = assoc_get(host, "arrow", bounds[i]) == Some(Value::TRUE);
        let stencil = assoc_get(host, "stencil", bounds[i]).unwrap_or(Value::FALSE);
        stencils[i] = host.heap().foreign::<Stencil>(stencil).copied();
        let common = assoc_get(host, "common-Y", bounds[i]).unwrap_or(Value::FALSE);
        if grob_of(host.heap(), common).is_some() {
            common_y[i] = common;
        } else {
            grob::unlocated_programming_error(host, "no common-Y in bound details")?;
            common_y[i] = me;
        }
    }

    let my_common_y =
        grob::common_refpoint(host.heap(), common_y[0], common_y[1], Y).unwrap_or(Value::NULL);
    for d in [LEFT, RIGHT] {
        let i = side(d);
        span_points[i].1 += grob::relative_coordinate(host, common_y[i], my_common_y, Y)?;
    }

    let normalized_endpoints = grob::property(host, me, "normalized-endpoints")?;
    let normalized_endpoints =
        grob::number_pair(host.heap(), normalized_endpoints).unwrap_or(Interval::new(0.0, 1.0));
    let y_length = span_points[1].1 - span_points[0].1;

    span_points[0].1 = fma(normalized_endpoints.left, y_length, span_points[0].1);
    span_points[1].1 = fma(
        -(1.0 - normalized_endpoints.right),
        y_length,
        span_points[1].1,
    );

    let dz = (
        span_points[1].0 - span_points[0].0,
        span_points[1].1 - span_points[0].1,
    );
    let dz_dir = direction(dz);
    if gaps[0] + gaps[1] > length(dz) {
        return Ok(Value::NULL);
    }

    let mut line = Stencil::default();
    for d in [LEFT, RIGHT] {
        let i = side(d);
        let k = f64::from(-d) * gaps[i] * magstep;
        let dir = direction(dz);
        span_points[i] = (span_points[i].0 + k * dir.0, span_points[i].1 + k * dir.1);

        if let Some(mut s) = stencils[i] {
            let align = assoc_get(host, "stencil-align-dir-y", bounds[i]);
            let off = assoc_get(host, "stencil-offset", bounds[i]).unwrap_or(Value::FALSE);

            if let Some(align) = align.filter(|&a| host.heap().is_number(a)) {
                let align = host.heap().to_f64(align).unwrap_or(0.0);
                s.align_to(host.heap(), Y, align);
            }

            if let Some(off) = grob::number_pair(host.heap(), off) {
                let off = (off.left * magstep, off.right * magstep);
                s.translate(host.heap(), off);
            }

            let heap = host.heap();
            s.translate(heap, span_points[i]);

            line.add_stencil(heap, &s);
            heap.root(line.expr);
        }
    }

    for d in [LEFT, RIGHT] {
        let i = side(d);
        if let Some(s) = stencils[i]
            && !s.is_empty()
        {
            let k = s.extent(X).at(-d) / dz_dir.0;
            span_points[i] = (
                span_points[i].0 + dz_dir.0 * k,
                span_points[i].1 + dz_dir.1 * k,
            );
        }
    }

    let ss = ssr::staff_space(host, me)?;
    let dir = direction(dz);
    let adjust = (dir.0 * ss, dir.1 * ss);
    let left_adjust = if arrows[0] {
        (adjust.0 * 1.4, adjust.1 * 1.4)
    } else {
        (0.0, 0.0)
    };
    let line_left = (
        span_points[0].0 + left_adjust.0,
        span_points[0].1 + left_adjust.1,
    );
    let right_adjust = if arrows[1] {
        (adjust.0 * 0.55, adjust.1 * 0.55)
    } else {
        (0.0, 0.0)
    };
    let line_right = (
        span_points[1].0 - right_adjust.0,
        span_points[1].1 - right_adjust.1,
    );

    if line_right.0 > line_left.0 {
        let drawn = grob_line(host, me, line_left, line_right)?;
        line.add_stencil(host.heap(), &drawn);
        host.heap().root(line.expr);

        let heads = grob_arrows(
            host,
            me,
            span_points[0],
            span_points[1],
            arrows[0],
            arrows[1],
        )?;
        line.add_stencil(host.heap(), &heads);
        host.heap().root(line.expr);
    }

    let x = grob::relative_coordinate(host, me, commonx, X)?;
    let y = grob::relative_coordinate(host, me, my_common_y, Y)?;
    let heap = host.heap();
    line.translate(heap, (-x, -y));
    Ok(heap.wrap(line))
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:line-spanner::` and
/// `ly:horizontal-line-spanner::` primitives (D127): the callback its
/// second argument names, of the grob given, with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 2] = [
    HostHandler {
        what: "line-spanner",
        f: host_line_spanner,
    },
    HostHandler {
        what: "horizontal-line-spanner",
        f: host_horizontal_line_spanner,
    },
];

/// The `ly:line-spanner::` callbacks [`HOSTS`] runs, by name, which the
/// callback table (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 5] = [
    ("calc-cross-staff", calc_cross_staff),
    ("calc-left-bound-info", calc_left_bound_info),
    (
        "calc-left-bound-info-and-text",
        calc_left_bound_info_and_text_of,
    ),
    ("calc-right-bound-info", calc_right_bound_info),
    ("print", print),
];

/// The `ly:horizontal-line-spanner::` callbacks, as [`CALLBACKS`].
pub(crate) const HORIZONTAL_CALLBACKS: [(&str, Callback); 3] = [
    ("calc-left-bound-info", horizontal_calc_left_bound_info),
    (
        "calc-left-bound-info-and-text",
        horizontal_calc_left_bound_info_and_text,
    ),
    ("calc-right-bound-info", horizontal_calc_right_bound_info),
];

fn host_line_spanner(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such line spanner callback")
}

fn host_horizontal_line_spanner(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(
        vm,
        &HORIZONTAL_CALLBACKS,
        args,
        "no such horizontal line spanner callback",
    )
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:line-spanner::… spanner)` and `(ly:horizontal-line-spanner::…
/// spanner)`: LilyPond's subrs, primitives here so that they print as
/// LilyPond's. A bound info checks its spanner as `calc_bound_info` does
/// (`ly:calc-bound-info`, at no position), the print under its full name;
/// the cross-staff of what is no spanner is `#f` without a call. Each
/// tail-calls `%host-call` with its module and name ([`HOSTS`]).
fn callback(cx: &mut Cx, module: &str, what: &str, grob: Value) -> R {
    if what == "calc-cross-staff" {
        if grob_of(cx.heap(), grob).is_none() {
            return Ok(Value::FALSE);
        }
        grob::grob_arg(cx, "ly:line-spanner::calc-cross-staff", 1, grob)?;
        if !spanner::is_spanner(cx.heap_mut(), grob) {
            return Ok(Value::FALSE);
        }
    } else if what == "print" {
        spanner::spanner_arg(cx, "ly:line-spanner::print", 1, grob)?;
    } else {
        // LY_ASSERT_SMOB (Spanner, smob, 0): Guile's error at no position.
        const SUBR: &str = "ly:calc-bound-info";
        let is_grob = grob_of(cx.heap(), grob).is_some();
        if is_grob {
            grob::grob_arg(cx, SUBR, 1, grob)?;
        }
        if !is_grob || !spanner::is_spanner(cx.heap_mut(), grob) {
            return Err(cx.wrong_type_vm(Some(SUBR), grob, "Spanner"));
        }
    }
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let module = cx.heap_mut().intern(module);
    let what = cx.heap_mut().intern(what);
    Ok(cx.tail_call(f, &[module, what, grob]))
}

macro_rules! callbacks {
    ($($f:ident $module:literal $what:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            callback(cx, $module, $what, args[0])
        })*
    };
}

callbacks! {
    ly_calc_cross_staff "line-spanner" "calc-cross-staff",
    ly_calc_left_bound_info "line-spanner" "calc-left-bound-info",
    ly_calc_left_bound_info_and_text "line-spanner" "calc-left-bound-info-and-text",
    ly_calc_right_bound_info "line-spanner" "calc-right-bound-info",
    ly_print "line-spanner" "print",
    ly_horizontal_calc_left_bound_info "horizontal-line-spanner" "calc-left-bound-info",
    ly_horizontal_calc_left_bound_info_and_text "horizontal-line-spanner" "calc-left-bound-info-and-text",
    ly_horizontal_calc_right_bound_info "horizontal-line-spanner" "calc-right-bound-info",
}
