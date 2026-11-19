//! `piano-pedal-bracket.cc` and `sustain-pedal.cc` (MS8 step 8, part 1,
//! 115 and 76 lines): `ly:piano-pedal-bracket::print` and
//! `ly:sustain-pedal::print`, the music crate's placeholders until then
//! (D67), Rust under primitives that print as LilyPond's subrs and call it
//! over `%host-call` (D127, [`HOSTS`]).
//!
//! - The bracket (`:34–106`): from its left bound to its right one, each
//!   bound's coordinate, or at a broken end its bound extent's right side,
//!   where the end of a middle piece has no edge and that of the first or
//!   last no flare; with `pedal-text` (the mixed style) its left end
//!   `bound-padding` past the text, without an edge. `Bracket::make_bracket`
//!   on Y of its `edge-height`, `bracket-flare` and `shorten-pair`, nothing
//!   for a span of 0.001 or less.
//! - The sustain pedal (`:47–73`): its `text`, each "Ped" the glyph
//!   `pedal.Ped` and each other byte `pedal.` and the byte, of the grob's
//!   font, side by side; nothing for a text that is no string.

use alloc::format;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::accidental::find_by_name;
use crate::bracket::{drul_property, make_bracket};
use crate::callbacks::Callback;
use crate::geometry::{Direction, Interval, X, Y};
use crate::hairpin::generic_bound_extent;
use crate::spacing_options::double_or;
use crate::stencil::Stencil;
use crate::{font_select, grob, item, spanner};

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// `Piano_pedal_bracket::print`, as above.
pub fn bracket_print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = bracket_print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn bracket_print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let orig = grob::original(host.heap(), me);

    let mut height = drul_property(host, me, "edge-height", [0.0, 0.0])?;
    let shorten = drul_property(host, me, "shorten-pair", [0.0, 0.0])?;
    let mut flare = drul_property(host, me, "bracket-flare", [0.0, 0.0])?;

    let left = spanner::get_bound(host, me, LEFT)?;
    let right = spanner::get_bound(host, me, RIGHT)?;
    // LilyPond's reads through a missing bound.
    let (Some(left), Some(right)) = (left, right) else {
        return Ok(host.heap().wrap(Stencil::default()));
    };
    let bounds = [left, right];
    let Some(mut common) = grob::common_refpoint(host.heap(), left, right, X) else {
        return Ok(host.heap().wrap(Stencil::default()));
    };
    let sym = host.heap().intern("pedal-text");
    let textbit = grob::object(host, me, sym)?;
    let textbit = lily_engine_music::grob_of(host.heap(), textbit).map(|_| textbit);

    if let Some(textbit) = textbit
        && let Some(c) = grob::common_refpoint(host.heap(), common, textbit, X)
    {
        common = c;
    }

    let mut span_points = Interval {
        left: 0.0,
        right: 0.0,
    };
    for (i, d) in [LEFT, RIGHT].into_iter().enumerate() {
        let b = bounds[i];
        let broken = item::break_status_dir(host.heap(), b) != 0;
        if broken {
            let middle = orig.is_some_and(|orig| {
                let index = grob::with_grob(host.heap(), me, |g| g.break_index).unwrap_or(0);
                let pieces = spanner::broken_intos(host.heap(), orig).len();
                (d == RIGHT && index != pieces.wrapping_sub(1)) || (d == LEFT && index != 0)
            });
            if middle {
                height[i] = 0.0;
            } else {
                flare[i] = 0.0;
            }
            *span_points.at_mut(d) = generic_bound_extent(host, b, common)?.right;
        } else {
            *span_points.at_mut(d) = grob::relative_coordinate(host, b, common, X)?;
        }
    }

    // For 'Mixed' style pedals, i.e. a bracket preceded by text: Ped._____|
    // need to shorten by the extent of the text grob
    if let Some(textbit) = textbit {
        height[0] = 0.0;

        let padding = grob::property(host, me, "bound-padding")?;
        let padding = double_or(host, padding, 0.0);

        span_points.left = padding + grob::robust_relative_extent(host, textbit, common, X)?.right;
    }

    let mut m = Stencil::default();
    if !span_points.is_empty() && span_points.length() > 0.001 {
        m = make_bracket(
            host,
            me,
            Y,
            (span_points.length(), 0.0),
            height,
            Interval::EMPTY,
            flare,
            shorten,
        )?;
    }
    let x = grob::relative_coordinate(host, me, common, X)?;
    m.translate_axis(host.heap(), span_points.left - x, X);
    Ok(host.heap().wrap(m))
}

/// `Sustain_pedal::print`, as above.
pub fn sustain_print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = sustain_print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn sustain_print_rooted(host: &mut dyn Host, e: Value) -> Result<Value, Exception> {
    let mut mol = Stencil::default();
    let glyph = grob::property(host, e, "text")?;
    let Some(text) = host
        .heap()
        .str(glyph)
        .map(|s| s.as_str().as_bytes().to_vec())
    else {
        return Ok(host.heap().wrap(mol));
    };

    let fm = font_select::get_default_font(host, e)?;
    host.heap().root(fm);
    let mut i = 0;
    while i < text.len() {
        let idx = if text[i..].starts_with(b"Ped") {
            i += 2;
            alloc::string::String::from("pedal.Ped")
        } else {
            format!("pedal.{}", char::from(text[i]))
        };
        let m = find_by_name(host, fm, &idx)?;
        if !m.is_empty() {
            mol.add_at_edge(host.heap(), X, RIGHT, &m, 0.0);
            host.heap().root(mol.expr);
        }
        i += 1;
    }

    Ok(host.heap().wrap(mol))
}

// --- the callbacks ------------------------------------------------------------

/// What `%host-call` runs for the `ly:piano-pedal-bracket::` and
/// `ly:sustain-pedal::` callbacks (D127).
pub(crate) const HOSTS: [HostHandler; 2] = [
    HostHandler {
        what: "piano-pedal-bracket",
        f: host_bracket,
    },
    HostHandler {
        what: "sustain-pedal",
        f: host_sustain,
    },
];

/// The `ly:piano-pedal-bracket::` callback, by name.
pub(crate) const BRACKET_CALLBACKS: [(&str, Callback); 1] = [("print", bracket_print)];

/// The `ly:sustain-pedal::` callback, by name.
pub(crate) const SUSTAIN_CALLBACKS: [(&str, Callback); 1] = [("print", sustain_print)];

fn host_bracket(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(
        vm,
        &BRACKET_CALLBACKS,
        args,
        "no such piano-pedal-bracket callback",
    )
}

fn host_sustain(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(
        vm,
        &SUSTAIN_CALLBACKS,
        args,
        "no such sustain-pedal callback",
    )
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_bracket_print(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "piano-pedal-bracket",
        "ly:piano-pedal-bracket::",
        "print",
        args,
    )
}

pub(crate) fn ly_sustain_print(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(cx, "sustain-pedal", "ly:sustain-pedal::", "print", args)
}
