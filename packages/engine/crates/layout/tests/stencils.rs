//! O1 (MS3 steps 2 and 3): stencils, boxes, transforms, shapes and lines
//! against LilyPond 2.26.0.
//!
//! - `tools/bench/probe-stencils.scm`'s calls, up to its markup commands,
//!   give what LilyPond gives for them (`tools/compare/stencil-calls.ly`),
//!   in a VM with the music crate's and layout's functions, the stencil
//!   heads of the installed `define-stencil-commands.scm` and
//!   `define-markup-commands.scm`'s `empty-stencil` and `point-stencil`
//!   (with `lily-library.scm`'s `empty-interval`). Skipped without
//!   LilyPond 2.26.0, and then unverified;
//!   `tools/bench/test/stencils.test.mjs` runs the whole probe from source,
//!   from an image and in wasm.
//! - The line interface's lines in each style that LilyPond drew through a
//!   note head's `ly:line-interface::line` (`stencil-lines.txt` of the same
//!   capture) are layout's [`line::line`] of the staff symbol's thickness
//!   and space and the same properties. Skipped without LilyPond 2.26.0.
//!   The engine's own `ly:line-interface::line` on a grob (MS3 step 52,
//!   OPEN A23) is checked on note heads against LilyPond's in
//!   `tools/compare/line-spanner-prints.ly` (`layout.test.mjs`), its
//!   arguments in `tests/layout.rs`.
//! - `lookup.cc`'s shapes that no `ly:` function draws, on exact values;
//!   the placeholders and `round_filled_box`'s warning; and `hypot` as
//!   macOS's libm gives it, which LilyPond's `Offset::length` calls.
//! - What the scheme crate's tests checked of the stand-ins layout replaced
//!   (`ly:make-stencil`, `ly:register-stencil-expression`, `ly:stencil?`),
//!   which LilyPond gave, and the geometry's exact cases, without LilyPond.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use lily_engine_layout::{
    Box, Interval, LineStyle, Stencil, Style, Transform, X, Y, angle_degrees, bezier,
    complex_multiply, direction, length, line, lookup, offset_directed,
};
use lily_engine_scheme::{Cx, Prim, Thrown, Value, Vm};

/// A VM with the music crate's and layout's functions, `(lily)` current.
fn vm() -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    vm.eval_text("(set-current-module (resolve-module '(lily)))", None)
        .unwrap();
    vm
}

/// What `source` gives: an empty key and the value as `write` writes it, or
/// the error's key and its text as Guile prints it.
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

fn tools() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools")
}

/// `scm/lily/` of the installed LilyPond 2.26.0, or None.
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
    lily.join("define-stencil-commands.scm")
        .exists()
        .then_some(lily)
}

/// What `stencil-calls.ly` gives: the records of the probe's calls, and
/// `stencil-lines.txt`'s lines, run once for the tests.
fn lilypond() -> &'static (Vec<String>, String) {
    static CAPTURE: OnceLock<(Vec<String>, String)> = OnceLock::new();
    CAPTURE.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("lily-engine-stencils-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let capture = std::fs::canonicalize(tools().join("compare/stencil-calls.ly")).unwrap();
        let out = Command::new("lilypond")
            .arg("-s")
            .arg(capture)
            .current_dir(&dir)
            .output()
            .unwrap();
        let lines = std::fs::read_to_string(dir.join("stencil-lines.txt"));
        std::fs::remove_dir_all(&dir).ok();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        (
            records(&String::from_utf8(out.stdout).unwrap()),
            lines.unwrap(),
        )
    })
}

/// A probe's output, one record per call.
fn records(output: &str) -> Vec<String> {
    let mut records: Vec<String> = output.split("\u{1e}\n").map(str::to_string).collect();
    assert_eq!(
        records.pop().as_deref(),
        Some(""),
        "the output ends a record"
    );
    records
}

/// The definitions from `head` to the end of the line that holds `last`, in
/// the installed `file`.
fn definitions(lily: &Path, file: &str, head: &str, last: &str) -> String {
    let text = std::fs::read_to_string(lily.join(file)).unwrap();
    let start = text.find(head).unwrap();
    let end = start + text[start..].find(last).unwrap();
    let end = end + text[end..].find('\n').unwrap();
    text[start..end].to_string()
}

#[test]
fn the_probe_gives_what_lilypond_gives() {
    let Some(lily) = lily_scm() else {
        eprintln!(
            "skipped: LilyPond 2.26.0 is not installed: the stencils are unverified against it"
        );
        return;
    };
    let mut vm = vm();
    let heads = std::fs::read_to_string(lily.join("define-stencil-commands.scm")).unwrap();
    let interval = definitions(
        &lily,
        "lily-library.scm",
        "(define-public empty-interval",
        "empty-interval",
    );
    let constants = definitions(
        &lily,
        "define-markup-commands.scm",
        "(define-public empty-stencil",
        "(define-public point-stencil",
    );
    for text in [heads, interval, constants] {
        if let Err(e) = vm.eval_text(&text, None) {
            panic!("{}", e.text(vm.heap()));
        }
    }
    let whole = std::fs::read_to_string(tools().join("bench/probe-stencils.scm")).unwrap();
    let probe = &whole[..whole.find("\n;; ---").unwrap()];
    if let Err(e) = vm.eval_text(probe, None) {
        panic!("the probe fails: {}", e.text(vm.heap()));
    }
    let engine = records(&vm.take_output());
    let all = &lilypond().0;
    let calls = probe.lines().filter(|l| l.starts_with('(')).count() - PROBE_DEFINITIONS;
    let in_all = whole.lines().filter(|l| l.starts_with('(')).count()
        - PROBE_DEFINITIONS
        - MARKUP_DEFINITIONS;
    assert_eq!(all.len(), in_all, "LilyPond ran every call");
    let expected = &all[..calls];
    assert_eq!(engine.len(), expected.len(), "the engine ran every call");
    let failures: Vec<String> = engine
        .iter()
        .zip(expected)
        .enumerate()
        .filter(|(_, (got, want))| got != want)
        .map(|(i, (got, want))| format!("call {}\n  engine   {got}\n  lilypond {want}", i + 1))
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} calls differ:\n{}",
        failures.len(),
        expected.len(),
        failures.join("\n")
    );
}

/// The probe's lines before its markup commands that begin with `(` and are
/// no call: its module, `probe-show`, `probe-error`, `probe-run`, its
/// seven stencils and the `for-each`.
const PROBE_DEFINITIONS: usize = 12;

/// Those of its markup commands' part: `probe-layout`,
/// `probe-interpret-markup`, the `set!` of `interpret-markup` and the one
/// that gives it back, `probe-box` and the `for-each`.
const MARKUP_DEFINITIONS: usize = 6;

/// The scheme crate's cases of the stand-ins (MS2 step 12), as LilyPond
/// gave them, with a head registered as `define-stencil-commands.scm` does.
const MOVED: &[(&str, &str, &str)] = &[
    (
        "(list (ly:make-stencil '()) (ly:stencil? (ly:make-stencil \"\" '(0 . 0) '(0 . 0))) (equal? (ly:make-stencil '()) (ly:make-stencil '())) (ly:stencil? 1))",
        "",
        "(#<Stencil> #t #f #f)",
    ),
    (
        "(ly:make-stencil 'a '(1 . 2) 3)",
        "wrong-type-arg",
        "In procedure ly:make-stencil: Wrong type argument in position 3 (expecting number pair): 3",
    ),
    (
        "(ly:make-stencil \"\" '(1 . 2) '(3 . a))",
        "wrong-type-arg",
        "In procedure ly:make-stencil: Wrong type argument in position 3 (expecting number pair): (3 . a)",
    ),
    (
        "(ly:make-stencil '(foo 1))",
        "wrong-type-arg",
        "In procedure ly_make_stencil: Wrong type argument in position 1 (expecting registered stencil expression): (foo 1)",
    ),
    (
        "(ly:stencil-expr (ly:make-stencil '(draw-line 0.1 0 0 1 1) '(0 . 1)))",
        "",
        "(draw-line 0.1 0 0 1 1)",
    ),
    (
        "(begin (ly:register-stencil-expression 'probe-head) (list (car (ly:all-stencil-expressions)) (object-property 'probe-head 'stencil-head?)))",
        "",
        "(probe-head #t)",
    ),
    (
        "(ly:register-stencil-expression \"x\")",
        "wrong-type-arg",
        "In procedure ly:register-stencil-expression: Wrong type argument in position 1 (expecting symbol): \"x\"",
    ),
];

#[test]
fn the_stand_ins_cases_hold() {
    let mut failures = Vec::new();
    for &(source, key, text) in MOVED {
        let mut vm = vm();
        run(&mut vm, "(ly:register-stencil-expression 'draw-line)");
        let (got_key, got) = run(&mut vm, source);
        if (got_key.as_str(), got.as_str()) != (key, text) {
            failures.push(format!(
                "{source}\n  engine {got_key}: {got}\n  want {key}: {text}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A stencil survives a collection, and an image restores it as layout's.
#[test]
fn stencils_outlive_a_collection_and_an_image() {
    let mut vm = vm();
    let make = "(ly:register-stencil-expression 'draw-line)
        (define s (ly:stencil-translate (ly:make-stencil '(draw-line 0.1 0 0 1 1) '(0 . 1) '(0 . 1)) '(1 . 2)))
        (define t (ly:make-rotation 30 '(1 . 1)))";
    run(&mut vm, make);
    vm.collect();
    let show = "(list (ly:stencil-expr s) (ly:stencil-extent s 0) (ly:stencil-extent s 1) (ly:transform->list t) (t '(1 . 0)))";
    let before = run(&mut vm, show);
    assert_eq!(before.0, "", "{}", before.1);
    let image = vm.write_image().unwrap();
    let mut restored = Vm::restore(
        &image,
        &[
            &lily_engine_music::EXTENSION,
            &lily_engine_layout::EXTENSION,
        ],
    )
    .unwrap();
    assert!(
        restored.write_image().unwrap() == image,
        "the restored VM writes the same image"
    );
    assert_eq!(run(&mut restored, show), before);
}

#[test]
fn directions_are_exact_where_lilypond_makes_them_so() {
    assert_eq!(offset_directed(0.0), (1.0, 0.0));
    assert_eq!(offset_directed(90.0), (0.0, 1.0));
    assert_eq!(offset_directed(-90.0), (0.0, -1.0));
    assert_eq!(offset_directed(180.0), (-1.0, 0.0));
    assert_eq!(offset_directed(-180.0), (-1.0, 0.0));
    assert_eq!(offset_directed(450.0), (0.0, 1.0));
    let (x, y) = offset_directed(45.0);
    assert_eq!(x, y);
    let (x, y) = offset_directed(-135.0);
    assert_eq!(x, y);
    assert_eq!(angle_degrees((0.0, 0.0)), 0.0);
    assert_eq!(angle_degrees((-1.0, 0.0)), 180.0);
    assert_eq!(angle_degrees((1.0, 1.0)), 45.0);
    assert_eq!(angle_degrees((0.0, -1.0)), -90.0);
    assert_eq!(direction((0.0, 0.0)), (0.0, 0.0));
    assert_eq!(direction((f64::INFINITY, 1.0)), (1.0, 0.0));
    assert_eq!(
        complex_multiply((1.0, 2.0), (0.0, f64::INFINITY)),
        (0.0, 0.0)
    );
}

#[test]
fn intervals_and_boxes_keep_lilypond_s_empty() {
    let mut i = Interval::EMPTY;
    assert!(i.is_empty());
    assert_eq!(i.length(), 0.0);
    i.scale(-2.0);
    assert_eq!(i, Interval::EMPTY, "an empty interval is not scaled");
    i.add_point(3.0);
    assert_eq!(i, Interval::new(3.0, 3.0));
    let mut j = Interval::new(1.0, 2.0);
    j.scale(-1.0);
    assert_eq!(j, Interval::new(-2.0, -1.0));
    assert_eq!(Interval::new(0.0, 2.0).linear_combination(0.5), 1.5);
    let mut b = Box::new(Interval::new(0.0, 1.0), Interval::EMPTY);
    b.translate((1.0, 1.0));
    assert_eq!(b.x(), Interval::new(1.0, 2.0));
    assert!(b.is_empty_on(Y) && !b.is_empty_on(X) && !b.is_empty());
}

#[test]
fn transforms_concatenate_as_pango_does() {
    let r = Transform::rotation(90.0, (1.0, 2.0));
    assert_eq!(r, Transform::new(0.0, -1.0, 1.0, 0.0, 3.0, 1.0));
    assert_eq!(r.apply((1.0, 2.0)), (1.0, 2.0), "the center stays");
    let mut t = Transform::translation((1.0, 0.0));
    t.scale(2.0, 3.0);
    assert_eq!(t.apply((1.0, 1.0)), (3.0, 3.0), "scaled first");
    assert_eq!(
        Transform::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0)
            .then(&Transform::new(6.0, 5.0, 4.0, 3.0, 2.0, 1.0)),
        Transform::new(14.0, 11.0, 34.0, 27.0, 9.0, 16.0)
    );
    assert_eq!(
        Transform::new(1.0, 3.0, 2.0, 4.0, 5.0, 6.0).text(),
        "[[1.000000 3.000000 5.000000] [2.000000 4.000000 6.000000]]"
    );
}

// --- the line interface through a grob -------------------------------------------

/// `(probe-line '(line-thickness staff-space style thickness dash-fraction
/// dash-period zigzag-width zigzag-length from to))`: layout's line of
/// those, `()` where a property is unset.
fn probe_line(cx: &mut Cx, args: &[Value]) -> Result<Value, Thrown> {
    let (style, from, to) = {
        let heap = cx.heap();
        let mut items = Vec::new();
        let mut list = args[0];
        while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
            items.push(item);
            list = rest;
        }
        let number = |v| heap.to_f64(v);
        let point = |v| {
            let (x, y) = (heap.car(v).unwrap(), heap.cdr(v).unwrap());
            (heap.to_f64(x).unwrap(), heap.to_f64(y).unwrap())
        };
        let style = LineStyle {
            style: line::style_of(heap, items[2]),
            thickness: number(items[3]),
            dash_fraction: number(items[4]),
            dash_period: number(items[5]),
            zigzag_width: number(items[6]),
            zigzag_length: number(items[7]),
            ..LineStyle::new(number(items[0]).unwrap(), number(items[1]).unwrap())
        };
        (style, point(items[8]), point(items[9]))
    };
    let s = line::line(cx, &style, from, to)?;
    Ok(cx.heap_mut().wrap(s))
}

fn probe_trill(cx: &mut Cx, _: &[Value]) -> Result<Value, Thrown> {
    let style = LineStyle {
        style: Style::Trill,
        ..LineStyle::new(0.1, 1.0)
    };
    line::line(cx, &style, (0.0, 0.0), (5.0, 0.0)).map(|_| Value::UNSPECIFIED)
}

static TEST_PRIMS: &[Prim] = &[
    Prim {
        name: "probe-line",
        min: 1,
        max: 1,
        f: probe_line,
    },
    Prim {
        name: "probe-trill",
        min: 0,
        max: 0,
        f: probe_trill,
    },
];

/// [`vm`] with the test's primitives and the probe's `probe-show`.
fn line_vm() -> Vm {
    let mut vm = vm();
    let show = "(define (probe-show x)
  (cond ((ly:stencil? x)
         (list 'stencil (ly:stencil-expr x) (ly:stencil-extent x 0) (ly:stencil-extent x 1)))
        ((pair? x) (cons (probe-show (car x)) (probe-show (cdr x))))
        (else x)))";
    vm.define_primitives("lily", TEST_PRIMS, show).unwrap();
    vm
}

#[test]
fn the_lines_give_what_lilypond_gives() {
    if lily_scm().is_none() {
        eprintln!("skipped: LilyPond 2.26.0 is not installed: the lines are unverified against it");
        return;
    }
    let mut vm = line_vm();
    let lines = &lilypond().1;
    let mut failures = Vec::new();
    let mut count = 0;
    for case in lines.lines() {
        let (call, want) = case.split_once('\t').unwrap();
        let (key, got) = run(&mut vm, &format!("(probe-show {call})"));
        count += 1;
        if !key.is_empty() || got != want {
            failures.push(format!("{call}\n  engine   {key} {got}\n  lilypond {want}"));
        }
    }
    assert_eq!(count, 40, "20 cases on each of two staves");
    assert!(
        failures.is_empty(),
        "{} of {count} lines differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn the_trill_throws_by_name_and_a_circular_polygon_is_no_list() {
    let mut vm = line_vm();
    for (call, key, text) in [
        (
            "(probe-trill)",
            "misc-error",
            // A style alone, no grob: the trill line needs the grob's font
            // (`grob_line` draws it since MS8 step 8's part 1, OPEN A19).
            "In procedure Line_interface::make_trill_line: needs the grob's font: Line_interface::line on a grob draws it",
        ),
        (
            "(let ((l (list '(0 . 0) '(1 . 0) '(1 . 1)))) (set-cdr! (cddr l) l) (catch #t (lambda () (ly:round-polygon l 0.1)) (lambda (key subr . _) (list key subr))))",
            "",
            "(wrong-type-arg \"ly_round_polygon\")",
        ),
    ] {
        assert_eq!(
            run(&mut vm, call),
            (key.to_string(), text.to_string()),
            "{call}"
        );
    }
}

#[test]
fn a_negative_box_warns_as_lilypond_does() {
    let mut vm = vm();
    let mut warnings = Vec::new();
    let b = Box::new(Interval::new(0.0, 1.0), Interval::new(0.0, 2.0));
    let s = lookup::round_filled_box(vm.heap_mut(), b, -0.5, &mut warnings);
    assert_eq!(s.expr, Value::NULL);
    assert_eq!(
        warnings,
        ["Not drawing a box with negative dimension, 1.00 by 2.00."]
    );
    lookup::round_filled_box(vm.heap_mut(), b, f64::NEG_INFINITY, &mut warnings);
    assert_eq!(warnings.len(), 1, "an infinite blot draws nothing silently");
    let (key, text) = run(
        &mut vm,
        "(list (ly:stencil-expr (ly:round-filled-box '(0 . 1) '(0 . 2) -0.5)) (vector-ref (caar %diagnostics) 0) (vector-ref (caar %diagnostics) 2))",
    );
    assert_eq!(
        (key.as_str(), text.as_str()),
        (
            "",
            "(() warning \"Not drawing a box with negative dimension, 1.00 by 2.00.\")"
        )
    );
}

/// `Lookup::bezier_sandwich` and `Lookup::slur` (MS3 step 54, D211): the
/// arch from (0, 0) to (3, 0) through (1, 1) and (2, 1) peaks at 0.75 at
/// its middle; the sandwich's box is the curves' widened by half its
/// thickness, and the slur's bows its upper curve by half its thickness
/// there, 0.0375 higher. The path is `(path thickness (moveto … curveto
/// … lineto … curveto … closepath) round round #t)`; a dashed slur is a
/// sandwich a dash. Their values on chord slurs are LilyPond's
/// (`tools/compare/test/layout.test.mjs`' arpeggios fixture).
#[test]
fn a_bezier_sandwich_and_a_slur_have_their_curves_extents() {
    let mut vm = vm();
    let curve = [(0.0, 0.0), (1.0, 1.0), (2.0, 1.0), (3.0, 0.0)];
    let near = |iv: Interval, want: (f64, f64)| {
        (iv.left - want.0).abs() < 1e-12 && (iv.right - want.1).abs() < 1e-12
    };
    let s = lookup::bezier_sandwich(vm.heap_mut(), curve, curve, 0.1);
    assert!(near(s.extent(0), (-0.05, 3.05)), "{:?}", s.extent(0));
    assert!(near(s.extent(1), (-0.05, 0.8)), "{:?}", s.extent(1));
    let text = expr(&vm, &s);
    assert!(
        text.starts_with("(path 0.1 (moveto 0.0 0.0 curveto 1.0 1.0 2.0 1.0 3.0 0.0 lineto 3.0 0.0 curveto 2.0 1.0 1.0 1.0 0.0 0.0 closepath) round round #t)"),
        "{text}"
    );
    let mut errors = Vec::new();
    let s = lookup::slur(vm.heap_mut(), curve, 0.1, 0.05, Value::NULL, &mut errors);
    assert!(near(s.extent(0), (-0.025, 3.025)), "{:?}", s.extent(0));
    assert!(near(s.extent(1), (-0.025, 0.8125)), "{:?}", s.extent(1));
    let dashes = vm.eval_text("'((0 1 0.4 0.75))", None).unwrap();
    let s = lookup::slur(vm.heap_mut(), curve, 0.1, 0.05, dashes, &mut errors);
    assert_eq!(expr(&vm, &s).matches("(path ").count(), 5);
    assert!(errors.is_empty(), "{errors:?}");
}

/// What the slurs read of `bezier.cc` (MS4 step 7): on the arch from
/// (0, 0) to (3, 0) through (1, 2) and (2, 2), X is 3t and Y 6t(1 - t),
/// so the other coordinate at X 1.5 is Y 1.5, the two Xs at Y 1 are those
/// of t = 1/2 ± 1/√12, and its top between 0 and 3 is 1.5; a coordinate
/// outside the curve gives none, and LilyPond's programming error. On a
/// curve whose X is a cubic of t, each solution's X is the X asked for.
#[test]
fn the_slurs_bezier_functions_solve_the_curve() {
    let arch = [(0.0, 0.0), (1.0, 2.0), (2.0, 2.0), (3.0, 0.0)];
    let mut errors = Vec::new();
    assert_eq!(
        bezier::get_other_coordinate(&arch, 0, 1.5, &mut errors),
        1.5
    );
    let mut xs = bezier::get_other_coordinates(&arch, 1, 1.0);
    xs.sort_by(f64::total_cmp);
    let d = 3.0 / 12f64.sqrt();
    assert!(
        xs.len() == 2 && (xs[0] - (1.5 - d)).abs() < 1e-12 && (xs[1] - (1.5 + d)).abs() < 1e-12,
        "{xs:?}"
    );
    assert!((bezier::minmax(&arch, 0, 0.0, 3.0, 1, &mut errors) - 1.5).abs() < 1e-12);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        bezier::get_other_coordinate(&arch, 0, 5.0, &mut errors),
        0.0
    );
    assert_eq!(errors, ["no solution found for Bezier intersection"]);

    let s = [(0.0, 0.0), (2.0, 1.0), (1.0, 2.0), (3.0, 3.0)];
    for x in [0.25, 1.0, 1.5, 2.75] {
        let ts = bezier::solve_point(&s, 0, x);
        assert!(!ts.is_empty(), "{x}");
        for t in ts {
            assert!((0.0..=1.0).contains(&t));
            assert!(
                (bezier::curve_coordinate(&s, t, 0) - x).abs() < 1e-12,
                "{x} at {t}"
            );
        }
    }
    assert_eq!(bezier::control_point_extent(&s, 1), Interval::new(0.0, 3.0));
    let mut moved = s;
    bezier::translate(&mut moved, (1.0, -1.0));
    bezier::scale(&mut moved, 2.0, 1.0);
    assert_eq!(moved[3], (8.0, 2.0));
}

/// `s`'s expression as `write` writes it.
fn expr(vm: &Vm, s: &Stencil) -> String {
    let mut out = String::new();
    vm.heap().write(s.expr, &mut out);
    out
}

#[test]
fn the_shapes_no_scheme_function_draws_are_lilypond_s() {
    let mut vm = vm();
    let mut warnings = Vec::new();
    let i = Interval::new;
    let h = vm.heap_mut();
    let shapes = [
        (
            lookup::beam(h, 0.5, 2.0, 0.5, 0.25),
            "(polygon (1.875 1.125 1.875 0.875 0.125 -0.125 0.125 0.125) 0.25 #t)",
            i(0.0, 2.0),
            i(-0.25, 1.25),
        ),
        (
            lookup::rotated_box(h, 0.0, 2.0, 0.5, 0.0),
            "(polygon (0.0 0.25 2.0 0.25 2.0 -0.25 0.0 -0.25) 0.0 #t)",
            i(0.0, 2.0),
            i(-0.25, 0.25),
        ),
        (
            lookup::horizontal_line(h, i(-1.0, 2.0), 0.5),
            "(draw-line 0.5 -1.0 0 2.0 0)",
            i(-1.0, 2.0),
            i(-0.25, 0.25),
        ),
        (
            lookup::blank(h, Box::new(i(0.0, 1.0), i(0.0, 2.0))),
            "\"\"",
            i(0.0, 1.0),
            i(0.0, 2.0),
        ),
        (
            lookup::circle(h, 0.5, 0.125, false),
            "(circle 0.5 0.125 #f)",
            i(-0.5, 0.5),
            i(-0.5, 0.5),
        ),
        (
            lookup::frame(
                h,
                Box::new(i(0.0, 2.0), i(0.0, 1.0)),
                0.5,
                0.25,
                &mut warnings,
            ),
            "(combine-stencil (round-filled-box 0.25 2.25 -0.75 1.25 0.25) (round-filled-box 0.25 2.25 0.25 0.25 0.25) (round-filled-box -1.75 2.25 0.25 1.25 0.25) (round-filled-box 0.25 0.25 0.25 1.25 0.25))",
            i(-0.25, 2.25),
            i(-0.25, 1.25),
        ),
        (
            lookup::triangle(h, i(0.0, 2.0), 0.5, 1.0),
            "(combine-stencil (draw-line 0.5 1.0 1.0 0.0 0.0) (draw-line 0.5 2.0 0.0 1.0 1.0) (draw-line 0.5 0.0 0.0 2.0 0.0))",
            i(-0.25, 2.25),
            i(-0.25, 1.25),
        ),
        (
            lookup::repeat_slash(h, 2.0, 0.75, 3.0),
            "(path 0 (moveto 0 0 rlineto 5.0 0 rlineto 2.0 1.5 rlineto -5.0 0 closepath) round round #t)",
            i(0.0, 7.0),
            i(0.0, 1.5),
        ),
        (
            lookup::points_to_line_stencil(
                h,
                0.5,
                &[(0.0, 0.0), (f64::INFINITY, 0.0), (1.0, 1.0), (2.0, 2.0)],
            ),
            "(draw-line 0.5 1.0 1.0 2.0 2.0)",
            i(0.75, 2.25),
            i(0.75, 2.25),
        ),
        (
            line::make_dashed_line(h, 0.5, (1.0, 1.0), (3.0, 1.0), 0.5, 1.5),
            "(translate-stencil (1.0 . 1.0) (dashed-line 0.5 0.5 0.0 2.0 0.0 0.0))",
            i(0.75, 3.25),
            i(0.75, 1.25),
        ),
    ];
    assert!(warnings.is_empty());
    for (s, want, x, y) in shapes {
        assert_eq!(
            (expr(&vm, &s).as_str(), s.extent(X), s.extent(Y)),
            (want, x, y)
        );
    }
    let h = vm.heap_mut();
    let style = LineStyle::new(0.25, 1.0);
    let head = line::make_arrow(h, (0.0, 0.0), (2.0, 0.0), 0.25, 1.0, 0.5);
    let polygon =
        lookup::round_polygon(h, &[(2.0, 0.0), (1.0, 0.5), (1.0, -0.5)], 0.25, -1.0, true);
    let both = line::arrows(h, &style, (0.0, 0.0), (2.0, 0.0), true, true);
    let none = line::arrows(h, &style, (0.0, 0.0), (2.0, 0.0), false, false);
    assert_eq!(expr(&vm, &head), expr(&vm, &polygon), "an arrow's head");
    assert!(expr(&vm, &both).starts_with("(combine-stencil (polygon"));
    assert!(none.is_empty());
}

/// `Offset::length` is macOS's `hypot`, which `f64::hypot` calls there:
/// equal for a million pairs of each kind.
#[cfg(target_os = "macos")]
#[test]
fn lengths_are_macos_hypot() {
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut differ = Vec::new();
    for n in 0..4_000_000u32 {
        let (x, y) = match n % 4 {
            0 => (f64::from_bits(next()), f64::from_bits(next())),
            1 => (
                (next() % 20_001) as f64 / 100.0 - 100.0,
                (next() % 20_001) as f64 / 100.0 - 100.0,
            ),
            2 => (
                next() as f64 / u64::MAX as f64 * 10.0,
                next() as f64 / u64::MAX as f64 * 3.0,
            ),
            _ => (f64::from_bits(next() >> 2), f64::from_bits(next() >> 2)),
        };
        if x.is_nan() || y.is_nan() {
            continue;
        }
        if length((x, y)).to_bits() != x.hypot(y).to_bits() && differ.len() < 5 {
            differ.push((x, y));
        }
    }
    assert!(differ.is_empty(), "{differ:?}");
    assert_eq!(length((0.36, -1.12)), 1.1764352935882194, "the pentagon's");
}
