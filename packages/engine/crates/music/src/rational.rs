//! LilyPond's `Rational` (`flower/rational.cc`) and `Moment` (`moment.cc`).
//!
//! A rational is ported as LilyPond holds it: a sign from −2 to 2 (±2 for
//! the infinities) and two unsigned magnitudes, a denominator of 0 for
//! not-a-number. What it gives on its edges is LilyPond's, and depends on
//! that layout: its zero by default is 0 over 1 with a numerator of 1, so
//! dividing that by zero gives +∞ where a normalised zero gives NaN; an
//! infinity's numerator is ±2; NaN compares equal to any positive number.
//!
//! Arithmetic is checked: where LilyPond's would wrap, a finite result that
//! does not fit is `None`, which the `ly:` functions throw as an overflow
//! (D61). Comparison is exact.

use alloc::string::String;
use core::any::Any;
use core::cmp::Ordering;
use core::fmt::Write;

use lily_engine_scheme::{Cx, Foreign, Heap, ImageReader, ImageWriter, Thrown, Value};

use crate::R;

#[derive(Clone, Copy, Debug)]
pub struct Rational {
    sign: i8,
    num: u64,
    den: u64,
}

impl Default for Rational {
    /// LilyPond's `Rational ()`: zero, with a numerator of 1.
    fn default() -> Rational {
        Rational::ZERO
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Rational {
    pub const ZERO: Rational = Rational {
        sign: 0,
        num: 1,
        den: 1,
    };
    pub const INFINITY: Rational = Rational {
        sign: 2,
        num: 1,
        den: 1,
    };
    pub const NAN: Rational = Rational {
        sign: 1,
        num: 0,
        den: 0,
    };

    /// `Rational (long long)`.
    pub const fn int(n: i64) -> Rational {
        Rational {
            sign: n.signum() as i8,
            num: n.unsigned_abs(),
            den: 1,
        }
    }

    /// `Rational (int64_t, int64_t)`: in lowest terms; `n/0` is ±∞ by the
    /// sign of `n`, and `0/0` NaN.
    pub fn new(n: i64, d: i64) -> Rational {
        let mut r = Rational {
            sign: n.signum() as i8 * if d < 0 { -1 } else { 1 },
            num: n.unsigned_abs(),
            den: d.unsigned_abs(),
        };
        r.normalize();
        r
    }

    fn normalize(&mut self) {
        if self.den != 0 {
            if self.sign == 0 {
                (self.num, self.den) = (0, 1);
            } else if self.num == 0 {
                (self.sign, self.den) = (0, 1);
            } else {
                let g = gcd(self.num, self.den);
                self.num /= g;
                self.den /= g;
            }
        } else if self.num != 0 {
            *self = if self.sign < 0 {
                -Rational::INFINITY
            } else {
                Rational::INFINITY
            };
        } else if self.sign == 0 {
            self.sign = 1;
        }
    }

    /// The sign times the numerator's magnitude: ±2 for an infinity.
    pub fn numerator(self) -> i64 {
        (self.sign as i64).wrapping_mul(self.num as i64)
    }

    pub fn denominator(self) -> i64 {
        self.den as i64
    }

    pub fn is_inf(self) -> bool {
        self.sign / 2 != 0
    }

    pub fn is_nan(self) -> bool {
        self.den == 0
    }

    pub fn is_finite(self) -> bool {
        self.den != 0 && !self.is_inf()
    }

    pub fn is_zero(self) -> bool {
        self.sign == 0
    }

    /// Whether it is negative: a finite value, an infinity or NaN.
    pub fn is_negative(self) -> bool {
        self.sign < 0
    }

    /// `operator double`.
    pub fn to_f64(self) -> f64 {
        match self.sign {
            -1..=1 => self.sign as f64 * self.num as f64 / self.den as f64,
            s if s < 0 => f64::NEG_INFINITY,
            _ => f64::INFINITY,
        }
    }

    /// `operator+=`.
    pub fn checked_add(self, r: Rational) -> Option<Rational> {
        if self.is_nan() || r.sign == 0 {
            return Some(self);
        }
        if r.is_nan() || self.sign == 0 {
            return Some(r);
        }
        if self.is_inf() {
            return Some(if self.sign == -r.sign {
                Rational::NAN
            } else {
                self
            });
        }
        if r.is_inf() {
            return Some(r);
        }
        let lcm = (self.den / gcd(self.den, r.den)).checked_mul(r.den)?;
        let term = |x: Rational| (x.num as i128).checked_mul((lcm / x.den) as i128);
        let n = (self.sign as i128 * term(self)?).checked_add(r.sign as i128 * term(r)?)?;
        let n = i64::try_from(n).ok()?;
        let d = i64::try_from(lcm).ok()?;
        let mut sum = Rational {
            sign: n.signum() as i8,
            num: n.unsigned_abs(),
            den: d as u64,
        };
        sum.normalize();
        Some(sum)
    }

    /// `operator-=`.
    pub fn checked_sub(self, r: Rational) -> Option<Rational> {
        self.checked_add(-r)
    }

    /// `operator*=`. A finite product is reduced before it is multiplied,
    /// which gives LilyPond's result where its fits, and `None` where its
    /// would wrap. An infinity's or NaN's magnitudes wrap as LilyPond's do:
    /// only its sign and its denominator's being zero are read.
    pub fn checked_mul(self, r: Rational) -> Option<Rational> {
        let mut x = self;
        x.sign *= r.sign.signum();
        if r.is_inf() {
            x.sign = x.sign.signum() * 2;
            return Some(x);
        }
        if x.is_finite() {
            let a = gcd(x.num, r.den).max(1);
            let b = gcd(r.num, x.den).max(1);
            x.num = (x.num / a).checked_mul(r.num / b)?;
            x.den = (x.den / b).checked_mul(r.den / a)?;
            x.normalize();
            return x.fits();
        }
        x.num = x.num.wrapping_mul(r.num);
        x.den = x.den.wrapping_mul(r.den);
        x.normalize();
        Some(x)
    }

    /// Itself if its numerator and denominator are `int64_t`s, as the
    /// conversions to Scheme read them.
    fn fits(self) -> Option<Rational> {
        (self.num <= i64::MAX as u64 && self.den <= i64::MAX as u64).then_some(self)
    }

    /// `operator/=`: multiplies by the reciprocal; by zero if `r` is
    /// infinite.
    pub fn checked_div(self, r: Rational) -> Option<Rational> {
        let reciprocal = if r.is_inf() {
            Rational::ZERO
        } else {
            Rational {
                sign: if r.num == 0 && r.sign == 0 { 1 } else { r.sign },
                num: r.den,
                den: r.num,
            }
        };
        self.checked_mul(reciprocal)
    }

    /// `Rational::compare`: by sign, then by magnitude.
    pub fn compare(self, s: Rational) -> Ordering {
        if self.sign != s.sign {
            return self.sign.cmp(&s.sign);
        }
        if self.is_inf() || self.sign == 0 {
            return Ordering::Equal;
        }
        let left = self.num as u128 * s.den as u128;
        let right = s.num as u128 * self.den as u128;
        if self.sign > 0 {
            left.cmp(&right)
        } else {
            right.cmp(&left)
        }
    }

    /// `to_string`: `3/8`, `-2`, `infinity`, `-infinity`, `nan`.
    pub fn write(self, out: &mut String) {
        if self.is_inf() || self.is_nan() {
            if self.sign < 0 {
                out.push('-');
            }
            out.push_str(if self.is_inf() { "infinity" } else { "nan" });
            return;
        }
        let _ = write!(out, "{}", self.numerator());
        if self.den != 1 && self.numerator() != 0 {
            let _ = write!(out, "/{}", self.den);
        }
    }
}

impl core::ops::Neg for Rational {
    type Output = Rational;
    fn neg(self) -> Rational {
        Rational {
            sign: -self.sign,
            ..self
        }
    }
}

impl PartialEq for Rational {
    fn eq(&self, other: &Rational) -> bool {
        self.compare(*other) == Ordering::Equal
    }
}

impl Rational {
    /// As it is held, its infinities and NaN too, for the image.
    pub(crate) fn save(&self, w: &mut ImageWriter) {
        w.u8(self.sign as u8);
        w.u64(self.num);
        w.u64(self.den);
    }

    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<Rational> {
        Some(Rational {
            sign: r.u8()? as i8,
            num: r.u64()?,
            den: r.u64()?,
        })
    }
}

/// A point in musical time: the main part, and the grace part for grace
/// notes before it (`moment.hh`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Moment {
    pub main: Rational,
    pub grace: Rational,
}

impl Moment {
    pub fn new(main: Rational, grace: Rational) -> Moment {
        Moment { main, grace }
    }

    pub fn checked_add(self, m: Moment) -> Option<Moment> {
        Some(Moment::new(
            self.main.checked_add(m.main)?,
            self.grace.checked_add(m.grace)?,
        ))
    }

    pub fn checked_sub(self, m: Moment) -> Option<Moment> {
        Some(Moment::new(
            self.main.checked_sub(m.main)?,
            self.grace.checked_sub(m.grace)?,
        ))
    }

    /// Both parts times `r`.
    pub fn checked_mul(self, r: Rational) -> Option<Moment> {
        Some(Moment::new(
            self.main.checked_mul(r)?,
            self.grace.checked_mul(r)?,
        ))
    }

    pub fn checked_div(self, r: Rational) -> Option<Moment> {
        Some(Moment::new(
            self.main.checked_div(r)?,
            self.grace.checked_div(r)?,
        ))
    }

    /// By the main part, then the grace part.
    pub fn compare(self, m: Moment) -> Ordering {
        self.main
            .compare(m.main)
            .then_with(|| self.grace.compare(m.grace))
    }

    /// `to_string`: `1/4`, or `1/4G-1/8` with a grace part.
    pub fn write(self, out: &mut String) {
        self.main.write(out);
        if !self.grace.is_zero() {
            out.push('G');
            self.grace.write(out);
        }
    }
}

impl PartialEq for Moment {
    fn eq(&self, other: &Moment) -> bool {
        self.compare(*other) == Ordering::Equal
    }
}

// --- the Scheme face (moment-scheme.cc) ----------------------------------------------

impl Foreign for Moment {
    fn type_name(&self) -> &'static str {
        "Moment"
    }

    fn print(&self, _: &Heap, out: &mut String) {
        out.push_str("#<Mom ");
        self.write(out);
        out.push('>');
    }

    fn equal(&self, other: &dyn Foreign, _: &Heap) -> bool {
        (other as &dyn Any).downcast_ref::<Moment>() == Some(self)
    }

    fn save(&self, out: &mut ImageWriter) -> bool {
        self.main.save(out);
        self.grace.save(out);
        true
    }
}

impl Moment {
    pub(crate) fn load(r: &mut ImageReader<'_>) -> Option<Moment> {
        Some(Moment::new(Rational::load(r)?, Rational::load(r)?))
    }
}

fn moment_arg(cx: &mut Cx, subr: &str, position: usize, v: Value) -> Result<Moment, Thrown> {
    crate::arg(cx, subr, position, v, "Moment")
}

fn moment(cx: &mut Cx, subr: &str, m: Option<Moment>) -> R {
    match m {
        Some(m) => Ok(cx.heap_mut().wrap(m)),
        None => Err(crate::overflow(cx, subr)),
    }
}

/// `(ly:make-moment m [g [gn [gd]]])`: the main part `m` and the grace part
/// `g`, rationals; or, if `g` is positive, the main part `m/g`; or with
/// three or four integers, `m/g` and `gn/gd`.
pub(crate) fn make_moment(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:make-moment";
    let m = args[0];
    let ok = crate::is_rational(cx.heap(), m);
    crate::check(cx, ok, SUBR, 1, m, "rational")?;
    let made = match *args {
        [_] => Moment::new(crate::to_rational(cx, m)?, Rational::ZERO),
        [_, g] => {
            let ok = crate::is_rational(cx.heap(), g);
            crate::check(cx, ok, SUBR, 2, g, "rational")?;
            if crate::is_positive(cx.heap(), g) {
                crate::check_integer(cx, SUBR, 1, m)?;
                crate::check_integer(cx, SUBR, 2, g)?;
                let (n, d) = (crate::to_i64(cx, m)?, crate::to_i64(cx, g)?);
                Moment::new(Rational::new(n, d), Rational::ZERO)
            } else {
                let main = crate::to_rational(cx, m)?;
                Moment::new(main, crate::to_rational(cx, g)?)
            }
        }
        _ => {
            for (i, &v) in args[..3].iter().enumerate() {
                crate::check_integer(cx, SUBR, i + 1, v)?;
            }
            let grace_num = crate::to_i64(cx, args[2])?;
            let grace_den = match args.get(3) {
                Some(&gd) => {
                    crate::check_integer(cx, SUBR, 4, gd)?;
                    crate::to_i64(cx, gd)?
                }
                None => 1,
            };
            let (n, d) = (crate::to_i64(cx, m)?, crate::to_i64(cx, args[1])?);
            Moment::new(Rational::new(n, d), Rational::new(grace_num, grace_den))
        }
    };
    Ok(cx.heap_mut().wrap(made))
}

pub(crate) fn is_moment(cx: &mut Cx, args: &[Value]) -> R {
    crate::is::<Moment>(cx, args[0])
}

fn two(cx: &mut Cx, subr: &str, args: &[Value]) -> Result<(Moment, Moment), Thrown> {
    Ok((
        moment_arg(cx, subr, 1, args[0])?,
        moment_arg(cx, subr, 2, args[1])?,
    ))
}

pub(crate) fn moment_add(cx: &mut Cx, args: &[Value]) -> R {
    let (a, b) = two(cx, "ly:moment-add", args)?;
    moment(cx, "ly:moment-add", a.checked_add(b))
}

pub(crate) fn moment_sub(cx: &mut Cx, args: &[Value]) -> R {
    let (a, b) = two(cx, "ly:moment-sub", args)?;
    moment(cx, "ly:moment-sub", a.checked_sub(b))
}

/// The factor of `ly:moment-mul` and `ly:moment-div`: a rational, or
/// another moment's main part.
fn factor(cx: &mut Cx, subr: &str, args: &[Value]) -> Result<(Moment, Rational), Thrown> {
    let a = moment_arg(cx, subr, 1, args[0])?;
    match cx.heap().foreign::<Moment>(args[1]) {
        Some(b) => Ok((a, b.main)),
        None => Ok((a, crate::rational_arg(cx, subr, 2, args[1])?)),
    }
}

pub(crate) fn moment_mul(cx: &mut Cx, args: &[Value]) -> R {
    let (a, r) = factor(cx, "ly:moment-mul", args)?;
    moment(cx, "ly:moment-mul", a.checked_mul(r))
}

pub(crate) fn moment_div(cx: &mut Cx, args: &[Value]) -> R {
    let (a, r) = factor(cx, "ly:moment-div", args)?;
    moment(cx, "ly:moment-div", a.checked_div(r))
}

pub(crate) fn moment_main(cx: &mut Cx, args: &[Value]) -> R {
    let m = moment_arg(cx, "ly:moment-main", 1, args[0])?;
    crate::rational(cx, m.main)
}

pub(crate) fn moment_grace(cx: &mut Cx, args: &[Value]) -> R {
    let m = moment_arg(cx, "ly:moment-grace", 1, args[0])?;
    crate::rational(cx, m.grace)
}

fn part(cx: &mut Cx, subr: &str, v: Value, f: fn(Moment) -> i64) -> R {
    let m = moment_arg(cx, subr, 1, v)?;
    Ok(cx.heap_mut().integer(f(m)))
}

pub(crate) fn moment_main_numerator(cx: &mut Cx, args: &[Value]) -> R {
    part(cx, "ly:moment-main-numerator", args[0], |m| {
        m.main.numerator()
    })
}

pub(crate) fn moment_main_denominator(cx: &mut Cx, args: &[Value]) -> R {
    part(cx, "ly:moment-main-denominator", args[0], |m| {
        m.main.denominator()
    })
}

pub(crate) fn moment_grace_numerator(cx: &mut Cx, args: &[Value]) -> R {
    part(cx, "ly:moment-grace-numerator", args[0], |m| {
        m.grace.numerator()
    })
}

pub(crate) fn moment_grace_denominator(cx: &mut Cx, args: &[Value]) -> R {
    part(cx, "ly:moment-grace-denominator", args[0], |m| {
        m.grace.denominator()
    })
}

pub(crate) fn moment_less(cx: &mut Cx, args: &[Value]) -> R {
    let (a, b) = two(cx, "ly:moment<?", args)?;
    Ok(Value::bool(a.compare(b) == Ordering::Less))
}
