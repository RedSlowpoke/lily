//! 05 §5.1's callback table (MS3 step 46): the callbacks layout has in
//! Rust, by their names in `(lily)`, so that a Rust read of a computed
//! property (`grob.rs`' `try_callback`) runs the Rust function its
//! procedure stands for, without the VM.
//!
//! A grob's (type, property) finds the property's data, as LilyPond's
//! lookup does (its own alist, then its definition's, D203's table); the
//! data, a procedure, finds its Rust function here by what it is. The
//! table is resolved in each translation's `(lily)` the first time one of
//! its grobs calls a callback, and kept in layout's slot (D108): a score
//! that defines a procedure under a callback's name gets its own, called
//! through the VM as any other. A procedure is in the table only if what
//! the VM would run of it checks that its one argument is a grob and
//! calls the Rust function: the `ly:…::` primitives over `%host-call`
//! (D127) and grob.cc's and staff-symbol-referencer.cc's callbacks, whose
//! Scheme does the same.
//!
//! The pure callbacks take three arguments, the grob and two columns, and
//! are called by `unpure.rs`' `call_pure_function`, not by a read (MS4
//! step 1): [`find_pure`] gives theirs, those of [`TABLE`] named `pure-…`,
//! which read no column, and [`COLUMNS`]', which do.

use alloc::format;
use alloc::vec::Vec;
use core::cell::RefCell;

use lily_engine_music::{Arena, Host};
use lily_engine_scheme::{Exception, Tracer, Value, Vm};

use crate::grob::grobs;

/// A callback in Rust: the grob's handle, the property's value.
pub type Callback = fn(&mut dyn Host, Value) -> Result<Value, Exception>;

/// A pure callback in Rust that reads its columns: the grob's handle, the
/// columns `start` and `end`, the value.
pub(crate) type Columns = fn(&mut dyn Host, Value, usize, usize) -> Result<Value, Exception>;

/// A pure callback's Rust function, as [`find_pure`] gives it.
#[derive(Clone, Copy)]
pub(crate) enum Pure {
    /// One of [`TABLE`]'s `pure-…`, which reads no column.
    Grob(Callback),
    Columns(Columns),
}

/// The pure callbacks in Rust that read their columns, by module, as
/// [`TABLE`].
static COLUMNS: &[(&str, &[(&str, Columns)])] = &[
    (
        "ly:axis-group-interface::",
        &crate::axis_group::PURE_CALLBACKS,
    ),
    ("ly:slur::", &crate::slur::PURE_CALLBACKS),
];

/// The callbacks in Rust, by module: the prefix of their names in
/// `(lily)`, and each name's rest with its function.
static TABLE: &[(&str, &[(&str, Callback)])] = &[
    ("ly:grob::", &crate::grob::CALLBACKS),
    (
        "ly:staff-symbol-referencer::",
        &crate::staff_symbol_referencer::CALLBACKS,
    ),
    ("ly:stem::", &crate::stem::CALLBACKS),
    ("ly:beam::", &crate::beam::CALLBACKS),
    ("ly:stem-tremolo::", &crate::stem_tremolo::CALLBACKS),
    ("ly:slur::", &crate::slur::CALLBACKS),
    ("ly:tie::", &crate::tie::CALLBACKS),
    ("ly:tie-column::", &crate::tie_column::CALLBACKS),
    ("ly:semi-tie::", &crate::semi_tie::CALLBACKS),
    ("ly:semi-tie-column::", &crate::semi_tie_column::CALLBACKS),
    ("ly:tuplet-bracket::", &crate::tuplet_bracket::CALLBACKS),
    ("ly:tuplet-number::", &crate::tuplet_number::CALLBACKS),
    ("ly:lyric-extender::", &crate::lyric_extender::CALLBACKS),
    ("ly:lyric-hyphen::", &crate::lyric_hyphen::CALLBACKS),
    ("ly:vowel-transition::", &crate::vowel_transition::CALLBACKS),
    ("ly:melody-spanner::", &crate::melody_engraver::CALLBACKS),
    (
        "ly:piano-pedal-bracket::",
        &crate::piano_pedal::BRACKET_CALLBACKS,
    ),
    ("ly:sustain-pedal::", &crate::piano_pedal::SUSTAIN_CALLBACKS),
    ("ly:grid-line-interface::", &crate::grid::CALLBACKS),
    ("ly:cluster::", &crate::cluster::CALLBACKS),
    ("ly:balloon-interface::", &crate::balloon::CALLBACKS),
    ("ly:cluster-beacon::", &crate::cluster::BEACON_CALLBACKS),
    ("ly:measure-grouping::", &crate::measure::GROUPING_CALLBACKS),
    ("ly:measure-spanner::", &crate::measure::SPANNER_CALLBACKS),
    (
        "ly:mensural-ligature::",
        &crate::mensural_ligature::CALLBACKS,
    ),
    (
        "ly:vaticana-ligature::",
        &crate::vaticana_ligature::CALLBACKS,
    ),
    ("ly:kievan-ligature::", &crate::kievan_ligature::CALLBACKS),
    ("ly:flag::", &crate::flag::CALLBACKS),
    ("ly:accidental-interface::", &crate::accidental::CALLBACKS),
    ("ly:clef::", &crate::clef::CALLBACKS),
    ("ly:breathing-sign::", &crate::breathing_sign::CALLBACKS),
    (
        "ly:percent-repeat-interface::",
        &crate::percent_repeat::CALLBACKS,
    ),
    ("ly:hairpin::", &crate::hairpin::CALLBACKS),
    (
        "ly:volta-bracket-interface::",
        &crate::volta_bracket::CALLBACKS,
    ),
    ("ly:ottava-bracket::", &crate::ottava_bracket::CALLBACKS),
    (
        "ly:horizontal-bracket::",
        &crate::horizontal_bracket::CALLBACKS,
    ),
    ("ly:line-spanner::", &crate::line_spanner::CALLBACKS),
    (
        "ly:horizontal-line-spanner::",
        &crate::line_spanner::HORIZONTAL_CALLBACKS,
    ),
    ("ly:script-interface::", &crate::script::CALLBACKS),
    ("ly:script-column::", &crate::script_column::CALLBACKS),
    ("ly:fingering-column::", &crate::fingering_column::CALLBACKS),
    ("ly:arpeggio::", &crate::arpeggio::CALLBACKS),
    ("ly:chord-bracket::", &crate::arpeggio::BRACKET_CALLBACKS),
    ("ly:chord-slur::", &crate::arpeggio::SLUR_CALLBACKS),
    (
        "ly:ledger-line-spanner::",
        &crate::ledger_line_spanner::CALLBACKS,
    ),
    ("ly:separation-item::", &crate::separation_item::CALLBACKS),
    ("ly:axis-group-interface::", &crate::axis_group::CALLBACKS),
];

/// The table resolved in a translation's `(lily)`: each callback's
/// procedure, by its bits, sorted, with its function.
#[derive(Clone)]
pub(crate) struct Resolved {
    by_proc: Vec<(u64, Callback)>,
    pure_by_proc: Vec<(u64, Pure)>,
    procs: Vec<Value>,
}

impl Resolved {
    /// The procedures, kept alive while the table names them.
    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        for &p in &self.procs {
            tracer.trace(p);
        }
    }
}

/// The table's procedures as `host`'s `(lily)` binds them.
fn resolve(host: &mut dyn Host) -> Resolved {
    let (mut by_proc, mut pure_by_proc, mut procs) = (Vec::new(), Vec::new(), Vec::new());
    for &(prefix, callbacks) in TABLE {
        for &(name, f) in callbacks {
            if let Some(proc) = host.lily(&format!("{prefix}{name}")) {
                if name.starts_with("pure-") {
                    pure_by_proc.push((proc.to_bits(), Pure::Grob(f)));
                } else {
                    by_proc.push((proc.to_bits(), f));
                }
                procs.push(proc);
            }
        }
    }
    for &(prefix, callbacks) in COLUMNS {
        for &(name, f) in callbacks {
            if let Some(proc) = host.lily(&format!("{prefix}{name}")) {
                pure_by_proc.push((proc.to_bits(), Pure::Columns(f)));
                procs.push(proc);
            }
        }
    }
    by_proc.sort_unstable_by_key(|e| e.0);
    pure_by_proc.sort_unstable_by_key(|e| e.0);
    Resolved {
        by_proc,
        pure_by_proc,
        procs,
    }
}

/// The table of `arena`'s translation, resolved the first time.
fn resolved<'a>(host: &mut dyn Host, arena: &'a RefCell<Arena>) -> core::cell::RefMut<'a, Arena> {
    if grobs(&mut arena.borrow_mut()).callbacks.is_none() {
        let resolved = resolve(host);
        grobs(&mut arena.borrow_mut()).callbacks = Some(resolved);
    }
    arena.borrow_mut()
}

/// The Rust function of the callback `proc`, of a grob of `arena`'s
/// translation, if the table has it.
pub(crate) fn find(host: &mut dyn Host, arena: &RefCell<Arena>, proc: Value) -> Option<Callback> {
    let mut a = resolved(host, arena);
    let table = grobs(&mut a).callbacks.as_ref()?;
    let i = table.by_proc.binary_search_by_key(&proc.to_bits(), |e| e.0);
    i.ok().map(|i| table.by_proc[i].1)
}

/// The Rust function of the pure callback `proc`, of a grob of `arena`'s
/// translation, if the table has it.
pub(crate) fn find_pure(host: &mut dyn Host, arena: &RefCell<Arena>, proc: Value) -> Option<Pure> {
    let mut a = resolved(host, arena);
    let table = grobs(&mut a).callbacks.as_ref()?;
    let i = table
        .pure_by_proc
        .binary_search_by_key(&proc.to_bits(), |e| e.0);
    i.ok().map(|i| table.pure_by_proc[i].1)
}

/// What a module's `%host-call` handler runs (D127): the callback of
/// `callbacks` that `args[1]` names, on the grob `args[2]`, with the VM as
/// its host; `none`, a `misc-error`, for no such callback.
pub fn dispatch(
    vm: &mut Vm,
    callbacks: &[(&str, Callback)],
    args: &[Value],
    none: &str,
) -> Result<Value, Exception> {
    let name = vm.heap().name(args[1]);
    match callbacks.iter().find(|&&(n, _)| Some(n) == name) {
        Some(&(_, f)) => f(vm, args[2]),
        None => Err(crate::misc_error(vm.heap(), "%host-call", none)),
    }
}
