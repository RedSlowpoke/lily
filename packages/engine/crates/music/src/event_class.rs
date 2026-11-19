//! Event classes as sets of bits (04 §8, N3): a class is a `u8`, and what
//! it is (itself and its ancestors) a 256-bit [`ClassSet`], so a listener
//! of `rhythmic-event` hears a `note-event` by one test of bits.
//!
//! [`EVENT_CLASSES`] numbers `define-event-classes.scm`'s classes at build
//! time (`build.rs`), so that a Rust translator can name what it listens to
//! by number (step 25). A stream event names its class as LilyPond's do,
//! by the list `ly:make-event-class` gives, the class first and its
//! ancestors after it; [`EventClasses::class`] reads it, and numbers a class
//! that `define-event-class` added after the table as it is met.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use lily_engine_scheme::{Heap, Tag, Value};

include!(concat!(env!("OUT_DIR"), "/event_classes.rs"));

/// A set of event classes: bit `i` is class `i`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClassSet(pub [u64; 4]);

impl ClassSet {
    pub fn contains(&self, class: u8) -> bool {
        self.0[usize::from(class >> 6)] & (1 << (class & 63)) != 0
    }

    pub fn insert(&mut self, class: u8) {
        self.0[usize::from(class >> 6)] |= 1 << (class & 63);
    }

    pub fn intersects(&self, other: &ClassSet) -> bool {
        self.0.iter().zip(other.0).any(|(a, b)| a & b != 0)
    }

    /// The classes in it, in order.
    pub fn iter(&self) -> impl Iterator<Item = u8> + '_ {
        (0..=255u8).filter(|&c| self.contains(c))
    }
}

/// The number of `name` in [`EVENT_CLASSES`].
pub fn class_index(name: &str) -> Option<u8> {
    EVENT_CLASSES
        .iter()
        .position(|&(n, _)| n == name)
        .map(|i| i as u8)
}

/// The classes of a translation: each one's symbol and what it is, and the
/// number of each symbol.
#[derive(Clone)]
pub struct EventClasses {
    symbols: Vec<Value>,
    sets: Vec<ClassSet>,
    by_symbol: BTreeMap<u64, u8>,
}

impl EventClasses {
    /// [`EVENT_CLASSES`]' classes, each with its ancestors.
    pub fn new(heap: &mut Heap) -> EventClasses {
        let mut classes = EventClasses {
            symbols: Vec::new(),
            sets: Vec::new(),
            by_symbol: BTreeMap::new(),
        };
        for &(name, parent) in EVENT_CLASSES.iter() {
            let symbol = heap.intern(name);
            classes.add(symbol, parent);
        }
        classes
    }

    fn add(&mut self, symbol: Value, parent: Option<u8>) -> Option<u8> {
        let class = u8::try_from(self.symbols.len()).ok()?;
        let mut set = parent.map_or(ClassSet::default(), |p| self.sets[usize::from(p)]);
        set.insert(class);
        self.symbols.push(symbol);
        self.sets.push(set);
        self.by_symbol.insert(symbol.to_bits(), class);
        Some(class)
    }

    /// The number of the class `symbol` names, if it has one.
    pub fn number(&self, symbol: Value) -> Option<u8> {
        self.by_symbol.get(&symbol.to_bits()).copied()
    }

    /// The number of the class of `list`, `ly:make-event-class`'s list of a
    /// class and its ancestors, numbering those not yet numbered; `None`
    /// for no list of symbols, or past 256 classes.
    pub fn class(&mut self, heap: &Heap, list: Value) -> Option<u8> {
        let leaf = heap.car(list)?;
        if let Some(class) = self.number(leaf) {
            return Some(class);
        }
        let mut items = Vec::new();
        let mut p = list;
        while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            if item.tag() != Tag::Symbol {
                return None;
            }
            items.push(item);
            p = rest;
        }
        let mut parent = None;
        for &symbol in items.iter().rev() {
            parent = match self.number(symbol) {
                Some(class) => Some(class),
                None => Some(self.add(symbol, parent)?),
            };
        }
        parent
    }

    /// What `class` is: it and its ancestors.
    pub fn set(&self, class: u8) -> ClassSet {
        self.sets[usize::from(class)]
    }

    /// The symbol of `class`.
    pub fn symbol(&self, class: u8) -> Value {
        self.symbols[usize::from(class)]
    }
}
