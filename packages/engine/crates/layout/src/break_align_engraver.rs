//! `break-align-engraver.cc` (MS3 step 21): `Break_align_engraver`, a
//! class of layout's ([`BREAK_ALIGN_ENGRAVER`]) that the core passes at its
//! `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - Each break-aligned item acknowledged (`:91–131`) that has no X parent,
//!   is non-musical (`Item::is_non_musical`) and whose
//!   `break-align-symbol` is a symbol: the timestep's `BreakAlignment` made
//!   if there is none (`create_alignment`, `:134–138`), with no cause; the
//!   item added to the `BreakAlignGroup` of its symbol (`add_to_group`,
//!   `:141–163`), which, made for the first item of the symbol with it as
//!   cause, has the symbol, the alignment as Y parent and is added to it
//!   (`Break_alignment_interface::add_element`, `break_alignment.rs`); then
//!   the item added to the group (`Axis_group_interface::add_element`).
//! - Before the timestep's first such item joins its group (`:119–128`), if
//!   `createSpacing` is `#t` (nothing else counts) in the context of the
//!   engraver that made it: a `LeftEdge` made as that engraver makes one
//!   (`make_defined_grob_from`), with no cause, kept as the timestep's left
//!   edge and added to the group of its own `break-align-symbol`. Announced
//!   up from that context, it is acknowledged here too, and skipped: by then
//!   it has its group as X parent.
//! - Each break-alignable item acknowledged (`:70–87`) that has no X parent
//!   and is non-musical: the alignment, made if there is none, its X
//!   parent.
//! - `stop_translation_timestep` (`:50–56`): the groups, the alignment and
//!   the left edge forgotten.
//!
//! An acknowledged grob that is no item, where LilyPond's trampoline gives
//! its type error, is taken as one.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, STOP, TranslatorClass, Tx, Work};
use lily_engine_scheme::{Exception, Tag, Value};

use crate::axis_group;
use crate::break_alignment;
use crate::geometry::{X, Y};
use crate::grob;
use crate::item;

type Run<T> = Result<T, Exception>;

pub static BREAK_ALIGN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Break_align_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    always: false,
    hears: &[],
    acknowledges: &[
        ("break-aligned-interface", false),
        ("break-alignable-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(BreakAlignEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `align_` by its handle, `column_alist_` (each symbol's group, in the
/// order made) and `left_edge_`, the timestep's `LeftEdge` by its handle.
#[derive(Clone, Default)]
struct BreakAlignEngraver {
    align: Option<Value>,
    column_alist: Vec<(Value, Value)>,
    left_edge: Option<Value>,
}

impl Work for BreakAlignEngraver {
    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.column_alist.clear();

        self.align = None;
        self.left_edge = None;
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        if which == 0 {
            self.acknowledge_break_aligned(tx, grob)
        } else {
            self.acknowledge_break_alignable(tx, grob)
        }
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.align);
        out.extend(self.column_alist.iter().flat_map(|&(s, g)| [s, g]));
        out.extend(self.left_edge);
    }
}

impl BreakAlignEngraver {
    fn acknowledge_break_alignable(&mut self, tx: &mut Tx<'_>, grob: u32) -> Run<()> {
        let item = tx.grob_handle(grob);

        if grob::get_parent(tx.heap(), item, X).is_some() {
            return Ok(());
        }

        // Handling musical items is more involved because they might need to be
        // aligned with notation (note heads, etc.).  We currently leave that to
        // other engravers, but maybe it could be done here.
        if !item::is_non_musical(tx.host, item)? {
            return Ok(());
        }

        let align = self.create_alignment(tx)?;
        grob::set_parent(tx.heap(), item, align, X);
        Ok(())
    }

    // Clef, BarLine, etc. are break-aligned grobs
    fn acknowledge_break_aligned(&mut self, tx: &mut Tx<'_>, grob: u32) -> Run<()> {
        let item = tx.grob_handle(grob);

        /*
          Removed check for item->empty (X_AXIS). --hwn 20/1/04
        */
        if grob::get_parent(tx.heap(), item, X).is_some() {
            return Ok(());
        }

        if !item::is_non_musical(tx.host, item)? {
            return Ok(());
        }

        let align_name = grob::property(tx.host, item, "break-align-symbol")?;
        if align_name.tag() != Tag::Symbol {
            return Ok(());
        }

        self.create_alignment(tx)?;

        // Create a single LeftEdge that appears to come from the same engraver
        // as the first staff-resident, break-aligned grob that we see.  This is
        // questionable and may contribute to problems discussed in issue #5385.
        // Practically, this is probably fine for single-staff scores.
        //
        // Break-aligned grobs can originate outside of a Staff context, but we
        // don't want to create a LeftEdge then (issue #6134).  The createSpacing
        // property tells us whether the grob originated within a Staff or
        // similar context.
        if self.left_edge.is_none() {
            let eng = tx.t.arena.borrow().records.grobs[grob as usize].origin;
            let sym = tx.heap().intern("createSpacing");
            if tx.t.arena.borrow().get(eng.0, sym) == Value::TRUE {
                let id = tx.make_defined_grob_from(eng, "LeftEdge", Cause::None)?;
                let left_edge = tx.grob_handle(id);
                self.left_edge = Some(left_edge);
                let name = grob::property(tx.host, left_edge, "break-align-symbol")?;
                self.add_to_group(tx, name, id)?;
            }
        }

        self.add_to_group(tx, align_name, grob)
    }

    /// `create_alignment`, giving the alignment.
    fn create_alignment(&mut self, tx: &mut Tx<'_>) -> Run<Value> {
        if let Some(align) = self.align {
            return Ok(align);
        }
        let id = tx.make_defined_grob("BreakAlignment", Cause::None)?;
        let align = tx.grob_handle(id);
        self.align = Some(align);
        Ok(align)
    }

    fn add_to_group(&mut self, tx: &mut Tx<'_>, align_name: Value, grob: u32) -> Run<()> {
        let item = tx.grob_handle(grob);
        let align = self.align.expect("made by create_alignment");

        let group = match self.column_alist.iter().find(|&&(s, _)| s == align_name) {
            Some(&(_, group)) => group,
            None => {
                let id = tx.make_defined_grob("BreakAlignGroup", Cause::Grob(grob))?;
                let group = tx.grob_handle(id);

                grob::set_property(tx.heap(), group, "break-align-symbol", align_name);
                grob::set_parent(tx.heap(), group, align, Y);

                self.column_alist.push((align_name, group));

                break_alignment::add_element(tx.host, align, group)?;
                group
            }
        };
        axis_group::add_element(tx.host, group, item)
    }
}
