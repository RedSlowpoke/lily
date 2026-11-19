//! LilyPond's geometry: flower's `Interval` and `Offset` (`interval.hh`,
//! `offset.cc`), `box.cc`'s `Box` and `transform.cc`'s `Transform`, Pango's
//! affine matrix with LilyPond's rotation, with `transform-scheme.cc`'s
//! Scheme face; and `dimensions.hh`'s units, with `dimensions-scheme.cc`'s
//! five (MS3 step 14).
//!
//! The references come from Homebrew's LilyPond 2.26.0 and Pango for macOS
//! on arm64, whose clang fuses `a * b + c` written as one expression into
//! one rounding (`-ffp-contract=on`, as the skyline crate found): [`fma`]
//! marks where.

use alloc::string::String;

use lily_engine_scheme::{Cx, Foreign, Heap, ImageReader, ImageWriter, Thrown, Tracer, Value};

use crate::R;

/// `a * b + c` rounded once, as clang compiles it.
pub(crate) use lily_engine_skyline::fma;

/// `std::min`: `b` only when `b < a`.
pub(crate) fn min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// `std::max`: `b` only when `a < b`.
pub(crate) fn max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// `Axis`: `X_AXIS` 0, `Y_AXIS` 1.
pub type Axis = usize;
pub const X: Axis = 0;
pub const Y: Axis = 1;

/// `other_axis`.
pub fn other_axis(a: Axis) -> Axis {
    1 - a
}

/// `Direction`: `LEFT` and `DOWN` -1, `RIGHT` and `UP` 1.
pub type Direction = i32;

// --- Interval --------------------------------------------------------------------

/// `Interval`: the closed interval from `left` to `right`, empty when
/// `left > right`; [`Interval::EMPTY`] is LilyPond's empty one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Interval {
    pub left: f64,
    pub right: f64,
}

impl Interval {
    pub const EMPTY: Interval = Interval {
        left: f64::INFINITY,
        right: f64::NEG_INFINITY,
    };

    pub const fn new(left: f64, right: f64) -> Interval {
        Interval { left, right }
    }

    /// `at (d)`: the left end for -1, the right end otherwise.
    pub fn at(&self, d: Direction) -> f64 {
        if d < 0 { self.left } else { self.right }
    }

    pub fn at_mut(&mut self, d: Direction) -> &mut f64 {
        if d < 0 {
            &mut self.left
        } else {
            &mut self.right
        }
    }

    pub fn is_empty(&self) -> bool {
        self.left > self.right
    }

    pub fn set_empty(&mut self) {
        *self = Interval::EMPTY;
    }

    pub fn length(&self) -> f64 {
        if self.is_empty() {
            0.0
        } else {
            self.right - self.left
        }
    }

    pub fn center(&self) -> f64 {
        (self.left + self.right) / 2.0
    }

    /// From -1, the left end, to 1, the right end, and beyond.
    pub fn linear_combination(&self, x: f64) -> f64 {
        fma(1.0 - x, self.left, (x + 1.0) * self.right) * 0.5
    }

    pub fn unite(&mut self, h: Interval) {
        self.left = min(h.left, self.left);
        self.right = max(h.right, self.right);
    }

    pub fn intersect(&mut self, h: Interval) {
        self.left = max(h.left, self.left);
        self.right = min(h.right, self.right);
    }

    pub fn add_point(&mut self, p: f64) {
        self.left = min(self.left, p);
        self.right = max(self.right, p);
    }

    pub fn translate(&mut self, t: f64) {
        self.left += t;
        self.right += t;
    }

    pub fn widen(&mut self, t: f64) {
        self.left -= t;
        self.right += t;
    }

    /// `*=`: an empty interval stays as it is; a negative factor swaps the
    /// ends.
    pub fn scale(&mut self, r: f64) {
        if !self.is_empty() {
            self.left *= r;
            self.right *= r;
            if r < 0.0 {
                core::mem::swap(&mut self.left, &mut self.right);
            }
        }
    }
}

// --- Offset ----------------------------------------------------------------------

/// `Offset`: a point or a vector, `(x, y)`.
pub type Offset = (f64, f64);

/// `complex_multiply`: zero when `z2`'s y is infinite.
pub fn complex_multiply(z1: Offset, z2: Offset) -> Offset {
    if z2.1.is_infinite() {
        return (0.0, 0.0);
    }
    (
        fma(z1.0, z2.0, -(z1.1 * z2.1)),
        fma(z1.0, z2.1, z1.1 * z2.0),
    )
}

/// `offset_directed`: the unit vector at `angle` degrees, exact at the
/// multiples of 90 and equal in x and y at the odd multiples of 45.
pub fn offset_directed(angle: f64) -> Offset {
    use core::f64::consts::PI;
    let mut angle = angle;
    if angle <= -360.0 || angle >= 360.0 {
        angle = libm::fmod(angle, 360.0);
    }
    if angle <= -180.0 {
        angle += 360.0;
    } else if angle > 180.0 {
        angle -= 360.0;
    }
    let sin = |degrees: f64| libm::sin(degrees * PI / 180.0);
    if angle > 0.0 {
        if angle > 90.0 {
            (sin(90.0 - angle), sin(180.0 - angle))
        } else {
            (sin(90.0 - angle), sin(angle))
        }
    } else if angle < -90.0 {
        (sin(90.0 + angle), sin(-180.0 - angle))
    } else {
        (sin(90.0 + angle), sin(angle))
    }
}

/// `atan2d`.
fn atan2d(y: f64, x: f64) -> f64 {
    libm::atan2(y, x) * (180.0 / core::f64::consts::PI)
}

/// `Offset::angle_degrees`, which keeps near the multiples of 45 degrees.
pub fn angle_degrees((x, y): Offset) -> f64 {
    if y < 0.0 {
        if 2.0 * x < -y {
            if -x > -2.0 * y {
                return -180.0 + atan2d(-y, -x);
            } else if -2.0 * x >= -y {
                return -135.0 + atan2d(x - y, -y - x);
            } else {
                return -90.0 + atan2d(x, -y);
            }
        } else if x <= -2.0 * y {
            return -45.0 + atan2d(x + y, x - y);
        }
    } else if y > 0.0 {
        if 2.0 * x < y {
            if -x > 2.0 * y {
                return 180.0 - atan2d(y, -x);
            } else if -2.0 * x >= y {
                return 135.0 - atan2d(x + y, y - x);
            } else {
                return 90.0 - atan2d(x, y);
            }
        } else if x <= 2.0 * y {
            return 45.0 - atan2d(x - y, x + y);
        }
    } else {
        return if x < 0.0 { 180.0 } else { 0.0 };
    }
    atan2d(y, x)
}

/// `Offset::length`.
pub fn length((x, y): Offset) -> f64 {
    hypot(x, y)
}

/// 2 to the power `e`, for |e| under 1023.
fn pow2(e: i32) -> f64 {
    f64::from_bits(((0x3ff + e) as u64) << 52)
}

/// `hypot` as macOS's libm computes it, which the `libm` crate's misses by
/// an ulp now and then (MS3 step 3): the root of the smaller squared and
/// added to the larger's square in one rounding, scaled by 2⁶⁰⁰ out of
/// overflow and underflow. It gave macOS's for all of 20 million pairs,
/// random bit patterns among them.
pub(crate) fn hypot(x: f64, y: f64) -> f64 {
    let (mut a, mut b) = (libm::fabs(x), libm::fabs(y));
    if a.is_infinite() || b.is_infinite() {
        return f64::INFINITY;
    }
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a < b {
        core::mem::swap(&mut a, &mut b);
    }
    if b == 0.0 {
        return a;
    }
    let mut scale = 1.0;
    if a > pow2(500) {
        (a, b, scale) = (a * pow2(-600), b * pow2(-600), pow2(600));
    } else if a < pow2(-500) {
        (a, b, scale) = (a * pow2(600), b * pow2(600), pow2(-600));
    }
    libm::sqrt(fma(b, b, a * a)) * scale
}

/// `Offset::direction`: the unit vector, as `/=` gives it, a multiplication
/// by the reciprocal of the length.
pub fn direction(d: Offset) -> Offset {
    if d.0.is_infinite() {
        if !d.1.is_infinite() {
            return (if d.0 > 0.0 { 1.0 } else { -1.0 }, 0.0);
        }
    } else if d.1.is_infinite() {
        return (0.0, if d.1 > 0.0 { 1.0 } else { -1.0 });
    } else if d.0 == 0.0 && d.1 == 0.0 {
        return d;
    }
    let r = 1.0 / length(d);
    (d.0 * r, d.1 * r)
}

// --- Box -------------------------------------------------------------------------

/// `Box`: an interval on each axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Box(pub [Interval; 2]);

impl Box {
    pub const EMPTY: Box = Box([Interval::EMPTY; 2]);

    pub fn new(x: Interval, y: Interval) -> Box {
        Box([x, y])
    }

    pub fn x(&self) -> Interval {
        self.0[X]
    }

    pub fn y(&self) -> Interval {
        self.0[Y]
    }

    /// Empty on both axes.
    pub fn is_empty(&self) -> bool {
        self.is_empty_on(X) && self.is_empty_on(Y)
    }

    /// Whether axis `a` is exactly LilyPond's empty interval.
    pub fn is_empty_on(&self, a: Axis) -> bool {
        self.0[a] == Interval::EMPTY
    }

    pub fn set_empty(&mut self) {
        *self = Box::EMPTY;
    }

    /// An axis that is empty stays so.
    pub fn translate(&mut self, o: Offset) {
        for (a, d) in [(X, o.0), (Y, o.1)] {
            if !self.is_empty_on(a) {
                self.0[a].translate(d);
            }
        }
    }

    pub fn unite(&mut self, b: Box) {
        self.0[X].unite(b.0[X]);
        self.0[Y].unite(b.0[Y]);
    }

    pub fn intersect(&mut self, b: Box) {
        self.0[X].intersect(b.0[X]);
        self.0[Y].intersect(b.0[Y]);
    }

    pub fn add_point(&mut self, o: Offset) {
        self.0[X].add_point(o.0);
        self.0[Y].add_point(o.1);
    }

    pub fn scale(&mut self, s: f64) {
        self.0[X].scale(s);
        self.0[Y].scale(s);
    }

    pub fn widen(&mut self, x: f64, y: f64) {
        self.0[X].widen(x);
        self.0[Y].widen(y);
    }

    pub fn center(&self) -> Offset {
        (self.0[X].center(), self.0[Y].center())
    }

    pub fn area(&self) -> f64 {
        self.0[X].length() * self.0[Y].length()
    }
}

// --- Transform -------------------------------------------------------------------

/// `Transform`: Pango's affine matrix, `x' = xx x + xy y + x0` and `y' =
/// yx x + yy y + y0`, with y upwards. A Scheme value that is a procedure:
/// called on a transform it concatenates, on a point it maps it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub xx: f64,
    pub xy: f64,
    pub yx: f64,
    pub yy: f64,
    pub x0: f64,
    pub y0: f64,
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        xx: 1.0,
        xy: 0.0,
        yx: 0.0,
        yy: 1.0,
        x0: 0.0,
        y0: 0.0,
    };

    /// In Pango's order: `xx`, `xy`, `yx`, `yy`, `x0`, `y0`.
    pub const fn new(xx: f64, xy: f64, yx: f64, yy: f64, x0: f64, y0: f64) -> Transform {
        Transform {
            xx,
            xy,
            yx,
            yy,
            x0,
            y0,
        }
    }

    pub const fn translation(o: Offset) -> Transform {
        Transform::new(1.0, 0.0, 0.0, 1.0, o.0, o.1)
    }

    /// A rotation by `angle` degrees about `center`, without
    /// `pango_matrix_rotate`'s roundings at the multiples of 45.
    pub fn rotation(angle: f64, center: Offset) -> Transform {
        let d = offset_directed(angle);
        let mut t = Transform::new(d.0, 0.0 - d.1, d.1, d.0, center.0, center.1);
        let d = t.apply((-center.0, -center.1));
        t.x0 = d.0;
        t.y0 = d.1;
        t
    }

    /// `pango_matrix_transform_point`.
    pub fn apply(&self, (x, y): Offset) -> Offset {
        let nx = fma(self.xx, x, self.xy * y);
        let ny = fma(self.yx, x, self.yy * y);
        (nx + self.x0, ny + self.y0)
    }

    /// `pango_matrix_concat`: `t` first, then this.
    pub fn concat(&mut self, t: &Transform) -> &mut Transform {
        let m = *self;
        self.xx = fma(m.xx, t.xx, m.xy * t.yx);
        self.xy = fma(m.xx, t.xy, m.xy * t.yy);
        self.yx = fma(m.yx, t.xx, m.yy * t.yx);
        self.yy = fma(m.yx, t.xy, m.yy * t.yy);
        self.x0 = fma(m.xx, t.x0, m.xy * t.y0) + m.x0;
        self.y0 = fma(m.yx, t.x0, m.yy * t.y0) + m.y0;
        self
    }

    /// `pango_matrix_translate`: by `p` first, then this.
    pub fn translate(&mut self, p: Offset) -> &mut Transform {
        self.x0 += fma(self.xx, p.0, self.xy * p.1);
        self.y0 += fma(self.yx, p.0, self.yy * p.1);
        self
    }

    /// A rotation first, then this.
    pub fn rotate(&mut self, angle: f64, center: Offset) -> &mut Transform {
        let r = Transform::rotation(angle, center);
        self.concat(&r)
    }

    /// `pango_matrix_scale`: a scaling first, then this.
    pub fn scale(&mut self, x: f64, y: f64) -> &mut Transform {
        self.xx *= x;
        self.xy *= y;
        self.yx *= x;
        self.yy *= y;
        self
    }

    /// `operator () (Transform)`: `t` first, then this.
    pub fn then(&self, t: &Transform) -> Transform {
        let mut result = *self;
        result.concat(t);
        result
    }

    /// `to_string`: `[[xx xy x0] [yx yy y0]]`, each as C's `%f`.
    pub fn text(&self) -> String {
        let f = |x: f64| -> String {
            if x.is_nan() {
                String::from(if x.is_sign_negative() { "-nan" } else { "nan" })
            } else {
                alloc::format!("{x:.6}")
            }
        };
        alloc::format!(
            "[[{} {} {}] [{} {} {}]]",
            f(self.xx),
            f(self.xy),
            f(self.x0),
            f(self.yx),
            f(self.yy),
            f(self.y0)
        )
    }
}

/// A [`Transform`] as a Scheme value, with the procedure that a call of it
/// calls (`%transform-call`).
#[derive(Clone, Copy)]
pub struct TransformObject {
    pub transform: Transform,
    call: Value,
}

impl Foreign for TransformObject {
    fn type_name(&self) -> &'static str {
        "Transform"
    }

    fn print(&self, _: &Heap, out: &mut String) {
        out.push_str("#<Transform ");
        out.push_str(&self.transform.text());
        out.push('>');
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.call);
    }

    fn applicable(&self) -> Option<Value> {
        (self.call != Value::FALSE).then_some(self.call)
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        let t = &self.transform;
        for x in [t.xx, t.xy, t.yx, t.yy, t.x0, t.y0] {
            out.f64(x);
        }
        out.value(self.call);
        true
    }
}

impl TransformObject {
    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<TransformObject> {
        let [xx, xy, yx, yy, x0, y0] = [(); 6].map(|_| r.f64());
        Some(TransformObject {
            transform: Transform::new(xx?, xy?, yx?, yy?, x0?, y0?),
            call: r.value()?,
        })
    }
}

/// `smobbed_copy` of a transform.
fn transform(cx: &mut Cx, transform: Transform) -> Value {
    let call = cx.global("lily", "%transform-call").unwrap_or(Value::FALSE);
    cx.heap_mut().wrap(TransformObject { transform, call })
}

/// A real argument, `scm_is_real`.
fn real(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<f64, Thrown> {
    match cx.heap().is_real(v) {
        true => Ok(cx.heap().to_f64(v).unwrap_or(f64::NAN)),
        false => Err(cx.wrong_type(subr, position, v, "real number")),
    }
}

/// `scm_c_real_part` and `scm_c_imag_part` of a number.
fn parts(cx: &Cx, v: Value) -> Offset {
    match cx.heap().as_complex(v) {
        Some(z) => z,
        None => (cx.heap().to_f64(v).unwrap_or(f64::NAN), 0.0),
    }
}

/// `scm_c_make_rectangular`: a real number when the imaginary part is zero.
fn rectangular(cx: &mut Cx, (re, im): Offset) -> Value {
    if im == 0.0 {
        cx.heap_mut().flonum(re)
    } else {
        cx.heap_mut().complex(re, im)
    }
}

/// `(%transform-call transform arg)`: `Transform::call`, what calling a
/// transform does.
pub(crate) fn transform_call(cx: &mut Cx, args: &[Value]) -> R {
    let Some(&TransformObject { transform: t, .. }) = cx.heap().foreign(args[0]) else {
        return Err(cx.wrong_type("ly:call", 0, args[0], "Transform"));
    };
    let arg = args[1];
    if let Some(&TransformObject {
        transform: other, ..
    }) = cx.heap().foreign(arg)
    {
        return Ok(transform(cx, t.then(&other)));
    }
    if cx.heap().is_number(arg) {
        let p = t.apply(parts(cx, arg));
        return Ok(rectangular(cx, p));
    }
    let p = crate::offset_arg(cx, "ly:call", 1, arg)?;
    let p = t.apply(p);
    Ok(crate::offset(cx.heap_mut(), p))
}

/// `(ly:transform? x)`.
pub(crate) fn is_transform(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        cx.heap().foreign::<TransformObject>(args[0]).is_some(),
    ))
}

/// `(ly:make-transform [xx yx xy yy [x0 y0]])`: the identity, a linear
/// transform or an affine one.
pub(crate) fn make_transform(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:make-transform";
    if args.is_empty() {
        return Ok(transform(cx, Transform::IDENTITY));
    }
    let mut m = [0.0; 6];
    let n = if args.len() > 4 { 6 } else { 4 };
    for (i, x) in m.iter_mut().enumerate().take(n) {
        let v = args.get(i).copied().unwrap_or(Value::UNDEFINED);
        *x = real(cx, SUBR, i + 1, v)?;
    }
    let [xx, yx, xy, yy, x0, y0] = m;
    Ok(transform(cx, Transform::new(xx, xy, yx, yy, x0, y0)))
}

/// `(ly:make-scaling scale [scaley])`: by a pair's coordinates, by a
/// complex number as a scaled rotation, or by two reals.
pub(crate) fn make_scaling(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:make-scaling";
    let scale = args[0];
    let Some(&scaley) = args.get(1) else {
        if cx.heap().car(scale).is_some() {
            let (x, y) = crate::offset_arg(cx, SUBR, 1, scale)?;
            return Ok(transform(cx, Transform::new(x, 0.0, 0.0, y, 0.0, 0.0)));
        }
        if !cx.heap().is_number(scale) {
            return Err(cx.wrong_type(SUBR, 1, scale, "number"));
        }
        let (re, im) = parts(cx, scale);
        return Ok(transform(
            cx,
            Transform::new(re, im, 0.0 - im, re, 0.0, 0.0),
        ));
    };
    let x = real(cx, SUBR, 1, scale)?;
    let y = real(cx, SUBR, 2, scaley)?;
    Ok(transform(cx, Transform::new(x, 0.0, 0.0, y, 0.0, 0.0)))
}

/// `(ly:make-rotation angle [center])`: about `center`, or the origin.
pub(crate) fn make_rotation(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:make-rotation";
    let angle = real(cx, SUBR, 1, args[0])?;
    let center = match args.get(1) {
        Some(&c) => crate::offset_arg(cx, SUBR, 2, c)?,
        None => (0.0, 0.0),
    };
    Ok(transform(cx, Transform::rotation(angle, center)))
}

/// `(ly:make-translation x [y])`: by two reals, a complex number or a pair.
pub(crate) fn make_translation(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:make-translation";
    if let Some(&y) = args.get(1) {
        let x = real(cx, SUBR, 1, args[0])?;
        let y = real(cx, SUBR, 2, y)?;
        return Ok(transform(cx, Transform::translation((x, y))));
    }
    let o = if cx.heap().is_number(args[0]) {
        parts(cx, args[0])
    } else {
        crate::offset_arg(cx, SUBR, 1, args[0])?
    };
    Ok(transform(cx, Transform::translation(o)))
}

/// `(ly:transform->list transform)`: `(xx yx xy yy x0 y0)`.
pub(crate) fn transform_to_list(cx: &mut Cx, args: &[Value]) -> R {
    let Some(&TransformObject { transform: t, .. }) = cx.heap().foreign(args[0]) else {
        return Err(cx.wrong_type("ly:transform->list", 1, args[0], "Transform"));
    };
    let items = [t.xx, t.yx, t.xy, t.yy, t.x0, t.y0].map(|x| cx.heap_mut().flonum(x));
    Ok(cx.heap_mut().list(&items))
}

// --- Units (dimensions.hh, dimensions-scheme.cc) ---------------------------------

/// `dimensions.hh`'s points to the inch, centimetre and millimetre, and its
/// internal unit, the millimetre, in points.
const INCH_TO_PT: f64 = 72.270;
const CM_TO_PT: f64 = INCH_TO_PT / 2.54;
const MM_TO_PT: f64 = CM_TO_PT / 10.0;
const PT_TO_PT: f64 = 1.0;
const INCH_TO_BP: f64 = 72.0;
const BIGPOINT_TO_POINT: f64 = INCH_TO_PT / INCH_TO_BP;
const PT_TO_MM: f64 = 1.0 / MM_TO_PT;

/// One of each unit in millimetres (`point_constant` and the rest), as
/// `1 PT`, `1 MM` and the others compute it.
pub const POINT: f64 = PT_TO_PT * PT_TO_MM;
pub const MM: f64 = MM_TO_PT * PT_TO_MM;
pub const CM: f64 = CM_TO_PT * PT_TO_MM;
pub const INCH: f64 = INCH_TO_PT * PT_TO_MM;
pub const BIGPOINT: f64 = BIGPOINT_TO_POINT * PT_TO_MM;

/// `(subr num)` of a unit: `num` of it, in millimetres. They replace the
/// scheme crate's, which do not convert a complex `num` as
/// `from_scm<double>` does.
fn units(cx: &mut Cx, subr: &str, constant: f64, num: Value) -> R {
    let x = crate::number_arg(cx, subr, 1, num)?;
    Ok(cx.heap_mut().flonum(constant * x))
}

/// `(ly:pt num)`.
pub(crate) fn ly_pt(cx: &mut Cx, args: &[Value]) -> R {
    units(cx, "ly:pt", POINT, args[0])
}

/// `(ly:cm num)`.
pub(crate) fn ly_cm(cx: &mut Cx, args: &[Value]) -> R {
    units(cx, "ly:cm", CM, args[0])
}

/// `(ly:inch num)`.
pub(crate) fn ly_inch(cx: &mut Cx, args: &[Value]) -> R {
    units(cx, "ly:inch", INCH, args[0])
}

/// `(ly:mm num)`.
pub(crate) fn ly_mm(cx: &mut Cx, args: &[Value]) -> R {
    units(cx, "ly:mm", MM, args[0])
}

/// `(ly:bp num)`.
pub(crate) fn ly_bp(cx: &mut Cx, args: &[Value]) -> R {
    units(cx, "ly:bp", BIGPOINT, args[0])
}
