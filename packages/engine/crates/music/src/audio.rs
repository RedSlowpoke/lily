//! LilyPond's audio elements (`audio-element.cc`, `audio-element-info.cc`,
//! `audio-column.cc`, `audio-item.cc`, `audio-staff.cc`; 07 §7's M2, MS5
//! step 1): what performers announce, kept in their translation's audio
//! records ([`crate::AudioRecord::audio`]) and written by `crates/smf`
//! once the performance is over (`performance.rs`, `midi_item.rs`).
//!
//! - An **audio item** (`Audio_item`) is a note, a text, a tempo, a key, a
//!   time signature, an instrument, a control change or a pedal (MS5 step
//!   3 the last five): its record holds its column, which
//!   `Score_performer` gives every item announced to it in the timestep
//!   (`translator.rs`), and its channel,
//!   which `Staff_performer` gives the items of its staves.
//! - A note tied to the one before it (`Tie_performer`, MS5 step 4) gives
//!   that note's first of the tie its length and the time between them,
//!   and has none.
//! - An **audio column** (`Audio_column`) is a moment, the timestep's, offset
//!   by `midiSkipOffset`: a number in the performance's columns.
//! - An **audio staff** (`Audio_staff`) is a track: its items, by record,
//!   in the order they were added; the moment it ends; whether it merges
//!   unisons and is percussion; its name. The control track is one.
//!
//! - An **`Audio_span_dynamic`** (MS5 step 2) is an element, not an item:
//!   `Dynamic_performer`'s span of one volume or of a change of it, which
//!   the notes of its voice played while it was open point to, and which
//!   gives `Midi_note` their velocity.
//!
//! A record a performer makes with [`crate::Tx::make_audio`] is
//! [`Audio::Other`], no item.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::Rows;

use crate::rational::{Moment, Rational};
use crate::records::AudioRecord;

/// What an audio record is.
#[derive(Clone, Debug, Default)]
pub enum Audio {
    /// A performer's own record, no item of the performance's.
    #[default]
    Other,
    Staff(AudioStaff),
    Note(AudioNote),
    Text(AudioText),
    /// `Audio_tempo`: wholes a minute.
    Tempo(Rational),
    SpanDynamic(AudioSpanDynamic),
    /// `Audio_key`: sharps less flats, and whether it is major.
    Key {
        accidentals: i32,
        major: bool,
    },
    TimeSignature(AudioTimeSignature),
    Instrument(AudioInstrument),
    /// `Audio_control_change`: a controller and its value.
    ControlChange {
        control: u8,
        value: u8,
    },
    /// `Audio_piano_pedal`: its `Pedal_type` and whether it goes down.
    PianoPedal {
        pedal: u8,
        start: bool,
    },
}

impl Audio {
    /// `class_name`, the record's name.
    pub fn name(&self) -> &'static str {
        match self {
            Audio::Other => "Audio_element",
            Audio::Staff(s) if s.control => "Audio_control_track_staff",
            Audio::Staff(_) => "Audio_staff",
            Audio::Note(_) => "Audio_note",
            Audio::Text(_) => "Audio_text",
            Audio::Tempo(_) => "Audio_tempo",
            Audio::SpanDynamic(_) => "Audio_span_dynamic",
            Audio::Key { .. } => "Audio_key",
            Audio::TimeSignature(_) => "Audio_time_signature",
            Audio::Instrument(_) => "Audio_instrument",
            Audio::ControlChange { .. } => "Audio_control_change",
            Audio::PianoPedal { .. } => "Audio_piano_pedal",
        }
    }

    /// Whether it is an `Audio_item`, which has a column and a channel.
    pub fn is_item(&self) -> bool {
        !matches!(self, Audio::Other | Audio::Staff(_) | Audio::SpanDynamic(_))
    }
}

/// `Audio_staff`.
#[derive(Clone, Debug, Default)]
pub struct AudioStaff {
    /// Its items' records, as they were added.
    pub items: Vec<u32>,
    pub end: Moment,
    pub percussion: bool,
    pub merge_unisons: bool,
    pub track_name: String,
    /// `Audio_control_track_staff`: `Control_track_performer`'s.
    pub control: bool,
}

/// `Audio_note`. Its pitch is kept as `Midi_note` reads it: the tones of
/// the pitch and of the instrument's transposition, added.
#[derive(Clone, Debug)]
pub struct AudioNote {
    pub tones: Rational,
    pub length: Moment,
    /// `dynamic_`: its span, an audio record while the translation runs
    /// and the performance's span of that number once it is over
    /// (`performance.rs`); none if no `Dynamic_performer` gave it one.
    pub dynamic: Option<u32>,
    pub extra_velocity: i32,
    /// `tied_`: the note it is tied to (`tie_to`), its record.
    pub tied: Option<u32>,
    pub tie_event: bool,
}

/// `Audio_text`: its type (`Audio_text::Type`, the meta event's) and text.
#[derive(Clone, Debug)]
pub struct AudioText {
    pub kind: u8,
    pub text: String,
}

/// `Audio_text::Type`.
pub const TEXT: u8 = 1;
pub const TRACK_NAME: u8 = 3;
pub const INSTRUMENT_NAME: u8 = 4;
pub const LYRIC: u8 = 5;
pub const MARKER: u8 = 6;

/// `Audio_time_signature`: the fraction `time-signature->fraction` gave,
/// and the MIDI clocks a metronome click.
#[derive(Clone, Copy, Debug)]
pub struct AudioTimeSignature {
    pub num: Rational,
    pub den: Rational,
    pub beat_base_clocks: i32,
}

/// `Audio_instrument`, as `Midi_instrument` writes it: its name in lower
/// case and `midi-program`'s program of it, looked up when the staff
/// performer made it, `None` for none.
#[derive(Clone, Debug)]
pub struct AudioInstrument {
    pub name: String,
    pub program: Option<u8>,
}

/// `Pedal_type`, in its order.
pub const SOSTENUTO: u8 = 0;
pub const SUSTAIN: u8 = 1;
pub const UNA_CORDA: u8 = 2;

/// `Audio_span_dynamic`: a volume from its start, growing by its gain
/// towards the end it is closed at, which the next span's start may cut
/// short. Volumes are 0 to 1.
#[derive(Clone, Copy, Debug)]
pub struct AudioSpanDynamic {
    pub start: Moment,
    pub start_volume: f64,
    /// `duration_`: the end less the start, `moment_to_real`.
    pub duration: f64,
    /// `gain_`: the target volume less the start's.
    pub gain: f64,
}

impl AudioSpanDynamic {
    pub const MINIMUM_VOLUME: f64 = 0.0;
    pub const MAXIMUM_VOLUME: f64 = 1.0;
    pub const DEFAULT_VOLUME: f64 = 90.0 / 127.0;

    /// A span of `volume` from `start`, of no duration yet.
    pub fn new(start: Moment, volume: f64) -> (AudioSpanDynamic, Vec<String>) {
        let mut span = AudioSpanDynamic {
            start,
            start_volume: 0.0,
            duration: 0.0,
            gain: 0.0,
        };
        let errors = span.set_volume(volume, volume);
        (span, errors)
    }

    /// `set_end_moment`: closed at `end`, no earlier than its start; the
    /// programming error if it was.
    pub fn set_end_moment(&mut self, mut end: Moment) -> Option<String> {
        let mut error = None;
        if end.compare(self.start).is_lt() {
            let (mut e, mut s) = (String::new(), String::new());
            end.write(&mut e);
            self.start.write(&mut s);
            error = Some(format!("end moment ({e}) < start moment ({s})"));
            end = self.start;
        }
        self.duration = moment_to_real(end.checked_sub(self.start).unwrap_or_default());
        error
    }

    /// `set_volume`: from `start` towards `target`, each checked as
    /// LilyPond checks them, with its programming errors.
    pub fn set_volume(&mut self, mut start: f64, mut target: f64) -> Vec<String> {
        let mut errors = Vec::new();
        if start.is_nan() || start < 0.0 {
            errors.push(format!("invalid start volume: {start:.6}"));
            start = Self::DEFAULT_VOLUME;
        }
        if target.is_nan() || target < 0.0 {
            errors.push(format!("invalid target volume: {target:.6}"));
            target = start;
        }
        self.start_volume = start;
        self.gain = target - start;
        errors
    }

    /// `get_volume`: the volume at `when`, the start's before the span and
    /// the target's after it, each with its programming error; fused as
    /// clang fuses `start_volume_ + gain_ * (when / duration_)`.
    pub fn get_volume(&self, when: Moment) -> (f64, Option<String>) {
        let at = moment_to_real(when.checked_sub(self.start).unwrap_or_default());
        let error = |sign: &str| {
            let mut start = String::new();
            self.start.write(&mut start);
            format!(
                "asked to compute volume at {sign}{at:.6} for dynamic span of duration {:.6} starting at {start}",
                self.duration
            )
        };
        if at <= 0.0 {
            return (self.start_volume, (at < 0.0).then(|| error("")));
        }
        if at >= self.duration {
            return (self.start_volume + self.gain, Some(error("+")));
        }
        (
            libm::fma(self.gain, at / self.duration, self.start_volume),
            None,
        )
    }
}

/// `Audio_note::tie_to`: the note `note` tied to the note `to`, the first
/// of the tie given the time to `note`'s start (`skip`) and `note`'s
/// length, `note` none: it is not written (MS5 step 4).
pub fn tie_to(audio: &mut Rows<AudioRecord>, note: u32, to: u32, skip: Moment) {
    let length = match &mut audio[note as usize].audio {
        Audio::Note(n) => {
            n.tied = Some(to);
            core::mem::take(&mut n.length)
        }
        _ => return,
    };
    let head = tie_head(audio, note);
    if let Audio::Note(h) = &mut audio[head as usize].audio
        && let Some(m) = skip
            .checked_add(length)
            .and_then(|m| h.length.checked_add(m))
    {
        h.length = m;
    }
}

/// `Audio_note::tie_head`: the first note of the tie `note` is in.
pub fn tie_head(audio: &Rows<AudioRecord>, mut note: u32) -> u32 {
    while let Audio::Note(AudioNote { tied: Some(t), .. }) = audio[note as usize].audio {
        note = t;
    }
    note
}

/// `moment_to_real`: the main part and 9/40 of the grace part, in wholes.
pub fn moment_to_real(m: Moment) -> f64 {
    m.grace
        .checked_mul(Rational::new(9, 40))
        .and_then(|g| m.main.checked_add(g))
        .map_or_else(
            || m.main.to_f64() + 9.0 / 40.0 * m.grace.to_f64(),
            Rational::to_f64,
        )
}

/// `moment_to_ticks`: 384 ticks a quarter, truncated as C++'s `int ()`.
pub fn moment_to_ticks(m: Moment) -> i32 {
    (moment_to_real(m) * 384.0 * 4.0) as i32
}
