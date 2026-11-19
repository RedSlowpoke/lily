//! 05 §5.1's slots for a grob's own properties (MS3 step 46, lead 2):
//! what is set on a grob and what its callbacks computed, found by the
//! property's symbol where `internal_get_property_data` walks the grob's
//! mutable alist (the record's `props`, D108) with `scm_sloppy_assq`.
//!
//! The alist stays what LilyPond's is, since Scheme reads it
//! (`ly:grob-alist-chain`, the markups' property chains) and the music
//! crate sets it (D108): the slots are the first entry of each of its
//! keys, sorted by the key's bits, as D203's table is of a definition, and
//! an entry's value is read when it is asked for. They are kept for the
//! alist they were made of: an alist set in front of them (the music
//! crate's `ly:grob-set-property!`, the engraver's properties, `suicide`)
//! makes them again at the next read; `grob.rs`' own writes keep them as
//! they go: an entry consed in front is added, an entry set in place is
//! the same entry, and an entry removed (`del_property`) gives its place to
//! the key's next entry. Like D203's table, a spine changed in place by
//! Scheme would not be seen (no LilyPond file does it). The per-type slots
//! of 05 §5.1, decoded values and generated defaults, are not here.

use alloc::vec::Vec;

use lily_engine_scheme::{Heap, Tracer, Value};

/// A grob's slots: the alist they index and its first entry of each key.
#[derive(Clone, Default)]
pub(crate) struct Own {
    /// The alist, `None` before the first read.
    alist: Option<Value>,
    entries: Vec<(u64, Value)>,
}

impl Own {
    /// Whether it names the same alist and entries (N8's splice).
    pub(crate) fn same(&self, other: &Own) -> bool {
        self.alist == other.alist && self.entries == other.entries
    }

    /// The alist and its entries, kept alive while the slots name them, so
    /// that another alist made where it was is not taken for it.
    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        if let Some(alist) = self.alist {
            tracer.trace(alist);
        }
    }

    /// The slots of `alist`, made again if they are another's.
    fn of(&mut self, heap: &Heap, alist: Value) -> &mut Own {
        if self.alist != Some(alist) {
            self.entries.clear();
            let mut p = alist;
            while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                if let Some(key) = heap.car(entry)
                    && let Err(i) = self.entries.binary_search_by_key(&key.to_bits(), |e| e.0)
                {
                    self.entries.insert(i, (key.to_bits(), entry));
                }
                p = rest;
            }
            self.alist = Some(alist);
        }
        self
    }

    /// [`Own::entry`] if the slots of `alist` are made: a read that writes
    /// nothing, as a grob's row may be shared (D259).
    pub(crate) fn kept_entry(&self, alist: Value, key: Value) -> Option<Option<Value>> {
        (self.alist == Some(alist)).then(|| {
            let i = self.entries.binary_search_by_key(&key.to_bits(), |e| e.0);
            i.ok().map(|i| self.entries[i].1)
        })
    }

    /// The first entry of `key` in `alist`, as `scm_sloppy_assq` finds it.
    pub(crate) fn entry(&mut self, heap: &Heap, alist: Value, key: Value) -> Option<Value> {
        let own = self.of(heap, alist);
        let i = own.entries.binary_search_by_key(&key.to_bits(), |e| e.0);
        i.ok().map(|i| own.entries[i].1)
    }

    /// `entry`, `key`'s, consed in front of `old`, making `new`.
    pub(crate) fn added(&mut self, old: Value, new: Value, key: Value, entry: Value) {
        if self.alist != Some(old) {
            return;
        }
        match self.entries.binary_search_by_key(&key.to_bits(), |e| e.0) {
            Ok(i) => self.entries[i].1 = entry,
            Err(i) => self.entries.insert(i, (key.to_bits(), entry)),
        }
        self.alist = Some(new);
    }

    /// `key`'s first entry of `old` removed, making `new`, whose `rest` (the
    /// pairs after where it was) may hold another.
    pub(crate) fn removed(&mut self, heap: &Heap, old: Value, new: Value, key: Value, rest: Value) {
        if self.alist != Some(old) {
            return;
        }
        let Ok(i) = self.entries.binary_search_by_key(&key.to_bits(), |e| e.0) else {
            return;
        };
        match heap.assq(key, rest) {
            Some(next) => self.entries[i].1 = next,
            None => {
                self.entries.remove(i);
            }
        }
        self.alist = Some(new);
    }
}
