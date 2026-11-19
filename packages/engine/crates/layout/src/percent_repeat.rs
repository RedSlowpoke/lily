//! `percent-repeat-interface.cc` (MS8 step 7, part 2, 134 lines): the
//! stencils of the percent repeats' grobs, `ly:percent-repeat-interface::`
//! `percent` (a `PercentRepeat`'s), `double-percent` (a
//! `DoublePercentRepeat`'s) and `beat-slash` (a `RepeatSlash`'s and a
//! `DoubleRepeatSlash`'s), the music crate's placeholders until then (D67).
//! Rust under primitives that print as LilyPond's subrs and call it over
//! `%host-call` (D127, [`HOSTS`]), as the clef's are.
//!
//! - `brew_slash` (`:37–59`): `count` slashes of `Lookup::repeat_slash`,
//!   `slope` and `thickness` scaled by the staff space alone, each the
//!   next one's `slash-negative-kern` into it, centred on Y.
//! - `x_percent` (`:61–80`): the slashes with the default font's
//!   `dots.dot` half a staff space above at their left and below at their
//!   right, `dot-negative-kern` into them.
//! - `percent`, `double_percent` (centred on X) and `beat_slash` (`:82–123`):
//!   a `slash-count` of 0 on the cause a double percent's figure.

use lily_engine_music::{Host, StreamEvent, event_property};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::font_select;
use crate::geometry::{Direction, X, Y};
use crate::grob;
use crate::lookup;
use crate::spacing_options::double_or;
use crate::staff_symbol_referencer as ssr;
use crate::stencil::Stencil;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// `from_scm<double> (get_property (me, name), fallback)`.
fn real_or(host: &mut dyn Host, me: Value, name: &str, fallback: f64) -> Result<f64, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(double_or(host, v, fallback))
}

/// The stencil `s`, its expression rooted until the caller unroots it.
fn kept(host: &mut dyn Host, s: Stencil) -> Stencil {
    host.heap().root(s.expr);
    s
}

/// `brew_slash`: `count` slashes, scaled by the staff space and not by the
/// line thickness, so that the sign keeps its proportions at every staff
/// size.
fn brew_slash(host: &mut dyn Host, me: Value, count: i64) -> Result<Stencil, Exception> {
    let staff_space = ssr::staff_space(host, me)?;
    let slope = real_or(host, me, "slope", 1.0)?;
    let wid = 2.0 / slope * staff_space;
    let thick = real_or(host, me, "thickness", 1.0)? * staff_space;
    let slash = lookup::repeat_slash(host.heap(), wid, slope, thick);
    let slash = kept(host, slash);
    let mut m = slash;

    let slash_neg_kern = real_or(host, me, "slash-negative-kern", 1.6)? * staff_space;
    // `for (int i = count - 1; i--;)`: count - 1 more.
    for _ in 1..count {
        m.add_at_edge(host.heap(), X, RIGHT, &slash, -slash_neg_kern);
        host.heap().root(m.expr);
    }

    m.align_to(host.heap(), Y, 0.0);
    Ok(kept(host, m))
}

/// `x_percent`: the slashes between the two dots of a percent sign.
fn x_percent(host: &mut dyn Host, me: Value, count: i64) -> Result<Stencil, Exception> {
    let staff_space = ssr::staff_space(host, me)?;
    let mut m = brew_slash(host, me, count)?;

    let dot_neg_kern = real_or(host, me, "dot-negative-kern", 0.75)? * staff_space;

    let font = font_select::get_default_font(host, me)?;
    host.heap().root(font);
    let d1 = crate::font::find_by_name(host.heap(), font, "dots.dot");
    let mut d1 = kept(host, d1);
    let mut d2 = d1;
    d1.translate_axis(host.heap(), 0.5 * staff_space, Y);
    host.heap().root(d1.expr);
    d2.translate_axis(host.heap(), -0.5 * staff_space, Y);
    host.heap().root(d2.expr);

    m.add_at_edge(host.heap(), X, LEFT, &d1, -dot_neg_kern);
    host.heap().root(m.expr);
    m.add_at_edge(host.heap(), X, RIGHT, &d2, -dot_neg_kern);
    Ok(kept(host, m))
}

/// The stencil of `f`'s figure as a value, what it rooted unrooted again.
fn printed(
    host: &mut dyn Host,
    me: Value,
    f: fn(&mut dyn Host, Value) -> Result<Stencil, Exception>,
) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let m = f(host, me);
    let v = m.map(|m| host.heap().wrap(m));
    host.heap().unroot_to(depth);
    v
}

/// `percent`: one slash between the dots.
pub fn percent(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    printed(host, me, |host, me| x_percent(host, me, 1))
}

/// `double_percent`: two slashes between the dots, centred on X.
pub fn double_percent(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    printed(host, me, |host, me| {
        let mut m = x_percent(host, me, 2)?;
        m.align_to(host.heap(), X, 0.0);
        Ok(kept(host, m))
    })
}

/// `beat_slash`: the cause's `slash-count` slashes (1 without one), or a
/// double percent's figure for 0.
pub fn beat_slash(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    printed(host, me, |host, me| {
        let cause = grob::property(host, me, "cause")?;
        let heap = host.heap();
        let count = if heap.foreign::<StreamEvent>(cause).is_some() {
            let v = event_property(heap, cause, "slash-count");
            heap.to_i64(v)
                .and_then(|n| i32::try_from(n).ok())
                .map_or(1, i64::from)
        } else {
            1
        };
        if count == 0 {
            x_percent(host, me, 2)
        } else {
            brew_slash(host, me, count)
        }
    })
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:percent-repeat-interface::`
/// primitives (D127): the callback its second argument names, of the grob
/// given, with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "percent-repeat-interface",
    f: host_percent_repeat,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 3] = [
    ("beat-slash", beat_slash),
    ("double-percent", double_percent),
    ("percent", percent),
];

fn host_percent_repeat(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such percent repeat callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:percent-repeat-interface::… grob)`: LilyPond's subrs, primitives
/// here so that they print as LilyPond's. Each checks its grob and
/// tail-calls `%host-call` with its name ([`HOSTS`]).
fn callback(cx: &mut Cx, subr: &str, what: &str, grob: Value) -> R {
    grob::grob_arg(cx, subr, 1, grob)?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let module = cx.heap_mut().intern("percent-repeat-interface");
    let what = cx.heap_mut().intern(what);
    Ok(cx.tail_call(f, &[module, what, grob]))
}

macro_rules! callbacks {
    ($($f:ident $what:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            callback(cx, concat!("ly:percent-repeat-interface::", $what), $what, args[0])
        })*
    };
}

callbacks! {
    ly_beat_slash "beat-slash",
    ly_double_percent "double-percent",
    ly_percent "percent",
}
