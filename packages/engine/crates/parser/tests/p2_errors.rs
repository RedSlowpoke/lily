//! P2, part 2 (02 §5.6, §3.4): syntax errors in Bison's terms, with what
//! Bison expects where it lists it, and recovery where LilyPond's `error`
//! productions put it. Each expected message was printed by LilyPond 2.26.0
//! (`lilypond --loglevel=WARNING` with `LANGUAGE=en`) for the same input,
//! less the warnings of processing a score; the error corpus's cases 08, 09,
//! 12 and 30 are scored with C0 by `tools/compare/test/c0-cases.test.mjs`.
//! Every test skips itself, and so verifies nothing, without that lilypond.

use lily_engine_parser::syntax::{Assignment, ItemKind, Value};
use lily_engine_parser::{NoteNames, parse};
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
    let parse = parse(
        name,
        format!("{V}{text}").into_bytes(),
        names()?,
        &mut Vm::new(),
    );
    Some(String::from_utf8_lossy(&parse.stderr()).into_owned())
}

/// The heads of the messages for `text` after the `\version` line:
/// `file:line:column: severity: message`, or `severity: message` for one
/// with no location.
fn heads(name: &str, text: &str) -> Option<Vec<String>> {
    Some(
        stderr(name, text)?
            .lines()
            .filter(|l| {
                l.contains(": error: ")
                    || l.contains(": warning: ")
                    || l.starts_with("error: ")
                    || l.starts_with("warning: ")
            })
            .map(String::from)
            .collect(),
    )
}

#[test]
fn bison_lists_what_could_follow_where_four_or_fewer_tokens_could() {
    let Some(h) = heads("e01.ly", "\\header title = \"x\"\n") else {
        return;
    };
    assert_eq!(
        h,
        ["e01.ly:2:9: error: syntax error, unexpected SYMBOL, expecting '{'"]
    );
    // After `=`, a token no value starts with reduces a mid-rule action, and
    // only `\header` is left (case 03's `expecting \header`).
    assert_eq!(
        heads(
            "e02.ly",
            "\\header { title \"x\" }\n\\header { composer = }\n"
        )
        .unwrap(),
        [
            "e02.ly:2:19: error: syntax error, unexpected STRING, expecting '.' or '='",
            "e02.ly:3:22: error: syntax error, unexpected '}', expecting \\header"
        ]
    );
    assert_eq!(
        heads("e03.ly", "\\score c\n").unwrap(),
        ["e03.ly:2:8: error: syntax error, unexpected NOTENAME_PITCH, expecting '{'"]
    );
    // An unknown command is a string, and so starts an assignment.
    assert_eq!(
        heads("e07.ly", "x { c }\n").unwrap(),
        ["e07.ly:2:3: error: syntax error, unexpected '{', expecting '.' or '='"]
    );
    assert_eq!(
        stderr("e06.ly", "\\notemode c4").unwrap(),
        "e06.ly:2:11: error: syntax error, unexpected NOTENAME_PITCH, expecting \
         \\sequential or \\simultaneous or << or '{'\n\\notemode \n          c4\n\
         e06.ly:2:12: error: Unfinished main input\n\\notemode c\n           4\n\
         fatal error: failed files: \"e06.ly\"\n"
    );
}

#[test]
fn a_score_body_expects_its_close_and_has_one_music() {
    // `score_items` is reduced to `score_body`, and its action run, before
    // Bison detects the error.
    let Some(h) = heads("e04.ly", "\\score { = { c } } }\n") else {
        return;
    };
    assert_eq!(
        h,
        [
            "error: Missing music in \\score",
            "e04.ly:2:10: error: syntax error, unexpected '=', expecting '}'"
        ]
    );
    // LilyPond then fails in init.ly on the empty score, which is processing.
    assert_eq!(
        heads("e18.ly", "\\score { }\n\\score { { c } { d } e }\n").unwrap(),
        [
            "error: Missing music in \\score",
            "e18.ly:3:16: error: Spurious expression in \\score",
            "e18.ly:3:22: error: Spurious expression in \\score"
        ]
    );
    // An error inside an item recovers at toplevel, not in the score.
    assert_eq!(
        heads("e19.ly", "\\score { c4 - = } c4 = d\n").unwrap(),
        ["e19.ly:2:15: error: syntax error, unexpected '='"]
    );
}

#[test]
fn no_syntax_error_is_reported_until_three_tokens_are_shifted() {
    let Some(h) = heads("e09.ly", "{ } } c foo } c\n") else {
        return;
    };
    assert_eq!(h, ["e09.ly:2:5: error: syntax error, unexpected '}'"]);
    assert_eq!(
        heads("e10.ly", "{ c4 = d e } { f = }\n").unwrap(),
        [
            "e10.ly:2:6: error: syntax error, unexpected '='",
            "e10.ly:2:1: error: errors found, ignoring music expression"
        ]
    );
    assert_eq!(
        heads("e11.ly", "{ { c4 = } d4 = e }\n").unwrap(),
        [
            "e11.ly:2:8: error: syntax error, unexpected '='",
            "e11.ly:2:15: error: syntax error, unexpected '='",
            "e11.ly:2:1: error: errors found, ignoring music expression"
        ]
    );
    assert_eq!(
        heads("e23.ly", "{ c4 } }\n{ d4 } } e\n").unwrap(),
        [
            "e23.ly:2:8: error: syntax error, unexpected '}'",
            "e23.ly:3:8: error: syntax error, unexpected '}'"
        ]
    );
    // An error with no token shifted since the last discards its token: the
    // `=` after `\notemode`, and then `{ c4 = }` is toplevel music.
    assert_eq!(
        heads("e30.ly", "\\notemode = { c4 = } \\notemode {").unwrap(),
        [
            "e30.ly:2:11: error: syntax error, unexpected '=', expecting \\sequential or \
             \\simultaneous or << or '{'",
            "e30.ly:2:18: error: syntax error, unexpected '='",
            "e30.ly:2:13: error: errors found, ignoring music expression",
            "e30.ly:2:32: error: Unfinished main input",
            "e30.ly:2:32: error: syntax error, unexpected end of input"
        ]
    );
}

#[test]
fn music_lists_resume_at_the_next_music_or_their_close() {
    let Some(h) = heads("e20.ly", "{ c4 \\header d e f g }\n{ -. = c }\n") else {
        return;
    };
    assert_eq!(
        h,
        [
            "e20.ly:2:6: error: syntax error, unexpected \\header",
            "e20.ly:2:1: error: errors found, ignoring music expression",
            "e20.ly:3:6: error: syntax error, unexpected '='",
            "e20.ly:3:3: warning: Unattached ArticulationEvent",
            "e20.ly:3:1: error: errors found, ignoring music expression"
        ]
    );
    assert_eq!(
        heads("e24.ly", "<< c4 } >> { d >> e }\n").unwrap(),
        [
            "e24.ly:2:7: error: syntax error, unexpected '}'",
            "e24.ly:2:1: error: errors found, ignoring music expression",
            "e24.ly:2:16: error: syntax error, unexpected >>",
            "e24.ly:2:12: error: errors found, ignoring music expression"
        ]
    );
}

#[test]
fn the_end_of_input_aborts_the_parse_except_at_toplevel() {
    let Some(h) = heads("e12.ly", "{ c4 -") else {
        return;
    };
    assert_eq!(
        h,
        ["e12.ly:2:6: error: syntax error, unexpected end of input"]
    );
    assert_eq!(
        heads("e13.ly", "\\score {").unwrap(),
        [
            "error: Missing music in \\score",
            "e13.ly:2:8: error: syntax error, unexpected end of input, expecting '}'"
        ]
    );
    // Aborted in the list: its music reaches no toplevel handler.
    assert_eq!(
        heads("e14.ly", "{ c = }\n{").unwrap(),
        ["e14.ly:3:1: error: syntax error, unexpected end of input"]
    );
    // A header has no `error` rule: toplevel recovers, and takes the end.
    assert_eq!(
        heads("e15.ly", "\\header { title = \"x\"").unwrap(),
        ["e15.ly:2:21: error: syntax error, unexpected end of input"]
    );
    // `\notemode` and `\markup` left their lexer modes pushed.
    assert_eq!(
        stderr("e26.ly", "\\notemode { c").unwrap(),
        "e26.ly:2:13: error: Unfinished main input\n\\notemode { \n            c\n\
         e26.ly:2:13: error: syntax error, unexpected end of input\n\\notemode { \n            c\n\
         fatal error: failed files: \"e26.ly\"\n"
    );
    assert_eq!(
        heads("e27.ly", "x = \\notemode { c = \n").unwrap(),
        [
            "e27.ly:2:20: error: Unfinished main input",
            "e27.ly:2:20: error: syntax error, unexpected end of input"
        ]
    );
    assert_eq!(
        heads("e31.ly", "<< \\notemode { c } -. \\notemode >> d").unwrap(),
        [
            "e31.ly:2:33: error: syntax error, unexpected >>, expecting \\sequential or \
             \\simultaneous or << or '{'",
            "e31.ly:2:1: error: errors found, ignoring music expression",
            "e31.ly:2:36: error: Unfinished main input"
        ]
    );
}

#[test]
fn music_with_a_syntax_error_is_ignored_where_it_would_make_a_score() {
    // Toplevel music and a score's music, after the next token is lexed;
    // not an assignment's.
    let Some(h) = heads(
        "e16.ly",
        "{ c4 = d }\n\\notemode { e4 = }\nx = { c4 = }\n\\score { { c4 = } }\n<< { d = } >>\n",
    ) else {
        return;
    };
    assert_eq!(
        h,
        [
            "e16.ly:2:6: error: syntax error, unexpected '='",
            "e16.ly:2:1: error: errors found, ignoring music expression",
            "e16.ly:3:16: error: syntax error, unexpected '='",
            "e16.ly:3:11: error: errors found, ignoring music expression",
            "e16.ly:4:10: error: syntax error, unexpected '='",
            "e16.ly:5:15: error: syntax error, unexpected '='",
            "e16.ly:5:10: error: errors found, ignoring music expression"
        ]
    );
}

#[test]
fn post_events_that_no_music_before_them_takes_are_unattached() {
    let Some(h) = heads(
        "e17.ly",
        "{ { R1 } -. }\n{ <c e> { q } -1 }\n{ << c >> -\"x\" \\1 }\n{ { } :8 ^- }\n\
         { { c4 { } } _. }\n<< \\notemode { c } -- >>\n{ -. -> c4 }\n<< -> c >>\n",
    ) else {
        return;
    };
    // After music that takes none, a run's first; at a list's start, all.
    assert_eq!(
        h,
        [
            "e17.ly:2:10: warning: Unattached ArticulationEvent",
            "e17.ly:4:11: warning: Unattached TextScriptEvent",
            "e17.ly:5:7: warning: Unattached TremoloEvent",
            "e17.ly:6:14: warning: Unattached ArticulationEvent",
            "e17.ly:8:3: warning: Unattached ArticulationEvent",
            "e17.ly:8:6: warning: Unattached ArticulationEvent",
            "e17.ly:9:4: warning: Unattached ArticulationEvent"
        ]
    );
    // An unattached run that ends a list became an event chord, which takes
    // the next.
    assert_eq!(
        heads("e05.ly", "{ c4 { -. } -> }\n").unwrap(),
        ["e05.ly:2:8: warning: Unattached ArticulationEvent"]
    );
}

#[test]
fn assignments_take_strings_words_numbers_and_music() {
    let Some(names) = names() else {
        return;
    };
    let text = format!(
        "{V}x = {{ c4 }} y = \"s\" z = 4 w = c4 v = \\notemode {{ d }}\n\\header {{ title = {{ c }} opus = 3 }}\n"
    );
    let parse = parse("e21.ly", text.into_bytes(), names, &mut Vm::new());
    assert_eq!(String::from_utf8_lossy(&parse.stderr()), "");
    let values: Vec<_> = parse
        .items
        .iter()
        .flat_map(|item| match &item.kind {
            ItemKind::Assignment(a) => vec![a],
            ItemKind::Header(fields) => fields.iter().collect(),
            kind => panic!("{kind:?}"),
        })
        .map(|Assignment { value, .. }| matches!(value, Value::Music(_)))
        .collect();
    assert_eq!(values, [true, false, false, true, true, true, false]);
}
