//! `new-fingering-engraver.cc` (part 4's, MS3 step 53, 440 lines):
//! `New_fingering_engraver`, a class of layout's
//! ([`NEW_FINGERING_ENGRAVER`]) that the core passes at its `\layout` call
//! and that a group finds before the music crate's placeholder of the name
//! (D67, D108). It makes the fingerings, string numbers, stroke fingers
//! and scripts of a chord's notes, and harmonic heads.
//!
//! - A rhythmic head caused by a note event (`acknowledge_rhythmic_head`,
//!   `:84–125`): of each of the note's `articulations`, a `Fingering`,
//!   `StringNumber` or `StrokeFinger` supported by the head (`add_fingering`,
//!   `:160–178`), a `Script` on the head made from its `articulation-type`
//!   (`add_script`, `:142–158`, part 3's `make_script_from_event`), the
//!   head's `style` `harmonic` and its dot killed unless `harmonicDots`;
//!   LilyPond's warning for a text script. The head is kept.
//! - The inline accidentals, the stem and the note column acknowledged are
//!   kept (`:78–82`, `:127–140`).
//! - `stop_translation_timestep` (`:345–353`) positions them all
//!   (`position_all`, `:355–395`): each kind by its context's orientations
//!   (`position_scripts`, `:180–343`), then each script supported by the
//!   accidentals (of the first alone, as LilyPond clears them there), the
//!   heads and the stem as it asks.
//!
//! LilyPond sorts the fingers by staff position with `std::sort`, which is
//! not stable; they are sorted stably here. Only two fingers at the same
//! staff position could be put in another order.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, STOP, TranslatorClass, Tx, Work, event_property, grob_of};
use lily_engine_scheme::{Exception, Value};

use crate::dynamic_align_engraver::direction;
use crate::fingering_engraver::set_aligned_on_parent;
use crate::geometry::{X, Y};
use crate::script_engraver::make_script_from_event;
use crate::stem::{int_or, to_int};
use crate::stencil::Stencil;
use crate::tie_engraver::{event_cause, in_event_class};
use crate::{grob, grob_array, rhythmic_head, side_position};

type Run<T> = Result<T, Exception>;

pub static NEW_FINGERING_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "New_fingering_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    always: false,
    hears: &[],
    acknowledges: &[
        ("rhythmic-head-interface", false),
        ("inline-accidental-interface", false),
        ("stem-interface", false),
        ("note-column-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(NewFingeringEngraver::default())),
    reads: &[
        "fingeringOrientations",
        "harmonicDots",
        "strokeFingerOrientations",
        "stringNumberOrientations",
    ],
    writes: &[],
};

/// `Finger_tuple`: the head, the script made, the finger's event, and
/// the head's staff position once positioned (its note's event is not
/// read).
#[derive(Clone, Copy)]
struct FingerTuple {
    head: Value,
    script: Value,
    finger_event: Value,
    position: i32,
}

/// The vectors of tuples, the heads and accidentals, the stem and the
/// note column, by their handles.
#[derive(Clone, Default)]
struct NewFingeringEngraver {
    fingerings: Vec<FingerTuple>,
    stroke_fingerings: Vec<FingerTuple>,
    articulations: Vec<Value>,
    string_numbers: Vec<FingerTuple>,

    heads: Vec<Value>,
    accidentals: Vec<Value>,
    stem: Option<Value>,
    note_column: Option<Value>,
}

/// Whether the symbol `name` is a member of the list `orientations`.
fn memq(tx: &mut Tx<'_>, name: &str, mut orientations: Value) -> bool {
    let heap = tx.heap();
    let sym = heap.intern(name);
    while let (Some(x), Some(rest)) = (heap.car(orientations), heap.cdr(orientations)) {
        if x == sym {
            return true;
        }
        orientations = rest;
    }
    false
}

/// `get_object (g, name)` if it is a grob.
fn grob_object(tx: &mut Tx<'_>, g: Value, name: &str) -> Run<Option<Value>> {
    let sym = tx.heap().intern(name);
    let o = grob_array::object(tx.host, g, sym)?;
    Ok(grob_of(tx.heap(), o).map(|_| o))
}

impl NewFingeringEngraver {
    /// `add_script`: a `Script` of the event on the head, its direction the
    /// event's where that is up or down.
    fn add_script(&mut self, tx: &mut Tx<'_>, head: Value, event: Value) -> Run<()> {
        let id = tx.make_defined_grob("Script", Cause::Event(event))?;
        let g = tx.grob_handle(id);
        let art_type = event_property(tx.heap(), event, "articulation-type");
        make_script_from_event(tx, g, art_type, 0)?;
        grob::set_parent(tx.heap(), g, head, X);

        let forced_dir = event_property(tx.heap(), event, "direction");
        if direction(forced_dir) != 0 {
            grob::set_property(tx.heap(), g, "direction", forced_dir);
        }

        self.articulations.push(g);
        Ok(())
    }

    /// `position_scripts`: the fingers of one kind supported by the stem
    /// and its flag and by every head; those whose event has a direction
    /// put there, the rest sorted by their heads' staff positions and put
    /// by `orientations`: beside the chord (left, or right if it has
    /// `right`), its top one above and bottom one below where it has `up`
    /// or `down` too; else half below and half above, all above, or all
    /// below (with LilyPond's warnings where it has none of these).
    fn position_scripts(
        &mut self,
        tx: &mut Tx<'_>,
        orientations: Value,
        mut scripts: Vec<FingerTuple>,
    ) -> Run<()> {
        if let Some(stem) = self.stem {
            for ft in &scripts {
                side_position::add_support(tx.host, ft.script, stem)?;
                if let Some(flag) = grob_object(tx, stem, "flag")? {
                    side_position::add_support(tx.host, ft.script, flag)?;
                }
            }
        }

        // This is not extremely elegant, but we have to do a little
        // formatting here, because the parent/child relations should be
        // known before we move on to the next time step.
        for ft in &mut scripts {
            let p = grob::property(tx.host, ft.head, "staff-position")?;
            ft.position = to_int(tx.heap(), p)?;
        }

        for ft in scripts.iter().rev() {
            for &h in self.heads.iter().rev() {
                side_position::add_support(tx.host, ft.script, h)?;
            }
        }

        let (mut up, mut down, mut horiz) = (Vec::new(), Vec::new(), Vec::new());
        for i in (0..scripts.len()).rev() {
            let d = event_property(tx.heap(), scripts[i].finger_event, "direction");
            match direction(d) {
                0 => {}
                d => {
                    if d > 0 { &mut up } else { &mut down }.push(scripts[i]);
                    scripts.remove(i);
                }
            }
        }

        scripts.sort_by_key(|ft| ft.position);

        let up_p = memq(tx, "up", orientations);
        let down_p = memq(tx, "down", orientations);
        let left_p = memq(tx, "left", orientations);
        let right_p = memq(tx, "right", orientations);
        let hordir = if right_p { 1 } else { -1 };
        if left_p || right_p {
            if up_p
                && up.is_empty()
                && let Some(ft) = scripts.pop()
            {
                up.push(ft);
            }

            if down_p && down.is_empty() && !scripts.is_empty() {
                down.push(scripts.remove(0));
            }

            horiz.extend_from_slice(&scripts);
        } else if up_p && down_p {
            let center = scripts.len() / 2;
            down.extend_from_slice(&scripts[..center]);
            up.extend_from_slice(&scripts[center..]);
        } else if up_p {
            up.append(&mut scripts);
        } else {
            if !down_p {
                tx.warning("no placement found for fingerings")?;
                tx.warning("placing below")?;
            }
            down.append(&mut scripts);
        }

        for ft in &horiz {
            let f = ft.script;
            grob::set_parent(tx.heap(), f, ft.head, X);
            grob::set_parent(tx.heap(), f, ft.head, Y);
            let inside = tx.heap().intern("inside");
            grob::set_property(tx.heap(), f, "avoid-slur", inside);
            let accidental = grob_object(tx, ft.head, "accidental-grob")?;
            if hordir == -1
                && let Some(accidental) = accidental
            {
                side_position::add_support(tx.host, f, accidental)?;
            } else if rhythmic_head::dot_count(tx.host, ft.head)? != 0 {
                for j in 0..self.heads.len() {
                    if let Some(d) = grob_object(tx, self.heads[j], "dot")? {
                        side_position::add_support(tx.host, f, d)?;
                    }
                }
            }

            if horiz.len() > 1 {
                // -> FingeringColumn
                // Ouch, should do this in the typesetting phase. --JeanAS
                let stil = grob::property(tx.host, f, "stencil")?;
                if let Some(&stil) = tx.heap().foreign::<Stencil>(stil) {
                    let depth = tx.heap().root_depth();
                    let mut aligned = stil;
                    aligned.align_to(tx.heap(), Y, 0.0);
                    tx.heap().root(aligned.expr);
                    let aligned = tx.heap().wrap(aligned);
                    grob::set_property(tx.heap(), f, "stencil", aligned);
                    tx.heap().unroot_to(depth);
                }
            } else {
                let one = tx.heap().integer(1);
                let self_align_y = crate::call_lily(tx.host, "%aligned-on-parent", &[f, one])?;
                let mut self_align_y = tx.heap().to_f64(self_align_y).unwrap_or(0.0);
                let yoff = grob::property(tx.host, f, "Y-offset")?;
                if tx.heap().is_number(yoff) {
                    self_align_y += crate::spacing_options::double_or(tx.host, yoff, 0.0);
                }
                let v = tx.heap().flonum(self_align_y);
                grob::set_property(tx.heap(), f, "Y-offset", v);
            }

            side_position::set_axis(tx.host, f, X)?;

            let d = tx.heap().integer(hordir);
            grob::set_property(tx.heap(), f, "direction", d);
        }

        for (d, vertical) in [(-1, &down), (1, &up)] {
            for ft in vertical {
                let f = ft.script;
                let prio = grob::property(tx.host, f, "script-priority")?;
                let finger_prio = int_or(tx.heap(), prio, 200);

                let main = grob::property(tx.host, f, "X-align-on-main-noteheads")? == Value::TRUE;
                if self.heads.len() > 1 && main {
                    // LilyPond sets no parent for no note column.
                    if let Some(c) = self.note_column {
                        grob::set_parent(tx.heap(), f, c, X);
                    }
                } else {
                    grob::set_parent(tx.heap(), f, ft.head, X);
                    if self.heads.len() > 1 {
                        for &a in &self.accidentals {
                            side_position::add_support(tx.host, f, a)?;
                        }
                    }
                }

                let p = tx.heap().integer(i64::from(finger_prio + d * ft.position));
                grob::set_property(tx.heap(), f, "script-priority", p);

                set_aligned_on_parent(tx, f, X)?;
                side_position::set_axis(tx.host, f, Y)?;

                let d = tx.heap().integer(i64::from(d));
                grob::set_property(tx.heap(), f, "direction", d);
            }
        }
        Ok(())
    }

    /// `position_all`: the fingerings, string numbers and stroke fingers
    /// positioned by their orientations, then the scripts last to first
    /// supported by the accidentals (which the first clears), the heads,
    /// and the stem where they ask, and given the stem as their
    /// `direction-source` where they have a `side-relative-direction`.
    fn position_all(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if !self.fingerings.is_empty() {
            let o = tx.get("fingeringOrientations");
            let scripts = core::mem::take(&mut self.fingerings);
            self.position_scripts(tx, o, scripts)?;
        }

        if !self.string_numbers.is_empty() {
            let o = tx.get("stringNumberOrientations");
            let scripts = core::mem::take(&mut self.string_numbers);
            self.position_scripts(tx, o, scripts)?;
        }

        if !self.stroke_fingerings.is_empty() {
            let o = tx.get("strokeFingerOrientations");
            let scripts = core::mem::take(&mut self.stroke_fingerings);
            self.position_scripts(tx, o, scripts)?;
        }

        for i in (0..self.articulations.len()).rev() {
            let script = self.articulations[i];
            for &a in &self.accidentals {
                side_position::add_support(tx.host, script, a)?;
            }

            self.accidentals.clear();
            for &h in self.heads.iter().rev() {
                side_position::add_support(tx.host, script, h)?;
            }

            if let Some(stem) = self.stem {
                let d = grob::property(tx.host, script, "side-relative-direction")?;
                if direction(d) != 0 {
                    let heap = tx.heap();
                    let sym = heap.intern("direction-source");
                    grob::set_object(heap, script, sym, stem);
                }
                if grob::property(tx.host, script, "add-stem-support")? == Value::TRUE {
                    side_position::add_support(tx.host, script, stem)?;
                }
            }
        }
        self.articulations.clear();
        Ok(())
    }

    /// `add_fingering`: a grob `grob_sym` of the finger's event, supported
    /// by the head.
    fn add_fingering(
        tx: &mut Tx<'_>,
        head: Value,
        grob_sym: &str,
        event: Value,
    ) -> Run<FingerTuple> {
        let id = tx.make_defined_grob(grob_sym, Cause::Event(event))?;
        let script = tx.grob_handle(id);

        side_position::add_support(tx.host, script, head)?;

        Ok(FingerTuple {
            head,
            script,
            finger_event: event,
            position: 0,
        })
    }
}

impl Work for NewFingeringEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        for list in [
            &self.fingerings,
            &self.stroke_fingerings,
            &self.string_numbers,
        ] {
            h.num(list.len() as i64);
            for f in list {
                h.num(f.position.into());
            }
        }
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        match which {
            0 => self.acknowledge_rhythmic_head(tx, g)?,
            1 => self.accidentals.push(g),
            2 => self.stem = Some(g),
            _ => self.note_column = Some(g),
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.position_all(tx)?;
        self.stem = None;
        self.note_column = None;
        self.heads.clear();
        self.accidentals.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        for ft in self
            .fingerings
            .iter()
            .chain(&self.stroke_fingerings)
            .chain(&self.string_numbers)
        {
            out.extend([ft.head, ft.script, ft.finger_event]);
        }
        out.extend(&self.articulations);
        out.extend(&self.heads);
        out.extend(&self.accidentals);
        out.extend(self.stem);
        out.extend(self.note_column);
    }
}

impl NewFingeringEngraver {
    /// `acknowledge_rhythmic_head`.
    fn acknowledge_rhythmic_head(&mut self, tx: &mut Tx<'_>, head: Value) -> Run<()> {
        let Some(note_ev) = event_cause(tx, head)? else {
            return Ok(());
        };

        let mut arts = event_property(tx.heap(), note_ev, "articulations");
        while let (Some(ev), Some(rest)) = (tx.heap().car(arts), tx.heap().cdr(arts)) {
            arts = rest;
            if tx
                .heap()
                .foreign::<lily_engine_music::StreamEvent>(ev)
                .is_none()
            {
                continue;
            }

            let heap = tx.heap();
            if in_event_class(heap, ev, "fingering-event") {
                let ft = Self::add_fingering(tx, head, "Fingering", ev)?;
                self.fingerings.push(ft);
            } else if in_event_class(heap, ev, "text-script-event") {
                tx.event_warning(ev, "cannot add text scripts to individual note heads")?;
            } else if in_event_class(heap, ev, "script-event") {
                self.add_script(tx, head, ev)?;
            } else if in_event_class(heap, ev, "string-number-event") {
                let ft = Self::add_fingering(tx, head, "StringNumber", ev)?;
                self.string_numbers.push(ft);
            } else if in_event_class(heap, ev, "stroke-finger-event") {
                let ft = Self::add_fingering(tx, head, "StrokeFinger", ev)?;
                self.stroke_fingerings.push(ft);
            } else if in_event_class(heap, ev, "harmonic-event") {
                let harmonic = heap.intern("harmonic");
                grob::set_property(heap, head, "style", harmonic);
                if let Some(d) = grob_object(tx, head, "dot")?
                    && tx.get("harmonicDots") != Value::TRUE
                {
                    grob::suicide(tx.heap(), d);
                }
            }
        }

        self.heads.push(head);
        Ok(())
    }
}
