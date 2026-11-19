//! `balloon-engraver.cc`, `footnote-engraver.cc` and `balloon.cc` (MS8
//! step 8, part 2; 106, 91 and 193 lines): `Balloon_engraver` and
//! `Footnote_engraver`, classes of layout's ([`BALLOON_ENGRAVER`],
//! [`FOOTNOTE_ENGRAVER`]) that shadow the music crate's placeholders of
//! their names (D67, D108), and `ly:balloon-interface::print`, `::width`
//! and `::pure-height`, Rust under primitives that print as LilyPond's
//! subrs and call it over `%host-call` (D127, [`HOSTS`]).
//!
//! - `Balloon_engraver`: each grob acknowledged gets a sticky `BalloonText`
//!   (made by its own engraver, `make_sticky`) for each
//!   `annotate-output-event` among its cause's articulations, and for each
//!   such event heard this timestep whose `symbol` is its type.
//! - `Footnote_engraver`: a grob with `footnote-music` (a `footnote-event`,
//!   else LilyPond's programming error) gets a sticky `Footnote` caused by
//!   that music's event, and its `footnote-music` emptied.
//! - The balloon (`internal_balloon_print`): a frame about its host's
//!   extents widened by `padding` where `annotation-balloon`, its `text`
//!   aligned by `text-alignment-X`/`-Y` and placed at its offset from the
//!   host's attachment point (`X-attachment`, `Y-attachment`), a line from
//!   the attachment point to it where `annotation-line`. `width` and
//!   `pure-height` are its extents on one axis, the host's other extent 0.

use alloc::boxed::Box as Boxed;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Host, Kind, STOP, StreamEvent, TranslatorClass, Tx, Work, event_property, is_music_type,
    music_event,
};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::geometry::{Box, Interval, Offset, X, Y};
use crate::parenthesis_engraver::make_sticky;
use crate::spacing_options::double_or;
use crate::stencil::Stencil;
use crate::tie_engraver::{event_cause, in_event_class};
use crate::{grob, lookup};

type Run<T> = Result<T, Exception>;

pub static BALLOON_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Balloon_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    always: false,
    hears: &["annotate-output-event"],
    acknowledges: &[("grob-interface", false)],
    must_be_last: false,
    make: Some(|| Boxed::new(BalloonEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `events_`.
#[derive(Clone, Default)]
struct BalloonEngraver {
    events: Vec<Value>,
}

impl Work for BalloonEngraver {
    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.events.push(ev);
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.events.clear();
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob_id: u32) -> Run<()> {
        let g = tx.grob_handle(grob_id);
        let cause = event_cause(tx, g)?;
        let mut arts = match cause {
            Some(c) => event_property(tx.heap(), c, "articulations"),
            None => Value::NULL,
        };
        while let (Some(e), Some(rest)) = (tx.heap().car(arts), tx.heap().cdr(arts)) {
            if tx.heap().foreign::<StreamEvent>(e).is_some()
                && in_event_class(tx.heap(), e, "annotate-output-event")
            {
                make_sticky(tx, "BalloonText", grob_id, g, Cause::Event(e))?;
            }
            arts = rest;
        }
        let mut name = alloc::string::String::new();
        if let Some((arena, id)) = lily_engine_music::grob_of(tx.heap(), g) {
            let n = arena.borrow().records.grobs[id as usize].name;
            tx.heap().display(n, &mut name);
        }
        for ev in self.events.clone() {
            let symbol = event_property(tx.heap(), ev, "symbol");
            if tx.heap().name(symbol) == Some(name.as_str()) {
                make_sticky(tx, "BalloonText", grob_id, g, Cause::Event(ev))?;
            }
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(&self.events);
    }
}

pub static FOOTNOTE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Footnote_engraver",
    kind: Kind::Engraver,
    methods: 0,
    always: false,
    hears: &[],
    acknowledges: &[("grob-interface", false)],
    must_be_last: false,
    make: Some(|| Boxed::new(FootnoteEngraver)),
    reads: &[],
    writes: &[],
};

#[derive(Clone, Default)]
struct FootnoteEngraver;

impl Work for FootnoteEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob_id: u32) -> Run<()> {
        let g = tx.grob_handle(grob_id);
        let mus = grob::property(tx.host, g, "footnote-music")?;
        if tx.heap().foreign::<lily_engine_music::Music>(mus).is_none() {
            return Ok(());
        }
        if !is_music_type(tx.heap(), mus, "footnote-event") {
            let text = tx.heap().string("Must be footnote-event.");
            crate::call_lily(tx.host, "ly:programming-error", &[text])?;
            return Ok(());
        }
        // `Music::to_event`: an event of the music's class, referring to
        // it (a footnote has no length and no articulations).
        let heap = tx.host.heap();
        let sym = heap.intern("footnote-event");
        let class = {
            let mut a = tx.t.arena.borrow_mut();
            match a.class_number(heap, sym) {
                Some(n) => a.class_list(heap, n),
                None => heap.list(&[sym]),
            }
        };
        let key = heap.intern("music-cause");
        let entry = heap.cons(key, mus);
        let props = heap.list(&[entry]);
        let ev = music_event(heap, class, props, mus);
        let depth = heap.root_depth();
        heap.root(ev);
        let made = make_sticky(tx, "Footnote", grob_id, g, Cause::Event(ev));
        tx.heap().unroot_to(depth);
        made?;
        // This grob has exhausted its footnote
        grob::set_property(tx.heap(), g, "footnote-music", Value::NULL);
        Ok(())
    }
}

// --- the callbacks ------------------------------------------------------------

/// The balloon's `sticky-host`, or LilyPond's programming error.
fn annotated(host: &mut dyn Host, me: Value) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern("sticky-host");
    let a = grob::object(host, me, sym)?;
    if lily_engine_music::grob_of(host.heap(), a).is_some() {
        return Ok(Some(a));
    }
    grob::programming_error(host, me, "sticky grob without host")?;
    Ok(None)
}

/// `sign`.
fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// `internal_balloon_print`, its expression rooted.
fn internal_balloon_print(
    host: &mut dyn Host,
    me: Value,
    mut b: Box,
    off: Offset,
) -> Result<Stencil, Exception> {
    let padding = grob::property(host, me, "padding")?;
    let padding = double_or(host, padding, 0.1);
    b.widen(padding, padding);

    let mut result = Stencil::default();
    if grob::property(host, me, "annotation-balloon")? == Value::TRUE {
        let thickness = grob::property(host, me, "thickness")?;
        let mut thickness = double_or(host, thickness, 1.0);
        thickness *= crate::staff_symbol_referencer::line_thickness(host, me)?;
        let blot_diameter = 0.05; // FIXME: hardcoded
        let mut warnings = Vec::new();
        result = lookup::frame(host.heap(), b, thickness, blot_diameter, &mut warnings);
        host.heap().root(result.expr);
        for text in &warnings {
            grob::unlocated_warning(host, text)?;
        }
    }

    let bt = grob::property(host, me, "text")?;
    host.heap().root(bt);
    // TODO: cache somehow?
    let s = crate::call_lily(host, "%grob-interpret-markup", &[me, bt])?;
    host.heap().root(s);
    let mut text_stil = host
        .heap()
        .foreign::<Stencil>(s)
        .copied()
        .unwrap_or_default();
    let mut z1 = (0.0, 0.0);
    for a in [X, Y] {
        let o = if a == X { off.0 } else { off.1 };
        let off_sign = sign(o);
        let prop = if a == X {
            "text-alignment-X"
        } else {
            "text-alignment-Y"
        };
        let v = grob::property(host, me, prop)?;
        let text_align = double_or(host, v, -off_sign);
        let prop = if a == X {
            "X-attachment"
        } else {
            "Y-attachment"
        };
        let v = grob::property(host, me, prop)?;
        let attach_align = double_or(host, v, off_sign);
        let z = b.0[a].linear_combination(attach_align);
        if a == X {
            z1.0 = z;
        } else {
            z1.1 = z;
        }
        text_stil.align_to(host.heap(), a, text_align);
        host.heap().root(text_stil.expr);
    }
    let z2 = (z1.0 + off.0, z1.1 + off.1);
    if grob::property(host, me, "annotation-line")? == Value::TRUE {
        let l = crate::line::grob_line(host, me, z1, z2)?;
        result.add_stencil(host.heap(), &l);
        host.heap().root(result.expr);
    }
    let heap = host.heap();
    text_stil.translate(heap, z2);
    heap.root(text_stil.expr);
    result.add_stencil(heap, &text_stil);
    heap.root(result.expr);
    result.translate(heap, (-off.0, -off.1));
    heap.root(result.expr);
    Ok(result)
}

fn interval_value(host: &mut dyn Host, iv: Interval) -> Value {
    let heap = host.heap();
    let (l, r) = (heap.flonum(iv.left), heap.flonum(iv.right));
    heap.cons(l, r)
}

/// `Balloon_interface::print`, as above.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = (|| {
        let Some(a) = annotated(host, me)? else {
            return Ok(host.heap().wrap(Stencil::default()));
        };
        let off = (
            grob::relative_coordinate(host, me, a, X)?,
            grob::relative_coordinate(host, me, a, Y)?,
        );
        let b = Box::new(
            grob::robust_relative_extent(host, a, a, X)?,
            grob::robust_relative_extent(host, a, a, Y)?,
        );
        let s = internal_balloon_print(host, me, b, off)?;
        Ok(host.heap().wrap(s))
    })();
    host.heap().unroot_to(depth);
    printed
}

/// `Balloon_interface::width`, as above.
pub fn width(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let w = (|| {
        let Some(a) = annotated(host, me)? else {
            return Ok(interval_value(host, Interval::EMPTY));
        };
        let b = Box::new(
            grob::robust_relative_extent(host, a, a, X)?,
            Interval::new(0.0, 0.0),
        );
        let off = grob::relative_coordinate(host, me, a, X)?;
        let s = internal_balloon_print(host, me, b, (off, 0.0))?;
        Ok(interval_value(host, s.extent(X)))
    })();
    host.heap().unroot_to(depth);
    w
}

/// `Balloon_interface::pure_height`, as above.
pub fn pure_height(
    host: &mut dyn Host,
    me: Value,
    start: usize,
    end: usize,
) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let h = (|| {
        let Some(a) = annotated(host, me)? else {
            return Ok(interval_value(host, Interval::EMPTY));
        };
        let mut y = crate::unpure::pure_y_extent(host, a, a, start, end)?;
        if y.is_empty() {
            y.add_point(crate::unpure::pure_relative_y_coordinate(
                host, a, a, start, end,
            )?);
        }
        let off = grob::relative_coordinate(host, me, a, Y)?;
        let s = internal_balloon_print(host, me, Box::new(Interval::new(0.0, 0.0), y), (0.0, off))?;
        Ok(interval_value(host, s.extent(Y)))
    })();
    host.heap().unroot_to(depth);
    h
}

/// What `%host-call` runs for the `ly:balloon-interface::` callbacks
/// (D127): the pure height with its columns.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "balloon-interface",
    f: host_balloon,
}];

/// The `ly:balloon-interface::` callbacks of the grob alone, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 2] = [("print", print), ("width", width)];

fn host_balloon(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    if vm.heap().name(args[1]) == Some("pure-height") {
        let heap = vm.heap();
        let (start, end) = (
            crate::slur::column(heap, args[3]),
            crate::slur::column(heap, args[4]),
        );
        return pure_height(vm, args[2], start, end);
    }
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such balloon-interface callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "balloon-interface",
        "ly:balloon-interface::",
        "print",
        args,
    )
}

pub(crate) fn ly_width(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "balloon-interface",
        "ly:balloon-interface::",
        "width",
        args,
    )
}

/// `(ly:balloon-interface::pure-height grob start end)`.
pub(crate) fn ly_pure_height(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:balloon-interface::pure-height";
    grob::grob_arg(cx, SUBR, 1, args[0])?;
    let start = crate::slur::column_arg(cx, SUBR, 2, args[1])?;
    let end = crate::slur::column_arg(cx, SUBR, 3, args[2])?;
    let what = cx.heap_mut().intern("balloon-interface");
    let name = cx.heap_mut().intern("pure-height");
    crate::slur::tail_host_call(cx, &[what, name, args[0], start, end])
}
