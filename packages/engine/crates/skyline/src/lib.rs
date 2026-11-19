//! The engine's skylines (chapter 06 §5.1, W2). [`Skyline`] is LilyPond's
//! (`skyline.cc`): buildings from -infinity to infinity, built from boxes or
//! segments by sorting, partitioning and merging (`building.rs`), and measured
//! against another (`distance.rs`); [`SkylinePair`] is `skyline-pair.cc`'s.
//! [`glyph_skylines`] is what `ly:skylines-for-stencil` computes for the
//! stencil of `ly:font-get-glyph`: `stencil-integral.cc`'s
//! `add_named_glyph_segments` over the glyph's outline as FreeType loads it
//! unscaled (`freetype.cc`'s `Path_interpreter`), each curve cut into
//! segments, collected by `Lazy_skyline_pair`. Its points are
//! LilyPond's, bit for bit, for every glyph of the eight sizes and the braces
//! (tests/skylines_ly.rs), so the engine computes them at first use instead of
//! shipping them.
//!
//! Two things of the build the references come from, Homebrew's LilyPond 2.26
//! for macOS on arm64, show in the points: clang fuses `a * b + c` written as
//! one expression (`-ffp-contract=on`), and `fma` marks the expressions of
//! `skyline.cc` it fuses; and buildings are sorted by libc++'s unstable
//! `std::sort`, which `sort` ports. Without either, most or some glyphs
//! differ (650 and 29 of emmentaler-20's 663).
#![no_std]
extern crate alloc;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_fonts::{Box, Font, Interval, Outlines, Segment};

use building::{
    Building, build_skyline, empty_skyline, max, merge_buildings, printf_f, single_skyline,
};

mod building;
mod distance;
mod fma;
mod sort;

pub use fma::fma;

use distance::min;

pub use sort::sort;

/// A point, `[x, y]`.
pub type Offset = [f64; 2];

/// The horizon axis: `X` for the skylines above and below a stencil
/// (`vertical-skylines`), `Y` for those left and right of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

impl Axis {
    fn index(self) -> usize {
        self as usize
    }

    fn other(self) -> Axis {
        match self {
            Axis::X => Axis::Y,
            Axis::Y => Axis::X,
        }
    }
}

/// The side a skyline bounds. A `Down` skyline is stored upside down, its
/// heights negated, as skyline.cc stores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Down,
    Up,
}

impl Direction {
    fn sign(self) -> f64 {
        match self {
            Direction::Down => -1.0,
            Direction::Up => 1.0,
        }
    }
}

/// LilyPond's `Skyline`.
#[derive(Clone, Debug, PartialEq)]
pub struct Skyline {
    buildings: Vec<Building>,
    sky: Direction,
}

impl Default for Skyline {
    /// `Skyline ()`: empty, bounding the side above.
    fn default() -> Skyline {
        Skyline::empty(Direction::Up)
    }
}

impl Skyline {
    /// One building at -infinity.
    pub fn empty(sky: Direction) -> Skyline {
        Skyline {
            buildings: empty_skyline(),
            sky,
        }
    }

    /// From boxes, each a flat roof at its `sky` side; boxes empty on either
    /// axis are left out.
    pub fn from_boxes(boxes: &[Box], horizon: Axis, sky: Direction) -> Skyline {
        let buildings = boxes
            .iter()
            .filter(|b| !b.x.is_empty() && !b.y.is_empty())
            .map(|&b| Building::from_box(b, horizon, sky))
            .collect();
        Skyline {
            buildings: build_skyline(buildings),
            sky,
        }
    }

    /// From one box. An empty box gives a skyline without buildings, which
    /// is empty, has no points and merges as an empty one.
    pub fn from_box(b: Box, horizon: Axis, sky: Direction) -> Skyline {
        let buildings = if b.x.is_empty() || b.y.is_empty() {
            Vec::new()
        } else {
            single_skyline(Building::from_box(b, horizon, sky))
        };
        Skyline { buildings, sky }
    }

    /// The `sky` sides of the pairs merged pairwise from a queue, as
    /// `Skyline (std::vector<Skyline_pair>, Direction)` does; no buildings
    /// when there are no pairs.
    pub fn from_pairs(pairs: &[SkylinePair], sky: Direction) -> Skyline {
        let mut partials: VecDeque<Skyline> = pairs.iter().map(|pair| pair[sky].clone()).collect();
        while partials.len() > 1 {
            let mut one = partials.pop_front().expect("two partials");
            let two = partials.pop_front().expect("two partials");
            one.merge(&two);
            partials.push_back(one);
        }
        Skyline {
            buildings: partials.pop_front().map_or(Vec::new(), |s| s.buildings),
            sky,
        }
    }

    /// From segments in either direction; those without length along the
    /// horizon axis are left out.
    pub fn from_segments(segments: &[[Offset; 2]], horizon: Axis, sky: Direction) -> Skyline {
        let (h, v) = (horizon.index(), horizon.other().index());
        let buildings = segments
            .iter()
            .filter_map(|&[mut left, mut right]| {
                if left[h] > right[h] {
                    core::mem::swap(&mut left, &mut right);
                }
                let (x1, x2) = (left[h], right[h]);
                let (y1, y2) = (left[v] * sky.sign(), right[v] * sky.sign());
                (x1 < x2).then(|| Building::new(x1, y1, y2, x2))
            })
            .collect();
        Skyline {
            buildings: build_skyline(buildings),
            sky,
        }
    }

    /// `Skyline::direction`: the side it bounds.
    pub fn sky(&self) -> Direction {
        self.sky
    }

    /// `Skyline::print`: each building's line, as LilyPond's debugging
    /// prints them.
    pub fn print(&self) -> String {
        self.buildings.iter().map(Building::print).collect()
    }

    /// `Skyline::print_points`: [`to_points`](Skyline::to_points) along X,
    /// two to a line, as LilyPond's debugging prints them.
    pub fn print_points(&self) -> String {
        let mut out = String::new();
        for (i, [x, y]) in self.to_points(Axis::X).into_iter().enumerate() {
            let end = if i % 2 == 1 { "\n" } else { " " };
            out.push_str(&alloc::format!("({},{}){end}", printf_f(x), printf_f(y)));
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        match self.buildings.first() {
            None => true,
            Some(b) => b.right == f64::INFINITY && b.y_intercept == f64::NEG_INFINITY,
        }
    }

    /// `Skyline::merge`; both must bound the same side.
    pub fn merge(&mut self, other: &Skyline) {
        assert_eq!(self.sky, other.sky, "merging skylines of two sides");
        if other.is_empty() {
            return;
        }
        if self.is_empty() {
            self.buildings = other.buildings.clone();
            return;
        }
        if let Some(merged) = merge_buildings(&other.buildings, &self.buildings) {
            self.buildings = merged;
        }
    }

    /// `Skyline::raise`: moves the roofs by `r` along the other axis, up (or
    /// right) for either side.
    pub fn raise(&mut self, r: f64) {
        for b in &mut self.buildings {
            b.y_intercept += self.sky.sign() * r;
        }
    }

    /// `Skyline::shift`: moves the buildings `s` along the horizon.
    pub fn shift(&mut self, s: f64) {
        for b in &mut self.buildings {
            b.left += s;
            b.right += s;
            b.y_intercept = fma(-s, b.slope, b.y_intercept);
        }
    }

    /// `Skyline::height`: the height at `x`, which must be finite, of the
    /// first building not ending before it. The skyline must have buildings.
    pub fn height(&self, x: f64) -> f64 {
        assert!(!x.is_infinite(), "the height of a skyline at infinity");
        let i = self.buildings.partition_point(|b| b.right < x);
        self.sky.sign() * self.buildings[i].height(x)
    }

    /// `Skyline::max_height`: the highest of the buildings' ends; -infinity
    /// (infinity for `Down`) without buildings.
    pub fn max_height(&self) -> f64 {
        let mut ret = f64::NEG_INFINITY;
        for b in &self.buildings {
            ret = max(ret, b.height(b.left));
            ret = max(ret, b.height(b.right));
        }
        self.sky.sign() * ret
    }

    /// `Skyline::left`: where the first building with a roof starts,
    /// infinity without one.
    pub fn left(&self) -> f64 {
        self.buildings
            .iter()
            .find(|b| b.y_intercept > f64::NEG_INFINITY)
            .map_or(f64::INFINITY, |b| b.left)
    }

    /// `Skyline::right`: where the last building with a roof ends,
    /// -infinity without one.
    pub fn right(&self) -> f64 {
        self.buildings
            .iter()
            .rev()
            .find(|b| b.y_intercept > f64::NEG_INFINITY)
            .map_or(f64::NEG_INFINITY, |b| b.right)
    }

    /// `Skyline::max_height_position`: where the skyline touches a floor
    /// at 0 below it ([`touching_point`](Skyline::touching_point)), which
    /// is where it is highest, the last such end of a building.
    pub fn max_height_position(&self) -> f64 {
        let other = match self.sky {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
        };
        let mut s = Skyline::empty(other);
        // TODO: should be able to calc without doing a merge?
        s.set_minimum_height(0.0);
        self.touching_point(&s, 0.0)
    }

    /// `Skyline::set_minimum_height`: a roof at `h` everywhere merged in.
    pub fn set_minimum_height(&mut self, h: f64) {
        let mut s = Skyline::empty(self.sky);
        s.buildings[0].y_intercept = h * self.sky.sign();
        self.merge(&s);
    }

    /// `Skyline::clear`: the empty skyline of the same side.
    pub fn clear(&mut self) {
        self.buildings = empty_skyline();
    }

    /// `ly:skyline->points`: each building's start and end, heights the right
    /// way up, coordinates swapped for horizon axis Y.
    pub fn to_points(&self, horizon: Axis) -> Vec<Offset> {
        let mut points = Vec::with_capacity(2 * self.buildings.len());
        for b in &self.buildings {
            for x in [b.left, b.right] {
                let point = [x, self.sky.sign() * b.height(x)];
                points.push(match horizon {
                    Axis::X => point,
                    Axis::Y => [point[1], point[0]],
                });
            }
        }
        points
    }
}

/// LilyPond's `Skyline_pair`: the skylines below and above (left and right
/// of) the same things, indexed by [`Direction`] as `operator[]` is.
#[derive(Clone, Debug, PartialEq)]
pub struct SkylinePair {
    pub down: Skyline,
    pub up: Skyline,
}

impl Default for SkylinePair {
    /// `Skyline_pair ()`: both empty.
    fn default() -> SkylinePair {
        SkylinePair::empty()
    }
}

impl SkylinePair {
    pub fn empty() -> SkylinePair {
        SkylinePair {
            down: Skyline::empty(Direction::Down),
            up: Skyline::empty(Direction::Up),
        }
    }

    /// `Skyline_pair (Skyline, Skyline)`.
    pub fn new(down: Skyline, up: Skyline) -> SkylinePair {
        SkylinePair { down, up }
    }

    /// `Skyline_pair (std::vector<Box>, Axis)`.
    pub fn from_boxes(boxes: &[Box], horizon: Axis) -> SkylinePair {
        SkylinePair {
            down: Skyline::from_boxes(boxes, horizon, Direction::Down),
            up: Skyline::from_boxes(boxes, horizon, Direction::Up),
        }
    }

    /// `Skyline_pair (Box, Axis)`.
    pub fn from_box(b: Box, horizon: Axis) -> SkylinePair {
        SkylinePair {
            down: Skyline::from_box(b, horizon, Direction::Down),
            up: Skyline::from_box(b, horizon, Direction::Up),
        }
    }

    /// `Skyline_pair (std::vector<Drul_array<Offset>>, Axis)`.
    pub fn from_segments(segments: &[[Offset; 2]], horizon: Axis) -> SkylinePair {
        SkylinePair {
            down: Skyline::from_segments(segments, horizon, Direction::Down),
            up: Skyline::from_segments(segments, horizon, Direction::Up),
        }
    }

    /// `Skyline_pair (std::vector<Skyline_pair>)`.
    pub fn from_pairs(pairs: &[SkylinePair]) -> SkylinePair {
        SkylinePair {
            down: Skyline::from_pairs(pairs, Direction::Down),
            up: Skyline::from_pairs(pairs, Direction::Up),
        }
    }

    pub fn raise(&mut self, r: f64) {
        self.up.raise(r);
        self.down.raise(r);
    }

    pub fn shift(&mut self, s: f64) {
        self.up.shift(s);
        self.down.shift(s);
    }

    pub fn merge(&mut self, other: &SkylinePair) {
        self.up.merge(&other.up);
        self.down.merge(&other.down);
    }

    /// `Skyline_pair::pad`: each side [`padded`](Skyline::padded), the
    /// upper first; nothing for a padding of 0. LilyPond warns "Cannot have
    /// negative horizon padding.  Junking." for each side when it is below
    /// 0, which is the caller's to report.
    pub fn pad(&mut self, r: f64) {
        if r == 0.0 {
            return;
        }
        self.up = self.up.padded(r);
        self.down = self.down.padded(r);
    }

    /// The nearer of the two skylines' `left`.
    pub fn left(&self) -> f64 {
        min(self.up.left(), self.down.left())
    }

    /// The further of the two skylines' `right`.
    pub fn right(&self) -> f64 {
        max(self.up.right(), self.down.right())
    }

    /// `Skyline_pair::print`: the upper skyline's buildings, then the
    /// lower's.
    pub fn print(&self) -> String {
        self.up.print() + &self.down.print()
    }

    /// `Skyline_pair::print_points`: the upper skyline's points, then the
    /// lower's.
    pub fn print_points(&self) -> String {
        self.up.print_points() + &self.down.print_points()
    }

    pub fn is_empty(&self) -> bool {
        self.up.is_empty() && self.down.is_empty()
    }
}

impl core::ops::Index<Direction> for SkylinePair {
    type Output = Skyline;

    fn index(&self, d: Direction) -> &Skyline {
        match d {
            Direction::Down => &self.down,
            Direction::Up => &self.up,
        }
    }
}

impl core::ops::IndexMut<Direction> for SkylinePair {
    fn index_mut(&mut self, d: Direction) -> &mut Skyline {
        match d {
            Direction::Down => &mut self.down,
            Direction::Up => &mut self.up,
        }
    }
}

/// LilyPond's `Transform`, a `PangoMatrix`; glyphs need only its scaling.
#[derive(Clone, Copy, Debug)]
struct Transform {
    xx: f64,
    xy: f64,
    yx: f64,
    yy: f64,
    x0: f64,
    y0: f64,
}

impl Transform {
    const IDENTITY: Transform = Transform {
        xx: 1.0,
        xy: 0.0,
        yx: 0.0,
        yy: 1.0,
        x0: 0.0,
        y0: 0.0,
    };

    /// `pango_matrix_scale`.
    fn scaled(self, x: f64, y: f64) -> Transform {
        Transform {
            xx: self.xx * x,
            xy: self.xy * y,
            yx: self.yx * x,
            yy: self.yy * y,
            ..self
        }
    }

    /// `pango_matrix_transform_point`. Whether pango's products are fused
    /// does not show while `xy` and `yx` are zero.
    fn apply(&self, [x, y]: Offset) -> Offset {
        [
            (self.xx * x + self.xy * y) + self.x0,
            (self.yx * x + self.yy * y) + self.y0,
        ]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Orientation {
    Ccw,
    Cw,
}

/// `Lazy_skyline_pair`: segments collected per side, the skylines built once.
struct LazySkylinePair {
    axis: Axis,
    todo: Vec<[Offset; 2]>,
    /// Down, up.
    per_dir: [Vec<[Offset; 2]>; 2],
}

impl LazySkylinePair {
    fn new(axis: Axis) -> LazySkylinePair {
        LazySkylinePair {
            axis,
            todo: Vec::new(),
            per_dir: [Vec::new(), Vec::new()],
        }
    }

    fn is_empty(&self) -> bool {
        self.todo.is_empty() && self.per_dir.iter().all(Vec::is_empty)
    }

    /// A segment for both sides.
    fn add_segment(&mut self, tr: &Transform, p1: Offset, p2: Offset) {
        self.todo.push([tr.apply(p1), tr.apply(p2)]);
    }

    /// A segment of a contour, which bounds one side only.
    fn add_contour_segment(
        &mut self,
        tr: &Transform,
        orientation: Orientation,
        p1: Offset,
        p2: Offset,
    ) {
        let segment = [tr.apply(p1), tr.apply(p2)];
        let a = self.axis.index();
        let first = (segment[0][a] > segment[1][a]) == (orientation == Orientation::Ccw);
        // per_dir_todo_[(a_ == X_AXIS) ? UP : LEFT], LEFT being DOWN.
        let up = first == (self.axis == Axis::X);
        self.per_dir[usize::from(up)].push(segment);
    }

    fn add_box(&mut self, tr: &Transform, b: Box) {
        let points = [
            [b.x.left, b.y.left],
            [b.x.left, b.y.right],
            [b.x.right, b.y.right],
            [b.x.right, b.y.left],
        ];
        for i in 0..4 {
            self.add_contour_segment(tr, Orientation::Cw, points[i], points[(i + 1) % 4]);
        }
    }

    fn into_pair(mut self) -> SkylinePair {
        let mut pair = SkylinePair::empty();
        for (d, sky) in [(0, &mut pair.down), (1, &mut pair.up)] {
            if self.todo.is_empty() && self.per_dir[d].is_empty() {
                continue;
            }
            self.per_dir[d].extend_from_slice(&self.todo);
            let direction = sky.sky;
            sky.merge(&Skyline::from_segments(
                &self.per_dir[d],
                self.axis,
                direction,
            ));
        }
        pair
    }
}

/// `Bezier::curve_point`.
pub fn curve_point(control: &[Offset; 4], t: f64) -> Offset {
    const BINOMIAL: [f64; 4] = [1.0, 3.0, 3.0, 1.0];
    let mut one_min_tj = [1.0; 4];
    for i in 1..4 {
        one_min_tj[i] = one_min_tj[i - 1] * (1.0 - t);
    }
    let mut tj = 1.0;
    let mut o = [0.0, 0.0];
    for j in 0..4 {
        for a in 0..2 {
            o[a] += control[j][a] * BINOMIAL[j] * tj * one_min_tj[3 - j];
        }
        tj *= t;
    }
    o
}

/// A point of a FreeType outline: font units, and whether it is on the curve
/// or a cubic control point.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Point {
    at: [i32; 2],
    on: bool,
}

/// The glyph's outline as `FT_Load_Glyph` with `FT_LOAD_NO_SCALE` gives it
/// for a CFF font: the Type 2 charstring's path through FreeType's CFF engine
/// (`pshints.c`), whose zero-length lines are dropped and whose moves count
/// only once a line or curve follows, and each contour closed by
/// `ps_builder_close_contour`, which drops a last point on the first.
fn freetype_outline(font: &impl Outlines, index: usize) -> Vec<Vec<Point>> {
    fn close(contour: &mut Vec<Point>, contours: &mut Vec<Vec<Point>>) {
        if contour.len() > 1 && contour.first() == contour.last() {
            contour.pop();
        }
        if contour.len() > 1 {
            contours.push(core::mem::take(contour));
        }
        contour.clear();
    }

    let mut contours = Vec::new();
    let mut contour = Vec::new();
    let mut pen = [0, 0];
    for segment in font.outline(index) {
        let points = match segment {
            Segment::Move(to) => {
                close(&mut contour, &mut contours);
                pen = to;
                continue;
            }
            Segment::Line(to) if to == pen => continue,
            Segment::Line(to) => [Some((to, true)), None, None],
            Segment::Curve(a, b, c) => [Some((a, false)), Some((b, false)), Some((c, true))],
        };
        if contour.is_empty() {
            contour.push(Point { at: pen, on: true });
        }
        for (at, on) in points.into_iter().flatten() {
            contour.push(Point { at, on });
            pen = at;
        }
    }
    close(&mut contour, &mut contours);
    contours
}

/// What `FT_Outline_Decompose` calls for an outline.
enum Element {
    Move([i32; 2]),
    Line([i32; 2]),
    Cubic([i32; 2], [i32; 2], [i32; 2]),
}

/// `FT_Outline_Decompose`: each contour from its first point, a cubic's end
/// past the last point being the first, closed with a line to the first point
/// unless a cubic ended there.
fn decompose(contours: &[Vec<Point>], mut f: impl FnMut(Element)) {
    for contour in contours {
        let start = contour[0].at;
        f(Element::Move(start));
        let mut i = 1;
        let mut closed = false;
        while i < contour.len() {
            if contour[i].on {
                f(Element::Line(contour[i].at));
                i += 1;
            } else if i + 2 < contour.len() {
                f(Element::Cubic(
                    contour[i].at,
                    contour[i + 1].at,
                    contour[i + 2].at,
                ));
                i += 3;
            } else {
                f(Element::Cubic(contour[i].at, contour[i + 1].at, start));
                closed = true;
                break;
            }
        }
        if !closed {
            f(Element::Line(start));
        }
    }
}

/// `FT_BBox` in font units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BBox {
    min: [i64; 2],
    max: [i64; 2],
}

impl BBox {
    fn add(&mut self, p: [i32; 2]) {
        for (a, &v) in p.iter().enumerate() {
            self.min[a] = self.min[a].min(i64::from(v));
            self.max[a] = self.max[a].max(i64::from(v));
        }
    }

    fn x(&self) -> Interval {
        Interval::new(self.min[0] as f64, self.max[0] as f64)
    }
}

/// `FT_Outline_Get_CBox`: all points, on and off the curve; zero when empty.
fn control_box(contours: &[Vec<Point>]) -> BBox {
    let mut points = contours.iter().flatten();
    let Some(first) = points.next() else {
        return BBox {
            min: [0, 0],
            max: [0, 0],
        };
    };
    let mut cbox = BBox {
        min: first.at.map(i64::from),
        max: first.at.map(i64::from),
    };
    points.for_each(|p| cbox.add(p.at));
    cbox
}

/// ftbbox.c's `cubic_peak`: the peak of a cubic above 0, by bisection in
/// fixed point.
fn cubic_peak(mut q1: i64, mut q2: i64, mut q3: i64, mut q4: i64) -> i64 {
    let mut peak = 0;
    let bits = (q1.abs() | q2.abs() | q3.abs() | q4.abs()) as u32;
    let mut shift = 27 - (31 - bits.leading_zeros() as i32);
    if shift > 0 {
        shift = shift.min(2);
        q1 *= 1 << shift;
        q2 *= 1 << shift;
        q3 *= 1 << shift;
        q4 *= 1 << shift;
    } else {
        q1 >>= -shift;
        q2 >>= -shift;
        q3 >>= -shift;
        q4 >>= -shift;
    }
    while q2 > 0 || q3 > 0 {
        if q1 + q2 > q3 + q4 {
            // The first half.
            q4 += q3;
            q3 += q2;
            q2 += q1;
            q4 += q3;
            q3 += q2;
            q4 = (q4 + q3) >> 3;
            q3 >>= 2;
            q2 >>= 1;
        } else {
            // The second half.
            q1 += q2;
            q2 += q3;
            q3 += q4;
            q1 += q2;
            q2 += q3;
            q1 = (q1 + q2) >> 3;
            q2 >>= 2;
            q3 >>= 1;
        }
        if q1 == q2 && q1 >= q3 {
            peak = q1;
            break;
        }
        if q3 == q4 && q2 <= q4 {
            peak = q4;
            break;
        }
    }
    if shift > 0 {
        peak >> shift
    } else {
        peak << -shift
    }
}

/// `FT_Outline_Get_BBox`: the box of the points on the curve, grown by each
/// cubic whose control points lie outside it to the cubic's peak.
fn outline_bbox(contours: &[Vec<Point>]) -> BBox {
    if contours.is_empty() {
        return BBox {
            min: [0, 0],
            max: [0, 0],
        };
    }
    let mut bbox = BBox {
        min: [0x7FFF_FFFF; 2],
        max: [-0x7FFF_FFFF; 2],
    };
    contours
        .iter()
        .flatten()
        .filter(|p| p.on)
        .for_each(|p| bbox.add(p.at));
    if control_box(contours) == bbox {
        return bbox;
    }
    let mut last = [0, 0];
    decompose(contours, |element| match element {
        Element::Move(to) => {
            bbox.add(to);
            last = to;
        }
        Element::Line(to) => last = to,
        Element::Cubic(c1, c2, to) => {
            for a in 0..2 {
                let (p2, p3) = (i64::from(c1[a]), i64::from(c2[a]));
                let outside = |p: i64| p < bbox.min[a] || p > bbox.max[a];
                if !(outside(p2) || outside(p3)) {
                    continue;
                }
                let (p1, p4) = (i64::from(last[a]), i64::from(to[a]));
                let max = bbox.max[a];
                if p2 > max || p3 > max {
                    bbox.max[a] += cubic_peak(p1 - max, p2 - max, p3 - max, p4 - max);
                }
                let min = bbox.min[a];
                if p2 < min || p3 < min {
                    bbox.min[a] -= cubic_peak(min - p1, min - p2, min - p3, min - p4);
                }
            }
            last = to;
        }
    });
    bbox
}

/// `Interval::length`: 0 when empty.
fn length(i: Interval) -> f64 {
    if i.is_empty() { 0.0 } else { i.right - i.left }
}

fn offset(p: [i32; 2]) -> Offset {
    [f64::from(p[0]), f64::from(p[1])]
}

/// `ly:skylines-for-stencil` of glyph `index`'s stencil in `font` scaled by
/// `magnification` (a `Modified_font_metric`, as `ly:paper-get-font` gives
/// it): the outline scaled so that FreeType's control box is the width of
/// its exact bounding box (`add_named_glyph_segments`), contours
/// counterclockwise as PostScript fonts have them (`Path_interpreter`), and
/// the stencil's extent as a box for a glyph without an outline.
pub fn glyph_skylines(font: &Font, index: usize, magnification: f64, horizon: Axis) -> SkylinePair {
    let contours = freetype_outline(font, index);
    let k = magnification * font.design_size() / f64::from(font.units_per_em());
    let width = length(control_box(&contours).x().scaled(k));
    let scale = width / length(outline_bbox(&contours).x());
    let tr = Transform::IDENTITY.scaled(scale, scale);

    let mut lazy = LazySkylinePair::new(horizon);
    let orientation = Orientation::Ccw;
    let mut cur = [0.0, 0.0];
    decompose(&contours, |element| match element {
        Element::Move(to) => cur = offset(to),
        Element::Line(to) => {
            let to = offset(to);
            lazy.add_contour_segment(&tr, orientation, cur, to);
            cur = to;
        }
        Element::Cubic(c1, c2, to) => {
            let control = [cur, offset(c1), offset(c2), offset(to)];
            let (start, end) = (tr.apply(control[0]), tr.apply(control[3]));
            let length = libm::hypot(end[0] - start[0], end[1] - start[1]);
            let quantization = ((length / 0.2) as i32).max(2);
            for i in 1..quantization {
                let point = curve_point(&control, f64::from(i) / f64::from(quantization));
                lazy.add_contour_segment(&tr, orientation, cur, point);
                cur = point;
            }
            lazy.add_segment(&tr, cur, control[3]);
            cur = control[3];
        }
    });

    let extent = font.char_dimensions(index).scaled(magnification);
    if lazy.is_empty() && !extent.x.is_empty() && !extent.y.is_empty() {
        lazy.add_box(&Transform::IDENTITY, extent);
    }
    lazy.into_pair()
}

/// What `FT_Outline_Decompose` calls for a glyph's outline, in font units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OutlineElement {
    Move(Offset),
    Line(Offset),
    Cubic(Offset, Offset, Offset),
}

/// Glyph `index`'s outline as FreeType loads it unscaled and
/// `FT_Outline_Decompose` walks it, [`glyph_skylines`]' walk, for a caller
/// that cuts it into segments under a transform of its own (layout's
/// `add_named_glyph_segments` and `add_glyph_string_segments`).
pub fn glyph_outline(font: &impl Outlines, index: usize, mut f: impl FnMut(OutlineElement)) {
    decompose(&freetype_outline(font, index), |element| {
        f(match element {
            Element::Move(to) => OutlineElement::Move(offset(to)),
            Element::Line(to) => OutlineElement::Line(offset(to)),
            Element::Cubic(c1, c2, to) => OutlineElement::Cubic(offset(c1), offset(c2), offset(to)),
        });
    });
}

impl BBox {
    fn to_box(self) -> Box {
        Box {
            x: self.x(),
            y: Interval::new(self.min[1] as f64, self.max[1] as f64),
        }
    }
}

/// `ly_FT_get_unscaled_indexed_char_dimensions`: FreeType's metrics of
/// glyph `index` loaded unscaled, the control box of its outline, all zero
/// for an empty outline.
pub fn glyph_unscaled_box(font: &impl Outlines, index: usize) -> Box {
    control_box(&freetype_outline(font, index)).to_box()
}

/// `ly_FT_get_glyph_outline_bbox`: `FT_Outline_Get_BBox` of glyph `index`
/// loaded unscaled, all zero for an empty outline.
pub fn glyph_outline_bbox(font: &impl Outlines, index: usize) -> Box {
    outline_bbox(&freetype_outline(font, index)).to_box()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_makes_three_buildings() {
        let up = Skyline::from_segments(&[[[2.0, 1.0], [0.0, 3.0]]], Axis::X, Direction::Up);
        let inf = f64::INFINITY;
        assert_eq!(
            up.to_points(Axis::X),
            [
                [-inf, -inf],
                [0.0, -inf],
                [0.0, 3.0],
                [2.0, 1.0],
                [2.0, -inf],
                [inf, -inf]
            ]
        );
        let down = Skyline::from_segments(&[[[0.0, 3.0], [2.0, 1.0]]], Axis::Y, Direction::Down);
        assert_eq!(down.to_points(Axis::Y)[2], [2.0, 1.0]);
        assert!(!up.is_empty());
        assert!(Skyline::empty(Direction::Up).is_empty());
        assert!(
            Skyline::from_segments(&[[[1.0, 0.0], [1.0, 5.0]]], Axis::X, Direction::Up).is_empty()
        );
    }

    #[test]
    fn overlapping_segments_keep_the_higher() {
        let up = Skyline::from_segments(
            &[
                [[0.0, 1.0], [4.0, 1.0]],
                [[1.0, 2.0], [2.0, 2.0]],
                [[3.0, 0.0], [5.0, 0.0]],
            ],
            Axis::X,
            Direction::Up,
        );
        let finite: Vec<Offset> = up
            .to_points(Axis::X)
            .into_iter()
            .filter(|p| p[1].is_finite())
            .collect();
        assert_eq!(
            finite,
            [
                [0.0, 1.0],
                [1.0, 1.0],
                [1.0, 2.0],
                [2.0, 2.0],
                [2.0, 1.0],
                [4.0, 1.0],
                [4.0, 0.0],
                [5.0, 0.0]
            ]
        );
    }

    #[test]
    fn a_box_bounds_both_sides() {
        let mut lazy = LazySkylinePair::new(Axis::X);
        let b = Box {
            x: Interval::new(0.0, 2.0),
            y: Interval::new(-1.0, 3.0),
        };
        lazy.add_box(&Transform::IDENTITY, b);
        let pair = lazy.into_pair();
        assert_eq!(pair.up.to_points(Axis::X)[2], [0.0, 3.0]);
        assert_eq!(pair.down.to_points(Axis::X)[2], [0.0, -1.0]);
    }

    /// FreeType's bisection finds a cubic's peak at 3/4 of its control
    /// points' height, and a bounding box that a control box is not.
    #[test]
    fn the_bounding_box_takes_the_cubic_s_peak() {
        assert_eq!(cubic_peak(0, 100, 100, 0), 75);
        let contour = Vec::from([
            Point {
                at: [0, 0],
                on: true,
            },
            Point {
                at: [0, 100],
                on: false,
            },
            Point {
                at: [100, 100],
                on: false,
            },
            Point {
                at: [100, 0],
                on: true,
            },
        ]);
        let contours = [contour];
        assert_eq!(control_box(&contours).max, [100, 100]);
        assert_eq!(
            outline_bbox(&contours),
            BBox {
                min: [0, 0],
                max: [100, 75]
            }
        );
    }
}
