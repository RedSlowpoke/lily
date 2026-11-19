//! `metronome-engraver.cc` (part 4's, MS3 step 54, 197 lines):
//! `Metronome_mark_engraver`, a class of layout's
//! ([`METRONOME_MARK_ENGRAVER`]) that the core passes at its `\layout`
//! call and that a group finds before the music crate's placeholder of the
//! name (D67, D108).
//!
//! - A tempo change heard (`:58–62`), the first of a timestep's, makes at
//!   `process_music` (`:153–164`) a `MetronomeMark` caused by it, its
//!   `text` what `metronomeMarkFormatter` gives of the event and the
//!   context (`format-metronome-markup`, from the image).
//! - A break-aligned item acknowledged (`:70–88`): the `staff-bar` one is
//!   the mark's bar; the first of its `break-align-symbols` its support and
//!   X parent; with either, the mark is `non-musical`. The break alignment
//!   (`:90–95`) is its X parent where it has a support. Any grob with an
//!   interface of its `non-break-align-symbols` (`:97–106`) is its X
//!   parent.
//! - `stop_translation_timestep` (`:108–151`): a mark on a multi-measure
//!   rest goes to the bar where there is one; without a support, to
//!   `currentMusicalColumn`, else `currentCommandColumn`. Its
//!   `side-support-elements` the grob array of `stavesFound`.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, assign_event_once, grob_of,
};
use lily_engine_scheme::{Exception, Value};

use crate::geometry::X;
use crate::grob;
use crate::grob_array::grob_list_to_grob_array;
use crate::staff_spacing::is_list;

type Run<T> = Result<T, Exception>;

pub static METRONOME_MARK_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Metronome_mark_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["tempo-change-event"],
    acknowledges: &[
        ("break-aligned-interface", false),
        ("break-alignment-interface", false),
        ("grob-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(MetronomeMarkEngraver::default())),
    reads: &[
        "currentCommandColumn",
        "currentMusicalColumn",
        "metronomeMarkFormatter",
        "stavesFound",
        "tempoHideNote",
    ],
    writes: &[],
};

/// `text_`, `support_`, `bar_` and `tempo_ev_`, by their handles.
#[derive(Clone, Default)]
struct MetronomeMarkEngraver {
    text: Option<Value>,
    support: Option<Value>,
    bar: Option<Value>,
    tempo_ev: Option<Value>,
}

/// `safe_is_member`: `lst` is a list with a member `equal?` to `scm`.
fn safe_is_member(tx: &mut Tx<'_>, scm: Value, lst: Value) -> bool {
    let heap = tx.heap();
    if !is_list(heap, lst) {
        return false;
    }
    let mut s = lst;
    while let (Some(x), Some(rest)) = (heap.car(s), heap.cdr(s)) {
        if heap.equal(scm, x) {
            return true;
        }
        s = rest;
    }
    false
}

impl MetronomeMarkEngraver {
    /// `acknowledge_break_aligned`.
    fn acknowledge_break_aligned(&mut self, tx: &mut Tx<'_>, g: Value) -> Run<()> {
        let Some(text) = self.text else {
            return Ok(());
        };
        let symbol = grob::property(tx.host, g, "break-align-symbol")?;
        if symbol == tx.heap().intern("staff-bar") {
            self.bar = Some(g);
        } else if self.support.is_none() {
            let symbols = grob::property(tx.host, text, "break-align-symbols")?;
            if safe_is_member(tx, symbol, symbols) {
                self.support = Some(g);
                grob::set_parent(tx.heap(), text, g, X);
            }
        }
        if self.bar.is_some() || self.support.is_some() {
            grob::set_property(tx.heap(), text, "non-musical", Value::TRUE);
        }
        Ok(())
    }

    /// `acknowledge_grob`: each of the mark's `non-break-align-symbols`
    /// that is an interface of `g` makes `g` its X parent.
    fn acknowledge_grob(&mut self, tx: &mut Tx<'_>, g: Value) -> Run<()> {
        let Some(text) = self.text else {
            return Ok(());
        };
        let mut s = grob::property(tx.host, text, "non-break-align-symbols")?;
        while let (Some(iface), Some(rest)) = (tx.heap().car(s), tx.heap().cdr(s)) {
            let name = tx.heap().name(iface).map(String::from);
            if name.is_some_and(|name| grob::has_interface(tx.heap(), g, &name)) {
                grob::set_parent(tx.heap(), text, g, X);
            }
            s = rest;
        }
        Ok(())
    }
}

impl Work for MetronomeMarkEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        assign_event_once(tx, &mut self.tempo_ev, ev)?;
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        match which {
            0 => self.acknowledge_break_aligned(tx, g),
            // acknowledge_break_alignment
            1 => {
                if let (Some(text), Some(_)) = (self.text, self.support) {
                    grob::set_parent(tx.heap(), text, g, X);
                }
                Ok(())
            }
            _ => self.acknowledge_grob(tx, g),
        }
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(text) = self.text else {
            return Ok(());
        };
        let on_mmrest = grob::get_parent(tx.heap(), text, X)
            .is_some_and(|p| grob::has_interface(tx.heap(), p, "multi-measure-rest-interface"));
        if let (true, Some(bar)) = (on_mmrest, self.bar) {
            grob::set_parent(tx.heap(), text, bar, X);
        } else if self.support.is_none() {
            /*
              Gardner Read "Music Notation", p.278
              Align the metronome mark over the time signature (or the
              first notational element of the measure if no time
              signature is present in that measure).
            */
            let mc = tx.get("currentMusicalColumn");
            let cc = tx.get("currentCommandColumn");
            if grob_of(tx.heap(), mc).is_some() {
                grob::set_parent(tx.heap(), text, mc, X);
            } else if grob_of(tx.heap(), cc).is_some() {
                grob::set_parent(tx.heap(), text, cc, X);
            }
        }
        let staves = tx.get("stavesFound");
        let (array, warnings) = grob_list_to_grob_array(tx.heap(), staves);
        let sym = tx.heap().intern("side-support-elements");
        grob::set_object(tx.heap(), text, sym, array);
        for w in warnings {
            tx.warning(&w)?;
        }
        self.text = None;
        self.support = None;
        self.bar = None;
        self.tempo_ev = None;
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(ev) = self.tempo_ev else {
            return Ok(());
        };
        let id = tx.make_defined_grob("MetronomeMark", Cause::Event(ev))?;
        let text = tx.grob_handle(id);
        self.text = Some(text);
        let proc = tx.get("metronomeMarkFormatter");
        let context = tx.t.arena.borrow().handle(tx.context);
        let result = tx.host.call(proc, &[ev, context])?;
        let depth = tx.heap().root_depth();
        tx.heap().root(result);
        grob::set_property(tx.heap(), text, "text", result);
        tx.heap().unroot_to(depth);
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.text);
        out.extend(self.support);
        out.extend(self.bar);
        out.extend(self.tempo_ev);
    }
}
