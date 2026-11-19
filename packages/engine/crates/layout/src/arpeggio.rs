//! `arpeggio.cc` (part 4's, MS3 step 54, 378 lines): the callbacks of the
//! `Arpeggio`, `ChordBracket` and `ChordSlur` grobs, the music crate's
//! placeholders until then (D67): `ly:arpeggio::calc-cross-staff`
//! (`:58–79`), `::calc-positions` (`:81–113`), `::print` (`:115–184`),
//! `::width` (`:303–313`) and `::pure-height` (`:315–324`);
//! `ly:chord-bracket::print` (`:186–211`) and `::width` (`:213–223`);
//! `ly:chord-slur::print` (`:225–268`) and `::width` (`:270–301`), whose
//! curve is `bezier-bow.cc`'s `slur_shape` (`bezier.rs`). Rust under
//! primitives that print as LilyPond's subrs and call it over `%host-call`
//! (D127, [`HOSTS`]); all but the pure height in 05 §5.1's callback table
//! (`callbacks.rs`) too.
//!
//! Each checks its grob under the method's name, as LilyPond's
//! `LY_ASSERT_SMOB` gives it (`ly:print`, `ly:width`, …). A stem of no
//! staff symbol leaves the common refpoint none, as LilyPond's
//! `common_refpoint` of a null does, and the arpeggio's coordinate is then
//! read up to its root.

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::{Direction, Interval, X, Y};
use crate::grob;
use crate::grob_array::extract_grob_array;
use crate::lookup;
use crate::spacing_options::double_or;
use crate::staff_symbol_referencer as ssr;
use crate::stencil::Stencil;
use crate::{accidental, bezier, font_select, stem};

const LEFT: Direction = -1;
const DOWN: Direction = -1;
const UP: Direction = 1;
const CENTER: Direction = 0;

/// `from_scm<double> (get_property (me, name), fallback)`.
fn real_or(host: &mut dyn Host, me: Value, name: &str, fallback: f64) -> Result<f64, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(double_or(host, v, fallback))
}

/// `from_scm (get_property (me, name), Interval ())`: a number pair, else
/// the empty interval.
fn interval_property(host: &mut dyn Host, me: Value, name: &str) -> Result<Interval, Exception> {
    let v = grob::property(host, me, name)?;
    Ok(grob::number_pair(host.heap(), v).unwrap_or(Interval::EMPTY))
}

/// `from_scm (get_property (me, "direction"), LEFT)`: an exact -1, 0 or
/// 1, else left.
fn side(host: &mut dyn Host, me: Value) -> Result<Direction, Exception> {
    let v = grob::property(host, me, "direction")?;
    Ok(match v.as_fixnum() {
        Some(d @ -1..=1) => d as Direction,
        _ => LEFT,
    })
}

/// `to_scm (Interval)`.
fn interval_value(host: &mut dyn Host, iv: Interval) -> Value {
    crate::offset(host.heap(), (iv.left, iv.right))
}

/// `get_squiggle`: the default font's `scripts.arpeggio`, rooted until the
/// caller unroots.
fn get_squiggle(host: &mut dyn Host, me: Value) -> Result<Stencil, Exception> {
    let fm = font_select::get_default_font(host, me)?;
    host.heap().root(fm);
    accidental::find_by_name(host, fm, "scripts.arpeggio")
}

/// `get_common_y`: the arpeggio and its stems' staff symbols' common
/// refpoint on Y.
fn get_common_y(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let mut common = Some(me);

    let stems = extract_grob_array(host, me, "stems")?;
    for stem in stems {
        let ss = ssr::staff_symbol(host, stem)?;
        common = match (common, ss) {
            (Some(c), Some(ss)) => grob::common_refpoint(host.heap(), c, ss, Y),
            _ => None,
        };
    }

    Ok(common)
}

/// `calc_cross_staff`: whether the stems are in more than one vertical
/// axis group.
pub fn calc_cross_staff(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let stems = extract_grob_array(host, me, "stems")?;
    let mut vag = None;

    for (i, &stem) in stems.iter().enumerate() {
        let g = grob::vertical_axis_group(host.heap(), stem);
        if i == 0 {
            vag = g;
        } else if vag != g {
            return Ok(Value::TRUE);
        }
    }

    Ok(Value::FALSE)
}

/// `calc_positions`: the span of the stems' heads, in staff spaces from
/// the arpeggio.
pub fn calc_positions(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let common = get_common_y(host, me)?;
    let refp = common.unwrap_or(Value::NULL);

    /*
      TODO:

      Using stems here is not very convenient; should store noteheads
      instead, and also put them into the support. Now we will mess up
      in vicinity of a collision.
    */
    let mut heads = Interval::EMPTY;
    let my_y = grob::relative_coordinate(host, me, refp, Y)?;

    let stems = extract_grob_array(host, me, "stems")?;
    for stem in stems {
        let ss = ssr::staff_symbol(host, stem)?;
        let mut iv = stem::head_positions(host, stem)?;
        iv.scale(ssr::staff_space(host, me)? / 2.0);
        let staff_y = match ss {
            Some(ss) => grob::relative_coordinate(host, ss, refp, Y)?,
            None => 0.0,
        };
        iv.translate(staff_y);
        iv.translate(-my_y);
        heads.unite(iv);
    }

    heads.scale(1.0 / ssr::staff_space(host, me)?);

    Ok(interval_value(host, heads))
}

/// `print`: squiggles stacked to the height of `positions`, at least two,
/// with an arrow at the end `arpeggio-direction` gives; a transparent one
/// of no heads (part of a span arpeggio) one squiggle high. Of no heads
/// and not transparent, LilyPond's warning, and the arpeggio dies.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let ss = ssr::staff_space(host, me)?;
    let mut heads = interval_property(host, me, "positions")?;
    heads.scale(ss);

    if heads.is_empty() {
        if grob::property(host, me, "transparent")? == Value::TRUE {
            /*
              This is part of a cross-staff/-voice span-arpeggio,
              so we need to ensure `heads' is large enough to encompass
              a single trill-element since the span-arpeggio depends on
              its children to prevent collisions.
            */
            heads.unite(get_squiggle(host, me)?.extent(Y));
        } else {
            grob::warning(host, me, "no heads for arpeggio found?")?;
            grob::suicide(host.heap(), me);
            return Ok(Value::NULL);
        }
    }

    // Adjust lower position to include note head in interval.
    *heads.at_mut(DOWN) -= 0.5;

    // Make sure that we have at least two wiggles (or a wiggle plus an arrow
    // head)
    if heads.length() < 1.5 * ss {
        heads.widen(0.5 * ss);
    }

    let ad = grob::property(host, me, "arpeggio-direction")?;
    let dir = match ad.as_fixnum() {
        Some(d @ -1..=1) => d as Direction,
        _ => CENTER,
    };

    let mut mol = Stencil::default();
    let squiggle = get_squiggle(host, me)?;

    /*
      Compensate for rounding error which may occur when a chord
      reaches the center line, resulting in an extra squiggle
      being added to the arpeggio stencil.  This value is appreciably
      larger than the rounding error, which is in the region of 1e-16
      for a global-staff-size of 20, but small enough that it does not
      interfere with smaller staff sizes.
    */
    let epsilon = 1e-3;

    let mut arrow = Stencil::default();
    if dir != CENTER {
        let fm = font_select::get_default_font(host, me)?;
        host.heap().root(fm);
        arrow =
            accidental::find_by_name(host, fm, &alloc::format!("scripts.arpeggio.arrow.{dir}"))?;
        *heads.at_mut(dir) -= f64::from(dir) * arrow.extent(Y).length();
    }

    let heap = host.heap();
    while mol.extent(Y).length() + epsilon < heads.length() {
        mol.add_at_edge(heap, Y, UP, &squiggle, 0.0);
    }

    mol.translate_axis(heap, heads.left, Y);
    if dir != CENTER {
        mol.add_at_edge(heap, Y, dir, &arrow, 0.0);
    }

    Ok(heap.wrap(mol))
}

/// `width`: the squiggle's, as `print` would trigger a vertical alignment
/// if it is cross-staff.
pub fn width(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let squiggle = get_squiggle(host, me);
    host.heap().unroot_to(depth);
    let x = squiggle?.extent(X);
    Ok(interval_value(host, x))
}

/// `pure_height`: none if cross-staff, else the stencil's height.
pub fn pure_height(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    if grob::property(host, me, "cross-staff")? == Value::TRUE {
        return Ok(interval_value(host, Interval::EMPTY));
    }

    grob::stencil_height(host, me)
}

// --- the chord bracket (arpeggio.cc:186–223) --------------------------------

/// `Chord_bracket::print (me, y_extent)`: a bracket along `y_extent`, its
/// wings `protrusion` long towards the side opposite its `direction`.
fn bracket_stencil(
    host: &mut dyn Host,
    me: Value,
    y_extent: Interval,
) -> Result<Stencil, Exception> {
    let thickness =
        grob::layout_dimension(host, me, "line-thickness")? * real_or(host, me, "thickness", 1.0)?;
    let side = side(host, me)?;
    let width = real_or(host, me, "protrusion", 0.4)?;
    let mut warnings = alloc::vec::Vec::new();
    let s = lookup::bracket(
        host.heap(),
        Y,
        y_extent,
        thickness,
        width * -f64::from(side),
        thickness,
        &mut warnings,
    );
    let depth = host.heap().root_depth();
    host.heap().root(s.expr);
    let warned = warnings
        .iter()
        .try_for_each(|text| grob::unlocated_warning(host, text));
    host.heap().unroot_to(depth);
    warned.map(|()| s)
}

/// `ly:chord-bracket::print`: the bracket along `positions` widened by
/// 0.75, in staff spaces.
pub fn bracket_print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let mut y_extent = interval_property(host, me, "positions")?;
    y_extent.widen(0.75); // candidate for a grob property
    y_extent.scale(ssr::staff_space(host, me)?);
    let s = bracket_stencil(host, me, y_extent)?;
    Ok(host.heap().wrap(s))
}

/// `ly:chord-bracket::width`: the width of a bracket of a dummy height,
/// which avoids triggering vertical alignment before line breaking.
pub fn bracket_width(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let y_extent = Interval::new(0.0, 1.0);
    let x_extent = bracket_stencil(host, me, y_extent)?.extent(X);
    Ok(interval_value(host, x_extent))
}

// --- the chord slur (arpeggio.cc:225–301) -----------------------------------

/// `Chord_slur::print (me, positions)`: a vertical slur along `positions`
/// in staff spaces, bowed away from its `direction`.
fn slur_stencil(host: &mut dyn Host, me: Value, positions: Interval) -> Result<Stencil, Exception> {
    let dash_definition = grob::property(host, me, "dash-definition")?;
    let depth = host.heap().root_depth();
    host.heap().root(dash_definition);
    let s = slur_stencil_rooted(host, me, positions, dash_definition);
    host.heap().unroot_to(depth);
    s
}

fn slur_stencil_rooted(
    host: &mut dyn Host,
    me: Value,
    positions: Interval,
    dash_definition: Value,
) -> Result<Stencil, Exception> {
    let ss = ssr::staff_space(host, me)?;
    let mut heads = positions;
    heads.scale(ssr::staff_space(host, me)?);

    let lt = grob::layout_dimension(host, me, "line-thickness")?
        * real_or(host, me, "line-thickness", 1.0)?;
    let th =
        grob::layout_dimension(host, me, "line-thickness")? * real_or(host, me, "thickness", 1.0)?;
    let side = side(host, me)?;

    // Adjust lower position to include note head in interval.
    *heads.at_mut(DOWN) -= 0.5;

    // Avoid too short chord slurs for small intervals.
    if heads.length() < 1.5 * ss {
        heads.widen(0.5 * ss);
    } else if heads.length() < 2.0 * ss {
        heads.widen(0.25 * ss);
    }

    let sp = 0.5 * ss;
    let dy = heads.length() - sp;

    let height_limit = 1.5;
    let ratio = 0.33;
    let mut curve = bezier::slur_shape(dy, height_limit, ratio * -f64::from(side));
    bezier::rotate(&mut curve, 90.0);

    let mut errors = alloc::vec::Vec::new();
    let mut mol = lookup::slur(host.heap(), curve, th, lt, dash_definition, &mut errors);
    mol.translate_axis(host.heap(), heads.left + 1.5 * sp / 2.0, Y);
    let depth = host.heap().root_depth();
    host.heap().root(mol.expr);
    let reported = errors
        .iter()
        .try_for_each(|text| grob::unlocated_programming_error(host, text));
    host.heap().unroot_to(depth);
    reported.map(|()| mol)
}

/// `ly:chord-slur::print`: the slur along `positions`.
pub fn slur_print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let positions = interval_property(host, me, "positions")?;
    let s = slur_stencil(host, me, positions)?;
    Ok(host.heap().wrap(s))
}

/// `ly:chord-slur::width`: the stencil's width, unless the slur or the
/// surrogate that appears in its place is cross-staff; then the width of a
/// slur of a dummy height, which avoids triggering vertical alignment
/// before line breaking.
pub fn slur_width(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    // A surrogate slur appears instead of this one, but does not control
    // horizontal spacing itself.
    let sym = host.heap().intern("vertically-spanning-surrogate");
    let surrogate = grob::object(host, me, sym)?;
    let surrogate = grob_of(host.heap(), surrogate).map(|_| surrogate);

    // Get the width from this grob's stencil if we're not in a cross-staff
    // situation.  Making a cross-staff stencil here would trigger vertical
    // alignment before line breaking.
    if surrogate.is_none() && grob::property(host, me, "cross-staff")? != Value::TRUE {
        return grob::stencil_width(host, me);
    }

    // If a surrogate slur isn't cross-staff (it might be just cross-voice), then
    // it should have no trouble calculating its actual width.
    if let Some(surrogate) = surrogate
        && grob::property(host, surrogate, "cross-staff")? != Value::TRUE
    {
        return grob::stencil_width(host, surrogate);
    }

    // We're in a cross-staff situation.  If this slur is cross-staff, we can't
    // make its stencil to get its actual width, and if this slur is not
    // cross-staff, its extent is probably not a good estimate of the extent of
    // the surrogate.  Using a dummy height avoids triggering vertical alignment
    // before line breaking.  We use a large value to aim for the worst case,
    // expecting the stencil code to limit the curvature of the slur.
    let positions = Interval::new(0.0, 100.0);
    let x_extent = slur_stencil(host, me, positions)?.extent(X);
    Ok(interval_value(host, x_extent))
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:arpeggio::`, `ly:chord-bracket::`
/// and `ly:chord-slur::` primitives (D127): the callback its second
/// argument names, of the grob given, with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 3] = [
    HostHandler {
        what: "arpeggio",
        f: host_arpeggio,
    },
    HostHandler {
        what: "chord-bracket",
        f: host_chord_bracket,
    },
    HostHandler {
        what: "chord-slur",
        f: host_chord_slur,
    },
];

/// The `ly:arpeggio::` callbacks [`HOSTS`] runs, by name, which the
/// callback table (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 5] = [
    ("calc-cross-staff", calc_cross_staff),
    ("calc-positions", calc_positions),
    ("print", print),
    ("pure-height", pure_height),
    ("width", width),
];

/// The `ly:chord-bracket::` callbacks.
pub(crate) const BRACKET_CALLBACKS: [(&str, Callback); 2] =
    [("print", bracket_print), ("width", bracket_width)];

/// The `ly:chord-slur::` callbacks.
pub(crate) const SLUR_CALLBACKS: [(&str, Callback); 2] =
    [("print", slur_print), ("width", slur_width)];

fn host_arpeggio(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such arpeggio callback")
}

fn host_chord_bracket(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(
        vm,
        &BRACKET_CALLBACKS,
        args,
        "no such chord bracket callback",
    )
}

fn host_chord_slur(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &SLUR_CALLBACKS, args, "no such chord slur callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// A subr of `module` (`arpeggio`, `chord-bracket` or `chord-slur`): its
/// grob checked under the method's name, as LilyPond's `LY_ASSERT_SMOB`
/// gives it, and `%host-call` tail-called with the callback's name
/// ([`HOSTS`]). The pure height's start and end are LilyPond's to ignore.
fn host_call(cx: &mut Cx, module: &str, name: &str, args: &[Value]) -> R {
    let method = alloc::format!("ly:{name}");
    grob::grob_arg(cx, &method, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern(module);
    let name = cx.heap_mut().intern(name);
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}

macro_rules! callbacks {
    ($($f:ident $module:literal $name:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            host_call(cx, $module, $name, args)
        })*
    };
}

callbacks! {
    ly_calc_cross_staff "arpeggio" "calc-cross-staff",
    ly_calc_positions "arpeggio" "calc-positions",
    ly_print "arpeggio" "print",
    ly_pure_height "arpeggio" "pure-height",
    ly_width "arpeggio" "width",
    ly_bracket_print "chord-bracket" "print",
    ly_bracket_width "chord-bracket" "width",
    ly_slur_print "chord-slur" "print",
    ly_slur_width "chord-slur" "width",
}
