//! `spacing-options.cc` (part 3's, MS3 step 31): the options the spacing
//! spanner reads from a `SpacingSpanner` (or a `GraceSpacing`) grob,
//! `init_from_grob` (`spacing-options.cc:30–52`, 23 lines), its
//! constructor (`:54–65`, 12), and the space of a duration,
//! `get_duration_space` (`:70–107`, 38). Rust with no Scheme face: its
//! callers are the spacing spanner's `set_springs`, `note_spacing` and
//! `musical_column_spacing` (`spacing_spanner.rs`, `spacing_basic.rs`).
//!
//! `std::log2` is `libm`'s, which gives macOS's (LilyPond 2.26.0's) on
//! every ratio of two durations with dots, double dots and tuplets of 3,
//! 5, 7, 9 and 15 to 2¹⁰ (1,044 ratios); of the ratios p/q ≥ 1 with
//! q ≤ p ≤ 1024 it misses macOS's by one unit in the last place on 7,106
//! of 524,800 (13/10, 25/8, …), where macOS's is no better rounded.

use lily_engine_music::{Host, Moment, Rational};
use lily_engine_scheme::{Exception, Value};

use crate::grob;

/// `Spacing_options`: what the spacing reads from its grob.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpacingOptions {
    /// `packed_`: `packed-spacing`.
    pub packed: bool,
    /// `stretch_uniformly_`: `uniform-stretching`.
    pub stretch_uniformly: bool,
    /// `float_nonmusical_columns_`: `strict-note-spacing`.
    pub float_nonmusical_columns: bool,
    /// `float_grace_columns_`: `strict-grace-spacing`.
    pub float_grace_columns: bool,
    /// `shortest_duration_space_`: `shortest-duration-space`.
    pub shortest_duration_space: f64,
    /// `increment_`: `spacing-increment`.
    pub increment: f64,
    /// `global_shortest_`: the main part of `common-shortest-duration`, or
    /// its grace part if the main part is 0.
    pub global_shortest: Rational,
}

impl Default for SpacingOptions {
    /// `Spacing_options ()`: not packed, nothing uniform or strict, a
    /// shortest duration space of 2, an increment of 1.2 and a shortest
    /// duration of 1/8.
    fn default() -> SpacingOptions {
        SpacingOptions {
            packed: false,
            stretch_uniformly: false,
            float_nonmusical_columns: false,
            float_grace_columns: false,
            shortest_duration_space: 2.0,
            increment: 1.2,
            global_shortest: Rational::new(1, 8),
        }
    }
}

/// `from_scm<double> (v, fallback)`: `v` if it is a real number, else
/// `fallback`.
pub(crate) fn double_or(host: &mut dyn Host, v: Value, fallback: f64) -> f64 {
    let heap = host.heap();
    match heap.to_f64(v) {
        Some(x) if heap.is_real(v) => x,
        _ => fallback,
    }
}

impl SpacingOptions {
    /// `init_from_grob`: the options read from `me`'s properties, a number
    /// that is no number as 1 (not the constructor's) and a flag as true
    /// only if `#t` (`from_scm<bool>`); a `common-shortest-duration` that
    /// is no moment as 1/8 (main part) and 1/16 (grace part).
    pub fn init_from_grob(&mut self, host: &mut dyn Host, me: Value) -> Result<(), Exception> {
        let v = grob::property(host, me, "spacing-increment")?;
        self.increment = double_or(host, v, 1.0);

        let flag = |host: &mut dyn Host, name| -> Result<bool, Exception> {
            Ok(grob::property(host, me, name)? == Value::TRUE)
        };
        self.packed = flag(host, "packed-spacing")?;
        self.stretch_uniformly = flag(host, "uniform-stretching")?;
        self.float_nonmusical_columns = flag(host, "strict-note-spacing")?;
        self.float_grace_columns = flag(host, "strict-grace-spacing")?;
        let v = grob::property(host, me, "shortest-duration-space")?;
        self.shortest_duration_space = double_or(host, v, 1.0);

        let v = grob::property(host, me, "common-shortest-duration")?;
        let shortest_dur = host
            .heap()
            .foreign::<Moment>(v)
            .copied()
            .unwrap_or(Moment::new(Rational::new(1, 8), Rational::new(1, 16)));
        self.global_shortest = if shortest_dur.main.is_zero() {
            shortest_dur.grace
        } else {
            shortest_dur.main
        };
        Ok(())
    }

    /// `get_duration_space`: the space of a note of the duration `d`, at
    /// the ratio `d / global_shortest`: below 1, linear, the
    /// `shortest-duration-space` less 1 plus the ratio; from 1, the
    /// `shortest-duration-space` plus the ratio's log₂ (Gourlay); either
    /// times the increment. A ratio that does not fit, where LilyPond's
    /// would wrap, is `numerical-overflow` (D61).
    pub fn get_duration_space(&self, host: &mut dyn Host, d: Rational) -> Result<f64, Exception> {
        let Some(ratio) = d.checked_div(self.global_shortest) else {
            return Err(crate::overflow(host.heap(), "get_duration_space"));
        };
        let ratio = ratio.to_f64();
        Ok(if ratio < 1.0 {
            (self.shortest_duration_space + ratio - 1.0) * self.increment
        } else {
            (self.shortest_duration_space + libm::log2(ratio)) * self.increment
        })
    }
}
