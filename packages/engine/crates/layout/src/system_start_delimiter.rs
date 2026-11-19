//! `system-start-delimiter.cc` (part 3's, MS3 step 40): the delimiter's
//! callback `ly:system-start-delimiter::print` (`:94–143`, 50 lines) over
//! its four styles, `staff_bracket` (`:35–68`, 34), `line_bracket`
//! (`:70–90`, 21), `simple_bar` (`:83–90`) and `staff_brace` (`:145–157`,
//! 13), the music crate's placeholder until then (D67). It is Rust under a
//! primitive that prints as LilyPond's subr and calls it over `%host-call`
//! (D127, [`HOSTS`]), as the stem's are.
//!
//! The bracket's tips are the accidental's `Font_metric::find_by_name`
//! (`accidental.rs`) in the default font (`font_select.rs`); the brace is
//! `make-left-brace-markup` interpreted on the grob (`text.rs`'
//! `%grob-interpret-markup`); `line-thickness` and `output-scale` are
//! `Output_def::get_dimension`'s (`grob::layout_dimension`); the staff
//! space the referencer's `%staff-space`.

use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::accidental::find_by_name;
use crate::font_select::{self, POINT};
use crate::geometry::{Box, Direction, Interval, Offset, X, Y};
use crate::line::make_line;
use crate::lookup;
use crate::spacing_options::double_or;
use crate::stencil::Stencil;
use crate::{grob, grob_array, spanner};

const LEFT: Direction = -1;
const DOWN: Direction = -1;
const UP: Direction = 1;

/// The stencil `s`, its expression rooted until the caller unroots it.
fn kept(host: &mut dyn Host, s: Stencil) -> Stencil {
    host.heap().root(s.expr);
    s
}

/// `staff_bracket`: a filled line `thickness` wide (0.25 by default)
/// `height` high and a tenth of the thickness more at each end, the
/// default font's `brackettips.down` and `brackettips.up` at its ends,
/// overlapping it by as much; its X extent the line's, its Y extent the
/// whole's; moved 0.8 left.
fn staff_bracket(
    host: &mut dyn Host,
    me: Value,
    height: f64,
    warnings: &mut Vec<String>,
) -> Result<Stencil, Exception> {
    let fm = font_select::get_default_font(host, me)?;
    host.heap().root(fm);
    let tips = [
        find_by_name(host, fm, "brackettips.down")?,
        find_by_name(host, fm, "brackettips.up")?,
    ];

    let thickness_scm = grob::property(host, me, "thickness")?;
    let thickness = double_or(host, thickness_scm, 0.25);

    let overlap = 0.1 * thickness;

    let half = height / 2.0 + overlap;
    let bracket_line_extents = Box::new(Interval::new(0.0, thickness), Interval::new(-half, half));

    let heap = host.heap();
    let mut bracket = lookup::filled_box(heap, bracket_line_extents, warnings);
    for (d, tip) in [(DOWN, tips[0]), (UP, tips[1])] {
        bracket.add_at_edge(heap, Y, d, &tip, -overlap);
    }

    // The reference for positioning the delimiter in X-direction should
    // be the bracket line, not the right bound of the bracket tips.
    // In Y-direction we have to take the tips into account, however,
    // to ensure correct bounding boxes with the EPS backend.
    // Therefore we take the X-dimensions only from the bracket line
    // and the Y-dimensions from the whole bracket.
    let bracket_extents = Box::new(bracket_line_extents.x(), bracket.extent(Y));
    let mut bracket = Stencil::new(bracket_extents, bracket.expr);

    bracket.translate_axis(heap, -0.8, X);

    Ok(bracket)
}

/// `line_bracket`: a vertical line `height` high with a tip 0.8 long to
/// the right at each end, `line-thickness` times `thickness` (1 by
/// default) thick, moved 0.8 left.
fn line_bracket(host: &mut dyn Host, me: Value, height: f64) -> Result<Stencil, Exception> {
    let lt = grob::layout_dimension(host, me, "line-thickness")?;
    let thickness_scm = grob::property(host, me, "thickness")?;
    let thick = lt * double_or(host, thickness_scm, 1.0);
    let w = 0.8;

    let heap = host.heap();
    let at = |x: f64, y: f64| -> Offset { (x, y) };
    let tip1 = make_line(heap, thick, at(0.0, -height / 2.0), at(w, -height / 2.0));
    let tip2 = make_line(heap, thick, at(0.0, height / 2.0), at(w, height / 2.0));
    let mut vline = make_line(heap, thick, at(0.0, -height / 2.0), at(0.0, height / 2.0));

    vline.add_stencil(heap, &tip1);
    vline.add_stencil(heap, &tip2);
    vline.translate_axis(heap, -w, X);
    Ok(vline)
}

/// `simple_bar`: a box `line-thickness` times `thickness` (1 by default)
/// wide and `h` high about 0, its corners rounded by the line thickness.
fn simple_bar(
    host: &mut dyn Host,
    me: Value,
    h: f64,
    warnings: &mut Vec<String>,
) -> Result<Stencil, Exception> {
    let lt = grob::layout_dimension(host, me, "line-thickness")?;
    let thickness_scm = grob::property(host, me, "thickness")?;
    let w = lt * double_or(host, thickness_scm, 1.0);
    let b = Box::new(Interval::new(0.0, w), Interval::new(-h / 2.0, h / 2.0));
    Ok(lookup::round_filled_box(host.heap(), b, lt, warnings))
}

/// `staff_brace`: the `left-brace` markup of `y` (in points at the
/// paper's `output-scale`) interpreted on the grob, centred on X and
/// moved 0.2 left.
fn staff_brace(host: &mut dyn Host, me: Value, y: f64) -> Result<Stencil, Exception> {
    let output_scale = grob::layout_dimension(host, me, "output-scale")?;
    let size = host.heap().flonum(y * output_scale / POINT);
    let mkup = crate::call_lily(host, "make-left-brace-markup", &[size])?;
    host.heap().root(mkup);
    let v = crate::call_lily(host, "%grob-interpret-markup", &[me, mkup])?;
    host.heap().root(v);
    let heap = host.heap();
    let mut stil = heap.foreign::<Stencil>(v).copied().unwrap_or_default();
    stil.align_to(heap, X, 0.0);
    stil.translate_axis(heap, -0.2, X);
    Ok(stil)
}

/// `print`: the delimiter of its `style` (`bracket`, `brace`, `bar-line`
/// or `line-bracket`; none for another) as high as the union of the Y
/// extents of its `elements` that are spanners beginning where it does,
/// centred on it, the staff space the last nonempty one's (from the
/// last); killed, giving `*unspecified*`, where that union is empty or
/// no more than `collapse-height` staff spaces.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let elts = grob_array::extract_grob_array(host, me, "elements")?;
    for &e in &elts {
        host.heap().root(e);
    }
    // None, LilyPond's null: extents up to their roots.
    let common =
        grob::common_refpoint_of_array(host.heap(), &elts, Some(me), Y).unwrap_or(Value::NULL);

    let mut ext = Interval::EMPTY;
    let mut staffspace = 1.0;
    for &sp in elts.iter().rev() {
        if spanner::is_spanner(host.heap(), sp)
            && spanner::get_bound(host, sp, LEFT)? == spanner::get_bound(host, me, LEFT)?
        {
            let dims = grob::extent(host, sp, common, Y)?;
            if !dims.is_empty() {
                ext.unite(dims);
                let ss = crate::call_lily(host, "%staff-space", &[sp])?;
                staffspace = grob::to_double(host.heap(), ss)?;
            }
        }
    }

    let glyph_sym = grob::property(host, me, "style")?;
    host.heap().root(glyph_sym);
    let len = ext.length();

    // Use collapse-height in multiples of the staff-space
    if ext.is_empty() || {
        let collapse = grob::property(host, me, "collapse-height")?;
        double_or(host, collapse, 0.0) >= len / staffspace
    } {
        grob::suicide(host.heap(), me);
        return Ok(Value::UNSPECIFIED);
    }

    let mut warnings = Vec::new();
    let m = match host.heap().name(glyph_sym) {
        Some("bracket") => staff_bracket(host, me, len, &mut warnings)?,
        Some("brace") => staff_brace(host, me, len)?,
        Some("bar-line") => simple_bar(host, me, len, &mut warnings)?,
        Some("line-bracket") => line_bracket(host, me, len)?,
        _ => Stencil::default(),
    };
    let mut m = kept(host, m);

    m.translate_axis(host.heap(), ext.center(), Y);
    let m = host.heap().wrap(m);
    // What a warning could free: the stencil.
    host.heap().root(m);
    warnings
        .iter()
        .try_for_each(|text| grob::unlocated_warning(host, text))?;
    Ok(m)
}

// --- the callback -----------------------------------------------------------

/// What `%host-call` runs for the `ly:system-start-delimiter::` primitive
/// (D127): the callback its second argument names, of the grob given,
/// with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "system-start-delimiter",
    f: host_system_start_delimiter,
}];

fn host_system_start_delimiter(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    match vm.heap().name(args[1]) {
        Some("print") => print(vm, args[2]),
        _ => Err(crate::misc_error(
            vm.heap(),
            "%host-call",
            "no such system-start-delimiter callback",
        )),
    }
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:system-start-delimiter::print grob)`: LilyPond's subr, a
/// primitive here so that it prints as LilyPond's. It checks its grob and
/// tail-calls `%host-call` ([`HOSTS`]).
pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    let grob = args[0];
    grob::grob_arg(cx, "ly:system-start-delimiter::print", 1, grob)?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("system-start-delimiter");
    let print = cx.heap_mut().intern("print");
    Ok(cx.tail_call(f, &[what, print, grob]))
}
