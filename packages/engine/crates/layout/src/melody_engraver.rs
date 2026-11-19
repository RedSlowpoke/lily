//! `melody-engraver.cc` and `melody-spanner.cc` (MS8 step 8, part 1, 129
//! and 100 lines): `Melody_engraver`, a class of layout's
//! ([`MELODY_ENGRAVER`]) that shadows the music crate's placeholder of its
//! name (D67, D108), and `ly:melody-spanner::calc-neutral-stem-direction`,
//! Rust under a primitive that prints as LilyPond's subr and calls it over
//! `%host-call` (D127, [`HOSTS`]).
//!
//! - A stem acknowledged (`:73–92`) while `suspendMelodyDecisions` is
//!   `#f`: without `rests`, the timestep's stem, and a `MelodyItem` caused
//!   by it made for the next melody where none waits; with rests, the
//!   melody breaks. A slur acknowledged breaks it too (`:66–71`).
//! - `stop_translation_timestep` (`:46–64`), with a stem: the melody breaks
//!   at a `currentBarLine` too; broken, the waiting `MelodyItem` becomes
//!   the melody's; the stem is added to it (`Melody_spanner::add_stem`:
//!   one of its `stems`, its `melody-spanner`, its `neutral-direction` the
//!   callback).
//! - The callback (`melody-spanner.cc:34–98`): a stem's neutral direction
//!   from its melody's stems' `default-direction`s, each run of neutral
//!   stems taking the direction of the stems on either side where they
//!   agree or only one has one, else the melody's `neutral-direction`; the
//!   other stems of the run are given theirs. `DOWN` without a live melody.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{Cause, Host, Kind, STOP, TranslatorClass, Tx, Work, grob_of};
use lily_engine_scheme::{Cx, Exception, HostHandler, Value, Vm};

use crate::callbacks::Callback;
use crate::dynamic_align_engraver::direction;
use crate::geometry::Direction;
use crate::{grob, grob_array};

type Run<T> = Result<T, Exception>;

/// `VPOS`.
const VPOS: usize = usize::MAX;

pub static MELODY_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Melody_engraver",
    kind: Kind::Engraver,
    methods: STOP,
    always: false,
    hears: &[],
    acknowledges: &[("stem-interface", false), ("slur-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(MelodyEngraver::default())),
    reads: &["currentBarLine", "suspendMelodyDecisions"],
    writes: &[],
};

/// The acknowledgers' indices in [`MELODY_ENGRAVER`].
const STEM: usize = 0;

/// `melody_item_`, `next_melody_item_` and `stem_` by their handles, and
/// `break_melody_`.
#[derive(Clone)]
struct MelodyEngraver {
    melody_item: Option<Value>,
    next_melody_item: Option<Value>,
    stem: Option<Value>,
    break_melody: bool,
}

impl Default for MelodyEngraver {
    fn default() -> MelodyEngraver {
        MelodyEngraver {
            melody_item: None,
            next_melody_item: None,
            stem: None,
            break_melody: true,
        }
    }
}

/// `Melody_spanner::add_stem`.
fn add_stem(tx: &mut Tx<'_>, me: Option<Value>, stem: Value) -> Run<()> {
    // LilyPond's reads through a null melody item; none is made only when
    // a stem is acknowledged without one, which makes one.
    let Some(me) = me else {
        return Ok(());
    };
    let sym = tx.heap().intern("stems");
    grob_array::add_grob(tx.host, me, sym, stem)?;
    let sym = tx.heap().intern("melody-spanner");
    grob::set_object(tx.heap(), stem, sym, me);
    let proc = crate::lily_procedure(tx.host, "ly:melody-spanner::calc-neutral-stem-direction")?;
    grob::set_property(tx.heap(), stem, "neutral-direction", proc);
    Ok(())
}

impl Work for MelodyEngraver {
    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        if which != STEM {
            // `acknowledge_slur`.
            self.break_melody = true;
            return Ok(());
        }
        let stem = tx.grob_handle(grob);
        if tx.get("suspendMelodyDecisions") == Value::FALSE {
            let rests = grob_array::extract_grob_array(tx.host, stem, "rests")?;
            if rests.is_empty() {
                self.stem = Some(stem);

                // We don't necessarily know yet whether we will need to
                // place this stem in a new melody span. Create a next
                // MelodyItem now because creating grobs in
                // stop_translation_timestep () isn't allowed.
                if self.next_melody_item.is_none() {
                    let id = tx.make_defined_grob("MelodyItem", Cause::Grob(grob))?;
                    self.next_melody_item = Some(tx.grob_handle(id));
                }
            } else {
                self.break_melody = true;
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(stem) = self.stem.take() {
            // If we don't already know a reason to start a new melody span,
            // check whether there is a bar line. We can't use
            // acknowledge_bar_line () for this because the Bar_engraver
            // operates in Staff context, so this engraver can't observe its
            // grobs.
            if !self.break_melody {
                let bar = tx.get("currentBarLine");
                self.break_melody = grob_of(tx.heap(), bar).is_some();
            }

            if self.break_melody {
                self.break_melody = false;
                self.melody_item = self.next_melody_item.take();
            }

            add_stem(tx, self.melody_item, stem)?;
        }
        Ok(())
    }

    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[self.break_melody]);
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.melody_item);
        out.extend(self.next_melody_item);
        out.extend(self.stem);
    }
}

// --- the callback -------------------------------------------------------------

/// `calc_neutral_stem_direction`, as above.
pub fn calc_neutral_stem_direction(host: &mut dyn Host, stem: Value) -> Result<Value, Exception> {
    let sym = host.heap().intern("melody-spanner");
    let me = grob::object(host, stem, sym)?;
    if !grob::is_live_grob(host.heap(), me) {
        return Ok(host.heap().integer(-1));
    }

    let stems = grob_array::extract_grob_array(host, me, "stems")?;
    let mut dirs: Vec<Direction> = Vec::with_capacity(stems.len());
    for &s in &stems {
        dirs.push(direction(grob::property(host, s, "default-direction")?));
    }

    // `VPOS` is `usize::MAX`, and LilyPond's arithmetic on it wraps.
    let n = dirs.len();
    if n == 0 {
        // The stem is among its melody's stems; LilyPond's loop would not
        // end without one.
        return Ok(Value::NULL);
    }
    let mut last_nonneutral = VPOS;
    let mut next_nonneutral = 0;
    while next_nonneutral < n && dirs[next_nonneutral] == 0 {
        next_nonneutral += 1;
    }

    let mut retval = Value::NULL;
    while last_nonneutral == VPOS || last_nonneutral + 1 < n {
        let d1 = if last_nonneutral == VPOS {
            0
        } else {
            dirs[last_nonneutral]
        };
        let d2 = if next_nonneutral < n {
            dirs[next_nonneutral]
        } else {
            0
        };

        let total = if (d1 != 0 && d1 == d2) || (d1 != 0 && d2 == 0) {
            d1
        } else if d2 != 0 && d1 == 0 {
            d2
        } else {
            direction(grob::property(host, me, "neutral-direction")?)
        };

        for &s in &stems[last_nonneutral.wrapping_add(1)..next_nonneutral] {
            let total = host.heap().integer(i64::from(total));
            if s == stem {
                retval = total;
            } else {
                grob::set_property(host.heap(), s, "neutral-direction", total);
            }
        }

        last_nonneutral = next_nonneutral;
        while last_nonneutral < n && dirs[last_nonneutral] != 0 {
            last_nonneutral += 1;
        }
        next_nonneutral = last_nonneutral;
        last_nonneutral = last_nonneutral.wrapping_sub(1);

        while next_nonneutral < n && dirs[next_nonneutral] == 0 {
            next_nonneutral += 1;
        }
    }

    Ok(retval)
}

/// What `%host-call` runs for the `ly:melody-spanner::` callback (D127).
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "melody-spanner",
    f: host_melody_spanner,
}];

/// The `ly:melody-spanner::` callback, by name.
pub(crate) const CALLBACKS: [(&str, Callback); 1] =
    [("calc-neutral-stem-direction", calc_neutral_stem_direction)];

fn host_melody_spanner(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    crate::callbacks::dispatch(vm, &CALLBACKS, args, "no such melody-spanner callback")
}

type R = Result<Value, lily_engine_scheme::Thrown>;

pub(crate) fn ly_calc_neutral_stem_direction(cx: &mut Cx, args: &[Value]) -> R {
    crate::tie::host_call_of(
        cx,
        "melody-spanner",
        "ly:melody-spanner::",
        "calc-neutral-stem-direction",
        args,
    )
}
