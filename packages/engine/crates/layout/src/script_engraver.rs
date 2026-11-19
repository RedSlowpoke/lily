//! `script-engraver.cc` (part 4's, MS3): `make_script_from_event`
//! (`script-engraver.cc:94–158`, 61 lines), ported first for
//! `Multi_measure_rest_engraver`'s MultiMeasureRestScript (MS3 step 42);
//! `Caesura_engraver`, `New_fingering_engraver` and `Drum_notes_engraver`
//! call it too. Since MS3 step 53 `Script_engraver`, a class of layout's
//! ([`SCRIPT_ENGRAVER`]) that the core passes at its `\layout` call and
//! that a group finds before the music crate's placeholder of the name
//! (D67, D108):
//!
//! - The articulations heard (`listen_articulation`, `:79–91`), but one
//!   whose `articulation-type` the timestep has already (part combining's
//!   doubles).
//! - `process_music` (`:160–178`): a `Script` of each, its cause the
//!   event, made from its `articulation-type` with its place among the
//!   timestep's; its direction the event's where that is up or down.
//! - The acknowledgers (`:180–262`): a stem is each script's support, and
//!   its `direction-source` where the script has a
//!   `side-relative-direction`; a stem tremolo, a tie (at its start and at
//!   its end) and an inline accidental each script's support; a rhythmic
//!   head caused by an event each script's support, and the Y parent of
//!   one on the X axis that has none; a note column the X parent of each
//!   script on the Y axis that has none, which
//!   `ly:script-interface::calc-positioning-done` moves onto the stem's
//!   first head (`script.rs`).
//! - `stop_translation_timestep` (`:264–268`) forgets them.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, Kind, PROCESS, STOP, TranslatorClass, Tx, Work, event_property, grob_of,
};
use lily_engine_scheme::{Exception, Tag, Value};

use crate::geometry::{X, Y};
use crate::tie_engraver::event_cause;
use crate::{grob, side_position};

type Run<T> = Result<T, Exception>;

pub static SCRIPT_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Script_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["articulation-event"],
    acknowledges: &[
        ("rhythmic-head-interface", false),
        ("stem-interface", false),
        ("tie-interface", false),
        ("tie-interface", true),
        ("note-column-interface", false),
        ("stem-tremolo-interface", false),
        ("inline-accidental-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(ScriptEngraver::default())),
    reads: &["scriptDefinitions"],
    writes: &[],
};

/// `scripts_`: each event, and its script by its handle once made.
#[derive(Clone, Default)]
struct ScriptEngraver {
    scripts: Vec<(Value, Option<Value>)>,
}

impl ScriptEngraver {
    /// The scripts made.
    fn made(&self) -> impl Iterator<Item = Value> + '_ {
        self.scripts.iter().filter_map(|&(_, script)| script)
    }
}

impl Work for ScriptEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.scripts.len() as i64);
        for (_, o) in &self.scripts {
            h.flags(&[o.is_some()]);
        }
    }

    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        /* Discard double articulations for part-combining.  */
        let heap = tx.heap();
        let ty = event_property(heap, ev, "articulation-type");
        if self
            .scripts
            .iter()
            .any(|&(e, _)| event_property(heap, e, "articulation-type") == ty)
        {
            return Ok(());
        }
        self.scripts.push((ev, None));
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for i in 0..self.scripts.len() {
            let ev = self.scripts[i].0;
            let id = tx.make_defined_grob("Script", Cause::Event(ev))?;
            let p = tx.grob_handle(id);

            let art_type = event_property(tx.heap(), ev, "articulation-type");
            make_script_from_event(tx, p, art_type, i)?;

            self.scripts[i].1 = Some(p);

            let force_dir = event_property(tx.heap(), ev, "direction");
            if matches!(force_dir.as_fixnum(), Some(-1 | 1)) {
                grob::set_property(tx.heap(), p, "direction", force_dir);
            }
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        let scripts: Vec<Value> = self.made().collect();
        match which {
            // `acknowledge_rhythmic_head`
            0 => {
                if event_cause(tx, g)?.is_some() {
                    for e in scripts {
                        if side_position::is_on_x_axis(tx.host, e)?
                            && grob::get_parent(tx.heap(), e, Y).is_none()
                        {
                            grob::set_parent(tx.heap(), e, g, Y);
                        }
                        side_position::add_support(tx.host, e, g)?;
                    }
                }
            }
            // `acknowledge_stem`
            1 => {
                for e in scripts {
                    let d = grob::property(tx.host, e, "side-relative-direction")?;
                    if matches!(d.as_fixnum(), Some(-1 | 1)) {
                        let heap = tx.heap();
                        let sym = heap.intern("direction-source");
                        grob::set_object(heap, e, sym, g);
                    }
                    side_position::add_support(tx.host, e, g)?;
                }
            }
            // `acknowledge_note_column`: the note column the parent of
            // the script. That is not correct, but due to seconds in a
            // chord, noteheads may be swapped around horizontally; the
            // head is chosen at the script's positioning.
            4 => {
                for e in scripts {
                    if grob::get_parent(tx.heap(), e, X).is_none()
                        && side_position::is_on_y_axis(tx.host, e)?
                    {
                        grob::set_parent(tx.heap(), e, g, X);
                    }
                }
            }
            // `acknowledge_tie`, `acknowledge_end_tie`,
            // `acknowledge_stem_tremolo`, `acknowledge_inline_accidental`
            _ => {
                for e in scripts {
                    side_position::add_support(tx.host, e, g)?;
                }
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.scripts.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        for &(ev, script) in &self.scripts {
            out.push(ev);
            out.extend(script);
        }
    }
}

/// `make_script_from_event`: the grob `p` given the properties of the
/// articulation `art_type` in the context's `scriptDefinitions`, each
/// whose value is `'()` or whose value set on `p` (its property data, no
/// callback called) fails the property's `backend-type?`, its
/// `script-priority` (the definition's plus `index`, else `index`) in
/// user input's order. LilyPond's programming errors for an `art_type`
/// that is no symbol and for a definition's property that has no
/// `backend-type?`, which is skipped; its warning for an articulation
/// with no definition, which sets nothing. Guile's error of `+` for a
/// `script-priority` that is no number. A `scriptDefinitions` that is no
/// alist is read as `assq` reads its pairs, an entry that is no pair
/// skipped.
pub fn make_script_from_event(
    tx: &mut Tx<'_>,
    p: Value,
    art_type: Value,
    index: usize,
) -> Result<(), Exception> {
    // Breathing_sign has similar (but simpler) code.  A change here might
    // warrant a change there.

    if art_type.tag() != Tag::Symbol {
        let msg = format!(
            "articulation-type must be a symbol since 2.23.6: {}",
            written(tx, art_type)
        );
        grob::programming_error(tx.host, p, &msg)?;
    }

    let alist = tx.get("scriptDefinitions");
    let Some(art) = tx.heap().assq(art_type, alist) else {
        let msg = format!(
            "do not know how to interpret articulation: {}",
            written(tx, art_type)
        );
        return grob::warning(tx.host, p, &msg);
    };

    let depth = tx.heap().root_depth();
    let ran = set_properties(tx, p, art, index);
    tx.heap().unroot_to(depth);
    ran
}

/// The loop over the definition `art` (its entry in `scriptDefinitions`)
/// and the `script-priority` set last; what is made is rooted until the
/// caller unroots.
fn set_properties(tx: &mut Tx<'_>, p: Value, art: Value, index: usize) -> Result<(), Exception> {
    let heap = tx.heap();
    heap.root(p);
    heap.root(art);
    let mut props = heap.cdr(art).unwrap_or(Value::NULL);
    let backend_type = heap.intern("backend-type?");
    let script_priority = heap.intern("script-priority");
    let object_property = crate::lily_procedure(tx.host, "object-property")?;

    let mut priority_found = false;

    while let (Some(prop_pair), Some(rest)) = (tx.heap().car(props), tx.heap().cdr(props)) {
        props = rest;
        let heap = tx.heap();
        let (Some(sym), Some(mut val)) = (heap.car(prop_pair), heap.cdr(prop_pair)) else {
            return Err(crate::wrong_type(heap, Some("car"), 1, prop_pair, "pair"));
        };
        let ty = tx.host.call(object_property, &[sym, backend_type])?;
        if !tx.heap().is_procedure(ty) {
            let msg = format!(
                "invalid grob property name in script definition: {}",
                written(tx, sym)
            );
            grob::programming_error(tx.host, p, &msg)?;
            continue;
        }
        tx.heap().root(ty);

        if sym == script_priority {
            priority_found = true;
            /* Make sure they're in order of user input by adding index i.
            Don't use the direction in this priority. Smaller means closer
            to the head.  */
            val = sum(tx, val, index)?;
            tx.heap().root(val);
        }

        let preset = property_data(tx, p, sym);
        if val == Value::NULL || tx.host.call(ty, &[preset])? == Value::FALSE {
            let heap = tx.heap();
            if let Some((arena, id)) = grob_of(heap, p) {
                grob::set_property_data(heap, &arena, id, sym, val);
            }
        }
    }

    if !priority_found {
        let heap = tx.heap();
        let index = heap.integer(index as i64);
        grob::set_property(heap, p, "script-priority", index);
    }
    Ok(())
}

/// `get_property_data (p, sym)`, `'()` for what is no grob.
fn property_data(tx: &mut Tx<'_>, p: Value, sym: Value) -> Value {
    let heap = tx.heap();
    match grob_of(heap, p) {
        Some((arena, id)) => grob::property_data(heap, &arena, id, p, sym),
        None => Value::NULL,
    }
}

/// `scm_sum (val, to_scm (index))`: Guile's `+`, whose error is the
/// image's for a `val` that is no number.
fn sum(tx: &mut Tx<'_>, val: Value, index: usize) -> Result<Value, Exception> {
    let heap = tx.heap();
    let i = heap.integer(index as i64);
    if let Ok(v) = heap.add(val, i) {
        return Ok(v);
    }
    let plus = crate::lily_procedure(tx.host, "+")?;
    tx.host.call(plus, &[val, i])
}

/// `ly_scm_write_string (v)`.
fn written(tx: &mut Tx<'_>, v: Value) -> String {
    let mut out = String::new();
    tx.heap().write(v, &mut out);
    out
}
