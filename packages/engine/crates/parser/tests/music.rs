//! N2 part 3 (04 §6, 02 §5.5): the music the parser makes natively, with
//! the constructors of `ly-syntax-constructors.scm` and `parser.yy`'s
//! actions, compared node by node with LilyPond 2.26.0's music trees as
//! `tools/compare/music.ly` writes them for the book handler: types,
//! properties, values and origins. Inputs of every construct of P2–P5 are
//! compiled by the installed lilypond with `music.ly` as a settings file
//! and parsed by the engine, whose trees `tests/support`'s `music_trees`
//! writes in the same format. T14's tree is LilyPond's, as `music.ly`
//! wrote it (`tools/compare/test/music.test.mjs`). Every test skips itself,
//! and so verifies nothing, without that lilypond.

use lily_engine_parser::{NoteNames, Parse, parse};
use lily_engine_scheme::Vm;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

mod support;

/// The installed `scm/lily/`, or None (and a note on stderr) without lilypond.
fn lily() -> Option<&'static Path> {
    static LILY: OnceLock<Option<PathBuf>> = OnceLock::new();
    let lily = LILY.get_or_init(support::lily_scm);
    if lily.is_none() {
        eprintln!("skipped: no LilyPond 2.26.0 installed");
    }
    lily.as_deref()
}

fn names() -> NoteNames {
    let text = std::fs::read(lily().unwrap().join("define-note-names.scm")).unwrap();
    NoteNames::read(&text).expect("define-note-names.scm reads")
}

/// The engine's trees of `text`, parsed as `name`, and its stderr.
fn engine(vm: &mut Vm, name: &str, text: &str) -> (String, String) {
    let parse: Parse = parse(name, text.as_bytes().to_vec(), &names(), vm);
    let stderr = String::from_utf8_lossy(&parse.stderr()).into_owned();
    (
        support::music_trees(vm, &parse).expect("the trees are written"),
        stderr,
    )
}

/// T14 (`input/regression/music-function.ly`), and its tree as LilyPond
/// 2.26.0 gives it to the book handler.
const T14: &str = r#"\version "2.23.8"

\header
{
texidoc = "Music functions are generic music transformation functions,
which can be used to extend music syntax seamlessly.  Here we define
and use a @code{\\myBar} function which works like @code{\\bar}."
}

myBar = #(define-music-function (bar-type) (string?)
          (make-music 'BarEvent 'bar-type bar-type))

\layout { ragged-right = ##t }

{
    d4 \myBar ".|:" d4
}
"#;

const T14_TREE: &str = "score 1
SequentialMusic @music-function.ly:15:0-17:1
  elements:
    NoteEvent @music-function.ly:16:4-16:6
      duration = #<Duration 4 >
      length = #<Mom 1/4>
      pitch = #<Pitch d >
    BarEvent @music-function.ly:16:7-16:19
      bar-type = \".|:\"
      length = #<Mom 0>
    NoteEvent @music-function.ly:16:20-16:22
      duration = #<Duration 4 >
      length = #<Mom 1/4>
      pitch = #<Pitch d >
  length = #<Mom 1/2>
";

#[test]
fn t14s_tree_is_lilyponds() {
    let Some(lily) = lily() else { return };
    let mut vm = support::library(lily).expect("the library loads");
    let (trees, stderr) = engine(&mut vm, "music-function.ly", T14);
    assert_eq!(stderr, "");
    support::same(T14_TREE, &trees, "T14");
}

/// Inputs of P2–P5's music, one score each, that need no file of part 2:
/// no script abbreviation (`script-init.ly`), no context, no property.
const INPUTS: &[(&str, &str)] = &[
    (
        "notes",
        "{ c4 d8. e16 r4 s2 c'4*2/3 d,,2*3 c4\\rest e!4 f?4 g='4 }",
    ),
    ("bare", "{ c d' es }"),
    ("rhythms", "{ 4 8. c }"),
    ("mmrest", "{ R1 R1*3 R2^\"text\" R1-1 }"),
    ("chords", "{ <c e g>4 <c=' e! g?>2.-1 <d f>\\2 <c-1 e-3>8 }"),
    ("repeat", "{ <c e>4 q8 q-1 }"),
    (
        "posts",
        "{ c4-1 d^2 e_3 f\\1 g:16 a: b^\"up\" c _word d-\"x\" }",
    ),
    ("shorthands", "{ c4( d) e~ e | f[ g] a\\< b\\! c\\> d\\! }"),
    ("lists", "{ c4 << d4 e >> << { f4 } { g4 } >> {} }"),
    ("unattached", "{ << c4 >> -1 d4 }"),
    ("leading", "{ -1 c4 }"),
    (
        "identifiers",
        "x = { c4 d }\ny = ^1\nz = c'\n{ \\x e4 \\x f\\y \\z }",
    ),
    (
        "events",
        "pe = _1 ^\"t\"\nqe = \\pe\nre = ^2\n{ c4 \\pe d^\\qe e_\\re f-\\re }",
    ),
    (
        "functions",
        "fn = #(define-music-function (m) (ly:music?) m)\n\
         ev = #(define-event-function (d) (number?) (make-music 'FingeringEvent 'digit d))\n\
         { \\fn { c4 d } e^\\ev 3 f\\ev 4 \\fn g8 }",
    ),
    (
        "scheme",
        "{ c4 #(make-music 'NoteEvent 'duration (ly:make-duration 2) 'pitch (ly:make-pitch 0 1)) $(make-music 'RestEvent 'duration (ly:make-duration 3)) }",
    ),
    ("scores", "{ c4 }\n\\score { { d4 } }\n<< e4 >>"),
];

#[test]
fn trees_of_p2_to_p5_are_lilyponds() {
    let Some(lily) = lily() else { return };
    let mut vm = support::library(lily).expect("the library loads");
    for (name, input) in INPUTS {
        let text = format!("\\version \"2.26.0\"\n{input}\n");
        let file = format!("{name}.ly");
        let expected = support::lilypond_trees(&file, &text);
        let (got, _) = engine(&mut vm, &file, &text);
        support::same(&expected, &got, name);
    }
}
