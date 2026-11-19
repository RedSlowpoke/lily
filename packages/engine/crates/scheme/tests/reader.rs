//! S2's reader (chapter 03 §5.2, R7, R8): Guile 3.0.11's `read` at a byte
//! offset, its errors, the spans of what it read, and LilyPond's `#{ … #}`.
//! The table's expected texts are Guile 3.0.11's, with LilyPond's
//! `read-lily-expression` installed; with `guile` installed, the table is
//! also run by guile and compared, and every `.scm` of the installed LilyPond
//! 2.26.0's `scm/` is read by both, datum by datum.
use std::path::{Path, PathBuf};
use std::process::Command;

use lily_engine_scheme::{Heap, LilyScan, MAX_DEPTH, Position, Value, read};

/// The file name the table's ports have.
const FILE: &str = "f.scm";

/// Texts, and what Guile 3.0.11 gives for `(read port)` on them: the datum
/// written and the byte the port is left at, or the error's key and text.
const CASES: &[(&str, &str)] = &[
    // Error case 21 of chapter 02's corpus, from its `#`.
    (
        "(define x (list 1 2)\n{ c4 }\n",
        "read-error: f.scm:3:1: unexpected end of input while searching for: )",
    ),
    (")", "read-error: f.scm:1:2: unexpected \")\""),
    ("]", "read-error: f.scm:1:2: unexpected \"]\""),
    ("(a ]", "read-error: f.scm:1:5: mismatched close paren: ]"),
    ("[a)", "read-error: f.scm:1:4: mismatched close paren: )"),
    ("(a . b c)", "read-error: f.scm:1:9: missing close paren: c"),
    (
        "(a . b",
        "read-error: f.scm:1:7: missing close paren: #<eof>",
    ),
    ("(a .)", "read-error: f.scm:1:6: unexpected \")\""),
    (
        "(a . ",
        "read-error: f.scm:1:6: unexpected end of input while reading tail of improper list",
    ),
    (
        "(1 . 2 . 3)",
        "read-error: f.scm:1:9: missing close paren: .",
    ),
    ("( . a)", "a @6"),
    ("(a . b)", "(a . b) @7"),
    ("[a . b]", "(a . b) @7"),
    (". a", "#{.}# @1"),
    (
        "\"abc",
        "read-error: f.scm:1:5: unexpected end of input while reading string",
    ),
    (
        "\"\\q\"",
        "read-error: f.scm:1:4: invalid character in escape sequence: #\\q",
    ),
    (
        "\"\\x4\"",
        "read-error: f.scm:1:6: invalid character in escape sequence: #\\\"",
    ),
    ("\"\\x41\"", "\"A\" @6"),
    ("\"\\u00e9\\U01F600\"", "\"é😀\" @16"),
    ("\"a\\\nb\"", "\"ab\" @6"),
    ("\"\\(\"", "\"(\" @4"),
    (
        "\"\\0\\f\\n\\r\\t\\a\\v\\b\\|\\\\\"",
        "\"\\x00\\f\\n\\r\\t\\a\\v\\b|\\\\\" @22",
    ),
    (
        "#\\foo",
        "read-error: f.scm:1:6: unknown character name foo",
    ),
    (
        "#\\",
        "read-error: f.scm:1:3: unexpected end of input after #\\",
    ),
    ("#\\(a", "#\\( @3"),
    ("#\\x;", "#\\x @3"),
    ("#\\x", "#\\x @3"),
    ("#\\xa", "#\\newline @4"),
    ("#\\x41", "#\\A @5"),
    (
        "#\\X41",
        "read-error: f.scm:1:6: unknown character name X41",
    ),
    ("#\\101", "#\\A @5"),
    ("#\\10", "#\\backspace @4"),
    ("#\\18", "read-error: f.scm:1:5: unknown character name 18"),
    ("#\\Space", "#\\space @7"),
    ("#\\SPACE", "#\\space @7"),
    ("#\\NUL", "#\\nul @5"),
    ("#\\nl", "#\\newline @4"),
    ("#\\λ", "#\\λ @4"),
    ("#\\λx", "read-error: f.scm:1:5: unknown character name λx"),
    (
        "#\\x110000",
        "out-of-range: Argument 1 out of range: 1114112",
    ),
    ("#\\xD800", "out-of-range: Argument 1 out of range: 55296"),
    (
        "#\\777777777",
        "out-of-range: Argument 1 out of range: 134217727",
    ),
    (
        "#\\1/2",
        "wrong-type-arg: Wrong type argument in position 1 (expecting small integer): 1/2",
    ),
    (
        "#",
        "read-error: f.scm:1:2: unexpected end of input after #",
    ),
    ("#q", "read-error: f.scm:1:3: Unknown # object: \"#q\""),
    ("#xyz", "read-error: f.scm:1:5: unknown # object: \"#xyz\""),
    (
        "#e+inf.0",
        "read-error: f.scm:1:9: unknown # object: \"#e+inf.0\"",
    ),
    ("#x-1F", "-31 @5"),
    ("#e1.5", "3/2 @5"),
    ("#e1e400", "out-of-range: Value out of range: 400"),
    ("1e400", "out-of-range: Value out of range: 400"),
    ("#e1.5e-400", "out-of-range: Value out of range: -400"),
    (
        "1e400000000000000000000",
        "out-of-range: Value out of range: 400000000000000000000",
    ),
    ("1/0", "#{1/0}# @3"),
    ("1+", "#{1+}# @2"),
    ("+", "+ @1"),
    ("...", "... @3"),
    (".5", "0.5 @2"),
    ("-.5e2", "-50.0 @5"),
    ("1/4)", "1/4 @3"),
    (
        "#:1",
        "read-error: f.scm:1:4: keyword prefix #: not followed by a symbol: 1",
    ),
    ("#: foo", "#:foo @6"),
    (
        "#:",
        "read-error: f.scm:1:3: unexpected end of input while reading keyword",
    ),
    ("#:)", "read-error: f.scm:1:4: unexpected \")\""),
    (":foo", ":foo @4"),
    (
        "\t\t(a",
        "read-error: f.scm:1:19: unexpected end of input while searching for: )",
    ),
    ("  a\tb (c", "a @3"),
    ("λλ (a", "λλ @4"),
    ("abc#d", "#{abc#d}# @5"),
    ("a;b", "a @1"),
    ("abc\x0b;def", "#{abc\\xb;}# @4"),
    ("|a b|", "|a @2"),
    ("{ c4 }", "#{\\x7b;}# @1"),
    (
        "#;",
        "read-error: f.scm:1:3: unexpected end of input while reading #; comment",
    ),
    ("#;a b", "b @5"),
    ("(a #;b c)", "(a c) @9"),
    ("(a #;)", "read-error: f.scm:1:7: unexpected \")\""),
    (
        "#| abc",
        "read-error: f.scm:1:7: unterminated `#| ... |#' comment",
    ),
    ("(a #| x #| y |# z |# b)", "(a b) @23"),
    (
        "#! abc",
        "read-error: f.scm:1:7: unterminated `#! ... !#' comment",
    ),
    ("(#!x y !# a)", "(a) @12"),
    ("#!/bin/sh\n!#\nz", "z @14"),
    ("#!fold-case ABC", "abc @15"),
    ("#!fold-case #!no-fold-case ABC", "ABC @30"),
    ("#!r6rs \"\\x41;\"", "\"A\" @14"),
    ("; x\n; y\n", "#<eof> @8"),
    ("", "#<eof> @0"),
    ("#tru1", "#t @2"),
    ("#true1", "#t @5"),
    ("#fAlse", "#f @6"),
    ("#T", "#t @2"),
    ("#F", "#f @2"),
    ("#false", "#f @6"),
    (
        "'",
        "read-error: f.scm:1:2: unexpected end of input while reading quoted expression",
    ),
    (
        "`",
        "read-error: f.scm:1:2: unexpected end of input while reading quasiquoted expression",
    ),
    (
        ",",
        "read-error: f.scm:1:2: unexpected end of input while reading unquoted expression",
    ),
    (
        ",@",
        "read-error: f.scm:1:3: unexpected end of input while reading subexpression of ,@",
    ),
    (
        "#'",
        "read-error: f.scm:1:3: unexpected end of input while reading syntax expression",
    ),
    (
        "'(a `(b ,c ,@d))",
        "(quote (a (quasiquote (b (unquote c) (unquote-splicing d))))) @16",
    ),
    ("#'a #`b", "(syntax a) @3"),
    (
        "#`(a #,b #,@c)",
        "(quasisyntax (a (unsyntax b) (unsyntax-splicing c))) @14",
    ),
    ("#(1 \"a\" #(b))", "#(1 \"a\" #(b)) @13"),
    ("#(1 . 2)", "wrong-type-arg: Not a list: (1 . 2)"),
    (
        "#(a b",
        "read-error: f.scm:1:6: unexpected end of input while searching for: )",
    ),
    ("#vu8(#xFE #xFF)", "#vu8(254 255) @15"),
    ("#vu8()", "#vu8() @6"),
    ("#vu8(256)", "out-of-range: Value out of range: 256"),
    ("#vu8(-1)", "out-of-range: Value out of range: -1"),
    (
        "#vu8(a)",
        "wrong-type-arg: Wrong type argument in position 3: a",
    ),
    (
        "#vu8(1.0)",
        "wrong-type-arg: Wrong type argument in position 3: 1.0",
    ),
    ("#vu8(1 . 2)", "wrong-type-arg: Not a list: (1 . 2)"),
    ("#vu", "read-error: f.scm:1:4: invalid bytevector prefix"),
    (
        "#vu8 (1)",
        "read-error: f.scm:1:6: invalid bytevector prefix",
    ),
    ("#1@1(17 32)", "#1@1(17 32) @11"),
    ("#@1(\"s\" #\\a)", "#1@1(\"s\" #\\a) @12"),
    ("#1@-1(a)", "#1@-1(a) @8"),
    ("#1(1 2)", "#(1 2) @7"),
    ("#1@0(1 2)", "#(1 2) @9"),
    ("#1:2(1 2)", "#(1 2) @9"),
    ("#1@1:2(1 2)", "#1@1(1 2) @11"),
    ("#1@1()", "#1@1() @6"),
    (
        "#1:3(1 2)",
        "misc-error: too few elements for array dimension 0, need 3",
    ),
    (
        "#1:1(1 2)",
        "misc-error: too many elements for array dimension 0, want 1",
    ),
    (
        "#1@1:-1(1)",
        "read-error: f.scm:1:9: array length must be non-negative",
    ),
    (
        "#1@1@2(1)",
        "read-error: f.scm:1:10: the number of shape specifications must match the array rank",
    ),
    (
        "#1@",
        "read-error: f.scm:1:4: unexpected end of input while reading array",
    ),
    (
        "#1@x(1)",
        "read-error: f.scm:1:5: missing '(' in vector or array literal",
    ),
    ("#1@1(1 . 2)", "wrong-type-arg: Not a list: (1 . 2)"),
    ("#2((#t #f) (#f #t))", "#2((#t #f) (#f #t)) @19"),
    ("#2@1@1((1))", "#2@1@1((1)) @11"),
    ("#2()", "#2() @4"),
    ("#2(())", "#2(()) @6"),
    ("#2(() ())", "#2(() ()) @9"),
    ("#2:2:1((1) (2))", "#2((1) (2)) @15"),
    ("#2:0:3()", "#2:0:3() @8"),
    ("#3(((1)))", "#3(((1))) @9"),
    (
        "#2((1) (2 3))",
        "misc-error: too many elements for array dimension 1, want 1",
    ),
    (
        "#2((1 2) (3))",
        "misc-error: too few elements for array dimension 1, need 2",
    ),
    (
        "#2((a) 2)",
        "misc-error: too many elements for array dimension 1, want 1",
    ),
    (
        "#2(1 2)",
        "wrong-type-arg: Wrong type argument in position 1: 1",
    ),
    (
        "#2((1 . 2))",
        "wrong-type-arg: Wrong type argument in position 1: (1 . 2)",
    ),
    (
        "#2@1((1))",
        "read-error: f.scm:1:10: the number of shape specifications must match the array rank",
    ),
    // LilyPond's #{ … #}: LilyScan, as `read-lily-expression` reads it.
    (
        "#{ \\foo #x #(a b) $@y #'z c\" #}\" %{ x % y #}",
        "((@@ (lily) read-lily-expression-internal) \" \\\\foo #x #(a b) $@y #'z c\\\" #}\\\" %{ x % y \" \"f.scm\" 0 (list (cons 7 (lambda () x)) (cons 10 (lambda () (a b))) (cons 17 (lambda () (apply values y))))) @44",
    ),
    (
        "#{ a #b\n#}",
        "((@@ (lily) read-lily-expression-internal) \" a #b\\n\" \"f.scm\" 0 (list (cons 4 (lambda () b)))) @10",
    ),
    (
        "#{ a",
        "wrong-type-arg: Wrong type argument in position 1 (expecting character): #<eof>",
    ),
    (
        "#{ #mus#}",
        "wrong-type-arg: Wrong type argument in position 1 (expecting character): #<eof>",
    ),
    (
        "#{ %{ a %} #}",
        "((@@ (lily) read-lily-expression-internal) \" %{ a %} \" \"f.scm\" 0 (list)) @13",
    ),
    (
        "#{ #1 #}",
        "((@@ (lily) read-lily-expression-internal) \" #1 \" \"f.scm\" 0 (list)) @8",
    ),
    (
        "#{ λ #y #}",
        "((@@ (lily) read-lily-expression-internal) \" λ #y \" \"f.scm\" 0 (list (cons 5 (lambda () y)))) @11",
    ),
    (
        "\n\n  #{ #(quote x) $(f) #}",
        "((@@ (lily) read-lily-expression-internal) \" #(quote x) $(f) \" \"f.scm\" 2 (list (cons 13 (lambda () (f))))) @25",
    ),
    (
        "#{ #\"s\" #}",
        "((@@ (lily) read-lily-expression-internal) \" #\\\"s\\\" \" \"f.scm\" 0 (list)) @10",
    ),
    (
        "(a #{ b #} c)",
        "(a ((@@ (lily) read-lily-expression-internal) \" b \" \"f.scm\" 0 (list)) c) @13",
    ),
    // The scan reads `{` after `#` as Scheme, the symbol `{`: `##{` nests.
    (
        "#{ #{ #x #} #}",
        "((@@ (lily) read-lily-expression-internal) \" #{ #x \" \"f.scm\" 0 (list (cons 2 (lambda () #{\\x7b;}#)) (cons 5 (lambda () x)))) @11",
    ),
    (
        "#{ ##{ #x #} #}",
        "((@@ (lily) read-lily-expression-internal) \" ##{ #x #} \" \"f.scm\" 0 (list (cons 2 (lambda () ((@@ (lily) read-lily-expression-internal) \" #x \" \"f.scm\" 0 (list (cons 2 (lambda () x)))))))) @15",
    ),
];

/// Read without LilyPond's `#{ … #}`: Guile's own `#{…}#` symbols.
const PLAIN: &[(&str, &str)] = &[
    ("#{a b}#", "#{a b}# @7"),
    ("#{a}}#", "#{a\\x7d;}# @6"),
    ("#{a}b}#", "#{a\\x7d;b}# @7"),
    ("#{\\x41;\\(}#", "#{A\\x28;}# @11"),
    (
        "#{a",
        "read-error: f.scm:1:4: end of input while reading symbol",
    ),
    (
        "#{ a #}",
        "read-error: f.scm:1:8: end of input while reading symbol",
    ),
];

/// What Guile reads and the engine does not, with the engine's errors:
/// none is in LilyPond's Scheme.
const UNSUPPORTED: &[(&str, &str)] = &[
    (
        "#u8(1 2)",
        "read-error: f.scm:1:3: \"#u\" objects are not supported",
    ),
    (
        "#f32(1)",
        "read-error: f.scm:1:3: \"#f\" objects are not supported",
    ),
    (
        "#*101",
        "read-error: f.scm:1:3: \"#*\" objects are not supported",
    ),
    (
        "#nil",
        "read-error: f.scm:1:3: \"#n\" objects are not supported",
    ),
    (
        "#0(a)",
        "read-error: f.scm:1:4: \"#0(\" objects are not supported",
    ),
    (
        "#1u8(1)",
        "read-error: f.scm:1:6: \"#1u8(\" objects are not supported",
    ),
    (
        "#!curly-infix {a + b}",
        "read-error: f.scm:1:14: curly infix is not supported",
    ),
];

/// What the reader gives for `text`, as the tables write it; with
/// LilyPond's `#{ … #}` if `lily`.
fn run(heap: &mut Heap, text: &str, lily: bool) -> String {
    let mut scan = LilyScan {
        file: Some(FILE.into()),
    };
    let embedded = lily.then_some(&mut scan as &mut dyn lily_engine_scheme::Embedded);
    match read(heap, text, 0, embedded) {
        Ok(read) => {
            let mut out = String::new();
            heap.write(read.datum, &mut out);
            format!("{out} @{}", read.end)
        }
        Err(error) => {
            let at = Position::default().advance(&text[..error.at.unwrap_or(0)]);
            format!("{}: {}", error.key, error.text(heap, Some(FILE), at))
        }
    }
}

/// `scm/` of the installed LilyPond 2.26.0, if there is one.
fn lilypond_scm() -> Option<PathBuf> {
    let version = Command::new("lilypond").arg("--version").output().ok()?;
    if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
        return None;
    }
    let path = Command::new("sh")
        .args(["-c", "command -v lilypond"])
        .output()
        .ok()?;
    let binary = std::fs::canonicalize(String::from_utf8(path.stdout).ok()?.trim()).ok()?;
    let scm = binary.parent()?.join("../share/lilypond/2.26.0/scm");
    scm.join("lily/parser-ly-from-scheme.scm")
        .exists()
        .then_some(scm)
}

/// Guile's output for `program`, which `prelude` begins: LilyPond's
/// `read-lily-expression` installed if `scm` is given. `None` without guile.
fn guile(scm: Option<&Path>, program: &str, args: &[PathBuf]) -> Option<String> {
    let mut text = String::new();
    if let Some(scm) = scm {
        text.push_str(&format!(
            "(load \"{}\")\n",
            scm.join("lily/parser-ly-from-scheme.scm").display()
        ));
    }
    text.push_str(program);
    let out = Command::new("guile")
        .args(["--no-auto-compile", "-c", &text])
        .args(args)
        .output()
        .ok()?;
    assert!(
        out.status.success(),
        "guile failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(String::from_utf8(out.stdout).unwrap())
}

#[test]
fn reads_as_guile_does() {
    let mut heap = Heap::new();
    for (text, expected) in CASES.iter().chain(UNSUPPORTED) {
        assert_eq!(&run(&mut heap, text, true), expected, "reading {text:?}");
    }
    for (text, expected) in PLAIN {
        assert_eq!(&run(&mut heap, text, false), expected, "reading {text:?}");
    }
}

/// Guile's lines for `cases`, read as the tables are; `None` without guile.
fn guile_reads(scm: Option<&Path>, cases: &[&(&str, &str)]) -> Option<Vec<String>> {
    let quote = |text: &str| text.replace('\\', "\\\\").replace('"', "\\\"");
    let mut program = String::from(
        r#"(define (try text)
  (let ((p (open-input-string text)))
    (set-port-filename! p "f.scm")
    (catch #t
      (lambda () (let ((d (read p))) (write d) (display " @") (display (ftell p))))
      (lambda (key subr msg args rest)
        (display key) (display ": ")
        ;; One message of Guile's has an argument that it does not show.
        (display (catch #t (lambda () (apply simple-format #f msg args)) (lambda _ msg)))))
    (newline)))
"#,
    );
    for (text, _) in cases {
        program.push_str(&format!("(try \"{}\")\n", quote(text)));
    }
    let out = guile(scm, &program, &[])?;
    Some(out.lines().map(String::from).collect())
}

#[test]
fn the_tables_are_what_guile_reads() {
    // Without LilyPond, `#{` is Guile's own syntax, and those cases differ.
    let scm = lilypond_scm();
    let lily: Vec<_> = CASES
        .iter()
        .filter(|(text, _)| scm.is_some() || !text.contains("#{"))
        .collect();
    let plain: Vec<_> = PLAIN.iter().collect();
    for (scm, cases) in [(scm.as_deref(), lily), (None, plain)] {
        let Some(lines) = guile_reads(scm, &cases) else {
            eprintln!("guile is not installed: the tables were not compared with it");
            return;
        };
        assert_eq!(lines.len(), cases.len(), "guile's output:\n{lines:#?}");
        for ((text, expected), line) in cases.iter().zip(lines) {
            assert_eq!(&line, expected, "guile on {text:?}");
        }
    }
}

#[test]
fn reads_from_a_byte_offset_and_gives_spans() {
    let mut heap = Heap::new();
    let text = "\\version \"2.26.0\"\n#(set! x '(a (b . c)))  { c4 }";
    let offset = text.find('#').unwrap() + 1;
    let read = read(&mut heap, text, offset, None).unwrap();
    let mut out = String::new();
    heap.write(read.datum, &mut out);
    assert_eq!(out, "(set! x (quote (a (b . c))))");
    assert_eq!(&text[read.start..read.end], "(set! x '(a (b . c)))");
    assert!(!read.peeked, "a list ends at its parenthesis");
    let spans: Vec<_> = read
        .spans
        .iter()
        .map(|span| &text[span.start..span.end])
        .collect();
    assert_eq!(
        spans,
        [
            "(b . c)",
            "(a (b . c))",
            "'(a (b . c))",
            "(set! x '(a (b . c)))"
        ]
    );
    assert_eq!(read.spans.last().unwrap().value, read.datum);

    // A token ends where its delimiter is, which was looked at.
    let read = lily_engine_scheme::read(&mut heap, "  #t x", 0, None).unwrap();
    assert_eq!(
        (read.datum, read.start, read.end, read.peeked),
        (Value::TRUE, 2, 4, true)
    );
    let read = lily_engine_scheme::read(&mut heap, "#t", 0, None).unwrap();
    assert_eq!((read.datum, read.end, read.peeked), (Value::TRUE, 2, false));
    let read = lily_engine_scheme::read(&mut heap, "#tr", 0, None).unwrap();
    assert_eq!((read.datum, read.end, read.peeked), (Value::TRUE, 2, true));
    let read = lily_engine_scheme::read(&mut heap, "#true", 0, None).unwrap();
    assert_eq!((read.datum, read.end, read.peeked), (Value::TRUE, 5, false));
    let read = lily_engine_scheme::read(&mut heap, "  ; only\n", 0, None).unwrap();
    assert_eq!((read.datum, read.start, read.end), (Value::EOF, 9, 9));
}

#[test]
fn error_case_21_is_worded_as_lilypond_prints_it() {
    // packages/common/test/parser/errors/21-scheme-read.ly, and the line of
    // its .err that is Guile's.
    let ly = "\\version \"2.24.0\"\n#(define x (list 1 2)\n{ c4 }\n";
    let expected = "21-scheme-read.ly:4:1: unexpected end of input while searching for: )";
    let mut heap = Heap::new();
    let hash = ly.find('#').unwrap();
    let error = read(&mut heap, ly, hash + 1, None).unwrap_err();
    let at = Position::default().advance(&ly[..error.at.unwrap()]);
    assert_eq!(error.text(&heap, Some("21-scheme-read.ly"), at), expected);
}

#[test]
fn guile_counts_columns_as_a_port_does() {
    let at = |text: &str| {
        let p = Position::default().advance(text);
        (p.line, p.column)
    };
    assert_eq!(at("ab"), (0, 2));
    assert_eq!(at("a\tb"), (0, 9));
    assert_eq!(at("\t\t"), (0, 16));
    assert_eq!(at("abc\rd"), (0, 1));
    assert_eq!(at("ab\x08\x07"), (0, 1));
    assert_eq!(at("λλ\nx"), (1, 1));
}

#[test]
fn nesting_is_limited_on_wasm_s_stack() {
    // An unoptimised build's frames are about five times an optimised one's.
    let test = || {
        let mut heap = Heap::new();
        let deep = |open: &str, close: &str, n: usize| open.repeat(n) + &close.repeat(n);
        for (open, close) in [("(", ")"), ("[", "]"), ("#(", ")")] {
            let ok = read(&mut heap, &deep(open, close, MAX_DEPTH), 0, None);
            assert!(ok.is_ok(), "{open}: {ok:?}");
            let error = read(&mut heap, &deep(open, close, MAX_DEPTH + 1), 0, None).unwrap_err();
            assert_eq!(error.message(&heap), "data nested too deeply");
        }
        let quotes = "'".repeat(MAX_DEPTH + 1) + "a";
        assert!(read(&mut heap, &quotes, 0, None).is_err());
    };
    std::thread::Builder::new()
        .stack_size(16 << 20)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

/// Every datum of `file`, as `<end> <written>` lines, the way the guile
/// program of the next test prints them.
fn read_file(heap: &mut Heap, file: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(file).unwrap();
    let mut scan = LilyScan {
        file: Some(file.display().to_string()),
    };
    let mut lines = Vec::new();
    let mut offset = 0;
    loop {
        let read = match read(heap, &text, offset, Some(&mut scan)) {
            Ok(read) => read,
            Err(error) => {
                let at = Position::default().advance(&text[..error.at.unwrap_or(0)]);
                panic!(
                    "{}",
                    error.text(heap, Some(&file.display().to_string()), at)
                );
            }
        };
        if read.datum == Value::EOF {
            return lines;
        }
        let mut out = format!("{} ", read.end);
        heap.write(read.datum, &mut out);
        lines.push(out);
        offset = read.end;
    }
}

#[test]
fn reads_lilypond_s_scheme_as_guile_does() {
    let Some(scm) = lilypond_scm() else {
        eprintln!("skipped: lilypond 2.26.0 is not installed");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(scm.join("lily"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "scm"))
        .collect();
    files.sort();
    assert!(
        files.len() >= 90,
        "{} files in {}",
        files.len(),
        scm.display()
    );
    let program = r#"(for-each
 (lambda (file)
   (call-with-input-file file
     (lambda (port)
       (display "file ") (display file) (newline)
       (let lp ()
         (let ((d (read port)))
           (unless (eof-object? d)
             (display (ftell port)) (display " ") (write d) (newline)
             (lp)))))
     #:encoding "UTF-8"))
 (cdr (command-line)))
"#;
    let Some(out) = guile(Some(&scm), program, &files) else {
        eprintln!("guile is not installed: LilyPond's scm/ was not read by it");
        return;
    };
    let mut expected: Vec<(String, Vec<&str>)> = Vec::new();
    for line in out.lines() {
        match line.strip_prefix("file ") {
            Some(file) => expected.push((file.into(), Vec::new())),
            None => expected.last_mut().unwrap().1.push(line),
        }
    }
    assert_eq!(expected.len(), files.len());
    let mut heap = Heap::new();
    let mut datums = 0;
    for (file, (name, guile)) in files.iter().zip(&expected) {
        assert_eq!(&file.display().to_string(), name);
        let ours = read_file(&mut heap, file);
        assert_eq!(
            ours.len(),
            guile.len(),
            "{name}: top-level datums, the engine's and guile's"
        );
        for (ours, guile) in ours.iter().zip(guile) {
            assert_eq!(ours, guile, "{name}: a datum and its end");
        }
        datums += ours.len();
        heap.collect(&[]);
    }
    eprintln!(
        "{} files, {datums} top-level datums, as guile reads them",
        files.len()
    );
}
