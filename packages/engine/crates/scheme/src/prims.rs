//! The primitives (chapter 03 §5.3): Rust procedures the VM calls with
//! their arguments on its stack. So far those that chapter 03 Appendix B's
//! benchmarks and the VM's own Scheme use, and the core around them: numbers,
//! pairs and lists, symbols, characters, strings, vectors, `values`, `apply`
//! and output. S3 adds the rest of §2.6's subset.
//!
//! [`Inline`] names the few the compiler runs in the loop itself. Each
//! primitive's errors are worded as Guile 3.0.11's (`errors.rs`).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use crate::array;
use crate::control;
use crate::errors::{MAX_VECTOR_LENGTH, SIZE_MAX};
use crate::expand::syntax_case;
use crate::format;
use crate::goops;
use crate::hash;
use crate::heap::Heap;
use crate::library;
use crate::lily;
use crate::lists;
use crate::module;
use crate::number::{NumError, NumResult, Rounding};
use crate::ports;
use crate::regex;
use crate::strings;
use crate::value::{Tag, Value};
use crate::vm::{Cx, Thrown};

/// A primitive's code: its arguments are on the VM's stack, as many as its
/// [`Prim`] allows.
pub type PrimFn = fn(&mut Cx, &[Value]) -> Result<Value, Thrown>;

/// `max` of a primitive that takes any number of arguments.
pub const REST: u8 = u8::MAX;

/// A primitive: its name, how many arguments it takes, at least `min` and
/// at most `max` ([`REST`] for any number), and its code. Another crate
/// gives a table of them to [`crate::Vm::define_primitives`].
pub struct Prim {
    pub name: &'static str,
    pub min: u8,
    pub max: u8,
    pub f: PrimFn,
}

/// The primitive at `index`: the crate's own ([`PRIMS`]), then those other
/// crates defined ([`Heap::natives`]), in the order they defined them.
#[inline]
pub(crate) fn get(heap: &Heap, index: u32) -> Option<&'static Prim> {
    let index = index as usize;
    match PRIMS.get(index) {
        Some(prim) => Some(prim),
        None => heap.natives.get(index - PRIMS.len()).copied(),
    }
}

/// `apply`'s index: the VM spreads its arguments itself.
pub(crate) const APPLY: u32 = 0;
/// `eval`'s, and `primitive-eval`'s after it: the VM compiles and calls the
/// form.
pub(crate) const EVAL: u32 = 1;
/// `gc`'s: the VM collects with its own roots. The four are the first
/// primitives, so that one comparison tells them from the others.
pub(crate) const GC: u32 = 3;

/// The primitive named `name`, which must be one.
pub(crate) fn primitive(name: &str) -> Value {
    let index = PRIMS.iter().position(|p| p.name == name).unwrap_or(0);
    Value::tagged(Tag::Primitive, index as u32)
}

/// Whether a call of the global `name` with `argc` arguments inside a
/// lexical environment words its errors as Guile's compiled code does, not
/// as the procedure (`errors.rs`): of `eval.scm`'s primitive calls, those
/// whose wording differs.
pub(crate) fn direct_call(name: &str, argc: usize) -> bool {
    matches!(
        (name, argc),
        ("car" | "cdr" | "vector-length", 1) | ("vector-ref", 2) | ("vector-set!", 3)
    )
}

/// The modules other than `(guile)` that every VM has, by their names:
/// the primitives of the table each holds, and its Scheme, run in it after
/// `(guile)`'s, in this order. `(lily)` is the one module here that is not
/// made for the list ([`crate::module::LILY`]).
pub(crate) const MODULES: &[(&[&str], &[&str], &str)] = &[
    (&["srfi", "srfi-1"], lists::SRFI_1, lists::SRFI_1_PRELUDE),
    (&["rnrs", "bytevectors"], array::BYTEVECTORS, ""),
    (&["ice-9", "iconv"], &["string->bytevector"], ""),
    // Guile's `ice-9/format.scm` makes its `format` the root module's when
    // it loads, which LilyPond's `lily.scm` makes it do: so a score's
    // Scheme formats with it (`~16a`), and `(lily)` keeps `simple-format`.
    (
        &["ice-9", "format"],
        &["format"],
        "(module-set! (resolve-module '(guile)) 'format format)",
    ),
    (&["srfi", "srfi-2"], &[], library::SRFI_2),
    (&["srfi", "srfi-13"], &[], ""),
    (&["srfi", "srfi-11"], &[], library::SRFI_11),
    (&["srfi", "srfi-39"], &[], library::SRFI_39),
    (&["ice-9", "receive"], &[], library::RECEIVE),
    (&["ice-9", "list"], &[], library::LIST),
    (&["ice-9", "optargs"], &[], library::OPTARGS),
    (&["ice-9", "match"], &[], library::r#match::PRELUDE),
    (
        &["ice-9", "pretty-print"],
        library::PRETTY_PRINT,
        library::PRETTY_PRINT_PRELUDE,
    ),
    (&["ice-9", "textual-ports"], library::TEXTUAL_PORTS, ""),
    (
        &["ice-9", "binary-ports"],
        library::BINARY_PORTS,
        library::BINARY_PORTS_PRELUDE,
    ),
    (&["ice-9", "arrays"], library::ARRAYS, ""),
    (&["oop", "goops"], goops::NAMES, goops::PRELUDE),
    (&["ice-9", "control"], &[], ""),
    (&["ice-9", "and-let-star"], &[], library::SRFI_2),
    (&["lily", "curried-definitions"], &[], ""),
    (&["lily"], lily::NAMES, lily::PRELUDE),
    (&["lily"], regex::NAMES, regex::PRELUDE),
];

/// `*unspecified*` and the like, in `(guile)`.
pub(crate) const PRELUDE: &str = r#"
(define *unspecified* (if #f #f))
(define (const value) (lambda args value))
(define (negate pred) (lambda args (not (apply pred args))))
(define format simple-format)
"#;

/// The primitives the compiler runs inline while their global still holds
/// them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Inline {
    Add,
    Sub,
    Mul,
    NumEq,
    Lt,
    Gt,
    Le,
    Ge,
    Car,
    Cdr,
    Cons,
    IsNull,
    IsPair,
    Not,
    IsEq,
    IsZero,
}

impl Inline {
    pub(crate) const ALL: [Inline; 16] = [
        Inline::Add,
        Inline::Sub,
        Inline::Mul,
        Inline::NumEq,
        Inline::Lt,
        Inline::Gt,
        Inline::Le,
        Inline::Ge,
        Inline::Car,
        Inline::Cdr,
        Inline::Cons,
        Inline::IsNull,
        Inline::IsPair,
        Inline::Not,
        Inline::IsEq,
        Inline::IsZero,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Inline::Add => "+",
            Inline::Sub => "-",
            Inline::Mul => "*",
            Inline::NumEq => "=",
            Inline::Lt => "<",
            Inline::Gt => ">",
            Inline::Le => "<=",
            Inline::Ge => ">=",
            Inline::Car => "car",
            Inline::Cdr => "cdr",
            Inline::Cons => "cons",
            Inline::IsNull => "null?",
            Inline::IsPair => "pair?",
            Inline::Not => "not",
            Inline::IsEq => "eq?",
            Inline::IsZero => "zero?",
        }
    }

    pub(crate) fn arity(self) -> usize {
        match self {
            Inline::Car
            | Inline::Cdr
            | Inline::IsNull
            | Inline::IsPair
            | Inline::Not
            | Inline::IsZero => 1,
            _ => 2,
        }
    }
}

/// The op for a call of the global `name` with `argc` arguments: `+`, `-`
/// and `*` of more than two are folded into ops of two.
pub(crate) fn inline(name: &str, argc: usize) -> Option<Inline> {
    let op = Inline::ALL.into_iter().find(|op| op.name() == name)?;
    let fits = match op {
        Inline::Add | Inline::Sub | Inline::Mul => argc >= 2,
        _ => argc == op.arity(),
    };
    fits.then_some(op)
}

/// How Guile writes a primitive: `#<procedure car (_)>`, its arguments as
/// `_`, the optional ones after `#:optional`.
pub(crate) fn write_primitive(prim: Option<&Prim>, out: &mut String) {
    let Some(prim) = prim else {
        out.push_str("#<procedure>");
        return;
    };
    // Guile's are Scheme, which names its arguments.
    let formals = match prim.name {
        "apply" => Some("(fun args) | (fun arg1 . args)"),
        "throw" => Some("(key . args)"),
        "eq?" | "eqv?" | "equal?" => Some("(#:optional _ _ . _)"),
        _ => None,
    };
    if let Some(formals) = formals {
        let _ = write!(out, "#<procedure {} {formals}>", prim.name);
        return;
    }
    let _ = write!(out, "#<procedure {} ", prim.name);
    let (min, max) = (prim.min as usize, prim.max);
    if min == 0 && max == REST {
        out.push_str("_>");
        return;
    }
    out.push('(');
    let mut items: Vec<&str> = (0..min).map(|_| "_").collect();
    if max != REST && max as usize > min {
        items.push("#:optional");
        items.extend((min..max as usize).map(|_| "_"));
    }
    out.push_str(&items.join(" "));
    if max == REST {
        out.push_str(" . _");
    }
    out.push_str(")>");
}

// --- helpers -----------------------------------------------------------------------

type R = Result<Value, Thrown>;

fn num(r: NumResult, cx: &mut Cx, subr: &str, args: &[Value]) -> R {
    r.map_err(|e| cx.num_error(subr, e, args))
}

fn test(r: Result<bool, NumError>, cx: &mut Cx, subr: &str, args: &[Value]) -> R {
    r.map(Value::bool).map_err(|e| cx.num_error(subr, e, args))
}

fn pair(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<(Value, Value), Thrown> {
    match (cx.heap.car(v), cx.heap.cdr(v)) {
        (Some(car), Some(cdr)) => Ok((car, cdr)),
        _ => Err(cx.wrong_type(subr, position, v, "pair")),
    }
}

/// The items of a list, and the tail that ended it, `()` for a proper one.
fn walk(cx: &Cx, list: Value) -> (Vec<Value>, Value) {
    let mut out = Vec::new();
    let mut at = list;
    while let (Some(car), Some(cdr)) = (cx.heap.car(at), cx.heap.cdr(at)) {
        out.push(car);
        at = cdr;
    }
    (out, at)
}

/// The items of a proper list, or `Wrong type argument in position N: list`.
fn items(cx: &mut Cx, subr: &str, position: usize, list: Value) -> Result<Vec<Value>, Thrown> {
    match walk(cx, list) {
        (out, Value::NULL) => Ok(out),
        _ => Err(cx.wrong_type_at(subr, position, list)),
    }
}

/// An exact integer from `low` to `high`, checked as Guile's C procedures
/// check a bounded index.
fn bounded(cx: &mut Cx, v: Value, low: i64, high: u64) -> Result<usize, Thrown> {
    match cx.heap.to_i64(v) {
        Some(n) if n >= low && n as u64 <= high => Ok(n as usize),
        Some(_) => Err(cx.out_of_bounds(low, high, v)),
        None if cx.heap.is_exact_integer(v) => Err(cx.out_of_bounds(low, high, v)),
        None => Err(cx.wrong_type_vm(None, v, "exact integer")),
    }
}

/// A string's index: below its length.
fn string_index(cx: &mut Cx, len: usize, v: Value) -> Result<usize, Thrown> {
    if len == 0 && cx.heap.is_exact_integer(v) {
        return Err(cx.value_out_of_range(None, v));
    }
    bounded(cx, v, 0, len.saturating_sub(1) as u64)
}

/// A vector's index for `vector-ref` and `vector-set!`: worded as the
/// procedure words it, or as compiled code for a direct call.
fn vector_index(cx: &mut Cx, subr: &str, len: usize, v: Value) -> Result<usize, Thrown> {
    if !cx.direct {
        return match cx.heap.to_i64(v) {
            Some(n) if n < 0 => Err(cx.out_of_bounds(0, SIZE_MAX, v)),
            Some(n) if (n as u64) < len as u64 => Ok(n as usize),
            _ if cx.heap.is_exact_integer(v) => Err(cx.value_out_of_range(None, v)),
            _ => Err(cx.wrong_type_vm(None, v, "exact integer")),
        };
    }
    match v.as_fixnum() {
        Some(n) if n >= 0 && (n as u64) < len as u64 => Ok(n as usize),
        Some(_) => Err(cx.out_of_range_compiled(subr, 2, v)),
        None => Err(cx.wrong_type_compiled(subr, 2, v, "small integer")),
    }
}

fn text(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<String, Thrown> {
    match cx.heap.str(v) {
        Some(s) => Ok(String::from(s.as_str())),
        None => Err(cx.wrong_type(subr, position, v, "string")),
    }
}

fn character(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<char, Thrown> {
    v.as_char()
        .ok_or_else(|| cx.wrong_type(subr, position, v, "character"))
}

/// A vector that may be changed.
fn vector_len(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<usize, Thrown> {
    match cx.heap.vector_items(v) {
        Some(items) => Ok(items.len()),
        None => Err(cx.wrong_type(subr, position, v, "mutable vector")),
    }
}

/// A radix from 2 to `high`.
fn radix(cx: &mut Cx, args: &[Value], at: usize, high: u64) -> Result<u32, Thrown> {
    match args.get(at) {
        None => Ok(10),
        Some(&r) => bounded(cx, r, 2, high).map(|n| n as u32),
    }
}

// --- numbers -------------------------------------------------------------------------

/// Methods of another crate's types for the arithmetic operators (D70):
/// called with the operator's name and the arguments that were no numbers,
/// a pair or, for `-`, one; None if it has no method for them, and the
/// operator's error is raised.
pub type OperatorFn = fn(&mut Cx, &str, &[Value]) -> Option<R>;

/// The method of [`OperatorFn`] for `args`, if the VM has one.
fn method(cx: &mut Cx, subr: &str, args: &[Value]) -> Option<R> {
    let f = cx.heap.operators?;
    f(cx, subr, args)
}

/// The one argument of `(+ x)` or `(* x)`, which must be a number.
fn number(cx: &mut Cx, subr: &str, args: &[Value], x: Value) -> R {
    if cx.heap.is_number(x) {
        Ok(x)
    } else {
        Err(cx.num_error(subr, NumError::WrongType(1), args))
    }
}

/// The one argument of `(< x)` or `(max x)`, which must be real.
fn real(cx: &mut Cx, subr: &str, args: &[Value], x: Value) -> R {
    if cx.heap.is_real(x) {
        Ok(x)
    } else {
        Err(cx.num_error(subr, NumError::WrongType(1), args))
    }
}

/// `+`, `-`, `*` and `/` of more than one argument, folded from the left as
/// Guile folds them, so a wrong type is the position within its pair.
fn fold(
    cx: &mut Cx,
    subr: &str,
    args: &[Value],
    f: fn(&mut crate::heap::Heap, Value, Value) -> NumResult,
) -> R {
    let mut acc = args[0];
    for &x in &args[1..] {
        acc = match f(&mut cx.heap, acc, x) {
            Ok(v) => v,
            Err(e @ NumError::WrongType(_)) => match method(cx, subr, &[acc, x]) {
                Some(r) => r?,
                None => return Err(cx.num_error(subr, e, &[acc, x])),
            },
            Err(e) => return Err(cx.num_error(subr, e, &[acc, x])),
        };
    }
    Ok(acc)
}

fn add(cx: &mut Cx, args: &[Value]) -> R {
    match args {
        [] => Ok(Value::fixnum(0).unwrap_or(Value::UNSPECIFIED)),
        [x] => number(cx, "+", args, *x),
        _ => fold(cx, "+", args, |h, a, b| h.add(a, b)),
    }
}

fn mul(cx: &mut Cx, args: &[Value]) -> R {
    match args {
        [] => Ok(Value::fixnum(1).unwrap_or(Value::UNSPECIFIED)),
        [x] => number(cx, "*", args, *x),
        _ => fold(cx, "*", args, |h, a, b| h.mul(a, b)),
    }
}

fn sub(cx: &mut Cx, args: &[Value]) -> R {
    match args {
        [x] => match cx.heap.negate(*x) {
            Err(NumError::WrongType(_)) if let Some(r) = method(cx, "-", args) => r,
            r => num(r, cx, "-", args),
        },
        _ => fold(cx, "-", args, |h, a, b| h.sub(a, b)),
    }
}

fn div(cx: &mut Cx, args: &[Value]) -> R {
    match args {
        [x] => num(cx.heap.reciprocal(*x), cx, "/", args),
        _ => fold(cx, "/", args, |h, a, b| h.div(a, b)),
    }
}

/// `=`, `<` and the rest over any number of arguments, pair by pair, until
/// one pair is false. A wrong type is the position within its pair, but `=`
/// always says position 1, as Guile's does.
fn compare(
    cx: &mut Cx,
    subr: &str,
    args: &[Value],
    f: fn(&crate::heap::Heap, Value, Value) -> Result<bool, NumError>,
) -> R {
    for w in args.windows(2) {
        match f(&cx.heap, w[0], w[1]) {
            Ok(true) => {}
            Ok(false) => return Ok(Value::FALSE),
            Err(NumError::WrongType(_)) if let Some(r) = method(cx, subr, w) => {
                if r?.is_false() {
                    return Ok(Value::FALSE);
                }
            }
            Err(NumError::WrongType(n)) => {
                let position = if subr == "=" { 1 } else { n as usize };
                return Err(cx.wrong_type_at(subr, position, w[n as usize - 1]));
            }
            Err(e) => return Err(cx.num_error(subr, e, w)),
        }
    }
    Ok(Value::TRUE)
}

fn num_eq(cx: &mut Cx, args: &[Value]) -> R {
    compare(cx, "=", args, |h, a, b| h.num_eq(a, b))
}
fn lt(cx: &mut Cx, args: &[Value]) -> R {
    compare(cx, "<", args, |h, a, b| h.less(a, b))
}
fn gt(cx: &mut Cx, args: &[Value]) -> R {
    compare(cx, ">", args, |h, a, b| h.greater(a, b))
}
fn le(cx: &mut Cx, args: &[Value]) -> R {
    compare(cx, "<=", args, |h, a, b| h.less_eq(a, b))
}
fn ge(cx: &mut Cx, args: &[Value]) -> R {
    compare(cx, ">=", args, |h, a, b| h.greater_eq(a, b))
}

fn max(cx: &mut Cx, args: &[Value]) -> R {
    if let [x] = args {
        return real(cx, "max", args, *x);
    }
    fold(cx, "max", args, |h, a, b| h.max(a, b))
}

fn min(cx: &mut Cx, args: &[Value]) -> R {
    if let [x] = args {
        return real(cx, "min", args, *x);
    }
    fold(cx, "min", args, |h, a, b| h.min(a, b))
}

fn gcd(cx: &mut Cx, args: &[Value]) -> R {
    match args {
        [] => Ok(Value::fixnum(0).unwrap_or(Value::UNSPECIFIED)),
        // Guile's gcd of one is its abs.
        [x] => {
            let zero = Value::fixnum(0).unwrap_or(Value::UNSPECIFIED);
            let r = cx.heap.gcd(zero, *x);
            r.map_err(|e| {
                let e = match e {
                    NumError::WrongType(_) => NumError::WrongType(1),
                    e => e,
                };
                cx.num_error("abs", e, args)
            })
        }
        _ => fold(cx, "gcd", args, |h, a, b| h.gcd(a, b)),
    }
}

fn lcm(cx: &mut Cx, args: &[Value]) -> R {
    match args {
        [] => Ok(Value::fixnum(1).unwrap_or(Value::UNSPECIFIED)),
        [x] => {
            let one = Value::fixnum(1).unwrap_or(Value::UNSPECIFIED);
            let r = cx.heap.lcm(one, *x);
            r.map_err(|e| {
                let e = match e {
                    NumError::WrongType(_) => NumError::WrongType(1),
                    e => e,
                };
                cx.num_error("lcm", e, args)
            })
        }
        _ => fold(cx, "lcm", args, |h, a, b| h.lcm(a, b)),
    }
}

/// `expt`: Guile multiplies, so a base that is no number is `*`'s error.
fn expt(cx: &mut Cx, args: &[Value]) -> R {
    let r = cx.heap.expt(args[0], args[1]);
    let subr = match r {
        Err(NumError::WrongType(1)) => "*",
        _ => "expt",
    };
    num(r, cx, subr, args)
}

/// A primitive of one number that a heap method computes.
macro_rules! unary {
    ($f:ident, $name:literal, $method:ident) => {
        fn $f(cx: &mut Cx, args: &[Value]) -> R {
            let r = cx.heap.$method(args[0]);
            num(r, cx, $name, args)
        }
    };
}

/// A primitive of two numbers that a heap method computes.
macro_rules! binary {
    ($f:ident, $name:literal, $method:ident) => {
        fn $f(cx: &mut Cx, args: &[Value]) -> R {
            let r = cx.heap.$method(args[0], args[1]);
            num(r, cx, $name, args)
        }
    };
}

/// A predicate on numbers that may refuse its argument.
macro_rules! number_test {
    ($f:ident, $name:literal, $method:ident) => {
        fn $f(cx: &mut Cx, args: &[Value]) -> R {
            let r = cx.heap.$method(args[0]);
            test(r, cx, $name, args)
        }
    };
}

/// A predicate on any value.
macro_rules! predicate {
    ($f:ident, |$cx:ident, $v:ident| $body:expr) => {
        fn $f($cx: &mut Cx, args: &[Value]) -> R {
            let $v = args[0];
            let _ = &$cx;
            Ok(Value::bool($body))
        }
    };
}

binary!(quotient, "quotient", quotient);
binary!(remainder, "remainder", remainder);
binary!(modulo, "modulo", modulo);
unary!(abs, "abs", abs);
unary!(one_plus, "+", one_plus);
unary!(one_minus, "-", one_minus);
unary!(exact, "inexact->exact", exact);
unary!(inexact, "exact->inexact", inexact);
unary!(numerator, "numerator", numerator);
unary!(denominator, "denominator", denominator);
unary!(sqrt, "sqrt", sqrt);
unary!(exp, "exp", exp);
unary!(log, "log", log);
unary!(sin, "sin", sin);
unary!(cos, "cos", cos);
unary!(tan, "tan", tan);
unary!(asin, "asin", asin);
unary!(acos, "acos", acos);
unary!(real_part, "real-part", real_part);
unary!(imag_part, "imag-part", imag_part);
unary!(magnitude, "magnitude", magnitude);
unary!(angle, "angle", angle);
unary!(lognot, "lognot", lognot);
unary!(logcount, "logcount", logcount);
unary!(integer_length, "integer-length", integer_length);
binary!(ash, "ash", ash);
binary!(make_rectangular, "make-rectangular", make_rectangular);
binary!(make_polar, "make-polar", make_polar);
number_test!(is_nan, "nan?", is_nan);
number_test!(is_inf, "inf?", is_inf);
number_test!(is_finite, "finite?", is_finite);

fn logtest(cx: &mut Cx, args: &[Value]) -> R {
    let r = cx.heap.logtest(args[0], args[1]);
    test(r, cx, "logtest", args)
}

fn logbit(cx: &mut Cx, args: &[Value]) -> R {
    let r = cx.heap.logbit(args[0], args[1]);
    test(r, cx, "logbit?", args)
}

/// `logand`, `logior` and `logxor` of any number of integers.
fn bitwise(
    cx: &mut Cx,
    subr: &str,
    identity: i64,
    args: &[Value],
    f: fn(&mut crate::heap::Heap, Value, Value) -> NumResult,
) -> R {
    match args {
        [] => Ok(cx.heap.integer(identity)),
        [x] => {
            let r = f(&mut cx.heap, *x, *x);
            num(r, cx, subr, args).map(|_| *x)
        }
        _ => fold(cx, subr, args, f),
    }
}

fn logand(cx: &mut Cx, args: &[Value]) -> R {
    bitwise(cx, "logand", -1, args, |h, a, b| h.logand(a, b))
}
fn logior(cx: &mut Cx, args: &[Value]) -> R {
    bitwise(cx, "logior", 0, args, |h, a, b| h.logior(a, b))
}
fn logxor(cx: &mut Cx, args: &[Value]) -> R {
    bitwise(cx, "logxor", 0, args, |h, a, b| h.logxor(a, b))
}

fn exact_integer_sqrt(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.exact_integer_sqrt(args[0]) {
        Ok((s, r)) => Ok(cx.heap.values(alloc::vec![s, r])),
        Err(e) => Err(cx.num_error("exact-integer-sqrt", e, args)),
    }
}

/// The division families: `floor/` gives both values, `floor-quotient`
/// and `floor-remainder` one each.
macro_rules! division {
    ($mode:ident, $both:ident, $bs:literal, $q:ident, $qs:literal, $r:ident, $rs:literal) => {
        fn $both(cx: &mut Cx, args: &[Value]) -> R {
            match cx.heap.divide(Rounding::$mode, args[0], args[1]) {
                Ok((q, r)) => Ok(cx.heap.values(alloc::vec![q, r])),
                Err(e) => Err(cx.num_error($bs, e, args)),
            }
        }
        fn $q(cx: &mut Cx, args: &[Value]) -> R {
            let r = cx
                .heap
                .divide(Rounding::$mode, args[0], args[1])
                .map(|p| p.0);
            num(r, cx, $qs, args)
        }
        fn $r(cx: &mut Cx, args: &[Value]) -> R {
            let r = cx
                .heap
                .divide(Rounding::$mode, args[0], args[1])
                .map(|p| p.1);
            num(r, cx, $rs, args)
        }
    };
}

division!(
    Floor,
    floor_div,
    "floor/",
    floor_quotient,
    "floor-quotient",
    floor_remainder,
    "floor-remainder"
);
division!(
    Ceiling,
    ceiling_div,
    "ceiling/",
    ceiling_quotient,
    "ceiling-quotient",
    ceiling_remainder,
    "ceiling-remainder"
);
division!(
    Truncate,
    truncate_div,
    "truncate/",
    truncate_quotient,
    "truncate-quotient",
    truncate_remainder,
    "truncate-remainder"
);
division!(
    Round,
    round_div,
    "round/",
    round_quotient,
    "round-quotient",
    round_remainder,
    "round-remainder"
);
division!(
    Euclidean,
    euclidean_div,
    "euclidean/",
    euclidean_quotient,
    "euclidean-quotient",
    euclidean_remainder,
    "euclidean-remainder"
);
division!(
    Centered,
    centered_div,
    "centered/",
    centered_quotient,
    "centered-quotient",
    centered_remainder,
    "centered-remainder"
);

number_test!(is_zero, "zero?", is_zero);
number_test!(is_positive, "positive?", is_positive);
number_test!(is_negative, "negative?", is_negative);
number_test!(is_odd, "odd?", is_odd);
number_test!(is_even, "even?", is_even);
number_test!(is_exact, "exact?", is_exact);
number_test!(is_inexact, "inexact?", is_inexact);
predicate!(is_number, |cx, v| cx.heap.is_number(v));
predicate!(is_real, |cx, v| cx.heap.is_real(v));
predicate!(is_rational, |cx, v| cx.heap.is_rational(v));
predicate!(is_integer, |cx, v| cx.heap.is_integer(v));
predicate!(is_exact_integer, |cx, v| cx.heap.is_exact_integer(v));

fn round_with(cx: &mut Cx, subr: &str, mode: Rounding, args: &[Value]) -> R {
    let r = cx.heap.round(mode, args[0]);
    num(r, cx, subr, args)
}

fn floor(cx: &mut Cx, args: &[Value]) -> R {
    round_with(cx, "floor", Rounding::Floor, args)
}
fn ceiling(cx: &mut Cx, args: &[Value]) -> R {
    round_with(cx, "ceiling", Rounding::Ceiling, args)
}
fn truncate(cx: &mut Cx, args: &[Value]) -> R {
    round_with(cx, "truncate", Rounding::Truncate, args)
}
fn round(cx: &mut Cx, args: &[Value]) -> R {
    round_with(cx, "round", Rounding::Round, args)
}

fn atan(cx: &mut Cx, args: &[Value]) -> R {
    let r = match args {
        [y, x] => cx.heap.atan2(*y, *x),
        _ => cx.heap.atan(args[0]),
    };
    num(r, cx, "atan", args)
}

fn number_to_string(cx: &mut Cx, args: &[Value]) -> R {
    let radix = radix(cx, args, 1, 36)?;
    if radix == 10
        && let Some(n) = args[0].as_fixnum()
    {
        let mut out = String::new();
        let _ = write!(out, "{n}");
        return Ok(cx.heap.string(out));
    }
    match cx.heap.number_to_string(args[0], radix) {
        Ok(text) => Ok(cx.heap.string(text)),
        Err(e) => Err(cx.num_error("number->string", e, args)),
    }
}

fn string_to_number(cx: &mut Cx, args: &[Value]) -> R {
    let s = text(cx, "string->number", 1, args[0])?;
    let radix = radix(cx, args, 1, i32::MAX as u64)?;
    if radix > 36 {
        return Ok(Value::FALSE);
    }
    let r = cx.heap.string_to_number(&s, radix);
    num(r, cx, "string->number", args)
}

// --- equivalence, pairs and lists -------------------------------------------------------

predicate!(not, |cx, v| v.is_false());
predicate!(is_boolean, |cx, v| v == Value::TRUE || v == Value::FALSE);
// Guile's `unspecified?`, which `c++.scm`'s `void?` calls.
predicate!(is_unspecified, |cx, v| v == Value::UNSPECIFIED);
predicate!(is_pair, |cx, v| v.tag() == Tag::Pair);
predicate!(is_null, |cx, v| v == Value::NULL);
predicate!(is_symbol, |cx, v| v.tag() == Tag::Symbol);
predicate!(is_keyword, |cx, v| v.tag() == Tag::Keyword);
predicate!(is_char, |cx, v| v.tag() == Tag::Char);
predicate!(is_string, |cx, v| v.tag() == Tag::String);
predicate!(is_vector, |cx, v| v.tag() == Tag::Vector);
predicate!(is_procedure, |cx, v| procedure(&cx.heap, v));

/// Whether `v` applies: a primitive, a closure, a parameter or a procedure
/// with a setter.
pub(crate) fn procedure(heap: &crate::heap::Heap, v: Value) -> bool {
    matches!(v.tag(), Tag::Primitive | Tag::Closure)
        || control::parameter_procedure(heap, v).is_some()
        || hash::with_setter_procedure(heap, v).is_some()
        || heap
            .foreign_object(v)
            .and_then(|f| f.applicable())
            .is_some()
}

/// `(%procedure-name p)`: the name its code or table gives it, else `#f`.
fn procedure_name(cx: &mut Cx, args: &[Value]) -> R {
    let mut p = args[0];
    if let Some(inner) = hash::with_setter_procedure(&cx.heap, p) {
        p = inner;
    }
    Ok(match p.tag() {
        Tag::Primitive => {
            let name = get(&cx.heap, p.payload()).map_or("", |p| p.name);
            cx.heap.intern(name)
        }
        Tag::Closure => cx
            .heap
            .closure_code(p)
            .map(|code| code.name)
            .filter(|name| name.tag() == Tag::Symbol)
            .unwrap_or(Value::FALSE),
        _ => Value::FALSE,
    })
}

fn identity(_: &mut Cx, args: &[Value]) -> R {
    Ok(args[0])
}

/// `(->bool x)`: `#f` for `#f`, else `#t`, as `boot-9.scm`'s
/// `(not (not x))` (MS3 step 49: output-lib.scm's
/// `ly:spanner::calc-connect-to-neighbors` calls it).
fn to_bool(_: &mut Cx, args: &[Value]) -> R {
    Ok(if args[0] == Value::FALSE {
        Value::FALSE
    } else {
        Value::TRUE
    })
}

fn is_list(cx: &mut Cx, args: &[Value]) -> R {
    // Floyd's: a cycle is not a list.
    let (mut slow, mut fast) = (args[0], args[0]);
    loop {
        for _ in 0..2 {
            if fast == Value::NULL {
                return Ok(Value::TRUE);
            }
            match cx.heap.cdr(fast) {
                Some(next) => fast = next,
                None => return Ok(Value::FALSE),
            }
        }
        slow = cx.heap.cdr(slow).unwrap_or(Value::NULL);
        if slow == fast {
            return Ok(Value::FALSE);
        }
    }
}

/// `eq?`, `eqv?` and `equal?` take any number of arguments, as Guile 3's
/// do: whether each is the same as the next.
fn is_eq(_: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(args.windows(2).all(|w| w[0] == w[1])))
}

fn is_eqv(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        args.windows(2).all(|w| cx.heap.eqv(w[0], w[1])),
    ))
}

fn is_equal(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        args.windows(2).all(|w| cx.heap.equal(w[0], w[1])),
    ))
}

fn cons(cx: &mut Cx, args: &[Value]) -> R {
    Ok(cx.heap.cons(args[0], args[1]))
}

/// `car` or `cdr`: the procedure's error, or compiled code's for a direct
/// call.
fn car_or_cdr(cx: &mut Cx, subr: &str, v: Value) -> Result<(Value, Value), Thrown> {
    match (cx.heap.car(v), cx.heap.cdr(v)) {
        (Some(car), Some(cdr)) => Ok((car, cdr)),
        _ if cx.direct => Err(cx.wrong_type_compiled(subr, 1, v, "pair")),
        _ => Err(cx.wrong_type_vm(Some(subr), v, "pair")),
    }
}

fn car(cx: &mut Cx, args: &[Value]) -> R {
    car_or_cdr(cx, "car", args[0]).map(|p| p.0)
}

fn cdr(cx: &mut Cx, args: &[Value]) -> R {
    car_or_cdr(cx, "cdr", args[0]).map(|p| p.1)
}

/// `c[ad]+r`: the letters between `c` and `r`, applied from the right.
/// Guile's are Scheme, so their errors are the VM's, about the value that
/// was not a pair.
macro_rules! cxr {
    ($f:ident, $name:literal) => {
        fn $f(cx: &mut Cx, args: &[Value]) -> R {
            let mut v = args[0];
            for step in $name[1..$name.len() - 1].bytes().rev() {
                let (car, cdr) = match (cx.heap.car(v), cx.heap.cdr(v)) {
                    (Some(car), Some(cdr)) => (car, cdr),
                    _ => return Err(cx.wrong_type_vm(Some($name), v, "pair")),
                };
                v = if step == b'a' { car } else { cdr };
            }
            Ok(v)
        }
    };
}

cxr!(caar, "caar");
cxr!(cadr, "cadr");
cxr!(cdar, "cdar");
cxr!(cddr, "cddr");
cxr!(caddr, "caddr");
cxr!(cdddr, "cdddr");
cxr!(caadr, "caadr");
cxr!(cddar, "cddar");
cxr!(cdadr, "cdadr");
cxr!(cadar, "cadar");
cxr!(cadddr, "cadddr");
cxr!(caaar, "caaar");
cxr!(cdaar, "cdaar");
cxr!(caaaar, "caaaar");
cxr!(caaadr, "caaadr");
cxr!(caadar, "caadar");
cxr!(caaddr, "caaddr");
cxr!(cadaar, "cadaar");
cxr!(cadadr, "cadadr");
cxr!(caddar, "caddar");
cxr!(cdaaar, "cdaaar");
cxr!(cdaadr, "cdaadr");
cxr!(cdadar, "cdadar");
cxr!(cdaddr, "cdaddr");
cxr!(cddaar, "cddaar");
cxr!(cddadr, "cddadr");
cxr!(cdddar, "cdddar");
cxr!(cddddr, "cddddr");

fn set_car(cx: &mut Cx, args: &[Value]) -> R {
    if !cx.heap.set_car(args[0], args[1]) {
        return Err(cx.wrong_type("set-car!", 1, args[0], "mutable pair"));
    }
    Ok(Value::UNSPECIFIED)
}

fn set_cdr(cx: &mut Cx, args: &[Value]) -> R {
    if !cx.heap.set_cdr(args[0], args[1]) {
        return Err(cx.wrong_type("set-cdr!", 1, args[0], "mutable pair"));
    }
    Ok(Value::UNSPECIFIED)
}

fn list(cx: &mut Cx, args: &[Value]) -> R {
    Ok(cx.heap.list(args))
}

fn cons_star(cx: &mut Cx, args: &[Value]) -> R {
    let (&last, init) = args.split_last().unwrap_or((&Value::NULL, &[]));
    Ok(init
        .iter()
        .rev()
        .fold(last, |tail, &item| cx.heap.cons(item, tail)))
}

fn length(cx: &mut Cx, args: &[Value]) -> R {
    if is_list(cx, args)? == Value::FALSE {
        return Err(cx.wrong_type_at("length", 1, args[0]));
    }
    let n = items(cx, "length", 1, args[0])?.len();
    Ok(cx.heap.integer(n as i64))
}

fn append(cx: &mut Cx, args: &[Value]) -> R {
    let Some((&last, init)) = args.split_last() else {
        return Ok(Value::NULL);
    };
    let mut all = Vec::new();
    for (i, &l) in init.iter().enumerate() {
        let (items, tail) = walk(cx, l);
        if tail != Value::NULL {
            return Err(cx.wrong_type("append", i + 1, tail, "empty list"));
        }
        all.extend(items);
    }
    Ok(all
        .into_iter()
        .rev()
        .fold(last, |tail, item| cx.heap.cons(item, tail)))
}

fn reverse(cx: &mut Cx, args: &[Value]) -> R {
    let items = items(cx, "reverse", 1, args[0])?;
    Ok(items
        .into_iter()
        .fold(Value::NULL, |tail, item| cx.heap.cons(item, tail)))
}

fn list_tail(cx: &mut Cx, args: &[Value]) -> R {
    let k = cx.size(args[1], SIZE_MAX)?;
    let mut at = args[0];
    for _ in 0..k {
        at = pair(cx, "list-tail", 1, at)?.1;
    }
    Ok(at)
}

fn list_ref(cx: &mut Cx, args: &[Value]) -> R {
    let k = cx.size(args[1], SIZE_MAX)?;
    let mut at = args[0];
    for _ in 0..k {
        match cx.heap.cdr(at) {
            Some(next) => at = next,
            None => return Err(cx.out_of_range("list-ref", 2, args[1])),
        }
    }
    match cx.heap.car(at) {
        Some(v) => Ok(v),
        None => Err(cx.out_of_range("list-ref", 2, args[1])),
    }
}

fn list_copy(cx: &mut Cx, args: &[Value]) -> R {
    let items = items(cx, "list-copy", 1, args[0])?;
    Ok(cx.heap.list(&items))
}

fn last_pair(cx: &mut Cx, args: &[Value]) -> R {
    if args[0] == Value::NULL {
        return Ok(Value::NULL);
    }
    pair(cx, "last-pair", 1, args[0])?;
    let mut at = args[0];
    while let Some(next) = cx.heap.cdr(at).filter(|n| n.tag() == Tag::Pair) {
        at = next;
    }
    Ok(at)
}

/// `memq`, `memv`, `member`: the first tail whose car is `same` to `x`.
/// `member` does not say what it expected.
fn mem(
    cx: &mut Cx,
    subr: &str,
    args: &[Value],
    same: fn(&crate::heap::Heap, Value, Value) -> bool,
) -> R {
    let mut at = args[1];
    while let (Some(car), Some(cdr)) = (cx.heap.car(at), cx.heap.cdr(at)) {
        if same(&cx.heap, args[0], car) {
            return Ok(at);
        }
        at = cdr;
    }
    match (at, subr) {
        (Value::NULL, _) => Ok(Value::FALSE),
        (_, "member") => Err(cx.wrong_type_at(subr, 2, args[1])),
        _ => Err(cx.wrong_type(subr, 2, args[1], "list")),
    }
}

/// `assq`, `assv`, `assoc`: the first pair of the alist whose car is `same`.
/// All three say `assq` of a list that is no alist, as Guile's do.
fn ass(cx: &mut Cx, args: &[Value], same: fn(&crate::heap::Heap, Value, Value) -> bool) -> R {
    let mut at = args[1];
    while let (Some(entry), Some(cdr)) = (cx.heap.car(at), cx.heap.cdr(at)) {
        match cx.heap.car(entry) {
            Some(key) if same(&cx.heap, args[0], key) => return Ok(entry),
            Some(_) => {}
            None => break,
        }
        at = cdr;
    }
    match at {
        Value::NULL => Ok(Value::FALSE),
        _ => Err(cx.wrong_type("assq", 2, args[1], "association list")),
    }
}

fn memq(cx: &mut Cx, args: &[Value]) -> R {
    mem(cx, "memq", args, |_, a, b| a == b)
}
fn memv(cx: &mut Cx, args: &[Value]) -> R {
    mem(cx, "memv", args, |h, a, b| h.eqv(a, b))
}
fn member(cx: &mut Cx, args: &[Value]) -> R {
    mem(cx, "member", args, |h, a, b| h.equal(a, b))
}
fn assq(cx: &mut Cx, args: &[Value]) -> R {
    ass(cx, args, |_, a, b| a == b)
}
fn assv(cx: &mut Cx, args: &[Value]) -> R {
    ass(cx, args, |h, a, b| h.eqv(a, b))
}
fn assoc(cx: &mut Cx, args: &[Value]) -> R {
    ass(cx, args, |h, a, b| h.equal(a, b))
}

// --- symbols, characters and strings -----------------------------------------------------

fn symbol_to_string(cx: &mut Cx, args: &[Value]) -> R {
    match cx
        .heap
        .name(args[0])
        .filter(|_| args[0].tag() == Tag::Symbol)
    {
        Some(name) => {
            let name = String::from(name);
            Ok(cx.heap.string(name))
        }
        None => Err(cx.wrong_type("symbol->string", 1, args[0], "symbol")),
    }
}

fn string_to_symbol(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.intern_string(args[0]) {
        Some(symbol) => Ok(symbol),
        None => Err(cx.wrong_type("string->symbol", 1, args[0], "string")),
    }
}

fn char_to_integer(cx: &mut Cx, args: &[Value]) -> R {
    let c = character(cx, "char->integer", 1, args[0])?;
    Ok(cx.heap.integer(c as i64))
}

fn integer_to_char(cx: &mut Cx, args: &[Value]) -> R {
    let v = args[0];
    let n = match cx.heap.to_i64(v) {
        Some(n) if (0..=0x10ffff).contains(&n) => n,
        Some(n) if n < 0 => return Err(cx.value_out_of_range(Some("integer->char"), v)),
        _ => return bounded(cx, v, -1, 0x10ffff).map(|_| Value::UNSPECIFIED),
    };
    match char::from_u32(n as u32) {
        Some(c) => Ok(Value::char(c)),
        None => Err(cx.value_out_of_range(Some("integer->char"), v)),
    }
}

/// Unicode's simple case mapping, as Guile's `char-upcase`: a character
/// whose mapping is longer stays.
fn simple(mut chars: impl Iterator<Item = char>, c: char) -> char {
    match (chars.next(), chars.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

fn char_upcase(cx: &mut Cx, args: &[Value]) -> R {
    let c = character(cx, "char-upcase", 1, args[0])?;
    Ok(Value::char(simple(c.to_uppercase(), c)))
}

fn char_downcase(cx: &mut Cx, args: &[Value]) -> R {
    let c = character(cx, "char-downcase", 1, args[0])?;
    Ok(Value::char(simple(c.to_lowercase(), c)))
}

fn string_length(cx: &mut Cx, args: &[Value]) -> R {
    match cx.heap.str(args[0]) {
        Some(s) => Ok(cx.heap.integer(s.len() as i64)),
        None => Err(cx.wrong_type("string-length", 1, args[0], "string")),
    }
}

fn string_ref(cx: &mut Cx, args: &[Value]) -> R {
    let Some(len) = cx.heap.str(args[0]).map(|s| s.len()) else {
        return Err(cx.wrong_type("string-ref", 1, args[0], "string"));
    };
    let k = string_index(cx, len, args[1])?;
    match cx.heap.str(args[0]).and_then(|s| s.char_at(k)) {
        Some(c) => Ok(Value::char(c)),
        None => Err(cx.value_out_of_range(None, args[1])),
    }
}

fn string_set(cx: &mut Cx, args: &[Value]) -> R {
    let Some(len) = cx.heap.str(args[0]).map(|s| s.len()) else {
        return Err(cx.wrong_type("string-set!", 1, args[0], "string"));
    };
    let k = string_index(cx, len, args[1])?;
    let c = character(cx, "string-set!", 3, args[2])?;
    match cx.heap.str_mut(args[0]).map(|s| s.set_char(k, c)) {
        Some(true) => Ok(Value::UNSPECIFIED),
        _ => Err(cx.value_out_of_range(None, args[1])),
    }
}

fn substring(cx: &mut Cx, args: &[Value]) -> R {
    let Some(len) = cx.heap.str(args[0]).map(|s| s.len()) else {
        return Err(cx.wrong_type("substring", 1, args[0], "string"));
    };
    let start = bounded(cx, args[1], 0, len as u64)?;
    let end = match args.get(2) {
        Some(&e) => bounded(cx, e, start as i64, len as u64)?,
        None => len,
    };
    let piece = cx.heap.str(args[0]).and_then(|s| {
        let (a, b) = (s.byte_offset(start)?, s.byte_offset(end)?);
        Some(String::from(&s.as_str()[a..b]))
    });
    Ok(cx.heap.string(piece.unwrap_or_default()))
}

fn string_append(cx: &mut Cx, args: &[Value]) -> R {
    let mut out = String::new();
    for &s in args {
        match cx.heap.str(s) {
            Some(s) => out.push_str(s.as_str()),
            None => return Err(cx.wrong_type_vm(Some("string-append"), s, "string")),
        }
    }
    Ok(cx.heap.string(out))
}

fn string_to_list(cx: &mut Cx, args: &[Value]) -> R {
    let s = text(cx, "string->list", 1, args[0])?;
    Ok(s.chars()
        .rev()
        .fold(Value::NULL, |tail, c| cx.heap.cons(Value::char(c), tail)))
}

/// `string` and `list->string`, which Guile's `string` makes.
fn chars_to_string(cx: &mut Cx, chars: &[Value]) -> R {
    let mut out = String::new();
    for &c in chars {
        match c.as_char() {
            Some(c) => out.push(c),
            None => return Err(cx.wrong_type_vm(Some("string"), c, "character")),
        }
    }
    Ok(cx.heap.string(out))
}

fn list_to_string(cx: &mut Cx, args: &[Value]) -> R {
    let chars = items(cx, "string", 1, args[0])?;
    chars_to_string(cx, &chars)
}

fn string(cx: &mut Cx, args: &[Value]) -> R {
    chars_to_string(cx, args)
}

fn make_string(cx: &mut Cx, args: &[Value]) -> R {
    let n = cx.size(args[0], SIZE_MAX)?;
    let c = match args.get(1) {
        Some(&c) => match c.as_char() {
            Some(c) => c,
            None => return Err(cx.wrong_type_vm(None, c, "character")),
        },
        None => ' ',
    };
    Ok(cx
        .heap
        .string(core::iter::repeat_n(c, n).collect::<String>()))
}

// --- vectors ----------------------------------------------------------------------------

fn vector(cx: &mut Cx, args: &[Value]) -> R {
    Ok(cx.heap.vector(args.to_vec()))
}

fn make_vector(cx: &mut Cx, args: &[Value]) -> R {
    let n = cx.size(args[0], MAX_VECTOR_LENGTH)?;
    let fill = args.get(1).copied().unwrap_or(Value::UNSPECIFIED);
    Ok(cx.heap.vector(alloc::vec![fill; n]))
}

/// A vector's length; not a vector is the procedure's error, or compiled
/// code's for a direct call.
fn vector_of(cx: &mut Cx, subr: &str, v: Value) -> Result<usize, Thrown> {
    match cx.heap.vector_items(v) {
        Some(items) => Ok(items.len()),
        None if cx.direct => Err(cx.wrong_type_compiled(subr, 1, v, "vector")),
        None => Err(cx.wrong_type_at(subr, 1, v)),
    }
}

fn vector_length(cx: &mut Cx, args: &[Value]) -> R {
    let n = vector_of(cx, "vector-length", args[0])?;
    Ok(cx.heap.integer(n as i64))
}

fn vector_ref(cx: &mut Cx, args: &[Value]) -> R {
    let len = vector_of(cx, "vector-ref", args[0])?;
    let k = vector_index(cx, "vector-ref", len, args[1])?;
    Ok(cx
        .heap
        .vector_items(args[0])
        .map_or(Value::UNSPECIFIED, |v| v[k]))
}

fn vector_set(cx: &mut Cx, args: &[Value]) -> R {
    if cx.direct && cx.heap.vector_items(args[0]).is_none() {
        return Err(cx.wrong_type_compiled("vector-set!", 1, args[0], "mutable vector"));
    }
    let len = vector_len(cx, "vector-set!", 1, args[0])?;
    let k = vector_index(cx, "vector-set!", len, args[1])?;
    if let Some(items) = cx.heap.vector_items_mut(args[0]) {
        items[k] = args[2];
    }
    Ok(Value::UNSPECIFIED)
}

fn vector_to_list(cx: &mut Cx, args: &[Value]) -> R {
    let Some(items) = cx.heap.vector_items(args[0]).map(<[Value]>::to_vec) else {
        return Err(cx.wrong_type_vm(None, args[0], "array"));
    };
    Ok(cx.heap.list(&items))
}

fn list_to_vector(cx: &mut Cx, args: &[Value]) -> R {
    let items = items(cx, "vector", 1, args[0])?;
    Ok(cx.heap.vector(items))
}

fn vector_fill(cx: &mut Cx, args: &[Value]) -> R {
    vector_len(cx, "vector-fill!", 1, args[0])?;
    if let Some(items) = cx.heap.vector_items_mut(args[0]) {
        items.fill(args[1]);
    }
    Ok(Value::UNSPECIFIED)
}

// --- control and output -------------------------------------------------------------------

/// Spread by the VM before it gets here.
fn apply(_: &mut Cx, _: &[Value]) -> R {
    Ok(Value::UNSPECIFIED)
}

fn values(cx: &mut Cx, args: &[Value]) -> R {
    Ok(match args {
        [one] => *one,
        _ => cx.heap.values(args.to_vec()),
    })
}

/// Compiled and called by the VM before it gets here.
fn eval(_: &mut Cx, _: &[Value]) -> R {
    Ok(Value::UNSPECIFIED)
}

/// Collected by the VM before it gets here.
fn gc(_: &mut Cx, _: &[Value]) -> R {
    Ok(Value::UNSPECIFIED)
}

// --- the table ---------------------------------------------------------------------------

const fn p(name: &'static str, min: u8, max: u8, f: PrimFn) -> Prim {
    Prim { name, min, max, f }
}

/// Every primitive, by the index a [`Tag::Primitive`] value holds. `apply`
/// is first ([`APPLY`]), then those the VM runs itself.
pub(crate) static PRIMS: &[Prim] = &[
    p("apply", 2, REST, apply),
    p("eval", 2, 2, eval),
    p("primitive-eval", 1, 1, eval),
    p("gc", 0, 0, gc),
    p("values", 0, REST, values),
    p("procedure?", 1, 1, is_procedure),
    // control
    p("throw", 1, REST, control::throw),
    p("scm-error", 5, 5, control::scm_error),
    p("%abort", 2, 2, control::abort),
    p("%resume", 1, 1, control::resume),
    p("make-fluid", 0, 1, control::make_fluid),
    p("fluid?", 1, 1, control::is_fluid),
    p("fluid-ref", 1, 1, control::fluid_ref),
    p("fluid-set!", 2, 2, control::fluid_set),
    p("%check-fluid", 1, 1, control::check_fluid),
    p("%make-parameter", 3, 3, control::make_parameter),
    p("parameter?", 1, 1, control::is_parameter),
    p("make-promise", 1, 1, control::make_promise),
    p("promise?", 1, 1, control::is_promise),
    p("%promise-data", 1, 1, control::promise_data),
    p("%promise-computed?", 1, 1, control::promise_computed),
    p("%promise-set!", 2, 2, control::promise_set),
    p("%parameter-parts", 1, 1, control::parameter_parts),
    // modules
    p("interaction-environment", 0, 0, module::current_module),
    p("current-module", 0, 0, module::current_module),
    p("set-current-module", 1, 1, module::set_current_module),
    p("module?", 1, 1, module::is_module),
    p("module-name", 1, 1, module::module_name),
    p("module-define!", 3, 3, module::module_define),
    p("module-ref", 2, 3, module::module_ref),
    p("module-export!", 2, 2, module::module_export),
    p("defined?", 1, 2, module::is_defined),
    p("module-set!", 3, 3, module::module_set),
    p("module-remove!", 2, 2, module::module_remove),
    p("%module-local-names", 1, 1, module::module_local_names),
    p("%resolve-module", 2, 2, module::resolve_module),
    p("%module-forms", 1, 1, module::module_forms),
    p("%spec-module", 1, 1, module::spec_module),
    p("%use-specs!", 1, 1, module::use_specs),
    p("%option-modules", 1, 1, module::option_modules),
    p(
        "%process-define-module",
        2,
        2,
        module::process_define_module,
    ),
    p("%export", 1, 2, module::export),
    p("%re-export", 1, 1, module::re_export),
    p("%module-at", 3, 3, module::module_at),
    // numbers
    p("+", 0, REST, add),
    p("-", 1, REST, sub),
    p("*", 0, REST, mul),
    p("/", 1, REST, div),
    p("=", 1, REST, num_eq),
    p("<", 1, REST, lt),
    p(">", 1, REST, gt),
    p("<=", 1, REST, le),
    p(">=", 1, REST, ge),
    p("max", 1, REST, max),
    p("min", 1, REST, min),
    p("quotient", 2, 2, quotient),
    p("remainder", 2, 2, remainder),
    p("modulo", 2, 2, modulo),
    p("abs", 1, 1, abs),
    p("1+", 1, 1, one_plus),
    p("1-", 1, 1, one_minus),
    p("gcd", 0, REST, gcd),
    p("lcm", 0, REST, lcm),
    p("expt", 2, 2, expt),
    p("sqrt", 1, 1, sqrt),
    p("exp", 1, 1, exp),
    p("log", 1, 1, log),
    p("sin", 1, 1, sin),
    p("cos", 1, 1, cos),
    p("tan", 1, 1, tan),
    p("atan", 1, 2, atan),
    p("floor", 1, 1, floor),
    p("ceiling", 1, 1, ceiling),
    p("truncate", 1, 1, truncate),
    p("round", 1, 1, round),
    p("numerator", 1, 1, numerator),
    p("denominator", 1, 1, denominator),
    p("exact", 1, 1, exact),
    p("inexact", 1, 1, inexact),
    p("inexact->exact", 1, 1, exact),
    p("exact->inexact", 1, 1, inexact),
    p("number?", 1, 1, is_number),
    p("complex?", 1, 1, is_number),
    p("real?", 1, 1, is_real),
    p("rational?", 1, 1, is_rational),
    p("integer?", 1, 1, is_integer),
    p("exact-integer?", 1, 1, is_exact_integer),
    p("exact?", 1, 1, is_exact),
    p("inexact?", 1, 1, is_inexact),
    p("zero?", 1, 1, is_zero),
    p("positive?", 1, 1, is_positive),
    p("negative?", 1, 1, is_negative),
    p("odd?", 1, 1, is_odd),
    p("even?", 1, 1, is_even),
    p("asin", 1, 1, asin),
    p("acos", 1, 1, acos),
    p("make-rectangular", 2, 2, make_rectangular),
    p("make-polar", 2, 2, make_polar),
    p("real-part", 1, 1, real_part),
    p("imag-part", 1, 1, imag_part),
    p("magnitude", 1, 1, magnitude),
    p("angle", 1, 1, angle),
    p("nan?", 1, 1, is_nan),
    p("inf?", 1, 1, is_inf),
    p("finite?", 1, 1, is_finite),
    p("ash", 2, 2, ash),
    p("logand", 0, REST, logand),
    p("logior", 0, REST, logior),
    p("logxor", 0, REST, logxor),
    p("lognot", 1, 1, lognot),
    p("logtest", 2, 2, logtest),
    p("logbit?", 2, 2, logbit),
    p("logcount", 1, 1, logcount),
    p("integer-length", 1, 1, integer_length),
    p("exact-integer-sqrt", 1, 1, exact_integer_sqrt),
    p("floor/", 2, 2, floor_div),
    p("floor-quotient", 2, 2, floor_quotient),
    p("floor-remainder", 2, 2, floor_remainder),
    p("ceiling/", 2, 2, ceiling_div),
    p("ceiling-quotient", 2, 2, ceiling_quotient),
    p("ceiling-remainder", 2, 2, ceiling_remainder),
    p("truncate/", 2, 2, truncate_div),
    p("truncate-quotient", 2, 2, truncate_quotient),
    p("truncate-remainder", 2, 2, truncate_remainder),
    p("round/", 2, 2, round_div),
    p("round-quotient", 2, 2, round_quotient),
    p("round-remainder", 2, 2, round_remainder),
    p("euclidean/", 2, 2, euclidean_div),
    p("euclidean-quotient", 2, 2, euclidean_quotient),
    p("euclidean-remainder", 2, 2, euclidean_remainder),
    p("centered/", 2, 2, centered_div),
    p("centered-quotient", 2, 2, centered_quotient),
    p("centered-remainder", 2, 2, centered_remainder),
    p("number->string", 1, 2, number_to_string),
    p("string->number", 1, 2, string_to_number),
    // equivalence, pairs and lists
    p("not", 1, 1, not),
    p("boolean?", 1, 1, is_boolean),
    p("unspecified?", 1, 1, is_unspecified),
    p("eq?", 0, REST, is_eq),
    p("eqv?", 0, REST, is_eqv),
    p("equal?", 0, REST, is_equal),
    p("pair?", 1, 1, is_pair),
    p("null?", 1, 1, is_null),
    p("list?", 1, 1, is_list),
    p("cons", 2, 2, cons),
    p("car", 1, 1, car),
    p("cdr", 1, 1, cdr),
    p("caar", 1, 1, caar),
    p("cadr", 1, 1, cadr),
    p("cdar", 1, 1, cdar),
    p("cddr", 1, 1, cddr),
    p("caddr", 1, 1, caddr),
    p("cdddr", 1, 1, cdddr),
    p("caadr", 1, 1, caadr),
    p("cddar", 1, 1, cddar),
    p("cdadr", 1, 1, cdadr),
    p("cadar", 1, 1, cadar),
    p("cadddr", 1, 1, cadddr),
    p("caaar", 1, 1, caaar),
    p("cdaar", 1, 1, cdaar),
    p("caaaar", 1, 1, caaaar),
    p("caaadr", 1, 1, caaadr),
    p("caadar", 1, 1, caadar),
    p("caaddr", 1, 1, caaddr),
    p("cadaar", 1, 1, cadaar),
    p("cadadr", 1, 1, cadadr),
    p("caddar", 1, 1, caddar),
    p("cdaaar", 1, 1, cdaaar),
    p("cdaadr", 1, 1, cdaadr),
    p("cdadar", 1, 1, cdadar),
    p("cdaddr", 1, 1, cdaddr),
    p("cddaar", 1, 1, cddaar),
    p("cddadr", 1, 1, cddadr),
    p("cdddar", 1, 1, cdddar),
    p("cddddr", 1, 1, cddddr),
    p("set-car!", 2, 2, set_car),
    p("set-cdr!", 2, 2, set_cdr),
    p("list", 0, REST, list),
    p("cons*", 1, REST, cons_star),
    p("length", 1, 1, length),
    p("append", 0, REST, append),
    p("reverse", 1, 1, reverse),
    p("list-tail", 2, 2, list_tail),
    p("list-ref", 2, 2, list_ref),
    p("list-copy", 1, 1, list_copy),
    p("last-pair", 1, 1, last_pair),
    p("memq", 2, 2, memq),
    p("memv", 2, 2, memv),
    p("member", 2, 2, member),
    p("assq", 2, 2, assq),
    p("assv", 2, 2, assv),
    p("assoc", 2, 2, assoc),
    p("identity", 1, 1, identity),
    p("->bool", 1, 1, to_bool),
    p("acons", 3, 3, lists::acons),
    p("assq-ref", 2, 2, lists::assq_ref),
    p("assv-ref", 2, 2, lists::assv_ref),
    p("assoc-ref", 2, 2, lists::assoc_ref),
    p("assq-set!", 3, 3, lists::assq_set),
    p("assv-set!", 3, 3, lists::assv_set),
    p("assoc-set!", 3, 3, lists::assoc_set),
    p("assq-remove!", 2, 2, lists::assq_remove),
    p("assv-remove!", 2, 2, lists::assv_remove),
    p("assoc-remove!", 2, 2, lists::assoc_remove),
    p("delq", 2, 2, lists::delq),
    p("delv", 2, 2, lists::delv),
    p("delete", 2, 2, lists::delete),
    p("delq!", 2, 2, lists::delq_bang),
    p("delv!", 2, 2, lists::delv_bang),
    p("delete!", 2, 2, lists::delete_bang),
    p("delq1!", 2, 2, lists::delq1_bang),
    p("delv1!", 2, 2, lists::delv1_bang),
    p("delete1!", 2, 2, lists::delete1_bang),
    p("iota", 1, 3, lists::iota),
    p("make-list", 1, 2, lists::make_list),
    p("list-head", 2, 2, lists::list_head),
    p("list-set!", 3, 3, lists::list_set),
    p("reverse!", 1, 2, lists::reverse_bang),
    p("append!", 0, REST, lists::append_bang),
    // (srfi srfi-1)'s, lists::SRFI_1
    p("take", 2, 2, lists::take),
    p("drop", 2, 2, lists::drop),
    p("take-right", 2, 2, lists::take_right),
    p("drop-right", 2, 2, lists::drop_right),
    p("last", 1, 1, lists::last),
    p("length+", 1, 1, lists::length_plus),
    p("concatenate", 1, 1, lists::concatenate),
    p("append-reverse", 2, 2, lists::append_reverse),
    // symbols, characters and strings
    p("symbol?", 1, 1, is_symbol),
    p("keyword?", 1, 1, is_keyword),
    p("keyword->symbol", 1, 1, strings::keyword_to_symbol),
    p("symbol->keyword", 1, 1, strings::symbol_to_keyword),
    p("symbol", 0, REST, strings::symbol),
    p("symbol-append", 0, REST, strings::symbol_append),
    p("gensym", 0, 1, strings::gensym),
    p("make-symbol", 1, 1, strings::make_symbol),
    p("symbol->string", 1, 1, symbol_to_string),
    p("string->symbol", 1, 1, string_to_symbol),
    p("char?", 1, 1, is_char),
    p("char->integer", 1, 1, char_to_integer),
    p("integer->char", 1, 1, integer_to_char),
    p("char-upcase", 1, 1, char_upcase),
    p("char-downcase", 1, 1, char_downcase),
    p("char=?", 1, REST, strings::char_eq),
    p("char<?", 1, REST, strings::char_lt),
    p("char>?", 1, REST, strings::char_gt),
    p("char<=?", 1, REST, strings::char_le),
    p("char>=?", 1, REST, strings::char_ge),
    p("char-ci=?", 1, REST, strings::char_ci_eq),
    p("char-ci<?", 1, REST, strings::char_ci_lt),
    p("char-ci>?", 1, REST, strings::char_ci_gt),
    p("char-ci<=?", 1, REST, strings::char_ci_le),
    p("char-ci>=?", 1, REST, strings::char_ci_ge),
    p("char-alphabetic?", 1, 1, strings::is_char_alphabetic),
    p("char-numeric?", 1, 1, strings::is_char_numeric),
    p("char-whitespace?", 1, 1, strings::is_char_whitespace),
    p("char-upper-case?", 1, 1, strings::is_char_upper_case),
    p("char-lower-case?", 1, 1, strings::is_char_lower_case),
    p("string?", 1, 1, is_string),
    p("string-length", 1, 1, string_length),
    p("string-ref", 2, 2, string_ref),
    p("string-set!", 3, 3, string_set),
    p("substring", 2, 3, substring),
    p("string-append", 0, REST, string_append),
    p("string=?", 1, REST, strings::string_eq_p),
    p("string<?", 1, REST, strings::string_lt_p),
    p("string>?", 1, REST, strings::string_gt_p),
    p("string<=?", 1, REST, strings::string_le_p),
    p("string>=?", 1, REST, strings::string_ge_p),
    p("string-ci=?", 1, REST, strings::string_ci_eq_p),
    p("string-ci<?", 1, REST, strings::string_ci_lt_p),
    p("string-ci>?", 1, REST, strings::string_ci_gt_p),
    p("string-ci<=?", 1, REST, strings::string_ci_le_p),
    p("string-ci>=?", 1, REST, strings::string_ci_ge_p),
    p("string=", 2, 6, strings::string_eq),
    p("string<>", 2, 6, strings::string_ne),
    p("string<", 2, 6, strings::string_lt),
    p("string>", 2, 6, strings::string_gt),
    p("string<=", 2, 6, strings::string_le),
    p("string>=", 2, 6, strings::string_ge),
    p("string-ci=", 2, 6, strings::string_ci_eq),
    p("string-ci<>", 2, 6, strings::string_ci_ne),
    p("string-ci<", 2, 6, strings::string_ci_lt),
    p("string-ci>", 2, 6, strings::string_ci_gt),
    p("string->list", 1, 1, string_to_list),
    p("list->string", 1, 1, list_to_string),
    p("string", 0, REST, string),
    p("make-string", 1, 2, make_string),
    p("string-copy", 1, 3, strings::string_copy),
    p("substring/shared", 2, 3, strings::string_copy),
    p("string-fill!", 2, 4, strings::string_fill),
    p("string-copy!", 3, 5, strings::string_copy_bang),
    p("string-null?", 1, 1, strings::is_string_null),
    p("string-join", 1, 3, strings::string_join),
    p("string-concatenate", 1, 1, strings::string_concatenate),
    p("string-contains", 2, 4, strings::string_contains),
    p("string-contains-ci", 2, 4, strings::string_contains_ci),
    p("string-prefix?", 2, 2, strings::string_prefix),
    p("string-suffix?", 2, 2, strings::string_suffix),
    p("string-prefix-ci?", 2, 2, strings::string_prefix_ci),
    p("string-suffix-ci?", 2, 2, strings::string_suffix_ci),
    p("string-upcase", 1, 3, strings::string_upcase),
    p("string-downcase", 1, 3, strings::string_downcase),
    p("string-capitalize", 1, 1, strings::string_titlecase),
    p("string-titlecase", 1, 1, strings::string_titlecase),
    p("string-reverse", 1, 1, strings::string_reverse),
    p("string-take", 2, 2, strings::string_take),
    p("string-drop", 2, 2, strings::string_drop),
    p("string-take-right", 2, 2, strings::string_take_right),
    p("string-drop-right", 2, 2, strings::string_drop_right),
    p("string-pad", 2, 3, strings::string_pad),
    p("string-pad-right", 2, 3, strings::string_pad_right),
    p(
        "reverse-list->string",
        1,
        1,
        strings::reverse_list_to_string,
    ),
    p("basename", 1, 2, strings::basename),
    p("dirname", 1, 1, strings::dirname),
    // vectors
    p("vector?", 1, 1, is_vector),
    p("vector", 0, REST, vector),
    p("make-vector", 1, 2, make_vector),
    p("vector-length", 1, 1, vector_length),
    p("vector-ref", 2, 2, vector_ref),
    p("vector-set!", 3, 3, vector_set),
    p("vector->list", 1, 1, vector_to_list),
    p("list->vector", 1, 1, list_to_vector),
    p("vector-fill!", 2, 2, vector_fill),
    // arrays (MS4 step 17, D229)
    p("make-array", 1, REST, array::make_array),
    p("array?", 1, 1, array::is_array),
    p("array-ref", 1, REST, array::array_ref),
    p("array-set!", 2, REST, array::array_set),
    p("array->list", 1, 1, array::array_to_list),
    p("array-shape", 1, 1, array::array_shape),
    p("array-dimensions", 1, 1, array::array_dimensions),
    p("array-rank", 1, 1, array::array_rank),
    p("%array-position", 2, 2, array::array_position),
    p("%make-shared-array", 4, 4, array::make_shared),
    p("transpose-array", 1, REST, array::transpose_array),
    p("array-cell-ref", 1, REST, array::array_cell_ref),
    p("array-copy!", 2, 2, array::array_copy_into),
    // (rnrs bytevectors)' and (ice-9 iconv)'s, array::BYTEVECTORS
    p("bytevector?", 1, 1, array::is_bytevector),
    p("bytevector-length", 1, 1, array::bytevector_length),
    p("bytevector-u8-ref", 2, 2, array::bytevector_u8_ref),
    p("bytevector->u8-list", 1, 1, array::bytevector_to_u8_list),
    p("string->bytevector", 2, 2, array::string_to_bytevector),
    // hash tables and procedures with setters
    p("make-hash-table", 0, 1, hash::make_hash_table),
    p(
        "make-weak-key-hash-table",
        0,
        1,
        hash::make_weak_key_hash_table,
    ),
    p("hash-table?", 1, 1, hash::is_hash_table),
    p("hash-ref", 2, 3, hash::hash_ref),
    p("hash-set!", 3, 3, hash::hash_set),
    p("hash-remove!", 2, 2, hash::hash_remove),
    p("hash-get-handle", 2, 2, hash::hash_get_handle),
    p("hash-create-handle!", 3, 3, hash::hash_create_handle),
    p("hashq-ref", 2, 3, hash::hashq_ref),
    p("hashq-set!", 3, 3, hash::hashq_set),
    p("hashq-remove!", 2, 2, hash::hashq_remove),
    p("hashq-get-handle", 2, 2, hash::hashq_get_handle),
    p("hashq-create-handle!", 3, 3, hash::hashq_create_handle),
    p("hashv-ref", 2, 3, hash::hashv_ref),
    p("hashv-set!", 3, 3, hash::hashv_set),
    p("hashv-remove!", 2, 2, hash::hashv_remove),
    p("hashv-get-handle", 2, 2, hash::hashv_get_handle),
    p("hashv-create-handle!", 3, 3, hash::hashv_create_handle),
    p("hash-clear!", 1, 1, hash::hash_clear),
    p("hash", 2, 2, hash::hash_fn),
    p("hashq", 2, 2, hash::hashq_fn),
    p("hashv", 2, 2, hash::hashv_fn),
    p("%hash-entries", 3, 3, hash::hash_entries),
    p("%hashx-bucket", 2, 2, hash::hashx_bucket),
    p("%hashx-bucket-set!", 3, 3, hash::hashx_bucket_set),
    p(
        "make-procedure-with-setter",
        2,
        2,
        hash::make_procedure_with_setter,
    ),
    p(
        "procedure-with-setter?",
        1,
        1,
        hash::is_procedure_with_setter,
    ),
    p("setter", 1, 1, hash::setter),
    p("%procedure-name", 1, 1, procedure_name),
    // ports and output
    p("display", 1, 2, ports::display),
    p("write", 1, 2, ports::write),
    p("newline", 0, 1, ports::newline),
    p("write-char", 1, 2, ports::write_char),
    p("force-output", 0, 1, ports::force_output),
    p("object->string", 1, 2, ports::object_to_string),
    p("open-output-string", 0, 0, ports::open_output_string),
    p("open-input-string", 1, 1, ports::open_input_string),
    p("get-output-string", 1, 1, ports::get_output_string),
    p("port?", 1, 1, ports::is_port),
    p("input-port?", 1, 1, ports::is_input_port),
    p("output-port?", 1, 1, ports::is_output_port),
    p("port-closed?", 1, 1, ports::is_port_closed),
    p("close-port", 1, 1, ports::close_port),
    p("close-input-port", 1, 1, ports::close_port),
    p("close-output-port", 1, 1, ports::close_port),
    p("read-char", 0, 1, ports::read_char),
    p("peek-char", 0, 1, ports::peek_char),
    p("char-ready?", 0, 1, ports::char_ready),
    p("eof-object?", 1, 1, ports::is_eof_object),
    p("read", 0, 1, ports::read_datum),
    p("read-hash-extend", 2, 2, ports::read_hash_extend),
    p("simple-format", 2, REST, format::simple_format),
    // (ice-9 format)'s
    p("format", 2, REST, format::format),
    p("gettext", 1, 3, lily::gettext),
    // the library's modules
    p("get-char", 1, 1, library::ports::get_char),
    p("lookahead-char", 1, 1, library::ports::lookahead_char),
    p("get-line", 1, 1, library::ports::get_line),
    p("get-string-all", 1, 1, library::ports::get_string_all),
    p("get-string-n", 2, 2, library::ports::get_string_n),
    p("put-char", 2, 2, library::ports::put_char),
    p("put-string", 2, 4, library::ports::put_string),
    p(
        "%open-bytevector-output-port",
        0,
        0,
        library::ports::open_bytevector_output_port,
    ),
    p("%take-bytevector", 1, 1, library::ports::take_bytevector),
    p("put-u8", 2, 2, library::ports::put_u8),
    p("put-bytevector", 2, 2, library::ports::put_bytevector),
    p("%pretty-print", 5, 5, library::pretty::pretty_print),
    p("%port-column", 1, 1, library::ports::port_column),
    p("array-copy", 1, 1, library::ports::array_copy),
    p("%make-class", 3, 3, goops::make_class),
    p("%make-instance", 2, 2, goops::make_instance),
    p("%class-thunks", 1, 1, goops::class_thunks),
    p("is-a?", 2, 2, goops::is_a),
    p("%subclass?", 2, 2, goops::is_subclass),
    p("class-name", 1, 1, goops::class_name),
    p("slot-ref", 2, 2, goops::slot_ref),
    p("slot-set!", 3, 3, goops::slot_set),
    p("slot-bound?", 2, 2, goops::is_slot_bound),
    // (lily)
    p("ly:assoc-get", 2, 4, lily::assoc_get),
    p("ly:chain-assoc-get", 2, 4, lily::chain_assoc_get),
    p("ly:version", 0, 0, lily::version),
    p(
        "ly:string-percent-encode",
        1,
        1,
        lily::string_percent_encode,
    ),
    p("ly:make-regex", 1, 1, regex::make_regex),
    p("ly:regex-quote", 1, 1, regex::regex_quote),
    p("%regex-exec", 4, 4, regex::regex_exec),
    p("ly:dir?", 1, 1, lily::is_dir),
    p("ly:dimension?", 1, 1, lily::is_dimension),
    p("ly:error", 1, REST, lily::error),
    p("%bound-locally?", 1, 1, lily::bound_locally),
    p("ly:module->alist", 1, 1, module::module_to_alist),
    p("ly:modules-lookup", 2, 3, module::modules_lookup),
    p(lily::UNITS[0].0, 1, 1, lily::pt),
    p(lily::UNITS[1].0, 1, 1, lily::mm),
    p(lily::UNITS[2].0, 1, 1, lily::cm),
    p(lily::UNITS[3].0, 1, 1, lily::inch),
    p(lily::UNITS[4].0, 1, 1, lily::bp),
    p(lily::NEVER[0], 1, 1, lily::never),
    p(lily::NEVER[1], 1, 1, lily::never),
    p(lily::NEVER[2], 1, 1, lily::never),
    p(lily::NEVER[3], 1, 1, lily::never),
    p(lily::NEVER[4], 1, 1, lily::never),
    p(lily::NEVER[5], 1, 1, lily::never),
    p(lily::NEVER[6], 1, 1, lily::never),
    p("%part-source", 1, 1, lily::part_source),
    p("%part-sources", 1, 1, lily::part_sources),
    p("%named-formals", 1, 1, lily::named_formals),
    p("%make-display-stub", 5, 5, lily::make_display_stub),
    p("%display-stub-target", 1, 1, lily::display_stub_target),
    p("%undo-suspend!", 0, 0, lily::undo_suspend),
    p("%undo-resume!", 1, 1, lily::undo_resume),
    p("%host-call", 1, REST, lily::host_call),
    // The `syntax-case` part's (D273), which wasm-split moves out of the core.
    p("%syntax-case-match", 3, 3, syntax_case::case_match),
    p("%syntax-case-fail", 1, 1, syntax_case::case_fail),
    p("%syntax", 2, REST, syntax_case::instantiate),
    p("syntax->datum", 1, 1, syntax_case::to_datum),
    p("datum->syntax", 2, 2, syntax_case::from_datum),
    p("identifier?", 1, 1, syntax_case::is_identifier),
    p("bound-identifier=?", 2, 2, syntax_case::bound_identifier_eq),
    p("free-identifier=?", 2, 2, syntax_case::free_identifier_eq),
    p(
        "generate-temporaries",
        1,
        1,
        syntax_case::generate_temporaries,
    ),
    p("syntax-violation", 3, 4, syntax_case::syntax_violation),
];
