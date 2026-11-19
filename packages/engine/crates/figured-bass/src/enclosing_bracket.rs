//! `enclosing-bracket.cc` (MS4 step 16): `ly:enclosing-bracket::print`
//! (`:94–123`) and `::width` (`:58–92`), a `BassFigureBracket`'s, the
//! music crate's placeholders until then (D67). Both kill a bracket with
//! no `elements`; the print draws a bracket at each side of their extent
//! (widened by `padding`), each as tall as the elements, the width those
//! of two brackets ten staff spaces tall, both pointing left as
//! LilyPond's do.

use lily_engine_layout::{
    Callback, Direction, Interval, Stencil, X, Y, axis_group, bracket, dispatch, grob,
    grob_array::extract_grob_array, host_call_of,
};
use lily_engine_music::Host;
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// `from_scm<double> (get_property (me, "padding"), 0.25)`.
fn padding(host: &mut dyn Host, me: Value) -> Result<f64, Exception> {
    let v = grob::property(host, me, "padding")?;
    let heap = host.heap();
    Ok(match heap.to_f64(v) {
        Some(x) if heap.is_real(v) => x,
        _ => 0.25,
    })
}

/// `width`: the X extent of the two brackets about the elements.
pub fn width(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let w = width_rooted(host, me);
    host.heap().unroot_to(depth);
    w
}

fn width_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    /*
       UGH. cut & paste code.
    */
    let elements = extract_grob_array(host, me, "elements")?;
    if elements.is_empty() {
        grob::suicide(host.heap(), me);
        return Ok(Value::NULL);
    }

    let common_x =
        grob::common_refpoint_of_array(host.heap(), &elements, Some(me), X).unwrap_or(me);
    let mut xext = axis_group::relative_group_extent(host, &elements, common_x, X)?;

    let mut left_br =
        bracket::make_axis_constrained_bracket(host, me, 10.0, Y, LEFT, Interval::EMPTY)?;
    host.heap().root(left_br.expr);
    let mut right_br =
        bracket::make_axis_constrained_bracket(host, me, 10.0, Y, LEFT, Interval::EMPTY)?;
    host.heap().root(right_br.expr);

    xext.widen(padding(host, me)?);
    let heap = host.heap();
    left_br.translate_axis(heap, xext.at(LEFT), X);
    right_br.translate_axis(heap, xext.at(RIGHT), X);

    left_br.add_stencil(heap, &right_br);
    let x = grob::relative_coordinate(host, me, common_x, X)?;
    left_br.translate_axis(host.heap(), -x, X);

    let ext = left_br.extent(X);
    let heap = host.heap();
    let (l, r) = (heap.flonum(ext.left), heap.flonum(ext.right));
    Ok(heap.cons(l, r))
}

/// `print`: a bracket at each side of the elements, as tall as they are.
pub fn print(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    let printed = print_rooted(host, me);
    host.heap().unroot_to(depth);
    printed
}

fn print_rooted(host: &mut dyn Host, me: Value) -> Result<Value, Exception> {
    let elements = extract_grob_array(host, me, "elements")?;
    if elements.is_empty() {
        grob::suicide(host.heap(), me);
        return Ok(Value::NULL);
    }

    let common_x =
        grob::common_refpoint_of_array(host.heap(), &elements, Some(me), X).unwrap_or(me);
    let mut xext = axis_group::relative_group_extent(host, &elements, common_x, X)?;
    if xext.is_empty() {
        grob::programming_error(host, me, "elements have no X extent.")?;
        xext = Interval::new(0.0, 0.0);
    }

    let mut left_br: Stencil = bracket::make_enclosing_bracket(host, me, me, &elements, Y, LEFT)?;
    let mut right_br = bracket::make_enclosing_bracket(host, me, me, &elements, Y, RIGHT)?;

    xext.widen(padding(host, me)?);
    let heap = host.heap();
    left_br.translate_axis(heap, xext.at(LEFT), X);
    right_br.translate_axis(heap, xext.at(RIGHT), X);

    left_br.add_stencil(heap, &right_br);
    let x = grob::relative_coordinate(host, me, common_x, X)?;
    left_br.translate_axis(host.heap(), -x, X);

    Ok(host.heap().wrap(left_br))
}

// --- the callbacks ------------------------------------------------------------

/// What `%host-call` runs for the `ly:enclosing-bracket::` primitives
/// (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "enclosing-bracket",
    f: host_enclosing_bracket,
}];

/// The `ly:enclosing-bracket::` callbacks, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 2] = [("print", print), ("width", width)];

fn host_enclosing_bracket(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    dispatch(vm, &CALLBACKS, args, "no such enclosing-bracket callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_print(cx: &mut Cx, args: &[Value]) -> R {
    host_call_of(
        cx,
        "enclosing-bracket",
        "ly:enclosing-bracket::",
        "print",
        args,
    )
}

pub(crate) fn ly_width(cx: &mut Cx, args: &[Value]) -> R {
    host_call_of(
        cx,
        "enclosing-bracket",
        "ly:enclosing-bracket::",
        "width",
        args,
    )
}
