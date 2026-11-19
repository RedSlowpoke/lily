//! The warnings of `warn-scheme.cc`, `input-scheme.cc` and
//! `lily-parser-scheme.cc` that music functions and `music-functions.scm`
//! call (`ly:music-warning` and `ly:music-message` are Scheme over them):
//! `ly:warning`, `ly:input-warning`, `ly:message`, `ly:input-message`,
//! `ly:deprecation-warning`, `ly:programming-error`, `ly:parser-error` and
//! `ly:expect-warning` (MS2 step 37) and `ly:check-expected-warnings`
//! (MS8 step 11, the `check` kind);
//! and `general-scheme.cc`'s `ly:number->string`, whose programming error
//! is one of them (MS2 step 33, T23's template); `ly:translate-cpp-warning-scheme`,
//! which turns a C++ warning's text into the one `ly:expect-warning` takes
//! (MS5 step 8, M01).
//!
//! Their text is LilyPond's, but where it goes is the parser's: a warning
//! at an input location prints the excerpt of the parser's source, and a
//! parser error fails the file. So each is kept in `%diagnostics` of
//! `(lily)`, the last first, as `#(kind location text mark)`, `location` an
//! [`crate::Input`] or `#f`, and the parser takes them after each Scheme
//! call into its diagnostics (`lily_engine_parser`'s `take_diagnostics`).
//! `kind` is `warning`, `message`, `programming-error` or `error`, or
//! `expect` for a warning expected from there on, which those who take
//! them keep in its place (D96). A fourth
//! item, `%diagnostic-mark`'s, is how much of what the error port got
//! `force-output` had flushed when the message was given: LilyPond prints
//! its messages at once and Guile's port when it is flushed, so the parser
//! puts each message after that much of the port's text (08 §6).

use lily_engine_scheme::{Cx, Value};

/// `(%diagnostic-mark)`: [`Cx::error_port_flushed`].
pub(crate) fn mark(cx: &mut Cx, _: &[Value]) -> crate::R {
    let flushed = cx.error_port_flushed() as i64;
    Ok(cx.heap_mut().integer(flushed))
}

/// The definitions, in `(lily)`. A message's format is Guile's
/// `simple-format`'s, which `format` covers.
pub(crate) const PRELUDE: &str = r#"
(define-public %diagnostics (list '()))
(define (%diagnose kind location text)
  (set-car! %diagnostics
           (cons (vector kind location text (%diagnostic-mark)) (car %diagnostics))))
(define (%diagnostic-string subr position str)
  (if (not (string? str))
      (scm-error 'wrong-type-arg subr
                 "Wrong type argument in position ~A (expecting ~A): ~S"
                 (list position "string" str) (list str)))
  str)
(define (%diagnostic-text subr position str rest)
  (apply format #f (%diagnostic-string subr position str) rest))
(define (%diagnostic-input subr sip)
  (if (not (ly:input-location? sip))
      (scm-error 'wrong-type-arg subr
                 "Wrong type argument in position ~A (expecting ~A): ~S"
                 (list 1 "Input" sip) (list sip))))
(define-public (ly:warning str . rest)
  (%diagnose 'warning #f (%diagnostic-text "ly:warning" 1 str rest)))
(define-public (ly:message str . rest)
  (%diagnose 'message #f (%diagnostic-text "ly:message" 1 str rest)))
(define-public (ly:programming-error str . rest)
  (%diagnose 'programming-error #f (%diagnostic-text "ly:programming-error" 1 str rest)))
(define-public (ly:input-warning sip msg . rest)
  (%diagnostic-input "ly:input-warning" sip)
  (%diagnose 'warning sip (%diagnostic-text "ly:input-warning" 2 msg rest)))
(define-public (ly:input-message sip msg . rest)
  (%diagnostic-input "ly:input-message" sip)
  (%diagnose 'message sip (%diagnostic-text "ly:input-message" 2 msg rest)))
(define %deprecation-warnings (make-hash-table))
(define-public (ly:deprecation-warning str . rest)
  (let ((text (%diagnostic-text "ly:deprecation-warning" 1 str rest)))
    (and (not (hash-ref %deprecation-warnings text #f))
         (begin (hash-set! %deprecation-warnings text #t)
                (%diagnose 'warning #f text)
                #t))))
(define-public (ly:expect-warning str . rest)
  (%diagnose 'expect #f (%diagnostic-text "ly:expect-warning" 1 str rest)))
(define-public (ly:check-expected-warnings)
  (%diagnose 'check #f ""))
(define-public (ly:parser-error msg . input)
  (%diagnose 'error (and (pair? input) (ly:input-location? (car input)) (car input))
             (%diagnostic-string "ly:parser-error" 1 msg)))
"#;

/// `%diagnose` from Rust: a message of `kind` at no location, kept in
/// `%diagnostics` as the Scheme procedures keep theirs.
pub(crate) fn diagnose(cx: &mut Cx, kind: &str, text: &str) {
    let Some(cell) = cx.global("lily", "%diagnostics") else {
        return;
    };
    let mark = cx.error_port_flushed() as i64;
    let heap = cx.heap_mut();
    let kept = heap.car(cell).unwrap_or(Value::NULL);
    let item = alloc::vec![
        heap.intern(kind),
        Value::FALSE,
        heap.string(text),
        heap.integer(mark)
    ];
    let item = heap.vector(item);
    let kept = heap.cons(item, kept);
    heap.set_car(cell, kept);
}

/// `(ly:number->string s)`: an inexact number as `%.4f` prints it, an
/// infinity or NaN as 0 after a programming error; an exact integer in
/// decimal. An exact number past `long long`, or no integer, is no
/// argument `from_scm<long long>` takes.
pub(crate) fn number_to_string(cx: &mut Cx, args: &[Value]) -> crate::R {
    const SUBR: &str = "ly:number->string";
    let s = args[0];
    let heap = cx.heap();
    if !heap.is_number(s) {
        return Err(cx.wrong_type(SUBR, 1, s, "number"));
    }
    let text = match heap.is_exact(s) {
        Ok(false) => {
            let Some(mut r) = heap.to_f64(s) else {
                return Err(cx.wrong_type(SUBR, 1, s, "real number"));
            };
            if !r.is_finite() {
                diagnose(
                    cx,
                    "programming-error",
                    "infinity or NaN encountered while converting Real number, setting to zero",
                );
                r = 0.0;
            }
            alloc::format!("{r:.4}")
        }
        _ => match heap.to_i64(s) {
            Some(n) => alloc::format!("{n}"),
            None => return Err(cx.wrong_type(SUBR, 1, s, "exact integer")),
        },
    };
    Ok(cx.heap_mut().string(&text))
}

/// `(ly:translate-cpp-warning-scheme str)`: a C++ `printf` format as
/// Guile's `format` takes it, `~` doubled, `%%` a `%`, `%s` and `%d` each
/// `~a`, any other `%` a `~`. Its `_` translates nothing: the engine's
/// messages are LilyPond's English.
pub(crate) fn translate_cpp_warning_scheme(cx: &mut Cx, args: &[Value]) -> crate::R {
    const SUBR: &str = "ly:translate-cpp-warning-scheme";
    let Some(s) = cx.heap().str(args[0]) else {
        return Err(cx.wrong_type(SUBR, 1, args[0], "string"));
    };
    let mut result = alloc::string::String::new();
    let mut chars = s.as_str().chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '~' => result.push_str("~~"),
            '%' => match chars.peek() {
                Some('%') => {
                    result.push('%');
                    chars.next();
                }
                Some('s' | 'd') => {
                    result.push_str("~a");
                    chars.next();
                }
                _ => result.push('~'),
            },
            c => result.push(c),
        }
    }
    Ok(cx.heap_mut().string(result))
}
