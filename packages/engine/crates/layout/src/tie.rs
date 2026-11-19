//! `tie.cc`: `set_head` (`tie.cc:43–47`) and `head` (`:49–54`), which
//! `Tie_engraver` and `Accidental_engraver` call, ported first (MS3 step
//! 17); `get_column_rank` (`:56–60`), `get_position` (`:62–82`) and
//! `get_control_points` (`:186–201`), which the formatting problem and the
//! tie column read (MS4 step 9); and the rest (step 10): `get_default_dir`
//! (`:84–126`), the callbacks `ly:tie::calc-direction` (`:128–150`),
//! `::calc-control-points` (`:203–230`, with `get_default_control_points`,
//! `:152–169`) and `::print` (`:232–280`), primitives over `%host-call`
//! (D127, [`HOSTS`]), as are the tie column's and the semi-tie's.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::direction::{get_grob_direction, set_grob_direction};
use crate::geometry::{Direction, X, Y};
use crate::grob;
use crate::grob_array::{self, GrobArray};
use crate::item;
use crate::lookup::{self, Bezier};
use crate::paper_column;
use crate::rhythmic_head;
use crate::spanner;
use crate::staff_symbol_referencer::{self as ssr, line_thickness};
use crate::stem;
use crate::stencil::Stencil;
use crate::tie_configuration::{Configuration, Details, property_data};
use crate::tie_formatting_problem::Problem;

/// `set_head`: `h` the tie's bound at `d`.
pub fn set_head(host: &mut dyn Host, me: Value, d: Direction, h: Value) -> Result<(), Exception> {
    spanner::set_bound(host, me, d, h)
}

/// `head`: the tie's bound at `d` if it is a note head.
pub fn head(host: &mut dyn Host, me: Value, d: Direction) -> Result<Option<Value>, Exception> {
    let it = spanner::get_bound(host, me, d)?;
    Ok(it.filter(|&it| grob::has_interface(host.heap(), it, "note-head-interface")))
}

/// `get_column_rank`: the rank of the column of the tie's bound at `d`,
/// 0 for none (where LilyPond reads through a null).
pub fn get_column_rank(host: &mut dyn Host, me: Value, d: Direction) -> Result<u64, Exception> {
    let b = spanner::get_bound(host, me, d)?;
    let heap = host.heap();
    Ok(b.and_then(|b| item::get_column(heap, b))
        .map_or(0, |c| paper_column::rank(heap, c)))
}

/// `get_position`: the rounded staff position of the tie's left head,
/// else its right one; with neither, LilyPond's programming error, the
/// tie killed, 0.
pub fn get_position(host: &mut dyn Host, me: Value) -> Result<i32, Exception> {
    for d in [-1, 1] {
        if let Some(h) = head(host, me, d)? {
            return Ok(libm::rint(ssr::get_position(host, h, false)?) as i32);
        }
    }

    /*
      TODO: this is theoretically possible for ties across more than 2
      systems.. We should look at the first broken copy.

    */
    grob::unlocated_programming_error(host, "Tie without heads.  Suicide")?;
    grob::suicide(host.heap(), me);
    Ok(0)
}

/// `get_control_points`: the configuration's curve relative to `me`, a
/// list of its four control points; LilyPond's programming error for one
/// not finite.
pub(crate) fn get_control_points(
    host: &mut dyn Host,
    me: Value,
    common: Value,
    conf: &Configuration,
    details: &Details,
) -> Result<Value, Exception> {
    let mut b = conf.get_transformed_bezier(host, details)?;
    let x = grob::relative_coordinate(host, me, common, X)?;
    crate::bezier::translate(&mut b, (-x, 0.0));

    let mut controls = Value::NULL;
    for &c in b.iter().rev() {
        if !(c.0.is_finite() && c.1.is_finite()) {
            grob::unlocated_programming_error(host, "Insane offset")?;
        }
        let heap = host.heap();
        let point = crate::offset(heap, c);
        controls = heap.cons(point, controls);
    }
    Ok(controls)
}

/// Whether `g` is a tie column or a semi-tie column with `ties`.
fn in_column(host: &mut dyn Host, g: Option<Value>) -> Result<Option<Value>, Exception> {
    let Some(g) = g else {
        return Ok(None);
    };
    let heap = host.heap();
    if !(grob::has_interface(heap, g, "tie-column-interface")
        || grob::has_interface(heap, g, "semi-tie-column-interface"))
    {
        return Ok(None);
    }
    let sym = host.heap().intern("ties");
    let ties = grob::object(host, g, sym)?;
    Ok(host.heap().foreign::<GrobArray>(ties).map(|_| g))
}

/// `get_default_dir`: opposite the stems where a stem at an end (or at a
/// broken neighbour's) is visible, down for two up; else by the tie's
/// position; else `neutral-direction`.
pub fn get_default_dir(host: &mut dyn Host, me: Value) -> Result<Direction, Exception> {
    let mut stems = [None; 2];
    for (i, d) in [-1, 1].into_iter().enumerate() {
        let mut one_head = head(host, me, d)?;
        if one_head.is_none()
            && let Some(n) = spanner::broken_neighbor(host.heap(), me, d)
        {
            one_head = head(host, n, d)?;
        }

        let stem = match one_head {
            Some(h) => rhythmic_head::get_stem(host, h)?,
            None => None,
        };
        if let Some(s) = stem
            && !stem::is_invisible(host, s)?
        {
            stems[i] = Some(s);
        }
    }

    match stems {
        [Some(l), Some(r)] => {
            if get_grob_direction(host, l)? == 1 && get_grob_direction(host, r)? == 1 {
                return Ok(-1);
            }

            // And why not return UP if both stems are DOWN?

            // And when stems conflict, why fall directly through to using
            // neutral-direction without considering get_position (me)?
        }
        [Some(l), None] => return Ok(-get_grob_direction(host, l)?),
        [None, Some(r)] => return Ok(-get_grob_direction(host, r)?),
        [None, None] => {
            let p = get_position(host, me)?;
            if p != 0 {
                return Ok(p.signum());
            }
        }
    }

    let nd = grob::property(host, me, "neutral-direction")?;
    Ok(crate::dynamic_align_engraver::direction(nd))
}

/// `calc_direction`: the direction the tie's column gives it, its
/// positioning triggered; without one, LilyPond's programming error and
/// the tie killed.
pub fn calc_direction(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    // In this method, Tie and Semi_tie require the same logic with different
    // types.  It might be clearer to use a template.
    let yparent = grob::get_parent(host.heap(), me, Y);
    if let Some(yparent) = in_column(host, yparent)? {
        /* trigger positioning. */
        grob::property(host, yparent, "positioning-done")?;

        return Ok(property_data(host, me, "direction"));
    }

    grob::unlocated_programming_error(host, "no Tie_column or Semi_tie_column.  Killing grob.")?;
    grob::suicide(host.heap(), me);
    Ok(host.heap().integer(0))
}

/// `get_default_control_points`: the tie formatted alone.
pub fn get_default_control_points(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let mut problem = Problem::new();
    problem.read_tie(host, me)?;

    if !grob::is_live_grob(host.heap(), me) {
        return Ok(Value::NULL);
    }

    let conf = problem.generate_optimal_configuration(host)?;

    get_control_points(
        host,
        me,
        problem.common_x_refpoint(),
        &conf.ties[0],
        &problem.details,
    )
}

/// `calc_control_points`: a broken tie alone in its column given its
/// default direction; the column's positioning triggered; the control
/// points it set, else the tie's own.
pub fn calc_control_points(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let yparent = grob::get_parent(host.heap(), me, Y);
    if let Some(yparent) = in_column(host, yparent)? {
        let ties = grob_array::extract_grob_array(host, yparent, "ties")?;
        let d = property_data(host, me, "direction");
        if grob::original(host.heap(), me).is_some()
            && ties.len() == 1
            && crate::dynamic_align_engraver::direction(d) == 0
        {
            let dir = get_default_dir(host, me)?;
            set_grob_direction(host, me, dir);
        }
        /* trigger positioning. */
        grob::property(host, yparent, "positioning-done")?;
    }

    let cp = property_data(host, me, "control-points");
    if host.heap().car(cp).is_some() {
        return Ok(cp);
    }
    get_default_control_points(host, me)
}

/// `print`: the curve of `control-points` made `thickness` thick in its
/// middle and `line-thickness` at its ends, in staff line thicknesses,
/// dashed by `dash-definition`, on its direction's side; its
/// `annotation` (a string) in red beside it.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let mut cp = grob::property(host, me, "control-points")?;

    let staff_thick = line_thickness(host, me)?;
    let base_thick = staff_thick * real_or(host, me, "thickness")?;
    let line_thick = staff_thick * real_or(host, me, "line-thickness")?;

    let mut b: Bezier = [(0.0, 0.0); 4];
    for c in &mut b {
        let heap = host.heap();
        if let (Some(point), Some(rest)) = (heap.car(cp), heap.cdr(cp)) {
            let (Some(x), Some(y)) = (heap.car(point), heap.cdr(point)) else {
                return Err(grob::to_double(heap, point).expect_err("no pair is a number"));
            };
            *c = (grob::to_double(heap, x)?, grob::to_double(heap, y)?);
            cp = rest;
        }
    }

    let dash_definition = grob::property(host, me, "dash-definition")?;
    let dir = get_grob_direction(host, me)?;
    let mut errors = Vec::new();
    let mut a = lookup::slur(
        host.heap(),
        b,
        f64::from(dir) * base_thick,
        line_thick,
        dash_definition,
        &mut errors,
    );
    let depth = host.heap().root_depth();
    host.heap().root(a.expr);
    let printed = print_rest(host, me, &b, &mut a, &errors);
    host.heap().unroot_to(depth);
    printed?;
    Ok(host.heap().wrap(a))
}

/// What `print` does after the curve, `a` rooted: `Bezier::extract`'s
/// programming errors written, the annotation added.
fn print_rest(
    host: &mut dyn Host,
    me: Value,
    b: &Bezier,
    a: &mut Stencil,
    errors: &[String],
) -> Result<(), Exception> {
    for text in errors {
        grob::unlocated_programming_error(host, text)?;
    }

    let annotation = grob::property(host, me, "annotation")?;
    if host.heap().str(annotation).is_some() {
        let tm = crate::call_lily(host, "%grob-interpret-markup", &[me, annotation])?;
        if let Some(mut tm) = host.heap().foreign::<Stencil>(tm).copied() {
            tm.translate(host.heap(), (b[3].0 + 0.5, b[0].1 * 2.0));
            let heap = host.heap();
            let red = {
                let (r, g, bl) = (heap.flonum(1.0), heap.flonum(0.0), heap.flonum(0.0));
                heap.list(&[r, g, bl])
            };
            heap.root(red);
            let tm = tm.in_color(host, red)?;

            /*
              It would be nice if we could put this in a different layer,
              but alas, this must be done with a Tie override.
            */
            let heap = host.heap();
            a.add_stencil(heap, &tm);
            heap.root(a.expr);
        }
    }
    Ok(())
}

/// `from_scm<double> (get_property (me, name), 1)`.
fn real_or(host: &mut dyn Host, me: Value, name: &str) -> Result<f64, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(crate::spacing_options::double_or(host, v, 1.0))
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:tie::` primitives (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "tie",
    f: host_tie,
}];

/// The `ly:tie::` callbacks [`HOSTS`] runs, by name, which the callback
/// table (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 3] = [
    ("calc-control-points", calc_control_points),
    ("calc-direction", calc_direction),
    ("print", print),
];

fn host_tie(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such tie callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// A subr of `prefix` over `%host-call`: its grob checked under the
/// callback's name, as LilyPond's `LY_ASSERT_SMOB` gives it, and
/// `%host-call` tail-called with `what` and the callback's name.
pub fn host_call_of(cx: &mut Cx, what: &str, prefix: &str, name: &str, args: &[Value]) -> R {
    let method = format!("{prefix}{name}");
    grob::grob_arg(cx, &method, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern(what);
    let name = cx.heap_mut().intern(name);
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}

macro_rules! callbacks {
    ($($f:ident $what:literal $prefix:literal $name:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            host_call_of(cx, $what, $prefix, $name, args)
        })*
    };
}

callbacks! {
    ly_calc_control_points "tie" "ly:tie::" "calc-control-points",
    ly_calc_direction "tie" "ly:tie::" "calc-direction",
    ly_print "tie" "ly:tie::" "print",
    ly_column_before_line_breaking "tie-column" "ly:tie-column::" "before-line-breaking",
    ly_column_calc_positioning_done "tie-column" "ly:tie-column::" "calc-positioning-done",
    ly_semi_calc_control_points "semi-tie" "ly:semi-tie::" "calc-control-points",
}
