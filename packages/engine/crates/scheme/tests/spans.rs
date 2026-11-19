//! Embedded forms evaluated at their spans (step 17a): [`Vm::eval_at`]
//! places psyntax's errors in the score as Guile places them for a form
//! LilyPond reads from the score's port, [`Vm::read_error`] places a read
//! error the same way, and `(*location*)` is the form's while it runs. Every
//! expected text is what LilyPond 2.26.0 printed for the same score. `(gc)`
//! collects at once.
use lily_engine_scheme::{Position, Source, Value, Vm, read};

/// The source of the form that begins at the byte `at` of `score`, as
/// `parse-scm.cc` sets the score's port: at the form's line, and at its
/// column less one. LilyPond counts these lines' columns as Guile does.
fn source<'t>(score: &'t str, file: &'t str, at: usize) -> Source<'t> {
    let mut port = Position::default().advance(&score[..at]);
    port.column -= 1;
    Source {
        text: score,
        file: Some(file),
        offset: at,
        at: port,
    }
}

/// The bytes after each `#` of `score` that begins a line's form.
fn forms(score: &str) -> Vec<usize> {
    score.match_indices("#(").map(|(i, _)| i + 1).collect()
}

fn written(vm: &Vm, value: Value) -> String {
    let mut out = String::new();
    vm.heap().write(value, &mut out);
    out
}

#[test]
fn syntax_errors_are_placed_in_the_score() {
    let scores = [
        (
            "spans.ly",
            "\\version \"2.26.0\"\n  #(let ((x)) x)\n#(lambda)\n é #(let ((y 1 2)) y)\n#(begin (define) 1)\n\t#(begin\t(let ((x)) x))\n#(let ((z 1))\n  (if))\n#(let ((z 1))\n  (let ((w)) w))\n#(car (quote ()))\n",
            &[
                "Syntax error:\nspans.ly:2:2: let: bad let in form (let ((x)) x)",
                "Syntax error:\nunknown location: lambda: bad lambda in form (lambda)",
                "Syntax error:\nspans.ly:4:3: let: bad let in form (let ((y 1 2)) y)",
                "Syntax error:\nspans.ly:5:7: source expression failed to match any pattern in form (define)",
                "Syntax error:\nspans.ly:6:16: let: bad let in form (let ((x)) x)",
                "Syntax error:\nunknown location: source expression failed to match any pattern in form (if)",
                "Syntax error:\nspans.ly:10:2: let: bad let in form (let ((w)) w)",
                "In procedure car: Wrong type (expecting pair): ()",
            ][..],
        ),
        // `(lily curried-definitions)`' `syntax-case` patterns: a form that
        // fits none fails with its place; `(define 1 2)` fits one and fails
        // in Guile's `define`.
        (
            "d.ly",
            "\\version \"2.26.0\"\n#(define x)\n#(define 1 2)\n#(define-public)\n#(define-public y)\n#(define* )\n#(define z . 1)\n",
            &[
                "Syntax error:\nd.ly:2:0: source expression failed to match any pattern in form (define x)",
                "Syntax error:\nunknown location: source expression failed to match any pattern in form (define 1 2)",
                "Syntax error:\nd.ly:4:0: source expression failed to match any pattern in form (define-public)",
                "Syntax error:\nd.ly:5:0: source expression failed to match any pattern in form (define-public y)",
                "Syntax error:\nd.ly:6:0: source expression failed to match any pattern in form (define*)",
                "Syntax error:\nd.ly:7:0: source expression failed to match any pattern in form (define z . 1)",
            ][..],
        ),
    ];
    for (file, score, expected) in scores {
        let at = forms(score);
        assert_eq!(at.len(), expected.len());
        let mut vm = Vm::new();
        // A score's module uses `(lily curried-definitions)`, whose `define`
        // is not Guile's.
        vm.eval_text("(use-modules (lily) (lily curried-definitions))", None)
            .unwrap();
        for (at, expected) in at.into_iter().zip(expected) {
            let r = read(vm.heap_mut(), score, at, None).unwrap();
            let source = source(score, file, at);
            let error = vm.eval_at(&r, &source, Value::FALSE).unwrap_err();
            assert_eq!(
                error.text(vm.heap()),
                *expected,
                "{}",
                &score[r.start..r.end]
            );
        }
    }
}

#[test]
fn a_read_error_is_placed_in_the_score() {
    // packages/common/test/parser/errors/21-scheme-read.ly and the line of
    // its .err that is Guile's.
    let score = "\\version \"2.24.0\"\n#(define x (list 1 2)\n{ c4 }\n";
    let at = forms(score)[0];
    let mut vm = Vm::new();
    let e = read(vm.heap_mut(), score, at, None).unwrap_err();
    let error = vm.read_error(&e, &source(score, "21-scheme-read.ly", at));
    assert_eq!(
        error.text(vm.heap()),
        "21-scheme-read.ly:4:1: unexpected end of input while searching for: )"
    );
}

#[test]
fn location_is_the_forms_while_it_runs() {
    let score = "#(use-modules (lily))\n#(*location*)\n#(define (where) (*location*))\n#(where)\n#(car (where))\n#(begin (gc) (*location*))\n";
    let mut vm = Vm::new();
    let at = forms(score);
    let eval = |vm: &mut Vm, i: usize, location: &str| {
        let r = read(vm.heap_mut(), score, at[i], None).unwrap();
        let location = vm.heap_mut().string(location);
        let result = vm.eval_at(&r, &source(score, "file.ly", at[i]), location);
        (result, location)
    };
    eval(&mut vm, 0, "file.ly:1:2").0.unwrap();
    let (seen, location) = eval(&mut vm, 1, "file.ly:2:2");
    assert_eq!(seen.unwrap(), location);
    assert_eq!(vm.location(), Value::FALSE, "the location before is back");
    eval(&mut vm, 2, "file.ly:3:2").0.unwrap();
    // A procedure sees the location of the form that calls it.
    let (seen, location) = eval(&mut vm, 3, "file.ly:4:2");
    assert_eq!(seen.unwrap(), location);
    // After an error too.
    let (error, _) = eval(&mut vm, 4, "file.ly:5:2");
    assert_eq!(
        error.unwrap_err().text(vm.heap()),
        "In procedure car: Wrong type (expecting pair): \"file.ly:5:2\""
    );
    assert_eq!(vm.location(), Value::FALSE);
    // The location a host set before outlives a collection in the form.
    let outer = vm.heap_mut().string("outer");
    vm.set_location(outer);
    let (seen, location) = eval(&mut vm, 5, "file.ly:6:2");
    assert_eq!(seen.unwrap(), location);
    assert_eq!(written(&vm, vm.location()), "\"outer\"");
}

#[test]
fn gc_collects_at_once() {
    let mut vm = Vm::new();
    let collections = |vm: &mut Vm, text: &str| {
        let before = vm.heap().stats().collections;
        let value = vm.eval_text(text, None).unwrap();
        (written(vm, value), vm.heap().stats().collections - before)
    };
    assert_eq!(collections(&mut vm, "(gc)"), ("#<unspecified>".into(), 1));
    assert_eq!(collections(&mut vm, "(apply gc '())").1, 1);
    let kept = "(define l (list 1 (string #\\a) (vector 2))) (define (f) (gc)) (f) (gc) l";
    assert_eq!(collections(&mut vm, kept), ("(1 \"a\" #(2))".into(), 2));
    let error = vm.eval_text("(gc 1)", None).unwrap_err();
    assert_eq!(
        error.text(vm.heap()),
        "Wrong number of arguments to #<procedure gc ()>"
    );
}
