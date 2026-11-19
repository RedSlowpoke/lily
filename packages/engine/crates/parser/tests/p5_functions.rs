//! P5, part 1 (02 §3.2, §5.3, §6): music, event and Scheme functions
//! applied by their signatures, in a file's module that uses a `(lily)`
//! loaded from the installed LilyPond 2.26.0 (`tests/support`). Each
//! expected stderr was printed by LilyPond 2.26.0 (`lilypond
//! --loglevel=WARNING`) for the same input, less the warnings of processing,
//! which a parser alone does not reach (D64). The predicates of [`PRELUDE`]
//! write what they are given to the error port, so each case's stderr shows
//! every predicate call in LilyPond's order; what they write comes where
//! Guile flushes the port, at the end. The error corpus's cases 17 and 18 are
//! scored with C0 by `tools/compare/test/c0-cases.test.mjs`. Every test skips
//! itself, and so verifies nothing, without that lilypond.

use lily_engine_parser::{NoteNames, parse};
use lily_engine_scheme::Vm;
use std::sync::OnceLock;

mod support;

const V: &str = "\\version \"2.26.0\"\n";

/// Predicates that trace their calls, the functions that use them, and the
/// others the cases call.
const PRELUDE: &str = r#"#(define (show x) (if (ly:music? x) (ly:music-property x 'name) x))
#(define (tr? x) (format (current-error-port) "tr? ~s\n" (show x)) #f)
#(define (tp? x) (format (current-error-port) "tp? ~s\n" (show x)) (ly:pitch? x))
#(define (tn? x) (format (current-error-port) "tn? ~s\n" (show x)) (number? x))
#(define (ts? x) (format (current-error-port) "ts? ~s\n" (show x)) (string? x))
#(define (tm? x) (format (current-error-port) "tm? ~s\n" (show x)) (ly:music? x))
#(define (tl? x) (format (current-error-port) "tl? ~s\n" (show x)) (symbol-list? x))
#(define (say x) (display x (current-error-port)))
#(define (td? x) (format (current-error-port) "td? ~s\n" (show x)) (ly:duration? x))
tst = #(define-music-function (a b) (tp? tr?) (make-music 'Music))
opt = #(define-music-function (a b c) ((tn? 1) (ts? "s") tm?) (format (current-error-port) "opt ~s ~s ~s\n" a b (show c)) c)
topt = #(define-music-function (a b) (tm? (tn? 7)) (format (current-error-port) "topt ~s ~s\n" (show a) b) a)
mopt = #(define-music-function (a b) ((tn? 7) tm?) b)
sl = #(define-scheme-function (l) (tl?) (format (current-error-port) "sl ~s\n" l) l)
dur = #(define-scheme-function (d) (td?) (format (current-error-port) "dur ~s\n" d) d)
ret = #(define-music-function (x) (scheme?) x)
sret = #(define-scheme-function (x) (scheme?) x)
ev = #(define-event-function () () (make-music 'TieEvent))"#;

/// The installed `scm/lily/`, or None (and a note on stderr) without lilypond.
fn lily() -> Option<&'static std::path::Path> {
    static LILY: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    let lily = LILY.get_or_init(support::lily_scm);
    if lily.is_none() {
        eprintln!("skipped: no LilyPond 2.26.0 installed");
    }
    lily.as_deref()
}

/// The installed pitch names.
fn names() -> Option<&'static NoteNames> {
    static NAMES: OnceLock<Option<NoteNames>> = OnceLock::new();
    NAMES
        .get_or_init(|| {
            let text = std::fs::read(lily()?.join("define-note-names.scm")).ok()?;
            Some(NoteNames::read(&text).expect("define-note-names.scm reads"))
        })
        .as_ref()
}

fn library() -> Option<Vm> {
    Some(support::library(lily()?).expect("the library loads"))
}

fn stderr(vm: &mut Vm, text: &str) -> String {
    let parse = parse(
        "f.ly",
        format!("{V}{PRELUDE}\n{text}\n").into_bytes(),
        names().unwrap(),
        vm,
    );
    String::from_utf8_lossy(&parse.stderr()).into_owned()
}

/// Each input after [`PRELUDE`], and LilyPond's stderr for it:
/// - the readings of an argument by its first token, and their predicate
///   calls: words, strings and symbol lists, pitches and notes, `UNSIGNED`s
///   as numbers, symbol lists, durations and notes, numbers and `-`,
///   post-events, Scheme, music, fractions;
/// - optional parameters: taken, backed up with the rest of their run
///   skipped, `\default`, at the end, and missing arguments;
/// - results checked, `music function cannot return`, and the fallback a
///   failed call gives; nested calls; event functions with a direction;
///   Scheme functions at toplevel, in music and as values;
/// - the minimal music value's Scheme face (`make-music`, `ly:event?`,
///   properties, copies) as `music->make-music` shows it;
/// - §5.4's timing: whether a `$` after `x = \fun …` sees `x` defined.
///
/// Not here, as the engine differs: an articulation's properties, which
/// `dashDot` gives (P6, N2); a music object printed, which shows a type's
/// callbacks in LilyPond; `\transpose` and `\tuplet` applied, whose
/// bodies are N2's.
const CASES: &[(&str, &str)] = &[
    (
        r#"{ \tst c "d" }"#,
        r#"f.ly:20:12: error: wrong type for argument 2.  Expecting tr, found "d"
{ \tst c "d
           " }
tp? NoteEvent
tp? #<Pitch c >
tr? "d"
tr? (d)
tr? d
tr? "d"
tr? "d"
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c d }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found (ly:make-pitch -1 1)
{ \tst c 
         d }
tp? NoteEvent
tp? #<Pitch c >
tr? NoteEvent
tr? #<Pitch d >
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c foo }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found "foo"
{ \tst c 
         foo }
tp? NoteEvent
tp? #<Pitch c >
tr? "foo"
tr? (foo)
tr? foo
tr? "foo"
tr? "foo"
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c 4 }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found 4
{ \tst c 
         4 }
tp? NoteEvent
tp? #<Pitch c >
tr? 4
tr? (4)
tr? #<Duration 4 >
tr? NoteEvent
tr? 4
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c 3 }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found 3
{ \tst c 
         3 }
tp? NoteEvent
tp? #<Pitch c >
tr? 3
tr? (3)
tr? 3
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c -3 }"#,
        r#"f.ly:20:11: error: wrong type for argument 2.  Expecting tr, found 3
{ \tst c -
          3 }
tp? NoteEvent
tp? #<Pitch c >
tr? -3
tr? FingeringEvent
tr? 3
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c 2.5 }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found 2.5
{ \tst c 
         2.5 }
tp? NoteEvent
tp? #<Pitch c >
tr? 2.5
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c { c } }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found (make-music (quote SequentialMusic) (quote elements) (list (make-music (quote NoteEvent) (quote pitch) (ly:make-pitch -1 0) (quote duration) (ly:make-duration 2))))
{ \tst c 
         { c } }
tp? NoteEvent
tp? #<Pitch c >
tr? SequentialMusic
tr? SequentialMusic
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c #5 }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found 5
{ \tst c 
         #5 }
tp? NoteEvent
tp? #<Pitch c >
tr? 5
tr? 5
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c r4 }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found (make-music (quote RestEvent) (quote duration) (ly:make-duration 2))
{ \tst c 
         r4 }
tp? NoteEvent
tp? #<Pitch c >
tr? RestEvent
tr? RestEvent
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c' d }"#,
        r#"f.ly:20:11: error: wrong type for argument 2.  Expecting tr, found (ly:make-pitch -1 1)
{ \tst c' 
          d }
tp? NoteEvent
tp? #<Pitch c' >
tr? NoteEvent
tr? #<Pitch d >
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c ##f }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found #f
{ \tst c 
         ##f }
tp? NoteEvent
tp? #<Pitch c >
tr? #f
tr? #f
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c 3/2 }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found (cons 3 2)
{ \tst c 
         3/2 }
tp? NoteEvent
tp? #<Pitch c >
tr? (3 . 2)
tr? (3 . 2)
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c -2.5 }"#,
        r#"f.ly:20:11: error: wrong type for argument 2.  Expecting tr, found -2.5
{ \tst c -
          2.5 }
tp? NoteEvent
tp? #<Pitch c >
tr? -2.5
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c "a.b" }"#,
        r#"f.ly:20:14: error: wrong type for argument 2.  Expecting tr, found "a.b"
{ \tst c "a.b
             " }
tp? NoteEvent
tp? #<Pitch c >
tr? "a.b"
tr? (a.b)
tr? a.b
tr? "a.b"
tr? "a.b"
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c ~ }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found (make-music (quote TieEvent))
{ \tst c 
         ~ }
tp? NoteEvent
tp? #<Pitch c >
tr? TieEvent
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c -1 }"#,
        r#"f.ly:20:11: error: wrong type for argument 2.  Expecting tr, found 1
{ \tst c -
          1 }
tp? NoteEvent
tp? #<Pitch c >
tr? -1
tr? FingeringEvent
tr? 1
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c ^1 }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found (make-music (quote FingeringEvent) (quote direction) 1 (quote digit) 1)
{ \tst c 
         ^1 }
tp? NoteEvent
tp? #<Pitch c >
tr? FingeringEvent
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c <c e>4 }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found (make-music (quote EventChord) (quote elements) (list (make-music (quote NoteEvent) (quote duration) (ly:make-duration 2) (quote pitch) (ly:make-pitch -1 0)) (make-music (quote NoteEvent) (quote duration) (ly:make-duration 2) (quote pitch) (ly:make-pitch -1 2))))
{ \tst c 
         <c e>4 }
tp? NoteEvent
tp? #<Pitch c >
tr? EventChord
tr? EventChord
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c4 d }"#,
        r#"f.ly:20:9: error: wrong type for argument 2.  Expecting tr, found 4
{ \tst c
        4 d }
tp? NoteEvent
tp? #<Pitch c >
tr? 4
tr? (4)
tr? #<Duration 4 >
tr? NoteEvent
tr? 4
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c Staff.Foo }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found "Staff"
{ \tst c 
         Staff.Foo }
f.ly:20:15: error: syntax error, unexpected '.'
{ \tst c Staff
              .Foo }
f.ly:20:16: error: not a note name: Foo
{ \tst c Staff.
               Foo }
f.ly:20:1: error: errors found, ignoring music expression

{ \tst c Staff.Foo }
tp? NoteEvent
tp? #<Pitch c >
tr? "Staff"
tr? (Staff)
tr? Staff
tr? "Staff"
tr? "Staff"
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \opt 3 "a" c }"#,
        r#"tn? 3
tn? 3
ts? "a"
tm? NoteEvent
tm? #<Pitch c >
tm? NoteEvent
opt 3 "a" NoteEvent
"#,
    ),
    (
        r#"{ \opt "a" c }"#,
        r#"f.ly:20:10: error: wrong type for argument 3.  Expecting tm, found "a"
{ \opt "a
         " c }
tn? "a"
tn? (a)
tn? a
tm? "a"
tm? (a)
tm? a
tm? "a"
tm? "a"
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \opt c }"#,
        r#"tn? NoteEvent
tn? #<Pitch c >
tm? NoteEvent
tm? #<Pitch c >
tm? NoteEvent
opt 1 "s" NoteEvent
"#,
    ),
    (
        r#"{ \opt \default c }"#,
        r#"tm? NoteEvent
tm? #<Pitch c >
tm? NoteEvent
opt 1 "s" NoteEvent
"#,
    ),
    (
        r#"{ \opt 3 \default c }"#,
        r#"tn? 3
tn? 3
tm? NoteEvent
tm? #<Pitch c >
tm? NoteEvent
opt 3 "s" NoteEvent
"#,
    ),
    (
        r#"{ \opt #2 c }"#,
        r#"tn? 2
ts? NoteEvent
ts? #<Pitch c >
tm? NoteEvent
tm? #<Pitch c >
tm? NoteEvent
opt 2 "s" NoteEvent
"#,
    ),
    (
        r#"{ \opt -. c }"#,
        r#"f.ly:20:3: warning: Unattached ArticulationEvent
{ 
  \opt -. c }
tn? ArticulationEvent
tm? ArticulationEvent
opt 1 "s" ArticulationEvent
"#,
    ),
    (
        r#"{ \opt 3 4 c }"#,
        r#"tn? 3
tn? 3
ts? 4
ts? (4)
ts? #<Duration 4 >
ts? NoteEvent
tm? 4
tm? (4)
tm? #<Duration 4 >
tm? NoteEvent
tm? NoteEvent
opt 3 "s" NoteEvent
"#,
    ),
    (
        r#"{ \opt -3 c }"#,
        r#"tn? -3
tn? -3
ts? NoteEvent
ts? #<Pitch c >
tm? NoteEvent
tm? #<Pitch c >
tm? NoteEvent
opt -3 "s" NoteEvent
"#,
    ),
    (
        r#"{ \opt 2.5 c }"#,
        r#"tn? 2.5
tn? 2.5
ts? NoteEvent
ts? #<Pitch c >
tm? NoteEvent
tm? #<Pitch c >
tm? NoteEvent
opt 2.5 "s" NoteEvent
"#,
    ),
    (
        r#"{ \topt c \default }"#,
        r#"tm? NoteEvent
tm? #<Pitch c >
tm? NoteEvent
topt NoteEvent 7
"#,
    ),
    (
        r#"{ \topt c "x" }"#,
        r#"f.ly:20:13: error: wrong type for argument 2.  Expecting tn, found "x"
{ \topt c "x
            " }
tm? NoteEvent
tm? #<Pitch c >
tm? NoteEvent
tn? "x"
tn? (x)
tn? x
tn? "x"
tn? "x"
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \topt c 4 }"#,
        r#"f.ly:20:13: error: syntax error, unexpected '}', expecting \header
{ \topt c 4 
            }
f.ly:20:1: error: errors found, ignoring music expression

{ \topt c 4 }
tm? NoteEvent
tm? NoteEvent
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \topt c }"#,
        r#"f.ly:20:11: error: syntax error, unexpected '}', expecting \header
{ \topt c 
          }
f.ly:20:1: error: errors found, ignoring music expression

{ \topt c }
tm? NoteEvent
tm? #<Pitch c >
tm? NoteEvent
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \topt c -3 }"#,
        r#"f.ly:20:14: error: syntax error, unexpected '}', expecting \header
{ \topt c -3 
             }
f.ly:20:1: error: errors found, ignoring music expression

{ \topt c -3 }
tm? NoteEvent
tm? NoteEvent
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \topt c -. }"#,
        r#"f.ly:20:14: error: syntax error, unexpected '}', expecting \header
{ \topt c -. 
             }
f.ly:20:1: error: errors found, ignoring music expression

{ \topt c -. }
tm? NoteEvent
tm? NoteEvent
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \mopt }"#,
        r#"f.ly:20:9: error: syntax error, unexpected '}', expecting (backed-up?)
{ \mopt 
        }
f.ly:20:1: error: errors found, ignoring music expression

{ \mopt }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \opt \default \default c }"#,
        r#"f.ly:20:17: error: syntax error, unexpected \default, expecting \header
{ \opt \default 
                \default c }
f.ly:20:1: error: errors found, ignoring music expression

{ \opt \default \default c }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\sl Staff.Foo"#,
        r#"f.ly:20:1: error: bad expression type

\sl Staff.Foo
tl? "Staff"
tl? (Staff)
tl? (Staff Foo)
sl (Staff Foo)
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\sl Staff"#,
        r#"f.ly:20:1: error: bad expression type

\sl Staff
tl? "Staff"
tl? (Staff)
tl? (Staff)
sl (Staff)
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\sl "a""#,
        r#"f.ly:20:1: error: bad expression type

\sl "a"
tl? "a"
tl? (a)
tl? (a)
sl (a)
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\sl 3"#,
        r#"f.ly:20:5: error: wrong type for argument 1.  Expecting tl, found 3
\sl 
    3
f.ly:20:1: error: bad expression type

\sl 3
tl? 3
tl? (3)
tl? 3
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\sl #'(a b)"#,
        r#"f.ly:20:1: error: bad expression type

\sl #'(a b)
tl? (a b)
sl (a b)
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\sl Foo,Bar"#,
        r#"f.ly:20:1: error: bad expression type

\sl Foo,Bar
tl? "Foo"
tl? (Foo)
tl? (Foo Bar)
sl (Foo Bar)
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\dur 4"#,
        r#"f.ly:20:1: error: bad expression type

\dur 4
td? 4
td? (4)
td? #<Duration 4 >
td? #<Duration 4 >
dur #<Duration 4 >
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\dur 4."#,
        r#"f.ly:20:1: error: bad expression type

\dur 4.
td? 4
td? (4)
td? #<Duration 4 >
td? #<Duration 4. >
dur #<Duration 4. >
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\dur 8*2/3"#,
        r#"f.ly:20:1: error: bad expression type

\dur 8*2/3
td? 8
td? (8)
td? #<Duration 8 >
td? #<Duration 8*2/3 >
dur #<Duration 8*2/3 >
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\dur 3"#,
        r#"f.ly:20:6: error: wrong type for argument 1.  Expecting td, found 3
\dur 
     3
f.ly:20:1: error: bad expression type

\dur 3
td? 3
td? (3)
td? 3
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \ret #5 }"#,
        r#"f.ly:20:3: error: music function cannot return #5
{ 
  \ret #5 }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \ret "x" }"#,
        r#"f.ly:20:3: error: music function cannot return "x"
{ 
  \ret "x" }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \ret #'sym }"#,
        r#"f.ly:20:3: error: music function cannot return #'sym
{ 
  \ret #'sym }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \ret #'(1 2) }"#,
        r#"f.ly:20:3: error: music function cannot return #'(1 2)
{ 
  \ret #'(1 2) }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \ret ##t }"#,
        r#"f.ly:20:3: error: music function cannot return ##t
{ 
  \ret ##t }
fatal error: failed files: "f.ly"
"#,
    ),
    (r#"{ \ret { c } }"#, r#""#),
    (r#"\ret { c }"#, r#""#),
    (
        r#"\sret #5"#,
        r#"f.ly:20:1: error: bad expression type

\sret #5
fatal error: failed files: "f.ly"
"#,
    ),
    (r#"\sret "s""#, r#""#),
    (
        r#"{ \ret }"#,
        r#"f.ly:20:8: error: syntax error, unexpected '}', expecting \header
{ \ret 
       }
f.ly:20:1: error: errors found, ignoring music expression

{ \ret }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c }"#,
        r#"f.ly:20:10: error: syntax error, unexpected '}', expecting \header
{ \tst c 
         }
f.ly:20:1: error: errors found, ignoring music expression

{ \tst c }
tp? NoteEvent
tp? #<Pitch c >
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"x = \sret"#,
        r#"f.ly:20:10: error: syntax error, unexpected end of input, expecting \header
x = \sret
         
fatal error: failed files: "f.ly"
"#,
    ),
    (r#"{ \ret \ret { c } }"#, r#""#),
    (r#"{ c \ev }"#, r#""#),
    (r#"{ c^\ev }"#, r#""#),
    (
        r#"{ \ev }"#,
        r#"f.ly:20:3: warning: Unattached TieEvent
{ 
  \ev }
"#,
    ),
    (
        r#"x = \ret { c }
{ \x }"#,
        r#""#,
    ),
    (
        r#"x = \ret { c }
{ \tst c \x }"#,
        r#"f.ly:21:10: error: wrong type for argument 2.  Expecting tr, found (make-music (quote SequentialMusic) (quote elements) (list (make-music (quote NoteEvent) (quote pitch) (ly:make-pitch -1 0) (quote duration) (ly:make-duration 2))))
{ \tst c 
         \x }
tp? NoteEvent
tp? #<Pitch c >
tr? SequentialMusic
tr? SequentialMusic
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst \sret c d }"#,
        r#"f.ly:20:16: error: wrong type for argument 2.  Expecting tr, found (ly:make-pitch -1 1)
{ \tst \sret c 
               d }
tp? #<Pitch c >
tr? NoteEvent
tr? #<Pitch d >
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \ret \default }"#,
        r#"f.ly:20:8: error: syntax error, unexpected \default, expecting \header
{ \ret 
       \default }
f.ly:20:1: error: errors found, ignoring music expression

{ \ret \default }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tuplet "3/2" { c8 d e } }"#,
        r#"f.ly:20:15: error: wrong type for argument 1.  Expecting fraction, as pair, found "3/2"
{ \tuplet "3/2
              " { c8 d e } }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \transpose c "d" { c4 } }"#,
        r#"f.ly:20:18: error: wrong type for argument 2.  Expecting pitch, found "d"
{ \transpose c "d
                 " { c4 } }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"rel = #(define-music-function (p m) ((ly:pitch? #f) ly:music?) (format (current-error-port) "rel ~s ~s\n" p (show m)) m)
tup = #(define-music-function (r s m) (fraction? (ly:duration? '()) ly:music?) (format (current-error-port) "tup ~s ~s\n" r s) m)
eva = #(define-event-function (n) (number?) (make-music 'FingeringEvent 'digit n))
two = #(define-music-function (x y) (number? ly:music?) y)"#,
        r#""#,
    ),
    (
        r#"{ \rel c' { d } }"#,
        r#"f.ly:20:3: error: unknown command: `\rel'
{ 
  \rel c' { d } }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \rel c' { d } }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \rel { d } }"#,
        r#"f.ly:20:3: error: unknown command: `\rel'
{ 
  \rel { d } }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \rel { d } }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \rel c'4 { d } }"#,
        r#"f.ly:20:3: error: unknown command: `\rel'
{ 
  \rel c'4 { d } }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \rel c'4 { d } }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \rel c d }"#,
        r#"f.ly:20:3: error: unknown command: `\rel'
{ 
  \rel c d }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \rel c d }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tup 3/2 4 { c } }"#,
        r#"f.ly:20:3: error: unknown command: `\tup'
{ 
  \tup 3/2 4 { c } }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \tup 3/2 4 { c } }
f.ly:20:8: error: syntax error, unexpected FRACTION
{ \tup 
       3/2 4 { c } }
f.ly:20:1: error: errors found, ignoring music expression

{ \tup 3/2 4 { c } }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tup 3/2 4. { c } }"#,
        r#"f.ly:20:3: error: unknown command: `\tup'
{ 
  \tup 3/2 4. { c } }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \tup 3/2 4. { c } }
f.ly:20:8: error: syntax error, unexpected FRACTION
{ \tup 
       3/2 4. { c } }
f.ly:20:1: error: errors found, ignoring music expression

{ \tup 3/2 4. { c } }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tup 3/2 8*2/3 { c } }"#,
        r#"f.ly:20:3: error: unknown command: `\tup'
{ 
  \tup 3/2 8*2/3 { c } }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \tup 3/2 8*2/3 { c } }
f.ly:20:8: error: syntax error, unexpected FRACTION
{ \tup 
       3/2 8*2/3 { c } }
f.ly:20:1: error: errors found, ignoring music expression

{ \tup 3/2 8*2/3 { c } }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tup 3/2 { c } }"#,
        r#"f.ly:20:3: error: unknown command: `\tup'
{ 
  \tup 3/2 { c } }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \tup 3/2 { c } }
f.ly:20:8: error: syntax error, unexpected FRACTION
{ \tup 
       3/2 { c } }
f.ly:20:1: error: errors found, ignoring music expression

{ \tup 3/2 { c } }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tup 3/2 c }"#,
        r#"f.ly:20:3: error: unknown command: `\tup'
{ 
  \tup 3/2 c }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \tup 3/2 c }
f.ly:20:8: error: syntax error, unexpected FRACTION
{ \tup 
       3/2 c }
f.ly:20:1: error: errors found, ignoring music expression

{ \tup 3/2 c }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tup 3/2 c4 d }"#,
        r#"f.ly:20:3: error: unknown command: `\tup'
{ 
  \tup 3/2 c4 d }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \tup 3/2 c4 d }
f.ly:20:8: error: syntax error, unexpected FRACTION
{ \tup 
       3/2 c4 d }
f.ly:20:1: error: errors found, ignoring music expression

{ \tup 3/2 c4 d }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\sl Staff.TimeSignature.color"#,
        r#"f.ly:20:1: error: bad expression type

\sl Staff.TimeSignature.color
tl? "Staff"
tl? (Staff)
tl? (Staff TimeSignature color)
sl (Staff TimeSignature color)
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\sl #'Staff"#,
        r#"f.ly:20:5: error: wrong type for argument 1.  Expecting tl, found (quote Staff)
\sl 
    #'Staff
f.ly:20:1: error: bad expression type

\sl #'Staff
tl? Staff
tl? Staff
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\sl Staff,Foo"#,
        r#"f.ly:20:1: error: bad expression type

\sl Staff,Foo
tl? "Staff"
tl? (Staff)
tl? (Staff Foo)
sl (Staff Foo)
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"\sl Staff.#'Foo"#,
        r#"f.ly:20:1: error: bad expression type

\sl Staff.#'Foo
tl? "Staff"
tl? (Staff)
tl? (Staff Foo)
sl (Staff Foo)
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ c\eva 3 }"#,
        r#"f.ly:20:4: error: unknown command: `\eva'
{ c
   \eva 3 }
f.ly:20:4: error: string outside of text script or \lyricmode
{ c
   \eva 3 }
f.ly:20:9: error: not a duration
{ c\eva 
        3 }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ c^\eva 3 }"#,
        r#"f.ly:20:5: error: unknown command: `\eva'
{ c^
    \eva 3 }
f.ly:20:10: error: not a duration
{ c^\eva 
         3 }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ c-\eva 3 }"#,
        r#"f.ly:20:5: error: unknown command: `\eva'
{ c-
    \eva 3 }
f.ly:20:10: error: not a duration
{ c-\eva 
         3 }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \eva 3 }"#,
        r#"f.ly:20:3: error: unknown command: `\eva'
{ 
  \eva 3 }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \eva 3 }
f.ly:20:8: error: not a duration
{ \eva 
       3 }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \sret #5 }"#,
        r#"f.ly:20:3: warning: Ignoring non-music expression
{ 
  \sret #5 }
"#,
    ),
    (r#"{ \sret { c } }"#, r#""#),
    (
        r#"{ \tst c \default }"#,
        r#"f.ly:20:10: error: syntax error, unexpected \default, expecting \header
{ \tst c 
         \default }
f.ly:20:1: error: errors found, ignoring music expression

{ \tst c \default }
tp? NoteEvent
tp? #<Pitch c >
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \opt { c } }"#,
        r#"tn? SequentialMusic
tm? SequentialMusic
opt 1 "s" SequentialMusic
"#,
    ),
    (
        r#"{ \opt 3 { c } }"#,
        r#"tn? 3
tn? 3
ts? SequentialMusic
tm? SequentialMusic
opt 3 "s" SequentialMusic
"#,
    ),
    (
        r#"{ \ret \tst c "d" }"#,
        r#"f.ly:20:17: error: wrong type for argument 2.  Expecting tr, found "d"
{ \ret \tst c "d
                " }
tp? NoteEvent
tp? #<Pitch c >
tr? "d"
tr? (d)
tr? d
tr? "d"
tr? "d"
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \ret c }"#,
        r#"f.ly:20:3: error: music function cannot return c
{ 
  \ret c }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"x = \sret 5
#(display "x")"#,
        r#""#,
    ),
    (r#"\score { \ret { c } }"#, r#""#),
    (
        r#"{ \two 3 c4 d }"#,
        r#"f.ly:20:3: error: unknown command: `\two'
{ 
  \two 3 c4 d }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \two 3 c4 d }
f.ly:20:8: error: not a duration
{ \two 
       3 c4 d }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \two #3 \two 4 c }"#,
        r#"f.ly:20:3: error: unknown command: `\two'
{ 
  \two #3 \two 4 c }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \two #3 \two 4 c }
f.ly:20:8: warning: Ignoring non-music expression
{ \two 
       #3 \two 4 c }
f.ly:20:11: error: unknown command: `\two'
{ \two #3 
          \two 4 c }
f.ly:20:11: error: string outside of text script or \lyricmode
{ \two #3 
          \two 4 c }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \two "3" c }"#,
        r#"f.ly:20:3: error: unknown command: `\two'
{ 
  \two "3" c }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \two "3" c }
f.ly:20:10: error: string outside of text script or \lyricmode
{ \two "3
         " c }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \two 3 \ret #5 }"#,
        r#"f.ly:20:3: error: unknown command: `\two'
{ 
  \two 3 \ret #5 }
f.ly:20:3: error: string outside of text script or \lyricmode
{ 
  \two 3 \ret #5 }
f.ly:20:8: error: not a duration
{ \two 
       3 \ret #5 }
f.ly:20:10: error: music function cannot return #5
{ \two 3 
         \ret #5 }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"x = \two 3 c
{ \x }"#,
        r#"f.ly:20:5: error: unknown command: `\two'
x = 
    \two 3 c
f.ly:20:10: error: syntax error, unexpected UNSIGNED
x = \two 
         3 c
f.ly:21:3: warning: Ignoring non-music expression
{ 
  \x }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"x = \ev
{ c \x }"#,
        r#""#,
    ),
    (
        r#"{ \tst \ret c d }"#,
        r#"f.ly:20:8: error: music function cannot return c
{ \tst 
       \ret c d }
f.ly:20:8: error: wrong type for argument 1.  Expecting tp, found (make-music (quote Music))
{ \tst 
       \ret c d }
f.ly:20:15: error: wrong type for argument 2.  Expecting tr, found (ly:make-pitch -1 1)
{ \tst \ret c 
              d }
tp? Music
tp? Music
tr? NoteEvent
tr? #<Pitch d >
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \tst c \sret #5 }"#,
        r#"f.ly:20:10: error: wrong type for argument 2.  Expecting tr, found 5
{ \tst c 
         \sret #5 }
tp? NoteEvent
tp? #<Pitch c >
tr? 5
tr? 5
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \ret ees' }"#,
        r#"f.ly:20:3: error: music function cannot return ees'
{ 
  \ret ees' }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \ret fis,, }"#,
        r#"f.ly:20:3: error: music function cannot return fis,,
{ 
  \ret fis,, }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"{ \ret c'' }"#,
        r#"f.ly:20:3: error: music function cannot return c''
{ 
  \ret c'' }
fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"#(format (current-error-port) "~s ~s ~s ~s\n" (ly:event? (make-music 'NoteEvent)) (ly:event? (make-music 'FingeringEvent)) (ly:music? (make-music 'Music)) (ly:music? 5))"#,
        r#"#f #t #t #f
"#,
    ),
    (
        r#"#(format (current-error-port) "~s\n" (music->make-music (make-music 'SequentialMusic 'elements (list (make-music 'NoteEvent 'pitch (ly:make-pitch 0 1 1/2) 'duration (ly:make-duration 3 1))))))"#,
        r#"(make-music (quote SequentialMusic) (quote elements) (list (make-music (quote NoteEvent) (quote duration) (ly:make-duration 3 1) (quote pitch) (ly:make-pitch 0 1 1/2))))
"#,
    ),
    (
        r#"#(format (current-error-port) "~s ~s ~s\n" (ly:music-property (make-music 'NoteEvent) 'name) (ly:music-property (make-music 'NoteEvent) 'types) (ly:music-property (make-music 'NoteEvent) 'pitch 'none))"#,
        r#"NoteEvent (event note-event rhythmic-event melodic-event) none
"#,
    ),
    (
        r#"#(let ((m (make-music 'NoteEvent 'pitch (ly:make-pitch 0 0)))) (ly:music-set-property! m 'pitch (ly:make-pitch 1 0)) (ly:music-set-property! m 'duration (ly:make-duration 2)) (format (current-error-port) "~s\n" (music->make-music m)))"#,
        r#"(make-music (quote NoteEvent) (quote duration) (ly:make-duration 2) (quote pitch) (ly:make-pitch 1 0))
"#,
    ),
    (
        r#"#(let* ((m (make-music 'SequentialMusic 'elements (list (make-music 'NoteEvent)))) (c (ly:music-deep-copy m))) (format (current-error-port) "~s ~s\n" (eq? (car (ly:music-property m 'elements)) (car (ly:music-property c 'elements))) (music->make-music c)))"#,
        r#"#f (make-music (quote SequentialMusic) (quote elements) (list (make-music (quote NoteEvent))))
"#,
    ),
    (
        r#"#(format (current-error-port) "~s ~s ~s\n" (ly:music-function? tst) (ly:music-function? car) (procedure? (ly:music-function-extract tst)))"#,
        r#"#t #f #t
"#,
    ),
    (
        r#"x = \sret #5
$(say (if (defined? 'x) "+" "-"))"#,
        r#"+"#,
    ),
    (
        r#"x = \sret "s"
$(say (if (defined? 'x) "+" "-"))"#,
        r#"+"#,
    ),
    (
        r#"x = \sret 5
$(say (if (defined? 'x) "+" "-"))"#,
        r#"-"#,
    ),
    (
        r#"x = \sret ##t
$(say (if (defined? 'x) "+" "-"))"#,
        r#"+"#,
    ),
    (
        r#"x = \ret { c }
$(say (if (defined? 'x) "+" "-"))"#,
        r#"-"#,
    ),
    (
        r#"x = \ret c'4
$(say (if (defined? 'x) "+" "-"))"#,
        r#"-"#,
    ),
    (
        r#"x = \ev
$(say (if (defined? 'x) "+" "-"))"#,
        r#"-"#,
    ),
    (
        r#"x = \eva 3
$(say (if (defined? 'x) "+" "-"))"#,
        r#"f.ly:20:5: error: unknown command: `\eva'
x = 
    \eva 3
f.ly:20:10: error: syntax error, unexpected UNSIGNED
x = \eva 
         3
+fatal error: failed files: "f.ly"
"#,
    ),
    (
        r#"x = \sl Staff.Foo
$(say (if (defined? 'x) "+" "-"))"#,
        r#"tl? "Staff"
tl? (Staff)
-tl? (Staff Foo)
sl (Staff Foo)
"#,
    ),
];

#[test]
fn functions_apply_as_lilypond_applies_them() {
    let Some(mut vm) = library() else { return };
    for (input, expected) in CASES {
        assert_eq!(stderr(&mut vm, input), *expected, "{input}");
    }
}
