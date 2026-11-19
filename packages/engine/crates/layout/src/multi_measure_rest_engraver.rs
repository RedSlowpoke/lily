//! `multi-measure-rest-engraver.cc` (MS3 step 42):
//! `Multi_measure_rest_engraver`, a class of layout's
//! ([`MULTI_MEASURE_REST_ENGRAVER`]) that the core passes at its `\layout`
//! call and that a group finds before the music crate's placeholder of
//! the name (D67, D108).
//!
//! - A `multi-measure-rest-event` heard (`:78–92`) is the running rest's,
//!   its stop moment now plus its length (in grace time its grace part);
//!   the texts and articulations heard are kept with it
//!   (`:94–105`). All three are forgotten once a timestep's main part
//!   reaches the stop moment (`clear_lapsed_events`, `:115–124`), at
//!   `start_translation_timestep` (`:264–268`) and at each rest heard.
//! - `process_music` (`:220–262`), at every timestep: at a measure's
//!   start (`measureStartNow`) or the first timestep, the command column
//!   is kept, and running grobs end: the rest's `measure-count` the bar
//!   numbers since its start (`internalBarNumber`), its number's `text`
//!   that count unless it has a text, or the number killed at no more
//!   than `restNumberThreshold` (1 if it is no integer); each takes the
//!   column as its right bound and the rest's end is announced. Then,
//!   while a rest event is kept and no grobs run, a `MultiMeasureRest`
//!   caused by it, its `MultiMeasureRestNumber` caused by the rest, a
//!   `MultiMeasureRestScript` of each articulation
//!   (`script_engraver::make_script_from_event`) and a
//!   `MultiMeasureRestText` of each text, each with the event's direction
//!   if it is one; those of a direction stacked, down then up, each on the
//!   one before (`Side_position_interface::add_support`), and all on the
//!   rest, their X and Y parent (`:126–190`). They take the kept column as
//!   their left bound.
//!
//! A `currentCommandColumn` that is no item is no column kept, and no
//! bound is set, as in LilyPond's.

use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, Moment, PROCESS, START, TranslatorClass, Tx, Work, event_property, grob_of,
};
use lily_engine_scheme::{Exception, Value};

use crate::clef_engraver::is_true;
use crate::geometry::{X, Y};
use crate::grob;
use crate::item;
use crate::script_engraver::make_script_from_event;
use crate::side_position;
use crate::spanner;
use crate::tie_engraver::event_length;

type Run<T> = Result<T, Exception>;

pub static MULTI_MEASURE_REST_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Multi_measure_rest_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS,
    // `process_music` ends the grobs at measures' starts no event wakes.
    always: true,
    hears: &[
        "multi-measure-rest-event",
        "multi-measure-text-event",
        "multi-measure-articulation-event",
    ],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(MultiMeasureRestEngraver::new())),
    reads: &[
        "internalBarNumber",
        "restNumberThreshold",
        "currentCommandColumn",
        "measureStartNow",
    ],
    writes: &[],
};

/// The listeners' indices in [`MULTI_MEASURE_REST_ENGRAVER`].
const REST: usize = 0;
const TEXT: usize = 1;

/// Its state, the grobs by their handles (the rest with its record
/// number).
#[derive(Clone)]
struct MultiMeasureRestEngraver {
    text_events: Vec<Value>,
    articulation_events: Vec<Value>,
    /// `text[0]` is a MultiMeasureRestNumber grob; the rest are optional
    /// MultiMeasureRestText and MultiMeasureRestScript grobs.
    text: Vec<Value>,
    rest_ev: Option<Value>,
    mmrest: Option<(Value, u32)>,
    stop_moment: Moment,
    start_measure: i64,
    /// Ugh, this is a kludge - need this for multi-measure-rest-grace.ly
    last_command_item: Option<Value>,
    first_time: bool,
    number_threshold: i64,
}

/// `from_scm<int>`: Guile's type error for what is no exact integer of an
/// `int`'s range.
fn to_int(tx: &mut Tx<'_>, v: Value) -> Run<i64> {
    let heap = tx.heap();
    match heap.to_i64(v) {
        Some(n) if heap.is_exact_integer(v) && i32::try_from(n).is_ok() => Ok(n),
        _ => Err(crate::wrong_type(heap, None, 0, v, "exact integer")),
    }
}

/// `is_scm<Direction>`: an exact integer from -1 to 1.
fn is_direction(v: Value) -> bool {
    matches!(v.as_fixnum(), Some(-1..=1))
}

impl MultiMeasureRestEngraver {
    fn new() -> MultiMeasureRestEngraver {
        MultiMeasureRestEngraver {
            text_events: Vec::new(),
            articulation_events: Vec::new(),
            text: Vec::new(),
            rest_ev: None,
            mmrest: None,
            stop_moment: Moment::default(),
            start_measure: 0,
            last_command_item: None,
            first_time: true,
            number_threshold: 0,
        }
    }

    fn add_bound_item_to_grobs(&self, tx: &mut Tx<'_>, item: Value) -> Run<()> {
        if let Some((mmrest, _)) = self.mmrest {
            spanner::add_bound_item(tx.host, mmrest, item)?;
        }
        for &t in &self.text {
            spanner::add_bound_item(tx.host, t, item)?;
        }
        Ok(())
    }

    fn clear_lapsed_events(&mut self, now: Moment) {
        if now.main.compare(self.stop_moment.main).is_ge() {
            self.rest_ev = None;
            self.text_events.clear();
            self.articulation_events.clear();
        }
    }

    fn initialize_grobs(&mut self, tx: &mut Tx<'_>, rest_ev: Value) -> Run<()> {
        let id = tx.make_defined_grob("MultiMeasureRest", Cause::Event(rest_ev))?;
        let mmrest = tx.grob_handle(id);
        self.mmrest = Some((mmrest, id));
        let number = tx.make_defined_grob("MultiMeasureRestNumber", Cause::Grob(id))?;
        let number = tx.grob_handle(number);
        self.text.push(number);

        for (i, &e) in self.articulation_events.iter().enumerate() {
            let sp = tx.make_defined_grob("MultiMeasureRestScript", Cause::Event(e))?;
            let sp = tx.grob_handle(sp);
            let art_type = event_property(tx.heap(), e, "articulation-type");
            make_script_from_event(tx, sp, art_type, i)?;
            let dir = event_property(tx.heap(), e, "direction");
            if is_direction(dir) {
                grob::set_property(tx.heap(), sp, "direction", dir);
            }

            self.text.push(sp);
        }

        for &e in &self.text_events {
            let sp = tx.make_defined_grob("MultiMeasureRestText", Cause::Event(e))?;
            let sp = tx.grob_handle(sp);
            let heap = tx.heap();
            let t = event_property(heap, e, "text");
            let dir = event_property(heap, e, "direction");
            grob::set_property(heap, sp, "text", t);
            if is_direction(dir) {
                grob::set_property(heap, sp, "direction", dir);
            }

            self.text.push(sp);
        }

        /*
          Stack different scripts.
        */
        for d in [-1, 1] {
            let dir = tx.heap().integer(d);
            let mut last = None;
            for &t in &self.text {
                let v = grob::property(tx.host, t, "direction")?;
                if tx.heap().equal(dir, v) {
                    if let Some(last) = last {
                        side_position::add_support(tx.host, t, last)?;
                    }
                    last = Some(t);
                }
            }
        }

        for &t in &self.text {
            side_position::add_support(tx.host, t, mmrest)?;
            grob::set_parent(tx.heap(), t, mmrest, Y);
            grob::set_parent(tx.heap(), t, mmrest, X);
        }
        Ok(())
    }

    fn set_measure_count(&mut self, tx: &mut Tx<'_>, mmrest: Value, n: i64) -> Run<()> {
        let n_scm = tx.heap().integer(n);
        grob::set_property(tx.heap(), mmrest, "measure-count", n_scm);

        let g = self.text[0]; // the MultiMeasureRestNumber
        if grob::property(tx.host, g, "text")? == Value::NULL {
            if n <= self.number_threshold {
                grob::suicide(tx.heap(), g);
            } else {
                let text = tx.heap().string(n.to_string());
                grob::set_property(tx.heap(), g, "text", text);
            }
        }
        Ok(())
    }
}

impl Work for MultiMeasureRestEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.maybe_record(self.mmrest.map(|m| m.1));
        h.moment(self.stop_moment);
        h.num(self.start_measure);
        h.flags(&[self.first_time]);
        h.num(self.number_threshold);
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        match which {
            REST => {
                /* FIXME: Should use assign_event_once. Can't do that yet because of
                the kill-mm-rests hack in part-combine-iterator. */
                self.rest_ev = Some(ev);
                let now = tx.now();
                let len = event_length(tx.heap(), ev, now);
                self.stop_moment = now.checked_add(len).unwrap_or(now);

                self.clear_lapsed_events(now);
            }
            TEXT => self.text_events.push(ev),
            _ => self.articulation_events.push(ev),
        }
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.clear_lapsed_events(tx.now());
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if is_true(tx.get("measureStartNow")) || self.first_time {
            let col = tx.get("currentCommandColumn");
            self.last_command_item =
                (grob_of(tx.heap(), col).is_some() && item::is_item(tx.heap(), col)).then_some(col);

            // Finalize the current grobs.
            if let Some((mmrest, id)) = self.mmrest {
                let v = tx.get("internalBarNumber");
                let curr_measure = to_int(tx, v)?;
                self.set_measure_count(tx, mmrest, curr_measure - self.start_measure)?;
                if let Some(item) = self.last_command_item {
                    self.add_bound_item_to_grobs(tx, item)?;
                }

                tx.announce_end_grob(id);
                self.text.clear();
                self.mmrest = None;
            }
        }

        // Create new grobs if a rest event is (still) active.
        if self.mmrest.is_none()
            && let Some(rest_ev) = self.rest_ev
        {
            self.initialize_grobs(tx, rest_ev)?;
            self.text_events.clear();
            self.articulation_events.clear();

            if let Some(item) = self.last_command_item.take() {
                self.add_bound_item_to_grobs(tx, item)?;
            }

            let v = tx.get("internalBarNumber");
            self.start_measure = to_int(tx, v)?;
            let v = tx.get("restNumberThreshold");
            let heap = tx.heap();
            self.number_threshold = match heap.to_i64(v) {
                Some(n) if heap.is_exact_integer(v) && i32::try_from(n).is_ok() => n,
                _ => 1,
            };
        }

        self.first_time = false;
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(&self.text_events);
        out.extend(&self.articulation_events);
        out.extend(&self.text);
        out.extend(self.rest_ev);
        out.extend(self.mmrest.map(|(m, _)| m));
        out.extend(self.last_command_item);
    }
}
