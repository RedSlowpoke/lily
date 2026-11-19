//! LilyPond's line interface (`line-interface.cc`): plain, dashed, dotted
//! and zigzag lines and arrows, drawn from a grob's style properties
//! ([`LineStyle`]); `line-interface-scheme.cc`'s `ly:line-interface::line`
//! is [`crate::lookup`]'s, since the engine's grobs have no staff symbol
//! before MS3's part 2. The trill line needs the music font (part 1's step
//! 5) and a stencil's skylines: it throws by name (D67).
//!
//! `Line_interface::line` on a grob ([`grob_line`], MS3 step 49, which the
//! hairpins and the brackets draw with) reads the grob's staff symbol and
//! properties in LilyPond's order and draws as [`line`] does; since MS3
//! step 52 (OPEN A23) `Line_interface::arrows` on a grob ([`grob_arrows`],
//! the line spanners') too, and `ly:line-interface::line` draws on the grob
//! it is given over `%host-call` ([`HOSTS`]).

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, Heap, HostHandler, Thrown, Value, Vm};

use crate::geometry::{Box, Offset, complex_multiply, direction, fma, length, max, min};
use crate::lookup::round_polygon;
use crate::stencil::Stencil;

/// The `style` of the line interface: `line`, `dashed-line`,
/// `dotted-line`, `zigzag`, `trill` or `none`; any other is a plain line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Line,
    DashedLine,
    DottedLine,
    Zigzag,
    Trill,
    None,
}

impl Style {
    /// The style a symbol's name gives.
    pub fn from_name(name: &str) -> Style {
        match name {
            "dashed-line" => Style::DashedLine,
            "dotted-line" => Style::DottedLine,
            "zigzag" => Style::Zigzag,
            "trill" => Style::Trill,
            "none" => Style::None,
            _ => Style::Line,
        }
    }
}

/// What `Line_interface` reads of a grob: its staff symbol's line
/// thickness and staff space (`Staff_symbol_referencer`), and its
/// properties, `None` where one is not a number (`from_scm`'s default).
#[derive(Clone, Copy, Debug)]
pub struct LineStyle {
    pub line_thickness: f64,
    pub staff_space: f64,
    pub style: Style,
    pub thickness: Option<f64>,
    pub dash_fraction: Option<f64>,
    pub dash_period: Option<f64>,
    pub zigzag_width: Option<f64>,
    pub zigzag_length: Option<f64>,
    pub arrow_length: Option<f64>,
    pub arrow_width: Option<f64>,
}

impl LineStyle {
    /// A plain line of the staff symbol's thickness and space, no property
    /// set.
    pub fn new(line_thickness: f64, staff_space: f64) -> LineStyle {
        LineStyle {
            line_thickness,
            staff_space,
            style: Style::Line,
            thickness: None,
            dash_fraction: None,
            dash_period: None,
            zigzag_width: None,
            zigzag_length: None,
            arrow_length: None,
            arrow_width: None,
        }
    }

    /// The staff symbol's line thickness times the grob's `thickness`.
    fn thick(&self) -> f64 {
        self.line_thickness * self.thickness.unwrap_or(1.0)
    }
}

/// The vector from `from` to `to`.
fn delta(from: Offset, to: Offset) -> Offset {
    (to.0 - from.0, to.1 - from.1)
}

/// `Line_interface::make_line`: a `draw-line` from `from` to `to`, `th`
/// thick.
pub fn make_line(heap: &mut Heap, th: f64, from: Offset, to: Offset) -> Stencil {
    let head = heap.intern("draw-line");
    let values = [th, from.0, from.1, to.0, to.1].map(|x| heap.flonum(x));
    let expr = heap.list(&[head, values[0], values[1], values[2], values[3], values[4]]);
    let mut b = Box::EMPTY;
    b.add_point(from);
    b.add_point(to);
    b.widen(th / 2.0, th / 2.0);
    Stencil::new(b, expr)
}

/// `Line_interface::make_dashed_line`: dashes of `dash_fraction` of each
/// `dash_period`, drawn from the origin and moved to `from`.
pub fn make_dashed_line(
    heap: &mut Heap,
    thick: f64,
    from: Offset,
    to: Offset,
    dash_period: f64,
    dash_fraction: f64,
) -> Stencil {
    let dash_fraction = min(max(dash_fraction, 0.0), 1.0);
    let on = dash_fraction * dash_period;
    let off = max(0.0, dash_period - on);
    let d = delta(from, to);
    let head = heap.intern("dashed-line");
    let values = [thick, on, off, d.0, d.1, 0.0].map(|x| heap.flonum(x));
    let mut items = alloc::vec![head];
    items.extend(values);
    let expr = heap.list(&items);
    let mut b = Box::EMPTY;
    b.add_point((0.0, 0.0));
    b.add_point(d);
    b.widen(thick / 2.0, thick / 2.0);
    let mut m = Stencil::new(b, expr);
    m.translate(heap, from);
    m
}

/// `Line_interface::make_arrow`: a filled head at `end`, pointing away from
/// `begin`, `length` long and `width` to each side.
pub fn make_arrow(
    heap: &mut Heap,
    begin: Offset,
    end: Offset,
    thick: f64,
    length: f64,
    width: f64,
) -> Stencil {
    let dir = direction(delta(begin, end));
    let points = [(0.0, 0.0), (-length, width), (-length, -width)].map(|p| {
        let q = complex_multiply(p, dir);
        (q.0 + end.0, q.1 + end.1)
    });
    round_polygon(heap, &points, thick, -1.0, true)
}

/// `Line_interface::make_zigzag_line`: whole zigzags of `zigzag-width`
/// staff spaces, or a little less, each side `zigzag-length` times as long.
pub fn make_zigzag_line(heap: &mut Heap, s: &LineStyle, from: Offset, to: Offset) -> Stencil {
    let dz = delta(from, to);
    let thick = s.thick();
    let mut w = s.zigzag_width.unwrap_or(1.0) * s.staff_space;
    let count = libm::ceil(length(dz) / w) as i32;
    w = length(dz) / f64::from(count);
    let l = s.zigzag_length.unwrap_or(1.0) * w;
    let h = if l > w / 2.0 {
        libm::sqrt(fma(l, l, -(w * w / 4.0)))
    } else {
        0.0
    };
    let rotation = direction(dz);
    let [p0, p1, p2] =
        [(0.0, -h / 2.0), (w / 2.0, h / 2.0), (w, -h / 2.0)].map(|p| complex_multiply(p, rotation));
    let mut squiggle = make_line(heap, thick, p0, p1);
    let second = make_line(heap, thick, p1, p2);
    squiggle.add_stencil(heap, &second);
    let mut total = Stencil::default();
    for i in 0..count {
        let mut moved = squiggle;
        let step = complex_multiply((f64::from(i) * w, 0.0), rotation);
        moved.translate(heap, (from.0 + step.0, from.1 + step.1));
        total.add_stencil(heap, &moved);
    }
    total
}

/// `Line_interface::line`: the line from `from` to `to` in the grob's
/// style. A dashed line begins and ends with a dash, its period stretched
/// to fit; a dotted line is a dashed one of no dash. The trill line needs
/// the grob's font: [`grob_line`] draws it ([`make_trill_line`]), and this
/// throws by name.
pub fn line(cx: &mut Cx, s: &LineStyle, from: Offset, to: Offset) -> Result<Stencil, Thrown> {
    if s.style == Style::Trill {
        return Err(cx.error("misc-error", Some(TRILL), TRILL_UNPORTED, &[]));
    }
    Ok(draw(cx.heap_mut(), s, from, to))
}

/// Where the trill line throws, and why.
const TRILL: &str = "Line_interface::make_trill_line";
const TRILL_UNPORTED: &str = "needs the grob's font: Line_interface::line on a grob draws it";

/// [`line`] of any style but the trill line's, which is a plain line here.
fn draw(heap: &mut Heap, s: &LineStyle, from: Offset, to: Offset) -> Stencil {
    let thick = s.thick();
    let fraction = match s.style {
        Style::Zigzag => return make_zigzag_line(heap, s, from, to),
        Style::Trill | Style::None => return Stencil::default(),
        Style::Line => return make_line(heap, thick, from, to),
        Style::DottedLine => 0.0,
        Style::DashedLine => s.dash_fraction.unwrap_or(0.4),
    };
    let fraction = min(max(fraction, 0.0), 1.0);
    let mut period = s.staff_space * s.dash_period.unwrap_or(1.0);
    if period <= 0.0 {
        return Stencil::default();
    }
    let len = length(delta(from, to));
    let full = (libm::rint(fma(-period, fraction, len) / period) as i32).max(0);
    if full > 0 {
        period = len / (fraction + f64::from(full));
    }
    make_dashed_line(heap, thick, from, to, period, fraction)
}

/// `Line_interface::arrows`: heads at `to`, at `from` or at both, of
/// `arrow-length` and `arrow-width` (1.3 and 0.5 staff spaces unset).
pub fn arrows(
    heap: &mut Heap,
    s: &LineStyle,
    from: Offset,
    to: Offset,
    from_arrow: bool,
    to_arrow: bool,
) -> Stencil {
    let mut a = Stencil::default();
    if from_arrow || to_arrow {
        let thick = s.thick();
        let len = s.arrow_length.unwrap_or(1.3 * s.staff_space);
        let wid = s.arrow_width.unwrap_or(0.5 * s.staff_space);
        if to_arrow {
            let head = make_arrow(heap, from, to, thick, len, wid);
            a.add_stencil(heap, &head);
        }
        if from_arrow {
            let head = make_arrow(heap, to, from, thick, len, wid);
            a.add_stencil(heap, &head);
        }
    }
    a
}

/// The style a grob's `style` property gives: a symbol's, else a plain
/// line.
pub fn style_of(heap: &Heap, v: Value) -> Style {
    heap.name(v).map_or(Style::Line, Style::from_name)
}

/// `from_scm<double> (get_property (me, name))`: the property if it is a
/// real number.
fn real_property(host: &mut dyn Host, me: Value, name: &str) -> Result<Option<f64>, Exception> {
    let v = crate::grob::property(host, me, name)?;
    let heap = host.heap();
    Ok(heap.to_f64(v).filter(|_| heap.is_real(v)))
}

/// `Line_interface::line` on a grob (`line-interface.cc:219–274`): the
/// line from `from` to `to` in the grob's `style`, its thickness the staff
/// symbol's line thickness times the grob's `thickness`; a zigzag of its
/// `zigzag-width` and `zigzag-length`, a dashed line of its
/// `dash-fraction` and `dash-period`, each read only for its style. The
/// trill line throws by name, as [`line`]'s. The stencil's expression is
/// rooted until the caller unroots it.
pub fn grob_line(
    host: &mut dyn Host,
    me: Value,
    from: Offset,
    to: Offset,
) -> Result<Stencil, Exception> {
    let line_thickness = crate::staff_symbol_referencer::line_thickness(host, me)?;
    let mut s = LineStyle::new(line_thickness, 1.0);
    s.thickness = real_property(host, me, "thickness")?;
    let style = crate::grob::property(host, me, "style")?;
    s.style = style_of(host.heap(), style);
    match s.style {
        Style::Trill => return make_trill_line(host, me, from, to),
        Style::None => return Ok(Stencil::default()),
        Style::Zigzag => {
            // make_zigzag_line reads the thickness again, then these.
            s.thickness = real_property(host, me, "thickness")?;
            s.staff_space = crate::staff_symbol_referencer::staff_space(host, me)?;
            s.zigzag_width = real_property(host, me, "zigzag-width")?;
            s.zigzag_length = real_property(host, me, "zigzag-length")?;
        }
        Style::DashedLine | Style::DottedLine => {
            if s.style == Style::DashedLine {
                s.dash_fraction = real_property(host, me, "dash-fraction")?;
            }
            s.staff_space = crate::staff_symbol_referencer::staff_space(host, me)?;
            s.dash_period = real_property(host, me, "dash-period")?;
        }
        Style::Line => {}
    }
    let heap = host.heap();
    let stencil = draw(heap, &s, from, to);
    heap.root(stencil.expr);
    Ok(stencil)
}

/// `Line_interface::make_trill_line` (`line-interface.cc:48–109`, MS8
/// step 8, part 1, OPEN A19): the grob's font's `scripts.trill_element`,
/// centred on Y, as many as fit from `from` to `to` after the first, each
/// further one `add_at_edge` to the right and adding its extent's length,
/// the first its true length (its skylines' extent on X, as the elements
/// overlap); the whole of that length, rotated to the line's angle about
/// its left end's middle and moved to `from`. An improbable offset (over
/// 10⁶) is 0, with LilyPond's unlocated programming error. The stencil's expression
/// is rooted until the caller unroots it.
pub fn make_trill_line(
    host: &mut dyn Host,
    me: Value,
    from: Offset,
    to: Offset,
) -> Result<Stencil, Exception> {
    let mut dz = delta(from, to);
    for a in [0, 1] {
        let d = if a == 0 { &mut dz.0 } else { &mut dz.1 };
        if d.is_infinite() || d.is_nan() || libm::fabs(*d) > 1e6 {
            // printf's `%f`.
            let f = if d.is_nan() {
                alloc::string::String::from("nan")
            } else if d.is_infinite() {
                alloc::string::String::from(if *d < 0.0 { "-inf" } else { "inf" })
            } else {
                alloc::format!("{:.6}", *d)
            };
            let text =
                alloc::format!("Improbable offset for stencil: {f} staff space\nSetting to zero.");
            *d = 0.0;
            let text = host.heap().string(text.replace('~', "~~"));
            crate::call_lily(host, "ly:programming-error", &[text])?;
        }
    }

    let fm = crate::font_select::get_default_font(host, me)?;
    host.heap().root(fm);
    let mut elt = crate::accidental::find_by_name(host, fm, "scripts.trill_element")?;
    let heap = host.heap();
    heap.root(elt.expr);
    elt.align_to(heap, crate::geometry::Y, 0.0);
    heap.root(elt.expr);
    let elt_len = elt.extent(crate::geometry::X).length();
    // Get the real length of the trill element, so as not to exceed the
    // allotted length for the line. The element sticks out of its bounding
    // box so that two elements blend when concatenated.
    let elt_true_ext = crate::skylines::true_extent(heap, &elt, crate::geometry::Y);
    let elt_true_len = elt_true_ext.length();
    if elt_true_len <= 0.0 || elt_true_ext.is_empty() {
        crate::grob::programming_error(host, me, "can't find scripts.trill_element")?;
        return Ok(elt);
    }
    // Always have at least one trill element, even if the space allotted
    // technically doesn't allow it.
    let mut line = elt;
    line.translate_axis(heap, -elt_true_ext.left, crate::geometry::X);
    heap.root(line.expr);
    let mut total_len = elt_true_len;
    let delta_len = length(dz) - total_len;
    if delta_len > 0.0 {
        // First trill element takes true_elt_len, each further element only
        // adds elt_len because of the overlap.
        let num_extra_elements = (delta_len / elt_len) as usize;
        for _ in 0..num_extra_elements {
            line.add_at_edge(heap, crate::geometry::X, 1, &elt, 0.0);
            heap.root(line.expr);
        }
        total_len += num_extra_elements as f64 * elt_len;
    }
    let b = line.dim;
    let new_b = Box::new(crate::geometry::Interval::new(0.0, total_len), b.y());
    let mut new_line = Stencil::new(new_b, line.expr);

    new_line.rotate_degrees(heap, crate::geometry::angle_degrees(dz), (-1.0, 0.0));
    heap.root(new_line.expr);
    new_line.translate(heap, from);
    heap.root(new_line.expr);
    Ok(new_line)
}

/// `Line_interface::arrows` on a grob (`line-interface.cc:195–217`): no
/// arrow, or the heads [`arrows`] draws, their thickness the staff
/// symbol's line thickness times the grob's `thickness`, their
/// `arrow-length` and `arrow-width` the grob's or 1.3 and 0.5 staff
/// spaces, read in LilyPond's order. The stencil's expression is rooted
/// until the caller unroots it.
pub fn grob_arrows(
    host: &mut dyn Host,
    me: Value,
    from: Offset,
    to: Offset,
    from_arrow: bool,
    to_arrow: bool,
) -> Result<Stencil, Exception> {
    if !(from_arrow || to_arrow) {
        return Ok(Stencil::default());
    }
    let line_thickness = crate::staff_symbol_referencer::line_thickness(host, me)?;
    let mut s = LineStyle::new(line_thickness, 1.0);
    s.thickness = real_property(host, me, "thickness")?;
    s.staff_space = crate::staff_symbol_referencer::staff_space(host, me)?;
    s.arrow_length = real_property(host, me, "arrow-length")?;
    s.arrow_width = real_property(host, me, "arrow-width")?;
    let heap = host.heap();
    let stencil = arrows(heap, &s, from, to, from_arrow, to_arrow);
    heap.root(stencil.expr);
    Ok(stencil)
}

/// What `%host-call` runs for `ly:line-interface::line` (D127,
/// `lookup.rs`' primitive, which checked its arguments): [`grob_line`] on
/// the grob from its two points, with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "line-interface-line",
    f: host_line,
}];

fn host_line(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let heap = vm.heap_mut();
    let [x1, y1, x2, y2] = [2, 3, 4, 5].map(|i| heap.to_f64(args[i]).unwrap_or(0.0));
    let depth = heap.root_depth();
    let drawn = grob_line(vm, args[1], (x1, y1), (x2, y2));
    let heap = vm.heap_mut();
    let v = drawn.map(|s| heap.wrap(s));
    heap.unroot_to(depth);
    v
}
