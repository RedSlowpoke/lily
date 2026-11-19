//! P9 (02 §6): `\repeat` of each type, with `\alternative` after it and in
//! its body, its count as a number, a variable and Scheme, and its type as
//! a string and Scheme; `\tempo` with a text, a metronome mark, both, a
//! range and markup; `\sequential` and `\simultaneous`, after `\notemode`
//! too; and P8's `--`, `__` and `\lyricsto`, brought forward for T02; in a
//! file's module that uses a `(lily)` loaded from the installed LilyPond
//! 2.26.0 (`tests/support`).
//!
//! - [`INPUTS`]: the music each makes, compiled by that lilypond with
//!   `tools/compare/music.ly` and compared node by node.
//! - [`CASES`]: what LilyPond prints for P9's actions' errors, the
//!   constructors' warnings and syntax errors in each rule, each expected
//!   stderr printed by LilyPond 2.26.0 (`lilypond --loglevel=WARNING`) for
//!   the same input; none warns while it is processed (D64).
//!
//! The token sequences of P9's scores (T17, T19, T29) and of T02 are
//! compared with LilyPond's by `tools/compare/test/parse-refs.test.mjs`.
//! Every test skips itself, and so verifies nothing, without that lilypond.

use lily_engine_parser::{NoteNames, parse};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

mod support;

const V: &str = "\\version \"2.26.0\"\n";

/// The installed `scm/lily/`, or None (and a note on stderr) without lilypond.
fn lily() -> Option<&'static Path> {
    static LILY: OnceLock<Option<PathBuf>> = OnceLock::new();
    let lily = LILY.get_or_init(support::lily_scm);
    if lily.is_none() {
        eprintln!("skipped: no LilyPond 2.26.0 installed");
    }
    lily.as_deref()
}

/// The installed pitch names.
fn names() -> &'static NoteNames {
    static NAMES: OnceLock<NoteNames> = OnceLock::new();
    NAMES.get_or_init(|| {
        let text = std::fs::read(lily().unwrap().join("define-note-names.scm")).unwrap();
        NoteNames::read(&text).expect("define-note-names.scm reads")
    })
}

/// Inputs that LilyPond compiles without a message.
const INPUTS: &[(&str, &str)] = &[
    (
        "types",
        "{ \\repeat unfold 2 { c4 d } \\repeat volta 2 { e4 } \\alternative { { f4 } { g4 } } \
         \\repeat tremolo 4 { c16 d } \\repeat percent 2 { c4 } \\repeat segno 2 c4 \
         \\repeat volta 2 c4 \\alternative { d4 } }",
    ),
    (
        "alternatives",
        "{ \\repeat volta 3 { c4 \\alternative { { d4 } { e4 } } } \
         \\repeat volta 2 { c4 } \\alternative { \\volta 1 { d4 } \\volta 2 { e4 } } \
         \\repeat volta 4 { c4 \\repeat unfold 2 { d4 \\alternative { e f } } } \\alternative { g a } }",
    ),
    (
        "counts",
        "n = 3\nt = \"unfold\"\n\
         { \\repeat unfold \\n c4 \\repeat unfold #2 d4 \\repeat unfold 0 e4 \
         \\repeat \"unfold\" 2 f4 \\repeat #\"volta\" 2 g4 \\repeat \\t 2 a4 \\repeat unfold 2 <c e>4 }",
    ),
    (
        "tempo",
        "{ \\tempo 4 = 96 c4 \\tempo \"Allegro\" d4 \\tempo \"Andante\" 8. = 60 e4 \
         \\tempo \\markup \\bold Presto 2 = 40-50 f4 \\tempo #\"Lento\" g4 \\tempo Adagio a4 \
         \\tempo 4 = #(+ 1 2) b4 }",
    ),
    (
        "heads",
        "\\sequential { c4 d } \\simultaneous { e4 g }\n\
         { \\notemode \\sequential { c4 } \\notemode \\simultaneous { d4 e } \\notemode << f4 >> }",
    ),
    (
        "values",
        "rep = \\repeat unfold 2 c4\ntem = \\tempo 4 = 60\nalt = \\alternative { d4 e }\n\
         \\score { { \\rep \\tem \\repeat volta 2 c4 \\alt } \\midi { \\tempo 2 = 30 } \\layout { } }",
    ),
    (
        "contexts",
        "\\new Staff \\repeat unfold 2 c4\n\\relative c' \\repeat unfold 2 { c d }\n\
         \\new Voice \\tempo 4 = 60",
    ),
    (
        "lyrics",
        "words = \\lyricmode { a -- b __ c d4 e2. }\n\
         << \\new Voice = \"v\" { c4 d e f g } \\new Lyrics \\lyricsto \"v\" { \\words } >>\n\
         \\new Lyrics \\lyricsto \"x\" \\words",
    ),
];

#[test]
fn p9_makes_lilyponds_music() {
    let Some(lily) = lily() else { return };
    let mut vm = support::library(lily).expect("the library loads");
    for (name, input) in INPUTS {
        let text = format!("{V}{input}\n");
        let file = format!("{name}.ly");
        let expected = support::lilypond_trees(&file, &text);
        let parse = parse(&file, text.into_bytes(), names(), &mut vm);
        let stderr = String::from_utf8_lossy(&parse.stderr()).into_owned();
        assert_eq!(stderr, "", "{name}");
        let got = support::music_trees(&mut vm, &parse).expect("the trees are written");
        support::same(&expected, &got, name);
        lily_engine_parser::end_session(&mut vm);
    }
}

/// Each input after [`V`], and LilyPond's stderr for it:
/// - `make-repeat`'s warnings: an unknown type (no place), and more
///   alternatives than repeats, at the body and at the alternatives, after
///   and in the body;
/// - `alternative`'s error for music that is not sequential;
/// - the actions' errors: `not an unsigned integer` of Scheme and of a
///   variable, `simple string expected`, `markup expected`, `not a
///   duration`;
/// - syntax errors: a repeat with no body or a Scheme one, `\sequential`
///   without braces, `\tempo` alone, `\alternative` of a note.
const CASES: &[(&str, &str)] = &[
    (
        r##"{ \repeat foo 2 c4 }"##,
        r##"warning: unknown repeat type `"foo"': must be volta, unfold, percent, or tremolo
"##,
    ),
    (
        r##"{ \repeat volta 2 c4 \alternative { d e f } }"##,
        r##"f.ly:2:19: warning: More alternatives than repeats.  Junking excess alternatives
{ \repeat volta 2 
                  c4 \alternative { d e f } }
f.ly:2:22: warning: More alternatives than repeats.  Junking excess alternatives
{ \repeat volta 2 c4 
                     \alternative { d e f } }
"##,
    ),
    (
        r##"{ \repeat volta 2 { c4 \alternative { d e f } } }"##,
        r##"f.ly:2:19: warning: More alternatives than repeats.  Junking excess alternatives
{ \repeat volta 2 
                  { c4 \alternative { d e f } } }
f.ly:2:24: warning: More alternatives than repeats.  Junking excess alternatives
{ \repeat volta 2 { c4 
                       \alternative { d e f } } }
"##,
    ),
    (
        r##"{ \repeat unfold #-1 c4 }"##,
        r##"f.ly:2:18: error: not an unsigned integer
{ \repeat unfold 
                 #-1 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"n = -2 { \repeat unfold \n c4 }"##,
        r##"f.ly:2:25: error: not an unsigned integer
n = -2 { \repeat unfold 
                        \n c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \repeat #5 2 c4 }"##,
        r##"f.ly:2:11: error: simple string expected
{ \repeat 
          #5 2 c4 }
warning: unknown repeat type `""': must be volta, unfold, percent, or tremolo
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \repeat volta 2 c4 \alternative << d4 >> }"##,
        r##"f.ly:2:35: error: sequential music required
{ \repeat volta 2 c4 \alternative 
                                  << d4 >> }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \tempo #5 c4 }"##,
        r##"f.ly:2:10: error: markup expected
{ \tempo 
         #5 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \tempo 3 = 60 c4 }"##,
        r##"f.ly:2:10: error: not a duration
{ \tempo 
         3 = 60 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \repeat unfold 2 }"##,
        r##"f.ly:2:20: error: syntax error, unexpected '}'
{ \repeat unfold 2 
                   }
f.ly:2:1: error: errors found, ignoring music expression

{ \repeat unfold 2 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \repeat unfold 2 #(make-music (quote NoteEvent)) }"##,
        r##"f.ly:2:20: error: syntax error, unexpected SCM_TOKEN
{ \repeat unfold 2 
                   #(make-music (quote NoteEvent)) }
f.ly:2:1: error: errors found, ignoring music expression

{ \repeat unfold 2 #(make-music (quote NoteEvent)) }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \sequential c4 }"##,
        r##"f.ly:2:15: error: syntax error, unexpected NOTENAME_PITCH, expecting '{'
{ \sequential 
              c4 }
f.ly:2:1: error: errors found, ignoring music expression

{ \sequential c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \tempo }"##,
        r##"f.ly:2:10: error: syntax error, unexpected '}'
{ \tempo 
         }
f.ly:2:1: error: errors found, ignoring music expression

{ \tempo }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \alternative c4 }"##,
        r##"f.ly:2:16: error: syntax error, unexpected NOTENAME_PITCH
{ \alternative 
               c4 }
f.ly:2:1: error: errors found, ignoring music expression

{ \alternative c4 }
fatal error: failed files: "f.ly"
"##,
    ),
];

#[test]
fn p9_errors_are_lilyponds() {
    let Some(lily) = lily() else { return };
    let mut vm = support::library(lily).expect("the library loads");
    for (input, expected) in CASES {
        let text = format!("{V}{input}\n");
        let parse = parse("f.ly", text.into_bytes(), names(), &mut vm);
        // The trees are made, as the book handler makes them, so that the
        // constructors' warnings come.
        support::music_trees(&mut vm, &parse).expect("the trees are written");
        let stderr = String::from_utf8_lossy(&parse.stderr()).into_owned();
        assert_eq!(stderr, *expected, "{input}");
        lily_engine_parser::end_session(&mut vm);
    }
}
