//! P7 (02 §6): markup, in a file's module that uses a `(lily)` loaded from
//! the installed LilyPond 2.26.0 (`tests/support`), whose
//! `define-markup-commands.scm` gives the commands their signatures: markup
//! mode, commands as heads and alone, braced lists, markup list commands,
//! Scheme markup and markup lists, LilyPond in braces as an argument,
//! `\markuplist`, `\score` in markup, markup in scripts, as an argument, a
//! value and a `scalar`, `\markup name = …`, and a part's command, which
//! loads its part (D74).
//!
//! - [`INPUTS`]: the music each makes, compiled by that lilypond with
//!   `tools/compare/music.ly` and compared node by node.
//! - [`CASES`]: what LilyPond 2.26.0 prints for an unknown command (case
//!   19's), a value that is no markup, arguments of the wrong type, which
//!   end the run with Guile's error (case 20's, D64), markup outside a
//!   text script, `Not a markup function`, and syntax errors, each expected
//!   stderr printed by it (`lilypond --loglevel=WARNING`) for the same
//!   input. Cases 19 and 20 are scored with C0 through the core by
//!   `tools/compare/test/c0-cases.test.mjs`, and the token sequences of
//!   these inputs and of T13 and T27 are compared with LilyPond's by
//!   `tools/compare/test/parse-refs.test.mjs`.
//!
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

/// Inputs that LilyPond compiles without an error.
const INPUTS: &[(&str, &str)] = &[
    (
        "scripts",
        r#"{ c4^\markup \bold x d4_\markup { a \italic b } e4-\markup \column { a b } f4^"s" g4-w a4^#(markup #:italic "y") }"#,
    ),
    (
        "heads",
        r#"{ c4^\markup \bold { a \italic b \line { c d } } d4^\markup \raise #0.5 \hbracket \bold \italic e
  e4^\markup \override #'(baseline-skip . 2) \column { a b } f4^\markup \combine "X" \bold "+" }"#,
    ),
    (
        "scheme",
        r#"m = \markup \bold x
l = \markuplist { p \italic q }
{ c4^\markup { \m #(markup #:italic "y") #"z" #(if #f #f) \l } d4^\markup \column \l e4^\m }"#,
    ),
    (
        "lists",
        r#"{ c4^\markup \column { \wordwrap-lines { a b } c } d4^\markup \bold \wordwrap-lines { e f } }"#,
    ),
    (
        "values",
        r#"t = \markup \italic t
{ \set Staff.instrumentName = \markup \bold Hi c4^\t \mark \markup \box A d4 }"#,
    ),
    (
        "functions",
        r#"fn = #(define-music-function (m) (markup?)
  (make-music 'NoteEvent 'duration (ly:make-duration 2) 'pitch (ly:make-pitch 0 0)
              'articulations (list (make-music 'TextScriptEvent 'text m))))
{ \fn \markup \bold x \fn \markup { y z } }"#,
    ),
    (
        "braced",
        r#"{ c4^\markup \note {4.} #UP d4^\markup \note-by-number #2 #0 #UP }"#,
    ),
    (
        "define",
        r#"\markup bolder = #bold-markup
{ c4^\markup \bolder x }"#,
    ),
    (
        "toplevel",
        r#"\markup \bold Title
\markuplist { a b }
\markup \score { { c4 } }
{ d4 }"#,
    ),
    (
        "part",
        r#"{ c4^\markup \fret-diagram "6-x;5-x;4-o;3-2;2-3;1-2;" }"#,
    ),
];

#[test]
fn markup_makes_lilyponds_music() {
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
    }
}

/// Each input after [`V`], and LilyPond's stderr for it.
const CASES: &[(&str, &str)] = &[
    (
        r##"\markup { \boldd text }"##,
        r##"f.ly:2:11: error: unknown command: `\boldd'
\markup { 
          \boldd text }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\markup { #5 }"##,
        r##"f.ly:2:11: error: not a markup
\markup { 
          #5 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\markup \draw-circle #"a" #0.3 ##f"##,
        r##"f.ly:2:22: error: wrong type for argument 1.  Expecting number, found "a"
\markup \draw-circle 
                     #"a" #0.3 ##f
ERROR: In procedure ly:parse-file:
In procedure reverse!: Wrong type argument in position 1: (#f 0.3 "a" . #f)
"##,
    ),
    (
        r##"\markup \draw-circle #"a" #"b" ##f"##,
        r##"f.ly:2:22: error: wrong type for argument 1.  Expecting number, found "a"
\markup \draw-circle 
                     #"a" #"b" ##f
f.ly:2:27: error: wrong type for argument 2.  Expecting number, found "b"
\markup \draw-circle #"a" 
                          #"b" ##f
ERROR: In procedure ly:parse-file:
In procedure reverse!: Wrong type argument in position 1: (#f "b" "a" . #f)
"##,
    ),
    (
        r##"{ c4^\markup \note {4.} #"up" }"##,
        r##"f.ly:2:25: error: wrong type for argument 2.  Expecting number, found "up"
{ c4^\markup \note {4.} 
                        #"up" }
ERROR: In procedure ly:parse-file:
In procedure reverse!: Wrong type argument in position 1: ("up" #<Duration 4. > . #f)
"##,
    ),
    (
        r##"{ \markup x c4 }"##,
        r##"f.ly:2:3: error: markup outside of text script or \lyricmode
{ 
  \markup x c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\markup x = 5"##,
        r##"f.ly:2:13: error: Not a markup function
\markup x = 
            5
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\markup \bold }"##,
        r##"f.ly:2:15: error: syntax error, unexpected '}'
\markup \bold 
              }
f.ly:2:16: error: Unfinished main input
\markup \bold }
               
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\markup \column x"##,
        r##"f.ly:2:17: error: syntax error, unexpected SYMBOL
\markup \column 
                x
f.ly:2:18: error: Unfinished main input
\markup \column x
                 
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\markuplist \bold x"##,
        r##"f.ly:2:19: error: syntax error, unexpected SYMBOL
\markuplist \bold 
                  x
f.ly:2:20: error: Unfinished main input
\markuplist \bold x
                   
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\markup { a \markuplist b }"##,
        r##"f.ly:2:13: error: syntax error, unexpected \markuplist
\markup { a 
            \markuplist b }
f.ly:2:28: error: Unfinished main input
\markup { a \markuplist b }
                           
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\markup \combine { a } b"##,
        r##"f.ly:2:18: error: syntax error, unexpected '{'
\markup \combine 
                 { a } b
f.ly:2:20: error: not a note name: a
\markup \combine { 
                   a } b
f.ly:2:25: error: Unfinished main input
\markup \combine { a } b
                        
f.ly:2:25: error: syntax error, unexpected end of input, expecting '.' or '='
\markup \combine { a } b
                        
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ c4^#5 }"##,
        r##"f.ly:2:6: error: not an articulation
{ c4^
     #5 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ c4^\markup \bold { a #'x } }"##,
        r##"f.ly:2:24: error: not a markup
{ c4^\markup \bold { a 
                       #'x } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"fn = #(define-music-function (m) (markup?) (make-music 'TextScriptEvent 'text m))
{ c4 \fn \markuplist { l } }"##,
        r##"f.ly:3:10: error: wrong type for argument 1.  Expecting markup, found (list "l")
{ c4 \fn 
         \markuplist { l } }
fatal error: failed files: "f.ly"
"##,
    ),
];

#[test]
fn markup_errors_print_what_lilypond_prints() {
    let Some(lily) = lily() else { return };
    let mut vm = support::library(lily).expect("the library loads");
    for (input, expected) in CASES {
        let text = format!("{V}{input}\n");
        let parse = parse("f.ly", text.into_bytes(), names(), &mut vm);
        let stderr = String::from_utf8_lossy(&parse.stderr()).into_owned();
        assert_eq!(&stderr, expected, "{input}");
    }
}
