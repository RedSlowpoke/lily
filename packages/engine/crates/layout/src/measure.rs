//! `measure-grouping-spanner.cc` and `measure-spanner.cc` (MS8 step 8,
//! part 2; 74 and 109 lines): `ly:measure-grouping::print` and
//! `ly:measure-spanner::print`, the music crate's placeholders until then
//! (D67), Rust under primitives that print as LilyPond's subrs and call it
//! over `%host-call` (D127, [`HOSTS`]). `Measure_grouping_engraver` and
//! `Measure_spanner_engraver` are Scheme engravers of LilyPond's image.
//!
//! - The grouping (`:28–61`): from its left bound's coordinate to its right
//!   bound's extent's centre, a `bracket` `height` deep or a `triangle`
//!   `height` high by its `style`, `thickness` staff lines thick, its
//!   bottom at 0.
//! - The measure spanner (`:30–94`): between its bounds' break-align widths
//!   (by `spacing-pair`, else `staff-bar`), a bracket on X towards its
//!   direction where `bracket-visibility` is true, gapped about its middle
//!   for its `text`, which is centred over it.

use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::bracket::make_axis_constrained_bracket;
use crate::callbacks::Callback;
use crate::direction::get_grob_direction;
use crate::geometry::{Interval, X, Y};
use crate::spacing_options::double_or;
use crate::stencil::Stencil;
use crate::text::is_markup;
use crate::{grob, lookup, spanner};

/// The spanner's two bounds and their common X refpoint, if it has them.
fn bounds_and_common(
    host: &mut dyn Host,
    me: Value,
) -> Result<Option<([Value; 2], Value)>, Exception> {
    let left = spanner::get_bound(host, me, -1)?;
    let right = spanner::get_bound(host, me, 1)?;
    let (Some(left), Some(right)) = (left, right) else {
        return Ok(None);
    };
    Ok(grob::common_refpoint(host.heap(), left, right, X).map(|c| ([left, right], c)))
}

/// `Measure_grouping::print`, as above.
pub fn grouping_print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = grouping_print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn grouping_print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let which = grob::property(host, me, "style")?;
    let height = grob::property(host, me, "height")?;
    let height = double_or(host, height, 1.0);

    let lt = crate::staff_symbol_referencer::line_thickness(host, me)?;
    let thickness = grob::property(host, me, "thickness")?;
    let t = lt * double_or(host, thickness, 1.0);
    // LilyPond's reads through a missing bound.
    let Some(([lb, rb], common)) = bounds_and_common(host, me)? else {
        return Ok(host.heap().wrap(Stencil::default()));
    };

    let right_point = grob::robust_relative_extent(host, rb, common, X)?.center();
    let left_point = grob::relative_coordinate(host, lb, common, X)?;

    let iv = Interval::new(left_point, right_point);
    let mut m = Stencil::default();

    let mut warnings = Vec::new();
    match host.heap().name(which) {
        Some("bracket") => m = lookup::bracket(host.heap(), X, iv, t, -height, t, &mut warnings),
        Some("triangle") => m = lookup::triangle(host.heap(), iv, t, height),
        _ => {}
    }
    let heap = host.heap();
    heap.root(m.expr);
    for text in &warnings {
        grob::unlocated_warning(host, text)?;
    }

    let heap = host.heap();
    m.align_to(heap, Y, -1.0);
    heap.root(m.expr);
    let x = grob::relative_coordinate(host, me, common, X)?;
    m.translate_axis(host.heap(), -x, X);
    Ok(host.heap().wrap(m))
}

/// `Measure_spanner::print`, as above.
pub fn spanner_print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = spanner_print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn spanner_print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let mut mol = Stencil::default();
    let txt = grob::property(host, me, "text")?;
    host.heap().root(txt);

    let visible = grob::property(host, me, "bracket-visibility")?;

    // LilyPond's reads through a missing bound.
    let Some((bounds, common_x)) = bounds_and_common(host, me)? else {
        return Ok(host.heap().wrap(mol));
    };
    let mut x_points = [0.0; 2];

    let sp = grob::property(host, me, "spacing-pair")?;
    for (i, d) in [-1.0, 1.0].into_iter().enumerate() {
        let heap = host.heap();
        let align_syms = if heap.car(sp).is_some() {
            if i == 0 { heap.car(sp) } else { heap.cdr(sp) }.unwrap_or(Value::NULL)
        } else {
            heap.intern("staff-bar")
        };
        let w = crate::call_lily(host, "%break-align-width", &[bounds[i], align_syms])?;
        let w = grob::number_pair(host.heap(), w).unwrap_or(Interval::EMPTY);
        x_points[i] = if d < 0.0 { w.right } else { w.left };
    }

    let mut bracket_text = Stencil::default();
    let mut gap_iv = Interval::EMPTY;

    if is_markup(host, txt)? {
        let s = crate::call_lily(host, "%grob-interpret-markup", &[me, txt])?;
        host.heap().root(s);
        bracket_text = host
            .heap()
            .foreign::<Stencil>(s)
            .copied()
            .unwrap_or_default();
        let heap = host.heap();
        bracket_text.align_to(heap, X, 0.0);
        let stil_y_ext = bracket_text.extent(Y);
        bracket_text.translate_axis(heap, (x_points[1] - x_points[0]) / 2.0, X);
        bracket_text.translate_axis(heap, -stil_y_ext.right / 2.0, Y);
        heap.root(bracket_text.expr);
        let gap = bracket_text.extent(X).length();
        gap_iv = Interval::new(-0.5, 0.5);
        gap_iv.scale(gap);
        gap_iv.widen(0.6);
    }

    let mut brack = Stencil::default();
    if visible != Value::FALSE {
        let dir = get_grob_direction(host, me)?;
        brack = make_axis_constrained_bracket(host, me, x_points[1] - x_points[0], X, dir, gap_iv)?;
    }

    let heap = host.heap();
    if !bracket_text.is_empty() {
        brack.add_stencil(heap, &bracket_text);
        heap.root(brack.expr);
    }

    mol.add_stencil(heap, &brack);
    heap.root(mol.expr);

    let me_coord = grob::relative_coordinate(host, me, common_x, X)?;

    mol.translate_axis(host.heap(), x_points[0] - me_coord, X);
    Ok(host.heap().wrap(mol))
}

// --- the callbacks ------------------------------------------------------------

/// What `%host-call` runs for the `ly:measure-grouping::` and
/// `ly:measure-spanner::` callbacks (D127).
pub(crate) const HOSTS: [HostHandler; 2] = [
    HostHandler {
        what: "measure-grouping",
        f: host_grouping,
    },
    HostHandler {
        what: "measure-spanner",
        f: host_spanner,
    },
];

/// The `ly:measure-grouping::` callback, by name.
pub(crate) const GROUPING_CALLBACKS: [(&str, Callback); 1] = [("print", grouping_print)];

/// The `ly:measure-spanner::` callback, by name.
pub(crate) const SPANNER_CALLBACKS: [(&str, Callback); 1] = [("print", spanner_print)];

fn host_grouping(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(
        vm,
        &GROUPING_CALLBACKS,
        args,
        "no such measure-grouping callback",
    )
}

fn host_spanner(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(
        vm,
        &SPANNER_CALLBACKS,
        args,
        "no such measure-spanner callback",
    )
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_grouping_print(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "measure-grouping",
        "ly:measure-grouping::",
        "print",
        args,
    )
}

pub(crate) fn ly_spanner_print(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(cx, "measure-spanner", "ly:measure-spanner::", "print", args)
}
