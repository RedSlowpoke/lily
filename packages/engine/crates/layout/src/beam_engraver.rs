//! `beam-engraver.cc` (MS4 step 2): `Beam_engraver` and
//! `Grace_beam_engraver`, classes of layout's ([`BEAM_ENGRAVER`],
//! [`GRACE_BEAM_ENGRAVER`]) that the core passes at its `\layout` call and
//! that a group finds before the music crate's classes of the names, which
//! keep `beamMelismaBusy` without grobs (D98, D108); these keep it as
//! LilyPond's do (`set_melisma`). With template-engraver-for-beams.cc's
//! `begin_beam`, `typeset_beam` and `add_stem` ([`Beams`]), which
//! `Auto_beam_engraver` shares (`auto_beam_engraver.rs`).
//!
//! - `listen_beam` (`:96–111`): a beam's start and stop, in main time
//!   (`Beam_engraver`) or in grace time (`Grace_beam_engraver`, whose stop
//!   needs a beam), the start's direction forced on its stems.
//! - `process_music` (`:121–143`): a `Beam` made of the start, unless one
//!   is open (`already have a beam`), and its end announced at the stop.
//! - `acknowledge_stem` (`:223–271`): each stem of the beam's time added
//!   to it and to its beaming pattern, with LilyPond's warnings for a stem
//!   with no rhythm and one too long for a beam; `acknowledge_rest`
//!   (`:211–221`) chains the beam's rest offsets on each rest under it
//!   (`beam.rs`).
//! - `stop_translation_timestep` (`:175–189`): the beam finished at its
//!   stop, its beamlets set (`typeset_beam`, `:145–164`); `finalize`
//!   (`:191–209`) kills a beam left open (`unterminated beam`).
//!
//! The engraver sleeps between beams: it stays awake while one is open,
//! for `beamMelismaBusy`, and its start is forgotten at the end of each
//! timestep, where LilyPond's forgets it at the next one's start, which a
//! sleeping translator is not called for.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Duration, FINALIZE, Kind, Moment, PROCESS, START, STOP, TranslatorClass, Tx, Work,
    assign_event_once, event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::beam;
use crate::beaming_pattern::{BeamingOptions, BeamingPattern};
use crate::clef_engraver::is_true;
use crate::direction;
use crate::dynamic_align_engraver::direction as to_direction;
use crate::fingering_engraver::property_data;
use crate::geometry::{Direction, Y};
use crate::grob;
use crate::spanner;
use crate::stem;
use crate::tie_engraver::in_event_class;

type Run<T> = Result<T, Exception>;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

const BEAM: TranslatorClass = TranslatorClass {
    name: "Beam_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS | STOP | FINALIZE,
    always: false,
    hears: &["beam-event"],
    acknowledges: &[("stem-interface", false), ("rest-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(BeamEngraver::default())),
    reads: &[
        "beamMelismaBusy",
        "beatBase",
        "beatStructure",
        "subdivideBeams",
    ],
    writes: &[],
};

pub static BEAM_ENGRAVER: TranslatorClass = BEAM;

pub static GRACE_BEAM_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Grace_beam_engraver",
    make: Some(|| {
        Box::new(BeamEngraver {
            grace: true,
            ..BeamEngraver::default()
        })
    }),
    ..BEAM
};

/// `Template_engraver_for_beams`: the beam's options and pattern, those of
/// the beam finished, and the moment of the last stem added.
#[derive(Clone, Default)]
pub(crate) struct Beams {
    pub beaming_options: BeamingOptions,
    pub beam_pattern: Option<BeamingPattern>,
    pub finished_beam: Option<Value>,
    pub finished_beaming_options: BeamingOptions,
    pub finished_beam_pattern: Option<BeamingPattern>,
    pub last_added_moment: Moment,
}

impl Beams {
    /// What it holds in Rust, for N8's state digest (D240).
    pub(crate) fn held(&self, h: &mut dyn lily_engine_music::Held) {
        self.beaming_options.held(h);
        match &self.beam_pattern {
            Some(p) => p.held(h),
            None => h.num(-1),
        }
        h.flags(&[self.finished_beam.is_some()]);
        self.finished_beaming_options.held(h);
        match &self.finished_beam_pattern {
            Some(p) => p.held(h),
            None => h.num(-1),
        }
        h.moment(self.last_added_moment);
    }

    /// `begin_beam`: the options read and a pattern begun at the measure
    /// position of the beam's start in its beat structure's period (in
    /// grace time, its grace part's).
    pub fn begin_beam(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let v = tx.get("measurePosition");
        let mut beam_start_position = tx.heap().foreign::<Moment>(v).copied().unwrap_or_default();

        self.beaming_options = BeamingOptions::new(tx);
        if !beam_start_position.grace.is_zero() {
            beam_start_position.main = beam_start_position.grace;
        }
        let offset = crate::bar_engraver::measure_position_of(
            tx,
            beam_start_position,
            self.beaming_options.period,
        )?;
        self.beam_pattern = Some(BeamingPattern::new(offset.main));
        Ok(())
    }

    /// `typeset_beam`: the finished beam's beamlets set from its pattern.
    pub fn typeset_beam(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(beam) = self.finished_beam.take()
            && let Some(mut pattern) = self.finished_beam_pattern.take()
        {
            pattern.beamify(tx.heap(), &self.finished_beaming_options)?;
            beam::set_beaming(tx.host, beam, &pattern)?;
        }
        Ok(())
    }

    /// `add_stem`: the stem added to the pattern at the last moment added,
    /// its grace part in grace time, with `currentTupletDescription`.
    pub fn add_stem(&mut self, tx: &mut Tx<'_>, stem: Value, dur: Duration) -> Run<()> {
        let m = if self.last_added_moment.grace.is_zero() {
            self.last_added_moment.main
        } else {
            self.last_added_moment.grace
        };
        let invisible = stem::is_invisible(tx.host, stem)?;
        let tuplet = tx.get("currentTupletDescription");
        if let Some(pattern) = &mut self.beam_pattern {
            pattern.add_stem(tx.heap(), m, invisible, dur, tuplet);
        }
        Ok(())
    }

    /// The Scheme values it holds.
    pub fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.finished_beam);
        for options in [&self.beaming_options, &self.finished_beaming_options] {
            out.push(options.beat_structure);
        }
        for pattern in [&self.beam_pattern, &self.finished_beam_pattern]
            .into_iter()
            .flatten()
        {
            pattern.values(out);
        }
    }
}

/// `Beam_engraver`, and `Grace_beam_engraver` (`grace`), which takes the
/// beams of grace time: `start_ev_`, `beam_` by its record and handle,
/// `prev_start_ev_`, `stop_ev_` and `forced_direction_`.
#[derive(Clone, Default)]
struct BeamEngraver {
    grace: bool,
    beams: Beams,
    start_ev: Option<Value>,
    beam: Option<(u32, Value)>,
    prev_start_ev: Option<Value>,
    stop_ev: Option<Value>,
    forced_direction: Direction,
}

impl BeamEngraver {
    /// `valid_start_point`: in main time, or for `Grace_beam_engraver` in
    /// grace time.
    fn valid_start_point(&self, tx: &Tx<'_>) -> bool {
        tx.now().grace.is_zero() != self.grace
    }

    /// `valid_end_point`: as its start; for `Grace_beam_engraver` with a
    /// beam.
    fn valid_end_point(&self, tx: &Tx<'_>) -> bool {
        (!self.grace || self.beam.is_some()) && self.valid_start_point(tx)
    }

    /// `set_melisma`: `beamMelismaBusy` set unless `autoBeaming`.
    fn set_melisma(tx: &mut Tx<'_>, ml: bool) {
        if !is_true(tx.get("autoBeaming")) {
            tx.set("beamMelismaBusy", Value::bool(ml));
        }
    }

    /// `typeset_beam`: the finished beam's right bound its left if it has
    /// none, its last stem given the forced direction.
    fn typeset_beam(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(finished) = self.beams.finished_beam {
            let mut stem = spanner::get_bound(tx.host, finished, RIGHT)?;
            if stem.is_none() {
                stem = spanner::get_bound(tx.host, finished, LEFT)?;
                if let Some(stem) = stem {
                    spanner::set_bound(tx.host, finished, RIGHT, stem)?;
                }
            }

            if let Some(stem) = stem
                && self.forced_direction != 0
            {
                direction::set_grob_direction(tx.host, stem, self.forced_direction);
            }

            self.forced_direction = 0;

            self.beams.typeset_beam(tx)?;
        }
        Ok(())
    }

    /// `acknowledge_stem`.
    fn acknowledge_stem(&mut self, tx: &mut Tx<'_>, stem: Value) -> Run<()> {
        let Some((_, beam)) = self.beam else {
            return Ok(());
        };

        if !self.valid_start_point(tx) {
            return Ok(());
        }

        // It's suboptimal that we don't support callbacks returning ##f,
        // but this makes beams have no effect on "stems" reliably in
        // TabStaff when \tabFullNotation is switched off: the real stencil
        // callback for beams is called quite late in the process, and we
        // don't want to trigger it early.
        if property_data(tx, beam, "stencil") == Value::FALSE {
            return Ok(());
        }

        if stem::get_beam(tx.host, stem)?.is_some() {
            return Ok(());
        }

        let ev = grob::ultimate_event_cause(tx.heap(), stem);
        let Some(ev) = ev.filter(|&ev| in_event_class(tx.heap(), ev, "rhythmic-event")) else {
            return grob::warning(tx.host, stem, "stem must have Rhythmic structure");
        };

        let v = event_property(tx.heap(), ev, "duration");
        let Some(stem_duration) = tx.heap().foreign::<Duration>(v).copied() else {
            return Ok(());
        };
        let durlog = stem_duration.log();
        if durlog <= 2 {
            tx.event_warning(ev, "stem does not fit in beam")?;
            if let Some(prev) = self.prev_start_ev {
                tx.event_warning(prev, "beam was started here")?;
            }
            // don't return, since
            //
            // [r4 c8] can just as well be modern notation.
        }

        if self.forced_direction != 0 {
            direction::set_grob_direction(tx.host, stem, self.forced_direction);
        }

        let durlog = tx.heap().integer(i64::from(durlog));
        grob::set_property(tx.heap(), stem, "duration-log", durlog);
        self.beams.last_added_moment = tx.now();
        self.beams.add_stem(tx, stem, stem_duration)?;
        beam::add_stem(tx.host, beam, stem)
    }

    /// `acknowledge_rest`: the beam's rest offsets chained on the rest's
    /// Y offset, unless it has a `staff-position`.
    fn acknowledge_rest(&mut self, tx: &mut Tx<'_>, rest: Value) -> Run<()> {
        let position = property_data(tx, rest, "staff-position");
        if self.beam.is_some() && !tx.heap().is_number(position) {
            let container = beam::rest_collision_container(tx.host)?;
            grob::chain_offset_callback(tx.host, rest, container, Y)?;
        }
        Ok(())
    }
}

impl Work for BeamEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[self.grace]);
        self.beams.held(h);
        h.maybe_record(self.beam.map(|b| b.0));
        h.num(self.forced_direction.into());
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        let d = to_direction(event_property(tx.heap(), ev, "span-direction"));

        if d == -1 && self.valid_start_point(tx) {
            assign_event_once(tx, &mut self.start_ev, ev)?;

            let updown = to_direction(event_property(tx.heap(), ev, "direction"));
            if updown != 0 {
                self.forced_direction = updown;
            }
        } else if d == 1 && self.valid_end_point(tx) {
            assign_event_once(tx, &mut self.stop_ev, ev)?;
        }
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.start_ev = None;

        if self.beam.is_some() {
            Self::set_melisma(tx, true);
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(start_ev) = self.start_ev {
            if self.beam.is_some() {
                return tx.event_warning(start_ev, "already have a beam");
            }

            Self::set_melisma(tx, true);
            self.prev_start_ev = Some(start_ev);
            let id = tx.make_defined_grob("Beam", lily_engine_music::Cause::Event(start_ev))?;
            self.beam = Some((id, tx.grob_handle(id)));

            self.beams.begin_beam(tx)?;
        }

        self.typeset_beam(tx)?;
        if let (Some(_), Some((id, _))) = (self.stop_ev, self.beam) {
            tx.announce_end_grob(id);
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        if which == 0 {
            self.acknowledge_stem(tx, g)
        } else {
            self.acknowledge_rest(tx, g)
        }
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.stop_ev.is_some() {
            self.beams.finished_beam = self.beam.map(|(_, b)| b);
            self.beams.finished_beam_pattern = self.beams.beam_pattern.take();
            self.beams.finished_beaming_options = self.beams.beaming_options;

            self.stop_ev = None;
            self.beam = None;
            self.typeset_beam(tx)?;
            Self::set_melisma(tx, false);
        }
        self.start_ev = None;
        if self.beam.is_some() {
            tx.stay_awake();
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.typeset_beam(tx)?;
        if let Some((_, beam)) = self.beam {
            if let Some(prev) = self.prev_start_ev {
                tx.event_warning(prev, "unterminated beam")?;
            }

            // we don't typeset it, (we used to, but it was commented out.
            // Reason unknown)
            grob::suicide(tx.heap(), beam);
            self.beams.beam_pattern = None;
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        self.beams.values(out);
        out.extend(self.start_ev);
        out.extend(self.beam.map(|(_, b)| b));
        out.extend(self.prev_start_ev);
        out.extend(self.stop_ev);
    }
}
