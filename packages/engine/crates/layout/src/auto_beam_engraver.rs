//! `auto-beam-engraver.cc` (MS4 step 2): `Auto_beam_engraver` and
//! `Grace_auto_beam_engraver`, classes of layout's
//! ([`AUTO_BEAM_ENGRAVER`], [`GRACE_AUTO_BEAM_ENGRAVER`]) that the core
//! passes at its `\layout` call and that a group finds before the music
//! crate's placeholders of the names (D67, D108), over
//! template-engraver-for-beams.cc's [`Beams`].
//!
//! - Each stem acknowledged is handled once the timestep's grobs are
//!   (`process_acknowledged`, `:466–513`; `handle_current_stem`,
//!   `:365–429`): a stem of an eighth or less, in the beam's grace state,
//!   ends the beam where `autoBeamCheck` says it ends at the shortest
//!   duration so far and begins one where it says one begins (`autoBeaming`
//!   on, no `\noBeam`), and is added to the beam and its pattern; a shorter
//!   duration checks the beam's ends again (`recheck_beam`, `:431–464`),
//!   the stems before an end beamed apart. A rest, a beam, a breathing
//!   sign, a bar line or `\noBeam` ends the beam, as does a skip
//!   (`process_music`, `:127–137`).
//! - A beam of two or more stems is made when it ends (`create_beam`,
//!   `:196–223`), from the `Beam` settings kept when it began, caused by
//!   its first stem and announced to the engraver's own group and to the
//!   context its voice was in when it began; its beamlets are set at the
//!   end of the timestep (`typeset_beam`).
//! - `Grace_auto_beam_engraver` (`:551–648`) beams one grace phrase whole:
//!   it begins at the start of the grace group and ends where the grace
//!   part does.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, CtxId, Duration, FINALIZE, Kind, Moment, PROCESS, PROCESS_ACKNOWLEDGED, Rational, START,
    STOP, TranslatorClass, Tx, Work, assign_event_once, event_property, grob_of,
};
use lily_engine_scheme::{Exception, Value};

use crate::beam;
use crate::beam_engraver::Beams;
use crate::beaming_pattern::BeamingPattern;
use crate::clef_engraver::is_true;
use crate::geometry::Direction;
use crate::grob;
use crate::paper_column_engraver::rational;
use crate::spanner;
use crate::stem;
use crate::tie_engraver::{event_length, in_event_class};

type Run<T> = Result<T, Exception>;

const START_DIR: Direction = -1;
const STOP_DIR: Direction = 1;

const AUTO_BEAM: TranslatorClass = TranslatorClass {
    name: "Auto_beam_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS | PROCESS_ACKNOWLEDGED | STOP | FINALIZE,
    // `start_translation_timestep` keeps the measure position of every
    // timestep.
    always: true,
    hears: &["beam-forbid-event"],
    acknowledges: &[
        ("stem-interface", false),
        ("beam-interface", false),
        ("breathing-sign-interface", false),
        ("rest-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(AutoBeamEngraver::new(false))),
    reads: &[
        "autoBeaming",
        "beamExceptions",
        "beamHalfMeasure",
        "beatBase",
        "beatStructure",
        "subdivideBeams",
    ],
    writes: &[],
};

pub static AUTO_BEAM_ENGRAVER: TranslatorClass = AUTO_BEAM;

pub static GRACE_AUTO_BEAM_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Grace_auto_beam_engraver",
    make: Some(|| Box::new(AutoBeamEngraver::new(true))),
    reads: &["autoBeaming"],
    ..AUTO_BEAM
};

/// `Auto_beam_engraver`, and `Grace_auto_beam_engraver` (`grace`): the
/// template's beams, `forbid_`, the measure position at the timestep's
/// start, `force_end_`, `considered_bar_`, the shortest duration in the
/// beam, the timestep's stem, the beam's stems, how often
/// `process_acknowledged` ran this timestep, the beam's projected end, the
/// context it began in and when, and the `Beam` settings it began with.
#[derive(Clone)]
struct AutoBeamEngraver {
    grace: bool,
    beams: Beams,
    forbid: Option<Value>,
    measure_position_at_start_of_timestep: Moment,
    force_end: bool,
    considered_bar: bool,
    shortest_dur: Rational,
    current_stem: Option<Value>,
    stems: Vec<Value>,
    process_acknowledged_count: u32,
    extend_mom: Moment,
    beam_start_context: Option<CtxId>,
    beam_start_moment: Moment,
    beam_settings: Value,
    /// `Grace_auto_beam_engraver`'s `last_grace_start_`, the full start
    /// of the last grace group, and `last_grace_position_`, its measure
    /// position.
    last_grace_start: Moment,
    last_grace_position: Moment,
}

/// `Moment::infinity ()`.
const INFINITE: Moment = Moment {
    main: Rational::INFINITY,
    grace: Rational::ZERO,
};

impl AutoBeamEngraver {
    fn new(grace: bool) -> AutoBeamEngraver {
        AutoBeamEngraver {
            grace,
            beams: Beams::default(),
            forbid: None,
            measure_position_at_start_of_timestep: Moment::default(),
            force_end: false,
            considered_bar: false,
            shortest_dur: Rational::new(1, 4),
            current_stem: None,
            stems: Vec::new(),
            process_acknowledged_count: 0,
            extend_mom: Moment::new(Rational::int(-1), Rational::ZERO),
            beam_start_context: None,
            beam_start_moment: INFINITE,
            beam_settings: Value::NULL,
            last_grace_start: Moment::new(-Rational::INFINITY, Rational::ZERO),
            last_grace_position: Moment::default(),
        }
    }

    /// `beaming_measure_position`: the measure position in the beat
    /// structure's period.
    fn beaming_measure_position(&self, tx: &mut Tx<'_>) -> Run<Moment> {
        crate::bar_engraver::scaled_measure_position(tx, self.beams.beaming_options.period)
    }

    /// `test_moment`: whether `autoBeamCheck` says a beam starts (`dir`
    /// -1) or stops (1) at `test_mom` for a duration `dur`;
    /// `Grace_auto_beam_engraver` begins at its grace group's start and
    /// ends where the grace part does.
    fn test_moment(
        &self,
        tx: &mut Tx<'_>,
        dir: Direction,
        test_mom: Moment,
        dur: Rational,
    ) -> Run<bool> {
        if self.grace {
            // If no grace group started this main moment, we have no
            // business beaming. Same if we have left the original main
            // time step.
            if self.last_grace_start.grace.is_zero()
                || self.last_grace_position.main != test_mom.main
            {
                return Ok(false);
            }
            // Autobeam start only when at the start of the grace group.
            if dir == START_DIR {
                return Ok(self.last_grace_position == test_mom);
            }
            // Autobeam end only when the grace part is finished.
            return Ok(test_mom.grace.is_zero());
        }
        // TODO: Scale test_mom to accumulate tuplet ratios
        let check = tx.get("autoBeamCheck");
        let context = tx.t.arena.borrow().handle(tx.context);
        let heap = tx.heap();
        let dir = heap.integer(i64::from(dir));
        let test_mom = heap.wrap(test_mom);
        let dur = heap.wrap(Moment::new(dur, Rational::ZERO));
        Ok(tx.apply(check, &[context, dir, test_mom, dur])? != Value::FALSE)
    }

    /// `busy`: whether a beam has begun.
    fn busy(&self) -> bool {
        self.beam_start_moment.compare(INFINITE).is_lt()
    }

    /// `consider_begin`.
    fn consider_begin(&mut self, tx: &mut Tx<'_>, dur: Rational) -> Run<()> {
        if !self.busy() && self.forbid.is_none() && is_true(tx.get("autoBeaming")) {
            let position = self.beaming_measure_position(tx)?;
            if self.test_moment(tx, START_DIR, position, dur)? {
                self.begin_beam(tx)?;
            }
        }
        Ok(())
    }

    /// `consider_end`: allow an autobeam to end when necessary, without
    /// checking `autoBeaming`, at the measure position the timestep began
    /// with (it might have changed, e.g. at a transition between volta
    /// alternatives).
    fn consider_end(&mut self, tx: &mut Tx<'_>, dur: Rational) -> Run<()> {
        if self.busy()
            && self.test_moment(
                tx,
                STOP_DIR,
                self.measure_position_at_start_of_timestep,
                dur,
            )?
        {
            self.end_beam(tx)?;
        }
        Ok(())
    }

    /// `create_beam`: none when typesetting is skipped or a stem has a
    /// beam already.
    fn create_beam(&mut self, tx: &mut Tx<'_>) -> Run<Option<(u32, Value)>> {
        if is_true(tx.get("skipTypesetting")) {
            return Ok(None);
        }

        for &stem in &self.stems {
            if stem::get_beam(tx.host, stem)?.is_some() {
                return Ok(None);
            }
        }

        // Can't use make_spanner () because we have to use beam_settings_.
        let Some((_, first)) = grob_of(tx.heap(), self.stems[0]) else {
            return Ok(None);
        };
        let id = tx.make_grob_with("Beam", Cause::Grob(first), self.beam_settings);
        let beam = tx.grob_handle(id);

        for &stem in &self.stems {
            beam::add_stem(tx.host, beam, stem)?;
        }

        tx.announce_locally(id, false);
        if let Some(c) = self.beam_start_context {
            tx.announce_in(c, id, false);
        }

        Ok(Some((id, beam)))
    }

    /// `begin_beam`: LilyPond's programming error if a beam has begun.
    fn begin_beam(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.busy() || self.beams.beam_pattern.is_some() {
            return tx.programming_error("already have autobeam");
        }

        self.stems.clear();

        self.beam_start_context = tx.t.arena.borrow().parent(tx.context);
        self.beam_start_moment = tx.now();
        self.beams.begin_beam(tx)?;
        self.beam_settings = tx.updated("Beam");
        Ok(())
    }

    /// `junk_beam`: the beam begun forgotten.
    fn junk_beam(&mut self) {
        if !self.busy() {
            return;
        }

        self.beam_start_context = None;
        self.beam_start_moment = INFINITE;
        self.stems.clear();
        self.beams.beam_pattern = None;
        self.beam_settings = Value::NULL;

        self.shortest_dur = Rational::new(1, 4);
    }

    /// `is_same_grace_state`: whether both are in grace time or neither
    /// is; `Grace_auto_beam_engraver` never ignores a grace note inside
    /// its beaming.
    fn is_same_grace_state(&self, start: Moment, now: Moment) -> bool {
        self.grace || start.grace.is_zero() == now.grace.is_zero()
    }

    /// `end_beam`: the beam made if it has two stems or more, else junked.
    fn end_beam(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.stems.len() < 2 {
            self.junk_beam();
        } else {
            let finished = self.create_beam(tx)?;
            self.beams.finished_beam = finished.map(|(_, b)| b);

            if let Some((id, _)) = finished {
                tx.announce_locally(id, true);
                if let Some(c) = self.beam_start_context {
                    tx.announce_in(c, id, true);
                }
                self.beams.finished_beam_pattern = self.beams.beam_pattern.take();
                self.beams.finished_beaming_options = self.beams.beaming_options;
            }

            self.beam_start_moment = INFINITE;
            self.stems.clear();
            self.beams.beam_pattern = None;
            self.beam_settings = Value::NULL;
        }

        self.beam_start_context = None;
        self.shortest_dur = Rational::new(1, 4);
        Ok(())
    }

    /// `typeset_beam`: the finished beam's right bound its left if it has
    /// none.
    fn typeset_beam(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(finished) = self.beams.finished_beam {
            if spanner::get_bound(tx.host, finished, 1)?.is_none()
                && let Some(left) = spanner::get_bound(tx.host, finished, -1)?
            {
                spanner::set_bound(tx.host, finished, 1, left)?;
            }

            self.beams.typeset_beam(tx)?;
        }
        Ok(())
    }

    /// `handle_current_stem`.
    fn handle_current_stem(&mut self, tx: &mut Tx<'_>, stem: Value) -> Run<()> {
        let ev = grob::ultimate_event_cause(tx.heap(), stem);
        let Some(ev) = ev.filter(|&ev| in_event_class(tx.heap(), ev, "rhythmic-event")) else {
            return tx.programming_error("stem must have rhythmic structure");
        };

        // Don't (start) auto-beam over empty stems (skips or rests) or
        // stems that already have beam
        if stem::head_count(tx.host, stem)? == 0 || stem::get_beam(tx.host, stem)?.is_some() {
            if self.busy() {
                self.end_beam(tx)?;
            }
            return Ok(());
        }

        let v = event_property(tx.heap(), ev, "duration");
        let stem_duration = tx
            .heap()
            .foreign::<Duration>(v)
            .copied()
            .unwrap_or(Duration::new(0, 0));

        let v = tx.get("meterScalingFactor");
        let meter_scaling_factor = rational(tx.heap(), v, Rational::int(1));
        let stem_duration = Rational::int(1)
            .checked_div(meter_scaling_factor)
            .and_then(|m| stem_duration.compressed(m))
            .unwrap_or(stem_duration);

        if stem_duration.log() <= 2 {
            if self.busy() {
                self.end_beam(tx)?;
            }
            return Ok(());
        }

        // ignore interspersed grace notes.
        let now = tx.now();
        if !self.is_same_grace_state(self.beam_start_moment, now) {
            return Ok(());
        }

        let dur = stem_duration.length().unwrap_or(Rational::NAN);
        let mut recheck_needed = false;

        if dur.compare(self.shortest_dur).is_lt() {
            // new shortest moment, so store it and set recheck_needed
            self.shortest_dur = dur;
            recheck_needed = true;
        }

        // end should be based on shortest_dur_, begin should be based on
        // current duration
        self.consider_end(tx, self.shortest_dur)?;
        self.consider_begin(tx, dur)?;

        if !self.busy() {
            return Ok(());
        }

        self.beams.last_added_moment = now;
        self.beams.add_stem(tx, stem, stem_duration)?;

        self.stems.push(stem);
        let start = if self.extend_mom.compare(now).is_lt() {
            now
        } else {
            self.extend_mom
        };
        let len = event_length(tx.heap(), ev, now);
        self.extend_mom = start.checked_add(len).unwrap_or(INFINITE);
        if recheck_needed {
            self.recheck_beam(tx)?;
        }
        Ok(())
    }

    /// `recheck_beam`: after the shortest duration has changed, the first
    /// part of the beam typeset and the beam begun again with the rest, at
    /// each stem where it now ends.
    fn recheck_beam(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let mut i = 0;
        while i + 1 < self.stems.len() {
            let Some(pattern) = &self.beams.beam_pattern else {
                break;
            };
            let at = pattern
                .end_moment(i)
                .checked_sub(pattern.start_moment(0))
                .and_then(|m| m.checked_add(pattern.measure_offset))
                .unwrap_or(Rational::NAN);
            let found_end = self.test_moment(
                tx,
                STOP_DIR,
                Moment::new(at, Rational::ZERO),
                self.shortest_dur,
            )?;
            if !found_end {
                i += 1;
            } else {
                // Save the current beam settings and shortest_dur_
                // Necessary because end_beam destroys them
                let saved_shortest_dur = self.shortest_dur;
                let saved_beam_settings = self.beam_settings;

                // Eliminate (and save) the items no longer part of the
                // first beam
                let period = self.beams.beaming_options.period;
                let new_grouping: Option<BeamingPattern> = self
                    .beams
                    .beam_pattern
                    .as_mut()
                    .map(|p| p.split_pattern(i, period));
                let new_stems = self.stems.split_off(i + 1);

                self.end_beam(tx)?;
                self.typeset_beam(tx)?;

                // now recreate the unbeamed data structures
                self.stems = new_stems;
                self.beams.beam_pattern = new_grouping;
                self.shortest_dur = saved_shortest_dur;
                self.beam_settings = saved_beam_settings;
                self.beam_start_context = tx.t.arena.borrow().parent(tx.context);
                self.beam_start_moment = tx.now();

                i = 0;
            }
        }
        Ok(())
    }
}

impl Work for AutoBeamEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[self.grace, self.force_end, self.considered_bar]);
        self.beams.held(h);
        h.moment(self.measure_position_at_start_of_timestep);
        h.rational(self.shortest_dur);
        h.num(self.process_acknowledged_count.into());
        h.moment(self.extend_mom);
        h.maybe_context(self.beam_start_context);
        h.moment(self.beam_start_moment);
        h.moment(self.last_grace_start);
        h.moment(self.last_grace_position);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        assign_event_once(tx, &mut self.forbid, ev)?;
        self.force_end = true;
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.measure_position_at_start_of_timestep = self.beaming_measure_position(tx)?;
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let now = tx.now();
        if self.grace {
            // Update last_grace_start_ and last_grace_position_ only when
            // the main time advances.
            if self.last_grace_start.main.compare(now.main).is_lt() {
                self.last_grace_start = now;
                self.last_grace_position = self.beaming_measure_position(tx)?;
            }
        }

        // don't beam over skips
        if self.busy() && self.extend_mom.compare(now).is_lt() {
            self.end_beam(tx)?;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        if which == 0 {
            self.current_stem = Some(tx.grob_handle(grob));
        } else {
            // A beam, a breathing sign or a rest.
            self.force_end = true;
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // This engraver can't observe bar lines with acknowledge_bar_line
        // () because the Bar_engraver operates in Staff context.
        // process_acknowledged () can be called more than once, but
        // currentBarLine won't change.
        if !self.considered_bar {
            self.considered_bar = true;

            if !self.force_end && self.busy() {
                let bar = tx.get("currentBarLine");
                if grob_of(tx.heap(), bar).is_some() {
                    self.force_end = true;
                }
            }
        }

        if self.force_end {
            self.force_end = false;

            if self.busy() {
                self.end_beam(tx)?;
            }
        }

        if let Some(stem) = self.current_stem.take() {
            self.handle_current_stem(tx, stem)?;
        }

        let now = tx.now();
        if self.extend_mom.compare(now).is_gt() {
            return Ok(());
        }

        if self.busy() {
            if self.process_acknowledged_count == 0 {
                self.consider_end(tx, self.shortest_dur)?;
            } else if self.process_acknowledged_count > 1 {
                if self.extend_mom.compare(now).is_lt()
                    || (self.extend_mom == now && self.beams.last_added_moment != now)
                {
                    self.end_beam(tx)?;
                } else if self.stems.is_empty() {
                    self.junk_beam();
                }
            }
        }

        self.process_acknowledged_count += 1;
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.typeset_beam(tx)?;
        self.process_acknowledged_count = 0;
        self.forbid = None;
        self.considered_bar = false;
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // finished beams may be typeset
        self.typeset_beam(tx)?;
        // but unfinished may need another announce/acknowledge pass
        if self.busy() {
            self.junk_beam();
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        self.beams.values(out);
        out.extend(self.forbid);
        out.extend(self.current_stem);
        out.extend(&self.stems);
        out.push(self.beam_settings);
    }
}
