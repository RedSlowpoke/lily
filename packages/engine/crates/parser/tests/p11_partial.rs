//! P11, part 2 (02 §6; MS2 step 34): functions and markup cut short by
//! `\etc` (`partial_function`, `partial_markup`), made by
//! `ly-syntax-constructors.scm`'s `partial-music-function`,
//! `partial-text-script` and `partial-markup`; in a file's module that uses
//! a `(lily)` loaded from the installed LilyPond 2.26.0 (`tests/support`).
//!
//! - [`INPUTS`]: the music their calls make, compiled by that lilypond with
//!   `tools/compare/music.ly` and compared node by node: music and event
//!   functions cut short at a mandatory or an optional parameter, chained
//!   through their last argument, `\override` and `\set` to `=`, `\repeat`
//!   after its kind or its count, directions alone and before a call or
//!   markup cut short, Scheme functions, markup commands defined by
//!   `\markup name = …`, markup cut short as an argument, templates cut
//!   short, and `syntax.ly`'s definitions with `\etc`.
//! - [`VALUES`]: the signatures of the functions made, and markup
//!   commands' markup, written by the same file in both.
//! - [`CASES`]: what LilyPond prints for `\etc` where none may stand, for
//!   markup that is no command, and for arguments that fail.
//!
//! The token sequences of `syntax.ly` and T26 are compared with LilyPond's
//! by `tools/compare/test/tokens.test.mjs`, and T26's tree by
//! `music.test.mjs`. Every test skips itself, and so verifies nothing,
//! without that lilypond.

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
        "chains",
        "hl = \\tweak font-size 2 \\tweak color #blue \\etc\n\
         md = \\key d \\etc\n\
         tq = \\transpose c d \\etc\n\
         { e' \\hl f' g'-\\hl -. \\md \\major \\tq { c'4 d' } }",
    ),
    (
        "repeats",
        "pc = \\repeat percent \\etc\n\
         tr = \\repeat tremolo 8 \\etc\n\
         un = \\repeat unfold 2 \\once \\override NoteHead.color = \\etc\n\
         { \\pc 2 { c'4 d' } \\tr { c'16 e' } \\un #red g'4 }",
    ),
    (
        "scripts",
        "up = ^\\markup \\italic \\with-color #blue \\etc\n\
         dn = _\\etc\n\
         tw = ^\\tweak color #blue \\etc\n\
         both = _\\tweak color #red -\\markup \\bold \\etc\n\
         { a'2\\up \"x\" \\dn \\markup \\bold \"y\" b'2\\tw -\"z\" \\both \"w\" }",
    ),
    (
        "properties",
        "fs = \\set Staff.fontSize = \\etc\n\
         sc = \\override Stem.color = \\etc\n\
         on = \\once \\override NoteHead.color = \\etc\n\
         st = \\set Staff.instrumentName = \\markup \\bold \\etc\n\
         { \\fs #-2 \\sc #red c'4 \\on #blue d'4 }",
    ),
    (
        "scheme",
        "sum = #(define-scheme-function (a b c) (number? number? number?) (+ a b c))\n\
         add = \\sum 1 2 \\etc\n\
         { \\repeat unfold \\add 0 { c'4 } }",
    ),
    (
        "markup",
        "\\markup bi = \\markup \\bold \\italic \\etc\n\
         \\markup cl = \\markup \\with-color \\etc\n\
         mk = #(define-music-function (f) (procedure?) #{ c'4^\\markup #(list f \"q\") #})\n\
         { c'4^\\markup \\bi \"x\" d'^\\markup \\cl #red \"y\" \\mk \\markup \\box \\etc }",
    ),
    (
        "templates",
        "mk = #(define-scheme-function () () #{ \\tweak color #red \\etc #})\n\
         hl = \\mk\n\
         rp = #(define-scheme-function () () #{ \\repeat unfold 2 \\etc #})\n\
         rr = \\rp\n\
         { \\hl c'4 \\rr { d'4 } $#{ \\tweak color #blue e'4 f' #} }",
    ),
    (
        "syntax",
        "color = #(define-music-function (col music) ((color? red) ly:music?)\n\
         \x20 #{ \\override NoteHead.color = #col $music \\revert NoteHead.color #})\n\
         colorBlue = \\color #blue \\etc\n\
         transposeUp = \\transpose c d \\etc\n\
         withAccent = \\override Accidental.color = \\etc\n\
         colorAny = \\color \\etc\n\
         \\relative { \\color { a4 } \\color #green { b4 } \\colorBlue { c4 } \\colorAny \\default { d4 }\n\
         \x20 \\withAccent #red cis4 \\transposeUp { c8 d } }",
    ),
];

#[test]
fn p11_partial_makes_lilyponds_music() {
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

/// Values of the functions and markup commands made, written one a line.
pub const VALUES: &str = r#"pc = \repeat percent \etc
tw = ^\tweak color #blue \etc
both = _\tweak color #red -\markup \bold \etc
add = #(define-scheme-function (a b c) (number? number? number?) (+ a b c))
half = \add 1 \etc
\markup bi = \markup \bold \italic \etc
\markup cl = \markup \with-color \etc
#(define (show x) (write x) (newline))
#(show (map (lambda (p) (if (pair? p) (list (procedure-name (car p)) (cdr p)) (procedure-name p)))
            (ly:music-function-signature pc)))
#(show (length (ly:music-function-signature tw)))
#(show (length (ly:music-function-signature both)))
#(show (length (ly:music-function-signature half)))
#(show (half 2 3))
#(show (map procedure-name (markup-command-signature bi-markup)))
#(show (map procedure-name (markup-command-signature cl-markup)))
#(show (markup->string (markup #:bi "text")))
#(show (markup? (markup #:cl red "c")))
"#;

/// What lilypond writes to stdout and stderr for `text` as `name`.
fn lilypond_output(name: &str, text: &str) -> (String, String) {
    let dir = std::env::temp_dir().join(format!("lily-p11p-{}-{name}", std::process::id()));
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
fn p11_partial_values_are_lilyponds() {
    let Some(lily) = lily() else { return };
    let mut vm = support::library(lily).expect("the library loads");
    let text = format!("{V}{VALUES}");
    let (stdout, stderr) = lilypond_output("v.ly", &text);
    let parse = parse("v.ly", text.into_bytes(), names(), &mut vm);
    assert_eq!(vm.take_output(), stdout);
    assert_eq!(String::from_utf8_lossy(&parse.stderr()), stderr);
    lily_engine_parser::end_session(&mut vm);
}

/// Each input after [`V`], and LilyPond's stderr for it with
/// `--parse-only` (D64).
pub const CASES: &[(&str, &str)] = &[
    (
        "xx = \\etc\n{ c4 }",
        "f.ly:2:6: error: syntax error, unexpected \\etc, expecting \\header\n\
         xx = \n\
         \x20    \\etc\n\
         fatal error: failed files: \"f.ly\"\n",
    ),
    (
        "{ c4 \\etc }",
        "f.ly:2:6: error: syntax error, unexpected \\etc\n\
         { c4 \n\
         \x20    \\etc }\n\
         f.ly:2:1: error: errors found, ignoring music expression\n\
         \n\
         { c4 \\etc }\n\
         fatal error: failed files: \"f.ly\"\n",
    ),
    (
        "\\markup foo = \\markup \\bold \"x\" \\etc",
        "f.ly:2:15: error: Not a markup function\n\
         \\markup foo = \n\
         \x20             \\markup \\bold \"x\" \\etc\n\
         f.ly:2:33: error: syntax error, unexpected \\etc\n\
         \\markup foo = \\markup \\bold \"x\" \n\
         \x20                               \\etc\n\
         fatal error: failed files: \"f.ly\"\n",
    ),
    (
        "hh = \\tweak color #red \\etc\n{ \\hh 3 }",
        "f.ly:3:7: error: wrong type for argument 1.  Expecting music, found 3\n\
         { \\hh \n\
         \x20     3 }\n\
         fatal error: failed files: \"f.ly\"\n",
    ),
    (
        "mm = \\markup \\with-color #3 \\etc\n\\markup \\mm x",
        "f.ly:2:26: error: wrong type for argument 1.  Expecting color, found 3\n\
         mm = \\markup \\with-color \n\
         \x20                        #3 \\etc\n\
         ERROR: In procedure ly:parse-file:\n\
         In procedure reverse!: Wrong type argument in position 1: (3 . #f)\n",
    ),
];

#[test]
fn p11_partial_errors_are_lilyponds() {
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

/// `\include` of a `\name` and of a `#` and a `$` form (P11, MS2 step 34),
/// each input after [`V`] with `inc.ly` beside it: none gives a message
/// but those whose name is no string, which leave the lexer in its `incl`
/// start condition, as LilyPond's.
pub const INCLUDES: &[&str] = &[
    "file = #\"inc.ly\"\n\\include \\file\n\\include #\"inc.ly\"\n\
     \\include $(string-append \"in\" \"c.ly\")\n{ \\incm }",
    "\\include \\nosuch",
    "\\include #5",
    "num = #5\n\\include \\num",
    "\\include \\nosuch\n{ c4 }\n\"inc.ly\"\n{ \\incm }",
];

#[test]
fn p11_include_names_are_lilyponds() {
    let Some(lily) = lily() else { return };
    let mut vm = support::library(lily).expect("the library loads");
    let dir = std::env::temp_dir().join(format!("lily-p11i-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let included = format!("{V}incm = {{ e4 }}\n");
    std::fs::write(dir.join("inc.ly"), &included).unwrap();
    for input in INCLUDES {
        let text = format!("{V}{input}\n");
        std::fs::write(dir.join("f.ly"), &text).unwrap();
        let run = Command::new("lilypond")
            .args(["--loglevel=WARNING", "-dno-print-pages", "f.ly"])
            .current_dir(&dir)
            .env("LANGUAGE", "en")
            .output()
            .expect("lilypond runs");
        let expected = String::from_utf8_lossy(&run.stderr).into_owned();
        let include = &mut |name: &str| match std::fs::read(dir.join(name)) {
            Ok(text) => lily_engine_scheme::Hosted::Text(text),
            Err(_) => lily_engine_scheme::Hosted::Missing,
        };
        let parse = lily_engine_parser::parse_including(
            "f.ly",
            text.into_bytes(),
            names(),
            &mut vm,
            include,
        );
        assert_eq!(
            String::from_utf8_lossy(&parse.stderr()),
            expected,
            "{input}"
        );
        lily_engine_parser::end_session(&mut vm);
    }
    std::fs::remove_dir_all(&dir).ok();
}
