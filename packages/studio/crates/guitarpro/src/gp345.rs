//! The reader of GP3, GP4 and GP5 (D166, README §2): a little-endian walk
//! from the header through the song's settings, master bars and tracks to
//! each track's bars, filling the model as it goes. One reader for the
//! three, branching on the layout where they differ (GP4 adds the lyrics
//! and two bytes of effects, GP5 the page setup, a second voice and RSE, and
//! v5.10 a few fields more than v5.00).
//!
//! It never indexes a slice: every read goes through `Reader::take`, which
//! fails with `Error::Truncated` at the end of the file, and a value that
//! cannot be (a negative count, an unknown duration) is `Error::Invalid` at
//! its offset. Text has no declared encoding, so the file is walked once to
//! gather its strings, and again in the encoding they suggest when that is
//! not Windows-1252 (`text`).
use crate::model::*;
use crate::{Error, Song, Version};

#[cfg(test)]
mod tests;
mod text;

use text::Encoding;

/// Reads a GP3, GP4 or GP5 file whose header `crate::sniff` read as
/// `version`.
pub fn read(bytes: &[u8], version: Version) -> Result<Song, Error> {
    read_with_end(bytes, version).map(|(song, _)| song)
}

/// `read`, and the offset where the walk ended: the file's length for a
/// GP5, 4 short of it for a GP3 or GP4, whose files end with 4 zero bytes
/// (README §2.11). Bytes after the last bar are not an error.
pub fn read_with_end(bytes: &[u8], version: Version) -> Result<(Song, usize), Error> {
    let layout = Layout::of(version)?;
    let mut first = Walk::new(bytes, layout, Encoding::Windows1252);
    let song = first.song()?;
    let encoding = text::guess(first.r.texts.iter().copied());
    if encoding == Encoding::Windows1252 {
        return Ok((song, first.r.pos));
    }
    let mut again = Walk::new(bytes, layout, encoding);
    let song = again.song()?;
    Ok((song, again.r.pos))
}

/// Which of the layouts a version has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Layout {
    /// 3, 4 or 5.
    gp: u8,
    /// v5.10 and later fives (README §2.18).
    v510: bool,
}

impl Layout {
    fn of(version: Version) -> Result<Layout, Error> {
        match version.major {
            3..=5 => Ok(Layout {
                gp: version.major,
                v510: version.major == 5 && version.minor >= 10,
            }),
            _ => Err(Error::Unsupported(format!("A Guitar Pro {version} file"))),
        }
    }
}

/// The bytes and a position in them.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    encoding: Encoding,
    /// Every string read, as stored, for `text::guess`.
    texts: Vec<&'a [u8]>,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let truncated = Error::Truncated {
            offset: self.pos,
            needed: n,
        };
        let end = self.pos.checked_add(n).ok_or(truncated.clone())?;
        let slice = self.bytes.get(self.pos..end).ok_or(truncated)?;
        self.pos = end;
        Ok(slice)
    }

    fn skip(&mut self, n: usize) -> Result<(), Error> {
        self.take(n).map(|_| ())
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.array::<1>()?[0])
    }

    fn i8(&mut self) -> Result<i8, Error> {
        Ok(i8::from_le_bytes(self.array()?))
    }

    fn bool(&mut self) -> Result<bool, Error> {
        Ok(self.u8()? != 0)
    }

    fn i16(&mut self) -> Result<i16, Error> {
        Ok(i16::from_le_bytes(self.array()?))
    }

    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    /// An `int` that counts something, so cannot be negative.
    fn count(&mut self, what: &str) -> Result<usize, Error> {
        let at = self.pos;
        let n = self.i32()?;
        usize::try_from(n).map_err(|_| invalid(at, format!("{what} is {n}")))
    }

    fn text(&mut self, raw: &'a [u8]) -> String {
        self.texts.push(raw);
        text::decode(raw, self.encoding)
    }

    /// `ByteString(size)`: a length byte, then a field of `size` bytes.
    fn byte_string(&mut self, size: usize) -> Result<String, Error> {
        let len = usize::from(self.u8()?);
        let field = self.take(size)?;
        let raw = field.get(..len.min(size)).unwrap_or(field);
        Ok(self.text(raw))
    }

    /// `IntString`: an `int` length, then the text.
    fn int_string(&mut self) -> Result<String, Error> {
        let len = self.count("a string's length")?;
        let raw = self.take(len)?;
        Ok(self.text(raw))
    }

    /// `IntByteString`: an `int` (the length + 1), a length byte, then the
    /// field. An `int` of 0 is an empty string with nothing after it.
    fn int_byte_string(&mut self) -> Result<String, Error> {
        let size = self.count("a string's size")?;
        if size == 0 {
            return Ok(String::new());
        }
        let len = usize::from(self.u8()?);
        let field = self.take(size - 1)?;
        let raw = field.get(..len.min(field.len())).unwrap_or(field);
        Ok(self.text(raw))
    }
}

fn invalid(offset: usize, what: impl Into<String>) -> Error {
    Error::Invalid {
        offset,
        what: what.into(),
    }
}

/// A whole note in ticks: every length a GP3–GP5 beat can have is a whole
/// number of them (a dotted 64th is 3/128, and a tuplet divides by 3, 5, 6,
/// 7, 9, 10, 11, 12 or 13, whose least common multiple is 180 180). Offsets
/// are summed in ticks as `u64`, which no file can overflow.
const TICKS_PER_WHOLE: u64 = 128 * 180_180;

fn ticks(duration: &Duration) -> u64 {
    let base = TICKS_PER_WHOLE / u64::from(duration.value.max(1));
    let dotted = if duration.dots > 0 {
        base + base / 2
    } else {
        base
    };
    match duration.tuplet {
        Some(t) if t.notes > 0 => dotted * u64::from(t.space) / u64::from(t.notes),
        _ => dotted,
    }
}

fn fraction_of_ticks(ticks: u64) -> Fraction {
    let (mut a, mut b) = (ticks, TICKS_PER_WHOLE);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    let g = a.max(1);
    Fraction::new(
        u32::try_from(ticks / g).unwrap_or(u32::MAX),
        u32::try_from(TICKS_PER_WHOLE / g).unwrap_or(u32::MAX),
    )
}

/// One of the 64 MIDI channels of README §2.8.
#[derive(Clone, Copy, Default)]
struct Channel {
    program: i32,
    volume: u8,
    balance: u8,
}

/// Guitar Pro's 0–16 volume or balance on MIDI's 0–127, 8 (the middle)
/// being 64 (README §2.8).
fn midi_scale(value: u8) -> u8 {
    u8::try_from((u16::from(value) * 8).min(127)).unwrap_or(127)
}

/// The GP5 directions in the order the file lists their bars (README §2.8).
const DIRECTIONS: [Direction; 19] = [
    Direction::Coda,
    Direction::DoubleCoda,
    Direction::Segno,
    Direction::SegnoSegno,
    Direction::Fine,
    Direction::DaCapo,
    Direction::DaCapoAlCoda,
    Direction::DaCapoAlDoubleCoda,
    Direction::DaCapoAlFine,
    Direction::DalSegno,
    Direction::DalSegnoAlCoda,
    Direction::DalSegnoAlDoubleCoda,
    Direction::DalSegnoAlFine,
    Direction::DalSegnoSegno,
    Direction::DalSegnoSegnoAlCoda,
    Direction::DalSegnoSegnoAlDoubleCoda,
    Direction::DalSegnoSegnoAlFine,
    Direction::DaCoda,
    Direction::DaDoubleCoda,
];

/// What a beat's effects say of the beat, and of each of its notes.
#[derive(Default)]
struct BeatFx {
    effects: BeatEffects,
    /// Every note vibrates (the beat's vibrato flag).
    vibrato: bool,
    /// GP3's harmonics, given on the beat for all its notes.
    harmonic: Option<Harmonic>,
}

/// The state of a track while its bars are read.
struct TrackState {
    /// The string count, for chord diagrams.
    strings: usize,
    /// The last fret on each string (1 to 7) in each voice, for ties.
    last_fret: [[Option<u8>; 8]; 2],
}

struct Walk<'a> {
    r: Reader<'a>,
    layout: Layout,
    /// Tempo changes as (master bar, offset in ticks, bpm), in file order.
    tempos: Vec<(usize, u64, f64)>,
}

impl<'a> Walk<'a> {
    fn new(bytes: &'a [u8], layout: Layout, encoding: Encoding) -> Walk<'a> {
        Walk {
            r: Reader {
                bytes,
                pos: 0,
                encoding,
                texts: Vec::new(),
            },
            layout,
            tempos: Vec::new(),
        }
    }

    fn gp(&self) -> u8 {
        self.layout.gp
    }

    fn song(&mut self) -> Result<Song, Error> {
        let mut song = Song {
            version: self.r.byte_string(30)?,
            ..Song::default()
        };
        self.info(&mut song.info)?;
        let triplet_feel = if self.gp() < 5 && self.r.bool()? {
            TripletFeel::Eighth
        } else {
            TripletFeel::None
        };
        let lyrics = if self.gp() >= 4 {
            self.lyrics()?
        } else {
            (0, Vec::new())
        };
        if self.layout.v510 {
            // RSE master effect: volume, an int, 11 equalizer bytes.
            self.r.skip(4 + 4 + 11)?;
        }
        if self.gp() == 5 {
            // Page setup: 7 ints, a short of flags, 10 templates.
            self.r.skip(7 * 4 + 2)?;
            for _ in 0..10 {
                self.r.int_byte_string()?;
            }
            song.tempo_name = self.r.int_byte_string()?;
        }
        song.tempo = f64::from(self.r.i32()?);
        if self.layout.v510 {
            self.r.bool()?; // the tempo hidden
        }
        let key = if self.gp() == 3 {
            self.r.i32()?
        } else {
            let fifths = self.r.i8()?;
            self.r.skip(4)?;
            i32::from(fifths)
        };
        let key = KeySignature {
            fifths: i8::try_from(key.clamp(-7, 7)).unwrap_or(0),
            minor: false,
        };
        let channels = self.channels()?;
        let mut directions = [-1i16; 19];
        if self.gp() == 5 {
            for d in &mut directions {
                *d = self.r.i16()?;
            }
            self.r.i32()?; // master reverb
        }
        let bar_count = self.r.count("the number of bars")?;
        let track_count = self.r.count("the number of tracks")?;

        let mut previous = MasterBar {
            key,
            triplet_feel,
            ..MasterBar::default()
        };
        for index in 0..bar_count {
            let bar = self.master_bar(index, &previous)?;
            previous = bar.clone();
            song.master_bars.push(bar);
        }
        for (index, at) in directions.iter().enumerate() {
            let bar = usize::try_from(*at).ok().and_then(|b| b.checked_sub(1));
            if let (Some(bar), Some(direction)) = (bar, DIRECTIONS.get(index))
                && let Some(master) = song.master_bars.get_mut(bar)
            {
                master.directions.push(direction.clone());
            }
        }

        let mut states = Vec::new();
        for index in 0..track_count {
            let (track, strings) = self.track(index, &channels)?;
            song.tracks.push(track);
            states.push(TrackState {
                strings,
                last_fret: [[None; 8]; 2],
            });
        }
        if self.gp() == 5 {
            self.r.skip(if self.layout.v510 { 1 } else { 2 })?;
        }
        let (lyrics_track, lines) = lyrics;
        if let Some(track) = lyrics_track
            .checked_sub(1)
            .and_then(|t| song.tracks.get_mut(t))
        {
            track.lyrics = lines;
        }

        // GP5 ends each track's bar with a byte, but for the file's last
        // (README §2.11).
        let measures = bar_count.saturating_mul(track_count);
        let mut read = 0;
        for bar in 0..bar_count {
            for (track, state) in song.tracks.iter_mut().zip(states.iter_mut()) {
                let measure = self.measure(bar, state)?;
                track.bars.push(measure);
                read += 1;
                if self.gp() == 5 && read < measures {
                    self.r.skip(1)?;
                }
            }
        }
        self.finish_tempos(&mut song);
        Ok(song)
    }

    fn info(&mut self, info: &mut SongInfo) -> Result<(), Error> {
        info.title = self.r.int_byte_string()?;
        info.subtitle = self.r.int_byte_string()?;
        info.artist = self.r.int_byte_string()?;
        info.album = self.r.int_byte_string()?;
        info.words = self.r.int_byte_string()?;
        if self.gp() == 5 {
            info.music = self.r.int_byte_string()?;
        }
        info.copyright = self.r.int_byte_string()?;
        info.transcriber = self.r.int_byte_string()?;
        info.instructions = self.r.int_byte_string()?;
        let notices = self.r.count("the number of notice lines")?;
        for _ in 0..notices {
            let line = self.r.int_byte_string()?;
            info.notices.push(line);
        }
        Ok(())
    }

    /// GP4 and GP5's words: the track they belong to (from 1, 0 for none)
    /// and their non-empty lines.
    fn lyrics(&mut self) -> Result<(usize, Vec<LyricLine>), Error> {
        let track = usize::try_from(self.r.i32()?).unwrap_or(0);
        let mut lines = Vec::new();
        for _ in 0..5 {
            let bar = self.r.i32()?;
            let text = self.r.int_string()?;
            if !text.trim().is_empty() {
                lines.push(LyricLine {
                    start_bar: usize::try_from(bar).unwrap_or(0).saturating_sub(1),
                    text: text.replace("\r\n", "\n"),
                });
            }
        }
        Ok((track, lines))
    }

    fn channels(&mut self) -> Result<Vec<Channel>, Error> {
        let mut channels = Vec::with_capacity(64);
        for _ in 0..64 {
            let program = self.r.i32()?;
            let [volume, balance, _chorus, _reverb, _phaser, _tremolo, _, _] = self.r.array()?;
            channels.push(Channel {
                program,
                volume,
                balance,
            });
        }
        Ok(channels)
    }

    fn master_bar(&mut self, index: usize, previous: &MasterBar) -> Result<MasterBar, Error> {
        if self.gp() == 5 && index > 0 {
            self.r.skip(1)?;
        }
        let flags = self.r.u8()?;
        let mut bar = MasterBar {
            time: previous.time,
            key: previous.key,
            triplet_feel: previous.triplet_feel,
            repeat_start: flags & 0x04 != 0,
            double_bar: flags & 0x80 != 0,
            ..MasterBar::default()
        };
        if flags & 0x01 != 0 {
            bar.time.numerator = self.time_part("numerator")?;
        }
        if flags & 0x02 != 0 {
            bar.time.denominator = self.time_part("denominator")?;
        }
        if flags & 0x08 != 0 {
            let stored = i16::from(self.r.i8()?);
            // GP3 and GP4 store the repeats, GP5 the plays (README §2.9).
            let plays = if self.gp() < 5 { stored + 1 } else { stored };
            bar.repeat_end = Some(u8::try_from(plays.clamp(2, 127)).unwrap_or(2));
        }
        if self.gp() < 5 {
            if flags & 0x10 != 0 {
                let number = self.r.u8()?;
                if number > 0 {
                    bar.alternate_endings.push(number);
                }
            }
            if flags & 0x20 != 0 {
                bar.marker = Some(self.marker()?);
            }
            if flags & 0x40 != 0 {
                bar.key = self.key()?;
            }
        } else {
            if flags & 0x20 != 0 {
                bar.marker = Some(self.marker()?);
            }
            if flags & 0x40 != 0 {
                bar.key = self.key()?;
            }
            if flags & 0x10 != 0 {
                let mask = self.r.u8()?;
                bar.alternate_endings = (0..8)
                    .filter(|i| mask & (1 << i) != 0)
                    .map(|i| i + 1)
                    .collect();
            }
            if flags & 0x03 != 0 {
                self.r.skip(4)?; // beaming
            }
            if flags & 0x10 == 0 {
                self.r.skip(1)?;
            }
            let at = self.r.pos;
            bar.triplet_feel = match self.r.u8()? {
                0 => TripletFeel::None,
                1 => TripletFeel::Eighth,
                2 => TripletFeel::Sixteenth,
                n => return Err(invalid(at, format!("a triplet feel of {n}"))),
            };
        }
        Ok(bar)
    }

    fn time_part(&mut self, what: &str) -> Result<u8, Error> {
        let at = self.r.pos;
        let n = self.r.i8()?;
        u8::try_from(n)
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid(at, format!("a time signature's {what} of {n}")))
    }

    fn marker(&mut self) -> Result<String, Error> {
        let name = self.r.int_byte_string()?;
        self.r.skip(4)?; // colour
        Ok(name)
    }

    fn key(&mut self) -> Result<KeySignature, Error> {
        let fifths = self.r.i8()?.clamp(-7, 7);
        let minor = self.r.i8()? != 0;
        Ok(KeySignature { fifths, minor })
    }

    /// A track, and how many strings it has.
    fn track(&mut self, index: usize, channels: &[Channel]) -> Result<(Track, usize), Error> {
        if self.gp() == 5 && (index == 0 || !self.layout.v510) {
            self.r.skip(1)?;
        }
        let flags = self.r.u8()?;
        let name = self.r.byte_string(40)?;
        let at = self.r.pos;
        let strings = self.r.count("the number of strings")?;
        if !(1..=7).contains(&strings) {
            return Err(invalid(at, format!("a track of {strings} strings")));
        }
        let mut tuning = Vec::with_capacity(strings);
        for string in 0..7 {
            let pitch = self.r.i32()?;
            if string < strings {
                tuning.push(u8::try_from(pitch.clamp(0, 127)).unwrap_or(0));
            }
        }
        let port = self.r.i32()?;
        let channel = self.r.i32()?;
        let effect_channel = self.r.i32()?;
        let frets = self.r.i32()?;
        let capo = self.r.i32()?;
        self.r.skip(4)?; // colour
        let mut bank = 0;
        if self.gp() == 5 {
            self.r.skip(2 + 1)?; // display flags, auto-accentuation
            bank = self.r.u8()?;
            self.r.skip(1 + 12 + 12 + 12)?; // humanize, 3 ints, 12 bytes, RSE instrument
            if self.layout.v510 {
                self.r.skip(4 + 4)?; // effect number, equalizer
                self.r.int_byte_string()?;
                self.r.int_byte_string()?;
            } else {
                self.r.skip(2 + 1)?;
            }
        }
        let small = |n: i32| u8::try_from(n.clamp(0, 255)).unwrap_or(0);
        let from_one = |n: i32| small(n.saturating_sub(1));
        // Into §2.8's table; a port or channel out of range has no entry.
        let entry = (1..=4)
            .contains(&port)
            .then_some(())
            .filter(|_| (1..=16).contains(&channel))
            .and_then(|_| usize::try_from((port - 1) * 16 + (channel - 1)).ok())
            .and_then(|slot| channels.get(slot))
            .copied();
        let mut midi = MidiChannel {
            port: from_one(port),
            channel: from_one(channel),
            effect_channel: from_one(effect_channel),
            bank,
            ..MidiChannel::default()
        };
        if let Some(entry) = entry {
            midi.program = u8::try_from(entry.program.clamp(0, 127)).unwrap_or(0);
            midi.volume = midi_scale(entry.volume);
            midi.pan = midi_scale(entry.balance);
        }
        let kind = if flags & 0x01 != 0 || channel == 10 {
            TrackKind::Drums
        } else if matches!(midi.program, 24..=39 | 104..=107) {
            TrackKind::Strings
        } else {
            TrackKind::Other
        };
        let track = Track {
            name,
            kind,
            tuning,
            capo: small(capo),
            frets: small(frets),
            midi,
            ..Track::default()
        };
        Ok((track, strings))
    }

    fn finish_tempos(&mut self, song: &mut Song) {
        let mut tempos = std::mem::take(&mut self.tempos);
        // Stable, so of two tracks' changes at one beat the later wins.
        tempos.sort_by_key(|&(bar, at, _)| (bar, at));
        let mut changes: Vec<(usize, u64, f64)> = Vec::new();
        for change in tempos {
            match changes.last_mut() {
                Some(last) if (last.0, last.1) == (change.0, change.1) => *last = change,
                _ => changes.push(change),
            }
        }
        for (bar, at, bpm) in changes {
            if bar == 0 && at == 0 {
                song.tempo = bpm;
            } else {
                song.tempo_changes.push(TempoChange {
                    bar,
                    offset: fraction_of_ticks(at),
                    bpm,
                });
            }
        }
    }
}

/// A track's bars: voices, beats, notes and their effects (README
/// §2.11–§2.17).
impl Walk<'_> {
    fn measure(&mut self, bar: usize, state: &mut TrackState) -> Result<Bar, Error> {
        let voices = if self.gp() == 5 { 2 } else { 1 };
        let mut out = Bar::default();
        for voice in 0..voices {
            let count = self.r.count("the number of beats")?;
            let mut beats = Vec::new();
            let mut at = 0u64;
            for _ in 0..count {
                let beat = self.beat(bar, at, voice, state)?;
                at = at.saturating_add(ticks(&beat.duration));
                beats.push(beat);
            }
            out.voices.push(Voice { beats });
        }
        Ok(out)
    }

    fn beat(
        &mut self,
        bar: usize,
        at: u64,
        voice: usize,
        state: &mut TrackState,
    ) -> Result<Beat, Error> {
        let flags = self.r.u8()?;
        let mut beat = Beat::default();
        if flags & 0x40 != 0 {
            let at = self.r.pos;
            beat.status = match self.r.u8()? {
                0 => BeatStatus::Empty,
                1 => BeatStatus::Normal,
                2 => BeatStatus::Rest,
                n => return Err(invalid(at, format!("a beat of status {n}"))),
            };
        }
        let duration_at = self.r.pos;
        let value = match self.r.i8()? {
            d @ -2..=-1 => 4 >> -d,
            d @ 0..=4 => 4 << d,
            d => return Err(invalid(duration_at, format!("a duration of {d}"))),
        };
        beat.duration = Duration {
            value,
            dots: flags & 0x01,
            tuplet: None,
        };
        if flags & 0x20 != 0 {
            let at = self.r.pos;
            beat.duration.tuplet = match self.r.i32()? {
                0 | 1 => None,
                3 => Some(Tuplet { notes: 3, space: 2 }),
                n @ 5..=7 => Some(Tuplet {
                    notes: n as u8,
                    space: 4,
                }),
                n @ 9..=13 => Some(Tuplet {
                    notes: n as u8,
                    space: 8,
                }),
                n => return Err(invalid(at, format!("a tuplet of {n}"))),
            };
        }
        if flags & 0x02 != 0 {
            beat.chord = Some(self.chord(state.strings)?);
        }
        if flags & 0x04 != 0 {
            beat.text = Some(self.r.int_byte_string()?);
        }
        let mut fx = BeatFx::default();
        if flags & 0x08 != 0 {
            fx = self.beat_effects()?;
        }
        if flags & 0x10 != 0
            && let Some(bpm) = self.mix_table()?
        {
            self.tempos.push((bar, at, f64::from(bpm)));
        }
        let strings = self.r.u8()?;
        let last_fret = state
            .last_fret
            .get_mut(voice)
            .ok_or_else(|| invalid(self.r.pos, "a third voice"))?;
        for string in 1..=7u8 {
            if strings & (0x80 >> string) == 0 {
                continue;
            }
            let mut note = self.note(string, last_fret)?;
            note.vibrato |= fx.vibrato;
            if note.harmonic.is_none() {
                note.harmonic = fx.harmonic;
            }
            if beat.status == BeatStatus::Normal {
                beat.notes.push(note);
            }
        }
        if self.gp() == 5 {
            let flags2 = self.r.i16()?;
            if flags2 & 0x0800 != 0 {
                self.r.skip(1)?;
            }
        }
        beat.effects = fx.effects;
        Ok(beat)
    }

    /// A chord (README §2.16), its frets cut to the track's `strings`.
    fn chord(&mut self, strings: usize) -> Result<Chord, Error> {
        let form = self.r.u8()?;
        let (name, frets) = if form == 0 {
            // The old form: a name, a first fret and, unless it is 0, six
            // frets.
            let name = self.r.int_byte_string()?;
            let first = self.r.i32()?;
            let frets = if first != 0 {
                self.frets(6)?
            } else {
                Vec::new()
            };
            (name, frets)
        } else if self.gp() == 3 {
            // sharp, 3 blanks, root, type, extension, bass, tonality, add
            self.r.skip(1 + 3 + 5 * 4 + 1)?;
            let name = self.r.byte_string(20)?;
            self.r.skip(2 + 3 * 4)?; // 2 blanks, fifth, ninth, eleventh
            self.r.i32()?; // first fret
            let frets = self.frets(6)?;
            self.r.skip(4 + 6 * 4 + 7 + 1)?; // barres, omissions, a blank
            (name, frets)
        } else {
            // sharp, 3 blanks, root, type, extension, bass, tonality, add
            self.r.skip(1 + 3 + 3 + 4 + 4 + 1)?;
            let name = self.r.byte_string(21)?;
            self.r.skip(4)?; // fifth, ninth, eleventh, a blank
            self.r.i32()?; // first fret
            let frets = self.frets(7)?;
            self.r.skip(32)?; // barres, omissions, fingering
            (name, frets)
        };
        let mut frets: Vec<Option<u8>> = frets.into_iter().take(strings).collect();
        if frets.iter().all(Option::is_none) {
            frets.clear();
        }
        Ok(Chord { name, frets })
    }

    fn frets(&mut self, n: usize) -> Result<Vec<Option<u8>>, Error> {
        let mut frets = Vec::with_capacity(n);
        for _ in 0..n {
            frets.push(u8::try_from(self.r.i32()?).ok());
        }
        Ok(frets)
    }

    fn beat_effects(&mut self) -> Result<BeatFx, Error> {
        let f1 = self.r.u8()?;
        let f2 = if self.gp() >= 4 { self.r.u8()? } else { 0 };
        let mut fx = BeatFx {
            vibrato: f1 & 0x01 != 0,
            ..BeatFx::default()
        };
        fx.effects.wide_vibrato = f1 & 0x02 != 0;
        fx.effects.fade_in = f1 & 0x10 != 0;
        fx.effects.rasgueado = f2 & 0x01 != 0;
        if self.gp() == 3 {
            if f1 & 0x04 != 0 {
                fx.harmonic = Some(Harmonic::Natural);
            }
            if f1 & 0x08 != 0 {
                fx.harmonic = Some(Harmonic::Artificial);
            }
        }
        if f1 & 0x20 != 0 {
            let kind = self.r.u8()?;
            if self.gp() == 3 {
                let value = self.r.i32()?;
                if kind == 0 {
                    // GP3's tremolo bar: a dip to `value` and back, in the
                    // bend's units (README §2.15).
                    fx.effects.tremolo_bar = Some(Bend {
                        points: vec![
                            BendPoint {
                                position: 0.0,
                                semitones: 0.0,
                            },
                            BendPoint {
                                position: 0.5,
                                semitones: value as f32 / 25.0,
                            },
                            BendPoint {
                                position: 1.0,
                                semitones: 0.0,
                            },
                        ],
                    });
                }
            }
            fx.effects.slap = match kind {
                1 => Some(Slap::Tap),
                2 => Some(Slap::Slap),
                3 => Some(Slap::Pop),
                _ => None,
            };
        }
        if f2 & 0x04 != 0 {
            fx.effects.tremolo_bar = Some(self.bend()?);
        }
        if f1 & 0x40 != 0 {
            let [first, second] = self.r.array()?;
            // GP3 and GP4 give the down stroke's speed first; GP5 the up
            // stroke's (README §2.15: most of the GP5 samples' strokes set
            // the second byte, and a strum is mostly down).
            let (down, up) = if self.gp() == 5 {
                (second, first)
            } else {
                (first, second)
            };
            let stroke = |direction, speed: u8| Stroke {
                direction,
                value: match speed {
                    1..=6 => 1 << (8 - speed),
                    _ => 0,
                },
            };
            fx.effects.stroke = if up > 0 {
                Some(stroke(StrokeDirection::Up, up))
            } else if down > 0 {
                Some(stroke(StrokeDirection::Down, down))
            } else {
                None
            };
        }
        if f2 & 0x02 != 0 {
            fx.effects.pick_stroke = match self.r.u8()? {
                1 => Some(StrokeDirection::Up),
                2 => Some(StrokeDirection::Down),
                _ => None,
            };
        }
        Ok(fx)
    }

    /// A bend or the tremolo bar's curve: 60 to the note's length, 25 to
    /// the semitone (README §2.14).
    fn bend(&mut self) -> Result<Bend, Error> {
        self.r.skip(1 + 4)?; // kind, value
        let count = self.r.count("the number of a bend's points")?;
        let mut points = Vec::new();
        for _ in 0..count {
            let position = self.r.i32()?;
            let value = self.r.i32()?;
            self.r.skip(1)?; // vibrato
            points.push(BendPoint {
                position: (position as f32 / 60.0).clamp(0.0, 1.0),
                semitones: value as f32 / 25.0,
            });
        }
        Ok(Bend { points })
    }

    /// A mix-table change (README §2.17), and the tempo it sets, if any.
    fn mix_table(&mut self) -> Result<Option<i32>, Error> {
        self.r.i8()?; // instrument
        if self.gp() == 5 {
            self.r.skip(16)?; // RSE instrument
        }
        let changes: [u8; 6] = self.r.array()?; // volume … tremolo, −1 none
        if self.gp() == 5 {
            self.r.int_byte_string()?; // the tempo's name
        }
        let tempo = self.r.i32()?;
        let changed = changes.iter().filter(|&&c| (c as i8) >= 0).count();
        self.r.skip(changed)?; // their durations
        if tempo >= 0 {
            self.r.skip(1)?;
            if self.layout.v510 {
                self.r.skip(1)?; // the tempo hidden
            }
        }
        if self.gp() >= 4 {
            self.r.skip(1)?; // which apply to all tracks
        }
        if self.gp() == 5 {
            self.r.skip(1)?; // wah
            if self.layout.v510 {
                self.r.int_byte_string()?;
                self.r.int_byte_string()?;
            }
        }
        Ok((tempo >= 0).then_some(tempo))
    }

    /// A note on `string`; `last_fret` is the voice's last fret on each
    /// string, which a tie takes.
    fn note(&mut self, string: u8, last_fret: &mut [Option<u8>; 8]) -> Result<Note, Error> {
        let flags = self.r.u8()?;
        let mut note = Note {
            string,
            ghost: flags & 0x04 != 0,
            accent: if flags & 0x40 != 0 {
                Accent::Normal
            } else {
                Accent::None
            },
            ..Note::default()
        };
        // GP5's 0x02 is a heavy accent; GP3 and GP4 set it on normal notes
        // for no known reason, and it is ignored there (README §2.13).
        if self.gp() == 5 && flags & 0x02 != 0 {
            note.accent = Accent::Heavy;
        }
        if flags & 0x20 != 0 {
            let at = self.r.pos;
            match self.r.u8()? {
                0 | 1 => {}
                2 => note.tie = true,
                3 => note.dead = true,
                n => return Err(invalid(at, format!("a note of type {n}"))),
            }
        }
        if self.gp() < 5 && flags & 0x01 != 0 {
            self.r.skip(2)?; // its own duration and tuplet
        }
        if flags & 0x10 != 0 {
            note.dynamic = Dynamic::from_step(self.r.i8()?.into());
        }
        if flags & 0x20 != 0 {
            note.fret = u8::try_from(self.r.i8()?.max(0)).unwrap_or(0);
        }
        let slot = last_fret.get_mut(usize::from(string));
        if note.tie
            && let Some(Some(fret)) = slot.as_deref()
        {
            note.fret = *fret;
        }
        if let Some(slot) = slot {
            *slot = Some(note.fret);
        }
        if flags & 0x80 != 0 {
            self.r.skip(2)?; // fingering
        }
        if self.gp() == 5 {
            if flags & 0x01 != 0 {
                self.r.skip(8)?; // its length in percent
            }
            self.r.skip(1)?;
        }
        if flags & 0x08 != 0 {
            self.note_effects(&mut note)?;
        }
        Ok(note)
    }

    fn note_effects(&mut self, note: &mut Note) -> Result<(), Error> {
        let f1 = self.r.u8()?;
        let f2 = if self.gp() >= 4 { self.r.u8()? } else { 0 };
        if f1 & 0x01 != 0 {
            note.bend = Some(self.bend()?);
        }
        note.hammer = f1 & 0x02 != 0;
        if self.gp() == 3 && f1 & 0x04 != 0 {
            note.slide = Some(Slide::Shift);
        }
        note.let_ring = f1 & 0x08 != 0;
        if f1 & 0x10 != 0 {
            let [fret, dynamic, transition, value] = self.r.array()?;
            let mut grace = Grace {
                fret,
                value: match value {
                    1 => 64,
                    3 => 16,
                    _ => 32,
                },
                dynamic: Dynamic::from_step(dynamic.into()),
                transition: match transition {
                    1 => GraceTransition::Slide,
                    2 => GraceTransition::Bend,
                    3 => GraceTransition::Hammer,
                    _ => GraceTransition::None,
                },
                on_beat: false,
                dead: false,
            };
            if self.gp() == 5 {
                let flags = self.r.u8()?;
                grace.dead = flags & 0x01 != 0;
                grace.on_beat = flags & 0x02 != 0;
            }
            note.grace = Some(grace);
        }
        note.staccato = f2 & 0x01 != 0;
        note.palm_mute = f2 & 0x02 != 0;
        if f2 & 0x04 != 0 {
            note.tremolo_picking = Some(match self.r.u8()? {
                1 => 8,
                3 => 32,
                _ => 16,
            });
        }
        if f2 & 0x08 != 0 {
            let kind = self.r.u8()?;
            note.slide = if self.gp() == 5 {
                // A bitmask; the model keeps one slide, the lowest bit's.
                [
                    Slide::Shift,
                    Slide::Legato,
                    Slide::OutDownwards,
                    Slide::OutUpwards,
                    Slide::IntoFromBelow,
                    Slide::IntoFromAbove,
                ]
                .into_iter()
                .enumerate()
                .find(|(bit, _)| kind & (1 << bit) != 0)
                .map(|(_, slide)| slide)
            } else {
                match kind as i8 {
                    1 => Some(Slide::Shift),
                    2 => Some(Slide::Legato),
                    3 => Some(Slide::OutDownwards),
                    4 => Some(Slide::OutUpwards),
                    -1 => Some(Slide::IntoFromBelow),
                    -2 => Some(Slide::IntoFromAbove),
                    _ => None,
                }
            };
        }
        if f2 & 0x10 != 0 {
            let at = self.r.pos;
            let kind = self.r.u8()?;
            note.harmonic = Some(match (self.gp(), kind) {
                (_, 1) => Harmonic::Natural,
                (5, 2) => {
                    self.r.skip(3)?; // the note, its accidental, its octave
                    Harmonic::Artificial
                }
                (5, 3) => Harmonic::Tapped { fret: self.r.u8()? },
                (_, 3) => Harmonic::Tapped {
                    fret: note.fret.saturating_add(12),
                },
                (_, 4) => Harmonic::Pinch,
                (_, 5) => Harmonic::Semi,
                (4, 2 | 15 | 17 | 22) => Harmonic::Artificial,
                _ => return Err(invalid(at, format!("a harmonic of kind {kind}"))),
            });
        }
        if f2 & 0x20 != 0 {
            let [fret, period] = self.r.array()?;
            note.trill = Some(Trill {
                fret,
                value: match period {
                    2 => 32,
                    3 => 64,
                    _ => 16,
                },
            });
        }
        note.vibrato = f2 & 0x40 != 0;
        Ok(())
    }
}
