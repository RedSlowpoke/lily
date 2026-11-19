//! The percent repeats' engravers (MS8 step 7, part 2), classes of layout's
//! that shadow the music crate's placeholders of their names (D67, D108):
//!
//! - `Percent_repeat_engraver` (`percent-repeat-engraver.cc`, 182 lines,
//!   [`PERCENT_REPEAT_ENGRAVER`]): a `PercentRepeat` spanner for each
//!   `percent-event`, from the first command column of the timestep's
//!   grace group to the one where the event's length ends, and a
//!   `PercentRepeatCounter` over it where `countPercentRepeats` and
//!   `repeatCountVisibility` say so. It runs at every timestep, as
//!   LilyPond's does, since a measure's grace notes come before the event.
//! - `Double_percent_repeat_engraver` (`double-percent-repeat-engraver.cc`,
//!   129 lines, [`DOUBLE_PERCENT_REPEAT_ENGRAVER`]): a `DoublePercentRepeat`
//!   one measure after a `double-percent-event`, with its counter, the
//!   Score's `forbidBreak` set there.
//! - `Slash_repeat_engraver` (`slash-repeat-engraver.cc`, 95 lines,
//!   [`SLASH_REPEAT_ENGRAVER`]): a `RepeatSlash` for each
//!   `repeat-slash-event`, a `DoubleRepeatSlash` for one of `slash-count` 0.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Kind, Moment, PRE_PROCESS, PROCESS, Rational, STOP, TranslatorClass, Tx, Work,
    assign_event_once, event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::geometry::{X, Y};
use crate::grob;
use crate::ledger_line_engraver::{command_column, set_bound};
use crate::paper_column_engraver::{find_score_context, rational};
use crate::side_position;

type Run<T> = Result<T, Exception>;

pub static PERCENT_REPEAT_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Percent_repeat_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    always: true,
    hears: &["percent-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(PercentRepeatEngraver::default())),
    reads: &[
        "countPercentRepeats",
        "currentCommandColumn",
        "repeatCountVisibility",
    ],
    writes: &[],
};

pub static DOUBLE_PERCENT_REPEAT_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Double_percent_repeat_engraver",
    kind: Kind::Engraver,
    methods: PRE_PROCESS | PROCESS | STOP,
    always: false,
    hears: &["double-percent-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(DoublePercentRepeatEngraver::default())),
    reads: &[
        "countPercentRepeats",
        "measureLength",
        "repeatCountVisibility",
    ],
    writes: &["forbidBreak"],
};

pub static SLASH_REPEAT_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Slash_repeat_engraver",
    kind: Kind::Engraver,
    methods: PROCESS,
    always: false,
    hears: &["repeat-slash-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(SlashRepeatEngraver::default())),
    reads: &[],
    writes: &[],
};

/// The event's `length`, a moment (`get_event_length`), 0 without one.
fn length(tx: &mut Tx<'_>, ev: Value) -> Moment {
    let heap = tx.heap();
    let v = event_property(heap, ev, "length");
    heap.foreign::<Moment>(v).copied().unwrap_or_default()
}

/// `check_repeat_count_visibility`: whether `repeatCountVisibility` is a
/// procedure that is true of `count` and the context.
fn count_visible(tx: &mut Tx<'_>, count: Value) -> Run<bool> {
    let proc = tx.get("repeatCountVisibility");
    if !tx.heap().is_procedure(proc) {
        return Ok(false);
    }
    let context = tx.t.arena.borrow().handle(tx.context);
    Ok(tx.apply(proc, &[count, context])? != Value::FALSE)
}

/// The event's `repeat-count` if a counter is to be printed for it:
/// `countPercentRepeats` and `repeatCountVisibility` say so.
fn counted(tx: &mut Tx<'_>, ev: Value) -> Run<Option<Value>> {
    let count = event_property(tx.heap(), ev, "repeat-count");
    if count == Value::NULL || tx.get("countPercentRepeats") != Value::TRUE {
        return Ok(None);
    }
    let depth = tx.heap().root_depth();
    tx.heap().root(count);
    let visible = count_visible(tx, count);
    tx.heap().unroot_to(depth);
    Ok(visible?.then_some(count))
}

/// A counter of `count` made as `name`, caused by `ev`: its text the
/// count in decimal, supported by `sign` and its parent on both axes.
fn make_counter(
    tx: &mut Tx<'_>,
    name: &str,
    ev: Value,
    count: Value,
    sign: Value,
) -> Run<(Value, u32)> {
    let id = tx.make_defined_grob(name, Cause::Event(ev))?;
    let counter = tx.grob_handle(id);
    let depth = tx.heap().root_depth();
    tx.heap().root(counter);
    let ten = tx.heap().integer(10);
    let text = tx.call("number->string", &[count, ten]);
    tx.heap().unroot_to(depth);
    grob::set_property(tx.heap(), counter, "text", text?);
    side_position::add_support(tx.host, counter, sign)?;
    grob::set_parent(tx.heap(), counter, sign, Y);
    grob::set_parent(tx.heap(), counter, sign, X);
    Ok((counter, id))
}

/// `Percent_repeat_engraver`'s members, the grobs by handle and record.
#[derive(Clone, Default)]
struct PercentRepeatEngraver {
    percent_event: Option<Value>,
    /// The global moment where the running percent ends.
    stop_mom: Moment,
    percent: Option<(Value, u32)>,
    percent_counter: Option<(Value, u32)>,
    /// The first command column of the timestep's grace group, and the
    /// moment it was taken at (`-1` before the first).
    first_command_column: Option<Value>,
    command_moment: Option<Moment>,
}

impl PercentRepeatEngraver {
    /// The running percent and its counter killed.
    fn kill(&mut self, tx: &mut Tx<'_>) {
        if let Some((percent, _)) = self.percent.take() {
            grob::suicide(tx.heap(), percent);
        }
        if let Some((counter, _)) = self.percent_counter.take() {
            grob::suicide(tx.heap(), counter);
        }
    }
}

impl Work for PercentRepeatEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.moment(self.stop_mom);
        h.maybe_record(self.percent.map(|p| p.1));
        h.maybe_record(self.percent_counter.map(|p| p.1));
        h.flags(&[self.command_moment.is_some()]);
        if let Some(m) = self.command_moment {
            h.moment(m);
        }
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        assign_event_once(tx, &mut self.percent_event, ev).map(drop)
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // Maintain first_command_column as the first non-musical column in
        // the grace group.
        let now = tx.now();
        if self.command_moment.is_none_or(|m| m.main != now.main) {
            self.first_command_column = command_column(tx);
            self.command_moment = Some(now);
        }

        // Stop a running percent if it has reached completion.
        if self.percent.is_some() && self.stop_mom.main == now.main {
            let col = self.first_command_column;
            if let Some((percent, _)) = self.percent.take() {
                set_bound(tx, percent, 1, col)?;
            }
            if let Some((counter, _)) = self.percent_counter.take() {
                set_bound(tx, counter, 1, col)?;
            }
        } else if let (Some(_), Some(ev)) = (self.percent, self.percent_event) {
            tx.event_warning(
                ev,
                "percent repeat started while another already in progress",
            )?;
            self.kill(tx);
        }

        // Start a new percent if asked.
        if let Some(ev) = self.percent_event {
            let len = length(tx, ev);
            self.stop_mom = now.checked_add(len).unwrap_or(now);
            tx.add_moment(self.stop_mom)?;
            let id = tx.make_defined_grob("PercentRepeat", Cause::Event(ev))?;
            let percent = tx.grob_handle(id);
            self.percent = Some((percent, id));
            let col = self.first_command_column;
            set_bound(tx, percent, -1, col)?;

            self.percent_counter = match counted(tx, ev)? {
                Some(count) => {
                    let (counter, id) =
                        make_counter(tx, "PercentRepeatCounter", ev, count, percent)?;
                    set_bound(tx, counter, -1, col)?;
                    Some((counter, id))
                }
                None => None,
            };
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.percent_event = None;
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((percent, _)) = self.percent {
            grob::programming_error(
                tx.host,
                percent,
                "percent end moment should have been processed",
            )?;
            self.kill(tx);
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.percent_event);
        out.extend(self.percent.map(|p| p.0));
        out.extend(self.percent_counter.map(|p| p.0));
        out.extend(self.first_command_column);
    }
}

/// `Double_percent_repeat_engraver`'s members.
#[derive(Clone, Default)]
struct DoublePercentRepeatEngraver {
    percent_event: Option<Value>,
    /// The global moment where the sign is printed, a measure on.
    start_mom: Moment,
    should_print_double_percent: bool,
}

impl Work for DoublePercentRepeatEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.moment(self.start_mom);
        h.flags(&[self.should_print_double_percent]);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        if assign_event_once(tx, &mut self.percent_event, ev)? {
            let v = tx.get("measureLength");
            let measure = rational(tx.heap(), v, Rational::int(1));
            let now = tx.now();
            self.start_mom = now
                .checked_add(Moment::new(measure, Rational::ZERO))
                .unwrap_or(now);
            tx.add_moment(self.start_mom)?;
        }
        Ok(())
    }

    fn pre_process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.should_print_double_percent =
            self.percent_event.is_some() && tx.now().main == self.start_mom.main;
        // Prevent breaks over the percent sign.
        if self.should_print_double_percent {
            let score = find_score_context(tx);
            let sym = tx.heap().intern("forbidBreak");
            tx.t.arena.borrow_mut().set(score, sym, Value::TRUE);
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(ev) = self
            .percent_event
            .filter(|_| self.should_print_double_percent)
        else {
            return Ok(());
        };
        let id = tx.make_defined_grob("DoublePercentRepeat", Cause::Event(ev))?;
        let double_percent = tx.grob_handle(id);
        if let Some(count) = counted(tx, ev)? {
            make_counter(tx, "DoublePercentRepeatCounter", ev, count, double_percent)?;
        }
        self.percent_event = None;
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // Awake until the measure on, where the sign is printed.
        if self.percent_event.is_some() {
            tx.stay_awake();
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.percent_event);
    }
}

/// `Slash_repeat_engraver`'s member.
#[derive(Clone, Default)]
struct SlashRepeatEngraver {
    slash: Option<Value>,
}

impl Work for SlashRepeatEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        assign_event_once(tx, &mut self.slash, ev).map(drop)
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(ev) = self.slash.take() {
            let count = event_property(tx.heap(), ev, "slash-count");
            let name = if tx.heap().to_i64(count) == Some(0) {
                "DoubleRepeatSlash"
            } else {
                "RepeatSlash"
            };
            tx.make_defined_grob(name, Cause::Event(ev))?;
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.slash);
    }
}
