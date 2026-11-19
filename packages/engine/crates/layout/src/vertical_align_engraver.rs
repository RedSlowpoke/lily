//! `vertical-align-engraver.cc` (MS3 step 21): `Vertical_align_engraver`,
//! a class of layout's ([`VERTICAL_ALIGN_ENGRAVER`]) that the core passes
//! at its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - `initialize` (`:97–101`): the table of each context id's last group.
//! - `process_music` (`:103–123`): at its first, an engraver whose context
//!   sees `hasAxisGroup` `#t` gives LilyPond's warning and does nothing
//!   more; else, by `topLevelAlignment`, a `VerticalAlignment` or a
//!   `StaffGrouper` (`:118`), with no cause and `currentCommandColumn` its
//!   left bound. As it must run at the first timestep, the class runs
//!   always.
//! - Each axis group acknowledged (`:136–212`, `hara-kiri-group-spanner`):
//!   in a top-level alignment, the group kept under its context's id and
//!   added through `Align_interface::add_element` (`align.rs`); with its
//!   context's `alignAboveContext` or `alignBelowContext`, moved before or
//!   after the last group of that id, its `staff-affinity`, if a number,
//!   `DOWN` or `UP`, or LilyPond's warning for an id with no group. In a
//!   staff grouper, the group added to its `elements` and given it as its
//!   `staff-grouper` if it has none.
//! - Each outside-staff grob acknowledged (`:214–232`) of the engraver's
//!   own context: a staff grouper its Y parent; LilyPond's programming
//!   error before there is one.
//! - `finalize` (`:125–134`): `currentCommandColumn` the right bound.
//!
//! Without a command column, where LilyPond's would set a null bound, none
//! is set (`ledger_line_engraver::set_bound`); an axis group acknowledged
//! before the first `process_music`, where LilyPond's would crash, does
//! nothing.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, INITIALIZE, Kind, PROCESS, TranslatorClass, Tx, Work, grob_of,
};
use lily_engine_scheme::{Exception, Heap, Value};

use crate::align;
use crate::clef_engraver::is_true;
use crate::geometry::Y;
use crate::grob;
use crate::grob_array::{self, GrobArray};
use crate::ledger_line_engraver::{command_column, set_bound};

type Run<T> = Result<T, Exception>;

pub static VERTICAL_ALIGN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Vertical_align_engraver",
    kind: Kind::Engraver,
    methods: INITIALIZE | PROCESS | FINALIZE,
    always: true,
    hears: &[],
    acknowledges: &[
        ("hara-kiri-group-spanner-interface", false),
        ("outside-staff-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(VerticalAlignEngraver::default())),
    reads: &["alignAboveContext", "alignBelowContext", "hasAxisGroup"],
    writes: &[],
};

/// `valign_` by its handle, `id_to_group_hashtab_` (`None` where LilyPond's
/// is `'()`: before `initialize`, and after the warning) and `top_level_`.
#[derive(Clone, Default)]
struct VerticalAlignEngraver {
    valign: Option<Value>,
    id_to_group: Option<Vec<(String, Value)>>,
    top_level: bool,
}

impl Work for VerticalAlignEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        match &self.id_to_group {
            Some(map) => {
                h.num(map.len() as i64);
                for (id, _) in map {
                    h.text(id);
                }
            }
            None => h.num(-1),
        }
        h.flags(&[self.top_level]);
    }

    fn initialize(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.id_to_group = Some(Vec::new());
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.valign.is_none() && self.id_to_group.is_some() {
            if is_true(tx.get("hasAxisGroup")) {
                tx.warning("Ignoring Vertical_align_engraver in VerticalAxisGroup")?;
                self.id_to_group = None;
                return Ok(());
            }

            self.top_level = is_true(tx.get("topLevelAlignment"));

            let name = if self.top_level {
                "VerticalAlignment"
            } else {
                "StaffGrouper"
            };
            let id = tx.make_defined_grob(name, Cause::None)?;
            let valign = tx.grob_handle(id);
            self.valign = Some(valign);
            let col = command_column(tx);
            set_bound(tx, valign, -1, col)?;
        }
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(valign) = self.valign.take() {
            let col = command_column(tx);
            set_bound(tx, valign, 1, col)?;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        if which == 0 {
            self.acknowledge_hara_kiri_group_spanner(tx, grob)
        } else {
            self.acknowledge_outside_staff(tx, grob)
        }
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.valign);
        out.extend(self.id_to_group.iter().flatten().map(|&(_, g)| g));
    }
}

impl VerticalAlignEngraver {
    fn acknowledge_hara_kiri_group_spanner(&mut self, tx: &mut Tx<'_>, grob: u32) -> Run<()> {
        if self.id_to_group.is_none() {
            return Ok(());
        }
        let Some(valign) = self.valign else {
            return Ok(());
        };
        let g = tx.grob_handle(grob);

        if self.top_level {
            let origin_ctx = tx.t.arena.borrow().records.grobs[grob as usize].origin.0;
            let id = String::from(tx.t.arena.borrow().id(origin_ctx));

            let table = self.id_to_group.as_mut().expect("checked");
            match table.iter_mut().find(|(k, _)| *k == id) {
                Some(entry) => entry.1 = g,
                None => table.push((id, g)),
            }

            let (above, below) = (
                tx.heap().intern("alignAboveContext"),
                tx.heap().intern("alignBelowContext"),
            );
            let before_id = tx.t.arena.borrow().get(origin_ctx, above);
            let after_id = tx.t.arena.borrow().get(origin_ctx, below);

            align::add_element(tx.host, valign, g)?;

            if before_id == Value::NULL && after_id == Value::NULL {
                return Ok(());
            }

            let before = self.hash_ref(tx, before_id);
            let after = self.hash_ref(tx, after_id);

            if before.is_none() && after.is_none() {
                let heap = tx.heap();
                let text = match string(heap, before_id) {
                    Some(s) => alloc::format!("alignAboveContext not found: {s}"),
                    None => alloc::format!(
                        "alignBelowContext not found: {}",
                        string(heap, after_id).unwrap_or_default()
                    ),
                };
                return tx.warning(&text);
            }

            let elements = tx.heap().intern("elements");
            let ga = grob_array::object(tx.host, valign, elements)?;
            let Some(arr) = tx.heap().foreign_mut::<GrobArray>(ga) else {
                return Ok(());
            };
            let arr = &mut arr.grobs;

            let Some(added) = arr.pop() else {
                return Ok(());
            };
            let mut affinity = None;
            for i in 0..arr.len() {
                if Some(arr[i]) == before {
                    arr.insert(i, added);

                    // Only set staff affinity if it already has one.  That way we won't
                    // set staff-affinity on things that don't want it (like staves).
                    affinity = Some(-1);
                    break;
                } else if Some(arr[i]) == after {
                    arr.insert(i + 1, added);
                    affinity = Some(1);
                    break;
                }
            }
            if let Some(d) = affinity {
                let v = grob::property(tx.host, added, "staff-affinity")?;
                if tx.heap().is_number(v) {
                    let d = tx.heap().integer(d);
                    grob::set_property(tx.heap(), added, "staff-affinity", d);
                }
            }
        } else {
            let elements = tx.heap().intern("elements");
            grob_array::add_grob(tx.host, valign, elements, g)?;
            let sym = tx.heap().intern("staff-grouper");
            let grouper = grob_array::object(tx.host, g, sym)?;
            if grob_of(tx.heap(), grouper).is_none() {
                grob::set_object(tx.heap(), g, sym, valign);
            }
        }
        Ok(())
    }

    fn acknowledge_outside_staff(&mut self, tx: &mut Tx<'_>, grob: u32) -> Run<()> {
        if !self.top_level {
            // valign_ is a staff grouper
            if let Some(valign) = self.valign {
                // Claim outside-staff grobs created by engravers in this immediate
                // context.
                let origin_ctx = tx.t.arena.borrow().records.grobs[grob as usize].origin.0;
                if origin_ctx == tx.context {
                    let g = tx.grob_handle(grob);
                    grob::set_parent(tx.heap(), g, valign, Y);
                }
            } else {
                tx.programming_error(
                    "cannot claim outside-staff grob before creating staff grouper",
                )?;
            }
        }
        Ok(())
    }

    /// `scm_hash_ref (id_to_group_hashtab_, key, SCM_BOOL_F)`: the last
    /// group of the context id `key`, none for a key that is no string.
    fn hash_ref(&self, tx: &mut Tx<'_>, key: Value) -> Option<Value> {
        let key = string(tx.heap(), key)?;
        self.id_to_group
            .iter()
            .flatten()
            .find(|(k, _)| *k == key)
            .map(|&(_, g)| g)
    }
}

/// The text of the string `v`, none for one that is no string.
fn string(heap: &Heap, v: Value) -> Option<String> {
    heap.str(v).map(|s| String::from(s.as_str()))
}
