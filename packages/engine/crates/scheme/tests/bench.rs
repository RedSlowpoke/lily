//! S2's compiler and VM (chapter 03 §5.3, R2): chapter 03 Appendix B's
//! benchmarks b1–b8 from `tools/bench/` print what the appendix says, the
//! special forms, closures, tail calls and multiple values give what Guile
//! 3.0.11 gives, and no call uses the native stack. With `guile` installed,
//! the benchmarks and the table are also run by guile and compared.
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use lily_engine_scheme::{MAX_FRAMES, MAX_NESTING, Value, Vm};

fn bench_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/bench")
}

/// The benchmarks, `b1-…scm` to `b8-…scm` as `run.mjs` takes them, with the
/// output each one's first line says it prints.
fn benchmarks() -> Vec<(PathBuf, String)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(bench_dir())
        .expect("tools/bench")
        .map(|e| e.unwrap().path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let mut chars = name.chars();
            chars.next() == Some('b')
                && chars.next().is_some_and(|c| c.is_ascii_digit())
                && name.ends_with(".scm")
        })
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).unwrap();
            let first = text.lines().next().unwrap_or("");
            let (_, expected) = first
                .split_once(", prints ")
                .unwrap_or_else(|| panic!("{} says what it prints", path.display()));
            (path, expected.to_string())
        })
        .collect()
}

/// `guile --no-auto-compile` on a file or a program: its standard output,
/// or `None` without guile.
fn guile(args: &[&str]) -> Option<String> {
    let out = Command::new("guile")
        .arg("--no-auto-compile")
        .args(args)
        .output()
        .ok()?;
    assert!(
        out.status.success(),
        "guile failed on {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(String::from_utf8(out.stdout).unwrap())
}

#[test]
fn benchmarks_print_what_appendix_b_says() {
    let benches = benchmarks();
    let names: Vec<String> = benches
        .iter()
        .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        [
            "b1-fib.scm",
            "b2-tak.scm",
            "b3-loop.scm",
            "b4-alist.scm",
            "b5-lists.scm",
            "b6-strings.scm",
            "b7-closures.scm",
            "b8-load.scm"
        ]
    );
    for ((path, expected), name) in benches.iter().zip(&names) {
        let text = std::fs::read_to_string(path).unwrap();
        let mut vm = Vm::new();
        let start = Instant::now();
        let result = vm.eval_text(&text, Some(name));
        let ms = start.elapsed().as_secs_f64() * 1e3;
        if let Err(e) = result {
            panic!("{name}: {}", e.text(vm.heap()));
        }
        let output = vm.take_output();
        assert_eq!(output.trim_end(), expected, "{name}");
        let stats = vm.heap().stats();
        eprintln!(
            "{name}: {ms:.1} ms, {} collections, {} objects live",
            stats.collections, stats.live
        );
    }
}

#[test]
fn b8_is_what_its_script_writes() {
    let script = bench_dir().join("gen-b8.mjs");
    let Ok(out) = Command::new("node").arg(&script).output() else {
        eprintln!("node is not installed: b8-load.scm is unchecked against gen-b8.mjs");
        return;
    };
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let file = std::fs::read(bench_dir().join("b8-load.scm")).unwrap();
    assert!(file == out.stdout, "b8-load.scm is not gen-b8.mjs's output");
    // Its `check`, which the benchmark leaves uncalled, reads each datum
    // back through eval; guile gives 244650.
    let mut vm = Vm::new();
    vm.eval_text(std::str::from_utf8(&file).unwrap(), None)
        .unwrap();
    assert_eq!(eval(&mut vm, "(check 0 0)"), "244650");
}

#[test]
fn guile_prints_the_same_for_the_benchmarks() {
    for (path, expected) in benchmarks() {
        let Some(output) = guile(&[path.to_str().unwrap()]) else {
            eprintln!("guile is not installed: the benchmarks are unverified against it");
            return;
        };
        assert_eq!(output.trim_end(), expected, "{}", path.display());
    }
}

/// Programs, and what Guile 3.0.11 writes for the last form's value.
const CASES: &[(&str, &str)] = &[
    // special forms
    ("(if #f #f)", "#<unspecified>"),
    ("(if '() 1 2)", "1"),
    ("(define x 5) (set! x (* x 2)) x", "10"),
    (
        "(let ((x 1) (y 2)) (let ((x y) (y x)) (list x y)))",
        "(2 1)",
    ),
    ("(let* ((x 1) (y (+ x 1))) (* x y))", "2"),
    (
        "(letrec ((ev? (lambda (n) (if (= n 0) #t (od? (- n 1))))) (od? (lambda (n) (if (= n 0) #f (ev? (- n 1)))))) (list (ev? 10) (od? 7)))",
        "(#t #t)",
    ),
    ("(letrec* ((a 1) (b (+ a 1))) (list a b))", "(1 2)"),
    (
        "(let loop ((i 0) (acc '())) (if (= i 3) (reverse acc) (loop (+ i 1) (cons i acc))))",
        "(0 1 2)",
    ),
    (
        "(cond ((assv 2 '((1 . a) (2 . b))) => cdr) (else 'no))",
        "b",
    ),
    ("(cond ((> 1 2) 'a) ((< 1 2)) (else 'c))", "#t"),
    ("(cond (#f 1))", "#<unspecified>"),
    (
        "(case (* 2 3) ((2 3 5 7) 'prime) ((1 4 6 8 9) 'composite) (else 'other))",
        "composite",
    ),
    ("(case 'z ((a) 1) (else => (lambda (x) (list x))))", "(z)"),
    (
        "(list (and) (or) (and 1 2) (or #f 3) (and 1 #f 3) (or #f #f))",
        "(#t #f 2 3 #f #f)",
    ),
    (
        "(list (when (= 1 1) 'a 'b) (unless (= 1 1) 'c))",
        "(b #<unspecified>)",
    ),
    ("(begin 1 2 3)", "3"),
    ("(define (f . xs) xs) (list (f) (f 1 2))", "(() (1 2))"),
    ("((lambda (a b . c) (list a b c)) 1 2 3 4)", "(1 2 (3 4))"),
    ("'#(1 \"a\" #\\b)", "#(1 \"a\" #\\b)"),
    (
        "(let ((if (lambda (a b c) 'shadowed))) (if 1 2 3))",
        "shadowed",
    ),
    (
        "(define (f) (define a 1) (define (g) (* a 10)) (g)) (f)",
        "10",
    ),
    ("(define (f) (display 1) (define b 2) b) (f)", "2"),
    // closures, boxes and assignment
    (
        "(define (counter) (let ((n 0)) (lambda () (set! n (+ n 1)) n))) (define c (counter)) (c) (c) (c)",
        "3",
    ),
    (
        "(define (adders) (let loop ((i 0) (acc '())) (if (= i 3) acc (loop (+ i 1) (cons (lambda (x) (+ x i)) acc))))) (map (lambda (f) (f 10)) (adders))",
        "(12 11 10)",
    ),
    (
        "(define (f x) (lambda () (set! x (+ x 1)) x)) (define g (f 5)) (g) (g)",
        "7",
    ),
    (
        "(define (f) (letrec ((a (lambda () b)) (b 2)) (a))) (f)",
        "2",
    ),
    (
        "(let loop ((i 0)) (if (< i 3) (begin ((lambda () (loop (+ i 1))))) i))",
        "3",
    ),
    (
        "(define (f p) (car p)) (define (car x) 'mine) (f '(1))",
        "mine",
    ),
    ("(define (f a b) (+ a b)) (define + -) (f 5 3)", "2"),
    // multiple values, apply, map and for-each
    (
        "(call-with-values (lambda () (values 1 2 3)) list)",
        "(1 2 3)",
    ),
    ("(call-with-values (lambda () (values)) list)", "()"),
    ("(call-with-values (lambda () 7) list)", "(7)"),
    ("(list (values 1 2))", "(1)"),
    ("(+ 1 (values 2 3))", "3"),
    (
        "(define (two) (values 1 2)) (call-with-values two cons)",
        "(1 . 2)",
    ),
    ("(begin (values) 5)", "5"),
    ("(apply + 1 2 '(3 4))", "10"),
    ("(apply apply list '((1 2)))", "(1 2)"),
    (
        "(map + '(1 2 3) '(10 20 30) '(100 200 300))",
        "(111 222 333)",
    ),
    ("(map (lambda (x) (* x x)) '(1 2 3))", "(1 4 9)"),
    (
        "(let ((acc '())) (for-each (lambda (a b) (set! acc (cons (+ a b) acc))) '(1 2) '(3 4)) acc)",
        "(6 4)",
    ),
    ("(map cadr '((a 1) (b 2)))", "(1 2)"),
    // primitives
    (
        "(list (quotient 17 5) (remainder -17 5) (modulo -17 5) (expt 2 70) (/ 6 4) (exact->inexact 1/3))",
        "(3 -2 3 1180591620717411303424 3/2 0.3333333333333333)",
    ),
    (
        "(list (< 1 2 3) (< 1 3 2) (= 1 1.0) (max 1 2.0) (min 1 2) (abs -5) (1+ 4))",
        "(#t #f #t 2.0 1 5 5)",
    ),
    ("(* 4611686018427387904 2)", "9223372036854775808"),
    ("(+ 2305843009213693951 1)", "2305843009213693952"),
    ("(- -2305843009213693952 1)", "-2305843009213693953"),
    (
        "(list (string->symbol \"a b\") (symbol->string 'abc) (string-append \"x\" \"y\" \"z\") (substring \"hello\" 1 3))",
        "(#{a b}# \"abc\" \"xyz\" \"el\")",
    ),
    (
        "(list (string->list \"héllo\") (list->string (list #\\a #\\λ)) (string-length \"λx→é\") (string-ref \"λx\" 1))",
        "((#\\h #\\é #\\l #\\l #\\o) \"aλ\" 4 #\\x)",
    ),
    (
        "(list (char-upcase #\\a) (char-upcase #\\ß) (make-string 3 #\\-) (number->string 255 16) (string->number \"1e3\"))",
        "(#\\A #\\ß \"---\" \"ff\" 1000.0)",
    ),
    (
        "(list (memq 'c '(a b c d)) (member \"b\" '(\"a\" \"b\")) (assq 'x '((y . 1))) (assoc 2.0 '((1 . a) (2 . b))))",
        "((c d) (\"b\") #f #f)",
    ),
    (
        "(list (append '(1) '(2 3) '() 4) (reverse '(1 2 3)) (length '(1 2)) (list-tail '(1 2 3) 1) (list-ref '(a b) 1) (cons* 1 2 '(3)))",
        "((1 2 3 . 4) (3 2 1) 2 (2 3) b (1 2 3))",
    ),
    (
        "(let ((v (make-vector 3 0))) (vector-set! v 1 'x) (list v (vector-length v) (vector->list v) (list->vector '(1 2))))",
        "(#(0 x 0) 3 (0 x 0) #(1 2))",
    ),
    (
        "(list (equal? '(1 #(2 \"x\")) (list 1 (vector 2 \"x\"))) (eqv? 2.0 2.0) (eq? '() '()) (list? '(1 . 2)))",
        "(#t #t #t #f)",
    ),
    (
        "(list (procedure? car) (procedure? (lambda () 1)) (procedure? 'car))",
        "(#t #t #f)",
    ),
];

#[test]
fn the_vm_gives_what_guile_gives() {
    let mut failures = Vec::new();
    for (source, expected) in CASES {
        let mut vm = Vm::new();
        match vm.eval_text(source, None) {
            Ok(value) => {
                let mut out = String::new();
                vm.heap().write(value, &mut out);
                if out != *expected {
                    failures.push(format!("{source}\n  gave {out}\n  want {expected}"));
                }
            }
            Err(e) => failures.push(format!("{source}\n  threw {}", e.text(vm.heap()))),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn guile_gives_the_table() {
    let mut program = String::from(
        "(define (run text) (let ((m (make-fresh-user-module)) (p (open-input-string text))) (let loop ((last *unspecified*)) (let ((x (read p))) (if (eof-object? x) (begin (write last) (newline)) (loop (eval x m)))))))\n",
    );
    for (source, _) in CASES {
        program.push_str("(run ");
        let mut s = String::new();
        // Written as a Scheme string literal.
        s.push('"');
        for c in source.chars() {
            match c {
                '"' => s.push_str("\\\""),
                '\\' => s.push_str("\\\\"),
                c => s.push(c),
            }
        }
        s.push('"');
        program.push_str(&s);
        program.push_str(")\n");
    }
    let path = std::env::temp_dir().join(format!("lily-vm-cases-{}.scm", std::process::id()));
    std::fs::write(&path, &program).unwrap();
    let output = guile(&[path.to_str().unwrap()]);
    let _ = std::fs::remove_file(&path);
    let Some(output) = output else {
        eprintln!("guile is not installed: the table is unverified against it");
        return;
    };
    // The display in one case is not part of its value.
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), CASES.len(), "{output}");
    for ((source, expected), line) in CASES.iter().zip(lines) {
        let line = line
            .strip_prefix('1')
            .filter(|_| source.contains("(display 1)"))
            .unwrap_or(line);
        assert_eq!(line, *expected, "guile on {source}");
    }
}

/// Runs `f` on a thread with a small native stack, which a VM that used
/// the native stack for Scheme calls would overflow.
fn on_small_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(512 << 10)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

fn eval(vm: &mut Vm, text: &str) -> String {
    match vm.eval_text(text, None) {
        Ok(value) => {
            let mut out = String::new();
            vm.heap().write(value, &mut out);
            out
        }
        Err(e) => format!("threw {}", e.text(vm.heap())),
    }
}

#[test]
fn deep_recursion_uses_no_native_stack() {
    on_small_stack(|| {
        let mut vm = Vm::new();
        // A million calls deep, not in tail position.
        let count = "(define (count n) (if (= n 0) 0 (+ 1 (count (- n 1))))) (count 1000000)";
        assert_eq!(eval(&mut vm, count), "1000000");
        // Guile's map recurses once per element.
        let big = "(define big (let loop ((i 0) (acc '())) (if (= i 1000000) acc (loop (+ i 1) (cons i acc)))))";
        eval(&mut vm, big);
        assert_eq!(eval(&mut vm, "(length (map 1+ big))"), "1000000");
        assert_eq!(
            eval(&mut vm, "(apply + (map (lambda (x y) 1) big big))"),
            "1000000"
        );
        // A chain of closures, each calling the next through apply, not in
        // tail position.
        let chain = "(define (chain n) (if (= n 0) (lambda args (car args)) (let ((next (chain (- n 1)))) (lambda args (+ 1 (apply next args))))))";
        eval(&mut vm, chain);
        assert_eq!(eval(&mut vm, "((chain 100000) 0)"), "100000");
        // Data this deep is read and compiled on the native stack, so it
        // stays at what the reader allows.
        assert_eq!(
            eval(
                &mut vm,
                "(let loop ((i 0) (x '())) (if (= i 100000) (length x) (loop (+ i 1) (list x))))"
            ),
            "1"
        );
    });
}

#[test]
fn tail_calls_take_no_frames() {
    on_small_stack(|| {
        let mut vm = Vm::new();
        let n = MAX_FRAMES + 1000;
        // Mutual recursion, and tail calls through if, cond, case, and, or,
        // when, apply and call-with-values, each more deep than the frames
        // the VM allows.
        let program = format!(
            "(define (ev? n) (if (= n 0) #t (od? (- n 1))))
             (define (od? n) (cond ((= n 0) #f) (else (ev? (- n 1)))))
             (define (c n) (case n ((0) 'done) (else (and #t (or #f (when #t (apply c (list (- n 1)))))))))
             (define (v n) (if (= n 0) 'ok (call-with-values (lambda () (values (- n 1))) v)))
             (list (ev? {n}) (c {n}) (v {n}))"
        );
        assert_eq!(eval(&mut vm, &program), "(#t done ok)");
    });
}

#[test]
fn long_forms_compile_without_nesting() {
    // A cond or case of 20,000 clauses and a sum of 5,000 arguments are
    // flat: compiling them does not recurse once per clause.
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            let mut vm = Vm::new();
            let n = 20000;
            let mut cond = String::from("(define (f x) (cond");
            let mut case = String::from("(define (g x) (case x");
            for i in 0..n {
                cond.push_str(&format!(" ((= x {i}) {i})"));
                case.push_str(&format!(" (({i}) {i})"));
            }
            cond.push_str(" ((assv x '((-1 . neg))) => cdr) ((= x -2)) (else 'none)))");
            case.push_str(" (else => list)))");
            eval(&mut vm, &cond);
            eval(&mut vm, &case);
            let calls = format!(
                "(list (f {}) (f -1) (f -2) (f -3) (g {}) (g 'z))",
                n - 1,
                n - 1
            );
            assert_eq!(eval(&mut vm, &calls), "(19999 neg #t none 19999 (z))");
            let sum = format!("(+{})", " 1".repeat(5000));
            assert_eq!(eval(&mut vm, &sum), "5000");
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn nesting_beyond_the_limit_is_a_syntax_error() {
    // Both passes recurse once per level of nesting, on the native stack
    // (an unoptimised build needs more than wasm's 1 MiB for the limit).
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let mut vm = Vm::new();
            let deepest = (MAX_NESTING - 1).to_string();
            for (open, close, value) in [
                ("(+ 1 ", ")", deepest.as_str()),
                ("(let ((x 1)) ", ")", "0"),
                ("(define (f) ", ") (f)", "0"),
            ] {
                let nested = |n: usize| format!("{}0{}", open.repeat(n), close.repeat(n));
                assert_eq!(eval(&mut vm, &nested(MAX_NESTING - 1)), value, "{open}");
                let error = vm.eval_text(&nested(MAX_NESTING + 1), None).unwrap_err();
                assert_eq!(vm.heap().name(error.key), Some("syntax-error"), "{open}");
                assert!(
                    error
                        .text(vm.heap())
                        .contains("expression nested too deeply")
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn a_runaway_recursion_is_an_error() {
    on_small_stack(|| {
        let mut vm = Vm::new();
        let error = vm
            .eval_text("(define (f n) (+ 1 (f n))) (f 1)", None)
            .unwrap_err();
        assert_eq!(vm.heap().name(error.key), Some("stack-overflow"));
        // The VM is whole after the error.
        assert_eq!(eval(&mut vm, "(f 'x)"), "threw Stack overflow");
        assert_eq!(eval(&mut vm, "(+ 1 2)"), "3");
    });
}

#[test]
fn errors_unwind_to_the_caller() {
    let mut vm = Vm::new();
    let cases = [
        (
            "(car '())",
            "wrong-type-arg",
            "In procedure car: Wrong type (expecting pair): ()",
        ),
        (
            "(undefined-thing)",
            "unbound-variable",
            "Unbound variable: undefined-thing",
        ),
        (
            "(set! nowhere 1)",
            "unbound-variable",
            "Unbound variable: nowhere",
        ),
        (
            "((lambda (x) x))",
            "wrong-number-of-args",
            "Wrong number of arguments to #<procedure 0 (a)>",
        ),
        ("(1 2)", "wrong-type-arg", "Wrong type to apply: 1"),
        (
            "(define (z) (values)) (+ 1 (z))",
            "misc-error",
            "Zero values returned to single-valued continuation",
        ),
        (
            "(map car 5)",
            "wrong-type-arg",
            "In procedure map: Not a list: 5",
        ),
        (
            "(map + '(1) '(1 2))",
            "wrong-type-arg",
            "In procedure map: List of wrong length: (1 2)",
        ),
        (
            "(if)",
            "syntax-error",
            "Syntax error:\nunknown location: source expression failed to match any pattern in form (if)",
        ),
        (
            "(+ 1 'a)",
            "wrong-type-arg",
            "In procedure +: Wrong type argument in position 2: a",
        ),
        (
            "(vector-ref (vector 1) 1)",
            "out-of-range",
            "Value out of range: 1",
        ),
        (
            "(",
            "read-error",
            "#<unknown port>:1:2: unexpected end of input while searching for: )",
        ),
    ];
    for (source, key, text) in cases {
        let depth = vm.heap().root_depth();
        let error = vm.eval_text(source, None).unwrap_err();
        assert_eq!(vm.heap().name(error.key), Some(key), "{source}");
        let got = error.text(vm.heap());
        // An anonymous procedure is named by its index, which varies.
        if !text.contains("#<procedure 0") {
            assert_eq!(got, text, "{source}");
        } else {
            assert!(
                got.starts_with("Wrong number of arguments to #<procedure "),
                "{got}"
            );
        }
        assert_eq!(vm.heap().root_depth(), depth);
        assert_eq!(eval(&mut vm, "(* 6 7)"), "42");
    }
}

#[test]
fn the_collector_keeps_what_the_vm_holds() {
    let mut vm = Vm::new();
    // Lists that outlive many collections, held by locals, closures, boxes
    // and globals while garbage is made.
    let program = "
        (define keep (list 1 2 3))
        (define (churn n) (let loop ((i 0) (junk '())) (if (= i n) (length junk) (loop (+ i 1) (if (= 0 (remainder i 1000)) '() (cons (make-string 20) junk))))))
        (define (hold) (let ((mine (map (lambda (x) (* x 10)) keep)) (box 0))
          (set! box (lambda () mine))
          (churn 400000)
          (list mine ((lambda () (box))) keep)))
        (hold)";
    assert_eq!(eval(&mut vm, program), "((10 20 30) (10 20 30) (1 2 3))");
    assert!(vm.heap().stats().collections > 0, "nothing was collected");
    let before = vm.heap().objects();
    vm.collect();
    assert!(vm.heap().objects() <= before);
    assert_eq!(eval(&mut vm, "(map car (list keep keep))"), "(1 1)");
}

#[test]
fn procedures_print_as_guile_prints_them() {
    let mut vm = Vm::new();
    let cases = [
        ("car", "#<procedure car (_)>"),
        ("list", "#<procedure list _>"),
        ("vector-ref", "#<procedure vector-ref (_ _)>"),
        ("(define (f x y z) x) f", "#<procedure f (a b c)>"),
        ("(define (g x y . z) x) g", "#<procedure g (a b . rest)>"),
        ("(define (h . z) z) h", "#<procedure h rest>"),
        ("(let loop ((i 0)) loop)", "#<procedure loop (a)>"),
    ];
    for (source, printed) in cases {
        assert_eq!(eval(&mut vm, source), printed, "{source}");
    }
    let value = vm.global("map").unwrap();
    assert_ne!(value, Value::UNDEFINED);
}
