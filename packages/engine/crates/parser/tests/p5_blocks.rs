//! P5, part 2 (02 §6): `\score`, `\header`, `\paper`, `\layout`, `\midi`,
//! `\book` and `\bookpart` as nested scopes with LilyPond's placement rules,
//! and number expressions with units, in a file's module that uses a
//! `(lily)` loaded from the installed LilyPond 2.26.0 (`tests/support`).
//! Each expected stderr was printed by LilyPond 2.26.0 (`lilypond
//! --loglevel=WARNING`) for the same input, less the warnings of processing,
//! which a parser alone does not reach (D64). [`PRELUDE`]'s `say` writes a
//! value to the error port, which shows what a scope holds; what it writes
//! comes where Guile flushes the port. The error corpus's cases 11 and 28
//! are scored with C0 by `tools/compare/test/c0-cases.test.mjs`. Every test
//! skips itself, and so verifies nothing, without that lilypond.

use lily_engine_parser::{NoteNames, parse};
use lily_engine_scheme::Vm;
use std::sync::OnceLock;

mod support;

const V: &str = "\\version \"2.26.0\"\n";

const PRELUDE: &str =
    "#(define (say x) (write x (current-error-port)) (newline (current-error-port)))";

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
/// - number expressions: `+`, `-`, one `*` or `/` a term, unary `-`, units
///   after `UNSIGNED`s and `REAL`s, `NUMBER_IDENTIFIER`s, the initial
///   mode's `REAL`s (`-2`, `1.`, `.5`), and the syntax errors of a missing
///   factor or a second `*`;
/// - units in `\paper` and `\layout`, none in `\midi` or at toplevel;
/// - scopes: what an output definition, a header and a score's header
///   define stays in them; a toplevel `\paper`, `\layout` or `\header`
///   becomes the default that the next one copies; a book's `\paper` is the
///   one its later ones copy; an output definition first in a body replaces
///   it, anywhere else it is `bad expression type`;
/// - placement: `\paper` in a score, `need \paper for paper block`,
///   spurious music and values in a score, a score as Scheme, a header as a
///   value copied into a score's, a part in a part or a book in a book;
/// - recovery in an output definition's body, a book's and a score's, and
///   at toplevel after a block;
/// - §5.4's timing: a block as a value is defined before the next token is
///   lexed, a number expression after.
///
/// Not here, as the engine differs: `Missing music in \score`, after which
/// LilyPond's processing fails on the empty score; music in an output
/// definition, which LilyPond's `output-def-music-handler` warns about
/// (P6).
const CASES: &[(&str, &str)] = &[
    (
        r##"\paper { x = 5\mm #(say x) }"##,
        r##"5.0
"##,
    ),
    (
        r##"\paper { x = 2\cm + 3\mm #(say x) y = 1 + 2 + 3 #(say y) z = 2 * 3 #(say z) w = 3 / 4 #(say w) }"##,
        r##"23.0
6
6
3/4
"##,
    ),
    (
        r##"\paper { x = 2 * 3 * 4 #(say x) }"##,
        r##"f.ly:3:20: error: syntax error, unexpected '*'
\paper { x = 2 * 3 
                   * 4 #(say x) }
6
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { x = -\mm #(say x) y = - 2 #(say y) z = -2 #(say z) u = 1.5\in #(say u) v = 4 \pt #(say v) t = -.5 #(say t) }"##,
        r##"-1.0
-2
-2
38.099999999999994
1.4058392140583922
-0.5
"##,
    ),
    (
        r##"x = 1 + 2
#(say x)
y = 3 - 4 * 2
#(say y)
z = - 4
#(say z)"##,
        r##"3
-5
-4
"##,
    ),
    (
        r##"x = 5\mm
#(say x)"##,
        r##"f.ly:3:6: error: unknown command: `\mm'
x = 5
     \mm
f.ly:4:1: error: syntax error, unexpected SCM_TOKEN, expecting '.' or '='

#(say x)
5
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\layout { x = 1\mm #(say x) #(say (list mm cm in pt bp)) }"##,
        r##"1.0
(1.0 10.0 25.4 0.35145980351459805 0.35277777777777775)
"##,
    ),
    (
        r##"\midi { x = 1\mm #(say x) }"##,
        r##"f.ly:3:14: error: unknown command: `\mm'
\midi { x = 1
             \mm #(say x) }
f.ly:3:18: error: syntax error, unexpected SCM_TOKEN, expecting '.' or '='
\midi { x = 1\mm 
                 #(say x) }
1
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { #(say (list mm cm in pt bp output-def-kind)) }"##,
        r##"(1.0 10.0 25.4 0.35145980351459805 0.35277777777777775 paper)
"##,
    ),
    (
        r##"\paper { y = 1 }
#(say (defined? 'y))
\paper { #(say y) }"##,
        r##"#f
1
"##,
    ),
    (
        r##"\header { title = "t" }
#(say (defined? 'title))
\header { #(say title) }"##,
        r##"#f
"t"
"##,
    ),
    (
        r##"\score { { c } \paper { } }"##,
        r##"f.ly:3:16: error: \paper cannot be used in \score, use \layout instead
\score { { c } 
               \paper { } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\score { { c } { d } }"##,
        r##"f.ly:3:16: error: Spurious expression in \score
\score { { c } 
               { d } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\score { { c } \header { piece = "p" #(say piece) } \layout { } \midi { } }"##,
        r##""p"
"##,
    ),
    (
        r##"\book { \paper { } \score { { c } } \bookpart { { d } } }"##,
        r##""##,
    ),
    (
        r##"\book { \layout { } }"##,
        r##"f.ly:3:9: error: need \paper for paper block
\book { 
        \layout { } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\bookpart { \bookpart { } }"##,
        r##"f.ly:3:13: error: syntax error, unexpected \bookpart
\bookpart { 
            \bookpart { } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\book { \book { } }"##,
        r##"f.ly:3:9: error: syntax error, unexpected \book
\book { 
        \book { } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { \foo }"##,
        r##"f.ly:3:10: error: unknown command: `\foo'
\paper { 
         \foo }
f.ly:3:15: error: syntax error, unexpected '}', expecting '.' or '='
\paper { \foo 
              }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { c4 }"##,
        r##"f.ly:3:11: error: syntax error, unexpected UNSIGNED, expecting '.' or '='
\paper { c
          4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"p = \paper { x = 2 }
\paper { \p #(say x) }"##,
        r##"2
"##,
    ),
    (
        r##"p = \paper { x = 2 }
\paper { y = 1 \p }"##,
        r##"f.ly:4:16: error: bad expression type
\paper { y = 1 
               \p }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"p = \paper { x = 2 }
\score { { c } \p }"##,
        r##"f.ly:4:16: error: \paper cannot be used in \score, use \layout instead
\score { { c } 
               \p }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"l = \layout { x = 2 }
\book { \l }"##,
        r##"f.ly:4:9: error: need \paper for paper block
\book { 
        \l }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { x = }
y = 3
#(say y)"##,
        r##"f.ly:3:14: error: syntax error, unexpected '}', expecting \header
\paper { x = 
             }
3
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"x = \score { { c } }
\x
\book { \x }
y = \book { }
\y"##,
        r##""##,
    ),
    (
        r##"\paper { x = 1/2 #(say x) }"##,
        r##"(1 . 2)
"##,
    ),
    (
        r##"\paper { x = 1 + c }"##,
        r##"f.ly:3:18: error: syntax error, unexpected SYMBOL, expecting UNSIGNED or NUMBER_IDENTIFIER or REAL or '-'
\paper { x = 1 + 
                 c }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"x = 1 + c"##,
        r##"f.ly:3:9: error: syntax error, unexpected NOTENAME_PITCH, expecting UNSIGNED or NUMBER_IDENTIFIER or REAL or '-'
x = 1 + 
        c
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\book { \paper { x = 1 } \paper { #(say x) } }"##,
        r##"1
"##,
    ),
    (
        r##"\book { x = 1 }"##,
        r##"f.ly:3:9: error: syntax error, unexpected SYMBOL
\book { 
        x = 1 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\bookpart { \layout { } }"##,
        r##"f.ly:3:13: error: need \paper for paper block
\bookpart { 
            \layout { } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\layout { x = 1 }
\layout { #(say x) }"##,
        r##"1
"##,
    ),
    (
        r##"\paper x"##,
        r##"f.ly:3:8: error: syntax error, unexpected SYMBOL, expecting '{'
\paper 
       x
f.ly:3:9: error: Unfinished main input
\paper x
        
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\score { { c } \layout }"##,
        r##"f.ly:3:24: error: syntax error, unexpected '}', expecting '{'
\score { { c } \layout 
                       }
f.ly:3:25: error: Unfinished main input
\score { { c } \layout }
                        
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\score { { c } \header { title = 1 } \header { #(say title) } }"##,
        r##"1
"##,
    ),
    (
        r##"\paper { "str" = 3 #(say str) }"##,
        r##"3
"##,
    ),
    (
        r##"\paper { $(+ 1 2) }"##,
        r##"f.ly:3:10: error: syntax error, unexpected NUMBER_IDENTIFIER
\paper { 
         $(+ 1 2) }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\score { { c } $(+ 1 2) }"##,
        r##"f.ly:3:16: error: syntax error, unexpected NUMBER_IDENTIFIER, expecting '}'
\score { { c } 
               $(+ 1 2) }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\score { { c } #(+ 1 2) #(define q 1) }"##,
        r##"f.ly:3:16: error: Spurious expression in \score
\score { { c } 
               #(+ 1 2) #(define q 1) }
fatal error: failed files: "f.ly"
"##,
    ),
    (r##"\score { #(make-music 'SequentialMusic) }"##, r##""##),
    (
        r##"\paper { x = 1\mm * 2 #(say x) y = 2 * -3 #(say y) z = 1 - -2 #(say z) }"##,
        r##"2.0
-6
3
"##,
    ),
    (
        r##"\paper { x = 1 - }"##,
        r##"f.ly:3:18: error: syntax error, unexpected '}', expecting UNSIGNED or NUMBER_IDENTIFIER or REAL or '-'
\paper { x = 1 - 
                 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { x = 3 * }"##,
        r##"f.ly:3:18: error: syntax error, unexpected '}', expecting UNSIGNED or NUMBER_IDENTIFIER or REAL or '-'
\paper { x = 3 * 
                 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { x = \mm \mm }"##,
        r##"f.ly:3:18: error: syntax error, unexpected NUMBER_IDENTIFIER
\paper { x = \mm 
                 \mm }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { x = 1 } \book { \paper { #(say x) } }"##,
        r##"1
"##,
    ),
    (
        r##"\header { title = "a" }
\book { \header { #(say title) } }"##,
        r##""a"
"##,
    ),
    (r##"\score { \header { } { c } }"##, r##""##),
    // `ly:parser-lookup` sees what `ly:parser-define!` defined before it
    // in the same Scheme; the toplevel bookpart handler puts the scores
    // before a part in a part of their own (`collect-bookpart-for-book`).
    (
        r##"#(begin (ly:parser-define! 'zz 1) (say (ly:parser-lookup 'zz)))
\score { { c } }
\bookpart { { d } }
#(say (map (lambda (p) (length (ly:book-scores p))) toplevel-bookparts))
#(say (length toplevel-scores))"##,
        r##"1
(1 1)
0
"##,
    ),
    // Texts go to the text handlers (MS3 step 63): a `\markup`, a markup
    // as Scheme or an identifier in a list of its own, a `\markuplist` as
    // it is, in the file's order at toplevel; a book's and a part's beside
    // their scores, a score as Scheme among them.
    (
        r##"m = \markup e
\markup { a }
\markuplist { b c }
$"d"
\m
#(say (map length (reverse toplevel-scores)))"##,
        r##"(1 2 1 1)
"##,
    ),
    (
        r##"sc = \score { { c } }
\book {
  \markup a
  \markuplist { b c d }
  $"e"
  $sc
  #(say (sort (map (lambda (x) (if (pair? x) (length x) 0)) (ly:book-scores $current-book)) <))
  \bookpart { \markup f $(markup "g") \markuplist { h i } $sc
    #(say (sort (map (lambda (x) (if (pair? x) (length x) 0)) (ly:book-scores $current-bookpart)) <)) }
}"##,
        r##"(0 1 1 3)
(0 1 1 2)
"##,
    ),
    (
        r##"\book { { c } \header { title = "x" } \bookpart { \header { #(say title) } { d } } }"##,
        r##"f.ly:3:62: error: Guile signaled an error for the expression beginning here
\book { { c } \header { title = "x" } \bookpart { \header { #
                                                             (say title) } { d } } }
Unbound variable: title
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { x = 1 y }
z = 2
#(say z)"##,
        r##"f.ly:3:18: error: syntax error, unexpected '}', expecting '.' or '='
\paper { x = 1 y 
                 }
2
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\book { } }"##,
        r##"f.ly:3:11: error: syntax error, unexpected '}'
\book { } 
          }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\score { { c } } }"##,
        r##"f.ly:3:18: error: syntax error, unexpected '}'
\score { { c } } 
                 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { x = 1 } }"##,
        r##"f.ly:3:18: error: syntax error, unexpected '}'
\paper { x = 1 } 
                 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { indent = 0 ragged-right = ##t line-width = 100\mm - 2\cm #(say line-width) }"##,
        r##"80.0
"##,
    ),
    (
        r##"\paper { x = 4 * \mm #(say x) y = \mm / 2 #(say y) }"##,
        r##"4.0
0.5
"##,
    ),
    (
        r##"\paper { x = 1 \cm #(say x) }"##,
        r##"10.0
"##,
    ),
    (
        r##"\paper { x = 1. #(say x) y = .5 #(say y) z = 1.\mm #(say z) }"##,
        r##"1.0
0.5
1.0
"##,
    ),
    (
        r##"x = 1.5
#(say x)
y = 2 \pt"##,
        r##"f.ly:5:7: error: unknown command: `\pt'
y = 2 
      \pt
f.ly:5:10: error: syntax error, unexpected end of input, expecting '.' or '='
y = 2 \pt
         
1.5
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"x = \paper { }
$(say (if (defined? 'x) "+" "-"))"##,
        r##""+"
"##,
    ),
    (
        r##"x = \header { title = "t" }
\header { \x }
\header { #(say title) }"##,
        r##""t"
"##,
    ),
    (
        r##"x = \score { { c } }
$(say (if (defined? 'x) "+" "-"))"##,
        r##""+"
"##,
    ),
    (
        r##"x = \book { }
$(say (if (defined? 'x) "+" "-"))"##,
        r##""+"
"##,
    ),
    (
        r##"x = 2 + 3
$(say (if (defined? 'x) "+" "-"))"##,
        r##""-"
"##,
    ),
    (
        r##"\paper { x = 1 = 2 y = 3 #(say y) }"##,
        r##"f.ly:3:16: error: syntax error, unexpected '='
\paper { x = 1 
               = 2 y = 3 #(say y) }
3
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\book { = \score { { c d } } }"##,
        r##"f.ly:3:9: error: syntax error, unexpected '='
\book { 
        = \score { { c d } } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\book { \bookpart { \paper { x = 1 } } \paper { #(say (defined? 'x)) } }"##,
        r##"#f
"##,
    ),
    (
        r##"\score { { c } \layout { indent = 2\cm #(say indent) } }"##,
        r##"20.0
"##,
    ),
    (
        r##"\midi { #(say output-def-kind) }
\layout { #(say output-def-kind) }"##,
        r##"midi
layout
"##,
    ),
    (
        r##"l = \layout { x = 3 }
\layout { \l #(say x) }
\paper { \l #(say output-def-kind) }
\layout { #(say x) }"##,
        r##"3
layout
3
"##,
    ),
    (
        r##"\score { { c } \layout { } \paper { } }"##,
        r##"f.ly:3:28: error: \paper cannot be used in \score, use \layout instead
\score { { c } \layout { } 
                           \paper { } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\header { title = 1 }
\paper { #(say (defined? 'title)) }"##,
        r##"#f
"##,
    ),
    (
        r##"\book { \header { title = } { c } }"##,
        r##"f.ly:3:27: error: syntax error, unexpected '}', expecting \header
\book { \header { title = 
                          } { c } }
f.ly:3:35: error: syntax error, unexpected '}'
\book { \header { title = } { c } 
                                  }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\score { { c } \layout { x = } }"##,
        r##"f.ly:3:30: error: syntax error, unexpected '}', expecting \header
\score { { c } \layout { x = 
                             } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { x = 1 \cm \mm }"##,
        r##"f.ly:3:20: error: syntax error, unexpected NUMBER_IDENTIFIER
\paper { x = 1 \cm 
                   \mm }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\paper { x = 1 }
x = 2
#(say x)"##,
        r##"2
"##,
    ),
    (
        r##"foo = 2
x = 2.5 \foo
#(say x)"##,
        r##"5.0
"##,
    ),
    (
        r##"\paper { x = #(+ 1 2) y = \x #(say y) z = ##t #(say z) s = "s" #(say s) }"##,
        r##"3
#t
"s"
"##,
    ),
    (
        r##"\paper { x = a.b #(say x) y.z = 3 #(say y) }"##,
        r##"(a b)
((z . 3))
"##,
    ),
    (
        r##"\paper { \header { } }"##,
        r##"f.ly:3:10: error: syntax error, unexpected \header
\paper { 
         \header { } }
fatal error: failed files: "f.ly"
"##,
    ),
    (r##"\book { \paper { } \paper { } }"##, r##""##),
    (
        r##"\score { \score { { c } } }"##,
        r##"error: Missing music in \score
f.ly:3:10: error: syntax error, unexpected \score, expecting '}'
\score { 
         \score { { c } } }
fatal error: failed files: "f.ly"
"##,
    ),
    (r##"\score { { c } \header { title = "x" } }"##, r##""##),
    (
        r##"\book { \score { { c } } \score { { d } } { e } \markup "m" #(define q 1) }"##,
        r##""##,
    ),
    (
        r##"\bookpart { \paper { } \score { { c } } \header { title = "x" } }"##,
        r##""##,
    ),
    (
        r##"\paper { x = 1 } \paper { y = 2 } \paper { #(say (list x y)) }"##,
        r##"(1 2)
"##,
    ),
    (
        r##"x = -\foo"##,
        r##"f.ly:3:6: error: unknown command: `\foo'
x = -
     \foo
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"foo = 3
x = -\foo
#(say x)"##,
        r##"-3
"##,
    ),
    (
        r##"foo = 3
x = \foo * 2 + 1
#(say x)"##,
        r##"7
"##,
    ),
    (
        r##"\paper { x = 2 / 4 #(say x) y = 1.0 / 4 #(say y) }"##,
        r##"1/2
0.25
"##,
    ),
    (
        r##"s = \score { { c } }
\score { \s }
\score { \s { d } }"##,
        r##"f.ly:3:1: error: syntax error, unexpected RESTNAME

s = \score { { c } }
f.ly:4:10: error: unknown command: `\s'
\score { 
         \s }
f.ly:4:10: error: string outside of text script or \lyricmode
\score { 
         \s }
f.ly:5:10: error: unknown command: `\s'
\score { 
         \s { d } }
f.ly:5:10: error: string outside of text script or \lyricmode
\score { 
         \s { d } }
f.ly:5:13: error: Spurious expression in \score
\score { \s 
            { d } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"h = \header { piece = "p" }
\score { { c } \h }"##,
        r##""##,
    ),
    (
        r##"\score { \header { title = 1 } \header { #(say title) } { c } }"##,
        r##"1
"##,
    ),
];

#[test]
fn blocks_and_numbers_parse_as_lilypond_parses_them() {
    let Some(lily) = lily() else { return };
    let mut vm = support::library(lily).expect("the library loads");
    for (input, expected) in CASES {
        assert_eq!(stderr(&mut vm, input), *expected, "{input}");
    }
}
