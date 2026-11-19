//! N3's context definitions (04 §7, D73): what `\context { … }` makes of
//! its mods, in a value and in an output definition, `\with` and music in
//! it (`context-mod-music-handler`), music in an output definition
//! (`output-def-music-handler`), and the Scheme face, `ly:context-def?`,
//! `ly:context-def-lookup`, `ly:context-def-modify`,
//! `ly:context-mod-apply!` and the output definitions' lookups, with their
//! errors and LilyPond's `programming error` for a mod of no known tag.
//! [`INPUT`] is parsed with `$defaultlayout`'s context definitions, which
//! the whole init made (`tests/support`'s `library`), and what it writes to stdout and
//! stderr equals what LilyPond 2.26.0 writes for it.
//! `tools/compare/test/contexts.test.mjs` compares the definitions of
//! `engraver-init.ly` and `performer-init.ly` themselves.
//!
//! The test skips itself, and so verifies nothing, without that lilypond.

use std::path::Path;
use std::process::Command;

mod support;

/// The input: definitions with every mod, their lookups, a derived one,
/// predicates, an output definition with definitions, music modifying
/// them through their names and aliases and naming one of none, and the
/// errors of the primitives' arguments.
const INPUT: &str = r##"\version "2.26.0"
x = \context { \name Foo \accepts A \accepts B \defaultchild C \accepts D \denies A \alias X \alias "Y"
  \consists "E" \consists #'((name . Tr)) \remove "E" \consists F \consists "E" \type Engraver_group
  \description "d" \grobdescriptions #'((G . ())) x = 1 \override G.p = 2 \revert G.p \unset u
  #(ly:make-context-mod '((foo bar))) \with { \denies C } \cadenzaOn }
#(write x) #(newline)
#(for-each (lambda (k) (write (ly:context-def-lookup x k)) (newline))
  '(context-name aliases accepts default-child group-type consists description grob-descriptions property-ops other))
#(write (ly:context-def-lookup x 'other 5)) #(newline)
y = #(ly:context-def-modify x (ly:make-context-mod '((context-name Bar) (accepts Q) (bad 1) (alias Z) (default-child D))))
#(write y) #(newline)
#(write (map (lambda (k) (ly:context-def-lookup y k)) '(accepts aliases default-child))) #(newline)
#(write (map (lambda (k) (ly:context-def-lookup x k)) '(context-name accepts))) #(newline)
#(write (list (ly:context-def? x) (ly:context-def? 1) (ly:output-def? $defaultlayout) (ly:output-def? x)))
#(newline)
lay = \layout {
  \context { \name Foo \alias Bar \consists "A" }
  \context { \name Bar }
  \context { \Foo \name Baz \description "z" }
  \override Bar.G.p = 3
  \set Foo.fontSize = 4
  \set Quux.fontSize = 4
  \unset Bar.fontSize
  \context { \Bar \with { \consists "B" } }
}
#(define (by-name l) (sort l (lambda (a b) (string<? (symbol->string (car a)) (symbol->string (car b))))))
#(write (by-name (ly:output-find-context-def lay 'Bar))) #(newline)
#(write (length (ly:output-find-context-def lay))) #(newline)
#(write (filter (lambda (d) (memq (car d) '(Foo Bar Baz))) (by-name (ly:output-description lay))))
#(newline)
#(write (list (ly:output-def-lookup lay 'Foo #f) (ly:output-def-lookup lay 'nothing) (ly:output-def-lookup lay 'nothing 7)))
#(newline)
#(ly:output-def-set-variable! lay 'abc 1)
#(write (ly:output-def-lookup lay 'abc)) #(newline)
#(write (module? (ly:output-def-scope lay))) #(newline)
#(ly:context-def-lookup 1 'x)
#(ly:context-def-lookup x 1)
#(ly:context-def-modify x 1)
#(ly:context-mod-apply! x (ly:make-context-mod))
#(ly:output-def-lookup x 'a)
#(ly:output-find-context-def lay 1)
"##;

/// What LilyPond 2.26.0 writes to stdout and stderr for `text`, the file
/// `name`, with `--loglevel=WARNING`.
fn lilypond(name: &str, text: &str) -> (String, String) {
    let dir = std::env::temp_dir().join(format!("lily-n3-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), text).unwrap();
    let run = Command::new("lilypond")
        .args(["--loglevel=WARNING", "-dno-print-pages", name])
        .current_dir(&dir)
        .env("LANGUAGE", "en")
        .output()
        .expect("lilypond runs");
    std::fs::remove_dir_all(&dir).ok();
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    (text(&run.stdout), text(&run.stderr))
}

#[test]
fn context_definitions_answer_as_lilyponds() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0 installed");
        return;
    };
    let lily: &Path = &lily;
    let mut vm = support::library(lily).expect("the library loads");
    let text = std::fs::read(lily.join("define-note-names.scm")).unwrap();
    let names = lily_engine_parser::NoteNames::read(&text).expect("define-note-names.scm reads");
    let parse = lily_engine_parser::parse("n3.ly", INPUT.as_bytes().to_vec(), &names, &mut vm);
    let stdout = vm.take_output();
    let stderr = String::from_utf8_lossy(&parse.stderr()).into_owned();
    let (want_out, want_err) = lilypond("n3.ly", INPUT);
    assert_eq!(
        want_out.lines().count(),
        22,
        "LilyPond's stdout:\n{want_out}"
    );
    assert!(want_err.contains("unknown context mod tag"), "{want_err}");
    support::same(&want_out, &stdout, "stdout");
    support::same(&want_err, &stderr, "stderr");
}
