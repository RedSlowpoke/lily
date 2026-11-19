//! The grobs' default skylines (MS3 step 26): `skylines.rs`'
//! `interpret_stencil_for_skyline` and `LazySkylinePair` on the stencils of
//! every head the walk knows.
//!
//! - [`CASES`]' stencils, made in a VM with the music crate's and layout's
//!   functions and in LilyPond 2.26.0 from the same Scheme, give LilyPond's
//!   `ly:skylines-for-stencil` on both axes, point for point and bit for
//!   bit: the walk under the identity, as `skylines_from_stencil` starts
//!   it, whose fallback to the extents no case reaches. The glyphs' cases
//!   read emmentaler-20 at staff size 20, `music-font` as the layout of a
//!   markup gets it and `(text string)` a string in Emmentaler as text
//!   (`fetaText`), from the packs `tools/packs/build.mjs` builds. Skipped
//!   without LilyPond 2.26.0, and then unverified.
//! - Hand-checked cases without LilyPond: what adds nothing, a box against
//!   `crates/skyline`'s from a box, a polygon of no blot against the box of
//!   its corners, a thick line's widening and sides, a translated box, a
//!   circle within its box, a path's curve against its control polygon.
//! - The `Grob::` callbacks of the skylines (D67): layout's, under
//!   LilyPond's names and arities, none of them the music crate's
//!   placeholders; and, on grobs a Scheme engraver makes (skipped without
//!   LilyPond 2.26.0's music types), the stencil's skylines padded and
//!   rotated, with the extents' fallback; the constructor's default
//!   skylines from the extents, pure and cross-staff; and the elements'
//!   merged at their offsets, against `crates/skyline`'s from the boxes.
//! - `skyline-scheme.cc`'s face (MS3 step 48): its primitives under
//!   LilyPond's names and arities, a skyline's values by hand,
//!   `ly:skylines-for-stencil` the walk's, `ly:make-skyline`'s checks, and
//!   the side positions' callbacks of the step under the names their
//!   errors give. `tools/bench/test/skylines.test.mjs` checks them against
//!   LilyPond 2.26.0.
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use lily_engine_layout::skylines::{
    LazySkylinePair, Orientation, SkylineObject, interpret_stencil_for_skyline,
};
use lily_engine_layout::{Transform, X, Y, font};
use lily_engine_scheme::{Value, Vm};
use lily_engine_skyline::{Axis as Horizon, Direction, SkylinePair};

/// The stencils, each a Scheme expression that LilyPond and the engine
/// both evaluate.
const CASES: &[&str] = &[
    // draw-line: no thickness, thick, upright, on a slope downwards, flat.
    "(ly:make-stencil '(draw-line 0 0 0 3 1) '(0 . 3) '(0 . 1))",
    "(ly:make-stencil '(draw-line 0.2 -1 0.5 2 -1.5) '(-1 . 2) '(-1.5 . 0.5))",
    "(ly:make-stencil '(draw-line 0.3 1 0 1 4) '(1 . 1) '(0 . 4))",
    "(ly:make-stencil '(draw-line 0.13 3 2 -0.5 0.25) '(-0.5 . 3) '(0.25 . 2))",
    "(ly:make-stencil '(draw-line 1 0 0 4 0) '(0 . 4) '(-0.5 . 0.5))",
    // dashed-line: the whole line from the origin.
    "(ly:make-stencil '(dashed-line 0.15 0.4 0.2 3 0.7 0) '(0 . 3) '(0 . 0.7))",
    // round-filled-box: plain, a blot under half a unit, rounded corners
    // of 3 and of 7 segments, scaled, rotated, rotated and rounded.
    "(ly:round-filled-box '(-1 . 2) '(0 . 1.5) 0)",
    "(ly:round-filled-box '(0 . 4) '(-0.5 . 0.5) 0.3)",
    "(ly:round-filled-box '(0 . 4) '(-0.5 . 0.5) 0.8)",
    "(ly:round-filled-box '(0 . 5) '(0 . 3) 2)",
    "(ly:stencil-scale (ly:round-filled-box '(0 . 1) '(0 . 1) 0.4) 2 3)",
    "(ly:stencil-rotate (ly:round-filled-box '(0 . 3) '(0 . 1) 0.1) 30 0 0)",
    "(ly:stencil-rotate (ly:round-filled-box '(-1 . 3) '(0 . 2) 1.2) -110 -1 1)",
    // polygon: through ly:round-polygon, with and without a blot; flat
    // coordinates and nested ones (get_number_list); an odd triangle.
    "(ly:round-polygon '((0 . 0) (3 . 1) (2 . 3) (-1 . 2)) 0.2)",
    "(ly:round-polygon '((0 . 0) (4 . 0) (4 . 1) (0 . 1)) 0)",
    "(ly:make-stencil '(polygon (0 0 2 0 1 2) 0.1 #t) '(0 . 2) '(0 . 2))",
    "(ly:make-stencil '(polygon ((0.5 -1 2.25 0.75 -0.3 1.1)) 0 #f) '(-0.3 . 2.25) '(-1 . 1.1))",
    // The walk: combined, translated, in a color, with attributes, with an
    // outline, scaled, rotated, a blank and an unknown head beside a line.
    "(ly:stencil-add (ly:stencil-translate (ly:make-stencil '(draw-line 0.1 0 0 1 1) '(0 . 1) '(0 . 1)) '(2 . 1)) (ly:make-stencil '(color (1 0 0 1) (round-filled-box 0 1 0 1 0)) '(0 . 1) '(0 . 1)))",
    "(ly:make-stencil '(output-attributes ((id . \"x\")) (draw-line 0.1 0 0 1 2)) '(0 . 1) '(0 . 2))",
    "(ly:stencil-outline (ly:make-stencil '(draw-line 0.1 0 0 3 0) '(0 . 3) '(0 . 0)) (ly:round-filled-box '(0 . 1) '(0 . 2) 0))",
    "(ly:stencil-scale (ly:make-stencil '(draw-line 0.2 0 0 1 1) '(0 . 1) '(0 . 1)) 1.5 0.5)",
    "(ly:stencil-rotate (ly:make-stencil '(draw-line 0.2 0 0 2 0) '(0 . 2) '(-0.1 . 0.1)) 45 0 0)",
    "(ly:stencil-add (ly:make-stencil \"\" '(0 . 1) '(0 . 1)) (ly:make-stencil '(embedded-ps \"0 0 moveto\") '(-1 . 1) '(-1 . 1)) (ly:make-stencil '(draw-line 0.1 0 0 1 0) '(0 . 1) '(0 . 0)))",
    // circle, ellipse: filled, thick, a small one of one segment, scaled
    // and rotated.
    "(ly:make-stencil '(circle 1 0.1 #t) '(-1 . 1) '(-1 . 1))",
    "(ly:make-stencil '(circle 0.03 0 #f) '(-0.03 . 0.03) '(-0.03 . 0.03))",
    "(ly:make-stencil '(ellipse 2 0.75 0.2 #f) '(-2 . 2) '(-0.75 . 0.75))",
    "(ly:stencil-scale (ly:make-stencil '(ellipse 1 0.5 0 #t) '(-1 . 1) '(-0.5 . 0.5)) 3 0.5)",
    "(ly:stencil-rotate (ly:make-stencil '(ellipse 1.5 0.5 0.1 #t) '(-1.5 . 1.5) '(-0.5 . 0.5)) 35 0 0)",
    // partial-ellipse: an arc from a start that is not 0, connected; filled
    // through the start; equal ends, a whole turn; flags not #t.
    "(ly:make-stencil '(partial-ellipse 2 1 30 200 0.15 #t #f) '(-2 . 2) '(-1 . 1))",
    "(ly:make-stencil '(partial-ellipse 1 1.5 -60 75 0.1 #f #t) '(-1 . 1) '(-1.5 . 1.5))",
    "(ly:make-stencil '(partial-ellipse 0.8 0.6 45 45 0 #f #f) '(-0.8 . 0.8) '(-0.6 . 0.6))",
    "(ly:make-stencil '(partial-ellipse 1 1 0 90 0.1 1 yes) '(0 . 1) '(0 . 1))",
    // path: lines and curves, absolute and relative, closed, thick; nested
    // in a list after the thickness; under a rotation; a malformed one's
    // pieces before the fault.
    "(ly:make-stencil '(path 0.1 (moveto 0 0 curveto 1 2 3 2 4 0 rlineto 0 -1 closepath) round round #t) '(0 . 4) '(-1 . 2))",
    "(ly:make-stencil '(path 0 (rmoveto 1 1 rcurveto 0.5 1 1.5 1 2 0 rcurveto 0.5 -1 1.5 -1 2 0 lineto 1 1) butt miter #f) '(1 . 5) '(0.25 . 1.75))",
    "(ly:make-stencil '(path 0.2 ((moveto -1 0 curveto -1 5 1 5 1 0)) round round #f) '(-1 . 1) '(0 . 4))",
    "(ly:stencil-rotate (ly:make-stencil '(path 0.15 (moveto 0 0 curveto 0 1 2 1 2 0 curveto 2 -1 4 -1 4 0) round round #f) '(0 . 4) '(-1 . 1)) -20 1 0)",
    // named-glyph: a note head, a clef, a rest, a flag; scaled, rotated and
    // translated; the time signature's digits beside each other.
    "(ly:font-get-glyph music-font \"noteheads.s2\")",
    "(ly:font-get-glyph music-font \"clefs.G\")",
    "(ly:font-get-glyph music-font \"rests.3\")",
    "(ly:font-get-glyph music-font \"flags.u3\")",
    "(ly:stencil-scale (ly:font-get-glyph music-font \"accidentals.sharp\") 1.5 0.75)",
    "(ly:stencil-rotate (ly:font-get-glyph music-font \"clefs.F\") 25 0 0)",
    "(ly:stencil-translate (ly:font-get-glyph music-font \"scripts.ufermata\") '(1 . 2))",
    "(ly:stencil-add (ly:font-get-glyph music-font \"three\") (ly:stencil-translate-axis (ly:font-get-glyph music-font \"four\") 1.2 0))",
    // glyph-string in Emmentaler as text: digits, a dynamic, a space (no
    // ink: its kerned box); rotated. Not two dynamics: Emmentaler's OTF
    // kerns them, its pack does not (`pango.rs`).
    "(text \"34\")",
    "(text \"f\")",
    "(text \"3 4\")",
    "(ly:stencil-rotate (text \"12\") 90 0 0)",
];

/// Whether [`CASES`]' `case` is a circle's or an ellipse's, whose points
/// are `offset_directed`'s, where the `libm` crate's `sin` is an ulp from
/// that of macOS, whose LilyPond made the references (4 % of their
/// angles), and an ulp may add or drop a building of no width: those are
/// compared by [`same_heights`], the others bit for bit. With the system's
/// `sin` in `offset_directed` they are equal bit for bit too (MS3 step 26).
fn by_libm_sin(case: &str) -> bool {
    case.contains("(circle ") || case.contains("ellipse ")
}

/// Whether the skylines whose points are `a` and `b`, flattened, the
/// horizon's coordinate the `along`th of each, have the same heights to
/// 1e-9: at their first and last finite points along the horizon, and
/// midway between each two of the points of either that are 1e-9 apart or
/// more, so that a step an ulp from the other's is not measured inside.
fn same_heights(a: &[f64], b: &[f64], along: usize) -> bool {
    let points = |p: &[f64]| -> Vec<(f64, f64)> {
        p.chunks_exact(2)
            .map(|c| (c[along], c[1 - along]))
            .collect()
    };
    let (a, b) = (points(a), points(b));
    // The height at `h`, between two points: the line between them.
    let height = |p: &[(f64, f64)], h: f64| {
        p.windows(2).find(|w| w[0].0 < h && h < w[1].0).map(|w| {
            let t = (h - w[0].0) / (w[1].0 - w[0].0);
            w[0].1 + t * (w[1].1 - w[0].1)
        })
    };
    let finite = |p: &[(f64, f64)]| -> Vec<f64> {
        p.iter().map(|q| q.0).filter(|h| h.is_finite()).collect()
    };
    let (fa, fb) = (finite(&a), finite(&b));
    let close = |x: f64, y: f64| x == y || (x - y).abs() < 1e-9;
    if fa.is_empty() || fb.is_empty() {
        return fa.is_empty() && fb.is_empty();
    }
    if !close(fa[0], fb[0]) || !close(fa[fa.len() - 1], fb[fb.len() - 1]) {
        return false;
    }
    let mut hs: Vec<f64> = fa.into_iter().chain(fb).collect();
    hs.sort_by(f64::total_cmp);
    hs.dedup();
    hs.windows(2).filter(|w| w[1] - w[0] >= 1e-9).all(|w| {
        let h = (w[0] + w[1]) / 2.0;
        match (height(&a, h), height(&b, h)) {
            (Some(x), Some(y)) => close(x, y),
            _ => false,
        }
    })
}

/// The stencil heads the cases write themselves.
const HEADS: &[&str] = &[
    "draw-line",
    "dashed-line",
    "round-filled-box",
    "polygon",
    "color",
    "output-attributes",
    "circle",
    "ellipse",
    "partial-ellipse",
    "path",
    "embedded-ps",
];

/// The packs [`CASES`]' glyphs read.
const PACKS: [&str; 1] = ["emmentaler-20"];

/// What the glyphs' cases are made with in the engine, as a markup's
/// layout and properties at staff size 20 give them in LilyPond: `paper`
/// as `paper-book.cc` scales a book's, `props` with the music font's family,
/// and `feta-design-size-mapping` as `lily-library.scm` defines it.
const FONTS: &str = "(define-public feta-design-size-mapping
       '((11 . 11.22) (13 . 12.60) (14 . 14.14) (16 . 15.87) (18 . 17.82) (20 . 20)
         (23 . 22.45) (26 . 25.20)))
     (define staff-height (* 20 (ly:pt 1)))
     (define paper (ly:make-output-def))
     (ly:output-def-set-variable! paper 'staff-height staff-height)
     (ly:output-def-set-variable! paper 'output-scale (/ staff-height 4))
     (ly:output-def-set-variable! paper 'text-font-size 11.0)
     (define props '(((fonts (music . \"emmentaler\")))))
     (define music-font (ly:paper-get-font paper (cons '((font-encoding . fetaMusic)) props)))
     (define (text s)
       (ly:text-interface::interpret-markup paper (cons '((font-encoding . fetaText)) props) s))";

/// The packs `build.mjs` built, once; `None` without LilyPond 2.26.0.
fn packs() -> Option<&'static Path> {
    static BUILT: OnceLock<Option<PathBuf>> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let version = Command::new("lilypond").arg("--version").output().ok()?;
            if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
                return None;
            }
            let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("layout-skylines-packs");
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).unwrap();
            let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            let run = Command::new("node")
                .arg(engine.join("tools/packs/build.mjs"))
                .arg("--out")
                .arg(&dir)
                .output()
                .expect("node runs tools/packs/build.mjs");
            assert!(
                run.status.success(),
                "tools/packs/build.mjs failed: {}",
                String::from_utf8_lossy(&run.stderr)
            );
            Some(dir)
        })
        .as_deref()
}

/// [`vm`] given [`PACKS`] from `packs`, with [`FONTS`].
fn vm_with_fonts(packs: &Path) -> Vm {
    let mut vm = vm();
    for pack in PACKS {
        let bytes = std::fs::read(packs.join(format!("{pack}.pack"))).unwrap();
        font::put_pack(&mut vm, &bytes).unwrap_or_else(|e| panic!("{pack}: {}", e.text(vm.heap())));
    }
    vm.eval_text(FONTS, None)
        .unwrap_or_else(|e| panic!("{}", e.text(vm.heap())));
    vm
}

/// A VM with the music crate's and layout's functions, `(lily)` current,
/// [`HEADS`] registered.
fn vm() -> Vm {
    let mut vm = Vm::new();
    lily_engine_music::install(&mut vm).unwrap();
    lily_engine_layout::install(&mut vm).unwrap();
    vm.eval_text("(set-current-module (resolve-module '(lily)))", None)
        .unwrap();
    for head in HEADS {
        vm.eval_text(&format!("(ly:register-stencil-expression '{head})"), None)
            .unwrap();
    }
    vm
}

/// The skylines of `expr` along `horizon` (`X` or `Y`), from the identity.
fn skylines(vm: &Vm, expr: Value, horizon: usize) -> (SkylinePair, Vec<String>) {
    let mut lazy = LazySkylinePair::new(horizon);
    let mut warnings = Vec::new();
    interpret_stencil_for_skyline(
        vm.heap(),
        &mut lazy,
        &Transform::IDENTITY,
        expr,
        &mut warnings,
    );
    assert!(!lazy.is_empty(), "the case adds segments");
    (lazy.to_pair(), warnings)
}

/// The expression of the stencil `source` makes.
fn expr(vm: &mut Vm, source: &str) -> Value {
    vm.eval_text(&format!("(ly:stencil-expr {source})"), None)
        .unwrap_or_else(|e| panic!("{source}: {}", e.text(vm.heap())))
}

/// One line per case, axis and side: `case axis side x y …`, after the
/// music font's magnification, `font magnification - m`.
fn lines(vm: &mut Vm) -> Vec<(String, Vec<f64>)> {
    let m = vm
        .eval_text("(ly:font-magnification music-font)", None)
        .unwrap();
    let mut out = vec![(
        String::from("font magnification -"),
        vec![vm.heap().to_f64(m).unwrap()],
    )];
    for (i, case) in CASES.iter().enumerate() {
        let e = expr(vm, case);
        for (axis, name, horizon) in [(X, "X", Horizon::X), (Y, "Y", Horizon::Y)] {
            let (pair, warnings) = skylines(vm, e, axis);
            assert_eq!(warnings, Vec::<String>::new());
            for (side, sky) in [("down", Direction::Down), ("up", Direction::Up)] {
                let numbers = pair[sky].to_points(horizon).into_iter().flatten().collect();
                out.push((format!("{i} {name} {side}"), numbers));
            }
        }
    }
    out
}

/// LilyPond's lines for [`CASES`], or `None` without LilyPond 2.26.0.
fn lilypond() -> Option<Vec<(String, Vec<f64>)>> {
    let version = Command::new("lilypond").arg("--version").output().ok()?;
    if !String::from_utf8_lossy(&version.stdout).contains("GNU LilyPond 2.26.0") {
        return None;
    }
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("layout-skylines");
    std::fs::create_dir_all(&dir).unwrap();
    let mut ly = String::from(
        "\\version \"2.26.0\"\n\
         #(define (probe-case i stil)\n\
         \x20  (for-each\n\
         \x20    (lambda (axis name)\n\
         \x20      (let ((pair (ly:skylines-for-stencil stil axis)))\n\
         \x20        (for-each\n\
         \x20          (lambda (side sky)\n\
         \x20            (display (format #f \"~a ~a ~a\" i name side))\n\
         \x20            (for-each (lambda (p) (display \" \") (display (car p))\n\
         \x20                                  (display \" \") (display (cdr p)))\n\
         \x20                      (ly:skyline->points sky axis))\n\
         \x20            (newline))\n\
         \x20          '(\"down\" \"up\") (list (car pair) (cdr pair)))))\n\
         \x20    (list X Y) '(\"X\" \"Y\")))\n",
    );
    // In a markup's layout and properties, which scale the fonts.
    ly.push_str(
        "#(define-markup-command (probe layout props) ()\n\
         \x20  (let* ((music-font (ly:paper-get-font layout (cons '((font-encoding . fetaMusic)) props)))\n\
         \x20         (text (lambda (s) (ly:text-interface::interpret-markup\n\
         \x20                             layout (cons '((font-encoding . fetaText)) props) s))))\n\
         \x20    (display \"font magnification - \")\n\
         \x20    (display (ly:font-magnification music-font))\n\
         \x20    (newline)\n",
    );
    for (i, case) in CASES.iter().enumerate() {
        ly.push_str(&format!("    (probe-case {i} {case})\n"));
    }
    ly.push_str("    empty-stencil))\n\\markup \\probe\n");
    let file = dir.join("skylines.ly");
    std::fs::write(&file, ly).unwrap();
    let out = Command::new("lilypond")
        .args(["-s", "-dno-print-pages"])
        .arg(&file)
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parse = |t: &str| match t {
        "+inf.0" => f64::INFINITY,
        "-inf.0" => f64::NEG_INFINITY,
        _ => t.parse().unwrap_or_else(|_| panic!("a number: {t}")),
    };
    Some(
        String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .map(|line| {
                let words: Vec<&str> = line.split(' ').collect();
                (
                    words[..3].join(" "),
                    words[3..].iter().map(|t| parse(t)).collect(),
                )
            })
            .collect(),
    )
}

#[test]
fn the_cases_give_lilypond_s_skylines() {
    let Some(want) = lilypond() else {
        eprintln!("skipped: LilyPond 2.26.0 is not installed; unverified");
        return;
    };
    let got = lines(&mut vm_with_fonts(packs().unwrap()));
    assert_eq!(got.len(), 1 + 4 * CASES.len());
    assert_eq!(want.len(), got.len(), "{want:?}");
    let mut failures = Vec::new();
    for ((key, numbers), (want_key, want_numbers)) in got.iter().zip(&want) {
        assert_eq!(key, want_key);
        let case = key
            .split(' ')
            .next()
            .unwrap()
            .parse::<usize>()
            .map_or("", |i| CASES[i]);
        let equal = if by_libm_sin(case) {
            let along = usize::from(key.split(' ').nth(1) == Some("Y"));
            same_heights(numbers, want_numbers, along)
        } else {
            numbers == want_numbers
        };
        if !equal {
            failures.push(format!(
                "{key}: {case}\n  engine   {numbers:?}\n  lilypond {want_numbers:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The down and up points of `pair` along `horizon` where the skylines
/// have height: without the ends at the infinities.
fn roofs(pair: &SkylinePair, horizon: Horizon) -> [Vec<[f64; 2]>; 2] {
    [Direction::Down, Direction::Up].map(|d| {
        let mut points = pair[d].to_points(horizon);
        points.retain(|p| p.iter().all(|x| x.is_finite()));
        points
    })
}

/// [`roofs`] along X.
fn points(pair: &SkylinePair) -> [Vec<[f64; 2]>; 2] {
    roofs(pair, Horizon::X)
}

#[test]
fn nothing_adds_no_segments() {
    let mut vm = vm();
    for source in [
        "'()",
        "\"\"",
        "'(path 0.1 (1 2 3) round round #t)",
        "'(named-glyph #f \"noteheads.s2\")",
        "'(glyph-string #f \"F\" 1.0 #f ((1 (0 . 1) 0 0 3 \"x\")))",
        "'(round-filled-box 1 -2 0 1 0)",
        "'(combine-stencil (translate-stencil (1 . 2) \"\") ())",
        "'(polygon (sym) 0.1 #t)",
        "42",
    ] {
        let e = vm.eval_text(source, None).unwrap();
        let mut lazy = LazySkylinePair::new(X);
        let mut warnings = Vec::new();
        interpret_stencil_for_skyline(vm.heap(), &mut lazy, &Transform::IDENTITY, e, &mut warnings);
        assert!(lazy.is_empty(), "{source}");
        assert!(warnings.is_empty(), "{source}");
        assert_eq!(lazy.to_pair(), SkylinePair::default(), "{source}");
    }
}

#[test]
fn a_box_is_its_contour() {
    let mut vm = vm();
    let e = expr(&mut vm, "(ly:round-filled-box '(-1 . 2) '(0.5 . 1.5) 0.1)");
    let b = lily_engine_fonts::Box {
        x: lily_engine_fonts::Interval {
            left: -1.0,
            right: 2.0,
        },
        y: lily_engine_fonts::Interval {
            left: 0.5,
            right: 1.5,
        },
    };
    for (axis, horizon) in [(X, Horizon::X), (Y, Horizon::Y)] {
        let (pair, _) = skylines(&vm, e, axis);
        assert_eq!(
            [Direction::Down, Direction::Up].map(|d| pair[d].to_points(horizon)),
            [Direction::Down, Direction::Up]
                .map(|d| SkylinePair::from_box(b, horizon)[d].to_points(horizon)),
        );
    }
    assert_eq!(
        points(&skylines(&vm, e, X).0),
        [vec![[-1.0, 0.5], [2.0, 0.5]], vec![[-1.0, 1.5], [2.0, 1.5]],]
    );
}

#[test]
fn a_polygon_without_blot_is_bounded_by_its_sides() {
    let mut vm = vm();
    let square = expr(
        &mut vm,
        "(ly:round-polygon '((0 . 0) (4 . 0) (4 . 1) (0 . 1)) 0)",
    );
    let b = expr(&mut vm, "(ly:round-filled-box '(0 . 4) '(0 . 1) 0)");
    assert_eq!(
        points(&skylines(&vm, square, X).0),
        points(&skylines(&vm, b, X).0)
    );
}

#[test]
fn a_thick_line_is_widened_and_moved_to_each_side() {
    let mut vm = vm();
    let e = expr(
        &mut vm,
        "(ly:make-stencil '(draw-line 1 0 0 4 0) '(0 . 4) '(-0.5 . 0.5))",
    );
    assert_eq!(
        points(&skylines(&vm, e, X).0),
        [
            vec![[-0.5, -0.5], [4.5, -0.5]],
            vec![[-0.5, 0.5], [4.5, 0.5]],
        ]
    );
    // Along Y the line has no length along the horizon: widening its ends
    // along Y makes it a diagonal, moved left and right by the half
    // thickness.
    assert_eq!(
        roofs(&skylines(&vm, e, Y).0, Horizon::Y),
        [
            vec![[-0.5, -0.5], [3.5, 0.5]],
            vec![[0.5, -0.5], [4.5, 0.5]],
        ]
    );
}

#[test]
fn a_translated_box_is_the_box_moved() {
    let mut vm = vm();
    let moved = expr(
        &mut vm,
        "(ly:stencil-translate (ly:round-filled-box '(0 . 1) '(0 . 2) 0) '(3 . -1))",
    );
    let b = expr(&mut vm, "(ly:round-filled-box '(3 . 4) '(-1 . 1) 0)");
    assert_eq!(
        points(&skylines(&vm, moved, X).0),
        points(&skylines(&vm, b, X).0)
    );
}

#[test]
fn a_contour_segment_bounds_one_side() {
    // Along X a clockwise contour's segment running right is the top's;
    // along Y one running up is the left's, one running down the right's.
    let id = Transform::IDENTITY;
    let mut lazy = LazySkylinePair::new(X);
    lazy.add_contour_segment(&id, Orientation::Cw, (0.0, 1.0), (2.0, 1.0));
    let pair = lazy.to_pair();
    assert!(pair[Direction::Down].is_empty());
    assert_eq!(points(&pair)[1], vec![[0.0, 1.0], [2.0, 1.0]]);

    let mut lazy = LazySkylinePair::new(Y);
    lazy.add_contour_segment(&id, Orientation::Cw, (0.0, 0.0), (0.0, 2.0));
    lazy.add_contour_segment(&id, Orientation::Cw, (3.0, 2.0), (3.0, 0.0));
    assert_eq!(
        roofs(&lazy.to_pair(), Horizon::Y),
        [vec![[0.0, 0.0], [0.0, 2.0]], vec![[3.0, 0.0], [3.0, 2.0]]]
    );
}

#[test]
fn a_circle_lies_within_its_box() {
    // Its segments are chords of the circle, the thickness widening them:
    // the skyline rises no higher than the radius and half the thickness,
    // and reaches the radius at the top, a point of the quantization.
    let mut vm = vm();
    let e = vm.eval_text("'(circle 1 0.1 #t)", None).unwrap();
    let (pair, _) = skylines(&vm, e, X);
    let [down, up] = points(&pair);
    let top = up.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    let bottom = down.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    assert!(top <= 1.05 && top > 1.0, "{top}");
    assert!((-1.05..-1.0).contains(&bottom), "{bottom}");
    assert!(up.iter().chain(&down).all(|p| p[0].abs() <= 1.05 + 1e-12));
}

#[test]
fn a_path_s_curve_lies_within_its_control_polygon() {
    // No thickness: each segment's ends are on the curve, under its
    // control points' hull, and the curve's ends are the path's.
    let mut vm = vm();
    let e = vm
        .eval_text(
            "'(path 0 (moveto 0 0 curveto 1 2 3 2 4 0) round round #f)",
            None,
        )
        .unwrap();
    let (pair, _) = skylines(&vm, e, X);
    let [_, up] = points(&pair);
    assert_eq!(up.first(), Some(&[0.0, 0.0]));
    assert_eq!(up.last(), Some(&[4.0, 0.0]));
    // The curve's peak is 1.5 at x = 2, a point of its 22 segments' 11th.
    let top = up.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    assert!((top - 1.5).abs() < 1e-12, "{top}");
    assert!(up.iter().all(|p| p[1] <= 1.5 + 1e-12 && p[1] >= 0.0));
}

// --- the Grob:: callbacks of the skylines -------------------------------------

/// The ten `Grob::` callbacks of `stencil-integral.cc` and their arities:
/// eight were the music crate's placeholders, the two non-pure ones from
/// the extents are the constructor's defaults.
const CALLBACKS: [(&str, usize); 10] = [
    ("ly:grob::vertical-skylines-from-stencil", 1),
    ("ly:grob::horizontal-skylines-from-stencil", 1),
    ("ly:grob::simple-vertical-skylines-from-extents", 1),
    ("ly:grob::simple-horizontal-skylines-from-extents", 1),
    ("ly:grob::pure-simple-vertical-skylines-from-extents", 3),
    ("ly:grob::pure-simple-horizontal-skylines-from-extents", 3),
    ("ly:grob::vertical-skylines-from-element-stencils", 1),
    ("ly:grob::horizontal-skylines-from-element-stencils", 1),
    ("ly:grob::pure-vertical-skylines-from-element-stencils", 3),
    ("ly:grob::pure-horizontal-skylines-from-element-stencils", 3),
];

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

#[test]
fn the_callbacks_are_layout_s_under_lilypond_s_names_and_arities() {
    let mut vm = vm();
    let music = lily_engine_music::EXTENSION.prims[1];
    for (name, arity) in CALLBACKS {
        assert!(
            music.iter().all(|p| p.name != name),
            "{name} is no placeholder"
        );
        let args = vec!["'x"; arity].join(" ");
        let got = caught(&mut vm, &format!("({name} {args})"));
        assert!(
            got.contains(name)
                && got.contains("Wrong type argument in position 1 (expecting Grob): x"),
            "{name}: {got}"
        );
        for wrong in [arity - 1, arity + 1] {
            let args = vec!["'x"; wrong].join(" ");
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(
                got.contains("Wrong number of arguments"),
                "{name} of {wrong}: {got}"
            );
        }
    }
    assert_eq!(caught(&mut vm, "(ly:skyline? 'x)"), "#f");
}

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

fn eval(vm: &mut Vm, text: &str) -> Value {
    vm.eval_text(text, None)
        .unwrap_or_else(|e| panic!("{text}: {}", e.text(vm.heap())))
}

/// Global with the `Thing`'s description, an item, and a Voice whose
/// `Thing_maker` makes seven of each note.
const DEFS: &[&str] = &[
    "((context-name Global) (accepts Score) (default-child Score)
      (grob-descriptions
       ((Thing (meta (name . Thing) (classes Item) (interfaces grob-interface item-interface))))))",
    "((context-name Score) (translator-type Score_engraver) (accepts Voice)
      (default-child Voice))",
    "((context-name Voice) (translator-type Engraver_group) (consists Thing_maker))",
];

/// [`vm`] with the music types, translated over one note: `(g n)` the
/// `n`th of its seven `Thing`s, `box` a stencil of the box (0, 1) × (0, 2),
/// and the warnings kept in `seen` (`(seen!)` empties it). `None` without
/// LilyPond 2.26.0's music types: skipped, and then unverified.
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
                         (do ((i 0 (1+ i))) ((= i 7))
                           (set! made (append made (list (ly:engraver-make-grob t 'Thing heard)))))
                         (set! heard #f)))))))
           'Thing_maker)",
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
        "(define (g n) (list-ref made n))
         (define seen '())
         (define (seen!) (let ((s seen)) (set! seen '()) s))
         (set! ly:warning (lambda (str . rest) (set! seen (append seen (list (apply format #f str rest))))))
         (define box (ly:round-filled-box '(0 . 1) '(0 . 2) 0))",
    );
    Some(vm)
}

/// The skylines of the value `v`, a pair of two.
fn pair_of(vm: &Vm, v: Value) -> SkylinePair {
    let heap = vm.heap();
    let side = |s: Option<Value>| {
        heap.foreign::<SkylineObject>(s.expect("a pair"))
            .expect("a skyline")
            .0
            .clone()
    };
    SkylinePair::new(side(heap.car(v)), side(heap.cdr(v)))
}

/// The skylines `text` gives.
fn pair(vm: &mut Vm, text: &str) -> SkylinePair {
    let v = eval(vm, text);
    pair_of(vm, v)
}

/// Each side's points along `horizon`, the infinities included.
fn sides(pair: &SkylinePair, horizon: Horizon) -> [Vec<[f64; 2]>; 2] {
    [Direction::Down, Direction::Up].map(|d| pair[d].to_points(horizon))
}

fn boxed(x: (f64, f64), y: (f64, f64)) -> lily_engine_fonts::Box {
    let i = |(left, right)| lily_engine_fonts::Interval { left, right };
    lily_engine_fonts::Box { x: i(x), y: i(y) }
}

/// `pair` with each side padded by `r`.
fn padded(pair: &SkylinePair, r: f64) -> SkylinePair {
    SkylinePair::new(pair.down.padded(r), pair.up.padded(r))
}

#[test]
fn a_grob_s_stencil_gives_its_skylines() {
    let Some(mut vm) = grobs_vm() else { return };
    let b = boxed((0.0, 1.0), (0.0, 2.0));
    eval(&mut vm, "(%grob-set-property-data! (g 0) 'stencil box)");
    for (callback, padding, horizon) in [
        ("vertical", "skyline-horizontal-padding", Horizon::X),
        ("horizontal", "skyline-vertical-padding", Horizon::Y),
    ] {
        let call = format!("(ly:grob::{callback}-skylines-from-stencil (g 0))");
        let plain = SkylinePair::from_box(b, horizon);
        assert_eq!(
            sides(&pair(&mut vm, &call), horizon),
            sides(&plain, horizon)
        );
        // Padded by the grob's padding; a padding below 0 warns on each
        // side and pads nothing.
        eval(
            &mut vm,
            &format!("(%grob-set-property-data! (g 0) '{padding} 0.5)"),
        );
        assert_eq!(
            sides(&pair(&mut vm, &call), horizon),
            sides(&padded(&plain, 0.5), horizon)
        );
        eval(
            &mut vm,
            &format!("(%grob-set-property-data! (g 0) '{padding} -1)"),
        );
        assert_eq!(
            sides(&pair(&mut vm, &call), horizon),
            sides(&plain, horizon)
        );
        assert_eq!(
            caught(&mut vm, "(seen!)"),
            "(\"Cannot have negative horizon padding.  Junking.\" \"Cannot have negative horizon padding.  Junking.\")"
        );
        eval(
            &mut vm,
            &format!("(%grob-set-property-data! (g 0) '{padding} 'none)"),
        );
        assert_eq!(
            sides(&pair(&mut vm, &call), horizon),
            sides(&plain, horizon)
        );
    }
    // Rotated by the grob's rotation, about a point relative to its extents.
    eval(
        &mut vm,
        "(%grob-set-property-data! (g 0) 'rotation '(30 1 -1))",
    );
    let rotated = expr(&mut vm, "(ly:stencil-rotate box 30 1 -1)");
    for (callback, axis, horizon) in [("vertical", X, Horizon::X), ("horizontal", Y, Horizon::Y)] {
        let call = format!("(ly:grob::{callback}-skylines-from-stencil (g 0))");
        assert_eq!(
            sides(&pair(&mut vm, &call), horizon),
            sides(&skylines(&vm, rotated, axis).0, horizon)
        );
    }
    // A stencil that adds no segment has its extents' box, unrotated.
    eval(
        &mut vm,
        "(%grob-set-property-data! (g 1) 'stencil
           (ly:make-stencil '(embedded-ps \"0 0 moveto\") '(-1 . 1) '(0 . 3)))
         (%grob-set-property-data! (g 1) 'rotation '(45 0 0))",
    );
    assert_eq!(
        sides(
            &pair(&mut vm, "(ly:grob::vertical-skylines-from-stencil (g 1))"),
            Horizon::X
        ),
        sides(
            &SkylinePair::from_box(boxed((-1.0, 1.0), (0.0, 3.0)), Horizon::X),
            Horizon::X
        )
    );
    // No stencil, or one empty on Y (LilyPond's empty interval, not just
    // any whose left is right of its right): no skylines.
    for stencil in ["'()", "(ly:make-stencil '(embedded-ps \"\") '(0 . 1))"] {
        eval(
            &mut vm,
            &format!("(%grob-set-property-data! (g 2) 'stencil {stencil})"),
        );
        let got = pair(&mut vm, "(ly:grob::vertical-skylines-from-stencil (g 2))");
        assert!(got.is_empty(), "{stencil}");
        assert_eq!(caught(&mut vm, "(seen!)"), "()");
    }
    assert_eq!(
        caught(
            &mut vm,
            "(ly:skyline? (car (ly:grob::vertical-skylines-from-stencil (g 0))))"
        ),
        "#t"
    );
}

#[test]
fn the_constructor_gives_a_grob_the_skylines_of_its_extents() {
    let Some(mut vm) = grobs_vm() else { return };
    let inf = f64::INFINITY;
    eval(
        &mut vm,
        "(%grob-set-property-data! (g 0) 'X-extent '(0 . 2))
         (%grob-set-property-data! (g 0) 'Y-extent '(-1 . 1))",
    );
    for sym in ["vertical-skylines", "horizontal-skylines"] {
        assert_eq!(
            caught(
                &mut vm,
                &format!("(ly:unpure-pure-container? (%grob-property-data (g 0) '{sym}))")
            ),
            "#t"
        );
    }
    let b = boxed((0.0, 2.0), (-1.0, 1.0));
    for (sym, horizon) in [
        ("vertical-skylines", Horizon::X),
        ("horizontal-skylines", Horizon::Y),
    ] {
        let got = pair(&mut vm, &format!("(ly:grob-property (g 0) '{sym})"));
        assert_eq!(
            sides(&got, horizon),
            sides(&SkylinePair::from_box(b, horizon), horizon)
        );
    }
    // Pure, the box is unbounded along X: widths are not known before line
    // breaking.
    assert_eq!(
        sides(
            &pair(
                &mut vm,
                "(ly:grob::pure-simple-vertical-skylines-from-extents (g 0) 0 0)"
            ),
            Horizon::X
        ),
        sides(
            &SkylinePair::from_box(boxed((-inf, inf), (-1.0, 1.0)), Horizon::X),
            Horizon::X
        )
    );
    assert_eq!(
        sides(
            &pair(
                &mut vm,
                "(ly:grob::pure-simple-horizontal-skylines-from-extents (g 0) 'x 'y)"
            ),
            Horizon::Y
        ),
        sides(&SkylinePair::from_box(b, Horizon::Y), Horizon::Y)
    );
    // A cross-staff grob's is unbounded along Y.
    eval(&mut vm, "(%grob-set-property-data! (g 0) 'cross-staff #t)");
    for call in [
        "(ly:grob::simple-horizontal-skylines-from-extents (g 0))",
        "(ly:grob::pure-simple-horizontal-skylines-from-extents (g 0) 0 1)",
    ] {
        assert_eq!(
            sides(&pair(&mut vm, call), Horizon::Y),
            sides(
                &SkylinePair::from_box(boxed((0.0, 2.0), (-inf, inf)), Horizon::Y),
                Horizon::Y
            ),
            "{call}"
        );
    }
    // An empty extent gives none; a grob's own skylines are kept.
    eval(
        &mut vm,
        "(%grob-set-property-data! (g 1) 'X-extent '(1 . -1))
         (%grob-set-property-data! (g 1) 'Y-extent '(0 . 1))",
    );
    assert!(pair(&mut vm, "(ly:grob-property (g 1) 'vertical-skylines)").is_empty());
    assert!(
        pair(
            &mut vm,
            "(ly:grob::simple-vertical-skylines-from-extents (g 1))"
        )
        .is_empty()
    );
    assert_eq!(
        caught(
            &mut vm,
            "(begin (%grob-set-property-data! (g 2) 'vertical-skylines 'mine) (ly:grob-property (g 2) 'vertical-skylines))"
        ),
        "mine"
    );
}

#[test]
fn the_elements_skylines_merge_at_their_offsets() {
    let Some(mut vm) = grobs_vm() else { return };
    let inf = f64::INFINITY;
    // (g 0) holds (g 1) at (1.5, 2) and (g 2) at (-1, 0.5), and (g 3),
    // whose skylines are none.
    eval(
        &mut vm,
        "(for-each (lambda (n x y)
                     (ly:grob-set-parent! (g n) 0 (g 0))
                     (ly:grob-set-parent! (g n) 1 (g 0))
                     (%grob-set-property-data! (g n) 'X-offset x)
                     (%grob-set-property-data! (g n) 'Y-offset y))
                   '(1 2 3) '(1.5 -1 0) '(2 0.5 0))
         (%grob-set-property-data! (g 1) 'stencil box)
         (%grob-set-property-data! (g 2) 'stencil (ly:round-filled-box '(0 . 2) '(0 . 0.5) 0))
         (%grob-set-property-data! (g 3) 'vertical-skylines 42)
         (%grob-set-property-data! (g 3) 'horizontal-skylines 42)
         (ly:grob-set-object! (g 0) 'elements (ly:grob-list->grob-array (list (g 1) (g 2) (g 3))))",
    );
    let elements = [
        (boxed((0.0, 1.0), (0.0, 2.0)), (1.5, 2.0)),
        (boxed((0.0, 2.0), (0.0, 0.5)), (-1.0, 0.5)),
    ];
    let merged = |horizon: Horizon, pure: bool| {
        let mut res = SkylinePair::default();
        for (b, (x, y)) in elements {
            let b = if pure {
                boxed((-inf, inf), (b.y.left, b.y.right))
            } else {
                b
            };
            let mut p = SkylinePair::from_box(b, horizon);
            let (along, across) = if horizon == Horizon::X {
                (x, y)
            } else {
                (y, x)
            };
            p.shift(along);
            p.raise(across);
            res.merge(&p);
        }
        res
    };
    // Pure, each element's pure skylines at its pure height; first, as an
    // element's skylines are kept once called.
    let call = "(ly:grob::pure-vertical-skylines-from-element-stencils (g 0) 0 0)";
    assert_eq!(
        sides(&pair(&mut vm, call), Horizon::X),
        sides(&merged(Horizon::X, true), Horizon::X)
    );
    for (callback, horizon) in [("vertical", Horizon::X), ("horizontal", Horizon::Y)] {
        let call = format!("(ly:grob::{callback}-skylines-from-element-stencils (g 0))");
        assert_eq!(
            sides(&pair(&mut vm, &call), horizon),
            sides(&merged(horizon, false), horizon),
            "{call}"
        );
    }
    // A grob of no elements has none; skylines the wrong way round are
    // LilyPond's error.
    assert!(
        pair(
            &mut vm,
            "(ly:grob::vertical-skylines-from-element-stencils (g 4))"
        )
        .is_empty()
    );
    eval(
        &mut vm,
        "(let ((s (ly:grob::vertical-skylines-from-stencil (g 1))))
           (%grob-set-property-data! (g 3) 'vertical-skylines (cons (cdr s) (car s))))",
    );
    let got = caught(
        &mut vm,
        "(ly:grob::vertical-skylines-from-element-stencils (g 0))",
    );
    assert!(
        got.contains("direction of first skyline in skyline pair must be DOWN/LEFT"),
        "{got}"
    );
}

/// skyline-scheme.cc's `ly:` functions and their arities, as LilyPond's
/// `#<procedure …>` prints them: required, then optional.
const SKYLINE_FACE: &[(&str, usize, usize)] = &[
    ("ly:make-skyline", 3, 0),
    ("ly:skyline-distance", 2, 1),
    ("ly:skyline-empty?", 1, 0),
    ("ly:skyline-height", 2, 0),
    ("ly:skyline-max-height", 1, 0),
    ("ly:skyline-max-height-position", 1, 0),
    ("ly:skyline-merge", 2, 0),
    ("ly:skyline-pad", 2, 0),
    ("ly:skyline-touching-point", 2, 1),
    ("ly:skylines-for-stencil", 2, 0),
    ("ly:skyline->points", 2, 0),
];

#[test]
fn the_skylines_scheme_face_is_skyline_scheme_cc_s() {
    let mut vm = vm();
    for &(name, required, optional) in SKYLINE_FACE {
        for wrong in [required - 1, required + optional + 1] {
            let args = vec!["'x"; wrong].join(" ");
            let got = caught(&mut vm, &format!("({name} {args})"));
            assert!(
                got.contains("Wrong number of arguments"),
                "{name} of {wrong}: {got}"
            );
        }
    }
    // (lily.scm's X and UP are 0 and 1.) A roof from (0, 0) up to (2, 2), flat to 3: its height, maximum and
    // where it is reached, the last end at that height; a skyline bounding
    // things above it at 1 is 1 from it, the last place being 3.
    vm.eval_text(
        "(define up (ly:make-skyline '((0 0 2 2) (2 2 3 2)) 0 1))
         (define down (ly:make-skyline '((-inf.0 1 +inf.0 1)) 0 -1))",
        None,
    )
    .unwrap();
    assert_eq!(caught(&mut vm, "(ly:skyline-height up 1)"), "1.0");
    assert_eq!(caught(&mut vm, "(ly:skyline-max-height up)"), "2.0");
    assert_eq!(
        caught(&mut vm, "(ly:skyline-max-height-position up)"),
        "3.0"
    );
    assert_eq!(caught(&mut vm, "(ly:skyline-distance up down)"), "1.0");
    assert_eq!(
        caught(&mut vm, "(ly:skyline-touching-point up down)"),
        "3.0"
    );
    assert_eq!(caught(&mut vm, "(ly:skyline-empty? up)"), "#f");
    assert_eq!(
        caught(
            &mut vm,
            "(ly:skyline->points (ly:skyline-merge up (ly:make-skyline '((1 3 1.5 3)) 0 1)) 0)"
        ),
        "((-inf.0 . -inf.0) (0.0 . -inf.0) (0.0 . 0.0) (1.0 . 1.0) (1.0 . 3.0) (1.5 . 3.0) \
         (1.5 . 1.5) (2.0 . 2.0) (2.0 . 2.0) (3.0 . 2.0) (3.0 . -inf.0) (+inf.0 . -inf.0))"
    );
    // The walk's skylines, unpadded, along either axis.
    let stencil = "(ly:round-filled-box '(0 . 3) '(0 . 2) 0.5)";
    for (axis, horizon) in [("0", X), ("1", Y)] {
        let v = vm
            .eval_text(&format!("(ly:skylines-for-stencil {stencil} {axis})"), None)
            .unwrap();
        let e = expr(&mut vm, stencil);
        assert_eq!(pair_of(&vm, v), skylines(&vm, e, horizon).0, "{axis}");
    }
    // The checks, LilyPond's own words; a height at an infinity, where
    // LilyPond asserts, the engine's out-of-range.
    for (call, words) in [
        (
            "(ly:make-skyline 5 0 1)",
            "Wrong type argument in position 1 (expecting list): 5",
        ),
        (
            "(ly:make-skyline '((0 0 1)) 0 1)",
            "Wrong type (expecting list of 4 numbers): (0 0 1)",
        ),
        (
            "(ly:make-skyline '((0 0 1 a)) 2 1)",
            "Wrong type (expecting real number): a",
        ),
        (
            "(ly:make-skyline '((0 0 +inf.0 1)) 0 1)",
            "building with infinite bound must be horizontal",
        ),
        (
            "(ly:make-skyline '() 0 2)",
            "Wrong type argument in position 3 (expecting direction): 2",
        ),
        (
            "(ly:skyline-merge up down)",
            "expecting skylines with the same direction",
        ),
        (
            "(ly:skyline-height up 'a)",
            "Wrong type argument in position 2 (expecting real number): a",
        ),
        (
            "(ly:skyline-height up +inf.0)",
            "Argument 2 out of range: +inf.0",
        ),
        (
            "(ly:skylines-for-stencil 5 0)",
            "Wrong type argument in position 1 (expecting Stencil): 5",
        ),
        (
            "(ly:skyline-distance up up)",
            "expecting skylines of opposite directions",
        ),
    ] {
        let got = caught(&mut vm, call);
        assert!(got.contains(words), "{call}: {got}");
    }
    // side-position-interface.cc's callbacks of the step, their type
    // errors under the method's name, as LilyPond's.
    for (call, subr) in [
        (
            "(ly:side-position-interface::calc-cross-staff 5)",
            "ly:calc-cross-staff",
        ),
        (
            "(ly:side-position-interface::set-axis! 5 0)",
            "ly:set-axis!",
        ),
        (
            "(ly:side-position-interface::y-aligned-side 5)",
            "ly:y-aligned-side",
        ),
        (
            "(ly:hara-kiri-group-spanner::force-hara-kiri-in-y-parent-callback 5)",
            "ly:force-hara-kiri-in-y-parent-callback",
        ),
    ] {
        let got = caught(&mut vm, call);
        assert!(
            got.contains(subr)
                && got.contains("Wrong type argument in position 1 (expecting Grob): 5"),
            "{call}: {got}"
        );
    }
}

/// How far `%outside-staff-move` should move `v` in the direction `dir`
/// to clear each of `others` (skylines, padding, horizon padding), the
/// grob's own paddings the larger where they are: each distance measured
/// alone, as `avoid_outside_staff_collisions` does, and the nearest shift
/// that way that is in no forbidden interval.
fn moved(
    v: &SkylinePair,
    pad: f64,
    hpad: f64,
    others: &[(SkylinePair, f64, f64)],
    dir: f64,
) -> f64 {
    let forbidden: Vec<(f64, f64)> = others
        .iter()
        .map(|(o, p, h)| {
            let (p, h) = (p.max(pad), h.max(hpad));
            let up = v.down.distance(&o.up, h) + p;
            let down = v.up.distance(&o.down, h) + p;
            (-down, up)
        })
        .collect();
    let mut x = 0.0;
    while let Some(&(lo, hi)) = forbidden.iter().find(|(lo, hi)| *lo < x && x < *hi) {
        x = if dir > 0.0 { hi } else { lo };
    }
    x
}

#[test]
fn the_outside_staff_move_pads_once_for_each_horizon_padding() {
    // A grob of sloped skylines against others of several horizon
    // paddings, some the same, below and above its own: the move padded
    // once for each is the one padded again for each other (MS3 step 59).
    let mut vm = vm();
    let v =
        "(ly:skylines-for-stencil (ly:make-stencil '(draw-line 0.2 0 0 3 1) '(0 . 3) '(0 . 1)) 0)";
    let others = [
        ("'(-1.5 . -0.2)", "'(0 . 0.8)", 0.0, 0.0),
        ("'(3.1 . 4)", "'(0.5 . 1.2)", 0.0, 0.3),
        ("'(-0.4 . 0.6)", "'(1.3 . 2)", 0.2, 0.3),
        ("'(2.4 . 3.5)", "'(-1 . 0.4)", 0.0, 0.7),
        ("'(5 . 6)", "'(0 . 1)", 0.6, 0.0),
        ("'(1 . 2)", "'(-2.5 . -1.5)", 0.0, 0.3),
    ];
    let list = |items: Vec<String>| format!("(list {})", items.join(" "));
    let skylines = list(
        others
            .iter()
            .map(|(x, y, _, _)| format!("(%box-skylines {x} {y} 0)"))
            .collect(),
    );
    let paddings = list(others.iter().map(|o| format!("{:?}", o.2)).collect());
    let horizon_paddings = list(others.iter().map(|o| format!("{:?}", o.3)).collect());
    let grob = pair(&mut vm, v);
    let theirs: Vec<(SkylinePair, f64, f64)> = others
        .iter()
        .map(|(x, y, p, h)| (pair(&mut vm, &format!("(%box-skylines {x} {y} 0)")), *p, *h))
        .collect();
    for (pad, hpad) in [(0.1, 0.2), (0.0, 0.0), (0.3, 0.5)] {
        for dir in [1, -1] {
            let call = format!(
                "(%outside-staff-move {v} {pad:?} {hpad:?} {skylines} {paddings} {horizon_paddings} {dir})"
            );
            let got = eval(&mut vm, &call);
            let got = vm.heap().to_f64(got).expect("a real");
            let want = moved(&grob, pad, hpad, &theirs, f64::from(dir));
            assert_eq!(
                got.to_bits(),
                want.to_bits(),
                "{pad} {hpad} {dir}: {got} {want}"
            );
        }
    }
}
