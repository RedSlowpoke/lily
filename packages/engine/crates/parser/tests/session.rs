//! The session (step 19, MS2 step 16): `declarations-init.ly` and the files
//! it includes parsed and run as an init file after the library files of
//! the installed LilyPond 2.26.0 (`support::load_library`), its variables
//! recorded in `(lily)`'s `lilypond-declarations` as `session-save` records
//! them, and each file's module using the init file's. That the 159 music
//! functions and their signatures are LilyPond's is checked by
//! `tools/bench/test/image.test.mjs`. Every test skips itself, and so
//! verifies nothing, without that lilypond.

use lily_engine_parser::{NoteNames, Parse, parse, parse_init};
use lily_engine_scheme::{Hosted, Vm};

mod support;

const V: &str = "\\version \"2.26.0\"\n";

/// A VM whose `(lily)` loaded the library files and the init file, and the
/// installed pitch names; None (and a note on stderr) without lilypond.
fn session() -> Option<(Vm, NoteNames)> {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0 installed");
        return None;
    };
    let names = NoteNames::read(&std::fs::read(lily.join("define-note-names.scm")).ok()?)
        .expect("define-note-names.scm reads");
    Some((support::library(&lily).expect("the library loads"), names))
}

fn stderr(parse: &Parse) -> String {
    String::from_utf8_lossy(&parse.stderr()).into_owned()
}

/// What `form` gives, written, in `(lily)`.
fn eval(vm: &mut Vm, form: &str) -> String {
    let value = vm
        .eval_text(
            &format!("(set-current-module (resolve-module '(lily))) {form}"),
            None,
        )
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    let mut out = String::new();
    vm.heap().write(value, &mut out);
    out
}

/// The pitch names the core takes from its image's `(lily)`, which loaded
/// `define-note-names.scm`, are those of the file's text.
#[test]
fn the_pitch_names_of_the_library_are_the_files() {
    let Some((mut vm, names)) = session() else {
        return;
    };
    let of_vm = NoteNames::of_vm(&mut vm).expect("(lily) has language-pitch-names");
    assert!(of_vm.languages_names().count() > 10);
    assert!(of_vm == names);
}

/// The core's start reads the pitch names from `(lily)`'s variable and
/// evaluates no Scheme, so that a fresh worker's first restore compiles
/// neither the reader nor the compiler (MS2 step 31, D90).
#[test]
fn the_pitch_names_are_read_with_no_procedure_applied() {
    let Some((mut vm, names)) = session() else {
        return;
    };
    let applications = vm.applications();
    let of_vm = NoteNames::of_vm(&mut vm).expect("(lily) has language-pitch-names");
    assert_eq!(vm.applications(), applications, "nothing applied");
    assert!(of_vm == names);
}

#[test]
fn the_init_files_variables_are_recorded_once() {
    let Some((mut vm, _)) = session() else { return };
    // Each variable the whole init defines, as LilyPond 2.26.0 records
    // them, but the ten `define-session-public` exports, which LilyPond's
    // `session-start-record` adds to the init's module and the engine's
    // `(lily)` exports (664 there); the 199 music functions among them.
    assert_eq!(
        eval(
            &mut vm,
            "(let ((parser (filter cadr lilypond-declarations)))
               (list (length parser)
                     (length (filter (lambda (r) (ly:music-function? (cdddr r))) parser))
                     (cdddr (assq 'afterGraceFraction parser))))"
        ),
        "(654 199 (3 . 4))"
    );
}

#[test]
fn each_file_is_given_them_and_keeps_its_own() {
    let Some((mut vm, names)) = session() else {
        return;
    };
    let first = parse(
        "a.ly",
        format!("{V}transpose = 5\n#(display (list transpose (ly:music-function? bar)))")
            .into_bytes(),
        &names,
        &mut vm,
    );
    assert_eq!(stderr(&first), "");
    assert_eq!(vm.take_output(), "(5 #t)");
    // The first file's assignment was its own.
    let second = parse(
        "b.ly",
        format!("{V}#(display (ly:music-function? transpose))").into_bytes(),
        &names,
        &mut vm,
    );
    assert_eq!(stderr(&second), "");
    assert_eq!(vm.take_output(), "#t");
}

#[test]
fn an_init_file_that_fails_records_nothing() {
    let Some((mut vm, names)) = session() else {
        return;
    };
    let count = "(length lilypond-declarations)";
    let before = eval(&mut vm, count);
    let failed = parse_init(
        "bad-init.ly",
        format!("{V}engineProbe = 1\n{{ c4").into_bytes(),
        &names,
        &mut vm,
        &mut |_| Hosted::Missing,
    );
    assert!(failed.failed());
    assert_eq!(eval(&mut vm, count), before);
    let ok = parse_init(
        "good-init.ly",
        format!("{V}engineProbe = 2").into_bytes(),
        &names,
        &mut vm,
        &mut |_| Hosted::Missing,
    );
    assert_eq!(stderr(&ok), "");
    let file = parse(
        "c.ly",
        format!("{V}#(display engineProbe)").into_bytes(),
        &names,
        &mut vm,
    );
    assert_eq!(stderr(&file), "");
    assert_eq!(vm.take_output(), "2");
}

/// The parts on demand (D74): their names are stubs in the library, which
/// stores them without loading the part, and a stub that is called loads
/// its part's files from source, after which the names are the part's.
#[test]
fn a_part_loads_when_one_of_its_stubs_is_called() {
    let Some((mut vm, _)) = session() else {
        return;
    };
    assert_eq!(eval(&mut vm, "(@@ (lily) %parts-loaded)"), "()");
    assert_eq!(
        eval(
            &mut vm,
            "(let ((stored (assq-ref (assq-ref all-grob-descriptions 'TabNoteHead) 'stencil)))
               (list (procedure-name stored) (eq? stored tab-note-head::print)))"
        ),
        "(tab-note-head::print #t)"
    );
    let stub = eval(&mut vm, "(object->string music->lily-string)");
    assert_eq!(
        eval(
            &mut vm,
            "(music->lily-string (make-music 'SkipEvent 'duration (ly:make-duration 2)))"
        ),
        "\"s4\""
    );
    assert_eq!(eval(&mut vm, "(@@ (lily) %parts-loaded)"), "(display)");
    assert_ne!(eval(&mut vm, "(object->string music->lily-string)"), stub);

    // Without the host's source, a stub's call fails, naming the file.
    vm.set_part_source(None);
    let e = vm
        .eval_text(
            "(set-current-module (resolve-module '(lily))) (make-modal-transposer 0 1 '())",
            None,
        )
        .unwrap_err();
    assert_eq!(
        e.text(vm.heap()),
        "In procedure %load-part!: needs modal-transforms.scm, which the host has not given"
    );
}

/// The init file's variables are shared, not copied (D66): a file's `set!`
/// changes the init's variable, which the next file sees until
/// `session-terminate` gives it its value again, as LilyPond does after
/// each file; and its `call-after-session` thunks run then.
#[test]
fn a_files_set_is_undone_when_the_session_ends() {
    let Some((mut vm, names)) = session() else {
        return;
    };
    let run = |vm: &mut Vm, text: &str| {
        let parse = parse("s.ly", format!("{V}{text}").into_bytes(), &names, vm);
        assert_eq!(stderr(&parse), "");
        vm.take_output()
    };
    assert_eq!(
        run(&mut vm, "#(set! breve 7) #(hash-set! musicQuotes 'q 1)"),
        ""
    );
    assert_eq!(
        run(&mut vm, "#(display (list breve (hash-ref musicQuotes 'q)))"),
        "(7 1)"
    );
    eval(&mut vm, "(session-terminate)");
    assert_eq!(
        run(
            &mut vm,
            "#(display (list (ly:duration? breve) (hash-ref musicQuotes 'q)))"
        ),
        "(#t #f)"
    );
}

/// What the whole init brings: `\language` switches the pitch names
/// (`ly:parser-set-note-names`), `ancient-init.ly`'s `\lyricmode`
/// syllables are lyric events, and toplevel music reaches the book
/// handlers, which collect a score of it in `toplevel-scores`.
#[test]
fn the_init_gives_languages_lyrics_and_book_handlers() {
    let Some((mut vm, names)) = session() else {
        return;
    };
    let parse = parse(
        "l.ly",
        format!(
            "{V}\\language \"english\"\nx = {{ cs4 }}\n{{ c4 }}\n\
             #(display (list (ly:music? x) (ly:music-property (car (ly:music-property IJ 'elements)) 'text)\n\
                             (ly:duration->string (ly:music-property (car (ly:music-property ij 'elements)) 'duration))\n\
                             (length toplevel-scores)))"
        )
        .into_bytes(),
        &names,
        &mut vm,
    );
    assert_eq!(stderr(&parse), "");
    assert_eq!(vm.take_output(), "(#t Ĳ 4 1)");
    assert_eq!(parse.toplevel_scores(&mut vm).len(), 1);
}

/// A file's scores come in its order (MS2 step 40): `toplevel-scores`
/// holds the last first, and LilyPond's `Book::process` reverses it.
#[test]
fn a_files_scores_come_in_its_order() {
    let Some((mut vm, names)) = session() else {
        return;
    };
    let parse = parse(
        "s.ly",
        format!("{V}{{ c4 }}\n{{ d2 }}\n{{ e1 }}\n").into_bytes(),
        &names,
        &mut vm,
    );
    assert_eq!(stderr(&parse), "");
    let scores = parse.toplevel_scores(&mut vm);
    let lily = vm.eval_text("(resolve-module '(lily))", None).ok().unwrap();
    for (i, &score) in scores.iter().enumerate() {
        assert!(vm.module_define(lily, &format!("score-{i}"), score));
    }
    assert_eq!(
        eval(
            &mut vm,
            "(map (lambda (s) (ly:moment-main (ly:music-length (ly:score-music s)))) (list score-0 score-1 score-2))"
        ),
        "(1/4 1/2 1)"
    );
}
