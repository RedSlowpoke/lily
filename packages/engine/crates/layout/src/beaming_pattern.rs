//! `beaming-pattern.cc` (MS4 step 2): `Beaming_options`
//! (`beaming-pattern.cc:574–594`), what a beam reads of its context when
//! it starts, and `Beaming_pattern` (`:31–571`), the beamlets of each stem
//! of a beam, which `Beam_engraver` and `Auto_beam_engraver` give
//! `Beam::set_beaming` (`beam_engraver.rs`, `auto_beam_engraver.rs`): the
//! invisible stems unbeamed, each stem's rhythmic importance in its beat
//! and tuplet spans (`Span_position`), the flags pointing to a beat, the
//! subdivisions, and the beamlets kept inside a tuplet's span.
//!
//! A stem's tuplet is `currentTupletDescription` when it is added, a
//! [`TupletDescription`] (tuplet-description.cc's `tuplet_start` and
//! `tuplet_stop`, borrowed), which `Tuplet_engraver` sets (MS4 step 12):
//! until then none is. A tuplet is known by its object, as LilyPond's by
//! its address; its chain of parents is read when its stem is added, as
//! `Tuplet_engraver` has set it by then.
//!
//! Its rationals are LilyPond's ([`Rational`]): where LilyPond's would
//! wrap, a result is NaN. `intlog2` of a number below 1, where LilyPond
//! exits, throws its message.

use alloc::format;
use alloc::vec::Vec;

use lily_engine_music::{Duration, Moment, Rational, Tx};
use lily_engine_scheme::{Exception, Foreign, Heap, Tracer, Value};

use crate::geometry::Direction;

type Run<T> = Result<T, Exception>;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

/// `a + b`, NaN where LilyPond's would wrap.
fn add(a: Rational, b: Rational) -> Rational {
    a.checked_add(b).unwrap_or(Rational::NAN)
}

/// `a - b`.
fn sub(a: Rational, b: Rational) -> Rational {
    a.checked_sub(b).unwrap_or(Rational::NAN)
}

/// `a * b`.
fn mul(a: Rational, b: Rational) -> Rational {
    a.checked_mul(b).unwrap_or(Rational::NAN)
}

/// `a / b`.
fn div(a: Rational, b: Rational) -> Rational {
    a.checked_div(b).unwrap_or(Rational::NAN)
}

fn lt(a: Rational, b: Rational) -> bool {
    a.compare(b).is_lt()
}

fn le(a: Rational, b: Rational) -> bool {
    a.compare(b).is_le()
}

/// `intlog2`: the position of the highest bit; LilyPond exits for a `d`
/// below 1, where this throws its message.
fn intlog2(heap: &mut Heap, d: i64) -> Run<i32> {
    if d <= 0 {
        return Err(crate::misc_error(
            heap,
            "intlog2",
            &format!("intlog2 with negative argument: {d}"),
        ));
    }
    Ok(63 - d.leading_zeros() as i32)
}

/// A `Tuplet_description` (`tuplet-description.cc`): a tuplet's span and
/// ratio, and the tuplet it is inside, which `Tuplet_engraver` (MS4 step
/// 12) makes for each tuplet and sets as `currentTupletDescription`.
pub struct TupletDescription {
    /// `start_moment_` and `stop_moment_`.
    pub start: Moment,
    pub stop: Moment,
    pub numerator: u32,
    pub denominator: u32,
    /// `parent_`: the description it is inside, or `#f`.
    pub parent: Value,
}

impl TupletDescription {
    /// `tuplet_start`: in grace time if its start or stop is.
    pub fn tuplet_start(&self) -> Rational {
        if !self.start.grace.is_zero() || !self.stop.grace.is_zero() {
            self.start.grace
        } else {
            self.start.main
        }
    }

    /// `tuplet_stop`.
    pub fn tuplet_stop(&self) -> Rational {
        if !self.start.grace.is_zero() || !self.stop.grace.is_zero() {
            self.stop.grace
        } else {
            self.stop.main
        }
    }
}

impl Foreign for TupletDescription {
    fn type_name(&self) -> &'static str {
        "Tuplet_description"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.trace(self.parent);
    }
}

/// A stem's tuplet as the pattern reads it: the description's object, its
/// span, its ratio and the index of its parent in the pattern's tuplets.
#[derive(Clone)]
struct Tuplet {
    value: Value,
    start: Rational,
    stop: Rational,
    numerator: u32,
    denominator: u32,
    parent: Option<usize>,
}

/// `Beaming_options`: what a beam reads of its context when it starts.
#[derive(Clone, Copy)]
pub struct BeamingOptions {
    pub subdivide_beams: bool,
    pub strict_beat_beaming: bool,
    pub respect_incomplete_beams: bool,
    /// `beatStructure`, a list of numbers.
    pub beat_structure: Value,
    pub beat_base: Rational,
    /// `period_`: the length of the beat structure in whole notes, or of
    /// the measure if it has none.
    pub period: Rational,
    pub minimum_subdivision_interval: Rational,
    pub maximum_subdivision_interval: Rational,
}

impl Default for BeamingOptions {
    fn default() -> BeamingOptions {
        BeamingOptions {
            subdivide_beams: false,
            strict_beat_beaming: false,
            respect_incomplete_beams: false,
            beat_structure: Value::NULL,
            beat_base: Rational::new(1, 4),
            period: Rational::int(1),
            minimum_subdivision_interval: Rational::ZERO,
            maximum_subdivision_interval: Rational::INFINITY,
        }
    }
}

impl BeamingOptions {
    /// What it holds in Rust, for N8's state digest (D240).
    pub fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[
            self.subdivide_beams,
            self.strict_beat_beaming,
            self.respect_incomplete_beams,
        ]);
        for r in [
            self.beat_base,
            self.period,
            self.minimum_subdivision_interval,
            self.maximum_subdivision_interval,
        ] {
            h.rational(r);
        }
    }

    /// `Beaming_options (Context const *)`: from the translator's context.
    pub fn new(tx: &mut Tx<'_>) -> BeamingOptions {
        let rational = crate::paper_column_engraver::rational;
        let mut flag = |name: &str| tx.get(name) == Value::TRUE;
        let (subdivide_beams, strict_beat_beaming, respect_incomplete_beams) = (
            flag("subdivideBeams"),
            flag("strictBeatBeaming"),
            flag("respectIncompleteBeams"),
        );
        let beat_structure = tx.get("beatStructure");
        let v = tx.get("beatBase");
        let beat_base = rational(tx.heap(), v, Rational::new(1, 4));
        let v = tx.get("measureLength");
        let measure_length = rational(tx.heap(), v, Rational::int(1));
        let period = Self::calc_period(tx.heap(), beat_structure, beat_base, measure_length);
        let v = tx.get("beamMinimumSubdivision");
        let minimum_subdivision_interval = rational(tx.heap(), v, Rational::ZERO);
        let v = tx.get("beamMaximumSubdivision");
        let maximum_subdivision_interval = rational(tx.heap(), v, Rational::INFINITY);
        BeamingOptions {
            subdivide_beams,
            strict_beat_beaming,
            respect_incomplete_beams,
            beat_structure,
            beat_base,
            period,
            minimum_subdivision_interval,
            maximum_subdivision_interval,
        }
    }

    /// `calc_period`: the beat base times the beats' sum, or
    /// `measure_length` if that is 0 (a senza-misura passage has no beat
    /// structure).
    fn calc_period(
        heap: &Heap,
        beat_structure: Value,
        beat_base: Rational,
        measure_length: Rational,
    ) -> Rational {
        let mut total = Rational::ZERO;
        let mut p = beat_structure;
        while let (Some(beat), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            total = add(
                total,
                crate::paper_column_engraver::rational(heap, beat, Rational::ZERO),
            );
            p = rest;
        }
        let period = mul(beat_base, total);
        if period.is_zero() {
            measure_length
        } else {
            period
        }
    }
}

/// `Beam_rhythmic_element`: a stem of the pattern.
#[derive(Clone)]
struct Element {
    start_moment: Rational,
    beam_count: u32,
    /// `beam_count_drul_`: its beamlets on each side.
    beam_count_drul: [u32; 2],
    rhythmic_importance: i32,
    /// Rests are invisible.
    invisible: bool,
    duration: Duration,
    /// Its tuplet, an index into the pattern's tuplets.
    tuplet: Option<usize>,
}

/// The index of `d` in a `Drul_array`.
fn ix(d: Direction) -> usize {
    usize::from(d > 0)
}

impl Element {
    fn new(m: Rational, inv: bool, duration: Duration, tuplet: Option<usize>) -> Element {
        let beam_count = (duration.log() - 2).max(0) as u32;
        Element {
            start_moment: m,
            beam_count,
            beam_count_drul: [beam_count; 2],
            rhythmic_importance: 0,
            invisible: inv,
            duration,
            tuplet,
        }
    }

    fn count(&self) -> u32 {
        self.beam_count
    }
}

/// `rhythmic_importance_for_position`.
fn rhythmic_importance_for_position(heap: &mut Heap, r: Rational) -> Run<i32> {
    let from_num = if r.denominator() == 1 {
        intlog2(heap, r.numerator())?
    } else {
        0
    };
    Ok(intlog2(heap, r.denominator())? - 2 - from_num)
}

/// `rhythmic_importance_for_length`.
fn rhythmic_importance_for_length(heap: &mut Heap, r: Rational) -> Run<i32> {
    Ok(intlog2(heap, r.denominator())? - 2 - intlog2(heap, r.numerator())?)
}

/// `Beaming_pattern`.
#[derive(Clone)]
pub struct BeamingPattern {
    /// `measure_offset_`: the measure position of the first stem.
    pub measure_offset: Rational,
    infos: Vec<Element>,
    tuplets: Vec<Tuplet>,
}

/// `Span_position`: the beats of a span, the whole beam's or a tuplet's.
struct SpanPosition {
    beat_base: Rational,
    beat_length: u32,
    current_moment: Rational,
    next_moment: Rational,
    moment_num: i32,
    end_moment: Rational,
    /// Its tuplet, `None` for the root.
    tuplet: Option<usize>,
}

impl SpanPosition {
    /// `Span_position (Tuplet_description const &)`: a sextuplet's beat
    /// length is 3.
    fn of_tuplet(t: &Tuplet, i: usize) -> SpanPosition {
        let den = t.denominator;
        SpanPosition {
            beat_base: div(sub(t.stop, t.start), Rational::int(i64::from(den))),
            // The last set bit of the denominator.
            beat_length: den.checked_div(den & den.wrapping_neg()).unwrap_or(0),
            current_moment: t.start,
            next_moment: t.start,
            moment_num: -1,
            end_moment: t.stop,
            tuplet: Some(i),
        }
    }

    /// `update`: called before each stem to align the moments.
    fn update(&mut self, pos: Rational) {
        while le(self.next_moment, pos) {
            self.current_moment = self.next_moment;
            self.next_moment = add(self.next_moment, self.beat_base);
            self.moment_num += 1;
        }
    }

    /// `beat_level`: 0 for an incomplete beat or the start of a span.
    fn beat_level(&self, heap: &mut Heap) -> Run<u32> {
        if self.moment_num == 0
            || self.beat_length == 0
            || !(self.moment_num as u32).is_multiple_of(self.beat_length)
        {
            return Ok(0);
        }
        let beat_num = self.moment_num / self.beat_length as i32;
        Ok((intlog2(heap, i64::from(beat_num & beat_num.wrapping_neg()))? + 1) as u32)
    }
}

impl BeamingPattern {
    /// What it holds in Rust, for N8's state digest (D240).
    pub fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.rational(self.measure_offset);
        h.num(self.infos.len() as i64);
        for e in &self.infos {
            h.rational(e.start_moment);
            for n in [e.beam_count, e.beam_count_drul[0], e.beam_count_drul[1]] {
                h.num(n.into());
            }
            h.num(e.rhythmic_importance.into());
            h.flags(&[e.invisible]);
            h.num(e.duration.log().into());
            h.num(e.duration.dots().into());
            h.rational(e.duration.factor());
            h.num(e.tuplet.map_or(-1, |t| t as i64));
        }
        h.num(self.tuplets.len() as i64);
        for t in &self.tuplets {
            h.rational(t.start);
            h.rational(t.stop);
            h.num(t.numerator.into());
            h.num(t.denominator.into());
            h.num(t.parent.map_or(-1, |p| p as i64));
        }
    }

    /// `Beaming_pattern (Rational const &)`; LilyPond's programming error
    /// for a negative offset is not written.
    pub fn new(measure_offset: Rational) -> BeamingPattern {
        BeamingPattern {
            measure_offset,
            infos: Vec::new(),
            tuplets: Vec::new(),
        }
    }

    /// The Scheme values it holds: its tuplets' descriptions.
    pub fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.tuplets.iter().map(|t| t.value));
    }

    /// The index of the tuplet `v` (a [`TupletDescription`], else none)
    /// and of its parents, added if they are not there.
    fn tuplet(&mut self, heap: &Heap, v: Value) -> Option<usize> {
        if let Some(i) = self.tuplets.iter().position(|t| t.value == v) {
            return Some(i);
        }
        let t = heap.foreign::<TupletDescription>(v)?;
        let (start, stop, numerator, denominator, parent) = (
            t.tuplet_start(),
            t.tuplet_stop(),
            t.numerator,
            t.denominator,
            t.parent,
        );
        let parent = self.tuplet(heap, parent);
        self.tuplets.push(Tuplet {
            value: v,
            start,
            stop,
            numerator,
            denominator,
            parent,
        });
        Some(self.tuplets.len() - 1)
    }

    /// `add_stem`: a stem at `m`, invisible for a rest, of `duration`, in
    /// the tuplet `tuplet` (`currentTupletDescription`). LilyPond's
    /// programming error for a stem not after the last is not written.
    pub fn add_stem(
        &mut self,
        heap: &Heap,
        m: Rational,
        inv: bool,
        duration: Duration,
        tuplet: Value,
    ) {
        let tuplet = self.tuplet(heap, tuplet);
        self.infos.push(Element::new(m, inv, duration, tuplet));
    }

    /// `beamlet_count`: of the stem `i` on the side `d`.
    pub fn beamlet_count(&self, i: usize, d: Direction) -> u32 {
        self.infos[i].beam_count_drul[ix(d)]
    }

    pub fn start_moment(&self, i: usize) -> Rational {
        self.infos[i].start_moment
    }

    pub fn end_moment(&self, i: usize) -> Rational {
        let e = &self.infos[i];
        add(e.start_moment, e.duration.length().unwrap_or(Rational::NAN))
    }

    /// The span of the stem `i`'s tuplet, if it has one.
    fn tuplet_span(&self, i: usize) -> Option<(Rational, Rational)> {
        let t = &self.tuplets[self.infos[i].tuplet?];
        Some((t.start, t.stop))
    }

    /// `at_span_start`: whether the stem `i` starts its tuplet's span, or
    /// the beam.
    fn at_span_start(&self, i: usize) -> bool {
        let first = self.start_moment(0);
        let start = match self.tuplet_span(i) {
            Some((start, _)) if lt(first, start) => start,
            _ => first,
        };
        self.start_moment(i) == start
    }

    /// `at_span_stop`.
    fn at_span_stop(&self, i: usize) -> bool {
        let last = self.end_moment(self.infos.len() - 1);
        let stop = match self.tuplet_span(i) {
            Some((_, stop)) if lt(stop, last) => stop,
            _ => last,
        };
        self.end_moment(i) == stop
    }

    /// `beamify`: the beamlets of each stem.
    pub fn beamify(&mut self, heap: &mut Heap, options: &BeamingOptions) -> Run<()> {
        if self.infos.len() <= 1 {
            return Ok(());
        }

        self.unbeam_invisible_stems();

        self.set_rhythmic_importance(heap, options)?;

        {
            let n = self.infos.len();
            let mut flag_directions: Vec<Direction> = alloc::vec![0; n];
            let mut cur_beat = Rational::ZERO;
            let mut next_beat = sub(self.infos[0].start_moment, self.measure_offset);
            let mut remaining_beats = Value::NULL;

            #[allow(clippy::needless_range_loop)]
            for i in 1..n - 1 {
                // If we ever allow setting custom stem flag directions that
                // automatic beam subdivision would obey, here would be the
                // place to set the value and 'continue'
                let left_count = self.infos[i - 1].count();
                let right_count = self.infos[i + 1].count();

                // Stems at boundaries of tuplet spans must have CENTER
                // direction. That's why we don't iterate 0 and
                // infos_.size() - 1 as an optimization.
                if !self.at_span_start(i)
                    && !self.at_span_stop(i)
                    && self.infos[i].count() > left_count.min(right_count)
                {
                    while le(next_beat, self.infos[i].start_moment) {
                        if remaining_beats == Value::NULL {
                            remaining_beats = options.beat_structure;
                        }

                        cur_beat = next_beat;
                        let (Some(beat), Some(rest)) =
                            (heap.car(remaining_beats), heap.cdr(remaining_beats))
                        else {
                            return Err(crate::wrong_type(
                                heap,
                                Some("car"),
                                1,
                                remaining_beats,
                                "pair",
                            ));
                        };
                        let beats = match heap.to_i64(beat) {
                            Some(b)
                                if heap.is_exact_integer(beat)
                                    && (0..=0xffff_ffff).contains(&b) =>
                            {
                                b
                            }
                            _ => {
                                return Err(crate::wrong_type(
                                    heap,
                                    None,
                                    0,
                                    beat,
                                    "unsigned integer",
                                ));
                            }
                        };
                        next_beat = add(next_beat, mul(Rational::int(beats), options.beat_base));
                        remaining_beats = rest;
                    }

                    let point_right = if !options.strict_beat_beaming && left_count != right_count {
                        right_count > left_count
                    } else if (self.infos[i].start_moment == cur_beat)
                        != (self.end_moment(i) == next_beat)
                    {
                        self.infos[i].start_moment == cur_beat
                    } else {
                        self.infos[i].rhythmic_importance < self.infos[i + 1].rhythmic_importance
                    };

                    flag_directions[i] = if point_right { RIGHT } else { LEFT };
                }
            }

            // Correct flag directions for subdivision
            for i in 1..n - 1 {
                if flag_directions[i] == 0 && flag_directions[i - 1] == LEFT {
                    flag_directions[i] = RIGHT;
                }
                if flag_directions[i] == 0 && flag_directions[i + 1] == RIGHT {
                    flag_directions[i] = LEFT;
                }
            }

            for (i, &flag) in flag_directions.iter().enumerate().take(n - 1).skip(1) {
                if flag != 0 {
                    // beamlet count in flag_directions[i] should be
                    // preserved which is why we reference the neighbor of
                    // opposite direction
                    let opposite_dir = -flag;
                    let neighbor_ind = if opposite_dir > 0 { i + 1 } else { i - 1 };
                    // if the neighbor has higher beamlet count, then the
                    // neighbor should be the one chipping their beamlet
                    // count
                    let (count, neighbor) =
                        (self.infos[i].count(), self.infos[neighbor_ind].count());
                    if count >= neighbor {
                        let side = &mut self.infos[i].beam_count_drul[ix(opposite_dir)];
                        *side = side.wrapping_sub((count - neighbor).max(1));
                    }
                }
            }
        }

        if options.subdivide_beams
            && options.maximum_subdivision_interval.numerator() != 0
            && options.minimum_subdivision_interval.is_finite()
        {
            self.subdivide_beams(heap, options)?;
        }

        // stems at boundaries of tuplets should not have beamlets sticking
        // out of the tuplet range
        for i in 1..self.infos.len() - 1 {
            if self.at_span_start(i) {
                self.infos[i].beam_count_drul[ix(LEFT)] = self
                    .beamlet_count(i, LEFT)
                    .min(self.beamlet_count(i - 1, RIGHT));
            } else if self.at_span_stop(i) {
                self.infos[i].beam_count_drul[ix(RIGHT)] = self
                    .beamlet_count(i, RIGHT)
                    .min(self.beamlet_count(i + 1, LEFT));
            }
        }
        Ok(())
    }

    /// `set_rhythmic_importance`.
    fn set_rhythmic_importance(&mut self, heap: &mut Heap, options: &BeamingOptions) -> Run<()> {
        let n = self.infos.len();
        // span_contexts will always be non empty since there always exists
        // the root span; its front is the vector's last.
        let mut span_contexts = alloc::vec![SpanPosition {
            beat_base: options.period,
            beat_length: 1,
            current_moment: sub(self.infos[0].start_moment, self.measure_offset),
            next_moment: sub(self.infos[0].start_moment, self.measure_offset),
            moment_num: -1,
            end_moment: self.end_moment(n - 1),
            tuplet: None,
        }];
        // infos_[i].duration_.factor_ is not sufficient for calculations
        // for certain cases, so we must manually alter the current factor
        // based on incoming and outgoing tuplet spans
        let mut current_factor = Rational::int(1);

        for i in 0..n {
            let stem_pos = self.infos[i].start_moment;

            // Delete expired tuplet spans
            while let Some(t) = span_contexts.last().and_then(|s| s.tuplet) {
                let cur_tuplet = &self.tuplets[t];
                if lt(stem_pos, cur_tuplet.stop) {
                    break;
                }
                // Undo the expired tuplet span factor to the current factor
                current_factor = mul(
                    div(
                        current_factor,
                        Rational::int(i64::from(cur_tuplet.numerator)),
                    ),
                    Rational::int(i64::from(cur_tuplet.denominator)),
                );
                span_contexts.pop();
            }

            // Insert tuplet spans that are not already added, each after
            // the last one inserted (below the front's).
            {
                let front = span_contexts.len();
                let current_parent = span_contexts.last().and_then(|s| s.tuplet);
                let mut tuplet_it = self.infos[i].tuplet;
                while tuplet_it != current_parent {
                    let Some(t) = tuplet_it else { break };
                    let tuplet = &self.tuplets[t];
                    if lt(tuplet.start, stem_pos) {
                        span_contexts.insert(front, SpanPosition::of_tuplet(tuplet, t));
                        current_factor = div(
                            mul(current_factor, Rational::int(i64::from(tuplet.numerator))),
                            Rational::int(i64::from(tuplet.denominator)),
                        );
                    }
                    // Rhythmic importance of start of tuplet span should be
                    // set from parent context. If the first stem is part of
                    // a tuplet span, it is not necessarily the first note of
                    // the tuplet span, whereas a subsequent stem that has a
                    // different tuplet span is guaranteed to be the start of
                    // said tuplet span and may break the loop as an
                    // optimization
                    else if i > 0 {
                        break;
                    }

                    tuplet_it = tuplet.parent;
                }
            }

            // the appropriate Span_position for current/next moment
            let Some(cur_position) = span_contexts.last_mut() else {
                break;
            };
            cur_position.update(stem_pos);
            let (current, next, end) = (
                cur_position.current_moment,
                cur_position.next_moment,
                cur_position.end_moment,
            );

            // Notice that if the current stem introduces new tuplets, those
            // tuplets' factors aren't used until the next stem.
            // rhythmic_importance_ of stems at start of a tuplet span is
            // irrelevant to the tuplet span, but is needed for the parent
            // span as if the stem represents the whole child tuplet
            if stem_pos == current {
                let level = cur_position.beat_level(heap)?;
                self.infos[i].rhythmic_importance =
                    rhythmic_importance_for_length(heap, div(sub(next, current), current_factor))?
                        - level as i32;
            } else {
                let moment_relative_to_beat = div(sub(stem_pos, current), current_factor);

                // We must account for numerator of
                // maximum_subdivision_interval which may be greater than 1.
                // Setting it to beamlet count when moment_relative_to_beat
                // numerator isn't divisible basically means don't
                // subdivide here, even though we technically can in the
                // case of the numerator being a power of 2
                let max = options.maximum_subdivision_interval;
                if max.is_finite()
                    && max.numerator() != 0
                    && moment_relative_to_beat.numerator() % max.numerator() != 0
                {
                    self.infos[i].rhythmic_importance = self.infos[i].duration.log() - 2;
                } else {
                    let mut importance =
                        rhythmic_importance_for_position(heap, moment_relative_to_beat)?;

                    // We must preserve the tuplet denominator subdivision
                    // structure as without this line, a sextuplet of 6
                    // equal-length notes would subdivide between the 2nd
                    // and 3rd notes
                    importance = importance.max(rhythmic_importance_for_length(
                        heap,
                        div(sub(next, stem_pos), current_factor),
                    )?);

                    // Account for the right side of the subdivision having
                    // incomplete length as that should make the
                    // rhythmic_importance_ value higher
                    if options.respect_incomplete_beams && lt(self.end_moment(i), end) {
                        importance = importance.max(rhythmic_importance_for_length(
                            heap,
                            div(sub(end, stem_pos), current_factor),
                        )?);
                    }
                    self.infos[i].rhythmic_importance = importance;
                }
            }
        }
        Ok(())
    }

    /// `subdivide_beams`.
    fn subdivide_beams(&mut self, heap: &mut Heap, options: &BeamingOptions) -> Run<()> {
        // if minimum beam subdivision interval is 0, don't bother with it
        let check_minimum_subdivision_count = options.maximum_subdivision_interval.is_finite();
        let check_maximum_subdivision_count = options.minimum_subdivision_interval.numerator() != 0;
        // meaning of min/max for beam count is opposite of min/max for
        // interval since we are taking the logarithm of the denominators
        // (basically negating the logarithm of the whole fraction)
        let mut minimum_subdivision_count = 0;
        let mut maximum_subdivision_count = 0;

        if check_minimum_subdivision_count {
            minimum_subdivision_count =
                rhythmic_importance_for_position(heap, options.maximum_subdivision_interval)?;
        }

        if check_maximum_subdivision_count {
            maximum_subdivision_count =
                rhythmic_importance_for_position(heap, options.minimum_subdivision_interval)?;

            if check_minimum_subdivision_count {
                maximum_subdivision_count +=
                    intlog2(heap, options.maximum_subdivision_interval.numerator())?
                        - intlog2(heap, options.minimum_subdivision_interval.numerator())?;
            }
        }

        if !check_minimum_subdivision_count || minimum_subdivision_count < 1 {
            minimum_subdivision_count = 1;
        }
        // beam counts will be set to at least
        // minimum_subdivision_beam_count_level, whereas
        // maximum_subdivision_beam_count_level is only used to compare
        // rhythmic importance
        for i in 1..self.infos.len() - 1 {
            let predicted_left_count = self.infos[i]
                .rhythmic_importance
                .max(minimum_subdivision_count) as u32;
            let predicted_right_count = self.infos[i + 1]
                .rhythmic_importance
                .max(minimum_subdivision_count) as u32;

            // we can only chip off one side
            if (!check_maximum_subdivision_count
                || self.infos[i].rhythmic_importance <= maximum_subdivision_count)
                && predicted_left_count < self.beamlet_count(i, LEFT)
                && self.beamlet_count(i, RIGHT) == self.infos[i].count()
            {
                self.infos[i].beam_count_drul[ix(LEFT)] = predicted_left_count;
            } else if (!check_maximum_subdivision_count
                || self.infos[i + 1].rhythmic_importance <= maximum_subdivision_count)
                && predicted_right_count < self.beamlet_count(i, RIGHT)
                && self.beamlet_count(i, LEFT) == self.infos[i].count()
            {
                self.infos[i].beam_count_drul[ix(RIGHT)] = predicted_right_count;
            }
        }
        Ok(())
    }

    /// `unbeam_invisible_stems`: invisible stems treated as though they had
    /// the beams of their least-beamed neighbour.
    fn unbeam_invisible_stems(&mut self) {
        for i in 1..self.infos.len() {
            if self.infos[i].invisible {
                let b = self.infos[i].count().min(self.infos[i - 1].count());
                let e = &mut self.infos[i];
                e.beam_count = b;
                e.beam_count_drul = [b; 2];
            }
        }

        for i in 0..self.infos.len().saturating_sub(1) {
            if self.infos[i].invisible {
                let b = self.infos[i].count().min(self.infos[i + 1].count());
                let e = &mut self.infos[i];
                e.beam_count = b;
                e.beam_count_drul = [b; 2];
            }
        }
    }

    /// `split_pattern`: the stems after `i` taken into a new pattern.
    pub fn split_pattern(&mut self, i: usize, measure_length: Rational) -> BeamingPattern {
        let offset = sub(
            self.end_moment(i),
            sub(self.infos[0].start_moment, self.measure_offset),
        );
        let mut new_pattern = BeamingPattern::new(mod_rat(offset, measure_length));
        let rest = self.infos.split_off(i + 1);
        for e in rest {
            let tuplet = e.tuplet.map(|t| self.tuplets[t].value);
            let tuplet = tuplet.and_then(|v| new_pattern.copy_tuplet(self, v));
            new_pattern.infos.push(Element::new(
                e.start_moment,
                e.invisible,
                e.duration,
                tuplet,
            ));
        }
        new_pattern
    }

    /// The tuplet `v` of `from` and its parents, added to its tuplets if
    /// they are not there.
    fn copy_tuplet(&mut self, from: &BeamingPattern, v: Value) -> Option<usize> {
        if let Some(i) = self.tuplets.iter().position(|t| t.value == v) {
            return Some(i);
        }
        let t = from.tuplets.iter().find(|t| t.value == v)?;
        let parent = t
            .parent
            .and_then(|p| self.copy_tuplet(from, from.tuplets[p].value));
        self.tuplets.push(Tuplet {
            value: t.value,
            start: t.start,
            stop: t.stop,
            numerator: t.numerator,
            denominator: t.denominator,
            parent,
        });
        Some(self.tuplets.len() - 1)
    }
}

/// `Rational::mod_rat`: `r` less the multiple of `div` its truncated
/// quotient gives, unless `div` is infinite.
pub(crate) fn mod_rat(r: Rational, by: Rational) -> Rational {
    if by.is_inf() {
        return r;
    }
    let q = div(r, by);
    mul(sub(q, trunc_rat(q)), by)
}

/// `Rational::trunc_rat`: towards zero; an infinity or NaN as it is.
fn trunc_rat(r: Rational) -> Rational {
    if !r.is_finite() {
        return r;
    }
    let (n, d) = (r.numerator(), r.denominator());
    Rational::new(n - n % d, d)
}
