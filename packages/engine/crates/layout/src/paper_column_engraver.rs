//! `paper-column-engraver.cc` (MS3 step 15): `Paper_column_engraver`, a
//! class of layout's ([`PAPER_COLUMN_ENGRAVER`]) that the core passes at
//! its `\layout` call and that a group finds before the music crate's
//! placeholder of the name (D67, D108).
//!
//! - Each timestep not skipped (`pre_process_music`, `:115–150`): a
//!   `NonMusicalPaperColumn` and a `PaperColumn` (`paper_column.rs`), set
//!   as `currentCommandColumn` and `currentMusicalColumn` and added to the
//!   root system (`system.rs`); the first command column the system's left
//!   bound, `line-break-permission` `allow`. Then the break events' penalties
//!   and permissions (`handle_manual_breaks`, `:68–112`), and `forceBreak`
//!   where one forces a permission.
//! - `process_music` (`:152–195`): the label events' `labels`, and the
//!   `measure-length` of the first command column of a measure.
//! - The items it acknowledges (`:197–221`) put on their column at the
//!   timestep's end (`stop_translation_timestep`, `:223–285`), with `when`
//!   and `rhythmic-location`; the spacing wishes and the break alignment on
//!   the timestep's columns. A forbidden break takes the command column's
//!   permissions after the first breakable one; `forbidBreak` and
//!   `forceBreak` are unset in the Score.
//! - `finalize` (`:287–308`): the last command column's page permissions,
//!   the break events' permissions, a line break allowed, and the column the
//!   system's right bound.
//!
//! The count of breakable columns is kept for the forbidden break of the
//! first moment; LilyPond's progress indication of it (`[8]`) is not
//! written, as the engine writes no progress.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use lily_engine_music::{
    Cause, FINALIZE, INITIALIZE, Kind, Moment, PRE_PROCESS, PROCESS, Rational, START, STOP,
    TranslatorClass, Tx, Work, event_property, grob_of,
};
use lily_engine_scheme::{Exception, Heap, Tag, Value};

use crate::geometry::X;
use crate::grob;
use crate::grob_array;
use crate::item;
use crate::paper_column;
use crate::separation_item;
use crate::spanner;
use crate::system;

type Run<T> = Result<T, Exception>;

pub static PAPER_COLUMN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Paper_column_engraver",
    kind: Kind::Engraver,
    methods: INITIALIZE | START | PRE_PROCESS | PROCESS | STOP | FINALIZE,
    always: true,
    hears: &["break-event", "label-event"],
    acknowledges: &[
        ("item-interface", false),
        ("note-spacing-interface", false),
        ("staff-spacing-interface", false),
        ("break-alignment-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(PaperColumnEngraver::default())),
    reads: &["forbidBreak"],
    writes: &[
        "forbidBreak",
        "forceBreak",
        "currentCommandColumn",
        "currentMusicalColumn",
    ],
};

/// The engraver's fields, the grobs and events by their handles.
#[derive(Clone, Default)]
struct PaperColumnEngraver {
    system: Option<Value>,
    break_events: Vec<Value>,
    label_events: Vec<Value>,
    breaks: u32,
    command_column: Option<Value>,
    musical_column: Option<Value>,
    items: Vec<Value>,
    /// `first_time_` is its negation.
    not_first: bool,
    have_timing: bool,
    skip_at_start: bool,
}

/// `set_property` of a grob, unchecked.
fn set(heap: &mut Heap, g: Value, name: &str, v: Value) {
    if let Some((arena, id)) = grob_of(heap, g) {
        let sym = heap.intern(name);
        grob::set_property_data(heap, &arena, id, sym, v);
    }
}

/// `from_scm<double> (v, fallback)`.
fn double(heap: &Heap, v: Value, fallback: f64) -> f64 {
    if heap.is_real(v) {
        heap.to_f64(v).unwrap_or(fallback)
    } else {
        fallback
    }
}

/// `from_scm (v, fallback)` of a rational.
pub(crate) fn rational(heap: &Heap, v: Value, fallback: Rational) -> Rational {
    if let Some((n, d)) = heap.as_ratio(v) {
        return match (heap.to_i64(n), heap.to_i64(d)) {
            (Some(n), Some(d)) => Rational::new(n, d),
            _ => fallback,
        };
    }
    match heap.as_flonum(v) {
        Some(x) if x.is_infinite() => {
            if x > 0.0 {
                Rational::INFINITY
            } else {
                -Rational::INFINITY
            }
        }
        Some(_) => fallback,
        None if heap.is_exact_integer(v) => heap.to_i64(v).map_or(fallback, Rational::int),
        None => fallback,
    }
}

/// `measure_length`: `measureLength`, 1 if it is no rational.
fn measure_length(tx: &mut Tx<'_>) -> Rational {
    let v = tx.get("measureLength");
    rational(tx.heap(), v, Rational::int(1))
}

/// Whether `measure_position (context ())`'s main part is zero (context.cc:
/// 886–942): `measurePosition` in `[0, measureLength)`, by the Euclidean
/// remainder; without a finite length, a negative position is `now`.
fn at_measure_start(tx: &mut Tx<'_>) -> Run<bool> {
    let length = measure_length(tx);
    let v = tx.get("measurePosition");
    let position = tx.heap().foreign::<Moment>(v).copied().unwrap_or_default();
    if length.is_zero() {
        tx.programming_error("cannot divide by zero measure length")?;
        return Ok(true);
    }
    let main = position.main;
    if length.is_finite() {
        // `euclidean_remainder`: zero exactly when `length` divides `main`.
        let quotient = main.checked_div(length);
        return Ok(quotient.is_some_and(|q| q.denominator() == 1));
    }
    if main.is_negative() {
        return Ok(tx.now().main.is_zero());
    }
    Ok(main.is_zero())
}

impl Work for PaperColumnEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.num(self.breaks.into());
        h.flags(&[self.not_first, self.have_timing, self.skip_at_start]);
    }

    fn initialize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let root = tx.get("rootSystem");
        self.system = (item::class(tx.heap(), root) == Some(item::Class::System)).then_some(root);
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.break_events.clear();
        self.skip_at_start = tx.get("skipTypesetting") == Value::TRUE;
        Ok(())
    }

    fn listen(&mut self, _: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        if which == 0 {
            self.break_events.push(ev);
        } else {
            self.label_events.push(ev);
        }
        Ok(())
    }

    fn pre_process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if !self.not_first {
            let sym = tx.heap().intern("internalBarNumber");
            self.have_timing = tx.t.arena.borrow().where_defined(tx.context, sym).is_some();
        }
        if !self.skip_at_start {
            let command = self.make_column(tx, "NonMusicalPaperColumn")?;
            self.command_column = Some(command);
            tx.set("currentCommandColumn", command);
            if let Some(system) = self.system {
                system::add_column(tx.host, system, command)?;
            }
            let musical = self.make_column(tx, "PaperColumn")?;
            self.musical_column = Some(musical);
            tx.set("currentMusicalColumn", musical);
            if let Some(system) = self.system {
                system::add_column(tx.host, system, musical)?;
                if spanner::get_bound(tx.host, system, -1)?.is_none() {
                    spanner::set_bound(tx.host, system, -1, command)?;
                    let allow = tx.heap().intern("allow");
                    set(tx.heap(), command, "line-break-permission", allow);
                }
            }
        }
        self.handle_manual_breaks(tx, false)
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(command) = self.command_column {
            for &ev in &self.label_events {
                let label = event_property(tx.heap(), ev, "page-label");
                let labels = grob::property(tx.host, command, "labels")?;
                let labels = tx.heap().cons(label, labels);
                set(tx.heap(), command, "labels", labels);
            }
        }
        let measure_start_now = if self.have_timing {
            tx.get("measureStartNow") == Value::TRUE
        } else {
            at_measure_start(tx)?
        };
        if measure_start_now {
            let length = measure_length(tx);
            let mlen = tx.heap().wrap(Moment::new(length, Rational::ZERO));
            let column = tx.get("currentCommandColumn");
            if grob_of(tx.heap(), column).is_some() {
                set(tx.heap(), column, "measure-length", mlen);
            } else {
                tx.programming_error("No command column?")?;
            }
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        match which {
            0 => self.items.push(g),
            1 | 2 => {
                let column = if which == 1 {
                    self.musical_column
                } else {
                    self.command_column
                };
                if let Some(column) = column {
                    let sym = tx.heap().intern("spacing-wishes");
                    grob_array::add_grob(tx.host, column, sym, g)?;
                }
            }
            _ => {
                if let Some(command) = self.command_column {
                    let sym = tx.heap().intern("break-alignment");
                    grob::set_object(tx.heap(), command, sym, g);
                }
            }
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        self.not_first = true;
        if tx.get("skipTypesetting") == Value::TRUE {
            return Ok(());
        }
        let (Some(command), Some(musical)) = (self.command_column, self.musical_column) else {
            return Ok(());
        };
        let now = tx.now();
        let m = tx.heap().wrap(now);
        set(tx.heap(), command, "when", m);
        set(tx.heap(), musical, "when", m);

        let mpos = tx.get("measurePosition");
        let barnum = tx.get("internalBarNumber");
        let heap = tx.heap();
        if heap.foreign::<Moment>(mpos).is_some() && heap.is_exact_integer(barnum) {
            let place = heap.cons(barnum, mpos);
            set(heap, command, "rhythmic-location", place);
            set(heap, musical, "rhythmic-location", place);
        }

        for elem in core::mem::take(&mut self.items) {
            let col = if item::is_non_musical(tx.host, elem)? {
                command
            } else {
                musical
            };
            let heap = tx.heap();
            if grob::get_parent(heap, elem, X).is_none() {
                grob::set_parent(heap, elem, col, X);
            }
            let sym = heap.intern("axis-group-parent-X");
            let parent = grob::object_data(heap, elem, sym);
            if grob_of(heap, parent).is_none() {
                grob::set_object(heap, elem, sym, col);
            }
            if grob::has_interface(heap, elem, "accidental-placement-interface")
                || grob::has_interface(heap, elem, "arpeggio-interface")
            {
                separation_item::add_conditional_item(tx.host, col, elem)?;
            } else if !grob::has_interface(heap, elem, "accidental-interface") {
                separation_item::add_item(tx.host, col, elem)?;
            }
        }

        let forbid = tx.get("forbidBreak") == Value::TRUE;
        let force = tx.get("forceBreak") == Value::TRUE;
        // A forbidden break on the score's first moment is not honoured.
        if forbid && !force && self.breaks > 0 {
            for name in [
                "page-turn-permission",
                "page-break-permission",
                "line-break-permission",
            ] {
                set(tx.heap(), command, name, Value::NULL);
            }
        } else if paper_column::is_breakable(tx.host, command)? {
            self.breaks += 1;
        }

        let score = find_score_context(tx);
        let mut a = tx.t.arena.borrow_mut();
        for name in ["forbidBreak", "forceBreak"] {
            let sym = tx.host.heap().intern(name);
            a.unset(score, sym);
        }
        drop(a);
        self.label_events.clear();
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some(command) = self.command_column else {
            return Ok(());
        };
        let allow = tx.heap().intern("allow");
        set(tx.heap(), command, "page-break-permission", allow);
        set(tx.heap(), command, "page-turn-permission", allow);
        self.handle_manual_breaks(tx, true)?;
        let permission = grob::property(tx.host, command, "line-break-permission")?;
        if permission.tag() != Tag::Symbol {
            set(tx.heap(), command, "line-break-permission", allow);
        }
        if let Some(system) = self.system {
            spanner::set_bound(tx.host, system, 1, command)?;
        }
        Ok(())
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.system);
        out.extend(self.command_column);
        out.extend(self.musical_column);
        out.extend_from_slice(&self.break_events);
        out.extend_from_slice(&self.label_events);
        out.extend_from_slice(&self.items);
    }
}

impl PaperColumnEngraver {
    /// `make_paper_column`: a column of the type `name`, announced with no
    /// cause.
    fn make_column(&mut self, tx: &mut Tx<'_>, name: &str) -> Run<Value> {
        let id = tx.make_defined_grob(name, Cause::None)?;
        let column = tx.grob_handle(id);
        paper_column::make(tx.heap(), column);
        Ok(column)
    }

    /// `handle_manual_breaks`: each break event's penalty added to the
    /// command column's, which allows the break, or else, and with
    /// `only_do_permissions`, its permission set; `forceBreak` where either
    /// gives a permission.
    fn handle_manual_breaks(&mut self, tx: &mut Tx<'_>, only_do_permissions: bool) -> Run<()> {
        for ev in self.break_events.clone() {
            let class = event_property(tx.heap(), ev, "class");
            let heap = tx.heap();
            let name = heap
                .car(class)
                .and_then(|c| heap.name(c))
                .map(String::from)
                .unwrap_or_default();
            let prefix = match name.rfind("-event") {
                Some(0) => {
                    return tx.programming_error(
                        "Paper_column_engraver doesn't know about this break-event",
                    );
                }
                Some(end) => &name[..end],
                None => name.as_str(),
            };
            let Some(command) = self.command_column else {
                continue;
            };
            let perm_str = alloc::format!("{prefix}-permission");
            let pen_str = alloc::format!("{prefix}-penalty");
            let cur_pen = grob::property(tx.host, command, &pen_str)?;
            let pen = event_property(tx.heap(), ev, "break-penalty");
            let perm = event_property(tx.heap(), ev, "break-permission");
            let heap = tx.heap();
            let force = if !only_do_permissions && heap.is_real(pen) {
                let new_pen = double(heap, cur_pen, 0.0) + double(heap, pen, 0.0);
                let new_pen = heap.flonum(new_pen);
                set(heap, command, &pen_str, new_pen);
                let allow = heap.intern("allow");
                set(heap, command, &perm_str, allow);
                true
            } else {
                set(heap, command, &perm_str, perm);
                perm != Value::NULL
            };
            if force {
                tx.set("forceBreak", Value::TRUE);
            }
        }
        Ok(())
    }
}

/// `find_score_context`: the context, or the nearest above it, that is a
/// `Score`; the context itself if none is.
pub(crate) fn find_score_context(tx: &mut Tx<'_>) -> lily_engine_music::CtxId {
    let score = tx.heap().intern("Score");
    let a = tx.t.arena.borrow();
    let mut at = Some(tx.context);
    while let Some(c) = at {
        if a.is_alias(tx.host.heap(), c, score) {
            return c;
        }
        at = a.parent(c);
    }
    tx.context
}
