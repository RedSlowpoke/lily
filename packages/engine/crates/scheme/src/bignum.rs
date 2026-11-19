//! Integers of any size (chapter 03 §5.1, R13): what a fixnum becomes when it
//! overflows. Sign and magnitude, the magnitude in 32-bit limbs with the
//! least significant first, so that wasm32's 64-bit multiply holds a limb's
//! product. Schoolbook multiplication and Knuth's division are enough: the
//! big numbers LilyPond's Scheme makes are a few limbs long.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

/// An integer of any size. The magnitude has no high zero limbs, and zero
/// has no limbs and no sign, so equal numbers are equal values.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct BigInt {
    neg: bool,
    mag: Vec<u32>,
}

// --- magnitudes ----------------------------------------------------------------

fn trim(mut mag: Vec<u32>) -> Vec<u32> {
    while mag.last() == Some(&0) {
        mag.pop();
    }
    mag
}

fn cmp_mag(a: &[u32], b: &[u32]) -> Ordering {
    a.len()
        .cmp(&b.len())
        .then_with(|| a.iter().rev().cmp(b.iter().rev()))
}

fn add_mag(a: &[u32], b: &[u32]) -> Vec<u32> {
    let (a, b) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let mut out = Vec::with_capacity(a.len() + 1);
    let mut carry = 0u64;
    for (i, &x) in a.iter().enumerate() {
        let sum = x as u64 + *b.get(i).unwrap_or(&0) as u64 + carry;
        out.push(sum as u32);
        carry = sum >> 32;
    }
    if carry != 0 {
        out.push(carry as u32);
    }
    out
}

/// `a - b`, for `a >= b`.
fn sub_mag(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0i64;
    for (i, &x) in a.iter().enumerate() {
        let diff = x as i64 - *b.get(i).unwrap_or(&0) as i64 - borrow;
        out.push(diff as u32);
        borrow = (diff < 0) as i64;
    }
    trim(out)
}

fn mul_mag(a: &[u32], b: &[u32]) -> Vec<u32> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let mut out = vec![0u32; a.len() + b.len()];
    for (i, &x) in a.iter().enumerate() {
        let mut carry = 0u64;
        for (j, &y) in b.iter().enumerate() {
            let t = x as u64 * y as u64 + out[i + j] as u64 + carry;
            out[i + j] = t as u32;
            carry = t >> 32;
        }
        out[i + b.len()] = carry as u32;
    }
    trim(out)
}

/// The quotient and remainder of a magnitude by one nonzero limb.
fn divrem_limb(a: &[u32], d: u32) -> (Vec<u32>, u32) {
    let mut q = vec![0u32; a.len()];
    let mut rem = 0u64;
    for i in (0..a.len()).rev() {
        let cur = (rem << 32) | a[i] as u64;
        q[i] = (cur / d as u64) as u32;
        rem = cur % d as u64;
    }
    (trim(q), rem as u32)
}

/// Knuth's algorithm D (TAOCP 4.3.1): the quotient and remainder of two
/// magnitudes, `b` nonzero.
fn divrem_mag(a: &[u32], b: &[u32]) -> (Vec<u32>, Vec<u32>) {
    if cmp_mag(a, b) == Ordering::Less {
        return (Vec::new(), a.to_vec());
    }
    if b.len() == 1 {
        let (q, r) = divrem_limb(a, b[0]);
        return (q, trim(vec![r]));
    }
    // Normalise so that the divisor's top limb has its high bit set.
    let shift = b[b.len() - 1].leading_zeros();
    let v = shl_mag(b, shift as usize);
    let mut u = shl_mag(a, shift as usize);
    u.resize(a.len() + 1, 0);
    let n = v.len();
    let m = u.len() - n;
    let mut q = vec![0u32; m];
    let top = v[n - 1] as u64;
    let next = v[n - 2] as u64;
    for j in (0..m).rev() {
        let num = ((u[j + n] as u64) << 32) | u[j + n - 1] as u64;
        let mut qhat = num / top;
        let mut rhat = num % top;
        while qhat >= 1 << 32 || qhat * next > ((rhat << 32) | u[j + n - 2] as u64) {
            qhat -= 1;
            rhat += top;
            if rhat >= 1 << 32 {
                break;
            }
        }
        // u[j..=j+n] -= qhat × v
        let mut borrow = 0i64;
        let mut carry = 0u64;
        for i in 0..n {
            let p = qhat * v[i] as u64 + carry;
            carry = p >> 32;
            let t = u[i + j] as i64 - borrow - (p & 0xffff_ffff) as i64;
            u[i + j] = t as u32;
            borrow = (t < 0) as i64;
        }
        let t = u[j + n] as i64 - borrow - carry as i64;
        u[j + n] = t as u32;
        if t < 0 {
            // qhat was one too large: add v back.
            qhat -= 1;
            let mut carry = 0u64;
            for i in 0..n {
                let s = u[i + j] as u64 + v[i] as u64 + carry;
                u[i + j] = s as u32;
                carry = s >> 32;
            }
            u[j + n] = u[j + n].wrapping_add(carry as u32);
        }
        q[j] = qhat as u32;
    }
    u.truncate(n);
    (trim(q), shr_mag(&trim(u), shift as usize))
}

fn shl_mag(a: &[u32], bits: usize) -> Vec<u32> {
    if a.is_empty() {
        return Vec::new();
    }
    let (limbs, bits) = (bits / 32, bits % 32);
    let mut out = vec![0u32; limbs];
    if bits == 0 {
        out.extend_from_slice(a);
    } else {
        let mut carry = 0u32;
        for &x in a {
            out.push((x << bits) | carry);
            carry = x >> (32 - bits);
        }
        out.push(carry);
    }
    trim(out)
}

/// The magnitude shifted right, the bits shifted out dropped.
fn shr_mag(a: &[u32], bits: usize) -> Vec<u32> {
    let (limbs, bits) = (bits / 32, bits % 32);
    if limbs >= a.len() {
        return Vec::new();
    }
    let a = &a[limbs..];
    if bits == 0 {
        return a.to_vec();
    }
    let mut out = Vec::with_capacity(a.len());
    for i in 0..a.len() {
        let high = a.get(i + 1).map_or(0, |&h| h << (32 - bits));
        out.push((a[i] >> bits) | high);
    }
    trim(out)
}

fn bit_length_mag(a: &[u32]) -> u64 {
    match a.last() {
        None => 0,
        Some(&top) => (a.len() as u64 - 1) * 32 + (32 - top.leading_zeros()) as u64,
    }
}

/// Whether any of the low `bits` bits is set.
fn low_bits_set(a: &[u32], bits: u64) -> bool {
    // Saturating, so that wasm32's usize does not wrap a huge count.
    let limbs = usize::try_from(bits / 32).unwrap_or(usize::MAX);
    a.iter().take(limbs).any(|&x| x != 0)
        || (!bits.is_multiple_of(32)
            && a.get(limbs)
                .is_some_and(|&x| x & ((1 << (bits % 32)) - 1) != 0))
}

// --- integers -------------------------------------------------------------------

impl BigInt {
    pub const fn zero() -> BigInt {
        BigInt {
            neg: false,
            mag: Vec::new(),
        }
    }

    fn from_parts(neg: bool, mag: Vec<u32>) -> BigInt {
        let mag = trim(mag);
        BigInt {
            neg: neg && !mag.is_empty(),
            mag,
        }
    }

    /// Its sign and limbs, for the image.
    pub(crate) fn save(&self, w: &mut crate::image::ImageWriter) {
        w.bool(self.neg);
        w.count(self.mag.len());
        for &limb in &self.mag {
            w.u32(limb);
        }
    }

    pub(crate) fn load(r: &mut crate::image::ImageReader<'_>) -> Option<BigInt> {
        let neg = r.bool()?;
        let mag = (0..r.count()?).map(|_| r.u32()).collect::<Option<_>>()?;
        Some(BigInt::from_parts(neg, mag))
    }

    pub fn from_u64(n: u64) -> BigInt {
        BigInt::from_parts(false, vec![n as u32, (n >> 32) as u32])
    }

    pub fn from_i64(n: i64) -> BigInt {
        let mut big = BigInt::from_u64(n.unsigned_abs());
        big.neg = n < 0;
        big
    }

    pub fn from_i128(n: i128) -> BigInt {
        let m = n.unsigned_abs();
        let limbs = (0..4).map(|i| (m >> (32 * i)) as u32).collect();
        BigInt::from_parts(n < 0, limbs)
    }

    /// The value, if it fits an `i64`.
    pub fn to_i64(&self) -> Option<i64> {
        if self.mag.len() > 2 {
            return None;
        }
        let m = self
            .mag
            .iter()
            .rev()
            .fold(0u64, |m, &x| (m << 32) | x as u64);
        if self.neg {
            0i64.checked_sub_unsigned(m)
        } else {
            i64::try_from(m).ok()
        }
    }

    pub fn is_zero(&self) -> bool {
        self.mag.is_empty()
    }

    pub fn is_negative(&self) -> bool {
        self.neg
    }

    pub fn is_odd(&self) -> bool {
        self.mag.first().is_some_and(|&x| x & 1 == 1)
    }

    /// Whether this is 1.
    pub fn is_one(&self) -> bool {
        !self.neg && self.mag == [1]
    }

    pub fn signum(&self) -> i32 {
        if self.neg {
            -1
        } else {
            !self.mag.is_empty() as i32
        }
    }

    pub fn neg(&self) -> BigInt {
        BigInt::from_parts(!self.neg, self.mag.clone())
    }

    pub fn abs(&self) -> BigInt {
        BigInt::from_parts(false, self.mag.clone())
    }

    pub fn add(&self, other: &BigInt) -> BigInt {
        if self.neg == other.neg {
            return BigInt::from_parts(self.neg, add_mag(&self.mag, &other.mag));
        }
        match cmp_mag(&self.mag, &other.mag) {
            Ordering::Less => BigInt::from_parts(other.neg, sub_mag(&other.mag, &self.mag)),
            _ => BigInt::from_parts(self.neg, sub_mag(&self.mag, &other.mag)),
        }
    }

    pub fn sub(&self, other: &BigInt) -> BigInt {
        self.add(&other.neg())
    }

    pub fn mul(&self, other: &BigInt) -> BigInt {
        BigInt::from_parts(self.neg != other.neg, mul_mag(&self.mag, &other.mag))
    }

    /// The quotient rounded toward zero, and the remainder with the
    /// dividend's sign. `other` must not be zero.
    pub fn div_rem(&self, other: &BigInt) -> (BigInt, BigInt) {
        assert!(!other.is_zero(), "division by zero");
        let (q, r) = divrem_mag(&self.mag, &other.mag);
        (
            BigInt::from_parts(self.neg != other.neg, q),
            BigInt::from_parts(self.neg, r),
        )
    }

    /// The quotient rounded toward minus infinity, and the remainder with
    /// the divisor's sign. `other` must not be zero.
    pub fn div_rem_floor(&self, other: &BigInt) -> (BigInt, BigInt) {
        let (q, r) = self.div_rem(other);
        if !r.is_zero() && r.neg != other.neg {
            (q.sub(&BigInt::from_i64(1)), r.add(other))
        } else {
            (q, r)
        }
    }

    /// Whether `other` divides this, the remainder zero.
    pub fn divides(&self, other: &BigInt) -> bool {
        other.div_rem(self).1.is_zero()
    }

    /// The greatest common divisor, never negative; `gcd(0, 0)` is 0.
    pub fn gcd(&self, other: &BigInt) -> BigInt {
        let (mut a, mut b) = (self.abs(), other.abs());
        while !b.is_zero() {
            let r = a.div_rem(&b).1;
            a = b;
            b = r;
        }
        a
    }

    pub fn pow(&self, mut k: u64) -> BigInt {
        let mut base = self.clone();
        let mut acc = BigInt::from_i64(1);
        while k > 0 {
            if k & 1 == 1 {
                acc = acc.mul(&base);
            }
            k >>= 1;
            if k > 0 {
                base = base.mul(&base);
            }
        }
        acc
    }

    /// `self × 2^bits`.
    pub fn shl(&self, bits: usize) -> BigInt {
        BigInt::from_parts(self.neg, shl_mag(&self.mag, bits))
    }

    /// `floor(self / 2^bits)`: `ash` to the right, which rounds negative
    /// numbers down.
    pub fn shr_floor(&self, bits: u64) -> BigInt {
        let shifted = match usize::try_from(bits) {
            Ok(bits) => shr_mag(&self.mag, bits),
            Err(_) => Vec::new(),
        };
        let shifted = BigInt::from_parts(self.neg, shifted);
        if self.neg && low_bits_set(&self.mag, bits) {
            shifted.sub(&BigInt::from_i64(1))
        } else {
            shifted
        }
    }

    /// `integer-length`: the bits of the two's complement without its sign.
    pub fn bit_length(&self) -> u64 {
        if self.neg {
            self.not().bit_length()
        } else {
            bit_length_mag(&self.mag)
        }
    }

    /// `logcount`: the one bits of a positive number, the zero bits of a
    /// negative one.
    pub fn count_ones(&self) -> u64 {
        if self.neg {
            self.not().count_ones()
        } else {
            self.mag.iter().map(|x| x.count_ones() as u64).sum()
        }
    }

    /// The low zero bits, `None` for zero.
    pub fn trailing_zeros(&self) -> Option<u64> {
        let i = self.mag.iter().position(|&x| x != 0)?;
        Some(i as u64 * 32 + self.mag[i].trailing_zeros() as u64)
    }

    /// `logbit?`: bit `n` of the two's complement.
    pub fn bit(&self, n: u64) -> bool {
        let limb = n / 32;
        if limb >= self.mag.len() as u64 {
            return self.neg;
        }
        self.twos(self.mag.len() + 1)[limb as usize] >> (n % 32) & 1 == 1
    }

    /// The two's complement in `len` limbs, enough to hold it with its sign.
    fn twos(&self, len: usize) -> Vec<u32> {
        let mut out = self.mag.clone();
        out.resize(len, 0);
        if self.neg {
            let mut carry = 1u64;
            for x in &mut out {
                let t = (!*x) as u64 + carry;
                *x = t as u32;
                carry = t >> 32;
            }
        }
        out
    }

    fn from_twos(mut limbs: Vec<u32>) -> BigInt {
        let neg = limbs.last().is_some_and(|&x| x >> 31 == 1);
        if neg {
            let mut carry = 1u64;
            for x in &mut limbs {
                let t = (!*x) as u64 + carry;
                *x = t as u32;
                carry = t >> 32;
            }
        }
        BigInt::from_parts(neg, limbs)
    }

    fn bitwise(&self, other: &BigInt, op: fn(u32, u32) -> u32) -> BigInt {
        let len = self.mag.len().max(other.mag.len()) + 1;
        let (a, b) = (self.twos(len), other.twos(len));
        BigInt::from_twos(a.iter().zip(&b).map(|(&x, &y)| op(x, y)).collect())
    }

    pub fn and(&self, other: &BigInt) -> BigInt {
        self.bitwise(other, |x, y| x & y)
    }

    pub fn or(&self, other: &BigInt) -> BigInt {
        self.bitwise(other, |x, y| x | y)
    }

    pub fn xor(&self, other: &BigInt) -> BigInt {
        self.bitwise(other, |x, y| x ^ y)
    }

    /// `lognot`: `-1 - self`.
    pub fn not(&self) -> BigInt {
        BigInt::from_i64(-1).sub(self)
    }

    /// `self × m + a`, for reading digits.
    pub fn mul_add_small(&self, m: u32, a: u32) -> BigInt {
        let mut out = Vec::with_capacity(self.mag.len() + 1);
        let mut carry = a as u64;
        for &x in &self.mag {
            let t = x as u64 * m as u64 + carry;
            out.push(t as u32);
            carry = t >> 32;
        }
        out.push(carry as u32);
        BigInt::from_parts(self.neg, out)
    }

    /// The floor of the square root and the rest, `s² + r = self`, for
    /// `self >= 0`: Newton's iteration from above.
    pub fn isqrt(&self) -> (BigInt, BigInt) {
        if self.is_zero() {
            return (BigInt::zero(), BigInt::zero());
        }
        let mut x = BigInt::from_i64(1).shl(self.bit_length().div_ceil(2) as usize);
        loop {
            let y = x.add(&self.div_rem(&x).0).shr_floor(1);
            if y >= x {
                break;
            }
            x = y;
        }
        let rest = self.sub(&x.mul(&x));
        (x, rest)
    }

    /// The significand in [0.5, 1) and the exponent of the nearest double,
    /// ties to even, as C's `frexp` would give them; `(0.0, 0)` for zero.
    /// The exponent is not limited to a double's.
    pub fn frexp(&self) -> (f64, i64) {
        let bits = bit_length_mag(&self.mag);
        if bits == 0 {
            return (0.0, 0);
        }
        // The top 64 bits, with a sticky bit for the ones below.
        let shift = bits.saturating_sub(64);
        let top = shr_mag(&self.mag, shift as usize);
        let mut m = top.iter().rev().fold(0u64, |m, &x| (m << 32) | x as u64);
        let sticky = low_bits_set(&self.mag, shift);
        // Round m to 53 significant bits.
        let width = 64 - m.leading_zeros() as u64;
        let mut exp = shift as i64 + width as i64;
        if width > 53 {
            let drop = width - 53;
            let half = 1u64 << (drop - 1);
            let rest = m & ((1u64 << drop) - 1);
            m >>= drop;
            if rest > half || (rest == half && (sticky || m & 1 == 1)) {
                m += 1;
                if m == 1 << 53 {
                    m >>= 1;
                    exp += 1;
                }
            }
            let significand = m as f64 / (1u64 << 53) as f64;
            return (if self.neg { -significand } else { significand }, exp);
        }
        let significand = libm::ldexp(m as f64, -(width as i32));
        (if self.neg { -significand } else { significand }, exp)
    }

    /// The nearest double, ties to even; infinite beyond the doubles.
    pub fn to_f64(&self) -> f64 {
        let (significand, exp) = self.frexp();
        libm::ldexp(significand, exp.clamp(-2000, 2000) as i32)
    }

    /// The integer a finite, integral double holds.
    pub fn from_f64(x: f64) -> BigInt {
        debug_assert!(x.is_finite() && libm::trunc(x) == x);
        if x.abs() < 9.2e18 {
            return BigInt::from_i64(x as i64);
        }
        let bits = x.to_bits();
        let exp = ((bits >> 52) & 0x7ff) as i64 - 1075;
        let mantissa = (bits & ((1 << 52) - 1)) | (1 << 52);
        let big = BigInt::from_u64(mantissa).shl(exp as usize);
        if x < 0.0 { big.neg() } else { big }
    }

    /// The digits in `radix`, 2 to 36, with a `-` before a negative number.
    pub fn to_string_radix(&self, radix: u32) -> String {
        debug_assert!((2..=36).contains(&radix));
        if self.is_zero() {
            return String::from("0");
        }
        // Take off as many digits per division as fit in a limb.
        let mut chunk = radix;
        let mut per = 1;
        while let Some(next) = chunk.checked_mul(radix) {
            chunk = next;
            per += 1;
        }
        let mut digits = Vec::new();
        let mut mag = self.mag.clone();
        while !mag.is_empty() {
            let (q, mut r) = divrem_limb(&mag, chunk);
            mag = q;
            for _ in 0..per {
                digits.push(DIGITS[(r % radix) as usize]);
                r /= radix;
                if mag.is_empty() && r == 0 {
                    break;
                }
            }
        }
        while digits.len() > 1 && digits.last() == Some(&b'0') {
            digits.pop();
        }
        if self.neg {
            digits.push(b'-');
        }
        digits.reverse();
        String::from_utf8(digits).unwrap_or_default()
    }
}

/// Guile's `number_chars`.
pub(crate) const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

impl Ord for BigInt {
    fn cmp(&self, other: &BigInt) -> Ordering {
        match (self.neg, other.neg) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => cmp_mag(&self.mag, &other.mag),
            (true, true) => cmp_mag(&other.mag, &self.mag),
        }
    }
}

impl PartialOrd for BigInt {
    fn partial_cmp(&self, other: &BigInt) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<i64> for BigInt {
    fn from(n: i64) -> BigInt {
        BigInt::from_i64(n)
    }
}

impl fmt::Display for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_radix(10))
    }
}

impl fmt::Debug for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
