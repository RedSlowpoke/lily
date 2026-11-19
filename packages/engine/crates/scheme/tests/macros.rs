//! S3's macros and Guile's extended forms (chapter 03 §5.5): `lambda*`,
//! `define*`, `case-lambda`, `do`, `while`, `quasiquote`, `eval-when`,
//! `syntax-rules` with its hygiene, `define-syntax-rule`, `defmacro` and
//! `define-macro` give what Guile 3.0.11 gives. With `guile` installed the
//! table is also run by guile, each program in a fresh process.
//!
//! `(lily)`'s native `assert` and `define-syntax-function` stand in for
//! the `syntax-case` definitions of the installed LilyPond 2.26.0's
//! `lily-library.scm` and `music-functions.scm` (D60): with those files
//! installed, their definitions are read from them, never copied here, and
//! the same programs run in the engine, where loading a definition keeps the
//! native macro, and in guile, which expands the file's.
use std::path::{Path, PathBuf};
use std::process::Command;

use lily_engine_scheme::{Heap, Value, Vm, read};

/// Programs and what they give: the last form's value as `write` writes
/// it, or the key and the text of the error, as in `tests/modules.rs`.
const CASES: &[(&str, &str, &str)] = &[
    // `define*` and `lambda*`.
    (
        "(define* (f a #:optional b (c 3)) (list a b c)) (list f (f 1) (f 1 2) (f 1 2 4))",
        "",
        "(#<procedure f args> (1 #f 3) (1 2 3) (1 2 4))",
    ),
    (
        "(define* (f a #:optional b (c 3)) (list a b c)) (f 1 2 3 4)",
        "wrong-number-of-args",
        "In procedure eval: Wrong number of arguments",
    ),
    (
        "(define* (f a #:optional b) (list a b)) (f)",
        "wrong-number-of-args",
        "In procedure eval: Wrong number of arguments",
    ),
    (
        "(define* (f #:key a (b 2)) (list a b)) (list (f) (f #:b 5) (f #:a 1 #:b 3))",
        "",
        "((#f 2) (#f 5) (1 3))",
    ),
    (
        "(define* (f #:key a (b 2)) (list a b)) (f #:c 1)",
        "keyword-argument-error",
        "Unrecognized keyword: #:c",
    ),
    (
        "(define* (f #:key a (b 2)) (list a b)) (f #:a)",
        "keyword-argument-error",
        "Keyword argument has no value: #:a",
    ),
    (
        "(define* (f #:key a (b 2)) (list a b)) (f 1)",
        "keyword-argument-error",
        "Invalid keyword: 1",
    ),
    (
        "(define* (f #:key a #:allow-other-keys) (list a)) (f #:c 1 #:a 2)",
        "",
        "(2)",
    ),
    (
        "(define* (f #:optional a #:key b) (list a b)) (list (f #:b 1) (f 1 #:b 2) (f 1))",
        "",
        "((#f 1) (1 2) (1 #f))",
    ),
    (
        "(define* (f a #:optional b . r) (list a b r)) (list f (f 1 2 3 4))",
        "",
        "(#<procedure f args> (1 2 (3 4)))",
    ),
    (
        "(define* (f #:key (y 5 #:x)) y) (list (f) (f #:x 1))",
        "",
        "(5 1)",
    ),
    (
        "((lambda* (a #:optional (b a) (c (+ a b))) (list a b c)) 1)",
        "",
        "(1 1 2)",
    ),
    (
        "((lambda* (#:optional a #:key b #:allow-other-keys #:rest r) (list a b r)) 1 #:b 2 #:c 3)",
        "",
        "(1 2 (#:b 2 #:c 3))",
    ),
    (
        "((lambda* (a #:key b) (list a b)) 1 #:b 2 #:b 3)",
        "",
        "(1 3)",
    ),
    // With a rest argument, what is not a keyword is skipped, as
    // `markup-macros.scm`'s `markup-lambda` needs.
    (
        "((lambda* (a #:key (k 1) #:rest r) (list a k r)) 1 \"a\" #:k 2 \"b\" #:k 3)",
        "",
        "(1 3 (\"a\" #:k 2 \"b\" #:k 3))",
    ),
    (
        "((lambda* (a #:key (k 1) #:rest r) (list a k r)) 1 #:z 2)",
        "keyword-argument-error",
        "Unrecognized keyword: #:z",
    ),
    (
        "((lambda* (a #:key (k 1)) (list a k)) 1 #:z)",
        "keyword-argument-error",
        "Unrecognized keyword: #:z",
    ),
    (
        "((lambda* (a #:key (k 1) #:allow-other-keys) (list a k)) 1 #:z)",
        "",
        "(1 1)",
    ),
    (
        "((lambda* (#:optional a b #:key c) (list a b c)) 1 #:c 2)",
        "",
        "(1 #f 2)",
    ),
    (
        "(define f (lambda* (a #:rest r) (list a r))) (list f (f 1 2))",
        "",
        "(#<procedure f (a . rest)> (1 (2)))",
    ),
    (
        "(define-module (t p)) (define*-public (g #:optional (x 1)) x) (define-module (t q) #:use-module (t p)) (list (g) (g 2))",
        "",
        "(1 2)",
    ),
    (
        "(define* (f #:optional (a 1 2)) a)",
        "syntax-error",
        "Syntax error:\nunknown location: lambda*: invalid optional argument list in subform ((a 1 2)) of (#:optional (a 1 2))",
    ),
    (
        "(lambda* (a a) a)",
        "syntax-error",
        "Syntax error:\nunknown file:1:9: lambda*: duplicate identifier in argument list in form (a a)",
    ),
    // `case-lambda`.
    (
        "(define f (case-lambda ((a) (list 'one a)) ((a b) (list 'two a b)) ((a . r) (list 'many a r)))) (list f (f 1) (f 1 2) (f 1 2 3))",
        "",
        "(#<procedure f args> (one 1) (two 1 2) (many 1 (2 3)))",
    ),
    (
        "((case-lambda) 1)",
        "wrong-number-of-args",
        "In procedure eval: Wrong number of arguments",
    ),
    // `do` and `while`.
    (
        "(do ((i 0 (+ i 1)) (acc '() (cons i acc))) ((= i 3) acc))",
        "",
        "(2 1 0)",
    ),
    ("(do ((i 0 (+ i 1))) ((= i 3)))", "", "#<unspecified>"),
    ("(do ((i 0)) (#t i))", "", "0"),
    (
        "(let ((doloop 5) (not 6)) (do ((i 0 (+ i 1))) ((= i 2) (list doloop not))))",
        "",
        "(5 6)",
    ),
    (
        "(do)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: source expression failed to match any pattern in form (do)",
    ),
    ("(let ((n 0)) (while (< n 5) (set! n (+ n 1))))", "", "#f"),
    (
        "(call-with-values (lambda () (let ((n 0)) (while #t (set! n (+ n 1)) (if (> n 3) (break n 'x))))) list)",
        "",
        "(4 x)",
    ),
    (
        "(let ((n 0)) (while #t (set! n (+ n 1)) (if (> n 3) (break))))",
        "",
        "#t",
    ),
    (
        "(let ((n 0) (l '())) (while (< n 5) (set! n (+ n 1)) (if (odd? n) (continue)) (set! l (cons n l))) l)",
        "",
        "(4 2)",
    ),
    (
        "(let ((i 0) (out '())) (while (< i 3) (set! i (+ i 1)) (let ((j 0)) (while #t (set! j (+ j 1)) (if (> j i) (break)) (set! out (cons (list i j) out))))) out)",
        "",
        "((3 3) (3 2) (3 1) (2 2) (2 1) (1 1))",
    ),
    // `quasiquote`.
    (
        "(let ((x 1) (l '(2 3))) `(a ,x ,@l b #(1 ,x ,@l) (nested `(c ,(d ,x)))))",
        "",
        "(a 1 2 3 b #(1 1 2 3) (nested (quasiquote (c (unquote (d 1))))))",
    ),
    ("(let ((l (list 1 2))) (eq? l `(,@l)))", "", "#t"),
    ("`(1 ,@2)", "", "(1 . 2)"),
    ("`(1 ,@'(2) . 3)", "", "(1 2 . 3)"),
    ("`(1 . ,(+ 1 1))", "", "(1 . 2)"),
    ("`(,@'(1) ,@'(2))", "", "(1 2)"),
    (
        "`(1 `(2 ,(3 ,(+ 1 3))))",
        "",
        "(1 (quasiquote (2 (unquote (3 4)))))",
    ),
    ("`#(,@'(1 2))", "", "#(1 2)"),
    ("(let ((a 1)) `#(x ,a))", "", "#(x 1)"),
    (
        "(let ((list vector) (cons 0) (append 1)) `(1 ,(+ 1 1) ,@'(3) . 4))",
        "",
        "(1 2 3 . 4)",
    ),
    (
        "(unquote 1)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: unquote: expression not valid outside of quasiquote in form (unquote 1)",
    ),
    (
        "(quasiquote)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: source expression failed to match any pattern in form (quasiquote)",
    ),
    // `eval-when`.
    ("(eval-when (eval) 1)", "", "1"),
    (
        "(eval-when (load) (define zz 1)) zz",
        "unbound-variable",
        "Unbound variable: zz",
    ),
    ("(eval-when (expand) (define zz 1)) zz", "", "1"),
    (
        "(eval-when (compile) (define zz 1)) zz",
        "unbound-variable",
        "Unbound variable: zz",
    ),
    ("(let () (eval-when (eval) 1 2))", "", "2"),
    ("(let () (eval-when (load) 1 2))", "", "#<unspecified>"),
    (
        "(eval-when (bogus) 1)",
        "syntax-error",
        "Syntax error:\nunknown location: eval-when: invalid situation in subform bogus of (eval-when (bogus) 1)",
    ),
    // `syntax-rules`.
    (
        "(define-syntax swap! (syntax-rules () ((_ a b) (let ((tmp a)) (set! a b) (set! b tmp))))) (let ((tmp 1) (y 2)) (swap! tmp y) (list tmp y))",
        "",
        "(2 1)",
    ),
    (
        "(define-syntax my-or (syntax-rules () ((_) #f) ((_ e) e) ((_ e r ...) (let ((t e)) (if t t (my-or r ...)))))) (let ((t 5)) (my-or #f t))",
        "",
        "5",
    ),
    (
        "(define-syntax m (syntax-rules () ((_ (a b ...) ...) '((b ... a) ...)))) (m (1 2 3) (4) (5 6))",
        "",
        "((2 3 1) (4) (6 5))",
    ),
    (
        "(define-syntax m (syntax-rules () ((_ a ...) (list 'x a ... 'y)))) (m)",
        "",
        "(x y)",
    ),
    (
        "(define-syntax m (syntax-rules (=>) ((_ a => b) (list a b)) ((_ a b c) 'no))) (list (m 1 => 2) (let ((=> 1)) (m 1 => 2)))",
        "",
        "((1 2) no)",
    ),
    (
        "(define-syntax m (syntax-rules () ((_ #(a ...)) (list a ...)))) (m #(1 2 3))",
        "",
        "(1 2 3)",
    ),
    (
        "(define-syntax m (syntax-rules () ((_ a ... z) '(z a ...)))) (m 1 2 3 4)",
        "",
        "(4 1 2 3)",
    ),
    (
        "(define-syntax m (syntax-rules () ((_ (a . b)) '(b a)))) (m (1 2 3))",
        "",
        "((2 3) 1)",
    ),
    (
        "(define-syntax m (syntax-rules () ((_ (a ...) . r) '(r a ...)))) (m (1 2) 3 4)",
        "",
        "((3 4) 1 2)",
    ),
    (
        "(define-syntax m (syntax-rules () ((_ (k #:once) v) '(k v)) ((_ x v) 'other))) (list (m (a #:once) 1) (m (a #:twice) 1))",
        "",
        "((a 1) other)",
    ),
    (
        "(define-syntax m (syntax-rules ::: () ((_ a :::) '(a ::: ...)))) (m 1 2)",
        "",
        "(1 2 ...)",
    ),
    (
        "(define-syntax m (syntax-rules () ((_ a) '(a (... ...))))) (m 1)",
        "",
        "(1 ...)",
    ),
    (
        "(define-syntax m (syntax-rules () ((_ a) (let ((x 1)) (+ x a))))) (let ((x 10)) (m x))",
        "",
        "11",
    ),
    (
        "(define-syntax m (syntax-rules () ((_) car))) (let ((car cdr)) ((m) '(1 2)))",
        "",
        "1",
    ),
    (
        "(define-syntax my-if (syntax-rules () ((_ c a b) (cond (c a) (else b))))) (let ((else #f)) (my-if #f 1 2))",
        "",
        "2",
    ),
    (
        "(define-syntax m (syntax-rules () ((_ a) (define (a x) (* x 2))))) (m dbl) (dbl 4)",
        "",
        "8",
    ),
    (
        "(define-syntax-rule (mk) (define hidden 1)) (mk) hidden",
        "unbound-variable",
        "Unbound variable: hidden",
    ),
    (
        "(define-syntax-rule (mk n) (define n 1)) (mk vis) vis",
        "",
        "1",
    ),
    (
        "(define-syntax-rule (foo a) \"doc\" (list a 'a)) (foo 3)",
        "",
        "(3 3)",
    ),
    (
        "(define-syntax-rule (two a) (begin (define x1 a) (define x2 a))) (two 7) (list x1 x2)",
        "unbound-variable",
        "Unbound variable: x1",
    ),
    (
        "(define-syntax-rule (two n m a) (begin (define n a) (define m n))) (two x1 x2 7) (list x1 x2)",
        "",
        "(7 7)",
    ),
    (
        "(let-syntax ((m (syntax-rules () ((_ x) (* x 2))))) (m 4))",
        "",
        "8",
    ),
    (
        "(letrec-syntax ((ev? (syntax-rules () ((_ n) (if (= n 0) #t (od? (- n 1)))))) (od? (syntax-rules () ((_ n) (if (= n 0) #f #t))))) (ev? 2))",
        "",
        "#t",
    ),
    (
        "(let ((y 3)) (let-syntax ((m (syntax-rules () ((_) y)))) (m)))",
        "",
        "3",
    ),
    (
        "(define (f) (define-syntax m (syntax-rules () ((_) 42))) (m)) (f)",
        "",
        "42",
    ),
    (
        "(define-syntax foo (syntax-rules () ((_) 1))) (module-ref (current-module) 'foo)",
        "",
        "#<syntax-transformer foo>",
    ),
    (
        "(define-syntax foo (syntax-rules () ((_) 1))) (foo 1)",
        "syntax-error",
        "Syntax error:\nunknown file:1:46: source expression failed to match any pattern in form (foo 1)",
    ),
    (
        "(define-syntax foo (syntax-rules () ((_) 1))) foo",
        "syntax-error",
        "Syntax error:\nunknown location: source expression failed to match any pattern in form foo",
    ),
    (
        "(define-syntax m (syntax-rules () ((_ a ...) (a ... ...))))",
        "syntax-error",
        "Syntax error:\nunknown file:1:17: syntax: extra ellipsis in form (syntax (a ... ...))",
    ),
    (
        "(define-syntax m (syntax-rules () ((_ (a ...) ...) (list a ...))))",
        "syntax-error",
        "Syntax error:\nunknown file:1:17: syntax: missing ellipsis in form (syntax (list a ...))",
    ),
    // Macros in modules: exported, imported, and hygienic across them.
    (
        "(define-module (t m)) (define (helper) 'h) (define-syntax-public mm (syntax-rules () ((_) (helper)))) (define-module (t u) #:use-module (t m)) (define (helper) 'u) (list (mm) (helper))",
        "",
        "(h u)",
    ),
    (
        "(define-module (t m2)) (define-syntax-rule-public (twice e) (list e e)) (define-module (t u2) #:use-module (t m2)) (twice 1)",
        "",
        "(1 1)",
    ),
    (
        "(define-module (t m3)) (define-syntax-rule (private) 1) (define-module (t u3) #:use-module (t m3)) (private)",
        "unbound-variable",
        "Unbound variable: private",
    ),
    // `defmacro` and `define-macro`.
    (
        "(defmacro foo (a . b) `(list ,a ',b)) (foo 1 2 3)",
        "",
        "(1 (2 3))",
    ),
    ("(defmacro m args (cons 'list args)) (m 1 2)", "", "(1 2)"),
    (
        "(use-modules (ice-9 optargs)) (defmacro* m (a #:key (b 2) #:rest r) `'(,a ,b ,r)) (m 1 #:b 3 \"d\" 4)",
        "",
        "(1 3 (#:b 3 \"d\" 4))",
    ),
    (
        "(define-module (t d*)) (use-modules (ice-9 optargs)) (defmacro*-public m* (#:optional (x 1)) x) (define-module (t e*) #:use-module (t d*)) (list (m*) (m* 2))",
        "",
        "(1 2)",
    ),
    ("(define-macro (foo a) `(+ ,a 1)) (foo 1)", "", "2"),
    ("(define-macro foo (lambda (a) `(+ ,a 1))) (foo 1)", "", "2"),
    (
        "(define-module (t d)) (defmacro-public inc (x) `(+ ,x 1)) (define-module (t e) #:use-module (t d)) (inc 4)",
        "",
        "5",
    ),
    (
        "(defmacro unless2 (c . body) `(if ,c #f (begin ,@body))) (define (f x) (unless2 x 'ran)) (list (f #f) (f #t))",
        "",
        "(ran #f)",
    ),
    (
        "(defmacro)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: source expression failed to match any pattern in form (defmacro)",
    ),
    // A macro that defines a macro.
    (
        "(define-syntax def-const (syntax-rules () ((_ name v) (define-syntax name (syntax-rules () ((_) v)))))) (def-const seven 7) (seven)",
        "",
        "7",
    ),
    // `syntax-case`, the part on demand (D273), and its forms.
    (
        "(define-syntax swap! (lambda (x) (syntax-case x () ((_ a b) #'(let ((tmp a)) (set! a b) (set! b tmp)))))) (define tmp 1) (define y 2) (swap! tmp y) (list tmp y)",
        "",
        "(2 1)",
    ),
    (
        "(define-syntax my-or (lambda (x) (syntax-case x () ((_) #'#f) ((_ e) #'e) ((_ e r ...) #'(let ((t e)) (if t t (my-or r ...))))))) (let ((t 5)) (my-or #f t))",
        "",
        "5",
    ),
    (
        "(define-syntax aif (lambda (x) (syntax-case x () ((k c a b) (with-syntax ((it (datum->syntax #'k 'it))) #'(let ((it c)) (if it a b))))))) (list (aif (assq 'b '((a . 1) (b . 2))) (cdr it) 'none) (let ((it 5)) (aif #f it 'none)))",
        "",
        "(2 none)",
    ),
    (
        "(define-syntax lit (lambda (x) (syntax-case x (=>) ((_ a => b) #'(list a b)) ((_ a b c) #'(quote other))))) (list (lit 1 => 2) (lit 1 2 3))",
        "",
        "((1 2) other)",
    ),
    (
        "(define-syntax fend (lambda (x) (syntax-case x () ((_ n) (number? (syntax->datum #'n)) #'(quote num)) ((_ n) #'(quote other))))) (list (fend 1) (fend a))",
        "",
        "(num other)",
    ),
    (
        "(define-syntax qs (lambda (x) (syntax-case x () ((_ a ...) #`(list #,(length #'(a ...)) a ... #,@(map (lambda (v) (* 2 (syntax->datum v))) #'(a ...))))))) (qs 1 2 3)",
        "",
        "(3 1 2 3 2 4 6)",
    ),
    (
        "(define-syntax nest (lambda (x) (syntax-case x () ((_ (a b ...) ...) #'(quote ((b ... a) ...)))))) (nest (1 2 3) (4 5))",
        "",
        "((2 3 1) (5 4))",
    ),
    (
        "(define-syntax gt (lambda (x) (syntax-case x () ((_ a ...) (with-syntax (((t ...) (generate-temporaries #'(a ...)))) #'(let ((t a) ...) (list t ...))))))) (gt 1 2 3)",
        "",
        "(1 2 3)",
    ),
    (
        "(define-syntax idp (lambda (x) (syntax-case x () ((_ a) (if (identifier? #'a) #''id #''not))))) (list (idp x) (idp 1))",
        "",
        "(id not)",
    ),
    (
        "(define-syntax bad (lambda (x) (syntax-case x () ((_ a) #'a)))) (bad)",
        "syntax-error",
        "Syntax error:\nunknown file:1:64: source expression failed to match any pattern in form (bad)",
    ),
    (
        "(define-syntax viol (lambda (x) (syntax-violation 'viol \"no good\" x))) (viol 1)",
        "syntax-error",
        "Syntax error:\nunknown file:1:71: viol: no good in form (viol 1)",
    ),
    (
        "(define-syntax s2d (lambda (x) (syntax-case x () ((_ a) (datum->syntax x (list 'quote (syntax->datum #'a))))))) (s2d (x y))",
        "",
        "(x y)",
    ),
    (
        "(define-syntax ffi (lambda (x) (syntax-case x () ((_ a b) (if (free-identifier=? #'a #'b) #''same #''diff))))) (list (ffi car car) (ffi car cdr))",
        "",
        "(same diff)",
    ),
    (
        "(define-syntax bi (lambda (x) (syntax-case x () ((_ a) (if (bound-identifier=? #'a #'a) #''same #''diff))))) (bi z)",
        "",
        "same",
    ),
    (
        "(define-syntax vec (lambda (x) (syntax-case x () ((_ #(a ...)) #'(list a ...))))) (vec #(1 2 3))",
        "",
        "(1 2 3)",
    ),
    (
        "(define-syntax dot (lambda (x) (syntax-case x () ((_ a . b) #'(quote (a b)))))) (dot 1 2 3)",
        "",
        "(1 (2 3))",
    ),
    (
        "(define-syntax rec (lambda (x) (syntax-case x () ((_ n) (if (= 0 (syntax->datum #'n)) #'0 (with-syntax ((m (- (syntax->datum #'n) 1))) #'(+ 1 (rec m)))))))) (rec 4)",
        "",
        "4",
    ),
    ("(define-syntax plain (lambda (x) 42)) (plain)", "", "42"),
    (
        "(define-syntax tl (lambda (x) (syntax-case x () (_ #''any)))) (list (tl) (tl 1 2))",
        "",
        "(any any)",
    ),
    (
        "(let () (define-syntax loc (lambda (x) (syntax-case x () ((_ a) #'(* a 2))))) (loc 21))",
        "",
        "42",
    ),
    (
        "(define (f x) (syntax-case x () ((a b) #'b))) (f '(1 2))",
        "",
        "2",
    ),
    ("(syntax->datum #'(a b))", "", "(a b)"),
    (
        "(define-syntax pv (lambda (x) (syntax-case x () ((_ a) a)))) (pv 1)",
        "syntax-error",
        "Syntax error:\nunknown location: reference to pattern variable outside syntax form in form a",
    ),
    (
        "(define-syntax el (lambda (x) (syntax-case x () ((_ a ...) #'(a))))) (el 1)",
        "syntax-error",
        "Syntax error:\nunknown file:1:59: syntax: missing ellipsis in form (syntax (a))",
    ),
    (
        "(define-syntax uq (lambda (x) (syntax-case x () ((_ a) #`(quote #,(+ 1 (syntax->datum #'a))))))) (uq 4)",
        "",
        "5",
    ),
];

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
fn macros_give_what_guile_gives() {
    let mut failures = Vec::new();
    for &(source, key, text) in CASES {
        let (got_key, got) = run(&mut Vm::new(), source);
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

/// What LilyPond adds to Guile's forms and the engine has natively:
/// `define*-public` from `(ice-9 optargs)`, `define-syntax-public` and
/// `define-syntax-rule-public` from `lily.scm`; defined in `(guile)` for
/// guile's runs, so that every module sees them as the engine's do.
const GUILE_PRELUDE: &str = r#"(eval '(begin
  (define-syntax define*-public
    (syntax-rules ()
      ((_ (id . args) b ...) (define-public id (lambda* args b ...)))
      ((_ id val) (define-public id val))))
  (define-syntax-rule (define-syntax-public name . rest)
    (begin (define-syntax name . rest) (export name)))
  (define-syntax-rule (define-syntax-rule-public (name . args) . rest)
    (begin (define-syntax-rule (name . args) . rest) (export name))))
  (resolve-module '(guile)))
"#;

/// Runs `source` in a fresh guile, its forms read and evaluated in turn as
/// `tests/modules.rs` does; `None` without guile.
fn guile(source: &str) -> Option<(String, String)> {
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
    let program = format!("{GUILE_PRELUDE}{runner}(run {})\n", literal(source));
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

// --- `(lily)`'s native macros against the installed LilyPond's definitions

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

/// The definitions the programs use, from the installed files.
fn lily_definitions(lily: &Path) -> String {
    [
        ("lily-library.scm", "(define-syntax-public assert"),
        (
            "music-functions.scm",
            "(define-syntax-public define-syntax-function",
        ),
        (
            "music-functions.scm",
            "(define-syntax-rule-public (define-music-function",
        ),
        (
            "music-functions.scm",
            "(define-syntax-rule-public (define-scheme-function",
        ),
    ]
    .map(|(file, head)| definition(lily, file, head))
    .join("\n")
}

/// Stand-ins, in `(lily)`, for what the definitions call that the engine
/// does not have until later steps (`ly:make-music-function` returns its
/// arguments); the property that records a music function's arguments is a
/// procedure with a setter, as `music-functions.scm`'s object property is.
const LILY_STANDINS: &str = "(define recorded '())
(define syntax-function-procedure-arguments
  (make-procedure-with-setter
    (lambda (p) (let ((e (assq p recorded))) (and e (cdr e))))
    (lambda (p v) (set! recorded (cons (cons p v) recorded)))))
(define (ly:make-music-function signature procedure) (list signature procedure))
(define (*parser*) 'parser)
(define (*location*) 'location)
(define (scheme? x) #t)
(define (res? x) #t)
(define (n? x) #t)
(define (shape signature) (map (lambda (s) (if (pair? s) (cdr s) 'plain)) signature))
(define (ly:music? x) #f)
(define (make-music . args) args)
";

/// What guile needs besides: the modules the files import, which the
/// engine has from step 10b.
const GUILE_STANDINS: &str = "(use-modules (ice-9 match) (srfi srfi-11))
";

/// Programs run in `(lily)` after the stand-ins and the definitions.
const LILY_CASES: &[(&str, &str, &str)] = &[
    ("(assert (= 1 1))", "", "#<unspecified>"),
    (
        "(catch 'misc-error (lambda () (assert (= 1 2))) (lambda (k s m a r) a))",
        "",
        "(\"assertion (= 1 2) failed\")",
    ),
    (
        "(catch 'misc-error (lambda () (assert #f \"why\")) (lambda (k s m a r) a))",
        "",
        "(\"assertion #f failed with message: why\")",
    ),
    (
        "(catch 'misc-error (lambda () (let ((not (lambda (x) #f)) (error 0)) (assert #f))) (lambda (k . r) 'raised))",
        "",
        "raised",
    ),
    (
        "(module-ref (current-module) 'assert)",
        "",
        "#<syntax-transformer assert>",
    ),
    (
        "(define-module (t user) #:use-module (lily)) (catch 'misc-error (lambda () (assert #f)) (lambda (k . r) k))",
        "",
        "misc-error",
    ),
    (
        "(let ((r (define-syntax-function res? (a b) (n? (n? 5)) \"doc\" (+ a b)))) (list (shape (car r)) ((cadr r) 1 2) (syntax-function-procedure-arguments (cadr r))))",
        "",
        "((plain plain 5) 3 (a b))",
    ),
    (
        "(let ((r (define-syntax-function res? (parser location x) (n?) (list parser location x)))) (list ((cadr r) 7) (syntax-function-procedure-arguments (cadr r))))",
        "",
        "((parser location 7) (x))",
    ),
    (
        "(let ((r (define-syntax-function res? ((a b) c) (n? n?) (list a b c)))) (list (((cadr r) 1 2) 3) (syntax-function-procedure-arguments (cadr r))))",
        "",
        "((1 2 3) (a b))",
    ),
    (
        "(let ((r (define-syntax-function res? (x) (n?) (_i \"doc\") (* x 2)))) ((cadr r) 4))",
        "",
        "8",
    ),
    (
        "(let ((r (define-syntax-function res? () () \"only\"))) ((cadr r)))",
        "",
        "\"only\"",
    ),
    (
        "(let ((r (define-syntax-function res? (parser location . rest) (n?) (list parser location rest)))) ((cadr r) 1 2))",
        "",
        "(parser location (1 2))",
    ),
    (
        "(let ((r (define-syntax-function res? (x . rest) (n?) (list x rest)))) ((cadr r) 1 2))",
        "",
        "(1 (2))",
    ),
    (
        "(let ((result 5)) (let ((r (define-syntax-function res? (x) (n?) (+ x result)))) ((cadr r) 1)))",
        "",
        "6",
    ),
    (
        "(let ((r (define-scheme-function (x) (number?) (* x 3)))) (list (map procedure? (car r)) ((cadr r) 2)))",
        "",
        "((#t #t) 6)",
    ),
    (
        "(let ((r (define-music-function (m) ((number? 1)) m))) (list (map pair? (car r)) (cdr (caar r)) ((cadr r) 2)))",
        "",
        "((#t #t) (Music) 2)",
    ),
    (
        "(catch 'syntax-error (lambda () (eval '(define-syntax-function) (current-module))) (lambda (k who what where form subform) (list who what form)))",
        "",
        "(#f \"source expression failed to match any pattern\" (define-syntax-function))",
    ),
];

#[test]
fn lily_macros_give_what_guile_gives() {
    let Some(lily) = lily_scm() else {
        eprintln!("LilyPond 2.26.0 is not installed: (lily)'s native macros are unverified");
        return;
    };
    let definitions = lily_definitions(&lily);
    let mut failures = Vec::new();
    for &(source, key, text) in LILY_CASES {
        let program = format!("(define-module (lily))\n{LILY_STANDINS}");
        let mut vm = Vm::new();
        let prelude = format!("{program}{definitions}");
        if let Err(e) = vm.eval_text(&prelude, None) {
            panic!("the definitions: {}", e.text(vm.heap()));
        }
        let got = run(&mut vm, source);
        let guile_program = format!("{program}{GUILE_STANDINS}{definitions}\n{source}");
        let from_guile = guile(&guile_program);
        for (who, (got_key, got)) in [("engine", Some(got)), ("guile", from_guile)]
            .into_iter()
            .filter_map(|(who, r)| Some((who, r?)))
        {
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
        LILY_CASES.len(),
        failures.join("\n")
    );
}

/// The procedure of a music function carries its signature, unevaluated, in
/// its header; any other procedure has none.
#[test]
fn a_music_function_carries_its_signature() {
    let mut vm = Vm::new();
    let program = format!(
        "(define-module (lily))\n{LILY_STANDINS}
         (list (cadr (define-syntax-function res? (a b) (n? (n? 5)) (+ a b)))
               (lambda (x) x))"
    );
    let value = vm.eval_text(&program, None).unwrap();
    let items = vm.heap().car(value).zip(vm.heap().cdr(value)).unwrap();
    let header = vm.procedure_header(items.0).unwrap();
    let mut out = String::new();
    vm.heap().write(header, &mut out);
    assert_eq!(out, "(res? n? (n? 5))");
    let other = vm.heap().car(items.1).unwrap();
    assert_eq!(vm.procedure_header(other), None);
    // Nor has any value that is no procedure, as the parser asks of each.
    for value in [items.1, Value::fixnum(155).unwrap(), Value::FALSE] {
        assert_eq!(vm.procedure_header(value), None);
    }
}

/// A `define-syntax` whose transformer is a procedure (`syntax-case`'s)
/// expands (D273); where the name is one of `(lily)`'s native macros, the
/// native one stays and the transformer is not evaluated (D60).
#[test]
fn procedure_transformers_expand_and_natives_stay() {
    let mut vm = Vm::new();
    let (_, value) = run(
        &mut vm,
        "(define-syntax foo (lambda (x) (syntax-case x () ((_) #'1)))) (foo)",
    );
    assert_eq!(value, "1");
    let (_, value) = run(
        &mut vm,
        "(define-module (lily)) (define-syntax-public assert (lambda (x) (error \"never evaluated\"))) (module-ref (current-module) 'assert)",
    );
    assert_eq!(value, "#<syntax-transformer assert>");
}

/// A transformer that allocates enough to collect runs while the expander
/// holds the form; the forms and the macro survive, and so does a
/// collection after them.
#[test]
fn transformers_run_while_the_expander_holds_values() {
    let mut vm = Vm::new();
    let program = "(defmacro big (x)
                     (let loop ((i 0) (l '()))
                       (if (< i 300000) (loop (+ i 1) (cons i l)) `(list ,x ',(length l)))))
                   (define (f) (list (big 'a) (big 'b)))
                   (f)";
    assert_eq!(run(&mut vm, program).1, "((a 300000) (b 300000))");
    vm.collect();
    assert_eq!(run(&mut vm, "(f)").1, "((a 300000) (b 300000))");
}

/// A quasiquotation of 5,000 items is one call, not 5,000 nested ones, so
/// it compiles on wasm's 1 MiB stack.
#[test]
fn long_quasiquotes_do_not_nest() {
    std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(|| {
            let mut vm = Vm::new();
            let items = (0..2500).map(|i| format!(" ,x {i}")).collect::<String>();
            let program = format!("(let ((x 'u)) (length `({items})))");
            assert_eq!(run(&mut vm, &program).1, "5000");
        })
        .unwrap()
        .join()
        .unwrap();
}
