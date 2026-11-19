//! P4 (02 §2.4, §5.4, §6): `#` and `$` forms read at their byte and run in
//! a file's module, which uses a `(lily)` that loaded the installed LilyPond
//! 2.26.0's `lily-library.scm` and `music-functions.scm`, with Bison's
//! timing. Each expected stderr was printed by LilyPond 2.26.0
//! (`lilypond --loglevel=WARNING`) for the same input, less the warnings of
//! processing, which a parser alone does not reach (D64); what the inputs
//! write with `display` comes where Guile flushes its error port. The error
//! corpus's cases 21–23 are scored with C0 by
//! `tools/compare/test/c0-cases.test.mjs`. Every test skips itself, and so
//! verifies nothing, without that lilypond.

use lily_engine_parser::{NoteNames, parse};
use lily_engine_scheme::Vm;
use std::sync::OnceLock;

mod support;

const V: &str = "\\version \"2.26.0\"\n";
/// Writes its argument to the error port.
const SAY: &str = "#(define (say x) (display x (current-error-port)))\n";

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

/// A VM whose `(lily)` loaded the two library files.
fn library() -> Option<Vm> {
    Some(support::library(lily()?).expect("the library loads"))
}

fn stderr(vm: &mut Vm, name: &str, text: &str) -> String {
    let parse = parse(
        name,
        format!("{V}{text}").into_bytes(),
        names().unwrap(),
        vm,
    );
    String::from_utf8_lossy(&parse.stderr()).into_owned()
}

#[test]
fn an_assignment_peeks_after_dollar_by_its_token_and_not_after_hash() {
    let Some(mut vm) = library() else { return };
    // Whether a `$` on the line after `x = value` sees `x` defined (§5.4's
    // table for P4's values): a `#` form's value is defined before the next
    // token is lexed; a `$` form's token peeks as a `\name`'s of its kind
    // does.
    let cases = [
        ("", "#1", true),
        ("", "$1", false),
        ("", "#\"s\"", true),
        ("", "$\"s\"", true),
        ("", "#'sym", true),
        ("", "$'sym", true),
        ("", "#'((a . 1))", true),
        ("", "$'((a . 1))", false),
        ("", "#(current-module)", true),
        ("", "$(current-module)", false),
        ("", "#car", true),
        ("", "$car", true),
        ("", "#'(1 2)", true),
        ("", "$'(1 2)", true),
        ("", "##t", true),
        ("", "$#t", true),
        ("", "#(if #f #f)", true),
        ("", "#3/4", true),
        ("", "$3/4", false),
        ("", "#1.5", true),
        ("", "$1.5", false),
        ("y = { c }\n", "$y", false),
        ("y = { c }\n", "#y", true),
        ("y = -.\n", "$y", false),
        ("y = c\n", "$y", false),
    ];
    for (before, value, defined) in cases {
        let text = format!("{SAY}{before}x = {value}\n$(say (if (defined? 'x) \"+\" \"-\"))\n");
        let out = stderr(&mut vm, "pk.ly", &text);
        assert_eq!(out, if defined { "+" } else { "-" }, "{value}");
    }
}

#[test]
fn scheme_parses_as_lilypond_parses_it() {
    let Some(mut vm) = library() else { return };
    // Each input after its `\version` line, and its stderr: at toplevel,
    // in music, in a header, in property paths and multipliers; several
    // values; errors of reading and running; `(*location*)`; the shorthands
    // of `declarations-init.ly`; `#{ … #}`'s extent.
    let cases = [
        (
            "#(define (say x) (display x (current-error-port)))\n#(say \"A\")\n$(say \"B\")\n",
            "AB",
        ),
        (
            "#(define (say x) (display x (current-error-port)))\nx = $(if #f #f)\n$(say (if (defined? 'x) \"+\" \"-\"))\n",
            "du.ly:4:34: error: syntax error, unexpected end of input, expecting \\header\n$(say (if (defined? 'x) \"+\" \"-\"))\n                                 \n-fatal error: failed files: \"du.ly\"\n",
        ),
        (
            "#(define (say x) (display x (current-error-port)))\n{ c #(say \"A\") $(say \"B\") d }\n",
            "AB",
        ),
        ("y = { c }\n{ #y }\n{ $y d }\n", ""),
        (
            "{ c #\"s\" }\n{ c $\"s\" }\n",
            "m03.ly:2:5: warning: Ignoring non-music expression\n{ c \n    #\"s\" }\nm03.ly:3:5: warning: Ignoring non-music expression\n{ c \n    $\"s\" }\n",
        ),
        ("{ #(values) c }\n{ $(values) c }\n", ""),
        (
            "{ #(values 1 2) c }\n",
            "m05.ly:2:3: warning: Ignoring non-music expression\n{ \n  #(values 1 2) c }\nm05.ly:2:3: warning: Ignoring non-music expression\n{ \n  #(values 1 2) c }\n",
        ),
        (
            "{ $(values 1 2) c }\n",
            "m06.ly:2:3: error: syntax error, unexpected NUMBER_IDENTIFIER\n{ \n  $(values 1 2) c }\nm06.ly:2:1: error: errors found, ignoring music expression\n\n{ $(values 1 2) c }\nfatal error: failed files: \"m06.ly\"\n",
        ),
        ("{ c $(if #f #f) d }\n", ""),
        ("y = -.\n{ c $y }\n{ c #y }\n", ""),
        ("#5\n#\"s\"\n#'x\n", ""),
        (
            "$5\n",
            "t02.ly:2:1: error: syntax error, unexpected NUMBER_IDENTIFIER\n\n$5\nfatal error: failed files: \"t02.ly\"\n",
        ),
        (
            "$\"s\"\n$'x\n",
            "t03.ly:3:1: error: bad expression type\n\n$'x\nfatal error: failed files: \"t03.ly\"\n",
        ),
        (
            "#(values 1 2)\n",
            "t04.ly:2:1: error: bad expression type\n\n#(values 1 2)\nfatal error: failed files: \"t04.ly\"\n",
        ),
        (
            "$(values 1 2)\n",
            "t05.ly:2:1: error: syntax error, unexpected NUMBER_IDENTIFIER\n\n$(values 1 2)\nfatal error: failed files: \"t05.ly\"\n",
        ),
        (
            "#@(list 1 2)\n#@(list \"a\" \"b\")\n",
            "t06.ly:2:1: error: bad expression type\n\n#@(list 1 2)\nfatal error: failed files: \"t06.ly\"\n",
        ),
        ("$@(list \"a\" \"b\")\n$@'()\n#@(list)\n", ""),
        (
            "$(values \"a\" 2 \"b\")\n",
            "t08.ly:2:1: error: syntax error, unexpected NUMBER_IDENTIFIER\n\n$(values \"a\" 2 \"b\")\nfatal error: failed files: \"t08.ly\"\n",
        ),
        (
            "#(define (say x) (display x (current-error-port)))\n\\header { #(say \"A\") $(say \"B\") title = \"t\" }\n",
            "AB",
        ),
        (
            "\\header { $\"s\" }\n",
            "e02.ly:2:11: error: bad expression type\n\\header { \n          $\"s\" }\nfatal error: failed files: \"e02.ly\"\n",
        ),
        ("\\header { $(current-module) #5 }\n", ""),
        (
            "#(define (say x) (display x (current-error-port)))\nx.#'y = 1\n#(say x)\n",
            "((y . 1))",
        ),
        (
            "#(define (say x) (display x (current-error-port)))\nx.#\"s\" = 1\nx.#5 = 2\nx.#'(a b) = 3\n#(say x)\n",
            "((a (b . 3)) (5 . 2) (s . 1))",
        ),
        (
            "x.#1.5 = 1\n",
            "k03.ly:2:3: error: not a key\nx.\n  #1.5 = 1\nfatal error: failed files: \"k03.ly\"\n",
        ),
        (
            "#(define (say x) (display x (current-error-port)))\nx.$'y = 1\ny = \\x.#'y\n#(say y)\n",
            "1",
        ),
        (
            "x = #'((a . 1))\ny = \\x.#\"a\"\n#(display y (current-error-port))\n",
            "1",
        ),
        ("x.#'() = 1\n", ""),
        (
            "{ c4*#3/4 c4*#'(2 . 3) c4*#-1 c4*#\"x\" }\n",
            "u01.ly:2:27: error: not a multiplier\n{ c4*#3/4 c4*#'(2 . 3) c4*\n                          #-1 c4*#\"x\" }\nu01.ly:2:34: error: not a multiplier\n{ c4*#3/4 c4*#'(2 . 3) c4*#-1 c4*\n                                 #\"x\" }\nfatal error: failed files: \"u01.ly\"\n",
        ),
        ("n = 3\n{ c4*\\n }\n", ""),
        (
            "{ c4*$2 c4*#1.5 c4*#0 }\n",
            "u03.ly:2:12: error: not a multiplier\n{ c4*$2 c4*\n           #1.5 c4*#0 }\nfatal error: failed files: \"u03.ly\"\n",
        ),
        (
            "#(car 1)\n",
            "x01.ly:2:2: error: Guile signaled an error for the expression beginning here\n#\n (car 1)\nIn procedure car: Wrong type (expecting pair): 1\nfatal error: failed files: \"x01.ly\"\n",
        ),
        (
            "x = #(car 1)\n",
            "x02.ly:2:6: error: Guile signaled an error for the expression beginning here\nx = #\n     (car 1)\nIn procedure car: Wrong type (expecting pair): 1\nfatal error: failed files: \"x02.ly\"\n",
        ),
        (
            "{ $(car 1) }\n",
            "x03.ly:2:4: error: Guile signaled an error for the expression beginning here\n{ $\n   (car 1) }\nIn procedure car: Wrong type (expecting pair): 1\nfatal error: failed files: \"x03.ly\"\n",
        ),
        (
            "#(let ((x)) x)\n",
            "x04.ly:2:2: error: Guile signaled an error for the expression beginning here\n#\n (let ((x)) x)\nSyntax error:\nx04.ly:2:0: let: bad let in form (let ((x)) x)\nfatal error: failed files: \"x04.ly\"\n",
        ),
        (
            "#(define (f) (car '()))\n#(f)\n",
            "x05.ly:3:2: error: Guile signaled an error for the expression beginning here\n#\n (f)\nIn procedure car: Wrong type argument in position 1 (expecting pair): ()\nfatal error: failed files: \"x05.ly\"\n",
        ),
        (
            "#(error \"boom\" 1)\n",
            "x06.ly:2:2: error: Guile signaled an error for the expression beginning here\n#\n (error \"boom\" 1)\nboom 1\nfatal error: failed files: \"x06.ly\"\n",
        ),
        (
            "#(throw 'foo 1 2)\n",
            "x07.ly:2:2: error: Guile signaled an error for the expression beginning here\n#\n (throw 'foo 1 2)\nThrow to key `foo' with args `(1 2)'.\nfatal error: failed files: \"x07.ly\"\n",
        ),
        (
            "#(undefined-thing)\n",
            "x08.ly:2:2: error: Guile signaled an error for the expression beginning here\n#\n (undefined-thing)\nUnbound variable: undefined-thing\nfatal error: failed files: \"x08.ly\"\n",
        ),
        (
            "$(no-such)\n{ c }\n",
            "x09.ly:2:2: error: Guile signaled an error for the expression beginning here\n$\n (no-such)\nUnbound variable: no-such\nfatal error: failed files: \"x09.ly\"\n",
        ),
        (
            "#(\n",
            "x10.ly:2:2: error: Guile signaled an error for the expression beginning here\n#\n (\nx10.ly:3:1: unexpected end of input while searching for: )\nx10.ly:2:2: error: syntax error, unexpected EVENT_IDENTIFIER\n#\n (\nfatal error: failed files: \"x10.ly\"\n",
        ),
        (
            "$(\n",
            "x11.ly:2:2: error: Guile signaled an error for the expression beginning here\n$\n (\nx11.ly:3:1: unexpected end of input while searching for: )\nx11.ly:2:2: error: syntax error, unexpected EVENT_IDENTIFIER\n$\n (\nfatal error: failed files: \"x11.ly\"\n",
        ),
        ("# ", "fatal error: failed files: \"x12.ly\"\n"),
        ("$ ", "fatal error: failed files: \"x13.ly\"\n"),
        (
            "#)\n{ c }\n",
            "x14.ly:2:2: error: Guile signaled an error for the expression beginning here\n#\n )\nx14.ly:2:2: unexpected \")\"\nx14.ly:2:2: error: syntax error, unexpected EVENT_IDENTIFIER\n#\n )\nfatal error: failed files: \"x14.ly\"\n",
        ),
        (
            "x = #{ c d #}\n",
            "x15.ly:2:6: error: Guile signaled an error for the expression beginning here\nx = #\n     { c d #}\nUnbound variable: #{\\x7b;}#\nx15.ly:2:8: error: syntax error, unexpected NOTENAME_PITCH\nx = #{ \n       c d #}\nx15.ly:2:13: error: Guile signaled an error for the expression beginning here\nx = #{ c d #\n            }\nUnbound variable: #{\\x7d;}#\nfatal error: failed files: \"x15.ly\"\n",
        ),
        (
            "#(define (say x) (display x (current-error-port)))\n#(begin (say \"A\") (car 1))\n",
            "x16.ly:3:2: error: Guile signaled an error for the expression beginning here\n#\n (begin (say \"A\") (car 1))\nAIn procedure car: Wrong type (expecting pair): 1\nfatal error: failed files: \"x16.ly\"\n",
        ),
        (
            "{ c #(car 1) d }\n",
            "x17.ly:2:6: error: Guile signaled an error for the expression beginning here\n{ c #\n     (car 1) d }\nIn procedure car: Wrong type (expecting pair): 1\nfatal error: failed files: \"x17.ly\"\n",
        ),
        (
            "#(define x)\n",
            "x18.ly:2:2: error: Guile signaled an error for the expression beginning here\n#\n (define x)\nSyntax error:\nx18.ly:2:0: source expression failed to match any pattern in form (define x)\nfatal error: failed files: \"x18.ly\"\n",
        ),
        ("#'#{ c d #}\n", ""),
        (
            "#(display (*location*) (current-error-port))\n",
            "#<location l01.ly:2:2>",
        ),
        (
            "  #(display (*location*) (current-error-port))\n",
            "#<location l02.ly:2:4>",
        ),
        ("{ c( d) }\n", ""),
        ("{ c~ c c c | d }\n", ""),
        ("{ c\\( d\\) e\\< f\\! }\n", ""),
        ("{ c8[ d] }\n<< { c } \\\\ { d } >>\n", ""),
        (
            "y = -. ->\n{ #y c }\n{ c #y }\n",
            "m09.ly:2:8: warning: Unattached ArticulationEvent\ny = -. \n       ->\nm09.ly:2:5: warning: Unattached ArticulationEvent\ny = \n    -. ->\n",
        ),
        (
            "y = -.\n{ #y c }\n{ c #y d }\n",
            "m10.ly:2:5: warning: Unattached ArticulationEvent\ny = \n    -.\n",
        ),
        (
            "y = -.\n{ $y c }\n",
            "m11.ly:2:5: warning: Unattached ArticulationEvent\ny = \n    -.\n",
        ),
        ("y = { c d }\nn = 1\n{ #y \\y $y }\n", ""),
        (
            "#(define (say x) (display x (current-error-port)))\n#(say \"A\\n\")\n#(car 1)\n",
            "o01.ly:4:2: error: Guile signaled an error for the expression beginning here\n#\n (car 1)\nA\nIn procedure car: Wrong type (expecting pair): 1\nfatal error: failed files: \"o01.ly\"\n",
        ),
        (
            "#(define (say x) (display x (current-error-port)))\n#(say \"A\")\n{ c \\foo }\n",
            "o02.ly:4:5: error: unknown command: `\\foo'\n{ c \n    \\foo }\no02.ly:4:5: error: string outside of text script or \\lyricmode\n{ c \n    \\foo }\nAfatal error: failed files: \"o02.ly\"\n",
        ),
        (
            "#(define (say x) (display x (current-error-port)))\n#(say \"A\\nB\")\n{ c \\foo }\n",
            "o03.ly:4:5: error: unknown command: `\\foo'\n{ c \n    \\foo }\no03.ly:4:5: error: string outside of text script or \\lyricmode\n{ c \n    \\foo }\nA\nBfatal error: failed files: \"o03.ly\"\n",
        ),
        (
            "x = ##{ c\n",
            "y03.ly:2:6: error: Guile signaled an error for the expression beginning here\nx = #\n     #{ c\nIn procedure char=?: Wrong type argument in position 1 (expecting character): #<eof>\ny03.ly:2:7: error: Guile signaled an error for the expression beginning here\nx = ##\n      { c\nUnbound variable: #{\\x7b;}#\ny03.ly:2:9: error: syntax error, unexpected NOTENAME_PITCH\nx = ##{ \n        c\nfatal error: failed files: \"y03.ly\"\n",
        ),
        (
            "x = #'(a #{ c $(+ 1 2) #} b)\n#(display (length x) (current-error-port))\n",
            "3",
        ),
        // A lookup's music is placed at the lookup, a `#` form's where it
        // was made.
        (
            "x.y = -.\nn = 1\n{ \\x.y c }\n",
            "v01.ly:4:3: warning: Unattached ArticulationEvent\n{ \n  \\x.y c }\n",
        ),
        (
            "x.y = -. ->\nn = 1\n{ \\x.y c }\n",
            "v02.ly:2:10: warning: Unattached ArticulationEvent\nx.y = -. \n         ->\nv02.ly:2:7: warning: Unattached ArticulationEvent\nx.y = \n      -. ->\n",
        ),
    ];
    for (i, (text, expected)) in cases.into_iter().enumerate() {
        let name = NAMES[i];
        assert_eq!(
            stderr(&mut vm, &format!("{name}.ly"), text),
            expected,
            "{name}"
        );
    }
}

/// The files of `scheme_parses_as_lilypond_parses_it`, by the names
/// LilyPond printed them under.
const NAMES: [&str; 66] = [
    "s01", "du", "m01", "m02", "m03", "m04", "m05", "m06", "m07", "m08", "t01", "t02", "t03",
    "t04", "t05", "t06", "t07", "t08", "e01", "e02", "e03", "k01", "k02", "k03", "k04", "k05",
    "k06", "u01", "u02", "u03", "x01", "x02", "x03", "x04", "x05", "x06", "x07", "x08", "x09",
    "x10", "x11", "x12", "x13", "x14", "x15", "x16", "x17", "x18", "x19", "l01", "l02", "w01",
    "w02", "w03", "w05", "m09", "m10", "m11", "m12", "o01", "o02", "o03", "y03", "y04", "v01",
    "v02",
];

#[test]
fn an_included_string_is_lexed_next_and_its_end_gives_no_token() {
    let Some(mut vm) = library() else { return };
    // `$` includes before the token after it is lexed, `#` when its token is
    // taken; the included music is a whole toplevel item, and `e4` after it
    // a note where none can start one. A string is all it takes.
    let text = "$(ly:parser-include-string \"{ c4 d }\") e4\n#(ly:parser-include-string 3)\n";
    let parse = lily_engine_parser::parse_tokens(
        "inc.ly",
        format!("{V}{text}").into_bytes(),
        names().unwrap(),
        &mut vm,
    );
    assert_eq!(
        String::from_utf8_lossy(&parse.stderr()),
        "inc.ly:2:40: error: syntax error, unexpected NOTENAME_PITCH
$(ly:parser-include-string \"{ c4 d }\") 
                                       e4
inc.ly:3:2: error: Guile signaled an error for the expression beginning here
#
 (ly:parser-include-string 3)
In procedure ly:parser-include-string: Wrong type argument in position 1 (expecting string): 3
fatal error: failed files: \"inc.ly\"
"
    );
    // LilyPond's -ddebug-parser reads these.
    assert_eq!(
        parse.tokens.unwrap().join(" "),
        "'{' NOTENAME_PITCH UNSIGNED NOTENAME_PITCH '}' NOTENAME_PITCH UNSIGNED SCM_TOKEN end of input"
    );
    assert_eq!(parse.sources.get(1).name(), "<included string>");
}
