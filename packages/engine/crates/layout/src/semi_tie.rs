//! `semi-tie.cc`: what the formatting problem reads of a semi-tie (MS4
//! step 9), `get_column_rank` (`:72–76`), `get_position` (`:78–83`) and
//! `head` (`:85–89`); and `ly:semi-tie::calc-control-points` (`:51–70`,
//! step 10), a primitive over `%host-call` (D127).

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::Y;
use crate::grob;
use crate::item;
use crate::paper_column;
use crate::staff_symbol_referencer as ssr;
use crate::tie_configuration::property_data;

/// `get_column_rank`: the rank of the semi-tie's column, 0 for none.
pub fn get_column_rank(host: &mut dyn Host, me: Value) -> u64 {
    let heap = host.heap();
    item::get_column(heap, me).map_or(0, |c| paper_column::rank(heap, c))
}

/// `get_position`: the rounded staff position of its head; 0 for none
/// (where LilyPond reads through a null).
pub fn get_position(host: &mut dyn Host, me: Value) -> Result<i32, Exception> {
    match head(host, me)? {
        Some(h) => Ok(libm::rint(ssr::get_position(host, h, false)?) as i32),
        None => Ok(0),
    }
}

/// `head`: its `note-head`, if a grob.
pub fn head(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern("note-head");
    let v = grob::object(host, me, sym)?;
    Ok(grob_of(host.heap(), v).map(|_| v))
}

/// `calc_control_points`: the control points its column's positioning
/// gave it; without a semi-tie column, LilyPond's programming error and
/// the semi-tie killed.
pub fn calc_control_points(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    grob::property(host, me, "direction")?;
    let yparent = grob::get_parent(host.heap(), me, Y);
    if let Some(yparent) = yparent
        && grob::has_interface(host.heap(), yparent, "semi-tie-column-interface")
    {
        /* trigger positioning. */
        grob::property(host, yparent, "positioning-done")?;
        return Ok(property_data(host, me, "control-points"));
    }

    grob::unlocated_programming_error(host, "lv tie without Semi_tie_column.  Killing lv tie.")?;
    grob::suicide(host.heap(), me);
    Ok(Value::NULL)
}

/// What `%host-call` runs for `ly:semi-tie::calc-control-points` (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "semi-tie",
    f: host_semi_tie,
}];

/// The `ly:semi-tie::` callbacks, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 1] = [("calc-control-points", calc_control_points)];

fn host_semi_tie(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such semi-tie callback")
}
