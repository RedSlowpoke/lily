//! `tuplet-engraver.cc` (MS4 step 12): `Tuplet_engraver`, a class of
//! layout's ([`TUPLET_ENGRAVER`]) that shadows the music crate's
//! placeholder of its name (D67, D108), and tuplet-description.cc's
//! constructor and `operator==`.
//!
//! - `listen_tuplet_span` (`:63–92`): a start makes a
//!   [`TupletDescription`] of its event and the moment, unless one equal
//!   to it is new this timestep; a stop ends the innermost open tuplet.
//! - `process_music` (`:94–180`): the tuplets whose stop is now ended, as
//!   in a quoted voice; each ended tuplet's bracket and number given their
//!   right bound (the command or musical column for `tupletFullLength`,
//!   their left for a tuplet of one note), or killed for a tuplet of no
//!   note; the new tuplets opened, each inside the innermost open one, and
//!   `currentTupletDescription` the innermost; each open tuplet without a
//!   bracket given its `TupletBracket` and `TupletNumber`, the number on
//!   the bracket, the bracket one of the `tuplets` of the bracket around
//!   it and the brackets inside it.
//! - The acknowledgers (`:182–227`): each note column a column of every
//!   open bracket and a bound of its number; each script but a dynamic or
//!   a spanner, fingering and string number one of their `scripts`.
//! - `start_translation_timestep` and `finalize` (`:229–253`): the ended
//!   and new tuplets forgotten; at the end, the brackets and numbers of
//!   the tuplets last ended with `tupletFullLengthNote` bound by the
//!   command column, not the musical column after the piece.
//!
//! The engraver runs at every timestep, as LilyPond's does: its
//! `process_music` ends the tuplets a quoted voice stops and sets
//! `currentTupletDescription` at each.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Kind, Moment, PROCESS, Rational, START, TranslatorClass, Tx, Work,
    event_property,
};
use lily_engine_scheme::{Exception, Heap, Value};

use crate::beaming_pattern::TupletDescription;
use crate::clef_engraver::is_true;
use crate::dynamic_engraver::musical_column;
use crate::geometry::{Direction, X, Y};
use crate::ledger_line_engraver::{command_column, set_bound};
use crate::{grob, item, spanner, tuplet_bracket};

type Run<T> = Result<T, Exception>;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

pub static TUPLET_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Tuplet_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS | FINALIZE,
    always: true,
    hears: &["tuplet-span-event"],
    acknowledges: &[
        ("note-column-interface", false),
        ("script-interface", false),
        ("finger-interface", false),
        ("string-number-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(TupletEngraver::default())),
    reads: &["tupletFullLength", "tupletFullLengthNote"],
    writes: &[],
};

/// A `Tuplet_description` as the engraver holds it: its object (the
/// moments, the ratio and the parent, which the beams read), its event,
/// and its bracket and number once made, with the full-length settings
/// read then.
#[derive(Clone, Copy)]
struct Tuplet {
    value: Value,
    event: Value,
    bracket: Option<Value>,
    number: Option<Value>,
    full_length: bool,
    full_length_note: bool,
}

/// `tuplets_`, `new_tuplets_` and `stopped_tuplets_`.
#[derive(Clone, Default)]
struct TupletEngraver {
    tuplets: Vec<Tuplet>,
    new_tuplets: Vec<Tuplet>,
    stopped_tuplets: Vec<Tuplet>,
}

/// `Tuplet_description (ev, now)`: the stop the event's `length` after
/// `now`, in grace time for a tuplet that starts in grace time and is not
/// one of grace notes.
fn description(heap: &mut Heap, ev: Value, now: Moment) -> TupletDescription {
    let v = event_property(heap, ev, "length");
    let length = heap.foreign::<Moment>(v).copied().unwrap_or_default();
    let length = if !now.grace.is_zero() && length.grace.is_zero() {
        Moment::new(Rational::ZERO, length.main)
    } else {
        length
    };
    let stop = now
        .checked_add(length)
        .unwrap_or(Moment::new(Rational::NAN, Rational::NAN));
    let count = |name: &str, heap: &mut Heap| {
        let v = event_property(heap, ev, name);
        heap.to_i64(v)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0)
    };
    TupletDescription {
        start: now,
        stop,
        numerator: count("numerator", heap),
        denominator: count("denominator", heap),
        parent: Value::FALSE,
    }
}

/// `operator== (Tuplet_description, Tuplet_description)`.
fn same(a: &TupletDescription, b: &TupletDescription) -> bool {
    a.start.compare(b.start).is_eq()
        && a.stop.compare(b.stop).is_eq()
        && a.parent == b.parent
        && a.numerator == b.numerator
        && a.denominator == b.denominator
}

impl TupletEngraver {
    /// `add_script_to_all_tuplets`.
    fn add_script_to_all_tuplets(&self, tx: &mut Tx<'_>, script: Value) -> Run<()> {
        for t in &self.tuplets {
            if let Some(bracket) = t.bracket {
                tuplet_bracket::add_script(tx.host, bracket, script)?;
            }
        }
        Ok(())
    }
}

impl Work for TupletEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        for list in [&self.tuplets, &self.new_tuplets, &self.stopped_tuplets] {
            h.num(list.len() as i64);
            for t in list {
                h.flags(&[
                    t.full_length,
                    t.full_length_note,
                    t.bracket.is_some(),
                    t.number.is_some(),
                ]);
            }
        }
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        let dir = event_property(tx.heap(), ev, "span-direction");
        match dir.as_fixnum() {
            Some(-1) => {
                let now = tx.now();
                let new = description(tx.heap(), ev, now);
                let heap = tx.heap();
                for existing in &self.new_tuplets {
                    if heap
                        .foreign::<TupletDescription>(existing.value)
                        .is_some_and(|e| same(e, &new))
                    {
                        // do not add already-existing tuplet
                        return Ok(());
                    }
                }
                let value = heap.wrap(new);
                self.new_tuplets.push(Tuplet {
                    value,
                    event: ev,
                    bracket: None,
                    number: None,
                    full_length: false,
                    full_length_note: false,
                });
            }
            Some(1) => {
                // "No tuplet to end" without a tuplet is only debug output.
                if let Some(t) = self.tuplets.pop() {
                    self.stopped_tuplets.push(t);
                }
            }
            _ => {
                tx.programming_error("direction tuplet-span-event_ invalid.")?;
            }
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        /*
         This may happen if the end of a tuplet is part of a quoted voice.
        */
        let now = tx.now();
        while let Some(&t) = self.tuplets.last() {
            let stop = tx
                .heap()
                .foreign::<TupletDescription>(t.value)
                .map(|d| d.stop);
            if !stop.is_some_and(|s| s.compare(now).is_eq()) {
                break;
            }
            self.tuplets.pop();
            self.stopped_tuplets.push(t);
        }

        for t in self.stopped_tuplets.clone() {
            let (Some(bracket), Some(number)) = (t.bracket, t.number) else {
                continue;
            };
            if let Some(left) = spanner::get_bound(tx.host, bracket, LEFT)? {
                if t.full_length {
                    let col = if t.full_length_note {
                        musical_column(tx)
                    } else {
                        command_column(tx)
                    };
                    set_bound(tx, bracket, RIGHT, col)?;
                    set_bound(tx, number, RIGHT, col)?;
                } else if spanner::get_bound(tx.host, bracket, RIGHT)?.is_none() {
                    // This tuplet only spans one note, e.g.,
                    // \tuplet 3/2 { s8 c'8 s8 }.
                    spanner::set_bound(tx.host, bracket, RIGHT, left)?;
                    spanner::set_bound(tx.host, number, RIGHT, left)?;
                }
            } else {
                // This tuplet spans no notes at all, e.g.,
                // \tuplet 3/2 { s8 s8 s8 }.  Remove it.
                grob::suicide(tx.heap(), bracket);
                grob::suicide(tx.heap(), number);
            }
        }

        for t in core::mem::take(&mut self.new_tuplets) {
            if let Some(parent) = self.tuplets.last() {
                let parent = parent.value;
                if let Some(d) = tx.heap().foreign_mut::<TupletDescription>(t.value) {
                    d.parent = parent;
                }
            }
            self.tuplets.push(t);
        }

        let current = self.tuplets.last().map_or(Value::NULL, |t| t.value);
        tx.set("currentTupletDescription", current);

        for i in (0..self.tuplets.len()).rev() {
            if self.tuplets[i].bracket.is_some() {
                continue;
            }

            let full_length = is_true(tx.get("tupletFullLength"));
            let full_length_note = is_true(tx.get("tupletFullLengthNote"));

            let ev = self.tuplets[i].event;
            let id = tx.make_defined_grob("TupletBracket", Cause::Event(ev))?;
            let bracket = tx.grob_handle(id);
            let id = tx.make_defined_grob("TupletNumber", Cause::Event(ev))?;
            let number = tx.grob_handle(id);
            let t = &mut self.tuplets[i];
            t.full_length = full_length;
            t.full_length_note = full_length_note;
            t.bracket = Some(bracket);
            t.number = Some(number);

            let heap = tx.heap();
            let sym = heap.intern("bracket");
            grob::set_object(heap, number, sym, bracket);
            let sym = heap.intern("tuplet-number");
            grob::set_object(heap, bracket, sym, number);
            grob::set_parent(heap, number, bracket, X);
            grob::set_parent(heap, number, bracket, Y);

            if let Some(inner) = self.tuplets.get(i + 1).and_then(|t| t.bracket) {
                tuplet_bracket::add_tuplet_bracket(tx.host, bracket, inner)?;
            }

            if let Some(outer) = i.checked_sub(1).and_then(|k| self.tuplets[k].bracket) {
                tuplet_bracket::add_tuplet_bracket(tx.host, outer, bracket)?;
            }
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        match which {
            // acknowledge_note_column
            0 => {
                for t in &self.tuplets {
                    if let (Some(bracket), Some(number)) = (t.bracket, t.number) {
                        tuplet_bracket::add_column(tx.host, bracket, g)?;
                        spanner::add_bound_item(tx.host, number, g)?;
                    }
                }
                Ok(())
            }
            // acknowledge_script: an item, and no dynamic.
            1 => {
                if item::is_item(tx.heap(), g)
                    && !grob::has_interface(tx.heap(), g, "dynamic-interface")
                {
                    self.add_script_to_all_tuplets(tx, g)?;
                }
                Ok(())
            }
            // acknowledge_finger and acknowledge_string_number
            _ => self.add_script_to_all_tuplets(tx, g),
        }
    }

    fn start_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.stopped_tuplets.clear();
        /*
         May seem superfluous, but necessary for skipTypesetting.
        */
        self.new_tuplets.clear();
        Ok(())
    }

    /// If tupletFullLengthNote is used, fix up bounds to avoid grobs
    /// extending to the musical column of the last time step, which is
    /// after the end of the piece.
    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let col = command_column(tx);
        for t in &self.stopped_tuplets {
            if t.full_length_note {
                for g in [t.bracket, t.number].into_iter().flatten() {
                    set_bound(tx, g, RIGHT, col)?;
                }
            }
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        for t in self
            .tuplets
            .iter()
            .chain(&self.new_tuplets)
            .chain(&self.stopped_tuplets)
        {
            out.extend([t.value, t.event]);
            out.extend(t.bracket);
            out.extend(t.number);
        }
    }
}
