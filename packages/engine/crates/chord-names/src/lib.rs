//! The engine's chord names (D223), 08 §2.4's `chord-names` module, which
//! K4 splits from the core (MS4 step 20). Since MS4 step 17,
//! `chord-name.cc` whole: `ly:chord-name::after-line-breaking`
//! (`:30–46`), a `ChordName`'s and a `FretBoard`'s, the music crate's
//! placeholder until then (D67), Rust under a primitive that prints as
//! LilyPond's subr and calls it over `%host-call` (D127, [`HOSTS`]): a
//! label visible only at the beginning of a line (`begin-of-line-visible`,
//! which `chordChanges` sets on a repeated chord) killed past the line's
//! first two columns. [`install`] replaces the placeholder, as the figured
//! bass's replace theirs (MS4 step 16); it is not in layout's callback
//! table (05 §5.1), so a Rust read of it calls it through the VM.
//!
//! `Chord_name_engraver` is the image's Scheme (`scheme-engravers.scm`),
//! and the names its `chordNameFunction` gives are the `chords` part's
//! (`chord-ignatzek-names.scm`, `chord-name.scm`), loaded at the parse by
//! chord mode or before the layout by the grobs that name them (A46, D228).
#![no_std]
extern crate alloc;

use lily_engine_layout::{Callback, dispatch, grob, host_call_of, item, paper_column};
use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, Extension, HostHandler, Prim, Value, Vm};

/// `after_line_breaking`.
pub fn after_line_breaking(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let s = grob::property(host, me, "begin-of-line-visible")?;
    if s == Value::TRUE {
        let heap = host.heap();
        if let (Some(column), Some(system)) =
            (item::get_column(heap, me), item::get_system(heap, me))
        {
            let rank = paper_column::rank(heap, column) as i64;
            let (left, _) = item::spanned_column_rank_interval(heap, system);
            if rank - left as i64 > 1 {
                grob::suicide(heap, me);
            }
        }
    }
    Ok(Value::UNSPECIFIED)
}

/// Defines the chord names' primitive in `vm`'s `(lily)`, over the music
/// crate's placeholder. Layout's [`lily_engine_layout::install`] comes
/// first.
pub fn install(vm: &mut Vm) -> Result<(), Exception> {
    vm.define_primitives("lily", PRIMS, "")?;
    vm.add_host_handlers(&HOSTS);
    Ok(())
}

/// What [`lily_engine_scheme::Vm::restore`] needs of a VM that [`install`]
/// was given, after layout's.
pub static EXTENSION: Extension = Extension {
    prims: &[PRIMS],
    operators: None,
    hosts: &HOSTS,
    loaders: &[],
};

/// What `%host-call` runs for `ly:chord-name::after-line-breaking` (D127).
static HOSTS: [HostHandler; 1] = [HostHandler {
    what: "chord-name",
    f: host_chord_name,
}];

/// The `ly:chord-name::` callbacks, by name.
const CALLBACKS: [(&str, Callback); 1] = [("after-line-breaking", after_line_breaking)];

fn host_chord_name(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    dispatch(vm, &CALLBACKS, args, "no such chord-name callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

fn ly_after_line_breaking(cx: &mut Cx, args: &[Value]) -> R {
    host_call_of(
        cx,
        "chord-name",
        "ly:chord-name::",
        "after-line-breaking",
        args,
    )
}

static PRIMS: &[Prim] = &[Prim {
    name: "ly:chord-name::after-line-breaking",
    min: 1,
    max: 1,
    f: ly_after_line_breaking,
}];
