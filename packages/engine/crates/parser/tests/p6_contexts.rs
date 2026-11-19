//! P6, part 1 (02 §6): `\new`, `\context`, `= "id"`, `\with` and its
//! context mods, `\context { … }` in output definitions, and `\change`, in
//! a file's module that uses a `(lily)` loaded from the installed LilyPond
//! 2.26.0 (`tests/support`).
//!
//! - [`INPUTS`]: the music each makes, compiled by that lilypond with
//!   `tools/compare/music.ly` and compared node by node, as `tests/music.rs`
//!   compares P2–P5's.
//! - [`CASES`]: what LilyPond prints for errors, values and context mods,
//!   each expected stderr printed by LilyPond 2.26.0 (`lilypond
//!   --loglevel=WARNING`) for the same input; none warns while it is
//!   processed (D64). Case 15 of the error corpus is scored with C0 by
//!   `tools/compare/test/c0-cases.test.mjs`, and the token sequences of
//!   these constructs and of P6's scores are compared with LilyPond's by
//!   `tools/compare/test/tokens.test.mjs`.
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

/// Inputs of P6's contexts, one or more scores each, that LilyPond compiles
/// without an error or a warning: every prefix with and without `= id`,
/// each `\with` (braces, a variable, a `#` and a `$` form, music made a
/// context modification), every `context_def_mod`, the music a prefix takes
/// (notes, rests, chords, `R`, more prefixes), post-events attached through
/// `ContextSpeccedMusic`, a `\with` as an argument, music as a variable's
/// value, `\context { … }` blocks and their values in `\layout`, and
/// `\change`.
const INPUTS: &[(&str, &str)] = &[
    (
        "prefixes",
        "m = \\with { \\consists \"Foo\" \\remove Bar_engraver \\accepts Baz \\denies \"Q\" \
         \\alias Staff \\defaultchild Voice \\type \"Engraver_group\" \\name Who \
         \\description \"d\" \\consists #'x }\n\
         \\new Staff = \"one\" \\with { \\consists \"Foo\" } \\with \\m { c4 \\change Staff = two d }\n\
         \\context Voice c4\n\
         \\new Staff \\new Voice = x { c4 }",
    ),
    (
        "contextable",
        "\\new Staff r4 \\new Staff R1 \\new Staff <c e>4 \\new Staff \\new Voice c4-1 \
         \\context Staff = \"a\" \\with { } d",
    ),
    ("attached", "{ \\new Voice { c4 } -1 c4 }"),
    ("value", "x = \\new Staff { c4 }\n{ \\x }"),
    (
        "scheme",
        "\\new Staff = $\"a\" \\with $(ly:make-context-mod) { c4 }\n\
         \\new Staff = #\"b\" \\with #(ly:make-context-mod '((consists \"X\"))) { c4 }\n\
         \\new #'Staff \\with { #(make-music 'NoteEvent) } { c4 }",
    ),
    (
        "change",
        "{ \\change Voice = \"x\" \\change \"Staff\" = up \\change #'Staff = #\"ab\" c4 }",
    ),
    // T25's (MS2 step 36): `\change` as a variable's value, defined before
    // the token after it is lexed, so `\csh` right after it is known.
    (
        "change value",
        "csh = \\change Staff = \"high\"\n\\csh\n\
         csl = \\change Staff = #\"low\"\n{ c4 \\csl d \\csh }",
    ),
    (
        "argument",
        "fn = #(define-music-function (m x) (ly:context-mod? ly:music?) (context-spec-music x 'Staff #f m))\n\
         { \\fn \\with { \\consists \"A\" } c4 }",
    ),
    (
        "definitions",
        "m = \\with { \\consists \"A\" \\description \"x\" }\n\
         dd = \\context { \\consists \"x\" \\m }\n\
         \\layout { \\context { \\dd \\remove \"y\" \\with \\m } \\context { } }\n\
         \\layout { x = \\context { \\alias \"S\" } \\context { \\x } }\n\
         \\new Staff \\with \\m { c4 }",
    ),
];

#[test]
fn contexts_make_lilyponds_music() {
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
/// - syntax errors after `\with`, in its braces, after a mod's keyword, in
///   `\context { … }`, after `\new` and `=`, and the `Unfinished main input`
///   of the notes mode that `\with {`, and music or `\context` in an output
///   definition, leave pushed;
/// - the actions' errors: `only \consists and \remove take non-string
///   argument.`, `symbol expected`, `simple string expected`, `not a context
///   mod`, a context definition at toplevel;
/// - music made a context modification, with `Music unsuitable for
///   context-mod`;
/// - `ly:make-context-mod`, `ly:context-mod?`, `ly:add-context-mod` and
///   `ly:get-context-mods`, with their errors, a context mod's print, and
///   `\name` copying one (`try_special_identifiers`).
///
/// Not here, as the engine differs: what LilyPond warns while it processes
/// the scores (an unknown context or translator, a context to change from),
/// and the `programming error` of a `Context_def` given a mod of no known
/// tag, N3's (part 2). An uninterned symbol (`\new #5`'s) prints as its
/// name, where Guile prints `#<uninterned-symbol undefined …>`.
const CASES: &[(&str, &str)] = &[
    (
        r##"\new Staff \with 5 { c4 }"##,
        r##"f.ly:2:18: error: syntax error, unexpected UNSIGNED, expecting '{'
\new Staff \with 
                 5 { c4 }
f.ly:2:26: error: Unfinished main input
\new Staff \with 5 { c4 }
                         
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"m = \with { \alias #'x }"##,
        r##"f.ly:2:13: error: only \consists and \remove take non-string argument.
m = \with { 
            \alias #'x }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"m = \with { \description #'x }"##,
        r##"f.ly:2:13: error: only \consists and \remove take non-string argument.
m = \with { 
            \description #'x }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\new #'Staff = #'x { c4 }"##,
        r##"f.ly:2:16: error: simple string expected
\new #'Staff = 
               #'x { c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"x = \new #5 { c4 }"##,
        r##"f.ly:2:10: error: symbol expected
x = \new 
         #5 { c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\new Staff = { c4 }"##,
        r##"f.ly:2:14: error: syntax error, unexpected '{', expecting SCM_IDENTIFIER or SCM_TOKEN or STRING or SYMBOL
\new Staff = 
             { c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\new { c4 }"##,
        r##"f.ly:2:6: error: syntax error, unexpected '{', expecting SCM_IDENTIFIER or SCM_TOKEN or STRING or SYMBOL
\new 
     { c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\new Staff \with { 5 } { c4 }"##,
        r##"f.ly:2:20: error: syntax error, unexpected UNSIGNED
\new Staff \with { 
                   5 } { c4 }
f.ly:2:30: error: Unfinished main input
\new Staff \with { 5 } { c4 }
                             
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"x = { \change Staff = #5 c4 }"##,
        r##"f.ly:2:23: error: simple string expected
x = { \change Staff = 
                      #5 c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\new Staff \with { \consists } { c4 }"##,
        r##"f.ly:2:30: error: syntax error, unexpected '}'
\new Staff \with { \consists 
                             } { c4 }
f.ly:2:38: error: Unfinished main input
\new Staff \with { \consists } { c4 }
                                     
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\new Staff \with { { c4 } } { c4 }"##,
        r##"f.ly:2:20: warning: Music unsuitable for context-mod
\new Staff \with { 
                   { c4 } } { c4 }
"##,
    ),
    (
        r##"x = \new Staff \with #(make-music 'NoteEvent 'duration (ly:make-duration 2)) { c4 }"##,
        r##"f.ly:2:22: warning: Music unsuitable for context-mod
x = \new Staff \with 
                     #(make-music 'NoteEvent 'duration (ly:make-duration 2)) { c4 }
"##,
    ),
    (
        r##"\new Staff \with #'() { c4 }"##,
        r##"f.ly:2:18: error: not a context mod
\new Staff \with 
                 #'() { c4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\layout { \context { 5 } }"##,
        r##"f.ly:2:22: error: syntax error, unexpected UNSIGNED
\layout { \context { 
                     5 } }
f.ly:2:27: error: Unfinished main input
\layout { \context { 5 } }
                          
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\layout { \notemode c } { c4 }"##,
        r##"f.ly:2:21: error: syntax error, unexpected NOTENAME_PITCH, expecting \sequential or \simultaneous or << or '{'
\layout { \notemode 
                    c } { c4 }
f.ly:2:31: error: Unfinished main input
\layout { \notemode c } { c4 }
                              
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\layout { \context { \with \with { } } }"##,
        r##"f.ly:2:28: error: syntax error, unexpected \with, expecting '{'
\layout { \context { \with 
                           \with { } } }
f.ly:2:40: error: syntax error, unexpected '}'
\layout { \context { \with \with { } } 
                                       }
f.ly:2:41: error: Unfinished main input
\layout { \context { \with \with { } } }
                                        
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\layout { \context { \consists "A" #(make-music 'NoteEvent 'duration (ly:make-duration 2)) } }"##,
        r##"f.ly:2:36: warning: Music unsuitable for context-mod
\layout { \context { \consists "A" 
                                   #(make-music 'NoteEvent 'duration (ly:make-duration 2)) } }
"##,
    ),
    (
        r##"x = \context { }
\x"##,
        r##"f.ly:3:1: error: bad expression type

\x
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"m = \with { \consists "A" }
n = \m
#(display (ly:get-context-mods n) (current-error-port))
#(display (list (ly:context-mod? m) (eq? m n) (ly:context-mod? '())) (current-error-port))"##,
        r##"((consists A))(#t #f #f)"##,
    ),
    (
        r##"m = \with { \consists #'x \remove #'y \accepts #"z" }
#(write (ly:get-context-mods m) (current-error-port))"##,
        r##"((consists x) (remove y) (accepts "z"))"##,
    ),
    (
        r##"#(display (ly:make-context-mod '((a b) (c))) (current-error-port))
#(define c (ly:make-context-mod))
#(ly:add-context-mod c '(consists "X"))
#(display c (current-error-port))
#(write (ly:get-context-mods c) (current-error-port))"##,
        r##"#<Context_mod ((a b) (c))>#<Context_mod ((consists X))>((consists "X"))"##,
    ),
    (
        r##"#(ly:make-context-mod 5)"##,
        r##"f.ly:2:2: error: Guile signaled an error for the expression beginning here
#
 (ly:make-context-mod 5)
In procedure ly:make-context-mod: Wrong type argument in position 1 (expecting list): 5
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"#(ly:add-context-mod 5 '())"##,
        r##"f.ly:2:2: error: Guile signaled an error for the expression beginning here
#
 (ly:add-context-mod 5 '())
In procedure ly:add-context-mod: Wrong type argument in position 1 (expecting Context_mod): 5
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"#(ly:get-context-mods 'x)"##,
        r##"f.ly:2:2: error: Guile signaled an error for the expression beginning here
#
 (ly:get-context-mods 'x)
In procedure ly:get-context-mods: Wrong type argument in position 1 (expecting Context_mod): x
fatal error: failed files: "f.ly"
"##,
    ),
];

#[test]
fn contexts_report_what_lilypond_reports() {
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
