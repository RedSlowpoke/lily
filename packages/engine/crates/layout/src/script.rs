//! `script-interface.cc` (part 4's, MS3 step 53, 117 lines): a script's
//! two callbacks, `ly:script-interface::calc-positioning-done` (`:33–45`),
//! which moves a script from its note column onto the stem's first head,
//! and `::calc-cross-staff` (`:47–65`), whether its stem or the slur it
//! avoids crosses staves, the music crate's placeholders until then (D67);
//! and `script_priority_less` (`:67–74`), the script column's order. Rust
//! under primitives that print as LilyPond's subrs and call it over
//! `%host-call` (D127, [`HOSTS`]), in 05 §5.1's callback table
//! (`callbacks.rs`) too.

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::X;
use crate::{grob, note_column, stem};

/// `calc_positioning_done`: a script whose X parent (its note column, or a
/// head) has a stem with a first head is put on that head.
pub fn calc_positioning_done(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    if let Some(par) = grob::get_parent(host.heap(), me, X)
        && let Some(stem) = note_column::get_stem(host, par)?
        && let Some(head) = stem::first_head(host, stem)?
    {
        grob::set_parent(host.heap(), me, head, X);
    }
    Ok(Value::TRUE)
}

/// `calc_cross_staff`: whether the stem of the script's X parent crosses
/// staves, or its `slur` does and it avoids the slur `outside` or
/// `around`. A script of no X parent has no stem, where LilyPond reads
/// through a null.
pub fn calc_cross_staff(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let stem = match grob::get_parent(host.heap(), me, X) {
        Some(par) => note_column::get_stem(host, par)?,
        None => None,
    };

    if let Some(stem) = stem
        && grob::property(host, stem, "cross-staff")? != Value::FALSE
    {
        return Ok(Value::TRUE);
    }

    let sym = host.heap().intern("slur");
    let slur = grob::object(host, me, sym)?;
    let avoid_slur = grob::property(host, me, "avoid-slur")?;
    if lily_engine_music::grob_of(host.heap(), slur).is_some()
        && grob::property(host, slur, "cross-staff")? != Value::FALSE
    {
        let heap = host.heap();
        if avoid_slur == heap.intern("outside") || avoid_slur == heap.intern("around") {
            return Ok(Value::TRUE);
        }
    }

    Ok(Value::FALSE)
}

/// `script_priority_less`: whether `g1`'s `script-priority` is less than
/// `g2`'s, each `from_scm<int>` (Guile's error for what is no exact
/// integer of an `int`'s range).
pub fn script_priority_less(host: &mut dyn Host, g1: Value, g2: Value) -> Result<bool, Exception> {
    let p1 = grob::property(host, g1, "script-priority")?;
    let p2 = grob::property(host, g2, "script-priority")?;
    let p1 = stem::to_int(host.heap(), p1)?;
    let p2 = stem::to_int(host.heap(), p2)?;
    Ok(p1 < p2)
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:script-interface::` primitives
/// (D127): the callback its second argument names, of the grob given, with
/// the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "script-interface",
    f: host_script,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 2] = [
    ("calc-positioning-done", calc_positioning_done),
    ("calc-cross-staff", calc_cross_staff),
];

fn host_script(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such script callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// A `ly:script-interface::` subr: its grob checked under the method's
/// name, as LilyPond's `LY_ASSERT_SMOB` gives it, and `%host-call`
/// tail-called with the callback's name ([`HOSTS`]).
fn host_call(cx: &mut Cx, method: &str, name: &str, args: &[Value]) -> R {
    grob::grob_arg(cx, method, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("script-interface");
    let name = cx.heap_mut().intern(name);
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}

/// `(ly:script-interface::calc-positioning-done grob)`.
pub(crate) fn ly_calc_positioning_done(cx: &mut Cx, args: &[Value]) -> R {
    host_call(
        cx,
        "ly:calc-positioning-done",
        "calc-positioning-done",
        args,
    )
}

/// `(ly:script-interface::calc-cross-staff grob)`.
pub(crate) fn ly_calc_cross_staff(cx: &mut Cx, args: &[Value]) -> R {
    host_call(cx, "ly:calc-cross-staff", "calc-cross-staff", args)
}
