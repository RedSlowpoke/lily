//! The page's numbers (06 §6.2): what LilyPond prints with `ly:format`'s
//! `~4f` (`~2f` for the page's size), then without trailing zeros and the
//! leading `0`: `12.0000` is `12`, `-0.5000` is `-.5`, `-0.0000` is `0`. The
//! same value LilyPond writes, in fewer characters.
//!
//! A [`Fixed`] keeps the printed value in ten-thousandths, so that a
//! position folded into an element's own attributes is the sum of the two
//! numbers LilyPond prints, the group's `translate` and the element's, and
//! draws where LilyPond's page draws it (06 §3.3).

use alloc::format;
use alloc::string::String;
use core::fmt;
use core::ops::{Add, Neg};

/// A number as LilyPond prints it, in ten-thousandths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fixed(pub i64);

impl Fixed {
    pub const ZERO: Fixed = Fixed(0);

    /// `(ly:format "~4f" x)`: `%.4f`, and `format_single_argument`'s 0 for
    /// an infinity or a NaN.
    pub fn of(x: f64) -> Fixed {
        Fixed::with(x, 4)
    }

    /// `(ly:format "~<digits>f" x)`, `digits` at most 4.
    pub fn with(x: f64, digits: usize) -> Fixed {
        if !x.is_finite() {
            return Fixed::ZERO;
        }
        // printf's rounding of the binary value, which Rust's is.
        let text = format!("{x:.digits$}");
        let (negative, text) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.as_str()),
        };
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        let mut n: i64 = whole.parse().unwrap_or(i64::MAX / 10_000);
        let mut fraction = fraction.bytes();
        for _ in 0..4 {
            let digit = fraction.next().map_or(0, |b| i64::from(b - b'0'));
            n = n.saturating_mul(10).saturating_add(digit);
        }
        Fixed(if negative { -n } else { n })
    }
}

impl Add for Fixed {
    type Output = Fixed;

    fn add(self, other: Fixed) -> Fixed {
        Fixed(self.0.saturating_add(other.0))
    }
}

impl Neg for Fixed {
    type Output = Fixed;

    fn neg(self) -> Fixed {
        Fixed(-self.0)
    }
}

impl fmt::Display for Fixed {
    /// Without trailing zeros and the leading `0`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 < 0 {
            f.write_str("-")?;
        }
        let n = self.0.unsigned_abs();
        let (whole, mut fraction) = (n / 10_000, n % 10_000);
        if whole != 0 || fraction == 0 {
            write!(f, "{whole}")?;
        }
        if fraction != 0 {
            let mut digits = 4;
            while fraction % 10 == 0 {
                fraction /= 10;
                digits -= 1;
            }
            write!(f, ".{fraction:0digits$}")?;
        }
        Ok(())
    }
}

/// `text` with each decimal number in it (`-?\d+\.\d+`) written as
/// [`Fixed`]'s are: for what LilyPond prints with `~a`, Guile's `display`
/// of a number, as `stroke-dasharray`'s.
pub fn compact(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        let sign = usize::from(bytes[at] == b'-');
        let digits = |from: usize| {
            bytes[from..]
                .iter()
                .take_while(|b| b.is_ascii_digit())
                .count()
        };
        let whole = digits(at + sign);
        let point = at + sign + whole;
        let fraction = if whole > 0 && bytes.get(point) == Some(&b'.') {
            digits(point + 1)
        } else {
            0
        };
        // A number's digits do not begin inside another's.
        let inside = start > 0 && bytes[start - 1].is_ascii_digit();
        if fraction == 0 || inside {
            let len = text[at..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&text[at..at + len]);
            at += len;
            continue;
        }
        let end = point + 1 + fraction;
        let number = &text[start..end];
        let mut trimmed = number.trim_end_matches('0').trim_end_matches('.');
        if trimmed == "-0" || trimmed.is_empty() {
            trimmed = "0";
        }
        if let Some(rest) = trimmed.strip_prefix("0.") {
            out.push('.');
            out.push_str(rest);
        } else if let Some(rest) = trimmed.strip_prefix("-0.") {
            out.push_str("-.");
            out.push_str(rest);
        } else {
            out.push_str(trimmed);
        }
        at = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn numbers_as_lilypond_prints_them_without_zeros() {
        let cases: &[(f64, &str)] = &[
            (12.0, "12"),
            (-0.5, "-.5"),
            (-0.0, "0"),
            (-0.00004, "0"),
            (0.05, ".05"),
            (14.6906, "14.6906"),
            (8.535826771653543, "8.5358"),
            (0.0040, ".004"),
            (-16.794776771653538, "-16.7948"),
            (100.0, "100"),
            // printf's ties go to the even digit.
            (0.03125, ".0312"),
            (f64::INFINITY, "0"),
            (f64::NAN, "0"),
        ];
        for &(x, text) in cases {
            assert_eq!(Fixed::of(x).to_string(), text, "{x}");
        }
        assert_eq!(Fixed::with(210.0, 2).to_string(), "210");
        assert_eq!(Fixed::with(296.999, 2).to_string(), "297");
    }

    #[test]
    fn a_folded_position_is_the_sum_of_the_printed_ones() {
        // translate(8.5358, …) and x="16.7948": LilyPond's page draws at 25.3306.
        let x = Fixed::of(8.535826771653543) + Fixed::of(16.794776771653538);
        assert_eq!(x.to_string(), "25.3306");
        assert_eq!((-Fixed::of(0.05)).to_string(), "-.05");
    }

    #[test]
    fn displayed_numbers_compacted() {
        assert_eq!(compact("0.5,1.0"), ".5,1");
        assert_eq!(compact("-0.25 3 10.50"), "-.25 3 10.5");
        assert_eq!(compact("1.0e-5"), "1e-5");
        assert_eq!(compact("a1.5"), "a1.5");
        assert_eq!(compact("rgba(100.0000%, -0.0000%)"), "rgba(100%, 0%)");
    }
}
