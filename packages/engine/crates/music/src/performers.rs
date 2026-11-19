//! M2's performers (07 §7, MS5 step 1): `Staff_performer`
//! (`staff-performer.cc`), `Note_performer` (`note-performer.cc`),
//! `Drum_note_performer` (`drum-note-performer.cc`, MS5 step 5),
//! `Control_track_performer` (`control-track-performer.cc`) and
//! `Tempo_performer` (`tempo-performer.cc`), on N5's translator framework
//! (`translator.rs`), their items `audio.rs`' and their performance
//! `performance.rs`'.
//!
//! `Staff_performer::set_instrument` and its control initializer are MS5
//! step 3's, with the performers of `control_performers.rs`;
//! `Note_performer`'s ties and breaths step 4's, which ties its notes in
//! `Tie_performer` (`melisma.rs`).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Exception, Heap, Tag, Value};

use crate::audio::{
    Audio, AudioInstrument, AudioNote, AudioStaff, AudioText, INSTRUMENT_NAME, TEXT, TRACK_NAME,
    tie_head,
};
use crate::dispatcher::{StreamEvent, event_property};
use crate::pitch::{Pitch, Scale};
use crate::rational::{Moment, Rational};
use crate::records::Cause;
use crate::translator::{
    ACKNOWLEDGE_AUDIO, FINALIZE, INITIALIZE, Kind, PROCESS, STOP, TranslatorClass, Tx, Work,
};

type Run<T> = Result<T, Exception>;

pub(crate) const fn performer(
    name: &'static str,
    methods: u8,
    always: bool,
    hears: &'static [&'static str],
    make: fn() -> Box<dyn Work>,
    reads: &'static [&'static str],
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
        reads,
        writes: &[],
    }
}

/// `version_string ()`, the lexer's `LILYPOND_VERSION`.
const VERSION: &str = "2.26.0";

/// `now + midiSkipOffset`, where a staff ends.
fn end_moment(tx: &mut Tx<'_>) -> Moment {
    let offset = tx.get("midiSkipOffset");
    let offset = tx
        .heap()
        .foreign::<Moment>(offset)
        .copied()
        .unwrap_or_default();
    let now = tx.now();
    now.checked_add(offset).unwrap_or(now)
}

/// The staff record `staff`'s, to add to.
fn add_item(tx: &mut Tx<'_>, staff: u32, item: u32) {
    add_to_staff(&mut tx.t.arena.borrow_mut(), staff, item);
}

/// The item `item` added to the audio staff `staff`.
pub(crate) fn add_to_staff(a: &mut crate::context::Arena, staff: u32, item: u32) {
    a.effect(crate::replay::Effect::StaffItem(staff, item));
    if let Audio::Staff(s) = &mut a.records.audio[staff as usize].audio {
        s.items.push(item);
    }
}

/// The channel of the audio record `id`.
fn set_channel(tx: &mut Tx<'_>, id: u32, channel: u8) {
    let mut a = tx.t.arena.borrow_mut();
    a.records.audio[id as usize].channel = channel;
    a.audio_changed(id);
}

fn is_sym(heap: &mut Heap, v: Value, name: &str) -> bool {
    v == heap.intern(name)
}

// --- Staff_performer -----------------------------------------------------------

pub(crate) const STAFF_PERFORMER: TranslatorClass = performer(
    "Staff_performer",
    INITIALIZE | FINALIZE | ACKNOWLEDGE_AUDIO,
    false,
    &[],
    || {
        Box::new(StaffPerformer {
            instrument_string: String::new(),
            channel: -1,
            staff_map: Vec::new(),
            channel_map: Vec::new(),
        })
    },
    &["midiChannelMapping", "midiMergeUnisons", "midiSkipOffset"],
);

#[derive(Clone)]
struct StaffPerformer {
    instrument_string: String,
    channel: i32,
    /// `staff_map_`: the staves by voice, in the order they were made.
    staff_map: Vec<(String, u32)>,
    /// `channel_map_`; the static one is the performance's.
    channel_map: Vec<(String, i32)>,
}

impl Work for StaffPerformer {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.text(&self.instrument_string);
        h.num(self.channel.into());
        h.num(self.staff_map.len() as i64);
        for (voice, staff) in &self.staff_map {
            h.text(voice);
            h.record(*staff);
        }
        h.num(self.channel_map.len() as i64);
        for (name, channel) in &self.channel_map {
            h.text(name);
            h.num((*channel).into());
        }
    }

    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        crate::performance::staff_performer(&mut tx.t.arena.borrow_mut(), true);
        Ok(())
    }

    fn acknowledge_audio(&mut self, tx: &mut Tx<'_>, audio: u32) -> Run<()> {
        // Each voice its own track (`origin_contexts (this)[0]`).
        let origin = tx.t.arena.borrow().records.audio[audio as usize].origin.0;
        let voice = {
            let sym = tx.heap().intern("Voice");
            let a = tx.t.arena.borrow();
            if a.is_alias(tx.host.heap(), origin, sym) {
                String::from(a.id(origin))
            } else {
                String::new()
            }
        };
        let mapping = tx.get("midiChannelMapping");
        let instrument = is_sym(tx.heap(), mapping, "instrument");
        let str = self.new_instrument_string(tx);
        if !instrument {
            self.channel = self.get_channel(tx, &voice)?;
        } else if self.channel < 0 && str.is_empty() {
            self.channel = self.get_channel(tx, &str)?;
        }
        if !str.is_empty() {
            if !is_sym(tx.heap(), mapping, "voice") {
                self.channel = self.get_channel(tx, &str)?;
            }
            self.set_instrument(tx, self.channel, &voice)?;
            self.set_instrument_name(tx, &voice)?;
        }
        let staff = self.get_audio_staff(tx, &voice)?;
        if tx.t.arena.borrow().records.audio[audio as usize]
            .audio
            .is_item()
        {
            set_channel(tx, audio, self.channel as u8);
            add_item(tx, staff, audio);
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let end = end_moment(tx);
        {
            let mut a = tx.t.arena.borrow_mut();
            for &(_, staff) in &self.staff_map {
                if let Audio::Staff(s) = &mut a.records.audio[staff as usize].audio {
                    s.end = end;
                    a.audio_changed(staff);
                }
            }
            crate::performance::staff_performer(&mut a, false);
        }
        self.staff_map.clear();
        self.channel_map.clear();
        Ok(())
    }
}

impl StaffPerformer {
    /// `new_audio_staff`: a staff for `voice`, its track named after the
    /// context's id and the voice's.
    fn new_audio_staff(&mut self, tx: &mut Tx<'_>, voice: &str) -> Run<u32> {
        let merge_unisons = tx.get("midiMergeUnisons") == Value::TRUE;
        let mut track_name = String::from(tx.t.arena.borrow().id(tx.context));
        track_name.push(':');
        track_name.push_str(voice);
        let mut items = Vec::new();
        if track_name != ":" {
            let text = Audio::Text(AudioText {
                kind: TRACK_NAME,
                text: track_name.clone(),
            });
            items.push(tx.announce(text, Cause::None));
        } else {
            track_name.clear();
        }
        let staff = tx.announce(
            Audio::Staff(AudioStaff {
                items,
                merge_unisons,
                track_name,
                ..AudioStaff::default()
            }),
            Cause::None,
        );
        match self.staff_map.iter_mut().find(|(v, _)| v == voice) {
            Some(entry) => entry.1 = staff,
            None => self.staff_map.push((String::from(voice), staff)),
        }
        // A new staff of an instrument has its program, and so a staff made
        // by `set_instrument` has two, as LilyPond's has.
        if !self.instrument_string.is_empty() {
            self.set_instrument(tx, self.channel, voice)?;
        }
        // `Midi_control_initializer`: the controllers' properties' values.
        let channel = self.channel as u8;
        crate::control_performers::announce_control_changes(
            tx,
            None,
            |tx, name| tx.get(name),
            |tx, control, value| {
                let item = tx.announce(Audio::ControlChange { control, value }, Cause::None);
                set_channel(tx, item, channel);
                add_item(tx, staff, item);
            },
        )?;
        Ok(staff)
    }

    /// `set_instrument`: the instrument's program change on `channel` in
    /// the voice's staff, which is percussion if the instrument is.
    fn set_instrument(&mut self, tx: &mut Tx<'_>, channel: i32, voice: &str) -> Run<()> {
        // `Midi_instrument`'s name and its program, as it looks them up.
        let name = self.instrument_string.to_ascii_lowercase();
        let sym = tx.heap().intern(&name);
        let program = tx.call("midi-program", &[sym])?;
        let program = tx.heap().to_i64(program).map(|p| p as u8);
        let instrument = Audio::Instrument(AudioInstrument { name, program });
        let instrument = tx.announce(instrument, Cause::None);
        set_channel(tx, instrument, channel as u8);
        let staff = self.get_audio_staff(tx, voice)?;
        add_item(tx, staff, instrument);
        let sym = tx.heap().intern(&self.instrument_string);
        let drums = tx.call("percussion?", &[sym])? == Value::TRUE;
        let mut a = tx.t.arena.borrow_mut();
        if let Audio::Staff(s) = &mut a.records.audio[staff as usize].audio {
            s.percussion = drums;
            a.audio_changed(staff);
        }
        Ok(())
    }

    /// `get_audio_staff`: the one staff but with `midiChannelMapping =
    /// #'instrument`, which has one a voice.
    fn get_audio_staff(&mut self, tx: &mut Tx<'_>, voice: &str) -> Run<u32> {
        let mapping = tx.get("midiChannelMapping");
        if !is_sym(tx.heap(), mapping, "instrument")
            && let Some(&(_, staff)) = self.staff_map.first()
        {
            return Ok(staff);
        }
        if let Some(&(_, staff)) = self.staff_map.iter().find(|(v, _)| v == voice) {
            return Ok(staff);
        }
        if let [(ref v, staff)] = self.staff_map[..]
            && v.is_empty()
        {
            self.staff_map.push((String::from(voice), staff));
            return Ok(staff);
        }
        self.new_audio_staff(tx, voice)
    }

    /// `set_instrument_name`: the instrument's name in the voice's track.
    fn set_instrument_name(&mut self, tx: &mut Tx<'_>, voice: &str) -> Run<()> {
        let text = Audio::Text(AudioText {
            kind: INSTRUMENT_NAME,
            text: self.instrument_string.clone(),
        });
        let name = tx.announce(text, Cause::None);
        let staff = self.get_audio_staff(tx, voice)?;
        add_item(tx, staff, name);
        Ok(())
    }

    /// `new_instrument_string`: `midiInstrument` if it changed, else empty.
    fn new_instrument_string(&mut self, tx: &mut Tx<'_>) -> String {
        let v = tx.get("midiInstrument");
        match tx.heap().str(v) {
            Some(s) if s.as_str() != self.instrument_string => {
                self.instrument_string = String::from(s.as_str());
                self.instrument_string.clone()
            }
            _ => String::new(),
        }
    }

    /// `get_channel`: the channel of `instrument` (a voice, or an
    /// instrument with `midiChannelMapping = #'instrument`), a new one
    /// given in order, the percussion channel 10 passed over and from 16
    /// on wrapped around with LilyPond's warnings (D99).
    fn get_channel(&mut self, tx: &mut Tx<'_>, instrument: &str) -> Run<i32> {
        let mapping = tx.get("midiChannelMapping");
        let by_instrument = is_sym(tx.heap(), mapping, "instrument");
        let by_staff = is_sym(tx.heap(), mapping, "staff");
        if by_staff && self.channel >= 0 {
            return Ok(self.channel);
        }
        let (mut shared, mut count) = {
            let mut a = tx.t.arena.borrow_mut();
            match a.performance.as_mut() {
                Some(s) => (core::mem::take(&mut s.channel_map), s.channel_count),
                None => (Vec::new(), 0),
            }
        };
        let map = if by_instrument {
            &mut shared
        } else {
            &mut self.channel_map
        };
        let found = map.iter().find(|(i, _)| i == instrument).map(|&(_, c)| c);
        let channel = match found {
            Some(c) => Ok(c),
            None => {
                // The channels the staves share: no replay of this call.
                tx.t.arena.borrow().unreplayable();
                let mut channel = if by_staff {
                    count += 1;
                    count - 1
                } else {
                    map.len() as i32
                };
                // MIDI players tend to ignore instruments on channel 10.
                if channel % 16 == 9 {
                    set(map, "percussion", channel);
                    channel += 1;
                    count += 1;
                }
                let mut warned = Ok(());
                if channel > 15 {
                    warned = tx
                        .warning("MIDI channel wrapped around")
                        .and_then(|()| tx.warning("remapping modulo 16"));
                    channel %= 16;
                }
                set(map, instrument, channel);
                warned.map(|()| channel)
            }
        };
        if let Some(s) = tx.t.arena.borrow_mut().performance.as_mut() {
            s.channel_map = shared;
            s.channel_count = count;
        }
        channel
    }
}

/// `map[key] = value`.
fn set(map: &mut Vec<(String, i32)>, key: &str, value: i32) {
    match map.iter_mut().find(|(k, _)| k == key) {
        Some(entry) => entry.1 = value,
        None => map.push((String::from(key), value)),
    }
}

// --- Note_performer --------------------------------------------------------------

pub(crate) const NOTE_PERFORMER: TranslatorClass = performer(
    "Note_performer",
    PROCESS | STOP,
    false,
    &[
        "note-event",
        "breathing-event",
        "tie-event",
        "articulation-event",
    ],
    || {
        Box::new(NotePerformer {
            note_evs: Vec::new(),
            script_evs: Vec::new(),
            notes: Vec::new(),
            last_notes: Vec::new(),
            last_start: Moment::default(),
        })
    },
    &[],
);

#[derive(Clone)]
struct NotePerformer {
    note_evs: Vec<Value>,
    script_evs: Vec<Value>,
    notes: Vec<u32>,
    last_notes: Vec<u32>,
    last_start: Moment,
}

impl Work for NotePerformer {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.records(&self.notes);
        h.records(&self.last_notes);
        h.moment(self.last_start);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        match which {
            0 => self.note_evs.push(ev),
            1 => self.listen_breathing(tx, ev)?,
            _ => self.script_evs.push(ev),
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.note_evs.is_empty() {
            return Ok(());
        }
        let transposing = tx.get("instrumentTransposition");
        let transposing = tones(tx.heap(), transposing).unwrap_or(Rational::ZERO);
        let now = tx.now();
        let context = tx.t.arena.borrow().handle(tx.context);
        for n in self.note_evs.clone() {
            let pitch = event_property(tx.heap(), n, "pitch");
            let Some(pitch) = tones(tx.heap(), pitch) else {
                continue;
            };
            let len = event_length(tx.heap(), n, now);
            let (len, tie_event, velocity) = articulate(tx, n, &self.script_evs, len, context)?;
            let note = Audio::Note(AudioNote {
                tones: pitch.checked_add(transposing).unwrap_or(pitch),
                length: len,
                dynamic: None,
                extra_velocity: velocity,
                tied: None,
                tie_event,
            });
            self.notes.push(tx.announce(note, Cause::Event(n)));
            // Grace notes shorten the note before them, the first of its
            // tie, if it would sound into them.
            if !now.grace.is_zero() && self.last_start.grace.is_zero() {
                let mut a = tx.t.arena.borrow_mut();
                for &last in &self.last_notes {
                    let head = tie_head(&a.records.audio, last);
                    let Some(start) = column_when(&a, head) else {
                        continue;
                    };
                    if let Audio::Note(h) = &mut a.records.audio[head as usize].audio
                        && let Some(end) = start.checked_add(h.length)
                        && end.compare(now).is_gt()
                        && let Some(length) = now.checked_sub(start)
                    {
                        h.length = length;
                        a.audio_changed(head);
                    }
                }
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if !self.note_evs.is_empty() {
            self.last_notes = core::mem::take(&mut self.notes);
            self.last_start = tx.now();
        }
        self.notes.clear();
        self.note_evs.clear();
        self.script_evs.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(&self.note_evs);
        out.extend(&self.script_evs);
    }
}

impl NotePerformer {
    /// `listen_breathing`: the notes before shortened, if they would sound
    /// longer than the breath's `midi-length` of the time since they
    /// started leaves them, each the first of its tie, which has its
    /// length.
    fn listen_breathing(&mut self, tx: &mut Tx<'_>, ev: Value) -> Run<()> {
        let f = event_property(tx.heap(), ev, "midi-length");
        if !tx.heap().is_procedure(f) {
            return Ok(());
        }
        let now = tx.now();
        let context = tx.t.arena.borrow().handle(tx.context);
        for &last in &self.last_notes {
            let (head, start, head_start) = {
                let a = tx.t.arena.borrow();
                let head = tie_head(&a.records.audio, last);
                (head, column_when(&a, last), column_when(&a, head))
            };
            let (Some(start), Some(head_start)) = (start, head_start) else {
                continue;
            };
            let available = now.checked_sub(start).unwrap_or_default();
            let m = tx.heap().wrap(available);
            let got = tx.apply(f, &[m, context])?;
            let len = tx
                .heap()
                .foreign::<Moment>(got)
                .copied()
                .unwrap_or(available);
            let Some(len) = start
                .checked_sub(head_start)
                .and_then(|d| len.checked_add(d))
            else {
                continue;
            };
            let mut a = tx.t.arena.borrow_mut();
            if let Audio::Note(h) = &mut a.records.audio[head as usize].audio
                && len.compare(h.length).is_lt()
            {
                h.length = len;
                a.audio_changed(head);
            }
        }
        Ok(())
    }
}

// --- Drum_note_performer ---------------------------------------------------------

pub(crate) const DRUM_NOTE_PERFORMER: TranslatorClass = performer(
    "Drum_note_performer",
    PROCESS | STOP,
    false,
    &["note-event", "tie-event", "articulation-event"],
    || {
        Box::new(DrumNotePerformer {
            note_evs: Vec::new(),
            script_evs: Vec::new(),
        })
    },
    &[],
);

#[derive(Clone)]
struct DrumNotePerformer {
    note_evs: Vec<Value>,
    script_evs: Vec<Value>,
}

impl Work for DrumNotePerformer {
    fn listen(&mut self, _: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        match which {
            0 => self.note_evs.push(ev),
            _ => self.script_evs.push(ev),
        }
        Ok(())
    }

    /// Each note's pitch its `drum-type`'s in `drumPitchTable`, the last
    /// heard first.
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let tab = tx.get("drumPitchTable");
        let now = tx.now();
        let context = tx.t.arena.borrow().handle(tx.context);
        while let Some(n) = self.note_evs.pop() {
            let sym = event_property(tx.heap(), n, "drum-type");
            let mut defn = Value::NULL;
            if sym.tag() == Tag::Symbol && tx.call("hash-table?", &[tab])? == Value::TRUE {
                defn = tx.call("hashq-ref", &[tab, sym, Value::NULL])?;
            }
            let Some(pitch) = tones(tx.heap(), defn) else {
                continue;
            };
            let len = event_length(tx.heap(), n, now);
            let (len, tie_event, velocity) = articulate(tx, n, &self.script_evs, len, context)?;
            let note = Audio::Note(AudioNote {
                tones: pitch,
                length: len,
                dynamic: None,
                extra_velocity: velocity,
                tied: None,
                tie_event,
            });
            tx.announce(note, Cause::Event(n));
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.note_evs.clear();
        self.script_evs.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(&self.note_evs);
        out.extend(&self.script_evs);
    }
}

/// The articulations heard and the note `n`'s own: the note's length
/// after each's `midi-length`, whether one is a tie, and the sum of their
/// `midi-extra-velocity`.
fn articulate(
    tx: &mut Tx<'_>,
    n: Value,
    script_evs: &[Value],
    mut len: Moment,
    context: Value,
) -> Run<(Moment, bool, i32)> {
    let mut tie_event = false;
    let mut velocity = 0;
    let mut list = event_property(tx.heap(), n, "articulations");
    for &ev in script_evs.iter().rev() {
        list = tx.heap().cons(ev, list);
    }
    let depth = tx.heap().root_depth();
    tx.heap().root(list);
    let ran = (|| {
        let mut p = list;
        while let (Some(ev), Some(rest)) = (tx.heap().car(p), tx.heap().cdr(p)) {
            p = rest;
            if tx.heap().foreign::<StreamEvent>(ev).is_none() {
                continue;
            }
            if in_class(tx.heap(), ev, "tie-event") {
                tie_event = true;
            }
            let f = event_property(tx.heap(), ev, "midi-length");
            if tx.heap().is_procedure(f) {
                let m = tx.heap().wrap(len);
                let got = tx.apply(f, &[m, context])?;
                if let Some(&m) = tx.heap().foreign::<Moment>(got) {
                    len = m;
                }
            }
            let extra = event_property(tx.heap(), ev, "midi-extra-velocity");
            velocity += extra.as_fixnum().unwrap_or(0) as i32;
        }
        Ok(())
    })();
    tx.heap().unroot_to(depth);
    ran.map(|()| (len, tie_event, velocity))
}

/// `audio_column_->when ()` of the item `item`, if it has a column.
fn column_when(a: &crate::context::Arena, item: u32) -> Option<Moment> {
    let column = a.records.audio[item as usize].column?;
    a.performance.as_ref().map(|s| s.columns[column as usize])
}

/// `tone_pitch` of the pitch `v`, `None` for no pitch.
fn tones(heap: &Heap, v: Value) -> Option<Rational> {
    let p = heap.foreign::<Pitch>(v)?;
    p.tones(heap.foreign::<Scale>(p.scale)?)
}

/// `get_event_length (e, now)`: the event's `length`, in grace time its
/// grace part.
fn event_length(heap: &mut Heap, ev: Value, now: Moment) -> Moment {
    let v = event_property(heap, ev, "length");
    let len = heap.foreign::<Moment>(v).copied().unwrap_or_default();
    if now.grace.is_zero() {
        len
    } else {
        Moment::new(Rational::ZERO, len.main)
    }
}

/// `in_event_class`.
fn in_class(heap: &mut Heap, ev: Value, class: &str) -> bool {
    let class = heap.intern(class);
    let mut p = event_property(heap, ev, "class");
    while let (Some(c), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if c == class {
            return true;
        }
        p = rest;
    }
    false
}

// --- Control_track_performer ----------------------------------------------------

pub(crate) const CONTROL_TRACK_PERFORMER: TranslatorClass = performer(
    "Control_track_performer",
    PROCESS | FINALIZE | ACKNOWLEDGE_AUDIO,
    true,
    &[],
    || Box::new(ControlTrackPerformer { track: None }),
    &["midiSkipOffset"],
);

#[derive(Clone)]
struct ControlTrackPerformer {
    track: Option<u32>,
}

impl Work for ControlTrackPerformer {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.maybe_record(self.track);
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.track.is_some() {
            return Ok(());
        }
        let staff = Audio::Staff(AudioStaff {
            track_name: String::from("control track"),
            control: true,
            ..AudioStaff::default()
        });
        let track = tx.announce(staff, Cause::None);
        self.track = Some(track);
        // The first is the placeholder of the performance's name
        // (`Performance::output`).
        let mut id = String::from("LilyPond ");
        id.push_str(VERSION);
        while id.len() < 30 {
            id.push(' ');
        }
        for (kind, text) in [
            (TRACK_NAME, String::from("control track")),
            (TEXT, String::from("creator: ")),
            (TEXT, id),
        ] {
            let item = tx.announce(Audio::Text(AudioText { kind, text }), Cause::None);
            add_item(tx, track, item);
        }
        Ok(())
    }

    fn acknowledge_audio(&mut self, tx: &mut Tx<'_>, audio: u32) -> Run<()> {
        let Some(track) = self.track else {
            return Ok(());
        };
        let taken = match &tx.t.arena.borrow().records.audio[audio as usize].audio {
            Audio::Tempo(_) | Audio::TimeSignature(_) => true,
            Audio::Text(t) => t.kind == crate::audio::MARKER,
            _ => false,
        };
        if taken {
            add_item(tx, track, audio);
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // None if skipTypesetting skipped the whole score.
        if let Some(track) = self.track {
            let end = end_moment(tx);
            let mut a = tx.t.arena.borrow_mut();
            if let Audio::Staff(s) = &mut a.records.audio[track as usize].audio {
                s.end = end;
                a.audio_changed(track);
            }
        }
        Ok(())
    }
}

// --- Tempo_performer -------------------------------------------------------------

pub(crate) const TEMPO_PERFORMER: TranslatorClass = performer(
    "Tempo_performer",
    PROCESS | STOP,
    true,
    &["tempo-change-event"],
    || {
        Box::new(TempoPerformer {
            event: None,
            wpm: -Rational::INFINITY,
        })
    },
    &["tempoWholesPerMinute"],
);

#[derive(Clone)]
struct TempoPerformer {
    event: Option<Value>,
    wpm: Rational,
}

impl Work for TempoPerformer {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.rational(self.wpm);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        crate::timing::assign_event_once(tx, &mut self.event, ev).map(|_| ())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let v = tx.get("tempoWholesPerMinute");
        let wpm = crate::rational_of(tx.heap(), v).unwrap_or(self.wpm);
        if wpm != self.wpm {
            let cause = self.event.map_or(Cause::None, Cause::Event);
            tx.announce(Audio::Tempo(wpm), cause);
            self.wpm = wpm;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.event = None;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.event);
    }
}
