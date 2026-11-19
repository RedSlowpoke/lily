//! LilyPond's `Context_def` (`context-def.cc`, `acceptance-set.cc`; N3's
//! definitions, 04 §7, D73): what a `\context { … }` block makes of its
//! mods, and what a context of its name is made from (part 3).
//!
//! A definition is immutable once it is a value. LilyPond clones one and
//! adds mods to the clone (the parser's `context_def_spec_body`,
//! `ly:context-def-modify`); here a new one is built from its base and the
//! mods ([`make_context_def`]), the same as far as anything but `eq?` can
//! tell. A definition derived by a context modification
//! ([`derive_context_def`], `ly:context-def-modify`, and part 3's
//! `\new … \with`) is cached by its base and the digest of the mods, so
//! that the same `\with` on twenty staves gives one definition.
//!
//! Its lists are LilyPond's Scheme lists, and its translators as LilyPond
//! names them ([`Translator`]): a C++ class by its symbol, which part 3's
//! framework makes in Rust, or a Scheme translator, a procedure or an
//! alist. Its Scheme face is
//! LilyPond's: `ly:context-def?`, `ly:context-def-lookup` and
//! `ly:context-def-modify` here, and the output definitions' lookups
//! (`output_def.rs`).

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Cx, Foreign, Heap, ImageReader, ImageWriter, Tag, Tracer, Value};

use crate::{Input, R};

/// A translator of a definition, as `\consists` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Translator {
    /// A C++ translator class, by its name: a symbol.
    Class(Value),
    /// A Scheme translator: a procedure, or an alist of its parts.
    Scheme(Value),
}

impl Translator {
    /// What `name`, an item of a definition's `consists`, names.
    pub fn of(name: Value) -> Translator {
        match name.tag() {
            Tag::Symbol => Translator::Class(name),
            _ => Translator::Scheme(name),
        }
    }
}

/// A derived definition in its base's cache: the digest and the list of
/// the mods, and the definition.
struct Derived {
    digest: u64,
    mods: Value,
    def: Value,
}

/// A context definition. Its lists are LilyPond's, in LilyPond's order:
/// the newest first, but `accepts`, whose default child comes first. A
/// list is never changed: a definition built from it makes a new one,
/// which may share its tail.
pub struct ContextDef {
    name: Value,
    aliases: Value,
    accepts: Value,
    /// The default child's name, or `'()`.
    default_child: Value,
    /// `(consists x)` and `(remove x)`, `x` a symbol for a string.
    translator_mods: Value,
    /// `(assign …)`, `(unset …)`, `(push …)`, `(pop …)` and `(apply …)`.
    property_ops: Value,
    description: Value,
    group_type: Value,
    grob_descriptions: Value,
    /// An [`Input`], or `#f` where LilyPond's `Input` has no source.
    origin: Value,
    /// The definitions derived from it ([`derive_context_def`]).
    derived: Vec<Derived>,
}

/// What a mod's tag is not: LilyPond's `unknown context mod tag`.
pub struct UnknownTag;

/// The items of `list`, as far as it is one.
fn items(heap: &Heap, list: Value) -> Vec<Value> {
    let mut items = Vec::new();
    let mut p = list;
    while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        items.push(item);
        p = rest;
    }
    items
}

/// `scm_delete_x` of a list it does not change: `list` without the items
/// `equal?` to `item`.
fn delete(heap: &mut Heap, list: Value, item: Value) -> Value {
    let mut kept = items(heap, list);
    kept.retain(|&x| !heap.equal(x, item));
    heap.list(&kept)
}

impl ContextDef {
    /// `Context_def ()`: no name (the empty symbol), nothing else.
    fn empty(heap: &mut Heap) -> ContextDef {
        ContextDef {
            name: heap.intern(""),
            aliases: Value::NULL,
            accepts: Value::NULL,
            default_child: Value::NULL,
            translator_mods: Value::NULL,
            property_ops: Value::NULL,
            description: Value::NULL,
            group_type: Value::NULL,
            grob_descriptions: Value::NULL,
            origin: Value::FALSE,
            derived: Vec::new(),
        }
    }

    /// The copy constructor: all but the derived ones.
    fn copy(&self) -> ContextDef {
        ContextDef {
            derived: Vec::new(),
            ..*self
        }
    }

    /// `add_context_mod`: `m`, a mod such as `(consists "Foo")`, applied.
    fn add_mod(&mut self, heap: &mut Heap, m: Value) -> Result<(), UnknownTag> {
        let tag = heap.car(m).and_then(|t| heap.name(t).map(String::from));
        let arg = heap.cdr(m).and_then(|r| heap.car(r));
        let (Some(tag), Some(arg)) = (tag, arg) else {
            return Err(UnknownTag);
        };
        match tag.as_str() {
            "description" => self.description = arg,
            "grob-descriptions" => self.grob_descriptions = arg,
            "pop" | "push" | "assign" | "unset" | "apply" => {
                self.property_ops = heap.cons(m, self.property_ops);
            }
            _ => {
                // The other mods take symbols.
                let sym = heap.intern_string(arg).unwrap_or(arg);
                match tag.as_str() {
                    // `accept_default`.
                    "default-child" => {
                        let rest = delete(heap, self.accepts, sym);
                        self.accepts = heap.cons(sym, rest);
                        self.default_child = sym;
                    }
                    "consists" | "remove" => {
                        let m = heap.list(&[heap.car(m).unwrap_or(m), sym]);
                        self.translator_mods = heap.cons(m, self.translator_mods);
                    }
                    // `accept`: after the default, which it may not be.
                    "accepts" if !heap.equal(sym, self.default_child) => {
                        self.accepts = match (heap.car(self.accepts), heap.cdr(self.accepts)) {
                            (Some(first), Some(rest)) if self.default_child != Value::NULL => {
                                let rest = heap.cons(sym, rest);
                                heap.cons(first, rest)
                            }
                            _ => heap.cons(sym, self.accepts),
                        };
                    }
                    "accepts" => {}
                    "denies" => {
                        self.accepts = delete(heap, self.accepts, sym);
                        if heap.equal(sym, self.default_child) {
                            self.default_child = Value::NULL;
                        }
                    }
                    "alias" => self.aliases = heap.cons(sym, self.aliases),
                    "translator-type" => self.group_type = sym,
                    "context-name" => self.name = sym,
                    _ => return Err(UnknownTag),
                }
            }
        }
        Ok(())
    }

    /// Its name, a symbol.
    pub fn name(&self) -> Value {
        self.name
    }

    /// Its aliases, the newest first.
    pub fn aliases(&self) -> Value {
        self.aliases
    }

    /// The names of the contexts it accepts, its default child first.
    pub fn accepts(&self) -> Value {
        self.accepts
    }

    /// Its default child's name, or `'()`.
    pub fn default_child(&self) -> Value {
        self.default_child
    }

    /// `(translator-type …)`'s symbol, or `'()`.
    pub fn group_type(&self) -> Value {
        self.group_type
    }

    /// `\grobdescriptions`' alist, or `'()`: Global's.
    pub fn grob_descriptions(&self) -> Value {
        self.grob_descriptions
    }

    /// Its property operations, the newest first.
    pub fn property_ops(&self) -> Value {
        self.property_ops
    }

    /// Where its block is, an [`Input`], or `#f`.
    pub fn origin(&self) -> Value {
        self.origin
    }

    /// `is_alias`: whether `sym` names it, as its name or an alias; `Bottom`
    /// names one with no default child.
    pub fn is_alias(&self, heap: &Heap, sym: Value) -> bool {
        if sym.tag() == Tag::Symbol && heap.name(sym) == Some("Bottom") {
            return self.default_child == Value::NULL;
        }
        sym == self.name || items(heap, self.aliases).contains(&sym)
    }

    /// `get_translator_names`: the names of its translators after
    /// `user_mods`, a list of `(consists x)` and `(remove x)`, each kept at
    /// its last `\consists` if no `\remove` follows; LilyPond's list, the
    /// last first. [`Translator::of`] tells what each names.
    pub fn translator_names(&self, heap: &mut Heap, user_mods: Value) -> Value {
        let mut mods = items(heap, self.translator_mods);
        mods.reverse();
        mods.extend(items(heap, user_mods));
        let arg = |heap: &mut Heap, m: Value| {
            let arg = heap.cdr(m).and_then(|r| heap.car(r)).unwrap_or(m);
            heap.intern_string(arg).unwrap_or(arg)
        };
        let mut args = Vec::new();
        for &m in &mods {
            args.push(arg(heap, m));
        }
        let mut names = Value::NULL;
        for (i, &m) in mods.iter().enumerate() {
            // The `added_index` of a translator is its last `\consists`,
            // unless a `\remove` of it follows: so no mod of it follows.
            let consists = heap.car(m).and_then(|t| heap.name(t)) == Some("consists");
            if consists && !args[i + 1..].contains(&args[i]) {
                names = heap.cons(args[i], names);
            }
        }
        names
    }

    /// `get_translator_names` of the definition `def` after `user_mods`;
    /// `'()` for no definition.
    pub(crate) fn names_of(heap: &mut Heap, def: Value, user_mods: Value) -> Value {
        match heap.foreign::<ContextDef>(def).map(ContextDef::copy) {
            Some(d) => d.translator_names(heap, user_mods),
            None => Value::NULL,
        }
    }

    /// `lookup`: the value of `key`, one of `ly:context-def-lookup`'s.
    fn lookup(&self, heap: &mut Heap, key: &str) -> Option<Value> {
        Some(match key {
            "default-child" => self.default_child,
            "consists" => self.translator_names(heap, Value::NULL),
            "description" => self.description,
            "aliases" => self.aliases,
            "accepts" => self.accepts,
            "property-ops" => self.property_ops,
            "context-name" => self.name,
            "group-type" => self.group_type,
            "grob-descriptions" => self.grob_descriptions,
            _ => return None,
        })
    }

    /// `to_alist`, as `ly:output-description` gives it: each consed in
    /// front, in `to_alist`'s order.
    fn to_alist(&self, heap: &mut Heap) -> Value {
        let mut alist = Value::NULL;
        let mut add = |heap: &mut Heap, key: &str, value: Value| {
            let key = heap.intern(key);
            let entry = heap.cons(key, value);
            alist = heap.cons(entry, alist);
        };
        let consists = self.translator_names(heap, Value::NULL);
        add(heap, "consists", consists);
        add(heap, "description", self.description);
        add(heap, "aliases", self.aliases);
        add(heap, "accepts", self.accepts);
        if self.default_child != Value::NULL {
            add(heap, "default-child", self.default_child);
        }
        add(heap, "property-ops", self.property_ops);
        add(heap, "context-name", self.name);
        if self.group_type.tag() == Tag::Symbol {
            add(heap, "group-type", self.group_type);
        }
        add(heap, "grob-descriptions", self.grob_descriptions);
        alist
    }

    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<ContextDef> {
        Some(ContextDef {
            name: r.value()?,
            aliases: r.value()?,
            accepts: r.value()?,
            default_child: r.value()?,
            translator_mods: r.value()?,
            property_ops: r.value()?,
            description: r.value()?,
            group_type: r.value()?,
            grob_descriptions: r.value()?,
            origin: r.value()?,
            derived: Vec::new(),
        })
    }

    /// Its fields, as [`ContextDef::load`] reads them.
    fn fields(&self) -> [Value; 10] {
        [
            self.name,
            self.aliases,
            self.accepts,
            self.default_child,
            self.translator_mods,
            self.property_ops,
            self.description,
            self.group_type,
            self.grob_descriptions,
            self.origin,
        ]
    }
}

/// A copy of the context definition `def`, which lists can be made of
/// while the heap is borrowed.
fn owned(heap: &Heap, def: Value) -> Option<ContextDef> {
    heap.foreign::<ContextDef>(def).map(ContextDef::copy)
}

/// `to_alist` of `def`, if it is a context definition.
pub(crate) fn alist(heap: &mut Heap, def: Value) -> Option<Value> {
    owned(heap, def).map(|d| d.to_alist(heap))
}

/// `location_string`: where `origin` is, an [`Input`], as `file:line:column`,
/// or ` (position unknown)`.
fn location_string(heap: &Heap, origin: Value, out: &mut String) {
    match heap.foreign::<Input>(origin) {
        Some(i) => out.push_str(&alloc::format!("{}:{}:{}", i.name, i.line, i.column + 1)),
        None => out.push_str(" (position unknown)"),
    }
}

impl Foreign for ContextDef {
    fn type_name(&self) -> &'static str {
        "Context_def"
    }

    /// `#<Context_def ` and its name, displayed, and its location.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<Context_def ");
        heap.display(self.name, out);
        out.push(' ');
        location_string(heap, self.origin, out);
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        for v in self.fields() {
            tracer.trace(v);
        }
        for d in &self.derived {
            tracer.trace(d.mods);
            tracer.trace(d.def);
        }
    }

    /// Its fields, without the derived ones, which are made again.
    fn save(&self, out: &mut ImageWriter) -> bool {
        for v in self.fields() {
            out.value(v);
        }
        true
    }
}

/// A context definition: `base`'s, or a new one's (`Context_def::make_scm`),
/// with the mods of `mods` added in order and `origin` (an [`Input`]) its
/// place, or `base`'s for `#f`. How many mods had a tag of none, which
/// LilyPond reports as `programming error: unknown context mod tag`.
pub fn make_context_def(
    heap: &mut Heap,
    base: Option<Value>,
    mods: &[Value],
    origin: Value,
) -> (Value, usize) {
    let mut def = match base.and_then(|b| heap.foreign::<ContextDef>(b)) {
        Some(b) => b.copy(),
        None => ContextDef::empty(heap),
    };
    let unknown = mods
        .iter()
        .filter(|&&m| def.add_mod(heap, m).is_err())
        .count();
    if origin != Value::FALSE {
        def.origin = origin;
    }
    (heap.wrap(def), unknown)
}

/// `ly:context-def-modify` of `base`, a context definition, by `mods`, a
/// list of mods: the definition derived, from `base`'s cache if it derived
/// one by mods `equal?` to these, and how many mods had a tag of none.
/// None if `base` is no context definition.
pub fn derive_context_def(heap: &mut Heap, base: Value, mods: Value) -> Option<(Value, usize)> {
    let digest = crate::frozen::digest(heap, mods);
    let def = heap.foreign::<ContextDef>(base)?;
    if let Some(d) = def
        .derived
        .iter()
        .find(|d| d.digest == digest && heap.equal(d.mods, mods))
    {
        return Some((d.def, 0));
    }
    let mods_items = items(heap, mods);
    let (derived, unknown) = make_context_def(heap, Some(base), &mods_items, Value::FALSE);
    if unknown == 0
        && let Some(def) = heap.foreign_mut::<ContextDef>(base)
    {
        def.derived.push(Derived {
            digest,
            mods,
            def: derived,
        });
    }
    Some((derived, unknown))
}

/// `programming_error ("unknown context mod tag")`, `count` times, kept
/// with the music crate's warnings (`warn.rs`).
fn unknown_tags(cx: &mut Cx, count: usize) {
    for _ in 0..count {
        crate::warn::diagnose(cx, "programming-error", "unknown context mod tag");
    }
}

/// `LY_ASSERT_SMOB (Context_def, …)`.
fn check(cx: &mut Cx, subr: &str, v: Value) -> Result<(), lily_engine_scheme::Thrown> {
    match cx.heap().foreign::<ContextDef>(v) {
        Some(_) => Ok(()),
        None => Err(cx.wrong_type(subr, 1, v, "Context_def")),
    }
}

/// `(ly:context-def? x)`.
pub(crate) fn is_context_def(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<ContextDef>(cx, args[0])
}

/// `(ly:context-def-lookup def sym [val])`: `sym`'s value in `def`, or
/// `val` or `'()` for `'()` or a key of none.
pub(crate) fn ly_context_def_lookup(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:context-def-lookup";
    check(cx, SUBR, args[0])?;
    if args[1].tag() != Tag::Symbol {
        return Err(cx.wrong_type(SUBR, 2, args[1], "symbol"));
    }
    let key = String::from(cx.heap().name(args[1]).unwrap_or_default());
    let heap = cx.heap_mut();
    let found = owned(heap, args[0])
        .and_then(|def| def.lookup(heap, &key))
        .unwrap_or(Value::NULL);
    match args.get(2) {
        Some(&val) if found == Value::NULL => Ok(val),
        _ => Ok(found),
    }
}

/// `(ly:context-def-modify def mod)`: `def` with the mods of the context
/// modification `mod` added.
pub(crate) fn ly_context_def_modify(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:context-def-modify";
    check(cx, SUBR, args[0])?;
    let Some(mods) = crate::context_mod::context_mods(cx.heap_mut(), args[1]) else {
        return Err(cx.wrong_type(SUBR, 2, args[1], "Context_mod"));
    };
    let (def, unknown) =
        derive_context_def(cx.heap_mut(), args[0], mods).unwrap_or((Value::UNSPECIFIED, 0));
    unknown_tags(cx, unknown);
    Ok(def)
}
