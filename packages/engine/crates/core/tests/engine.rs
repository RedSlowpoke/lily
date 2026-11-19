//! K2 (MS2 steps 10–11): the core's engine as chapter 08 §4.2's ABI drives
//! it, natively, from a core image of a VM with the music model that ran a
//! stand-in for `define-note-names.scm`, so that it needs no lilypond: its
//! two outputs from one list (08 §6), and compiles in a row in one
//! instance. `tools/bench/test` runs the built core.wasm through the same
//! calls, and `tools/compare/test/c0-cases.test.mjs` the error corpus.
//! Since step 19, the files a compile asks for: those a score includes,
//! and a part's (D74), through `lily_step`'s 2 and `lily_needs`; since
//! step 30, a part's files all asked for at one stop, T05's and T29's
//! parts among them, from the installed LilyPond 2.26.0 and the corpus,
//! which skip themselves without either.

use lily_engine_core::{
    DONE, Engine, FILE, MODULE, MORE, NEEDS, PART_MODULES, START_MARKS, write_image, write_part,
};
use lily_engine_parser::NoteNames;
use lily_engine_parser::source::DATADIR;
use lily_engine_scheme::{RESTORE_MARKS, Vm};
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[path = "../../parser/tests/support/mod.rs"]
mod support;

/// The two names the scores here use, in `define-note-names.scm`'s form.
const NOTE_NAMES: &str = "(define-session-public language-pitch-names
  `((nederlands . ((c . ,(ly:make-pitch -1 0 NATURAL))
                   (d . ,(ly:make-pitch -1 1 NATURAL))))))";

/// A VM with the music model and layout, before the pitch names.
fn bare() -> Vm {
    let mut vm = Vm::new();
    assert!(lily_engine_core::install(&mut vm).is_ok());
    vm
}

/// A VM with the music model and [`NOTE_NAMES`] run in `(lily)`.
fn vm() -> Vm {
    let mut vm = bare();
    let text =
        format!("(set-current-module (resolve-module '(lily))) (define NATURAL 0) {NOTE_NAMES}");
    assert!(vm.eval_text(&text, None).is_ok());
    vm
}

fn engine() -> Engine {
    engine_of(vm())
}

fn engine_of(mut vm: Vm) -> Engine {
    let scheme = vm.write_image().expect("the VM is at rest");
    Engine::start(&write_image(&scheme)).expect("the image restores")
}

/// The compile's two outputs, `stderr` and `diagnostics.json`.
fn outputs(engine: &Engine) -> (String, String) {
    match engine.outputs() {
        [(e, stderr), (d, json)] if e == "stderr" && d == "diagnostics.json" => (
            String::from_utf8_lossy(stderr).into_owned(),
            String::from_utf8(json.clone()).expect("JSON is UTF-8"),
        ),
        outputs => panic!("{:?}", outputs.iter().map(|(n, _)| n).collect::<Vec<_>>()),
    }
}

fn stderr(engine: &Engine) -> String {
    outputs(engine).0
}

/// Compiles `text` as `name` to its end.
fn compile(engine: &mut Engine, name: &str, text: &str) {
    engine.put(FILE, name, Some(text.as_bytes()));
    engine.compile(1, name);
    assert_eq!(engine.step(10.0), DONE);
}

#[test]
fn a_compile_asks_for_its_root_then_writes_what_the_parser_prints() {
    let mut engine = engine();
    engine.put(MODULE, "emmentaler-20", Some(b"a pack"));
    engine.compile(1, "/s/score.ly");
    assert_eq!(engine.step(10.0), NEEDS);
    assert_eq!(engine.needs(), Some((FILE, "/s/score.ly")));
    assert!(engine.outputs().is_empty());

    let text = b"{ c4 d e }\n";
    engine.put(FILE, "/s/score.ly", Some(text));
    engine.compile(1, "/s/score.ly");
    assert_eq!(engine.step(10.0), DONE);
    let names = NoteNames::read(NOTE_NAMES.as_bytes()).unwrap();
    let parsed = lily_engine_parser::parse("/s/score.ly", text.to_vec(), &names, &mut vm());
    assert!(parsed.failed(), "e is no note name here");
    assert_eq!(stderr(&engine), String::from_utf8_lossy(&parsed.stderr()));
    assert_eq!(engine.step(10.0), DONE, "a compile done stays done");

    // The same instance compiles again, from the files it has.
    engine.put(
        FILE,
        "/s/score.ly",
        Some(b"\\version \"2.26.0\"\n{ c4 d }\n"),
    );
    engine.compile(1, "/s/score.ly");
    assert_eq!(engine.step(10.0), DONE);
    assert_eq!(outputs(&engine), (String::new(), String::from("[]")));
}

#[test]
fn a_root_the_host_does_not_have_is_lilyponds_missing_file() {
    let mut engine = engine();
    engine.put(FILE, "nosuch.ly", None);
    engine.compile(1, "nosuch.ly");
    assert_eq!(engine.step(10.0), DONE);
    assert_eq!(
        outputs(&engine),
        (
            String::from(
                "warning: cannot find file: `nosuch.ly'\nfatal error: failed files: \"nosuch.ly\"\n"
            ),
            String::from(concat!(
                r#"[{"file":"nosuch.ly","line":1,"severity":"warning","message":"cannot find file: `nosuch.ly'"},"#,
                r#"{"file":"nosuch.ly","line":1,"severity":"error","message":"failed files: \"nosuch.ly\""}]"#
            ))
        )
    );
}

/// What Scheme writes to the error port waits for `force-output`, and
/// LilyPond's messages do not: LilyPond 2.26.0 printed this file's stderr
/// so. Each message's JSON holds the lines that follow it, as
/// `parseStderr` reads them, an excerpt left out.
#[test]
fn messages_take_their_place_among_what_the_error_port_flushed() {
    let mut engine = engine();
    let err = "(current-error-port)";
    compile(
        &mut engine,
        "/s/a.ly",
        &format!(
            "\\version \"2.26.0\"
#(begin (display \"one\\n\" {err}) (ly:warning \"w1\") (force-output {err}) (ly:warning \"w2\") (display \"two\\n\" {err}) (ly:warning \"w3\"))
#(ly:warning \"w4\")
#(begin (display \"three\\n\" {err}) (force-output {err}))
#(ly:warning \"w5\")
#(display \"four\\n\" {err})
#(ly:warning \"w6\")
{{ c4 c, }}
"
        ),
    );
    let (stderr, json) = outputs(&engine);
    assert_eq!(
        stderr,
        "warning: w1\none\nwarning: w2\nwarning: w3\nwarning: w4\ntwo\nthree\n\
         warning: w5\nwarning: w6\nfour\n"
    );
    let warning =
        |m: &str| format!(r#"{{"file":"/s/a.ly","line":1,"severity":"warning","message":"{m}"}}"#);
    let expected = [
        "w1\\none",
        "w2",
        "w3",
        "w4\\ntwo\\nthree",
        "w5",
        "w6\\nfour",
    ]
    .map(warning)
    .join(",");
    assert_eq!(json, format!("[{expected}]"));
}

/// A Guile error that escapes `ly:parse-file` ends the run (case 20, D64):
/// `reverse!` of a markup command's arguments, one of which failed, here of
/// a stand-in for `define-markup-commands.scm`'s `\fontsize`. Its text
/// follows the error, which `parseStderr` reads it into, and no `failed
/// files` closes the run.
#[test]
fn a_guile_error_ends_the_run_in_the_message_before_it() {
    let mut vm = vm();
    let markup = "(set-current-module (resolve-module '(lily)))
        (define-public markup-command-signature (make-object-property))
        (define-public markup-list-function? (make-object-property))
        (define-public (markup? x) (string? x))
        (define-public (markup-list? x) #f)
        (define-public (type-name pred) \"number\")
        (define-public (fontsize-markup layout props size arg) arg)
        (set! (markup-command-signature fontsize-markup) (list number? markup?))";
    assert!(vm.eval_text(markup, None).is_ok());
    let mut engine = engine_of(vm);
    compile(
        &mut engine,
        "a.ly",
        "\\version \"2.26.0\"\n\\markup \\fontsize #\"big\" text\n",
    );
    let abort = "ERROR: In procedure ly:parse-file:\n\
                 In procedure reverse!: Wrong type argument in position 1: (\"big\" . #f)";
    let (stderr, json) = outputs(&engine);
    assert_eq!(
        stderr,
        format!(
            "a.ly:2:19: error: wrong type for argument 1.  Expecting number, found \"big\"\n\
             \\markup \\fontsize \n                  #\"big\" text\n{abort}\n"
        )
    );
    let message = "wrong type for argument 1.  Expecting number, found \\\"big\\\"\\n\
                   ERROR: In procedure ly:parse-file:\\n\
                   In procedure reverse!: Wrong type argument in position 1: (\\\"big\\\" . #f)";
    assert_eq!(
        json,
        format!(
            r#"[{{"file":"a.ly","line":2,"column":19,"severity":"error","message":"{message}"}}]"#
        )
    );
}

/// A score with its own variables, Scheme, music and scopes (`\score`,
/// `\layout`, `\header`), and an error, compiled a hundred times in one
/// instance: the heap live after each is what it was after the first, so
/// the values each made and the modules of its file and scopes are gone.
#[test]
fn compiles_in_a_row_leave_the_live_heap_where_the_first_left_it() {
    let mut engine = engine();
    let score = "\\version \"2.26.0\"
\\header { title = \"t\" }
#(define-public notes (list 1 2 3))
melody = { c4 d c d }
\\score { \\new Staff \\melody \\layout { indent = 0 } }
\\paper { #(define line-width 100) }
{ \\melody #(make-list 100 'x) e }
";
    compile(&mut engine, "/s/score.ly", score);
    let first = stderr(&engine);
    assert!(first.contains("error:"), "{first}");
    let after_one = engine.collect().live_bytes;
    for _ in 0..99 {
        compile(&mut engine, "/s/score.ly", score);
        assert_eq!(stderr(&engine), first);
    }
    let after_hundred = engine.collect().live_bytes;
    assert_eq!(
        after_hundred, after_one,
        "bytes live after 1 and 100 compiles"
    );
}

#[test]
fn a_start_marks_its_parts_in_order() {
    let image = write_image(&vm().write_image().unwrap());
    let mut marks = Vec::new();
    let marked = Engine::start_marked(&image, &mut |part| marks.push(part));
    assert!(marked.is_ok(), "the image restores");
    assert_eq!(marks, START_MARKS, "lily_start's parts (lsb_marks)");
    assert_eq!(
        &marks[..RESTORE_MARKS.len()],
        RESTORE_MARKS,
        "the VM's first"
    );
    let mut cut = Vec::new();
    let refused = Engine::start_marked(&image[..image.len() / 2], &mut |part| cut.push(part));
    assert!(refused.is_err(), "cut short");
    assert!(
        cut.len() < RESTORE_MARKS.len(),
        "a refused image ends no later part: {cut:?}"
    );
}

#[test]
fn an_image_that_is_not_the_cores_is_refused() {
    let scheme = vm().write_image().unwrap();
    assert!(
        Engine::start(&scheme).is_err(),
        "the interpreter's image alone"
    );
    let bare = bare().write_image().unwrap();
    assert!(
        Engine::start(&write_image(&bare)).is_err(),
        "no pitch names"
    );
    let image = write_image(&scheme);
    assert!(
        Engine::start(&image[..image.len() / 2]).is_err(),
        "cut short"
    );
}

/// Steps a compile of `root` to its end, answering each file it asks for
/// from `host` (None: the host has no such file); the names it asked for.
fn answer(engine: &mut Engine, root: &str, host: &[(&str, Option<&str>)]) -> Vec<String> {
    let mut asked = Vec::new();
    engine.compile(1, root);
    while engine.step(10.0) == NEEDS {
        let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
        assert_eq!(kind, FILE);
        assert!(!asked.contains(&name), "{name} is asked for twice");
        let data = host.iter().find(|(n, _)| *n == name).and_then(|(_, d)| *d);
        engine.put(FILE, &name, data.map(str::as_bytes));
        asked.push(name);
        engine.compile(1, root);
    }
    asked
}

/// S5 (MS6 step 2): what a compile changes of what is older than it, a
/// global, a table, an option, is undone after it, and after a stop for the
/// host too, so that each compile starts where a fresh engine's would
/// (OPEN C7) and a score's `ly:set-option` ends with it (C2.2).
#[test]
fn a_compiles_scheme_changes_end_with_it() {
    let mut vm = vm();
    let state = "(set-current-module (resolve-module '(lily)))
        (define-public undo-count 0)
        (define-public undo-table (make-hash-table))";
    assert!(vm.eval_text(state, None).is_ok());
    let mut engine = engine_of(vm);
    let score = "\\version \"2.26.0\"
#(ly:warning (format #f \"~a ~a ~a\" undo-count (hashq-ref undo-table 'k) (ly:get-option 'point-and-click)))
#(set! undo-count (1+ undo-count))
#(hashq-set! undo-table 'k 'set)
#(ly:set-option 'point-and-click #f)
";
    for _ in 0..3 {
        compile(&mut engine, "a.ly", score);
        assert_eq!(stderr(&engine), "warning: 0 #f #t\n");
    }
    // A compile that stops for an include after its changes parses again
    // from the state before them.
    let stopping = format!("{score}\\include \"b.ily\"\n");
    engine.put(FILE, "/s/a.ly", Some(stopping.as_bytes()));
    let asked = answer(&mut engine, "/s/a.ly", &[("/s/b.ily", Some("% b\n"))]);
    // LilyPond's `ly/` at the same stop (OPEN F, D263).
    assert_eq!(asked, ["/s/b.ily", "<datadir>/ly/b.ily"]);
    assert_eq!(engine.parses(), 2);
    assert_eq!(stderr(&engine), "warning: 0 #f #t\n");
}

/// `\include` looks beside the including file, then in the score's
/// directory, then in LilyPond's `ly/` (`<datadir>/ly`), asking the host
/// for each until one is there; locations name the file as found. Since
/// MS7 step 7 (OPEN F, D263) the candidates are asked at one stop, up to one
/// the host has: three parses, not four. The compile goes on from the files
/// given, and gives what a compile that had them all from the start gives.
#[test]
fn an_include_is_asked_for_where_lilypond_looks_and_the_compile_goes_on() {
    let score = "\\version \"2.26.0\"\n\\include \"parts/a.ily\"\n{ \\partA \\partB }\n";
    let host = [
        ("/s/score.ly", Some(score)),
        (
            "/s/parts/a.ily",
            Some("partA = { c4 }\n\\include \"b.ily\"\n"),
        ),
        ("/s/b.ily", Some("partB = { d4 \\oops }\n")),
    ];
    let mut engine = engine();
    engine.put(FILE, "/s/score.ly", Some(score.as_bytes()));
    let asked = answer(&mut engine, "/s/score.ly", &host);
    assert_eq!(
        asked,
        [
            "/s/parts/a.ily",
            "<datadir>/ly/parts/a.ily",
            "/s/parts/b.ily",
            "/s/b.ily",
            "<datadir>/ly/b.ily"
        ]
    );
    assert_eq!(engine.parses(), 3);
    let (stderr, json) = outputs(&engine);
    assert!(
        stderr.starts_with("/s/b.ily:1:14: error: unknown command: `\\oops'\n"),
        "{stderr}"
    );

    // The same in one step, every file given first.
    let mut fresh = engine_of(vm());
    for (name, text) in host {
        fresh.put(FILE, name, text.map(str::as_bytes));
    }
    fresh.put(FILE, "/s/parts/b.ily", None);
    assert!(answer(&mut fresh, "/s/score.ly", &[]).is_empty());
    assert_eq!(outputs(&fresh), (stderr, json));
}

/// A file no directory has is LilyPond's error, with the search path it
/// prints, a directory of none as `.`; the parse goes on after it.
#[test]
fn an_include_the_host_does_not_have_is_lilyponds_error() {
    let mut engine = engine();
    let score = "\\version \"2.26.0\"\n\\include \"nosuch.ily\"\n{ c4 }\n";
    engine.put(FILE, "score.ly", Some(score.as_bytes()));
    let asked = answer(&mut engine, "score.ly", &[]);
    assert_eq!(asked, ["nosuch.ily", "<datadir>/ly/nosuch.ily"]);
    assert_eq!(
        stderr(&engine),
        "score.ly:2:10: error: cannot find file: `nosuch.ily'\n\
         (search path: `.:.:<datadir>/ly:<datadir>/ps:<datadir>/scm:<datadir>/fonts/otf/:<datadir>/fonts/svg/:')\n\
         \\include \n         \"nosuch.ily\"\n\
         fatal error: failed files: \"score.ly\"\n"
    );
}

/// A VM whose `(lily)` has a part `tiny` of two files, with a stub for
/// `tiny-twice`, and a part `tinymod` whose own module, `(lily tiny)`,
/// loads it when a file uses it (D74, D76).
fn parts_vm() -> Vm {
    let mut vm = vm();
    let parts = "(set-current-module (resolve-module '(lily)))
        (set! %parts '((tiny \"tiny-a.scm\" \"tiny-b.scm\") (tinymod \"tiny-mod.scm\")))
        (define-public tiny-twice (%part-stub 'tiny '(lily) 'tiny-twice))
        (define-module (lily tiny))
        (set-current-module (resolve-module '(lily)))
        (set! %module-hooks (acons '(lily tiny) (lambda () (%load-part! 'tinymod)) %module-hooks))";
    assert!(vm.eval_text(parts, None).is_ok());
    vm
}

const TINY: [(&str, Option<&str>); 3] = [
    (
        "<datadir>/scm/lily/tiny-a.scm",
        Some("(define-public (tiny-once x) x)"),
    ),
    (
        "<datadir>/scm/lily/tiny-b.scm",
        Some("(define-public (tiny-twice x) (string-append (tiny-once x) x))"),
    ),
    (
        "<datadir>/scm/lily/tiny-mod.scm",
        Some("(define-module (lily tiny)) (define-public (tiny-hello) \"hello\")"),
    ),
];

/// A stub's call asks for each of its part's files, under LilyPond's
/// `scm/lily/`, before any of them runs, all at one stop; the stop passes
/// the score's own `catch`. Then the part is loaded and the call goes on:
/// two parses.
#[test]
fn a_stubs_call_asks_for_its_parts_files_and_the_compile_goes_on() {
    let mut engine = engine_of(parts_vm());
    let score = "\\version \"2.26.0\"
#(ly:warning (catch #t (lambda () (tiny-twice \"x\")) (lambda _ \"caught\")))
";
    engine.put(FILE, "a.ly", Some(score.as_bytes()));
    let asked = answer(&mut engine, "a.ly", &TINY);
    assert_eq!(
        asked,
        [
            "<datadir>/scm/lily/tiny-a.scm",
            "<datadir>/scm/lily/tiny-b.scm"
        ]
    );
    assert_eq!(engine.parses(), 2);
    assert_eq!(stderr(&engine), "warning: xx\n");
    // Loaded, the part asks for nothing more.
    assert!(answer(&mut engine, "a.ly", &[]).is_empty());
    assert_eq!(engine.parses(), 1);
    assert_eq!(stderr(&engine), "warning: xx\n");
}

/// A kept part loaded again does not run its `add-new-clef` a second time:
/// the image holds what it added, and the list is `set!`, not a `define`
/// (OPEN C9: `moderntab` twice in an unknown clef's message after T22).
#[test]
fn a_kept_parts_clef_is_added_once() {
    let mut vm = parts_vm();
    let kept = "(set-current-module (resolve-module '(lily)))
        (define-public clefs-added 0)
        (define-public (add-new-clef . clef) (set! clefs-added (+ clefs-added 1)))
        (add-new-clef \"moderntab\")
        (set! %parts-kept '(tiny))";
    assert!(vm.eval_text(kept, None).is_ok());
    let mut engine = engine_of(vm);
    let tiny = [
        (
            "<datadir>/scm/lily/tiny-a.scm",
            Some("(define-public (tiny-once x) x) (add-new-clef \"moderntab\")"),
        ),
        TINY[1],
    ];
    let score = "\\version \"2.26.0\"
#(ly:warning (tiny-twice \"x\"))
#(ly:warning (number->string clefs-added))
";
    engine.put(FILE, "a.ly", Some(score.as_bytes()));
    answer(&mut engine, "a.ly", &tiny);
    assert_eq!(stderr(&engine), "warning: xx\nwarning: 1\n");
}

/// The names a stop asked for come one a step, with no parse between,
/// whether the host starts the compile again after each answer or only
/// steps; a part's file the host already gave is not asked for. A compile
/// of another revision starts afresh, and asks for what it lacks itself.
#[test]
fn a_stops_names_come_one_a_step_until_the_host_has_put_them_all() {
    let score = "\\version \"2.26.0\"\n#(ly:warning (tiny-twice \"x\"))\n";
    let [(a, a_text), (b, b_text), _] = TINY;
    let stop = |engine: &mut Engine| {
        engine.put(FILE, "a.ly", Some(score.as_bytes()));
        engine.compile(1, "a.ly");
        assert_eq!(engine.step(10.0), NEEDS);
        assert_eq!(engine.needs(), Some((FILE, a)));
        assert_eq!(engine.parses(), 1);
    };

    // Started again after each answer, as the studio's driver does.
    let mut engine = engine_of(parts_vm());
    stop(&mut engine);
    engine.put(FILE, a, a_text.map(str::as_bytes));
    engine.compile(1, "a.ly");
    assert_eq!(engine.step(10.0), NEEDS);
    assert_eq!(engine.needs(), Some((FILE, b)));
    assert_eq!(engine.parses(), 1, "no parse for the stop's second name");
    engine.put(FILE, b, b_text.map(str::as_bytes));
    engine.compile(1, "a.ly");
    assert_eq!(engine.step(10.0), DONE);
    assert_eq!(engine.parses(), 2);
    assert_eq!(stderr(&engine), "warning: xx\n");

    // Only stepped: the same, and a name not answered is asked again.
    let mut engine = engine_of(parts_vm());
    stop(&mut engine);
    assert_eq!(engine.step(10.0), NEEDS);
    assert_eq!(engine.needs(), Some((FILE, a)));
    engine.put(FILE, a, a_text.map(str::as_bytes));
    engine.put(FILE, b, b_text.map(str::as_bytes));
    assert_eq!(engine.step(10.0), DONE);
    assert_eq!(
        (engine.parses(), stderr(&engine)),
        (2, "warning: xx\n".into())
    );

    // A file given before the stop is not asked for.
    let mut engine = engine_of(parts_vm());
    engine.put(FILE, a, a_text.map(str::as_bytes));
    engine.put(FILE, "a.ly", Some(score.as_bytes()));
    assert_eq!(answer(&mut engine, "a.ly", &TINY), [b]);
    assert_eq!(engine.parses(), 2);

    // Another revision drops what the stopped compile asked for.
    let mut engine = engine_of(parts_vm());
    stop(&mut engine);
    engine.put(FILE, "a.ly", Some(b"\\version \"2.26.0\"\n{ c4 }\n"));
    engine.compile(2, "a.ly");
    assert_eq!(engine.step(10.0), DONE);
    assert_eq!((engine.parses(), stderr(&engine)), (1, String::new()));
}

/// A part's own module loads the part when a file uses it, however often
/// the compile stops before it has its files.
#[test]
fn a_parts_module_loads_the_part_when_a_file_uses_it() {
    let mut engine = engine_of(parts_vm());
    let score = "\\version \"2.26.0\"
#(use-modules (lily tiny))
#(ly:warning (tiny-hello))
";
    engine.put(FILE, "a.ly", Some(score.as_bytes()));
    let asked = answer(&mut engine, "a.ly", &TINY);
    assert_eq!(asked, ["<datadir>/scm/lily/tiny-mod.scm"]);
    assert_eq!(stderr(&engine), "warning: hello\n");
}

/// A part's file the host does not have is D76's error.
#[test]
fn a_parts_file_the_host_does_not_have_fails_the_call() {
    let mut engine = engine_of(parts_vm());
    let score = "\\version \"2.26.0\"\n#(tiny-twice \"x\")\n";
    engine.put(FILE, "a.ly", Some(score.as_bytes()));
    answer(&mut engine, "a.ly", &[]);
    let stderr = stderr(&engine);
    assert!(
        stderr.contains("needs tiny-a.scm, which the host has not given"),
        "{stderr}"
    );
}

/// `$LILY_CORPUS/corpus.tsv` (by default `~/.cache/lily-engine`): each
/// score's id and path.
fn corpus() -> Option<Vec<(String, PathBuf)>> {
    let root = std::env::var_os("LILY_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache/lily-engine")
        });
    let tsv = std::fs::read_to_string(root.join("corpus.tsv")).ok()?;
    Some(
        tsv.lines()
            .filter_map(|line| line.split_once('\t'))
            .map(|(id, path)| (id.into(), PathBuf::from(path)))
            .collect(),
    )
}

/// Steps a compile of `root` to its end, answering each name from disk as
/// `examples/compile.rs` does (one under `<datadir>/` from `datadir`, a
/// font pack from the build's `dist/engine/fonts/`, W5, a part's module
/// from `datadir`'s `scm/lily/`, I2); the files and the parts' modules
/// asked, each with the parses the compile had made when it asked.
fn answer_from_disk(engine: &mut Engine, root: &str, datadir: &Path) -> Vec<(String, u32)> {
    let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    let mut asked: Vec<(String, u32)> = Vec::new();
    engine.compile(1, root);
    while engine.step(10.0) == NEEDS {
        let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
        if kind == MODULE && PART_MODULES.iter().any(|(part, _)| *part == name) {
            engine.put(MODULE, &name, Some(&part_module(&name, datadir)));
            asked.push((name, engine.parses()));
            engine.compile(1, root);
            continue;
        }
        if kind == MODULE {
            let pack = std::fs::read(fonts.join(format!("{name}.pack"))).ok();
            engine.put(MODULE, &name, pack.as_deref());
            engine.compile(1, root);
            continue;
        }
        assert_eq!(kind, FILE);
        assert!(
            !asked.iter().any(|(n, _)| *n == name),
            "{name} is asked for twice"
        );
        let path = match name.strip_prefix(DATADIR) {
            Some(rest) => datadir.join(rest.trim_start_matches('/')),
            None => PathBuf::from(&name),
        };
        engine.put(FILE, &name, std::fs::read(path).ok().as_deref());
        asked.push((name, engine.parses()));
        engine.compile(1, root);
    }
    asked
}

/// The module of one of [`PART_MODULES`], its files from `datadir`'s
/// `scm/lily/`, as the build writes it.
fn part_module(part: &str, datadir: &Path) -> Vec<u8> {
    let (_, files) = PART_MODULES.iter().find(|(p, _)| *p == part).unwrap();
    let texts: Vec<Vec<u8>> = files
        .iter()
        .map(|file| std::fs::read(datadir.join("scm/lily").join(file)).unwrap())
        .collect();
    let named: Vec<(&str, &[u8])> = files
        .iter()
        .copied()
        .zip(texts.iter().map(Vec::as_slice))
        .collect();
    write_part(&named)
}

/// T05's `chords` part and T29's `diagrams` part, each of three files, are
/// asked for at one stop of a fresh engine from the installed LilyPond's
/// `(lily)`: two parses, where each file was a stop of its own until MS2
/// step 30 (four), with the preloaded packs; T29's three, its packs a stop
/// of their own since MS3 step 50 (W5). Since MS4 step 20 (I2) T05's part
/// is asked for as its module, `chords`, not as its three files, and since
/// MS7 step 7 (D263) T29's as `diagrams`. What the compile then writes is
/// what it writes with every file given before it starts.
#[test]
fn t05s_and_t29s_parts_are_asked_for_at_one_stop() {
    let (Some(lily), Some(corpus)) = (support::lily_scm(), corpus()) else {
        eprintln!("skipped: no LilyPond 2.26.0 installed, or no $LILY_CORPUS/corpus.tsv");
        return;
    };
    let mut vm = support::library_with(&lily, lily_engine_core::install).expect("(lily) loads");
    let scheme = vm.write_image().expect("the VM is at rest");
    let image = write_image(&scheme);
    let datadir = lily.join("../..");
    for (id, files) in [
        ("T05", vec![(String::from("chords"), 1)]),
        ("T29", vec![(String::from("diagrams"), 1)]),
    ] {
        let Some((_, path)) = corpus.iter().find(|(i, _)| i == id) else {
            eprintln!("skipped: {id} is not in corpus.tsv");
            continue;
        };
        let root = path.to_str().unwrap();
        let text = std::fs::read(path).expect("the score reads");
        // The packs a host preloads (D105); T29's staff size and braces
        // are one stop more, for their packs (W5, D208), and since MS4 step
        // 10 (its ties drawn) its bold italic text another (OPEN E22).
        let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine");
        let start = || {
            let mut engine = Engine::start(&image).expect("the image restores");
            for name in ["emmentaler-20", "C059-Roman", "C059-Bold", "C059-Italic"] {
                let pack = std::fs::read(dist.join(format!("{name}.pack"))).ok();
                engine.put(MODULE, name, pack.as_deref());
            }
            engine.put(FILE, root, Some(&text));
            engine
        };
        // Since MS7 step 5 (D262) the outlines a skyline needs are a stop
        // of their own too, asked at a compile's end: T05's Roman, T29's
        // Roman and Italic with C059-BdIta's face, then its outlines.
        let packs = if id == "T29" { 2 + 1 } else { 1 };
        let mut engine = start();
        assert_eq!(answer_from_disk(&mut engine, root, &datadir), files, "{id}");
        assert_eq!(engine.parses(), 2 + packs, "{id}");

        let mut given = start();
        for (name, _) in &files {
            let Some(rest) = name.strip_prefix(DATADIR) else {
                given.put(MODULE, name, Some(&part_module(name, &datadir)));
                continue;
            };
            let rest = rest.trim_start_matches('/');
            given.put(
                FILE,
                name,
                Some(&std::fs::read(datadir.join(rest)).unwrap()),
            );
        }
        assert!(
            answer_from_disk(&mut given, root, &datadir).is_empty(),
            "{id}"
        );
        assert_eq!(given.parses(), 1 + packs, "{id}");
        assert_eq!(given.outputs(), engine.outputs(), "{id}");
    }
}

/// W5 (MS3 step 50, D208): a score at staff size 13 asks for
/// `emmentaler-13` (`Need.Module`) after its translation, once, and with
/// the pack put has its page; put as none, the pack is a diagnostic and the
/// compile goes on to its playback map; put but refused by layout (no pack
/// of W1's), the compile is not cut short either: its `\midi` translation
/// and playback map are made. Skipped without LilyPond 2.26.0 or a build.
#[test]
fn a_pack_is_asked_for_once_and_one_that_cannot_be_given_ends_nothing_else() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine");
    let (Some(lily), true) = (
        support::lily_scm(),
        dist.join("fonts/emmentaler-13.pack").exists(),
    ) else {
        eprintln!("skipped: no LilyPond 2.26.0, or no build (npm run build)");
        return;
    };
    let mut vm = support::library_with(&lily, lily_engine_core::install).expect("(lily) loads");
    let scheme = vm.write_image().expect("the VM is at rest");
    let image = write_image(&scheme);
    let text = b"\\version \"2.26.0\"\n#(set-global-staff-size 13)\n{ c'4 d' e' f' }\n";
    let start = |thirteen: Option<&[u8]>| {
        let mut engine = Engine::start(&image).expect("the image restores");
        for name in ["emmentaler-20", "C059-Roman", "C059-Bold", "C059-Italic"] {
            let pack = std::fs::read(dist.join(format!("{name}.pack"))).ok();
            engine.put(MODULE, name, pack.as_deref());
        }
        if let Some(data) = thirteen {
            engine.put(MODULE, "emmentaler-13", Some(data));
        }
        engine.put(FILE, "small.ly", Some(text));
        engine.compile(1, "small.ly");
        engine
    };
    let names = |engine: &Engine| {
        engine
            .outputs()
            .iter()
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>()
    };
    let said = |engine: &Engine| String::from_utf8_lossy(&engine.outputs()[0].1).into_owned();

    let pack = std::fs::read(dist.join("fonts/emmentaler-13.pack")).unwrap();
    let mut online = start(None);
    assert_eq!(online.step(10.0), NEEDS);
    assert_eq!(online.needs(), Some((MODULE, "emmentaler-13")));
    online.put(MODULE, "emmentaler-13", Some(&pack));
    online.compile(1, "small.ly");
    assert_eq!(online.step(10.0), DONE);
    assert_eq!(
        names(&online),
        [
            "stderr",
            "diagnostics.json",
            "small.svg",
            "small.midi",
            "small.timing.json"
        ]
    );
    assert_eq!(said(&online), "");

    let mut offline = start(None);
    assert_eq!(offline.step(10.0), NEEDS);
    offline.put(MODULE, "emmentaler-13", None);
    offline.compile(1, "small.ly");
    assert_eq!(offline.step(10.0), DONE);
    assert_eq!(
        names(&offline),
        [
            "stderr",
            "diagnostics.json",
            "small.midi",
            "small.timing.json"
        ]
    );
    assert_eq!(
        said(&offline),
        "error: cannot find font 'emmentaler-13' (its font pack emmentaler-13 could not be fetched)\n"
    );

    let mut refused = start(Some(b"not a pack"));
    assert_eq!(refused.step(10.0), DONE, "nothing is asked for again");
    assert_eq!(
        names(&refused),
        [
            "stderr",
            "diagnostics.json",
            "small.midi",
            "small.timing.json"
        ]
    );
    assert_eq!(
        refused.outputs()[2].1,
        offline.outputs()[2].1,
        "the playback map"
    );
}

/// `ly:expect-warning` (D96): a warning, an error or a programming error
/// whose message starts with what an earlier call expects is not printed,
/// each call meeting one, the first it matches; one given before its call
/// is printed, and the call is left unmet. Those none met are LilyPond's
/// warning at the end of the compile, before `failed files:` (an expected
/// error still fails the file), traced or not, since every compile
/// translates its `\layout`s (MS2 step 40, D99).
#[test]
fn expected_warnings_are_not_printed_and_the_missing_ones_are_warned_of_at_the_end() {
    let text = "\\version \"2.26.0\"
#(ly:warning \"early\")
#(ly:expect-warning \"early\")
#(ly:expect-warning \"twice ~a\" 1)
#(ly:expect-warning \"twice\")
#(ly:warning \"twice 1 and more\")
#(ly:warning \"twice 2\")
#(ly:warning \"twice 3\")
#(ly:expect-warning \"oops\")
#(ly:programming-error \"oops\")
#(ly:expect-warning \"never\")
#(ly:expect-warning \"syntax error\")
{ c4 } }
";
    let mut engine = engine();
    compile(&mut engine, "/s/e.ly", text);
    let failed = "fatal error: failed files: \"/s/e.ly\"\n";
    let printed = "warning: early\nwarning: twice 3\n";
    let unmet = "warning: 2 expected warning(s) not encountered: \n        early\n        never\n";
    let (stderr, json) = outputs(&engine);
    assert_eq!(stderr, format!("{printed}{unmet}{failed}"));
    assert!(
        !json.contains("twice 1") && !json.contains("syntax"),
        "{json}"
    );

    engine.set_trace(true);
    compile(&mut engine, "/s/e.ly", text);
    let stderr = engine
        .outputs()
        .iter()
        .find(|(name, _)| name == "stderr")
        .map(|(_, text)| String::from_utf8_lossy(text).into_owned());
    assert_eq!(
        stderr.as_deref(),
        Some(format!("{printed}{unmet}{failed}").as_str())
    );
}

/// A font pack of W1's made by hand (`crates/layout/tests/fonts.rs`): the
/// font `test`, PostScript name `Test`, its glyphs `a` and `b`.
const PACK: &[u8] = b"LILP\x03\x04test\x04Test\x00\xe8\x07\x03\x01\x0bdesign_size\x01\x90\x03\x02\x01a\x01b\xf4\x03\x00\xc3\x01\x00\x09\x01\xeb\x30\x98\x75\x00\xc1\x3e\x90\x4e\x00\x00\x06\x00\x00\x07\x01\x02\x02\x00\x00\x00";

/// MS3 step 5: a module that is a font pack is given to layout's fonts,
/// once however often the host puts it, and a module that is none is only
/// kept; the fonts a compile loads are forgotten after it, as
/// `lilypond-all` forgets them after each file.
#[test]
fn a_font_pack_put_as_a_module_is_read_and_its_fonts_forgotten_after_each_compile() {
    let mut vm = vm();
    assert!(vm.eval_text("(define-public probe-font #f)", None).is_ok());
    let mut engine = engine_of(vm);
    engine.put(MODULE, "parts", Some(b"no pack"));
    engine.put(MODULE, "test", Some(PACK));
    engine.put(MODULE, "test", Some(PACK));
    assert_eq!(engine.module("parts"), Some(&b"no pack"[..]));
    assert_eq!(engine.module("test"), Some(PACK));
    compile(
        &mut engine,
        "/s/a.ly",
        "\\version \"2.26.0\"
#(set! probe-font (ly:system-font-load \"test\"))
#(ly:warning \"~a ~a ~a\" (ly:font-name probe-font) (ly:otf-glyph-list probe-font) (eq? probe-font (ly:system-font-load \"test\")))",
    );
    assert_eq!(stderr(&engine), "warning: Test (a b) #t\n");
    compile(
        &mut engine,
        "/s/b.ly",
        "\\version \"2.26.0\"
#(ly:warning \"~a\" (eq? probe-font (ly:system-font-load \"test\")))",
    );
    assert_eq!(stderr(&engine), "warning: #f\n");
}

/// MS3 step 35: a `\layout` compile writes its books' pages, from the
/// installed LilyPond's `(lily)` with the build's packs
/// (`dist/engine/`): path.ly's one page as `path.svg`, after `stderr` and
/// `diagnostics.json` and before the playback map, its notes linked by
/// their places and the tagline at its foot; three books, the first and
/// the last of two parts, named as LilyPond 2.26.0 names them (`-1` the
/// second book's page over the first's page 1, as LilyPond writes it
/// over); and no page of a book whose layout failed (a stencil set to
/// a procedure that calls `ly:music-iterator::constructor`, a placeholder
/// no step ports; the Kievan ligature's print was one until MS8 step 9, a
/// balloon's print until step 8, a clef's until MS3 step 40, a
/// hairpin's print until step 49 and a tie column's `before-line-breaking`
/// until MS4 step 10, and since then a staff of one note, of a hairpin and
/// of a tied chord has its page), unreported (D130).
/// Since step 36 (D204) path.ly's layout is timed apart from the writer,
/// its 43 live grobs counted as its `layout.tsv` has them.
/// Skipped without LilyPond 2.26.0 or the build, and then unverified.
#[test]
fn a_layout_compile_writes_its_books_pages_as_svg() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let packs = ["emmentaler-20", "C059-Roman"].map(|name| {
        std::fs::read(root.join(format!("dist/engine/{name}.pack"))).map(|data| (name, data))
    });
    let Ok(packs) = packs.into_iter().collect::<Result<Vec<_>, _>>() else {
        eprintln!("skipped: no dist/engine packs (npm run build)");
        return;
    };
    let mut engine = engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
    for (name, data) in &packs {
        engine.put(MODULE, name, Some(data));
    }
    let datadir = lily.join("../..");
    let names = |engine: &Engine| -> Vec<String> {
        engine.outputs().iter().map(|(n, _)| n.clone()).collect()
    };
    let output = |engine: &Engine, name: &str| -> String {
        let (_, data) = engine.outputs().iter().find(|(n, _)| n == name).unwrap();
        String::from_utf8(data.clone()).unwrap()
    };

    let path = std::fs::read(root.join("tools/compare/fixtures/path.ly")).unwrap();
    engine.put(FILE, "path.ly", Some(&path));
    answer_from_disk(&mut engine, "path.ly", &datadir);
    assert_eq!(
        names(&engine),
        [
            "stderr",
            "diagnostics.json",
            "path.svg",
            "path.midi",
            "path.timing.json"
        ]
    );
    assert_eq!(output(&engine, "stderr"), "");
    let svg = output(&engine, "path.svg");
    assert!(svg.starts_with("<svg "), "{svg}");
    assert!(svg.contains("textedit:///path.ly:16:4:5"), "{svg}");
    assert!(svg.contains("<tspan>LilyPond v2.26.0</tspan>"), "{svg}");
    let times = engine.times();
    assert_eq!(times.live_grobs, 43);
    assert!(
        0.0 < times.lay_out_ms && times.lay_out_ms < times.translate_ms,
        "{times:?}"
    );
    assert!(times.stencils_ms > 0.0 && times.write_ms > 0.0, "{times:?}");

    let layout = "\\layout {
  \\context { \\Staff \\remove Clef_engraver \\remove Key_engraver \\remove Ledger_line_engraver }
  \\context { \\Score \\remove System_start_delimiter_engraver \\remove Bar_number_engraver }
}
skips = \\new Staff { \\time 3/4 s2. | s4 s2 \\bar \"|.\" }
";
    let books = format!(
        "\\version \"2.26.0\"
{layout}
\\book {{ \\bookpart {{ \\score {{ \\skips }} }} \\bookpart {{ \\score {{ \\skips }} }} }}
\\book {{ \\score {{ \\skips }} }}
\\book {{ \\bookpart {{ \\score {{ \\skips }} }} \\bookpart {{ \\score {{ \\skips }} }} }}
"
    );
    engine.put(FILE, "books.ly", Some(books.as_bytes()));
    answer_from_disk(&mut engine, "books.ly", &datadir);
    assert_eq!(
        names(&engine),
        [
            "stderr",
            "diagnostics.json",
            "books-1.svg",
            "books-2.svg",
            "books-2-1.svg",
            "books-2-2.svg",
            "books.midi",
            "books-1.midi",
            "books-2.midi",
            "books-2-1.midi",
            "books.timing.json"
        ]
    );
    assert_eq!(output(&engine, "stderr"), "");
    // The second book's only page, the last of its book, has the tagline;
    // the first book's first page, which it replaced, has not.
    assert!(output(&engine, "books-1.svg").contains("<tspan>LilyPond v2.26.0</tspan>"));
    for name in ["books-2.svg", "books-2-2.svg"] {
        assert!(output(&engine, name).contains("<tspan>2</tspan>"), "{name}");
    }
    assert!(!output(&engine, "books-2-1.svg").contains("<tspan>"));

    engine.put(FILE, "clef.ly", Some(b"\\version \"2.26.0\"\n{ c'4 }\n"));
    answer_from_disk(&mut engine, "clef.ly", &datadir);
    assert_eq!(
        names(&engine),
        [
            "stderr",
            "diagnostics.json",
            "clef.svg",
            "clef.midi",
            "clef.timing.json"
        ]
    );
    assert_eq!(output(&engine, "stderr"), "");

    engine.put(
        FILE,
        "hairpin.ly",
        Some(b"\\version \"2.26.0\"\n{ c'4\\< c'\\! }\n"),
    );
    answer_from_disk(&mut engine, "hairpin.ly", &datadir);
    assert_eq!(
        names(&engine),
        [
            "stderr",
            "diagnostics.json",
            "hairpin.svg",
            "hairpin.midi",
            "hairpin.timing.json"
        ]
    );
    assert_eq!(output(&engine, "stderr"), "");

    engine.put(
        FILE,
        "ties.ly",
        Some(b"\\version \"2.26.0\"\n{ <c' e'>2~ q }\n"),
    );
    answer_from_disk(&mut engine, "ties.ly", &datadir);
    assert_eq!(
        names(&engine),
        [
            "stderr",
            "diagnostics.json",
            "ties.svg",
            "ties.midi",
            "ties.timing.json"
        ]
    );
    assert_eq!(output(&engine, "stderr"), "");

    engine.put(
        FILE,
        "placeholder.ly",
        Some(b"\\version \"2.26.0\"\n{ \\override NoteHead.stencil = #(lambda (g) (ly:music-iterator::constructor)) c'4 }\n"),
    );
    answer_from_disk(&mut engine, "placeholder.ly", &datadir);
    assert_eq!(
        names(&engine),
        [
            "stderr",
            "diagnostics.json",
            "placeholder.midi",
            "placeholder.timing.json"
        ]
    );
    assert_eq!(output(&engine, "stderr"), "");
}

/// MS3 step 35 (D132): an error of a translation ends the run as it ends
/// LilyPond's, reported at `init.ly` for the book `init.ly` makes, after
/// what the translation said: a Scheme engraver's in `process-music` and
/// in `finalize` (`tools/compare/fixtures`, which `c0-cases.test.mjs`
/// compares with LilyPond's stderr), with no translation, page or playback
/// map after it; a `\book` block's as Guile's text, without its frame; and
/// a placeholder's call, the engine's own, unreported, the compile going
/// on. Skipped without LilyPond 2.26.0, and then unverified.
#[test]
fn a_translations_error_ends_the_run_as_lilyponds() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let mut engine = engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
    let datadir = lily.join("../..");
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/compare/fixtures");
    let at_init = "<datadir>/ly/init.ly:66:2: error: Guile signaled an error for the expression beginning here\n\
                   #\n \
                   (let ((book-handler (if (defined? 'default-toplevel-book-handler)\n";
    let car = "In procedure car: Wrong type argument in position 1 (expecting pair): ()\n";
    let mut run = |name: &str, text: &[u8]| {
        engine.put(FILE, name, Some(text));
        answer_from_disk(&mut engine, name, &datadir);
        outputs(&engine)
    };

    let text = std::fs::read(fixtures.join("translation-error.ly")).unwrap();
    let (stderr, json) = run("translation-error.ly", &text);
    assert_eq!(
        stderr,
        format!("warning: before the translations\nwarning: in the translation\n{at_init}{car}")
    );
    assert!(
        json.ends_with(
            r#"{"file":"<datadir>/ly/init.ly","line":66,"column":2,"severity":"error","message":"Guile signaled an error for the expression beginning here\nIn procedure car: Wrong type argument in position 1 (expecting pair): ()"}]"#
        ),
        "{json}"
    );

    let text = std::fs::read(fixtures.join("finalize-error.ly")).unwrap();
    let (stderr, _) = run("finalize-error.ly", &text);
    assert_eq!(stderr, format!("{at_init}the engraver's finalize\n"));

    let engraver = |call: &str| {
        format!(
            "\\layout {{ \\context {{ \\Voice \\consists #`((process-music . ,(lambda (e) {call}))) }} }}"
        )
    };
    let book = format!(
        "\\version \"2.26.0\"\n\\book {{ \\score {{ {{ c'4 }} {} }} }}\n{{ d'4 }}\n",
        engraver("(car '())")
    );
    let (stderr, json) = run("book.ly", book.as_bytes());
    assert_eq!((stderr.as_str(), json.as_str()), (car, "[]"));

    let placeholder = format!(
        "\\version \"2.26.0\"\n{}\n{{ c'4 }}\n",
        engraver("(ly:music-iterator::constructor)")
    );
    engine.put(FILE, "placeholder.ly", Some(placeholder.as_bytes()));
    answer_from_disk(&mut engine, "placeholder.ly", &datadir);
    let names: Vec<&str> = engine.outputs().iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "stderr",
            "diagnostics.json",
            "placeholder.midi",
            "placeholder.timing.json"
        ]
    );
    assert_eq!(engine.outputs()[0].1, b"");
}

/// OPEN B33 (MS8 step 11a): an error while a `\layout`'s Paper_score is
/// processed (a grob's `before-line-breaking`) ends the run as a
/// translation's does, as LilyPond's ends it, where the book was left
/// without its pages and nothing said: untraced (`Processing::on`'s error)
/// and traced (`dump::lay_out`'s read); a book before it keeps its pages, and
/// an error of the engine's own (a placeholder's call) is still unreported.
/// Skipped without LilyPond 2.26.0, and then unverified.
#[test]
fn a_layouts_error_ends_the_run_as_lilyponds() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let datadir = lily.join("../..");
    let at_init = "<datadir>/ly/init.ly:66:2: error: Guile signaled an error for the expression beginning here\n\
                   #\n \
                   (let ((book-handler (if (defined? 'default-toplevel-book-handler)\n";
    let car = "In procedure car: Wrong type argument in position 1 (expecting pair): ()\n";
    let breaking =
        |call: &str| format!("\\override NoteHead.before-line-breaking = #(lambda (g) {call})");
    let toplevel = format!(
        "\\version \"2.26.0\"\n\\header {{ title = \"B33\" }}\n{{ {} c'4 d'4 }}\n",
        breaking("(car '())")
    );
    let books = format!(
        "\\version \"2.26.0\"\n\\book {{ {{ c'4 }} }}\n\\book {{ {{ {} d'4 }} }}\n",
        breaking("(car '())")
    );
    let own = format!(
        "\\version \"2.26.0\"\n{{ {} c'4 }}\n",
        breaking("(ly:music-iterator::constructor)")
    );
    for traced in [false, true] {
        let mut engine =
            engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
        engine.set_trace(traced);
        let mut run = |name: &str, text: &str| {
            engine.put(FILE, name, Some(text.as_bytes()));
            answer_from_disk(&mut engine, name, &datadir);
            let outputs = engine.outputs();
            let pages: Vec<String> = outputs
                .iter()
                .map(|(n, _)| n.clone())
                .filter(|n| n.ends_with(".svg"))
                .collect();
            let said = outputs
                .iter()
                .find(|(n, _)| n == "stderr")
                .map(|(_, b)| b.clone());
            (String::from_utf8(said.unwrap_or_default()).unwrap(), pages)
        };
        let (said, pages) = run("b33.ly", &toplevel);
        assert_eq!(said, format!("{at_init}{car}"), "traced {traced}");
        assert!(pages.is_empty(), "traced {traced}: {pages:?}");
        // A `\book` block's: Guile's text without its frame (D132).
        let (said, pages) = run("books.ly", &books);
        assert_eq!(said, car, "traced {traced}");
        assert_eq!(pages, ["books.svg"], "traced {traced}");
        let (said, pages) = run("own.ly", &own);
        assert_eq!(said, "", "traced {traced}");
        assert!(pages.is_empty(), "traced {traced}: {pages:?}");
    }
}

/// MS3 step 55 (D213): layout's `Apply_output_engraver` in the image's
/// place calls each `\applyOutput` procedure with the grobs, contexts and
/// in the order LilyPond 2.26.0's Scheme engraver does (its stderr).
#[test]
fn apply_output_s_procedures_see_what_lilyponds_do() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let mut engine = engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
    let datadir = lily.join("../..");
    let text = "\\version \"2.26.0\"
#(define (show g c ctx)
   (ly:warning \"~a ~a ~a\" (grob::name g) (ly:context-name c) (ly:context-name ctx)))
\\new Staff {
  c'4 \\applyOutput Voice.NoteHead #show d'4 \\applyOutput Staff #show <e' g'>4-.
  \\applyOutput Voice #show f'4 \\applyOutput Score.Stem #show g'4 a'4
}
";
    engine.put(FILE, "apply-output.ly", Some(text.as_bytes()));
    answer_from_disk(&mut engine, "apply-output.ly", &datadir);
    let lines = [
        "NoteHead Voice Voice",
        "NoteHead Voice Staff",
        "NoteHead Voice Staff",
        "Script Voice Staff",
        "Stem Voice Staff",
        "StemStub Voice Staff",
        "NoteSpacing Voice Staff",
        "NoteColumn Voice Staff",
        "NoteHead Voice Voice",
        "Stem Voice Voice",
        "StemStub Voice Voice",
        "NoteSpacing Voice Voice",
        "NoteColumn Voice Voice",
        "Stem Voice Score",
    ];
    let expected: String = lines.iter().map(|l| format!("warning: {l}\n")).collect();
    let (name, stderr) = &engine.outputs()[0];
    assert_eq!(name, "stderr");
    assert_eq!(String::from_utf8_lossy(stderr), expected);
}

/// 02 §5.7's re-parse (MS6 step 3): a compile after an edit does again the
/// parse's segments that end before the first byte the edit changed, from
/// what undoing them kept, and parses from there; its outputs are a fresh
/// engine's, and what the kept items did to Scheme is there for the items
/// after them. An edit before them all, or of a file they included, parses
/// from the start.
#[test]
fn a_parse_after_an_edit_does_the_items_before_it_again() {
    let include = "partB = { d4 }\n";
    let score = |last: &str| {
        format!(
            "\\version \"2.26.0\"
#(define-public kept-count 0)
#(set! kept-count (1+ kept-count))
#(ly:set-option 'point-and-click #f)
\\include \"b.ily\"
partA = {{ c4 d }}
\\score {{ {{ \\partA \\partB }} }}
#(ly:warning (format #f \"~a ~a\" kept-count (ly:get-option 'point-and-click)))
{last}
"
        )
    };
    let fresh_outputs = |text: &str, b: &str| {
        let mut fresh = engine();
        fresh.put(FILE, "/s/b.ily", Some(b.as_bytes()));
        fresh.put(FILE, "<datadir>/ly/b.ily", None);
        compile(&mut fresh, "/s/a.ly", text);
        (outputs(&fresh), fresh.reused())
    };
    let mut engine = engine();
    engine.put(FILE, "/s/b.ily", Some(include.as_bytes()));
    engine.put(FILE, "<datadir>/ly/b.ily", None);
    compile(&mut engine, "/s/a.ly", &score("{ c4 }"));
    let (_, (reused, parsed)) = fresh_outputs(&score("{ c4 }"), include);
    assert_eq!((reused, engine.reused().0), (0, 0));
    for last in ["{ d4 }", "{ d4 \\oops }", "{ c2 }"] {
        compile(&mut engine, "/s/a.ly", &score(last));
        let (kept, again) = engine.reused();
        assert!(kept > 0 && kept + again == parsed, "{last}: {kept} {again}");
        assert_eq!(
            outputs(&engine),
            fresh_outputs(&score(last), include).0,
            "{last}"
        );
        assert!(
            stderr(&engine).starts_with("warning: 1 #f\n"),
            "{}",
            stderr(&engine)
        );
    }
    // An edit at the top parses it all.
    let top = score("{ c2 }").replacen("kept-count 0", "kept-count 10", 1);
    compile(&mut engine, "/s/a.ly", &top);
    assert_eq!(engine.reused(), (0, parsed));
    assert!(stderr(&engine).starts_with("warning: 11 #f\n"));
    // The file the kept items included, edited.
    compile(&mut engine, "/s/a.ly", &score("{ c2 }"));
    assert_eq!(engine.reused().0, 0);
    compile(&mut engine, "/s/a.ly", &score("{ d2 }"));
    assert!(engine.reused().0 > 0);
    engine.put(FILE, "/s/b.ily", Some(b"partB = { d4 \\oops }\n"));
    compile(&mut engine, "/s/a.ly", &score("{ d2 }"));
    assert_eq!(engine.reused().0, 0);
    let fresh = fresh_outputs(&score("{ d2 }"), "partB = { d4 \\oops }\n").0;
    assert!(
        fresh.0.contains("b.ily:1:14: error: unknown command"),
        "{}",
        fresh.0
    );
    assert_eq!(outputs(&engine), fresh);
}

/// A compile that stops for an include parses again after the host's
/// answer from the boundary before it, the segments before it done again:
/// the two Scheme items, not the assignment, whose lookahead the include
/// stopped.
#[test]
fn a_parse_after_a_stop_starts_at_the_boundary_before_it() {
    let score = "\\version \"2.26.0\"
#(define-public stop-count 0)
#(set! stop-count (1+ stop-count))
partA = { c4 d }
\\include \"b.ily\"
#(ly:warning (format #f \"~a\" stop-count))
{ \\partA \\partB }
";
    let mut engine = engine();
    engine.put(FILE, "/s/a.ly", Some(score.as_bytes()));
    let asked = answer(
        &mut engine,
        "/s/a.ly",
        &[("/s/b.ily", Some("partB = { d4 }\n"))],
    );
    assert_eq!(asked, ["/s/b.ily", "<datadir>/ly/b.ily"]);
    assert_eq!(engine.parses(), 2);
    assert_eq!(engine.reused().0, 2);
    assert_eq!(stderr(&engine), "warning: 1\n");
}

/// OPEN F's include gaps (MS6 step 3): an `\include` read again in the
/// `incl` state is looked up and found Guile's `include` macro, as
/// LilyPond prints it; a template's `\include` reads the host's files,
/// asked for at stops as a file's are, beside the file the template is in.
#[test]
fn an_include_in_a_template_reads_the_hosts_files() {
    let mut incl = engine();
    incl.put(
        FILE,
        "i.ly",
        Some(b"\\version \"2.26.0\"\n\\include \\include \"x.ly\"\n"),
    );
    answer(&mut incl, "i.ly", &[("x.ly", Some("% x\n"))]);
    assert_eq!(
        stderr(&incl),
        "i.ly:2:10: error: wrong or undefined identifier: `include'\n\
         \\include \n         \\include \"x.ly\"\n\
         This value was found in the table: #<syntax-transformer include>\
         fatal error: failed files: \"i.ly\"\n"
    );
    let score = "\\version \"2.26.0\"
m = #(define-music-function () () #{ \\include \"x.ly\" #})
n = #(define-music-function () () #{ \\include \"nope.ly\" #})
\\score { { \\m \\n } }
";
    // Templates need `parser-ly-from-scheme.scm`: LilyPond's library.
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let vm = support::library_with(&lily, lily_engine_core::install).expect("(lily) loads");
    let mut engine = engine_of(vm);
    engine.put(FILE, "/s/sub/a.ly", Some(score.as_bytes()));
    // The files it asks for, the font packs it asks for put as none.
    let mut asked = Vec::new();
    engine.compile(1, "/s/sub/a.ly");
    while engine.step(10.0) == NEEDS {
        let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
        let text = (name == "/s/sub/x.ly").then_some(&b"{ d4 }"[..]);
        engine.put(kind, &name, text);
        if kind == FILE {
            asked.push(name);
        }
        engine.compile(1, "/s/sub/a.ly");
    }
    assert_eq!(
        asked,
        [
            "/s/sub/x.ly",
            "<datadir>/ly/x.ly",
            "/s/sub/nope.ly",
            "<datadir>/ly/nope.ly"
        ]
    );
    let stderr = engine
        .outputs()
        .iter()
        .find(|(name, _)| name == "stderr")
        .map(|(_, text)| String::from_utf8_lossy(text).into_owned())
        .unwrap_or_default();
    assert!(
        stderr.starts_with("/s/sub/a.ly:3:11: error: cannot find file: `nope.ly'\n"),
        "{stderr}"
    );
    assert!(!stderr.contains("x.ly"), "{stderr}");
}

/// A boundary is where a parse started again lexes the lookahead alike:
/// an assignment of music defines after its lookahead is lexed, so `\y`
/// after `y = { c4 }` is the `y` before it, a number, which no toplevel
/// item starts with, in a parse after an edit as in a fresh one.
#[test]
fn a_parse_after_an_edit_lexes_the_lookahead_as_the_first_parse() {
    let score = |last: &str| format!("\\version \"2.26.0\"\ny = #1\ny = {{ c4 }}\n\\y\n{last}\n");
    let mut engine = engine();
    compile(&mut engine, "/s/a.ly", &score("{ d4 }"));
    let first = stderr(&engine);
    assert!(first.contains("unexpected NUMBER_IDENTIFIER"), "{first}");
    compile(&mut engine, "/s/a.ly", &score("{ c4 }"));
    let mut fresh = self::engine();
    compile(&mut fresh, "/s/a.ly", &score("{ c4 }"));
    assert_eq!(outputs(&engine), outputs(&fresh));
    assert!(engine.reused().0 > 0, "{:?}", engine.reused());
}

/// What a translation changes of music a kept segment made is undone with
/// the compile (D235): the next compile's translation reads the music as
/// its parse made it. Skipped without LilyPond 2.26.0 (`\applyContext`).
#[test]
fn a_translations_change_of_kept_music_is_undone() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let score = |last: &str| {
        format!(
            "\\version \"2.26.0\"
m = {{ c4 }}
note = #(car (ly:music-property m 'elements))
\\score {{ {{ \\applyContext #(lambda (context)
  (ly:warning (format #f \"seen ~a\" (ly:music-property note 'seen)))
  (ly:music-set-property! note 'seen #t)) \\m }} }}
{last}
"
        )
    };
    let vm = support::library_with(&lily, lily_engine_core::install).expect("(lily) loads");
    let mut engine = engine_of(vm);
    // The font packs it asks for put as none; its stderr.
    let mut run = |text: &str| {
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        engine.compile(1, "/s/a.ly");
        while engine.step(10.0) == NEEDS {
            let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
            engine.put(kind, &name, None);
            engine.compile(1, "/s/a.ly");
        }
        let stderr = engine
            .outputs()
            .iter()
            .find(|(name, _)| name == "stderr")
            .map(|(_, text)| String::from_utf8_lossy(text).into_owned())
            .unwrap_or_default();
        (stderr, engine.reused())
    };
    for last in ["{ c4 }", "{ d4 }", "{ c2 }"] {
        let (stderr, (reused, _)) = run(&score(last));
        // The engine runs the procedure twice (OPEN C8): the first call
        // sees the music as the parse made it.
        assert!(stderr.starts_with("warning: seen ()\n"), "{last}: {stderr}");
        assert!(last == "{ c4 }" || reused > 0, "{last}: {reused}");
    }
}

/// 02 §5.7's step 4: after an edit that moves no byte or line, the old
/// segments past the edit's line are taken over, not parsed, while each
/// read what it read at the versions it read; an item that reads the edited
/// variable is parsed again. Outputs are a fresh engine's, the taken-over
/// segments' messages among them.
#[test]
fn a_parse_after_an_edit_takes_over_the_items_after_it() {
    let score = |a: &str| {
        format!(
            "\\version \"2.26.0\"
a = {{ {a} }}
b = {{ d4 }}
#(ly:warning \"b\")
\\score {{ \\a }}
c = {{ c2 }}
#(ly:warning \"c\")
"
        )
    };
    let fresh = |text: &str| {
        let mut fresh = engine();
        compile(&mut fresh, "/s/a.ly", text);
        outputs(&fresh)
    };
    let mut engine = engine();
    compile(&mut engine, "/s/a.ly", &score("c4"));
    let parsed = engine.reused().1;
    // Same length: `b`, its warning, `c` and its warning taken over.
    compile(&mut engine, "/s/a.ly", &score("d4"));
    assert_eq!(outputs(&engine), fresh(&score("d4")));
    let (again, now) = engine.reused();
    assert!(
        again >= 4 && again + now == parsed,
        "{again} {now} of {parsed}"
    );
    // Longer: nothing after the edit taken over.
    compile(&mut engine, "/s/a.ly", &score("d4 c"));
    assert_eq!(outputs(&engine), fresh(&score("d4 c")));
    assert!(engine.reused().0 < again, "{:?}", engine.reused());
}

/// What keeps an old segment from being taken over: an object a segment
/// parsed again changed and did not give back (a table it fills), which
/// the old one may read; and a variable written that the old parse did not
/// write (a name edited into another), which the old one may have looked
/// for. Each parse's outputs are a fresh engine's.
#[test]
fn a_segment_reading_what_the_edit_changed_is_parsed_again() {
    let table = |n: &str| {
        format!(
            "\\version \"2.26.0\"
#(define-public tbl (make-hash-table))
a = #(begin (hashq-set! tbl 'k {n}) 1)
#(ly:warning (format #f \"~a\" (hashq-ref tbl 'k)))
"
        )
    };
    let renamed = |name: &str| {
        format!(
            "\\version \"2.26.0\"
{name} = {{ c4 }}
\\score {{ \\xb }}
"
        )
    };
    for (first, second) in [(table("1"), table("2")), (renamed("xa"), renamed("xb"))] {
        let mut engine = engine();
        compile(&mut engine, "/s/a.ly", &first);
        compile(&mut engine, "/s/a.ly", &second);
        let mut fresh = self::engine();
        compile(&mut fresh, "/s/a.ly", &second);
        assert_eq!(outputs(&engine), outputs(&fresh), "{second}");
    }
}

/// A form whose read failed examined past where its error is placed: the
/// segment that read it ends past the edit of its text, so the parse after
/// the edit reads it again (the seeded run of part 1's close found T16's
/// included string read from the text before).
#[test]
fn a_segment_whose_read_failed_is_not_done_again_after_an_edit_of_it() {
    let score = |text: &str| {
        format!("\\version \"2.26.0\"\n{{ c4 }}\n#(ly:warning \"{text}\")\n{{ d4 }}\n")
    };
    let mut engine = engine();
    for text in ["x\\\\y", "x\\oy", "x\\\\y", "x\\\\z"] {
        compile(&mut engine, "/s/a.ly", &score(text));
        let mut fresh = self::engine();
        compile(&mut fresh, "/s/a.ly", &score(text));
        assert_eq!(outputs(&engine), outputs(&fresh), "{text}");
    }
}

/// 02 §5.7's step 3: in an item parsed again, the braced blocks before the
/// edit that ran no Scheme are taken as they were, when entered in the
/// state they were (a block entered with another default duration is
/// parsed again); outputs are a fresh engine's.
#[test]
fn a_parse_after_an_edit_takes_the_blocks_before_it() {
    let score = |first: &str, last: &str| {
        format!(
            "\\version \"2.26.0\"\na = {{ {first} {{ c d }} {{ d c }} {last} }}\n\\score {{ \\a }}\n"
        )
    };
    let mut engine = engine();
    for (first, last, taken) in [
        ("c2", "c4", 0),
        ("c2", "d4", 2),
        ("c8", "d4", 0),
        ("c8", "c2", 2),
    ] {
        compile(&mut engine, "/s/a.ly", &score(first, last));
        let mut fresh = self::engine();
        compile(&mut fresh, "/s/a.ly", &score(first, last));
        assert_eq!(outputs(&engine), outputs(&fresh), "{first} {last}");
        assert_eq!(engine.blocks_taken(), taken, "{first} {last}");
    }
}

/// Past a same-length edit, on the lines after it, a braced block is taken
/// only when entered with the default duration it was: the music's length
/// is a fresh parse's.
#[test]
fn a_block_entered_in_another_state_is_parsed_again() {
    let score = |first: &str| {
        format!(
            "\\version \"2.26.0\"\na = {{ {first}\n{{ c d }} {{ d c }} }}\n#(ly:warning (format #f \"~a\" (ly:music-length a)))\n"
        )
    };
    let mut engine = engine();
    for (first, taken) in [("c2", 0), ("c8", 0), ("d8", 2)] {
        compile(&mut engine, "/s/a.ly", &score(first));
        let mut fresh = self::engine();
        compile(&mut fresh, "/s/a.ly", &score(first));
        assert_eq!(outputs(&engine), outputs(&fresh), "{first}");
        assert_eq!(engine.blocks_taken(), taken, "{first}");
    }
}

#[test]
fn a_translation_after_an_edit_finds_where_it_changed_and_converged() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    use lily_engine_music::{Moment, Rational};
    let score = |header: &str, note: &str| {
        format!(
            "\\version \"2.26.0\"
{header}
\\score {{ \\new Staff {{ \\time 2/4 c'4 d' | e' {note} | e' f' | g' g' | a' b' | c'' c'' }} \\layout {{ }} }}
"
        )
    };
    let vm = support::library_with(&lily, lily_engine_core::install).expect("(lily) loads");
    let mut engine = engine_of(vm);
    engine.set_check_n8(true);
    // The font packs it asks for put as none.
    let mut run = |text: &str| {
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        engine.compile(1, "/s/a.ly");
        while engine.step(10.0) == NEEDS {
            let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
            engine.put(kind, &name, None);
            engine.compile(1, "/s/a.ly");
        }
        assert!(engine.n8_wrong().is_empty(), "{:?}", engine.n8_wrong());
        engine.changes().to_vec()
    };
    let at = |n, d| Moment::new(Rational::new(n, d), Rational::ZERO);

    // A first compile has nothing to find against.
    assert!(run(&score("", "f'")).is_empty());
    // The same text, and a header's edit (E3): its `\layout` and its
    // `\midi`, the same music.
    for header in ["", "\\header { title = \"T\" }"] {
        let changes = run(&score(header, "f'"));
        assert_eq!(changes.len(), 2, "{header}");
        assert!(
            changes.iter().all(|c| c.changed.is_none() && c.run == 0),
            "{changes:?}"
        );
    }
    // A pitch's edit (E1) in the second bar: from the checkpoint at its bar's
    // start to the bar after both pitches came again (`localAlterations`
    // remembers each pitch's last bar), only its own column changed.
    let changes = run(&score("", "g'"));
    for c in &changes {
        assert_eq!(c.changed, Some(at(3, 4)), "{c:?}");
        assert_eq!(c.resume, Some(0), "{c:?}");
        assert_eq!(c.columns, [at(3, 4)], "{c:?}");
    }
    assert_eq!(changes[0].converged, Some(3), "{:?}", changes[0]);
    assert!(changes[0].run < changes[0].steps, "{:?}", changes[0]);
    // An accidental is remembered to the end in the `\layout`, which does
    // not converge; the `\midi` does. Only its column changed.
    run(&score("", "f'"));
    let changes = run(&score("", "fis'"));
    assert_eq!(changes[0].converged, None, "{changes:?}");
    assert!(changes[1].converged.is_some(), "{changes:?}");
    assert!(
        changes.iter().all(|c| c.columns == [at(3, 4)]),
        "{changes:?}"
    );
    // A duration's edit (E2) moves every timestep after it: no convergence.
    // Its sequence has an element more, whose grace time the one before
    // looks at: from that one's start.
    let changes = run(&score("", "f'8 f'"));
    for c in &changes {
        assert_eq!(c.changed, Some(at(1, 2)), "{c:?}");
        assert_eq!(c.converged, None, "{c:?}");
    }
}

/// N8's checks where an edit's music is met out of the shape's order (OPEN
/// C15): under `\relative` a pitch in a repeat's body moves its alternatives
/// an octave, which its properties hold before the body, so the change is
/// the body's note's moment; and a run whose first grobs are others numbers
/// the interfaces otherwise, which the records of a staff it did not touch
/// do not show.
#[test]
fn n8s_checks_hold_for_a_repeats_body_and_interfaces_met_in_another_order() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    use lily_engine_music::{Moment, Rational};
    let vm = support::library_with(&lily, lily_engine_core::install).expect("(lily) loads");
    let mut engine = engine_of(vm);
    engine.set_check_n8(true);
    let mut run = |text: &str| {
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        engine.compile(1, "/s/a.ly");
        while engine.step(10.0) == NEEDS {
            let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
            engine.put(kind, &name, None);
            engine.compile(1, "/s/a.ly");
        }
        assert!(engine.n8_wrong().is_empty(), "{:?}", engine.n8_wrong());
        engine.changes().to_vec()
    };
    let repeat = |note: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ \\new Staff \\relative {{ \\time 3/4 \\repeat volta 2 {{ c''4 {note} f }} \\alternative {{ {{ e2. }} {{ f2. }} }} c2. }} \\layout {{ }} }}
"
        )
    };
    run(&repeat("f"));
    let changes = run(&repeat("g"));
    assert!(!changes.is_empty());
    for c in &changes {
        assert_eq!(
            c.changed,
            Some(Moment::new(Rational::new(1, 4), Rational::ZERO)),
            "{c:?}"
        );
    }
    let staves = |first: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ << \\new Staff {{ {first} d'4 e' }} \\new Staff {{ e'4 f' g' }} >> \\layout {{ }} }}
"
        )
    };
    run(&staves("r4"));
    run(&staves("c'4"));
}

/// N8's partitions (04 §10.3, §10.4; MS6 step 6): a Lyrics context joins
/// its voice's staff and a voice that changes staff joins both; an edit in
/// one partition runs it and replays the other, each translation checked
/// against its whole run.
#[test]
fn an_edit_runs_its_partition_and_replays_the_others() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let score = |one: &str, down: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ <<
  \\new Staff = \"one\" \\new Voice = \"v\" {{ \\time 2/4 c'4 d' | e' {one} | e' f' | g' g' | a' b' | c'' c'' }}
  \\new Lyrics \\lyricsto \"v\" {{ a b c d e f g h i j k l }}
  \\new PianoStaff <<
    \\new Staff = \"up\" {{ s2*6 }}
    \\new Staff = \"down\" {{ \\clef bass c4 d | {down} f | \\change Staff = \"up\" g' a' | b' c'' | d'' e'' | f'' g'' }}
  >>
>> \\layout {{ }} \\midi {{ }} }}
"
        )
    };
    let vm = support::library_with(&lily, lily_engine_core::install).expect("(lily) loads");
    let mut engine = engine_of(vm);
    engine.set_check_n8(true);
    let mut run = |text: &str| {
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        engine.compile(1, "/s/a.ly");
        while engine.step(10.0) == NEEDS {
            let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
            engine.put(kind, &name, None);
            engine.compile(1, "/s/a.ly");
        }
        assert!(engine.n8_wrong().is_empty(), "{:?}", engine.n8_wrong());
        let keys: Vec<Vec<String>> = engine.kept().iter().map(|k| k.parts.keys.clone()).collect();
        (keys, engine.changes().to_vec())
    };
    let (keys, _) = run(&score("f'", "e"));
    // The Lyrics with its voice's staff, the piano's two staves one; the
    // `\midi`'s the same (its Lyrics make a track).
    assert_eq!(keys.len(), 2, "{keys:?}");
    let layout = &keys[0];
    assert_eq!(layout.len(), 2, "{layout:?}");
    let has = |key: &String, names: &[&str]| names.iter().all(|n| key.contains(n));
    assert!(
        layout.iter().any(|k| has(k, &["Staff=one", "Lyrics"])),
        "{layout:?}"
    );
    assert!(
        layout.iter().any(|k| has(k, &["Staff=up", "Staff=down"])),
        "{layout:?}"
    );
    assert_eq!(keys[1], keys[0]);
    // A pitch's edit in the first staff runs its partition only, and one in
    // the piano's only the piano's.
    for (one, down, edited) in [("g'", "e", "Staff=one"), ("g'", "g", "Staff=down")] {
        let (keys, changes) = run(&score(one, down));
        assert_eq!(changes.len(), 2, "{changes:?}");
        for (k, c) in changes.iter().enumerate() {
            let parts = c.parts.as_ref().expect("the same partitions");
            for (key, &n) in keys[k].iter().zip(&parts.runs) {
                assert_eq!(n > 0, key.contains(edited), "{key}: {parts:?}");
            }
            assert!(parts.work < parts.unpartitioned, "{parts:?}");
            // Its writes outside itself (MS6 step 7a): none here but the
            // grobs a staff adds to a column's grob arrays.
            assert_eq!(parts.reads_only, parts.work, "{parts:?}");
        }
    }
}

#[test]
fn an_edit_lays_out_again_only_the_cells_it_reaches() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let fonts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    if !fonts.join("emmentaler-20.pack").exists() {
        eprintln!("skipped: no dist/engine packs (npm run build)");
        return;
    }
    let datadir = lily.join("../..");
    // Two staves on two lines, the first's slur across the break; an
    // articulation moves no column, a pitch here does (a stem's length).
    let score = |up: &str, down: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ <<
  \\new Staff {{ \\stemUp c'4 d' e'( f' | \\break g' a'{up} b' c'') }}
  \\new Staff {{ \\clef bass \\stemDown c4 d{down} e f | g a b c' }}
>> \\layout {{ }} }}
"
        )
    };
    let compile = |engine: &mut Engine, text: &str| {
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        answer_from_disk(engine, "/s/a.ly", &datadir);
        let outputs = engine.outputs().to_vec();
        let reports = engine.cell_reports().to_vec();
        (outputs, reports)
    };
    let fresh = |text: &str| {
        let mut engine =
            engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
        compile(&mut engine, text).0
    };
    let mut engine = engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
    let (_, reports) = compile(&mut engine, &score("", ""));
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert_eq!((reports[0].cells, reports[0].reused), (4, 0), "{reports:?}");
    // The same again: every cell, the upper staff's on the second line
    // holding the bar number its group moved (D260).
    let text = score("", "");
    let (outputs, reports) = compile(&mut engine, &text);
    assert_eq!(outputs, fresh(&text));
    assert_eq!((reports[0].cells, reports[0].reused), (4, 4), "{reports:?}");
    // The lower staff's first line changed, and the first line's columns:
    // both staves' second lines are the last compile's.
    let text = score("", "-.");
    let (outputs, reports) = compile(&mut engine, &text);
    assert_eq!(outputs, fresh(&text));
    assert_eq!((reports[0].cells, reports[0].reused), (4, 2), "{reports:?}");
    // Under the slur on the second line: the slur's first line read it too,
    // and only the lower staff's two cells are reused.
    let text = score("-.", "");
    compile(&mut engine, &score("", ""));
    let (outputs, reports) = compile(&mut engine, &text);
    assert_eq!(outputs, fresh(&text));
    assert_eq!((reports[0].cells, reports[0].reused), (4, 2), "{reports:?}");
    // Laid out whole, the same.
    engine.set_cells(false);
    let text = score("", "");
    let (outputs, reports) = compile(&mut engine, &text);
    assert_eq!(outputs, fresh(&text));
    assert!(reports.is_empty(), "{reports:?}");
}

#[test]
fn an_edit_fills_again_only_the_lines_it_reaches() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let fonts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    if !fonts.join("emmentaler-20.pack").exists() {
        eprintln!("skipped: no dist/engine packs (npm run build)");
        return;
    }
    let datadir = lily.join("../..");
    // Twelve bars the line breaker may break at each bar line: the lines it
    // weighs from one bar line to another (MS6 step 9, D249).
    let score = |last: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ \\new Staff {{ \\repeat unfold 11 {{ c'4 d' e' f' | }} g' a' b' {last} }} \\layout {{ }} }}
"
        )
    };
    let compile = |engine: &mut Engine, text: &str| {
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        answer_from_disk(engine, "/s/a.ly", &datadir);
        (engine.outputs().to_vec(), engine.cell_reports().to_vec())
    };
    let fresh = |text: &str| {
        let mut engine =
            engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
        compile(&mut engine, text).0
    };
    let mut engine = engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
    let (_, reports) = compile(&mut engine, &score("c''"));
    assert_eq!(reports[0].lines_reused, 0, "{reports:?}");
    let lines = reports[0].lines;
    assert!(lines > 12, "{reports:?}");
    // The same again: every line the last compile's.
    let text = score("c''");
    let (outputs, reports) = compile(&mut engine, &text);
    assert_eq!(outputs, fresh(&text));
    assert_eq!((reports[0].lines, reports[0].lines_reused), (lines, lines));
    // A pitch in the last bar: the lines before it are the last compile's,
    // those that reach it are filled again.
    let text = score("d''");
    let (outputs, reports) = compile(&mut engine, &text);
    assert_eq!(outputs, fresh(&text));
    let reused = reports[0].lines_reused;
    assert!(reused > 0 && reused < lines, "{reports:?}");
}

/// Y7 before the line breaks (MS6 step 8, part 2, D250, OPEN B17): what a
/// line's grobs got before the breaks is given back where the line, the
/// breaks and what it read are as they were, the edited staff's laid out
/// again; and each compile is the one from scratch.
#[test]
fn an_edit_lays_out_again_before_the_breaks_only_the_lines_it_reaches() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let fonts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    if !fonts.join("emmentaler-20.pack").exists() {
        eprintln!("skipped: no dist/engine packs (npm run build)");
        return;
    }
    let datadir = lily.join("../..");
    // Three staves, a dynamic spanner on the second (whose
    // `before-line-breaking` writes its own `bound-details` and runs again
    // on what was given back).
    let score = |pitch: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ <<
  \\new Staff {{ \\repeat unfold 4 {{ c'4 d' e' f' | }} {pitch}1 }}
  \\new Staff {{ \\repeat unfold 4 {{ g4\\cresc a b c' | }} d'1\\! }}
  \\new Staff {{ \\clef bass \\repeat unfold 5 {{ c1 | }} }}
>> \\layout {{ }} }}
"
        )
    };
    let compile = |engine: &mut Engine, text: &str| {
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        answer_from_disk(engine, "/s/a.ly", &datadir);
        (engine.outputs().to_vec(), engine.cell_reports().to_vec())
    };
    let fresh = |text: &str| {
        let mut engine =
            engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
        compile(&mut engine, text).0
    };
    let mut engine = engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
    let (_, reports) = compile(&mut engine, &score("g'"));
    assert_eq!(reports[0].before_reused, 0, "{reports:?}");
    let lines = reports[0].before;
    // The same again: every line the last compile's.
    let text = score("g'");
    let (outputs, reports) = compile(&mut engine, &text);
    assert_eq!(outputs, fresh(&text));
    assert_eq!(reports[0].before_reused, lines, "{reports:?}");
    // A pitch in the first staff: its line is laid out again, the others not.
    let text = score("a'");
    let (outputs, reports) = compile(&mut engine, &text);
    assert_eq!(outputs, fresh(&text));
    let reused = reports[0].before_reused;
    assert!(reused > 0 && reused < lines, "{reports:?}");
    // And back, after a compile that gave lines back.
    let text = score("g'");
    let (outputs, _) = compile(&mut engine, &text);
    assert_eq!(outputs, fresh(&text));
}

/// W4 (MS6 step 10, D253): for a preview a compile's pages are in the
/// preview form, each line a cell; an edit of one staff sends its cell, the
/// others taken and not written again, and what it sends rebuilds the page.
/// D258: the same music splices as it resumes, whatever the edit; and an edit
/// typed again and again in one place resumes every time, near it, where it
/// ran whole one compile in four (D255) when a snapshot aged out.
#[test]
fn a_resume_stays_near_its_edit() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let score = |header: &str, note: &str| {
        let bars = "c'4 d' e' f' | ".repeat(24);
        format!(
            "\\version \"2.26.0\"\n{header}\n\\score {{ {{ {bars}{note}4 d' e' f' | {bars} }} \\layout {{ }} \\midi {{ }} }}\n"
        )
    };
    // A compile that stops for a pack keeps nothing, so each needs its packs.
    let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    if !fonts.join("emmentaler-20.pack").exists() {
        eprintln!("skipped: no build's font packs");
        return;
    }
    let vm = support::library_with(&lily, lily_engine_core::install).expect("(lily) loads");
    let mut engine = engine_of(vm);
    let mut run = |text: &str| {
        engine.put(FILE, "/s/near.ly", Some(text.as_bytes()));
        engine.compile(1, "/s/near.ly");
        while engine.step(10.0) == NEEDS {
            let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
            let pack = std::fs::read(fonts.join(format!("{name}.pack"))).ok();
            engine.put(kind, &name, pack.as_deref());
            engine.compile(1, "/s/near.ly");
        }
        (engine.resumed_at().to_vec(), engine.spliced().to_vec())
    };
    let title = |t: &str| format!("\\header {{ title = \"{t}\" }}");
    assert_eq!(
        run(&score(&title("T"), "e'")).0,
        [None, None],
        "a first compile"
    );
    // The header's edit: the music the same, each translation spliced where
    // it resumed.
    let (resumed, spliced) = run(&score(&title("Tx"), "e'"));
    assert!(resumed.iter().all(Option::is_some), "{resumed:?}");
    assert_eq!(spliced, [true, true]);
    // One note typed over again: no compile runs whole, and past the first
    // the snapshots after the edit are there for it to splice at: the `\\midi`
    // every time, the `\\layout` at least where it comes back to the note it
    // had (its accidentals' and stems' state otherwise reaches further).
    let mut laid_out = 0;
    for round in 0..12 {
        // Pitches that come again: `localAlterations` keeps each one's last bar.
        let note = if round % 2 == 0 { "d'" } else { "e'" };
        let (resumed, spliced) = run(&score(&title("Tx"), note));
        assert!(resumed.iter().all(Option::is_some), "{round}: {resumed:?}");
        if round > 1 {
            assert!(spliced[1], "{round}");
            laid_out += usize::from(spliced[0]);
        }
    }
    assert!(laid_out >= 5, "{laid_out}");
}

#[test]
fn an_edit_sends_the_cells_it_changes() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let fonts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    if !fonts.join("emmentaler-20.pack").exists() {
        eprintln!("skipped: no dist/engine packs (npm run build)");
        return;
    }
    let datadir = lily.join("../..");
    let text = |note: &str| {
        format!(
            "\\version \"2.26.0\"
\\score {{ <<
  \\new Staff {{ c'4 d' e' f' }}
  \\new Staff {{ {note}4 a' b' c'' }}
  \\new Staff {{ c'1 }}
>> \\layout {{ }} }}
"
        )
    };
    let mut engine = engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
    engine.set_preview(true);
    let mut compile = |note: &str| {
        engine.put(FILE, "/s/a.ly", Some(text(note).as_bytes()));
        answer_from_disk(&mut engine, "/s/a.ly", &datadir);
        let page = |name| engine.outputs().iter().find(|(n, _)| n == name).cloned();
        let svg = page("a.svg").map(|(_, data)| String::from_utf8(data).unwrap());
        let cells = page("a.cells").map(|(_, data)| String::from_utf8(data).unwrap());
        (svg.unwrap(), cells, engine.page_reports()[0].clone())
    };

    let (first, none, report) = compile("g'");
    assert_eq!(none, None, "a new page goes whole");
    assert!(
        !first.contains("<use") && !first.contains("<defs"),
        "{first}"
    );
    assert_eq!(first.matches("<g data-lily-cell=").count(), report.cells);
    assert!(report.cells >= 4, "{report:?}: three lines and the system");
    assert_eq!(report.taken, 0);

    let (same, none, report) = compile("g'");
    assert_eq!((same.as_str(), none), (first.as_str(), None));
    assert_eq!((report.taken, report.replaced), (report.entries, false));

    let (edited, cells, report) = compile("a'");
    let cells = cells.expect("a page with cells kept goes by its cells");
    let (digests, markup) = cells.split_once('\n').unwrap();
    assert_eq!(digests.split(' ').count(), report.cells);
    assert_eq!(report.sent, markup.matches("<g data-lily-cell=").count());
    assert!(report.replaced && report.sent < report.cells, "{report:?}");
    assert!(report.taken > report.entries / 2, "{report:?}");
    assert!(edited.contains(markup), "the page's cells, as written");
}

/// 05 §7's queue (MS6 step 9, D251): a score of several pages with the
/// third page visible draws it first, and its outputs are those of the first
/// page visible; the longest step is timed.
#[test]
fn the_visible_page_is_drawn_first_and_changes_no_output() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let fonts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    if !fonts.join("emmentaler-20.pack").exists() {
        eprintln!("skipped: no dist/engine packs (npm run build)");
        return;
    }
    let datadir = lily.join("../..");
    // Four pages.
    let text = "\\version \"2.26.0\"
\\paper { page-count = 4 }
\\score { \\new Staff { \\repeat unfold 160 { c'8 d' e' f' g' a' b' c'' | } } \\layout { } }
";
    let compile = |page: u32| {
        let mut engine =
            engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
        engine.set_visible_page(page);
        engine.put(FILE, "/s/a.ly", Some(text.as_bytes()));
        answer_from_disk(&mut engine, "/s/a.ly", &datadir);
        (engine.outputs().to_vec(), engine.times())
    };
    let (first, times) = compile(0);
    let pages = first
        .iter()
        .filter(|(name, _)| name.ends_with(".svg"))
        .count();
    assert!(
        pages >= 3,
        "{:?}",
        first.iter().map(|o| &o.0).collect::<Vec<_>>()
    );
    assert!(times.steps > 0 && times.longest_step_ms > 0.0, "{times:?}");
    let (third, times) = compile(2);
    assert_eq!(third, first);
    assert!(times.visible_ms > 0.0, "{times:?}");
}

/// I7 (D269): a preview's compile in steps hands its visible page over as
/// soon as it is written, a step's outputs until the next call: its `.svg`,
/// and its `.cells` when it goes by them, each the bytes the outputs at the
/// end hold. A step that hands nothing over has no outputs, nor does a
/// compile for no preview.
#[test]
fn the_visible_page_is_handed_over_before_the_compile_ends() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let fonts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    if !fonts.join("emmentaler-20.pack").exists() {
        eprintln!("skipped: no dist/engine packs (npm run build)");
        return;
    }
    let datadir = lily.join("../..");
    // Four pages; the note edited is on the second, the page shown.
    let text = |note: &str| {
        format!(
            "\\version \"2.26.0\"
\\paper {{ page-count = 4 }}
\\score {{ \\new Staff {{ \\repeat unfold 100 {{ c'8 d' e' f' g' a' b' c'' | }}
  {note}8 d' e' f' g' a' b' c'' |
  \\repeat unfold 59 {{ c'8 d' e' f' g' a' b' c'' | }} }} \\layout {{ }} }}
"
        )
    };
    let mut engine = engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
    let mut revision = 0;
    // Each step's outputs while the compile goes on, and the outputs at its end.
    let mut compile = |engine: &mut Engine, note: &str, preview: bool| {
        engine.set_preview(preview);
        engine.set_returns(true);
        engine.set_visible_page(if preview { 1 } else { 0 });
        engine.put(FILE, "/s/a.ly", Some(text(note).as_bytes()));
        revision += 1;
        let mut handed = Vec::new();
        let mut steps = 0;
        engine.compile(revision, "/s/a.ly");
        loop {
            match engine.step(5.0) {
                MORE => {
                    steps += 1;
                    if !engine.outputs().is_empty() {
                        handed.push((steps, engine.outputs().to_vec()));
                    }
                    continue;
                }
                // Started again, it hands its page over again.
                NEEDS => handed.clear(),
                _ => break,
            }
            let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
            let bytes = match kind {
                MODULE => std::fs::read(fonts.join(format!("{name}.pack"))).ok(),
                _ => match name.strip_prefix(DATADIR) {
                    Some(rest) => std::fs::read(datadir.join(rest.trim_start_matches('/'))).ok(),
                    None => None,
                },
            };
            engine.put(kind, &name, bytes.as_deref());
            engine.compile(revision, "/s/a.ly");
        }
        (handed, steps, engine.outputs().to_vec())
    };
    let names =
        |outputs: &[(String, Vec<u8>)]| outputs.iter().map(|o| o.0.clone()).collect::<Vec<_>>();

    for (note, cells) in [("c'", false), ("a'", true)] {
        let (handed, steps, done) = compile(&mut engine, note, true);
        assert_eq!(handed.len(), 1, "{note}: handed over once");
        let (at, page) = &handed[0];
        // At a step that returned before the compile's end, which the next calls reach.
        assert!(
            *at <= steps,
            "{note}: before the compile's end ({at} of {steps} steps)"
        );
        let want = match cells {
            false => vec!["a-2.svg"],
            true => vec!["a-2.svg", "a-2.cells"],
        };
        assert_eq!(names(page), want, "{note}");
        for (name, bytes) in page {
            let end = done.iter().find(|(n, _)| n == name).map(|(_, b)| b);
            assert_eq!(
                end,
                Some(bytes),
                "{note}: {name} as the outputs at the end hold it"
            );
        }
    }

    let (handed, _, done) = compile(&mut engine, "c'", false);
    assert!(handed.is_empty(), "no preview, no page handed over");
    assert_eq!(done.iter().filter(|(n, _)| n.ends_with(".svg")).count(), 4);
}

/// E14 (D262): a text face's outlines are asked for when a skyline first
/// needs one, in steps where it is met and in one call at its end, and the
/// pages are those of a compile given them first; given before their face,
/// they wait for it; ones the host cannot give leave the glyphs' boxes, and
/// a warning.
#[test]
fn a_text_face_s_outlines_are_asked_for_when_a_skyline_needs_one() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let fonts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    if !fonts.join("C059-Roman.outlines.pack").exists() {
        eprintln!("skipped: no dist/engine packs (npm run build)");
        return;
    }
    // Roman lyrics, and a bold italic markup: a face fetched on demand, then its outlines.
    let text = "\\version \"2.26.0\"
\\score { << \\new Staff { c'4 d' e' f'^\\markup \\bold \\italic \"fine\" } \\addlyrics { Hel -- lo good world } >> \\layout { } }
";
    let image = {
        let mut vm = support::library_with(&lily, lily_engine_core::install).unwrap();
        write_image(&vm.write_image().expect("the VM is at rest"))
    };
    let pack = |name: &str| std::fs::read(fonts.join(format!("{name}.pack"))).unwrap();
    let compile = |returns: bool, given: &[&str], offline: bool| {
        let mut engine = Engine::start(&image).expect("the image restores");
        engine.set_returns(returns);
        for name in ["emmentaler-20", "C059-Roman", "C059-Bold", "C059-Italic"] {
            engine.put(MODULE, name, Some(&pack(name)));
        }
        for name in given {
            engine.put(MODULE, name, Some(&pack(name)));
        }
        engine.put(FILE, "/s/l.ly", Some(text.as_bytes()));
        engine.compile(1, "/s/l.ly");
        let mut asked: Vec<String> = Vec::new();
        loop {
            match engine.step(10.0) {
                MORE => continue,
                NEEDS => {
                    let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
                    assert_eq!(kind, MODULE, "{name}");
                    let fetched = (!offline || !name.ends_with(".outlines")).then(|| pack(&name));
                    engine.put(kind, &name, fetched.as_deref());
                    asked.push(name);
                    engine.compile(1, "/s/l.ly");
                }
                _ => break,
            }
        }
        let output = |name: &str| {
            let data = engine
                .outputs()
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, d)| d.clone());
            String::from_utf8(data.unwrap_or_default()).unwrap()
        };
        // A face first; the outlines in the order the skylines met them.
        asked.sort_by_key(|name| (name.ends_with(".outlines"), name.clone()));
        (asked, output("l.svg"), output("stderr"))
    };
    let all = ["C059-BdIta", "C059-Roman.outlines", "C059-BdIta.outlines"];
    let (none, first, said) = compile(true, &all, false);
    assert_eq!(none, Vec::<String>::new());
    assert!(
        first.contains("<text") && !said.contains("outlines"),
        "{said}"
    );
    for returns in [true, false] {
        let (asked, svg, said) = compile(returns, &[], false);
        assert_eq!(
            asked,
            ["C059-BdIta", "C059-BdIta.outlines", "C059-Roman.outlines"],
            "returns {returns}"
        );
        assert_eq!(
            (svg.as_str(), said.as_str()),
            (first.as_str(), ""),
            "returns {returns}"
        );
    }
    // The outlines before their face: kept until the face comes.
    let (asked, svg, _) = compile(true, &["C059-BdIta.outlines", "C059-Roman.outlines"], false);
    assert_eq!(asked, ["C059-BdIta"]);
    assert_eq!(svg, first);
    // Offline: the boxes, said once a face.
    let (asked, svg, said) = compile(true, &[], true);
    assert_eq!(
        asked,
        ["C059-BdIta", "C059-BdIta.outlines", "C059-Roman.outlines"]
    );
    assert!(svg.contains("<text"));
    assert_eq!(
        said.matches("could not be fetched (font pack C059-")
            .count(),
        2,
        "{said}"
    );
    assert!(
        said.contains("the outlines of font 'C059-Roman' could not be fetched"),
        "{said}"
    );
}

/// Steps a compile, or with `perform` a performance (D264), of `root` to its
/// end, each name asked answered as [`answer_from_disk`] does, and started
/// again the same way; its outputs by name.
fn run_from_disk(
    engine: &mut Engine,
    root: &str,
    revision: u32,
    perform: bool,
    datadir: &Path,
) -> std::collections::BTreeMap<String, Vec<u8>> {
    let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    let start = |engine: &mut Engine| match perform {
        true => engine.perform(revision, root),
        false => engine.compile(revision, root),
    };
    start(engine);
    loop {
        match engine.step(10.0) {
            MORE => continue,
            NEEDS => {}
            _ => break,
        }
        let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
        let bytes = if kind == MODULE && PART_MODULES.iter().any(|(part, _)| *part == name) {
            Some(part_module(&name, datadir))
        } else if kind == MODULE {
            std::fs::read(fonts.join(format!("{name}.pack"))).ok()
        } else {
            let path = match name.strip_prefix(DATADIR) {
                Some(rest) => datadir.join(rest.trim_start_matches('/')),
                None => PathBuf::from(&name),
            };
            std::fs::read(path).ok()
        };
        engine.put(kind, &name, bytes.as_deref());
        start(engine);
    }
    engine.outputs().iter().cloned().collect()
}

/// M7 (07 §3.8, D264): a compile for a page leaves its `\midi`s out, and the
/// performance after it writes the MIDI file and the playback map a compile
/// of all its definitions writes, byte for byte, and what its translations
/// said, which after the page's is the compile's; a compile a translation's
/// error ended performs nothing.
#[test]
fn the_performance_after_the_page_writes_what_the_compile_would() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let fonts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    if !fonts.join("emmentaler-20.pack").exists() {
        eprintln!("skipped: no dist/engine packs (npm run build)");
        return;
    }
    let datadir = lily.join("../..");
    // No `\\version`: the parse's warning, which the page says alone.
    let text = "\\score { <<
  \\new Staff \\with { midiInstrument = \"no such\" } { \\tempo 4 = 90 c'4 d' e' f' | g'1 }
  \\new Staff { \\clef bass c4 e g c' | c1 }
>> \\layout { } \\midi { } }
";
    let new_engine = || engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
    let file = |outputs: &std::collections::BTreeMap<String, Vec<u8>>, name: &str| {
        String::from_utf8_lossy(outputs.get(name).map_or(&[][..], |d| &d[..])).into_owned()
    };
    let mut whole = new_engine();
    whole.put(FILE, "/s/p.ly", Some(text.as_bytes()));
    let all = run_from_disk(&mut whole, "/s/p.ly", 1, false, &datadir);
    assert!(
        all.contains_key("p.midi") && all.contains_key("p.timing.json"),
        "{:?}",
        all.keys()
    );
    assert!(
        file(&all, "stderr").contains("no such MIDI instrument"),
        "{}",
        file(&all, "stderr")
    );

    let mut paged = new_engine();
    paged.set_performs(false);
    paged.set_preview(true);
    paged.put(FILE, "/s/p.ly", Some(text.as_bytes()));
    let page = run_from_disk(&mut paged, "/s/p.ly", 1, false, &datadir);
    let names: Vec<&str> = page.keys().map(String::as_str).collect();
    assert_eq!(names, ["diagnostics.json", "p.svg", "stderr"]);
    assert!(
        file(&page, "stderr").contains("no \\version statement found")
            && !file(&page, "stderr").contains("MIDI instrument"),
        "{}",
        file(&page, "stderr")
    );
    let performed = run_from_disk(&mut paged, "/s/p.ly", 1, true, &datadir);
    let names: Vec<&str> = performed.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        ["diagnostics.json", "p.midi", "p.timing.json", "stderr"]
    );
    assert_eq!(performed["p.midi"], all["p.midi"], "the same MIDI file");
    assert_eq!(
        performed["p.timing.json"], all["p.timing.json"],
        "the same map"
    );
    assert_eq!(
        file(&page, "stderr") + &file(&performed, "stderr"),
        file(&all, "stderr"),
        "the page's messages, then the performers'"
    );
    // Performed again, the same.
    assert_eq!(
        run_from_disk(&mut paged, "/s/p.ly", 1, true, &datadir),
        performed
    );

    // A run a translation's error ended: no MIDI file, and nothing to perform.
    let ended = "\\version \"2.26.0\"\n\\score { { c'4 } \\layout { \\context { \\Voice \\consists #(lambda (c) (error \"stop\")) } } \\midi { } }\n";
    paged.put(FILE, "/s/p.ly", Some(ended.as_bytes()));
    let page = run_from_disk(&mut paged, "/s/p.ly", 2, false, &datadir);
    assert!(
        file(&page, "stderr").contains("stop"),
        "{}",
        file(&page, "stderr")
    );
    assert!(run_from_disk(&mut paged, "/s/p.ly", 2, true, &datadir).is_empty());
}

/// D264: a performance keeps nothing, ended or stopped, so the compiles
/// after it resume what the compile before it kept and write what a fresh
/// engine writes for their text.
#[test]
fn a_performance_leaves_the_kept_translations_as_they_were() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let fonts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    if !fonts.join("emmentaler-20.pack").exists() {
        eprintln!("skipped: no dist/engine packs (npm run build)");
        return;
    }
    let datadir = lily.join("../..");
    let score = |note: &str| {
        let bars = "c'4 d' e' f' | ".repeat(24);
        format!(
            "\\version \"2.26.0\"\n\\score {{ {{ {bars}{note}4 d' e' f' | {bars} }} \\layout {{ }} \\midi {{ }} }}\n"
        )
    };
    let new_engine = || {
        let mut engine =
            engine_of(support::library_with(&lily, lily_engine_core::install).unwrap());
        engine.set_performs(false);
        engine.set_preview(true);
        engine.set_returns(true);
        engine
    };
    let mut engine = new_engine();
    let mut revision = 0;
    for (round, note) in ["e'", "d'", "g'", "a'", "b'"].into_iter().enumerate() {
        revision += 1;
        let text = score(note);
        engine.put(FILE, "/s/k.ly", Some(text.as_bytes()));
        let mut page = run_from_disk(&mut engine, "/s/k.ly", revision, false, &datadir);
        if round > 0 {
            assert!(
                engine.resumed_at().iter().all(Option::is_some),
                "{round}: {:?}",
                engine.resumed_at()
            );
            // The writer's markup kept too: the page goes by its cells.
            assert!(
                page.remove("k.cells").is_some(),
                "{round}: {:?}",
                page.keys()
            );
        }
        let mut fresh = new_engine();
        fresh.put(FILE, "/s/k.ly", Some(text.as_bytes()));
        assert_eq!(
            page,
            run_from_disk(&mut fresh, "/s/k.ly", 1, false, &datadir),
            "{round}: the page"
        );
        if round % 2 == 0 {
            // Performed to its end, the same as a fresh engine's.
            let performed = run_from_disk(&mut engine, "/s/k.ly", revision, true, &datadir);
            assert_eq!(
                performed,
                run_from_disk(&mut fresh, "/s/k.ly", 1, true, &datadir),
                "{round}: the performance"
            );
        } else {
            // Stopped after its first step by the next compile.
            engine.perform(revision, "/s/k.ly");
            engine.step(0.0);
        }
    }
}

/// OPEN C18: a splice reads the old run's records as they were at the
/// checkpoint it splices at, not as the resume gave its objects back. A Scheme
/// engraver colours a grob red where the edited note is a `d'`, and the
/// compile after it, of an `e'`, splices: the red is the old run's change
/// alone, of the first note's grob, older than the snapshot resumed from
/// (`first`), and of the edited bar's second note's, younger, changed three
/// bars on between two of the old run's snapshots (`later`). Each compile
/// writes what a fresh engine writes.
#[test]
fn a_splice_reads_the_old_runs_records_as_they_were_where_it_splices() {
    let Some(lily) = support::lily_scm() else {
        eprintln!("skipped: no LilyPond 2.26.0");
        return;
    };
    let fonts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/engine/fonts");
    if !fonts.join("emmentaler-20.pack").exists() {
        eprintln!("skipped: no build's font packs");
        return;
    }
    // `n` counts the note heads: the 97th is the edited bar's first.
    let first = "(let ((first #f) (n 0))
          (make-engraver (acknowledgers ((note-head-interface engraver grob source)
            (if (not first)
                (begin (set! first grob) (ly:grob-set-property! grob 'color black))
                (if (and (= n 96) (= 1 (ly:pitch-notename (ly:event-property (event-cause grob) 'pitch))))
                    (ly:grob-set-property! first 'color red)))
            (set! n (+ n 1))))))";
    let later = "(let ((target #f) (pending #f) (n 0))
          (make-engraver (acknowledgers ((note-head-interface engraver grob source)
            (if (and (= n 96) (= 1 (ly:pitch-notename (ly:event-property (event-cause grob) 'pitch))))
                (set! pending #t))
            (if (= n 97)
                (begin (set! target grob) (ly:grob-set-property! grob 'color black)))
            (if (and (= n 108) pending)
                (begin (ly:grob-set-property! target 'color red) (set! pending #f)))
            (set! n (+ n 1))))))";
    let score = |engraver: &str, note: &str| {
        let bars = "c'4 d' e' f' | ".repeat(24);
        format!(
            "\\version \"2.26.0\"
\\score {{ {{ {bars}{note}4 d' e' f' | {bars} }}
  \\layout {{ \\context {{ \\Voice \\consists #(lambda (context) {engraver}) }} }} }}
"
        )
    };
    let mut vm = support::library_with(&lily, lily_engine_core::install).expect("(lily) loads");
    let image = write_image(&vm.write_image().expect("the VM is at rest"));
    let run = |engine: &mut Engine, text: &str| {
        engine.put(FILE, "/s/c18.ly", Some(text.as_bytes()));
        engine.compile(1, "/s/c18.ly");
        while engine.step(10.0) == NEEDS {
            let (kind, name) = engine.needs().map(|(k, n)| (k, n.to_string())).unwrap();
            let pack = std::fs::read(fonts.join(format!("{name}.pack"))).ok();
            engine.put(kind, &name, pack.as_deref());
            engine.compile(1, "/s/c18.ly");
        }
        let svg: Vec<Vec<u8>> = engine
            .outputs()
            .iter()
            .filter(|(n, _)| n.ends_with(".svg"))
            .map(|(_, b)| b.clone())
            .collect();
        (svg, engine.resumed_at().to_vec())
    };
    for (name, engraver) in [("first", first), ("later", later)] {
        let mut engine = Engine::start(&image).expect("the image restores");
        for round in 0..6 {
            let text = score(engraver, if round % 2 == 0 { "d'" } else { "e'" });
            let (svg, resumed) = run(&mut engine, &text);
            // Each `e'` resumes the `d'` before it, where the splice is.
            if round % 2 == 1 {
                assert!(resumed[0].is_some(), "{name} {round}: {resumed:?}");
            }
            let fresh = run(
                &mut Engine::start(&image).expect("the image restores"),
                &text,
            )
            .0;
            assert!(
                svg == fresh,
                "{name} {round}: not what a fresh engine writes"
            );
        }
    }
}
