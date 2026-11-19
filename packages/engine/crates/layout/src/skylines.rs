//! `stencil-integral.cc`'s default skylines (MS3 step 26, D111): a stencil's
//! expression walked into segments ([`interpret_stencil_for_skyline`]),
//! which `lazy-skyline-pair.hh`'s [`LazySkylinePair`] collects per side and
//! builds into `crates/skyline`'s [`SkylinePair`] once, through layout's
//! [`Transform`], whose products are fused as clang fuses LilyPond's. Every
//! head the walk knows adds its segments: lines (`draw-line`,
//! `dashed-line`), round filled boxes, polygons and paths (their lines and
//! Bézier curves, `all_commands_to_absolute_and_group`), circles, ellipses
//! and partial ellipses, and glyphs, whose outlines `crates/skyline` reads
//! from W2's packs as FreeType loads them (`named-glyph`, and a
//! `glyph-string`'s glyphs, in an Emmentaler face or a text face, whose
//! packs hold their outlines since D209); a glyph without ink adds its
//! kerned box, as LilyPond does.

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_fonts::Outlines;
use lily_engine_scheme::{Cx, Foreign, Heap, Thrown, Value};
use lily_engine_skyline::{
    Axis as Horizon, Direction, OutlineElement, Skyline, SkylinePair, curve_point, glyph_outline,
    glyph_outline_bbox, glyph_unscaled_box,
};

use crate::font::{FontPack, metric};
use crate::geometry::{
    Axis, Box, Interval, Offset, Transform, X, Y, fma, length, max, offset_directed, other_axis,
};
use crate::pango::{TextPack, pango_font, scaled_glyph_box};
use crate::stencil::Stencil;

type R = Result<Value, Thrown>;

/// `QUANTIZATION_UNIT`: about the length of the segments a rounded corner or
/// a curve is cut into.
const QUANTIZATION_UNIT: f64 = 0.2;

/// A segment as `crates/skyline` takes it, its two ends `[x, y]`.
type Segment = [[f64; 2]; 2];

/// `Orientation`: which way a contour runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Counterclockwise.
    Ccw,
    /// Clockwise.
    Cw,
}

/// `Lazy_skyline_pair`: segments collected per side, the skylines built
/// from them at [`merge`](LazySkylinePair::merge). A segment that bounds
/// both sides waits in `todo`; a contour's or a thick line's edge bounds one.
pub struct LazySkylinePair {
    axis: Axis,
    todo: Vec<Segment>,
    /// Down (left), up (right).
    per_dir_todo: [Vec<Segment>; 2],
    skylines: SkylinePair,
}

fn at(p: Offset, a: Axis) -> f64 {
    if a == X { p.0 } else { p.1 }
}

fn at_mut(p: &mut Offset, a: Axis) -> &mut f64 {
    if a == X { &mut p.0 } else { &mut p.1 }
}

fn segment(p1: Offset, p2: Offset) -> Segment {
    [[p1.0, p1.1], [p2.0, p2.1]]
}

impl LazySkylinePair {
    /// For skylines along the horizon axis `a`: `X` for those below and
    /// above, `Y` for those left and right.
    pub fn new(a: Axis) -> LazySkylinePair {
        LazySkylinePair {
            axis: a,
            todo: Vec::new(),
            per_dir_todo: [Vec::new(), Vec::new()],
            skylines: SkylinePair::default(),
        }
    }

    pub fn axis(&self) -> Axis {
        self.axis
    }

    /// `empty`: whether no segment waits; what was merged does not count.
    pub fn is_empty(&self) -> bool {
        self.todo.is_empty() && self.per_dir_todo.iter().all(Vec::is_empty)
    }

    /// `add_segment (tr, p1, p2)`: a segment that bounds both sides.
    pub fn add_segment(&mut self, tr: &Transform, p1: Offset, p2: Offset) {
        self.todo.push(segment(tr.apply(p1), tr.apply(p2)));
    }

    /// `add_contour_segment`: a segment of a contour running `orientation`,
    /// which bounds the one side its direction along the axis gives.
    pub fn add_contour_segment(
        &mut self,
        tr: &Transform,
        orientation: Orientation,
        p1: Offset,
        p2: Offset,
    ) {
        let (p1, p2) = (tr.apply(p1), tr.apply(p2));
        let reversed = at(p1, self.axis) > at(p2, self.axis);
        // per_dir_todo_[(a_ == X_AXIS) ? UP : LEFT] when the segment runs
        // against the orientation, else [(a_ == X_AXIS) ? DOWN : RIGHT];
        // LEFT is DOWN and RIGHT is UP.
        let up = (reversed == (orientation == Orientation::Ccw)) == (self.axis == X);
        self.per_dir_todo[usize::from(up)].push(segment(p1, p2));
    }

    /// `add_segment (tr, p1, p2, thickness)`: a line `thickness` thick, its
    /// half thickness as `tr` maps it widening it along the axis and
    /// moving it to each side across it. No thickness is
    /// [`add_segment`](LazySkylinePair::add_segment).
    pub fn add_thick_segment(&mut self, tr: &Transform, p1: Offset, p2: Offset, thickness: f64) {
        if thickness == 0.0 {
            self.add_segment(tr, p1, p2);
            return;
        }
        let (end, origin) = (tr.apply((thickness / 2.0, 0.0)), tr.apply((0.0, 0.0)));
        let radius = length((end.0 - origin.0, end.1 - origin.1));

        let a = self.axis;
        let (mut p1, mut p2) = (tr.apply(p1), tr.apply(p2));
        if at(p1, a) > at(p2, a) {
            core::mem::swap(&mut p1, &mut p2);
        }
        *at_mut(&mut p1, a) -= radius;
        *at_mut(&mut p2, a) += radius;

        let across = other_axis(a);
        for (d, sign) in [(0, -1.0), (1, 1.0)] {
            let (mut q1, mut q2) = (p1, p2);
            *at_mut(&mut q1, across) += sign * radius;
            *at_mut(&mut q2, across) += sign * radius;
            self.per_dir_todo[d].push(segment(q1, q2));
        }
    }

    /// `add_box`: the box's sides as a clockwise contour.
    pub fn add_box(&mut self, tr: &Transform, b: Box) {
        let (x, y) = (b.x(), b.y());
        let points = [
            (x.left, y.left),
            (x.left, y.right),
            (x.right, y.right),
            (x.right, y.left),
        ];
        for i in 0..4 {
            self.add_contour_segment(tr, Orientation::Cw, points[i], points[(i + 1) % 4]);
        }
    }

    /// `merge`: each side's waiting segments and those of both sides built
    /// into a skyline and merged into what that side has.
    pub fn merge(&mut self) {
        let horizon = horizon(self.axis);
        for (d, sky) in [(0, Direction::Down), (1, Direction::Up)] {
            if self.todo.is_empty() && self.per_dir_todo[d].is_empty() {
                continue;
            }
            let mut segments = core::mem::take(&mut self.per_dir_todo[d]);
            segments.extend_from_slice(&self.todo);
            let built = lily_engine_skyline::Skyline::from_segments(&segments, horizon, sky);
            self.skylines[sky].merge(&built);
        }
        self.todo.clear();
    }

    /// `to_pair`: the skylines, once what waits is merged.
    pub fn to_pair(&mut self) -> SkylinePair {
        self.merge();
        self.skylines.clone()
    }
}

/// The item `i` of a list, or `()`.
fn nth(heap: &Heap, mut list: Value, i: usize) -> Value {
    for _ in 0..i {
        list = heap.cdr(list).unwrap_or(Value::NULL);
    }
    heap.car(list).unwrap_or(Value::NULL)
}

/// `from_scm<double> (v, 0.0)`: a number as a double, 0 for what is none.
fn number(heap: &Heap, v: Value) -> f64 {
    heap.to_f64(v).unwrap_or(0.0)
}

/// `from_scm (v, Offset (0.0, 0.0))`: a pair of numbers, else the origin.
fn offset_or_origin(heap: &Heap, v: Value) -> Offset {
    match (heap.car(v), heap.cdr(v)) {
        (Some(a), Some(d)) if heap.is_number(a) && heap.is_number(d) => {
            (number(heap, a), number(heap, d))
        }
        _ => (0.0, 0.0),
    }
}

/// `interpret_stencil_for_skyline`: the segments of the stencil expression
/// `expr` under `transform`, added to `skyline`. The empty list and the
/// empty string, a blank's, add nothing, and so does what is no list,
/// where LilyPond's `scm_car` would throw. A head the walk does not know
/// adds nothing, as LilyPond's, which leaves checking the expressions to
/// `stencil-expression.cc`. `warnings` takes LilyPond's warnings, which
/// the caller gives.
pub fn interpret_stencil_for_skyline(
    heap: &Heap,
    skyline: &mut LazySkylinePair,
    transform: &Transform,
    expr: Value,
    warnings: &mut Vec<String>,
) {
    if heap.str(expr).is_some_and(|s| s.is_empty()) {
        return;
    }
    let Some(head) = heap.car(expr) else {
        return;
    };
    let args = heap.cdr(expr).unwrap_or(Value::NULL);
    match heap.name(head).unwrap_or("") {
        "combine-stencil" => {
            let mut s = args;
            while let Some(part) = heap.car(s) {
                interpret_stencil_for_skyline(heap, skyline, transform, part, warnings);
                s = heap.cdr(s).unwrap_or(Value::NULL);
            }
        }
        "translate-stencil" => {
            let p = offset_or_origin(heap, nth(heap, expr, 1));
            let mut local = *transform;
            local.translate(p);
            interpret_stencil_for_skyline(heap, skyline, &local, nth(heap, expr, 2), warnings);
        }
        "scale-stencil" => {
            let factors = nth(heap, expr, 1);
            let x = number(heap, nth(heap, factors, 0));
            let y = number(heap, nth(heap, factors, 1));
            let mut local = *transform;
            local.scale(x, y);
            interpret_stencil_for_skyline(heap, skyline, &local, nth(heap, expr, 2), warnings);
        }
        "rotate-stencil" => {
            let rotation = nth(heap, expr, 1);
            let angle = number(heap, nth(heap, rotation, 0));
            let center = offset_or_origin(heap, nth(heap, rotation, 1));
            let mut local = *transform;
            local.rotate(angle, center);
            interpret_stencil_for_skyline(heap, skyline, &local, nth(heap, expr, 2), warnings);
        }
        "grob-cause" | "color" | "output-attributes" => {
            interpret_stencil_for_skyline(heap, skyline, transform, nth(heap, expr, 2), warnings);
        }
        "utf-8-string" => {
            interpret_stencil_for_skyline(heap, skyline, transform, nth(heap, expr, 3), warnings);
        }
        // The outline, not the stencil it stands for.
        "with-outline" => {
            interpret_stencil_for_skyline(heap, skyline, transform, nth(heap, expr, 1), warnings);
        }
        "draw-line" => add_draw_line_segments(heap, skyline, transform, args),
        "dashed-line" => {
            // (dashed-line thick on off x y phase): the whole line, from
            // the origin.
            let th = number(heap, nth(heap, args, 0));
            let x = number(heap, nth(heap, args, 3));
            let y = number(heap, nth(heap, args, 4));
            skyline.add_thick_segment(transform, (0.0, 0.0), (x, y), th);
        }
        "circle" => {
            // (circle radius thickness filled)
            let rad = number(heap, nth(heap, args, 0));
            let th = number(heap, nth(heap, args, 1));
            add_partial_ellipse_segments(
                skyline,
                transform,
                (rad, rad),
                0.0,
                360.0,
                th,
                false,
                true,
            );
        }
        "ellipse" => {
            // (ellipse x-radius y-radius thickness filled)
            let [x_rad, y_rad, th] = [0, 1, 2].map(|i| number(heap, nth(heap, args, i)));
            add_partial_ellipse_segments(
                skyline,
                transform,
                (x_rad, y_rad),
                0.0,
                360.0,
                th,
                false,
                true,
            );
        }
        "partial-ellipse" => add_partial_ellipse_segments_scm(heap, skyline, transform, args),
        "round-filled-box" => add_round_filled_box_segments(heap, skyline, transform, args),
        "named-glyph" => add_named_glyph_segments(heap, skyline, transform, args),
        "polygon" => add_polygon_segments(heap, skyline, transform, args, warnings),
        "path" => add_path_segments(heap, skyline, transform, args, warnings),
        "glyph-string" => add_glyph_string_segments(heap, skyline, transform, args),
        _ => {}
    }
}

/// `add_draw_line_segments`: `(thick x0 y0 x1 y1)`.
fn add_draw_line_segments(
    heap: &Heap,
    skyline: &mut LazySkylinePair,
    transform: &Transform,
    expr: Value,
) {
    let [thick, x0, y0, x1, y1] = [0, 1, 2, 3, 4].map(|i| number(heap, nth(heap, expr, i)));
    skyline.add_thick_segment(transform, (x0, y0), (x1, y1), thick);
}

/// `add_round_filled_box_segments`: `(left right bottom top diameter)`, the
/// box from `-left` to `right` and `-bottom` to `top`; empty, nothing. A
/// box neither rotated nor rounded (its blot under half a unit as
/// `transform` scales it) is its contour; else its four sides, shortened by
/// the blot's radius when the corners are cut into segments, each a
/// contour's, and each rounded corner's segments, which bound both sides.
fn add_round_filled_box_segments(
    heap: &Heap,
    skyline: &mut LazySkylinePair,
    transform: &Transform,
    expr: Value,
) {
    let [left, right, bottom, top, diameter] =
        [0, 1, 2, 3, 4].map(|i| number(heap, nth(heap, expr, i)));
    let x = Interval::new(-left, right);
    let y = Interval::new(-bottom, top);
    if x.is_empty() || y.is_empty() {
        return;
    }

    // sqrt (sqr (a) + sqr (b)): the products are sqr's, not fused.
    let t = transform;
    let x_scale = libm::sqrt(t.xx * t.xx + t.yx * t.yx);
    let y_scale = libm::sqrt(t.xy * t.xy + t.yy * t.yy);
    let rounded = diameter * max(x_scale, y_scale) > 0.5;
    let rotated = t.yx != 0.0 || t.xy != 0.0;

    if !rotated && !rounded {
        skyline.add_box(transform, Box::new(x, y));
        return;
    }

    let rounded = if rounded { 1.0 } else { 0.0 };
    let quantization = max(
        0.0,
        rounded * diameter * (x_scale + y_scale) * core::f64::consts::PI / QUANTIZATION_UNIT / 8.0,
    ) as usize;
    // No segments, no rounded corners: the sides meet.
    let radius = if quantization != 0 {
        diameter / 2.0
    } else {
        0.0
    };

    let points = [
        (-left, -bottom + radius),
        (-left, top - radius),
        (-left + radius, top),
        (right - radius, top),
        (right, top - radius),
        (right, -bottom + radius),
        (right - radius, -bottom),
        (-left + radius, -bottom),
    ];
    for side in points.chunks_exact(2) {
        skyline.add_contour_segment(transform, Orientation::Cw, side[0], side[1]);
    }

    if radius == 0.0 {
        return;
    }
    let cx = [-left + radius, right - radius];
    let cy = [-bottom + radius, top - radius];
    for (v, vsign) in [(0, -1.0), (1, 1.0)] {
        for (h, hsign) in [(0, -1.0), (1, 1.0)] {
            let mut last = (0.0, 0.0);
            for i in 0..=quantization {
                let angle = linear_interpolate(i as f64, 0.0, quantization as f64, 0.0, 90.0);
                let pt = offset_directed(angle);
                let pt = (pt.0 * radius, pt.1 * radius);
                let inter = (cx[h] + hsign * pt.0, cy[v] + vsign * pt.1);
                if i > 0 {
                    skyline.add_segment(transform, last, inter);
                }
                last = inter;
            }
        }
    }
}

/// `linear_interpolate` (misc.hh): `x` from `x1`–`x2` onto `y1`–`y2`, the
/// first product fused into the sum as clang fuses it.
fn linear_interpolate(x: f64, x1: f64, x2: f64, y1: f64, y2: f64) -> f64 {
    fma((x2 - x) / (x2 - x1), y1, (x - x1) / (x2 - x1) * y2)
}

/// `add_partial_ellipse_segments_scm`: `(x-radius y-radius start end
/// thickness connect filled)`, the two flags true only if `#t`.
fn add_partial_ellipse_segments_scm(
    heap: &Heap,
    skyline: &mut LazySkylinePair,
    transform: &Transform,
    expr: Value,
) {
    let [x_rad, y_rad, start, end, th] = [0, 1, 2, 3, 4].map(|i| number(heap, nth(heap, expr, i)));
    let connect = nth(heap, expr, 5) == Value::TRUE;
    let fill = nth(heap, expr, 6) == Value::TRUE;
    add_partial_ellipse_segments(
        skyline,
        transform,
        (x_rad, y_rad),
        start,
        end,
        th,
        connect,
        fill,
    );
}

/// `add_partial_ellipse_segments`: the arc of radii `rad` from `start` to
/// `end` degrees (a whole turn when they are equal) cut into segments
/// about `QUANTIZATION_UNIT` long as `transform` scales them, each `th`
/// thick, and the chord between its ends if it is connected or filled.
#[allow(clippy::too_many_arguments)]
fn add_partial_ellipse_segments(
    skyline: &mut LazySkylinePair,
    transform: &Transform,
    rad: Offset,
    start: f64,
    end: f64,
    th: f64,
    connect: bool,
    fill: bool,
) {
    let end = if end == start { end + 360.0 } else { end };

    // sqrt (sqr (a) + sqr (b)): the products are sqr's, not fused.
    let t = transform;
    let x_scale = libm::sqrt(t.xx * t.xx + t.yx * t.yx);
    let y_scale = libm::sqrt(t.xy * t.xy + t.yy * t.yy);
    let quantization = max(
        1.0,
        fma(rad.0, x_scale, rad.1 * y_scale) * core::f64::consts::PI / QUANTIZATION_UNIT,
    ) as usize;

    let mut last = (0.0, 0.0);
    let mut first = (0.0, 0.0);
    for i in 0..=quantization {
        let angle = linear_interpolate(i as f64, 0.0, quantization as f64, start, end);
        let pt = offset_directed(angle);
        let pt = (pt.0 * rad.0, pt.1 * rad.1);
        if i > 0 {
            skyline.add_thick_segment(transform, last, pt, th);
        } else {
            first = pt;
        }
        last = pt;
    }
    if connect || fill {
        skyline.add_thick_segment(transform, first, last, th);
    }
}

/// `add_draw_bezier_segments`: the curve through `control` cut into
/// segments about `QUANTIZATION_UNIT` long, its control polygon measured as
/// `transform` maps it, each `th` thick.
fn add_draw_bezier_segments(
    skyline: &mut LazySkylinePair,
    transform: &Transform,
    th: f64,
    control: [Offset; 4],
) {
    let mut len = 0.0;
    let mut last = (0.0, 0.0);
    for (i, &c) in control.iter().enumerate() {
        let transformed = transform.apply(c);
        if i > 0 {
            len += length((transformed.0 - last.0, transformed.1 - last.1));
        }
        last = transformed;
    }
    let quantization = max(0.0, len / QUANTIZATION_UNIT) as usize;

    let curve = control.map(|(x, y)| [x, y]);
    let mut last = control[0];
    for i in 1..quantization {
        let t = i as f64 / quantization as f64;
        let [x, y] = curve_point(&curve, t);
        skyline.add_thick_segment(transform, last, (x, y), th);
        last = (x, y);
    }
    skyline.add_thick_segment(transform, last, control[3], th);
}

/// `get_number_list`: the first list in the nested list `l` that starts
/// with a number, or `#f` (`None`).
fn get_number_list(heap: &Heap, l: Value) -> Option<Value> {
    let (head, tail) = (heap.car(l)?, heap.cdr(l)?);
    if heap.is_number(head) {
        return Some(l);
    }
    get_number_list(heap, head).or_else(|| get_number_list(heap, tail))
}

/// `add_polygon_segments`: `(coords blot filled)`, the first list of
/// numbers in `coords` as `x y` pairs, the path through them closed
/// (`moveto`, `lineto`s, `closepath`), its sides `blot` thick. Where
/// `coords` holds an odd number of items, the last, where LilyPond's
/// `scm_cadr` would throw, is left out.
fn add_polygon_segments(
    heap: &Heap,
    skyline: &mut LazySkylinePair,
    transform: &Transform,
    expr: Value,
    warnings: &mut Vec<String>,
) {
    let mut coords = get_number_list(heap, nth(heap, expr, 0)).unwrap_or(Value::NULL);
    let blot = nth(heap, expr, 1);

    let mut path = Vec::new();
    while let (Some(x), Some(rest)) = (heap.car(coords), heap.cdr(coords)) {
        let Some(y) = heap.car(rest) else {
            break;
        };
        path.push(PathItem::Command(if path.is_empty() {
            "moveto"
        } else {
            "lineto"
        }));
        path.push(PathItem::Number(number(heap, x)));
        path.push(PathItem::Number(number(heap, y)));
        coords = heap.cdr(rest).unwrap_or(Value::NULL);
    }
    path.push(PathItem::Command("closepath"));
    internal_add_path_segments(heap, skyline, transform, blot, &path, warnings);
}

/// The commands of a path.
const PATH_COMMANDS: [&str; 7] = [
    "moveto",
    "rmoveto",
    "lineto",
    "rlineto",
    "curveto",
    "rcurveto",
    "closepath",
];

/// `get_path_list`: the first list in the nested list `l` that starts with
/// a path's command, or `#f` (`None`).
fn get_path_list(heap: &Heap, mut l: Value) -> Option<Value> {
    while let (Some(head), Some(tail)) = (heap.car(l), heap.cdr(l)) {
        if heap.name(head).is_some_and(|n| PATH_COMMANDS.contains(&n)) {
            return Some(l);
        }
        if let Some(found) = get_path_list(heap, head) {
            return Some(found);
        }
        l = tail;
    }
    None
}

/// `add_path_segments`: `(thickness commands …)`, the first list of
/// commands after the thickness as `internal_add_path_segments` adds it. A
/// command's symbol is its name; any other item reads as a number,
/// `from_scm<double> (…, 0.0)`.
fn add_path_segments(
    heap: &Heap,
    skyline: &mut LazySkylinePair,
    transform: &Transform,
    expr: Value,
    warnings: &mut Vec<String>,
) {
    let blot = nth(heap, expr, 0);
    let mut l = get_path_list(heap, heap.cdr(expr).unwrap_or(Value::NULL)).unwrap_or(Value::NULL);
    let mut path = Vec::new();
    while let (Some(item), Some(tail)) = (heap.car(l), heap.cdr(l)) {
        path.push(match heap.name(item) {
            Some(name) => PathItem::Command(name),
            None => PathItem::Number(number(heap, item)),
        });
        l = tail;
    }
    internal_add_path_segments(heap, skyline, transform, blot, &path, warnings);
}

/// `ly_FT_add_outline_to_skyline` of glyph `index` in `font`, a CFF font:
/// `Path_interpreter` over its outline, contours counterclockwise as
/// PostScript fonts have them, each curve cut into at least two segments
/// about `QUANTIZATION_UNIT` long as `transform` maps its ends, the last of
/// which bounds both sides. An empty outline adds nothing.
fn add_outline_to_skyline(
    skyline: &mut LazySkylinePair,
    transform: &Transform,
    font: &impl Outlines,
    index: usize,
) {
    let orientation = Orientation::Ccw;
    let mut cur = (0.0, 0.0);
    glyph_outline(font, index, |element| match element {
        OutlineElement::Move([x, y]) => cur = (x, y),
        OutlineElement::Line([x, y]) => {
            skyline.add_contour_segment(transform, orientation, cur, (x, y));
            cur = (x, y);
        }
        OutlineElement::Cubic(c1, c2, to) => {
            let control = [[cur.0, cur.1], c1, c2, to];
            let start = transform.apply(cur);
            let end = transform.apply((to[0], to[1]));
            let quantization = ((length((end.0 - start.0, end.1 - start.1)) / 0.2) as i32).max(2);
            for i in 1..quantization {
                let [x, y] = curve_point(&control, f64::from(i) / f64::from(quantization));
                skyline.add_contour_segment(transform, orientation, cur, (x, y));
                cur = (x, y);
            }
            skyline.add_segment(transform, cur, (to[0], to[1]));
            cur = (to[0], to[1]);
        }
    });
}

/// A box of `crates/fonts`' as layout's.
fn layout_box(b: lily_engine_fonts::Box) -> Box {
    Box::new(
        Interval::new(b.x.left, b.x.right),
        Interval::new(b.y.left, b.y.right),
    )
}

/// `add_named_glyph_segments`: `(font name)`, the glyph's outline scaled so
/// that its exact width is that of its control box at the font's
/// magnification. A glyph the font has not, or what is no font, adds
/// nothing; an unscaled `Open_type_font`, on which LilyPond's cast to a
/// `Modified_font_metric` fails, is read at magnification 1.
fn add_named_glyph_segments(
    heap: &Heap,
    skyline: &mut LazySkylinePair,
    transform: &Transform,
    expr: Value,
) {
    let Some(m) = metric(heap, nth(heap, expr, 0)) else {
        return;
    };
    let Some(index) = heap
        .str(nth(heap, expr, 1))
        .and_then(|name| m.font.name_to_index(name.as_str()))
    else {
        return;
    };
    let mut bbox = layout_box(glyph_unscaled_box(m.font, index));
    bbox.scale(m.magnification * m.font.design_size() / f64::from(m.font.units_per_em()));
    let real_bbox = layout_box(glyph_outline_bbox(m.font, index));
    let scale = bbox.x().length() / real_bbox.x().length();

    let mut local = *transform;
    local.scale(scale, scale);
    add_outline_to_skyline(skyline, &local, m.font, index);
}

/// `add_glyph_string_segments`: `(font name size cid? glyphs …)`, each
/// glyph `(width (down . up) x-offset y-offset index name)` placed after the
/// widths before it. A glyph without ink adds its kerned box; one with ink,
/// its outline moved and scaled from the face's units onto its ink as Pango
/// gives it: an Emmentaler face's from its [`FontPack`], a text face's from
/// its pack. A glyph the face has not adds nothing, as FreeType loads none;
/// what is no Pango font adds nothing. Until a text face's outlines are
/// given, a glyph that needs one notes it on its face (D262).
fn add_glyph_string_segments(
    heap: &Heap,
    skyline: &mut LazySkylinePair,
    transform: &Transform,
    expr: Value,
) {
    let Some(p) = pango_font(heap, nth(heap, expr, 0)) else {
        return;
    };
    let Some(face) = heap.foreign::<TextPack>(p.face) else {
        return;
    };
    let music = face
        .music
        .and_then(|pack| heap.foreign::<FontPack>(pack))
        .map(|pack| &pack.font);

    let mut cumulative_x = 0.0;
    let mut s = nth(heap, expr, 4);
    while let (Some(now), Some(rest)) = (heap.car(s), heap.cdr(s)) {
        s = rest;
        let width = number(heap, nth(heap, now, 0));
        let height = offset_or_origin(heap, nth(heap, now, 1));
        let xo = number(heap, nth(heap, now, 2));
        let yo = number(heap, nth(heap, now, 3));
        let index = nth(heap, now, 4);
        let index = if heap.is_exact_integer(index) {
            heap.to_i64(index).unwrap_or(0)
        } else {
            0
        };

        let pt0 = (cumulative_x + xo, height.0 + yo);
        let pt1 = (cumulative_x + width + xo, height.1 + yo);
        cumulative_x += width;
        let mut kerned_bbox = Box::EMPTY;
        kerned_bbox.add_point(pt0);
        kerned_bbox.add_point(pt1);

        let Ok(gidx) = usize::try_from(index) else {
            continue;
        };
        let real_bbox = scaled_glyph_box(p, &face.font, gidx);
        let outlined = match music {
            Some(font) => gidx < font.count(),
            None => gidx < face.font.glyphs().len(),
        };
        // A text face whose outlines no host gave yet (D262): a glyph that
        // draws something needs its outline, which the face notes for the
        // core to ask for, and meanwhile adds its kerned box. One that
        // draws nothing adds its kerned box either way.
        let ink = face.font.glyphs().get(gidx).map_or([0; 4], |g| g.ink);
        if music.is_none() && outlined && !face.font.has_outlines() && ink != [0; 4] {
            face.wants_outlines.set(true);
            skyline.add_box(transform, kerned_bbox);
            continue;
        }
        let bbox = match music {
            Some(font) if outlined => layout_box(glyph_unscaled_box(font, gidx)),
            None if outlined => layout_box(glyph_unscaled_box(&face.font, gidx)),
            _ => face.font.glyphs().get(gidx).map_or(
                Box::new(Interval::new(0.0, 0.0), Interval::new(0.0, 0.0)),
                |g| {
                    let [x0, y0, x1, y1] = g.ink.map(f64::from);
                    Box::new(Interval::new(x0, x1), Interval::new(y0, y1))
                },
            ),
        };

        // The scales may round apart but should be close. NaN for a glyph
        // without ink, which only fills; infinite where Pango gives ink and
        // FreeType none: both its kerned box.
        let xlen = real_bbox.x().length() / bbox.x().length();
        let ylen = real_bbox.y().length() / bbox.y().length();
        if xlen.is_nan() || ylen.is_nan() || xlen.is_infinite() || ylen.is_infinite() {
            skyline.add_box(transform, kerned_bbox);
            continue;
        }
        if !outlined {
            continue;
        }
        let scale_factor = max(xlen, ylen);
        let mut transcopy = *transform;
        transcopy
            .translate((
                kerned_bbox.x().left,
                kerned_bbox.y().left - real_bbox.y().left,
            ))
            .translate((real_bbox.x().left, real_bbox.y().left))
            .scale(scale_factor, scale_factor)
            .translate((-bbox.x().left, -bbox.y().left));
        match music {
            Some(font) => add_outline_to_skyline(skyline, &transcopy, font, gidx),
            None => add_outline_to_skyline(skyline, &transcopy, &face.font, gidx),
        }
    }
}

/// An item of a path's command list: a command's name, or what follows it,
/// as `from_scm<double> (…, 0.0)` reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum PathItem<'a> {
    Command(&'a str),
    Number(f64),
}

/// A path's piece in absolute coordinates: a line's ends, or a curve's
/// start, controls and end.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Piece {
    Line([f64; 4]),
    Curve([f64; 8]),
}

/// `internal_add_path_segments`: the path's lines and curves, each `blot`
/// thick, as `add_draw_line_segments` and `add_draw_bezier_segments` add
/// them.
fn internal_add_path_segments(
    heap: &Heap,
    skyline: &mut LazySkylinePair,
    transform: &Transform,
    blot: Value,
    path: &[PathItem],
    warnings: &mut Vec<String>,
) {
    let thick = number(heap, blot);
    for piece in all_commands_to_absolute_and_group(path, warnings) {
        match piece {
            Piece::Line([x0, y0, x1, y1]) => {
                skyline.add_thick_segment(transform, (x0, y0), (x1, y1), thick);
            }
            Piece::Curve([x0, y0, x1, y1, x2, y2, x3, y3]) => {
                let control = [(x0, y0), (x1, y1), (x2, y2), (x3, y3)];
                add_draw_bezier_segments(skyline, transform, thick, control);
            }
        }
    }
}

/// `all_commands_to_absolute_and_group`: a path's commands as its pieces in
/// absolute coordinates. An `rmoveto` that comes first is a `moveto`; a
/// `closepath` away from the start is a line to it. A command it does not
/// know ends the path, with LilyPond's warning.
fn all_commands_to_absolute_and_group(path: &[PathItem], warnings: &mut Vec<String>) -> Vec<Piece> {
    let arg = |i: usize| match path.get(i) {
        Some(PathItem::Number(x)) => *x,
        _ => 0.0,
    };
    let mut out = Vec::new();
    let mut start = (0.0, 0.0);
    let mut current = (0.0, 0.0);
    let mut first = true;
    let mut i = 0;
    while let Some(&item) = path.get(i) {
        let command = match item {
            PathItem::Command(c) => c,
            PathItem::Number(_) => "",
        };
        match command {
            "moveto" | "rmoveto" => {
                let p = (arg(i + 1), arg(i + 2));
                start = if command == "moveto" || first {
                    p
                } else {
                    (p.0 + current.0, p.1 + current.1)
                };
                current = start;
                i += 3;
            }
            "lineto" | "rlineto" => {
                let mut p = (arg(i + 1), arg(i + 2));
                if command == "rlineto" {
                    p = (p.0 + current.0, p.1 + current.1);
                }
                out.push(Piece::Line([current.0, current.1, p.0, p.1]));
                current = p;
                i += 3;
            }
            "curveto" | "rcurveto" => {
                let mut c = [0.0; 8];
                c[0] = current.0;
                c[1] = current.1;
                for k in 0..6 {
                    c[2 + k] = arg(i + 1 + k);
                    if command == "rcurveto" {
                        c[2 + k] += if k % 2 == 0 { current.0 } else { current.1 };
                    }
                }
                out.push(Piece::Curve(c));
                current = (c[6], c[7]);
                i += 7;
            }
            "closepath" => {
                if current.0 != start.0 || current.1 != start.1 {
                    out.push(Piece::Line([current.0, current.1, start.0, start.1]));
                    current = start;
                }
                i += 1;
            }
            _ => {
                warnings.push(String::from("Malformed path for path stencil."));
                return out;
            }
        }
        first = false;
    }
    out
}

// --- the grobs' default skylines (stencil-integral.cc:768–993) ---------------

/// `skyline.cc`'s smob: a [`Skyline`] as a Scheme value (`ly:skyline?`).
/// A `Skyline_pair` is a pair of two, the one below (left) first
/// ([`skyline_pair_value`]).
#[derive(Clone)]
pub struct SkylineObject(pub Skyline);

impl Foreign for SkylineObject {
    fn type_name(&self) -> &'static str {
        "Skyline"
    }
}

/// The horizon axis of the skylines along `a`.
fn horizon(a: Axis) -> Horizon {
    if a == X { Horizon::X } else { Horizon::Y }
}

/// `to_scm (Skyline_pair)`: the pair of its two skylines' smobs.
pub fn skyline_pair_value(heap: &mut Heap, pair: SkylinePair) -> Value {
    let down = heap.wrap(SkylineObject(pair.down));
    let up = heap.wrap(SkylineObject(pair.up));
    heap.cons(down, up)
}

/// `is_scm<Skyline_pair>`, then `from_scm<Skyline_pair>`: the skylines of
/// a pair of two, `None` for anything else, and LilyPond's error for two
/// the wrong way round.
pub(crate) fn skyline_pair_of(cx: &mut Cx, v: Value) -> Result<Option<SkylinePair>, Thrown> {
    skyline_pair_in(cx.heap(), v)
        .map_err(|message| cx.error("misc-error", Some("from_scm"), message, &[]))
}

/// [`skyline_pair_of`] in `heap`, LilyPond's error its message.
pub(crate) fn skyline_pair_in(heap: &Heap, v: Value) -> Result<Option<SkylinePair>, &'static str> {
    let (Some(a), Some(d)) = (heap.car(v), heap.cdr(v)) else {
        return Ok(None);
    };
    let (Some(down), Some(up)) = (
        heap.foreign::<SkylineObject>(a),
        heap.foreign::<SkylineObject>(d),
    ) else {
        return Ok(None);
    };
    if down.0.sky() != Direction::Down {
        Err("direction of first skyline in skyline pair must be DOWN/LEFT")
    } else if up.0.sky() != Direction::Up {
        Err("direction of second skyline in skyline pair must be UP/RIGHT")
    } else {
        Ok(Some(SkylinePair::new(down.0.clone(), up.0.clone())))
    }
}

/// `Skyline_pair::pad`, with `Skyline::padded`'s warning for a padding
/// below 0 for each side.
pub(crate) fn pad(pair: &mut SkylinePair, r: f64, warnings: &mut Vec<String>) {
    if r < 0.0 {
        warnings.extend([NEGATIVE_PADDING, NEGATIVE_PADDING].map(String::from));
    }
    pair.pad(r);
}

/// `(ly:skyline? x)`.
pub(crate) fn is_skyline(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        cx.heap().foreign::<SkylineObject>(args[0]).is_some(),
    ))
}

/// `(ly:skyline->points skyline horizon-axis)` (`skyline-scheme.cc`): the
/// skyline's points seen with `horizon-axis` as its horizon, each an
/// `(x . y)` of reals.
pub(crate) fn skyline_points(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:skyline->points";
    let Some(sky) = cx.heap().foreign::<SkylineObject>(args[0]) else {
        return Err(cx.wrong_type(SUBR, 1, args[0], "Skyline"));
    };
    let sky = sky.0.clone();
    let a = crate::axis_arg(cx, SUBR, 2, args[1])?;
    let heap = cx.heap_mut();
    let mut points = Vec::new();
    for [x, y] in sky.to_points(horizon(a)) {
        let (x, y) = (heap.flonum(x), heap.flonum(y));
        points.push(heap.cons(x, y));
    }
    Ok(heap.list(&points))
}

// --- skyline-scheme.cc's ly:skyline-* ------------------------------------------

/// The warning of `Skyline::padded` for a padding below 0.
const NEGATIVE_PADDING: &str = "Cannot have negative horizon padding.  Junking.";

/// `LY_ASSERT_TYPE (scm_is_real, …)`, then `from_scm<Real>`.
fn real_arg(cx: &mut Cx, subr: &str, pos: usize, v: Value) -> Result<f64, Thrown> {
    match cx.heap().is_real(v) {
        true => Ok(real(cx, v)),
        false => Err(cx.wrong_type(subr, pos, v, "real number")),
    }
}

/// The optional horizon padding of `subr`, its argument 3: 0 if not given.
fn padding_arg(cx: &mut Cx, subr: &str, args: &[Value]) -> Result<f64, Thrown> {
    match args.get(2) {
        Some(&v) => real_arg(cx, subr, 3, v),
        None => Ok(0.0),
    }
}

/// The two skylines whose distance `subr` measures, which must bound
/// opposite sides: LilyPond asserts it and aborts, the engine throws
/// `misc-error`.
fn facing_args(cx: &mut Cx, subr: &str, args: &[Value]) -> Result<(Skyline, Skyline), Thrown> {
    let sky = skyline_arg(cx, subr, 1, args[0])?;
    let other = skyline_arg(cx, subr, 2, args[1])?;
    if sky.sky() == other.sky() {
        let message = "expecting skylines of opposite directions";
        return Err(cx.error("misc-error", Some(subr), message, &[]));
    }
    Ok((sky, other))
}

/// `value`, after the warning of a padding below 0 if `padding` is one.
fn padding_warned(cx: &mut Cx, value: Value, padding: f64) -> R {
    if padding < 0.0 {
        return crate::warned(cx, value, &[String::from(NEGATIVE_PADDING)]);
    }
    Ok(value)
}

/// `(ly:skyline-touching-point skyline other-skyline [horizon-padding])`:
/// where the two, of opposite sides, reach their distance, `skyline`
/// padded first.
pub(crate) fn ly_skyline_touching_point(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:skyline-touching-point";
    let (sky, other) = facing_args(cx, SUBR, args)?;
    let hp = padding_arg(cx, SUBR, args)?;
    let v = cx.heap_mut().flonum(sky.touching_point(&other, hp));
    padding_warned(cx, v, hp)
}

/// `(ly:skyline-distance skyline other-skyline [horizon-padding])`.
pub(crate) fn ly_skyline_distance(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:skyline-distance";
    let (sky, other) = facing_args(cx, SUBR, args)?;
    let hp = padding_arg(cx, SUBR, args)?;
    let v = cx.heap_mut().flonum(sky.distance(&other, hp));
    padding_warned(cx, v, hp)
}

/// `(ly:skyline-max-height skyline)`.
pub(crate) fn ly_skyline_max_height(cx: &mut Cx, args: &[Value]) -> R {
    let sky = skyline_arg(cx, "ly:skyline-max-height", 1, args[0])?;
    Ok(cx.heap_mut().flonum(sky.max_height()))
}

/// `(ly:skyline-max-height-position skyline)`.
pub(crate) fn ly_skyline_max_height_position(cx: &mut Cx, args: &[Value]) -> R {
    let sky = skyline_arg(cx, "ly:skyline-max-height-position", 1, args[0])?;
    Ok(cx.heap_mut().flonum(sky.max_height_position()))
}

/// `(ly:skyline-height skyline x)`. LilyPond asserts that `x` is finite
/// and the skyline has buildings, and aborts; the engine throws
/// `out-of-range` for the one and `misc-error` for the other.
pub(crate) fn ly_skyline_height(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:skyline-height";
    let sky = skyline_arg(cx, SUBR, 1, args[0])?;
    let x = real_arg(cx, SUBR, 2, args[1])?;
    if x.is_infinite() {
        let two = cx.heap_mut().integer(2);
        return Err(cx.error(
            "out-of-range",
            Some(SUBR),
            "Argument ~A out of range: ~S",
            &[two, args[1]],
        ));
    }
    if sky.to_points(Horizon::X).is_empty() {
        return Err(cx.error("misc-error", Some(SUBR), "skyline without buildings", &[]));
    }
    Ok(cx.heap_mut().flonum(sky.height(x)))
}

/// `(ly:skyline-empty? sky)`.
pub(crate) fn ly_skyline_empty_p(cx: &mut Cx, args: &[Value]) -> R {
    let sky = skyline_arg(cx, "ly:skyline-empty?", 1, args[0])?;
    Ok(Value::bool(sky.is_empty()))
}

/// `(ly:skylines-for-stencil stencil axis)`: `skylines_from_stencil`,
/// unrotated and unpadded ([`stencil_skylines`]).
pub(crate) fn ly_skylines_for_stencil(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:skylines-for-stencil";
    if cx.heap().foreign::<Stencil>(args[0]).is_none() {
        return Err(cx.wrong_type(SUBR, 1, args[0], "Stencil"));
    }
    crate::axis_arg(cx, SUBR, 2, args[1])?;
    let zero = cx.heap_mut().flonum(0.0);
    stencil_skylines(cx, &[args[0], Value::FALSE, args[1], zero])
}

/// `(ly:skyline-pad skyline horizon-padding)`: a copy padded along the
/// horizon (`Skyline::padded`).
pub(crate) fn ly_skyline_pad(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:skyline-pad";
    let sky = skyline_arg(cx, SUBR, 1, args[0])?;
    let hp = real_arg(cx, SUBR, 2, args[1])?;
    let v = skyline_value(cx, sky.padded(hp))?;
    padding_warned(cx, v, hp)
}

/// A proper list's items, `None` for anything else (`ly_is_list`).
fn list_items(heap: &Heap, mut l: Value) -> Option<Vec<Value>> {
    let mut items = Vec::new();
    let mut slow = l;
    while l != Value::NULL {
        items.push(heap.car(l)?);
        l = heap.cdr(l)?;
        if items.len() % 2 == 0 {
            slow = heap.cdr(slow)?;
            if slow == l && l != Value::NULL {
                return None;
            }
        }
    }
    Some(items)
}

/// `(ly:make-skyline segments axis direction)`: the skyline of `segments`,
/// each a list `(x1 y1 x2 y2)` of reals, along the horizon `axis`, bounding
/// the side `direction`. Each segment is checked before the axis and the
/// direction, as LilyPond checks them. LilyPond also takes the direction
/// 0, a skyline of no side; the engine's skylines bound one, and 0 is its
/// type error.
pub(crate) fn ly_make_skyline(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:make-skyline";
    let Some(segments) = list_items(cx.heap(), args[0]) else {
        return Err(cx.wrong_type(SUBR, 1, args[0], "list"));
    };
    let mut offs = Vec::with_capacity(segments.len());
    for segment in segments {
        let xs = match list_items(cx.heap(), segment) {
            Some(xs) if xs.len() == 4 => xs,
            _ => return Err(cx.wrong_type_vm(Some(SUBR), segment, "list of 4 numbers")),
        };
        if let Some(&x) = xs.iter().find(|&&x| !cx.heap().is_real(x)) {
            return Err(cx.wrong_type_vm(Some(SUBR), x, "real number"));
        }
        let [x1, y1, x2, y2] = [xs[0], xs[1], xs[2], xs[3]].map(|x| real(cx, x));
        if (x1.is_infinite() || x2.is_infinite()) && y1 != y2 {
            let message = "building with infinite bound must be horizontal";
            return Err(cx.error("misc-error", Some(SUBR), message, &[]));
        }
        offs.push([[x1, y1], [x2, y2]]);
    }
    let a = crate::axis_arg(cx, SUBR, 2, args[1])?;
    let d = crate::direction_arg(cx, SUBR, 3, args[2])?;
    let sky = match d {
        1 => Direction::Up,
        -1 => Direction::Down,
        _ => return Err(cx.wrong_type(SUBR, 3, args[2], "UP or DOWN")),
    };
    skyline_value(cx, Skyline::from_segments(&offs, horizon(a), sky))
}

/// `(ly:skyline-merge skyline1 skyline2)`: a copy of the first with the
/// second, of the same side, merged in.
pub(crate) fn ly_skyline_merge(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:skyline-merge";
    let mut merged = skyline_arg(cx, SUBR, 1, args[0])?;
    let other = skyline_arg(cx, SUBR, 2, args[1])?;
    if merged.sky() != other.sky() {
        let message = "expecting skylines with the same direction";
        return Err(cx.error("misc-error", Some(SUBR), message, &[]));
    }
    merged.merge(&other);
    skyline_value(cx, merged)
}

// --- one skyline, as align-interface.cc and page-layout-problem.cc use it ---

/// The skyline `v`, the argument `pos` of `subr`.
fn skyline_arg(cx: &mut Cx, subr: &str, pos: usize, v: Value) -> Result<Skyline, Thrown> {
    match cx.heap().foreign::<SkylineObject>(v) {
        Some(sky) => Ok(sky.0.clone()),
        None => Err(cx.wrong_type(subr, pos, v, "Skyline")),
    }
}

fn real(cx: &mut Cx, v: Value) -> f64 {
    cx.heap().to_f64(v).unwrap_or(0.0)
}

fn skyline_value(cx: &mut Cx, sky: Skyline) -> R {
    Ok(cx.heap_mut().wrap(SkylineObject(sky)))
}

/// `(%skyline-empty sky)`: `Skyline (sky)`, bounding the side above for
/// 1 and below for anything else.
pub(crate) fn skyline_empty(cx: &mut Cx, args: &[Value]) -> R {
    let sky = if args[0].as_fixnum() == Some(1) {
        Direction::Up
    } else {
        Direction::Down
    };
    skyline_value(cx, Skyline::empty(sky))
}

/// `(%skyline-distance a b horizon-padding)`: `a.distance (b,
/// horizon-padding)`, with `Skyline::padded`'s warning for a padding below
/// 0.
pub(crate) fn skyline_distance(cx: &mut Cx, args: &[Value]) -> R {
    let a = skyline_arg(cx, "%skyline-distance", 1, args[0])?;
    let b = skyline_arg(cx, "%skyline-distance", 2, args[1])?;
    let padding = real(cx, args[2]);
    let d = a.distance(&b, padding);
    let v = cx.heap_mut().flonum(d);
    if padding < 0.0 {
        let warning = String::from("Cannot have negative horizon padding.  Junking.");
        return crate::warned(cx, v, &[warning]);
    }
    Ok(v)
}

/// `(%skyline-raised sky r)`: a copy of `sky` raised by `r`
/// (`Skyline::raise`).
pub(crate) fn skyline_raised(cx: &mut Cx, args: &[Value]) -> R {
    let mut sky = skyline_arg(cx, "%skyline-raised", 1, args[0])?;
    sky.raise(real(cx, args[1]));
    skyline_value(cx, sky)
}

/// `(%skyline-shifted sky s)`: a copy of `sky` moved by `s` along its
/// horizon (`Skyline::shift`).
pub(crate) fn skyline_shifted(cx: &mut Cx, args: &[Value]) -> R {
    let mut sky = skyline_arg(cx, "%skyline-shifted", 1, args[0])?;
    sky.shift(real(cx, args[1]));
    skyline_value(cx, sky)
}

/// `(%skyline-merged a b)`: a copy of `a` with `b`, which bounds the same
/// side, merged in (`Skyline::merge`); LilyPond asserts the sides, and
/// two sides are the type error of the argument 2 here.
pub(crate) fn skyline_merged(cx: &mut Cx, args: &[Value]) -> R {
    let mut a = skyline_arg(cx, "%skyline-merged", 1, args[0])?;
    let b = skyline_arg(cx, "%skyline-merged", 2, args[1])?;
    if a.sky() != b.sky() {
        return Err(cx.wrong_type("%skyline-merged", 2, args[1], "Skyline of the same side"));
    }
    a.merge(&b);
    skyline_value(cx, a)
}

/// `(%skyline-max-height sky)`: `Skyline::max_height`.
pub(crate) fn skyline_max_height(cx: &mut Cx, args: &[Value]) -> R {
    let sky = skyline_arg(cx, "%skyline-max-height", 1, args[0])?;
    Ok(cx.heap_mut().flonum(sky.max_height()))
}

/// `(%skyline-with-minimum-height sky h)`: a copy of `sky` with a roof at
/// `h` everywhere merged in (`Skyline::set_minimum_height`).
pub(crate) fn skyline_with_minimum_height(cx: &mut Cx, args: &[Value]) -> R {
    let mut sky = skyline_arg(cx, "%skyline-with-minimum-height", 1, args[0])?;
    sky.set_minimum_height(real(cx, args[1]));
    skyline_value(cx, sky)
}

/// `(%boxes-skylines boxes [axis padding])`: `Skyline_pair (boxes, axis)`
/// of the list `boxes`, each `(x-extent . y-extent)`, along `X` if no
/// `axis` is given; boxes empty on either axis are left out. Each side is
/// then `padded` by `padding`, if given, with `Skyline::padded`'s warning
/// for a padding below 0, as `ly:separation-item::calc-skylines` pads them.
pub(crate) fn boxes_skylines(cx: &mut Cx, args: &[Value]) -> R {
    let a = match args.get(1) {
        Some(&v) => crate::axis_arg(cx, "%boxes-skylines", 2, v)?,
        None => X,
    };
    let padding = args.get(2).map(|&v| real(cx, v));
    let heap = cx.heap();
    let end = |v: Option<Value>, none: f64| v.and_then(|e| heap.to_f64(e)).unwrap_or(none);
    let interval = |v: Option<Value>| lily_engine_fonts::Interval {
        left: end(v.and_then(|v| heap.car(v)), f64::INFINITY),
        right: end(v.and_then(|v| heap.cdr(v)), f64::NEG_INFINITY),
    };
    let mut boxes = Vec::new();
    let mut l = args[0];
    while let (Some(b), Some(rest)) = (heap.car(l), heap.cdr(l)) {
        boxes.push(lily_engine_fonts::Box {
            x: interval(heap.car(b)),
            y: interval(heap.cdr(b)),
        });
        l = rest;
    }
    let mut pair = SkylinePair::from_boxes(&boxes, horizon(a));
    let mut warnings = Vec::new();
    if let Some(padding) = padding {
        pad(&mut pair, padding, &mut warnings);
    }
    let v = skyline_pair_value(cx.heap_mut(), pair);
    crate::warned(cx, v, &warnings)
}

/// `(%stencil-skylines stencil rotation axis padding)`:
/// `skylines_from_stencil` along `axis` of `stencil`, rotated by
/// `rotation`, `(angle x y)` or `#f`, as the grob's `rotation` gives it,
/// then padded by `padding` (`Skyline_pair::pad`). What is no stencil has
/// no skylines; a stencil whose expression adds no segment, such as an
/// `embedded-ps`, has those of its unrotated extents, if not empty.
pub(crate) fn stencil_skylines(cx: &mut Cx, args: &[Value]) -> R {
    let a = crate::axis_arg(cx, "%stencil-skylines", 3, args[2])?;
    let padding = cx.heap().to_f64(args[3]).unwrap_or(0.0);
    let mut lazy = LazySkylinePair::new(a);
    let mut warnings = Vec::new();
    if let Some(&s) = cx.heap().foreign::<Stencil>(args[0]) {
        let mut rotated = s;
        if args[1] != Value::FALSE {
            let heap = cx.heap();
            let [angle, x, y] = [0, 1, 2].map(|i| number(heap, nth(heap, args[1], i)));
            rotated.rotate_degrees(cx.heap_mut(), angle, (x, y));
        }
        let identity = Transform::IDENTITY;
        interpret_stencil_for_skyline(cx.heap(), &mut lazy, &identity, rotated.expr, &mut warnings);
        if lazy.is_empty() && !s.is_empty_on(X) && !s.is_empty_on(Y) {
            lazy.add_box(&identity, s.dim);
        }
    }
    let mut pair = lazy.to_pair();
    pad(&mut pair, padding, &mut warnings);
    let v = skyline_pair_value(cx.heap_mut(), pair);
    crate::warned(cx, v, &warnings)
}

/// `skylines_from_stencil (sten, SCM_EOL, a)` (as `%stencil-skylines`
/// makes them, unpadded), its two skylines' `max_height`s: the stencil's
/// true extent on the other axis, where its ink is (MS8 step 8, part 1,
/// for `Line_interface::make_trill_line`).
pub(crate) fn true_extent(heap: &Heap, s: &Stencil, a: Axis) -> Interval {
    let mut lazy = LazySkylinePair::new(a);
    let mut warnings = Vec::new();
    let identity = Transform::IDENTITY;
    interpret_stencil_for_skyline(heap, &mut lazy, &identity, s.expr, &mut warnings);
    if lazy.is_empty() && !s.is_empty_on(X) && !s.is_empty_on(Y) {
        lazy.add_box(&identity, s.dim);
    }
    let pair = lazy.to_pair();
    Interval::new(pair.down.max_height(), pair.up.max_height())
}

/// `(%box-skylines x-extent y-extent axis)`: the skylines along `axis` of
/// the box of the two extents (`Skyline_pair (boxes, a)`), none if either
/// is empty, as `maybe_pure_internal_simple_skylines_from_extents` makes
/// them.
pub(crate) fn box_skylines(cx: &mut Cx, args: &[Value]) -> R {
    let a = crate::axis_arg(cx, "%box-skylines", 3, args[2])?;
    let heap = cx.heap();
    let [x, y] = [args[0], args[1]].map(|v| lily_engine_fonts::Interval {
        left: heap
            .car(v)
            .and_then(|e| heap.to_f64(e))
            .unwrap_or(f64::INFINITY),
        right: heap
            .cdr(v)
            .and_then(|e| heap.to_f64(e))
            .unwrap_or(f64::NEG_INFINITY),
    });
    let pair = if x.left > x.right || y.left > y.right {
        SkylinePair::default()
    } else {
        SkylinePair::from_boxes(&[lily_engine_fonts::Box { x, y }], horizon(a))
    };
    Ok(skyline_pair_value(cx.heap_mut(), pair))
}

/// `(%merged-skylines axis items)`: the skylines of
/// `internal_skylines_from_element_stencils`, each item `(skylines x y)`,
/// an element's skylines and its offset from the grob; skylines that are
/// no pair of skylines are left out, and the others shifted along `axis`,
/// raised across it and merged.
pub(crate) fn merged_skylines(cx: &mut Cx, args: &[Value]) -> R {
    let a = crate::axis_arg(cx, "%merged-skylines", 1, args[0])?;
    let mut res = SkylinePair::default();
    let mut items = args[1];
    while let (Some(item), Some(rest)) = (cx.heap().car(items), cx.heap().cdr(items)) {
        let heap = cx.heap();
        let skyp = nth(heap, item, 0);
        let off = (
            number(heap, nth(heap, item, 1)),
            number(heap, nth(heap, item, 2)),
        );
        if let Some(mut p) = skyline_pair_of(cx, skyp)? {
            p.shift(at(off, a));
            p.raise(at(off, other_axis(a)));
            res.merge(&p);
        }
        items = rest;
    }
    Ok(skyline_pair_value(cx.heap_mut(), res))
}

/// `stencil-integral.cc`'s `Grob::` callbacks of the skylines, under their
/// names and arities (D67): the stencil's, padded by the grob's
/// `skyline-horizontal-padding` or `skyline-vertical-padding`; the box of
/// the grob's extents, which the constructor sets where a grob has none
/// (`%grob-default-skylines`, `grob.cc:79–88`), unbounded across the staff
/// before line breaking and, for a `cross-staff` grob, along it; and the
/// elements' skylines merged at their offsets. Each reads the grob's
/// properties and coordinates in LilyPond's order; a pure one's columns
/// are `from_scm<vsize>`'s, `0` or `INT_MAX` for what is none.
pub(crate) const PRELUDE: &str = r#"
(define (%real-or v fallback) (if (real? v) (exact->inexact v) fallback))
(define (%vsize-or v fallback)
  (if (and (integer? v) (exact? v) (<= 0 v 18446744073709551615)) v fallback))
(define (%grob-stencil-skylines grob a padding)
  (let* ((stencil (ly:grob-property grob 'stencil))
         (rot (ly:grob-property grob 'rotation))
         (rotation (and (ly:stencil? stencil) (pair? rot)
                        (list (%real-or (car rot) 0.0)
                              (%real-or (cadr rot) 0.0)
                              (%real-or (caddr rot) 0.0)))))
    (%stencil-skylines stencil rotation a (%real-or (ly:grob-property grob padding) 0.0))))
(define-public (ly:grob::vertical-skylines-from-stencil grob)
  (%check-type "ly:grob::vertical-skylines-from-stencil" 1 ly:grob? "Grob" grob)
  (%grob-stencil-skylines grob 0 'skyline-horizontal-padding))
(define-public (ly:grob::horizontal-skylines-from-stencil grob)
  (%check-type "ly:grob::horizontal-skylines-from-stencil" 1 ly:grob? "Grob" grob)
  (%grob-stencil-skylines grob 1 'skyline-vertical-padding))
(define (%simple-skylines-from-extents me a pure start end ignore-x ignore-y)
  (let* ((xex (if ignore-x '(-inf.0 . +inf.0) (%grob-extent me me 0)))
         (yex (cond (ignore-y '(-inf.0 . +inf.0))
                    (pure (%grob-pure-y-extent me me start end))
                    (else (%grob-extent me me 1)))))
    (%box-skylines xex yex a)))
(define-public (ly:grob::pure-simple-vertical-skylines-from-extents grob start end)
  (%check-type "ly:grob::pure-simple-vertical-skylines-from-extents" 1 ly:grob? "Grob" grob)
  (%simple-skylines-from-extents grob 0 #t (%vsize-or start 0) (%vsize-or end 2147483647) #t #f))
(define-public (ly:grob::simple-vertical-skylines-from-extents grob)
  (%check-type "ly:grob::simple-vertical-skylines-from-extents" 1 ly:grob? "Grob" grob)
  (%simple-skylines-from-extents grob 0 #f 0 0 #f #f))
(define-public (ly:grob::pure-simple-horizontal-skylines-from-extents grob start end)
  (%check-type "ly:grob::pure-simple-horizontal-skylines-from-extents" 1 ly:grob? "Grob" grob)
  (let ((start (%vsize-or start 0))
        (end (%vsize-or end 2147483647)))
    (%simple-skylines-from-extents grob 1 #t start end #f
                                   (eq? (ly:grob-property grob 'cross-staff) #t))))
(define-public (ly:grob::simple-horizontal-skylines-from-extents grob)
  (%check-type "ly:grob::simple-horizontal-skylines-from-extents" 1 ly:grob? "Grob" grob)
  (%simple-skylines-from-extents grob 1 #f 0 0 #f
                                 (eq? (ly:grob-property grob 'cross-staff) #t)))
(define (%maybe-pure-coordinate grob refp a pure start end)
  (if (and pure (eqv? a 1))
      (%grob-pure-relative-y-coordinate grob refp start end)
      (%grob-relative-coordinate grob refp a)))
(define (%skylines-from-element-stencils me a pure start end)
  (let* ((arr (ly:grob-object me 'elements))
         (elts (if (ly:grob-array? arr) (ly:grob-array->list arr) '()))
         (x-common (if (ly:grob-array? arr) (ly:grob-common-refpoint-of-array me arr 0) me))
         (y-common (if (ly:grob-array? arr) (ly:grob-common-refpoint-of-array me arr 1) me))
         (positions
          (let loop ((es elts) (out '()))
            (if (null? es)
                (reverse! out)
                (let* ((x (%grob-relative-coordinate (car es) x-common 0))
                       (y (%maybe-pure-coordinate (car es) y-common 1 pure start end)))
                  (loop (cdr es) (cons (cons x y) out))))))
         (my-x (%grob-relative-coordinate me x-common 0))
         (my-y (%maybe-pure-coordinate me y-common 1 pure start end))
         (sym (if (eqv? a 0) 'vertical-skylines 'horizontal-skylines)))
    (let loop ((es elts) (ps positions) (items '()))
      (if (null? es)
          (%merged-skylines a (reverse! items))
          (let ((skyp (if pure
                          (%grob-pure-property (car es) sym start end)
                          (ly:grob-property (car es) sym))))
            (loop (cdr es) (cdr ps)
                  (cons (list skyp (- (caar ps) my-x) (- (cdar ps) my-y)) items)))))))
(define-public (ly:grob::vertical-skylines-from-element-stencils grob)
  (%check-type "ly:grob::vertical-skylines-from-element-stencils" 1 ly:grob? "Grob" grob)
  (%skylines-from-element-stencils grob 0 #f 0 2147483647))
(define-public (ly:grob::horizontal-skylines-from-element-stencils grob)
  (%check-type "ly:grob::horizontal-skylines-from-element-stencils" 1 ly:grob? "Grob" grob)
  (%skylines-from-element-stencils grob 1 #f 0 2147483647))
(define-public (ly:grob::pure-vertical-skylines-from-element-stencils grob start end)
  (%check-type "ly:grob::pure-vertical-skylines-from-element-stencils" 1 ly:grob? "Grob" grob)
  (%skylines-from-element-stencils grob 0 #t (%vsize-or start 0) (%vsize-or end 0)))
(define-public (ly:grob::pure-horizontal-skylines-from-element-stencils grob start end)
  (%check-type "ly:grob::pure-horizontal-skylines-from-element-stencils" 1 ly:grob? "Grob" grob)
  (%skylines-from-element-stencils grob 1 #t (%vsize-or start 0) (%vsize-or end 0)))
(define %grob-default-skylines
  (cons (ly:make-unpure-pure-container ly:grob::simple-vertical-skylines-from-extents
                                       ly:grob::pure-simple-vertical-skylines-from-extents)
        (ly:make-unpure-pure-container ly:grob::simple-horizontal-skylines-from-extents
                                       ly:grob::pure-simple-horizontal-skylines-from-extents)))
"#;

#[cfg(test)]
mod tests {
    use super::*;

    /// A path's items from its words, as LilyPond writes them.
    fn path(text: &str) -> Vec<PathItem<'_>> {
        text.split(' ')
            .map(|w| w.parse().map_or(PathItem::Command(w), PathItem::Number))
            .collect()
    }

    #[test]
    fn a_path_is_grouped_into_absolute_pieces() {
        // stencil-integral.cc's example. Each of an rcurveto's points is
        // relative to where the curve starts, so its curve is not the
        // comment's (6 6 5 5 4 4 3 3).
        let path = path(
            "moveto 1 2 lineto 3 4 rlineto -1 -1 curveto 3 3 5 5 6 6 \
             rcurveto -1 -1 -1 -1 -1 -1 closepath",
        );
        let mut warnings = Vec::new();
        assert_eq!(
            all_commands_to_absolute_and_group(&path, &mut warnings),
            [
                Piece::Line([1.0, 2.0, 3.0, 4.0]),
                Piece::Line([3.0, 4.0, 2.0, 3.0]),
                Piece::Curve([2.0, 3.0, 3.0, 3.0, 5.0, 5.0, 6.0, 6.0]),
                Piece::Curve([6.0, 6.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0]),
                Piece::Line([5.0, 5.0, 1.0, 2.0]),
            ]
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn a_first_rmoveto_is_a_moveto_and_an_unknown_command_ends_the_path() {
        let path = path("rmoveto 1 1 rmoveto 1 0 rlineto 0 1 closepath closepath 7 lineto 0 0");
        let mut warnings = Vec::new();
        assert_eq!(
            all_commands_to_absolute_and_group(&path, &mut warnings),
            [
                Piece::Line([2.0, 1.0, 2.0, 2.0]),
                Piece::Line([2.0, 2.0, 2.0, 1.0]),
            ]
        );
        assert_eq!(warnings, ["Malformed path for path stencil."]);
    }
}
