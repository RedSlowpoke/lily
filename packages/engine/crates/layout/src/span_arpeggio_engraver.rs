//! `span-arpeggio-engraver.cc` (part 4's, MS3 step 54, 203 lines):
//! `Span_arpeggio_engraver`, a class of layout's
//! ([`SPAN_ARPEGGIO_ENGRAVER`]) that the core passes at its `\layout` call
//! and that a group finds before the music crate's placeholder of the name
//! (D67, D108).
//!
//! The arpeggios, chord brackets and chord slurs acknowledged (`:76–92`)
//! are a timestep's, each kind apart, with its note columns (`:94–98`).
//! At `process_acknowledged` (`:100–123`), of each kind, where there are
//! two or more and its `connectArpeggios`, `connectChordBrackets` or
//! `connectChordSlurs` is `#t`, one more of it, of no cause, spanning them,
//! which each note column takes as a conditional item. At
//! `stop_translation_timestep` (`:125–169`) the span takes its items'
//! stems and side supports, each item it as its
//! `vertically-spanning-surrogate` and is made transparent, and the span
//! the first item's Y parent.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, PROCESS_ACKNOWLEDGED, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Value};

use crate::clef_engraver::is_true;
use crate::geometry::Y;
use crate::grob_array::extract_grob_array;
use crate::{grob, grob_array, separation_item};

type Run<T> = Result<T, Exception>;

pub static SPAN_ARPEGGIO_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Span_arpeggio_engraver",
    kind: Kind::Engraver,
    methods: PROCESS_ACKNOWLEDGED | STOP,
    always: false,
    hears: &[],
    acknowledges: &[
        ("arpeggio-interface", false),
        ("chord-bracket-interface", false),
        ("chord-slur-interface", false),
        ("note-column-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(SpanArpeggioEngraver::default())),
    reads: &[
        "connectArpeggios",
        "connectChordBrackets",
        "connectChordSlurs",
    ],
    writes: &[],
};

/// `Chord_onset_info`: the grob and context property of one kind, the
/// timestep's items of it and its span item, by their handles.
#[derive(Clone)]
struct ChordOnsetInfo {
    item_name: &'static str,
    connect_property_name: &'static str,
    items: Vec<Value>,
    span_item: Option<Value>,
}

impl ChordOnsetInfo {
    const fn new(item_name: &'static str, connect_property_name: &'static str) -> Self {
        ChordOnsetInfo {
            item_name,
            connect_property_name,
            items: Vec::new(),
            span_item: None,
        }
    }
}

/// `arpeggio_`, `bracket_` and `slur_`, in that order, and
/// `note_columns_` by their handles.
#[derive(Clone)]
struct SpanArpeggioEngraver {
    infos: [ChordOnsetInfo; 3],
    note_columns: Vec<Value>,
}

impl Default for SpanArpeggioEngraver {
    fn default() -> Self {
        SpanArpeggioEngraver {
            infos: [
                ChordOnsetInfo::new("Arpeggio", "connectArpeggios"),
                ChordOnsetInfo::new("ChordBracket", "connectChordBrackets"),
                ChordOnsetInfo::new("ChordSlur", "connectChordSlurs"),
            ],
            note_columns: Vec::new(),
        }
    }
}

impl Work for SpanArpeggioEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        for i in &self.infos {
            h.flags(&[i.span_item.is_some()]);
            h.num(i.items.len() as i64);
        }
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        match self.infos.get_mut(which) {
            Some(info) => info.items.push(g),
            None => self.note_columns.push(g),
        }
        Ok(())
    }

    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for info in &mut self.infos {
            /*
              connectArpeggios is slightly brusque; we should really read a grob
              property of the caught non-span arpeggios. That way, we can have
              both non-connected and connected arps in one pianostaff.
            */
            if info.span_item.is_none()
                && info.items.len() > 1
                && is_true(tx.get(info.connect_property_name))
            {
                let id = tx.make_defined_grob(info.item_name, Cause::None)?;
                info.span_item = Some(tx.grob_handle(id));
            }
            if let Some(span) = info.span_item {
                for &col in &self.note_columns {
                    separation_item::add_conditional_item(tx.host, col, span)?;
                }
                self.note_columns.clear();
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for info in &mut self.infos {
            if let Some(span) = info.span_item.take() {
                /*
                  we do this very late, to make sure we also catch `extra'
                  side-pos support like accidentals.
                */
                let stems_sym = tx.heap().intern("stems");
                let sses_sym = tx.heap().intern("side-support-elements");
                let surrogate_sym = tx.heap().intern("vertically-spanning-surrogate");
                for &item in &info.items {
                    for stem in extract_grob_array(tx.host, item, "stems")? {
                        grob_array::add_grob(tx.host, span, stems_sym, stem)?;
                    }
                    for sse in extract_grob_array(tx.host, item, "side-support-elements")? {
                        grob_array::add_grob(tx.host, span, sses_sym, sse)?;
                    }
                    /*
                      we can't kill the children, since we don't want to the
                      previous note to bump into the span arpeggio; so we make
                      it transparent.
                    */
                    grob::set_object(tx.heap(), item, surrogate_sym, span);
                    grob::set_property(tx.heap(), item, "transparent", Value::TRUE);
                }
                if let Some(parent) = grob::get_parent(tx.heap(), info.items[0], Y) {
                    grob::set_parent(tx.heap(), span, parent, Y);
                }
            }
        }
        for info in &mut self.infos {
            info.items.clear();
        }
        self.note_columns.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        for info in &self.infos {
            out.extend(&info.items);
            out.extend(info.span_item);
        }
        out.extend(&self.note_columns);
    }
}
