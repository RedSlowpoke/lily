//! P6, part 2 (02 §6): `\set`, `\unset`, `\override` and `\revert` in music,
//! as values and in output definitions, `\once` and `\temporary` of them,
//! their paths (dotted, with `,`, Scheme and quoted parts, split with no
//! `.`), the values they take (`scalar`), and the property operations of
//! `\with { … }` and `\context { … }`, in a file's module that uses a
//! `(lily)` loaded from the installed LilyPond 2.26.0 (`tests/support`),
//! with `lily.scm`'s init data: `define-grobs.scm` gives the grobs' names
//! `is-grob?`, and `define-grob-properties.scm` the properties their types.
//!
//! - [`INPUTS`]: the music each makes, compiled by that lilypond with
//!   `tools/compare/music.ly` and compared node by node.
//! - [`CASES`]: what LilyPond prints for wrong paths and types, syntax
//!   errors in each rule, the deprecated paths with no `.`, and inputs
//!   that print nothing, with `$defaultlayout`'s context definitions,
//!   which an output definition's music modifies (the whole init's,
//!   `tests/support`'s `library`), each expected stderr printed by LilyPond 2.26.0
//!   (`lilypond --loglevel=WARNING`) for the same input; none warns while
//!   it is processed (D64). Cases 16 and 29 of the error corpus are scored
//!   with C0 by `tools/compare/test/c0-cases.test.mjs`, and the token
//!   sequences of these constructs and of P6's scores are compared with
//!   LilyPond's by `tools/compare/test/tokens.test.mjs`.
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

/// Inputs that LilyPond compiles without an error: each operation with and
/// without its context, paths of strings, of Scheme and with `,`, `\once`
/// and `\temporary` of each, values of every kind `scalar` takes (a word,
/// a string, numbers, a negative one, Scheme, `$`, a pitch, music, a
/// symbol list), a `\with`'s operations, and operations as variables.
const INPUTS: &[(&str, &str)] = &[
    (
        "operations",
        "{ \\override Staff.Stem.color = #red \\revert Stem.color \\revert Staff.Stem.color \
         \\revert #'(Staff Stem color) \\set Staff.instrumentName = \"x\" \
         \\unset Staff.instrumentName \\set fontSize = -2 \\unset fontSize \
         \\override Stem.details.beamed-lengths = #'(1 2) \\override Beam.positions = #'(1 . 2) \
         \\override Staff.\"Stem\".color = #blue \\override #'(Staff Stem color) = #red c4 }",
    ),
    (
        "once",
        "{ \\once \\override Stem.color = #red \\temporary \\override Beam.positions = #'(1 . 2) \
         \\once \\set fontSize = 2 \\once \\revert Stem.color \\once \\unset fontSize c4 }",
    ),
    (
        "values",
        "{ \\set Staff.instrumentName = Guitar \\override Stem.length = 3.5 \
         \\override Stem.length = -1 \\set Staff.midiInstrument = \"acoustic guitar (steel)\" \
         \\set fingeringOrientations = #'(left) \\override NoteHead.style = #'cross \
         \\set Timing.measureLength = #(ly:make-moment 3/4) \\set Staff.tonic = d' \
         \\set Staff.instrumentName = { c4 } \\set Staff.instrumentName = Stem.color \
         \\override Stem.direction = #UP \\set Staff.fontSize = $(+ 1 2) c4 }",
    ),
    (
        "with",
        "\\new Staff \\with { \\override Stem.color = #red \\revert Stem.color \\unset fontSize \
         instrumentName = \"G\" \"shortInstrumentName\" = \"g\" #'fontSize = 2 \\consists \"Foo\" \
         \\override Glissando.bound-details.right.padding = #1 } { c4 }",
    ),
    (
        "variables",
        "myUp = \\override Stem.direction = #UP\n\
         myRevert = \\revert Stem.direction\n\
         size = \\set Staff.fontSize = 1\n\
         { \\myUp c4 \\myRevert \\size c4 }",
    ),
    (
        "separators",
        "\\new Voice \\with { \\override NoteHead.color = #red } \
         { \\override Voice.Stem.color = #blue c4 \\set Staff . fontSize = 1 \\set Staff, fontSize = 2 }",
    ),
];

#[test]
fn properties_make_lilyponds_music() {
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

/// Each input after [`V`], and LilyPond's stderr for it:
/// - the actions' errors: `bad grob property path` (case 16's, a path too
///   long, or a split path whose first part is not a context and a grob),
///   `bad context property path`, the syntax functions' `wrong type for
///   argument` at the whole call;
/// - the paths with no `.` (case 29's), in music, in `\with` and after a
///   context, with LilyPond's warning;
/// - syntax errors in each rule and what Bison lists there;
/// - property operations at toplevel (an error), in `\context { … }` and
///   in `\layout`, where an unknown command in a definition is recovered
///   from in the output definition's body, its `\revert` read as music;
/// - `symbol '=' scalar` of Scheme and strings in `\with`, `symbol
///   expected`, and `\once` of `\revert` and `\unset`.
const CASES: &[(&str, &str)] = &[
    (
        r##"{ \override color = #red c4 }"##,
        r##"f.ly:2:13: error: bad grob property path
{ \override 
            color = #red c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \set Staff.Foo.bar = 1 c4 }"##,
        r##"f.ly:2:8: error: bad context property path
{ \set 
       Staff.Foo.bar = 1 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \unset Foo.bar.baz c4 }"##,
        r##"f.ly:2:10: error: bad context property path
{ \unset 
         Foo.bar.baz c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \set #'() = 1 c4 }"##,
        r##"f.ly:2:8: error: bad context property path
{ \set 
       #'() = 1 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \override Staff.Stem.color.x Foo = 1 c4 }"##,
        r##"f.ly:2:13: error: bad grob property path
{ \override 
            Staff.Stem.color.x Foo = 1 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \override Staff.#"x" = 1 c4 }"##,
        r##"f.ly:2:13: error: bad grob property path
{ \override 
            Staff.#"x" = 1 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \set Staff.#5 = 1 c4 }"##,
        r##"f.ly:2:3: error: wrong type for argument 2.  Expecting symbol, found 5
{ 
  \set Staff.#5 = 1 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \set 3 = 1 c4 }"##,
        r##"f.ly:2:3: error: wrong type for argument 2.  Expecting symbol, found 3
{ 
  \set 3 = 1 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \override 3.Stem.color = #red c4 }"##,
        r##"f.ly:2:3: error: wrong type for argument 1.  Expecting symbol, found 3
{ 
  \override 3.Stem.color = #red c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \revert Staff #'color c4 }"##,
        r##"f.ly:2:17: warning: deprecated: missing `.' in property path color
{ \revert Staff 
                #'color c4 }
f.ly:2:25: error: syntax error, unexpected NOTENAME_PITCH
{ \revert Staff #'color 
                        c4 }
f.ly:2:1: error: errors found, ignoring music expression

{ \revert Staff #'color c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \revert Stem.color.x \revert #'(Staff Stem color) \revert Staff.Stem #'color c4 }"##,
        r##"f.ly:2:72: warning: deprecated: missing `.' in property path Stem.color
{ \revert Stem.color.x \revert #'(Staff Stem color) \revert Staff.Stem 
                                                                       #'color c4 }
"##,
    ),
    (
        r##"{ \override NoteHead #'color #'x = 1 c4 }"##,
        r##"f.ly:2:22: warning: deprecated: missing `.' in property path NoteHead.color
{ \override NoteHead 
                     #'color #'x = 1 c4 }
f.ly:2:30: error: syntax error, unexpected SCM_TOKEN, expecting '='
{ \override NoteHead #'color 
                             #'x = 1 c4 }
f.ly:2:30: warning: Ignoring non-music expression
{ \override NoteHead #'color 
                             #'x = 1 c4 }
f.ly:2:1: error: errors found, ignoring music expression

{ \override NoteHead #'color #'x = 1 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \revert NoteHead #'color #'x c4 }"##,
        r##"f.ly:2:20: warning: deprecated: missing `.' in property path NoteHead.color
{ \revert NoteHead 
                   #'color #'x c4 }
f.ly:2:28: warning: Ignoring non-music expression
{ \revert NoteHead #'color 
                           #'x c4 }
"##,
    ),
    (
        r##"\new Staff \with { \override Stem #'color = #red } { c4 }"##,
        r##"f.ly:2:35: warning: deprecated: missing `.' in property path Stem.color
\new Staff \with { \override Stem 
                                  #'color = #red } { c4 }
"##,
    ),
    (
        r##"{ \revert Stem c4 }"##,
        r##"f.ly:2:16: error: syntax error, unexpected NOTENAME_PITCH
{ \revert Stem 
               c4 }
f.ly:2:1: error: errors found, ignoring music expression

{ \revert Stem c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \override Stem.color c4 }"##,
        r##"f.ly:2:24: error: syntax error, unexpected NOTENAME_PITCH, expecting '='
{ \override Stem.color 
                       c4 }
f.ly:2:1: error: errors found, ignoring music expression

{ \override Stem.color c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \set x c4 }"##,
        r##"f.ly:2:10: error: syntax error, unexpected NOTENAME_PITCH, expecting '='
{ \set x 
         c4 }
f.ly:2:1: error: errors found, ignoring music expression

{ \set x c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \override Stem.color = }"##,
        r##"f.ly:2:26: error: syntax error, unexpected '}', expecting \header
{ \override Stem.color = 
                         }
f.ly:2:1: error: errors found, ignoring music expression

{ \override Stem.color = }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \override Stem.color = - x c4 }"##,
        r##"f.ly:2:28: error: syntax error, unexpected SYMBOL, expecting UNSIGNED or NUMBER_IDENTIFIER or REAL
{ \override Stem.color = - 
                           x c4 }
f.ly:2:28: error: not a note name: x
{ \override Stem.color = - 
                           x c4 }
f.ly:2:1: error: errors found, ignoring music expression

{ \override Stem.color = - x c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \override Stem. = 1 c4 }"##,
        r##"f.ly:2:19: error: syntax error, unexpected '='
{ \override Stem. 
                  = 1 c4 }
f.ly:2:1: error: errors found, ignoring music expression

{ \override Stem. = 1 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \set = 1 c4 }"##,
        r##"f.ly:2:8: error: syntax error, unexpected '='
{ \set 
       = 1 c4 }
f.ly:2:1: error: errors found, ignoring music expression

{ \set = 1 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \override = 1 c4 }"##,
        r##"f.ly:2:13: error: syntax error, unexpected '='
{ \override 
            = 1 c4 }
f.ly:2:1: error: errors found, ignoring music expression

{ \override = 1 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \revert = c4 }"##,
        r##"f.ly:2:11: error: syntax error, unexpected '='
{ \revert 
          = c4 }
f.ly:2:1: error: errors found, ignoring music expression

{ \revert = c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\new Staff \with { \revert Stem c4 } { c4 }"##,
        r##"f.ly:2:33: error: syntax error, unexpected NOTENAME_PITCH
\new Staff \with { \revert Stem 
                                c4 } { c4 }
f.ly:2:44: error: Unfinished main input
\new Staff \with { \revert Stem c4 } { c4 }
                                           
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\new Staff \with { \override Stem = 1 } { c4 }"##,
        r##"f.ly:2:35: error: syntax error, unexpected '='
\new Staff \with { \override Stem 
                                  = 1 } { c4 }
f.ly:2:47: error: Unfinished main input
\new Staff \with { \override Stem = 1 } { c4 }
                                              
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\new Staff \with { x 1 } { c4 }"##,
        r##"f.ly:2:22: error: syntax error, unexpected UNSIGNED, expecting '='
\new Staff \with { x 
                     1 } { c4 }
f.ly:2:32: error: Unfinished main input
\new Staff \with { x 1 } { c4 }
                               
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\new Staff \with { \unset 4 } { c4 }"##,
        r##"f.ly:2:27: error: syntax error, unexpected UNSIGNED, expecting SCM_IDENTIFIER or SCM_TOKEN or STRING or SYMBOL
\new Staff \with { \unset 
                          4 } { c4 }
f.ly:2:37: error: Unfinished main input
\new Staff \with { \unset 4 } { c4 }
                                    
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\new Staff \with { \override Stem.color 1 } { c4 }"##,
        r##"f.ly:2:41: error: syntax error, unexpected UNSIGNED, expecting '='
\new Staff \with { \override Stem.color 
                                        1 } { c4 }
f.ly:2:51: error: Unfinished main input
\new Staff \with { \override Stem.color 1 } { c4 }
                                                  
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \override Staff.Stem.color = #red c4 } \override Stem.color = #red"##,
        r##"f.ly:2:42: error: syntax error, unexpected \override
{ \override Staff.Stem.color = #red c4 } 
                                         \override Stem.color = #red
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\layout { \context { \override Stem.color = #red \revert Beam.positions instrumentName = "x" \unset fontSize } }"##,
        r##""##,
    ),
    (
        r##"\layout { \context { \Foo \revert Arpeggio.stencil } }"##,
        r##"f.ly:2:22: error: unknown command: `\Foo'
\layout { \context { 
                     \Foo \revert Arpeggio.stencil } }
f.ly:2:27: error: syntax error, unexpected \revert, expecting '='
\layout { \context { \Foo 
                          \revert Arpeggio.stencil } }
f.ly:2:54: error: syntax error, unexpected '}'
\layout { \context { \Foo \revert Arpeggio.stencil } 
                                                     }
f.ly:2:55: error: Unfinished main input
\layout { \context { \Foo \revert Arpeggio.stencil } }
                                                      
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\layout { \override Stem.color = #red \revert Stem.color }"##,
        r##""##,
    ),
    (
        r##"\new Staff \with { #(string->symbol "fontSize") = 2 $"fontSize" = 3 "fontSize" = 4 } { c4 }"##,
        r##""##,
    ),
    (
        r##"\layout { \context { #5 = 1 } }"##,
        r##"f.ly:2:22: error: symbol expected
\layout { \context { 
                     #5 = 1 } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"{ \once \revert Stem.color c4 \once \unset fontSize }"##,
        r##""##,
    ),
];

#[test]
fn properties_print_lilyponds_messages() {
    let Some(lily) = lily() else { return };
    let mut vm = support::library(lily).expect("the library loads");
    let mut failures = Vec::new();
    for &(input, expected) in CASES {
        let parse = parse(
            "f.ly",
            format!("{V}{input}\n").into_bytes(),
            names(),
            &mut vm,
        );
        let got = String::from_utf8_lossy(&parse.stderr()).into_owned();
        if got != expected {
            failures.push(format!("{input}\n--- expected\n{expected}--- got\n{got}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}
