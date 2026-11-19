//! `vowel-transition.cc` (MS8 step 8, part 1, 130 lines):
//! `ly:vowel-transition::set-spacing-rods`, the music crate's placeholder
//! until then (D67), Rust under a primitive that prints as LilyPond's subr
//! and calls it over `%host-call` (D127, [`HOSTS`]). `Hyphen_engraver`
//! makes the `VowelTransition` (`hyphen_engraver.rs`), and
//! `ly:line-spanner::print` draws it.
//!
//! With a `minimum-length` or a `minimum-length-after-break` and both
//! bounds: where the root system has breakable columns between the bounds'
//! columns (`System::broken_col_range`), a rod from the left bound to the
//! first one's piece at the end of a line, and one from the last one's
//! piece at the start of a line to the right bound when the piece after
//! the break spans time or `after-line-breaking` is `#t` as data; then a
//! rod between the bounds and the same rod again to the right bound's
//! piece at the end of a line. Each is the length, the `padding` of
//! `bound-details`' sides (`left`, `right`, `left-broken`,
//! `right-broken`) and the bounds' protrusion.

use lily_engine_music::{Host, Moment, Rational};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::Direction;
use crate::grob;
use crate::item;
use crate::paper_column;
use crate::rod::Rod;
use crate::spacing_options::double_or;
use crate::spanner;
use crate::system;
use crate::tie_configuration::property_data;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// `padding` of `bound-details`' entry `side`, 0 without one.
fn padding_of(host: &mut dyn Host, bounds: Value, side: &str) -> f64 {
    let heap = host.heap();
    let key = heap.intern(side);
    let details = grob::assq_ref(heap, key, bounds).unwrap_or(Value::FALSE);
    if details == Value::FALSE {
        return 0.0;
    }
    let heap = host.heap();
    let key = heap.intern("padding");
    let padding = grob::assq_ref(heap, key, details).unwrap_or(Value::FALSE);
    double_or(host, padding, 0.0)
}

/// `spanned_time_interval` of two items with columns, its left end's
/// grace part 0: whether it has a length.
fn spans_time(host: &mut dyn Host, left: Value, right: Value) -> Result<bool, Exception> {
    let mut iv: [Moment; 2] = [Moment::default(); 2];
    for (i, it) in [left, right].into_iter().enumerate() {
        let column = item::get_column(host.heap(), it).unwrap_or(it);
        iv[i] = paper_column::when_mom(host, column)?;
    }
    iv[0].grace = Rational::ZERO;
    Ok(!iv[1].compare(iv[0]).is_eq())
}

/// `set_spacing_rods`, as above.
pub fn set_spacing_rods(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let minimum_length = grob::property(host, me, "minimum-length")?;
    let broken_length = grob::property(host, me, "minimum-length-after-break")?;
    let heap = host.heap();
    if !heap.is_number(minimum_length) && !heap.is_number(broken_length) {
        return Ok(Value::UNSPECIFIED);
    }
    let root = system::get_root_system(heap, me);
    let left = spanner::get_bound(host, me, LEFT)?;
    let right = spanner::get_bound(host, me, RIGHT)?;
    let (Some(left), Some(right)) = (left, right) else {
        return Ok(Value::UNSPECIFIED);
    };
    let left_column = item::get_column(host.heap(), left).unwrap_or(left);
    let right_column = item::get_column(host.heap(), right).unwrap_or(right);
    let cols = match root {
        Some(root) => system::broken_col_range(host, root, left_column, right_column)?,
        None => alloc::vec::Vec::new(),
    };

    let bound_details = grob::property(host, me, "bound-details")?;
    let padding = [
        padding_of(host, bound_details, "left"),
        padding_of(host, bound_details, "right"),
    ];
    let padding_broken = [
        padding_of(host, bound_details, "left-broken"),
        padding_of(host, bound_details, "right-broken"),
    ];
    let minimum = double_or(host, minimum_length, 0.0);

    if let (Some(&first), Some(&last)) = (cols.first(), cols.last()) {
        // Before line break
        let mut rod_before_break = Rod {
            item_drul: [
                Some(left),
                item::find_prebroken_piece(host.heap(), first, LEFT),
            ],
            distance: minimum,
        };
        rod_before_break.distance += padding[0];
        rod_before_break.distance += padding_broken[1];
        rod_before_break.distance += rod_before_break.bounds_protrusion(host)?;
        rod_before_break.add_to_cols(host)?;

        // After line break
        let after = item::find_prebroken_piece(host.heap(), last, RIGHT);
        let mut rod_after_break = Rod {
            item_drul: [after, Some(right)],
            distance: 0.0,
        };
        // Calculate and add space only if the vowel transition is to be
        // drawn. I.e., either it does not end on the first note after
        // breaking, or property after-line-breaking is set to #t.
        let spans = match after {
            Some(after) => spans_time(host, after, right)?,
            None => spans_time(host, right, right)?,
        };
        if spans || property_data(host, me, "after-line-breaking") == Value::TRUE {
            rod_after_break.distance = if host.heap().is_number(broken_length) {
                double_or(host, broken_length, 0.0)
            } else {
                minimum
            };
            rod_after_break.distance += padding_broken[0];
            rod_after_break.distance += padding[1];
            rod_after_break.distance += rod_after_break.bounds_protrusion(host)?;
            rod_after_break.add_to_cols(host)?;
        }
    }

    let mut rod = Rod {
        item_drul: [Some(left), Some(right)],
        distance: minimum,
    };
    for p in padding {
        rod.distance += p;
    }
    rod.distance += rod.bounds_protrusion(host)?;
    rod.add_to_cols(host)?;

    if let Some(left_pbp) = item::find_prebroken_piece(host.heap(), right, LEFT) {
        rod.item_drul[1] = Some(left_pbp);
        rod.add_to_cols(host)?;
    }

    Ok(Value::UNSPECIFIED)
}

/// What `%host-call` runs for the `ly:vowel-transition::` callback (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "vowel-transition",
    f: host_vowel_transition,
}];

/// The `ly:vowel-transition::` callback, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 1] = [("set-spacing-rods", set_spacing_rods)];

fn host_vowel_transition(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such vowel-transition callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_set_spacing_rods(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "vowel-transition",
        "ly:vowel-transition::",
        "set-spacing-rods",
        args,
    )
}
