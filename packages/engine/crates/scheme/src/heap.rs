//! The heap (chapter 03 §5.1): the symbol table, typed arenas in chunks with
//! an epoch per chunk, foreign objects, the root stack, and a precise,
//! non-moving mark-sweep collector that runs at safe points only, so it never
//! has to scan wasm's stack. Also `equal?` and Guile's printer, which walk
//! the same objects. The numbers kept here are number.rs's.
//!
//! R11's write barrier and undo log (03 §5.1, MS6 step 2): while a toplevel
//! item runs ([`Heap::begin_item`]), the first change in it of each object
//! older than the item (its chunk's epoch before the item's) is logged with
//! what the object was: a pair's car and cdr, a string's or a vector's
//! contents, a foreign object's [`Foreign::snapshot`]. [`Heap::undo`] gives
//! them back, the latest first. Every change goes through the accessors
//! that give a mutable object ([`Heap::set_car`], [`Heap::set_cdr`],
//! [`Heap::str_mut`], [`Heap::vector_items_mut`], [`Heap::foreign_mut`]),
//! so the barrier is in them and nowhere else. A logged object and what it
//! held are roots until the entry is undone or dropped, so no sweep frees a
//! slot the log will write to.
//!
//! The redo half (02 §5.7's reuse, MS6 step 3): [`Heap::undo_keeping`]
//! undoes an item as [`Heap::undo`] does and keeps what each object it gives
//! back was at the item's end, which [`Heap::redo`] puts back in a later
//! item, through the barrier, so that a parse's items before an edit are
//! done again without being parsed again. What it keeps is a root until
//! [`Heap::forget_redo`].

use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::cell::Cell;
use core::fmt::Write;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::bignum::BigInt;
use crate::compile::{Op, Template};
use crate::image::{ImageError, ImageReader, ImageWriter, Loader, Templates};
use crate::number;
use crate::prims::{self, Prim};
use crate::value::{Str, Tag, Value, write_char, write_string, write_symbol};

/// Objects in one chunk of an arena. All objects of a chunk share its epoch.
pub const CHUNK: usize = 256;
const WORDS: usize = CHUNK / 64;

/// Collect when this many bytes have been allocated since the last
/// collection, or as many as were live after it, whichever is more.
const MIN_THRESHOLD: usize = 4 << 20;

/// Entries in a block of the collector's stack, and in its `recent`.
const GRAY_BLOCK: usize = 1 << 16;
const GRAY_RECENT: usize = 1 << 12;

// --- arenas ------------------------------------------------------------------

struct Chunk<T> {
    /// The epoch the chunk was opened in: no object in it is younger.
    epoch: u32,
    live: [u64; WORDS],
    marks: [u64; WORDS],
    /// The epoch each object put into the chunk after it was opened was
    /// put in (MS6 step 3; 0 for none), once one is: those objects report
    /// it, so the write barrier does not log what the item allocated
    /// (D235), and N8 tells a translation's objects from older ones (D241).
    young: Option<Box<[u32; CHUNK]>>,
    /// [`CHUNK`] slots, or none in a chunk an image left empty, until it
    /// is opened (MS3 step 56).
    slots: Box<[T]>,
}

/// Objects of one type, addressed by index: chunk × [`CHUNK`] + slot. A
/// dead slot holds `T::default()`, so what it owned is dropped when it dies.
struct Arena<T> {
    chunks: Vec<Chunk<T>>,
    /// Free slots in chunks of the current epoch, the lowest index last.
    free: Vec<u32>,
    /// Chunks the last sweep left empty, to be opened again in any epoch.
    empty: Vec<u32>,
    /// Free slots in chunks of earlier epochs.
    old: Vec<u32>,
    count: usize,
}

/// In a free list, a chunk whose free slots are listed when it is reached
/// ([`Arena::take`]), not when the list is made: `PENDING | chunk`.
const PENDING: u32 = 1 << 31;

const fn split(index: u32) -> (usize, usize) {
    (index as usize / CHUNK, index as usize % CHUNK)
}

/// Pushes the free slots of `chunk` onto `list`, the lowest last.
fn list_free(list: &mut Vec<u32>, c: usize, live: &[u64; WORDS]) {
    let base = (c * CHUNK) as u32;
    for word in (0..WORDS).rev() {
        let mut free = !live[word];
        while free != 0 {
            let bit = 63 - free.leading_zeros();
            list.push(base + (word * 64) as u32 + bit);
            free &= !(1 << bit);
        }
    }
}

impl<T: Default> Arena<T> {
    const fn new() -> Self {
        Arena {
            chunks: Vec::new(),
            free: Vec::new(),
            empty: Vec::new(),
            old: Vec::new(),
            count: 0,
        }
    }

    /// Stores `value`, preferring, in order: a free slot of the current
    /// epoch's chunks, an empty chunk (which takes the current epoch), a free
    /// slot of an older chunk, a new chunk. An object put into an older chunk
    /// reports that chunk's epoch, which is never later than its own: the
    /// write barrier may then log a change to it that was not needed, but
    /// never misses one.
    fn alloc(&mut self, epoch: u32, value: T) -> u32 {
        let index = match Arena::take(&mut self.free, &self.chunks) {
            Some(index) => index,
            None => match self.empty.pop() {
                Some(chunk) => {
                    let opened = &mut self.chunks[chunk as usize];
                    opened.epoch = epoch;
                    opened.young = None;
                    self.open(chunk as usize)
                }
                None => match Arena::take(&mut self.old, &self.chunks) {
                    // An older chunk's slot: the object is this epoch's.
                    Some(index) => {
                        let (c, slot) = split(index);
                        let chunk = &mut self.chunks[c];
                        chunk.young.get_or_insert_with(|| Box::new([0; CHUNK]))[slot] = epoch;
                        index
                    }
                    None => {
                        let slots = (0..CHUNK).map(|_| T::default()).collect();
                        self.chunks.push(Chunk {
                            epoch,
                            live: [0; WORDS],
                            marks: [0; WORDS],
                            young: None,
                            slots,
                        });
                        self.open(self.chunks.len() - 1)
                    }
                },
            },
        };
        let (chunk, slot) = split(index);
        let chunk = &mut self.chunks[chunk];
        chunk.live[slot / 64] |= 1 << (slot % 64);
        chunk.slots[slot] = value;
        self.count += 1;
        index
    }

    /// The free slot `list` gives next, a chunk it reaches listed first.
    /// Nothing takes from a list but this, so a chunk's free slots are
    /// still those its live bits had when it was put on the list.
    fn take(list: &mut Vec<u32>, chunks: &[Chunk<T>]) -> Option<u32> {
        loop {
            let top = list.pop()?;
            if top & PENDING == 0 {
                return Some(top);
            }
            let c = (top & !PENDING) as usize;
            list_free(list, c, &chunks[c].live);
        }
    }

    /// Frees every slot of `chunk` but the first, which it returns.
    fn open(&mut self, chunk: usize) -> u32 {
        let slots = &mut self.chunks[chunk].slots;
        if slots.is_empty() {
            *slots = (0..CHUNK).map(|_| T::default()).collect();
        }
        let base = (chunk * CHUNK) as u32;
        self.free
            .extend((1..CHUNK as u32).rev().map(|slot| base + slot));
        base
    }

    /// Moves the free slots of the epoch that ends among the old ones.
    fn retire(&mut self) {
        self.old.append(&mut self.free);
    }

    fn get(&self, index: u32) -> &T {
        let (chunk, slot) = split(index);
        let chunk = &self.chunks[chunk];
        debug_assert!(
            chunk.live[slot / 64] & (1 << (slot % 64)) != 0,
            "a dead object"
        );
        &chunk.slots[slot]
    }

    fn get_mut(&mut self, index: u32) -> &mut T {
        let (chunk, slot) = split(index);
        let chunk = &mut self.chunks[chunk];
        debug_assert!(
            chunk.live[slot / 64] & (1 << (slot % 64)) != 0,
            "a dead object"
        );
        &mut chunk.slots[slot]
    }

    /// The epoch the object at `index` was allocated in: its chunk's, or
    /// a later one it was put into the chunk in.
    fn epoch(&self, index: u32) -> u32 {
        let (chunk, slot) = split(index);
        let chunk = &self.chunks[chunk];
        match &chunk.young {
            Some(young) if young[slot] != 0 => young[slot].max(chunk.epoch),
            _ => chunk.epoch,
        }
    }

    fn is_marked(&self, index: u32) -> bool {
        let (chunk, slot) = split(index);
        self.chunks[chunk].marks[slot / 64] & (1 << (slot % 64)) != 0
    }

    /// Marks the object; true if it was not marked yet.
    fn mark(&mut self, index: u32) -> bool {
        let (chunk, slot) = split(index);
        let word = &mut self.chunks[chunk].marks[slot / 64];
        let bit = 1 << (slot % 64);
        let new = *word & bit == 0;
        *word |= bit;
        new
    }

    /// Frees what is not marked and clears the marks, of up to `parts` chunks
    /// from `from`: the bytes still live in them, by `size`, and the next
    /// chunk, or `None` when [`Arena::relist`] has rebuilt the free lists.
    fn sweep_part(
        &mut self,
        (from, parts): (usize, usize),
        epoch: u32,
        size: fn(&T) -> usize,
    ) -> (usize, Option<usize>) {
        let mut bytes = 0;
        for chunk in self.chunks.iter_mut().skip(from).take(parts) {
            for word in 0..WORDS {
                let mut dead = chunk.live[word] & !chunk.marks[word];
                chunk.live[word] &= chunk.marks[word];
                chunk.marks[word] = 0;
                while dead != 0 {
                    let slot = word * 64 + dead.trailing_zeros() as usize;
                    if let Some(young) = &mut chunk.young {
                        young[slot] = 0;
                    }
                    chunk.slots[slot] = T::default();
                    dead &= dead - 1;
                }
                let mut live = chunk.live[word];
                while live != 0 {
                    bytes += size(&chunk.slots[word * 64 + live.trailing_zeros() as usize]);
                    live &= live - 1;
                }
            }
        }
        if from + parts < self.chunks.len() {
            return (bytes, Some(from + parts));
        }
        self.relist(epoch);
        (bytes, None)
    }

    /// Forgets every mark: a collection dropped halfway.
    fn clear_marks(&mut self) {
        for chunk in &mut self.chunks {
            chunk.marks = [0; WORDS];
        }
    }

    /// Gives the allocator back the slots of the chunks left empty, as an
    /// image leaves them until they are opened (D259).
    fn trim(&mut self) {
        for &c in &self.empty {
            let chunk = &mut self.chunks[c as usize];
            chunk.slots = Box::default();
            chunk.young = None;
        }
    }

    /// Rebuilds the free lists and the count from the live bits: each
    /// chunk with a free slot as one entry, its slots listed when an
    /// allocation reaches it (MS3 step 56, D215).
    fn relist(&mut self, epoch: u32) {
        self.free.clear();
        self.empty.clear();
        self.old.clear();
        self.count = 0;
        // From the last chunk, so that the lowest index pops first.
        for (c, chunk) in self.chunks.iter().enumerate().rev() {
            let alive: usize = chunk.live.iter().map(|w| w.count_ones() as usize).sum();
            self.count += alive;
            if alive == 0 {
                self.empty.push(c as u32);
            } else if alive < CHUNK {
                let list = if chunk.epoch == epoch {
                    &mut self.free
                } else {
                    &mut self.old
                };
                list.push(PENDING | c as u32);
            }
        }
    }

    /// Writes each chunk's epoch and live bits, then `item` of each object
    /// that lives in it, in order.
    fn save(
        &self,
        w: &mut ImageWriter,
        mut item: impl FnMut(&T, &mut ImageWriter) -> Result<(), ImageError>,
    ) -> Result<(), ImageError> {
        w.count(self.chunks.len());
        for chunk in &self.chunks {
            w.u32(chunk.epoch);
            for (word, &bits) in chunk.live.iter().enumerate() {
                w.u64(bits);
                let mut live = bits;
                while live != 0 {
                    item(&chunk.slots[word * 64 + live.trailing_zeros() as usize], w)?;
                    live &= live - 1;
                }
            }
        }
        Ok(())
    }

    /// The arena [`Arena::save`] wrote, each object at its index, the free
    /// lists made as a sweep in `epoch` makes them.
    fn load(
        r: &mut ImageReader<'_>,
        epoch: u32,
        mut item: impl FnMut(&mut ImageReader<'_>) -> Option<T>,
    ) -> Option<Arena<T>> {
        let mut arena = Arena::new();
        let n = r.count()?;
        arena.chunks.reserve_exact(n);
        for _ in 0..n {
            let epoch = r.u32()?;
            let mut live = [0; WORDS];
            // The slots made dead at once, none for an empty chunk, then
            // each live one read into its place in a plain loop: no call a
            // slot (MS3 step 56).
            let mut slots: Vec<T> = Vec::new();
            let mut word = 0;
            while word < WORDS {
                let bits = r.u64()?;
                live[word] = bits;
                if bits != 0 && slots.is_empty() {
                    slots.reserve_exact(CHUNK);
                    slots.resize_with(CHUNK, T::default);
                }
                let mut rest = bits;
                while rest != 0 {
                    slots[word * 64 + rest.trailing_zeros() as usize] = item(r)?;
                    rest &= rest - 1;
                }
                word += 1;
            }
            arena.chunks.push(Chunk {
                epoch,
                live,
                marks: [0; WORDS],
                young: None,
                slots: slots.into(),
            });
        }
        arena.relist(epoch);
        Some(arena)
    }
}

// --- symbols -------------------------------------------------------------------

/// Interned names: a symbol's and a keyword's payload is the index of its
/// name. Names are never freed. An uninterned symbol has a name but is not
/// in the table, so no other symbol is it.
struct Symbols {
    names: Names,
    /// Open addressing: indices into `names`, `u32::MAX` where empty.
    table: Vec<u32>,
    /// Whether each name is out of the table.
    uninterned: Vec<bool>,
    /// The names interned last, by the address of the text they were asked
    /// with, made when the first is: a name the program holds, as a literal,
    /// is found again without hashing it, as LilyPond keeps the symbol of
    /// each `ly_symbol2scm` call. The text is compared with the name kept.
    recent: Vec<(usize, u32)>,
}

/// The symbols' names end to end in one text, each found by where it ends,
/// so that a restore makes no allocation a name (MS3 step 56, D215).
#[derive(Default)]
struct Names {
    text: String,
    ends: Vec<u32>,
}

impl Names {
    #[inline(always)]
    fn len(&self) -> usize {
        self.ends.len()
    }

    fn push(&mut self, name: &str) {
        self.text.push_str(name);
        self.ends.push(self.text.len() as u32);
    }

    #[inline(always)]
    fn get(&self, index: usize) -> &str {
        let start = match index {
            0 => 0,
            i => self.ends[i - 1] as usize,
        };
        &self.text[start..self.ends[index] as usize]
    }

    fn iter(&self) -> impl Iterator<Item = &str> {
        (0..self.len()).map(|index| self.get(index))
    }
}

/// The places of [`Symbols::recent`], a power of two.
const RECENT: usize = 1024;

/// FNV-1a over the name four bytes at a time, as little-endian words, the
/// last one padded with zeros, then mixed as MurmurHash3's `fmix32` does,
/// so that every byte reaches the low bits the table takes: a quarter of
/// FNV-1a's steps for the restore's rebuild of the table (MS3 step 56).
#[inline(always)]
fn hash(name: &str) -> usize {
    let bytes = name.as_bytes();
    let mut h = 0x811c_9dc5_u32;
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let word = [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]];
        h = (h ^ u32::from_le_bytes(word)).wrapping_mul(0x0100_0193);
        i += 4;
    }
    if i < bytes.len() {
        let mut word = 0;
        let mut shift = 0;
        while i < bytes.len() {
            word |= (bytes[i] as u32) << shift;
            shift += 8;
            i += 1;
        }
        h = (h ^ word).wrapping_mul(0x0100_0193);
    }
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    (h ^ h >> 16) as usize
}

impl Symbols {
    fn intern(&mut self, name: &str) -> u32 {
        let address = name.as_ptr() as usize;
        let place = (address as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) >> (64 - RECENT.ilog2());
        match self.recent.get(place as usize) {
            Some(&(a, index)) if a == address && self.names.get(index as usize) == name => {
                return index;
            }
            Some(_) => {}
            None => self.recent = alloc::vec![(0, u32::MAX); RECENT],
        }
        let index = self.find_or_add(name);
        self.recent[place as usize] = (address, index);
        index
    }

    fn find_or_add(&mut self, name: &str) -> u32 {
        if (self.names.len() + 1) * 2 > self.table.len() {
            self.rebuild((self.table.len() * 2).max(1024));
        }
        let mask = self.table.len() - 1;
        let mut at = hash(name) & mask;
        loop {
            match self.table[at] {
                u32::MAX => break,
                index if self.names.get(index as usize) == name => return index,
                _ => at = (at + 1) & mask,
            }
        }
        let index = self.names.len() as u32;
        self.names.push(name);
        self.uninterned.push(false);
        self.table[at] = index;
        index
    }

    /// Makes the table again with `size` places, a power of two.
    fn rebuild(&mut self, size: usize) {
        let mut table = alloc::vec![u32::MAX; size];
        let mask = size - 1;
        // A plain loop, the restore's (MS3 step 56).
        let mut index = 0;
        while index < self.names.len() {
            if !self.uninterned[index] {
                let mut at = hash(self.names.get(index)) & mask;
                while table[at] != u32::MAX {
                    at = (at + 1) & mask;
                }
                table[at] = index as u32;
            }
            index += 1;
        }
        self.table = table;
    }
}

// --- foreign objects -----------------------------------------------------------

/// A Rust value with a Scheme face (R12): pitches, durations, moments, music,
/// contexts, grobs, stencils, output definitions. The heap owns it and drops
/// it when it is collected.
pub trait Foreign: Any {
    /// The name its type predicate and its type errors use, as LilyPond's
    /// smobs name theirs: `Pitch`, `Music`.
    fn type_name(&self) -> &'static str;

    /// How `write` and `display` print it; `#<Pitch c' >` for a pitch.
    fn print(&self, heap: &Heap, out: &mut String) {
        let _ = heap;
        out.push_str("#<");
        out.push_str(self.type_name());
        out.push('>');
    }

    /// `equal?` with another foreign object, of any type: `other` must be
    /// downcast. Without it, two foreign objects are `equal?` only if `eq?`.
    fn equal(&self, other: &dyn Foreign, heap: &Heap) -> bool {
        let _ = (other, heap);
        false
    }

    /// Hands every Scheme value it holds to `tracer`, so the collector keeps
    /// them; a value it holds and does not trace may be freed.
    fn trace(&self, tracer: &mut Tracer<'_>) {
        let _ = tracer;
    }

    /// For an object made with [`Heap::wrap_weak`]: after marking, drops
    /// what it holds weakly (and did not trace) that `live` says is dead.
    fn prune(&mut self, live: &dyn Fn(Value) -> bool) {
        let _ = live;
    }

    /// Writes what it holds into an image (S4), for the [`Loader`] named
    /// as its type to read back; false, the default, for a type that no
    /// image holds, which fails the write.
    fn save(&self, out: &mut ImageWriter) -> bool {
        let _ = out;
        false
    }

    /// For an object that is a procedure, as Guile's applicable smobs are
    /// (LilyPond's music functions): the procedure a call of it calls, with
    /// it before the call's arguments. `procedure?` is true of it.
    fn applicable(&self) -> Option<Value> {
        None
    }

    /// A copy of it for R11's undo log, which [`Heap::undo`] puts back in
    /// its place; `None`, the default, for a type whose changes are not
    /// undone (a port, whose text the host takes, or another crate's).
    fn snapshot(&self) -> Option<Box<dyn Foreign>> {
        None
    }

    /// Whether `other`, a [`Foreign::snapshot`] of it, holds what it
    /// holds: an undo log's object given back what it was (MS6 step 3).
    /// False, the default, for a type that cannot tell.
    fn same(&self, _other: &dyn Foreign) -> bool {
        false
    }
}

/// What a foreign object's [`Foreign::trace`] gives its values to.
/// The collector's stack, in blocks: it comes to millions of values on a long
/// score, all at once, when memory is shortest, and a [`Vec`] of them doubled
/// to 64 MiB, one block, and kept that. A block goes as it empties.
///
/// Most of what a translation's records hold is held by many (a definition's
/// alist, an event, a context's handle), and they push it all before one is
/// popped: `recent` leaves out a value pushed lately, which this collection
/// has marked or will. It holds each value's bits at a place of its own by
/// its index, so that runs of objects do not fall on each other.
#[derive(Default)]
pub(crate) struct Gray {
    top: Vec<Value>,
    full: Vec<Vec<Value>>,
    recent: Vec<u64>,
}

impl Gray {
    /// A collection begins: what was pushed in the last is not marked in it.
    fn begin(&mut self) {
        self.recent.clear();
        self.recent.resize(GRAY_RECENT, 0);
    }

    #[inline]
    pub(crate) fn push(&mut self, value: Value) {
        let bits = value.to_bits();
        let slot = ((bits >> 32) ^ (bits >> 1) << 7) as usize & (GRAY_RECENT - 1);
        if self.recent[slot] == bits {
            return;
        }
        self.recent[slot] = bits;
        if self.top.len() == GRAY_BLOCK {
            self.spill();
        }
        self.top.push(value);
    }

    #[cold]
    fn spill(&mut self) {
        let block = Vec::with_capacity(GRAY_BLOCK);
        self.full.push(core::mem::replace(&mut self.top, block));
    }

    fn pop(&mut self) -> Option<Value> {
        if self.top.is_empty() {
            self.top = self.full.pop()?;
        }
        self.top.pop()
    }

    /// A value popped and not marked yet, back: `push` might leave it out.
    fn push_back(&mut self, value: Value) {
        if self.top.len() == GRAY_BLOCK {
            self.spill();
        }
        self.top.push(value);
    }

    pub(crate) fn extend(&mut self, values: impl IntoIterator<Item = Value>) {
        for value in values {
            self.push(value);
        }
    }

    /// Forgets what it holds: a collection dropped halfway.
    fn clear(&mut self) {
        self.top.clear();
        self.full.clear();
    }
}

pub struct Tracer<'a> {
    /// The running collection's number.
    collection: u32,
    gray: &'a mut Gray,
    modules: &'a mut Vec<u32>,
}

impl Tracer<'_> {
    pub fn trace(&mut self, value: Value) {
        if value.is_heap() {
            self.gray.push(value);
        }
    }

    /// Whether this collection has not come to what `stamp` is of yet: what
    /// many objects share is traced once ([`crate::Rows`]). A stamp starts
    /// at 0, no collection's number.
    pub(crate) fn first_time(&mut self, stamp: &Cell<u32>) -> bool {
        stamp.replace(self.collection) != self.collection
    }

    /// The module of that index is live: its object or a macro of it is.
    pub(crate) fn module(&mut self, index: u32) {
        self.modules.push(index);
    }
}

// --- the heap --------------------------------------------------------------------

#[derive(Default)]
struct Pair {
    car: Value,
    cdr: Value,
}

/// An exact ratio in lowest terms: fixnums or bignums, the denominator
/// above 1.
#[derive(Default)]
struct Ratio {
    numerator: Value,
    denominator: Value,
}

/// A procedure made by `lambda`: its code, which all closures of the
/// `lambda` share, and the values of its free variables (chapter 03 §5.3).
#[derive(Default)]
pub(crate) struct Closure {
    pub code: Option<Rc<Template>>,
    pub free: Box<[Value]>,
}

/// What an object was before its first change in an item (R11), or at
/// the end of an item undone ([`Heap::undo_keeping`]).
enum Old {
    Pair(Value, Value),
    Str(String),
    Vector(Vec<Value>),
    Foreign(Box<dyn Foreign>),
}

/// A generation of N8's pictures: log entries with their places in their
/// compile's log.
type Pictures = Vec<(usize, Value, Old)>;

/// A copy of a picture: `None` for a foreign object that gives none.
fn copy_old(old: &Old) -> Option<Old> {
    Some(match old {
        Old::Pair(car, cdr) => Old::Pair(*car, *cdr),
        Old::Str(text) => Old::Str(text.clone()),
        Old::Vector(items) => Old::Vector(items.clone()),
        Old::Foreign(object) => Old::Foreign(object.snapshot()?),
    })
}

/// A table of values by their bits, for what looks up every entry of the
/// undo log: open addressing, a slot's key 0 for none.
struct BitsMap<'a> {
    keys: Vec<u64>,
    olds: Vec<Option<&'a Old>>,
    len: usize,
}

impl<'a> BitsMap<'a> {
    fn new() -> Self {
        BitsMap {
            keys: alloc::vec![0; 1 << 12],
            olds: alloc::vec![None; 1 << 12],
            len: 0,
        }
    }

    /// `key`'s slot: where it is, or the empty one it would take.
    fn slot(&self, key: u64) -> usize {
        let mask = self.keys.len() - 1;
        let mut at = key.wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(23) as usize & mask;
        while self.keys[at] != 0 && self.keys[at] != key {
            at = (at + 1) & mask;
        }
        at
    }

    fn get(&self, key: u64) -> Option<Option<&'a Old>> {
        let at = self.slot(key);
        (self.keys[at] == key).then(|| self.olds[at])
    }

    fn insert(&mut self, key: u64, old: Option<&'a Old>) {
        if self.len * 2 >= self.keys.len() {
            let size = self.keys.len() * 2;
            let keys = core::mem::replace(&mut self.keys, alloc::vec![0; size]);
            let olds = core::mem::replace(&mut self.olds, alloc::vec![None; size]);
            for (k, o) in keys.into_iter().zip(olds).filter(|e| e.0 != 0) {
                let at = self.slot(k);
                self.keys[at] = k;
                self.olds[at] = o;
            }
        }
        let at = self.slot(key);
        self.len += usize::from(self.keys[at] == 0);
        self.keys[at] = key;
        self.olds[at] = old;
    }
}

/// R11's undo log: what [`Heap::undo`] gives back, oldest first.
#[derive(Default)]
struct Undo {
    /// The current item's epoch while the barrier is on.
    item: Option<u32>,
    log: Vec<(Value, Old)>,
    /// The objects the current item changed that are older than it, by
    /// their bits: logged once, at their first change.
    seen: BTreeSet<u64>,
    /// What the objects of the items undone by [`Heap::undo_keeping`] were
    /// at each item's end, an item's in a range of its own, oldest first.
    redo: Vec<(Value, Old)>,
    /// Values kept alive with them ([`Heap::keep_for_redo`]).
    kept: Vec<Value>,
    /// N8's kept pictures (D241): log entries taken out of a compile's log
    /// before its undo ([`Heap::keep_pictures`]), each generation (a
    /// compile's) by its number with the entries' places in that log,
    /// oldest first; the number the next one takes; the epochs of the
    /// compiles whose objects they keep (kept translations' own).
    pictures: Vec<(u32, Pictures)>,
    next_generation: u32,
    lineage: Vec<(u32, u32)>,
    /// Values kept alive between compiles ([`Heap::hold_between`]).
    held: Vec<Value>,
    /// What the objects given back in the running compile held before it
    /// ([`Heap::give_back`]), which a stop puts back.
    given: Vec<(Value, Old)>,
}

/// A place on the root stack, from [`Heap::root`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Root(u32);

/// The microseconds every heap of the process has spent collecting, which
/// 05 §7's steps leave out of a step's time (`queue.rs` in layout, D251).
static COLLECTING_US: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// [`COLLECTING_US`] in milliseconds.
pub fn collecting_ms() -> f64 {
    COLLECTING_US.load(core::sync::atomic::Ordering::Relaxed) as f64 / 1000.0
}

/// The collector's counts, for the bench export and the tests. The pauses
/// are timed by the host's clock ([`Heap::set_clock`]), and are 0 without one.
#[derive(Clone, Copy, Default, Debug)]
pub struct Stats {
    pub collections: u32,
    /// Objects and their bytes, estimated, live after the last collection.
    pub live: usize,
    pub live_bytes: usize,
    /// Objects the last collection freed.
    pub freed: usize,
    /// Milliseconds: the longest collection, and all of them.
    pub longest_pause_ms: f64,
    pub pause_ms: f64,
}

/// A collection under way (B20, D261). A compile's steps can leave it for the
/// host in the middle, with no barrier and no incremental mark: nothing runs on
/// the heap between two `lily_step`s, so what it has marked stays true, and a
/// newer edit drops it ([`Heap::drop_collecting`]).
struct Collecting {
    /// The live objects before it, and the milliseconds it has taken.
    before: usize,
    ms: f64,
    /// `None` while it marks; then the arena and the chunk it sweeps next and
    /// the bytes it found live so far.
    sweeping: Option<(usize, usize, usize)>,
}

/// The bytes a collection waits for a step's end for: what a step of 10 ms
/// allocates, and little enough for what it costs to stay small. What it waits
/// for grows the arenas, whose free slots count as used, for good: at twice
/// the threshold it took T29 × 8 over the cap (D261).
const DEFERRED: usize = 4 << 20;

/// Every how many procedure entries the VM collects whatever was allocated
/// ([`set_collect_every`], OPEN H15, D268); 0, by what was allocated alone.
static COLLECT_EVERY: AtomicU32 = AtomicU32::new(0);

/// Makes every `n`th procedure entry of each VM of the process a collection,
/// 1 each, 0 none beyond what allocation asks for: a value Rust holds unrooted
/// across a call into Scheme (OPEN H15) is then swept at the first call that
/// reaches one, not only as timing allows. Natively only, for the tests and
/// the compile example's `--collect-every`; nothing sets it in wasm.
pub fn set_collect_every(n: u32) {
    COLLECT_EVERY.store(n, Ordering::Relaxed);
}

/// The chunks a collection sweeps between looks at the clock.
const SWEEP_PART: usize = 32;

/// The objects a collection marks between looks at the clock.
const MARK_PART: usize = 1 << 12;

/// Every Scheme object, and the roots Rust holds.
///
/// Allocation never collects. The interpreter calls [`Heap::safe_point`]
/// where no value lives only in Rust variables: at a procedure's entry,
/// between toplevel forms, at the yield points of B8. Rust code that holds
/// values across one pushes them on the root stack ([`Heap::root`]) and pops
/// them after ([`Heap::unroot_to`]). A compile in steps collects between them
/// instead, as long as a step takes ([`Heap::set_deferring`], B20, D261); a
/// safe point then collects only when memory is short or a step has run on
/// past [`DEFERRED`] more.
pub struct Heap {
    symbols: Symbols,
    pairs: Arena<Pair>,
    strings: Arena<Str>,
    vectors: Arena<Vec<Value>>,
    foreign: Arena<Option<Box<dyn Foreign>>>,
    flonums: Arena<f64>,
    bignums: Arena<BigInt>,
    ratios: Arena<Ratio>,
    complexes: Arena<[f64; 2]>,
    closures: Arena<Closure>,
    values: Arena<Vec<Value>>,
    /// The foreign objects that hold values weakly ([`Foreign::prune`]).
    weak: Vec<u32>,
    roots: Vec<Value>,
    gray: Gray,
    /// What the marking reached besides objects, for the VM to keep: the
    /// modules ([`Tracer::module`]) and the global slots compiled code names.
    traced_modules: Vec<u32>,
    traced_slots: Vec<u32>,
    epoch: u32,
    undo: Undo,
    allocated: usize,
    threshold: usize,
    /// Whether the next sweep gives back the chunks it leaves empty, and
    /// whether a compile is to stop for want of memory.
    trim: bool,
    over_memory: bool,
    /// While above 0, no safe point collects (`hold`).
    held: u32,
    /// The procedure entries since the last forced by [`set_collect_every`].
    entries: u32,
    /// Whether what runs returns between its steps, which is where it
    /// collects (B20, D261), and the collection under way between them.
    deferring: bool,
    collecting: Option<Collecting>,
    stats: Stats,
    clock: Option<fn() -> f64>,
    /// The primitives other crates defined ([`crate::Vm::define_primitives`]),
    /// numbered after the crate's own: here so that the printer finds their
    /// names.
    pub(crate) natives: Vec<&'static Prim>,
    /// The methods of another crate's types for `+`, `-`, `*`, `/` and the
    /// comparisons ([`crate::Extension::operators`]).
    pub(crate) operators: Option<crate::OperatorFn>,
    /// The code the image held, by address ([`Heap::is_image_code`]).
    image_code: BTreeSet<usize>,
}

impl Default for Heap {
    fn default() -> Heap {
        Heap::new()
    }
}

impl Heap {
    pub fn new() -> Heap {
        Heap {
            symbols: Symbols {
                names: Names::default(),
                table: Vec::new(),
                uninterned: Vec::new(),
                recent: Vec::new(),
            },
            pairs: Arena::new(),
            strings: Arena::new(),
            vectors: Arena::new(),
            foreign: Arena::new(),
            flonums: Arena::new(),
            bignums: Arena::new(),
            ratios: Arena::new(),
            complexes: Arena::new(),
            closures: Arena::new(),
            values: Arena::new(),
            weak: Vec::new(),
            roots: Vec::new(),
            gray: Gray::default(),
            traced_modules: Vec::new(),
            traced_slots: Vec::new(),
            epoch: 0,
            undo: Undo::default(),
            allocated: 0,
            threshold: MIN_THRESHOLD,
            trim: false,
            over_memory: false,
            held: 0,
            entries: 0,
            deferring: false,
            collecting: None,
            stats: Stats::default(),
            clock: None,
            natives: Vec::new(),
            operators: None,
            image_code: BTreeSet::new(),
        }
    }

    /// The host's clock in milliseconds, which times each collection.
    pub fn set_clock(&mut self, clock: fn() -> f64) {
        self.clock = Some(clock);
    }

    // --- symbols and keywords

    /// The symbol named `name`, `string->symbol`.
    pub fn intern(&mut self, name: &str) -> Value {
        Value::tagged(Tag::Symbol, self.symbols.intern(name))
    }

    /// The symbol named by a string, `string->symbol` without copying the
    /// text; `None` if `string` is not a string.
    pub fn intern_string(&mut self, string: Value) -> Option<Value> {
        if string.tag() != Tag::String {
            return None;
        }
        let text = self.strings.get(string.payload()).as_str();
        Some(Value::tagged(Tag::Symbol, self.symbols.intern(text)))
    }

    /// A new symbol named `name` that is not interned: it prints as `name`
    /// but is no other symbol, as the expander's renamed identifiers are.
    pub fn uninterned(&mut self, name: &str) -> Value {
        let s = &mut self.symbols;
        s.names.push(name);
        s.uninterned.push(true);
        Value::tagged(Tag::Symbol, s.names.len() as u32 - 1)
    }

    /// The keyword `#:name`.
    pub fn keyword(&mut self, name: &str) -> Value {
        Value::tagged(Tag::Keyword, self.symbols.intern(name))
    }

    /// A symbol's or a keyword's name.
    pub fn name(&self, value: Value) -> Option<&str> {
        match value.tag() {
            Tag::Symbol | Tag::Keyword => Some(self.symbols.names.get(value.payload() as usize)),
            _ => None,
        }
    }

    // --- pairs

    pub fn cons(&mut self, car: Value, cdr: Value) -> Value {
        self.allocated += size_of::<Pair>();
        Value::tagged(Tag::Pair, self.pairs.alloc(self.epoch, Pair { car, cdr }))
    }

    /// A proper list of `items`.
    pub fn list(&mut self, items: &[Value]) -> Value {
        items
            .iter()
            .rev()
            .fold(Value::NULL, |tail, &item| self.cons(item, tail))
    }

    pub fn car(&self, pair: Value) -> Option<Value> {
        (pair.tag() == Tag::Pair).then(|| self.pairs.get(pair.payload()).car)
    }

    pub fn cdr(&self, pair: Value) -> Option<Value> {
        (pair.tag() == Tag::Pair).then(|| self.pairs.get(pair.payload()).cdr)
    }

    /// `scm_sloppy_assq`: the first entry of `alist` that is a pair whose
    /// car is `key`, the list read until it is no pair; each pair read once.
    pub fn assq(&self, key: Value, alist: Value) -> Option<Value> {
        let mut p = alist;
        while p.tag() == Tag::Pair {
            let pair = self.pairs.get(p.payload());
            let entry = pair.car;
            if entry.tag() == Tag::Pair && self.pairs.get(entry.payload()).car == key {
                return Some(entry);
            }
            p = pair.cdr;
        }
        None
    }

    /// `set-car!`; false if `pair` is not a pair.
    pub fn set_car(&mut self, pair: Value, value: Value) -> bool {
        let is = pair.tag() == Tag::Pair;
        if is {
            self.barrier(pair);
            self.pairs.get_mut(pair.payload()).car = value;
        }
        is
    }

    /// `set-cdr!`; false if `pair` is not a pair.
    pub fn set_cdr(&mut self, pair: Value, value: Value) -> bool {
        let is = pair.tag() == Tag::Pair;
        if is {
            self.barrier(pair);
            self.pairs.get_mut(pair.payload()).cdr = value;
        }
        is
    }

    // --- strings and vectors

    pub fn string(&mut self, text: impl Into<String>) -> Value {
        let text = text.into();
        self.allocated += size_of::<Str>() + text.len();
        Value::tagged(Tag::String, self.strings.alloc(self.epoch, Str::new(text)))
    }

    pub fn str(&self, string: Value) -> Option<&Str> {
        (string.tag() == Tag::String).then(|| self.strings.get(string.payload()))
    }

    pub fn str_mut(&mut self, string: Value) -> Option<&mut Str> {
        if string.tag() != Tag::String {
            return None;
        }
        self.barrier(string);
        Some(self.strings.get_mut(string.payload()))
    }

    pub fn vector(&mut self, items: Vec<Value>) -> Value {
        self.allocated += size_of::<Vec<Value>>() + items.len() * size_of::<Value>();
        Value::tagged(Tag::Vector, self.vectors.alloc(self.epoch, items))
    }

    pub fn vector_items(&self, vector: Value) -> Option<&[Value]> {
        (vector.tag() == Tag::Vector).then(|| &self.vectors.get(vector.payload())[..])
    }

    pub fn vector_items_mut(&mut self, vector: Value) -> Option<&mut [Value]> {
        if vector.tag() != Tag::Vector {
            return None;
        }
        self.barrier(vector);
        Some(&mut self.vectors.get_mut(vector.payload())[..])
    }

    // --- foreign objects

    /// Gives `object` to the heap, which drops it when it is collected.
    pub fn wrap<T: Foreign>(&mut self, object: T) -> Value {
        self.allocated += size_of::<T>() + size_of::<Box<dyn Foreign>>();
        Value::tagged(
            Tag::Foreign,
            self.foreign.alloc(self.epoch, Some(Box::new(object))),
        )
    }

    /// [`Heap::wrap`] for an object that holds values weakly: each
    /// collection prunes it after marking ([`Foreign::prune`]).
    pub fn wrap_weak<T: Foreign>(&mut self, object: T) -> Value {
        let value = self.wrap(object);
        self.weak.push(value.payload());
        value
    }

    pub fn foreign_object(&self, value: Value) -> Option<&dyn Foreign> {
        if value.tag() != Tag::Foreign {
            return None;
        }
        self.foreign.get(value.payload()).as_deref()
    }

    /// The foreign object if it is a `T`: the type predicate.
    pub fn foreign<T: Foreign>(&self, value: Value) -> Option<&T> {
        let object: &dyn Any = self.foreign_object(value)?;
        object.downcast_ref()
    }

    /// The foreign object if it is a `T`, to change: logged first by the
    /// write barrier, whatever its type.
    pub fn foreign_mut<T: Foreign>(&mut self, value: Value) -> Option<&mut T> {
        if value.tag() != Tag::Foreign {
            return None;
        }
        self.barrier(value);
        let object: &mut dyn Any = self.foreign.get_mut(value.payload()).as_deref_mut()?;
        object.downcast_mut()
    }

    /// Each live foreign object that is a `T` and was allocated before epoch
    /// `before`, and that `moves` says so of, given to `f` with its value to
    /// change, unlogged: N8's resume after an edit that moved bytes (MS6 step
    /// 7) moves the old parse's places to the new text, past the compile's
    /// undo, which keeps them if it ends; if it stops the caller puts them back
    /// (D261).
    pub fn each_older_foreign_mut<T: Foreign>(
        &mut self,
        before: u32,
        moves: &dyn Fn(&T) -> bool,
        f: &mut dyn FnMut(Value, &mut T),
    ) {
        let arena = &mut self.foreign;
        for c in 0..arena.chunks.len() {
            for slot in 0..CHUNK {
                let index = (c * CHUNK + slot) as u32;
                let chunk = &arena.chunks[c];
                if chunk.slots.is_empty()
                    || chunk.live[slot / 64] & (1 << (slot % 64)) == 0
                    || arena.epoch(index) >= before
                {
                    continue;
                }
                let Some(object) = arena.chunks[c].slots[slot].as_deref_mut() else {
                    continue;
                };
                let object: &mut dyn Any = object;
                if let Some(t) = object.downcast_mut::<T>()
                    && moves(t)
                {
                    f(Value::tagged(Tag::Foreign, index), t);
                }
            }
        }
    }

    /// The name of the primitive `value` is, or None for anything else:
    /// what tells another crate's native code one of its primitives from a
    /// procedure it must leave to Scheme.
    pub fn primitive_name(&self, value: Value) -> Option<&'static str> {
        if value.tag() != Tag::Primitive {
            return None;
        }
        prims::get(self, value.payload()).map(|p| p.name)
    }

    /// `procedure?`.
    pub fn is_procedure(&self, value: Value) -> bool {
        crate::prims::procedure(self, value)
    }

    // --- numbers: the arenas under number.rs's tower

    /// A flonum, a double in the heap.
    pub fn flonum(&mut self, x: f64) -> Value {
        self.allocated += size_of::<f64>();
        Value::tagged(Tag::Flonum, self.flonums.alloc(self.epoch, x))
    }

    pub fn as_flonum(&self, value: Value) -> Option<f64> {
        (value.tag() == Tag::Flonum).then(|| *self.flonums.get(value.payload()))
    }

    /// A bignum: `n` must be outside the fixnums ([`Heap::integer_big`]
    /// chooses).
    pub(crate) fn bignum(&mut self, n: BigInt) -> Value {
        self.allocated += size_of::<BigInt>() + n.bit_length().div_ceil(8) as usize;
        Value::tagged(Tag::Bignum, self.bignums.alloc(self.epoch, n))
    }

    pub fn as_bignum(&self, value: Value) -> Option<&BigInt> {
        (value.tag() == Tag::Bignum).then(|| self.bignums.get(value.payload()))
    }

    /// The ratio of two exact integers already in lowest terms, the
    /// denominator above 1.
    pub(crate) fn ratio(&mut self, numerator: Value, denominator: Value) -> Value {
        self.allocated += size_of::<Ratio>();
        let ratio = Ratio {
            numerator,
            denominator,
        };
        Value::tagged(Tag::Ratio, self.ratios.alloc(self.epoch, ratio))
    }

    /// An exact ratio's numerator and denominator.
    pub fn as_ratio(&self, value: Value) -> Option<(Value, Value)> {
        (value.tag() == Tag::Ratio).then(|| {
            let ratio = self.ratios.get(value.payload());
            (ratio.numerator, ratio.denominator)
        })
    }

    /// A complex number of two doubles; Guile keeps one even when the
    /// imaginary part is 0.0.
    pub fn complex(&mut self, re: f64, im: f64) -> Value {
        self.allocated += size_of::<[f64; 2]>();
        Value::tagged(Tag::Complex, self.complexes.alloc(self.epoch, [re, im]))
    }

    pub fn as_complex(&self, value: Value) -> Option<(f64, f64)> {
        (value.tag() == Tag::Complex).then(|| {
            let [re, im] = *self.complexes.get(value.payload());
            (re, im)
        })
    }

    // --- procedures and multiple values

    pub(crate) fn closure(&mut self, code: Rc<Template>, free: Box<[Value]>) -> Value {
        self.allocated += size_of::<Closure>() + free.len() * size_of::<Value>();
        let closure = Closure {
            code: Some(code),
            free,
        };
        Value::tagged(Tag::Closure, self.closures.alloc(self.epoch, closure))
    }

    /// The name of a primitive, or of a closure whose code has one: what
    /// `procedure-name` gives without a property table.
    pub fn procedure_name(&self, value: Value) -> Option<&str> {
        match value.tag() {
            Tag::Primitive => self.primitive_name(value),
            Tag::Closure => self
                .closure_code(value)
                .map(|code| code.name)
                .filter(|name| name.tag() == Tag::Symbol)
                .and_then(|name| self.name(name)),
            _ => None,
        }
    }

    /// A digest of a closure's code, which every closure of its `lambda`
    /// and of the same `lambda` compiled again has, and the values of its
    /// free variables: what N8's digests take a closure as (music's
    /// `frozen::digest`, MS6 step 4), its printed number being new with
    /// each closure.
    pub fn closure_parts(&self, closure: Value) -> Option<(u64, &[Value])> {
        if closure.tag() != Tag::Closure {
            return None;
        }
        let c = self.closures.get(closure.payload());
        let code = c.code.as_ref()?;
        Some((self.template_digest(code), &c.free))
    }

    /// Whether `closure`'s code came with the image the heap was restored
    /// from (LilyPond's own Scheme), not compiled since: N8's partitions
    /// (MS6 step 6) trust an `\applyContext` procedure of that code to
    /// touch only its own context.
    pub fn is_image_code(&self, closure: Value) -> bool {
        if closure.tag() != Tag::Closure {
            return false;
        }
        let c = self.closures.get(closure.payload());
        c.code
            .as_ref()
            .is_some_and(|code| self.image_code.contains(&(Rc::as_ptr(code) as usize)))
    }

    /// The digest of `t`'s name, arguments, instructions (a global's slot,
    /// which the VM rewrites, left out), constants as `write` writes them,
    /// and its children's.
    fn template_digest(&self, t: &Template) -> u64 {
        if t.digest.get() != 0 {
            return t.digest.get();
        }
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |x: u64| h = (h ^ x).wrapping_mul(0x0100_0000_01b3);
        let mut text = String::new();
        let mix_str = |text: &str, mix: &mut dyn FnMut(u64)| {
            mix(text.len() as u64);
            text.bytes().for_each(|b| mix(b.into()));
        };
        mix_str(self.name(t.name).unwrap_or_default(), &mut mix);
        for x in [
            t.required.into(),
            t.rest.into(),
            t.frame.into(),
            t.free.into(),
            t.star.into(),
        ] {
            mix(x);
        }
        for op in t.ops.iter() {
            text.clear();
            let _ = write!(text, "{:?}", op.get());
            let op = match op.get() {
                Op::Global(_) | Op::SetGlobal(_) | Op::Define(_) | Op::Inline(..) => {
                    text.split('(').next().unwrap_or_default()
                }
                _ => &text,
            };
            mix_str(op, &mut mix);
        }
        for &c in &t.consts {
            text.clear();
            self.write(c, &mut text);
            mix_str(&text, &mut mix);
        }
        for child in &t.children {
            mix(self.template_digest(child));
        }
        let h = h.max(1);
        t.digest.set(h);
        h
    }

    pub(crate) fn closure_code(&self, closure: Value) -> Option<&Rc<Template>> {
        self.closures.get(closure.payload()).code.as_ref()
    }

    pub(crate) fn closure_free(&self, closure: Value) -> &[Value] {
        &self.closures.get(closure.payload()).free
    }

    /// What `values` returns for other than one value.
    pub(crate) fn values(&mut self, items: Vec<Value>) -> Value {
        self.allocated += size_of::<Vec<Value>>() + items.len() * size_of::<Value>();
        Value::tagged(Tag::Values, self.values.alloc(self.epoch, items))
    }

    /// The values of a [`Tag::Values`] object, which `values` returns for
    /// other than one.
    pub fn values_items(&self, values: Value) -> &[Value] {
        self.values.get(values.payload())
    }

    // --- epochs

    /// The epoch new objects are allocated in.
    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Starts the next epoch, as chapter 02 §5.7 does for each toplevel item,
    /// and returns it. Objects allocated before report an earlier epoch.
    pub fn begin_epoch(&mut self) -> u32 {
        self.epoch += 1;
        self.pairs.retire();
        self.strings.retire();
        self.vectors.retire();
        self.foreign.retire();
        self.flonums.retire();
        self.bignums.retire();
        self.ratios.retire();
        self.complexes.retire();
        self.closures.retire();
        self.values.retire();
        self.epoch
    }

    /// Whether `value` is a live foreign object: one the collector has not
    /// freed since it was had.
    pub fn is_live_foreign(&self, value: Value) -> bool {
        if value.tag() != Tag::Foreign {
            return false;
        }
        let (chunk, slot) = split(value.payload());
        self.foreign
            .chunks
            .get(chunk)
            .is_some_and(|c| c.live[slot / 64] & (1 << (slot % 64)) != 0)
    }

    /// The epoch of an object's chunk: not later than the epoch the object
    /// was allocated in. `None` for what is not in the heap.
    pub fn epoch_of(&self, value: Value) -> Option<u32> {
        let index = value.payload();
        match value.tag() {
            Tag::Pair => Some(self.pairs.epoch(index)),
            Tag::String => Some(self.strings.epoch(index)),
            Tag::Vector => Some(self.vectors.epoch(index)),
            Tag::Foreign => Some(self.foreign.epoch(index)),
            Tag::Flonum => Some(self.flonums.epoch(index)),
            Tag::Bignum => Some(self.bignums.epoch(index)),
            Tag::Ratio => Some(self.ratios.epoch(index)),
            Tag::Complex => Some(self.complexes.epoch(index)),
            Tag::Closure => Some(self.closures.epoch(index)),
            Tag::Values => Some(self.values.epoch(index)),
            _ => None,
        }
    }

    // --- the write barrier and the undo log (R11)

    /// Starts a toplevel item (02 §5.7): the next epoch, and the write
    /// barrier on for it. Returns the undo log's length, which
    /// [`Heap::undo`] goes back to.
    pub fn begin_item(&mut self) -> usize {
        let epoch = self.begin_epoch();
        self.undo.item = Some(epoch);
        self.undo.seen.clear();
        self.undo.log.len()
    }

    /// A mark inside the running item (N8's checkpoints, MS6 step 5,
    /// D241): the next epoch, so that the first change from here on of
    /// each object allocated before it is logged too; [`Heap::undo`] to
    /// the length it returns gives every object what it held here. `None`
    /// outside an item.
    pub fn checkpoint_mark(&mut self) -> Option<usize> {
        self.undo.item?;
        Some(self.begin_item())
    }

    /// N8's pictures (D241): the epochs from `since` to now become kept
    /// translations' own, and the undo log's entries from `from` on of
    /// objects of such epochs are taken out of it as the next generation of
    /// pictures, so that undoing the compile's item leaves those objects as
    /// they are.
    pub fn keep_pictures(&mut self, from: usize, since: u32) {
        self.undo.lineage.push((since, self.epoch));
        let log = core::mem::take(&mut self.undo.log);
        let mut taken = Vec::new();
        for (i, (value, old)) in log.into_iter().enumerate() {
            if i >= from && self.epoch_of(value).is_some_and(|e| self.in_lineage(e)) {
                taken.push((i, value, old));
            } else {
                self.undo.log.push((value, old));
            }
        }
        self.undo.seen.clear();
        let number = self.undo.next_generation;
        self.undo.pictures.push((number, taken));
        self.undo.next_generation += 1;
    }

    /// The number the generation of pictures of the running compile will
    /// take.
    pub fn generation(&self) -> u32 {
        self.undo.next_generation
    }

    /// Whether objects of `epoch` are kept translations' own.
    fn in_lineage(&self, epoch: u32) -> bool {
        self.undo
            .lineage
            .iter()
            .any(|&(a, b)| (a..=b).contains(&epoch))
    }

    /// Gives every object of an epoch `owns` names (a translation's own)
    /// kept in the generations of pictures after `generation`, and those
    /// of `generation` logged at or after `mark` in its compile's log, what
    /// they held then, the latest first: each object as it was at that
    /// mark (D241). What each held at `end` (the generation and mark where
    /// the translation that changed them ended: what layout changed after
    /// is not its), or now if nothing changed it after, is kept in the
    /// redo store: the range returned, which [`Heap::redo`] gives them
    /// again (N8's splice, D243).
    pub fn give_back(
        &mut self,
        generation: u32,
        mark: usize,
        owns: &dyn Fn(u32) -> bool,
        end: Option<(u32, usize)>,
    ) -> core::ops::Range<usize> {
        // Each object's first picture from the mark, the one it is given:
        // a later one's would be written over.
        let mut seen = BitsMap::new();
        let mut first = Vec::new();
        for (g, entries) in &self.undo.pictures {
            if *g < generation {
                continue;
            }
            for (i, value, old) in entries {
                if (*g > generation || *i >= mark)
                    && seen.get(value.to_bits()).is_none()
                    && self.epoch_of(*value).is_some_and(owns)
                    && let Some(old) = copy_old(old)
                {
                    seen.insert(value.to_bits(), None);
                    first.push((*value, old));
                }
            }
        }
        // What each of them was at `end`: its first picture from there.
        if let Some((eg, em)) = end {
            for (g, entries) in &self.undo.pictures {
                for (i, value, old) in entries {
                    if ((*g == eg && *i >= em) || *g > eg)
                        && matches!(seen.get(value.to_bits()), Some(None))
                    {
                        seen.insert(value.to_bits(), Some(old));
                    }
                }
            }
        }
        let mut ends = Vec::new();
        for (value, _) in &first {
            let then = match seen.get(value.to_bits()).flatten() {
                Some(o) => copy_old(o),
                None => self.picture(*value),
            };
            if let Some(then) = then {
                ends.push((*value, then));
            }
        }
        for (value, old) in first {
            if let Some(now) = self.picture(value) {
                self.undo.given.push((value, now));
            }
            self.put_back(value, old);
        }
        let start = self.undo.redo.len();
        self.undo.redo.extend(ends);
        start..self.undo.redo.len()
    }

    /// N8's splice (D243, OPEN C18): `f` given the heap with each object of
    /// `range` of the redo store ([`Heap::give_back`]'s, the objects a resume
    /// gave back) holding what it held at `mark` of generation `generation`'s
    /// compile, a later snapshot of the old run (its first picture from
    /// there, else what the redo store keeps of it, its end), and the objects
    /// that held something else, which hold it again once `f` returns. `f`
    /// reads the heap only: nothing is allocated or collected meanwhile.
    pub fn with_held_at<R>(
        &mut self,
        (generation, mark): (u32, usize),
        range: core::ops::Range<usize>,
        f: impl FnOnce(&Heap, &[Value]) -> R,
    ) -> R {
        let range = range.start.min(self.undo.redo.len())..range.end.min(self.undo.redo.len());
        let mut seen = BitsMap::new();
        for (value, _) in &self.undo.redo[range.clone()] {
            seen.insert(value.to_bits(), None);
        }
        for (g, entries) in &self.undo.pictures {
            for (i, value, old) in entries {
                if (*g > generation || (*g == generation && *i >= mark))
                    && matches!(seen.get(value.to_bits()), Some(None))
                {
                    seen.insert(value.to_bits(), Some(old));
                }
            }
        }
        let mut then = Vec::new();
        for (value, end) in &self.undo.redo[range] {
            let old = seen.get(value.to_bits()).flatten().unwrap_or(end);
            if !self.holds(*value, old)
                && let Some(old) = copy_old(old)
            {
                then.push((*value, old));
            }
        }
        let mut now = Vec::with_capacity(then.len());
        for (value, old) in then {
            if let Some(was) = self.replace(value, old) {
                now.push((value, was));
            }
        }
        let values: Vec<Value> = now.iter().map(|(v, _)| *v).collect();
        let out = f(self, &values);
        for (value, was) in now.into_iter().rev() {
            self.replace(value, was);
        }
        out
    }

    /// Gives `value` what `old` pictures, and what it held before: `None`
    /// (and nothing changed) for a foreign object's empty slot.
    fn replace(&mut self, value: Value, old: Old) -> Option<Old> {
        let index = value.payload();
        Some(match old {
            Old::Pair(car, cdr) => {
                let pair = self.pairs.get_mut(index);
                let was = Old::Pair(pair.car, pair.cdr);
                (pair.car, pair.cdr) = (car, cdr);
                was
            }
            Old::Str(text) => Old::Str(
                core::mem::replace(self.strings.get_mut(index), Str::new(text))
                    .as_str()
                    .into(),
            ),
            Old::Vector(items) => {
                Old::Vector(core::mem::replace(self.vectors.get_mut(index), items))
            }
            Old::Foreign(object) => {
                let slot = self.foreign.get_mut(index);
                slot.as_ref()?;
                Old::Foreign(slot.replace(object)?)
            }
        })
    }

    /// Gives each object given back in this compile what it held before it,
    /// the latest first, after the compile is undone: a compile a newer one
    /// stopped leaves the kept translations as they were (B24, D261).
    pub fn put_given_back(&mut self) {
        for (value, old) in core::mem::take(&mut self.undo.given).into_iter().rev() {
            self.put_back(value, old);
        }
    }

    /// Forgets what [`Heap::put_given_back`] would put back.
    pub fn forget_given(&mut self) {
        self.undo.given.clear();
    }

    /// Drops the generations of pictures before `generation`, and with
    /// none left the lineage (D241).
    pub fn drop_pictures(&mut self, generation: u32) {
        self.undo.pictures.retain(|(g, _)| *g >= generation);
        if self.undo.pictures.is_empty() {
            self.undo.lineage.clear();
        }
    }

    /// Whether an object the log has logged since it was `mark` long, older
    /// than epoch `since` and no kept translation's own, is not what it was
    /// then, but `except`: a change a translation made for good to what is
    /// older than it (D241).
    pub fn changed_older_since(&self, mark: usize, since: u32, except: Value) -> bool {
        self.undo.log[mark.min(self.undo.log.len())..]
            .iter()
            .any(|(value, old)| {
                *value != except
                    && self
                        .epoch_of(*value)
                        .is_some_and(|e| e < since && !self.in_lineage(e))
                    && !self.holds(*value, old)
            })
    }

    /// Keeps `value` alive until [`Heap::release_held`] (N8's kept
    /// translations, D241).
    pub fn hold_between(&mut self, value: Value) {
        self.undo.held.push(value);
    }

    pub fn release_held(&mut self) {
        self.undo.held.clear();
    }

    /// Turns the write barrier off: changes after it are not logged.
    pub fn end_item(&mut self) {
        self.undo.item = None;
        self.undo.seen.clear();
    }

    /// Whether an item is running, its changes logged.
    pub fn in_item(&self) -> bool {
        self.undo.item.is_some()
    }

    /// The undo log's length: the entries [`Heap::undo`] has to give back.
    pub fn undo_len(&self) -> usize {
        self.undo.log.len()
    }

    /// Gives each object logged since the log was `mark` long back what it
    /// was, the latest change first, and forgets those entries. What the
    /// items allocated is left to the collector.
    pub fn undo(&mut self, mark: usize) {
        if mark >= self.undo.log.len() {
            return;
        }
        let entries = self.undo.log.split_off(mark);
        for (value, old) in entries.into_iter().rev() {
            self.put_back(value, old);
        }
        self.undo.seen.clear();
    }

    /// The write barrier: `value` logged as it is now if it is older than
    /// the current item and this is its first change in it.
    #[inline]
    fn barrier(&mut self, value: Value) {
        if let Some(item) = self.undo.item
            && self.epoch_of(value).is_some_and(|epoch| epoch < item)
        {
            self.log_old(value);
        }
    }

    #[cold]
    fn log_old(&mut self, value: Value) {
        if !self.undo.seen.insert(value.to_bits()) {
            return;
        }
        if let Some(old) = self.picture(value) {
            self.undo.log.push((value, old));
        }
    }

    /// What `value` is now, as the undo log keeps it: None for a foreign
    /// object without [`Foreign::snapshot`] and for what is no object.
    fn picture(&self, value: Value) -> Option<Old> {
        let index = value.payload();
        Some(match value.tag() {
            Tag::Pair => {
                let pair = self.pairs.get(index);
                Old::Pair(pair.car, pair.cdr)
            }
            Tag::String => Old::Str(self.strings.get(index).as_str().into()),
            Tag::Vector => Old::Vector(self.vectors.get(index).clone()),
            Tag::Foreign => Old::Foreign(self.foreign.get(index).as_deref()?.snapshot()?),
            _ => return None,
        })
    }

    /// Gives `value` what `old` pictures.
    fn put_back(&mut self, value: Value, old: Old) {
        let index = value.payload();
        match old {
            Old::Pair(car, cdr) => {
                let pair = self.pairs.get_mut(index);
                (pair.car, pair.cdr) = (car, cdr);
            }
            Old::Str(text) => *self.strings.get_mut(index) = Str::new(text),
            Old::Vector(items) => *self.vectors.get_mut(index) = items,
            Old::Foreign(object) => *self.foreign.get_mut(index) = Some(object),
        }
    }

    /// [`Heap::undo`], keeping what each object it gives back is now, at
    /// the end of what is undone: the range of the redo store that
    /// [`Heap::redo`] takes. Undone the latest item first, each item kept
    /// apart, the items are done again the oldest first.
    pub fn undo_keeping(&mut self, mark: usize) -> core::ops::Range<usize> {
        let start = self.undo.redo.len();
        if mark < self.undo.log.len() {
            let entries = self.undo.log.split_off(mark);
            let mut kept = Vec::with_capacity(entries.len());
            for (value, old) in entries.into_iter().rev() {
                if let Some(now) = self.picture(value) {
                    kept.push((value, now));
                }
                self.put_back(value, old);
            }
            kept.reverse();
            self.undo.redo.extend(kept);
            self.undo.seen.clear();
        }
        start..self.undo.redo.len()
    }

    /// Gives the objects of `range` of the redo store what they were at the
    /// end of the item [`Heap::undo_keeping`] kept it of, the oldest first,
    /// each through the write barrier: logged as the current item's change.
    pub fn redo(&mut self, range: core::ops::Range<usize>) {
        for i in range {
            let Some(&(value, ref old)) = self.undo.redo.get(i) else {
                break;
            };
            let copy = match old {
                Old::Pair(car, cdr) => Old::Pair(*car, *cdr),
                Old::Str(text) => Old::Str(text.clone()),
                Old::Vector(items) => Old::Vector(items.clone()),
                Old::Foreign(object) => match object.snapshot() {
                    Some(object) => Old::Foreign(object),
                    None => continue,
                },
            };
            self.barrier(value);
            self.put_back(value, copy);
        }
    }

    /// Whether an object the undo log has logged since it was `mark` long
    /// is not what it was when it was logged: what the items since changed
    /// and did not give back (MS6 step 3, 02 §5.7's "effects on objects").
    pub fn changed_since(&self, mark: usize) -> bool {
        self.undo.log[mark.min(self.undo.log.len())..]
            .iter()
            .any(|(value, old)| !self.holds(*value, old))
    }

    /// Whether `value` holds what `old` pictures.
    fn holds(&self, value: Value, old: &Old) -> bool {
        let index = value.payload();
        match old {
            Old::Pair(car, cdr) => {
                let pair = self.pairs.get(index);
                (pair.car, pair.cdr) == (*car, *cdr)
            }
            Old::Str(text) => self.strings.get(index).as_str() == text.as_str(),
            Old::Vector(items) => self.vectors.get(index) == items,
            Old::Foreign(then) => self
                .foreign
                .get(index)
                .as_deref()
                .is_some_and(|now| now.same(then.as_ref())),
        }
    }

    /// The redo store's length.
    pub fn redo_len(&self) -> usize {
        self.undo.redo.len()
    }

    /// Keeps `value` alive with the redo store, as a parse's module is
    /// while nothing else reaches it between compiles.
    pub fn keep_for_redo(&mut self, value: Value) {
        self.undo.kept.push(value);
    }

    /// Drops the redo store and what it kept alive.
    pub fn forget_redo(&mut self) {
        self.undo.redo.clear();
        self.undo.kept.clear();
    }

    /// Stops logging until [`Heap::resume_undo`]: what changes meanwhile
    /// stays changed after an undo, as a part a stub loads (D74) stays
    /// loaded. Returns whether an item was running.
    pub fn suspend_undo(&mut self) -> bool {
        self.undo.item.take().is_some()
    }

    /// Logs again after [`Heap::suspend_undo`] if `running`, in an epoch
    /// of its own, so that what was allocated meanwhile is older than what
    /// the item changes after.
    pub fn resume_undo(&mut self, running: bool) {
        if running {
            self.undo.item = Some(self.begin_epoch());
        }
    }

    // --- roots

    /// Keeps `value` alive until the root stack is cut below it.
    pub fn root(&mut self, value: Value) -> Root {
        self.roots.push(value);
        Root(self.roots.len() as u32 - 1)
    }

    /// Roots each value `push` adds, in order, as [`Heap::root`] roots one.
    pub fn root_all(&mut self, push: impl FnOnce(&mut Vec<Value>)) {
        push(&mut self.roots);
    }

    pub fn rooted(&self, root: Root) -> Value {
        self.roots[root.0 as usize]
    }

    /// Replaces a root's value, as a loop that builds a list does.
    pub fn set_rooted(&mut self, root: Root, value: Value) {
        self.roots[root.0 as usize] = value;
    }

    /// The root stack's depth, to cut it back to with [`Heap::unroot_to`].
    pub fn root_depth(&self) -> usize {
        self.roots.len()
    }

    pub fn unroot_to(&mut self, depth: usize) {
        self.roots.truncate(depth);
    }

    // --- the collector

    /// Live objects in all arenas, the dead not yet swept among them.
    pub fn objects(&self) -> usize {
        self.pairs.count
            + self.strings.count
            + self.vectors.count
            + self.foreign.count
            + self.flonums.count
            + self.bignums.count
            + self.ratios.count
            + self.complexes.count
            + self.closures.count
            + self.values.count
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Whether the next safe point collects. What returns between its steps
    /// collects there ([`Heap::collection_wanted`]), unless memory is short or
    /// its step has run on past [`DEFERRED`] more.
    pub fn collection_due(&self) -> bool {
        self.collection_wanted()
            && (!self.deferring || self.trim || self.allocated - self.threshold >= DEFERRED)
    }

    /// Whether a procedure entry collects as [`set_collect_every`] asks.
    pub fn collection_forced(&mut self) -> bool {
        let every = COLLECT_EVERY.load(Ordering::Relaxed);
        if every == 0 || self.held > 0 {
            return false;
        }
        self.entries += 1;
        if self.entries < every {
            return false;
        }
        self.entries = 0;
        true
    }

    /// Whether enough was allocated for a collection.
    pub fn collection_wanted(&self) -> bool {
        self.held == 0 && self.allocated >= self.threshold
    }

    /// Whether what runs returns between its steps, and collects there.
    pub fn set_deferring(&mut self, deferring: bool) {
        self.deferring = deferring;
    }

    /// Whether a collection is under way between steps.
    pub fn collecting(&self) -> bool {
        self.collecting.is_some()
    }

    /// A compile is to stop for want of memory (D259), which what runs it
    /// finds in the next [`Heap::take_over_memory`].
    pub fn over_memory(&mut self) {
        self.over_memory = true;
    }

    pub fn take_over_memory(&mut self) -> bool {
        core::mem::take(&mut self.over_memory)
    }

    pub fn is_over_memory(&self) -> bool {
        self.over_memory
    }

    /// The next safe point collects, whatever has been allocated, and gives
    /// the allocator back the slots of the chunks that leaves empty (D259):
    /// memory is short.
    pub fn collect_soon(&mut self) {
        self.allocated = self.threshold;
        self.trim = true;
    }

    /// Keeps safe points from collecting until [`Heap::release`], while
    /// Rust holds values that are not rooted: the expander's, while a
    /// macro's transformer runs.
    pub(crate) fn hold(&mut self) {
        self.held += 1;
    }

    pub(crate) fn release(&mut self) {
        self.held -= 1;
    }

    /// Collects if enough was allocated since the last collection; true if
    /// it did. `roots` are the values the caller keeps outside the heap's
    /// root stack: the global slots, the VM's stacks.
    pub fn safe_point(&mut self, roots: &[&[Value]]) -> bool {
        let due = self.collection_due();
        if due {
            self.collect(roots);
        }
        due
    }

    /// The number of the collection running or next, which is never 0.
    fn collection_number(&self) -> u32 {
        self.stats.collections.wrapping_add(1).max(1)
    }

    /// Marks from the root stack and `roots`, then sweeps every arena.
    pub fn collect(&mut self, roots: &[&[Value]]) {
        self.collect_with(roots, |_, _| Vec::new());
    }

    /// [`Heap::collect`], where what is live besides depends on what the
    /// marking reached: `more` is given the modules and the global slots it
    /// reached since the last call and gives the values they keep, which
    /// are marked in turn, until it reaches nothing new (`Vm::collect`).
    pub(crate) fn collect_with(
        &mut self,
        roots: &[&[Value]],
        more: impl FnMut(&[u32], &[u32]) -> Vec<Value>,
    ) {
        self.begin_collecting(roots);
        self.go_on_collecting(f64::INFINITY, more);
    }

    /// A collection begun: its roots grayed, nothing marked yet.
    pub(crate) fn begin_collecting(&mut self, roots: &[&[Value]]) {
        let started = self.clock.map_or(0.0, |now| now());
        let before = self.objects();
        let collection = self.collection_number();
        let mut gray = core::mem::take(&mut self.gray);
        gray.begin();
        for &set in [&self.roots[..]].iter().chain(roots) {
            gray.extend(set.iter().copied().filter(|value| value.is_heap()));
        }
        // The undo log's objects and what it gives back to them, and the
        // redo store's.
        gray.extend(self.undo.kept.iter().copied().filter(|v| v.is_heap()));
        gray.extend(self.undo.held.iter().copied().filter(|v| v.is_heap()));
        let pictures = self
            .undo
            .pictures
            .iter()
            .flat_map(|(_, p)| p)
            .map(|(_, v, o)| (v, o));
        for (value, old) in self
            .undo
            .log
            .iter()
            .map(|(v, o)| (v, o))
            .chain(self.undo.redo.iter().map(|(v, o)| (v, o)))
            .chain(self.undo.given.iter().map(|(v, o)| (v, o)))
            .chain(pictures)
        {
            gray.push(*value);
            match old {
                Old::Pair(car, cdr) => {
                    gray.extend([*car, *cdr].into_iter().filter(|v| v.is_heap()))
                }
                Old::Str(_) => {}
                Old::Vector(items) => gray.extend(items.iter().copied().filter(|v| v.is_heap())),
                Old::Foreign(object) => object.trace(&mut Tracer {
                    collection,
                    gray: &mut gray,
                    modules: &mut self.traced_modules,
                }),
            }
        }
        self.gray = gray;
        let ms = self.clock.map_or(0.0, |now| now() - started);
        self.collecting = Some(Collecting {
            before,
            ms,
            sweeping: None,
        });
    }

    /// The collection under way goes on until `until`, by the clock: whether
    /// it is over. It looks at the clock between some thousands of objects,
    /// so it does something however little time it has.
    pub(crate) fn go_on_collecting(
        &mut self,
        until: f64,
        mut more: impl FnMut(&[u32], &[u32]) -> Vec<Value>,
    ) -> bool {
        let Some(mut c) = self.collecting.take() else {
            return true;
        };
        let started = self.clock.map_or(0.0, |now| now());
        let time = |heap: &Heap| heap.clock.map_or(0.0, |now| now());
        let mut gray = core::mem::take(&mut self.gray);
        let marked = c.sweeping.is_some()
            || loop {
                if !self.mark(&mut gray, until) {
                    break false;
                }
                if self.traced_modules.is_empty() && self.traced_slots.is_empty() {
                    self.prune_weak();
                    c.sweeping = Some((0, 0, 0));
                    break true;
                }
                let values = more(&self.traced_modules, &self.traced_slots);
                self.traced_modules.clear();
                self.traced_slots.clear();
                gray.extend(values.into_iter().filter(|value| value.is_heap()));
            };
        self.gray = gray;
        let swept = marked && self.sweep(&mut c, until);
        c.ms += time(self) - started;
        if swept {
            self.swept(&c);
        } else {
            self.collecting = Some(c);
        }
        swept
    }

    /// Whether the collection under way has begun to free things, so that it
    /// can only end.
    pub(crate) fn sweeping(&self) -> bool {
        self.collecting
            .as_ref()
            .is_some_and(|c| c.sweeping.is_some())
    }

    /// Drops the collection under way, which has freed nothing yet.
    pub(crate) fn drop_collecting(&mut self) {
        if self.collecting.take().is_none() {
            return;
        }
        self.gray.clear();
        self.traced_modules.clear();
        self.traced_slots.clear();
        self.pairs.clear_marks();
        self.strings.clear_marks();
        self.vectors.clear_marks();
        self.foreign.clear_marks();
        self.flonums.clear_marks();
        self.bignums.clear_marks();
        self.ratios.clear_marks();
        self.complexes.clear_marks();
        self.closures.clear_marks();
        self.values.clear_marks();
        // What many objects share is stamped with a collection's number, and
        // this one did not reach all of it.
        self.stats.collections += 1;
    }

    /// Marks what `gray` reaches, until `until` by the clock: whether it is
    /// empty.
    fn mark(&mut self, gray: &mut Gray, until: f64) -> bool {
        let mark = self.collection_number();
        let mut left = MARK_PART;
        while let Some(value) = gray.pop() {
            left -= 1;
            if left == 0 {
                left = MARK_PART;
                if until.is_finite() && self.clock.is_some_and(|now| now() >= until) {
                    gray.push_back(value);
                    return false;
                }
            }
            let index = value.payload();
            match value.tag() {
                Tag::Pair if self.pairs.mark(index) => {
                    let pair = self.pairs.get(index);
                    gray.extend(
                        [pair.car, pair.cdr]
                            .into_iter()
                            .filter(|value| value.is_heap()),
                    );
                }
                Tag::String => {
                    self.strings.mark(index);
                }
                Tag::Vector if self.vectors.mark(index) => {
                    gray.extend(
                        self.vectors
                            .get(index)
                            .iter()
                            .copied()
                            .filter(|value| value.is_heap()),
                    );
                }
                Tag::Foreign if self.foreign.mark(index) => {
                    if let Some(object) = self.foreign.get(index) {
                        object.trace(&mut Tracer {
                            collection: mark,
                            gray,
                            modules: &mut self.traced_modules,
                        });
                    }
                }
                Tag::Ratio if self.ratios.mark(index) => {
                    let ratio = self.ratios.get(index);
                    gray.extend(
                        [ratio.numerator, ratio.denominator]
                            .into_iter()
                            .filter(|value| value.is_heap()),
                    );
                }
                Tag::Flonum => {
                    self.flonums.mark(index);
                }
                Tag::Bignum => {
                    self.bignums.mark(index);
                }
                Tag::Complex => {
                    self.complexes.mark(index);
                }
                Tag::Closure if self.closures.mark(index) => {
                    let closure = self.closures.get(index);
                    gray.extend(closure.free.iter().copied().filter(|v| v.is_heap()));
                    if let Some(code) = &closure.code {
                        code.trace(gray, &mut self.traced_slots, mark);
                    }
                }
                Tag::Values if self.values.mark(index) => {
                    gray.extend(
                        self.values
                            .get(index)
                            .iter()
                            .copied()
                            .filter(|value| value.is_heap()),
                    );
                }
                _ => {}
            }
        }
        true
    }

    /// Sweeps the arenas after marking, some chunks at a time until `until`
    /// by the clock: whether they are done.
    fn sweep(&mut self, c: &mut Collecting, until: f64) -> bool {
        let epoch = self.epoch;
        loop {
            let Some((arena, chunk, bytes)) = c.sweeping else {
                return false;
            };
            let at = (chunk, SWEEP_PART);
            let (live, next) = match arena {
                0 => self.pairs.sweep_part(at, epoch, |_| size_of::<Pair>()),
                1 => self
                    .strings
                    .sweep_part(at, epoch, |s| size_of::<Str>() + s.as_str().len()),
                2 => self.vectors.sweep_part(at, epoch, |v| {
                    size_of::<Vec<Value>>() + v.len() * size_of::<Value>()
                }),
                3 => self.foreign.sweep_part(at, epoch, |o| {
                    o.as_deref().map_or(0, size_of_val) + size_of::<Box<dyn Foreign>>()
                }),
                4 => self.flonums.sweep_part(at, epoch, |_| size_of::<f64>()),
                5 => self.bignums.sweep_part(at, epoch, |n| {
                    size_of::<BigInt>() + n.bit_length().div_ceil(8) as usize
                }),
                6 => self.ratios.sweep_part(at, epoch, |_| size_of::<Ratio>()),
                7 => self
                    .complexes
                    .sweep_part(at, epoch, |_| size_of::<[f64; 2]>()),
                8 => self.closures.sweep_part(at, epoch, |c| {
                    size_of::<Closure>() + c.free.len() * size_of::<Value>()
                }),
                9 => self.values.sweep_part(at, epoch, |v| {
                    size_of::<Vec<Value>>() + v.len() * size_of::<Value>()
                }),
                _ => return true,
            };
            c.sweeping = Some(match next {
                Some(chunk) => (arena, chunk, bytes + live),
                None => (arena + 1, 0, bytes + live),
            });
            if until.is_finite() && self.clock.is_some_and(|now| now() >= until) {
                return false;
            }
        }
    }

    /// A collection is over: its counts, and when the next is due.
    fn swept(&mut self, c: &Collecting) {
        let (Collecting { before, ms, .. }, bytes) =
            (c, c.sweeping.map_or(0, |(_, _, bytes)| bytes));
        let (before, pause) = (*before, *ms);
        if core::mem::take(&mut self.trim) {
            self.pairs.trim();
            self.strings.trim();
            self.vectors.trim();
            self.foreign.trim();
            self.flonums.trim();
            self.bignums.trim();
            self.ratios.trim();
            self.complexes.trim();
            self.closures.trim();
            self.values.trim();
        }
        let live = self.objects();
        COLLECTING_US.fetch_add(
            (pause * 1000.0) as u64,
            core::sync::atomic::Ordering::Relaxed,
        );
        self.stats = Stats {
            collections: self.stats.collections + 1,
            live,
            live_bytes: bytes,
            freed: before - live,
            longest_pause_ms: self.stats.longest_pause_ms.max(pause),
            pause_ms: self.stats.pause_ms + pause,
        };
        self.allocated = 0;
        self.threshold = bytes.max(MIN_THRESHOLD);
    }

    /// After marking: forgets the weak holders that die, and prunes the
    /// others of what dies.
    fn prune_weak(&mut self) {
        let mut weak = core::mem::take(&mut self.weak);
        weak.retain(|&index| self.foreign.is_marked(index));
        for &index in &weak {
            let Some(mut object) = self.foreign.get_mut(index).take() else {
                continue;
            };
            object.prune(&|value| self.is_marked(value));
            *self.foreign.get_mut(index) = Some(object);
        }
        // A weak holder's copy in the undo log and the redo store holds
        // weakly too.
        let mut log = core::mem::take(&mut self.undo.log);
        let mut redo = core::mem::take(&mut self.undo.redo);
        let mut given = core::mem::take(&mut self.undo.given);
        let mut pictures = core::mem::take(&mut self.undo.pictures);
        let kept = pictures
            .iter_mut()
            .flat_map(|(_, p)| p)
            .map(|(_, v, o)| (&*v, o));
        for (value, old) in log
            .iter_mut()
            .map(|(v, o)| (&*v, o))
            .chain(redo.iter_mut().map(|(v, o)| (&*v, o)))
            .chain(given.iter_mut().map(|(v, o)| (&*v, o)))
            .chain(kept)
        {
            if let Old::Foreign(object) = old
                && weak.contains(&value.payload())
            {
                object.prune(&|value| self.is_marked(value));
            }
        }
        self.undo.log = log;
        self.undo.redo = redo;
        self.undo.given = given;
        self.undo.pictures = pictures;
        self.weak = weak;
    }

    /// Whether marking reached `value`; values outside the heap always live.
    fn is_marked(&self, value: Value) -> bool {
        let index = value.payload();
        match value.tag() {
            Tag::Pair => self.pairs.is_marked(index),
            Tag::String => self.strings.is_marked(index),
            Tag::Vector => self.vectors.is_marked(index),
            Tag::Foreign => self.foreign.is_marked(index),
            Tag::Flonum => self.flonums.is_marked(index),
            Tag::Bignum => self.bignums.is_marked(index),
            Tag::Ratio => self.ratios.is_marked(index),
            Tag::Complex => self.complexes.is_marked(index),
            Tag::Closure => self.closures.is_marked(index),
            Tag::Values => self.values.is_marked(index),
            _ => true,
        }
    }

    // --- the image

    /// Writes the symbols, the arenas and the weak holders (`image.rs`),
    /// numbering the closures' code in `templates`.
    pub(crate) fn save(
        &self,
        w: &mut ImageWriter,
        templates: &mut Templates,
    ) -> Result<(), ImageError> {
        let symbols = &self.symbols;
        w.count(symbols.names.len());
        for (name, &uninterned) in symbols.names.iter().zip(&symbols.uninterned) {
            w.str(name);
            w.bool(uninterned);
        }
        w.u32(self.epoch);
        w.u64(self.threshold as u64);
        self.pairs.save(w, |p, w| {
            w.value(p.car);
            w.value(p.cdr);
            Ok(())
        })?;
        self.strings.save(w, |s, w| {
            w.str(s.as_str());
            Ok(())
        })?;
        self.vectors.save(w, |v, w| {
            w.values(v);
            Ok(())
        })?;
        self.flonums.save(w, |&x, w| {
            w.f64(x);
            Ok(())
        })?;
        self.bignums.save(w, |n, w| {
            n.save(w);
            Ok(())
        })?;
        self.ratios.save(w, |q, w| {
            w.value(q.numerator);
            w.value(q.denominator);
            Ok(())
        })?;
        self.complexes.save(w, |&[re, im], w| {
            w.f64(re);
            w.f64(im);
            Ok(())
        })?;
        self.closures.save(w, |c, w| {
            w.u32(
                c.code
                    .as_ref()
                    .map_or(u32::MAX, |code| templates.number(code)),
            );
            w.values(&c.free);
            Ok(())
        })?;
        self.values.save(w, |v, w| {
            w.values(v);
            Ok(())
        })?;
        // The foreign objects after the names of their types.
        let mut names: Vec<&'static str> = Vec::new();
        let mut objects = ImageWriter::default();
        self.foreign.save(&mut objects, |object, w| {
            let Some(object) = object else {
                return Err(ImageError::NotAtRest);
            };
            let name = object.type_name();
            let n = match names.iter().position(|&n| n == name) {
                Some(n) => n,
                None => {
                    names.push(name);
                    names.len() - 1
                }
            };
            w.u32(n as u32);
            if object.save(w) {
                Ok(())
            } else {
                Err(ImageError::Unsaved(name))
            }
        })?;
        w.count(names.len());
        for name in names {
            w.str(name);
        }
        w.append(objects);
        w.count(self.weak.len());
        for &index in &self.weak {
            w.u32(index);
        }
        Ok(())
    }

    /// The heap [`Heap::save`] wrote, with `templates` its closures' code and
    /// `loaders` reading its foreign objects. Its primitive tables are the
    /// caller's to give.
    pub(crate) fn load(
        r: &mut ImageReader<'_>,
        templates: &[Rc<Template>],
        loaders: &[&Loader],
        mark: &mut dyn FnMut(&'static str),
    ) -> Result<Heap, ImageError> {
        let corrupt = || ImageError::Corrupt;
        let mut heap = Heap::new();
        heap.image_code = templates.iter().map(|t| Rc::as_ptr(t) as usize).collect();
        let n = r.count().ok_or_else(corrupt)?;
        // The names' bytes end to end, checked as one text: each piece is
        // text if the whole is and each ends at a character's boundary.
        let mut text = Vec::new();
        let mut ends = alloc::vec![0; n];
        let mut uninterned = alloc::vec![false; n];
        let mut i = 0;
        while i < n {
            text.extend_from_slice(r.bytes().ok_or_else(corrupt)?);
            ends[i] = text.len() as u32;
            uninterned[i] = r.bool().ok_or_else(corrupt)?;
            i += 1;
        }
        heap.symbols.uninterned = uninterned;
        let text = String::from_utf8(text).map_err(|_| corrupt())?;
        if !ends.iter().all(|&end| text.is_char_boundary(end as usize)) {
            return Err(corrupt());
        }
        heap.symbols.names = Names { text, ends };
        let mut size = 1024;
        while (n + 1) * 2 > size {
            size *= 2;
        }
        heap.symbols.rebuild(size);
        mark("symbols");
        let epoch = r.u32().ok_or_else(corrupt)?;
        heap.epoch = epoch;
        heap.threshold = r.u64().ok_or_else(corrupt)? as usize;
        heap.pairs = Arena::load(r, epoch, |r| {
            Some(Pair {
                car: r.value()?,
                cdr: r.value()?,
            })
        })
        .ok_or_else(corrupt)?;
        heap.strings =
            Arena::load(r, epoch, |r| Some(Str::new(r.str()?.into()))).ok_or_else(corrupt)?;
        heap.vectors = Arena::load(r, epoch, |r| r.values()).ok_or_else(corrupt)?;
        heap.flonums = Arena::load(r, epoch, |r| r.f64()).ok_or_else(corrupt)?;
        heap.bignums = Arena::load(r, epoch, BigInt::load).ok_or_else(corrupt)?;
        heap.ratios = Arena::load(r, epoch, |r| {
            Some(Ratio {
                numerator: r.value()?,
                denominator: r.value()?,
            })
        })
        .ok_or_else(corrupt)?;
        heap.complexes =
            Arena::load(r, epoch, |r| Some([r.f64()?, r.f64()?])).ok_or_else(corrupt)?;
        heap.closures = Arena::load(r, epoch, |r| {
            let code = match r.u32()? {
                u32::MAX => None,
                n => Some(templates.get(n as usize)?.clone()),
            };
            let free = r.values()?.into_boxed_slice();
            Some(Closure { code, free })
        })
        .ok_or_else(corrupt)?;
        heap.values = Arena::load(r, epoch, |r| r.values()).ok_or_else(corrupt)?;
        mark("arenas");
        let mut types = Vec::new();
        for _ in 0..r.count().ok_or_else(corrupt)? {
            let name = r.str().ok_or_else(corrupt)?;
            match loaders.iter().find(|l| l.name == name) {
                Some(loader) => types.push(loader.load),
                None => return Err(ImageError::NoLoader(name.into())),
            }
        }
        heap.foreign = Arena::load(r, epoch, |r| {
            let load = types.get(r.u32()? as usize)?;
            load(r).map(Some)
        })
        .ok_or_else(corrupt)?;
        for _ in 0..r.count().ok_or_else(corrupt)? {
            heap.weak.push(r.u32().ok_or_else(corrupt)?);
        }
        mark("foreign");
        Ok(heap)
    }

    // --- equality

    /// `eqv?`: `eq?`, and numbers of the same kind with the same value. Two
    /// flonums are `eqv?` when `=` and of the same sign, or both NaN.
    pub fn eqv(&self, a: Value, b: Value) -> bool {
        if a == b {
            return true;
        }
        let same = |x: f64, y: f64| {
            (x == y && x.is_sign_negative() == y.is_sign_negative()) || (x.is_nan() && y.is_nan())
        };
        match (a.tag(), b.tag()) {
            (Tag::Flonum, Tag::Flonum) => same(
                *self.flonums.get(a.payload()),
                *self.flonums.get(b.payload()),
            ),
            (Tag::Bignum, Tag::Bignum) => {
                self.bignums.get(a.payload()) == self.bignums.get(b.payload())
            }
            (Tag::Ratio, Tag::Ratio) => {
                let (x, y) = (self.ratios.get(a.payload()), self.ratios.get(b.payload()));
                self.eqv(x.numerator, y.numerator) && self.eqv(x.denominator, y.denominator)
            }
            (Tag::Complex, Tag::Complex) => {
                let ([a, b], [c, d]) = (
                    *self.complexes.get(a.payload()),
                    *self.complexes.get(b.payload()),
                );
                same(a, c) && same(b, d)
            }
            _ => false,
        }
    }

    /// `equal?`: pairs, vectors and strings by contents, foreign objects by
    /// [`Foreign::equal`], the rest by `eqv?`. Iterative, so a long list or a
    /// deep tree does not use the native stack.
    pub fn equal(&self, a: Value, b: Value) -> bool {
        let mut work = Vec::new();
        let (mut a, mut b) = (a, b);
        loop {
            if !self.eqv(a, b) {
                if a.tag() != b.tag() {
                    return false;
                }
                match a.tag() {
                    Tag::Pair => {
                        let (x, y) = (self.pairs.get(a.payload()), self.pairs.get(b.payload()));
                        work.push((x.cdr, y.cdr));
                        work.push((x.car, y.car));
                    }
                    Tag::String
                        if self.strings.get(a.payload()).as_str()
                            == self.strings.get(b.payload()).as_str() => {}
                    Tag::Vector => {
                        let (x, y) = (self.vectors.get(a.payload()), self.vectors.get(b.payload()));
                        if x.len() != y.len() {
                            return false;
                        }
                        work.extend(x.iter().copied().zip(y.iter().copied()));
                    }
                    Tag::Foreign => match (self.foreign_object(a), self.foreign_object(b)) {
                        (Some(x), Some(y)) if x.equal(y, self) => {}
                        _ => return false,
                    },
                    _ => return false,
                }
            }
            match work.pop() {
                Some(next) => (a, b) = next,
                None => return true,
            }
        }
    }

    // --- printing

    /// `(write value)` as Guile 3 prints it.
    pub fn write(&self, value: Value, out: &mut String) {
        self.print(value, out, true);
    }

    /// `(display value)` as Guile 3 prints it.
    pub fn display(&self, value: Value, out: &mut String) {
        self.print(value, out, false);
    }

    fn print(&self, value: Value, out: &mut String, write: bool) {
        match value.tag() {
            Tag::Fixnum => {
                let _ = write!(out, "{}", value.as_fixnum().unwrap_or_default());
            }
            Tag::Null => out.push_str("()"),
            Tag::False => out.push_str("#f"),
            Tag::True => out.push_str("#t"),
            Tag::Unspecified => out.push_str("#<unspecified>"),
            Tag::Undefined => out.push_str("#<undefined>"),
            Tag::Eof => out.push_str("#<eof>"),
            Tag::Error => out.push_str("#<error>"),
            Tag::Char => match value.as_char() {
                Some(c) if !write => out.push(c),
                Some(c) => write_char(c, out),
                None => {}
            },
            Tag::Symbol => write_symbol(self.symbols.names.get(value.payload() as usize), out),
            Tag::Keyword => {
                out.push_str("#:");
                write_symbol(self.symbols.names.get(value.payload() as usize), out);
            }
            Tag::String => {
                let text = self.strings.get(value.payload()).as_str();
                if write {
                    write_string(text, out)
                } else {
                    out.push_str(text)
                }
            }
            Tag::Pair => {
                out.push('(');
                let mut pair = self.pairs.get(value.payload());
                loop {
                    self.print(pair.car, out, write);
                    match pair.cdr.tag() {
                        Tag::Null => break,
                        Tag::Pair => {
                            out.push(' ');
                            pair = self.pairs.get(pair.cdr.payload());
                        }
                        _ => {
                            out.push_str(" . ");
                            self.print(pair.cdr, out, write);
                            break;
                        }
                    }
                }
                out.push(')');
            }
            Tag::Vector => {
                out.push_str("#(");
                for (i, &item) in self.vectors.get(value.payload()).iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    self.print(item, out, write);
                }
                out.push(')');
            }
            Tag::Foreign => match self.foreign_object(value) {
                Some(object) => object.print(self, out),
                None => out.push_str("#<foreign>"),
            },
            Tag::Flonum | Tag::Bignum | Tag::Ratio | Tag::Complex => {
                number::write_number(self, value, 10, out)
            }
            Tag::Primitive => prims::write_primitive(prims::get(self, value.payload()), out),
            Tag::Closure => {
                let closure = self.closures.get(value.payload());
                out.push_str("#<procedure ");
                let Some(code) = &closure.code else {
                    return;
                };
                match self.name(code.name) {
                    Some(name) => write_symbol(name, out),
                    None => {
                        let _ = write!(out, "{:x}", value.payload());
                    }
                }
                // LilyPond's compiled Scheme prints its formals as written:
                // `(len context)`, `(name . props)`, a rest alone as it.
                if code.names != Value::FALSE && !code.star {
                    out.push(' ');
                    self.print(code.names, out, false);
                    out.push('>');
                    return;
                }
                // Guile's evaluator names the arguments of the procedures
                // it makes a, b, c and rest.
                let names = (b'a'..=b'z').map(|c| c as char);
                let required = names.take(code.required as usize);
                match (code.required, code.rest) {
                    // And a `lambda*` or `case-lambda` procedure as args.
                    _ if code.star => out.push_str(" args"),
                    (0, true) => out.push_str(" rest"),
                    _ => {
                        out.push_str(" (");
                        for (i, c) in required.enumerate() {
                            if i > 0 {
                                out.push(' ');
                            }
                            out.push(c);
                        }
                        if code.rest {
                            out.push_str(" . rest");
                        }
                        out.push(')');
                    }
                }
                out.push('>');
            }
            Tag::Values => {
                out.push_str("#<values (");
                for (i, &item) in self.values.get(value.payload()).iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    self.print(item, out, write);
                }
                out.push_str(")>");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(i: i64) -> Value {
        Value::fixnum(i).unwrap()
    }

    /// OPEN C18: a kept translation's object changed after the snapshot a
    /// resume gives it back to, and again after a later one, `then`, is read
    /// as it was then, and given back what it held when that ends.
    #[test]
    fn an_object_given_back_is_seen_as_a_later_snapshot_held_it() {
        let mut heap = Heap::new();
        let k = heap.intern("k");
        // A compile: its translation makes `p`, then takes two snapshots,
        // changing `p` after each; `q`, made after the first, is changed
        // after the second only.
        let from = heap.begin_item();
        let since = heap.epoch();
        let p = heap.cons(k, n(1));
        let mark = heap.checkpoint_mark().unwrap();
        let q = heap.cons(k, n(10));
        heap.set_cdr(p, n(2));
        heap.set_cdr(q, n(11));
        let then = heap.checkpoint_mark().unwrap();
        heap.set_cdr(p, n(3));
        heap.set_cdr(q, n(12));
        let end = (heap.generation(), heap.undo_len());
        let last = heap.epoch();
        heap.keep_pictures(from, since);
        heap.undo(from);
        heap.end_item();
        // The next compile resumes at the first snapshot.
        heap.begin_item();
        let owns = |e: u32| (since..=last).contains(&e);
        let redo = heap.give_back(0, mark, &owns, Some(end));
        assert_eq!((heap.cdr(p), heap.cdr(q)), (Some(n(1)), Some(n(11))));
        let seen = heap.with_held_at((0, then), redo.clone(), |heap, changed| {
            (heap.cdr(p), heap.cdr(q), changed.to_vec())
        });
        assert_eq!(seen, (Some(n(2)), Some(n(11)), alloc::vec![p]));
        assert_eq!((heap.cdr(p), heap.cdr(q)), (Some(n(1)), Some(n(11))));
        // At the snapshot resumed from, as they are; at the end, as the redo
        // store keeps them.
        let none = heap.with_held_at((0, mark), redo.clone(), |_, changed| changed.len());
        assert_eq!(none, 0);
        let ended = heap.with_held_at(end, redo, |heap, _| (heap.cdr(p), heap.cdr(q)));
        assert_eq!(ended, (Some(n(3)), Some(n(12))));
    }
}
