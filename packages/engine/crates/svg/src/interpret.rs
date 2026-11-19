//! `stencil-interpret.cc`: the walk of a stencil's expression into the
//! outputter's calls (O1). A translation is added up and given with each
//! primitive (`settranslation`); `grob-cause`, `color`, `output-attributes`,
//! `rotate-stencil` and `scale-stencil` open and close around what they
//! hold; `combine-stencil` walks each part; `delay-stencil-evaluation` walks
//! what its promise gives; `with-outline` walks its stencil, without the
//! outline; `footnote` gives nothing.

use lily_engine_layout::Offset;
use lily_engine_scheme::{Exception, Heap, Value, Vm};

/// What the walk gives the outputter, as `Stencil_sink::output` receives it:
/// the head of each list `interpret_stencil_expression` makes, and its
/// arguments.
#[derive(Clone, Copy, Debug)]
pub enum Call {
    /// `(settranslation x y)`, before each primitive.
    SetTranslation(Offset),
    /// The primitive, `(head arg …)`, as the stencil holds it.
    Primitive(Value),
    /// `(resettranslation)`, after it.
    ResetTranslation,
    /// `(grob-cause offset grob)`.
    GrobCause(Offset, Value),
    /// `(no-origin)`.
    NoOrigin,
    /// `(setcolor r g b a)`.
    SetColor([Value; 4]),
    /// `(resetcolor)`.
    ResetColor,
    /// `(start-group-node attributes)`.
    StartGroupNode(Value),
    /// `(end-group-node)`.
    EndGroupNode,
    /// `(setrotation angle x y)`, about the point `(x, y)` of the page.
    SetRotation(Value, f64, f64),
    /// `(resetrotation angle x y)`.
    ResetRotation,
    /// `(setscale x y)`.
    SetScale(Value, Value),
    /// `(resetscale)`.
    ResetScale,
}

/// `Stencil_sink`: where the walk's calls go. `output` gives whether a
/// procedure took the call, as `Paper_outputter::output_scheme`'s result is
/// a string or `#f`.
pub trait Sink {
    fn output(&mut self, vm: &mut Vm, call: Call) -> Result<bool, Exception>;
}

/// `interpret_stencil_expression`: walks `expr` at the offset `o`, giving
/// `sink` its calls. `expr` must stay reachable from a root of the heap's
/// while the walk runs, as a promise it forces may collect.
pub fn interpret(
    vm: &mut Vm,
    mut expr: Value,
    sink: &mut dyn Sink,
    mut o: Offset,
) -> Result<(), Exception> {
    loop {
        let heap = vm.heap();
        let Some(head) = heap.car(expr) else {
            return Ok(());
        };
        match heap.name(head).unwrap_or("") {
            "delay-stencil-evaluation" => {
                let promise = nth(heap, expr, 1);
                let forced = force(vm, promise)?;
                return interpret(vm, forced, sink, o);
            }
            "footnote" => return Ok(()),
            "translate-stencil" => {
                let (x, y) = pair(heap, nth(heap, expr, 1));
                o = (o.0 + x, o.1 + y);
                expr = nth(heap, expr, 2);
            }
            "combine-stencil" => {
                let mut parts = heap.cdr(expr).unwrap_or(Value::NULL);
                while let Some(part) = vm.heap().car(parts) {
                    interpret(vm, part, sink, o)?;
                    parts = vm.heap().cdr(parts).unwrap_or(Value::NULL);
                }
                return Ok(());
            }
            "grob-cause" => {
                let (grob, inner) = (nth(heap, expr, 1), nth(heap, expr, 2));
                sink.output(vm, Call::GrobCause(o, grob))?;
                interpret(vm, inner, sink, o)?;
                sink.output(vm, Call::NoOrigin)?;
                return Ok(());
            }
            "color" => {
                let color = nth(heap, expr, 1);
                let rgba = [0, 1, 2, 3].map(|i| nth(heap, color, i));
                let inner = nth(heap, expr, 2);
                sink.output(vm, Call::SetColor(rgba))?;
                interpret(vm, inner, sink, o)?;
                sink.output(vm, Call::ResetColor)?;
                return Ok(());
            }
            "output-attributes" => {
                let (attributes, inner) = (nth(heap, expr, 1), nth(heap, expr, 2));
                sink.output(vm, Call::StartGroupNode(attributes))?;
                interpret(vm, inner, sink, o)?;
                sink.output(vm, Call::EndGroupNode)?;
                return Ok(());
            }
            "rotate-stencil" => {
                let args = nth(heap, expr, 1);
                let angle = nth(heap, args, 0);
                // `from_scm (…, Offset (0.0, 0.0))`: the origin for what is no offset.
                let about = offset_or_origin(heap, nth(heap, args, 1));
                let (x, y) = (o.0 + about.0, o.1 + about.1);
                let inner = nth(heap, expr, 2);
                sink.output(vm, Call::SetRotation(angle, x, y))?;
                interpret(vm, inner, sink, o)?;
                sink.output(vm, Call::ResetRotation)?;
                return Ok(());
            }
            "scale-stencil" => {
                let args = nth(heap, expr, 1);
                let (sx, sy) = (nth(heap, args, 0), nth(heap, args, 1));
                let (fx, fy) = (number(heap, sx), number(heap, sy));
                let unscaled = (o.0 / fx, o.1 / fy);
                let inner = nth(heap, expr, 2);
                sink.output(vm, Call::SetScale(sx, sy))?;
                interpret(vm, inner, sink, unscaled)?;
                sink.output(vm, Call::ResetScale)?;
                return Ok(());
            }
            "with-outline" => expr = nth(heap, expr, 2),
            _ => {
                sink.output(vm, Call::SetTranslation(o))?;
                let taken = sink.output(vm, Call::Primitive(expr))?;
                sink.output(vm, Call::ResetTranslation)?;
                // A `utf-8-string` no procedure took is drawn as its glyphs.
                if !taken && vm.heap().name(head) == Some("utf-8-string") {
                    expr = nth(vm.heap(), expr, 3);
                    continue;
                }
                return Ok(());
            }
        }
    }
}

/// `scm_force`: what `promise` gives, through Guile's `force` where the
/// interpreter has one; anything that is no promise is itself.
fn force(vm: &mut Vm, promise: Value) -> Result<Value, Exception> {
    let force = vm
        .slot("guile", "force")
        .and_then(|slot| vm.slot_value(slot))
        .filter(|&f| vm.heap().is_procedure(f));
    match force {
        Some(f) => vm.apply(f, &[promise]),
        None => Ok(promise),
    }
}

/// The item `i` of a list, or `()`.
pub(crate) fn nth(heap: &Heap, mut list: Value, i: usize) -> Value {
    for _ in 0..i {
        list = heap.cdr(list).unwrap_or(Value::NULL);
    }
    heap.car(list).unwrap_or(Value::NULL)
}

/// A number as a double, 0 for what is none (`from_scm<double>`).
pub(crate) fn number(heap: &Heap, v: Value) -> f64 {
    heap.to_f64(v).unwrap_or(0.0)
}

/// `from_scm<Offset>`: a pair of numbers.
pub(crate) fn pair(heap: &Heap, v: Value) -> Offset {
    let (a, d) = (
        heap.car(v).unwrap_or(Value::NULL),
        heap.cdr(v).unwrap_or(Value::NULL),
    );
    (number(heap, a), number(heap, d))
}

/// `from_scm (v, Offset (0.0, 0.0))`.
fn offset_or_origin(heap: &Heap, v: Value) -> Offset {
    match (heap.car(v), heap.cdr(v)) {
        (Some(a), Some(d)) if heap.is_number(a) && heap.is_number(d) => pair(heap, v),
        _ => (0.0, 0.0),
    }
}
