//! `tab-note-heads-engraver.cc` (MS4 step 7a): `Tab_note_heads_engraver`
//! ([`TAB_NOTE_HEADS_ENGRAVER`]), with `articulations.cc`'s
//! `articulation_list` (borrowed).
//!
//! - The notes, string numbers and fingerings heard (`:62–78`), forgotten
//!   at the timestep's end (`:125–131`).
//! - `process_music` (`:80–123`): each note's string number and finger,
//!   from its articulations or else the next of those heard on their own
//!   ([`articulation_list`]); `noteToFretFunction`, if a procedure, called
//!   with the context, the notes and both lists, which the image runs
//!   (`determine-frets`); then for each entry of what it gives whose string
//!   is not `#f`, a `TabNoteHead` caused by its note (by the first note if
//!   the entries are not one a note), its `text` what `tablatureFormat`
//!   gives of the context, the string and the fret, and its
//!   `staff-position` what `tabStaffLineLayoutFunction` gives of the
//!   context and the string. An entry that is not a list is Guile's type
//!   error, as `scm_car` and `scm_cadr` throw it; so is a result that is
//!   not a proper list, past its pairs, as `scm_list_ref` throws it on the
//!   `scm_ilength` of -1 that LilyPond counts to. With no note, where
//!   LilyPond's would read past its notes, an entry makes no head.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_layout::{grob, wrong_type};
use lily_engine_music::{
    Cause, Kind, PROCESS, STOP, StreamEvent, TranslatorClass, Tx, Work, event_property,
};
use lily_engine_scheme::{Exception, Heap, Value};

type Run<T> = Result<T, Exception>;

pub static TAB_NOTE_HEADS_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Tab_note_heads_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["note-event", "string-number-event", "fingering-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(TabNoteHeadsEngraver::default())),
    reads: &[
        "defaultStrings",
        "fretLabels",
        "highStringOne",
        "maximumFretStretch",
        "middleCPosition",
        "minimumFret",
        "noteToFretFunction",
        "stringOneTopmost",
        "stringTunings",
        "tablatureFormat",
        "tabStaffLineLayoutFunction",
    ],
    writes: &[],
};

/// `note_events_`, `tabstring_events_` and `fingering_events_`.
#[derive(Clone, Default)]
struct TabNoteHeadsEngraver {
    notes: Vec<Value>,
    strings: Vec<Value>,
    fingerings: Vec<Value>,
}

/// `scm_ilength`: the length of a proper list, `None` for an improper or
/// circular one.
pub(crate) fn ilength(heap: &Heap, list: Value) -> Option<usize> {
    let (mut slow, mut fast) = (list, list);
    let mut n = 0;
    loop {
        if fast == Value::NULL {
            return Some(n);
        }
        fast = heap.cdr(fast)?;
        n += 1;
        if fast == Value::NULL {
            return Some(n);
        }
        fast = heap.cdr(fast)?;
        n += 1;
        slow = heap.cdr(slow)?;
        if fast == slow {
            return None;
        }
    }
}

/// `Stream_event::in_event_class`: whether the event's classes have
/// `class`.
fn in_event_class(heap: &mut Heap, ev: Value, class: &str) -> bool {
    let class = heap.intern(class);
    let mut p = event_property(heap, ev, "class");
    while let (Some(c), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        if c == class {
            return true;
        }
        p = rest;
    }
    false
}

/// `articulation_list`: for each note, its first articulation of the class
/// `name`, or else the next of `events` (the last again once they run
/// out), or `'()`.
pub(crate) fn articulation_list(
    heap: &mut Heap,
    notes: &[Value],
    events: &[Value],
    name: &str,
) -> Value {
    let mut out = Vec::with_capacity(notes.len());
    let mut j = 0;
    for &note in notes {
        let mut found = None;
        let mut arts = event_property(heap, note, "articulations");
        while let (Some(art), Some(rest)) = (heap.car(arts), heap.cdr(arts)) {
            if heap.foreign::<StreamEvent>(art).is_some() && in_event_class(heap, art, name) {
                found = Some(art);
                break;
            }
            arts = rest;
        }
        if found.is_none() && j < events.len() {
            found = Some(events[j]);
            if j + 1 < events.len() {
                j += 1;
            }
        }
        out.push(found.unwrap_or(Value::NULL));
    }
    heap.list(&out)
}

/// `scm_car` and `scm_cadr` of `v`, with Guile's type error, which names
/// `v` and no position.
fn nth(heap: &mut Heap, v: Value, n: usize, subr: &str) -> Run<Value> {
    let mut p = v;
    for _ in 0..n {
        p = heap.cdr(p).unwrap_or(Value::FALSE);
    }
    heap.car(p)
        .ok_or_else(|| wrong_type(heap, Some(subr), 0, v, "pair"))
}

/// `scm_list_ref`'s `SCM_WRONG_TYPE_ARG (1, list)`: an improper list.
fn not_a_list(heap: &mut Heap, list: Value) -> Exception {
    let key = heap.intern("wrong-type-arg");
    let subr = heap.string("list-ref");
    let message = heap.string("Wrong type argument in position ~A: ~S");
    let position = heap.integer(1);
    let items = heap.list(&[position, list]);
    let rest = heap.list(&[list]);
    let args = heap.list(&[subr, message, items, rest]);
    Exception { key, args }
}

impl Work for TabNoteHeadsEngraver {
    fn listen(&mut self, _: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        match which {
            0 => self.notes.push(ev),
            1 => self.strings.push(ev),
            _ => self.fingerings.push(ev),
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let depth = tx.heap().root_depth();
        let ran = self.make_heads(tx);
        tx.heap().unroot_to(depth);
        ran
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.notes.clear();
        self.strings.clear();
        self.fingerings.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend_from_slice(&self.notes);
        out.extend_from_slice(&self.strings);
        out.extend_from_slice(&self.fingerings);
    }
}

impl TabNoteHeadsEngraver {
    /// `process_music`, its values rooted by the caller's depth.
    fn make_heads(&self, tx: &mut Tx<'_>) -> Run<()> {
        let heap = tx.heap();
        for &v in self
            .notes
            .iter()
            .chain(&self.strings)
            .chain(&self.fingerings)
        {
            heap.root(v);
        }
        let tab_strings =
            articulation_list(heap, &self.notes, &self.strings, "string-number-event");
        heap.root(tab_strings);
        let defined_fingers =
            articulation_list(heap, &self.notes, &self.fingerings, "fingering-event");
        heap.root(defined_fingers);
        let tab_notes = heap.list(&self.notes);
        heap.root(tab_notes);
        let proc = tx.get("noteToFretFunction");
        let context = tx.t.arena.borrow().handle(tx.context);
        let mut string_fret_finger = Value::NULL;
        if tx.heap().is_procedure(proc) {
            let specified = tx.heap().list(&[tab_strings, defined_fingers]);
            tx.heap().root(specified);
            string_fret_finger = tx.host.call(proc, &[context, tab_notes, specified])?;
            tx.heap().root(string_fret_finger);
        }
        let fret_procedure = tx.get("tablatureFormat");
        let staff_line_procedure = tx.get("tabStaffLineLayoutFunction");
        let fret_count = ilength(tx.heap(), string_fret_finger);
        let length_changed = fret_count != Some(self.notes.len());

        let mut entries = string_fret_finger;
        let mut i = 0;
        while entries != Value::NULL {
            // `scm_list_ref` past the pairs of an improper list.
            let (Some(note_entry), Some(rest)) = (tx.heap().car(entries), tx.heap().cdr(entries))
            else {
                return Err(not_a_list(tx.heap(), string_fret_finger));
            };
            entries = rest;
            let string_number = nth(tx.heap(), note_entry, 0, "car")?;
            if string_number != Value::FALSE {
                let fret = nth(tx.heap(), note_entry, 1, "cadr")?;
                let fret_label = tx
                    .host
                    .call(fret_procedure, &[context, string_number, fret])?;
                tx.heap().root(fret_label);
                let index = if length_changed { 0 } else { i };
                let Some(&ev) = self.notes.get(index) else {
                    i += 1;
                    continue;
                };
                let id = tx.make_defined_grob("TabNoteHead", Cause::Event(ev))?;
                let note = tx.grob_handle(id);
                grob::set_property(tx.heap(), note, "text", fret_label);
                let staff_position = tx
                    .host
                    .call(staff_line_procedure, &[context, string_number])?;
                grob::set_property(tx.heap(), note, "staff-position", staff_position);
            }
            i += 1;
        }
        Ok(())
    }
}
