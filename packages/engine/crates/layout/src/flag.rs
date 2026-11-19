//! `flag.cc` (part 3's, MS3 step 41): the flag's callbacks
//! `ly:flag::width` (`flag.cc:49–67`, 19 lines), `::glyph-name`
//! (`:69–114`, 46), `::print` (`:116–165`, 50), `::pure-calc-y-offset`
//! and `::calc-y-offset` (`:167–180`, 14) over `internal_calc_y_offset`
//! (`:182–196`, 15), and `::calc-x-offset` (`:198–205`, 8), the music
//! crate's placeholders until then (D67). All six are Rust under
//! primitives that print as LilyPond's subrs and call it over
//! `%host-call` (D127, [`HOSTS`]), as the stem's are.
//!
//! The glyphs are the accidental's `Font_metric::find_by_name`
//! (`accidental.rs`) in the flag's default font (`font_select.rs`); the
//! stem is the flag's X parent, its direction `direction.rs`' and its
//! duration log `stem.rs`'. A flag without an X parent, which LilyPond
//! dereferences, throws `misc-error`.

use alloc::format;
use alloc::string::String;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Tag, Value, Vm};

use crate::accidental::find_by_name;
use crate::callbacks::Callback;
use crate::geometry::{Direction, Interval, X, Y};
use crate::stencil::Stencil;
use crate::{direction, font_select, grob, stem};

const RIGHT: Direction = 1;
const UP: Direction = 1;

/// `me->get_x_parent ()`: the flag's stem.
fn stem_of(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    grob::get_parent(host.heap(), me, X)
        .ok_or_else(|| crate::misc_error(host.heap(), "ly:flag", "flag without a stem"))
}

/// The flag's `style`'s name if it is a symbol, else `""`.
fn style(host: &mut dyn Host, me: Value) -> Result<String, Exception> {
    let v = grob::property(host, me, "style")?;
    let heap = host.heap();
    Ok(match v.tag() {
        Tag::Symbol => heap.name(v).map(String::from).unwrap_or_default(),
        _ => String::new(),
    })
}

/// `'u'` for a stem up, else `'d'`.
fn dir_char(d: Direction) -> char {
    if d == UP { 'u' } else { 'd' }
}

/// `width`: the stencil's X extent less the stem's right edge, `(0 . 0)`
/// without a stencil. (LilyPond's TODO: a bad hard-coding, the flags'
/// boxes slightly off.)
pub fn width(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let v = grob::property(host, me, "stencil")?;
    let Some(&sten) = host.heap().foreign::<Stencil>(v) else {
        return Ok(crate::offset(host.heap(), (0.0, 0.0)));
    };
    let stem = stem_of(host, me)?;
    let mut x = sten.extent(X);
    x.translate(-grob::extent(host, stem, stem, X)?.at(RIGHT));
    Ok(crate::offset(host.heap(), (x.left, x.right)))
}

/// `glyph_name`: `flags.` and the style, `u` or `d` by the stem's
/// direction, for `mensural` `0` where the stem's end is on a staff line
/// and `1` where it is not, and the stem's duration log.
pub fn glyph_name(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let stem = stem_of(host, me)?;
    let d = direction::get_grob_direction(host, stem)?;
    let log = stem::duration_log(host, stem)?;
    let flag_style = style(host, me)?;

    // Mensural notation: For notes on staff lines, use different flags
    // than for notes between staff lines. The idea is that flags are
    // always vertically aligned with the staff lines, regardless if the
    // note head is on a staff line or between two staff lines. In other
    // words, the inner end of a flag always touches a staff line.
    let staffline_offs = if flag_style == "mensural" {
        let ss = crate::staff_symbol_referencer::staff_space(host, me)?;
        let end = grob::extent(host, stem, stem, Y)?.at(d);
        let p = libm::rint(end * 2.0 / ss) as i32;
        let p = host.heap().integer(i64::from(p));
        if crate::call_lily(host, "%staff-on-line?", &[stem, p])? != Value::FALSE {
            "0"
        } else {
            "1"
        }
    } else {
        ""
    };
    let font_char = format!("{flag_style}{}{staffline_offs}{log}", dir_char(d));
    Ok(host.heap().string(format!("flags.{font_char}")))
}

/// `print`: nothing for the `no-flag` style; else the glyph of the
/// flag's `glyph-name` in its default font, LilyPond's warning if there
/// is none, with the stroke of a nonempty `stroke-style` string added,
/// the style's or else the plain one, or LilyPond's warning.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let stem = stem_of(host, me)?;
    let d = direction::get_grob_direction(host, stem)?;
    let flag_style = style(host, me)?;
    if flag_style == "no-flag" {
        return Ok(host.heap().wrap(Stencil::default()));
    }

    let dir = dir_char(d);
    let fm = font_select::get_default_font(host, me)?;
    host.heap().root(fm);
    let v = grob::property(host, me, "glyph-name")?;
    let font_char = host
        .heap()
        .str(v)
        .map(|s| String::from(s.as_str()))
        .unwrap_or_default();
    let mut flag = find_by_name(host, fm, &font_char)?;
    if flag.is_empty() {
        grob::warning(host, me, &format!("flag `{font_char}' not found"))?;
    }

    // TODO: maybe property stroke-style should take different values,
    // e.g. "" (i.e. no stroke), "single" and "double" (currently, it's
    // '() or "grace").
    let v = grob::property(host, me, "stroke-style")?;
    if let Some(stroke_style) = host.heap().str(v).map(|s| String::from(s.as_str()))
        && !stroke_style.is_empty()
    {
        let mut font_char = format!("{flag_style}{dir}{stroke_style}");
        let mut stroke = find_by_name(host, fm, &format!("flags.{font_char}"))?;
        if stroke.is_empty() {
            font_char = format!("{dir}{stroke_style}");
            stroke = find_by_name(host, fm, &format!("flags.{font_char}"))?;
        }
        if stroke.is_empty() {
            grob::warning(host, me, &format!("flag stroke `{font_char}' not found"))?;
        } else {
            flag.add_stencil(host.heap(), &stroke);
        }
    }
    Ok(host.heap().wrap(flag))
}

/// `pure_calc_y_offset`: [`internal_calc_y_offset`], pure.
pub fn pure_calc_y_offset(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    internal_calc_y_offset(host, me, true)
}

/// `calc_y_offset`: [`internal_calc_y_offset`].
pub fn calc_y_offset(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    internal_calc_y_offset(host, me, false)
}

/// `internal_calc_y_offset`: the stem's end its way, less half the
/// layout's `blot-diameter`; 0 for a stem without height. The pure one
/// takes the stem's pure height from column 0 to `INT_MAX`.
fn internal_calc_y_offset(host: &mut dyn Host, me: Value, pure: bool) -> Result<Value, Exception> {
    let stem = stem_of(host, me)?;
    let d = direction::get_grob_direction(host, stem)?;
    let blot = grob::layout_dimension(host, me, "blot-diameter")?;
    let stem_extent: Interval = if pure {
        crate::unpure::pure_y_extent(host, stem, stem, 0, i32::MAX as usize)?
    } else {
        grob::extent(host, stem, stem, Y)?
    };
    let y = if stem_extent.is_empty() {
        0.0
    } else {
        stem_extent.at(d) - f64::from(d) * blot / 2.0
    };
    Ok(host.heap().flonum(y))
}

/// `calc_x_offset`: the stem's right edge.
pub fn calc_x_offset(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let stem = stem_of(host, me)?;
    let x = grob::extent(host, stem, stem, X)?.at(RIGHT);
    Ok(host.heap().flonum(x))
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:flag::` primitives (D127): the
/// callback its second argument names, of the grob given, with the VM as
/// its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "flag",
    f: host_flag,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 6] = [
    ("calc-x-offset", calc_x_offset),
    ("calc-y-offset", calc_y_offset),
    ("glyph-name", glyph_name),
    ("print", print),
    ("pure-calc-y-offset", pure_calc_y_offset),
    ("width", width),
];

fn host_flag(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such flag callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:flag::… grob)`, the pure one `(… grob start end)`: LilyPond's
/// subrs, primitives here so that they print as LilyPond's. Each checks
/// its grob and tail-calls `%host-call` with its name ([`HOSTS`]).
fn callback(cx: &mut Cx, subr: &str, what: &str, grob: Value) -> R {
    grob::grob_arg(cx, subr, 1, grob)?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let flag = cx.heap_mut().intern("flag");
    let what = cx.heap_mut().intern(what);
    Ok(cx.tail_call(f, &[flag, what, grob]))
}

macro_rules! callbacks {
    ($($f:ident $what:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            callback(cx, concat!("ly:flag::", $what), $what, args[0])
        })*
    };
}

callbacks! {
    ly_calc_x_offset "calc-x-offset",
    ly_calc_y_offset "calc-y-offset",
    ly_glyph_name "glyph-name",
    ly_print "print",
    ly_pure_calc_y_offset "pure-calc-y-offset",
    ly_width "width",
}
