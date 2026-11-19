//! Layout's part of a grob (01 §3.5's L1, D108, MS3 step 13): what
//! LilyPond's `Grob` holds beyond the music crate's record (`records.rs`'
//! `GrobRecord`, whose number is the grob's identity and which holds its
//! type, cause, interfaces, definition and the properties set on it).
//! Layout keeps it in D108's slot of the record's translation
//! ([`lily_engine_music::GrobSlot`]), by the record's number, for every
//! record: a class's, a Scheme engraver's (`ly:engraver-make-grob`) too.
//!
//! - The property lookup of `grob-property.cc`: the properties set on the
//!   grob (the record's `props`, LilyPond's mutable alist, with the cause
//!   `Engraver::announce_grob` sets), then its definition's (the record's
//!   `definition`, the immutable alist, searched by its table, D203), else
//!   `'()` ([`property_data`]); an unpure-pure container's unpure part; a
//!   procedure called with the grob once, `calculation-in-progress` in its
//!   place while it runs and its value kept in the grob
//!   (`try_callback_on_alist`), and LilyPond's programming error for a
//!   cycle that reads the marker. A primitive cannot call a procedure, so
//!   `ly:grob-property` leaves the call to [`PRELUDE`]'s
//!   `%grob-try-callback`.
//! - `set_property` (`internal_set_value_on_alist`): on a live grob, the
//!   entry set in place or added in front ([`set_property_data`]);
//!   `ly:grob-set-property!` checks the value's `backend-type?` first, as
//!   `ly:context-set-property!` checks its own (`%type-check`).
//! - The object alist (`object_alist_`): `ly:grob-object`, whose callback
//!   is called as a property's, and `ly:grob-set-object!`. Its grob arrays
//!   and pointer groups are `grob_array.rs`' (MS3 step 14).
//! - The callbacks' chains of `grob-closure.cc` ([`chain_callback`],
//!   [`chain_offset_callback`], [`add_offset_callback`]): a property's
//!   data, as it is, composed with a procedure by `output-lib.scm`'s
//!   `grob::compose-function` or `grob::offset-function`, which the image
//!   holds and the [`Host`] calls, and set in its place.
//! - The constructor's part (`grob.cc:43–90`, [`construct`]), run the first
//!   time layout reaches a grob: `meta`'s object callbacks, an `X-extent`
//!   and a `Y-extent` from the stencil where the grob has none, and
//!   `vertical-skylines` and `horizontal-skylines` from its extents
//!   (`skylines.rs`, MS3 step 26) where it has none.
//! - Parents, offsets and extents on both axes (`Dimension_cache`, [`Dim`]):
//!   `set_parent`, `get_offset`, `relative_coordinate`, `extent`,
//!   `translate_axis` and `common_refpoint`, with `ly:grob-parent`,
//!   `ly:grob-set-parent!`, `ly:grob-relative-coordinate`,
//!   `ly:grob-extent`, `ly:grob-translate-axis!` and
//!   `ly:grob-common-refpoint`. A primitive cannot call a callback, so
//!   [`PRELUDE`] walks the parents and reads the properties, and the
//!   primitives keep the caches; Rust reaches them through a [`Host`]
//!   ([`relative_coordinate`], [`extent`]).
//! - `grob.cc`'s callbacks of the stencil's extents and of the parent's
//!   positioning: `ly:grob::stencil-width`, `ly:grob::stencil-height`,
//!   `ly:grob::pure-stencil-height` and `ly:grob::x-parent-positioning`,
//!   in [`PRELUDE`]; two were the music crate's placeholders. And
//!   `ly:grob::y-parent-positioning` (`grob.cc:921–932`), which
//!   `align.rs`' `add_element` sets on Y (MS3 step 21), its type error
//!   from `ly:y-parent-positioning` as LilyPond names it.
//! - The grob's origin (`Grob::origin`, [`origin`]), the input location
//!   of the event its causes lead to, at which its warnings and
//!   programming errors are given ([`warning`], [`programming_error`];
//!   [`PRELUDE`]'s `%grob-reported` for a primitive's).
//! - `suicide` ([`suicide`], `ly:grob-suicide!`) and the print stencil
//!   ([`get_print_stencil`]): the `stencil` with its whiteout, the
//!   `grob-cause` wrapper, rotation, color and output attributes, through
//!   the image's `stencil.scm`.
//! - The link the SVG writer's `grob-cause` writes for a grob: its
//!   cause's origin ([`cause_link`], [`PRELUDE`]'s `%grob-cause-link`).
//! - The pure lookups (MS3 step 14, `unpure.rs`): `call_pure_function`,
//!   the pure property, the pure coordinate and height on Y, an item's
//!   pure height kept ([`Grob::pure_height`]), with `ly:grob-pure-property`
//!   and `ly:grob-pure-height`.
//! - `has_interface` (MS3 step 14, [`has_interface`]): by the grob's own
//!   interfaces, the record's, which its class's constructors add to
//!   ([`add_interface`], `item.rs`). `grob-interface.cc`'s check of a
//!   property against them is `check-internal-types`', and not here;
//!   `ly:add-interface` and `ly:all-grob-interfaces` are the scheme
//!   crate's (`lily.rs`), as `grob-interface-scheme.cc` gives them.
//! - A spanner's bounds, its broken pieces and a piece's index
//!   ([`Grob::bounds`], [`Grob::broken_intos`]), which `spanner.rs` keeps
//!   (MS3 step 15).
//! - Breaking: `fixup_refpoint` (MS3 step 32, [`fixup_refpoint`]), and,
//!   after line breaking (MS3 step 33), `do_break_processing`
//!   ([`do_break_processing`]) and `handle_broken_dependencies`
//!   ([`handle_broken_dependencies`]).
//! - The rest of `grob-scheme.cc` (MS3 step 14): `ly:grob-property-data`,
//!   `ly:grob-set-nested-property!` (`nested-property.cc`'s
//!   `set_nested_property`), `ly:grob-interfaces` (LilyPond's list,
//!   [`interfaces`]), `ly:grob-layout`, `ly:grob-alist-chain`
//!   ([`alist_chain`]), `ly:grob-robust-relative-extent`,
//!   `ly:grob-basic-properties`, `ly:grob-system` and `ly:grob-original`,
//!   `ly:grob-common-refpoint-of-array`, `ly:grob-chain-callback`,
//!   `ly:grob-vertical<?`, `ly:grob-get-vertical-axis-group-index` and
//!   `ly:grob-spanned-column-rank-interval`; `ly:grob-default-font` is
//!   `font_select.rs`', `ly:grob-pure-relative-coordinate` `unpure.rs'`.
//!   Those that call a callback are [`PRELUDE`]'s, over primitives that
//!   do not.
//!
//! Since MS3 step 46 (05 §5.1's machinery, lead 1) a Rust read is Rust
//! over the [`Host`] from end to end: `internal_get_property` and
//! `try_callback_on_alist` ([`get_property`], `try_callback`, whose
//! callback the callback table runs in Rust where it has it,
//! `callbacks.rs`), `get_offset`, `relative_coordinate`, `extent`,
//! `robust_relative_extent`, the vertical ordering and grob.cc's callbacks
//! of the stencil's extents and the parent's positioning; the Host is
//! called only for a callback in Scheme, a report, or a read the dump
//! takes over (`Grobs::dumping`). [`PRELUDE`]'s coordinates, extents and
//! callbacks call the Rust over `%host-call` ([`HOSTS`], D127), and keep
//! the image's Scheme of each for a run left waiting for the host
//! (`%host-waiting?`), where a `%host-call` would be cancelled, as a traced
//! compile's reads are after a part's file is asked for (T22).
//!
//! A grob is live while its definition is an alist (`is_live`). The
//! debugging callbacks of a build with checking
//! (`ly:set-grob-modification-callback`, `ly:set-property-cache-callback`,
//! `debug-property-callbacks`) and the type checks of
//! `check-internal-types` are not here. Nor are the skylines the print
//! stencil shows for `show-horizontal-skylines` and
//! `show-vertical-skylines` (part 3). `flush_extent_cache` is here since
//! MS3 step 39, which the accidental placement calls.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::cell::RefCell;

use lily_engine_music::{
    Arena, GrobSlot, Host, Input, InterfaceSet, StreamEvent, event_property, grob_cause, grob_of,
    host_lookup_variable, unpure_of,
};
use lily_engine_scheme::{Cx, Exception, Heap, HostHandler, Rows, Tag, Thrown, Tracer, Value, Vm};

use crate::R;
use crate::break_substitution;
use crate::callbacks::Callback;
use crate::geometry::{Axis, Interval, X, Y};
use crate::item::{self, Class};
use crate::paper_column::Column;
use crate::stencil::{self, Stencil};

/// `Dimension_cache`: a grob's offset from its parent and its extent on an
/// axis, once computed or translated, and its parent there.
#[derive(Clone, Copy, Default)]
pub struct Dim {
    pub offset: Option<f64>,
    pub extent: Option<Interval>,
    /// The parent's handle.
    pub parent: Option<Value>,
}

/// What layout keeps of one grob.
#[derive(Clone)]
pub struct Grob {
    /// `object_alist_`: its grobs and grob arrays, `'()` when made.
    pub objects: Value,
    /// `dim_cache_`, by axis.
    pub dims: [Dim; 2],
    /// `original_`: the handle of the grob it was copied from, if it is a
    /// broken copy.
    pub original: Option<Value>,
    /// An item's `broken_to_drul_`: the handles of its copies at the end of
    /// a line and at the beginning of the next, which breaking makes (part 5).
    pub broken: [Option<Value>; 2],
    /// An item's `cached_pure_height_`, relative to itself, once
    /// `unpure.rs`' pure height has computed it; suicide keeps it.
    pub pure_height: Option<Interval>,
    /// Whether making it read an alignment's translation
    /// ([`crate::unpure::translations_read`]).
    pub(crate) pure_height_moves: bool,
    /// Its pure offsets on Y as `unpure.rs` computed them before the line
    /// breaks, each with the epoch of the breaks, its columns and the offset
    /// (D252).
    pub(crate) pure_offsets: Vec<(u32, usize, usize, f64, bool)>,
    /// Its class (`item.rs`), and whether it has been given one or looked
    /// for its definition's.
    pub(crate) class: Option<Class>,
    pub(crate) classified: bool,
    /// `layout_`: the handle of the output definition it was typeset with
    /// (`system.rs`' `typeset_grob`, `paper_score.rs`' `typeset_system`
    /// for a system), none before (MS3 step 15).
    pub layout: Option<Value>,
    /// A system's `pscore_`: the handle of its `Paper_score`
    /// (`paper_score.rs`), none before it is typeset in one.
    pub pscore: Option<Value>,
    /// A system's `rank_`: its place among the broken systems, 0 until
    /// `break_into_pieces` numbers them (`system.rs`, MS3 step 33); a copy
    /// starts at 0, as `System`'s copy constructor leaves it.
    pub system_rank: u64,
    /// A column's `system_` and `rank_` (`paper_column.rs`).
    pub column: Column,
    /// A spanner's `spanned_drul_`: the handles of its left and right
    /// bounds, items (`spanner.rs`, MS3 step 15).
    pub bounds: [Option<Value>; 2],
    /// A spanner's `broken_intos_`: the handles of its pieces, one a
    /// system, which breaking makes (part 5), and a piece's
    /// `break_index_`, its place among them.
    pub broken_intos: Vec<Value>,
    pub break_index: usize,
    /// A spanner's `pure_property_cache_`: each property's value between
    /// two columns, by its symbol's bits, start and end, with the symbol,
    /// once kept (`spanner.rs`' `cache_pure_property`, MS3 step 28; a map
    /// since MS4 step 1, as the breakers ask it for every candidate line).
    pub pure_cache: BTreeMap<(u64, usize, usize), (Value, Value)>,
    /// A line read while a line of marks was held (B28, D272, `system.rs`):
    /// the next part-of-line height asked of it is made again, not found,
    /// as LilyPond's nested read had asked it before the line was read.
    pub(crate) pure_fresh: bool,
    /// An alignment's `minimum-translations-alist` by its keys (`align.rs`,
    /// MS4 step 1).
    pub(crate) translations: crate::align::Translations,
    /// The number of its definition's [`Table`], once searched.
    table: Option<u32>,
    /// Its own properties' slots (`slots.rs`), over the record's `props`.
    own: crate::slots::Own,
    /// Whether [`construct`] has run.
    made: bool,
}

impl Grob {
    fn new() -> Grob {
        Grob {
            objects: Value::NULL,
            dims: [Dim::default(); 2],
            original: None,
            broken: [None; 2],
            pure_height: None,
            pure_height_moves: false,
            pure_offsets: Vec::new(),
            class: None,
            classified: false,
            layout: None,
            pscore: None,
            system_rank: 0,
            column: Column::default(),
            bounds: [None; 2],
            broken_intos: Vec::new(),
            break_index: 0,
            pure_cache: BTreeMap::new(),
            pure_fresh: false,
            translations: crate::align::Translations::default(),
            table: None,
            own: crate::slots::Own::default(),
            made: false,
        }
    }
}

/// Whether two states of one grob are the same (N8's splice, D243): what
/// they name by identity, and their numbers bit for bit.
fn same(a: &Grob, b: &Grob) -> bool {
    let bits = |v: Option<Value>| v.map(Value::to_bits);
    let dim = |d: &Dim| {
        (
            d.offset.map(f64::to_bits),
            d.extent.map(|e| (e.left.to_bits(), e.right.to_bits())),
            bits(d.parent),
        )
    };
    let pure = |i: Option<Interval>| i.map(|e| (e.left.to_bits(), e.right.to_bits()));
    a.objects == b.objects
        && dim(&a.dims[0]) == dim(&b.dims[0])
        && dim(&a.dims[1]) == dim(&b.dims[1])
        && a.original == b.original
        && a.broken == b.broken
        && pure(a.pure_height) == pure(b.pure_height)
        && a.class == b.class
        && a.classified == b.classified
        && a.layout == b.layout
        && a.pscore == b.pscore
        && a.system_rank == b.system_rank
        && a.column == b.column
        && a.bounds == b.bounds
        && a.broken_intos == b.broken_intos
        && a.break_index == b.break_index
        && a.pure_cache == b.pure_cache
        && a.translations.same(&b.translations)
        && a.table == b.table
        && a.own.same(&b.own)
        && a.made == b.made
}

/// A grob definition's table (05 §5.1's generated slots, D203): the first
/// entry of each key of the alist, by the key's bits, which
/// [`property_data`] searches where `scm_sloppy_assq` walks the alist. A
/// definition is not changed in place (LilyPond's
/// `immutable_property_alist_`: the override stack cooks a new one,
/// `override.rs`' `updated`), and an entry's value is read when it is
/// asked for, so the table finds what the walk finds.
#[derive(Clone)]
struct Table {
    alist: Value,
    entries: Vec<(u64, Value)>,
}

impl Table {
    fn new(heap: &Heap, alist: Value) -> Table {
        let mut entries: Vec<(u64, Value)> = Vec::new();
        let mut p = alist;
        while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            // A key keeps its first entry.
            if let Some(key) = heap.car(entry)
                && let Err(i) = entries.binary_search_by_key(&key.to_bits(), |e| e.0)
            {
                entries.insert(i, (key.to_bits(), entry));
            }
            p = rest;
        }
        Table { alist, entries }
    }

    /// The first entry of `key`, as `scm_sloppy_assq` finds it.
    fn entry(&self, key: Value) -> Option<Value> {
        let i = self.entries.binary_search_by_key(&key.to_bits(), |e| e.0);
        i.ok().map(|i| self.entries[i].1)
    }
}

/// Layout's grobs of a translation, by record number, and their
/// definitions' tables: what it keeps in D108's slot.
#[derive(Clone, Default)]
pub struct Grobs {
    /// In rows, which N8's snapshots share (D259).
    grobs: Rows<Grob>,
    tables: Vec<Table>,
    /// Each table's number by its alist's bits, sorted.
    by_alist: Vec<(u64, u32)>,
    /// How many of the dump's reads are running on this translation's
    /// grobs (`dump.rs`' `dump_read`): while any is, a callback and an
    /// offset are read through `(lily)`, whose procedures the dump
    /// replaces ([`try_callback`], [`get_offset`]).
    pub(crate) dumping: u32,
    /// The callback table, resolved in this translation's `(lily)`
    /// (`callbacks.rs`), once a callback is called.
    pub(crate) callbacks: Option<crate::callbacks::Resolved>,
    /// What Y7's cells note of the reads and writes of these grobs
    /// (`cell.rs`), once the translation is laid out.
    pub(crate) tracker: crate::cell::Tracker,
}

impl GrobSlot for Grobs {
    fn clone_slot(&self) -> alloc::boxed::Box<dyn GrobSlot> {
        alloc::boxed::Box::new(self.clone())
    }

    fn take_layout(&mut self, heap: &mut Heap, old: Value, new: Value) {
        // Read first: most grobs of a translation are not laid out yet, and
        // a write would copy a row its snapshots share.
        for i in 0..self.grobs.len() {
            let g = &self.grobs[i];
            let (typeset, pscore) = (g.layout == Some(old), g.pscore);
            if typeset {
                self.grobs[i].layout = Some(new);
            }
            if let Some(p) =
                pscore.and_then(|p| heap.foreign_mut::<crate::paper_score::PaperScore>(p))
                && p.layout == old
            {
                p.layout = new;
            }
        }
    }

    fn same_grob(&self, before: &dyn GrobSlot, i: usize) -> bool {
        let before: &dyn Any = before;
        let Some(before) = before.downcast_ref::<Grobs>() else {
            return false;
        };
        // A slot fills itself as far as its grobs when it is asked: a grob
        // it has not got yet is as made.
        let made = Grob::new();
        same(
            self.grobs.get(i).unwrap_or(&made),
            before.grobs.get(i).unwrap_or(&made),
        )
    }

    fn grob_digest(&self, i: usize, value: &mut dyn FnMut(Value) -> u64) -> u64 {
        let made = Grob::new();
        let g = self.grobs.get(i).unwrap_or(&made);
        let interval = |e: Interval| e.left.to_bits() ^ e.right.to_bits().rotate_left(1);
        // Its values, then its numbers.
        let mut values = alloc::vec![Some(g.objects)];
        let mut numbers = alloc::vec![];
        for d in &g.dims {
            values.push(d.parent);
            numbers.push(d.offset.map_or(1, f64::to_bits));
            numbers.push(d.extent.map_or(1, interval));
        }
        values.extend([g.original, g.layout, g.pscore, g.column.system]);
        values.extend(g.broken.iter().chain(&g.bounds).copied());
        values.extend(g.broken_intos.iter().map(|&b| Some(b)));
        numbers.extend([
            g.pure_height.map_or(1, interval),
            u64::from(g.classified) | u64::from(g.made) << 1,
            g.system_rank,
            g.column.rank,
            g.break_index as u64,
            g.pure_cache.len() as u64,
            g.broken_intos.len() as u64,
        ]);
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |x: u64| h = (h ^ x).wrapping_mul(0x0100_0000_01b3);
        for v in values {
            mix(v.map_or(1, &mut *value));
        }
        for x in numbers {
            mix(x);
        }
        mix(u64::from(g.class.is_some()));
        if let Some(c) = g.class {
            mix(c.name().len() as u64);
            for b in c.name().bytes() {
                mix(b.into());
            }
        }
        h
    }

    fn put_grob(&mut self, other: &dyn GrobSlot, i: usize) {
        let other: &dyn Any = other;
        let Some(other) = other.downcast_ref::<Grobs>() else {
            return;
        };
        while self.grobs.len() <= i {
            self.grobs.push(Grob::new());
        }
        self.grobs[i] = other.grobs.get(i).cloned().unwrap_or_else(Grob::new);
        // Its table is found again in this slot's (`definition_entry`).
        self.grobs[i].table = None;
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        if let Some(callbacks) = &self.callbacks {
            callbacks.trace(tracer);
        }
        for t in &self.tables {
            tracer.trace(t.alist);
        }
        self.grobs.trace(tracer, |g, tracer| {
            tracer.trace(g.objects);
            let parents = g.dims.iter().filter_map(|d| d.parent);
            for grob in parents
                .chain(g.original)
                .chain(g.broken.into_iter().flatten())
                .chain(g.layout)
                .chain(g.pscore)
                .chain(g.column.system)
                .chain(g.bounds.into_iter().flatten())
                .chain(g.broken_intos.iter().copied())
            {
                tracer.trace(grob);
            }
            g.own.trace(tracer);
            for &(sym, value) in g.pure_cache.values() {
                tracer.trace(sym);
                tracer.trace(value);
            }
            g.translations.trace(tracer);
        });
    }
}

impl Grobs {
    /// The grob of the record `id`.
    pub fn get(&self, id: u32) -> Option<&Grob> {
        self.grobs.get(id as usize)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Grob> {
        self.grobs.get_mut(id as usize)
    }

    /// How many records it has a grob of.
    pub fn len(&self) -> usize {
        self.grobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.grobs.is_empty()
    }

    /// The entry of `key` in `definition`, the definition of the grob `id`,
    /// by its [`Table`], made the first time a grob of it asks.
    fn definition_entry(
        &mut self,
        heap: &Heap,
        id: u32,
        definition: Value,
        key: Value,
    ) -> Option<Value> {
        let n = match self.grobs[id as usize].table {
            Some(n) if self.tables[n as usize].alist == definition => n,
            _ => {
                let bits = definition.to_bits();
                let n = match self.by_alist.binary_search_by_key(&bits, |e| e.0) {
                    Ok(i) => self.by_alist[i].1,
                    Err(i) => {
                        let n = self.tables.len() as u32;
                        self.tables.push(Table::new(heap, definition));
                        self.by_alist.insert(i, (bits, n));
                        n
                    }
                };
                self.grobs[id as usize].table = Some(n);
                n
            }
        };
        self.tables[n as usize].entry(key)
    }
}

/// The grobs of the translation whose arena is `a`: its slot, filled the
/// first time it is asked for, with a grob for each record made since.
pub fn grobs(a: &mut Arena) -> &mut Grobs {
    let made = a.records.grobs.len();
    let slot = a
        .grob_slot
        .get_or_insert_with(|| Box::new(Grobs::default()));
    let slot: &mut dyn Any = &mut **slot;
    let grobs = slot
        .downcast_mut::<Grobs>()
        .expect("only layout fills D108's slot");
    while grobs.grobs.len() < made {
        grobs.grobs.push(Grob::new());
    }
    grobs
}

/// What `f` makes of layout's part of the grob `grob`, if it is a grob.
pub fn with_grob<T>(heap: &Heap, grob: Value, f: impl FnOnce(&Grob) -> T) -> Option<T> {
    let (arena, id) = grob_of(heap, grob)?;
    let mut a = arena.borrow_mut();
    grobs(&mut a).get(id).map(f)
}

/// [`with_grob`] that changes it, which copies its row if N8's snapshots
/// share it (D259), so it is for what writes, never to read.
pub fn with_grob_mut<T>(heap: &Heap, grob: Value, f: impl FnOnce(&mut Grob) -> T) -> Option<T> {
    let (arena, id) = grob_of(heap, grob)?;
    let mut a = arena.borrow_mut();
    grobs(&mut a).get_mut(id).map(f)
}

// --- the property lookup --------------------------------------------------

/// `scm_sloppy_assq`: the entry of `alist` whose key is `key`.
pub(crate) fn assq(heap: &Heap, key: Value, alist: Value) -> Option<Value> {
    heap.assq(key, alist)
}

/// The value of `key` in `alist`.
pub(crate) fn assq_ref(heap: &Heap, key: Value, alist: Value) -> Option<Value> {
    assq(heap, key, alist).and_then(|e| heap.cdr(e))
}

/// `is_live`: the grob's definition is an alist (`suicide` empties it).
fn is_live(arena: &RefCell<Arena>, id: u32) -> bool {
    arena.borrow().records.grobs[id as usize].definition.tag() == Tag::Pair
}

/// `is_live` of the grob `grob`; false for no grob.
pub fn is_live_grob(heap: &mut Heap, grob: Value) -> bool {
    grob_of(heap, grob).is_some_and(|(arena, id)| is_live(&arena, id))
}

/// `internal_get_property_data` of `sym` in the grob `id` of `arena`,
/// whose handle is `grob`: set on it, its cause for `cause`, its
/// definition's, else `'()`. No callback is called.
pub fn property_data(
    heap: &mut Heap,
    arena: &RefCell<Arena>,
    id: u32,
    grob: Value,
    sym: Value,
) -> Value {
    let (props, definition) = {
        let a = arena.borrow();
        let g = &a.records.grobs[id as usize];
        (g.props, g.definition)
    };
    let entry = {
        let mut a = arena.borrow_mut();
        let slot = grobs(&mut a);
        slot.tracker.read(id, sym);
        match slot.grobs[id as usize].own.kept_entry(props, sym) {
            Some(entry) => entry,
            None => slot.grobs[id as usize].own.entry(heap, props, sym),
        }
    };
    if let Some(v) = entry.and_then(|e| heap.cdr(e)) {
        return v;
    }
    if heap.name(sym) == Some("cause")
        && let Some(cause) = grob_cause(heap, grob)
    {
        return cause;
    }
    if definition.tag() != Tag::Pair {
        return Value::NULL;
    }
    let entry = grobs(&mut arena.borrow_mut()).definition_entry(heap, id, definition, sym);
    entry.and_then(|e| heap.cdr(e)).unwrap_or(Value::NULL)
}

/// Which of a grob's alists a value is set in: its properties (the
/// record's `props`) or its objects.
#[derive(Clone, Copy)]
enum Store {
    Property,
    Object,
}

/// `internal_set_value_on_alist` of `alist`, an alist and the place it is
/// kept: on a live grob, `sym`'s entry set in place, or added in front
/// (`scm_assq_set_x`).
///
/// The record's `props` is found and kept by its slots (`slots.rs`).
fn set_on_alist(
    heap: &mut Heap,
    arena: &RefCell<Arena>,
    id: u32,
    sym: Value,
    v: Value,
    store: Store,
) {
    if !is_live(arena, id) {
        return;
    }
    let op = match store {
        Store::Property => lily_engine_music::Op::Property(sym, v),
        // A grob array as it is now: the one set is added to after (N8's
        // replay, MS6 step 7b).
        Store::Object if lily_engine_music::recorded(arena) => {
            lily_engine_music::Op::Object(sym, crate::grob_array::copied(heap, v))
        }
        Store::Object => lily_engine_music::Op::Object(sym, v),
    };
    lily_engine_music::grob_wrote(arena, Some(id), op, None);
    let mut a = arena.borrow_mut();
    let (alist, slots): (fn(&mut Arena, u32) -> &mut Value, bool) = match store {
        Store::Property => (props, true),
        Store::Object => (objects, false),
    };
    grobs(&mut a)
        .tracker
        .wrote(id, if slots { sym } else { Value::NULL });
    let old = *alist(&mut a, id);
    let found = if slots {
        grobs(&mut a).grobs[id as usize].own.entry(heap, old, sym)
    } else {
        assq(heap, sym, old)
    };
    if let Some(entry) = found {
        heap.set_cdr(entry, v);
        return;
    }
    let entry = heap.cons(sym, v);
    let new = heap.cons(entry, old);
    *alist(&mut a, id) = new;
    if slots {
        grobs(&mut a).grobs[id as usize]
            .own
            .added(old, new, sym, entry);
    }
}

fn props(a: &mut Arena, id: u32) -> &mut Value {
    &mut a.records.grobs[id as usize].props
}

/// Its object alist, read. A read through [`objects`] would copy a row N8's
/// snapshots share (D259), and a translation's last reads go over them all.
fn object_alist(a: &mut Arena, id: u32) -> Value {
    let slot = grobs(a);
    slot.tracker.read(id, Value::NULL);
    slot.grobs[id as usize].objects
}

fn objects(a: &mut Arena, id: u32) -> &mut Value {
    let slot = grobs(a);
    slot.tracker.read(id, Value::NULL);
    &mut slot.grobs[id as usize].objects
}

/// `internal_set_property`: `sym` set to `v` on the grob `id`, unchecked.
pub fn set_property_data(heap: &mut Heap, arena: &RefCell<Arena>, id: u32, sym: Value, v: Value) {
    set_on_alist(heap, arena, id, sym, v, Store::Property);
}

/// `Grob::name`: its definition's `meta` name, or `dead` and its class's,
/// `Grob` for a grob of no class.
fn name(heap: &mut Heap, arena: &RefCell<Arena>, id: u32, grob: Value) -> String {
    let meta = heap.intern("meta");
    let meta = property_data(heap, arena, id, grob, meta);
    let key = heap.intern("name");
    match assq_ref(heap, key, meta) {
        Some(n) if n.tag() == Tag::Symbol => String::from(heap.name(n).unwrap_or_default()),
        _ => {
            let class = grobs(&mut arena.borrow_mut()).grobs[id as usize].class;
            format!("dead {}", class.map_or("Grob", Class::name))
        }
    }
}

/// `name` of the grob `grob`; `Grob` for what is no grob.
pub(crate) fn grob_name(heap: &mut Heap, grob: Value) -> String {
    match grob_of(heap, grob) {
        Some((arena, id)) => name(heap, &arena, id, grob),
        None => String::from("Grob"),
    }
}

/// `add_interface`: the grob `id` of `arena` given the interface `sym`, as
/// its class's constructors give theirs. Past a translation's 256
/// interfaces, none is given.
pub(crate) fn add_interface(arena: &RefCell<Arena>, id: u32, sym: Value) {
    let mut a = arena.borrow_mut();
    if let Some(n) = a.records.interfaces.number(sym) {
        a.records.grobs[id as usize].interfaces.insert(n);
    }
}

/// `has_interface`: whether the grob `grob` has the interface `name`, by
/// its own interfaces: its definition's `meta.interfaces` and its class's
/// (`internal_has_interface`). A dead grob has none.
pub fn has_interface(heap: &mut Heap, grob: Value, name: &str) -> bool {
    let Some((arena, id)) = grob_of(heap, grob) else {
        return false;
    };
    item::classify(heap, &arena, id, grob);
    let sym = heap.intern(name);
    let mut a = arena.borrow_mut();
    let Some(n) = a.records.interfaces.number(sym) else {
        return false;
    };
    a.records.grobs[id as usize].interfaces.contains(n)
}

/// [`has_interface`] of any of the interfaces `syms` (symbols), asked in
/// their order.
pub fn has_any_interface(heap: &mut Heap, grob: Value, syms: &[Value]) -> bool {
    let Some((arena, id)) = grob_of(heap, grob) else {
        return false;
    };
    item::classify(heap, &arena, id, grob);
    let mut a = arena.borrow_mut();
    syms.iter().any(|&sym| {
        a.records
            .interfaces
            .number(sym)
            .is_some_and(|n| a.records.grobs[id as usize].interfaces.contains(n))
    })
}

/// The arena and number of the grob `v`, argument `position` of `subr`
/// (LilyPond's `LY_ASSERT_SMOB`), [`construct`]ed.
pub(crate) fn grob_arg(
    cx: &mut Cx,
    subr: &str,
    position: usize,
    v: Value,
) -> Result<(Rc<RefCell<Arena>>, u32), Thrown> {
    let Some((arena, id)) = grob_of(cx.heap(), v) else {
        return Err(cx.wrong_type(subr, position, v, "Grob"));
    };
    if !is_made(&arena, id) {
        let defaults = [
            cx.global("lily", "%grob-default-extents"),
            cx.global("lily", "%grob-default-skylines"),
        ];
        construct(cx.heap_mut(), &arena, id, v, defaults);
    }
    Ok((arena, id))
}

fn is_made(arena: &RefCell<Arena>, id: u32) -> bool {
    grobs(&mut arena.borrow_mut()).grobs[id as usize].made
}

/// [`construct`] of the grob `grob` the first time layout reaches it, as
/// [`grob_arg`] makes it, its defaults read through `host`.
pub(crate) fn make(host: &mut dyn Host, grob: Value) {
    let Some((arena, id)) = grob_of(host.heap(), grob) else {
        return;
    };
    if !is_made(&arena, id) {
        let defaults = [
            host.lily("%grob-default-extents"),
            host.lily("%grob-default-skylines"),
        ];
        construct(host.heap(), &arena, id, grob, defaults);
    }
}

/// The part of `Grob::Grob (Grob const &)` (`grob.cc:93–112`) layout
/// keeps, of `copy`, a record copied from the grob `src`
/// ([`lily_engine_music::clone_grob`]): `src`'s offsets, extents and
/// parents (`dim_cache_`), `src` as its original, and `src`'s class, made
/// already; no objects and no layout, as a new grob has.
pub(crate) fn copied(heap: &Heap, copy: Value, src: Value) {
    let Some((dims, class)) = with_grob(heap, src, |g| (g.dims, g.class)) else {
        return;
    };
    with_grob_mut(heap, copy, |g| {
        g.dims = dims;
        g.original = Some(src);
        (g.class, g.classified, g.made) = (class, true, true);
    });
    if let (Some((arena, c)), Some((_, s))) = (grob_of(heap, copy), grob_of(heap, src)) {
        grobs(&mut arena.borrow_mut()).tracker.copy(c, s);
    }
}

/// The `Grob` constructor's part (`grob.cc:43–90`) of the grob `id` of
/// `arena`, whose handle is `grob`, the first time layout reaches it: its
/// class and its interfaces (`item.rs`), the object callbacks of its
/// `meta`, then its `X-extent` and `Y-extent`
/// where it has none, `defaults`' first (`%grob-default-extents`):
/// `ly:grob::stencil-width`, and a container of `ly:grob::stencil-height`
/// and `ly:grob::pure-stencil-height`; and its `vertical-skylines` and
/// `horizontal-skylines` where it has none, its second
/// (`%grob-default-skylines`, `skylines.rs`): containers of
/// `ly:grob::simple-vertical-skylines-from-extents` and
/// `ly:grob::simple-horizontal-skylines-from-extents` and their pure forms.
/// LilyPond makes the containers for each grob, the engine once.
fn construct(
    heap: &mut Heap,
    arena: &RefCell<Arena>,
    id: u32,
    grob: Value,
    defaults: [Option<Value>; 2],
) {
    grobs(&mut arena.borrow_mut()).grobs[id as usize].made = true;
    item::classify(heap, arena, id, grob);
    let meta = heap.intern("meta");
    let meta = property_data(heap, arena, id, grob, meta);
    let key = heap.intern("object-callbacks");
    let mut p = assq_ref(heap, key, meta).unwrap_or(Value::NULL);
    while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if let (Some(sym), Some(v)) = (heap.car(entry), heap.cdr(entry)) {
            set_on_alist(heap, arena, id, sym, v, Store::Object);
        }
        p = rest;
    }
    let names = [
        ["X-extent", "Y-extent"],
        ["vertical-skylines", "horizontal-skylines"],
    ];
    for (names, defaults) in names.into_iter().zip(defaults) {
        let Some((first, second)) = defaults.and_then(|d| heap.car(d).zip(heap.cdr(d))) else {
            continue;
        };
        for (name, v) in names.into_iter().zip([first, second]) {
            let sym = heap.intern(name);
            if property_data(heap, arena, id, grob, sym) == Value::NULL {
                set_property_data(heap, arena, id, sym, v);
            }
        }
    }
}

fn symbol_arg(cx: &mut Cx, subr: &str, v: Value) -> Result<(), Thrown> {
    if v.tag() == Tag::Symbol {
        Ok(())
    } else {
        Err(cx.wrong_type(subr, 2, v, "symbol"))
    }
}

/// `value`, or `default` for `'()`, as `ly:grob-property` and
/// `ly:grob-object` give it; a procedure is called by
/// `%grob-try-callback`, which keeps its value with `store`.
fn called(cx: &mut Cx, grob: Value, sym: Value, value: Value, default: Value, store: &str) -> R {
    if cx.heap().is_procedure(value) {
        let (Some(call), Some(store)) = (
            cx.global("lily", "%grob-try-callback"),
            cx.global("lily", store),
        ) else {
            return Ok(value);
        };
        return Ok(cx.tail_call(call, &[store, grob, sym, value, default]));
    }
    Ok(if value == Value::NULL { default } else { value })
}

/// `(ly:grob-property grob sym [val])`: `internal_get_property`, `val` or
/// `'()` for `'()`. `calculation-in-progress` is LilyPond's programming
/// error and is given as it is.
pub(crate) fn ly_grob_property(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-property";
    let (arena, id) = grob_arg(cx, SUBR, 1, args[0])?;
    symbol_arg(cx, SUBR, args[1])?;
    let (grob, sym) = (args[0], args[1]);
    let default = args.get(2).copied().unwrap_or(Value::NULL);
    let heap = cx.heap_mut();
    let mut value = property_data(heap, &arena, id, grob, sym);
    if value.tag() == Tag::Symbol && heap.name(value) == Some("calculation-in-progress") {
        let text = format!(
            "cyclic dependency: calculation-in-progress encountered for {}.{}",
            name(heap, &arena, id, grob),
            heap.name(sym).unwrap_or_default()
        );
        return reported(cx, grob, "programming-error", value, &[text]);
    }
    if let Some(unpure) = unpure_of(heap, value) {
        value = unpure;
    }
    called(cx, grob, sym, value, default, "%grob-set-property-data!")
}

/// `(%grob-set-property-data! grob sym val)`: `internal_set_property`, no
/// type checked; what `ly:grob-set-property!` sets once the type is.
pub(crate) fn set_property_data_prim(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-set-property-data!", 1, args[0])?;
    set_property_data(cx.heap_mut(), &arena, id, args[1], args[2]);
    Ok(Value::UNSPECIFIED)
}

/// `(ly:grob-object grob sym [val])`: `internal_get_object`, `val` or `'()`
/// for `'()`.
pub(crate) fn ly_grob_object(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-object";
    let (arena, id) = grob_arg(cx, SUBR, 1, args[0])?;
    symbol_arg(cx, SUBR, args[1])?;
    let default = args.get(2).copied().unwrap_or(Value::NULL);
    let alist = object_alist(&mut arena.borrow_mut(), id);
    let value = assq_ref(cx.heap(), args[1], alist).unwrap_or(Value::NULL);
    called(cx, args[0], args[1], value, default, "ly:grob-set-object!")
}

/// The data of the object `sym` of the grob `grob`, as it is set, or
/// `'()`: no callback is called.
pub(crate) fn object_data(heap: &mut Heap, grob: Value, sym: Value) -> Value {
    let Some((arena, id)) = grob_of(heap, grob) else {
        return Value::NULL;
    };
    let alist = object_alist(&mut arena.borrow_mut(), id);
    assq_ref(heap, sym, alist).unwrap_or(Value::NULL)
}

/// `set_property` (`internal_set_property`): `name` set to `v` on the
/// grob `grob`, unchecked.
pub fn set_property(heap: &mut Heap, grob: Value, name: &str, v: Value) {
    if let Some((arena, id)) = grob_of(heap, grob) {
        let sym = heap.intern(name);
        set_property_data(heap, &arena, id, sym, v);
    }
}

/// `internal_set_object`: `sym` set to `v` in the object alist of the grob
/// `grob`, if it is a live grob.
pub fn set_object(heap: &mut Heap, grob: Value, sym: Value, v: Value) {
    if let Some((arena, id)) = grob_of(heap, grob) {
        set_on_alist(heap, &arena, id, sym, v, Store::Object);
    }
}

/// `(ly:grob-set-object! grob sym val)`: `internal_set_object`.
pub(crate) fn ly_grob_set_object(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-set-object!";
    let (arena, id) = grob_arg(cx, SUBR, 1, args[0])?;
    symbol_arg(cx, SUBR, args[1])?;
    set_on_alist(cx.heap_mut(), &arena, id, args[1], args[2], Store::Object);
    Ok(Value::UNSPECIFIED)
}

// --- the callbacks' chains (grob-closure.cc) -------------------------------

/// `axis_offset_symbol`: `X-offset` or `Y-offset`.
pub fn axis_offset_symbol(heap: &mut Heap, a: Axis) -> Value {
    heap.intern(if a == X { "X-offset" } else { "Y-offset" })
}

/// The arena and number of the grob `grob`, [`construct`]ed, for a
/// [`Host`].
pub(crate) fn host_grob(host: &mut dyn Host, grob: Value) -> Option<(Rc<RefCell<Arena>>, u32)> {
    let (arena, id) = grob_of(host.heap(), grob)?;
    if !is_made(&arena, id) {
        let defaults = [
            host.lily("%grob-default-extents"),
            host.lily("%grob-default-skylines"),
        ];
        construct(host.heap(), &arena, id, grob, defaults);
    }
    Some((arena, id))
}

/// `sym`'s data in the grob `grob` replaced by what `(lily)`'s `compose`
/// makes of `proc` and it, unchecked (`set_property`).
fn compose(
    host: &mut dyn Host,
    compose: &str,
    grob: Value,
    proc: Value,
    sym: Value,
) -> Result<(), Exception> {
    let f = crate::lily_procedure(host, compose)?;
    let Some((arena, id)) = host_grob(host, grob) else {
        return Ok(());
    };
    let data = property_data(host.heap(), &arena, id, grob, sym);
    let composed = host.call(f, &[proc, data])?;
    set_property_data(host.heap(), &arena, id, sym, composed);
    Ok(())
}

/// `add_offset_callback`: the grob's offset on `a` made `(+ (proc grob)
/// (data grob))`, or `proc` for no data (`grob::offset-function`).
pub fn add_offset_callback(
    host: &mut dyn Host,
    grob: Value,
    proc: Value,
    a: Axis,
) -> Result<(), Exception> {
    let sym = axis_offset_symbol(host.heap(), a);
    compose(host, "grob::offset-function", grob, proc, sym)
}

/// `chain_callback`: `sym`'s data in the grob made `(proc grob (data
/// grob))` (`grob::compose-function`).
pub fn chain_callback(
    host: &mut dyn Host,
    grob: Value,
    proc: Value,
    sym: Value,
) -> Result<(), Exception> {
    compose(host, "grob::compose-function", grob, proc, sym)
}

/// `chain_offset_callback`: [`chain_callback`] of the offset on `a`.
pub fn chain_offset_callback(
    host: &mut dyn Host,
    grob: Value,
    proc: Value,
    a: Axis,
) -> Result<(), Exception> {
    let sym = axis_offset_symbol(host.heap(), a);
    chain_callback(host, grob, proc, sym)
}

// --- parents, offsets and extents ------------------------------------------

/// The layout state of the grob `grob`'s dimension cache on `a`, if it is
/// a grob.
fn dim(heap: &Heap, grob: Value, a: Axis) -> Option<Dim> {
    let (arena, id) = grob_of(heap, grob)?;
    let mut ar = arena.borrow_mut();
    Some(grobs(&mut ar).grobs[id as usize].dims[a])
}

/// What `f` makes of the grob `id`'s dimension cache on `a`.
pub(crate) fn with_dim<T>(
    arena: &RefCell<Arena>,
    id: u32,
    a: Axis,
    f: impl FnOnce(&mut Dim) -> T,
) -> T {
    let mut borrowed = arena.borrow_mut();
    let slot = grobs(&mut borrowed);
    slot.tracker.read(id, Value::NULL);
    f(&mut slot.grobs[id as usize].dims[a])
}

/// `get_parent`: the handle of the grob's parent on `a`.
pub fn get_parent(heap: &Heap, grob: Value, a: Axis) -> Option<Value> {
    dim(heap, grob, a)?.parent
}

/// `set_parent`: `parent`, a grob's handle, the grob's parent on `a`.
pub fn set_parent(heap: &Heap, grob: Value, parent: Value, a: Axis) {
    if let Some((arena, id)) = grob_of(heap, grob) {
        let op = lily_engine_music::Op::Parent(a as u8, parent);
        lily_engine_music::grob_wrote(&arena, Some(id), op, None);
        with_dim(&arena, id, a, |d| d.parent = Some(parent));
    }
}

/// `set_parent` as `fixup_refpoint` calls it: `parent`, or no parent,
/// the grob's parent on `a`.
fn set_parent_or_none(heap: &Heap, grob: Value, parent: Option<Value>, a: Axis) {
    if let Some((arena, id)) = grob_of(heap, grob) {
        let op = lily_engine_music::Op::Parent(a as u8, parent.unwrap_or(Value::FALSE));
        lily_engine_music::grob_wrote(&arena, Some(id), op, None);
        with_dim(&arena, id, a, |d| d.parent = parent);
    }
}

/// `find_broken_piece` (`Grob`'s, `Item`'s or `Spanner`'s): the grob's
/// piece on the system `l`, if it has one; none for a grob of no class.
pub(crate) fn find_broken_piece(heap: &mut Heap, grob: Value, l: Value) -> Option<Value> {
    match item::class(heap, grob)? {
        Class::Item | Class::PaperColumn => item::find_broken_piece(heap, grob, l),
        Class::Spanner | Class::System => crate::spanner::find_broken_piece(heap, grob, l),
    }
}

/// `fixup_refpoint` (`grob.cc:623–654`): on each axis, a parent on
/// another system than the grob's, if the grob has one, replaced by the
/// parent's piece on the grob's (none if it has none); then, for an item
/// whose parent was an item broken on another side than the grob is, by
/// that parent's piece on the grob's side (none if it has none).
pub fn fixup_refpoint(heap: &mut Heap, me: Value) {
    for a in [X, Y] {
        let Some(parent) = get_parent(heap, me, a) else {
            continue;
        };
        let system = item::get_system(heap, me);
        if item::get_system(heap, parent) != system
            && let Some(l) = system
        {
            let newparent = find_broken_piece(heap, parent, l);
            set_parent_or_none(heap, me, newparent, a);
        }
        if item::is_item(heap, me) && item::is_item(heap, parent) {
            let my_dir = item::break_status_dir(heap, me);
            if my_dir != item::break_status_dir(heap, parent) {
                let newparent = item::find_prebroken_piece(heap, parent, my_dir);
                set_parent_or_none(heap, me, newparent, a);
            }
        }
    }
}

/// `do_break_processing`: a spanner's ([`crate::spanner::do_break_processing`]);
/// `Grob`'s, an item's, does nothing (`grob.cc:247–250`).
pub fn do_break_processing(host: &mut dyn Host, me: Value) -> Result<(), Exception> {
    if crate::spanner::is_spanner(host.heap(), me) {
        crate::spanner::do_break_processing(host, me)?;
    }
    Ok(())
}

/// `handle_broken_dependencies` (`grob.cc:275–314`), after line breaking:
/// a spanner without both bounds killed; an original spanner's objects
/// given to its pieces, each grob in them substituted by its piece on the
/// piece's system (`substitute_one_mutable_property`,
/// `break_substitution.rs`); then a live grob on a system that is its
/// ancestor on both axes takes its objects substituted by that system
/// (`substitute_object_links`), and any other is killed, its links
/// already moved. A piece of a spanner keeps its objects, given by its
/// original's.
pub fn handle_broken_dependencies(host: &mut dyn Host, me: Value) -> Result<(), Exception> {
    let sp = crate::spanner::is_spanner(host.heap(), me);
    if sp
        && (crate::spanner::get_bound(host, me, -1)?.is_none()
            || crate::spanner::get_bound(host, me, 1)?.is_none())
    {
        suicide(host.heap(), me);
        return Ok(());
    }
    let heap = host.heap();
    let (original, objects) =
        with_grob(heap, me, |g| (g.original, g.objects)).unwrap_or((None, Value::NULL));
    if original.is_some() && sp {
        return Ok(());
    }
    if sp {
        let mut s = objects;
        while let (Some(entry), Some(rest)) = (heap.car(s), heap.cdr(s)) {
            if let (Some(sym), Some(val)) = (heap.car(entry), heap.cdr(entry)) {
                break_substitution::substitute_one_mutable_property(heap, me, sym, val);
            }
            s = rest;
        }
    }
    let system = item::get_system(heap, me);
    match system {
        Some(system)
            if is_live_grob(heap, me)
                && common_refpoint(heap, me, system, X).is_some()
                && common_refpoint(heap, me, system, Y).is_some() =>
        {
            let crit = break_substitution::Criterion::System(system);
            break_substitution::substitute_object_links(heap, me, crit, objects);
        }
        // THIS element is `invalid'; it has been removed from all
        // dependencies, so let's junk the element itself.
        _ => suicide(heap, me),
    }
    Ok(())
}

/// `has_in_ancestry`: `ancestor` is the grob, or its parent on `a`, or
/// theirs.
pub fn has_in_ancestry(heap: &Heap, grob: Value, ancestor: Value, a: Axis) -> bool {
    let mut p = Some(grob);
    while let Some(g) = p {
        if g == ancestor {
            return true;
        }
        p = get_parent(heap, g, a);
    }
    false
}

/// `check_cross_staff` (`grob.cc:1018–1030`, borrowed for the tuplet
/// bracket, MS4 step 12): whether `commony` or a grob between `me` and it
/// on Y is a vertical alignment.
pub fn check_cross_staff(heap: &mut Heap, me: Value, commony: Value) -> bool {
    if has_interface(heap, commony, "align-interface") {
        return true;
    }

    let mut g = Some(me);
    while let Some(c) = g
        && c != commony
    {
        if has_interface(heap, c, "align-interface") {
            return true;
        }
        g = get_parent(heap, c, Y);
    }

    false
}

/// `common_refpoint`: the nearest grob on `a` that is `grob` or one of its
/// ancestors and `other` or one of its.
pub fn common_refpoint(heap: &Heap, grob: Value, other: Value, a: Axis) -> Option<Value> {
    let up = |g: Option<Value>| g.and_then(|g| get_parent(heap, g, a));
    let mut balance = 0i32;
    let mut c = Some(grob);
    while c.is_some() {
        c = up(c);
        balance += 1;
    }
    let mut d = Some(other);
    while d.is_some() {
        d = up(d);
        balance -= 1;
    }
    // Cut down ancestry to same size.
    let (mut c, mut d) = (Some(grob), Some(other));
    while balance > 0 {
        c = up(c);
        balance -= 1;
    }
    while balance < 0 {
        d = up(d);
        balance += 1;
    }
    // Now find point where our lineages converge.
    while c != d {
        c = up(c);
        d = up(d);
    }
    c
}

/// LilyPond's programming error when [`translate_axis`] is given an
/// infinite distance, or NaN.
pub const INFINITE_TRANSLATION: &str = "Infinity or NaN encountered";

/// `translate_axis`: the grob moved `y` on `a`, its offset made `y` if it
/// has none yet; for an infinite `y`, or NaN, not moved and
/// [`INFINITE_TRANSLATION`].
pub fn translate_axis(heap: &Heap, grob: Value, y: f64, a: Axis) -> Result<(), &'static str> {
    if !y.is_finite() {
        return Err(INFINITE_TRANSLATION);
    }
    if let Some((arena, id)) = grob_of(heap, grob) {
        // A translator's move (the mensural ligature's, MS8 step 9): a
        // write N8's partitions note, which replay cannot make again.
        lily_engine_music::grob_wrote(&arena, Some(id), lily_engine_music::Op::Unknown, None);
        with_dim(&arena, id, a, |d| *d.offset.get_or_insert(0.0) += y);
        grobs(&mut arena.borrow_mut())
            .tracker
            .wrote(id, Value::NULL);
    }
    Ok(())
}

/// `flush_extent_cache`: if the grob keeps an extent on `a`, its
/// `X-extent` or `Y-extent` property removed, the extent no longer kept,
/// and its parent's flushed too.
pub fn flush_extent_cache(heap: &mut Heap, grob: Value, a: Axis) {
    let Some((arena, id)) = grob_of(heap, grob) else {
        return;
    };
    if with_dim(&arena, id, a, |d| d.extent.is_none()) {
        return;
    }
    let sym = heap.intern(if a == X { "X-extent" } else { "Y-extent" });
    del_property(heap, &arena, id, sym);
    let parent = with_dim(&arena, id, a, |d| {
        d.extent = None;
        d.parent
    });
    if let Some(parent) = parent {
        flush_extent_cache(heap, parent, a);
    }
}

/// The arena and number of the grob `grob`, [`construct`]ed, else the
/// type error of `subr`'s first argument.
pub(crate) fn host_grob_arg(
    host: &mut dyn Host,
    subr: &str,
    grob: Value,
) -> Result<(Rc<RefCell<Arena>>, u32), Exception> {
    match host_grob(host, grob) {
        Some(g) => Ok(g),
        None => Err(crate::wrong_type(host.heap(), Some(subr), 1, grob, "Grob")),
    }
}

/// `get_offset` (`grob.cc:338–360`): the grob's offset on `a`, its offset
/// property read the first time, while the offset is 0, and added to
/// what callbacks translated it by meanwhile; the property then given up
/// (`del_property`), the offset kept.
pub fn get_offset(host: &mut dyn Host, grob: Value, a: Axis) -> Result<f64, Exception> {
    let (arena, id) = host_grob_arg(host, "%grob-offset-begin", grob)?;
    let dumping = {
        let mut borrowed = arena.borrow_mut();
        let slot = grobs(&mut borrowed);
        if a == Y {
            slot.tracker.read_y(id);
        }
        slot.dumping > 0
    };
    if dumping {
        let f = crate::lily_procedure(host, "%grob-offset")?;
        let v = host.call(f, &[grob, Value::fixnum(a as i64).unwrap_or(Value::NULL)])?;
        return Ok(host.heap().to_f64(v).unwrap_or(0.0));
    }
    let cached = with_dim(&arena, id, a, |d| {
        let cached = d.offset;
        d.offset.get_or_insert(0.0);
        cached
    });
    if let Some(offset) = cached {
        return Ok(offset);
    }
    let sym = axis_offset_symbol(host.heap(), a);
    let v = get_property(host, grob, sym)?;
    let heap = host.heap();
    let off = if heap.is_real(v) {
        heap.to_f64(v).unwrap_or(0.0)
    } else {
        0.0
    };
    let Some(offset) = with_dim(&arena, id, a, |d| {
        d.offset.as_mut().map(|o| {
            *o += off;
            *o
        })
    }) else {
        return Ok(0.0);
    };
    del_property(heap, &arena, id, sym);
    Ok(offset)
}

/// `relative_coordinate` (`grob.cc:316–336`): the sum of the offsets on `a`
/// from the grob up to `refp`, a grob's handle (or anything else, up to
/// its root), each read by [`get_offset`] before the parent is.
pub fn relative_coordinate(
    host: &mut dyn Host,
    grob: Value,
    refp: Value,
    a: Axis,
) -> Result<f64, Exception> {
    let (mut ancestor, mut result) = (grob, 0.0);
    while ancestor != refp && ancestor != Value::NULL {
        let offset = get_offset(host, ancestor, a)?;
        ancestor = get_parent(host.heap(), ancestor, a).unwrap_or(Value::NULL);
        result += offset;
    }
    Ok(result)
}

/// `extent` (`grob.cc:362–403`): the grob's extent on `a` relative to
/// `refp`: its [`relative_coordinate`] first, then its extent property
/// united with its minimum, each a number pair or nothing, read the first
/// time and kept; translated by the coordinate, which an infinite one is
/// not, with LilyPond's warning.
pub fn extent(
    host: &mut dyn Host,
    grob: Value,
    refp: Value,
    a: Axis,
) -> Result<Interval, Exception> {
    let offset = relative_coordinate(host, grob, refp, a)?;
    let (arena, id) = host_grob_arg(host, "%grob-extent-at", grob)?;
    let mut ext = match with_dim(&arena, id, a, |d| d.extent) {
        Some(ext) => ext,
        None => {
            let names = if a == X {
                ["X-extent", "minimum-X-extent"]
            } else {
                ["Y-extent", "minimum-Y-extent"]
            };
            let extent = property(host, grob, names[0])?;
            let minimum = property(host, grob, names[1])?;
            let mut real = Interval::EMPTY;
            for v in [extent, minimum] {
                if let Some(ext) = number_pair(host.heap(), v) {
                    real.unite(ext);
                }
            }
            with_dim(&arena, id, a, |d| d.extent = Some(real));
            real
        }
    };
    if offset.is_infinite() {
        let text = format!("ignored infinite {}-offset", if a == X { "X" } else { "Y" });
        warning(host, grob, &text)?;
    } else {
        ext.translate(offset);
    }
    Ok(ext)
}

/// `(ly:grob-parent grob axis [def])`: its parent, else `def` or `'()`.
pub(crate) fn ly_grob_parent(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-parent";
    grob_arg(cx, SUBR, 1, args[0])?;
    let a = crate::axis_arg(cx, SUBR, 2, args[1])?;
    let default = args.get(2).copied().unwrap_or(Value::NULL);
    Ok(get_parent(cx.heap(), args[0], a).unwrap_or(default))
}

/// `(ly:grob-set-parent! grob axis parent-grob)`.
pub(crate) fn ly_grob_set_parent(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-set-parent!";
    grob_arg(cx, SUBR, 1, args[0])?;
    let a = crate::axis_arg(cx, SUBR, 2, args[1])?;
    grob_arg(cx, SUBR, 3, args[2])?;
    set_parent(cx.heap(), args[0], args[2], a);
    Ok(Value::UNSPECIFIED)
}

/// `(ly:grob-translate-axis! grob d a)`.
pub(crate) fn ly_grob_translate_axis(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-translate-axis!";
    grob_arg(cx, SUBR, 1, args[0])?;
    let y = crate::number_arg(cx, SUBR, 2, args[1])?;
    let a = crate::axis_arg(cx, SUBR, 3, args[2])?;
    match translate_axis(cx.heap(), args[0], y, a) {
        Ok(()) => Ok(Value::UNSPECIFIED),
        Err(error) => reported(
            cx,
            args[0],
            "programming-error",
            Value::UNSPECIFIED,
            &[error.into()],
        ),
    }
}

/// `(ly:grob-common-refpoint grob other axis)`: their common refpoint, or
/// `#f`.
pub(crate) fn ly_grob_common_refpoint(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-common-refpoint";
    grob_arg(cx, SUBR, 1, args[0])?;
    grob_arg(cx, SUBR, 2, args[1])?;
    let a = crate::axis_arg(cx, SUBR, 3, args[2])?;
    Ok(common_refpoint(cx.heap(), args[0], args[1], a).unwrap_or(Value::FALSE))
}

/// `(%grob-check-refp subr grob refp axis)`: the arguments of `subr`
/// checked, and `check_refp`: `refp` must be `grob` or an ancestor of it.
pub(crate) fn check_refp(cx: &mut Cx, args: &[Value]) -> R {
    let mut subr = String::new();
    cx.heap().display(args[0], &mut subr);
    let (arena, id) = grob_arg(cx, &subr, 1, args[1])?;
    let (refp_arena, refp_id) = grob_arg(cx, &subr, 2, args[2])?;
    let a = crate::axis_arg(cx, &subr, 3, args[3])?;
    if has_in_ancestry(cx.heap(), args[1], args[2], a) {
        return Ok(Value::UNSPECIFIED);
    }
    let heap = cx.heap_mut();
    let refp_name = name(heap, &refp_arena, refp_id, args[2]);
    let grob_name = name(heap, &arena, id, args[1]);
    let items = [
        heap.string(refp_name),
        heap.string(grob_name),
        heap.string(if a == X { "X" } else { "Y" }),
    ];
    Err(cx.error(
        "misc-error",
        Some(&subr),
        "grob ~a is an invalid refpoint for ~a on ~a axis.\nThe refpoint should be an ancestor (direct or indirect) of the grob.",
        &items,
    ))
}

/// `(%grob-offset-begin grob a)`: `get_offset`'s cached offset, or `#f`
/// and the offset made 0 while the offset property is read.
pub(crate) fn offset_begin(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-offset-begin", 1, args[0])?;
    let a = crate::axis_arg(cx, "%grob-offset-begin", 2, args[1])?;
    let cached = with_dim(&arena, id, a, |d| {
        let cached = d.offset;
        d.offset.get_or_insert(0.0);
        cached
    });
    match cached {
        Some(offset) => Ok(cx.heap_mut().flonum(offset)),
        None => Ok(Value::FALSE),
    }
}

/// `(%grob-offset-end grob a value)`: the rest of `get_offset` with
/// `value`, the offset property read: added to the offset, if one is
/// still kept, which the property then gives up (`del_property`); else 0.
pub(crate) fn offset_end(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-offset-end", 1, args[0])?;
    let a = crate::axis_arg(cx, "%grob-offset-end", 2, args[1])?;
    let heap = cx.heap_mut();
    let off = if heap.is_real(args[2]) {
        heap.to_f64(args[2]).unwrap_or(0.0)
    } else {
        0.0
    };
    let Some(offset) = with_dim(&arena, id, a, |d| {
        d.offset.as_mut().map(|o| {
            *o += off;
            *o
        })
    }) else {
        return Ok(heap.flonum(0.0));
    };
    let sym = axis_offset_symbol(heap, a);
    del_property(heap, &arena, id, sym);
    Ok(heap.flonum(offset))
}

/// `internal_del_property`: the first entry of `sym` set on the grob
/// removed (`scm_assq_remove_x`).
pub(crate) fn del_property(heap: &mut Heap, arena: &RefCell<Arena>, id: u32, sym: Value) {
    let old = *props(&mut arena.borrow_mut(), id);
    let (mut prev, mut p) = (None, old);
    while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if heap.car(entry) == Some(sym) {
            let new = match prev {
                Some(pair) => {
                    heap.set_cdr(pair, rest);
                    old
                }
                None => rest,
            };
            let mut a = arena.borrow_mut();
            *props(&mut a, id) = new;
            let slot = grobs(&mut a);
            slot.tracker.removed(id, sym);
            slot.grobs[id as usize]
                .own
                .removed(heap, old, new, sym, rest);
            return;
        }
        (prev, p) = (Some(p), rest);
    }
}

/// `is_number_pair`, then `from_scm<Interval>`.
pub fn number_pair(heap: &Heap, v: Value) -> Option<Interval> {
    let (l, r) = (heap.car(v)?, heap.cdr(v)?);
    if !heap.is_number(l) || !heap.is_number(r) {
        return None;
    }
    Some(Interval {
        left: heap.to_f64(l)?,
        right: heap.to_f64(r)?,
    })
}

/// `(%grob-cache-extent! grob a extent minimum)`: what `extent` keeps of
/// the grob's extent property and its minimum, read in that order, each
/// a number pair or nothing.
pub(crate) fn cache_extent(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-cache-extent!", 1, args[0])?;
    let a = crate::axis_arg(cx, "%grob-cache-extent!", 2, args[1])?;
    let mut real = Interval::EMPTY;
    for v in [args[2], args[3]] {
        if let Some(ext) = number_pair(cx.heap(), v) {
            real.unite(ext);
        }
    }
    with_dim(&arena, id, a, |d| d.extent = Some(real));
    Ok(Value::UNSPECIFIED)
}

/// `(%grob-extent-at grob a offset)`: the grob's extent on `a`, if kept,
/// translated by `offset`, which an infinite one is not, with LilyPond's
/// warning; `#f` if none is kept.
pub(crate) fn extent_at(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-extent-at", 1, args[0])?;
    let a = crate::axis_arg(cx, "%grob-extent-at", 2, args[1])?;
    let Some(mut ext) = with_dim(&arena, id, a, |d| d.extent) else {
        return Ok(Value::FALSE);
    };
    let offset = crate::to_double(cx, args[2])?;
    let mut warnings = Vec::new();
    if offset.is_infinite() {
        warnings.push(format!(
            "ignored infinite {}-offset",
            if a == X { "X" } else { "Y" }
        ));
    } else {
        ext.translate(offset);
    }
    let heap = cx.heap_mut();
    let (l, r) = (heap.flonum(ext.left), heap.flonum(ext.right));
    let v = heap.cons(l, r);
    reported(cx, args[0], "warning", v, &warnings)
}

/// `(%grob-property-data grob sym)`: `get_property_data`, no callback
/// called.
pub(crate) fn property_data_prim(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-property-data", 1, args[0])?;
    Ok(property_data(cx.heap_mut(), &arena, id, args[0], args[1]))
}

/// `(%grob-live? grob)`: `is_live`.
pub(crate) fn is_live_prim(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-live?", 1, args[0])?;
    Ok(Value::bool(is_live(&arena, id)))
}

/// `(%grob-name grob)`: `name`, as a string.
pub(crate) fn name_prim(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-name", 1, args[0])?;
    let heap = cx.heap_mut();
    let n = name(heap, &arena, id, args[0]);
    Ok(heap.string(&n))
}

// --- the origin and the grob's warnings (grob.cc:775–797) -----------------

/// `ultimate_event_cause`: the grob's `cause`, or its cause's through grob
/// causes, if that is a stream event. The causes' data, as a primitive
/// reads them: no callback is called.
pub fn ultimate_event_cause(heap: &mut Heap, grob: Value) -> Option<Value> {
    let sym = heap.intern("cause");
    let mut cause = grob;
    while let Some((arena, id)) = grob_of(heap, cause) {
        cause = property_data(heap, &arena, id, cause, sym);
    }
    heap.foreign::<StreamEvent>(cause).map(|_| cause)
}

/// `Grob::origin`: the input location of the grob's ultimate event cause
/// (`Stream_event::origin`), where its warnings are given; `None` for none.
pub fn origin(heap: &mut Heap, grob: Value) -> Option<Value> {
    let ev = ultimate_event_cause(heap, grob)?;
    let origin = event_property(heap, ev, "origin");
    heap.foreign::<Input>(origin).map(|_| origin)
}

/// `value`, after the grob's messages `texts` of `kind` (`warning` or
/// `programming-error`; `Grob::warning`, `Grob::programming_error`): at
/// its origin by `%diagnose`, else as `ly:warning` and
/// `ly:programming-error` give them.
pub(crate) fn reported(cx: &mut Cx, grob: Value, kind: &str, value: Value, texts: &[String]) -> R {
    if texts.is_empty() {
        return Ok(value);
    }
    let Some(location) = origin(cx.heap_mut(), grob) else {
        return if kind == "warning" {
            crate::warned(cx, value, texts)
        } else {
            crate::programming_errored(cx, value, texts)
        };
    };
    let Some(proc) = cx.global("lily", "%grob-reported") else {
        return Ok(value);
    };
    let heap = cx.heap_mut();
    let kind = heap.intern(kind);
    let texts: Vec<Value> = texts.iter().map(|t| heap.string(t.as_str())).collect();
    let texts = heap.list(&texts);
    Ok(cx.tail_call(proc, &[kind, location, texts, value]))
}

/// The grob's message `text` of `kind` from Rust, as [`reported`] gives
/// it.
fn diagnose(host: &mut dyn Host, grob: Value, kind: &str, text: &str) -> Result<(), Exception> {
    let Some(location) = origin(host.heap(), grob) else {
        let name = if kind == "warning" {
            "ly:warning"
        } else {
            "ly:programming-error"
        };
        return unlocated(host, name, text);
    };
    let proc = crate::lily_procedure(host, "%diagnose")?;
    let heap = host.heap();
    let kind = heap.intern(kind);
    let text = heap.string(text);
    host.call(proc, &[kind, location, text]).map(|_| ())
}

/// `text` given by `(lily)`'s `name`, `ly:warning` or
/// `ly:programming-error`, at no location.
fn unlocated(host: &mut dyn Host, name: &str, text: &str) -> Result<(), Exception> {
    let proc = crate::lily_procedure(host, name)?;
    let text = host.heap().string(text.replace('~', "~~"));
    host.call(proc, &[text]).map(|_| ())
}

/// `programming_error` (`warn.cc`), not a grob's: `text` given at no
/// location.
pub fn unlocated_programming_error(host: &mut dyn Host, text: &str) -> Result<(), Exception> {
    unlocated(host, "ly:programming-error", text)
}

/// `warning` (`warn.cc`), not a grob's: `text` given at no location.
pub fn unlocated_warning(host: &mut dyn Host, text: &str) -> Result<(), Exception> {
    unlocated(host, "ly:warning", text)
}

/// `Grob::warning`: `text` given at the grob's origin.
pub fn warning(host: &mut dyn Host, grob: Value, text: &str) -> Result<(), Exception> {
    diagnose(host, grob, "warning", text)
}

/// `Grob::programming_error`: `text` given at the grob's origin.
pub fn programming_error(host: &mut dyn Host, grob: Value, text: &str) -> Result<(), Exception> {
    diagnose(host, grob, "programming-error", text)
}

// --- suicide and the print stencil (grob.cc:131–236, :321–338) -------------

/// `suicide`: a live grob emptied but for its cause: its offsets and
/// extents forgotten, not its parents; what was set on it but its cause,
/// its objects, its definition, which makes it dead (`is_live`), and its
/// interfaces. The cause `Engraver::announce_grob` set is the record's,
/// which [`property_data`] gives on.
pub fn suicide(heap: &mut Heap, grob: Value) {
    let Some((arena, id)) = grob_of(heap, grob) else {
        return;
    };
    if !is_live(&arena, id) {
        return;
    }
    lily_engine_music::grob_wrote(&arena, Some(id), lily_engine_music::Op::Suicide, None);
    // Its class outlives it, as LilyPond's object does.
    item::classify(heap, &arena, id, grob);
    let sym = heap.intern("cause");
    let props = arena.borrow().records.grobs[id as usize].props;
    let kept = match assq(heap, sym, props) {
        Some(entry) => heap.list(&[entry]),
        None => Value::NULL,
    };
    let mut a = arena.borrow_mut();
    let g = &mut grobs(&mut a).grobs[id as usize];
    for d in &mut g.dims {
        (d.offset, d.extent) = (None, None);
    }
    g.objects = Value::NULL;
    let record = &mut a.records.grobs[id as usize];
    record.props = kept;
    record.definition = Value::NULL;
    record.interfaces = InterfaceSet::default();
}

/// `(ly:grob-suicide! grob)` (`grob-scheme.cc:372–381`): [`suicide`].
pub(crate) fn ly_grob_suicide(cx: &mut Cx, args: &[Value]) -> R {
    grob_arg(cx, "ly:grob-suicide!", 1, args[0])?;
    suicide(cx.heap_mut(), args[0]);
    Ok(Value::UNSPECIFIED)
}

/// `get_property`: what `(ly:grob-property grob 'name)` gives, read here
/// as LilyPond's C++ calls `internal_get_property` ([`get_property`]).
pub fn property(host: &mut dyn Host, grob: Value, name: &str) -> Result<Value, Exception> {
    let sym = host.heap().intern(name);
    get_property(host, grob, sym)
}

/// `internal_get_property` (`grob-property.cc:262–290`) of `sym`, a symbol,
/// in the grob `grob`: [`ly_grob_property`]'s value, its callback called
/// by [`try_callback`]. Through the image when it reports (a cycle's
/// `calculation-in-progress`), or `grob` is no grob.
pub(crate) fn get_property(
    host: &mut dyn Host,
    grob: Value,
    sym: Value,
) -> Result<Value, Exception> {
    if let Some((arena, id)) = host_grob(host, grob) {
        let heap = host.heap();
        let mut value = property_data(heap, &arena, id, grob, sym);
        if value.tag() != Tag::Symbol || heap.name(value) != Some("calculation-in-progress") {
            if let Some(unpure) = unpure_of(heap, value) {
                value = unpure;
            }
            if !heap.is_procedure(value) {
                return Ok(value);
            }
            return try_callback(host, &arena, id, grob, sym, value, Store::Property);
        }
    }
    let f = crate::lily_procedure(host, "ly:grob-property")?;
    host.call(f, &[grob, sym])
}

/// `try_callback_on_alist` (`grob-property.cc:222–260`), [`PRELUDE`]'s
/// `%grob-try-callback` in Rust: `calculation-in-progress` set as `sym`'s
/// value in the grob's alist that `store` names while `proc` runs on the
/// grob, then what it gives. `proc` is run by
/// its Rust function where the callback table has it (`callbacks.rs`),
/// through the image else.
///
/// While the dump reads (`dump.rs`' `%dump-read`, [`Grobs::dumping`]),
/// `(lily)`'s `%grob-try-callback` is called in its place, as the
/// primitives call it, so that the dump's own keeps what it undoes.
fn try_callback(
    host: &mut dyn Host,
    arena: &RefCell<Arena>,
    id: u32,
    grob: Value,
    sym: Value,
    proc: Value,
    store: Store,
) -> Result<Value, Exception> {
    let foreign = proc.tag() == Tag::Closure && !host.heap().is_image_code(proc);
    let mark = crate::cell::said_mark(host, arena);
    let outer = crate::cell::enter(&mut arena.borrow_mut(), id, sym, foreign);
    let called = run_callback(host, arena, id, grob, sym, proc, store);
    crate::cell::said_since(host, arena, id, mark);
    crate::cell::leave(&mut arena.borrow_mut(), id, outer);
    crate::queue::after_callback(sym);
    called
}

/// A read of `sym` whose callback the caller runs, in parts, between other
/// steps (B23, D261): what [`try_callback`] does around it. A compile between
/// its steps runs nothing else on the VM, so this is as if one call held it.
pub(crate) struct OpenRead {
    arena: Rc<RefCell<Arena>>,
    id: u32,
    sym: Value,
    mark: Option<(usize, u64)>,
    outer: u32,
}

/// The read of `sym` in `grob` opened, if its data is `proc` and not a dump's.
pub(crate) fn open_read(
    host: &mut dyn Host,
    grob: Value,
    sym: Value,
    proc: Value,
) -> Option<OpenRead> {
    let (arena, id) = host_grob(host, grob)?;
    if property_data(host.heap(), &arena, id, grob, sym) != proc
        || grobs(&mut arena.borrow_mut()).dumping > 0
    {
        return None;
    }
    let foreign = proc.tag() == Tag::Closure && !host.heap().is_image_code(proc);
    let mark = crate::cell::said_mark(host, &arena);
    let outer = crate::cell::enter(&mut arena.borrow_mut(), id, sym, foreign);
    let marker = host.heap().intern("calculation-in-progress");
    set_on_alist(host.heap(), &arena, id, sym, marker, Store::Property);
    Some(OpenRead {
        arena,
        id,
        sym,
        mark,
        outer,
    })
}

impl OpenRead {
    /// The read over: `value` its result, or `None` if an error ended it.
    pub(crate) fn close(self, host: &mut dyn Host, value: Option<Value>) {
        let OpenRead {
            arena,
            id,
            sym,
            mark,
            outer,
        } = self;
        if let Some(value) = value {
            set_on_alist(host.heap(), &arena, id, sym, value, Store::Property);
        }
        crate::cell::said_since(host, &arena, id, mark);
        crate::cell::leave(&mut arena.borrow_mut(), id, outer);
        crate::queue::after_callback(sym);
    }
}

/// [`try_callback`]'s work, what it reads and writes noted as the grob's.
fn run_callback(
    host: &mut dyn Host,
    arena: &RefCell<Arena>,
    id: u32,
    grob: Value,
    sym: Value,
    proc: Value,
    store: Store,
) -> Result<Value, Exception> {
    let name = match store {
        Store::Property => "%grob-set-property-data!",
        Store::Object => "ly:grob-set-object!",
    };
    if grobs(&mut arena.borrow_mut()).dumping > 0 {
        let f = crate::lily_procedure(host, "%grob-try-callback")?;
        let store = crate::lily_procedure(host, name)?;
        return host.call(f, &[store, grob, sym, proc, Value::NULL]);
    }
    let marker = host.heap().intern("calculation-in-progress");
    set_on_alist(host.heap(), arena, id, sym, marker, store);
    let value = match crate::callbacks::find(host, arena, proc) {
        Some(f) => f(host, grob)?,
        None => host.call(proc, &[grob])?,
    };
    set_on_alist(host.heap(), arena, id, sym, value, store);
    Ok(value)
}

/// `(%grob-enter grob sym proc)`: [`crate::cell::enter`] for
/// [`PRELUDE`]'s `%grob-try-callback`; what ran before, for `%grob-leave`.
pub(crate) fn grob_enter(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-enter", 1, args[0])?;
    let heap = cx.heap();
    let foreign = args[2].tag() == Tag::Closure && !heap.is_image_code(args[2]);
    let errors = cx.error_len();
    crate::cell::grob_entered(cx.heap(), &mut arena.borrow_mut(), id, errors);
    let outer = crate::cell::enter(&mut arena.borrow_mut(), id, args[1], foreign);
    Ok(Value::fixnum(i64::from(outer)).unwrap_or(Value::FALSE))
}

/// `(%grob-leave grob outer)`: [`crate::cell::leave`].
pub(crate) fn grob_leave(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "%grob-leave", 1, args[0])?;
    let outer = args[1].as_fixnum().unwrap_or(-1);
    let outer = u32::try_from(outer).unwrap_or(crate::cell::NONE);
    let errors = cx.error_len();
    crate::cell::grob_left(cx.heap(), &mut arena.borrow_mut(), id, errors);
    crate::cell::leave(&mut arena.borrow_mut(), id, outer);
    Ok(Value::UNSPECIFIED)
}

/// `internal_get_object`: what `(ly:grob-object grob sym)` gives, read here
/// as [`get_property`] reads a property; through the image when `grob` is
/// no grob or `sym` no symbol.
pub(crate) fn object(host: &mut dyn Host, grob: Value, sym: Value) -> Result<Value, Exception> {
    if sym.tag() == Tag::Symbol
        && let Some((arena, id)) = host_grob(host, grob)
    {
        let alist = object_alist(&mut arena.borrow_mut(), id);
        let heap = host.heap();
        let value = assq_ref(heap, sym, alist).unwrap_or(Value::NULL);
        if !heap.is_procedure(value) {
            return Ok(value);
        }
        return try_callback(host, &arena, id, grob, sym, value, Store::Object);
    }
    let f = crate::lily_procedure(host, "ly:grob-object")?;
    host.call(f, &[grob, sym])
}

/// The item `i` of a list, or `'()`: `scm_car`, `scm_cadr` and so on.
fn nth(heap: &Heap, mut list: Value, i: usize) -> Value {
    for _ in 0..i {
        list = heap.cdr(list).unwrap_or(Value::NULL);
    }
    heap.car(list).unwrap_or(Value::NULL)
}

/// `from_scm<double>`: Guile's type error for what is no real.
pub(crate) fn to_double(heap: &mut Heap, v: Value) -> Result<f64, Exception> {
    match heap.to_f64(v) {
        Some(x) if heap.is_real(v) => Ok(x),
        _ => Err(crate::wrong_type(heap, None, 0, v, "real number")),
    }
}

/// `layout ()->get_dimension (name)`: `name` in the output definition of
/// the grob's translation, or its parents.
pub(crate) fn layout_dimension(
    host: &mut dyn Host,
    grob: Value,
    name: &str,
) -> Result<f64, Exception> {
    let odef = grob_of(host.heap(), grob).map(|(arena, _)| arena.borrow().output_def());
    let sym = host.heap().intern(name);
    let v = odef
        .and_then(|odef| host_lookup_variable(host, odef, sym))
        .unwrap_or(Value::UNDEFINED);
    to_double(host.heap(), v)
}

/// `get_print_stencil`: the grob's `stencil` as the page prints it, or an
/// empty stencil for none. Unless `transparent`, its `whiteout` first
/// (`stencil.scm`'s `stencil-whiteout`, from the image), then the
/// `grob-cause` wrapper that links it to the grob; `transparent`, its
/// extents only. Then its `rotation`, its `color` (`Stencil::in_color`)
/// and its `output-attributes`. The properties are read through
/// `ly:grob-property`, in `grob.cc`'s order. The skylines that
/// `show-horizontal-skylines` and `show-vertical-skylines` draw need
/// `skyline.cc` (part 3) and throw by name until then. The expression
/// is not rooted: it lives until the caller's next safe point.
pub fn get_print_stencil(host: &mut dyn Host, grob: Value) -> Result<Stencil, Exception> {
    let depth = host.heap().root_depth();
    host.heap().root(grob);
    let printed = print_stencil(host, grob);
    host.heap().unroot_to(depth);
    printed
}

fn print_stencil(host: &mut dyn Host, grob: Value) -> Result<Stencil, Exception> {
    let stil = property(host, grob, "stencil")?;
    let mut retval = Stencil::default();
    if let Some(&m) = host.heap().foreign::<Stencil>(stil) {
        // What a call into the image could free: the stencil printed so far.
        let kept = host.heap().root(stil);
        retval = m;
        let transparent = property(host, grob, "transparent")? == Value::TRUE;

        // Process whiteout before color and grob-cause to prevent colored
        // whiteout background and larger file sizes with \pointAndClickOn.
        // A grob has to be visible, otherwise the whiteout property has no
        // effect. Calls the scheme procedure stencil-whiteout in
        // scm/stencils.scm
        if !transparent {
            let whiteout = property(host, grob, "whiteout")?;
            host.heap().root(whiteout);
            if host.heap().is_number(whiteout) || whiteout == Value::TRUE {
                let line_thickness = layout_dimension(host, grob, "line-thickness")?;
                let mut whiteout_color = property(host, grob, "whiteout-color")?;
                if whiteout_color.tag() != Tag::String && whiteout_color.tag() != Tag::Pair {
                    whiteout_color = host.heap().string("white");
                }
                host.heap().root(whiteout_color);
                let style = property(host, grob, "whiteout-style")?;
                let f = crate::lily_procedure(host, "stencil-whiteout")?;
                let heap = host.heap();
                let s = heap.wrap(retval);
                let thickness = heap.flonum(line_thickness);
                let white = host.call(f, &[s, style, whiteout, thickness, whiteout_color])?;
                retval = stencil::from_image(host.heap(), "Grob::get_print_stencil", white)?;
            }
        }

        let heap = host.heap();
        if transparent {
            retval = Stencil::new(m.dim, Value::NULL);
        } else {
            let head = heap.intern("grob-cause");
            let expr = heap.list(&[head, grob, retval.expr]);
            retval = Stencil::new(retval.dim, expr);
        }
        heap.set_rooted(kept, retval.expr);

        let rot = property(host, grob, "rotation")?;
        if rot.tag() == Tag::Pair {
            let heap = host.heap();
            let items = [0, 1, 2].map(|i| nth(heap, rot, i));
            let [angle, x, y] = items.map(|v| to_double(heap, v));
            retval.rotate_degrees(heap, angle?, (x?, y?));
            heap.set_rooted(kept, retval.expr);
        }

        // color support... see interpret_stencil_expression () for more...
        let color = property(host, grob, "color")?;
        if color.tag() == Tag::String {
            retval = retval.in_color(host, color)?;
        } else if color.tag() == Tag::Pair {
            let heap = host.heap();
            let mut rgba = [1.0; 4];
            for (i, c) in rgba.iter_mut().enumerate() {
                let item = nth(heap, color, i);
                if i < 3 || item != Value::NULL {
                    *c = to_double(heap, item)?;
                }
            }
            let rgba = rgba.map(|c| heap.flonum(c));
            let color = heap.list(&rgba);
            retval = retval.in_color(host, color)?;
        }
        host.heap().set_rooted(kept, retval.expr);

        let attributes = property(host, grob, "output-attributes")?;
        if attributes.tag() == Tag::Pair {
            let heap = host.heap();
            let head = heap.intern("output-attributes");
            let expr = heap.list(&[head, attributes, retval.expr]);
            retval = Stencil::new(retval.dim, expr);
            heap.set_rooted(kept, retval.expr);
        }
    }
    for name in ["show-horizontal-skylines", "show-vertical-skylines"] {
        if property(host, grob, name)? == Value::TRUE {
            return Err(crate::misc_error(
                host.heap(),
                "Grob::get_print_stencil",
                "skylines are not in the engine before MS3's part 3",
            ));
        }
    }
    Ok(retval)
}

// --- the rest of grob-scheme.cc (MS3 step 14) -------------------------------

/// `interfaces ()`: the grob's `interfaces_` as LilyPond's constructors
/// make it, its definition's `meta.interfaces` with its class's consed in
/// front in the order they add them (`item.rs`); `'()` for a dead grob.
/// It is read from the definition each time, where LilyPond keeps the list
/// it was made with.
pub fn interfaces(heap: &mut Heap, grob: Value) -> Value {
    let Some((arena, id)) = grob_of(heap, grob) else {
        return Value::NULL;
    };
    if !is_live(&arena, id) {
        return Value::NULL;
    }
    let class = item::classify(heap, &arena, id, grob);
    let meta = heap.intern("meta");
    let meta = property_data(heap, &arena, id, grob, meta);
    let key = heap.intern("interfaces");
    let mut list = assq_ref(heap, key, meta).unwrap_or(Value::NULL);
    for name in class.map_or(&[][..], Class::interfaces) {
        let sym = heap.intern(name);
        list = heap.cons(sym, list);
    }
    list
}

/// `get_property_alist_chain (global)`: the grob's mutable alist, its
/// definition (the immutable one) and `global`. The cause
/// `Engraver::announce_grob` sets, an event's or a grob's, is the record's:
/// unless one was set on the grob, it ends a copy of the mutable alist,
/// where LilyPond conses it in front of what the constructor set.
pub fn alist_chain(heap: &mut Heap, grob: Value, global: Value) -> Value {
    let Some((arena, id)) = grob_of(heap, grob) else {
        return Value::NULL;
    };
    let (props, definition) = {
        let a = arena.borrow();
        let g = &a.records.grobs[id as usize];
        (g.props, g.definition)
    };
    let sym = heap.intern("cause");
    let mut mutable = props;
    if assq(heap, sym, props).is_none()
        && let Some(cause) = grob_cause(heap, grob)
    {
        let mut entries = Vec::new();
        let mut p = props;
        while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            entries.push(entry);
            p = rest;
        }
        entries.push(heap.cons(sym, cause));
        mutable = heap.list(&entries);
    }
    heap.list(&[mutable, definition, global])
}

/// `robust_relative_extent` (`grob.cc:613–621`): the grob's [`extent`] on
/// `a` relative to `refp`, or, if it is empty, its
/// [`relative_coordinate`] there.
pub fn robust_relative_extent(
    host: &mut dyn Host,
    grob: Value,
    refp: Value,
    a: Axis,
) -> Result<Interval, Exception> {
    let ext = extent(host, grob, refp, a)?;
    if ext.left > ext.right {
        let c = relative_coordinate(host, grob, refp, a)?;
        return Ok(Interval { left: c, right: c });
    }
    Ok(ext)
}

/// `common_refpoint_of_array`: the common refpoint on `a` of `common`
/// and `grobs`, one after the other; past a grob with none, the next grob
/// starts again.
pub fn common_refpoint_of_array(
    heap: &Heap,
    grobs: &[Value],
    common: Option<Value>,
    a: Axis,
) -> Option<Value> {
    grobs.iter().fold(common, |common, &g| match common {
        Some(c) => common_refpoint(heap, c, g, a),
        None => Some(g),
    })
}

/// `get_maybe_root_vertical_alignment` from `g` up its parents on Y: the
/// topmost alignment (`align-interface`), else `maybe`.
pub(crate) fn root_vertical_alignment(heap: &mut Heap, g: Value) -> Option<Value> {
    let (mut g, mut found) = (Some(g), None);
    while let Some(grob) = g {
        if has_interface(heap, grob, "align-interface") {
            found = Some(grob);
        }
        g = get_parent(heap, grob, crate::geometry::Y);
    }
    found
}

/// `get_vertical_axis_group`: the grob or its first ancestor on Y that is
/// an axis group (`axis-group-interface`) whose parent there is an
/// alignment.
pub(crate) fn vertical_axis_group(heap: &mut Heap, g: Value) -> Option<Value> {
    let mut g = g;
    loop {
        let parent = get_parent(heap, g, crate::geometry::Y)?;
        if has_interface(heap, g, "axis-group-interface")
            && has_interface(heap, parent, "align-interface")
        {
            return Some(g);
        }
        g = parent;
    }
}

/// `(%grob-root-vertical-alignment grob)`: its root alignment, or `#f`.
pub(crate) fn root_vertical_alignment_prim(cx: &mut Cx, args: &[Value]) -> R {
    grob_arg(cx, "%grob-root-vertical-alignment", 1, args[0])?;
    Ok(root_vertical_alignment(cx.heap_mut(), args[0]).unwrap_or(Value::FALSE))
}

/// `(%grob-vertical-axis-group grob)`: its axis group, or `#f`.
pub(crate) fn vertical_axis_group_prim(cx: &mut Cx, args: &[Value]) -> R {
    grob_arg(cx, "%grob-vertical-axis-group", 1, args[0])?;
    Ok(vertical_axis_group(cx.heap_mut(), args[0]).unwrap_or(Value::FALSE))
}

/// `extract_grob_set`: the grob array `sym` of the grob `grob`, read
/// through its callback, as a list of handles; none for no grob array.
fn grob_set(host: &mut dyn Host, grob: Value, sym: &str) -> Result<Vec<Value>, Exception> {
    let sym = host.heap().intern(sym);
    let arr = object(host, grob, sym)?;
    Ok(crate::grob_array::grobs_of(host.heap(), arr))
}

/// `vertical_less` (`internal_vertical_less`, `grob.cc:723–751`): whether
/// `g1` lies above `g2`: by their axis groups' order in their root
/// alignment's `elements`, or, in one axis group, by their coordinates
/// from their common refpoint on Y; false, with LilyPond's programming
/// errors, for no alignment or an axis group not in it.
pub fn vertical_less(host: &mut dyn Host, g1: Value, g2: Value) -> Result<bool, Exception> {
    let Some(vag) = root_vertical_alignment(host.heap(), g1) else {
        programming_error(host, g1, "grob does not belong to a VerticalAlignment?")?;
        return Ok(false);
    };
    let ag1 = vertical_axis_group(host.heap(), g1);
    let ag2 = vertical_axis_group(host.heap(), g2);
    let elts = grob_set(host, vag, "elements")?;
    if ag1 == ag2 {
        let common = common_refpoint(host.heap(), g1, g2, Y).unwrap_or(Value::FALSE);
        let y1 = relative_coordinate(host, g1, common, Y)?;
        return Ok(y1 > relative_coordinate(host, g2, common, Y)?);
    }
    for g in elts {
        if Some(g) == ag1 {
            return Ok(true);
        }
        if Some(g) == ag2 {
            return Ok(false);
        }
    }
    programming_error(host, g1, "could not place this grob in its axis group")?;
    Ok(false)
}

/// `get_vertical_axis_group_index` (`grob.cc:705–721`): the index of the
/// grob's axis group among its root alignment's `elements`, or -1, with
/// LilyPond's programming error for one not in them.
pub fn get_vertical_axis_group_index(host: &mut dyn Host, g: Value) -> Result<i64, Exception> {
    let Some(vag) = root_vertical_alignment(host.heap(), g) else {
        return Ok(-1);
    };
    let vax = vertical_axis_group(host.heap(), g);
    let elts = grob_set(host, vag, "elements")?;
    if let Some(i) = elts.iter().position(|&e| Some(e) == vax) {
        return Ok(i as i64);
    }
    let text = "could not find this grob's vertical axis group in the vertical alignment";
    programming_error(host, g, text)?;
    Ok(-1)
}

/// `(ly:grob-property-data grob sym)`: `get_property_data`, no callback
/// called.
pub(crate) fn ly_grob_property_data(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-property-data";
    let (arena, id) = grob_arg(cx, SUBR, 1, args[0])?;
    symbol_arg(cx, SUBR, args[1])?;
    Ok(property_data(cx.heap_mut(), &arena, id, args[0], args[1]))
}

/// `(ly:grob-interfaces grob)`: [`interfaces`].
pub(crate) fn ly_grob_interfaces(cx: &mut Cx, args: &[Value]) -> R {
    grob_arg(cx, "ly:grob-interfaces", 1, args[0])?;
    Ok(interfaces(cx.heap_mut(), args[0]))
}

/// `(ly:grob-layout grob)`: the output definition of its translation.
pub(crate) fn ly_grob_layout(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, _) = grob_arg(cx, "ly:grob-layout", 1, args[0])?;
    let layout = arena.borrow().output_def();
    Ok(layout)
}

/// `(ly:grob-alist-chain grob [global])`: [`alist_chain`], `global` `'()`
/// if not given.
pub(crate) fn ly_grob_alist_chain(cx: &mut Cx, args: &[Value]) -> R {
    grob_arg(cx, "ly:grob-alist-chain", 1, args[0])?;
    let global = args.get(1).copied().unwrap_or(Value::NULL);
    Ok(alist_chain(cx.heap_mut(), args[0], global))
}

/// `(ly:grob-basic-properties grob)`: its definition, the immutable alist.
pub(crate) fn ly_grob_basic_properties(cx: &mut Cx, args: &[Value]) -> R {
    let (arena, id) = grob_arg(cx, "ly:grob-basic-properties", 1, args[0])?;
    let definition = arena.borrow().records.grobs[id as usize].definition;
    Ok(definition)
}

/// `(ly:grob-system grob)`: its system (`item.rs`), or `'()`.
pub(crate) fn ly_grob_system(cx: &mut Cx, args: &[Value]) -> R {
    grob_arg(cx, "ly:grob-system", 1, args[0])?;
    Ok(item::get_system(cx.heap_mut(), args[0]).unwrap_or(Value::NULL))
}

/// `original`: the handle of the grob `grob` was broken from, if it is a
/// broken copy.
pub fn original(heap: &Heap, grob: Value) -> Option<Value> {
    let (arena, id) = grob_of(heap, grob)?;
    grobs(&mut arena.borrow_mut()).grobs[id as usize].original
}

/// `(ly:grob-original grob)`: the grob it was broken from, or itself.
pub(crate) fn ly_grob_original(cx: &mut Cx, args: &[Value]) -> R {
    grob_arg(cx, "ly:grob-original", 1, args[0])?;
    Ok(original(cx.heap(), args[0]).unwrap_or(args[0]))
}

/// `(ly:grob-common-refpoint-of-array grob others axis)`: the common
/// refpoint of `grob` and the grob array `others`, or `#f`.
pub(crate) fn ly_grob_common_refpoint_of_array(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:grob-common-refpoint-of-array";
    grob_arg(cx, SUBR, 1, args[0])?;
    if cx
        .heap()
        .foreign::<crate::grob_array::GrobArray>(args[1])
        .is_none()
    {
        return Err(cx.wrong_type(SUBR, 2, args[1], "Grob_array"));
    }
    let a = crate::axis_arg(cx, SUBR, 3, args[2])?;
    let others = crate::grob_array::grobs_of(cx.heap(), args[1]);
    let common = common_refpoint_of_array(cx.heap(), &others, Some(args[0]), a);
    Ok(common.unwrap_or(Value::FALSE))
}

/// `(ly:grob-spanned-column-rank-interval grob)`: the ranks of its first
/// and last columns (`item.rs`).
pub(crate) fn ly_grob_spanned_column_rank_interval(cx: &mut Cx, args: &[Value]) -> R {
    grob_arg(cx, "ly:grob-spanned-column-rank-interval", 1, args[0])?;
    let (left, right) = item::spanned_column_rank_interval(cx.heap_mut(), args[0]);
    let heap = cx.heap_mut();
    let [left, right] =
        [left, right].map(|r| heap.integer_big(lily_engine_scheme::BigInt::from_u64(r)));
    Ok(heap.cons(left, right))
}

// --- what the writer links a grob to (output-svg.scm:440–466) --------------

/// The link `output-svg.scm`'s `grob-cause` writes for `grob`, as
/// [`PRELUDE`]'s `%grob-cause-link` makes it: with `point-and-click` on,
/// the `textedit://` link to the origin of the grob's cause, a stream
/// event, if the option is `#t`, or names a class of the event's, or a
/// list of which one is. Not through grob causes, as `Grob::origin`
/// reaches: a grob caused by a grob has no link. `None` for no link, for
/// what is no grob (the references' `#f`), or without the image's
/// `%grob-cause-link`.
pub fn cause_link(host: &mut dyn Host, grob: Value) -> Result<Option<String>, Exception> {
    if grob_of(host.heap(), grob).is_none() {
        return Ok(None);
    }
    let Some(f) = host.lily("%grob-cause-link") else {
        return Ok(None);
    };
    let link = host.call(f, &[grob])?;
    Ok(host
        .heap()
        .str(link)
        .map(|link| String::from(link.as_str())))
}

// --- grob.cc's callbacks in Rust --------------------------------------------

/// `grob_stencil_extent` (`grob.cc:426–437`): the extent on `a` of a live
/// grob's `stencil`, empty for none.
fn stencil_extent(host: &mut dyn Host, grob: Value, a: Axis) -> Result<Value, Exception> {
    let stencil = if is_live_grob(host.heap(), grob) {
        property(host, grob, "stencil")?
    } else {
        Value::NULL
    };
    let heap = host.heap();
    let ext = match heap.foreign::<Stencil>(stencil) {
        Some(s) => s.extent(a),
        None => Interval::EMPTY,
    };
    let (l, r) = (heap.flonum(ext.left), heap.flonum(ext.right));
    Ok(heap.cons(l, r))
}

/// `ly:grob::stencil-width` (`Grob::stencil_width`).
pub(crate) fn stencil_width(host: &mut dyn Host, grob: Value) -> Result<Value, Exception> {
    stencil_extent(host, grob, X)
}

/// `ly:grob::stencil-height` (`Grob::stencil_height`).
pub(crate) fn stencil_height(host: &mut dyn Host, grob: Value) -> Result<Value, Exception> {
    stencil_extent(host, grob, Y)
}

/// `Grob::x_parent_positioning` and `y_parent_positioning`
/// (`grob.cc:905–932`): the parent's `positioning-done` read on `a`, then
/// 0.
fn parent_positioning(host: &mut dyn Host, grob: Value, a: Axis) -> Result<Value, Exception> {
    host_grob_arg(host, "ly:grob-parent", grob)?;
    if let Some(parent) = get_parent(host.heap(), grob, a)
        && grob_of(host.heap(), parent).is_some()
    {
        property(host, parent, "positioning-done")?;
    }
    Ok(host.heap().flonum(0.0))
}

/// `ly:grob::x-parent-positioning`.
pub(crate) fn x_parent_positioning(host: &mut dyn Host, grob: Value) -> Result<Value, Exception> {
    parent_positioning(host, grob, X)
}

/// `ly:grob::y-parent-positioning`.
pub(crate) fn y_parent_positioning(host: &mut dyn Host, grob: Value) -> Result<Value, Exception> {
    parent_positioning(host, grob, Y)
}

/// grob.cc's callbacks, which the callback table finds (`callbacks.rs`).
pub(crate) const CALLBACKS: [(&str, Callback); 4] = [
    ("stencil-height", stencil_height),
    ("stencil-width", stencil_width),
    ("x-parent-positioning", x_parent_positioning),
    ("y-parent-positioning", y_parent_positioning),
];

/// What `%host-call` runs for [`PRELUDE`]'s `(%host-call 'grob what grob
/// …)` (D127): grob.cc's callbacks, offsets, coordinates and extents in
/// Rust, with the VM as their host.
pub(crate) const HOSTS: [HostHandler; 2] = [
    HostHandler {
        what: "grob",
        f: host_grob_call,
    },
    HostHandler {
        what: "n8-write",
        f: host_n8_write,
    },
];

/// N8's replay (MS6 step 7b): a write the old run's translator made, made
/// again to this run's grob (`lily_engine_music::Op`): `(%host-call
/// 'n8-write grob kind a b flag)`, kind 0 a property `a` set to `b`, 1 an
/// object (a grob array a copy, its grobs this run's), 2 the parent on the
/// axis `a`, `b` (`#f` none), 3 the grob `b` added to the grob array `a`
/// (ordered if `flag`), 4 that array ordered as `flag`, 5 the bound on the
/// side `a`, `b`, 6 `suicide`, 7 the grob array `a` holding `b` alone
/// (none for `#f`).
fn host_n8_write(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let (grob, a, b, flag) = (args[1], args[3], args[4], args[5] == Value::TRUE);
    let Some((arena, id)) = grob_of(vm.heap(), grob) else {
        return Ok(Value::UNSPECIFIED);
    };
    match args[2].as_fixnum() {
        Some(0) => set_property_data(vm.heap(), &arena, id, a, b),
        Some(1) => match crate::grob_array::replayed(vm.heap(), b) {
            Some(v) => set_on_alist(vm.heap(), &arena, id, a, v, Store::Object),
            None => return Ok(Value::FALSE),
        },
        Some(2) => {
            let axis = if a.as_fixnum() == Some(1) { Y } else { X };
            let parent = (b != Value::FALSE).then_some(b);
            set_parent_or_none(vm.heap(), grob, parent, axis);
        }
        Some(3) if flag => crate::grob_array::add_grob(vm, grob, a, b)?,
        Some(3) => crate::grob_array::add_unordered_grob(vm, grob, a, b)?,
        Some(4) => crate::grob_array::set_ordered(vm, grob, a, flag)?,
        Some(5) => {
            let side = a.as_fixnum().unwrap_or(0) as usize;
            crate::spanner::set_bound_replayed(vm.heap_mut(), grob, side, b);
        }
        Some(6) => suicide(vm.heap_mut(), grob),
        Some(7) => {
            let p = (b != Value::FALSE).then_some(b);
            crate::grob_array::reset(vm, grob, a, p)?;
        }
        _ => {}
    }
    Ok(Value::UNSPECIFIED)
}

fn host_grob_call(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let what = vm.heap().name(args[1]);
    if let Some(&(_, f)) = CALLBACKS.iter().find(|&&(n, _)| Some(n) == what) {
        return f(vm, args[2]);
    }
    let what = String::from(what.unwrap_or_default());
    let (grob, refp) = (args[2], args.get(3).copied().unwrap_or(Value::NULL));
    let a = if args.get(4).and_then(|v| v.as_fixnum()) == Some(1) {
        Y
    } else {
        X
    };
    let ext = match what.as_str() {
        "relative-coordinate" => {
            let c = relative_coordinate(vm, grob, refp, a)?;
            return Ok(vm.heap_mut().flonum(c));
        }
        "extent" => extent(vm, grob, refp, a)?,
        "robust-relative-extent" => robust_relative_extent(vm, grob, refp, a)?,
        _ => {
            let heap = vm.heap_mut();
            return Err(crate::misc_error(heap, "%host-call", "no such grob call"));
        }
    };
    let heap = vm.heap_mut();
    let (l, r) = (heap.flonum(ext.left), heap.flonum(ext.right));
    Ok(heap.cons(l, r))
}

/// `(%host-waiting?)`: whether a run was cancelled for the host, so that a
/// `%host-call` would be cancelled too ([`PRELUDE`]).
pub(crate) fn host_waiting(cx: &mut Cx, _: &[Value]) -> R {
    Ok(Value::bool(cx.waiting()))
}

/// `try_callback_on_alist` (`%grob-try-callback`), and the primitives that
/// check a value's type before they set it. `ly:grob-property`,
/// `ly:grob-object` and `ly:grob-parent` have their setters, as LilyPond's
/// `LY_DEFINE_WITH_SETTER` gives them. `get_offset`, `relative_coordinate`
/// and `extent` read properties, whose callbacks a primitive cannot call:
/// `%grob-offset`, `%grob-relative-coordinate` and `%grob-extent` read
/// them between the primitives that keep the caches, in `grob.cc`'s
/// order, and `ly:grob-relative-coordinate` and `ly:grob-extent` check
/// their arguments first (`check_refp`). And `grob.cc`'s callbacks of the
/// stencil's extents (`grob_stencil_extent`) and of the parent's
/// positioning, and the constructor's defaults of the extents. And
/// `%grob-reported`, which gives a grob's messages at its origin, and
/// `%grob-cause-link`, `output-svg.scm`'s `grob-cause` but its `<a>`: the
/// link alone, or `#f`. And the calls of `grob-scheme.cc` that read a
/// property through its callbacks: `ly:grob-set-nested-property!` with
/// `nested-property.cc`'s `nested_property_alist`,
/// `ly:grob-robust-relative-extent`, `ly:grob-chain-callback`
/// (`grob-closure.cc`'s `chain_callback`), and `grob.cc`'s vertical
/// ordering, `ly:grob-vertical<?` and
/// `ly:grob-get-vertical-axis-group-index`, which read the root
/// alignment's `elements` as `extract_grob_set` does (`%grob-set`).
pub(crate) const PRELUDE: &str = r#"
(define (%grob-reported kind location texts value)
  (for-each (lambda (text) (%diagnose kind location text)) texts)
  value)
(define (%grob-try-callback store grob sym proc default)
  (let ((outer (%grob-enter grob sym proc)))
    (store grob sym 'calculation-in-progress)
    (let ((value (proc grob)))
      (store grob sym value)
      (%grob-leave grob outer)
      (if (null? value) default value))))
(define-public (ly:grob-set-property! grob sym val)
  (%check-type "ly:grob-set-property!" 1 ly:grob? "Grob" grob)
  (%check-type "ly:grob-set-property!" 2 symbol? "symbol" sym)
  (let ((checked (%type-check sym val 'backend-type? #f)))
    (if checked (%grob-set-property-data! grob (car checked) (cdr checked))))
  *unspecified*)
(define ly:grob-property (make-procedure-with-setter ly:grob-property ly:grob-set-property!))
(define ly:grob-object (make-procedure-with-setter ly:grob-object ly:grob-set-object!))
(define ly:grob-parent (make-procedure-with-setter ly:grob-parent ly:grob-set-parent!))
(define (%grob-offset grob a)
  (or (%grob-offset-begin grob a)
      (%grob-offset-end grob a (ly:grob-property grob (if (eqv? a 0) 'X-offset 'Y-offset)))))
(define (%grob-relative-coordinate grob refp a)
  (if (%host-waiting?)
      (%grob-scheme-relative-coordinate grob refp a)
      (%host-call 'grob 'relative-coordinate grob refp a)))
(define (%grob-scheme-relative-coordinate grob refp a)
  (let loop ((ancestor grob) (result 0.0))
    (if (or (eq? ancestor refp) (null? ancestor))
        result
        (let ((offset (%grob-offset ancestor a)))
          (loop (ly:grob-parent ancestor a) (+ result offset))))))
(define (%grob-extent grob refp a)
  (if (%host-waiting?)
      (%grob-scheme-extent grob refp a)
      (%host-call 'grob 'extent grob refp a)))
(define (%grob-scheme-extent grob refp a)
  (let ((offset (%grob-relative-coordinate grob refp a)))
    (or (%grob-extent-at grob a offset)
        (let* ((extent (ly:grob-property grob (if (eqv? a 0) 'X-extent 'Y-extent)))
               (minimum (ly:grob-property grob (if (eqv? a 0) 'minimum-X-extent 'minimum-Y-extent))))
          (%grob-cache-extent! grob a extent minimum)
          (%grob-extent-at grob a offset)))))
(define-public (ly:grob-relative-coordinate grob refp axis)
  (%grob-check-refp "ly:grob-relative-coordinate" grob refp axis)
  (%grob-relative-coordinate grob refp axis))
(define-public (ly:grob-extent grob refp axis)
  (%grob-check-refp "ly:grob-extent" grob refp axis)
  (%grob-extent grob refp axis))
(define (%grob-stencil-extent grob a)
  (let ((stencil (if (%grob-live? grob) (ly:grob-property grob 'stencil) '())))
    (if (ly:stencil? stencil) (ly:stencil-extent stencil a) (cons +inf.0 -inf.0))))
(define-public (ly:grob::stencil-width grob)
  (%check-type "ly:grob::stencil-width" 1 ly:grob? "Grob" grob)
  (if (%host-waiting?)
      (%grob-stencil-extent grob 0)
      (%host-call 'grob 'stencil-width grob)))
(define-public (ly:grob::stencil-height grob)
  (%check-type "ly:grob::stencil-height" 1 ly:grob? "Grob" grob)
  (if (%host-waiting?)
      (%grob-stencil-extent grob 1)
      (%host-call 'grob 'stencil-height grob)))
(define-public (ly:grob::pure-stencil-height grob start end)
  (%check-type "ly:grob::pure-stencil-height" 1 ly:grob? "Grob" grob)
  (if (ly:stencil? (%grob-property-data grob 'stencil))
      (%grob-stencil-extent grob 1)
      (cons +inf.0 -inf.0)))
(define (%grob-parent-positioning grob a)
  (let ((parent (ly:grob-parent grob a)))
    (if (ly:grob? parent) (ly:grob-property parent 'positioning-done)))
  0.0)
(define-public (ly:grob::x-parent-positioning grob)
  (%check-type "ly:grob::x-parent-positioning" 1 ly:grob? "Grob" grob)
  (if (%host-waiting?)
      (%grob-parent-positioning grob 0)
      (%host-call 'grob 'x-parent-positioning grob)))
(define-public (ly:grob::y-parent-positioning grob)
  (%check-type "ly:y-parent-positioning" 1 ly:grob? "Grob" grob)
  (if (%host-waiting?)
      (%grob-parent-positioning grob 1)
      (%host-call 'grob 'y-parent-positioning grob)))
(define %grob-default-extents
  (cons ly:grob::stencil-width
        (ly:make-unpure-pure-container ly:grob::stencil-height ly:grob::pure-stencil-height)))
(define (%grob-cause-link grob)
  (and (ly:get-option 'point-and-click)
       (let* ((cause (ly:grob-property grob 'cause))
              (music-origin (if (ly:stream-event? cause)
                                (ly:event-property cause 'origin)))
              (point-and-click (ly:get-option 'point-and-click)))
         (and (ly:input-location? music-origin)
              (cond ((boolean? point-and-click) point-and-click)
                    ((symbol? point-and-click)
                     (ly:in-event-class? cause point-and-click))
                    (else (any (lambda (t)
                                 (ly:in-event-class? cause t))
                               point-and-click)))
              (let* ((location (ly:input-file-line-char-column music-origin))
                     (raw-file (car location))
                     (file (if (is-absolute? raw-file)
                               raw-file
                               (string-append (ly-getcwd) "/" raw-file))))
                (format #f "textedit://~a:~a:~a:~a"
                        (ly:string-percent-encode
                         (ly:string-substitute "\\" "/" file))
                        (cadr location)
                        (caddr location)
                        (1+ (cadddr location))))))))
(define (%nested-property-alist alist path value)
  (let ((key (car path)) (rest (cdr path)))
    (if (pair? rest)
        (let ((where (let find ((p alist))
                       (cond ((not (pair? p)) #f)
                             ((eq? (caar p) key) p)
                             (else (find (cdr p)))))))
          (if where
              (acons key (%nested-property-alist (cdar where) rest value)
                     (let copy ((p alist))
                       (if (eq? p where) (cdr where) (cons (car p) (copy (cdr p))))))
              (acons key
                     (let create ((path rest))
                       (if (null? path) value (list (cons (car path) (create (cdr path))))))
                     alist)))
        (acons key value alist))))
(define-public (ly:grob-set-nested-property! grob symlist val)
  (%check-type "ly:grob-set-nested-property!" 1 ly:grob? "Grob" grob)
  (if (not (and (pair? symlist)
                (let symbols? ((s symlist))
                  (or (not (pair? s)) (and (symbol? (car s)) (symbols? (cdr s)))))))
      (scm-error 'wrong-type-arg "ly_grob_set_nested_property_x"
                 "Wrong type argument in position ~A (expecting ~A): ~S"
                 (list 2 "list of symbols" symlist) (list symlist)))
  (if (pair? (cdr symlist))
      (%grob-set-property-data!
       grob (car symlist)
       (%nested-property-alist (ly:grob-property grob (car symlist)) (cdr symlist) val))
      (ly:grob-set-property! grob (car symlist) val))
  *unspecified*)
(define (%grob-robust-relative-extent grob refp a)
  (if (%host-waiting?)
      (%grob-scheme-robust-relative-extent grob refp a)
      (%host-call 'grob 'robust-relative-extent grob refp a)))
(define (%grob-scheme-robust-relative-extent grob refp a)
  (let ((extent (%grob-extent grob refp a)))
    (if (> (car extent) (cdr extent))
        (let ((coordinate (%grob-relative-coordinate grob refp a)))
          (cons coordinate coordinate))
        extent)))
(define-public (ly:grob-robust-relative-extent grob refp axis)
  (%grob-check-refp "ly:grob-robust-relative-extent" grob refp axis)
  (%grob-robust-relative-extent grob refp axis))
(define-public (ly:grob-chain-callback grob proc sym)
  (%check-type "ly:grob-chain-callback" 1 ly:grob? "Grob" grob)
  (if (not (or (procedure? proc) (ly:unpure-pure-container? proc)))
      (scm-error 'wrong-type-arg "ly_grob_chain_callback"
                 "Wrong type argument in position ~A (expecting ~A): ~S"
                 (list 2 "procedure or unpure pure container" proc) (list proc)))
  (%check-type "ly:grob-chain-callback" 3 symbol? "symbol" sym)
  (%grob-set-property-data! grob sym (grob::compose-function proc (%grob-property-data grob sym)))
  *unspecified*)
(define (%grob-set grob sym)
  (let ((arr (ly:grob-object grob sym)))
    (if (ly:grob-array? arr) (ly:grob-array->list arr) '())))
(define-public (ly:grob-vertical<? a b)
  (%check-type "ly:grob-vertical<?" 1 ly:grob? "Grob" a)
  (%check-type "ly:grob-vertical<?" 2 ly:grob? "Grob" b)
  (let ((vag (%grob-root-vertical-alignment a)))
    (if (not vag)
        (begin
          (%grob-programming-error a "grob does not belong to a VerticalAlignment?")
          #f)
        (let* ((ag1 (%grob-vertical-axis-group a))
               (ag2 (%grob-vertical-axis-group b))
               (elts (%grob-set vag 'elements)))
          (if (eq? ag1 ag2)
              (let ((common (ly:grob-common-refpoint a b 1)))
                (> (%grob-relative-coordinate a common 1)
                   (%grob-relative-coordinate b common 1)))
              (let find ((elts elts))
                (cond ((null? elts)
                       (%grob-programming-error a "could not place this grob in its axis group")
                       #f)
                      ((eq? (car elts) ag1) #t)
                      ((eq? (car elts) ag2) #f)
                      (else (find (cdr elts))))))))))
(define-public (ly:grob-get-vertical-axis-group-index grob)
  (%check-type "ly:grob-get-vertical-axis-group-index" 1 ly:grob? "Grob" grob)
  (let ((vag (%grob-root-vertical-alignment grob)))
    (if (not vag)
        -1
        (let ((vax (%grob-vertical-axis-group grob)))
          (let find ((elts (%grob-set vag 'elements)) (i 0))
            (cond ((null? elts)
                   (%grob-programming-error
                    grob "could not find this grob's vertical axis group in the vertical alignment")
                   -1)
                  ((eq? (car elts) vax) i)
                  (else (find (cdr elts) (1+ i)))))))))
"#;
