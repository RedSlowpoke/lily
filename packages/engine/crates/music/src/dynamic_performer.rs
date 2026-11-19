//! M3's `Dynamic_performer` (`dynamic-performer.cc`; 07 §7, MS5 step 2):
//! the dynamic spans of a voice, whose volumes give its notes' velocities
//! (`midi_item.rs`).
//!
//! Some span is always open, from the voice's first timestep on: one of a
//! volume, or a (de)crescendo. An absolute dynamic, or a hairpin's start or
//! end, closes it and opens the next. A closed span waits in a queue until
//! the volumes around it are known: the first (de)crescendo, those that
//! change in its direction or not at all after it (the departure), then
//! those that change back or not at all (the return), cut short by an
//! absolute dynamic, a change in the first direction again, or the end of
//! the voice. Each (de)crescendo of a queue is given its share of the
//! queue's change in volume by its share of the queue's time spent
//! changing.
//!
//! Volumes are equalized by `midiMinimumVolume` and `midiMaximumVolume`,
//! else by `instrumentEqualizer` (midi-init.ly's, of `midiInstrument` or
//! `instrumentName` or "piano"); an absolute dynamic's is
//! `dynamicAbsoluteVolumeFunction`'s. Where clang fuses a multiply and an
//! add in LilyPond's arm64 binary (`fmadd`), the port does (`libm::fma`,
//! correctly rounded as the instruction is, and called a few times a span).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_scheme::{Exception, Heap, Value};

use crate::audio::{Audio, AudioSpanDynamic};
use crate::dispatcher::event_property;
use crate::records::Cause;
use crate::translator::{ACKNOWLEDGE_AUDIO, FINALIZE, PROCESS, STOP, TranslatorClass, Tx, Work};

type Run<T> = Result<T, Exception>;

/// `Direction`: -1, 0 or 1.
type Dir = i8;
const SMALLER: Dir = -1;
const CENTER: Dir = 0;
const BIGGER: Dir = 1;
/// A span event's `span-direction`.
const START: Dir = -1;

/// The listeners' numbers but the crescendo's (1).
const DECRESCENDO: usize = 0;
const ABSOLUTE_DYNAMIC: usize = 2;

pub(crate) const DYNAMIC_PERFORMER: TranslatorClass = crate::performers::performer(
    "Dynamic_performer",
    PROCESS | STOP | FINALIZE | ACKNOWLEDGE_AUDIO,
    // A span opens in each voice's first timestep, heard or not.
    true,
    &[
        "decrescendo-event",
        "crescendo-event",
        "absolute-dynamic-event",
    ],
    || Box::<DynamicPerformer>::default(),
    &[
        "dynamicAbsoluteVolumeFunction",
        "instrumentEqualizer",
        "midiMaximumVolume",
        "midiMinimumVolume",
        "midiInstrument",
    ],
);

/// The search for the queue's pattern.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum State {
    /// Waiting for a (de)crescendo.
    #[default]
    Initial,
    /// The first span queued, gathering those of its direction.
    Depart,
    /// Gathering those of the other direction.
    Return,
}

/// `UnfinishedSpan`: a span's record and its direction of growth.
#[derive(Clone, Copy)]
struct Span {
    record: u32,
    grow: Dir,
}

/// `DynamicQueue`.
#[derive(Clone, Default)]
struct Queue {
    spans: Vec<Span>,
    /// The total duration of its (de)crescendi.
    change_duration: f64,
    min_target: f64,
    max_target: f64,
}

impl Queue {
    fn clear(&mut self) {
        self.spans.clear();
        self.change_duration = 0.0;
    }
}

#[derive(Clone, Default)]
struct DynamicPerformer {
    /// The notes played in this timestep, to point to the open span.
    notes: Vec<u32>,
    script_event: Option<Value>,
    /// `span_events_`: the start's and the stop's.
    span_events: [Option<Value>; 2],
    next_grow: Dir,
    depart_dir: Dir,
    open: Option<Span>,
    depart: Queue,
    ret: Queue,
    state: State,
}

/// The span of the record `id`.
fn span(tx: &Tx<'_>, id: u32) -> AudioSpanDynamic {
    match tx.t.arena.borrow().records.audio[id as usize].audio {
        Audio::SpanDynamic(s) => s,
        _ => unreachable!("a dynamic span's record"),
    }
}

/// The span of the record `id`, changed by `f`, and its programming errors
/// reported.
fn change<E: IntoIterator<Item = alloc::string::String>>(
    tx: &mut Tx<'_>,
    id: u32,
    f: impl FnOnce(&mut AudioSpanDynamic) -> E,
) -> Run<()> {
    let errors = {
        let mut a = tx.t.arena.borrow_mut();
        let Audio::SpanDynamic(s) = &mut a.records.audio[id as usize].audio else {
            unreachable!("a dynamic span's record")
        };
        let errors = f(s);
        a.audio_changed(id);
        errors
    };
    for e in errors {
        tx.programming_error(&e)?;
    }
    Ok(())
}

/// `minmax (d, a, b)`: `std::max` up, else `std::min`.
fn minmax(d: Dir, a: f64, b: f64) -> f64 {
    if d == BIGGER { max(a, b) } else { min(a, b) }
}

/// `std::min`: `b < a ? b : a`.
fn min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// `std::max`: `a < b ? b : a`.
fn max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// `Direction (x)` of a real: 0 for either zero, else its sign bit's.
fn direction(x: f64) -> Dir {
    if x == 0.0 {
        CENTER
    } else if x.is_sign_negative() {
        SMALLER
    } else {
        BIGGER
    }
}

/// `from_scm<Direction> (get_property (ev, "span-direction"))`.
fn span_direction(heap: &mut Heap, ev: Value) -> Dir {
    let d = event_property(heap, ev, "span-direction");
    heap.to_i64(d).map_or(CENTER, |d| d.signum() as Dir)
}

/// `Interval::length`: 0 if it is empty.
fn length(down: f64, up: f64) -> f64 {
    if down > up { 0.0 } else { up - down }
}

impl Work for DynamicPerformer {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.records(&self.notes);
        h.flags(&[self.span_events[0].is_some(), self.span_events[1].is_some()]);
        h.num(self.next_grow.into());
        h.num(self.depart_dir.into());
        match self.open {
            Some(s) => {
                h.record(s.record);
                h.num(s.grow.into());
            }
            None => h.num(-2),
        }
        for q in [&self.depart, &self.ret] {
            h.num(q.spans.len() as i64);
            for s in &q.spans {
                h.record(s.record);
                h.num(s.grow.into());
            }
            h.float(q.change_duration);
            h.float(q.min_target);
            h.float(q.max_target);
        }
        h.num(self.state as i64);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        if which == ABSOLUTE_DYNAMIC {
            return crate::timing::assign_event_once(tx, &mut self.script_event, ev).map(|_| ());
        }
        let d = span_direction(tx.heap(), ev);
        let slot = &mut self.span_events[usize::from(d != START)];
        if crate::timing::assign_event_once(tx, slot, ev)? && d == START {
            self.next_grow = if which == DECRESCENDO {
                SMALLER
            } else {
                BIGGER
            };
        }
        Ok(())
    }

    fn acknowledge_audio(&mut self, tx: &mut Tx<'_>, audio: u32) -> Run<()> {
        if matches!(
            tx.t.arena.borrow().records.audio[audio as usize].audio,
            Audio::Note(_)
        ) {
            self.notes.push(audio);
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let mut volume = -1.0;
        if let Some(ev) = self.script_event {
            // An explicit dynamic.
            let text = event_property(tx.heap(), ev, "text");
            volume = self.look_up_absolute_volume(tx, text, AudioSpanDynamic::DEFAULT_VOLUME)?;
            volume = self.equalize_volume(tx, volume)?;
        } else if self.open.is_none() {
            // The first time only.
            volume = self.equalize_volume(tx, AudioSpanDynamic::DEFAULT_VOLUME)?;
        }
        // The open span ends where a span or a dynamic is.
        let [start, stop] = self.span_events;
        if self.open.is_some() && (start.is_some() || stop.is_some() || self.script_event.is_some())
        {
            self.close_and_enqueue_span(tx)?;
            if self.script_event.is_some() {
                self.state = State::Initial;
                volume = self.finish_queued_spans(tx, volume)?;
            }
        }
        // A new one, so that some dynamic is always in effect.
        if self.open.is_none() {
            if self.drive_state_machine(self.next_grow) {
                volume = self.finish_queued_spans(tx, volume)?;
            }
            if volume < 0.0 {
                volume = self.equalize_volume(tx, AudioSpanDynamic::DEFAULT_VOLUME)?;
            }
            let cause = start.or(self.script_event).or(stop);
            let (made, errors) = AudioSpanDynamic::new(tx.now(), volume);
            for e in errors {
                tx.programming_error(&e)?;
            }
            let record = tx.announce(
                Audio::SpanDynamic(made),
                cause.map_or(Cause::None, Cause::Event),
            );
            self.open = Some(Span {
                record,
                grow: self.next_grow,
            });
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // The notes of the timestep point to the open span.
        match self.open {
            None => tx.programming_error("no current dynamic")?,
            Some(open) => {
                let mut a = tx.t.arena.borrow_mut();
                for &n in &self.notes {
                    if let Audio::Note(note) = &mut a.records.audio[n as usize].audio {
                        note.dynamic = Some(open.record);
                        a.audio_changed(n);
                    }
                }
            }
        }
        self.notes.clear();
        self.script_event = None;
        self.span_events = [None; 2];
        self.next_grow = CENTER;
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.open.is_some() {
            self.close_and_enqueue_span(tx)?;
        }
        self.finish_queued_spans(tx, -1.0).map(|_| ())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.script_event);
        out.extend(self.span_events.iter().flatten());
    }
}

impl DynamicPerformer {
    /// `drive_state_machine`: whether the queues are to be finished, a
    /// change in the first direction again.
    fn drive_state_machine(&mut self, next: Dir) -> bool {
        match self.state {
            State::Initial => {
                if next != CENTER {
                    self.state = State::Depart;
                    self.depart_dir = next;
                }
            }
            State::Depart => {
                if next == -self.depart_dir {
                    self.state = State::Return;
                }
            }
            State::Return => {
                if next == self.depart_dir {
                    self.state = State::Depart;
                    return true;
                }
            }
        }
        false
    }

    /// `close_and_enqueue_span`: the open span ended now and queued, with
    /// the equalized limits of the queue's target as they are now.
    fn close_and_enqueue_span(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(open) = self.open.take() else {
            return tx.programming_error("no open dynamic span");
        };
        // Resist diminishing to silence.
        let min_target = self.equalize_volume(tx, 0.1)?;
        let max_target = self.equalize_volume(tx, AudioSpanDynamic::MAXIMUM_VOLUME)?;
        let now = tx.now();
        change(tx, open.record, |s| s.set_end_moment(now))?;
        let duration = span(tx, open.record).duration;
        let q = if self.state == State::Return {
            &mut self.ret
        } else {
            &mut self.depart
        };
        if open.grow != CENTER {
            q.change_duration += duration;
        }
        q.min_target = min_target;
        q.max_target = max_target;
        q.spans.push(open);
        Ok(())
    }

    /// `DynamicQueue::set_volume`: each span of `queue` from its
    /// predecessor's target, each (de)crescendo given the change in
    /// volume by its share of the queue's time spent changing.
    fn set_volume(tx: &mut Tx<'_>, queue: &Queue, start: f64, target: f64) -> Run<()> {
        let gain = target - start;
        let mut dur = 0.0;
        let mut vol = start;
        for s in &queue.spans {
            let prev = vol;
            if s.grow != CENTER {
                dur += span(tx, s.record).duration;
                vol = libm::fma(gain, dur / queue.change_duration, start);
            }
            change(tx, s.record, |d| d.set_volume(prev, vol))?;
        }
        Ok(())
    }

    /// `calc_departure_volume`: a volume reasonably far from `start` and
    /// `end` in the direction `depart`, within `min` and `max`, the peak
    /// of a change and its return (or a single change's target).
    fn calc_departure_volume(depart: Dir, start: f64, end: f64, min_vol: f64, max_vol: f64) -> f64 {
        if depart == CENTER {
            return start;
        }
        const FAR_PADDING: f64 = 0.25;
        const NEAR_PADDING: f64 = 0.07;
        // An end already past the limits is taken as it is.
        let min_vol = min(min(min_vol, start), end);
        let max_vol = max(max(max_vol, start), end);
        let range = max_vol - min_vol;
        let d = f64::from(depart);
        let near = libm::fma(d * NEAR_PADDING, range, minmax(depart, start, end));
        let far = libm::fma(d * FAR_PADDING, range, minmax(-depart, start, end));
        max(min(minmax(depart, near, far), max_vol), min_vol)
    }

    /// `finish_queued_spans`: the queues' volumes, towards `next` (none if
    /// negative, a target then chosen by the direction); the volume they
    /// end at.
    fn finish_queued_spans(&mut self, tx: &mut Tx<'_>, next: f64) -> Run<f64> {
        let Some(first) = self.depart.spans.first() else {
            tx.programming_error("no dynamic span to finish")?;
            return Ok(next);
        };
        let start = span(tx, first.record).start_volume;
        if self.ret.spans.is_empty() {
            // A target not given, or against the direction, chosen.
            let mut depart_vol = next;
            if next < 0.0 || self.depart_dir != direction(next - start) {
                depart_vol = Self::calc_departure_volume(
                    self.depart_dir,
                    start,
                    start,
                    self.depart.min_target,
                    self.depart.max_target,
                );
            }
            Self::set_volume(tx, &self.depart, start, depart_vol)?;
            self.depart.clear();
            Ok(if next >= 0.0 { next } else { depart_vol })
        } else {
            // With no next dynamic, back to the start's volume.
            let return_vol = if next >= 0.0 { next } else { start };
            let depart_vol = Self::calc_departure_volume(
                self.depart_dir,
                start,
                return_vol,
                self.depart.min_target,
                self.depart.max_target,
            );
            Self::set_volume(tx, &self.depart, start, depart_vol)?;
            self.depart.clear();
            Self::set_volume(tx, &self.ret, depart_vol, return_vol)?;
            self.ret.clear();
            Ok(return_vol)
        }
    }

    /// `equalize_volume`: `volume` within `midiMinimumVolume` and
    /// `midiMaximumVolume` if either is set, else within the instrument's
    /// range by `instrumentEqualizer`, and then within 0 and 1.
    fn equalize_volume(&self, tx: &mut Tx<'_>, mut volume: f64) -> Run<f64> {
        let min_v = tx.get("midiMinimumVolume");
        let max_v = tx.get("midiMaximumVolume");
        let number = |tx: &mut Tx<'_>, v: Value| {
            let heap = tx.heap();
            if heap.is_number(v) {
                heap.to_f64(v)
            } else {
                None
            }
        };
        let (lo, hi) = (number(tx, min_v), number(tx, max_v));
        if lo.is_some() || hi.is_some() {
            let down = lo.unwrap_or(AudioSpanDynamic::MINIMUM_VOLUME);
            let up = hi.unwrap_or(AudioSpanDynamic::MAXIMUM_VOLUME);
            volume = libm::fma(length(down, up), volume, down);
        } else {
            let is_string = |tx: &mut Tx<'_>, v: Value| tx.heap().str(v).is_some();
            let mut s = tx.get("midiInstrument");
            if !is_string(tx, s) {
                s = tx.get("instrumentName");
            }
            if !is_string(tx, s) {
                s = tx.heap().string("piano");
            }
            let eq = tx.get("instrumentEqualizer");
            if tx.heap().is_procedure(eq) {
                s = tx.apply(eq, &[s])?;
            }
            let heap = tx.heap();
            if let (Some(a), Some(b)) = (heap.car(s), heap.cdr(s))
                && let (Some(down), Some(up)) = (number(tx, a), number(tx, b))
            {
                volume = libm::fma(length(down, up), volume, down);
            }
        }
        Ok(max(
            min(volume, AudioSpanDynamic::MAXIMUM_VOLUME),
            AudioSpanDynamic::MINIMUM_VOLUME,
        ))
    }

    /// `look_up_absolute_volume`: `dynamicAbsoluteVolumeFunction`'s volume
    /// of the dynamic's text, `default` if it gives no number.
    fn look_up_absolute_volume(&self, tx: &mut Tx<'_>, text: Value, default: f64) -> Run<f64> {
        let proc = tx.get("dynamicAbsoluteVolumeFunction");
        if !tx.heap().is_procedure(proc) {
            return Ok(default);
        }
        let v = tx.apply(proc, &[text])?;
        let heap = tx.heap();
        Ok(if heap.is_number(v) {
            heap.to_f64(v).unwrap_or(default)
        } else {
            default
        })
    }
}
