//! `page-turn-engraver.cc` (MS8 step 9, part 3; 455 lines):
//! `Page_turn_engraver`, a class of layout's ([`PAGE_TURN_ENGRAVER`]) that
//! shadows the music crate's placeholder of its name (D67, D108). It
//! decides where the page-turn breaker (`page_breakers.rs`'
//! `ly:page-turn-breaking`) may turn a page: after a rest at least
//! `pageTurnMinimumRestLength` long (1 by default), at a special bar line
//! within the rest if there is one, unless a repeat would have to be
//! turned back over.
//!
//! - Each timestep (`stop_translation_timestep`, `:261–340`): the
//!   timestep's command column kept with its moment and whether a special
//!   bar line (a glyph neither `""` nor `"|"`) was acknowledged or is the
//!   context's `currentBarLine`; `repeatCommands`' start and end of a
//!   repeat, an end penalizing the repeat's span (`pageTurnMinimumRepeatLength`).
//!   A column that is not breakable is dropped at the next timestep's start
//!   (`start_translation_timestep`, `:242–259`).
//! - A note head heard (`:183–206`): the rest before it, if any, an
//!   automatic turn when long enough; its end where the next rest begins.
//! - A `page-turn-event` (`listen_break`, `:208–222`): a manual turn.
//! - `finalize` (`:360–433`): the automatic turns filtered through the
//!   repeats' penalties (`Page_turn_event::penalize`, `:51–79`), each put on
//!   its column (`breakable_column`, `:121–145`: the last special bar line
//!   in it, else its last column) with the more permissive permission and
//!   the smaller penalty; a turn allowed at the end of the piece; then the
//!   manual turns, which override.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;

use lily_engine_music::{
    FINALIZE, Kind, Moment, Rational, START, STOP, TranslatorClass, Tx, Work, event_property,
};
use lily_engine_scheme::{Exception, Heap, Value};

use crate::bar_engraver::long_or;
use crate::paper_column_engraver::rational;
use crate::tie_engraver::event_cause;
use crate::{grob, item, paper_column};

type Run<T> = Result<T, Exception>;

pub static PAGE_TURN_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Page_turn_engraver",
    kind: Kind::Engraver,
    methods: START | STOP | FINALIZE,
    // `stop_translation_timestep` keeps every timestep's command column.
    always: true,
    hears: &["break-event"],
    acknowledges: &[
        ("bar-line-interface", false),
        ("note-head-interface", false),
    ],
    must_be_last: false,
    make: Some(|| Box::new(PageTurnEngraver::default())),
    reads: &[
        "currentBarLine",
        "currentCommandColumn",
        "pageTurnMinimumRestLength",
        "pageTurnMinimumRepeatLength",
        "repeatCommands",
    ],
    writes: &[],
};

/// `Page_turn_event`: a span of the music where a turn is allowed
/// (`permission`, a symbol or `'()`) at a penalty.
#[derive(Clone, Copy)]
struct PageTurnEvent {
    permission: Value,
    penalty: f64,
    start: Rational,
    end: Rational,
}

/// `Interval_t<Rational>::is_empty`.
fn is_empty(start: Rational, end: Rational) -> bool {
    start.compare(end) == Ordering::Greater
}

fn max_rational(a: Rational, b: Rational) -> Rational {
    if a.compare(b) == Ordering::Less { b } else { a }
}

fn min_rational(a: Rational, b: Rational) -> Rational {
    if b.compare(a) == Ordering::Less { b } else { a }
}

fn lt(a: Rational, b: Rational) -> bool {
    a.compare(b) == Ordering::Less
}

impl PageTurnEvent {
    /// `penalize`: this span re-penalized where `penalty`'s span meets it,
    /// in up to three pieces: before, the intersection (unless `penalty`
    /// forbids the turn) at the greater penalty, and after.
    fn penalize(&self, penalty: &PageTurnEvent) -> Vec<PageTurnEvent> {
        let (start, end) = (
            max_rational(self.start, penalty.start),
            min_rational(self.end, penalty.end),
        );
        if is_empty(start, end) {
            return alloc::vec![*self];
        }
        let new_pen = self.penalty.max(penalty.penalty);
        let mut ret = Vec::new();
        if lt(self.start, penalty.start) {
            ret.push(PageTurnEvent {
                end: penalty.start,
                ..*self
            });
        }
        if penalty.permission != Value::NULL {
            ret.push(PageTurnEvent {
                permission: self.permission,
                penalty: new_pen,
                start,
                end,
            });
        }
        if lt(penalty.end, self.end) {
            ret.push(PageTurnEvent {
                start: penalty.end,
                ..*self
            });
        }
        ret
    }
}

#[derive(Clone)]
struct PageTurnEngraver {
    rest_begin: Moment,
    repeat_begin: Moment,
    note_end: Moment,
    repeat_begin_rest_length: Rational,
    found_special_bar_line: bool,
    forced_breaks: Vec<PageTurnEvent>,
    automatic_breaks: Vec<PageTurnEvent>,
    repeat_penalties: Vec<PageTurnEvent>,
    /// `breakable_moments_`, `breakable_columns_` and `special_barlines_`,
    /// in step.
    breakable_moments: Vec<Rational>,
    breakable_columns: Vec<Value>,
    special_barlines: Vec<bool>,
}

impl Default for PageTurnEngraver {
    fn default() -> PageTurnEngraver {
        PageTurnEngraver {
            rest_begin: Moment::default(),
            repeat_begin: Moment::new(Rational::int(-1), Rational::ZERO),
            note_end: Moment::default(),
            repeat_begin_rest_length: Rational::ZERO,
            found_special_bar_line: false,
            forced_breaks: Vec::new(),
            automatic_breaks: Vec::new(),
            repeat_penalties: Vec::new(),
            breakable_moments: Vec::new(),
            breakable_columns: Vec::new(),
            special_barlines: Vec::new(),
        }
    }
}

/// `is_bar_line_special`: a glyph neither empty nor `"|"`.
fn is_bar_line_special(glyph: &str) -> bool {
    !glyph.is_empty() && glyph != "|"
}

/// `from_scm (get_property (bar, "glyph"), "")`.
fn glyph(tx: &mut Tx<'_>, bar: Value) -> Run<String> {
    let v = grob::property(tx.host, bar, "glyph")?;
    Ok(tx
        .heap()
        .str(v)
        .map_or(String::new(), |s| String::from(s.as_str())))
}

/// `Moment - Moment`'s main part; zero where it overflows.
fn main_since(a: Moment, b: Moment) -> Rational {
    a.main.checked_sub(b.main).unwrap_or(Rational::ZERO)
}

/// `from_scm<double> (v, fallback)`.
fn double(heap: &Heap, v: Value, fallback: f64) -> f64 {
    if heap.is_real(v) {
        heap.to_f64(v).unwrap_or(fallback)
    } else {
        fallback
    }
}

impl PageTurnEngraver {
    /// `penalty`: infinite for a rest shorter than
    /// `pageTurnMinimumRestLength` (1 if it is no rational), else 0.
    fn penalty(&self, tx: &mut Tx<'_>, rest_len: Rational) -> f64 {
        let v = tx.get("pageTurnMinimumRestLength");
        let min_turn = rational(tx.heap(), v, Rational::int(1));
        if lt(rest_len, min_turn) {
            f64::INFINITY
        } else {
            0.0
        }
    }

    /// `breakable_column`: of the breakable columns in the span, the last
    /// at a special bar line, else the last; none if none is in it.
    fn breakable_column(&self, brk: &PageTurnEvent) -> Option<Value> {
        let moments = &self.breakable_moments;
        // `lower_bound` of the start, `upper_bound` of the end.
        let start = moments.partition_point(|&m| lt(m, brk.start));
        let end = moments.partition_point(|&m| !lt(brk.end, m));
        if start == moments.len() || end == 0 {
            return None;
        }
        let end_idx = end - 1;
        for i in (start..=end_idx).rev() {
            if self.special_barlines[i] {
                return Some(self.breakable_columns[i]);
            }
        }
        Some(self.breakable_columns[end_idx])
    }
}

/// `max_permission`: the more permissive, `force` over `allow` over `'()`.
fn max_permission(heap: &mut Heap, perm1: Value, perm2: Value) -> Value {
    if perm1 == Value::NULL {
        return perm2;
    }
    if perm1 == heap.intern("allow") && perm2 == heap.intern("force") {
        return perm2;
    }
    perm1
}

impl Work for PageTurnEngraver {
    fn listen(&mut self, tx: &mut Tx<'_>, _: usize, ev: Value) -> Run<()> {
        let class = event_property(tx.heap(), ev, "class");
        let heap = tx.heap();
        let name = heap.car(class).and_then(|c| heap.name(c));
        if name == Some("page-turn-event") {
            let permission = event_property(heap, ev, "break-permission");
            let pen = event_property(heap, ev, "break-penalty");
            let penalty = double(heap, pen, 0.0);
            let now = tx.now().main;
            self.forced_breaks.push(PageTurnEvent {
                permission,
                penalty,
                start: now,
                end: now,
            });
        }
        Ok(())
    }

    fn acknowledge(&mut self, tx: &mut Tx<'_>, which: usize, grob: u32) -> Run<()> {
        let g = tx.grob_handle(grob);
        if which == 0 {
            // acknowledge_bar_line, an item's (`Grob_info_t<Item>`)
            if !self.found_special_bar_line && item::is_item(tx.heap(), g) {
                let glyph = glyph(tx, g)?;
                self.found_special_bar_line = is_bar_line_special(&glyph);
            }
            return Ok(());
        }

        // acknowledge_note_head
        let Some(cause) = event_cause(tx, g)? else {
            return Ok(());
        };
        let dur = event_property(tx.heap(), cause, "duration");
        let Some(length) = tx
            .heap()
            .foreign::<lily_engine_music::Duration>(dur)
            .copied()
            .and_then(|d| d.length())
        else {
            return Ok(());
        };

        let now = tx.now();
        if self.rest_begin.compare(now) == Ordering::Less {
            let pen = self.penalty(tx, main_since(now, self.rest_begin));
            if !pen.is_infinite() {
                let allow = tx.heap().intern("allow");
                self.automatic_breaks.push(PageTurnEvent {
                    permission: allow,
                    penalty: 0.0,
                    start: self.rest_begin.main,
                    end: now.main,
                });
            }
        }

        if self.rest_begin.compare(self.repeat_begin) != Ordering::Greater {
            self.repeat_begin_rest_length = main_since(now, self.repeat_begin);
        }
        self.note_end = Moment::new(now.main.checked_add(length).unwrap_or(now.main), now.grace);
        Ok(())
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if let Some(&last) = self.breakable_columns.last()
            && !paper_column::is_breakable(tx.host, last)?
        {
            self.breakable_columns.pop();
            self.breakable_moments.pop();
            self.special_barlines.pop();
        }
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let pc = tx.get("currentCommandColumn");
        if lily_engine_music::grob_of(tx.heap(), pc).is_some() {
            // In a context below where bar lines are engraved (a Voice), no
            // bar line is acknowledged, but one made above is the
            // context's `currentBarLine`.
            if !self.found_special_bar_line {
                let bar = tx.get("currentBarLine");
                if lily_engine_music::grob_of(tx.heap(), bar).is_some()
                    && item::is_item(tx.heap(), bar)
                {
                    let glyph = glyph(tx, bar)?;
                    self.found_special_bar_line = is_bar_line_special(&glyph);
                }
            }
            self.breakable_columns.push(pc);
            self.breakable_moments.push(tx.now().main);
            self.special_barlines.push(self.found_special_bar_line);
        }

        // As Bar_engraver's.
        let mut start = false;
        let mut end = false;
        let (start_sym, end_sym) = {
            let heap = tx.heap();
            (heap.intern("start-repeat"), heap.intern("end-repeat"))
        };
        let mut p = tx.get("repeatCommands");
        while let (Some(mut command), Some(rest)) = (tx.heap().car(p), tx.heap().cdr(p)) {
            let mut options = Value::NULL;
            // (command option...)
            if let (Some(c), Some(o)) = (tx.heap().car(command), tx.heap().cdr(command)) {
                options = o;
                command = c;
            }
            let count = |tx: &mut Tx<'_>, fallback: i64| match tx.heap().car(options) {
                Some(first) => long_or(tx, first, fallback),
                None => fallback,
            };
            if command == start_sym {
                // Trivial repeats are ignored.
                if count(tx, 2) >= 2 {
                    start = true;
                }
            } else if command == end_sym && count(tx, 1) >= 1 {
                end = true;
            }
            p = rest;
        }

        let now = tx.now();
        if end && !self.repeat_begin.main.is_negative() {
            let rest = main_since(now, self.rest_begin)
                .checked_add(self.repeat_begin_rest_length)
                .unwrap_or(Rational::ZERO);
            let mut pen = self.penalty(tx, rest);

            let v = tx.get("pageTurnMinimumRepeatLength");
            let min_repeat = rational(tx.heap(), v, Rational::ZERO);
            if lt(main_since(now, self.repeat_begin), min_repeat) {
                pen = f64::INFINITY;
            }

            let (permission, penalty) = if pen == f64::INFINITY {
                (Value::NULL, f64::NEG_INFINITY)
            } else {
                (tx.heap().intern("allow"), pen)
            };
            self.repeat_penalties.push(PageTurnEvent {
                permission,
                penalty,
                start: self.repeat_begin.main,
                end: now.main,
            });
            self.repeat_begin = Moment::new(Rational::int(-1), Rational::ZERO);
        }

        if start {
            self.repeat_begin = now;
            self.repeat_begin_rest_length = Rational::ZERO;
        }
        self.rest_begin = self.note_end;
        self.found_special_bar_line = false;
        Ok(())
    }

    fn finalize(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let mut rep_index = 0;
        let mut auto_breaks = Vec::new();

        // The automatic breaks filtered through the repeat penalties.
        let mut i = 0;
        while i < self.automatic_breaks.len() {
            let brk = self.automatic_breaks[i];
            // The next applicable repeat penalty.
            while rep_index < self.repeat_penalties.len()
                && !lt(brk.start, self.repeat_penalties[rep_index].end)
            {
                rep_index += 1;
            }

            if rep_index >= self.repeat_penalties.len()
                || !lt(self.repeat_penalties[rep_index].start, brk.end)
            {
                auto_breaks.push(brk);
            } else {
                let mut split = brk.penalize(&self.repeat_penalties[rep_index]);
                // The last of the split events may overlap the next repeat
                // penalty: it is filtered again.
                if rep_index + 1 < self.repeat_penalties.len()
                    && let Some(&last) = split.last()
                    && lt(self.repeat_penalties[rep_index + 1].start, last.end)
                {
                    self.automatic_breaks[i] = last;
                    split.pop();
                    auto_breaks.extend(split);
                    continue;
                }
                auto_breaks.extend(split);
            }
            i += 1;
        }

        // The automatic breaks applied.
        for brk in &auto_breaks {
            if let Some(pc) = self.breakable_column(brk) {
                let cur = grob::property(tx.host, pc, "page-turn-permission")?;
                let perm = max_permission(tx.heap(), cur, brk.permission);
                let cur_pen = grob::property(tx.host, pc, "page-turn-penalty")?;
                let pen = double(tx.heap(), cur_pen, f64::INFINITY).min(brk.penalty);
                let heap = tx.heap();
                grob::set_property(heap, pc, "page-turn-permission", perm);
                let pen = heap.flonum(pen);
                grob::set_property(heap, pc, "page-turn-penalty", pen);
            }
        }

        // Unless a manual break overrides it, a page turn is allowed at
        // the end of the piece.
        if let Some(&last) = self.breakable_columns.last() {
            let heap = tx.heap();
            let allow = heap.intern("allow");
            grob::set_property(heap, last, "page-turn-permission", allow);
        }

        // The manual breaks applied.
        for brk in &self.forced_breaks {
            if let Some(pc) = self.breakable_column(brk) {
                let heap = tx.heap();
                grob::set_property(heap, pc, "page-turn-permission", brk.permission);
                let pen = heap.flonum(brk.penalty);
                grob::set_property(heap, pc, "page-turn-penalty", pen);
            }
        }
        Ok(())
    }

    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.moment(self.rest_begin);
        h.moment(self.repeat_begin);
        h.moment(self.note_end);
        h.rational(self.repeat_begin_rest_length);
        h.flags(&[self.found_special_bar_line]);
        for events in [
            &self.forced_breaks,
            &self.automatic_breaks,
            &self.repeat_penalties,
        ] {
            h.num(events.len() as i64);
            for e in events {
                h.float(e.penalty);
                h.rational(e.start);
                h.rational(e.end);
            }
        }
        h.num(self.breakable_moments.len() as i64);
        for (&m, &special) in self.breakable_moments.iter().zip(&self.special_barlines) {
            h.rational(m);
            h.flags(&[special]);
        }
    }

    fn values(&self, out: &mut Vec<Value>) {
        for events in [
            &self.forced_breaks,
            &self.automatic_breaks,
            &self.repeat_penalties,
        ] {
            out.extend(events.iter().map(|e| e.permission));
        }
        out.extend_from_slice(&self.breakable_columns);
    }
}
