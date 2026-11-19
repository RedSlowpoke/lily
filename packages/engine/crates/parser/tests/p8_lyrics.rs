//! P8 (02 §6): lyrics. `\lyricmode` and `\lyrics` (with `\with`) and the
//! music of their lists, as notes mode's (`music_list`); syllables
//! (`lyric_element_music`) as words, strings, `\markup` and Scheme markup,
//! with durations, dots and multipliers, and `--` and `__` after them; `_`
//! as a syllable of a space (`lyric_fudge`), words of non-ASCII letters,
//! and digits and dots as the lexer's lyric rules split them; `\addlyrics`
//! after music, one or several, with `\with` and as a variable, after
//! `\new`, `\context` and a function's music (`new_lyrics`, `add-lyrics`);
//! `\lyricsto`'s forms (`lyric_mode_music`); and a syllable as a function's
//! argument (`LYRIC_ELEMENT`); in a file's module that uses a `(lily)`
//! loaded from the installed LilyPond 2.26.0 (`tests/support`).
//!
//! - [`INPUTS`]: the music each makes, compiled by that lilypond with
//!   `tools/compare/music.ly` and compared node by node.
//! - [`CASES`]: what LilyPond prints for syntax errors in and after lyrics,
//!   a Scheme value that is no syllable, and syllables outside lyrics mode,
//!   each expected stderr printed by LilyPond 2.26.0 (`lilypond
//!   --loglevel=WARNING`) for the same input.
//!
//! The token sequences of T20 and of these inputs are compared with
//! LilyPond's by `tools/compare/test/tokens.test.mjs`, and the trees of
//! T20, T02, T04 and T30 by `music.test.mjs`. Every test skips itself, and
//! so verifies nothing, without that lilypond.

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

/// Inputs that LilyPond parses without a message.
pub const INPUTS: &[(&str, &str)] = &[
    (
        "syllables",
        "\\lyricmode { la4 -- li8. __ lu2*2/3 lo \"quoted word\" \"d'un\"8 \\markup \\bold bold _ \
         a_b c1. Hello, world. 1st x.y end.4 à vo -- cê Жълтата いろはにほへど }",
    ),
    (
        "heads",
        "\\lyrics { a b } \\lyrics \\with { \\override LyricText.color = #red } { c } \
         \\new Lyrics \\lyricmode { d } \\lyricmode \\sequential { e } \\lyricmode << f g >> \
         \\lyricmode \\simultaneous { h i }",
    ),
    (
        "addlyrics",
        "words = \\lyricmode { q r }\nmel = { c4 d }\n{ c4 d } \\addlyrics { a b }\n\
         \\new Voice { c4 } \\addlyrics { x }\n\
         \\new Voice = \"v\" { c4 } \\addlyrics { x } \\addlyrics \\with { \\override LyricText.color = #red } { y }\n\
         \\new Staff \\new Voice { c4 } \\addlyrics { z }\n\\context Voice { c4 } \\addlyrics { w }\n\
         \\relative c' { c4 d } \\addlyrics { a b } \\addlyrics \\words\n\\mel \\addlyrics \\words\n\
         \\new Voice c4 \\addlyrics << { x y } >>\n\
         \\new Staff { \\new Voice = \"\" { c4 } \\addlyrics \\sequential { m } }",
    ),
    (
        "lyricsto",
        "words = \\lyricmode { q r }\n<< \\new Voice = \"v\" { c4 d } \\new Lyrics \\lyricsto \"v\" { a b } \
         \\new Lyrics \\lyricsto \"v\" \\words\n\\new Lyrics \\lyricsto v << { a b } >> \
         \\new Lyrics \\lyricsto #\"v\" \\sequential { c } >>",
    ),
    (
        "music",
        "\\lyricmode { \\set stanza = \"1.\" a4 \\skip 4 b \\override LyricText.color = #red c \
         \\once \\override LyricText.font-size = #2 d { f g } << h i >> #\"scm\" \
         #(markup #:bold \"m\") 8 j -- \\markup { k l } \\\"skip\" 4 m }",
    ),
    (
        "arguments",
        "\\lyricmode { \\tag #'x e4 -- \\tag #'y \\markup \\italic m2 \\tag #'z \"q\" \
         \\tag #'w word_with_under 8. __ }",
    ),
];

#[test]
fn p8_makes_lilyponds_music() {
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

/// Each input after [`V`], and LilyPond's stderr for it:
/// - syntax errors: `\addlyrics`, `\lyricsto`, `\lyricmode` and `\lyrics`
///   without their music, which leave lyrics mode pushed, as the end of
///   input does after `\addlyrics`; a `REAL`, which lyrics mode lexes;
///   `\addlyrics` in a list;
/// - a Scheme value in lyrics that is no markup, ignored;
/// - a string and markup outside lyrics mode.
pub const CASES: &[(&str, &str)] = &[
    (
        r##"{ c4 } \addlyrics x"##,
        r##"f.ly:2:19: error: syntax error, unexpected SYMBOL, expecting \sequential or \simultaneous or << or '{'
{ c4 } \addlyrics 
                  x
f.ly:2:20: error: Unfinished main input
{ c4 } \addlyrics x
                   
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\lyricsto "v" x"##,
        r##"f.ly:2:15: error: syntax error, unexpected SYMBOL, expecting \sequential or \simultaneous or << or '{'
\lyricsto "v" 
              x
f.ly:2:16: error: Unfinished main input
\lyricsto "v" x
               
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\lyricmode x"##,
        r##"f.ly:2:12: error: syntax error, unexpected SYMBOL, expecting \sequential or \simultaneous or << or '{'
\lyricmode 
           x
f.ly:2:13: error: Unfinished main input
\lyricmode x
            
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\lyrics \with { } x"##,
        r##"f.ly:2:19: error: syntax error, unexpected SYMBOL, expecting \sequential or \simultaneous or << or '{'
\lyrics \with { } 
                  x
f.ly:2:20: error: Unfinished main input
\lyrics \with { } x
                   
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\lyricmode { a 1.5 b }"##,
        r##"f.ly:2:16: error: syntax error, unexpected REAL
\lyricmode { a 
               1.5 b }
f.ly:2:12: error: errors found, ignoring music expression
\lyricmode 
           { a 1.5 b }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\lyricmode { a #'sym b }"##,
        r##"f.ly:2:16: warning: Ignoring non-music expression
\lyricmode { a 
               #'sym b }
"##,
    ),
    (
        r##"{ c4 "s" d \markup x e }"##,
        r##"f.ly:2:8: error: string outside of text script or \lyricmode
{ c4 "s
       " d \markup x e }
f.ly:2:12: error: markup outside of text script or \lyricmode
{ c4 "s" d 
           \markup x e }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\lyricmode { a4 b8 \addlyrics { c } }"##,
        r##"f.ly:2:20: error: syntax error, unexpected \addlyrics
\lyricmode { a4 b8 
                   \addlyrics { c } }
f.ly:2:12: error: errors found, ignoring music expression
\lyricmode 
           { a4 b8 \addlyrics { c } }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\new Voice { c4 } \addlyrics { a } \addlyrics"##,
        r##"f.ly:2:46: error: syntax error, unexpected end of input, expecting \sequential or \simultaneous or << or '{'
\new Voice { c4 } \addlyrics { a } \addlyrics
                                             
fatal error: failed files: "f.ly"
"##,
    ),
];

#[test]
fn p8_errors_are_lilyponds() {
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

/// `add-lyrics` names each voice it makes or finds unnamed by
/// `get-next-unique-voice-name`, a session's count, in the order of the
/// file: an `\addlyrics` in a function's music is named before one that
/// ends after it.
#[test]
fn p8_names_voices_in_the_files_order() {
    let Some(lily) = lily() else { return };
    let mut vm = support::library(lily).expect("the library loads");
    let input = "{ \\new Voice { c4 } \\addlyrics { a } \\relative { c'4 } \\addlyrics { b } }\n\
                 { c4 } \\addlyrics { d }";
    let text = format!("{V}{input}\n");
    let expected = support::lilypond_trees("order.ly", &text);
    let parse = parse("order.ly", text.into_bytes(), names(), &mut vm);
    assert_eq!(String::from_utf8_lossy(&parse.stderr()), "");
    let got = support::music_trees(&mut vm, &parse).expect("the trees are written");
    for n in 0..3 {
        assert!(
            got.contains(&format!("\"uniqueContext{n}\"")),
            "uniqueContext{n}"
        );
    }
    support::same(&expected, &got, "order");
    lily_engine_parser::end_session(&mut vm);
}
