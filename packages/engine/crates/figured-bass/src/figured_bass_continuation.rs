//! `figured-bass-continuation.cc` (MS4 step 16):
//! `ly:figured-bass-continuation::center-on-figures` (`:37–51`), a
//! `BassFigureContinuation`'s `Y-offset`, the music crate's placeholder
//! until then (D67): the middle of its `figures`' height less its own
//! coordinate there, 0 with no figures or none of any height.

use lily_engine_layout::grob_array::extract_grob_array;
use lily_engine_layout::{Callback, Y, axis_group, dispatch, grob, host_call_of};
use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

/// `center_on_figures`.
pub fn center_on_figures(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let y = center_on_figures_rooted(host, me);
    host.heap().unroot_to(depth);
    Ok(host.heap().flonum(y?))
}

fn center_on_figures_rooted(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    let figures = extract_grob_array(host, me, "figures")?;
    if figures.is_empty() {
        return Ok(0.0);
    }
    let common = grob::common_refpoint_of_array(host.heap(), &figures, Some(me), Y).unwrap_or(me);

    let ext = axis_group::relative_group_extent(host, &figures, common, Y)?;
    if ext.is_empty() {
        return Ok(0.0);
    }
    Ok(ext.center() - grob::relative_coordinate(host, me, common, Y)?)
}

// --- the callback -------------------------------------------------------------

/// What `%host-call` runs for the `ly:figured-bass-continuation::`
/// primitive (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "figured-bass-continuation",
    f: host_figured_bass_continuation,
}];

/// The `ly:figured-bass-continuation::` callbacks, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 1] = [("center-on-figures", center_on_figures)];

fn host_figured_bass_continuation(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    dispatch(
        vm,
        &CALLBACKS,
        args,
        "no such figured-bass-continuation callback",
    )
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_center_on_figures(cx: &mut Cx, args: &[Value]) -> R {
    host_call_of(
        cx,
        "figured-bass-continuation",
        "ly:figured-bass-continuation::",
        "center-on-figures",
        args,
    )
}
