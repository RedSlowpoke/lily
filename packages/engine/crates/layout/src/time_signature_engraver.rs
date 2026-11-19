//! `time-signature-engraver.cc` (MS3 step 17): `Time_signature_engraver`, a
//! class of layout's ([`TIME_SIGNATURE_ENGRAVER`]) that the core passes at
//! its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! A `\time` heard (`listen_reference_time_signature`, `:87–91`) and a
//! `\polymetric` one whose `time-signature` equals the context's
//! `timeSignature` (`listen_polymetric_time_signature`, `:65–85`) are the
//! cause. `process_music` (`:93–122`) makes the `TimeSignature` where
//! `timeSignature` is not `eq?` to the last one made of and is a pair or
//! `#f`: its `time-signature` the spec unless an override set one, the
//! first one's `break-visibility` `initialTimeSignatureVisibility`.
//! `stop_translation_timestep` (`:124–141`) warns of one heard in mid-measure
//! without `\partial`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, Moment, PROCESS, Rational, STOP, TranslatorClass, Tx, Work, event_property,
};
use lily_engine_scheme::{Exception, Tag, Value};

use crate::clef_engraver::is_true;
use crate::grob;

type Run<T> = Result<T, Exception>;

/// The index of each event class in [`TIME_SIGNATURE_ENGRAVER`]'s `hears`.
const POLYMETRIC: usize = 0;

pub static TIME_SIGNATURE_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Time_signature_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: true,
    hears: &[
        "polymetric-time-signature-event",
        "reference-time-signature-event",
    ],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(TimeSignatureEngraver::default())),
    reads: &[
        "initialTimeSignatureVisibility",
        "partialBusy",
        "timeSignature",
    ],
    writes: &[],
};

/// `time_signature_` by its handle, `last_spec_`, `event_` and
/// `local_event_`.
#[derive(Clone)]
struct TimeSignatureEngraver {
    time_signature: Option<Value>,
    last_spec: Value,
    event: Option<Value>,
    local_event: Option<Value>,
}

impl Default for TimeSignatureEngraver {
    fn default() -> Self {
        TimeSignatureEngraver {
            time_signature: None,
            last_spec: Value::NULL,
            event: None,
            local_event: None,
        }
    }
}

impl Work for TimeSignatureEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, i: usize, ev: Value) -> Run<()> {
        if i == POLYMETRIC {
            // Ignoring events that do not correspond to the value of
            // timeSignature in this context (`\context Voice \polymetric
            // \time …` in a Staff's engraver).
            let event_val = event_property(tx.heap(), ev, "time-signature");
            let context_val = tx.get("timeSignature");
            if tx.heap().equal(context_val, event_val) {
                self.local_event = Some(ev);
            }
        } else {
            self.event = Some(ev);
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.time_signature.is_some() {
            return Ok(());
        }

        let spec = tx.get("timeSignature");
        if self.last_spec != spec && (spec.tag() == Tag::Pair || spec == Value::FALSE) {
            let cause = self
                .local_event
                .or(self.event)
                .map_or(Cause::None, Cause::Event);
            let id = tx.make_defined_grob("TimeSignature", cause)?;
            let item = tx.grob_handle(id);
            self.time_signature = Some(item);

            // check value before setting to respect overrides
            if grob::property(tx.host, item, "time-signature")? == Value::NULL {
                grob::set_property(tx.heap(), item, "time-signature", spec);
            }

            if self.last_spec == Value::NULL {
                let visibility = tx.get("initialTimeSignatureVisibility");
                grob::set_property(tx.heap(), item, "break-visibility", visibility);
            }

            self.last_spec = spec;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(item) = self.time_signature
            && (self.event.is_some() || self.local_event.is_some())
        {
            // Not measure_position (context ()), which is normalized to be
            // >= 0 always.
            let v = tx.get("measurePosition");
            let mp = tx.heap().foreign::<Moment>(v).copied();
            if mp.is_some_and(|mp| mp.main.compare(Rational::ZERO).is_gt())
                && !is_true(tx.get("partialBusy"))
            {
                grob::warning(
                    tx.host,
                    item,
                    "mid-measure time signature without \\partial",
                )?;
            }
        }

        self.time_signature = None;
        self.event = None;
        self.local_event = None;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.time_signature);
        out.push(self.last_spec);
        out.extend(self.event);
        out.extend(self.local_event);
    }
}
