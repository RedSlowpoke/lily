//! `lyric-extender.cc` (MS4 step 15): `ly:lyric-extender::print`
//! (`:30–135`), the music crate's placeholder until then (D67), Rust under
//! a primitive that prints as LilyPond's subr and calls it over
//! `%host-call` (D127, [`HOSTS`]), in 05 §5.1's callback table
//! (`callbacks.rs`).
//!
//! The line runs from the syllable's right (or its first head's left)
//! past its last head, at least `minimum-length` long but not past the
//! system's end, up to the next syllable (`next`) less `right-padding`, or
//! to the end of the line where it is broken; shorter than one and a half
//! times its thickness it is not drawn, and an automatic one of an unbroken
//! spanner (`unbroken-spanner?`, the image's) that ends before its last
//! head begins is killed. clang fuses none of its sums (otool).

use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::{Box, Direction, Interval, X};
use crate::grob;
use crate::grob_array::extract_grob_array;
use crate::lookup;
use crate::spacing_options::double_or;
use crate::spanner;
use crate::stencil::Stencil;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// `from_scm<double> (get_property (me, name), fallback)`.
fn real_or(host: &mut dyn Host, me: Value, name: &str, fallback: f64) -> Result<f64, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(double_or(host, v, fallback))
}

/// `from_scm<bool>`: `#t` only.
fn is_true(host: &mut dyn Host, me: Value, name: &str) -> Result<bool, Exception> {
    Ok(grob::property(host, me, name)? == Value::TRUE)
}

/// `common_refpoint` of a grob that may be none, as LilyPond's of a null
/// is none.
fn common(host: &mut dyn Host, a: Option<Value>, b: Option<Value>) -> Option<Value> {
    grob::common_refpoint(host.heap(), a?, b?, X)
}

/// `print`: the extender's line, or `'()`.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let left_edge = spanner::get_bound(host, me, LEFT)?;
    let next = host.heap().intern("next");
    let next = grob::object(host, me, next)?;
    let right_text = grob_of(host.heap(), next)
        .is_some()
        .then_some(next)
        .filter(|&g| crate::item::is_item(host.heap(), g));

    let mut common_x = left_edge;
    if right_text.is_some() {
        common_x = common(host, common_x, right_text);
    }
    let right_bound = spanner::get_bound(host, me, RIGHT)?;
    common_x = common(host, common_x, right_bound);
    let system = spanner::get_system(host.heap(), me);
    common_x = common(host, common_x, system);

    let sl = grob::layout_dimension(host, me, "line-thickness")?;

    let heads = extract_grob_array(host, me, "heads")?;

    if heads.is_empty() {
        return Ok(Value::NULL);
    }

    let common_x = grob::common_refpoint_of_array(host.heap(), &heads, common_x, X);
    let (Some(common_x), Some(left_edge)) = (common_x, left_edge) else {
        return Ok(Value::NULL);
    };

    let mut left_point = if grob::has_interface(host.heap(), left_edge, "lyric-syllable-interface")
    {
        grob::extent(host, left_edge, common_x, X)?.right
    } else {
        grob::extent(host, heads[0], common_x, X)?.left
    };

    if left_point.is_infinite() {
        return Ok(Value::NULL);
    }

    /* It seems that short extenders are even lengthened to go past the
    note head, but haven't found a pattern in it yet. --hwn 1/1/04  */
    let minlen = real_or(host, me, "minimum-length", 0.0)?;
    let mut right_point = left_point + minlen;
    if let Some(rb) = system
        .map(|s| spanner::get_bound(host, s, RIGHT))
        .transpose()?
    {
        let limit = match rb {
            Some(rb) => grob::relative_coordinate(host, rb, common_x, X)?,
            None => 0.0,
        };
        right_point = right_point.min(limit);
    }

    let last = heads[heads.len() - 1];
    let limit = grob::extent(host, last, common_x, X)?.right;
    right_point = right_point.max(limit);

    let h = sl * real_or(host, me, "thickness", 0.0)?;
    let paddings = [
        real_or(host, me, "left-padding", h)?,
        real_or(host, me, "right-padding", h)?,
    ];

    if let Some(right_text) = right_text {
        let limit = grob::robust_relative_extent(host, right_text, common_x, X)?.left - paddings[1];
        right_point = right_point.min(limit);
    }

    /* run to end of line. */
    if let Some(rb) = right_bound
        && crate::item::break_status_dir(host.heap(), rb) != 0
    {
        let limit = grob::robust_relative_extent(host, rb, common_x, X)?.left - paddings[1];
        right_point = right_point.max(limit);
    }

    left_point += paddings[0];
    let w = right_point - left_point;

    if w < 1.5 * h {
        return Ok(Value::NULL);
    }

    if crate::call_lily(host, "unbroken-spanner?", &[me])? == Value::TRUE
        && is_true(host, me, "auto-generated")?
        && is_true(host, me, "remove-short-autoextender")?
    {
        let last_nodehead_x = grob::extent(host, last, common_x, X)?.left;
        if last_nodehead_x < left_point {
            grob::suicide(host.heap(), me);
            return Ok(Value::NULL);
        }
    }

    let mut warnings = Vec::new();
    let b = Box::new(Interval::new(0.0, w), Interval::new(0.0, h));
    let mut mol: Stencil = lookup::round_filled_box(host.heap(), b, 0.8 * h, &mut warnings);
    host.heap().root(mol.expr);
    for text in &warnings {
        grob::unlocated_warning(host, text)?;
    }
    let x = grob::relative_coordinate(host, me, common_x, X)?;
    mol.translate_axis(host.heap(), left_point - x, X);
    Ok(host.heap().wrap(mol))
}

/// What `%host-call` runs for `ly:lyric-extender::print` (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "lyric-extender",
    f: host_lyric_extender,
}];

/// The `ly:lyric-extender::` callbacks, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 1] = [("print", print)];

fn host_lyric_extender(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such lyric-extender callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(cx, "lyric-extender", "ly:lyric-extender::", "print", args)
}
