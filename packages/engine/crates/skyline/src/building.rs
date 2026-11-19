//! `skyline.cc`'s buildings: [`Building`], and the functions that build a
//! skyline's buildings from buildings in any order
//! (`Skyline::internal_build_skyline`) and merge two skylines' buildings
//! (`Skyline::internal_merge_skyline`).
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_fonts::Box;

use crate::{Axis, Direction, fma, sort};

/// `std::max`: `b` only when `a < b`.
pub(crate) fn max(a: f64, b: f64) -> f64 {
    if a < b { b } else { a }
}

/// `printf`'s `%f`: six decimals, `inf` and `nan` as C writes them.
pub(crate) fn printf_f(x: f64) -> String {
    if x.is_nan() {
        String::from(if x.is_sign_negative() { "-nan" } else { "nan" })
    } else if x.is_infinite() {
        String::from(if x > 0.0 { "inf" } else { "-inf" })
    } else {
        alloc::format!("{x:.6}")
    }
}

/// A building of a skyline: a roof from `left` to `right`, as a line of
/// `slope` through `y_intercept` at 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Building {
    pub(crate) left: f64,
    pub(crate) right: f64,
    pub(crate) y_intercept: f64,
    pub(crate) slope: f64,
}

impl Building {
    pub(crate) fn new(start: f64, start_height: f64, end_height: f64, end: f64) -> Building {
        let mut building = Building {
            left: start,
            right: end,
            y_intercept: 0.0,
            slope: 0.0,
        };
        building.precompute(start_height, end_height);
        building
    }

    /// `Building (Box, Axis, Direction)`: the box's extent along the
    /// horizon, at the height of its `sky` side.
    pub(crate) fn from_box(b: Box, horizon: Axis, sky: Direction) -> Building {
        let (x, y) = match horizon {
            Axis::X => (b.x, b.y),
            Axis::Y => (b.y, b.x),
        };
        let height = sky.sign()
            * match sky {
                Direction::Down => y.left,
                Direction::Up => y.right,
            };
        let mut building = Building {
            left: x.left,
            right: x.right,
            y_intercept: 0.0,
            slope: 0.0,
        };
        building.precompute(height, height);
        building
    }

    /// `Interval::length`.
    fn length(&self) -> f64 {
        if self.left > self.right {
            0.0
        } else {
            self.right - self.left
        }
    }

    fn precompute(&mut self, start_height: f64, end_height: f64) {
        self.slope = 0.0;
        if start_height != end_height {
            self.slope = (end_height - start_height) / self.length();
        }
        if self.left.is_infinite() {
            self.y_intercept = start_height;
        } else if self.slope.abs() > 1e6 {
            // Too steep for slope-intercept form.
            self.slope = 0.0;
            self.y_intercept = max(start_height, end_height);
        } else {
            self.y_intercept = fma(-self.slope, self.left, start_height);
        }
    }

    pub(crate) fn height(&self, x: f64) -> f64 {
        if x.is_infinite() {
            self.y_intercept
        } else {
            fma(self.slope, x, self.y_intercept)
        }
    }

    /// `Building::print`: `printf`'s line of the building, as LilyPond's
    /// debugging prints it.
    pub(crate) fn print(&self) -> String {
        alloc::format!(
            "{} x + {} from {} to {}\n",
            printf_f(self.slope),
            printf_f(self.y_intercept),
            printf_f(self.left),
            printf_f(self.right)
        )
    }

    fn intersection_x(&self, other: &Building) -> f64 {
        let slope_delta = other.slope - self.slope;
        if slope_delta.abs() < 1e-4 {
            return max(self.left, other.left);
        }
        (self.y_intercept - other.y_intercept) / slope_delta
    }

    fn above(&self, other: &Building, x: f64) -> bool {
        if self.y_intercept.is_infinite() || other.y_intercept.is_infinite() || x.is_infinite() {
            self.y_intercept > other.y_intercept
        } else {
            fma(self.slope - other.slope, x, self.y_intercept) > other.y_intercept
        }
    }

    /// `LessThanBuilding`: by start, then the higher start first.
    fn less(&self, other: &Building) -> bool {
        self.left < other.left
            || (self.left == other.left && self.height(self.left) > other.height(self.left))
    }
}

/// `Skyline::internal_merge_skyline`: walks the two skylines at once, the
/// building that reaches further continuing as `b`. `None` for an empty one,
/// where LilyPond reports a programming error and leaves the result as it
/// was.
pub(crate) fn merge_buildings(one: &[Building], two: &[Building]) -> Option<Vec<Building>> {
    if one.is_empty() || two.is_empty() {
        return None;
    }
    let mut result = Vec::with_capacity(one.len().max(two.len()));
    let (mut sbp, mut scp) = (one, two);
    let (mut bit, mut cit) = (0, 0);
    let mut b = sbp[bit];
    while cit < scp.len() {
        let mut c = scp[cit];
        if b.right < c.right {
            // Finish with b.
            if b.right <= b.left {
                // Already finished with b.
            } else if c.above(&b, c.left) {
                let mut m = b;
                m.right = c.left;
                if m.right > m.left {
                    result.push(m);
                }
                if b.above(&c, b.right) {
                    let mut n = c;
                    let x = b.intersection_x(&c);
                    n.right = x;
                    b.left = x;
                    result.push(n);
                    result.push(b);
                    c.left = b.right;
                }
            } else {
                if c.above(&b, b.right) {
                    let x = b.intersection_x(&c);
                    b.right = x;
                    c.left = x;
                } else {
                    c.left = b.right;
                }
                result.push(b);
            }
            // c continues further, so it becomes b.
            b = c;
            core::mem::swap(&mut bit, &mut cit);
            core::mem::swap(&mut sbp, &mut scp);
        } else {
            // Finish with c.
            if c.above(&b, c.left) {
                let mut m = b;
                m.right = c.left;
                if m.right > m.left {
                    result.push(m);
                }
                if b.above(&c, c.right) {
                    c.right = b.intersection_x(&c);
                }
            } else if c.above(&b, c.right) {
                let mut m = b;
                let x = b.intersection_x(&c);
                c.left = x;
                m.right = x;
                result.push(m);
            } else {
                // c is hidden by b.
                cit += 1;
                continue;
            }
            result.push(c);
            b.left = c.right;
        }
        cit += 1;
    }
    if b.right > b.left {
        result.push(b);
    }
    Some(result)
}

pub(crate) fn empty_skyline() -> Vec<Building> {
    Vec::from([Building::new(
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
    )])
}

pub(crate) fn single_skyline(b: Building) -> Vec<Building> {
    let mut result = Vec::with_capacity(3);
    if b.left != f64::NEG_INFINITY {
        result.push(Building::new(
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
            b.left,
        ));
    }
    result.push(b);
    if b.right != f64::INFINITY {
        result.push(Building::new(
            b.right,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
        ));
    }
    result
}

/// `non_overlapping_skyline`: the sorted buildings split into a skyline of
/// those that do not overlap, gaps filled, and the rest.
fn non_overlapping_skyline(buildings: &[Building]) -> (Vec<Building>, Vec<Building>) {
    let mut trimmed = Vec::with_capacity(buildings.len() / 2);
    let mut result = Vec::with_capacity(buildings.len() / 2);
    let mut last_end = f64::NEG_INFINITY;
    let mut last_building = Building::new(
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
    );
    for &b in buildings {
        let (x1, x2) = (b.left, b.right);
        let (y1, y2) = (b.height(x1), b.height(x2));
        // Drop buildings that obviously have no effect.
        if last_building.height(x1) >= y1
            && last_building.right >= x2
            && last_building.height(x2) >= y2
        {
            continue;
        }
        if x1 < last_end {
            trimmed.push(b);
            continue;
        }
        if x1 > last_end {
            result.push(Building::new(
                last_end,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
                x1,
            ));
        }
        result.push(b);
        last_building = b;
        last_end = b.right;
    }
    if last_end < f64::INFINITY {
        result.push(Building::new(
            last_end,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
        ));
    }
    (trimmed, result)
}

/// `Skyline::internal_build_skyline`: overlapping buildings in any order to
/// an ordered skyline, merging the partial skylines pairwise from a queue.
pub(crate) fn build_skyline(mut buildings: Vec<Building>) -> Vec<Building> {
    match buildings.len() {
        0 => return empty_skyline(),
        1 => return single_skyline(buildings[0]),
        _ => {}
    }
    sort::sort(&mut buildings, &Building::less);
    let mut partials = VecDeque::new();
    while !buildings.is_empty() {
        let (trimmed, partial) = non_overlapping_skyline(&buildings);
        partials.push_back(partial);
        buildings = trimmed;
    }
    loop {
        let one = partials.pop_front().expect("one partial skyline at least");
        let Some(two) = partials.pop_front() else {
            return one;
        };
        partials.push_back(merge_buildings(&one, &two).expect("partial skylines are not empty"));
    }
}

#[cfg(test)]
mod tests {
    use lily_engine_fonts::Interval;

    use super::*;

    const INF: f64 = f64::INFINITY;

    fn rect(x: [f64; 2], y: [f64; 2]) -> Box {
        Box {
            x: Interval::new(x[0], x[1]),
            y: Interval::new(y[0], y[1]),
        }
    }

    /// A box's building spans its horizon extent at its `sky` side, stored
    /// negated for `Down`.
    #[test]
    fn a_box_is_a_flat_building_at_its_side() {
        let b = rect([0.0, 2.0], [-1.0, 3.0]);
        let flat = |left, right, height| Building {
            left,
            right,
            y_intercept: height,
            slope: 0.0,
        };
        assert_eq!(
            Building::from_box(b, Axis::X, Direction::Up),
            flat(0.0, 2.0, 3.0)
        );
        assert_eq!(
            Building::from_box(b, Axis::X, Direction::Down),
            flat(0.0, 2.0, 1.0)
        );
        assert_eq!(
            Building::from_box(b, Axis::Y, Direction::Up),
            flat(-1.0, 3.0, 2.0)
        );
        assert_eq!(
            Building::from_box(b, Axis::Y, Direction::Down),
            flat(-1.0, 3.0, 0.0)
        );
        let endless = Building::from_box(rect([-INF, INF], [0.0, 1.0]), Axis::X, Direction::Up);
        assert_eq!(endless, flat(-INF, INF, 1.0));
    }

    /// The same as a flat segment along the box's side.
    #[test]
    fn a_box_s_building_is_its_side_s() {
        let b = rect([0.5, 2.25], [-1.5, 3.75]);
        assert_eq!(
            Building::from_box(b, Axis::X, Direction::Up),
            Building::new(0.5, 3.75, 3.75, 2.25)
        );
        assert_eq!(
            Building::from_box(b, Axis::Y, Direction::Down),
            Building::new(-1.5, -0.5, -0.5, 3.75)
        );
    }

    #[test]
    fn merging_an_empty_skyline_gives_nothing() {
        assert_eq!(merge_buildings(&[], &empty_skyline()), None);
        assert_eq!(merge_buildings(&empty_skyline(), &[]), None);
        assert_eq!(
            merge_buildings(&empty_skyline(), &empty_skyline()),
            Some(empty_skyline())
        );
    }

    /// Two buildings in any order build the same ordered skyline.
    #[test]
    fn buildings_build_in_any_order() {
        let a = Building::new(0.0, 1.0, 1.0, 4.0);
        let b = Building::new(1.0, 2.0, 2.0, 2.0);
        let built = build_skyline(Vec::from([b, a]));
        assert_eq!(built, build_skyline(Vec::from([a, b])));
        let roofs: Vec<[f64; 3]> = built
            .iter()
            .map(|b| [b.left, b.right, b.y_intercept])
            .collect();
        assert_eq!(
            roofs,
            [
                [-INF, 0.0, -INF],
                [0.0, 1.0, 1.0],
                [1.0, 2.0, 2.0],
                [2.0, 4.0, 1.0],
                [4.0, INF, -INF]
            ]
        );
    }
}
