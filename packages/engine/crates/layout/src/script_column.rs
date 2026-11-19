//! `script-column.cc` (part 4's, MS3): `add_side_positioned`
//! (`script-column.cc:35–44`, 10 lines), which `Script_row_engraver` calls,
//! ported first (MS3 step 19); `Non_musical_script_column_engraver` and
//! `Script_column_engraver` call it too. Since MS3 step 53 the rest, the
//! scripts' order and their stacking: `ly:script-column::before-line-breaking`
//! (`:93–118`) and `::row-before-line-breaking` (`:59–91`), the music
//! crate's placeholders until then (D67), over `order_grobs` (`:120–178`).
//! Rust under primitives that print as LilyPond's subrs and call it over
//! `%host-call` (D127, [`HOSTS`]), in 05 §5.1's callback table
//! (`callbacks.rs`) too. `ly:grob-script-priority-less` (`:46–57`), which
//! no Scheme of LilyPond's calls, is not defined: the order sorts by
//! [`script_priority_less`] itself.

use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::direction::get_strict_grob_direction;
use crate::geometry::Y;
use crate::script::script_priority_less;
use crate::spacing_options::double_or;
use crate::{grob, grob_array, side_position};

/// `add_side_positioned`: `script` added to `me`'s `scripts` and given
/// `me` as its `script-column`; nothing for a script whose
/// `script-priority` is no number.
pub fn add_side_positioned(host: &mut dyn Host, me: Value, script: Value) -> Result<(), Exception> {
    let p = grob::property(host, script, "script-priority")?;
    if !host.heap().is_number(p) {
        return Ok(());
    }

    let sym = host.heap().intern("scripts");
    grob_array::add_grob(host, me, sym, script)?;
    let heap = host.heap();
    let sym = heap.intern("script-column");
    grob::set_object(heap, script, sym, me);
    Ok(())
}

/// `get_property_data (g, name)`: what is set for `name` on the grob `g`,
/// no callback called.
fn property_data(host: &mut dyn Host, g: Value, name: &str) -> Value {
    let heap = host.heap();
    let sym = heap.intern(name);
    match grob_of(heap, g) {
        Some((arena, id)) => grob::property_data(heap, &arena, id, g, sym),
        None => Value::NULL,
    }
}

/// `row_before_line_breaking`: the row's scripts but those on the Y axis
/// of their side position, ordered by their Y parent (a head), each
/// group with the accidental placements and arpeggios of the row, which
/// affect every group.
///
/// LilyPond keeps the groups in a `std::unordered_map` by the parent's
/// address and orders them in its order; they are ordered here as their
/// first scripts are in `scripts`. That matters only where the row has
/// accidental placements or arpeggios and two groups.
pub fn row_before_line_breaking(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let scripts = grob_array::extract_grob_array(host, me, "scripts")?;
    let y_aligned_side = crate::lily_procedure(host, "ly:side-position-interface::y-aligned-side")?;

    let mut head_scripts: Vec<(Option<Value>, Vec<Value>)> = Vec::new();
    let mut affect_all_grobs = Vec::new();
    for &sc in &scripts {
        // Don't want to consider scripts horizontally next to notes.
        let heap = host.heap();
        if grob::has_interface(heap, sc, "accidental-placement-interface")
            || grob::has_interface(heap, sc, "arpeggio-interface")
        {
            affect_all_grobs.push(sc);
        } else if property_data(host, sc, "Y-offset") != y_aligned_side {
            let parent = grob::get_parent(host.heap(), sc, Y);
            match head_scripts.iter_mut().find(|(p, _)| *p == parent) {
                Some((_, grobs)) => grobs.push(sc),
                None => head_scripts.push((parent, alloc::vec![sc])),
            }
        }
    }

    for (_, mut grobs) in head_scripts {
        // this isn't right in all cases, but in general a safe assumption.
        grobs.extend_from_slice(&affect_all_grobs);
        order_grobs(host, &grobs)?;
    }

    Ok(Value::UNSPECIFIED)
}

/// `before_line_breaking`: the column's live scripts but those on the X
/// axis of their side position, ordered.
pub fn before_line_breaking(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let scripts = grob_array::extract_grob_array(host, me, "scripts")?;
    let x_aligned_side = crate::lily_procedure(host, "ly:side-position-interface::x-aligned-side")?;

    let mut staff_sided = Vec::new();
    for &sc in &scripts {
        // Don't want to consider scripts horizontally next to notes.
        if grob::is_live_grob(host.heap(), sc)
            && property_data(host, sc, "X-offset") != x_aligned_side
        {
            staff_sided.push(sc);
        }
    }

    order_grobs(host, &staff_sided)?;
    Ok(Value::UNSPECIFIED)
}

/// `order_grobs`: the grobs of each direction (their strict direction)
/// stably sorted by `script-priority`; each takes as supports all before
/// it where the one before it has no `outside-staff-priority`, and else,
/// where it has none of its own or the same as the one before it had at
/// first, the one before it's plus 0.1, so that they stay in order.
fn order_grobs(host: &mut dyn Host, grobs: &[Value]) -> Result<(), Exception> {
    let mut scripts_drul: [Vec<Value>; 2] = [Vec::new(), Vec::new()];
    for &g in grobs {
        let d = get_strict_grob_direction(host, g)?;
        scripts_drul[usize::from(d > 0)].push(g);
    }

    for mut ss in scripts_drul {
        stable_sort(host, &mut ss)?;

        let mut last: Option<Value> = None;
        let mut last_initial_outside_staff = Value::NULL;
        for (i, &g) in ss.iter().enumerate() {
            let initial_outside_staff = grob::property(host, g, "outside-staff-priority")?;
            if let Some(last) = last {
                let last_outside_staff = grob::property(host, last, "outside-staff-priority")?;
                if !host.heap().is_number(last_outside_staff) {
                    // Use all the scripts so far as support for the
                    // current grob.
                    for &t in &ss[..i] {
                        side_position::add_support(host, g, t)?;
                    }
                } else if !host.heap().is_number(initial_outside_staff)
                    || (double_or(host, initial_outside_staff, 0.0)
                        - double_or(host, last_initial_outside_staff, 0.0))
                    .abs()
                        < 0.001
                {
                    // Just higher than the previous grob's, to keep the
                    // order.
                    let p = double_or(host, last_outside_staff, 0.0) + 0.1;
                    let p = host.heap().flonum(p);
                    grob::set_property(host.heap(), g, "outside-staff-priority", p);
                }
            }
            last = Some(g);
            last_initial_outside_staff = initial_outside_staff;
        }
    }
    Ok(())
}

/// `scm_stable_sort_x` by [`script_priority_less`]: an insertion sort,
/// stable as Guile's merge sort is; the pairs compared differ, which only
/// changes which error comes first for two priorities that are no
/// integers.
fn stable_sort(host: &mut dyn Host, ss: &mut [Value]) -> Result<(), Exception> {
    for i in 1..ss.len() {
        let mut j = i;
        while j > 0 && script_priority_less(host, ss[j], ss[j - 1])? {
            ss.swap(j, j - 1);
            j -= 1;
        }
    }
    Ok(())
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:script-column::` primitives (D127):
/// the callback its second argument names, of the grob given, with the VM
/// as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "script-column",
    f: host_script_column,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 2] = [
    ("before-line-breaking", before_line_breaking),
    ("row-before-line-breaking", row_before_line_breaking),
];

fn host_script_column(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such script column callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// A `ly:script-column::` subr: its grob checked under the method's name,
/// as LilyPond's `LY_ASSERT_SMOB` gives it, and `%host-call` tail-called
/// with the callback's name ([`HOSTS`]).
fn host_call(cx: &mut Cx, method: &str, name: &str, args: &[Value]) -> R {
    grob::grob_arg(cx, method, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("script-column");
    let name = cx.heap_mut().intern(name);
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}

/// `(ly:script-column::before-line-breaking grob)`.
pub(crate) fn ly_before_line_breaking(cx: &mut Cx, args: &[Value]) -> R {
    host_call(cx, "ly:before-line-breaking", "before-line-breaking", args)
}

/// `(ly:script-column::row-before-line-breaking grob)`.
pub(crate) fn ly_row_before_line_breaking(cx: &mut Cx, args: &[Value]) -> R {
    host_call(
        cx,
        "ly:row-before-line-breaking",
        "row-before-line-breaking",
        args,
    )
}
