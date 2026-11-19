//! Frozen music (04 §6.1): a tree of nodes in preorder, so that a subtree is
//! a contiguous range, with each node's properties in a side table and a
//! digest of its contents. A [`Tree`] is a foreign object of the Scheme
//! heap that [`crate::Music`]'s handles hold; nothing changes it once it
//! is made. [`freeze`] makes one of live music, and [`thaw`] gives a handle
//! the live music it stands for.
//!
//! A property whose value is music, or a list of music, is held as the
//! node's children; any other value is held as it is in the side table, as
//! 04 §6.1's `extra` holds the rare ones, `origin` among them as an
//! [`Input`] ([`Tree::origin`]). 04 §6.1's payloads by kind are not here
//! (D67). Lengths and starts are kept in a side array of two moments per
//! node, filled as `length.rs` computes them, and not saved.
//!
//! Digests (04 §6.3) are of a node's contents without its `origin`, so that
//! music that only moved keeps them. [`first_difference`] finds where two
//! trees differ, and [`Tree::identity`] names a node by the digests on its
//! path, for the caches of later stages.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use lily_engine_scheme::{Foreign, Heap, ImageReader, ImageWriter, Tag, Tracer, Value};

use crate::Input;
use crate::Moment;
use crate::music::{Live, Music, State};

/// One music object of a tree.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Node {
    /// Its type: an index in [`crate::types::TYPES`].
    pub kind: u8,
    /// Its properties: `props_len` from `props` in [`Tree::props`].
    props_len: u16,
    /// Nodes in its subtree, itself included: the next sibling is at its
    /// index plus `size`.
    pub size: u32,
    props: u32,
    /// Of its kind, its properties and its children's digests, without its
    /// `origin` (04 §6.3).
    pub digest: u64,
}

/// How a node holds a property.
#[derive(Clone, Copy, Debug)]
pub enum Prop {
    /// A value that is no music and no list of music.
    Value(Value),
    /// Music: the next child.
    Music,
    /// A list of music: the next `n` children.
    Musics(u32),
}

/// A frozen tree: its nodes in preorder, their properties, and the
/// immutable properties of each kind it holds, as music had them when it
/// was frozen.
pub struct Tree {
    nodes: Vec<Node>,
    props: Vec<(Value, Prop)>,
    immutable: Vec<(u8, Value)>,
    /// Each node's length and start once computed natively: empty until
    /// the first is.
    times: RefCell<Vec<[Option<Moment>; 2]>>,
}

/// Which of a node's times: [`Tree::time`].
#[derive(Clone, Copy)]
pub(crate) enum Time {
    Length = 0,
    Start = 1,
}

/// A frozen node's property: a value, or the children that hold it.
pub(crate) enum Found {
    Value(Value),
    Music(u32),
    Musics(Vec<u32>),
}

impl Tree {
    fn new(nodes: Vec<Node>, props: Vec<(Value, Prop)>, immutable: Vec<(u8, Value)>) -> Tree {
        Tree {
            nodes,
            props,
            immutable,
            times: RefCell::new(Vec::new()),
        }
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The children of node `i`, in the order of the properties that hold
    /// them.
    pub fn children(&self, i: u32) -> impl Iterator<Item = u32> + '_ {
        let end = i + self.nodes[i as usize].size;
        let mut child = i + 1;
        core::iter::from_fn(move || {
            (child < end).then(|| {
                let c = child;
                child += self.nodes[c as usize].size;
                c
            })
        })
    }

    /// The span of node `i`'s `origin`: its file, its first byte and the
    /// byte after it. None if it has none, or one that is no location.
    pub fn origin(&self, heap: &Heap, i: u32) -> Option<(u16, u32, u32)> {
        self.props(i).iter().find_map(|&(key, prop)| match prop {
            Prop::Value(v) if heap.name(key) == Some("origin") => {
                heap.foreign::<Input>(v).map(|o| (o.file, o.start, o.end))
            }
            _ => None,
        })
    }

    /// Node `i`'s identity (04 §6.3): a hash of the digests on the path
    /// from the root to it, each with its ordinal among the siblings before
    /// it that have the same digest. It is the same in another tree where
    /// the music on the path has the same contents, wherever it moved.
    pub fn identity(&self, i: u32) -> u64 {
        let mut h = mix(BASIS, self.nodes[0].digest);
        let mut at = 0;
        while at != i {
            let Some(child) = self
                .children(at)
                .find(|&c| c <= i && i < c + self.nodes[c as usize].size)
            else {
                break;
            };
            let digest = self.nodes[child as usize].digest;
            let ordinal = self
                .children(at)
                .take_while(|&c| c != child)
                .filter(|&c| self.nodes[c as usize].digest == digest)
                .count();
            h = mix(mix(h, digest), ordinal as u64);
            at = child;
        }
        h
    }

    /// Node `i`'s length or start if it was computed.
    pub(crate) fn time(&self, i: u32, which: Time) -> Option<Moment> {
        self.times.borrow().get(i as usize)?[which as usize]
    }

    pub(crate) fn set_time(&self, i: u32, which: Time, m: Moment) {
        let mut times = self.times.borrow_mut();
        if times.is_empty() {
            times.resize(self.nodes.len(), [None; 2]);
        }
        times[i as usize][which as usize] = Some(m);
    }

    /// The properties of node `i`, in the order of its live property list.
    pub fn props(&self, i: u32) -> &[(Value, Prop)] {
        let node = &self.nodes[i as usize];
        let start = node.props as usize;
        &self.props[start..start + usize::from(node.props_len)]
    }

    /// The immutable property list of `kind`.
    pub(crate) fn immutable(&self, kind: u8) -> Value {
        self.immutable
            .iter()
            .find(|&&(k, _)| k == kind)
            .map_or(Value::NULL, |&(_, alist)| alist)
    }

    /// Node `i`'s properties with the children that hold them.
    pub(crate) fn found(&self, i: u32) -> Vec<(Value, Found)> {
        let mut child = i + 1;
        let mut take = || {
            let c = child;
            child += self.nodes[c as usize].size;
            c
        };
        self.props(i)
            .iter()
            .map(|&(key, prop)| {
                let found = match prop {
                    Prop::Value(v) => Found::Value(v),
                    Prop::Music => Found::Music(take()),
                    Prop::Musics(n) => Found::Musics((0..n).map(|_| take()).collect()),
                };
                (key, found)
            })
            .collect()
    }

    /// Node `i`'s property `key` if it holds no music and no list: what can
    /// be read without thawing. `Err` when it is there but must be thawed.
    pub(crate) fn immediate(&self, i: u32, key: Value) -> Result<Option<Value>, ()> {
        match self.props(i).iter().find(|&&(k, _)| k == key) {
            Some(&(_, Prop::Value(v))) if v.tag() != Tag::Pair => Ok(Some(v)),
            Some(_) => Err(()),
            None => Ok(None),
        }
    }

    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<Tree> {
        let n = r.count()?;
        let nodes = (0..n)
            .map(|_| {
                Some(Node {
                    kind: r.u8()?,
                    props_len: u16::try_from(r.u32()?).ok()?,
                    size: r.u32()?,
                    props: r.u32()?,
                    digest: r.u64()?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let n = r.count()?;
        let props = (0..n)
            .map(|_| {
                let key = r.value()?;
                let prop = match r.u8()? {
                    0 => Prop::Value(r.value()?),
                    1 => Prop::Music,
                    2 => Prop::Musics(r.u32()?),
                    _ => return None,
                };
                Some((key, prop))
            })
            .collect::<Option<Vec<_>>>()?;
        let n = r.count()?;
        let immutable = (0..n)
            .map(|_| Some((r.u8()?, r.value()?)))
            .collect::<Option<Vec<_>>>()?;
        Some(Tree::new(nodes, props, immutable))
    }

    /// Writes node `i`'s properties as `write` writes a property list,
    /// its music as [`Foreign::print`] prints music.
    pub(crate) fn print_props(&self, heap: &Heap, i: u32, out: &mut String) {
        out.push('(');
        for (n, (key, found)) in self.found(i).into_iter().enumerate() {
            if n > 0 {
                out.push(' ');
            }
            out.push('(');
            heap.write(key, out);
            match found {
                Found::Value(Value::NULL) => {}
                Found::Value(v) if v.tag() == Tag::Pair => {
                    // `(key . (a b))` is written `(key a b)`.
                    let mut list = String::new();
                    heap.write(v, &mut list);
                    out.push(' ');
                    out.push_str(&list[1..list.len() - 1]);
                }
                Found::Value(v) => {
                    out.push_str(" . ");
                    heap.write(v, out);
                }
                Found::Music(c) => {
                    out.push_str(" . ");
                    self.print_node(heap, c, out);
                }
                Found::Musics(children) => {
                    for c in children {
                        out.push(' ');
                        self.print_node(heap, c, out);
                    }
                }
            }
            out.push(')');
        }
        out.push(')');
    }

    pub(crate) fn print_node(&self, heap: &Heap, i: u32, out: &mut String) {
        out.push_str(crate::music::PRINT_HEAD);
        self.print_props(heap, i, out);
        heap.write(self.immutable(self.nodes[i as usize].kind), out);
        out.push_str(crate::music::PRINT_TAIL);
    }
}

impl Foreign for Tree {
    fn type_name(&self) -> &'static str {
        "Music_tree"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        for &(key, prop) in &self.props {
            tracer.trace(key);
            if let Prop::Value(v) = prop {
                tracer.trace(v);
            }
        }
        for &(_, alist) in &self.immutable {
            tracer.trace(alist);
        }
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.count(self.nodes.len());
        for node in &self.nodes {
            out.u8(node.kind);
            out.u32(node.props_len.into());
            out.u32(node.size);
            out.u32(node.props);
            out.u64(node.digest);
        }
        out.count(self.props.len());
        for &(key, prop) in &self.props {
            out.value(key);
            match prop {
                Prop::Value(v) => {
                    out.u8(0);
                    out.value(v);
                }
                Prop::Music => out.u8(1),
                Prop::Musics(n) => {
                    out.u8(2);
                    out.u32(n);
                }
            }
        }
        out.count(self.immutable.len());
        for &(kind, alist) in &self.immutable {
            out.u8(kind);
            out.value(alist);
        }
        true
    }
}

// --- freezing ----------------------------------------------------------------------

/// Freezes the live music `music` in place: it becomes a handle of a new
/// tree of what it holds, music it holds that is frozen already copied
/// into it. False, and nothing changed, for music that is frozen already
/// and for music that cannot be frozen: of a type that is none of the 131
/// (`ly:make-music` of a list of its own), holding itself, two kinds' music
/// with other immutable properties than the tree has for them, or a
/// property list that is no list of pairs.
pub fn freeze(heap: &mut Heap, music: Value) -> bool {
    if !matches!(
        heap.foreign::<Music>(music).map(|m| &m.0),
        Some(State::Live(_))
    ) {
        return false;
    }
    let mut b = Builder {
        nodes: Vec::new(),
        props: Vec::new(),
        immutable: Vec::new(),
        ancestors: Vec::new(),
    };
    if b.add(heap, music).is_none() {
        return false;
    }
    let tree = heap.wrap(Tree::new(b.nodes, b.props, b.immutable));
    if let Some(m) = heap.foreign_mut::<Music>(music) {
        m.0 = State::Frozen { tree, node: 0 };
    }
    true
}

struct Builder {
    nodes: Vec<Node>,
    props: Vec<(Value, Prop)>,
    immutable: Vec<(u8, Value)>,
    ancestors: Vec<Value>,
}

impl Builder {
    /// The immutable properties of `kind` as this tree has them, or None if
    /// it has others.
    fn immutable(&mut self, kind: u8, alist: Value) -> Option<()> {
        match self.immutable.iter().find(|&&(k, _)| k == kind) {
            Some(&(_, held)) => (held == alist).then_some(()),
            None => {
                self.immutable.push((kind, alist));
                Some(())
            }
        }
    }

    /// Adds `music` and its subtree in preorder.
    fn add(&mut self, heap: &Heap, music: Value) -> Option<()> {
        match heap.foreign::<Music>(music)?.0 {
            State::Frozen { tree, node } => self.copy(heap, tree, node),
            State::Live(live) => self.add_live(heap, music, live),
        }
    }

    /// Copies the subtree at `node` of the frozen `tree`.
    fn copy(&mut self, heap: &Heap, tree: Value, node: u32) -> Option<()> {
        let tree = heap.foreign::<Tree>(tree)?;
        let size = tree.nodes[node as usize].size;
        for i in node..node + size {
            let n = tree.nodes[i as usize];
            self.immutable(n.kind, tree.immutable(n.kind))?;
            let props = u32::try_from(self.props.len()).ok()?;
            self.props.extend_from_slice(tree.props(i));
            self.nodes.push(Node { props, ..n });
        }
        Some(())
    }

    fn add_live(&mut self, heap: &Heap, music: Value, live: Live) -> Option<()> {
        let kind = live.kind?;
        if self.ancestors.contains(&music) {
            return None;
        }
        self.immutable(kind, live.immutable)?;
        let at = self.nodes.len();
        let start = self.props.len();
        let mut children = Vec::new();
        if !crate::is_list(heap, live.props) {
            return None;
        }
        let mut p = live.props;
        while p != Value::NULL {
            let (entry, rest) = (heap.car(p)?, heap.cdr(p)?);
            let (key, value) = (heap.car(entry)?, heap.cdr(entry)?);
            let prop = if heap.foreign::<Music>(value).is_some() {
                children.push(value);
                Prop::Music
            } else if let Some(items) = music_list(heap, value) {
                let n = u32::try_from(items.len()).ok()?;
                children.extend(items);
                Prop::Musics(n)
            } else {
                Prop::Value(value)
            };
            self.props.push((key, prop));
            p = rest;
        }
        self.nodes.push(Node {
            kind,
            props_len: u16::try_from(self.props.len() - start).ok()?,
            size: 0,
            props: u32::try_from(start).ok()?,
            digest: 0,
        });
        self.ancestors.push(music);
        for child in children {
            self.add(heap, child)?;
        }
        self.ancestors.pop();
        let size = u32::try_from(self.nodes.len() - at).ok()?;
        let digest = self.digest(heap, at);
        let node = &mut self.nodes[at];
        node.size = size;
        node.digest = digest;
        Some(())
    }

    /// The digest of node `at`, whose children are in already.
    fn digest(&self, heap: &Heap, at: usize) -> u64 {
        let node = self.nodes[at];
        let start = node.props as usize;
        let props = &self.props[start..start + usize::from(node.props_len)];
        let mut h = own_digest(heap, node.kind, props);
        let mut child = at + 1;
        while child < self.nodes.len() {
            h = mix(h, self.nodes[child].digest);
            child += self.nodes[child].size as usize;
        }
        h
    }
}

/// The digest of a node's kind and properties, `origin` left out.
fn own_digest(heap: &Heap, kind: u8, props: &[(Value, Prop)]) -> u64 {
    let mut h = mix(BASIS, kind.into());
    for &(key, prop) in props {
        let name = heap.name(key).unwrap_or_default();
        if name == "origin" {
            continue;
        }
        h = mix_str(h, name);
        h = match prop {
            Prop::Value(v) => {
                let mut budget = DIGEST_BUDGET;
                mix(mix(h, 1), digest_value(heap, v, &mut budget))
            }
            Prop::Music => mix(h, 2),
            Prop::Musics(n) => mix(mix(h, 3), n.into()),
        };
    }
    h
}

/// The items of `value` if it is a list of music, not empty; a circular
/// list is none.
fn music_list(heap: &Heap, value: Value) -> Option<Vec<Value>> {
    if !crate::is_list(heap, value) {
        return None;
    }
    let mut items = Vec::new();
    let mut p = value;
    while p != Value::NULL {
        let item = heap.car(p)?;
        heap.foreign::<Music>(item)?;
        items.push(item);
        p = heap.cdr(p)?;
    }
    (!items.is_empty()).then_some(items)
}

/// How many pairs, vectors and items a property's digest looks at, so
/// that a circular value ends.
const DIGEST_BUDGET: u32 = 4096;

/// [`digest_value`] of `v`, as far as [`DIGEST_BUDGET`] goes.
pub(crate) fn digest(heap: &Heap, v: Value) -> u64 {
    digest_value(heap, v, &mut DIGEST_BUDGET.clone())
}

/// A digest of a value: lists and vectors by their items, music by its
/// type's name, anything else as `write` writes it.
fn digest_value(heap: &Heap, v: Value, budget: &mut u32) -> u64 {
    if *budget == 0 {
        return 7;
    }
    *budget -= 1;
    match v.tag() {
        Tag::Pair => {
            let (car, cdr) = (heap.car(v).unwrap_or(v), heap.cdr(v).unwrap_or(v));
            let h = mix(2, digest_value(heap, car, budget));
            mix(h, digest_value(heap, cdr, budget))
        }
        Tag::Vector => heap.vector_items(v).map_or(3, |items| {
            items.iter().fold(mix(3, items.len() as u64), |h, &x| {
                mix(h, digest_value(heap, x, budget))
            })
        }),
        _ if heap.foreign::<Music>(v).is_some() => {
            mix_str(4, crate::music::music_name(heap, v).unwrap_or_default())
        }
        // A place in the input, which an edit before it moves (N8).
        _ if heap.foreign::<Input>(v).is_some() => 8,
        // An unpure-pure container by its procedures (N8).
        _ if heap.foreign::<crate::unpure::UnpurePure>(v).is_some() => {
            let parts = heap
                .foreign::<crate::unpure::UnpurePure>(v)
                .map(|u| u.parts());
            parts
                .unwrap_or_default()
                .iter()
                .fold(7, |h, &p| mix(h, digest_value(heap, p, budget)))
        }
        // A closure by its code and its free values, not the number it is
        // printed with (N8, MS6 step 4).
        Tag::Closure if heap.closure_parts(v).is_some() => {
            let (code, free) = heap.closure_parts(v).unwrap_or_default();
            free.iter()
                .fold(mix(6, code), |h, &x| mix(h, digest_value(heap, x, budget)))
        }
        _ => {
            let mut text = String::new();
            heap.write(v, &mut text);
            // An object that prints nothing of itself (a `\score` in a
            // markup) is the one the parse made: another parse's differs.
            let h = mix_str(5, &text);
            if opaque(&text) {
                mix(h, v.to_bits())
            } else {
                h
            }
        }
    }
}

/// Whether `text` is what `write` writes of an object that shows nothing of
/// its contents, `#<Score>`.
pub(crate) fn opaque(text: &str) -> bool {
    text.starts_with("#<") && !text.contains(' ')
}

const BASIS: u64 = 0xcbf2_9ce4_8422_2325;

fn mix(h: u64, x: u64) -> u64 {
    (h ^ x).wrapping_mul(0x0100_0000_01b3)
}

fn mix_str(h: u64, text: &str) -> u64 {
    text.bytes()
        .fold(mix(h, text.len() as u64), |h, b| mix(h, b.into()))
}

/// Where `new` first differs from `old` (04 §6.3, §10.1), in preorder: the
/// first pair of nodes, walked in parallel from the roots past children
/// whose digests are equal, whose kinds, own properties or numbers of
/// children differ, or the deepest pair on the path to the first children
/// that do. None if the roots' digests are equal.
pub fn first_difference(heap: &Heap, old: &Tree, new: &Tree) -> Option<(u32, u32)> {
    let (mut a, mut b) = (0, 0);
    if old.nodes.first()?.digest == new.nodes.first()?.digest {
        return None;
    }
    loop {
        let xs: Vec<u32> = old.children(a).collect();
        let ys: Vec<u32> = new.children(b).collect();
        if old.nodes[a as usize].kind != new.nodes[b as usize].kind
            || xs.len() != ys.len()
            || old.own_digest(heap, a) != new.own_digest(heap, b)
        {
            return Some((a, b));
        }
        let differs = xs
            .iter()
            .zip(&ys)
            .find(|&(&c, &d)| old.nodes[c as usize].digest != new.nodes[d as usize].digest);
        match differs {
            Some((&c, &d)) => (a, b) = (c, d),
            None => return Some((a, b)),
        }
    }
}

impl Tree {
    /// The digest of node `i`'s kind and properties without its children's.
    fn own_digest(&self, heap: &Heap, i: u32) -> u64 {
        own_digest(heap, self.nodes[i as usize].kind, self.props(i))
    }
}

// --- thawing -----------------------------------------------------------------------

/// Thaws the handle `music` in place, if it is frozen: it becomes live
/// music with the properties of its node, values that are lists copied,
/// and its music new frozen handles of its children. So it is a copy of
/// the subtree that no other handle shares, as LilyPond's music copied
/// at a reference is.
pub fn thaw(heap: &mut Heap, music: Value) {
    let Some(&State::Frozen { tree, node }) = heap.foreign::<Music>(music).map(|m| &m.0) else {
        return;
    };
    let Some(t) = heap.foreign::<Tree>(tree) else {
        return;
    };
    let kind = t.nodes[node as usize].kind;
    let immutable = t.immutable(kind);
    let found = t.found(node);
    let mut entries = Vec::with_capacity(found.len());
    for (key, found) in found {
        let handle = |heap: &mut Heap, node| heap.wrap(Music(State::Frozen { tree, node }));
        let value = match found {
            Found::Value(v) => crate::music::deep_copy(heap, v),
            Found::Music(c) => handle(heap, c),
            Found::Musics(children) => {
                let items: Vec<Value> = children.into_iter().map(|c| handle(heap, c)).collect();
                heap.list(&items)
            }
        };
        entries.push(heap.cons(key, value));
    }
    let props = heap.list(&entries);
    if let Some(m) = heap.foreign_mut::<Music>(music) {
        m.0 = State::Live(Live {
            kind: Some(kind),
            immutable,
            props,
        });
    }
}
