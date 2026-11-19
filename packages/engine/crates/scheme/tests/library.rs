//! S3, step 10b: the Guile modules LilyPond's library files import, `(lily)`'s
//! own definitions, and `lily-library.scm` and `music-functions.scm` loaded
//! from the installed LilyPond 2.26.0's `scm/lily/`.
//!
//! - The modules (`(srfi srfi-11)`, `(srfi srfi-39)`, `(ice-9 match)`,
//!   `(ice-9 receive)`, `(ice-9 optargs)`, `(ice-9 pretty-print)`, the ports,
//!   `(ice-9 arrays)` and `(oop goops)`) give what guile 3.0.11 gives on a
//!   table of programs, each also run by guile in a fresh process when guile
//!   is installed.
//! - `(lily)`'s `ly:assoc-get`, `ly:chain-assoc-get`, `ly:version`,
//!   `ly:make-regex`, and what `lily.scm`'s init data calls as it loads
//!   (`ly:dimension?`, the type predicates and `ly:add-interface`; the
//!   stencils' are layout's since MS3 step 2), give what the installed LilyPond
//!   gives, its programs run in one `lilypond` process in its `(lily)`.
//! - The two files load into `(lily)` in `lily.scm`'s order, read from the
//!   installed LilyPond and never copied here, and a probe of their
//!   definitions gives what LilyPond's own `(lily)` gives. The music crate
//!   gives the moments and durations they make as they load; `symbol-list?`,
//!   which `music-functions.scm` calls as it loads, is read from the
//!   installed `c++.scm`, which `lily.scm` loads between them; `(lily
//!   display-lily)`, which `music-functions.scm` imports, is an empty
//!   stand-in. Skipped without LilyPond 2.26.0.
use std::path::{Path, PathBuf};
use std::process::Command;

use lily_engine_scheme::{Heap, Vm, read};

/// Programs and what they give: the last form's value as `write` writes
/// it, or the key and the text of the error, as in `tests/macros.rs`.
const CASES: &[(&str, &str, &str)] = &[
    // `(srfi srfi-2)` and `(ice-9 list)` (MS2 step 13).
    (
        "(use-modules (srfi srfi-2)) (list (and-let* ((x 5) ((> x 3)) (y (* x 2))) y) (and-let* ((x #f)) 'no) (and-let* ((x 5))) (and-let* ()) (let ((z 1)) (and-let* (z) 'z)))",
        "",
        "(10 #f 5 #t z)",
    ),
    (
        "(use-modules (ice-9 list)) (list (rassoc 2 '((a . 1) (b . 2))) (rassq 'x '((1 . y))) (rassoc 2.0 '((a . 2)) =) (rassv 2 '((c . 2))))",
        "",
        "((b . 2) #f (a . 2) (c . 2))",
    ),
    // `(ice-9 match)`.
    (
        "(use-modules (ice-9 match)) (match '(1 2 3) ((a b ...) (list a b)))",
        "",
        "(1 (2 3))",
    ),
    (
        "(use-modules (ice-9 match)) (match '(1 2 3 4) ((a ... b c) (list a b c)))",
        "",
        "((1 2) 3 4)",
    ),
    (
        "(use-modules (ice-9 match)) (match #(1 2 3) (#(a b c) (+ a b c)))",
        "",
        "6",
    ),
    (
        "(use-modules (ice-9 match)) (match '(1 (2 3)) ((a (b c)) (=> fail) (if (= a 1) (fail) 'x)) (_ 'failed))",
        "",
        "failed",
    ),
    (
        "(use-modules (ice-9 match)) (list (match '(1 1) ((a a) 'same) (_ 'diff)) (match '(1 2) ((a a) 'same) (_ 'diff)))",
        "",
        "(same diff)",
    ),
    (
        "(use-modules (ice-9 match)) (list (match 'x ('x 1)) (match \"s\" (\"s\" 'str) (_ 'no)) (match '(#t #f) ((#t #f) 'bools)))",
        "",
        "(1 str bools)",
    ),
    (
        "(use-modules (ice-9 match)) (match '(1 . 2) (((? number? a) . (= 1+ b)) (list a b)))",
        "",
        "(1 3)",
    ),
    (
        "(use-modules (ice-9 match)) (match '(a 1 #(2 3)) (`(a ,x #(2 ,y)) (list x y)))",
        "",
        "(1 3)",
    ),
    (
        "(use-modules (ice-9 match)) (match '((1 2) (3 4)) (((a b) ...) (list a b)))",
        "",
        "((1 3) (2 4))",
    ),
    (
        "(use-modules (ice-9 match)) (match '(1 2) ((and l (a . _)) (list l a)))",
        "",
        "((1 2) 1)",
    ),
    (
        "(use-modules (ice-9 match)) (list (match 5 ((or (? string?) (? number?)) 'ok)) (match 5 ((not 5) 'no) (_ 'yes)))",
        "",
        "(ok yes)",
    ),
    (
        "(use-modules (ice-9 match)) (list ((match-lambda* ((a b) (+ a b))) 1 2) ((match-lambda ((a . b) b)) '(1 2)))",
        "",
        "(3 (2))",
    ),
    (
        "(use-modules (ice-9 match)) (match-let* (((a . b) '(1 2)) ((c) b)) (list a c))",
        "",
        "(1 2)",
    ),
    (
        "(use-modules (ice-9 match)) (let ((b 5)) (match-let (((a . b) '(1 2)) (c b)) (list a b c)))",
        "",
        "(1 (2) 5)",
    ),
    (
        "(use-modules (ice-9 match)) (match-let loop ((x 3) (acc '())) (if (= x 0) acc (loop (- x 1) (cons x acc))))",
        "",
        "(1 2 3)",
    ),
    (
        "(use-modules (ice-9 match)) (list (match '(1 2 3) ((a ..1) a)) (match '() ((a ..1) a) (_ 'none)))",
        "",
        "((1 2 3) none)",
    ),
    (
        "(use-modules (ice-9 match)) (list (match '(1 2 3) ((1 x ...) x) (_ 'no)) (match '(a b c) ((x ... 'c) x)))",
        "",
        "((2 3) (a b))",
    ),
    (
        "(use-modules (ice-9 match)) (let ((v 5) (fail 6)) (match '(1 2) ((a . rest) (list a rest v fail))))",
        "",
        "(1 (2) 5 6)",
    ),
    (
        "(use-modules (ice-9 match)) (define (f lst) (match lst (() 'empty) ((x . y) x))) (list (f '()) (f '(3)))",
        "",
        "(empty 3)",
    ),
    (
        "(use-modules (ice-9 match)) (match-let (((a b) '(1))) a)",
        "match-error",
        "Throw to key `match-error' with args `(\"match\" \"no matching pattern\" (1))'.",
    ),
    (
        "(use-modules (ice-9 match)) ((match-lambda ((a) a)) 5)",
        "match-error",
        "Throw to key `match-error' with args `(\"match\" \"no matching pattern\" 5)'.",
    ),
    // `(srfi srfi-11)`, `(srfi srfi-39)`, `(ice-9 receive)`.
    (
        "(use-modules (srfi srfi-11)) (let-values (((a b) (values 1 2)) ((c . d) (values 3 4 5)) (e (values 6 7))) (list a b c d e))",
        "",
        "(1 2 3 (4 5) (6 7))",
    ),
    (
        "(use-modules (srfi srfi-11)) (let ((a 1)) (let-values (((a) (values 2)) ((b) (values a))) (list a b)))",
        "",
        "(2 1)",
    ),
    (
        "(use-modules (srfi srfi-11)) (let*-values (((a b) (values 1 2)) ((c) (values (+ a b)))) (define d 4) (list a b c d))",
        "",
        "(1 2 3 4)",
    ),
    (
        "(use-modules (srfi srfi-39)) (define p (make-parameter 1)) (define q (make-parameter 2 -)) (list (with-parameters* (list p q) (list 3 4) (lambda () (list (p) (q)))) (p) (q))",
        "",
        "((3 -4) 1 -2)",
    ),
    (
        "(use-modules (ice-9 receive)) (receive (a . rest) (values 1 2 3) (list a rest))",
        "",
        "(1 (2 3))",
    ),
    // `(ice-9 optargs)`.
    (
        "(use-modules (ice-9 optargs)) (let-keywords '(#:a 1 #:c 3) #t ((a 5) (b 6) c) (list a b c))",
        "",
        "(1 6 3)",
    ),
    (
        "(use-modules (ice-9 optargs)) (let ((x 1)) (let-keywords '() #f ((x 2) (y x)) (list x y)))",
        "",
        "(2 1)",
    ),
    (
        "(use-modules (ice-9 optargs)) (let-keywords* '(#:x 4) #f ((x 2) (y x)) (list x y))",
        "",
        "(4 4)",
    ),
    (
        "(use-modules (ice-9 optargs)) (define (f . rest) (let-optional rest (a (b 2)) (list a b rest))) (list (f 1) (f 1 3 4))",
        "",
        "((1 2 ()) (1 3 (4)))",
    ),
    (
        "(use-modules (ice-9 optargs)) (define (f . r) (let-optional* r ((a 1) (b (+ a 1))) (list a b))) (list (f) (f 5))",
        "",
        "((1 2) (5 6))",
    ),
    // `(ice-9 pretty-print)`.
    (
        "(use-modules (ice-9 pretty-print)) (with-output-to-string (lambda () (pretty-print '(define (f x) (let ((y (* x x)) (z (+ x 1))) (if (> y z) (list y z) (vector y z \"a string\" #\\c 1.5)))))))",
        "",
        "\"(define (f x)\\n  (let ((y (* x x)) (z (+ x 1)))\\n    (if (> y z) (list y z) (vector y z \\\"a string\\\" #\\\\c 1.5))))\\n\"",
    ),
    (
        "(use-modules (ice-9 pretty-print)) (with-output-to-string (lambda () (pretty-print (iota 40))))",
        "",
        "\"(0\\n 1\\n 2\\n 3\\n 4\\n 5\\n 6\\n 7\\n 8\\n 9\\n 10\\n 11\\n 12\\n 13\\n 14\\n 15\\n 16\\n 17\\n 18\\n 19\\n 20\\n 21\\n 22\\n 23\\n 24\\n 25\\n 26\\n 27\\n 28\\n 29\\n 30\\n 31\\n 32\\n 33\\n 34\\n 35\\n 36\\n 37\\n 38\\n 39)\\n\"",
    ),
    (
        "(use-modules (ice-9 pretty-print)) (call-with-output-string (lambda (p) (pretty-print '(define (f x) (if (> x 0) (display \"positive and long enough\") (display \"negative and long enough\"))) p #:width 30 #:per-line-prefix \"; \")))",
        "",
        "\"; (define (f x)\\n;   (if (> x 0)\\n;       (display\\n;        \\\"positive and long enough\\\")\\n;       (display\\n;        \\\"negative and long enough\\\")))\\n\"",
    ),
    (
        "(use-modules (ice-9 pretty-print)) (call-with-output-string (lambda (p) (display \"abc\" p) (pretty-print '(a b) p)))",
        "",
        "\"abc(a b)\\n\"",
    ),
    (
        "(use-modules (ice-9 pretty-print)) (with-output-to-string (lambda () (pretty-print '(let loop ((i 0) (acc '())) (cond ((> i 10) (reverse acc)) (else (loop (+ i 1) (cons (* i i) acc))))) #:width 40)))",
        "",
        "\"(let loop ((i 0) (acc '()))\\n  (cond\\n    ((> i 10) (reverse acc))\\n    (else (loop (+ i 1)\\n                (cons (* i i) acc)))))\\n\"",
    ),
    (
        "(use-modules (ice-9 pretty-print)) (with-output-to-string (lambda () (pretty-print '#((a . b) \"string\" #\\x 'quoted `(qq ,u ,@v) (1 . (2 . (3 . 4)))) #:width 20)))",
        "",
        "\"#((a . b)\\n  \\\"string\\\"\\n  #\\\\x\\n  'quoted\\n  `(qq ,u ,@v)\\n  (1 2 3 . 4))\\n\"",
    ),
    (
        "(use-modules (ice-9 pretty-print)) (with-output-to-string (lambda () (pretty-print '(lambda (x) (case x ((1) 'one) ((2) 'two) (else 'many))) #:width 25)))",
        "",
        "\"(lambda (x)\\n  (case x\\n    ((1) 'one)\\n    ((2) 'two)\\n    (else 'many)))\\n\"",
    ),
    (
        "(use-modules (ice-9 pretty-print)) (with-output-to-string (lambda () (pretty-print '(do ((i 0 (+ i 1))) ((= i 5) 'done) (display i) (newline)) #:width 20) (pretty-print '(some-long-procedure-name argument-one argument-two) #:width 30) (pretty-print \"s\\nt\" #:display? #t)))",
        "",
        "\"(do ((i 0 (+ i 1))) ((= i\\n                        5)\\n                     'done)\\n  (display i)\\n  (newline))\\n(some-long-procedure-name\\n argument-one\\n argument-two)\\ns\\nt\\n\"",
    ),
    // `(ice-9 textual-ports)`, `(ice-9 binary-ports)`, `(ice-9 arrays)`.
    (
        "(use-modules (ice-9 textual-ports)) (with-input-from-string \"ab\\ncd\\n\" (lambda () (let ((p (current-input-port))) (list (get-line p) (lookahead-char p) (get-char p) (get-string-n p 5) (get-line p) (get-string-all p) (get-string-n p 2)))))",
        "",
        "(\"ab\" #\\c #\\c \"d\\n\" #<eof> \"\" #<eof>)",
    ),
    (
        "(use-modules (ice-9 textual-ports)) (call-with-output-string (lambda (p) (put-string p \"hello\" 1 2) (put-char p #\\z) (put-string p \"abc\" 1)))",
        "",
        "\"elzbc\"",
    ),
    (
        "(use-modules (ice-9 binary-ports)) (call-with-values open-bytevector-output-port (lambda (p get) (put-bytevector p #vu8(1 2 3)) (display \"é\" p) (write 'x p) (put-u8 p 255) (list (output-port? p) (input-port? p) (get) (get))))",
        "",
        "(#t #f #vu8(1 2 3 233 120 255) #vu8())",
    ),
    (
        "(use-modules ((ice-9 arrays) #:select (array-copy))) (let* ((v (vector 1 2)) (c (array-copy v))) (vector-set! c 0 9) (list v c (array-copy \"ab\")))",
        "",
        "(#(1 2) #(9 2) \"ab\")",
    ),
    // `(oop goops)`.
    (
        "(use-modules (oop goops)) (define-class <a> () (x #:init-value 5) (y #:init-keyword #:y #:init-form (list 1)) z) (define-class <b> (<a>) (w #:init-keyword #:w)) (define b (make <b> #:w 1 #:q 2)) (list (slot-ref b 'x) (slot-ref b 'y) (slot-bound? b 'z) (slot-ref b 'w) (is-a? b <a>) (is-a? b <b>) (is-a? 5 <a>) (class-name <b>))",
        "",
        "(5 (1) #f 1 #t #t #f <b>)",
    ),
    (
        "(use-modules (oop goops)) (define-class <a> () (s #:init-keyword #:s)) (define o (make <a> #:s 1)) (slot-set! o 's 2) (list (slot-ref o 's) (slot-ref (make <a> #:s 3) 's))",
        "",
        "(2 3)",
    ),
    (
        "(use-modules (oop goops)) (define-class <a> () (s #:init-keyword #:s)) (catch 'goops-error (lambda () (slot-ref (make <a>) 's)) (lambda (k s m a r) (list s m (length a) r)))",
        "",
        "(#f \"Unbound slot in object ~S\" 1 ())",
    ),
    (
        "(use-modules (oop goops)) (define-class <a> () (s #:init-keyword #:s)) (catch 'goops-error (lambda () (slot-ref (make <a>) 'nope)) (lambda (k s m a r) (list s m (car a))))",
        "",
        "(#f \"No slot with name `~S' in object ~S\" nope)",
    ),
    (
        "(use-modules (oop goops)) (define-class <a> () (s #:init-keyword #:s)) (catch 'wrong-type-arg (lambda () (make <a> #:s)) (lambda (k s m a r) (list s m a r)))",
        "",
        "(#f \"Invalid initargs: ~S\" ((#:s)) #f)",
    ),
    // Generic functions (MS5 step 6a): accessors with setters, getters, a
    // method of a subclass before its superclass's, rest arguments, one
    // accessor over two classes, and no applicable method, as Guile 3.0.
    (
        "(use-modules (oop goops)) (define-class <a> () (x #:init-value 1 #:accessor ax #:init-keyword #:x) (y #:getter ay #:init-value 2)) (define-class <b> (<a>) (z #:accessor ax2) (when #:accessor moment #:init-keyword #:m)) (define-class <c> () (when #:accessor moment #:init-keyword #:m)) (define-method (f (o <a>)) 'a) (define-method (f (o <b>)) 'b) (define-method (g (o <a>) y) (list 'g y)) (define-method (h (o <a>) . rest) rest) (define b (make <b> #:x 5 #:m 7)) (define c (make <c> #:m 9)) (set! (ax b) 6) (list (ax b) (ay b) (f b) (f (make <a>)) (g b 3) (h b 1 2) (moment b) (moment c) (catch 'goops-error (lambda () (f 5)) (lambda (k s m a r) (list k s m))))",
        "",
        "(6 2 b a (g 3) (1 2) 7 9 (goops-error #f \"No applicable method for ~S in call ~S\"))",
    ),
    // `write`, a primitive generic, kept as the method for any arguments;
    // another procedure is no generic.
    (
        "(use-modules (oop goops)) (define-class <a> () (s #:init-value 1)) (define-method (write (o <a>) p) (display \"A\" p)) (define (k x) x) (list (call-with-output-string (lambda (p) (write (make <a>) p) (write '(1 \"b\") p))) (catch 'goops-error (lambda () (define-method (k (o <a>)) 'a) 1) (lambda (key s m a r) (list s m (length a) r))))",
        "",
        "(\"A(1 \\\"b\\\")\" (#f \"~S is not a valid generic function\" 1 ()))",
    ),
];

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
        let mut vm = Vm::new();
        let (got_key, got) = run(&mut vm, source);
        if (got_key.as_str(), got.as_str()) != (key, text) {
            failures.push(format!(
                "{source}\n  got  {got_key}: {got}\n  want {key}: {text}"
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

/// The runner both guile and LilyPond use: the forms of `text` read and
/// evaluated in turn, then a tab and the last value written, or the key, a
/// tab and the error as Guile prints it.
const RUNNER: &str = r#"(define (run text)
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

/// Runs `source` in a fresh guile; `None` without guile.
fn guile(source: &str) -> Option<(String, String)> {
    let program = format!("{RUNNER}(run {})\n", literal(source));
    let out = Command::new("guile")
        .args(["--no-auto-compile", "-c", &program])
        .output()
        .ok()?;
    let stdout = String::from_utf8(out.stdout).unwrap();
    let (key, text) = stdout.split_once('\t').unwrap_or((&stdout, ""));
    Some((key.to_string(), text.trim_end_matches('\n').to_string()))
}

#[test]
fn guile_gives_the_table() {
    let mut failures = Vec::new();
    for &(source, key, text) in CASES {
        let Some((got_key, got)) = guile(source) else {
            eprintln!("guile is not installed: the table is unverified against it");
            return;
        };
        if (got_key.as_str(), got.as_str()) != (key, text) {
            failures.push(format!(
                "{source}\n  guile {got_key}: {got}\n  table {key}: {text}"
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

// --- `(lily)` against the installed LilyPond ---------------------------------------

/// `scm/lily/` of the installed LilyPond 2.26.0, if there is one.
fn lily_scm() -> Option<PathBuf> {
    let version = Command::new("lilypond").arg("--version").output().ok()?;
    if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
        return None;
    }
    let path = Command::new("sh")
        .args(["-c", "command -v lilypond"])
        .output()
        .ok()?;
    let binary = std::fs::canonicalize(String::from_utf8(path.stdout).ok()?.trim()).ok()?;
    let lily = binary.parent()?.join("../share/lilypond/2.26.0/scm/lily");
    lily.join("music-functions.scm").exists().then_some(lily)
}

/// Runs each of `sources` in turn in LilyPond's `(lily)`, in one `lilypond`
/// process: what each gives, as [`run`] gives it, and what LilyPond wrote
/// to its standard error.
fn lilypond(name: &str, sources: &[&str]) -> (Vec<(String, String)>, String) {
    let dir = std::env::temp_dir().join(format!("lily-engine-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cases: Vec<String> = sources.iter().map(|s| literal(s)).collect();
    let score = format!(
        "#(begin\n{RUNNER}\n(save-module-excursion (lambda ()\n  (set-current-module (resolve-module '(lily)))\n  (for-each (lambda (s) (run s) (display (integer->char 30)))\n    (list {}))))\n)\n",
        cases.join("\n")
    );
    let file = dir.join("probe.ly");
    std::fs::write(&file, score).unwrap();
    let out = Command::new("lilypond")
        .arg("-s")
        .arg(&file)
        .current_dir(&dir)
        .output()
        .unwrap();
    std::fs::remove_dir_all(&dir).ok();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let results = stdout
        .split('\u{1e}')
        .take(sources.len())
        .map(|r| {
            let (key, text) = r.split_once('\t').unwrap_or((r, ""));
            (key.to_string(), text.trim_end_matches('\n').to_string())
        })
        .collect();
    (results, String::from_utf8(out.stderr).unwrap())
}

/// `(lily)`'s natives: programs run in `(lily)` and what they give.
const LILY_CASES: &[(&str, &str, &str)] = &[
    ("(ly:version)", "", "(2 26 0)"),
    // What `lily.scm`'s init data calls as it loads (MS2 step 12).
    (
        "(map ly:dimension? (list 1 1/2 1.5 +inf.0 1+2i 'a \"s\"))",
        "",
        "(#t #t #t #t #t #f #f)",
    ),
    (
        "(map (lambda (p) (p 1)) (list ly:context? ly:grob-array? ly:grob-properties? ly:grob? ly:music-output? ly:stream-event? ly:tuplet-description?))",
        "",
        "(#f #f #f #f #f #f #f)",
    ),
    // What the rest of `lily.scm`'s core files call as they load (MS2
    // step 13).
    (
        "(list (map (lambda (f) (f 2)) (list ly:pt ly:mm ly:cm ly:inch ly:bp)) (ly:pt 1/3) (ly:inch 0))",
        "",
        "((0.7029196070291961 1.9999999999999998 19.999999999999996 50.8 0.7055555555555555) 0.11715326783819935 0.0)",
    ),
    (
        "(ly:pt 'a)",
        "wrong-type-arg",
        "In procedure ly:pt: Wrong type argument in position 1 (expecting number): a",
    ),
    (
        "(ly:register-translator (lambda (c) '()) 'Probe_engraver '((description . \"d\")))",
        "",
        "#<unspecified>",
    ),
    (
        "(ly:register-translator '() 'Probe_engraver)",
        "wrong-type-arg",
        "In procedure ly_register_translator: Wrong type argument in position 1 (expecting translator creator): ()",
    ),
    (
        "(ly:register-translator (lambda (c) '()) 'P 5)",
        "wrong-type-arg",
        "In procedure ly:register-translator: Wrong type argument in position 3 (expecting list): 5",
    ),
    (
        "(list (ly:add-interface 'probe-interface \"A probe.\" '(a b)) (hashq-ref (ly:all-grob-interfaces) 'probe-interface))",
        "",
        "(#<unspecified> (probe-interface \"A probe.\" (a b)))",
    ),
    (
        "(ly:add-interface \"x\" \"doc\" '())",
        "wrong-type-arg",
        "In procedure ly:add-interface: Wrong type argument in position 1 (expecting symbol): \"x\"",
    ),
    (
        "(ly:add-interface 'x 3 '())",
        "wrong-type-arg",
        "In procedure ly:add-interface: Wrong type argument in position 2 (expecting string): 3",
    ),
    (
        "(ly:add-interface 'x \"doc\" 'y)",
        "wrong-type-arg",
        "In procedure ly:add-interface: Wrong type argument in position 3 (expecting list): y",
    ),
    (
        "(list (ly:make-regex \"a+\") (eq? (ly:version) (ly:version)) (equal? (ly:make-regex \"a\") (ly:make-regex \"a\")))",
        "",
        "(#<Regex> #f #f)",
    ),
    (
        "(list ly:assoc-get ly:chain-assoc-get ly:make-regex ly:version)",
        "",
        "(#<procedure ly:assoc-get (_ _ #:optional _ _)> #<procedure ly:chain-assoc-get (_ _ #:optional _ _)> #<procedure ly:make-regex (_)> #<procedure ly:version ()>)",
    ),
    (
        "(list (ly:assoc-get 'a '((a . 1)) 5) (ly:assoc-get 'b '((a . 1)) 5) (ly:assoc-get 'b '((a . 1))) (ly:assoc-get \"a\" '((\"a\" . 1))) (ly:assoc-get 2.0 '((2 . 1))) (ly:assoc-get 'a '() 'none #f))",
        "",
        "(1 5 #f 1 #f none)",
    ),
    (
        "(ly:assoc-get 'a 5)",
        "wrong-type-arg",
        "In procedure ly:assoc-get: Wrong type argument in position 2 (expecting list): 5",
    ),
    (
        "(ly:assoc-get 'a '(1 2))",
        "wrong-type-arg",
        "In procedure assq: Wrong type argument in position 2 (expecting association list): (1 2)",
    ),
    (
        "(ly:assoc-get 'a '((b . 1) . 3))",
        "wrong-type-arg",
        "In procedure assq: Wrong type argument in position 2 (expecting association list): ((b . 1) . 3)",
    ),
    (
        "(list (ly:chain-assoc-get 'b '(((a . 1)) ((b . 2)))) (ly:chain-assoc-get 'c '(((a . 1)) ((b . 2))) 'd) (ly:chain-assoc-get 'a 5) (ly:chain-assoc-get 'a '(() ((a . 7)))) (ly:chain-assoc-get 'a '(((b . 1))) 'dflt #t))",
        "",
        "(2 d #f 7 dflt)",
    ),
    (
        "(ly:chain-assoc-get 'a '(5))",
        "wrong-type-arg",
        "In procedure assq: Wrong type argument in position 2 (expecting association list): 5",
    ),
    (
        "(ly:make-regex 5)",
        "wrong-type-arg",
        "In procedure ly:make-regex: Wrong type argument in position 1 (expecting string): 5",
    ),
    (
        "(ly:version 1)",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure ly:version ()>",
    ),
    ("(list (G_ \"x\") (_i \"y\"))", "", "(\"x\" \"y\")"),
    ("(ly:assoc-get \"k\" '() \"d\" #t)", "", "\"d\""),
];

/// What `ly:assoc-get` with strict checking writes to the error port for
/// the last case, as LilyPond writes it to its standard error.
const PROGRAMMING_ERROR: &str = "programming error: Cannot find key `\"k\"' in alist, setting to `\"d\"'.\ncontinuing, cross fingers\n";

#[test]
fn lily_natives_give_what_lilypond_gives() {
    let mut failures = Vec::new();
    let mut errors = String::new();
    for &(source, key, text) in LILY_CASES {
        let mut vm = Vm::new();
        vm.eval_text("(set-current-module (resolve-module '(lily)))", None)
            .unwrap();
        let (got_key, got) = run(&mut vm, source);
        errors.push_str(&vm.take_errors());
        if (got_key.as_str(), got.as_str()) != (key, text) {
            failures.push(format!(
                "{source}\n  engine {got_key}: {got}\n  want {key}: {text}"
            ));
        }
    }
    assert_eq!(errors, PROGRAMMING_ERROR);
    if lily_scm().is_some() {
        let sources: Vec<&str> = LILY_CASES.iter().map(|c| c.0).collect();
        let (results, stderr) = lilypond("natives", &sources);
        for (&(source, key, text), (got_key, got)) in LILY_CASES.iter().zip(&results) {
            if (got_key.as_str(), got.as_str()) != (key, text) {
                failures.push(format!(
                    "{source}\n  lilypond {got_key}: {got}\n  want {key}: {text}"
                ));
            }
        }
        assert_eq!(results.len(), LILY_CASES.len(), "LilyPond ran every case");
        assert!(
            stderr.contains(PROGRAMMING_ERROR),
            "LilyPond's standard error:\n{stderr}"
        );
    } else {
        eprintln!("LilyPond 2.26.0 is not installed: (lily)'s natives are unverified against it");
    }
    assert!(
        failures.is_empty(),
        "{} failures of {}:\n{}",
        failures.len(),
        LILY_CASES.len(),
        failures.join("\n")
    );
}

/// `lily.scm`'s session variables, which a running LilyPond no longer lets
/// a file define: the engine's alone.
#[test]
fn session_variables_are_recorded() {
    let mut vm = Vm::new();
    let program = "(set-current-module (resolve-module '(lily)))
        (define-session s 1)
        (define-session-public sp 2)
        (call-after-session (lambda () 'after))
        (list s sp (map car lilypond-declarations) (map car lilypond-exports)
              (@ (lily) sp) (map (lambda (t) (t)) after-session-thunks))";
    assert_eq!(run(&mut vm, program).1, "(1 2 (s) (sp) 2 (after))");
    assert_eq!(
        run(&mut vm, "(define-session s 3)"),
        ("misc-error".to_string(), "symbol s redefined".to_string())
    );
    assert_eq!(
        run(
            &mut vm,
            "(set! first-session-done? #t) (call-after-session (lambda () 1))"
        ),
        (
            "misc-error".to_string(),
            "call-after-session used after session start".to_string()
        )
    );
}

// --- the two files --------------------------------------------------------------------

/// The toplevel form of `file` that begins with `head`, as its text.
fn definition(lily: &Path, file: &str, head: &str) -> String {
    let text = std::fs::read_to_string(lily.join(file)).unwrap();
    let start = text
        .find(head)
        .unwrap_or_else(|| panic!("{file} has no {head}"));
    let mut heap = Heap::new();
    let r = read(&mut heap, &text, start, None).unwrap();
    text[r.start..r.end].to_string()
}

/// Expressions over the two files' definitions, evaluated in `(lily)` in
/// turn after the files load, and what LilyPond's own `(lily)` gives.
const PROBES: &[(&str, &str, &str)] = &[
    (
        "(list X Y LEFT RIGHT UP DOWN CENTER SHARP DOUBLE-FLAT INFINITY-INT)",
        "",
        "(0 1 -1 1 1 -1 0 1/2 -1 1000000)",
    ),
    (
        "(list PI TWO-PI PI-OVER-TWO PI-OVER-180)",
        "",
        "(3.141592653589793 6.283185307179586 1.5707963267948966 0.017453292519943295)",
    ),
    (
        "(map-selected-alist-keys - '(a b) '((a . 1) (b . -2) (c . 3) (d . 4)))",
        "",
        "((a . -1) (b . 2) (c . 3) (d . 4))",
    ),
    (
        "(split-list-by-separator '(a 0 b c 1 d) number?)",
        "",
        "((a) (b c) (d))",
    ),
    (
        "(split-list-by-group-lengths '(a b c d e f) '(3 2 1))",
        "",
        "((a b c) (d e) (f))",
    ),
    (
        "(flat-zip-longest '(a b c d) '(e f) '(g h i))",
        "",
        "(a e g b f h c i d)",
    ),
    (
        "(group-into-ranges '(1 4 3 6 7 2))",
        "",
        "((1 . 4) (6 . 7))",
    ),
    (
        "(uniqued-alist '((a . 1) (b . 2) (a . 3)))",
        "",
        "((a . 1) (b . 2))",
    ),
    ("(list-insert-separator '(a b c) 'x)", "", "(a x b x c)"),
    (
        "(list INF-MOMENT ZERO-MOMENT ZERO-DURATION (moment-min (ly:make-moment 1/4) (ly:make-moment 1/4 -1/8)) (moment<=? ZERO-MOMENT ZERO-MOMENT))",
        "",
        "(#<Mom infinity> #<Mom 0> #<Duration 1*0 > #<Mom 1/4G-1/8> #t)",
    ),
    (
        "(list (fraction->moment '(3 . 4)) (fraction->moment '()) (moment->fraction (ly:make-moment 6/8 -1/8)) (moment->fraction INF-MOMENT))",
        "",
        "(#<Mom 3/4> #<Mom 0> (3 . 4) (2 . 1))",
    ),
    (
        "(list (duration-visual (ly:make-duration 3 1 2/3)) (duration-visual-length (ly:make-duration 3 1 2/3)) (make-duration-of-length (ly:make-moment 3/8)) (make-duration-of-length (ly:make-moment 5/16)))",
        "",
        "(#<Duration 8. > 3/16 #<Duration 4. > #<Duration 4*5/4 >)",
    ),
    (
        "(list (duration-log-factor 3) (duration-log-factor -1) (duration-dot-factor 2))",
        "",
        "(1/8 2 7/4)",
    ),
    (
        "(duration-log-factor 1.5)",
        "wrong-type-arg",
        "In procedure duration-log-factor: Not an integer: 1.5",
    ),
    (
        "(list (string-endswith \"hello\" \"LO\" #:case-insensitive #t) (string-startswith \"hello\" \"he\") (string-encode-integer 1234) (remove-whitespace \" a b \"))",
        "",
        "(#t #t \"MVBo\" \"ab\")",
    ),
    (
        "(list (interval-union '(0 . 1) '(2 . 3)) (interval-length '(1 . 5)) (ordered-cons 3 1) (interval-widen '(1 . 2) 1))",
        "",
        "((0 . 3) 4 (1 . 3) (0 . 3))",
    ),
    (
        "(list (binary-search 0 10 (lambda (x) (* x x)) 50) (bit-list->int '(#t #f #t)) (byte-list->bit-list '(5)))",
        "",
        "(7 5 (#f #f #f #f #f #t #f #t))",
    ),
    (
        "(list (car< '(1 . a) '(2 . b)) (symbol<? 'b 'a) (symbol-key<? '(a . 1) '(b . 2)))",
        "",
        "(#t #f #t)",
    ),
    (
        "(list (magstep 6) (magnification->font-size 2))",
        "",
        "(2.0 6.0)",
    ),
    (
        "(list (ly:version? > '(2 20 0)) (lilypond-version-outdated? '(2 24 0) '(2 26 0)))",
        "",
        "(#t #t)",
    ),
    (
        "(@@ (lily) convert-ly-msg)",
        "",
        "\"\\nNote: compilation failed and \\\\version outdated, did you\\nupdate input syntax with convert-ly?\\n\\n  https://lilypond.org/doc/v2.26/Documentation/usage/updating-files-with-convert_002dly\\n\\n\"",
    ),
    (
        "(list (ly:regex-match? 5) (ly:regex-match? (ly:make-regex \"a\")))",
        "",
        "(#f #f)",
    ),
    (
        "(call-with-output-bytevector (lambda (p) (display \"ab\" p)))",
        "",
        "#vu8(97 98)",
    ),
    (
        "(eval-carefully '(+ 1 2) (resolve-module '(lily)))",
        "",
        "3",
    ),
    (
        "(list (get-next-unique-voice-name) (get-next-unique-voice-name))",
        "",
        "(\"uniqueContext0\" \"uniqueContext1\")",
    ),
    ("(length toplevel-music-functions)", "", "8"),
    (
        "(map car accidental-styles)",
        "",
        "(default voice modern modern-cautionary neo-modern neo-modern-cautionary neo-modern-voice neo-modern-voice-cautionary dodecaphonic dodecaphonic-no-repeat dodecaphonic-first modern-voice modern-voice-cautionary piano piano-cautionary choral choral-cautionary teaching forget no-reset)",
    ),
    (
        "score-grace-settings",
        "",
        "((Voice Stem direction 1) (Voice Slur direction -1) (Voice Stem font-size -3) (Voice Flag font-size -3) (Voice NoteHead font-size -3) (Voice TabNoteHead font-size -4) (Voice Dots font-size -3) (Voice Stem length-fraction 0.8) (Voice Stem no-stem-extend #t) (Voice Beam beam-thickness 0.384) (Voice Beam length-fraction 0.8) (Voice Accidental font-size -4) (Voice AccidentalCautionary font-size -4) (Voice Script font-size -3) (Voice Fingering font-size -8) (Voice StringNumber font-size -8))",
    ),
    (
        "(list (define-tag-group '(engine-probe-a engine-probe-b)) (define-tag-group '(engine-probe-a)) (define-tag-group 5))",
        "",
        "(#f \"conflicting tag group (engine-probe-a engine-probe-b)\" \"not a symbol list: 5\")",
    ),
    (
        "(let ((p (lambda (x) x))) (set! (syntax-function-procedure-arguments p) '(x)) (syntax-function-procedure-arguments p))",
        "",
        "(x)",
    ),
    (
        "(list (eq? 'a 'a 'a) (eqv? 1 1 2) (equal?))",
        "",
        "(#t #f #t)",
    ),
];

/// Loads the two files into a VM's `(lily)` as `lily.scm` does, with the
/// music crate's functions.
fn load_library(lily: &Path) -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    vm.provide_module("lily display-lily", "(define-module (lily display-lily))");
    let symbol_list = definition(lily, "c++.scm", "(define-public (symbol-list? x)");
    let prelude = format!("(set-current-module (resolve-module '(lily)))\n{symbol_list}");
    vm.eval_text(&prelude, None).unwrap();
    for file in ["lily-library.scm", "music-functions.scm"] {
        let path = lily.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        if let Err(e) = vm.eval_text(&text, path.to_str()) {
            panic!("{file} does not load: {}", e.text(vm.heap()));
        }
    }
    vm
}

#[test]
fn the_two_files_load_from_source() {
    let Some(lily) = lily_scm() else {
        eprintln!("LilyPond 2.26.0 is not installed: the two files are not loaded");
        return;
    };
    let mut vm = load_library(&lily);
    let mut failures = Vec::new();
    let engine: Vec<(String, String)> = PROBES.iter().map(|p| run(&mut vm, p.0)).collect();
    let sources: Vec<&str> = PROBES.iter().map(|p| p.0).collect();
    let (from_lilypond, _) = lilypond("probe", &sources);
    assert_eq!(
        from_lilypond.len(),
        PROBES.len(),
        "LilyPond ran every probe"
    );
    for ((&(source, key, text), got), lily) in PROBES.iter().zip(&engine).zip(&from_lilypond) {
        for (who, (got_key, got)) in [("engine", got), ("lilypond", lily)] {
            if (got_key.as_str(), got.as_str()) != (key, text) {
                failures.push(format!(
                    "{source}\n  {who} {got_key}: {got}\n  want {key}: {text}"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures of {}:\n{}",
        failures.len(),
        PROBES.len(),
        failures.join("\n")
    );
}
