//! Y7's cells (05 §6.1–6.3, MS6 step 8, D248): what layout keeps of one
//! compile for the next, and gives a grob back when what made it is the same.
//!
//! - **A cell** is one line (a `VerticalAxisGroup`'s piece) in one system: the
//!   grobs of a broken system whose ancestors on Y reach that piece. The rest
//!   of the system (its columns, `SpanBar`s, marks) is laid out every time, as
//!   are the staff distances, which read what the cells hold.
//! - **What a cell holds** is what was computed for its grobs after the line
//!   breaks: each property written since ([`Tracker::wrote`]), what a value of
//!   no grob is ([`portable`]), and each offset and extent. Before breaking
//!   nothing is reused yet (OPEN B17). The results are given back where
//!   `do_break_substitution_and_fixup_refpoints` ends, before any
//!   `after-line-breaking`, and the code after it runs as it does in a fresh
//!   compile and finds them: a value not given back is computed when read.
//! - **Its key** ([`Now::key`]) is the output definition, the line, its
//!   system's columns (each one's place among them, record and X) and each
//!   grob's digest from N8 (`GrobDigest`), in order.
//! - **What it read** is not guessed: while a callback runs for a grob
//!   ([`enter`]), each grob it reads adds to what that grob depends on
//!   ([`Dep`]): a range of each line's grobs in moment order, what the grob
//!   read depends on with them, and what a callback that writes to a grob
//!   depends on goes to that grob. A cell is reused only where those ranges
//!   digest as they did ([`Line::sum`]), each system they reach having its
//!   columns as it had ([`SYSTEMS`]). So a beam broken across a line break
//!   takes the notes of both lines into each piece's cell.
//! - **Not reused:** a cell whose callbacks read where a line is placed
//!   ([`READS_Y`], a cross-staff grob's), ran a procedure of the score's own
//!   ([`IMPURE`]), or wrote outside their line or were written to from outside
//!   it ([`COUPLED`]); and every cell after a compile whose pages said
//!   something, which a cell left out would not say again.
//! - **What a whole line's grobs are** (a staff symbol, its ledger lines, the
//!   axis group: the hubs) stands apart from the ranges (D260): a hub sits at
//!   its line's start, so a range to it would hold all the line before the
//!   reader. What is read of one is a line's [`Line::hubs`]; of an axis
//!   group or its ledger lines, whose values all the line's grobs make, by
//!   what is not a column's skylines (OPEN B25), the whole line with them
//!   ([`Range::content`]).
//! - **A mark** (a bar number, a tempo mark) belongs to no line until its
//!   `after-line-breaking` moves it to the extremal staff, which is after
//!   [`reuse`]. A cell holds the marks the last compile moved to its line
//!   ([`Cells::marks`]), last among its grobs, and takes what its group writes
//!   to them as its own ([`carried`]). What moves a mark runs again in every
//!   compile. A mark that joins a group given back without it (a staff above
//!   died again: OPEN B26) takes away what the group was given on Y
//!   ([`Given`]), and its cell is not kept.
//! - **The check** ([`set_check`], 05 §6.3): what a callback read of the
//!   grobs no range covers (columns, the system, alignments) is listed by
//!   [`Report::uncovered`], for the harness to compare with what the key
//!   covers.
//! - **Line breaking's lines** (05 §6.2's step 2, MS6 step 9, D249): what
//!   `fill_line_details` found of a line from one break to another is kept
//!   by its key ([`LineKeys`]: the grobs whose columns it spans or whose span
//!   meets it), and a line of the next compile with that key takes it
//!   ([`LineKeys::kept`]); with the check, it is filled too and the two
//!   compared.
//! - **What a line's grobs got before the line breaks** (OPEN B17, D250) is
//!   kept too, the line the unit: every property written, offset, extent and
//!   pure height from the end of the prebroken pieces to the end of
//!   `read_before_breaking`, with what each grob depended on, is given back
//!   as one where the line's grobs, the breaks and all it read of other lines
//!   ([`COLUMNS`]' row among them) are the same ([`before_begin`]). A value
//!   alone would not do: a beam's direction computed for its original stands
//!   for both its pieces. `before-line-breaking` and `springs-and-rods` run
//!   again, on what was given back ([`rerunning`]). A column is in no line: its
//!   `horizontal-skylines` are kept by themselves ([`kept_column_skylines`]),
//!   and where the lines have moved they are made again from what stayed of
//!   their boxes (OPEN B21, [`KeptBoxes`]).

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};

use lily_engine_music::{Arena, GrobDigest, Host, Kept, Moment, OutputDef, grob_of};
use lily_engine_scheme::{Exception, Foreign, Heap, Tag, Tracer, Value, Vm};

use crate::constrained_breaking::LineDetails;
use crate::geometry::{Interval, X, Y};
use crate::grob::{self, grobs};
use crate::grob_array;
use crate::paper_column;
use crate::paper_score::{self, PaperScore};
use crate::separation_item::{self, KeptBoxes, Place};
use crate::spanner;

/// No grob: nothing is being computed for one.
pub(crate) const NONE: u32 = u32::MAX;
/// The line of a grob no range covers: a column, a system, an alignment.
const EXEMPT: u32 = u32::MAX;
/// The line of what belongs to no line: a mark, a `SpanBar`.
const SHARED: u32 = 0;

/// What a grob's callbacks did that keeps its cell from being reused: a
/// procedure of the score's own ran.
const IMPURE: u8 = 1;
/// It read where a line is placed in its system.
const READS_Y: u8 = 2;
/// It wrote outside its line, or was written to from outside it.
const COUPLED: u8 = 4;
/// It read an alignment's pure translations, which a value's digest covers
/// before the line breaks ([`State::translations`]).
const PURE_Y: u8 = 8;
/// Something was said while it ran before the line breaks: a line given back
/// would not say it again ([`said_since`]).
const SAID: u8 = 16;

/// The key of [`Line`]s' row of systems among a kept cell's ranges.
const SYSTEMS: u64 = u64::MAX;
/// The key of the row of columns: each one's moment and the records of what
/// is break-aligned in it, which what the break alignment gives before the
/// line breaks depends on.
const COLUMNS: u64 = u64::MAX - 1;

/// How many boxes of the columns are kept for when the lines move ([`KeptBoxes`],
/// ≈ 90 bytes each): an edit's ordinary score has fewer, and a score near the
/// memory's cap has no room for them (D260).
const BOXES: usize = 1 << 14;

static CHECK: AtomicBool = AtomicBool::new(false);

/// The grob whose cell of a page `grob` is written in (W4, 06 §6.3): its
/// line's `VerticalAxisGroup` piece, the cell above; else, as the rest of
/// its system, the root of its parents on Y. None for what is no grob.
pub fn print_line(heap: &mut Heap, grob: Value) -> Option<Value> {
    grob_of(heap, grob)?;
    grob::vertical_axis_group(heap, grob).or_else(|| {
        let mut root = grob;
        while let Some(parent) = grob::get_parent(heap, root, Y) {
            root = parent;
        }
        Some(root)
    })
}

/// Lists what callbacks read that no range covers, for the harness and the
/// tests (05 §6.3), in every compile of the process.
pub fn set_check(on: bool) {
    CHECK.store(on, Ordering::Relaxed);
}

/// Never in wasm, where nothing sets `CHECK`: the check is the native
/// harness's (MS7 step 5, D262).
fn checking() -> bool {
    !cfg!(target_arch = "wasm32") && CHECK.load(Ordering::Relaxed)
}

fn mix(h: u64, x: u64) -> u64 {
    (h ^ x).wrapping_mul(0x0100_0000_01b3)
}

/// A column's X as a key takes it, to 2⁻²⁴ of a staff space: an edit
/// elsewhere in a line leaves the sums of its springs a few last bits
/// away, which no output shows, and a grob reads its own column's X anew
/// (D248).
fn quantised(x: f64) -> u64 {
    libm_round(x * 16_777_216.0) as i64 as u64
}

fn libm_round(x: f64) -> f64 {
    let floor = x as i64 as f64;
    let floor = if floor > x { floor - 1.0 } else { floor };
    if x - floor < 0.5 { floor } else { floor + 1.0 }
}

/// What a grob's computed values depend on, as far as they have been computed.
#[derive(Clone)]
pub(crate) struct Dep {
    /// The range of its own line, empty while `lo > hi`.
    lo: u32,
    hi: u32,
    /// Those of other lines: the line, then the range.
    others: Vec<(u32, u32, u32)>,
    /// The other lines whose hubs it read, apart from `others`: a hub is at
    /// its line's start, and a range to it would take in all between.
    hubs: Vec<u32>,
    /// Those of them whose content it read, all of the line with its hubs
    /// ([`Tracker::holds`]).
    contents: Vec<u32>,
    /// The places among the [`COLUMNS`] of the break alignments it read
    /// before the line breaks, in order.
    cols: Vec<u32>,
    flags: u8,
}

impl Dep {
    const fn new() -> Dep {
        Dep {
            lo: u32::MAX,
            hi: 0,
            others: Vec::new(),
            hubs: Vec::new(),
            contents: Vec::new(),
            cols: Vec::new(),
            flags: 0,
        }
    }

    fn add_hubs(&mut self, line: u32) {
        if !self.hubs.contains(&line) {
            self.hubs.push(line);
        }
    }

    fn add_contents(&mut self, line: u32) {
        if !self.contents.contains(&line) {
            self.contents.push(line);
        }
    }

    fn add_column(&mut self, place: u32) {
        if let Err(at) = self.cols.binary_search(&place) {
            self.cols.insert(at, place);
        }
    }

    fn add(&mut self, own: u32, line: u32, lo: u32, hi: u32) {
        if line == own {
            self.lo = self.lo.min(lo);
            self.hi = self.hi.max(hi);
        } else if let Some(o) = self.others.iter_mut().find(|o| o.0 == line) {
            o.1 = o.1.min(lo);
            o.2 = o.2.max(hi);
        } else {
            self.others.push((line, lo, hi));
        }
    }

    /// `other`'s, of a grob of the line `line`, taken in by a grob of `own`.
    fn take(&mut self, own: u32, line: u32, other: &Dep) {
        if other.lo <= other.hi {
            self.add(own, line, other.lo, other.hi);
        }
        for &(l, lo, hi) in &other.others {
            self.add(own, l, lo, hi);
        }
        for &l in &other.hubs {
            self.add_hubs(l);
        }
        for &l in &other.contents {
            self.add_contents(l);
        }
        for &c in &other.cols {
            self.add_column(c);
        }
        self.flags |= other.flags;
    }
}

/// What layout notes of its grobs' reads and writes while cells are kept
/// ([`begin`]): held in D108's slot, read where the slot is already at hand.
#[derive(Clone)]
pub(crate) struct Tracker {
    /// The grob a callback is running for, if any.
    pub(crate) current: u32,
    /// Each grob's line, its place in the line's moment order, and whether
    /// it is a line's own axis group.
    lines: Vec<u32>,
    places: Vec<u32>,
    groups: Vec<bool>,
    /// Whether it is a grob of a whole line that all of the line's grobs
    /// read (a staff symbol, its ledger lines, the axis group): a cell holds
    /// its piece, and what it depends on is not theirs (D248).
    hubs: Vec<bool>,
    /// Whether it is a hub whose values its line's grobs make: an axis
    /// group (its extents, skylines, pure heights, elements) or its ledger
    /// lines (from its note heads). A staff symbol's are its own and its
    /// columns' (OPEN B25).
    holds: Vec<bool>,
    deps: Vec<Dep>,
    /// The properties written or removed since the line breaks ([`reuse`]).
    log: Vec<(u32, Value)>,
    logging: bool,
    /// With [`set_check`], what a cell's key does not cover: what happened,
    /// the grob running, the grob and the symbol.
    uncovered: Vec<(&'static str, u32, u32, Value)>,
    check: bool,
    y_offset: Value,
    pure_mark: Value,
    /// The system's alignments of lines, and the symbol of what they place
    /// the lines by.
    alignments: Vec<u32>,
    translations: Value,
    /// The cells' state and what is written while `read_before_breaking` runs.
    before: Option<Rc<RefCell<State>>>,
    before_log: Vec<(u32, Value)>,
    /// The row of [`COLUMNS`] among the lines.
    columns: u32,
    /// The grob whose `before-line-breaking` or `springs-and-rods`
    /// `read_before_breaking` reads
    /// ([`rerunning`]): what it writes is written again.
    rerun_of: u32,
    /// The symbols of a line's heights and of a column's skylines.
    heights: Value,
    skylines: Value,
    /// The marks a line's group took on after the breaks ([`carried`]), by
    /// the line: what the group writes to them is its own.
    carried: BTreeMap<u32, u32>,
    /// Where the columns' skylines read each grob of a line on Y while
    /// `read_before_breaking` runs ([`note_place`], OPEN B27), and where
    /// they first read those of the lines given back that moved after
    /// ([`given_place`]).
    read_places: BTreeMap<u32, Noted>,
    given_places: BTreeMap<u32, Rc<Place>>,
    /// The lines whose grobs a callback wrote to that is not theirs, while
    /// `read_before_breaking` runs, in order ([`crossed_mark`], OPEN B27).
    crossed: Vec<u32>,
    /// `(lily)`'s cell of what was said, and what was said as each callback
    /// the Scheme runs began ([`grob_entered`]).
    diagnostics: Value,
    marks: Vec<(u32, (usize, u64))>,
    /// The groups of the lines given back, by the group ([`Given`]).
    given: BTreeMap<u32, Given>,
}

/// What a line's group was given back that a mark joining it after
/// ([`carried`]) changes: what a compile from scratch has not computed of
/// it on Y when the mark joins (OPEN B26).
#[derive(Clone)]
struct Given {
    /// The marks its cell holds, which it was computed with.
    marks: Vec<u32>,
    /// Its `Y-extent` and `vertical-skylines` given back, where it had
    /// none, and whether its extent on Y was.
    syms: Vec<Value>,
    extent: bool,
    /// A mark its cell does not hold has joined it.
    spoiled: bool,
}

impl Default for Tracker {
    fn default() -> Tracker {
        Tracker {
            current: NONE,
            lines: Vec::new(),
            places: Vec::new(),
            groups: Vec::new(),
            hubs: Vec::new(),
            holds: Vec::new(),
            deps: Vec::new(),
            log: Vec::new(),
            logging: false,
            uncovered: Vec::new(),
            check: false,
            y_offset: Value::NULL,
            pure_mark: Value::NULL,
            alignments: Vec::new(),
            translations: Value::NULL,
            before: None,
            before_log: Vec::new(),
            columns: NONE,
            rerun_of: NONE,
            heights: Value::NULL,
            skylines: Value::NULL,
            carried: BTreeMap::new(),
            read_places: BTreeMap::new(),
            given_places: BTreeMap::new(),
            crossed: Vec::new(),
            diagnostics: Value::NULL,
            marks: Vec::new(),
            given: BTreeMap::new(),
        }
    }
}

impl Tracker {
    /// Space for the grob `id`, one layout made: no range covers it until
    /// [`Tracker::copy`] says what it is a copy of.
    fn ensure(&mut self, id: usize) {
        if self.lines.len() <= id && !self.lines.is_empty() {
            self.lines.resize(id + 1, EXEMPT);
            self.places.resize(id + 1, 0);
            self.groups.resize(id + 1, false);
            self.hubs.resize(id + 1, false);
            self.holds.resize(id + 1, false);
            self.deps.resize(id + 1, Dep::new());
        }
    }

    /// The grob `copy` made from `src`: of its line and place, depending on
    /// what `src` does so far.
    pub(crate) fn copy(&mut self, copy: u32, src: u32) {
        let (copy, src) = (copy as usize, src as usize);
        if self.lines.is_empty() || src >= self.lines.len() {
            return;
        }
        self.ensure(copy);
        self.lines[copy] = self.lines[src];
        self.places[copy] = self.places[src];
        self.groups[copy] = self.groups[src];
        self.hubs[copy] = self.hubs[src];
        self.holds[copy] = self.holds[src];
        self.deps[copy] = self.deps[src].clone();
    }

    /// What the callback running read of the grob `target`: its `sym` if a
    /// property, `'()` else.
    #[inline]
    pub(crate) fn read(&mut self, target: u32, sym: Value) {
        if self.current != NONE && self.current != target {
            self.note_read(target, sym);
        }
    }

    fn note_read(&mut self, target: u32, sym: Value) {
        let (cur, t) = (self.current as usize, target as usize);
        let Some(&line) = self.lines.get(t) else {
            return;
        };
        if line == EXEMPT {
            // Where the alignment puts the lines is no key's.
            // Before the line breaks, what is read of it is pure.
            if self.alignments.contains(&target) {
                if sym == self.translations || (sym == self.y_offset && self.before.is_some()) {
                    self.deps[cur].flags |= PURE_Y;
                } else if sym == self.y_offset {
                    self.deps[cur].flags |= READS_Y;
                }
            }
            // The break alignment's values are computed anew in every
            // compile, from what its column's grobs are.
            if self.before.is_some()
                && let Some(&place) = self.places.get(t)
                && place != NONE
            {
                self.deps[cur].add_column(place);
            }
            if self.check {
                self.uncovered.push(("read", self.current, target, sym));
            }
            return;
        }
        let Some(&own) = self.lines.get(cur) else {
            return;
        };
        let place = self.places[t];
        if self.hubs[t] {
            // What another line's axis group or ledger lines hold, all its
            // grobs made (OPEN B25); a column's skylines read of the lines'
            // hubs where each line is put, which their translations' digest
            // holds (D260).
            if line != own && self.holds[t] && own != EXEMPT {
                self.deps[cur].add_contents(line);
            } else if line != own {
                self.deps[cur].add_hubs(line);
            }
            let said = self.deps[t].flags & SAID;
            self.deps[cur].flags |= said;
            return;
        }
        let (reader, read) = if cur < t {
            let (l, r) = self.deps.split_at_mut(t);
            (&mut l[cur], &r[0])
        } else {
            let (l, r) = self.deps.split_at_mut(cur);
            (&mut r[0], &l[t])
        };
        reader.add(own, line, place, place);
        reader.take(own, line, read);
    }

    /// The offset of the grob `target` on Y read by the callback running: a
    /// line's own is where its staff is put.
    #[inline]
    pub(crate) fn read_y(&mut self, target: u32) {
        if self.current != NONE
            && self.current != target
            && self.groups.get(target as usize) == Some(&true)
            && self.lines[target as usize] != self.lines[self.current as usize]
            && self.carried.get(&self.current) != Some(&self.lines[target as usize])
        {
            self.deps[self.current as usize].flags |= READS_Y;
            if self.check {
                self.uncovered
                    .push(("reads-y", self.current, target, Value::NULL));
            }
        }
    }

    /// The grob `target` written to, by what is running: a callback's
    /// dependencies go with the value, and since the line breaks a write is
    /// logged, and one across a line's edge keeps the cells from being
    /// reused (a line's own group, which the alignment moves, apart).
    pub(crate) fn wrote(&mut self, target: u32, sym: Value) {
        let (cur, t) = (self.current as usize, target as usize);
        let Some(&line) = self.lines.get(t) else {
            return;
        };
        // The pure coordinate's mark that it is running holds nothing.
        if sym == self.pure_mark {
            return;
        }
        let own = self.lines.get(cur).copied();
        if let Some(own_line) = own.filter(|&o| cur != t && line != EXEMPT && o != EXEMPT) {
            let dep = self.deps[cur].clone();
            self.deps[t].take(line, own_line, &dep);
        }
        if self.before.is_some() {
            self.wrote_before(target, sym, own);
        }
        if !self.logging {
            return;
        }
        if line == EXEMPT {
            if own.is_some_and(|o| o != SHARED) {
                self.deps[cur].flags |= COUPLED;
                if self.check {
                    self.uncovered
                        .push(("wrote-out", self.current, target, sym));
                }
            }
            return;
        }
        match own {
            Some(o) if o == line => {}
            _ if self.groups[t] => {}
            Some(o) if self.carried.get(&target) == Some(&o) => {}
            Some(o) => {
                self.deps[t].flags |= COUPLED;
                if o != SHARED && o != EXEMPT {
                    self.deps[cur].flags |= COUPLED;
                }
                if self.check {
                    self.uncovered
                        .push(("wrote-across", self.current, target, sym));
                }
            }
            None => {
                self.deps[t].flags |= COUPLED;
                if self.check {
                    self.uncovered.push(("wrote-outside", NONE, target, sym));
                }
            }
        }
        if sym != Value::NULL {
            self.log.push((target, sym));
        }
    }

    /// [`Tracker::wrote`] while `read_before_breaking` runs, by what `own`
    /// says is running: a write that a line given back would lose, or lose
    /// to, keeps the lines it joins from being kept.
    fn wrote_before(&mut self, target: u32, sym: Value, own: Option<u32>) {
        let (cur, t) = (self.current as usize, target as usize);
        let line = self.lines[t];
        // What a callback that runs again writes is written again, not given
        // back: given back, it would be written twice.
        let again = self.current != NONE && self.rerun_of == self.current;
        if sym != Value::NULL && !again && sym != self.pure_mark {
            self.before_log.push((target, sym));
        }
        // A write to a grob of a line by what is not that grob (a dot's
        // column puts its dots): a column's skylines that made it are not
        // taken where the line is laid out again (OPEN B27).
        if line != EXEMPT && cur != t && !self.groups[t] && sym != self.pure_mark && !again {
            self.crossed.push(line);
        }
        // An alignment's pure translations are a cache, made again by
        // whoever reads them first.
        if line == EXEMPT && sym != self.translations {
            if own.is_some_and(|o| o != EXEMPT && !again) {
                self.deps[cur].flags |= COUPLED;
                if self.check {
                    self.uncovered
                        .push(("before-out", self.current, target, sym));
                }
            }
            return;
        }
        match own {
            Some(o) if o == line => {}
            _ if self.groups[t] => {}
            Some(o) => {
                self.deps[t].flags |= COUPLED;
                if o != EXEMPT {
                    self.deps[cur].flags |= COUPLED;
                }
                if self.check {
                    self.uncovered
                        .push(("before-across", self.current, target, sym));
                }
            }
            None => {
                self.deps[t].flags |= COUPLED;
                if self.check {
                    self.uncovered.push(("before-outside", NONE, target, sym));
                }
            }
        }
    }

    /// The property `sym` of the grob `target` removed.
    pub(crate) fn removed(&mut self, target: u32, sym: Value) {
        if self.logging {
            self.log.push((target, sym));
        }
        let again = self.current != NONE && self.rerun_of == self.current;
        if self.before.is_some() && !again && sym != self.pure_mark {
            self.before_log.push((target, sym));
        }
    }
}

/// Where the columns' skylines read a grob of a line on Y ([`note_place`]).
#[derive(Clone)]
enum Noted {
    /// One place, each time.
    At(Place),
    /// Places that differ: it moved between the columns that read it.
    Moved,
    /// None that stays with its line ([`Place`]): not compared, as before
    /// D268, the differential the only check.
    Unknown,
}

/// The mark `me`, in no line, has moved to the line `staff` is the group of
/// (`move_to_extremal_staff`). `extremal` says that it passed no staff to get
/// there: the staves it passes are other lines, and a cell holding it could
/// not see them change. Such a mark is carried: what the group writes to it
/// is the group's own, and its cell holds it.
///
/// A group given back without the mark (OPEN B26: a staff above it died
/// again, and the mark passes it) has what it was given on Y taken away,
/// computed again with the mark as from scratch, and its cell is not kept.
pub(crate) fn carried(heap: &mut Heap, me: Value, staff: Value, extremal: bool) {
    let (Some((arena, id)), Some((_, s))) = (grob_of(heap, me), grob_of(heap, staff)) else {
        return;
    };
    let (undo, outer) = {
        let mut a = arena.borrow_mut();
        let t = &mut grobs(&mut a).tracker;
        if let (Some(&own), Some(&line)) = (t.lines.get(id as usize), t.lines.get(s as usize))
            && extremal
            && own == SHARED
            && line != SHARED
            && line != EXEMPT
            && t.groups[s as usize]
        {
            t.carried.insert(id, line);
        }
        let undo = match t.given.get_mut(&s) {
            Some(g) if !g.spoiled && !g.marks.contains(&id) => {
                g.spoiled = true;
                Some((core::mem::take(&mut g.syms), g.extent))
            }
            _ => None,
        };
        (undo, core::mem::replace(&mut t.current, NONE))
    };
    if let Some((syms, extent)) = undo {
        if extent {
            grob::flush_extent_cache(heap, staff, Y);
        }
        for sym in syms {
            grob::del_property(heap, &arena, s, sym);
        }
    }
    grobs(&mut arena.borrow_mut()).tracker.current = outer;
}

/// What runs from here reads for no grob, until [`inside`] with what this
/// gives.
pub(crate) fn outside(heap: &Heap, g: Value) -> u32 {
    grob_of(heap, g).map_or(NONE, |(arena, _)| {
        core::mem::replace(&mut grobs(&mut arena.borrow_mut()).tracker.current, NONE)
    })
}

/// What ran since [`outside`] has ended.
pub(crate) fn inside(heap: &Heap, g: Value, outer: u32) {
    if let Some((arena, _)) = grob_of(heap, g) {
        grobs(&mut arena.borrow_mut()).tracker.current = outer;
    }
}

/// `grob`'s callback for `sym` begins: what it reads is its own, and the
/// reader it runs for gets it at [`leave`]. An exempt grob's runs as code
/// outside any callback, and so does a line's group's before the lines
/// break (its pure heights, which every compile computes) and its offset
/// (the alignment's). What was running, for [`leave`].
pub(crate) fn enter(a: &mut Arena, id: u32, sym: Value, foreign: bool) -> u32 {
    let t = &mut grobs(a).tracker;
    let outer = t.current;
    if t.lines.is_empty() {
        return outer;
    }
    let line = t.lines.get(id as usize).copied().unwrap_or(EXEMPT);
    let group = t.groups.get(id as usize) == Some(&true);
    let heights = sym == t.heights && t.before.is_some();
    let column = sym == t.skylines && t.before.is_some();
    let alignment = group && (sym == t.y_offset || (!t.logging && !heights));
    t.current = if (line == EXEMPT && !column) || alignment {
        NONE
    } else {
        id
    };
    if foreign && t.current != NONE {
        t.deps[id as usize].flags |= IMPURE;
    }
    outer
}

/// The callback [`enter`] began has ended.
pub(crate) fn leave(a: &mut Arena, id: u32, outer: u32) {
    let t = &mut grobs(a).tracker;
    t.current = outer;
    t.read(id, Value::NULL);
}

/// One line's grobs, or the systems, in moment order: each with its digest
/// and its record's, and the sums of each before it.
#[derive(Default)]
struct Line {
    key: u64,
    rows: Vec<(Moment, u64, u64)>,
    sums: Vec<u64>,
    /// Of the records' digests (D249's: what an edit elsewhere in the staff
    /// moves, a column's rank among it, left out), which what is kept
    /// before the line breaks is keyed by (D250).
    records: Vec<u64>,
    /// The sum of its hubs' records ([`Dep::hubs`]).
    hubs: u64,
}

impl Line {
    fn finish(&mut self) {
        self.rows.sort_by(|a, b| a.0.compare(b.0));
        self.sums = Vec::with_capacity(self.rows.len() + 1);
        self.records = Vec::with_capacity(self.rows.len() + 1);
        let (mut sum, mut records) = (0u64, 0u64);
        self.sums.push(0);
        self.records.push(0);
        for r in &self.rows {
            sum = sum.wrapping_add(r.1);
            records = records.wrapping_add(r.2);
            self.sums.push(sum);
            self.records.push(records);
        }
    }

    /// The digest of what was made from `lo` to `hi`, by `sums`.
    fn summed(&self, sums: &[u64], lo: Moment, hi: Moment) -> u64 {
        let a = self.rows.partition_point(|r| r.0.compare(lo).is_lt());
        let b = self.rows.partition_point(|r| r.0.compare(hi).is_le());
        match b > a {
            true => sums[b].wrapping_sub(sums[a]),
            false => 0,
        }
    }

    /// The digest of what was made from `lo` to `hi`.
    fn sum(&self, lo: Moment, hi: Moment) -> u64 {
        self.summed(&self.sums, lo, hi)
    }

    /// The same, by the records' digests.
    fn records(&self, lo: Moment, hi: Moment) -> u64 {
        self.summed(&self.records, lo, hi)
    }

    /// What holds `r` in this line, by `sums`; its hubs if it is of them,
    /// with all of `sums` if it read their content.
    fn read(&self, r: &Range, sums: &[u64]) -> u64 {
        match (r.hubs, r.content) {
            (true, false) => self.hubs,
            (true, true) => mix(self.hubs, sums.last().copied().unwrap_or(0)),
            (false, _) => self.summed(sums, r.lo, r.hi),
        }
    }

    /// The [`Range`] of this line's hubs, with all of `sums` if `content`;
    /// none if it has no row to place it.
    fn hub_range(&self, sums: &[u64], content: bool) -> Option<Range> {
        let at = self.rows.first()?.0;
        let mut r = Range {
            line: self.key,
            lo: at,
            hi: at,
            sum: 0,
            hubs: true,
            content,
        };
        r.sum = self.read(&r, sums);
        Some(r)
    }
}

/// A range a kept cell's callbacks read: the line's key, its moments and
/// their digest.
#[derive(Clone)]
struct Range {
    line: u64,
    lo: Moment,
    hi: Moment,
    sum: u64,
    /// Whether it stands for the line's hubs, with no moments.
    hubs: bool,
    /// Whether it read what they hold (an axis group's extent, its ledger
    /// lines), which all of the line's grobs make: the whole line with them
    /// (OPEN B25).
    content: bool,
}

/// The [`Range`]s of the lines whose hubs were read, `hubs`, and of those
/// whose hubs' content was, `contents`, each line once, by each line's
/// `records` if `records`, else its `sums`.
fn hub_ranges(
    lines: &[Line],
    mut hubs: Vec<u32>,
    mut contents: Vec<u32>,
    records: bool,
) -> Vec<Range> {
    contents.sort_unstable();
    contents.dedup();
    hubs.sort_unstable();
    hubs.dedup();
    hubs.retain(|l| contents.binary_search(l).is_err());
    let read = hubs.iter().map(|&l| (l, false));
    read.chain(contents.iter().map(|&l| (l, true)))
        .filter_map(|(l, content)| {
            let line = &lines[l as usize];
            let sums = if records { &line.records } else { &line.sums };
            line.hub_range(sums, content)
        })
        .collect()
}

/// What a grob of a kept cell got after the line breaks: the properties
/// written (`None` where one was removed) and its offsets and extents.
struct Done {
    writes: Vec<(Value, Option<Value>)>,
    offsets: [Option<f64>; 2],
    extents: [Option<Interval>; 2],
    /// Whether it is its line's own group, the alignment's to place.
    group: bool,
    dead: bool,
}

struct KeptCell {
    grobs: Vec<Done>,
    ranges: Vec<Range>,
    /// Its system's columns' X bit for bit ([`Now::exact`]), if what it
    /// holds prints a number unrounded ([`prints_exact`]): then the key's
    /// 2⁻²⁴ is not enough (OPEN B29).
    exact: Option<u64>,
}

/// What a grob of a line got before the line breaks: as [`Done`], its pure
/// height, and what it depended on: the line's key and the moments.
struct Before {
    done: Done,
    pure_height: Option<Interval>,
    reach: Vec<(u64, Moment, Moment)>,
    /// The keys of the other lines whose hubs it read ([`Dep::hubs`]), and
    /// of those whose hubs' content it read ([`Dep::contents`]).
    hubs: Vec<u64>,
    contents: Vec<u64>,
    /// Where a column's skylines first read it on Y, if it moved after
    /// ([`moved_since_read`]).
    first_place: Option<Rc<Place>>,
}

/// What a line's grobs got before the line breaks, in their order, and the
/// ranges of other lines they read.
struct KeptBefore {
    grobs: Vec<Before>,
    ranges: Vec<Range>,
}

/// A column's `horizontal-skylines`, what they read, and the digest of the
/// alignment's translations if they read those.
struct KeptSkylines {
    value: Value,
    ranges: Vec<Range>,
    translations: Option<u64>,
    /// What stays of its boxes when the lines move, and the padding.
    boxes: Option<(Rc<KeptBoxes>, f64)>,
    /// The keys of the lines whose grobs computing them wrote to from
    /// another grob ([`Tracker::crossed`]): taken only where each of those
    /// lines is given back, which holds what was written.
    wrote: Vec<u64>,
}

/// A compile's cells of one translation, kept for the next compile's.
struct Cells {
    cells: Vec<KeptCell>,
    by_key: BTreeMap<u64, u32>,
    /// Line breaking's lines by their key ([`LineKeys`]), with no column.
    lines: BTreeMap<u64, LineDetails>,
    /// What came before the line breaks, by line and by column (B17).
    before: BTreeMap<u64, Rc<KeptBefore>>,
    skylines: BTreeMap<u64, Rc<KeptSkylines>>,
    /// The line each mark's digest was carried by.
    marks: BTreeMap<u64, u64>,
}

fn trace_done(done: &Done, tracer: &mut Tracer<'_>) {
    for &(_, v) in &done.writes {
        if let Some(v) = v {
            tracer.trace(v);
        }
    }
}

fn trace_before(
    before: &BTreeMap<u64, Rc<KeptBefore>>,
    skylines: &BTreeMap<u64, Rc<KeptSkylines>>,
    tracer: &mut Tracer<'_>,
) {
    for line in before.values() {
        for b in &line.grobs {
            trace_done(&b.done, tracer);
        }
    }
    for s in skylines.values() {
        tracer.trace(s.value);
    }
}

/// [`Cells`] as the Scheme object the engine holds between compiles.
pub struct KeptCells(Rc<Cells>);

impl Foreign for KeptCells {
    fn type_name(&self) -> &'static str {
        "Layout_cells"
    }

    fn trace(&self, tracer: &mut Tracer<'_>) {
        for cell in &self.0.cells {
            for done in &cell.grobs {
                trace_done(done, tracer);
            }
        }
        trace_before(&self.0.before, &self.0.skylines, tracer);
        for d in self.0.lines.values() {
            for v in [d.break_permission, d.page_permission, d.turn_permission] {
                tracer.trace(v);
            }
        }
    }
}

/// A cell of the compile running.
struct Now {
    line: u32,
    /// Its system.
    system: Value,
    /// Its grobs, the marks it is thought to carry ([`Cells::marks`]) last.
    grobs: Vec<(Value, u32)>,
    /// How many are not marks, and the key's digest of them.
    own: usize,
    base: u64,
    key: u64,
    /// Its system's first and last moments, whose columns the key holds.
    span: Option<(Moment, Moment)>,
    /// Its system's columns' and break-aligned grobs' X bit for bit, which
    /// the key has to 2⁻²⁴ ([`KeptCell::exact`]).
    exact: u64,
    /// What it depended on, if it was reused: the cell's it was reused from.
    from: Option<u32>,
}

/// What a Paper_score's cells were of one compile.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub cells: u32,
    pub reused: u32,
    pub grobs: u32,
    pub grobs_reused: u32,
    /// Cells kept for the next compile.
    pub kept: u32,
    /// Of the cells not reused, those no kept cell has the key of, and those
    /// one has whose ranges differ.
    pub no_key: u32,
    pub no_range: u32,
    /// Of the cells not kept, those that ran a score's own procedure, read
    /// where a line is placed, or wrote across a line's edge.
    pub impure: u32,
    pub reads_y: u32,
    pub coupled: u32,
    /// What callbacks read that no range covers, each once: the reader's
    /// name, the grob's and the property.
    pub uncovered: Vec<String>,
    /// Line breaking's lines filled or taken (D249), those taken from the
    /// last compile, and with the check, those taken that came out other
    /// than filled.
    pub lines: u32,
    pub lines_reused: u32,
    pub lines_wrong: u32,
    /// What came before the line breaks (D250): lines given back of those
    /// there are, their grobs, those kept for the next compile, and the
    /// columns' skylines computed or taken and those taken.
    pub before: u32,
    pub before_reused: u32,
    pub before_grobs: u32,
    pub before_grobs_reused: u32,
    pub before_kept: u32,
    pub skylines: u32,
    pub skylines_reused: u32,
    /// Of those taken, those made again from their boxes as the lines moved.
    pub skylines_moved: u32,
    /// Of the lines not given back, those not kept as a grob of theirs
    /// was read on Y in two places by the columns' skylines (OPEN B27,
    /// D268); and the grobs kept with where they were first read, as they
    /// moved after.
    pub before_moved: u32,
    pub before_places: u32,
}

/// A Paper_score's cells while it is laid out.
pub struct State {
    /// The digest of what every grob of it reads: its output definition.
    settings: u64,
    digests: Vec<GrobDigest>,
    /// What each grob's grob arrays hold, by the records (D249).
    arrays: Vec<u64>,
    lines: Vec<Line>,
    by_key: BTreeMap<u64, u32>,
    /// The cells the last compile kept.
    old: Option<Rc<Cells>>,
    now: Vec<Now>,
    report: Report,
    /// What line breaking's keys are made of, once the columns are ranked.
    ranks: Option<Rc<Ranks>>,
    /// The lines of this compile, by their key.
    breaking: BTreeMap<u64, LineDetails>,
    /// Each line's key before the line breaks where it was not given back.
    before_keys: Vec<Option<u64>>,
    /// What is kept of this compile before the line breaks.
    before: BTreeMap<u64, Rc<KeptBefore>>,
    skylines: BTreeMap<u64, Rc<KeptSkylines>>,
    /// The digest of the alignments' pure translations, once asked for.
    translations: Option<u64>,
    /// The digest of the staves the marks choose among, in order: a cell
    /// that carries a mark holds its choice.
    staves: u64,
    /// Each line's group, by the line's key.
    groups: BTreeMap<u64, Value>,
    /// How many of [`BOXES`] this compile holds.
    boxes: usize,
}

impl State {
    /// The values it holds of this compile, which the grobs may no longer.
    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        trace_before(&self.before, &self.skylines, tracer);
    }
}

/// `pscore`'s grobs given lines (05 §6.1), and what they read noted from
/// here on; `old`, the last compile's [`keep`] of this translation, is what
/// [`reuse`] looks in. For a Paper_score of no root system, nothing.
pub fn begin(vm: &mut Vm, pscore: Value, kept: &Kept, old: Option<Value>) {
    let Some(system) = paper_score::root_system(vm.heap(), pscore) else {
        return;
    };
    let mut settings = settings(vm, pscore, kept.start);
    let mut alignments = Vec::new();
    let old = old.and_then(|v| vm.heap().foreign::<KeptCells>(v).map(|c| c.0.clone()));
    let heap = vm.heap_mut();
    let sym = heap.intern("all-elements");
    let y_offset = heap.intern("Y-offset");
    let pure_mark = heap.intern("pure-Y-offset-in-progress");
    let translations = heap.intern("minimum-translations-alist");
    let heights = heap.intern("adjacent-pure-heights");
    let skylines = heap.intern("horizontal-skylines");
    let all = grob::object_data(heap, system, sym);
    let Some((arena, _)) = grob_of(heap, system) else {
        return;
    };
    let n = arena.borrow().records.grobs.len();
    if let Some((_, s)) = grob_of(heap, system)
        && let Some(d) = kept.grobs.get(s as usize)
    {
        settings = mix(settings, d.record);
    }
    let mut lines = alloc::vec![Line::default()];
    let mut by_key: BTreeMap<u64, u32> = BTreeMap::from([(0, SHARED)]);
    let mut by_group: BTreeMap<u32, u32> = BTreeMap::new();
    let mut of = alloc::vec![EXEMPT; n];
    let mut places = alloc::vec![0u32; n];
    let mut is_group = alloc::vec![false; n];
    let mut is_hub = alloc::vec![false; n];
    let mut holds = alloc::vec![false; n];
    let mut members: Vec<u32> = Vec::new();
    for g in grob_array::grobs_of(heap, all) {
        let Some((_, id)) = grob_of(heap, g) else {
            continue;
        };
        if is_exempt(heap, g) {
            // What the key covers of these but their X: what was set on
            // them (a spacing spanner's, an alignment's properties).
            if is_alignment(heap, g) {
                alignments.push(id);
            }
            let columnar = grob::has_interface(heap, g, "paper-column-interface");
            if let (false, Some(d)) = (
                columnar || is_break_aligned(heap, g),
                kept.grobs.get(id as usize),
            ) {
                settings = mix(settings, d.record);
            }
            continue;
        }
        let group = grob::vertical_axis_group(heap, g).and_then(|v| grob_of(heap, v));
        let line = match group {
            Some((_, v)) => *by_group.entry(v).or_insert_with(|| {
                let context = kept
                    .grobs
                    .get(v as usize)
                    .map_or(u64::from(v), |d| d.context);
                lines.push(Line {
                    key: context,
                    ..Line::default()
                });
                by_key.insert(context, (lines.len() - 1) as u32);
                (lines.len() - 1) as u32
            }),
            None => SHARED,
        };
        is_group[id as usize] = group.is_some_and(|(_, v)| v == id);
        holds[id as usize] =
            is_group[id as usize] || grob::has_interface(heap, g, "ledger-line-spanner-interface");
        is_hub[id as usize] =
            holds[id as usize] || grob::has_interface(heap, g, "staff-symbol-interface");
        of[id as usize] = line;
        members.push(id);
    }
    // What a line's group holds and places though no parent leads to it (a
    // bar number, a metronome mark) is the line's.
    for (&v, &line) in &by_group {
        let group = arena.borrow().records.grobs[v as usize].handle;
        for g in held_by(heap, group) {
            if let Some((_, id)) = grob_of(heap, g)
                && of.get(id as usize) == Some(&SHARED)
            {
                of[id as usize] = line;
            }
        }
    }
    // N8's digests write a grob array as its type: what each holds goes into
    // its grob's, each grob by its record's.
    let mut digests = kept.grobs.clone();
    let mut arrays = alloc::vec![0u64; digests.len()];
    for (id, d) in digests.iter_mut().enumerate() {
        let (objects, named) = {
            let mut a = arena.borrow_mut();
            grobs(&mut a)
                .get(id as u32)
                .map_or((Value::NULL, [None; 4]), |g| {
                    let [x, y] = [g.dims[X].parent, g.dims[Y].parent];
                    (g.objects, [x, y, g.bounds[0], g.bounds[1]])
                })
        };
        let record_of = |heap: &Heap, g: Value| {
            let record = grob_of(heap, g).and_then(|(_, g)| kept.grobs.get(g as usize));
            record.map_or(0, |r| r.record)
        };
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        // What it names one by one, by the records: N8's digest leaves them
        // out, as their numbers move with an edit before them (OPEN B19).
        let mut one = 0xcbf2_9ce4_8422_2325u64;
        for g in named {
            one = mix(one, g.map_or(1, |g| record_of(heap, g)));
        }
        let mut p = objects;
        while let (Some(entry), Some(rest)) = (heap.car(p), heap.cdr(p)) {
            let value = heap.cdr(entry).unwrap_or(Value::NULL);
            if let Some(array) = heap.foreign::<grob_array::GrobArray>(value) {
                h = mix(h, array.grobs.len() as u64);
                for &e in &array.grobs {
                    h = mix(h, record_of(heap, e));
                }
            } else if grob_of(heap, value).is_some() {
                one = mix(one, record_of(heap, value));
            }
            p = rest;
        }
        arrays[id] = h;
        d.whole = mix(mix(d.whole, h), one);
    }
    for &id in &members {
        if let Some(d) = digests.get(id as usize) {
            let (digest, record) = if is_hub[id as usize] {
                (d.record, d.record)
            } else {
                (d.whole, mix(d.record, arrays[id as usize]))
            };
            let line = &mut lines[of[id as usize] as usize];
            line.rows.push((d.moment, digest, record));
            if is_hub[id as usize] {
                line.hubs = line.hubs.wrapping_add(d.record);
            }
        }
    }
    for line in &mut lines {
        line.finish();
    }
    let staves = staves(heap, all, kept, &arrays);
    let groups = by_group
        .iter()
        .map(|(&v, &line)| {
            (
                lines[line as usize].key,
                arena.borrow().records.grobs[v as usize].handle,
            )
        })
        .collect();
    // Each grob's place: the first row of its moment.
    for id in members {
        let line = &lines[of[id as usize] as usize];
        if let Some(d) = kept.grobs.get(id as usize) {
            let at = line.rows.partition_point(|r| r.0.compare(d.moment).is_lt());
            places[id as usize] = at.min(line.rows.len().saturating_sub(1)) as u32;
        }
    }
    {
        let mut a = arena.borrow_mut();
        let t = &mut grobs(&mut a).tracker;
        *t = Tracker {
            lines: of,
            places,
            groups: is_group,
            hubs: is_hub,
            holds,
            deps: alloc::vec![Dep::new(); n],
            check: checking(),
            y_offset,
            pure_mark,
            alignments,
            translations,
            heights,
            skylines,
            ..Tracker::default()
        };
    }
    let state = State {
        settings,
        digests,
        arrays,
        lines,
        by_key,
        old,
        now: Vec::new(),
        report: Report::default(),
        ranks: None,
        breaking: BTreeMap::new(),
        before_keys: Vec::new(),
        before: BTreeMap::new(),
        skylines: BTreeMap::new(),
        translations: None,
        staves,
        groups,
        boxes: 0,
    };
    if let Some(ps) = vm.heap_mut().foreign_mut::<PaperScore>(pscore) {
        ps.cells = Some(Rc::new(RefCell::new(state)));
    }
}

/// The digest of what [`carried`]'s marks choose among: the lines of each
/// alignment of `all`, in order, and what each staff grouper holds.
fn staves(heap: &mut Heap, all: Value, kept: &Kept, arrays: &[u64]) -> u64 {
    let sym = heap.intern("elements");
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for g in grob_array::grobs_of(heap, all) {
        let Some((_, id)) = grob_of(heap, g) else {
            continue;
        };
        if grob::has_interface(heap, g, "staff-grouper-interface") {
            h = mix(h, arrays.get(id as usize).copied().unwrap_or(0));
        } else if is_alignment(heap, g) {
            let elements = grob::object_data(heap, g, sym);
            for e in grob_array::grobs_of(heap, elements) {
                let line = grob_of(heap, e).and_then(|(_, e)| kept.grobs.get(e as usize));
                h = mix(h, line.map_or(0, |d| d.context));
            }
        }
    }
    h
}

/// What the line's group `group` holds and places: its `elements` and its
/// `vertical-skyline-elements`, which `calc-skylines` reads in their place.
fn held_by(heap: &mut Heap, group: Value) -> Vec<Value> {
    let mut held = Vec::new();
    for name in ["elements", "vertical-skyline-elements"] {
        let sym = heap.intern(name);
        let array = grob::object_data(heap, group, sym);
        held.extend(grob_array::grobs_of(heap, array));
    }
    held
}

/// Whether no line's range covers the grob `g`: a column, an alignment, or
/// what the break alignment is made of. Its X is in the key ([`reuse`]).
fn is_exempt(heap: &mut Heap, g: Value) -> bool {
    grob::has_interface(heap, g, "paper-column-interface")
        || is_alignment(heap, g)
        || is_break_aligned(heap, g)
        || is_spacing(heap, g)
}

/// Whether `g` aligns the system's lines: an alignment in no line (one in a
/// line, a figured bass's, is the line's).
fn is_alignment(heap: &mut Heap, g: Value) -> bool {
    grob::has_interface(heap, g, "align-interface") && grob::vertical_axis_group(heap, g).is_none()
}

fn is_break_aligned(heap: &Heap, g: Value) -> bool {
    let Some((arena, id)) = grob_of(heap, g) else {
        return false;
    };
    let name = arena.borrow().records.grobs[id as usize].name;
    matches!(heap.name(name), Some("BreakAlignment" | "BreakAlignGroup"))
}

/// Whether the grob `g` is a spacing spanner, whose results are the columns'.
fn is_spacing(heap: &mut Heap, g: Value) -> bool {
    grob::has_interface(heap, g, "spacing-spanner-interface")
}

/// The digest of a value of an output definition.
fn value_digest(heap: &Heap, v: Value, budget: &mut u32) -> u64 {
    if *budget == 0 {
        return 1;
    }
    *budget -= 1;
    let mut h = mix(0xcbf2_9ce4_8422_2325, v.tag() as u64);
    match v.tag() {
        Tag::Pair => {
            h = mix(h, value_digest(heap, heap.car(v).unwrap_or(v), budget));
            h = mix(h, value_digest(heap, heap.cdr(v).unwrap_or(v), budget));
        }
        Tag::Vector => {
            for &x in heap.vector_items(v).unwrap_or(&[]) {
                h = mix(h, value_digest(heap, x, budget));
            }
        }
        Tag::Symbol => h = name_digest(h, heap.name(v).unwrap_or_default()),
        Tag::String => h = name_digest(h, heap.str(v).map_or("", |s| s.as_str())),
        Tag::Flonum | Tag::Ratio | Tag::Bignum => {
            h = mix(h, heap.to_f64(v).unwrap_or(0.0).to_bits());
        }
        Tag::Closure => h = mix(h, heap.closure_parts(v).map_or(0, |(code, _)| code)),
        Tag::Foreign => {
            h = name_digest(h, heap.foreign_object(v).map_or("", |f| f.type_name()));
        }
        _ => h = mix(h, v.to_bits()),
    }
    h
}

fn name_digest(mut h: u64, s: &str) -> u64 {
    h = mix(h, s.len() as u64);
    for b in s.bytes() {
        h = mix(h, b.into());
    }
    h
}

/// The digest of `pscore`'s output definition and its parents, a variable
/// at a time, and of the contexts' first state, `start`.
fn settings(vm: &mut Vm, pscore: Value, start: u64) -> u64 {
    let mut sum = start;
    let mut def = paper_score::layout(vm.heap(), pscore);
    while let Some(d) = def {
        let Some((scope, parent)) = vm
            .heap()
            .foreign::<OutputDef>(d)
            .map(|o| (o.scope, o.parent))
        else {
            break;
        };
        for (name, value) in vm.module_locals(scope).unwrap_or_default() {
            let mut budget = 2048;
            let heap = vm.heap();
            let h = mix(
                value_digest(heap, name, &mut budget),
                value_digest(heap, value, &mut budget),
            );
            sum = sum.wrapping_add(h);
        }
        def = parent;
    }
    sum
}

/// Whether `v`, a value a cell keeps, holds a stencil expression that prints
/// a number unrounded: a `dashed-line`'s on and off, which LilyPond writes
/// with `~a`, or an `output-attributes`' value. A dotted line's period is its
/// length divided, so the last bits of its columns' X show (OPEN B29); the
/// preview's digest reads the same two exactly (`crates/svg`'s `Sum`). Past
/// `budget`, true.
fn prints_exact(heap: &Heap, v: Value, budget: &mut u32) -> bool {
    if *budget == 0 {
        return true;
    }
    *budget -= 1;
    match v.tag() {
        Tag::Pair => {
            let head = heap.car(v).unwrap_or(Value::NULL);
            if matches!(heap.name(head), Some("dashed-line" | "output-attributes")) {
                return true;
            }
            let mut p = v;
            while p.tag() == Tag::Pair {
                if prints_exact(heap, heap.car(p).unwrap_or(Value::NULL), budget) {
                    return true;
                }
                p = heap.cdr(p).unwrap_or(Value::NULL);
            }
            false
        }
        Tag::Foreign => heap
            .foreign::<crate::stencil::Stencil>(v)
            .is_some_and(|s| prints_exact(heap, s.expr, budget)),
        _ => false,
    }
}

/// Whether `v` holds no grob, nor anything else a compile's own: it can be
/// a later compile's value as it is.
fn portable(heap: &Heap, v: Value, budget: &mut u32) -> bool {
    if *budget == 0 {
        return false;
    }
    *budget -= 1;
    match v.tag() {
        Tag::Pair => {
            let mut p = v;
            while p.tag() == Tag::Pair {
                if *budget == 0 || !portable(heap, heap.car(p).unwrap_or(v), budget) {
                    return false;
                }
                *budget -= 1;
                p = heap.cdr(p).unwrap_or(Value::NULL);
            }
            portable(heap, p, budget)
        }
        Tag::Vector => heap
            .vector_items(v)
            .unwrap_or(&[])
            .iter()
            .all(|&x| portable(heap, x, budget)),
        Tag::Closure | Tag::Values => false,
        Tag::Foreign => {
            if let Some(s) = heap.foreign::<crate::stencil::Stencil>(v) {
                return portable(heap, s.expr, budget);
            }
            matches!(
                heap.foreign_object(v).map(|f| f.type_name()),
                Some(
                    "Skyline"
                        | "Skyline_pair"
                        | "Moment"
                        | "Font_metric"
                        | "Open_type_font"
                        | "Pango_font"
                        | "Modified_font_metric"
                )
            )
        }
        _ => true,
    }
}

/// The digest of the grob `id`, of the records it was copied from for a
/// piece layout made: which piece it is mixed in.
fn digest_of(state: &State, heap: &Heap, a: &mut Arena, id: u32) -> u64 {
    if let Some(d) = state.digests.get(id as usize) {
        // A hub's whole holds what all the line's grobs put in its arrays.
        let hub = grobs(a).tracker.hubs.get(id as usize) == Some(&true);
        return if hub { d.record } else { d.whole };
    }
    let slot = grobs(a);
    let Some(g) = slot.get(id) else {
        return 0;
    };
    let (original, index) = (g.original, g.break_index as u64);
    let Some((_, oid)) = original.and_then(|o| grob_of(heap, o)) else {
        return 0;
    };
    let side = slot.get(oid).map_or(0, |o| {
        match o
            .broken
            .iter()
            .position(|b| b.and_then(|b| grob_of(heap, b)).is_some_and(|b| b.1 == id))
        {
            Some(i) => i as u64 + 1,
            None => 0,
        }
    });
    mix(mix(digest_of(state, heap, a, oid), index), side)
}

/// The digest of the grob `id` that line breaking's keys take (D249): its
/// record and what its grob arrays hold, not what layout's slot held of it
/// as the translation ended (its parents, the extents read), which an edit
/// elsewhere in its staff renumbers; a hub's by its record, as
/// [`digest_of`]'s; a piece layout made by its original's, which piece it is
/// mixed in.
fn line_digest_of(state: &State, heap: &Heap, a: &mut Arena, id: u32, hub: bool) -> u64 {
    if let Some(d) = state.digests.get(id as usize) {
        return match hub {
            true => d.record,
            false => mix(d.record, state.arrays[id as usize]),
        };
    }
    let slot = grobs(a);
    let Some(g) = slot.get(id) else {
        return 0;
    };
    let (original, index) = (g.original, g.break_index as u64);
    let Some((_, oid)) = original.and_then(|o| grob_of(heap, o)) else {
        return 0;
    };
    let side = slot.get(oid).map_or(0, |o| {
        match o
            .broken
            .iter()
            .position(|b| b.and_then(|b| grob_of(heap, b)).is_some_and(|b| b.1 == id))
        {
            Some(i) => i as u64 + 1,
            None => 0,
        }
    });
    mix(mix(line_digest_of(state, heap, a, oid, hub), index), side)
}

/// Whether line breaking takes the grob `g` by its record (D249): what spans
/// a staff or the system and holds what its lines have, which a line's key
/// has by the items in it.
fn is_line_hub(heap: &mut Heap, g: Value) -> bool {
    [
        "ledger-line-spanner-interface",
        "staff-symbol-interface",
        "axis-group-interface",
        "align-interface",
        "spacing-spanner-interface",
        "system-start-delimiter-interface",
        "staff-grouper-interface",
    ]
    .iter()
    .any(|i| grob::has_interface(heap, g, i))
}

/// The cells of `pscore`, its lines broken: each system's columns digested
/// and its grobs put in cells, those a kept cell's key and ranges match
/// given what it holds, and what is written from here on logged. Called
/// where the break substitution has done its work, before `after-line-
/// breaking`.
pub fn reuse(host: &mut dyn Host, pscore: Value, root: Value) -> Result<(), Exception> {
    let Some(state) = host
        .heap()
        .foreign::<PaperScore>(pscore)
        .and_then(|p| p.cells.clone())
    else {
        return Ok(());
    };
    let Some((arena, _)) = grob_of(host.heap(), root) else {
        return Ok(());
    };
    let mut state = state.borrow_mut();
    let state = &mut *state;
    let sym = host.heap().intern("all-elements");
    let mut systems = Line {
        key: SYSTEMS,
        ..Line::default()
    };
    let mut now: Vec<Now> = Vec::new();
    for child in spanner::broken_intos(host.heap(), root) {
        let all = grob_array::object(host, child, sym)?;
        let elements = grob_array::grobs_of(host.heap(), all);
        // What is break-aligned, its X computed, not as cached: a column
        // whose skylines were given back has not asked for it before the
        // breaks (D250).
        let mut aligned = Vec::new();
        for &g in &elements {
            if is_break_aligned(host.heap(), g) {
                let offset = grob::get_offset(host, g, X)?;
                aligned.push((offset, grob::extent(host, g, g, X)?));
            }
        }
        let mut aligned = aligned.into_iter();
        let heap = host.heap();
        // The columns: each one's place from the first, its record and X.
        let mut x = 0xcbf2_9ce4_8422_2325u64;
        let mut exact = x;
        let mut first = None;
        let (mut from, mut to) = (None, None);
        for &g in &elements {
            if is_break_aligned(heap, g) {
                let (offset, extent) = aligned.next().unwrap_or((0.0, Interval::EMPTY));
                x = mix(x, quantised(offset));
                x = mix(mix(x, quantised(extent.left)), quantised(extent.right));
                exact = mix(exact, offset.to_bits());
                exact = mix(mix(exact, extent.left.to_bits()), extent.right.to_bits());
                let original = grob::original(heap, g).unwrap_or(g);
                let record =
                    grob_of(heap, original).and_then(|(_, o)| state.digests.get(o as usize));
                x = mix(x, record.map_or(0, |d| d.record));
                continue;
            }
            if !grob::has_interface(heap, g, "paper-column-interface") {
                continue;
            }
            let Some((_, id)) = grob_of(heap, g) else {
                continue;
            };
            let rank = paper_column::rank(heap, g);
            let base = *first.get_or_insert(rank);
            let offset = grob::with_grob(heap, g, |c| c.dims[X].offset).flatten();
            let record = state.digests.get(id as usize).map_or_else(
                || {
                    let original = grob::original(heap, g).and_then(|o| grob_of(heap, o));
                    original
                        .and_then(|(_, o)| state.digests.get(o as usize))
                        .copied()
                },
                |d| Some(*d),
            );
            x = mix(mix(x, rank.wrapping_sub(base)), offset.map_or(1, quantised));
            exact = mix(exact, offset.map_or(1, f64::to_bits));
            if let Some(d) = record {
                x = mix(x, d.record);
                from = Some(from.map_or(d.moment, |m: Moment| {
                    if d.moment.compare(m).is_lt() {
                        d.moment
                    } else {
                        m
                    }
                }));
                to = Some(to.map_or(d.moment, |m: Moment| {
                    if d.moment.compare(m).is_gt() {
                        d.moment
                    } else {
                        m
                    }
                }));
            }
        }
        if let (Some(from), Some(to)) = (from, to) {
            systems.rows.push((from, x, x));
            systems.rows.push((to, x.rotate_left(1), x.rotate_left(1)));
        }
        // The cells, in the order their first grobs have. A grob no parent
        // leads to a line's group is that of the line's that holds it.
        let mut at: BTreeMap<u32, usize> = BTreeMap::new();
        let first_cell = now.len();
        let mut held: BTreeMap<u32, Value> = BTreeMap::new();
        for &g in &elements {
            if !grob::has_interface(heap, g, "axis-group-interface") {
                continue;
            }
            if grob::vertical_axis_group(heap, g) == Some(g) {
                for e in held_by(heap, g) {
                    if let Some((_, id)) = grob_of(heap, e) {
                        held.insert(id, g);
                    }
                }
            }
        }
        for &g in &elements {
            let Some((_, id)) = grob_of(heap, g) else {
                continue;
            };
            let group = grob::vertical_axis_group(heap, g).or_else(|| held.get(&id).copied());
            let Some(group) = group else {
                continue;
            };
            let Some((_, group_id)) = grob_of(heap, group) else {
                continue;
            };
            let (line, own) = {
                let mut a = arena.borrow_mut();
                let t = &grobs(&mut a).tracker;
                let of = |i: u32| t.lines.get(i as usize).copied().unwrap_or(EXEMPT);
                (of(group_id), of(id))
            };
            if line == EXEMPT || line == SHARED || own == EXEMPT {
                continue;
            }
            // A mark the line's group took on after it was made is the
            // line's from here: what it is written is the line's.
            if own == SHARED {
                let moment = state.digests.get(id as usize).map(|d| d.moment);
                let rows = &state.lines[line as usize].rows;
                let mut a = arena.borrow_mut();
                let t = &mut grobs(&mut a).tracker;
                t.lines[id as usize] = line;
                if let Some(m) = moment {
                    let place = rows.partition_point(|r| r.0.compare(m).is_lt());
                    t.places[id as usize] = place.min(rows.len().saturating_sub(1)) as u32;
                }
            }
            let i = *at.entry(group_id).or_insert_with(|| {
                now.push(Now {
                    line,
                    system: child,
                    grobs: Vec::new(),
                    own: 0,
                    base: 0,
                    key: 0,
                    span: from.zip(to),
                    exact,
                    from: None,
                });
                now.len() - 1
            });
            now[i].grobs.push((g, id));
        }
        for cell in &mut now[first_cell..] {
            let mut h = mix(state.settings, state.lines[cell.line as usize].key);
            h = mix(h, x);
            for &(_, id) in &cell.grobs {
                h = mix(h, digest_of(state, heap, &mut arena.borrow_mut(), id));
            }
            cell.own = cell.grobs.len();
            cell.base = h;
            cell.key = h;
            crate::queue::check(crate::queue::Step::Cells);
        }
        // The marks, in the cells that carried them last.
        let Some(old) = state.old.clone() else {
            continue;
        };
        if old.marks.is_empty() {
            continue;
        }
        for &g in &elements {
            let Some((_, id)) = grob_of(heap, g) else {
                continue;
            };
            let in_no_line = {
                let mut a = arena.borrow_mut();
                grobs(&mut a).tracker.lines.get(id as usize) == Some(&SHARED)
            };
            if !in_no_line || grob::vertical_axis_group(heap, g).is_some() {
                continue;
            }
            let digest = line_digest_of(state, heap, &mut arena.borrow_mut(), id, false);
            let Some(line) = old.marks.get(&digest) else {
                continue;
            };
            let cell = now[first_cell..]
                .iter_mut()
                .find(|c| state.lines[c.line as usize].key == *line);
            if let Some(cell) = cell {
                cell.grobs.push((g, id));
            }
        }
        for cell in &mut now[first_cell..] {
            cell.grobs[cell.own..].sort_unstable_by_key(|&(_, id)| id);
            cell.key = marks_key(state, heap, &mut arena.borrow_mut(), cell);
        }
    }
    systems.finish();
    if state.lines.last().is_none_or(|l| l.key != SYSTEMS) {
        state.by_key.insert(SYSTEMS, state.lines.len() as u32);
        state.lines.push(systems);
    }
    {
        let mut a = arena.borrow_mut();
        let t = &mut grobs(&mut a).tracker;
        t.logging = true;
        for id in 0..t.deps.len() {
            if t.hubs[id] {
                t.deps[id] = Dep::new();
            }
        }
    }
    for cell in &mut now {
        state.report.cells += 1;
        state.report.grobs += cell.grobs.len() as u32;
        let Some(old) = &state.old else {
            continue;
        };
        let Some(&k) = old.by_key.get(&cell.key) else {
            state.report.no_key += 1;
            continue;
        };
        let kept = &old.cells[k as usize];
        // What prints a column's X unrounded wants it bit for bit.
        if kept.exact.is_some_and(|e| e != cell.exact) {
            state.report.no_key += 1;
            continue;
        }
        let holds = kept.grobs.len() == cell.grobs.len()
            && kept.ranges.iter().all(|r| {
                let sum = state.by_key.get(&r.line).map_or(0, |&l| {
                    let line = &state.lines[l as usize];
                    line.read(r, &line.sums)
                });
                sum == r.sum
            });
        if !holds {
            state.report.no_range += 1;
            continue;
        }
        let old = old.clone();
        give_back(host, &arena, cell, &old.cells[k as usize]);
        crate::queue::check(crate::queue::Step::Cells);
        cell.from = Some(k);
        state.report.reused += 1;
        state.report.grobs_reused += cell.grobs.len() as u32;
    }
    state.now = now;
    Ok(())
}

/// `cell`'s key with the marks it holds: none, its key without them.
fn marks_key(state: &State, heap: &Heap, a: &mut Arena, cell: &Now) -> u64 {
    let marks = &cell.grobs[cell.own..];
    if marks.is_empty() {
        return cell.base;
    }
    let mut h = mix(cell.base, state.staves);
    for &(_, id) in marks {
        h = mix(h, digest_of(state, heap, a, id));
    }
    h
}

/// The cells of the compile that ran, each holding the marks it carried
/// ([`carried`]): a mark goes to its cell the first time, and a mark thought
/// carried that went elsewhere leaves. A reused cell's are as they were.
fn settle_marks(state: &mut State, heap: &mut Heap, arena: &RefCell<Arena>) -> BTreeMap<u64, u64> {
    let carried: Vec<(u32, u32)> = {
        let mut a = arena.borrow_mut();
        let t = &grobs(&mut a).tracker;
        t.carried.iter().map(|(&id, &line)| (id, line)).collect()
    };
    let carried: Vec<(u32, Value, Option<Value>, u32)> = carried
        .into_iter()
        .map(|(id, line)| {
            let g = arena.borrow().records.grobs[id as usize].handle;
            (line, g, crate::item::get_system(heap, g), id)
        })
        .collect();
    let mut now = core::mem::take(&mut state.now);
    let mut marks = BTreeMap::new();
    let mut a = arena.borrow_mut();
    for cell in &mut now {
        if cell.from.is_none() {
            cell.grobs.truncate(cell.own);
            for &(line, g, system, id) in &carried {
                if line == cell.line && system == Some(cell.system) {
                    cell.grobs.push((g, id));
                }
            }
            cell.key = marks_key(state, heap, &mut a, cell);
        }
        for &(_, id) in &cell.grobs[cell.own..] {
            let digest = line_digest_of(state, heap, &mut a, id, false);
            marks.insert(digest, state.lines[cell.line as usize].key);
        }
    }
    state.now = now;
    marks
}

/// What `kept` holds given to the grobs of `cell`, in order.
fn give_back(host: &mut dyn Host, arena: &RefCell<Arena>, cell: &Now, kept: &KeptCell) {
    for (&(g, id), done) in cell.grobs.iter().zip(&kept.grobs) {
        if done.group {
            let marks = cell.grobs[cell.own..].iter().map(|&(_, m)| m).collect();
            let given = given(host.heap(), arena, g, id, done, marks);
            grobs(&mut arena.borrow_mut())
                .tracker
                .given
                .insert(id, given);
        }
        give_back_to(host, arena, g, id, done);
    }
}

/// What of `done` the group `g`, `id` on `arena`, would lose to a mark its
/// cell does not hold, `marks` those it does ([`Given`]).
fn given(
    heap: &mut Heap,
    arena: &RefCell<Arena>,
    g: Value,
    id: u32,
    done: &Done,
    marks: Vec<u32>,
) -> Given {
    let on_y = [heap.intern("Y-extent"), heap.intern("vertical-skylines")];
    let props = arena.borrow().records.grobs[id as usize].props;
    let syms = done
        .writes
        .iter()
        .filter(|w| w.1.is_some() && on_y.contains(&w.0))
        .filter(|w| grob::assq_ref(heap, w.0, props).is_none())
        .map(|w| w.0)
        .collect();
    let none = grob::with_grob(heap, g, |o| o.dims[Y].extent.is_none());
    Given {
        marks,
        syms,
        extent: done.extents[Y].is_some() && none == Some(true),
        spoiled: false,
    }
}

/// What `done` holds given to the grob `g`, `id` on `arena`.
fn give_back_to(host: &mut dyn Host, arena: &RefCell<Arena>, g: Value, id: u32, done: &Done) {
    let outer = {
        let mut a = arena.borrow_mut();
        let t = &mut grobs(&mut a).tracker;
        core::mem::replace(&mut t.current, id)
    };
    for &(sym, v) in &done.writes {
        match v {
            Some(v) => grob::set_property_data(host.heap(), arena, id, sym, v),
            None => grob::del_property(host.heap(), arena, id, sym),
        }
    }
    grob::with_grob_mut(host.heap(), g, |o| {
        for a in [X, Y] {
            if !(done.group && a == Y) {
                o.dims[a].offset = done.offsets[a];
            }
            o.dims[a].extent = done.extents[a];
        }
    });
    if done.dead {
        grob::suicide(host.heap(), g);
    }
    let mut a = arena.borrow_mut();
    grobs(&mut a).tracker.current = outer;
}

/// What `pscore`'s cells hold, its pages made, for the next compile's
/// [`begin`]; none where it has none. `quiet` says that its pages said
/// nothing: a cell reused would not say it again.
pub fn keep(host: &mut dyn Host, pscore: Value, quiet: bool) -> Option<Value> {
    let state = host
        .heap()
        .foreign::<PaperScore>(pscore)
        .and_then(|p| p.cells.clone())?;
    let kept = keep_cells(host, &state, pscore, quiet);
    // What this compile's cells were made with goes with it: the translation
    // the next compile resumes from outlives it.
    let mut state = state.borrow_mut();
    let state = &mut *state;
    (state.digests, state.arrays, state.lines) = (Vec::new(), Vec::new(), Vec::new());
    (state.old, state.ranks) = (None, None);
    state.by_key.clear();
    state.now.clear();
    state.breaking.clear();
    state.before.clear();
    state.skylines.clear();
    if let Some((arena, _)) =
        paper_score::root_system(host.heap(), pscore).and_then(|root| grob_of(host.heap(), root))
    {
        grobs(&mut arena.borrow_mut()).tracker = Tracker::default();
    }
    kept
}

/// [`keep`]'s work.
fn keep_cells(
    host: &mut dyn Host,
    state: &Rc<RefCell<State>>,
    pscore: Value,
    quiet: bool,
) -> Option<Value> {
    let mut state = state.borrow_mut();
    let state = &mut *state;
    let root = paper_score::root_system(host.heap(), pscore)?;
    let (arena, _) = grob_of(host.heap(), root)?;
    if !quiet {
        return None;
    }
    let marks = settle_marks(state, host.heap(), &arena);
    let moving = host.heap().intern("after-line-breaking");
    let mut a = arena.borrow_mut();
    let heap = host.heap();
    let mut log = core::mem::take(&mut grobs(&mut a).tracker.log);
    log.sort_by_key(|&(id, _)| id);
    let mut cells = Vec::new();
    let mut by_key = BTreeMap::new();
    let old = state.old.clone();
    for cell in &state.now {
        let t = &grobs(&mut a).tracker;
        // A cell reused whose group took a mark it does not hold has
        // what the mark changed computed again: as one not reused, it
        // was written across ([`carried`]).
        let spoiled = cell.from.is_some()
            && cell.grobs[..cell.own]
                .iter()
                .any(|&(_, id)| t.given.get(&id).is_some_and(|g| g.spoiled));
        if spoiled {
            state.report.coupled += 1;
            continue;
        }
        // What a cell reused depended on is what the cell it came from did.
        let (mut flags, mut ranges) = (0u8, BTreeMap::<u32, (u32, u32)>::new());
        let (mut hubs, mut contents) = (Vec::new(), Vec::new());
        if cell.from.is_none() {
            for (i, &(_, id)) in cell.grobs.iter().enumerate() {
                let (line, d) = (t.lines[id as usize], &t.deps[id as usize]);
                // What was said before the breaks is said whatever the cell;
                // and a mark's pure offset, kept till the breaks, is no value
                // of the cell's.
                let pure = if i < cell.own { 0 } else { PURE_Y };
                flags |= d.flags & !SAID & !pure;
                let mut add = |l: u32, lo: u32, hi: u32| {
                    let e = ranges.entry(l).or_insert((lo, hi));
                    (e.0, e.1) = (e.0.min(lo), e.1.max(hi));
                };
                if d.lo <= d.hi {
                    add(line, d.lo, d.hi);
                }
                // A hub's place is its line's start: the key holds its piece.
                if !t.hubs[id as usize] {
                    add(line, t.places[id as usize], t.places[id as usize]);
                }
                for &(l, lo, hi) in &d.others {
                    add(l, lo, hi);
                }
                hubs.extend_from_slice(&d.hubs);
                // What its own line's group holds is in this system, whose
                // grobs the key has.
                contents.extend(d.contents.iter().filter(|&&l| l != cell.line));
            }
        }
        if flags != 0 {
            state.report.impure += u32::from(flags & IMPURE != 0);
            state.report.reads_y += u32::from(flags & (READS_Y | PURE_Y) != 0);
            state.report.coupled += u32::from(flags & COUPLED != 0);
            continue;
        }
        let mut kept_ranges = Vec::new();
        let mut exact = None;
        match cell
            .from
            .and_then(|k| old.as_ref().map(|o| &o.cells[k as usize]))
        {
            Some(from) => {
                kept_ranges.clone_from(&from.ranges);
                exact = from.exact;
            }
            None => {
                let mut hull: Option<(Moment, Moment)> = None;
                for (&l, &(lo, hi)) in &ranges {
                    let line = &state.lines[l as usize];
                    // The key has the columns.
                    if line.key == COLUMNS {
                        continue;
                    }
                    let (Some(lo), Some(hi)) =
                        (line.rows.get(lo as usize), line.rows.get(hi as usize))
                    else {
                        continue;
                    };
                    let (lo, hi) = (lo.0, hi.0);
                    let sum = line.sum(lo, hi);
                    kept_ranges.push(Range {
                        line: line.key,
                        lo,
                        hi,
                        sum,
                        hubs: false,
                        content: false,
                    });
                    if l == cell.line {
                        hull = Some((lo, hi));
                    }
                }
                kept_ranges.extend(hub_ranges(&state.lines, hubs, contents, false));
                // Its own system's columns are in its key, and the system
                // next to it shares the column at its edge.
                let beyond = |(lo, hi): (Moment, Moment)| {
                    cell.span
                        .is_none_or(|(a, b)| lo.compare(a).is_lt() || hi.compare(b).is_gt())
                };
                if let (Some((lo, hi)), Some(&l)) =
                    (hull.filter(|&h| beyond(h)), state.by_key.get(&SYSTEMS))
                {
                    let sum = state.lines[l as usize].sum(lo, hi);
                    kept_ranges.push(Range {
                        line: SYSTEMS,
                        lo,
                        hi,
                        sum,
                        hubs: false,
                        content: false,
                    });
                }
            }
        }
        let mut done = Vec::with_capacity(cell.grobs.len());
        for (i, &(_, id)) in cell.grobs.iter().enumerate() {
            if let Some(mut d) = done_of(heap, &mut a, &log, id) {
                // What moves a mark runs again: its parents are the
                // compile's. It flushes the extent on Y and, with one kept,
                // those of the mark's parents, which the cell gives back.
                if i >= cell.own {
                    d.writes.retain(|w| w.0 != moving);
                    d.extents[Y] = None;
                }
                done.push(d);
            }
        }
        if cell.from.is_none() {
            let mut budget = 1 << 16;
            let exactly = done
                .iter()
                .flat_map(|d| &d.writes)
                .any(|&(_, v)| v.is_some_and(|v| prints_exact(heap, v, &mut budget)));
            exact = exactly.then_some(cell.exact);
        }
        by_key.insert(cell.key, cells.len() as u32);
        cells.push(KeptCell {
            grobs: done,
            ranges: kept_ranges,
            exact,
        });
    }
    state.report.kept = cells.len() as u32;
    let names = |heap: &Heap, a: &mut Arena| {
        let mut out: Vec<String> = Vec::new();
        for (what, reader, target, sym) in core::mem::take(&mut grobs(a).tracker.uncovered) {
            let of = |id: u32, a: &Arena| {
                let name = a
                    .records
                    .grobs
                    .get(id as usize)
                    .map_or(Value::NULL, |g| g.name);
                String::from(heap.name(name).unwrap_or("?"))
            };
            let line = format!(
                "{what}\t{}\t{}\t{}",
                of(reader, a),
                of(target, a),
                heap.name(sym).unwrap_or("-")
            );
            if !out.contains(&line) {
                out.push(line);
            }
        }
        out.sort();
        out
    };
    let more = names(heap, &mut a);
    state.report.uncovered.extend(more);
    drop(a);
    let lines = core::mem::take(&mut state.breaking);
    let (before, skylines) = (
        core::mem::take(&mut state.before),
        core::mem::take(&mut state.skylines),
    );
    Some(host.heap().wrap(KeptCells(Rc::new(Cells {
        cells,
        by_key,
        lines,
        before,
        skylines,
        marks,
    }))))
}

/// What `log`, sorted by grob, says the grob `id` got, as it is now.
fn done_of(heap: &Heap, a: &mut Arena, log: &[(u32, Value)], id: u32) -> Option<Done> {
    let at = log.partition_point(|&(i, _)| i < id);
    let mut writes: Vec<(Value, Option<Value>)> = Vec::new();
    let props = a.records.grobs[id as usize].props;
    for &(i, sym) in &log[at..] {
        if i != id {
            break;
        }
        if writes.iter().any(|w| w.0 == sym) {
            continue;
        }
        match grob::assq_ref(heap, sym, props) {
            None => writes.push((sym, None)),
            Some(v) => {
                let mut budget = 4096;
                if portable(heap, v, &mut budget) {
                    writes.push((sym, Some(v)));
                }
            }
        }
    }
    let live = a.records.grobs[id as usize].definition.tag() == Tag::Pair;
    let dims = grobs(a).get(id).map(|g| g.dims)?;
    let t = &grobs(a).tracker;
    Some(Done {
        writes,
        offsets: [dims[X].offset, dims[Y].offset],
        extents: [dims[X].extent, dims[Y].extent],
        group: t.groups[id as usize],
        dead: !live,
    })
}

/// What `pscore`'s cells did in the compile that ran.
pub fn report(heap: &Heap, pscore: Value) -> Report {
    heap.foreign::<PaperScore>(pscore)
        .and_then(|p| p.cells.as_ref().map(|s| s.borrow().report.clone()))
        .unwrap_or_default()
}

/// What line breaking's keys are made of (D249): each grob of the root
/// system after its pieces are made, by the columns it spans. A grob's
/// digest is a cell's ([`digest_of`]); one of no column is in every line's.
struct Ranks {
    /// The output definition's, and those of the grobs of no column.
    base: u64,
    /// The sums of the items' digests up to each rank, `items[r]` those
    /// before rank `r`.
    items: Vec<u64>,
    /// The spanners': all of them, those that end before each rank, and
    /// those that begin at or after it.
    spans: u64,
    ended: Vec<u64>,
    begun: Vec<u64>,
}

impl Ranks {
    fn items(&self, lo: u64, hi: u64) -> u64 {
        let at = |r: u64| self.items[(r as usize).min(self.items.len() - 1)];
        at(hi + 1).wrapping_sub(at(lo))
    }

    fn spans(&self, lo: u64, hi: u64) -> u64 {
        let ended = self.ended[(lo as usize).min(self.ended.len() - 1)];
        let begun = self.begun[(hi as usize + 1).min(self.begun.len() - 1)];
        self.spans.wrapping_sub(ended).wrapping_sub(begun)
    }
}

/// The keys of the lines of a Paper_score's line breaking (D249): a line
/// from one break to another is what its columns' grobs and the spanners
/// that meet it are, for its output definition, and whether it is the
/// score's last (`fill_line_details` reads its spacing then).
pub(crate) struct LineKeys {
    state: Rc<RefCell<State>>,
    ranks: Rc<Ranks>,
}

impl LineKeys {
    /// The key of the line from the column ranked `lo` to the one ranked `hi`.
    pub(crate) fn key(&self, lo: u64, hi: u64, last: bool) -> u64 {
        let r = &self.ranks;
        let h = mix(mix(r.base, u64::from(last)), r.items(lo, hi));
        mix(h, r.spans(lo, hi))
    }

    /// The details a line with `key` had in the last compile, or earlier in
    /// this one; none for a new key. Its last column is the caller's.
    pub(crate) fn kept(&self, key: u64) -> Option<LineDetails> {
        let state = self.state.borrow();
        state
            .breaking
            .get(&key)
            .or_else(|| state.old.as_ref().and_then(|o| o.lines.get(&key)))
            .cloned()
    }

    /// Whether the details of `key` came from the last compile.
    pub(crate) fn is_old(&self, key: u64) -> bool {
        let state = self.state.borrow();
        !state.breaking.contains_key(&key)
            && state
                .old
                .as_ref()
                .is_some_and(|o| o.lines.contains_key(&key))
    }

    /// `details`, filled or taken, as the line of `key` has them in this
    /// compile; `reused` if they were taken from the last, `wrong` if the
    /// check filled them and found them other than taken.
    pub(crate) fn keep(&self, key: u64, details: &LineDetails, reused: bool, wrong: bool) {
        let mut state = self.state.borrow_mut();
        let r = &mut state.report;
        r.lines += 1;
        r.lines_reused += u32::from(reused);
        r.lines_wrong += u32::from(wrong);
        let mut d = details.clone();
        d.last_column = None;
        state.breaking.insert(key, d);
    }

    /// Whether the check is on: a line taken is filled too, and compared.
    pub(crate) fn check(&self) -> bool {
        checking()
    }
}

/// The keys of `pscore`'s lines (D249), made the first time a line breaker
/// asks for them in a compile, its root system's pieces made and its columns
/// ranked; none for a Paper_score that keeps no cells.
pub(crate) fn line_keys(heap: &mut Heap, pscore: Value) -> Option<LineKeys> {
    let state = heap.foreign::<PaperScore>(pscore)?.cells.clone()?;
    let made = state.borrow().ranks.clone();
    if let Some(ranks) = made {
        return Some(LineKeys { state, ranks });
    }
    let root = paper_score::root_system(heap, pscore)?;
    let (arena, _) = grob_of(heap, root)?;
    let sym = heap.intern("all-elements");
    let all = grob::object_data(heap, root, sym);
    let mut base = state.borrow().settings;
    let mut items: Vec<(u64, u64)> = Vec::new();
    let mut spans: Vec<(u64, u64, u64)> = Vec::new();
    for g in grob_array::grobs_of(heap, all) {
        let Some((_, id)) = grob_of(heap, g) else {
            continue;
        };
        let (lo, hi) = crate::item::spanned_column_rank_interval(heap, g);
        let hub = is_line_hub(heap, g);
        let digest = line_digest_of(&state.borrow(), heap, &mut arena.borrow_mut(), id, hub);
        if lo > hi || hi == crate::item::VPOS {
            base = base.wrapping_add(digest);
        } else if lo == hi && !spanner::is_spanner(heap, g) {
            items.push((lo, digest));
        } else {
            spans.push((lo, hi, digest));
        }
    }
    let ranks = 2 + items
        .iter()
        .map(|i| i.0)
        .chain(spans.iter().map(|s| s.1))
        .max()
        .unwrap_or(0) as usize;
    let mut by_rank = alloc::vec![0u64; ranks + 1];
    for (r, d) in items {
        by_rank[r as usize + 1] = by_rank[r as usize + 1].wrapping_add(d);
    }
    for r in 1..by_rank.len() {
        by_rank[r] = by_rank[r].wrapping_add(by_rank[r - 1]);
    }
    let (mut ended, mut begun) = (alloc::vec![0u64; ranks + 1], alloc::vec![0u64; ranks + 1]);
    let mut all_spans = 0u64;
    for &(lo, hi, d) in &spans {
        all_spans = all_spans.wrapping_add(d);
        // Summed from the start, a span has ended before each rank past
        // `hi`; summed from the end, it has begun at or after each rank up
        // to `lo`.
        ended[hi as usize + 1] = ended[hi as usize + 1].wrapping_add(d);
        begun[lo as usize] = begun[lo as usize].wrapping_add(d);
    }
    for r in 1..ended.len() {
        ended[r] = ended[r].wrapping_add(ended[r - 1]);
    }
    for r in (0..begun.len() - 1).rev() {
        begun[r] = begun[r].wrapping_add(begun[r + 1]);
    }
    let ranks = Rc::new(Ranks {
        base,
        items: by_rank,
        spans: all_spans,
        ended,
        begun,
    });
    state.borrow_mut().ranks = Some(ranks.clone());
    Some(LineKeys { state, ranks })
}

/// The moment's bits, for a key.
fn moment_digest(h: u64, m: Moment) -> u64 {
    let h = mix(
        mix(h, m.main.numerator() as u64),
        m.main.denominator() as u64,
    );
    mix(
        mix(h, m.grace.numerator() as u64),
        m.grace.denominator() as u64,
    )
}

/// `read_before_breaking` begins, on the root system `system`: the columns'
/// row made, each line whose grobs, the breaks and what it read are as they
/// were given what it got ([`Before`]), and what is written from here on
/// logged, for [`before_end`].
pub fn before_begin(host: &mut dyn Host, system: Value) -> Result<(), Exception> {
    let pscore = grob::with_grob(host.heap(), system, |g| g.pscore).flatten();
    let Some(keys) = pscore.and_then(|ps| line_keys(host.heap(), ps)) else {
        return Ok(());
    };
    crate::queue::check(crate::queue::Step::Pieces);
    let Some((arena, _)) = grob_of(host.heap(), system) else {
        return Ok(());
    };
    let shared = keys.state;
    let all = {
        let sym = host.heap().intern("all-elements");
        let all = grob::object_data(host.heap(), system, sym);
        grob_array::grobs_of(host.heap(), all)
    };
    // What is break-aligned, by its column's rank.
    let mut aligned: BTreeMap<u64, u64> = BTreeMap::new();
    for &g in &all {
        let heap = host.heap();
        if !(is_break_aligned(heap, g) || grob::has_interface(heap, g, "break-aligned-interface")) {
            continue;
        }
        let (Some((_, id)), Some(column)) = (grob_of(heap, g), crate::item::get_column(heap, g))
        else {
            continue;
        };
        let rank = paper_column::rank(heap, column);
        let d = line_digest_of(&shared.borrow(), heap, &mut arena.borrow_mut(), id, true);
        let e = aligned.entry(rank).or_insert(0);
        *e = e.wrapping_add(d);
    }
    let columns = {
        let sym = host.heap().intern("columns");
        let columns = grob::object_data(host.heap(), system, sym);
        grob_array::grobs_of(host.heap(), columns)
    };
    let mut row = Line {
        key: COLUMNS,
        ..Line::default()
    };
    let mut breaks = 0xcbf2_9ce4_8422_2325u64;
    for &c in &columns {
        let Some((_, id)) = grob_of(host.heap(), c) else {
            continue;
        };
        let rank = paper_column::rank(host.heap(), c);
        let moment = shared.borrow().digests.get(id as usize).map(|d| d.moment);
        let Some(moment) = moment else {
            continue;
        };
        let aligned = aligned.get(&rank).copied().unwrap_or(0);
        row.rows.push((moment, aligned, aligned));
        if paper_column::is_breakable(host, c)? {
            breaks = moment_digest(breaks, moment);
        }
    }
    row.finish();
    crate::queue::check(crate::queue::Step::Pieces);
    let mut state = shared.borrow_mut();
    let state = &mut *state;
    let columns_line = state.lines.len() as u32;
    // What the break alignment is made of has its place among the columns;
    // a column, an alignment or a spacing spanner none (what a column holds
    // is the translation's, and its breaks are in the key).
    let heap = host.heap();
    let mut places = Vec::new();
    for g in all {
        let Some((_, id)) = grob_of(heap, g) else {
            continue;
        };
        if !is_exempt(heap, g) {
            continue;
        }
        let place = match is_break_aligned(heap, g) {
            false => NONE,
            true => {
                let original = grob::original(heap, g).unwrap_or(g);
                let digest =
                    grob_of(heap, original).and_then(|(_, o)| state.digests.get(o as usize));
                digest.map_or(NONE, |d| {
                    let at = row.rows.partition_point(|r| r.0.compare(d.moment).is_lt());
                    at.min(row.rows.len().saturating_sub(1)) as u32
                })
            }
        };
        places.push((id, place));
    }
    state.lines.push(row);
    state.by_key.insert(COLUMNS, columns_line);
    let mut members: Vec<Vec<u32>> = alloc::vec![Vec::new(); columns_line as usize];
    {
        let mut a = arena.borrow_mut();
        let t = &mut grobs(&mut a).tracker;
        t.columns = columns_line;
        for (id, place) in places {
            if let Some(p) = t.places.get_mut(id as usize) {
                *p = place;
            }
        }
        for (id, &l) in t.lines.iter().enumerate() {
            if let Some(m) = members.get_mut(l as usize) {
                m.push(id as u32);
            }
        }
    }
    state.before_keys = alloc::vec![None; columns_line as usize];
    let old = state.old.clone();
    let mut given = Vec::new();
    for (l, ids) in members.iter().enumerate() {
        let line = &state.lines[l];
        let mut key = mix(mix(state.settings, line.key), breaks);
        key = mix(key, line.records.last().copied().unwrap_or(0));
        key = mix(key, ids.len() as u64);
        state.report.before += 1;
        state.report.before_grobs += ids.len() as u32;
        let found = old.as_ref().and_then(|o| o.before.get(&key)).filter(|k| {
            k.grobs.len() == ids.len()
                && k.ranges.iter().all(|r| {
                    let sum = state.by_key.get(&r.line).map_or(0, |&l| {
                        let line = &state.lines[l as usize];
                        line.read(r, &line.records)
                    });
                    sum == r.sum
                })
        });
        let Some(found) = found else {
            state.before_keys[l] = Some(key);
            continue;
        };
        for (&id, b) in ids.iter().zip(&found.grobs) {
            let g = arena.borrow().records.grobs[id as usize].handle;
            give_back_to(host, &arena, g, id, &b.done);
            if let Some(first) = &b.first_place {
                given.push((id, first.clone()));
            }
            let mut a = arena.borrow_mut();
            let slot = grobs(&mut a);
            if let Some(o) = slot.get_mut(id) {
                o.pure_height = b.pure_height;
            }
            for &(key, lo, hi) in &b.reach {
                if let Some(&other) = state.by_key.get(&key) {
                    let rows = &state.lines[other as usize].rows;
                    let from = rows.partition_point(|r| r.0.compare(lo).is_lt());
                    let to = rows.partition_point(|r| r.0.compare(hi).is_le());
                    if key == COLUMNS {
                        if to > from {
                            slot.tracker.deps[id as usize].add_column(from as u32);
                        }
                    } else if to > from {
                        let own = slot.tracker.lines[id as usize];
                        slot.tracker.deps[id as usize].add(own, other, from as u32, to as u32 - 1);
                    }
                }
            }
            for key in &b.hubs {
                if let Some(&other) = state.by_key.get(key) {
                    slot.tracker.deps[id as usize].add_hubs(other);
                }
            }
            for key in &b.contents {
                if let Some(&other) = state.by_key.get(key) {
                    slot.tracker.deps[id as usize].add_contents(other);
                }
            }
        }
        state.before.insert(key, found.clone());
        crate::queue::check(crate::queue::Step::Pieces);
        state.report.before_reused += 1;
        state.report.before_grobs_reused += ids.len() as u32;
    }
    drop(old);
    let diagnostics = host.lily("%diagnostics").unwrap_or(Value::NULL);
    let mut a = arena.borrow_mut();
    let t = &mut grobs(&mut a).tracker;
    t.diagnostics = diagnostics;
    t.marks.clear();
    t.before = Some(shared.clone());
    t.before_log.clear();
    t.rerun_of = NONE;
    t.read_places.clear();
    t.crossed.clear();
    t.given_places = given.into_iter().collect();
    Ok(())
}

/// `read_before_breaking` has ended: what the lines not given back got is
/// kept, those whose callbacks kept to their line.
pub fn before_end(host: &mut dyn Host, system: Value) {
    let Some((arena, _)) = grob_of(host.heap(), system) else {
        return;
    };
    let moved = moved_since_read(host, &arena, system);
    let mut a = arena.borrow_mut();
    grobs(&mut a).tracker.given_places.clear();
    grobs(&mut a).tracker.crossed = Vec::new();
    let Some(shared) = grobs(&mut a).tracker.before.take() else {
        return;
    };
    let heap = host.heap();
    let mut state = shared.borrow_mut();
    let state = &mut *state;
    let mut log = core::mem::take(&mut grobs(&mut a).tracker.before_log);
    log.sort_by_key(|&(id, _)| id);
    let keys = core::mem::take(&mut state.before_keys);
    let mut members: Vec<Vec<u32>> = alloc::vec![Vec::new(); keys.len()];
    for (id, &l) in grobs(&mut a).tracker.lines.iter().enumerate() {
        if let Some(m) = members.get_mut(l as usize) {
            m.push(id as u32);
        }
    }
    for (l, key) in keys.into_iter().enumerate() {
        let Some(key) = key else {
            continue;
        };
        let ids = &members[l];
        let mut flags = 0;
        let mut ranges = BTreeMap::<u32, (u32, u32)>::new();
        let mut cols = Vec::new();
        let (mut hubs, mut contents) = (Vec::new(), Vec::new());
        {
            let t = &grobs(&mut a).tracker;
            for &id in ids {
                let d = &t.deps[id as usize];
                flags |= d.flags;
                cols.extend_from_slice(&d.cols);
                hubs.extend(d.hubs.iter().filter(|&&h| h as usize != l));
                contents.extend(d.contents.iter().filter(|&&h| h as usize != l));
                for &(other, lo, hi) in &d.others {
                    if other as usize != l {
                        let e = ranges.entry(other).or_insert((lo, hi));
                        (e.0, e.1) = (e.0.min(lo), e.1.max(hi));
                    }
                }
            }
        }
        if flags != 0 {
            continue;
        }
        if ids
            .iter()
            .any(|id| moved.get(id).is_some_and(Option::is_none))
        {
            state.report.before_moved += 1;
            continue;
        }
        let moments = |state: &State, l: u32, lo: u32, hi: u32| {
            let rows = &state.lines[l as usize].rows;
            Some((rows.get(lo as usize)?.0, rows.get(hi as usize)?.0))
        };
        cols.sort_unstable();
        cols.dedup();
        let columns = state.by_key.get(&COLUMNS).copied().unwrap_or(NONE);
        let mut kept_ranges: Vec<Range> = ranges
            .iter()
            .filter_map(|(&other, &(lo, hi))| {
                let (lo, hi) = moments(state, other, lo, hi)?;
                let line = &state.lines[other as usize];
                Some(Range {
                    line: line.key,
                    lo,
                    hi,
                    sum: line.records(lo, hi),
                    hubs: false,
                    content: false,
                })
            })
            .collect();
        kept_ranges.extend(hub_ranges(&state.lines, hubs, contents, true));
        for c in cols {
            if let Some((m, _)) = moments(state, columns, c, c) {
                let sum = state.lines[columns as usize].records(m, m);
                kept_ranges.push(Range {
                    line: COLUMNS,
                    lo: m,
                    hi: m,
                    sum,
                    hubs: false,
                    content: false,
                });
            }
        }
        let mut kept = Vec::with_capacity(ids.len());
        for &id in ids {
            let Some(done) = done_of(heap, &mut a, &log, id) else {
                continue;
            };
            let slot = grobs(&mut a);
            let pure_height = slot.get(id).and_then(|g| g.pure_height);
            let d = &slot.tracker.deps[id as usize];
            let mut reach = Vec::new();
            if d.lo <= d.hi
                && let Some((lo, hi)) = moments(state, l as u32, d.lo, d.hi)
            {
                reach.push((state.lines[l].key, lo, hi));
            }
            for &(other, lo, hi) in &d.others {
                if let Some((lo, hi)) = moments(state, other, lo, hi) {
                    reach.push((state.lines[other as usize].key, lo, hi));
                }
            }
            for &c in &d.cols {
                if let Some((m, _)) = moments(state, columns, c, c) {
                    reach.push((COLUMNS, m, m));
                }
            }
            let keys = |lines: &[u32]| lines.iter().map(|&h| state.lines[h as usize].key).collect();
            kept.push(Before {
                done,
                pure_height,
                reach,
                hubs: keys(&d.hubs),
                contents: keys(&d.contents),
                first_place: moved.get(&id).cloned().flatten(),
            });
        }
        state.report.before_places +=
            kept.iter().filter(|b| b.first_place.is_some()).count() as u32;
        state.before.insert(
            key,
            Rc::new(KeptBefore {
                grobs: kept,
                ranges: kept_ranges,
            }),
        );
        state.report.before_kept += 1;
    }
}

/// The grobs of lines that are not where the columns' skylines first read
/// them on Y (OPEN B27, D268), each with that place: a line given back holds
/// what its grobs ended with, and a column computed on it in the next
/// compile would read that, not what a compile from scratch reads (a dot
/// read on a staff line before its column puts it in a space), so it takes
/// the place kept ([`given_place`]). None for one read in two places: its
/// line is not kept. Asked before the cells' state is let go, as a place is
/// by its line's key.
fn moved_since_read(
    host: &mut dyn Host,
    arena: &RefCell<Arena>,
    system: Value,
) -> BTreeMap<u32, Option<Rc<Place>>> {
    let noted = core::mem::take(&mut grobs(&mut arena.borrow_mut()).tracker.read_places);
    let mut moved = BTreeMap::new();
    for (id, noted) in noted {
        let first = match noted {
            Noted::At(place) => place,
            Noted::Moved => {
                moved.insert(id, None);
                continue;
            }
            Noted::Unknown => continue,
        };
        let g = arena.borrow().records.grobs[id as usize].handle;
        let now = separation_item::kept_place(host, g, system);
        if !now.is_some_and(|now| now.same(&first)) {
            moved.insert(id, Some(Rc::new(first)));
        }
    }
    moved
}

/// Where a column's skylines first read `il` on Y in the compile its line was
/// given back from, if it moved after ([`moved_since_read`]).
pub(crate) fn given_place(heap: &Heap, il: Value) -> Option<Rc<Place>> {
    let (arena, id) = grob_of(heap, il)?;
    let mut a = arena.borrow_mut();
    let t = &grobs(&mut a).tracker;
    t.before.as_ref()?;
    t.given_places.get(&id).cloned()
}

/// Whether where the columns' skylines read `il` on Y is noted
/// ([`note_place`]): it is a grob of a line, while `read_before_breaking`
/// runs.
pub(crate) fn notes_places(heap: &Heap, il: Value) -> bool {
    grob_of(heap, il).is_some_and(|(arena, id)| {
        let mut a = arena.borrow_mut();
        let t = &grobs(&mut a).tracker;
        t.before.is_some() && t.lines.get(id as usize).is_some_and(|&l| l != EXEMPT)
    })
}

/// `place`, where a column's skylines read `il` on Y, noted: the first, or
/// that it moved since.
pub(crate) fn note_place(heap: &Heap, il: Value, place: Option<&Place>) {
    let Some((arena, id)) = grob_of(heap, il) else {
        return;
    };
    let mut a = arena.borrow_mut();
    let noted = &mut grobs(&mut a).tracker.read_places;
    let now = match place {
        Some(place) if place.in_line() => Noted::At(place.clone()),
        _ => Noted::Unknown,
    };
    match noted.entry(id) {
        alloc::collections::btree_map::Entry::Vacant(e) => {
            e.insert(now);
        }
        alloc::collections::btree_map::Entry::Occupied(mut e) => {
            if let (Noted::At(was), Noted::At(now)) = (e.get(), &now)
                && !was.same(now)
            {
                e.insert(Noted::Moved);
            }
        }
    }
}

/// The key of the column `me`'s skylines: its record, and what its grobs are.
fn column_key(state: &State, heap: &Heap, a: &mut Arena, rank: u64, id: u32) -> Option<u64> {
    let ranks = state.ranks.as_ref()?;
    let own = line_digest_of(state, heap, a, id, false);
    Some(mix(mix(state.settings, own), ranks.items(rank, rank)))
}

/// The digest of the alignments' pure translations of the whole score and of
/// their own pure coordinates, which what the columns' skylines read of a
/// line's place is.
fn translations(
    host: &mut dyn Host,
    arena: &RefCell<Arena>,
    state: &Rc<RefCell<State>>,
) -> Result<u64, Exception> {
    if let Some(digest) = state.borrow().translations {
        return Ok(digest);
    }
    let alignments = grobs(&mut arena.borrow_mut()).tracker.alignments.clone();
    let mut digest = 0xcbf2_9ce4_8422_2325u64;
    for id in alignments {
        let me = arena.borrow().records.grobs[id as usize].handle;
        if let Some(system) = grob::get_parent(host.heap(), me, Y) {
            let y = crate::unpure::pure_relative_y_coordinate(host, me, system, 0, 2_147_483_647)?;
            digest = mix(digest, y.to_bits());
        }
        for child in grob_array::extract_grob_array(host, me, "elements")? {
            let y = crate::align::get_pure_child_y_translation(host, me, child, 0, 2_147_483_647)?;
            digest = mix(digest, y.to_bits());
        }
    }
    state.borrow_mut().translations = Some(digest);
    Ok(digest)
}

/// The key of the line of the grob `group`, while cells are kept.
pub(crate) fn line_of(heap: &Heap, group: Value) -> Option<u64> {
    let (arena, id) = grob_of(heap, group)?;
    let mut a = arena.borrow_mut();
    let t = &grobs(&mut a).tracker;
    let state = t.before.as_ref()?.borrow();
    Some(state.lines.get(*t.lines.get(id as usize)? as usize)?.key)
}

/// Whether the skylines of `me` being computed are a column's, whose boxes
/// can be kept.
pub(crate) fn keeps_column_skylines(heap: &Heap, me: Value) -> bool {
    grob_of(heap, me).is_some_and(|(arena, id)| {
        let mut a = arena.borrow_mut();
        let t = &grobs(&mut a).tracker;
        let column = t.lines.get(id as usize) == Some(&EXEMPT);
        column && t.before.as_ref().is_some_and(|s| s.borrow().boxes < BOXES)
    })
}

/// The column `me`'s `horizontal-skylines` as the last compile had them, if
/// their key and what they read are the same; if the translations differ,
/// made again from what stayed of their boxes. None else, and for what is no
/// column while `read_before_breaking` runs.
pub(crate) fn kept_column_skylines(
    host: &mut dyn Host,
    me: Value,
) -> Result<Option<Value>, Exception> {
    let Some((arena, id)) = grob_of(host.heap(), me) else {
        return Ok(None);
    };
    let shared = {
        let mut a = arena.borrow_mut();
        let t = &grobs(&mut a).tracker;
        match (&t.before, t.lines.get(id as usize)) {
            (Some(state), Some(&EXEMPT)) => state.clone(),
            _ => return Ok(None),
        }
    };
    let rank = paper_column::rank(host.heap(), me);
    let key = {
        let mut a = arena.borrow_mut();
        column_key(&shared.borrow(), host.heap(), &mut a, rank, id)
    };
    let found = {
        let state = shared.borrow();
        let found = key.and_then(|k| state.old.as_ref()?.skylines.get(&k).cloned());
        found.filter(|k| {
            k.ranges.iter().all(|r| {
                let sum = state.by_key.get(&r.line).map_or(0, |&l| {
                    let line = &state.lines[l as usize];
                    line.read(r, &line.records)
                });
                sum == r.sum
            }) && k.wrote.iter().all(|key| {
                // Given back: [`before_begin`] keyed only the lines it did
                // not give back.
                state.by_key.get(key).is_some_and(|&l| {
                    state
                        .before_keys
                        .get(l as usize)
                        .is_some_and(Option::is_none)
                })
            })
        })
    };
    let (Some(key), Some(found)) = (key, found) else {
        return Ok(None);
    };
    if let Some(digest) = found.translations {
        let now = translations(host, &arena, &shared)?;
        if digest != now {
            let Some((boxes, padding)) = found.boxes.clone() else {
                return Ok(None);
            };
            let groups = shared.borrow().groups.clone();
            let group = |line: u64| groups.get(&line).copied();
            let again = separation_item::skylines_again(host, me, &boxes, padding, &group)?;
            let Some(value) = again else {
                return Ok(None);
            };
            let mut state = shared.borrow_mut();
            state.boxes += boxes.len();
            let kept = KeptSkylines {
                value,
                ranges: found.ranges.clone(),
                translations: Some(now),
                boxes: found.boxes.clone(),
                wrote: found.wrote.clone(),
            };
            state.skylines.insert(key, Rc::new(kept));
            state.report.skylines += 1;
            state.report.skylines_reused += 1;
            state.report.skylines_moved += 1;
            return Ok(Some(value));
        }
    }
    let value = found.value;
    let mut state = shared.borrow_mut();
    state.boxes += found.boxes.as_ref().map_or(0, |b| b.0.len());
    state.skylines.insert(key, found);
    state.report.skylines += 1;
    state.report.skylines_reused += 1;
    Ok(Some(value))
}

/// The column `me`'s `horizontal-skylines` computed, `value`: kept by their
/// key, what they read with them, if they kept to reads a next compile can
/// check.
pub(crate) fn keep_column_skylines(
    host: &mut dyn Host,
    me: Value,
    value: Value,
    boxes: Option<(KeptBoxes, f64)>,
    crossed: usize,
) -> Result<(), Exception> {
    let Some((arena, id)) = grob_of(host.heap(), me) else {
        return Ok(());
    };
    let (shared, flags) = {
        let mut a = arena.borrow_mut();
        let t = &grobs(&mut a).tracker;
        match (&t.before, t.lines.get(id as usize)) {
            (Some(state), Some(&EXEMPT)) => (state.clone(), t.deps[id as usize].flags),
            _ => return Ok(()),
        }
    };
    shared.borrow_mut().report.skylines += 1;
    if flags & !PURE_Y != 0 {
        return Ok(());
    }
    let reads = flags & PURE_Y != 0;
    let digest = if reads {
        Some(translations(host, &arena, &shared)?)
    } else {
        None
    };
    let rank = paper_column::rank(host.heap(), me);
    let mut a = arena.borrow_mut();
    let key = column_key(&shared.borrow(), host.heap(), &mut a, rank, id);
    let Some(key) = key else {
        return Ok(());
    };
    let t = &grobs(&mut a).tracker;
    let d = &t.deps[id as usize];
    let mut state = shared.borrow_mut();
    let mut wrote: Vec<u64> = t
        .crossed
        .get(crossed..)
        .unwrap_or(&[])
        .iter()
        .filter_map(|&l| state.lines.get(l as usize).map(|line| line.key))
        .collect();
    wrote.sort_unstable();
    wrote.dedup();
    let mut ranges = Vec::with_capacity(d.others.len() + d.cols.len());
    if let Some(&columns) = state.by_key.get(&COLUMNS) {
        let row = &state.lines[columns as usize];
        for &c in &d.cols {
            if let Some(&(m, _, _)) = row.rows.get(c as usize) {
                ranges.push(Range {
                    line: COLUMNS,
                    lo: m,
                    hi: m,
                    sum: row.records(m, m),
                    hubs: false,
                    content: false,
                });
            }
        }
    }
    for &(l, lo, hi) in &d.others {
        let line = &state.lines[l as usize];
        if let (Some(lo), Some(hi)) = (line.rows.get(lo as usize), line.rows.get(hi as usize)) {
            let (lo, hi) = (lo.0, hi.0);
            ranges.push(Range {
                line: line.key,
                lo,
                hi,
                sum: line.records(lo, hi),
                hubs: false,
                content: false,
            });
        }
    }
    ranges.extend(hub_ranges(
        &state.lines,
        d.hubs.clone(),
        d.contents.clone(),
        true,
    ));
    let boxes = boxes.map(|(mut boxes, padding)| {
        boxes.shrink();
        state.boxes += boxes.len();
        (Rc::new(boxes), padding)
    });
    state.skylines.insert(
        key,
        Rc::new(KeptSkylines {
            value,
            ranges,
            translations: digest,
            boxes,
            wrote,
        }),
    );
    Ok(())
}

/// Where [`Tracker::crossed`] is: what is written to a line from another grob
/// after, while the column `me`'s skylines are computed, goes with them
/// ([`keep_column_skylines`]).
pub(crate) fn crossed_mark(heap: &Heap, me: Value) -> usize {
    grob_of(heap, me).map_or(0, |(arena, _)| {
        grobs(&mut arena.borrow_mut()).tracker.crossed.len()
    })
}

/// What has been said, while `read_before_breaking` runs (none else): the
/// error port's length and the last of `(lily)`'s diagnostics.
pub(crate) fn said_mark(host: &mut dyn Host, arena: &RefCell<Arena>) -> Option<(usize, u64)> {
    let cell = {
        let mut a = arena.borrow_mut();
        let t = &grobs(&mut a).tracker;
        t.before.as_ref()?;
        t.diagnostics
    };
    let errors = host.error_len();
    let last = host.heap().car(cell).map_or(0, Value::to_bits);
    Some((errors, last))
}

/// The callback of the grob `id` that began at `mark` has ended: if it
/// said something, its grob's line or column is not kept (the readers it
/// ran for take the flag from it).
pub(crate) fn said_since(
    host: &mut dyn Host,
    arena: &RefCell<Arena>,
    id: u32,
    mark: Option<(usize, u64)>,
) {
    let Some(mark) = mark else {
        return;
    };
    if said_mark(host, arena).is_some_and(|now| now != mark)
        && let Some(d) = grobs(&mut arena.borrow_mut())
            .tracker
            .deps
            .get_mut(id as usize)
    {
        d.flags |= SAID;
    }
}

/// [`said_mark`] for a callback the Scheme runs (`%grob-enter`), kept until
/// [`grob_left`]; `errors` the error port's length.
pub(crate) fn grob_entered(heap: &Heap, a: &mut Arena, id: u32, errors: usize) {
    let t = &mut grobs(a).tracker;
    if t.before.is_some() {
        let last = heap.car(t.diagnostics).map_or(0, Value::to_bits);
        t.marks.push((id, (errors, last)));
    }
}

/// [`said_since`] for a callback the Scheme ran (`%grob-leave`).
pub(crate) fn grob_left(heap: &Heap, a: &mut Arena, id: u32, errors: usize) {
    let t = &mut grobs(a).tracker;
    while let Some((at, mark)) = t.marks.pop() {
        if at == id {
            let last = heap.car(t.diagnostics).map_or(0, Value::to_bits);
            if (errors, last) != mark
                && let Some(d) = t.deps.get_mut(id as usize)
            {
                d.flags |= SAID;
            }
            break;
        }
    }
}

/// `read_before_breaking` reads the grob `g`'s `before-line-breaking` or
/// `springs-and-rods`: in every compile, whatever was
/// given back, so what its callback writes to itself is written again and
/// not kept, and what it writes to a column is not a line's.
pub(crate) fn rerunning(heap: &Heap, g: Value) {
    let Some((arena, id)) = grob_of(heap, g) else {
        return;
    };
    let mut a = arena.borrow_mut();
    let t = &mut grobs(&mut a).tracker;
    if t.before.is_some() {
        t.rerun_of = id;
    }
}

/// [`rerunning`] has ended for what `read_before_breaking` read of `g`.
pub(crate) fn rerun_done(heap: &Heap, g: Value) {
    if let Some((arena, _)) = grob_of(heap, g) {
        grobs(&mut arena.borrow_mut()).tracker.rerun_of = NONE;
    }
}
