//! `slur-engraver.cc` (MS4 step 7): `Slur_engraver`, a class of layout's
//! ([`SLUR_ENGRAVER`]) that the core passes at its `\layout` call and that
//! a group finds before the music crate's class of the name, which keeps
//! `slurMelismaBusy` without grobs (D98, D108); this keeps it as
//! LilyPond's does (`set_melisma`). `Phrasing_slur_engraver`
//! (`phrasing_slur_engraver.rs`) is the same engraver over phrasing slurs
//! ([`Slurs`]), as LilyPond's derives from it.
//!
//! - `listen_note_slur`, `listen_note` (`:96–118`): a slur's start or
//!   stop, on its own or among a note's articulations (a slur on a note
//!   of a chord), with its note.
//! - `process_music` (`:302–335`): the slurs ended by each stop's
//!   `spanner-id` (`try_to_end`, `:283–300`; "cannot end slur" where none
//!   is), then one made for each start (`create_slur`, `:181–212`, two
//!   for `doubleSlurs`) unless `can_create_slur` (`:214–281`) finds one of
//!   its id ("already have slur", or the start's direction taken over a
//!   new slur's without one).
//! - `acknowledge_note_column` (`:120–149`): each column added to the open
//!   and ended slurs; a slur started or ended on a note of a chord bound
//!   to that note's head. Scripts but dynamics, accidentals, fingerings,
//!   text scripts, dots, ties at their end and tuplet numbers kept for
//!   `Slur::auxiliary_acknowledge_extra_object` (`slur.rs`).
//! - `stop_translation_timestep` (`:337–364`): the command column
//!   encompassed by the ended slurs, and by the open ones unless a slur
//!   starts; the ended slurs' right bound the musical column where they
//!   have none, their ends announced; the extra objects acknowledged.
//! - `finalize` (`:166–175`): a slur left open killed ("unterminated
//!   slur").
//!
//! The engraver runs at every timestep, as `process_music` sets
//! `slurMelismaBusy` and `stop_translation_timestep` adds the command
//! column to the open slurs at each. In a context whose heads a
//! placeholder makes (`Tab_note_heads_engraver` where the translation has
//! no tablature's, since MS4 step 7a; `Drum_notes_engraver` until step 13)
//! it is the
//! music crate's class of its name, as `Tie_engraver` is
//! (`tie_engraver.rs`): a slur there would have no columns to be bound
//! by, which LilyPond's never lacks (`Tab_tie_follow_engraver` reads its
//! left bound's heads).

use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;

use lily_engine_music::{
    CLASSES, Cause, FINALIZE, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::direction::set_grob_direction;
use crate::dynamic_engraver::musical_column;
use crate::geometry::Direction;
use crate::grob;
use crate::grob_array;
use crate::ledger_line_engraver::command_column;
use crate::slur;
use crate::spanner;
use crate::tie_engraver::{event_cause, in_event_class, placeholder_heads};

type Run<T> = Result<T, Exception>;

const START: Direction = -1;
const STOP_: Direction = 1;
const DOWN: Direction = -1;
const UP: Direction = 1;

/// What `Slur_engraver`'s virtual methods give, for slurs and for phrasing
/// slurs: `event_symbol`, `grob_symbol`, `object_name`, whether
/// `double_property` reads `doubleSlurs`, and whether `set_melisma` sets
/// `slurMelismaBusy`; with the interfaces of the class's acknowledgers, in
/// their order.
pub(crate) struct Slurs {
    pub name: &'static str,
    pub event: &'static str,
    pub grob: &'static str,
    pub object: &'static str,
    pub phrasing: bool,
    pub acknowledges: &'static [(&'static str, bool)],
}

/// `Slur_engraver::boot`'s acknowledgers.
const SLUR_ACKNOWLEDGES: &[(&str, bool)] = &[
    ("inline-accidental-interface", false),
    ("fingering-interface", false),
    ("note-column-interface", false),
    ("script-interface", false),
    ("text-script-interface", false),
    ("dots-interface", false),
    ("tie-interface", true),
    ("tuplet-number-interface", false),
];

const SLURS: Slurs = Slurs {
    name: "Slur_engraver",
    event: "slur-event",
    grob: "Slur",
    object: "slur",
    phrasing: false,
    acknowledges: SLUR_ACKNOWLEDGES,
};

pub static SLUR_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Slur_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    always: true,
    hears: &["slur-event", "note-event"],
    acknowledges: SLUR_ACKNOWLEDGES,
    must_be_last: false,
    make: Some(|| Box::new(SlurEngraver::new(&SLURS))),
    reads: &["slurMelismaBusy", "doubleSlurs"],
    writes: &[],
};

/// `Event_info`: a slur event, and the note whose articulation it is.
#[derive(Clone, Copy)]
struct EventInfo {
    slur: Value,
    note: Option<Value>,
}

/// `Slur_engraver`, and `Phrasing_slur_engraver` by its [`Slurs`]: the
/// timestep's starts and stops, `note_slurs_` (each a note and a slur
/// started or ended on it, by `START` and `STOP`, in the order inserted),
/// the open slurs and those ended now, by record and handle, and the
/// objects to acknowledge.
#[derive(Clone)]
pub(crate) struct SlurEngraver {
    what: &'static Slurs,
    /// Where a placeholder makes the heads ([`placeholder_heads`]), the
    /// music crate's class of its name, if that has work.
    stand_in: Option<Option<Box<dyn Work>>>,
    start_events: Vec<EventInfo>,
    stop_events: Vec<EventInfo>,
    note_slurs: [Vec<(Value, Value)>; 2],
    slurs: Vec<(u32, Value)>,
    end_slurs: Vec<(u32, Value)>,
    objects_to_acknowledge: Vec<Value>,
}

/// `from_scm<Direction>`: an exact -1, 0 or 1, else `CENTER`.
fn to_direction(v: Value) -> Direction {
    match v.as_fixnum() {
        Some(d @ -1..=1) => d as Direction,
        _ => 0,
    }
}

impl SlurEngraver {
    pub(crate) fn new(what: &'static Slurs) -> SlurEngraver {
        SlurEngraver {
            what,
            stand_in: None,
            start_events: Vec::new(),
            stop_events: Vec::new(),
            note_slurs: [Vec::new(), Vec::new()],
            slurs: Vec::new(),
            end_slurs: Vec::new(),
            objects_to_acknowledge: Vec::new(),
        }
    }

    /// `set_melisma`: `slurMelismaBusy`, for slurs only.
    fn set_melisma(&self, tx: &mut Tx<'_>, m: bool) {
        if !self.what.phrasing {
            tx.set("slurMelismaBusy", Value::bool(m));
        }
    }

    /// `double_property`: `doubleSlurs`, for slurs only.
    fn double_property(&self, tx: &mut Tx<'_>) -> bool {
        !self.what.phrasing && tx.get("doubleSlurs") == Value::TRUE
    }

    /// `listen_note_slur`.
    fn listen_note_slur(&mut self, tx: &mut Tx<'_>, ev: Value, note: Option<Value>) -> Run<()> {
        let d = to_direction(event_property(tx.heap(), ev, "span-direction"));
        let info = EventInfo { slur: ev, note };
        if d == START {
            self.start_events.push(info);
        } else if d == STOP_ {
            self.stop_events.push(info);
        } else {
            let name = event_property(tx.heap(), ev, "name");
            let name = alloc::string::String::from(tx.heap().name(name).unwrap_or(""));
            tx.event_warning(ev, &format!("direction of {name} invalid: {d}"))?;
        }
        Ok(())
    }

    /// `acknowledge_note_column`.
    fn acknowledge_note_column(&mut self, tx: &mut Tx<'_>, e: Value) -> Run<()> {
        for &(_, s) in self.slurs.iter().rev() {
            slur::add_column(tx.host, s, e)?;
        }
        for &(_, s) in self.end_slurs.iter().rev() {
            slur::add_column(tx.host, s, e)?;
        }
        // Now cater for slurs starting/ending at a notehead: those override
        // the column bounds
        if self.note_slurs[0].is_empty() && self.note_slurs[1].is_empty() {
            return Ok(());
        }
        let heads = grob_array::extract_grob_array(tx.host, e, "note-heads")?;
        for &head in heads.iter().rev() {
            if let Some(ev) = event_cause(tx, head)? {
                for (i, d) in [START, STOP_].into_iter().enumerate() {
                    let found: Vec<Value> = self.note_slurs[i]
                        .iter()
                        .filter(|&&(note, _)| note == ev)
                        .map(|&(_, s)| s)
                        .collect();
                    for s in found {
                        spanner::set_bound(tx.host, s, d, head)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// `create_slur`: a slur of `evi`, of `spanner_id`, in `dir` if any;
    /// with `doubleSlurs` two, down and up.
    fn create_slur(
        &mut self,
        tx: &mut Tx<'_>,
        spanner_id: Value,
        evi: EventInfo,
        dir: Direction,
    ) -> Run<()> {
        let cause = Cause::Event(evi.slur);
        let id = tx.make_defined_grob(self.what.grob, cause)?;
        let s = tx.grob_handle(id);
        grob::set_property(tx.heap(), s, "spanner-id", spanner_id);
        if dir != 0 {
            set_grob_direction(tx.host, s, dir);
        }
        self.slurs.push((id, s));
        if let Some(note) = evi.note {
            self.note_slurs[0].push((note, s));
        }

        if self.double_property(tx) {
            set_grob_direction(tx.host, s, DOWN);
            let id = tx.make_defined_grob(self.what.grob, cause)?;
            let s = tx.grob_handle(id);
            grob::set_property(tx.heap(), s, "spanner-id", spanner_id);
            set_grob_direction(tx.host, s, UP);
            self.slurs.push((id, s));
            if let Some(note) = evi.note {
                self.note_slurs[0].push((note, s));
            }
        }
        Ok(())
    }

    /// `can_create_slur`: whether the start `ev` of `id` makes a slur:
    /// not if a slur of its id was open before the timestep ("already
    /// have slur", the start forgotten), nor if one made now has its
    /// direction or it has none; one made now without a direction is
    /// killed for it.
    fn can_create_slur(
        &mut self,
        tx: &mut Tx<'_>,
        id: Value,
        old_slurs: usize,
        event_idx: usize,
        ev: Value,
    ) -> Run<bool> {
        for j in (0..self.slurs.len()).rev() {
            let (_, s) = self.slurs[j];
            let updown = to_direction(event_property(tx.heap(), ev, "direction"));

            // Check if we already have a slur with the same spanner-id.
            let sid = grob::property(tx.host, s, "spanner-id")?;
            if tx.heap().equal(id, sid) {
                if j < old_slurs {
                    // We already have an old slur, so give a warning
                    // and completely ignore the new slur.
                    tx.event_warning(ev, &format!("already have {}", self.what.object))?;
                    self.start_events.remove(event_idx);
                    return Ok(false);
                }

                // If this slur event has no direction, it will not
                // contribute anything new to the existing slur(s), so
                // we can ignore it.

                if updown == 0 {
                    return Ok(false);
                }

                let Some(c) = event_cause(tx, s)? else {
                    let text = format!("{} without a cause", self.what.object);
                    grob::programming_error(tx.host, s, &text)?;
                    return Ok(true);
                };

                let slur_dir = to_direction(event_property(tx.heap(), c, "direction"));

                // If the existing slur does not have a direction yet,
                // we'd rather take the new one.

                if slur_dir == 0 {
                    grob::suicide(tx.heap(), s);
                    self.slurs.remove(j);
                    return Ok(true);
                }

                // If the existing slur has the same direction as ours, drop ours

                if slur_dir == updown {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// `try_to_end`: the open slurs of the stop's `spanner-id` ended;
    /// whether there were any.
    fn try_to_end(&mut self, tx: &mut Tx<'_>, evi: EventInfo) -> Run<bool> {
        let id = event_property(tx.heap(), evi.slur, "spanner-id");

        // Find the slurs that are ended with this event (by checking the spanner-id)
        let mut ended = false;
        for j in (0..self.slurs.len()).rev() {
            let (_, s) = self.slurs[j];
            let sid = grob::property(tx.host, s, "spanner-id")?;
            if tx.heap().equal(id, sid) {
                ended = true;
                self.end_slurs.push(self.slurs[j]);
                if let Some(note) = evi.note {
                    self.note_slurs[1].push((note, s));
                }
                self.slurs.remove(j);
            }
        }
        Ok(ended)
    }
}

impl Work for SlurEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        if let Some(Some(w)) = &self.stand_in {
            w.held(h);
        }
        for list in [&self.start_events, &self.stop_events] {
            h.num(list.len() as i64);
            for e in list {
                h.flags(&[e.note.is_some()]);
            }
        }
        for list in [&self.slurs, &self.end_slurs] {
            h.num(list.len() as i64);
            for &(id, _) in list {
                h.record(id);
            }
        }
        h.num(self.note_slurs[0].len() as i64);
        h.num(self.note_slurs[1].len() as i64);
    }

    fn connect(&mut self, tx: &mut Tx<'_>) {
        if placeholder_heads(tx) {
            let class = CLASSES.iter().find(|c| c.name == self.what.name);
            let mut w = class.and_then(|c| c.make).map(|make| make());
            if let Some(w) = &mut w {
                w.connect(tx);
            }
            self.stand_in = Some(w);
        }
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        if let Some(stand_in) = &mut self.stand_in {
            return stand_in
                .as_mut()
                .map_or(Ok(()), |w| w.listen(tx, which, ev));
        }
        if which == 0 {
            return self.listen_note_slur(tx, ev, None);
        }
        // `listen_note`: a slur among a note's articulations.
        let mut arts = event_property(tx.heap(), ev, "articulations");
        while let (Some(art), Some(rest)) = (tx.heap().car(arts), tx.heap().cdr(arts)) {
            if in_event_class(tx.heap(), art, self.what.event) {
                self.listen_note_slur(tx, art, Some(ev))?;
            }
            arts = rest;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        if self.stand_in.is_some() {
            return Ok(());
        }
        let g = tx.grob_handle(grob);
        match self.what.acknowledges[which].0 {
            "note-column-interface" => self.acknowledge_note_column(tx, g),
            // `acknowledge_script`: but a dynamic.
            "script-interface" => {
                if !grob::has_interface(tx.heap(), g, "dynamic-interface") {
                    self.objects_to_acknowledge.push(g);
                }
                Ok(())
            }
            // `acknowledge_extra_object`.
            _ => {
                self.objects_to_acknowledge.push(g);
                Ok(())
            }
        }
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(stand_in) = &mut self.stand_in {
            return stand_in.as_mut().map_or(Ok(()), |w| w.process_music(tx));
        }
        let mut i = 0;
        while i < self.stop_events.len() {
            let ev = self.stop_events[i];
            let id = event_property(tx.heap(), ev.slur, "spanner-id");
            let ended = self.try_to_end(tx, ev)?;
            if ended {
                // Ignore redundant stop events for this id
                let mut j = self.stop_events.len();
                while j - 1 > i {
                    j -= 1;
                    let other = event_property(tx.heap(), self.stop_events[j].slur, "spanner-id");
                    if tx.heap().equal(id, other) {
                        self.stop_events.remove(j);
                    }
                }
            } else {
                tx.event_warning(ev.slur, &format!("cannot end {}", self.what.object))?;
            }
            i += 1;
        }

        let old_slurs = self.slurs.len();
        let mut i = self.start_events.len();
        while i > 0 {
            i -= 1;
            let ev = self.start_events[i].slur;
            let id = event_property(tx.heap(), ev, "spanner-id");
            let updown = to_direction(event_property(tx.heap(), ev, "direction"));

            if self.can_create_slur(tx, id, old_slurs, i, ev)? {
                let evi = self.start_events[i];
                self.create_slur(tx, id, evi, updown)?;
            }
        }

        self.set_melisma(tx, !self.slurs.is_empty());
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(stand_in) = &mut self.stand_in {
            return stand_in
                .as_mut()
                .map_or(Ok(()), |w| w.stop_translation_timestep(tx));
        }
        if let Some(g) = command_column(tx) {
            for &(_, s) in &self.end_slurs {
                slur::add_extra_encompass(tx.host, s, g)?;
            }

            if self.start_events.is_empty() {
                for &(_, s) in &self.slurs {
                    slur::add_extra_encompass(tx.host, s, g)?;
                }
            }
        }

        for &(id, s) in &self.end_slurs {
            if spanner::get_bound(tx.host, s, 1)?.is_none()
                && let Some(col) = musical_column(tx)
            {
                spanner::set_bound(tx.host, s, 1, col)?;
            }
            tx.announce_end_grob(id);
        }

        let slurs: Vec<Value> = self.slurs.iter().map(|&(_, s)| s).collect();
        let end_slurs: Vec<Value> = self.end_slurs.iter().map(|&(_, s)| s).collect();
        for &obj in &self.objects_to_acknowledge {
            slur::auxiliary_acknowledge_extra_object(tx.host, obj, &slurs, &end_slurs)?;
        }

        self.note_slurs[0].clear();
        self.note_slurs[1].clear();
        self.objects_to_acknowledge.clear();
        self.end_slurs.clear();
        self.start_events.clear();
        self.stop_events.clear();
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(stand_in) = &mut self.stand_in {
            return stand_in.as_mut().map_or(Ok(()), |w| w.finalize(tx));
        }
        for &(_, s) in &self.slurs {
            grob::warning(tx.host, s, &format!("unterminated {}", self.what.object))?;
            grob::suicide(tx.heap(), s);
        }
        self.slurs.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        if let Some(Some(w)) = &self.stand_in {
            w.values(out);
        }
        for e in self.start_events.iter().chain(&self.stop_events) {
            out.push(e.slur);
            out.extend(e.note);
        }
        for &(note, s) in self.note_slurs.iter().flatten() {
            out.extend([note, s]);
        }
        out.extend(self.slurs.iter().chain(&self.end_slurs).map(|&(_, s)| s));
        out.extend_from_slice(&self.objects_to_acknowledge);
    }
}
