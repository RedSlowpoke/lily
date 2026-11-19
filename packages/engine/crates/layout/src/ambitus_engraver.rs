//! `ambitus-engraver.cc` (MS8 step 8, part 2, 269 lines):
//! `Ambitus_engraver`, a class of layout's ([`AMBITUS_ENGRAVER`]) that
//! shadows the music crate's placeholder of its name (D67, D108).
//! `ambitus::print` is Scheme, LilyPond's image's.
//!
//! - `process_music` (`:94–101`): at the first timestep, an `AmbitusLine`,
//!   an `Ambitus` group and each side's `AmbitusNoteHead` and
//!   `AmbitusAccidental`, the accidental on the head's Y and its
//!   `accidental-grob`, the line on the lower head's X (`:66–92`).
//! - `stop_translation_timestep` (`:103–137`), once: the middle C position
//!   then (the clef's under `ottavaStartNow`, else `middleCPosition` unless
//!   a cue's is an integer, else the clef's and its offset) and
//!   `keyAlterations`.
//! - Each note head of a `note-event` with a pitch and no `ignore-ambitus`
//!   (`:139–163`) widens the pitch interval by tone, its event the cause of
//!   the side it widens.
//! - `finalize` (`:165–237`): with pitches, an `AccidentalPlacement`; each
//!   head caused by its event at its staff position, its accidental killed
//!   where the key gives its alteration and the other side is not the same
//!   step differently altered, else given the alteration; each head a
//!   conditional item of the placement, its accidental the placement's, the
//!   head one of the line's `note-heads`. Without pitches, all killed.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Held, Kind, PROCESS, Pitch, Rational, STOP, Scale, TranslatorClass, Tx, Work,
    event_property,
};
use lily_engine_scheme::{Exception, Heap, Value};

use crate::accidental_engraver::rational_value;
use crate::geometry::{X, Y};
use crate::note_heads_engraver::steps;
use crate::paper_column_engraver::rational;
use crate::tie_engraver::{event_cause, in_event_class};
use crate::{accidental_placement, axis_group, grob, grob_array, separation_item};

type Run<T> = Result<T, Exception>;

pub static AMBITUS_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Ambitus_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    always: true,
    hears: &[],
    acknowledges: &[("note-head-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(AmbitusEngraver::default())),
    reads: &[
        "keyAlterations",
        "middleCClefPosition",
        "middleCCuePosition",
        "middleCOffset",
        "middleCPosition",
        "ottavaStartNow",
        "staffLineLayoutFunction",
    ],
    writes: &[],
};

/// `ambitus_`, `group_`, `heads_` and `accidentals_` (down first) by their
/// handles and record numbers, `causes_`, `pitch_interval_` (none while
/// empty, each side's pitch and its value), `is_typeset_`, `start_c0_` and
/// `start_key_sig_`.
#[derive(Clone, Default)]
struct AmbitusEngraver {
    ambitus: Option<(Value, u32)>,
    group: Option<(Value, u32)>,
    heads: [Option<(Value, u32)>; 2],
    accidentals: [Option<(Value, u32)>; 2],
    causes: [Option<Value>; 2],
    pitch_interval: Option<[(Pitch, Value); 2]>,
    is_typeset: bool,
    start_c0: i32,
    start_key_sig: Value,
}

/// `Pitch::tone_pitch`.
fn tones(heap: &Heap, p: &Pitch) -> Rational {
    heap.foreign::<Scale>(p.scale)
        .and_then(|s| p.tones(s))
        .unwrap_or(Rational::ZERO)
}

/// `from_scm (v, fallback)` of an int.
fn int_or(tx: &mut Tx<'_>, v: Value, fallback: i32) -> Run<i32> {
    if tx.heap().is_number(v) {
        crate::stem::to_int(tx.heap(), v)
    } else {
        Ok(fallback)
    }
}

/// `ly_assoc`: the first entry of `alist` whose car is `equal?` to `key`.
fn assoc(heap: &Heap, key: Value, mut alist: Value) -> Option<Value> {
    while let (Some(entry), Some(rest)) = (heap.car(alist), heap.cdr(alist)) {
        if heap.car(entry).is_some_and(|k| heap.equal(k, key)) {
            return Some(entry);
        }
        alist = rest;
    }
    None
}

impl AmbitusEngraver {
    fn create_ambitus(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let id = tx.make_defined_grob("AmbitusLine", Cause::None)?;
        let ambitus = (tx.grob_handle(id), id);
        self.ambitus = Some(ambitus);
        let id = tx.make_defined_grob("Ambitus", Cause::None)?;
        let group = tx.grob_handle(id);
        self.group = Some((group, id));
        for i in 0..2 {
            let id = tx.make_defined_grob("AmbitusNoteHead", Cause::None)?;
            let head = tx.grob_handle(id);
            self.heads[i] = Some((head, id));
            let id = tx.make_defined_grob("AmbitusAccidental", Cause::None)?;
            let acc = tx.grob_handle(id);
            self.accidentals[i] = Some((acc, id));
            grob::set_parent(tx.heap(), acc, head, Y);
            let sym = tx.heap().intern("accidental-grob");
            grob::set_object(tx.heap(), head, sym, acc);
            axis_group::add_element(tx.host, group, head)?;
            axis_group::add_element(tx.host, group, acc)?;
        }
        if let Some((down, _)) = self.heads[0] {
            grob::set_parent(tx.heap(), ambitus.0, down, X);
        }
        axis_group::add_element(tx.host, group, ambitus.0)?;
        self.is_typeset = false;
        Ok(())
    }
}

impl Work for AmbitusEngraver {
    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        // Ensure that ambitus is created in the very first timestep
        if self.ambitus.is_none() {
            self.create_ambitus(tx)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.ambitus.is_some() && !self.is_typeset {
            let c_pos = tx.get("middleCPosition");
            let cue_pos = tx.get("middleCCuePosition");
            // \ottava reads middleCClefPosition and overrides middleCOffset
            // and middleCPosition ignoring previously set values. Therefore
            //  1. \ottava is incompatible with non-default offset and
            //     position values (is this a bug? TODO)
            //  2. we don't need to read these values and revert the changes
            //     \ottava made but we can just read the clef position.
            // `scm_is_integer`: an integer, exact or not.
            let is_integer = |heap: &Heap, v: Value| {
                v.as_fixnum().is_some()
                    || (heap.is_real(v)
                        && heap
                            .to_f64(v)
                            .is_some_and(|x| x.is_finite() && x == libm::trunc(x)))
            };
            self.start_c0 = if tx.get("ottavaStartNow") == Value::TRUE {
                let v = tx.get("middleCClefPosition");
                int_or(tx, v, 0)?
            } else if is_integer(tx.heap(), c_pos) && !is_integer(tx.heap(), cue_pos) {
                crate::stem::to_int(tx.heap(), c_pos)?
            } else {
                let v = tx.get("middleCClefPosition");
                let clef_pos = int_or(tx, v, 0)?;
                let v = tx.get("middleCOffset");
                let offset = int_or(tx, v, 0)?;
                clef_pos.wrapping_add(offset)
            };
            self.start_key_sig = tx.get("keyAlterations");
            self.is_typeset = true;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        let Some(nr) = event_cause(tx, g)? else {
            return Ok(());
        };
        if !in_event_class(tx.heap(), nr, "note-event")
            || grob::property(tx.host, g, "ignore-ambitus")? == Value::TRUE
        {
            return Ok(());
        }
        let p = event_property(tx.heap(), nr, "pitch");
        // If the engraver is added to a percussion context, filter out
        // unpitched note heads.
        let Some(pitch) = tx.heap().foreign::<Pitch>(p).copied() else {
            return Ok(());
        };
        let heap = tx.heap();
        let t = tones(heap, &pitch);
        match &mut self.pitch_interval {
            None => {
                self.pitch_interval = Some([(pitch, p), (pitch, p)]);
                self.causes = [Some(nr), Some(nr)];
            }
            Some(iv) => {
                if tones(heap, &iv[0].0).compare(t).is_gt() {
                    iv[0] = (pitch, p);
                    self.causes[0] = Some(nr);
                }
                if tones(heap, &iv[1].0).compare(t).is_lt() {
                    iv[1] = (pitch, p);
                    self.causes[1] = Some(nr);
                }
            }
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some((ambitus, _)) = self.ambitus else {
            return Ok(());
        };
        let Some(iv) = self.pitch_interval else {
            for i in 0..2 {
                if let Some((acc, _)) = self.accidentals[i] {
                    grob::suicide(tx.heap(), acc);
                }
                if let Some((head, _)) = self.heads[i] {
                    grob::suicide(tx.heap(), head);
                }
            }
            grob::suicide(tx.heap(), ambitus);
            return Ok(());
        };
        let (Some((_, acc_down)), Some(group)) = (self.accidentals[0], self.group) else {
            return Ok(());
        };
        let id = tx.make_defined_grob("AccidentalPlacement", Cause::Grob(acc_down))?;
        let placement = tx.grob_handle(id);
        let layout_proc = tx.get("staffLineLayoutFunction");
        for d in 0..2 {
            let (Some((head, _)), Some((acc, _))) = (self.heads[d], self.accidentals[d]) else {
                continue;
            };
            let (p, pv) = iv[d];
            let pos = if tx.heap().is_procedure(layout_proc) {
                let v = tx.host.call(layout_proc, &[pv])?;
                crate::stem::to_int(tx.heap(), v)?
            } else {
                steps(tx.heap(), &p)
            };
            let cause = self.causes[d].unwrap_or(Value::NULL);
            grob::set_property(tx.heap(), head, "cause", cause);
            let pos = tx
                .heap()
                .integer(i64::from(self.start_c0.wrapping_add(pos)));
            grob::set_property(tx.heap(), head, "staff-position", pos);

            let heap = tx.heap();
            let octave = heap.integer(i64::from(p.octave));
            let notename = heap.integer(i64::from(p.notename));
            let key = heap.cons(octave, notename);
            let handle = assoc(heap, key, self.start_key_sig)
                .or_else(|| assoc(heap, notename, self.start_key_sig));
            let sig_alter = match handle.and_then(|h| heap.cdr(h)) {
                Some(alter) => rational(heap, alter, Rational::ZERO),
                None => Rational::ZERO,
            };
            let other = iv[1 - d].0;
            if sig_alter == p.alteration
                && !(steps(heap, &p) == steps(heap, &other) && p.alteration != other.alteration)
            {
                grob::suicide(heap, acc);
                let sym = heap.intern("accidental-grob");
                grob::set_object(heap, head, sym, Value::NULL);
            } else {
                let alteration = rational_value(heap, p.alteration);
                grob::set_property(heap, acc, "alteration", alteration);
            }
            separation_item::add_conditional_item(tx.host, head, placement)?;
            accidental_placement::add_accidental(tx.host, placement, acc, false, Value::FALSE)?;
            let sym = tx.heap().intern("note-heads");
            grob_array::add_grob(tx.host, ambitus, sym, head)?;
        }
        axis_group::add_element(tx.host, group.0, placement)?;
        Ok(())
    }

    fn held(&self, h: &mut dyn Held) {
        for g in [self.ambitus, self.group]
            .into_iter()
            .chain(self.heads)
            .chain(self.accidentals)
        {
            h.maybe_record(g.map(|g| g.1));
        }
        h.flags(&[self.is_typeset, self.pitch_interval.is_some()]);
        h.num(i64::from(self.start_c0));
    }

    fn values(&self, out: &mut Vec<Value>) {
        for g in [self.ambitus, self.group]
            .into_iter()
            .chain(self.heads)
            .chain(self.accidentals)
        {
            out.extend(g.map(|g| g.0));
        }
        out.extend(self.causes.iter().flatten());
        if let Some(iv) = self.pitch_interval {
            out.extend(iv.iter().map(|p| p.1));
        }
        out.push(self.start_key_sig);
    }
}
