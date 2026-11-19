//! `slur-configuration.cc` (MS4 step 8): `Slur_configuration`, a candidate
//! position of a slur, its two ends, its curve (`generate_curve`,
//! `:160–240`, with `fit_factor`, `:115–158`, over the points to avoid and
//! `avoid_staff_line`, `:41–113`) and its demerits by the scorers in
//! increasing cost (`run_next_scorer`, `:539–561`): the slope
//! (`score_slopes`, `:497–537`), the edges (`score_edges`, `:468–495`), the
//! extra objects (`score_extra_encompass`, `:346–466`, the ties' ends
//! among them, with misc.cc's `peak_around`, borrowed) and the encompassed
//! heads and stems (`score_encompass`, `:262–344`); its score card,
//! `"name=%.2f"` joined by commas, the slur's annotation when debugging.
//!
//! LilyPond's clang fuses here (otool): `dot_product`'s sum, the staff
//! line's minimum distance and the curve's remaining shift,
//! `linear_interpolate`'s sum and the slope's demerits ([`fma`]).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::Host;
use lily_engine_scheme::{Exception, Value};

use crate::beam_quanting::round_halfway_up;
use crate::bezier;
use crate::geometry::{
    Direction, Interval, Offset, X, Y, angle_degrees, complex_multiply, fma, length, max, min,
};
use crate::grob;
use crate::item;
use crate::lookup::Bezier;
use crate::slur_scoring::{State, contains, ix, linear_interpolate, on_staff_line};
use crate::stem;

type Run<T> = Result<T, Exception>;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const DOWN: Direction = -1;
const UP: Direction = 1;

/// The scorers, ordered by increasing computational cost.
const INITIAL_SCORE: usize = 0;
const SLOPE: usize = 1;
const EDGES: usize = 2;
const EXTRA_ENCOMPASS: usize = 3;
const ENCOMPASS: usize = 4;
const NUM_SCORERS: usize = 5;

/// `dot_product`: its sum fused, as clang compiles offset.hh's.
fn dot_product(o1: Offset, o2: Offset) -> f64 {
    fma(o1.0, o2.0, o1.1 * o2.1)
}

/// `peak_around` (misc.cc): 1 below 0, falling from `epsilon`'s peak to 0
/// at `threshold`, 0 past it.
pub(crate) fn peak_around(epsilon: f64, threshold: f64, x: f64) -> f64 {
    if x < 0.0 {
        return 1.0;
    }
    max(
        -epsilon * (x - threshold) / ((x + epsilon) * threshold),
        0.0,
    )
}

/// `programming_error`s that bezier.rs gave, written.
fn report(host: &mut dyn Host, errors: &[String]) -> Run<()> {
    for text in errors {
        grob::unlocated_programming_error(host, text)?;
    }
    Ok(())
}

/// `Bezier::get_other_coordinate (X_AXIS, x)`, its programming error
/// written.
fn y_at(host: &mut dyn Host, curve: &Bezier, x: f64) -> Run<f64> {
    let mut errors = Vec::new();
    let y = bezier::get_other_coordinate(curve, X, x, &mut errors);
    report(host, &errors)?;
    Ok(y)
}

/// `avoid_staff_line`: a curve whose horizontal point is too near a line
/// of its staff moved off it. The staff is asked through
/// `Staff_symbol_referencer::on_staff_line`, which reads a staff symbol's
/// own `staff-symbol`, so as in LilyPond's this leaves the curve as it is.
fn avoid_staff_line(host: &mut dyn Host, state: &State, mut bez: Bezier) -> Run<Bezier> {
    let ts = bezier::solve_derivative(&bez, (1.0, 0.0));

    /* TODO: handle case of broken slur.  */
    let [l, r] = [state.extremes[0].staff, state.extremes[1].staff];
    if let (Some(&t), Some(staff), true) = (ts.first(), l, l == r && r.is_some()) {
        //the first (usually only) point where slur is horizontal
        let y = bezier::curve_point(&bez, t).1;
        // A Bezier curve at t moves 3t-3t² as far as the middle control points
        let factor = 3.0 * t * (1.0 - t);

        let at = grob::relative_coordinate(host, staff, state.common[Y], Y)?;
        let p = 2.0 * (y - at) / state.staff_space;

        let mut round_p = round_halfway_up(p) as i32;
        if !on_staff_line(host, staff, round_p)? {
            round_p += if p > f64::from(round_p) { 1 } else { -1 };
        }
        if !on_staff_line(host, staff, round_p)? {
            return Ok(bez);
        }

        let distance = (p - f64::from(round_p)) * state.staff_space / 2.0;
        // Allow half the thickness of the slur at the point t, plus one basic
        // blot-diameter (half for the slur outline, half for the staff line)
        let gap = if f64::from(state.dir) * distance > 0.0 {
            state.parameters.gap_to_staffline_inside
        } else {
            state.parameters.gap_to_staffline_outside
        };
        let min_distance = fma(0.5 * state.thickness, factor, state.line_thickness) + gap;
        if libm::fabs(distance) < min_distance {
            let resolution_dir = if distance > 0.0 { UP } else { DOWN };

            let dy = f64::from(resolution_dir) * (min_distance - libm::fabs(distance));

            // Shape the curve, moving the horizontal point by factor * dy
            bez[1].1 += dy;
            bez[2].1 += dy;
            // Move the entire curve by the remaining amount
            bezier::translate(&mut bez, (0.0, fma(-factor, dy, dy)));
        }
    }
    Ok(bez)
}

/// `fit_factor`: how much higher the curve must be to pass outside each
/// point of `avoid` not near its ends, measured with its chord level.
fn fit_factor(
    host: &mut dyn Host,
    dz_unit: Offset,
    dz_perp: Offset,
    close_to_edge_length: f64,
    mut curve: Bezier,
    d: Direction,
    avoid: &[Offset],
) -> Run<f64> {
    let mut fit_factor = 0.0;
    let x0 = curve[0];
    bezier::translate(&mut curve, (-x0.0, -x0.1));
    bezier::rotate(&mut curve, -angle_degrees(dz_unit));
    bezier::scale(&mut curve, 1.0, f64::from(d));

    let mut curve_xext = Interval::EMPTY;
    curve_xext.add_point(curve[0].0);
    curve_xext.add_point(curve[3].0);

    for &a in avoid {
        let z = (a.0 - x0.0, a.1 - x0.1);
        let p = (
            dot_product(z, dz_unit),
            f64::from(d) * dot_product(z, dz_perp),
        );

        let close_to_edge = [LEFT, RIGHT]
            .into_iter()
            .any(|e| f64::from(-e) * (p.0 - curve_xext.at(e)) < close_to_edge_length);

        if close_to_edge {
            continue;
        }

        let eps = 0.01;
        let mut pext = Interval::new(-eps + p.0, eps + p.0);
        pext.intersect(curve_xext);

        if pext.is_empty() || pext.length() <= 1.999 * eps {
            continue;
        }

        let y = y_at(host, &curve, p.0)?;
        if y != 0.0 {
            fit_factor = max(fit_factor, p.1 / y);
        }
    }
    Ok(fit_factor)
}

/// `Slur_configuration`.
pub(crate) struct Configuration {
    score: f64,
    score_card: String,
    pub attachment: [Offset; 2],
    pub curve: Bezier,
    pub height: f64,
    pub index: usize,
    next_scorer_todo: usize,
}

impl Configuration {
    /// `new_config`: the ends `offs`, the `idx`th, unscored.
    pub fn new_config(offs: [Offset; 2], idx: usize) -> Configuration {
        Configuration {
            score: 0.0,
            score_card: String::new(),
            attachment: offs,
            curve: [(0.0, 0.0); 4],
            height: 0.0,
            index: idx,
            next_scorer_todo: INITIAL_SCORE + 1,
        }
    }

    pub fn score(&self) -> f64 {
        self.score
    }

    pub fn card(&self) -> String {
        self.score_card.clone()
    }

    pub fn done(&self) -> bool {
        self.next_scorer_todo >= NUM_SCORERS
    }

    /// `add_score`: a demerit, LilyPond's programming error and 0 if it is
    /// negative; in the card unless it is 0.
    fn add_score(&mut self, host: &mut dyn Host, mut s: f64, desc: &str) -> Run<()> {
        if s < 0.0 {
            grob::unlocated_programming_error(host, "Negative demerits found for slur.  Ignoring")?;
            s = 0.0;
        }

        if s != 0.0 {
            if !self.score_card.is_empty() {
                self.score_card.push_str(", ");
            }
            self.score_card.push_str(&format!("{desc}={s:.2}"));
            self.score += s;
        }
        Ok(())
    }

    /// `generate_curve`: the curve between the ends, its indent and height
    /// by `get_slur_indent_height` from `r_0` and `h_inf`, made higher to
    /// clear `avoid` up to the most it can be without bulging at its ends,
    /// off the staff lines.
    pub fn generate_curve(
        &mut self,
        host: &mut dyn Host,
        state: &State,
        r_0: f64,
        h_inf: f64,
        avoid: &[Offset],
    ) -> Run<()> {
        let [l, r] = self.attachment;
        let dz = (r.0 - l.0, r.1 - l.1);
        let inv = 1.0 / length(dz);
        let dz_unit = (dz.0 * inv, dz.1 * inv);
        let dz_perp = complex_multiply(dz_unit, (0.0, 1.0));

        let (mut indent, mut height) = bezier::get_slur_indent_height(length(dz), h_inf, r_0);

        let len = length(dz);

        /* This condition,

        len^2 > 4h^2 +  3 (i + 1/3len)^2  - 1/3 len^2

        is equivalent to:

        |bez' (0)| < | bez' (.5)|

        when (control2 - control1) has the same direction as
        (control3 - control0).  */

        let max_indent = len / 3.1;
        indent = min(indent, max_indent);

        let a1 = len * len / 3.0;
        let a2 = 0.75 * ((indent + len / 3.0) * (indent + len / 3.0));
        let mut max_h = a1 - a2;

        if max_h < 0.0 {
            grob::unlocated_programming_error(host, "slur indent too small")?;
            max_h = len / 3.0;
        } else {
            max_h = libm::sqrt(max_h);
        }

        let eccentricity = {
            let v = grob::property(host, state.slur, "eccentricity")?;
            crate::spacing_options::double_or(host, v, 0.0)
        };

        let x1 = eccentricity + indent;
        let x2 = eccentricity - indent;

        let df = f64::from(state.dir);
        let make = |height: f64| -> Bezier {
            let up = (dz_perp.0 * height * df, dz_perp.1 * height * df);
            [
                l,
                (l.0 + up.0 + dz_unit.0 * x1, l.1 + up.1 + dz_unit.1 * x1),
                (r.0 + up.0 + dz_unit.0 * x2, r.1 + up.1 + dz_unit.1 * x2),
                r,
            ]
        };
        let curve = make(height);

        let ff = fit_factor(
            host,
            dz_unit,
            dz_perp,
            state.parameters.close_to_edge_length,
            curve,
            state.dir,
            avoid,
        )?;

        height = max(height, min(height * ff, max_h));

        let curve = make(height);

        self.curve = avoid_staff_line(host, state, curve)?;
        self.height = height;
        Ok(())
    }

    /// `score_encompass`: the heads between the ends that the curve does
    /// not clear or nearly touches, the stems it crosses (less at the
    /// near edge), then how unevenly far from the heads it runs.
    fn score_encompass(&mut self, host: &mut dyn Host, state: &State) -> Run<()> {
        let bez = self.curve;
        let p = &state.parameters;
        let dir = state.dir;
        let df = f64::from(dir);
        let [al, ar] = self.attachment;
        let mut demerit = 0.0;

        /*
          Distances for heads that are between slur and line between
          attachment points.
        */
        let mut convex_head_distances = Vec::new();
        let n = state.encompass_infos.len();
        for (j, info) in state.encompass_infos.iter().enumerate() {
            let x = info.x;

            let l_edge = j == 0;
            let r_edge = j == n - 1;
            let edge = l_edge || r_edge;

            if !(x < ar.0 && x > al.0) {
                continue;
            }

            let y = y_at(host, &bez, x)?;
            if !edge {
                let head_dy = y - info.head;
                if df * head_dy < 0.0 {
                    demerit += p.head_encompass_penalty;
                    convex_head_distances.push(0.0);
                } else {
                    let mut hd = if head_dy != 0.0 {
                        1.0 / libm::fabs(head_dy) - 1.0 / p.free_head_distance
                    } else {
                        p.head_encompass_penalty
                    };
                    hd = min(max(hd, 0.0), p.head_encompass_penalty);

                    demerit += hd;
                }

                let line_y = linear_interpolate(x, ar.0, al.0, ar.1, al.1);

                // state.dir_ * state.encompass_infos_[j].get_point (state.dir_) > state.dir_ *line_y
                let closest = df * max(df * info.get_point(dir), df * line_y);
                let d = libm::fabs(closest - y);

                convex_head_distances.push(d);
            }

            if df * (y - info.stem) < 0.0 {
                let mut stem_dem = p.stem_encompass_penalty;
                if (l_edge && dir == UP) || (r_edge && dir == DOWN) {
                    stem_dem /= 5.0;
                }

                demerit += stem_dem;
            }
        }
        self.add_score(host, demerit, "encompass")?;

        let mut n = convex_head_distances.len();
        if n > 0 {
            let mut avg_distance = 0.0;
            let mut min_dist = f64::INFINITY;

            for &d in &convex_head_distances {
                min_dist = min(min_dist, d);
                avg_distance += d;
            }

            /*
              For slurs over 3 or 4 heads, the average distance is not a
              good normalizer.
            */
            if n <= 2 {
                let fact = 1.0;
                avg_distance += self.height * fact;
                n += 1;
            }

            /*
              TODO: maybe it's better to use (avgdist - mindist)*factor
              as penalty.
            */
            avg_distance /= n as f64;
            let mut variance_penalty = p.head_slur_distance_max_ratio;
            if min_dist > 0.0 {
                variance_penalty = min(
                    avg_distance / (min_dist + p.absolute_closeness_measure) - 1.0,
                    variance_penalty,
                );
            }

            variance_penalty = max(variance_penalty, 0.0);
            variance_penalty *= p.head_slur_distance_factor;

            self.add_score(host, variance_penalty, "variance")?;
        }
        Ok(())
    }

    /// `score_extra_encompass`: ends too near a tie's, then each extra
    /// object the curve passes too near or through, by `peak_around`.
    fn score_extra_encompass(&mut self, host: &mut dyn Host, state: &State) -> Run<()> {
        let p = &state.parameters;
        // we find forbidden attachments
        let mut forbidden_attachments = Vec::new();
        for info in &state.extra_encompass_infos {
            let t = info.grob;
            if !grob::has_interface(host.heap(), t, "tie-interface") {
                continue;
            }
            let common_x = grob::vertical_axis_group(host.heap(), t).unwrap_or(Value::NULL);
            let rp = grob::relative_coordinate(host, t, common_x, X)?;
            let mut cp = grob::property(host, t, "control-points")?;

            let mut b: Bezier = [(0.0, 0.0); 4];
            for c in &mut b {
                let heap = host.heap();
                if let (Some(point), Some(rest)) = (heap.car(cp), heap.cdr(cp)) {
                    let (Some(x), Some(y)) = (heap.car(point), heap.cdr(point)) else {
                        return Err(grob::to_double(heap, point).expect_err("no pair is a number"));
                    };
                    *c = (grob::to_double(heap, x)?, grob::to_double(heap, y)?);
                    cp = rest;
                }
            }
            forbidden_attachments.push((b[0].0 + rp, b[0].1 + 0.0));
            forbidden_attachments.push((b[3].0 + rp, b[3].1 + 0.0));
        }

        let too_close = forbidden_attachments.iter().any(|f| {
            self.attachment
                .iter()
                .any(|a| length((f.0 - a.0, f.1 - a.1)) < p.slur_tie_extrema_min_distance)
        });

        if too_close {
            self.add_score(host, p.slur_tie_extrema_min_distance_penalty, "extra")?;
        }

        for info in &state.extra_encompass_infos {
            let attachment = self.attachment;

            let slur_wid = Interval::new(attachment[ix(LEFT)].0, attachment[ix(RIGHT)].0);

            /*
              to prevent numerical inaccuracies in
              Bezier::get_other_coordinate ().
            */

            let mut found = false;
            let mut y = 0.0;

            for d in [LEFT, RIGHT] {
                /*
                  We need to check for the bound explicitly, since the
                  slur-ending can be almost vertical, making the Y
                  coordinate a bad approximation of the object-slur
                  distance.
                */
                if !item::is_item(host.heap(), info.grob) {
                    continue;
                }

                let mut item_x = grob::extent(host, info.grob, state.common[X], X)?;
                item_x.intersect(state.extremes[ix(d)].slur_head_x_extent);
                if !item_x.is_empty() {
                    y = attachment[ix(d)].1;
                    found = true;
                }
            }

            if !found {
                let x = info.extents.x().linear_combination(info.idx);

                if !contains(slur_wid, x) {
                    continue;
                }

                y = y_at(host, &self.curve, x)?;
            }

            let heap = host.heap();
            let mut dist = 0.0;
            if info.type_ == heap.intern("around") {
                let ext = info.extents.y();
                dist = if y > ext.right {
                    y - ext.right
                } else if y < ext.left {
                    ext.left - y
                } else {
                    0.0
                };
            }
            /*
             Have to score too: the curve enumeration is limited in its
             shape, and may produce curves which collide anyway.
            */
            else if info.type_ == heap.intern("inside") {
                dist = f64::from(state.dir) * (y - info.extents.y().at(state.dir));
            } else {
                grob::unlocated_programming_error(host, "unknown avoidance type")?;
            }

            dist = max(dist, 0.0);

            let penalty = info.penalty
                * peak_around(
                    0.1 * p.extra_encompass_free_distance,
                    p.extra_encompass_free_distance,
                    dist,
                );

            self.add_score(host, penalty, "extra")?;
        }
        Ok(())
    }

    /// `score_edges`: each end's distance from its base, less beside an
    /// unbeamed stem towards the slur, more where the slur rises from it.
    fn score_edges(&mut self, host: &mut dyn Host, state: &State) -> Run<()> {
        let [al, ar] = self.attachment;
        let dz = (ar.0 - al.0, ar.1 - al.1);
        let slope = dz.1 / dz.0;
        let factor = state.parameters.edge_attraction_factor;
        for d in [LEFT, RIGHT] {
            let y = self.attachment[ix(d)].1;
            let dy = libm::fabs(y - state.base_attachments[ix(d)].1);
            let mut demerit = factor * dy;
            let ex = state.extremes[ix(d)];
            if let Some(stem) = ex.stem
                && ex.stem_dir == state.dir
                // TODO - Stem::get_beaming() should be precomputed.
                && stem::get_beaming(host, stem, -d)? == 0
            {
                demerit /= 5.0;
            }

            let sign = f64::from((state.dir * d).signum());
            demerit *= libm::exp(sign * slope * state.parameters.edge_slope_exponent);

            let dir_str = if d == LEFT { "L" } else { "R" };
            self.add_score(host, demerit, &format!("{dir_str} edge"))?;
        }
        Ok(())
    }

    /// `score_slopes`: a slope past `max-slope` (counted twice, as
    /// LilyPond's does), steeper than the music, not level over level
    /// music, or against it.
    fn score_slopes(&mut self, host: &mut dyn Host, state: &State) -> Run<()> {
        let p = &state.parameters;
        let dy = state.musical_dy;
        let [al, ar] = self.attachment;
        let slur_dz = (ar.0 - al.0, ar.1 - al.1);
        let slur_dy = slur_dz.1;

        let steep = max(libm::fabs(slur_dy / slur_dz.0) - p.max_slope, 0.0);
        let mut demerit = fma(steep, p.max_slope_factor, 0.0);

        /* 0.2: account for staffline offset. */
        let mut max_dy = libm::fabs(dy) + 0.2;
        if state.edge_has_beams {
            max_dy += 1.0;
        }

        if !state.is_broken {
            demerit = fma(
                p.steeper_slope_factor,
                max(libm::fabs(slur_dy) - max_dy, 0.0),
                demerit,
            );
        }

        demerit = fma(steep, p.max_slope_factor, demerit);

        // This morally checks for 0, but account for rounding errors.  TODO: use a
        // detail to set a threshold for what a 'horizontal' slur is?
        if libm::fabs(dy) < 0.01 && libm::fabs(slur_dy) > 0.01 && !state.is_broken {
            demerit += p.non_horizontal_penalty;
        }

        let sign = |x: f64| {
            if x == 0.0 {
                0
            } else if x > 0.0 {
                1
            } else {
                -1
            }
        };
        if sign(dy) != 0 && !state.is_broken && sign(slur_dy) != 0 && sign(slur_dy) != sign(dy) {
            demerit += if state.edge_has_beams {
                p.same_slope_penalty / 10.0
            } else {
                p.same_slope_penalty
            };
        }

        self.add_score(host, demerit, "slope")
    }

    /// `run_next_scorer`.
    pub fn run_next_scorer(&mut self, host: &mut dyn Host, state: &State) -> Run<()> {
        match self.next_scorer_todo {
            EXTRA_ENCOMPASS => self.score_extra_encompass(host, state)?,
            SLOPE => self.score_slopes(host, state)?,
            EDGES => self.score_edges(host, state)?,
            ENCOMPASS => self.score_encompass(host, state)?,
            _ => unreachable!("no such slur scorer"),
        }
        self.next_scorer_todo += 1;
        Ok(())
    }
}
