//! The song model (D166): what a Guitar Pro file says, whichever format it
//! came in. `gp345` and `gpx` fill it, `lilypond` reads it, and nothing in it
//! names a byte or an XML element. Where the formats count differently the
//! model keeps one count, and the readers convert (README §4):
//!
//! - bars, tracks and voices are indexed from 0, strings from 1, the first
//!   string being the highest, as in GP3–GP5;
//! - pitches are MIDI note numbers, 60 being middle C;
//! - positions and lengths are fractions of a whole note;
//! - a repeat's count is how many times its bars are played in all.
//!
//! The model keeps what a score can show or play; what only Guitar Pro's
//! player uses (RSE sounds, the mixer's effects, page layout) is left out.
use std::fmt;
use std::ops::Add;

use serde::{Deserialize, Serialize};

/// A song read from a Guitar Pro file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    /// The file's header as written (`FICHIER GUITAR PRO v5.10`), or
    /// `GPIF <revision>` for a `.gpx`.
    pub version: String,
    pub info: SongInfo,
    /// The tempo at the start, in quarter notes a minute.
    pub tempo: f64,
    /// The tempo's text, such as *Moderate* (GP5, GPX), else empty.
    pub tempo_name: String,
    /// Each change of tempo after the start, in order.
    pub tempo_changes: Vec<TempoChange>,
    /// One per bar of the song, in order; every track has a `Bar` for each.
    pub master_bars: Vec<MasterBar>,
    pub tracks: Vec<Track>,
}

/// The song's header: the file's own fields, empty where it has none.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongInfo {
    pub title: String,
    pub subtitle: String,
    pub artist: String,
    pub album: String,
    /// Who wrote the words. GP3 and GP4 have one field for words and music,
    /// which goes here, and `music` stays empty.
    pub words: String,
    pub music: String,
    pub copyright: String,
    /// Who made the tab (GP's *Tab* field, GPX's `Tabber`).
    pub transcriber: String,
    pub instructions: String,
    /// The notice's lines, in order.
    pub notices: Vec<String>,
}

/// A change of tempo at a place in the song.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TempoChange {
    /// The master bar, from 0.
    pub bar: usize,
    /// Where in the bar, from its start.
    pub offset: Fraction,
    /// Quarter notes a minute.
    pub bpm: f64,
}

/// A bar of the song, shared by every track: its meter, key, repeats and
/// marks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterBar {
    pub time: TimeSignature,
    pub key: KeySignature,
    /// A repeat begins at this bar.
    pub repeat_start: bool,
    /// A repeat ends at this bar, its bars played this many times in all
    /// (2 for a plain repeat). GP3 and GP4 store one less (README §2.9).
    pub repeat_end: Option<u8>,
    /// The alternate endings this bar belongs to, from 1, ascending: `[1, 2]`
    /// is played on the first and second times.
    pub alternate_endings: Vec<u8>,
    /// A rehearsal mark or section name (GP's marker, GPX's `Section`).
    pub marker: Option<String>,
    /// Segno, coda and the jumps to them.
    pub directions: Vec<Direction>,
    pub triplet_feel: TripletFeel,
    /// A double bar line ends this bar.
    pub double_bar: bool,
}

impl Default for MasterBar {
    fn default() -> Self {
        MasterBar {
            time: TimeSignature::default(),
            key: KeySignature::default(),
            repeat_start: false,
            repeat_end: None,
            alternate_endings: Vec::new(),
            marker: None,
            directions: Vec::new(),
            triplet_feel: TripletFeel::None,
            double_bar: false,
        }
    }
}

impl MasterBar {
    /// How long the bar is by its time signature.
    pub fn length(&self) -> Fraction {
        Fraction::new(self.time.numerator.into(), self.time.denominator.into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSignature {
    pub numerator: u8,
    /// 1, 2, 4, 8, 16 or 32.
    pub denominator: u8,
}

impl Default for TimeSignature {
    fn default() -> Self {
        TimeSignature {
            numerator: 4,
            denominator: 4,
        }
    }
}

/// A key as its count of sharps (above 0) or flats (below 0), and whether it
/// is minor: `{ fifths: -1, minor: false }` is F major.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySignature {
    /// −7 to 7.
    pub fifths: i8,
    pub minor: bool,
}

/// Eighths or sixteenths played long–short, as a bar or the song asks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TripletFeel {
    #[default]
    None,
    Eighth,
    Sixteenth,
}

/// A navigation mark: where a jump lands (`Segno`, `Coda`, …) or the jump
/// itself (`DaCapo`, `DalSegnoAlCoda`, …), as GP5's directions and GPX's
/// `Directions` name them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Segno,
    SegnoSegno,
    Coda,
    DoubleCoda,
    Fine,
    DaCapo,
    DaCapoAlCoda,
    DaCapoAlDoubleCoda,
    DaCapoAlFine,
    DalSegno,
    DalSegnoAlCoda,
    DalSegnoAlDoubleCoda,
    DalSegnoAlFine,
    DalSegnoSegno,
    DalSegnoSegnoAlCoda,
    DalSegnoSegnoAlDoubleCoda,
    DalSegnoSegnoAlFine,
    DaCoda,
    DaDoubleCoda,
    /// A name the reader does not know, as written.
    Other(String),
}

/// A part of the song: one instrument, its bars and how it is tuned.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub name: String,
    /// GPX's short name (`Lead`, `Drums`), else empty.
    pub short_name: String,
    pub kind: TrackKind,
    /// GPX's instrument (`e-gtr6`, `e-bass4`, `drmkt`), else empty.
    pub instrument: String,
    /// The open strings as MIDI pitches, the first string (the highest)
    /// first. A drum track keeps what the file has, often six zeros.
    pub tuning: Vec<u8>,
    /// The capo's fret, 0 for none. Frets are written above it, so a note
    /// sounds at `tuning + capo + fret` (README §2.10).
    pub capo: u8,
    /// How many frets the neck has (24 in most files; 87 or 99 on drums).
    pub frets: u8,
    pub midi: MidiChannel,
    /// The track's words, if the file gives it any.
    pub lyrics: Vec<LyricLine>,
    /// One per master bar, in order.
    pub bars: Vec<Bar>,
}

impl Track {
    /// The MIDI pitch a note sounds on this track, or `None` when its string
    /// is not one of the track's. On a drum track it is the note's
    /// percussion key.
    pub fn pitch(&self, note: &Note) -> Option<u8> {
        if self.kind == TrackKind::Drums {
            return Some(note.fret);
        }
        let open = *self.tuning.get(usize::from(note.string).checked_sub(1)?)?;
        u8::try_from(u32::from(open) + u32::from(self.capo) + u32::from(note.fret))
            .ok()
            .filter(|p| *p < 128)
    }
}

/// What a track is, which decides what the score gives it (D166).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackKind {
    /// A fretted string instrument (guitar, bass, banjo, …): a Staff and a
    /// TabStaff with the track's tuning.
    #[default]
    Strings,
    /// A drum kit: notes are General MIDI percussion keys.
    Drums,
    /// Any other instrument: a piano, voice or strings that Guitar Pro still
    /// stores as frets on strings (README §5.1). A Staff alone, its notes at
    /// the pitches the frets give.
    Other,
}

/// How the track plays: General MIDI's program on a channel, and the
/// channel's volume and pan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiChannel {
    /// From 0.
    pub port: u8,
    /// From 0; 9 is General MIDI's drums.
    pub channel: u8,
    /// The second channel GP gives a track for its effects (bends), from 0.
    pub effect_channel: u8,
    /// General MIDI's program, from 0 (24 is a nylon guitar).
    pub program: u8,
    pub bank: u8,
    /// 0 to 127.
    pub volume: u8,
    /// 0 to 127, 64 the middle.
    pub pan: u8,
}

impl Default for MidiChannel {
    fn default() -> Self {
        MidiChannel {
            port: 0,
            channel: 0,
            effect_channel: 1,
            program: 25,
            bank: 0,
            volume: 100,
            pan: 64,
        }
    }
}

/// A line of words, sung from a bar on, its syllables split by spaces and
/// hyphens as Guitar Pro takes them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    /// The master bar it starts at, from 0.
    pub start_bar: usize,
    pub text: String,
}

/// A track's bar: its voices, each of which fills the bar on its own.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bar {
    /// GP3 and GP4 have one voice, GP5 two, GPX up to four. A voice the file
    /// leaves out, or fills only with `BeatStatus::Empty`, plays nothing.
    pub voices: Vec<Voice>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    pub beats: Vec<Beat>,
}

impl Voice {
    /// Whether the voice plays or shows nothing: no beats, or only empty
    /// ones.
    pub fn is_empty(&self) -> bool {
        self.beats.iter().all(|b| b.status == BeatStatus::Empty)
    }
}

/// A moment in a voice: the notes struck together, or a rest, for a
/// duration.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Beat {
    pub duration: Duration,
    pub status: BeatStatus,
    /// The notes, at most one a string. Empty unless `status` is `Normal`.
    pub notes: Vec<Note>,
    /// Text above the staff.
    pub text: Option<String>,
    pub chord: Option<Chord>,
    pub effects: BeatEffects,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BeatStatus {
    /// Notes, or a rest when it has none.
    #[default]
    Normal,
    Rest,
    /// Takes its time but shows nothing: GP's filler in a voice not written
    /// (README §2.12). A writer makes it a spacer.
    Empty,
}

/// A written duration: a note value, its dots, and the tuplet it is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Duration {
    /// 1 (whole), 2, 4, 8, 16, 32, 64 or 128.
    pub value: u8,
    pub dots: u8,
    pub tuplet: Option<Tuplet>,
}

impl Default for Duration {
    fn default() -> Self {
        Duration {
            value: 4,
            dots: 0,
            tuplet: None,
        }
    }
}

impl Duration {
    /// How long it lasts, as a fraction of a whole note.
    pub fn length(&self) -> Fraction {
        let base = Fraction::new(1, self.value.max(1).into());
        let mut length = base;
        let mut dot = base;
        for _ in 0..self.dots {
            dot = Fraction::new(dot.num, dot.den * 2);
            length = length + dot;
        }
        match self.tuplet {
            Some(t) if t.notes > 0 => Fraction::new(
                length.num * u32::from(t.space),
                length.den * u32::from(t.notes),
            ),
            _ => length,
        }
    }
}

/// `notes` in the time of `space`: a triplet of eighths is 3 in the time of
/// 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tuplet {
    pub notes: u8,
    pub space: u8,
}

/// A chord's name, and its diagram where the file has one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chord {
    pub name: String,
    /// The diagram's frets, the first string first: `None` is not played,
    /// `Some(0)` open. Empty when there is no diagram.
    pub frets: Vec<Option<u8>>,
}

/// What a beat does with all its notes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeatEffects {
    /// The notes strummed, not struck together.
    pub stroke: Option<Stroke>,
    /// A pick stroke marked over the beat.
    pub pick_stroke: Option<StrokeDirection>,
    /// The whammy bar's curve, in semitones.
    pub tremolo_bar: Option<Bend>,
    pub slap: Option<Slap>,
    pub fade_in: bool,
    /// A wide vibrato on the whole beat (GP's beat vibrato).
    pub wide_vibrato: bool,
    pub rasgueado: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stroke {
    pub direction: StrokeDirection,
    /// How long the strum takes, as a note value (4 to 128); 0 when not
    /// said.
    pub value: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StrokeDirection {
    /// Towards the floor, from the lowest string.
    Down,
    Up,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Slap {
    Tap,
    Slap,
    Pop,
}

/// A note on a string.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    /// From 1, the highest string. On a drum track, what the file says.
    pub string: u8,
    /// The fret above the capo. On a drum track, the General MIDI percussion
    /// key (36 is the bass drum); `gpx` maps its drum elements to one.
    pub fret: u8,
    /// Held over from the note before it on this string: not struck again,
    /// and its fret is that note's.
    pub tie: bool,
    /// A muted string, drawn as an x.
    pub dead: bool,
    /// A ghost note, drawn in parentheses.
    pub ghost: bool,
    pub accent: Accent,
    pub dynamic: Dynamic,
    pub staccato: bool,
    pub palm_mute: bool,
    pub let_ring: bool,
    /// A hammer-on or pull-off to the next note on this string; which one
    /// follows from the frets.
    pub hammer: bool,
    pub slide: Option<Slide>,
    pub bend: Option<Bend>,
    pub harmonic: Option<Harmonic>,
    pub vibrato: bool,
    pub trill: Option<Trill>,
    /// Tremolo picking, as the note value of each stroke (8, 16 or 32).
    pub tremolo_picking: Option<u8>,
    /// A grace note before this one, on its string.
    pub grace: Option<Grace>,
}

impl Default for Note {
    fn default() -> Self {
        Note {
            string: 1,
            fret: 0,
            tie: false,
            dead: false,
            ghost: false,
            accent: Accent::None,
            dynamic: Dynamic::F,
            staccato: false,
            palm_mute: false,
            let_ring: false,
            hammer: false,
            slide: None,
            bend: None,
            harmonic: None,
            vibrato: false,
            trill: None,
            tremolo_picking: None,
            grace: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Accent {
    #[default]
    None,
    /// `>`.
    Normal,
    /// `^`, GP5's heavy accent.
    Heavy,
}

/// Guitar Pro's eight dynamics, `F` being its default (README §2.13).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Dynamic {
    Ppp,
    Pp,
    P,
    Mp,
    Mf,
    #[default]
    F,
    Ff,
    Fff,
}

impl Dynamic {
    /// GP3–GP5's step, 1 (ppp) to 8 (fff); a step out of range is the
    /// nearest.
    pub fn from_step(step: i32) -> Dynamic {
        match step {
            ..=1 => Dynamic::Ppp,
            2 => Dynamic::Pp,
            3 => Dynamic::P,
            4 => Dynamic::Mp,
            5 => Dynamic::Mf,
            6 => Dynamic::F,
            7 => Dynamic::Ff,
            _ => Dynamic::Fff,
        }
    }

    /// The MIDI velocity Guitar Pro plays it at (15 for ppp, 16 more a step).
    pub fn velocity(self) -> u8 {
        15 + 16 * self as u8
    }

    /// LilyPond's name for it, which is also GPX's in lower case.
    pub fn name(self) -> &'static str {
        ["ppp", "pp", "p", "mp", "mf", "f", "ff", "fff"][self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Slide {
    /// To the next note on the string, which is struck again.
    Shift,
    /// To the next note on the string, which is not struck again.
    Legato,
    OutDownwards,
    OutUpwards,
    IntoFromBelow,
    IntoFromAbove,
}

/// A bend or the whammy bar's curve: its points across the note.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bend {
    pub points: Vec<BendPoint>,
}

impl Bend {
    /// The point furthest from the note's own pitch, in semitones.
    pub fn peak(&self) -> f32 {
        self.points
            .iter()
            .map(|p| p.semitones)
            .fold(0.0, |a, b| if b.abs() > a.abs() { b } else { a })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BendPoint {
    /// Where in the note, from 0 (its start) to 1 (its end).
    pub position: f32,
    /// How far from the note's pitch, in semitones (2.0 is a full bend).
    pub semitones: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Harmonic {
    /// At the note's own fret.
    Natural,
    /// Fretted, and touched an octave or a fifth above.
    Artificial,
    /// Fretted, and tapped at `fret`.
    Tapped {
        fret: u8,
    },
    Pinch,
    Semi,
    Feedback,
}

/// A trill with `fret`, each stroke lasting a note value (16, 32 or 64).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trill {
    pub fret: u8,
    pub value: u8,
}

/// A grace note before a note: its fret on the same string, how long it
/// lasts, and how it leads into the note.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Grace {
    pub fret: u8,
    /// Its note value (16, 32 or 64).
    pub value: u8,
    pub dynamic: Dynamic,
    pub transition: GraceTransition,
    /// Played on the beat, taking time from the note, rather than before it.
    pub on_beat: bool,
    pub dead: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GraceTransition {
    #[default]
    None,
    Slide,
    Bend,
    Hammer,
}

/// A fraction in lowest terms, for positions and lengths in whole notes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fraction {
    pub num: u32,
    pub den: u32,
}

impl Fraction {
    pub const ZERO: Fraction = Fraction { num: 0, den: 1 };

    /// `num / den` in lowest terms; a zero `den` is taken as 1.
    pub fn new(num: u32, den: u32) -> Fraction {
        let den = den.max(1);
        let g = gcd(num, den);
        Fraction {
            num: num / g,
            den: den / g,
        }
    }
}

impl Default for Fraction {
    fn default() -> Self {
        Fraction::ZERO
    }
}

impl Add for Fraction {
    type Output = Fraction;

    fn add(self, other: Fraction) -> Fraction {
        let den = self.den / gcd(self.den, other.den) * other.den;
        Fraction::new(
            self.num * (den / self.den) + other.num * (den / other.den),
            den,
        )
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(&self, other: &Fraction) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Fraction {
    fn cmp(&self, other: &Fraction) -> std::cmp::Ordering {
        (u64::from(self.num) * u64::from(other.den))
            .cmp(&(u64::from(other.num) * u64::from(self.den)))
    }
}

impl fmt::Display for Fraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.num, self.den)
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_add_dots_and_tuplets() {
        let quarter = Duration::default();
        assert_eq!(quarter.length(), Fraction::new(1, 4));
        let dotted = Duration { dots: 1, ..quarter };
        assert_eq!(dotted.length(), Fraction::new(3, 8));
        let double = Duration {
            value: 2,
            dots: 2,
            tuplet: None,
        };
        assert_eq!(double.length(), Fraction::new(7, 8));
        let triplet = Duration {
            value: 8,
            dots: 0,
            tuplet: Some(Tuplet { notes: 3, space: 2 }),
        };
        assert_eq!(
            triplet.length() + triplet.length() + triplet.length(),
            Fraction::new(1, 4)
        );
    }

    #[test]
    fn fractions_compare_across_denominators() {
        assert!(Fraction::new(1, 3) < Fraction::new(3, 8));
        assert_eq!(Fraction::new(2, 4), Fraction::new(1, 2));
        assert_eq!(MasterBar::default().length(), Fraction::new(1, 1));
    }

    #[test]
    fn a_note_sounds_at_its_string_capo_and_fret() {
        let track = Track {
            tuning: vec![64, 59, 55, 50, 45, 40],
            capo: 2,
            ..Track::default()
        };
        let note = Note {
            string: 6,
            fret: 3,
            ..Note::default()
        };
        assert_eq!(track.pitch(&note), Some(45));
        assert_eq!(
            track.pitch(&Note {
                string: 7,
                ..note.clone()
            }),
            None
        );
        let drums = Track {
            kind: TrackKind::Drums,
            ..Track::default()
        };
        assert_eq!(drums.pitch(&Note { fret: 36, ..note }), Some(36));
    }

    #[test]
    fn dynamics_follow_guitar_pro_steps() {
        assert_eq!(Dynamic::from_step(6), Dynamic::F);
        assert_eq!(Dynamic::F.velocity(), 95);
        assert_eq!(Dynamic::Ppp.velocity(), 15);
        assert_eq!(Dynamic::Fff.velocity(), 127);
        assert_eq!(Dynamic::Mf.name(), "mf");
    }
}
