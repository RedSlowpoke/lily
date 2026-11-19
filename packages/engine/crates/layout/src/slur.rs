//! `slur.cc` (MS4 step 7): the Slur's and PhrasingSlur's callbacks, their
//! control points slur-scoring.cc's (`slur_scoring.rs`, step 8):
//! `ly:slur::calc-direction` (`:47–68`), `::pure-height` (`:70–131`, a
//! pure callback that reads its columns), `::height` (`:133–142`),
//! `::print` (`:144–175`), `::pure-outside-slur-callback` (`:229–253`),
//! `::outside-slur-callback` (`:255–356`), `::outside-slur-cross-staff`
//! (`:405–425`) and `::calc-cross-staff` (`:427–453`); and what the slur
//! engravers and the scoring call: `replace_breakable_encompass_objects`
//! (`:177–211`), `get_curve` (`:213–217`), `add_column` (`:219–224`),
//! `add_extra_encompass` (`:226–231`) and
//! `auxiliary_acknowledge_extra_object` (`:358–399`). The vertical
//! skylines are the stencil's (`grob::unpure-vertical-skylines-from-stencil`).
//!
//! The callbacks are subrs that print as LilyPond's and call their Rust
//! over `%host-call` (D127, [`HOSTS`]); all but the pure ones in 05 §5.1's
//! callback table, the pure height among its pure callbacks that read
//! their columns (`callbacks.rs`).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::bezier;
use crate::callbacks::{Callback, Columns};
use crate::direction::get_grob_direction;
use crate::geometry::{Direction, Interval, X, Y, max, min};
use crate::grob;
use crate::grob_array::{self, GrobArray};
use crate::lookup::{self, Bezier};
use crate::note_column;
use crate::spanner;
use crate::staff_symbol_referencer::line_thickness;
use crate::stencil::Stencil;
use crate::unpure;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const DOWN: Direction = -1;
const UP: Direction = 1;
const CENTER: Direction = 0;

type Run<T> = Result<T, Exception>;

/// `to_scm (Interval)`.
fn interval_value(host: &mut dyn Host, iv: Interval) -> Value {
    crate::offset(host.heap(), (iv.left, iv.right))
}

/// `from_scm<double> (v, fallback)`.
fn double_or(host: &mut dyn Host, v: Value, fallback: f64) -> f64 {
    let heap = host.heap();
    if heap.is_real(v) {
        heap.to_f64(v).unwrap_or(fallback)
    } else {
        fallback
    }
}

/// `from_scm<double> (get_property (me, name), fallback)`.
fn real_or(host: &mut dyn Host, me: Value, name: &str, fallback: f64) -> Run<f64> {
    let v = grob::property(host, me, name)?;
    Ok(double_or(host, v, fallback))
}

/// `minmax (d, a, b)`: whichever of `a` and `b` is further along `d`, `a`
/// at a tie (`std::max`, `std::min`).
fn minmax(d: Direction, a: f64, b: f64) -> f64 {
    if d > 0 { max(a, b) } else { min(a, b) }
}

/// The grob in the object `name` of `me`, if it is one.
fn grob_object(host: &mut dyn Host, me: Value, name: &str) -> Run<Option<Value>> {
    let sym = host.heap().intern(name);
    let v = grob::object(host, me, sym)?;
    Ok(grob_of(host.heap(), v).map(|_| v))
}

/// `calc_direction`: up if a column of notes without rests has its stem
/// down, else down; a slur over no column is killed.
pub fn calc_direction(host: &mut dyn Host, me: Value) -> Run<Value> {
    let encompasses = grob_array::extract_grob_array(host, me, "note-columns")?;

    if encompasses.is_empty() {
        grob::suicide(host.heap(), me);
        return Ok(Value::FALSE);
    }

    let mut d = DOWN;
    for col in encompasses {
        if !note_column::has_rests(host, col)? && note_column::dir(host, col)? == DOWN {
            d = UP;
            break;
        }
    }
    Ok(host.heap().integer(i64::from(d)))
}

/// `pure_height`: from the encompassed columns' pure heights between the
/// columns `start` and `end`, the range of their edges on the slur's side,
/// its inner end no further in than the nearer of the outer columns',
/// half a staff space out; nothing for a slur whose columns are not
/// under its parent (a cross-staff one).
pub fn pure_height(host: &mut dyn Host, me: Value, start: usize, end: usize) -> Run<Value> {
    /*
      Note that this estimation uses a rote add-on of 0.5 to the
      highest encompassed note-head for a slur estimate.  This is,
      in most cases, shorter than the actual slur.

      Ways to improve this could include:
      -- adding extra height for scripts that avoid slurs on the inside
      -- adding extra height for the "bulge" in a slur above a note head
    */
    let dir = get_grob_direction(host, me)?;

    let encompasses = grob_array::extract_grob_array(host, me, "note-columns")?;
    let mut ret = Interval::EMPTY;

    let parent = grob::get_parent(host.heap(), me, Y);
    let mut extremal_heights = [f64::INFINITY, f64::NEG_INFINITY];
    if grob::common_refpoint_of_array(host.heap(), &encompasses, Some(me), Y) != parent {
        /* this could happen if, for example, we are a cross-staff slur.
        in this case, we want to be ignored */
        return Ok(interval_value(host, Interval::EMPTY));
    }
    let Some(parent) = parent else {
        return Ok(interval_value(host, Interval::EMPTY));
    };

    for &col in &encompasses {
        let d = unpure::pure_y_extent(host, col, parent, start, end)?;
        if !d.is_empty() {
            ret.add_point(d.at(dir));

            if extremal_heights[0] == f64::INFINITY {
                extremal_heights[0] = d.at(dir);
            }
            extremal_heights[1] = d.at(dir);
        }
    }

    if ret.is_empty() {
        return Ok(interval_value(host, Interval::EMPTY));
    }

    let mut extremal_span = Interval::EMPTY;
    for h in extremal_heights {
        extremal_span.add_point(h);
    }
    *ret.at_mut(-dir) = minmax(dir, extremal_span.at(-dir), ret.at(-dir));

    /*
      The +0.5 comes from the fact that we try to place a slur
      0.5 staff spaces from the note-head.
      (see Slur_score_state.get_base_attachments ())
    */
    ret.translate(0.5 * f64::from(dir));
    Ok(interval_value(host, ret))
}

/// `height`: the stencil's height.
pub fn height(host: &mut dyn Host, me: Value) -> Run<Value> {
    // FIXME uncached
    grob::stencil_height(host, me)
}

/// `get_curve`: `control-points` as `from_scm<Bezier>` reads them: the
/// first four pairs of the list, (0, 0) for those it lacks.
pub fn get_curve(host: &mut dyn Host, me: Value) -> Run<Bezier> {
    let mut l = grob::property(host, me, "control-points")?;
    let mut curve: Bezier = [(0.0, 0.0); 4];
    for c in &mut curve {
        let heap = host.heap();
        let (Some(point), Some(rest)) = (heap.car(l), heap.cdr(l)) else {
            break;
        };
        l = rest;
        let (Some(x), Some(y)) = (heap.car(point), heap.cdr(point)) else {
            return Err(grob::to_double(heap, point).expect_err("no pair is a number"));
        };
        *c = (grob::to_double(heap, x)?, grob::to_double(heap, y)?);
    }
    Ok(curve)
}

/// `print`: the curve of `control-points` made `thickness` thick in its
/// middle and `line-thickness` at its ends, in staff line thicknesses,
/// dashed by `dash-definition`, with its `annotation` (a string) on its
/// side; a slur over no column is killed.
pub fn print(host: &mut dyn Host, me: Value) -> Run<Value> {
    let encompasses = grob_array::extract_grob_array(host, me, "note-columns")?;
    if encompasses.is_empty() {
        grob::suicide(host.heap(), me);
        return Ok(Value::NULL);
    }

    let staff_thick = line_thickness(host, me)?;
    let base_thick = staff_thick * real_or(host, me, "thickness", 1.0)?;
    let line_thick = staff_thick * real_or(host, me, "line-thickness", 1.0)?;

    let one = get_curve(host, me)?;

    let dash_definition = grob::property(host, me, "dash-definition")?;
    let dir = get_grob_direction(host, me)?;
    let mut errors = Vec::new();
    let mut a = lookup::slur(
        host.heap(),
        one,
        f64::from(dir) * base_thick,
        line_thick,
        dash_definition,
        &mut errors,
    );
    let depth = host.heap().root_depth();
    host.heap().root(a.expr);
    let printed = print_rest(host, me, dir, &mut a, &errors);
    host.heap().unroot_to(depth);
    printed?;
    Ok(host.heap().wrap(a))
}

/// What `print` does after the curve, `a` rooted: `Bezier::extract`'s
/// programming errors written, the annotation added.
fn print_rest(
    host: &mut dyn Host,
    me: Value,
    dir: Direction,
    a: &mut Stencil,
    errors: &[String],
) -> Run<()> {
    for text in errors {
        grob::unlocated_programming_error(host, text)?;
    }

    let annotation = grob::property(host, me, "annotation")?;
    if host.heap().str(annotation).is_some() {
        let tm = crate::call_lily(host, "%grob-interpret-markup", &[me, annotation])?;
        if let Some(tm) = host.heap().foreign::<Stencil>(tm).copied() {
            let heap = host.heap();
            a.add_at_edge(heap, Y, dir, &tm, 1.0);
            heap.root(a.expr);
        }
    }
    Ok(())
}

/// `replace_breakable_encompass_objects`: of the slur's
/// `encompass-objects`, each separation item replaced by those of its
/// `elements` that are on the slur's staff and avoid the slur inside.
pub fn replace_breakable_encompass_objects(host: &mut dyn Host, me: Value) -> Run<()> {
    let extra_objects = grob_array::extract_grob_array(host, me, "encompass-objects")?;
    let mut new_encompasses = Vec::new();

    for g in extra_objects {
        if grob::has_interface(host.heap(), g, "separation-item-interface") {
            let breakables = grob_array::extract_grob_array(host, g, "elements")?;
            for b in breakables {
                /* if we encompass a separation-item that spans multiple staves,
                we filter out the grobs that don't belong to our staff */
                let heap = host.heap();
                let on_staff =
                    grob::common_refpoint(heap, me, b, Y) == grob::get_parent(heap, me, Y);
                if on_staff {
                    let avoid = grob::property(host, b, "avoid-slur")?;
                    if avoid == host.heap().intern("inside") {
                        new_encompasses.push(b);
                    }
                }
            }
        } else {
            new_encompasses.push(g);
        }
    }

    let sym = host.heap().intern("encompass-objects");
    let arr = grob::object(host, me, sym)?;
    if let Some(a) = host.heap().foreign_mut::<GrobArray>(arr) {
        a.grobs = new_encompasses;
    }
    Ok(())
}

/// `add_column`: the note column `n` added to the slur's `note-columns`
/// and its bounds.
pub fn add_column(host: &mut dyn Host, me: Value, n: Value) -> Run<()> {
    let sym = host.heap().intern("note-columns");
    grob_array::add_grob(host, me, sym, n)?;
    spanner::add_bound_item(host, me, n)
}

/// `add_extra_encompass`: `n` added to the slur's `encompass-objects`.
pub fn add_extra_encompass(host: &mut dyn Host, me: Value, n: Value) -> Run<()> {
    let sym = host.heap().intern("encompass-objects");
    grob_array::add_grob(host, me, sym, n)
}

/// Whether the script's `avoid-slur` is `outside` or `around`.
fn avoids_outside(host: &mut dyn Host, avoid: Value) -> bool {
    let heap = host.heap();
    avoid == heap.intern("outside") || avoid == heap.intern("around")
}

/// `pure_outside_slur_callback`: the offset `offset` (0 if no number) of a
/// script that avoids its slur outside or around, moved a quarter of the
/// slur's pure height between the columns towards its side; unchanged
/// otherwise, or without a slur.
pub fn pure_outside_slur_callback(
    host: &mut dyn Host,
    script: Value,
    start: usize,
    end: usize,
    offset_scm: Value,
) -> Run<Value> {
    let Some(slur) = grob_object(host, script, "slur")? else {
        return Ok(offset_scm);
    };

    let avoid = grob::property(host, script, "avoid-slur")?;
    if !avoids_outside(host, avoid) {
        return Ok(offset_scm);
    }

    let offset = double_or(host, offset_scm, 0.0);
    let dir = get_grob_direction(host, script)?;
    let h = unpure::pure_y_extent(host, slur, slur, start, end)?.length();
    Ok(host.heap().flonum(offset + f64::from(dir) * h / 4.0))
}

/// `outside_slur_callback`: the offset `offset` (0 if no number) of a
/// script beside its slur that avoids it outside or around, moved clear of
/// the curve by `slur-padding` where they meet; unchanged otherwise.
pub fn outside_slur_callback(host: &mut dyn Host, script: Value, offset_scm: Value) -> Run<Value> {
    let Some(slur) = grob_object(host, script, "slur")? else {
        return Ok(offset_scm);
    };

    let avoid = grob::property(host, script, "avoid-slur")?;
    if !avoids_outside(host, avoid) {
        return Ok(offset_scm);
    }

    let dir = get_grob_direction(host, script)?;
    if dir == CENTER {
        return Ok(offset_scm);
    }

    let heap = host.heap();
    let cx = grob::common_refpoint(heap, script, slur, X);
    let cy = grob::common_refpoint(heap, script, slur, Y);
    let (Some(cx), Some(cy)) = (cx, cy) else {
        return Ok(offset_scm);
    };

    let mut curve = get_curve(host, slur)?;

    let shift = (
        grob::relative_coordinate(host, slur, cx, X)?,
        grob::relative_coordinate(host, slur, cy, Y)?,
    );
    bezier::translate(&mut curve, shift);

    let mut yext = grob::robust_relative_extent(host, script, cy, Y)?;
    let xext = grob::robust_relative_extent(host, script, cx, X)?;
    let slur_wid = Interval::new(curve[0].0, curve[3].0);

    /*
      cannot use is_empty because some 0-extent scripts
      come up with TabStaffs.
    */
    if xext.length() <= 0.0 || yext.length() <= 0.0 {
        return Ok(offset_scm);
    }

    let contains = |iv: Interval, x: f64| iv.left <= x && x <= iv.right;
    if !(contains(slur_wid, xext.left) || contains(slur_wid, xext.right)) {
        return Ok(offset_scm);
    }

    let offset = double_or(host, offset_scm, 0.0);
    yext.translate(offset);

    /* FIXME: slur property, script property?  */
    let slur_padding = real_or(host, script, "slur-padding", 0.0)?;
    yext.widen(slur_padding);

    let exts = [xext, yext];
    let mut do_shift = false;
    const EPS: f64 = 1.0e-5;
    let mut errors = Vec::new();
    if avoid == host.heap().intern("outside") {
        for d in [LEFT, RIGHT] {
            let end = if d == LEFT { curve[0].0 } else { curve[3].0 };
            let x = minmax(-d, xext.at(d), end + f64::from(-d) * EPS);
            let y = bezier::get_other_coordinate(&curve, X, x, &mut errors);
            do_shift = y == minmax(dir, yext.at(-dir), y);
            if do_shift {
                break;
            }
        }
    } else {
        'axes: for a in [X, Y] {
            for d in [LEFT, RIGHT] {
                let coords = bezier::get_other_coordinates(&curve, a, exts[a].at(d));
                for c in coords {
                    do_shift = contains(exts[crate::geometry::other_axis(a)], c);
                    if do_shift {
                        break 'axes;
                    }
                }
            }
        }
    }

    let avoidance_offset = if do_shift {
        bezier::minmax(
            &curve,
            X,
            max(xext.left, curve[0].0 + EPS),
            min(xext.right, curve[3].0 - EPS),
            dir,
            &mut errors,
        ) - yext.at(-dir)
    } else {
        0.0
    };

    for text in &errors {
        grob::unlocated_programming_error(host, text)?;
    }
    Ok(host.heap().flonum(offset + avoidance_offset))
}

/// `auxiliary_acknowledge_extra_object`, which both slur engravers call: a
/// tie or a grob that avoids the slur inside is encompassed by each of
/// `slurs` and `end_slurs`; one that avoids it outside or around has the
/// outside-slur offset and cross-staff callbacks chained; each given the
/// first of `slurs` (or of `end_slurs` if `slurs` is empty) as its `slur`.
/// Another, but one that ignores slurs, is warned about.
pub fn auxiliary_acknowledge_extra_object(
    host: &mut dyn Host,
    e: Value,
    slurs: &[Value],
    end_slurs: &[Value],
) -> Run<()> {
    if slurs.is_empty() && end_slurs.is_empty() {
        return Ok(());
    }

    let avoid = grob::property(host, e, "avoid-slur")?;
    let slur = if !end_slurs.is_empty() && slurs.is_empty() {
        end_slurs[0]
    } else {
        slurs[0]
    };

    let inside = host.heap().intern("inside");
    if grob::has_interface(host.heap(), e, "tie-interface") || avoid == inside {
        for &s in slurs.iter().rev() {
            add_extra_encompass(host, s, e)?;
        }
        for &s in end_slurs.iter().rev() {
            add_extra_encompass(host, s, e)?;
        }
        let sym = host.heap().intern("slur");
        grob::set_object(host.heap(), e, sym, slur);
    } else if avoids_outside(host, avoid) {
        let unpure = crate::lily_procedure(host, "ly:slur::outside-slur-callback")?;
        let pure = crate::lily_procedure(host, "ly:slur::pure-outside-slur-callback")?;
        let container = crate::call_lily(host, "ly:make-unpure-pure-container", &[unpure, pure])?;
        grob::chain_offset_callback(host, e, container, Y)?;
        let cross_staff = crate::lily_procedure(host, "ly:slur::outside-slur-cross-staff")?;
        let sym = host.heap().intern("cross-staff");
        grob::chain_callback(host, e, cross_staff, sym)?;
        let sym = host.heap().intern("slur");
        grob::set_object(host.heap(), e, sym, slur);
    } else if avoid != host.heap().intern("ignore") {
        let name = grob::grob_name(host.heap(), e);
        let text = format!("Ignoring grob for slur: {name}.  avoid-slur not set?");
        grob::warning(host, e, &text)?;
    }
    Ok(())
}

/// `outside_slur_cross_staff`, chained on the `cross-staff` of a grob
/// placed outside or around a slur: cross-staff if it was, or if its slur
/// is.
pub fn outside_slur_cross_staff(host: &mut dyn Host, me: Value, previous: Value) -> Run<Value> {
    if previous == Value::TRUE {
        return Ok(previous);
    }

    let Some(slur) = grob_object(host, me, "slur")? else {
        return Ok(Value::FALSE);
    };
    grob::property(host, slur, "cross-staff")
}

/// `calc_cross_staff`: whether a column's stem is cross-staff, or the
/// columns and the encompassed objects but the separation items are not
/// all under the slur's parent.
pub fn calc_cross_staff(host: &mut dyn Host, me: Value) -> Run<Value> {
    let cols = grob_array::extract_grob_array(host, me, "note-columns")?;
    let extras = grob_array::extract_grob_array(host, me, "encompass-objects")?;

    for &col in &cols {
        if let Some(s) = note_column::get_stem(host, col)?
            && grob::property(host, s, "cross-staff")? == Value::TRUE
        {
            return Ok(Value::TRUE);
        }
    }

    /* the separation items are dealt with in replace_breakable_encompass_objects
    so we can ignore them here */
    let heap = host.heap();
    let non_sep_extras: Vec<Value> = extras
        .into_iter()
        .filter(|&g| !grob::has_interface(heap, g, "separation-item-interface"))
        .collect();

    let common = grob::common_refpoint_of_array(heap, &cols, Some(me), Y);
    let common = grob::common_refpoint_of_array(heap, &non_sep_extras, common, Y);

    Ok(Value::bool(common != grob::get_parent(heap, me, Y)))
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:slur::` primitives (D127): the
/// callback its second argument names, of the grob given (the pure height
/// with its columns), and the outside-slur callbacks, of the grob and the
/// offset or value given, with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 4] = [
    HostHandler {
        what: "slur",
        f: host_slur,
    },
    HostHandler {
        what: "slur-outside",
        f: host_outside,
    },
    HostHandler {
        what: "slur-pure-outside",
        f: host_pure_outside,
    },
    HostHandler {
        what: "slur-outside-cross-staff",
        f: host_outside_cross_staff,
    },
];

/// The `ly:slur::` callbacks [`HOSTS`] runs, by name, which the callback
/// table (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 5] = [
    (
        "calc-control-points",
        crate::slur_scoring::calc_control_points,
    ),
    ("calc-cross-staff", calc_cross_staff),
    ("calc-direction", calc_direction),
    ("height", height),
    ("print", print),
];

/// The pure callback that reads its columns, which the callback table
/// calls.
pub(crate) const PURE_CALLBACKS: [(&str, Columns); 1] = [("pure-height", pure_height)];

/// `from_scm<vsize>` of a column given over `%host-call`, as the subr
/// checked it.
pub(crate) fn column(heap: &lily_engine_scheme::Heap, v: Value) -> usize {
    heap.to_i64(v)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0)
}

fn host_slur(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    if vm.heap().name(args[1]) == Some("pure-height") {
        let (start, end) = (column(vm.heap(), args[3]), column(vm.heap(), args[4]));
        return pure_height(vm, args[2], start, end);
    }
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such slur callback")
}

fn host_outside(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    outside_slur_callback(vm, args[1], args[2])
}

fn host_pure_outside(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let (start, end) = (column(vm.heap(), args[2]), column(vm.heap(), args[3]));
    pure_outside_slur_callback(vm, args[1], start, end, args[4])
}

fn host_outside_cross_staff(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    outside_slur_cross_staff(vm, args[1], args[2])
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `%host-call` tail-called with `args`.
pub(crate) fn tail_host_call(cx: &mut Cx, args: &[Value]) -> R {
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    Ok(cx.tail_call(f, args))
}

/// A `ly:slur::` subr: its grob checked under the callback's name, as
/// LilyPond's `LY_ASSERT_SMOB` gives it, and `%host-call` tail-called with
/// the callback's name ([`HOSTS`]).
fn host_call(cx: &mut Cx, name: &str, args: &[Value]) -> R {
    let method = format!("ly:slur::{name}");
    grob::grob_arg(cx, &method, 1, args[0])?;
    let what = cx.heap_mut().intern("slur");
    let name = cx.heap_mut().intern(name);
    tail_host_call(cx, &[what, name, args[0]])
}

macro_rules! callbacks {
    ($($f:ident $name:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            host_call(cx, $name, args)
        })*
    };
}

callbacks! {
    ly_calc_control_points "calc-control-points",
    ly_calc_cross_staff "calc-cross-staff",
    ly_calc_direction "calc-direction",
    ly_height "height",
    ly_print "print",
}

/// A column argument of `subr`: `from_scm<vsize>`'s exact non-negative
/// integer.
pub(crate) fn column_arg(
    cx: &mut Cx,
    subr: &str,
    pos: usize,
    v: Value,
) -> Result<Value, lily_engine_scheme::Thrown> {
    let heap = cx.heap();
    let ok = heap.is_exact_integer(v) && heap.to_i64(v).is_some_and(|n| n >= 0);
    if ok {
        Ok(v)
    } else {
        Err(cx.wrong_type(subr, pos, v, "vsize"))
    }
}

/// `(ly:slur::pure-height grob start end)`.
pub(crate) fn ly_pure_height(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:slur::pure-height";
    grob::grob_arg(cx, SUBR, 1, args[0])?;
    let start = column_arg(cx, SUBR, 2, args[1])?;
    let end = column_arg(cx, SUBR, 3, args[2])?;
    let what = cx.heap_mut().intern("slur");
    let name = cx.heap_mut().intern("pure-height");
    tail_host_call(cx, &[what, name, args[0], start, end])
}

/// `(ly:slur::outside-slur-callback grob [offset])`, an `offset` not given
/// being no number.
pub(crate) fn ly_outside_slur_callback(cx: &mut Cx, args: &[Value]) -> R {
    grob::grob_arg(cx, "ly:slur::outside-slur-callback", 1, args[0])?;
    let what = cx.heap_mut().intern("slur-outside");
    let offset = args.get(1).copied().unwrap_or(Value::UNSPECIFIED);
    tail_host_call(cx, &[what, args[0], offset])
}

/// `(ly:slur::pure-outside-slur-callback grob start end [offset])`: the
/// columns `from_scm`'s, 0 for what is none.
pub(crate) fn ly_pure_outside_slur_callback(cx: &mut Cx, args: &[Value]) -> R {
    grob::grob_arg(cx, "ly:slur::pure-outside-slur-callback", 1, args[0])?;
    let what = cx.heap_mut().intern("slur-pure-outside");
    let offset = args.get(3).copied().unwrap_or(Value::UNSPECIFIED);
    let zero = cx.heap_mut().integer(0);
    let col = |cx: &mut Cx, v: Value| {
        let heap = cx.heap();
        if heap.is_exact_integer(v) && heap.to_i64(v).is_some_and(|n| n >= 0) {
            v
        } else {
            zero
        }
    };
    let (start, end) = (col(cx, args[1]), col(cx, args[2]));
    tail_host_call(cx, &[what, args[0], start, end, offset])
}

/// `(ly:slur::outside-slur-cross-staff grob previous)`.
pub(crate) fn ly_outside_slur_cross_staff(cx: &mut Cx, args: &[Value]) -> R {
    grob::grob_arg(cx, "ly:slur::outside-slur-cross-staff", 1, args[0])?;
    let what = cx.heap_mut().intern("slur-outside-cross-staff");
    tail_host_call(cx, &[what, args[0], args[1]])
}
