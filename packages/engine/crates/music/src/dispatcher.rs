//! LilyPond's `Dispatcher` (`dispatcher.cc`) and `Stream_event`
//! (`stream-event.cc`), N3's instances (04 §7–§8, D73).
//!
//! A dispatcher keeps, per event class, its listeners in the order of
//! their priorities, and the dispatchers it hears with the priority each
//! gave it. Classes are numbers and a dispatcher's are a [`ClassSet`]
//! (`event_class.rs`), so an event's listeners are those of the classes
//! its own set has. [`Dispatchers::targets`] gives them merged by priority
//! as `Dispatcher::dispatch` calls them, a priority heard once, and the
//! contexts call each in turn (`context.rs`), a dispatcher's own dispatch
//! when it is reached, as LilyPond's does.
//!
//! A stream event is LilyPond's: its properties, `class` among them,
//! `ly:make-event-class`'s list. Its Scheme face is `ly:stream-event?`,
//! `ly:make-stream-event`, `ly:event-property` and
//! `ly:event-set-property!`. An event an iterator makes of music
//! (`Music::to_event`, 04 §8) refers to it: its own properties are only
//! what LilyPond's adds to the music's (`class`, `length`, the
//! articulations as events, `music-cause`) and what is set on it later,
//! and the music's mutable properties are read behind them, never copied
//! (MS2 step 23).

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Cx, Foreign, Heap, Tag, Tracer, Value};

use crate::R;
use crate::event_class::ClassSet;
use crate::music::{At, Item};

/// A dispatcher's number in its translation.
pub type DispId = u32;

/// What a context does with an event it hears (`context.cc`,
/// `global-context.cc`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handler {
    CreateContext,
    RemoveContext,
    ChangeParent,
    SetProperty,
    UnsetProperty,
    AcknowledgeInfant,
    Prepare,
    /// `Engraver_group::override` and `revert`, which a context whose
    /// group type is `Engraver_group` listens with (MS2 step 24).
    Override,
    Revert,
    /// The translation trace's recorder (`trace.rs`), which `trace.ly`'s
    /// engraver is in LilyPond.
    Record,
    /// `Translator_group::create_child_translator` (`translator.rs`).
    CreateChildTranslator,
    /// A Score group's `prepare`, `one_time_step` and `finish`, which hear
    /// Global's `Prepare`, `OneTimeStep` and `Finish`.
    TimestepStart,
    OneTimeStep,
    Finish,
}

/// A listener: another dispatcher's dispatch, a context's handler, a
/// translator's (its context, its place in the group and the index of the
/// class's listener), a lyric combine iterator's (its number among the
/// arena's, `lyrics.rs`), or a Scheme procedure (`ly:add-listener`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listener {
    Dispatch(DispId),
    Context(u32, Handler),
    Translator(u32, u16, u8),
    Combine(u32, crate::lyrics::Heard),
    Scheme(Value),
}

/// One dispatcher.
#[derive(Clone, Default)]
pub struct Dispatcher {
    /// Per class, its listeners by increasing priority.
    listeners: Vec<(u8, Vec<(i32, Listener)>)>,
    /// The classes with listeners (`listen_classes_`).
    listened: ClassSet,
    /// The dispatchers it hears, with the priority each gave it.
    sources: Vec<(DispId, i32)>,
    priority_count: i32,
}

/// The dispatchers of a translation.
#[derive(Clone, Default)]
pub struct Dispatchers(Vec<Dispatcher>);

impl Dispatchers {
    pub fn make(&mut self) -> DispId {
        self.0.push(Dispatcher::default());
        (self.0.len() - 1) as DispId
    }

    /// `add_listener`: `listener` for `class`, after those added before.
    pub fn add_listener(&mut self, d: DispId, listener: Listener, class: u8) {
        let dispatcher = &mut self.0[d as usize];
        dispatcher.priority_count += 1;
        let priority = dispatcher.priority_count;
        self.internal_add_listener(d, listener, class, priority);
    }

    /// `internal_add_listener`: the first listener of a class makes `d`
    /// listen for it in the dispatchers it hears, at the priority each gave.
    fn internal_add_listener(&mut self, d: DispId, listener: Listener, class: u8, priority: i32) {
        if !self.0[d as usize].listened.contains(class) {
            for (source, p) in self.0[d as usize].sources.clone() {
                self.internal_add_listener(source, Listener::Dispatch(d), class, p);
            }
            self.0[d as usize].listened.insert(class);
        }
        let dispatcher = &mut self.0[d as usize];
        let at = match dispatcher.listeners.iter().position(|(c, _)| *c == class) {
            Some(at) => at,
            None => {
                dispatcher.listeners.push((class, Vec::new()));
                dispatcher.listeners.len() - 1
            }
        };
        let list = &mut dispatcher.listeners[at].1;
        // `scm_merge`: after those of the same priority.
        let place = list.partition_point(|(p, _)| *p <= priority);
        list.insert(place, (priority, listener));
    }

    /// `remove_listener`: the first `listener` of `class`; false if there
    /// is none, LilyPond's `Attempting to remove nonexisting listener.`
    pub fn remove_listener(&mut self, d: DispId, listener: Listener, class: u8) -> bool {
        let dispatcher = &mut self.0[d as usize];
        let Some(at) = dispatcher.listeners.iter().position(|(c, _)| *c == class) else {
            return false;
        };
        let list = &mut dispatcher.listeners[at].1;
        let Some(i) = list.iter().position(|(_, l)| *l == listener) else {
            return false;
        };
        list.remove(i);
        if list.is_empty() {
            dispatcher.listeners.remove(at);
            dispatcher.listened.0[usize::from(class >> 6)] &= !(1 << (class & 63));
            for (source, _) in dispatcher.sources.clone() {
                self.remove_listener(source, Listener::Dispatch(d), class);
            }
        }
        true
    }

    /// `register_as_listener`: `d` hears what `source` dispatches of the
    /// classes `d` listens to; false if it already did.
    pub fn register_as_listener(&mut self, d: DispId, source: DispId) -> bool {
        let s = &mut self.0[source as usize];
        s.priority_count += 1;
        let priority = s.priority_count;
        if self.0[d as usize].sources.iter().any(|&(x, _)| x == source) {
            return false;
        }
        self.0[d as usize].sources.insert(0, (source, priority));
        for class in self.0[d as usize].listened.iter().collect::<Vec<_>>() {
            self.internal_add_listener(source, Listener::Dispatch(d), class, priority);
        }
        true
    }

    /// `unregister_as_listener`.
    pub fn unregister_as_listener(&mut self, d: DispId, source: DispId) {
        self.0[d as usize].sources.retain(|&(x, _)| x != source);
        for class in self.0[d as usize].listened.iter().collect::<Vec<_>>() {
            self.remove_listener(source, Listener::Dispatch(d), class);
        }
    }

    /// The classes `d` has listeners for.
    pub fn listened(&self, d: DispId) -> ClassSet {
        self.0[d as usize].listened
    }

    /// What `dispatch` calls for an event whose class is `set`, in order:
    /// the listeners of its classes merged by priority, each priority once.
    pub fn targets(&self, d: DispId, set: ClassSet) -> Vec<Listener> {
        let mut all: Vec<(i32, Listener)> = Vec::new();
        for (class, list) in &self.0[d as usize].listeners {
            if set.contains(*class) {
                all.extend_from_slice(list);
            }
        }
        // Stable, so equal priorities keep their class's order.
        all.sort_by_key(|&(p, _)| p);
        all.dedup_by_key(|&mut (p, _)| p);
        all.into_iter().map(|(_, l)| l).collect()
    }

    /// The Scheme values its listeners hold.
    pub fn trace(&self, tracer: &mut Tracer<'_>) {
        for d in &self.0 {
            for (_, list) in &d.listeners {
                for (_, l) in list {
                    if let Listener::Scheme(v) = l {
                        tracer.trace(*v);
                    }
                }
            }
        }
    }
}

/// A stream event: its properties, the newest first, `class` among them,
/// and the music it was made of, `#f` for none, whose mutable properties
/// come after its own.
pub struct StreamEvent {
    pub props: Value,
    pub music: Value,
}

impl StreamEvent {
    /// The value of `key`, or `None`. Music a frozen node holds is in the
    /// event's own properties ([`music_event`]).
    pub fn property(&self, heap: &Heap, key: Value) -> Option<Value> {
        let mut p = self.props;
        while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            if heap.car(entry) == Some(key) {
                return heap.cdr(entry);
            }
            p = rest;
        }
        match At::of(heap, self.music)?.mutable(heap, key)? {
            Item::Value(v) => Some(v),
            _ => None,
        }
    }
}

impl Foreign for StreamEvent {
    fn type_name(&self) -> &'static str {
        "Stream_event"
    }

    /// `Prob::equal_p`, as `listener->once-listener` compares a repeated
    /// event (MS2 step 26).
    fn equal(&self, other: &dyn Foreign, heap: &Heap) -> bool {
        let Some(other) = (other as &dyn core::any::Any).downcast_ref::<StreamEvent>() else {
            return false;
        };
        crate::music::event_equal(heap, (self.props, self.music), (other.props, other.music))
    }

    /// `Prob::print_smob`: `Stream_event`, its type and class, then its
    /// mutable properties and its immutable ones, ` >` and a newline (OPEN
    /// C6). LilyPond's event holds `class` and what it was made with as
    /// immutable, and what is set on it after as mutable, the newest first:
    /// an event of music has the music's properties as immutable and its
    /// own, `class` but, as mutable; another, those before `class`.
    fn print(&self, heap: &Heap, out: &mut String) {
        out.push_str("#<Prob: Stream_event C++: Stream_event");
        let mut own = Vec::new();
        let mut p = self.props;
        while let (Some(e), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            if let (Some(k), Some(v)) = (heap.car(e), heap.cdr(e)) {
                own.push((k, v));
            }
            p = rest;
        }
        let class = own.iter().position(|&(k, _)| heap.name(k) == Some("class"));
        let at = At::of(heap, self.music);
        let (mutable, mut immutable): (Entries, Entries) = match (class, &at) {
            (Some(c), Some(_)) => (
                own.iter()
                    .enumerate()
                    .filter(|&(i, _)| i != c)
                    .map(|(_, &e)| e)
                    .collect(),
                alloc::vec![own[c]],
            ),
            (Some(c), None) => (own[..c].to_vec(), own[c..].to_vec()),
            (None, _) => (own, Vec::new()),
        };
        if let Some(at) = at {
            for (key, item) in at.entries(heap) {
                if let Item::Value(v) = item {
                    immutable.push((key, v));
                }
            }
        }
        for list in [mutable, immutable] {
            out.push('(');
            for (i, &(key, value)) in list.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                write_entry(heap, key, value, out);
            }
            out.push(')');
        }
        out.push_str(crate::music::PRINT_TAIL);
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.props);
        tracer.trace(self.music);
    }
}

/// An alist's entries.
type Entries = Vec<(Value, Value)>;

/// An alist's entry `(key . value)` as `write` prints it.
fn write_entry(heap: &Heap, key: Value, value: Value, out: &mut String) {
    out.push('(');
    heap.write(key, out);
    match value.tag() {
        Tag::Null => {}
        Tag::Pair => {
            let mut list = String::new();
            heap.write(value, &mut list);
            match list.strip_prefix('(').and_then(|l| l.strip_suffix(')')) {
                Some(inner) => {
                    out.push(' ');
                    out.push_str(inner);
                }
                None => {
                    out.push_str(" . ");
                    out.push_str(&list);
                }
            }
        }
        _ => {
            out.push_str(" . ");
            heap.write(value, out);
        }
    }
    out.push(')');
}

/// A stream event of the class `class` (a list) with `props` (an alist).
pub fn make_event(heap: &mut Heap, class: Value, props: Value) -> Value {
    let key = heap.intern("class");
    let entry = heap.cons(key, class);
    let props = heap.cons(entry, props);
    heap.wrap(StreamEvent {
        props,
        music: Value::FALSE,
    })
}

/// An event of the class `class` made of `music`, with `props` its own.
pub fn music_event(heap: &mut Heap, class: Value, props: Value, music: Value) -> Value {
    let key = heap.intern("class");
    let entry = heap.cons(key, class);
    let props = heap.cons(entry, props);
    heap.wrap(StreamEvent { props, music })
}

/// The value of `key` (a name) in the stream event `ev`, or `'()`.
pub fn event_property(heap: &mut Heap, ev: Value, key: &str) -> Value {
    let key = heap.intern(key);
    heap.foreign::<StreamEvent>(ev)
        .and_then(|e| e.property(heap, key))
        .unwrap_or(Value::NULL)
}

/// `Prob::set_property`: `key` set in place, or added in front.
pub fn set_event_property(heap: &mut Heap, ev: Value, key: Value, value: Value) -> bool {
    let Some(props) = heap.foreign::<StreamEvent>(ev).map(|e| e.props) else {
        return false;
    };
    let mut p = props;
    while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if heap.car(entry) == Some(key) {
            return heap.set_cdr(entry, value);
        }
        p = rest;
    }
    let entry = heap.cons(key, value);
    let props = heap.cons(entry, props);
    if let Some(e) = heap.foreign_mut::<StreamEvent>(ev) {
        e.props = props;
    }
    true
}

/// `ly_deep_copy`: pairs and vectors copied, anything else shared.
pub fn ly_deep_copy(heap: &mut Heap, v: Value) -> Value {
    if let (Some(car), Some(cdr)) = (heap.car(v), heap.cdr(v)) {
        let car = ly_deep_copy(heap, car);
        let cdr = ly_deep_copy(heap, cdr);
        return heap.cons(car, cdr);
    }
    if let Some(items) = heap.vector_items(v) {
        let items = items.to_vec();
        let items = items.into_iter().map(|i| ly_deep_copy(heap, i)).collect();
        return heap.vector(items);
    }
    v
}

/// `Stream_event::clone`: a new event of the same music, its own
/// properties copied (`copy_mutable_properties`).
pub fn clone_event(heap: &mut Heap, ev: Value) -> Option<Value> {
    let (props, music) = heap
        .foreign::<StreamEvent>(ev)
        .map(|e| (e.props, e.music))?;
    let props = ly_deep_copy(heap, props);
    Some(heap.wrap(StreamEvent { props, music }))
}

/// `(ly:stream-event? x)`.
pub(crate) fn is_stream_event(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<StreamEvent>(cx, args[0])
}

/// `(ly:make-stream-event class [props])`.
pub(crate) fn ly_make_stream_event(cx: &mut Cx, args: &[Value]) -> R {
    let props = args.get(1).copied().unwrap_or(Value::NULL);
    if !matches!(props.tag(), Tag::Pair | Tag::Null) {
        return Err(cx.wrong_type("ly:make-stream-event", 2, props, "list"));
    }
    Ok(make_event(cx.heap_mut(), args[0], props))
}

/// `(ly:event-property ev sym [default])`: `default`, or `'()`, for a
/// property that is `'()` or none, as `ly_prob_property` gives it (a
/// `\set`'s `once` is `'()`, which part-combiner.scm reads with `#f`).
pub(crate) fn ly_event_property(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:event-property";
    if cx.heap().foreign::<StreamEvent>(args[0]).is_none() {
        return Err(cx.wrong_type(SUBR, 1, args[0], "Stream_event"));
    }
    if args[1].tag() != Tag::Symbol {
        return Err(cx.wrong_type(SUBR, 2, args[1], "symbol"));
    }
    let heap = cx.heap();
    let found = heap
        .foreign::<StreamEvent>(args[0])
        .and_then(|e| e.property(heap, args[1]));
    Ok(match (found, args.get(2)) {
        (Some(v), _) if v != Value::NULL => v,
        (_, Some(&d)) => d,
        (_, None) => Value::NULL,
    })
}

/// `(ly:event-set-property! ev sym val)`.
pub(crate) fn ly_event_set_property(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:event-set-property!";
    if cx.heap().foreign::<StreamEvent>(args[0]).is_none() {
        return Err(cx.wrong_type(SUBR, 1, args[0], "Stream_event"));
    }
    if args[1].tag() != Tag::Symbol {
        return Err(cx.wrong_type(SUBR, 2, args[1], "symbol"));
    }
    set_event_property(cx.heap_mut(), args[0], args[1], args[2]);
    Ok(Value::UNSPECIFIED)
}
