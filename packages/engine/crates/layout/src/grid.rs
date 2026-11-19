//! `grid-point-engraver.cc`, `grid-line-span-engraver.cc` and
//! `grid-line-interface.cc` (MS8 step 8, part 2; 78, 95 and 103 lines):
//! `Grid_point_engraver` and `Grid_line_span_engraver`, classes of
//! layout's ([`GRID_POINT_ENGRAVER`], [`GRID_LINE_SPAN_ENGRAVER`]) that
//! shadow the music crate's placeholders of their names (D67, D108), and
//! `ly:grid-line-interface::print` and `::width`, Rust under primitives
//! that print as LilyPond's subrs and call it over `%host-call` (D127,
//! [`HOSTS`]).
//!
//! - `Grid_point_engraver::process_music`: a `GridPoint` where the moment's
//!   main part is a whole multiple of a finite `gridInterval`.
//! - `Grid_line_span_engraver`: the timestep's grid points acknowledged; at
//!   the second, a `GridLine` on the first's X; at
//!   `stop_translation_timestep` each point is one of its `elements`.
//! - `print`: a filled box `thickness` line thicknesses wide over the
//!   points' united Y extents; with none, the line killed and `'()`.
//!   `width`: from 0 to that thickness.

use alloc::boxed::Box as Boxed;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Host, Kind, PROCESS, Rational, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::{Box, Interval, X, Y};
use crate::paper_column_engraver::rational;
use crate::spacing_options::double_or;
use crate::{grob, grob_array, lookup};

type Run<T> = Result<T, Exception>;

pub static GRID_POINT_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Grid_point_engraver",
    kind: Kind::Engraver,
    methods: PROCESS,
    always: true,
    hears: &[],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Boxed::new(GridPointEngraver)),
    reads: &["gridInterval"],
    writes: &[],
};

#[derive(Clone, Default)]
struct GridPointEngraver;

impl Work for GridPointEngraver {
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let v = tx.get("gridInterval");
        let grid_interval = rational(tx.heap(), v, Rational::INFINITY);
        if grid_interval.is_finite() && !grid_interval.is_zero() {
            let now = tx.now().main;
            // `!now.main_part_.mod_rat (grid_interval)`: the quotient whole.
            if now
                .checked_div(grid_interval)
                .is_some_and(|q| q.is_finite() && q.denominator() == 1)
            {
                tx.make_defined_grob("GridPoint", Cause::None)?;
            }
        }
        Ok(())
    }
}

pub static GRID_LINE_SPAN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Grid_line_span_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    always: false,
    hears: &[],
    acknowledges: &[("grid-point-interface", false)],
    must_be_last: false,
    make: Some(|| Boxed::new(GridLineSpanEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `spanline_` and `lines_`, by their handles.
#[derive(Clone, Default)]
struct GridLineSpanEngraver {
    spanline: Option<Value>,
    lines: Vec<Value>,
}

impl Work for GridLineSpanEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        self.lines.push(g);
        if self.lines.len() >= 2 && self.spanline.is_none() {
            let id = tx.make_defined_grob("GridLine", Cause::None)?;
            let line = tx.grob_handle(id);
            grob::set_parent(tx.heap(), line, self.lines[0], X);
            self.spanline = Some(line);
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(line) = self.spanline.take() {
            let sym = tx.heap().intern("elements");
            for &point in &self.lines {
                grob_array::add_grob(tx.host, line, sym, point)?;
            }
        }
        self.lines.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.spanline);
        out.extend(&self.lines);
    }
}

// --- the callbacks ------------------------------------------------------------

/// The line's thickness: `thickness` (1) times the layout's
/// `line-thickness`.
fn thickness(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    let staffline = grob::layout_dimension(host, me, "line-thickness")?;
    let t = grob::property(host, me, "thickness")?;
    Ok(double_or(host, t, 1.0) * staffline)
}

/// `Grid_line_interface::print`, as above.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let elts = grob_array::extract_grob_array(host, me, "elements")?;
    // compute common refpoint of elements
    let refp = grob::common_refpoint_of_array(host.heap(), &elts, Some(me), Y).unwrap_or(me);
    let mut iv = Interval::EMPTY;
    for &point in &elts {
        iv.unite(grob::extent(host, point, refp, Y)?);
    }
    if iv.is_empty() {
        grob::suicide(host.heap(), me);
        return Ok(Value::NULL);
    }
    let thick = thickness(host, me)?;
    iv.translate(-grob::relative_coordinate(host, me, refp, Y)?);
    let mut warnings = Vec::new();
    let st = lookup::filled_box(
        host.heap(),
        Box::new(Interval::new(0.0, thick), iv),
        &mut warnings,
    );
    host.heap().root(st.expr);
    for text in &warnings {
        grob::unlocated_warning(host, text)?;
    }
    Ok(host.heap().wrap(st))
}

/// `Grid_line_interface::width`, as above.
pub fn width(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let thick = thickness(host, me)?;
    let heap = host.heap();
    let (l, r) = (heap.flonum(0.0), heap.flonum(thick));
    Ok(heap.cons(l, r))
}

/// What `%host-call` runs for the `ly:grid-line-interface::` callbacks
/// (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "grid-line-interface",
    f: host_grid_line,
}];

/// The `ly:grid-line-interface::` callbacks, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 2] = [("print", print), ("width", width)];

fn host_grid_line(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such grid-line-interface callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "grid-line-interface",
        "ly:grid-line-interface::",
        "print",
        args,
    )
}

pub(crate) fn ly_width(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "grid-line-interface",
        "ly:grid-line-interface::",
        "width",
        args,
    )
}
