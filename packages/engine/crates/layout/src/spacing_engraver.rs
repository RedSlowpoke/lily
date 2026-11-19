//! `spacing-engraver.cc` (MS3 step 22): `Spacing_engraver`, a class of
//! layout's ([`SPACING_ENGRAVER`]) that the core passes at its `\layout`
//! call and that a group finds before the music crate's placeholder of the
//! name (D67, D108).
//!
//! - A spacing section heard (`:100–104`, `assign_event_once`).
//! - `process_music` (`:106–114`): a section heard ends the SpacingSpanner;
//!   one is started where none runs (`start_spanner`, `:116–124`), with no
//!   cause, its left bound `currentCommandColumn`.
//! - Each staff spacing and note spacing acknowledged (`:150–162`) added to
//!   the spanner's `wishes`.
//! - Each rhythmic head and rhythmic grob acknowledged
//!   (`add_starter_duration`, `:176–198`), unless a lyric syllable or a
//!   multi-measure grob, and not in grace time, is kept with its end if its
//!   cause is a rhythmic event.
//! - `stop_translation_timestep` (`:200–253`): the spanner, started if none
//!   runs, is the `spacing` of both current columns; with
//!   `proportionalNotationDuration` not negative, that is the musical
//!   column's shortest playing and starter durations, and it is `used`;
//!   else the shortest length of the playing grobs' events and of the kept
//!   ones', the kept ones playing from then on.
//! - `start_translation_timestep` (`:255–273`): the section forgotten, now
//!   taken, the playing grobs that end by now dropped. As the columns are
//!   set at every timestep, the class runs always.
//! - `finalize` (`:132–136`): the spanner's right bound
//!   `currentCommandColumn` (`stop_spanner`, `:138–148`).
//!
//! `playing_durations_` (a `PQueue`) is a list whose minimum is never
//! taken: `start_translation_timestep` drops every entry that ends by now,
//! and `stop_translation_timestep` reads them all. `stopped_durations_`,
//! filled and never read, is left out. Without a column, where LilyPond's
//! would crash, nothing is set on it. A sum that does not fit is infinite.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;

use lily_engine_music::{
    Cause, FINALIZE, INITIALIZE, Kind, Moment, PROCESS, Rational, START, STOP, TranslatorClass, Tx,
    Work, assign_event_once,
};
use lily_engine_scheme::{Exception, Heap, Value};

use crate::dynamic_engraver::musical_column;
use crate::grob;
use crate::grob_array;
use crate::ledger_line_engraver::{command_column, set_bound};
use crate::paper_column_engraver::rational;
use crate::tie_engraver::{event_cause, event_length, in_event_class};

type Run<T> = Result<T, Exception>;

pub static SPACING_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Spacing_engraver",
    kind: Kind::Engraver,
    methods: INITIALIZE | START | PROCESS | STOP | FINALIZE,
    // `stop_translation_timestep` sets the columns of every timestep.
    always: true,
    hears: &["spacing-section-event"],
    acknowledges: &[
        ("staff-spacing-interface", false),
        ("note-spacing-interface", false),
        ("rhythmic-head-interface", false),
        ("rhythmic-grob-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(SpacingEngraver::default())),
    reads: &[
        "currentMusicalColumn",
        "currentCommandColumn",
        "proportionalNotationDuration",
    ],
    writes: &[],
};

/// `Rhythmic_tuple`: a grob by its handle, with its end.
#[derive(Clone)]
struct RhythmicTuple {
    grob: Value,
    end: Rational,
}

/// `playing_durations_`, `now_durations_`, `now_`, `spacing_` and
/// `start_section_`.
#[derive(Clone, Default)]
struct SpacingEngraver {
    playing_durations: Vec<RhythmicTuple>,
    now_durations: Vec<RhythmicTuple>,
    now: Moment,
    spacing: Option<Value>,
    start_section: Option<Value>,
}

/// `to_scm (Rational)`: `±inf.0` for an infinity.
fn rational_value(heap: &mut Heap, r: Rational) -> Value {
    if r.is_inf() {
        let x = if r.is_negative() {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
        return heap.flonum(x);
    }
    let n = heap.integer(r.numerator());
    let d = heap.integer(r.denominator());
    heap.div(n, d).unwrap_or(Value::FALSE)
}

/// `std::min (a, b)`.
fn min(a: Rational, b: Rational) -> Rational {
    if b.compare(a) == Ordering::Less { b } else { a }
}

/// `get_event_length (ev).main_part_` of the grob's cause, if it has one.
fn main_length(tx: &mut Tx<'_>, g: Value) -> Run<Option<Rational>> {
    Ok(event_cause(tx, g)?.map(|ev| event_length(tx.heap(), ev, Moment::default()).main))
}

impl SpacingEngraver {
    fn start_spanner(&mut self, tx: &mut Tx<'_>) -> Run<Value> {
        let id = tx.make_defined_grob("SpacingSpanner", Cause::None)?;
        let sp = tx.grob_handle(id);
        self.spacing = Some(sp);
        let col = command_column(tx);
        set_bound(tx, sp, -1, col)?;
        Ok(sp)
    }

    fn stop_spanner(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(sp) = self.spacing.take() {
            let col = command_column(tx);
            set_bound(tx, sp, 1, col)?;
        }
        Ok(())
    }

    fn add_starter_duration(&mut self, tx: &mut Tx<'_>, g: Value) -> Run<()> {
        if grob::has_interface(tx.heap(), g, "lyric-syllable-interface")
            || grob::has_interface(tx.heap(), g, "multi-measure-interface")
        {
            return Ok(());
        }

        // Only pay attention to durations that are not grace notes.
        if self.now.grace.is_zero()
            && let Some(r) = event_cause(tx, g)?
            && in_event_class(tx.heap(), r, "rhythmic-event")
        {
            let len = event_length(tx.heap(), r, Moment::default()).main;
            let end = self.now.main.checked_add(len).unwrap_or(Rational::INFINITY);
            self.now_durations.push(RhythmicTuple { grob: g, end });
        }
        Ok(())
    }
}

impl Work for SpacingEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        for list in [&self.playing_durations, &self.now_durations] {
            h.num(list.len() as i64);
            for t in list {
                h.rational(t.end);
            }
        }
        h.moment(self.now);
    }

    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.now = tx.now();
        Ok(())
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        assign_event_once(tx, &mut self.start_section, ev).map(drop)
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.start_section.is_some() && self.spacing.is_some() {
            self.stop_spanner(tx)?;
        }

        if self.spacing.is_none() {
            self.start_spanner(tx)?;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        if which >= 2 {
            return self.add_starter_duration(tx, g);
        }
        if let Some(sp) = self.spacing {
            let wishes = tx.heap().intern("wishes");
            grob_array::add_grob(tx.host, sp, wishes, g)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let musical = musical_column(tx);

        let sp = match self.spacing {
            Some(sp) => sp,
            None => self.start_spanner(tx)?,
        };

        let spacing = tx.heap().intern("spacing");
        for col in [musical, command_column(tx)].into_iter().flatten() {
            grob::set_object(tx.heap(), col, spacing, sp);
        }

        let proportional = tx.get("proportionalNotationDuration");
        let proportional = rational(tx.heap(), proportional, -Rational::INFINITY);
        if proportional.compare(Rational::ZERO) != Ordering::Less {
            if let Some(col) = musical {
                let scm = rational_value(tx.heap(), proportional);
                grob::set_property(tx.heap(), col, "shortest-playing-duration", scm);
                grob::set_property(tx.heap(), col, "shortest-starter-duration", scm);
                grob::set_property(tx.heap(), col, "used", Value::TRUE);
            }
            return Ok(());
        }

        let mut shortest_playing = Rational::INFINITY;
        for t in &self.playing_durations {
            if let Some(len) = main_length(tx, t.grob)? {
                shortest_playing = min(shortest_playing, len);
            }
        }
        let mut starter = Rational::INFINITY;

        for nd in core::mem::take(&mut self.now_durations) {
            let len = main_length(tx, nd.grob)?.unwrap_or_default();
            if !len.is_zero() {
                starter = min(starter, len);
                self.playing_durations.push(nd);
            }
        }

        shortest_playing = min(shortest_playing, starter);

        if let Some(col) = musical {
            let playing = rational_value(tx.heap(), shortest_playing);
            grob::set_property(tx.heap(), col, "shortest-playing-duration", playing);
            let starter = rational_value(tx.heap(), starter);
            grob::set_property(tx.heap(), col, "shortest-starter-duration", starter);
        }
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.start_section = None;

        self.now = tx.now();
        let now = self.now.main;
        self.playing_durations
            .retain(|t| t.end.compare(now) == Ordering::Greater);
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.stop_spanner(tx)
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.playing_durations.iter().map(|t| t.grob));
        out.extend(self.now_durations.iter().map(|t| t.grob));
        out.extend(self.spacing);
        out.extend(self.start_section);
    }
}
