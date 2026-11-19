//! P11, part 1 (02 §2.4, §6; MS2 step 33): `#{ … #}` templates, run by
//! `parser-ly-from-scheme.scm`'s `read-lily-expression-internal` through
//! `ly:parser-clone` and `ly:parse-string-expression`, and
//! `ly:parser-parse-string`; in a file's module that uses a `(lily)`
//! loaded from the installed LilyPond 2.26.0 (`tests/support`).
//!
//! - [`INPUTS`]: the music templates make, compiled by that lilypond with
//!   `tools/compare/music.ly` and compared node by node: in music
//!   functions, with closures by byte (`$m`, `#m`, `$@`, a macro's
//!   quasi-quotation), nested, with the default duration and the pitch
//!   language of the parse that runs them, placed at their call; and the
//!   init's functions whose bodies are templates (`property-init.ly`'s,
//!   `chord-repetition-init.ly`'s `\tabChordRepeats`).
//! - [`VALUES`]: what `embedded_lilypond` gives besides music, written by
//!   the same file in both: nothing, a duration, a pitch, a string, a
//!   symbol, post-events, markup, numbers, a fraction, lists and spliced
//!   values; and `ly:parser-parse-string`'s definitions.
//! - [`CASES`]: what LilyPond prints for errors in templates, placed in
//!   the outer file at their lines and `error in #{ ... #}` at the call;
//!   a Guile error in a closure; a syntax error after a whole item, which
//!   keeps it; and `ly:parser-parse-string`'s and
//!   `ly:parse-string-expression`'s, which fail the clone, not the file.
//!
//! The token sequences of T23 and T28 and of these inputs are compared
//! with LilyPond's by `tools/compare/test/tokens.test.mjs`, and the trees
//! of T23 and T28 by `music.test.mjs`. Every test skips itself, and so
//! verifies nothing, without that lilypond.

use lily_engine_parser::{NoteNames, parse};
use std::path::{Path, PathBuf};
use std::process::Command;
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

/// Inputs that LilyPond parses without a message.
pub const INPUTS: &[(&str, &str)] = &[
    (
        "functions",
        "fn = #(define-music-function (m) (ly:music?) #{ \\tuplet 3/2 { $m $m } #})\n\
         mk = #(define-music-function (p) (ly:pitch?) #{ $p 4. -. #})\n\
         nested = #(define-music-function (m) (ly:music?)\n\
         \x20 #{ \\fn { #m #(make-music 'RestEvent 'duration (ly:make-duration 3)) } #})\n\
         { \\fn c'8 \\mk d' \\nested e'16 }",
    ),
    (
        "closures",
        "#(define-macro (pattern args result)\n\
         \x20 `(define-music-function ,args ,(make-list (length args) 'ly:music?)\n\
         \x20    #{ $@(list ,@result) #}))\n\
         swap = #(pattern (A B) (B A))\n\
         { \\swap { a' } { b' } #(let ((x #{ c4 #})) #{ \\relative { #x d e } #}) }",
    ),
    (
        "state",
        "{ c8 $#{ d e #} f }\n\\language \"deutsch\"\n{ $#{ b h #} }",
    ),
    (
        "init-functions",
        "{ \\arpeggioArrowUp \\arpeggioArrowDown \\arpeggioNormal \\arpeggioBracket\n\
         \x20 \\arpeggioParenthesis \\arpeggioParenthesisDashed\n\
         \x20 \\phrasingSlurDashPattern #0.5 #0.75 \\slurDashPattern #0.5 #0.75\n\
         \x20 \\tieDashPattern #0.4 #0.5 \\xNote c'4 \\xNote <c' e'>4\n\
         \x20 \\tabChordRepeats { <c e>4 q } \\tabChordRepeats #'(articulation-event) { <c e>4-. q } }\n\
         \\incipit { c'4 }",
    ),
    (
        "events",
        "tw = #(define-event-function (x ev) (number? ly:event?)\n\
         \x20 #{ \\tweak X-offset #x \\offset Y-offset #x #ev #})\n\
         { c4 \\tw 2 ^\\ff d-\\tw 3 -> }",
    ),
];

#[test]
fn p11_makes_lilyponds_music() {
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

/// Values of templates, written one a line by `show`: music by its name.
pub const VALUES: &str = r#"#(define (show x) (write (if (ly:music? x) (ly:music-property x 'name) x)) (newline))
#(show #{ #})
#(show #{ 4. #})
#(show #{ 4-. #})
#(show #{ c' #})
#(show #{ c'4 #})
#(show #{ c4 d #})
#(show #{ "str" #})
#(show #{ #'sym #})
#(show #{ -. #})
#(show #{ -. -> #})
#(show (markup? #{ \markup \bold x #}))
#(show #{ -3 #})
#(show #{ 1.5 #})
#(show #{ -1.5 #})
#(show #{ 3/4 #})
#(show #{ 4 c #})
#(show #{ #'sym c #})
#(show #{ $(+ 1 2) #})
#(show (let ((x 5)) #{ #x #}))
#(show (let ((l '(1 2))) #{ #@l #}))
#(ly:parser-parse-string (ly:parser-clone) "pp = 7")
#(show pp)
#(show (ly:parser-has-error? (ly:parser-clone)))
"#;

/// What lilypond writes to stdout and stderr for `text` as `name`.
fn lilypond_output(name: &str, text: &str) -> (String, String) {
    let dir = std::env::temp_dir().join(format!("lily-p11-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), text).unwrap();
    let run = Command::new("lilypond")
        .args(["--loglevel=WARNING", "-dno-print-pages"])
        .arg(name)
        .current_dir(&dir)
        .env("LANGUAGE", "en")
        .output()
        .expect("lilypond runs");
    std::fs::remove_dir_all(&dir).ok();
    (
        String::from_utf8_lossy(&run.stdout).into_owned(),
        String::from_utf8_lossy(&run.stderr).into_owned(),
    )
}

#[test]
fn p11_values_are_lilyponds() {
    let Some(lily) = lily() else { return };
    let mut vm = support::library(lily).expect("the library loads");
    let text = format!("{V}{VALUES}");
    let (stdout, stderr) = lilypond_output("v.ly", &text);
    let parse = parse("v.ly", text.into_bytes(), names(), &mut vm);
    assert_eq!(vm.take_output(), stdout);
    assert_eq!(String::from_utf8_lossy(&parse.stderr()), stderr);
    lily_engine_parser::end_session(&mut vm);
}

/// Each input after [`V`], and LilyPond's stderr for it.
pub const CASES: &[(&str, &str)] = &[
    (
        "m = #(define-music-function () ()\n  #{ c4\n     d4 \\foo #})\n{ \\m }",
        r##"f.ly:4:9: error: unknown command: `\foo'
     d4 
        \foo 
f.ly:4:9: error: string outside of text script or \lyricmode
     d4 
        \foo 
f.ly:5:3: error: error in #{ ... #}
{ 
  \m }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        "m = #(define-music-function (x) (number?) #{ c4 #(car x) #})\n{ \\m 1 }",
        r##"f.ly:2:6: error: Guile signaled an error for the expression beginning here
 c4 #
     (car x) 
In procedure car: Wrong type argument in position 1 (expecting pair): 1
f.ly:3:3: error: error in #{ ... #}
{ 
  \m 1 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        "m = #(define-music-function () () #{ c4 } #})\n{ \\m }",
        r##"f.ly:2:5: error: syntax error, unexpected '}', expecting end of input
 c4 
    } 
f.ly:3:3: error: error in #{ ... #}
{ 
  \m }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        "#(define z #{ } #})\n#(define w #{ c4 d \\bar #})",
        r##"f.ly:2:2: error: syntax error, unexpected '}'
 
 } 
f.ly:2:2: error: error in #{ ... #}
#
 (define z #{ } #})
f.ly:3:11: error: syntax error, unexpected end of input, expecting \header
 c4 d \bar
           
f.ly:3:2: error: error in #{ ... #}
#
 (define w #{ c4 d \bar #})
fatal error: failed files: "f.ly"
"##,
    ),
    (
        "#(ly:parser-parse-string (ly:parser-clone) \"{ c4 \\\\foo }\")",
        r##"<string>:1:6: error: unknown command: `\foo'
{ c4 
     \foo }
<string>:1:6: error: string outside of text script or \lyricmode
{ c4 
     \foo }
"##,
    ),
    (
        "#(define y (ly:parse-string-expression (ly:parser-clone) \"c4 \\\\bar\"))",
        r##"<string>:1:4: error: syntax error, unexpected end of input, expecting \header
c4 
   \bar
"##,
    ),
];

#[test]
fn p11_errors_are_lilyponds() {
    let Some(lily) = lily() else { return };
    let mut vm = support::library(lily).expect("the library loads");
    for (input, expected) in CASES {
        let text = format!("{V}{input}\n");
        let parse = parse("f.ly", text.into_bytes(), names(), &mut vm);
        support::music_trees(&mut vm, &parse).expect("the trees are written");
        let stderr = String::from_utf8_lossy(&parse.stderr()).into_owned();
        assert_eq!(stderr, *expected, "{input}");
        lily_engine_parser::end_session(&mut vm);
    }
}

/// Templates that run templates without end: LilyPond 2.26.0 crashes when
/// its stack ends; the engine stops at 64 deep with Guile's `Stack
/// overflow`, and each call is `error in #{ ... #}`. Run on a thread with
/// the main thread's stack, as a debug build's frames are large.
#[test]
fn p11_templates_nest_to_their_bound() {
    let Some(lily) = lily() else { return };
    let lily = lily.to_path_buf();
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let mut vm = support::library(&lily).expect("the library loads");
            let rec = "rec = #(define-music-function (n) (integer?)\n\
                       \x20 (if (> n 0) #{ c4 \\rec #(- n 1) #} #{ d4 #}))\n";
            let deep = format!("{V}{rec}{{ \\rec 63 }}\n");
            let deep = parse("f.ly", deep.into_bytes(), names(), &mut vm);
            assert_eq!(String::from_utf8_lossy(&deep.stderr()), "");
            lily_engine_parser::end_session(&mut vm);
            let endless = format!("{V}{rec}{{ \\rec 64 }}\n");
            let endless = parse("f.ly", endless.into_bytes(), names(), &mut vm);
            let stderr = String::from_utf8_lossy(&endless.stderr()).into_owned();
            assert!(stderr.starts_with("Stack overflow\n"), "{stderr}");
            assert_eq!(stderr.matches("error: error in #{ ... #}").count(), 64);
            assert!(endless.failed());
        })
        .unwrap()
        .join()
        .unwrap();
}
