//! `dots-engraver.cc` (MS3 step 16): `Dots_engraver`, a class of layout's
//! ([`DOTS_ENGRAVER`]) that the core passes at its `\layout` call and that
//! a group finds before the music crate's placeholder of the name (D67,
//! D108).
//!
//! Each rhythmic head acknowledged (`acknowledge_rhythmic_head`, `:39–58`)
//! whose cause is an event, that has no dots yet and whose duration has
//! dots: `Dots`, caused by the head, set as its `dot`
//! (`Rhythmic_head::set_dots`) and put on it in Y.

use alloc::boxed::Box;

use lily_engine_music::{
    Cause, Duration, Kind, StreamEvent, TranslatorClass, Tx, Work, event_property, grob_of,
};
use lily_engine_scheme::Exception;

use crate::geometry::Y;
use crate::grob;
use crate::grob_array;
use crate::rhythmic_head;

type Run<T> = Result<T, Exception>;

pub static DOTS_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Dots_engraver",
    kind: Kind::Engraver,
    methods: 0,
    always: false,
    hears: &[],
    acknowledges: &[("rhythmic-head-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(DotsEngraver)),
    reads: &[],
    writes: &[],
};

#[derive(Clone)]
struct DotsEngraver;

impl Work for DotsEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let note = tx.grob_handle(grob);
        let cause = grob::property(tx.host, note, "cause")?;
        if tx.heap().foreign::<StreamEvent>(cause).is_none() {
            return Ok(());
        }
        let sym = tx.heap().intern("dot");
        let dot = grob_array::object(tx.host, note, sym)?;
        if grob_of(tx.heap(), dot).is_some() {
            return Ok(());
        }
        let d = event_property(tx.heap(), cause, "duration");
        let dots = tx.heap().foreign::<Duration>(d).map_or(0, |d| d.dots());
        if dots != 0 {
            let id = tx.make_defined_grob("Dots", Cause::Grob(grob))?;
            let d = tx.grob_handle(id);
            rhythmic_head::set_dots(tx.host, note, d);
            grob::set_parent(tx.heap(), d, note, Y);
        }
        Ok(())
    }
}
