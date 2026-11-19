//! M4's performers (07 §7, §3.2, §3.6; MS5 step 3): `Key_performer`
//! (`key-performer.cc`), `Time_signature_performer`
//! (`time-signature-performer.cc`), `Midi_control_change_performer`
//! (`midi-cc-performer.cc`) and `Piano_pedal_performer`
//! (`piano-pedal-performer.cc`), with `Midi_control_change_announcer`
//! (`midi-cc-announcer.cc`), which `Staff_performer`'s control initializer
//! shares (`performers.rs`). Their items are `audio.rs`', written as
//! `midi_item.rs` writes them.
//!
//! - A key is announced in the timestep of its `\key`, transposed by
//!   `instrumentTransposition`: its sharps less its flats
//!   (`alterations-in-key`), and minor if its third above the tonic is
//!   flat, else major.
//! - A time signature is announced when `timeSignature`'s fraction
//!   (`time-signature->fraction`) changes, or at a `\time`, with the MIDI
//!   clocks of `beatBase` times the greatest common divisor of
//!   `beatStructure`; the Score's control track takes it.
//! - A control change is announced at each `\set` of one of the five
//!   controller properties heard below the Staff, the value scaled into
//!   seven bits or fourteen, the latter as two changes.
//! - A pedal is announced at its start and at its end.

use alloc::boxed::Box;
use alloc::format;

use lily_engine_scheme::{Exception, Tag, Value};

use crate::audio::{Audio, AudioTimeSignature, SOSTENUTO, SUSTAIN, UNA_CORDA};
use crate::dispatcher::event_property;
use crate::rational::Rational;
use crate::records::Cause;
use crate::translator::{PROCESS, START, STOP, TranslatorClass, Tx, Work};

type Run<T> = Result<T, Exception>;

// --- Midi_control_change_announcer -----------------------------------------------

/// `controls_`: each controller's property, the property's range, and its
/// controller numbers, the second for the low seven bits of one of fourteen.
const CONTROLS: [(&str, f64, f64, u8, Option<u8>); 5] = [
    ("midiBalance", -1.0, 1.0, 8, Some(40)),
    ("midiPanPosition", -1.0, 1.0, 10, Some(42)),
    ("midiExpression", 0.0, 1.0, 11, Some(43)),
    ("midiReverbLevel", 0.0, 1.0, 91, None),
    ("midiChorusLevel", 0.0, 1.0, 93, None),
];

/// `announce_control_changes`: the control changes of each controller
/// whose property `value` gives a number for, given to `announce`; one out
/// of its range warned of, at the event `origin` if there is one.
pub(crate) fn announce_control_changes(
    tx: &mut Tx<'_>,
    origin: Option<Value>,
    value: impl Fn(&mut Tx<'_>, &str) -> Value,
    mut announce: impl FnMut(&mut Tx<'_>, u8, u8),
) -> Run<()> {
    for (name, min, max, msb, lsb) in CONTROLS {
        let v = value(tx, name);
        let Some(val) = Some(v)
            .filter(|&v| tx.heap().is_number(v))
            .and_then(|v| tx.heap().to_f64(v))
        else {
            continue;
        };
        if val >= min && val <= max {
            // Normalized to 0 to 1, then to fourteen bits or seven,
            // `round_halfway_up` as LilyPond's binary computes it.
            let val = (val - min) / (max - min);
            let scale = if lsb.is_some() { 0x3fff } else { 0x7f };
            let v = (libm::floor(val * f64::from(scale) - 0.5) + 1.0) as i32;
            match lsb {
                Some(lsb) => {
                    announce(tx, msb, (v >> 7) as u8);
                    announce(tx, lsb, (v & 0x7f) as u8);
                }
                None => announce(tx, msb, v as u8),
            }
        } else {
            let text = format!("ignoring out-of-range value change for MIDI property `{name}'");
            match origin {
                Some(ev) => tx.event_warning(ev, &text)?,
                None => tx.warning(&text)?,
            }
        }
    }
    Ok(())
}

// --- Midi_control_change_performer ---------------------------------------------

/// It hears the `SetProperty` events of its context and those below it
/// (`connect_to_context`'s listener on `events_below`).
pub(crate) const MIDI_CONTROL_CHANGE_PERFORMER: TranslatorClass = crate::performers::performer(
    "Midi_control_change_performer",
    0,
    false,
    &["SetProperty"],
    || Box::new(MidiControlChangePerformer),
    &[
        "midiBalance",
        "midiPanPosition",
        "midiExpression",
        "midiReverbLevel",
        "midiChorusLevel",
    ],
);

#[derive(Clone)]
struct MidiControlChangePerformer;

impl Work for MidiControlChangePerformer {
    /// `announce_control_change`: the event's value, if it sets one of the
    /// controllers' properties.
    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        let sym = event_property(tx.heap(), ev, "symbol");
        if sym.tag() != Tag::Symbol {
            return Ok(());
        }
        let value = event_property(tx.heap(), ev, "value");
        announce_control_changes(
            tx,
            Some(ev),
            |tx, name| {
                if tx.heap().intern(name) == sym {
                    value
                } else {
                    Value::NULL
                }
            },
            |tx, control, value| {
                tx.announce(Audio::ControlChange { control, value }, Cause::None);
            },
        )
    }
}

// --- Key_performer ---------------------------------------------------------------

pub(crate) const KEY_PERFORMER: TranslatorClass = crate::performers::performer(
    "Key_performer",
    PROCESS,
    // As LilyPond's, every timestep: a key heard in one that
    // `skipTypesetting` skipped is announced at the next one processed.
    true,
    &["key-change-event"],
    || Box::new(KeyPerformer { event: None }),
    &["instrumentTransposition"],
);

#[derive(Clone)]
struct KeyPerformer {
    event: Option<Value>,
}

impl Work for KeyPerformer {
    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.event.get_or_insert(ev);
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(ev) = self.event else {
            return Ok(());
        };
        let depth = tx.heap().root_depth();
        let key = Self::key(tx, ev);
        tx.heap().unroot_to(depth);
        if let Some((accidentals, major)) = key? {
            tx.announce(Audio::Key { accidentals, major }, Cause::Event(ev));
        }
        self.event = None;
        Ok(())
    }

    fn values(&self, out: &mut alloc::vec::Vec<Value>) {
        out.extend(self.event);
    }
}

impl KeyPerformer {
    /// The key of the event `ev`: its alterations' count, and whether it
    /// is major; `None` for a key of no pitches. What it makes is rooted.
    fn key(tx: &mut Tx<'_>, ev: Value) -> Run<Option<(i32, bool)>> {
        let mut pitches = event_property(tx.heap(), ev, "pitch-alist");
        let tp = tx.get("instrumentTransposition");
        if tx.heap().foreign::<crate::pitch::Pitch>(tp).is_some() {
            pitches = tx.call("ly:transpose-key-alist", &[pitches, tp])?;
            tx.heap().root(pitches);
        }
        let acc = tx.call("alterations-in-key", &[pitches])?;
        let acc = tx.heap().to_i64(acc).unwrap_or(0) as i32;
        let heap = tx.heap();
        let Some((step, alter)) = heap
            .car(pitches)
            .and_then(|first| Some((heap.car(first)?, heap.cdr(first)?)))
        else {
            return Ok(None);
        };
        // The key moved to C: minor if its third is flat.
        let octave = tx.heap().integer(0);
        let tonic = tx.call("ly:make-pitch", &[octave, step, alter])?;
        tx.heap().root(tonic);
        let negated = tx.call("ly:pitch-negate", &[tonic])?;
        tx.heap().root(negated);
        let in_c = tx.call("ly:transpose-key-alist", &[pitches, negated])?;
        let heap = tx.heap();
        let mut third = None;
        let mut p = in_c;
        while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            if heap.car(entry).and_then(|k| heap.to_i64(k)) == Some(2) {
                third = heap.cdr(entry);
                break;
            }
            p = rest;
        }
        let minor = third
            .filter(|&t| heap.is_number(t))
            .and_then(|t| crate::rational_of(heap, t))
            .is_some_and(|t| t == Rational::new(-1, 2));
        Ok(Some((acc, !minor)))
    }
}

// --- Time_signature_performer ----------------------------------------------------

pub(crate) const TIME_SIGNATURE_PERFORMER: TranslatorClass = crate::performers::performer(
    "Time_signature_performer",
    PROCESS | STOP,
    true,
    &["reference-time-signature-event"],
    || {
        Box::new(TimeSignaturePerformer {
            announced: false,
            last: Value::FALSE,
            event: None,
        })
    },
    &["timeSignature"],
);

#[derive(Clone)]
struct TimeSignaturePerformer {
    /// `audio_`: whether one was announced in this timestep.
    announced: bool,
    /// `last_time_fraction_`.
    last: Value,
    event: Option<Value>,
}

impl Work for TimeSignaturePerformer {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.flags(&[self.announced]);
    }

    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.event = Some(ev);
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.announced {
            return Ok(());
        }
        let ts = tx.get("timeSignature");
        let fr = tx.call("time-signature->fraction", &[ts])?;
        let heap = tx.heap();
        let (Some(num), Some(den)) = (heap.car(fr), heap.cdr(fr)) else {
            return Ok(());
        };
        // A `\time` gives one even if it is the same.
        if self.event.is_none() && heap.equal(fr, self.last) {
            return Ok(());
        }
        let fraction = |v| crate::rational_of(heap, v).unwrap_or(Rational::NAN);
        let (num, den) = (fraction(num), fraction(den));
        heap.root(fr);
        self.last = fr;
        let base = tx.get("beatBase");
        let base = crate::rational_of(tx.heap(), base).unwrap_or(Rational::new(1, 4));
        let mut clocks = Rational::int(96).checked_mul(base);
        let structure = tx.get("beatStructure");
        if let Some(common) = calc_common_beat(tx.heap(), structure).filter(|c| *c != 0) {
            clocks = clocks.and_then(|c| c.checked_mul(Rational::int(common)));
        }
        let beat_base_clocks = match clocks {
            Some(c) if c.denominator() == 1 && (1..=255).contains(&c.numerator()) => {
                c.numerator() as i32
            }
            _ => {
                let text = "bad beatBase/beatStructure for MIDI time signature";
                match self.event {
                    Some(ev) => tx.event_warning(ev, text)?,
                    None => tx.warning(text)?,
                }
                // A quarter note, 24 MIDI clocks.
                24
            }
        };
        let audio = Audio::TimeSignature(AudioTimeSignature {
            num,
            den,
            beat_base_clocks,
        });
        tx.announce(audio, self.event.map_or(Cause::None, Cause::Event));
        self.announced = true;
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.announced = false;
        self.event = None;
        Ok(())
    }

    fn values(&self, out: &mut alloc::vec::Vec<Value>) {
        out.push(self.last);
        out.extend(self.event);
    }
}

/// `calc_common_beat`: the greatest common divisor of `beatStructure`'s
/// integers, 1 if one is not an integer; `None` where Guile's `gcd` gives
/// an inexact number, which is no rational.
fn calc_common_beat(heap: &lily_engine_scheme::Heap, structure: Value) -> Option<i64> {
    let (mut common, mut exact) = (0i64, true);
    let mut p = structure;
    while let (Some(beat), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        p = rest;
        if !heap.is_integer(beat) {
            return Some(1);
        }
        let n = match heap.to_i64(beat) {
            Some(n) => n,
            None => {
                exact = false;
                heap.to_f64(beat).unwrap_or(0.0) as i64
            }
        };
        common = gcd(common, n);
    }
    exact.then_some(common)
}

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a as i64
}

// --- Piano_pedal_performer -------------------------------------------------------

pub(crate) const PIANO_PEDAL_PERFORMER: TranslatorClass = crate::performers::performer(
    "Piano_pedal_performer",
    START | PROCESS,
    // Every timestep, so that what a skipped one heard is dropped.
    true,
    &["sostenuto-event", "sustain-event", "una-corda-event"],
    || Box::new(PianoPedalPerformer::default()),
    &[],
);

/// `Pedal_info`: the start heard last, and the timestep's start and stop.
#[derive(Clone, Copy, Default)]
struct PedalInfo {
    start_event: Option<Value>,
    start: Option<Value>,
    stop: Option<Value>,
}

#[derive(Clone, Default)]
struct PianoPedalPerformer {
    /// By `Pedal_type`, as the listeners are.
    info: [PedalInfo; 3],
}

impl Work for PianoPedalPerformer {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        for i in &self.info {
            h.flags(&[i.start_event.is_some(), i.start.is_some(), i.stop.is_some()]);
        }
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        let d = event_property(tx.heap(), ev, "span-direction");
        match d.as_fixnum() {
            Some(-1) => self.info[which].start = Some(ev),
            Some(1) => self.info[which].stop = Some(ev),
            _ => {}
        }
        Ok(())
    }

    fn start_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        for p in &mut self.info {
            p.start = None;
            p.stop = None;
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for (pedal, p) in [SOSTENUTO, SUSTAIN, UNA_CORDA]
            .into_iter()
            .zip(&mut self.info)
        {
            if let Some(stop) = p.stop {
                if p.start_event.is_none() {
                    tx.event_warning(stop, "cannot find start of piano pedal")?;
                } else {
                    let audio = Audio::PianoPedal {
                        pedal,
                        start: false,
                    };
                    tx.announce(audio, Cause::Event(stop));
                }
                p.start_event = None;
            }
            if let Some(start) = p.start {
                p.start_event = Some(start);
                let audio = Audio::PianoPedal { pedal, start: true };
                tx.announce(audio, Cause::Event(start));
            }
            p.start = None;
            p.stop = None;
        }
        Ok(())
    }

    fn values(&self, out: &mut alloc::vec::Vec<Value>) {
        for p in &self.info {
            out.extend(p.start_event.into_iter().chain(p.start).chain(p.stop));
        }
    }
}
