//! `bracket.cc` (part 4's, MS3 step 49, 156 lines): the bracket the volta,
//! ottava and horizontal brackets draw, `make_bracket` (`:42–90`),
//! `make_axis_constrained_bracket` (`:96–129`) and
//! `make_enclosing_bracket` (`:136–156`), each of its lines
//! `Line_interface::line` on the grob (`line.rs`' [`grob_line`]).
//!
//! The stencils' expressions are rooted until the caller unroots them, as
//! [`grob_line`]'s are.

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Heap, Value};

use crate::geometry::{Axis, Direction, Interval, Offset, length, other_axis};
use crate::grob;
use crate::line::grob_line;
use crate::stencil::Stencil;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// A `Drul_array`'s index of `d`: 0 for `LEFT`, 1 for `RIGHT`.
fn side(d: Direction) -> usize {
    usize::from(d == RIGHT)
}

/// The coordinate of `o` on `a`.
fn at_mut(o: &mut Offset, a: Axis) -> &mut f64 {
    if a == 0 { &mut o.0 } else { &mut o.1 }
}

/// `from_scm (v, Drul_array (l, r))`: a pair of real numbers, else the
/// fallback.
pub(crate) fn real_drul(heap: &Heap, v: Value, fallback: [f64; 2]) -> [f64; 2] {
    match (heap.car(v), heap.cdr(v)) {
        (Some(l), Some(r)) if heap.is_real(l) && heap.is_real(r) => [
            heap.to_f64(l).unwrap_or(fallback[0]),
            heap.to_f64(r).unwrap_or(fallback[1]),
        ],
        _ => fallback,
    }
}

/// `from_scm (get_property (me, name), Drul_array (l, r))`.
pub(crate) fn drul_property(
    host: &mut dyn Host,
    me: Value,
    name: &str,
    fallback: [f64; 2],
) -> Result<[f64; 2], Exception> {
    let v = grob::property(host, me, name)?;
    Ok(real_drul(host.heap(), v, fallback))
}

/// `make_bracket`: a line from the origin to `dz` along the bracket's axis,
/// the other axis `protrusion_axis`, each end shortened by `shorten` and
/// moved in by `flare`, with a gap of `gap` about its middle unless that
/// is empty, and at each end a line `height` out along `protrusion_axis`.
/// A `dashed-line` `style` without `dashed-edge` is set to `line` before
/// the ends are drawn, as LilyPond does.
#[allow(clippy::too_many_arguments)]
pub fn make_bracket(
    host: &mut dyn Host,
    me: Value, // for line properties.
    protrusion_axis: Axis,
    dz: Offset,
    height: [f64; 2],
    gap: Interval,
    flare: [f64; 2],
    shorten: [f64; 2],
) -> Result<Stencil, Exception> {
    let corners = [(0.0, 0.0), dz];

    let length = length(dz);
    let mut gap_corners = [(0.0, 0.0); 2];

    let bracket_axis = other_axis(protrusion_axis);

    let mut straight_corners = corners;

    for d in [LEFT, RIGHT] {
        let k = -f64::from(d) * shorten[side(d)] / length;
        let c = &mut straight_corners[side(d)];
        *c = (c.0 + k * dz.0, c.1 + k * dz.1);
    }

    if !gap.is_empty() {
        for d in [LEFT, RIGHT] {
            let k = gap.at(d) / length;
            gap_corners[side(d)] = (dz.0 * 0.5 + k * dz.0, dz.1 * 0.5 + k * dz.1);
        }
    }

    let mut flare_corners = straight_corners;
    for d in [LEFT, RIGHT] {
        let i = side(d);
        *at_mut(&mut flare_corners[i], bracket_axis) =
            *at_mut(&mut straight_corners[i], bracket_axis);
        *at_mut(&mut flare_corners[i], protrusion_axis) += height[i];
        *at_mut(&mut straight_corners[i], bracket_axis) += -f64::from(d) * flare[i];
    }

    let mut m = Stencil::default();
    if !gap.is_empty() {
        for d in [LEFT, RIGHT] {
            let l = grob_line(host, me, straight_corners[side(d)], gap_corners[side(d)])?;
            m.add_stencil(host.heap(), &l);
            host.heap().root(m.expr);
        }
    } else {
        let l = grob_line(host, me, straight_corners[0], straight_corners[1])?;
        m.add_stencil(host.heap(), &l);
        host.heap().root(m.expr);
    }

    let style = grob::property(host, me, "style")?;
    if host.heap().name(style) == Some("dashed-line")
        && grob::property(host, me, "dashed-edge")? != Value::TRUE
    {
        let line = host.heap().intern("line");
        grob::set_property(host.heap(), me, "style", line);
    }
    for d in [LEFT, RIGHT] {
        let l = grob_line(host, me, straight_corners[side(d)], flare_corners[side(d)])?;
        m.add_stencil(host.heap(), &l);
        host.heap().root(m.expr);
    }
    Ok(m)
}

/// `make_axis_constrained_bracket`: a bracket `length` long along `a`,
/// pointing `dir`, of the grob's `edge-height` (1, 1), `bracket-flare`,
/// `shorten-pair` and, at a side it `connect-to-neighbor`s, no edge or
/// flare and lengthened by its `break-overshoot`. An empty `gap` makes an
/// unbroken bracket.
pub fn make_axis_constrained_bracket(
    host: &mut dyn Host,
    me: Value,
    length: f64,
    a: Axis,
    dir: Direction,
    gap: Interval,
) -> Result<Stencil, Exception> {
    let mut edge_height = drul_property(host, me, "edge-height", [1.0, 1.0])?;
    let mut flare = drul_property(host, me, "bracket-flare", [0.0, 0.0])?;
    let mut shorten = drul_property(host, me, "shorten-pair", [0.0, 0.0])?;
    let overshoot = drul_property(host, me, "break-overshoot", [0.0, 0.0])?;

    // Make sure that it points in the correct direction:
    for h in &mut edge_height {
        *h *= -f64::from(dir);
    }

    let mut start = (0.0, 0.0);
    *at_mut(&mut start, a) = length;

    let v = grob::property(host, me, "connect-to-neighbor")?;
    let heap = host.heap();
    // from_scm<Drul_array<bool>>: a pair of booleans, each true if #t.
    let connect_to_other = match (heap.car(v), heap.cdr(v)) {
        (Some(l), Some(r))
            if [l, r]
                .iter()
                .all(|&b| b == Value::TRUE || b == Value::FALSE) =>
        {
            [l == Value::TRUE, r == Value::TRUE]
        }
        _ => [false, false],
    };

    for i in 0..2 {
        if connect_to_other[i] {
            edge_height[i] = 0.0;
            flare[i] = 0.0;
            shorten[i] = -overshoot[i];
        }
    }

    make_bracket(
        host,
        me,
        other_axis(a),
        start,
        edge_height,
        gap,
        flare,
        shorten,
    )
}

/// `make_enclosing_bracket`: an unbroken bracket along `a` over the
/// extent of `grobs` there, relative to `refpoint`; for an empty extent,
/// LilyPond's programming error and no stencil.
pub fn make_enclosing_bracket(
    host: &mut dyn Host,
    me: Value,
    refpoint: Value,
    grobs: &[Value],
    a: Axis,
    dir: Direction,
) -> Result<Stencil, Exception> {
    let common = grob::common_refpoint_of_array(host.heap(), grobs, Some(refpoint), a)
        .unwrap_or(Value::NULL);
    let ext = crate::axis_group::relative_group_extent(host, grobs, common, a)?;

    if ext.is_empty() {
        grob::programming_error(host, me, "Can't enclose empty extents with bracket")?;
        Ok(Stencil::default())
    } else {
        let mut b = make_axis_constrained_bracket(host, me, ext.length(), a, dir, Interval::EMPTY)?;
        let rc = grob::relative_coordinate(host, refpoint, common, a)?;
        b.translate_axis(host.heap(), ext.at(LEFT) - rc, a);
        host.heap().root(b.expr);
        Ok(b)
    }
}
