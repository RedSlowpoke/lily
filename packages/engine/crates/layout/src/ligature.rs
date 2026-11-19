//! `ligature-engraver.cc`, `coherent-ligature-engraver.cc` and
//! `ligature-bracket-engraver.cc` (MS8 step 9, part 1; 214, 184 and 134
//! lines): the ligatures' base, and `Ligature_bracket_engraver`, a class of
//! layout's ([`LIGATURE_BRACKET_ENGRAVER`]) that shadows the music crate's
//! placeholder of its name (D67, D108).
//!
//! - **`Ligature_engraver`** ([`LigatureEngraver`]), abstract in LilyPond:
//!   a concrete engraver is it over a [`Coherent`] (each of LilyPond's is a
//!   `Coherent_ligature_engraver`), which names its spanner and its
//!   primitives' stencil and builds the ligature.
//!   - `pre_process_music` (`:93–98`): while a ligature runs and none ends,
//!     the Score's `forbidBreak` set.
//!   - `process_music` (`:100–144`): a `ligature-event` stop (heard as a
//!     `Unique_span_event_listener` hears it) ends the running ligature, its
//!     right bound the column of the last `process_music`
//!     (`last_bound_`), its heads its finished primitives; a start makes the
//!     subclass's spanner, its left bound `currentMusicalColumn`; LilyPond's
//!     warnings where there is none, or one already, or no bound.
//!   - `stop_translation_timestep` (`:146–166`) and `finalize`
//!     (`:168–181`): a finished ligature typeset (an empty one junked with
//!     LilyPond's programming error); an unterminated one killed with its
//!     warning.
//!   - Each ligature head acknowledged while one runs (`:189–197`) is one of
//!     its primitives, its `stencil` the subclass's primitive procedure; a
//!     rest warns (`:199–210`).
//! - **`Coherent_ligature_engraver`** (`typeset_ligature`, `:161–171`): each
//!   primitive's `delta-position` to the next one's pitch
//!   ([`calc_delta_pitches`], `:138–159`), then the subclass's
//!   `build_ligature`; `collect_accidentals` is LilyPond's TODO, nothing.
//!   [`move_related_items_to_column`] (`:86–118`) moves an item's column's
//!   elements of its staff onto another column.
//! - **`Ligature_bracket_engraver`** (`:34–110`): a `LigatureBracket` from
//!   each start to each stop, each note column or rest acknowledged while
//!   it runs one of its `note-columns` and its bound.

use alloc::boxed::Box;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, Held, Kind, PRE_PROCESS, PROCESS, STOP, TranslatorClass, Tx, Work,
    assign_event_once, event_property,
};
use lily_engine_scheme::{Exception, Value};

use crate::dynamic_engraver::musical_column;
use crate::geometry::X;
use crate::note_heads_engraver::steps;
use crate::paper_column_engraver::find_score_context;
use crate::staff_symbol_referencer::staff_symbol;
use crate::tie_engraver::event_cause;
use crate::{grob, grob_array, item, spanner, tuplet_bracket};

type Run<T> = Result<T, Exception>;

/// A `span-direction`'s event kept as `Unique_span_event_listener` keeps
/// it: the first start and the first stop, a conflict warned.
pub(crate) fn listen_unique(
    tx: &mut Tx<'_>,
    start: &mut Option<Value>,
    stop: &mut Option<Value>,
    ev: Value,
) -> Run<()> {
    let d = event_property(tx.heap(), ev, "span-direction");
    match tx.heap().to_i64(d) {
        Some(-1) => assign_event_once(tx, start, ev).map(drop),
        Some(1) => assign_event_once(tx, stop, ev).map(drop),
        _ => tx.programming_error("event span-direction is not set"),
    }
}

// --- Ligature_bracket_engraver ------------------------------------------------

pub static LIGATURE_BRACKET_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Ligature_bracket_engraver",
    kind: Kind::Engraver,
    methods: PROCESS | STOP,
    always: false,
    hears: &["ligature-event"],
    acknowledges: &[("rest-interface", false), ("note-column-interface", false)],
    must_be_last: false,
    make: Some(|| Box::new(LigatureBracketEngraver::default())),
    reads: &[],
    writes: &[],
};

/// `ligature_listener_`'s start and stop, and `ligature_` by its handle
/// and record number.
#[derive(Clone, Default)]
struct LigatureBracketEngraver {
    start: Option<Value>,
    stop: Option<Value>,
    ligature: Option<(Value, u32)>,
}

impl Work for LigatureBracketEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        listen_unique(tx, &mut self.start, &mut self.stop, ev)
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(ender) = self.stop {
            if self.ligature.is_none() {
                return tx.event_warning(ender, "cannot find start of ligature");
            }
            self.ligature = None;
        }

        if let Some(starter) = self.start {
            if let Some((ligature, _)) = self.ligature {
                tx.event_warning(starter, "already have a ligature")?;
                return grob::warning(tx.host, ligature, "ligature was started here");
            }
            let id = tx.make_defined_grob("LigatureBracket", Cause::Event(starter))?;
            self.ligature = Some((tx.grob_handle(id), id));
        }
        Ok(())
    }

    /// `acknowledge_note_column`, and `acknowledge_rest` as it: the item
    /// one of the bracket's `note-columns` and its bound. A rest that is
    /// no item (a `MultiMeasureRest`), which LilyPond's TODO leaves open
    /// and its `dynamic_cast` would pass as null, is not taken.
    fn acknowledge(&mut self, tx: &mut Tx<'_>, _: usize, grob: u32) -> Run<()> {
        if let Some((ligature, _)) = self.ligature {
            let it = tx.grob_handle(grob);
            if !item::is_item(tx.heap(), it) {
                return Ok(());
            }
            tuplet_bracket::add_column(tx.host, ligature, it)?;
            spanner::add_bound_item(tx.host, ligature, it)?;
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, _: &mut Tx<'_>) -> Run<()> {
        self.start = None;
        self.stop = None;
        Ok(())
    }

    fn held(&self, h: &mut dyn Held) {
        h.maybe_record(self.ligature.map(|l| l.1));
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.start);
        out.extend(self.stop);
        out.extend(self.ligature.map(|l| l.0));
    }
}

// --- Ligature_engraver and Coherent_ligature_engraver ---------------------------

/// What a concrete `Coherent_ligature_engraver` gives
/// [`LigatureEngraver`]: `create_ligature_spanner`'s grob,
/// `brew_ligature_primitive_proc` and `build_ligature`.
pub(crate) trait Coherent: Clone + Default + 'static {
    /// The spanner `create_ligature_spanner` makes, of no cause.
    const SPANNER: &'static str;
    /// The procedure of `(lily)` each primitive's `stencil` is set to, if
    /// any (`brew_ligature_primitive_proc`).
    const PRIMITIVE: Option<&'static str>;

    /// `build_ligature`: the ligature made of its primitives.
    fn build_ligature(&mut self, tx: &mut Tx<'_>, ligature: Value, primitives: &[Value])
    -> Run<()>;

    /// An event of a class its engraver hears past `ligature-event`, the
    /// first of them `which` 1 (the Gregorian engraver's `pes-or-flexa`).
    fn listen(&mut self, _tx: &mut Tx<'_>, _which: usize, _ev: Value) -> Run<()> {
        Ok(())
    }

    /// Its own part of `stop_translation_timestep`, after
    /// `Ligature_engraver`'s.
    fn stop_translation_timestep(&mut self, _tx: &mut Tx<'_>) -> Run<()> {
        Ok(())
    }

    /// The values it keeps between calls, which a collection must keep.
    fn values(&self, _out: &mut Vec<Value>) {}
}

/// `Ligature_engraver`'s state over a [`Coherent`]: `ligature_listener_`'s
/// events, `ligature_`, `primitives_`, their finished copies and
/// `last_bound_`, each grob by its handle and record number.
/// (`ligature_start_mom_` is kept by LilyPond and read by none.)
#[derive(Clone, Default)]
pub(crate) struct LigatureEngraver<C: Coherent> {
    start: Option<Value>,
    stop: Option<Value>,
    ligature: Option<(Value, u32)>,
    primitives: Vec<(Value, u32)>,
    finished_ligature: Option<(Value, u32)>,
    finished_primitives: Vec<(Value, u32)>,
    last_bound: Option<Value>,
    coherent: C,
}

/// The methods a [`LigatureEngraver`]'s class has.
pub(crate) const LIGATURE_METHODS: u8 = PRE_PROCESS | PROCESS | STOP | FINALIZE;
/// Whether it runs at every timestep: `process_music` keeps every
/// timestep's `currentMusicalColumn` as `last_bound_`, a ligature's right
/// bound the column of the timestep before its stop, whether the voice had
/// anything there or not.
pub(crate) const LIGATURE_ALWAYS: bool = true;
/// What it acknowledges, as its subclasses' `boot` adds them: rests, then
/// ligature heads.
pub(crate) const LIGATURE_ACKNOWLEDGES: &[(&str, bool)] = &[
    ("rest-interface", false),
    ("ligature-head-interface", false),
];

impl<C: Coherent> LigatureEngraver<C> {
    /// `Coherent_ligature_engraver::typeset_ligature`.
    fn typeset_ligature(
        &mut self,
        tx: &mut Tx<'_>,
        ligature: Value,
        primitives: &[Value],
    ) -> Run<()> {
        // compute some commonly needed context info stored as grob
        // properties
        calc_delta_pitches(tx, primitives)?;

        // prepare ligature for typesetting
        self.coherent.build_ligature(tx, ligature, primitives)
        // collect_accidentals: LilyPond's TODO, nothing.
    }
}

impl<C: Coherent> Work for LigatureEngraver<C> {
    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        if which > 0 {
            return self.coherent.listen(tx, which, ev);
        }
        listen_unique(tx, &mut self.start, &mut self.stop, ev)
    }

    fn pre_process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.ligature.is_some() && self.stop.is_none() {
            let score = find_score_context(tx);
            let sym = tx.heap().intern("forbidBreak");
            tx.t.arena.borrow_mut().set(score, sym, Value::TRUE);
        }
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(ender) = self.stop {
            let Some((ligature, id)) = self.ligature.take() else {
                return tx.event_warning(ender, "cannot find start of ligature");
            };

            match self.last_bound {
                None => tx.event_warning(ender, "no right bound")?,
                Some(bound) => spanner::set_bound(tx.host, ligature, 1, bound)?,
            }

            self.finished_primitives = core::mem::take(&mut self.primitives);
            self.finished_ligature = Some((ligature, id));
        }
        self.last_bound = musical_column(tx);

        if let Some(starter) = self.start {
            if let Some((ligature, _)) = self.ligature {
                tx.event_warning(starter, "already have a ligature")?;
                return grob::warning(tx.host, ligature, "ligature was started here");
            }

            let id = tx.make_defined_grob(C::SPANNER, Cause::None)?;
            let ligature = tx.grob_handle(id);
            self.ligature = Some((ligature, id));

            match musical_column(tx) {
                None => tx.event_warning(starter, "no left bound")?,
                Some(bound) => spanner::set_bound(tx.host, ligature, -1, bound)?,
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((finished, _)) = self.finished_ligature {
            if self.finished_primitives.is_empty() {
                grob::programming_error(
                    tx.host,
                    finished,
                    "Ligature_engraver::stop_translation_timestep (): junking empty ligature",
                )?;
            } else {
                let primitives: Vec<Value> = self.finished_primitives.iter().map(|p| p.0).collect();
                self.typeset_ligature(tx, finished, &primitives)?;
                self.finished_primitives.clear();
            }
            self.finished_ligature = None;
        }

        self.start = None;
        self.stop = None;
        self.coherent.stop_translation_timestep(tx)
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some((finished, _)) = self.finished_ligature.take() {
            let primitives: Vec<Value> = self.finished_primitives.iter().map(|p| p.0).collect();
            self.typeset_ligature(tx, finished, &primitives)?;
            self.finished_primitives.clear();
        }
        if let Some((ligature, _)) = self.ligature {
            grob::warning(tx.host, ligature, "unterminated ligature")?;
            grob::suicide(tx.heap(), ligature);
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let Some((ligature, _)) = self.ligature else {
            return Ok(());
        };
        let g = tx.grob_handle(grob);
        if which == 0 {
            // acknowledge_rest
            match event_cause(tx, g)? {
                Some(ev) => tx.event_warning(ev, "ignoring rest: ligature may not contain rest")?,
                None => tx.warning("ignoring rest: ligature may not contain rest")?,
            }
            // TODO: maybe better should stop ligature here rather than
            // ignoring the rest?
            return grob::warning(tx.host, ligature, "ligature was started here");
        }
        // acknowledge_ligature_head, an item's (`Grob_info_t<Item>`)
        if !item::is_item(tx.heap(), g) {
            return Ok(());
        }
        self.primitives.push((g, grob));
        if let Some(name) = C::PRIMITIVE {
            let proc = crate::lily_procedure(tx.host, name)?;
            grob::set_property(tx.heap(), g, "stencil", proc);
        }
        Ok(())
    }

    fn held(&self, h: &mut dyn Held) {
        h.maybe_record(self.ligature.map(|l| l.1));
        h.maybe_record(self.finished_ligature.map(|l| l.1));
        let ids = |ps: &[(Value, u32)]| ps.iter().map(|p| p.1).collect::<Vec<_>>();
        h.records(&ids(&self.primitives));
        h.records(&ids(&self.finished_primitives));
        h.flags(&[self.last_bound.is_some()]);
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.start);
        out.extend(self.stop);
        out.extend(self.ligature.map(|l| l.0));
        out.extend(self.primitives.iter().map(|p| p.0));
        out.extend(self.finished_ligature.map(|l| l.0));
        out.extend(self.finished_primitives.iter().map(|p| p.0));
        out.extend(self.last_bound);
        self.coherent.values(out);
    }
}

/// The pitch of a primitive's event cause in steps, as `Pitch::steps`
/// counts them; 0 where it has none (LilyPond reads through a null).
pub(crate) fn cause_steps(tx: &mut Tx<'_>, primitive: Value) -> Run<i32> {
    let Some(ev) = event_cause(tx, primitive)? else {
        return Ok(0);
    };
    let heap = tx.heap();
    let p = event_property(heap, ev, "pitch");
    Ok(heap
        .foreign::<lily_engine_music::Pitch>(p)
        .copied()
        .map_or(0, |p| steps(heap, &p)))
}

/// `calc_delta_pitches` (`:138–159`): each primitive's `delta-position`
/// the steps from its pitch to the next one's, the last's 0.
pub(crate) fn calc_delta_pitches(tx: &mut Tx<'_>, primitives: &[Value]) -> Run<()> {
    let Some(&first) = primitives.first() else {
        return Ok(());
    };
    let mut prev_pitch = cause_steps(tx, first)?;
    for i in 1..primitives.len() {
        let pitch = cause_steps(tx, primitives[i])?;
        let delta = tx.heap().integer(i64::from(pitch) - i64::from(prev_pitch));
        grob::set_property(tx.heap(), primitives[i - 1], "delta-position", delta);
        prev_pitch = pitch;
    }
    let zero = tx.heap().integer(0);
    grob::set_property(
        tx.heap(),
        primitives[primitives.len() - 1],
        "delta-position",
        zero,
    );
    Ok(())
}

/// `move_related_items_to_column` (`:86–118`): each element of `item`'s
/// column on its staff symbol (from the last) given `target_column` as its
/// X parent and moved `offset` on X. LilyPond reads through an item of no
/// column; here nothing moves.
pub(crate) fn move_related_items_to_column(
    tx: &mut Tx<'_>,
    item: Value,
    target_column: Value,
    offset: f64,
) -> Run<()> {
    let Some(source_column) = item::get_column(tx.heap(), item) else {
        return Ok(());
    };
    let staff = staff_symbol(tx.host, item)?;
    let elements = grob_array::extract_grob_array(tx.host, source_column, "elements")?;
    for &sibling in elements.iter().rev() {
        if staff_symbol(tx.host, sibling)? != staff {
            // sibling is from a staff different than that of the item of
            // interest
            continue;
        }
        grob::set_parent(tx.heap(), sibling, target_column, X);
        translate(tx, sibling, offset, X)?;
    }
    Ok(())
}

/// `translate_axis` on a grob, LilyPond's programming error for an
/// infinite offset.
pub(crate) fn translate(tx: &mut Tx<'_>, g: Value, by: f64, a: crate::geometry::Axis) -> Run<()> {
    match grob::translate_axis(tx.heap(), g, by, a) {
        Ok(()) => Ok(()),
        Err(text) => grob::programming_error(tx.host, g, text),
    }
}
