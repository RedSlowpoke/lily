//! P3 (02 §2.3, §6): `\name` and shorthands classified by their values in
//! a file's module, which uses a `(lily)` that loaded the installed LilyPond
//! 2.26.0's `lily-library.scm` and `music-functions.scm`; assignments, paths
//! and lookups defined into and read from it. Each expected message and
//! value was printed by LilyPond 2.26.0 (`lilypond --loglevel=WARNING`,
//! values with `#(write …)`) for the same input, less the warnings of
//! processing (`skipping zero-duration score` and its hint), which a parser
//! alone does not reach (D64). The error corpus's cases 01 and 02 are scored
//! with C0 by `tools/compare/test/c0-cases.test.mjs`. Every test skips
//! itself, and so verifies nothing, without that lilypond.

use lily_engine_parser::{Env, Id, Lexer, Mode, NoteNames, Parse, Source, Tok, parse};
use lily_engine_scheme::Vm;
use std::sync::OnceLock;

mod support;

const V: &str = "\\version \"2.26.0\"\n";

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

fn run(vm: &mut Vm, name: &str, text: &str) -> Parse {
    parse(
        name,
        format!("{V}{text}").into_bytes(),
        names().unwrap(),
        vm,
    )
}

fn stderr(vm: &mut Vm, name: &str, text: &str) -> String {
    String::from_utf8_lossy(&run(vm, name, text).stderr()).into_owned()
}

/// What `#(write name)` prints after the file's assignments.
fn written(vm: &mut Vm, parse: &Parse, name: &str) -> String {
    let value = vm.module_value(parse.module, name).expect("defined");
    let mut out = String::new();
    vm.heap().write(value, &mut out);
    out
}

#[test]
fn names_are_classified_by_their_values_in_lily() {
    let Some(mut vm) = library() else { return };
    // A music function is the music crate's, by the predicate of its result
    // (step 18a).
    vm.eval_text(
        "(define-public mfun (define-music-function (m) (ly:music?) m))
         (define-public ef (define-event-function () () #f))
         (define-public sfun (define-scheme-function (x) (number?) x))
         (define-public al '((a . 1)))
         (define-public st \"s\")
         (define-public md (current-module))",
        None,
    )
    .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    let text = "\\UP \\car \\mfun \\ef \\sfun \\al \\st \\md \\nothing \\header &";
    let source = Source::new("c.ly", text.as_bytes().to_vec());
    let mut lexer = Lexer::new(&source, 0);
    let mut env = Env::new(&mut vm);
    let mut kinds = Vec::new();
    loop {
        let token = lexer.next_in(Mode::Notes, &mut env);
        if token.kind == Tok::Eof {
            break;
        }
        kinds.push(token.kind);
    }
    assert_eq!(
        kinds,
        [
            Tok::Identifier(Id::Number),
            Tok::Identifier(Id::Scm),
            Tok::Identifier(Id::MusicFunction),
            Tok::Identifier(Id::EventFunction),
            Tok::Identifier(Id::ScmFunction),
            Tok::Identifier(Id::Lookup),
            Tok::Identifier(Id::Scm),
            Tok::Identifier(Id::Module),
            Tok::String,
            Tok::Keyword(lily_engine_parser::lexer::keyword("header")),
            Tok::String,
        ]
    );
    // The two that are not bound, and no more.
    let messages: Vec<_> = lexer
        .diags
        .list()
        .iter()
        .map(|d| d.message.clone())
        .collect();
    assert_eq!(
        messages,
        [
            "unknown command: `\\nothing'",
            "undefined character or shorthand: &"
        ]
    );
}

#[test]
fn an_assignment_reads_the_token_after_its_value_before_it_defines() {
    let Some(mut vm) = library() else { return };
    // Whether `\y` on the line after the assignment is an unknown command:
    // lexed before `y` is defined (§5.4's table for P3's values).
    let cases = [
        ("y = \"s\"", true),
        ("y = \\UP", true),
        ("y = foo.bar", true),
        ("y = 3/4", false),
        ("y = 1.5", true),
        ("y = -.", true),
        ("y = foo", true),
        ("y = -1", true),
        ("y = \\car", false),
        ("y = c'", true),
        ("y = { c }", true),
        ("y = -. ->", true),
        ("x.k = 3\ny = \\x.k", true),
        ("y.k = 3", true),
        ("x = { c }\n\"y\" = \\x", true),
        ("x.k = 3\ny = \\x", true),
    ];
    for (value, peeks) in cases {
        let out = stderr(&mut vm, "pk.ly", &format!("{value}\n\\y\n"));
        assert_eq!(
            out.contains("unknown command: `\\y'"),
            peeks,
            "{value}\n{out}"
        );
    }
}

#[test]
fn assignments_define_into_the_files_module() {
    let Some(mut vm) = library() else { return };
    let cases = [
        ("x = 3/4", "x", "(3 . 4)"),
        ("x = 1.5", "x", "1.5"),
        ("x = -1", "x", "-1"),
        ("x = foo", "x", "\"foo\""),
        ("x = \\foo", "x", "\"foo\""),
        ("x = \\UP", "x", "1"),
        ("x = 3\ny = \\x", "y", "3"),
        ("x.y.z = 3\nx.y.w = 4", "x", "((y (w . 4) (z . 3)))"),
        ("x = 3\nx.y = 4", "x", "((y . 4) . 3)"),
        ("x.y,z.3 = 1", "x", "((y (z (3 . 1))))"),
        ("x.3 = 3", "x", "((3 . 3))"),
        ("\"x\".y = 3", "x", "((y . 3))"),
        (
            "x.\"a b\" = 3\nx.y.3 = 1",
            "x",
            "((y (3 . 1)) (#{a b}# . 3))",
        ),
        ("x = foo.bar", "x", "(foo bar)"),
        ("y = foo,bar.3", "y", "(foo bar 3)"),
        ("x = \"foo\".bar", "x", "(foo bar)"),
        ("x.y = 3\nz = \\x.zz", "z", "()"),
        ("x.3 = 3\ny = \\x.3", "y", "3"),
        ("\"x.y\" = 3\nx = \\\"x.y\"", "x", "3"),
    ];
    for (text, name, value) in cases {
        let parse = run(&mut vm, "v.ly", text);
        // `\foo` is an unknown command, whose name is the value.
        assert_eq!(parse.failed(), text.contains("\\foo"), "{text}");
        assert_eq!(written(&mut vm, &parse, name), value, "{text}");
    }
    // Music, post-events and pitches are defined as music, frozen, and as
    // pitches (N2): Scheme reaches them through `#x`.
    let text = "music = { c }\nevents = -. ->\npitch = c'\ncopy = \\pitch\nevent = -.\n";
    let parse = run(&mut vm, "h.ly", text);
    assert!(!parse.failed());
    let value = |vm: &mut Vm, name: &str| vm.module_value(parse.module, name).unwrap();
    let music = |vm: &mut Vm, name: &str| {
        let v = value(vm, name);
        assert!(lily_engine_music::is_frozen(vm.heap(), v), "{name}");
        lily_engine_music::music_name(vm.heap(), v).map(String::from)
    };
    assert_eq!(music(&mut vm, "music").as_deref(), Some("SequentialMusic"));
    assert_eq!(music(&mut vm, "events").as_deref(), Some("PostEvents"));
    assert_eq!(
        music(&mut vm, "event").as_deref(),
        Some("ArticulationEvent")
    );
    let pitch = |vm: &mut Vm, name: &str| {
        let v = value(vm, name);
        vm.heap()
            .foreign::<lily_engine_music::Pitch>(v)
            .map(|p| (p.octave, p.notename))
    };
    assert_eq!(pitch(&mut vm, "pitch"), Some((0, 0)));
    assert_eq!(pitch(&mut vm, "pitch"), pitch(&mut vm, "copy"));
    // Each file has a module of its own.
    let other = run(&mut vm, "o.ly", "n = 1\n");
    assert_eq!(vm.module_value(other.module, "music"), None);
}

#[test]
fn identifiers_parse_as_lilypond_parses_them() {
    let Some(mut vm) = library() else { return };
    // `a.ly`'s input after its `\version` line, and its stderr.
    let cases = [
        (
            "x = { c d }\n\\x\n",
            "a01.ly:3:1: error: unknown command: `\\x'\n\n\\x\na01.ly:3:3: error: syntax error, unexpected end of input, expecting '.' or '='\n\\x\n  \nfatal error: failed files: \"a01.ly\"\n",
        ),
        (
            "x = \"str\"\n{ \\x }\n",
            "a04.ly:3:3: warning: Ignoring non-music expression\n{ \n  \\x }\n",
        ),
        (
            "x = 3\n{ c\\x }\n",
            "a06.ly:3:4: error: syntax error, unexpected NUMBER_IDENTIFIER\n{ c\n   \\x }\na06.ly:3:1: error: errors found, ignoring music expression\n\n{ c\\x }\nfatal error: failed files: \"a06.ly\"\n",
        ),
        (
            "x.y = 3\nz = \\x.q\n",
            "a08.ly:3:8: error: syntax error, unexpected CHORD_REPETITION\nz = \\x.\n       q\nfatal error: failed files: \"a08.ly\"\n",
        ),
        (
            "header = 3\n",
            "warning: identifier name is a keyword: `header'\n",
        ),
        ("\"~\" = { c }\n{ c~ }\n", ""),
        ("x = -.\n{ c\\x }\n", ""),
        ("x = c'\n{ \\x 4 \\x, }\n", ""),
        (
            "x = \\foo\n",
            "a14.ly:2:5: error: unknown command: `\\foo'\nx = \n    \\foo\nfatal error: failed files: \"a14.ly\"\n",
        ),
        (
            "\\UP\n",
            "a16.ly:2:1: error: syntax error, unexpected NUMBER_IDENTIFIER\n\n\\UP\nfatal error: failed files: \"a16.ly\"\n",
        ),
        (
            "\"x y\" = { c }\n\\\"x y\"\n",
            "a20.ly:3:6: error: unknown command: `\\x y'\n\\\"x y\n     \"\na20.ly:3:7: error: syntax error, unexpected end of input, expecting '.' or '='\n\\\"x y\"\n      \nfatal error: failed files: \"a20.ly\"\n",
        ),
        (
            "{ c \\UP }\n",
            "a22.ly:2:5: error: syntax error, unexpected NUMBER_IDENTIFIER\n{ c \n    \\UP }\na22.ly:2:1: error: errors found, ignoring music expression\n\n{ c \\UP }\nfatal error: failed files: \"a22.ly\"\n",
        ),
        (
            "{ c \\x.y }\n",
            "a28.ly:2:5: error: unknown command: `\\x'\n{ c \n    \\x.y }\na28.ly:2:5: error: string outside of text script or \\lyricmode\n{ c \n    \\x.y }\na28.ly:2:7: error: syntax error, unexpected '.'\n{ c \\x\n      .y }\na28.ly:2:8: error: not a note name: y\n{ c \\x.\n       y }\na28.ly:2:1: error: errors found, ignoring music expression\n\n{ c \\x.y }\nfatal error: failed files: \"a28.ly\"\n",
        ),
        ("x = \"s\"\nn = 1\n\\x\n", ""),
        (
            "x.k = 3\nn = 1\n\\x.k\n",
            "b04.ly:4:1: error: bad expression type\n\n\\x.k\nfatal error: failed files: \"b04.ly\"\n",
        ),
        ("x = { c }\nn = 1\n\\x\n\\score { \\x }\n", ""),
        (
            "x = -.\nn = 1\n{ \\x c }\n{ c \\x }\n{ c-\\x }\n{ c^\\x d_\\x }\n",
            "b06.ly:4:3: warning: Unattached ArticulationEvent\n{ \n  \\x c }\n",
        ),
        ("\"&\" = -.\nn = 1\n{ c& c-& }\n", ""),
        ("x = c'\nn = 1\n{ \\x \\x' \\x! \\x4 }\n", ""),
        (
            "x.3 = 3\ny = \\x.3\nz = \\x.c\n",
            "b13.ly:4:8: error: syntax error, unexpected NOTENAME_PITCH\nz = \\x.\n       c\nfatal error: failed files: \"b13.ly\"\n",
        ),
        (
            "\\car\n{ \\car }\n",
            "b15.ly:2:1: error: bad expression type\n\n\\car\nb15.ly:3:3: warning: Ignoring non-music expression\n{ \n  \\car }\nfatal error: failed files: \"b15.ly\"\n",
        ),
        (
            "x = -. ->\nn = 1\n{ c\\x }\n{ \\x }\n",
            "b16.ly:2:8: warning: Unattached ArticulationEvent\nx = -. \n       ->\nb16.ly:2:5: warning: Unattached ArticulationEvent\nx = \n    -. ->\n",
        ),
        (
            "x = { c }\n\\x = 3\n",
            "b17.ly:3:1: error: unknown command: `\\x'\n\n\\x = 3\nfatal error: failed files: \"b17.ly\"\n",
        ),
        (
            "x = \"s\"\ny = \\x\nz = \\x.y\n",
            "b20.ly:4:7: error: syntax error, unexpected '.'\nz = \\x\n      .y\nfatal error: failed files: \"b20.ly\"\n",
        ),
        (
            "x.y = { c }\nn = 2\n{ \\x.y }\n\\x.y\n",
            "p17.ly:5:1: error: bad expression type\n\n\\x.y\nfatal error: failed files: \"p17.ly\"\n",
        ),
        (
            "x = { c }\nn = 2\n\\score { \\x }\n{ c \\x }\n<< \\x >>\n",
            "",
        ),
        (
            "x = { c }\nn = 2\ny = \\x.k\n",
            "p19.ly:4:7: error: syntax error, unexpected '.'\ny = \\x\n      .k\nfatal error: failed files: \"p19.ly\"\n",
        ),
        ("x = c\nn = 2\n{ <\\x e>4 }\n", ""),
        (
            "x = -.\nn = 1\n{ \\x }\n{ c-\\x d-\\x-> }\ny = \\x \\x\n",
            "q04.ly:4:3: warning: Unattached ArticulationEvent\n{ \n  \\x }\n",
        ),
        (
            "x = c'\ny = x\nn = 1\n{ \\x' }\n\\x\n",
            "q06.ly:6:1: error: syntax error, unexpected PITCH_IDENTIFIER\n\n\\x\nfatal error: failed files: \"q06.ly\"\n",
        ),
        (
            "\"header\" = 1\nx = 1\n\"header\".k = 2\n",
            "warning: identifier name is a keyword: `header'\nwarning: identifier name is a keyword: `header'\n",
        ),
        (
            "x.y }\n",
            "r01.ly:2:5: error: syntax error, unexpected '}', expecting '='\nx.y \n    }\nfatal error: failed files: \"r01.ly\"\n",
        ),
        (
            "x. }\n",
            "r02.ly:2:4: error: syntax error, unexpected '}'\nx. \n   }\nfatal error: failed files: \"r02.ly\"\n",
        ),
        (
            "x.y = }\n",
            "r03.ly:2:7: error: syntax error, unexpected '}', expecting \\header\nx.y = \n      }\nfatal error: failed files: \"r03.ly\"\n",
        ),
        (
            "x = foo.\n",
            "r06.ly:2:9: error: syntax error, unexpected end of input\nx = foo.\n        \nfatal error: failed files: \"r06.ly\"\n",
        ),
        (
            "x = 3 }\n",
            "r07.ly:2:7: error: syntax error, unexpected '}'\nx = 3 \n      }\nfatal error: failed files: \"r07.ly\"\n",
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

/// The files of `identifiers_parse_as_lilypond_parses_them`, by the names
/// LilyPond printed them under.
const NAMES: [&str; 36] = [
    "a01", "a04", "a06", "a08", "a10", "a11", "a12", "a13", "a14", "a16", "a20", "a22", "a28",
    "b03", "b04", "b05", "b06", "b07", "b10", "b13", "b15", "b16", "b17", "b20", "p17", "p18",
    "p19", "p20", "q04", "q06", "q08", "r01", "r02", "r03", "r06", "r07",
];
