//! `beam-quanting.cc` (MS4 step 4): `Beam_scoring_problem`, the search for
//! a beam's two end positions among the quants of a staff, which
//! `ly:beam::quanting` (`beam.cc:1083–1095`) runs for the Beam's
//! `positions` (output-lib.scm's `beam::place-broken-parts-individually`
//! and its kin, the image's). Its instance variables read of the beam and
//! its stems (`init_instance_variables`, the stems' `stem-info` among
//! them, `Stem::get_stem_info`) and of the grobs the beam covers
//! (`covered-grobs`, `Beam_collision_engraver`'s), the unquanted positions
//! by least squares (least-squares.cc's `minimise_least_squares`, borrowed)
//! damped by the slope and the beam's concaveness and shifted to a region
//! where the stems can be long enough (`shift_region_to_valid`, over the
//! interval minefield), then each configuration of two quants near them
//! scored lazily, cheapest first, by a priority queue (libc++'s
//! `std::priority_queue`, its sift up and its Floyd sift down as LilyPond
//! 2.26.0's binary inlines them, so that configurations of equal demerits
//! come out in its order) until the cheapest is scored whole. With
//! `inspect-quants` the nearest configuration is scored, and with
//! `debug-beam-scoring` (or `inspect-quants`) its score card is the beam's
//! `annotation`.
//!
//! LilyPond's clang fuses the sums of products here (otool): the least
//! squares' sums and quotients, the concaveness's line, the start's
//! shift, the forbidden quants' gaps and their demerits, and the stem
//! lengths' scores. `tanh` and `pow` are `libm`'s.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of, host_lookup_variable};
use lily_engine_scheme::{Exception, Value};

use crate::beam::{self, BeamSegment, Slice};
use crate::direction;
use crate::geometry::{Box as Bbox, Direction, Interval, Offset, X, Y, fma, max, min};
use crate::grob;
use crate::grob_array;
use crate::interval_minefield::IntervalMinefield;
use crate::item;
use crate::spanner;
use crate::staff_symbol_referencer::{line_thickness, staff_radius, staff_space};
use crate::stem;

type Run<T> = Result<T, Exception>;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const DOWN: Direction = -1;
const UP: Direction = 1;

/// A `Drul_array`'s index of `d`: 0 left or down, 1 right or up.
fn ix(d: Direction) -> usize {
    usize::from(d > 0)
}

/// `delta`: the increase from the left to the right.
fn delta(dr: [f64; 2]) -> f64 {
    dr[1] - dr[0]
}

/// flower's `sign`: 0 for 0, else 1 if positive and -1 if not.
fn sign(x: f64) -> f64 {
    if x == 0.0 {
        0.0
    } else if x > 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// `round_halfway_up` (libc-extension.cc).
pub(crate) fn round_halfway_up(x: f64) -> f64 {
    libm::floor(x - 0.5) + 1.0
}

/// `my_modf`: the fractional part above the floor.
fn my_modf(x: f64) -> f64 {
    x - libm::floor(x)
}

/// `shrink_extra_weight`: `x` if positive, `|x| fac` if negative.
fn shrink_extra_weight(x: f64, fac: f64) -> f64 {
    libm::fabs(x) * if x < 0.0 { fac } else { 1.0 }
}

/// `get_detail`: `sym` in the alist `details` as a real, else `def`.
fn get_detail(host: &mut dyn Host, details: Value, sym: &str, def: f64) -> f64 {
    let heap = host.heap();
    let sym = heap.intern(sym);
    match grob::assq_ref(heap, sym, details) {
        Some(v) => crate::spacing_options::double_or(host, v, def),
        None => def,
    }
}

/// `Stem_info`: a stem's direction and its ideal and shortest ends.
#[derive(Clone, Copy, Default)]
pub(crate) struct StemInfo {
    pub dir: Direction,
    pub ideal_y: f64,
    pub shortest_y: f64,
}

/// `Stem::get_stem_info`: its direction and its `stem-info`'s two reals.
pub(crate) fn get_stem_info(host: &mut dyn Host, me: Value) -> Run<StemInfo> {
    let dir = direction::get_grob_direction(host, me)?;
    let scm_info = grob::property(host, me, "stem-info")?;
    let heap = host.heap();
    let (Some(ideal), Some(rest)) = (heap.car(scm_info), heap.cdr(scm_info)) else {
        return Err(crate::wrong_type(heap, Some("car"), 1, scm_info, "pair"));
    };
    let Some(shortest) = heap.car(rest) else {
        return Err(crate::wrong_type(heap, Some("car"), 1, rest, "pair"));
    };
    Ok(StemInfo {
        dir,
        ideal_y: grob::to_double(heap, ideal)?,
        shortest_y: grob::to_double(heap, shortest)?,
    })
}

/// `minimise_least_squares` (least-squares.cc, borrowed): the slope and
/// offset of the line closest to `input`; LilyPond's programming error and
/// the mean for none or one x.
fn minimise_least_squares(host: &mut dyn Host, input: &[Offset]) -> Run<(f64, f64)> {
    let (mut sx, mut sy, mut sqx, mut sxy) = (0.0, 0.0, 0.0, 0.0);

    for &(x, y) in input {
        sx += x;
        sy += y;
        sqx += x * x;
        sxy = fma(x, y, sxy);
    }

    let count = input.len() as f64;

    let den = fma(count, sqx, -(sx * sx));
    if count == 0.0 || den == 0.0 {
        grob::unlocated_programming_error(
            host,
            "minimise_least_squares ():  Nothing to minimise\n\
             This means that vertical spacing is triggered\n\
             before line breaking\n",
        )?;
        return Ok((0.0, if count != 0.0 { sy / count } else { 0.0 }));
    }
    let coef = fma(count, sxy, -(sx * sy)) / den;
    Ok((coef, fma(-coef, sx, sy) / count))
}

// --- the parameters ---------------------------------------------------------

/// `Beam_quant_parameters`: the beam's `details`.
#[derive(Default)]
struct Parameters {
    secondary_beam_demerit: f64,
    stem_length_demerit_factor: f64,
    region_size: f64,
    /// Threshold to combat rounding errors.
    beam_eps: f64,
    /// Possibly ridiculous, but too short stems just won't do.
    stem_length_limit_penalty: f64,
    damping_direction_penalty: f64,
    musical_direction_factor: f64,
    hint_direction_penalty: f64,
    ideal_slope_factor: f64,
    round_to_zero_slope: f64,
    collision_penalty: f64,
    collision_padding: f64,
    horizontal_inter_quant_penalty: f64,
    stem_collision_factor: f64,
}

impl Parameters {
    /// `fill`.
    fn fill(host: &mut dyn Host, him: Value) -> Run<Parameters> {
        let details = grob::property(host, him, "details")?;
        let mut d = |name: &str, def: f64| get_detail(host, details, name, def);

        // General
        let beam_eps = d("beam-eps", 1e-3);
        let region_size = d("region-size", 2.0);

        // forbidden quants
        let secondary = d("secondary-beam-demerit", 10.0);
        let stem_length_demerit_factor = d("stem-length-demerit-factor", 5.0);
        let horizontal_inter_quant_penalty = d("horizontal-inter-quant", 500.0);

        let stem_length_limit_penalty = d("stem-length-limit-penalty", 5000.0);
        let damping_direction_penalty = d("damping-direction-penalty", 800.0);
        let hint_direction_penalty = d("hint-direction-penalty", 20.0);
        let musical_direction_factor = d("musical-direction-factor", 400.0);
        let ideal_slope_factor = d("ideal-slope-factor", 10.0);
        let round_to_zero_slope = d("round-to-zero-slope", 0.02);

        // Collisions
        let collision_penalty = d("collision-penalty", 500.0);
        let collision_padding = d("collision-padding", 0.5);
        let stem_collision_factor = d("stem-collision-factor", 0.1);

        let v = grob::property(host, him, "length-fraction")?;
        let length_fraction = crate::spacing_options::double_or(host, v, 1.0);

        Ok(Parameters {
            beam_eps,
            region_size,
            // For stems that are non-standard, the forbidden beam quanting
            // doesn't really work, so decrease their importance.
            secondary_beam_demerit: secondary * libm::exp(-8.0 * libm::fabs(1.0 - length_fraction)),
            stem_length_demerit_factor,
            horizontal_inter_quant_penalty,
            stem_length_limit_penalty,
            damping_direction_penalty,
            hint_direction_penalty,
            musical_direction_factor,
            ideal_slope_factor,
            round_to_zero_slope,
            collision_penalty,
            // For grace notes, beams get scaled down to 80%, but glyphs go
            // down to 63% (magstep -4 for accidentals). To make the padding
            // commensurate with glyph size for grace notes, we take the
            // square of the length fraction, yielding a 64% decrease.
            collision_padding: collision_padding * (length_fraction * length_fraction),
            stem_collision_factor,
        })
    }
}

// --- the configurations -----------------------------------------------------

/// The scorers, in order of increasing expensiveness.
const ORIGINAL_DISTANCE: usize = 0;
const SLOPE_IDEAL: usize = 1;
const SLOPE_MUSICAL: usize = 2;
const SLOPE_DIRECTION: usize = 3;
const HORIZONTAL_INTER: usize = 4;
const FORBIDDEN: usize = 5;
const STEM_LENGTHS: usize = 6;
const COLLISIONS: usize = 7;
const NUM_SCORERS: usize = 8;

/// `Beam_configuration`: the two ends, the demerits so far, the score
/// card and the next scorer.
struct Configuration {
    y: [f64; 2],
    demerits: f64,
    score_card: String,
    next_scorer_todo: usize,
}

impl Configuration {
    fn done(&self) -> bool {
        self.next_scorer_todo >= NUM_SCORERS
    }

    /// `add`: the demerit, and in the card unless it is 0.
    fn add(&mut self, demerit: f64, reason: &str) {
        self.demerits += demerit;

        if demerit != 0.0 {
            self.score_card.push_str(&format!(" {reason} {demerit:.2}"));
        }
    }

    /// `new_config`: the start's ends truncated to integers and offset,
    /// first ordered by the offsets, the combinations closest to the
    /// ideal offset first.
    fn new_config(start: [f64; 2], offset: [f64; 2]) -> Configuration {
        let start_score = libm::fabs(offset[1]) + libm::fabs(offset[0]);
        Configuration {
            y: [
                f64::from(start[0] as i32) + offset[0],
                f64::from(start[1] as i32) + offset[1],
            ],
            demerits: start_score / 1000.0,
            score_card: String::new(),
            next_scorer_todo: ORIGINAL_DISTANCE + 1,
        }
    }
}

/// `Beam_collision`: a thing the beam should not cross, at `x`, over `y`,
/// and the beams' heights there.
#[derive(Clone, Copy)]
struct Collision {
    x: f64,
    y: Interval,
    base_penalty: f64,
    /// Need to add the configuration's y to get actual offsets.
    beam_y: Interval,
}

// --- the problem ------------------------------------------------------------

/// `Beam_scoring_problem`: what scoring a configuration needs, precomputed.
struct Problem {
    beam: Value,
    unquanted_y: [f64; 2],
    align_broken_intos: bool,
    do_initial_slope_calculations: bool,
    staff_space: f64,
    beam_thickness: f64,
    line_thickness: f64,
    musical_dy: f64,
    normal_stem_count: usize,
    x_span: f64,
    stem_infos: Vec<StemInfo>,
    chord_start_y: Vec<f64>,
    head_positions: Vec<Interval>,
    beam_multiplicity: Vec<Slice>,
    is_normal: Vec<bool>,
    base_lengths: Vec<f64>,
    stem_xpositions: Vec<f64>,
    stem_ypositions: Vec<f64>,
    is_xstaff: bool,
    is_knee: bool,
    parameters: Parameters,
    staff_radius: f64,
    edge_beam_counts: [i32; 2],
    edge_dirs: [Direction; 2],
    /// Half-open intervals, the positions allowed for the beam from close
    /// to the note head towards the stem's end, which weed out invalid
    /// configurations quickly.
    quant_range: [Interval; 2],
    max_beam_count: i32,
    length_fraction: f64,
    beam_translation: f64,
    collisions: Vec<Collision>,
    segments: Vec<BeamSegment>,
}

/// `Interval::set_full`.
const FULL: Interval = Interval::new(f64::NEG_INFINITY, f64::INFINITY);

fn contains(iv: Interval, x: f64) -> bool {
    iv.left <= x && x <= iv.right
}

/// `Interval::distance`: from `t` to the interval, 0 inside it.
fn distance(iv: Interval, t: f64) -> f64 {
    if t > iv.right {
        t - iv.right
    } else if t < iv.left {
        iv.left - t
    } else {
        0.0
    }
}

impl Problem {
    /// `Beam_scoring_problem (me, ys, align_broken_intos)`.
    fn new(host: &mut dyn Host, me: Value, ys: [f64; 2], align_broken_intos: bool) -> Run<Problem> {
        let parameters = Parameters::fill(host, me)?;
        let mut p = Problem {
            beam: me,
            unquanted_y: ys,
            align_broken_intos,
            do_initial_slope_calculations: false,
            staff_space: 0.0,
            beam_thickness: 0.0,
            line_thickness: 0.0,
            musical_dy: 0.0,
            normal_stem_count: 0,
            x_span: 0.0,
            stem_infos: Vec::new(),
            chord_start_y: Vec::new(),
            head_positions: Vec::new(),
            beam_multiplicity: Vec::new(),
            is_normal: Vec::new(),
            base_lengths: Vec::new(),
            stem_xpositions: Vec::new(),
            stem_ypositions: Vec::new(),
            is_xstaff: false,
            is_knee: false,
            parameters,
            staff_radius: 0.0,
            edge_beam_counts: [0; 2],
            edge_dirs: [0; 2],
            quant_range: [FULL; 2],
            max_beam_count: 0,
            length_fraction: 1.0,
            beam_translation: 0.0,
            collisions: Vec::new(),
            segments: Vec::new(),
        };
        p.init_instance_variables(host, me, ys, align_broken_intos)?;
        if p.do_initial_slope_calculations {
            p.least_squares_positions(host)?;
            p.slope_damping(host)?;
            p.shift_region_to_valid(host)?;
        }
        Ok(p)
    }

    /// `y_at`: the configuration's y at `x`.
    fn y_at(&self, x: f64, c: &Configuration) -> f64 {
        c.y[0] + x * delta(c.y) / self.x_span
    }

    /// `add_collision`: the thing at `x` over `y` (in staff spaces), with
    /// the beams' heights over it.
    fn add_collision(&mut self, x: f64, mut y: Interval, score_factor: f64) {
        // We used to screen for quant range, but no more.
        let mut beam_y = Interval::EMPTY;

        for s in &self.segments {
            if contains(s.horizontal, x) {
                beam_y.add_point(f64::from(s.vertical_count) * self.beam_translation);
            }
            if s.horizontal.left > x {
                break;
            }
        }
        beam_y.widen(0.5 * self.beam_thickness);

        y.scale(1.0 / self.staff_space);
        self.collisions.push(Collision {
            x,
            y,
            base_penalty: score_factor,
            beam_y,
        });
    }

    /// `init_instance_variables`.
    fn init_instance_variables(
        &mut self,
        host: &mut dyn Host,
        me: Value,
        ys: [f64; 2],
        align_broken_intos: bool,
    ) -> Run<()> {
        // If 'ys' are finite, use them as starting points for y-positions
        // of the ends of the beam, instead of the best-fit through the
        // natural ends of the stems.  Otherwise, we want to do initial
        // slope calculations.
        self.do_initial_slope_calculations = !ys[0].is_finite() || !ys[1].is_finite();

        // Calculations are relative to a unit-scaled staff, i.e. the quants
        // are divided by the current staff_space_.
        self.staff_space = staff_space(host, me)?;
        self.beam_thickness = beam::get_beam_thickness(host, me)? / self.staff_space;
        self.line_thickness = line_thickness(host, me)? / self.staff_space;
        self.max_beam_count = beam::get_beam_count(host, me)?;
        let v = grob::property(host, me, "length-fraction")?;
        self.length_fraction = crate::spacing_options::double_or(host, v, 1.0);
        // This is the least-squares DY, corrected for concave beams.
        let v = grob::property(host, me, "least-squares-dy")?;
        self.musical_dy = crate::spacing_options::double_or(host, v, 0.0);

        let mut beams = Vec::new();
        self.align_broken_intos = align_broken_intos;
        if self.align_broken_intos {
            match grob::original(host.heap(), me) {
                Some(orig) if orig != me || spanner::is_broken(host.heap(), orig) => {
                    let pieces = spanner::broken_intos(host.heap(), orig);
                    if pieces.is_empty() {
                        self.align_broken_intos = false;
                    } else {
                        beams.extend(pieces);
                    }
                }
                _ => self.align_broken_intos = false,
            }
        }
        if !self.align_broken_intos {
            beams.push(me);
        }

        // x_span_ is a single scalar, cumulatively summing the length of
        // all the segments the parent beam was broken-into.
        self.x_span = 0.0;
        self.is_knee = false;
        self.normal_stem_count = 0;
        for &b in &beams {
            let stems = grob_array::extract_grob_array(host, b, "stems")?;
            let fake_collisions = grob_array::extract_grob_array(host, b, "covered-grobs")?;
            let mut collisions = Vec::new();

            let b_system = item::get_system(host.heap(), b);
            for &c in &fake_collisions {
                if item::get_system(host.heap(), c) == b_system {
                    collisions.push(c);
                }
            }

            let mut common = [Value::NULL; 2];
            for a in [X, Y] {
                common[a] = grob::common_refpoint_of_array(host.heap(), &stems, Some(b), a)
                    .unwrap_or(Value::NULL);
            }

            for d in [LEFT, RIGHT] {
                if let Some(bound) = spanner::get_bound(host, b, d)? {
                    common[X] = grob::common_refpoint(host.heap(), bound, common[X], X)
                        .unwrap_or(Value::NULL);
                }
            }

            // positions of the endpoints of this beam segment, including
            // any overhangs
            let x_pos = {
                let v = grob::property(host, b, "X-positions")?;
                grob::number_pair(host.heap(), v).unwrap_or(Interval::new(0.0, 0.0))
            };

            let normal = grob_array::extract_grob_array(host, b, "normal-stems")?;
            let edge_stems = [normal.first().copied(), normal.last().copied()];

            let mut dirs_found = [false; 2];

            let my_y = grob::relative_coordinate(host, b, common[Y], Y)?;

            let mut beam_width = Interval::new(-1.0, -1.0);
            for &s in &stems {
                self.beam_multiplicity
                    .push(stem::beam_multiplicity(host, s)?);
                self.head_positions.push(stem::head_positions(host, s)?);
                self.is_normal.push(stem::is_normal_stem(host, s)?);

                let mut si = get_stem_info(host, s)?;
                si.ideal_y /= self.staff_space;
                si.shortest_y /= self.staff_space;
                self.stem_infos.push(si);
                self.chord_start_y.push(stem::chord_start_y(host, s)?);
                if si.dir != 0 {
                    dirs_found[ix(si.dir)] = true;
                }

                let stem_end = beam::calc_stem_y(
                    host,
                    b,
                    s,
                    common,
                    x_pos.left,
                    x_pos.right,
                    0,
                    Interval::new(0.0, 0.0),
                    0,
                )?;
                let y = stem_end.stem_y;
                // Remark:  French Beaming is irrelevant for beam quanting
                self.base_lengths.push(y / self.staff_space);
                let sx = grob::relative_coordinate(host, s, common[X], X)?;
                self.stem_xpositions.push(sx - x_pos.left + self.x_span);
                let sy = grob::relative_coordinate(host, s, common[Y], Y)?;
                self.stem_ypositions.push(sy - my_y);

                if self.is_normal.last() == Some(&true) {
                    let last = self.stem_xpositions[self.stem_xpositions.len() - 1];
                    if beam_width.left == -1.0 {
                        beam_width.left = last;
                    }
                    beam_width.right = last;
                }
            }

            self.edge_dirs = [0; 2];
            self.normal_stem_count += beam::normal_stem_count(host, b)?;
            if self.normal_stem_count != 0 {
                self.edge_dirs = [
                    self.stem_infos[0].dir,
                    self.stem_infos[self.stem_infos.len() - 1].dir,
                ];
            }

            self.is_xstaff = grob::has_interface(host.heap(), common[Y], "align-interface");
            self.is_knee |= dirs_found[0] && dirs_found[1];

            self.staff_radius = staff_radius(host, b)?;
            let (Some(&first), Some(&last)) = (stems.first(), stems.last()) else {
                return Err(crate::misc_error(
                    host.heap(),
                    "Beam_scoring_problem",
                    "a beam with no stems",
                ));
            };
            self.edge_beam_counts = [
                stem::beam_multiplicity(host, first)?
                    .length()
                    .wrapping_add(1),
                stem::beam_multiplicity(host, last)?
                    .length()
                    .wrapping_add(1),
            ];

            // TODO - why are we dividing by staff_space_?
            self.beam_translation = beam::get_beam_translation(host, b)? / self.staff_space;

            for d in [LEFT, RIGHT] {
                self.quant_range[ix(d)] = FULL;
                let Some(edge) = edge_stems[ix(d)] else {
                    continue;
                };

                let stem_offset = grob::relative_coordinate(host, edge, common[Y], Y)?
                    - grob::relative_coordinate(host, b, common[Y], Y)?;
                let mut heads = stem::head_positions(host, edge)?;
                heads.scale(0.5);
                heads.scale(self.staff_space);

                let ed = self.edge_dirs[ix(d)];
                heads.widen(fma(
                    self.beam_thickness,
                    0.5,
                    fma(
                        0.5,
                        self.staff_space,
                        f64::from(self.edge_beam_counts[ix(d)] - 1) * self.beam_translation,
                    ),
                ));
                *self.quant_range[ix(d)].at_mut(-ed) = heads.at(ed) + stem_offset;
            }

            self.segments = beam::get_beam_segments(host, b)?;
            lily_engine_skyline::sort(&mut self.segments, &|a: &BeamSegment, b: &BeamSegment| {
                a.horizontal.left < b.horizontal.left
            });
            for s in &mut self.segments {
                s.horizontal.translate(self.x_span - x_pos.left);
            }

            let mut colliding_stems: Vec<Value> = Vec::new();
            for &c in &collisions {
                if !grob::is_live_grob(host.heap(), c) {
                    continue;
                }

                if grob::has_interface(host.heap(), c, "beam-interface")
                    && beam::is_cross_staff(host, c)?
                {
                    continue;
                }

                let mut bx = Bbox::EMPTY;
                for a in [X, Y] {
                    bx.0[a] = grob::extent(host, c, common[a], a)?;
                }

                if bx.0[X].right < x_pos.left || bx.0[X].left > x_pos.right {
                    continue;
                }
                if bx.0[X].is_empty() || bx.0[Y].is_empty() {
                    continue;
                }

                bx.0[X].translate(self.x_span - x_pos.left);
                bx.0[Y].translate(-my_y);
                let width = bx.0[X].length();
                let width_factor = libm::sqrt(width / self.staff_space);

                for d in [LEFT, RIGHT] {
                    self.add_collision(bx.0[X].at(d), bx.0[Y], width_factor);
                }

                let sym = host.heap().intern("stem");
                let stem = grob_array::object(host, c, sym)?;
                if grob_of(host.heap(), stem).is_some()
                    && grob::has_interface(host.heap(), stem, "stem-interface")
                    && stem::is_normal_stem(host, stem)?
                    && !colliding_stems.contains(&stem)
                {
                    colliding_stems.push(stem);
                }
            }

            // `std::unordered_set`'s order, which LilyPond's binary gives
            // by the stems' addresses: by their records here.
            colliding_stems.sort_by_key(|&s| grob_of(host.heap(), s).map(|(_, id)| id));
            for s in colliding_stems {
                let mut ext = grob::robust_relative_extent(host, s, common[X], X)?;
                ext.translate(-x_pos.left);
                ext.translate(self.x_span);
                let x = ext.center();

                let stem_dir = direction::get_grob_direction(host, s)?;
                let mut y = FULL;
                *y.at_mut(-stem_dir) = stem::chord_start_y(host, s)?
                    + grob::relative_coordinate(host, s, common[Y], Y)?
                    - my_y;

                let mut factor = self.parameters.stem_collision_factor;
                if stem::get_beam(host, s)?.is_none() {
                    factor = 1.0;
                }
                self.add_collision(x, y, factor);
            }
            self.x_span += spanner::spanner_length(host, b)?;
        }
        Ok(())
    }

    /// `no_visible_stem_positions`: the heads' extreme on the beam's side
    /// past their beams.
    fn no_visible_stem_positions(&mut self, host: &mut dyn Host) -> Run<()> {
        if self.head_positions.is_empty() {
            self.unquanted_y = [0.0; 2];
            return Ok(());
        }

        let mut head_positions = Interval::EMPTY;
        let mut multiplicity = Slice::EMPTY;
        for i in 0..self.head_positions.len() {
            head_positions.unite(self.head_positions[i]);
            multiplicity.unite(self.beam_multiplicity[i]);
        }

        let dir = direction::get_grob_direction(host, self.beam)?;

        if dir == 0 {
            grob::unlocated_programming_error(host, "The beam should have a direction by now.")?;
        }

        let y = fma(
            f64::from(dir) * self.beam_translation,
            f64::from(multiplicity.length().wrapping_add(1)),
            head_positions.linear_combination(f64::from(dir)) * 0.5 * self.staff_space,
        );

        self.unquanted_y = [y, y];
        Ok(())
    }

    /// `first_normal_index`, LilyPond's programming error for none.
    fn first_normal_index(&self, host: &mut dyn Host) -> Run<usize> {
        if let Some(i) = self.is_normal.iter().position(|&n| n) {
            return Ok(i);
        }
        grob::programming_error(
            host,
            self.beam,
            "No normal stems, but asking for first normal stem index.",
        )?;
        Ok(0)
    }

    /// `last_normal_index`.
    fn last_normal_index(&self, host: &mut dyn Host) -> Run<usize> {
        if let Some(i) = self.is_normal.iter().rposition(|&n| n) {
            return Ok(i);
        }
        grob::programming_error(
            host,
            self.beam,
            "No normal stems, but asking for first normal stem index.",
        )?;
        Ok(0)
    }

    /// `least_squares_positions`: the line closest to the normal stems'
    /// ideal ends, at least the smallest quant steep, or of the ideal ends
    /// where they are level (two stems on the middle line sloped slightly);
    /// kept as `least-squares-dy`.
    fn least_squares_positions(&mut self, host: &mut dyn Host) -> Run<()> {
        if self.normal_stem_count == 0 {
            return self.no_visible_stem_positions(host);
        }

        if self.stem_infos.is_empty() {
            return Ok(());
        }

        let fnx = self.first_normal_index(host)?;
        let lnx = self.last_normal_index(host)?;

        let ideal = [
            self.stem_infos[fnx].ideal_y + self.stem_ypositions[fnx],
            self.stem_infos[lnx].ideal_y + self.stem_ypositions[lnx],
        ];

        let ldy;
        if delta(ideal) == 0.0 {
            let chord = [
                self.chord_start_y[0],
                self.chord_start_y[self.chord_start_y.len() - 1],
            ];

            // Simple beams (2 stems) on middle line should be allowed to be
            // slightly sloped.
            //
            // However, if both stems reach middle line, ideal[LEFT] ==
            // ideal[RIGHT] and delta (ideal) == 0.
            //
            // For that case, we apply artificial slope
            if ideal[0] == 0.0 && delta(chord) != 0.0 && self.stem_infos.len() == 2 {
                let d = beam_dir_of(delta(chord));
                let half = beam::get_beam_thickness(host, self.beam)? / 2.0;
                self.unquanted_y[ix(d)] = half;
                self.unquanted_y[ix(-d)] = -half;
            } else {
                self.unquanted_y = ideal;
            }

            ldy = self.unquanted_y[1] - self.unquanted_y[0];
        } else {
            let mut ideals = Vec::new();
            for i in 0..self.stem_infos.len() {
                if self.is_normal[i] {
                    ideals.push((
                        self.stem_xpositions[i],
                        self.stem_infos[i].ideal_y + self.stem_ypositions[i],
                    ));
                }
            }

            let (slope, y) = minimise_least_squares(host, &ideals)?;

            let mut dy = slope * self.x_span;

            set_minimum_dy(host, self.beam, &mut dy)?;

            ldy = dy;
            self.unquanted_y = [y, y + dy];
        }

        self.musical_dy = ldy;
        let v = host.heap().flonum(self.musical_dy);
        grob::set_property(host.heap(), self.beam, "least-squares-dy", v);
        Ok(())
    }

    /// `calc_concaveness`: `concaveness` if it is a number; else 0 for a
    /// knee, a cross-staff beam or one of two normal stems, 10000 for a
    /// concave beam of single notes, and the mean of the far and close
    /// heads' concaveness otherwise.
    fn calc_concaveness(&self, host: &mut dyn Host) -> Run<f64> {
        let conc = grob::property(host, self.beam, "concaveness")?;
        if host.heap().is_number(conc) {
            return grob::to_double(host.heap(), conc);
        }

        if self.is_knee || self.is_xstaff {
            return Ok(0.0);
        }

        let mut beam_dir = 0;
        for i in (0..self.is_normal.len()).rev() {
            if self.is_normal[i] && self.stem_infos[i].dir != 0 {
                beam_dir = self.stem_infos[i].dir;
            }
        }

        if self.normal_stem_count <= 2 {
            return Ok(0.0);
        }

        let mut close_positions = Vec::new();
        let mut far_positions = Vec::new();
        for i in 0..self.is_normal.len() {
            if self.is_normal[i] {
                // For chords, we take the note head that is closest to the
                // beam.
                //
                // Hmmm.. wait, for the beams in the last measure of
                // morgenlied, this doesn't look so good. Let's try the
                // heads farthest from the beam.
                close_positions.push(libm::rint(self.head_positions[i].at(beam_dir)) as i32);
                far_positions.push(libm::rint(self.head_positions[i].at(-beam_dir)) as i32);
            }
        }

        let concaveness = if is_concave_single_notes(
            if beam_dir == UP {
                &close_positions
            } else {
                &far_positions
            },
            beam_dir,
        ) {
            10000.0
        } else {
            (calc_positions_concaveness(&far_positions, beam_dir)
                + calc_positions_concaveness(&close_positions, beam_dir))
                / 2.0
        };

        Ok(concaveness)
    }

    /// `slope_damping`: the slope damped by `damping` and the concaveness,
    /// a concave beam made level.
    fn slope_damping(&mut self, host: &mut dyn Host) -> Run<()> {
        if self.normal_stem_count <= 1 {
            return Ok(());
        }

        let s = grob::property(host, self.beam, "damping")?;
        let mut damping = grob::to_double(host.heap(), s)?;
        let concaveness = self.calc_concaveness(host)?;
        if concaveness >= 10000.0 || damping >= 10000.0 {
            self.unquanted_y[0] = self.unquanted_y[1];
            self.musical_dy = 0.0;
            damping = 0.0;
        }

        if damping != 0.0 && (damping + concaveness) != 0.0 {
            let dy = self.unquanted_y[1] - self.unquanted_y[0];

            let mut slope = if dy != 0.0 && self.x_span != 0.0 {
                dy / self.x_span
            } else {
                0.0
            };

            slope = 0.6 * libm::tanh(slope) / (damping + concaveness);

            let mut damped_dy = slope * self.x_span;

            set_minimum_dy(host, self.beam, &mut damped_dy)?;

            self.unquanted_y[0] += (dy - damped_dy) / 2.0;
            self.unquanted_y[1] -= (dy - damped_dy) / 2.0;
        }
        Ok(())
    }

    /// `shift_region_to_valid`: the start moved where the stems can be
    /// long enough and, past what the collisions forbid, the nearest
    /// feasible place; LilyPond's warning where there is none.
    fn shift_region_to_valid(&mut self, host: &mut dyn Host) -> Run<()> {
        if self.normal_stem_count == 0 {
            return Ok(());
        }

        let beam_dy = self.unquanted_y[1] - self.unquanted_y[0];
        let slope = if self.x_span != 0.0 {
            beam_dy / self.x_span
        } else {
            0.0
        };

        // Shift the positions so that we have a chance of finding good
        // quants (i.e. no short stem failures.)
        let mut feasible_left_point = FULL;

        for i in 0..self.stem_infos.len() {
            // TODO - check for invisible here...
            let mut left_y = fma(
                -slope,
                self.stem_xpositions[i],
                self.stem_infos[i].shortest_y,
            );

            // left_y is now relative to the stem S. We want relative to
            // ourselves, so translate:
            left_y += self.stem_ypositions[i];
            let mut flp = FULL;
            *flp.at_mut(-self.stem_infos[i].dir) = left_y;

            feasible_left_point.intersect(flp);
        }

        // We only update these for objects that are too large for quanting
        // to find a workaround.  Typically, these are notes with stems, and
        // timesig/keysig/clef, which take out the entire area inside the
        // staff as feasible.
        //
        // The code below disregards the thickness and multiplicity of the
        // beam.  This should not be a problem, as the beam quanting will
        // take care of computing the impact those exactly.
        let min_y_size = 2.0;

        // A list of intervals into which beams may not fall
        let mut forbidden_intervals = Vec::new();

        for c in &self.collisions {
            if c.x < 0.0 || c.x > self.x_span {
                continue;
            }

            if c.y.length() < min_y_size {
                continue;
            }

            let dy = slope * c.x;

            forbidden_intervals.push(Interval::new(c.y.left - dy, c.y.right - dy));
        }

        lily_engine_skyline::sort(&mut forbidden_intervals, &|a: &Interval, b: &Interval| {
            a.left < b.left
        });
        let mut beam_left_y = self.unquanted_y[0];
        let feasible_beam_placements = Interval::new(beam_left_y, beam_left_y);

        let mut minefield = IntervalMinefield::new(feasible_beam_placements, 0.0);
        for f in forbidden_intervals {
            minefield.add_forbidden_interval(f);
        }
        minefield.solve();
        let mut feasible_beam_placements = minefield.feasible_placements();

        // if the beam placement falls out of the feasible region, we push
        // it to infinity so that it can never be a feasible candidate below
        for d in [DOWN, UP] {
            if !contains(feasible_left_point, feasible_beam_placements.at(d)) {
                *feasible_beam_placements.at_mut(d) = f64::from(d) * f64::INFINITY;
            }
        }

        if feasible_beam_placements.right == f64::INFINITY
            && feasible_beam_placements.left == f64::NEG_INFINITY
            && !feasible_left_point.is_empty()
        {
            // We are somewhat screwed: we have a collision, but at least
            // there is a way to satisfy stem length constraints.
            beam_left_y = point_in_interval(feasible_left_point, 2.0);
        } else if !feasible_left_point.is_empty() {
            // Only one of them offers is feasible solution. Pick that one.
            if libm::fabs(beam_left_y - feasible_beam_placements.left)
                > libm::fabs(beam_left_y - feasible_beam_placements.right)
            {
                beam_left_y = feasible_beam_placements.right;
            } else {
                beam_left_y = feasible_beam_placements.left;
            }
        } else {
            // We are completely screwed.
            grob::warning(
                host,
                self.beam,
                "no viable initial configuration found: may not find good beam slope",
            )?;
        }

        self.unquanted_y = [beam_left_y, beam_left_y + beam_dy];
        Ok(())
    }

    /// `generate_quants`: each pair of quants (straddle, sit, inter and
    /// hang at each staff space of the region) near the unquanted ends, a
    /// widened beam's outside the staff shifted, that the quant ranges
    /// allow.
    fn generate_quants(&self) -> Vec<Configuration> {
        let mut region_size = self.parameters.region_size as i32;

        // Knees and collisions are harder, lets try some more possibilities
        if self.is_knee {
            region_size += 2;
        }
        if !self.collisions.is_empty() {
            region_size += 2;
        }

        let straddle = 0.0;
        let sit = (self.beam_thickness - self.line_thickness) / 2.0;
        let inter = 0.5;
        let hang = 1.0 - (self.beam_thickness - self.line_thickness) / 2.0;
        let base_quants = [straddle, sit, inter, hang];

        // for normal-sized beams, in case of more than 4 beams, the outer
        // beam used for generating quants will never interfere with staff
        // lines, but prevent the inside-staff beams from being neatly
        // positioned. A correctional grid_shift has to be applied to
        // compensate.
        let mut grid_shift = 0.0;
        // grid shift only makes sense for widened normal-sized beams:
        if !self.is_knee && self.max_beam_count > 4 && self.length_fraction == 1.0 {
            grid_shift = f64::from(self.max_beam_count - 4) * (1.0 - self.beam_translation);
        }

        // Asymetry ? should run to <= region_size ?
        let mut unshifted_quants = Vec::new();
        for i in -region_size..region_size {
            for q in base_quants {
                unshifted_quants.push(f64::from(i) + q);
            }
        }

        let mut scores = Vec::new();
        for &qi in &unshifted_quants {
            for &qj in &unshifted_quants {
                let mut corr = [0.0; 2];
                if grid_shift != 0.0 {
                    for d in [LEFT, RIGHT] {
                        // apply grid shift if quant outside 5-line staff:
                        let ed = f64::from(self.edge_dirs[ix(d)]);
                        if (self.unquanted_y[ix(d)] + qi) * ed > 2.5 {
                            corr[ix(d)] = grid_shift * ed;
                        }
                    }
                }
                let c = Configuration::new_config(self.unquanted_y, [qi - corr[0], qj - corr[1]]);

                if (0..2).all(|d| contains(self.quant_range[d], c.y[d])) {
                    scores.push(c);
                }
            }
        }
        scores
    }

    /// `one_scorer`: the configuration's next scorer.
    fn one_scorer(&self, c: &mut Configuration) {
        match c.next_scorer_todo {
            SLOPE_IDEAL => self.score_slope_ideal(c),
            SLOPE_DIRECTION => self.score_slope_direction(c),
            SLOPE_MUSICAL => self.score_slope_musical(c),
            FORBIDDEN => self.score_forbidden_quants(c),
            STEM_LENGTHS => self.score_stem_lengths(c),
            COLLISIONS => self.score_collisions(c),
            HORIZONTAL_INTER => self.score_horizontal_inter_quants(c),
            _ => {}
        }
        c.next_scorer_todo += 1;
    }

    /// `force_score`: the configuration nearest `ins`, scored whole;
    /// LilyPond's programming error if none is near.
    fn force_score(
        &self,
        host: &mut dyn Host,
        ins: [f64; 2],
        configs: &mut [Configuration],
    ) -> Run<usize> {
        let mut mindist = 1e6;
        let mut best = None;
        for (i, c) in configs.iter().enumerate() {
            let d = libm::fabs(c.y[0] - ins[0]) + libm::fabs(c.y[1] - ins[1]);
            if d < mindist {
                best = Some(i);
                mindist = d;
            }
        }
        if mindist > 1e5 {
            grob::unlocated_programming_error(host, "cannot find quant")?;
        }

        let Some(best) = best else {
            return Err(crate::misc_error(
                host.heap(),
                "force_score",
                "cannot find quant",
            ));
        };
        while !configs[best].done() {
            let mut c = core::mem::replace(
                &mut configs[best],
                Configuration::new_config([0.0; 2], [0.0; 2]),
            );
            self.one_scorer(&mut c);
            configs[best] = c;
        }

        Ok(best)
    }

    /// `solve`: the cheapest configuration scored whole, or the unquanted
    /// ends for none or with `skip-quanting`; with debugging its score card
    /// as the beam's `annotation`; aligned with the beam's broken pieces'
    /// `normalized-endpoints` for `align_broken_intos`.
    fn solve(&self, host: &mut dyn Host) -> Run<[f64; 2]> {
        let mut configs = self.generate_quants();

        if configs.is_empty() {
            grob::unlocated_programming_error(
                host,
                "No viable beam quanting found.  Using unquanted y value.",
            )?;
            return Ok(self.unquanted_y);
        }

        if grob::property(host, self.beam, "skip-quanting")? == Value::TRUE {
            return Ok(self.unquanted_y);
        }

        let mut debug = {
            let odef =
                grob_of(host.heap(), self.beam).map(|(arena, _)| arena.borrow().output_def());
            let sym = host.heap().intern("debug-beam-scoring");
            odef.and_then(|odef| host_lookup_variable(host, odef, sym)) == Some(Value::TRUE)
        };
        let inspect_quants = grob::property(host, self.beam, "inspect-quants")?;
        let best = if host.heap().car(inspect_quants).is_some() {
            debug = true;
            let ins = match grob::number_pair(host.heap(), inspect_quants) {
                Some(i) => [i.left, i.right],
                None => {
                    return Err(crate::wrong_type(
                        host.heap(),
                        None,
                        0,
                        inspect_quants,
                        "pair of numbers",
                    ));
                }
            };
            self.force_score(host, ins, &mut configs)?
        } else {
            // libc++'s priority queue of the configurations, cheapest on
            // top.
            let less = |configs: &[Configuration], l: usize, r: usize| {
                configs[l].demerits > configs[r].demerits
            };
            let mut queue: Vec<usize> = Vec::with_capacity(configs.len());
            for i in 0..configs.len() {
                queue.push(i);
                let n = queue.len();
                sift_up(&mut queue, n, &|l, r| less(&configs, l, r));
            }

            // TODO: It would be neat if we generated new configurations on
            // the fly, depending on the best complete score so far, that
            // would allow us to do away with region_size altogether.
            loop {
                let best = queue[0];
                if configs[best].done() {
                    break best;
                }

                pop_heap(&mut queue, &|l, r| less(&configs, l, r));
                queue.pop();
                let mut c = core::mem::replace(
                    &mut configs[best],
                    Configuration::new_config([0.0; 2], [0.0; 2]),
                );
                self.one_scorer(&mut c);
                configs[best] = c;
                queue.push(best);
                let n = queue.len();
                sift_up(&mut queue, n, &|l, r| less(&configs, l, r));
            }
        };

        let mut final_positions = configs[best].y;

        if debug {
            // debug quanting
            let completed = configs.iter().filter(|c| c.done()).count();

            let card = format!(
                "{} c{}/{}",
                configs[best].score_card,
                completed,
                configs.len()
            );
            let heap = host.heap();
            let v = heap.string(card);
            grob::set_property(heap, self.beam, "annotation", v);
        }

        if self.align_broken_intos {
            let normalized_endpoints = {
                let v = grob::property(host, self.beam, "normalized-endpoints")?;
                grob::number_pair(host.heap(), v).unwrap_or(Interval::new(0.0, 1.0))
            };
            let y_length = final_positions[1] - final_positions[0];

            final_positions[0] += normalized_endpoints.left * y_length;
            final_positions[1] -= (1.0 - normalized_endpoints.right) * y_length;
        }

        Ok(final_positions)
    }

    /// `score_stem_lengths`: each normal stem's distance past its shortest
    /// and from its ideal end, scale-free, a knee's convex.
    fn score_stem_lengths(&self, c: &mut Configuration) {
        let limit_penalty = self.parameters.stem_length_limit_penalty;
        let mut score = [0.0f64; 2];
        let mut count = [0i32; 2];

        for i in 0..self.stem_xpositions.len() {
            if !self.is_normal[i] {
                continue;
            }

            let x = self.stem_xpositions[i];
            let dx = self.x_span;
            let beam_y = if dx != 0.0 {
                c.y[1] * x / dx + c.y[0] * (self.x_span - x) / dx
            } else {
                (c.y[1] + c.y[0]) / 2.0
            };
            let current_y = beam_y + self.base_lengths[i];
            let length_pen = self.parameters.stem_length_demerit_factor;

            let info = self.stem_infos[i];
            let d = info.dir;
            let df = f64::from(d);

            score[ix(d)] = fma(
                limit_penalty,
                max(0.0, df * (info.shortest_y - current_y)),
                score[ix(d)],
            );

            let ideal_diff = df * (current_y - info.ideal_y);
            let mut ideal_score = shrink_extra_weight(ideal_diff, 1.5);

            // We introduce a power, to make the scoring strictly convex.
            // Otherwise a symmetric knee beam (up/down/up/down) does not
            // have an optimum in the middle.
            if self.is_knee {
                ideal_score = libm::pow(ideal_score, 1.1);
            }

            score[ix(d)] = fma(length_pen, ideal_score, score[ix(d)]);
            count[ix(d)] += 1;
        }

        // Divide by number of stems, to make the measure scale-free.
        for d in [DOWN, UP] {
            score[ix(d)] /= f64::from(count[ix(d)].max(1));
        }

        // sometimes, two perfectly symmetric kneed beams will have the same
        // score and can either be quanted up or down.
        //
        // we choose the quanting in the direction of the slope so that the
        // first stem always seems longer, reaching to the second, rather
        // than squashed.
        if self.is_knee && count[0] == count[1] && count[0] == 1 {
            let d = beam_dir_of(delta(self.unquanted_y));
            if d != 0 {
                score[ix(d)] += if score[ix(d)] < 1.0 { 0.01 } else { 0.0 };
            }
        }

        c.add(score[0] + score[1], "L");
    }

    /// `score_slope_direction`: against the damped slope's direction.
    fn score_slope_direction(&self, c: &mut Configuration) {
        let dy = delta(c.y);
        let damped_dy = delta(self.unquanted_y);
        let mut dem = 0.0;
        // DAMPING_DIRECTION_PENALTY is a very harsh measure, while for
        // complex beaming patterns, horizontal is often a good choice.
        //
        // TODO: find a way to incorporate the complexity of the beam in
        // this penalty.
        if sign(damped_dy) != sign(dy) {
            if dy == 0.0 {
                if libm::fabs(damped_dy / self.x_span) > self.parameters.round_to_zero_slope {
                    dem += self.parameters.damping_direction_penalty;
                } else {
                    dem += self.parameters.hint_direction_penalty;
                }
            } else {
                dem += self.parameters.damping_direction_penalty;
            }
        }

        c.add(dem, "Sd");
    }

    /// `score_slope_musical`: going against the direction of the musical
    /// pattern.
    fn score_slope_musical(&self, c: &mut Configuration) {
        let dy = delta(c.y);
        let dem = self.parameters.musical_direction_factor
            * (libm::fabs(dy) - libm::fabs(self.musical_dy)).max(0.0);
        c.add(dem, "Sm");
    }

    /// `score_slope_ideal`: the deviation from the damped slope, steeper
    /// cheaper, a cross-staff beam's ten times dearer.
    fn score_slope_ideal(&self, c: &mut Configuration) {
        let dy = delta(c.y);
        let damped_dy = delta(self.unquanted_y);

        let mut slope_penalty = self.parameters.ideal_slope_factor;

        // Xstaff beams tend to use extreme slopes to get short stems. We
        // put in a penalty here.
        if self.is_xstaff {
            slope_penalty *= 10.0;
        }

        // Huh, why would a too steep beam be better than a too flat one ?
        let dem = shrink_extra_weight(libm::fabs(damped_dy) - libm::fabs(dy), 1.5) * slope_penalty;

        c.add(dem, "Si");
    }

    /// `score_horizontal_inter_quants`: a level beam inside the staff
    /// between two lines.
    ///
    /// TODO - there is some overlap with forbidden quants, but for
    /// horizontal beams, it is much more serious to have stafflines
    /// appearing in the wrong place, so we have a separate scorer.
    fn score_horizontal_inter_quants(&self, c: &mut Configuration) {
        if delta(c.y) == 0.0 && libm::fabs(c.y[0]) < self.staff_radius * self.staff_space {
            let yshift = c.y[0] - 0.5 * self.staff_space;
            if libm::fabs(round_halfway_up(yshift) - yshift) < 0.01 * self.staff_space {
                c.add(self.parameters.horizontal_inter_quant_penalty, "H");
            }
        }
    }

    /// `score_forbidden_quants`: the edges' beams whose gaps hold a staff
    /// line (`Fl`), and the secondary beams sitting or hanging badly
    /// (`Fs`).
    ///
    /// TODO: The fixed value SECONDARY_BEAM_DEMERIT is probably flawed:
    /// because for 32nd and 64th beams the forbidden quants are relatively
    /// more important than stem lengths.
    fn score_forbidden_quants(&self, c: &mut Configuration) {
        let dy = delta(c.y);

        let extra_demerit = self.parameters.secondary_beam_demerit
            / f64::from(self.edge_beam_counts[0].max(self.edge_beam_counts[1]));

        let mut dem = 0.0;
        let eps = self.parameters.beam_eps;

        for d in [LEFT, RIGHT] {
            for j in 1..=self.edge_beam_counts[ix(d)] {
                let stem_dir = f64::from(self.edge_dirs[ix(d)]);

                // The fudge_factor is to provide a little leniency for
                // borderline cases. If we do 2.0, then the upper outer line
                // will be in the gap of the (2, sit) quant, leading to a
                // false demerit. By increasing the fudge factor to 2.2, we
                // fix this case.
                let fudge_factor = 2.2;
                let gap1 = fma(
                    -stem_dir,
                    fma(
                        f64::from(j - 1),
                        self.beam_translation,
                        self.beam_thickness / 2.0,
                    ) - self.line_thickness / fudge_factor,
                    c.y[ix(d)],
                );
                let gap2 = fma(
                    -stem_dir,
                    fma(
                        f64::from(j),
                        self.beam_translation,
                        -(self.beam_thickness / 2.0),
                    ) + self.line_thickness / fudge_factor,
                    c.y[ix(d)],
                );

                let mut gap = Interval::EMPTY;
                gap.add_point(gap1);
                gap.add_point(gap2);

                let mut k = -self.staff_radius;
                while k <= self.staff_radius + eps {
                    if contains(gap, k) {
                        let dist = min(libm::fabs(gap.right - k), libm::fabs(gap.left - k));

                        // this parameter is tuned to grace-stem-length.ly
                        // retuned from 0.40 to 0.39 by MS because of slight
                        // increases in gap.length () resulting from
                        // measuring beams at real ends instead of from the
                        // middle of stems.
                        //
                        // TODO: This function needs better comments so we
                        // know what is forbidden and why.
                        let fixed_demerit = 0.39;

                        dem = fma(
                            extra_demerit,
                            fma(
                                (1.0 - fixed_demerit) * (dist / gap.length()),
                                2.0,
                                fixed_demerit,
                            ),
                            dem,
                        );
                    }
                    k += 1.0;
                }
            }
        }

        c.add(dem, "Fl");
        dem = 0.0;
        if self.edge_beam_counts[0].max(self.edge_beam_counts[1]) >= 2 {
            let straddle = 0.0;
            let sit = (self.beam_thickness - self.line_thickness) / 2.0;
            let inter = 0.5;
            let hang = 1.0 - (self.beam_thickness - self.line_thickness) / 2.0;

            for d in [LEFT, RIGHT] {
                let ed = self.edge_dirs[ix(d)];
                let y = c.y[ix(d)];
                if self.edge_beam_counts[ix(d)] >= 2
                    && libm::fabs(fma(-f64::from(ed), self.beam_translation, y))
                        < self.staff_radius + inter
                {
                    // TODO up/down symmetry.
                    if ed == UP && dy <= eps && libm::fabs(my_modf(y) - sit) < eps {
                        dem += extra_demerit;
                    }

                    if ed == DOWN && dy >= eps && libm::fabs(my_modf(y) - hang) < eps {
                        dem += extra_demerit;
                    }
                }

                if self.edge_beam_counts[ix(d)] >= 3
                    && libm::fabs(fma(-f64::from(2 * ed), self.beam_translation, y))
                        < self.staff_radius + inter
                {
                    // TODO up/down symmetry.
                    if ed == UP && dy <= eps && libm::fabs(my_modf(y) - straddle) < eps {
                        dem += extra_demerit;
                    }

                    if ed == DOWN && dy >= eps && libm::fabs(my_modf(y) - straddle) < eps {
                        dem += extra_demerit;
                    }
                }
            }
        }

        c.add(dem, "Fs");
    }

    /// `score_collisions`: each collision within the padding of the beams'
    /// height there, by the cube of how far within.
    fn score_collisions(&self, c: &mut Configuration) {
        let mut demerits = 0.0;
        for col in &self.collisions {
            let collision_y = col.y;
            let x = col.x;

            let center_beam_y = self.y_at(x, c);
            let mut beam_y = col.beam_y;
            beam_y.translate(center_beam_y);

            let mut both = beam_y;
            both.intersect(collision_y);
            let dist = if !both.is_empty() {
                0.0
            } else {
                min(
                    distance(beam_y, collision_y.left),
                    distance(beam_y, collision_y.right),
                )
            };

            let scale_free = max(self.parameters.collision_padding - dist, 0.0)
                / self.parameters.collision_padding;
            let collision_demerit =
                col.base_penalty * libm::pow(scale_free, 3.0) * self.parameters.collision_penalty;

            if collision_demerit > 0.0 {
                demerits += collision_demerit;
            }
        }

        c.add(demerits, "C");
    }
}

/// `Direction (x)` of a real: its sign, 0 for 0, a NaN by its sign bit.
fn beam_dir_of(x: f64) -> Direction {
    if x != 0.0 {
        if x.is_sign_negative() { -1 } else { 1 }
    } else {
        0
    }
}

/// `point_in_interval`: a reasonable point inside `v`, `dist` inside its
/// finite end if the other is infinite, else its center.
fn point_in_interval(v: Interval, dist: f64) -> f64 {
    if v.left.is_infinite() {
        v.right - dist
    } else if v.right.is_infinite() {
        v.left + dist
    } else {
        v.center()
    }
}

/// `set_minimum_dy`: a slope not smaller than the smallest quant, which
/// would give absurd direction-sign penalties.
fn set_minimum_dy(host: &mut dyn Host, me: Value, dy: &mut f64) -> Run<()> {
    if *dy != 0.0 {
        let ss = staff_space(host, me)?;
        let beam_thickness = beam::get_beam_thickness(host, me)? / ss;
        let slt = line_thickness(host, me)? / ss;
        let sit = (beam_thickness - slt) / 2.0;
        let inter = 0.5;
        let hang = 1.0 - (beam_thickness - slt) / 2.0;

        *dy = sign(*dy) * max(libm::fabs(*dy), min(min(sit, inter), hang));
    }
    Ok(())
}

/// `is_concave_single_notes`: notes above and below the first and last,
/// one as close to the beam as they are reached against their slope, or
/// all inner ones closer.
fn is_concave_single_notes(positions: &[i32], beam_dir: Direction) -> bool {
    let (first, last) = (positions[0], positions[positions.len() - 1]);
    let covering = Slice {
        left: first.min(last),
        right: first.max(last),
    };

    let mut above = false;
    let mut below = false;
    let mut concave = false;

    // notes above and below the interval covered by 1st and last note.
    for &p in &positions[1..positions.len() - 1] {
        above = above || p > covering.right;
        below = below || p < covering.left;
    }

    concave = concave || (above && below);
    // A note as close or closer to the beam than begin and end, but the
    // note is reached in the opposite direction as the last-first dy
    let dy = last - first;
    let closest = (beam_dir * last).max(beam_dir * first);
    let mut i = 2;
    while !concave && i + 1 < positions.len() {
        let inner_dy = positions[i] - positions[i - 1];
        if inner_dy.signum() != dy.signum()
            && (beam_dir * positions[i] >= closest || beam_dir * positions[i - 1] >= closest)
        {
            concave = true;
        }
        i += 1;
    }

    let mut all_closer = true;
    let mut i = 1;
    while all_closer && i + 1 < positions.len() {
        all_closer = all_closer && (beam_dir * positions[i] > closest);
        i += 1;
    }

    concave || all_closer
}

/// `calc_positions_concaveness`: the inner positions' mean distance
/// towards the beam past the line from the first to the last, by its
/// height.
fn calc_positions_concaveness(positions: &[i32], beam_dir: Direction) -> f64 {
    let n = positions.len();
    let dy = f64::from(positions[n - 1] - positions[0]);
    let slope = dy / (n - 1) as f64;
    let mut concaveness = 0.0;
    for (i, &p) in positions.iter().enumerate().take(n - 1).skip(1) {
        let line_y = fma(slope, i as f64, f64::from(positions[0]));
        concaveness += max(f64::from(beam_dir) * (f64::from(p) - line_y), 0.0);
    }

    concaveness /= n as f64;

    // Normalize. For dy = 0, the slope ends up as 0 anyway, so the scaling
    // of concaveness doesn't matter much.
    if dy != 0.0 {
        concaveness /= libm::fabs(dy);
    }
    concaveness
}

// --- libc++'s heap ----------------------------------------------------------

/// `std::__sift_up` over the heap's first `len`: the last moved up past
/// each parent `less` than it.
pub(crate) fn sift_up(heap: &mut [usize], len: usize, less: &dyn Fn(usize, usize) -> bool) {
    if len > 1 {
        let mut len = (len - 2) / 2;
        let mut last = heap.len() - 1;
        if less(heap[len], heap[last]) {
            let t = heap[last];
            loop {
                heap[last] = heap[len];
                last = len;
                if len == 0 {
                    break;
                }
                len = (len - 1) / 2;
                if !less(heap[len], t) {
                    break;
                }
            }
            heap[last] = t;
        }
    }
}

/// `std::__pop_heap`: the top moved to the end through a hole sifted down
/// Floyd's way (`std::__floyd_sift_down`), the last put in the hole and
/// sifted up.
pub(crate) fn pop_heap(heap: &mut [usize], less: &dyn Fn(usize, usize) -> bool) {
    let len = heap.len();
    if len > 1 {
        let top = heap[0];
        // `__floyd_sift_down`
        let mut hole = 0;
        let mut child_i = 0;
        let mut child = 0usize;
        loop {
            child_i += child + 1;
            child = 2 * child + 1;
            if child + 1 < len && less(heap[child_i], heap[child_i + 1]) {
                // right-child exists and is greater than left-child
                child_i += 1;
                child += 1;
            }
            heap[hole] = heap[child_i];
            hole = child_i;
            if child > (len - 2) / 2 {
                break;
            }
        }
        let last = len - 1;
        if hole == last {
            heap[hole] = top;
        } else {
            heap[hole] = heap[last];
            hole += 1;
            heap[last] = top;
            sift_up(&mut heap[..hole], hole, less);
        }
    }
}

/// `(ly:beam::quanting beam ys align-broken-intos)`'s work: the beam's
/// two ends from `ys` (the unquanted ones if finite), quanted.
pub fn quanting(host: &mut dyn Host, me: Value, ys: Value, align: Value) -> Run<Value> {
    let ys = grob::number_pair(host.heap(), ys)
        .map_or([f64::INFINITY, f64::NEG_INFINITY], |i| [i.left, i.right]);
    let cbs = align == Value::TRUE;

    let problem = Problem::new(host, me, ys, cbs)?;
    let ys = problem.solve(host)?;

    let heap = host.heap();
    let (l, r) = (heap.flonum(ys[0]), heap.flonum(ys[1]));
    Ok(heap.cons(l, r))
}
