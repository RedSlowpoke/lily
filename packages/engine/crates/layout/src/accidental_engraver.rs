//! `accidental-engraver.cc` (MS3 step 17): `Accidental_engraver`, a class
//! of layout's ([`ACCIDENTAL_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! `process_music` (`:494–500`) gives each context from its own down, and
//! those above it that hold one, a copy of `keyAlterations` as
//! `localAlterations` when the key changed (`update_local_key_signature`,
//! `:107–128`, `:112` and `:125`). Each accidental-participating head
//! acknowledged (`:447–468`) with an event cause, unless it is a harmonic
//! (without `harmonicAccidentals`) or its engraver's context sets
//! `nullAccidentals`, is an entry. `process_acknowledged` (`:197–254`)
//! runs `autoAccidentals`' and `autoCautionaries`' rules from the image
//! on each new entry's pitch (`check_pitch_against_rules`, `:152–195`,
//! `:171`). Where one is needed, the entry's `Accidental` (or
//! `AccidentalCautionary`, or `AccidentalSuggestion` where
//! `suggestAccidentals` asks) is made through the head's engraver, its
//! origin the head's, as that engraver's (`create_accidental`, `:256–279`;
//! `make_standard_accidental`, `:281–326`, `:296`), and joined to the
//! timestep's `AccidentalPlacement`, made here in the Staff
//! (`Accidental_placement::add_accidental`); the arpeggios and fingerings
//! on X take it as a support (`Side_position_interface::add_support`).
//! `stop_translation_timestep` (`:349–445`) gives a tie that ends on an
//! entry's head to its accidental (`:352–379`, `Tie::head`), puts each
//! entry's alteration, or `tied` for a tied one not `forced`, with its bar
//! number and end into `localAlterations` from its context up, wherever
//! one is defined above (`:405–429`), and makes the placement a
//! conditional item of the note columns acknowledged.
//!
//! `right_objects_`, which nothing fills in LilyPond 2.26.0, is left out.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, CtxId, Dir, Duration, FINALIZE, Kind, Moment, Origin, PROCESS, PROCESS_ACKNOWLEDGED,
    Pitch, Rational, STOP, StreamEvent, TranslatorClass, Tx, Work, event_property, grob_of,
    ly_deep_copy, translator_handle,
};
use lily_engine_scheme::{Exception, Heap, Tag, Value};

use crate::accidental_placement;
use crate::clef_engraver::is_true;
use crate::geometry::{X, Y};
use crate::grob;
use crate::grob_array;
use crate::separation_item;
use crate::side_position;
use crate::tie;
use crate::tie_engraver::{event_cause, in_event_class};
use crate::wrong_type;

type Run<T> = Result<T, Exception>;

pub static ACCIDENTAL_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Accidental_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | PROCESS_ACKNOWLEDGED | STOP | FINALIZE,
    always: true,
    hears: &[],
    acknowledges: &[
        ("accidental-participating-head-interface", false),
        ("arpeggio-interface", false),
        ("chord-bracket-interface", false),
        ("chord-slur-interface", false),
        ("finger-interface", false),
        ("tie-interface", true),
        ("note-column-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(AccidentalEngraver::default())),
    reads: &[
        "autoAccidentals",
        "autoCautionaries",
        "internalBarNumber",
        "extraNatural",
        "harmonicAccidentals",
        "accidentalGrouping",
        "keyAlterations",
        "localAlterations",
    ],
    writes: &["localAlterations"],
};

/// `Accidental_entry`: the event that caused the head, its accidental,
/// the engraver that made the head (whose context is `origin_`), the head
/// by its record and handle, and whether a tie ends on it.
#[derive(Clone)]
struct Entry {
    done: bool,
    melodic: Value,
    accidental: Option<Value>,
    origin: Origin,
    head: (u32, Value),
    tied: bool,
}

/// `last_keysig_`, `left_objects_`, `accidental_placement_`,
/// `accidentals_`, `ties_` and `note_columns_`, the grobs by their
/// handles.
#[derive(Clone)]
struct AccidentalEngraver {
    last_keysig: Value,
    left_objects: Vec<Value>,
    accidental_placement: Option<Value>,
    accidentals: Vec<Entry>,
    ties: Vec<Value>,
    note_columns: Vec<Value>,
}

impl Default for AccidentalEngraver {
    fn default() -> AccidentalEngraver {
        AccidentalEngraver {
            last_keysig: Value::NULL,
            left_objects: Vec::new(),
            accidental_placement: None,
            accidentals: Vec::new(),
            ties: Vec::new(),
            note_columns: Vec::new(),
        }
    }
}

/// `Accidental_result`: whether to restore a natural, and whether an
/// accidental is needed.
#[derive(Clone, Copy, Default)]
struct Need {
    restore: bool,
    acc: bool,
}

impl Need {
    fn score(self) -> u8 {
        u8::from(self.acc) + u8::from(self.restore)
    }
}

/// `measure_number` (`context.cc`): `internalBarNumber`, one less before
/// the measure's start.
pub(crate) fn measure_number(tx: &mut Tx<'_>, c: CtxId) -> i64 {
    let heap = tx.host.heap();
    let (barnum, smp) = (
        heap.intern("internalBarNumber"),
        heap.intern("measurePosition"),
    );
    let a = tx.t.arena.borrow();
    let (barnum, smp) = (a.get(c, barnum), a.get(c, smp));
    let bn = barnum.as_fixnum().unwrap_or(0);
    let mp = heap.foreign::<Moment>(smp).copied().unwrap_or_default();
    if mp.main.is_negative() { bn - 1 } else { bn }
}

/// `note_end_mom` (`context.cc`): when a note of `dur` heard now ends.
fn note_end_mom(now: Moment, dur: Option<Duration>) -> Moment {
    let len = dur.and_then(Duration::length).unwrap_or(Rational::ZERO);
    if now.grace.is_negative() {
        Moment::new(now.main, now.grace.checked_add(len).unwrap_or(now.grace))
    } else {
        Moment::new(
            now.main.checked_add(len).unwrap_or(now.main),
            Rational::ZERO,
        )
    }
}

/// `to_scm (Rational)` of a finite alteration.
pub(crate) fn rational_value(heap: &mut Heap, r: Rational) -> Value {
    let n = heap.integer(r.numerator());
    let d = heap.integer(r.denominator());
    heap.div(n, d).unwrap_or(Value::FALSE)
}

/// `check_pitch_against_rules`: each procedure of `rules` called with the
/// context reached so far, the pitch and the bar number; a symbol moves to
/// the context of that name above, where there is one.
fn check_pitch_against_rules(
    tx: &mut Tx<'_>,
    pitch: Pitch,
    origin: CtxId,
    mut rules: Value,
    bar_number: i64,
) -> Run<Need> {
    let mut result = Need::default();
    let heap = tx.host.heap();
    let pitch_scm = heap.wrap(pitch);
    let barnum_scm = heap.integer(bar_number);

    if let Some(first) = heap.car(rules)
        && first.tag() != Tag::Symbol
    {
        let mut name = String::new();
        heap.display(first, &mut name);
        tx.warning(&alloc::format!(
            "accidental typesetting list must begin with context-name: {name}"
        ))?;
    }

    let mut origin = origin;
    while let (Some(rule), Some(rest)) = (tx.heap().car(rules), tx.heap().cdr(rules)) {
        if tx.heap().is_procedure(rule) {
            let handle = tx.t.arena.borrow().handle(origin);
            let r = tx.host.call(rule, &[handle, pitch_scm, barnum_scm])?;
            let heap = tx.heap();
            let (Some(restore), Some(acc)) = (heap.car(r), heap.cdr(r)) else {
                return Err(wrong_type(heap, Some("car"), 1, r, "pair"));
            };
            result.acc |= is_true(acc);
            result.restore |= is_true(restore);
        }
        // If symbol then it is a context name. Scan parent contexts to
        // find it.
        else if rule.tag() == Tag::Symbol {
            let heap = tx.host.heap();
            let a = tx.t.arena.borrow();
            if let Some(dad) = a.find_context(heap, origin, Dir::Up, rule, "") {
                origin = dad;
            }
        } else {
            let mut text = String::new();
            tx.heap().write(rule, &mut text);
            tx.warning(&alloc::format!(
                "procedure or context-name expected for accidental rule, found {text}"
            ))?;
        }
        rules = rest;
    }
    Ok(result)
}

impl AccidentalEngraver {
    /// `update_local_key_signature`.
    fn update_local_key_signature(&mut self, tx: &mut Tx<'_>, new_sig: Value) {
        self.last_keysig = new_sig;
        let sym = tx.heap().intern("localAlterations");
        let mut walk = Vec::from([tx.context]);
        while let Some(c) = walk.pop() {
            let copy = ly_deep_copy(tx.host.heap(), new_sig);
            let mut a = tx.t.arena.borrow_mut();
            a.set(c, sym, copy);
            walk.extend(a.children(c).iter().rev());
        }

        // Reset parent contexts so that e.g. piano-accidentals won't
        // remember old cross-staff accidentals after key-sig-changes.
        let mut trans = tx.t.arena.borrow().parent(tx.context);
        while let Some(c) = trans {
            if tx.t.arena.borrow().here(c, sym).is_none() {
                break;
            }
            let copy = ly_deep_copy(tx.host.heap(), self.last_keysig);
            let mut a = tx.t.arena.borrow_mut();
            a.set(c, sym, copy);
            trans = a.parent(c);
        }
    }

    /// `create_accidental`: the entry's accidental, suggested where its
    /// context's `suggestAccidentals` is true, or `cautionary` for a
    /// cautionary one; its `restore-first` set for `restore_natural` where
    /// `extraNatural` is true.
    fn create_accidental(
        &mut self,
        tx: &mut Tx<'_>,
        i: usize,
        restore_natural: bool,
        cautionary: bool,
    ) -> Run<()> {
        let (origin, support) = (self.accidentals[i].origin, self.accidentals[i].head);
        let sym = tx.heap().intern("suggestAccidentals");
        let suggest = tx.t.arena.borrow().get(origin.0, sym);
        let a = if is_true(suggest) || (cautionary && suggest == tx.heap().intern("cautionary")) {
            Self::make_suggested_accidental(tx, support, origin)?
        } else {
            self.make_standard_accidental(tx, support, origin, cautionary)?
        };

        if restore_natural && is_true(tx.get("extraNatural")) {
            grob::set_property(tx.heap(), a, "restore-first", Value::TRUE);
        }

        self.accidentals[i].accidental = Some(a);
        Ok(())
    }

    /// `make_standard_accidental`: an `Accidental`, or an
    /// `AccidentalCautionary`, made through `trans`, the engraver of the
    /// head `note_head`, so that it gets the property settings of its Voice;
    /// supported by the left objects on X, a child of the head on Y, in the
    /// timestep's placement and the head's `accidental-grob`.
    fn make_standard_accidental(
        &mut self,
        tx: &mut Tx<'_>,
        (head_id, note_head): (u32, Value),
        trans: Origin,
        cautionary: bool,
    ) -> Run<Value> {
        // We construct the accidentals at the originating Voice level, so
        // that we get the property settings for Accidental from the
        // respective Voice.
        let name = if cautionary {
            "AccidentalCautionary"
        } else {
            "Accidental"
        };
        let id = tx.make_defined_grob_from(trans, name, Cause::Grob(head_id))?;
        let a = tx.grob_handle(id);

        // We add the accidentals to the support of the arpeggio, so it is
        // put left of the accidentals.
        for &left in &self.left_objects {
            let axis = grob::property(tx.host, left, "side-axis")?;
            if tx.heap().equal(axis, Value::fixnum(0).unwrap()) {
                side_position::add_support(tx.host, left, a)?;
            }
        }

        grob::set_parent(tx.heap(), a, note_head, Y);

        let placement = match self.accidental_placement {
            Some(p) => p,
            None => {
                let id = tx.make_defined_grob("AccidentalPlacement", Cause::Grob(id))?;
                let p = tx.grob_handle(id);
                self.accidental_placement = Some(p);
                p
            }
        };

        let grouping = tx.get("accidentalGrouping");
        let stagger = grouping == tx.heap().intern("voice");
        let t = tx.t;
        let hash_key =
            translator_handle(tx.host.heap(), t, trans.0, trans.1).unwrap_or(Value::FALSE);
        accidental_placement::add_accidental(tx.host, placement, a, stagger, hash_key)?;

        let sym = tx.heap().intern("accidental-grob");
        grob::set_object(tx.heap(), note_head, sym, a);
        Ok(a)
    }

    /// `make_suggested_accidental`: an `AccidentalSuggestion` made through
    /// `trans`, supported by the head and its stem, a child of the head on
    /// X.
    fn make_suggested_accidental(
        tx: &mut Tx<'_>,
        (head_id, note_head): (u32, Value),
        trans: Origin,
    ) -> Run<Value> {
        let id = tx.make_defined_grob_from(trans, "AccidentalSuggestion", Cause::Grob(head_id))?;
        let a = tx.grob_handle(id);

        side_position::add_support(tx.host, a, note_head)?;
        let sym = tx.heap().intern("stem");
        let stem = grob_array::object(tx.host, a, sym)?;
        if grob_of(tx.heap(), stem).is_some() {
            side_position::add_support(tx.host, a, stem)?;
        }

        grob::set_parent(tx.heap(), a, note_head, X);
        Ok(a)
    }

    /// `acknowledge_accidental_participating_head`.
    fn acknowledge_accidental_participating_head(
        &mut self,
        tx: &mut Tx<'_>,
        grob: u32,
        head: Value,
    ) -> Run<()> {
        let note = grob::property(tx.host, head, "cause")?;
        if tx.heap().foreign::<StreamEvent>(note).is_none() {
            return Ok(());
        }
        // option to skip accidentals on string harmonics
        if !is_true(tx.get("harmonicAccidentals")) {
            let style = grob::property(tx.host, head, "style")?;
            if style == tx.heap().intern("harmonic") {
                return Ok(());
            }
        }
        // ignore accidentals in non-printing voices like NullVoice
        let origin = tx.t.arena.borrow().records.grobs[grob as usize].origin;
        let sym = tx.heap().intern("nullAccidentals");
        if is_true(tx.t.arena.borrow().get(origin.0, sym)) {
            return Ok(());
        }
        self.accidentals.push(Entry {
            done: false,
            melodic: note,
            accidental: None,
            origin,
            head: (grob, head),
            tied: false,
        });
        Ok(())
    }
}

impl Work for AccidentalEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.accidentals.len() as i64);
        for e in &self.accidentals {
            h.flags(&[e.done, e.tied, e.accidental.is_some()]);
            h.context(e.origin.0);
            h.num(e.origin.1.into());
            h.record(e.head.0);
        }
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let sig = tx.get("keyAlterations");
        if self.last_keysig != sig {
            self.update_local_key_signature(tx, sig);
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        match which {
            0 => self.acknowledge_accidental_participating_head(tx, grob, g),
            // `acknowledge_arpeggio`, of arpeggios, chord brackets and
            // chord slurs, and `acknowledge_finger`.
            1..=4 => {
                self.left_objects.push(g);
                Ok(())
            }
            // `acknowledge_end_tie`.
            5 => {
                self.ties.push(g);
                Ok(())
            }
            _ => {
                self.note_columns.push(g);
                Ok(())
            }
        }
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.accidentals.last().is_none_or(|e| e.done) {
            return Ok(());
        }
        let accidental_rules = tx.get("autoAccidentals");
        let cautionary_rules = tx.get("autoCautionaries");
        let c = tx.context;
        let barnum = measure_number(tx, c);

        for i in 0..self.accidentals.len() {
            let entry = &mut self.accidentals[i];
            if entry.done {
                continue;
            }
            entry.done = true;
            let (note, origin) = (entry.melodic, entry.origin.0);

            let heap = tx.heap();
            let pitch = event_property(heap, note, "pitch");
            let Some(pitch) = heap.foreign::<Pitch>(pitch).copied() else {
                continue;
            };

            let mut acc = check_pitch_against_rules(tx, pitch, origin, accidental_rules, barnum)?;
            let caut = check_pitch_against_rules(tx, pitch, origin, cautionary_rules, barnum)?;

            let heap = tx.heap();
            let mut cautionary = is_true(event_property(heap, note, "cautionary"));
            if caut.score() > acc.score() {
                acc.acc |= caut.acc;
                acc.restore |= caut.restore;
                cautionary = true;
            }

            let forced = is_true(event_property(heap, note, "force-accidental"));
            if !acc.acc && forced {
                acc.acc = true;
            }

            // Cannot look for ties: it's not guaranteed that they reach us
            // before the notes.
            if !in_event_class(tx.heap(), note, "trill-span-event") {
                if acc.acc {
                    self.create_accidental(tx, i, acc.restore, cautionary)?;
                }
                // LilyPond's would crash for a cautionary without an
                // accidental; nothing is set here.
                if (forced || cautionary)
                    && let Some(a) = self.accidentals[i].accidental
                {
                    grob::set_property(tx.heap(), a, "forced", Value::TRUE);
                }
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for j in (0..self.ties.len()).rev() {
            let r = tie::head(tx.host, self.ties[j], 1)?;
            let l = tie::head(tx.host, self.ties[j], -1)?;
            if let (Some(l), Some(r)) = (l, r) {
                // Don't mark accidentals as "tied" when the pitch is not
                // actually the same. This is relevant for enharmonic ties.
                let le = event_cause(tx, l)?;
                let re = event_cause(tx, r)?;
                if let (Some(le), Some(re)) = (le, re) {
                    let heap = tx.heap();
                    let (lp, rp) = (
                        event_property(heap, le, "pitch"),
                        event_property(heap, re, "pitch"),
                    );
                    if !heap.equal(lp, rp) {
                        continue;
                    }
                }
            }

            let Some(r) = r else {
                continue;
            };
            if let Some(i) = self.accidentals.iter().rposition(|e| e.head.1 == r) {
                if let Some(g) = self.accidentals[i].accidental {
                    let sym = tx.heap().intern("tie");
                    grob::set_object(tx.heap(), g, sym, self.ties[j]);
                    self.accidentals[i].tied = true;
                }
                self.ties.remove(j);
            }
        }

        let sym = tx.heap().intern("localAlterations");
        for entry in self.accidentals.iter().rev() {
            let note = entry.melodic;
            let mut origin = Some(entry.origin.0);

            let barnum = measure_number(tx, entry.origin.0);

            let heap = tx.heap();
            let pitch = event_property(heap, note, "pitch");
            let Some(pitch) = heap.foreign::<Pitch>(pitch).copied() else {
                continue;
            };

            let n = heap.integer(i64::from(pitch.notename));
            let o = heap.integer(i64::from(pitch.octave));
            let a = rational_value(heap, pitch.alteration);
            let key = heap.cons(o, n);

            let dur = event_property(heap, note, "duration");
            let dur = heap.foreign::<Duration>(dur).copied();
            let end_mom = note_end_mom(tx.now(), dur);
            let heap = tx.heap();
            let barnum = heap.integer(barnum);
            let end_mom = heap.wrap(end_mom);
            let position = heap.cons(barnum, end_mom);

            while let Some(c) = origin {
                if tx.t.arena.borrow().where_defined(c, sym).is_none() {
                    break;
                }
                let tied = match entry.accidental {
                    Some(g) if entry.tied => !is_true(grob::property(tx.host, g, "forced")?),
                    _ => false,
                };
                let heap = tx.host.heap();
                let val = if tied {
                    // Remember an alteration that is different both from
                    // that of the tied note and of the key signature.
                    let tied = heap.intern("tied");
                    heap.cons(tied, position)
                } else {
                    // not really correct if there is more than one note
                    // head with the same notename.
                    heap.cons(a, position)
                };
                let mut arena = tx.t.arena.borrow_mut();
                arena.prepend_property(heap, c, sym, key, val);
                origin = arena.parent(c);
            }
        }

        if let Some(placement) = self.accidental_placement.take() {
            for &column in &self.note_columns {
                separation_item::add_conditional_item(tx.host, column, placement)?;
            }
        }

        self.accidentals.clear();
        self.note_columns.clear();
        self.left_objects.clear();
        Ok(())
    }

    fn finalize(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.last_keysig = Value::NULL;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.push(self.last_keysig);
        out.extend(&self.left_objects);
        out.extend(self.accidental_placement);
        for e in &self.accidentals {
            out.extend([e.melodic, e.head.1]);
            out.extend(e.accidental);
        }
        out.extend(&self.ties);
        out.extend(&self.note_columns);
    }
}
