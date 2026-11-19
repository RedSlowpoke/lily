//! The springs and the horizontal solver (MS3 step 29):
//!
//! - `spring.cc` whole: `Spring ()`, `Spring (ideal, min)`, `operator*=`,
//!   `operator>`, `merge_springs`, `set_blocking_force` and the default
//!   strengths, field for field against LilyPond 2.26.0's own `spring.cc`
//!   compiled with clang beside a stub of its header (session 1's
//!   `scratch/s1-oracle/`), since none of them has a Scheme face.
//! - `Spring::length` fused as LilyPond's clang fuses it (`fmadd` in the
//!   2.26.0 binary): spring positions of `ly:solve-spring-rod-problem`
//!   that LilyPond 2.26.0 gives and an unfused length misses
//!   (`scratch/s1-fma.ly`, 15 of whose 40 cases it misses).
//! - `spring-smob.cc`'s smob: `ly:make-spring`, `ly:spring?`,
//!   `ly:spring-set-inverse-compress-strength!` and
//!   `-stretch-strength!`, their printer, equality, copies, programming
//!   errors and type errors, as LilyPond 2.26.0 gives them
//!   (`scratch/s1-smob.ly`).
//!
//! - `rod.cc` and `spaceable-grob.cc`, which have no Scheme face: a rod
//!   moved to its items' columns and kept on the left one, the bounds'
//!   protrusion, two rods to a column merged, the springs kept on a column
//!   and found again, and LilyPond's programming errors, as their lines
//!   give them, on grobs translated over one note (skipped without
//!   LilyPond 2.26.0's music types, and then unverified). Step 29's probe
//!   checks their effect through `ly:solve-spring-rod-problem`'s rods.
//! - `simple-spacer.cc`'s rods: springs and rods built and solved as
//!   `ly:solve-spring-rod-problem` builds and solves them, and through it,
//!   against LilyPond 2.26.0's forces and positions (session 3's
//!   `scratch/s3-rods.ly`); `force_penalty` by its lines.
//! - `simple-spacer-scheme.cc`'s `ly:solve-spring-rod-problem`: step 28's
//!   eight cases (its `scratch/s5-spacer.ly`, moved here from
//!   tests/layout.rs), through `%spacer-solve` too, and its answers and
//!   errors for odd arguments as LilyPond 2.26.0 gives them (session 4's
//!   `scratch/s4-face.ly`); a rod LilyPond crashes on throws, one past
//!   the last spring changes nothing.
//! - The line's columns, `get_line_configuration` (`Column_x_positions`)
//!   and `get_line_forces`, on grobs translated over one note: the springs
//!   and rods they take from the columns, as a spacer built by hand from
//!   them gives them, loose columns, `keep-inside-line`, forced breaks, a
//!   rod to a column's left piece and LilyPond's programming error (no
//!   Scheme face; skipped without LilyPond 2.26.0's music types).
//!
//! The numbers are LilyPond's, written here; nothing runs lilypond.
use std::path::PathBuf;
use std::process::Command;

use lily_engine_layout::rod::Rod;
use lily_engine_layout::simple_spacer::{SimpleSpacer, get_line_configuration, get_line_forces};
use lily_engine_layout::spaceable::{add_rod, add_spring, get_minimum_distances, get_spring};
use lily_engine_layout::spring::{Errors, Spring, SpringObject, merge_springs};
use lily_engine_layout::{grob, paper_column};
use lily_engine_scheme::{Value, Vm};

/// A VM with the music crate's and layout's functions, `(lily)` current.
fn vm() -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    eval(&mut vm, "(set-current-module (resolve-module '(lily)))");
    vm
}

fn eval(vm: &mut Vm, text: &str) -> Value {
    vm.eval_text(text, None)
        .unwrap_or_else(|e| panic!("{text}: {}", e.text(vm.heap())))
}

/// What `text` throws, as its text, or what it gives, written.
fn caught(vm: &mut Vm, text: &str) -> String {
    match vm.eval_text(text, None) {
        Ok(v) => {
            let mut out = String::new();
            vm.heap().write(v, &mut out);
            out
        }
        Err(e) => e.text(vm.heap()),
    }
}

/// The programming errors reported since the last call, written.
fn diagnosed(vm: &mut Vm) -> String {
    let v = eval(
        vm,
        "(let ((d (reverse (map (lambda (v) (vector-ref v 2)) (car %diagnostics)))))
           (set-car! %diagnostics '())
           d)",
    );
    let mut out = String::new();
    vm.heap().write(v, &mut out);
    out
}

/// A spring's ideal and minimum distances, inverse stretch and compress
/// strengths and blocking force, as the oracle prints them.
fn fields(s: &Spring) -> [f64; 5] {
    [
        s.ideal_distance,
        s.min_distance,
        s.inverse_stretch_strength,
        s.inverse_compress_strength,
        s.blocking_force,
    ]
}

fn spring(ideal: f64, min: f64) -> Spring {
    let mut errors = Errors::new();
    let s = Spring::new(ideal, min, &mut errors);
    assert!(errors.is_empty(), "{errors:?}");
    s
}

/// `Spring (ideal, min)` with its inverse strengths set, stretch first.
fn strong(ideal: f64, min: f64, stretch: f64, compress: f64) -> Spring {
    let mut errors = Errors::new();
    let mut s = Spring::new(ideal, min, &mut errors);
    s.set_inverse_stretch_strength(stretch, &mut errors);
    s.set_inverse_compress_strength(compress, &mut errors);
    assert!(errors.is_empty(), "{errors:?}");
    s
}

#[test]
fn springs_are_made_as_lilypond_makes_them() {
    assert_eq!(fields(&Spring::default()), [1.0, 1.0, 1.0, 1.0, 0.0]);
    assert_eq!(fields(&spring(2.0, 1.0)), [2.0, 1.0, 2.0, 1.0, -1.0]);
    assert_eq!(fields(&spring(1.0, 2.0)), [1.0, 2.0, 1.0, 0.0, 1.0]);

    // set_default_compress_strength updates the blocking force,
    // set_default_stretch_strength does not.
    let mut errors = Errors::new();
    let mut s = spring(2.0, 0.5);
    s.set_inverse_stretch_strength(7.0, &mut errors);
    s.set_default_compress_strength();
    assert_eq!(fields(&s), [2.0, 0.5, 7.0, 1.5, -1.0]);
    let mut s = spring(2.0, 0.5);
    s.set_inverse_stretch_strength(7.0, &mut errors);
    s.set_min_distance(3.0, &mut errors);
    s.set_default_stretch_strength();
    assert_eq!(fields(&s), [2.0, 3.0, 2.0, 1.5, 0.14285714285714285]);
    assert!(errors.is_empty());

    let s = spring(2.0, 1.0);
    assert_eq!(
        [
            s.length(-0.5, &mut errors),
            s.length(-5.0, &mut errors),
            s.length(0.25, &mut errors)
        ],
        [1.5, 1.0, 2.5]
    );
}

#[test]
fn springs_merge_as_lilypond_merges_them() {
    let mut errors = Errors::new();
    let merged = |springs: &[Spring], errors: &mut Errors| fields(&merge_springs(springs, errors));
    assert_eq!(
        merged(&[spring(2.0, 1.0)], &mut errors),
        [2.0, 1.0, 2.0, 1.0, -1.0]
    );
    assert_eq!(
        merged(&[spring(2.0, 1.0), spring(3.0, 0.5)], &mut errors),
        [2.5, 1.0, 2.5, 1.4285714285714286, -1.05]
    );
    assert_eq!(
        merged(
            &[
                strong(1.5, 1.4, 2.0, 0.25),
                strong(1.0, 0.2, 0.5, 0.75),
                strong(4.0, 1.0, 1.0, 3.0),
            ],
            &mut errors
        ),
        [
            2.1666666666666665,
            1.4,
            1.1666666666666667,
            0.5294117647058825,
            -1.4481481481481477
        ]
    );
    // The headroom above the largest minimum distance.
    assert_eq!(
        merged(&[spring(1.0, 0.9), spring(1.1, 1.0)], &mut errors),
        [1.3, 1.0, 1.05, 0.10000000000000003, -2.9999999999999996]
    );
    // A stiff spring makes the merged one stiff to compress.
    assert_eq!(
        merged(&[spring(1.0, 1.0), spring(2.0, 0.5)], &mut errors),
        [1.5, 1.0, 1.5, 0.0, 0.0]
    );
    assert!(errors.is_empty(), "{errors:?}");
    // Of none, the NaN strengths are refused.
    assert_eq!(merged(&[], &mut errors), [0.3, 0.0, 0.3, 0.3, -1.0]);
    assert_eq!(errors, ["insane spring constant"; 2]);
}

#[test]
fn springs_scale_compare_and_block_as_lilypond_s_do() {
    let scaled = |mut s: Spring, r: f64| {
        s *= r;
        fields(&s)
    };
    assert_eq!(scaled(spring(2.0, 1.0), 1.5), [3.0, 1.0, 3.0, 2.0, -1.0]);
    assert_eq!(scaled(spring(2.0, 1.0), 0.25), [1.0, 1.0, 0.5, 0.0, 0.0]);
    assert_eq!(
        scaled(strong(3.0, 0.5, 2.0, 1.0), 0.6),
        [1.7999999999999998, 0.5, 1.2, 1.2999999999999998, -1.0]
    );
    assert_eq!(scaled(spring(2.0, 1.0), 0.0), [1.0, 1.0, 0.0, 0.0, 0.0]);

    assert!(!spring(2.0, 1.0).is_greater(&spring(1.0, 2.0)));
    assert!(spring(1.0, 2.0).is_greater(&spring(2.0, 1.0)));
    assert!(!spring(2.0, 1.0).is_greater(&spring(2.0, 1.0)));

    let mut errors = Errors::new();
    let blocked = |mut s: Spring, f: f64, errors: &mut Errors| {
        s.set_blocking_force(f, errors);
        fields(&s)
    };
    assert_eq!(
        blocked(spring(2.0, 1.0), 0.5, &mut errors),
        [2.0, 3.0, 2.0, 1.0, 0.5]
    );
    assert_eq!(
        blocked(spring(2.0, 1.0), -0.5, &mut errors),
        [2.0, 1.5, 2.0, 1.0, -0.5]
    );
    assert_eq!(
        blocked(spring(2.0, 1.0), -3.0, &mut errors),
        [2.0, 1.0, 2.0, 1.0, -1.0]
    );
    assert_eq!(
        blocked(strong(3.0, 0.5, 0.0, 0.0), 1.0, &mut errors),
        [3.0, 3.0, 0.0, 0.0, 0.0]
    );
    assert!(errors.is_empty(), "{errors:?}");
    for f in [f64::INFINITY, f64::NAN] {
        assert_eq!(
            blocked(spring(2.0, 1.0), f, &mut errors),
            [2.0, 1.0, 2.0, 1.0, -1.0]
        );
    }
    assert_eq!(errors, ["insane blocking force"; 2]);
}

/// `(ly:solve-spring-rod-problem springs rods length ragged)` of `args`:
/// what it gives, written, or what it throws, as `catch` gives it.
fn solved(vm: &mut Vm, args: &str) -> String {
    caught(
        vm,
        &format!(
            "(catch #t
               (lambda () (ly:solve-spring-rod-problem {args}))
               (lambda args args))"
        ),
    )
}

/// `%spacer-solve`'s answer for springs `(ideal inverse-hooke)` built as
/// `ly:solve-spring-rod-problem` builds them, written as that writes its
/// own: the force, `#f` if it does not fit, then the positions.
fn spacer_solved(vm: &mut Vm, springs: &str, length: &str, ragged: &str) -> String {
    caught(
        vm,
        &format!(
            "(let ((r (%spacer-solve
                       (map (lambda (s)
                              (let ((sp (%make-spring (car s) 0.0)))
                                (%spring-set! sp 'inverse-compress-strength (cadr s))
                                (%spring-set! sp 'inverse-stretch-strength (cadr s))
                                sp))
                            '{springs})
                       {length} {ragged})))
               (cons (and (cadr r) (car r)) (caddr r)))"
        ),
    )
}

#[test]
fn a_spring_s_length_is_fused_as_lilypond_s() {
    let mut vm = vm();
    // LilyPond 2.26.0's `ly:solve-spring-rod-problem` with a rod it
    // ignores; each position differs in its last bit unfused.
    for (springs, length, got) in [
        (
            "((3.705 1.179) (2.46 0.711) (2.454 0.651) (1.07 1.37))",
            "3.755",
            "(-1.9142070051160958 0.0 1.448149940968123 2.547148760330579 \
             3.755000000000001 3.755000000000001)",
        ),
        (
            "((0.793 0.793) (2.738 1.563) (3.565 1.108) (0.73 0.766))",
            "5.006",
            "(-0.6666666666666666 0.0 0.26433333333333336 1.9603333333333335 \
             4.786666666666667 5.006)",
        ),
        (
            "((0.112 1.902) (1.512 1.033) (1.099 1.496))",
            "0.955",
            "(-0.6548042704626333 0.0 4.306681778344507e-18 0.8355871886120998 \
             0.9550000000000003)",
        ),
        (
            "((3.807 1.94) (2.768 0.512) (3.099 0.453))",
            "15.89",
            "(2.1397590361445786 0.0 7.958132530120483 11.821689156626507 15.89)",
        ),
    ] {
        let args = format!("'{springs} '((0 1 -1)) {length} #f");
        assert_eq!(solved(&mut vm, &args), got, "{springs}");
    }
}

#[test]
fn the_solver_gives_lilypond_s_forces_and_positions() {
    let mut vm = vm();
    // LilyPond 2.26.0's `ly:solve-spring-rod-problem` with a rod it
    // ignores (step 28's `scratch/s5-spacer.ly`, moved here from
    // tests/layout.rs), through it and through `%spacer-solve`, which
    // page_layout_problem.rs solves with.
    for (springs, length, ragged, got) in [
        (
            "((1 1) (2 0.5) (3 0))",
            "10",
            "#f",
            "(2.6666666666666665 0.0 3.6666666666666665 7.0 10.0)",
        ),
        (
            "((1 1) (2 0.5) (3 1))",
            "4",
            "#f",
            "(-0.8 0.0 0.19999999999999996 1.8 4.0)",
        ),
        (
            "((1 1) (2 0.5))",
            "10",
            "#t",
            "(4.666666666666667 0.0 1.0 3.0)",
        ),
        (
            "((1 1) (2 0.5))",
            "2",
            "#t",
            "(#f 0.0 0.33333333333333337 2.0)",
        ),
        ("((1 0.5) (2 0.5))", "-1", "#f", "(#f 0.0 0.0 0.0)"),
        ("((1 0) (2 0))", "5", "#f", "(2000000.0 0.0 1.0 3.0)"),
        ("((1 1) (2 1))", "3", "#f", "(0.0 0.0 1.0 3.0)"),
        (
            "((2 2) (0.5 0.25) (4 1))",
            "1.5",
            "#f",
            "(-2.5 0.0 0.0 0.0 1.5)",
        ),
    ] {
        let args = format!("'{springs} '((0 1 -1)) {length} {ragged}");
        assert_eq!(solved(&mut vm, &args), got, "{springs}");
        assert_eq!(
            spacer_solved(&mut vm, springs, length, ragged),
            got,
            "{springs}"
        );
    }
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn the_solver_s_scheme_face_is_lilypond_s() {
    let mut vm = vm();
    // LilyPond 2.26.0's answers and errors (session 4's
    // `scratch/s4-face.ly`).
    let wrong = |function: &str, position: usize, expecting: &str, what: &str| {
        format!(
            "(wrong-type-arg \"{function}\" \"Wrong type argument in position ~A \
             (expecting ~A): ~S\" ({position} \"{expecting}\" {what}) ({what}))"
        )
    };
    let wrong_vm = |subr: &str, expecting: &str, what: &str| {
        format!(
            "(wrong-type-arg {subr} \"Wrong type (expecting ~A): ~S\" \
             (\"{expecting}\" {what}) ({what}))"
        )
    };
    let out_of_range = "(out-of-range #f \"Value out of range ~S to< ~S: ~S\" \
                        (0 18446744073709551615 -1) (-1))";
    let srp = "ly_solve_spring_rod_problem";
    for (args, got) in [
        // No springs: nothing else is read.
        ("'() '() 'x 'y", "(0.0 0.0)".into()),
        ("'() 5 \"s\" 1", "(0.0 0.0)".into()),
        (
            "'((1 1) . 3) '((0 1 1)) 3 #f",
            wrong(srp, 1, "list of springs", "((1 1) . 3)"),
        ),
        ("7 '((0 1 1)) 3 #f", wrong(srp, 1, "list of springs", "7")),
        // The rods' type error names the first argument.
        ("'((1 1)) '() 3 #f", wrong(srp, 1, "list of rods", "()")),
        ("'((1 1)) 5 3 #f", wrong(srp, 1, "list of rods", "5")),
        (
            "'((1 1)) '((0 1 1) . 2) 3 #f",
            wrong(srp, 1, "list of rods", "((0 1 1) . 2)"),
        ),
        (
            "'((1 1)) '((0 1 1)) 'x #f",
            wrong("ly:solve-spring-rod-problem", 3, "number", "x"),
        ),
        (
            "'((1 1)) '((0 1 1)) 1+2i #f",
            wrong_vm("#f", "real number", "1.0+2.0i"),
        ),
        (
            "'((1 1) (1 1)) '((0 1 -1)) 7/2 #f",
            "(0.75 0.0 1.75 3.5)".into(),
        ),
        // Only #t is ragged.
        (
            "'((1 1) (2 0.5)) '((0 1 -1)) 2 1",
            "(-0.6666666666666666 0.0 0.33333333333333337 2.0)".into(),
        ),
        (
            "'((1 1) (2 0.5)) '((0 1 -1)) 2 #t",
            "(#f 0.0 0.33333333333333337 2.0)".into(),
        ),
        ("'(1) '((0 1 1)) 3 #f", wrong_vm("\"caar\"", "pair", "1")),
        (
            "'((1)) '((0 1 1)) 3 #f",
            wrong_vm("\"cadar\"", "pair", "()"),
        ),
        (
            "'((a 1)) '((0 1 1)) 3 #f",
            wrong_vm("#f", "real number", "a"),
        ),
        (
            "'((1 b)) '((0 1 1)) 3 #f",
            wrong_vm("#f", "real number", "b"),
        ),
        (
            "'((1/3 2/3) (2 1)) '((0 1 -1)) 2 #f",
            "(-0.2000000000000001 0.0 0.19999999999999993 1.9999999999999998)".into(),
        ),
        (
            "'((1 1 9 9) (2 1)) '((0 1 -1)) 2 #f",
            "(-0.5 0.0 0.5 2.0)".into(),
        ),
        ("'((1 1)) '((0)) 3 #f", wrong_vm("\"cadr\"", "pair", "()")),
        ("'((1 1)) '((-1 1 2)) 3 #f", out_of_range.into()),
        ("'((1 1)) '((0 -1 2)) 3 #f", out_of_range.into()),
        (
            "'((1 1)) '((0.0 1 2)) 3 #f",
            wrong_vm("#f", "exact integer", "0.0"),
        ),
        (
            "'((1 1)) '((0 x 2)) 3 #f",
            wrong_vm("#f", "exact integer", "x"),
        ),
        (
            "'((1 1)) '((0 1 d)) 3 #f",
            wrong_vm("#f", "real number", "d"),
        ),
        (
            "'((1 1) (1 1)) '((0 2 3 9)) 3 #f",
            "(0.5 0.0 1.5 3.0)".into(),
        ),
        (
            "'((1 1) (1 1)) '((0 2 7/2)) 2 #f",
            "(#f 0.0 1.75 3.5)".into(),
        ),
        ("'((2 1)) '((0 1 -1)) 3 #f", "(1.0 0.0 3.0)".into()),
        ("'((2 1) (1 1)) '((0 1 -1)) 0 #f", "(#f 0.0 0.0 0.0)".into()),
        (
            "'((2 1) (1 1)) '((0 1 -1)) -1 #t",
            "(#f 0.0 0.0 0.0)".into(),
        ),
    ] {
        assert_eq!(solved(&mut vm, args), got, "{args}");
    }
    assert_eq!(diagnosed(&mut vm), "()");
    // Springs and rods refused with LilyPond's programming errors.
    for (args, got, errors) in [
        (
            "'((-1 -2) (2 1)) '((0 1 -1)) 2 #f",
            "(-0.5 0.0 0.5 2.0)".to_string(),
            "(\"insane spring distance requested, ignoring it\" \
             \"insane spring constant\" \"insane spring constant\")",
        ),
        (
            "'((+inf.0 1) (2 1)) '((0 1 -1)) 2 #f",
            "(-0.5 0.0 0.5 2.0)".into(),
            "(\"insane spring distance requested, ignoring it\")",
        ),
        (
            "'((1 1) (1 1)) '((0 2 +nan.0)) 2 #f",
            "(0.0 0.0 1.0 2.0)".into(),
            "(\"ignoring weird minimum distance\")",
        ),
    ] {
        assert_eq!(solved(&mut vm, args), got, "{args}");
        assert_eq!(diagnosed(&mut vm), errors, "{args}");
    }
    // LilyPond prints the spring's programming error before the rod
    // throws; here the throw drops it.
    assert_eq!(
        solved(&mut vm, "'((-1 1)) '((0 1 x)) 3 #f"),
        wrong_vm("#f", "real number", "x")
    );
    assert_eq!(diagnosed(&mut vm), "()");
    // Where LilyPond reads a rod unchecked and crashes (2.26.0: SIGSEGV),
    // `car` throws; a rod past the last spring, which LilyPond reads past
    // its springs for (an answer that changes from run to run), changes
    // nothing.
    for (args, got) in [
        ("'((1 1)) '(3) 3 #f", wrong_vm("\"car\"", "pair", "3")),
        ("'((1 1)) '((0 1)) 3 #f", wrong_vm("\"car\"", "pair", "()")),
        ("'((1 1) (1 1)) '((0 3 5)) 4 #f", "(1.0 0.0 2.0 4.0)".into()),
        ("'((1 1) (1 1)) '((5 9 5)) 4 #f", "(1.0 0.0 2.0 4.0)".into()),
    ] {
        assert_eq!(solved(&mut vm, args), got, "{args}");
    }
    for call in [
        "(ly:solve-spring-rod-problem '((1 1)) '((0 1 1)) 3)",
        "(ly:solve-spring-rod-problem '((1 1)) '((0 1 1)) 3 #f 1)",
    ] {
        let got = caught(&mut vm, call);
        assert!(got.contains("Wrong number of arguments"), "{call}: {got}");
    }
}

#[test]
fn the_springs_smob_face_is_lilypond_s() {
    let mut vm = vm();
    eval(&mut vm, "(define s (ly:make-spring 2 1))");
    assert_eq!(caught(&mut vm, "s"), "#<Spring>");
    assert_eq!(caught(&mut vm, "(format #f \"~a\" s)"), "\"#<Spring>\"");
    assert_eq!(
        caught(
            &mut vm,
            "(list (ly:spring? s) (ly:spring? 1) (ly:spring? '()))"
        ),
        "(#t #f #f)"
    );
    // Equal only to itself, as Spring::equal_p.
    assert_eq!(
        caught(
            &mut vm,
            "(list (equal? s s) (equal? s (ly:make-spring 2 1)) (eqv? s s))"
        ),
        "(#t #f #t)"
    );
    let at = |vm: &mut Vm, text: &str| {
        let v = eval(vm, text);
        vm.heap().foreign::<SpringObject>(v).map(|s| fields(&s.0))
    };
    assert_eq!(at(&mut vm, "s"), Some([2.0, 1.0, 2.0, 1.0, -1.0]));

    // The setters change the spring and give a new copy of it.
    eval(
        &mut vm,
        "(define t (ly:spring-set-inverse-compress-strength! s 0.5))",
    );
    assert_eq!(
        caught(&mut vm, "(list (eq? s t) (equal? s t) (ly:spring? t))"),
        "(#f #f #t)"
    );
    assert_eq!(at(&mut vm, "s"), Some([2.0, 1.0, 2.0, 0.5, -2.0]));
    assert_eq!(at(&mut vm, "t"), at(&mut vm, "s"));
    eval(
        &mut vm,
        "(define u (ly:spring-set-inverse-stretch-strength! s 3))",
    );
    assert_eq!(caught(&mut vm, "(eq? s u)"), "#f");
    assert_eq!(at(&mut vm, "s"), Some([2.0, 1.0, 3.0, 0.5, -2.0]));
    assert_eq!(at(&mut vm, "u"), at(&mut vm, "s"));
    assert_eq!(caught(&mut vm, "(ly:make-spring 1/3 0)"), "#<Spring>");
    assert_eq!(diagnosed(&mut vm), "()");

    // Insane values: LilyPond's programming errors, a spring all the same.
    assert_eq!(
        caught(
            &mut vm,
            "(list (ly:spring? (ly:spring-set-inverse-stretch-strength! s -1))
                   (ly:spring? (ly:spring-set-inverse-compress-strength! s +inf.0))
                   (ly:make-spring -1 +nan.0))"
        ),
        "(#t #t #<Spring>)"
    );
    assert_eq!(
        diagnosed(&mut vm),
        "(\"insane spring constant\" \"insane spring constant\" \
         \"insane spring distance requested, ignoring it\" \
         \"insane spring min_distance requested, ignoring it\")"
    );
    assert_eq!(at(&mut vm, "s"), Some([2.0, 1.0, 3.0, 0.5, -2.0]));

    // LilyPond's type errors, the subr's name and the argument's
    // position, and Guile's for an arity.
    for (call, subr, position, expecting, what) in [
        ("(ly:make-spring 'a 1)", "ly:make-spring", 1, "number", "a"),
        (
            "(ly:make-spring 1 \"b\")",
            "ly:make-spring",
            2,
            "number",
            "\"b\"",
        ),
        (
            "(ly:spring-set-inverse-compress-strength! 1 2)",
            "ly:spring-set-inverse-compress-strength!",
            1,
            "Spring",
            "1",
        ),
        (
            "(ly:spring-set-inverse-compress-strength! s 'x)",
            "ly:spring-set-inverse-compress-strength!",
            2,
            "number",
            "x",
        ),
        (
            "(ly:spring-set-inverse-stretch-strength! 1 2)",
            "ly:spring-set-inverse-stretch-strength!",
            1,
            "Spring",
            "1",
        ),
        (
            "(ly:spring-set-inverse-stretch-strength! s 'x)",
            "ly:spring-set-inverse-stretch-strength!",
            2,
            "number",
            "x",
        ),
        (
            "(ly:spring-set-inverse-stretch-strength! 'y 'x)",
            "ly:spring-set-inverse-stretch-strength!",
            1,
            "Spring",
            "y",
        ),
    ] {
        let got = caught(&mut vm, call);
        let message =
            format!("Wrong type argument in position {position} (expecting {expecting}): {what}");
        assert!(
            got.contains(subr) && got.contains(&message),
            "{call}: {got}"
        );
    }
    for call in [
        "(ly:make-spring 1)",
        "(ly:make-spring 1 2 3)",
        "(ly:spring? 1 2)",
        "(ly:spring-set-inverse-compress-strength! s)",
        "(ly:spring-set-inverse-stretch-strength! s 1 2)",
    ] {
        let got = caught(&mut vm, call);
        assert!(got.contains("Wrong number of arguments"), "{call}: {got}");
    }
}

// --- rods and the columns' springs (rod.cc, spaceable-grob.cc) -------------

/// `scm/lily/` of LilyPond 2.26.0, as `build.rs` finds it.
fn lily_scm() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("LILY_SCM") {
        return Some(PathBuf::from(dir));
    }
    let path = Command::new("sh")
        .args(["-c", "command -v lilypond"])
        .output()
        .ok()?;
    let binary = std::fs::canonicalize(String::from_utf8(path.stdout).ok()?.trim()).ok()?;
    Some(binary.parent()?.join("../share/lilypond/2.26.0/scm/lily"))
}

/// Global with `Column`, a paper column, and `Thing`, an item, and a
/// Voice whose `Maker` makes four of each of each note.
const DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((Column (meta (name . Column) (classes Paper_column)
                      (interfaces grob-interface item-interface paper-column-interface)))
        (Thing (meta (name . Thing) (classes Item) (interfaces grob-interface item-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (default-child Voice))",
    "((context-name Voice) (translator-type Engraver_group) (consists Maker))",
];

/// [`vm`] with the music types, translated over one note: `(col n)` the
/// `n`th of its four `Column`s, ranked `n`, and `(thing n)` of its four
/// `Thing`s. `None` without LilyPond 2.26.0's music types: skipped, and
/// then unverified.
fn grobs_vm() -> Option<Vm> {
    if lily_engine_music::TYPES.is_empty() {
        eprintln!("skipped: no music types without LilyPond 2.26.0");
        return None;
    }
    let mut vm = vm();
    eval(
        &mut vm,
        "(define-public INF-MOMENT (ly:make-moment +inf.0))
         (define-public ZERO-MOMENT (ly:make-moment 0 1))",
    );
    let callbacks = lily_scm().unwrap().join("define-music-callbacks.scm");
    let text = std::fs::read_to_string(&callbacks).unwrap();
    vm.eval_text(&text, callbacks.to_str()).unwrap();
    lily_engine_music::define_music_types(&mut vm).unwrap();
    eval(
        &mut vm,
        "(define-public (ly:make-event-class class) (list class))
         (define-public made '())
         (ly:register-translator
           (lambda (context)
             (let ((heard #f))
               `((listeners (note-event . ,(lambda (t ev) (set! heard ev))))
                 (process-music
                  . ,(lambda (t)
                       (when heard
                         (for-each
                          (lambda (kind)
                            (do ((i 0 (1+ i))) ((= i 4))
                              (set! made (append made (list (ly:engraver-make-grob t kind heard))))))
                          '(Column Thing))
                         (set! heard #f)))))))
           'Maker)",
    );
    let scope = vm.make_module(&["lily"]).unwrap();
    let layout = vm.heap_mut().wrap(lily_engine_music::OutputDef::new(scope));
    vm.heap_mut().root(layout);
    for text in DEFS {
        let mods = eval(&mut vm, &format!("'{text}"));
        let mut items = Vec::new();
        let mut p = mods;
        while let (Some(m), Some(rest)) = (vm.heap().car(p), vm.heap().cdr(p)) {
            items.push(m);
            p = rest;
        }
        let (def, unknown) =
            lily_engine_music::make_context_def(vm.heap_mut(), None, &items, Value::FALSE);
        assert_eq!(unknown, 0, "{text}");
        let name = vm.heap().car(vm.heap().cdr(items[0]).unwrap()).unwrap();
        let name = vm.heap().name(name).unwrap().to_string();
        vm.module_define(scope, &name, def);
    }
    let music = eval(
        &mut vm,
        "(make-music 'NoteEvent 'pitch (ly:make-pitch 0 0 0) 'duration (ly:make-duration 2))",
    );
    vm.heap_mut().root(music);
    let done = lily_engine_music::run_translator_with(&mut vm, music, layout, None, &[])
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())))
        .unwrap();
    vm.heap_mut().root(done.translation.value);
    eval(
        &mut vm,
        "(define (col n) (list-ref made n))
         (define (thing n) (list-ref made (+ 4 n)))",
    );
    for n in 0..4 {
        let c = eval(&mut vm, &format!("(col {n})"));
        paper_column::set_rank(vm.heap(), c, n);
    }
    diagnosed(&mut vm);
    Some(vm)
}

/// `(thing n)` put on `(col c)` at `x`, its X extent `extent`.
fn place(vm: &mut Vm, n: usize, c: usize, x: f64, extent: &str) {
    eval(
        vm,
        &format!(
            "(ly:grob-set-parent! (thing {n}) 0 (col {c}))
             (%grob-set-property-data! (thing {n}) 'X-offset {x:?})
             (%grob-set-property-data! (thing {n}) 'X-extent '{extent})"
        ),
    );
}

/// The `minimum-distances` of `(col n)`, its columns by rank.
fn minimum_distances(vm: &mut Vm, n: usize) -> String {
    let c = eval(vm, &format!("(col {n})"));
    let mins = get_minimum_distances(vm, c).unwrap();
    let mut out = String::new();
    let mut s = mins;
    while let (Some(dist), Some(rest)) = (vm.heap().car(s), vm.heap().cdr(s)) {
        let (p, d) = (vm.heap().car(dist).unwrap(), vm.heap().cdr(dist).unwrap());
        out.push_str(&format!("({} . ", paper_column::rank(vm.heap(), p)));
        vm.heap().write(d, &mut out);
        out.push(')');
        s = rest;
    }
    out
}

#[test]
fn a_rod_moves_to_its_items_columns_and_stays_on_the_left_one() {
    let Some(mut vm) = grobs_vm() else { return };
    place(&mut vm, 0, 1, 0.75, "(-1 . 2)");
    place(&mut vm, 1, 2, -0.5, "(-0.25 . 3)");
    let (t0, t1) = (eval(&mut vm, "(thing 0)"), eval(&mut vm, "(thing 1)"));
    let (c1, c2) = (eval(&mut vm, "(col 1)"), eval(&mut vm, "(col 2)"));
    let mut r = Rod {
        item_drul: [Some(t0), Some(t1)],
        distance: 3.0,
    };
    // The bounds reach 2 into the rod on the left, 0.25 on the right.
    assert_eq!(r.bounds_protrusion(&mut vm).unwrap(), 2.25);
    // 3 + 0.75 from the left item's column, + 0.5 to the right one's.
    r.add_to_cols(&mut vm).unwrap();
    assert_eq!(r.item_drul, [Some(c1), Some(c2)]);
    assert_eq!(r.distance, 4.25);
    assert_eq!(minimum_distances(&mut vm, 1), "(2 . 4.25)");
    assert_eq!(minimum_distances(&mut vm, 2), "");
    // Columns are their own columns: the rod is added again, and kept.
    r.add_to_cols(&mut vm).unwrap();
    assert_eq!(r.distance, 4.25);
    assert_eq!(minimum_distances(&mut vm, 1), "(2 . 4.25)");
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_rod_without_both_columns_is_not_kept() {
    let Some(mut vm) = grobs_vm() else { return };
    place(&mut vm, 0, 1, 0.75, "(-1 . 2)");
    place(&mut vm, 1, 1, 1.5, "(0 . 1)");
    let (t0, t1) = (eval(&mut vm, "(thing 0)"), eval(&mut vm, "(thing 1)"));
    let (c1, t3) = (eval(&mut vm, "(col 1)"), eval(&mut vm, "(thing 3)"));
    // Without a right item, nothing moves.
    let mut r = Rod {
        item_drul: [Some(t0), None],
        distance: 3.0,
    };
    r.add_to_cols(&mut vm).unwrap();
    assert_eq!(
        r,
        Rod {
            item_drul: [Some(t0), None],
            distance: 3.0
        }
    );
    assert_eq!(r.bounds_protrusion(&mut vm).unwrap(), 2.0);
    assert_eq!(Rod::default().bounds_protrusion(&mut vm).unwrap(), 0.0);
    // Both on one column: moved, not kept.
    let mut r = Rod {
        item_drul: [Some(t0), Some(t1)],
        distance: 3.0,
    };
    r.add_to_cols(&mut vm).unwrap();
    assert_eq!(r.item_drul, [Some(c1), Some(c1)]);
    assert_eq!(r.distance, 2.25);
    // An item on no column: its coordinate from its root, and no column.
    eval(
        &mut vm,
        "(%grob-set-property-data! (thing 3) 'X-offset 0.5)",
    );
    let mut r = Rod {
        item_drul: [Some(t0), Some(t3)],
        distance: 3.0,
    };
    r.add_to_cols(&mut vm).unwrap();
    assert_eq!(r.item_drul, [Some(c1), None]);
    assert_eq!(r.distance, 3.25);
    assert_eq!(minimum_distances(&mut vm, 1), "");
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn two_rods_to_a_column_keep_the_larger_distance() {
    let Some(mut vm) = grobs_vm() else { return };
    let c: Vec<Value> = (0..4)
        .map(|n| eval(&mut vm, &format!("(col {n})")))
        .collect();
    add_rod(&mut vm, c[1], c[2], 2.0).unwrap();
    add_rod(&mut vm, c[1], c[3], 1.0).unwrap();
    add_rod(&mut vm, c[1], c[2], 1.5).unwrap();
    assert_eq!(minimum_distances(&mut vm, 1), "(3 . 1.0)(2 . 2.0)");
    add_rod(&mut vm, c[1], c[2], 2.5).unwrap();
    add_rod(&mut vm, c[1], c[3], 0.0).unwrap();
    // A negative distance is none.
    add_rod(&mut vm, c[1], c[3], -4.0).unwrap();
    add_rod(&mut vm, c[2], c[3], -0.0).unwrap();
    assert_eq!(minimum_distances(&mut vm, 1), "(3 . 1.0)(2 . 2.5)");
    assert_eq!(minimum_distances(&mut vm, 2), "(3 . -0.0)");
    assert_eq!(diagnosed(&mut vm), "()");
    // An infinite rod and a reverse one are LilyPond's programming
    // errors, and kept.
    add_rod(&mut vm, c[1], c[3], f64::INFINITY).unwrap();
    add_rod(&mut vm, c[2], c[0], 1.0).unwrap();
    assert_eq!(minimum_distances(&mut vm, 1), "(3 . +inf.0)(2 . 2.5)");
    assert_eq!(minimum_distances(&mut vm, 2), "(0 . 1.0)(3 . -0.0)");
    assert_eq!(
        diagnosed(&mut vm),
        "(\"infinite rod\" \"Adding reverse rod\")"
    );
}

#[test]
fn a_column_s_spring_to_the_next_is_the_last_one_added() {
    let Some(mut vm) = grobs_vm() else { return };
    let c: Vec<Value> = (0..4)
        .map(|n| eval(&mut vm, &format!("(col {n})")))
        .collect();
    let mut a = spring(2.0, 1.0);
    add_spring(&mut vm, c[1], c[2], &a).unwrap();
    // The column keeps a copy.
    a.set_ideal_distance(5.0, &mut Errors::new());
    assert_eq!(get_spring(&mut vm, c[1], c[2]).unwrap(), spring(2.0, 1.0));
    add_spring(&mut vm, c[1], c[3], &spring(3.0, 0.5)).unwrap();
    add_spring(&mut vm, c[1], c[2], &a).unwrap();
    assert_eq!(get_spring(&mut vm, c[1], c[2]).unwrap(), a);
    assert_eq!(get_spring(&mut vm, c[1], c[3]).unwrap(), spring(3.0, 0.5));
    assert_eq!(diagnosed(&mut vm), "()");
    let ideal = eval(&mut vm, "(ly:grob-object (col 1) 'ideal-distances)");
    assert_eq!(
        caught(
            &mut vm,
            "(map (lambda (e) (cons (ly:spring? (car e)) (eq? (cdr e) (col 3))))
                  (ly:grob-object (col 1) 'ideal-distances))"
        ),
        "((#t . #f) (#t . #t) (#t . #f))"
    );
    let first = vm.heap().car(vm.heap().car(ideal).unwrap()).unwrap();
    assert_eq!(vm.heap().foreign::<SpringObject>(first).unwrap().0, a);
    // None to the column: LilyPond's programming error and `Spring ()`.
    assert_eq!(get_spring(&mut vm, c[1], c[0]).unwrap(), Spring::default());
    assert_eq!(get_spring(&mut vm, c[3], c[0]).unwrap(), Spring::default());
    assert_eq!(
        diagnosed(&mut vm),
        "(\"No spring between column 1 and next one\" \
         \"No spring between column 3 and next one\")"
    );
}

// --- Simple_spacer's rods and the line's columns (simple-spacer.cc) --------

/// A `Simple_spacer` of springs `(ideal inverse-hooke)` and rods
/// `(left right distance)`, as `ly:solve-spring-rod-problem` builds it.
fn spacer(
    springs: &[(f64, f64)],
    rods: &[(usize, usize, f64)],
    errors: &mut Errors,
) -> SimpleSpacer {
    let mut spacer = SimpleSpacer::default();
    for &(ideal, inv_hooke) in springs {
        let mut sp = Spring::new(ideal, 0.0, errors);
        sp.set_inverse_compress_strength(inv_hooke, errors);
        sp.set_inverse_stretch_strength(inv_hooke, errors);
        spacer.add_spring(sp);
    }
    for &(l, r, distance) in rods {
        spacer.add_rod(l, r, distance, errors);
    }
    spacer
}

/// `ly:solve-spring-rod-problem`'s answer, as Guile writes it: the force,
/// `#f` if it does not fit, then the positions.
fn solve_written(spacer: &SimpleSpacer, length: f64, ragged: bool, errors: &mut Errors) -> String {
    let sol = spacer.solve(length, ragged, errors);
    let mut out = String::from("(");
    out.push_str(&if sol.fits {
        format!("{:?}", sol.force)
    } else {
        "#f".to_string()
    });
    for p in spacer.spring_positions(sol.force, ragged, errors) {
        out.push_str(&format!(" {p:?}"));
    }
    out.push(')');
    out
}

#[test]
fn rods_solve_as_lilypond_solves_them() {
    // LilyPond 2.26.0's `ly:solve-spring-rod-problem` (session 3's
    // `scratch/s3-rods.ly`): rods that make springs block, rods it
    // ignores, rods over springs that cannot move, which lengthen them,
    // and rods that overlap; built and solved as it does, and through it.
    let mut vm = vm();
    let one = [(1.0, 1.0); 3];
    let overlap = [(1.0, 1.0), (2.0, 0.5), (1.5, 2.0), (1.0, 1.0)];
    let overlap_rods = [(0, 2, 4.0), (1, 3, 5.0), (2, 4, 3.0), (0, 4, 9.0)];
    let thirds = [(0.7, 0.3), (1.1, 0.9), (0.4, 0.2)];
    let thirds_rods = [(0, 3, 3.3), (0, 1, 1.3), (1, 2, 1.7)];
    type Case<'a> = (
        &'a str,
        &'a [(f64, f64)],
        &'a [(usize, usize, f64)],
        f64,
        bool,
        &'a str,
    );
    let cases: &[Case] = &[
        (
            "rod-stretch",
            &one,
            &[(0, 2, 5.0)],
            4.0,
            false,
            "(#f 0.0 2.5 5.0 5.0)",
        ),
        (
            "rod-stretch-wide",
            &one,
            &[(0, 2, 5.0)],
            9.0,
            false,
            "(2.0 0.0 3.0 6.0 9.0)",
        ),
        (
            "rod-over",
            &one,
            &[(0, 1, 2.0)],
            2.0,
            false,
            "(#f 0.0 2.0 2.0 2.0)",
        ),
        (
            "rod-ignored",
            &one,
            &[(0, 2, -0.5)],
            2.0,
            false,
            "(-0.3333333333333333 0.0 0.6666666666666667 1.3333333333333335 2.0)",
        ),
        (
            "rod-short",
            &[(1.0, 1.0)],
            &[(0, 1, 0.5)],
            0.2,
            false,
            "(#f 0.0 0.5)",
        ),
        (
            "rod-short-fits",
            &[(1.0, 1.0)],
            &[(0, 1, 0.5)],
            0.7,
            false,
            "(-0.30000000000000004 0.0 0.7)",
        ),
        (
            "stiff-scaled",
            &[(1.0, 0.0), (2.0, 0.0)],
            &[(0, 2, 6.0)],
            5.0,
            false,
            "(#f 0.0 2.0 6.0)",
        ),
        (
            "stiff-zero",
            &[(0.0, 0.0), (0.0, 0.0)],
            &[(0, 2, 3.0)],
            5.0,
            false,
            "(2000000.0 0.0 1.5 3.0)",
        ),
        (
            "stiff-part",
            &[(1.0, 0.0), (1.0, 1.0)],
            &[(0, 1, 2.0)],
            3.0,
            false,
            "(0.0 0.0 2.0 3.0)",
        ),
        (
            "stiff-long",
            &[(1.0, 0.0), (2.0, 0.0)],
            &[(0, 2, 2.0)],
            5.0,
            false,
            "(2000000.0 0.0 1.0 3.0)",
        ),
        (
            "overlap",
            &overlap,
            &overlap_rods,
            8.0,
            false,
            "(#f 0.0 1.7777777777777781 4.166666666666668 7.222222222222224 9.000000000000002)",
        ),
        (
            "overlap-ragged",
            &overlap,
            &overlap_rods,
            8.0,
            true,
            "(#f 0.0 1.7777777777777781 4.166666666666668 7.222222222222224 9.000000000000002)",
        ),
        (
            "overlap-wide",
            &overlap,
            &overlap_rods,
            12.0,
            false,
            "(1.4444444444444444 0.0 2.4444444444444446 5.166666666666667 9.555555555555557 12.000000000000002)",
        ),
        (
            "overlap-wide-ragged",
            &overlap,
            &overlap_rods,
            12.0,
            true,
            "(1.4444444444444444 0.0 1.7777777777777781 4.166666666666668 7.222222222222224 9.000000000000002)",
        ),
        (
            "overlap-tight",
            &overlap,
            &overlap_rods,
            9.0,
            false,
            "(#f 0.0 1.7777777777777781 4.166666666666668 7.222222222222224 9.000000000000002)",
        ),
        (
            "reversed",
            &[(1.0, 1.0), (2.0, 1.0)],
            &[(2, 0, 3.0)],
            2.0,
            false,
            "(-0.5 0.0 0.5 2.0)",
        ),
        (
            "empty-range",
            &[(1.0, 1.0), (2.0, 1.0)],
            &[(1, 1, 3.0)],
            2.0,
            false,
            "(-0.5 0.0 0.5 2.0)",
        ),
        (
            "thirds",
            &thirds,
            &thirds_rods,
            2.9,
            false,
            "(#f 0.0 1.3 3.1071428571428568 3.6642857142857137)",
        ),
        (
            "thirds-wide",
            &thirds,
            &thirds_rods,
            4.1,
            false,
            "(1.181818181818182 0.0 1.3 3.463636363636364 4.1000000000000005)",
        ),
    ];
    for &(name, springs, rods, length, ragged, lilypond) in cases {
        let mut errors = Errors::new();
        let s = spacer(springs, rods, &mut errors);
        assert_eq!(
            solve_written(&s, length, ragged, &mut errors),
            lilypond,
            "{name}"
        );
        assert!(errors.is_empty(), "{name}: {errors:?}");
        let springs: Vec<String> = springs.iter().map(|(i, h)| format!("({i} {h})")).collect();
        let rods: Vec<String> = rods
            .iter()
            .map(|(l, r, d)| format!("({l} {r} {d})"))
            .collect();
        let args = format!(
            "'({}) '({}) {length} {}",
            springs.join(" "),
            rods.join(" "),
            if ragged { "#t" } else { "#f" }
        );
        assert_eq!(solved(&mut vm, &args), lilypond, "{name}");
    }
    assert_eq!(diagnosed(&mut vm), "()");
    // An infinite rod is LilyPond's programming error, and ignored.
    let mut errors = Errors::new();
    let s = spacer(
        &[(1.0, 1.0), (2.0, 1.0)],
        &[(0, 1, f64::INFINITY), (1, 2, 2.5)],
        &mut errors,
    );
    assert_eq!(
        solve_written(&s, 2.0, false, &mut errors),
        "(#f 0.0 0.0 2.5)"
    );
    assert_eq!(errors, ["ignoring weird minimum distance"]);
    assert_eq!(
        solved(&mut vm, "'((1 1) (2 1)) '((0 1 +inf.0) (1 2 2.5)) 2 #f"),
        "(#f 0.0 0.0 2.5)"
    );
    assert_eq!(diagnosed(&mut vm), "(\"ignoring weird minimum distance\")");
    let mut errors = Errors::new();
    let mut s = spacer(&[(1.0, 1.0)], &[], &mut errors);
    s.add_rod(0, 1, f64::NAN, &mut errors);
    // A rod past the last spring changes nothing.
    s.add_rod(0, 2, 5.0, &mut errors);
    assert_eq!(s.springs, [strong(1.0, 0.0, 1.0, 1.0)]);
    assert_eq!(errors, ["ignoring weird minimum distance"]);
}

#[test]
fn the_force_penalty_is_the_force_or_the_room_left() {
    let mut errors = Errors::new();
    let s = spacer(&[(1.0, 1.0), (2.0, 0.5)], &[], &mut errors);
    assert_eq!(s.force_penalty(10.0, 1.5, false, &mut errors), 1.5);
    assert_eq!(s.force_penalty(10.0, 0.0, false, &mut errors), 0.0);
    // A compression: f - 2 f⁴.
    assert_eq!(s.force_penalty(10.0, -0.5, false, &mut errors), -0.625);
    assert_eq!(s.force_penalty(10.0, -2.0, false, &mut errors), -34.0);
    // Ragged: the room after the springs at rest, never below 0.
    assert_eq!(s.force_penalty(10.0, -2.0, true, &mut errors), 7.0);
    assert_eq!(s.force_penalty(2.0, 1.0, true, &mut errors), 0.0);
    assert!(errors.is_empty(), "{errors:?}");
}

/// The columns `(col 0)` … `(col 3)`, the first its own right piece, the
/// last its own left piece.
fn line_columns(vm: &mut Vm) -> Vec<Value> {
    let c: Vec<Value> = (0..4).map(|n| eval(vm, &format!("(col {n})"))).collect();
    grob::with_grob_mut(vm.heap(), c[0], |g| g.broken = [None, Some(c[0])]);
    grob::with_grob_mut(vm.heap(), c[3], |g| g.broken = [Some(c[3]), None]);
    c
}

/// `got` within 1e-12 of `want`, a hand's numbers.
fn near(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?} {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 1e-12, "{got:?} {want:?}");
    }
}

/// A configuration's positions, force and satisfaction, from a spacer.
fn configured(s: &SimpleSpacer, line_len: f64, indent: f64, ragged: bool) -> (Vec<f64>, f64, bool) {
    let mut errors = Errors::new();
    let sol = s.solve(line_len, ragged, &mut errors);
    let force = s.force_penalty(line_len, sol.force, ragged, &mut errors);
    let config = s
        .spring_positions(sol.force, ragged, &mut errors)
        .into_iter()
        .map(|x| x + indent)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    (config, force, sol.fits)
}

#[test]
fn a_line_is_spaced_by_its_columns_springs_and_rods() {
    let Some(mut vm) = grobs_vm() else { return };
    let c = line_columns(&mut vm);
    let springs = [
        spring(2.0, 1.0),
        strong(3.0, 1.0, 1.5, 0.5),
        spring(1.0, 0.5),
    ];
    for i in 0..3 {
        add_spring(&mut vm, c[i], c[i + 1], &springs[i]).unwrap();
    }
    // A spring to a later column and a rod to an earlier one are not the
    // line's; the rod is LilyPond's programming error.
    add_spring(&mut vm, c[0], c[2], &spring(9.0, 9.0)).unwrap();
    add_rod(&mut vm, c[2], c[1], 7.0).unwrap();
    add_rod(&mut vm, c[0], c[2], 6.0).unwrap();
    add_rod(&mut vm, c[1], c[3], 3.5).unwrap();
    assert_eq!(diagnosed(&mut vm), "(\"Adding reverse rod\")");
    let mut expected = SimpleSpacer::default();
    for s in springs {
        expected.add_spring(s);
    }
    let mut errors = Errors::new();
    expected.add_rod(0, 2, 6.0, &mut errors);
    expected.add_rod(1, 3, 3.5, &mut errors);
    for (line_len, ragged) in [(8.0, false), (12.0, false), (5.0, false), (12.0, true)] {
        let got = get_line_configuration(&mut vm, &c, line_len, 1.5, ragged).unwrap();
        assert_eq!(got.cols, c);
        assert!(got.loose_cols.is_empty());
        let (config, force, fits) = configured(&expected, line_len, 1.5, ragged);
        assert_eq!(
            (got.config, got.force, got.satisfies_constraints),
            (config, force, fits)
        );
    }
    // By hand: the rod over the first two springs makes them block at the
    // force 2/7, which leaves them longer than the second rod; at 12 the
    // springs, 2 + 2f, 3 + 1.5f and 1 + f long, need the force 4/3.
    let got = get_line_configuration(&mut vm, &c, 12.0, 0.0, false).unwrap();
    near(&got.config, &[0.0, 14.0 / 3.0, 29.0 / 3.0, 12.0]);
    near(&[got.force], &[4.0 / 3.0]);
    assert!(got.satisfies_constraints);
    let got = get_line_configuration(&mut vm, &c, 5.0, 0.0, false).unwrap();
    assert!(!got.satisfies_constraints);
    assert_eq!(diagnosed(&mut vm), "()");

    // A loose column is set apart, and the spring over it is the line's.
    eval(
        &mut vm,
        "(ly:grob-set-object! (col 2) 'between-cols (cons (col 1) (col 3)))",
    );
    add_spring(&mut vm, c[1], c[3], &spring(4.0, 2.0)).unwrap();
    let got = get_line_configuration(&mut vm, &c, 9.0, 0.0, false).unwrap();
    assert_eq!(got.cols, [c[0], c[1], c[3]]);
    assert_eq!(got.loose_cols, [c[2]]);
    let mut expected = SimpleSpacer::default();
    expected.add_spring(springs[0]);
    expected.add_spring(spring(4.0, 2.0));
    expected.add_rod(1, 2, 3.5, &mut errors);
    assert!(errors.is_empty(), "{errors:?}");
    let (config, force, fits) = configured(&expected, 9.0, 0.0, false);
    assert_eq!(
        (got.config, got.force, got.satisfies_constraints),
        (config, force, fits)
    );
    // The rod from the first column to the loose one is no rod of the line.
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_line_keeps_its_columns_inside_and_its_breaks_unforced() {
    let Some(mut vm) = grobs_vm() else { return };
    let c = line_columns(&mut vm);
    let springs = [spring(2.0, 1.0), spring(3.0, 1.0), spring(1.0, 0.5)];
    for i in 0..3 {
        add_spring(&mut vm, c[i], c[i + 1], &springs[i]).unwrap();
    }
    // The second column reaches 2.5 left and 4.5 right of itself: rods
    // from the line's start to it and from it to the line's end.
    eval(
        &mut vm,
        "(%grob-set-property-data! (col 1) 'keep-inside-line #t)
         (%grob-set-property-data! (col 1) 'X-extent '(-2.5 . 4.5))
         (%grob-set-property-data! (col 0) 'keep-inside-line #t)
         (%grob-set-property-data! (col 0) 'X-extent '(-20 . 20))",
    );
    let mut expected = SimpleSpacer::default();
    for s in springs {
        expected.add_spring(s);
    }
    let mut errors = Errors::new();
    expected.add_rod(1, 3, 4.5, &mut errors);
    expected.add_rod(0, 1, 2.5, &mut errors);
    assert!(errors.is_empty(), "{errors:?}");
    let got = get_line_configuration(&mut vm, &c, 5.0, 0.0, false).unwrap();
    let (config, force, fits) = configured(&expected, 5.0, 0.0, false);
    assert_eq!(
        (&got.config, got.force, got.satisfies_constraints),
        (&config, force, fits)
    );
    // By hand: the rods raise the springs' minimum distances to 2.5,
    // 3.375 and 1.125, which do not fit in 5.
    assert_eq!(got.config, [0.0, 2.5, 5.875, 7.0]);
    assert!(!got.satisfies_constraints);
    // `#t` only: anything else does not keep it inside.
    eval(
        &mut vm,
        "(%grob-set-property-data! (col 1) 'keep-inside-line 1)",
    );
    let got = get_line_configuration(&mut vm, &c, 6.0, 0.0, false).unwrap();
    assert!(got.satisfies_constraints);
    assert_eq!(*got.config.last().unwrap(), 6.0);
    // A break forced inside the line: it does not satisfy its constraints;
    // at its ends, it does.
    eval(
        &mut vm,
        "(%grob-set-property-data! (col 2) 'line-break-permission 'force)",
    );
    let got = get_line_configuration(&mut vm, &c, 6.0, 0.0, false).unwrap();
    assert!(!got.satisfies_constraints);
    eval(
        &mut vm,
        "(%grob-set-property-data! (col 2) 'line-break-permission 'allow)
         (%grob-set-property-data! (col 3) 'line-break-permission 'force)",
    );
    let got = get_line_configuration(&mut vm, &c, 6.0, 0.0, false).unwrap();
    assert!(got.satisfies_constraints);
    // A rod to what is not a column is LilyPond's programming error.
    eval(
        &mut vm,
        "(ly:grob-set-object! (col 1) 'minimum-distances
           (list (cons (thing 0) 2.0) (cons (col 3) 1.0)))",
    );
    get_line_configuration(&mut vm, &c, 6.0, 0.0, false).unwrap();
    assert_eq!(
        diagnosed(&mut vm),
        "(\"minimum-distances holds an object that is not a paper column\")"
    );
}

#[test]
fn a_line_s_forces_are_found_for_each_pair_of_breaks() {
    let Some(mut vm) = grobs_vm() else { return };
    let c: Vec<Value> = (0..4)
        .map(|n| eval(&mut vm, &format!("(col {n})")))
        .collect();
    // Columns 0, 1 and 2, breakable at 0 and 2; column 3 is column 2's
    // left piece, of its rank.
    paper_column::set_rank(vm.heap(), c[3], 2);
    grob::with_grob_mut(vm.heap(), c[0], |g| g.broken = [None, Some(c[0])]);
    grob::with_grob_mut(vm.heap(), c[2], |g| g.broken = [Some(c[3]), Some(c[2])]);
    eval(
        &mut vm,
        "(%grob-set-property-data! (col 0) 'line-break-permission 'allow)
         (%grob-set-property-data! (col 2) 'line-break-permission 'allow)",
    );
    add_spring(&mut vm, c[0], c[1], &spring(2.0, 1.0)).unwrap();
    add_spring(&mut vm, c[1], c[2], &spring(9.0, 9.0)).unwrap();
    add_spring(&mut vm, c[1], c[3], &spring(3.0, 1.0)).unwrap();
    // A rod to the left piece is the line's end rod; to column 2, none.
    add_rod(&mut vm, c[1], c[3], 4.0).unwrap();
    add_rod(&mut vm, c[1], c[2], 8.0).unwrap();
    add_rod(&mut vm, c[0], c[1], 2.5).unwrap();
    let mut expected = SimpleSpacer::default();
    expected.add_spring(spring(2.0, 1.0));
    expected.add_spring(spring(3.0, 1.0));
    let mut errors = Errors::new();
    expected.add_rod(0, 1, 2.5, &mut errors);
    expected.add_rod(1, 2, 4.0, &mut errors);
    for (line_len, indent, ragged) in [(10.0, 1.0, false), (12.0, 0.0, false), (10.0, 1.0, true)] {
        let sol = expected.solve(line_len - indent, ragged, &mut errors);
        assert!(sol.fits);
        let force = expected.force_penalty(line_len, sol.force, ragged, &mut errors);
        let got = get_line_forces(&mut vm, &c[..3], line_len, indent, ragged).unwrap();
        let inf = f64::INFINITY;
        assert_eq!(got, [inf, force, inf, inf], "{line_len} {indent} {ragged}");
    }
    // By hand: the rods make the springs, 2 + 2f and 3 + 3f long, block
    // at 1/4 and 1/3; 9 long (10 less the indent) at 0.8.
    let got = get_line_forces(&mut vm, &c[..3], 10.0, 1.0, false).unwrap();
    near(&[got[1]], &[0.8]);
    // A line between neighbouring breaks that does not fit (6.5 at
    // least): -200000.
    let got = get_line_forces(&mut vm, &c[..3], 6.0, 0.0, false).unwrap();
    assert_eq!(got[1], -200000.0);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_line_of_one_column_is_its_right_piece_to_its_left_piece() {
    let Some(mut vm) = grobs_vm() else { return };
    let c: Vec<Value> = (0..4)
        .map(|n| eval(&mut vm, &format!("(col {n})")))
        .collect();
    // Column 0's pieces: column 3 on the left, itself on the right.
    grob::with_grob_mut(vm.heap(), c[0], |g| g.broken = [Some(c[3]), Some(c[0])]);
    add_spring(&mut vm, c[0], c[3], &spring(2.0, 1.0)).unwrap();
    let got = get_line_configuration(&mut vm, &c[..1], 5.0, 0.5, false).unwrap();
    assert_eq!(got.cols, [c[0], c[3]]);
    assert!(got.loose_cols.is_empty());
    // 2 + 2f = 5 at f = 1.5.
    assert_eq!(got.config, [0.5, 5.5]);
    assert_eq!((got.force, got.satisfies_constraints), (1.5, true));
    assert_eq!(diagnosed(&mut vm), "()");
}

#[test]
fn a_line_squeezed_to_exactly_its_springs_minimum_does_not_fit() {
    // OPEN B6: as LilyPond's `compress_line`, which stops at a spring only
    // while the line is still longer than wanted (`cur_len - block_dist <
    // line_len`), a line exactly as long as its blocked springs runs out
    // of springs and does not fit; so `get_line_forces` gives it an
    // infinite force (-200000 between neighbouring breaks).
    let mut errors = Errors::new();
    let mut spacer = SimpleSpacer::default();
    spacer.add_spring(spring(2.0, 1.0));
    spacer.add_spring(spring(3.0, 1.0));
    let sol = spacer.solve(2.0, false, &mut errors);
    assert!(!sol.fits);
    let sol = spacer.solve(2.0 + 1e-9, false, &mut errors);
    assert!(sol.fits);
    assert!(errors.is_empty(), "{errors:?}");
}
