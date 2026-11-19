//! LilyPond's `Lookup` (`lookup.cc`): the shapes that grobs and markups
//! draw, as stencils, with `stencil-scheme.cc`'s `ly:round-filled-box`,
//! `ly:round-polygon` and `ly:bracket`; since MS3 step 54 `slur` and
//! `bezier_sandwich`, over what of `bezier.cc` they need (`bezier.rs`,
//! D211).
//!
//! `round_filled_box`'s warning about a negative box is pushed onto the
//! `warnings` its callers give; the Scheme face gives it to `(lily)`'s
//! `ly:warning`, as LilyPond's C++ `warning` would print it.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Cx, Heap, Value};

use crate::bezier;
use crate::geometry::{
    Axis, Box, Interval, Offset, X, Y, complex_multiply, direction, fma, hypot, length, min,
    other_axis,
};
use crate::line::make_line;
use crate::stencil::{Stencil, wrap};
use crate::{R, axis_arg, number_arg, number_pair_arg, to_double};

/// A Bézier curve's four control points (`bezier.hh`).
pub type Bezier = [Offset; 4];

/// The values as `to_scm (Real)` makes them.
fn reals(heap: &mut Heap, xs: &[f64]) -> Vec<Value> {
    xs.iter().map(|&x| heap.flonum(x)).collect()
}

/// `flower`'s `sign`: -1, 0 or 1 by the sign bit.
fn sign(x: f64) -> f64 {
    if x != 0.0 {
        if x.is_sign_negative() { -1.0 } else { 1.0 }
    } else {
        0.0
    }
}

/// `(polygon points blot filled)`, the points' coordinates in reverse
/// order of the points, as `scm_cons` builds them.
fn polygon(heap: &mut Heap, points: &[Offset], blot: f64, filled: bool) -> Value {
    let mut list = Value::NULL;
    for &(x, y) in points {
        let (x, y) = (heap.flonum(x), heap.flonum(y));
        let tail = heap.cons(y, list);
        list = heap.cons(x, tail);
    }
    let head = heap.intern("polygon");
    let blot = heap.flonum(blot);
    heap.list(&[head, list, blot, Value::bool(filled)])
}

/// `(path thickness commands round round #t)`.
fn path(heap: &mut Heap, thickness: Value, commands: &[Value]) -> Value {
    let head = heap.intern("path");
    let commands = heap.list(commands);
    let round = heap.intern("round");
    heap.list(&[head, thickness, commands, round, round, Value::TRUE])
}

/// `Lookup::beam`: a parallelogram of `width`, rising by `slope`, `thick`
/// high, its corners rounded by `blot`.
pub fn beam(heap: &mut Heap, slope: f64, width: f64, thick: f64, blot: f64) -> Stencil {
    let mut b = Box::EMPTY;
    let half = blot / 2.0;
    let corners = [
        ((0.0, thick / 2.0), (1.0, -1.0)),
        ((0.0, -thick / 2.0), (1.0, 1.0)),
        ((width, fma(width, slope, -(thick / 2.0))), (-1.0, 1.0)),
        ((width, fma(width, slope, thick / 2.0)), (-1.0, -1.0)),
    ];
    let mut points = Vec::with_capacity(4);
    for (p, inward) in corners {
        b.add_point(p);
        points.push((p.0 + inward.0 * half, p.1 + inward.1 * half));
    }
    Stencil::new(b, polygon(heap, &points, blot, true))
}

/// `Lookup::rotated_box`: a box of `width` and `thick` rotated to `slope`.
pub fn rotated_box(heap: &mut Heap, slope: f64, width: f64, thick: f64, blot: f64) -> Stencil {
    let rot = direction((1.0, slope));
    let points = [
        (0.0, -thick / 2.0),
        (width, -thick / 2.0),
        (width, thick / 2.0),
        (0.0, thick / 2.0),
    ]
    .map(|p| complex_multiply(p, rot));
    round_polygon(heap, &points, blot, -1.0, true)
}

/// `Lookup::horizontal_line`: from `w`'s left to its right, `th` thick.
pub fn horizontal_line(heap: &mut Heap, w: Interval, th: f64) -> Stencil {
    let head = heap.intern("draw-line");
    let [thick, left, right] = [th, w.left, w.right].map(|x| heap.flonum(x));
    let zero = heap.integer(0);
    let expr = heap.list(&[head, thick, left, zero, right, zero]);
    Stencil::new(Box::new(w, Interval::new(-th / 2.0, th / 2.0)), expr)
}

/// `Lookup::blank`: the box, with the empty string as its expression.
pub fn blank(heap: &mut Heap, b: Box) -> Stencil {
    Stencil::new(b, heap.string(""))
}

/// `Lookup::circle`.
pub fn circle(heap: &mut Heap, rad: f64, thick: f64, filled: bool) -> Stencil {
    let head = heap.intern("circle");
    let [r, t] = [rad, thick].map(|x| heap.flonum(x));
    let expr = heap.list(&[head, r, t, Value::bool(filled)]);
    let i = Interval::new(-rad, rad);
    Stencil::new(Box::new(i, i), expr)
}

/// `Lookup::filled_box`: [`round_filled_box`] with no rounding.
pub fn filled_box(heap: &mut Heap, b: Box, warnings: &mut Vec<String>) -> Stencil {
    round_filled_box(heap, b, 0.0, warnings)
}

/// `Lookup::round_filled_box`: the box filled, its corners rounded by a
/// blot no larger than its width or height. A negative blot draws
/// nothing, with a warning unless it is infinite.
pub fn round_filled_box(
    heap: &mut Heap,
    b: Box,
    blotdiameter: f64,
    warnings: &mut Vec<String>,
) -> Stencil {
    let width = b.x().length();
    let height = b.y().length();
    let blot = min(min(blotdiameter, width), height);
    if blot < 0.0 {
        if !blot.is_infinite() {
            warnings.push(format!(
                "Not drawing a box with negative dimension, {width:.2} by {height:.2}."
            ));
        }
        return Stencil::new(b, Value::NULL);
    }
    let head = heap.intern("round-filled-box");
    let values = reals(
        heap,
        &[-b.x().left, b.x().right, -b.y().left, b.y().right, blot],
    );
    let mut items = Vec::with_capacity(6);
    items.push(head);
    items.extend(values);
    Stencil::new(b, heap.list(&items))
}

/// `Lookup::round_polygon`: the polygon through `points`, its outer
/// corners rounded by `blotdiameter`, moved in by half of it times
/// `extroversion` (-1 inside the points, 0 on them, 1 outside). One point
/// is a circle, two a line. LilyPond's programming error for duplicate
/// points, which only a checking build gives, is not written.
pub fn round_polygon(
    heap: &mut Heap,
    points: &[Offset],
    blotdiameter: f64,
    extroversion: f64,
    filled: bool,
) -> Stencil {
    match points {
        [] => return Stencil::default(),
        &[p] => {
            let mut circ = circle(heap, 0.5 * (1.0 + extroversion) * blotdiameter, 0.0, true);
            circ.translate(heap, p);
            return circ;
        }
        &[p, q] => return make_line(heap, (1.0 + extroversion) * blotdiameter, p, q),
        _ => {}
    }
    let n = points.len();
    let shrunk: Vec<Offset> = if extroversion == 0.0 {
        points.to_vec()
    } else {
        // The orientation, by twice the oriented area about the center.
        let mut center = (0.0, 0.0);
        for p in points {
            center = (center.0 + p.0, center.1 + p.1);
        }
        let count = n as f64;
        center = (center.0 / count, center.1 / count);
        let mut area = 0.0;
        let mut last = (points[n - 1].0 - center.0, points[n - 1].1 - center.1);
        for p in points {
            let here = (p.0 - center.0, p.1 - center.1);
            area += fma(last.0, here.1, -(last.1 * here.0));
            last = here;
        }
        let ccw = area >= 0.0;
        let mut shrunk = alloc::vec![(0.0, 0.0); n];
        for i in 0..n {
            let (p0, p1, p2) = (points[i], points[(i + 1) % n], points[(i + 2) % n]);
            let p01 = (p1.0 - p0.0, p1.1 - p0.1);
            let p12 = (p2.0 - p1.0, p2.1 - p1.1);
            let mut inward0 = direction((-p01.1, p01.0));
            let mut inward2 = direction((-p12.1, p12.0));
            if !ccw {
                inward0 = (-inward0.0, -inward0.1);
                inward2 = (-inward2.0, -inward2.1);
            }
            let middle = (0.5 * (inward0.0 + inward2.0), 0.5 * (inward0.1 + inward2.1));
            let mut proj = fma(middle.0, inward0.0, middle.1 * inward0.1);
            if libm::fabs(proj) < 0.03 {
                proj = if proj < 0.0 { -0.03 } else { 0.03 };
            }
            let k = 0.5 * blotdiameter / proj;
            let moved = ((k * middle.0) * extroversion, (k * middle.1) * extroversion);
            shrunk[(i + 1) % n] = (p1.0 - moved.0, p1.1 - moved.1);
        }
        shrunk
    };
    let mut b = Box::EMPTY;
    let mut shrunk_box = Box::EMPTY;
    for (&p, &s) in points.iter().zip(&shrunk) {
        b.add_point(p);
        shrunk_box.add_point(s);
    }
    shrunk_box.widen(0.5 * blotdiameter, 0.5 * blotdiameter);
    b.unite(shrunk_box);
    Stencil::new(b, polygon(heap, &shrunk, blotdiameter, filled))
}

/// `Lookup::frame`: the box's four edges, `thick` wide, as rounded boxes.
pub fn frame(
    heap: &mut Heap,
    b: Box,
    thick: f64,
    blot: f64,
    warnings: &mut Vec<String>,
) -> Stencil {
    let mut m = Stencil::default();
    for a in [X, Y] {
        let o = other_axis(a);
        for d in [-1, 1] {
            let mut edge = Interval::new(-1.0, 1.0);
            edge.scale(0.5 * thick);
            edge.translate(b.0[a].at(d));
            let mut edges = Box::EMPTY;
            edges.0[a] = edge;
            edges.0[o] = Interval::new(b.0[o].left - thick / 2.0, b.0[o].right + thick / 2.0);
            let side = round_filled_box(heap, edges, blot, warnings);
            m.add_stencil(heap, &side);
        }
    }
    m
}

/// `Lookup::slur`: the curve made `curvethick` thick in its middle, as a
/// sandwich of two curves `linethick` thick; with `dash_details`, a list of
/// `(t-min t-max dash-fraction dash-period)`, each part of it from
/// `t-min` to `t-max` solid, or dashed where `dash-fraction` is not 1.
/// `Bezier::extract`'s programming errors are pushed onto `errors`. A part
/// that is no list of four reads LilyPond's defaults where Guile's
/// `car` would throw (MS3 step 54, D211).
pub fn slur(
    heap: &mut Heap,
    mut curve: Bezier,
    curvethick: f64,
    linethick: f64,
    dash_details: Value,
    errors: &mut Vec<String>,
) -> Stencil {
    let mut return_value = Stencil::default();

    /*
        calculate the offset for the two beziers that make the sandwich
        for the slur
    */
    let dir = direction((curve[3].0 - curve[0].0, curve[3].1 - curve[0].1));
    let mut back = curve;
    let half = 0.5 * curvethick;
    let perp = (-dir.1 * half, dir.0 * half);
    for i in [1, 2] {
        back[i] = (back[i].0 + perp.0, back[i].1 + perp.1);
        curve[i] = (curve[i].0 - perp.0, curve[i].1 - perp.1);
    }

    if heap.car(dash_details).is_none() {
        /* solid slur  */
        return bezier_sandwich(heap, back, curve, linethick);
    }

    /* dashed or combination slur */
    let mut details = dash_details;
    while let (Some(dash_pattern), Some(rest)) = (heap.car(details), heap.cdr(details)) {
        details = rest;
        let mut fields = [0.0, 1.0, 1.0, 0.75];
        let mut l = dash_pattern;
        for field in &mut fields {
            let Some(v) = heap.car(l) else { break };
            if heap.is_real(v) {
                *field = heap.to_f64(v).unwrap_or(*field);
            }
            l = heap.cdr(l).unwrap_or(Value::NULL);
        }
        let [t_min, t_max, dash_fraction, dash_period] = fields;
        let back_segment = bezier::extract(&back, t_min, t_max, errors);
        let curve_segment = bezier::extract(&curve, t_min, t_max, errors);
        if dash_fraction == 1.0 {
            let s = bezier_sandwich(heap, back_segment, curve_segment, linethick);
            return_value.add_stencil(heap, &s);
        } else {
            let seg = (
                back_segment[3].0 - back_segment[0].0,
                back_segment[3].1 - back_segment[0].1,
            );
            let seg_length = length(seg);
            let pattern_count = (seg_length / dash_period) as i32;
            let pattern_length = 1.0 / (f64::from(pattern_count) + dash_fraction);
            for p in 0..=pattern_count {
                let start_t = f64::from(p) * pattern_length;
                let end_t = (f64::from(p) + dash_fraction) * pattern_length;
                let back_dash = bezier::extract(&back_segment, start_t, end_t, errors);
                let curve_dash = bezier::extract(&curve_segment, start_t, end_t, errors);
                let s = bezier_sandwich(heap, back_dash, curve_dash, linethick);
                return_value.add_stencil(heap, &s);
            }
        }
    }
    return_value
}

/// `Lookup::bezier_sandwich`: the region between `top_curve` and
/// `bottom_curve`, its outline `thickness` thick, its extents the curves'
/// widened by half of it.
pub fn bezier_sandwich(
    heap: &mut Heap,
    top_curve: Bezier,
    bottom_curve: Bezier,
    thickness: f64,
) -> Stencil {
    let [moveto, curveto, lineto, closepath] =
        ["moveto", "curveto", "lineto", "closepath"].map(|n| heap.intern(n));
    let [t, b] = [top_curve, bottom_curve];
    let xs = |heap: &mut Heap, points: &[Offset]| -> Vec<Value> {
        let flat: Vec<f64> = points.iter().flat_map(|&(x, y)| [x, y]).collect();
        reals(heap, &flat)
    };
    let mut commands = Vec::with_capacity(23);
    commands.push(moveto);
    commands.extend(xs(heap, &t[..1]));
    commands.push(curveto);
    commands.extend(xs(heap, &t[1..]));
    commands.push(lineto);
    commands.extend(xs(heap, &[b[3]]));
    commands.push(curveto);
    commands.extend(xs(heap, &[b[2], b[1], b[0]]));
    commands.push(closepath);

    let thick = heap.flonum(thickness);
    let horizontal_bend = path(heap, thick, &commands);

    let mut x_extent = bezier::extent(&top_curve, X);
    x_extent.unite(bezier::extent(&bottom_curve, X));
    let mut y_extent = bezier::extent(&top_curve, Y);
    y_extent.unite(bezier::extent(&bottom_curve, Y));
    let mut b = Box::new(x_extent, y_extent);

    b.widen(0.5 * thickness, 0.5 * thickness);
    Stencil::new(b, horizontal_bend)
}

/// `Lookup::repeat_slash`: a slash `w` wide rising by `s`, `t` thick.
pub fn repeat_slash(heap: &mut Heap, w: f64, s: f64, t: f64) -> Stencil {
    let x_width = hypot(t, t / s);
    let height = w * s;
    let [moveto, rlineto, closepath] = ["moveto", "rlineto", "closepath"].map(|n| heap.intern(n));
    let zero = heap.integer(0);
    let [xw, w_, h, nxw] = [x_width, w, height, -x_width].map(|x| heap.flonum(x));
    let commands = [
        moveto, zero, zero, rlineto, xw, zero, rlineto, w_, h, rlineto, nxw, zero, closepath,
    ];
    let expr = path(heap, zero, &commands);
    Stencil::new(
        Box::new(Interval::new(0.0, w + x_width), Interval::new(0.0, height)),
        expr,
    )
}

/// `Lookup::bracket`: a bracket along `iv` on axis `a`, `thick` thick,
/// its wings protruding by `protrude` (negative to the other side).
pub fn bracket(
    heap: &mut Heap,
    a: Axis,
    iv: Interval,
    thick: f64,
    protrude: f64,
    blot: f64,
    warnings: &mut Vec<String>,
) -> Stencil {
    let other = other_axis(a);
    let mut b = Box::EMPTY;
    b.0[a] = iv;
    let mut across = Interval::new(-1.0, 1.0);
    across.scale(thick);
    across.scale(0.5);
    b.0[other] = across;
    let mut m = round_filled_box(heap, b, blot, warnings);
    b.0[a] = Interval::new(iv.right - thick, iv.right);
    let mut oi = Interval::new(-thick / 2.0, thick / 2.0 + libm::fabs(protrude));
    oi.scale(sign(protrude));
    b.0[other] = oi;
    let wing = round_filled_box(heap, b, blot, warnings);
    m.add_stencil(heap, &wing);
    b.0[a] = Interval::new(iv.left, iv.left + thick);
    let wing = round_filled_box(heap, b, blot, warnings);
    m.add_stencil(heap, &wing);
    m
}

/// `Lookup::triangle`: lines from `iv`'s ends to a point `protrude` above
/// its center, and back.
pub fn triangle(heap: &mut Heap, iv: Interval, thick: f64, protrude: f64) -> Stencil {
    let points = [
        (iv.left, 0.0),
        (iv.right, 0.0),
        (iv.center(), protrude),
        (iv.left, 0.0),
    ];
    points_to_line_stencil(heap, thick, &points)
}

/// `Offset::is_sane`: neither coordinate infinite or not a number.
fn is_sane((x, y): Offset) -> bool {
    x.is_finite() && y.is_finite()
}

/// `Lookup::points_to_line_stencil`: a line between each two points in
/// turn that are both sane.
pub fn points_to_line_stencil(heap: &mut Heap, thick: f64, points: &[Offset]) -> Stencil {
    let mut ret = Stencil::default();
    for pair in points.windows(2) {
        if is_sane(pair[0]) && is_sane(pair[1]) {
            let line = make_line(heap, thick, pair[0], pair[1]);
            ret.add_stencil(heap, &line);
        }
    }
    ret
}

// --- the Scheme face ---------------------------------------------------------------

/// The stencil, after the warnings `round_filled_box` gave, through
/// `(lily)`'s `%warned`, which calls `ly:warning` with each.
fn warned(cx: &mut Cx, s: Stencil, warnings: Vec<String>) -> R {
    let value = wrap(cx, s)?;
    crate::warned(cx, value, &warnings)
}

/// `(ly:round-filled-box xext yext blot)`.
pub(crate) fn ly_round_filled_box(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:round-filled-box";
    let (x0, x1) = number_pair_arg(cx, SUBR, 1, args[0])?;
    let (y0, y1) = number_pair_arg(cx, SUBR, 2, args[1])?;
    let blot = number_arg(cx, SUBR, 3, args[2])?;
    let mut warnings = Vec::new();
    let b = Box::new(Interval::new(x0, x1), Interval::new(y0, y1));
    let s = round_filled_box(cx.heap_mut(), b, blot, &mut warnings);
    warned(cx, s, warnings)
}

/// `(%points-to-line-stencil thick points)`: [`points_to_line_stencil`]
/// of the list `points`, pairs of reals.
pub(crate) fn ly_points_to_line_stencil(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "%points-to-line-stencil";
    let thick = number_arg(cx, SUBR, 1, args[0])?;
    let mut points = Vec::new();
    let mut rest = args[1];
    while let (Some(p), Some(next)) = (cx.heap().car(rest), cx.heap().cdr(rest)) {
        points.push(crate::offset_arg(cx, SUBR, 2, p)?);
        rest = next;
    }
    let s = points_to_line_stencil(cx.heap_mut(), thick, &points);
    wrap(cx, s)
}

/// `(ly:round-polygon points blot [extroversion [filled]])`: the items of
/// `points` that are pairs, as offsets; the others are left out.
pub(crate) fn ly_round_polygon(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:round-polygon";
    // `scm_ilength`: a proper list, an improper or circular one none.
    let heap = cx.heap();
    let mut items = Vec::new();
    let (mut list, mut slow) = (args[0], args[0]);
    let mut circular = false;
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        items.push(item);
        list = rest;
        if items.len() % 2 == 0 {
            slow = heap.cdr(slow).unwrap_or(Value::NULL);
            if slow == list {
                circular = true;
                break;
            }
        }
    }
    if items.is_empty() || circular || list != Value::NULL {
        return Err(cx.wrong_type("ly_round_polygon", 1, args[0], "list of coordinate pairs"));
    }
    let blot = number_arg(cx, SUBR, 2, args[1])?;
    let extroversion = match args.get(2) {
        Some(&e) => number_arg(cx, SUBR, 3, e)?,
        None => 0.0,
    };
    let filled = match args.get(3) {
        Some(&f) if f == Value::TRUE || f == Value::FALSE => f == Value::TRUE,
        Some(&f) => return Err(cx.wrong_type(SUBR, 4, f, "boolean")),
        None => true,
    };
    let mut points = Vec::with_capacity(items.len());
    for item in items {
        let heap = cx.heap();
        if let Some((a, d)) = heap.car(item).zip(heap.cdr(item)) {
            points.push((to_double(cx, a)?, to_double(cx, d)?));
        }
    }
    let s = round_polygon(cx.heap_mut(), &points, blot, extroversion, filled);
    wrap(cx, s)
}

/// `(ly:bracket axis extent thickness protrusion)`, its corners rounded by
/// 0.95 of the thickness. An infinite extent gives an empty stencil,
/// without LilyPond's programming error.
pub(crate) fn ly_bracket(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:bracket";
    let a = axis_arg(cx, SUBR, 1, args[0])?;
    let (l, r) = number_pair_arg(cx, SUBR, 2, args[1])?;
    let t = number_arg(cx, SUBR, 3, args[2])?;
    let p = number_arg(cx, SUBR, 4, args[3])?;
    if l.is_infinite() || r.is_infinite() {
        return wrap(cx, Stencil::default());
    }
    let mut warnings = Vec::new();
    let s = bracket(
        cx.heap_mut(),
        a,
        Interval::new(l, r),
        t,
        p,
        0.95 * t,
        &mut warnings,
    );
    warned(cx, s, warnings)
}

/// `(ly:line-interface::line grob startx starty endx endy)`: its arguments
/// checked as LilyPond checks them, then the line drawn on the grob's
/// staff symbol and properties (`line.rs`' `grob_line`, since MS3 step 52,
/// OPEN A23) over `%host-call`.
pub(crate) fn ly_line_interface_line(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:line-interface::line";
    crate::grob::grob_arg(cx, SUBR, 1, args[0])?;
    for (i, &v) in args.iter().enumerate().skip(1) {
        number_arg(cx, SUBR, i + 1, v)?;
    }
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("line-interface-line");
    Ok(cx.tail_call(f, &[what, args[0], args[1], args[2], args[3], args[4]]))
}
