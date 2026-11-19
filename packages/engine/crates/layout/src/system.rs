//! `system.cc` (part 3's, MS3): what `Score_engraver` and
//! `Paper_column_engraver` call of it, ported first (MS3 step 15): the two
//! constructors (`system.cc:73–77`, `:79–86`), `typeset_grob`
//! (`:116–132`) and `add_column` (`:501–517`), 47 lines, and
//! `init_elements` (`:88–94`), which the constructors call. Pulled forward
//! from step 33 for the spacing spanner (MS3 step 31,
//! `spacing_spanner.rs`): `used_columns_in_range` (`:751–777`, 27 lines),
//! `column` (`:779–787`, 9) and `get_root_system` (`:795–804`, 10).
//! Pulled forward from step 33 with the breakable items' pieces (MS3 step
//! 32, the user's answer at its session 6): `pre_processing`'s first three
//! scopes ([`make_prebroken_pieces`], `:520–553`), and of
//! `break_into_pieces` what places a spaced line's columns
//! ([`place_line`], `:471–474`, `:488`). MS3 step 33: `broken_col_range`
//! ([`broken_col_range`], `:723–745`), which `ly:spanner::set-spacing-rods`
//! reads (`spanner.rs`), and the system's rank ([`get_rank`]), which
//! `break_into_pieces` sets; the rest of `break_into_pieces`
//! ([`break_into_pieces`], `:449–491`, with `collect_labels`, `:493–499`,
//! [`collect_labels`])
//! and the copy constructor's `clone` (`:73–77`), which make the broken
//! systems of the lines `Paper_score::calc_breaking` gives; and
//! `do_break_substitution_and_fixup_refpoints`
//! ([`do_break_substitution_and_fixup_refpoints`], `:142–190`), which
//! breaks the spanners into a piece a broken system and moves each grob's
//! links and parents to its system. The rest of `pre_processing`
//! ([`pre_processing`], `:519–572`), `post_processing`
//! ([`post_processing`], `:574–594`) and the System's callbacks and its
//! staves ([`PRELUDE`], `:233–301`, `:352–446`, `:806–830`, `:892–990`,
//! `:1022–1072`). The paper systems: `get_broken_system_grobs`
//! ([`get_broken_system_grobs`], `:206–213`), `get_paper_systems`
//! ([`get_paper_systems`], `:215–231`) and `get_paper_system`
//! ([`get_paper_system`], `:596–721`), with paper-system.cc's
//! `make_paper_system` and `paper_system_set_stencil` borrowed
//! ([`make_paper_system`], [`paper_system_set_stencil`]).
//!
//! A system is a spanner of the class `System` (`item.rs`), whose
//! `pscore_` and `layout_` layout keeps with its grob (`grob.rs`), set
//! when it is typeset in a Paper_score (`paper_score.rs`). Its grobs are
//! its `all-elements`, an unordered grob array, and its columns its
//! `columns`. LilyPond's protection pool (`protection-pool.cc`'s
//! `new_protection_pool` and `protection_pool_add`, which the constructor,
//! `typeset_grob` and `break_into_pieces` call), which keeps a system's
//! grobs and broken systems from Guile's collector, is the heap's tracing
//! here: the system's object alist holds its grobs, its `broken_intos_`
//! its broken systems (`grob.rs`), and a paper system its system
//! (`system-grob`). `get_neighboring_staff` ([`get_neighboring_staff`],
//! `:833–861`, OPEN B2) is the hairpins' (MS3 step 49);
//! What `Constrained_breaking::fill_line_details` reads of the root system
//! (MS3 step 61): `begin_of_line_pure_height` and
//! `rest_of_line_pure_height` ([`begin_of_line_pure_height`],
//! [`rest_of_line_pure_height`], `:926–936`, over [`PRELUDE`]'s
//! `part_of_line_pure_height`), `pure_refpoint_extent`
//! ([`pure_refpoint_extent`], `:864–890`), and
//! `get_footnote_heights_in_range`, `get_in_note_heights_in_range` and
//! `internal_get_note_heights_in_range` ([`get_footnote_heights_in_range`],
//! [`get_in_note_heights_in_range`], `:303–343`, [`PRELUDE`]'s
//! `%system-note-heights-in-range`). `num_footnotes` (`:345–350`) has no
//! caller in LilyPond 2.26.0 and is not ported.

use crate::axis_group::Heights;
use crate::grob::OpenRead;
use crate::queue::Step;
use crate::spacing_spanner::Spacing;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use lily_engine_music::Host;
use lily_engine_music::returns::{due, time_up};
use lily_engine_scheme::{Cx, Exception, Heap, HostHandler, Value, Vm};

use crate::R;
use crate::axis_group;
use crate::geometry::{Box, Direction, Interval, X, Y};
use crate::grob::{self, with_grob, with_grob_mut};
use crate::grob_array::{self, GrobArray};
use crate::item::{self, Class};
use crate::page_layout_problem;
use crate::paper_column;
use crate::paper_score;
use crate::simple_spacer::ColumnXPositions;
use crate::spacing_loose_columns::set_loose_columns;
use crate::spanner;
use crate::stencil::Stencil;
use crate::unpure::pure_y_extent;

const LEFT: Direction = -1;
const RIGHT: Direction = 1;
const DOWN: Direction = -1;
const UP: Direction = 1;

/// `System::System (SCM)`: the grob `grob` made a system, with
/// `system-interface` after `Spanner`'s `spanner-interface`
/// ([`item::set_class`]), and its elements.
pub fn make(heap: &mut Heap, grob: Value) {
    item::set_class(heap, grob, Class::System);
    init_elements(heap, grob);
}

/// `System::System (System const &)`: the system's own part of `copy`, a
/// copy of a system: elements of its own.
pub fn copied(heap: &mut Heap, copy: Value) {
    init_elements(heap, copy);
}

/// `clone` of the system `system`: a spanner's copy
/// ([`spanner::clone`]), with no Paper_score or rank of its own either,
/// and the system's own part ([`copied`]). `None` for what is not a grob.
fn clone(host: &mut dyn Host, system: Value) -> Option<Value> {
    let copy = spanner::clone(host, system)?;
    copied(host.heap(), copy);
    Some(copy)
}

/// `init_elements`: an empty, unordered grob array set as `me`'s
/// `all-elements`.
fn init_elements(heap: &mut Heap, me: Value) {
    let sym = heap.intern("all-elements");
    let all = heap.wrap(GrobArray {
        grobs: alloc::vec::Vec::new(),
        ordered: false,
    });
    grob::set_object(heap, me, sym, all);
}

/// `typeset_grob`: `elem` given the layout of `system`'s Paper_score and
/// added to its `all-elements`; LilyPond's programming error, at the
/// system, for a grob that has a layout already.
pub fn typeset_grob(host: &mut dyn Host, system: Value, elem: Value) -> Result<(), Exception> {
    let heap = host.heap();
    if with_grob(heap, elem, |g| g.layout.is_some()) == Some(true) {
        return grob::programming_error(host, system, "adding element twice");
    }
    let pscore = with_grob(heap, system, |g| g.pscore).flatten();
    let layout = pscore.and_then(|ps| paper_score::layout(heap, ps));
    with_grob_mut(heap, elem, |g| g.layout = layout);
    let sym = heap.intern("all-elements");
    let all = grob_array::object(host, system, sym)?;
    if let Some(a) = host.heap().foreign_mut::<GrobArray>(all) {
        a.grobs.push(elem);
    }
    Ok(())
}

/// `add_column`: the column `p` added to `system`'s `columns`, made the
/// first time, its rank the number of columns before it, and made an
/// element of the system ([`axis_group::add_element`]).
pub fn add_column(host: &mut dyn Host, system: Value, p: Value) -> Result<(), Exception> {
    let sym = host.heap().intern("columns");
    let ga = grob_array::get_grob_array(host, system, sym)?;
    let heap = host.heap();
    let rank = heap.foreign::<GrobArray>(ga).map_or(0, |a| a.grobs.len());
    paper_column::set_rank(heap, p, rank as u64);
    if let Some(a) = heap.foreign_mut::<GrobArray>(ga) {
        a.grobs.push(p);
    }
    axis_group::add_element(host, system, p)
}

/// `pre_processing` (`system.cc:519–572`): the breakable items' pieces
/// ([`make_prebroken_pieces`]), then the reads of the two scopes after
/// them ([`read_before_breaking`]).
pub fn pre_processing(host: &mut dyn Host, system: Value) -> Result<(), Exception> {
    let mut processing = Processing::default();
    while !processing.on(host, system)? {}
    Ok(())
}

/// Where a [`Processing`] is.
#[derive(Clone, Copy, Default, PartialEq)]
enum Scope {
    #[default]
    Pieces,
    Dependencies,
    Reads,
    /// The system's own read of `READS[r]`, then each grob's.
    ReadSystem(usize),
    ReadGrobs(usize),
    /// Each line's pure heights, read here in steps (B23, D261): the read
    /// of a column or a rest that first wants them would pay for them all.
    LineHeights,
    Done,
}

/// The grobs of a score up to which each line's heights are read ahead of
/// `springs-and-rods` (D261), of `all-elements`: T24 has 5,005, T29 5,119,
/// T29 × 3 13,747, T29 × 8 35,272.
const LINE_HEIGHTS_GROBS: usize = 32_768;

/// The two reads of `pre_processing`'s last scopes, and what ends a step of each.
const READS: [(&str, Step); 2] = [
    ("before-line-breaking", Step::BeforeLineBreaking),
    ("springs-and-rods", Step::SpringsAndRods),
];

/// `pre_processing` under way (H3, D254): [`Processing::on`] goes on until it
/// is done or the step's time is spent, between grobs.
#[derive(Default)]
pub struct Processing {
    scope: Scope,
    /// The scope it stops before, if not [`Scope::Done`].
    until: Option<Scope>,
    /// The next grob of `all-elements`, and how many the scope reads.
    i: usize,
    n: usize,
    /// A spacing spanner's `springs-and-rods` under way (B23, D261), by column.
    spacing: Option<(Value, OpenRead, Spacing)>,
    /// The lines whose heights are to be read, the next last, from the top:
    /// from the bottom moves T29's marks, as D251's forced order moved them.
    /// And one line's read under way, by grob and measure.
    groups: Vec<Value>,
    heights: Option<(OpenRead, Heights)>,
    /// Lines of marks held while the lines below them are read (B28, D272),
    /// the last held last.
    held: Vec<Held>,
    /// The line whose read began last.
    line: Option<Value>,
}

/// Whether lines of marks are held (B28, D272), and how many have been: off
/// only for a test that compares LilyPond's nested read.
static HOLDING: AtomicBool = AtomicBool::new(true);
static HELD: AtomicU64 = AtomicU64::new(0);

/// Holds lines of marks or not, for the tests (B28, D272).
pub fn set_holding(on: bool) {
    HOLDING.store(on, Ordering::Relaxed);
}

/// How many lines of marks have been held (B28, D272).
pub fn held() -> u64 {
    HELD.load(Ordering::Relaxed)
}

/// A line of marks held before its first outside-staff measure (B28, D272):
/// LilyPond reads the lines below it there, nested in that measure's pure
/// height, through its alignment's translations between the measure's
/// columns. Held, they are read in steps of their own, in the same order,
/// and when it goes on what that nested read had asked before the lines were
/// read is made again: the translations, if they had not been kept then, and
/// each line's first part-of-line height.
struct Held {
    read: (OpenRead, Heights),
    /// Its alignment and the measure's columns, if the translations between
    /// them were not kept when it was held.
    fresh: Option<(Value, (i64, i64))>,
    /// The lines whose reads began while it was the last held.
    lines: Vec<Value>,
}

impl Processing {
    /// Goes on; whether it is done.
    pub fn on(&mut self, host: &mut dyn Host, system: Value) -> Result<bool, Exception> {
        let sym = host.heap().intern("all-elements");
        let all = grob_array::object(host, system, sym)?;
        let len = |host: &mut dyn Host| {
            host.heap()
                .foreign::<GrobArray>(all)
                .map_or(0, |a| a.grobs.len())
        };
        let at = |host: &mut dyn Host, i: usize| {
            host.heap()
                .foreign::<GrobArray>(all)
                .and_then(|a| a.grobs.get(i).copied())
        };
        loop {
            if Some(self.scope) == self.until {
                return Ok(true);
            }
            match self.scope {
                // The pieces are typeset at the end of the array, after the
                // originals.
                Scope::Pieces => {
                    if self.i == 0 {
                        self.n = len(host);
                    }
                    while self.i < self.n {
                        let g = at(host, self.i);
                        self.i += 1;
                        if let Some(g) = g {
                            item::break_breakable_item(host, g, system)?;
                            crate::queue::check(Step::Pieces);
                            if due() {
                                return Ok(false);
                            }
                        }
                    }
                    self.scope = Scope::Dependencies;
                    self.n = len(host);
                    self.i = self.n;
                }
                Scope::Dependencies => {
                    while self.i > 0 {
                        self.i -= 1;
                        if let Some(g) = at(host, self.i) {
                            item::handle_prebroken_dependencies(host, g)?;
                            crate::queue::check(Step::Pieces);
                            if due() {
                                return Ok(false);
                            }
                        }
                    }
                    for i in 0..self.n {
                        if let Some(g) = at(host, i) {
                            grob::fixup_refpoint(host.heap(), g);
                        }
                    }
                    self.scope = Scope::Reads;
                }
                Scope::Reads => {
                    crate::unpure::keep_pure_offsets();
                    crate::cell::before_begin(host, system)?;
                    self.scope = Scope::ReadSystem(0);
                    // What it gave back may have moved the array.
                    return self.on(host, system);
                }
                Scope::ReadSystem(r) if r == READS.len() => {
                    crate::cell::before_end(host, system);
                    self.scope = Scope::Done;
                }
                Scope::ReadSystem(r) => {
                    grob::property(host, system, READS[r].0)?;
                    self.n = len(host);
                    self.i = 0;
                    self.scope = Scope::ReadGrobs(r);
                }
                // A grob a callback typesets is not reached, as LilyPond's
                // loops end where the array ended when they began.
                Scope::ReadGrobs(r) => {
                    if !self.go_on_spacing(host)? {
                        return Ok(false);
                    }
                    while self.i < self.n {
                        let g = at(host, self.i);
                        self.i += 1;
                        if let Some(g) = g {
                            crate::cell::rerunning(host.heap(), g);
                            let spacing = match READS[r].0 == "springs-and-rods" {
                                true => spacing_read(host, g),
                                false => Ok(None),
                            };
                            match spacing {
                                Ok(Some(spacing)) => {
                                    self.spacing = Some(spacing);
                                    if !self.go_on_spacing(host)? {
                                        return Ok(false);
                                    }
                                }
                                Ok(None) => {
                                    let read = grob::property(host, g, READS[r].0);
                                    crate::cell::rerun_done(host.heap(), g);
                                    read?;
                                }
                                Err(e) => {
                                    crate::cell::rerun_done(host.heap(), g);
                                    return Err(e);
                                }
                            }
                            crate::queue::check(READS[r].1);
                            if due() {
                                return Ok(false);
                            }
                        }
                    }
                    self.scope = match READS[r].0 == "before-line-breaking" {
                        true => {
                            // Not on a score of more grobs: what it holds of the
                            // heights read ahead took T29 × 8 over the cap, and
                            // its steps are long anyway (OPEN B23).
                            if self.n <= LINE_HEIGHTS_GROBS {
                                self.groups = line_groups(host, system)?;
                                self.groups.reverse();
                            }
                            Scope::LineHeights
                        }
                        false => Scope::ReadSystem(r + 1),
                    };
                }
                Scope::LineHeights => {
                    if !self.go_on_heights(host)? {
                        return Ok(false);
                    }
                    self.scope = Scope::ReadSystem(1);
                }
                Scope::Done => return Ok(true),
            }
        }
    }
}

impl Processing {
    /// The spacing spanner's read under way goes on: whether it is over, if
    /// there is one.
    fn go_on_spacing(&mut self, host: &mut dyn Host) -> Result<bool, Exception> {
        let Some((g, _, spacing)) = self.spacing.as_mut() else {
            return Ok(true);
        };
        let g = *g;
        let on = spacing.on(host, &time_up);
        if matches!(on, Ok(false)) {
            return Ok(false);
        }
        if let Some((_, read, _)) = self.spacing.take() {
            read.close(host, on.as_ref().ok().map(|_| Value::UNSPECIFIED));
        }
        crate::cell::rerun_done(host.heap(), g);
        on
    }
}

impl Processing {
    /// Each line's heights go on; whether they are over.
    fn go_on_heights(&mut self, host: &mut dyn Host) -> Result<bool, Exception> {
        loop {
            if self.heights.is_none() {
                let Some(group) = self.groups.pop() else {
                    let Some(held) = self.held.pop() else {
                        return Ok(true);
                    };
                    let heap = host.heap();
                    for &line in &held.lines {
                        with_grob_mut(heap, line, |g| g.pure_fresh = true);
                    }
                    if let Some((align, key)) = held.fresh {
                        crate::align::make_fresh(heap, align, Some(key));
                    }
                    self.heights = Some(held.read);
                    // The held measure, and what it did not ask made as found.
                    let ran = self.go_on_held(host, held.fresh, &held.lines);
                    if !ran? {
                        return Ok(false);
                    }
                    continue;
                };
                if let Some(held) = self.held.last_mut() {
                    held.lines.push(group);
                }
                self.line = Some(group);
                self.heights = heights_read(host, group)?;
                // A line of marks (`staff-affinity` DOWN) with lines below.
                if !self.groups.is_empty()
                    && let Some((_, heights)) = self.heights.as_mut()
                {
                    let affinity = grob::property(host, group, "staff-affinity")?;
                    heights.hold =
                        affinity.as_fixnum() == Some(-1) && HOLDING.load(Ordering::Relaxed);
                }
                continue;
            }
            let Some((_, heights)) = self.heights.as_mut() else {
                continue;
            };
            if let Some((start, end)) = heights.holding() {
                heights.hold = false;
                if !self.groups.is_empty()
                    && let Some(group) = self.line
                    && let Some(align) = grob::get_parent(host.heap(), group, Y)
                    && grob::has_interface(host.heap(), align, "align-interface")
                {
                    let key = (start as i64, end as i64);
                    let kept = crate::align::translations_kept(host, align, key)?;
                    if let Some(read) = self.heights.take() {
                        HELD.fetch_add(1, Ordering::Relaxed);
                        self.held.push(Held {
                            read,
                            fresh: (!kept).then_some((align, key)),
                            lines: Vec::new(),
                        });
                    }
                    continue;
                }
            }
            let read = match heights.done() {
                true => None,
                false => Some(heights.next(host)),
            };
            match read {
                None => {
                    if let Some((open, heights)) = self.heights.take() {
                        let value = heights.end(host);
                        open.close(host, Some(value));
                    }
                }
                Some(Ok(())) => {}
                Some(Err(e)) => {
                    if let Some((open, _)) = self.heights.take() {
                        open.close(host, None);
                    }
                    return Err(e);
                }
            }
            crate::queue::check(Step::Box);
            if time_up() {
                return Ok(false);
            }
        }
    }
}

impl Processing {
    /// The held measure of the line that goes on (B28, D272), and what it
    /// did not ask of what was to be made again made as found again.
    fn go_on_held(
        &mut self,
        host: &mut dyn Host,
        fresh: Option<(Value, (i64, i64))>,
        lines: &[Value],
    ) -> Result<bool, Exception> {
        let read = match self.heights.as_mut() {
            Some((_, heights)) if !heights.done() => heights.next(host),
            _ => Ok(()),
        };
        let heap = host.heap();
        for &line in lines {
            with_grob_mut(heap, line, |g| g.pure_fresh = false);
        }
        if let Some((align, _)) = fresh {
            crate::align::make_fresh(heap, align, None);
        }
        if let Err(e) = read {
            if let Some((open, _)) = self.heights.take() {
                open.close(host, None);
            }
            return Err(e);
        }
        crate::queue::check(Step::Box);
        Ok(!time_up())
    }
}

/// The lines of the system's `vertical-alignment`, in order.
fn line_groups(host: &mut dyn Host, system: Value) -> Result<Vec<Value>, Exception> {
    let sym = host.heap().intern("vertical-alignment");
    let align = grob_array::object(host, system, sym)?;
    if with_grob(host.heap(), align, |_| ()).is_none() {
        return Ok(Vec::new());
    }
    grob_array::extract_grob_array(host, align, "elements")
}

/// `group`'s `adjacent-pure-heights` opened, with its read begun, if it has
/// not been read and is what a line's are; none else.
fn heights_read(
    host: &mut dyn Host,
    group: Value,
) -> Result<Option<(OpenRead, Heights)>, Exception> {
    let Some(proc) = host.lily("ly:axis-group-interface::adjacent-pure-heights") else {
        return Ok(None);
    };
    let sym = host.heap().intern("adjacent-pure-heights");
    let Some(read) = grob::open_read(host, group, sym, proc) else {
        return Ok(None);
    };
    match Heights::begin(host, group) {
        Ok(heights) => Ok(Some((read, heights))),
        Err(e) => {
            read.close(host, None);
            Err(e)
        }
    }
}

/// `g`'s `springs-and-rods` opened, if it is `ly:spacing-spanner::set-springs`
/// on a spanner, with the read of its columns begun; none if it goes as any
/// other read.
fn spacing_read(
    host: &mut dyn Host,
    g: Value,
) -> Result<Option<(Value, OpenRead, Spacing)>, Exception> {
    let Some(proc) = host.lily("ly:spacing-spanner::set-springs") else {
        return Ok(None);
    };
    if !spanner::is_spanner(host.heap(), g) {
        return Ok(None);
    }
    let sym = host.heap().intern("springs-and-rods");
    let Some(read) = grob::open_read(host, g, sym, proc) else {
        return Ok(None);
    };
    match Spacing::begin(host, g) {
        Ok(spacing) => Ok(Some((g, read, spacing))),
        Err(e) => {
            read.close(host, None);
            Err(e)
        }
    }
}

/// `pre_processing`'s last two scopes (`system.cc:555–571`), after the
/// pieces: the `before-line-breaking` of `system` and of each grob of its
/// `all-elements`, then their `springs-and-rods` ("before-line-breaking"
/// and "springs-and-rods"), read for what their callbacks do, between what
/// the last compile's lines hold being given back and what this one's is
/// kept (`cell.rs`, D250). A traced compile reads them apart from the
/// pieces, each read its own (`dump.rs`, D126).
pub fn read_before_breaking(host: &mut dyn Host, system: Value) -> Result<(), Exception> {
    let mut processing = Processing {
        scope: Scope::Reads,
        ..Processing::default()
    };
    while !processing.on(host, system)? {}
    Ok(())
}

/// `pre_processing`'s first three scopes (`system.cc:520–553`), which
/// make the breakable items' pieces before line breaking: each grob of
/// `system`'s `all-elements` before them breaks ("Break items",
/// [`item::break_breakable_item`]); each, from the last, the pieces
/// first, takes its original's objects and dies if not visible
/// ("Handle dependencies", [`item::handle_prebroken_dependencies`]); and
/// each moves its parents to their pieces ("Fix up refpoints",
/// [`grob::fixup_refpoint`]). [`pre_processing`] reads the next two
/// scopes after these ([`read_before_breaking`]).
pub fn make_prebroken_pieces(host: &mut dyn Host, system: Value) -> Result<(), Exception> {
    let mut processing = Processing {
        until: Some(Scope::Reads),
        ..Processing::default()
    };
    while !processing.on(host, system)? {}
    Ok(())
}

/// What `break_into_pieces` (`system.cc:449–491`) does with the columns
/// of a spaced line, `posns`, on its system `system` (`:471–474`, `:488`):
/// each moved by its position and put on `system`, then the loose ones
/// placed ([`set_loose_columns`]); LilyPond's programming error, at the
/// column, for an infinite position, which does not move it. The broken
/// system it makes for the line, its bounds, pure height and labels are
/// step 33's: `system` is the one the caller gives.
pub fn place_line(
    host: &mut dyn Host,
    system: Value,
    posns: &ColumnXPositions,
) -> Result<(), Exception> {
    for (&c, &x) in posns.cols.iter().zip(&posns.config) {
        if let Err(text) = grob::translate_axis(host.heap(), c, x, X) {
            grob::programming_error(host, c, text)?;
        }
        paper_column::set_system(host.heap(), c, system);
    }
    set_loose_columns(host, system, posns)
}

/// `break_into_pieces`: for each line of `breaking`, a copy of `system`
/// ([`clone`]) made its broken system ([`break_line`]) and then the last
/// of `system`'s broken systems. A line with no columns, where LilyPond
/// reads past their end, makes no system.
pub fn break_into_pieces(
    host: &mut dyn Host,
    system: Value,
    breaking: &[ColumnXPositions],
) -> Result<(), Exception> {
    for line in breaking {
        let (Some(&first), Some(&last)) = (line.cols.first(), line.cols.last()) else {
            continue;
        };
        let Some(copy) = clone(host, system) else {
            return Ok(());
        };
        let depth = host.heap().root_depth();
        host.heap().root(copy);
        let made = break_line(host, system, copy, line, [first, last]);
        host.heap().unroot_to(depth);
        made?;
        with_grob_mut(host.heap(), system, |g| g.broken_intos.push(copy));
    }
    Ok(())
}

/// What `break_into_pieces` makes of `copy`, a copy of `system`, for the
/// spaced `line` whose first and last columns are `ends`: ranked by
/// `system`'s broken systems before it ([`set_rank`]), typeset in
/// `system`'s Paper_score ([`paper_score::typeset_system`]; in none for
/// none, where LilyPond reads through a null), its `pure-Y-extent`
/// `system`'s pure height relative to itself between the ends' ranks,
/// bounded by the ends, its `labels` its columns' and then its loose
/// columns' ([`collect_labels`]), and the columns placed on it
/// ([`place_line`]). LilyPond reads a column's labels after moving it;
/// the engine reads them all before placing the line, which reads none.
fn break_line(
    host: &mut dyn Host,
    system: Value,
    copy: Value,
    line: &ColumnXPositions,
    ends: [Value; 2],
) -> Result<(), Exception> {
    let rank = with_grob(host.heap(), system, |g| g.broken_intos.len()).unwrap_or(0);
    set_rank(host.heap(), copy, rank as u64);
    if let Some(pscore) = with_grob(host.heap(), system, |g| g.pscore).flatten() {
        paper_score::typeset_system(host.heap(), pscore, copy);
    }

    let [st, end] = ends.map(|c| paper_column::rank(host.heap(), c) as usize);
    let iv = pure_y_extent(host, system, system, st, end)?;
    let heap = host.heap();
    let (l, r) = (heap.flonum(iv.left), heap.flonum(iv.right));
    let iv = heap.cons(l, r);
    grob::set_property(heap, copy, "pure-Y-extent", iv);

    spanner::set_bound(host, copy, LEFT, ends[0])?;
    spanner::set_bound(host, copy, RIGHT, ends[1])?;
    let cols: Vec<Value> = line.cols.iter().chain(&line.loose_cols).copied().collect();
    let system_labels = collect_labels(host, &cols)?;
    grob::set_property(host.heap(), copy, "labels", system_labels);

    place_line(host, copy, line)
}

/// `collect_labels` of each of `cols` in turn, from `'()`: each column's
/// `labels`, if a pair, appended before the labels so far (`ly_append`,
/// which copies them), so the last column's first. Every `labels` is read
/// before the list is made, which a read could otherwise collect.
fn collect_labels(host: &mut dyn Host, cols: &[Value]) -> Result<Value, Exception> {
    let mut col_labels = Vec::with_capacity(cols.len());
    for &col in cols {
        col_labels.push(grob::property(host, col, "labels")?);
    }
    let heap = host.heap();
    let mut labels = Value::NULL;
    for mut p in col_labels {
        let mut items = Vec::new();
        while let (Some(label), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            items.push(label);
            p = rest;
        }
        labels = items
            .into_iter()
            .rev()
            .fold(labels, |tail, label| heap.cons(label, tail));
    }
    Ok(labels)
}

/// `do_break_substitution_and_fixup_refpoints` (`system.cc:142–190`),
/// after `system` is broken into its broken systems
/// ([`break_into_pieces`]): each of its grobs broken
/// ([`grob::do_break_processing`], a spanner into a piece a system); the
/// grobs of each broken system, then its own, moving their parents to
/// their pieces ([`grob::fixup_refpoint`]); each of its grobs, then
/// itself, given links on its system or killed
/// ([`grob::handle_broken_dependencies`]), which gives the broken
/// systems their `all-elements`; and each broken system's grobs, the
/// items among them three times over, kept once, and its and their
/// `after-line-breaking` read. LilyPond's debug output of the count of
/// elements is not given.
pub fn do_break_substitution_and_fixup_refpoints(
    host: &mut dyn Host,
    system: Value,
) -> Result<(), Exception> {
    let mut substituting = Substituting::default();
    while !substituting.on(host, system)? {}
    Ok(())
}

/// Where a [`Substituting`] is.
#[derive(Clone, Copy, Default)]
enum Substitution {
    #[default]
    Break,
    Dependencies,
    /// Each broken system's `after-line-breaking`, then its grobs'.
    AfterSystem(usize),
    AfterGrobs(usize),
}

/// [`do_break_substitution_and_fixup_refpoints`] under way (H3, D254):
/// [`Substituting::on`] goes on until it is done or the step's time is
/// spent, between grobs.
#[derive(Default)]
pub struct Substituting {
    stage: Substitution,
    /// The next grob of the array the stage reads, and how many.
    i: usize,
    n: usize,
}

impl Substituting {
    /// Goes on; whether it is done.
    pub fn on(&mut self, host: &mut dyn Host, system: Value) -> Result<bool, Exception> {
        let sym = host.heap().intern("all-elements");
        let all = grob_array::object(host, system, sym)?;
        let elements = |host: &mut dyn Host, ga: Value| {
            host.heap()
                .foreign::<GrobArray>(ga)
                .map_or(0, |a| a.grobs.len())
        };
        // Each grob is read by its index when it is reached, since a call can
        // collect; nothing here adds to `system`'s own array.
        let at = |host: &mut dyn Host, ga: Value, i: usize| {
            host.heap()
                .foreign::<GrobArray>(ga)
                .and_then(|a| a.grobs.get(i).copied())
        };
        let children = spanner::broken_intos(host.heap(), system);
        loop {
            match self.stage {
                Substitution::Break => {
                    if self.i == 0 {
                        if let Some((arena, _)) = lily_engine_music::grob_of(host.heap(), system) {
                            crate::unpure::forget_pure_offsets(&arena);
                        }
                        self.n = elements(host, all);
                    }
                    while self.i < self.n {
                        let g = at(host, all, self.i);
                        self.i += 1;
                        if let Some(g) = g {
                            grob::do_break_processing(host, g)?;
                            crate::queue::check(Step::Substitution);
                            if due() {
                                return Ok(false);
                            }
                        }
                    }
                    // A grob's breaking made the broken systems' arrays.
                    let children = spanner::broken_intos(host.heap(), system);

                    // fixups must be done in broken line_of_scores, because
                    // new elements are put over there.
                    for &child in &children {
                        let child_elts = grob_array::object(host, child, sym)?;
                        let heap = host.heap();
                        for g in grob_array::grobs_of(heap, child_elts) {
                            grob::fixup_refpoint(heap, g);
                        }
                    }

                    // needed for doing items.
                    for i in 0..self.n {
                        if let Some(g) = at(host, all, i) {
                            grob::fixup_refpoint(host.heap(), g);
                        }
                    }
                    crate::queue::check(Step::Substitution);
                    self.stage = Substitution::Dependencies;
                    self.i = 0;
                }
                Substitution::Dependencies => {
                    while self.i < self.n {
                        let g = at(host, all, self.i);
                        self.i += 1;
                        if let Some(g) = g {
                            grob::handle_broken_dependencies(host, g)?;
                            crate::queue::check(Step::Substitution);
                            if due() {
                                return Ok(false);
                            }
                        }
                    }
                    grob::handle_broken_dependencies(host, system)?;

                    // Because the get_property (all-elements) contains items
                    // in 3 versions, handle_broken_dependencies () will leave
                    // duplicated items in all-elements. uniq makes sure that
                    // no duplicates are in the list.
                    for &child in &children {
                        let all_elts_ga = grob_array::object(host, child, sym)?;
                        let heap = host.heap();
                        let mut unique = GrobArray {
                            grobs: grob_array::grobs_of(heap, all_elts_ga),
                            ordered: false,
                        };
                        unique.remove_duplicates(heap);
                        if let Some(a) = heap.foreign_mut::<GrobArray>(all_elts_ga) {
                            a.grobs = unique.grobs;
                        }
                    }
                    // What the last compile's cells hold of the grobs whose
                    // inputs are the same, before anything is computed for a
                    // broken grob (Y7, `cell.rs`).
                    if let Some(pscore) = with_grob(host.heap(), system, |g| g.pscore).flatten() {
                        crate::queue::check(Step::Substitution);
                        crate::cell::reuse(host, pscore, system)?;
                        crate::queue::check(Step::Cells);
                    }
                    self.stage = Substitution::AfterSystem(0);
                }
                Substitution::AfterSystem(c) => {
                    let Some(&child) = children.get(c) else {
                        return Ok(true);
                    };
                    grob::property(host, child, "after-line-breaking")?;
                    self.i = 0;
                    self.stage = Substitution::AfterGrobs(c);
                }
                Substitution::AfterGrobs(c) => {
                    let all_elts_ga = grob_array::object(host, children[c], sym)?;
                    let n = elements(host, all_elts_ga);
                    while self.i < n {
                        let g = at(host, all_elts_ga, self.i);
                        self.i += 1;
                        if let Some(g) = g {
                            grob::property(host, g, "after-line-breaking")?;
                            crate::queue::check(Step::AfterLineBreaking);
                            if due() {
                                return Ok(false);
                            }
                        }
                    }
                    self.stage = Substitution::AfterSystem(c + 1);
                }
            }
        }
    }
}

/// `post_processing` (`system.cc:574–594`), before a broken system
/// becomes a paper system: `system` moved up by the top of its Y extent
/// relative to itself, LilyPond's programming error at it for an empty
/// one; then the `stencil` of `system` and of each of its live
/// `all-elements`, each once and in the order [`GrobArray::remove_duplicates`]
/// gives (LilyPond's `uniquify`), read for the fonts they load.
pub fn post_processing(host: &mut dyn Host, system: Value) -> Result<(), Exception> {
    let depth = host.heap().root_depth();
    let begun = begin_post_processing(host, system);
    let stencils = begun.and_then(|(sorted, n)| {
        (0..n).try_for_each(|k| {
            let g = host
                .heap()
                .foreign::<GrobArray>(sorted)
                .and_then(|a| a.grobs.get(k).copied());
            match g {
                Some(g) => get_stencil(host, g),
                None => Ok(()),
            }
        })
    });
    host.heap().unroot_to(depth);
    stencils
}

/// [`post_processing`] as far as its grobs: their array, rooted, and how many.
fn begin_post_processing(host: &mut dyn Host, system: Value) -> Result<(Value, usize), Exception> {
    let iv = grob::extent(host, system, system, Y)?;
    if iv.is_empty() {
        grob::programming_error(host, system, "system with empty extent")?;
    } else if let Err(text) = grob::translate_axis(host.heap(), system, -iv.right, Y) {
        grob::programming_error(host, system, text)?;
    }

    let sym = host.heap().intern("all-elements");
    let all = grob_array::object(host, system, sym)?;
    let heap = host.heap();
    let mut sorted = GrobArray {
        grobs: grob_array::grobs_of(heap, all),
        ordered: false,
    };
    sorted.remove_duplicates(heap);
    // Rooted, since a stencil's callbacks can collect.
    let n = sorted.grobs.len();
    let all_elts_sorted = heap.wrap(sorted);
    heap.root(all_elts_sorted);
    get_stencil(host, system)?;
    Ok((all_elts_sorted, n))
}

/// `Grob::get_stencil`, for what its callbacks do: a live grob's
/// `stencil` read.
fn get_stencil(host: &mut dyn Host, g: Value) -> Result<(), Exception> {
    if grob::is_live_grob(host.heap(), g) {
        grob::property(host, g, "stencil")?;
    }
    Ok(())
}

/// `get_neighboring_staff`: the last live element of the system's
/// `vertical-alignment`, from the top for `dir` up and from the bottom for
/// down, that comes before `vertical_axis_group` and spans some of
/// `bounds` (the column ranks, cut down to each element's as LilyPond
/// cuts them, live or not); none if `vertical_axis_group` is not reached
/// or the system has no alignment. A hara-kiri group is first given its
/// chance to die (`consider_suicide`).
pub fn get_neighboring_staff(
    host: &mut dyn Host,
    system: Value,
    dir: Direction,
    vertical_axis_group: Option<Value>,
    mut bounds: (u64, u64),
) -> Result<Option<Value>, Exception> {
    let sym = host.heap().intern("vertical-alignment");
    let align = grob::object(host, system, sym)?;
    if grob::with_grob(host.heap(), align, |_| ()).is_none() {
        return Ok(None);
    }

    let mut elts = grob_array::extract_grob_array(host, align, "elements")?;
    if dir != 1 {
        elts.reverse();
    }

    let mut out = None;
    for g in elts {
        if Some(g) == vertical_axis_group {
            return Ok(out);
        }

        if grob::has_interface(host.heap(), g, "hara-kiri-group-spanner-interface") {
            crate::hara_kiri_group_spanner::consider_suicide(host, g)?;
        }

        let (l, r) = item::spanned_column_rank_interval(host.heap(), g);
        bounds = (bounds.0.max(l), bounds.1.min(r));
        if grob::is_live_grob(host.heap(), g) && bounds.0 <= bounds.1 {
            out = Some(g);
        }
    }

    Ok(None)
}

/// `begin_of_line_pure_height (start, end)`: the pure height of `system`
/// at the beginning of a line from the column ranked `start` to the one
/// ranked `end`, `part_of_line_pure_height`'s (`%system-part-of-line-pure-height`).
pub fn begin_of_line_pure_height(
    host: &mut dyn Host,
    system: Value,
    start: u64,
    end: u64,
) -> Result<Interval, Exception> {
    part_of_line_pure_height(host, system, start, end, true)
}

/// `rest_of_line_pure_height (start, end)`: the pure height of `system`
/// in the rest of a line from the column ranked `start` to the one ranked
/// `end`.
pub fn rest_of_line_pure_height(
    host: &mut dyn Host,
    system: Value,
    start: u64,
    end: u64,
) -> Result<Interval, Exception> {
    part_of_line_pure_height(host, system, start, end, false)
}

fn part_of_line_pure_height(
    host: &mut dyn Host,
    system: Value,
    start: u64,
    end: u64,
    begin: bool,
) -> Result<Interval, Exception> {
    let (s, e) = (rank_value(host.heap(), start), rank_value(host.heap(), end));
    let iv = crate::call_lily(
        host,
        "%system-part-of-line-pure-height",
        &[system, s, e, Value::bool(begin)],
    )?;
    Ok(interval_of(host.heap(), iv))
}

/// `pure_refpoint_extent (start, end)`: from the pure minimum translation
/// of the last spaceable staff of `system`'s `vertical-alignment` (`DOWN`)
/// to that of its first (`UP`), between the columns ranked `start` and
/// `end`; an end with no spaceable staff stays empty, and the whole is
/// empty for no alignment.
pub fn pure_refpoint_extent(
    host: &mut dyn Host,
    system: Value,
    start: u64,
    end: u64,
) -> Result<Interval, Exception> {
    let mut ret = Interval::EMPTY;
    let sym = host.heap().intern("vertical-alignment");
    let alignment = grob::object(host, system, sym)?;
    if grob::with_grob(host.heap(), alignment, |_| ()).is_none() {
        return Ok(Interval::EMPTY);
    }

    let staves = grob_array::extract_grob_array(host, alignment, "elements")?;
    let list = host.heap().list(&staves);
    let (s, e) = (rank_value(host.heap(), start), rank_value(host.heap(), end));
    let axis = Value::fixnum(Y as i64).unwrap_or(Value::FALSE);
    let offsets = crate::call_lily(
        host,
        "%align-pure-minimum-translations",
        &[alignment, list, axis, s, e],
    )?;
    let offsets = reals(host.heap(), offsets);

    for (i, &offset) in offsets.iter().enumerate() {
        if page_layout_problem::is_spaceable(host, staves[i])? {
            *ret.at_mut(UP) = offset;
            break;
        }
    }

    for (i, &offset) in offsets.iter().enumerate().rev() {
        if page_layout_problem::is_spaceable(host, staves[i])? {
            *ret.at_mut(DOWN) = offset;
            break;
        }
    }

    Ok(ret)
}

/// `get_footnote_heights_in_range (start, end)`: the heights of the
/// footnotes' texts of `system` between the columns ranked `start` and
/// `end` that go to the bottom of the page (`footnote` `#t`).
pub fn get_footnote_heights_in_range(
    host: &mut dyn Host,
    system: Value,
    start: u64,
    end: u64,
) -> Result<Vec<f64>, Exception> {
    note_heights_in_range(host, system, start, end, true)
}

/// `get_in_note_heights_in_range (start, end)`: those of the in-notes,
/// the footnotes that are no footnote (`footnote` not `#t`).
pub fn get_in_note_heights_in_range(
    host: &mut dyn Host,
    system: Value,
    start: u64,
    end: u64,
) -> Result<Vec<f64>, Exception> {
    note_heights_in_range(host, system, start, end, false)
}

/// `internal_get_note_heights_in_range` (`%system-note-heights-in-range`).
fn note_heights_in_range(
    host: &mut dyn Host,
    system: Value,
    start: u64,
    end: u64,
    foot: bool,
) -> Result<Vec<f64>, Exception> {
    let (s, e) = (rank_value(host.heap(), start), rank_value(host.heap(), end));
    let heights = crate::call_lily(
        host,
        "%system-note-heights-in-range",
        &[system, s, e, Value::bool(foot)],
    )?;
    Ok(reals(host.heap(), heights))
}

/// A column's rank as Scheme gives it.
fn rank_value(heap: &mut Heap, rank: u64) -> Value {
    heap.integer(i64::try_from(rank).unwrap_or(i64::MAX))
}

/// The interval of a pair of reals, empty for what is none.
fn interval_of(heap: &Heap, v: Value) -> Interval {
    let end = |e: Option<Value>| e.and_then(|e| heap.to_f64(e));
    match (end(heap.car(v)), end(heap.cdr(v))) {
        (Some(left), Some(right)) => Interval::new(left, right),
        _ => Interval::EMPTY,
    }
}

/// The reals of the list `list`.
fn reals(heap: &Heap, mut list: Value) -> Vec<f64> {
    let mut out = Vec::new();
    while let (Some(x), Some(rest)) = (heap.car(list), heap.cdr(list)) {
        out.extend(heap.to_f64(x));
        list = rest;
    }
    out
}

/// `get_broken_system_grobs` (`system.cc:206–213`): the list of
/// `system`'s broken systems, in order.
pub fn get_broken_system_grobs(heap: &mut Heap, system: Value) -> Value {
    let broken = spanner::broken_intos(heap, system);
    heap.list(&broken)
}

/// `get_paper_systems` (`system.cc:215–231`): a vector of the paper
/// system of each of `system`'s broken systems ([`get_paper_system`]), in
/// order. LilyPond's debug output of their numbers is not given.
pub fn get_paper_systems(host: &mut dyn Host, system: Value) -> Result<Value, Exception> {
    let depth = host.heap().root_depth();
    host.heap().root(system);
    let mut lines = Vec::new();
    let mut made = Ok(());
    for child in spanner::broken_intos(host.heap(), system) {
        match get_paper_system(host, child) {
            Ok(pl) => {
                host.heap().root(pl);
                lines.push(pl);
            }
            Err(e) => {
                made = Err(e);
                break;
            }
        }
    }
    let lines = made.map(|()| host.heap().vector(lines));
    host.heap().unroot_to(depth);
    lines
}

/// `get_paper_system` (`system.cc:611–721`): the broken system `system`
/// as a paper system for page layout, after [`post_processing`]. Its
/// stencil ([`Placing`]); its `line-break-system-details`
/// those of its left bound, the paper system's immutable properties
/// ([`make_paper_system`], [`paper_system_set_stencil`]); its
/// `vertical-skylines`; the page-break and page-turn permissions and
/// penalties of its right bound; `last-in-score` if that bound is a piece
/// of its original's right bound; the extent of the Y coordinates of its
/// `vertical-alignment`'s live spaceable staves (`staff-refpoint-extent`);
/// and `system` itself (`system-grob`). A system without a bound, or one
/// with no original, where LilyPond reads through a null, has `'()` for
/// what the bound gives and is not last in the score.
pub fn get_paper_system(host: &mut dyn Host, system: Value) -> Result<Value, Exception> {
    let mut paper = PaperSystem::begin(host, system);
    loop {
        if let Some(made) = paper.on(host, &|| false)? {
            return Ok(made);
        }
    }
}

/// [`get_paper_system`] under way (B23, D261): a system's stencils are one
/// grob's after another's, with no reason to be one step, and what it makes
/// is rooted until it is done.
pub struct PaperSystem {
    depth: usize,
    system: Value,
    stage: Drawn,
}

/// What a [`PaperSystem`] is doing.
enum Drawn {
    Begun,
    /// [`post_processing`]'s grobs, each stencil read.
    Stencils {
        sorted: Value,
        i: usize,
        n: usize,
    },
    /// The system's stencil, each grob's placed.
    Placing(Placing),
    Done,
}

impl PaperSystem {
    /// The paper system of `system` begun: nothing done yet.
    pub fn begin(host: &mut dyn Host, system: Value) -> PaperSystem {
        let depth = host.heap().root_depth();
        host.heap().root(system);
        PaperSystem {
            depth,
            system,
            stage: Drawn::Begun,
        }
    }

    /// Goes on until `stop` says so, between grobs: the paper system, once it
    /// is made; rooted no more, as what it made is not.
    pub fn on(
        &mut self,
        host: &mut dyn Host,
        stop: &dyn Fn() -> bool,
    ) -> Result<Option<Value>, Exception> {
        let made = self.go_on(host, stop);
        if !matches!(made, Ok(None)) {
            host.heap().unroot_to(self.depth);
        }
        made
    }

    fn go_on(
        &mut self,
        host: &mut dyn Host,
        stop: &dyn Fn() -> bool,
    ) -> Result<Option<Value>, Exception> {
        let system = self.system;
        loop {
            match &mut self.stage {
                Drawn::Begun => {
                    let (sorted, n) = begin_post_processing(host, system)?;
                    self.stage = Drawn::Stencils { sorted, i: 0, n };
                }
                Drawn::Stencils { sorted, i, n } => {
                    if *i >= *n {
                        self.stage = Drawn::Placing(Placing::begin(host, system)?);
                        continue;
                    }
                    let k = *i;
                    *i += 1;
                    let sorted = *sorted;
                    let g = host
                        .heap()
                        .foreign::<GrobArray>(sorted)
                        .and_then(|a| a.grobs.get(k).copied());
                    if let Some(g) = g {
                        get_stencil(host, g)?;
                        crate::queue::check(Step::Stencil);
                    }
                }
                Drawn::Placing(placing) => {
                    if !placing.done() {
                        placing.next(host, system)?;
                        crate::queue::check(Step::Stencil);
                    } else {
                        let Drawn::Placing(placing) =
                            core::mem::replace(&mut self.stage, Drawn::Done)
                        else {
                            continue;
                        };
                        let stencil = placing.end(host, system)?;
                        return paper_system_of(host, system, stencil).map(Some);
                    }
                }
                Drawn::Done => return Ok(None),
            }
            if stop() {
                return Ok(None);
            }
        }
    }
}

/// [`get_paper_system`]'s work after its stencil, with `system` rooted; what
/// it makes is rooted until the caller unroots it.
fn paper_system_of(
    host: &mut dyn Host,
    system: Value,
    sys_stencil: Stencil,
) -> Result<Value, Exception> {
    host.heap().root(sys_stencil.expr);

    let left_bound = spanner::get_bound(host, system, LEFT)?;
    let prop_init = match left_bound {
        Some(b) => grob::property(host, b, "line-break-system-details")?,
        None => Value::NULL,
    };
    let pl = make_paper_system(host.heap(), prop_init);
    host.heap().root(pl);
    paper_system_set_stencil(host.heap(), pl, sys_stencil);

    // information that the page breaker might need
    let right_bound = spanner::get_bound(host, system, RIGHT)?;
    let skylines = grob::property(host, system, "vertical-skylines")?;
    set_prob(host.heap(), pl, "vertical-skylines", skylines);
    for name in [
        "page-break-permission",
        "page-turn-permission",
        "page-break-penalty",
        "page-turn-penalty",
    ] {
        let v = match right_bound {
            Some(b) => grob::property(host, b, name)?,
            None => Value::NULL,
        };
        set_prob(host.heap(), pl, name, v);
    }

    let original = with_grob(host.heap(), system, |g| g.original).flatten();
    if let (Some(original), Some(right_bound)) = (original, right_bound) {
        let last = spanner::get_bound(host, original, RIGHT)?;
        let piece_of = with_grob(host.heap(), right_bound, |g| g.original).flatten();
        if piece_of == last {
            set_prob(host.heap(), pl, "last-in-score", Value::TRUE);
        }
    }

    let mut staff_refpoints = Interval::EMPTY;
    let sym = host.heap().intern("vertical-alignment");
    let align = grob_array::object(host, system, sym)?;
    if with_grob(host.heap(), align, |_| ()).is_some() {
        let staves = grob_array::extract_grob_array(host, align, "elements")?;
        for staff in staves {
            if grob::is_live_grob(host.heap(), staff)
                && page_layout_problem::is_spaceable(host, staff)?
            {
                staff_refpoints.add_point(grob::relative_coordinate(host, staff, system, Y)?);
            }
        }
    }

    let heap = host.heap();
    let (l, r) = (
        heap.flonum(staff_refpoints.left),
        heap.flonum(staff_refpoints.right),
    );
    let iv = heap.cons(l, r);
    set_prob(heap, pl, "staff-refpoint-extent", iv);
    set_prob(heap, pl, "system-grob", system);
    Ok(pl)
}

/// The stencil of the paper system of `system` (`system.cc:617–683`): each
/// grob of its `all-elements` in the order libc++'s `std::sort` leaves
/// them by `layer` (an `int`, 1 for none), its print stencil
/// ([`grob::get_print_stencil`]), if it has an expression, moved by its
/// coordinates relative to `system` and its `extra-offset` in staff
/// spaces; the system's own print stencil first; all combined
/// (`combine-stencil`) in a box of `system`'s extents united with theirs.
/// The expressions are rooted until the caller's next safe point.
struct Placing {
    entries: Vec<(Value, i32)>,
    i: usize,
    exprs: Vec<Value>,
    stencil_bbox: Box,
}

impl Placing {
    /// The grobs of `system`, by their layers.
    fn begin(host: &mut dyn Host, system: Value) -> Result<Placing, Exception> {
        let sym = host.heap().intern("all-elements");
        let all = grob_array::object(host, system, sym)?;
        let all_elts = grob_array::grobs_of(host.heap(), all);
        let mut entries = Vec::with_capacity(all_elts.len());
        for g in all_elts {
            let layer = grob::property(host, g, "layer")?;
            let layer = layer
                .as_fixnum()
                .and_then(|n| i32::try_from(n).ok())
                .unwrap_or(1);
            entries.push((g, layer));
        }
        lily_engine_skyline::sort(&mut entries, &|a: &(Value, i32), b: &(Value, i32)| {
            a.1 < b.1
        });
        Ok(Placing {
            entries,
            i: 0,
            exprs: Vec::new(),
            stencil_bbox: Box::EMPTY,
        })
    }

    fn done(&self) -> bool {
        self.i >= self.entries.len()
    }

    /// The next grob's stencil placed in the system.
    fn next(&mut self, host: &mut dyn Host, system: Value) -> Result<(), Exception> {
        let (g, _) = self.entries[self.i];
        self.i += 1;
        let mut st = grob::get_print_stencil(host, g)?;
        if st.expr == Value::NULL {
            return Ok(());
        }
        host.heap().root(st.expr);
        let ox = grob::relative_coordinate(host, g, system, X)?;
        let oy = grob::relative_coordinate(host, g, system, Y)?;
        let extra = grob::property(host, g, "extra-offset")?;
        let extra = grob::number_pair(host.heap(), extra).map_or((0.0, 0.0), |i| (i.left, i.right));
        let space = crate::call_lily(host, "%staff-space", &[g])?;
        let space = host.heap().to_f64(space).unwrap_or(1.0);
        let extra = (extra.0 * space, extra.1 * space);

        // Must copy the stencil, for we cannot change the stencil cached
        // in G.
        st.translate(host.heap(), (ox + extra.0, oy + extra.1));
        host.heap().root(st.expr);

        // Accumulate the actual drawn stencil extents
        for a in [X, Y] {
            self.stencil_bbox.0[a].unite(st.extent(a));
        }
        self.exprs.push(st.expr);
        Ok(())
    }

    /// The system's stencil, of its own and those placed.
    fn end(mut self, host: &mut dyn Host, system: Value) -> Result<Stencil, Exception> {
        let me = grob::get_print_stencil(host, system)?;
        if me.expr != Value::NULL {
            host.heap().root(me.expr);
            self.exprs.insert(0, me.expr);
            // Unite with the system's own stencil extent
            for a in [X, Y] {
                self.stencil_bbox.0[a].unite(me.extent(a));
            }
        }

        // Start with the System's extent (used for layout calculations), and
        // extend it to include all drawn stencil content.
        let mut x = grob::extent(host, system, system, X)?;
        let mut y = grob::extent(host, system, system, Y)?;
        if !self.stencil_bbox.0[X].is_empty() {
            x.unite(self.stencil_bbox.0[X]);
        }
        if !self.stencil_bbox.0[Y].is_empty() {
            y.unite(self.stencil_bbox.0[Y]);
        }

        let heap = host.heap();
        let head = heap.intern("combine-stencil");
        let tail = heap.list(&self.exprs);
        let expr = heap.cons(head, tail);
        Ok(Stencil::new(Box::new(x, y), expr))
    }
}

/// `make_paper_system` (`paper-system.cc:24–29`): a prob of the type
/// `paper-system` whose immutable properties are `immutable_init`.
pub fn make_paper_system(heap: &mut Heap, immutable_init: Value) -> Value {
    let ty = heap.intern("paper-system");
    lily_engine_music::make_prob(heap, ty, immutable_init)
}

/// `(ly:paper-system? x)` (`paper-system-scheme.cc:25–31`): whether `x`
/// is a prob of the type `paper-system` (`ly:prob-type?`).
pub(crate) fn is_paper_system(cx: &mut Cx, args: &[Value]) -> R {
    let ty = cx.heap_mut().intern("paper-system");
    let prob = cx.heap().foreign::<lily_engine_music::Prob>(args[0]);
    Ok(Value::bool(prob.is_some_and(|p| p.type_ == ty)))
}

/// `paper_system_set_stencil` (`paper-system.cc:80–94`): `s` set as the
/// `stencil` of the paper system `prob`, its Y extent the paper system's
/// `Y-extent` if that is a number pair.
pub fn paper_system_set_stencil(heap: &mut Heap, prob: Value, mut s: Stencil) {
    let sym = heap.intern("Y-extent");
    let yext = lily_engine_music::prob_get(heap, prob, sym);
    if let Some(iv) = grob::number_pair(heap, yext) {
        s.dim.0[Y] = iv;
    }
    let s = heap.wrap(s);
    set_prob(heap, prob, "stencil", s);
}

/// `set_property` of the prob `prob`'s property `name`.
fn set_prob(heap: &mut Heap, prob: Value, name: &str, v: Value) {
    let sym = heap.intern(name);
    lily_engine_music::prob_set(heap, prob, sym, v);
}

/// `get_rank`: the system's place among the broken systems, 0 for what
/// is no grob and until [`set_rank`].
pub fn get_rank(heap: &Heap, system: Value) -> u64 {
    with_grob(heap, system, |g| g.system_rank).unwrap_or(0)
}

/// `rank_ = …`, as `break_into_pieces` numbers the broken systems.
pub fn set_rank(heap: &Heap, system: Value, rank: u64) {
    with_grob_mut(heap, system, |g| g.system_rank = rank);
}

/// `broken_col_range`: the breakable paper columns of `system`'s
/// `columns` strictly between the columns of `left_item` and
/// `right_item`, by their ranks, that are on no system yet. An item
/// without a column, which LilyPond reads through, has none.
pub fn broken_col_range(
    host: &mut dyn Host,
    system: Value,
    left_item: Value,
    right_item: Value,
) -> Result<Vec<Value>, Exception> {
    let heap = host.heap();
    let (Some(left_col), Some(right_col)) = (
        item::get_column(heap, left_item),
        item::get_column(heap, right_item),
    ) else {
        return Ok(Vec::new());
    };
    let (left_rank, right_rank) = (
        paper_column::rank(heap, left_col),
        paper_column::rank(heap, right_col),
    );
    let cols = grob_array::extract_grob_array(host, system, "columns")?;
    let end_rank = usize::try_from(right_rank).map_or(cols.len(), |r| r.min(cols.len()));
    // A rank of VPOS wraps to 0 when one is added, as LilyPond's vsize does.
    let start = usize::try_from(left_rank.wrapping_add(1)).unwrap_or(usize::MAX);
    let mut ret = Vec::new();
    for &c in cols.iter().take(end_rank).skip(start) {
        if is_paper_column(host.heap(), c)
            && paper_column::is_breakable(host, c)?
            && paper_column::system(host.heap(), c).is_none()
        {
            ret.push(c);
        }
    }
    Ok(ret)
}

/// `(%system? g)`: whether `g` is a `System`, as `dynamic_cast<System *>`
/// tells.
pub(crate) fn is_system(cx: &mut Cx, args: &[Value]) -> R {
    Ok(Value::bool(
        item::class(cx.heap_mut(), args[0]) == Some(Class::System),
    ))
}

/// `(%system-rank sys)`: [`get_rank`], which page layout reads for the
/// line's indent (`page_layout_problem.rs`).
pub(crate) fn rank_prim(cx: &mut Cx, args: &[Value]) -> R {
    let rank = get_rank(cx.heap(), args[0]);
    Ok(cx.heap_mut().integer(rank as i64))
}

/// `get_root_system`: the grob at the top of `me`'s Y parents, if it is a
/// system.
pub fn get_root_system(heap: &mut Heap, me: Value) -> Option<Value> {
    let mut system_grob = me;
    while let Some(parent) = grob::get_parent(heap, system_grob, Y) {
        system_grob = parent;
    }
    (item::class(heap, system_grob) == Some(Class::System)).then_some(system_grob)
}

/// `System::used_columns_in_range`: the used paper columns of `system`'s
/// `columns` from `start` up to `end`, not past its last breakable
/// column; none if it has no breakable column (LilyPond's `end` wraps to
/// 0).
pub fn used_columns_in_range(
    host: &mut dyn Host,
    system: Value,
    start: usize,
    end: usize,
) -> Result<Vec<Value>, Exception> {
    let ro_columns = grob_array::extract_grob_array(host, system, "columns")?;
    let mut last_breakable = None;
    for (i, &c) in ro_columns.iter().enumerate().rev() {
        if is_paper_column(host.heap(), c) && paper_column::is_breakable(host, c)? {
            last_breakable = Some(i);
            break;
        }
    }
    let end = last_breakable.map_or(0, |last| end.min(last + 1));

    let mut columns = Vec::new();
    for &c in ro_columns.iter().take(end).skip(start) {
        if is_paper_column(host.heap(), c) && paper_column::is_used(host, c)? {
            columns.push(c);
        }
    }
    Ok(columns)
}

/// `System::column`: the column of rank `which` of `system`, if there is
/// one and it is a paper column.
pub fn column(host: &mut dyn Host, system: Value, which: u64) -> Result<Option<Value>, Exception> {
    let columns = grob_array::extract_grob_array(host, system, "columns")?;
    let c = usize::try_from(which)
        .ok()
        .and_then(|i| columns.get(i).copied());
    Ok(c.filter(|&c| is_paper_column(host.heap(), c)))
}

/// `dynamic_cast<Paper_column *>`: whether `g` is a paper column.
fn is_paper_column(heap: &mut Heap, g: Value) -> bool {
    item::class(heap, g) == Some(Class::PaperColumn)
}

/// What `%host-call` runs for [`PRELUDE`] (D127): libc++'s `std::sort` of
/// a list by a Scheme `less` (`lily_engine_skyline::sort`), which
/// `footnotes_after_line_breaking` sorts with, with the VM as its host.
pub(crate) const HOSTS: [HostHandler; 1] = [HostHandler {
    what: "std-sort",
    f: host_sort,
}];

/// `(%host-call 'std-sort lst less)`: `lst`'s items as `std::sort` leaves
/// them, ties in its order, `less` called as it compares. An error in
/// `less` ends the sort there, as LilyPond's unwinds out of it.
fn host_sort(vm: &mut Vm, args: &[Value]) -> Result<Value, Exception> {
    let (lst, less) = (args[1], args[2]);
    let heap = vm.heap_mut();
    let mut items = Vec::new();
    let mut p = lst;
    while let (Some(x), Some(rest)) = (heap.car(p), heap.cdr(p)) {
        items.push(x);
        p = rest;
    }
    let depth = heap.root_depth();
    heap.root(lst);
    heap.root(less);
    let failed = RefCell::new(None);
    let host = RefCell::new(&mut *vm);
    lily_engine_skyline::sort(&mut items, &|a: &Value, b: &Value| {
        if failed.borrow().is_some() {
            return false;
        }
        match host.borrow_mut().call(less, &[*a, *b]) {
            Ok(v) => v != Value::FALSE,
            Err(e) => {
                *failed.borrow_mut() = Some(e);
                false
            }
        }
    });
    vm.heap_mut().unroot_to(depth);
    match failed.into_inner() {
        Some(e) => Err(e),
        None => Ok(vm.heap_mut().list(&items)),
    }
}

/// `system.cc`'s callbacks, the music crate's placeholders until now, and
/// its staves:
///
/// - `ly:system::height` (`:969–974`): `Axis_group_interface::height`'s
///   (`ly:axis-group-interface::height`).
/// - `ly:system::calc-pure-height me start end` (`:976–990`): the union
///   of `begin_of_line_pure_height` and `rest_of_line_pure_height`
///   (`:925–935`), `part_of_line_pure_height`'s (`:892–923`,
///   `%system-part-of-line-pure-height`): for no `vertical-alignment`,
///   empty; else the union of the alignment's `elements`' pure heights
///   at the beginning of a line (from `start` to `start + 1`) or in the
///   rest of it, each moved by its pure minimum translation
///   (`%align-pure-minimum-translations`), and the system's own.
///   `start` and `end` as `from_scm<vsize>` converts them; what is no
///   system then is Guile's type error, where LilyPond reads through a
///   null.
/// - `ly:system::calc-pure-relevant-grobs` (`:940–967`): the system's
///   `elements` that are no axis group and no broken piece of an item.
/// - `ly:system::footnotes-before-line-breaking` (`:382–397`): its
///   `all-elements` with `footnote-interface`.
/// - `ly:system::footnotes-after-line-breaking` (`:399–414`): the
///   footnotes of its columns (`get_footnote_grobs_in_range`, `:233–301`,
///   `%system-footnote-grobs-in-range`) sorted by `grob_2D_less`
///   (`:352–380`, `%grob-2d-less?`) with libc++'s `std::sort`
///   ([`HOSTS`]).
/// - `%system-note-heights-in-range sys start end foot`,
///   `internal_get_note_heights_in_range` (`:315–343`): of the footnotes
///   of `get_footnote_grobs_in_range`, those whose `footnote` is `#t`
///   (`foot`) or those whose is not, each whose `footnote-text` is a
///   markup interpreted with the system's layout and its page properties
///   (`layout-extract-page-properties`), its height.
/// - `ly:system::vertical-skyline-elements` (`:416–446`): its
///   `elements` that are system-start delimiters, then the hara-kiri
///   groups of its `vertical-alignment`'s `elements`.
/// - `ly:system::get-vertical-alignment` (`:806–830`): the one of its
///   `elements` with `align-interface`, the last if several, with
///   LilyPond's programming errors for several and for none (`'()`).
/// - `ly:system::get-staves`, `::get-spaceable-staves` and
///   `::get-nonspaceable-staves` (`:1022–1072`): the live staves of the
///   object `vertical_alignment`, as LilyPond 2.26.0 spells it, which no
///   grob has, so `'()`; what is no system is Guile's type error, where
///   LilyPond reads through a null.
///
/// Each `LY_ASSERT_SMOB` is `%check-type`'s with LilyPond's mangled name.
pub(crate) const PRELUDE: &str = r#"
(define-public (ly:system::height smob)
  (ly:axis-group-interface::height smob))
(define (%interval-translated iv x) (cons (+ (car iv) x) (+ (cdr iv) x)))
(define (%system-part-of-line-pure-height me start end begin)
  (let ((alignment (ly:grob-object me 'vertical-alignment))
        (height (lambda (g)
                  (if begin
                      (%part-of-line-pure-height g #t start (1+ start))
                      (%part-of-line-pure-height g #f start end)))))
    (if (not (ly:grob? alignment))
        '(+inf.0 . -inf.0)
        (let* ((staves (%grob-set alignment 'elements))
               (offsets (%align-pure-minimum-translations alignment staves 1 start end)))
          (let loop ((staves staves) (offsets offsets) (ret '(+inf.0 . -inf.0)))
            (if (pair? staves)
                (let ((iv (height (car staves))))
                  (loop (cdr staves)
                        (if (pair? offsets) (cdr offsets) '())
                        (%interval-unite ret (if (pair? offsets)
                                                 (%interval-translated iv (car offsets))
                                                 iv))))
                (%interval-unite ret (height me))))))))
(define-public (ly:system::calc-pure-height smob start-scm end-scm)
  (let ((start (%scm->vsize start-scm))
        (end (%scm->vsize end-scm)))
    (%check-type "ly:calc-pure-height" 1 %system? "System" smob)
    (let* ((begin (%system-part-of-line-pure-height smob start end #t))
           (rest (%system-part-of-line-pure-height smob start end #f)))
      (%interval-unite begin rest))))
(define-public (ly:system::calc-pure-relevant-grobs smob)
  (%check-type "ly:calc-pure-relevant-grobs" 1 ly:grob? "Grob" smob)
  (ly:grob-list->grob-array
   (filter (lambda (g)
             (not (or (%grob-has-interface? g 'axis-group-interface)
                      (and (ly:item? g) (not (eq? (ly:grob-original g) g))))))
           (%grob-set smob 'elements))))
(define-public (ly:system::footnotes-before-line-breaking smob)
  (%check-type "ly:footnotes-before-line-breaking" 1 ly:grob? "Grob" smob)
  (ly:grob-list->grob-array
   (filter (lambda (g) (%grob-has-interface? g 'footnote-interface))
           (%grob-set smob 'all-elements))))
(define (%direction-or v fallback)
  (if (and (exact-integer? v) (<= -1 v 1)) v fallback))
(define (%system-footnote-grobs-in-range sys start end)
  (let loop ((gs (%grob-set sys 'footnotes-before-line-breaking)) (out '()))
    (if (null? gs)
        (reverse out)
        (let* ((g (car gs))
               (at-bat+pos
                (if (ly:spanner? g)
                    (let* ((placement (%direction-or (ly:grob-property g 'spanner-placement) -1))
                           (placement (if (= placement 0) -1 placement))
                           (sri (ly:grob-spanned-column-rank-interval g))
                           (orig (ly:grob-original g)))
                      (if (not (eq? orig g))
                          (let* ((pieces (ly:spanner-broken-into orig))
                                 (at-bat (if (= placement -1) (car pieces) (car (last-pair pieces)))))
                            (cons at-bat (cdr (ly:grob-spanned-column-rank-interval at-bat))))
                          (cons g (if (= placement -1) (car sri) (cdr sri)))))
                    (cons g (car (ly:grob-spanned-column-rank-interval g)))))
               (at-bat (car at-bat+pos))
               (pos (cdr at-bat+pos))
               (item (ly:item? at-bat))
               (dir (if item (ly:item-break-dir at-bat) 0))
               (end-of-line-visible (or (not item) (= dir -1)))
               (skip (or (and item
                              (or (not (%item-break-visible? at-bat))
                                  (and (= pos start) (not (= dir 1)))
                                  (and (= pos end) (not (= dir -1)))
                                  (and (not (= pos end)) (not (= pos start)) (not (= dir 0)))))
                         (< pos start)
                         (> pos end)
                         (and (= pos start) end-of-line-visible)
                         (and (= pos end) (not end-of-line-visible))
                         (not (%grob-live? at-bat))
                         (memq at-bat out))))
          (loop (cdr gs) (if skip out (cons at-bat out)))))))
(define (%grob-2d-key g)
  (let ((sri (car (ly:grob-spanned-column-rank-interval g))))
    (if (ly:spanner? g)
        (let* ((pieces (ly:spanner-broken-into g))
               (s (cond ((null? pieces) g)
                        ((= (%direction-or (ly:grob-property (car pieces) 'spanner-placement) 0) -1)
                         (car pieces))
                        (else (car (last-pair pieces))))))
          (cons (if (> (%real-or (ly:grob-property s 'X-offset) 0.0) 0)
                    (cdr (ly:grob-spanned-column-rank-interval s))
                    sri)
                s))
        (cons sri g))))
(define (%grob-2d-less? g1 g2)
  (let* ((k1 (%grob-2d-key g1))
         (k2 (%grob-2d-key g2)))
    (if (= (car k1) (car k2))
        (ly:grob-vertical<? (cdr k1) (cdr k2))
        (< (car k1) (car k2)))))
(define (%system-note-heights-in-range sys start end foot)
  (let ((layout (ly:grob-layout sys)))
    (let loop ((gs (%system-footnote-grobs-in-range sys start end)) (out '()))
      (if (null? gs)
          (reverse out)
          (let* ((g (car gs))
                 (kept (eq? foot (eq? #t (ly:grob-property g 'footnote))))
                 (markup (ly:grob-property g 'footnote-text)))
            (if (and kept (or (string? markup) (%markup? markup)))
                (let* ((st (%interpret-markup layout (%layout-extract-page-properties layout) markup))
                       (iv (ly:stencil-extent st 1)))
                  (loop (cdr gs) (cons (if (<= (cdr iv) (car iv)) 0.0 (- (cdr iv) (car iv))) out)))
                (loop (cdr gs) out)))))))
(define-public (ly:system::footnotes-after-line-breaking smob)
  (%check-type "ly:footnotes-after-line-breaking" 1 %system? "System" smob)
  (let ((sri (ly:grob-spanned-column-rank-interval smob)))
    (ly:grob-list->grob-array
     (%host-call 'std-sort
                 (%system-footnote-grobs-in-range smob (car sri) (cdr sri))
                 %grob-2d-less?))))
(define-public (ly:system::vertical-skyline-elements smob)
  (%check-type "ly:vertical-skyline-elements" 1 %system? "System" smob)
  (let ((delimiters (filter (lambda (g) (%grob-has-interface? g 'system-start-delimiter-interface))
                            (%grob-set smob 'elements)))
        (align (ly:grob-object smob 'vertical-alignment)))
    (ly:grob-list->grob-array
     (if (ly:grob? align)
         (append delimiters
                 (filter (lambda (g) (%grob-has-interface? g 'hara-kiri-group-spanner-interface))
                         (%grob-set align 'elements)))
         delimiters))))
(define-public (ly:system::get-vertical-alignment smob)
  (%check-type "ly:get-vertical-alignment" 1 ly:grob? "Grob" smob)
  (let loop ((elts (%grob-set smob 'elements)) (ret #f))
    (cond ((pair? elts)
           (if (%grob-has-interface? (car elts) 'align-interface)
               (begin
                 (if ret
                     (%grob-programming-error
                      smob "found multiple vertical alignments in this system"))
                 (loop (cdr elts) (car elts)))
               (loop (cdr elts) ret)))
          (ret ret)
          (else
           (%grob-programming-error smob "didn't find a vertical alignment in this system")
           '()))))
(define (%system-staves subr smob keep?)
  (%check-type subr 1 %system? "System" smob)
  (let ((align (ly:grob-object smob 'vertical_alignment)))
    (if (ly:grob? align)
        (filter (lambda (s)
                  (let ((spaceable (%spaceable? s)))
                    (and (%grob-live? s) (keep? spaceable))))
                (%grob-set align 'elements))
        '())))
(define-public (ly:system::get-staves smob)
  (%system-staves "ly:get-staves" smob (lambda (spaceable) #t)))
(define-public (ly:system::get-spaceable-staves smob)
  (%system-staves "ly:get-spaceable-staves" smob (lambda (spaceable) spaceable)))
(define-public (ly:system::get-nonspaceable-staves smob)
  (%system-staves "ly:get-nonspaceable-staves" smob not))
"#;
