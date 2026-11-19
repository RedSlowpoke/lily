//! `spring.cc` and `spring-smob.cc` (part 3's, MS3 step 29's in part 3's
//! order): LilyPond's spring, its merging and its smob. What
//! `page-layout-problem.cc`'s solver needs of one was ported first by MS3
//! step 28; step 29 added `Spring ()`, `operator*=`, `operator>`,
//! `merge_springs`, `set_blocking_force`, the default strengths one by one
//! and the smob face.
//!
//! A spring's length under a force `f` is `ideal + f × flexibility`, the
//! flexibility the inverse stretch strength for `f` ≥ 0 and the inverse
//! compress strength below, and never less than its minimum distance.
//! LilyPond's programming errors for insane values are pushed to
//! `errors`, the spring left as it was.
//!
//! A spring is a Scheme value as a [`SpringObject`] (`ly:spring?`), made
//! by `ly:make-spring` and changed by `ly:spring-set-inverse-…-strength!`,
//! as `spring-smob.cc` makes them. The page-layout problem's Scheme
//! (`page_layout_problem.rs`) makes its own with `%make-spring` and
//! changes them with `%spring-set!`.

use alloc::vec::Vec;

use lily_engine_scheme::{Cx, Foreign, Value};

use crate::R;
use crate::geometry::{fma, max};

/// A `Spring`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub ideal_distance: f64,
    pub min_distance: f64,
    pub inverse_stretch_strength: f64,
    pub inverse_compress_strength: f64,
    /// The force below which the length is the minimum distance.
    pub blocking_force: f64,
}

/// LilyPond's programming errors, as a spring's setters report them.
pub type Errors = Vec<&'static str>;

/// `Spring ()`: 1 long, at its minimum, of strength 1.
impl Default for Spring {
    fn default() -> Spring {
        let mut s = Spring {
            ideal_distance: 1.0,
            min_distance: 1.0,
            inverse_stretch_strength: 1.0,
            inverse_compress_strength: 1.0,
            blocking_force: 0.0,
        };
        s.update_blocking_force();
        s
    }
}

impl Spring {
    /// `Spring (dist, min_dist)`: the distances set (or refused) in turn,
    /// then the default strengths.
    pub fn new(dist: f64, min_dist: f64, errors: &mut Errors) -> Spring {
        let mut s = Spring::default();
        s.set_ideal_distance(dist, errors);
        s.set_min_distance(min_dist, errors);
        s.set_default_strength();
        s.update_blocking_force();
        s
    }

    /// `update_blocking_force`: the force where the length leaves the
    /// minimum distance, 0 for a spring that cannot move toward it.
    fn update_blocking_force(&mut self) {
        self.blocking_force = if self.min_distance > self.ideal_distance {
            if self.inverse_stretch_strength > 0.0 {
                (self.min_distance - self.ideal_distance) / self.inverse_stretch_strength
            } else {
                0.0
            }
        } else if self.inverse_compress_strength > 0.0 {
            (self.min_distance - self.ideal_distance) / self.inverse_compress_strength
        } else {
            0.0
        };
    }

    pub fn set_ideal_distance(&mut self, d: f64, errors: &mut Errors) {
        if d < 0.0 || !d.is_finite() {
            errors.push("insane spring distance requested, ignoring it");
        } else {
            self.ideal_distance = d;
            self.update_blocking_force();
        }
    }

    pub fn set_min_distance(&mut self, d: f64, errors: &mut Errors) {
        if d < 0.0 || !d.is_finite() {
            errors.push("insane spring min_distance requested, ignoring it");
        } else {
            self.min_distance = d;
            self.update_blocking_force();
        }
    }

    /// `ensure_min_distance`: the minimum distance raised to `d`.
    pub fn ensure_min_distance(&mut self, d: f64, errors: &mut Errors) {
        self.set_min_distance(max(d, self.min_distance), errors);
    }

    pub fn set_inverse_stretch_strength(&mut self, f: f64, errors: &mut Errors) {
        if !f.is_finite() || f < 0.0 {
            errors.push("insane spring constant");
        } else {
            self.inverse_stretch_strength = f;
        }
        self.update_blocking_force();
    }

    pub fn set_inverse_compress_strength(&mut self, f: f64, errors: &mut Errors) {
        if !f.is_finite() || f < 0.0 {
            errors.push("insane spring constant");
        } else {
            self.inverse_compress_strength = f;
        }
        self.update_blocking_force();
    }

    /// `set_blocking_force`: the minimum distance raised or lowered to the
    /// length under `f`, so that the spring blocks there.
    pub fn set_blocking_force(&mut self, f: f64, errors: &mut Errors) {
        if !f.is_finite() {
            errors.push("insane blocking force");
            return;
        }
        self.blocking_force = f64::NEG_INFINITY;
        self.min_distance = self.length(f, errors);
        self.update_blocking_force();
    }

    /// `set_default_strength`: stretching as flexible as the spring is
    /// long, compressing as its room above the minimum.
    pub fn set_default_strength(&mut self) {
        self.set_default_stretch_strength();
        self.set_default_compress_strength();
    }

    /// `set_default_compress_strength`.
    pub fn set_default_compress_strength(&mut self) {
        self.inverse_compress_strength = if self.ideal_distance >= self.min_distance {
            self.ideal_distance - self.min_distance
        } else {
            0.0
        };
        self.update_blocking_force();
    }

    /// `set_default_stretch_strength`, which leaves the blocking force as
    /// it was.
    pub fn set_default_stretch_strength(&mut self) {
        self.inverse_stretch_strength = self.ideal_distance;
    }

    /// `operator>`: whether the spring blocks at a greater force.
    pub fn is_greater(&self, other: &Spring) -> bool {
        self.blocking_force > other.blocking_force
    }

    /// `length`: the length under the force `f`, not below the blocking
    /// force; "cruelty to springs" and the ideal length for an infinite one.
    /// LilyPond's clang fuses the multiply-add (`fmadd`).
    pub fn length(&self, f: f64, errors: &mut Errors) -> f64 {
        let mut force = max(f, self.blocking_force);
        let inv_k = if force < 0.0 {
            self.inverse_compress_strength
        } else {
            self.inverse_stretch_strength
        };
        if force.is_infinite() {
            errors.push("cruelty to springs");
            force = 0.0;
        }
        max(self.min_distance, fma(force, inv_k, self.ideal_distance))
    }
}

/// `operator*=`: the spring scaled by `r`, not below its minimum distance,
/// compressing as its new room above the minimum.
impl core::ops::MulAssign<f64> for Spring {
    fn mul_assign(&mut self, r: f64) {
        self.ideal_distance = max(self.min_distance, self.ideal_distance * r);
        self.inverse_compress_strength = max(0.0, self.ideal_distance - self.min_distance);
        self.inverse_stretch_strength *= r;
        self.update_blocking_force();
    }
}

/// `merge_springs`: the springs averaged (their compress strengths, not
/// the inverses), the ideal distance at least 0.3 above the largest
/// minimum distance, which is the merged spring's. Of none, a spring 0.3
/// long whose NaN strengths are refused as insane, as in LilyPond.
pub fn merge_springs(springs: &[Spring], errors: &mut Errors) -> Spring {
    let mut avg_distance = 0.0;
    let mut min_distance = 0.0;
    let mut avg_stretch = 0.0;
    let mut avg_compress = 0.0;
    for s in springs {
        avg_distance += s.ideal_distance;
        avg_stretch += s.inverse_stretch_strength;
        avg_compress += 1.0 / s.inverse_compress_strength;
        min_distance = max(s.min_distance, min_distance);
    }
    let n = springs.len() as f64;
    avg_stretch /= n;
    avg_compress /= n;
    avg_distance /= n;
    avg_distance = max(min_distance + 0.3, avg_distance);

    let mut ret = Spring::new(avg_distance, min_distance, errors);
    ret.set_inverse_stretch_strength(avg_stretch, errors);
    ret.set_inverse_compress_strength(1.0 / avg_compress, errors);
    ret
}

/// A [`Spring`] as a Scheme value, `spring-smob.cc`'s smob, which
/// `ly:spring-set-inverse-…-strength!` and `%spring-set!` change in place.
/// It prints as `#<Spring>` and is `equal?` only to itself
/// (`Spring::equal_p`), the trait's defaults.
#[derive(Clone)]
pub struct SpringObject(pub Spring);

impl Foreign for SpringObject {
    fn type_name(&self) -> &'static str {
        "Spring"
    }
}

/// `(ly:make-spring ideal min-dist)`: `Spring (ideal, min_dist)`.
pub(crate) fn ly_make_spring(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "ly:make-spring";
    let ideal = crate::number_arg(cx, SUBR, 1, args[0])?;
    let min = crate::number_arg(cx, SUBR, 2, args[1])?;
    let mut errors = Errors::new();
    let s = Spring::new(ideal, min, &mut errors);
    let v = cx.heap_mut().wrap(SpringObject(s));
    reported(cx, v, &errors)
}

/// `(%spring-ideal-distance spring)`: its ideal distance, which
/// `ly:paper-column::print` draws; wrong-type-arg for what is no spring,
/// where LilyPond reads through a null.
pub(crate) fn ideal_distance(cx: &mut Cx, args: &[Value]) -> R {
    let Some(d) = cx
        .heap()
        .foreign::<SpringObject>(args[0])
        .map(|s| s.0.ideal_distance)
    else {
        return Err(cx.wrong_type("%spring-ideal-distance", 1, args[0], "Spring"));
    };
    Ok(cx.heap_mut().flonum(d))
}

/// `(ly:spring? x)`.
pub(crate) fn is_spring(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        cx.heap().foreign::<SpringObject>(args[0]).is_some(),
    ))
}

/// `(ly:spring-set-inverse-compress-strength! spring strength)`.
pub(crate) fn ly_set_inverse_compress_strength(cx: &mut Cx, args: &[Value]) -> R {
    set_strength(
        cx,
        "ly:spring-set-inverse-compress-strength!",
        args,
        Spring::set_inverse_compress_strength,
    )
}

/// `(ly:spring-set-inverse-stretch-strength! spring strength)`.
pub(crate) fn ly_set_inverse_stretch_strength(cx: &mut Cx, args: &[Value]) -> R {
    set_strength(
        cx,
        "ly:spring-set-inverse-stretch-strength!",
        args,
        Spring::set_inverse_stretch_strength,
    )
}

/// `spring` changed in place by `set` with `strength`, and a copy of it
/// (`smobbed_copy`), a new spring.
fn set_strength(
    cx: &mut Cx,
    subr: &str,
    args: &[Value],
    set: fn(&mut Spring, f64, &mut Errors),
) -> R {
    let Some(mut s) = cx.heap().foreign::<SpringObject>(args[0]).map(|s| s.0) else {
        return Err(cx.wrong_type(subr, 1, args[0], "Spring"));
    };
    let strength = crate::number_arg(cx, subr, 2, args[1])?;
    let mut errors = Errors::new();
    set(&mut s, strength, &mut errors);
    if let Some(o) = cx.heap_mut().foreign_mut::<SpringObject>(args[0]) {
        o.0 = s;
    }
    let v = cx.heap_mut().wrap(SpringObject(s));
    reported(cx, v, &errors)
}

/// The real `v`, 0 for what is none.
fn real(cx: &mut Cx, v: Value) -> f64 {
    cx.heap().to_f64(v).unwrap_or(0.0)
}

/// `(%make-spring ideal min-dist)`: `Spring (ideal, min_dist)`.
pub(crate) fn make_spring(cx: &mut Cx, args: &[Value]) -> R {
    let mut errors = Errors::new();
    let (ideal, min) = (real(cx, args[0]), real(cx, args[1]));
    let s = Spring::new(ideal, min, &mut errors);
    let v = cx.heap_mut().wrap(SpringObject(s));
    reported(cx, v, &errors)
}

/// `(%spring-set! spring what x)`: `what` one of `ideal-distance`,
/// `min-distance`, `ensure-min-distance`, `inverse-stretch-strength`,
/// `inverse-compress-strength` or `default-strength` (`x` unread), set on
/// `spring` as its setter sets it.
pub(crate) fn spring_set(cx: &mut Cx, args: &[Value]) -> R {
    const SUBR: &str = "%spring-set!";
    let Some(mut s) = cx.heap().foreign::<SpringObject>(args[0]).map(|s| s.0) else {
        return Err(cx.wrong_type(SUBR, 1, args[0], "Spring"));
    };
    let x = real(cx, args[2]);
    let mut errors = Errors::new();
    match cx.heap().name(args[1]) {
        Some("ideal-distance") => s.set_ideal_distance(x, &mut errors),
        Some("min-distance") => s.set_min_distance(x, &mut errors),
        Some("ensure-min-distance") => s.ensure_min_distance(x, &mut errors),
        Some("inverse-stretch-strength") => s.set_inverse_stretch_strength(x, &mut errors),
        Some("inverse-compress-strength") => s.set_inverse_compress_strength(x, &mut errors),
        Some("default-strength") => s.set_default_strength(),
        _ => return Err(cx.wrong_type(SUBR, 2, args[1], "spring setter")),
    }
    if let Some(o) = cx.heap_mut().foreign_mut::<SpringObject>(args[0]) {
        o.0 = s;
    }
    reported(cx, Value::UNSPECIFIED, &errors)
}

/// `value`, after the programming errors `errors`.
pub(crate) fn reported(cx: &mut Cx, value: Value, errors: &[&str]) -> R {
    let errors: Vec<alloc::string::String> = errors.iter().map(|&e| e.into()).collect();
    crate::programming_errored(cx, value, &errors)
}
