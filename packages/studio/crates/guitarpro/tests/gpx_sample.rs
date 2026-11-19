//! `gpx-a.gpx` (README §1) read whole, from `GUITAR_PRO_SAMPLES` (unset,
//! it is not found). The file is a
//! copyrighted song and not in the repository; the tests skip when it is
//! not found.
use std::path::PathBuf;

use lily_guitarpro::{BeatStatus, Direction, Error, Fraction, Song, TrackKind, read};

fn sample(name: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("GUITAR_PRO_SAMPLES").map_or_else(PathBuf::new, PathBuf::from);
    let path = dir.join(name);
    let bytes = std::fs::read(&path).ok();
    if bytes.is_none() {
        eprintln!("skipped: {} not found", path.display());
    }
    bytes
}

fn gpx_a() -> Option<Song> {
    let bytes = sample("gpx-a.gpx")?;
    Some(read(&bytes, Some("gpx")).expect("the sample reads"))
}

#[test]
fn reads_the_song_and_its_tracks() {
    let Some(song) = gpx_a() else { return };
    assert_eq!(song.version, "GPIF 9934");
    for field in [
        &song.info.title,
        &song.info.artist,
        &song.info.album,
        &song.info.words,
        &song.info.music,
    ] {
        assert!(!field.is_empty());
    }
    assert_eq!(song.info.words, song.info.music);
    assert_eq!(song.tempo, 95.0);
    assert!(song.tempo_changes.is_empty());

    let tracks: Vec<_> = song
        .tracks
        .iter()
        .map(|t| {
            assert!(!t.name.is_empty());
            (t.instrument.as_str(), t.kind, t.midi.program)
        })
        .collect();
    assert_eq!(
        tracks,
        [
            ("e-gtr6", TrackKind::Strings, 27),
            ("e-bass4", TrackKind::Strings, 33),
            ("s-gtr6", TrackKind::Strings, 25),
            ("drmkt", TrackKind::Drums, 0),
        ]
    );
    assert_eq!(song.tracks[0].tuning, [64, 59, 55, 50, 45, 40]);
    assert_eq!(song.tracks[1].tuning, [43, 38, 33, 28]);
    assert_eq!(song.tracks[2].tuning, [64, 59, 55, 50, 45, 40]);
    assert!(song.tracks[3].tuning.is_empty());
    assert_eq!(song.tracks[3].midi.channel, 9);
    assert!(song.tracks.iter().all(|t| t.capo == 0));
}

#[test]
fn reads_the_master_bars() {
    let Some(song) = gpx_a() else { return };
    let bars = &song.master_bars;
    assert_eq!(bars.len(), 17);
    for bar in bars {
        assert_eq!((bar.time.numerator, bar.time.denominator), (4, 4));
        assert_eq!((bar.key.fifths, bar.key.minor), (-1, false));
    }
    // Bars 1–7 repeat (README §2.9: GPX's count is the plays), and so do
    // 8–15.
    let repeats: Vec<_> = bars
        .iter()
        .enumerate()
        .filter(|(_, b)| b.repeat_start || b.repeat_end.is_some())
        .map(|(i, b)| (i, b.repeat_start, b.repeat_end))
        .collect();
    assert_eq!(
        repeats,
        [(0, true, None), (6, false, Some(2)), (14, false, Some(2))]
    );
    assert_eq!(bars[0].directions, [Direction::Segno]);
    assert_eq!(bars[15].directions, [Direction::Coda]);
}

#[test]
fn reads_the_first_bars_notes() {
    let Some(song) = gpx_a() else { return };
    let pitches = |track: usize, bar: usize| -> Vec<Vec<u8>> {
        let t = &song.tracks[track];
        t.bars[bar].voices[0]
            .beats
            .iter()
            .map(|b| {
                let mut p: Vec<u8> = b.notes.iter().map(|n| t.pitch(n).unwrap()).collect();
                p.sort_unstable();
                p
            })
            .collect()
    };
    // The melody's first three notes: G F F.
    assert_eq!(pitches(0, 0), [[55], [53], [53]]);
    // The rhythm guitar's barre F (README §3.8): strings 6 to 1 at frets
    // 1 3 3 2 1 1, F C F A C F from the lowest, under its diagram "F".
    let rhythm = &song.tracks[2];
    let beat = &rhythm.bars[0].voices[0].beats[0];
    let mut frets: Vec<(u8, u8)> = beat.notes.iter().map(|n| (n.string, n.fret)).collect();
    frets.sort_unstable();
    assert_eq!(frets, [(1, 1), (2, 1), (3, 2), (4, 3), (5, 3), (6, 1)]);
    assert_eq!(pitches(2, 0)[0], [41, 48, 53, 57, 60, 65]);
    let chord = beat.chord.as_ref().expect("the F diagram");
    assert_eq!(chord.name, "F");
    assert_eq!(
        chord.frets,
        [Some(1), Some(1), Some(2), Some(3), Some(3), Some(1)]
    );
    // The bass's F, then its F held.
    assert_eq!(pitches(1, 0), [[31], [29], [29]]);
}

#[test]
fn every_voice_fills_its_bar() {
    let Some(song) = gpx_a() else { return };
    for track in &song.tracks {
        assert_eq!(track.bars.len(), song.master_bars.len());
        for (i, (bar, master)) in track.bars.iter().zip(&song.master_bars).enumerate() {
            assert!(!bar.voices.is_empty(), "{} bar {i}", track.name);
            for voice in bar.voices.iter().filter(|v| !v.beats.is_empty()) {
                let length = voice
                    .beats
                    .iter()
                    .fold(Fraction::ZERO, |sum, b| sum + b.duration.length());
                assert_eq!(length, master.length(), "{} bar {i}", track.name);
            }
        }
    }
}

#[test]
fn drum_notes_are_left_out() {
    let Some(song) = gpx_a() else { return };
    let drums = &song.tracks[3];
    let beats: Vec<_> = drums
        .bars
        .iter()
        .flat_map(|b| &b.voices)
        .flat_map(|v| &v.beats)
        .collect();
    assert!(!beats.is_empty());
    assert!(
        beats
            .iter()
            .all(|b| b.status == BeatStatus::Rest && b.notes.is_empty())
    );
}

/// `gp3-a.gp3` is another arrangement of the same song (1 track of 25
/// bars): its tempo and first repeat agree. Not its key: the GP3
/// leaves the key at C (0) for a song in F, which the GPX writes as one flat.
#[test]
fn agrees_with_the_gp3_arrangement() {
    let Some(gpx) = gpx_a() else { return };
    let Some(bytes) = sample("gp3-a.gp3") else {
        return;
    };
    let gp3 = match read(&bytes, Some("gp3")) {
        Ok(song) => song,
        Err(Error::Unsupported(what)) => {
            eprintln!("skipped: {what} cannot be read yet");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    assert_eq!(gp3.tempo, gpx.tempo);
    assert_eq!(gp3.master_bars[0].key.fifths, 0);
    assert_eq!(gpx.master_bars[0].key.fifths, -1);
    assert!(gp3.master_bars[0].repeat_start);
    assert_eq!(gp3.master_bars[6].repeat_end, gpx.master_bars[6].repeat_end);
    let tuning = &gp3.tracks[0].tuning;
    assert_eq!(tuning, &gpx.tracks[2].tuning);
}
