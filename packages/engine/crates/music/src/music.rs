//! Music (N2, 04 §6.2) and music functions, with the `ly:` functions of
//! `music-scheme.cc` and `music-function-scheme.cc` that the parser and
//! `define-syntax-function` call, and `define-music-types.scm`'s
//! `make-music` (lib.rs) over [`crate::define_music_types`]' table.
//!
//! A [`Music`] is LilyPond's `Music`, a prob of mutable properties and its
//! type's immutable ones, held one of two ways:
//!
//! - **live:** both property lists on the Scheme heap, as LilyPond holds
//!   them, which `ly:make-music` makes and Scheme changes;
//! - **frozen:** a handle of a node of a [`Tree`] (`frozen.rs`), which
//!   [`crate::freeze`] makes of live music when it is given back to the
//!   parser.
//!
//! Reading a property that is no list and no music reads the node, and so
//! does printing. Reading one that is, any change and
//! `ly:music-mutable-properties` first [`thaw`] the handle into live music
//! in place, so that Scheme sees what D65's music showed it.
//! `ly:music-deep-copy` of a handle is a new handle of the same node.
//!
//! [`At`] reads either kind without thawing, for native code: lengths and
//! starts (`length.rs`) and `equal?`, which is `Prob::equal_p`'s. `origin`
//! is an [`crate::Input`]. The C++ callbacks that types name and the engine
//! does not have yet are placeholders (D67).

use alloc::string::String;

use alloc::vec::Vec;
use core::any::Any;

use lily_engine_scheme::{Cx, Foreign, Heap, ImageReader, ImageWriter, Tag, Thrown, Tracer, Value};

use crate::dispatcher::StreamEvent;
use crate::frozen::{Found, Tree, thaw};
use crate::{Input, R};

/// A music object, frozen or live.
pub struct Music(pub(crate) State);

#[derive(Clone, Copy)]
pub(crate) enum State {
    /// Node `node` of the [`Tree`] `tree`.
    Frozen {
        tree: Value,
        node: u32,
    },
    Live(Live),
}

/// Live music: its kind if its type is one of the 131, its immutable
/// property list as `ly:make-music` was given it, and its mutable one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Live {
    pub kind: Option<u8>,
    pub immutable: Value,
    pub props: Value,
}

/// How a prob prints, up to its property lists.
pub(crate) const PRINT_HEAD: &str = "#<Prob: Music C++: Music";
/// `Prob::print_smob`'s end, a newline after it (OPEN C6).
pub(crate) const PRINT_TAIL: &str = " >\n";

impl Music {
    /// A handle of node `node` of the tree `tree`.
    pub(crate) fn frozen(tree: Value, node: u32) -> Music {
        Music(State::Frozen { tree, node })
    }

    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<Music> {
        Some(Music(match r.u8()? {
            0 => State::Frozen {
                tree: r.value()?,
                node: r.u32()?,
            },
            1 => State::Live(Live {
                kind: match r.u8()? {
                    0 => None,
                    _ => Some(r.u8()?),
                },
                immutable: r.value()?,
                props: r.value()?,
            }),
            _ => return None,
        }))
    }
}

impl Foreign for Music {
    fn type_name(&self) -> &'static str {
        "Music"
    }

    /// A copy for S5's undo log (MS6 step 3, D235): a parse's segment made
    /// it, and what changes it after (a translation) is undone with it.
    fn snapshot(&self) -> Option<alloc::boxed::Box<dyn Foreign>> {
        Some(alloc::boxed::Box::new(Music(self.0)))
    }

    /// As LilyPond's prob prints, its mutable properties and then its
    /// immutable ones.
    fn print(&self, heap: &Heap, out: &mut String) {
        match self.0 {
            State::Frozen { tree, node } => match heap.foreign::<Tree>(tree) {
                Some(t) => t.print_node(heap, node, out),
                None => out.push_str("#<Music>"),
            },
            State::Live(live) => {
                out.push_str(PRINT_HEAD);
                heap.write(live.props, out);
                heap.write(live.immutable, out);
                out.push_str(PRINT_TAIL);
            }
        }
    }

    /// `Prob::equal_p`: the immutable lists and then the mutable ones
    /// entry by entry, `origin` skipped, keys `eq?` and values `equal?`.
    fn equal(&self, other: &dyn Foreign, heap: &Heap) -> bool {
        let Some(other) = (other as &dyn Any).downcast_ref::<Music>() else {
            return false;
        };
        music_equal(heap, At::of_state(&self.0), At::of_state(&other.0))
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        match self.0 {
            State::Frozen { tree, .. } => tracer.trace(tree),
            State::Live(live) => {
                tracer.trace(live.immutable);
                tracer.trace(live.props);
            }
        }
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        match self.0 {
            State::Frozen { tree, node } => {
                out.u8(0);
                out.value(tree);
                out.u32(node);
            }
            State::Live(live) => {
                out.u8(1);
                match live.kind {
                    Some(kind) => {
                        out.u8(1);
                        out.u8(kind);
                    }
                    None => out.u8(0),
                }
                out.value(live.immutable);
                out.value(live.props);
            }
        }
        true
    }
}

// --- what the parser and the primitives share -----------------------------------------

/// The immutable property list of `music`, or None for what is no music.
fn immutable(heap: &Heap, music: Value) -> Option<Value> {
    match heap.foreign::<Music>(music)?.0 {
        State::Frozen { tree, node } => {
            let t = heap.foreign::<Tree>(tree)?;
            Some(t.immutable(t.nodes()[node as usize].kind))
        }
        State::Live(live) => Some(live.immutable),
    }
}

/// The live properties of `music`, thawed first.
fn live(heap: &mut Heap, music: Value) -> Option<Live> {
    thaw(heap, music);
    match heap.foreign::<Music>(music)?.0 {
        State::Live(live) => Some(live),
        State::Frozen { .. } => None,
    }
}

fn set_props(heap: &mut Heap, music: Value, props: Value) {
    if let Some(Music(State::Live(live))) = heap.foreign_mut::<Music>(music) {
        live.props = props;
    }
}

// --- reading without thawing -----------------------------------------------------------

/// Music as native code reads it: live music, or a node of a frozen tree,
/// which nothing here thaws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum At {
    Live(Live),
    Frozen { tree: Value, node: u32 },
}

/// A property as [`At`] finds it: a value, or a frozen node's children.
pub(crate) enum Item {
    Value(Value),
    Music(At),
    Musics(Vec<At>),
}

impl At {
    /// `music` if it is music.
    pub(crate) fn of(heap: &Heap, music: Value) -> Option<At> {
        Some(At::of_state(&heap.foreign::<Music>(music)?.0))
    }

    fn of_state(state: &State) -> At {
        match *state {
            State::Frozen { tree, node } => At::Frozen { tree, node },
            State::Live(live) => At::Live(live),
        }
    }

    /// Its immutable property list.
    pub(crate) fn immutable(self, heap: &Heap) -> Value {
        match self {
            At::Live(live) => live.immutable,
            At::Frozen { tree, node } => heap
                .foreign::<Tree>(tree)
                .map_or(Value::NULL, |t| t.immutable(t.nodes()[node as usize].kind)),
        }
    }

    /// Its mutable properties in their order.
    pub(crate) fn entries(self, heap: &Heap) -> Vec<(Value, Item)> {
        match self {
            At::Live(live) => {
                let mut entries = Vec::new();
                let mut p = live.props;
                while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                    if let (Some(key), Some(value)) = (heap.car(entry), heap.cdr(entry)) {
                        entries.push((key, Item::Value(value)));
                    }
                    p = rest;
                }
                entries
            }
            At::Frozen { tree, node } => {
                let Some(t) = heap.foreign::<Tree>(tree) else {
                    return Vec::new();
                };
                let child = |c| At::Frozen { tree, node: c };
                t.found(node)
                    .into_iter()
                    .map(|(key, found)| {
                        let item = match found {
                            Found::Value(v) => Item::Value(v),
                            Found::Music(c) => Item::Music(child(c)),
                            Found::Musics(cs) => Item::Musics(cs.into_iter().map(child).collect()),
                        };
                        (key, item)
                    })
                    .collect()
            }
        }
    }

    /// Its mutable property `key`.
    pub(crate) fn mutable(self, heap: &Heap, key: Value) -> Option<Item> {
        match self {
            At::Live(live) => assq(heap, key, live.props)
                .and_then(|e| heap.cdr(e))
                .map(Item::Value),
            At::Frozen { tree, node } => {
                let t = heap.foreign::<Tree>(tree)?;
                let at = t.props(node).iter().position(|&(k, _)| k == key)?;
                let (_, item) = t.found(node).into_iter().nth(at)?;
                Some(match item {
                    Found::Value(v) => Item::Value(v),
                    Found::Music(c) => Item::Music(At::Frozen { tree, node: c }),
                    Found::Musics(cs) => Item::Musics(
                        cs.into_iter()
                            .map(|c| At::Frozen { tree, node: c })
                            .collect(),
                    ),
                })
            }
        }
    }

    /// Its immutable property `key`.
    pub(crate) fn immutable_property(self, heap: &Heap, key: Value) -> Option<Value> {
        assq(heap, key, self.immutable(heap)).and_then(|e| heap.cdr(e))
    }

    /// `get_property`: the mutable property `key`, then the immutable one.
    pub(crate) fn property(self, heap: &Heap, key: Value) -> Option<Item> {
        self.mutable(heap, key)
            .or_else(|| self.immutable_property(heap, key).map(Item::Value))
    }
}

impl Item {
    /// The value it is, if it holds no frozen music.
    pub(crate) fn value(&self) -> Option<Value> {
        match *self {
            Item::Value(v) => Some(v),
            _ => None,
        }
    }

    /// `unsmob<Music>`: the music it is.
    pub(crate) fn music(&self, heap: &Heap) -> Option<At> {
        match *self {
            Item::Value(v) => At::of(heap, v),
            Item::Music(at) => Some(at),
            Item::Musics(_) => None,
        }
    }

    /// `as_ly_smob_list<Music>`: the items of a list, each music or None.
    pub(crate) fn musics(&self, heap: &Heap) -> Vec<Option<At>> {
        match self {
            Item::Value(v) => {
                let mut items = Vec::new();
                let mut p = *v;
                while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                    items.push(At::of(heap, item));
                    p = rest;
                }
                items
            }
            Item::Music(_) => Vec::new(),
            Item::Musics(all) => all.iter().copied().map(Some).collect(),
        }
    }
}

// --- equal? --------------------------------------------------------------------------

/// `Prob::equal_p` of two music objects.
fn music_equal(heap: &Heap, a: At, b: At) -> bool {
    if a == b {
        return true;
    }
    let origin = |key: Value| key.tag() == Tag::Symbol && heap.name(key) == Some("origin");
    let (x, y) = (a.immutable(heap), b.immutable(heap));
    if x != y {
        let entries = |alist| {
            At::Live(Live {
                kind: None,
                immutable: Value::NULL,
                props: alist,
            })
            .entries(heap)
        };
        if !entries_equal(heap, entries(x), entries(y), &origin) {
            return false;
        }
    }
    entries_equal(heap, a.entries(heap), b.entries(heap), &origin)
}

/// `Prob::equal_p` of two stream events, each its own properties and its
/// music: those properties and then the music's mutable ones, entry by
/// entry, `origin` skipped.
pub(crate) fn event_equal(heap: &Heap, a: (Value, Value), b: (Value, Value)) -> bool {
    let origin = |key: Value| key.tag() == Tag::Symbol && heap.name(key) == Some("origin");
    let entries = |(props, music): (Value, Value)| {
        let mut all = At::Live(Live {
            kind: None,
            immutable: Value::NULL,
            props,
        })
        .entries(heap);
        if let Some(at) = At::of(heap, music) {
            all.extend(at.entries(heap));
        }
        all
    };
    entries_equal(heap, entries(a), entries(b), &origin)
}

/// Two property lists entry by entry, the entries `skip` names left out.
fn entries_equal(
    heap: &Heap,
    a: Vec<(Value, Item)>,
    b: Vec<(Value, Item)>,
    skip: &dyn Fn(Value) -> bool,
) -> bool {
    let mut a = a.into_iter().filter(|(key, _)| !skip(*key));
    let mut b = b.into_iter().filter(|(key, _)| !skip(*key));
    loop {
        match (a.next(), b.next()) {
            (None, None) => return true,
            (Some((k, x)), Some((l, y))) if k == l && item_equal(heap, &x, &y) => {}
            _ => return false,
        }
    }
}

/// `equal?` of two properties' values, where either may be frozen music.
fn item_equal(heap: &Heap, a: &Item, b: &Item) -> bool {
    match (a, b) {
        (Item::Value(x), Item::Value(y)) => heap.equal(*x, *y),
        (Item::Musics(xs), Item::Musics(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(&x, &y)| music_equal(heap, x, y))
        }
        (Item::Musics(xs), Item::Value(v)) | (Item::Value(v), Item::Musics(xs)) => {
            // A circular list is no list, and is not walked.
            crate::is_list(heap, *v) && {
                let items = Item::Value(*v).musics(heap);
                items.len() == xs.len()
                    && items
                        .iter()
                        .zip(xs)
                        .all(|(item, &x)| item.is_some_and(|y| music_equal(heap, x, y)))
            }
        }
        _ => match (a.music(heap), b.music(heap)) {
            (Some(x), Some(y)) => music_equal(heap, x, y),
            _ => false,
        },
    }
}

/// The name of `music`'s type: `NoteEvent`.
pub fn music_name(heap: &Heap, music: Value) -> Option<&str> {
    let entry = assq_named(heap, "name", immutable(heap, music)?)?;
    heap.name(heap.cdr(entry)?)
}

/// `is_mus_type`: whether `class` is among the `types` of `music`.
pub fn is_music_type(heap: &Heap, music: Value, class: &str) -> bool {
    let Some(types) = immutable(heap, music)
        .and_then(|alist| assq_named(heap, "types", alist))
        .and_then(|entry| heap.cdr(entry))
    else {
        return false;
    };
    let mut p = types;
    while let (Some(t), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if t.tag() == Tag::Symbol && heap.name(t) == Some(class) {
            return true;
        }
        p = rest;
    }
    false
}

/// `get_property`: the mutable property `key`, then the immutable one. A
/// frozen handle is thawed for a list or music.
pub fn music_property(heap: &mut Heap, music: Value, key: Value) -> Option<Value> {
    if let State::Frozen { tree, node } = heap.foreign::<Music>(music)?.0 {
        match heap.foreign::<Tree>(tree)?.immediate(node, key) {
            Ok(Some(v)) => return Some(v),
            Ok(None) => return assq(heap, key, immutable(heap, music)?).and_then(|e| heap.cdr(e)),
            Err(()) => {}
        }
    }
    let live = live(heap, music)?;
    assq(heap, key, live.props)
        .or_else(|| assq(heap, key, live.immutable))
        .and_then(|entry| heap.cdr(entry))
}

/// Puts `(key . value)` in front of `music`'s mutable properties, as the
/// parser sets an event function's `direction` (D65).
pub fn push_property(heap: &mut Heap, music: Value, key: Value, value: Value) {
    let Some(live) = live(heap, music) else {
        return;
    };
    let entry = heap.cons(key, value);
    let props = heap.cons(entry, live.props);
    set_props(heap, music, props);
}

/// Replaces the mutable properties of `music` with `alist`.
pub fn set_mutable_properties(heap: &mut Heap, music: Value, alist: Value) {
    if live(heap, music).is_some() {
        set_props(heap, music, alist);
    }
}

/// The tree and node of `music` if it is a handle of a frozen tree.
pub fn frozen_node(heap: &Heap, music: Value) -> Option<(&Tree, u32)> {
    match heap.foreign::<Music>(music)?.0 {
        State::Frozen { tree, node } => Some((heap.foreign::<Tree>(tree)?, node)),
        State::Live(_) => None,
    }
}

/// Whether `music` is a handle of a frozen tree.
pub fn is_frozen(heap: &Heap, music: Value) -> bool {
    matches!(
        heap.foreign::<Music>(music),
        Some(Music(State::Frozen { .. }))
    )
}

/// A music function: its signature, the predicate of its result first, an
/// optional parameter's as `(predicate . default)`, and its procedure. A
/// call of it from Scheme calls `call`, lib.rs' `%music-function-call`
/// (`Music_function::call`), with it first; `#f` before that is defined.
#[derive(Clone, Copy, Debug)]
pub struct MusicFunction {
    pub signature: Value,
    pub procedure: Value,
    pub call: Value,
}

impl MusicFunction {
    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<MusicFunction> {
        Some(MusicFunction {
            signature: r.value()?,
            procedure: r.value()?,
            call: r.value()?,
        })
    }
}

impl Foreign for MusicFunction {
    fn type_name(&self) -> &'static str {
        "Music_function"
    }

    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<Music function ");
        heap.write(self.procedure, out);
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.signature);
        tracer.trace(self.procedure);
        tracer.trace(self.call);
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.value(self.signature);
        out.value(self.procedure);
        out.value(self.call);
        true
    }

    /// LilyPond's music functions are applicable smobs.
    fn applicable(&self) -> Option<Value> {
        (self.call != Value::FALSE).then_some(self.call)
    }
}

/// `scm_assq`: the entry of `alist` whose key is `key`.
pub(crate) fn assq(heap: &Heap, key: Value, alist: Value) -> Option<Value> {
    heap.assq(key, alist)
}

/// The entry of `alist` whose key is the symbol `name`.
pub(crate) fn assq_named(heap: &Heap, name: &str, alist: Value) -> Option<Value> {
    let mut p = alist;
    while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if let Some(key) = heap.car(entry)
            && key.tag() == Tag::Symbol
            && heap.name(key) == Some(name)
        {
            return Some(entry);
        }
        p = rest;
    }
    None
}

fn check_music(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<(), Thrown> {
    let ok = cx.heap().foreign::<Music>(v).is_some();
    crate::check(cx, ok, subr, position, v, "Music")
}

fn check_symbol(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<(), Thrown> {
    crate::check(cx, v.tag() == Tag::Symbol, subr, position, v, "symbol")
}

/// `(ly:make-music props)`: live music whose immutable properties `props`
/// are, as `music-name-to-property-table` holds them, with no mutable
/// properties.
pub(crate) fn make_music(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:make-music";
    let ok = crate::is_list(cx.heap(), args[0]);
    crate::check(cx, ok, SUBR, 1, args[0], "list")?;
    let heap = cx.heap_mut();
    let kind = assq_named(heap, "name", args[0])
        .and_then(|entry| heap.cdr(entry))
        .and_then(|name| heap.name(name))
        .and_then(crate::types::kind);
    Ok(heap.wrap(Music(State::Live(Live {
        kind,
        immutable: args[0],
        props: Value::NULL,
    }))))
}

pub(crate) fn is_music(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<Music>(cx, args[0])
}

/// `(ly:event? x)`: music that is a post-event.
pub(crate) fn is_event(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(is_music_type(cx.heap(), args[0], "post-event")))
}

/// `(ly:music-property music key [default])`: `'()` when it has none.
pub(crate) fn ly_music_property(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:music-property";
    check_music(cx, SUBR, 1, args[0])?;
    check_symbol(cx, SUBR, 2, args[1])?;
    let fallback = args.get(2).copied().unwrap_or(Value::NULL);
    // `ly_prob_property`: a value that is `'()` is none, so the default.
    Ok(music_property(cx.heap_mut(), args[0], args[1])
        .filter(|&v| v != Value::NULL)
        .unwrap_or(fallback))
}

/// `set_property`: an entry `music` has is changed in place, else one is
/// added in front, as `scm_assq_set_x` does.
pub fn set_property(heap: &mut Heap, music: Value, key: Value, value: Value) {
    let Some(live) = live(heap, music) else {
        return;
    };
    match assq(heap, key, live.props) {
        Some(entry) => {
            heap.set_cdr(entry, value);
        }
        None => push_property(heap, music, key, value),
    }
}

/// `(ly:music-set-property! music key value)`.
pub(crate) fn music_set_property(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:music-set-property!";
    check_music(cx, SUBR, 1, args[0])?;
    check_symbol(cx, SUBR, 2, args[1])?;
    set_property(cx.heap_mut(), args[0], args[1], args[2]);
    Ok(Value::UNSPECIFIED)
}

pub(crate) fn music_mutable_properties(cx: &mut Cx, args: &[Value]) -> R {
    check_music(cx, "ly:music-mutable-properties", 1, args[0])?;
    Ok(mutable_properties(cx.heap_mut(), args[0]))
}

/// `ly:music-mutable-properties`: the mutable property list of `music`,
/// thawed first; `'()` of what is no music.
pub fn mutable_properties(heap: &mut Heap, music: Value) -> Value {
    live(heap, music).map_or(Value::NULL, |live| live.props)
}

/// `music_deep_copy`: music is copied with what its properties hold, as
/// lists and pairs are; anything else is shared. A frozen handle's copy is
/// a new handle of its node.
pub fn deep_copy(heap: &mut Heap, v: Value) -> Value {
    match heap.foreign::<Music>(v).map(|m| &m.0) {
        Some(&State::Frozen { tree, node }) => {
            return heap.wrap(Music(State::Frozen { tree, node }));
        }
        Some(&State::Live(live)) => {
            let props = deep_copy(heap, live.props);
            return heap.wrap(Music(State::Live(Live { props, ..live })));
        }
        None => {}
    }
    match (heap.car(v), heap.cdr(v)) {
        (Some(car), Some(cdr)) if v.tag() == Tag::Pair => {
            let car = deep_copy(heap, car);
            let cdr = deep_copy(heap, cdr);
            heap.cons(car, cdr)
        }
        _ => v,
    }
}

/// `(ly:music-deep-copy music [origin])`: with `origin`, an input location
/// or music whose `origin` is taken, the copy's music is given it as
/// `set_origin` does: music in lists too, but not what music holds.
pub(crate) fn music_deep_copy(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:music-deep-copy";
    let copy = deep_copy(cx.heap_mut(), args[0]);
    let Some(&origin) = args.get(1) else {
        return Ok(copy);
    };
    let key = cx.heap_mut().intern("origin");
    let origin = match cx.heap().foreign::<Music>(origin) {
        Some(_) => music_property(cx.heap_mut(), origin, key).unwrap_or(Value::NULL),
        None => origin,
    };
    if origin == Value::FALSE || origin == Value::NULL {
        return Ok(copy);
    }
    let ok = cx.heap().foreign::<Input>(origin).is_some();
    crate::check(cx, ok, SUBR, 2, origin, "Input")?;
    set_origin(cx.heap_mut(), copy, key, origin);
    Ok(copy)
}

/// `set_origin`: each music of `m`, a music or a list of them however
/// nested, gets `origin`.
pub(crate) fn set_origin(heap: &mut Heap, m: Value, key: Value, origin: Value) {
    let mut p = m;
    while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        set_origin(heap, item, key, origin);
        p = rest;
    }
    if heap.foreign::<Music>(p).is_some() {
        set_property(heap, p, key, origin);
    }
}

/// `(%music-set-origin! m origin)`, `ly:set-origin!` once its default,
/// `(*location*)`, is given (lib.rs): `origin`, or the `origin` of music
/// given as it, set on `m`'s music by `set_origin`; nothing for `#f` and
/// `'()`. `m` is returned.
pub(crate) fn music_set_origin(cx: &mut Cx, args: &[Value]) -> R {
    let key = cx.heap_mut().intern("origin");
    let origin = match cx.heap().foreign::<Music>(args[1]) {
        Some(_) => music_property(cx.heap_mut(), args[1], key).unwrap_or(Value::NULL),
        None => args[1],
    };
    if origin == Value::FALSE || origin == Value::NULL {
        return Ok(args[0]);
    }
    let ok = cx.heap().foreign::<Input>(origin).is_some();
    crate::check(cx, ok, "ly:set-origin!", 2, origin, "Input")?;
    set_origin(cx.heap_mut(), args[0], key, origin);
    Ok(args[0])
}

/// `(ly:music-duration-length music)`: the length of its `duration`, zero
/// without one, where LilyPond also reports a programming error.
pub(crate) fn music_duration_length(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:music-duration-length";
    check_music(cx, SUBR, 1, args[0])?;
    let key = cx.heap_mut().intern("duration");
    let d = music_property(cx.heap_mut(), args[0], key)
        .and_then(|v| cx.heap().foreign::<crate::Duration>(v).copied());
    let length = match d {
        Some(d) => d.length(),
        None => Some(crate::Rational::ZERO),
    };
    match length {
        Some(main) => Ok(cx
            .heap_mut()
            .wrap(crate::Moment::new(main, crate::Rational::ZERO))),
        None => Err(crate::overflow(cx, SUBR)),
    }
}

/// `(ly:music-duration-compress music factor)`: its `duration`, if it
/// has one, compressed by the moment `factor`'s main part.
pub(crate) fn music_duration_compress(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:music-duration-compress";
    check_music(cx, SUBR, 1, args[0])?;
    let factor: crate::Moment = crate::arg(cx, SUBR, 2, args[1], "Moment")?;
    let key = cx.heap_mut().intern("duration");
    let d = music_property(cx.heap_mut(), args[0], key)
        .and_then(|v| cx.heap().foreign::<crate::Duration>(v).copied());
    if let Some(d) = d {
        let Some(compressed) = d.compressed(factor.main) else {
            return Err(crate::overflow(cx, SUBR));
        };
        let value = cx.heap_mut().wrap(compressed);
        set_property(cx.heap_mut(), args[0], key, value);
    }
    Ok(Value::UNSPECIFIED)
}

/// `(ly:music-list? x)`: a proper list of music.
pub(crate) fn is_music_list(cx: &mut Cx, args: &[Value]) -> R {
    let heap = cx.heap();
    if !crate::is_list(heap, args[0]) {
        return Ok(Value::FALSE);
    }
    let mut p = args[0];
    while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if heap.foreign::<Music>(item).is_none() {
            return Ok(Value::FALSE);
        }
        p = rest;
    }
    Ok(Value::TRUE)
}

// `prob-scheme.cc`'s functions: the engine's probs are music, stream
// events for `ly:prob-property` (a `Stream_event` is a `Prob`), which
// `Finger_glide_engraver` reads of a grob's cause (MS3 step 12), and
// [`Prob`]s, which paper systems are (MS3 step 33).

/// A `Prob` that is neither music nor a stream event, as
/// `make_paper_system` makes a paper system: its type, its immutable
/// property alist as it was given, and its mutable one, the newest first.
pub struct Prob {
    pub type_: Value,
    pub immutable: Value,
    pub props: Value,
}

impl Foreign for Prob {
    fn type_name(&self) -> &'static str {
        "Prob"
    }

    /// A copy for S5's undo log (MS6 step 3, D235): a parse's segment made
    /// it, and what changes it after (a translation) is undone with it.
    fn snapshot(&self) -> Option<alloc::boxed::Box<dyn Foreign>> {
        Some(alloc::boxed::Box::new(Prob {
            type_: self.type_,
            immutable: self.immutable,
            props: self.props,
        }))
    }

    /// `Prob::print_smob`: its type, its class, its mutable properties and
    /// then its immutable ones.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<Prob: ");
        heap.display(self.type_, out);
        out.push_str(" C++: Prob");
        heap.write(self.props, out);
        heap.write(self.immutable, out);
        out.push_str(" >\n");
    }

    /// `Prob::equal_p`: the immutable lists and then the mutable ones
    /// entry by entry, `origin` skipped, keys `eq?` and values `equal?`;
    /// the types are not compared.
    fn equal(&self, other: &dyn Foreign, heap: &Heap) -> bool {
        let Some(other) = (other as &dyn Any).downcast_ref::<Prob>() else {
            return false;
        };
        let origin = |key: Value| key.tag() == Tag::Symbol && heap.name(key) == Some("origin");
        let entries = |props| {
            At::Live(Live {
                kind: None,
                immutable: Value::NULL,
                props,
            })
            .entries(heap)
        };
        entries_equal(
            heap,
            entries(self.immutable),
            entries(other.immutable),
            &origin,
        ) && entries_equal(heap, entries(self.props), entries(other.props), &origin)
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.type_);
        tracer.trace(self.immutable);
        tracer.trace(self.props);
    }
}

/// `new Prob (type, immutable_init)`: a [`Prob`] of the type `type_`
/// whose immutable properties are `immutable`, with no mutable ones.
pub fn make_prob(heap: &mut Heap, type_: Value, immutable: Value) -> Value {
    heap.wrap(Prob {
        type_,
        immutable,
        props: Value::NULL,
    })
}

/// `get_property` of the [`Prob`] `prob`: its mutable value of `key`, else
/// its immutable one, else `'()`; `'()` for what is no [`Prob`].
pub fn prob_get(heap: &Heap, prob: Value, key: Value) -> Value {
    let Some(p) = heap.foreign::<Prob>(prob) else {
        return Value::NULL;
    };
    assq(heap, key, p.props)
        .or_else(|| assq(heap, key, p.immutable))
        .and_then(|entry| heap.cdr(entry))
        .unwrap_or(Value::NULL)
}

/// `set_property` of the [`Prob`] `prob`, `scm_assq_set_x` of its mutable
/// properties: `key`'s entry changed in place, or a new one first.
pub fn prob_set(heap: &mut Heap, prob: Value, key: Value, value: Value) {
    let Some(props) = heap.foreign::<Prob>(prob).map(|p| p.props) else {
        return;
    };
    if let Some(entry) = assq(heap, key, props) {
        heap.set_cdr(entry, value);
        return;
    }
    let entry = heap.cons(key, value);
    let props = heap.cons(entry, props);
    if let Some(p) = heap.foreign_mut::<Prob>(prob) {
        p.props = props;
    }
}

/// `(ly:make-prob type init key value …)` (`prob-scheme.cc:77–95`): a
/// [`Prob`] of the type `type` whose immutable properties are `init`, and
/// each `key` set to its `value`; a last key without one is ignored.
pub(crate) fn make_prob_p(cx: &mut Cx, args: &[Value]) -> R {
    let heap = cx.heap_mut();
    let prob = make_prob(heap, args[0], args[1]);
    for pair in args[2..].chunks_exact(2) {
        prob_set(heap, prob, pair[0], pair[1]);
    }
    Ok(prob)
}

/// `(ly:prob? x)`: music or a [`Prob`].
pub(crate) fn is_prob(cx: &mut Cx, args: &[Value]) -> R {
    let heap = cx.heap();
    Ok(Value::bool(
        heap.foreign::<Music>(args[0]).is_some() || heap.foreign::<Prob>(args[0]).is_some(),
    ))
}

fn check_prob(cx: &mut Cx, subr: &str, v: Value) -> Result<(), Thrown> {
    let heap = cx.heap();
    let ok = heap.foreign::<Music>(v).is_some() || heap.foreign::<Prob>(v).is_some();
    crate::check(cx, ok, subr, 1, v, "Prob")
}

/// `(ly:prob-property prob sym [val])`: `val`, or `'()`, for a property
/// that is `'()` or none.
pub(crate) fn prob_property(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:prob-property";
    let event = cx.heap().foreign::<StreamEvent>(args[0]).is_some();
    if !event {
        check_prob(cx, SUBR, args[0])?;
    }
    check_symbol(cx, SUBR, 2, args[1])?;
    let fallback = args.get(2).copied().unwrap_or(Value::NULL);
    let heap = cx.heap_mut();
    let found = match heap.foreign::<StreamEvent>(args[0]) {
        Some(e) => e.property(heap, args[1]),
        None if heap.foreign::<Prob>(args[0]).is_some() => Some(prob_get(heap, args[0], args[1])),
        None => music_property(heap, args[0], args[1]),
    };
    Ok(match found {
        None | Some(Value::NULL) => fallback,
        Some(v) => v,
    })
}

/// `(ly:prob-set-property! prob sym [value])`: an omitted value is
/// Guile's unbound marker, which the engine sets as unspecified.
pub(crate) fn prob_set_property(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:prob-set-property!";
    check_prob(cx, SUBR, args[0])?;
    check_symbol(cx, SUBR, 2, args[1])?;
    let value = args.get(2).copied().unwrap_or(Value::UNSPECIFIED);
    let heap = cx.heap_mut();
    if heap.foreign::<Prob>(args[0]).is_some() {
        prob_set(heap, args[0], args[1], value);
    } else {
        set_property(heap, args[0], args[1], value);
    }
    Ok(Value::UNSPECIFIED)
}

/// `(ly:prob-property? obj sym)`: whether the property is `#t`.
pub(crate) fn prob_property_p(cx: &mut Cx, args: &[Value]) -> R {
    let v = prob_property(cx, &[args[0], args[1], Value::FALSE])?;
    Ok(Value::bool(v == Value::TRUE))
}

pub(crate) fn prob_mutable_properties(cx: &mut Cx, args: &[Value]) -> R {
    check_prob(cx, "ly:prob-mutable-properties", args[0])?;
    if let Some(p) = cx.heap().foreign::<Prob>(args[0]) {
        return Ok(p.props);
    }
    Ok(mutable_properties(cx.heap_mut(), args[0]))
}

pub(crate) fn prob_immutable_properties(cx: &mut Cx, args: &[Value]) -> R {
    check_prob(cx, "ly:prob-immutable-properties", args[0])?;
    if let Some(p) = cx.heap().foreign::<Prob>(args[0]) {
        return Ok(p.immutable);
    }
    Ok(immutable(cx.heap(), args[0]).unwrap_or(Value::NULL))
}

/// `from_scm<bool> (get_property (music, key))`: whether the property is
/// `#t`, read without thawing.
pub(crate) fn is_true_property(heap: &Heap, music: Value, key: Value) -> bool {
    At::of(heap, music)
        .and_then(|at| at.property(heap, key))
        .and_then(|i| i.value())
        == Some(Value::TRUE)
}

/// `(%music-freeze! music)`: `music`, frozen in place if it can be, as the
/// parser freezes what a music function gives it; for probes and tests
/// that need frozen music without the parser.
pub(crate) fn music_freeze(cx: &mut Cx, args: &[Value]) -> R {
    check_music(cx, "%music-freeze!", 1, args[0])?;
    crate::freeze(cx.heap_mut(), args[0]);
    Ok(args[0])
}

/// `(%music-frozen? x)`: whether `x` is a handle of a frozen tree.
pub(crate) fn music_frozen(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(is_frozen(cx.heap(), args[0])))
}

/// `(ly:make-music-function signature procedure)`.
pub(crate) fn make_music_function(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:make-music-function";
    let ok = crate::is_list(cx.heap(), args[0]);
    crate::check(cx, ok, SUBR, 1, args[0], "list")?;
    let ok = cx.heap().is_procedure(args[1]);
    crate::check(cx, ok, SUBR, 2, args[1], "procedure")?;
    let call = cx
        .global("lily", "%music-function-call")
        .unwrap_or(Value::FALSE);
    Ok(cx.heap_mut().wrap(MusicFunction {
        signature: args[0],
        procedure: args[1],
        call,
    }))
}

pub(crate) fn is_music_function(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<MusicFunction>(cx, args[0])
}

fn function_arg(cx: &mut Cx, subr: &str, v: Value) -> Result<MusicFunction, Thrown> {
    crate::arg(cx, subr, 1, v, "Music_function")
}

pub(crate) fn music_function_signature(cx: &mut Cx, args: &[Value]) -> R {
    Ok(function_arg(cx, "ly:music-function-signature", args[0])?.signature)
}

pub(crate) fn music_function_extract(cx: &mut Cx, args: &[Value]) -> R {
    Ok(function_arg(cx, "ly:music-function-extract", args[0])?.procedure)
}
