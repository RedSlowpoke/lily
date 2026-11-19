//! `figured-bass-engraver.cc` (MS4 step 16): `Figured_bass_engraver`
//! ([`FIGURED_BASS_ENGRAVER`]), which `FiguredBass` and every `Staff`
//! consist, a class of this crate's that shadows the music crate's
//! placeholder of its name (D67, D108).
//!
//! - The figures heard (`:166–188`), each a group's (`Figure_group`, one
//!   per place in the chord): with `useBassFigureExtenders`, one equal to
//!   a group's figure of before continues it, unless `no-continuation`;
//!   the others are new. The figures' end is kept: until then a timestep
//!   begins (`:139–151`) and ends (`:153–164`) with what it had; after
//!   it, a timestep in which no group has a figure ends the spanners.
//! - `process_music` (`:248–386`): the spanners ended where no figure
//!   comes, at a rest (`ignoreFiguredBassRest`), at each figure without
//!   extenders, and where no figure continues; the new figures given
//!   groups, the first free ones; a continued figure's group gets a
//!   `BassFigureContinuation` from its figure of before, whose `figures`
//!   it is, on the group's line; a group whose figure no longer continues
//!   leaves its continuation, centred with its neighbours' on their
//!   figures (`figuredBassCenterContinuations`, `:190–246`).
//! - `create_grobs` (`:388–496`): the `BassFigureAlignment`, from the
//!   musical column to the musical column; each group's `BassFigure`
//!   (transparent if `implicitBassFigures` has its number, or for `<_>`,
//!   as an invisible 0), on the group's `BassFigureLine`, its `text` its
//!   markup, else what `figuredBassFormatter` gives of its number; a
//!   continued figure transparent, its text the formatter's of 0 without
//!   the alterations, the continuation's right bound.
//! - `add_brackets` (`:498–530`): a `BassFigureBracket` of the figures
//!   from a `bracket-start` to a `bracket-stop`.
//!
//! It is woken at each timestep but those in which it has nothing, as
//! LilyPond calls it at each: no groups, spanners, figures, rest or
//! figure that lasts.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;

use lily_engine_layout::grob_array::{GrobArray, add_grob};
use lily_engine_layout::tie_engraver::event_length;
use lily_engine_layout::{Direction, Y, align, axis_group, grob, is_markup, item, spanner};
use lily_engine_music::{
    Cause, Kind, Moment, PROCESS, Rational, START, STOP, TranslatorClass, Tx, Work, clone_event,
    event_property, set_event_property,
};
use lily_engine_scheme::{Exception, Heap, Value};

type Run<T> = Result<T, Exception>;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;

pub static FIGURED_BASS_ENGRAVER: TranslatorClass = TranslatorClass {
    name: "Figured_bass_engraver",
    kind: Kind::Engraver,
    methods: START | PROCESS | STOP,
    always: false,
    hears: &["rest-event", "bass-figure-event"],
    acknowledges: &[],
    must_be_last: false,
    make: Some(|| Box::new(FiguredBassEngraver::default())),
    reads: &[
        "figuredBassAlterationDirection",
        "figuredBassCenterContinuations",
        "figuredBassFormatter",
        "implicitBassFigures",
        "useBassFigureExtenders",
        "ignoreFiguredBassRest",
    ],
    writes: &[],
};

/// `from_scm<bool>`: `#t` only.
fn is_true(v: Value) -> bool {
    v == Value::TRUE
}

/// `Figure_group`: a place in the chord's figures, its spanners by their
/// records and handles.
#[derive(Clone, Copy)]
struct FigureGroup {
    group: Option<(u32, Value)>,
    continuation_line: Option<(u32, Value)>,

    number: Value,
    alteration: Value,
    augmented: Value,
    diminished: Value,
    augmented_slash: Value,
    text: Value,

    figure_item: Option<Value>,
    current_event: Option<Value>,
}

/// The event properties a group keeps, in its fields' order.
const KEPT: [&str; 6] = [
    "figure",
    "alteration",
    "augmented",
    "diminished",
    "augmented-slash",
    "text",
];

impl FigureGroup {
    fn new() -> FigureGroup {
        FigureGroup {
            group: None,
            continuation_line: None,
            number: Value::FALSE,
            alteration: Value::FALSE,
            augmented: Value::FALSE,
            diminished: Value::FALSE,
            augmented_slash: Value::FALSE,
            text: Value::FALSE,
            figure_item: None,
            current_event: None,
        }
    }

    /// Reset (or init) all figure information to FALSE
    fn reset_figure(&mut self) {
        for v in self.kept_mut() {
            *v = Value::FALSE;
        }
    }

    fn kept(&self) -> [Value; 6] {
        [
            self.number,
            self.alteration,
            self.augmented,
            self.diminished,
            self.augmented_slash,
            self.text,
        ]
    }

    fn kept_mut(&mut self) -> [&mut Value; 6] {
        [
            &mut self.number,
            &mut self.alteration,
            &mut self.augmented,
            &mut self.diminished,
            &mut self.augmented_slash,
            &mut self.text,
        ]
    }

    fn group_is_equal_to(&self, heap: &mut Heap, evt: Value) -> bool {
        self.kept().iter().zip(KEPT).all(|(&v, name)| {
            let w = event_property(heap, evt, name);
            heap.equal(v, w)
        })
    }

    fn is_continuation(&self, heap: &mut Heap) -> bool {
        self.current_event
            .is_some_and(|ev| self.group_is_equal_to(heap, ev))
    }

    fn assign_from_event(&mut self, heap: &mut Heap, currevt: Value, item: Value) {
        for (v, name) in self.kept_mut().into_iter().zip(KEPT) {
            *v = event_property(heap, currevt, name);
        }
        self.figure_item = Some(item);
    }

    fn values(&self, out: &mut Vec<Value>) {
        out.extend(self.group.map(|(_, h)| h));
        out.extend(self.continuation_line.map(|(_, h)| h));
        out.extend(self.kept());
        out.extend(self.figure_item);
        out.extend(self.current_event);
    }
}

/// The engraver's state; `rest_heard` is `rest_listener_`'s.
#[derive(Clone, Default)]
struct FiguredBassEngraver {
    groups: Vec<FigureGroup>,
    alignment: Option<(u32, Value)>,
    new_events: Vec<Value>,
    continuation: bool,
    new_event_found: bool,
    stop_moment: Moment,
    rest_heard: bool,
}

impl Work for FiguredBassEngraver {
    fn held(&self, h: &mut dyn lily_engine_music::Held) {
        h.flags(&[self.continuation, self.new_event_found, self.rest_heard]);
        h.moment(self.stop_moment);
        h.num(self.groups.len() as i64);
        for g in &self.groups {
            h.flags(&[
                g.group.is_some(),
                g.continuation_line.is_some(),
                g.figure_item.is_some(),
                g.current_event.is_some(),
            ]);
        }
        h.maybe_record(self.alignment.map(|a| a.0));
    }

    fn start_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        if self.before_stop(tx.now()) {
            return Ok(());
        }

        self.rest_heard = false;
        self.new_events.clear();
        for g in &mut self.groups {
            g.current_event = None;
        }

        self.continuation = false;
        Ok(())
    }

    fn stop_translation_timestep(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let now = tx.now();
        if !self.groups.is_empty()
            && !self.before_stop(now)
            && !self.groups.iter().any(|g| g.current_event.is_some())
        {
            self.clear_spanners(tx)?;
        }

        // Woken where a start or a process of LilyPond's would do anything.
        if !self.groups.is_empty()
            || self.alignment.is_some()
            || !self.new_events.is_empty()
            || self.continuation
            || self.new_event_found
            || self.rest_heard
            || self.stop_moment.main.compare(now.main) == Ordering::Greater
        {
            tx.stay_awake();
        }
        Ok(())
    }

    fn listen(&mut self, tx: &mut Tx<'_>, which: usize, ev: Value) -> Run<()> {
        if which == 0 {
            self.rest_heard = true;
            return Ok(());
        }
        // listen_bass_figure
        self.new_event_found = true;
        let now = tx.now();
        let length = event_length(tx.heap(), ev, now);
        let stop = now
            .checked_add(length)
            .unwrap_or(Moment::new(Rational::INFINITY, Rational::ZERO));
        if self.stop_moment.compare(stop) == Ordering::Less {
            self.stop_moment = stop;
        }

        // Handle no-continuation here, don't even add it to the already existing
        // spanner... This fixes some layout issues (figure will be placed separately)
        if is_true(tx.get("useBassFigureExtenders"))
            && !is_true(event_property(tx.heap(), ev, "no-continuation"))
        {
            for i in 0..self.groups.len() {
                if self.groups[i].current_event.is_none()
                    && self.groups[i].group_is_equal_to(tx.heap(), ev)
                {
                    self.groups[i].current_event = Some(ev);
                    self.continuation = true;
                    return Ok(());
                }
            }
        }
        self.new_events.push(ev);
        Ok(())
    }

    fn process_music(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let depth = tx.heap().root_depth();
        let ran = self.process(tx);
        tx.heap().unroot_to(depth);
        ran
    }

    fn values(&self, out: &mut Vec<Value>) {
        for g in &self.groups {
            g.values(out);
        }
        out.extend(self.alignment.map(|(_, h)| h));
        out.extend_from_slice(&self.new_events);
    }
}

impl FiguredBassEngraver {
    /// Whether `now` is before the figures' end, or in grace time before
    /// a moment.
    fn before_stop(&self, now: Moment) -> bool {
        now.main.compare(self.stop_moment.main) == Ordering::Less || now.grace.is_negative()
    }

    /// `center_continuations`: each line's `figures` its own copy of the
    /// lines' left bounds, the last first.
    fn center_continuations(tx: &mut Tx<'_>, consecutive_lines: &[Value]) -> Run<()> {
        let mut left_figs = Vec::new();
        for &line in consecutive_lines.iter().rev() {
            left_figs.extend(spanner::get_bound(tx.host, line, LEFT)?);
        }

        let figures = tx.heap().intern("figures");
        for &line in consecutive_lines.iter().rev() {
            let heap = tx.heap();
            let ga = heap.wrap(GrobArray {
                grobs: left_figs.clone(),
                ordered: true,
            });
            grob::set_object(heap, line, figures, ga);
        }
        Ok(())
    }

    /// The column of the spanner's bound `d`.
    fn bound_column(tx: &mut Tx<'_>, sp: Value, d: Direction) -> Run<Option<Value>> {
        Ok(spanner::get_bound(tx.host, sp, d)?.and_then(|b| item::get_column(tx.heap(), b)))
    }

    /// `center_repeated_continuations`: the continuations of neighbouring
    /// groups that span the same columns centred together.
    fn center_repeated_continuations(&self, tx: &mut Tx<'_>) -> Run<()> {
        let mut consecutive_lines: Vec<Value> = Vec::new();
        for group in &self.groups {
            let same = match (group.continuation_line, consecutive_lines.first()) {
                (Some(_), None) => true,
                (Some((_, cont_line)), Some(&first)) => {
                    Self::bound_column(tx, first, LEFT)? == Self::bound_column(tx, cont_line, LEFT)?
                        && Self::bound_column(tx, first, RIGHT)?
                            == Self::bound_column(tx, cont_line, RIGHT)?
                }
                (None, _) => false,
            };
            if let (true, Some((_, cont_line))) = (same, group.continuation_line) {
                consecutive_lines.push(cont_line);
            } else {
                Self::center_continuations(tx, &consecutive_lines)?;
                consecutive_lines.clear();
            }
        }

        Self::center_continuations(tx, &consecutive_lines)
    }

    /// `clear_spanners`: the alignment, each group's line and each
    /// continuation ended.
    fn clear_spanners(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let Some((alignment, _)) = self.alignment.take() else {
            return Ok(());
        };
        tx.announce_end_grob(alignment);

        if is_true(tx.get("figuredBassCenterContinuations")) {
            self.center_repeated_continuations(tx)?;
        }

        for g in &mut self.groups {
            if let Some((id, _)) = g.group.take() {
                tx.announce_end_grob(id);
            }

            if let Some((id, _)) = g.continuation_line.take() {
                tx.announce_end_grob(id);
            }
        }

        /* Check me, groups_.clear () ? */
        Ok(())
    }

    /// `process_music`, its values rooted by the caller's depth.
    fn process(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let use_extenders = is_true(tx.get("useBassFigureExtenders"));
        if self.alignment.is_some() && !use_extenders && !self.before_stop(tx.now()) {
            self.clear_spanners(tx)?;
        }

        // If we have a rest, or we have no new or continued events, clear all spanners
        if (!self.continuation && self.new_events.is_empty())
            || (self.rest_heard && is_true(tx.get("ignoreFiguredBassRest")))
        {
            self.clear_spanners(tx)?;
            self.groups.clear();
            return Ok(());
        }

        if !self.new_event_found {
            return Ok(());
        }

        self.new_event_found = false;

        /*
         Don't need to sync alignments, if we're not using extenders.
        */
        if !use_extenders {
            self.clear_spanners(tx)?;
        }

        if !self.continuation {
            self.clear_spanners(tx)?;
            self.groups.clear();
        }

        let mut k = 0;
        for &ev in &self.new_events {
            while k < self.groups.len() && self.groups[k].current_event.is_some() {
                k += 1;
            }

            if k >= self.groups.len() {
                self.groups.push(FigureGroup::new());
            }

            let group = &mut self.groups[k];
            group.reset_figure();
            group.current_event = Some(ev);
            group.figure_item = None;
            k += 1;
        }

        for i in 0..self.groups.len() {
            if !self.groups[i].is_continuation(tx.heap()) {
                self.groups[i].reset_figure();
            }
        }

        if use_extenders {
            let mut junk_continuations = Vec::new();
            for i in 0..self.groups.len() {
                if self.groups[i].is_continuation(tx.heap()) {
                    if self.groups[i].continuation_line.is_none() {
                        let id = tx.make_defined_grob("BassFigureContinuation", Cause::None)?;
                        let line = tx.grob_handle(id);
                        let group = &mut self.groups[i];
                        group.continuation_line = Some((id, line));
                        let item = group.figure_item.take();
                        let parent = group.group;
                        if let Some(item) = item {
                            spanner::set_bound(tx.host, line, LEFT, item)?;
                        }

                        /*
                          Don't add as child. This will cache the wrong
                          (pre-break) stencil when callbacks are triggered.
                        */
                        if let Some((_, parent)) = parent {
                            grob::set_parent(tx.heap(), line, parent, Y);
                        }
                        if let Some(item) = item {
                            let figures = tx.heap().intern("figures");
                            add_grob(tx.host, line, figures, item)?;
                        }
                    }
                } else if self.groups[i].continuation_line.is_some() {
                    junk_continuations.push(i);
                }
            }

            /*
             Ugh, repeated code.
            */
            let mut consecutive = Vec::new();
            if is_true(tx.get("figuredBassCenterContinuations")) {
                let line =
                    |groups: &[FigureGroup], i: usize| groups[i].continuation_line.map(|(_, h)| h);
                for i in 0..=junk_continuations.len() {
                    if i < junk_continuations.len()
                        && (i == 0 || junk_continuations[i - 1] + 1 == junk_continuations[i])
                    {
                        consecutive.extend(line(&self.groups, junk_continuations[i]));
                    } else {
                        Self::center_continuations(tx, &consecutive)?;
                        consecutive.clear();
                        if i < junk_continuations.len() {
                            consecutive.extend(line(&self.groups, junk_continuations[i]));
                        }
                    }
                }
            }
            for &i in &junk_continuations {
                self.groups[i].continuation_line = None;
            }
        }

        self.create_grobs(tx)?;
        self.add_brackets(tx)
    }

    /// `create_grobs`.
    fn create_grobs(&mut self, tx: &mut Tx<'_>) -> Run<()> {
        let muscol = tx.get("currentMusicalColumn");
        let alignment = match self.alignment {
            Some((_, h)) => h,
            None => {
                let id = tx.make_defined_grob("BassFigureAlignment", Cause::None)?;
                let h = tx.grob_handle(id);
                self.alignment = Some((id, h));
                spanner::set_bound(tx.host, h, LEFT, muscol)?;
                h
            }
        };
        spanner::set_bound(tx.host, alignment, RIGHT, muscol)?;

        let proc = tx.get("figuredBassFormatter");
        let context = tx.t.arena.borrow().handle(tx.context);
        for i in 0..self.groups.len() {
            if let Some(ev) = self.groups[i].current_event {
                let id = tx.make_defined_grob("BassFigure", Cause::Event(ev))?;
                let item = tx.grob_handle(id);
                self.groups[i].assign_from_event(tx.heap(), ev, item);

                let line = match self.groups[i].group {
                    Some((_, h)) => h,
                    None => {
                        let id = tx.make_defined_grob("BassFigureLine", Cause::None)?;
                        let h = tx.grob_handle(id);
                        self.groups[i].group = Some((id, h));
                        spanner::set_bound(tx.host, h, LEFT, muscol)?;
                        align::add_element(tx.host, alignment, h)?;
                        h
                    }
                };

                let group = self.groups[i];
                let implicit = tx.get("implicitBassFigures");
                if memq(tx.heap(), group.number, implicit) {
                    grob::set_property(tx.heap(), item, "transparent", Value::TRUE);
                    grob::set_property(tx.heap(), item, "implicit", Value::TRUE);
                }

                let mut number = group.number;
                if [
                    group.number,
                    group.text,
                    group.alteration,
                    group.augmented,
                    group.diminished,
                    group.augmented_slash,
                ]
                .iter()
                .all(|&v| v == Value::NULL)
                {
                    // Convert `<_>` to an invisible digit.
                    number = tx.heap().integer(0);
                    grob::set_property(tx.heap(), item, "transparent", Value::TRUE);
                    grob::set_property(tx.heap(), item, "implicit", Value::TRUE);
                }

                let mut text = group.text;
                if !is_markup(tx.host, text)? && tx.heap().is_procedure(proc) {
                    text = tx.host.call(proc, &[number, ev, context])?;
                }

                grob::set_property(tx.heap(), item, "text", text);

                axis_group::add_element(tx.host, line, item)?;
            }

            let group = self.groups[i];
            if let Some((_, cont_line)) = group.continuation_line {
                /*
                  UGH should connect to the bass staff, and get the note heads.
                  For now, simply set the hidden figure to a default value to
                  ensure the extenders of different figures always end at the same
                  position, e.g. in <12 5> <12 5>
                */
                let text = match group.current_event {
                    Some(event) if tx.heap().is_procedure(proc) => {
                        let Some(event) = clone_event(tx.heap(), event) else {
                            continue;
                        };
                        tx.heap().root(event);
                        for name in ["alteration", "augmented", "diminished", "augmented-slash"] {
                            let key = tx.heap().intern(name);
                            set_event_property(tx.heap(), event, key, Value::FALSE);
                        }

                        let zero = tx.heap().integer(0);
                        tx.host.call(proc, &[zero, event, context])?
                    }
                    _ => tx.heap().string("0"),
                };

                if let Some(figure_item) = group.figure_item {
                    grob::set_property(tx.heap(), figure_item, "transparent", Value::TRUE);
                    grob::set_property(tx.heap(), figure_item, "text", text);
                    spanner::set_bound(tx.host, cont_line, RIGHT, figure_item)?;
                }
            }

            if let Some((_, h)) = group.group {
                spanner::set_bound(tx.host, h, RIGHT, muscol)?;
            }
        }
        Ok(())
    }

    /// `add_brackets`.
    fn add_brackets(&self, tx: &mut Tx<'_>) -> Run<()> {
        let mut encompass = Vec::new();
        let mut inside = false;
        for group in &self.groups {
            let Some(ev) = group.current_event else {
                continue;
            };

            if is_true(event_property(tx.heap(), ev, "bracket-start")) {
                inside = true;
            }

            if inside && let Some(item) = group.figure_item {
                encompass.push(item);
            }

            if is_true(event_property(tx.heap(), ev, "bracket-stop")) {
                inside = false;

                let id = tx.make_defined_grob("BassFigureBracket", Cause::Event(ev))?;
                let brack = tx.grob_handle(id);
                let elements = tx.heap().intern("elements");
                for &e in &encompass {
                    add_grob(tx.host, brack, elements, e)?;
                }
                encompass.clear();
            }
        }
        Ok(())
    }
}

/// `scm_is_true (scm_memq (x, list))`.
fn memq(heap: &Heap, x: Value, mut list: Value) -> bool {
    while let (Some(car), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        if car == x {
            return true;
        }
        list = rest;
    }
    false
}
