//! S3's primitives (chapter 03 §2.6), those LilyPond's library files call:
//! the (scheme base) names Guile binds, Guile's list, string and character
//! procedures, `(srfi srfi-1)`, hash tables, object and procedure
//! properties, procedures with setters, keywords, `simple-format` and the
//! subset of `(ice-9 format)`, and string ports give what Guile 3.0.11
//! gives. With `guile` installed the table is also run by guile, each call
//! in a fresh process; without it, the table is unverified against it.
//!
//! Hash tables are compared on what does not depend on the order of their
//! entries, which is the order they were added in the engine and Guile's
//! buckets' in Guile.
use std::process::Command;

use lily_engine_scheme::Vm;

/// Calls and what they give: the last form's value as `write` writes it,
/// or the key and the text of the error, as in `tests/macros.rs`.
const CASES: &[(&str, &str, &str)] = &[
    // (scheme base) names and numbers.
    (r#"(char>? #\b #\a #\A)"#, "", "#t"),
    (r#"(char<=? #\a #\a #\b)"#, "", "#t"),
    (r#"(char-ci=? #\a #\A)"#, "", "#t"),
    (
        r#"(list (char-upper-case? #\A) (char-lower-case? #\A))"#,
        "",
        "(#t #f)",
    ),
    (r#"(string>? "b" "a" "A")"#, "", "#t"),
    (r#"(string<=? "a" "a" "b")"#, "", "#t"),
    (r#"(string-ci=? "abc" "ABC")"#, "", "#t"),
    (
        r#"(string>? "a" 1)"#,
        "wrong-type-arg",
        "In procedure string>: Wrong type argument in position 2 (expecting string): 1",
    ),
    (
        "(call-with-values (lambda () (floor/ 7 -2)) list)",
        "",
        "(-4 -1)",
    ),
    (
        "(list (floor-quotient 7 -2) (floor-remainder 7 -2) (truncate-quotient 7 -2) (truncate-remainder 7 -2))",
        "",
        "(-4 -1 -3 1)",
    ),
    (
        "(list (ceiling-quotient 7 2) (euclidean-quotient -7 2) (euclidean-remainder -7 2) (round-quotient 7 2))",
        "",
        "(4 -4 1 4)",
    ),
    (
        "(call-with-values (lambda () (exact-integer-sqrt 17)) list)",
        "",
        "(4 1)",
    ),
    (
        "(floor-quotient 1 0)",
        "numerical-overflow",
        r#"Throw to key `numerical-overflow' with args `("floor-quotient" "Numerical overflow" #f #f)'."#,
    ),
    (
        "(list (ash 1 10) (ash -8 -1) (logand 12 10) (logior 12 10) (logxor 12 10) (lognot 5))",
        "",
        "(1024 -4 8 14 6 -6)",
    ),
    (
        "(list (logand) (logior) (logior 5) (logtest 4 5) (logbit? 2 4) (logcount 7) (integer-length 255))",
        "",
        "(-1 0 5 #t #t 3 8)",
    ),
    (
        "(list (nan? (/ 0. 0.)) (inf? (/ 1. 0.)) (finite? 1.5) (inf? 1))",
        "",
        "(#t #t #t #f)",
    ),
    (
        "(list (magnitude -3) (real-part 1+2i) (imag-part 1+2i) (angle 1))",
        "",
        "(3 1.0 2.0 0.0)",
    ),
    (
        "(list (asin 0) (acos 1) (make-rectangular 1 2))",
        "",
        "(0 0 1.0+2.0i)",
    ),
    (
        "(let ((l (list 1 2 3))) (list-set! l 1 'x) l)",
        "",
        "(1 x 3)",
    ),
    ("(list (make-list 2 'x) (make-list 0))", "", "((x x) ())"),
    (
        "(let ((v (vector 1 2 3 4))) (list (vector-copy v) (vector-copy v 1) (vector-copy v 1 3)))",
        "",
        "(#(1 2 3 4) #(2 3 4) #(2 3))",
    ),
    (
        "(let ((v (vector 1 2 3 4))) (vector-copy! v 0 #(a b) 0 2) v)",
        "",
        "#(a b 3 4)",
    ),
    (
        r#"(let ((s (make-string 4 #\a))) (string-copy! s 1 "xyz" 1) s)"#,
        "",
        r#""ayza""#,
    ),
    (
        r#"(let ((s (make-string 3 #\a))) (string-fill! s #\z) s)"#,
        "",
        r#""zzz""#,
    ),
    (
        r#"(let ((s (string #\a #\b #\c))) (string-fill! s #\z 1 2) s)"#,
        "",
        r#""azc""#,
    ),
    (
        r#"(list (string-copy "abcd" 1) (string-copy "abcd" 1 3))"#,
        "",
        r#"("bcd" "bc")"#,
    ),
    (r#"(string-map char-upcase "abc")"#, "", r#""ABC""#),
    (
        r#"(let ((n 0)) (string-for-each (lambda (c) (set! n (+ n 1))) "abc") n)"#,
        "",
        "3",
    ),
    (
        r#"(string-for-each 5 "a")"#,
        "wrong-type-arg",
        "In procedure string-for-each: Wrong type argument in position 1: 5",
    ),
    (
        "(list *unspecified* (identity 3))",
        "",
        "(#<unspecified> 3)",
    ),
    (
        "(list (->bool #f) (->bool '()) (->bool 0))",
        "",
        "(#f #t #t)",
    ),
    (
        "(list (caddar '((1 2 3))) (cddddr '(1 2 3 4 5)) (caaar '(((1)))) (cdadr '(1 (2 3))))",
        "",
        "(3 (5) 1 (3))",
    ),
    // Guile's lists.
    ("(acons 'a 1 '((b . 2)))", "", "((a . 1) (b . 2))"),
    (
        "(list (assq-ref '((a . 1) (b . 2)) 'b) (assq-ref '((a . 1)) 'z) (assq-ref '(5 (a . 1)) 'a) (assq-ref 5 'a))",
        "",
        "(2 #f 1 #f)",
    ),
    (
        r#"(list (assv-ref '((1 . a)) 1) (assoc-ref '(("a" . 1)) "a"))"#,
        "",
        "(a 1)",
    ),
    ("(assq-set! '() 'a 1)", "", "((a . 1))"),
    (
        "(let ((l (list (cons 'a 1) (cons 'b 2)))) (assq-set! l 'b 3) l)",
        "",
        "((a . 1) (b . 3))",
    ),
    (
        r#"(let ((l (list (cons "a" 1)))) (assoc-set! l "a" 2))"#,
        "",
        r#"(("a" . 2))"#,
    ),
    (
        "(assq-remove! (list (cons 'a 1) (cons 'b 2) (cons 'a 3)) 'a)",
        "",
        "((b . 2) (a . 3))",
    ),
    (
        r#"(assoc-remove! (list (cons "a" 1) (cons "b" 2)) "b")"#,
        "",
        r#"(("a" . 1))"#,
    ),
    (
        "(list (iota 3) (iota 3 1) (iota 3 1 2) (iota 3 1.5) (iota 0))",
        "",
        "((0 1 2) (1 2 3) (1 3 5) (1.5 2.5 3.5) ())",
    ),
    (
        "(iota -1)",
        "wrong-type-arg",
        "Throw to key `wrong-type-arg' with args `(-1)'.",
    ),
    // An inexact count, as Guile 2.2's iota takes it (make-dotted-bar-line's
    // (iota 3.0 …), MS8 step 7): LilyPond 2.26.0's answers.
    (
        "(list (iota 3.0) (iota 3.0 1) (iota 3 1 0.5) (iota 3.0 1 2) (iota 0.0 1 2) (iota 3 1.0 2))",
        "",
        "((0.0 1.0 2.0) (1.0 2.0 3.0) (1.0 1.5 2.0) (1.0 3.0 5.0) () (1.0 3.0 5.0))",
    ),
    (
        "(iota 2.5)",
        "wrong-type-arg",
        "Throw to key `wrong-type-arg' with args `(2.5)'.",
    ),
    (
        "(iota +inf.0)",
        "wrong-type-arg",
        "Throw to key `wrong-type-arg' with args `(+inf.0)'.",
    ),
    (
        "(iota -1.0)",
        "wrong-type-arg",
        "Throw to key `wrong-type-arg' with args `(-1.0)'.",
    ),
    ("(reverse! (list 1 2 3))", "", "(3 2 1)"),
    ("(reverse! (list 1 2 3) '(4))", "", "(3 2 1 4)"),
    // What a markup command's failed argument leaves (case 20, D64).
    (
        r#"(reverse! (cons* 2 "big" #f))"#,
        "wrong-type-arg",
        r#"In procedure reverse!: Wrong type argument in position 1: (2 "big" . #f)"#,
    ),
    (
        r#"(let ((l (cons* 1 2 3))) (catch #t (lambda () (reverse! l '(9))) (lambda _ l)))"#,
        "",
        "(1 2 . 3)",
    ),
    ("(append! (list 1) '() (list 2 3) 4)", "", "(1 2 3 . 4)"),
    ("(append!)", "", "()"),
    (
        "(let* ((l (list 1 2)) (m (append! l (list 3)))) (list l m))",
        "",
        "((1 2 3) (1 2 3))",
    ),
    (
        r#"(list (delq 'a '(a b a c)) (delv 1 '(1 2 1)) (delete "a" '("a" "b")))"#,
        "",
        r#"((b c) (2) ("b"))"#,
    ),
    ("(let ((l '(a b c))) (eq? (cdr l) (delq 'a l)))", "", "#f"),
    (
        r#"(list (delq! 'a (list 'a 'b 'a)) (delete! "a" (list "a" "b")) (delq1! 'a (list 'a 'b 'a)))"#,
        "",
        r#"((b) ("b") (b a))"#,
    ),
    (
        "(list (list-head '(1 2 3) 2) (list-head '(1 2 3) 0))",
        "",
        "((1 2) ())",
    ),
    (
        "(list-head '(1 2 3) 4)",
        "wrong-type-arg",
        "In procedure list-head: Wrong type argument in position 1 (expecting pair): ()",
    ),
    (
        "(list (filter odd? '(1 2 3 4 5)) (filter odd? '()))",
        "",
        "((1 3 5) ())",
    ),
    (
        "(filter odd? 5)",
        "wrong-type-arg",
        "In procedure filter: Wrong type argument in position 2: 5",
    ),
    (
        "(list (sort '(3 1 2) <) (sort (vector 3 1 2) <) (sort '() <))",
        "",
        "((1 2 3) #(1 2 3) ())",
    ),
    (
        "(stable-sort '((1 . a) (0 . b) (1 . c) (0 . d)) (lambda (x y) (< (car x) (car y))))",
        "",
        "((0 . b) (0 . d) (1 . a) (1 . c))",
    ),
    (
        "(list (merge '(1 3 5) '(2 4) <) (sorted? '(1 2 2) <) (sorted? '(2 1) <) (sorted? #(1 2) <))",
        "",
        "((1 2 3 4 5) #t #f #t)",
    ),
    ("(map-in-order + '(1 2) '(10 20))", "", "(11 22)"),
    (
        "(list (sort-list (list 3 1 2) <) (sort-list! (list 2 1) <))",
        "",
        "((1 2 3) (1 2))",
    ),
    (
        "(sort-list (vector 3 1 2) <)",
        "wrong-type-arg",
        "In procedure sort-list: Wrong type argument in position 1: #(3 1 2)",
    ),
    // SRFI-1.
    (
        "(use-modules (srfi srfi-1)) (list (first '(1 2 3)) (second '(1 2 3)) (third '(1 2 3)) (fifth '(1 2 3 4 5 6)) (tenth (iota 10)))",
        "",
        "(1 2 3 5 9)",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (take '(1 2 3) 2) (drop '(1 2 3) 2) (take-right '(1 2 3) 2) (drop-right '(1 2 3) 2) (last '(1 2 3)))",
        "",
        "((1 2) (3) (2 3) (1) 3)",
    ),
    (
        "(use-modules (srfi srfi-1)) (call-with-values (lambda () (split-at '(1 2 3 4) 1)) list)",
        "",
        "((1) (2 3 4))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (fold cons '() '(1 2 3)) (fold + 0 '(1 2) '(10 20)) (fold-right cons '() '(1 2 3)) (fold-right list 'z '(1 2) '(a b)))",
        "",
        "((3 2 1) 33 (1 2 3) (1 a (2 b z)))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (reduce + 0 '(1 2 3)) (reduce + 0 '()) (reduce-right list 0 '(1 2 3)) (reduce max 0 '(3 9 2)))",
        "",
        "(6 0 (1 (2 3)) 9)",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (any odd? '(2 4 5)) (any odd? '()) (any (lambda (x) (and (odd? x) x)) '(2 3 5)) (any < '(1 5) '(2 4)))",
        "",
        "(#t #f 3 #t)",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (every odd? '(1 3)) (every odd? '()) (every (lambda (x) (and (odd? x) x)) '(1 3)) (every < '(1 2) '(2 1)))",
        "",
        "(#t #t 3 #f)",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (count odd? '(1 2 3)) (count < '(1 5) '(2 4)))",
        "",
        "(2 1)",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (append-map (lambda (x) (list x x)) '(1 2)) (filter-map (lambda (x) (and (odd? x) (* x x))) '(1 2 3)))",
        "",
        "((1 1 2 2) (1 9))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (find even? '(1 2 4)) (find even? '(1)) (find-tail even? '(1 2 4)) (remove odd? '(1 2 3)))",
        "",
        "(2 #f (2 4) (2))",
    ),
    (
        "(use-modules (srfi srfi-1)) (call-with-values (lambda () (partition odd? '(1 2 3 4))) list)",
        "",
        "((1 3) (2 4))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (take-while odd? '(1 3 4 5)) (drop-while odd? '(1 3 4 5)))",
        "",
        "((1 3) (4 5))",
    ),
    (
        "(use-modules (srfi srfi-1)) (call-with-values (lambda () (span odd? '(1 3 4 5))) (lambda (a b) (call-with-values (lambda () (break odd? '(2 4 5 6))) (lambda (c d) (list a b c d)))))",
        "",
        "((1 3) (4 5) (2 4) (5 6))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (list-index even? '(1 3 4)) (list-index even? '(1)) (list-index < '(3 1) '(1 2)))",
        "",
        "(2 #f 1)",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (delete 3 '(1 3 2 3)) (delete 3 '(1 3 4) <) (member 2.0 '(1 2 3) =) (assoc 2.0 '((1 . a) (2 . b)) =))",
        "",
        "((1 2) (1 3) (2 3) (2 . b))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (delete-duplicates '(a b a c b)) (delete-duplicates '(1 2 1.0) =))",
        "",
        "((a b c) (1 2))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (lset-union eq? '(a b) '(b c)) (lset-intersection eq? '(a b c) '(b c d)) (lset-difference eq? '(a b c) '(b)) (lset-adjoin eq? '(a b) 'c 'a))",
        "",
        "((c a b) (b c) (a c) (c a b))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (lset= eq? '(a b) '(b a)) (lset<= eq? '(a) '(a b)) (lset<= eq? '(c) '(a)) (lset-xor eq? '(a b) '(b c)))",
        "",
        "(#t #t #f (c a))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (alist-cons 'a 1 '()) (alist-delete 'a '((a . 1) (b . 2) (a . 3))) (alist-copy '((a . 1))))",
        "",
        "(((a . 1)) ((b . 2)) ((a . 1)))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (concatenate '((1) (2 3) ())) (append-reverse '(2 1) '(3)) (xcons 1 2) (list-tabulate 4 (lambda (i) (* i i))))",
        "",
        "((1 2 3) (1 2 3) (2 . 1) (0 1 4 9))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (zip '(1 2) '(a b)) (unzip1 '((1) (2))) (length+ '(1 2)) (length+ (circular-list 1 2)))",
        "",
        "(((1 a) (2 b)) (1 2) 2 #f)",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (proper-list? '(1)) (dotted-list? '(1 . 2)) (circular-list? (circular-list 1)) (null-list? '()) (not-pair? 1))",
        "",
        "(#t #t #t #t #t)",
    ),
    (
        "(use-modules (srfi srfi-1)) (call-with-values (lambda () (car+cdr '(1 . 2))) list)",
        "",
        "(1 2)",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (unfold (lambda (x) (> x 3)) (lambda (x) (* x x)) (lambda (x) (+ x 1)) 1) (unfold-right zero? (lambda (x) (* x x)) (lambda (x) (- x 1)) 3))",
        "",
        "((1 4 9) (1 4 9))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (pair-fold cons '() '(1 2)) (pair-fold-right cons '() '(1 2)) (map + '(1 2 3) '(10 20)))",
        "",
        "(((2) (1 2)) ((1 2) (2)) (11 22))",
    ),
    (
        "(use-modules (srfi srfi-1)) (let ((acc '())) (for-each (lambda (a b) (set! acc (cons (+ a b) acc))) '(1 2 3) '(10 20)) acc)",
        "",
        "(22 11)",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (list= eq? '(a b) '(a b)) (list= eq? '(a) '(a b)) (list-copy '(1 2 . 3)))",
        "",
        "(#t #f (1 2 . 3))",
    ),
    (
        "(use-modules (srfi srfi-1)) (list (map! (lambda (x) (* x 2)) (list 1 2)) (filter! odd? (list 1 2 3)) (remove! odd? (list 1 2 3)) (delete! 1 (list 1 2)))",
        "",
        "((2 4) (1 3) (2) (2))",
    ),
    (
        "(use-modules (srfi srfi-1)) (call-with-values (lambda () (unzip2 '((1 a) (2 b)))) list)",
        "",
        "((1 2) (a b))",
    ),
    (
        "(fold cons '() '(1))",
        "unbound-variable",
        "Unbound variable: fold",
    ),
    // Strings.
    (
        r#"(list (string-null? "") (string-null? "a"))"#,
        "",
        "(#t #f)",
    ),
    (
        "(string-null? 5)",
        "wrong-type-arg",
        "In procedure string-null?: Wrong type argument in position 1 (expecting string): 5",
    ),
    (
        r#"(list (string-join '("a" "b" "c")) (string-join '("a" "b") ", ") (string-join '() ","))"#,
        "",
        r#"("a b c" "a, b" "")"#,
    ),
    (
        r#"(list (string-join '("a" "b") "," 'suffix) (string-join '("a" "b") "," 'prefix) (string-join '("a") "," 'strict-infix))"#,
        "",
        r#"("a,b," ",a,b" "a")"#,
    ),
    (
        r#"(string-join '() "," 'strict-infix)"#,
        "misc-error",
        "In procedure string-join: strict-infix grammar requires non-empty list",
    ),
    (
        r#"(list (string-split "a.b..c" #\.) (string-split "" #\a) (string-split "a b" char-whitespace?))"#,
        "",
        r#"(("a" "b" "" "c") ("") ("a" "b"))"#,
    ),
    (
        r#"(list (string-index "abc" #\b) (string-index "abc" #\z) (string-index "abcb" #\b 2) (string-index "a1" char-numeric?))"#,
        "",
        "(1 #f 3 1)",
    ),
    (
        r#"(list (string-rindex "abcb" #\b) (string-rindex "abc" #\z))"#,
        "",
        "(3 #f)",
    ),
    (
        r#"(string-index "abc" 5)"#,
        "wrong-type-arg",
        "In procedure string-index: Wrong type argument in position 2: 5",
    ),
    (
        r#"(list (string-contains "abcd" "cd") (string-contains "abcd" "x") (string-contains "abab" "ab" 1) (string-contains-ci "ABC" "bc"))"#,
        "",
        "(2 #f 2 1)",
    ),
    (
        r#"(list (string-prefix? "ab" "abc") (string-prefix? "abc" "ab") (string-suffix? "bc" "abc") (string-prefix-ci? "AB" "abc"))"#,
        "",
        "(#t #f #t #t)",
    ),
    (
        r#"(list (string-trim "  a  ") (string-trim-right "  a  ") (string-trim-both "  a  ") (string-trim-both "xxaxx" #\x))"#,
        "",
        r#"("a  " "  a" "a" "a")"#,
    ),
    (
        r#"(list (string-delete #\a "banana") (string-delete char-alphabetic? "a1b2") (string-filter char-alphabetic? "a1b2"))"#,
        "",
        r#"("bnn" "12" "ab")"#,
    ),
    (
        r#"(list (string-count "banana" #\a) (string-any char-numeric? "a1b") (string-every char-numeric? "12") (string-every char-numeric? ""))"#,
        "",
        "(3 #t #t #t)",
    ),
    (
        r#"(list (string-upcase "aé") (string-downcase "ABC") (string-capitalize "hello wORLD") (string-titlecase "x-ray"))"#,
        "",
        r#"("AÉ" "abc" "Hello World" "X-Ray")"#,
    ),
    (
        r#"(list (string-pad "abc" 5) (string-pad "abcdef" 3) (string-pad-right "abc" 5 #\*) (string-pad-right "abcdef" 3))"#,
        "",
        r#"("  abc" "def" "abc**" "abc")"#,
    ),
    (
        r#"(list (string-take "abc" 2) (string-drop "abc" 2) (string-take-right "abc" 2) (string-drop-right "abc" 2))"#,
        "",
        r#"("ab" "c" "bc" "a")"#,
    ),
    (
        r#"(string-take "abc" 4)"#,
        "out-of-range",
        "Value out of range 0 to< 3: 4",
    ),
    (
        r#"(list (string-tokenize "  a b  c ") (string-reverse "abc") (reverse-list->string '(#\a #\b)) (string-concatenate '("a" "b")))"#,
        "",
        r#"(("a" "b" "c") "cba" "ba" "ab")"#,
    ),
    (
        r#"(list (substring/shared "abc" 1) (string-fold cons '() "ab") (string-fold-right cons '() "ab"))"#,
        "",
        r#"("bc" (#\b #\a) (#\a #\b))"#,
    ),
    (
        r#"(list (string= "ab" "ab") (string< "a" "b") (string<> "a" "a") (string-ci= "Ab" "aB") (string< "abc" "abd" 0 2 0 2))"#,
        "",
        "(#t #t #f #t #f)",
    ),
    (
        r#"(list (symbol #\a #\b) (symbol-append 'a 'bc) (symbol))"#,
        "",
        "(ab abc #{}#)",
    ),
    (
        r#"(list (basename "/a/b.ly" ".ly") (basename "/a/b/") (basename "/") (basename "a/b.ly" "ly") (basename "b.ly" "b.ly"))"#,
        "",
        r#"("b" "b/" "/" "b." "b.ly")"#,
    ),
    (
        r#"(list (dirname "/a/b") (dirname "b") (dirname "/b") (dirname "a/b//") (dirname "") (dirname "//a"))"#,
        "",
        r#"("/a" "." "/" "a" "." "/")"#,
    ),
    (
        "(list (keyword->symbol #:a) (symbol->keyword 'a) (keyword? (symbol->keyword 'b)))",
        "",
        "(a #:a #t)",
    ),
    (
        "(keyword->symbol 'a)",
        "wrong-type-arg",
        "In procedure keyword->symbol: Wrong type argument in position 1 (expecting keyword): a",
    ),
    (
        r#"(list (symbol? (gensym)) (eq? (gensym) (gensym)) (string-prefix? "x" (symbol->string (gensym "x"))))"#,
        "",
        "(#t #f #t)",
    ),
    (r#"(object->string "a")"#, "", r#""\"a\"""#),
    (
        r#"(list (object->string 'a) (object->string "a" display) (object->string '(1 "b")))"#,
        "",
        r#"("a" "a" "(1 \"b\")")"#,
    ),
    // Arrays (MS4 step 17, D229), as determine-frets' barre-list calls them.
    (
        "(list (make-array 0 3) (make-array 0 '(0 4) 2) (make-array 'x '(1 2) 2) (make-array 7) (make-array 1 0 2))",
        "",
        "(#(0 0 0) #2((0 0) (0 0) (0 0) (0 0) (0 0)) #2@1@0((x x) (x x)) #0(7) #2:0:2())",
    ),
    (
        "(let ((a (make-array 0 '(0 2) 3))) (array-set! a 'barre 1 0) (array-set! a 5 2 2) (list a (array-ref a 1 0) (array->list a) (array->list (make-array 7)) (array->list (make-array 1 0 2))))",
        "",
        "(#2((0 0 0) (barre 0 0) (0 0 5)) barre ((0 0 0) (barre 0 0) (0 0 5)) 7 ())",
    ),
    (
        "(list (map array? (list (make-array 0 2 2) #(1) \"s\" '(1) 5)) (equal? (make-array 0 2 2) (make-array 0 2 2)) (array-ref (make-array 'v 2) 1))",
        "",
        "((#t #t #t #f #f) #t v)",
    ),
    (
        "(array-ref (make-array 0 2 2) 2 0)",
        "out-of-range",
        "Value out of range 0 to 1: 2",
    ),
    (
        "(array-set! (make-array 0 '(1 2) 2) 1 0 0)",
        "out-of-range",
        "Value out of range 1 to 2: 0",
    ),
    (
        "(array-ref (make-array 0 2 2) 0)",
        "misc-error",
        "wrong number of indices, expecting 2",
    ),
    (
        "(array-ref 5 0)",
        "wrong-type-arg",
        "Wrong type (expecting array): 5",
    ),
    (
        "(array-ref (make-array 0 2 2) 'a 0)",
        "wrong-type-arg",
        "Wrong type (expecting exact integer): a",
    ),
    (
        "(make-array 0 '(3 1))",
        "misc-error",
        "bad array bound (3 1)",
    ),
    // Hash tables.
    (
        r#"(let ((t (make-hash-table))) (hash-set! t "a" 1) (hash-set! t '(1 2) 2) (list (hash-ref t "a") (hash-ref t (list 1 2)) (hash-ref t "z") (hash-ref t "z" 'd)))"#,
        "",
        "(1 2 #f d)",
    ),
    (
        "(let ((t (make-hash-table))) (hashq-set! t 'a 1) (list (hashq-ref t 'a) (hashq-ref t 'b 0) (hash-table? t) (hash-table? 5)))",
        "",
        "(1 0 #t #f)",
    ),
    (
        r#"(let ((t (make-hash-table))) (hashv-set! t 1.5 'x) (hashv-set! t 100000000000000000000 'y) (list (hashv-ref t 1.5) (hashv-ref t 100000000000000000000) (hashq-ref t "no")))"#,
        "",
        "(x y #f)",
    ),
    (
        "(let ((t (make-hash-table))) (list (hashq-set! t 'a 1) (hashq-remove! t 'a) (hashq-remove! t 'a) (hashq-ref t 'a)))",
        "",
        "(1 (a . 1) #f #f)",
    ),
    (
        "(let ((t (make-hash-table))) (hashq-set! t 'a 1) (let ((h (hashq-get-handle t 'a))) (set-cdr! h 5) (list h (hashq-ref t 'a) (hashq-get-handle t 'b))))",
        "",
        "((a . 5) 5 #f)",
    ),
    (
        "(let ((t (make-hash-table))) (list (hashq-create-handle! t 'a 1) (hashq-create-handle! t 'a 2) (hashq-ref t 'a)))",
        "",
        "((a . 1) (a . 1) 1)",
    ),
    (
        "(let ((t (make-hash-table))) (hash-set! t 'a 1) (hash-set! t 'b 2) (hash-set! t 'a 3) (sort (hash-map->list cons t) (lambda (x y) (string<? (symbol->string (car x)) (symbol->string (car y))))))",
        "",
        "((a . 3) (b . 2))",
    ),
    (
        "(let ((t (make-hash-table))) (hash-set! t 'a 1) (hash-set! t 'b 2) (list (hash-fold (lambda (k v acc) (+ v acc)) 0 t) (hash-count (lambda (k v) (> v 1)) t)))",
        "",
        "(3 1)",
    ),
    (
        "(let ((t (make-hash-table))) (hashq-set! t 'a 1) (hash-clear! t) (list (hashq-ref t 'a) (hash-count (const #t) t)))",
        "",
        "(#f 0)",
    ),
    (
        "(let ((t (make-hash-table)) (n 0)) (hashq-set! t 'a 1) (hashq-set! t 'b 2) (hash-for-each (lambda (k v) (set! n (+ n v))) t) n)",
        "",
        "3",
    ),
    (
        "(let ((t (make-hash-table))) (hashq-set! t 'a 1) (hash-fold acons '() t))",
        "",
        "((a . 1))",
    ),
    (
        "(let ((t (make-weak-key-hash-table))) (hashq-set! t 'a 1) (list (hashq-ref t 'a) (hash-fold acons '() t) (hash-table? t)))",
        "",
        "(1 ((a . 1)) #t)",
    ),
    (
        "(catch #t (lambda () (hashq-get-handle (make-weak-key-hash-table) 'a)) (lambda (key subr . rest) (list key subr)))",
        "",
        r#"(wrong-type-arg "scm_hash_fn_get_handle")"#,
    ),
    (
        r#"(let ((t (make-hash-table))) (hashx-set! hash assoc t "k" 1) (hashx-set! hash assoc t "k" 2) (list (hashx-ref hash assoc t "k") (hashx-ref hash assoc t "z" 'd) (hashx-get-handle hash assoc t "k")))"#,
        "",
        r#"(2 d ("k" . 2))"#,
    ),
    (
        r#"(let ((t (make-hash-table))) (hashx-set! hash assoc t "k" 1) (hashx-remove! hash assoc t "k") (hashx-ref hash assoc t "k"))"#,
        "",
        "#f",
    ),
    (
        r#"(list (< (hash "abc" 100) 100) (= (hash "abc" 100) (hash (string #\a #\b #\c) 100)) (< (hashq 'a 7) 7))"#,
        "",
        "(#t #t #t)",
    ),
    (
        "(hash-ref 5 1)",
        "wrong-type-arg",
        "In procedure scm_hash_fn_get_handle: Wrong type argument in position 1 (expecting hash-table): 5",
    ),
    (
        "(hash-set! 5 1 2)",
        "wrong-type-arg",
        "In procedure scm_hash_fn_create_handle_x: Wrong type argument in position 1 (expecting hash-table): 5",
    ),
    (
        "(hash-remove! 5 1)",
        "wrong-type-arg",
        "In procedure hash_fn_remove_x: Wrong type argument in position 1 (expecting hash-table): 5",
    ),
    (
        "(hash-fold acons '() 5)",
        "wrong-type-arg",
        "In procedure hash-fold: Wrong type argument in position 3 (expecting hash-table): 5",
    ),
    (
        "(hash-for-each cons 5)",
        "wrong-type-arg",
        "In procedure hash-for-each: Wrong type argument in position 2 (expecting hash-table): 5",
    ),
    (
        "(make-hash-table -1)",
        "out-of-range",
        "Value out of range 0 to< 18446744073709551615: -1",
    ),
    // Properties and setters.
    (
        "(let ((p (make-object-property))) (set! (p 'x) 5) (list (p 'x) (p 'y)))",
        "",
        "(5 #f)",
    ),
    (
        "(let ((p (make-object-property)) (o (list 1))) ((setter p) o 'v) (p o))",
        "",
        "v",
    ),
    (
        "(begin (set-object-property! 'a 'b 3) (list (object-property 'a 'b) (object-property 'a 'c) (object-properties 'a)))",
        "",
        "(3 #f ((b . 3)))",
    ),
    ("(set-object-property! 'q 'c 4)", "", "4"),
    (
        "(let ((f (make-procedure-with-setter car set-car!)) (l (list 1 2))) (set! (f l) 9) (list (f l) l (procedure? f) (procedure-with-setter? f) (procedure-with-setter? car)))",
        "",
        "(9 (9 2) #t #t #f)",
    ),
    (
        "(procedure-name (make-procedure-with-setter car set-car!))",
        "",
        "car",
    ),
    (
        "(setter car)",
        "wrong-type-arg",
        "In procedure setter: Wrong type argument in position 1: #<procedure car (_)>",
    ),
    (
        "(make-procedure-with-setter 1 2)",
        "wrong-type-arg",
        "In procedure make-procedure-with-setter: Wrong type argument in position 1: 1",
    ),
    (
        "(list (procedure-name car) (procedure-name (lambda (x) x)) (let ((f (lambda (x) x))) (procedure-name f)))",
        "",
        "(car #f f)",
    ),
    (
        "(define (f x) x) (list (procedure-name f) (procedure-property f 'name) (procedure-properties car))",
        "",
        "(f f ((name . car)))",
    ),
    (
        "(procedure-name 5)",
        "wrong-type-arg",
        "In procedure procedure-name: Wrong type argument in position 1: 5",
    ),
    (
        "(define (f x) x) (set-procedure-property! f 'color 'red) (list (procedure-property f 'color) (procedure-property f 'size))",
        "",
        "(red #f)",
    ),
    (
        "(define (f x) x) (set-procedure-property! f 'name 'g) (procedure-name f)",
        "",
        "g",
    ),
    ("(module-defined? (current-module) 'car)", "", "#t"),
    (
        "(begin (define zz 1) (list (module-defined? (current-module) 'zz) (module-defined? (current-module) 'zzz)))",
        "",
        "(#t #f)",
    ),
    // Simple-format and format.
    (
        r#"(simple-format #f "~a ~s ~A ~S~%~~" "x" "y" 'z "w")"#,
        "",
        r#""x \"y\" z \"w\"\n~""#,
    ),
    (r#"(format #f "~a-~s" 1 "x")"#, "", r#""1-\"x\"""#),
    (
        r#"(simple-format #f "~a" 1 2)"#,
        "misc-error",
        "In procedure simple-format: FORMAT: 1 superfluous arguments",
    ),
    (
        r#"(simple-format #f "~a ~a" 1)"#,
        "misc-error",
        "In procedure simple-format: FORMAT: Missing argument for ~a",
    ),
    (
        r#"(simple-format #f "~d" 1)"#,
        "misc-error",
        "In procedure simple-format: FORMAT: Unsupported format option ~d - use (ice-9 format) instead",
    ),
    (
        r#"(simple-format 5 "x")"#,
        "wrong-type-arg",
        "In procedure simple-format: Wrong type argument in position 1: 5",
    ),
    (
        "(simple-format #f 5)",
        "wrong-type-arg",
        "In procedure simple-format: Wrong type argument in position 2 (expecting string): 5",
    ),
    (
        r#"(with-output-to-string (lambda () (simple-format #t "~a" 1)))"#,
        "",
        r#""1""#,
    ),
    (
        r#"(call-with-output-string (lambda (p) (simple-format p "~a-~s" 1 "x")))"#,
        "",
        r#""1-\"x\"""#,
    ),
    (
        r#"(use-modules (ice-9 format)) (list (format #f "~,2f" 1.005) (format #f "~,2f" 2.675) (format #f "~,2f" 1/3) (format #f "~,2f" 2) (format #f "~,1f" -0.05))"#,
        "",
        r#"("1.01" "2.68" "0.33" "2.00" "-0.1")"#,
    ),
    (
        r#"(use-modules (ice-9 format)) (list (format #f "~,0f" 2.5) (format #f "~4f" 1.5) (format #f "~4f" 123.456) (format #f "~6,2f" 3.14159) (format #f "~f" 3.5) (format #f "~8,3f|" -1.5))"#,
        "",
        r#"("3." " 1.5" "123." "  3.14" "3.5" "  -1.500|")"#,
    ),
    (
        r#"(use-modules (ice-9 format)) (list (format #f "~,2f" 1e21) (format #f "~,3f" 1e-7) (format #f "~,1f" 0.25) (format #f "~,2f" 100) (format #f "~f" 3.0))"#,
        "",
        r#"("1000000000000000000000.00" "0.000" "0.3" "100.00" "3.0")"#,
    ),
    (
        r#"(use-modules (ice-9 format)) (list (format #f "~@r ~@r ~@r" 4 1994 3999) (format #f "~:@r" 4) (format #f "~x ~o ~b ~d" 255 8 5 -10) (format #f "~16r" 255))"#,
        "",
        r#"("IV MCMXCIV MMMCMXCIX" "IIII" "ff 10 101 -10" "ff")"#,
    ),
    // Case conversion, piece by piece as Guile 3.0's (MS3 step 64).
    (
        r#"(use-modules (ice-9 format)) (list (format #f "~(~@r~)" 14) (format #f "~:(hello wORLD~)") (format #f "~@(*hello you~)") (format #f "~:@(ab~a~)" 'c) (format #f "~(A~{~a~}B~) z" '("X" "Y")) (format #f "~:(~a~)" "hello wORLD") (format #f "~@(~a~)" "hello wORLD"))"#,
        "",
        r#"("xiv" "HELLO WORLD" "*HELLO YOU" "ABC" "axyb z" "Hello World" "Hello world")"#,
    ),
    (
        r#"(use-modules (ice-9 format)) (format #f "a~)")"#,
        "misc-error",
        "error in format",
    ),
    // Promises, Guile 3's (MS3 step 64): forced once, a forcing inside
    // its own keeping the first value.
    (
        r#"(define n 0) (define p (delay (begin (set! n (+ n 1)) n))) (define r #f) (define q (delay (if r 1 (begin (set! r #t) (force q) 2)))) (list (promise? p) (promise? 1) (force p) (force p) n (force (make-promise (lambda () 5))) (force q))"#,
        "",
        "(#t #f 1 1 1 5 1)",
    ),
    (
        "(force 5)",
        "wrong-type-arg",
        "In procedure force: Wrong type argument in position 1: 5",
    ),
    (
        "(make-promise 5)",
        "wrong-type-arg",
        "In procedure make-promise: Wrong type argument in position 1: 5",
    ),
    (
        r#"(use-modules (ice-9 format)) (list (format #f "~5d|~5,'0d|~@d" 42 42 42) (format #f "~5a|~5@a|~5s" 'ab 'ab "ab") (format #f "~c" #\x))"#,
        "",
        r#"("   42|00042|+42" "ab   |   ab|\"ab\" " "x")"#,
    ),
    (
        r#"(use-modules (ice-9 format)) (list (format #f "~{~a~^ ~}" '(1 2 3)) (format #f "~{~a ~}" '()) (format #f "~:{~a=~a ~}" '((a 1) (b 2))) (format #f "~@{~a~^,~}" 1 2 3))"#,
        "",
        r#"("1 2 3" "" "a=1 b=2 " "1,2,3")"#,
    ),
    (
        r#"(use-modules (ice-9 format)) (list (format #f "~:{~:[-~;~]~a~^ ~}" '((#t a) (#f b))) (format #f "< ~{~a ~}>~a~:{~:[-~;~]~a~^ ~}" '(c e) 4 '((#t "x"))))"#,
        "",
        r#"("a-b" "< c e >4x")"#,
    ),
    (
        r#"(use-modules (ice-9 format)) (list (format #f "a~%b~3%c") (format #f "x~v_y" 3) (format #f "~v%" 2) (format #f "~%~v_" 2) (format #f "~~ ~5~"))"#,
        "",
        r#"("a\nb\n\n\nc" "x   y" "\n\n" "\n  " "~ ~~~~~")"#,
    ),
    (
        r#"(use-modules (ice-9 format)) (list (format #f "~[zero~;one~;two~]" 1) (format #f "~:[no~;yes~]" #f) (format #f "~:[no~;yes~]" 'x) (format #f "~a ~* ~a" 1 2 3))"#,
        "",
        r#"("one" "no" "yes" "1  3")"#,
    ),
    (
        "(use-modules (ice-9 format)) (list (format #f \"a~\n     b\") (format #f \"~a\" 1 2) (format #f \"~a~^~a\" 1))",
        "",
        r#"("ab" "1" "1")"#,
    ),
    (
        r#"(use-modules (ice-9 format)) (format #f "~a")"#,
        "misc-error",
        "error in format",
    ),
    (
        r#"(use-modules (ice-9 format)) (format #f "~d" 1.5)"#,
        "misc-error",
        "error in format",
    ),
    (
        r#"(use-modules (ice-9 format)) (format #f "~{~a~}" 5)"#,
        "misc-error",
        "error in format",
    ),
    (
        r#"(use-modules ((ice-9 format) #:select ((format . ice9-format)))) (list (ice9-format #f "~@r" 12) (format #f "~a" 1))"#,
        "",
        r#"("XII" "1")"#,
    ),
    (
        r#"(use-modules (ice-9 format)) (with-output-to-string (lambda () (format #t "x~a" 1)))"#,
        "",
        r#""x1""#,
    ),
    // Ports.
    (
        r#"(let ((p (open-input-string "ab"))) (list (peek-char p) (read-char p) (read-char p) (eof-object? (read-char p)) (char-ready? p)))"#,
        "",
        r#"(#\a #\a #\b #t #t)"#,
    ),
    (
        r#"(let ((p (open-input-string "(a b) 1 \"s\""))) (list (read p) (read p) (read p) (eof-object? (read p))))"#,
        "",
        r#"((a b) 1 "s" #t)"#,
    ),
    (
        r#"(let ((p (open-output-string))) (write "x" p) (display 1 p) (newline p) (write-char #\z p) (get-output-string p))"#,
        "",
        r#""\"x\"1\nz""#,
    ),
    (
        "(read-char 5)",
        "wrong-type-arg",
        "In procedure read-char: Wrong type argument in position 1 (expecting open input port): 5",
    ),
    (
        r#"(catch #t (lambda () (get-output-string (open-input-string ""))) (lambda (key subr . rest) (list key subr)))"#,
        "",
        r#"(wrong-type-arg "get-output-string")"#,
    ),
    (
        "(let ((p (open-output-string))) (close-port p) (catch #t (lambda () (get-output-string p)) (lambda (key subr . rest) (list key subr))))",
        "",
        r#"(wrong-type-arg "get-output-string")"#,
    ),
    (
        r#"(let ((p (open-input-string "x"))) (close-input-port p) (catch #t (lambda () (read-char p)) (lambda (key subr . rest) (list key subr))))"#,
        "",
        r#"(wrong-type-arg "read-char")"#,
    ),
    (
        r#"(list (port? (open-input-string "")) (input-port? (open-input-string "")) (output-port? (current-output-port)) (input-port? (current-output-port)) (port? 5))"#,
        "",
        "(#t #t #t #f #f)",
    ),
    (
        r#"(let ((p (open-input-string ""))) (list (port-closed? p) (close-port p) (port-closed? p) (close-port p)))"#,
        "",
        "(#f #t #t #f)",
    ),
    (
        r#"(list (with-output-to-string (lambda () (write 'a) (display "b") (newline))) (with-input-from-string "x y" read))"#,
        "",
        r#"("ab\n" x)"#,
    ),
    (
        r#"(list (call-with-input-string "1 2" read) (call-with-output-string (lambda (p) (display "q" p))))"#,
        "",
        r#"(1 "q")"#,
    ),
    (
        "(with-error-to-string (lambda () (display 1 (current-error-port))))",
        "",
        r#""1""#,
    ),
    (
        r#"(list (call-with-port (open-input-string "z") read-char) (force-output))"#,
        "",
        r#"(#\z #<unspecified>)"#,
    ),
    (
        "(write 1 5)",
        "wrong-type-arg",
        "In procedure write: Wrong type argument in position 2: 5",
    ),
    (
        r#"(catch #t (lambda () (display 1 (open-input-string ""))) (lambda (key subr . rest) (list key subr)))"#,
        "",
        r#"(wrong-type-arg "display")"#,
    ),
    (
        r#"(let ((p (open-output-string))) (with-output-to-port p (lambda () (display "in"))) (get-output-string p))"#,
        "",
        r#""in""#,
    ),
    (
        r#"(let ((p (open-output-string))) (parameterize ((current-output-port p)) (display "pz")) (get-output-string p))"#,
        "",
        r#""pz""#,
    ),
    (
        r#"(read (open-input-string "(a"))"#,
        "read-error",
        "#<unknown port>:1:3: unexpected end of input while searching for: )",
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
fn primitives_give_what_guile_gives() {
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

/// Runs `source` in a fresh guile, its forms read and evaluated in turn;
/// `None` without guile. `(ice-9 format)` prints its own report of an
/// error before the key, which is left out.
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
    let program = format!("{runner}(run {})\n", literal(source));
    let out = Command::new("guile")
        .args(["--no-auto-compile", "-c", &program])
        .output()
        .ok()?;
    let stdout = String::from_utf8(out.stdout).unwrap();
    let (key, text) = stdout.split_once('\t').unwrap_or((&stdout, ""));
    let key = key.rsplit('\n').next().unwrap_or(key);
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

fn eval(vm: &mut Vm, source: &str) -> String {
    let (key, text) = run(vm, source);
    assert_eq!(key, "", "{source}: {text}");
    text
}

/// A weak-key table forgets a key nothing else holds once the collector
/// has run; a strong table, and a weak one whose key is still held, keep
/// their entries. Object properties are weak-key tables.
#[test]
fn weak_tables_forget_dead_keys() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        "(define weak (make-weak-key-hash-table))
         (define strong (make-hash-table))
         (define kept (list 'kept))
         (define prop (make-object-property))
         (let loop ((i 0))
           (when (< i 100)
             (let ((key (list i)))
               (hashq-set! weak key i)
               (hashq-set! strong (list i) i)
               (set! (prop key) i))
             (loop (+ i 1))))
         (hashq-set! weak kept 'yes)
         (set! (prop kept) 'yes)",
    );
    let count = "(list (hash-count (const #t) weak) (hash-count (const #t) strong))";
    assert_eq!(eval(&mut vm, count), "(101 100)");
    vm.collect();
    assert_eq!(eval(&mut vm, count), "(1 100)");
    assert_eq!(
        eval(&mut vm, "(list (hashq-ref weak kept) (prop kept))"),
        "(yes yes)"
    );
    // The strong table's keys, and their values, survive the collection.
    assert_eq!(
        eval(
            &mut vm,
            "(hash-fold (lambda (k v acc) (+ acc (car k) v)) 0 strong)"
        ),
        "9900"
    );
}

/// Entries survive collections, and removing most of them compacts the
/// table without losing the rest.
#[test]
fn tables_survive_collections() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        "(define t (make-hash-table))
         (let loop ((i 0))
           (when (< i 2000)
             (hash-set! t (number->string i) (list i))
             (loop (+ i 1))))",
    );
    vm.collect();
    eval(
        &mut vm,
        "(let loop ((i 0))
           (when (< i 1990)
             (hash-remove! t (number->string i))
             (loop (+ i 1))))",
    );
    vm.collect();
    assert_eq!(
        eval(
            &mut vm,
            "(list (hash-count (const #t) t) (hash-ref t \"1995\") (hash-ref t \"5\"))"
        ),
        "(10 (1995) #f)"
    );
}

/// The standard output and error ports are what the host takes.
#[test]
fn the_host_takes_output_and_errors() {
    let mut vm = Vm::new();
    eval(
        &mut vm,
        "(display \"out\")
         (simple-format #t \" ~a\" 1)
         (display \"err\" (current-error-port))
         (simple-format (current-error-port) \"~s\" \"q\")
         (force-output (current-error-port))",
    );
    assert_eq!(vm.take_output(), "out 1");
    assert_eq!(vm.take_errors(), "err\"q\"");
    assert_eq!(vm.take_errors(), "");
}
