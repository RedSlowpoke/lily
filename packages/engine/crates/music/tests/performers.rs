//! MS5 step 1's performers (07 §7's M2): `Staff_performer`,
//! `Note_performer`, `Control_track_performer` and `Tempo_performer` on
//! the translator framework, step 2's `Dynamic_performer` (M3), step 3's
//! programs, controllers, keys, time signatures and pedals (M4), step 4's
//! ties, breaths, markers and lyrics (M5),
//! `Score_performer`'s columns, and the
//! `Performance` a `\midi` translation leaves, written by
//! `ly:performance-write` through `crates/smf`. Hand-made MIDI definitions
//! of `performer-init.ly`'s shape in a VM with only the music crate and the
//! music types; LilyPond's own files are compared byte for byte in
//! `tools/compare/test/c2.test.mjs`. Without LilyPond 2.26.0's music types
//! these tests are skipped, and nothing is verified.
use std::path::PathBuf;
use std::process::Command;

use lily_engine_music::{OutputDef, TYPES, make_context_def, run_performance, take_written};
use lily_engine_scheme::{Value, Vm};

/// `scm/lily/` as `build.rs` finds it.
fn lily_scm() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("LILY_SCM") {
        return Some(PathBuf::from(dir));
    }
    let path = Command::new("sh")
        .args(["-c", "command -v lilypond"])
        .output()
        .ok()?;
    let binary = std::fs::canonicalize(String::from_utf8(path.stdout).ok()?.trim()).ok()?;
    Some(binary.parent()?.join("../share/lilypond/2.26.0/scm/lily"))
}

fn eval(vm: &mut Vm, text: &str) -> Value {
    vm.eval_text(text, None)
        .unwrap_or_else(|e| panic!("{text}: {}", e.text(vm.heap())))
}

/// A Score of the control track and the tempo, staves of their own
/// tracks, voices of notes: `performer-init.ly`'s, but its other
/// performers.
const DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score))",
    "((context-name Score) (translator-type Score_performer) (accepts Staff)
      (default-child Staff) (assign tempoWholesPerMinute 15) (assign midiChannelMapping staff)
      (consists Control_track_performer) (consists Tempo_performer))",
    "((context-name Staff) (translator-type Performer_group) (accepts Voice)
      (default-child Voice) (consists Staff_performer))",
    "((context-name Voice) (translator-type Performer_group) (consists Note_performer))",
];

/// [`DEFS`] with `Dynamic_performer` in the Voice, as `performer-init.ly`
/// has it.
const DYNAMIC_DEFS: &[&str] = &[
    DEFS[0],
    DEFS[1],
    DEFS[2],
    "((context-name Voice) (translator-type Performer_group) (consists Note_performer)
      (consists Dynamic_performer))",
];

/// [`DEFS`] with step 3's performers where `performer-init.ly` has them,
/// an oboe of its pan at the left in each staff.
const CONTROL_DEFS: &[&str] = &[
    DEFS[0],
    "((context-name Score) (translator-type Score_performer) (accepts Staff)
      (default-child Staff) (assign tempoWholesPerMinute 15) (assign midiChannelMapping staff)
      (assign timeSignature (4 . 4))
      (consists Time_signature_performer) (consists Control_track_performer)
      (consists Tempo_performer))",
    "((context-name Staff) (translator-type Performer_group) (accepts Voice)
      (default-child Voice) (assign midiInstrument \"Oboe\") (assign midiPanPosition -1)
      (consists Staff_performer) (consists Key_performer)
      (consists Midi_control_change_performer))",
    "((context-name Voice) (translator-type Performer_group) (consists Piano_pedal_performer)
      (consists Note_performer))",
];

/// [`DEFS`] with step 4's performers where `performer-init.ly` has them:
/// marks in the Score, ties in the Voice, syllables in Lyrics of a track of
/// their own.
const MARK_DEFS: &[&str] = &[
    DEFS[0],
    "((context-name Score) (translator-type Score_performer) (accepts Staff) (accepts Lyrics)
      (default-child Staff) (assign tempoWholesPerMinute 15) (assign midiChannelMapping staff)
      (assign rehearsalMark 1)
      (consists Mark_tracking_translator) (consists Control_track_performer)
      (consists Tempo_performer) (consists Mark_performer))",
    DEFS[2],
    "((context-name Voice) (translator-type Performer_group) (consists Tie_performer)
      (consists Note_performer))",
    "((context-name Lyrics) (translator-type Performer_group) (consists Staff_performer)
      (consists Lyric_performer))",
];

/// [`DEFS`] with drums as `performer-init.ly` has them: a DrumStaff of the
/// instrument `drums`, its DrumVoice's notes `Drum_note_performer`'s.
const DRUM_DEFS: &[&str] = &[
    DEFS[0],
    "((context-name Score) (translator-type Score_performer) (accepts DrumStaff)
      (default-child DrumStaff) (assign tempoWholesPerMinute 15)
      (assign midiChannelMapping staff)
      (consists Control_track_performer) (consists Tempo_performer))",
    "((context-name DrumStaff) (translator-type Performer_group) (accepts DrumVoice)
      (default-child DrumVoice) (assign midiInstrument \"drums\") (consists Staff_performer))",
    "((context-name DrumVoice) (translator-type Performer_group) (consists Drum_note_performer))",
];

/// A VM with the music types, the makers of music, and the `\midi` of
/// [`DEFS`].
fn vm() -> Option<(Vm, Value)> {
    vm_of(DEFS)
}

/// A VM with the music types, the makers of music, and the `\midi` of
/// `defs`.
fn vm_of(defs: &[&str]) -> Option<(Vm, Value)> {
    if TYPES.is_empty() {
        eprintln!("skipped: no music types without LilyPond 2.26.0");
        return None;
    }
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    eval(
        &mut vm,
        "(set-current-module (resolve-module '(lily)))
         (define-public INF-MOMENT (ly:make-moment +inf.0))
         (define-public ZERO-MOMENT (ly:make-moment 0 1))",
    );
    let callbacks = lily_scm().unwrap().join("define-music-callbacks.scm");
    let text = std::fs::read_to_string(&callbacks).unwrap();
    vm.eval_text(&text, callbacks.to_str()).unwrap();
    lily_engine_music::define_music_types(&mut vm).unwrap();
    eval(
        &mut vm,
        "(define (note step log)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 step 0) 'duration (ly:make-duration log)))
         (define (chord . notes) (make-music 'EventChord 'elements notes))
         (define (rest log) (make-music 'RestEvent 'duration (ly:make-duration log)))
         (define (seq . elements) (make-music 'SequentialMusic 'elements elements))
         (define (sim . elements) (make-music 'SimultaneousMusic 'elements elements))
         (define (with music . events) (ly:music-set-property! music 'articulations events) music)
         (define (dynamic text) (make-music 'AbsoluteDynamicEvent 'text text))
         (define (cresc dir) (make-music 'CrescendoEvent 'span-direction dir))
         (define (decresc dir) (make-music 'DecrescendoEvent 'span-direction dir))
         (define (accent velocity) (make-music 'ArticulationEvent 'articulation-type 'accent
                                               'midi-extra-velocity velocity))
         (define (set symbol value) (make-music 'PropertySet 'symbol symbol 'value value))
         (define (at type music) (make-music 'ContextSpeccedMusic 'context-type type 'element music))
         (define (key alist) (make-music 'KeyChangeEvent 'pitch-alist alist))
         (define (pedal type dir) (make-music type 'span-direction dir))
         (define (tie) (make-music 'TieEvent))
         (define (pitched step alter log)
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 step alter) 'duration (ly:make-duration log)))
         (define (breathe)
           (make-music 'BreathingEvent
                       'midi-length (lambda (len context) (ly:moment-sub len (ly:make-moment 1/8)))))
         (define (skip log) (make-music 'SkipEvent 'duration (ly:make-duration log)))
         (define (lyric text log) (make-music 'LyricEvent 'text text 'duration (ly:make-duration log)))
         (define (format-mark n context) (list 'mark (string (integer->char (+ 64 n)))))
         (define (make-tied-lyric-markup s) (list 'tied s))
         (define (%markup? m) (pair? m))
         (define (markup->string m) (if (pair? m) (string-append \"<\" (cadr m) \">\") m))
         (define (midi-program i) (case i ((oboe) 68) ((drums) 0) (else #f)))
         (define (percussion? i) (eq? i 'drums))
         (define (drum type log)
           (make-music 'NoteEvent 'drum-type type 'duration (ly:make-duration log)))
         (define drums (make-hash-table))
         (hashq-set! drums 'bassdrum (ly:make-pitch -1 0 0))
         (hashq-set! drums 'snare (ly:make-pitch -1 1 0))
         (define (alterations-in-key l) (apply + (map (lambda (p) (round (* (cdr p) 2))) l)))
         (define (time-signature->fraction t) t)
         (define (new type id music)
           (make-music 'ContextSpeccedMusic 'context-type type 'context-id id 'create-new #t
                       'element music))
         (define-public (ly:make-event-class class) (list class))
         (define-public all-translation-properties '())
         (set-object-property! 'tempoWholesPerMinute 'translation-type? rational?)
         (set-object-property! 'midiChannelMapping 'translation-type? symbol?)
         (for-each (lambda (p) (set-object-property! p 'translation-type? (const #t)))
                   '(dynamicAbsoluteVolumeFunction instrumentEqualizer midiMinimumVolume
                     midiMaximumVolume midiInstrument midiPanPosition midiReverbLevel
                     timeSignature beatBase beatStructure rehearsalMark rehearsalMarkFormatter
                     drumPitchTable))
         (define (volumes s)
           (assoc-ref '((\"pp\" . 0.49) (\"p\" . 0.55) (\"f\" . 0.75) (\"ff\" . 0.8)) s))",
    );
    let scope = vm.make_module(&["lily"]).unwrap();
    let midi = vm.heap_mut().wrap(OutputDef::new(scope));
    vm.heap_mut().root(midi);
    for text in defs {
        let mods = eval(&mut vm, &format!("'{text}"));
        let mut items = Vec::new();
        let mut p = mods;
        while let (Some(m), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
            items.push(m);
            p = rest;
        }
        let (def, unknown) = make_context_def(vm.heap_mut(), None, &items, Value::FALSE);
        assert_eq!(unknown, 0, "{text}");
        let name = vm.heap().car(vm.heap().cdr(items[0]).unwrap()).unwrap();
        let name = vm.heap().name(name).unwrap().to_string();
        vm.module_define(scope, &name, def);
    }
    Some((vm, midi))
}

/// `music` performed and written with the name `name`: the file's bytes,
/// and what writing it warned.
fn perform(vm: &mut Vm, midi: Value, music: &str, name: &str) -> (Vec<u8>, String) {
    let music = eval(vm, music);
    vm.heap_mut().root(music);
    let done = run_performance(vm, music, midi)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    let performance = done.performance(vm.heap_mut()).unwrap();
    // Taken once, as `ly:format-output` processes it once.
    assert_eq!(done.performance(vm.heap_mut()), None);
    vm.heap_mut().root(performance);
    vm.define("performance", performance);
    eval(
        vm,
        &format!("(ly:performance-write performance \"score.midi\" \"{name}\")"),
    );
    let mut written = take_written(vm.heap_mut(), performance);
    assert_eq!(written.len(), 1);
    let (file, bytes) = written.remove(0);
    assert_eq!(file, "score.midi");
    let warned = eval(
        vm,
        "(let ((l (reverse (car %diagnostics)))) (set-car! %diagnostics '())
           (map (lambda (d) (vector-ref d 2)) l))",
    );
    let mut text = String::new();
    vm.heap().write(warned, &mut text);
    (bytes, text)
}

/// A track of `events`, each a delta and its bytes, and its end of track.
fn track(events: &[(u32, &[u8])], end: u32) -> Vec<u8> {
    let mut body = Vec::new();
    for &(delta, bytes) in events {
        lily_engine_smf::variable(delta, &mut body);
        body.extend_from_slice(bytes);
    }
    lily_engine_smf::variable(end, &mut body);
    body.extend_from_slice(b"\xff\x2f\x00");
    let mut out = b"MTrk".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend(body);
    out
}

fn text(kind: u8, s: &str) -> Vec<u8> {
    let mut out = vec![0xff, kind, s.len() as u8];
    out.extend_from_slice(s.as_bytes());
    out
}

/// The control track LilyPond writes: the name, if any, the creator and
/// LilyPond's version, then 60 quarters a minute; `end` after them.
fn control(name: &str, end: u32) -> Vec<u8> {
    let (named, creator, version) = (
        text(3, name),
        text(1, "creator: "),
        text(1, "LilyPond 2.26.0               "),
    );
    let tempo = [0xff, 0x51, 0x03, 0x0f, 0x42, 0x40];
    let mut events: Vec<(u32, &[u8])> = Vec::new();
    if !name.is_empty() {
        events.push((0, &named));
    }
    events.extend([(0, &creator[..]), (0, &version[..]), (0, &tempo[..])]);
    track(&events, end)
}

fn header(tracks: u16) -> Vec<u8> {
    let mut out = b"MThd\0\0\0\x06\0\x01".to_vec();
    out.extend_from_slice(&tracks.to_be_bytes());
    out.extend_from_slice(&384u16.to_be_bytes());
    out
}

#[test]
fn a_staff_of_notes_rests_and_a_chord_is_written_as_lilypond_writes_it() {
    let Some((mut vm, midi)) = vm() else { return };
    let (bytes, warned) = perform(
        &mut vm,
        midi,
        "(new 'Staff \"up\" (new 'Voice \"v\"
           (seq (note 0 2) (rest 2) (chord (note 2 1) (note 4 1)))))",
        "",
    );
    assert_eq!(warned, "()");
    let name = text(3, "up:v");
    let mut expected = header(2);
    expected.extend(control("", 4 * 384));
    expected.extend(track(
        &[
            (0, &name),
            (0, &[0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0]),
            (384, &[0x90, 64, 0x5a]),
            (0, &[0x90, 67, 0x5a]),
            (768, &[0x90, 64, 0]),
            (0, &[0x90, 67, 0]),
        ],
        0,
    ));
    assert_eq!(bytes, expected);
}

#[test]
fn each_staff_has_a_track_and_a_channel_and_the_name_is_the_control_tracks() {
    let Some((mut vm, midi)) = vm() else { return };
    let (bytes, _) = perform(
        &mut vm,
        midi,
        "(sim (new 'Staff \"\" (note 0 1)) (new 'Staff \"low\" (seq (rest 2) (note 1 2))))",
        "A title",
    );
    // A staff and voice of no ids have a track of no name; the second
    // staff's notes are on the second channel, its track made at its
    // first note.
    let low = text(3, "low:");
    let mut expected = header(3);
    expected.extend(control("A title", 2 * 384));
    expected.extend(track(&[(0, &[0x90, 60, 0x5a]), (768, &[0x90, 60, 0])], 0));
    expected.extend(track(
        &[(384, &low), (0, &[0x91, 62, 0x5a]), (384, &[0x91, 62, 0])],
        0,
    ));
    assert_eq!(bytes, expected);
}

#[test]
fn a_grace_note_shortens_the_note_before_it_and_the_tracks_start_before_zero() {
    let Some((mut vm, midi)) = vm() else { return };
    let (bytes, _) = perform(
        &mut vm,
        midi,
        "(new 'Staff \"\" (seq (make-music 'GraceMusic 'element (note 1 4)) (note 0 2)
           (note 0 2) (make-music 'GraceMusic 'element (note 2 4)) (note 0 2)))",
        "",
    );
    // A sixteenth of grace time is 9/40 of it, each column's ticks
    // truncated: the first grace note from -21 to 0, where the track
    // starts; the second quarter shortened to end at the second grace
    // note's column, 746, which ends at 767, a tick before the last note.
    let mut expected = header(2);
    expected.extend(control("", 3 * 384 + 21));
    expected.extend(track(
        &[
            (0, &[0x90, 62, 0x5a]),
            (21, &[0x90, 62, 0]),
            (0, &[0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0]),
            (0, &[0x90, 60, 0x5a]),
            (362, &[0x90, 60, 0]),
            (0, &[0x90, 64, 0x5a]),
            (21, &[0x90, 64, 0]),
            (1, &[0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0]),
        ],
        0,
    ));
    assert_eq!(bytes, expected);
}

#[test]
fn a_grace_note_that_ends_a_note_before_it_began_goes_back_in_midi_time() {
    let Some((mut vm, midi)) = vm() else { return };
    let (bytes, warned) = perform(
        &mut vm,
        midi,
        "(new 'Staff \"\" (seq
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2 0 24/25))
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 1 0) 'duration (ly:make-duration 2 0 1/25))
           (make-music 'GraceMusic 'element (note 2 4)) (note 3 2)))",
        "",
    );
    // As fixtures/midi-walker.ly's first bar: the grace note shortens d
    // (at 368) to end at its column, 362, before d began. Midi_walker
    // writes d's off with no delta, before its on, and the delta after it
    // is taken from 362.
    let mut expected = header(2);
    expected.extend(control("", 768));
    expected.extend(track(
        &[
            (0, &[0x90, 60, 0x5a]),
            (368, &[0x90, 60, 0]),
            (0, &[0x90, 62, 0]),
            (0, &[0x90, 62, 0x5a]),
            (0, &[0x90, 64, 0x5a]),
            (21, &[0x90, 64, 0]),
            (1, &[0x90, 65, 0x5a]),
            (384, &[0x90, 65, 0]),
        ],
        0,
    ));
    assert_eq!(bytes, expected);
    assert_eq!(warned, "(\"Going back in MIDI time.\")");
}

#[test]
fn a_note_shorter_than_a_tick_starts_and_stops_at_one_tick() {
    let Some((mut vm, midi)) = vm() else { return };
    let (bytes, warned) = perform(
        &mut vm,
        midi,
        "(new 'Staff \"\" (seq
           (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2 0 1/500))
           (note 1 2)))",
        "",
    );
    // c is 1/2000 of a whole, 0 ticks but not of no length: Midi_walker
    // starts it and stops it at 0, the off first (Midi_track::add), so it
    // is never stopped after its on.
    let mut expected = header(2);
    expected.extend(control("", 384));
    expected.extend(track(
        &[
            (0, &[0x90, 60, 0]),
            (0, &[0x90, 60, 0x5a]),
            (0, &[0x90, 62, 0x5a]),
            (384, &[0x90, 62, 0]),
        ],
        0,
    ));
    assert_eq!(bytes, expected);
    assert_eq!(warned, "()");
}

/// The velocities of the note-ons of `bytes`' last track.
fn velocities(bytes: &[u8]) -> Vec<u8> {
    let last = bytes.windows(4).rposition(|w| w == b"MTrk").unwrap();
    let body = &bytes[last + 8..];
    body.windows(3)
        .filter(|w| w[0] & 0xf0 == 0x90 && w[2] != 0)
        .map(|w| w[2])
        .collect()
}

#[test]
fn a_crescendo_to_a_dynamic_and_a_decrescendo_to_none_give_lilyponds_velocities() {
    let Some((mut vm, midi)) = vm_of(DYNAMIC_DEFS) else {
        return;
    };
    let (bytes, warned) = perform(
        &mut vm,
        midi,
        "(new 'Staff \"\" (seq (set 'dynamicAbsoluteVolumeFunction volumes)
           (with (note 0 2) (dynamic \"p\") (cresc -1)) (note 1 2) (note 2 2) (note 3 2)
           (with (note 4 2) (dynamic \"f\") (decresc -1)) (note 3 2) (note 2 2) (note 1 2)))",
        "",
    );
    assert_eq!(warned, "()");
    // p (0.55) to f (0.75) in four quarters, 127 times each volume
    // truncated; then from f to a target of its own, a decrescendo with no
    // end: 0.25 of the range from the volume and 0.1 down to 1 (0.525),
    // reached at the end of the piece.
    assert_eq!(velocities(&bytes), [69, 76, 82, 88, 95, 88, 80, 73]);
}

#[test]
fn the_volume_limits_and_the_equalizer_set_the_range_of_volumes() {
    let Some((mut vm, midi)) = vm_of(DYNAMIC_DEFS) else {
        return;
    };
    let (bytes, _) = perform(
        &mut vm,
        midi,
        "(new 'Staff \"\" (seq (set 'dynamicAbsoluteVolumeFunction volumes)
           (set 'instrumentEqualizer (lambda (s) (and (equal? s \"piano\") '(0.2 . 1.0))))
           (note 0 2) (with (note 0 2) (dynamic \"p\"))
           (set 'midiMinimumVolume 0.2) (set 'midiMaximumVolume 0.6)
           (with (note 0 2) (dynamic \"p\")) (with (note 0 2) (dynamic \"ff\"))))",
        "",
    );
    // The default volume (90/127) and p through the piano's (0.2 . 1.0),
    // then p and ff within 0.2 and 0.6, which outrank the equalizer.
    assert_eq!(velocities(&bytes), [97, 81, 53, 66]);
}

#[test]
fn an_accent_adds_its_extra_velocity_and_what_is_past_0x7e_is_0x7f() {
    let Some((mut vm, midi)) = vm_of(DYNAMIC_DEFS) else {
        return;
    };
    let (bytes, _) = perform(
        &mut vm,
        midi,
        "(new 'Staff \"\" (seq (set 'dynamicAbsoluteVolumeFunction volumes)
           (with (note 0 2) (dynamic \"p\") (accent 20)) (with (note 0 2) (dynamic \"ff\") (accent 40))
           (with (note 0 2) (accent -120))))",
        "",
    );
    // 69 + 20; 101 + 40 past 0x7e; 101 - 120 below 0, which LilyPond's
    // conversion to `uint8_t` makes 0x7f too.
    assert_eq!(velocities(&bytes), [89, 127, 127]);
}

#[test]
fn a_voices_first_dynamic_span_makes_its_staffs_track_at_the_start() {
    let Some((mut vm, midi)) = vm_of(DYNAMIC_DEFS) else {
        return;
    };
    let (bytes, _) = perform(
        &mut vm,
        midi,
        "(sim (new 'Staff \"\" (note 0 1)) (new 'Staff \"low\" (seq (rest 2) (note 1 2))))",
        "",
    );
    // As T11's: the second staff's track name at 0, not at its note.
    let low = text(3, "low:");
    let mut expected = header(3);
    expected.extend(control("", 2 * 384));
    expected.extend(track(&[(0, &[0x90, 60, 0x5a]), (768, &[0x90, 60, 0])], 0));
    expected.extend(track(
        &[(0, &low), (384, &[0x91, 62, 0x5a]), (384, &[0x91, 62, 0])],
        0,
    ));
    assert_eq!(bytes, expected);
}

#[test]
fn an_instrument_has_its_program_twice_and_its_controllers_initial_and_set_values() {
    let Some((mut vm, midi)) = vm_of(CONTROL_DEFS) else {
        return;
    };
    let (bytes, warned) = perform(
        &mut vm,
        midi,
        "(new 'Staff \"\" (seq (note 0 2) (set 'midiPanPosition 0) (set 'midiReverbLevel 2)
           (note 0 2)))",
        "",
    );
    // The staff made by `set_instrument`, whose own program comes first,
    // then the initial pan (fourteen bits of 0), the outer program and the
    // name; at the second note the pan at the centre, 0x2000 by a hair, and
    // the reverb out of range.
    assert_eq!(
        warned,
        "(\"ignoring out-of-range value change for MIDI property `midiReverbLevel'\")"
    );
    let (ts, name) = ([0xff, 0x58, 0x04, 4, 2, 24, 8], text(4, "Oboe"));
    let tempo = [0xff, 0x51, 0x03, 0x0f, 0x42, 0x40];
    let (creator, version) = (
        text(1, "creator: "),
        text(1, "LilyPond 2.26.0               "),
    );
    let mut expected = header(2);
    expected.extend(track(
        &[(0, &creator), (0, &version), (0, &ts), (0, &tempo)],
        2 * 384,
    ));
    expected.extend(track(
        &[
            (0, &[0xc0, 68]),
            (0, &[0xb0, 10, 0]),
            (0, &[0xb0, 42, 0]),
            (0, &[0xc0, 68]),
            (0, &name),
            (0, &[0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0]),
            (0, &[0xb0, 10, 0x40]),
            (0, &[0xb0, 42, 0]),
            (0, &[0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0]),
        ],
        0,
    ));
    assert_eq!(bytes, expected);
}

#[test]
fn keys_are_major_or_minor_by_the_third_and_time_signatures_follow_the_beats() {
    let Some((mut vm, midi)) = vm_of(CONTROL_DEFS) else {
        return;
    };
    let (bytes, warned) = perform(
        &mut vm,
        midi,
        "(new 'Staff \"\" (seq
           (key '((4 . 0) (5 . 0) (6 . 0) (0 . 0) (1 . 0) (2 . 0) (3 . 1/2))) (note 0 2)
           (at 'Score (set 'timeSignature '(6 . 8))) (at 'Score (set 'beatBase 1/8))
           (at 'Score (set 'beatStructure '(3 3)))
           (key '((2 . 0) (3 . 1/2) (4 . 0) (5 . 0) (6 . 0) (0 . 0) (1 . 0))) (note 0 2)
           (at 'Score (set 'timeSignature '(9 . 12))) (note 0 2)))",
        "",
    );
    assert_eq!(warned, "()");
    // 4/4 at a quarter's 24 clocks; 6/8 at 3 eighths' 36; 9/12 reduced to
    // 3/4, at the same 36. G major's one sharp; E minor's, its third flat
    // once the key is moved to C.
    let tempo = [0xff, 0x51, 0x03, 0x0f, 0x42, 0x40];
    let (creator, version) = (
        text(1, "creator: "),
        text(1, "LilyPond 2.26.0               "),
    );
    let mut expected = header(2);
    expected.extend(track(
        &[
            (0, &creator),
            (0, &version),
            (0, &[0xff, 0x58, 0x04, 4, 2, 24, 8]),
            (0, &tempo),
            (384, &[0xff, 0x58, 0x04, 6, 3, 36, 8]),
            (384, &[0xff, 0x58, 0x04, 3, 2, 36, 8]),
        ],
        384,
    ));
    let name = text(4, "Oboe");
    expected.extend(track(
        &[
            (0, &[0xc0, 68]),
            (0, &[0xb0, 10, 0]),
            (0, &[0xb0, 42, 0]),
            (0, &[0xc0, 68]),
            (0, &name),
            (0, &[0xff, 0x59, 0x02, 1, 0]),
            (0, &[0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0]),
            (0, &[0xff, 0x59, 0x02, 1, 1]),
            (0, &[0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0]),
            (0, &[0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0]),
        ],
        0,
    ));
    assert_eq!(bytes, expected);
}

#[test]
fn pedals_go_down_and_up_and_a_quarter_tone_bends_the_pitch_wheel() {
    let Some((mut vm, midi)) = vm_of(CONTROL_DEFS) else {
        return;
    };
    let (bytes, warned) = perform(
        &mut vm,
        midi,
        "(new 'Staff \"\" (seq (with (note 0 2) (pedal 'SustainEvent -1))
           (with (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 1/4) 'duration (ly:make-duration 2))
                 (pedal 'SustainEvent 1))
           (with (note 0 2) (pedal 'UnaCordaEvent 1))))",
        "",
    );
    // A quarter-tone above C is C with the wheel half a semitone up
    // (0x2800), set in the note-on's event and reset in its off's; an
    // una corda that never went down is warned of.
    assert_eq!(warned, "(\"cannot find start of piano pedal\")");
    let last = bytes.windows(4).rposition(|w| w == b"MTrk").unwrap();
    let name = text(4, "Oboe");
    let expected = track(
        &[
            (0, &[0xc0, 68]),
            (0, &[0xb0, 10, 0]),
            (0, &[0xb0, 42, 0]),
            (0, &[0xc0, 68]),
            (0, &name),
            (0, &[0xb0, 0x40, 0x7f]),
            (0, &[0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0]),
            (0, &[0xb0, 0x40, 0]),
            (0, &[0xe0, 0x00, 0x50, 0, 0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0, 0, 0xe0, 0x00, 0x40]),
            (0, &[0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0]),
        ],
        0,
    );
    assert_eq!(bytes[last..], expected);
}

#[test]
fn a_tied_note_is_one_note_and_a_breath_shortens_the_tie_before_it() {
    let Some((mut vm, midi)) = vm_of(MARK_DEFS) else {
        return;
    };
    let (bytes, warned) = perform(
        &mut vm,
        midi,
        "(new 'Staff \"\" (new 'Voice \"\" (seq (with (note 0 2) (tie)) (note 0 2) (breathe)
           (note 1 2) (with (pitched 0 1/2 2) (tie)) (pitched 1 -1/2 2) (with (note 2 2) (tie))
           (note 3 2))))",
        "",
    );
    // The two Cs are one note of a half, which the breath's eighth less
    // than the quarter since the second started shortens to three
    // eighths; C sharp is tied to D flat, of the same tones, and sounds
    // a half; the tie of E has no note to end on.
    assert_eq!(warned, "()");
    let last = bytes.windows(4).rposition(|w| w == b"MTrk").unwrap();
    let expected = track(
        &[
            (0, &[0x90, 60, 0x5a]),
            (576, &[0x90, 60, 0]),
            (192, &[0x90, 62, 0x5a]),
            (384, &[0x90, 62, 0]),
            (0, &[0x90, 61, 0x5a]),
            (768, &[0x90, 61, 0]),
            (0, &[0x90, 64, 0x5a]),
            (384, &[0x90, 64, 0]),
            (0, &[0x90, 65, 0x5a]),
            (384, &[0x90, 65, 0]),
        ],
        0,
    );
    assert_eq!(bytes[last..], expected);
}

#[test]
fn marks_are_markers_of_the_control_track_and_syllables_lyrics_of_their_own() {
    let Some((mut vm, midi)) = vm_of(MARK_DEFS) else {
        return;
    };
    let (bytes, warned) = perform(
        &mut vm,
        midi,
        "(sim (seq (at 'Score (set 'rehearsalMarkFormatter format-mark))
                   (make-music 'RehearsalMarkEvent) (skip 2) (make-music 'AdHocMarkEvent 'text \"Fine\"))
              (new 'Staff \"\" (seq (note 0 2) (note 0 2)))
              (new 'Lyrics \"la\" (seq (lyric \"a~b\" 2) (lyric '(markup \"x\") 2))))",
        "",
    );
    // The default rehearsal mark is the formatter's of `rehearsalMark`,
    // an ad-hoc mark its text, each `markup->string`'s; a syllable's
    // string is wrapped as `make-tied-lyric-markup` wraps it first, a
    // markup is not.
    assert_eq!(warned, "()");
    let (creator, version, marker, fine) = (
        text(1, "creator: "),
        text(1, "LilyPond 2.26.0               "),
        text(6, "<A>"),
        text(6, "Fine"),
    );
    let tempo = [0xff, 0x51, 0x03, 0x0f, 0x42, 0x40];
    let (name, a, x) = (text(3, "la:"), text(5, "<a~b>"), text(5, "<x>"));
    let mut expected = header(3);
    expected.extend(track(
        &[
            (0, &creator),
            (0, &version),
            (0, &tempo),
            (0, &marker),
            (384, &fine),
        ],
        384,
    ));
    expected.extend(track(
        &[
            (0, &[0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0]),
            (0, &[0x90, 60, 0x5a]),
            (384, &[0x90, 60, 0]),
        ],
        0,
    ));
    // The Lyrics' track ends where its staff does.
    expected.extend(track(&[(0, &name), (0, &a), (384, &x)], 384));
    assert_eq!(bytes, expected);
}

#[test]
fn drums_are_their_tables_pitches_on_channel_10_the_last_of_a_chord_first() {
    let Some((mut vm, midi)) = vm_of(DRUM_DEFS) else {
        return;
    };
    let (bytes, warned) = perform(
        &mut vm,
        midi,
        "(sim (at 'Score (set 'drumPitchTable drums))
              (new 'DrumStaff \"\" (new 'DrumVoice \"\"
                (seq (chord (drum 'bassdrum 2) (drum 'snare 2)) (with (drum 'snare 2) (accent 20))
                     (drum 'cowbell 2) (drum 'bassdrum 2)))))",
        "",
    );
    // The staff of `drums` is percussion: its program, twice, and its
    // notes are on channel 10, each drum the pitch the table has for it
    // and a cowbell, which it lacks, none; the chord's snare is announced
    // before its bass drum.
    assert_eq!(warned, "()");
    let last = bytes.windows(4).rposition(|w| w == b"MTrk").unwrap();
    let name = text(4, "drums");
    let expected = track(
        &[
            (0, &[0xc9, 0]),
            (0, &[0xc9, 0]),
            (0, &name),
            (0, &[0x99, 50, 0x5a]),
            (0, &[0x99, 48, 0x5a]),
            (384, &[0x99, 50, 0]),
            (0, &[0x99, 48, 0]),
            (0, &[0x99, 50, 0x6e]),
            (384, &[0x99, 50, 0]),
            (384, &[0x99, 48, 0x5a]),
            (384, &[0x99, 48, 0]),
        ],
        0,
    );
    assert_eq!(bytes[last..], expected);
}
