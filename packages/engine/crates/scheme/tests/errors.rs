//! S2's escapes and errors (chapter 03 §5.3, R7, R9, R14): every primitive's
//! errors, the VM's, psyntax's and the throws' are worded as Guile 3.0.11
//! prints them; `catch`, `throw`, `dynamic-wind`, escapes, fluids and
//! parameters give what guile gives; error cases 22 and 23 of chapter 02's
//! corpus print their `.err`'s Scheme line; `*location*`, `eval` and the
//! cancel check work. With `guile` installed, the table is also run by guile
//! and compared.
use std::cell::Cell;
use std::process::Command;
use std::rc::Rc;

use lily_engine_scheme::{CANCEL_INTERVAL, Position, Value, Vm, read};

/// Programs, and what Guile 3.0.11 gives for the last form: the error's key
/// and `print-exception`'s text, or `""` and the value written. Each runs in
/// a fresh VM, and in guile in a fresh module with `(ice-9 control)`.
const CASES: &[(&str, &str, &str)] = &[
    // pairs and lists
    (
        "(car '())",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): ()",
    ),
    (
        "(cdr 5)",
        "wrong-type-arg",
        "In procedure cdr: Wrong type (expecting pair): 5",
    ),
    (
        "((lambda (q) (car 'a)) 0)",
        "wrong-type-arg",
        "In procedure car: Wrong type argument in position 1 (expecting pair): a",
    ),
    (
        "((lambda (q) (cdr 'a)) 0)",
        "wrong-type-arg",
        "In procedure cdr: Wrong type argument in position 1 (expecting pair): a",
    ),
    (
        "(define (f x) (car x)) (f 1)",
        "wrong-type-arg",
        "In procedure car: Wrong type argument in position 1 (expecting pair): 1",
    ),
    (
        "(let () (car 1))",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): 1",
    ),
    (
        "(let ((y 1)) (car 1))",
        "wrong-type-arg",
        "In procedure car: Wrong type argument in position 1 (expecting pair): 1",
    ),
    (
        "(let ((y (car 1))) y)",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): 1",
    ),
    (
        "(let* ((y (car 1))) y)",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): 1",
    ),
    (
        "(let loop ((i (car 1))) i)",
        "wrong-type-arg",
        "In procedure car: Wrong type argument in position 1 (expecting pair): 1",
    ),
    (
        "(case (car 1) ((1) 2))",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): 1",
    ),
    (
        "(case 1 ((1) (car 1)))",
        "wrong-type-arg",
        "In procedure car: Wrong type argument in position 1 (expecting pair): 1",
    ),
    (
        "(cond ((car 1) 1))",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): 1",
    ),
    (
        "(cond (#f => (lambda (x) x)) (else (car 1)))",
        "wrong-type-arg",
        "In procedure car: Wrong type argument in position 1 (expecting pair): 1",
    ),
    (
        "(define x (car 1))",
        "wrong-type-arg",
        "In procedure car: Wrong type argument in position 1 (expecting pair): 1",
    ),
    (
        "(begin (define x 1) (car 1))",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): 1",
    ),
    (
        "(and 1 (when 1 (list (car 1))))",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): 1",
    ),
    (
        "(map car '(1))",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): 1",
    ),
    (
        "(cadr '(1))",
        "wrong-type-arg",
        "In procedure cadr: Wrong type (expecting pair): ()",
    ),
    (
        "(caadr '(1 2))",
        "wrong-type-arg",
        "In procedure caadr: Wrong type (expecting pair): 2",
    ),
    (
        "(define (f x) (cddr x)) (f '(1))",
        "wrong-type-arg",
        "In procedure cddr: Wrong type (expecting pair): ()",
    ),
    (
        "(set-car! 1 2)",
        "wrong-type-arg",
        "In procedure set-car!: Wrong type argument in position 1 (expecting mutable pair): 1",
    ),
    (
        "(set-cdr! '() 2)",
        "wrong-type-arg",
        "In procedure set-cdr!: Wrong type argument in position 1 (expecting mutable pair): ()",
    ),
    (
        "(length 5)",
        "wrong-type-arg",
        "In procedure length: Wrong type argument in position 1: 5",
    ),
    (
        "(length '(1 2 . 3))",
        "wrong-type-arg",
        "In procedure length: Wrong type argument in position 1: (1 2 . 3)",
    ),
    (
        "(append '(1 . 2) '(3))",
        "wrong-type-arg",
        "In procedure append: Wrong type argument in position 1 (expecting empty list): 2",
    ),
    (
        "(append '(1) '(2 . 3) '())",
        "wrong-type-arg",
        "In procedure append: Wrong type argument in position 2 (expecting empty list): 3",
    ),
    (
        "(reverse '(1 . 2))",
        "wrong-type-arg",
        "In procedure reverse: Wrong type argument in position 1: (1 . 2)",
    ),
    (
        "(list-tail '(1) 3)",
        "wrong-type-arg",
        "In procedure list-tail: Wrong type argument in position 1 (expecting pair): ()",
    ),
    (
        "(list-tail '(1) -1)",
        "out-of-range",
        "Value out of range 0 to< 18446744073709551615: -1",
    ),
    (
        "(list-tail '(1) 'a)",
        "wrong-type-arg",
        "Wrong type (expecting exact integer): a",
    ),
    (
        "(list-ref '(1) 3)",
        "out-of-range",
        "In procedure list-ref: Argument 2 out of range: 3",
    ),
    (
        "(list-ref '(1) -1)",
        "out-of-range",
        "Value out of range 0 to< 18446744073709551615: -1",
    ),
    (
        "(list-copy 5)",
        "wrong-type-arg",
        "In procedure list-copy: Wrong type argument in position 1: 5",
    ),
    (
        "(last-pair 5)",
        "wrong-type-arg",
        "In procedure last-pair: Wrong type argument in position 1 (expecting pair): 5",
    ),
    ("(last-pair '())", "", "()"),
    (
        "(memq 1 5)",
        "wrong-type-arg",
        "In procedure memq: Wrong type argument in position 2 (expecting list): 5",
    ),
    (
        "(memv 1 '(2 . 3))",
        "wrong-type-arg",
        "In procedure memv: Wrong type argument in position 2 (expecting list): (2 . 3)",
    ),
    (
        "(member 1 '(2 . 3))",
        "wrong-type-arg",
        "In procedure member: Wrong type argument in position 2: (2 . 3)",
    ),
    (
        "(assq 'a 5)",
        "wrong-type-arg",
        "In procedure assq: Wrong type argument in position 2 (expecting association list): 5",
    ),
    (
        "(assv 1 '(2))",
        "wrong-type-arg",
        "In procedure assq: Wrong type argument in position 2 (expecting association list): (2)",
    ),
    (
        "(assoc 1 '((2 . 2) . 3))",
        "wrong-type-arg",
        "In procedure assq: Wrong type argument in position 2 (expecting association list): ((2 . 2) . 3)",
    ),
    // symbols, characters and strings
    (
        "(symbol->string 1)",
        "wrong-type-arg",
        "In procedure symbol->string: Wrong type argument in position 1 (expecting symbol): 1",
    ),
    (
        "(string->symbol 1)",
        "wrong-type-arg",
        "In procedure string->symbol: Wrong type argument in position 1 (expecting string): 1",
    ),
    (
        "(char->integer 1)",
        "wrong-type-arg",
        "In procedure char->integer: Wrong type argument in position 1 (expecting character): 1",
    ),
    (
        "(integer->char -1)",
        "out-of-range",
        "In procedure integer->char: Value out of range: -1",
    ),
    (
        "(integer->char 'a)",
        "wrong-type-arg",
        "Wrong type (expecting exact integer): a",
    ),
    (
        "(integer->char 55296)",
        "out-of-range",
        "In procedure integer->char: Value out of range: 55296",
    ),
    (
        "(integer->char 1114112)",
        "out-of-range",
        "Value out of range -1 to< 1114111: 1114112",
    ),
    (
        "(char-upcase 1)",
        "wrong-type-arg",
        "In procedure char-upcase: Wrong type argument in position 1 (expecting character): 1",
    ),
    (
        "(char=? #\\a 1)",
        "wrong-type-arg",
        "In procedure char=?: Wrong type argument in position 2 (expecting character): 1",
    ),
    (
        "(char-alphabetic? 1)",
        "wrong-type-arg",
        "In procedure char-set-contains?: Wrong type argument in position 2 (expecting character): 1",
    ),
    (
        "(char-whitespace? 1)",
        "wrong-type-arg",
        "In procedure char-set-contains?: Wrong type argument in position 2 (expecting character): 1",
    ),
    (
        "(string-length 'a)",
        "wrong-type-arg",
        "In procedure string-length: Wrong type argument in position 1 (expecting string): a",
    ),
    (
        "(string-ref \"abc\" 5)",
        "out-of-range",
        "Value out of range 0 to< 2: 5",
    ),
    (
        "(string-ref \"abc\" -1)",
        "out-of-range",
        "Value out of range 0 to< 2: -1",
    ),
    (
        "(string-ref \"\" 0)",
        "out-of-range",
        "Value out of range: 0",
    ),
    (
        "(string-ref 1 0)",
        "wrong-type-arg",
        "In procedure string-ref: Wrong type argument in position 1 (expecting string): 1",
    ),
    (
        "(string-ref \"abc\" 'a)",
        "wrong-type-arg",
        "Wrong type (expecting exact integer): a",
    ),
    (
        "(string-set! (string #\\a) -1 #\\a)",
        "out-of-range",
        "Value out of range 0 to< 0: -1",
    ),
    (
        "(string-set! (string #\\a) 0 1)",
        "wrong-type-arg",
        "In procedure string-set!: Wrong type argument in position 3 (expecting character): 1",
    ),
    (
        "(substring \"abc\" 2 1)",
        "out-of-range",
        "Value out of range 2 to< 3: 1",
    ),
    (
        "(substring \"abc\" 0 5)",
        "out-of-range",
        "Value out of range 0 to< 3: 5",
    ),
    (
        "(substring \"abc\" 5)",
        "out-of-range",
        "Value out of range 0 to< 3: 5",
    ),
    (
        "(substring 1 0)",
        "wrong-type-arg",
        "In procedure substring: Wrong type argument in position 1 (expecting string): 1",
    ),
    (
        "(string-append \"a\" 1)",
        "wrong-type-arg",
        "In procedure string-append: Wrong type (expecting string): 1",
    ),
    (
        "(string=? \"a\" 1)",
        "wrong-type-arg",
        "In procedure string=: Wrong type argument in position 2 (expecting string): 1",
    ),
    (
        "(string<? 1 \"a\")",
        "wrong-type-arg",
        "In procedure string<: Wrong type argument in position 1 (expecting string): 1",
    ),
    (
        "(string->list 1)",
        "wrong-type-arg",
        "In procedure string->list: Wrong type argument in position 1 (expecting string): 1",
    ),
    (
        "(list->string '(1))",
        "wrong-type-arg",
        "In procedure string: Wrong type (expecting character): 1",
    ),
    (
        "(list->string 5)",
        "wrong-type-arg",
        "In procedure string: Wrong type argument in position 1: 5",
    ),
    (
        "(string #\\a 1)",
        "wrong-type-arg",
        "In procedure string: Wrong type (expecting character): 1",
    ),
    (
        "(make-string -1)",
        "out-of-range",
        "Value out of range 0 to< 18446744073709551615: -1",
    ),
    (
        "(make-string 2 1)",
        "wrong-type-arg",
        "Wrong type (expecting character): 1",
    ),
    (
        "(string-copy 1)",
        "wrong-type-arg",
        "In procedure string-copy: Wrong type argument in position 1 (expecting string): 1",
    ),
    // vectors
    (
        "(make-vector -1)",
        "out-of-range",
        "Value out of range 0 to< 72057594037927935: -1",
    ),
    (
        "(make-vector 'a)",
        "wrong-type-arg",
        "Wrong type (expecting exact integer): a",
    ),
    (
        "(vector-length 'a)",
        "wrong-type-arg",
        "In procedure vector-length: Wrong type argument in position 1: a",
    ),
    (
        "((lambda (q) (vector-length 'a)) 0)",
        "wrong-type-arg",
        "In procedure vector-length: Wrong type argument in position 1 (expecting vector): a",
    ),
    (
        "(vector-ref 'a 0)",
        "wrong-type-arg",
        "In procedure vector-ref: Wrong type argument in position 1: a",
    ),
    (
        "(vector-ref (vector 1) 5)",
        "out-of-range",
        "Value out of range: 5",
    ),
    (
        "(vector-ref (vector 1) -1)",
        "out-of-range",
        "Value out of range 0 to< 18446744073709551615: -1",
    ),
    (
        "(vector-ref (vector 1) 'x)",
        "wrong-type-arg",
        "Wrong type (expecting exact integer): x",
    ),
    (
        "(vector-ref (vector 1) 4611686018427387904)",
        "out-of-range",
        "Value out of range: 4611686018427387904",
    ),
    (
        "((lambda (q) (vector-ref 'a 0)) 0)",
        "wrong-type-arg",
        "In procedure vector-ref: Wrong type argument in position 1 (expecting vector): a",
    ),
    (
        "((lambda (q) (vector-ref (vector 1) 5)) 0)",
        "out-of-range",
        "In procedure vector-ref: Argument 2 out of range: 5",
    ),
    (
        "((lambda (q) (vector-ref (vector 1) -1)) 0)",
        "out-of-range",
        "In procedure vector-ref: Argument 2 out of range: -1",
    ),
    (
        "((lambda (q) (vector-ref (vector 1) 1.0)) 0)",
        "wrong-type-arg",
        "In procedure vector-ref: Wrong type argument in position 2 (expecting small integer): 1.0",
    ),
    (
        "(vector-set! 'a 0 0)",
        "wrong-type-arg",
        "In procedure vector-set!: Wrong type argument in position 1 (expecting mutable vector): a",
    ),
    (
        "(vector-set! (vector 1) 5 0)",
        "out-of-range",
        "Value out of range: 5",
    ),
    (
        "((lambda (q) (vector-set! (vector 1) 'x 0)) 0)",
        "wrong-type-arg",
        "In procedure vector-set!: Wrong type argument in position 2 (expecting small integer): x",
    ),
    (
        "(vector->list 1)",
        "wrong-type-arg",
        "Wrong type (expecting array): 1",
    ),
    (
        "(list->vector '(1 . 2))",
        "wrong-type-arg",
        "In procedure vector: Wrong type argument in position 1: (1 . 2)",
    ),
    (
        "(vector-fill! 1 0)",
        "wrong-type-arg",
        "In procedure vector-fill!: Wrong type argument in position 1 (expecting mutable vector): 1",
    ),
    (
        "(write-char 1)",
        "wrong-type-arg",
        "In procedure write-char: Wrong type argument in position 1 (expecting character): 1",
    ),
    // numbers
    (
        "(+ 1 'a)",
        "wrong-type-arg",
        "In procedure +: Wrong type argument in position 2: a",
    ),
    (
        "(+ 'a 1)",
        "wrong-type-arg",
        "In procedure +: Wrong type argument in position 1: a",
    ),
    (
        "(+ 1 2 'a)",
        "wrong-type-arg",
        "In procedure +: Wrong type argument in position 2: a",
    ),
    (
        "((lambda (q) (+ 1 2 'a)) 0)",
        "wrong-type-arg",
        "In procedure +: Wrong type argument in position 2: a",
    ),
    (
        "(+ 'a)",
        "wrong-type-arg",
        "In procedure +: Wrong type argument in position 1: a",
    ),
    (
        "(- 'a)",
        "wrong-type-arg",
        "In procedure -: Wrong type argument in position 1: a",
    ),
    (
        "(- 1 'a)",
        "wrong-type-arg",
        "In procedure -: Wrong type argument in position 2: a",
    ),
    (
        "(* 'a)",
        "wrong-type-arg",
        "In procedure *: Wrong type argument in position 1: a",
    ),
    (
        "(/ 1 'a)",
        "wrong-type-arg",
        "In procedure /: Wrong type argument in position 2: a",
    ),
    (
        "(/ 1 0)",
        "numerical-overflow",
        "Throw to key `numerical-overflow' with args `(\"divide\" \"Numerical overflow\" #f #f)'.",
    ),
    (
        "(/ 1.0 0)",
        "numerical-overflow",
        "Throw to key `numerical-overflow' with args `(\"divide\" \"Numerical overflow\" #f #f)'.",
    ),
    (
        "(quotient 1 0)",
        "numerical-overflow",
        "Throw to key `numerical-overflow' with args `(\"truncate-quotient\" \"Numerical overflow\" #f #f)'.",
    ),
    (
        "(remainder 1 0)",
        "numerical-overflow",
        "Throw to key `numerical-overflow' with args `(\"truncate-remainder\" \"Numerical overflow\" #f #f)'.",
    ),
    (
        "(modulo 1 0)",
        "numerical-overflow",
        "Throw to key `numerical-overflow' with args `(\"floor-remainder\" \"Numerical overflow\" #f #f)'.",
    ),
    (
        "(log 0)",
        "numerical-overflow",
        "Throw to key `numerical-overflow' with args `(\"log\" \"Numerical overflow\" #f #f)'.",
    ),
    (
        "(= 1 'a)",
        "wrong-type-arg",
        "In procedure =: Wrong type argument in position 1: a",
    ),
    (
        "(= 1 'a 2)",
        "wrong-type-arg",
        "In procedure =: Wrong type argument in position 1: a",
    ),
    ("(= 1 2 'a)", "", "#f"),
    (
        "(< 'a 1)",
        "wrong-type-arg",
        "In procedure <: Wrong type argument in position 1: a",
    ),
    (
        "(< 1 'a 2)",
        "wrong-type-arg",
        "In procedure <: Wrong type argument in position 2: a",
    ),
    (
        "(< 1 2 'a)",
        "wrong-type-arg",
        "In procedure <: Wrong type argument in position 2: a",
    ),
    (
        "(>= 1 'a)",
        "wrong-type-arg",
        "In procedure >=: Wrong type argument in position 2: a",
    ),
    ("(list (< 'a) (= 'a) (> 'a))", "", "(#t #t #t)"),
    (
        "(max 'a)",
        "wrong-type-arg",
        "In procedure max: Wrong type argument in position 1: a",
    ),
    (
        "(max 1 2 'a)",
        "wrong-type-arg",
        "In procedure max: Wrong type argument in position 2: a",
    ),
    (
        "(min 'a 1)",
        "wrong-type-arg",
        "In procedure min: Wrong type argument in position 1: a",
    ),
    (
        "(max 1+i)",
        "wrong-type-arg",
        "In procedure max: Wrong type argument in position 1: 1.0+1.0i",
    ),
    (
        "(quotient 1 'a)",
        "wrong-type-arg",
        "In procedure quotient: Wrong type argument in position 2: a",
    ),
    (
        "(modulo 'a 1)",
        "wrong-type-arg",
        "In procedure modulo: Wrong type argument in position 1: a",
    ),
    (
        "(abs 'a)",
        "wrong-type-arg",
        "In procedure abs: Wrong type argument in position 1: a",
    ),
    (
        "(1+ 'a)",
        "wrong-type-arg",
        "In procedure +: Wrong type argument in position 1: a",
    ),
    (
        "(1- 'a)",
        "wrong-type-arg",
        "In procedure -: Wrong type argument in position 1: a",
    ),
    (
        "(gcd 'a)",
        "wrong-type-arg",
        "In procedure abs: Wrong type argument in position 1: a",
    ),
    (
        "(lcm 1.5 2)",
        "wrong-type-arg",
        "In procedure lcm: Wrong type argument in position 1: 1.5",
    ),
    (
        "(expt 'a 2)",
        "wrong-type-arg",
        "In procedure *: Wrong type argument in position 1: a",
    ),
    (
        "(sqrt 'a)",
        "wrong-type-arg",
        "In procedure sqrt: Wrong type argument in position 1: a",
    ),
    (
        "(atan 1 'a)",
        "wrong-type-arg",
        "In procedure atan: Wrong type argument in position 2: a",
    ),
    (
        "(floor 'a)",
        "wrong-type-arg",
        "In procedure floor: Wrong type argument in position 1: a",
    ),
    (
        "(denominator 1.5+i)",
        "wrong-type-arg",
        "In procedure denominator: Wrong type argument in position 1: 1.5+1.0i",
    ),
    (
        "(inexact->exact +inf.0)",
        "out-of-range",
        "In procedure inexact->exact: Argument 1 out of range: +inf.0",
    ),
    (
        "(exact->inexact 'a)",
        "wrong-type-arg",
        "In procedure exact->inexact: Wrong type argument in position 1: a",
    ),
    (
        "(zero? 'a)",
        "wrong-type-arg",
        "In procedure zero?: Wrong type argument in position 1: a",
    ),
    (
        "(even? 1.5)",
        "wrong-type-arg",
        "In procedure even?: Wrong type argument in position 1: 1.5",
    ),
    (
        "(number->string 'a)",
        "wrong-type-arg",
        "In procedure number->string: Wrong type argument in position 1: a",
    ),
    (
        "(number->string 1 1)",
        "out-of-range",
        "Value out of range 2 to< 36: 1",
    ),
    (
        "(number->string 1 'a)",
        "wrong-type-arg",
        "Wrong type (expecting exact integer): a",
    ),
    (
        "(string->number 1)",
        "wrong-type-arg",
        "In procedure string->number: Wrong type argument in position 1 (expecting string): 1",
    ),
    (
        "(string->number \"1\" 1)",
        "out-of-range",
        "Value out of range 2 to< 2147483647: 1",
    ),
    // calls
    (
        "(car)",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure car (_)>",
    ),
    (
        "(cons 1)",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure cons (_ _)>",
    ),
    (
        "(apply car '(1 2))",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure car (_)>",
    ),
    (
        "(apply +)",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure apply (fun args) | (fun arg1 . args)>",
    ),
    (
        "((lambda (x) x))",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure ANON (a)>",
    ),
    (
        "((lambda (x . y) x))",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure ANON (a . rest)>",
    ),
    (
        "(define (f x) x) (f)",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure f (a)>",
    ),
    (
        "(define (g x y . z) x) (g 1)",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure g (a b . rest)>",
    ),
    (
        "(let loop ((i 0)) (loop))",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure loop (a)>",
    ),
    ("(1 2)", "wrong-type-arg", "Wrong type to apply: 1"),
    ("(\"a\")", "wrong-type-arg", "Wrong type to apply: \"a\""),
    ("(apply 1 '())", "wrong-type-arg", "Wrong type to apply: 1"),
    (
        "(define (f x) (x)) (f 1)",
        "wrong-type-arg",
        "Wrong type to apply: 1",
    ),
    (
        "(apply + 1)",
        "wrong-type-arg",
        "In procedure apply: Apply to non-list: 1",
    ),
    (
        "(no-such-thing)",
        "unbound-variable",
        "Unbound variable: no-such-thing",
    ),
    (
        "(define (f) (no-such)) (f)",
        "unbound-variable",
        "Unbound variable: no-such",
    ),
    (
        "(set! nowhere 1)",
        "unbound-variable",
        "Unbound variable: nowhere",
    ),
    (
        "(list (values))",
        "misc-error",
        "Zero values returned to single-valued continuation",
    ),
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
        "(map + '(1) '(1) '(1 2))",
        "wrong-type-arg",
        "In procedure map: List of wrong length: (1 2)",
    ),
    (
        "(for-each + '(1) '(1 2))",
        "wrong-type-arg",
        "In procedure for-each: List of wrong length: (1 2)",
    ),
    (
        "(call-with-values 1 list)",
        "wrong-type-arg",
        "Wrong type to apply: 1",
    ),
    // throws and their printing
    (
        "(throw 'foo 1 2)",
        "foo",
        "Throw to key `foo' with args `(1 2)'.",
    ),
    ("(throw 'foo)", "foo", "Throw to key `foo' with args `()'."),
    (
        "(throw 1)",
        "wrong-type-arg",
        "In procedure throw: Wrong type argument in position 1: 1",
    ),
    (
        "(throw)",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure throw (key . args)>",
    ),
    (
        "(throw 'wrong-type-arg \"car\" \"Wrong type: ~S\" (list 1) #f)",
        "wrong-type-arg",
        "In procedure car: Wrong type: 1",
    ),
    (
        "(throw 'foo \"car\" \"Wrong type: ~S\" (list 1) #f)",
        "foo",
        "Throw to key `foo' with args `(\"car\" \"Wrong type: ~S\" (1) #f)'.",
    ),
    (
        "(throw 'misc-error \"a\" \"b ~S\")",
        "misc-error",
        "Throw to key `misc-error' with args `(\"a\" \"b ~S\")'.",
    ),
    (
        "(throw 'misc-error #f \"~A ~S\" '(1))",
        "misc-error",
        "1Error while printing exception.",
    ),
    (
        "(throw 'misc-error #f \"~Z\" '(1))",
        "misc-error",
        "Error while printing exception.",
    ),
    (
        "(throw 'misc-error 1 2 3 4)",
        "misc-error",
        "In procedure 1: Error while printing exception.",
    ),
    ("(throw 'misc-error #f \"x\" #f #f)", "misc-error", "x"),
    (
        "(throw 'unbound-variable #f \"Unbound variable: ~S\" '(x) #f)",
        "unbound-variable",
        "Unbound variable: x",
    ),
    (
        "(throw 'stack-overflow #f \"Stack overflow\" #f #f)",
        "stack-overflow",
        "Stack overflow",
    ),
    (
        "(throw 'vm-error #f \"x\" '() #f)",
        "vm-error",
        "Throw to key `vm-error' with args `(#f \"x\" () #f)'.",
    ),
    (
        "(throw 'keyword-argument-error \"proc\" \"Invalid keyword\" '() '(#:x))",
        "keyword-argument-error",
        "Invalid keyword: #:x",
    ),
    (
        "(throw 'syntax-error 'who \"what\" #f 'form #f)",
        "syntax-error",
        "Syntax error:\nunknown location: who: what in form form",
    ),
    (
        "(throw 'syntax-error #f \"msg\" '((line . 4) (column . 2) (filename . \"f.scm\")) 'x 'y)",
        "syntax-error",
        "Syntax error:\nf.scm:5:2: msg in subform y of x",
    ),
    (
        "(throw 'syntax-error #f \"msg\" #f #f #f)",
        "syntax-error",
        "Syntax error:\nunknown location: msg",
    ),
    (
        "(throw 'syntax-error 1)",
        "syntax-error",
        "Throw to key `syntax-error' with args `(1)'.",
    ),
    (
        "(scm-error 'misc-error \"me\" \"hi ~a ~s\" '(1 \"x\") #f)",
        "misc-error",
        "In procedure me: hi 1 \"x\"",
    ),
    (
        "(scm-error 'misc-error #f \"hi ~a ~s ~% ~~\" '(1 \"x\") #f)",
        "misc-error",
        "hi 1 \"x\" \n ~",
    ),
    (
        "(scm-error 'my-key \"me\" \"hi\" '() #f)",
        "my-key",
        "Throw to key `my-key' with args `(\"me\" \"hi\" () #f)'.",
    ),
    (
        "(scm-error 'wrong-type-arg #f \"x\" 1 #f)",
        "wrong-type-arg",
        "Error while printing exception.",
    ),
    (
        "(scm-error 1 #f \"x\" '() #f)",
        "wrong-type-arg",
        "In procedure throw: Wrong type argument in position 1: 1",
    ),
    ("(error \"boom\" 1 \"two\")", "misc-error", "boom 1 \"two\""),
    ("(error 'sym \"msg\")", "misc-error", "sym \"msg\""),
    ("(error \"a ~S\" 1)", "misc-error", "a ~S 1"),
    ("(error)", "misc-error", "?"),
    // syntax errors
    (
        "(if)",
        "syntax-error",
        "Syntax error:\nunknown location: source expression failed to match any pattern in form (if)",
    ),
    (
        "(if 1 2 3 4)",
        "syntax-error",
        "Syntax error:\nunknown location: source expression failed to match any pattern in form (if 1 2 3 4)",
    ),
    (
        "(let ((x)) x)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: let: bad let in form (let ((x)) x)",
    ),
    (
        "(let)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: let: bad let in form (let)",
    ),
    (
        "(let ())",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: let: bad let in form (let ())",
    ),
    (
        "(let loop ((i 0)))",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: let: bad let in form (let loop ((i 0)))",
    ),
    (
        "(let ((1 2)) 3)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: let: bad let in form (let ((1 2)) 3)",
    ),
    (
        "(let ((x 1) (x 2)) x)",
        "syntax-error",
        "Syntax error:\nunknown location: let: duplicate bound variable in form (let ((x 1) (x 2)) x)",
    ),
    (
        "(let* ((x)) x)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: source expression failed to match any pattern in form (let* ((x)) x)",
    ),
    (
        "(letrec ((x)) x)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: letrec: bad letrec in form (letrec ((x)) x)",
    ),
    (
        "(letrec* ((x)) x)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: letrec*: bad letrec* in form (letrec* ((x)) x)",
    ),
    (
        "(lambda)",
        "syntax-error",
        "Syntax error:\nunknown location: lambda: bad lambda in form (lambda)",
    ),
    (
        "(lambda (x))",
        "syntax-error",
        "Syntax error:\nunknown location: lambda: bad lambda in form (lambda (x))",
    ),
    (
        "(lambda (1) 1)",
        "syntax-error",
        "Syntax error:\nunknown file:1:8: lambda: invalid argument list in subform (1) of (1)",
    ),
    (
        "(lambda (x x) 1)",
        "syntax-error",
        "Syntax error:\nunknown file:1:8: lambda: duplicate identifier in argument list in form (x x)",
    ),
    (
        "(lambda (x) (define y 1))",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: body should end with an expression in form (lambda (x) (define y 1))",
    ),
    (
        "(define (f) (define y 1))",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: body should end with an expression in form (lambda () (define y 1))",
    ),
    (
        "(let () (define x 1))",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: body should end with an expression in form (let () (define x 1))",
    ),
    (
        "(define)",
        "syntax-error",
        "Syntax error:\nunknown location: source expression failed to match any pattern in form (define)",
    ),
    (
        "(define 1 2)",
        "syntax-error",
        "Syntax error:\nunknown location: source expression failed to match any pattern in form (define 1 2)",
    ),
    (
        "(define (f))",
        "syntax-error",
        "Syntax error:\nunknown location: source expression failed to match any pattern in form (define (f))",
    ),
    (
        "(define (f 1) 1)",
        "syntax-error",
        "Syntax error:\nunknown location: source expression failed to match any pattern in form (define (f 1) 1)",
    ),
    (
        "(begin (define))",
        "syntax-error",
        "Syntax error:\nunknown location: source expression failed to match any pattern in form (define)",
    ),
    (
        "(begin . 1)",
        "syntax-error",
        "Syntax error:\nunknown location: source expression failed to match any pattern in form (begin . 1)",
    ),
    (
        "(quote)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: quote: bad syntax in form (quote)",
    ),
    (
        "(quote 1 2)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: quote: bad syntax in form (quote 1 2)",
    ),
    (
        "(set!)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: set!: bad set! in form (set!)",
    ),
    (
        "(set! 1 2)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: set!: bad set! in form (set! 1 2)",
    ),
    (
        "(cond)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: source expression failed to match any pattern in form (cond)",
    ),
    (
        "(cond 1)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: cond: invalid clause in subform 1 of (cond 1)",
    ),
    (
        "(cond (else))",
        "syntax-error",
        "Syntax error:\nunknown file:1:6: cond: invalid clause in subform (else) of (cond (else))",
    ),
    (
        "(cond (else 1) (#t 2))",
        "syntax-error",
        "Syntax error:\nunknown file:1:6: cond: else must be the last clause in subform (else 1) of (cond (else 1) (#t 2))",
    ),
    (
        "(cond (1 =>))",
        "syntax-error",
        "Syntax error:\nunknown file:1:6: cond: wrong number of receiver expressions in subform (1 =>) of (cond (1 =>))",
    ),
    (
        "(case)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: source expression failed to match any pattern in form (case)",
    ),
    (
        "(case 1 (2 3))",
        "syntax-error",
        "Syntax error:\nunknown file:1:8: case: invalid clause in subform (2 3) of (case 1 (2 3))",
    ),
    (
        "(case 1 (else))",
        "syntax-error",
        "Syntax error:\nunknown file:1:8: case: invalid clause in subform (else) of (case 1 (else))",
    ),
    (
        "(case 1 ((1)))",
        "syntax-error",
        "Syntax error:\nunknown file:1:8: case: invalid clause in subform ((1)) of (case 1 ((1)))",
    ),
    (
        "(when)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: source expression failed to match any pattern in form (when)",
    ),
    (
        "(unless)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: source expression failed to match any pattern in form (unless)",
    ),
    (
        "(and . 1)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: source expression failed to match any pattern in form (and . 1)",
    ),
    (
        "(or . 1)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: source expression failed to match any pattern in form (or . 1)",
    ),
    (
        "(+ 1 (define x 2))",
        "syntax-error",
        "Syntax error:\nunknown file:1:5: definition in expression context, where definitions are not allowed, in form (define x 2)",
    ),
    (
        "()",
        "syntax-error",
        "Syntax error:\nunknown location: unexpected syntax in form ()",
    ),
    (
        "(parameterize)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: source expression failed to match any pattern in form (parameterize)",
    ),
    (
        "(parameterize ((x)) 1)",
        "syntax-error",
        "Syntax error:\nunknown file:1:0: source expression failed to match any pattern in form (parameterize ((x)) 1)",
    ),
    // catch and throw
    (
        "(catch 'a (lambda () (throw 'a 1 2)) (lambda args args))",
        "",
        "(a 1 2)",
    ),
    (
        "(catch #t (lambda () (car 1)) (lambda (k . args) k))",
        "",
        "wrong-type-arg",
    ),
    (
        "(catch #t (lambda () (+ 1 (car 1))) (lambda (k s m a r) (list s r)))",
        "",
        "(\"car\" (1))",
    ),
    (
        "(catch 'b (lambda () (throw 'a 1)) (lambda args args))",
        "a",
        "Throw to key `a' with args `(1)'.",
    ),
    (
        "(catch 'a (lambda () (catch 'b (lambda () (throw 'a 1)) (lambda args 'inner))) (lambda args 'outer))",
        "",
        "outer",
    ),
    (
        "(catch 'a (lambda () (catch 'a (lambda () (throw 'a 1)) (lambda (k x) (throw 'a (+ x 1))))) (lambda (k x) x))",
        "",
        "2",
    ),
    ("(catch 'a (lambda () 5) (lambda args 'no))", "", "5"),
    (
        "(list (catch 'a (lambda () (values 1 2)) (lambda a a)))",
        "",
        "(1)",
    ),
    (
        "(call-with-values (lambda () (catch 'a (lambda () (values 1 2)) (lambda a a))) list)",
        "",
        "(1 2)",
    ),
    (
        "(call-with-values (lambda () (catch 'a (lambda () (throw 'a)) (lambda a (values 3 4)))) list)",
        "",
        "(3 4)",
    ),
    (
        "(define (f n) (if (= n 0) (throw 'done 'deep) (+ 1 (f (- n 1))))) (catch 'done (lambda () (f 10000)) (lambda (k v) v))",
        "",
        "deep",
    ),
    (
        "(let loop ((i 0)) (if (= i 1000) i (loop (catch #t (lambda () (throw 'x (+ i 1))) (lambda (k v) v)))))",
        "",
        "1000",
    ),
    (
        "(catch 'a 1 (lambda a a))",
        "wrong-type-arg",
        "Wrong type to apply: 1",
    ),
    (
        "(catch 'a 1 2)",
        "wrong-type-arg",
        "In procedure catch: Wrong type argument in position 3: 2",
    ),
    (
        "(catch 1 (lambda () 1) (lambda a a))",
        "wrong-type-arg",
        "In procedure catch: Wrong type argument in position 2: 1",
    ),
    (
        "(catch #t (lambda () (error \"x\" 1)) (lambda (k s m a r) (list k s m a r)))",
        "",
        "(misc-error #f \"~A ~S\" (\"x\" 1) #f)",
    ),
    (
        "(catch #t (lambda () (no-such)) (lambda (k s m a r) (list k s m a r)))",
        "",
        "(unbound-variable #f \"Unbound variable: ~S\" (no-such) #f)",
    ),
    (
        "(catch 'syntax-error (lambda () (eval '(if) (interaction-environment))) (lambda (k . a) k))",
        "",
        "syntax-error",
    ),
    // the arguments a handler sees, compiled code's and the procedures'
    (
        "(catch #t (lambda () (let ((x 1)) (car x))) (lambda (k s m a r) (list k s m a r)))",
        "",
        "(wrong-type-arg \"car\" \"Wrong type argument in position 1 (expecting pair): ~S\" (1) (1))",
    ),
    (
        "(catch #t (lambda () (let ((x 1)) (vector-ref (vector 1) 5))) (lambda (k s m a r) (list k s m a r)))",
        "",
        "(out-of-range \"vector-ref\" \"Argument 2 out of range: ~S\" (5) (5))",
    ),
    (
        "(catch #t (lambda () (let ((x 1)) (vector-set! x 0 0))) (lambda (k s m a r) (list k s m a r)))",
        "",
        "(wrong-type-arg \"vector-set!\" \"Wrong type argument in position 1 (expecting mutable vector): ~S\" (1) (1))",
    ),
    (
        "(catch #t (lambda () (let ((x 1)) (vector-ref (vector 1) 'y))) (lambda (k s m a r) (list k s m a r)))",
        "",
        "(wrong-type-arg \"vector-ref\" \"Wrong type argument in position 2 (expecting small integer): ~S\" (y) (y))",
    ),
    (
        "(catch #t (lambda () (map car '(1))) (lambda (k s m a r) (list k s m a r)))",
        "",
        "(wrong-type-arg \"car\" \"Wrong type (expecting ~A): ~S\" (\"pair\" 1) (1))",
    ),
    (
        "(catch #t (lambda () (vector-set! 'a 0 0)) (lambda (k s m a r) (list k s m a r)))",
        "",
        "(wrong-type-arg \"vector-set!\" \"Wrong type argument in position 1 (expecting mutable vector): ~S\" (a) (a))",
    ),
    (
        "(catch #t (lambda () (symbol->string 1)) (lambda (k s m a r) (list k s m a r)))",
        "",
        "(wrong-type-arg \"symbol->string\" \"Wrong type argument in position ~A (expecting ~A): ~S\" (1 \"symbol\" 1) (1))",
    ),
    // dynamic-wind
    (
        "(let ((l '())) (dynamic-wind (lambda () (set! l (cons 'in l))) (lambda () (set! l (cons 'body l))) (lambda () (set! l (cons 'out l)))) l)",
        "",
        "(out body in)",
    ),
    (
        "(let ((l '())) (catch 'x (lambda () (dynamic-wind (lambda () (set! l (cons 'in l))) (lambda () (throw 'x)) (lambda () (set! l (cons 'out l))))) (lambda a #f)) l)",
        "",
        "(out in)",
    ),
    (
        "(list (dynamic-wind (lambda () 1) (lambda () (values 1 2)) (lambda () 3)))",
        "",
        "(1)",
    ),
    (
        "(call-with-values (lambda () (dynamic-wind (lambda () 1) (lambda () (values 1 2)) (lambda () 3))) list)",
        "",
        "(1 2)",
    ),
    (
        "(let ((l '())) (catch #t (lambda () (dynamic-wind (lambda () #f) (lambda () (dynamic-wind (lambda () #f) (lambda () (car 1)) (lambda () (set! l (cons 'inner l))))) (lambda () (set! l (cons 'outer l))))) (lambda (k . a) (set! l (cons k l)))) l)",
        "",
        "(wrong-type-arg outer inner)",
    ),
    (
        "(let ((l '())) (catch 'b (lambda () (dynamic-wind (lambda () #f) (lambda () (throw 'a)) (lambda () (throw 'b 1)))) (lambda (k x) (set! l (cons x l)))) l)",
        "",
        "(1)",
    ),
    (
        "(dynamic-wind 1 2 3)",
        "wrong-type-arg",
        "Wrong type to apply: 1",
    ),
    // escapes
    ("(call/ec (lambda (k) (+ 1 (k 42))))", "", "42"),
    ("(call-with-escape-continuation (lambda (k) 5))", "", "5"),
    (
        "(call-with-values (lambda () (call/ec (lambda (k) (k 1 2)))) list)",
        "",
        "(1 2)",
    ),
    (
        "(list (call/ec (lambda (k) (k))))",
        "misc-error",
        "Zero values returned to single-valued continuation",
    ),
    (
        "((lambda (f) (f 1)) car)",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): 1",
    ),
    (
        "(define (h x) (apply car (list x))) (h 1)",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): 1",
    ),
    (
        "(define (h x) (map vector-length (list x))) (h 1)",
        "wrong-type-arg",
        "In procedure vector-length: Wrong type argument in position 1: 1",
    ),
    (
        "(define (h x) (let ((c car)) (c x))) (h 1)",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): 1",
    ),
    (
        "(let ((x 1)) (vector-ref x 0))",
        "wrong-type-arg",
        "In procedure vector-ref: Wrong type argument in position 1 (expecting vector): 1",
    ),
    (
        "(define v (vector 1)) (define (h) (vector-set! v 9 0)) (h)",
        "out-of-range",
        "In procedure vector-set!: Argument 2 out of range: 9",
    ),
    (
        "((call/ec (lambda (k) k)) 1)",
        "misc-error",
        "In procedure abort: Abort to unknown promptError while printing exception.",
    ),
    ("(call/ec 1)", "wrong-type-arg", "Wrong type to apply: 1"),
    (
        "(call/ec (lambda (outer) (call/ec (lambda (inner) (outer 'out))) 'no))",
        "",
        "out",
    ),
    (
        "(catch #t (lambda () (call/ec (lambda (k) (dynamic-wind (lambda () #f) (lambda () (k 'escaped)) (lambda () (throw 'unwound)))))) (lambda (key . a) key))",
        "",
        "unwound",
    ),
    (
        "(let ((l '())) (call/ec (lambda (k) (dynamic-wind (lambda () (set! l (cons 'in l))) (lambda () (k 1)) (lambda () (set! l (cons 'out l)))))) l)",
        "",
        "(out in)",
    ),
    (
        "(call/ec (lambda (k) (catch #t (lambda () (k 'past-catch)) (lambda a 'caught))))",
        "",
        "past-catch",
    ),
    (
        "(define (find-first p l) (call/ec (lambda (return) (for-each (lambda (x) (if (p x) (return x))) l) #f))) (list (find-first even? '(1 3 4 5)) (find-first even? '(1 3)))",
        "",
        "(4 #f)",
    ),
    // fluids and parameters
    ("(fluid-ref (make-fluid))", "", "#f"),
    (
        "(let ((f (make-fluid 1))) (with-fluid* f 2 (lambda () (fluid-ref f))))",
        "",
        "2",
    ),
    (
        "(let ((f (make-fluid 1))) (with-fluids ((f 2)) (fluid-set! f 3) (fluid-ref f)))",
        "",
        "3",
    ),
    (
        "(let ((f (make-fluid 1))) (with-fluids ((f 2)) (fluid-set! f 3)) (fluid-ref f))",
        "",
        "1",
    ),
    (
        "(let ((f (make-fluid 1))) (catch #t (lambda () (with-fluid* f 2 (lambda () (throw 'x)))) (lambda a (fluid-ref f))))",
        "",
        "1",
    ),
    (
        "(let ((f (make-fluid 1)) (g (make-fluid 'a))) (with-fluids ((f 2) (g 'b)) (list (fluid-ref f) (fluid-ref g))))",
        "",
        "(2 b)",
    ),
    (
        "(with-fluids* (list (make-fluid)) (list 2) (lambda () 3))",
        "",
        "3",
    ),
    ("(list (fluid? (make-fluid)) (fluid? 1))", "", "(#t #f)"),
    (
        "(fluid-ref 1)",
        "wrong-type-arg",
        "In procedure fluid-ref: Wrong type argument in position 1 (expecting fluid): 1",
    ),
    (
        "(fluid-set! 1 2)",
        "wrong-type-arg",
        "In procedure fluid-set!: Wrong type argument in position 1 (expecting fluid): 1",
    ),
    (
        "(with-fluid* 1 2 (lambda () 3))",
        "wrong-type-arg",
        "In procedure with-fluid*: Wrong type: 1",
    ),
    (
        "(with-fluids ((1 2)) 3)",
        "wrong-type-arg",
        "In procedure with-fluid*: Wrong type: 1",
    ),
    (
        "(with-fluids* 1 2 3)",
        "wrong-type-arg",
        "In procedure scm_c_with_fluids: Wrong type argument in position 1: 1",
    ),
    (
        "(let ((p (make-parameter 1 (lambda (x) (* x 10))))) (list (p) (parameterize ((p 2)) (p)) (p)))",
        "",
        "(10 20 10)",
    ),
    (
        "(let ((p (make-parameter 1))) (list (p 2) (p)))",
        "",
        "(1 2)",
    ),
    (
        "(let ((p (make-parameter 1)) (q (make-parameter 2))) (parameterize ((p 3) (q 4)) (+ (p) (q))))",
        "",
        "7",
    ),
    (
        "(let ((p (make-parameter 1))) (catch #t (lambda () (parameterize ((p 2)) (throw 'x))) (lambda a (p))))",
        "",
        "1",
    ),
    (
        "(list (parameter? (make-parameter 1)) (procedure? (make-parameter 1)) (parameter? car))",
        "",
        "(#t #t #f)",
    ),
    ("(parameterize () 1)", "", "1"),
    (
        "(parameterize ((1 2)) 3)",
        "wrong-type-arg",
        "In procedure parameterize: Not a parameter: 1",
    ),
    // eval
    ("(eval '(+ 1 2) (interaction-environment))", "", "3"),
    ("(primitive-eval '(* 2 3))", "", "6"),
    ("(define x 5) (eval 'x (interaction-environment))", "", "5"),
    ("(eval '(define y 7) (interaction-environment)) y", "", "7"),
    (
        "(eval '(car 1) (interaction-environment))",
        "wrong-type-arg",
        "In procedure car: Wrong type (expecting pair): 1",
    ),
    (
        "(eval 'no-such-thing (interaction-environment))",
        "unbound-variable",
        "Unbound variable: no-such-thing",
    ),
    (
        "(eval '(if) (interaction-environment))",
        "syntax-error",
        "Syntax error:\nunknown location: source expression failed to match any pattern in form (if)",
    ),
    (
        "(eval 'x 1)",
        "wrong-type-arg",
        "In procedure eval: Wrong type argument in position 2 (expecting module): 1",
    ),
    (
        "(eval 'x)",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure eval (_ _)>",
    ),
    (
        "(interaction-environment 1)",
        "wrong-number-of-args",
        "Wrong number of arguments to #<procedure interaction-environment ()>",
    ),
];

/// `#<procedure 1a2b (a)>` and Guile's `#<procedure 1a2b at
/// ice-9/eval.scm:333:13 (a)>` as `#<procedure ANON (a)>`: an anonymous
/// procedure is named by its address in Guile and its index here.
fn anonymous(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("#<procedure ") {
        let (head, tail) = rest.split_at(i + "#<procedure ".len());
        out.push_str(head);
        // The table's names have no digits; an address or index has one.
        let hex = tail.chars().take_while(|c| c.is_ascii_hexdigit()).count();
        let after = &tail[hex..];
        if tail[..hex].contains(|c: char| c.is_ascii_digit()) && after.starts_with(' ') {
            out.push_str("ANON");
            rest = after;
            if let Some(at) = rest.strip_prefix(" at ")
                && let Some(end) = at.find(' ')
            {
                rest = &at[end..];
            }
        } else {
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

/// What the engine gives for a program: the key and text, or `""` and the
/// value written.
fn engine(source: &str) -> (String, String) {
    let mut vm = Vm::new();
    match vm.eval_text(source, None) {
        Ok(value) => {
            let mut out = String::new();
            vm.heap().write(value, &mut out);
            (String::new(), out)
        }
        Err(e) => {
            let key = vm.heap().name(e.key).unwrap_or("?").to_string();
            (key, anonymous(&e.text(vm.heap())))
        }
    }
}

#[test]
fn errors_are_worded_as_guile_words_them() {
    let mut failures = Vec::new();
    for &(source, key, text) in CASES {
        let (got_key, got) = engine(source);
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
fn guile_words_the_table_the_same() {
    // Each program in a fresh module, its forms evaluated in turn; one line
    // each: the key and print-exception's text, or a tab and the value.
    let mut program = String::from(
        r#"(use-modules (ice-9 control))
(define (escape s) (with-output-to-string (lambda () (string-for-each (lambda (c) (if (char=? c #\newline) (display "\\n") (display c))) s))))
(define (run text)
  (let ((m (make-fresh-user-module)) (p (open-input-string text)))
    (eval '(use-modules (ice-9 control)) m)
    (catch #t
      (lambda ()
        (let loop ((last *unspecified*))
          (let ((x (read p)))
            (if (eof-object? x)
                (begin (display "\t") (display (escape (with-output-to-string (lambda () (write last))))))
                (loop (eval x m))))))
      (lambda (key . args)
        (display key) (display "\t")
        (display (escape (with-output-to-string (lambda () (print-exception (current-output-port) #f key args))))))))
  (newline))
"#,
    );
    for (source, _, _) in CASES {
        program.push_str(&format!("(run {})\n", literal(source)));
    }
    let path = std::env::temp_dir().join(format!("lily-errors-{}.scm", std::process::id()));
    std::fs::write(&path, &program).unwrap();
    let out = Command::new("guile")
        .arg("--no-auto-compile")
        .arg(&path)
        .output();
    let _ = std::fs::remove_file(&path);
    let Ok(out) = out else {
        eprintln!("guile is not installed: the table is unverified against it");
        return;
    };
    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        CASES.len(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut failures = Vec::new();
    for ((source, key, text), line) in CASES.iter().zip(lines) {
        let (got_key, got) = line.split_once('\t').unwrap_or((line, ""));
        // Guile's printer ends with a newline, which `text` leaves out.
        let got = anonymous(&got.replace("\\n", "\n"));
        let got = got.strip_suffix('\n').unwrap_or(&got);
        if (got_key, got) != (*key, *text) {
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

#[test]
fn error_cases_22_and_23_print_their_err_line() {
    // packages/common/test/parser/errors/22-scheme-eval.ly and
    // 23-dollar-unbound.ly, the place of the Scheme form from the first line
    // of their .err, and the line of the .err that is Guile's.
    let cases = [
        (
            "\\version \"2.24.0\"\n#(car '())\n{ c4 }\n",
            (2, 2),
            "In procedure car: Wrong type (expecting pair): ()",
        ),
        (
            "\\version \"2.24.0\"\n{ c4 $(no-such-thing) d }\n",
            (2, 7),
            "Unbound variable: no-such-thing",
        ),
    ];
    for (ly, (line, column), expected) in cases {
        // LilyPond's line and column, from 1, of the form after `#` or `$`.
        let at = ly
            .char_indices()
            .find(|&(i, _)| {
                let p = Position::default().advance(&ly[..i]);
                (p.line + 1, p.column + 1) == (line, column)
            })
            .map(|(i, _)| i)
            .unwrap();
        let mut vm = Vm::new();
        let form = read(vm.heap_mut(), ly, at, None).unwrap().datum;
        let error = vm.eval(form).unwrap_err();
        assert_eq!(error.text(vm.heap()), expected);
    }
}

#[test]
fn location_is_what_the_host_sets() {
    let mut vm = Vm::new();
    // `*location*` is `(lily)`'s, as in LilyPond.
    let used = vm.eval_text("(use-modules (lily)) (*location*)", None);
    assert_eq!(used.unwrap(), Value::FALSE);
    let location = vm.heap_mut().string("file.ly:2:3");
    assert_eq!(vm.set_location(location), Value::FALSE);
    let seen = vm
        .eval_text("(define (where) (*location*)) (where)", None)
        .unwrap();
    assert_eq!(seen, location);
    assert_eq!(vm.location(), location);
    assert_eq!(vm.set_location(Value::FALSE), location);
    assert_eq!(vm.eval_text("(where)", None).unwrap(), Value::FALSE);
}

#[test]
fn a_cancelled_run_stops_and_the_vm_goes_on() {
    let mut vm = Vm::new();
    let checks = Rc::new(Cell::new(0u32));
    let seen = checks.clone();
    vm.set_cancel(Some(Box::new(move || {
        seen.set(seen.get() + 1);
        seen.get() >= 3
    })));
    // A loop that would run forever, inside a catch of every key, a wind and
    // a fluid's binding: the cancel passes the catch, skips `after`, and
    // restores the fluid.
    let program = "
        (define f (make-fluid 'outer))
        (define after-ran #f)
        (catch #t
          (lambda ()
            (with-fluid* f 'inner
              (lambda ()
                (dynamic-wind
                  (lambda () #f)
                  (lambda () (let loop ((i 0)) (loop (+ i 1))))
                  (lambda () (set! after-ran #t))))))
          (lambda args 'caught))";
    let error = vm.eval_text(program, None).unwrap_err();
    assert_eq!(vm.heap().name(error.key), Some("cancelled"));
    assert_eq!(checks.get(), 3, "checked every {CANCEL_INTERVAL} calls");
    vm.set_cancel(None);
    let mut out = String::new();
    let state = vm
        .eval_text("(list (fluid-ref f) after-ran)", None)
        .unwrap();
    vm.heap().write(state, &mut out);
    assert_eq!(out, "(outer #f)");
    // Backward jumps count too: none are emitted yet, so a loop of calls
    // is what the check sees.
    let count = Rc::new(Cell::new(0u32));
    let seen = count.clone();
    vm.set_cancel(Some(Box::new(move || {
        seen.set(seen.get() + 1);
        false
    })));
    let calls = 10 * CANCEL_INTERVAL;
    let program = format!("(let loop ((i 0)) (if (< i {calls}) (loop (+ i 1)) i))");
    vm.eval_text(&program, None).unwrap();
    assert!((10..=11).contains(&count.get()), "{} checks", count.get());
}

#[test]
fn escapes_leave_the_stacks_as_they_were() {
    let mut vm = Vm::new();
    // A hundred thousand catches, escapes and winds, each left by a throw,
    // in a loop in tail position: nothing is left behind, and the
    // collector runs among them.
    let program = "
        (define (spin n acc)
          (if (= n 0)
              acc
              (spin (- n 1)
                    (+ acc
                       (catch 'x (lambda () (throw 'x (make-string 10))) (lambda (k s) 1))
                       (call/ec (lambda (k) (dynamic-wind (lambda () #f) (lambda () (k 1)) (lambda () (make-string 10)))))))))
        (spin 100000 0)";
    let mut out = String::new();
    let value = vm.eval_text(program, None).unwrap();
    vm.heap().write(value, &mut out);
    assert_eq!(out, "200000");
    assert!(vm.heap().stats().collections > 0);
    // An error that nothing catches restores the fluids it passes.
    vm.eval_text("(define g (make-fluid 1))", None).unwrap();
    let error = vm
        .eval_text("(with-fluid* g 2 (lambda () (car 1)))", None)
        .unwrap_err();
    assert_eq!(
        error.text(vm.heap()),
        "In procedure car: Wrong type argument in position 1 (expecting pair): 1"
    );
    let mut out = String::new();
    let value = vm.eval_text("(fluid-ref g)", None).unwrap();
    vm.heap().write(value, &mut out);
    assert_eq!(out, "1");
}
