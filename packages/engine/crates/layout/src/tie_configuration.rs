//! `tie-configuration.cc` (MS4 step 9): a tie's place and shape as the
//! formatting problem (`tie_formatting_problem.rs`) tries it,
//! [`Configuration`] (`Tie_configuration`, `:24–110`), and a column's
//! ties together with their score, [`TiesConfiguration`]
//! (`Ties_configuration`, `:112–190`); with what it reads,
//! `tie-specification.cc` (a tie as its grob asks, [`Specification`],
//! `:24–77`) and `tie-details.cc` (its `details`, [`Details`], `:31–101`,
//! step 10's in the split, which the problem reads first). Their
//! `distance`, `complete_score_card` (for GDB) and the problem's
//! `print_ties_configuration` (a `printf`) have no caller and are not
//! ported.
//!
//! LilyPond's clang fuses one sum here (otool): the transformed curve's
//! Y, an [`fma`].

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{Host, grob_of};
use lily_engine_scheme::{Exception, Value};

use crate::bezier;
use crate::geometry::{Direction, Interval, fma};
use crate::grob;
use crate::lookup::Bezier;
use crate::semi_tie;
use crate::spacing_options::double_or;
use crate::spanner;
use crate::staff_symbol_referencer as ssr;
use crate::stem::int_or;
use crate::tie;

type Run<T> = Result<T, Exception>;

const LEFT: Direction = -1;
const UP: Direction = 1;
const DOWN: Direction = -1;

/// `Tie_details`: the tie's `details`, its staff space and its neutral
/// direction (`Tie_details::from_grob`).
#[derive(Clone, Copy)]
pub(crate) struct Details {
    pub height_limit: f64,
    pub ratio: f64,
    pub staff_space: f64,
    pub x_gap: f64,
    pub stem_gap: f64,
    /// Read, as LilyPond's, by nothing.
    #[allow(dead_code)]
    pub between_length_limit: f64,
    pub wrong_direction_offset_penalty: f64,
    pub same_dir_as_stem_penalty: f64,
    pub min_length_penalty_factor: f64,
    pub min_length: f64,
    pub skyline_padding: f64,
    pub tip_staff_line_clearance: f64,
    pub center_staff_line_clearance: f64,
    pub staff_line_collision_penalty: f64,
    pub dot_collision_clearance: f64,
    pub dot_collision_penalty: f64,
    pub tie_column_monotonicity_penalty: f64,
    pub tie_tie_collision_penalty: f64,
    pub tie_tie_collision_distance: f64,
    pub horizontal_distance_penalty_factor: f64,
    pub vertical_distance_penalty_factor: f64,
    pub intra_space_threshold: f64,
    pub outer_tie_length_symmetry_penalty_factor: f64,
    pub outer_tie_vertical_distance_symmetry_penalty_factor: f64,
    pub outer_tie_vertical_gap: f64,
    pub staff_symbol_referencer: Value,
    pub single_tie_region_size: i32,
    pub multi_tie_region_size: i32,
    pub neutral_direction: Direction,
}

impl Default for Details {
    /// `Tie_details ()`: what it sets; the rest, which LilyPond leaves
    /// unset, 0 (`from_grob` sets every one before it is read).
    fn default() -> Self {
        Details {
            height_limit: 1.0,
            ratio: 0.333,
            staff_space: 1.0,
            x_gap: 0.0,
            stem_gap: 0.0,
            between_length_limit: 0.0,
            wrong_direction_offset_penalty: 0.0,
            same_dir_as_stem_penalty: 0.0,
            min_length_penalty_factor: 0.0,
            min_length: 0.0,
            skyline_padding: 0.0,
            tip_staff_line_clearance: 0.0,
            center_staff_line_clearance: 0.0,
            staff_line_collision_penalty: 0.0,
            dot_collision_clearance: 0.0,
            dot_collision_penalty: 0.0,
            tie_column_monotonicity_penalty: 0.0,
            tie_tie_collision_penalty: 0.0,
            tie_tie_collision_distance: 0.0,
            horizontal_distance_penalty_factor: 0.0,
            vertical_distance_penalty_factor: 0.0,
            intra_space_threshold: 0.0,
            outer_tie_length_symmetry_penalty_factor: 0.0,
            outer_tie_vertical_distance_symmetry_penalty_factor: 0.0,
            outer_tie_vertical_gap: 0.0,
            staff_symbol_referencer: Value::NULL,
            single_tie_region_size: 0,
            multi_tie_region_size: 0,
            neutral_direction: 0,
        }
    }
}

impl Details {
    /// `from_grob`: `me`'s staff space, `neutral-direction` (down for
    /// none) and `details`, each a real (an exact integer for the region
    /// sizes) or its default.
    pub fn read_grob(&mut self, host: &mut dyn Host, me: Value) -> Run<()> {
        self.staff_symbol_referencer = me;
        self.staff_space = ssr::staff_space(host, me)?;

        let nd = grob::property(host, me, "neutral-direction")?;
        self.neutral_direction = crate::dynamic_align_engraver::direction(nd);
        if self.neutral_direction == 0 {
            self.neutral_direction = DOWN;
        }

        let details = grob::property(host, me, "details")?;
        let detail = |host: &mut dyn Host, name: &str| {
            let sym = host.heap().intern(name);
            grob::assq_ref(host.heap(), sym, details).unwrap_or(Value::NULL)
        };
        let real = |host: &mut dyn Host, name: &str, fallback: f64| {
            let v = detail(host, name);
            double_or(host, v, fallback)
        };
        let int = |host: &mut dyn Host, name: &str, fallback: i32| {
            let v = detail(host, name);
            int_or(host.heap(), v, fallback)
        };

        self.height_limit = real(host, "height-limit", 0.75);
        self.ratio = real(host, "ratio", 0.333);
        self.between_length_limit = real(host, "between-length-limit", 1.0);
        self.wrong_direction_offset_penalty = real(host, "wrong-direction-offset-penalty", 10.0);
        self.min_length = real(host, "min-length", 1.0);
        self.min_length_penalty_factor = real(host, "min-length-penalty-factor", 1.0);

        // in half-space
        self.center_staff_line_clearance = real(host, "center-staff-line-clearance", 0.4);
        self.tip_staff_line_clearance = real(host, "tip-staff-line-clearance", 0.4);
        self.staff_line_collision_penalty = real(host, "staff-line-collision-penalty", 5.0);
        self.dot_collision_clearance = real(host, "dot-collision-clearance", 0.25);
        self.dot_collision_penalty = real(host, "dot-collision-penalty", 0.25);
        self.x_gap = real(host, "note-head-gap", 0.2);
        self.stem_gap = real(host, "stem-gap", 0.3);
        self.tie_column_monotonicity_penalty = real(host, "tie-column-monotonicity-penalty", 100.0);
        self.tie_tie_collision_penalty = real(host, "tie-tie-collision-penalty", 30.0);
        self.tie_tie_collision_distance = real(host, "tie-tie-collision-distance", 0.25);
        self.horizontal_distance_penalty_factor =
            real(host, "horizontal-distance-penalty-factor", 5.0);
        self.same_dir_as_stem_penalty = real(host, "same-dir-as-stem-penalty", 20.0);
        self.vertical_distance_penalty_factor = real(host, "vertical-distance-penalty-factor", 5.0);
        self.intra_space_threshold = real(host, "intra-space-threshold", 1.0);
        self.outer_tie_length_symmetry_penalty_factor =
            real(host, "outer-tie-length-symmetry-penalty-factor", 3.0);
        self.outer_tie_vertical_distance_symmetry_penalty_factor = real(
            host,
            "outer-tie-vertical-distance-symmetry-penalty-factor",
            3.0,
        );

        self.outer_tie_vertical_gap = real(host, "outer-tie-vertical-gap", 0.15);

        self.single_tie_region_size = int(host, "single-tie-region-size", 3);
        self.skyline_padding = real(host, "skyline-padding", 0.05);
        self.multi_tie_region_size = int(host, "multi-tie-region-size", 1);
        Ok(())
    }
}

/// `Tie_configuration`: a tie at a staff position, on a side, moved by
/// `delta_y`, between two columns; its X ends and its score once scored.
#[derive(Clone, Debug)]
pub(crate) struct Configuration {
    score_card: String,
    score: f64,
    scored: bool,
    pub position: i32,
    pub dir: Direction,
    pub delta_y: f64,
    pub column_ranks: [i32; 2],
    pub attachment_x: Interval,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            score_card: String::new(),
            score: 0.0,
            scored: false,
            position: 0,
            dir: 0,
            delta_y: 0.0,
            column_ranks: [0; 2],
            attachment_x: Interval::EMPTY,
        }
    }
}

impl Configuration {
    pub fn score(&self) -> f64 {
        self.score
    }

    pub fn card(&self) -> &str {
        &self.score_card
    }

    pub fn is_scored(&self) -> bool {
        self.scored
    }

    pub fn set_scored(&mut self) {
        self.scored = true;
    }

    /// `center_tie_vertically`: moved by the middle of its edge's and
    /// its middle's heights, against its side.
    pub fn center_tie_vertically(&mut self, host: &mut dyn Host, details: &Details) -> Run<()> {
        let b = self.get_untransformed_bezier(host, details)?;
        let middle = bezier::curve_point(&b, 0.5);
        let edge = bezier::curve_point(&b, 0.0);
        let center = (edge.1 + middle.1) / 2.0;

        self.delta_y = f64::from(-self.dir) * center;
        Ok(())
    }

    /// `get_transformed_bezier`: the curve turned to its side and put at
    /// its left end and its staff position, moved by `delta_y`.
    pub fn get_transformed_bezier(&self, host: &mut dyn Host, details: &Details) -> Run<Bezier> {
        let mut b = self.get_untransformed_bezier(host, details)?;

        bezier::scale(&mut b, 1.0, f64::from(self.dir));
        bezier::translate(
            &mut b,
            (
                self.attachment_x.at(LEFT),
                fma(
                    details.staff_space * 0.5,
                    f64::from(self.position),
                    self.delta_y,
                ),
            ),
        );
        Ok(b)
    }

    /// `get_untransformed_bezier`: the curve from (0, 0) as long as the
    /// attachments are apart, 1 long for what is not finite (LilyPond's
    /// programming error).
    pub fn get_untransformed_bezier(&self, host: &mut dyn Host, details: &Details) -> Run<Bezier> {
        let mut l = self.attachment_x.length();
        if !l.is_finite() {
            grob::unlocated_programming_error(host, "Inf or NaN encountered")?;
            l = 1.0;
        }
        Ok(bezier::slur_shape(l, details.height_limit, details.ratio))
    }

    /// `column_span_length`.
    pub fn column_span_length(&self) -> i32 {
        self.column_ranks[1] - self.column_ranks[0]
    }

    /// `add_score`: `s` added, and to the card unless 0.
    pub fn add_score(&mut self, s: f64, desc: &str) {
        debug_assert!(!self.scored);
        self.score += s;
        if s != 0.0 {
            self.score_card.push_str(&format!("{desc}={s:.2} "));
        }
    }

    /// `height`: the curve's height at its middle.
    pub fn height(&self, details: &Details) -> f64 {
        let l = self.attachment_x.length();

        let b = bezier::slur_shape(l, details.height_limit, details.ratio);
        bezier::curve_point(&b, 0.5).1
    }
}

/// `Ties_configuration`: a column's ties, and their score with its card
/// and each tie's.
#[derive(Clone, Debug, Default)]
pub(crate) struct TiesConfiguration {
    pub ties: Vec<Configuration>,
    score: f64,
    score_card: String,
    scored: bool,
    tie_score_cards: Vec<String>,
}

impl TiesConfiguration {
    pub fn len(&self) -> usize {
        self.ties.len()
    }

    pub fn is_scored(&self) -> bool {
        self.scored
    }

    pub fn set_scored(&mut self) {
        self.scored = true;
    }

    pub fn score(&self) -> f64 {
        self.score
    }

    /// `reset_score`.
    pub fn reset_score(&mut self) {
        self.score = 0.0;
        self.scored = false;
        self.score_card.clear();
        self.tie_score_cards.clear();
    }

    /// `add_tie_score`: `s` added, and to tie `i`'s card unless 0.
    pub fn add_tie_score(&mut self, s: f64, i: usize, desc: &str) {
        debug_assert!(!self.scored);
        self.score += s;
        if s != 0.0 {
            while self.tie_score_cards.len() < self.ties.len() {
                self.tie_score_cards.push(String::new());
            }
            self.tie_score_cards[i].push_str(&format!("{desc}={s:.2} "));
        }
    }

    /// `add_score`: `s` added, and to the card unless 0.
    pub fn add_score(&mut self, s: f64, desc: &str) {
        debug_assert!(!self.scored);
        self.score += s;
        if s != 0.0 {
            self.score_card.push_str(&format!("{desc}={s:.2} "));
        }
    }

    /// `complete_tie_card`: tie `i`'s position, offset and side, its
    /// configuration's card and its own; the column's card on the first,
    /// the total on the last. A tie with no card of its own has none
    /// (LilyPond reads past its vector).
    pub fn complete_tie_card(&self, i: usize) -> String {
        let t = &self.ties[i];
        let side = if t.dir == UP { 'u' } else { 'd' };
        let mut s = format!("{} ({:.2}) {side}: ", t.position, t.delta_y);
        s.push_str(t.card());
        if let Some(c) = self.tie_score_cards.get(i) {
            s.push_str(c);
        }

        /*
         this is a little awkward, but we must decide where to put
         aggregrates.
        */
        if i == 0 {
            s.push_str(&self.score_card);
        }

        if i + 1 == self.ties.len() {
            s.push_str(&format!("TOTAL={:.2}", self.score));
        }
        s
    }
}

/// `Tie_specification`: a tie as its grob asks it: its staff position,
/// heads and columns, and what is set by hand.
#[derive(Clone, Copy)]
pub(crate) struct Specification {
    pub position: i32,
    pub note_head_drul: [Option<Value>; 2],
    pub column_ranks: [i32; 2],
    pub tie_grob: Value,
    pub has_manual_position: bool,
    pub has_manual_dir: bool,
    pub has_manual_delta_y: bool,
    pub manual_position: f64,
    pub manual_dir: Direction,
}

impl Default for Specification {
    fn default() -> Self {
        Specification {
            position: 0,
            note_head_drul: [None; 2],
            column_ranks: [0; 2],
            tie_grob: Value::NULL,
            has_manual_position: false,
            has_manual_dir: false,
            has_manual_delta_y: false,
            manual_position: 0.0,
            manual_dir: 0,
        }
    }
}

impl Specification {
    /// `from_grob`: a `direction` set as a number, the tie's (or
    /// semi-tie's) staff position, and its `staff-position` if a number,
    /// one not exact moving it by hand.
    pub fn read_grob(&mut self, host: &mut dyn Host, tie: Value) -> Run<()> {
        // In this method, Tie and Semi_tie require the same logic with different
        // types.  It might be clearer to use a template.
        self.tie_grob = tie;
        let d = property_data(host, tie, "direction");
        if host.heap().is_number(d) {
            let d = grob::property(host, tie, "direction")?;
            self.manual_dir = crate::dynamic_align_engraver::direction(d);
            self.has_manual_dir = true;
        }

        self.position = if spanner::is_spanner(host.heap(), tie) {
            tie::get_position(host, tie)?
        } else {
            semi_tie::get_position(host, tie)?
        };

        let pos_scm = grob::property(host, tie, "staff-position")?;
        if host.heap().is_number(pos_scm) {
            // `is_scm<Rational>`: exact, or infinite.
            let heap = host.heap();
            let exact = heap.is_exact(pos_scm).unwrap_or(false)
                || heap.to_f64(pos_scm).is_some_and(f64::is_infinite);
            self.has_manual_delta_y = !exact;
            self.manual_position = double_or(host, pos_scm, 0.0);
            self.has_manual_position = true;
        }
        Ok(())
    }

    /// `column_span`.
    pub fn column_span(&self) -> i32 {
        self.column_ranks[1] - self.column_ranks[0]
    }
}

/// `get_property_data (g, name)`: what is set for `name` on the grob,
/// no callback called.
pub(crate) fn property_data(host: &mut dyn Host, g: Value, name: &str) -> Value {
    let heap = host.heap();
    let sym = heap.intern(name);
    match grob_of(heap, g) {
        Some((arena, id)) => grob::property_data(heap, &arena, id, g, sym),
        None => Value::NULL,
    }
}
