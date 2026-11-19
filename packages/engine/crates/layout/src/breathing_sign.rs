//! `breathing-sign.cc` (MS3 step 48, OPEN A36): the breathing sign's
//! `Y-offset`, `ly:breathing-sign::offset-callback` (`:259–277`, 19
//! lines), the music crate's placeholder until then (D67), which no part's
//! split had listed: until it was, the traced dump's pure read of a
//! `BreathingSign` threw inside the pure Y offset's guard, and the next
//! read reported a cycle (T15). Rust under a primitive that prints as
//! LilyPond's subr and calls it over `%host-call` (D127, [`HOSTS`]), as the
//! clef's are.
//!
//! Since MS8 step 9's part 2 (OPEN A38) the Gregorian divisiones' stencils
//! too, the music crate's placeholders until then: `::divisio-minima`
//! (`:93–118`, a stroke a staff space long), `::divisio-maior` (`:123–194`,
//! a stroke over half the staff, its ends in the middle of staff spaces),
//! `::divisio-maxima` (`:199–218`, over the staff) and `::finalis`
//! (`:223–245`, two strokes over it half a staff space apart), each as
//! thick as the staff's lines times the sign's `thickness`, its corners
//! rounded by the layout's `blot-diameter`.

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use alloc::vec::Vec;

use crate::callbacks::Callback;
use crate::direction::get_strict_grob_direction;
use crate::geometry::{Box, Interval, X};
use crate::spacing_options::double_or;
use crate::staff_symbol_referencer as ssr;
use crate::stencil::Stencil;
use crate::{grob, lookup};

type Run<T> = Result<T, Exception>;

/// `offset_callback`: half a staff space times the end of the staff's line
/// span on the sign's side (its `direction`, up unless down); 0 without a
/// staff symbol.
pub fn offset_callback(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let d = get_strict_grob_direction(host, me)?;
    let offset = if ssr::staff_symbol(host, me)?.is_some() {
        let iv = ssr::staff_span(host, me)?;
        let inter = ssr::staff_space(host, me)? / 2.0;
        inter * iv.at(d)
    } else {
        0.0
    };
    Ok(host.heap().flonum(offset))
}

/// The staff's line thickness times the sign's `thickness` (1 if unset),
/// and the layout's `blot-diameter`.
fn thickness_and_blot(host: &mut dyn Host, me: Value) -> Run<(f64, f64)> {
    let mut thickness = ssr::line_thickness(host, me)?;
    let v = grob::property(host, me, "thickness")?;
    thickness *= double_or(host, v, 1.0);
    let blotdiameter = grob::layout_dimension(host, me, "blot-diameter")?;
    Ok((thickness, blotdiameter))
}

/// A stroke `thickness` wide over `ydim`, its corners rounded.
fn stroke(host: &mut dyn Host, thickness: f64, ydim: Interval, blot: f64) -> Run<Stencil> {
    let mut warnings = Vec::new();
    let b = Box::new(Interval::new(0.0, thickness), ydim);
    let out = lookup::round_filled_box(host.heap(), b, blot, &mut warnings);
    for text in &warnings {
        grob::unlocated_warning(host, text)?;
    }
    Ok(out)
}

/// `divisio_minima` (`:93–118`): a small vertical line through the
/// uppermost (or, depending on direction, lowermost) staff line.
pub fn divisio_minima(host: &mut dyn Host, me: Value) -> Run<Value> {
    let staff_space = ssr::staff_space(host, me)?;
    let (thickness, blotdiameter) = thickness_and_blot(host, me)?;
    let ydim = Interval::new(-0.5 * staff_space, 0.5 * staff_space);
    let out = stroke(host, thickness, ydim, blotdiameter)?;
    Ok(host.heap().wrap(out))
}

/// `divisio_maior` (`:123–194`): a vertical line roughly centered in the
/// staff (just like a bar), at least half the staff long and its ends in
/// the middle of staff spaces; the first wins where they contradict (the
/// first or last space larger than half the staff).
pub fn divisio_maior(host: &mut dyn Host, me: Value) -> Run<Value> {
    let (thickness, blotdiameter) = thickness_and_blot(host, me)?;

    let mut ydim = Interval::new(0.0, 0.0);
    if let Some(staff) = ssr::staff_symbol(host, me)? {
        let v = grob::property(host, staff, "line-positions")?;
        let mut line_pos = Vec::new();
        let mut l = v;
        while let (Some(x), Some(rest)) = (host.heap().car(l), host.heap().cdr(l)) {
            line_pos.push(grob::to_double(host.heap(), x)?);
            l = rest;
        }
        if !line_pos.is_empty() {
            line_pos.sort_by(f64::total_cmp);
            let n = line_pos.len();
            ydim = Interval::new(line_pos[0], line_pos[n - 1]);

            let height = ydim.length();
            if height != 0.0 {
                ydim.widen(-0.25 * height);

                // ydim has now the required height; to satisfy req. 2
                // find the staff spaces containing current endpoints:
                // the space below a line exactly at a quarter of the
                // staff, above one exactly at three quarters; if the
                // middle of the space found is not low/high enough, take
                // the next space (if there are no more spaces, ydim won't
                // be enlarged further).
                let it = line_pos.partition_point(|&x| x < ydim.left);
                if it > 0 && it < n {
                    let mut val = (line_pos[it - 1] + line_pos[it]) / 2.0;
                    if ydim.left < val && it > 1 {
                        val = (line_pos[it - 2] + line_pos[it - 1]) / 2.0;
                    }
                    ydim.add_point(val);
                }

                let it = line_pos.partition_point(|&x| x <= ydim.right);
                if it > 0 && it < n {
                    let mut val = (line_pos[it - 1] + line_pos[it]) / 2.0;
                    if val < ydim.right && it + 1 < n {
                        val = (line_pos[it] + line_pos[it + 1]) / 2.0;
                    }
                    ydim.add_point(val);
                }
            }
        }
    }

    ydim.scale(ssr::staff_space(host, me)? / 2.0);

    let out = stroke(host, thickness, ydim, blotdiameter)?;
    Ok(host.heap().wrap(out))
}

/// `divisio_maxima` (`:199–218`): like a "|" type bar.
pub fn divisio_maxima(host: &mut dyn Host, me: Value) -> Run<Value> {
    let staff_space = ssr::staff_space(host, me)?;
    let (thickness, blotdiameter) = thickness_and_blot(host, me)?;
    let mut ydim = ssr::staff_span(host, me)?;
    ydim.scale(staff_space / 2.0);
    let out = stroke(host, thickness, ydim, blotdiameter)?;
    Ok(host.heap().wrap(out))
}

/// `finalis` (`:223–245`): like a "||" type bar.
pub fn finalis(host: &mut dyn Host, me: Value) -> Run<Value> {
    let staff_space = ssr::staff_space(host, me)?;
    let (thickness, blotdiameter) = thickness_and_blot(host, me)?;
    let mut ydim = ssr::staff_span(host, me)?;
    ydim.scale(staff_space / 2.0);
    let depth = host.heap().root_depth();
    let mut line1 = stroke(host, thickness, ydim, blotdiameter)?;
    let heap = host.heap();
    heap.root(line1.expr);
    let mut line2 = line1;
    line2.translate_axis(heap, 0.5 * staff_space, X);
    heap.root(line2.expr);
    line1.add_stencil(heap, &line2);
    let out = heap.wrap(line1);
    heap.unroot_to(depth);
    Ok(out)
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:breathing-sign::` primitive (D127):
/// the callback its second argument names, of the grob given, with the VM
/// as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "breathing-sign",
    f: host_breathing_sign,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 5] = [
    ("offset-callback", offset_callback),
    ("divisio-minima", divisio_minima),
    ("divisio-maior", divisio_maior),
    ("divisio-maxima", divisio_maxima),
    ("finalis", finalis),
];

fn host_breathing_sign(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such breathing sign callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:breathing-sign::offset-callback grob)`: LilyPond's subr, a
/// primitive here so that it prints as LilyPond's. It checks its grob under
/// the method's name, as LilyPond's `LY_ASSERT_SMOB` gives it, and
/// tail-calls `%host-call` ([`HOSTS`]).
pub(crate) fn ly_offset_callback(cx: &mut Cx, args: &[Value]) -> R {
    grob::grob_arg(cx, "ly:offset-callback", 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("breathing-sign");
    let name = cx.heap_mut().intern("offset-callback");
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}

/// The divisiones' subrs, over `%host-call` ([`HOSTS`]).
pub(crate) fn ly_divisio_minima(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "breathing-sign",
        "ly:breathing-sign::",
        "divisio-minima",
        args,
    )
}

pub(crate) fn ly_divisio_maior(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "breathing-sign",
        "ly:breathing-sign::",
        "divisio-maior",
        args,
    )
}

pub(crate) fn ly_divisio_maxima(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "breathing-sign",
        "ly:breathing-sign::",
        "divisio-maxima",
        args,
    )
}

pub(crate) fn ly_finalis(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(cx, "breathing-sign", "ly:breathing-sign::", "finalis", args)
}
