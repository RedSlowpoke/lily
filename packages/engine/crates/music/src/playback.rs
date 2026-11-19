//! The playback map (D26, 04 §9.3, D79): where every note of a `\midi`
//! performance is on the page, written by built-in performers that record
//! what `packages/common/runtime/timing.ly`'s Scheme performers record, so
//! that no Scheme runs per event (MS2 step 26).
//!
//! A `\midi` translation run by [`crate::run_performance`] gives the
//! performer groups of its Score, Staff and Voice contexts one of these
//! classes each, where `timing.ly`'s `\consists` puts its performers: by
//! the name of the context's definition, last in its `consists`.
//!
//! - **`Lily_playback_performer`** (Score) hears every rhythmic event below
//!   it: its link (`file:line:char:column`, as the SVG backend writes it
//!   for the grob the event caused), the moment's main and grace parts, and
//!   its duration's length, 0 for a syllable. At the end of each timestep it
//!   records a bar start where `currentBarNumber` changed, `measurePosition`
//!   before now. Its `finalize` writes the performance's JSON.
//! - **`Lily_playback_staff_performer`** (Staff) records the staff when it
//!   is made, with its group (the parent context, numbered as met, unless
//!   it is the Score), and its name, short name, clef and clef
//!   transposition at its first note.
//! - **`Lily_playback_voice_performer`** (Voice) adds the voice's id to its
//!   staff at its first note.
//!
//! A file's name that is not absolute is made so from the current
//! directory, which in the engine is the root of the host's names, `/`
//! (`ly-getcwd` is empty, D87).

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Exception, Heap, Tag, Value};

use crate::context::{Arena, CtxId, Dir};
use crate::context_def::ContextDef;
use crate::dispatcher::event_property;
use crate::duration::Duration;
use crate::input::Input;
use crate::rational::Rational;
use crate::translator::{FINALIZE, INITIALIZE, Kind, STOP, TranslatorClass, Tx, Work};

type Run<T> = Result<T, Exception>;

/// What a performance's performers record, kept in its translation.
#[derive(Clone, Default)]
pub struct Playback {
    events: Vec<String>,
    bars: Vec<(f64, i64)>,
    last_bar: Option<i64>,
    staves: Vec<Staff>,
    /// The staff groups met so far, in order.
    groups: Vec<CtxId>,
    /// The performance's JSON, once the Score is finalized.
    pub json: Option<String>,
}

impl Playback {
    /// Its links moved to the text after an edit (N8's resume, D244).
    pub(crate) fn move_links(&mut self, moved: &crate::Moved) {
        for event in &mut self.events {
            *event = moved.links(event);
        }
        if let Some(json) = &mut self.json {
            *json = moved.links(json);
        }
    }
}

/// What a staff says about the part its MIDI track plays (D48).
#[derive(Clone)]
pub(crate) struct Staff {
    context: CtxId,
    id: String,
    group: (String, i64),
    name: String,
    short_name: String,
    clef: Option<String>,
    transposition: i64,
    voices: Vec<String>,
}

const fn performer(
    name: &'static str,
    methods: u8,
    always: bool,
    hears: &'static [&'static str],
    make: fn() -> Box<dyn Work>,
) -> TranslatorClass {
    TranslatorClass {
        name,
        kind: Kind::Performer,
        methods,
        always,
        hears,
        acknowledges: &[],
        must_be_last: false,
        make: Some(make),
        reads: &[],
        writes: &[],
    }
}

static SCORE: TranslatorClass = performer(
    "Lily_playback_performer",
    INITIALIZE | STOP | FINALIZE,
    true,
    &["rhythmic-event"],
    || Box::new(ScorePerformer),
);
static STAFF: TranslatorClass = performer(
    "Lily_playback_staff_performer",
    INITIALIZE,
    false,
    &["note-event"],
    || Box::new(StaffPerformer { known: false }),
);
static VOICE: TranslatorClass = performer(
    "Lily_playback_voice_performer",
    0,
    false,
    &["note-event"],
    || Box::new(VoicePerformer { known: false }),
);

/// The playback map's class for a context of the definition `def`, in a
/// translation that writes one.
pub(crate) fn class_for(heap: &Heap, a: &Arena, def: Value) -> Option<&'static TranslatorClass> {
    a.playback.as_ref()?;
    let name = heap.foreign::<ContextDef>(def)?.name();
    match heap.name(name)? {
        "Score" => Some(&SCORE),
        "Staff" => Some(&STAFF),
        "Voice" => Some(&VOICE),
        _ => None,
    }
}

/// The entry of the staff `c` written by the translator call running: its
/// log keeps it as it is now (MS6 step 7c).
fn logged(tx: &Tx<'_>, c: CtxId) {
    let a = tx.t.arena.borrow();
    let staff = a
        .log
        .as_ref()
        .and(a.playback.as_ref())
        .and_then(|p| p.staves.iter().find(|s| s.context == c));
    if let Some(s) = staff {
        a.effect(crate::replay::Effect::Playback(c, Box::new(s.clone())));
    }
}

/// N8's replay (MS6 step 7c): the old run's entry `staff` of the context
/// now `c`, written again, with the group it numbered.
pub(crate) fn replayed(a: &mut Arena, c: CtxId, mut staff: Staff) {
    let parent = a.parent(c);
    staff.context = c;
    let Some(p) = a.playback.as_mut() else {
        return;
    };
    if let (Some(parent), true) = (parent, staff.group.1 >= 0)
        && !p.groups.contains(&parent)
    {
        p.groups.push(parent);
    }
    match p.staves.iter_mut().find(|s| s.context == c) {
        Some(s) => *s = staff.clone(),
        None => p.staves.push(staff.clone()),
    }
    a.effect(crate::replay::Effect::Playback(c, Box::new(staff)));
}

/// Runs `f` on the translation's playback map.
fn with<T>(tx: &Tx<'_>, f: impl FnOnce(&mut Playback) -> T) -> Option<T> {
    tx.t.arena.borrow_mut().playback.as_mut().map(f)
}

#[derive(Clone)]
struct ScorePerformer;

impl Work for ScorePerformer {
    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        let heap = tx.heap();
        let origin = event_property(heap, ev, "origin");
        let Some(href) = heap.foreign::<Input>(origin).map(href) else {
            return Ok(());
        };
        let duration = event_property(heap, ev, "duration");
        let length = heap
            .foreign::<Duration>(duration)
            .and_then(|d| d.length())
            .unwrap_or(Rational::ZERO);
        let classes = event_property(heap, ev, "class");
        let lyric = heap.intern("lyric-event");
        let syllable = list_has(heap, classes, lyric);
        let now = tx.now();
        let event = format!(
            "{{\"href\":{},\"at\":{},\"grace\":{},\"length\":{}}}",
            string(&href),
            number(now.main.to_f64()),
            number(now.grace.to_f64()),
            number(if syllable { 0.0 } else { length.to_f64() })
        );
        with(tx, |p| p.events.push(event));
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let number = tx.get("currentBarNumber");
        let position = tx.get("measurePosition");
        let (Some(number), Some(&position)) = (
            number.as_fixnum(),
            tx.heap().foreign::<crate::rational::Moment>(position),
        ) else {
            return Ok(());
        };
        let now = tx.now();
        with(tx, |p| {
            if p.last_bar != Some(number) {
                p.last_bar = Some(number);
                // The bar began measurePosition ago, whether or not anything
                // happened then.
                p.bars
                    .push((now.main.to_f64() - position.main.to_f64(), number));
            }
        });
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        with(tx, |p| p.json = Some(p.to_json()));
        Ok(())
    }
}

impl Playback {
    /// The performance's JSON as `timing.ly` writes it when another
    /// performance follows it in its file (OPEN C2.3): each staff's voices
    /// none, as a staff's thunk reads its voices from the table the next
    /// performance's `Lily_timing_performer` cleared when it began.
    pub(crate) fn followed_json(&self) -> String {
        let mut followed = self.clone();
        for staff in &mut followed.staves {
            staff.voices.clear();
        }
        followed.to_json()
    }

    /// The performance's JSON.
    fn to_json(&self) -> String {
        let bars: Vec<String> = self
            .bars
            .iter()
            .map(|&(at, n)| format!("{{\"at\":{},\"number\":{n}}}", number(at)))
            .collect();
        let staves: Vec<String> = self.staves.iter().map(Staff::json).collect();
        format!(
            "{{\"events\":[{}],\"bars\":[{}],\"staves\":[{}]}}",
            self.events.join(","),
            bars.join(","),
            staves.join(",")
        )
    }

    /// N8's splice (D243): the last compile's map as its run ended, `end`,
    /// with what `new` had written in place of what it had by `then` (its
    /// map at the checkpoint spliced at); the JSON made again if it was.
    pub(crate) fn splice(new: &Playback, then: &Playback, end: &Playback) -> Option<Playback> {
        let mut out = end.clone();
        out.events = new.events.clone();
        out.events
            .extend_from_slice(end.events.get(then.events.len()..)?);
        out.bars = new.bars.clone();
        out.bars.extend_from_slice(end.bars.get(then.bars.len()..)?);
        if out.json.is_some() {
            out.json = Some(out.to_json());
        }
        Some(out)
    }
}

impl Staff {
    fn json(&self) -> String {
        let voices: Vec<String> = self.voices.iter().map(|v| string(v)).collect();
        format!(
            "{{\"id\":{},\"name\":{},\"shortName\":{},\"group\":{},\"groupIndex\":{},\
             \"clef\":{},\"clefTransposition\":{},\"voices\":[{}]}}",
            string(&self.id),
            string(&self.name),
            string(&self.short_name),
            string(&self.group.0),
            self.group.1,
            string(self.clef.as_deref().unwrap_or("")),
            self.transposition,
            voices.join(",")
        )
    }
}

#[derive(Clone)]
struct StaffPerformer {
    known: bool,
}

impl Work for StaffPerformer {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.flags(&[self.known]);
    }

    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let c = tx.context;
        let (parent, id) = {
            let a = tx.t.arena.borrow();
            (a.parent(c), String::from(a.id(c)))
        };
        let parent = parent.filter(|&p| {
            let name = tx.t.arena.borrow().name(tx.host.heap(), p);
            tx.host.heap().name(name) != Some("Score")
        });
        let group = match parent {
            None => (String::new(), -1),
            Some(p) => {
                let name = tx.t.arena.borrow().name(tx.host.heap(), p);
                let mut text = String::new();
                tx.host.heap().display(name, &mut text);
                let index = with(tx, |pb| match pb.groups.iter().position(|&g| g == p) {
                    Some(i) => i,
                    None => {
                        pb.groups.push(p);
                        pb.groups.len() - 1
                    }
                });
                (text, index.unwrap_or(0) as i64)
            }
        };
        with(tx, |p| {
            p.staves.push(Staff {
                context: c,
                id,
                group,
                name: String::new(),
                short_name: String::new(),
                clef: None,
                transposition: 0,
                voices: Vec::new(),
            });
        });
        logged(tx, c);
        Ok(())
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, _: Value) -> Run<()> {
        if core::mem::replace(&mut self.known, true) {
            return Ok(());
        }
        // No \clef yet means the treble clef, which the MIDI Staff does not
        // set.
        let glyph = tx.get("clefGlyph");
        let shift = tx.get("clefTransposition");
        let long = tx.get("instrumentName");
        let short = tx.get("shortInstrumentName");
        let clef = tx
            .heap()
            .str(glyph)
            .map_or(String::from("clefs.G"), |s| String::from(s.as_str()));
        let shift = shift.as_fixnum();
        let long = text(tx, long)?;
        let short = text(tx, short)?;
        let c = tx.context;
        with(tx, |p| {
            if let Some(s) = p.staves.iter_mut().find(|s| s.context == c) {
                s.clef = Some(clef);
                if let Some(shift) = shift {
                    s.transposition = shift;
                }
                // "bright acoustic" is the MIDI Staff's own default, not the
                // score's.
                if long != "bright acoustic" {
                    s.name = long;
                }
                s.short_name = short;
            }
        });
        logged(tx, c);
        Ok(())
    }
}

#[derive(Clone)]
struct VoicePerformer {
    known: bool,
}

impl Work for VoicePerformer {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.flags(&[self.known]);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, _: Value) -> Run<()> {
        if core::mem::replace(&mut self.known, true) {
            return Ok(());
        }
        let c = tx.context;
        let staff = tx.heap().intern("Staff");
        let (staff, id) = {
            let a = tx.t.arena.borrow();
            let found = a.find_context(tx.host.heap(), c, Dir::Up, staff, "");
            (found, String::from(a.id(c)))
        };
        if let Some(staff) = staff {
            with(tx, |p| {
                if let Some(s) = p.staves.iter_mut().find(|s| s.context == staff) {
                    s.voices.push(id);
                }
            });
            logged(tx, staff);
        }
        Ok(())
    }
}

/// `lily-timing-text`: a string, a markup as `markup->string` gives it, or
/// nothing.
fn text(tx: &mut Tx<'_>, v: Value) -> Run<String> {
    let v = match v.tag() {
        Tag::Pair => tx.call("markup->string", &[v])?,
        _ => v,
    };
    Ok(tx
        .heap()
        .str(v)
        .map_or(String::new(), |s| String::from(s.as_str())))
}

fn list_has(heap: &Heap, list: Value, x: Value) -> bool {
    let mut p = list;
    while let (Some(item), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if item == x {
            return true;
        }
        p = rest;
    }
    false
}

/// `lily-timing-href`: the link the SVG backend writes for a grob `origin`
/// caused, `textedit://file:line:char:column` with the column from 1.
fn href(origin: &Input) -> String {
    let mut file = origin.name.replace('\\', "/");
    if !file.starts_with('/') {
        file.insert(0, '/');
    }
    let file = lily_engine_scheme::percent_encode(&file);
    format!(
        "textedit://{file}:{}:{}:{}",
        origin.line,
        origin.char,
        origin.column + 1
    )
}

/// `lily-timing-json-string`.
fn string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 32 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `lily-timing-json-number`: an integer as one, any other number as the
/// shortest that reads back the same.
fn number(x: f64) -> String {
    if -1e15 < x && x < 1e15 && x == (x as i64) as f64 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}
