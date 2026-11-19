//! `skyline.cc`'s distance between two skylines facing each other
//! (`Skyline::distance` and both `Skyline::internal_distance`), where they
//! touch (`Skyline::touching_point`), and the
//! skyline padded along its horizon that the distance with padding measures
//! from (`Skyline::padded`).
use alloc::vec::Vec;

use crate::Skyline;
use crate::building::{Building, build_skyline, max, merge_buildings};

/// `std::min`: `b` only when `b < a`.
pub(crate) fn min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

impl Skyline {
    /// `Skyline::distance`: how far `other`, bounding the opposite side,
    /// must move away from this skyline for the two to touch, with this one
    /// padded by `horizon_padding` along the horizon. -infinity when either
    /// has no buildings or neither has a roof over any point.
    pub fn distance(&self, other: &Skyline, horizon_padding: f64) -> f64 {
        self.internal_distance_padded(other, horizon_padding).0
    }

    /// `Skyline::touching_point`: where the two skylines, this one padded
    /// as for [`distance`](Skyline::distance), reach their distance, the
    /// last such end of a stretch; -infinity where `distance` is.
    pub fn touching_point(&self, other: &Skyline, horizon_padding: f64) -> f64 {
        self.internal_distance_padded(other, horizon_padding).1
    }

    /// `Skyline::internal_distance (Skyline const &, Real, Real *)`: the
    /// distance and where the skylines touch. Padding `other` too would give
    /// the same as padding this one twice as much.
    fn internal_distance_padded(&self, other: &Skyline, horizon_padding: f64) -> (f64, f64) {
        if horizon_padding == 0.0 {
            return self.internal_distance(other);
        }
        self.padded(horizon_padding).internal_distance(other)
    }

    /// `Skyline::padded`: each roof continued flat for `horizon_padding`
    /// beyond its ends, then falling by as much over as long again. A
    /// padding not above 0 leaves the skyline as it is; LilyPond warns
    /// "Cannot have negative horizon padding.  Junking." for one below 0,
    /// which is the caller's to report. A skyline without buildings pads to
    /// the empty one.
    pub fn padded(&self, horizon_padding: f64) -> Skyline {
        if horizon_padding <= 0.0 {
            return self.clone();
        }
        let mut pad_buildings = Vec::with_capacity(4 * self.buildings.len());
        for b in &self.buildings {
            if b.left > f64::NEG_INFINITY {
                let height = b.height(b.left);
                if height > f64::NEG_INFINITY {
                    // The sloped building, then the flat one, left of b.
                    let start = b.left - 2.0 * horizon_padding;
                    let end = b.left - horizon_padding;
                    pad_buildings.push(Building::new(start, height - horizon_padding, height, end));
                    pad_buildings.push(Building::new(end, height, height, b.left));
                }
            }
            if b.right < f64::INFINITY {
                let height = b.height(b.right);
                if height > f64::NEG_INFINITY {
                    // The flat building, then the sloped one, right of b.
                    let start = b.right;
                    let end = start + horizon_padding;
                    pad_buildings.push(Building::new(start, height, height, end));
                    let (start, end) = (end, end + horizon_padding);
                    pad_buildings.push(Building::new(start, height, height - horizon_padding, end));
                }
            }
        }
        let pad_skyline = build_skyline(pad_buildings);
        let mut padded = Skyline::empty(self.sky);
        if let Some(merged) = merge_buildings(&pad_skyline, &self.buildings) {
            padded.buildings = merged;
        }
        padded
    }

    /// `Skyline::internal_distance (Skyline const &, Real *)`: the largest
    /// sum of the two skylines' heights at the ends of each stretch where
    /// neither changes building, and the last end where that sum was reached.
    fn internal_distance(&self, other: &Skyline) -> (f64, f64) {
        assert_ne!(self.sky, other.sky, "distance between skylines of one side");
        let (mut i, mut j) = (self.buildings.iter(), other.buildings.iter());
        let (mut bi, mut bj) = (i.next(), j.next());
        let mut dist = f64::NEG_INFINITY;
        let mut start = f64::NEG_INFINITY;
        let mut touch = f64::NEG_INFINITY;
        while let (Some(a), Some(b)) = (bi, bj) {
            let end = min(a.right, b.right);
            let start_dist = a.height(start) + b.height(start);
            let end_dist = a.height(end) + b.height(end);
            dist = max(dist, max(start_dist, end_dist));
            if end_dist == dist {
                touch = end;
            } else if start_dist == dist {
                touch = start;
            }
            if a.right <= b.right {
                bi = i.next();
            } else {
                bj = j.next();
            }
            start = end;
        }
        (dist, touch)
    }
}
