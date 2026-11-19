//! The writer on songs built by hand, and, where lilypond is installed, the
//! scores it writes compiled by it. A compile test skips itself without
//! lilypond (`LILYPOND_PATH`, else the PATH), so a run that skipped them has
//! not checked that the scores compile.
use std::path::PathBuf;
use std::process::Command;

use super::write;
use crate::*;

const STANDARD: [u8; 6] = [64, 59, 55, 50, 45, 40];

fn note(string: u8, fret: u8) -> Note {
    Note {
        string,
        fret,
        ..Note::default()
    }
}

fn dur(value: u8) -> Duration {
    Duration {
        value,
        dots: 0,
        tuplet: None,
    }
}

fn beat(value: u8, notes: Vec<Note>) -> Beat {
    Beat {
        duration: dur(value),
        notes,
        ..Beat::default()
    }
}

fn rest(value: u8) -> Beat {
    Beat {
        duration: dur(value),
        status: BeatStatus::Rest,
        ..Beat::default()
    }
}

fn empty(value: u8) -> Beat {
    Beat {
        duration: dur(value),
        status: BeatStatus::Empty,
        ..Beat::default()
    }
}

fn bar(beats: Vec<Beat>) -> Bar {
    Bar {
        voices: vec![Voice { beats }],
    }
}

fn track(name: &str, kind: TrackKind, tuning: &[u8], program: u8, bars: Vec<Bar>) -> Track {
    Track {
        name: name.into(),
        kind,
        tuning: tuning.to_vec(),
        frets: 24,
        midi: MidiChannel {
            program,
            channel: if kind == TrackKind::Drums { 9 } else { 0 },
            ..MidiChannel::default()
        },
        bars,
        ..Track::default()
    }
}

fn song(bars: usize, tracks: Vec<Track>) -> Song {
    Song {
        version: "FICHIER GUITAR PRO v5.10".into(),
        info: SongInfo {
            title: "Test".into(),
            ..SongInfo::default()
        },
        tempo: 120.0,
        master_bars: vec![MasterBar::default(); bars],
        tracks,
        ..Song::default()
    }
}

/// Four quarter notes on a string.
fn quarters(string: u8, frets: [u8; 4]) -> Bar {
    bar(frets
        .iter()
        .map(|&f| beat(4, vec![note(string, f)]))
        .collect())
}

/// A guitar, a bass in drop D and a drum kit, two bars each.
fn band() -> Song {
    let guitar = track(
        "Lead",
        TrackKind::Strings,
        &STANDARD,
        30,
        vec![
            quarters(6, [0, 3, 5, 7]),
            bar(vec![
                beat(2, vec![note(5, 2), note(4, 2), note(3, 1)]),
                beat(2, vec![note(1, 0)]),
            ]),
        ],
    );
    let bass = track(
        "Bass",
        TrackKind::Strings,
        &[43, 38, 33, 26],
        33,
        vec![quarters(4, [0, 0, 5, 7]), quarters(3, [0, 2, 3, 5])],
    );
    let kick_hat = || beat(4, vec![note(1, 36), note(2, 42)]);
    let snare_hat = || beat(4, vec![note(1, 38), note(2, 42)]);
    let drums = track(
        "Drums",
        TrackKind::Drums,
        &[0; 6],
        0,
        vec![
            bar(vec![kick_hat(), snare_hat(), kick_hat(), snare_hat()]),
            bar(vec![
                kick_hat(),
                snare_hat(),
                beat(4, vec![note(1, 49), note(2, 36)]),
                beat(4, vec![note(1, 200)]),
            ]),
        ],
    );
    song(2, vec![guitar, bass, drums])
}

#[test]
fn a_guitar_a_drop_tuned_bass_and_drums() {
    let ly = write(&band());
    assert!(ly.starts_with("\\version \"2.24.0\"\n"), "{ly}");
    assert!(ly.contains("leadMusic = {\n"), "{ly}");
    assert!(ly.contains("  \\tempo 4 = 120\n"), "{ly}");
    assert!(
        ly.contains("e,4\\6 g,\\6 a,\\6 b,\\6 |  % 1"),
        "a bar a line, durations once: {ly}"
    );
    assert!(
        ly.contains("<b,\\5 e\\4 gis\\3>2 e'\\1 \\bar \"|.\" |  % 2"),
        "{ly}"
    );
    assert!(ly.contains("stringTunings = \\stringTuning <e, a, d g b e'>"));
    assert!(ly.contains("\\clef \"treble_8\" \\leadMusic"));
    assert!(ly.contains("midiInstrument = \"distorted guitar\""));
    // Drop D on a four-string bass: D1 A1 D2 G2, a bass clef.
    assert!(
        ly.contains("stringTunings = \\stringTuning <d,, a,, d, g,>"),
        "{ly}"
    );
    assert!(ly.contains("\\clef \"bass_8\" \\bassMusic"), "{ly}");
    assert!(ly.contains("drumsMusic = \\drummode {"), "{ly}");
    assert!(
        ly.contains("<bassdrum closedhihat>4 <acousticsnare closedhihat>"),
        "{ly}"
    );
    assert!(
        ly.contains("<bassdrum crashcymbala> sidestick \\bar"),
        "an unknown key falls back: {ly}"
    );
    assert!(ly.contains("\\new DrumStaff"), "{ly}");
    assert!(
        !ly.contains("\\unfoldRepeats"),
        "no repeats, one score: {ly}"
    );
    assert!(ly.contains("\\remove \"Staff_performer\""), "{ly}");
    // Only the first track carries the tempo.
    assert_eq!(ly.matches("\\tempo").count(), 1, "{ly}");
    compile("band", &ly, &[]);
}

#[test]
fn a_seven_string_guitar() {
    let seven = [64, 59, 55, 50, 45, 40, 35];
    let t = track(
        "Seven",
        TrackKind::Strings,
        &seven,
        29,
        vec![quarters(7, [0, 2, 3, 5])],
    );
    let ly = write(&song(1, vec![t]));
    assert!(ly.contains("\\stringTuning <b,, e, a, d g b e'>"), "{ly}");
    assert!(ly.contains("b,,4\\7 cis,\\7 d,\\7 e,\\7"), "{ly}");
    assert!(ly.contains("\\clef \"treble_8\""), "{ly}");
    compile("seven", &ly, &[]);
}

#[test]
fn tuplets_dots_and_ties_across_bars() {
    let triplet = |fret| Beat {
        duration: Duration {
            value: 8,
            dots: 0,
            tuplet: Some(Tuplet { notes: 3, space: 2 }),
        },
        notes: vec![note(3, fret)],
        ..Beat::default()
    };
    let dotted = Beat {
        duration: Duration {
            value: 4,
            dots: 1,
            tuplet: None,
        },
        notes: vec![note(2, 1)],
        ..Beat::default()
    };
    let tied = Note {
        tie: true,
        ..note(2, 1)
    };
    let t = track(
        "Gtr",
        TrackKind::Strings,
        &STANDARD,
        25,
        vec![
            bar(vec![
                triplet(0),
                triplet(2),
                triplet(4),
                dotted,
                beat(8, vec![note(2, 3)]),
                beat(4, vec![note(2, 1)]),
            ]),
            bar(vec![beat(2, vec![tied]), rest(2)]),
        ],
    );
    let ly = write(&song(2, vec![t]));
    assert!(ly.contains("\\tuplet 3/2 { g8\\3 a\\3 b\\3 }"), "{ly}");
    assert!(ly.contains("c'4.\\2 d'8\\2 c'4~\\2 |"), "{ly}");
    assert!(ly.contains("c'2\\2 r \\bar"), "{ly}");
    compile("tuplets", &ly, &[]);
}

#[test]
fn two_voices_are_two_voice_contexts() {
    let up = bar(vec![beat(2, vec![note(1, 0)]), beat(2, vec![note(1, 3)])]);
    let down = Voice {
        beats: vec![beat(1, vec![note(6, 0)])],
    };
    let silent = Voice {
        beats: vec![empty(1)],
    };
    let bars = vec![
        Bar {
            voices: vec![up.voices[0].clone(), down],
        },
        Bar {
            voices: vec![up.voices[0].clone(), silent],
        },
    ];
    let t = track("Lead", TrackKind::Strings, &STANDARD, 27, bars);
    let ly = write(&song(2, vec![t]));
    assert!(ly.contains("leadVoiceOne = {"), "{ly}");
    assert!(ly.contains("leadVoiceTwo = {"), "{ly}");
    assert!(ly.contains("\\voiceOne\n"), "{ly}");
    assert!(ly.contains("\\voiceTwo\n"), "{ly}");
    assert!(ly.contains("\\oneVoice\n"), "{ly}");
    assert!(
        ly.contains("s1 \\bar \"|.\" |  % 2"),
        "an empty second voice is a spacer: {ly}"
    );
    assert!(ly.contains("\\new TabVoice { \\leadVoiceTwo }"), "{ly}");
    compile("voices", &ly, &[]);
}

#[test]
fn an_empty_second_voice_is_dropped() {
    let bars = vec![Bar {
        voices: vec![
            quarters(1, [0, 1, 3, 5]).voices.remove(0),
            Voice {
                beats: vec![empty(1)],
            },
        ],
    }];
    let t = track("Lead", TrackKind::Strings, &STANDARD, 27, bars);
    let ly = write(&song(1, vec![t]));
    assert!(ly.contains("leadMusic = {"), "{ly}");
    assert!(!ly.contains("VoiceTwo"), "{ly}");
}

/// Bars 0–1 repeat with two endings (bars 2 and 3); bars 4–6 repeat with
/// no start mark, as in the .gpx sample, from the bar after the last repeat.
fn repeated() -> Song {
    let mut s = song(
        7,
        vec![track(
            "Gtr",
            TrackKind::Strings,
            &STANDARD,
            25,
            (0..7).map(|b| quarters(1, [b, b, b, b])).collect(),
        )],
    );
    let m = &mut s.master_bars;
    m[0].repeat_start = true;
    m[0].marker = Some("Verse".into());
    m[2].alternate_endings = vec![1];
    m[2].repeat_end = Some(2);
    m[3].alternate_endings = vec![2];
    m[6].repeat_end = Some(3);
    m[5].directions = vec![Direction::Segno];
    m[6].directions = vec![Direction::DalSegnoAlFine];
    s
}

#[test]
fn a_repeat_with_alternate_endings() {
    let ly = write(&repeated());
    let expected = "\
  \\repeat volta 2 {
    \\unfolded \\volta 2 \\set Score.currentBarNumber = #1
    \\time 4/4
    \\key c \\major
    \\tempo 4 = 120
    \\sectionLabel \"Verse\"
    e'4\\1 e'\\1 e'\\1 e'\\1 |  % 1
    f'4\\1 f'\\1 f'\\1 f'\\1 |  % 2
  }
  \\alternative {
    \\volta 1 {
      \\unfolded \\set Score.currentBarNumber = #3
      fis'4\\1 fis'\\1 fis'\\1 fis'\\1 |  % 3
    }
    \\volta 2 {
      \\unfolded \\set Score.currentBarNumber = #4
      g'4\\1 g'\\1 g'\\1 g'\\1 |  % 4
    }
  }
  \\repeat volta 3 {
    \\unfolded \\volta 2,3 \\set Score.currentBarNumber = #5
    gis'4\\1 gis'\\1 gis'\\1 gis'\\1 |  % 5
    \\segnoMark 1
    a'4\\1 a'\\1 a'\\1 a'\\1 |  % 6
    ais'4\\1 ais'\\1 ais'\\1 ais'\\1 \\jump \"D.S. al Fine\" |  % 7
  }
";
    assert!(ly.contains(expected), "{ly}");
    assert!(
        ly.contains("\\score {\n  \\staves\n  \\layout { }\n}"),
        "{ly}"
    );
    assert!(ly.contains("\\unfoldRepeats \\staves"), "{ly}");
    compile("repeats", &ly, &[]);
}

#[test]
fn contradictory_repeats_are_unfolded() {
    let mut s = repeated();
    // The second ending claims the first time through as well.
    s.master_bars[3].alternate_endings = vec![1];
    let ly = write(&s);
    assert!(!ly.contains("\\repeat"), "{ly}");
    assert!(
        ly.contains("% Guitar Pro's repeat marks contradict"),
        "{ly}"
    );
    assert!(ly.contains("\\set Score.currentBarNumber = #1\n"), "{ly}");
    compile("unfolded", &ly, &[]);
}

#[test]
fn cyrillic_and_duplicate_names() {
    let mut s = song(
        1,
        vec![
            track(
                "Гитара",
                TrackKind::Strings,
                &STANDARD,
                29,
                vec![quarters(1, [0; 4])],
            ),
            track(
                "FX",
                TrackKind::Other,
                &STANDARD,
                84,
                vec![quarters(1, [0; 4])],
            ),
            track(
                "FX",
                TrackKind::Other,
                &STANDARD,
                80,
                vec![quarters(6, [0; 4])],
            ),
            track(
                "Acoustic 01",
                TrackKind::Strings,
                &STANDARD,
                25,
                vec![quarters(2, [0; 4])],
            ),
            track(
                "Acoustic 01",
                TrackKind::Strings,
                &STANDARD,
                25,
                vec![quarters(3, [0; 4])],
            ),
        ],
    );
    s.info = SongInfo {
        title: "Песня".into(),
        artist: "Группа".into(),
        album: "Новый альбом".into(),
        words: "Иван \"Ваня\" Петров".into(),
        transcriber: "кто-то".into(),
        notices: vec!["Первая строка".into(), String::new()],
        ..SongInfo::default()
    };
    let ly = write(&s);
    assert!(ly.contains("  title = \"Песня\"\n"), "{ly}");
    assert!(ly.contains("  composer = \"Группа\"\n"), "{ly}");
    assert!(ly.contains("  subsubtitle = \"Новый альбом\"\n"), "{ly}");
    assert!(
        ly.contains("  poet = \"Иван \\\"Ваня\\\" Петров\"\n"),
        "{ly}"
    );
    assert!(ly.contains("  arranger = \"Tab: кто-то\"\n"), "{ly}");
    assert!(ly.contains("% Первая строка\n"), "{ly}");
    for var in [
        "gitaraMusic",
        "fxMusic",
        "fxBMusic",
        "acousticOneMusic",
        "acousticOneBMusic",
    ] {
        assert!(ly.contains(&format!("{var} = {{")), "{var}: {ly}");
    }
    assert!(ly.contains("instrumentName = \"Гитара\""), "{ly}");
    // An FX track is a Staff alone; the low one gets a bass clef.
    assert!(ly.contains("\\clef \"treble\" \\fxMusic"), "{ly}");
    assert!(ly.contains("\\clef \"bass\" \\fxBMusic"), "{ly}");
    assert!(ly.contains("midiInstrument = \"lead 5 (charang)\""), "{ly}");
    compile("names", &ly, &[]);
}

#[test]
fn short_and_long_bars_are_made_to_fit() {
    let t = track(
        "Bass",
        TrackKind::Strings,
        &[43, 38, 33, 28],
        33,
        vec![
            // One quarter in 4/4, as `gp3-b.gp3`'s last bass bar.
            bar(vec![beat(4, vec![note(4, 0)])]),
            // Five quarters and a half in 4/4.
            bar(vec![
                beat(4, vec![note(4, 0)]),
                beat(4, vec![note(4, 0)]),
                beat(4, vec![note(4, 0)]),
                beat(2, vec![note(4, 0)]),
                beat(4, vec![note(4, 0)]),
            ]),
        ],
    );
    let ly = write(&song(2, vec![t]));
    assert!(ly.contains("e,,4\\4 r2. |  % 1"), "{ly}");
    assert!(
        ly.contains("% Guitar Pro's bar 2 runs past its 4/4: the rest is left out."),
        "{ly}"
    );
    assert!(
        ly.contains("e,,4\\4 e,,\\4 e,,\\4 e,,2*1/2\\4 \\bar"),
        "{ly}"
    );
    compile("lengths", &ly, &[]);
}

#[test]
fn a_drum_track_of_rests_is_written() {
    let drums = track(
        "Drums",
        TrackKind::Drums,
        &[0; 6],
        0,
        vec![bar(vec![rest(2), rest(2)]), Bar::default()],
    );
    let ly = write(&song(2, vec![drums]));
    assert!(ly.contains("R1 |  % 1"), "{ly}");
    assert!(ly.contains("R1 \\bar \"|.\" |  % 2"), "{ly}");
    compile("drum-rests", &ly, &[]);
}

/// A guitar part with most of what Guitar Pro marks on notes.
fn effects() -> Song {
    let n = note;
    let with = |note: Note, f: &dyn Fn(&mut Note)| {
        let mut note = note;
        f(&mut note);
        note
    };
    let bend = Bend {
        points: vec![
            BendPoint {
                position: 0.0,
                semitones: 0.0,
            },
            BendPoint {
                position: 0.5,
                semitones: 2.0,
            },
        ],
    };
    let mut chord_beat = beat(4, vec![n(5, 0), n(4, 2), n(3, 2), n(2, 1)]);
    chord_beat.chord = Some(Chord {
        name: "Am".into(),
        frets: vec![],
    });
    chord_beat.effects.stroke = Some(Stroke {
        direction: StrokeDirection::Down,
        value: 16,
    });
    let mut text_beat = beat(4, vec![with(n(3, 7), &|x| x.vibrato = true)]);
    text_beat.text = Some("solo \"loud\"".into());
    text_beat.effects.pick_stroke = Some(StrokeDirection::Down);
    let bars = vec![
        bar(vec![
            beat(8, vec![with(n(3, 5), &|x| x.hammer = true)]),
            beat(8, vec![with(n(3, 7), &|x| x.hammer = true)]),
            beat(8, vec![n(3, 5)]),
            beat(8, vec![with(n(3, 5), &|x| x.slide = Some(Slide::Shift))]),
            beat(4, vec![with(n(3, 9), &|x| x.bend = Some(bend.clone()))]),
            beat(4, vec![with(n(6, 0), &|x| x.dead = true)]),
        ]),
        bar(vec![
            chord_beat,
            beat(
                8,
                vec![with(n(6, 0), &|x| {
                    x.palm_mute = true;
                    x.accent = Accent::Normal;
                })],
            ),
            beat(
                8,
                vec![with(n(6, 0), &|x| {
                    x.palm_mute = true;
                    x.staccato = true;
                    x.dynamic = Dynamic::Mf;
                })],
            ),
            beat(
                4,
                vec![with(n(2, 12), &|x| x.harmonic = Some(Harmonic::Natural))],
            ),
            beat(
                4,
                vec![with(n(1, 5), &|x| {
                    x.ghost = true;
                    x.grace = Some(Grace {
                        fret: 3,
                        value: 32,
                        dynamic: Dynamic::F,
                        transition: GraceTransition::Slide,
                        on_beat: false,
                        dead: false,
                    });
                })],
            ),
        ]),
        bar(vec![
            text_beat,
            beat(
                4,
                vec![with(n(2, 8), &|x| {
                    x.tremolo_picking = Some(16);
                    x.let_ring = true;
                    x.harmonic = Some(Harmonic::Pinch);
                })],
            ),
            beat(
                8,
                vec![with(n(2, 8), &|x| {
                    x.trill = Some(Trill {
                        fret: 10,
                        value: 16,
                    })
                })],
            ),
            beat(
                8,
                vec![with(n(2, 8), &|x| x.slide = Some(Slide::OutDownwards))],
            ),
        ]),
    ];
    let mut t = track("Lead", TrackKind::Strings, &STANDARD, 29, bars);
    t.capo = 2;
    t.short_name = "Ld".into();
    let mut s = song(3, vec![t]);
    s.tempo_name = "Moderate".into();
    s.tempo_changes = vec![
        TempoChange {
            bar: 1,
            offset: Fraction::ZERO,
            bpm: 120.0,
        },
        TempoChange {
            bar: 2,
            offset: Fraction::new(1, 2),
            bpm: 96.0,
        },
    ];
    s.master_bars[1].key = KeySignature {
        fifths: -1,
        minor: false,
    };
    s.master_bars[2].time = TimeSignature {
        numerator: 3,
        denominator: 4,
    };
    s.master_bars[2].triplet_feel = TripletFeel::Eighth;
    s.master_bars[2].marker = Some("Solo".into());
    s
}

#[test]
fn effects_are_written_where_lilypond_has_them() {
    let ly = write(&effects());
    // With the capo on 2, fret 5 on the G string sounds D4; the tab is
    // tuned to the capo so that it shows 5.
    assert!(
        ly.contains("\\stringTuning <fis, b, e a cis' fis'>"),
        "{ly}"
    );
    assert!(
        ly.contains("instrumentName = \\markup \\center-column { \"Lead\" \\small \"capo 2\" }"),
        "{ly}"
    );
    assert!(ly.contains("shortInstrumentName = \"Ld\""), "{ly}");
    assert!(ly.contains("\\tempo \"Moderate\" 4 = 120"), "{ly}");
    // A hammer-on chain is one slur; a shift slide a glissando.
    assert!(
        ly.contains("d'8\\3( e'\\3 d'\\3) d'\\3\\glissando fis'4\\3\\bendAfter 2"),
        "{ly}"
    );
    assert!(ly.contains("\\bendAfter 2"), "{ly}");
    assert!(ly.contains("\\deadNote fis,\\6 |"), "{ly}");
    assert!(ly.contains("\\arpeggio^\\markup \\bold \"Am\""), "{ly}");
    // Bar 2 is in F, so it is spelt with flats.
    assert!(ly.contains("ges,8\\6->^\\markup \\small \"P.M.\""), "{ly}");
    assert!(ly.contains("ges,\\6-.\\mf"), "palm mute once a run: {ly}");
    assert!(ly.contains("<des''\\harmonic\\2>4"), "{ly}");
    assert!(
        ly.contains("\\slashedGrace a'32\\1\\glissando \\parenthesize b'4\\1"),
        "{ly}"
    );
    assert!(ly.contains("\\key f \\major"), "{ly}");
    assert!(ly.contains("\\time 3/4"), "{ly}");
    assert!(ly.contains("\\sectionLabel \"Solo\""), "{ly}");
    assert!(
        ly.contains("^\\markup \\italic \"triplet feel 8ths\""),
        "{ly}"
    );
    assert!(ly.contains("\\prall\\downbow"), "{ly}");
    assert!(ly.contains("^\"solo \\\"loud\\\"\""), "{ly}");
    assert!(ly.contains("a'4:16\\2"), "{ly}");
    assert!(ly.contains("\\small \"let ring\""), "{ly}");
    assert!(ly.contains("\\small \"P.H.\""), "{ly}");
    assert!(ly.contains("\\trill"), "{ly}");
    assert!(ly.contains("\\bendAfter -2"), "{ly}");
    // The change to 120 at bar 2 is the tempo already; 96 falls mid-bar.
    assert_eq!(ly.matches("\\tempo").count(), 2, "{ly}");
    assert!(ly.contains("\\tempo 4 = 96 a'8\\2\\trill"), "{ly}");
    compile("effects", &ly, &[]);
}

#[test]
fn the_output_is_deterministic() {
    let s = effects();
    assert_eq!(write(&s), write(&s));
    let b = band();
    assert_eq!(write(&b), write(&b));
}

#[test]
fn a_song_without_tracks_has_no_score() {
    let ly = write(&song(2, vec![]));
    assert!(!ly.contains("\\score"), "{ly}");
}

#[test]
fn a_chord_name_is_printed_where_it_changes() {
    let chord = |name: &str, string: u8| Beat {
        chord: Some(Chord {
            name: name.into(),
            frets: Vec::new(),
        }),
        ..beat(4, vec![note(string, 0)])
    };
    let rhythm = track(
        "Rhythm",
        TrackKind::Strings,
        &STANDARD,
        25,
        vec![
            bar(vec![
                chord("F", 1),
                chord("F", 2),
                chord("A7", 3),
                chord("A7", 4),
            ]),
            bar(vec![
                chord("A7", 1),
                chord("Dm", 2),
                chord("Dm", 3),
                chord("F", 4),
            ]),
        ],
    );
    let ly = write(&song(2, vec![rhythm]));
    assert_eq!(ly.matches("\\bold \"F\"").count(), 2, "{ly}");
    assert_eq!(ly.matches("\\bold \"A7\"").count(), 1, "{ly}");
    assert_eq!(ly.matches("\\bold \"Dm\"").count(), 1, "{ly}");
}

#[test]
fn ghost_notes_leave_the_dynamic_alone() {
    let at = |dynamic: Dynamic, ghost: bool| {
        beat(
            4,
            vec![Note {
                dynamic,
                ghost,
                ..note(3, 2)
            }],
        )
    };
    let guitar = track(
        "Lead",
        TrackKind::Strings,
        &STANDARD,
        30,
        vec![bar(vec![
            at(Dynamic::F, false),
            at(Dynamic::Mf, true),
            at(Dynamic::F, false),
            at(Dynamic::P, false),
        ])],
    );
    let ly = write(&song(1, vec![guitar]));
    assert!(!ly.contains("\\mf"), "{ly}");
    assert_eq!(ly.matches("\\f").count(), 0, "{ly}");
    assert!(
        ly.contains("a4\\3 \\parenthesize a\\3 a\\3 a\\3\\p "),
        "{ly}"
    );
}

/// `gp3-b.gp3`'s `\\bendAfter #+3\\mp` was read by Guile as the symbol `+3\\mp`.
#[test]
fn a_bend_before_a_dynamic_compiles() {
    let bent = Note {
        bend: Some(Bend {
            points: vec![
                BendPoint {
                    position: 0.0,
                    semitones: 0.0,
                },
                BendPoint {
                    position: 0.5,
                    semitones: 3.0,
                },
            ],
        }),
        dynamic: Dynamic::Mp,
        ..note(2, 4)
    };
    let guitar = track(
        "Lead",
        TrackKind::Strings,
        &STANDARD,
        30,
        vec![bar(vec![
            beat(4, vec![bent]),
            beat(4, vec![note(2, 4)]),
            beat(2, vec![note(2, 4)]),
        ])],
    );
    let ly = write(&song(1, vec![guitar]));
    assert!(ly.contains("\\bendAfter 3\\mp"), "{ly}");
    compile("bend-dynamic", &ly, &[]);
}

#[test]
fn a_long_name_takes_two_lines() {
    use super::name_lines;
    assert_eq!(name_lines(""), Vec::<&str>::new());
    assert_eq!(name_lines("Acoustic 01"), ["Acoustic 01"]);
    assert_eq!(name_lines("Distortion Guitar"), ["Distortion", "Guitar"]);
    assert_eq!(name_lines("Distortion 03 - SE"), ["Distortion", "03 - SE"]);
    assert_eq!(name_lines("Перегруженная"), ["Перегруженная"]);
    let lead = track(
        "Overdriven Guitar",
        TrackKind::Strings,
        &STANDARD,
        29,
        vec![quarters(1, [0; 4])],
    );
    let ly = write(&song(1, vec![lead]));
    assert!(
        ly.contains("instrumentName = \\markup \\center-column { \"Overdriven\" \"Guitar\" }"),
        "{ly}"
    );
}

#[test]
fn many_staves_are_smaller() {
    use super::{DEFAULT_STAFF_SIZE, staff_size};
    // `gpx-a.gpx`: three guitars and a kit.
    assert_eq!(staff_size(8.5), DEFAULT_STAFF_SIZE);
    assert_eq!(staff_size(15.0), DEFAULT_STAFF_SIZE);
    // `gp5-a.gp5`: seven guitars and a bass with tab, two staves and a kit.
    assert_eq!(staff_size(20.5), 14);
    assert_eq!(staff_size(100.0), 11);
    let guitars = (0..7)
        .map(|_| {
            track(
                "Gtr",
                TrackKind::Strings,
                &STANDARD,
                29,
                vec![quarters(1, [0; 4])],
            )
        })
        .collect();
    let ly = write(&song(1, guitars));
    assert!(ly.contains("#(set-global-staff-size 17)"), "{ly}");
    assert!(
        ly.contains("\\context { \\TabStaff \\RemoveEmptyStaves }"),
        "{ly}"
    );
    assert!(!write(&band()).contains("set-global-staff-size"));
}

// ---- compiling with lilypond -------------------------------------------

fn lilypond() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("LILYPOND_PATH") {
        return Some(PathBuf::from(path));
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("lilypond"))
        .find(|p| p.is_file())
}

/// Compiles `ly` and fails on any error, or on a warning not in `allowed`
/// (each a piece of the warning's text). Skips without lilypond.
fn compile(name: &str, ly: &str, allowed: &[&str]) {
    let Some(lilypond) = lilypond() else {
        eprintln!("skipped compiling {name}: lilypond is not installed");
        return;
    };
    let dir = std::env::temp_dir().join(format!("lily-guitarpro-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{name}.ly"));
    std::fs::write(&file, ly).unwrap();
    let output = Command::new(&lilypond)
        .arg("-dno-point-and-click")
        .arg("--svg")
        .arg("-o")
        .arg(dir.join(name))
        .arg(&file)
        .output()
        .expect("lilypond runs");
    let log = String::from_utf8_lossy(&output.stderr).to_string();
    let problems: Vec<&str> = log
        .lines()
        .filter(|l| l.contains("error:") || l.contains("warning:"))
        .filter(|l| l.contains("error:") || !allowed.iter().any(|a| l.contains(a)))
        .collect();
    let midi =
        dir.join(format!("{name}.midi")).is_file() || dir.join(format!("{name}-1.midi")).is_file();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success() && problems.is_empty(),
        "{name} did not compile cleanly:\n{}\n\n{ly}",
        log
    );
    assert!(midi, "{name} wrote no MIDI:\n{log}");
    eprintln!("compiled {name} with {} cleanly", lilypond.display());
}
