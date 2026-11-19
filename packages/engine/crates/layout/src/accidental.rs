//! `accidental.cc` (part 3's, MS3 step 39): the accidental's callbacks
//! `ly:accidental-interface::print` (`accidental.cc:119–149`, 31 lines)
//! with `parenthesize` (`:32–43`, 12), `::horizontal-skylines`
//! (`:45–84`, 40), `::height` (`:86–100`, 15) and `::remove-tied`
//! (`:102–117`, 16), the music crate's placeholders until then (D67). All
//! four are Rust under primitives that print as LilyPond's subrs and call
//! it over `%host-call` (D127, [`HOSTS`]), as the stem's are.
//!
//! The skylines are `skylines_from_stencil`'s (`%stencil-skylines`,
//! `skylines.rs`), of the stencil and its `rotation`, unpadded; the
//! height of an accidental not hidden is `Grob::stencil_height`'s
//! (`ly:grob::stencil-height`).

use alloc::format;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};
use lily_engine_skyline::{Axis as Horizon, Direction as Sky, Skyline, SkylinePair};

use crate::callbacks::Callback;
use crate::font_select;
use crate::geometry::{Direction, X, Y};
use crate::grob;
use crate::grob_array;
use crate::skylines;
use crate::stencil::Stencil;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// `from_scm<bool>`: only `#t` is true.
fn flag(host: &mut dyn Host, me: Value, name: &str) -> Result<bool, Exception> {
    Ok(grob::property(host, me, name)? == Value::TRUE)
}

/// `from_scm (get_property (me, "glyph-name"), "")`: the string, else `""`.
fn glyph_name(host: &mut dyn Host, me: Value) -> Result<alloc::string::String, Exception> {
    let v = grob::property(host, me, "glyph-name")?;
    Ok(host
        .heap()
        .str(v)
        .map(|s| alloc::string::String::from(s.as_str()))
        .unwrap_or_default())
}

/// `Font_metric::find_by_name`: the glyph `name` of the font `fm`, its
/// value rooted until the caller unroots it.
pub(crate) fn find_by_name(
    host: &mut dyn Host,
    fm: Value,
    name: &str,
) -> Result<Stencil, Exception> {
    let name = host.heap().string(name);
    let v = crate::call_lily(host, "ly:font-get-glyph", &[fm, name])?;
    host.heap().root(v);
    Ok(host
        .heap()
        .foreign::<Stencil>(v)
        .copied()
        .unwrap_or_default())
}

/// `parenthesize`: `m` between the default font's `accidentals.leftparen`
/// and `accidentals.rightparen`, touching it.
fn parenthesize(host: &mut dyn Host, me: Value, mut m: Stencil) -> Result<Stencil, Exception> {
    let font = font_select::get_default_font(host, me)?;
    host.heap().root(font);
    let open = find_by_name(host, font, "accidentals.leftparen")?;
    let close = find_by_name(host, font, "accidentals.rightparen")?;
    m.add_at_edge(host.heap(), X, LEFT, &open, 0.0);
    m.add_at_edge(host.heap(), X, RIGHT, &close, 0.0);
    Ok(m)
}

/// `print`: the glyph of the accidental's `glyph-name` in its default
/// font, LilyPond's warning if there is none; for `restore-first`, a
/// natural before it 0.1 apart (a warning if the font has none); and for
/// `parenthesized`, in parentheses.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let fm = font_select::get_default_font(host, me)?;
    host.heap().root(fm);
    let glyph_name = glyph_name(host, me)?;
    let mut st = find_by_name(host, fm, &glyph_name)?;
    if st.is_empty() {
        grob::warning(host, me, &format!("cannot find glyph {glyph_name}"))?;
    }
    if flag(host, me, "restore-first")? {
        // This isn't correct for ancient accidentals, but they don't use
        // double flats or sharps anyway.
        let acc = find_by_name(host, fm, "accidentals.natural")?;
        if acc.is_empty() {
            grob::warning(host, me, "natural alteration glyph not found")?;
        } else {
            st.add_at_edge(host.heap(), X, LEFT, &acc, 0.1);
        }
    }
    if flag(host, me, "parenthesized")? {
        st = parenthesize(host, me, st)?;
    }
    Ok(host.heap().wrap(st))
}

/// `horizontal_skylines`: the skylines along Y of the accidental's
/// stencil, rotated by its `rotation`; a flat's or a double flat's not in
/// parentheses with its right one raised to 0.375 of the glyph's right
/// edge over its height, which brings flats closer to double flats. None
/// for an accidental that is dead or has no stencil.
pub fn horizontal_skylines(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    if !grob::is_live_grob(host.heap(), me) {
        return Ok(skylines::skyline_pair_value(
            host.heap(),
            SkylinePair::empty(),
        ));
    }
    let stencil = grob::property(host, me, "stencil")?;
    let Some(my_stencil) = host.heap().foreign::<Stencil>(stencil).copied() else {
        return Ok(skylines::skyline_pair_value(
            host.heap(),
            SkylinePair::empty(),
        ));
    };
    let depth = host.heap().root_depth();
    host.heap().root(stencil);
    let sky = stencil_skylines(host, me, stencil);
    host.heap().unroot_to(depth);
    let mut sky = sky?;
    let parenthesized = flag(host, me, "parenthesized")?;
    let glyph_name = glyph_name(host, me)?;
    if (glyph_name == "accidentals.flat" || glyph_name == "accidentals.flatflat") && !parenthesized
    {
        // A bit more padding for the right of the stem: the stem raised
        // horizontally to a bit less than the average horizontal "height"
        // of the entire glyph.
        let left = my_stencil.extent(X).left;
        let right = my_stencil.extent(X).right * 0.375;
        let y = my_stencil.extent(Y);
        let b = lily_engine_fonts::Box {
            x: lily_engine_fonts::Interval::new(left, right),
            y: lily_engine_fonts::Interval::new(y.left, y.right),
        };
        let merge_with_me = Skyline::from_boxes(&[b], Horizon::Y, Sky::Up);
        sky.up.merge(&merge_with_me);
    }
    Ok(skylines::skyline_pair_value(host.heap(), sky))
}

/// `skylines_from_stencil (stencil, get_property (me, "rotation"),
/// Y_AXIS)`: `%stencil-skylines` of the stencil, its rotation a list of
/// three reals if `rotation` is a pair (each 0 where it is none), unpadded.
fn stencil_skylines(
    host: &mut dyn Host,
    me: Value,
    stencil: Value,
) -> Result<SkylinePair, Exception> {
    let rot = grob::property(host, me, "rotation")?;
    let heap = host.heap();
    let rotation = if heap.car(rot).is_some() {
        let nth = |heap: &mut lily_engine_scheme::Heap, i: usize| {
            let mut at = rot;
            for _ in 0..i {
                at = heap.cdr(at).unwrap_or(Value::NULL);
            }
            let x = heap.car(at).and_then(|v| heap.to_f64(v)).unwrap_or(0.0);
            heap.flonum(x)
        };
        let items = [nth(heap, 0), nth(heap, 1), nth(heap, 2)];
        heap.list(&items)
    } else {
        Value::FALSE
    };
    let axis = Value::fixnum(Y as i64).unwrap();
    let padding = heap.flonum(0.0);
    let v = crate::call_lily(
        host,
        "%stencil-skylines",
        &[stencil, rotation, axis, padding],
    )?;
    Ok(skylines::skyline_pair_in(host.heap(), v)
        .ok()
        .flatten()
        .unwrap_or_default())
}

/// Whether the accidental has a grob as its `tie` and is not `forced`.
fn tied_not_forced(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern("tie");
    let tie = grob_array::object(host, me, sym)?;
    if grob_of(host.heap(), tie).is_none() || flag(host, me, "forced")? {
        return Ok(None);
    }
    Ok(Some(tie))
}

/// `height`: empty for a tied accidental, not forced, whose
/// `hide-tied-accidental-after-break` is set; else its stencil's height.
pub fn height(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    if tied_not_forced(host, me)?.is_some() && flag(host, me, "hide-tied-accidental-after-break")? {
        let heap = host.heap();
        let (l, r) = (heap.flonum(f64::INFINITY), heap.flonum(f64::NEG_INFINITY));
        return Ok(heap.cons(l, r));
    }
    crate::call_lily(host, "ly:grob::stencil-height", &[me])
}

/// `remove_tied`: a tied accidental, not forced, killed if its
/// `hide-tied-accidental-after-break` is set or its tie is not broken
/// (has no original).
pub fn remove_tied(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    if let Some(tie) = tied_not_forced(host, me)? {
        let broken = grob::original(host.heap(), tie).is_some();
        if flag(host, me, "hide-tied-accidental-after-break")? || !broken {
            grob::suicide(host.heap(), me);
        }
    }
    Ok(Value::UNSPECIFIED)
}

// --- the callbacks ----------------------------------------------------------

/// What `%host-call` runs for the `ly:accidental-interface::` primitives
/// (D127): the callback its second argument names, of the grob given,
/// with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "accidental",
    f: host_accidental,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 4] = [
    ("height", height),
    ("horizontal-skylines", horizontal_skylines),
    ("print", print),
    ("remove-tied", remove_tied),
];

fn host_accidental(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such accidental callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:accidental-interface::… grob)`: LilyPond's subrs, primitives here
/// so that they print as LilyPond's. Each checks its grob and tail-calls
/// `%host-call` with its name ([`HOSTS`]).
fn callback(cx: &mut Cx, subr: &str, what: &str, grob: Value) -> R {
    grob::grob_arg(cx, subr, 1, grob)?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let accidental = cx.heap_mut().intern("accidental");
    let what = cx.heap_mut().intern(what);
    Ok(cx.tail_call(f, &[accidental, what, grob]))
}

macro_rules! callbacks {
    ($($f:ident $what:literal,)*) => {
        $(pub(crate) fn $f(cx: &mut Cx, args: &[Value]) -> R {
            callback(cx, concat!("ly:accidental-interface::", $what), $what, args[0])
        })*
    };
}

callbacks! {
    ly_height "height",
    ly_horizontal_skylines "horizontal-skylines",
    ly_print "print",
    ly_remove_tied "remove-tied",
}
