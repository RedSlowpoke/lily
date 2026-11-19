//! `tie-column.cc`: `add_tie` (`tie-column.cc:37–53`), which
//! `Tie_engraver` calls, ported first (MS3 step 17); and the tie column's
//! callbacks (MS4 steps 9–10): `before_line_breaking` (`:55–75`) and
//! `calc_positioning_done` (`:77–110`), the ties sorted by position and
//! formatted together (`tie_formatting_problem.rs`), each given its
//! control points and direction.

use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::direction::set_grob_direction;
use crate::geometry::Y;
use crate::grob;
use crate::grob_array;
use crate::item;
use crate::paper_column;
use crate::spanner;
use crate::tie;
use crate::tie_formatting_problem::Problem;

/// The rank of the column of `me`'s bound at `d`, if both are there.
fn bound_rank(host: &mut dyn Host, me: Value, d: i32) -> Result<Option<u64>, Exception> {
    let Some(b) = spanner::get_bound(host, me, d)? else {
        return Ok(None);
    };
    let heap = host.heap();
    Ok(item::get_column(heap, b).map(|c| paper_column::rank(heap, c)))
}

/// `add_tie`: the tie put in the column, unless it is in one: the column's
/// bounds the tie's heads where it has no left bound or one of a later
/// column than the tie's, the tie its child on Y and one of its `ties`.
/// A bound without a column, on which LilyPond would crash, is taken as
/// one of no later column.
pub fn add_tie(host: &mut dyn Host, me: Value, tie: Value) -> Result<(), Exception> {
    if let Some(parent) = grob::get_parent(host.heap(), tie, Y)
        && grob::has_interface(host.heap(), parent, "tie-column-interface")
    {
        return Ok(());
    }

    let later = match spanner::get_bound(host, me, -1)? {
        None => true,
        Some(_) => matches!(
            (bound_rank(host, me, -1)?, bound_rank(host, tie, -1)?),
            (Some(mine), Some(its)) if mine > its
        ),
    };
    if later {
        if let Some(h) = tie::head(host, tie, -1)? {
            spanner::set_bound(host, me, -1, h)?;
        }
        if let Some(h) = tie::head(host, tie, 1)? {
            spanner::set_bound(host, me, 1, h)?;
        }
    }

    grob::set_parent(host.heap(), tie, me, Y);
    let sym = host.heap().intern("ties");
    grob_array::add_grob(host, me, sym, tie)
}

/// `before_line_breaking`: the column's bounds extended over its ties'.
/// It reads the property `ties`, which is an object, so `'()`: nothing
/// is extended, as LilyPond's.
pub fn before_line_breaking(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let mut s = grob::property(host, me, "ties")?;
    while let (Some(t), Some(rest)) = (host.heap().car(s), host.heap().cdr(s)) {
        for dir in [-1, 1] {
            let theirs = bound_rank(host, t, dir)?.unwrap_or(0) as i64;
            let mine = bound_rank(host, me, dir)?.unwrap_or(0) as i64;
            if i64::from(dir) * theirs > i64::from(dir) * mine
                && let Some(h) = tie::head(host, t, dir)?
            {
                spanner::set_bound(host, me, dir, h)?;
            }
        }
        s = rest;
    }

    Ok(Value::UNSPECIFIED)
}

/// `calc_positioning_done`: the column's ties sorted by position (libc++'s
/// `std::sort`) and formatted together, with `tie-configuration`; each
/// given its control points and direction.
pub fn calc_positioning_done(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let ties = grob_array::extract_spanner_array(host, me, "ties")?;
    if ties.is_empty() {
        return Ok(Value::TRUE);
    }

    grob::set_property(host.heap(), me, "positioning-done", Value::TRUE);
    let ties = sorted_by_position(host, &ties, tie::get_position)?;

    let mut problem = Problem::new();
    problem.read_ties(host, &ties)?;

    let manual_configs = grob::property(host, me, "tie-configuration")?;
    problem.set_manual_tie_configuration(host, manual_configs)?;

    let base = problem.generate_optimal_configuration(host)?;
    for (i, conf) in base.ties.iter().enumerate() {
        let cp = tie::get_control_points(
            host,
            ties[i],
            problem.common_x_refpoint(),
            conf,
            &problem.details,
        )?;

        grob::set_property(host.heap(), ties[i], "control-points", cp);
        set_grob_direction(host, ties[i], conf.dir);

        problem.set_debug_scoring(host, &base)?;
    }
    Ok(Value::TRUE)
}

/// `ties` sorted by `position` (`Tie::less`, `Semi_tie::less`) as libc++'s
/// `std::sort` sorts them, each position read once.
pub(crate) fn sorted_by_position(
    host: &mut dyn Host,
    ties: &[Value],
    position: fn(&mut dyn Host, Value) -> Result<i32, Exception>,
) -> Result<Vec<Value>, Exception> {
    let mut keyed = Vec::with_capacity(ties.len());
    for &t in ties {
        keyed.push((position(host, t)?, t));
    }
    lily_engine_skyline::sort(&mut keyed, &|a: &(i32, Value), b: &(i32, Value)| a.0 < b.0);
    Ok(keyed.into_iter().map(|(_, t)| t).collect())
}

/// What `%host-call` runs for the tie column's callbacks (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "tie-column",
    f: host_tie_column,
}];

/// The `ly:tie-column::` callbacks, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 2] = [
    ("before-line-breaking", before_line_breaking),
    ("calc-positioning-done", calc_positioning_done),
];

fn host_tie_column(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such tie column callback")
}
