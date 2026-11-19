//! LilyPond's `Timing_translator` (`timing-translator.cc`), the one C++
//! translator part 3 runs (04 §13: C1 needs none beyond it), N5's first
//! part (MS2 step 25).
//!
//! It keeps the timing properties in its context (the Score, which it
//! makes `Timing`): `initialize` gives the context its time signature,
//! measure length and beaming from the properties above it and
//! `time-signature-settings.scm`'s functions; each timestep that moves on
//! advances `measurePosition` and the bar numbers, and marks
//! `measureStartNow` at a bar start; `stop_translation_timestep` asks the
//! global loop for the next bar start, which is a timestep even where no
//! music starts. It hears `\bar`, bar checks, `\partial`, `\fine`, volta
//! alternatives and `\polymetric` time signatures, with LilyPond's
//! warnings.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_scheme::{Exception, Heap, Tag, Value};

use crate::dispatcher::event_property;
use crate::duration::Duration;
use crate::rational::{Moment, Rational};
use crate::translator::{
    INITIALIZE, Kind, PRE_PROCESS, PROCESS, START, STOP, TranslatorClass, Tx, Work,
};
use crate::{rational_of, rational_value};

type Run<T> = Result<T, Exception>;

pub(crate) const CLASS: TranslatorClass = TranslatorClass {
    name: "Timing_translator",
    kind: Kind::Translator,
    methods: INITIALIZE | START | PRE_PROCESS | PROCESS | STOP,
    always: true,
    hears: &[
        "alternative-event",
        "bar-event",
        "bar-check-event",
        "fine-event",
        "partial-event",
        "polymetric-time-signature-event",
    ],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(Timing::default())),
    reads: &[
        "alternativeNumberingStyle",
        "beatBase",
        "currentBarNumber",
        "internalBarNumber",
        "measureLength",
        "measurePosition",
        "timeSignature",
    ],
    writes: &[
        "alternativeNumber",
        "beatBase",
        "currentBarNumber",
        "internalBarNumber",
        "measureLength",
        "measurePosition",
        "measureStartNow",
        "timeSignature",
    ],
};

/// The index of each event class in [`CLASS`]' `hears`.
const ALTERNATIVE: usize = 0;
const BAR: usize = 1;
const BAR_CHECK: usize = 2;
const FINE: usize = 3;
const PARTIAL: usize = 4;
const POLYMETRIC: usize = 5;

/// `alternative-dir`'s directions.
const LEFT: i64 = -1;
const CENTER: i64 = 0;
const RIGHT: i64 = 1;

const INFINITE: Moment = Moment {
    main: Rational::INFINITY,
    grace: Rational::ZERO,
};

#[derive(Clone)]
struct Timing {
    measure_start: Moment,
    warned_for_bar_check: bool,
    alt_event: Option<Value>,
    alt_starting_bar_number: i64,
    alt_number: i64,
    alt_reset_enabled: bool,
    bar_check_event: Option<Value>,
    fine_event: Option<Value>,
    partial_event: Option<Value>,
    polymetric: Vec<Value>,
}

impl Default for Timing {
    fn default() -> Timing {
        Timing {
            measure_start: INFINITE,
            warned_for_bar_check: false,
            alt_event: None,
            alt_starting_bar_number: 0,
            alt_number: 0,
            alt_reset_enabled: false,
            bar_check_event: None,
            fine_event: None,
            partial_event: None,
            polymetric: Vec::new(),
        }
    }
}

/// `from_scm<bool>`: only `#t` is true.
fn is_true(v: Value) -> bool {
    v == Value::TRUE
}

/// `from_scm (v, Moment (0))`.
fn moment(heap: &Heap, v: Value) -> Moment {
    heap.foreign::<Moment>(v).copied().unwrap_or_default()
}

/// `measure_length`: `measureLength`, 1 if it is no rational.
fn measure_length(tx: &mut Tx<'_>) -> Rational {
    let v = tx.get("measureLength");
    rational_of(tx.heap(), v).unwrap_or(Rational::new(1, 1))
}

/// The length of an event's `duration`, if it has one.
fn duration(tx: &mut Tx<'_>, ev: Value) -> Option<Duration> {
    let d = event_property(tx.heap(), ev, "duration");
    tx.heap().foreign::<Duration>(d).copied()
}

/// `Moment::to_string`: the main part, and `G` and the grace part if any.
fn moment_text(m: Moment) -> String {
    let mut out = String::new();
    m.main.write(&mut out);
    if !m.grace.is_zero() {
        out.push('G');
        m.grace.write(&mut out);
    }
    out
}

/// `assign_event_once`: `ev` kept in `slot` if it is empty, else LilyPond's
/// warnings for a different event.
pub fn assign_event_once(tx: &mut Tx<'_>, slot: &mut Option<Value>, ev: Value) -> Run<bool> {
    let Some(old) = *slot else {
        *slot = Some(ev);
        return Ok(true);
    };
    if tx.heap().equal(old, ev) {
        return Ok(false);
    }
    let class = |tx: &mut Tx<'_>, e: Value| {
        let list = event_property(tx.heap(), e, "class");
        let heap = tx.heap();
        String::from(
            heap.car(list)
                .and_then(|c| heap.name(c))
                .unwrap_or_default(),
        )
    };
    let (oc, nc) = (class(tx, old), class(tx, ev));
    tx.event_warning(old, &format!("conflict with event: `{oc}'"))?;
    tx.event_warning(ev, &format!("discarding event: `{nc}'"))?;
    Ok(false)
}

impl Work for Timing {
    fn held(&self, h: &mut dyn crate::translator::Held) {
        h.moment(self.measure_start);
        h.flags(&[self.warned_for_bar_check, self.alt_reset_enabled]);
        h.num(self.alt_starting_bar_number);
        h.num(self.alt_number);
    }

    fn connect(&mut self, tx: &mut Tx<'_>) {
        let timing = tx.heap().intern("Timing");
        let heap = tx.host.heap();
        let mut a = tx.t.arena.borrow_mut();
        if !a.is_alias(heap, tx.context, timing) {
            a.add_alias(heap, tx.context, timing);
        }
    }

    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // Below a context the user reaches, one above is to be `Timing`.
        let timing = tx.heap().intern("Timing");
        let missing = {
            let heap = tx.host.heap();
            let a = tx.t.arena.borrow();
            // Global is the one context the user does not reach.
            a.parent(tx.context)
                .filter(|&parent| a.parent(parent).is_some())
                .is_some_and(|parent| {
                    let mut at = Some(parent);
                    while let Some(c) = at {
                        if a.is_alias(heap, c, timing) {
                            return false;
                        }
                        at = a.parent(c);
                    }
                    true
                })
        };
        if missing {
            tx.programming_error("Can't find Timing context template")?;
        }
        let mut bar_number = tx.get("currentBarNumber");
        if !tx.heap().is_integer(bar_number) {
            bar_number = tx.heap().integer(1);
        }
        tx.set("currentBarNumber", bar_number);
        tx.set("internalBarNumber", bar_number);

        let mut time_signature = tx.get("timeSignature");
        if time_signature.tag() != Tag::Pair && time_signature != Value::FALSE {
            tx.programming_error("missing timeSignature")?;
            time_signature = Value::FALSE;
        }
        tx.set("timeSignature", time_signature);

        let mut length = tx.get("measureLength");
        if length == Value::NULL {
            length = tx.call("calc-measure-length", &[time_signature])?;
        }
        tx.set("measureLength", length);
        let position = Moment::new(Rational::ZERO, tx.now().grace);
        let position = tx.heap().wrap(position);
        tx.set("measurePosition", position);
        tx.set("measureStartNow", Value::TRUE);

        let mut settings = tx.get("timeSignatureSettings");
        if settings.tag() != Tag::Pair {
            tx.programming_error("missing timeSignatureSettings")?;
            settings = tx
                .host
                .lily("default-time-signature-settings")
                .unwrap_or(Value::NULL);
        }
        tx.set("timeSignatureSettings", settings);

        let mut exceptions = tx.get("beamExceptions");
        if exceptions.tag() != Tag::Pair {
            exceptions = tx.call("beam-exceptions", &[time_signature, settings])?;
        }
        tx.set("beamExceptions", exceptions);

        let mut base = tx.get("beatBase");
        if rational_of(tx.heap(), base).is_none() {
            base = tx.call("beat-base", &[time_signature, settings])?;
        }
        tx.set("beatBase", base);

        let mut structure = tx.get("beatStructure");
        if structure.tag() != Tag::Pair {
            structure = tx.call("beat-structure", &[base, time_signature, settings])?;
        }
        tx.set("beatStructure", structure);

        let mut submeasures = tx.get("submeasureStructure");
        if submeasures.tag() != Tag::Pair {
            let args = [base, time_signature, settings];
            submeasures = tx.call("calc-submeasure-structure", &args)?;
        }
        tx.set("submeasureStructure", submeasures);

        let half = tx.get("beamHalfMeasure");
        tx.set("beamHalfMeasure", half);
        let auto = tx.get("autoBeaming");
        tx.set("autoBeaming", auto);
        Ok(())
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        match which {
            ALTERNATIVE => {
                // The outermost volta brackets' alternatives number bars.
                let depth = event_property(tx.heap(), ev, "volta-depth");
                if tx.heap().to_i64(depth).unwrap_or(0) == 1 && self.alt_event.is_none() {
                    self.alt_event = Some(ev);
                }
            }
            BAR => {
                let ty = event_property(tx.heap(), ev, "bar-type");
                tx.set("whichBar", ty);
            }
            BAR_CHECK => self.listen_bar_check(tx, ev)?,
            FINE => {
                let mut fine = self.fine_event;
                assign_event_once(tx, &mut fine, ev)?;
                self.fine_event = fine;
            }
            PARTIAL => self.listen_partial(tx, ev)?,
            POLYMETRIC => self.polymetric.push(ev),
            _ => {}
        }
        Ok(())
    }

    fn pre_process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let now = tx.now();
        if let Some(ev) = self.partial_event {
            let length = measure_length(tx);
            if !length.is_finite() {
                let text = "cannot calculate a finite measurePosition from an infinite \
                            measureLength";
                tx.event_warning(ev, text)?;
            }
            // `\partial` in mid piece; at the start, `listen_partial` set it.
            if tx.init_mom().compare(now).is_lt()
                && let Some(d) = duration(tx, ev).and_then(Duration::length)
                && length.is_finite()
                && let Some(main) = length.checked_sub(d)
            {
                let position = tx.heap().wrap(Moment::new(main, now.grace));
                tx.set("measurePosition", position);
            }
        }
        for ev in core::mem::take(&mut self.polymetric) {
            self.check_polymetric(tx, ev)?;
        }

        let mut ok = now.main == self.measure_start.main;
        if is_true(tx.get("measureStartNow")) {
            self.measure_start = now;
            ok = true;
        }
        // One mistake offsets every later bar check: warn once.
        if !ok
            && let Some(ev) = self.bar_check_event
            && !self.warned_for_bar_check
            && !is_true(tx.get("ignoreBarChecks"))
        {
            let position = tx.get("measurePosition");
            let position = moment(tx.heap(), position);
            let text = format!("bar check failed at: {}", moment_text(position));
            tx.event_warning(ev, &text)?;
            self.warned_for_bar_check = true;
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(ev) = self.alt_event else {
            return Ok(());
        };
        let dir = event_property(tx.heap(), ev, "alternative-dir");
        let dir = tx.heap().to_i64(dir).unwrap_or(CENTER);
        match dir {
            LEFT => {
                // One numbering for the whole set, chosen at the first.
                let style = tx.get("alternativeNumberingStyle");
                let name = tx.heap().name(style);
                self.alt_reset_enabled = style.tag() == Tag::Symbol
                    && matches!(name, Some("numbers" | "numbers-with-letters"));
                if self.alt_reset_enabled {
                    let n = tx.get("currentBarNumber");
                    self.alt_starting_bar_number = tx.heap().to_i64(n).unwrap_or(0);
                }
            }
            CENTER if self.alt_reset_enabled => {
                let n = tx.heap().integer(self.alt_starting_bar_number);
                tx.set("currentBarNumber", n);
            }
            _ => {}
        }
        self.alt_number = 0;
        if dir < RIGHT {
            let numbers = event_property(tx.heap(), ev, "volta-numbers");
            let heap = tx.heap();
            let mut p = numbers;
            let mut least = None;
            while let (Some(n), Some(rest)) = (heap.car(p), heap.cdr(p)) {
                if let Some(n) = heap.to_i64(n) {
                    least = Some(least.map_or(n, |l: i64| l.min(n)));
                }
                p = rest;
            }
            if let Some(n) = least {
                self.alt_number = n;
            }
            let n = tx.heap().integer(self.alt_number);
            tx.set("alternativeNumber", n);
        } else {
            tx.set("alternativeNumber", Value::NULL);
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if is_true(tx.get("timing")) && !is_true(tx.get("skipBars")) {
            let position = tx.get("measurePosition");
            let position = moment(tx.heap(), position).main;
            let left = if position.is_negative() {
                Some(-position)
            } else {
                measure_length(tx).checked_sub(position)
            };
            if let Some(left) = left
                && left.compare(Rational::ZERO).is_gt()
                && left.is_finite()
                && let Some(main) = tx.now().main.checked_add(left)
            {
                tx.add_moment(Moment::new(main, Rational::ZERO))?;
            }
        }
        self.alt_event = None;
        self.bar_check_event = None;
        self.polymetric.clear();
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let now = tx.now();
        let mut dt = now.checked_sub(tx.prev()).unwrap_or_default();
        if dt.compare(Moment::default()).is_lt() {
            tx.programming_error("moving backwards in time")?;
            dt = Moment::default();
        } else if dt.main.is_inf() {
            tx.programming_error("moving infinitely to future")?;
            dt = Moment::default();
        }
        if dt.main.is_zero() && dt.grace.is_zero() {
            return Ok(());
        }
        if let Some(ev) = self.fine_event.take() {
            let folded = event_property(tx.heap(), ev, "fine-folded");
            if !is_true(folded) {
                tx.event_warning(ev, "found music after \\fine")?;
            }
        }
        if self.partial_event.take().is_some() {
            tx.unset("partialBusy");
        }
        let position = tx.get("measurePosition");
        let mut position = moment(tx.heap(), position).main;
        let mut start_now = Value::NULL;
        if is_true(tx.get("timing")) {
            let length = measure_length(tx);
            position = position.checked_add(dt.main).unwrap_or(position);
            if !position.compare(length).is_lt() && length.compare(Rational::ZERO).is_gt() {
                let number = |tx: &mut Tx<'_>, name: &str| {
                    let v = tx.get(name);
                    tx.heap().to_i64(v).unwrap_or(0)
                };
                let mut current = number(tx, "currentBarNumber");
                let mut internal = number(tx, "internalBarNumber");
                // One measure, then any more whole ones.
                position = position.checked_sub(length).unwrap_or(position);
                current += 1;
                internal += 1;
                let measures = position
                    .checked_div(length)
                    .filter(|q| q.is_finite())
                    .map_or(0, |q| q.numerator() / q.denominator());
                if let Some(whole) = length.checked_mul(Rational::new(measures, 1))
                    && let Some(rest) = position.checked_sub(whole)
                {
                    position = rest;
                }
                current += measures;
                internal += measures;
                let (current, internal) = (tx.heap().integer(current), tx.heap().integer(internal));
                tx.set("currentBarNumber", current);
                tx.set("internalBarNumber", internal);
                self.measure_start = INFINITE;
            }
            if position.is_zero() && !dt.main.is_zero() && self.measure_start == INFINITE {
                // Arrived at a bar start, not come back to it.
                start_now = Value::TRUE;
                self.measure_start = now;
            }
        }
        let position = tx.heap().wrap(Moment::new(position, now.grace));
        tx.set("measurePosition", position);
        tx.set("measureStartNow", start_now);
        // Set at every timestep, which erases a `\set Timing.whichBar` of
        // the timestep before.
        tx.set("whichBar", Value::NULL);
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.alt_event);
        out.extend(self.bar_check_event);
        out.extend(self.fine_event);
        out.extend(self.partial_event);
        out.extend_from_slice(&self.polymetric);
    }
}

impl Timing {
    fn listen_bar_check(&mut self, tx: &mut Tx<'_>, ev: Value) -> Run<()> {
        // Simultaneous bar checks are normal: one warning at most.
        if self.bar_check_event.is_none() {
            self.bar_check_event = Some(ev);
        }
        // `barCheckSynchronize`, here so that the timing properties change
        // before any translator's `pre_process_music`.
        if tx.now().main != self.measure_start.main
            && is_true(tx.get("deprecatedBarCheckSynchronize"))
        {
            let position = tx.get("measurePosition");
            let position = moment(tx.heap(), position);
            if !position.main.is_zero() && !is_true(tx.get("ignoreBarChecks")) {
                let zero = tx.heap().wrap(Moment::default());
                tx.set("measurePosition", zero);
                tx.set("measureStartNow", Value::TRUE);
            }
        }
        Ok(())
    }

    fn listen_partial(&mut self, tx: &mut Tx<'_>, ev: Value) -> Run<()> {
        let Some(length) = duration(tx, ev).map(Duration::length) else {
            return tx.programming_error("invalid duration in \\partial");
        };
        let mut partial = self.partial_event;
        let assigned = assign_event_once(tx, &mut partial, ev)?;
        self.partial_event = partial;
        if !assigned {
            return Ok(());
        }
        if tx.init_mom().compare(tx.now()).is_lt() {
            // In mid piece: `pre_process_music` sets the position.
            tx.set("partialBusy", Value::TRUE);
            return Ok(());
        }
        let position = tx.get("measurePosition");
        let mut position = moment(tx.heap(), position);
        position.main = -length.unwrap_or(Rational::ZERO);
        let nonzero = !position.main.is_zero() || !position.grace.is_zero();
        let position = tx.heap().wrap(position);
        tx.set("measurePosition", position);
        if nonzero {
            tx.set("measureStartNow", Value::NULL);
        }
        Ok(())
    }

    /// A `\polymetric` time signature's measure length against `Timing`'s
    /// time signature's, its duration's factor applied.
    fn check_polymetric(&mut self, tx: &mut Tx<'_>, ev: Value) -> Run<()> {
        let reference = tx.get("timeSignature");
        let reference = tx.call("calc-measure-length", &[reference])?;
        let depth = tx.heap().root_depth();
        tx.heap().root(reference);
        let ran = (|| {
            let signature = event_property(tx.heap(), ev, "time-signature");
            if signature == Value::NULL {
                return Ok(());
            }
            let length = tx.call("calc-measure-length", &[signature])?;
            let factor = duration(tx, ev).map_or(Rational::new(1, 1), Duration::factor);
            let length = rational_of(tx.heap(), length).and_then(|l| l.checked_mul(factor));
            let wanted = rational_of(tx.heap(), reference);
            if let Some(length) = length
                && Some(length) != wanted
            {
                let mut text = String::new();
                let v = rational_value(tx.heap(), length);
                tx.heap().write(v, &mut text);
                tx.event_warning(ev, &format!("conflicting measure length: {text}"))?;
                let mut text = String::new();
                tx.heap().write(reference, &mut text);
                tx.warning(&format!("measure length in Timing context: {text}"))?;
            }
            Ok(())
        })();
        tx.heap().unroot_to(depth);
        ran
    }
}
