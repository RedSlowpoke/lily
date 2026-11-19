//! LilyPond's `Duration` (`duration.cc`): a power of two, dots and a
//! factor. Its integers are C++ `int`s as LilyPond's are, its shifts as
//! LilyPond's compile on arm64 (D59), which takes the count modulo 32.

use alloc::string::String;
use core::any::Any;
use core::cmp::Ordering;
use core::fmt::Write;

use lily_engine_scheme::{Cx, Foreign, Heap, ImageReader, ImageWriter, Thrown, Value};

use crate::R;
use crate::rational::{Moment, Rational};

#[derive(Clone, Copy, Debug)]
pub struct Duration {
    log: i32,
    dots: i32,
    factor: Rational,
}

/// `intlog2`: the position of the highest bit; `d` must be positive.
fn intlog2(d: i128) -> i32 {
    127 - d.leading_zeros() as i32
}

impl Duration {
    /// `Duration (int, int)`: `log` 2 and no dots is a quarter.
    pub fn new(log: i32, dots: i32) -> Duration {
        Duration {
            log,
            dots,
            factor: Rational::new(1, 1),
        }
    }

    /// `Duration (Rational, bool)`: the duration of `length` whole notes,
    /// with the longest power of two and the most dots that fit; with
    /// `scale`, or below a 64th, a factor makes up the rest.
    pub fn from_length(length: Rational, scale: bool) -> Option<Duration> {
        let (sign, r) = if length.is_negative() {
            (Rational::int(-1), -length)
        } else {
            (Rational::int(1), length)
        };
        let mut d = Duration {
            log: 0,
            dots: 0,
            factor: sign,
        };
        if !r.is_finite() || r.is_zero() {
            d.factor = d.factor.checked_mul(r)?;
            return Some(d);
        }
        // The k with 2q/p > 2^k >= q/p: k is the log, and the 1s after the
        // first bit of p/q are the dots.
        let (mut p, mut q) = (r.numerator() as i128, r.denominator() as i128);
        let shift = |x: i128, k: i32| if k >= 0 { x << k } else { x >> -k };
        let mut k = intlog2(q) - intlog2(p);
        if shift(p, k) < q {
            k += 1;
        }
        if k >= 0 {
            p <<= k;
        } else {
            q <<= -k;
        }
        p -= q;
        loop {
            p *= 2;
            if p < q {
                break;
            }
            p -= q;
            d.dots += 1;
        }
        // Down to 64th notes only.
        if k > 6 {
            (d.log, d.dots) = (6, 0);
        } else {
            d.log = k;
        }
        if scale || k > 6 {
            d.factor = r.checked_div(d.length()?)?;
        }
        Some(d)
    }

    pub fn log(self) -> i32 {
        self.log
    }

    pub fn dots(self) -> i32 {
        self.dots
    }

    pub fn factor(self) -> Rational {
        self.factor
    }

    /// Its factor times `m`.
    pub fn compressed(self, m: Rational) -> Option<Duration> {
        Some(Duration {
            factor: self.factor.checked_mul(m)?,
            ..self
        })
    }

    /// `operator Rational`: its length in whole notes.
    pub fn length(self) -> Option<Rational> {
        let power = Rational::int(1i32.wrapping_shl(self.log.wrapping_abs() as u32) as i64);
        let mut length = if self.log > 0 {
            Rational::int(1).checked_div(power)?
        } else {
            power
        };
        let mut delta = length;
        for _ in 0..self.dots {
            delta = delta.checked_div(Rational::int(2))?;
            length = length.checked_add(delta)?;
        }
        length.checked_mul(self.factor)
    }

    /// `Duration::compare`: by length.
    pub fn compare(self, other: Duration) -> Option<Ordering> {
        Some(self.length()?.compare(other.length()?))
    }

    /// `to_string`: `4.`, `8*2/3`, `log = -1`.
    pub fn write(self, out: &mut String) {
        if self.log < 0 {
            let _ = write!(out, "log = {}", self.log);
        } else {
            let _ = write!(out, "{}", 1i32.wrapping_shl(self.log as u32));
        }
        for _ in 0..self.dots {
            out.push('.');
        }
        if self.factor != Rational::int(1) {
            out.push('*');
            self.factor.write(out);
        }
    }
}

impl PartialEq for Duration {
    /// `Duration::equal_p`: the same log, dots and factor.
    fn eq(&self, other: &Duration) -> bool {
        self.log == other.log && self.dots == other.dots && self.factor == other.factor
    }
}

// --- the Scheme face (duration-scheme.cc) --------------------------------------------

impl Foreign for Duration {
    fn type_name(&self) -> &'static str {
        "Duration"
    }

    fn print(&self, _: &Heap, out: &mut String) {
        out.push_str("#<Duration ");
        self.write(out);
        out.push_str(" >");
    }

    fn equal(&self, other: &dyn Foreign, _: &Heap) -> bool {
        (other as &dyn Any).downcast_ref::<Duration>() == Some(self)
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        out.i32(self.log);
        out.i32(self.dots);
        self.factor.save(out);
        true
    }
}

impl Duration {
    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<Duration> {
        Some(Duration {
            log: r.i32()?,
            dots: r.i32()?,
            factor: Rational::load(r)?,
        })
    }
}

fn duration_arg(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<Duration, Thrown> {
    crate::arg(cx, subr, position, v, "Duration")
}

fn duration(cx: &mut Cx, subr: &str, d: Option<Duration>) -> R {
    match d {
        Some(d) => Ok(cx.heap_mut().wrap(d)),
        None => Err(crate::overflow(cx, subr)),
    }
}

fn length(cx: &mut Cx, subr: &str, d: Duration) -> Result<Rational, Thrown> {
    d.length().ok_or_else(|| crate::overflow(cx, subr))
}

/// `(ly:make-duration log [dots [num [den]]])`: a factor `num/den` when
/// either is given.
pub(crate) fn make_duration(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:make-duration";
    crate::check_integer(cx, SUBR, 1, args[0])?;
    let dots = match args.get(1) {
        Some(&v) => {
            crate::check_integer(cx, SUBR, 2, v)?;
            crate::to_i32(cx, v)?
        }
        None => 0,
    };
    let one = cx.heap_mut().integer(1);
    let num = match args.get(2) {
        Some(&v) => {
            let ok = crate::is_rational(cx.heap(), v);
            crate::check(cx, ok, SUBR, 3, v, "rational")?;
            Some(v)
        }
        None => None,
    };
    let den = match args.get(3) {
        Some(&v) => {
            crate::check_integer(cx, SUBR, 4, v)?;
            Some(v)
        }
        None => None,
    };
    let d = Duration::new(crate::to_i32(cx, args[0])?, dots);
    if num.is_none() && den.is_none() {
        return Ok(cx.heap_mut().wrap(d));
    }
    let (num, den) = (num.unwrap_or(one), den.unwrap_or(one));
    let factor = cx
        .heap_mut()
        .div(num, den)
        .map_err(|e| cx.num_error("/", e, &[num, den]))?;
    let factor = crate::to_rational(cx, factor)?;
    duration(cx, SUBR, d.compressed(factor))
}

pub(crate) fn is_duration(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<Duration>(cx, args[0])
}

fn less_than(cx: &mut Cx, subr: &str, args: &[Value]) -> R {
    let a = duration_arg(cx, subr, 1, args[0])?;
    let b = duration_arg(cx, subr, 2, args[1])?;
    match a.compare(b) {
        Some(order) => Ok(Value::bool(order == Ordering::Less)),
        None => Err(crate::overflow(cx, subr)),
    }
}

/// `(ly:duration<? a b)`: whether `a` is shorter.
pub(crate) fn duration_less(cx: &mut Cx, args: &[Value]) -> R {
    less_than(cx, "ly:duration<?", args)
}

/// `ly:duration::less?`, the same as a callback.
pub(crate) fn less(cx: &mut Cx, args: &[Value]) -> R {
    less_than(cx, "ly:duration::less?", args)
}

/// `(ly:number->duration x)`: the duration of `x` whole notes, with a
/// factor for what a log and dots do not make.
pub(crate) fn number_to_duration(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:number->duration";
    let r = crate::rational_arg(cx, SUBR, 1, args[0])?;
    duration(cx, SUBR, Duration::from_length(r, true))
}

pub(crate) fn duration_log(cx: &mut Cx, args: &[Value]) -> R {
    let d = duration_arg(cx, "ly:duration-log", 1, args[0])?;
    Ok(cx.heap_mut().integer(d.log as i64))
}

pub(crate) fn duration_dot_count(cx: &mut Cx, args: &[Value]) -> R {
    let d = duration_arg(cx, "ly:duration-dot-count", 1, args[0])?;
    Ok(cx.heap_mut().integer(d.dots as i64))
}

/// `(ly:intlog2 d)`: the logarithm of `d` in base 2, rounded down. Where
/// LilyPond exits for a `d` below 1, this throws its message.
pub(crate) fn ly_intlog2(cx: &mut Cx, args: &[Value]) -> R {
    let d = args[0];
    let ok = cx.heap().num(d).is_some();
    crate::check(cx, ok, "ly:intlog2", 1, d, "number")?;
    let n = crate::to_i32(cx, d)?;
    if n <= 0 {
        return Err(cx.error(
            "misc-error",
            None,
            "intlog2 with negative argument: ~A",
            &[d],
        ));
    }
    Ok(cx.heap_mut().integer(31 - n.leading_zeros() as i64))
}

pub(crate) fn duration_to_moment(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:duration->moment";
    let d = duration_arg(cx, SUBR, 1, args[0])?;
    let main = length(cx, SUBR, d)?;
    Ok(cx.heap_mut().wrap(Moment::new(main, Rational::ZERO)))
}

pub(crate) fn duration_to_number(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:duration->number";
    let d = duration_arg(cx, SUBR, 1, args[0])?;
    let r = length(cx, SUBR, d)?;
    crate::rational(cx, r)
}

pub(crate) fn duration_to_string(cx: &mut Cx, args: &[Value]) -> R {
    let d = duration_arg(cx, "ly:duration->string", 1, args[0])?;
    Ok(crate::string(cx, |out| d.write(out)))
}

/// `(ly:duration-factor d)`: the factor as a pair.
pub(crate) fn duration_factor(cx: &mut Cx, args: &[Value]) -> R {
    let d = duration_arg(cx, "ly:duration-factor", 1, args[0])?;
    let n = cx.heap_mut().integer(d.factor.numerator());
    let m = cx.heap_mut().integer(d.factor.denominator());
    Ok(cx.heap_mut().cons(n, m))
}

/// `(ly:duration-scale d)`: the factor as a rational.
pub(crate) fn duration_scale(cx: &mut Cx, args: &[Value]) -> R {
    let d = duration_arg(cx, "ly:duration-scale", 1, args[0])?;
    crate::rational(cx, d.factor)
}

pub(crate) fn duration_compress(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:duration-compress";
    let d = duration_arg(cx, SUBR, 1, args[0])?;
    let r = crate::rational_arg(cx, SUBR, 2, args[1])?;
    duration(cx, SUBR, d.compressed(r))
}
