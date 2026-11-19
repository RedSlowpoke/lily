//! `volta-bracket.cc` (part 4's, MS3): `add_bar`
//! (`volta-bracket.cc:144–151`, 8 lines), which `Volta_engraver` calls,
//! ported first (MS3 step 20); the bracket's print,
//! `ly:volta-bracket-interface::print` (`:45–112`), with
//! `modify_edge_height` (`:114–142`), the music crate's placeholder until
//! MS3 step 49 (D67, OPEN A17). Rust under a primitive that prints as
//! LilyPond's subr and calls it over `%host-call` (D127, [`HOSTS`]), in
//! 05 §5.1's callback table (`callbacks.rs`) too.
//!
//! The bracket is `bracket.rs`' [`make_bracket`]; the number its `text`
//! interpreted on it (`%grob-interpret-markup`, `text.rs`), the column's
//! break-align width `paper_column.rs`' `%break-align-width`, the length
//! `spanner.rs`' `%spanner-length`. The print checks its spanner under
//! its full name, where LilyPond's dereferences what is none; a bracket
//! without a left bound, or its bound without a column, throws
//! `misc-error`.

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::bracket::{drul_property, make_bracket, real_drul};
use crate::callbacks::Callback;
use crate::direction::get_grob_direction;
use crate::geometry::{Direction, Interval, X, Y};
use crate::stencil::Stencil;
use crate::{grob, grob_array, item, spanner};

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const UP: Direction = 1;

/// `add_bar`: the bar line `b` added to `me`'s `bars-left` at `LEFT`, else
/// to its `bars-right`, and taken as a bound (`add_bound_item`).
pub fn add_bar(host: &mut dyn Host, me: Value, b: Value, d: Direction) -> Result<(), Exception> {
    let bars = if d == -1 { "bars-left" } else { "bars-right" };
    let bars = host.heap().intern(bars);
    grob_array::add_grob(host, me, bars, b)?;
    spanner::add_bound_item(host, me, b)
}

/*
this is too complicated. Yet another version of side-positioning,
badly implemented.

--

* Should look for system_start_delim to find left edge of staff.
*/

/// Whether `me` is the first and the last piece of its original, if it
/// is a broken piece.
fn pieces(heap: &mut lily_engine_scheme::Heap, me: Value) -> Option<(bool, bool)> {
    let orig_span = grob::original(heap, me)?;
    let pieces = spanner::broken_intos(heap, orig_span);
    Some((pieces.first() == Some(&me), pieces.last() == Some(&me)))
}

/// `print`: the bracket over the spanner's length, its edges `edge-height`
/// (2, 2) high pointing away from its `direction`, after the prefatory
/// matter where it begins a line; on an unbroken or first piece, its
/// `text` at its left, aligned to its top and moved by
/// `volta-number-offset` (1, -0.5). `'()` where `modify_edge_height`
/// killed it.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let pieces = pieces(host.heap(), me);
    let broken_first_bracket = pieces.is_some_and(|(first, _)| first);

    let bound = spanner::get_bound(host, me, LEFT)?.ok_or_else(|| {
        crate::misc_error(
            host.heap(),
            "ly:volta-bracket-interface::print",
            "volta bracket without a bound",
        )
    })?;

    /*
      If the volta bracket appears after a line-break, make
      it start after the prefatory matter.
    */
    let mut left = 0.0;
    if item::break_status_dir(host.heap(), bound) == RIGHT {
        let pc = item::get_column(host.heap(), bound).ok_or_else(|| {
            crate::misc_error(
                host.heap(),
                "ly:volta-bracket-interface::print",
                "volta bracket bound in no column",
            )
        })?;
        let sym = host.heap().intern("break-alignment");
        let width = crate::call_lily(host, "%break-align-width", &[pc, sym])?;
        let width = grob::number_pair(host.heap(), width).unwrap_or(Interval::EMPTY);
        // For some reason, break_align_width is relative to
        // the x-parent of the column.
        let parent = grob::get_parent(host.heap(), pc, X).unwrap_or(Value::NULL);
        left = width.at(RIGHT) - grob::relative_coordinate(host, bound, parent, X)?;
    } else {
        /*
          the volta spanner is attached to the bar-line, which is moved
          to the right. We don't need to compensate for the left edge.
        */
    }

    modify_edge_height(host, me)?;
    if !grob::is_live_grob(host.heap(), me) {
        return Ok(Value::NULL);
    }

    let mut edge_height = drul_property(host, me, "edge-height", [2.0, 2.0])?;
    let flare = drul_property(host, me, "bracket-flare", [0.0, 0.0])?;
    let shorten = drul_property(host, me, "shorten-pair", [0.0, 0.0])?;

    let dir = get_grob_direction(host, me)?;
    for h in &mut edge_height {
        *h *= -f64::from(dir);
    }

    let length = crate::call_lily(host, "%spanner-length", &[me])?;
    let length = host.heap().to_f64(length).unwrap_or(0.0);
    let start = (length - left, 0.0);

    let mut total = make_bracket(
        host,
        me,
        Y,
        start,
        edge_height,
        Interval::EMPTY,
        flare,
        shorten,
    )?;
    host.heap().root(total.expr);

    if pieces.is_none() || broken_first_bracket {
        let text = grob::property(host, me, "text")?;
        let offset = grob::property(host, me, "volta-number-offset")?;
        let offset = real_drul(host.heap(), offset, [1.0, -0.5]);

        let num = crate::call_lily(host, "%grob-interpret-markup", &[me, text])?;
        host.heap().root(num);
        let heap = host.heap();
        let mut num = heap.foreign::<Stencil>(num).copied().unwrap_or_default();
        num.align_to(heap, Y, f64::from(UP));
        num.translate_axis(heap, offset[1], Y);
        let padding = -num.extent(X).length() - offset[0];
        total.add_at_edge(heap, X, LEFT, &num, padding);
    }

    let heap = host.heap();
    total.translate_axis(heap, left, X);
    Ok(heap.wrap(total))
}

/// `modify_edge_height`: a piece of a broken bracket has no edge where
/// the bracket goes on into the piece before or after it (`edge-height`
/// set, 0 at that side).
pub fn modify_edge_height(host: &mut dyn Host, me: Value) -> Result<(), Exception> {
    let pieces = pieces(host.heap(), me);

    let broken_first_bracket = pieces.is_some_and(|(first, _)| first);
    let broken_last_bracket = pieces.is_some_and(|(_, last)| last);
    let no_vertical_start = pieces.is_some() && !broken_first_bracket;
    let no_vertical_end = pieces.is_some() && !broken_last_bracket;

    if no_vertical_end || no_vertical_start {
        let mut edge_height = drul_property(host, me, "edge-height", [2.0, 2.0])?;
        if no_vertical_start {
            edge_height[0] = 0.0;
        }

        if no_vertical_end {
            edge_height[1] = 0.0;
        }

        let value = crate::offset(host.heap(), (edge_height[0], edge_height[1]));
        grob::set_property(host.heap(), me, "edge-height", value);
    }

    if broken_last_bracket && no_vertical_end && no_vertical_start && !broken_first_bracket {
        grob::suicide(host.heap(), me);
    }
    Ok(())
}

// --- the callback -----------------------------------------------------------

/// What `%host-call` runs for the `ly:volta-bracket-interface::` primitive
/// (D127): the callback its second argument names, of the grob given,
/// with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "volta-bracket-interface",
    f: host_volta_bracket,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 1] = [("print", print)];

fn host_volta_bracket(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such volta bracket callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:volta-bracket-interface::print spanner)`: LilyPond's subr, a
/// primitive here so that it prints as LilyPond's. It checks its spanner
/// and tail-calls `%host-call` with its name ([`HOSTS`]).
pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    spanner::spanner_arg(cx, "ly:volta-bracket-interface::print", 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("volta-bracket-interface");
    let name = cx.heap_mut().intern("print");
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}
