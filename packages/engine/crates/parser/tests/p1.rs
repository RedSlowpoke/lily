//! P1 (02 §6): sources, comments, strings, `\version`, spans and LilyPond's
//! message format. Each expected stderr was printed by LilyPond 2.26.0
//! (`lilypond --loglevel=WARNING` with `LANGUAGE=en`) for the same input; the
//! error corpus's cases 03–06, 26 and 27 are scored with C0 by
//! `tools/compare/test/c0-cases.test.mjs`.

use lily_engine_parser::{Lexer, Mode, NoteNames, Parse, Source, Tok, VersionSeen};
use lily_engine_scheme::Vm;

const V: &str = "\\version \"2.24.0\"\n";

/// A stand-in for `define-note-names.scm` with the two names P1's inputs
/// use, in its form, so that P1's tests need no lilypond.
const NOTE_NAMES: &str = "(define-session-public language-pitch-names
  `((nederlands . ((c . ,(ly:make-pitch -1 0 NATURAL))
                   (d . ,(ly:make-pitch -1 1 NATURAL))))))";

fn parse(name: &str, text: Vec<u8>) -> Parse {
    let names = NoteNames::read(NOTE_NAMES.as_bytes()).unwrap();
    lily_engine_parser::parse(name, text, &names, &mut Vm::new())
}

fn stderr(name: &str, text: &[u8]) -> String {
    String::from_utf8_lossy(&parse(name, text.to_vec()).stderr()).into_owned()
}

/// The locations, `file:line:column: severity`, of each message.
fn heads(name: &str, text: &[u8]) -> Vec<String> {
    stderr(name, text)
        .lines()
        .filter(|l| {
            l.contains(": error: ") || l.contains(": warning: ") || l.starts_with("error: ")
        })
        .map(|l| l.split(": ").take(2).collect::<Vec<_>>().join(": "))
        .collect()
}

#[test]
fn columns_count_tabs_to_eight_and_utf8_characters_once() {
    let source = Source::new(
        "t.ly",
        "{\n\tc4\t\\foo d\n\\markup \"äöü\" x\n".as_bytes().to_vec(),
    );
    let foo = source
        .text()
        .windows(4)
        .position(|w| w == b"\\foo")
        .unwrap() as u32;
    let c = source.counts(foo);
    assert_eq!((c.line, c.char, c.column, c.offset), (2, 4, 16, 4));
    assert_eq!(source.location(foo), "t.ly:2:17");
    assert_eq!(source.quote(foo), b"\tc4\t\n                \\foo d");
    let x = source.text().len() as u32 - 2;
    let c = source.counts(x);
    assert_eq!((c.line, c.char, c.column, c.offset), (3, 14, 14, 17));
    assert_eq!(source.location(0), "t.ly:1:1");
    assert_eq!(source.quote(1), b"{\n ");
}

#[test]
fn the_line_of_a_byte_is_counted_by_the_newlines_before_it() {
    let source = Source::new("t.ly", b"a\nb\n".to_vec());
    assert_eq!([0, 1, 2, 3, 4].map(|p| source.line(p)), [1, 1, 2, 2, 3]);
    assert_eq!(Source::new("t.ly", b"ab".to_vec()).line(2), 1);
}

#[test]
fn strings_keep_lilyponds_escapes_and_are_located_at_their_closing_quote() {
    let text = format!("{V}{{ c4 \"a\\qb\\\"c\\n\\t\\\\\\'\" }}\n");
    let source = Source::new("c.ly", text.into_bytes());
    let mut lexer = Lexer::new(&source, 0);
    let token = std::iter::from_fn(|| Some(lexer.next(Mode::Notes)))
        .find(|t| t.kind == Tok::String)
        .unwrap();
    assert_eq!(lexer.string(&token), "a\\qb\"c\n\t\\'");
    assert_eq!(source.location(token.loc.start), "c.ly:2:22");
    assert_eq!(
        &source.text()[token.span.start as usize..token.span.end as usize],
        b"\"a\\qb\\\"c\\n\\t\\\\\\'\""
    );
}

#[test]
fn a_string_in_music_is_an_error_at_its_closing_quote() {
    let text = format!("{V}{{ c4 \"a\\qb\\\"c\\n\" }}\n");
    assert_eq!(
        stderr("c.ly", text.as_bytes()),
        "c.ly:2:16: error: string outside of text script or \\lyricmode\n\
         { c4 \"a\\qb\\\"c\\n\n               \" }\n\
         fatal error: failed files: \"c.ly\"\n"
    );
}

#[test]
fn the_end_of_input_inside_a_string_is_located_at_what_was_matched_last() {
    // The text after the quote, then the quote itself when nothing follows it.
    let text = format!("{V}{{ c4 \"abc");
    assert_eq!(
        stderr("h.ly", text.as_bytes()),
        "h.ly:2:7: error: EOF found inside string\n{ c4 \"\n      abc\n\
         h.ly:2:7: error: syntax error, unexpected end of input\n{ c4 \"\n      abc\n\
         fatal error: failed files: \"h.ly\"\n"
    );
    let text = format!("{V}{{ c4 \"");
    assert_eq!(
        heads("i.ly", text.as_bytes()),
        ["i.ly:2:6: error", "i.ly:2:6: error"]
    );
}

#[test]
fn the_end_of_input_is_located_at_the_last_white_space() {
    // Case 09's shape.
    let text = format!("{V}{{\n  {{ c4 d\n");
    assert_eq!(
        stderr("e.ly", text.as_bytes()),
        "e.ly:3:9: error: syntax error, unexpected end of input\n  { c4 d\n        \n\
         fatal error: failed files: \"e.ly\"\n"
    );
}

#[test]
fn comments_end_at_the_line_or_at_percent_brace() {
    let text = format!("{V}%{{ a %%}} {{ c4 \\\"foo\" }} % \\bar\n{{ d4 }} %\n%");
    assert_eq!(
        stderr("d.ly", text.as_bytes()),
        "d.ly:2:20: error: unknown command: `\\foo'\n%{ a %%} { c4 \\\"foo\n                   \" } % \\bar\n\
         d.ly:2:20: error: string outside of text script or \\lyricmode\n%{ a %%} { c4 \\\"foo\n                   \" } % \\bar\n\
         fatal error: failed files: \"d.ly\"\n"
    );
}

#[test]
fn an_unclosed_comment_is_located_at_its_last_piece() {
    let text = format!("{V}{{ c4 }}\n%{{ never %% closed\n");
    assert_eq!(heads("c.ly", text.as_bytes()), ["c.ly:3:10: error"]);
    let text = format!("{V}{{ c4 }}\n%{{");
    assert_eq!(heads("c.ly", text.as_bytes()), ["c.ly:3:1: error"]);
}

#[test]
fn version_strings_are_checked_as_lily_library_checks_them() {
    let run = |version: &str| {
        let parsed = parse("v.ly", format!("{V}\\version \"{version}\"\n").into_bytes());
        (
            String::from_utf8_lossy(&parsed.stderr()).into_owned(),
            parsed.version,
        )
    };
    assert_eq!(
        run("2.24.0.foo"),
        (String::new(), VersionSeen::Valid("2.24.0.foo".into()))
    );
    assert_eq!(
        run("2.26"),
        (String::new(), VersionSeen::Valid("2.26".into()))
    );
    assert_eq!(
        run("9.1.0"),
        (
            "error: program too old: 2.26.0 (file requires: 9.1.0)\nfatal error: failed files: \"v.ly\"\n".into(),
            VersionSeen::Invalid
        )
    );
    assert_eq!(
        run("x.y.z").0,
        "error: Invalid version string \"x.y.z\"\nfatal error: failed files: \"v.ly\"\n"
    );
    assert_eq!(
        run("2").0,
        "error: Invalid version string \"2\"\nfatal error: failed files: \"v.ly\"\n"
    );
    assert_eq!(
        run("2.25").0,
        "error: version with third number omitted is only allowed for stable releases (when the second number is even)\n\
         fatal error: failed files: \"v.ly\"\n"
    );
}

#[test]
fn a_version_needs_its_quotes() {
    // Case 05's shape: the character after \version is consumed with the error.
    assert_eq!(
        heads("v.ly", format!("{V}\\version 2.24\n").as_bytes()),
        ["v.ly:2:10: error", "v.ly:2:11: error"]
    );
    assert_eq!(
        stderr("c.ly", b"\\version \"2.24.0\n{ c4 }\n"),
        "c.ly:1:10: error: end quote missing\n\\version \n         \"2.24.0\n\
         c.ly:1: warning: no \\version statement found, please add\n\n\\version \"2.26.0\"\n\nfor future compatibility\n\
         fatal error: failed files: \"c.ly\"\n"
    );
    // A longer word is a command.
    assert_eq!(
        heads("k.ly", format!("{V}{{ \\versionx }}\n").as_bytes()),
        ["k.ly:2:3: error", "k.ly:2:3: error"]
    );
}

#[test]
fn a_byte_order_mark_is_skipped_at_the_start_and_warned_about_elsewhere() {
    assert_eq!(
        heads("j.ly", format!("\u{feff}{V}{{ c4 }}\n").as_bytes()),
        Vec::<String>::new()
    );
    assert_eq!(
        stderr("e.ly", "{ c4 }\n\u{feff}\n".as_bytes()),
        "e.ly:2:1: warning: stray UTF-8 BOM encountered\n\n\u{feff}\n\
         e.ly:1: warning: no \\version statement found, please add\n\n\\version \"2.26.0\"\n\nfor future compatibility\n"
    );
}

#[test]
fn non_utf8_input_is_warned_about_as_yytext_utf8_does() {
    // An overlong C0 80, a surrogate ED A0 80, a valid U+1F3B5, and Latin-1 é:
    // each byte of a refused sequence on its own, the byte after a broken one
    // skipped with it.
    let mut text = V.as_bytes().to_vec();
    text.extend_from_slice(b"% \xc0\x80 \xed\xa0\x80 \xf0\x9f\x8e\xb5 \xe9\n{ c4 }\n");
    let parsed = parse("g.ly", text);
    assert!(!parsed.failed());
    assert_eq!(
        heads("g.ly", parsed.sources.get(0).text()),
        [
            "g.ly:2:3: warning",
            "g.ly:2:4: warning",
            "g.ly:2:5: warning",
            "g.ly:2:6: warning",
            "g.ly:2:6: warning",
            "g.ly:2:9: warning"
        ]
    );
    // The excerpt keeps the source's bytes.
    assert!(parsed.stderr().windows(4).any(|w| w == b"\xed\xa0\x80 "));
}

#[test]
fn bison_reports_no_syntax_error_until_three_tokens_are_shifted() {
    let text = format!("{V}{{ c4 }} }} }} }}\n");
    assert_eq!(heads("a.ly", text.as_bytes()), ["a.ly:2:8: error"]);
    let text = format!("{V}}} }} {{ c4 }}\n}} {{ c4 }} }}\n");
    assert_eq!(
        heads("n.ly", text.as_bytes()),
        ["n.ly:2:1: error", "n.ly:3:1: error", "n.ly:3:10: error"]
    );
}

#[test]
fn headers_and_markup_parse_without_errors() {
    let text = format!(
        "{V}\\header {{ title = \"x\" composer = y }}\n{{ c4 }}\n\\markup \"a\" \\markup {{ b c }}\n"
    );
    assert_eq!(stderr("o.ly", text.as_bytes()), "");
}
