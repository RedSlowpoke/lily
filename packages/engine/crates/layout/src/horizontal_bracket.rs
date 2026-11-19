//! `horizontal-bracket.cc` (part 4's, MS3 step 49, 71 lines): the
//! analysis bracket's print, `ly:horizontal-bracket::print` (`:30–55`),
//! the music crate's placeholder until then (D67): `bracket.rs`'
//! [`make_enclosing_bracket`] over its `columns`. Rust under a primitive
//! that prints as LilyPond's subr and calls it over `%host-call` (D127,
//! [`HOSTS`]), in 05 §5.1's callback table (`callbacks.rs`) too. The
//! print checks its spanner under its full name, where LilyPond's
//! dereferences what is none.

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::bracket::make_enclosing_bracket;
use crate::callbacks::Callback;
use crate::direction::get_grob_direction;
use crate::geometry::X;
use crate::{grob, grob_array, item, spanner};

/// `print`: the bracket enclosing the spanner's `columns`, and its bounds
/// where it is broken; a bracket with no columns killed.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let gs = grob_array::extract_grob_array(host, me, "columns")?;

    let mut enclosed = gs;
    if enclosed.is_empty() {
        grob::suicide(host.heap(), me);
        return Ok(Value::NULL);
    }

    for d in [-1, 1] {
        if let Some(b) = spanner::get_bound(host, me, d)?
            && item::break_status_dir(host.heap(), b) != 0
        {
            enclosed.push(b);
        }
    }

    let dir = get_grob_direction(host, me)?;
    let b = make_enclosing_bracket(host, me, me, &enclosed, X, dir)?;
    Ok(host.heap().wrap(b))
}

// --- the callback -----------------------------------------------------------

/// What `%host-call` runs for the `ly:horizontal-bracket::` primitive
/// (D127): the callback its second argument names, of the grob given,
/// with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "horizontal-bracket",
    f: host_horizontal_bracket,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 1] = [("print", print)];

fn host_horizontal_bracket(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such horizontal bracket callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:horizontal-bracket::print spanner)`: LilyPond's subr, a primitive
/// here so that it prints as LilyPond's. It checks its spanner and
/// tail-calls `%host-call` with its name ([`HOSTS`]).
pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    spanner::spanner_arg(cx, "ly:horizontal-bracket::print", 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("horizontal-bracket");
    let name = cx.heap_mut().intern("print");
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}
