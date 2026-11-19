//! P2, part 1 (02 §6, §5.3): the notes core in spanned syntax, with the
//! pitch names of the installed LilyPond 2.26.0's `define-note-names.scm`.
//! Each expected message was printed by LilyPond 2.26.0 (`lilypond
//! --loglevel=WARNING` with `LANGUAGE=en`) for the same input; the error
//! corpus's cases 10, 13 and 14 are scored with C0 by
//! `tools/compare/test/c0-cases.test.mjs`. Every test skips itself, and so
//! verifies nothing, without that lilypond.

use lily_engine_parser::syntax::{
    ChordElement, Direction, Duration, ItemKind, Music, MusicKind, Note, PostEventKind,
};
use lily_engine_parser::{Lexer, Mode, NoteNames, Pitch, Source, Tok, parse};
use lily_engine_scheme::Vm;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

const V: &str = "\\version \"2.24.0\"\n";

/// `scm/lily/define-note-names.scm` of the installed LilyPond 2.26.0, if there is one.
fn installed() -> Option<PathBuf> {
    let version = Command::new("lilypond").arg("--version").output().ok()?;
    if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
        return None;
    }
    let path = Command::new("sh")
        .args(["-c", "command -v lilypond"])
        .output()
        .ok()?;
    let binary = std::fs::canonicalize(String::from_utf8(path.stdout).ok()?.trim()).ok()?;
    let file = binary
        .parent()?
        .join("../share/lilypond/2.26.0/scm/lily/define-note-names.scm");
    file.exists().then_some(file)
}

/// The installed tables, or None (and a note on stderr) without lilypond.
fn names() -> Option<&'static NoteNames> {
    static NAMES: OnceLock<Option<NoteNames>> = OnceLock::new();
    let names = NAMES.get_or_init(|| {
        let text = std::fs::read(installed()?).ok()?;
        Some(NoteNames::read(&text).expect("define-note-names.scm reads"))
    });
    if names.is_none() {
        eprintln!("skipped: no LilyPond 2.26.0 installed");
    }
    names.as_ref()
}

fn stderr(name: &str, text: &str) -> Option<String> {
    let parse = parse(name, text.as_bytes().to_vec(), names()?, &mut Vm::new());
    Some(String::from_utf8_lossy(&parse.stderr()).into_owned())
}

/// The heads, `file:line:column: severity: message`, of each message.
fn heads(name: &str, text: &str) -> Option<Vec<String>> {
    Some(
        stderr(name, text)?
            .lines()
            .filter(|l| l.contains(": error: ") || l.contains(": warning: "))
            .map(String::from)
            .collect(),
    )
}

/// The elements of the one toplevel `{ … }` of `text`, with no message.
fn music(text: &str) -> Option<Vec<Music>> {
    let parse = parse(
        "m.ly",
        format!("{V}{text}").into_bytes(),
        names()?,
        &mut Vm::new(),
    );
    assert_eq!(String::from_utf8_lossy(&parse.stderr()), "");
    let [item] = &parse.items[..] else {
        panic!("{:?}", parse.items)
    };
    let ItemKind::Music(Music {
        kind: MusicKind::Sequential(elements),
        ..
    }) = &item.kind
    else {
        panic!("{item:?}")
    };
    Some(elements.clone())
}

fn pitch(octave: i8, step: u8, alteration: i8) -> Pitch {
    Pitch {
        octave,
        step,
        alteration,
    }
}

fn note(music: &Music) -> &Note {
    match &music.kind {
        MusicKind::Note(note) => note,
        kind => panic!("{kind:?}"),
    }
}

fn written(log: i32, dots: u8, factor: (u64, u64)) -> (i32, u8, (u64, u64)) {
    (log, dots, factor)
}

fn shape(d: &Duration) -> (i32, u8, (u64, u64)) {
    (d.log, d.dots, d.factor)
}

#[test]
fn every_language_and_alias_of_define_note_names_is_read() {
    let Some(names) = names() else { return };
    let languages: Vec<&str> = names.languages_names().collect();
    assert_eq!(
        languages,
        [
            "nederlands",
            "català",
            "deutsch",
            "english",
            "español",
            "français",
            "italiano",
            "norsk",
            "português",
            "suomi",
            "svenska",
            "vlaams",
            "catalan",
            "espanol",
            "semi-german",
            "portugues"
        ]
    );
    // The file's 910 ly:make-pitch calls, one name each.
    let total: usize = languages[..12]
        .iter()
        .map(|l| names.language(l).unwrap().len())
        .sum();
    assert_eq!(total, 910);
    let get = |l: &str, n: &str| names.language(l).unwrap().get(n.as_bytes());
    assert_eq!(get("nederlands", "c"), Some(pitch(-1, 0, 0)));
    assert_eq!(get("nederlands", "ceseh"), Some(pitch(-1, 0, -3)));
    assert_eq!(get("nederlands", "bes"), Some(pitch(-1, 6, -2)));
    assert_eq!(get("nederlands", "cs"), None);
    assert_eq!(get("english", "cs"), Some(pitch(-1, 0, 2)));
    assert_eq!(get("deutsch", "h"), Some(pitch(-1, 6, 0)));
    assert_eq!(get("deutsch", "b"), Some(pitch(-1, 6, -2)));
    assert_eq!(get("français", "ré"), Some(pitch(-1, 1, 0)));
    assert_eq!(get("catalan", "re"), get("català", "re"));
    assert!(get("catalan", "re").is_some());
}

#[test]
fn notes_mode_words_are_pitches_rests_and_symbols_by_flex_rules() {
    let Some(names) = names() else { return };
    let source = Source::new("t.ly", b"c cis r s q R rest r-. r_ r-x foo \\rest".to_vec());
    let mut lexer = Lexer::new(&source, 0);
    lexer.set_pitch_names(names.language("nederlands").unwrap());
    let kinds: Vec<Tok> = std::iter::from_fn(|| Some(lexer.next(Mode::Notes)))
        .take_while(|t| t.kind != Tok::Eof)
        .map(|t| t.kind)
        .collect();
    use Tok::*;
    assert_eq!(
        kinds,
        [
            NotenamePitch,
            NotenamePitch,
            RestName,
            RestName,
            ChordRepetition,
            MultiMeasureRest,
            Symbol,
            RestName,
            Char(b'-'),
            Char(b'.'),
            RestName,
            Char(b'_'),
            Symbol,
            Symbol,
            Keyword(lily_engine_parser::lexer::keyword("rest"))
        ]
    );
    // In initial mode a pitch name is a word.
    let source = Source::new("t.ly", b"c r".to_vec());
    let mut lexer = Lexer::new(&source, 0);
    lexer.set_pitch_names(names.language("nederlands").unwrap());
    assert_eq!(lexer.next(Mode::Initial).kind, Tok::Symbol);
    assert_eq!(lexer.next(Mode::Initial).kind, Tok::Symbol);
}

#[test]
fn notes_carry_octave_marks_accidental_marks_and_octave_checks() {
    let Some(m) = music("{ c'' bes,! fis?? e=' g!?='' }") else {
        return;
    };
    let notes: Vec<_> = m.iter().map(note).collect();
    assert_eq!(notes[0].pitch, pitch(1, 0, 0));
    assert_eq!(notes[1].pitch, pitch(-2, 6, -2));
    assert!(notes[1].force_accidental && !notes[1].cautionary);
    assert!(notes[2].cautionary && !notes[2].force_accidental);
    assert_eq!(notes[3].octave_check, Some(1));
    assert_eq!(notes[4].octave_check, Some(2));
    assert!(notes[4].force_accidental && notes[4].cautionary);
    assert!(notes.iter().all(|n| n.duration == Duration::new(2, 0)));
}

#[test]
fn durations_have_dots_and_multipliers_and_set_the_default_in_parse_order() {
    let Some(m) = music("{ c4.. d8*2/3*3 e f1*1/2 16 r s2 R1*4 q }") else {
        return;
    };
    let durations: Vec<Duration> = m
        .iter()
        .map(|m| match &m.kind {
            MusicKind::Note(n) => n.duration,
            MusicKind::Rhythm { duration, .. }
            | MusicKind::Rest { duration, .. }
            | MusicKind::MultiMeasureRest { duration, .. }
            | MusicKind::ChordRepetition { duration, .. } => *duration,
            kind => panic!("{kind:?}"),
        })
        .collect();
    assert_eq!(
        durations.iter().map(shape).collect::<Vec<_>>(),
        [
            written(2, 2, (1, 1)),
            written(3, 0, (2, 1)),
            written(3, 0, (2, 1)),
            written(0, 0, (1, 2)),
            written(4, 0, (1, 1)),
            written(4, 0, (1, 1)),
            written(1, 0, (1, 1)),
            written(0, 0, (4, 1)),
            written(0, 0, (4, 1)),
        ]
    );
    assert!(durations[2].span.is_none() && durations[1].span.is_some());
    // A duration is a note's whatever white space stands between them.
    assert!(matches!(m[4].kind, MusicKind::Rhythm { .. }));
    assert!(matches!(m[5].kind, MusicKind::Rest { skip: false, .. }));
    assert!(matches!(m[6].kind, MusicKind::Rest { skip: true, .. }));
    // Across toplevel items: after `{ c8 }`, `{ d }` holds d8 (02 §3.3).
    let Some(names) = names() else { return };
    let parse = parse(
        "d.ly",
        format!("{V}{{ c8 }}\n{{ d }}").into_bytes(),
        names,
        &mut Vm::new(),
    );
    let ItemKind::Music(Music {
        kind: MusicKind::Sequential(second),
        ..
    }) = &parse.items[1].kind
    else {
        panic!()
    };
    assert_eq!(shape(&note(&second[0]).duration), written(3, 0, (1, 1)));
}

#[test]
fn chords_take_their_duration_and_post_events_attach_with_directions() {
    let Some(m) = music("{ <c e-1 g>4.-. c-1^2_3\\1 c:16 c: c4\\rest-\"x\"^foo-> }") else {
        return;
    };
    let MusicKind::Chord {
        elements,
        duration,
        post,
    } = &m[0].kind
    else {
        panic!("{:?}", m[0])
    };
    assert_eq!(shape(duration), written(2, 1, (1, 1)));
    assert_eq!(elements.len(), 3);
    let ChordElement::Note { note: e, span } = &elements[1] else {
        panic!()
    };
    assert_eq!(e.duration.log, 2);
    assert_eq!(e.post[0].kind, PostEventKind::Fingering(1));
    // `e-1`, after the version line and `{ <c `.
    let e_at = (V.len() + 5) as u32;
    assert_eq!((span.start, span.end), (e_at, e_at + 3));
    assert_eq!(post[0].kind, PostEventKind::Script(b'.'));
    let kinds = |m: &Music| -> Vec<(Direction, PostEventKind)> {
        note(m)
            .post
            .iter()
            .map(|p| (p.direction, p.kind.clone()))
            .collect()
    };
    use Direction::*;
    assert_eq!(
        kinds(&m[1]),
        [
            (Neutral, PostEventKind::Fingering(1)),
            (Up, PostEventKind::Fingering(2)),
            (Down, PostEventKind::Fingering(3)),
            (Neutral, PostEventKind::StringNumber(1))
        ]
    );
    // `:` alone repeats the last tremolo, 8 at first.
    assert_eq!(kinds(&m[2]), [(Neutral, PostEventKind::Tremolo(16))]);
    assert_eq!(kinds(&m[3]), [(Neutral, PostEventKind::Tremolo(16))]);
    assert_eq!(
        kinds(&m[4]),
        [
            (Neutral, PostEventKind::Text("x".into())),
            (Up, PostEventKind::Text("foo".into())),
            (Neutral, PostEventKind::Script(b'>'))
        ]
    );
    assert!(note(&m[4]).rest);
}

#[test]
fn simultaneous_music_nests_and_spans_cover_their_text() {
    let Some(names) = names() else { return };
    let text = format!("{V}<< {{ c4 }} \\\\ d >>\n{{ }}");
    let parse = parse("s.ly", text.clone().into_bytes(), names, &mut Vm::new());
    assert_eq!(parse.items.len(), 2);
    let ItemKind::Music(music) = &parse.items[0].kind else {
        panic!()
    };
    let at = |s: &str| text.find(s).unwrap() as u32;
    assert_eq!((music.span.start, music.span.end), (at("<<"), at(">>") + 2));
    let MusicKind::Simultaneous(elements) = &music.kind else {
        panic!()
    };
    // `\\` is a shorthand, undefined until P3: an error and a string.
    assert!(matches!(elements[0].kind, MusicKind::Sequential(_)));
    assert_eq!(
        (elements[0].span.start, elements[0].span.end),
        (at("{"), at("}") + 1)
    );
}

#[test]
fn misplaced_octave_marks_are_reported_where_the_note_ends() {
    let Some(h) = heads("a7.ly", &format!("{V}{{ c4'' }}")) else {
        return;
    };
    assert_eq!(h, ["a7.ly:2:5: error: octave marks must precede duration"]);
    assert_eq!(
        heads("quotes2.ly", &format!("{V}{{ c?' }}")).unwrap(),
        ["quotes2.ly:2:5: error: badly placed octave marks"]
    );
    // After the token that ends the note is lexed.
    assert_eq!(
        heads("o1.ly", &format!("{V}{{ c4' \\foo }}")).unwrap(),
        [
            "o1.ly:2:7: error: unknown command: `\\foo'",
            "o1.ly:2:5: error: octave marks must precede duration",
            "o1.ly:2:7: error: string outside of text script or \\lyricmode"
        ]
    );
}

#[test]
fn durations_that_are_no_power_of_two_are_no_durations() {
    let Some(h) = heads("dur7.ly", &format!("{V}{{ c7 }}")) else {
        return;
    };
    assert_eq!(h, ["dur7.ly:2:4: error: not a duration"]);
    for (name, text, head) in [
        ("b1.ly", "{ c4 r3 }", "b1.ly:2:7: error: not a duration"),
        ("a9.ly", "{ <c e>3 }", "a9.ly:2:8: error: not a duration"),
        ("a4.ly", "{ c4:3 }", "a4.ly:2:6: error: not a duration"),
        ("dur0.ly", "{ c0 }", "dur0.ly:2:4: error: not a duration"),
    ] {
        assert_eq!(heads(name, &format!("{V}{text}")).unwrap(), [head]);
    }
    // A duration's dots need the next token; `:3` does not.
    assert_eq!(
        heads("a5.ly", &format!("{V}{{ c3. \\foo }}")).unwrap(),
        [
            "a5.ly:2:7: error: unknown command: `\\foo'",
            "a5.ly:2:4: error: not a duration",
            "a5.ly:2:7: error: string outside of text script or \\lyricmode"
        ]
    );
    assert_eq!(
        heads("a6.ly", &format!("{V}{{ 3 \\foo }}")).unwrap(),
        [
            "a6.ly:2:5: error: unknown command: `\\foo'",
            "a6.ly:2:3: error: not a duration",
            "a6.ly:2:5: error: string outside of text script or \\lyricmode"
        ]
    );
    assert_eq!(
        heads("o4.ly", &format!("{V}{{ c:3 \\foo }}")).unwrap(),
        [
            "o4.ly:2:5: error: not a duration",
            "o4.ly:2:7: error: unknown command: `\\foo'",
            "o4.ly:2:7: error: string outside of text script or \\lyricmode"
        ]
    );
}

#[test]
fn words_and_strings_in_notes_are_no_notes() {
    let Some(h) = heads("a1.ly", &format!("{V}{{ foo \\foo }}")) else {
        return;
    };
    // A word is reduced before the next token is lexed.
    assert_eq!(
        h,
        [
            "a1.ly:2:3: error: not a note name: foo",
            "a1.ly:2:7: error: unknown command: `\\foo'",
            "a1.ly:2:7: error: string outside of text script or \\lyricmode"
        ]
    );
    assert_eq!(
        heads("l1.ly", &format!("{V}{{ foo4-1 \"x\"8 }}")).unwrap(),
        [
            "l1.ly:2:3: error: not a note name: foo",
            "l1.ly:2:12: error: string outside of text script or \\lyricmode"
        ]
    );
}

#[test]
fn a_syntax_error_in_a_note_resumes_at_the_list_with_the_token_there() {
    let Some(h) = heads("mult.ly", &format!("{V}{{ c4*x }}")) else {
        return;
    };
    assert_eq!(
        h,
        [
            "mult.ly:2:6: error: syntax error, unexpected SYMBOL",
            "mult.ly:2:6: error: not a note name: x",
            "mult.ly:2:1: error: errors found, ignoring music expression"
        ]
    );
    assert_eq!(
        heads("b2.ly", &format!("{V}{{ c4 r4' }}")).unwrap(),
        [
            "b2.ly:2:8: error: syntax error, unexpected '\\''",
            "b2.ly:2:1: error: errors found, ignoring music expression"
        ]
    );
    // Music is no toplevel item without braces.
    assert_eq!(
        stderr("bare.ly", &format!("{V}c4 d e")).unwrap(),
        "bare.ly:2:1: error: syntax error, unexpected NOTENAME_PITCH\n\nc4 d e\n\
         fatal error: failed files: \"bare.ly\"\n"
    );
}

#[test]
fn a_syntax_error_in_markup_leaves_the_lexer_in_markup_mode() {
    // No rule pops the markup mode after the error (P7, MS2 step 14): the
    // mode stays pushed until the end of input (step 16b), and the lexer
    // reads on in it, so that mk2's `c4` is a word, as in LilyPond 2.26.0.
    let Some(h) = heads("mk1.ly", &format!("{V}\\markup {{ a\n")) else {
        return;
    };
    assert_eq!(
        h,
        [
            "mk1.ly:2:12: error: Unfinished main input",
            "mk1.ly:2:12: error: syntax error, unexpected end of input"
        ]
    );
    assert_eq!(
        heads("mk2.ly", &format!("{V}\\markup }} {{ c4 }}\n")).unwrap(),
        [
            "mk2.ly:2:9: error: syntax error, unexpected '}'",
            "mk2.ly:2:13: error: not a note name: c4",
            "mk2.ly:2:17: error: Unfinished main input"
        ]
    );
}
