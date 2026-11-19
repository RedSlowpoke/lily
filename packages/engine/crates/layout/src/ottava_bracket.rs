//! `ottava-bracket.cc` (part 4's, MS3 step 49, 198 lines): the ottava
//! bracket's print, `ly:ottava-bracket::print` (`:46–184`), the music
//! crate's placeholder until then (D67, OPEN A17). Rust under a primitive
//! that prints as LilyPond's subr and calls it over `%host-call` (D127,
//! [`HOSTS`]), in 05 §5.1's callback table (`callbacks.rs`) too.
//!
//! The bracket is `bracket.rs`' [`make_bracket`], the text its `text`
//! interpreted on it where it is a markup (`%grob-interpret-markup`,
//! `text.rs`), a broken bound's extent `axis_group.rs`'
//! `%generic-bound-extent`. The print checks its spanner under its full
//! name, where LilyPond's dereferences what is none; a bracket without a
//! bound throws `misc-error`.

use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::bracket::{drul_property, make_bracket};
use crate::callbacks::Callback;
use crate::direction::get_grob_direction;
use crate::geometry::{Box, Direction, Interval, X, Y};
use crate::spacing_options::double_or;
use crate::stencil::Stencil;
use crate::{grob, grob_array, item, rhythmic_head, spanner, text};

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// A `Drul_array`'s index of `d`: 0 for `LEFT`, 1 for `RIGHT`.
fn side(d: Direction) -> usize {
    usize::from(d == RIGHT)
}

/*
  TODO: the string for ottava shoudl depend on the available space, ie.

  Long: 15ma        Short: 15ma    Empty: 15
  8va                8va            8
  8va bassa          8ba            8
*/

/// `print`: the `text` from the left bound's note heads or edge, then the
/// bracket from after the text to the right one's (their dots' at the
/// right), each end shortened by `shorten-pair` unless broken, where it
/// goes to the bound's end and has no edge; the text at least
/// `minimum-length` from the right end, the bracket only if longer than
/// 0.001, its lines taking 0.1 of height.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let mut span_points = Interval::EMPTY;

    let mut bounds = [Value::NULL; 2];
    for d in [LEFT, RIGHT] {
        bounds[side(d)] = spanner::get_bound(host, me, d)?.ok_or_else(|| {
            crate::misc_error(
                host.heap(),
                "ly:ottava-bracket::print",
                "ottava bracket without a bound",
            )
        })?;
    }
    let mut common = grob::common_refpoint(host.heap(), bounds[0], bounds[1], X);

    let mut broken = [false; 2];
    for d in [LEFT, RIGHT] {
        let b = bounds[side(d)];
        broken[side(d)] = item::break_status_dir(host.heap(), b) != 0;

        if grob::has_interface(host.heap(), b, "note-column-interface") {
            let heads = grob_array::extract_grob_array(host, b, "note-heads")?;
            common = grob::common_refpoint_of_array(host.heap(), &heads, common, X);
            for h in heads {
                if let Some(dots) = rhythmic_head::get_dots(host, h)? {
                    common = common.and_then(|c| grob::common_refpoint(host.heap(), dots, c, X));
                }
            }
        }
    }
    let common = common.unwrap_or(Value::NULL);

    let markup = grob::property(host, me, "text")?;
    let mut text = Stencil::default();
    if text::is_markup(host, markup)? {
        let v = crate::call_lily(host, "%grob-interpret-markup", &[me, markup])?;
        host.heap().root(v);
        text = host
            .heap()
            .foreign::<Stencil>(v)
            .copied()
            .unwrap_or_default();
    }

    let mut shorten = drul_property(host, me, "shorten-pair", [0.0, 0.0])?;

    /*
      TODO: we should check if there are ledgers, and modify length of
      the spanner to that.
    */
    for d in [LEFT, RIGHT] {
        let b = bounds[side(d)];

        let mut ext = Interval::EMPTY;
        if grob::has_interface(host.heap(), b, "note-column-interface") {
            let heads = grob_array::extract_grob_array(host, b, "note-heads")?;
            for h in heads {
                ext.unite(grob::extent(host, h, common, X)?);
                let dots = rhythmic_head::get_dots(host, h)?;

                if let Some(dots) = dots
                    && d == RIGHT
                {
                    ext.unite(grob::extent(host, dots, common, X)?);
                }
            }
        }

        if ext.is_empty() {
            ext = grob::robust_relative_extent(host, b, common, X)?;
        }

        if broken[side(d)] {
            let axis = host.heap().integer(0);
            let e = crate::call_lily(host, "%generic-bound-extent", &[b, common, axis])?;
            let e = grob::number_pair(host.heap(), e).unwrap_or(Interval::EMPTY);
            *span_points.at_mut(d) = e.at(RIGHT);
            shorten[side(d)] = 0.0;
        } else {
            *span_points.at_mut(d) = ext.at(d);
        }

        *span_points.at_mut(d) -= f64::from(d) * shorten[side(d)];
    }

    /*
      0.3 is ~ italic correction.
    */
    let text_size = if text.extent(X).is_empty() {
        0.0
    } else {
        text.extent(X).at(RIGHT) + 0.3
    };

    let minimum_length = grob::property(host, me, "minimum-length")?;
    let minimum_length = double_or(host, minimum_length, -1.0);
    span_points.left = f64::min(
        span_points.left,
        span_points.right - text_size - minimum_length,
    );

    let mut bracket_span_points = span_points;
    bracket_span_points.left += text_size;

    let mut edge_height = drul_property(host, me, "edge-height", [1.0, 1.0])?;

    let flare = drul_property(host, me, "bracket-flare", [0.0, 0.0])?;

    for d in [LEFT, RIGHT] {
        edge_height[side(d)] *= -f64::from(get_grob_direction(host, me)?);
        if broken[side(d)] {
            edge_height[side(d)] = 0.0;
        }
    }

    let mut b = Stencil::default();

    if !bracket_span_points.is_empty() && bracket_span_points.length() > 0.001 {
        b = make_bracket(
            host,
            me,
            Y,
            (bracket_span_points.length(), 0.0),
            edge_height,
            Interval::EMPTY,
            flare,
            [0.0, 0.0],
        )?;
    }

    /*
     * The vertical lines should not take space, for the following scenario:
     *
     * 8 -----+
     *     o  |
     *    |
     *    |
     *
     * Just a small amount, yes.  In tight situations, it is even
     * possible to center the `8' directly below the note, dropping the
     * ottava line completely...
     */

    b = Stencil::new(Box::new(b.extent(X), Interval::new(0.1, 0.1)), b.expr);

    let heap = host.heap();
    b.translate_axis(heap, bracket_span_points.left, X);
    text.translate_axis(heap, span_points.left, X);
    text.align_to(heap, Y, 0.0);
    b.add_stencil(heap, &text);
    heap.root(b.expr);

    let x = grob::relative_coordinate(host, me, common, X)?;
    let heap = host.heap();
    b.translate_axis(heap, -x, X);

    Ok(heap.wrap(b))
}

// --- the callback -----------------------------------------------------------

/// What `%host-call` runs for the `ly:ottava-bracket::` primitive (D127):
/// the callback its second argument names, of the grob given, with the VM
/// as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "ottava-bracket",
    f: host_ottava_bracket,
}];

/// The callbacks [`HOSTS`] runs, by name, which the callback table
/// (`callbacks.rs`) finds too.
pub(crate) const CALLBACKS: [(&str, Callback); 1] = [("print", print)];

fn host_ottava_bracket(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such ottava bracket callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

/// `(ly:ottava-bracket::print spanner)`: LilyPond's subr, a primitive
/// here so that it prints as LilyPond's. It checks its spanner and
/// tail-calls `%host-call` with its name ([`HOSTS`]).
pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    spanner::spanner_arg(cx, "ly:ottava-bracket::print", 1, args[0])?;
    let Some(f) = cx.global("lily", "%host-call") else {
        return Ok(Value::UNSPECIFIED);
    };
    let what = cx.heap_mut().intern("ottava-bracket");
    let name = cx.heap_mut().intern("print");
    Ok(cx.tail_call(f, &[what, name, args[0]]))
}
