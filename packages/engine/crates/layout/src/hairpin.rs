//! `hairpin.cc` (part 4's, MS3 step 49, 379 lines): the hairpin's
//! callbacks `ly:hairpin::pure-height` (`:37–51`), `::broken-bound-padding`
//! (`:53–109`), which reads the neighbouring staff's span bar through
//! `System::get_neighboring_staff` (`system.rs`, OPEN B2), and `::print`
//! (`:111–355`), the music crate's placeholders until then (D67). Rust
//! under primitives that print as LilyPond's subrs and call it over
//! `%host-call` (D127, [`HOSTS`]); the two that are no pure callback are
//! in 05 §5.1's callback table (`callbacks.rs`) too.
//!
//! The lines are `Line_interface::line` on the hairpin (`line.rs`' [`grob_line`]),
//! the bounds' extents `Axis_group_interface::generic_bound_extent`
//! (`axis_group.rs`' `%generic-bound-extent`). `print` and
//! `broken-bound-padding` check their spanner under their full names,
//! where LilyPond's dereference what is none; `pure-height` its grob under
//! the method's name, as LilyPond's `LY_ASSERT_SMOB` gives it
//! (`ly:pure-height`). A spanner without a bound, which LilyPond
//! dereferences, throws `misc-error`.

use alloc::format;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::bracket::drul_property;
use crate::callbacks::Callback;
use crate::direction::get_grob_direction;
use crate::geometry::{Box, Direction, Interval, X, Y};
use crate::grob;
use crate::line::grob_line;
use crate::spacing_options::double_or;
use crate::staff_symbol_referencer as ssr;
use crate::stencil::Stencil;
use crate::{grob_array, item, note_column, spanner, system};

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const DOWN: Direction = -1;
const UP: Direction = 1;
const BIGGER: Direction = 1;
const SMALLER: Direction = -1;

/// A `Drul_array`'s index of `d`: 0 for `LEFT`, 1 for `RIGHT`.
fn side(d: Direction) -> usize {
    usize::from(d == RIGHT)
}

/// `from_scm<double> (get_property (me, name), fallback)`.
fn real_or(host: &mut dyn Host, me: Value, name: &str, fallback: f64) -> Result<f64, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(double_or(host, v, fallback))
}

/// `get_bound (d)`, which LilyPond reads through: a spanner without one
/// throws `misc-error`.
fn bound(host: &mut dyn Host, me: Value, d: Direction) -> Result<Value, Exception> {
    spanner::get_bound(host, me, d)?
        .ok_or_else(|| crate::misc_error(host.heap(), "ly:hairpin", "hairpin without a bound"))
}

/// `Axis_group_interface::generic_bound_extent (b, common, X_AXIS)`.
pub(crate) fn generic_bound_extent(
    host: &mut dyn Host,
    b: Value,
    common: Value,
) -> Result<Interval, Exception> {
    let axis = host.heap().integer(0);
    let e = crate::call_lily(host, "%generic-bound-extent", &[b, common, axis])?;
    Ok(grob::number_pair(host.heap(), e).unwrap_or(Interval::EMPTY))
}

/// `pure_height`: half the `height` (0) in staff spaces and half the line
/// `thickness` (1) to each side.
pub fn pure_height(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let mut height = real_or(host, me, "height", 0.0)? * ssr::staff_space(host, me)?;

    let thickness = real_or(host, me, "thickness", 1.0)? * ssr::line_thickness(host, me)?;

    height += thickness / 2.0;
    Ok(crate::offset(host.heap(), (-height, height)))
}

/// `broken_bound_padding`: half the `bound-padding` (0.5) where the
/// hairpin ends at a line break whose bar line has the span bar that its
/// staff's neighbour on the side of its line spanner's `direction` has
/// too; else 0, with LilyPond's warning at a bound that is not broken.
pub fn broken_bound_padding(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let zero = host.heap().flonum(0.0);
    let r_bound = bound(host, me, RIGHT)?;
    if item::break_status_dir(host.heap(), r_bound) != LEFT {
        grob::warning(
            host,
            me,
            "Asking for broken bound padding at a non-broken bound.",
        )?;
        return Ok(zero);
    }

    let sys = spanner::get_system(host.heap(), me);
    let dir = match grob::get_parent(host.heap(), me, Y) {
        Some(parent) => get_grob_direction(host, parent)?,
        None => 0,
    };
    if dir == 0 {
        return Ok(zero);
    }

    let my_vertical_axis_group = grob::vertical_axis_group(host.heap(), me);
    let mut vertical_axis_groups = [None; 2];
    for d in [DOWN, UP] {
        vertical_axis_groups[side(d)] = if d == dir {
            match sys {
                Some(sys) => {
                    let ranks = spanner::spanned_column_rank_interval(host.heap(), me);
                    system::get_neighboring_staff(host, sys, d, my_vertical_axis_group, ranks)?
                }
                None => None,
            }
        } else {
            my_vertical_axis_group
        };
    }

    if vertical_axis_groups[side(dir)].is_none() {
        return Ok(zero);
    }

    let mut span_bars = [None; 2];
    for d in [DOWN, UP] {
        if let Some(group) = vertical_axis_groups[side(d)] {
            let elts = grob_array::extract_grob_array(host, group, "elements")?;
            for &e in elts.iter().rev() {
                if grob::has_interface(host.heap(), e, "bar-line-interface")
                    && item::break_status_dir(host.heap(), e) == LEFT
                {
                    let sym = host.heap().intern("has-span-bar");
                    let hsb = grob::object(host, e, sym)?;
                    let heap = host.heap();
                    let (Some(up), Some(down)) = (heap.car(hsb), heap.cdr(hsb)) else {
                        break;
                    };
                    let g = if d == UP { up } else { down };
                    span_bars[side(d)] = grob_of(heap, g).map(|_| g);
                    break;
                }
            }
        }

        if span_bars[side(d)].is_none() {
            return Ok(zero);
        }
    }

    if span_bars[0] != span_bars[1] {
        return Ok(zero);
    }

    let padding = real_or(host, me, "bound-padding", 0.5)? / 2.0;
    Ok(host.heap().flonum(padding))
}

/// `print`: the two lines of the hairpin between its bounds, each end
/// placed by what its bound is (broken, a text, a neighbouring hairpin or
/// dynamic text spanner, a rest, a note column by `endpoint-alignments`),
/// shortened by `shorten-pair`, with `circled-tip`'s circle; a hairpin
/// with no `grow-direction` killed. LilyPond's warnings for an endpoint
/// alignment that is no direction and for a hairpin too short.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let s = grob::property(host, me, "grow-direction")?;
    if !matches!(s.as_fixnum(), Some(-1..=1)) {
        grob::suicide(host.heap(), me);
        return Ok(Value::NULL);
    }

    let grow_dir = s.as_fixnum().unwrap_or(0) as Direction;
    let padding = real_or(host, me, "bound-padding", 0.5)?;

    let bounds = [bound(host, me, LEFT)?, bound(host, me, RIGHT)?];
    let mut broken = [false; 2];
    for d in [LEFT, RIGHT] {
        broken[side(d)] = item::break_status_dir(host.heap(), bounds[side(d)]) != 0;
    }

    if broken[1] {
        let next = spanner::broken_neighbor(host.heap(), me, RIGHT);
        // Hairpin-parts suicide in after-line-breaking if they need not be drawn
        broken[1] = match next {
            Some(next) => {
                grob::property(host, next, "after-line-breaking")?;
                grob::is_live_grob(host.heap(), next)
            }
            None => false,
        };
    }

    let common = grob::common_refpoint(host.heap(), bounds[0], bounds[1], X).unwrap_or(Value::NULL);
    let mut x_points = [0.0; 2];

    /*
      Use the height and thickness of the hairpin when making a circled tip
    */
    let circled_tip = grob::property(host, me, "circled-tip")? != Value::FALSE;
    let height = real_or(host, me, "height", 0.2)? * ssr::staff_space(host, me)?;
    /*
      FIXME: 0.525 is still just a guess...
             same method is used in `circle-radius' of scm/output-lib.scm
    */
    let rad = height * 0.525;
    let mut thick = 1.0;
    if circled_tip {
        thick = real_or(host, me, "thickness", 1.0)? * ssr::line_thickness(host, me)?;
    }
    let mut shorten = drul_property(host, me, "shorten-pair", [0.0, 0.0])?;

    let mut endpoint_alignments = drul_property(host, me, "endpoint-alignments", [-1.0, 1.0])?;

    for d in [LEFT, RIGHT] {
        let alignment = endpoint_alignments[side(d)];
        let sanitized_alignment = sign(alignment);
        if alignment != f64::from(sanitized_alignment) {
            grob::warning(
                host,
                me,
                &format!(
                    "hairpin: '{alignment:.6}' is not a valid direction for property \
                     'endpoint-alignments', setting to '{sanitized_alignment}'"
                ),
            )?;
            endpoint_alignments[side(d)] = f64::from(sanitized_alignment);
        }
    }

    for d in [LEFT, RIGHT] {
        let i = side(d);
        let b = bounds[i];
        let e = generic_bound_extent(host, b, common)?;

        x_points[i] = grob::relative_coordinate(host, b, common, X)?;
        if broken[i] {
            if d == LEFT {
                x_points[i] = e.at(-d) + padding;
            } else {
                let mut broken_bound_padding = real_or(host, me, "broken-bound-padding", 0.0)?;
                let chp = grob_array::extract_grob_array(host, me, "concurrent-hairpins")?;
                for span_elt in chp {
                    let right = bound(host, span_elt, RIGHT)?;
                    if item::break_status_dir(host.heap(), right) == LEFT {
                        broken_bound_padding = f64::max(
                            broken_bound_padding,
                            real_or(host, span_elt, "broken-bound-padding", 0.0)?,
                        );
                    }
                }
                x_points[i] -= f64::from(d) * broken_bound_padding;
            }
        } else if grob::has_interface(host.heap(), b, "text-interface") {
            if !e.is_empty() {
                x_points[i] = e.at(-d) - f64::from(d) * padding;
            }
        } else {
            let mut adjacent = None;
            let neighbors = grob_array::extract_grob_array(host, me, "adjacent-spanners")?;
            for n in neighbors {
                /*
                  FIXME: this will fuck up in case of polyphonic
                  notes in other voices. Need to look at note-columns
                  in the current staff/voice.
                */
                if !spanner::is_spanner(host.heap(), n) {
                    continue;
                }
                let other = bound(host, n, -d)?;
                let heap = host.heap();
                if item::get_column(heap, other) == item::get_column(heap, b) {
                    adjacent = Some(n);
                    break;
                }
            }

            if let Some(adjacent) = adjacent {
                if grob::has_interface(host.heap(), adjacent, "hairpin-interface") {
                    /*
                      Handle back-to-back hairpins with a circle in the middle
                    */
                    if circled_tip && grow_dir != d {
                        x_points[i] = e.center() + f64::from(d) * (rad - thick / 2.0);
                        shorten[i] = 0.0;
                    }
                    /*
                      If we're hung on a paper column, that means we're not
                      adjacent to a text-dynamic, and we may move closer. We
                      make the padding a little smaller, here.
                    */
                    else {
                        x_points[i] = e.center() - f64::from(d) * padding / 3.0;
                    }
                }
                // Our neighbor is a dynamic text spanner.
                // If we end on the text, pad as for text dynamics
                else if d == RIGHT {
                    x_points[i] = e.at(-d) - f64::from(d) * padding;
                }
            } else {
                if d == RIGHT // end at the left edge of a rest
                    && grob::has_interface(host.heap(), b, "note-column-interface")
                    && note_column::has_rests(host, b)?
                {
                    x_points[i] = e.at(-d);
                } else {
                    // Endpoint alignment relative to NoteColumn
                    let alignment = endpoint_alignments[i];
                    x_points[i] = if alignment == 0.0 {
                        e.center()
                    } else if alignment != f64::from(d) {
                        e.at(-d)
                    } else {
                        e.at(d)
                    };
                }

                if item::is_non_musical(host, b)? {
                    x_points[i] -= f64::from(d) * padding;
                }
            }
        }

        x_points[i] -= shorten[i] * f64::from(d);
    }

    let mut width = x_points[1] - x_points[0];

    if width < 0.0 {
        let text = if grow_dir == SMALLER {
            "decrescendo too small"
        } else {
            "crescendo too small"
        };
        grob::warning(host, me, text)?;
        width = 0.0;
    }

    let continued = broken[side(-grow_dir)];
    let continuing = broken[side(grow_dir)];

    let (starth, endh) = if grow_dir == SMALLER {
        (
            if continuing {
                2.0 * height / 3.0
            } else {
                height
            },
            if continued { height / 3.0 } else { 0.0 },
        )
    } else {
        (
            if continued { height / 3.0 } else { 0.0 },
            if continuing {
                2.0 * height / 3.0
            } else {
                height
            },
        )
    };

    /*
      should do relative to staff-symbol staff-space?
    */
    let mut x = 0.0;

    /*
      Compensate for size of circle
    */
    let tip_dir = -grow_dir;
    if circled_tip && !broken[side(tip_dir)] {
        if grow_dir == BIGGER {
            x = rad * 2.0;
        } else if grow_dir == SMALLER {
            width -= rad * 2.0;
        }
    }
    let mut mol = grob_line(host, me, (x, starth), (width, endh))?;
    let second = grob_line(host, me, (x, -starth), (width, -endh))?;
    mol.add_stencil(host.heap(), &second);
    host.heap().root(mol.expr);

    /*
      Support al/del niente notation by putting a circle at the
      tip of the (de)crescendo.
    */
    if circled_tip {
        let extent = Box::new(Interval::new(-rad, rad), Interval::new(-rad, rad));

        /* Hmmm, perhaps we should have a Lookup::circle () method? */
        let heap = host.heap();
        let head = heap.intern("circle");
        let (r, t) = (heap.flonum(rad), heap.flonum(thick));
        let expr = heap.list(&[head, r, t, Value::FALSE]);
        let circle = Stencil::new(extent, expr);

        /*
          don't add another circle if the hairpin is broken
        */
        if !broken[side(tip_dir)] {
            mol.add_at_edge(heap, X, tip_dir, &circle, 0.0);
        }
        heap.root(mol.expr);
    }

    let left = grob::relative_coordinate(host, bounds[0], common, X)?;
    let heap = host.heap();
    mol.translate_axis(heap, x_points[0] - left, X);
    Ok(heap.wrap(mol))
}

/// `sign`: -1, 0 or 1.
fn sign(x: f64) -> i32 {
    if x < 0.0 {
        -1
    } else if x > 0.0 {
        1
    } else {
        0
    }
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:hairpin::` primitives (D127): the
/// callback its second argument names, of the grob given, with the VM as
/// its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "hairpin",
    f: host_hairpin,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 3] = [
    ("broken-bound-padding", broken_bound_padding),
    ("print", print),
    ("pure-height", pure_height),
];

fn host_hairpin(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such hairpin callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:hairpin::… spanner)`, the pure one `(… grob start end)`:
/// LilyPond's subrs, primitives here so that they print as LilyPond's.
/// Each checks its grob ([the module's](self)) and tail-calls
/// `%host-call` with its name ([`HOSTS`]).
fn callback(cx: &mut Cx, what: &str, grob: Value) -> R {
    if what == "pure-height" {
        grob::grob_arg(cx, "ly:pure-height", 1, grob)?;
    } else {
        let subr = format!("ly:hairpin::{what}");
        spanner::spanner_arg(cx, &subr, 1, grob)?;
    }
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let hairpin = cx.heap_mut().intern("hairpin");
    let what = cx.heap_mut().intern(what);
    Ok(cx.tail_call(f, &[hairpin, what, grob]))
}

macro_rules! callbacks {
    ($($f:ident $what:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            callback(cx, $what, args[0])
        })*
    };
}

callbacks! {
    ly_broken_bound_padding "broken-bound-padding",
    ly_print "print",
    ly_pure_height "pure-height",
}
