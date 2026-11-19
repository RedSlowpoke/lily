//! `stem-engraver.cc` (MS3 step 16): `Stem_engraver`, a class of layout's
//! ([`STEM_ENGRAVER`]) that the core passes at its `\layout` call and that
//! a group finds before the music crate's placeholder of the name (D67,
//! D108).
//!
//! - Each rhythmic head acknowledged (`acknowledge_rhythmic_head`,
//!   `:118–165`) that has no stem and whose cause is an event with a
//!   duration: the timestep's `Stem` and `StemStub` made from the first,
//!   their cause the head (`make_stem`, `:62–115`), with a `StemTremolo`
//!   of the tremolo heard; LilyPond's two warnings for a head whose
//!   duration's stem differs; the head added to the stem
//!   (`Stem::add_head`); a `Flag` of a normal stem of an eighth or less.
//! - `stop_translation_timestep` (`:182–208`): the flags of beamed stems
//!   killed at a bar line; `stemLeftBeamCount` and `stemRightBeamCount`,
//!   which `\set` sets, read into the stem's `beaming` and unset
//!   (`:195`, `:201`). `finalize` (`:176–180`) kills the flags of beamed
//!   stems. As the flags are kept to the end, the engraver stays awake
//!   from its first flag on.

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Duration, FINALIZE, Kind, STOP, StreamEvent, TranslatorClass, Tx, Work,
    assign_event_once, event_property, grob_of,
};
use lily_engine_scheme::{Exception, Heap, Value};

use crate::geometry::{Direction, X};
use crate::grob;
use crate::grob_array;
use crate::rhythmic_head;
use crate::stem;
use crate::stem::to_int;

type Run<T> = Result<T, Exception>;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

pub static STEM_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Stem_engraver",
    kind: Kind::Engraver,
    methods: STOP | FINALIZE,
    always: false,
    hears: &["tremolo-event"],
    acknowledges: &[("rhythmic-head-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(StemEngraver::default())),
    reads: &["currentBarLine", "stemLeftBeamCount", "stemRightBeamCount"],
    writes: &[],
};

/// `stem_`, by its record and handle, `tremolo_`, `maybe_flags_` and
/// `tremolo_ev_`.
#[derive(Clone, Default)]
struct StemEngraver {
    stem: Option<(u32, Value)>,
    tremolo: Option<Value>,
    maybe_flags: Vec<Value>,
    tremolo_ev: Option<Value>,
}

/// `from_scm (v, fallback)` of an `int`.
pub(crate) fn int_or(heap: &Heap, v: Value, fallback: i32) -> i32 {
    if !heap.is_exact_integer(v) {
        return fallback;
    }
    heap.to_i64(v)
        .and_then(|n| i32::try_from(n).ok())
        .unwrap_or(fallback)
}

/// `intlog2`; LilyPond exits for a `d` below 1, where this throws its
/// message.
pub(crate) fn intlog2(heap: &mut Heap, d: i32) -> Run<i32> {
    if d <= 0 {
        let key = heap.intern("misc-error");
        let text = heap.string(format!("intlog2 with negative argument: {d}"));
        let args = heap.list(&[Value::FALSE, text, Value::NULL, Value::FALSE]);
        return Err(Exception { key, args });
    }
    Ok(31 - d.leading_zeros() as i32)
}

/// `unsmob<Duration> (get_property (ev, "duration"))`.
fn duration(heap: &mut Heap, ev: Value) -> Option<Duration> {
    let d = event_property(heap, ev, "duration");
    heap.foreign::<Duration>(d).copied()
}

impl StemEngraver {
    /// `make_stem`: the stem and its stub, caused by the head `head`, and
    /// the tremolo heard.
    fn make_stem(&mut self, tx: &mut Tx<'_>, head: u32, cause: Value) -> Run<Value> {
        let id = tx.make_defined_grob("Stem", Cause::Grob(head))?;
        let stem = tx.grob_handle(id);
        self.stem = Some((id, stem));
        tx.make_defined_grob("StemStub", Cause::Grob(head))?;
        let Some(tremolo_ev) = self.tremolo_ev else {
            return Ok(stem);
        };
        let v = event_property(tx.heap(), tremolo_ev, "tremolo-type");
        let requested_type = int_or(tx.heap(), v, 8);
        // The duration log from the event: a note head's is at most 2.
        let log = duration(tx.heap(), cause).map_or(0, Duration::log);
        let mut tremolo_flags = intlog2(tx.heap(), requested_type)? - 2 - (log - 2).max(0);
        if tremolo_flags <= 0 {
            tx.event_warning(tremolo_ev, "tremolo duration is too long")?;
            tremolo_flags = 0;
        }
        if tremolo_flags != 0 {
            let id = tx.make_defined_grob("StemTremolo", Cause::Event(tremolo_ev))?;
            let tremolo = tx.grob_handle(id);
            self.tremolo = Some(tremolo);
            let heap = tx.heap();
            let count = heap.integer(i64::from(tremolo_flags));
            grob::set_property(heap, tremolo, "flag-count", count);
            grob::set_parent(heap, tremolo, stem, X);
            let sym = heap.intern("tremolo-flag");
            grob::set_object(heap, stem, sym, tremolo);
            let sym = heap.intern("stem");
            grob::set_object(heap, tremolo, sym, stem);
        }
        Ok(stem)
    }

    /// `kill_unused_flags`: the flags whose stem has a beam.
    fn kill_unused_flags(&self, tx: &mut Tx<'_>) -> Run<()> {
        for &flag in &self.maybe_flags {
            let Some(stem) = grob::get_parent(tx.heap(), flag, X) else {
                continue;
            };
            let sym = tx.heap().intern("beam");
            let beam = grob_array::object(tx.host, stem, sym)?;
            if grob_of(tx.heap(), beam).is_some() {
                grob::suicide(tx.heap(), flag);
            }
        }
        Ok(())
    }

    /// `stemLeftBeamCount` or `stemRightBeamCount` into the stem's
    /// `beaming` on `d`, and unset, if it is a number.
    fn beam_count(tx: &mut Tx<'_>, stem: Value, name: &str, d: Direction) -> Run<()> {
        let prop = tx.get(name);
        if tx.heap().is_number(prop) {
            let count = to_int(tx.heap(), prop)?;
            stem::set_beaming(tx.host, stem, count, d)?;
            tx.unset(name);
        }
        Ok(())
    }
}

impl Work for StemEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.maybe_record(self.stem.map(|s| s.0));
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        assign_event_once(tx, &mut self.tremolo_ev, ev)?;
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let head = tx.grob_handle(grob);
        if rhythmic_head::get_stem(tx.host, head)?.is_some() {
            return Ok(());
        }
        let cause = grob::property(tx.host, head, "cause")?;
        if tx.heap().foreign::<StreamEvent>(cause).is_none() {
            return Ok(());
        }
        let Some(d) = duration(tx.heap(), cause) else {
            return Ok(());
        };
        let stem = match self.stem {
            Some((_, stem)) => stem,
            None => self.make_stem(tx, grob, cause)?,
        };

        let mut ds = stem::duration_log(tx.host, stem)?;
        let mut dc = d.log();
        // Half and quarter notes have compatible stems; whole notes and
        // breves none.
        for log in [&mut ds, &mut dc] {
            match *log {
                1 => *log = 2,
                -1 => *log = 0,
                _ => {}
            }
        }
        if ds != dc {
            // `1 << n` as LilyPond's compile on arm64 shifts (D59).
            let num = if ds < 0 {
                1i32 << (ds.wrapping_neg() & 31)
            } else {
                1
            };
            let den = if ds > 0 { 1i32 << (ds & 31) } else { 1 };
            let text = format!("adding note head to incompatible stem (type = {num}/{den})");
            tx.event_warning(cause, &text)?;
            tx.event_warning(cause, "maybe input should specify polyphonic voices")?;
        }

        stem::add_head(tx.host, stem, head)?;

        if stem::is_normal_stem(tx.host, stem)? && stem::duration_log(tx.host, stem)? > 2 {
            let sym = tx.heap().intern("flag");
            let flag = grob_array::object(tx.host, stem, sym)?;
            if grob_of(tx.heap(), flag).is_none() {
                let (stem_id, _) = self.stem.expect("made above");
                let id = tx.make_defined_grob("Flag", Cause::Grob(stem_id))?;
                let flag = tx.grob_handle(id);
                let heap = tx.heap();
                grob::set_parent(heap, flag, stem, X);
                grob::set_object(heap, stem, sym, flag);
                self.maybe_flags.push(flag);
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let bar = tx.get("currentBarLine");
        if grob_of(tx.heap(), bar).is_some() {
            self.kill_unused_flags(tx)?;
        }
        self.tremolo = None;
        if let Some((_, stem)) = self.stem.take() {
            // FIXME: junk these properties.
            Self::beam_count(tx, stem, "stemLeftBeamCount", LEFT)?;
            Self::beam_count(tx, stem, "stemRightBeamCount", RIGHT)?;
        }
        self.tremolo_ev = None;
        if !self.maybe_flags.is_empty() {
            tx.stay_awake();
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.kill_unused_flags(tx)
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.stem.map(|(_, stem)| stem));
        out.extend(self.tremolo);
        out.extend_from_slice(&self.maybe_flags);
        out.extend(self.tremolo_ev);
    }
}
