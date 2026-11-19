//! `pitched-trill-engraver.cc` (MS8 step 8, part 1, 208 lines):
//! `Pitched_trill_engraver`, a class of layout's
//! ([`PITCHED_TRILL_ENGRAVER`]) that shadows the music crate's placeholder
//! of its name (D67, D108). The trill line is `Trill_spanner_engraver`'s,
//! a Scheme engraver of LilyPond's image (OPEN A19).
//!
//! - A `TrillSpanner` acknowledged (`:87–95`) whose cause is a starting
//!   `trill-span-event` with a `pitch` makes the trill (`:97–169`): a
//!   `TrillPitchHead` at the pitch's staff position from `middleCPosition`
//!   in a `TrillPitchGroup`; a `TrillPitchAccidental` of its alteration,
//!   the head's `accidental-grob`, unless `localAlterations` has the
//!   pitch's alteration in this measure and it is altered and not
//!   `force-accidental`; a `TrillPitchParentheses` caused by the head,
//!   holding it. Each is caused by the event.
//! - The note heads, dots, stems and flags acknowledged are the group's
//!   support at `stop_translation_timestep` (`:171–182`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, Pitch, Rational, STOP, TranslatorClass, Tx, Work, event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::accidental_engraver::{measure_number, rational_value};
use crate::geometry::{X, Y};
use crate::note_heads_engraver::steps;
use crate::paper_column_engraver::rational;
use crate::tie_engraver::{event_cause, in_event_class};
use crate::{axis_group, grob, grob_array, side_position};

type Run<T> = Result<T, Exception>;

pub static PITCHED_TRILL_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Pitched_trill_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    always: false,
    hears: &[],
    acknowledges: &[
        ("note-head-interface", false),
        ("dots-interface", false),
        ("stem-interface", false),
        ("flag-interface", false),
        ("trill-spanner-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(PitchedTrillEngraver::default())),
    reads: &[],
    writes: &[],
};

/// The acknowledgers' indices in [`PITCHED_TRILL_ENGRAVER`].
const TRILL_SPANNER: usize = 4;

/// `trill_head_`, `trill_group_`, `trill_accidental_` and
/// `trill_parentheses_` by their handles, and `heads_`.
#[derive(Clone, Default)]
struct PitchedTrillEngraver {
    trill_head: Option<Value>,
    trill_group: Option<Value>,
    trill_accidental: Option<Value>,
    trill_parentheses: Option<Value>,
    heads: Vec<Value>,
}

/// `ly_assoc`: the first entry of `alist` whose car is `equal?` to `key`.
fn assoc(heap: &lily_engine_scheme::Heap, key: Value, mut alist: Value) -> Option<Value> {
    while let (Some(entry), Some(rest)) = (heap.car(alist), heap.cdr(alist)) {
        if heap.car(entry).is_some_and(|k| heap.equal(k, key)) {
            return Some(entry);
        }
        alist = rest;
    }
    None
}

impl PitchedTrillEngraver {
    fn make_trill(&mut self, tx: &mut Tx<'_>, ev: Value) -> Run<()> {
        let scm_pitch = event_property(tx.heap(), ev, "pitch");
        let Some(p) = tx.heap().foreign::<Pitch>(scm_pitch).copied() else {
            return Ok(());
        };

        let keysig = tx.get("localAlterations");

        let heap = tx.heap();
        let octave = heap.integer(i64::from(p.octave));
        let notename = heap.integer(i64::from(p.notename));
        let key = heap.cons(octave, notename);

        let bn = measure_number(tx, tx.context);

        let heap = tx.heap();
        let mut handle = assoc(heap, key, keysig);
        if let Some(h) = handle {
            let rest = heap.cdr(h).unwrap_or(Value::NULL);
            let alt = heap.car(rest).unwrap_or(Value::NULL);
            let rest = heap.cdr(rest).unwrap_or(Value::NULL);
            let bar = heap.car(rest).unwrap_or(Value::NULL);
            let bar = if heap.is_number(bar) {
                heap.to_i64(bar).unwrap_or(0)
            } else {
                0
            };
            let same_bar = bn == bar;
            let same_alt = p.alteration == rational(heap, alt, Rational::ZERO);

            if !same_bar || !same_alt {
                handle = None;
            }
        }

        let print_acc = handle.is_none()
            || p.alteration == Rational::ZERO
            || event_property(tx.heap(), ev, "force-accidental") == Value::TRUE;

        if self.trill_head.is_some() {
            tx.programming_error("already have a trill head.")?;
            self.trill_head = None;
        }

        let head_id = tx.make_defined_grob("TrillPitchHead", Cause::Event(ev))?;
        let head = tx.grob_handle(head_id);
        self.trill_head = Some(head);
        let c0 = tx.get("middleCPosition");
        let c0 = if tx.heap().is_number(c0) {
            crate::stem::to_int(tx.heap(), c0)?
        } else {
            0
        };
        let pos = steps(tx.heap(), &p).wrapping_add(c0);
        let pos = tx.heap().integer(i64::from(pos));
        grob::set_property(tx.heap(), head, "staff-position", pos);

        let id = tx.make_defined_grob("TrillPitchGroup", Cause::Event(ev))?;
        let group = tx.grob_handle(id);
        self.trill_group = Some(group);

        axis_group::add_element(tx.host, group, head)?;

        if print_acc {
            let id = tx.make_defined_grob("TrillPitchAccidental", Cause::Event(ev))?;
            let acc = tx.grob_handle(id);
            self.trill_accidental = Some(acc);

            // fixme: naming -> alterations
            let alteration = rational_value(tx.heap(), p.alteration);
            grob::set_property(tx.heap(), acc, "alteration", alteration);
            side_position::add_support(tx.host, acc, head)?;

            let sym = tx.heap().intern("accidental-grob");
            grob::set_object(tx.heap(), head, sym, acc);
            grob::set_parent(tx.heap(), acc, head, Y);
            axis_group::add_element(tx.host, group, acc)?;
        }

        let id = tx.make_defined_grob("TrillPitchParentheses", Cause::Grob(head_id))?;
        let parens = tx.grob_handle(id);
        self.trill_parentheses = Some(parens);
        let sym = tx.heap().intern("elements");
        grob_array::add_grob(tx.host, parens, sym, head)?;
        grob::set_parent(tx.heap(), parens, head, X);
        grob::set_parent(tx.heap(), parens, head, Y);
        axis_group::add_element(tx.host, group, parens)?;
        Ok(())
    }
}

impl Work for PitchedTrillEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        if which != TRILL_SPANNER {
            // `acknowledge_note_head`, `_dots`, `_stem`, `_flag`.
            self.heads.push(g);
            return Ok(());
        }
        let Some(ev) = event_cause(tx, g)? else {
            return Ok(());
        };
        let heap = tx.heap();
        let starts = in_event_class(heap, ev, "trill-span-event")
            && event_property(heap, ev, "span-direction").as_fixnum() == Some(-1);
        let pitch = event_property(tx.heap(), ev, "pitch");
        if starts && tx.heap().foreign::<Pitch>(pitch).is_some() {
            self.make_trill(tx, ev)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(group) = self.trill_group {
            for &head in &self.heads {
                side_position::add_support(tx.host, group, head)?;
            }
        }

        self.heads.clear();
        self.trill_head = None;
        self.trill_group = None;
        self.trill_accidental = None;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.trill_head);
        out.extend(self.trill_group);
        out.extend(self.trill_accidental);
        out.extend(self.trill_parentheses);
        out.extend(&self.heads);
    }
}
