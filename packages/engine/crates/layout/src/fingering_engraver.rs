//! `fingering-engraver.cc` (part 4's, MS3 step 53, 169 lines):
//! `Fingering_engraver`, a class of layout's ([`FINGERING_ENGRAVER`]) that
//! the core passes at its `\layout` call and that a group finds before the
//! music crate's placeholder of the name (D67, D108). It makes the
//! fingerings of single notes; a chord's are `New_fingering_engraver`'s.
//!
//! - The fingering events heard (`listen_fingering`, `:47–51`).
//! - `process_music` (`:85–93`), the events last to first: a `Fingering`
//!   of each (`make_script`, `:95–123`), its cause the event, on the Y axis
//!   of its side position and aligned on its X parent, its
//!   `script-priority` its own (200 if it is no number) plus the event's
//!   place among the timestep's, its direction the event's, else up where
//!   it has no direction set.
//! - The acknowledgers (`:53–83`): a stem, a flag and a rhythmic head each
//!   fingering's support; a note column each fingering's X parent, which it
//!   aligns on the column's main note heads.
//! - `stop_translation_timestep` (`:125–130`) forgets them.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, event_property, grob_of,
};
use lily_engine_scheme::{Exception, Value};

use crate::dynamic_align_engraver::direction;
use crate::geometry::{Axis, Direction, X, Y};
use crate::{grob, side_position};

type Run<T> = Result<T, Exception>;

pub static FINGERING_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Fingering_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["fingering-event"],
    acknowledges: &[
        ("rhythmic-head-interface", false),
        ("stem-interface", false),
        ("flag-interface", false),
        ("note-column-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(FingeringEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `events_` and `fingerings_`, by their handles.
#[derive(Clone, Default)]
struct FingeringEngraver {
    events: Vec<Value>,
    fingerings: Vec<Value>,
}

/// `Self_alignment_interface::set_aligned_on_parent`: the grob's offset on
/// `a` added `ly:self-alignment-interface::aligned-on-x-parent` (or
/// `…-y-parent`).
pub(crate) fn set_aligned_on_parent(tx: &mut Tx<'_>, me: Value, a: Axis) -> Run<()> {
    let name = if a == X {
        "ly:self-alignment-interface::aligned-on-x-parent"
    } else {
        "ly:self-alignment-interface::aligned-on-y-parent"
    };
    let proc = crate::lily_procedure(tx.host, name)?;
    grob::add_offset_callback(tx.host, me, proc, a)
}

/// `get_property_data (me, name)`: what is set for `name` on the grob,
/// no callback called.
pub(crate) fn property_data(tx: &mut Tx<'_>, me: Value, name: &str) -> Value {
    let heap = tx.heap();
    let sym = heap.intern(name);
    match grob_of(heap, me) {
        Some((arena, id)) => grob::property_data(heap, &arena, id, me, sym),
        None => Value::NULL,
    }
}

impl FingeringEngraver {
    /// `make_script`: the fingering of the event `r`, the `i`th, in the
    /// direction `d`.
    fn make_script(&mut self, tx: &mut Tx<'_>, d: Direction, r: Value, i: usize) -> Run<()> {
        let id = tx.make_defined_grob("Fingering", Cause::Event(r))?;
        let fingering = tx.grob_handle(id);

        // We can't fold these definitions into define-grobs since
        // fingerings for chords need different settings.
        side_position::set_axis(tx.host, fingering, Y)?;
        set_aligned_on_parent(tx, fingering, X)?;

        /* See script-engraver.cc */
        let mut priority = grob::property(tx.host, fingering, "script-priority")?;
        let heap = tx.heap();
        if !heap.is_number(priority) {
            priority = heap.integer(200); // TODO: Explain magic.
        }
        let i = heap.integer(i as i64);
        // A number and an integer: `+` cannot fail.
        let priority = heap.add(priority, i).unwrap_or(priority);
        grob::set_property(heap, fingering, "script-priority", priority);

        if d != 0 {
            let d = tx.heap().integer(i64::from(d));
            grob::set_property(tx.heap(), fingering, "direction", d);
        } else if !matches!(
            property_data(tx, fingering, "direction").as_fixnum(),
            Some(-1..=1)
        ) {
            let up = tx.heap().integer(1);
            grob::set_property(tx.heap(), fingering, "direction", up);
        }

        self.fingerings.push(fingering);
        Ok(())
    }
}

impl Work for FingeringEngraver {
    fn listen(&mut self, _: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        self.events.push(ev);
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for i in (0..self.events.len()).rev() {
            let ev = self.events[i];
            let dir = event_property(tx.heap(), ev, "direction");
            self.make_script(tx, direction(dir), ev, i)?;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        if which == 3 {
            // set NoteColumn as parent and X-align on main noteheads
            let heap = tx.heap();
            for &t in &self.fingerings {
                grob::set_parent(heap, t, g, X);
                grob::set_property(heap, t, "X-align-on-main-noteheads", Value::TRUE);
            }
        } else {
            for &f in &self.fingerings {
                side_position::add_support(tx.host, f, g)?;
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.fingerings.clear();
        self.events.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(&self.events);
        out.extend(&self.fingerings);
    }
}
