//! Grob and audio records (04 §9.1, N5, MS2 step 25): what translation
//! hands to layout (chapter 05) and to the playback map (chapter 07),
//! where LilyPond makes its `Grob` and `Audio_element` objects.
//!
//! A record is a number in its translation's [`Records`]: the grob's or
//! the audio item's name, the context it was made in, the translator that
//! made it, its cause (an event or another record), and the moment. A grob
//! has the interfaces it is acknowledged by, as a 256-bit set of numbers a
//! translation gives each interface the first time one is named
//! ([`Interfaces`]). Their layout is chapter 05's; part 3 makes no grob
//! but those a Scheme engraver makes, so these carry what the framework
//! announces and acknowledges, and the properties Scheme sets.
//!
//! The rest of a grob is layout's (D108): it keeps it in its translation's
//! [`GrobSlot`], by record number, which this crate holds without knowing
//! its type.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::any::Any;

use lily_engine_scheme::{Heap, Rows, Tracer, Value};

use crate::context::CtxId;
use crate::event_class::ClassSet;
use crate::rational::Moment;

/// A set of grob interfaces: bit `i` is interface `i` of its translation.
pub type InterfaceSet = ClassSet;

/// A translation's grob interfaces, numbered as they are first named.
#[derive(Clone, Default)]
pub struct Interfaces {
    symbols: Vec<Value>,
    by_symbol: BTreeMap<u64, u8>,
}

impl Interfaces {
    /// The number of the interface `sym`, made if it has none; `None` past
    /// 256 interfaces.
    pub fn number(&mut self, sym: Value) -> Option<u8> {
        if let Some(&n) = self.by_symbol.get(&sym.to_bits()) {
            return Some(n);
        }
        let n = u8::try_from(self.symbols.len()).ok()?;
        self.symbols.push(sym);
        self.by_symbol.insert(sym.to_bits(), n);
        Some(n)
    }

    /// The set of the interfaces of `list`, a list of symbols.
    pub fn set(&mut self, heap: &Heap, list: Value) -> InterfaceSet {
        let mut set = InterfaceSet::default();
        let mut p = list;
        while let (Some(sym), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            if let Some(n) = self.number(sym) {
                set.insert(n);
            }
            p = rest;
        }
        set
    }

    /// The symbol of interface `n`.
    pub fn symbol(&self, n: u8) -> Value {
        self.symbols[usize::from(n)]
    }
}

/// What a record was made for (`Grob_info`'s and `Audio_element_info`'s
/// cause).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    None,
    /// A stream event.
    Event(Value),
    Grob(u32),
    Audio(u32),
}

/// Who made a record: a context and the number of its translator there.
pub type Origin = (CtxId, u16);

/// A grob made by an engraver.
#[derive(Clone)]
pub struct GrobRecord {
    /// Its type, a symbol of `all-grob-descriptions`.
    pub name: Value,
    pub origin: Origin,
    pub cause: Cause,
    /// When it was made, and when its end was announced.
    pub moment: Moment,
    pub ended: Option<Moment>,
    pub interfaces: InterfaceSet,
    /// Its properties: those set on it, an alist, and its definition's
    /// (`'()` for one made by a class of chapter 05's).
    pub props: Value,
    pub definition: Value,
    /// Its Scheme handle, made when it is first asked for, or `#f`
    /// (`scheme_translator.rs`).
    pub handle: Value,
}

/// An audio item made by a performer.
#[derive(Clone)]
pub struct AudioRecord {
    pub name: Value,
    pub origin: Origin,
    pub cause: Cause,
    pub moment: Moment,
    /// What the performer keeps with it: a pitch, a tempo, a text.
    pub value: Value,
    /// The element it is (`audio.rs`), and, for an item, its column in
    /// the performance once `Score_performer` gave it one, and its channel.
    pub audio: crate::audio::Audio,
    pub column: Option<u32>,
    pub channel: u8,
}

/// D108's slot: what layout keeps of a translation's grobs, each record's
/// state by its number (property lookup, the object alist, parents and
/// offsets), a Scheme engraver's records among them. The translation
/// holds one, empty until layout fills it (`Arena::grob_slot`),
/// and traces it with its own values; nothing else of it is this crate's.
pub trait GrobSlot: Any {
    /// Traces the Scheme values it holds.
    fn trace(&self, tracer: &mut Tracer<'_>);
    /// Itself copied, for N8's snapshots (`resume.rs`, D241): the
    /// translation fills it as it makes grobs (their parents).
    fn clone_slot(&self) -> Box<dyn GrobSlot>;
    /// N8's resume (D241): the grobs, and the Paper_score, typeset with the
    /// output definition `old` given `new`, the next compile's.
    fn take_layout(&mut self, heap: &mut lily_engine_scheme::Heap, old: Value, new: Value) {
        let _ = (heap, old, new);
    }
    /// N8's splice (D243): whether its grob `i` holds what `before`'s did,
    /// `before` a copy of it taken earlier in its run, the same objects.
    fn same_grob(&self, before: &dyn GrobSlot, i: usize) -> bool {
        let _ = (before, i);
        false
    }
    /// N8's splice (D243): a digest of its grob `i`'s contents, each value
    /// by `value`, so that another run's grob in the same place with the
    /// same contents has it.
    fn grob_digest(&self, i: usize, value: &mut dyn FnMut(Value) -> u64) -> u64 {
        let _ = (i, value);
        0
    }
    /// N8's splice (D243): its grob `i` another run's, from `other`.
    fn put_grob(&mut self, other: &dyn GrobSlot, i: usize) {
        let _ = (other, i);
    }
}

/// A translation's records.
#[derive(Clone, Default)]
pub struct Records {
    /// In rows, which N8's snapshots share (D259).
    pub grobs: Rows<GrobRecord>,
    pub audio: Rows<AudioRecord>,
    pub interfaces: Interfaces,
}

impl Records {
    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        let cause = |tracer: &mut Tracer<'_>, c: Cause| {
            if let Cause::Event(ev) = c {
                tracer.trace(ev);
            }
        };
        self.grobs.trace(tracer, |g, tracer| {
            tracer.trace(g.name);
            tracer.trace(g.props);
            tracer.trace(g.definition);
            tracer.trace(g.handle);
            cause(tracer, g.cause);
        });
        self.audio.trace(tracer, |a, tracer| {
            tracer.trace(a.name);
            tracer.trace(a.value);
            cause(tracer, a.cause);
        });
        for &s in &self.interfaces.symbols {
            tracer.trace(s);
        }
    }
}
