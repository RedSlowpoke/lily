//! `gp345` on the six binary samples of README §1, which are copyrighted
//! songs and so not in the repository: each is read from
//! `GUITAR_PRO_SAMPLES` (unset, none is found), and a test
//! skips the files it cannot find. The facts checked are what
//! `docs/research/guitar-pro/spike/inspect_gp.py` prints for them.
use std::path::PathBuf;

use lily_guitarpro::{BeatStatus, Dynamic, Format, Fraction, Song, TrackKind, Voice, gp345, sniff};

fn samples() -> PathBuf {
    std::env::var_os("GUITAR_PRO_SAMPLES").map_or_else(PathBuf::new, PathBuf::from)
}

/// The sample read whole, or `None` when it is not there.
fn sample(name: &str) -> Option<Song> {
    let path = samples().join(name);
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipped {}: not found", path.display());
        return None;
    };
    let Ok(Format::Binary(version)) = sniff(&bytes, None) else {
        panic!("{name} is not a binary Guitar Pro file");
    };
    let (song, end) =
        gp345::read_with_end(&bytes, version).unwrap_or_else(|e| panic!("{name}: {e}"));
    let rest = bytes.get(end..).unwrap_or_default();
    if version.major < 5 {
        assert_eq!(rest, [0, 0, 0, 0], "{name}: GP3 and GP4 end with 4 zeros");
    } else {
        assert!(rest.is_empty(), "{name}: {} bytes left over", rest.len());
    }
    check_bars(name, &song);
    Some(song)
}

fn length(voice: &Voice) -> Fraction {
    voice
        .beats
        .iter()
        .fold(Fraction::ZERO, |sum, beat| sum + beat.duration.length())
}

/// The bars the samples leave short of their time signature, which Guitar
/// Pro allows: (file, track, bar, length), from 0. The bass of
/// `gp3-b.gp3`'s last bar is a single quarter note.
const SHORT_BARS: [(&str, usize, usize, Fraction); 1] =
    [("gp3-b.gp3", 2, 141, Fraction { num: 1, den: 4 })];

/// Every track has a bar for each master bar, and each voice that plays
/// fills its bar's time signature, but for `SHORT_BARS`.
fn check_bars(name: &str, song: &Song) {
    for (t, track) in song.tracks.iter().enumerate() {
        assert_eq!(track.bars.len(), song.master_bars.len(), "{name} track {t}");
        for (b, (bar, master)) in track.bars.iter().zip(&song.master_bars).enumerate() {
            for (v, voice) in bar.voices.iter().enumerate() {
                if voice.is_empty() {
                    continue;
                }
                let expected = SHORT_BARS
                    .iter()
                    .find(|s| (s.0, s.1, s.2) == (name, t, b))
                    .map_or(master.length(), |s| s.3);
                assert_eq!(
                    length(voice),
                    expected,
                    "{name}: track {t} bar {b} voice {v}"
                );
            }
        }
    }
}

/// The notes of a beat as (string, fret), the first string first.
fn notes(song: &Song, track: usize, bar: usize, beat: usize) -> Vec<(u8, u8)> {
    song.tracks[track].bars[bar].voices[0].beats[beat]
        .notes
        .iter()
        .map(|n| (n.string, n.fret))
        .collect()
}

fn names_and_kinds(song: &Song) -> Vec<(&str, TrackKind)> {
    song.tracks
        .iter()
        .map(|t| (t.name.as_str(), t.kind))
        .collect()
}

const STANDARD: [u8; 6] = [64, 59, 55, 50, 45, 40];
const BASS: [u8; 4] = [43, 38, 33, 28];

#[test]
fn reads_gp3_a() {
    let Some(song) = sample("gp3-a.gp3") else {
        return;
    };
    assert_eq!(song.version, "FICHIER GUITAR PRO v3.00");
    for field in [
        &song.info.title,
        &song.info.artist,
        &song.info.words,
        &song.info.transcriber,
    ] {
        assert!(!field.is_empty());
    }
    assert_eq!(song.tempo, 95.0);
    assert_eq!(song.master_bars.len(), 25);
    assert_eq!(names_and_kinds(&song), [("Traccia 1", TrackKind::Strings)]);
    let track = &song.tracks[0];
    assert_eq!(track.tuning, STANDARD);
    assert_eq!(
        (track.midi.program, track.midi.channel, track.frets),
        (24, 0, 24)
    );
    // The repeat of bars 1–7, stored as 1 (README §2.9).
    assert!(song.master_bars[0].repeat_start);
    assert_eq!(song.master_bars[6].repeat_end, Some(2));
    // The tempo change on the first beat of bar 23 (from 1).
    assert_eq!(song.tempo_changes.len(), 1);
    assert_eq!(song.tempo_changes[0].bar, 22);
    assert_eq!(song.tempo_changes[0].offset, Fraction::ZERO);
    assert_eq!(notes(&song, 0, 0, 0), [(2, 3), (5, 3)]);
    assert_eq!(notes(&song, 0, 0, 2), [(2, 1), (3, 0), (4, 2)]);
    assert_eq!(notes(&song, 0, 1, 3), [(1, 4), (6, 0)]);
    let first = &track.bars[0].voices[0].beats[0];
    assert_eq!(first.duration.value, 8);
    let let_ring = track
        .bars
        .iter()
        .flat_map(|b| &b.voices)
        .flat_map(|v| &v.beats)
        .flat_map(|b| &b.notes)
        .filter(|n| n.let_ring)
        .count();
    assert_eq!(let_ring, 107);
}

#[test]
fn reads_gp3_b() {
    let Some(song) = sample("gp3-b.gp3") else {
        return;
    };
    assert!(song.info.title.ends_with(" (batterie)"));
    assert_eq!(song.tempo, 127.0);
    assert_eq!(song.master_bars.len(), 142);
    assert_eq!(
        names_and_kinds(&song),
        [
            ("guitare", TrackKind::Strings),
            ("guitare distorsion", TrackKind::Strings),
            ("basse", TrackKind::Strings),
            ("Percussions", TrackKind::Drums),
        ]
    );
    assert_eq!(song.tracks[2].tuning, BASS);
    assert_eq!(song.tracks[3].midi.channel, 9);
    assert_eq!(song.tracks[3].frets, 87);
    // A dotted eighth of three strings, then dead notes.
    let first = &song.tracks[0].bars[0].voices[0].beats[0];
    assert_eq!((first.duration.value, first.duration.dots), (8, 1));
    assert_eq!(notes(&song, 0, 0, 0), [(4, 3), (5, 3), (6, 1)]);
    let dead = &song.tracks[0].bars[0].voices[0].beats[3];
    assert!(dead.notes.iter().all(|n| n.dead));
    assert_eq!(notes(&song, 0, 1, 0), [(4, 6), (5, 6), (6, 4)]);
}

#[test]
fn reads_gp4_a() {
    let Some(song) = sample("gp4-a.gp4") else {
        return;
    };
    assert_eq!(song.version, "FICHIER GUITAR PRO v4.00");
    for field in [
        &song.info.title,
        &song.info.artist,
        &song.info.album,
        &song.info.transcriber,
    ] {
        assert!(!field.is_empty());
    }
    assert_eq!(song.tempo, 120.0);
    assert_eq!(song.master_bars.len(), 100);
    assert_eq!(
        names_and_kinds(&song),
        [
            ("Piano", TrackKind::Other),
            ("Distortion Guitar", TrackKind::Strings),
            ("Overdriven Guitar", TrackKind::Strings),
            ("Bass", TrackKind::Strings),
            ("Strings", TrackKind::Other),
            ("Percussion", TrackKind::Drums),
        ]
    );
    assert_eq!(song.tracks[0].midi.program, 1);
    assert_eq!(song.tracks[4].midi.program, 41);
    assert_eq!(song.tracks[3].tuning, BASS);
    let markers: Vec<_> = song
        .master_bars
        .iter()
        .enumerate()
        .filter_map(|(i, m)| m.marker.as_deref().map(|name| (i, name)))
        .collect();
    assert_eq!(markers[0], (0, "Intro"));
    assert_eq!(markers[1], (14, "Verse 1"));
    assert_eq!(markers.len(), 7);
    // Its one line of words names no track (README §2.5).
    assert!(song.tracks.iter().all(|t| t.lyrics.is_empty()));
    assert_eq!(song.tempo_changes.len(), 1);
    assert_eq!(song.tempo_changes[0].bar, 98);
    assert_eq!(notes(&song, 0, 0, 0), [(1, 0)]);
    assert_eq!(notes(&song, 0, 0, 3), [(1, 16)]);
    // A tied half note keeps the fret it holds.
    let tied = &song.tracks[0].bars[0].voices[0].beats[4];
    assert_eq!(tied.duration.value, 2);
    assert!(tied.notes[0].tie);
    assert_eq!(tied.notes[0].fret, 16);
}

#[test]
fn reads_gp5_a_v500() {
    let Some(song) = sample("gp5-a.gp5") else {
        return;
    };
    assert_eq!(song.version, "FICHIER GUITAR PRO v5.00");
    assert_eq!(song.info.title, "");
    assert!(!song.info.artist.is_empty() && !song.info.words.is_empty());
    assert_eq!(song.info.music, song.info.artist);
    assert_eq!(song.info.instructions, "listen only on GP 5.0 !!!");
    assert_eq!(song.info.notices.len(), 3);
    assert_eq!(song.tempo, 125.0);
    assert_eq!(song.tempo_name, "Moderate");
    assert_eq!(song.master_bars.len(), 139);
    assert!(song.master_bars[0].double_bar);
    assert_eq!(song.master_bars[1].marker.as_deref(), Some("Intro"));
    let kinds = names_and_kinds(&song);
    assert_eq!(kinds.len(), 10);
    assert_eq!(kinds[0], ("FX", TrackKind::Other));
    assert_eq!(kinds[2], ("Acoustic 01", TrackKind::Strings));
    assert_eq!(kinds[8], ("Bass", TrackKind::Strings));
    assert_eq!(kinds[9], ("Drums", TrackKind::Drums));
    assert_eq!(song.tracks[0].tuning, [61, 56, 52, 47, 42, 35]);
    assert_eq!(song.tracks[2].tuning, [61, 56, 52, 47, 42, 37]);
    assert_eq!(song.tracks[8].tuning, [40, 35, 30, 23]);
    assert!(song.tempo_changes.is_empty());
    // The first bar is an empty beat with text, then an empty second voice.
    let first = &song.tracks[0].bars[0];
    assert_eq!(first.voices.len(), 2);
    assert_eq!(first.voices[0].beats[0].status, BeatStatus::Empty);
    assert!(first.voices[0].beats[0].text.is_some());
    let empty = all_beats(&song)
        .filter(|b| b.status == BeatStatus::Empty)
        .count();
    assert_eq!(empty, 2051);
    let strokes = all_beats(&song)
        .filter(|b| b.effects.stroke.is_some())
        .count();
    assert_eq!(strokes, 1147);
}

fn all_beats(song: &Song) -> impl Iterator<Item = &lily_guitarpro::Beat> {
    song.tracks
        .iter()
        .flat_map(|t| &t.bars)
        .flat_map(|b| &b.voices)
        .flat_map(|v| &v.beats)
}

#[test]
fn reads_gp5_b_in_windows_1251() {
    let Some(song) = sample("gp5-b.gp5") else {
        return;
    };
    assert_eq!(song.version, "FICHIER GUITAR PRO v5.10");
    // The title and the notice are Cyrillic, decoded from Windows-1251.
    let cyrillic = |s: &str| s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
    assert!(cyrillic(&song.info.title), "{}", song.info.title);
    assert!(!song.info.artist.is_empty());
    assert_eq!(song.info.notices.len(), 1);
    assert!(cyrillic(&song.info.notices[0]), "{:?}", song.info.notices);
    assert_eq!(song.tempo, 95.0);
    assert_eq!(song.master_bars.len(), 80);
    let markers: Vec<_> = song
        .master_bars
        .iter()
        .filter_map(|m| m.marker.as_deref())
        .collect();
    assert_eq!(
        markers,
        ["Интро", "Куплет", "Припев", "Куплет", "Припев", "Соло"]
    );
    assert_eq!(
        names_and_kinds(&song),
        [
            ("Guitar", TrackKind::Strings),
            ("Second guitar", TrackKind::Strings),
            ("Bass", TrackKind::Strings),
            ("Percussion", TrackKind::Drums),
        ]
    );
    assert_eq!(song.tracks[0].tuning, STANDARD);
    assert_eq!(song.tracks[2].tuning, BASS);
    assert_eq!(notes(&song, 0, 0, 0), [(6, 0)]);
    assert_eq!(notes(&song, 0, 0, 1), [(4, 2)]);
    // A note at mf (dynamic 5).
    let beat = &song.tracks[0].bars[0].voices[0].beats[3];
    assert_eq!(beat.duration.value, 16);
    assert_eq!(beat.notes[0].dynamic, Dynamic::Mf);
    assert_eq!(notes(&song, 0, 1, 0), [(5, 3)]);
    // Every stroke of this strummed song is the same, a down stroke.
    let strokes: Vec<_> = all_beats(&song).filter_map(|b| b.effects.stroke).collect();
    assert_eq!(strokes.len(), 273);
    assert!(
        strokes
            .iter()
            .all(|s| s.direction == lily_guitarpro::StrokeDirection::Down)
    );
}

#[test]
fn reads_gp5_c_v510() {
    let Some(song) = sample("gp5-c.gp5") else {
        return;
    };
    assert_eq!(song.version, "FICHIER GUITAR PRO v5.10");
    assert!(!song.info.title.is_empty() && !song.info.music.is_empty());
    assert_eq!(song.info.album, song.info.title);
    assert_eq!(song.tempo, 96.0);
    assert_eq!(song.master_bars.len(), 127);
    assert_eq!(
        names_and_kinds(&song),
        [
            ("Gtr 1", TrackKind::Strings),
            ("Vox", TrackKind::Other),
            ("Gtr 2", TrackKind::Strings),
            ("Backing Vox", TrackKind::Other),
            ("Keyboard", TrackKind::Other),
            ("Bass", TrackKind::Strings),
            ("Drums", TrackKind::Drums),
        ]
    );
    assert_eq!(song.tracks[6].frets, 99);
    assert_eq!(song.tracks[5].tuning, BASS);
    assert_eq!(song.tracks[0].midi.program, 27);
    assert_eq!(notes(&song, 0, 0, 0), [(5, 0)]);
    assert_eq!(notes(&song, 0, 0, 3), [(2, 1)]);
    assert_eq!(notes(&song, 0, 1, 0), [(6, 1)]);
    let tied = &song.tracks[0].bars[0].voices[0].beats[4];
    assert!(tied.notes[0].tie);
    assert_eq!(tied.notes[0].fret, 1, "a tie takes the fret it holds");
    // Two-tone bends (100 in the file).
    let peak = all_beats(&song)
        .flat_map(|b| &b.notes)
        .filter_map(|n| n.bend.as_ref())
        .map(|b| b.peak())
        .fold(0.0f32, f32::max);
    assert_eq!(peak, 4.0);
}

/// How many notes of the song have each effect, against the tallies of
/// `inspect_gp.py`'s walk.
fn tally(song: &Song) -> std::collections::BTreeMap<&'static str, usize> {
    let mut counts = std::collections::BTreeMap::new();
    for note in all_beats(song).flat_map(|b| &b.notes) {
        let mut add = |what: &'static str, yes: bool| {
            if yes {
                *counts.entry(what).or_insert(0) += 1;
            }
        };
        add("tie", note.tie);
        add("dead", note.dead);
        add("ghost", note.ghost);
        add("grace", note.grace.is_some());
        add("hammer", note.hammer);
        add("let ring", note.let_ring);
        add("palm mute", note.palm_mute);
        add("bend", note.bend.is_some());
        add("slide", note.slide.is_some());
        add("harmonic", note.harmonic.is_some());
    }
    counts
}

#[test]
fn reads_every_note_effect_the_walk_counts() {
    let cases: [(&str, &[(&str, usize)]); 4] = [
        (
            "gp5-a.gp5",
            &[
                ("dead", 85),
                ("grace", 33),
                ("hammer", 1),
                ("harmonic", 3),
                ("let ring", 5600),
                ("palm mute", 174),
                ("slide", 90),
                ("tie", 646),
            ],
        ),
        (
            "gp3-b.gp3",
            &[
                ("bend", 8),
                ("dead", 669),
                ("ghost", 13),
                ("grace", 8),
                ("slide", 11),
                ("tie", 133),
            ],
        ),
        (
            "gp5-b.gp5",
            &[
                ("bend", 8),
                ("dead", 9),
                ("ghost", 28),
                ("grace", 8),
                ("hammer", 24),
                ("let ring", 378),
                ("tie", 111),
            ],
        ),
        (
            "gp5-c.gp5",
            &[
                ("bend", 12),
                ("dead", 32),
                ("hammer", 52),
                ("let ring", 134),
                ("tie", 307),
            ],
        ),
    ];
    for (name, expected) in cases {
        let Some(song) = sample(name) else {
            continue;
        };
        let counts = tally(&song);
        let expected: std::collections::BTreeMap<_, _> = expected.iter().copied().collect();
        assert_eq!(counts, expected, "{name}");
    }
}

const BINARY: [&str; 6] = [
    "gp3-a.gp3",
    "gp3-b.gp3",
    "gp4-a.gp4",
    "gp5-a.gp5",
    "gp5-b.gp5",
    "gp5-c.gp5",
];

/// A sample cut short fails with an error, and one with bytes changed at
/// random fails or reads, but never panics.
#[test]
fn damaged_samples_never_panic() {
    for name in BINARY {
        let Ok(bytes) = std::fs::read(samples().join(name)) else {
            eprintln!("skipped {name}: not found");
            continue;
        };
        let Ok(Format::Binary(version)) = sniff(&bytes, None) else {
            panic!("{name}");
        };
        let end = bytes.len() - if version.major < 5 { 4 } else { 0 };
        for k in 0..100 {
            let len = end * k / 100;
            assert!(
                gp345::read(&bytes[..len], version).is_err(),
                "{name} cut to {len}"
            );
        }
        let mut state = 0x9E37_79B9u32;
        for _ in 0..250 {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let mut damaged = bytes.clone();
            let at = state as usize % damaged.len();
            damaged[at] = (state >> 8) as u8;
            let _ = gp345::read(&damaged, version);
        }
    }
}
