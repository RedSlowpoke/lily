//! The C++ callbacks of layout that `lily.scm`'s init data stores at load
//! (`define-grobs.scm`, `output-lib.scm`, `script.scm`, `breath.scm`,
//! `markup.scm`) and the init files (`paper-defaults-init.ly`'s page
//! breaking and text replacements),
//! which the engine does not have before layout (MS3): named placeholders,
//! as D67's are, each a primitive with LilyPond 2.26.0's name and arity
//! that throws `misc-error` when called. Their names and arities are what
//! LilyPond 2.26.0's `procedure-minimum-arity` gives for each name those
//! files evaluate at load; the `ly:` names `output-lib.scm` defines itself
//! are not among them. Layout replaces each under the same name and arity.
//!
//! And `bezier-scheme.cc`'s `ly:bezier-extract`, which is no placeholder:
//! the `diagrams` part calls it while it loads (`define-woodwind-diagrams.scm`'s
//! stencils), so T29's `\harp-pedal` needs it (MS2 step 20).

use lily_engine_scheme::{Cx, Prim, Tag, Thrown, Value};

/// A Bézier curve's four control points.
type Curve = [(f64, f64); 4];

/// The four control points of a Bézier curve as `from_scm<Bezier>` takes
/// them: a list of four pairs of reals.
fn bezier(cx: &Cx, mut list: Value) -> Option<Curve> {
    let heap = cx.heap();
    let mut points = [(0.0, 0.0); 4];
    for point in &mut points {
        let (pair, rest) = (heap.car(list)?, heap.cdr(list)?);
        if pair.tag() != Tag::Pair {
            return None;
        }
        *point = (heap.to_f64(heap.car(pair)?)?, heap.to_f64(heap.cdr(pair)?)?);
        list = rest;
    }
    (list == Value::NULL).then_some(points)
}

/// `Bezier::subdivide`: the curve split at `t` by de Casteljau's
/// algorithm, its left part and its right part.
fn subdivide(c: Curve, t: f64) -> (Curve, Curve) {
    let lerp = |a: (f64, f64), b: (f64, f64)| (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
    let (p01, p12, p23) = (lerp(c[0], c[1]), lerp(c[1], c[2]), lerp(c[2], c[3]));
    let (p012, p123) = (lerp(p01, p12), lerp(p12, p23));
    let mid = lerp(p012, p123);
    ([c[0], p01, p012, mid], [mid, p123, p23, c[3]])
}

/// `(ly:bezier-extract control-points t-min t-max)`: `Bezier::extract`.
/// LilyPond's programming errors for limits outside 0 to 1, or out of
/// order, are not written.
pub(crate) fn bezier_extract(cx: &mut Cx, args: &[Value]) -> Result<Value, Thrown> {
    const SUBR: &str = "ly:bezier-extract";
    let Some(curve) = bezier(cx, args[0]) else {
        return Err(cx.wrong_type(SUBR, 1, args[0], "Bezier"));
    };
    let mut limits = [0.0; 2];
    for (i, limit) in limits.iter_mut().enumerate() {
        match cx.heap().to_f64(args[i + 1]) {
            Some(x) if cx.heap().num(args[i + 1]).is_some() => *limit = x,
            _ => return Err(cx.wrong_type(SUBR, i + 2, args[i + 1], "Real")),
        }
    }
    let [t_min, t_max] = limits;
    let right = if t_min == 0.0 {
        curve
    } else {
        subdivide(curve, t_min).1
    };
    let part = if t_max == 1.0 {
        right
    } else {
        subdivide(right, (t_max - t_min) / (1.0 - t_min)).0
    };
    let heap = cx.heap_mut();
    let points: alloc::vec::Vec<Value> = part
        .iter()
        .map(|&(x, y)| {
            let (x, y) = (heap.flonum(x), heap.flonum(y));
            heap.cons(x, y)
        })
        .collect();
    Ok(heap.list(&points))
}

/// A placeholder's call: `misc-error` from the primitive, naming what
/// brings it.
#[cold]
fn unimplemented(cx: &mut Cx, name: &str) -> Result<Value, Thrown> {
    Err(cx.error(
        "misc-error",
        Some(name),
        "not in the engine before layout (MS3)",
        &[],
    ))
}

/// A callback of `min` required and `optional` optional arguments.
macro_rules! cb {
    ($name:literal, $min:literal, $optional:literal) => {
        Prim {
            name: $name,
            min: $min,
            max: $min + $optional,
            f: |cx, _| unimplemented(cx, $name),
        }
    };
}

/// A callback of the grob alone, as most are.
macro_rules! one {
    ($name:literal) => {
        cb!($name, 1, 0)
    };
}

/// The placeholders, sorted by name, in the order a VM numbers them after
/// the crate's other primitives.
pub(crate) static PRIMS: &[Prim] = &[
    one!("ly:balloon-interface::print"),
    cb!("ly:balloon-interface::pure-height", 3, 0),
    one!("ly:balloon-interface::width"),
    one!("ly:bar-line::calc-anchor"),
    one!("ly:bar-line::calc-bar-extent"),
    one!("ly:bar-line::calc-short-bar-extent"),
    one!("ly:bar-line::print"),
    one!("ly:breathing-sign::divisio-maior"),
    one!("ly:breathing-sign::divisio-maxima"),
    one!("ly:breathing-sign::divisio-minima"),
    one!("ly:breathing-sign::finalis"),
    one!("ly:chord-name::after-line-breaking"),
    one!("ly:cluster-beacon::height"),
    one!("ly:cluster::calc-cross-staff"),
    one!("ly:cluster::print"),
    one!("ly:enclosing-bracket::print"),
    one!("ly:enclosing-bracket::width"),
    one!("ly:figured-bass-continuation::center-on-figures"),
    one!("ly:grid-line-interface::print"),
    one!("ly:grid-line-interface::width"),
    one!("ly:kievan-ligature::print"),
    one!("ly:measure-grouping::print"),
    one!("ly:measure-spanner::print"),
    one!("ly:mensural-ligature::print"),
    one!("ly:percent-repeat-interface::beat-slash"),
    one!("ly:percent-repeat-interface::double-percent"),
    one!("ly:percent-repeat-interface::percent"),
    one!("ly:piano-pedal-bracket::print"),
    one!("ly:span-bar::before-line-breaking"),
    one!("ly:span-bar::calc-anchor"),
    one!("ly:span-bar::calc-glyph-name"),
    one!("ly:span-bar::print"),
    one!("ly:span-bar::width"),
    one!("ly:sustain-pedal::print"),
    one!("ly:time-signature::calc-fraction"),
    one!("ly:time-signature::print"),
    one!("ly:vaticana-ligature::print"),
    one!("ly:volta-bracket::calc-shorten-pair"),
    one!("ly:vowel-transition::set-spacing-rods"),
];
