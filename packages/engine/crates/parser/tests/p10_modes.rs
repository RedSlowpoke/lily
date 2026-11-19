//! P10 (02 §6): chord mode (`\chordmode`, `\chords`), with durations,
//! modifiers, steps with `+` and `-`, `^`, `/` and `/+`, chords in `< >`,
//! post-events and a bare pitch, its chords made by the `chords` part's
//! `construct-chord-elements` (D74, D82); drum mode (`\drummode`,
//! `\drums`), with drum notes, chords of them and their post-events; and
//! figure mode (`\figuremode`, `\figures`), with numbers, strings, words,
//! markup and Scheme, `_`, brackets, alterations and modifications; the
//! heads with `\with`, in contexts, as values and under `\relative`; in a
//! file's module that uses a `(lily)` loaded from the installed LilyPond
//! 2.26.0 (`tests/support`).
//!
//! - [`INPUTS`]: the music each makes, compiled by that lilypond with
//!   `tools/compare/music.ly` and compared node by node.
//! - [`CASES`]: what LilyPond prints for P10's actions' errors and warnings
//!   and for syntax errors in its modes, each expected stderr printed by
//!   LilyPond 2.26.0 (`lilypond --loglevel=WARNING`) for the same input.
//!
//! The token sequences of P10's scores (T05, T06, T21) and of these inputs
//! are compared with LilyPond's by `tools/compare/test/tokens.test.mjs`.
//! Every test skips itself, and so verifies nothing, without that lilypond.

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

/// Inputs that LilyPond compiles without a message.
pub const INPUTS: &[(&str, &str)] = &[
    (
        "chords",
        "\\chordmode { c1 c:7 c:m7 c:maj7.9+ c:dim7 c:aug c:sus2 c:sus4.9 c:13^11 c:9^3.5 \
         c/e c/+d c:7/g c4.:m d e:7-.9+ f' g,8 c:5.3- c:1.5 c:m/+bes, }",
    ),
    (
        "chord-bodies",
        "\\chordmode { <c e g>4 <c' e>8. c_\"t\" d:m_\"u\" e2_\\markup \\bold x f q r s R1 }",
    ),
    (
        "heads",
        "\\chords { c1 g:7 }\n\\new Staff \\chordmode { c d }\n\
         \\chords \\with { \\override ChordName.color = #red } { c2 }\n\
         \\relative c' \\chordmode { c e } \\transpose c d \\chordmode { c:m }\n\
         \\chordmode \\sequential { c } \\chordmode << c e >>",
    ),
    (
        "drums",
        "\\drummode { bd4 sn8 hh hh4-> <bd sn>4 <bd-> hh>8-. r4 s4 bd4.:16 tomh tommh8 }\n\
         \\drums { bd4 sn } \\new DrumStaff \\drummode { cymc1 }\n\
         \\drums \\with { \\consists \"Pitch_squash_engraver\" } { bd4 }",
    ),
    (
        "figures",
        "\\figuremode { <6>4 <6 4>8 <_+> <5+ [3-]> <\"a\" \\markup \\bold b 7\\+ 6\\! 5/ 4\\\\> \
         <[3 5]> <3 _ 5> <#7 #\"x\"> s4 <6>4^\"t\" <6+! 5--> <6[+]> <3 [5 7]> <_!> }\n\
         \\figures { <6>2 } \\new FiguredBass \\figuremode { <5>1 }",
    ),
    (
        "values",
        "ch = \\chordmode { c:m }\nfig = \\figuremode { <6> }\ndr = \\drummode { bd }\n\
         { \\ch \\fig \\dr }\n\\chordmode { \\notemode { c4 } d \\ch }",
    ),
];

#[test]
fn p10_makes_lilyponds_music() {
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
/// - the actions': `bass number expected`, after a Scheme error too,
///   `Dropping surplus alteration symbols`, a word in chord and drum mode
///   that names nothing (`not a note name`), and
///   `construct-chord-elements`' `Spurious garbage`, at the token after
///   the chord;
/// - syntax errors: a separator without its pitch, a head without music,
///   with and without `\with`, which leaves its mode pushed.
pub const CASES: &[(&str, &str)] = &[
    (
        r##"\figuremode { <#(- 1) 5> }"##,
        r##"f.ly:2:16: error: bass number expected
\figuremode { <
               #(- 1) 5> }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\figuremode { <#-1> }"##,
        r##"f.ly:2:17: error: Guile signaled an error for the expression beginning here
\figuremode { <#
                -1> }
Unbound variable: -1>
f.ly:2:16: error: bass number expected
\figuremode { <
               #-1> }
f.ly:2:21: error: syntax error, unexpected '}'
\figuremode { <#-1> 
                    }
f.ly:2:13: error: errors found, ignoring music expression
\figuremode 
            { <#-1> }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\figuremode { <6+\+ -> }"##,
        r##"f.ly:2:16: warning: Dropping surplus alteration symbols for bass figure.
\figuremode { <
               6+\+ -> }
"##,
    ),
    (
        r##"\chordmode { c:foo }"##,
        r##"f.ly:2:16: error: not a note name: foo
\chordmode { c:
               foo }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\drummode { foo4 }"##,
        r##"f.ly:2:13: error: not a note name: foo
\drummode { 
            foo4 }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\chordmode { c/+e/f }"##,
        r##"f.ly:2:21: error: Spurious garbage following chord: (chord-slash #<Pitch f >)
\chordmode { c/+e/f 
                    }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\chordmode { c:7/ }"##,
        r##"f.ly:2:19: error: syntax error, unexpected '}', expecting TONICNAME_PITCH
\chordmode { c:7/ 
                  }
f.ly:2:1: error: errors found, ignoring music expression

\chordmode { c:7/ }
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\figures c"##,
        r##"f.ly:2:10: error: syntax error, unexpected SYMBOL, expecting \sequential or \simultaneous or << or '{'
\figures 
         c
f.ly:2:11: error: Unfinished main input
\figures c
          
fatal error: failed files: "f.ly"
"##,
    ),
    (
        r##"\figures \with { } c"##,
        r##"f.ly:2:20: error: syntax error, unexpected SYMBOL, expecting \sequential or \simultaneous or << or '{'
\figures \with { } 
                   c
f.ly:2:21: error: Unfinished main input
\figures \with { } c
                    
fatal error: failed files: "f.ly"
"##,
    ),
];

#[test]
fn p10_errors_are_lilyponds() {
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
