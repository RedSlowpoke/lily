//! `lyric-hyphen.cc` (MS4 step 15): `ly:lyric-hyphen::print` (`:36–155`)
//! and `ly:lyric-hyphen::set-spacing-rods` (`:157–195`), the music
//! crate's placeholders until then (D67), Rust under primitives that print
//! as LilyPond's subrs and call it over `%host-call` (D127, [`HOSTS`]), in
//! 05 §5.1's callback table (`callbacks.rs`).
//!
//! - `print`: dashes `dash-period` apart between the bounds' extents, as
//!   many as fit, centred; one shortened to fit a short span, none where it
//!   cannot fit unless the line breaks there; each with a whiteout box
//!   behind it where `whiteout` is positive. Nothing for a broken piece
//!   whose left bound's moment is its right bound's column's, unless
//!   `after-line-breaking` is `#t`.
//! - `set-spacing-rods`: a rod of `minimum-distance` between the bounds,
//!   and, where a line may break between them and `after-line-breaking` is
//!   `#t` as data, one of `length` and twice the `padding` from the last
//!   break's piece that begins a line to the right bound.
//!
//! clang fuses the space left over, each dash's position and the whiteout
//! box's sides into `fmadd`s and `fmsub`s (otool): [`fma`] here.

use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::{Box, Direction, Interval, X, fma};
use crate::grob;
use crate::hairpin::generic_bound_extent;
use crate::item;
use crate::lookup;
use crate::paper_column;
use crate::rod::Rod;
use crate::spacing_options::double_or;
use crate::spanner;
use crate::stencil::Stencil;
use crate::system;
use crate::tie_configuration::property_data;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// `from_scm<double> (get_property (me, name), fallback)`.
fn real_or(host: &mut dyn Host, me: Value, name: &str, fallback: f64) -> Result<f64, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(double_or(host, v, fallback))
}

/// The spanner's two bounds, if it has both.
fn bounds(host: &mut dyn Host, me: Value) -> Result<Option<[Value; 2]>, Exception> {
    let left = spanner::get_bound(host, me, LEFT)?;
    let right = spanner::get_bound(host, me, RIGHT)?;
    Ok(left.zip(right).map(|(l, r)| [l, r]))
}

/// `round_filled_box`, its warnings given and its expression rooted.
fn filled_box(host: &mut dyn Host, b: Box, blot: f64) -> Result<Stencil, Exception> {
    let mut warnings = Vec::new();
    let s = lookup::round_filled_box(host.heap(), b, blot, &mut warnings);
    host.heap().root(s.expr);
    for text in &warnings {
        grob::unlocated_warning(host, text)?;
    }
    Ok(s)
}

/// `print`: the hyphen's dashes, or `'()`.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let Some(bounds) = bounds(host, me)? else {
        return Ok(Value::NULL);
    };
    // FIXME: does this bring anything more than
    // ly:spanner::kill-zero-spanned-time in
    // after-line-breaking? --JeanAS
    if item::break_status_dir(host.heap(), bounds[0]) != 0 {
        let left = paper_column::when_mom(host, bounds[0])?;
        let right_column = item::get_column(host.heap(), bounds[1]).unwrap_or(Value::NULL);
        let right = paper_column::when_mom(host, right_column)?;
        if left.compare(right).is_eq()
            && grob::property(host, me, "after-line-breaking")? != Value::TRUE
        {
            return Ok(Value::NULL);
        }
    }

    let Some(common) = grob::common_refpoint(host.heap(), bounds[0], bounds[1], X) else {
        return Ok(Value::NULL);
    };

    let mut span_points = Interval::new(0.0, 0.0);
    for (i, d) in [LEFT, RIGHT].into_iter().enumerate() {
        let iv = generic_bound_extent(host, bounds[i], common)?;
        *span_points.at_mut(d) = if iv.is_empty() {
            grob::relative_coordinate(host, bounds[i], common, X)?
        } else {
            iv.at(-d)
        };
    }

    let lt = grob::layout_dimension(host, me, "line-thickness")?;
    let th = real_or(host, me, "thickness", 1.0)? * lt;
    let font_size_step = real_or(host, me, "font-size", 0.0)?;
    // `pow (2.0, font_size_step / 6.0)`: libm's `exp2`, as clang calls it.
    let h = real_or(host, me, "height", 0.5)? * libm::exp2(font_size_step / 6.0);

    // interval?
    let mut dash_period = real_or(host, me, "dash-period", 1.0)?;
    let mut dash_length = real_or(host, me, "length", 0.5)?;
    let padding = real_or(host, me, "padding", 0.1)?;
    let whiteout = real_or(host, me, "whiteout", -1.0)?;

    // A color's name, or its red, green, blue and alpha, each 1 if left
    // out, as `stencil-with-color` takes either.
    let whiteout_color = grob::property(host, me, "whiteout-color")?;
    let whiteout_color = if whiteout_color.tag() == lily_engine_scheme::Tag::String {
        whiteout_color
    } else {
        let heap = host.heap();
        let mut rgba = [1.0; 4];
        let mut p = whiteout_color;
        for c in &mut rgba {
            let (Some(v), Some(rest)) = (heap.car(p), heap.cdr(p)) else {
                break;
            };
            *c = heap.to_f64(v).unwrap_or(1.0);
            p = rest;
        }
        let items = rgba.map(|c| heap.flonum(c));
        heap.list(&items)
    };
    host.heap().root(whiteout_color);

    if dash_period < dash_length {
        dash_period = 1.5 * dash_length;
    }

    let l = span_points.length();

    let mut n = libm::ceil(l / dash_period - 0.5) as i32;
    if n <= 0 {
        n = 1;
    }

    if l < fma(padding, 2.0, dash_length) && item::break_status_dir(host.heap(), bounds[1]) == 0 {
        let minimum_length = real_or(host, me, "minimum-length", 0.3)?;
        // `std::max (l - 2 * padding, minimum_length)`.
        let fitted = fma(padding, -2.0, l);
        dash_length = if fitted < minimum_length {
            minimum_length
        } else {
            fitted
        };
    }

    let mut space_left = fma(-f64::from(n - 1), dash_period, l - dash_length);

    /*
      If there is not enough space, the hyphen should disappear, but not
      at the end of the line.
    */
    if space_left < 0.0 && item::break_status_dir(host.heap(), bounds[1]) == 0 {
        return Ok(Value::NULL);
    }

    space_left = space_left.max(0.0);

    let b = Box::new(Interval::new(0.0, dash_length), Interval::new(h, h + th));
    let dash_mol = filled_box(host, b, 0.8 * lt)?;

    let mut total = Stencil::default();
    for i in 0..n {
        let mut m = dash_mol;
        let x = fma(f64::from(i), dash_period, span_points.left) + space_left / 2.0;
        m.translate_axis(host.heap(), x, X);
        total.add_stencil(host.heap(), &m);
        host.heap().root(total.expr);
        if whiteout > 0.0 {
            let c = Box::new(
                Interval::new(0.0, fma(2.0 * whiteout, lt, dash_length)),
                Interval::new(fma(-whiteout, lt, h), fma(whiteout, lt, h + th)),
            );
            let w = filled_box(host, c, 0.8 * lt)?;
            let mut w = w.in_color(host, whiteout_color)?;
            w.translate_axis(host.heap(), fma(-whiteout, lt, x), X);
            total.add_stencil(host.heap(), &w);
            host.heap().root(total.expr);
        }
    }

    let x = grob::relative_coordinate(host, me, common, X)?;
    total.translate_axis(host.heap(), -x, X);
    Ok(host.heap().wrap(total))
}

/// `set_spacing_rods`: the hyphen's rods, as above.
pub fn set_spacing_rods(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let Some(root) = system::get_root_system(host.heap(), me) else {
        return Ok(Value::UNSPECIFIED);
    };
    let Some(bounds) = bounds(host, me)? else {
        return Ok(Value::UNSPECIFIED);
    };

    let left_column = item::get_column(host.heap(), bounds[0]).unwrap_or(bounds[0]);
    let right_column = item::get_column(host.heap(), bounds[1]).unwrap_or(bounds[1]);
    let cols = system::broken_col_range(host, root, left_column, right_column)?;

    let mut rod = Rod {
        item_drul: [Some(bounds[0]), Some(bounds[1])],
        distance: real_or(host, me, "minimum-distance", 0.0)?,
    };
    rod.distance += rod.bounds_protrusion(host)?;
    rod.add_to_cols(host)?;

    if let Some(&last) = cols.last()
        && property_data(host, me, "after-line-breaking") == Value::TRUE
    {
        let mut rod_after_break = Rod {
            item_drul: [
                item::find_prebroken_piece(host.heap(), last, RIGHT),
                Some(bounds[1]),
            ],
            distance: real_or(host, me, "length", 0.5)?,
        };
        let padding = real_or(host, me, "padding", 0.1)?;
        rod_after_break.distance = fma(padding, 2.0, rod_after_break.distance);
        rod_after_break.distance += rod_after_break.bounds_protrusion(host)?;
        rod_after_break.add_to_cols(host)?;
    }

    Ok(Value::UNSPECIFIED)
}

/// What `%host-call` runs for the `ly:lyric-hyphen::` callbacks (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "lyric-hyphen",
    f: host_lyric_hyphen,
}];

/// The `ly:lyric-hyphen::` callbacks, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 2] =
    [("print", print), ("set-spacing-rods", set_spacing_rods)];

fn host_lyric_hyphen(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such lyric-hyphen callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(cx, "lyric-hyphen", "ly:lyric-hyphen::", "print", args)
}

pub(crate) fn ly_set_spacing_rods(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "lyric-hyphen",
        "ly:lyric-hyphen::",
        "set-spacing-rods",
        args,
    )
}
