//! `clef.cc` (part 3's, MS3 step 40): the clef's callbacks
//! `ly:clef::calc-glyph-name` (`clef.cc:27–50`, 24 lines) and
//! `ly:clef::print` (`:52–68`, 17), the music crate's placeholders until
//! then (D67). Both are Rust under primitives that print as LilyPond's
//! subrs and call it over `%host-call` (D127, [`HOSTS`]), as the stem's
//! are.
//!
//! The glyph is the accidental's `Font_metric::find_by_name`
//! (`accidental.rs`) in the clef's default font (`font_select.rs`).

use alloc::format;
use alloc::string::String;

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::accidental::find_by_name;
use crate::callbacks::Callback;
use crate::font_select;
use crate::grob;
use crate::item;

const RIGHT: i32 = 1;

/// The string `v`, if it is one.
fn string_of(host: &mut dyn Host, v: Value) -> Option<String> {
    host.heap().str(v).map(|s| String::from(s.as_str()))
}

/// `calc_glyph_name`: the clef's `glyph`, with `_change` for a
/// `non-default` clef that does not begin a line and is not a
/// `full-size-change`; a clef whose `glyph` is no string is killed.
pub fn calc_glyph_name(host: &mut dyn Host, s: Value) -> Result<Value, Exception> {
    let glyph = grob::property(host, s, "glyph")?;
    let Some(mut str) = string_of(host, glyph) else {
        grob::suicide(host.heap(), s);
        return Ok(Value::UNSPECIFIED);
    };
    if grob::property(host, s, "non-default")? == Value::TRUE
        && item::break_status_dir(host.heap(), s) != RIGHT
        && grob::property(host, s, "full-size-change")? != Value::TRUE
    {
        str.push_str("_change");
    }
    Ok(host.heap().string(str))
}

/// `print`: the glyph of the clef's `glyph-name` in its default font,
/// LilyPond's warning if there is none; `()` if the name is no string.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let glyph_scm = grob::property(host, me, "glyph-name")?;
    let Some(glyph) = string_of(host, glyph_scm) else {
        return Ok(Value::NULL);
    };
    let depth = host.heap().root_depth();
    let printed = print_glyph(host, me, &glyph);
    host.heap().unroot_to(depth);
    printed
}

fn print_glyph(host: &mut dyn Host, me: Value, glyph: &str) -> Result<Value, Exception> {
    let fm = font_select::get_default_font(host, me)?;
    host.heap().root(fm);
    let out = find_by_name(host, fm, glyph)?;
    if out.is_empty() {
        grob::warning(host, me, &format!("clef `{glyph}' not found"))?;
    }
    Ok(host.heap().wrap(out))
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:clef::` primitives (D127): the
/// callback its second argument names, of the grob given, with the VM as
/// its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "clef",
    f: host_clef,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 2] =
    [("calc-glyph-name", calc_glyph_name), ("print", print)];

fn host_clef(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such clef callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:clef::… grob)`: LilyPond's subrs, primitives here so that they
/// print as LilyPond's. Each checks its grob and tail-calls `%host-call`
/// with its name ([`HOSTS`]).
fn callback(cx: &mut Cx, subr: &str, what: &str, grob: Value) -> R {
    grob::grob_arg(cx, subr, 1, grob)?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let clef = cx.heap_mut().intern("clef");
    let what = cx.heap_mut().intern(what);
    Ok(cx.tail_call(f, &[clef, what, grob]))
}

macro_rules! callbacks {
    ($($f:ident $what:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            callback(cx, concat!("ly:clef::", $what), $what, args[0])
        })*
    };
}

callbacks! {
    ly_calc_glyph_name "calc-glyph-name",
    ly_print "print",
}
