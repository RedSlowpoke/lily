//! `dynamic-align-engraver.cc` (MS3 step 19): `Dynamic_align_engraver`, a
//! class of layout's ([`DYNAMIC_ALIGN_ENGRAVER`]) that the core passes at
//! its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - Each dynamic acknowledged (`:119–160`) is an element of the line
//!   spanner, a `DynamicLineSpanner` caused by the first; an explicit
//!   direction of its event that is not the line's ends the line and
//!   starts another, and becomes the line's. A spanner is one started, an
//!   item a script. The rhythmic heads and stems acknowledged
//!   (`:107–117`) are its support; a footnote spanner whose Y parent is a
//!   dynamic (`:98–105`), its element.
//! - A dynamic spanner's end acknowledged (`:73–96`) is one ended; the
//!   running spanner, `spanner-broken` set, ends the line.
//! - `stop_translation_timestep` (`:194–235`): the started spanners run
//!   until they end, LilyPond's programming error for an end of one not
//!   running. The line ended and the current line take their bounds
//!   (`:162–192`), the current one its right bound only when no spanner
//!   runs, which ends it: the timestep's first script, else its first
//!   started (left) or ended (right) spanner's bound, else
//!   `currentMusicalColumn` with LilyPond's programming error. The line
//!   takes the timestep's support unless the running spanner is broken.
//!
//! The end of a spanner not running, which LilyPond reports at
//! `started_[i]` past the started ones' end, is reported unlocated there.
//! Without a musical column, where LilyPond's would set a null bound, none
//! is set (`ledger_line_engraver::set_bound`).

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Kind, STOP, TranslatorClass, Tx, Work, event_property, grob_of};
use lily_engine_scheme::{Exception, Value};

use crate::axis_group;
use crate::geometry::{Direction, Y};
use crate::grob;
use crate::item;
use crate::ledger_line_engraver::set_bound;
use crate::side_position;
use crate::spanner;
use crate::tie_engraver::event_cause;

type Run<T> = Result<T, Exception>;

pub static DYNAMIC_ALIGN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Dynamic_align_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    always: false,
    hears: &[],
    acknowledges: &[
        ("dynamic-interface", false),
        ("rhythmic-head-interface", false),
        ("stem-interface", false),
        ("footnote-spanner-interface", false),
        ("dynamic-interface", true),
    ],
    must_be_last: false,
    make: Some(|| Box::new(DynamicAlignEngraver::default())),
    reads: &["currentMusicalColumn"],
    writes: &[],
};

/// The acknowledgers' indices in [`DYNAMIC_ALIGN_ENGRAVER`].
const DYNAMIC: usize = 0;
const FOOTNOTE_SPANNER: usize = 3;
const END_DYNAMIC: usize = 4;

/// `line_`, `ended_line_`, `current_dynamic_spanner_`, `ended_`,
/// `started_`, `scripts_`, `support_` and `running_`, by their handles.
#[derive(Clone, Default)]
struct DynamicAlignEngraver {
    line: Option<Value>,
    ended_line: Option<Value>,
    current_dynamic_spanner: Option<Value>,
    ended: Vec<Value>,
    started: Vec<Value>,
    scripts: Vec<Value>,
    support: Vec<Value>,
    running: Vec<Value>,
}

/// `from_scm<Direction>`: an exact -1, 0 or 1, else 0 (`CENTER`).
pub(crate) fn direction(v: Value) -> Direction {
    match v.as_fixnum() {
        Some(d @ -1..=1) => d as Direction,
        _ => 0,
    }
}

/// `from_scm<bool> (get_property (sp, "spanner-broken"))`: `#t` only.
fn is_broken(tx: &mut Tx<'_>, sp: Value) -> Run<bool> {
    Ok(grob::property(tx.host, sp, "spanner-broken")? == Value::TRUE)
}

impl DynamicAlignEngraver {
    fn create_line_spanner(&mut self, tx: &mut Tx<'_>, cause: u32) -> Run<Value> {
        if let Some(line) = self.line {
            return Ok(line);
        }
        let id = tx.make_defined_grob("DynamicLineSpanner", Cause::Grob(cause))?;
        let line = tx.grob_handle(id);
        self.line = Some(line);
        Ok(line)
    }

    fn acknowledge_end_dynamic(&mut self, tx: &mut Tx<'_>, sp: Value) -> Run<()> {
        self.ended.push(sp);

        if self.line.is_none() {
            return Ok(());
        }

        /* If the break flag is set, store the current spanner and let new dynamics
         * create a new spanner
         */
        let spanner_broken = self.current_dynamic_spanner == Some(sp) && is_broken(tx, sp)?;
        if spanner_broken {
            if self.ended_line.is_some() {
                tx.programming_error("already have a force-ended DynamicLineSpanner.")?;
            }
            self.ended_line = self.line.take();
            self.current_dynamic_spanner = None;
        }
        Ok(())
    }

    fn acknowledge_dynamic(&mut self, tx: &mut Tx<'_>, grob: u32, g: Value) -> Run<()> {
        let cause = event_cause(tx, g)?;
        // Check whether an existing line spanner has the same direction
        if let (Some(line), Some(cause)) = (self.line, cause) {
            let line_dir = direction(grob::property(tx.host, line, "direction")?);
            let grob_dir = direction(event_property(tx.heap(), cause, "direction"));

            // If we have an explicit direction for the new dynamic grob
            // that differs from the current line spanner, break the spanner
            if grob_dir != 0 && line_dir != grob_dir {
                if self.ended_line.is_none() {
                    self.ended_line = Some(line);
                }
                self.line = None;
                self.current_dynamic_spanner = None;
            }
        }

        let line = self.create_line_spanner(tx, grob)?;
        if spanner::is_spanner(tx.heap(), g) {
            self.started.push(g);
            self.current_dynamic_spanner = Some(g);
        } else if item::is_item(tx.heap(), g) {
            self.scripts.push(g);
        } else {
            grob::programming_error(tx.host, g, "unknown dynamic grob")?;
        }

        axis_group::add_element(tx.host, line, g)?;

        if let Some(cause) = cause {
            let d = event_property(tx.heap(), cause, "direction");
            if direction(d) != 0 {
                grob::set_property(tx.heap(), line, "direction", d);
            }
        }
        Ok(())
    }

    fn set_spanner_bounds(&self, tx: &mut Tx<'_>, line: Option<Value>, end: bool) -> Run<()> {
        let Some(line) = line else {
            return Ok(());
        };

        for d in [-1, 1] {
            if (d == -1 && spanner::get_bound(tx.host, line, -1)?.is_none())
                || (end && d == 1 && spanner::get_bound(tx.host, line, 1)?.is_none())
            {
                let spanners = if d == -1 { &self.started } else { &self.ended };

                let bound = if let Some(&script) = self.scripts.first() {
                    Some(script)
                } else if let Some(&sp) = spanners.first() {
                    spanner::get_bound(tx.host, sp, d)?
                } else {
                    let col = tx.get("currentMusicalColumn");
                    let col = grob_of(tx.heap(), col).map(|_| col);
                    tx.programming_error("started DynamicLineSpanner but have no left bound")?;
                    col
                };

                set_bound(tx, line, d, bound)?;
            }
        }
        Ok(())
    }
}

impl Work for DynamicAlignEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        match which {
            DYNAMIC => self.acknowledge_dynamic(tx, grob, g),
            FOOTNOTE_SPANNER => {
                if let Some(line) = self.line
                    && let Some(parent) = grob::get_parent(tx.heap(), g, Y)
                    && grob::has_interface(tx.heap(), parent, "dynamic-interface")
                {
                    axis_group::add_element(tx.host, line, g)?;
                }
                Ok(())
            }
            END_DYNAMIC => self.acknowledge_end_dynamic(tx, g),
            // `acknowledge_rhythmic_head`, `acknowledge_stem`.
            _ => {
                self.support.push(g);
                Ok(())
            }
        }
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        for &sp in &self.started {
            if !self.running.contains(&sp) {
                self.running.push(sp);
            }
        }
        for (i, &sp) in self.ended.iter().enumerate() {
            if let Some(at) = self.running.iter().position(|&r| r == sp) {
                self.running.swap_remove(at);
            } else if let Some(&started) = self.started.get(i) {
                grob::programming_error(tx.host, started, "lost track of this dynamic spanner")?;
            } else {
                tx.programming_error("lost track of this dynamic spanner")?;
            }
        }

        let end = self.line.is_some() && self.running.is_empty();

        // Set the proper bounds for the current spanner and for a spanner that
        // is ended now
        self.set_spanner_bounds(tx, self.ended_line, true)?;
        self.set_spanner_bounds(tx, self.line, end)?;
        // If the flag is set to break the spanner after the current child, don't
        // add any more support points (needed e.g. for style=none, where the
        // invisible spanner should NOT be shifted since we don't have a line).
        let spanner_broken = match self.current_dynamic_spanner {
            Some(sp) => is_broken(tx, sp)?,
            None => false,
        };
        if let Some(line) = self.line
            && !spanner_broken
        {
            for &s in &self.support {
                side_position::add_support(tx.host, line, s)?;
            }
        }

        if end {
            self.line = None;
        }

        self.ended_line = None;
        self.ended.clear();
        self.started.clear();
        self.scripts.clear();
        self.support.clear();
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.line);
        out.extend(self.ended_line);
        out.extend(self.current_dynamic_spanner);
        out.extend(&self.ended);
        out.extend(&self.started);
        out.extend(&self.scripts);
        out.extend(&self.support);
        out.extend(&self.running);
    }
}
