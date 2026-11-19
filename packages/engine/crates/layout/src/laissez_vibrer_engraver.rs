//! `laissez-vibrer-engraver.cc` (MS4 step 10): `Laissez_vibrer_engraver`,
//! a class of layout's ([`LAISSEZ_VIBRER_ENGRAVER`]) that shadows the music
//! crate's placeholder of its name (D67, D108): a `LaissezVibrerTie` at
//! each note head of a `laissez-vibrer-event` heard, or of one among its
//! note's articulations (a single note of a chord), each its timestep's
//! `LaissezVibrerTieColumn`'s child on Y and one of its `ties`, with the
//! event's direction. And `repeat-tie-engraver.cc` (MS8 step 7):
//! `Repeat_tie_engraver`, LilyPond's subclass, [`REPEAT_TIE_ENGRAVER`], the
//! same for a `repeat-tie-event` with a `RepeatTie` and its
//! `RepeatTieColumn` (`is_my_event_class`, `make_my_tie`, `make_my_column`,
//! here a [`Semi`]).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, STOP, StreamEvent, TranslatorClass, Tx, Work, assign_event_once, event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::geometry::Y;
use crate::grob;
use crate::grob_array;
use crate::tie_engraver::{event_cause, in_event_class};

type Run<T> = Result<T, Exception>;

pub static LAISSEZ_VIBRER_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Laissez_vibrer_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    always: false,
    hears: &["laissez-vibrer-event"],
    acknowledges: &[("note-head-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(LaissezVibrerEngraver::of(&LAISSEZ_VIBRER))),
    reads: &[],
    writes: &[],
};

pub static REPEAT_TIE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Repeat_tie_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    always: false,
    hears: &["repeat-tie-event"],
    acknowledges: &[("note-head-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(LaissezVibrerEngraver::of(&REPEAT_TIE))),
    reads: &[],
    writes: &[],
};

/// What LilyPond's subclass overrides: `is_my_event_class`'s class,
/// `make_my_tie`'s grob and `make_my_column`'s.
pub struct Semi {
    event: &'static str,
    tie: &'static str,
    column: &'static str,
}

static LAISSEZ_VIBRER: Semi = Semi {
    event: "laissez-vibrer-event",
    tie: "LaissezVibrerTie",
    column: "LaissezVibrerTieColumn",
};

static REPEAT_TIE: Semi = Semi {
    event: "repeat-tie-event",
    tie: "RepeatTie",
    column: "RepeatTieColumn",
};

/// `event_`, `lv_column_` and `lv_ties_`, the grobs by their handles.
#[derive(Clone)]
struct LaissezVibrerEngraver {
    semi: &'static Semi,
    event: Option<Value>,
    lv_column: Option<Value>,
    lv_ties: Vec<Value>,
}

impl LaissezVibrerEngraver {
    fn of(semi: &'static Semi) -> Self {
        Self {
            semi,
            event: None,
            lv_column: None,
            lv_ties: Vec::new(),
        }
    }
}

impl Work for LaissezVibrerEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        assign_event_once(tx, &mut self.event, ev)?;
        Ok(())
    }

    /// `acknowledge_note_head`: the heard event for every head, or one of
    /// the head's note's articulations for it alone.
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let head = tx.grob_handle(grob);
        let mut tie_ev = self.event;
        let note_ev = event_cause(tx, head)?;
        if tie_ev.is_none()
            && let Some(note_ev) = note_ev
            && in_event_class(tx.heap(), note_ev, "note-event")
        {
            let heap = tx.heap();
            let mut s = event_property(heap, note_ev, "articulations");
            while let (None, Some(ev), Some(rest)) = (tie_ev, heap.car(s), heap.cdr(s)) {
                if heap.foreign::<StreamEvent>(ev).is_some()
                    && in_event_class(heap, ev, self.semi.event)
                {
                    tie_ev = Some(ev);
                }
                s = rest;
            }
        }

        let Some(cause) = tie_ev else {
            return Ok(());
        };

        let id = tx.make_defined_grob(self.semi.tie, Cause::Event(cause))?;
        let lv_tie = tx.grob_handle(id);

        let column = match self.lv_column {
            Some(c) => c,
            None => {
                let c = tx.make_defined_grob(self.semi.column, Cause::Grob(id))?;
                let c = tx.grob_handle(c);
                self.lv_column = Some(c);
                c
            }
        };

        let sym = tx.heap().intern("note-head");
        grob::set_object(tx.heap(), lv_tie, sym, head);

        let sym = tx.heap().intern("ties");
        grob_array::add_grob(tx.host, column, sym, lv_tie)?;

        let d = event_property(tx.heap(), cause, "direction");
        if matches!(d.as_fixnum(), Some(-1..=1)) {
            grob::set_property(tx.heap(), lv_tie, "direction", d);
        }

        grob::set_parent(tx.heap(), lv_tie, column, Y);

        self.lv_ties.push(lv_tie);
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.event = None;
        self.lv_column = None;
        self.lv_ties.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.event);
        out.extend(self.lv_column);
        out.extend(&self.lv_ties);
    }
}
