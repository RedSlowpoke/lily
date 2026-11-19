//! The translation trace (04 Appendix A, MS2 steps 22–23), recorded
//! natively in `tools/compare/trace.ly`'s format, so that `trace.mjs`
//! compares a translation of the engine's with LilyPond's line by line.
//!
//! `trace.ly` adds a recording engraver to every context of a `\layout`
//! translation and a translator to the Score. The engine's recorder is the
//! arena's ([`crate::Arena`]), and the translator groups (`translator.rs`)
//! call it where those translators run:
//!
//! - `P` and `C` where the parent's translator group makes the new
//!   context's (`create_child_translator`, after its property operations
//!   and its translators' `initialize`): a pass begins with the Score, a
//!   child of Global;
//! - `E` from a listener ([`Handler::Record`]) on each context's events
//!   below, of `music-event`, `SetProperty`, `UnsetProperty`, `Override`
//!   and `Revert`, as the engraver's; an event is its deepest hearer's,
//!   and the lines of a timestep are written after the Score's
//!   `stop-translation-timestep`, in the order the events were first heard;
//! - `X` where a context's translators are finalized: in `check_removal`,
//!   and at `Finish` for the Score and what is left below it, children
//!   first.
//!
//! `S` lines are written after the `E` lines, where the Score's
//! `measurePosition` is zero, which `Timing_translator` sets: each
//! context's own properties of `trace.ly`'s list, down from the Score. A
//! value is written as `trace-value` writes it. Labels (`Name#n=id`) and passes count across the translations
//! of a compile, as `trace.ly`'s do across a run.
//!
//! A `\score` in a markup (`ly:score-embedded-format`, MS3 step 43) is
//! translated with a `\layout` copied from `$defaultlayout` before
//! `trace.ly`'s book handler gives its contexts the recording engraver:
//! only the Score's translator records it ([`Recorder::score_only`]), its
//! `S` lines alone, its labels counted on from the compile's.
//!
//! The grobs (MS3 step 12) are written as `tools/compare/grobs.ly` writes
//! them, into [`Trace::grobs`], from the translation's grob records (D108):
//! a line for each grob the Score's group acknowledges (`grob_heard`), in
//! that order, with the label of the context its translator is in and that
//! translator's class, taken when it is heard; then, at the Score's
//! finalize, the lines and the System's, which no engraver announces. The
//! columns are read there as `grobs-columns` reads them (MS3 step 15),
//! through the image's procedures on each record's handle: layout's
//! columns, which this crate does not name.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

use lily_engine_scheme::{Heap, Tag, Tracer, Value};

use crate::context::{Arena, ContextHandle, CtxId, GLOBAL, Host, Translation};
use crate::dispatcher::{Handler, Listener, StreamEvent};
use crate::music::music_name;
use crate::records::{Cause, Records};
use crate::scheme_translator;
use crate::{Input, Moment};

/// What a compile's translations have written, and the counts their labels
/// and passes go on from.
#[derive(Clone, Default)]
pub struct Trace {
    pub text: String,
    /// The grobs' lines, `grobs.ly`'s.
    pub grobs: String,
    passes: u32,
    counts: BTreeMap<String, u32>,
}

impl Trace {
    /// Its links moved to the text after an edit (N8's resume, D244).
    pub fn move_links(&mut self, moved: &crate::Moved) {
        self.text = moved.links(&self.text);
        self.grobs = moved.links(&self.grobs);
    }
}

/// What `trace-properties` leaves out of `all-translation-properties`:
/// those that hold grobs and the like.
const LEFT_OUT: &[&str] = &[
    "currentCommandColumn",
    "currentMusicalColumn",
    "currentBarLine",
    "busyGrobs",
    "stavesFound",
    "breakableSeparationItem",
    "hasStaffSpacing",
    "measureStartNow",
    "rootSystem",
    "lastChord",
];

/// A traced translation's recorder.
#[derive(Clone)]
pub struct Recorder {
    trace: Trace,
    labels: BTreeMap<CtxId, String>,
    /// The events of the timestep, in the order first heard, with their
    /// deepest hearer and its depth.
    heard: Vec<(Value, CtxId, u32)>,
    score: Option<CtxId>,
    /// The grobs the Score heard, in order: their record, type, cause,
    /// link, context and engraver, the columns left for the Score's
    /// finalize.
    grobs: Vec<(u32, [String; 5])>,
    /// `trace-properties`, sorted by name.
    properties: Vec<(String, Value)>,
    /// The Score's translator alone, no recording engraver: no `P`, `C`,
    /// `E` or `X` lines and no grobs.
    score_only: bool,
    /// Whether the Score's grobs are recorded: `grobs.ly` consists its
    /// `grobs-recorder` in the context definition named `Score` alone, so
    /// one of another name (a `VaticanaScore`, MS8 step 9) has none.
    grobs_recorded: bool,
}

impl Recorder {
    pub fn new(host: &mut dyn Host, trace: Trace) -> Recorder {
        let list = host
            .lily("all-translation-properties")
            .unwrap_or(Value::NULL);
        let heap = host.heap();
        let mut properties = Vec::new();
        let mut p = list;
        while let (Some(sym), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            let name = String::from(heap.name(sym).unwrap_or_default());
            if !LEFT_OUT.contains(&name.as_str()) {
                properties.push((name, sym));
            }
            p = rest;
        }
        properties.sort_by(|x, y| x.0.cmp(&y.0));
        Recorder {
            trace,
            labels: BTreeMap::new(),
            heard: Vec::new(),
            score: None,
            grobs: Vec::new(),
            properties,
            score_only: false,
            grobs_recorded: false,
        }
    }

    /// A recorder of the Score's translator alone, as `trace.ly` records an
    /// embedded score's translation.
    pub fn score_only(host: &mut dyn Host, trace: Trace) -> Recorder {
        Recorder {
            score_only: true,
            ..Recorder::new(host, trace)
        }
    }

    /// What it wrote, with the counts to go on from.
    pub fn into_trace(self) -> Trace {
        self.trace
    }

    /// Its links to places moved to the text after an edit (N8's resume,
    /// D244).
    pub(crate) fn move_links(&mut self, moved: &crate::Moved) {
        self.trace.move_links(moved);
        for (_, fields) in &mut self.grobs {
            for field in fields {
                *field = moved.links(field);
            }
        }
    }

    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        for &(ev, _, _) in &self.heard {
            tracer.trace(ev);
        }
        for &(_, sym) in &self.properties {
            tracer.trace(sym);
        }
    }

    fn line(&mut self, fields: &[&str]) {
        self.trace.text.push_str(&fields.join("\t"));
        self.trace.text.push('\n');
    }

    /// `trace-label`: `Name#n`, `=id` after it for an id, made the first
    /// time it is asked for.
    fn label(&mut self, heap: &Heap, a: &Arena, c: CtxId) -> String {
        if let Some(l) = self.labels.get(&c) {
            return l.clone();
        }
        let mut name = String::new();
        heap.display(a.name(heap, c), &mut name);
        let n = self.trace.counts.entry(name.clone()).or_default();
        let mut l = format!("{name}#{n}");
        *n += 1;
        if !a.id(c).is_empty() {
            l.push('=');
            l.push_str(a.id(c));
        }
        self.labels.insert(c, l.clone());
        l
    }
}

/// `trace-moment`: the main and grace parts, written as `display` writes
/// rationals.
fn moment(m: Moment) -> (String, String) {
    let (mut main, mut grace) = (String::new(), String::new());
    m.main.write(&mut main);
    m.grace.write(&mut grace);
    (main, grace)
}

/// Runs `f` on `a`'s recorder, if it has one.
fn with<T>(a: &mut Arena, f: impl FnOnce(&mut Recorder, &Arena) -> T) -> Option<T> {
    let mut r = a.recorder.take()?;
    let result = f(&mut r, a);
    a.recorder = Some(r);
    Some(result)
}

/// The context `c` is made: the recording engraver's `initialize`, and its
/// listeners added to `c`'s events below.
pub(crate) fn created(heap: &mut Heap, a: &mut Arena, c: CtxId) {
    let Some(score_only) = a.recorder.as_ref().map(|r| r.score_only) else {
        return;
    };
    if score_only {
        if a.parent(c) == Some(GLOBAL)
            && let Some(r) = a.recorder.as_mut()
        {
            r.score = Some(c);
        }
        return;
    }
    let below = a.events_below(c);
    for class in [
        "music-event",
        "SetProperty",
        "UnsetProperty",
        "Override",
        "Revert",
    ] {
        a.add_listener(heap, below, Listener::Context(c, Handler::Record), class);
    }
    let heap = &*heap;
    with(a, |r, a| {
        let parent = a.parent(c);
        let (main, grace) = moment(a.now());
        if parent == Some(GLOBAL) {
            r.trace.passes += 1;
            let pass = format!("{}", r.trace.passes);
            r.line(&["P", &pass, "layout"]);
            r.score = Some(c);
            let name = a.name(heap, c);
            r.grobs_recorded = heap.name(name) == Some("Score");
        }
        let label = r.label(heap, a, c);
        let parent = match parent {
            Some(p) => r.label(heap, a, p),
            None => "-".into(),
        };
        r.line(&["C", &main, &grace, &label, &parent]);
    });
}

/// `trace-heard!`: `ev` heard by `c`, which keeps it if it is deeper than
/// the context that heard it before.
pub(crate) fn heard(a: &mut Arena, c: CtxId, ev: Value) {
    let mut depth = 0;
    let mut at = Some(c);
    while let Some(x) = at {
        depth += 1;
        at = a.parent(x);
    }
    let Some(r) = a.recorder.as_mut() else {
        return;
    };
    match r.heard.iter_mut().find(|(e, _, _)| *e == ev) {
        Some(h) if depth > h.2 => *h = (ev, c, depth),
        Some(_) => {}
        None => r.heard.push((ev, c, depth)),
    }
}

/// The context `c` of the translation `t` is finalized: an `X` line; for
/// the Score, the grobs it heard and its System, with their columns
/// (`grobs-recorder`'s `finalize`).
pub(crate) fn finalized(host: &mut dyn Host, t: &Translation, c: CtxId) {
    let heard = match t.arena.borrow().recorder.as_ref() {
        Some(r) if r.score_only => return,
        Some(r) if r.score == Some(c) && r.grobs_recorded => {
            Some(r.grobs.iter().map(|g| g.0).collect::<Vec<_>>())
        }
        Some(_) => None,
        None => return,
    };
    // The columns are read with the arena free: layout's procedures borrow it.
    let columns = heard.map(|ids| {
        let root = host.heap().intern("rootSystem");
        let system = t.arena.borrow().get(c, root);
        let mut columns: Vec<String> = ids
            .into_iter()
            .map(|id| {
                let grob = scheme_translator::grob_handle(host.heap(), &t.arena, t.value, id);
                grob_columns(host, grob)
            })
            .collect();
        columns.push(grob_columns(host, system));
        columns
    });
    with(&mut t.arena.borrow_mut(), |r, a| {
        let label = r.labels.get(&c).cloned().unwrap_or_default();
        let (main, grace) = moment(a.now());
        r.line(&["X", &main, &grace, &label]);
        let Some(mut columns) = columns else {
            return;
        };
        let system = columns.pop().unwrap_or_default();
        for ((_, [name, cause, link, context, engraver]), columns) in
            core::mem::take(&mut r.grobs).into_iter().zip(columns)
        {
            grob_line(
                &mut r.trace,
                &[&name, &cause, &link, &columns, &context, &engraver],
            );
        }
        grob_line(
            &mut r.trace,
            &["System", "-", "-", &system, &label, "Score_engraver"],
        );
    });
}

/// The grobs the Score heard: each record and its type, cause, link,
/// context and engraver.
pub(crate) type Heard = Vec<(u32, [String; 5])>;

/// N8's splice (D243): the last compile's recorder as its run ended,
/// `end`, with what `new` had written in place of what it had by `then`
/// (its recorder at the checkpoint spliced at): the lines from there the
/// old run's. The grobs the Score heard by then are `new`'s, whose lines,
/// with their columns, [`spliced_grobs`] writes once the arena is spliced,
/// before the old run's that follow (given back).
pub(crate) fn splice(
    new: &Recorder,
    then: &Recorder,
    end: &Recorder,
) -> Option<(Recorder, Heard, String)> {
    if new.score_only != end.score_only || new.score != end.score {
        return None;
    }
    let text = end.trace.text.get(then.trace.text.len()..)?;
    // This translation's grob lines, the first the grobs heard by then.
    let mut lines = end.trace.grobs.get(then.trace.grobs.len()..)?;
    for _ in 0..then.grobs.len() {
        lines = &lines[lines.find('\n')? + 1..];
    }
    let mut out = end.clone();
    out.trace.text = new.trace.text.clone() + text;
    out.trace.grobs = new.trace.grobs.clone();
    out.grobs = Vec::new();
    Some((out, new.grobs.clone(), String::from(lines)))
}

/// After [`splice`]: the lines of the grobs the Score heard before the
/// splice, `heard`, with their columns as the spliced translation has
/// them, then the old run's lines after them, `after`.
pub(crate) fn spliced_grobs(host: &mut dyn Host, t: &Translation, heard: Heard, after: &str) {
    let columns: Vec<String> = heard
        .iter()
        .map(|&(id, _)| {
            let grob = scheme_translator::grob_handle(host.heap(), &t.arena, t.value, id);
            grob_columns(host, grob)
        })
        .collect();
    let mut a = t.arena.borrow_mut();
    let Some(r) = a.recorder.as_mut() else {
        return;
    };
    for ((_, [name, cause, link, context, engraver]), columns) in heard.into_iter().zip(columns) {
        grob_line(
            &mut r.trace,
            &[&name, &cause, &link, &columns, &context, &engraver],
        );
    }
    r.trace.grobs.push_str(after);
}

/// `grobs-columns`: a spanner's two bounds' column moments joined by `;`,
/// an item's own; `-` for a grob killed in the translation.
pub fn grob_columns(host: &mut dyn Host, grob: Value) -> String {
    if call(host, "grob::name", &[grob]) == Value::FALSE {
        return "-".into();
    }
    if call(host, "ly:spanner?", &[grob]) == Value::TRUE {
        let bound = |host: &mut dyn Host, d: i64| {
            let d = host.heap().integer(d);
            let item = call(host, "ly:spanner-bound", &[grob, d]);
            grob_moment(host, item)
        };
        let left = bound(host, -1);
        return format!("{left};{}", bound(host, 1));
    }
    grob_moment(host, grob)
}

/// `grobs-moment`: the `when` of an item's column, or `-`.
fn grob_moment(host: &mut dyn Host, item: Value) -> String {
    if call(host, "ly:item?", &[item]) != Value::TRUE {
        return "-".into();
    }
    let column = call(host, "ly:item-get-column", &[item]);
    if call(host, "ly:grob?", &[column]) != Value::TRUE {
        return "-".into();
    }
    let when = host.heap().intern("when");
    let when = call(host, "ly:grob-property", &[column, when, Value::FALSE]);
    match host.heap().foreign::<Moment>(when) {
        Some(&m) => {
            let (main, grace) = moment(m);
            format!("{main} {grace}")
        }
        None => "-".into(),
    }
}

/// `(name args…)` in `(lily)`, or `#f` if it is unbound or throws.
fn call(host: &mut dyn Host, name: &str, args: &[Value]) -> Value {
    host.lily(name)
        .and_then(|p| host.call(p, args).ok())
        .unwrap_or(Value::FALSE)
}

fn grob_line(trace: &mut Trace, fields: &[&str]) {
    trace.grobs.push_str(&fields.join("\t"));
    trace.grobs.push('\n');
}

/// The grob `grob` announced to `c`'s group, acknowledged there, its start
/// (`end` false) heard by `grobs-recorder` if `c` is the Score. A grob
/// killed before it is acknowledged has no interfaces, `grob-interface`
/// among them, and is not heard (MS3 step 42: a `SpanBarStub` killed as
/// it is made).
pub(crate) fn grob_heard(heap: &mut Heap, a: &mut Arena, c: CtxId, grob: u32, end: bool) {
    if end
        || a.recorder
            .as_ref()
            .is_none_or(|r| r.score_only || !r.grobs_recorded || r.score != Some(c))
    {
        return;
    }
    let Some(g) = a.records.grobs.get(grob as usize) else {
        return;
    };
    if g.interfaces.iter().next().is_none() {
        return;
    }
    let (origin, engraver) = (g.origin.0, a.translator_name(g.origin));
    let mut name = String::new();
    heap.display(g.name, &mut name);
    let cause = cause(heap, &a.records, g.cause);
    let link = link(heap, &a.records, g.cause);
    with(a, |r, a| {
        let context = r.label(heap, a, origin);
        r.grobs
            .push((grob, [name, cause, link, context, engraver.into()]));
    });
}

/// `grobs-cause`: an event as `<ev class>`, its first class; a grob as
/// `<Type>`, a dead one as `<#f>` (`grob::name` of a grob killed before
/// this one is heard, whose `meta` is gone, as Y19's ScriptColumn's
/// CaesuraScript, remade at a bar line); `-` for none.
fn cause(heap: &mut Heap, records: &Records, cause: Cause) -> String {
    match cause {
        Cause::Event(ev) => {
            let class = crate::dispatcher::event_property(heap, ev, "class");
            let mut out = String::from("<ev ");
            heap.display(heap.car(class).unwrap_or(Value::NULL), &mut out);
            out.push('>');
            out
        }
        Cause::Grob(n) => {
            let mut out = String::from("<");
            if let Some(g) = records.grobs.get(n as usize) {
                if g.interfaces.iter().next().is_none() {
                    out.push_str("#f");
                } else {
                    heap.display(g.name, &mut out);
                }
            }
            out.push('>');
            out
        }
        Cause::None | Cause::Audio(_) => "-".into(),
    }
}

/// `grobs-link`: the origin of the event reached through grob causes
/// (`Grob::origin`), `file:line:char:column` with the file's name only;
/// `-` if there is none.
fn link(heap: &mut Heap, records: &Records, mut cause: Cause) -> String {
    while let Cause::Grob(n) = cause {
        match records.grobs.get(n as usize) {
            Some(g) => cause = g.cause,
            None => return "-".into(),
        }
    }
    let Cause::Event(ev) = cause else {
        return "-".into();
    };
    let origin = crate::dispatcher::event_property(heap, ev, "origin");
    let Some(input) = heap.foreign::<Input>(origin) else {
        return "-".into();
    };
    let file = input.name.rsplit('/').next().unwrap_or_default();
    format!("{file}:{}:{}:{}", input.line, input.char, input.column)
}

/// After the Score's `stop-translation-timestep`: the timestep's events, by
/// the Score's moment (`trace-flush-events!`), then at a bar start the
/// properties (`trace-state!`).
pub fn one_time_step(heap: &mut Heap, a: &mut Arena) {
    let Some(mut r) = a.recorder.take() else {
        return;
    };
    if let Some(score) = r.score {
        let (main, grace) = moment(a.now());
        for (ev, c, _) in core::mem::take(&mut r.heard) {
            let label = r.label(heap, a, c);
            let (class, args) = event_args(heap, a, &mut r, ev);
            r.line(&["E", &main, &grace, &label, &class, &args]);
        }
        let position = heap.intern("measurePosition");
        let at_bar = heap
            .foreign::<Moment>(a.get(score, position))
            .is_some_and(|m| m.main.is_zero() && m.grace.is_zero());
        if !a.removed(score) && at_bar {
            states(heap, a, &mut r, score, &main, &grace);
        }
    }
    a.recorder = Some(r);
}

/// `trace-state!` of `c` and the contexts below it, parents first: each
/// property of the list that the context itself holds.
fn states(heap: &mut Heap, a: &Arena, r: &mut Recorder, c: CtxId, main: &str, grace: &str) {
    let label = r.label(heap, a, c);
    let class = heap.intern("class");
    for i in 0..r.properties.len() {
        let (name, sym) = r.properties[i].clone();
        let Some(v) = a.here(c, sym) else {
            continue;
        };
        let mut out = String::new();
        value(heap, a, r, class, v, &mut out);
        r.line(&["S", main, grace, &label, &name, &out]);
    }
    for &child in a.children(c) {
        states(heap, a, r, child, main, grace);
    }
}

/// An event's class, the first of its list, and `trace-event-args`.
fn event_args(heap: &mut Heap, a: &Arena, r: &mut Recorder, ev: Value) -> (String, String) {
    let class_key = heap.intern("class");
    let get = |heap: &Heap, key: Value| {
        heap.foreign::<StreamEvent>(ev)
            .and_then(|e| e.property(heap, key))
    };
    let class = get(heap, class_key)
        .and_then(|l| heap.car(l))
        .unwrap_or(Value::NULL);
    let mut name = String::new();
    heap.display(class, &mut name);
    let keys: &[&str] = match name.as_str() {
        "SetProperty" | "UnsetProperty" => &["symbol", "value", "once"],
        "Override" | "Revert" => &["symbol", "property-path", "value", "once"],
        _ => &[
            "pitch",
            "duration",
            "drum-type",
            "text",
            "span-direction",
            "articulation-type",
            "tremolo-type",
            "alteration",
            "figure",
        ],
    };
    let mut args = Vec::new();
    for key in keys {
        let key = heap.intern(key);
        let v = get(heap, key).unwrap_or(Value::NULL);
        let mut out = String::new();
        value(heap, a, r, class_key, v, &mut out);
        args.push(out);
    }
    (name, args.join(" "))
}

/// `trace-value`: as `write` writes it, but a procedure as `<p>`, a context
/// as its label, a stream event, music and an input location in short.
/// `class` is the symbol `class`.
fn value(heap: &Heap, a: &Arena, r: &mut Recorder, class: Value, v: Value, out: &mut String) {
    if heap.is_procedure(v) {
        out.push_str("<p>");
    } else if let Some(h) = heap.foreign::<ContextHandle>(v) {
        let label = r.label(heap, a, h.id);
        let _ = write!(out, "<{label}>");
    } else if let Some(e) = heap.foreign::<StreamEvent>(v) {
        let class = e
            .property(heap, class)
            .and_then(|l| heap.car(l))
            .unwrap_or(Value::NULL);
        out.push_str("<ev ");
        heap.display(class, out);
        out.push('>');
    } else if heap.foreign::<crate::Music>(v).is_some() {
        let _ = write!(out, "<m {}>", music_name(heap, v).unwrap_or_default());
    } else if heap.foreign::<Input>(v).is_some() {
        out.push_str("<loc>");
    } else if heap.foreign_object(v).map(|f| f.type_name()) == Some("hash-table") {
        out.push_str("<hash>");
    } else if v.tag() == Tag::Pair {
        out.push('(');
        let mut p = v;
        let mut first = true;
        while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            if !core::mem::take(&mut first) {
                out.push(' ');
            }
            value(heap, a, r, class, item, out);
            p = rest;
        }
        if p != Value::NULL {
            out.push_str(" . ");
            value(heap, a, r, class, p, out);
        }
        out.push(')');
    } else if let Some(items) = heap.vector_items(v) {
        out.push_str("#(");
        for (i, &item) in items.to_vec().iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            value(heap, a, r, class, item, out);
        }
        out.push(')');
    } else {
        let mut s = String::new();
        heap.write(v, &mut s);
        // Tabs and newlines would split a line.
        out.extend(
            s.chars()
                .map(|c| if matches!(c, '\t' | '\n') { ' ' } else { c }),
        );
    }
}
