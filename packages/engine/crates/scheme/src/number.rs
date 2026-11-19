//! Scheme's numbers as Guile 3 has them (chapter 03 §5.1, R13): fixnums that
//! become bignums when they overflow, exact ratios of either, flonums, and
//! complex numbers as two flonums. The arithmetic, comparison and conversion
//! primitives follow Guile 3.0.11's `libguile/numbers.c`, and so do
//! `number->string` (Burger and Dybvig's shortest digits, in any radix) and
//! `string->number` (its reader of R5RS's number syntax).
//!
//! The slow paths take a number out of the heap as a [`Num`], compute, and
//! put the result back with [`Heap::number`]; two fixnums, or a flonum and a
//! fixnum, never leave the word.

use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::f64::consts::{LN_2, PI};
use core::fmt;

use crate::bignum::{BigInt, DIGITS};
use crate::heap::Heap;
use crate::value::{Tag, Value};

/// Why a numeric primitive failed. Chapter 03 §5.3's error texts (step 5)
/// word it as Guile does, with the procedure's name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NumError {
    /// `Wrong type argument in position N`, N counted from 1.
    WrongType(u8),
    /// `Argument N out of range`.
    OutOfRange(u8),
    /// Guile's `numerical-overflow`: an exact division by zero, `(log 0)`,
    /// a shift or an exact power too large.
    Overflow,
    /// `string->number`'s exponent beyond a double's, Guile's `Value out of
    /// range: E`: E as written, its sign included, at these byte offsets of
    /// the text.
    Exponent { start: usize, end: usize },
}

pub type NumResult = Result<Value, NumError>;

/// How a division's quotient is rounded: Guile's `floor/`, `ceiling/`,
/// `truncate/`, `round/`, `euclidean/` and `centered/` families.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rounding {
    Floor,
    Ceiling,
    Truncate,
    /// To the nearest, ties to even.
    Round,
    /// The remainder never negative.
    Euclidean,
    /// The remainder in [-|y|/2, |y|/2).
    Centered,
}

/// A number outside the heap, what the slow paths compute with.
#[derive(Clone, PartialEq, Debug)]
pub enum Num {
    Int(BigInt),
    /// Numerator and denominator in lowest terms, the denominator above 1.
    Ratio(BigInt, BigInt),
    Real(f64),
    Complex(f64, f64),
}

use Num::{Complex, Int, Ratio, Real};

fn one() -> BigInt {
    BigInt::from_i64(1)
}

impl Num {
    pub fn int(n: i64) -> Num {
        Int(BigInt::from_i64(n))
    }

    /// `n/d` in lowest terms, an integer when `d` divides `n`. `d` must not
    /// be zero.
    pub fn ratio(n: BigInt, d: BigInt) -> Num {
        let g = n.gcd(&d);
        let (mut n, mut d) = if g.is_one() {
            (n, d)
        } else {
            (n.div_rem(&g).0, d.div_rem(&g).0)
        };
        if d.is_negative() {
            (n, d) = (n.neg(), d.neg());
        }
        if d.is_one() { Int(n) } else { Ratio(n, d) }
    }

    pub fn is_exact(&self) -> bool {
        matches!(self, Int(_) | Ratio(..))
    }

    pub fn is_real(&self) -> bool {
        !matches!(self, Complex(..))
    }

    fn is_exact_zero(&self) -> bool {
        matches!(self, Int(n) if n.is_zero())
    }

    fn is_nan(&self) -> bool {
        matches!(self, Real(x) if x.is_nan())
    }

    /// `integer?`.
    fn is_integer(&self) -> bool {
        match self {
            Int(_) => true,
            Real(x) => x.is_finite() && libm::floor(*x) == *x,
            _ => false,
        }
    }

    /// `negative?` of a real.
    fn is_negative(&self) -> bool {
        match self {
            Int(n) | Ratio(n, _) => n.is_negative(),
            Real(x) => *x < 0.0,
            Complex(..) => false,
        }
    }

    /// `zero?`.
    fn is_zero(&self) -> bool {
        match self {
            Int(n) => n.is_zero(),
            Ratio(..) => false,
            Real(x) => *x == 0.0,
            Complex(re, im) => *re == 0.0 && *im == 0.0,
        }
    }

    /// An exact number's numerator and denominator.
    fn parts(&self) -> (BigInt, BigInt) {
        match self {
            Int(n) => (n.clone(), one()),
            Ratio(n, d) => (n.clone(), d.clone()),
            _ => unreachable!("an inexact number has no exact parts"),
        }
    }

    /// The nearest double to a real, `scm_to_double`; a complex number's
    /// real part.
    pub fn to_f64(&self) -> f64 {
        match self {
            Int(n) => n.to_f64(),
            Ratio(n, d) => ratio_to_f64(n, d),
            Real(x) | Complex(x, _) => *x,
        }
    }
}

impl fmt::Display for Num {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        write_num(self, 10, &mut out);
        f.write_str(&out)
    }
}

/// The double nearest `n/d`, ties to even: Guile's `scm_i_divide2double`.
fn ratio_to_f64(n: &BigInt, d: &BigInt) -> f64 {
    const EXACT: i64 = 1 << 53;
    if let (Some(a), Some(b)) = (n.to_i64(), d.to_i64())
        && (-EXACT..=EXACT).contains(&a)
        && (-EXACT..=EXACT).contains(&b)
    {
        // Both are doubles, and IEEE division rounds once.
        return a as f64 / b as f64;
    }
    let negative = n.is_negative() != d.is_negative();
    let (n, d) = (n.abs(), d.abs());
    // Find e with 2^52 <= n / (d × 2^e) < 2^53, no lower than a subnormal's.
    let mut e = (n.bit_length() as i64 - d.bit_length() as i64 - 53).max(-1074);
    let (q, r, divisor) = loop {
        let (num, den) = if e < 0 {
            (n.shl(-e as usize), d.clone())
        } else {
            (n.clone(), d.shl(e as usize))
        };
        let (q, r) = num.div_rem(&den);
        if q.bit_length() > 53 {
            e += 1;
        } else if q.bit_length() < 53 && e > -1074 {
            e -= 1;
        } else {
            break (q, r, den);
        }
    };
    let q = match r.shl(1).cmp(&divisor) {
        Ordering::Greater => q.add(&one()),
        Ordering::Equal if q.is_odd() => q.add(&one()),
        _ => q,
    };
    let x = libm::ldexp(q.to_f64(), e.min(2000) as i32);
    if negative { -x } else { x }
}

/// The exact value of a finite double: `inexact->exact`.
fn exact_of_f64(x: f64) -> Num {
    if x == 0.0 {
        return Num::int(0);
    }
    let bits = x.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i64;
    let fraction = bits & ((1 << 52) - 1);
    let (mut m, mut e) = if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | 1 << 52, biased - 1075)
    };
    if e < 0 {
        let shift = (m.trailing_zeros() as i64).min(-e);
        m >>= shift;
        e += shift;
    }
    let n = BigInt::from_u64(m);
    let n = if x < 0.0 { n.neg() } else { n };
    if e >= 0 {
        Int(n.shl(e as usize))
    } else {
        Ratio(n, one().shl(-e as usize))
    }
}

/// `exact->inexact` of a number that is one.
fn to_inexact(x: &Num) -> Num {
    match x {
        Int(_) | Ratio(..) => Real(x.to_f64()),
        _ => x.clone(),
    }
}

// --- arithmetic --------------------------------------------------------------------

fn add(x: &Num, y: &Num) -> Num {
    match (x, y) {
        (Int(a), Int(b)) => Int(a.add(b)),
        (Complex(a, b), Complex(c, d)) => Complex(a + c, b + d),
        (Complex(a, b), y) => Complex(a + y.to_f64(), *b),
        (x, Complex(c, d)) => Complex(x.to_f64() + c, *d),
        _ if x.is_exact() && y.is_exact() => {
            let ((a, b), (c, d)) = (x.parts(), y.parts());
            Num::ratio(a.mul(&d).add(&c.mul(&b)), b.mul(&d))
        }
        _ => Real(x.to_f64() + y.to_f64()),
    }
}

fn negate(x: &Num) -> Num {
    match x {
        Int(n) => Int(n.neg()),
        Ratio(n, d) => Ratio(n.neg(), d.clone()),
        Real(x) => Real(-x),
        Complex(a, b) => Complex(-a, -b),
    }
}

fn sub(x: &Num, y: &Num) -> Num {
    match (x, y) {
        (Int(a), Int(b)) => Int(a.sub(b)),
        (Complex(a, b), Complex(c, d)) => Complex(a - c, b - d),
        (Complex(a, b), y) => Complex(a - y.to_f64(), *b),
        (x, Complex(c, d)) => Complex(x.to_f64() - c, -d),
        _ if x.is_exact() && y.is_exact() => add(x, &negate(y)),
        _ => Real(x.to_f64() - y.to_f64()),
    }
}

fn mul(x: &Num, y: &Num) -> Num {
    match (x, y) {
        (Int(a), Int(b)) => Int(a.mul(b)),
        (Complex(a, b), Complex(c, d)) => Complex(a * c - b * d, a * d + b * c),
        (Complex(a, b), y) => {
            let r = y.to_f64();
            Complex(a * r, b * r)
        }
        (x, Complex(c, d)) => {
            let r = x.to_f64();
            Complex(r * c, r * d)
        }
        _ if x.is_exact() && y.is_exact() => {
            let ((a, b), (c, d)) = (x.parts(), y.parts());
            Num::ratio(a.mul(&c), b.mul(&d))
        }
        _ => Real(x.to_f64() * y.to_f64()),
    }
}

/// A real divided by a complex number, Guile's `complex_div`.
fn real_by_complex(a: f64, r: f64, i: f64) -> Num {
    if r.abs() <= i.abs() {
        let t = r / i;
        let d = i * (1.0 + t * t);
        Complex((a * t) / d, -a / d)
    } else {
        let t = i / r;
        let d = r * (1.0 + t * t);
        Complex(a / d, -(a * t) / d)
    }
}

fn div(x: &Num, y: &Num) -> Result<Num, NumError> {
    if y.is_exact_zero() {
        return Err(NumError::Overflow);
    }
    Ok(match (x, y) {
        (Complex(rx, ix), Complex(ry, iy)) => {
            if ry.abs() <= iy.abs() {
                let t = ry / iy;
                let d = iy * (1.0 + t * t);
                Complex((rx * t + ix) / d, (ix * t - rx) / d)
            } else {
                let t = iy / ry;
                let d = ry * (1.0 + t * t);
                Complex((rx + ix * t) / d, (ix - rx * t) / d)
            }
        }
        (Complex(a, b), y) => {
            let d = y.to_f64();
            Complex(a / d, b / d)
        }
        (x, Complex(r, i)) => real_by_complex(x.to_f64(), *r, *i),
        _ if x.is_exact() && y.is_exact() => {
            let ((a, b), (c, d)) = (x.parts(), y.parts());
            Num::ratio(a.mul(&d), b.mul(&c))
        }
        _ => Real(x.to_f64() / y.to_f64()),
    })
}

/// Two reals compared by value, exactly even when one is inexact; `None`
/// when either is a NaN.
fn compare(x: &Num, y: &Num) -> Option<Ordering> {
    fn real_with(a: f64, y: &Num) -> Option<Ordering> {
        if a.is_nan() {
            None
        } else if a.is_infinite() {
            Some(if a > 0.0 {
                Ordering::Greater
            } else {
                Ordering::Less
            })
        } else {
            compare(&exact_of_f64(a), y)
        }
    }
    match (x, y) {
        (Real(a), Real(b)) => a.partial_cmp(b),
        (Real(a), y) => real_with(*a, y),
        (x, Real(b)) => real_with(*b, x).map(Ordering::reverse),
        (Int(a), Int(b)) => Some(a.cmp(b)),
        _ => {
            let ((a, b), (c, d)) = (x.parts(), y.parts());
            Some(a.mul(&d).cmp(&c.mul(&b)))
        }
    }
}

/// `=`, which also takes complex numbers.
fn num_eq(x: &Num, y: &Num) -> bool {
    match (x, y) {
        (Complex(a, b), Complex(c, d)) => a == c && b == d,
        (Complex(a, b), y) | (y, Complex(a, b)) => {
            *b == 0.0 && compare(&Real(*a), y) == Some(Ordering::Equal)
        }
        _ => compare(x, y) == Some(Ordering::Equal),
    }
}

/// `max` or `min` of two reals. Exact numbers compare exactly; otherwise
/// both become doubles, a NaN wins, and 0.0 is above -0.0.
fn max_min(x: &Num, y: &Num, max: bool) -> Num {
    if x.is_exact() && y.is_exact() {
        let less = compare(x, y) == Some(Ordering::Less);
        return if less == max { y.clone() } else { x.clone() };
    }
    let (a, b) = (x.to_f64(), y.to_f64());
    let first = if a.is_nan() {
        true
    } else if b.is_nan() {
        false
    } else if a != b {
        (a > b) == max
    } else {
        a.is_sign_negative() != max
    };
    Real(if first { a } else { b })
}

/// Guile's `scm_c_round`: to the nearest integer, ties to even.
fn round_half_even(x: f64) -> f64 {
    if x == libm::floor(x) {
        return x;
    }
    let plus_half = x + 0.5;
    let result = libm::floor(plus_half);
    if plus_half == result && plus_half / 2.0 != libm::floor(plus_half / 2.0) {
        result - 1.0
    } else {
        result
    }
}

/// The quotient and remainder of two exact integers, `d` not zero.
fn div_int(mode: Rounding, n: &BigInt, d: &BigInt) -> (BigInt, BigInt) {
    match mode {
        Rounding::Truncate => n.div_rem(d),
        Rounding::Floor | Rounding::Euclidean => n.div_rem_floor(d),
        Rounding::Ceiling => {
            let (q, r) = n.neg().div_rem_floor(d);
            (q.neg(), r.neg())
        }
        Rounding::Round => {
            // floor's remainder has the divisor's sign: r/d is in [0, 1).
            let (q, r) = n.div_rem_floor(d);
            match r.abs().shl(1).cmp(&d.abs()) {
                Ordering::Greater => (q.add(&one()), r.sub(d)),
                Ordering::Equal if q.is_odd() => (q.add(&one()), r.sub(d)),
                _ => (q, r),
            }
        }
        Rounding::Centered => {
            let size = d.abs();
            let r = n.div_rem_floor(&size).1;
            let r = if r.shl(1) >= size { r.sub(&size) } else { r };
            (n.sub(&r).div_rem(d).0, r)
        }
    }
}

/// `x = q × y + r` with `q` an integer rounded as `mode` says, for two
/// reals. Inexact when either is.
fn divide_rounded(mode: Rounding, x: &Num, y: &Num) -> Result<(Num, Num), NumError> {
    let mode = match mode {
        Rounding::Euclidean if y.is_negative() => Rounding::Ceiling,
        Rounding::Euclidean => Rounding::Floor,
        mode => mode,
    };
    if y.is_exact_zero() {
        return Err(NumError::Overflow);
    }
    if !x.is_exact() || !y.is_exact() {
        let (a, b) = (x.to_f64(), y.to_f64());
        if b == 0.0 {
            return Err(NumError::Overflow);
        }
        let q = match mode {
            Rounding::Floor | Rounding::Euclidean => libm::floor(a / b),
            Rounding::Ceiling => libm::ceil(a / b),
            Rounding::Truncate => libm::trunc(a / b),
            Rounding::Round => round_half_even(a / b),
            Rounding::Centered if b > 0.0 => libm::floor(a / b + 0.5),
            Rounding::Centered if b < 0.0 => libm::ceil(a / b - 0.5),
            Rounding::Centered => f64::NAN,
        };
        return Ok((Real(q), Real(a - q * b)));
    }
    let ((a, b), (c, d)) = (x.parts(), y.parts());
    let (q, r) = div_int(mode, &a.mul(&d), &c.mul(&b));
    Ok((Int(q), Num::ratio(r, b.mul(&d))))
}

/// `floor`, `ceiling`, `truncate` or `round` of a real.
fn round_num(x: &Num, mode: Rounding) -> Num {
    match x {
        Int(_) => x.clone(),
        Ratio(n, d) => Int(div_int(mode, n, d).0),
        Real(v) => Real(match mode {
            Rounding::Floor => libm::floor(*v),
            Rounding::Ceiling => libm::ceil(*v),
            Rounding::Truncate => libm::trunc(*v),
            _ => round_half_even(*v),
        }),
        Complex(..) => unreachable!("rounding a complex number"),
    }
}

// --- powers, roots, logarithms, trigonometry --------------------------------------

/// `make-rectangular` of two reals: the real part alone when the
/// imaginary part is an exact 0.
fn rectangular(re: &Num, im: &Num) -> Num {
    if im.is_exact_zero() {
        re.clone()
    } else {
        Complex(re.to_f64(), im.to_f64())
    }
}

/// Guile's `scm_c_make_polar`.
fn polar(magnitude: f64, angle: f64) -> Num {
    let (s, c) = (libm::sin(angle), libm::cos(angle));
    if s.is_nan() && c.is_nan() && magnitude == 0.0 {
        Complex(0.0, 0.0)
    } else {
        Complex(magnitude * c, magnitude * s)
    }
}

/// `make-polar` of two reals.
fn make_polar(magnitude: &Num, angle: &Num) -> Num {
    if magnitude.is_exact_zero() {
        Num::int(0)
    } else if angle.is_exact_zero() {
        magnitude.clone()
    } else {
        polar(magnitude.to_f64(), angle.to_f64())
    }
}

/// `n^k` for an exact integer `k`: Guile's `integer-expt`.
fn integer_expt(n: &Num, k: &BigInt) -> Result<Num, NumError> {
    let (mut n, mut k) = (n.clone(), k.clone());
    if k.is_negative() {
        if n.is_zero() {
            return Ok(Real(f64::NAN));
        }
        k = k.neg();
        n = div(&Num::int(1), &n)?;
    }
    if k.is_zero() {
        // Exact 1, whatever n is.
        return Ok(Num::int(1));
    }
    match &n {
        Ratio(a, b) => {
            // Lowest terms stay lowest terms.
            let (Int(a), Int(b)) = (
                integer_expt(&Int(a.clone()), &k)?,
                integer_expt(&Int(b.clone()), &k)?,
            ) else {
                unreachable!("an exact integer's power is one");
            };
            return Ok(Ratio(a, b));
        }
        Int(v) => {
            return match k.to_i64() {
                Some(k) => Ok(Int(v.pow(k as u64))),
                None if v.is_zero() || v.is_one() => Ok(n),
                None if *v == BigInt::from_i64(-1) => Ok(if k.is_odd() { n } else { Num::int(1) }),
                None => Err(NumError::Overflow),
            };
        }
        _ => {}
    }
    let mut acc = Num::int(1);
    loop {
        if k.is_one() {
            return Ok(mul(&acc, &n));
        }
        if k.is_odd() {
            acc = mul(&acc, &n);
        }
        n = mul(&n, &n);
        k = k.shr_floor(1);
    }
}

fn expt(x: &Num, y: &Num) -> Result<Num, NumError> {
    if y.is_integer() {
        match y {
            Int(k) => integer_expt(x, k),
            Real(k) => {
                let Int(k) = exact_of_f64(*k) else {
                    unreachable!("an integral double is an exact integer");
                };
                Ok(to_inexact(&integer_expt(&to_inexact(x), &k)?))
            }
            _ => unreachable!("integer? holds of an Int or a Real"),
        }
    } else if x.is_real() && y.is_real() && x.to_f64() >= 0.0 {
        Ok(Real(libm::pow(x.to_f64(), y.to_f64())))
    } else {
        Ok(exp(&mul(&log(x)?, y)))
    }
}

fn exp(z: &Num) -> Num {
    match z {
        Complex(re, im) => polar(libm::exp(*re), *im),
        _ => Real(libm::exp(z.to_f64())),
    }
}

/// `log(|x| × 2^shift)`, with an imaginary part of π when x is negative.
fn log_shifted(x: f64, shift: i64) -> Num {
    let ans = libm::log(x.abs()) + shift as f64 * LN_2;
    if x.is_sign_negative() && !x.is_nan() {
        Complex(ans, PI)
    } else {
        Real(ans)
    }
}

fn log_int(n: &BigInt) -> Num {
    match n.to_i64() {
        Some(k) if Value::fixnum(k).is_some() => log_shifted(k as f64, 0),
        _ => {
            let (significand, exponent) = n.frexp();
            log_shifted(significand, exponent)
        }
    }
}

fn log(z: &Num) -> Result<Num, NumError> {
    Ok(match z {
        Complex(re, im) => Complex(libm::log(libm::hypot(*re, *im)), libm::atan2(*im, *re)),
        Real(x) => log_shifted(*x, 0),
        Int(n) if n.is_zero() => return Err(NumError::Overflow),
        Int(n) => log_int(n),
        Ratio(n, d) => {
            if (n.bit_length() as i64 - d.bit_length() as i64).abs() > 1 {
                sub(&log_int(n), &log_int(d))
            } else if !n.is_negative() {
                Real(libm::log1p(ratio_to_f64(&n.sub(d), d)))
            } else {
                Complex(libm::log1p(ratio_to_f64(&n.abs().sub(d), d)), PI)
            }
        }
    })
}

/// The square root of an exact integer as a double, negative for a
/// negative one.
fn inexact_sqrt(n: &BigInt) -> f64 {
    if let Some(k) = n.to_i64().filter(|&k| Value::fixnum(k).is_some()) {
        return if k < 0 {
            -libm::sqrt(-k as f64)
        } else {
            libm::sqrt(k as f64)
        };
    }
    let (mut significand, mut exponent) = n.frexp();
    let negative = significand < 0.0;
    significand = significand.abs();
    if exponent & 1 == 1 {
        significand *= 2.0;
        exponent -= 1;
    }
    let root = libm::ldexp(libm::sqrt(significand), (exponent / 2) as i32);
    if negative { -root } else { root }
}

fn sqrt(z: &Num) -> Num {
    match z {
        Int(n) => {
            if !n.is_negative() {
                let (s, r) = n.isqrt();
                if r.is_zero() {
                    return Int(s);
                }
            }
            let root = inexact_sqrt(n);
            if root < 0.0 {
                Complex(0.0, -root)
            } else {
                Real(root)
            }
        }
        Real(x) if *x < 0.0 => Complex(0.0, libm::sqrt(-x)),
        Real(x) => Real(libm::sqrt(*x)),
        Complex(re, im) => polar(
            libm::sqrt(libm::hypot(*re, *im)),
            0.5 * libm::atan2(*im, *re),
        ),
        Ratio(n, d) => {
            if !n.is_negative() {
                let ((ns, nr), (ds, dr)) = (n.isqrt(), d.isqrt());
                if nr.is_zero() && dr.is_zero() {
                    return Ratio(ns, ds);
                }
            }
            let mut x = ratio_to_f64(n, d);
            let mut shift = 0i64;
            if x.abs() > f64::MAX || x.abs() < f64::MIN_POSITIVE {
                shift = (n.bit_length() as i64 - d.bit_length() as i64) / 2;
                x = if shift > 0 {
                    ratio_to_f64(n, &d.shl((2 * shift) as usize))
                } else {
                    ratio_to_f64(&n.shl((-2 * shift) as usize), d)
                };
            }
            if x < 0.0 {
                Complex(0.0, libm::ldexp(libm::sqrt(-x), shift as i32))
            } else {
                Real(libm::ldexp(libm::sqrt(x), shift as i32))
            }
        }
    }
}

fn sin(z: &Num) -> Num {
    match z {
        _ if z.is_exact_zero() => z.clone(),
        Complex(x, y) => Complex(
            libm::sin(*x) * libm::cosh(*y),
            libm::cos(*x) * libm::sinh(*y),
        ),
        _ => Real(libm::sin(z.to_f64())),
    }
}

fn cos(z: &Num) -> Num {
    match z {
        _ if z.is_exact_zero() => Num::int(1),
        Complex(x, y) => Complex(
            libm::cos(*x) * libm::cosh(*y),
            -libm::sin(*x) * libm::sinh(*y),
        ),
        _ => Real(libm::cos(z.to_f64())),
    }
}

fn tan(z: &Num) -> Num {
    match z {
        _ if z.is_exact_zero() => z.clone(),
        Complex(x, y) => {
            let (x, y) = (2.0 * x, 2.0 * y);
            let w = libm::cos(x) + libm::cosh(y);
            Complex(libm::sin(x) / w, libm::sinh(y) / w)
        }
        _ => Real(libm::tan(z.to_f64())),
    }
}

fn asinh(z: &Num) -> Result<Num, NumError> {
    match z {
        _ if z.is_exact_zero() => Ok(z.clone()),
        Complex(..) => log(&add(z, &sqrt(&add(&mul(z, z), &Num::int(1))))),
        _ => Ok(Real(libm::asinh(z.to_f64()))),
    }
}

fn asin(z: &Num) -> Result<Num, NumError> {
    match z {
        _ if z.is_exact_zero() => Ok(z.clone()),
        Complex(x, y) => Ok(mul(&Complex(0.0, -1.0), &asinh(&Complex(-y, *x))?)),
        _ => {
            let w = z.to_f64();
            if (-1.0..=1.0).contains(&w) {
                Ok(Real(libm::asin(w)))
            } else {
                Ok(mul(&Complex(0.0, -1.0), &asinh(&Complex(0.0, w))?))
            }
        }
    }
}

fn acos(z: &Num) -> Result<Num, NumError> {
    let via_asinh = |z: Num| -> Result<Num, NumError> {
        Ok(add(
            &Real(libm::acos(0.0)),
            &mul(&Complex(0.0, 1.0), &asinh(&z)?),
        ))
    };
    match z {
        Int(n) if n.is_one() => Ok(Num::int(0)),
        Complex(x, y) => via_asinh(Complex(-y, *x)),
        _ => {
            let w = z.to_f64();
            if (-1.0..=1.0).contains(&w) {
                Ok(Real(libm::acos(w)))
            } else {
                via_asinh(Complex(0.0, w))
            }
        }
    }
}

fn atan(z: &Num) -> Result<Num, NumError> {
    match z {
        _ if z.is_exact_zero() => Ok(z.clone()),
        Complex(v, w) => {
            let q = div(&Complex(-v, 1.0 - w), &Complex(*v, 1.0 + w))?;
            div(&log(&q)?, &Complex(0.0, 2.0))
        }
        _ => Ok(Real(libm::atan(z.to_f64()))),
    }
}

// --- number->string ------------------------------------------------------------

/// A double as Guile's `idbl2str` writes it: the shortest digits that read
/// back as the same double (Burger and Dybvig's free-format algorithm), in
/// `radix`, with an exponent below 10^-3 and above 10^7 or so.
fn write_f64(x: f64, radix: u32, out: &mut String) {
    let radix = if (2..=36).contains(&radix) { radix } else { 10 };
    if x.is_infinite() {
        out.push_str(if x > 0.0 { "+inf.0" } else { "-inf.0" });
        return;
    }
    if x.is_nan() {
        out.push_str("+nan.0");
        return;
    }
    if x.is_sign_negative() {
        out.push('-');
    }
    if x == 0.0 {
        out.push_str("0.0");
        return;
    }
    let bits = x.abs().to_bits();
    let biased = (bits >> 52) as i64;
    let fraction = bits & ((1 << 52) - 1);
    let (f, e) = if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | 1 << 52, biased - 1075)
    };
    let odd = (f & 1) as i32;
    let even = 1 - odd;
    let lowest = f == 1 << 52 && e != -1074;
    let f = BigInt::from_u64(f);
    let times = |n: &BigInt| n.mul_add_small(radix, 0);
    let (mut r, mut s, mut plus, mut minus);
    if e < 0 {
        minus = one();
        if !lowest {
            plus = one();
            r = f.shl(1);
            s = one().shl((1 - e) as usize);
        } else {
            plus = BigInt::from_i64(2);
            r = f.shl(2);
            s = one().shl((2 - e) as usize);
        }
    } else {
        minus = one().shl(e as usize);
        if !lowest {
            plus = minus.clone();
            r = f.shl((1 + e) as usize);
            s = BigInt::from_i64(2);
        } else {
            plus = minus.shl(1);
            r = f.shl((2 + e) as usize);
            s = BigInt::from_i64(4);
        }
    }
    // The smallest k with (r + plus) / s below radix^k (or at it, for odd f).
    let mut high = r.add(&plus);
    let mut k: i64 = 0;
    while high.cmp(&s) as i32 >= odd {
        s = times(&s);
        k += 1;
    }
    if k == 0 {
        high = times(&high);
        while (high.cmp(&s) as i32) < odd {
            r = times(&r);
            plus = times(&plus);
            minus = times(&minus);
            high = times(&high);
            k -= 1;
        }
    }
    let exponent = k - 1;
    let mut a: Vec<u8> = Vec::new();
    let mut show_exponent = false;
    if k <= 0 {
        if k <= -3 {
            show_exponent = true;
            k = 1;
        } else {
            a.extend_from_slice(b"0.");
            a.extend((0..-k).map(|_| b'0'));
        }
    }
    loop {
        plus = times(&plus);
        minus = times(&minus);
        let (digit, rest) = times(&r).div_rem(&s);
        r = rest;
        let mut d = digit.to_i64().unwrap_or(0) as usize;
        high = r.add(&plus);
        let low_end = (r.cmp(&minus) as i32) < even;
        let high_end = (s.cmp(&high) as i32) < even;
        let last = low_end || high_end;
        if last {
            r = r.shl(1);
            if !high_end {
            } else if !low_end || r.cmp(&s) as i32 >= (d & 1 == 0) as i32 {
                d += 1;
            }
        }
        a.push(DIGITS[d.min(35)]);
        k -= 1;
        if k == 0 {
            a.push(b'.');
        }
        if last {
            break;
        }
    }
    if k > 0 {
        if exponent >= 7 && k >= 4 && exponent >= k {
            // Rather than four or more zeros before the point, an exponent.
            k -= exponent;
            a.insert((a.len() as i64 + k) as usize, b'.');
            show_exponent = true;
        } else {
            a.extend((0..k).map(|_| b'0'));
            a.push(b'.');
            k = 0;
        }
    }
    if k == 0 {
        a.push(b'0');
    }
    out.push_str(core::str::from_utf8(&a).unwrap_or_default());
    if show_exponent {
        out.push('e');
        out.push_str(&BigInt::from_i64(exponent).to_string_radix(radix));
    }
}

/// `number->string` of a number in `radix`, 2 to 36.
pub fn write_num(n: &Num, radix: u32, out: &mut String) {
    match n {
        Int(n) => out.push_str(&n.to_string_radix(radix)),
        Ratio(n, d) => {
            out.push_str(&n.to_string_radix(radix));
            out.push('/');
            out.push_str(&d.to_string_radix(radix));
        }
        Real(x) => write_f64(*x, radix, out),
        Complex(re, im) => {
            write_f64(*re, radix, out);
            // Infinities and NaNs bring their own sign.
            if im.is_sign_positive() && im.is_finite() {
                out.push('+');
            }
            write_f64(*im, radix, out);
            out.push('i');
        }
    }
}

/// How the printer writes a number of the heap.
pub(crate) fn write_number(heap: &Heap, value: Value, radix: u32, out: &mut String) {
    if let Some(n) = heap.num(value) {
        write_num(&n, radix, out);
    }
}

// --- string->number --------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Exactness {
    Unspecified,
    Inexact,
    Exact,
}

/// The text as characters, as Guile's reader indexes it, and the text for
/// the byte offsets of an error.
struct Text<'a> {
    chars: Vec<char>,
    text: &'a str,
}

impl Text<'_> {
    fn len(&self) -> usize {
        self.chars.len()
    }

    fn at(&self, i: usize) -> Option<char> {
        self.chars.get(i).copied()
    }

    fn byte(&self, i: usize) -> usize {
        self.text
            .char_indices()
            .nth(i)
            .map_or(self.text.len(), |(b, _)| b)
    }
}

/// Guile's `char_decimal_value`, for ASCII digits and letters (it also takes
/// Unicode's other decimal digits, which this does not).
fn digit_value(c: char) -> u32 {
    match c {
        '0'..='9' => c as u32 - '0' as u32,
        _ => {
            let c = c.to_ascii_lowercase();
            if c >= 'a' {
                c as u32 - 'a' as u32 + 10
            } else {
                u32::MAX
            }
        }
    }
}

/// `<uinteger R>`, with `#` for trailing digits that make it inexact.
fn uinteger(text: &Text, at: &mut usize, radix: u32, x: &mut Exactness) -> Option<BigInt> {
    let mut i = *at;
    let first = text.at(i)?;
    // Guile reads the first character into a C char, which keeps its low
    // byte and makes the bytes above ASCII negative.
    let low = first as u32 as u8;
    let d = if low < 0x80 {
        digit_value(low as char)
    } else {
        u32::MAX
    };
    if d >= radix {
        return None;
    }
    i += 1;
    let mut value = BigInt::from_u64(d as u64);
    let mut hash = false;
    while let Some(c) = text.at(i) {
        let d = if c == '#' {
            hash = true;
            0
        } else if hash {
            break;
        } else {
            match digit_value(c) {
                d if d < radix => d,
                _ => break,
            }
        };
        i += 1;
        value = value.mul_add_small(radix, d);
    }
    *at = i;
    if hash {
        *x = Exactness::Inexact;
    }
    Some(value)
}

/// `<decimal 10>` from a possible point on: the digits after it and an
/// exponent, applied to `result`, exactly.
fn decimal_from_point(
    mut result: Num,
    text: &Text,
    at: &mut usize,
    x: &mut Exactness,
) -> Result<Option<Num>, NumError> {
    let mut i = *at;
    let mut exactness = *x;
    if i == text.len() {
        return Ok(Some(result));
    }
    if text.at(i) == Some('.') {
        i += 1;
        let (mut digits, mut scale) = (BigInt::zero(), 0u64);
        while let Some(c) = text.at(i) {
            let d = if c.is_ascii_digit() {
                if exactness == Exactness::Inexact {
                    return Ok(None);
                }
                c as u32 - '0' as u32
            } else if c == '#' {
                exactness = Exactness::Inexact;
                0
            } else {
                break;
            };
            i += 1;
            digits = digits.mul_add_small(10, d);
            scale += 1;
        }
        let shift = Int(BigInt::from_i64(10).pow(scale));
        result = div(&add(&mul(&result, &shift), &Int(digits)), &shift)?;
        exactness = Exactness::Inexact;
    }
    if let Some('d' | 'D' | 'e' | 'E' | 'f' | 'F' | 'l' | 'L' | 's' | 'S') = text.at(i) {
        i += 1;
        let start = i;
        let mut negative = false;
        match text.at(i) {
            None => return Ok(None),
            Some(sign @ ('-' | '+')) => {
                negative = sign == '-';
                i += 1;
            }
            _ => {}
        }
        let Some(c) = text.at(i).filter(char::is_ascii_digit) else {
            return Ok(None);
        };
        i += 1;
        let mut exponent = c as u64 - '0' as u64;
        while let Some(c) = text.at(i).filter(char::is_ascii_digit) {
            i += 1;
            if exponent <= MAX_EXPONENT {
                exponent = exponent * 10 + (c as u64 - '0' as u64);
            }
        }
        // DBL_MAX_10_EXP, and DBL_DIG + 1 more for the subnormals.
        if exponent
            > if negative {
                MAX_EXPONENT + 16
            } else {
                MAX_EXPONENT
            }
        {
            return Err(NumError::Exponent {
                start: text.byte(start),
                end: text.byte(i),
            });
        }
        let e = Int(BigInt::from_i64(10).pow(exponent));
        result = if negative {
            div(&result, &e)?
        } else {
            mul(&result, &e)
        };
        exactness = Exactness::Inexact;
    }
    *at = i;
    if exactness == Exactness::Inexact {
        *x = exactness;
    }
    Ok(Some(result))
}

/// Guile's `SCM_MAXEXP`, DBL_MAX_10_EXP of IEEE doubles.
const MAX_EXPONENT: u64 = 308;

/// `<ureal R>`, with `inf.0` and `nan.0` after a sign when `special`.
fn ureal(
    text: &Text,
    at: &mut usize,
    radix: u32,
    forced: Exactness,
    special: bool,
) -> Result<Option<Num>, NumError> {
    let mut i = *at;
    let mut implicit = Exactness::Exact;
    let len = text.len();
    if i == len {
        return Ok(None);
    }
    if special && forced != Exactness::Exact && i + 5 <= len {
        let lower = |k: usize| text.chars[i + k].to_ascii_lowercase();
        if lower(0) == 'i'
            && lower(1) == 'n'
            && lower(2) == 'f'
            && text.chars[i + 3] == '.'
            && text.chars[i + 4] == '0'
        {
            *at = i + 5;
            return Ok(Some(Real(f64::INFINITY)));
        }
        // Guile's switch falls through from `i` to `n`: `+ian.0` is a NaN.
        if matches!(lower(0), 'i' | 'n')
            && lower(1) == 'a'
            && lower(2) == 'n'
            && text.chars[i + 3] == '.'
        {
            let mut j = i + 4;
            return Ok(match uinteger(text, &mut j, 10, &mut implicit) {
                Some(zero) if zero.is_zero() => {
                    *at = j;
                    Some(Real(f64::NAN))
                }
                _ => None,
            });
        }
    }
    let result = if text.at(i) == Some('.') {
        if radix != 10 || !text.at(i + 1).is_some_and(|c| c.is_ascii_digit()) {
            return Ok(None);
        }
        match decimal_from_point(Num::int(0), text, at, &mut implicit)? {
            Some(result) => result,
            None => return Ok(None),
        }
    } else {
        let Some(whole) = uinteger(text, &mut i, radix, &mut implicit) else {
            return Ok(None);
        };
        let result = if i == len {
            Int(whole)
        } else if text.at(i) == Some('/') {
            i += 1;
            match uinteger(text, &mut i, radix, &mut implicit) {
                Some(divisor) if !divisor.is_zero() => Num::ratio(whole, divisor),
                _ => return Ok(None),
            }
        } else if radix == 10 {
            match decimal_from_point(Int(whole), text, &mut i, &mut implicit)? {
                Some(result) => result,
                None => return Ok(None),
            }
        } else {
            Int(whole)
        };
        *at = i;
        result
    };
    Ok(Some(match forced {
        Exactness::Exact => result,
        Exactness::Inexact => to_inexact(&result),
        Exactness::Unspecified if implicit == Exactness::Inexact => to_inexact(&result),
        Exactness::Unspecified => result,
    }))
}

/// `<complex R>`: a real, `±i`, `a±bi`, `±bi` or `m@a`, to the text's end.
fn complex(
    text: &Text,
    mut i: usize,
    radix: u32,
    forced: Exactness,
) -> Result<Option<Num>, NumError> {
    let len = text.len();
    let mut sign = 0;
    match text.at(i) {
        None => return Ok(None),
        Some('+') => (i, sign) = (i + 1, 1),
        Some('-') => (i, sign) = (i + 1, -1),
        _ => {}
    }
    if i == len {
        return Ok(None);
    }
    let Some(mut real) = ureal(text, &mut i, radix, forced, sign != 0)? else {
        // `+i` or `-i`.
        return Ok(
            (sign != 0 && matches!(text.at(i), Some('i' | 'I')) && i + 1 == len)
                .then(|| rectangular(&Num::int(0), &Num::int(sign))),
        );
    };
    if sign == -1 && !real.is_nan() {
        real = negate(&real);
    }
    if i == len {
        return Ok(Some(real));
    }
    let c = text.chars[i];
    i += 1;
    match c {
        'i' | 'I' => Ok((sign != 0 && i == len).then(|| rectangular(&Num::int(0), &real))),
        '@' => {
            let mut sign = 0;
            match text.at(i) {
                None => return Ok(None),
                Some('+') => (i, sign) = (i + 1, 1),
                Some('-') => (i, sign) = (i + 1, -1),
                _ => {}
            }
            if i == len {
                return Ok(None);
            }
            let Some(mut angle) = ureal(text, &mut i, radix, forced, sign != 0)? else {
                return Ok(None);
            };
            if i != len {
                return Ok(None);
            }
            // Guile asks whether the magnitude, not the angle, is a NaN.
            if sign == -1 && !real.is_nan() {
                angle = negate(&angle);
            }
            Ok(Some(make_polar(&real, &angle)))
        }
        '+' | '-' => {
            if i == len {
                return Ok(None);
            }
            let sign = if c == '+' { 1 } else { -1 };
            let imaginary = match ureal(text, &mut i, radix, forced, true)? {
                None => Num::int(sign),
                Some(im) if sign == -1 && !im.is_nan() => negate(&im),
                Some(im) => im,
            };
            let closed = matches!(text.at(i), Some('i' | 'I')) && i + 1 == len;
            Ok(closed.then(|| rectangular(&real, &imaginary)))
        }
        _ => Ok(None),
    }
}

/// `string->number` without the heap: what `text` reads as with the default
/// `radix` (2 and up), `Ok(None)` when it is not a number. An exponent
/// beyond a double's is an error, as in Guile.
pub fn parse(text: &str, radix: u32) -> Result<Option<Num>, NumError> {
    let text = Text {
        chars: text.chars().collect(),
        text,
    };
    let mut i = 0;
    let mut prefix_radix = None;
    let mut forced = Exactness::Unspecified;
    while i + 2 < text.len() && text.chars[i] == '#' {
        match text.chars[i + 1].to_ascii_lowercase() {
            c @ ('b' | 'o' | 'd' | 'x') => {
                if prefix_radix.is_some() {
                    return Ok(None);
                }
                prefix_radix = Some(match c {
                    'b' => 2,
                    'o' => 8,
                    'd' => 10,
                    _ => 16,
                });
            }
            c @ ('i' | 'e') => {
                if forced != Exactness::Unspecified {
                    return Ok(None);
                }
                forced = if c == 'i' {
                    Exactness::Inexact
                } else {
                    Exactness::Exact
                };
            }
            _ => return Ok(None),
        }
        i += 2;
    }
    complex(&text, i, prefix_radix.unwrap_or(radix), forced)
}

// --- the primitives -----------------------------------------------------------------

impl Heap {
    /// `n`, a fixnum or a bignum.
    pub fn integer(&mut self, n: i64) -> Value {
        match Value::fixnum(n) {
            Some(value) => value,
            None => self.bignum(BigInt::from_i64(n)),
        }
    }

    fn integer_i128(&mut self, n: i128) -> Value {
        match i64::try_from(n) {
            Ok(n) => self.integer(n),
            Err(_) => self.bignum(BigInt::from_i128(n)),
        }
    }

    pub fn integer_big(&mut self, n: BigInt) -> Value {
        match n.to_i64().and_then(Value::fixnum) {
            Some(value) => value,
            None => self.bignum(n),
        }
    }

    /// Puts a number into the heap, in its smallest form.
    pub fn number(&mut self, n: Num) -> Value {
        match n {
            Int(n) => self.integer_big(n),
            Ratio(n, d) => {
                let (n, d) = (self.integer_big(n), self.integer_big(d));
                self.ratio(n, d)
            }
            Real(x) => self.flonum(x),
            Complex(re, im) => self.complex(re, im),
        }
    }

    /// The number a value is, out of the heap; `None` if it is not one.
    pub fn num(&self, value: Value) -> Option<Num> {
        match value.tag() {
            Tag::Fixnum => Some(Num::int(value.as_fixnum()?)),
            Tag::Bignum => Some(Int(self.as_bignum(value)?.clone())),
            Tag::Ratio => {
                let (n, d) = self.as_ratio(value)?;
                match (self.num(n)?, self.num(d)?) {
                    (Int(n), Int(d)) => Some(Ratio(n, d)),
                    _ => None,
                }
            }
            Tag::Flonum => Some(Real(self.as_flonum(value)?)),
            Tag::Complex => {
                let (re, im) = self.as_complex(value)?;
                Some(Complex(re, im))
            }
            _ => None,
        }
    }

    /// The nearest double to a real number, as Rust code wants one.
    pub fn to_f64(&self, value: Value) -> Option<f64> {
        match value.tag() {
            Tag::Fixnum => Some(value.as_fixnum()? as f64),
            Tag::Flonum => self.as_flonum(value),
            Tag::Complex => None,
            _ => Some(self.num(value)?.to_f64()),
        }
    }

    /// An exact integer that fits an `i64`.
    pub fn to_i64(&self, value: Value) -> Option<i64> {
        value
            .as_fixnum()
            .or_else(|| self.as_bignum(value)?.to_i64())
    }

    fn arg(&self, value: Value, position: u8) -> Result<Num, NumError> {
        self.num(value).ok_or(NumError::WrongType(position))
    }

    fn real_arg(&self, value: Value, position: u8) -> Result<Num, NumError> {
        self.num(value)
            .filter(Num::is_real)
            .ok_or(NumError::WrongType(position))
    }

    /// An exact or inexact integer, as `quotient` and `gcd` take.
    fn integer_arg(&self, value: Value, position: u8) -> Result<Num, NumError> {
        self.num(value)
            .filter(Num::is_integer)
            .ok_or(NumError::WrongType(position))
    }

    fn exact_integer_arg(&self, value: Value, position: u8) -> Result<BigInt, NumError> {
        match self.num(value) {
            Some(Int(n)) => Ok(n),
            _ => Err(NumError::WrongType(position)),
        }
    }

    /// Two fixnums or flonums, at least one a flonum, as doubles.
    fn doubles(&self, x: Value, y: Value) -> Option<(f64, f64)> {
        if x.tag() != Tag::Flonum && y.tag() != Tag::Flonum {
            return None;
        }
        let double = |v: Value| match v.tag() {
            Tag::Fixnum => v.as_fixnum().map(|n| n as f64),
            Tag::Flonum => self.as_flonum(v),
            _ => None,
        };
        Some((double(x)?, double(y)?))
    }

    // --- predicates

    /// `number?` and `complex?`.
    pub fn is_number(&self, value: Value) -> bool {
        matches!(
            value.tag(),
            Tag::Fixnum | Tag::Flonum | Tag::Bignum | Tag::Ratio | Tag::Complex
        )
    }

    /// `real?`: a complex number is not, even with an imaginary part of 0.0.
    pub fn is_real(&self, value: Value) -> bool {
        self.is_number(value) && value.tag() != Tag::Complex
    }

    /// `rational?`: exact, or a finite flonum.
    pub fn is_rational(&self, value: Value) -> bool {
        match value.tag() {
            Tag::Fixnum | Tag::Bignum | Tag::Ratio => true,
            Tag::Flonum => self.as_flonum(value).is_some_and(f64::is_finite),
            _ => false,
        }
    }

    /// `integer?`: `2` and `2.0`.
    pub fn is_integer(&self, value: Value) -> bool {
        self.num(value).is_some_and(|n| n.is_integer())
    }

    /// `exact-integer?`.
    pub fn is_exact_integer(&self, value: Value) -> bool {
        matches!(value.tag(), Tag::Fixnum | Tag::Bignum)
    }

    /// `exact?`.
    pub fn is_exact(&self, value: Value) -> Result<bool, NumError> {
        Ok(self.arg(value, 1)?.is_exact())
    }

    /// `inexact?`.
    pub fn is_inexact(&self, value: Value) -> Result<bool, NumError> {
        Ok(!self.arg(value, 1)?.is_exact())
    }

    /// `nan?`, which takes reals only, like `inf?` and `finite?`.
    pub fn is_nan(&self, value: Value) -> Result<bool, NumError> {
        Ok(self.real_arg(value, 1)?.is_nan())
    }

    pub fn is_inf(&self, value: Value) -> Result<bool, NumError> {
        Ok(matches!(self.real_arg(value, 1)?, Real(x) if x.is_infinite()))
    }

    pub fn is_finite(&self, value: Value) -> Result<bool, NumError> {
        Ok(match self.real_arg(value, 1)? {
            Real(x) => x.is_finite(),
            _ => true,
        })
    }

    /// `zero?`; a complex number is zero when both parts are.
    pub fn is_zero(&self, value: Value) -> Result<bool, NumError> {
        Ok(self.arg(value, 1)?.is_zero())
    }

    pub fn is_positive(&self, value: Value) -> Result<bool, NumError> {
        Ok(match self.real_arg(value, 1)? {
            Real(x) => x > 0.0,
            n => !n.is_negative() && !n.is_zero(),
        })
    }

    pub fn is_negative(&self, value: Value) -> Result<bool, NumError> {
        Ok(self.real_arg(value, 1)?.is_negative())
    }

    /// `odd?`: of an integer, `3.0` too.
    pub fn is_odd(&self, value: Value) -> Result<bool, NumError> {
        match self.num(value) {
            Some(Int(n)) => Ok(n.is_odd()),
            Some(Real(x)) if x.is_finite() => {
                let rest = libm::fmod(x, 2.0).abs();
                if rest == 1.0 || rest == 0.0 {
                    Ok(rest == 1.0)
                } else {
                    Err(NumError::WrongType(1))
                }
            }
            _ => Err(NumError::WrongType(1)),
        }
    }

    pub fn is_even(&self, value: Value) -> Result<bool, NumError> {
        self.is_odd(value).map(|odd| !odd)
    }

    // --- arithmetic

    /// `+` of two numbers; the VM folds the n-ary forms.
    pub fn add(&mut self, x: Value, y: Value) -> NumResult {
        if let (Some(a), Some(b)) = (x.as_fixnum(), y.as_fixnum()) {
            return Ok(self.integer(a + b));
        }
        if let Some((a, b)) = self.doubles(x, y) {
            return Ok(self.flonum(a + b));
        }
        let (a, b) = (self.arg(x, 1)?, self.arg(y, 2)?);
        Ok(self.number(add(&a, &b)))
    }

    pub fn sub(&mut self, x: Value, y: Value) -> NumResult {
        if let (Some(a), Some(b)) = (x.as_fixnum(), y.as_fixnum()) {
            return Ok(self.integer(a - b));
        }
        if let Some((a, b)) = self.doubles(x, y) {
            return Ok(self.flonum(a - b));
        }
        let (a, b) = (self.arg(x, 1)?, self.arg(y, 2)?);
        Ok(self.number(sub(&a, &b)))
    }

    pub fn mul(&mut self, x: Value, y: Value) -> NumResult {
        if let (Some(a), Some(b)) = (x.as_fixnum(), y.as_fixnum()) {
            return Ok(self.integer_i128(a as i128 * b as i128));
        }
        if let Some((a, b)) = self.doubles(x, y) {
            return Ok(self.flonum(a * b));
        }
        let (a, b) = (self.arg(x, 1)?, self.arg(y, 2)?);
        Ok(self.number(mul(&a, &b)))
    }

    /// `/` of two numbers: exact when both are, and an exact 0 divisor is
    /// an error even under an inexact dividend.
    pub fn div(&mut self, x: Value, y: Value) -> NumResult {
        if let (Some(a), Some(b)) = (x.as_fixnum(), y.as_fixnum())
            && b != 0
            && a % b == 0
        {
            return Ok(self.integer(a / b));
        }
        if y.as_fixnum() != Some(0)
            && let Some((a, b)) = self.doubles(x, y)
        {
            return Ok(self.flonum(a / b));
        }
        let (a, b) = (self.arg(x, 1)?, self.arg(y, 2)?);
        let quotient = div(&a, &b)?;
        Ok(self.number(quotient))
    }

    /// `(- x)`, which keeps the sign of a zero: `(- 0.0)` is `-0.0`.
    pub fn negate(&mut self, x: Value) -> NumResult {
        let n = negate(&self.arg(x, 1)?);
        Ok(self.number(n))
    }

    /// `(/ x)`.
    pub fn reciprocal(&mut self, x: Value) -> NumResult {
        let n = self.arg(x, 1)?;
        let quotient = div(&Num::int(1), &n)?;
        Ok(self.number(quotient))
    }

    /// `1+`.
    pub fn one_plus(&mut self, x: Value) -> NumResult {
        self.add(x, Value::fixnum(1).unwrap_or_default())
    }

    /// `1-`.
    pub fn one_minus(&mut self, x: Value) -> NumResult {
        self.sub(x, Value::fixnum(1).unwrap_or_default())
    }

    pub fn abs(&mut self, x: Value) -> NumResult {
        let n = match self.real_arg(x, 1)? {
            Real(v) => Real(v.abs()),
            n if n.is_negative() => negate(&n),
            _ => return Ok(x),
        };
        Ok(self.number(n))
    }

    // --- comparison

    fn compare_reals(&self, x: Value, y: Value) -> Result<Option<Ordering>, NumError> {
        if let (Some(a), Some(b)) = (x.as_fixnum(), y.as_fixnum()) {
            return Ok(Some(a.cmp(&b)));
        }
        let (a, b) = (self.real_arg(x, 1)?, self.real_arg(y, 2)?);
        Ok(compare(&a, &b))
    }

    /// `=` of two numbers, complex ones too, by value: `(= 1 1.0)`.
    pub fn num_eq(&self, x: Value, y: Value) -> Result<bool, NumError> {
        if let (Some(a), Some(b)) = (x.as_fixnum(), y.as_fixnum()) {
            return Ok(a == b);
        }
        let (a, b) = (self.arg(x, 1)?, self.arg(y, 2)?);
        Ok(num_eq(&a, &b))
    }

    /// `<` of two reals, exact against inexact compared exactly; false
    /// with a NaN, like the others.
    pub fn less(&self, x: Value, y: Value) -> Result<bool, NumError> {
        Ok(self.compare_reals(x, y)? == Some(Ordering::Less))
    }

    pub fn greater(&self, x: Value, y: Value) -> Result<bool, NumError> {
        Ok(self.compare_reals(x, y)? == Some(Ordering::Greater))
    }

    pub fn less_eq(&self, x: Value, y: Value) -> Result<bool, NumError> {
        Ok(matches!(
            self.compare_reals(x, y)?,
            Some(Ordering::Less | Ordering::Equal)
        ))
    }

    pub fn greater_eq(&self, x: Value, y: Value) -> Result<bool, NumError> {
        Ok(matches!(
            self.compare_reals(x, y)?,
            Some(Ordering::Greater | Ordering::Equal)
        ))
    }

    pub fn max(&mut self, x: Value, y: Value) -> NumResult {
        self.max_min(x, y, true)
    }

    pub fn min(&mut self, x: Value, y: Value) -> NumResult {
        self.max_min(x, y, false)
    }

    fn max_min(&mut self, x: Value, y: Value, max: bool) -> NumResult {
        if let (Some(a), Some(b)) = (x.as_fixnum(), y.as_fixnum()) {
            return Ok(if (a < b) == max { y } else { x });
        }
        let (a, b) = (self.real_arg(x, 1)?, self.real_arg(y, 2)?);
        if a.is_exact() && b.is_exact() {
            let less = compare(&a, &b) == Some(Ordering::Less);
            return Ok(if less == max { y } else { x });
        }
        Ok(self.number(max_min(&a, &b, max)))
    }

    // --- division and rounding

    /// `quotient`, `remainder` and `modulo` of two integers, exact or not.
    pub fn quotient(&mut self, x: Value, y: Value) -> NumResult {
        Ok(self.integer_division(Rounding::Truncate, x, y)?.0)
    }

    pub fn remainder(&mut self, x: Value, y: Value) -> NumResult {
        Ok(self.integer_division(Rounding::Truncate, x, y)?.1)
    }

    pub fn modulo(&mut self, x: Value, y: Value) -> NumResult {
        Ok(self.integer_division(Rounding::Floor, x, y)?.1)
    }

    fn integer_division(
        &mut self,
        mode: Rounding,
        x: Value,
        y: Value,
    ) -> Result<(Value, Value), NumError> {
        if let (Some(a), Some(b)) = (x.as_fixnum(), y.as_fixnum())
            && b != 0
            && matches!(mode, Rounding::Truncate | Rounding::Floor)
        {
            let (mut q, mut r) = (a / b, a % b);
            if mode == Rounding::Floor && r != 0 && (r < 0) != (b < 0) {
                q -= 1;
                r += b;
            }
            return Ok((self.integer(q), self.integer(r)));
        }
        let (a, b) = (self.integer_arg(x, 1)?, self.integer_arg(y, 2)?);
        let (q, r) = divide_rounded(mode, &a, &b)?;
        Ok((self.number(q), self.number(r)))
    }

    /// The quotient and remainder of two reals, the quotient rounded as
    /// `mode` says: `floor/`, `truncate/` and the rest, and their
    /// `-quotient` and `-remainder` halves.
    pub fn divide(
        &mut self,
        mode: Rounding,
        x: Value,
        y: Value,
    ) -> Result<(Value, Value), NumError> {
        let (a, b) = (self.real_arg(x, 1)?, self.real_arg(y, 2)?);
        let (q, r) = divide_rounded(mode, &a, &b)?;
        Ok((self.number(q), self.number(r)))
    }

    /// `floor`, `ceiling`, `truncate` or `round` (ties to even) of a real.
    pub fn round(&mut self, mode: Rounding, x: Value) -> NumResult {
        let n = round_num(&self.real_arg(x, 1)?, mode);
        Ok(self.number(n))
    }

    pub fn gcd(&mut self, x: Value, y: Value) -> NumResult {
        self.gcd_lcm(x, y, true)
    }

    pub fn lcm(&mut self, x: Value, y: Value) -> NumResult {
        self.gcd_lcm(x, y, false)
    }

    /// Of exact integers, or inexact ones through their exact values.
    fn gcd_lcm(&mut self, x: Value, y: Value, gcd: bool) -> NumResult {
        let (a, b) = (self.integer_arg(x, 1)?, self.integer_arg(y, 2)?);
        let inexact = !a.is_exact() || !b.is_exact();
        let exact = |n: Num| match n {
            Real(x) => exact_of_f64(x),
            n => n,
        };
        let (Int(a), Int(b)) = (exact(a), exact(b)) else {
            unreachable!("an integer's exact value is an Int");
        };
        let result = if gcd {
            a.gcd(&b)
        } else if a.is_zero() || b.is_zero() {
            BigInt::zero()
        } else {
            a.mul(&b).abs().div_rem(&a.gcd(&b)).0
        };
        let result = if inexact {
            Real(result.to_f64())
        } else {
            Int(result)
        };
        Ok(self.number(result))
    }

    pub fn numerator(&mut self, x: Value) -> NumResult {
        self.fraction_part(x, true)
    }

    pub fn denominator(&mut self, x: Value) -> NumResult {
        self.fraction_part(x, false)
    }

    /// Of a flonum, the part of its exact value made inexact again; an
    /// integral or infinite flonum is its own numerator over 1.0.
    fn fraction_part(&mut self, x: Value, numerator: bool) -> NumResult {
        let n = match self.real_arg(x, 1)? {
            Int(_) if numerator => return Ok(x),
            Int(_) => Num::int(1),
            Ratio(n, d) => Int(if numerator { n } else { d }),
            Real(v) if v == libm::floor(v) => {
                if numerator {
                    return Ok(x);
                }
                Real(1.0)
            }
            Real(v) if v.is_nan() => return Err(NumError::OutOfRange(1)),
            Real(v) => {
                let (n, d) = exact_of_f64(v).parts();
                Real(if numerator { n } else { d }.to_f64())
            }
            Complex(..) => unreachable!("real_arg"),
        };
        Ok(self.number(n))
    }

    // --- exactness

    /// `inexact->exact`, R7RS's `exact`: the exact value of a finite
    /// flonum, or of a complex number with an imaginary part of 0.0.
    pub fn exact(&mut self, x: Value) -> NumResult {
        let v = match self.arg(x, 1)? {
            Int(_) | Ratio(..) => return Ok(x),
            Real(v) => v,
            Complex(re, 0.0) => re,
            Complex(..) => return Err(NumError::WrongType(1)),
        };
        if !v.is_finite() {
            return Err(NumError::OutOfRange(1));
        }
        Ok(self.number(exact_of_f64(v)))
    }

    /// `exact->inexact`, R7RS's `inexact`: the nearest double, ties to
    /// even.
    pub fn inexact(&mut self, x: Value) -> NumResult {
        let n = self.arg(x, 1)?;
        if !n.is_exact() {
            return Ok(x);
        }
        Ok(self.flonum(n.to_f64()))
    }

    // --- powers, roots, logarithms, trigonometry

    pub fn expt(&mut self, x: Value, y: Value) -> NumResult {
        let (a, b) = (self.arg(x, 1)?, self.arg(y, 2)?);
        let n = expt(&a, &b)?;
        Ok(self.number(n))
    }

    pub fn exp(&mut self, x: Value) -> NumResult {
        let n = exp(&self.arg(x, 1)?);
        Ok(self.number(n))
    }

    /// `log`; of an exact 0, an error.
    pub fn log(&mut self, x: Value) -> NumResult {
        let n = log(&self.arg(x, 1)?)?;
        Ok(self.number(n))
    }

    /// `sqrt`: exact for an exact square, `(sqrt 1/4)` is `1/2`; complex
    /// below zero.
    pub fn sqrt(&mut self, x: Value) -> NumResult {
        let n = sqrt(&self.arg(x, 1)?);
        Ok(self.number(n))
    }

    /// `exact-integer-sqrt`: `s` and `r` with `s² + r = k`.
    pub fn exact_integer_sqrt(&mut self, k: Value) -> Result<(Value, Value), NumError> {
        let k = self.exact_integer_arg(k, 1)?;
        if k.is_negative() {
            return Err(NumError::WrongType(1));
        }
        let (s, r) = k.isqrt();
        Ok((self.integer_big(s), self.integer_big(r)))
    }

    pub fn sin(&mut self, x: Value) -> NumResult {
        let n = sin(&self.arg(x, 1)?);
        Ok(self.number(n))
    }

    pub fn cos(&mut self, x: Value) -> NumResult {
        let n = cos(&self.arg(x, 1)?);
        Ok(self.number(n))
    }

    pub fn tan(&mut self, x: Value) -> NumResult {
        let n = tan(&self.arg(x, 1)?);
        Ok(self.number(n))
    }

    /// `asin`, complex outside [-1, 1].
    pub fn asin(&mut self, x: Value) -> NumResult {
        let n = asin(&self.arg(x, 1)?)?;
        Ok(self.number(n))
    }

    pub fn acos(&mut self, x: Value) -> NumResult {
        let n = acos(&self.arg(x, 1)?)?;
        Ok(self.number(n))
    }

    /// `(atan z)`.
    pub fn atan(&mut self, x: Value) -> NumResult {
        let n = atan(&self.arg(x, 1)?)?;
        Ok(self.number(n))
    }

    /// `(atan y x)`, of two reals.
    pub fn atan2(&mut self, y: Value, x: Value) -> NumResult {
        let (a, b) = (self.real_arg(y, 1)?, self.real_arg(x, 2)?);
        Ok(self.flonum(libm::atan2(a.to_f64(), b.to_f64())))
    }

    // --- complex numbers: (scheme complex)

    pub fn make_rectangular(&mut self, re: Value, im: Value) -> NumResult {
        let (a, b) = (self.real_arg(re, 1)?, self.real_arg(im, 2)?);
        Ok(self.number(rectangular(&a, &b)))
    }

    pub fn make_polar(&mut self, magnitude: Value, angle: Value) -> NumResult {
        let (a, b) = (self.real_arg(magnitude, 1)?, self.real_arg(angle, 2)?);
        Ok(self.number(make_polar(&a, &b)))
    }

    pub fn real_part(&mut self, z: Value) -> NumResult {
        match self.arg(z, 1)? {
            Complex(re, _) => Ok(self.flonum(re)),
            _ => Ok(z),
        }
    }

    /// `imag-part`: an exact 0 for a real.
    pub fn imag_part(&mut self, z: Value) -> NumResult {
        match self.arg(z, 1)? {
            Complex(_, im) => Ok(self.flonum(im)),
            _ => Ok(self.integer(0)),
        }
    }

    pub fn magnitude(&mut self, z: Value) -> NumResult {
        match self.arg(z, 1)? {
            Complex(re, im) => Ok(self.flonum(libm::hypot(re, im))),
            _ => self.abs(z),
        }
    }

    /// `angle`: π for a negative real, -0.0 included, and 0.0 otherwise.
    pub fn angle(&mut self, z: Value) -> NumResult {
        let angle = match self.arg(z, 1)? {
            Complex(re, im) => libm::atan2(im, re),
            Real(x) if x.is_sign_negative() => libm::atan2(0.0, -1.0),
            n if n.is_negative() => libm::atan2(0.0, -1.0),
            _ => 0.0,
        };
        Ok(self.flonum(angle))
    }

    // --- bits, of exact integers in two's complement

    /// `(ash n count)`: `n × 2^count`, rounded down when `count` is
    /// negative.
    pub fn ash(&mut self, n: Value, count: Value) -> NumResult {
        if let (Some(a), Some(c)) = (n.as_fixnum(), count.as_fixnum()) {
            if c <= 0 {
                return Ok(self.integer(a >> (-c).min(63)));
            }
            if c < 64 {
                return Ok(self.integer_i128((a as i128) << c));
            }
        }
        let (a, c) = (
            self.exact_integer_arg(n, 1)?,
            self.exact_integer_arg(count, 2)?,
        );
        if !c.is_negative() {
            if a.is_zero() {
                return Ok(n);
            }
            // Guile's limits: an unsigned long, and a bit count an int holds.
            let bits = c
                .to_i64()
                .filter(|&c| (c as u64) / 32 < i32::MAX as u64)
                .and_then(|c| usize::try_from(c).ok());
            return match bits {
                Some(bits) => Ok(self.integer_big(a.shl(bits))),
                None => Err(NumError::Overflow),
            };
        }
        let shifted = match c.neg().to_i64() {
            Some(c) => a.shr_floor(c as u64),
            None if a.is_negative() => BigInt::from_i64(-1),
            None => BigInt::zero(),
        };
        Ok(self.integer_big(shifted))
    }

    pub fn logand(&mut self, x: Value, y: Value) -> NumResult {
        self.bitwise(x, y, |a, b| a & b, BigInt::and)
    }

    pub fn logior(&mut self, x: Value, y: Value) -> NumResult {
        self.bitwise(x, y, |a, b| a | b, BigInt::or)
    }

    pub fn logxor(&mut self, x: Value, y: Value) -> NumResult {
        self.bitwise(x, y, |a, b| a ^ b, BigInt::xor)
    }

    fn bitwise(
        &mut self,
        x: Value,
        y: Value,
        small: fn(i64, i64) -> i64,
        big: fn(&BigInt, &BigInt) -> BigInt,
    ) -> NumResult {
        if let (Some(a), Some(b)) = (x.as_fixnum(), y.as_fixnum()) {
            return Ok(self.integer(small(a, b)));
        }
        let (a, b) = (self.exact_integer_arg(x, 1)?, self.exact_integer_arg(y, 2)?);
        Ok(self.integer_big(big(&a, &b)))
    }

    pub fn lognot(&mut self, x: Value) -> NumResult {
        let n = self.exact_integer_arg(x, 1)?;
        Ok(self.integer_big(n.not()))
    }

    /// `(logtest a b)`: whether `a` and `b` share a one bit. With a bignum
    /// among them, Guile 3.0.11 answers the opposite (`integers.c` tests
    /// that their `logand` is 0), and so does this.
    pub fn logtest(&self, x: Value, y: Value) -> Result<bool, NumError> {
        let (a, b) = (self.exact_integer_arg(x, 1)?, self.exact_integer_arg(y, 2)?);
        let shared = !a.and(&b).is_zero();
        Ok(shared == (x.tag() == Tag::Fixnum && y.tag() == Tag::Fixnum))
    }

    /// `(logbit? index n)`.
    pub fn logbit(&self, index: Value, n: Value) -> Result<bool, NumError> {
        let i = self.exact_integer_arg(index, 1)?;
        let n = self.exact_integer_arg(n, 2)?;
        match i.to_i64() {
            Some(i) if i >= 0 => Ok(n.bit(i as u64)),
            _ => Err(NumError::OutOfRange(1)),
        }
    }

    pub fn logcount(&mut self, x: Value) -> NumResult {
        let n = self.exact_integer_arg(x, 1)?;
        Ok(self.integer(n.count_ones() as i64))
    }

    pub fn integer_length(&mut self, x: Value) -> NumResult {
        let n = self.exact_integer_arg(x, 1)?;
        Ok(self.integer(n.bit_length() as i64))
    }

    // --- strings

    /// `(number->string n radix)`, radix 2 to 36, as Guile 3 writes it.
    /// The radix is checked first, as Guile does.
    pub fn number_to_string(&self, n: Value, radix: u32) -> Result<String, NumError> {
        if !(2..=36).contains(&radix) {
            return Err(NumError::OutOfRange(2));
        }
        let n = self.arg(n, 1)?;
        let mut out = String::new();
        write_num(&n, radix, &mut out);
        Ok(out)
    }

    /// `(string->number text radix)`: the number, or `#f`.
    pub fn string_to_number(&mut self, text: &str, radix: u32) -> NumResult {
        if radix < 2 {
            return Err(NumError::OutOfRange(2));
        }
        Ok(match parse(text, radix)? {
            Some(n) => self.number(n),
            None => Value::FALSE,
        })
    }
}
