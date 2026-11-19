//! `semi-tie-column.cc` (MS4 step 10): the laissez vibrer tie column's
//! callbacks, primitives over `%host-call` (D127):
//! `ly:semi-tie-column::calc-positioning-done` (`:45–79`), its ties
//! sorted by position and formatted together (`tie_formatting_problem.rs`)
//! at their heads' side, each given its control points and direction; and
//! `::calc-head-direction` (`:81–101`), its ties' common `head-direction`.

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::direction::set_grob_direction;
use crate::grob;
use crate::grob_array;
use crate::semi_tie;
use crate::tie;
use crate::tie_column::sorted_by_position;
use crate::tie_formatting_problem::Problem;

/// `calc_positioning_done`.
pub fn calc_positioning_done(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    grob::set_property(host.heap(), me, "positioning-done", Value::TRUE);

    let lv_ties = grob_array::extract_item_array(host, me, "ties")?;
    let lv_ties = sorted_by_position(host, &lv_ties, semi_tie::get_position)?;

    let mut problem = Problem::new();

    let hd = grob::property(host, me, "head-direction")?;
    let head_dir = crate::dynamic_align_engraver::direction(hd);
    problem.read_semi_ties(host, &lv_ties, head_dir)?;

    let manual_configs = grob::property(host, me, "tie-configuration")?;
    problem.set_manual_tie_configuration(host, manual_configs)?;

    let base = problem.generate_optimal_configuration(host)?;
    for (i, &t) in lv_ties.iter().enumerate() {
        let conf = &base.ties[i];
        let cp =
            tie::get_control_points(host, t, problem.common_x_refpoint(), conf, &problem.details)?;

        grob::set_property(host.heap(), t, "control-points", cp);
        set_grob_direction(host, t, conf.dir);

        problem.set_debug_scoring(host, &base)?;
    }

    Ok(Value::TRUE)
}

/// `calc_head_direction`: the ties' `head-direction`, left for none;
/// LilyPond's programming error where two differ, the one before.
pub fn calc_head_direction(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let ties = grob_array::extract_grob_array(host, me, "ties")?;
    let mut d = -1;
    for (i, &t) in ties.iter().enumerate() {
        let v = grob::property(host, t, "head-direction")?;
        let this_d = crate::dynamic_align_engraver::direction(v);
        if i > 0 && d != this_d {
            grob::unlocated_programming_error(
                host,
                "all semi-ties in a semi-tie-column should have the same head-direction",
            )?;
            return Ok(host.heap().integer(i64::from(d)));
        }
        d = this_d;
    }
    Ok(host.heap().integer(i64::from(d)))
}

/// What `%host-call` runs for the semi-tie column's callbacks (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "semi-tie-column",
    f: host_semi_tie_column,
}];

/// The `ly:semi-tie-column::` callbacks, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 2] = [
    ("calc-head-direction", calc_head_direction),
    ("calc-positioning-done", calc_positioning_done),
];

fn host_semi_tie_column(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such semi-tie column callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// A `ly:semi-tie-column::` subr: its grob checked, `%host-call`
/// tail-called with the callback's name.
fn host_call(cx: &mut Cx, name: &str, args: &[Value]) -> R {
    crate::tie::host_call_of(cx, "semi-tie-column", "ly:semi-tie-column::", name, args)
}

pub(crate) fn ly_calc_positioning_done(cx: &mut Cx, args: &[Value]) -> R {
    host_call(cx, "calc-positioning-done", args)
}

pub(crate) fn ly_calc_head_direction(cx: &mut Cx, args: &[Value]) -> R {
    host_call(cx, "calc-head-direction", args)
}
