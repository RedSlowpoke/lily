//! Lengths and starts of music (N2, 04 §6.1): `ly:music-length`,
//! `ly:music-start` and the C++ callbacks of `music.cc`,
//! `music-sequence.cc`, `music-wrapper.cc`, `grace-music.cc` and
//! `calculated-sequential-music.cc` that the music types name.
//!
//! They are computed natively on live and frozen music alike, through
//! [`At`], so nothing is thawed. A frozen node keeps what was computed of it
//! in its tree's side array ([`Tree::time`]) when the computation read no
//! live music, which Scheme could still change.
//!
//! A length or start whose computation reaches a procedure of Scheme (a
//! type's `elements-callback`, or a callback of the user's) is left to
//! Scheme: the primitive calls its fallback in [`PRELUDE`] in its place
//! ([`Cx::tail_call`]), which does what the C++ does with `ly:music-length`
//! and `ly:music-start` of each element, so each element is native again
//! where it can be.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use lily_engine_scheme::{Cx, Exception, Heap, Thrown, Value};

use crate::context::Host;

use crate::frozen::{Time, Tree};
use crate::music::{At, Item};
use crate::{Duration, Moment, R, Rational};

/// The callbacks that are native, by their names and the rules that
/// compute them, in [`crate::PRIMS`]' order.
pub static CALLBACKS: [&str; 11] = [
    "ly:music::duration-length-callback",
    "ly:music-sequence::cumulative-length-callback",
    "ly:music-sequence::maximum-length-callback",
    "ly:music-sequence::event-chord-length-callback",
    "ly:music-sequence::minimum-start-callback",
    "ly:music-sequence::first-start-callback",
    "ly:music-wrapper::length-callback",
    "ly:music-wrapper::start-callback",
    "ly:grace-music::start-callback",
    "ly:calculated-sequential-music::length",
    "ly:calculated-sequential-music::start",
];

/// The names their errors give, LilyPond's C++ functions': the grace
/// music's start checks its argument as the wrapper's length does.
const SUBRS: [&str; 11] = [
    "ly:duration-length-callback",
    "ly:cumulative-length-callback",
    "ly:maximum-length-callback",
    "ly:event-chord-length-callback",
    "ly:minimum-start-callback",
    "ly:first-start-callback",
    "ly:length-callback",
    "ly:start-callback",
    "ly:length-callback",
    "ly:length",
    "ly:start",
];

/// A callback's rule: its index in [`CALLBACKS`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rule {
    DurationLength,
    CumulativeLength,
    MaximumLength,
    EventChordLength,
    MinimumStart,
    FirstStart,
    WrapperLength,
    WrapperStart,
    GraceStart,
    CalculatedLength,
    CalculatedStart,
}

const RULES: [Rule; 11] = [
    Rule::DurationLength,
    Rule::CumulativeLength,
    Rule::MaximumLength,
    Rule::EventChordLength,
    Rule::MinimumStart,
    Rule::FirstStart,
    Rule::WrapperLength,
    Rule::WrapperStart,
    Rule::GraceStart,
    Rule::CalculatedLength,
    Rule::CalculatedStart,
];

/// Why a computation stopped.
enum Stop {
    /// It reached a procedure of Scheme.
    Scheme,
    /// A moment did not fit (D61).
    Overflow,
    /// A procedure of Scheme it called threw.
    Thrown(Exception),
}

/// A moment, and whether it was computed from frozen music alone.
type T = Result<(Moment, bool), Stop>;

/// The property names a computation reads.
#[derive(Clone)]
pub(crate) struct Keys {
    length: Value,
    length_callback: Value,
    start_callback: Value,
    duration: Value,
    elements: Value,
    element: Value,
    elements_callback: Value,
}

impl Keys {
    pub(crate) fn new(heap: &mut Heap) -> Keys {
        Keys {
            length: heap.intern("length"),
            length_callback: heap.intern("length-callback"),
            start_callback: heap.intern("start-callback"),
            duration: heap.intern("duration"),
            elements: heap.intern("elements"),
            element: heap.intern("element"),
            elements_callback: heap.intern("elements-callback"),
        }
    }
}

/// Music a computation reads, with its handle if it is live: a frozen
/// node is given to Scheme as a new handle (`%music-children`'s).
#[derive(Clone, Copy)]
struct Mu {
    at: At,
    handle: Option<Value>,
}

impl Mu {
    fn of(heap: &Heap, v: Value) -> Option<Mu> {
        Some(Mu {
            at: At::of(heap, v)?,
            handle: Some(v),
        })
    }

    fn handle(self, heap: &mut Heap) -> Value {
        match (self.handle, self.at) {
            (Some(v), _) => v,
            (None, At::Frozen { tree, node }) => heap.wrap(crate::Music::frozen(tree, node)),
            (None, At::Live(_)) => Value::FALSE,
        }
    }
}

/// What a computation may ask of Scheme. A primitive cannot call it
/// (D70), so on its heap alone each question stops it; an iterator, with
/// its host, asks.
trait Ask {
    fn heap(&mut self) -> &mut Heap;
    /// `Calculated_sequential_music::calc_elements` with the music as
    /// `%music-calculated-elements` gives it: the list its
    /// `elements-callback` gives, `'()` without one.
    fn elements(&mut self, k: &Keys, m: Mu) -> Result<Value, Stop>;
    /// What the procedure `callback` gives of the music.
    fn call(&mut self, callback: Value, m: Mu) -> Result<Value, Stop>;
}

impl Ask for Heap {
    fn heap(&mut self) -> &mut Heap {
        self
    }
    fn elements(&mut self, _: &Keys, _: Mu) -> Result<Value, Stop> {
        Err(Stop::Scheme)
    }
    fn call(&mut self, _: Value, _: Mu) -> Result<Value, Stop> {
        Err(Stop::Scheme)
    }
}

/// An iterator's: the values Scheme gives stay rooted till the
/// computation ends.
struct Hosted<'a>(&'a mut dyn Host, &'a Asked);

impl Ask for Hosted<'_> {
    fn heap(&mut self) -> &mut Heap {
        self.0.heap()
    }

    fn elements(&mut self, k: &Keys, m: Mu) -> Result<Value, Stop> {
        if let Some(list) = self.1.list(m) {
            return Ok(list);
        }
        let heap = self.0.heap();
        let proc =
            m.at.property(heap, k.elements_callback)
                .and_then(|i| i.value())
                .unwrap_or(Value::NULL);
        if !heap.is_procedure(proc) {
            return Ok(Value::NULL);
        }
        // A frozen node, even one given by its handle, as a new handle
        // (`ly:music-deep-copy`'s), which the callback may thaw.
        let m = match m.at {
            At::Frozen { .. } => Mu { handle: None, ..m },
            At::Live(_) => m,
        };
        let list = self.call(proc, m)?;
        self.0.heap().root(list);
        Ok(list)
    }

    fn call(&mut self, callback: Value, m: Mu) -> Result<Value, Stop> {
        let handle = m.handle(self.0.heap());
        self.0.heap().root(handle);
        let v = self.0.call(callback, &[handle]).map_err(Stop::Thrown)?;
        self.0.heap().root(v);
        Ok(v)
    }
}

const ZERO: Moment = Moment {
    main: Rational::ZERO,
    grace: Rational::ZERO,
};

/// `Moment::infinity ()`.
const INFINITY: Moment = Moment {
    main: Rational::INFINITY,
    grace: Rational::ZERO,
};

/// The rule of a callback the music's types name; a procedure of Scheme
/// is none.
fn rule(heap: &Heap, callback: Value) -> Option<Rule> {
    let name = heap.primitive_name(callback)?;
    let i = CALLBACKS.iter().position(|&n| n == name)?;
    Some(RULES[i])
}

fn is_live(at: At) -> bool {
    matches!(at, At::Live(_))
}

/// `%music-moment`: what a procedure of Scheme gave, if a moment, else
/// zero; never kept.
fn moment(heap: &Heap, v: Value) -> T {
    Ok((heap.foreign::<Moment>(v).copied().unwrap_or(ZERO), false))
}

/// A frozen node's time if it was kept, or computed by `f` and kept if it
/// read no live music.
fn kept<A: Ask + ?Sized>(a: &mut A, at: At, which: Time, f: impl FnOnce(&mut A) -> T) -> T {
    let At::Frozen { tree, node } = at else {
        return f(a);
    };
    let found = a.heap().foreign::<Tree>(tree).map(|t| t.time(node, which));
    if let Some(Some(m)) = found {
        return Ok((m, true));
    }
    let (m, frozen) = f(a)?;
    if frozen && let Some(t) = a.heap().foreign::<Tree>(tree) {
        t.set_time(node, which, m);
    }
    Ok((m, frozen))
}

/// `Music::get_length`: a moment in `length`, else what its type's
/// `length-callback` gives, the duration's length without one.
fn length<A: Ask + ?Sized>(a: &mut A, k: &Keys, m: Mu) -> T {
    kept(a, m.at, Time::Length, |a| {
        let heap = a.heap();
        if let Some(v) = m.at.property(heap, k.length).and_then(|i| i.value())
            && let Some(&moment) = heap.foreign::<Moment>(v)
        {
            return Ok((moment, !is_live(m.at)));
        }
        let callback =
            m.at.immutable_property(heap, k.length_callback)
                .unwrap_or(Value::NULL);
        if !heap.is_procedure(callback) {
            return apply(a, k, Rule::DurationLength, m);
        }
        match rule(heap, callback) {
            Some(rule) => apply(a, k, rule, m),
            None => {
                let v = a.call(callback, m)?;
                moment(a.heap(), v)
            }
        }
    })
}

/// `Music::start_mom`: what its type's `start-callback` gives, else zero.
fn start<A: Ask + ?Sized>(a: &mut A, k: &Keys, m: Mu) -> T {
    kept(a, m.at, Time::Start, |a| {
        let heap = a.heap();
        let callback =
            m.at.immutable_property(heap, k.start_callback)
                .unwrap_or(Value::NULL);
        if !heap.is_procedure(callback) {
            return Ok((ZERO, !is_live(m.at)));
        }
        match rule(heap, callback) {
            Some(rule) => apply(a, k, rule, m),
            None => {
                let v = a.call(callback, m)?;
                moment(a.heap(), v)
            }
        }
    })
}

/// The items of the list `item`, each music or `None`.
fn musics(heap: &Heap, item: Option<Item>) -> Vec<Option<Mu>> {
    match item {
        Some(Item::Value(v)) => items(heap, v),
        Some(Item::Musics(all)) => all
            .into_iter()
            .map(|at| Some(Mu { at, handle: None }))
            .collect(),
        Some(Item::Music(_)) | None => Vec::new(),
    }
}

/// The items of the list `list`, each music or `None`.
fn items(heap: &Heap, list: Value) -> Vec<Option<Mu>> {
    let mut items = Vec::new();
    let mut p = list;
    while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        items.push(Mu::of(heap, item));
        p = rest;
    }
    items
}

/// The callback of `rule` called with the music `m`.
fn apply<A: Ask + ?Sized>(a: &mut A, k: &Keys, rule: Rule, m: Mu) -> T {
    let frozen = !is_live(m.at);
    let and = |(t, f): (Moment, bool)| (t, f && frozen);
    let heap = a.heap();
    let elements = |heap: &Heap| musics(heap, m.at.property(heap, k.elements));
    let element = |heap: &Heap| match m.at.property(heap, k.element) {
        Some(Item::Value(v)) => Mu::of(heap, v),
        Some(Item::Music(at)) => Some(Mu { at, handle: None }),
        _ => None,
    };
    match rule {
        Rule::DurationLength => duration_length(heap, k, m.at).map(and),
        Rule::CumulativeLength => {
            let items = elements(heap);
            cumulative_length(a, k, &items).map(and)
        }
        Rule::MaximumLength => {
            let items = elements(heap);
            maximum_length(a, k, &items).map(and)
        }
        Rule::EventChordLength => match duration(heap, k, m.at) {
            // A preset duration is used in chord repetitions.
            Some(d) => Ok((
                Moment::new(d.length().ok_or(Stop::Overflow)?, Rational::ZERO),
                frozen,
            )),
            None => {
                let items = elements(heap);
                maximum_length(a, k, &items).map(and)
            }
        },
        Rule::MinimumStart => {
            let items = elements(heap);
            minimum_start(a, k, &items).map(and)
        }
        Rule::FirstStart => {
            let items = elements(heap);
            first_start(a, k, &items).map(and)
        }
        Rule::WrapperLength => match element(heap) {
            Some(e) => length(a, k, e).map(and),
            None => Ok((ZERO, frozen)),
        },
        Rule::WrapperStart => match element(heap) {
            Some(e) => start(a, k, e).map(and),
            None => Ok((ZERO, frozen)),
        },
        Rule::GraceStart => {
            let (l, f) = apply(a, k, Rule::WrapperLength, m)?;
            let sum = l.main.checked_add(l.grace).ok_or(Stop::Overflow)?;
            Ok((Moment::new(Rational::ZERO, -sum), f))
        }
        // The elements come from a procedure of Scheme: never kept.
        Rule::CalculatedLength => {
            let list = a.elements(k, m)?;
            let items = items(a.heap(), list);
            let (t, _) = cumulative_length(a, k, &items)?;
            Ok((t, false))
        }
        Rule::CalculatedStart => {
            let list = a.elements(k, m)?;
            let items = items(a.heap(), list);
            let (t, _) = first_start(a, k, &items)?;
            Ok((t, false))
        }
    }
}

/// The duration in `duration`, if it is one.
fn duration(heap: &Heap, k: &Keys, at: At) -> Option<Duration> {
    let v = at.property(heap, k.duration)?.value()?;
    heap.foreign::<Duration>(v).copied()
}

/// `Music::duration_length_callback`.
fn duration_length(heap: &Heap, k: &Keys, at: At) -> T {
    let main = match duration(heap, k, at) {
        Some(d) => d.length().ok_or(Stop::Overflow)?,
        None => Rational::ZERO,
    };
    Ok((Moment::new(main, Rational::ZERO), !is_live(at)))
}

// `Music_sequence`'s: items that are no music are left out, where LilyPond
// reports a programming error.

fn cumulative_length<A: Ask + ?Sized>(a: &mut A, k: &Keys, items: &[Option<Mu>]) -> T {
    let (mut sum, mut frozen) = (ZERO, true);
    for &m in items.iter().flatten() {
        let (t, f) = length(a, k, m)?;
        sum = sum.checked_add(t).ok_or(Stop::Overflow)?;
        frozen &= f;
    }
    Ok((sum, frozen))
}

/// The longest of the definite lengths; infinite only if all are.
fn maximum_length<A: Ask + ?Sized>(a: &mut A, k: &Keys, items: &[Option<Mu>]) -> T {
    let (mut dur, mut frozen) = (ZERO, true);
    let (mut definite, mut indefinite) = (false, false);
    for item in items {
        let Some(m) = *item else {
            definite = true;
            continue;
        };
        let (len, f) = length(a, k, m)?;
        frozen &= f;
        if len.compare(INFINITY).is_lt() {
            definite = true;
            if dur.compare(len).is_lt() {
                dur = len;
            }
        } else {
            indefinite = true;
        }
    }
    let t = if definite || !indefinite {
        dur
    } else {
        INFINITY
    };
    Ok((t, frozen))
}

fn minimum_start<A: Ask + ?Sized>(a: &mut A, k: &Keys, items: &[Option<Mu>]) -> T {
    let (mut t, mut frozen) = (ZERO, true);
    for &m in items.iter().flatten() {
        let (s, f) = start(a, k, m)?;
        frozen &= f;
        if s.compare(t).is_lt() {
            t = s;
        }
    }
    Ok((t, frozen))
}

/// The grace time of the elements up to the first with a length.
fn first_start<A: Ask + ?Sized>(a: &mut A, k: &Keys, items: &[Option<Mu>]) -> T {
    let (mut grace, mut frozen) = (Rational::ZERO, true);
    for item in items {
        let Some(m) = *item else { break };
        let (s, f) = start(a, k, m)?;
        grace = grace.checked_add(s.grace).ok_or(Stop::Overflow)?;
        let (l, g) = length(a, k, m)?;
        frozen &= f && g;
        if !(l.main.is_zero() && l.grace.is_zero()) {
            break;
        }
    }
    Ok((Moment::new(Rational::ZERO, grace), frozen))
}

/// `get_length` (`start`: `start_mom`) of the music `handle` (or the
/// frozen node `at`) for N4's iterators, asking Scheme only for what is
/// Scheme's: an `elements-callback`'s elements and a callback of the
/// user's (MS3 step 55, D88). `Ok(None)` where it does not fit, which the
/// caller leaves to `ly:music-length` or `ly:music-start` for their error.
pub(crate) fn hosted(
    host: &mut dyn Host,
    k: &Keys,
    asked: &Asked,
    at: At,
    handle: Option<Value>,
    start_mom: bool,
) -> Result<Option<Moment>, Exception> {
    let depth = host.heap().root_depth();
    let m = Mu { at, handle };
    let mut a = Hosted(host, asked);
    let result = if start_mom {
        start(&mut a, k, m)
    } else {
        length(&mut a, k, m)
    };
    host.heap().unroot_to(depth);
    match result {
        Ok((t, _)) => Ok(Some(t)),
        Err(Stop::Thrown(e)) => Err(e),
        Err(Stop::Scheme | Stop::Overflow) => Ok(None),
    }
}

// --- the elements asked ahead (B23, D272) ---------------------------------------

/// LilyPond's own `elements-callback`s whose lists a translation's start asks
/// ahead, one music a step's check, and keeps for its `create_top`: they make
/// new music and say nothing, so one list each, read for the music's start and
/// length and given to its iterator, is what LilyPond's several calls made.
/// `make-tremolo-set` can warn, and is called where LilyPond calls it.
const ASKED_AHEAD: [&str; 3] = ["make-unfolded-set", "make-volta-set", "make-percent-set"];

/// What a `create_top` asks ahead of it ([`Asked::on`]): the music left to
/// look at, and the lists kept, by the music's identity.
#[derive(Clone, Default)]
pub(crate) struct Asked {
    begun: bool,
    todo: Vec<Mu>,
    lists: BTreeMap<(u64, u32), Value>,
    procs: Vec<Value>,
}

/// The identity of music: a frozen node's place, else its handle's.
fn identity(m: Mu) -> Option<(u64, u32)> {
    match (m.at, m.handle) {
        (At::Frozen { tree, node }, _) => Some((tree.to_bits(), node)),
        (At::Live(_), Some(v)) => Some((v.to_bits(), u32::MAX)),
        (At::Live(_), None) => None,
    }
}

impl Asked {
    /// Begun at `music`, the translation's.
    pub(crate) fn begin(host: &mut dyn Host, music: Value) -> Asked {
        let procs = ASKED_AHEAD
            .iter()
            .filter_map(|name| host.lily(name))
            .collect();
        Asked {
            begun: true,
            todo: Mu::of(host.heap(), music).into_iter().collect(),
            lists: BTreeMap::new(),
            procs,
        }
    }

    /// Whether it has not begun.
    pub(crate) fn is_new(&self) -> bool {
        !self.begun
    }

    /// The list kept of `m`, if one was.
    fn list(&self, m: Mu) -> Option<Value> {
        self.lists.get(&identity(m)?).copied()
    }

    /// The list kept of the music at `at`, given by `handle` if live.
    pub(crate) fn list_at(&self, at: At, handle: Option<Value>) -> Option<Value> {
        self.list(Mu { at, handle })
    }

    /// Asks on until it has looked at all the music, `Ok(true)`, or `due`
    /// says the step's time is spent, `Ok(false)`. Each list is rooted, as
    /// the iterators hold it, till the translation ends. What the music
    /// under a list's music is, is that list's, whose music is looked at in
    /// its place.
    pub(crate) fn on(
        &mut self,
        host: &mut dyn Host,
        k: &Keys,
        due: &dyn Fn() -> bool,
    ) -> Result<bool, Exception> {
        let mut looked = 0u32;
        while let Some(m) = self.todo.pop() {
            looked += 1;
            if looked.is_multiple_of(256) && due() {
                self.todo.push(m);
                return Ok(false);
            }
            let heap = host.heap();
            let callback =
                m.at.property(heap, k.elements_callback)
                    .and_then(|i| i.value())
                    .filter(|p| self.procs.contains(p));
            let Some(callback) = callback else {
                let mut children = musics(heap, m.at.property(heap, k.elements));
                if let Some(e) = match m.at.property(heap, k.element) {
                    Some(Item::Value(v)) => Mu::of(heap, v),
                    Some(Item::Music(at)) => Some(Mu { at, handle: None }),
                    _ => None,
                } {
                    children.push(Some(e));
                }
                self.todo.extend(children.into_iter().rev().flatten());
                continue;
            };
            let Some(id) = identity(m) else {
                continue;
            };
            if self.lists.contains_key(&id) {
                continue;
            }
            // As `Hosted::elements` gives it: a frozen node as a new handle.
            let given = match m.at {
                At::Frozen { .. } => Mu { handle: None, ..m },
                At::Live(_) => m,
            };
            let handle = given.handle(host.heap());
            let depth = host.heap().root_depth();
            host.heap().root(handle);
            let list = host.call(callback, &[handle]);
            host.heap().unroot_to(depth);
            let list = list?;
            host.heap().root(list);
            self.lists.insert(id, list);
            let items = items(host.heap(), list);
            self.todo.extend(items.into_iter().rev().flatten());
            if due() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Done with: what it kept is the iterators' now.
    pub(crate) fn clear(&mut self) {
        self.todo.clear();
        self.lists.clear();
    }
}

// --- the primitives ------------------------------------------------------------------

/// A moment the computation of `name` gave, with `subr`'s error if it did
/// not fit; for one that reached Scheme, a call of `name`'s fallback, `%`
/// and its name, in its place.
fn give(cx: &mut Cx, name: &str, subr: &str, music: Value, result: T) -> R {
    match result {
        Ok((m, _)) => Ok(cx.heap_mut().wrap(m)),
        Err(Stop::Overflow) => Err(crate::overflow(cx, subr)),
        // A primitive's computation calls no Scheme.
        Err(Stop::Scheme | Stop::Thrown(_)) => {
            let fallback = alloc::format!("%{name}");
            match cx.global("lily", &fallback) {
                Some(fallback) => Ok(cx.tail_call(fallback, &[music])),
                None => Err(cx.error("misc-error", Some(name), "no fallback in (lily)", &[])),
            }
        }
    }
}

/// `LY_ASSERT_SMOB (Music, m, 1)`, then the music and the names it reads.
fn music_arg(cx: &mut Cx, subr: &str, music: Value) -> Result<(Mu, Keys), Thrown> {
    let Some(m) = Mu::of(cx.heap(), music) else {
        return Err(cx.wrong_type(subr, 1, music, "Music"));
    };
    Ok((m, Keys::new(cx.heap_mut())))
}

/// `(ly:music-length music)`.
pub(crate) fn music_length(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:music-length";
    let (m, k) = music_arg(cx, SUBR, args[0])?;
    let result = length(cx.heap_mut(), &k, m);
    give(cx, SUBR, SUBR, args[0], result)
}

/// `(ly:music-start music)`.
pub(crate) fn music_start(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:music-start";
    let (m, k) = music_arg(cx, SUBR, args[0])?;
    let result = start(cx.heap_mut(), &k, m);
    give(cx, SUBR, SUBR, args[0], result)
}

/// The callback `CALLBACKS[i]` of `music`.
pub(crate) fn callback(cx: &mut Cx, i: usize, music: Value) -> R {
    let (m, k) = music_arg(cx, SUBRS[i], music)?;
    let result = apply(cx.heap_mut(), &k, RULES[i], m);
    give(cx, CALLBACKS[i], SUBRS[i], music, result)
}

/// `(%music-children music key)`: the property `key` as
/// `ly:music-property` gives it, but a frozen node's music as new handles
/// of its children and its lists copied, so that nothing is thawed.
pub(crate) fn music_children(cx: &mut Cx, args: &[Value]) -> R {
    let Some(at) = At::of(cx.heap(), args[0]) else {
        return Ok(Value::NULL);
    };
    let heap = cx.heap_mut();
    // A frozen node's children are frozen.
    let handle = |heap: &mut Heap, at: At| match at {
        At::Frozen { tree, node } => heap.wrap(crate::Music::frozen(tree, node)),
        At::Live(_) => Value::UNSPECIFIED,
    };
    Ok(match at.property(heap, args[1]) {
        None => Value::NULL,
        Some(Item::Value(v)) if is_live(at) => v,
        Some(Item::Value(v)) => crate::deep_copy(heap, v),
        Some(Item::Music(child)) => handle(heap, child),
        Some(Item::Musics(children)) => {
            let items: alloc::vec::Vec<Value> =
                children.into_iter().map(|c| handle(heap, c)).collect();
            heap.list(&items)
        }
    })
}

/// `(%music-immutable-property music key)`: the property `key` of the
/// type's, which the C++ reads a type's callbacks from.
pub(crate) fn music_immutable_property(cx: &mut Cx, args: &[Value]) -> R {
    Ok(At::of(cx.heap(), args[0])
        .and_then(|at| at.immutable_property(cx.heap(), args[1]))
        .unwrap_or(Value::NULL))
}

/// The fallbacks, in `(lily)`: what the C++ does, for music whose length or
/// start needs Scheme. Each element's is native again where it can be.
pub(crate) const PRELUDE: &str = r#"
(define (%music-moment x) (if (ly:moment? x) x (ly:make-moment 0)))
(define (%music-elements m) (%music-children m 'elements))
(define (%music-element-length m)
  (let ((e (%music-children m 'element)))
    (if (ly:music? e) (ly:music-length e) (ly:make-moment 0))))
(define (%music-cumulative-length elements)
  (let loop ((l elements) (sum (ly:make-moment 0)))
    (cond ((not (pair? l)) sum)
          ((ly:music? (car l)) (loop (cdr l) (ly:moment-add sum (ly:music-length (car l)))))
          (else (loop (cdr l) sum)))))
(define (%music-maximum-length elements)
  (let loop ((l elements) (dur (ly:make-moment 0)) (definite #f) (indefinite #f))
    (cond ((not (pair? l))
           (if (or definite (not indefinite)) dur (ly:make-moment +inf.0)))
          ((ly:music? (car l))
           (let ((len (ly:music-length (car l))))
             (if (ly:moment<? len (ly:make-moment +inf.0))
                 (loop (cdr l) (if (ly:moment<? dur len) len dur) #t indefinite)
                 (loop (cdr l) dur definite #t))))
          (else (loop (cdr l) dur #t indefinite)))))
(define (%music-minimum-start elements)
  (let loop ((l elements) (m (ly:make-moment 0)))
    (cond ((not (pair? l)) m)
          ((ly:music? (car l))
           (let ((s (ly:music-start (car l))))
             (loop (cdr l) (if (ly:moment<? s m) s m))))
          (else (loop (cdr l) m)))))
(define (%music-first-start elements)
  (let loop ((l elements) (accum (ly:make-moment 0)))
    (if (and (pair? l) (ly:music? (car l)))
        (let ((accum (ly:moment-add accum (ly:make-moment 0 (ly:moment-grace (ly:music-start (car l)))))))
          (if (equal? (ly:music-length (car l)) (ly:make-moment 0))
              (loop (cdr l) accum)
              accum))
        accum)))
;; A frozen node is given to the callback as a new handle, which it thaws
;; in place of the node's.
(define (%music-calculated-elements m)
  (let ((proc (ly:music-property m 'elements-callback)))
    (if (procedure? proc)
        (proc (if (%music-frozen? m) (ly:music-deep-copy m) m))
        '())))
(define (%ly:music-length m)
  (let ((length (ly:music-property m 'length)))
    (if (ly:moment? length)
        length
        (let ((callback (%music-immutable-property m 'length-callback)))
          (%music-moment
           ((if (procedure? callback) callback ly:music::duration-length-callback) m))))))
(define (%ly:music-start m)
  (let ((callback (%music-immutable-property m 'start-callback)))
    (if (procedure? callback) (%music-moment (callback m)) (ly:make-moment 0))))
(define (%ly:music-sequence::cumulative-length-callback m)
  (%music-cumulative-length (%music-elements m)))
(define (%ly:music-sequence::maximum-length-callback m)
  (%music-maximum-length (%music-elements m)))
(define (%ly:music-sequence::event-chord-length-callback m)
  (let ((d (ly:music-property m 'duration)))
    (if (ly:duration? d)
        (ly:duration->moment d)
        (%music-maximum-length (%music-elements m)))))
(define (%ly:music-sequence::minimum-start-callback m)
  (%music-minimum-start (%music-elements m)))
(define (%ly:music-sequence::first-start-callback m)
  (%music-first-start (%music-elements m)))
(define %ly:music-wrapper::length-callback %music-element-length)
(define (%ly:music-wrapper::start-callback m)
  (let ((e (%music-children m 'element)))
    (if (ly:music? e) (ly:music-start e) (ly:make-moment 0))))
(define (%ly:grace-music::start-callback m)
  (let ((l (%music-element-length m)))
    (ly:make-moment 0 (- (+ (ly:moment-main l) (ly:moment-grace l))))))
(define (%ly:calculated-sequential-music::length m)
  (%music-cumulative-length (%music-calculated-elements m)))
(define (%ly:calculated-sequential-music::start m)
  (%music-first-start (%music-calculated-elements m)))
"#;
