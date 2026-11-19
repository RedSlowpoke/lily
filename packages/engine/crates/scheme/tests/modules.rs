//! S3's globals and modules (chapter 03 §5.4, R6, R10): Guile's module forms
//! and procedures give what Guile 3.0.11 gives, a global's slot has a
//! version and a module, read tracking records reads only when it is on,
//! and `set_location` sets `(lily)`'s `%location`, `lily.scm`'s own once it
//! is defined again. With `guile` installed, the table is also run by guile,
//! each program in a fresh process with [`SOURCES`] on its load path.
use std::path::PathBuf;
use std::process::Command;

use lily_engine_scheme::{Value, Vm};

/// The modules the programs load, as files under guile's load path and as
/// [`Vm::provide_module`] code for the engine.
const SOURCES: &[(&str, &str)] = &[
    (
        "test a",
        "(define-module (test a) #:export (a1 a2 shared))
(define a1 'a1)
(define a2 'a2)
(define shared 'from-a)
(define private 'private)
(define-public pub 'pub)",
    ),
    (
        "test b",
        "(define-module (test b) #:use-module (test a) #:re-export (a1) #:export (shared b1))
(define shared 'from-b)
(define b1 (list a1 a2))",
    ),
    (
        "test c",
        "(define-module (test c))
(define-public counter 0)
(define-public (bump!) (set! counter (+ counter 1)) counter)",
    ),
];

/// Programs and what they give: the last form's value as `write` writes
/// it, or the key and the text of the error, as in `tests/errors.rs`. A
/// module prints without its address, which is left out of guile's.
const CASES: &[(&str, &str, &str)] = &[
    // The current module, and `define-module`.
    ("(current-module)", "", "#<directory (guile-user)>"),
    ("(define-module (t one))", "", "#<directory (t one)>"),
    (
        "(define-module (t one)) (define x 1) (current-module)",
        "",
        "#<directory (t one)>",
    ),
    (
        "(define-module (t one)) (define x 1) (define-module (t two)) x",
        "unbound-variable",
        "Unbound variable: x",
    ),
    (
        "(begin (define-module (t one)) (define y 1)) (list (current-module) (@@ (t one) y))",
        "",
        "(#<directory (t one)> 1)",
    ),
    ("(eq? (interaction-environment) (current-module))", "", "#t"),
    (
        "(define-module (t one)) (save-module-excursion (lambda () (set-current-module (resolve-module '(t two))))) (current-module)",
        "",
        "#<directory (t one)>",
    ),
    (
        "(define-module (t one)) (catch #t (lambda () (save-module-excursion (lambda () (set-current-module (resolve-module '(t two))) (error \"x\")))) (lambda a #f)) (current-module)",
        "",
        "#<directory (t one)>",
    ),
    // Exports, `use-modules` and the interfaces it makes.
    (
        "(use-modules (test a)) (list a1 a2 shared pub)",
        "",
        "(a1 a2 from-a pub)",
    ),
    (
        "(use-modules (test a)) private",
        "unbound-variable",
        "Unbound variable: private",
    ),
    (
        "(use-modules ((test a) #:select (a1 (a2 . two)))) (list a1 two)",
        "",
        "(a1 a2)",
    ),
    (
        "(use-modules ((test a) #:select (a1))) a2",
        "unbound-variable",
        "Unbound variable: a2",
    ),
    (
        "(use-modules ((test a) #:prefix a:)) (list a:a1 a:pub)",
        "",
        "(a1 pub)",
    ),
    (
        "(use-modules ((test a) #:hide (a1))) (list a2 (catch #t (lambda () a1) (lambda (k . r) k)))",
        "",
        "(a2 unbound-variable)",
    ),
    (
        "(use-modules (test b)) (list a1 shared b1)",
        "",
        "(a1 from-b (a1 a2))",
    ),
    ("(use-modules (test a) (test b)) shared", "", "from-b"),
    ("(use-modules (test b) (test a)) shared", "", "from-a"),
    (
        "(use-modules (test b)) (use-modules (test a)) (use-modules (test b)) shared",
        "",
        "from-a",
    ),
    (
        "(define-module (t three) #:export (e1)) (define e1 1) (define-module (t four) #:use-module (t three)) e1",
        "",
        "1",
    ),
    (
        "(define-module (t five)) (define-public (pf x) (* x 2)) (define-module (t six) #:use-module (t five)) (pf 7)",
        "",
        "14",
    ),
    (
        "(define-module (t seven) #:use-module ((test a) #:select (a1)) #:use-module ((test b) #:prefix b:)) (list a1 b:b1)",
        "",
        "(a1 (a1 a2))",
    ),
    (
        "(define-module (t eight)) (define-public (car x) 'mine) (define-module (t nine) #:use-module (t eight)) (car '(1))",
        "",
        "mine",
    ),
    (
        "(use-modules (no such module))",
        "misc-error",
        "no code for module (no such module)",
    ),
    (
        "(re-export nothere)",
        "misc-error",
        "Undefined variable: nothere",
    ),
    ("(export later) (set! later 1) later", "", "1"),
    // A reference is resolved when it first runs, and stays so.
    ("(define (f) car) (define car 5) (f)", "", "5"),
    (
        "(define (g) cdr) (procedure? (g)) (define cdr 6) (list (procedure? (g)) cdr)",
        "",
        "(#t 6)",
    ),
    ("(define (h) a1) (use-modules (test a)) (h)", "", "a1"),
    // Variables are shared: `set!` of an import sets the exporter's.
    (
        "(use-modules (test c)) (bump!) (bump!) (list counter (@ (test c) counter))",
        "",
        "(2 2)",
    ),
    (
        "(use-modules (test c)) (set! counter 10) (list (bump!) counter)",
        "",
        "(11 11)",
    ),
    // `@` and `@@`.
    ("(@ (test a) a1)", "", "a1"),
    ("(@@ (test a) private)", "", "private"),
    (
        "(@ (test a) private)",
        "unbound-variable",
        "In procedure module-lookup: Unbound variable: private",
    ),
    ("((@ (guile) car) '(1 2))", "", "1"),
    (
        "(@ (no mod) x)",
        "wrong-type-arg",
        "In procedure module-variable: Wrong type argument in position 1 (expecting module): #f",
    ),
    (
        "(@@ (no mod) x)",
        "unbound-variable",
        "In procedure module-lookup: Unbound variable: x",
    ),
    // The module procedures.
    (
        "(define-module (t ex)) (define a 1) (define b 2) (module-export! (current-module) '(a b)) (define-module (t ey) #:use-module (t ex)) (list a b)",
        "",
        "(1 2)",
    ),
    (
        "(define-module (t ten)) (module-define! (current-module) 'md 3) (list md (module-ref (current-module) 'md) (module-ref (current-module) 'car))",
        "",
        "(3 3 #<procedure car (_)>)",
    ),
    (
        "(module-ref (current-module) 'nope 'default)",
        "",
        "default",
    ),
    (
        "(module-ref (current-module) 'nope)",
        "misc-error",
        "No variable named nope in #<directory (guile-user)>",
    ),
    (
        "(define-module (t eleven)) (define v 1) (define-module (t twelve)) (module-set! (resolve-module '(t eleven)) 'v 2) (@@ (t eleven) v)",
        "",
        "2",
    ),
    ("(module-ref (resolve-module '(test c)) 'counter)", "", "0"),
    ("(resolve-module '(t thirteen) #f #:ensure #f)", "", "#f"),
    (
        "(list (module? (current-module)) (module? 1) (module-name (current-module)) (module-name (resolve-module '(t x y))))",
        "",
        "(#t #f (guile-user) (t x y))",
    ),
    // `eval` in a module, which is current while it runs.
    (
        "(define-module (t fourteen)) (define-module (t user)) (eval '(define ev 5) (resolve-module '(t fourteen))) (list (@@ (t fourteen) ev) (current-module))",
        "",
        "(5 #<directory (t user)>)",
    ),
    (
        "(define-module (t fifteen)) (define-module (t user)) (eval '(current-module) (resolve-module '(t fifteen)))",
        "",
        "#<directory (t fifteen)>",
    ),
    // A module that `resolve-module` makes uses nothing, not even (guile).
    (
        "(eval '(current-module) (resolve-module '(t empty)))",
        "unbound-variable",
        "Unbound variable: current-module",
    ),
    (
        "(catch #t (lambda () (eval '(car 1) (resolve-module '(t sixteen)))) (lambda e #f)) (current-module)",
        "",
        "#<directory (guile-user)>",
    ),
    (
        "(define-module (t seventeen)) (define-public w 'seventeen) (define-module (t eighteen)) (eval 'w (resolve-module '(t seventeen)))",
        "",
        "seventeen",
    ),
    // Errors.
    (
        "(use-modules 1)",
        "syntax-error",
        "Syntax error:\nunknown location: source expression failed to match any pattern in form (1)",
    ),
    (
        "(@ 1 x)",
        "syntax-error",
        "Syntax error:\nunknown location: source expression failed to match any pattern in form (@ 1 x)",
    ),
    (
        "(eval '(define-module (t nineteen) #:bogus 1) (current-module))",
        "syntax-error",
        "Syntax error:\nunknown location: define-module: unknown keyword or bad argument in subform 1 of #:bogus",
    ),
    (
        "(eval '(define-module (t twenty) #:export x) (current-module))",
        "syntax-error",
        "Syntax error:\nunknown location: define-module: unknown keyword or bad argument in subform x of #:export",
    ),
    (
        "(module-define! 1 'a 1)",
        "wrong-type-arg",
        "In procedure module-local-variable: Wrong type argument in position 1 (expecting module): 1",
    ),
    (
        "(module-define! (current-module) \"a\" 1)",
        "wrong-type-arg",
        "In procedure module-local-variable: Wrong type argument in position 2 (expecting symbol): \"a\"",
    ),
    (
        "(module-ref 1 'a)",
        "wrong-type-arg",
        "In procedure module-variable: Wrong type argument in position 1 (expecting module): 1",
    ),
    (
        "(eval 1 1)",
        "wrong-type-arg",
        "In procedure eval: Wrong type argument in position 2 (expecting module): 1",
    ),
];

/// `#<directory (t one) 10a05f500>` as `#<directory (t one)>`.
fn without_addresses(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = ["#<directory ", "#<module "]
        .iter()
        .filter_map(|p| rest.find(p))
        .min()
    {
        let close = rest[i..].find('>').map_or(rest.len(), |j| i + j);
        let inside = &rest[..close];
        let inside = match inside.rsplit_once(' ') {
            Some((head, tail))
                if !tail.is_empty()
                    && tail.chars().all(|c| c.is_ascii_hexdigit())
                    && head.ends_with(')') =>
            {
                head
            }
            _ => inside,
        };
        out.push_str(inside);
        rest = &rest[close..];
        if let Some(r) = rest.strip_prefix('>') {
            out.push('>');
            rest = r;
        }
    }
    out.push_str(rest);
    out
}

fn with_sources() -> Vm {
    let mut vm = Vm::new();
    for (name, source) in SOURCES {
        vm.provide_module(name, *source);
    }
    vm
}

/// The key and text of an error, or `""` and the value written.
fn run(vm: &mut Vm, source: &str) -> (String, String) {
    match vm.eval_text(source, None) {
        Ok(value) => {
            let mut out = String::new();
            vm.heap().write(value, &mut out);
            (String::new(), out)
        }
        Err(e) => {
            let key = vm.heap().name(e.key).unwrap_or("?").to_string();
            (key, e.text(vm.heap()))
        }
    }
}

#[test]
fn modules_give_what_guile_gives() {
    let mut failures = Vec::new();
    for &(source, key, text) in CASES {
        let (got_key, got) = run(&mut with_sources(), source);
        if (got_key.as_str(), got.as_str()) != (key, text) {
            failures.push(format!(
                "{source}\n  gave {got_key}: {got}\n  want {key}: {text}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {}:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}

/// A string as a Scheme literal.
fn literal(text: &str) -> String {
    let mut s = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

#[test]
fn guile_gives_the_table() {
    let dir = std::env::temp_dir().join(format!("lily-modules-{}", std::process::id()));
    for (name, source) in SOURCES {
        let mut path: PathBuf = dir.clone();
        for part in name.split(' ') {
            path.push(part);
        }
        path.set_extension("scm");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
    }
    // Each program's forms read and evaluated in turn in the current
    // module, which they may change; one line: the key and print-exception's
    // text, or a tab and the value.
    let runner = r#"(define (run text)
  (let ((p (open-input-string text)))
    (catch #t
      (lambda ()
        (let loop ((last *unspecified*))
          (let ((x (read p)))
            (if (eof-object? x)
                (begin (display "\t") (write last))
                (loop (primitive-eval x))))))
      (lambda (key . args)
        (display key) (display "\t")
        (print-exception (current-output-port) #f key args)))))
"#;
    let mut failures = Vec::new();
    for &(source, key, text) in CASES {
        let program = format!("{runner}(run {})\n", literal(source));
        let out = Command::new("guile")
            .args(["--no-auto-compile", "-L"])
            .arg(&dir)
            .args(["-c", &program])
            .output();
        let Ok(out) = out else {
            eprintln!("guile is not installed: the table is unverified against it");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        };
        let stdout = String::from_utf8(out.stdout).unwrap();
        let (got_key, got) = stdout.split_once('\t').unwrap_or((&stdout, ""));
        let got = without_addresses(got.trim_end_matches('\n'));
        if (got_key, got.as_str()) != (key, text) {
            failures.push(format!(
                "{source}\n  guile {got_key}: {got}\n  table {key}: {text}\n  {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        failures.is_empty(),
        "{} of {}:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}

#[test]
fn a_module_s_code_loads_once_when_first_used() {
    // Each load of (test once) bumps (test c)'s counter.
    let mut vm = with_sources();
    let source = "(define-module (test once)) ((@ (test c) bump!)) (define-public loads (@ (test c) counter))";
    vm.provide_module("test once", source);
    assert_eq!(run(&mut vm, "(@ (test c) counter)").1, "0");
    let used = "(use-modules (test once)) (use-modules (test once)) (resolve-module '(test once)) (list loads (@ (test once) loads))";
    assert_eq!(run(&mut vm, used).1, "(1 1)");
    assert_eq!(run(&mut vm, "(@ (test c) counter)").1, "1");
    // Loaded by `@`, `resolve-module` or `use-modules`, whichever is first.
    for first in [
        "((@ (test c) bump!))",
        "((module-ref (resolve-module '(test c)) 'bump!))",
        "(use-modules (test c)) (bump!)",
    ] {
        let mut vm = with_sources();
        assert_eq!(run(&mut vm, first).1, "1");
        assert_eq!(run(&mut vm, "(use-modules (test c)) (bump!)").1, "2");
    }
}

#[test]
fn a_slot_has_a_version_and_a_module() {
    let mut vm = with_sources();
    run(&mut vm, "(define-module (t v)) (define x 1)");
    let x = vm.slot("t v", "x").unwrap();
    assert_eq!(vm.slot_value(x), vm.heap_mut().integer(1).into());
    // Each `define` and `set!` gives a version no slot has had (MS6 step 3,
    // D235): later than any before.
    let first = vm.slot_version(x);
    run(&mut vm, "(set! x 2) (define x 3)");
    let third = vm.slot_version(x);
    assert!(third > first + 1, "{first} {third}");
    run(&mut vm, "(module-define! (current-module) 'x 4)");
    run(&mut vm, "(module-set! (current-module) 'x 5)");
    assert!(vm.slot_version(x) > third + 1);
    let mut module = String::new();
    vm.heap().write(vm.slot_module(x), &mut module);
    assert_eq!(module, "#<directory (t v)>");
    // An import's slot is the exporter's, whose version `set!` increments
    // from the importer too.
    run(&mut vm, "(use-modules (test c)) (set! counter 7)");
    let counter = vm.slot("t v", "counter").unwrap();
    assert_eq!(Some(counter), vm.slot("test c", "counter"));
    assert!(vm.slot_version(counter) > vm.slot_version(x));
    let mut module = String::new();
    vm.heap().write(vm.slot_module(counter), &mut module);
    assert_eq!(module, "#<directory (test c)>");
    // A reference alone does not make a variable.
    run(&mut vm, "(define (f) never-defined)");
    assert_eq!(vm.slot("t v", "never-defined"), None);
    assert_eq!(vm.slot("no such", "x"), None);
}

#[test]
fn reads_are_tracked_only_when_tracking_is_on() {
    let mut vm = with_sources();
    run(
        &mut vm,
        "(use-modules (test c)) (define a 1) (define b 2) (define (f) (+ a counter))",
    );
    run(&mut vm, "(f)");
    assert!(vm.take_reads().is_empty(), "tracking is off by default");
    vm.track_reads(true);
    run(&mut vm, "(f) (f)");
    let reads = vm.take_reads();
    let slots: Vec<_> = ["f", "a", "counter", "+"]
        .iter()
        .map(|n| vm.slot("guile-user", n).unwrap())
        .collect();
    // Each read once, `+` through its inline instruction.
    assert_eq!(reads.len(), slots.len(), "{reads:?}");
    for slot in &slots {
        assert!(reads.contains(slot), "{slot:?} in {reads:?}");
    }
    assert!(!reads.contains(&vm.slot("guile-user", "b").unwrap()));
    // Taking them clears them; a reference resolved now is tracked too.
    run(&mut vm, "(define (g) (bump!)) (g)");
    let reads = vm.take_reads();
    assert!(reads.contains(&vm.slot("test c", "bump!").unwrap()));
    assert!(reads.contains(&vm.slot("test c", "counter").unwrap()));
    vm.track_reads(false);
    run(&mut vm, "(g)");
    assert!(vm.take_reads().is_empty());
}

#[test]
fn set_location_sets_lily_scm_s_own_fluid() {
    let mut vm = with_sources();
    let here = vm.heap_mut().string("file.ly:1:2");
    assert_eq!(vm.set_location(here), Value::FALSE);
    let old = run(&mut vm, "(@@ (lily) %location)").1;
    assert_eq!(old, "#<fluid>");
    // As lily.scm begins, after its use-modules.
    let lily = "(define-module (lily))
(define %parser (make-fluid))
(define %location (make-fluid))
(define-public (*parser*) (fluid-ref %parser))
(define-public (*location*) (fluid-ref %location))";
    run(&mut vm, lily);
    assert_eq!(vm.location(), Value::FALSE, "lily.scm's fluid is new");
    let there = vm.heap_mut().string("file.ly:3:4");
    assert_eq!(vm.set_location(there), Value::FALSE);
    let seen = run(
        &mut vm,
        "(define-module (score) #:use-module (lily)) (define (where) (*location*)) (where)",
    );
    assert_eq!(seen.1, "\"file.ly:3:4\"");
    assert_eq!(
        run(&mut vm, "(fluid-ref (@@ (lily) %location))").1,
        "\"file.ly:3:4\""
    );
    assert_eq!(vm.set_location(Value::FALSE), there);
    assert_eq!(run(&mut vm, "(where)").1, "#f");
}

#[test]
fn the_collector_keeps_modules_and_their_variables() {
    let mut vm = with_sources();
    run(
        &mut vm,
        "(define-module (t gc)) (define-public kept (list 'kept)) (define-module (t other))",
    );
    run(
        &mut vm,
        "(let loop ((i 0) (l '())) (if (< i 300000) (loop (+ i 1) (cons i '())) 'done))",
    );
    vm.collect();
    let (key, text) = run(&mut vm, "(use-modules (t gc)) (list kept (current-module))");
    assert_eq!(
        (key.as_str(), text.as_str()),
        ("", "((kept) #<directory (t other)>)")
    );
}

/// The module of a score, as the parser makes one (`Vm::make_module`):
/// nameless, using `(guile)` and the modules given, the last first; its
/// definitions are its own, and shadow what it imports.
#[test]
fn a_nameless_module_uses_what_it_is_given() {
    let mut vm = Vm::new();
    vm.eval_text(
        "(define-module (a)) (define-public x 1) (define-public y 1)",
        None,
    )
    .unwrap();
    vm.eval_text("(define-module (b)) (define-public y 2)", None)
        .unwrap();
    let module = vm.make_module(&["a", "b"]).unwrap();
    let other = vm.make_module(&["a"]).unwrap();
    assert_eq!(vm.make_module(&["no such"]), None);
    let fixnum = |v: Option<Value>| v.and_then(Value::as_fixnum);
    assert_eq!(fixnum(vm.module_value(module, "x")), Some(1));
    assert_eq!(fixnum(vm.module_value(module, "y")), Some(2));
    assert!(vm.module_value(module, "car").is_some());
    assert!(vm.module_define(module, "x", Value::fixnum(3).unwrap()));
    assert_eq!(fixnum(vm.module_value(module, "x")), Some(3));
    assert_eq!(fixnum(vm.module_value(other, "x")), Some(1));
    assert_eq!(vm.module_value(module, "z"), None);
    assert!(!vm.module_define(Value::FALSE, "x", Value::NULL));
    let mut out = String::new();
    vm.heap().write(module, &mut out);
    assert_eq!(out, "#<module (%anonymous)>");
}

/// `defined?`, which LilyPond's `init.ly` calls: in the current module or
/// the one given, with guile 3.0.11's errors.
#[test]
fn defined_looks_in_a_module() {
    let mut vm = Vm::new();
    let text = |vm: &mut Vm, form: &str| match vm.eval_text(form, None) {
        Ok(value) => {
            let mut out = String::new();
            vm.heap().write(value, &mut out);
            out
        }
        Err(e) => e.text(vm.heap()),
    };
    vm.eval_text("(define-module (m)) (define x 1)", None)
        .unwrap();
    assert_eq!(
        text(
            &mut vm,
            "(list (defined? 'car) (defined? 'x) (defined? 'nope))"
        ),
        "(#t #t #f)"
    );
    assert_eq!(
        text(&mut vm, "(defined? 'x (resolve-module '(guile)))"),
        "#f"
    );
    assert_eq!(
        text(&mut vm, "(defined? 1)"),
        "In procedure defined?: Wrong type argument in position 1 (expecting symbol): 1"
    );
    assert_eq!(
        text(&mut vm, "(defined? 'car 5)"),
        "In procedure defined?: Wrong type argument in position 2 (expecting module): 5"
    );
}

/// K2 (MS2 step 11): an anonymous module, a score's or a scope's, is
/// collected once nothing reaches it, and its slots are used again; one
/// whose procedure is still reachable is kept, and the procedure still
/// reads its variables.
#[test]
fn anonymous_modules_are_collected_when_nothing_reaches_them() {
    let mut vm = Vm::new();
    let kept = vm.make_module(&[]).expect("a module");
    let previous = vm.set_current_module(kept);
    vm.eval_text("(define y 42) (define (get) y)", None)
        .unwrap();
    vm.set_current_module(previous);
    let get = vm.module_value(kept, "get").expect("defined");
    vm.define("get-y", get);
    vm.collect();
    let live = vm.heap().stats().live_bytes;
    for i in 0..50 {
        let module = vm.make_module(&[]).expect("a module");
        let previous = vm.set_current_module(module);
        let text = format!("(define big (make-list 1000 {i})) (define (f) big)");
        vm.eval_text(&text, None).unwrap();
        vm.set_current_module(previous);
        vm.collect();
    }
    assert_eq!(vm.heap().stats().live_bytes, live);
    let value = vm.eval_text("(get-y)", None).unwrap();
    assert_eq!(value.as_fixnum(), Some(42));
}

/// A procedure still running keeps its module, though nothing else reaches
/// it any more: here it drops the only reference to itself, called in tail
/// position, and collects before it reads its module's variable.
#[test]
fn a_running_procedure_keeps_its_module() {
    let mut vm = Vm::new();
    let m = vm.make_module(&[]).expect("a module");
    let previous = vm.set_current_module(m);
    vm.eval_text("(define big (list 1 2 3)) (define (f c) (set-car! c #f) (gc) (make-list 100000 0) (gc) big)", None).unwrap();
    vm.set_current_module(previous);
    let f = vm.module_value(m, "f").unwrap();
    let cell = vm.heap_mut().cons(f, Value::NULL);
    vm.define("c", cell);
    let value = vm.eval_text("(define (h c) ((car c) c)) (h c)", None);
    let mut text = String::new();
    match value {
        Ok(v) => vm.heap().write(v, &mut text),
        Err(e) => text = e.text(vm.heap()),
    }
    assert_eq!(text, "(1 2 3)");
}
