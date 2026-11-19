//! `tie-formatting-problem.cc` (MS4 step 9): the search for a column's
//! ties, which the tie column and the semi-tie column run for their ties
//! (`tie_column.rs`, `semi_tie_column.rs`) and a tie alone for its control
//! points (`tie.rs`). Each end's chord outline, a skyline of the heads,
//! dots, stems, flags, accidentals and the other heads of the chord
//! (`set_column_chord_outline`, `:89–286`), from which a configuration's
//! ends are read at its height (`get_attachment`, `:58–74`); the ties as
//! their grobs ask them (`from_ties`, `:327–380`, `from_semi_ties`,
//! `:382–436`); a configuration of each position and side made once and
//! kept (`get_configuration`, `generate_configuration`, `:438–616`); the
//! base configuration with the standard directions
//! (`generate_base_chord_configuration`, `set_ties_config_standard_directions`)
//! and its variations, collisions or a single tie's region, then the
//! extremal ties' (`generate_optimal_configuration`), each scored
//! (`score_configuration`, `score_aptitude`, `score_ties_configuration`)
//! and the cheapest kept; `tie-configuration` set by hand
//! (`set_manual_tie_configuration`), and `debug-tie-scoring`'s cards as
//! the ties' `annotation` (`set_debug_scoring`).
//!
//! With what it reads of other files (the split's step 9): stem.cc's
//! `flag` and `is_cross_staff`, axis-group-interface.cc's `staff_extent`,
//! misc.cc's `convex_amplifier`, and staff-symbol-referencer.cc's
//! `extent_in_staff`.
//!
//! LilyPond's clang fuses the sums of products here (otool,
//! `lilypond-otool.txt:478548–482700`): a configuration's offsets from the
//! heads, dots and lines, its close-by attachment, its Y beside the stems
//! and the stem's gap, the top of its curve and its new Y; the aptitude's
//! curve Y; the configuration score's tip and top and its dots' distance;
//! the position symmetry; the semi-ties' open side. Each is an [`fma`].

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of, host_lookup_variable};
use lily_engine_scheme::{Exception, Value};
use lily_engine_skyline::{Axis as Horizon, Direction as Sky, Skyline};

use crate::beam::{self, Slice};
use crate::beam_quanting::round_halfway_up;
use crate::bezier;
use crate::direction::get_grob_direction;
use crate::geometry::{Box as Bbox, Direction, Interval, X, Y, fma, max};
use crate::grob;
use crate::grob_array;
use crate::item;
use crate::paper_column;
use crate::rhythmic_head;
use crate::semi_tie;
use crate::slur_configuration::peak_around;
use crate::spanner;
use crate::staff_symbol;
use crate::staff_symbol_referencer as ssr;
use crate::stem;
use crate::tie;
use crate::tie_configuration::{Configuration, Details, Specification, TiesConfiguration};

type Run<T> = Result<T, Exception>;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const DOWN: Direction = -1;
const UP: Direction = 1;

/// `Tie_rank_and_dir`: a column's rank and a side.
type Key = (i64, Direction);

/// `Tie_configuration_map_key`: a position, a side and the columns.
type ConfigurationKey = (i32, Direction, i32, i32);

/// `Tie_configuration_variation`: configurations of the map to put in
/// place of some of a column's, by index.
#[derive(Default)]
struct Variation {
    index_suggestion_pairs: Vec<(usize, ConfigurationKey)>,
}

impl Variation {
    fn add_suggestion(&mut self, index: usize, suggestion: ConfigurationKey) {
        self.index_suggestion_pairs.push((index, suggestion));
    }
}

/// A `Drul_array`'s index of `d`.
fn ix(d: Direction) -> usize {
    usize::from(d > 0)
}

/// `Direction (x)` of a real: its sign, 0 for 0.
fn sign_of(x: f64) -> Direction {
    if x != 0.0 {
        if x.is_sign_negative() { -1 } else { 1 }
    } else {
        0
    }
}

/// `sign` of an int.
fn sign(x: i32) -> Direction {
    x.signum()
}

/// `Interval::contains`.
fn contains(iv: Interval, x: f64) -> bool {
    iv.left <= x && x <= iv.right
}

/// `Slice::contains`.
fn slice_contains(s: Slice, x: i32) -> bool {
    s.left <= x && x <= s.right
}

/// `Interval::distance`: how far `t` is outside the interval.
fn distance(iv: Interval, t: f64) -> f64 {
    if t > iv.right {
        t - iv.right
    } else if t < iv.left {
        iv.left - t
    } else {
        0.0
    }
}

/// `convex_amplifier` (misc.cc): 0 at 0, 1 at `standard_x`, increasing
/// thereafter.
fn convex_amplifier(standard_x: f64, increase_factor: f64, x: f64) -> f64 {
    (libm::exp(increase_factor * x / standard_x) - 1.0) / (libm::exp(increase_factor) - 1.0)
}

/// The skyline side of `d`.
fn sky(d: Direction) -> Sky {
    if d < 0 { Sky::Down } else { Sky::Up }
}

/// `Box`, in a skyline's form.
fn sky_box(b: Bbox) -> lily_engine_fonts::Box {
    let i = |i: Interval| lily_engine_fonts::Interval::new(i.left, i.right);
    lily_engine_fonts::Box {
        x: i(b.x()),
        y: i(b.y()),
    }
}

/// `boundary (v, dir, 0)`: the first for down or left, the last else.
fn boundary<T>(v: &[T], dir: Direction) -> &T {
    if dir == LEFT { &v[0] } else { &v[v.len() - 1] }
}

/// The grob in `me`'s object `name`, if it is one.
fn grob_object(host: &mut dyn Host, me: Value, name: &str) -> Run<Option<Value>> {
    let sym = host.heap().intern(name);
    let v = grob::object(host, me, sym)?;
    Ok(grob_of(host.heap(), v).map(|_| v))
}

/// `Stem::flag`.
fn stem_flag(host: &mut dyn Host, stem: Value) -> Run<Option<Value>> {
    grob_object(host, stem, "flag")
}

/// `Stem::is_cross_staff`: whether its beam is cross-staff.
fn stem_is_cross_staff(host: &mut dyn Host, stem: Value) -> Run<bool> {
    match grob_object(host, stem, "beam")? {
        Some(b) => beam::is_cross_staff(host, b),
        None => Ok(false),
    }
}

/// `Axis_group_interface::staff_extent`: the extent on `ext_a` relative
/// to `refp` of `me`'s elements under `staff` on `parent_a`.
fn staff_extent(
    host: &mut dyn Host,
    me: Value,
    refp: Value,
    ext_a: usize,
    staff: Value,
    parent_a: usize,
) -> Run<Interval> {
    let elts = grob_array::extract_grob_array(host, me, "elements")?;
    let heap = host.heap();
    let new_elts: Vec<Value> = elts
        .into_iter()
        .filter(|&e| grob::has_in_ancestry(heap, e, staff, parent_a))
        .collect();
    let list = heap.list(&new_elts);
    let axis = heap.integer(ext_a as i64);
    let ext = crate::call_lily(
        host,
        "%relative-maybe-bound-group-extent",
        &[list, refp, axis, Value::FALSE],
    )?;
    Ok(grob::number_pair(host.heap(), ext).unwrap_or(Interval::EMPTY))
}

/// `Staff_symbol_referencer::on_line`: whether `pos` is on a line of the
/// grob's staff symbol, or a ledger line there.
fn on_line(host: &mut dyn Host, me: Value, pos: i32) -> Run<bool> {
    match ssr::staff_symbol(host, me)? {
        Some(st) => staff_symbol::on_line(host, st, pos, true),
        None => Ok(false),
    }
}

/// `Staff_symbol_referencer::on_staff_line`: whether `pos` is on a line
/// of the grob's staff symbol.
fn on_staff_line(host: &mut dyn Host, me: Value, pos: i32) -> Run<bool> {
    match ssr::staff_symbol(host, me)? {
        Some(st) => staff_symbol::on_line(host, st, pos, false),
        None => Ok(false),
    }
}

/// `Tie_formatting_problem`.
pub(crate) struct Problem {
    chord_outlines: BTreeMap<Key, Skyline>,
    stem_extents: BTreeMap<Key, Bbox>,
    head_extents: BTreeMap<Key, Bbox>,
    head_positions: BTreeMap<i64, Slice>,

    dot_positions: BTreeSet<i32>,
    dot_x: Interval,
    specifications: Vec<Specification>,
    use_horizontal_spacing: bool,

    possibilities: BTreeMap<ConfigurationKey, Configuration>,

    x_refpoint: Value,
    y_refpoint: Value,

    pub details: Details,
}

impl Problem {
    pub fn new() -> Problem {
        Problem {
            chord_outlines: BTreeMap::new(),
            stem_extents: BTreeMap::new(),
            head_extents: BTreeMap::new(),
            head_positions: BTreeMap::new(),
            dot_positions: BTreeSet::new(),
            dot_x: Interval::EMPTY,
            specifications: Vec::new(),
            use_horizontal_spacing: true,
            possibilities: BTreeMap::new(),
            x_refpoint: Value::NULL,
            y_refpoint: Value::NULL,
            details: Details::default(),
        }
    }

    /// `common_x_refpoint`.
    pub fn common_x_refpoint(&self) -> Value {
        self.x_refpoint
    }

    /// `get_attachment`: the outlines' X at `y` of the columns' left and
    /// right ends; 0 where an outline is missing (LilyPond's programming
    /// error).
    fn get_attachment(&self, host: &mut dyn Host, y: f64, columns: [i32; 2]) -> Run<Interval> {
        let mut attachments = Interval::new(0.0, 0.0);

        for d in [LEFT, RIGHT] {
            match self.chord_outlines.get(&(i64::from(columns[ix(d)]), d)) {
                None => grob::unlocated_programming_error(host, "Cannot find chord outline")?,
                Some(s) => *attachments.at_mut(d) = s.height(y),
            }
        }

        Ok(attachments)
    }

    fn x_extent(&self, host: &mut dyn Host, g: Value) -> Run<Interval> {
        grob::extent(host, g, self.x_refpoint, X)
    }

    /// `set_column_chord_outline`: the outline at side `dir` of the heads
    /// `bounds` of the column `column_rank`: their boxes and their dots' on
    /// the left, their stem's (its flag's on the left) or, for no normal
    /// stem, the heads' height out to the side, the chord's other heads,
    /// its accidentals on the right, and above and below the extremal
    /// heads; the heads' positions, extents and the stem's extent kept.
    fn set_column_chord_outline(
        &mut self,
        host: &mut dyn Host,
        bounds: &[Value],
        dir: Direction,
        column_rank: i64,
    ) -> Run<()> {
        let staff_space = ssr::staff_space(host, bounds[0])?;

        let mut boxes = Vec::new();
        let mut head_boxes = Vec::new();

        let mut stem = None;
        for &head in bounds {
            if !grob::has_interface(host.heap(), head, "note-head-interface") {
                continue;
            }

            if stem.is_none() {
                stem = grob_object(host, head, "stem")?;
            }

            let p = ssr::get_position(host, head, false)?;
            let y = Interval::new((p - 1.0) * 0.5 * staff_space, (p + 1.0) * 0.5 * staff_space);

            let x = self.x_extent(host, head)?;
            head_boxes.push(Bbox::new(x, y));
            boxes.push(Bbox::new(x, y));

            let dots = rhythmic_head::get_dots(host, head)?;
            if dir == LEFT
                && let Some(dots) = dots
            {
                let x = self.x_extent(host, dots)?;
                let p = ssr::get_position(host, dots, false)? as i32;

                /*
                  TODO: shouldn't this use column-rank dependent key?
                */
                self.dot_positions.insert(p);
                self.dot_x.unite(x);

                let mut y = grob::extent(host, dots, dots, Y)?;
                y.translate(f64::from(p) * staff_space * 0.5);

                boxes.push(Bbox::new(x, y));
            }
        }

        let key = (column_rank, dir);

        if let Some(stem) = stem {
            if stem::is_normal_stem(host, stem)? {
                let mut x = Interval::EMPTY;
                x.add_point(grob::relative_coordinate(host, stem, self.x_refpoint, X)?);
                x.widen(staff_space / 20.0); // ugh.
                let mut y = Interval::EMPTY;

                let stem_dir = get_grob_direction(host, stem)?;
                let stem_end_position = if stem_is_cross_staff(host, stem)? {
                    f64::from(stem_dir) * f64::INFINITY
                } else if self.use_horizontal_spacing || stem::get_beam(host, stem)?.is_none() {
                    grob::extent(host, stem, stem, Y)?.at(stem_dir)
                } else {
                    // May want to change this to the stem's pure height...
                    stem::head_positions(host, stem)?.at(stem_dir) * staff_space * 0.5
                };

                y.add_point(stem_end_position);

                y.add_point(stem::head_positions(host, stem)?.at(-stem_dir) * staff_space * 0.5);

                /*
                  add extents of stem.
                */
                boxes.push(Bbox::new(x, y));

                self.stem_extents
                    .entry(key)
                    .or_insert(Bbox::EMPTY)
                    .unite(Bbox::new(x, y));

                if dir == LEFT
                    && let Some(flag) = stem_flag(host, stem)?
                {
                    let commony = grob::common_refpoint(host.heap(), stem, flag, Y);
                    let fx = self.x_extent(host, flag)?;
                    let fy = grob::extent(host, flag, commony.unwrap_or(Value::NULL), Y)?;
                    boxes.push(Bbox::new(fx, fy));
                }
            } else if let Some(head) = stem::support_head(host, stem)? {
                /*
                  In case of invisible stem, don't pass x-center of heads.
                */
                let x_center = self.x_extent(host, head)?.center();
                let mut x_ext = Interval::EMPTY;
                *x_ext.at_mut(-dir) = x_center;
                *x_ext.at_mut(dir) = f64::INFINITY * f64::from(dir);
                let mut y_ext = Interval::EMPTY;
                for b in &head_boxes {
                    y_ext.unite(b.y());
                }

                boxes.push(Bbox::new(x_ext, y_ext));
            }

            let heads = grob_array::extract_grob_array(host, stem, "note-heads")?;
            for &h in &heads {
                if !bounds.contains(&h) {
                    /*
                      other untied notes in the same chord.
                    */
                    let y = ssr::extent_in_staff(host, h)?;
                    let x = self.x_extent(host, h)?;
                    boxes.push(Bbox::new(x, y));
                }

                let acc = grob_object(host, h, "accidental-grob")?;
                if let Some(acc) = acc {
                    grob::property(host, acc, "after-line-breaking")?; /* trigger tie-related suicide */
                    if grob::is_live_grob(host.heap(), acc) && dir == RIGHT {
                        let ax = self.x_extent(host, acc)?;
                        let ay = ssr::extent_in_staff(host, acc)?;
                        boxes.push(Bbox::new(ax, ay));
                    }
                }

                let p = ssr::get_position(host, h, false)? as i32;
                self.head_positions
                    .entry(column_rank)
                    .or_insert(Slice::EMPTY)
                    .add_point(p);
            }
        }

        for updowndir in [DOWN, UP] {
            let mut x = Interval::EMPTY;
            let mut y = Interval::EMPTY;
            if !head_boxes.is_empty() {
                let b = *boundary(&head_boxes, updowndir);
                x = b.x();
                // `-dir / 2` is an int: 0.
                *x.at_mut(-dir) = b.x().linear_combination(0.0);
                *y.at_mut(-updowndir) = b.y().at(updowndir);
                *y.at_mut(updowndir) = f64::from(updowndir) * f64::INFINITY;
            }

            if !x.is_empty() {
                boxes.push(Bbox::new(x, y));
            }
        }

        let sky_boxes: Vec<_> = boxes.iter().map(|&b| sky_box(b)).collect();
        if self.details.skyline_padding < 0.0 {
            grob::unlocated_warning(host, "Cannot have negative horizon padding.  Junking.")?;
        }
        let mut outline = Skyline::from_boxes(&sky_boxes, Horizon::Y, sky(-dir))
            .padded(self.details.skyline_padding);
        if item::break_status_dir(host.heap(), bounds[0]) != 0 {
            let mut iv = staff_extent(host, bounds[0], self.x_refpoint, X, self.y_refpoint, Y)?;
            if iv.is_empty() {
                iv.add_point(grob::relative_coordinate(
                    host,
                    bounds[0],
                    self.x_refpoint,
                    X,
                )?);
            }

            outline.set_minimum_height(iv.at(-dir));
        } else {
            let mut x = Interval::EMPTY;
            for b in &head_boxes {
                x.unite(b.x());
            }

            outline.set_minimum_height(x.at(dir));
        }
        self.chord_outlines.insert(key, outline);

        let mut head_extent = Bbox::EMPTY;
        for &b in &head_boxes {
            head_extent.unite(b);
        }
        self.head_extents.insert(key, head_extent);
        Ok(())
    }

    /// `set_chord_outline`: each column's outline of the items `bounds`,
    /// by rank.
    fn set_chord_outline(
        &mut self,
        host: &mut dyn Host,
        bounds: &[Value],
        dir: Direction,
    ) -> Run<()> {
        let rank = |host: &mut dyn Host, b: Value| {
            let heap = host.heap();
            item::get_column(heap, b).map_or(0, |c| paper_column::rank(heap, c)) as i64
        };
        let mut ranks: Vec<i64> = bounds.iter().map(|&b| rank(host, b)).collect();

        ranks.sort_unstable();
        ranks.dedup();

        for r in ranks {
            let mut col_items = Vec::new();
            for &b in bounds {
                if rank(host, b) == r {
                    col_items.push(b);
                }
            }

            self.set_column_chord_outline(host, &col_items, dir, r)?;
        }
        Ok(())
    }

    /// `from_tie`.
    pub fn read_tie(&mut self, host: &mut dyn Host, tie: Value) -> Run<()> {
        self.read_ties(host, &[tie])?;

        self.details.read_grob(host, tie)
    }

    /// `from_ties`: the refpoints of the ties and their bounds (on Y those
    /// not broken), the details of the first, the outlines of the bounds
    /// (a broken one's column) at each end, and each tie's specification.
    pub fn read_ties(&mut self, host: &mut dyn Host, ties: &[Value]) -> Run<()> {
        if ties.is_empty() {
            return Ok(());
        }

        let bound = |host: &mut dyn Host, t: Value, d: Direction| -> Run<Value> {
            Ok(spanner::get_bound(host, t, d)?.unwrap_or(Value::NULL))
        };

        self.x_refpoint = ties[0];
        self.y_refpoint = ties[0];
        for &tie in ties {
            let l = bound(host, tie, LEFT)?;
            let r = bound(host, tie, RIGHT)?;

            let heap = host.heap();
            let common = |a: Value, b: Value, ax| {
                grob::common_refpoint(heap, a, b, ax).unwrap_or(Value::NULL)
            };
            self.x_refpoint = common(l, self.x_refpoint, X);
            self.x_refpoint = common(r, self.x_refpoint, X);

            if item::break_status_dir(heap, l) == 0 {
                self.y_refpoint = common(l, self.y_refpoint, Y);
            }
            if item::break_status_dir(heap, r) == 0 {
                self.y_refpoint = common(r, self.y_refpoint, Y);
            }
        }

        self.details.read_grob(host, ties[0])?;

        for d in [LEFT, RIGHT] {
            let mut bounds = Vec::new();

            for &tie in ties {
                let mut it = bound(host, tie, d)?;
                let heap = host.heap();
                if item::break_status_dir(heap, it) != 0 {
                    it = item::get_column(heap, it).unwrap_or(Value::NULL);
                }

                bounds.push(it);
            }

            self.set_chord_outline(host, &bounds, d)?;
        }

        for &tie in ties {
            let mut spec = Specification::default();
            spec.read_grob(host, tie)?;

            for d in [LEFT, RIGHT] {
                spec.note_head_drul[ix(d)] = tie::head(host, tie, d)?;
                spec.column_ranks[ix(d)] = tie::get_column_rank(host, tie, d)? as i32;
            }
            self.specifications.push(spec);
        }
        Ok(())
    }

    /// `from_semi_ties`: each semi-tie's specification at its head on
    /// `head_dir`, the refpoints of the semi-ties and their heads, the
    /// heads' outline, and on the open side an outline 1.5 out from it.
    pub fn read_semi_ties(
        &mut self,
        host: &mut dyn Host,
        semi_ties: &[Value],
        head_dir: Direction,
    ) -> Run<()> {
        if semi_ties.is_empty() {
            return Ok(());
        }

        self.use_horizontal_spacing = false;
        self.details.read_grob(host, semi_ties[0])?;
        let mut heads = Vec::new();

        let mut column_rank = i64::MAX;
        for &semi_tie in semi_ties {
            let mut spec = Specification::default();
            let head = semi_tie::head(host, semi_tie)?;

            if head.is_none() {
                grob::unlocated_programming_error(host, "LV tie without head?!")?;
            }

            if let Some(head) = head {
                spec.position = ssr::get_position(host, head, false)? as i32;
            }

            spec.read_grob(host, semi_tie)?;

            spec.note_head_drul[ix(head_dir)] = head;
            column_rank = semi_tie::get_column_rank(host, semi_tie) as i64;
            spec.column_ranks = [column_rank as i32; 2];
            heads.extend(head);
            self.specifications.push(spec);
        }

        self.x_refpoint = semi_ties[0];
        self.y_refpoint = semi_ties[0];

        let heap = host.heap();
        let common =
            |a: Value, b: Value, ax| grob::common_refpoint(heap, a, b, ax).unwrap_or(Value::NULL);
        for &st in semi_ties.iter().chain(&heads) {
            self.x_refpoint = common(st, self.x_refpoint, X);
            self.y_refpoint = common(st, self.y_refpoint, Y);
        }

        self.set_chord_outline(host, &heads, head_dir)?;

        let head_key = (column_rank, head_dir);
        let open_key = (column_rank, -head_dir);
        let extremal = self
            .chord_outlines
            .entry(head_key)
            .or_insert_with(|| Skyline::empty(Sky::Up))
            .max_height();

        let mut open = Skyline::empty(sky(head_dir));
        open.set_minimum_height(fma(f64::from(head_dir), -1.5, extremal));
        self.chord_outlines.insert(open_key, open);
        Ok(())
    }

    /// `get_configuration`: the configuration's key, made the first time
    /// it is asked.
    fn get_configuration(
        &mut self,
        host: &mut dyn Host,
        pos: i32,
        dir: Direction,
        columns: [i32; 2],
        tune_dy: bool,
    ) -> Run<ConfigurationKey> {
        let key = (pos, dir, columns[0], columns[1]);

        if !self.possibilities.contains_key(&key) {
            let conf = self.generate_configuration(host, pos, dir, columns, tune_dy)?;
            self.possibilities.insert(key, conf);
        }
        Ok(key)
    }

    /// `generate_configuration`: the tie at `pos` on side `dir` between
    /// `columns`: moved off the dots, off the heads when it nearly touches
    /// them, and, `y_tune`, off the staff lines by its tips or its top, or
    /// centred in its space; its ends where the outlines are at its height
    /// (nearer for a flat one), a gap from the heads and from the stems it
    /// meets.
    fn generate_configuration(
        &mut self,
        host: &mut dyn Host,
        pos: i32,
        dir: Direction,
        columns: [i32; 2],
        mut y_tune: bool,
    ) -> Run<Configuration> {
        let d = self.details;
        let mut conf = Configuration::default();
        conf.position = pos;
        conf.dir = dir;

        conf.column_ranks = columns;

        let y = f64::from(conf.position) * 0.5 * d.staff_space;

        if self.dot_positions.contains(&pos) {
            conf.delta_y = fma(f64::from(dir) * 0.25, d.staff_space, conf.delta_y);
            y_tune = false;
        }

        if y_tune
            && max(
                libm::fabs(self.get_head_extent(columns[0], LEFT, Y).at(dir) - y),
                libm::fabs(self.get_head_extent(columns[1], RIGHT, Y).at(dir) - y),
            ) < 0.25
            && !on_line(host, d.staff_symbol_referencer, pos)?
        {
            conf.delta_y = fma(
                f64::from(dir),
                d.outer_tie_vertical_gap,
                self.get_head_extent(columns[0], LEFT, Y).at(dir) - y,
            );
        }

        if y_tune {
            conf.attachment_x = self.get_attachment(host, y + conf.delta_y, conf.column_ranks)?;
            let h = conf.height(&d);

            /*
             TODO:

             - should make sliding criterion, should flatten ties if

             - they're just the wrong (ie. touching line at top & bottom)
             size.

            */
            let mut staff_span = ssr::staff_span(host, d.staff_symbol_referencer)?;
            staff_span.widen(-1.0);
            let within_staff = contains(staff_span, f64::from(pos));
            if slice_contains(self.head_positions_slice(columns[0]), pos)
                || slice_contains(self.head_positions_slice(columns[1]), pos)
                || within_staff
            {
                if h < d.intra_space_threshold * 0.5 * d.staff_space {
                    if on_line(host, d.staff_symbol_referencer, pos)? {
                        conf.delta_y = fma(
                            f64::from(dir) * d.tip_staff_line_clearance * 0.5,
                            d.staff_space,
                            conf.delta_y,
                        );
                    } else if within_staff {
                        conf.center_tie_vertically(host, &d)?;
                    }
                } else {
                    let top_y = fma(f64::from(conf.dir), h, y + conf.delta_y);
                    let top_pos = top_y / (0.5 * d.staff_space);
                    let round_pos = round_halfway_up(top_pos) as i32;

                    /* TODO: should use other variable? */
                    let clearance = d.center_staff_line_clearance;
                    if libm::fabs(top_pos - f64::from(round_pos)) < clearance
                        && on_staff_line(host, d.staff_symbol_referencer, round_pos)?
                    {
                        let new_y = fma(clearance, f64::from(conf.dir), f64::from(round_pos))
                            * 0.5
                            * d.staff_space;
                        conf.delta_y = new_y - top_y;
                    }
                }
            }
        }
        conf.attachment_x = self.get_attachment(host, y + conf.delta_y, conf.column_ranks)?;
        if conf.height(&d) < d.intra_space_threshold * 0.5 * d.staff_space {
            /*
              This is less sensible for long ties, since those are more
              horizontal.
            */
            let close_by = self.get_attachment(
                host,
                fma(
                    f64::from(dir) * d.intra_space_threshold * 0.25,
                    d.staff_space,
                    y + conf.delta_y,
                ),
                conf.column_ranks,
            )?;

            conf.attachment_x.intersect(close_by);
        }

        conf.attachment_x.widen(-d.x_gap);

        if conf.column_span_length() != 0 {
            /*
              avoid the stems that we attach to as well. We don't do this
              for semities (span length = 0)

              It would be better to check D against HEAD-DIRECTION if
              applicable.
            */
            for e in [LEFT, RIGHT] {
                let y = fma(f64::from(conf.position) * d.staff_space, 0.5, conf.delta_y);
                let rank = conf.column_ranks[ix(e)];
                if self.get_stem_extent(rank, e, X).is_empty()
                    || !contains(self.get_stem_extent(rank, e, Y), y)
                {
                    continue;
                }

                let e_f = f64::from(e);
                let ours = e_f * conf.attachment_x.at(e);
                let stems = e_f * fma(-e_f, d.stem_gap, self.get_stem_extent(rank, e, X).at(-e));
                *conf.attachment_x.at_mut(e) = e_f * if stems < ours { stems } else { ours };
            }
        }
        Ok(conf)
    }

    /// `get_head_extent`: the heads' extent on `a` at the column's side,
    /// empty for none.
    fn get_head_extent(&self, col: i32, d: Direction, a: usize) -> Interval {
        self.head_extents
            .get(&(i64::from(col), d))
            .map_or(Interval::EMPTY, |b| b.0[a])
    }

    /// `get_stem_extent`: the stem's extent on `a` at the column's side,
    /// empty for none.
    fn get_stem_extent(&self, col: i32, d: Direction, a: usize) -> Interval {
        self.stem_extents
            .get(&(i64::from(col), d))
            .map_or(Interval::EMPTY, |b| b.0[a])
    }

    /// `score_aptitude`, always given `ties_conf` (`score_ties_aptitude`):
    /// tie `tie_idx`'s configuration against its specification: offset the
    /// wrong way, its distance from its position and its ends' from its
    /// heads; a single tie on its stems' side, or against its position.
    fn score_aptitude(
        &self,
        host: &mut dyn Host,
        conf: &Configuration,
        spec: &Specification,
        ties_conf: &mut TiesConfiguration,
        tie_idx: usize,
    ) -> Run<()> {
        let d = &self.details;
        let curve_y = fma(f64::from(conf.position) * d.staff_space, 0.5, conf.delta_y);
        let tie_y = f64::from(spec.position) * d.staff_space * 0.5;
        if sign_of(curve_y - tie_y) != conf.dir {
            let p = d.wrong_direction_offset_penalty;
            ties_conf.add_tie_score(p, tie_idx, "wrong dir");
        }

        {
            let gap = libm::fabs(curve_y - tie_y) - 0.5;
            let relevant_dist = if gap < 0.0 { 0.0 } else { gap };
            let p = d.vertical_distance_penalty_factor * convex_amplifier(1.0, 0.9, relevant_dist);
            ties_conf.add_tie_score(p, tie_idx, "vdist");
        }

        for e in [LEFT, RIGHT] {
            let Some(head) = spec.note_head_drul[ix(e)] else {
                continue;
            };

            let head_x = self.x_extent(host, head)?;
            let dist = distance(head_x, conf.attachment_x.at(e));

            /*
             TODO: flatten with log or sqrt.
            */
            let p = d.horizontal_distance_penalty_factor * convex_amplifier(1.25, 1.0, dist);
            ties_conf.add_tie_score(p, tie_idx, if e == LEFT { "lhdist" } else { "rhdist" });
        }

        if ties_conf.len() == 1 {
            let mut stems = [None; 2];
            for e in [LEFT, RIGHT] {
                let Some(head) = spec.note_head_drul[ix(e)] else {
                    continue;
                };

                let stem = grob_object(host, head, "stem")?;
                if let Some(stem) = stem
                    && stem::is_normal_stem(host, stem)?
                {
                    stems[ix(e)] = Some(stem);
                }
            }

            let mut tie_stem_dir_ok = true;
            let mut tie_position_dir_ok = true;
            match stems {
                [Some(l), None] => tie_stem_dir_ok = conf.dir != get_grob_direction(host, l)?,
                [None, Some(r)] => tie_stem_dir_ok = conf.dir != get_grob_direction(host, r)?,
                [Some(l), Some(r)]
                    if get_grob_direction(host, l)? == get_grob_direction(host, r)? =>
                {
                    tie_stem_dir_ok = conf.dir != get_grob_direction(host, l)?;
                }
                _ => {
                    if spec.position != 0 {
                        tie_position_dir_ok = conf.dir == sign(spec.position);
                    }
                }
            }

            if !tie_stem_dir_ok {
                ties_conf.add_score(d.same_dir_as_stem_penalty, "tie/stem dir");
            }
            if !tie_position_dir_ok {
                ties_conf.add_score(d.same_dir_as_stem_penalty, "tie/pos dir");
            }
        }

        Ok(())
    }

    /// `head_positions_slice`: the positions of the column's heads, empty
    /// for none.
    fn head_positions_slice(&self, rank: i32) -> Slice {
        self.head_positions
            .get(&i64::from(rank))
            .copied()
            .unwrap_or(Slice::EMPTY)
    }

    /// `score_configuration`: how well the tie looks without regard to the
    /// heads it connects: too short, its top or tips near a staff line,
    /// its curve near the dots.
    fn score_configuration(&self, host: &mut dyn Host, conf: &mut Configuration) -> Run<()> {
        if conf.is_scored() {
            return Ok(());
        }
        let d = &self.details;

        let length = conf.attachment_x.length();

        let length_penalty = peak_around(0.33 * d.min_length, d.min_length, length);
        conf.add_score(d.min_length_penalty_factor * length_penalty, "minlength");

        let tip_pos = fma(conf.delta_y / 0.5, d.staff_space, f64::from(conf.position));
        let tip_y = tip_pos * d.staff_space * 0.5;
        let height = conf.height(d);

        let top_y = fma(f64::from(conf.dir), height, tip_y);
        let top_pos = 2.0 * top_y / d.staff_space;
        let round_top_pos = libm::rint(top_pos);
        let mut staff_span = ssr::staff_span(host, d.staff_symbol_referencer)?;
        if on_line(host, d.staff_symbol_referencer, round_top_pos as i32)?
            && staff_span.right * 0.5 > top_y
        {
            conf.add_score(
                d.staff_line_collision_penalty
                    * peak_around(
                        0.1 * d.center_staff_line_clearance,
                        d.center_staff_line_clearance,
                        libm::fabs(top_pos - round_top_pos),
                    ),
                "line center",
            );
        }

        let rounded_tip_pos = libm::rint(tip_pos) as i32;
        staff_span.widen(-1.0);
        if on_line(host, d.staff_symbol_referencer, rounded_tip_pos)?
            && (slice_contains(
                self.head_positions_slice(conf.column_ranks[0]),
                rounded_tip_pos,
            ) || slice_contains(
                self.head_positions_slice(conf.column_ranks[1]),
                rounded_tip_pos,
            ) || contains(staff_span, f64::from(rounded_tip_pos)))
        {
            conf.add_score(
                d.staff_line_collision_penalty
                    * peak_around(
                        0.1 * d.tip_staff_line_clearance,
                        d.tip_staff_line_clearance,
                        libm::fabs(tip_pos - libm::rint(tip_pos)),
                    ),
                "tipline",
            );
        }

        if !self.dot_x.is_empty() {
            /* use left edge? */
            let x = self.dot_x.center();

            let b = conf.get_transformed_bezier(host, d)?;
            if contains(bezier::control_point_extent(&b, X), x) {
                let mut errors = Vec::new();
                let y = bezier::get_other_coordinate(&b, X, x, &mut errors);
                for text in &errors {
                    grob::unlocated_programming_error(host, text)?;
                }

                for &dot_pos in &self.dot_positions {
                    conf.add_score(
                        d.dot_collision_penalty
                            * peak_around(
                                0.1 * d.dot_collision_clearance,
                                d.dot_collision_clearance,
                                libm::fabs(fma(f64::from(dot_pos) * d.staff_space, 0.5, -y)),
                            ),
                        "dot collision",
                    );
                }
            }
        }

        conf.set_scored();
        Ok(())
    }

    /// `score_ties_aptitude`: each tie's aptitude.
    fn score_ties_aptitude(&self, host: &mut dyn Host, ties: &mut TiesConfiguration) -> Run<()> {
        if ties.len() != self.specifications.len() {
            return grob::unlocated_programming_error(host, "Huh? Mismatch between sizes.");
        }

        for i in 0..ties.len() {
            let conf = ties.ties[i].clone();
            self.score_aptitude(host, &conf, &self.specifications[i], ties, i)?;
        }
        Ok(())
    }

    /// `score_ties`: scored once.
    fn score_ties(&self, host: &mut dyn Host, ties: &mut TiesConfiguration) -> Run<()> {
        if ties.is_scored() {
            return Ok(());
        }

        self.score_ties_configuration(host, ties)?;
        self.score_ties_aptitude(host, ties)?;
        ties.set_scored();
        Ok(())
    }

    /// `score_ties_configuration`: each tie's configuration score; ties out
    /// of order or close to the one below; the outer ties' lengths and
    /// offsets from their heads unequal.
    fn score_ties_configuration(
        &self,
        host: &mut dyn Host,
        ties: &mut TiesConfiguration,
    ) -> Run<()> {
        let d = &self.details;
        for i in 0..ties.len() {
            self.score_configuration(host, &mut ties.ties[i])?;
            let s = ties.ties[i].score();
            ties.add_tie_score(s, i, "conf");
        }

        let mut last_edge = 0.0;
        let mut last_center = 0.0;
        for i in 0..ties.len() {
            let b = ties.ties[i].get_transformed_bezier(host, d)?;

            let center = bezier::curve_point(&b, 0.5).1;
            let edge = bezier::curve_point(&b, 0.0).1;

            if i != 0 {
                if edge <= last_edge {
                    ties.add_score(d.tie_column_monotonicity_penalty, "monotone edge");
                }
                if center <= last_center {
                    ties.add_score(d.tie_column_monotonicity_penalty, "monotone cent");
                }

                ties.add_score(
                    d.tie_tie_collision_penalty
                        * peak_around(
                            0.1 * d.tie_tie_collision_distance,
                            d.tie_tie_collision_distance,
                            libm::fabs(center - last_center),
                        ),
                    "tietie center",
                );
                ties.add_score(
                    d.tie_tie_collision_penalty
                        * peak_around(
                            0.1 * d.tie_tie_collision_distance,
                            d.tie_tie_collision_distance,
                            libm::fabs(edge - last_edge),
                        ),
                    "tietie edge",
                );
            }

            last_edge = edge;
            last_center = center;
        }

        if ties.len() > 1 {
            let front = &ties.ties[0];
            let back = &ties.ties[ties.len() - 1];
            let length_symm = d.outer_tie_length_symmetry_penalty_factor
                * libm::fabs(front.attachment_x.length() - back.attachment_x.length());

            let offset = |spec: &Specification, conf: &Configuration| {
                let at = fma(f64::from(conf.position) * 0.5, d.staff_space, conf.delta_y);
                libm::fabs(fma(f64::from(spec.position) * 0.5, d.staff_space, -at))
            };
            let first = self.specifications[0];
            let last = self.specifications[self.specifications.len() - 1];
            let pos_symmetry = d.outer_tie_vertical_distance_symmetry_penalty_factor
                * libm::fabs(offset(&first, front) - offset(&last, back));

            ties.add_score(length_symm, "length symm");
            ties.add_score(pos_symmetry, "pos symmetry");
        }
        Ok(())
    }

    /// `generate_ties_configuration`: each tie's configuration of the map,
    /// its offset set by hand where its specification says so.
    fn generate_ties_configuration(
        &mut self,
        host: &mut dyn Host,
        ties_config: &TiesConfiguration,
    ) -> Run<TiesConfiguration> {
        let mut copy = TiesConfiguration::default();
        for (i, t) in ties_config.ties.iter().enumerate() {
            let spec = self.specifications[i];
            let key = self.get_configuration(
                host,
                t.position,
                t.dir,
                t.column_ranks,
                !spec.has_manual_delta_y,
            )?;
            let ptr = self.possibilities.get_mut(&key).expect("made above");
            if spec.has_manual_delta_y {
                ptr.delta_y =
                    (spec.manual_position - f64::from(t.position)) * 0.5 * self.details.staff_space;
            }
            copy.ties.push(ptr.clone());
        }

        Ok(copy)
    }

    /// `generate_base_chord_configuration`: each tie at its position (set
    /// by hand, rounded), on its side set by hand or the standard one, a
    /// position out unless set to 0.
    fn generate_base_chord_configuration(&mut self, host: &mut dyn Host) -> Run<TiesConfiguration> {
        let mut ties_config = TiesConfiguration::default();
        for spec in &self.specifications {
            let mut conf = Configuration::default();
            if spec.has_manual_dir {
                conf.dir = spec.manual_dir;
            }
            if spec.has_manual_position {
                conf.position = round_halfway_up(spec.manual_position) as i32;
                if spec.has_manual_delta_y {
                    conf.delta_y = (spec.manual_position - f64::from(conf.position))
                        * 0.5
                        * self.details.staff_space;
                }
            } else {
                conf.position = spec.position;
            }

            conf.column_ranks = spec.column_ranks;
            ties_config.ties.push(conf);
        }

        self.set_ties_config_standard_directions(&mut ties_config);
        for (i, t) in ties_config.ties.iter_mut().enumerate() {
            if self.specifications[i].manual_position == 0.0 {
                t.position += t.dir;
            }
        }

        self.generate_ties_configuration(host, &ties_config)
    }

    /// `find_best_variation`: `base` with each variation's configurations
    /// in place, the cheapest (the first of equal scores).
    fn find_best_variation(
        &self,
        host: &mut dyn Host,
        base: &TiesConfiguration,
        vars: &[Variation],
    ) -> Run<TiesConfiguration> {
        let mut best = base.clone();

        /*
          This simply is 1-opt: we have K substitions, and we try applying
          exactly every one for each.
        */
        for var in vars {
            let mut variant = base.clone();
            for &(index, key) in &var.index_suggestion_pairs {
                variant.ties[index] = self.possibilities[&key].clone();
            }

            variant.reset_score();
            self.score_ties(host, &mut variant)?;

            if variant.score() < best.score() {
                best = variant;
            }
        }

        Ok(best)
    }

    /// `generate_optimal_configuration`: the base, then its best
    /// variation, then (several ties) the best of its extremal ties'.
    pub fn generate_optimal_configuration(
        &mut self,
        host: &mut dyn Host,
    ) -> Run<TiesConfiguration> {
        let mut base = self.generate_base_chord_configuration(host)?;
        self.score_ties(host, &mut base)?;

        let vars = if self.specifications.len() > 1 {
            self.generate_collision_variations(host, &base)?
        } else {
            self.generate_single_tie_variations(host, &base)?
        };

        let mut best = self.find_best_variation(host, &base, &vars)?;

        if self.specifications.len() > 1 {
            let vars = self.generate_extremal_tie_variations(host, &best)?;
            best = self.find_best_variation(host, &best, &vars)?;
        }
        Ok(best)
    }

    /// `set_ties_config_standard_directions`: the lowest tie down (a
    /// single one by its position, else its neutral direction), the
    /// highest up; seconds apart, and ties of unequal spans, each other's
    /// way; the rest by their positions, down at the middle line.
    fn set_ties_config_standard_directions(&self, tie_configs: &mut TiesConfiguration) {
        let ties = &mut tie_configs.ties;
        if ties.is_empty() {
            return;
        }

        let n = ties.len();
        if ties[0].dir == 0 {
            if n == 1 {
                ties[0].dir = sign(ties[0].position);
            }

            if ties[0].dir == 0 {
                ties[0].dir = if n > 1 {
                    DOWN
                } else {
                    self.details.neutral_direction
                };
            }
        }

        if ties[n - 1].dir == 0 {
            ties[n - 1].dir = UP;
        }

        /*
         Seconds
        */
        for i in 1..n {
            let diff = f64::from(ties[i].position - ties[i - 1].position);

            let span_diff = f64::from(
                self.specifications[i].column_span() - self.specifications[i - 1].column_span(),
            );
            if span_diff != 0.0 && libm::fabs(diff) <= 2.0 {
                if span_diff > 0.0 {
                    ties[i].dir = UP;
                } else if span_diff < 0.0 {
                    ties[i - 1].dir = DOWN;
                }
            } else if libm::fabs(diff) <= 1.0 {
                if ties[i - 1].dir == 0 {
                    ties[i - 1].dir = DOWN;
                }
                if ties[i].dir == 0 {
                    ties[i].dir = UP;
                }
            }
        }

        for conf in ties.iter_mut() {
            if conf.dir != 0 {
                continue;
            }

            let mut position_dir = sign(conf.position);
            if position_dir == 0 {
                position_dir = DOWN;
            }

            conf.dir = position_dir;
        }
    }

    /// `generate_extremal_tie_variations`: the outer ties, if on their own
    /// sides and not placed by hand, moved out by 1 to the region's size,
    /// each alone and both.
    fn generate_extremal_tie_variations(
        &mut self,
        host: &mut dyn Host,
        ties: &TiesConfiguration,
    ) -> Run<Vec<Variation>> {
        let mut vars = Vec::new();
        for i in 1..=self.details.multi_tie_region_size {
            let mut configs = [None; 2];
            for d in [DOWN, UP] {
                let config = boundary(&ties.ties, d).clone();
                if config.dir == d && !boundary(&self.specifications, d).has_manual_position {
                    let mut var = Variation::default();
                    let key = self.get_configuration(
                        host,
                        config.position + d * i,
                        d,
                        config.column_ranks,
                        true,
                    )?;
                    configs[ix(d)] = Some(key);
                    var.add_suggestion(if d == DOWN { 0 } else { ties.len() - 1 }, key);
                    vars.push(var);
                }
            }
            if let [Some(down), Some(up)] = configs {
                let mut var = Variation::default();
                var.add_suggestion(0, down);
                var.add_suggestion(ties.len() - 1, up);
                vars.push(var);
            }
        }

        Ok(vars)
    }

    /// `generate_single_tie_variations`: the tie on the other side, then
    /// moved by up to the region's size each way (once if placed by hand),
    /// on the side set by hand if one is.
    fn generate_single_tie_variations(
        &mut self,
        host: &mut dyn Host,
        ties: &TiesConfiguration,
    ) -> Run<Vec<Variation>> {
        let mut vars = Vec::new();

        let spec = self.specifications[0];
        let mut sz = self.details.single_tie_region_size;
        if spec.has_manual_position {
            sz = 1;
        }
        for i in 0..sz {
            for d in [LEFT, RIGHT] {
                if i == 0 && ties.ties[0].dir == d {
                    continue;
                }

                let p = ties.ties[0].position + i * d;

                if !spec.has_manual_dir || d == spec.manual_dir {
                    let mut var = Variation::default();
                    let key = self.get_configuration(
                        host,
                        p,
                        d,
                        spec.column_ranks,
                        !spec.has_manual_delta_y,
                    )?;
                    var.add_suggestion(0, key);
                    vars.push(var);
                }
            }
        }
        Ok(vars)
    }

    /// `generate_collision_variations`: where a tie's middle is no more
    /// than 0.25 above the one below's, each of the two flipped to its
    /// other side (unless set by hand) and the lowest moved down; else a
    /// tie on a dot's position moved out.
    fn generate_collision_variations(
        &mut self,
        host: &mut dyn Host,
        ties: &TiesConfiguration,
    ) -> Run<Vec<Variation>> {
        let center_distance_tolerance = 0.25;

        let mut vars = Vec::new();
        let mut last_center = 0.0;
        for i in 0..ties.len() {
            let details = self.details;
            let b = ties.ties[i].get_transformed_bezier(host, &details)?;

            let center = bezier::curve_point(&b, 0.5).1;

            if i != 0 {
                let (spec, prev) = (self.specifications[i], self.specifications[i - 1]);
                let (tie, prev_tie) = (&ties.ties[i], &ties.ties[i - 1]);
                if center <= last_center + center_distance_tolerance {
                    if !spec.has_manual_dir {
                        let mut var = Variation::default();
                        let key = self.get_configuration(
                            host,
                            spec.position - tie.dir,
                            -tie.dir,
                            tie.column_ranks,
                            !spec.has_manual_delta_y,
                        )?;
                        var.add_suggestion(i, key);

                        vars.push(var);
                    }

                    if !prev.has_manual_dir {
                        let mut var = Variation::default();
                        let key = self.get_configuration(
                            host,
                            prev.position - prev_tie.dir,
                            -prev_tie.dir,
                            prev.column_ranks,
                            !prev.has_manual_delta_y,
                        )?;
                        var.add_suggestion(i - 1, key);

                        vars.push(var);
                    }

                    if i == 1 && !prev.has_manual_position && prev_tie.dir == DOWN {
                        let mut var = Variation::default();
                        let key = self.get_configuration(
                            host,
                            prev.position - 1,
                            DOWN,
                            prev.column_ranks,
                            !prev.has_manual_delta_y,
                        )?;
                        var.add_suggestion(i - 1, key);
                        vars.push(var);
                    }
                    // LilyPond's `i == ties.size ()` case is never reached
                    // (i < size), and not ported.
                } else if self.dot_positions.contains(&tie.position) && !spec.has_manual_position {
                    let mut var = Variation::default();
                    let key = self.get_configuration(
                        host,
                        tie.position + tie.dir,
                        tie.dir,
                        tie.column_ranks,
                        !spec.has_manual_delta_y,
                    )?;
                    var.add_suggestion(i, key);
                    vars.push(var);
                }
            }

            last_center = center;
        }

        Ok(vars)
    }

    /// `set_manual_tie_configuration`: each pair of `manual_configs` the
    /// position (inexact moving it by hand) and the side of its tie, as
    /// far as there are ties.
    pub fn set_manual_tie_configuration(
        &mut self,
        host: &mut dyn Host,
        manual_configs: Value,
    ) -> Run<()> {
        let mut k = 0;
        let mut s = manual_configs;
        while let (Some(entry), Some(rest)) = (host.heap().car(s), host.heap().cdr(s)) {
            if k >= self.specifications.len() {
                break;
            }
            let heap = host.heap();
            if let (Some(pos), Some(dir)) = (heap.car(entry), heap.cdr(entry)) {
                let spec = &mut self.specifications[k];

                if heap.is_number(pos) {
                    spec.has_manual_position = true;
                    spec.manual_position = heap.to_f64(pos).unwrap_or(0.0);
                    /* TODO: check whether inexact? is an appropriate condition here */
                    spec.has_manual_delta_y = !heap.is_exact(pos).unwrap_or(true);
                }

                if heap.is_number(dir) {
                    spec.has_manual_dir = true;
                    spec.manual_dir = sign(stem::to_int(heap, dir)?);
                }
            }
            k += 1;
            s = rest;
        }
        Ok(())
    }

    /// `set_debug_scoring`: with `debug-tie-scoring`, each tie's complete
    /// card its `annotation`.
    pub fn set_debug_scoring(&self, host: &mut dyn Host, base: &TiesConfiguration) -> Run<()> {
        let debug = {
            let odef =
                grob_of(host.heap(), self.x_refpoint).map(|(arena, _)| arena.borrow().output_def());
            let sym = host.heap().intern("debug-tie-scoring");
            odef.and_then(|odef| host_lookup_variable(host, odef, sym)) == Some(Value::TRUE)
        };
        if debug {
            for i in 0..base.len() {
                let card = base.complete_tie_card(i);
                let heap = host.heap();
                let v = heap.string(&card);
                grob::set_property(heap, self.specifications[i].tie_grob, "annotation", v);
            }
        }
        Ok(())
    }
}
