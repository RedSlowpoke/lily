//! `axis-group-engraver.cc` (MS3 step 21): `Axis_group_engraver`, a class
//! of layout's ([`AXIS_GROUP_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108). It is last in its group
//! (`must_be_last`, `:76–80`).
//!
//! - `initialize` (`:62–68`): active if `hasAxisGroup`, as seen from the
//!   context, is not `#t`, and then sets it to `#t` in its context, so an
//!   engraver of a context below it is not.
//! - `process_music` (`:82–92`): an active one's first makes its
//!   `VerticalAxisGroup` (`get_spanner`, `:94–98`), with no cause and
//!   `currentCommandColumn` its left bound; at each timestep
//!   `keepAliveInterfaces` read. As it must run at the first timestep, the
//!   class runs always.
//! - Each grob acknowledged (`:113–133`), once there is a group, is kept
//!   as an element; with the group's `remove-empty` `#t`, one of
//!   `keepAliveInterfaces` is its `items-worth-living`
//!   (`Hara_kiri_group_spanner::add_interesting_item`).
//! - `process_acknowledged` (`:139–164`) adds the kept grobs that have no
//!   `axis-group-parent-Y` to the group (`Axis_group_interface::add_element`,
//!   `:166–170`); one that is the group's own Y parent is LilyPond's three
//!   warnings, and the group's suicide.
//! - `finalize` (`:100–111`): `currentCommandColumn` the group's right
//!   bound, and its `elements` unordered.
//!
//! Without a command column, where LilyPond's would set a null bound, none
//! is set (`ledger_line_engraver::set_bound`).

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, INITIALIZE, Kind, PROCESS, PROCESS_ACKNOWLEDGED, TranslatorClass, Tx, Work,
    grob_of,
};
use lily_engine_scheme::{Exception, Value};

use crate::axis_group;
use crate::clef_engraver::is_true;
use crate::geometry::Y;
use crate::grob;
use crate::grob_array;
use crate::hara_kiri_group_spanner;
use crate::ledger_line_engraver::{command_column, set_bound};

type Run<T> = Result<T, Exception>;

pub static AXIS_GROUP_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Axis_group_engraver",
    kind: Kind::Engraver,
    methods: INITIALIZE | PROCESS | PROCESS_ACKNOWLEDGED | FINALIZE,
    always: true,
    hears: &[],
    acknowledges: &[("grob-interface", false)],
    must_be_last: true,
    make: Some(|| Box::new(AxisGroupEngraver::default())),
    reads: &[
        "currentCommandColumn",
        "keepAliveInterfaces",
        "hasAxisGroup",
    ],
    writes: &["hasAxisGroup"],
};

/// `active_`, `staffline_` by its handle, `interesting_` and `elts_`.
#[derive(Clone)]
struct AxisGroupEngraver {
    active: bool,
    staffline: Option<Value>,
    interesting: Value,
    elts: Vec<Value>,
}

impl Default for AxisGroupEngraver {
    fn default() -> Self {
        AxisGroupEngraver {
            active: false,
            staffline: None,
            interesting: Value::NULL,
            elts: Vec::new(),
        }
    }
}

impl Work for AxisGroupEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[self.active]);
    }

    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.active = !is_true(tx.get("hasAxisGroup"));
        if self.active {
            tx.set("hasAxisGroup", Value::TRUE);
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.staffline.is_none() && self.active {
            let id = tx.make_defined_grob("VerticalAxisGroup", Cause::None)?;
            let staffline = tx.grob_handle(id);
            self.staffline = Some(staffline);
            let col = command_column(tx);
            set_bound(tx, staffline, -1, col)?;
        }
        self.interesting = tx.get("keepAliveInterfaces");
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(staffline) = self.staffline {
            let col = command_column(tx);
            set_bound(tx, staffline, 1, col)?;

            let elements = tx.heap().intern("elements");
            grob_array::set_ordered(tx.host, staffline, elements, false)?;
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        let Some(staffline) = self.staffline else {
            return Ok(());
        };

        let g = tx.grob_handle(grob);
        self.elts.push(g);

        if is_true(grob::property(tx.host, staffline, "remove-empty")?) {
            let mut s = self.interesting;
            while let (Some(iface), Some(rest)) = (tx.heap().car(s), tx.heap().cdr(s)) {
                s = rest;
                let name = tx.heap().name(iface).map(String::from);
                if name.is_some_and(|name| grob::has_interface(tx.heap(), g, &name)) {
                    hara_kiri_group_spanner::add_interesting_item(tx.host, staffline, g)?;
                    break;
                }
            }
        }
        Ok(())
    }

    // maybe should check if our parent is set, because we now get a cyclic
    // parent relationship if we have two Axis_group_engravers in the
    // context.
    fn process_acknowledged(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(staffline) = self.staffline else {
            return Ok(());
        };

        let parent_y = tx.heap().intern("axis-group-parent-Y");
        for &e in &self.elts {
            let p = grob::object_data(tx.heap(), e, parent_y);
            if grob_of(tx.heap(), p).is_none() {
                if grob::get_parent(tx.heap(), staffline, Y) == Some(e) {
                    let host = &mut *tx.host;
                    grob::warning(
                        host,
                        staffline,
                        "Axis_group_engraver: vertical group already has a parent",
                    )?;
                    grob::warning(host, staffline, "are there two Axis_group_engravers?")?;
                    grob::warning(host, staffline, "removing this vertical group")?;
                    grob::suicide(tx.heap(), staffline);
                    self.staffline = None;
                    break;
                }
                axis_group::add_element(tx.host, staffline, e)?;
            }
        }
        self.elts.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.staffline);
        out.push(self.interesting);
        out.extend(self.elts.iter().copied());
    }
}
