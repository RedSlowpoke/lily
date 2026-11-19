//! `clef-modifier.cc` (part 3's, MS3 step 42): the clef modifier's
//! callback `ly:clef-modifier::calc-parent-alignment`
//! (`clef-modifier.cc:27–55`, 29 lines), the music crate's placeholder
//! until then (D67). Rust under a primitive that prints as LilyPond's
//! subr and calls it over `%host-call` (D127, [`HOSTS`]), as the clef's.
//!
//! The ClefModifier (the `8` or `15` the clef engraver makes of a
//! `clefTransposition`, `clef_engraver.rs`) is centred on its X parent,
//! the clef, by `ly:self-alignment-interface::aligned-on-x-parent`
//! (`self_alignment.rs`), at the parent alignment this gives, and placed
//! under or over it by `side-position-interface::y-aligned-side`
//! (`side_position.rs`). A modifier without an X parent, which LilyPond
//! dereferences, throws `misc-error`.

use alloc::string::String;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, Heap, HostHandler, Value, Vm};

use crate::geometry::{Direction, X};
use crate::grob;
use crate::note_collision::direction_or;

const DOWN: Direction = -1;

/// `calc_parent_alignment`: the entry of the clef's glyph without its
/// `clefs.` in `clef-alignments`, a pair of the alignment below the clef
/// and the one above: its first for a modifier of `direction` down (or
/// none), else its second; 0, centred, for no entry or one that is no
/// pair.
pub fn calc_parent_alignment(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let clef = grob::get_parent(host.heap(), me, X).ok_or_else(|| {
        crate::misc_error(
            host.heap(),
            "ly:clef-modifier::calc-parent-alignment",
            "clef modifier without a clef",
        )
    })?;
    let glyph = grob::property(host, clef, "glyph")?;
    let Some(full_clef_name) = host.heap().str(glyph).map(|s| String::from(s.as_str())) else {
        return Err(crate::wrong_type(
            host.heap(),
            Some("scm_to_utf8_stringn"),
            0,
            glyph,
            "string",
        ));
    };
    let clef_name = full_clef_name.replace("clefs.", "");

    let key = host.heap().intern(&clef_name);
    let alignments = grob::property(host, me, "clef-alignments")?;
    let Some(entry) = assq(host.heap(), key, alignments)? else {
        return Ok(host.heap().integer(0));
    };
    let heap = host.heap();
    let value = heap.cdr(entry).unwrap_or(Value::NULL);
    let (Some(below), Some(above)) = (heap.car(value), heap.cdr(value)) else {
        return Ok(host.heap().integer(0));
    };
    let direction = grob::property(host, me, "direction")?;
    Ok(if direction_or(direction, DOWN) == DOWN {
        below
    } else {
        above
    })
}

/// `scm_assq`: the first entry of `alist` whose car is `key`; Guile's
/// type error, naming the whole `alist`, for an element before it that is
/// no pair or an `alist` that does not end in `'()`.
fn assq(heap: &mut Heap, key: Value, alist: Value) -> Result<Option<Value>, Exception> {
    let mut at = alist;
    while let (Some(e), Some(rest)) = (heap.car(at), heap.cdr(at)) {
        match heap.car(e) {
            Some(k) if k == key => return Ok(Some(e)),
            Some(_) => at = rest,
            None => break,
        }
    }
    if at != Value::NULL {
        return Err(crate::wrong_type(
            heap,
            Some("assq"),
            2,
            alist,
            "association list",
        ));
    }
    Ok(None)
}

// --- the callback -----------------------------------------------------------

/// What `%host-call` runs for the `ly:clef-modifier::` primitive (D127):
/// the callback its second argument names, of the grob given, with the VM
/// as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "clef-modifier",
    f: host_clef_modifier,
}];

fn host_clef_modifier(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    match vm.heap().name(args[1]) {
        Some("calc-parent-alignment") => calc_parent_alignment(vm, args[2]),
        _ => Err(crate::misc_error(
            vm.heap(),
            "%host-call",
            "no such clef-modifier callback",
        )),
    }
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:clef-modifier::calc-parent-alignment grob)`: LilyPond's subr, a
/// primitive here so that it prints as LilyPond's. It checks its grob and
/// tail-calls `%host-call` with its name ([`HOSTS`]).
pub(crate) fn ly_calc_parent_alignment(cx: &mut Cx, args: &[Value]) -> R {
    let grob = args[0];
    grob::grob_arg(cx, "ly:clef-modifier::calc-parent-alignment", 1, grob)?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("clef-modifier");
    let callback = cx.heap_mut().intern("calc-parent-alignment");
    Ok(cx.tail_call(f, &[what, callback, grob]))
}
