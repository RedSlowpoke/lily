//! `a * b + c` rounded once, as clang's `fmadd` computes it where it fuses
//! the expression (`-ffp-contract=on`). arm64 has the instruction; wasm has
//! none, and libm's `fma` there is a software one in integers, 17 % of T13's
//! compile at MS3 step 57 (OPEN A42). [`fma`] is as exact and cheaper: the
//! product and the sum without error (Dekker's, Knuth's) and their low parts
//! added rounded to odd, which Boldo and Melquiond prove rounds once
//! ("Emulation of FMA and correctly-rounded sums: proved algorithms using
//! rounding to odd", IEEE Transactions on Computers 57(4), 2008), so long as
//! nothing underflows or overflows. Arguments that could are libm's.

/// `a * b + c` rounded once: `fmadd` on arm64, else [`emulated`]. In a
/// debug build each result is checked against [`emulated`]'s, so the tests
/// check the emulation on every fused multiply-add they reach.
#[inline]
pub fn fma(a: f64, b: f64, c: f64) -> f64 {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    let r = {
        let mut r = a;
        // SAFETY: `fmadd` is available with neon and has no side effects.
        unsafe {
            core::arch::asm!(
                "fmadd {r:d}, {r:d}, {b:d}, {c:d}",
                r = inout(vreg) r,
                b = in(vreg) b,
                c = in(vreg) c,
                options(nomem, nostack, pure)
            );
        }
        debug_assert!(same(r, emulated(a, b, c)), "fma({a:e}, {b:e}, {c:e})");
        r
    };
    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    let r = emulated(a, b, c);
    r
}

/// The same value, NaN as any NaN.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
fn same(x: f64, y: f64) -> bool {
    x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan())
}

/// The unbiased exponent of a normal number; -1023 for 0 and subnormals,
/// 1024 for infinities and NaN.
fn exponent(x: f64) -> i32 {
    ((x.to_bits() >> 52) & 0x7ff) as i32 - 1023
}

/// [`fma`] without the instruction. A product of exponents within ±450 and
/// a `c` within ±900, or 0, can neither overflow nor lose a bit below the
/// smallest normal number, in the split, the product's low part or the sum;
/// the rest is libm's.
pub fn emulated(a: f64, b: f64, c: f64) -> f64 {
    let (ea, eb, ec) = (exponent(a), exponent(b), exponent(c));
    if a == 0.0 || b == 0.0 {
        // The product is exact: ±0 with the sign the fused one takes.
        return a * b + c;
    }
    if !(-450..=450).contains(&ea)
        || !(-450..=450).contains(&eb)
        || (c != 0.0 && !(-900..=900).contains(&ec))
    {
        return libm::fma(a, b, c);
    }
    let (ph, pl) = two_product(a, b);
    if pl == 0.0 {
        return ph + c;
    }
    let (sh, sl) = two_sum(c, ph);
    sh + add_odd(sl, pl)
}

/// Veltkamp's split: `x` as `hi + lo`, each of 26 significant bits or fewer.
#[inline]
fn split(x: f64) -> (f64, f64) {
    let t = 134_217_729.0 * x; // 2^27 + 1
    let hi = t - (t - x);
    (hi, x - hi)
}

/// Dekker's product: `a * b` exactly, as its rounding and the error.
#[inline]
fn two_product(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    let (ah, al) = split(a);
    let (bh, bl) = split(b);
    (p, ((ah * bh - p) + ah * bl + al * bh) + al * bl)
}

/// Knuth's sum: `a + b` exactly, as its rounding and the error.
#[inline]
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    (s, (a - (s - bb)) + (b - bb))
}

/// `a + b` rounded to odd: the exact sum, or the one of the two numbers
/// around it whose last bit is 1.
#[inline]
fn add_odd(a: f64, b: f64) -> f64 {
    let (s, e) = two_sum(a, b);
    let bits = s.to_bits();
    if e == 0.0 || bits & 1 == 1 {
        s
    } else if (e > 0.0) == (s > 0.0) {
        f64::from_bits(bits + 1)
    } else {
        f64::from_bits(bits - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// xorshift64*, so the cases are the same at every run.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }

        /// A number of either sign with a random significand and an
        /// exponent in `-e..e`.
        fn number(&mut self, e: i64) -> f64 {
            let r = self.next();
            let exp = (1023 + (r % (2 * e as u64)) as i64 - e) as u64;
            f64::from_bits((r & (1 << 63)) | (exp << 52) | (self.next() & ((1 << 52) - 1)))
        }

        /// A significand of few bits, so products are often exact or halfway.
        fn short(&mut self) -> f64 {
            let r = self.next();
            let m = (r >> 40) as i64 % 4096 - 2048;
            m as f64 * libm::ldexp(1.0, (r % 40) as i32 - 20)
        }
    }

    fn check(a: f64, b: f64, c: f64) {
        let (want, got) = (libm::fma(a, b, c), emulated(a, b, c));
        assert!(
            same(want, got),
            "fma({a:e}, {b:e}, {c:e}): libm {want:e}, emulated {got:e}"
        );
    }

    #[test]
    fn rounds_as_libm_does() {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        for _ in 0..2_000_000 {
            let (a, b) = (rng.number(60), rng.number(60));
            let c = rng.number(120);
            check(a, b, c);
            // Cancelling: c near -a * b, a few units of its last place off.
            let p = -(a * b);
            let off = (rng.next() % 9) as i64 - 4;
            check(a, b, f64::from_bits((p.to_bits() as i64 + off) as u64));
            let (x, y, z) = (rng.short(), rng.short(), rng.short());
            check(x, y, z);
            check(x, y, -(x * y));
            check(a, y, z);
        }
    }

    /// `a * b` near half a unit of `c`'s last place, off by less than the
    /// product's rounding: rounding the low parts to nearest instead of to
    /// odd rounds twice and gets these wrong.
    #[test]
    fn rounds_once_near_halfway() {
        let mut rng = Rng(0x2545_f491_4f6c_dd1d);
        for _ in 0..1_000_000 {
            let c = rng.number(60);
            let half = libm::ldexp(1.0, exponent(c) - 53);
            let k = (rng.next() % 17) as f64 - 8.0;
            let j = (rng.next() % 17) as f64 - 8.0;
            let ulp = libm::ldexp(1.0, -52);
            let (a, b) = (half * (1.0 + k * ulp), 1.0 + j * ulp);
            check(a, b, c);
            check(-a, b, c);
            check(a, b, -c);
        }
    }

    #[test]
    fn edges_go_to_libm() {
        let specials = [
            0.0,
            -0.0,
            1.0,
            -1.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
            f64::MAX,
            f64::MIN_POSITIVE,
            5e-324,
            1e300,
            1e-300,
            1e6,
            -1e-6,
        ];
        for &a in &specials {
            for &b in &specials {
                for &c in &specials {
                    check(a, b, c);
                }
            }
        }
    }
}
