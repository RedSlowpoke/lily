//! LilyPond's `Performance` (`performance.cc`, `performance-scheme.cc`)
//! and `Score_performer`'s own work (`score-performer.cc`; 07 §7's M2, MS5
//! step 1): what a `\midi` translation leaves, and its MIDI file.
//!
//! - **While it runs**, the translation's Score keeps a [`State`] in its
//!   arena: the performance's Scheme object, which `Score_performer::
//!   initialize` sets as the Score's `output`; the audio columns, a moment
//!   each, the timestep's one of which every item announced to the Score
//!   is given (`acknowledge_audio_elements`); the staves in the order they
//!   were announced (`audio_staffs_`); `midiSkipOffset`'s skipping; and
//!   `Staff_performer`'s statics, the channels a performance's staves share
//!   (`performers.rs`), kept here so that each performance has its own.
//! - **When it ends** ([`finished`], `ly:format-output`'s `process`), each
//!   staff's items are moved into the object with their columns' moments
//!   and channels, the dynamic spans their notes point to copied in with
//!   them, and the performance's start found: the earliest column of any
//!   item.
//! - **`ly:performance-write`** writes it through `crates/smf` (`output`,
//!   `Audio_staff::output`), the control track's first text the
//!   performance's name or left out, each staff a track, `midiChannelMapping
//!   = #'voice`'s ports before each; the file is kept in the object
//!   ([`take_written`]), where the core takes it as an output, and the
//!   `\midi`'s `after-writing` is called as LilyPond calls it.
//!   `ly:performance-headers` gives the headers `Book::process_score`
//!   pushed (layout's `paper_book.rs`), which `write-performances-midis`
//!   names the performance by.

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Cx, Foreign, Heap, Tracer, Value};

use crate::R;
use crate::audio::{Audio, AudioSpanDynamic, TRACK_NAME, moment_to_real, moment_to_ticks};
use crate::context::{Arena, CtxId, Host, Translation};
use crate::midi_item::{Midi, get_midi};
use crate::rational::{Moment, Rational};

/// What `Score_performer` and `Staff_performer` keep while a performance
/// runs.
#[derive(Clone)]
pub(crate) struct State {
    /// The `Performance` object, the Score's `output`.
    pub(crate) value: Value,
    /// The audio columns' moments, by number.
    pub(crate) columns: Vec<Moment>,
    /// `audio_column_`: the timestep's.
    pub(crate) column: Option<u32>,
    /// `audio_staffs_`: the staves' records.
    pub(crate) staves: Vec<u32>,
    ports: bool,
    skipping: bool,
    skip_last: Moment,
    offset: Moment,
    /// `Staff_performer::staff_performer_count_`, `channel_count_` and
    /// `static_channel_map_`.
    pub(crate) staff_performers: u32,
    pub(crate) channel_count: i32,
    pub(crate) channel_map: Vec<(String, i32)>,
}

impl State {
    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.value);
    }

    /// `new Audio_column (when)`, the timestep's.
    fn new_column(&mut self, when: Moment) {
        self.column = Some(self.columns.len() as u32);
        self.columns.push(when);
    }
}

/// A performance's staff, as it is written.
#[derive(Clone)]
struct Staff {
    track_name: String,
    control: bool,
    end: Moment,
    percussion: bool,
    merge_unisons: bool,
    /// Its items: their columns' moments, channels and what they are.
    items: Vec<(Moment, u8, Audio)>,
}

/// `Performance`: a `\midi` translation's output.
#[derive(Clone)]
pub struct Performance {
    /// `midi_`, the `\midi` it was translated with.
    midi: Value,
    /// `headers_`, the innermost first.
    headers: Value,
    staves: Vec<Staff>,
    /// The dynamic spans its notes point to, by the number they give.
    dynamics: Vec<AudioSpanDynamic>,
    /// `start_mom_`: where every track starts.
    start: Moment,
    ports: bool,
    /// The files `ly:performance-write` wrote, by name.
    written: Vec<(String, Vec<u8>)>,
}

impl Foreign for Performance {
    fn type_name(&self) -> &'static str {
        "Performance"
    }

    fn print(&self, _: &Heap, out: &mut String) {
        out.push_str("#<Performance>");
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.midi);
        tracer.trace(self.headers);
    }

    /// A copy for S5's undo log: the compile gives it its headers and
    /// writes it after the translation, and N8's snapshots give it back as
    /// a checkpoint had it (D241).
    fn snapshot(&self) -> Option<alloc::boxed::Box<dyn Foreign>> {
        Some(alloc::boxed::Box::new(self.clone()))
    }
}

/// N8's resume (D241): the performance `value` made with the `\midi`
/// `old` given `new`, the next compile's.
pub(crate) fn take_midi(heap: &mut Heap, value: Value, old: Value, new: Value) {
    if let Some(p) = heap.foreign_mut::<Performance>(value)
        && p.midi == old
    {
        p.midi = new;
    }
}

/// `Score_performer::initialize`'s performance: the Score's `output`, its
/// `\midi` the translation's.
pub(crate) fn initialize(host: &mut dyn Host, t: &Translation, score: CtxId) {
    let midi = t.arena.borrow().output_def();
    let heap = host.heap();
    let value = heap.wrap(Performance {
        midi,
        headers: Value::NULL,
        staves: Vec::new(),
        dynamics: Vec::new(),
        start: Moment::new(Rational::INFINITY, Rational::ZERO),
        ports: false,
        written: Vec::new(),
    });
    let output = heap.intern("output");
    let mut a = t.arena.borrow_mut();
    a.set(score, output, value);
    a.performance = Some(State {
        value,
        columns: Vec::new(),
        column: None,
        staves: Vec::new(),
        ports: false,
        skipping: false,
        skip_last: Moment::default(),
        offset: Moment::default(),
        staff_performers: 0,
        channel_count: 0,
        channel_map: Vec::new(),
    });
}

/// `Score_performer::prepare`: the timestep's column, at its moment.
pub(crate) fn prepare(a: &mut Arena) {
    let now = a.now();
    if let Some(s) = a.performance.as_mut() {
        s.new_column(now);
    }
}

/// `Score_performer::one_time_step`'s start: a column if `prepare` made
/// none (the Score was made in this timestep), then `midiSkipOffset`'s
/// skipping, which offsets the column by the time skipped so far. Whether
/// `skipTypesetting` skips the timestep.
pub(crate) fn one_time_step(host: &mut dyn Host, t: &Translation, score: CtxId) -> bool {
    let heap = host.heap();
    let skip_sym = heap.intern("skipTypesetting");
    let offset_sym = heap.intern("midiSkipOffset");
    let mut a = t.arena.borrow_mut();
    let skip = a.get(score, skip_sym) == Value::TRUE;
    let now = a.now();
    let Some(s) = a.performance.as_mut() else {
        return skip;
    };
    if s.column.is_none() {
        s.new_column(now);
    }
    let Some(column) = s.column else {
        return skip;
    };
    let when = s.columns[column as usize];
    let mut offset = None;
    if skip {
        if !s.skipping {
            s.skipping = true;
        } else {
            s.offset = sub(s.offset, sub(when, s.skip_last));
            offset = Some(s.offset);
        }
        s.skip_last = when;
    } else if s.skipping {
        s.offset = sub(s.offset, sub(when, s.skip_last));
        offset = Some(s.offset);
        s.skipping = false;
    }
    // `offset_when`.
    s.columns[column as usize] = add(when, s.offset);
    if let Some(m) = offset {
        let m = heap.wrap(m);
        a.set(score, offset_sym, m);
    }
    skip
}

/// `Score_performer::finish`'s ports: one a track for `midiChannelMapping
/// = #'voice`.
pub(crate) fn finish(host: &mut dyn Host, t: &Translation, score: CtxId) {
    let heap = host.heap();
    let mapping = heap.intern("midiChannelMapping");
    let voice = heap.intern("voice");
    let mut a = t.arena.borrow_mut();
    let ports = a.get(score, mapping) == voice;
    if let Some(s) = a.performance.as_mut() {
        s.ports = ports;
    }
}

/// A `Staff_performer` initialized (`begun`) or finalized: the last to
/// finish resets the channels the staves share.
pub(crate) fn staff_performer(a: &mut Arena, begun: bool) {
    a.effect(crate::replay::Effect::StaffPerformer(begun));
    if let Some(s) = a.performance.as_mut() {
        if begun {
            s.staff_performers += 1;
        } else {
            s.staff_performers = s.staff_performers.saturating_sub(1);
            if s.staff_performers == 0 {
                s.channel_map.clear();
                s.channel_count = 0;
            }
        }
    }
}

/// `Score_performer::announce_element`'s part: a staff announced to the
/// Score is one of the performance's.
pub(crate) fn announced(a: &mut Arena, audio: u32) {
    let staff = matches!(a.records.audio[audio as usize].audio, Audio::Staff(_));
    if let Some(s) = a.performance.as_mut()
        && staff
    {
        s.staves.push(audio);
    }
}

/// `Score_performer::acknowledge_audio_elements`' part: each item of
/// `audio`, announced to the Score in this pass, given the timestep's
/// column.
pub(crate) fn add_to_column(a: &mut Arena, audio: &[u32]) {
    let Some(column) = a.performance.as_ref().and_then(|s| s.column) else {
        return;
    };
    for &id in audio {
        let r = &mut a.records.audio[id as usize];
        if r.audio.is_item() {
            r.column = Some(column);
        }
    }
}

/// The performance of the translation `a` ran, once it is over: its
/// staves' items moved into its object, and its start found
/// (`Performance::process`); `None` if the translation performed nothing.
pub(crate) fn finished(heap: &mut Heap, a: &mut Arena) -> Option<Value> {
    let s = a.performance.take()?;
    let mut start = Moment::new(Rational::INFINITY, Rational::ZERO);
    for r in a.records.audio.iter() {
        if let Some(c) = r.column.filter(|_| r.audio.is_item()) {
            let when = s.columns[c as usize];
            if when.compare(start).is_lt() {
                start = when;
            }
        }
    }
    let mut staves = Vec::new();
    let mut dynamics = Vec::new();
    // The spans' records, by the number they have among `dynamics`.
    let mut numbers = alloc::collections::BTreeMap::new();
    for &id in &s.staves {
        let Audio::Staff(staff) = core::mem::take(&mut a.records.audio[id as usize].audio) else {
            continue;
        };
        let mut items = Vec::new();
        for &item in &staff.items {
            let r = &a.records.audio[item as usize];
            // An item with no column was announced after the Score's
            // acknowledging, which LilyPond would not write.
            let Some(c) = r.column else {
                continue;
            };
            let mut audio = r.audio.clone();
            if let Audio::Note(n) = &mut audio {
                n.dynamic = n.dynamic.and_then(|d| {
                    let Audio::SpanDynamic(span) = a.records.audio[d as usize].audio else {
                        return None;
                    };
                    Some(*numbers.entry(d).or_insert_with(|| {
                        dynamics.push(span);
                        dynamics.len() as u32 - 1
                    }))
                });
            }
            items.push((s.columns[c as usize], r.channel, audio));
        }
        staves.push(Staff {
            track_name: staff.track_name,
            control: staff.control,
            end: staff.end,
            percussion: staff.percussion,
            merge_unisons: staff.merge_unisons,
            items,
        });
    }
    let p = heap.foreign_mut::<Performance>(s.value)?;
    p.staves = staves;
    p.dynamics = dynamics;
    p.start = start;
    p.ports = s.ports;
    Some(s.value)
}

/// `Performance::push_header`.
pub fn push_header(heap: &mut Heap, performance: Value, header: Value) {
    let Some(headers) = heap.foreign::<Performance>(performance).map(|p| p.headers) else {
        return;
    };
    let list = heap.cons(header, headers);
    if let Some(p) = heap.foreign_mut::<Performance>(performance) {
        p.headers = list;
    }
}

/// The files `ly:performance-write` wrote of `performance` since the last
/// time, by name, in order.
pub fn take_written(heap: &mut Heap, performance: Value) -> Vec<(String, Vec<u8>)> {
    heap.foreign_mut::<Performance>(performance)
        .map(|p| core::mem::take(&mut p.written))
        .unwrap_or_default()
}

/// An event of a track before it is written: a note, or an event's bytes
/// at a tick.
#[derive(Clone)]
enum Event {
    Note(lily_engine_smf::Item<'static>),
    Raw(i32, Vec<u8>),
}

impl Performance {
    /// `Performance::output`: the file, the performance named `name`, and
    /// what writing it reported, with its kind.
    fn output(&mut self, name: &str) -> (Vec<u8>, Vec<(&'static str, String)>) {
        let mut diagnostics = Vec::new();
        let start = moment_to_ticks(self.start);
        // The control track's first item is the placeholder of the
        // performance's name, which is left out if it has none.
        for s in self.staves.iter_mut().filter(|s| s.control) {
            if let Some((_, _, Audio::Text(t))) = s.items.first_mut()
                && t.kind == TRACK_NAME
            {
                if name.is_empty() {
                    s.items.remove(0);
                } else {
                    t.text = String::from(name);
                    s.track_name = String::from(name);
                }
            }
        }
        // Each track's events, whose bytes the writer's items borrow.
        let mut tracks = Vec::new();
        for (i, s) in self.staves.iter().enumerate() {
            let mut events = Vec::new();
            // `Midi_track`'s port, before any event.
            if self.ports {
                events.push(Event::Raw(start, alloc::vec![0xff, 0x21, 0x01, i as u8]));
            }
            // `Midi_walker`'s stable sort by column, the order the writer
            // walks them in.
            let mut items: Vec<&(Moment, u8, Audio)> = s.items.iter().collect();
            items.sort_by(|a, b| a.0.compare(b.0));
            for (when, channel, audio) in items {
                let tick = moment_to_ticks(*when);
                let midi = get_midi(
                    audio,
                    *when,
                    *channel,
                    s.percussion,
                    &self.dynamics,
                    &mut diagnostics,
                );
                match midi {
                    Some(Midi::Note {
                        channel,
                        key,
                        velocity,
                        bend,
                    }) => {
                        let Audio::Note(n) = audio else {
                            continue;
                        };
                        // `Midi_walker::process`: a note of no length is not
                        // started.
                        if n.length == Moment::default() {
                            continue;
                        }
                        let len = (moment_to_real(n.length) * f64::from(384 * 4)) as i32;
                        events.push(Event::Note(lily_engine_smf::Item::Note {
                            tick,
                            len,
                            channel,
                            key,
                            velocity,
                            bend,
                        }));
                    }
                    Some(Midi::Raw(bytes)) => events.push(Event::Raw(tick, bytes)),
                    None => {}
                }
            }
            tracks.push(events);
        }
        let staves: Vec<lily_engine_smf::Staff<'_>> = self
            .staves
            .iter()
            .zip(&tracks)
            .map(|(s, events)| lily_engine_smf::Staff {
                items: events
                    .iter()
                    .map(|e| match e {
                        Event::Note(item) => *item,
                        Event::Raw(tick, bytes) => {
                            lily_engine_smf::Item::Raw { tick: *tick, bytes }
                        }
                    })
                    .collect(),
                start,
                end: moment_to_ticks(s.end),
                merge_unisons: s.merge_unisons,
            })
            .collect();
        let mut out = Vec::new();
        for _ in 0..lily_engine_smf::write(&staves, &mut out) {
            diagnostics.push((
                "programming-error",
                String::from("Going back in MIDI time."),
            ));
        }
        (out, diagnostics)
    }
}

// --- the Scheme face (performance-scheme.cc) ----------------------------------

fn performance_arg(cx: &mut Cx, subr: &str, v: Value) -> Result<(), lily_engine_scheme::Thrown> {
    match cx.heap().foreign::<Performance>(v) {
        Some(_) => Ok(()),
        None => Err(cx.wrong_type(subr, 1, v, "Performance")),
    }
}

/// `(ly:performance-headers performance)`: its headers, the innermost
/// first.
pub(crate) fn ly_performance_headers(cx: &mut Cx, args: &[Value]) -> R {
    performance_arg(cx, "ly:performance-headers", args[0])?;
    Ok(cx
        .heap()
        .foreign::<Performance>(args[0])
        .map_or(Value::NULL, |p| p.headers))
}

/// `(ly:performance-write performance filename name)`: `write_output`,
/// the file kept in the performance for the core ([`take_written`]), then
/// the `\midi`'s `after-writing` called with the performance and the
/// file's name.
pub(crate) fn ly_performance_write(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:performance-write";
    performance_arg(cx, SUBR, args[0])?;
    let text = |cx: &mut Cx, i: usize| match cx.heap().str(args[i]) {
        Some(s) => Ok(String::from(s.as_str())),
        None => Err(cx.wrong_type(SUBR, i + 1, args[i], "string")),
    };
    let (mut out, name) = (text(cx, 1)?, text(cx, 2)?);
    if out == "-" {
        out = String::from("lelie.midi");
    }
    let Some(p) = cx.heap_mut().foreign_mut::<Performance>(args[0]) else {
        return Ok(Value::UNSPECIFIED);
    };
    if p.staves.is_empty() {
        // Only a piece skipped whole with skipTypesetting has none.
        let text = alloc::format!("cannot create a zero-track MIDI file; skipping `{out}'");
        crate::warn::diagnose(cx, "warning", &text);
        return Ok(Value::UNSPECIFIED);
    }
    let (bytes, diagnostics) = p.output(&name);
    p.written.push((out.clone(), bytes));
    let midi = p.midi;
    for (kind, text) in &diagnostics {
        crate::warn::diagnose(cx, kind, text);
    }
    let sym = cx.heap_mut().intern("after-writing");
    match crate::output_def::lookup_variable(cx, midi, sym) {
        Some(after) if cx.heap().is_procedure(after) => {
            let file = cx.heap_mut().string(out);
            Ok(cx.tail_call(after, &[args[0], file]))
        }
        _ => Ok(Value::UNSPECIFIED),
    }
}

fn add(a: Moment, b: Moment) -> Moment {
    a.checked_add(b).unwrap_or(a)
}

fn sub(a: Moment, b: Moment) -> Moment {
    a.checked_sub(b).unwrap_or(a)
}
