//! LilyPond's `Stencil` (`stencil.cc`): an expression of the output
//! backends and its extents, with `stencil-scheme.cc`'s Scheme face but
//! `ly:round-filled-box`, `ly:round-polygon` and `ly:bracket`, which draw
//! `lookup.cc`'s shapes ([`crate::lookup`]). The stencil heads that
//! `ly:register-stencil-expression` registers (`stencil-expression.cc`) and
//! `ly:make-stencil`, which checks them, are Scheme ([`crate::PRELUDE`]).

use alloc::boxed::Box as Owned;
use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{
    Cx, Exception, Foreign, Heap, ImageReader, ImageWriter, Thrown, Tracer, Value,
};

use crate::geometry::{
    Axis, Box, Direction, Interval, Offset, X, Y, angle_degrees, complex_multiply, direction,
    length, offset_directed, other_axis,
};
use crate::{R, axis_arg, direction_arg, number_arg, number_pair_arg, offset};

/// A stencil: its expression and its extents.
#[derive(Clone, Copy, Debug)]
pub struct Stencil {
    pub expr: Value,
    pub dim: Box,
}

impl Default for Stencil {
    /// `Stencil ()`: no expression, empty extents.
    fn default() -> Stencil {
        Stencil {
            expr: Value::NULL,
            dim: Box::EMPTY,
        }
    }
}

impl Foreign for Stencil {
    fn type_name(&self) -> &'static str {
        "Stencil"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.expr);
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.value(self.expr);
        for x in [self.dim.x(), self.dim.y()]
            .iter()
            .flat_map(|i| [i.left, i.right])
        {
            out.f64(x);
        }
        true
    }
}

/// Reads what [`Stencil::save`] wrote, under the name `Stencil`, which the
/// images have had since MS2 (`define-markup-commands.scm`'s constant
/// stencils).
pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<Owned<dyn Foreign>> {
    let expr = r.value()?;
    let [x0, x1, y0, y1] = [(); 4].map(|_| r.f64());
    Some(Owned::new(Stencil {
        expr,
        dim: Box::new(Interval::new(x0?, x1?), Interval::new(y0?, y1?)),
    }))
}

/// The items of a proper list, or of a list's proper part.
fn items(heap: &Heap, mut list: Value) -> Vec<Value> {
    let mut out = Vec::new();
    while let (Some(item), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        out.push(item);
        list = rest;
    }
    out
}

/// `ly_append (a, b)`: a copy of the list `a` ending in `b`.
fn append(heap: &mut Heap, a: Value, b: Value) -> Value {
    items(heap, a)
        .into_iter()
        .rev()
        .fold(b, |tail, item| heap.cons(item, tail))
}

impl Stencil {
    pub fn new(dim: Box, expr: Value) -> Stencil {
        Stencil { expr, dim }
    }

    pub fn extent(&self, a: Axis) -> Interval {
        self.dim.0[a]
    }

    /// No expression, or empty on both axes.
    pub fn is_empty(&self) -> bool {
        self.expr == Value::NULL || self.dim.is_empty()
    }

    pub fn is_empty_on(&self, a: Axis) -> bool {
        self.dim.is_empty_on(a)
    }

    /// `set_empty`: empty extents, or `(0, 0)` on both axes.
    pub fn set_empty(&mut self, empty: bool) {
        self.dim = if empty {
            Box::EMPTY
        } else {
            Box::new(Interval::new(0.0, 0.0), Interval::new(0.0, 0.0))
        };
    }

    /// Rotated by `a` degrees about the point `off`.
    pub fn rotate_degrees_absolute(&mut self, heap: &mut Heap, a: f64, off: Offset) {
        let head = heap.intern("rotate-stencil");
        let angle = heap.flonum(a);
        let at = offset(heap, off);
        let spec = heap.list(&[angle, at]);
        self.expr = heap.list(&[head, spec, self.expr]);
        let mut shifted = self.dim;
        shifted.translate((-off.0, -off.1));
        let (x, y) = (shifted.x(), shifted.y());
        let rot = offset_directed(a);
        self.dim.set_empty();
        for p in [
            (x.left, y.left),
            (x.right, y.left),
            (x.right, y.right),
            (x.left, y.right),
        ] {
            let q = complex_multiply(p, rot);
            self.dim.add_point((q.0 + off.0, q.1 + off.1));
        }
    }

    /// Rotated by `a` degrees about the point `off` gives relative to the
    /// extents: -1 the left or lower edge, 1 the right or upper.
    pub fn rotate_degrees(&mut self, heap: &mut Heap, a: f64, off: Offset) {
        let x = self.extent(X).linear_combination(off.0);
        let y = self.extent(Y).linear_combination(off.1);
        self.rotate_degrees_absolute(heap, a, (x, y));
    }

    /// Translated by `o`; an improbable offset (infinite, not a number, or
    /// over 10⁶) is taken as 0, without LilyPond's programming error.
    pub fn translate(&mut self, heap: &mut Heap, o: Offset) {
        let sane = |d: f64| {
            if d.is_infinite() || d.is_nan() || libm::fabs(d) > 1e6 {
                0.0
            } else {
                d
            }
        };
        let o = (sane(o.0), sane(o.1));
        if self.expr != Value::NULL {
            let head = heap.intern("translate-stencil");
            let at = offset(heap, o);
            self.expr = heap.list(&[head, at, self.expr]);
        }
        self.dim.translate(o);
    }

    pub fn translate_axis(&mut self, heap: &mut Heap, x: f64, a: Axis) {
        let o = if a == X { (x, 0.0) } else { (0.0, x) };
        self.translate(heap, o);
    }

    pub fn scale(&mut self, heap: &mut Heap, x: f64, y: f64) {
        let head = heap.intern("scale-stencil");
        let factors = [heap.flonum(x), heap.flonum(y)];
        let factors = heap.list(&factors);
        self.expr = heap.list(&[head, factors, self.expr]);
        self.dim.0[X].scale(x);
        self.dim.0[Y].scale(y);
    }

    /// `s` added: its expression first, in a `combine-stencil` with this
    /// one's, whose own items are spliced in.
    pub fn add_stencil(&mut self, heap: &mut Heap, s: &Stencil) {
        let cs = heap.intern("combine-stencil");
        let combined = |v: Value| heap.car(v) == Some(cs);
        if self.expr == Value::NULL {
            self.expr = s.expr;
        } else if s.expr == Value::NULL {
        } else if combined(self.expr) {
            let rest = heap.cdr(self.expr).unwrap_or(Value::NULL);
            self.expr = if combined(s.expr) {
                append(heap, s.expr, rest)
            } else {
                let tail = heap.cons(s.expr, rest);
                heap.cons(cs, tail)
            };
        } else {
            self.expr = if combined(s.expr) {
                let last = heap.list(&[self.expr]);
                append(heap, s.expr, last)
            } else {
                heap.list(&[cs, s.expr, self.expr])
            };
        }
        self.dim.unite(s.dim);
    }

    /// Aligned on axis `a` by its own extent: -1 left, 1 right, and between.
    pub fn align_to(&mut self, heap: &mut Heap, a: Axis, x: f64) {
        if self.is_empty_on(a) {
            return;
        }
        let at = -self.extent(a).linear_combination(x);
        self.translate_axis(heap, at, a);
    }

    /// `s` put next to this on axis `a` in direction `d`, `padding` apart
    /// unless either is spacing (empty on the other axis).
    pub fn add_at_edge(
        &mut self,
        heap: &mut Heap,
        a: Axis,
        d: Direction,
        s: &Stencil,
        padding: f64,
    ) {
        if self.is_empty_on(a) {
            self.add_stencil(heap, s);
            return;
        }
        let first = self.extent(a);
        if s.is_empty_on(a) {
            let mut toadd = *s;
            toadd.translate_axis(heap, first.at(d), a);
            self.add_stencil(heap, &toadd);
            return;
        }
        let next = s.extent(a);
        let first_is_spacing = self.is_empty_on(other_axis(a));
        let next_is_spacing = s.is_empty_on(other_axis(a));
        let mut offset = first.at(d) - next.at(-d);
        if !(first_is_spacing || next_is_spacing) {
            offset += f64::from(d) * padding;
        }
        let mut toadd = *s;
        toadd.translate_axis(heap, offset, a);
        self.add_stencil(heap, &toadd);
    }

    /// `s` stacked after this on axis `a` in direction `d`, as lines or
    /// columns are: `padding` apart and reference points at least `mindist`
    /// apart, unless either is spacing; `s` is drawn last.
    pub fn stack(
        &mut self,
        heap: &mut Heap,
        a: Axis,
        d: Direction,
        s: &Stencil,
        padding: f64,
        mindist: f64,
    ) {
        let df = f64::from(d);
        if self.is_empty_on(a) {
            let mut toadd = *s;
            toadd.add_stencil(heap, self);
            *self = toadd;
            return;
        }
        let first = self.extent(a);
        if s.is_empty_on(a) {
            let mut toadd = *s;
            toadd.translate_axis(heap, first.at(d), a);
            toadd.add_stencil(heap, self);
            *self = toadd;
            return;
        }
        let next = s.extent(a);
        if self.is_empty_on(other_axis(a)) {
            let mut toadd = *s;
            let offset = first.at(d) - first.at(-d);
            toadd.translate_axis(heap, offset, a);
            toadd.add_stencil(heap, self);
            *self = toadd;
            *self.dim.0[a].at_mut(-d) = next.at(-d);
            *self.dim.0[a].at_mut(d) = next.at(d) + offset;
            return;
        }
        if s.is_empty_on(other_axis(a)) {
            let mut toadd = *s;
            let offset = first.at(d);
            toadd.translate_axis(heap, offset, a);
            toadd.add_stencil(heap, self);
            *self = toadd;
            *self.dim.0[a].at_mut(-d) = first.at(-d);
            *self.dim.0[a].at_mut(d) = first.at(d) + next.at(d) - next.at(-d);
            return;
        }
        let mut offset = first.at(d);
        if df * next.at(-d) < 0.0 {
            offset -= next.at(-d);
        }
        offset += df * padding;
        if offset * df < mindist {
            offset = df * mindist;
        }
        let mut toadd = *s;
        toadd.translate_axis(heap, offset, a);
        toadd.add_stencil(heap, self);
        *self = toadd;
        *self.dim.0[a].at_mut(-d) = first.at(-d);
        *self.dim.0[a].at_mut(d) = next.at(d) + offset;
    }

    /// This stencil's ink with `outline`'s extents.
    pub fn with_outline(&self, heap: &mut Heap, outline: &Stencil) -> Stencil {
        let head = heap.intern("with-outline");
        let expr = heap.list(&[head, outline.expr, self.expr]);
        Stencil::new(outline.dim, expr)
    }

    /// `in_color`: this stencil in `color`, a CSS color's name or the list
    /// of its red, green, blue and alpha, as `stencil.scm`'s
    /// `stencil-with-color` makes it (`Lily::stencil_with_color`), from
    /// the image.
    pub fn in_color(&self, host: &mut dyn Host, color: Value) -> Result<Stencil, Exception> {
        let f = crate::lily_procedure(host, "stencil-with-color")?;
        let s = host.heap().wrap(*self);
        let colored = host.call(f, &[s, color])?;
        from_image(host.heap(), "in_color", colored)
    }
}

/// The stencil a procedure of the image gave `subr`:
/// `*LY_ASSERT_SMOB (const Stencil, v, 0)`.
pub(crate) fn from_image(heap: &mut Heap, subr: &str, v: Value) -> Result<Stencil, Exception> {
    match heap.foreign::<Stencil>(v) {
        Some(&s) => Ok(s),
        None => Err(crate::wrong_type(heap, Some(subr), 0, v, "Stencil")),
    }
}

// --- the Scheme face ---------------------------------------------------------------

/// `smobbed_copy`.
pub(crate) fn wrap(cx: &mut Cx, s: Stencil) -> R {
    Ok(cx.heap_mut().wrap(s))
}

/// `LY_ASSERT_SMOB (const Stencil, v, position)`.
fn stencil_arg(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<Stencil, Thrown> {
    match cx.heap().foreign::<Stencil>(v) {
        Some(&s) => Ok(s),
        None => Err(cx.wrong_type(subr, position, v, "Stencil")),
    }
}

/// `to_scm (Interval)`.
fn interval(cx: &mut Cx, i: Interval) -> Value {
    let (l, r) = (cx.heap_mut().flonum(i.left), cx.heap_mut().flonum(i.right));
    cx.heap_mut().cons(l, r)
}

/// A vector of `ly:angle` and `ly:length`: a number pair, or two numbers.
fn vector(cx: &mut Cx, subr: &str, args: &[Value]) -> Result<Offset, Thrown> {
    match args.get(1) {
        None => number_pair_arg(cx, subr, 1, args[0]),
        Some(&y) => {
            let x = number_arg(cx, subr, 1, args[0])?;
            Ok((x, number_arg(cx, subr, 2, y)?))
        }
    }
}

/// `(ly:angle x [y])`: the vector's angle in degrees.
pub(crate) fn angle(cx: &mut Cx, args: &[Value]) -> R {
    let v = vector(cx, "ly:angle", args)?;
    Ok(cx.heap_mut().flonum(angle_degrees(v)))
}

/// `(ly:length x [y])`: the vector's length.
pub(crate) fn ly_length(cx: &mut Cx, args: &[Value]) -> R {
    let v = vector(cx, "ly:length", args)?;
    Ok(cx.heap_mut().flonum(length(v)))
}

/// `(ly:directed direction [magnitude])`: the vector of an angle in
/// degrees or of a pair's direction, scaled by a number or by a pair's
/// coordinates.
pub(crate) fn directed(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:directed";
    let mut res = if cx.heap().car(args[0]).is_some() {
        direction(number_pair_arg(cx, SUBR, 1, args[0])?)
    } else {
        offset_directed(number_arg(cx, SUBR, 1, args[0])?)
    };
    if let Some(&magnitude) = args.get(1) {
        if cx.heap().car(magnitude).is_some() {
            let (x, y) = number_pair_arg(cx, SUBR, 2, magnitude)?;
            res = (res.0 * x, res.1 * y);
        } else {
            let m = number_arg(cx, SUBR, 2, magnitude)?;
            res = (res.0 * m, res.1 * m);
        }
    }
    Ok(offset(cx.heap_mut(), res))
}

/// `(ly:stencil-translate-axis stil amount axis)`.
pub(crate) fn translate_axis(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:stencil-translate-axis";
    let mut s = stencil_arg(cx, SUBR, 1, args[0])?;
    let amount = number_arg(cx, SUBR, 2, args[1])?;
    let a = axis_arg(cx, SUBR, 3, args[2])?;
    s.translate_axis(cx.heap_mut(), amount, a);
    wrap(cx, s)
}

/// `(ly:stencil-translate stil offset)`.
pub(crate) fn translate(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:stencil-translate";
    let mut s = stencil_arg(cx, SUBR, 1, args[0])?;
    let o = number_pair_arg(cx, SUBR, 2, args[1])?;
    s.translate(cx.heap_mut(), o);
    wrap(cx, s)
}

/// `(ly:stencil-expr stil)`.
pub(crate) fn expr(cx: &mut Cx, args: &[Value]) -> R {
    Ok(stencil_arg(cx, "ly:stencil-expr", 1, args[0])?.expr)
}

/// `(ly:stencil-extent stil axis)`.
pub(crate) fn extent(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:stencil-extent";
    let s = stencil_arg(cx, SUBR, 1, args[0])?;
    let a = axis_arg(cx, SUBR, 2, args[1])?;
    Ok(interval(cx, s.extent(a)))
}

/// `(ly:stencil-empty? stil [axis])`.
pub(crate) fn is_empty(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:stencil-empty?";
    let s = stencil_arg(cx, SUBR, 1, args[0])?;
    Ok(Value::bool(match args.get(1) {
        None => s.is_empty(),
        Some(&a) => s.is_empty_on(axis_arg(cx, SUBR, 2, a)?),
    }))
}

/// The two stencils of `ly:stencil-combine-at-edge` and `ly:stencil-stack`,
/// each a stencil, `#f` or `()`, with the axis, the direction and the
/// padding.
type Edge = (Option<Stencil>, Axis, Direction, Option<Stencil>, f64);

fn edge_args(cx: &mut Cx, subr: &str, args: &[Value]) -> Result<Edge, Thrown> {
    let mut stencils = [None; 2];
    for (s, (position, &v)) in stencils.iter_mut().zip([(1, &args[0]), (4, &args[3])]) {
        *s = cx.heap().foreign::<Stencil>(v).copied();
        if s.is_none() && v != Value::FALSE && v != Value::NULL {
            let c_name = subr.replace([':', '-'], "_");
            return Err(cx.wrong_type(&c_name, position, v, "Stencil, #f or ()"));
        }
    }
    let a = axis_arg(cx, subr, 2, args[1])?;
    let d = direction_arg(cx, subr, 3, args[2])?;
    let padding = match args.get(4) {
        Some(&p) => number_arg(cx, subr, 5, p)?,
        None => 0.0,
    };
    Ok((stencils[0], a, d, stencils[1], padding))
}

/// `(ly:stencil-combine-at-edge first axis direction second [padding])`.
pub(crate) fn combine_at_edge(cx: &mut Cx, args: &[Value]) -> R {
    let (first, a, d, second, padding) = edge_args(cx, "ly:stencil-combine-at-edge", args)?;
    let mut result = first.unwrap_or_default();
    if let Some(second) = second {
        result.add_at_edge(cx.heap_mut(), a, d, &second, padding);
    }
    wrap(cx, result)
}

/// `(ly:stencil-stack first axis direction second [padding [mindist]])`.
pub(crate) fn stack(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:stencil-stack";
    let (first, a, d, second, padding) = edge_args(cx, SUBR, args)?;
    let mindist = match args.get(5) {
        Some(&m) => number_arg(cx, SUBR, 6, m)?,
        None => f64::NEG_INFINITY,
    };
    let mut result = first.unwrap_or_default();
    if let Some(second) = second {
        result.stack(cx.heap_mut(), a, d, &second, padding, mindist);
    }
    wrap(cx, result)
}

/// `(ly:stencil-add stil …)`: a `combine-stencil` of the stencils'
/// expressions, in order, and their boxes united. LilyPond 2.26.0 means to
/// splice an expression that is a `combine-stencil` but compares the head
/// with the whole expression, so it never does, and neither does this.
pub(crate) fn add(cx: &mut Cx, args: &[Value]) -> R {
    let mut exprs = Vec::with_capacity(args.len() + 1);
    exprs.push(cx.heap_mut().intern("combine-stencil"));
    let mut dim = Box::EMPTY;
    for &arg in args {
        let Some(&s) = cx.heap().foreign::<Stencil>(arg) else {
            // `SCM_ARGn`: Guile's message without a position.
            return Err(cx.wrong_type_vm(Some("ly_stencil_add"), arg, "Stencil"));
        };
        dim.unite(s.dim);
        exprs.push(s.expr);
    }
    let expr = cx.heap_mut().list(&exprs);
    wrap(cx, Stencil::new(dim, expr))
}

/// `(%stencil-add-stencil stil other)`: `stil` with `other` added
/// ([`Stencil::add_stencil`]), which `ly:stencil-add` does not nest so.
pub(crate) fn add_stencil(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "%stencil-add-stencil";
    let mut s = stencil_arg(cx, SUBR, 1, args[0])?;
    let other = stencil_arg(cx, SUBR, 2, args[1])?;
    s.add_stencil(cx.heap_mut(), &other);
    wrap(cx, s)
}

/// `(%make-stencil expr xext yext)`, after `ly:make-stencil` has checked
/// them ([`crate::PRELUDE`]): number pairs.
pub(crate) fn make(cx: &mut Cx, args: &[Value]) -> R {
    let mut ext = [Interval::EMPTY; 2];
    for (e, &pair) in ext.iter_mut().zip(&args[1..]) {
        let (l, r) = number_pair_arg(cx, "ly:make-stencil", 2, pair)?;
        *e = Interval::new(l, r);
    }
    wrap(cx, Stencil::new(Box(ext), args[0]))
}

/// `(ly:stencil-aligned-to stil axis dir)`.
pub(crate) fn aligned_to(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:stencil-aligned-to";
    let mut s = stencil_arg(cx, SUBR, 1, args[0])?;
    let a = axis_arg(cx, SUBR, 2, args[1])?;
    let dir = number_arg(cx, SUBR, 3, args[2])?;
    s.align_to(cx.heap_mut(), a, dir);
    wrap(cx, s)
}

/// The arguments of the two rotations: the stencil, the angle and a point.
fn rotation(cx: &mut Cx, subr: &str, args: &[Value]) -> Result<(Stencil, f64, Offset), Thrown> {
    let s = stencil_arg(cx, subr, 1, args[0])?;
    let a = number_arg(cx, subr, 2, args[1])?;
    let x = number_arg(cx, subr, 3, args[2])?;
    let y = number_arg(cx, subr, 4, args[3])?;
    Ok((s, a, (x, y)))
}

/// `(ly:stencil-rotate stil angle x y)`: about the point `(x, y)` gives
/// relative to its extents.
pub(crate) fn rotate(cx: &mut Cx, args: &[Value]) -> R {
    let (mut s, a, off) = rotation(cx, "ly:stencil-rotate", args)?;
    s.rotate_degrees(cx.heap_mut(), a, off);
    wrap(cx, s)
}

/// `(ly:stencil-rotate-absolute stil angle x y)`: about the point `(x, y)`.
pub(crate) fn rotate_absolute(cx: &mut Cx, args: &[Value]) -> R {
    let (mut s, a, off) = rotation(cx, "ly:stencil-rotate-absolute", args)?;
    s.rotate_degrees_absolute(cx.heap_mut(), a, off);
    wrap(cx, s)
}

/// `(ly:stencil-scale stil x [y])`.
pub(crate) fn scale(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:stencil-scale";
    let mut s = stencil_arg(cx, SUBR, 1, args[0])?;
    let x = number_arg(cx, SUBR, 2, args[1])?;
    let y = match args.get(2) {
        Some(&y) => number_arg(cx, SUBR, 3, y)?,
        None => x,
    };
    s.scale(cx.heap_mut(), x, y);
    wrap(cx, s)
}

/// `(ly:stencil-outline stil outline)`.
pub(crate) fn outline(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:stencil-outline";
    let s = stencil_arg(cx, SUBR, 1, args[0])?;
    let o = stencil_arg(cx, SUBR, 2, args[1])?;
    let result = s.with_outline(cx.heap_mut(), &o);
    wrap(cx, result)
}

/// `(ly:stencil? x)`.
pub(crate) fn is_stencil(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(cx.heap().foreign::<Stencil>(args[0]).is_some()))
}
