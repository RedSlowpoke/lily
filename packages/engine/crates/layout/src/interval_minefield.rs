//! `interval-minefield.cc` (part 4's, MS3 step 48; the whole file, 86
//! lines): [`IntervalMinefield`], the feasible placements of something of
//! a given bulk pushed out of the intervals forbidden to it.
//! `beam-quanting.cc` is its one user (MS4), so nothing calls it yet.

use alloc::vec::Vec;

use crate::geometry::{Direction, Interval};

/// `infinity_f`.
const INFINITY: f64 = f64::INFINITY;

/// `Interval::contains`.
fn contains(iv: Interval, x: f64) -> bool {
    iv.left <= x && x <= iv.right
}

/// `Interval_minefield`: an interval of feasible placements, the bulk of
/// what is placed, and the intervals it may not overlap.
#[derive(Clone, Debug)]
pub struct IntervalMinefield {
    feasible_placements: Interval,
    forbidden_intervals: Vec<Interval>,
    bulk: f64,
}

impl IntervalMinefield {
    pub fn new(feasible_placements: Interval, bulk: f64) -> IntervalMinefield {
        IntervalMinefield {
            feasible_placements,
            forbidden_intervals: Vec::new(),
            bulk,
        }
    }

    pub fn add_forbidden_interval(&mut self, forbidden: Interval) {
        self.forbidden_intervals.push(forbidden);
    }

    pub fn feasible_placements(&self) -> Interval {
        self.feasible_placements
    }

    /// `solve`: each end of the feasible placements pushed epsilon past each
    /// forbidden interval it collides with, the thing's bulk around it, or
    /// to the infinity on its side when the interval reaches that infinity;
    /// again over all the intervals until no end moves.
    ///
    /// TODO: figure out a faster ways that this loop can happen via a
    /// better search algorithm.
    pub fn solve(&mut self) {
        let epsilon = 1.0e-10;
        loop {
            let mut dirty = false;
            for forbidden in &self.forbidden_intervals {
                for d in [-1, 1] as [Direction; 2] {
                    let at = self.feasible_placements.at(d);
                    let mut feasible_widened = Interval::new(at, at);
                    feasible_widened.widen(self.bulk / 2.0);

                    if forbidden.at(d) == f64::from(d) * INFINITY {
                        *self.feasible_placements.at_mut(d) = f64::from(d) * INFINITY;
                    } else if contains(*forbidden, feasible_widened.at(d))
                        || contains(*forbidden, feasible_widened.at(-d))
                        || contains(feasible_widened, forbidden.at(d))
                        || contains(feasible_widened, forbidden.at(-d))
                    {
                        *self.feasible_placements.at_mut(d) =
                            forbidden.at(d) + f64::from(d) * (epsilon + (self.bulk / 2.0));
                        dirty = true;
                    }
                }
            }
            if !dirty {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solved(feasible: (f64, f64), bulk: f64, forbidden: &[(f64, f64)]) -> (f64, f64) {
        let mut minefield = IntervalMinefield::new(Interval::new(feasible.0, feasible.1), bulk);
        for &(l, r) in forbidden {
            minefield.add_forbidden_interval(Interval::new(l, r));
        }
        minefield.solve();
        let iv = minefield.feasible_placements();
        (iv.left, iv.right)
    }

    #[test]
    fn no_mine_leaves_the_placements() {
        assert_eq!(solved((-2.0, 3.0), 0.5, &[]), (-2.0, 3.0));
        assert_eq!(solved((-2.0, 3.0), 0.5, &[(10.0, 11.0)]), (-2.0, 3.0));
    }

    #[test]
    fn each_end_is_pushed_past_a_mine_by_half_the_bulk() {
        // The left end collides with [-3, -1]: pushed down past -3; the
        // right end with [2, 4]: pushed up past 4.
        let (l, r) = solved((-2.0, 3.0), 1.0, &[(-3.0, -1.0), (2.0, 4.0)]);
        assert_eq!(l, -3.0 - (1.0e-10 + 0.5));
        assert_eq!(r, 4.0 + (1.0e-10 + 0.5));
    }

    #[test]
    fn a_mine_to_an_infinity_sends_that_end_there() {
        let (l, r) = solved((0.0, 1.0), 0.0, &[(f64::NEG_INFINITY, 0.5)]);
        assert_eq!(l, f64::NEG_INFINITY);
        // The right end, 1, is not in the mine: it stays.
        assert_eq!(r, 1.0);
        let (_, r) = solved((0.0, 1.0), 0.0, &[(0.5, f64::INFINITY)]);
        assert_eq!(r, f64::INFINITY);
    }

    #[test]
    fn pushes_repeat_until_no_end_moves() {
        // The end pushed past one mine lands in the next, which is before it
        // in the list, so a second pass moves it again.
        let (_, r) = solved((0.0, 1.0), 0.0, &[(2.0, 3.0), (0.5, 2.0)]);
        assert_eq!(r, 3.0 + 1.0e-10);
    }
}
