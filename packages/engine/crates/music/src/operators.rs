//! `operators.scm`'s methods (D70): `+`, `-`, `*`, `/` and `<` of moments,
//! durations and pitches, which LilyPond defines with GOOPS on Guile's
//! primitive generics, given natively to the engine's operators through
//! [`lily_engine_scheme::Vm::set_operators`]. Each calls the `ly:` function
//! the method calls, so its errors are that function's: `<number>` takes
//! an inexact number too, which the function then refuses.

use lily_engine_scheme::{Cx, Value};

use crate::{Duration, Moment, Pitch, R, duration, pitch, rational};

/// What an argument is, for the methods' specializers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Moment,
    Duration,
    Pitch,
    Number,
    Other,
}

fn class(cx: &Cx, v: Value) -> Class {
    let heap = cx.heap();
    if heap.foreign::<Moment>(v).is_some() {
        Class::Moment
    } else if heap.foreign::<Duration>(v).is_some() {
        Class::Duration
    } else if heap.foreign::<Pitch>(v).is_some() {
        Class::Pitch
    } else if heap.is_number(v) {
        Class::Number
    } else {
        Class::Other
    }
}

/// The method of `op` for `args`, one or two of them; None if
/// `operators.scm` has none.
pub(crate) fn operator(cx: &mut Cx, op: &str, args: &[Value]) -> Option<R> {
    use Class::*;
    let classes = match *args {
        [a] => (class(cx, a), Other),
        [a, b] => (class(cx, a), class(cx, b)),
        _ => return None,
    };
    let (a, b) = (args[0], args.get(1).copied().unwrap_or(Value::UNSPECIFIED));
    Some(match (op, args.len(), classes) {
        ("+", 2, (Moment, Moment)) => rational::moment_add(cx, &[a, b]),
        ("-", 2, (Moment, Moment)) => rational::moment_sub(cx, &[a, b]),
        ("-", 1, (Moment, _)) => {
            let zero = cx.heap_mut().integer(0);
            rational::make_moment(cx, &[zero]).and_then(|z| rational::moment_sub(cx, &[z, a]))
        }
        ("-", 2, (Number, Moment)) => {
            rational::make_moment(cx, &[a]).and_then(|m| rational::moment_sub(cx, &[m, b]))
        }
        ("*", 2, (Moment, Number)) => rational::moment_mul(cx, &[a, b]),
        ("*", 2, (Number, Moment)) => rational::moment_mul(cx, &[b, a]),
        ("/", 2, (Moment, Number)) => rational::moment_div(cx, &[a, b]),
        ("/", 2, (Moment, Moment)) => {
            rational::moment_div(cx, &[a, b]).and_then(|m| rational::moment_main(cx, &[m]))
        }
        ("<", 2, (Moment, Moment)) => rational::moment_less(cx, &[a, b]),
        ("*", 2, (Duration, Number)) => duration::duration_compress(cx, &[a, b]),
        ("*", 2, (Number, Duration)) => duration::duration_compress(cx, &[b, a]),
        ("/", 2, (Duration, Number)) => match cx.heap_mut().reciprocal(b) {
            Ok(r) => duration::duration_compress(cx, &[a, r]),
            Err(e) => Err(cx.num_error("/", e, &[b])),
        },
        ("+", 2, (Pitch, Pitch)) => pitch::pitch_transpose(cx, &[a, b]),
        ("-", 2, (Pitch, Pitch)) => pitch::pitch_diff(cx, &[a, b]),
        ("-", 1, (Pitch, _)) => pitch::pitch_negate(cx, &[a]),
        ("-", 2, (Number, Pitch)) => {
            let zero = cx.heap_mut().integer(0);
            pitch::make_pitch(cx, &[zero, a]).and_then(|p| pitch::pitch_diff(cx, &[p, b]))
        }
        ("<", 2, (Pitch, Pitch)) => pitch::pitch_less(cx, &[a, b]),
        _ => return None,
    })
}
