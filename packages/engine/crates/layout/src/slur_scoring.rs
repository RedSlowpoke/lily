//! `slur-scoring.cc` (MS4 step 8): `Slur_score_state`, the search for a
//! slur's curve, which `ly:slur::calc-control-points` (`:388–441`) runs for
//! the Slur's and PhrasingSlur's `control-points`. Its state read of the
//! slur, its bounds (`get_bound_info`, `:182–252`), the note columns it
//! encompasses (`get_encompass_info`, `:117–180`) and the extra objects
//! (`get_extra_encompass_infos`, `:767–876`, the small slurs' curves among
//! them); the base attachments at the bounds (`get_base_attachments`,
//! `:532–626`) moved off the staff lines (`move_away_from_staffline`,
//! `:628–648`), the range of the ends' heights (`get_y_attachment_range`,
//! `:483–516`) widened by the extra objects inside the slur (`fill`,
//! `:254–386`); then each pair of ends half a staff space apart in that
//! range a configuration (`enumerate_attachments`, `:694–765`), its curve
//! fitted over the heads and objects to avoid (`generate_avoid_offsets`,
//! `:650–692`, `generate_curves`), scored lazily, cheapest first, by a
//! priority queue (`get_best_curve`, `:462–481`: libc++'s, as
//! `beam_quanting.rs`' is) until the cheapest is scored whole, or the one
//! nearest `positions` (`get_forced_configuration`, `:440–460`). With
//! `inspect-quants` the nearest to it, and with `debug-slur-scoring` (or
//! `inspect-quants`) the score card is the slur's `annotation`.
//! slur-score-parameters.cc's `details` are [`Parameters`].
//!
//! LilyPond's clang fuses the sums of products here (otool): the stem's
//! half beam, the extra objects' range and `linear_interpolate`, the
//! musical slope, the attachments' half staff spaces and the avoided
//! objects' distances; each is an [`fma`] below.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{Host, Rational, grob_of, host_lookup_variable};
use lily_engine_scheme::{Exception, Value};

use crate::beam;
use crate::beam_quanting::{pop_heap, round_halfway_up, sift_up};
use crate::bezier;
use crate::direction::{get_grob_direction, set_grob_direction};
use crate::geometry::{Box as Bbox, Direction, Interval, Offset, X, Y, fma, max, min};
use crate::grob;
use crate::grob_array;
use crate::item;
use crate::note_column;
use crate::paper_column;
use crate::paper_column_engraver::rational;
use crate::slur;
use crate::slur_configuration::Configuration;
use crate::spacing_options::double_or;
use crate::spanner;
use crate::staff_symbol;
use crate::staff_symbol_referencer as ssr;
use crate::stem;

type Run<T> = Result<T, Exception>;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const UP: Direction = 1;

/// A `Drul_array`'s index of `d`: 0 left or down, 1 right or up.
pub(crate) fn ix(d: Direction) -> usize {
    usize::from(d > 0)
}

/// `minmax (d, a, b)` (misc.hh): `std::max` up, `std::min` otherwise.
pub(crate) fn minmax(d: Direction, a: f64, b: f64) -> f64 {
    if d == UP { max(a, b) } else { min(a, b) }
}

/// `linear_interpolate` (misc.hh): `x` from `x1`–`x2` onto `y1`–`y2`, the
/// first product fused into the sum as clang fuses it.
pub(crate) fn linear_interpolate(x: f64, x1: f64, x2: f64, y1: f64, y2: f64) -> f64 {
    fma((x2 - x) / (x2 - x1), y1, (x - x1) / (x2 - x1) * y2)
}

/// `Interval::contains`.
pub(crate) fn contains(iv: Interval, x: f64) -> bool {
    iv.left <= x && x <= iv.right
}

/// The grob's property `name` as a real, else `fallback`.
fn real_or(host: &mut dyn Host, me: Value, name: &str, fallback: f64) -> Run<f64> {
    let v = grob::property(host, me, name)?;
    Ok(double_or(host, v, fallback))
}

/// `Staff_symbol_referencer::on_staff_line`: whether `pos` is on a line of
/// `me`'s `staff-symbol`; false without one (a staff symbol has none).
pub(crate) fn on_staff_line(host: &mut dyn Host, me: Value, pos: i32) -> Run<bool> {
    match ssr::staff_symbol(host, me)? {
        Some(st) => staff_symbol::on_line(host, st, pos, false),
        None => Ok(false),
    }
}

/// `Slur_score_parameters`: the slur's `details`, each 0 if it is not
/// there or no real.
#[derive(Clone, Copy, Default)]
pub(crate) struct Parameters {
    pub region_size: i32,
    pub head_encompass_penalty: f64,
    pub stem_encompass_penalty: f64,
    pub edge_attraction_factor: f64,
    pub same_slope_penalty: f64,
    pub steeper_slope_factor: f64,
    pub non_horizontal_penalty: f64,
    pub max_slope: f64,
    pub max_slope_factor: f64,
    pub extra_object_collision_penalty: f64,
    pub accidental_collision: f64,
    pub free_slur_distance: f64,
    pub free_head_distance: f64,
    /// Read, as LilyPond's, by no scorer.
    #[allow(dead_code)]
    pub extra_encompass_collision_distance: f64,
    pub extra_encompass_free_distance: f64,
    pub gap_to_staffline_inside: f64,
    pub gap_to_staffline_outside: f64,
    pub absolute_closeness_measure: f64,
    pub edge_slope_exponent: f64,
    pub close_to_edge_length: f64,
    pub head_slur_distance_max_ratio: f64,
    pub head_slur_distance_factor: f64,
    pub encompass_object_range_overshoot: f64,
    pub slur_tie_extrema_min_distance: f64,
    pub slur_tie_extrema_min_distance_penalty: f64,
}

impl Parameters {
    /// `fill`, in slur-score-parameters.cc's order.
    fn fill(host: &mut dyn Host, me: Value) -> Run<Parameters> {
        let details = grob::property(host, me, "details")?;
        let mut get = |name: &str| {
            let sym = host.heap().intern(name);
            let v = grob::assq_ref(host.heap(), sym, details).unwrap_or(Value::NULL);
            double_or(host, v, 0.0)
        };
        Ok(Parameters {
            region_size: get("region-size") as i32,
            head_encompass_penalty: get("head-encompass-penalty"),
            stem_encompass_penalty: get("stem-encompass-penalty"),
            edge_attraction_factor: get("edge-attraction-factor"),
            same_slope_penalty: get("same-slope-penalty"),
            steeper_slope_factor: get("steeper-slope-factor"),
            non_horizontal_penalty: get("non-horizontal-penalty"),
            max_slope: get("max-slope"),
            max_slope_factor: get("max-slope-factor"),
            free_head_distance: get("free-head-distance"),
            gap_to_staffline_inside: get("gap-to-staffline-inside"),
            gap_to_staffline_outside: get("gap-to-staffline-outside"),
            absolute_closeness_measure: get("absolute-closeness-measure"),
            extra_object_collision_penalty: get("extra-object-collision-penalty"),
            accidental_collision: get("accidental-collision"),
            extra_encompass_free_distance: get("extra-encompass-free-distance"),
            extra_encompass_collision_distance: get("extra-encompass-collision-distance"),
            head_slur_distance_factor: get("head-slur-distance-factor"),
            head_slur_distance_max_ratio: get("head-slur-distance-max-ratio"),
            free_slur_distance: get("free-slur-distance"),
            edge_slope_exponent: get("edge-slope-exponent"),
            close_to_edge_length: get("close-to-edge-length"),
            encompass_object_range_overshoot: get("encompass-object-range-overshoot"),
            slur_tie_extrema_min_distance: get("slur-tie-extrema-min-distance"),
            slur_tie_extrema_min_distance_penalty: get("slur-tie-extrema-min-distance-penalty"),
        })
    }
}

/// `Encompass_info`: an encompassed column's x and its head's and stem's
/// ends on the slur's side.
#[derive(Clone, Copy, Default)]
pub(crate) struct EncompassInfo {
    pub x: f64,
    pub stem: f64,
    pub head: f64,
}

impl EncompassInfo {
    /// `get_point`: of the stem's and the head's ends, the one towards
    /// `dir`.
    pub fn get_point(&self, dir: Direction) -> f64 {
        let mut y = Interval::EMPTY;
        y.add_point(self.stem);
        y.add_point(self.head);
        y.at(dir)
    }
}

/// `Extra_collision_info`: an extra object the slur should avoid, its
/// place in its X extent (-1 to 1) where the slur is measured, its box,
/// its penalty and its `avoid-slur`.
#[derive(Clone, Copy)]
pub(crate) struct ExtraCollision {
    pub idx: f64,
    pub extents: Bbox,
    pub penalty: f64,
    pub grob: Value,
    pub type_: Value,
}

impl ExtraCollision {
    fn new(host: &mut dyn Host, g: Value, idx: f64, x: Interval, y: Interval, p: f64) -> Run<Self> {
        Ok(ExtraCollision {
            idx,
            extents: Bbox::new(x, y),
            penalty: p,
            grob: g,
            type_: grob::property(host, g, "avoid-slur")?,
        })
    }
}

/// `Bound_info`: what is at an end of the slur.
#[derive(Clone, Copy)]
pub(crate) struct BoundInfo {
    pub stem_extent: Bbox,
    pub stem_dir: Direction,
    pub bound: Option<Value>,
    pub note_column: Option<Value>,
    pub slur_head: Option<Value>,
    pub staff: Option<Value>,
    pub stem: Option<Value>,
    pub flag: Option<Value>,
    pub slur_head_x_extent: Interval,
}

impl Default for BoundInfo {
    fn default() -> Self {
        BoundInfo {
            stem_extent: Bbox::EMPTY,
            stem_dir: 0,
            bound: None,
            note_column: None,
            slur_head: None,
            staff: None,
            stem: None,
            flag: None,
            slur_head_x_extent: Interval::EMPTY,
        }
    }
}

/// `Slur_score_state`: a slur's formatting problem; the configurations are
/// kept apart, scored against it.
pub(crate) struct State {
    pub slur: Value,
    pub common: [Value; 2],
    pub edge_has_beams: bool,
    pub is_broken: bool,
    pub has_same_beam: bool,
    pub musical_dy: f64,
    pub note_columns: Vec<Value>,
    pub encompass_infos: Vec<EncompassInfo>,
    pub extra_encompass_infos: Vec<ExtraCollision>,
    pub dir: Direction,
    pub parameters: Parameters,
    pub extremes: [BoundInfo; 2],
    pub base_attachments: [Offset; 2],
    pub staff_space: f64,
    pub line_thickness: f64,
    pub thickness: f64,
}

impl State {
    fn new(slur: Value) -> State {
        State {
            slur,
            common: [Value::NULL; 2],
            edge_has_beams: false,
            is_broken: false,
            has_same_beam: false,
            musical_dy: 0.0,
            note_columns: Vec::new(),
            encompass_infos: Vec::new(),
            extra_encompass_infos: Vec::new(),
            dir: 0,
            parameters: Parameters::default(),
            extremes: [BoundInfo::default(); 2],
            base_attachments: [(0.0, 0.0); 2],
            staff_space: 0.0,
            line_thickness: 0.0,
            thickness: 0.0,
        }
    }

    fn extent(&self, host: &mut dyn Host, g: Value, a: usize) -> Run<Interval> {
        grob::extent(host, g, self.common[a], a)
    }

    fn coordinate(&self, host: &mut dyn Host, g: Value, a: usize) -> Run<f64> {
        grob::relative_coordinate(host, g, self.common[a], a)
    }

    /// `slur_direction`: a broken slur's after a line break is its left
    /// neighbour's, if that is live; else the slur's, given to its right
    /// neighbour.
    fn slur_direction(&self, host: &mut dyn Host) -> Run<Direction> {
        let left_neighbor = spanner::broken_neighbor(host.heap(), self.slur, LEFT);

        if let Some(l) = left_neighbor
            && grob::is_live_grob(host.heap(), l)
        {
            return get_grob_direction(host, l);
        }

        let dir = get_grob_direction(host, self.slur)?;

        if let Some(r) = spanner::broken_neighbor(host.heap(), self.slur, RIGHT) {
            set_grob_direction(host, r, dir);
        }

        Ok(dir)
    }

    /// `get_encompass_info`: the column's head or stem centre and the
    /// ends towards the slur of its extremal head and, if it points there
    /// and is visible, its stem (half its beam's thickness out); the
    /// column's own without a stem or a head.
    pub fn get_encompass_info(&self, host: &mut dyn Host, notecol: Value) -> Run<EncompassInfo> {
        let sym = host.heap().intern("stem");
        let stem = grob::object(host, notecol, sym)?;
        let stem = grob_of(host.heap(), stem).map(|_| stem);
        let mut ei = EncompassInfo::default();

        let Some(stem) = stem else {
            ei.x = self.coordinate(host, notecol, X)?;
            ei.head = self.extent(host, notecol, Y)?.at(self.dir);
            ei.stem = ei.head;
            return Ok(ei);
        };
        let stem_dir = get_grob_direction(host, stem)?;

        if let Some(head) = note_column::first_head(host, notecol)? {
            let head_ext = self.extent(host, head, X)?;
            // FIXME: Is there a better option than setting to 0?
            ei.x = if head_ext.is_empty() {
                0.0
            } else {
                head_ext.center()
            };
        } else {
            ei.x = self.extent(host, notecol, X)?.center();
        }

        let Some(h) = stem::extremal_heads(host, stem)?[ix(self.dir)] else {
            ei.head = self.extent(host, notecol, Y)?.at(self.dir);
            ei.stem = ei.head;
            return Ok(ei);
        };

        ei.head = self.extent(host, h, Y)?.at(self.dir);

        if stem_dir == self.dir && !grob::extent(host, stem, stem, Y)?.is_empty() {
            ei.stem = self.extent(host, stem, Y)?.at(self.dir);
            if let Some(b) = stem::get_beam(host, stem)? {
                let thick = beam::get_beam_thickness(host, b)?;
                ei.stem = fma(f64::from(stem_dir) * 0.5, thick, ei.stem);
            }

            let x = self.extent(host, stem, X)?;
            ei.x = if x.is_empty() {
                self.coordinate(host, stem, X)?
            } else {
                x.center()
            };
        } else {
            ei.stem = ei.head;
        }

        Ok(ei)
    }

    /// `get_bound_info`: at each end, its note column with its stem, flag
    /// and their box, and the head on the slur's side (the rest without
    /// one), or the head that is the bound; the head's X extent and staff.
    fn get_bound_info(&self, host: &mut dyn Host) -> Run<[BoundInfo; 2]> {
        let mut extremes = [BoundInfo::default(); 2];
        let slur_dir = self.dir;

        for bound_dir in [LEFT, RIGHT] {
            let info = &mut extremes[ix(bound_dir)];
            let bound = spanner::get_bound(host, self.slur, bound_dir)?;
            info.bound = bound;
            let is = |host: &mut dyn Host, name| {
                bound.is_some_and(|b| grob::has_interface(host.heap(), b, name))
            };
            if let Some(note_col) = bound.filter(|_| is(host, "note-column-interface")) {
                info.note_column = Some(note_col);
                let stem = note_column::get_stem(host, note_col)?;
                info.stem = stem;
                let flag = note_column::get_flag(host, note_col)?;
                info.flag = flag;

                if let Some(stem) = stem {
                    info.stem_dir = get_grob_direction(host, stem)?;

                    for ax in [X, Y] {
                        let mut s = grob::extent(host, stem, self.common[ax], ax)?;
                        if let Some(flag) = flag {
                            s.unite(grob::extent(host, flag, self.common[ax], ax)?);
                        }
                        if s.is_empty() {
                            /*
                              do not issue warning. This happens for rests and
                              whole notes.
                            */
                            let c = grob::relative_coordinate(host, stem, self.common[ax], ax)?;
                            s = Interval::new(0.0 + c, 0.0 + c);
                        }
                        info.stem_extent.0[ax] = s;
                    }

                    info.slur_head = stem::extremal_heads(host, stem)?[ix(slur_dir)];
                } else {
                    info.slur_head = note_column::extremal_heads(host, note_col)?[ix(slur_dir)];
                }

                if info.slur_head.is_none() {
                    info.slur_head = note_column::get_rest(host, note_col)?;
                }
            } else if is(host, "note-head-interface") {
                info.slur_head = bound;
            }

            if let Some(head) = info.slur_head {
                info.slur_head_x_extent = grob::extent(host, head, self.common[X], X)?;
                info.staff = ssr::staff_symbol(host, head)?;
            }
        }

        Ok(extremes)
    }

    /// `fill`: the state, then the configurations; none for a slur over no
    /// column, which is killed.
    fn fill(&mut self, host: &mut dyn Host) -> Run<Option<Vec<Configuration>>> {
        let me = self.slur;
        self.note_columns = grob_array::extract_grob_array(host, me, "note-columns")?;

        if self.note_columns.is_empty() {
            grob::suicide(host.heap(), me);
            return Ok(None);
        }

        slur::replace_breakable_encompass_objects(host, me)?;
        self.staff_space = ssr::staff_space(host, me)?;
        self.line_thickness = grob::layout_dimension(host, me, "line-thickness")?;
        self.thickness = real_or(host, me, "thickness", 1.0)? * self.line_thickness;

        self.dir = self.slur_direction(host)?;
        self.parameters = Parameters::fill(host, me)?;

        let columns = grob_array::extract_grob_array(host, me, "note-columns")?;
        let extra_objects = grob_array::extract_grob_array(host, me, "encompass-objects")?;

        for a in [X, Y] {
            let heap = host.heap();
            let mut common = grob::common_refpoint_of_array(heap, &columns, Some(me), a);
            common = grob::common_refpoint_of_array(heap, &extra_objects, common, a);

            for d in [LEFT, RIGHT] {
                /*
                  If bound is not in note-columns, we don't want to know about
                  its Y-position
                */
                if a != Y {
                    let bound = spanner::get_bound(host, me, d)?;
                    common = match (common, bound) {
                        (Some(c), Some(b)) => grob::common_refpoint(host.heap(), c, b, a),
                        _ => None,
                    };
                }
            }
            self.common[a] = common.unwrap_or(Value::NULL);
        }

        self.extremes = self.get_bound_info(host)?;
        let [l, r] = self.extremes;
        self.is_broken = !(l.note_column.is_some() || l.slur_head.is_some())
            || !(r.note_column.is_some() || r.slur_head.is_some());

        self.has_same_beam = match (l.stem, r.stem) {
            (Some(ls), Some(rs)) => stem::get_beam(host, ls)? == stem::get_beam(host, rs)?,
            _ => false,
        };

        self.base_attachments = self.get_base_attachments(host)?;

        let mut end_ys = self.get_y_attachment_range(host)?;

        self.extra_encompass_infos = self.get_extra_encompass_infos(host)?;

        let mut additional_ys = [0.0f64; 2];
        let dir = self.dir;
        let base = self.base_attachments;

        for info in self.extra_encompass_infos.clone() {
            if info.extents.x().is_empty() {
                continue;
            }

            let center = info.extents.x().center();
            let y_place = linear_interpolate(
                center,
                base[ix(RIGHT)].0,
                base[ix(LEFT)].0,
                end_ys[ix(RIGHT)],
                end_ys[ix(LEFT)],
            );
            let encompass_place = info.extents.y().at(dir);
            let heap = host.heap();
            let avoid_inside = info.type_ == heap.intern("inside");
            if avoid_inside
                && minmax(dir, encompass_place, y_place) == encompass_place
                && (!grob::has_interface(heap, info.grob, "key-signature-interface")
                    && !grob::has_interface(heap, info.grob, "clef-interface")
                    && !grob::has_interface(heap, info.grob, "time-signature-interface"))
            {
                // `normalize (x, x1, x2)` (misc.hh)
                let normalized =
                    (center - base[ix(RIGHT)].0) / (base[ix(LEFT)].0 - base[ix(RIGHT)].0);
                let shift = if dir == LEFT { 0.0 } else { -1.0 };
                let dy = fma(
                    y_place - encompass_place,
                    normalized + shift,
                    self.parameters.encompass_object_range_overshoot,
                ) * f64::from(dir);
                for y in &mut additional_ys {
                    *y = minmax(dir, *y, dy);
                }
            }
        }

        for (y, add) in end_ys.iter_mut().zip(additional_ys) {
            *y += add;
        }

        let configurations = self.enumerate_attachments(host, end_ys)?;
        for col in self.note_columns.clone() {
            let info = self.get_encompass_info(host, col)?;
            self.encompass_infos.push(info);
        }

        self.musical_dy = 0.0;
        for d in [LEFT, RIGHT] {
            if !self.is_broken
                && let Some(head) = self.extremes[ix(d)].slur_head
            {
                let y = self.coordinate(host, head, Y)?;
                self.musical_dy = fma(f64::from(d), y, self.musical_dy);
            }
        }

        let beamed = |host: &mut dyn Host, s: Option<Value>| -> Run<bool> {
            Ok(match s {
                Some(s) => stem::get_beam(host, s)?.is_some(),
                None => false,
            })
        };
        self.edge_has_beams = beamed(host, l.stem)? || beamed(host, r.stem)?;

        if self.is_broken {
            self.musical_dy = 0.0;
        }

        Ok(Some(configurations))
    }

    /// `get_forced_configuration`: the configuration whose ends are
    /// nearest `ys`, scored whole.
    fn get_forced_configuration(
        &self,
        host: &mut dyn Host,
        configs: &mut [Configuration],
        ys: Interval,
    ) -> Run<usize> {
        let mut best = 0;
        let mut mindist = 1e6;
        for (i, c) in configs.iter().enumerate() {
            let d = libm::fabs(c.attachment[ix(LEFT)].1 - ys.left)
                + libm::fabs(c.attachment[ix(RIGHT)].1 - ys.right);
            if d < mindist {
                best = i;
                mindist = d;
            }
        }

        while !configs[best].done() {
            configs[best].run_next_scorer(host, self)?;
        }

        if mindist > 1e5 {
            grob::unlocated_programming_error(host, "cannot find quant")?;
        }

        Ok(best)
    }

    /// `get_best_curve`: the cheapest configuration, scoring the cheapest
    /// so far one scorer further until it is scored whole.
    fn get_best_curve(&self, host: &mut dyn Host, configs: &mut [Configuration]) -> Run<usize> {
        // libc++'s priority queue of the configurations, cheapest on top.
        let less =
            |configs: &[Configuration], l: usize, r: usize| configs[l].score() > configs[r].score();
        let mut queue: Vec<usize> = Vec::with_capacity(configs.len());
        for i in 0..configs.len() {
            queue.push(i);
            let n = queue.len();
            sift_up(&mut queue, n, &|l, r| less(configs, l, r));
        }

        loop {
            let best = queue[0];
            if configs[best].done() {
                return Ok(best);
            }

            pop_heap(&mut queue, &|l, r| less(configs, l, r));
            queue.pop();
            configs[best].run_next_scorer(host, self)?;
            queue.push(best);
            let n = queue.len();
            sift_up(&mut queue, n, &|l, r| less(configs, l, r));
        }
    }

    /// `breakable_bound_extent`: the X extent of the extra objects in the
    /// column of the bound `d`.
    fn breakable_bound_extent(&self, host: &mut dyn Host, d: Direction) -> Run<Interval> {
        let bound = spanner::get_bound(host, self.slur, d)?;
        let paper_col = bound.and_then(|b| item::get_column(host.heap(), b));
        let mut ret = Interval::EMPTY;

        for g in grob_array::extract_grob_array(host, self.slur, "encompass-objects")? {
            let heap = host.heap();
            if item::is_item(heap, g) && paper_col == item::get_column(heap, g) {
                ret.unite(grob::robust_relative_extent(host, g, self.common[X], X)?);
            }
        }

        Ok(ret)
    }

    /*
      TODO: should analyse encompasses to determine sensible region, and
      should limit slopes available.
    */

    /// `get_y_attachment_range`: how far each end may move from its base:
    /// `region-size` out, at least to the column's edge and the other
    /// end's base; a little at a head that is the bound.
    fn get_y_attachment_range(&self, host: &mut dyn Host) -> Run<[f64; 2]> {
        let mut end_ys = [0.0f64; 2];
        let dir = self.dir;
        let df = f64::from(dir);
        let region = f64::from(self.parameters.region_size * dir);
        for d in [LEFT, RIGHT] {
            let base = self.base_attachments;
            if let Some(nc) = self.extremes[ix(d)].note_column {
                let nc_extent = self.extent(host, nc, Y)?;
                if nc_extent.is_empty() {
                    grob::warning(
                        host,
                        self.slur,
                        "slur trying to encompass an empty note column.",
                    )?;
                } else {
                    end_ys[ix(d)] = df
                        * max(
                            max(df * (base[ix(d)].1 + region), df * (df + nc_extent.at(dir))),
                            df * base[ix(-d)].1,
                        );
                }
            } else if self.extremes[ix(d)].slur_head.is_some() {
                // allow only minimal movement
                end_ys[ix(d)] = base[ix(d)].1 + 0.3 * df;
            } else {
                end_ys[ix(d)] = base[ix(d)].1 + region;
            }
        }

        Ok(end_ys)
    }

    /// `get_base_attachments`: at each end, half a staff space out from
    /// its stem (beamed towards the slur) or its head, off the staff lines,
    /// over its first head; at a head that is the bound, its middle on the
    /// slur's side; at a broken end, by the bound's extra objects or edge,
    /// level with the nearest column.
    fn get_base_attachments(&self, host: &mut dyn Host) -> Run<[Offset; 2]> {
        let mut base_attachment = [(0.0, 0.0); 2];
        let dir = self.dir;
        let df = f64::from(dir);
        for d in [LEFT, RIGHT] {
            let ex = self.extremes[ix(d)];
            let stem = ex.stem;
            let head = ex.slur_head;

            let mut x = 0.0;
            let mut y = 0.0;
            if let Some(nc) = ex.note_column {
                /*
                  fixme: X coord should also be set in this case.
                */
                let mut stem_y = false;
                if let Some(stem) = stem
                    && !stem::is_invisible(host, stem)?
                    && ex.stem_dir == dir
                    && stem::get_beaming(host, stem, -d)? != 0
                    && let Some(beam) = stem::get_beam(host, stem)?
                {
                    stem_y = !spanner_less(host, self.slur, beam)? || self.has_same_beam;
                }
                if stem_y {
                    y = ex.stem_extent.y().at(dir);
                } else if let Some(head) = head {
                    y = self.extent(host, head, Y)?.at(dir);
                }
                y = fma(df * 0.5, self.staff_space, y);

                y = self.move_away_from_staffline(host, y, head)?;

                let fh = note_column::first_head(host, nc)?;
                x = match fh {
                    Some(fh) => self.extent(host, fh, X)?,
                    None => match ex.bound {
                        Some(b) => self.extent(host, b, X)?,
                        None => Interval::EMPTY,
                    },
                }
                .center();
                if !x.is_finite() {
                    x = self.extent(host, nc, X)?.center();
                }
                if !y.is_finite() {
                    y = self.extent(host, nc, Y)?.center();
                }
            } else if let Some(head) = head {
                y = self.extent(host, head, Y)?.linear_combination(0.5 * df);

                // Don't "move_away_from_staffline" because that makes it
                // harder to recognize the specific attachment point
                x = self.extent(host, head, X)?.at(-d);
            }

            base_attachment[ix(d)] = (x, y);
        }

        for d in [LEFT, RIGHT] {
            let ex = self.extremes[ix(d)];
            if ex.note_column.is_none() && ex.slur_head.is_none() {
                let mut ext = self.breakable_bound_extent(host, d)?;
                if ext.is_empty() {
                    ext = match ex.bound {
                        Some(b) => crate::hairpin::generic_bound_extent(host, b, self.common[X])?,
                        None => Interval::EMPTY,
                    };
                }
                let x = ext.at(-d);

                let col = if d == LEFT {
                    self.note_columns[0]
                } else {
                    self.note_columns[self.note_columns.len() - 1]
                };

                let mut y;
                if self.extremes[ix(-d)].bound != Some(col) {
                    y = grob::robust_relative_extent(host, col, self.common[Y], Y)?.at(dir);
                    y = fma(df * 0.5, self.staff_space, y);

                    // FIXME: dead code? NoteColumn doesn't have a direction defined.
                    if get_grob_direction(host, col)? == dir
                        && let Some(s) = note_column::get_stem(host, col)?
                        && !stem::is_invisible(host, s)?
                    {
                        y = fma(df * -1.5, self.staff_space, y);
                    }
                } else {
                    y = base_attachment[ix(-d)].1;
                }

                y = self.move_away_from_staffline(host, y, Some(col))?;

                base_attachment[ix(d)] = (x, y);
            }
        }

        for b in &mut base_attachment {
            for c in [&mut b.0, &mut b.1] {
                if !c.is_finite() {
                    grob::unlocated_programming_error(host, "slur attachment is inf/nan")?;
                    *c = 0.0;
                }
            }
        }

        Ok(base_attachment)
    }

    /// `move_away_from_staffline`: `y` moved 0.15 staff spaces out if it
    /// is within a fifth of a staff space of a line of `on_staff`'s staff.
    fn move_away_from_staffline(
        &self,
        host: &mut dyn Host,
        y: f64,
        on_staff: Option<Value>,
    ) -> Run<f64> {
        let Some(on_staff) = on_staff else {
            return Ok(y);
        };

        let Some(staff_symbol) = ssr::staff_symbol(host, on_staff)? else {
            return Ok(y);
        };

        let pos = (y - self.coordinate(host, staff_symbol, Y)?) * 2.0 / self.staff_space;

        if libm::fabs(pos - round_halfway_up(pos)) < 0.2
            && on_staff_line(host, on_staff, libm::rint(pos) as i32)?
        {
            return Ok(y + 1.5 * self.staff_space * f64::from(self.dir) / 10.0);
        }

        Ok(y)
    }

    /// `generate_avoid_offsets`: the points the curve should pass outside:
    /// each inner column's head or stem `free-head-distance` out, each small
    /// slur's middle `free-slur-distance` out, each extra object inside's
    /// edge.
    fn generate_avoid_offsets(&self, host: &mut dyn Host) -> Run<Vec<Offset>> {
        let mut avoid = Vec::new();
        let dir = self.dir;
        let df = f64::from(dir);

        for &col in &self.note_columns {
            if self.extremes[ix(LEFT)].note_column == Some(col)
                || self.extremes[ix(RIGHT)].note_column == Some(col)
            {
                continue;
            }

            let inf = self.get_encompass_info(host, col)?;
            let y = df * max(df * inf.head, df * inf.stem);

            avoid.push((inf.x, fma(df, self.parameters.free_head_distance, y)));
        }

        for g in grob_array::extract_grob_array(host, self.slur, "encompass-objects")? {
            if grob::has_interface(host.heap(), g, "slur-interface") {
                let b = slur::get_curve(host, g)?;

                let mut z = bezier::curve_point(&b, 0.5);
                z.0 += self.coordinate(host, g, X)?;
                z.1 += self.coordinate(host, g, Y)?;

                z.1 = fma(df, self.parameters.free_slur_distance, z.1);
                avoid.push(z);
            } else if grob::property(host, g, "avoid-slur")? == host.heap().intern("inside") {
                let xe = self.extent(host, g, X)?;
                let ye = self.extent(host, g, Y)?;

                if !xe.is_empty() && !ye.is_empty() {
                    avoid.push((xe.center(), ye.at(dir)));
                }
            }
        }
        Ok(avoid)
    }

    /// `generate_curves`: each configuration's curve, by `ratio` and
    /// `height-limit`.
    fn generate_curves(&self, host: &mut dyn Host, configs: &mut [Configuration]) -> Run<()> {
        let r_0 = real_or(host, self.slur, "ratio", 0.33)?;
        let limit = grob::property(host, self.slur, "height-limit")?;
        let h_inf = self.staff_space * grob::to_double(host.heap(), limit)?;

        let avoid = self.generate_avoid_offsets(host)?;
        for c in configs {
            c.generate_curve(host, self, r_0, h_inf, &avoid)?;
        }
        Ok(())
    }

    /// `enumerate_attachments`: each pair of ends from the bases to
    /// `end_ys` half a staff space apart, at the stem if the end is beside
    /// one that points towards the slur, else over the head, moved in
    /// where the slur tilts.
    fn enumerate_attachments(
        &self,
        host: &mut dyn Host,
        end_ys: [f64; 2],
    ) -> Run<Vec<Configuration>> {
        let mut scores = Vec::new();
        let dir = self.dir;
        let df = f64::from(dir);
        let ss = self.staff_space;

        let mut os = [(0.0, 0.0); 2];
        os[ix(LEFT)] = self.base_attachments[ix(LEFT)];
        let minimum_length = ss * real_or(host, self.slur, "minimum-length", 2.0)?;

        while df * os[ix(LEFT)].1 <= df * end_ys[ix(LEFT)] {
            os[ix(RIGHT)] = self.base_attachments[ix(RIGHT)];

            while df * os[ix(RIGHT)].1 <= df * end_ys[ix(RIGHT)] {
                let mut attach_to_stem = [false; 2];
                for d in [LEFT, RIGHT] {
                    let ex = self.extremes[ix(d)];
                    os[ix(d)].0 = self.base_attachments[ix(d)].0;
                    if let Some(stem) = ex.stem
                        && !stem::is_invisible(host, stem)?
                        && ex.stem_dir == dir
                    {
                        let mut stem_y = ex.stem_extent.y();
                        stem_y.widen(0.25 * ss);
                        if contains(stem_y, os[ix(d)].1) {
                            os[ix(d)].0 = ex.stem_extent.x().at(-d) - f64::from(d) * 0.3;
                            attach_to_stem[ix(d)] = true;
                        } else if df * ex.stem_extent.y().at(dir) < df * os[ix(d)].1
                            && !ex.stem_extent.x().is_empty()
                        {
                            os[ix(d)].0 = ex.stem_extent.x().center();
                        }
                    }
                }

                let dz = (os[1].0 - os[0].0, os[1].1 - os[0].1);
                if dz.0 < minimum_length || libm::fabs(dz.1 / dz.0) > self.parameters.max_slope {
                    for d in [LEFT, RIGHT] {
                        let ex = self.extremes[ix(d)];
                        if ex.slur_head.is_some() && !ex.slur_head_x_extent.is_empty() {
                            os[ix(d)].0 = ex.slur_head_x_extent.center();
                            attach_to_stem[ix(d)] = false;
                        }
                    }
                }

                let dz = crate::geometry::direction((os[1].0 - os[0].0, os[1].1 - os[0].1));
                for d in [LEFT, RIGHT] {
                    let ex = self.extremes[ix(d)];
                    if ex.slur_head.is_some() && !attach_to_stem[ix(d)] {
                        /* Horizontally move tilted slurs a little.  Move
                        more for bigger tilts.

                        TODO: parameter */
                        os[ix(d)].0 -= df * ex.slur_head_x_extent.length() * dz.1 / 3.0;
                    }
                }

                scores.push(Configuration::new_config(os, scores.len()));

                os[ix(RIGHT)].1 += df * ss / 2.0;
            }

            os[ix(LEFT)].1 += df * ss / 2.0;
        }

        Ok(scores)
    }

    /// `get_extra_encompass_infos`: the extra objects, last first: a small
    /// slur's middle and the ends it shares with this one, each a box
    /// twice the slur's thickness wide that reaches out from the curve; any
    /// other's extents (a dot's taller), an accidental's measured on its
    /// side by its alteration.
    fn get_extra_encompass_infos(&self, host: &mut dyn Host) -> Run<Vec<ExtraCollision>> {
        let encompasses = grob_array::extract_grob_array(host, self.slur, "encompass-objects")?;
        let mut collision_infos = Vec::new();
        let dir = self.dir;
        let df = f64::from(dir);
        let p = self.parameters;
        for &g in encompasses.iter().rev() {
            if grob::has_interface(host.heap(), g, "slur-interface") {
                let small_slur = g;
                let b = slur::get_curve(host, small_slur)?;

                let relative = (
                    self.coordinate(host, small_slur, X)?,
                    self.coordinate(host, small_slur, Y)?,
                );

                for k in 0..3 {
                    let hdir = k - 1;

                    /*
                      Only take bound into account if small slur starts
                      together with big slur.
                    */
                    if hdir != 0
                        && spanner::get_bound(host, small_slur, hdir)?
                            != spanner::get_bound(host, self.slur, hdir)?
                    {
                        continue;
                    }

                    let mut z = bezier::curve_point(&b, f64::from(k) / 2.0);
                    z = (z.0 + relative.0, z.1 + relative.1);

                    let mut yext = Interval::new(f64::NEG_INFINITY, f64::INFINITY);
                    *yext.at_mut(dir) = z.1 + df * self.thickness * 1.0;

                    let s = self.thickness * 2.0;
                    let xext = Interval::new(-s + z.0, s + z.0);
                    let info = ExtraCollision::new(
                        host,
                        small_slur,
                        f64::from(hdir),
                        xext,
                        yext,
                        p.extra_object_collision_penalty,
                    )?;
                    collision_infos.push(info);
                }
            } else {
                let mut xe = self.extent(host, g, X)?;
                let mut ye = self.extent(host, g, Y)?;
                if grob::has_interface(host.heap(), g, "dots-interface") {
                    ye.widen(0.2);
                }

                let mut xp = 0.0;
                let mut penalty = p.extra_object_collision_penalty;
                if grob::has_interface(host.heap(), g, "accidental-interface") {
                    penalty = p.accidental_collision;

                    let v = grob::property(host, g, "alteration")?;
                    let alt = rational(host.heap(), v, Rational::ZERO);
                    let scm_style = grob::property(host, g, "style")?;
                    if host.heap().name(scm_style).is_none()
                        && grob::property(host, g, "parenthesized")? != Value::TRUE
                        && grob::property(host, g, "restore-first")? != Value::TRUE
                    {
                        if alt == Rational::new(-1, 2) || alt == Rational::int(-1) {
                            xp = f64::from(LEFT);
                        } else if alt == Rational::new(1, 2) {
                            xp = 0.5 * df;
                        } else if alt == Rational::ZERO {
                            xp = -df;
                        }
                    }
                }

                ye.widen(self.thickness * 0.5);
                xe.widen(self.thickness * 1.0);
                let info = ExtraCollision::new(host, g, xp, xe, ye, penalty)?;
                collision_infos.push(info);
            }
        }

        Ok(collision_infos)
    }
}

/// `spanner_less`: whether `s2` spans `s1`'s columns and more.
fn spanner_less(host: &mut dyn Host, s1: Value, s2: Value) -> Run<bool> {
    let mut b1 = [0u64; 2];
    let mut b2 = [0u64; 2];
    for d in [LEFT, RIGHT] {
        for (b, s) in [(&mut b1, s1), (&mut b2, s2)] {
            let bound = spanner::get_bound(host, s, d)?;
            let heap = host.heap();
            let col = bound.and_then(|g| item::get_column(heap, g));
            b[ix(d)] = col.map_or(0, |c| paper_column::rank(heap, c));
        }
    }

    Ok(b2[0] <= b1[0] && b2[1] >= b1[1] && (b2[0] != b1[0] || b2[1] != b1[1]))
}

/// `ly:slur::calc-control-points`: the best configuration's curve,
/// relative to the slur; none for a slur over no column, or with
/// LilyPond's warning if no configuration is viable.
pub fn calc_control_points(host: &mut dyn Host, me: Value) -> Run<Value> {
    let mut state = State::new(me);
    let Some(mut configs) = state.fill(host)? else {
        return Ok(Value::NULL);
    };
    if configs.is_empty() {
        grob::warning(host, me, "no viable slur configuration found")?;
        return Ok(Value::NULL);
    }

    state.generate_curves(host, &mut configs)?;

    let mut end_ys = grob::property(host, me, "positions")?;
    let inspect_quants = grob::property(host, me, "inspect-quants")?;
    let mut debug_slurs = {
        let odef = grob_of(host.heap(), me).map(|(arena, _)| arena.borrow().output_def());
        let sym = host.heap().intern("debug-slur-scoring");
        odef.and_then(|odef| host_lookup_variable(host, odef, sym)) == Some(Value::TRUE)
    };

    if grob::number_pair(host.heap(), inspect_quants).is_some() {
        debug_slurs = true;
        end_ys = inspect_quants;
    }

    let best = match grob::number_pair(host.heap(), end_ys) {
        Some(ys) => state.get_forced_configuration(host, &mut configs, ys)?,
        None => state.get_best_curve(host, &mut configs)?,
    };
    let best = &configs[best];

    if debug_slurs {
        let mut total: String = best.card();
        total.push_str(&format!(" TOTAL={:.2} idx={}", best.score(), best.index));
        let heap = host.heap();
        let v = heap.string(total);
        grob::set_property(heap, me, "annotation", v);
    }

    let curve = best.curve;
    let depth = host.heap().root_depth();
    let mut controls = Value::NULL;
    for i in (0..4).rev() {
        let at = (
            grob::relative_coordinate(host, me, state.common[X], X)?,
            grob::relative_coordinate(host, me, state.common[Y], Y)?,
        );
        let o = (curve[i].0 - at.0, curve[i].1 - at.1);
        let heap = host.heap();
        let p = crate::offset(heap, o);
        controls = heap.cons(p, controls);
        heap.root(controls);
    }
    host.heap().unroot_to(depth);

    Ok(controls)
}
