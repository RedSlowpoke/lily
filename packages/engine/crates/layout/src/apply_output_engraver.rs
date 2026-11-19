//! `Apply_output_engraver` (`scheme-engravers.scm`, 2.26.0's
//! `:2641–2677`, MS3 step 55): LilyPond writes it in Scheme, and the
//! image's is found by name after layout's classes, so this class of
//! layout's ([`APPLY_OUTPUT_ENGRAVER`]) takes its place in `\layout` (D213).
//! It acknowledges every grob, and the image's acknowledger was the
//! translation's costliest Scheme call (D88); what it does is the same.
//!
//! - An `apply-output-event` heard (`:2651–2655`) whose `context-type` the
//!   context is an alias of is kept, the last first.
//! - Each grob acknowledged (`:2657–2667`) is given to each kept event's
//!   `procedure`, with the context of the translator that made it and the
//!   engraver's context, where the event's `symbol` is no symbol or is the
//!   grob's name (`grob::name`).
//! - `stop_translation_timestep` (`:2668–2669`) forgets the events.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Kind, STOP, TranslatorClass, Tx, Work, event_property};
use lily_engine_scheme::{Exception, Tag, Value};

type Run<T> = Result<T, Exception>;

pub static APPLY_OUTPUT_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Apply_output_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    // `make-engraver`'s: a timestep method makes it run always.
    always: true,
    hears: &["apply-output-event"],
    acknowledges: &[("grob-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(ApplyOutputEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `requests`: the events kept this timestep, the last heard first.
#[derive(Clone, Default)]
struct ApplyOutputEngraver {
    requests: Vec<Value>,
}

impl Work for ApplyOutputEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        let kind = event_property(tx.heap(), ev, "context-type");
        let context = tx.t.arena.borrow().handle(tx.context);
        let alias = if kind.tag() == Tag::Symbol {
            let heap = tx.host.heap();
            tx.t.arena.borrow().is_alias(heap, tx.context, kind)
        } else {
            // `ly:context-alias?`'s wrong-type error.
            tx.call("ly:context-alias?", &[context, kind])? != Value::FALSE
        };
        if alias {
            self.requests.insert(0, ev);
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        if self.requests.is_empty() {
            return Ok(());
        }
        let g = tx.grob_handle(grob);
        let (source, context) = {
            let a = tx.t.arena.borrow();
            let origin = a.records.grobs[grob as usize].origin.0;
            (a.handle(origin), a.handle(tx.context))
        };
        for req in self.requests.clone() {
            let symbol = event_property(tx.heap(), req, "symbol");
            let proc = event_property(tx.heap(), req, "procedure");
            if symbol.tag() != Tag::Symbol || symbol == tx.call("grob::name", &[g])? {
                tx.apply(proc, &[g, source, context])?;
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.requests.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend_from_slice(&self.requests);
    }
}
