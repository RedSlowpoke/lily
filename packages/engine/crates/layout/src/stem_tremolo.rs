//! `stem-tremolo.cc` (MS4 step 5): the `StemTremolo`'s ten callbacks, the
//! music crate's placeholders until then (D67), Rust under primitives that
//! print as LilyPond's subrs and call it over `%host-call` (D127,
//! [`HOSTS`]): `ly:stem-tremolo::calc-cross-staff` (`:33–40`), `::calc-slope`
//! (`:42–71`), `::calc-width` (`:73–85`), `::calc-shape` (`:87–103`),
//! `::pure-height` (`:150–184`), `::width` (`:186–197`), `::calc-y-offset`
//! and `::pure-calc-y-offset` (`:224–241`) over `y_offset` (`:281–342`),
//! `::calc-direction` (`:243–279`) and `::print` (`:344–353`); with
//! `get_beam_translation` (`:105–116`), `raw_stencil` (`:118–148`),
//! `untranslated_stencil` (`:205–222`) and `vertical_length` (`:199–203`),
//! which `stem.rs` calls for a tremolo flag's length.
//!
//! LilyPond 2.26.0's clang fuses the offsets' steps past the beams and a
//! flag's translations (otool).

use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::direction;
use crate::geometry::{Direction, Interval, X, Y, fma};
use crate::grob;
use crate::grob_array;
use crate::lookup;
use crate::spacing_options::double_or;
use crate::staff_symbol_referencer::staff_space;
use crate::stem;
use crate::stencil::Stencil;
use crate::unpure;

type Run<T> = Result<T, Exception>;

const DOWN: Direction = -1;
const UP: Direction = 1;

/// `get_object (me, "stem")` if it is a grob.
fn get_stem(host: &mut dyn Host, me: Value) -> Run<Option<Value>> {
    let sym = host.heap().intern("stem");
    let g = grob_array::object(host, me, sym)?;
    Ok(grob_of(host.heap(), g).is_some().then_some(g))
}

/// The tremolo's stem; LilyPond reads through a null without one, where
/// this throws its name.
fn the_stem(host: &mut dyn Host, me: Value, name: &str) -> Run<Value> {
    match get_stem(host, me)? {
        Some(s) => Ok(s),
        None => Err(crate::misc_error(
            host.heap(),
            name,
            "a stem tremolo with no stem",
        )),
    }
}

fn interval_value(host: &mut dyn Host, iv: Interval) -> Value {
    let heap = host.heap();
    let (l, r) = (heap.flonum(iv.left), heap.flonum(iv.right));
    heap.cons(l, r)
}

/// `calc_cross_staff`: its stem's `cross-staff`.
pub fn calc_cross_staff(host: &mut dyn Host, me: Value) -> Run<Value> {
    let stem = the_stem(host, me, "Stem_tremolo::calc_cross_staff")?;
    grob::property(host, stem, "cross-staff")
}

/// `calc_slope`: a beamed stem's beam's, unless the `style` is
/// `constant`; else 0.40 for a down stem with flags, 0.25 otherwise.
pub fn calc_slope(host: &mut dyn Host, me: Value) -> Run<Value> {
    let stem = the_stem(host, me, "Stem_tremolo::calc_slope")?;
    let beam = stem::get_beam(host, stem)?;

    let style = grob::property(host, me, "style")?;
    let constant = host.heap().intern("constant");

    let slope = match beam {
        Some(beam) if style != constant => {
            let mut dy = 0.0;
            let s = grob::property(host, beam, "quantized-positions")?;
            if let Some(p) = grob::number_pair(host.heap(), s) {
                dy = -p.left + p.right;
            }

            let normal = grob_array::extract_grob_array(host, beam, "normal-stems")?;
            let (Some(&s1), Some(&s2)) = (normal.first(), normal.last()) else {
                return Err(crate::misc_error(
                    host.heap(),
                    "Stem_tremolo::calc_slope",
                    "a beam with no normal stems",
                ));
            };

            let common = grob::common_refpoint(host.heap(), s1, s2, X).unwrap_or(Value::NULL);
            let dx = grob::relative_coordinate(host, s2, common, X)?
                - grob::relative_coordinate(host, s1, common, X)?;

            if dx != 0.0 { dy / dx } else { 0.0 }
        }
        _ => {
            // down stems with flags should have more sloped trems (helps
            // avoid flag/stem collisions without making the stem very long)
            if stem::duration_log(host, stem)? >= 3
                && direction::get_grob_direction(host, me)? == DOWN
                && beam.is_none()
            {
                0.40
            } else {
                0.25
            }
        }
    };
    Ok(host.heap().flonum(slope))
}

/// `calc_width`: beamed stems and up-stems with flags have shorter
/// tremolos.
pub fn calc_width(host: &mut dyn Host, me: Value) -> Run<Value> {
    let stem = the_stem(host, me, "Stem_tremolo::calc_width")?;
    let dir = direction::get_grob_direction(host, me)?;
    let beam = stem::get_beam(host, stem)?.is_some();
    let flag = stem::duration_log(host, stem)? >= 3 && !beam;

    Ok(host.heap().flonum(if (dir == UP && flag) || beam {
        1.0
    } else {
        1.5
    }))
}

/// `calc_shape`: a rectangle where the width is short, unless the `style`
/// is `constant`; else beam-like.
pub fn calc_shape(host: &mut dyn Host, me: Value) -> Run<Value> {
    let stem = the_stem(host, me, "Stem_tremolo::calc_shape")?;
    let dir = direction::get_grob_direction(host, me)?;
    let beam = stem::get_beam(host, stem)?.is_some();
    let flag = stem::duration_log(host, stem)? >= 3 && !beam;
    let style = grob::property(host, me, "style")?;

    let heap = host.heap();
    let constant = heap.intern("constant");
    Ok(
        heap.intern(if style != constant && ((dir == UP && flag) || beam) {
            "rectangle"
        } else {
            "beam-like"
        }),
    )
}

/// `get_beam_translation`: its live beam's, else 0.81 staff spaces by its
/// `length-fraction`.
pub fn get_beam_translation(host: &mut dyn Host, me: Value) -> Run<f64> {
    let stem = the_stem(host, me, "Stem_tremolo::get_beam_translation")?;
    match stem::get_beam(host, stem)? {
        Some(beam) if grob::is_live_grob(host.heap(), beam) => {
            crate::beam::get_beam_translation(host, beam)
        }
        _ => {
            let ss = staff_space(host, me)?;
            let v = grob::property(host, me, "length-fraction")?;
            Ok(ss * double_or(host, v, 1.0) * 0.81)
        }
    }
}

/// `raw_stencil`: `flag-count` beams (or rectangles) of `slope`, stacked
/// against `dir`; with none, LilyPond's programming error and the tremolo
/// dies.
fn raw_stencil(host: &mut dyn Host, me: Value, slope: f64, dir: Direction) -> Run<Stencil> {
    let ss = staff_space(host, me)?;
    let v = grob::property(host, me, "beam-thickness")?;
    let mut thick = double_or(host, v, 1.0);
    let v = grob::property(host, me, "beam-width")?;
    let mut width = double_or(host, v, 1.0);
    let blot = grob::layout_dimension(host, me, "blot-diameter")?;
    let mut shape = grob::property(host, me, "shape")?;
    let heap = host.heap();
    if shape.tag() != lily_engine_scheme::Tag::Symbol {
        shape = heap.intern("beam-like");
    }

    width *= ss;
    thick *= ss;

    let rectangle = heap.intern("rectangle");
    let mut a = if shape == rectangle {
        lookup::rotated_box(heap, slope, width, thick, blot)
    } else {
        lookup::beam(heap, slope, width, thick, blot)
    };
    a.align_to(heap, X, 0.0);
    a.align_to(heap, Y, 0.0);
    // Aligned, it is a new expression: a callback below may collect.
    heap.root(a.expr);

    let v = grob::property(host, me, "flag-count")?;
    let tremolo_flags = crate::stem_engraver::int_or(host.heap(), v, 0);
    if tremolo_flags == 0 {
        grob::unlocated_programming_error(host, "no tremolo flags")?;

        grob::suicide(host.heap(), me);
        return Ok(Stencil::default());
    }

    let beam_translation = get_beam_translation(host, me)?;

    let heap = host.heap();
    let mut mol = Stencil::default();
    for i in 0..tremolo_flags {
        let mut b = a;
        b.translate_axis(heap, beam_translation * f64::from(i) * -f64::from(dir), Y);
        mol.add_stencil(heap, &b);
        heap.root(mol.expr);
    }
    Ok(mol)
}

/// `untranslated_stencil`: the raw stencil, stacked from the head for a
/// whole note; LilyPond's programming error and none without a stem.
fn untranslated_stencil(host: &mut dyn Host, me: Value, slope: f64) -> Run<Stencil> {
    let Some(stem) = get_stem(host, me)? else {
        grob::unlocated_programming_error(host, "no stem for stem-tremolo")?;
        return Ok(Stencil::default());
    };

    let dir = direction::get_grob_direction(host, me)?;

    let whole_note = stem::duration_log(host, stem)? <= 0;

    // for a whole note, we position relative to the notehead, so we want
    // the stencil aligned on the flag closest to the head
    let stencil_dir = if whole_note { -dir } else { dir };
    raw_stencil(host, me, slope, stencil_dir)
}

/// `vertical_length`: the height of its stencil at the slope 0.35.
pub fn vertical_length(host: &mut dyn Host, me: Value) -> Run<f64> {
    let depth = host.heap().root_depth();
    let s = untranslated_stencil(host, me, 0.35);
    host.heap().unroot_to(depth);
    Ok(s?.extent(Y).length())
}

/// `pure_height`: of its stencil at the slope 0.35 (the real slope looks
/// at the beam), or, beamed, its stem's pure extent from its shortest end
/// past its beams, centered.
pub fn pure_height(host: &mut dyn Host, me: Value) -> Run<Value> {
    let depth = host.heap().root_depth();
    let s1 = untranslated_stencil(host, me, 0.35);
    host.heap().unroot_to(depth);
    let s1 = s1?.extent(Y);
    let Some(stem) = get_stem(host, me)? else {
        return Ok(interval_value(host, s1));
    };

    let dir = direction::get_grob_direction(host, me)?;

    if stem::get_beam(host, stem)?.is_none() {
        return Ok(interval_value(host, s1));
    }

    let mut ph = unpure::pure_y_extent(host, stem, stem, 0, i32::MAX as usize)?;
    // This should not really happen but does
    if ph.is_empty() {
        return Ok(interval_value(host, s1));
    }

    let si = crate::beam_quanting::get_stem_info(host, stem)?;
    *ph.at_mut(-dir) = si.shortest_y;
    // This should not really happen either
    if ph.is_empty() {
        return Ok(interval_value(host, s1));
    }

    let beam_count = stem::beam_multiplicity(host, stem)?
        .length()
        .wrapping_add(1);
    let beam_translation = get_beam_translation(host, me)?;

    ph.translate(-(f64::from(dir.wrapping_mul(beam_count.max(1))) * beam_translation));
    // TODO: this nullifies the previous line?!?
    ph.translate(-ph.center());

    Ok(interval_value(host, ph))
}

/// `width`: of its stencil at the slope 0.35.
pub fn width(host: &mut dyn Host, me: Value) -> Run<Value> {
    let depth = host.heap().root_depth();
    let s1 = untranslated_stencil(host, me, 0.35);
    host.heap().unroot_to(depth);
    let x = s1?.extent(X);
    Ok(interval_value(host, x))
}

/// `y_offset`: past the stem's end and its beams (and flags), or by the
/// head for a whole note or an infinite end, with LilyPond's warning for
/// none; pure, a beamed stem's from its shortest end.
fn y_offset(host: &mut dyn Host, me: Value, pure: bool) -> Run<f64> {
    let Some(stem) = get_stem(host, me)? else {
        return Ok(0.0);
    };

    let dir = direction::get_grob_direction(host, me)?;

    let beam = stem::get_beam(host, stem)?;
    let beam_translation = get_beam_translation(host, me)?;

    let beam_count = if beam.is_some() {
        stem::beam_multiplicity(host, stem)?
            .length()
            .wrapping_add(1)
    } else {
        0
    };
    let steps = f64::from(dir.wrapping_mul(beam_count.max(1)));

    if pure && beam.is_some() {
        let mut ph = unpure::pure_y_extent(host, stem, stem, 0, i32::MAX as usize)?;
        let si = crate::beam_quanting::get_stem_info(host, stem)?;
        *ph.at_mut(-dir) = si.shortest_y;

        let at = ph.at(dir) - steps * beam_translation;
        let own = unpure::pure_y_extent(host, me, me, 0, i32::MAX as usize)?.length();
        return Ok(fma(-(f64::from(dir) * 0.5), own, at));
    }

    let stem_end = if pure {
        unpure::pure_y_extent(host, stem, stem, 0, i32::MAX as usize)?.at(dir)
    } else {
        grob::extent(host, stem, stem, Y)?.at(dir)
    };
    let mut end_y =
        fma(-steps, beam_translation, stem_end) - stem::beam_end_corrective(host, stem)?;

    let dlog = stem::duration_log(host, stem)?;
    if beam.is_none() && dlog >= 3 {
        end_y = fma(
            -f64::from(dir.wrapping_mul(dlog - 2)),
            beam_translation,
            end_y,
        );
        if dir == UP {
            end_y = fma(beam_translation, -0.5, end_y);
        }
    }

    let whole_note = dlog <= 0;
    if whole_note || end_y.is_infinite() {
        // we shouldn't position relative to the end of the stem since the
        // stem is invisible
        let ss = staff_space(host, me)?;
        let nhp = stem::note_head_positions(host, stem, false)?;
        match (nhp.first(), nhp.last()) {
            (Some(&first), Some(&last)) => {
                let note_head = f64::from(if dir == UP { last } else { first }) * ss / 2.0;
                end_y = fma(f64::from(dir), 1.5, note_head);
            }
            _ => {
                grob::warning(host, me, "stem tremolo has no note heads")?;
                end_y = 0.0;
            }
        }
    }

    Ok(end_y)
}

/// `calc_y_offset`.
pub fn calc_y_offset(host: &mut dyn Host, me: Value) -> Run<Value> {
    let y = y_offset(host, me, false)?;
    Ok(host.heap().flonum(y))
}

/// `pure_calc_y_offset`.
pub fn pure_calc_y_offset(host: &mut dyn Host, me: Value) -> Run<Value> {
    let y = y_offset(host, me, true)?;
    Ok(host.heap().flonum(y))
}

/// `calc_direction`: its stem's, turned for a whole note to avoid the
/// heads of a note collision, with LilyPond's warning where it cannot.
pub fn calc_direction(host: &mut dyn Host, me: Value) -> Run<Value> {
    let Some(stem) = get_stem(host, me)? else {
        return Ok(host.heap().integer(0));
    };

    let mut stemdir = direction::get_grob_direction(host, stem)?;

    let nhp = stem::note_head_positions(host, stem, false)?;
    // We re-decide stem-dir if there may be collisions with other note
    // heads in the staff.
    let maybe_nc =
        grob::get_parent(host.heap(), stem, X).and_then(|p| grob::get_parent(host.heap(), p, X));
    let whole_note = stem::duration_log(host, stem)? <= 0;
    if let Some(nc) = maybe_nc
        && whole_note
        && grob::has_interface(host.heap(), nc, "note-collision-interface")
    {
        let mut avoid_me = [false; 2];
        let mut all_nhps: Vec<i32> = Vec::new();
        for elt in grob_array::extract_grob_array(host, nc, "elements")? {
            if let Some(s) = get_stem(host, elt)? {
                all_nhps.extend(stem::note_head_positions(host, s, false)?);
            }
        }
        all_nhps.sort_unstable();
        let ix = |d: Direction| usize::from(d > 0);
        if let (Some(&a), Some(&b)) = (all_nhps.first(), nhp.first()) {
            avoid_me[0] = a < b;
        }
        if let (Some(&a), Some(&b)) = (all_nhps.last(), nhp.last()) {
            avoid_me[1] = a > b;
        }
        if stemdir != 0 && avoid_me[ix(stemdir)] {
            stemdir = -stemdir;
            if avoid_me[ix(stemdir)] {
                grob::warning(
                    host,
                    me,
                    "Whole-note tremolo may collide with simultaneous notes.",
                )?;
                stemdir = -stemdir;
            }
        }
    }
    Ok(host.heap().integer(i64::from(stemdir)))
}

/// `print`: its stencil at its `slope`.
pub fn print(host: &mut dyn Host, me: Value) -> Run<Value> {
    let v = grob::property(host, me, "slope")?;
    let slope = double_or(host, v, 0.25);
    let depth = host.heap().root_depth();
    let s = untranslated_stencil(host, me, slope);
    host.heap().unroot_to(depth);
    let s = s?;
    Ok(host.heap().wrap(s))
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:stem-tremolo::` primitives (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "stem-tremolo",
    f: host_stem_tremolo,
}];

/// The `ly:stem-tremolo::` callbacks [`HOSTS`] runs, by name, which the
/// callback table (`callbacks.rs`) finds too; the pure ones read no
/// column.
pub(crate) const CALLBACKS: [(&str, Callback); 10] = [
    ("calc-cross-staff", calc_cross_staff),
    ("calc-direction", calc_direction),
    ("calc-shape", calc_shape),
    ("calc-slope", calc_slope),
    ("calc-width", calc_width),
    ("calc-y-offset", calc_y_offset),
    ("print", print),
    ("pure-calc-y-offset", pure_calc_y_offset),
    ("pure-height", pure_height),
    ("width", width),
];

fn host_stem_tremolo(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such stem tremolo callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// A `ly:stem-tremolo::` subr: its grob checked under the callback's name
/// and `%host-call` tail-called with the callback's name ([`HOSTS`]); a
/// pure one's start and end are LilyPond's to ignore.
fn host_call(cx: &mut Cx, name: &str, args: &[Value]) -> R {
    let method = alloc::format!("ly:stem-tremolo::{name}");
    grob::grob_arg(cx, &method, 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("stem-tremolo");
    let name = cx.heap_mut().intern(name);
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}

macro_rules! callbacks {
    ($($f:ident $name:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            host_call(cx, $name, args)
        })*
    };
}

callbacks! {
    ly_calc_cross_staff "calc-cross-staff",
    ly_calc_direction "calc-direction",
    ly_calc_shape "calc-shape",
    ly_calc_slope "calc-slope",
    ly_calc_width "calc-width",
    ly_calc_y_offset "calc-y-offset",
    ly_print "print",
    ly_pure_calc_y_offset "pure-calc-y-offset",
    ly_pure_height "pure-height",
    ly_width "width",
}
