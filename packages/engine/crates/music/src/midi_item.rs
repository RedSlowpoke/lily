//! `midi-item.cc`'s conversions (07 §7's M2, MS5 step 1): each audio item
//! as `Midi_item::get_midi` turns it into an event, which `crates/smf`
//! writes as `Midi_walker` does. A note stays a note, which the writer
//! starts and stops; any other item is its event's bytes.
//!
//! A note's velocity is `Midi_note`'s, from its dynamic span (MS5 step 2),
//! and a microtone's pitch bend its `get_fine_tuning`; keys, time
//! signatures, programs, controllers and pedals are MS5 step 3's.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::audio::{Audio, AudioSpanDynamic, AudioTimeSignature, SOSTENUTO, SUSTAIN};
use crate::rational::{Moment, Rational};

/// What an item is written as.
pub(crate) enum Midi {
    Note {
        channel: u8,
        key: u8,
        velocity: u8,
        bend: u16,
    },
    Raw(Vec<u8>),
}

/// `Midi_item::get_midi` of the item `audio` of the column at `when` on
/// `channel`, with `Midi_walker::get_midi`'s channel 10 for a channel item
/// of a staff of percussion; `None` for none. A note's dynamic is its
/// number in `dynamics`. What it reports is pushed onto `diagnostics`, with
/// its kind, as `Midi_item::to_string` reports it when the track is written.
pub(crate) fn get_midi(
    audio: &Audio,
    when: Moment,
    channel: u8,
    percussion: bool,
    dynamics: &[AudioSpanDynamic],
    diagnostics: &mut Vec<(&'static str, String)>,
) -> Option<Midi> {
    let channel = if percussion { 9 } else { channel };
    match audio {
        Audio::Note(n) => {
            // `Midi_note::Midi_note`: the volume at the column, or 0x5a
            // with no dynamic, and the extra velocity, a double converted
            // as LilyPond's arm64 binary converts it to `uint8_t`
            // (`fcvtzs`, then an unsigned compare with 0x7f): what is not
            // 0 to 0x7e is 0x7f.
            let volume = match n.dynamic.and_then(|d| dynamics.get(d as usize)) {
                Some(span) => {
                    let (volume, error) = span.get_volume(when);
                    if let Some(e) = error {
                        diagnostics.push(("programming-error", e));
                    }
                    volume * f64::from(0x7f)
                }
                None => f64::from(0x5a),
            };
            let velocity = (volume + f64::from(n.extra_velocity)) as i32;
            let semitones = semitone_pitch(n.tones);
            Some(Midi::Note {
                channel,
                key: (semitones + C0_PITCH) as u8,
                velocity: if (velocity as u32) < 0x7f {
                    velocity as u8
                } else {
                    0x7f
                },
                bend: (i32::from(lily_engine_smf::CENTER) + fine_tuning(n.tones, semitones)) as u16,
            })
        }
        Audio::Text(t) => Some(Midi::Raw(text(t.kind, &t.text))),
        Audio::Tempo(wpm) => Some(Midi::Raw(tempo(*wpm, diagnostics))),
        Audio::Key { accidentals, major } => Some(Midi::Raw(alloc::vec![
            0xff,
            0x59,
            0x02,
            *accidentals as u8,
            u8::from(!major),
        ])),
        Audio::TimeSignature(t) => Some(Midi::Raw(time_signature(t, diagnostics))),
        // `Midi_instrument::to_string`: program 0 for a name `midi-program`
        // does not know, with its warning.
        Audio::Instrument(i) if i.name.is_empty() => None,
        Audio::Instrument(i) => {
            if i.program.is_none() {
                diagnostics.push(("warning", format!("no such MIDI instrument: `{}'", i.name)));
            }
            Some(Midi::Raw(alloc::vec![
                0xc0u8.wrapping_add(channel),
                i.program.unwrap_or(0)
            ]))
        }
        Audio::ControlChange { control, value } => Some(Midi::Raw(alloc::vec![
            0xb0u8.wrapping_add(channel),
            *control,
            *value
        ])),
        Audio::PianoPedal { pedal, start } => {
            let control = match *pedal {
                SOSTENUTO => 0x42,
                SUSTAIN => 0x40,
                _ => 0x43,
            };
            Some(Midi::Raw(alloc::vec![
                0xb0u8.wrapping_add(channel),
                control,
                if *start { 0x7f } else { 0 }
            ]))
        }
        Audio::Other | Audio::Staff(_) | Audio::SpanDynamic(_) => None,
    }
}

/// `Midi_note::c0_pitch_`.
const C0_PITCH: i32 = 60;

/// `Midi_note::get_semitone_pitch`: the tones in semitones, rounded to the
/// nearest, a half to even (`rint`).
fn semitone_pitch(tones: Rational) -> i32 {
    let semitones = tones
        .checked_mul(Rational::int(2))
        .map_or(tones.to_f64() * 2.0, Rational::to_f64);
    libm::rint(semitones) as i32
}

/// `Midi_note::get_fine_tuning`: the tones' distance from `semitones`, in
/// the pitch wheel's 0x1000 a semitone, truncated.
fn fine_tuning(tones: Rational, semitones: i32) -> i32 {
    let exact = tones
        .checked_mul(Rational::int(2))
        .and_then(|t| t.checked_sub(Rational::int(semitones.into())))
        .and_then(|t| t.checked_mul(Rational::int(0x1000)));
    match exact {
        Some(t) => t.to_f64() as i32,
        None => ((tones.to_f64() * 2.0 - f64::from(semitones)) * f64::from(0x1000)) as i32,
    }
}

/// `Midi_text::to_string`: a meta event of the text's type.
fn text(kind: u8, text: &str) -> Vec<u8> {
    let mut out = alloc::vec![0xff, kind];
    lily_engine_smf::variable(text.len() as u32, &mut out);
    out.extend_from_slice(text.as_bytes());
    out
}

/// `Midi_tempo::to_string`: microseconds a quarter, truncated, from 1 to
/// 0xffffff with LilyPond's warning outside them.
fn tempo(wpm: Rational, diagnostics: &mut Vec<(&'static str, String)>) -> Vec<u8> {
    const MIN: i64 = 1;
    const MAX: i64 = 0xff_ffff;
    let per_quarter = wpm
        .checked_mul(Rational::int(4))
        .and_then(|q| Rational::int(60_000_000).checked_div(q))
        .filter(|r| r.is_finite())
        .map_or(0, |r| r.numerator() / r.denominator());
    let mut us = per_quarter;
    if !(MIN..=MAX).contains(&us) {
        us = us.clamp(MIN, MAX);
        let mut text = String::new();
        wpm.write(&mut text);
        diagnostics.push((
            "warning",
            format!("Unsupported MIDI tempo (wholes/minute): {text}"),
        ));
    }
    let be = (us as u32).to_be_bytes();
    alloc::vec![0xff, 0x51, 0x03, be[1], be[2], be[3]]
}

/// `Midi_time_signature::to_string`: the fraction as a power of two below
/// a number of at most 255, reduced if it must be, else with the other
/// factors of its denominator dropped; nothing (an event of no bytes, as
/// LilyPond writes it) for what cannot be written, with the warning.
fn time_signature(
    t: &AudioTimeSignature,
    diagnostics: &mut Vec<(&'static str, String)>,
) -> Vec<u8> {
    let mut warned = false;
    let mut warn = |diagnostics: &mut Vec<(&'static str, String)>| {
        if !core::mem::replace(&mut warned, true) {
            let (mut n, mut d) = (String::new(), String::new());
            t.num.write(&mut n);
            t.den.write(&mut d);
            diagnostics.push((
                "warning",
                format!("Unsupported MIDI time signature: ({n})/({d})"),
            ));
        }
    };
    let (mut num, mut den) = (t.num, t.den);
    if !num.is_finite()
        || num.is_negative()
        || !den.is_finite()
        || den.compare(Rational::ZERO).is_le()
    {
        warn(diagnostics);
        return Vec::new();
    }
    let mut midi_dlog = 0x100;
    for _ in 0..2 {
        if num.denominator() == 1 {
            let dlog = intlog2(den.numerator());
            if den.numerator() == 1 << dlog
                && let Some(n) = num.checked_mul(Rational::int(den.denominator()))
                && n.compare(Rational::int(0xff)).is_le()
            {
                num = n;
                midi_dlog = dlog;
                break;
            }
        }
        num = num.checked_div(den).unwrap_or(num);
        den = Rational::int(num.denominator());
        num = Rational::int(num.numerator());
    }
    if midi_dlog > 0xff {
        warn(diagnostics);
        midi_dlog = intlog2(den.numerator());
    }
    if num.numerator() > 0xff || midi_dlog > 0xff {
        warn(diagnostics);
        return Vec::new();
    }
    alloc::vec![
        0xff,
        0x58,
        0x04,
        num.numerator() as u8,
        midi_dlog as u8,
        t.beat_base_clocks as u8,
        0x08,
    ]
}

/// `intlog2`: the position of the highest bit of a positive number.
fn intlog2(d: i64) -> i64 {
    63 - i64::from(d.leading_zeros())
}
