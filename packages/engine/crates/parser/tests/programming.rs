//! Programming errors as LilyPond prints them (`flower/warn.cc`,
//! `Input::programming_error`, MS3 step 13a): one given at an input
//! location, as layout gives a grob's at its origin, prints that location
//! and the excerpt, then the location again before `continuing, cross
//! fingers`; one at none prints its two lines without a place. Either is
//! one `ly:expect-warning` can expect. The probe of the fixture's grobs
//! compares them with LilyPond's stderr (`tools/compare/test/grobs.test.mjs`).
//! Every test skips itself, and so verifies nothing, without that lilypond.

use lily_engine_parser::{NoteNames, parse};

mod support;

const V: &str = "\\version \"2.26.0\"\n";

/// What LilyPond writes to stderr for `text` after the `\version` line,
/// parsed in a VM whose `(lily)` loaded the library files; None (and a
/// note on stderr) without lilypond.
fn stderr(text: &str) -> Option<String> {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0 installed");
        return None;
    };
    let names = NoteNames::read(&std::fs::read(lily.join("define-note-names.scm")).ok()?)
        .expect("define-note-names.scm reads");
    let mut vm = support::library(&lily).expect("the library loads");
    let parse = parse("p.ly", format!("{V}{text}").into_bytes(), &names, &mut vm);
    Some(String::from_utf8_lossy(&parse.stderr()).into_owned())
}

#[test]
fn a_located_programming_error_prints_its_place_and_excerpt_before_continuing() {
    let diagnose = "(@@ (lily) %diagnose) 'programming-error";
    let Some(got) = stderr(&format!(
        "#(ly:programming-error \"plain ~a\" 1)\n\
         {{ c'4 #({diagnose} (*location*) \"at the hash\") d'4 }}\n\
         #(ly:warning \"after\")\n"
    )) else {
        return;
    };
    assert_eq!(
        got,
        "programming error: plain 1\n\
         continuing, cross fingers\n\
         p.ly:3:8: programming error: at the hash\n\
         { c'4 #\n       \
         ((@@ (lily) %diagnose) 'programming-error (*location*) \"at the hash\") d'4 }\n\
         p.ly:3:8: continuing, cross fingers\n\
         warning: after\n"
    );
}

#[test]
fn a_located_programming_error_is_one_an_earlier_call_can_expect() {
    let Some(got) = stderr(
        "#(ly:expect-warning \"at the\")\n\
         #((@@ (lily) %diagnose) 'programming-error (*location*) \"at the hash\")\n\
         #((@@ (lily) %diagnose) 'programming-error (*location*) \"at the end\")\n",
    ) else {
        return;
    };
    assert_eq!(
        got,
        "p.ly:4:2: programming error: at the end\n\
         #\n \
         ((@@ (lily) %diagnose) 'programming-error (*location*) \"at the end\")\n\
         p.ly:4:2: continuing, cross fingers\n"
    );
}

#[test]
fn a_cpp_warnings_text_is_translated_for_ly_expect_warning() {
    // `ly:translate-cpp-warning-scheme` (warn-scheme.cc, M01's): `~`
    // doubled, `%%` a `%`, `%s` and `%d` each `~a`, another `%` a `~`.
    let Some(got) = stderr(
        "#(ly:warning \"~a\" (ly:translate-cpp-warning-scheme \"a %s of %d%%, ~ and %f\"))\n\
         #(ly:expect-warning (ly:translate-cpp-warning-scheme \"got %s\") \"it\")\n\
         #(ly:warning \"got it\")\n",
    ) else {
        return;
    };
    assert_eq!(got, "warning: a ~a of ~a%, ~~ and ~f\n");
}
