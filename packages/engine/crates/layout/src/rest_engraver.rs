//! `rest-engraver.cc` (MS3 step 16): `Rest_engraver`, a class of layout's
//! ([`REST_ENGRAVER`]) that the core passes at its `\layout` call and that
//! a group finds before the music crate's placeholder of the name (D67,
//! D108).
//!
//! The rest heard once a timestep (`assign_event_once`, `:82–86`); at
//! `process_music` (`:62–80`) a `Rest` of it, its cause the rest; a rest
//! with a pitch gets the pitch's steps as its `staff-position`,
//! `middleCPosition` added if it is a number. `start_translation_timestep`
//! (`:54–60`) forgets them: the engraver stays awake after a timestep
//! that heard a rest so that it runs.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, PROCESS, Pitch, START, TranslatorClass, Tx, Work, assign_event_once,
    event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::grob;
use crate::note_heads_engraver::steps;
use crate::stem::to_int;

type Run<T> = Result<T, Exception>;

pub static REST_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Rest_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS,
    always: false,
    hears: &["rest-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(RestEngraver::default())),
    reads: &["middleCPosition"],
    writes: &[],
};

/// `rest_event_` and `rest_`, by their handles; `dot_` is never set.
#[derive(Clone, Default)]
struct RestEngraver {
    event: Option<Value>,
    rest: Option<Value>,
}

impl Work for RestEngraver {
    fn start_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.event = None;
        self.rest = None;
        Ok(())
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        assign_event_once(tx, &mut self.event, ev)?;
        tx.stay_awake();
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(ev) = self.event.filter(|_| self.rest.is_none()) else {
            return Ok(());
        };
        let id = tx.make_defined_grob("Rest", Cause::Event(ev))?;
        let rest = tx.grob_handle(id);
        self.rest = Some(rest);
        let pitch = event_property(tx.heap(), ev, "pitch");
        if let Some(p) = tx.heap().foreign::<Pitch>(pitch).copied() {
            let mut pos = steps(tx.heap(), &p);
            let c0 = tx.get("middleCPosition");
            if tx.heap().is_number(c0) {
                pos = pos.wrapping_add(to_int(tx.heap(), c0)?);
            }
            let pos = tx.heap().integer(i64::from(pos));
            grob::set_property(tx.heap(), rest, "staff-position", pos);
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.event);
        out.extend(self.rest);
    }
}
