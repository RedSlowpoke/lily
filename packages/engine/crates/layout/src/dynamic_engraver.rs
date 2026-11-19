//! `dynamic-engraver.cc` (MS3 step 19): `Dynamic_engraver`, a class of
//! layout's ([`DYNAMIC_ENGRAVER`]) that the core passes at its `\layout`
//! call and that a group finds before the music crate's placeholder of the
//! name (D67, D108).
//!
//! - An absolute dynamic heard (`:66–70`, `assign_event_once`), and a
//!   timestep's first crescendo or decrescendo start and stop by their
//!   `span-direction` (`span_dynamic_listener_`, a
//!   `Unique_span_event_listener`, as `staff_symbol_engraver.rs` hears its
//!   own); a `\breakDynamicSpan` (`:72–82`) breaks the spanner started at
//!   its timestep, or else the running one.
//! - `process_music` (`:95–178`): the running spanner ended by a stop, an
//!   absolute dynamic or a start, in that order; a start makes a
//!   `DynamicTextSpanner` where its `span-type`, or else the context's
//!   `crescendoSpanner` or `decrescendoSpanner`, is `text`, with its
//!   `span-text` or the context's text when that is a markup
//!   (`Text_interface::is_markup`, `:127`), and a `Hairpin` otherwise,
//!   with LilyPond's warning for a type that is not `hairpin`; an ended
//!   and a started hairpin are each other's `adjacent-spanners`. A
//!   `DynamicText` of the absolute dynamic, its `text` the event's, the
//!   ended spanner's right bound and the started one's left.
//! - Each note column acknowledged (`:229–250`), or its rest where it has
//!   no heads, is the X parent of the `DynamicText` that has none, and the
//!   started spanner's left bound and the ended one's right where they
//!   have none; `stop_translation_timestep` (`:180–195`) gives them
//!   `currentMusicalColumn` where they still have none, and forgets the
//!   timestep.
//! - `finalize` (`:197–211`): a running spanner, if live, is unterminated,
//!   LilyPond's warning at it, and killed.
//!
//! LilyPond's end of a spanner names the ender as its cause, which
//! `make_grob_info` (`engraver.cc:92–105`) sets only on a grob with none:
//! the spanner's is its start, so the end's cause is not kept. Without a
//! musical column, where LilyPond's would set a null bound, none is set
//! (`ledger_line_engraver::set_bound`).

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, assign_event_once,
    event_property, grob_of,
};
use lily_engine_scheme::{Exception, Value};

use crate::geometry::X;
use crate::grob;
use crate::grob_array;
use crate::ledger_line_engraver::set_bound;
use crate::spanner;
use crate::text;
use crate::tie_engraver::event_cause;

type Run<T> = Result<T, Exception>;

pub static DYNAMIC_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Dynamic_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP | FINALIZE,
    always: false,
    hears: &[
        "absolute-dynamic-event",
        "break-dynamic-span-event",
        "span-dynamic-event",
    ],
    acknowledges: &[("note-column-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(DynamicEngraver::default())),
    reads: &[
        "crescendoSpanner",
        "crescendoText",
        "currentMusicalColumn",
        "decrescendoSpanner",
        "decrescendoText",
    ],
    writes: &[],
};

/// `span_dynamic_listener_`'s start and stop events, `current_spanner_`
/// and `finished_spanner_` by their handles and record numbers, `script_`
/// and `script_event_` by their handles, and `end_new_spanner_`.
#[derive(Clone, Default)]
struct DynamicEngraver {
    start: Option<Value>,
    stop: Option<Value>,
    current: Option<(Value, u32)>,
    finished: Option<(Value, u32)>,
    script: Option<Value>,
    script_event: Option<Value>,
    end_new: bool,
}

/// `unsmob<Grob> (get_property (this, "currentMusicalColumn"))`.
pub(crate) fn musical_column(tx: &mut Tx<'_>) -> Option<Value> {
    let col = tx.get("currentMusicalColumn");
    grob_of(tx.heap(), col).map(|_| col)
}

/// `get_property_setting`: the event's `evprop`, or the context's
/// `ctxprop` where the event has none.
fn property_setting(tx: &mut Tx<'_>, ev: Value, evprop: &str, ctxprop: &str) -> Value {
    let v = event_property(tx.heap(), ev, evprop);
    if v == Value::NULL { tx.get(ctxprop) } else { v }
}

/// `get_spanner_type`: `crescendo` or `decrescendo` by the event's class,
/// LilyPond's programming error and an empty type for any other.
fn spanner_type(tx: &mut Tx<'_>, ev: Option<Value>) -> Run<&'static str> {
    let class = ev.map(|ev| event_property(tx.heap(), ev, "class"));
    let start = class.and_then(|c| tx.heap().car(c));
    let ty = match start.and_then(|s| tx.heap().name(s)) {
        Some("decrescendo-event") => "decrescendo",
        Some("crescendo-event") => "crescendo",
        _ => "",
    };
    if ty.is_empty() {
        tx.programming_error("unknown dynamic spanner type")?;
    }
    Ok(ty)
}

impl DynamicEngraver {
    /// The spanner a start event makes (`:114–148`).
    fn make_spanner(&mut self, tx: &mut Tx<'_>, starter: Value) -> Run<(Value, u32)> {
        let start_type = spanner_type(tx, Some(starter))?;
        let cresc_type =
            property_setting(tx, starter, "span-type", &format!("{start_type}Spanner"));

        if tx.heap().name(cresc_type) == Some("text") {
            let id = tx.make_defined_grob("DynamicTextSpanner", Cause::Event(starter))?;
            let span = tx.grob_handle(id);

            let mark = property_setting(tx, starter, "span-text", &format!("{start_type}Text"));
            if text::is_markup(tx.host, mark)? {
                grob::set_property(tx.heap(), span, "text", mark);
            }
            /*
              If the line of a text spanner is hidden, end the alignment spanner
              early: this allows dynamics to be spaced individually instead of
              being linked together.
            */
            let style = grob::property(tx.host, span, "style")?;
            if tx.heap().name(style) == Some("none") {
                grob::set_property(tx.heap(), span, "spanner-broken", Value::TRUE);
            }
            return Ok((span, id));
        }

        if tx.heap().name(cresc_type) != Some("hairpin") {
            let mut as_string = String::new();
            tx.heap().write(cresc_type, &mut as_string);
            tx.event_warning(
                starter,
                &format!("unknown crescendo style: {as_string}\ndefaulting to hairpin."),
            )?;
        }
        let id = tx.make_defined_grob("Hairpin", Cause::Event(starter))?;
        Ok((tx.grob_handle(id), id))
    }
}

impl Work for DynamicEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[self.end_new]);
        h.maybe_record(self.current.map(|c| c.1));
        h.maybe_record(self.finished.map(|c| c.1));
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        match which {
            0 => assign_event_once(tx, &mut self.script_event, ev).map(drop),
            1 => {
                // Case 1: Already have a start dynamic event -> break applies to new
                //         spanner (created later) -> set a flag
                // Case 2: no new spanner, but spanner already active -> break it now
                if self.start.is_some() {
                    self.end_new = true;
                } else if let Some((span, _)) = self.current {
                    grob::set_property(tx.heap(), span, "spanner-broken", Value::TRUE);
                }
                Ok(())
            }
            _ => {
                let d = event_property(tx.heap(), ev, "span-direction");
                match tx.heap().to_i64(d) {
                    Some(-1) => assign_event_once(tx, &mut self.start, ev).map(drop),
                    Some(1) => assign_event_once(tx, &mut self.stop, ev).map(drop),
                    _ => tx.programming_error("event span-direction is not set"),
                }
            }
        }
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((_, id)) = self.current
            && self.stop.or(self.script_event).or(self.start).is_some()
        {
            self.finished = self.current.take();
            tx.announce_end_grob(id);
        }

        if let Some(starter) = self.start {
            let current = self.make_spanner(tx, starter)?;
            self.current = Some(current);
            // if we have a break-dynamic-span event right after the start dynamic, break the new spanner immediately
            if self.end_new {
                grob::set_property(tx.heap(), current.0, "spanner-broken", Value::TRUE);
                self.end_new = false;
            }
            if let Some((finished, _)) = self.finished {
                let sym = tx.heap().intern("adjacent-spanners");
                if grob::has_interface(tx.heap(), finished, "hairpin-interface") {
                    grob_array::add_grob(tx.host, finished, sym, current.0)?;
                }
                if grob::has_interface(tx.heap(), current.0, "hairpin-interface") {
                    grob_array::add_grob(tx.host, current.0, sym, finished)?;
                }
            }
        }

        if let Some(ev) = self.script_event {
            let id = tx.make_defined_grob("DynamicText", Cause::Event(ev))?;
            let script = tx.grob_handle(id);
            self.script = Some(script);
            let mark = event_property(tx.heap(), ev, "text");
            grob::set_property(tx.heap(), script, "text", mark);

            if let Some((finished, _)) = self.finished {
                spanner::set_bound(tx.host, finished, 1, script)?;
            }
            if let Some((current, _)) = self.current {
                spanner::set_bound(tx.host, current, -1, script)?;
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((finished, _)) = self.finished
            && spanner::get_bound(tx.host, finished, 1)?.is_none()
        {
            let col = musical_column(tx);
            set_bound(tx, finished, 1, col)?;
        }

        if let Some((current, _)) = self.current
            && spanner::get_bound(tx.host, current, -1)?.is_none()
        {
            let col = musical_column(tx);
            set_bound(tx, current, -1, col)?;
        }
        self.script = None;
        self.script_event = None;
        self.start = None;
        self.stop = None;
        self.finished = None;
        self.end_new = false;
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((current, _)) = self.current
            && !grob::is_live_grob(tx.heap(), current)
        {
            self.current = None;
        }

        if let Some((current, _)) = self.current.take() {
            let event = event_cause(tx, current)?;
            let ty = spanner_type(tx, event)?;
            grob::warning(tx.host, current, &format!("unterminated {ty}"))?;
            grob::suicide(tx.heap(), current);
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let column = tx.grob_handle(grob);
        if let Some(script) = self.script
            && grob::get_parent(tx.heap(), script, X).is_none()
        {
            let heads = grob_array::extract_grob_array(tx.host, column, "note-heads")?;
            /*
              Spacing constraints may require dynamics to be attached to rests,
              so check for a rest if this note column has no note heads.
            */
            let x_parent = if heads.is_empty() {
                let sym = tx.heap().intern("rest");
                let rest = grob_array::object(tx.host, column, sym)?;
                grob_of(tx.heap(), rest).map(|_| rest)
            } else {
                Some(column)
            };
            if let Some(p) = x_parent {
                grob::set_parent(tx.heap(), script, p, X);
            }
        }

        if let Some((current, _)) = self.current
            && spanner::get_bound(tx.host, current, -1)?.is_none()
        {
            spanner::set_bound(tx.host, current, -1, column)?;
        }
        if let Some((finished, _)) = self.finished
            && spanner::get_bound(tx.host, finished, 1)?.is_none()
        {
            spanner::set_bound(tx.host, finished, 1, column)?;
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.start);
        out.extend(self.stop);
        out.extend(self.current.map(|(v, _)| v));
        out.extend(self.finished.map(|(v, _)| v));
        out.extend(self.script);
        out.extend(self.script_event);
    }
}
