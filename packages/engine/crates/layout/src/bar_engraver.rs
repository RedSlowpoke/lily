//! `bar-engraver.cc` (MS3 step 18): `Bar_engraver`, a class of layout's
//! ([`BAR_ENGRAVER`]) that the core passes at its `\layout` call and that
//! a group finds before the music crate's placeholder of the name (D67,
//! D108):
//!
//! - `\caesura` heard (`listen_caesura`, `:380–384`) once a timestep, with
//!   `assign_event_once`'s warnings; a segno mark (`listen_segno_mark`,
//!   `:386–399`) observed but for a `\segnoMark \default` at the first
//!   timestep; `\fine`, `\section` and the underlying repeat's ad hoc
//!   jumps, coda marks and dal segnos heard (`Boolean_event_listener`s).
//! - `initialize` (`:401–418`): `currentBarLine` set to `'()`, and the
//!   first timestep the `Timing` context's first.
//! - `start_translation_timestep` (`:420–430`): `currentBarLine` unset of
//!   the last timestep's bar.
//! - `pre_process_music` (`:433–521`): `whichBar` if it is a string, else
//!   the repeats of `repeatCommands` observed (none at the first timestep
//!   unless `printInitialRepeatBar`) and the bar types of the timestep from
//!   the highest priority down (`calc_bar_type`, `:182–378`): repeat, fine,
//!   section, caesura (`caesuraType` with the event's articulations, through
//!   `caesuraTypeTransform`), measure, submeasure
//!   (`is_at_submeasure_division`, `:124–179`), underlying caesura and
//!   underlying repeat; then the glyph of each break direction from them,
//!   by the image's `bar-line::calc-glyph-name-for-direction`. With no
//!   glyph and `forbidBreakBetweenBarLines`, `forbidBreak` set on the
//!   Score context (`:516–520`), for the other engravers to see at
//!   `process_music`.
//! - `process_music` (`:523–546`): a `BarLine` with no cause if any glyph
//!   is a string, each glyph set where it differs from the grob's, and
//!   `currentBarLine` set to it.
//! - The end of a spanner acknowledged (`acknowledge_end_spanner`,
//!   `:579–588`): kept if a bar line was made and its `to-barline` is
//!   true; `process_acknowledged` (`:548–558`) makes the bar line the right
//!   bound of each kept, then forgets them.
//! - A span bar stub acknowledged (`acknowledge_span_bar_stub`,
//!   `:590–599`) where no bar line was made: its `allow-span-bar` and
//!   `allow-span-bar-above` set to `#f`.
//! - `stop_translation_timestep` (`:563–577`): the timestep's state reset.
//!
//! A bar type is the property's string itself where LilyPond copies it
//! (`to_scm (ub)`); the glyphs are compared by `equal?`, as LilyPond does.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, CtxId, INITIALIZE, Kind, Moment, PRE_PROCESS, PROCESS, PROCESS_ACKNOWLEDGED, Rational,
    START, STOP, TranslatorClass, Tx, Work, assign_event_once, event_property,
};
use lily_engine_scheme::{Exception, Tag, Value};

use crate::clef_engraver::is_true;
use crate::grob;
use crate::paper_column_engraver::{find_score_context, rational};
use crate::spanner;

type Run<T> = Result<T, Exception>;

pub static BAR_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Bar_engraver",
    kind: Kind::Engraver,
    methods: INITIALIZE | START | PRE_PROCESS | PROCESS | PROCESS_ACKNOWLEDGED | STOP,
    always: true,
    hears: &[
        "caesura-event",
        "segno-mark-event",
        "fine-event",
        "section-event",
        "ad-hoc-jump-event",
        "coda-mark-event",
        "dal-segno-event",
    ],
    acknowledges: &[
        ("span-bar-stub-interface", false),
        ("spanner-interface", true),
    ],
    must_be_last: false,
    make: Some(|| Box::new(BarEngraver::default())),
    reads: &[
        "caesuraType",
        "caesuraTypeTransform",
        "doubleRepeatBarType",
        "doubleRepeatSegnoBarType",
        "endRepeatBarType",
        "endRepeatSegnoBarType",
        "fineBarType",
        "fineSegnoBarType",
        "fineStartRepeatSegnoBarType",
        "forbidBreakBetweenBarLines",
        "measureBarType",
        "printInitialRepeatBar",
        "printTrivialVoltaRepeats",
        "repeatCommands",
        "sectionBarType",
        "segnoBarType",
        "segnoStyle",
        "startRepeatBarType",
        "startRepeatSegnoBarType",
        "submeasureBarsEnabled",
        "submeasureBarType",
        "submeasureStructure",
        "underlyingRepeatBarType",
        "whichBar",
    ],
    writes: &["currentBarLine", "forbidBreak"],
};

const CAESURA: usize = 0;
const SEGNO_MARK: usize = 1;
const FINE: usize = 2;
const SECTION: usize = 3;

const SPAN_BAR_STUB: usize = 0;

/// `BarType`, from low to high priority.
#[derive(Clone, Copy)]
enum BarType {
    UnderlyingRepeat,
    UnderlyingCaesura,
    Submeasure,
    Measure,
    Caesura,
    Section,
    Fine,
    Repeat,
}

/// The order `calc_bar_type` tries them in.
const TYPES_BY_PRIORITY: [BarType; 8] = [
    BarType::Repeat,
    BarType::Fine,
    BarType::Section,
    BarType::Caesura,
    BarType::Measure,
    BarType::Submeasure,
    BarType::UnderlyingCaesura,
    BarType::UnderlyingRepeat,
];

/// `Observations`.
#[derive(Clone, Copy, Default)]
struct Observations {
    repeat_end: bool,
    repeat_start: bool,
    segno: bool,
}

/// `caesura_ev_`, the three `Boolean_event_listener`s (`fine_listener_`,
/// `section_listener_`, `underlying_repeat_listener_`), the observations,
/// the glyphs, the submeasures' structure as last computed, `bar_` and
/// `spanners_` by their handles, `first_time_` and `has_any_glyph_`.
#[derive(Clone)]
struct BarEngraver {
    caesura: Option<Value>,
    fine: bool,
    section: bool,
    underlying_repeat: bool,
    observations: Observations,
    glyph: Value,
    glyph_left: Value,
    glyph_right: Value,
    /// `#f` forces the first calculation.
    submeasure_structure: Value,
    submeasure_positions: Vec<Rational>,
    submeasure_period: Rational,
    bar: Option<Value>,
    spanners: Vec<Value>,
    first_time: bool,
    has_any_glyph: bool,
}

impl Default for BarEngraver {
    fn default() -> Self {
        BarEngraver {
            caesura: None,
            fine: false,
            section: false,
            underlying_repeat: false,
            observations: Observations::default(),
            glyph: Value::NULL,
            glyph_left: Value::NULL,
            glyph_right: Value::NULL,
            submeasure_structure: Value::FALSE,
            submeasure_positions: Vec::new(),
            submeasure_period: Rational::ZERO,
            bar: None,
            spanners: Vec::new(),
            first_time: true,
            has_any_glyph: false,
        }
    }
}

/// `find_context_above (c, 'Timing)`: `c` or the first context above it
/// with the alias.
fn find_timing(tx: &mut Tx<'_>) -> Option<CtxId> {
    let heap = tx.host.heap();
    let timing = heap.intern("Timing");
    let a = tx.t.arena.borrow();
    let mut at = Some(tx.context);
    while let Some(c) = at {
        if a.is_alias(heap, c, timing) {
            return Some(c);
        }
        at = a.parent(c);
    }
    None
}

/// `euclidean_remainder` of `x` by a positive `length`: in `[0, length)`.
fn euclidean_remainder(x: Rational, length: Rational) -> Rational {
    let Some(q) = x.checked_div(length) else {
        return x;
    };
    let floor = Rational::int(q.numerator().div_euclid(q.denominator()));
    floor
        .checked_mul(length)
        .and_then(|m| x.checked_sub(m))
        .unwrap_or(x)
}

/// `measure_position (context (), length)` (`context.cc:931–936`):
/// [`measure_position_of`] `measurePosition`.
fn measure_position(tx: &mut Tx<'_>, length: Rational) -> Run<Moment> {
    let v = tx.get("measurePosition");
    let position = tx.heap().foreign::<Moment>(v).copied().unwrap_or_default();
    measure_position_of(tx, position, length)
}

/// `measure_position (context (), position, length)` (`context.cc:
/// 886–929`): `position` by the Euclidean remainder of `length`; without
/// a finite length, a negative position is `now`.
pub(crate) fn measure_position_of(
    tx: &mut Tx<'_>,
    mut position: Moment,
    length: Rational,
) -> Run<Moment> {
    if length.is_zero() {
        tx.programming_error("cannot divide by zero measure length")?;
        position.main = Rational::ZERO;
        return Ok(position);
    }
    if length.is_finite() {
        position.main = euclidean_remainder(position.main, length);
    } else if position.main.is_negative() {
        return Ok(tx.now());
    }
    Ok(position)
}

/// `scaled_measure_position` (`context.cc:944–953`): the position in a
/// measure of `length` scaled by `meterScalingFactor`.
pub(crate) fn scaled_measure_position(tx: &mut Tx<'_>, length: Rational) -> Run<Moment> {
    let v = tx.get("meterScalingFactor");
    let factor = rational(tx.heap(), v, Rational::int(1));
    let actual_len = length.checked_mul(factor).unwrap_or(length);
    let actual_pos = measure_position(tx, actual_len)?;
    Ok(actual_pos.checked_div(factor).unwrap_or(actual_pos))
}

/// `from_scm (v, fallback)` of a `long`: an exact integer, or `fallback`.
pub(crate) fn long_or(tx: &mut Tx<'_>, v: Value, fallback: i64) -> i64 {
    let heap = tx.heap();
    if heap.is_exact_integer(v) {
        heap.to_i64(v).unwrap_or(fallback)
    } else {
        fallback
    }
}

impl BarEngraver {
    fn is_at_submeasure_division(&mut self, tx: &mut Tx<'_>) -> Run<bool> {
        // Recompute submeasure positions after changes in
        // submeasureStructure, set since the last time if not `eq?`.
        let structure = tx.get("submeasureStructure");
        if self.submeasure_structure != structure {
            self.submeasure_structure = structure;

            self.submeasure_positions.clear();
            self.submeasure_positions.push(Rational::ZERO);
            let mut sum = Rational::ZERO;
            let mut p = structure;
            while let (Some(n), Some(rest)) = (tx.heap().car(p), tx.heap().cdr(p)) {
                let n = rational(tx.heap(), n, Rational::ZERO);
                sum = sum.checked_add(n).unwrap_or(sum);
                self.submeasure_positions.push(sum);
                p = rest;
            }
            // The last is the structure's period in beatBase units, zero
            // for an empty structure.
            self.submeasure_period = self.submeasure_positions.pop().unwrap_or(Rational::ZERO);
            let v = tx.get("beatBase");
            let beat_base = rational(tx.heap(), v, Rational::new(1, 4));
            let scale = |r: Rational| r.checked_mul(beat_base).unwrap_or(r);
            self.submeasure_period = scale(self.submeasure_period);
            for pos in &mut self.submeasure_positions {
                *pos = scale(*pos);
            }
        }

        // A submeasure can only begin when the main part of the time
        // advances.
        if tx.now().main == tx.prev().main {
            return Ok(false);
        }

        if self.submeasure_period.is_zero() {
            // At a measure boundary, also at a submeasure boundary.
            return Ok(is_true(tx.get("measureStartNow")));
        }

        let pos = scaled_measure_position(tx, self.submeasure_period)?;
        Ok(self.submeasure_positions.contains(&pos.main))
    }

    /// `get_caesura_type`: `caesuraType` with the event's articulation
    /// types in front as `articulations`, through `caesuraTypeTransform`
    /// if it is a procedure.
    fn caesura_type(tx: &mut Tx<'_>, ev: Value) -> Run<Value> {
        let caesura_type = tx.get("caesuraType");
        let arts = event_property(tx.heap(), ev, "articulations");
        let mut types = Vec::new();
        let mut p = arts;
        while let (Some(art), Some(rest)) = (tx.heap().car(p), tx.heap().cdr(p)) {
            let t = event_property(tx.heap(), art, "articulation-type");
            if t.tag() == Tag::Symbol {
                types.push(t);
            }
            p = rest;
        }
        let heap = tx.heap();
        let types = heap.list(&types);
        let key = heap.intern("articulations");
        let entry = heap.cons(key, types);
        let caesura_type = heap.cons(entry, caesura_type);
        let transform = tx.get("caesuraTypeTransform");
        if tx.heap().is_procedure(transform) {
            let depth = tx.heap().root_depth();
            tx.heap().root(caesura_type);
            let handle = tx.t.arena.borrow().handle(tx.context);
            let result = tx
                .host
                .call(transform, &[handle, caesura_type, Value::NULL]);
            tx.heap().unroot_to(depth);
            return result;
        }
        Ok(caesura_type)
    }

    /// `calc_bar_type`: the timestep's `BarLine.glyph` values from the
    /// highest priority to the lowest, each a string.
    fn calc_bar_type(&mut self, tx: &mut Tx<'_>) -> Run<Vec<Value>> {
        let caesura_type = match self.caesura {
            Some(ev) => Self::caesura_type(tx, ev)?,
            None => Value::NULL,
        };
        let depth = tx.heap().root_depth();
        tx.heap().root(caesura_type);
        let glyphs = self.bar_types(tx, caesura_type);
        tx.heap().unroot_to(depth);
        glyphs
    }

    fn bar_types(&mut self, tx: &mut Tx<'_>, caesura_type: Value) -> Run<Vec<Value>> {
        let segno_style = tx.get("segnoStyle");
        let bar_line = tx.heap().intern("bar-line");
        let segno = self.observations.segno && segno_style == bar_line;
        let obs = self.observations;

        let mut glyphs = Vec::new();
        for layer in TYPES_BY_PRIORITY {
            let read = |tx: &mut Tx<'_>, name: &str| Some(tx.get(name));
            let read_caesura = |tx: &mut Tx<'_>, name: &str| {
                let heap = tx.heap();
                let sym = heap.intern(name);
                grob::assq_ref(heap, sym, caesura_type)
            };
            let ub = match layer {
                BarType::Repeat if segno => {
                    if obs.repeat_start {
                        if obs.repeat_end {
                            read(tx, "doubleRepeatSegnoBarType")
                        } else if self.fine {
                            read(tx, "fineStartRepeatSegnoBarType")
                        } else {
                            read(tx, "startRepeatSegnoBarType")
                        }
                    } else if obs.repeat_end {
                        read(tx, "endRepeatSegnoBarType")
                    } else if self.fine {
                        read(tx, "fineSegnoBarType")
                    } else {
                        read(tx, "segnoBarType")
                    }
                }
                BarType::Repeat => {
                    if obs.repeat_start {
                        if obs.repeat_end {
                            read(tx, "doubleRepeatBarType")
                        } else {
                            read(tx, "startRepeatBarType")
                        }
                    } else if obs.repeat_end {
                        read(tx, "endRepeatBarType")
                    } else {
                        None
                    }
                }
                BarType::Fine if self.fine => read(tx, "fineBarType"),
                BarType::Section if self.section => read(tx, "sectionBarType"),
                BarType::Caesura if self.caesura.is_some() => read_caesura(tx, "bar-line"),
                BarType::Measure if !self.first_time && is_true(tx.get("measureStartNow")) => {
                    read(tx, "measureBarType")
                }
                BarType::Submeasure
                    if !self.first_time && is_true(tx.get("submeasureBarsEnabled")) =>
                {
                    if self.is_at_submeasure_division(tx)? {
                        read(tx, "submeasureBarType")
                    } else {
                        None
                    }
                }
                BarType::UnderlyingCaesura if self.caesura.is_some() => {
                    read_caesura(tx, "underlying-bar-line")
                }
                BarType::UnderlyingRepeat if self.underlying_repeat => {
                    read(tx, "underlyingRepeatBarType")
                }
                _ => None,
            };
            if let Some(s) = ub
                && s.tag() == Tag::String
            {
                glyphs.push(s);
            }
        }
        Ok(glyphs)
    }
}

impl Work for BarEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        let o = self.observations;
        h.flags(&[
            self.fine,
            self.section,
            self.underlying_repeat,
            o.repeat_end,
            o.repeat_start,
            o.segno,
            self.first_time,
            self.has_any_glyph,
        ]);
        h.num(self.submeasure_positions.len() as i64);
        for &r in &self.submeasure_positions {
            h.rational(r);
        }
        h.rational(self.submeasure_period);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        match which {
            CAESURA => assign_event_once(tx, &mut self.caesura, ev).map(drop),
            SEGNO_MARK => {
                // Ignore a default segno at the beginning of a piece, as
                // Mark_tracking_translator does.
                if self.first_time {
                    let label = event_property(tx.heap(), ev, "label");
                    if !tx.heap().is_integer(label) {
                        return Ok(());
                    }
                }
                self.observations.segno = true;
                Ok(())
            }
            FINE => {
                self.fine = true;
                Ok(())
            }
            SECTION => {
                self.section = true;
                Ok(())
            }
            // An ad hoc jump, a coda mark or a dal segno.
            _ => {
                self.underlying_repeat = true;
                Ok(())
            }
        }
    }

    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        tx.set("currentBarLine", Value::NULL);

        // For initial bar lines, what matters is the Timing context's
        // lifetime, not a late-created staff's.
        if let Some(timing) = find_timing(tx) {
            let a = tx.t.arena.borrow();
            self.first_time = a.init_mom(timing).compare(a.now()).is_ge();
        }
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // Reset here rather than at the stop, so that other engravers can
        // use it then.
        if self.bar.take().is_some() {
            tx.set("currentBarLine", Value::NULL);
        }
        Ok(())
    }

    fn pre_process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let depth = tx.heap().root_depth();
        let ran = self.choose_glyphs(tx);
        tx.heap().unroot_to(depth);
        ran?;

        // In pre_process_music, so that other engravers notice at
        // process_music that a break will not be allowed (unless forced),
        // and can skip what is only needed where one may be.
        if !self.has_any_glyph && is_true(tx.get("forbidBreakBetweenBarLines")) {
            let score = find_score_context(tx);
            let sym = tx.heap().intern("forbidBreak");
            tx.t.arena.borrow_mut().set(score, sym, Value::TRUE);
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if !self.has_any_glyph {
            return Ok(());
        }
        let id = tx.make_defined_grob("BarLine", Cause::None)?;
        let bar = tx.grob_handle(id);
        self.bar = Some(bar);
        for (name, glyph) in [
            ("glyph", self.glyph),
            ("glyph-left", self.glyph_left),
            ("glyph-right", self.glyph_right),
        ] {
            let current = grob::property(tx.host, bar, name)?;
            if !tx.heap().equal(glyph, current) {
                grob::set_property(tx.heap(), bar, name, glyph);
            }
        }
        tx.set("currentBarLine", bar);
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        if which == SPAN_BAR_STUB {
            if self.bar.is_none() {
                // Tell Span_bar_engraver that drawing a bar to this
                // context is unwanted.
                grob::set_property(tx.heap(), g, "allow-span-bar", Value::FALSE);
                grob::set_property(tx.heap(), g, "allow-span-bar-above", Value::FALSE);
            }
        } else if self.bar.is_some() && is_true(grob::property(tx.host, g, "to-barline")?) {
            self.spanners.push(g);
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(bar) = self.bar {
            for &sp in &self.spanners {
                spanner::set_bound(tx.host, sp, 1, bar)?;
            }
        }
        self.spanners.clear();
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.glyph = Value::NULL;
        self.glyph_left = Value::NULL;
        self.glyph_right = Value::NULL;
        self.first_time = false;
        self.has_any_glyph = false;
        self.observations = Observations::default();

        self.caesura = None;
        self.fine = false;
        self.section = false;
        self.underlying_repeat = false;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.caesura);
        out.extend([
            self.glyph,
            self.glyph_left,
            self.glyph_right,
            self.submeasure_structure,
        ]);
        out.extend(self.bar);
        out.extend(&self.spanners);
    }
}

impl BarEngraver {
    /// `pre_process_music` but for its `forbidBreak`; what it makes is
    /// rooted until the caller unroots.
    fn choose_glyphs(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // whichBar, if set, probably with \bar.
        let wb = tx.get("whichBar");
        let glyphs = if wb.tag() == Tag::String {
            tx.heap().list(&[wb])
        } else {
            if !self.first_time || is_true(tx.get("printInitialRepeatBar")) {
                self.observe_repeats(tx);
            } else {
                self.observations.repeat_end = false;
                self.observations.repeat_start = false;
                self.underlying_repeat = false;
            }

            let obs = self.observations;
            if obs.repeat_start || obs.repeat_end || obs.segno {
                self.underlying_repeat = true;
            }

            let types = self.calc_bar_type(tx)?;
            tx.heap().list(&types)
        };
        tx.heap().root(glyphs);

        let calc_name = |tx: &mut Tx<'_>, dir: i64| -> Run<Value> {
            let dir = tx.heap().integer(dir);
            let name = tx.call("bar-line::calc-glyph-name-for-direction", &[glyphs, dir])?;
            tx.heap().root(name);
            Ok(name)
        };
        self.glyph = calc_name(tx, 0)?;
        self.glyph_left = calc_name(tx, -1)?;
        self.glyph_right = calc_name(tx, 1)?;
        self.has_any_glyph = [self.glyph, self.glyph_left, self.glyph_right]
            .iter()
            .any(|g| g.tag() == Tag::String);
        Ok(())
    }

    /// The repeats of `repeatCommands`: an `end-repeat` of a count of 1 or
    /// more (1 if none), a `start-repeat` of 2 or more (2 if none), or
    /// either under `printTrivialVoltaRepeats`.
    fn observe_repeats(&mut self, tx: &mut Tx<'_>) {
        let (end, start) = {
            let heap = tx.heap();
            (heap.intern("end-repeat"), heap.intern("start-repeat"))
        };
        let mut p = tx.get("repeatCommands");
        while let (Some(mut command), Some(rest)) = (tx.heap().car(p), tx.heap().cdr(p)) {
            let mut options = Value::NULL;
            // (command option...)
            if let (Some(c), Some(o)) = (tx.heap().car(command), tx.heap().cdr(command)) {
                options = o;
                command = c;
            }
            let count = |tx: &mut Tx<'_>, fallback: i64| match tx.heap().car(options) {
                Some(first) => long_or(tx, first, fallback),
                None => fallback,
            };
            if command == end {
                if count(tx, 1) >= 1 || is_true(tx.get("printTrivialVoltaRepeats")) {
                    self.observations.repeat_end = true;
                }
            } else if command == start
                && (count(tx, 2) >= 2 || is_true(tx.get("printTrivialVoltaRepeats")))
            {
                self.observations.repeat_start = true;
            }
            p = rest;
        }
    }
}
