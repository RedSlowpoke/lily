//! What of Béziers part 4 needs (MS3 step 54): `bezier-bow.cc`'s
//! `slur_shape` (`:118–132`, the split's `first.txt`) with the two it
//! calls, `slur_height` (`:34–38`) and `get_slur_indent_height`
//! (`:105–116`), the curve of a slur `width` long that rises towards
//! `h_inf`, which the chord slur draws (`arpeggio.rs`); and, by the
//! user's answer to the step's stop (D211), what `Lookup::slur` and
//! `Lookup::bezier_sandwich` need of `bezier.cc`: `curve_point`
//! (`:125–148`), `polynomial` (`:183–197`) over `Polynomial_cache`'s terms
//! (`:166–181`), `filter_solutions` (`:199–210`), `solve_derivative`
//! (`:212–225`), `extent` (`:288–303`), `rotate` (`:329–335`),
//! `subdivide` (`:362–381`) and `extract` (`:383–406`), with
//! `flower/polynomial.cc`'s arithmetic as they use it (`differentiate`,
//! `clean`, `solve` of a degree up to 2).
//!
//! And what the slurs and ties read of bezier.cc (MS4 step 7):
//! `get_other_coordinate` (`:73–92`), `get_other_coordinates`
//! (`:94–103`), `curve_coordinate` (`:105–123`), `solve_point`
//! (`:227–237`), `minmax` (`:239–285`), `control_point_extent`
//! (`:305–313`), `scale` and `translate` (`:315–343`); with
//! polynomial.cc's `solve_cubic` and `cubic_root`, which `solve_point`'s
//! cubic needs. Its array functions, `dir_at_point` and `reverse` have no
//! caller among them (slur*.cc, tie*.cc, semi-tie*.cc, lookup.cc).
//!
//! Each operation is rounded as LilyPond's: `Offset`'s operators are
//! functions, which clang does not fuse; `solve_quadric`'s `p * p - q`,
//! `solve_cubic`'s `p`, `q` and `D`, and `curve_coordinate`'s sum are
//! single expressions, which it does ([`fma`], as Homebrew's arm64 binary
//! shows).

use alloc::string::String;
use alloc::vec::Vec;

use crate::geometry::{
    Axis, Interval, Offset, X, Y, complex_multiply, fma, offset_directed, other_axis,
};
use crate::lookup::Bezier;
use lily_engine_scheme::{Cx, Thrown, Value};

/// `F0_1`: 0 at 0, rising with slope 1 towards 1.
fn f0_1(x: f64) -> f64 {
    use core::f64::consts::PI;
    2.0 / PI * libm::atan(PI * x / 2.0)
}

/// `slur_height`: the height of a slur `width` long, `r_0` its slope at
/// 0, `h_inf` its limit.
pub fn slur_height(width: f64, h_inf: f64, r_0: f64) -> f64 {
    f0_1(width * r_0 / h_inf) * h_inf
}

/// `get_slur_indent_height`: the slur's indent, a third of its width for a
/// short one and towards `2 * h_inf` for a long one, and its height.
pub fn get_slur_indent_height(width: f64, h_inf: f64, r_0: f64) -> (f64, f64) {
    let max_fraction = 1.0 / 3.1;
    let height = slur_height(width, h_inf, r_0);

    let q = 2.0 * h_inf / max_fraction;
    let indent = 2.0 * h_inf - q * q * max_fraction / (width + q);
    (indent, height)
}

/// `slur_shape`: the curve from `(0, 0)` to `(width, 0)` through its
/// indent at its height.
pub fn slur_shape(width: f64, h_inf: f64, r_0: f64) -> Bezier {
    let (indent, height) = get_slur_indent_height(width, h_inf, r_0);

    let control: [Offset; 4] = [
        (0.0, 0.0),
        (indent, height),
        (width - indent, height),
        (width, 0.0),
    ];
    control
}

/// `Bezier::rotate`: each control point turned by `deg` degrees about the
/// origin.
pub fn rotate(curve: &mut Bezier, deg: f64) {
    let rot = offset_directed(deg);
    for c in curve.iter_mut() {
        *c = complex_multiply(rot, *c);
    }
}

/// `binomial_coefficient_3`.
const BINOMIAL_COEFFICIENT_3: [f64; 4] = [1.0, 3.0, 3.0, 1.0];

/// `Polynomial_cache`'s terms, binom (3, j) t^j (1-t)^{3-j}, lowest
/// coefficient first: integers, which `Polynomial::power` and `multiply`
/// give exactly.
const TERMS: [[f64; 4]; 4] = [
    [1.0, -3.0, 3.0, -1.0],
    [0.0, 3.0, -6.0, 3.0],
    [0.0, 0.0, 3.0, -3.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// The coordinate `a` of an offset.
fn at(o: Offset, a: Axis) -> f64 {
    if a == X { o.0 } else { o.1 }
}

/// `curve_point`: the point at `t`.
pub fn curve_point(curve: &Bezier, t: f64) -> Offset {
    let mut tj = 1.0;
    let mut one_min_tj = [1.0; 4];
    for i in 1..4 {
        one_min_tj[i] = one_min_tj[i - 1] * (1.0 - t);
    }

    let mut o = (0.0, 0.0);
    for (j, c) in curve.iter().enumerate() {
        let b = BINOMIAL_COEFFICIENT_3[j];
        o.0 += c.0 * b * tj * one_min_tj[3 - j];
        o.1 += c.1 * b * tj * one_min_tj[3 - j];

        tj *= t;
    }
    o
}

/// `polynomial`: the coordinate `a` as a polynomial in t, lowest
/// coefficient first.
fn polynomial(curve: &Bezier, a: Axis) -> [f64; 4] {
    let mut p = [0.0; 4];
    for (term, c) in TERMS.iter().zip(curve) {
        for (p, t) in p.iter_mut().zip(term) {
            *p += t * at(*c, a);
        }
    }
    p
}

/// `Polynomial::solve` of `coefs` (`clean` first: a top coefficient that
/// is 0, or under 1e-8 of the next, dropped): the roots of a linear,
/// quadratic or cubic one, none of a constant.
fn solve(mut coefs: Vec<f64>) -> Vec<f64> {
    const FUDGE: f64 = 1e-8;
    while coefs.len() > 1 {
        let (top, next) = (coefs[coefs.len() - 1], coefs[coefs.len() - 2]);
        if libm::fabs(top) < FUDGE * libm::fabs(next) || top == 0.0 {
            coefs.pop();
        } else {
            break;
        }
    }
    let mut sol = Vec::new();
    match coefs[..] {
        // solve_linear
        [c0, c1] => {
            if c1 != 0.0 {
                sol.push(-c0 / c1);
            }
        }
        // solve_quadric: normal form x^2 + px + q = 0
        [c0, c1, c2] => {
            let p = c1 / (2.0 * c2);
            let q = c0 / c2;
            let mut d = fma(p, p, -q);
            if d > 0.0 {
                d = libm::sqrt(d);
                sol.push(d - p);
                sol.push(-d - p);
            }
        }
        [c0, c1, c2, c3] => sol = solve_cubic(c0, c1, c2, c3),
        _ => {}
    }
    sol
}

/// `cubic_root`: the real cube root, 0 at 0.
fn cubic_root(x: f64) -> f64 {
    if x > 0.0 {
        libm::pow(x, 1.0 / 3.0)
    } else if x < 0.0 {
        -libm::pow(-x, 1.0 / 3.0)
    } else {
        0.0
    }
}

/// `solve_cubic`: the real roots of `c0 + c1 x + c2 x^2 + c3 x^3` by
/// Cardano's formula, as LilyPond's binary rounds them.
fn solve_cubic(c0: f64, c1: f64, c2: f64, c3: f64) -> Vec<f64> {
    use core::f64::consts::PI;
    let mut sol = Vec::new();

    /* normal form: x^3 + Ax^2 + Bx + C = 0 */
    let a = c2 / c3;
    let b = c1 / c3;
    let c = c0 / c3;

    /*
     * substitute x = y - A/3 to eliminate quadric term: x^3 +px + q = 0
     */

    let sq_a = a * a;
    let p = 1.0 / 3.0 * fma(-1.0 / 3.0, sq_a, b);
    let q = 1.0 / 2.0 * (c + fma(2.0 / 27.0 * a, sq_a, -(b * (1.0 / 3.0 * a))));

    /* use Cardano's formula */

    let cb = p * (p * p);
    let d = fma(q, q, cb);

    if d == 0.0 {
        if q == 0.0 {
            /* one triple solution */
            sol.extend([0.0, 0.0, 0.0]);
        } else {
            /* one single and one double solution */
            let u = cubic_root(-q);

            sol.push(2.0 * u);
            sol.push(-u);
        }
    } else if d < 0.0 {
        /* Casus irreducibilis: three real solutions */
        let phi = 1.0 / 3.0 * libm::acos(-q / libm::sqrt(-cb));
        let t = 2.0 * libm::sqrt(-p);

        sol.push(t * libm::cos(phi));
        sol.push(-t * libm::cos(phi + PI / 3.0));
        sol.push(-t * libm::cos(phi - PI / 3.0));
    } else {
        /* one real solution */
        let sqrt_d = libm::sqrt(d);
        let u = cubic_root(sqrt_d - q);
        let v = -cubic_root(sqrt_d + q);

        sol.push(u + v);
    }

    /* resubstitute */
    let sub = 1.0 / 3.0 * a;

    for s in &mut sol {
        *s -= sub;
    }
    sol
}

/// `filter_solutions`: what is not below 0 nor above 1, NaN too.
fn filter_solutions(mut sol: Vec<f64>) -> Vec<f64> {
    sol.retain(|&t| (0.0..=1.0).contains(&t) || t.is_nan());
    sol
}

/// `solve_derivative`: the t in [0, 1] (or NaN, which `filter_solutions`
/// keeps) where the curve's direction is proportional to `deriv`.
pub(crate) fn solve_derivative(curve: &Bezier, deriv: Offset) -> Vec<f64> {
    let [x, y] = [X, Y].map(|a| polynomial(curve, a));
    // differentiate, then xp * deriv[Y] - yp * deriv[X]
    let combine = (1..4)
        .map(|i| {
            let i_ = i as f64;
            (x[i] * i_) * deriv.1 - (y[i] * i_) * deriv.0
        })
        .collect();

    filter_solutions(solve(combine))
}

/// `solve_point`: the t in [0, 1] (or NaN) where the curve's coordinate
/// `ax` is `coordinate`.
pub fn solve_point(curve: &Bezier, ax: Axis, coordinate: f64) -> Vec<f64> {
    let mut p = polynomial(curve, ax);
    p[0] -= coordinate;
    filter_solutions(solve(p.to_vec()))
}

/// `curve_coordinate`: the coordinate `a` of the point at `t`, each term
/// added in one rounding.
pub fn curve_coordinate(curve: &Bezier, t: f64, a: Axis) -> f64 {
    let mut tj = 1.0;
    let mut one_min_tj = [1.0; 4];
    for i in 1..4 {
        one_min_tj[i] = one_min_tj[i - 1] * (1.0 - t);
    }

    let mut r = 0.0;
    for j in 0..4 {
        r = fma(
            at(curve[j], a) * BINOMIAL_COEFFICIENT_3[j] * tj,
            one_min_tj[3 - j],
            r,
        );

        tj *= t;
    }
    r
}

/// `get_other_coordinate`: the other coordinate where the curve's
/// coordinate `a` is `x`, at the first solution; 0 with LilyPond's
/// programming error, pushed onto `errors`, if there is none.
pub fn get_other_coordinate(curve: &Bezier, a: Axis, x: f64, errors: &mut Vec<String>) -> f64 {
    let other = other_axis(a);
    let ts = solve_point(curve, a, x);

    if ts.is_empty() {
        errors.push(String::from("no solution found for Bezier intersection"));
        return 0.0;
    }

    curve_coordinate(curve, ts[0], other)
}

/// `get_other_coordinates`: the other coordinate at each solution.
pub fn get_other_coordinates(curve: &Bezier, a: Axis, x: f64) -> Vec<f64> {
    let other = other_axis(a);
    solve_point(curve, a, x)
        .into_iter()
        .map(|t| curve_coordinate(curve, t, other))
        .collect()
}

/// `minmax`: of the curve between `l` and `r` on `ax`, its limit on the
/// other axis towards `d`; 0 with LilyPond's programming error, pushed
/// onto `errors`, if no part of it is there.
pub fn minmax(
    curve: &Bezier,
    ax: Axis,
    l: f64,
    r: f64,
    d: crate::geometry::Direction,
    errors: &mut Vec<String>,
) -> f64 {
    let bx = other_axis(ax);

    // The curve could hit its bounding box limit along BX at:
    //  points where the curve is parallel to AX,
    let mut vec = (0.0, 0.0);
    if ax == X {
        vec.0 = 1.0;
    } else {
        vec.1 = 1.0;
    }
    let mut sols = solve_derivative(curve, vec);
    //  or endpoints of the curve,
    sols.push(0.999);
    sols.push(0.001);
    // (using points just inside the ends, so that an endpoint is evaulated
    //  if it falls within rounding error of L or R and the curve lies inside)

    let mut iv = Interval::EMPTY;
    for &t in sols.iter().rev() {
        let p = curve_point(curve, t);
        if at(p, ax) >= l && at(p, ax) <= r {
            iv.add_point(at(p, bx));
        }
    }

    //  or intersections of the curve with the bounding lines at L and R.
    for x in [l, r] {
        for &v in get_other_coordinates(curve, ax, x).iter().rev() {
            iv.add_point(v);
        }
    }

    if iv.is_empty() {
        errors.push(String::from(
            "Bezier curve does not cross region of concern",
        ));
        return 0.0;
    }

    iv.at(d)
}

/// `control_point_extent`: the extent on `a` of the control points.
pub fn control_point_extent(curve: &Bezier, a: Axis) -> Interval {
    let mut ext = Interval::EMPTY;
    for c in curve.iter().rev() {
        ext.add_point(at(*c, a));
    }
    ext
}

/// `scale`: each control point's coordinates multiplied by `x` and `y`.
pub fn scale(curve: &mut Bezier, x: f64, y: f64) {
    for c in curve.iter_mut() {
        *c = (x * c.0, y * c.1);
    }
}

/// `translate`: each control point moved by `o`.
pub fn translate(curve: &mut Bezier, o: Offset) {
    for c in curve.iter_mut() {
        *c = (c.0 + o.0, c.1 + o.1);
    }
}

/// `extent`: the curve's extent on `a`, at its ends and where it turns.
pub fn extent(curve: &Bezier, a: Axis) -> Interval {
    let mut d = (0.0, 0.0);
    if other_axis(a) == X {
        d.0 = 1.0;
    } else {
        d.1 = 1.0;
    }
    let mut iv = Interval::EMPTY;
    let mut sols = solve_derivative(curve, d);
    sols.push(1.0);
    sols.push(0.0);
    for &t in sols.iter().rev() {
        let o = at(curve_point(curve, t), a);
        iv.unite(Interval::new(o, o));
    }
    iv
}

/// `(ly:bezier-extent control-points axis)` (`bezier-scheme.cc:23–36`),
/// which `stencil.scm`'s `make-path-stencil` calls for each `curveto`:
/// [`extent`] of the curve given as four pairs of reals, `is_scm<Bezier>`
/// and `is_scm<Axis>` checked as LilyPond's `LY_ASSERT_TYPE` names them.
pub(crate) fn ly_bezier_extent(cx: &mut Cx, args: &[Value]) -> Result<Value, Thrown> {
    const SUBR: &str = "ly:bezier-extent";
    let heap = cx.heap();
    let mut curve: Bezier = [(0.0, 0.0); 4];
    let mut list = args[0];
    let mut ok = true;
    for point in &mut curve {
        let coordinate =
            |v: Option<Value>| v.filter(|&v| heap.is_real(v)).and_then(|v| heap.to_f64(v));
        match heap
            .car(list)
            .map(|p| (coordinate(heap.car(p)), coordinate(heap.cdr(p))))
        {
            Some((Some(x), Some(y))) => *point = (x, y),
            _ => ok = false,
        }
        list = heap.cdr(list).unwrap_or(Value::FALSE);
    }
    if !ok || list != Value::NULL {
        return Err(cx.wrong_type(SUBR, 1, args[0], "list of four number pairs"));
    }
    let a = match cx.heap().to_i64(args[1]) {
        Some(0) if cx.heap().is_exact_integer(args[1]) => X,
        Some(1) if cx.heap().is_exact_integer(args[1]) => Y,
        _ => return Err(cx.wrong_type(SUBR, 2, args[1], "axis")),
    };
    let iv = extent(&curve, a);
    Ok(crate::offset(cx.heap_mut(), (iv.left, iv.right)))
}

/// `subdivide`: the curve split at `t` by de Casteljau's algorithm.
fn subdivide(curve: &Bezier, t: f64) -> (Bezier, Bezier) {
    let mut p = [[(0.0, 0.0); 4]; 4];

    for i in 0..4 {
        p[i][3] = curve[i];
    }
    for j in (0..3).rev() {
        for i in 0..3 {
            let (a, b) = (p[i][j + 1], p[i + 1][j + 1]);
            p[i][j] = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
        }
    }
    let (mut left, mut right) = ([(0.0, 0.0); 4], [(0.0, 0.0); 4]);
    for i in 0..4 {
        left[i] = p[0][3 - i];
        right[i] = p[i][i];
    }
    (left, right)
}

/// `extract`: the part of the curve from `t_min` to `t_max`, with
/// LilyPond's programming errors, pushed onto `errors`, for bounds out of
/// [0, 1] or out of order.
pub fn extract(curve: &Bezier, t_min: f64, t_max: f64, errors: &mut Vec<String>) -> Bezier {
    if t_min < 0.0 || t_max > 1.0 {
        errors.push(String::from(
            "bezier extract arguments outside of limits: curve may have bad shape",
        ));
    }
    if t_min >= t_max {
        errors.push(String::from(
            "lower bezier extract value not less than upper value: curve may have bad shape",
        ));
    }
    let bez2 = if t_min == 0.0 {
        *curve
    } else {
        subdivide(curve, t_min).1
    };
    if t_max == 1.0 {
        bez2
    } else {
        subdivide(&bez2, (t_max - t_min) / (1.0 - t_min)).0
    }
}
